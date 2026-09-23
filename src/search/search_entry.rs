//! Input and result types for the search core.

use crate::domain::{ids::FolderId, settings::AppSettings};

use super::{
    filter::{Accessibility, Origin},
    highlight::HighlightRange,
    keys::{DerivedKeys, FieldKeys},
};

/// A searchable field tag used to identify which field a hit came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchField {
    Name,
    Alias(usize),
    Path,
    Category,
    Tag(usize),
    Note,
}

/// One search hit on one field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub field: SearchField,
    /// The strategy that produced this hit.
    pub strategy: &'static str,
    /// Within-tier strategy score contributed by this hit.
    pub score: u64,
    /// Range in the ORIGINAL source string (char indices, half-open), if this
    /// strategy yields a concrete span.
    pub range: Option<(usize, usize)>,
}

/// Aggregate search result for one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchScore {
    /// Number of matched tokens (all must match for a result).
    pub matched_token_count: usize,
    /// Total number of tokens in the query.
    pub token_count: usize,
    pub total_score: u64,
    pub hits: Vec<SearchHit>,
}

impl SearchScore {
    pub fn all_tokens_matched(&self) -> bool {
        self.matched_token_count == self.token_count
    }
}

/// Searchable view of one folder record. M02-A consumes this; the caller (a
/// future presenter) builds it from `domain::document::AppData`.
///
/// `PartialEq` supports fixture-determinism tests and cheap diffing in tests;
/// it is not part of ranking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchEntry {
    pub id: FolderId,
    pub display_name: String,
    pub aliases: Vec<String>,
    pub path: String,
    pub category_name: Option<String>,
    pub tag_names: Vec<String>,
    pub note: String,
    pub pinned: bool,
    pub favorite: bool,
    pub manual_weight: i16,
    pub open_count: u64,
    pub last_opened_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Path accessibility state supplied by the presenter (filter input;
    /// never probed by the search core). See [`Accessibility`].
    pub accessibility: Accessibility,
    /// Volume origin supplied by the presenter (filter input). See
    /// [`Origin`].
    pub origin: Origin,
}

impl SearchEntry {
    pub fn derive_keys(&self, settings: &AppSettings) -> DerivedKeys {
        let name = FieldKeys::new(SearchField::Name, &self.display_name, true, settings);
        let aliases = self
            .aliases
            .iter()
            .enumerate()
            .map(|(index, alias)| {
                FieldKeys::new(
                    SearchField::Alias(index),
                    alias,
                    settings.search_aliases,
                    settings,
                )
            })
            .collect();
        let path = FieldKeys::new(
            SearchField::Path,
            &self.path,
            settings.search_paths,
            settings,
        );
        let category = self.category_name.as_deref().map(|name| {
            FieldKeys::new(
                SearchField::Category,
                name,
                settings.search_categories,
                settings,
            )
        });
        let tags = self
            .tag_names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                FieldKeys::new(
                    SearchField::Tag(index),
                    name,
                    settings.search_tags,
                    settings,
                )
            })
            .collect();
        let note = FieldKeys::new(
            SearchField::Note,
            &self.note,
            settings.search_notes,
            settings,
        );

        DerivedKeys {
            name,
            aliases,
            path,
            category,
            tags,
            note,
        }
    }
}

/// A ranked search result.
#[derive(Debug, Clone)]
pub struct RankedResult {
    pub entry_id: FolderId,
    pub total_score: u64,
    pub score: SearchScore,
    /// Highlight ranges per field: each range's `(start, end)` chars are
    /// indices into the ORIGINAL display string. Empty when highlighting is
    /// disabled.
    pub highlights: Vec<HighlightRange>,
}

impl RankedResult {
    pub(crate) fn new(
        entry: &SearchEntry,
        score: SearchScore,
        highlights: Vec<HighlightRange>,
    ) -> Self {
        RankedResult {
            entry_id: entry.id,
            total_score: score.total_score,
            score,
            highlights,
        }
    }
}
