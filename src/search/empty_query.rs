//! Empty-query display strategies and filter-aware result status (M02.4).
//!
//! A blank query never runs the non-empty matching engine. Instead, this module
//! selects an ordered display list using [`EmptyQueryStrategy`]. The selection
//! is pure: no clock, filesystem, network or path probing is used.
//!
//! `FavoritesFirst` makes an at-most-five-item favorites quick-access section,
//! then lists remaining pinned entries, remaining previously opened entries,
//! and then all remaining entries. Pinned-but-not-favorite entries are never
//! lost. The cap applies only to the favorites section; it never deletes or
//! hides an entry from the general portion of the list.
//!
//! Every strategy performs its full deterministic order before `max_results`
//! truncates the visible list. The order is manual weight descending, pinned
//! first, open count descending, then folder id ascending — M02-A's existing
//! stable tie-break with no text-relevance tier.

use crate::{
    domain::{folder::MAX_FAVORITES, settings::AppSettings},
    search::SearchEntry,
};

use super::{
    filter::FilterSet, highlight::HighlightOptions, query::Query, search_entry::RankedResult,
};

pub use crate::domain::settings::EmptyQueryStrategy;

/// Why a request resulted in no visible results (M02.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoResultReason {
    /// There are no stored entries.
    NoData,
    /// Entries exist, but active filtering (or a filtering strategy such as
    /// `PinnedOnly`) removed every candidate.
    FilteredOut,
    /// Candidates existed but a non-empty query matched none of them.
    NoMatch,
}

/// Empty-query result represented as source indices, retaining an optional
/// reason only when nothing can be displayed. `None` means entries are visible
/// or the caller explicitly requested the `Blank` strategy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyQueryResult {
    /// Indices into the original `entries` slice, already fully ordered and
    /// truncated by `settings.max_results`.
    pub indices: Vec<usize>,
    pub no_result_reason: Option<NoResultReason>,
}

/// A filter-aware response for either a ranked non-empty query or an empty
/// display strategy. This preserves M02-A's [`RankedResult`] payload for typed
/// queries while giving the presenter source indices for blank-query rows.
#[derive(Debug, Clone)]
pub enum SearchDisplay {
    Ranked(Vec<RankedResult>),
    EmptyQuery(Vec<usize>),
}

/// Filter-aware search response with a localizable empty-state distinction.
#[derive(Debug, Clone)]
pub struct SearchResponse {
    pub display: SearchDisplay,
    pub no_result_reason: Option<NoResultReason>,
}

/// Apply one empty-query strategy to an already-filtered candidate slice.
///
/// The returned indices refer to `entries`, not an original unfiltered slice.
/// It is public for callers that have already applied a filter; ordinary
/// callers should prefer [`empty_query`] or [`search_with_filter`].
pub fn empty_query_indices(entries: &[SearchEntry], strategy: EmptyQueryStrategy) -> Vec<usize> {
    empty_query_candidate_indices(entries, (0..entries.len()).collect(), strategy)
}

/// Equivalent to [`empty_query_indices`], but works against already-filtered
/// indices into `entries` without cloning the entries themselves.
fn empty_query_candidate_indices(
    entries: &[SearchEntry],
    mut candidates: Vec<usize>,
    strategy: EmptyQueryStrategy,
) -> Vec<usize> {
    candidates.sort_by(|&left, &right| empty_rank(entries, left, right));

    match strategy {
        EmptyQueryStrategy::Blank => Vec::new(),
        EmptyQueryStrategy::PinnedOnly => candidates
            .into_iter()
            .filter(|&index| entries[index].pinned)
            .collect(),
        EmptyQueryStrategy::All => candidates,
        EmptyQueryStrategy::FavoritesFirst => favorites_first_indices(entries, &candidates),
    }
}

fn empty_rank(entries: &[SearchEntry], left: usize, right: usize) -> std::cmp::Ordering {
    entries[right]
        .manual_weight
        .cmp(&entries[left].manual_weight)
        .then_with(|| entries[right].pinned.cmp(&entries[left].pinned))
        .then_with(|| entries[right].open_count.cmp(&entries[left].open_count))
        .then_with(|| entries[left].id.as_uuid().cmp(&entries[right].id.as_uuid()))
}

fn favorites_first_indices(entries: &[SearchEntry], order: &[usize]) -> Vec<usize> {
    let mut selected = vec![false; entries.len()];
    let mut kept = Vec::with_capacity(entries.len());

    // At most five favorites occupy the quick-access section.
    for &index in order.iter().filter(|&&index| entries[index].favorite) {
        if kept.len() == MAX_FAVORITES {
            break;
        }
        selected[index] = true;
        kept.push(index);
    }

    // Remaining pinned, then remaining recently opened, then the rest. Each
    // loop preserves `order`; `selected` makes this O(n) after the sort.
    append_section(
        &mut kept,
        &mut selected,
        order,
        |entry| !entry.favorite && entry.pinned,
        entries,
    );
    append_section(
        &mut kept,
        &mut selected,
        order,
        |entry| !entry.favorite && entry.last_opened_at.is_some(),
        entries,
    );
    // Data validation prevents this clause from excluding legitimate stored
    // entries: persisted AppData has at most MAX_FAVORITES favorites. It keeps
    // the display cap defensive for hand-built/invalid inputs too.
    append_section(
        &mut kept,
        &mut selected,
        order,
        |entry| !entry.favorite,
        entries,
    );

    kept
}

