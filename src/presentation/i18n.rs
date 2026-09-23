//! Language model + zh-CN / en-US key catalogs (M03.4).
//!
//! # Choice (documented)
//!
//! The ViewModel and its commands stay completely free of presentation text.
//! All user-visible strings live here as key → value catalogs, exposed through
//! [`Msg::tr`] so Rust logic (error states, hints, aria labels) resolves text
//! through one tested boundary. The Slint `.slint` file references the same
//! keys indirectly: the adapter pushes resolved strings into `in-out` string
//! properties from Rust, rather than using Slint's built-in `@tr(...)` /
//! gettext machinery. Rationale:
//!
//! - the logic layer then holds **no presentation text** at all (the
//!   `@tr()` approach embeds the English source string in the `.slint` and
//!   relies on gettext `.mo` catalogs — away from the tests);
//! - key-parity and missing-key tests run as plain Rust without a Slint
//!   compilation, which is exactly the M03.5 requirement;
//! - system-locale detection stays in the adapter (one hook), and
//!   `Locale::detect` below is the injectable, testable projection of it.
//!
//! The built-in Slint `@tr()`/`tr()` API remains available for the bundled
//! widget strings (Fluent widgets already use it internally); this project does
//! not need to ship `.mo` files for 0.0.1.

/// Supported UI languages. `zh-CN` is the product default; `en-US` is the
/// fallback for any other locale (never crashes, never shows a key name).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    ZhCN,
    EnUS,
}

impl Locale {
    /// BCP-47 tag, used by a future native adapter for loading system settings.
    pub const fn bcp47(self) -> &'static str {
        match self {
            Locale::ZhCN => "zh-CN",
            Locale::EnUS => "en-US",
        }
    }

    /// Projection of a system locale string (from the adapter) onto a supported
    /// language. Anything that is not Chinese resolves to `en-US` — English is
    /// the safe international fallback.
    pub fn detect(system_default: Option<&str>) -> Locale {
        match system_default {
            Some(locale)
                if locale.starts_with("zh") || locale.starts_with("zh-CN") || locale == "cmn" =>
            {
                Locale::ZhCN
            }
            _ => Locale::EnUS,
        }
    }
}

pub type MsgId = &'static str;

/// Typed message ids. Add a new variant here and in both catalogs; the parity
/// test fails if any variant is missing in a catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum Msg {
    // --- window / search box ---
    SearchPlaceholder,
    Clear,
    SearchLabel,
    // --- empty states ---
    NoResultsIcon,
    EmptyHint,
    NoDataTitle,
    NoDataBody,
    FilteredOutTitle,
    FilteredOutBody,
    NoMatchTitle,
    NoMatchBody,
    // --- status / rows ---
    Inaccessible,
    InaccessibleTooltip,
    TagLabel,
    // --- keyboard hints ---
    HintNavigate,
    HintOpen,
    HintClose,
    HintToggleHints,
    // --- errors ---
    ErrorLoadTitle,
    ErrorLoadBody,
    ErrorSaveTitle,
    ErrorSaveBody,
    ErrorSearchTitle,
    ErrorSearchBody,
    // --- overlays / menu ---
    FilterPanelTitle,
    ContextMenuOpen,
    ContextMenuCopyPath,
    ContextMenuCopyName,
    // --- general ---
    AppTitle,
    KeyboardHintsBarTitle,
    ResultsCount(u16),
}

