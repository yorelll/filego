//! Matching strategies (M02.1).
//!
//! A *strategy* is one way to match a query token against one derived key.
//! The engine tries every applicable strategy on every enabled field and keeps
//! the best hit per token. The strategies, grouped by the key they run against:
//!
//! * `Exact` / `Prefix` / `Contains` — contiguous character matches on the
//!   case-folded key (Exact requires a full-key match, Prefix requires the
//!   match to start at the first char).
//! * `PinyinFull` / `PinyinInitial` / `EnglishInitials` — contiguous matches
//!   against the corresponding derived key (names/aliases only).
//! * `Subsequence` — ordered, non-contiguous characters on the case-folded key.
//! * `EditDistance` — typo tolerance: Levenshtein within
//!   `settings.max_edit_distance` of any word of the folded key (or of the
//!   whole key).
//!
//! Every range is computed in **characters** of the ORIGINAL display string
//! (never bytes); multi-char derived keys map back through each key's `origin`
//! list so no UTF-8 character is ever split.

use crate::domain::settings::AppSettings;

use super::{
    keys::{FieldKeys, MappedText},
    query::{Query, Token, TokenKind},
    scoring,
    search_entry::SearchField,
};

/// The strategy that produced a hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MatchStrategy {
    Exact,
    Prefix,
    Contains,
    PinyinFull,
    PinyinInitial,
    EnglishInitials,
    Subsequence,
    EditDistance,
}

impl MatchStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            MatchStrategy::Exact => "exact",
            MatchStrategy::Prefix => "prefix",
            MatchStrategy::Contains => "contains",
            MatchStrategy::PinyinFull => "pinyin_full",
            MatchStrategy::PinyinInitial => "pinyin_initial",
            MatchStrategy::EnglishInitials => "english_initials",
            MatchStrategy::Subsequence => "subsequence",
            MatchStrategy::EditDistance => "edit_distance",
        }
    }
}

/// One field's best hit for one token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FieldCoin {
    pub(crate) field: SearchField,
    pub(crate) strategy: MatchStrategy,
    /// Range in the ORIGINAL source string (char indices, half-open). `None`
    /// for strategies that do not map to a single span (edit distance).
    pub(crate) range_chars: Option<(usize, usize)>,
}

/// Byte-level substring search returning char indices of the key string.
fn find_contiguous(haystack: &str, needle: &str) -> Option<(usize, usize)> {
    if needle.is_empty() || haystack.is_empty() {
        return None;
    }
    let byte_start = haystack.find(needle)?;
    let start = haystack[..byte_start].chars().count();
    let end = start + needle.chars().count();
    Some((start, end))
}

/// Map a key span back to original source chars via the key's `origins`.
fn map_key_range(key: &MappedText, key_start: usize, key_end: usize) -> Option<(usize, usize)> {
    if key_start >= key_end || key_start >= key.origins.len() {
        return None;
    }
    let first_origin = key.origins[key_start];
    let last_origin = key.origins[key_end - 1];
    Some((first_origin, last_origin + 1))
}

fn exact(key: &MappedText, pattern: &str) -> Option<(usize, usize)> {
    let (start, end) = find_contiguous(&key.text, pattern)?;
    if start == 0 && end == key.text.chars().count() {
        map_key_range(key, start, end)
    } else {
        None
    }
}

fn prefix(key: &MappedText, pattern: &str) -> Option<(usize, usize)> {
    let (start, end) = find_contiguous(&key.text, pattern)?;
    if start == 0 {
        map_key_range(key, start, end)
    } else {
        None
    }
}

fn contains(key: &MappedText, pattern: &str) -> Option<(usize, usize)> {
    let (start, end) = find_contiguous(&key.text, pattern)?;
    map_key_range(key, start, end)
}

/// Ordered, non-contiguous subsequence match. Returns a source range spanning
/// from the first matched key char to the last matched key char.
fn subsequence(key: &MappedText, pattern: &str) -> Option<(usize, usize)> {
    if pattern.is_empty() {
        return None;
    }
    let mut pattern_iter = pattern.chars();
    let mut current_pattern = pattern_iter.next()?;
    let mut matched = 0usize;
    let mut first_origin = None;
    let mut last_origin = 0usize;
    let mut matched_all = false;

    for (key_index, key_char) in key.text.chars().enumerate() {
        if key_char == current_pattern {
            if matched == 0 {
                first_origin = Some(key.origins[key_index]);
            }
            matched += 1;
            last_origin = key.origins[key_index];
            match pattern_iter.next() {
                Some(next) => current_pattern = next,
                None => {
                    matched_all = true;
                    break;
                }
            }
        }
    }

    if !matched_all {
        return None;
    }
    let start = first_origin?;
    Some((start, last_origin + 1))
}