fn append_section(
    kept: &mut Vec<usize>,
    selected: &mut [bool],
    order: &[usize],
    predicate: impl Fn(&SearchEntry) -> bool,
    entries: &[SearchEntry],
) {
    for &index in order {
        if !selected[index] && predicate(&entries[index]) {
            selected[index] = true;
            kept.push(index);
        }
    }
}

/// Produce a visible blank-query list after applying `filter`.
///
/// A `Blank` strategy is intentional and therefore returns no empty-state
/// reason. NoData/FilteredOut are only reported when the caller could have
/// shown candidates but none exist or none survive filtering/selection.
pub fn empty_query(
    entries: &[SearchEntry],
    filter: &FilterSet,
    settings: &AppSettings,
) -> EmptyQueryResult {
    if entries.is_empty() {
        return EmptyQueryResult {
            indices: Vec::new(),
            no_result_reason: Some(NoResultReason::NoData),
        };
    }

    let filtered_indices = filter.apply(entries);
    if filtered_indices.is_empty() {
        return EmptyQueryResult {
            indices: Vec::new(),
            no_result_reason: Some(NoResultReason::FilteredOut),
        };
    }

    if settings.empty_query_strategy == EmptyQueryStrategy::Blank {
        return EmptyQueryResult {
            indices: Vec::new(),
            no_result_reason: None,
        };
    }

    let visible =
        empty_query_candidate_indices(entries, filtered_indices, settings.empty_query_strategy);
    let max_results = settings.max_results as usize;
    let indices = visible.into_iter().take(max_results).collect::<Vec<_>>();

    let no_result_reason = indices.is_empty().then_some(NoResultReason::FilteredOut);
    EmptyQueryResult {
        indices,
        no_result_reason,
    }
}

