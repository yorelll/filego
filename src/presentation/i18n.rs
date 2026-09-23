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
    /// The shell could not open the selected folder (M04.5).
    ErrorOpenTitle,
    /// Generic body (also the historical fallback).
    ErrorOpenBody,
    // --- open-error kinds (F006): per-kind, still anonymous (no path) -----
    /// The path does not exist (SE_ERR_FNF/PNF).
    ErrorOpenNotFound,
    /// The path is missing but permission was denied (SE_ERR_ACCESSDENIED).
    ErrorOpenAccessDenied,
    /// No shell verb is associated (SE_ERR_NOASSOC).
    ErrorOpenNoAssociation,
    /// A DDE-style failure (SE_ERR_DDEFAIL).
    ErrorOpenDde,
    /// The shell started but declined (hInstApp <= 32).
    ErrorOpenShellRejected,
    /// Everything else / unknown.
    ErrorOpenUnavailable,
    // --- overlays / menu ---
    FilterPanelTitle,
    ContextMenuOpen,
    ContextMenuCopyPath,
    ContextMenuCopyName,
    // --- context menu (M05.5) ---
    ContextMenuEdit,
    ContextMenuPin,
    ContextMenuUnpin,
    ContextMenuDisable,
    ContextMenuEnable,
    ContextMenuRemove,
    RemoveNeverDeletes,
    RemoveConfirm,
    Undo,
    // --- settings window (M05) ---
    SettingsTitle,
    SettingsFolders,
    SettingsCategories,
    SettingsTags,
    ColName,
    ColPath,
    ColStatus,
    ColActions,
    ActionAdd,
    ActionEdit,
    ActionRemoveRecord,
    ActionCheck,
    ActionPaste,
    ActionBrowse,
    FilterNamePlaceholder,
    Uncategorized,
    EnabledLabel,
    DisabledLabel,
    PinnedLabel,
    NoticeSaved,
    NoticeSaveFailed,
    NoticeRemoved,
    NoticeRestored,
    NoticeUndoExpired,
    NoticeCannotUndo,
    NoticeDuplicateBlocked,
    NoticeInvalidPath,
    NoticeInvalidName,
    NoticeNotFound,
    NoticePathAccessible,
    NoticePathInaccessible,
    NoticeImportLimited,
    AddDialogTitle,
    EditDialogTitle,
    FieldName,
    FieldPath,
    Save,
    Cancel,
    ImportOfferTitle,
    ImportParentOnly,
    ImportChildren,
    ImportSecondConfirm,
    BatchPreviewTitle,
    BatchAdd,
    BatchStatusDuplicate,
    BatchStatusInvalid,
    CategoryCreate,
    CategoryDelete,
    TagCreate,
    TagMerge,
    TagDelete,
    UsageSuffix,
    FilterStatusAll,
    FilterStatusEnabled,
    FilterStatusDisabled,
    SortName,
    SortRecent,
    SortAdded,
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
            Msg::ErrorOpenTitle => "error.open.title",
            Msg::ErrorOpenBody => "error.open.body",
            Msg::ErrorOpenNotFound => "error.open.not_found",
            Msg::ErrorOpenAccessDenied => "error.open.access_denied",
            Msg::ErrorOpenNoAssociation => "error.open.no_association",
            Msg::ErrorOpenDde => "error.open.dde",
            Msg::ErrorOpenShellRejected => "error.open.shell_rejected",
            Msg::ErrorOpenUnavailable => "error.open.unavailable",
            Msg::FilterPanelTitle => "overlay.filter_panel.title",
            Msg::ContextMenuOpen => "contextmenu.open",
            Msg::ContextMenuCopyPath => "contextmenu.copy_path",
            Msg::ContextMenuCopyName => "contextmenu.copy_name",
            Msg::ContextMenuEdit => "contextmenu.edit",
            Msg::ContextMenuPin => "contextmenu.pin",
            Msg::ContextMenuUnpin => "contextmenu.unpin",
            Msg::ContextMenuDisable => "contextmenu.disable",
            Msg::ContextMenuEnable => "contextmenu.enable",
            Msg::ContextMenuRemove => "contextmenu.remove_record",
            Msg::RemoveNeverDeletes => "contextmenu.remove_never_deletes",
            Msg::RemoveConfirm => "contextmenu.remove_confirm",
            Msg::Undo => "contextmenu.undo",
            Msg::SettingsTitle => "settings.title",
            Msg::SettingsFolders => "settings.page.folders",
            Msg::SettingsCategories => "settings.page.categories",
            Msg::SettingsTags => "settings.page.tags",
            Msg::ColName => "settings.col.name",
            Msg::ColPath => "settings.col.path",
            Msg::ColStatus => "settings.col.status",
            Msg::ColActions => "settings.col.actions",
            Msg::ActionAdd => "settings.action.add",
            Msg::ActionEdit => "settings.action.edit",
            Msg::ActionRemoveRecord => "settings.action.remove_record",
            Msg::ActionCheck => "settings.action.check",
            Msg::ActionPaste => "settings.action.paste",
            Msg::ActionBrowse => "settings.action.browse",
            Msg::FilterNamePlaceholder => "settings.filter.name_placeholder",
            Msg::Uncategorized => "settings.uncategorized",
            Msg::EnabledLabel => "settings.enabled",
            Msg::DisabledLabel => "settings.disabled",
            Msg::PinnedLabel => "settings.pinned",
            Msg::NoticeSaved => "settings.notice.saved",
            Msg::NoticeSaveFailed => "settings.notice.save_failed",
            Msg::NoticeRemoved => "settings.notice.removed",
            Msg::NoticeRestored => "settings.notice.restored",
            Msg::NoticeUndoExpired => "settings.notice.undo_expired",
            Msg::NoticeCannotUndo => "settings.notice.cannot_undo",
            Msg::NoticeDuplicateBlocked => "settings.notice.duplicate_blocked",
            Msg::NoticeInvalidPath => "settings.notice.invalid_path",
            Msg::NoticeInvalidName => "settings.notice.invalid_name",
            Msg::NoticeNotFound => "settings.notice.not_found",
            Msg::NoticePathAccessible => "settings.notice.path_accessible",
            Msg::NoticePathInaccessible => "settings.notice.path_inaccessible",
            Msg::NoticeImportLimited => "settings.notice.import_limited",
            Msg::AddDialogTitle => "settings.dialog.add",
            Msg::EditDialogTitle => "settings.dialog.edit",
            Msg::FieldName => "settings.field.name",
            Msg::FieldPath => "settings.field.path",
            Msg::Save => "settings.save",
            Msg::Cancel => "settings.cancel",
            Msg::ImportOfferTitle => "settings.import.offer_title",
            Msg::ImportParentOnly => "settings.import.parent_only",
            Msg::ImportChildren => "settings.import.children",
            Msg::ImportSecondConfirm => "settings.import.second_confirm",
            Msg::BatchPreviewTitle => "settings.batch.preview_title",
            Msg::BatchAdd => "settings.batch.add",
            Msg::BatchStatusDuplicate => "settings.batch.status_duplicate",
            Msg::BatchStatusInvalid => "settings.batch.status_invalid",
            Msg::CategoryCreate => "settings.category.create",
            Msg::CategoryDelete => "settings.category.delete",
            Msg::TagCreate => "settings.tag.create",
            Msg::TagMerge => "settings.tag.merge",
            Msg::TagDelete => "settings.tag.delete",
            Msg::UsageSuffix => "settings.usage_suffix",
            Msg::FilterStatusAll => "settings.filter.status_all",
            Msg::FilterStatusEnabled => "settings.filter.status_enabled",
            Msg::FilterStatusDisabled => "settings.filter.status_disabled",
            Msg::SortName => "settings.sort.name",
            Msg::SortRecent => "settings.sort.recent",
            Msg::SortAdded => "settings.sort.added",
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
    "error.open.title",
    "error.open.body",
    "error.open.not_found",
    "error.open.access_denied",
    "error.open.no_association",
    "error.open.dde",
    "error.open.shell_rejected",
    "error.open.unavailable",
    "overlay.filter_panel.title",
    "contextmenu.open",
    "contextmenu.copy_path",
    "contextmenu.copy_name",
    "contextmenu.edit",
    "contextmenu.pin",
    "contextmenu.unpin",
    "contextmenu.disable",
    "contextmenu.enable",
    "contextmenu.remove_record",
    "contextmenu.remove_never_deletes",
    "contextmenu.remove_confirm",
    "contextmenu.undo",
    "settings.title",
    "settings.page.folders",
    "settings.page.categories",
    "settings.page.tags",
    "settings.col.name",
    "settings.col.path",
    "settings.col.status",
    "settings.col.actions",
    "settings.action.add",
    "settings.action.edit",
    "settings.action.remove_record",
    "settings.action.check",
    "settings.action.paste",
    "settings.action.browse",
    "settings.filter.name_placeholder",
    "settings.uncategorized",
    "settings.enabled",
    "settings.disabled",
    "settings.pinned",
    "settings.notice.saved",
    "settings.notice.save_failed",
    "settings.notice.removed",
    "settings.notice.restored",
    "settings.notice.undo_expired",
    "settings.notice.cannot_undo",
    "settings.notice.duplicate_blocked",
    "settings.notice.invalid_path",
    "settings.notice.invalid_name",
    "settings.notice.not_found",
    "settings.notice.path_accessible",
    "settings.notice.path_inaccessible",
    "settings.notice.import_limited",
    "settings.dialog.add",
    "settings.dialog.edit",
    "settings.field.name",
    "settings.field.path",
    "settings.save",
    "settings.cancel",
    "settings.import.offer_title",
    "settings.import.parent_only",
    "settings.import.children",
    "settings.import.second_confirm",
    "settings.batch.preview_title",
    "settings.batch.add",
    "settings.batch.status_duplicate",
    "settings.batch.status_invalid",
    "settings.category.create",
    "settings.category.delete",
    "settings.tag.create",
    "settings.tag.merge",
    "settings.tag.delete",
    "settings.usage_suffix",
    "settings.filter.status_all",
    "settings.filter.status_enabled",
    "settings.filter.status_disabled",
    "settings.sort.name",
    "settings.sort.recent",
    "settings.sort.added",
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
        ("error.open.title", "无法打开文件夹"),
        (
            "error.open.body",
            "该文件夹暂时无法打开（可能已断开或没有权限）。按 Enter 重试，Ctrl+C 复制路径。",
        ),
        (
            "error.open.not_found",
            "找不到该路径（文件夹可能已被移动或删除）。按 Enter 重试，Ctrl+C 复制路径。",
        ),
        (
            "error.open.access_denied",
            "没有权限打开此文件夹，请检查共享或权限设置。按 Enter 重试，Ctrl+C 复制路径。",
        ),
        (
            "error.open.no_association",
            "没有关联的程序可以打开此文件夹。按 Enter 重试，Ctrl+C 复制路径。",
        ),
        (
            "error.open.dde",
            "打开请求未能完成（系统繁忙或其它程序正在处理）。按 Enter 重试，Ctrl+C 复制路径。",
        ),
        (
            "error.open.shell_rejected",
            "系统已收到请求但未能打开，请稍后重试。按 Enter 重试，Ctrl+C 复制路径。",
        ),
        (
            "error.open.unavailable",
            "该文件夹暂时无法打开。按 Enter 重试，Ctrl+C 复制路径。",
        ),
        ("overlay.filter_panel.title", "筛选"),
        ("contextmenu.open", "打开"),
        ("contextmenu.copy_path", "复制路径"),
        ("contextmenu.copy_name", "复制名称"),
        ("contextmenu.edit", "编辑"),
        ("contextmenu.pin", "置顶"),
        ("contextmenu.unpin", "取消置顶"),
        ("contextmenu.disable", "禁用"),
        ("contextmenu.enable", "恢复"),
        ("contextmenu.remove_record", "从 FileGo 移除"),
        (
            "contextmenu.remove_never_deletes",
            "不会删除磁盘中的真实文件夹；如需删除真实目录，请先在文件管理器中打开后自行操作。",
        ),
        ("contextmenu.remove_confirm", "从 FileGo 移除记录"),
        ("contextmenu.undo", "撤销"),
        ("settings.title", "设置"),
        ("settings.page.folders", "文件夹"),
        ("settings.page.categories", "分类"),
        ("settings.page.tags", "标签"),
        ("settings.col.name", "名称"),
        ("settings.col.path", "路径"),
        ("settings.col.status", "状态"),
        ("settings.col.actions", "操作"),
        ("settings.action.add", "添加"),
        ("settings.action.edit", "编辑"),
        ("settings.action.remove_record", "从 FileGo 移除"),
        ("settings.action.check", "检查"),
        ("settings.action.paste", "粘贴"),
        ("settings.action.browse", "浏览…"),
        ("settings.filter.name_placeholder", "按名称筛选"),
        ("settings.uncategorized", "未分类"),
        ("settings.enabled", "已启用"),
        ("settings.disabled", "已禁用"),
        ("settings.pinned", "已置顶"),
        ("settings.notice.saved", "已保存"),
        ("settings.notice.save_failed", "保存失败"),
        ("settings.notice.removed", "已从 FileGo 移除记录"),
        ("settings.notice.restored", "已恢复记录"),
        ("settings.notice.undo_expired", "撤销已过期"),
        ("settings.notice.cannot_undo", "无法撤销（数据已变化）"),
        ("settings.notice.duplicate_blocked", "路径已存在，未添加"),
        ("settings.notice.invalid_path", "路径无效"),
        ("settings.notice.invalid_name", "名称无效或重复"),
        ("settings.notice.not_found", "记录不存在"),
        ("settings.notice.path_accessible", "路径可访问"),
        (
            "settings.notice.path_inaccessible",
            "路径当前不可访问（记录保留）",
        ),
        ("settings.notice.import_limited", "未发现可直接导入的子目录"),
        ("settings.dialog.add", "添加文件夹"),
        ("settings.dialog.edit", "编辑文件夹"),
        ("settings.field.name", "名称"),
        ("settings.field.path", "路径"),
        ("settings.save", "保存"),
        ("settings.cancel", "取消"),
        ("settings.import.offer_title", "导入方式"),
        ("settings.import.parent_only", "仅导入父目录"),
        ("settings.import.children", "导入直接子目录"),
        (
            "settings.import.second_confirm",
            "再次确认：导入仅限直接子目录，不递归",
        ),
        ("settings.batch.preview_title", "添加预览"),
        ("settings.batch.add", "添加可用项"),
        ("settings.batch.status_duplicate", "重复，已跳过"),
        ("settings.batch.status_invalid", "无效，已跳过"),
        ("settings.category.create", "新建分类"),
        ("settings.category.delete", "删除分类"),
        ("settings.tag.create", "新建标签"),
        ("settings.tag.merge", "合并"),
        ("settings.tag.delete", "删除标签"),
        ("settings.usage_suffix", " 使用"),
        ("settings.filter.status_all", "全部状态"),
        ("settings.filter.status_enabled", "已启用"),
        ("settings.filter.status_disabled", "已禁用"),
        ("settings.sort.name", "按名称"),
        ("settings.sort.recent", "按最近使用"),
        ("settings.sort.added", "按添加时间"),
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
        ("error.open.title", "Could not open folder"),
        (
            "error.open.body",
            "This folder cannot be opened right now (it may be offline or have no permission). Press Enter to retry or Ctrl+C to copy the path.",
        ),
        (
            "error.open.not_found",
            "The path could not be found (the folder may have been moved or deleted). Press Enter to retry or Ctrl+C to copy the path.",
        ),
        (
            "error.open.access_denied",
            "You do not have permission to open this folder; check the share or permissions. Press Enter to retry or Ctrl+C to copy the path.",
        ),
        (
            "error.open.no_association",
            "There is no associated app that can open this folder. Press Enter to retry or Ctrl+C to copy the path.",
        ),
        (
            "error.open.dde",
            "The open request could not complete (system busy or another program is handling it). Press Enter to retry or Ctrl+C to copy the path.",
        ),
        (
            "error.open.shell_rejected",
            "The system received the request but could not open it; try again shortly. Press Enter to retry or Ctrl+C to copy the path.",
        ),
        (
            "error.open.unavailable",
            "This folder cannot be opened right now. Press Enter to retry or Ctrl+C to copy the path.",
        ),
        ("overlay.filter_panel.title", "Filters"),
        ("contextmenu.open", "Open"),
        ("contextmenu.copy_path", "Copy path"),
        ("contextmenu.copy_name", "Copy name"),
        ("contextmenu.edit", "Edit"),
        ("contextmenu.pin", "Pin"),
        ("contextmenu.unpin", "Unpin"),
        ("contextmenu.disable", "Disable"),
        ("contextmenu.enable", "Enable"),
        ("contextmenu.remove_record", "Remove from FileGo"),
        (
            "contextmenu.remove_never_deletes",
            "This never deletes the real folder on disk; if you want to delete the real folder, open it in File Explorer and delete it there.",
        ),
        ("contextmenu.remove_confirm", "Remove record from FileGo"),
        ("contextmenu.undo", "Undo"),
        ("settings.title", "Settings"),
        ("settings.page.folders", "Folders"),
        ("settings.page.categories", "Categories"),
        ("settings.page.tags", "Tags"),
        ("settings.col.name", "Name"),
        ("settings.col.path", "Path"),
        ("settings.col.status", "Status"),
        ("settings.col.actions", "Actions"),
        ("settings.action.add", "Add"),
        ("settings.action.edit", "Edit"),
        ("settings.action.remove_record", "Remove from FileGo"),
        ("settings.action.check", "Check"),
        ("settings.action.paste", "Paste"),
        ("settings.action.browse", "Browse…"),
        ("settings.filter.name_placeholder", "Filter by name"),
        ("settings.uncategorized", "Uncategorized"),
        ("settings.enabled", "Enabled"),
        ("settings.disabled", "Disabled"),
        ("settings.pinned", "Pinned"),
        ("settings.notice.saved", "Saved"),
        ("settings.notice.save_failed", "Could not save"),
        ("settings.notice.removed", "Record removed from FileGo"),
        ("settings.notice.restored", "Record restored"),
        ("settings.notice.undo_expired", "Undo window expired"),
        ("settings.notice.cannot_undo", "Cannot undo (data changed)"),
        (
            "settings.notice.duplicate_blocked",
            "Path already exists; not added",
        ),
        ("settings.notice.invalid_path", "Invalid path"),
        ("settings.notice.invalid_name", "Invalid or duplicate name"),
        ("settings.notice.not_found", "Record not found"),
        ("settings.notice.path_accessible", "Path is accessible"),
        (
            "settings.notice.path_inaccessible",
            "Path is currently inaccessible (kept)",
        ),
        (
            "settings.notice.import_limited",
            "No directly importable child folders found",
        ),
        ("settings.dialog.add", "Add folder"),
        ("settings.dialog.edit", "Edit folder"),
        ("settings.field.name", "Name"),
        ("settings.field.path", "Path"),
        ("settings.save", "Save"),
        ("settings.cancel", "Cancel"),
        ("settings.import.offer_title", "Import method"),
        ("settings.import.parent_only", "Only import the parent"),
        ("settings.import.children", "Import direct children"),
        (
            "settings.import.second_confirm",
            "Confirm again: import is limited to direct children, never recursive",
        ),
        ("settings.batch.preview_title", "Add preview"),
        ("settings.batch.add", "Add ready items"),
        ("settings.batch.status_duplicate", "Duplicate, skipped"),
        ("settings.batch.status_invalid", "Invalid, skipped"),
        ("settings.category.create", "New category"),
        ("settings.category.delete", "Delete category"),
        ("settings.tag.create", "New tag"),
        ("settings.tag.merge", "Merge"),
        ("settings.tag.delete", "Delete tag"),
        ("settings.usage_suffix", " used"),
        ("settings.filter.status_all", "All statuses"),
        ("settings.filter.status_enabled", "Enabled"),
        ("settings.filter.status_disabled", "Disabled"),
        ("settings.sort.name", "By name"),
        ("settings.sort.recent", "By recent use"),
        ("settings.sort.added", "By added time"),
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
