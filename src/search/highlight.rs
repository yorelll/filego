//! UTF-8-safe highlight ranges (M02.2).
//!
//! Match keys are case-folded derivatives that can have a different length
//! than the original display string (pinyin full/initial keys, english
//! initials). Every derived key carries an `origins` list mapping each key char
//! back to the original source char, so a key match is remapped onto
//! **character** indices of the ORIGINAL string. Ranges are half-open
//! `(start, end)` and always lie on UTF-8 character boundaries, so callers can
//! safely slice the original string without ever splitting a multi-byte char.

use super::{
    matching::{FieldCoin, MatchStrategy},
    search_entry::{SearchEntry, SearchField},
};

/// Controls whether highlight ranges are computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightOptions {
    pub compute_highlights: bool,
}

impl Default for HighlightOptions {
    fn default() -> Self {
        HighlightOptions {
            compute_highlights: true,
        }
    }
}

/// One highlighted span on one field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HighlightRange {
    pub field: SearchField,
    /// Character index of the first highlighted char (inclusive).
    pub start: usize,
    /// Character index one past the last highlighted char (exclusive).
    pub end: usize,
}

/// The original searchable text for a field.
fn field_source(entry: &SearchEntry, field: SearchField) -> Option<&str> {
    match field {
        SearchField::Name => Some(&entry.display_name),
        SearchField::Alias(index) => entry.aliases.get(index).map(String::as_str),
        SearchField::Path => Some(&entry.path),
        SearchField::Category => entry.category_name.as_deref(),
        SearchField::Tag(index) => entry.tag_names.get(index).map(String::as_str),
        SearchField::Note => Some(&entry.note),
    }
}

/// Compute highlight ranges for one result, mapped back onto the ORIGINAL
/// source strings and validated against their char counts so no invalid or
/// mid-char slice is ever produced.
pub(crate) fn compute_highlights(
    entry: &SearchEntry,
    hits: &[FieldCoin],
    options: &HighlightOptions,
) -> Vec<HighlightRange> {
    if !options.compute_highlights {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    for coin in hits {
        // Edit-distance hits carry no contiguous span to highlight.
        if coin.strategy == MatchStrategy::EditDistance {
            continue;
        }
        let Some((start, end)) = coin.range_chars else {
            continue;
        };
        let Some(source) = field_source(entry, coin.field) else {
            continue;
        };
        let char_count = source.chars().count();
        if start < end && end <= char_count {
            ranges.push(HighlightRange {
                field: coin.field,
                start,
                end,
            });
        }
    }
    ranges
}