/// One API for M02-B presentation callers: apply a [`FilterSet`], dispatch a
/// blank query to [`empty_query`], otherwise preserve M02-A full scoring and
/// highlighting. Existing [`super::search`] remains unchanged for M02-A users.
pub fn search_with_filter(
    entries: &[SearchEntry],
    query: &Query,
    filter: &FilterSet,
    settings: &AppSettings,
    options: &HighlightOptions,
) -> SearchResponse {
    if query.tokens().is_empty() {
        let result = empty_query(entries, filter, settings);
        return SearchResponse {
            display: SearchDisplay::EmptyQuery(result.indices),
            no_result_reason: result.no_result_reason,
        };
    }

    if entries.is_empty() {
        return SearchResponse {
            display: SearchDisplay::Ranked(Vec::new()),
            no_result_reason: Some(NoResultReason::NoData),
        };
    }

    let filtered_indices = filter.apply(entries);
    if filtered_indices.is_empty() {
        return SearchResponse {
            display: SearchDisplay::Ranked(Vec::new()),
            no_result_reason: Some(NoResultReason::FilteredOut),
        };
    }

    let filtered: Vec<SearchEntry> = filtered_indices
        .iter()
        .map(|&index| entries[index].clone())
        .collect();
    let ranked = super::search(&filtered, query, settings, options);
    let no_result_reason = ranked.is_empty().then_some(NoResultReason::NoMatch);
    SearchResponse {
        display: SearchDisplay::Ranked(ranked),
        no_result_reason,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EmptyQueryStrategy, NoResultReason, SearchDisplay, empty_query, empty_query_indices,
        search_with_filter,
    };
    use crate::{
        domain::{folder::MAX_FAVORITES, ids::FolderId, settings::AppSettings},
        search::{
            HighlightOptions, QueryParser, SearchEntry,
            filter::{Accessibility, FilterSet, Origin},
        },
    };
    use uuid::Uuid;

    fn utc(value: &str) -> chrono::DateTime<chrono::Utc> {
        value.parse().expect("fixed RFC3339 fixture must parse")
    }

    fn entry(id: u128, favorite: bool, pinned: bool, open_count: u64) -> SearchEntry {
        SearchEntry {
            id: FolderId::from_uuid(Uuid::from_u128(id)),
            display_name: format!("entry-{id}"),
            aliases: Vec::new(),
            path: format!(r"C:\entry-{id}"),
            category_name: None,
            tag_names: Vec::new(),
            note: String::new(),
            pinned,
            favorite,
            manual_weight: 0,
            open_count,
            last_opened_at: Some(utc("2026-09-21T00:00:00Z")),
            accessibility: Accessibility::Unknown,
            origin: Origin::Unknown,
        }
    }

    fn ids(indices: &[usize], entries: &[SearchEntry]) -> Vec<u128> {
        indices
            .iter()
            .map(|&index| entries[index].id.as_uuid().as_u128())
            .collect()
    }

    #[test]
    fn favorites_are_capped_at_five_and_remaining_entries_stay_listed() {
        // Invalid persisted data cannot contain seven favorites. The strategy
        // still enforces the quick-access cap defensively for hand-built input.
        let entries: Vec<SearchEntry> = (1..=8u128)
            .map(|id| entry(id, id <= 7, id == 8, (100 - id) as u64))
            .collect();
        let indices = empty_query_indices(&entries, EmptyQueryStrategy::FavoritesFirst);
        let shown = ids(&indices, &entries);

        let favorites_before_first_nonfavorite = indices
            .iter()
            .take_while(|&&index| entries[index].favorite)
            .count();
        assert_eq!(favorites_before_first_nonfavorite, MAX_FAVORITES);
        assert_eq!(shown, [1, 2, 3, 4, 5, 8]);
        assert_eq!(shown.len(), MAX_FAVORITES + 1);
    }

    #[test]
    fn favorites_first_lists_pinned_not_favorite_after_favorites() {
        let favorite = entry(1, true, false, 0);
        let pinned = entry(2, false, true, 0);
        let plain = entry(3, false, false, 0);
        let entries = [favorite, pinned, plain];

        let indices = empty_query_indices(&entries, EmptyQueryStrategy::FavoritesFirst);
        assert_eq!(ids(&indices, &entries), vec![1, 2, 3]);
    }

    #[test]
    fn all_pinned_only_and_blank_strategies_are_distinct() {
        let entries = [
            entry(1, false, true, 10),
            entry(2, true, false, 900),
            entry(3, false, false, 0),
        ];
        assert_eq!(
            ids(
                &empty_query_indices(&entries, EmptyQueryStrategy::All),
                &entries
            ),
            vec![1, 2, 3]
        );
        assert_eq!(
            ids(
                &empty_query_indices(&entries, EmptyQueryStrategy::PinnedOnly),
                &entries
            ),
            vec![1]
        );
        assert!(empty_query_indices(&entries, EmptyQueryStrategy::Blank).is_empty());
    }

    #[test]
    fn empty_query_applies_filter_and_truncates_after_full_sort() {
        let settings = AppSettings {
            max_results: 2,
            empty_query_strategy: EmptyQueryStrategy::All,
            ..AppSettings::default()
        };
        let mut e1 = entry(1, false, false, 10);
        e1.origin = Origin::Local;
        let mut e2 = entry(2, false, false, 300);
        e2.origin = Origin::Local;
        let mut e3 = entry(3, false, false, 200);
        e3.origin = Origin::Local;
        let mut e4 = entry(4, false, false, 999);
        e4.origin = Origin::Network;
        let entries = [e1, e2, e3, e4];
        let filter = FilterSet {
            origin: Some(Origin::Local),
            ..FilterSet::default()
        };

        let result = empty_query(&entries, &filter, &settings);
        assert_eq!(
            ids(&result.indices, &entries),
            vec![2, 3],
            "filter full set, sort full set, then truncate"
        );
        assert_eq!(result.no_result_reason, None);
    }

    #[test]
    fn no_result_reason_distinguishes_no_data_filtered_out_and_no_match() {
        let settings = AppSettings::default();
        let parser = QueryParser;
        let empty = parser.parse("");
        let unmatched = parser.parse("not-present");
        let filter = FilterSet::default();

        let no_data = search_with_filter(
            &[],
            &empty,
            &filter,
            &settings,
            &HighlightOptions::default(),
        );
        assert_eq!(no_data.no_result_reason, Some(NoResultReason::NoData));

        let entries = [entry(1, false, false, 0)];
        let filtered = FilterSet {
            pinned_only: true,
            ..FilterSet::default()
        };
        let filtered_out = search_with_filter(
            &entries,
            &empty,
            &filtered,
            &settings,
            &HighlightOptions::default(),
        );
        assert_eq!(
            filtered_out.no_result_reason,
            Some(NoResultReason::FilteredOut)
        );

        let no_match = search_with_filter(
            &entries,
            &unmatched,
            &filter,
            &settings,
            &HighlightOptions::default(),
        );
        assert_eq!(no_match.no_result_reason, Some(NoResultReason::NoMatch));
        assert!(matches!(no_match.display, SearchDisplay::Ranked(results) if results.is_empty()));
    }

    #[test]
    fn blank_strategy_is_intentionally_blank_not_a_no_result_error() {
        let settings = AppSettings {
            empty_query_strategy: EmptyQueryStrategy::Blank,
            ..AppSettings::default()
        };
        let entries = [entry(1, false, false, 0)];
        let result = empty_query(&entries, &FilterSet::default(), &settings);
        assert!(result.indices.is_empty());
        assert_eq!(result.no_result_reason, None);
    }
}
