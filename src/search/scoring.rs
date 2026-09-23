//! Deterministic tiered scoring (M02.2).
//!
//! # Tiers (highest → lowest)
//!
//! The requirement asks for absolute tier boundaries in this order:
//!
//! 1. `Name` — exact / prefix / contains on the display name;
//! 2. `Alias` — any strategy on an alias;
//! 3. `PinyinEng` — pinyin full / pinyin initials / english initials;
//! 4. `Tags`;
//! 5. `Category`;
//! 6. `Path`;
//! 7. `Note`;
//! 8. `Subsequence` — ordered non-contiguous match on any field;
//! 9. `EditDistance` — whole-key Levenshtein.
//!
//! A hit in a higher tier ALWAYS outranks a hit in a lower tier regardless of
//! raw weights. Within a tier the secondary criterion is the strategy points
//! (exact 7 > prefix 6 > contains 5 > pinyin full 4 > pinyin initial 3 =
//! english initials 3 > subsequence 2 > edit distance 1). Neither
//! `manual_weight` nor `pinned` can move a result across a tier: both are
//! bounded values packed into a dedicated low bit range of the total score.
//!
//! # Total score layout (u64)
//!
//! ```text
//! bits 56..60  quality index (higher = better), inverted from the tier
//! bits 40..55  per-token strategy points sum (16 bits)
//! bits 20..37  within-tier bonus = manual-weight bias + pinned (18 bits)
//! bits 0..19   reserved (0)
//! ```
//!
//! The layout is monotone in (quality index, points, bonus) so sorting by the
//! single `total_score` descending is equivalent to sorting by that triple.
//! Exact ties fall through to the documented [`tiebreak`], which never uses
//! time or randomness, so repeated identical queries produce bit-identical
//! orderings.

use super::{
    matching::MatchStrategy,
    search_entry::{SearchEntry, SearchField, SearchHit, SearchScore},
};

/// Tier of a (field, strategy) hit — lower is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Tier {
    Name = 0,
    Alias = 1,
    PinyinEng = 2,
    Tags = 3,
    Category = 4,
    Path = 5,
    Note = 6,
    Subsequence = 7,
    EditDistance = 8,
}

/// All tiers, ordered from best (Name) to worst (EditDistance).
///
/// The score layout uses an inverted "quality index" (higher = better) so that
/// the single `total_score` sorts descending into the correct rank.
const ALL_TIERS_BEST_FIRST: [Tier; 9] = [
    Tier::Name,
    Tier::Alias,
    Tier::PinyinEng,
    Tier::Tags,
    Tier::Category,
    Tier::Path,
    Tier::Note,
    Tier::Subsequence,
    Tier::EditDistance,
];

/// Map a tier to a quality index (higher = better) for the score layout.
fn quality_index(tier: Tier) -> u64 {
    ALL_TIERS_BEST_FIRST
        .iter()
        .position(|candidate| *candidate == tier)
        .map_or(0, |position| {
            (ALL_TIERS_BEST_FIRST.len() - 1 - position) as u64
        })
}

/// The tier a (field, strategy) hit belongs to.
pub(crate) fn tier_of(field: SearchField, strategy: MatchStrategy) -> Tier {
    match strategy {
        MatchStrategy::Exact | MatchStrategy::Prefix | MatchStrategy::Contains => match field {
            SearchField::Name => Tier::Name,
            SearchField::Alias(_) => Tier::Alias,
            SearchField::Tag(_) => Tier::Tags,
            SearchField::Category => Tier::Category,
            SearchField::Path => Tier::Path,
            SearchField::Note => Tier::Note,
        },
        MatchStrategy::PinyinFull
        | MatchStrategy::PinyinInitial
        | MatchStrategy::EnglishInitials => Tier::PinyinEng,
        MatchStrategy::Subsequence => Tier::Subsequence,
        MatchStrategy::EditDistance => Tier::EditDistance,
    }
}

/// Within-tier strategy points (higher = better).
fn strategy_points(strategy: MatchStrategy) -> u64 {
    match strategy {
        MatchStrategy::Exact => 7,
        MatchStrategy::Prefix => 6,
        MatchStrategy::Contains => 5,
        MatchStrategy::PinyinFull => 4,
        MatchStrategy::PinyinInitial | MatchStrategy::EnglishInitials => 3,
        MatchStrategy::Subsequence => 2,
        MatchStrategy::EditDistance => 1,
    }
}