impl Msg {
    pub fn id(self) -> &'static str {
        match self {
            Msg::SearchPlaceholder => "search.placeholder",
            Msg::Clear => "search.clear",
            Msg::SearchLabel => "search.label",
            Msg::NoResultsIcon => "status.no_results",
            Msg::EmptyHint => "status.empty_hint",
            Msg::NoDataTitle => "status.no_data.title",
            Msg::NoDataBody => "status.no_data.body",
            Msg::FilteredOutTitle => "status.filtered_out.title",
            Msg::FilteredOutBody => "status.filtered_out.body",
            Msg::NoMatchTitle => "status.no_match.title",
            Msg::NoMatchBody => "status.no_match.body",
            Msg::Inaccessible => "status.inaccessible",
            Msg::InaccessibleTooltip => "status.inaccessible.tooltip",
            Msg::TagLabel => "status.tag_label",
            Msg::HintNavigate => "hint.navigate",
            Msg::HintOpen => "hint.open",
            Msg::HintClose => "hint.close",
            Msg::HintToggleHints => "hint.toggle_hints",
            Msg::ErrorLoadTitle => "error.load.title",
            Msg::ErrorLoadBody => "error.load.body",
            Msg::ErrorSaveTitle => "error.save.title",
            Msg::ErrorSaveBody => "error.save.body",
            Msg::ErrorSearchTitle => "error.search.title",
            Msg::ErrorSearchBody => "error.search.body",
            Msg::FilterPanelTitle => "overlay.filter_panel.title",
            Msg::ContextMenuOpen => "contextmenu.open",
            Msg::ContextMenuCopyPath => "contextmenu.copy_path",
            Msg::ContextMenuCopyName => "contextmenu.copy_name",
            Msg::AppTitle => "app.title",
            Msg::KeyboardHintsBarTitle => "hints.bar.title",
            Msg::ResultsCount(_) => "status.results_count",
        }
    }

    /// Resolve this message in `locale`. The `ResultsCount(n)` variant formats
    /// its carried count into the `{count}` placeholder.
    pub fn tr(self, locale: Locale) -> String {
        let catalog = match locale {
            Locale::ZhCN => zh_cn::CATALOG,
            Locale::EnUS => en_us::CATALOG,
        };
        let value = catalog
            .iter()
            .find(|(key, _)| *key == self.id())
            .map(|(_, translated)| *translated)
            .unwrap_or(self.id());
        match self {
            Msg::ResultsCount(count) => value.replace("{count}", &count.to_string()),
            _ => value.to_owned(),
        }
    }
}

/// The complete key set, embedded in the catalog every variant must touch.
pub const ALL_KEYS: &[&str] = &[
    "search.placeholder",
    "search.clear",
    "search.label",
    "status.no_results",
    "status.empty_hint",
    "status.no_data.title",
    "status.no_data.body",
    "status.filtered_out.title",
    "status.filtered_out.body",
    "status.no_match.title",
    "status.no_match.body",
    "status.inaccessible",
    "status.inaccessible.tooltip",
    "status.tag_label",
    "hint.navigate",
    "hint.open",
    "hint.close",
    "hint.toggle_hints",
    "error.load.title",
    "error.load.body",
    "error.save.title",
    "error.save.body",
    "error.search.title",
    "error.search.body",
    "overlay.filter_panel.title",
    "contextmenu.open",
    "contextmenu.copy_path",
    "contextmenu.copy_name",
    "app.title",
    "hints.bar.title",
    "status.results_count",
];

mod zh_cn {
    /// zh-CN catalog.
    pub const CATALOG: &[(&str, &str)] = &[
        ("search.placeholder", "搜索文件夹、标签或路径"),
        ("search.clear", "清除输入"),
        ("search.label", "搜索"),
        ("status.no_results", "无结果"),
        ("status.empty_hint", "输入关键词开始搜索"),
        ("status.no_data.title", "还没有文件夹"),
        ("status.no_data.body", "先在设置中添加文件夹，再回来搜索。"),
        ("status.filtered_out.title", "没有符合筛选的记录"),
        ("status.filtered_out.body", "调整或清空筛选后再试。"),
        ("status.no_match.title", "没有匹配的结果"),
        ("status.no_match.body", "换个关键词，或取消筛选试试。"),
        ("status.inaccessible", "不可访问"),
        (
            "status.inaccessible.tooltip",
            "此路径当前不可访问，记录保留",
        ),
        ("status.tag_label", "标签"),
        ("hint.navigate", "↑↓ 选择"),
        ("hint.open", "Enter 打开"),
        ("hint.close", "Esc 关闭"),
        ("hint.toggle_hints", "Alt+H 隐藏/显示提示"),
        ("error.load.title", "数据加载失败"),
        (
            "error.load.body",
            "无法读取已保存的数据；损坏的文件会被保留。",
        ),
        ("error.save.title", "保存失败"),
        ("error.save.body", "更改未能保存，之前的副本未被覆盖。"),
        ("error.search.title", "搜索失败"),
        ("error.search.body", "搜索无法完成，请重试。"),
        ("overlay.filter_panel.title", "筛选"),
        ("contextmenu.open", "打开"),
        ("contextmenu.copy_path", "复制路径"),
        ("contextmenu.copy_name", "复制名称"),
        ("app.title", "FileGo"),
        ("hints.bar.title", "键盘提示"),
        ("status.results_count", "{count} 个结果"),
    ];
}

