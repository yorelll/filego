//! Pure, deterministic search core (M02-A).
//!
//! This module owns query parsing/normalization, derived search keys,
//! matching strategies, deterministic tiered scoring and UTF-8-safe highlight
//! ranges. It is deliberately free of filesystem, network, environment and
//! time access: every input arrives through [`SearchEntry`] and
//! [`crate::domain::settings::AppSettings`], and every output is a pure
//! function of those inputs.
//!
//! M02-A covers non-empty queries only. Empty-query display strategies
//! (favorites/pinned/recent), the filter matrix and the 10k release benchmark
//! are later slices and intentionally absent.

#[cfg(test)]
mod benchmark;
mod highlight;
pub mod keys;
mod matching;
mod query;
mod scoring;
mod search_entry;

pub mod empty_query;
pub mod filter;
mod generation;
pub use empty_query::{
    EmptyQueryResult, EmptyQueryStrategy, NoResultReason, SearchDisplay, SearchResponse,
    empty_query, empty_query_indices, search_with_filter,
};
pub use filter::{Accessibility, FilterSet, Origin};
pub use generation::QueryGeneration;

pub use highlight::{HighlightOptions, HighlightRange};
pub use query::{Query, QueryParser, Token, TokenKind};
pub use scoring::weights;
pub use search_entry::{RankedResult, SearchEntry, SearchField, SearchHit, SearchScore};

use crate::domain::settings::AppSettings;

/// Run a deterministic search over `entries`.
///
/// Every token must match at least one searchable field (multi-token AND);
/// different tokens may match different fields. For multi-token queries the
/// result's tier is determined by the **weakest matched token** (the worst
/// tier among the matched tokens), while every matched token still contributes
/// its points — so a result whose second token only hits an editable field is
/// ranked as that (weaker) tier, never promoted by the stronger token.
/// Scoring is tier-based and fully deterministic; a documented tie-break and
/// truncation by `settings.max_results` are applied after full scoring so the
/// limit never distorts ranking.
pub fn search(
    entries: &[SearchEntry],
    query: &Query,
    settings: &AppSettings,
    options: &HighlightOptions,
) -> Vec<RankedResult> {
    if query.tokens().is_empty() {
        // M02-A is scoped to non-empty queries; empty input yields no ranked
        // results. Empty-query display strategies are M02.4.
        return Vec::new();
    }

    // Collect matches as (entry, score, highlight ranges) and sort by total
    // score descending, then the documented deterministic tie-break.
    let mut matches: Vec<(&SearchEntry, SearchScore, Vec<HighlightRange>)> = Vec::new();
    for entry in entries {
        let keys = entry.derive_keys(settings);
        let Some(hits) = matching::best_hit_for_all_tokens(&keys, query, settings) else {
            continue;
        };
        let score = scoring::build_score(entry, &hits);
        let ranges = highlight::compute_highlights(entry, &hits, options);
        matches.push((entry, score, ranges));
    }

    matches.sort_by(|left, right| {
        right
            .1
            .total_score
            .cmp(&left.1.total_score)
            .then_with(|| scoring::tiebreak(left.0, right.0, settings.recent_sort_first))
    });

    matches
        .into_iter()
        .take(settings.max_results as usize)
        .map(|(entry, score, highlights)| RankedResult::new(entry, score, highlights))
        .collect()
}

#[cfg(test)]
mod tests;