/// Public point weights (informational; tiers trump them).
pub mod weights {
    pub const NAME_EXACT_POINTS: u64 = 7;
    pub const NAME_PREFIX_POINTS: u64 = 6;
    pub const NAME_CONTAINS_POINTS: u64 = 5;
    pub const PINYIN_FULL_POINTS: u64 = 4;
    pub const PINYIN_INITIAL_POINTS: u64 = 3;
    pub const ENGLISH_INITIALS_POINTS: u64 = 3;
    pub const SUBSEQUENCE_POINTS: u64 = 2;
    pub const EDIT_DISTANCE_POINTS: u64 = 1;
    /// Bounded within-tier bonus for a pinned entry (cannot cross a tier).
    pub const PINNED_BONUS: u64 = 10_000;
    /// Max reachable within-tier manual-weight bias.
    pub const MAX_MANUAL_WEIGHT_BIAS: u64 = 200_000;
}

use crate::domain::folder::{MAX_MANUAL_WEIGHT, MIN_MANUAL_WEIGHT};

/// Bit layout constants.
const TIER_SHIFT: u64 = 56;
const POINTS_SHIFT: u64 = 40;
const BONUS_SHIFT: u64 = 20;
const POINTS_MAX: u64 = (1 << 16) - 1;
const BONUS_MAX: u64 = (1 << 18) - 1;

/// Bounded manual-weight bias in [0, 200_000]. `manual_weight` is in
/// [MIN_MANUAL_WEIGHT, MAX_MANUAL_WEIGHT] = [-100, 100].
pub(crate) fn manual_weight_bias(manual_weight: i16) -> u64 {
    let weight =
        i32::from(manual_weight).clamp(i32::from(MIN_MANUAL_WEIGHT), i32::from(MAX_MANUAL_WEIGHT));
    u64::try_from(weight + 100).unwrap_or(u64::MAX) * 1000
}

/// Build the `SearchScore` for a per-token best-hit set.
pub(crate) fn build_score(entry: &SearchEntry, hits: &[super::matching::FieldCoin]) -> SearchScore {
    let token_count = hits.len();
    let mut hits_out = Vec::with_capacity(token_count);
    let mut points_sum = 0u64;
    let mut best_tier: Option<Tier> = None;

    for coin in hits {
        let strategy = coin.strategy;
        let field = coin.field;
        hits_out.push(SearchHit {
            field,
            strategy: strategy.as_str(),
            score: strategy_points(strategy),
            range: coin.range_chars,
        });
        points_sum += strategy_points(strategy);
        best_tier = Some(match best_tier {
            // The result tier is the WORST tier among the matched tokens: the
            // weakest link dominates the result rank. Every token still
            // contributes its points.
            Some(current) => current.max(tier_of(field, strategy)),
            None => tier_of(field, strategy),
        });
    }

    let tier_bits = quality_index(best_tier.unwrap_or(Tier::EditDistance)) << TIER_SHIFT;
    let points_bits = (points_sum.min(POINTS_MAX)) << POINTS_SHIFT;
    let within_bonus = manual_weight_bias(entry.manual_weight)
        .saturating_add(if entry.pinned {
            weights::PINNED_BONUS
        } else {
            0
        })
        .min(BONUS_MAX);
    let bonus_bits = within_bonus << BONUS_SHIFT;

    let total_score = tier_bits | points_bits | bonus_bits;

    SearchScore {
        matched_token_count: token_count,
        token_count,
        total_score,
        hits: hits_out,
    }
}

/// Candidate-sorting key used while picking the best hit for one token.
///
/// Lower is better; the order mirrors the final rank: tier first, then
/// strategy points. `origin_order` is a secondary tie-break so that two hits
/// of equal (tier, points) resolve deterministically (e.g. an earlier alias
/// beats a later alias).
pub(crate) fn candidate_rank(field: SearchField, strategy: MatchStrategy) -> u64 {
    // Lower is better. Within a tier the points are inverted: Exact (7 points)
    // becomes the smallest within-tier value, so `min_by_key` picks it.
    let within_tier = 8u64 - strategy_points(strategy);
    let rank = ((tier_of(field, strategy) as u64) << 8) | within_tier;
    // Stable secondary ensures equal (tier, points) collisions resolve by
    // field order rather than input order.
    (rank << 16) | origin_order(field)
}