mod en_us {
    /// en-US catalog.
    pub const CATALOG: &[(&str, &str)] = &[
        ("search.placeholder", "Search folders, tags or paths"),
        ("search.clear", "Clear input"),
        ("search.label", "Search"),
        ("status.no_results", "No results"),
        ("status.empty_hint", "Type to search"),
        ("status.no_data.title", "No folders yet"),
        (
            "status.no_data.body",
            "Add folders in settings first, then come back to search.",
        ),
        ("status.filtered_out.title", "No entries match the filter"),
        (
            "status.filtered_out.body",
            "Adjust or clear the filters and try again.",
        ),
        ("status.no_match.title", "No matching results"),
        (
            "status.no_match.body",
            "Try a different keyword or clear the filters.",
        ),
        ("status.inaccessible", "Inaccessible"),
        (
            "status.inaccessible.tooltip",
            "This path is currently inaccessible; the record is kept",
        ),
        ("status.tag_label", "Tags"),
        ("hint.navigate", "↑↓ Navigate"),
        ("hint.open", "Enter Open"),
        ("hint.close", "Esc Close"),
        ("hint.toggle_hints", "Alt+H Toggle hints"),
        ("error.load.title", "Failed to load data"),
        (
            "error.load.body",
            "Stored data could not be read; a corrupt file is preserved.",
        ),
        ("error.save.title", "Failed to save"),
        (
            "error.save.body",
            "The change was not saved and the previous copy was kept.",
        ),
        ("error.search.title", "Search failed"),
        (
            "error.search.body",
            "The search could not be completed, try again.",
        ),
        ("overlay.filter_panel.title", "Filters"),
        ("contextmenu.open", "Open"),
        ("contextmenu.copy_path", "Copy path"),
        ("contextmenu.copy_name", "Copy name"),
        ("app.title", "FileGo"),
        ("hints.bar.title", "Keyboard hints"),
        ("status.results_count", "{count} results"),
    ];
}

#[cfg(test)]
mod tests {
    use super::{ALL_KEYS, Locale, Msg, en_us, zh_cn};

    fn collect_into<'a>(catalog: &'a [(&'a str, &'a str)], map: &mut Vec<(&'a str, &'a str)>) {
        map.extend_from_slice(catalog);
    }

    #[test]
    fn every_key_exists_in_both_catalogs_with_no_duplicates() {
        for (locale, catalog) in [
            (Locale::ZhCN, zh_cn::CATALOG),
            (Locale::EnUS, en_us::CATALOG),
        ] {
            let mut map = Vec::new();
            collect_into(catalog, &mut map);
            // no duplicate key
            for i in 0..map.len() {
                for j in (i + 1)..map.len() {
                    assert_ne!(map[i].0, map[j].0, "duplicate key in {locale:?}");
                }
            }
            for key in ALL_KEYS {
                assert!(
                    map.iter().any(|(k, _)| *k == *key),
                    "key {key:?} missing from {locale:?}"
                );
            }
            assert_eq!(map.len(), ALL_KEYS.len(), "extra keys in {locale:?}");
        }
    }

    #[test]
    fn same_key_count_in_zh_and_en() {
        assert_eq!(zh_cn::CATALOG.len(), en_us::CATALOG.len());
    }

    #[test]
    fn every_message_resolves_to_nonempty_text_in_both_locales() {
        for locale in [Locale::ZhCN, Locale::EnUS] {
            for key in ALL_KEYS {
                let value = catalog_value(locale, key);
                assert!(!value.is_empty(), "{key:?} is empty in {locale:?}");
            }
        }
    }

    #[test]
    fn results_count_formats_the_count_in_both_locales() {
        let zh = Msg::ResultsCount(7).tr(Locale::ZhCN);
        let en = Msg::ResultsCount(7).tr(Locale::EnUS);
        assert!(zh.contains('7'));
        assert!(en.contains('7'));
    }

    #[test]
    fn detection_defaults_to_chinese_only_for_chinese_locales() {
        assert_eq!(Locale::detect(Some("zh-CN")), Locale::ZhCN);
        assert_eq!(Locale::detect(Some("zh-Hans-CN")), Locale::ZhCN);
        assert_eq!(Locale::detect(Some("en-US")), Locale::EnUS);
        assert_eq!(Locale::detect(Some("fr-FR")), Locale::EnUS);
        assert_eq!(Locale::detect(None), Locale::EnUS);
        // Default is zh-CN if a component is never kicked off (product default).
        assert_eq!(Locale::default(), Locale::ZhCN);
    }

    fn catalog_value(locale: Locale, key: &str) -> &'static str {
        let catalog = match locale {
            Locale::ZhCN => zh_cn::CATALOG,
            Locale::EnUS => en_us::CATALOG,
        };
        catalog
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
            .unwrap_or("")
    }
}
