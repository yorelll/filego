//! Derived search keys (M02.1).
//!
//! For every searchable text slice we build a case-folded match key and, when
//! the settings toggles are active, pinyin-based keys. Display text is NEVER
//! mutated: keys are derived copies only, so the product's raw display strings
//! stay intact. A per-key-char "origin" records which original character each
//! key character came from, which lets highlight ranges be mapped back to
//! UTF-8-safe character indices of the original string.
//!
//! Normalization is deliberately 1:1 per character:
//! * ASCII and single-char Unicode lowercase folds (`A` -> `a`, `Θ` -> `θ`)
//!   are applied.
//! * Multi-char folds (`ß` -> `ss`, `İ` -> `i̇`) keep the original char so the
//!   key keeps a 1:1 mapping back to the display string. This is documented
//!   behavior, not accidental loss.
//!
//! Pinyin keys are derived from the characters of the **name and aliases**
//! only (matching the `AppSettings::search_pinyin` comment: "M02 must derive
//! pinyin keys from names and aliases"). Path/category/tag/note participate
//! with direct text strategies through their folded keys. Derived keys are
//! computed per search and are never persisted.
//!
//! Pinyin uses the **first reading** of each character (`pinyin 0.11.0`,
//! `plain` feature, no `heteronym` table): multi-reading characters such as
//! 「乐」 always map to one fixed reading (deterministic for scoring), and
//! retroflex initials take only the first letter (`zh` → `z`). This is a
//! known product limitation — a name/path pinyin-searched under a non-first
//! reading may not match — and is recorded as a manual-acceptance item (M02-B
//! may optionally enable `heteronym`).

use pinyin::ToPinyin;

use crate::domain::settings::AppSettings;

use super::search_entry::SearchField;

/// Fold one char for KEY and TOKEN comparison.
///
/// Only single-char lowercase folds are applied so the mapping stays 1:1 and
/// display strings are never corrupted. Multi-char folds keep the original.
pub(crate) fn fold_char(character: char) -> char {
    let mut iter = character.to_lowercase();
    match (iter.next(), iter.next()) {
        (Some(single), None) => single,
        _ => character,
    }
}

/// A derived key plus the mapping of each key char back to an original char.
///
/// `origins[i]` is the index (in `char`s) of the original character in the
/// source string that produced key char `i`. For pinyin and initial keys a
/// single original char can produce several key chars; `origins` is therefore
/// monotonic non-decreasing.
#[derive(Debug, Clone)]
pub(crate) struct MappedText {
    pub(crate) text: String,
    pub(crate) origins: Vec<usize>,
}

impl MappedText {
    /// Case-folded key, 1:1 per source char.
    pub(crate) fn char_fold(source: &str) -> MappedText {
        let mut text = String::with_capacity(source.len());
        let mut origins = Vec::with_capacity(source.chars().count());
        for (index, character) in source.chars().enumerate() {
            let folded = fold_char(character);
            text.push(folded);
            origins.push(index);
        }
        MappedText { text, origins }
    }

    /// Whether `source` contains any Han character (pinyin derivable).
    pub(crate) fn contains_han(source: &str) -> bool {
        source
            .chars()
            .any(|character| character.to_pinyin().is_some())
    }

    /// Full pinyin key without syllable separators, e.g. "中文" -> "zhongwen".
    ///
    /// Non-Han chars are case-folded in place. Queries may type "zhongwen" (one
    /// token) or "zhong wen" (two tokens) — both match, because the key is
    /// concatenated and each token is matched by substring.
    pub(crate) fn pinyin_full(source: &str) -> MappedText {
        let mut text = String::with_capacity(source.len());
        let mut origins = Vec::with_capacity(source.chars().count());
        for (index, character) in source.chars().enumerate() {
            if let Some(syllable) = character.to_pinyin() {
                for c in syllable.plain().chars() {
                    text.push(c);
                    origins.push(index);
                }
            } else {
                text.push(fold_char(character));
                origins.push(index);
            }
        }
        MappedText { text, origins }
    }

    /// Pinyin initial key, 1:1 per source char, e.g. "中文" -> "zw".
    ///
    /// Non-Han chars are case-folded in place.
    pub(crate) fn pinyin_initial(source: &str) -> MappedText {
        let mut text = String::with_capacity(source.chars().count());
        let mut origins = Vec::with_capacity(source.chars().count());
        for (index, character) in source.chars().enumerate() {
            if let Some(syllable) = character.to_pinyin() {
                let initial = syllable.first_letter();
                for c in initial.chars() {
                    text.push(c);
                    origins.push(index);
                }
            } else {
                text.push(fold_char(character));
                origins.push(index);
            }
        }
        MappedText { text, origins }
    }

    /// English-initial key: first (case-folded) letter of each
    /// whitespace/hyphen-separated word, e.g. "My Documents" -> "md",
    /// "usb-driver" -> "ud".
    pub(crate) fn english_initial(source: &str) -> MappedText {
        let mut text = String::new();
        let mut origins = Vec::new();
        let mut in_word = false;
        for (index, character) in source.chars().enumerate() {
            if character.is_whitespace() || character == '-' {
                in_word = false;
            } else if !in_word {
                in_word = true;
                text.push(fold_char(character));
                origins.push(index);
            }
        }
        MappedText { text, origins }
    }
}