fn origin_order(field: SearchField) -> u64 {
    match field {
        SearchField::Name => 0,
        SearchField::Alias(index) => (index + 1) as u64,
        SearchField::Path => 0,
        SearchField::Category => 1,
        SearchField::Tag(index) => (index + 2) as u64,
        SearchField::Note => u64::MAX >> 8,
    }
}

/// Stable, documented tie-break for two entries whose `total_score` is zapped
/// equal. Never time- or random-dependent.
///
/// Order: manual_weight descending → pinned (true first) → open_count
/// descending → entry id ascending. `manual_weight` here is the final
/// within-tier discriminator and cannot cross a tier.
pub(crate) fn tiebreak(left: &SearchEntry, right: &SearchEntry) -> std::cmp::Ordering {
    right
        .manual_weight
        .cmp(&left.manual_weight)
        .then_with(|| right.pinned.cmp(&left.pinned))
        .then_with(|| right.open_count.cmp(&left.open_count))
        .then_with(|| left.id.as_uuid().cmp(&right.id.as_uuid()))
}

#[cfg(test)]
mod tests {
    use super::{Tier, candidate_rank, manual_weight_bias, tiebreak, tier_of, weights};
    use crate::domain::ids::FolderId;
    use crate::search::{
        filter::{Accessibility, Origin},
        matching::MatchStrategy,
        search_entry::SearchEntry,
        search_entry::SearchField,
    };
    use std::cmp::Ordering;
    use uuid::Uuid;

    /// Entry with a single difference from a pair of canonical same-score
    /// entries: `id` = 100 + `offset`, everything else zeroed.
    fn entry(offset: u128) -> SearchEntry {
        SearchEntry {
            id: FolderId::from_uuid(Uuid::from_u128(100 + offset)),
            display_name: "same".to_owned(),
            aliases: Vec::new(),
            path: "C:\\same".to_owned(),
            category_name: None,
            tag_names: Vec::new(),
            note: String::new(),
            pinned: false,
            favorite: false,
            manual_weight: 0,
            open_count: 0,
            last_opened_at: None,
            accessibility: Accessibility::Unknown,
            origin: Origin::Unknown,
        }
    }

    /// Differs from the canonical entry by the given `manual_weight`.
    fn weighted(offset: u128, manual_weight: i16) -> SearchEntry {
        let mut e = entry(offset);
        e.manual_weight = manual_weight;
        e
    }

    /// Asserts the full documented tie-break order between two entries.
    fn assert_order(left: &SearchEntry, right: &SearchEntry, expect: Ordering) {
        assert_eq!(tiebreak(left, right), expect);
    }

    #[test]
    fn tier_rules_match_required_weight_order() {
        assert_eq!(tier_of(SearchField::Name, MatchStrategy::Exact), Tier::Name);
        assert_eq!(
            tier_of(SearchField::Name, MatchStrategy::Prefix),
            Tier::Name
        );
        assert_eq!(
            tier_of(SearchField::Name, MatchStrategy::Contains),
            Tier::Name
        );
        assert_eq!(
            tier_of(SearchField::Alias(0), MatchStrategy::Exact),
            Tier::Alias
        );
        assert_eq!(
            tier_of(SearchField::Alias(0), MatchStrategy::Contains),
            Tier::Alias
        );
        assert_eq!(
            tier_of(SearchField::Name, MatchStrategy::PinyinFull),
            Tier::PinyinEng
        );
        assert_eq!(
            tier_of(SearchField::Name, MatchStrategy::PinyinInitial),
            Tier::PinyinEng
        );
        assert_eq!(
            tier_of(SearchField::Alias(0), MatchStrategy::EnglishInitials),
            Tier::PinyinEng
        );
        assert_eq!(
            tier_of(SearchField::Tag(0), MatchStrategy::Contains),
            Tier::Tags
        );
        assert_eq!(
            tier_of(SearchField::Category, MatchStrategy::Contains),
            Tier::Category
        );
        assert_eq!(
            tier_of(SearchField::Path, MatchStrategy::Contains),
            Tier::Path
        );
        assert_eq!(
            tier_of(SearchField::Note, MatchStrategy::Contains),
            Tier::Note
        );
        assert_eq!(
            tier_of(SearchField::Name, MatchStrategy::Subsequence),
            Tier::Subsequence
        );
        assert_eq!(
            tier_of(SearchField::Name, MatchStrategy::EditDistance),
            Tier::EditDistance
        );
    }