/// Char-based Levenshtein distance between `a` and `b` (no length cap).
fn levenshtein(a: &str, b: &str) -> u64 {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = b_chars.len();

    let mut previous: Vec<u64> = (0..=m as u64).collect();
    let mut current = vec![0u64; m + 1];
    for (i, x) in a_chars.iter().enumerate() {
        current[0] = i as u64 + 1;
        for (j, y) in b_chars.iter().enumerate() {
            let cost = if x == y { 0 } else { 1 };
            current[j + 1] = (previous[j + 1] + 1)
                .min(current[j] + 1)
                .min(previous[j] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[m]
}

/// Typo tolerance: the pattern matches if it is within `max` edits of any
/// whitespace-separated WORD of the folded key, or of the whole key. This is
/// the useful "编辑距离容错" semantic for multi-word names: "driber" within 1
/// of "driver", and a single-word key compares as a whole. `max` is capped by
/// `AppSettings::MAX_EDIT_DISTANCE` (2) at the settings layer.
fn edit_distance_match(pattern: &str, key_text: &str, max: u64) -> bool {
    if max == 0 {
        // At distance 0 the substring strategy already covers exact words; an
        // exact whole-key match is also covered by the Exact strategy.
        return false;
    }
    if levenshtein(pattern, key_text) <= max {
        return true;
    }
    key_text
        .split_whitespace()
        .any(|word| levenshtein(pattern, word) <= max)
}

fn coin(field: SearchField, strategy: MatchStrategy, range: Option<(usize, usize)>) -> FieldCoin {
    FieldCoin {
        field,
        strategy,
        range_chars: range,
    }
}

/// Adopt a candidate if its quality beats the current best.
fn adopt(best: &mut Option<FieldCoin>, candidate: FieldCoin) {
    let better = best.as_ref().is_none_or(|current| {
        scoring::candidate_rank(candidate.field, candidate.strategy)
            < scoring::candidate_rank(current.field, current.strategy)
    });
    if better {
        *best = Some(candidate);
    }
}

/// Try the contiguous strategy `strategy` on `key` and adopt the best hit.
fn try_contiguous(
    field: SearchField,
    key: &MappedText,
    strategy: MatchStrategy,
    pattern: &str,
    best: &mut Option<FieldCoin>,
) {
    let range = match strategy {
        MatchStrategy::Exact => exact(key, pattern),
        MatchStrategy::Prefix => prefix(key, pattern),
        _ => contains(key, pattern),
    };
    if let Some(range) = range {
        adopt(best, coin(field, strategy, Some(range)));
    }
}

/// Match one token against one field's derived keys, returning the best hit.
pub(crate) fn match_token_against_field(
    token: &Token,
    field: &FieldKeys,
    settings: &AppSettings,
) -> Option<FieldCoin> {
    if !field.is_enabled() {
        return None;
    }

    let pattern = token.text();
    if pattern.is_empty() {
        return None;
    }
    let is_latin = token.kind() == TokenKind::Latin;
    let fuzzy = settings.fuzzy_matching;
    let mut best: Option<FieldCoin> = None;

    // Text strategies on the case-folded key.
    let field_tag = field.field();
    try_contiguous(
        field_tag,
        field.folded(),
        MatchStrategy::Exact,
        pattern,
        &mut best,
    );
    try_contiguous(
        field_tag,
        field.folded(),
        MatchStrategy::Prefix,
        pattern,
        &mut best,
    );
    try_contiguous(
        field_tag,
        field.folded(),
        MatchStrategy::Contains,
        pattern,
        &mut best,
    );

    // Ordered-subsequence on the case-folded key (fuzzy).
    if fuzzy && let Some(range) = subsequence(field.folded(), pattern) {
        adopt(
            &mut best,
            coin(field_tag, MatchStrategy::Subsequence, Some(range)),
        );
    }

    // Edit distance (typo tolerance) on the folded key (fuzzy).
    if fuzzy
        && edit_distance_match(
            pattern,
            &field.folded().text,
            u64::from(settings.max_edit_distance),
        )
    {
        adopt(
            &mut best,
            coin(field_tag, MatchStrategy::EditDistance, None),
        );
    }

    // Derived keys (name/aliases only) for Latin tokens. These are governed by
    // their own toggles (`search_pinyin`, `search_english_initials`), which
    // gate whether the derived keys exist at all; when they exist they match
    // independently of `fuzzy_matching`.
    if is_latin {
        if let Some(pinyin_full) = field.pinyin_full() {
            try_contiguous(
                field_tag,
                pinyin_full,
                MatchStrategy::PinyinFull,
                pattern,
                &mut best,
            );
        }
        if let Some(pinyin_initial) = field.pinyin_initial() {
            try_contiguous(
                field_tag,
                pinyin_initial,
                MatchStrategy::PinyinInitial,
                pattern,
                &mut best,
            );
        }
        if let Some(english_initial) = field.english_initial() {
            try_contiguous(
                field_tag,
                english_initial,
                MatchStrategy::EnglishInitials,
                pattern,
                &mut best,
            );
        }
    }

    best
}

/// Best hit for one token across all enabled fields of an entry. Different
/// tokens may hit different fields.
pub(crate) fn best_hit_for_token(
    keys: &super::keys::DerivedKeys,
    token: &Token,
    settings: &AppSettings,
) -> Option<FieldCoin> {
    keys.enabled_fields()
        .filter_map(|field| match_token_against_field(token, field, settings))
        .min_by_key(|coin| scoring::candidate_rank(coin.field, coin.strategy))
}

/// Match every token (multi-token AND). Returns the per-token best hits, or
/// `None` if any token matched nothing.
pub(crate) fn best_hit_for_all_tokens(
    keys: &super::keys::DerivedKeys,
    query: &Query,
    settings: &AppSettings,
) -> Option<Vec<FieldCoin>> {
    let mut hits = Vec::with_capacity(query.tokens().len());
    for token in query.tokens() {
        hits.push(best_hit_for_token(keys, token, settings)?);
    }
    Some(hits)
}

#[cfg(test)]
mod tests {
    use super::{edit_distance_match, levenshtein, map_key_range, subsequence};
    use crate::search::keys::MappedText;

    #[test]
    fn levenshtein_is_char_based_and_symmetric() {
        assert_eq!(levenshtein("usb", "usb"), 0);
        assert_eq!(levenshtein("driver", "driber"), 1);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("你好", "你好吗"), 1);
    }

    #[test]
    fn edit_distance_matches_whole_key_or_any_word() {
        // Whole-key.
        assert!(edit_distance_match("usb", "usb", 1));
        assert!(edit_distance_match("usbb", "usb", 1));
        assert!(edit_distance_match("usb", "uxb", 1));
        // Per-word: "driber" is within 1 of "driver".
        assert!(edit_distance_match("driber", "usb driver", 1));
        // "drivr" is within 1 of "driver"; "drvr" needs 2.
        assert!(edit_distance_match("drivr", "usb driver", 1));
        assert!(edit_distance_match("drvr", "usb driver", 2));
        assert!(!edit_distance_match("drvr", "usb driver", 1));
        // Whole-key too far, no word matches.
        assert!(!edit_distance_match("zebra", "usb driver", 1));
        // Distance 0 is disallowed (handled by exact/substring strategies).
        assert!(!edit_distance_match("driver", "usb driver", 0));
    }

    #[test]
    fn map_key_range_maps_pinyin_keys_back_to_source() {
        // "中文字" -> "zhong"(5) + "wen"(3) + "zi"(2) = "zhongwenzi".
        let key = MappedText::pinyin_full("中文字");
        assert_eq!(key.text, "zhongwenzi");
        assert_eq!(key.origins.len(), key.text.chars().count());
        assert_eq!(map_key_range(&key, 0, 5), Some((0, 1)));
        assert_eq!(map_key_range(&key, 5, 8), Some((1, 2)));
        assert_eq!(map_key_range(&key, 8, 10), Some((2, 3)));

        // "中文" -> "zw" (one first-letter char per Han char): origins [0, 1].
        let init = MappedText::pinyin_initial("中文");
        assert_eq!(init.text, "zw");
        assert_eq!(&init.origins, &[0, 1]);
        assert_eq!(map_key_range(&init, 0, 1), Some((0, 1)));
        assert_eq!(map_key_range(&init, 1, 2), Some((1, 2)));
    }

    #[test]
    fn subsequence_spans_first_to_last_matched_origin() {
        let key = MappedText::char_fold("My Documents");
        // folded "my documents": 'm' at 0, 'd' at 3, span covers [0,4).
        assert_eq!(subsequence(&key, "md").unwrap(), (0, 4));
    }

    #[test]
    fn subsequence_requires_order() {
        let key = MappedText::char_fold("abc");
        assert!(subsequence(&key, "ac").is_some());
        assert!(subsequence(&key, "ca").is_none());
    }
}