/// Search keys for one searchable field of one entry.
#[derive(Debug, Clone)]
pub struct FieldKeys {
    pub(crate) field: SearchField,
    /// Whether this field participates in search (its settings toggle is on).
    /// Name and aliases are always enabled.
    pub(crate) enabled: bool,
    folded: MappedText,
    pinyin_full: Option<MappedText>,
    pinyin_initial: Option<MappedText>,
    english_initial: Option<MappedText>,
}

impl FieldKeys {
    /// Build the derived keys for one searchable slice.
    ///
    /// The folded key is always built. Pinyin full/initial keys are built only
    /// for name/aliases when `search_pinyin` is on and the text contains Han
    /// characters. The english-initials key is built only for name/aliases when
    /// `search_english_initials` is on.
    pub(crate) fn new(
        field: SearchField,
        source: &str,
        enabled: bool,
        settings: &AppSettings,
    ) -> Self {
        let is_name_or_alias = matches!(field, SearchField::Name | SearchField::Alias(_));
        let has_han = MappedText::contains_han(source);
        let build_pinyin = is_name_or_alias && enabled && settings.search_pinyin && has_han;

        FieldKeys {
            field,
            enabled,
            folded: MappedText::char_fold(source),
            pinyin_full: build_pinyin.then(|| MappedText::pinyin_full(source)),
            pinyin_initial: build_pinyin.then(|| MappedText::pinyin_initial(source)),
            english_initial: (is_name_or_alias && enabled && settings.search_english_initials)
                .then(|| MappedText::english_initial(source)),
        }
    }

    /// Whether this field is enabled by its settings toggle.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// The searchable field this key set belongs to.
    pub fn field(&self) -> SearchField {
        self.field
    }

    pub(crate) fn folded(&self) -> &MappedText {
        &self.folded
    }

    pub(crate) fn pinyin_full(&self) -> Option<&MappedText> {
        self.pinyin_full.as_ref()
    }

    pub(crate) fn pinyin_initial(&self) -> Option<&MappedText> {
        self.pinyin_initial.as_ref()
    }

    pub(crate) fn english_initial(&self) -> Option<&MappedText> {
        self.english_initial.as_ref()
    }
}

/// All derived keys for one searchable entry.
#[derive(Debug, Clone)]
pub struct DerivedKeys {
    pub name: FieldKeys,
    pub aliases: Vec<FieldKeys>,
    pub path: FieldKeys,
    pub category: Option<FieldKeys>,
    pub tags: Vec<FieldKeys>,
    pub note: FieldKeys,
}

impl DerivedKeys {
    /// Iterate every enabled field of this entry in a stable order.
    pub(crate) fn enabled_fields(&self) -> impl Iterator<Item = &FieldKeys> {
        let name = Some(&self.name);
        let aliases = self.aliases.iter();
        let path = Some(&self.path);
        let category = self.category.iter();
        let tags = self.tags.iter();
        let note = Some(&self.note);
        name.into_iter()
            .chain(aliases)
            .chain(path)
            .chain(category)
            .chain(tags)
            .chain(note)
            .filter(|field| field.enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::{MappedText, fold_char};

    #[test]
    fn fold_char_keeps_1_to_1_mapping_for_single_char_folds() {
        assert_eq!(fold_char('A'), 'a');
        assert_eq!(fold_char('Θ'), 'θ');
        assert_eq!(fold_char('Ж'), 'ж');
        assert_eq!(fold_char('中'), '中');
    }

    #[test]
    fn fold_char_never_expands_multi_char_folds() {
        // These fold to more than one char in Unicode; we keep the original so
        // the key mapping stays 1:1 and display strings are never corrupted.
        assert_eq!(fold_char('ß'), 'ß');
        assert_eq!(fold_char('İ'), 'İ');
    }

    #[test]
    fn english_initial_rule_is_whitespace_or_hyphen_words() {
        assert_eq!(MappedText::english_initial("My Documents").text, "md");
        assert_eq!(MappedText::english_initial("usb-driver").text, "ud");
        assert_eq!(
            MappedText::english_initial("My-Documents Folder").text,
            "mdf"
        );
        assert_eq!(MappedText::english_initial("Single").text, "s");
        assert_eq!(MappedText::english_initial("  -  ").text, "");
    }

    #[test]
    fn pinyin_full_is_concatenated_without_separators() {
        assert_eq!(MappedText::pinyin_full("中文").text, "zhongwen");
        assert_eq!(MappedText::pinyin_full("拼音").text, "pinyin");
        // ASCII is case-folded in place.
        assert_eq!(MappedText::pinyin_full("USB").text, "usb");
    }

    #[test]
    fn pinyin_initial_is_first_letters() {
        assert_eq!(MappedText::pinyin_initial("中文").text, "zw");
        assert_eq!(MappedText::pinyin_initial("中国").text, "zg");
    }

    #[test]
    fn pinyin_origins_are_monotonic_and_map_back() {
        // "中文字" -> "zhong"(5) + "wen"(3) + "zi"(2).
        let key = MappedText::pinyin_full("中文字");
        assert_eq!(key.text, "zhongwenzi");
        assert_eq!(&key.origins, &[0, 0, 0, 0, 0, 1, 1, 1, 2, 2]);
    }

    #[test]
    fn contains_han_detects_chinese_only() {
        assert!(MappedText::contains_han("中文"));
        assert!(MappedText::contains_han("A中文"));
        assert!(!MappedText::contains_han("USB Driver"));
        assert!(!MappedText::contains_han("emoji 📁"));
    }
}