    #[test]
    fn tiers_are_strictly_ordered_and_never_overlap() {
        let mut previous: Option<Tier> = None;
        for tier in [
            Tier::Name,
            Tier::Alias,
            Tier::PinyinEng,
            Tier::Tags,
            Tier::Category,
            Tier::Path,
            Tier::Note,
            Tier::Subsequence,
            Tier::EditDistance,
        ] {
            if let Some(prev) = previous {
                assert!(prev < tier, "{prev:?} must outrank {tier:?}");
            }
            previous = Some(tier);
        }
    }

    #[test]
    fn manual_weight_bias_is_bounded_and_monotone() {
        assert_eq!(manual_weight_bias(-100), 0);
        assert_eq!(manual_weight_bias(0), 100_000);
        assert_eq!(manual_weight_bias(100), 200_000);
        // Out-of-range values are clamped.
        assert_eq!(manual_weight_bias(-500), 0);
        assert_eq!(manual_weight_bias(500), 200_000);
        assert!(weights::MAX_MANUAL_WEIGHT_BIAS >= manual_weight_bias(i16::MIN));
        assert!(weights::MAX_MANUAL_WEIGHT_BIAS >= manual_weight_bias(i16::MAX));
    }

    #[test]
    fn candidate_rank_is_monotone_in_tier_then_points() {
        let name_exact = candidate_rank(SearchField::Name, MatchStrategy::Exact);
        let name_prefix = candidate_rank(SearchField::Name, MatchStrategy::Prefix);
        let alias_exact = candidate_rank(SearchField::Alias(0), MatchStrategy::Exact);
        let pinyin = candidate_rank(SearchField::Name, MatchStrategy::PinyinFull);
        let path = candidate_rank(SearchField::Path, MatchStrategy::Contains);
        let subsequence = candidate_rank(SearchField::Name, MatchStrategy::Subsequence);
        let edited = candidate_rank(SearchField::Name, MatchStrategy::EditDistance);

        assert!(name_exact < name_prefix);
        assert!(name_prefix < alias_exact);
        assert!(alias_exact < pinyin);
        assert!(pinyin < path);
        assert!(path < subsequence);
        assert!(subsequence < edited);
    }

    #[test]
    fn tiebreak_sorts_manual_weight_descending_then_pinned() {
        // manual_weight descending: higher weight first.
        assert_order(&weighted(1, 100), &weighted(2, 0), Ordering::Less);
        assert_order(&weighted(1, -100), &weighted(2, 0), Ordering::Greater);
        // Equal weight falls through to pinned: pinned (true) first.
        let mut pinned_left = entry(1);
        pinned_left.pinned = true;
        let mut pinned_right = entry(2);
        pinned_right.pinned = true;
        assert_order(&pinned_left, &entry(2), Ordering::Less);
        assert_order(&entry(1), &pinned_right, Ordering::Greater);
    }

    #[test]
    fn tiebreak_sorts_open_count_descending_then_id_ascending() {
        // open_count descending: higher count first.
        let mut open_a = entry(1);
        open_a.open_count = 100;
        let mut open_b = entry(2);
        open_b.open_count = 0;
        assert_order(&open_a, &open_b, Ordering::Less);
        assert_order(&open_b, &open_a, Ordering::Greater);
        // Equal counts fall through to entry id ascending: lower id first.
        assert_order(&entry(1), &entry(2), Ordering::Less);
        assert_order(&entry(2), &entry(1), Ordering::Greater);
    }

    #[test]
    fn tiebreak_order_is_total_and_consistent() {
        // The full documented order: manual_weight desc → pinned first →
        // open_count desc → id ascending, exercised in one comparison chain.
        let mut most_used = entry(1);
        most_used.open_count = 10;
        let mut pinned_star = entry(2);
        pinned_star.pinned = true;
        let mut heavy = entry(3);
        heavy.manual_weight = 100;
        // heavy (weight) < pinned_star (pinned, then id) < most_used (open_count,
        // then id) < entry(4).
        assert_order(&heavy, &pinned_star, Ordering::Less);
        assert_order(&pinned_star, &most_used, Ordering::Less);
        assert_order(&most_used, &entry(4), Ordering::Less);
        // And antisymmetric: pair flipped gives the mirrored result.
        assert_order(&pinned_star, &heavy, Ordering::Greater);
        assert_order(&most_used, &pinned_star, Ordering::Greater);
        assert_order(&entry(4), &most_used, Ordering::Greater);
    }
}
