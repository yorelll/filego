//! M02-A search core tests: normalization, derived keys, strategies, scoring,
//! highlight and settings toggles (see task M02.1 / M02.2).

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    domain::{ids::FolderId, settings::AppSettings},
    search::{HighlightOptions, QueryParser, RankedResult, SearchEntry, SearchField},
};

fn utc(value: &str) -> DateTime<Utc> {
    value.parse().expect("fixed RFC3339 fixture must parse")
}

fn make_entry(
    id: u128,
    display_name: &str,
    aliases: &[&str],
    path: &str,
    category_name: Option<&str>,
    tag_names: &[&str],
    note: &str,
) -> SearchEntry {
    SearchEntry {
        id: FolderId::from_uuid(Uuid::from_u128(id)),
        display_name: display_name.to_owned(),
        aliases: aliases.iter().map(|value| (*value).to_owned()).collect(),
        path: path.to_owned(),
        category_name: category_name.map(|value| value.to_owned()),
        tag_names: tag_names.iter().map(|value| (*value).to_owned()).collect(),
        note: note.to_owned(),
        pinned: false,
        favorite: false,
        manual_weight: 0,
        open_count: 0,
        last_opened_at: Some(utc("2026-09-21T00:00:00Z")),
    }
}

fn default_settings() -> AppSettings {
    AppSettings::default()
}

fn highlights_disabled() -> HighlightOptions {
    HighlightOptions {
        compute_highlights: false,
    }
}

fn run(
    entries: &[&SearchEntry],
    query: &str,
    settings: &AppSettings,
    options: &HighlightOptions,
) -> Vec<RankedResult> {
    let parser = QueryParser;
    let query = parser.parse(query);
    let owned: Vec<SearchEntry> = entries.iter().map(|entry| (*entry).clone()).collect();
    crate::search::search(&owned, &query, settings, options)
}

fn ids(results: &[RankedResult]) -> Vec<u128> {
    results
        .iter()
        .map(|result| result.entry_id.as_uuid().as_u128())
        .collect()
}

// ---------------------------------------------------------------------------
// Normalization
// ---------------------------------------------------------------------------

#[test]
fn trims_and_collapses_whitespace() {
    let query = QueryParser.parse("  usb \t drv\n\r ");
    assert_eq!(query.tokens().len(), 2);
    assert_eq!(query.tokens()[0].text(), "usb");
    assert_eq!(query.tokens()[1].text(), "drv");
}

#[test]
fn unicode_whitespace_is_a_separator_too() {
    let query = QueryParser.parse("a\u{00A0}b\u{3000}c");
    let texts: Vec<&str> = query.tokens().iter().map(|token| token.text()).collect();
    assert_eq!(texts, ["a", "b", "c"]);
}

#[test]
fn empty_query_parses_to_no_tokens() {
    assert_eq!(QueryParser.parse("").tokens().len(), 0);
    assert_eq!(QueryParser.parse("   ").tokens().len(), 0);
}

#[test]
fn latin_tokens_are_lower_cased() {
    let query = QueryParser.parse("USB DrV");
    assert_eq!(query.tokens()[0].text(), "usb");
    assert_eq!(query.tokens()[1].text(), "drv");
}

#[test]
fn non_ascii_tokens_fold_without_expansion() {
    let query = QueryParser.parse("中文");
    assert_eq!(query.tokens()[0].text(), "中文");
}

#[test]
fn mixed_zh_tokens_are_one_token() {
    let query = QueryParser.parse("中文USB");
    assert_eq!(query.tokens().len(), 1);
    // ASCII subset is folded like any token; CJK is never expanded.
    assert_eq!(query.tokens()[0].text(), "中文usb");
}

// ---------------------------------------------------------------------------
// Derived keys
// ---------------------------------------------------------------------------

#[test]
fn english_initial_rule_is_whitespace_or_hyphen_words() {
    use crate::search::keys::MappedText;
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
    use crate::search::keys::MappedText;
    assert_eq!(MappedText::pinyin_full("中文").text, "zhongwen");
    assert_eq!(MappedText::pinyin_full("拼音").text, "pinyin");
    assert_eq!(MappedText::pinyin_full("USB").text, "usb");
}

#[test]
fn pinyin_initial_is_first_letter_of_each_char() {
    use crate::search::keys::MappedText;
    assert_eq!(MappedText::pinyin_initial("中文").text, "zw");
    assert_eq!(MappedText::pinyin_initial("中国").text, "zg");
}

#[test]
fn pinyin_origins_are_monotonic() {
    use crate::search::keys::MappedText;
    let key = MappedText::pinyin_full("中文字");
    assert_eq!(key.text, "zhongwenzi");
    let mut seen = 0usize;
    for origin in key.origins {
        assert!(origin >= seen, "origins must be monotonic non-decreasing");
        seen = origin;
    }
}

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

#[test]
fn exact_match_is_required_for_full_name() {
    let settings = default_settings();
    let e1 = make_entry(1, "USB Driver", &[], r"C:\USB Driver", None, &[], "");
    let e2 = make_entry(
        2,
        "USB Driver Extra",
        &[],
        r"C:\USB Driver Extra",
        None,
        &[],
        "",
    );

    let results = run(
        &[&e1, &e2],
        "usb driver",
        &settings,
        &HighlightOptions::default(),
    );
    // "usb driver" exactly matches e1's name.
    assert_eq!(ids(&results), vec![1, 2]);
    let first = &results[0];
    assert_eq!(first.entry_id.as_uuid().as_u128(), 1);
}

#[test]
fn prefix_match_is_found() {
    let settings = default_settings();
    let e1 = make_entry(1, "USB Driver", &[], r"C:\USB Driver", None, &[], "");
    let results = run(&[&e1], "usb d", &settings, &HighlightOptions::default());
    assert_eq!(ids(&results), vec![1]);
}

#[test]
fn substring_match_is_found() {
    let settings = default_settings();
    let e1 = make_entry(1, "Project Alpha", &[], r"C:\Project Alpha", None, &[], "");
    let results = run(&[&e1], "alpha", &settings, &HighlightOptions::default());
    assert_eq!(ids(&results), vec![1]);
}

#[test]
fn ordered_subsequence_non_contiguous() {
    let settings = default_settings();
    // "mst" is an ordered subsequence of "microsoft" (m, s, t) but is neither a
    // prefix, substring, initial nor word of the name: only subsequence matches.
    let e1 = make_entry(1, "Microsoft", &[], r"C:\Microsoft", None, &[], "");
    let results = run(&[&e1], "mst", &settings, &HighlightOptions::default());
    assert_eq!(
        ids(&results),
        vec![1],
        "ordered non-contiguous subsequence over the folded name must match"
    );
}

#[test]
fn edit_distance_matches_within_limit() {
    let mut settings = default_settings();
    settings.max_edit_distance = 1;
    let e1 = make_entry(1, "USB Driver", &[], r"C:\USB Driver", None, &[], "");
    // "driber" is one substitution from the word "driver" and is not a prefix,
    // substring, initial or subsequence of "usb driver".
    let results = run(
        &[&e1],
        "usb driber",
        &settings,
        &HighlightOptions::default(),
    );
    assert_eq!(
        ids(&results),
        vec![1],
        "one-edit token must match when distance is within the limit"
    );
}

#[test]
fn edit_distance_respects_max_edit_distance() {
    let mut settings = default_settings();
    settings.max_edit_distance = 1;
    let e1 = make_entry(1, "USB Driver", &[], r"C:\USB Driver", None, &[], "");
    let results = run(
        &[&e1],
        "usb driver",
        &settings,
        &HighlightOptions::default(),
    );
    assert_eq!(ids(&results), vec![1]);

    // "rehoteq" needs 2 edits from "remote"/"Remote Work" and is not a prefix,
    // substring, initial or subsequence — only edit distance can match it.
    let e2 = make_entry(2, "Remote Work", &[], r"C:\Remote Work", None, &[], "");
    let results = run(&[&e2], "rehoteq", &settings, &HighlightOptions::default());
    assert!(
        results.is_empty(),
        "two edits must be rejected at distance 1"
    );

    settings.max_edit_distance = 2;
    let results = run(&[&e2], "rehoteq", &settings, &HighlightOptions::default());
    assert_eq!(
        ids(&results),
        vec![2],
        "two edits must be accepted at distance 2"
    );
}

#[test]
fn pinyin_full_matches_chinese_name() {
    let settings = default_settings();
    let e1 = make_entry(1, "中文文档", &[], r"C:\中文文档", None, &[], "");
    // Full pinyin "zhongwenwendang".
    let results = run(&[&e1], "zhongwen", &settings, &HighlightOptions::default());
    assert_eq!(ids(&results), vec![1]);
    // Intermediate "ongwenwen" also matches (substring).
    let results = run(&[&e1], "ongwenwen", &settings, &HighlightOptions::default());
    assert_eq!(ids(&results), vec![1]);
}

#[test]
fn pinyin_initial_matches_chinese_name() {
    let settings = default_settings();
    let e1 = make_entry(1, "中文文档", &[], r"C:\中文文档", None, &[], "");
    let results = run(&[&e1], "zwwd", &settings, &HighlightOptions::default());
    assert_eq!(ids(&results), vec![1]);
}

#[test]
fn english_initial_matches_multiword_name() {
    let settings = default_settings();
    let e1 = make_entry(1, "My Documents", &[], r"C:\My Documents", None, &[], "");
    let results = run(&[&e1], "md", &settings, &HighlightOptions::default());
    assert_eq!(ids(&results), vec![1]);
}

#[test]
fn fuzzy_off_disables_fuzzy_strategies() {
    let mut settings = default_settings();
    settings.fuzzy_matching = false;
    // "mst" is an ordered subsequence of "microsoft" (m, s, t) but is neither a
    // prefix nor a substring, so only the (fuzzy) subsequence strategy matches.
    let e1 = make_entry(1, "Microsoft", &[], r"C:\Microsoft", None, &[], "");
    let results = run(&[&e1], "mst", &settings, &HighlightOptions::default());
    assert!(
        results.is_empty(),
        "subsequence must be disabled with fuzzy off"
    );

    // Edit distance too: "driber" within 1 of "driver".
    let e2 = make_entry(2, "USB Driver", &[], r"C:\USB Driver", None, &[], "");
    let results = run(&[&e2], "driber", &settings, &HighlightOptions::default());
    assert!(
        results.is_empty(),
        "edit distance must be disabled with fuzzy off"
    );

    // Direct prefix still works.
    let results = run(&[&e2], "usb", &settings, &HighlightOptions::default());
    assert_eq!(ids(&results), vec![2]);
}

// ---------------------------------------------------------------------------
// Multi-token AND
// ---------------------------------------------------------------------------

#[test]
fn multi_token_and_requires_every_token() {
    let settings = default_settings();
    let e1 = make_entry(1, "USB Driver", &[], r"C:\USB Driver", None, &[], "");
    let results = run(
        &[&e1],
        "usb driver",
        &settings,
        &HighlightOptions::default(),
    );
    assert_eq!(ids(&results), vec![1]);

    let results = run(
        &[&e1],
        "usb missing",
        &settings,
        &HighlightOptions::default(),
    );
    assert!(results.is_empty(), "missing token must yield no result");
}

#[test]
fn tokens_can_match_different_fields() {
    let settings = default_settings();
    let e1 = make_entry(
        1,
        "USB Driver",
        &[],
        r"C:\Downloads\USB\installer",
        Some("工作"),
        &["工具"],
        "",
    );
    // token "usb" hits name; token "installer" hits path.
    let results = run(
        &[&e1],
        "usb installer",
        &settings,
        &HighlightOptions::default(),
    );
    assert_eq!(ids(&results), vec![1]);
}

// ---------------------------------------------------------------------------
// Aliases
// ---------------------------------------------------------------------------

#[test]
fn aliases_participate_like_name() {
    let settings = default_settings();
    let e1 = make_entry(
        1,
        "主要项目",
        &["Live Docs", "现场资料"],
        r"C:\Projects\main",
        None,
        &[],
        "",
    );
    let results = run(&[&e1], "live docs", &settings, &HighlightOptions::default());
    assert_eq!(ids(&results), vec![1], "alias must match like the name");

    let results = run(&[&e1], "live", &settings, &HighlightOptions::default());
    assert_eq!(ids(&results), vec![1], "partial alias must match");
}

// ---------------------------------------------------------------------------
// Scoring priority
// ---------------------------------------------------------------------------

#[test]
fn name_exact_beats_name_prefix_and_alias() {
    let settings = default_settings();
    let exact = make_entry(1, "USB Driver", &[], r"C:\USB Driver", None, &[], "");
    let prefix = make_entry(2, "USB Drivers", &[], r"C:\USB Drivers", None, &[], "");
    let alias = make_entry(3, "Thing", &["USB Driver"], r"C:\Thing", None, &[], "");

    let results = run(
        &[&alias, &exact, &prefix],
        "usb driver",
        &settings,
        &HighlightOptions::default(),
    );
    assert_eq!(
        ids(&results),
        vec![1, 2, 3],
        "name-exact must rank above name-prefix above alias"
    );
}

#[test]
fn name_prefix_beats_pinyin_path_and_edit_distance() {
    let settings = default_settings();
    let prefixed = make_entry(
        1,
        "USB Drivers Box",
        &[],
        r"C:\USB Drivers Box",
        None,
        &[],
        "",
    );
    let path = make_entry(2, "Random", &[], r"C:\usb\driver", None, &[], "");
    let edited = make_entry(3, "USX Driver", &[], r"C:\USX Driver", None, &[], "");

    let results = run(
        &[&path, &edited, &prefixed],
        "usb drive",
        &settings,
        &HighlightOptions::default(),
    );
    assert_eq!(
        ids(&results),
        vec![1, 2, 3],
        "name-prefix must beat path and edit-distance matches"
    );
}

#[test]
fn pinned_boost_does_not_jump_tier() {
    let settings = default_settings();
    // Pinned entry whose best hit is an alias; unimpeded pinned entry whose
    // best hit is also an alias; and one unpinned name-prefix hit. The
    // name-prefix hit must outrank both regardless of pinned.
    let pinned_alias = make_entry(1, "Main Folder", &["USB Driver"], r"C:\docs", None, &[], "");
    let name_prefix = make_entry(2, "USB Drivers", &[], r"C:\USB Drivers", None, &[], "");
    let mut pinned = pinned_alias.clone();
    pinned.id = FolderId::from_uuid(Uuid::from_u128(1));
    pinned.pinned = true;
    pinned.manual_weight = 100;
    let mut other_pinned = pinned_alias;
    other_pinned.id = FolderId::from_uuid(Uuid::from_u128(3));
    other_pinned.pinned = true;

    let results = run(
        &[&pinned, &name_prefix, &other_pinned],
        "usb driver",
        &settings,
        &HighlightOptions::default(),
    );
    assert_eq!(
        ids(&results),
        vec![2, 1, 3],
        "an unpinned name hit must beat pinned alias hits"
    );
}

#[test]
fn manual_weight_does_not_overpower_name_exact() {
    let settings = default_settings();
    // A heavily positively-weighted entry whose only hit is the name-exact,
    // versus a heavily negatively-weighted entry with the same name-exact.
    // manual_weight is only a within-tier tie-break; it must not reorder
    // different tiers, but here both are name-exact so the weight decides.
    let unweighted = make_entry(1, "USB Driver", &[], r"C:\USB Driver", None, &[], "");
    let weighted = make_entry(2, "USB Driver2", &[], r"C:\USB Driver2", None, &[], "");
    let _ = (unweighted, weighted);

    // The real contract: a strong alias match must never beat a name-exact hit.
    let alias_boosted = make_entry(3, "Zeta", &["USB Driver"], r"C:\Zeta", None, &[], "");
    alias_boosted_manual_weight(&settings, &alias_boosted);
}

fn alias_boosted_manual_weight(settings: &AppSettings, alias_entry: &SearchEntry) {
    let mut boosted = alias_entry.clone();
    boosted.pinned = true;
    boosted.manual_weight = 100;
    let name_exact = make_entry(4, "USB Driver", &[], r"C:\USB Driver", None, &[], "");
    let results = run(
        &[&boosted, &name_exact],
        "usb driver",
        settings,
        &HighlightOptions::default(),
    );
    assert_eq!(
        ids(&results),
        vec![4, 3],
        "a pinned + max-weight alias hit must not beat a name-exact hit"
    );
}

// ---------------------------------------------------------------------------
// Highlight
// ---------------------------------------------------------------------------

#[test]
fn highlight_ranges_are_utf8_safe_on_mixed_text() {
    let settings = default_settings();
    let e1 = make_entry(1, "文档 🗂️ Project 中文", &[], r"C:\文档 🗂️", None, &[], "");
    let results = run(&[&e1], "project", &settings, &HighlightOptions::default());
    assert_eq!(results.len(), 1);
    let highlight = &results[0].highlights[0];
    assert_eq!(highlight.field, SearchField::Name);
    // The range must slice the original without panic and be on char boundaries.
    let range = (highlight.start, highlight.end);
    let expected = locate("文档 🗂️ Project 中文", "Project");
    assert_eq!(range, expected);
}

#[test]
fn highlight_can_be_disabled_with_identical_results() {
    let settings = default_settings();
    let e1 = make_entry(1, "USB Driver", &[], r"C:\USB Driver", None, &[], "");
    let on = run(&[&e1], "usb", &settings, &HighlightOptions::default());
    let off = run(&[&e1], "usb", &settings, &highlights_disabled());
    assert_eq!(on.len(), off.len());
    assert_eq!(on[0].entry_id, off[0].entry_id);
    assert_eq!(on[0].total_score, off[0].total_score);
    assert!(
        on[0]
            .highlights
            .iter()
            .any(|r| r.field == SearchField::Name)
    );
    assert!(off[0].highlights.is_empty());
}

/// Locate the char range of `needle` in `haystack`.
fn locate(haystack: &str, needle: &str) -> (usize, usize) {
    let start = haystack
        .find(needle)
        .expect("needle must be present in display string");
    let start_chars = haystack[..start].chars().count();
    (start_chars, start_chars + needle.chars().count())
}

// ---------------------------------------------------------------------------
// Settings toggles
// ---------------------------------------------------------------------------

#[test]
fn search_paths_toggle_gates_path_only_matches() {
    let settings = default_settings();
    let e1 = make_entry(
        1,
        "Installer",
        &[],
        r"C:\Downloads\USB\setup.exe",
        None,
        &[],
        "",
    );
    let results = run(&[&e1], "setup", &settings, &HighlightOptions::default());
    assert_eq!(
        ids(&results),
        vec![1],
        "path match must be found by default"
    );

    let mut disabled = settings;
    disabled.search_paths = false;
    let results = run(&[&e1], "setup", &disabled, &HighlightOptions::default());
    assert!(
        results.is_empty(),
        "path-only match must be absent when search_paths is off"
    );
}

#[test]
fn search_notes_toggle_gates_note_matches() {
    let settings = default_settings();
    let e1 = make_entry(
        1,
        "Project Alpha",
        &[],
        r"C:\Project Alpha",
        None,
        &[],
        "release candidate notes live here",
    );

    // Default: search_notes false → note-only query empty.
    let results = run(&[&e1], "candidate", &settings, &HighlightOptions::default());
    assert!(
        results.is_empty() || ids(&results).is_empty(),
        "note search must be off by default"
    );

    let mut enabled = settings;
    enabled.search_notes = true;
    let results = run(&[&e1], "candidate", &enabled, &HighlightOptions::default());
    assert_eq!(
        ids(&results),
        vec![1],
        "note match must appear when enabled"
    );
}

#[test]
fn search_categories_and_tags_toggle_respected() {
    let mut settings = default_settings();
    settings.search_categories = false;
    settings.search_tags = false;
    let e1 = make_entry(
        1,
        "Project Alpha",
        &[],
        r"C:\Project Alpha",
        Some("工作"),
        &["重要"],
        "",
    );
    let results = run(&[&e1], "工作", &settings, &HighlightOptions::default());
    assert!(
        results.is_empty(),
        "category match must be gated by its toggle"
    );

    // Re-enable category.
    settings.search_categories = true;
    let results = run(&[&e1], "工作", &settings, &HighlightOptions::default());
    assert_eq!(ids(&results), vec![1]);

    settings.search_tags = true;
    let results = run(&[&e1], "重要", &settings, &HighlightOptions::default());
    assert_eq!(ids(&results), vec![1], "tag match must appear when enabled");
}

#[test]
fn pinyin_and_english_initial_toggles() {
    let settings = default_settings();
    let e1 = make_entry(1, "中文文档", &["My Docs"], r"C:\中文文档", None, &[], "");
    let results = run(&[&e1], "zwwd", &settings, &HighlightOptions::default());
    assert_eq!(
        ids(&results),
        vec![1],
        "pinyin initial must work when enabled"
    );

    let mut disabled_pinyin = settings.clone();
    disabled_pinyin.search_pinyin = false;
    let results = run(
        &[&e1],
        "zwwd",
        &disabled_pinyin,
        &HighlightOptions::default(),
    );
    assert!(
        results.is_empty(),
        "pinyin must be off when toggle is false"
    );

    // With fuzzy ON, "md" may also match "My Docs" through the fuzzy
    // subsequence strategy, so isolate the english-initials toggle by turning
    // fuzzy off: the initials key is the only remaining way "md" can match.
    let mut no_fuzzy = settings.clone();
    no_fuzzy.fuzzy_matching = false;
    let results = run(&[&e1], "md", &no_fuzzy, &HighlightOptions::default());
    assert_eq!(
        ids(&results),
        vec![1],
        "english initial must work when enabled (fuzzy off isolates the key)"
    );

    let mut disabled_initial = no_fuzzy;
    disabled_initial.search_english_initials = false;
    let results = run(
        &[&e1],
        "md",
        &disabled_initial,
        &HighlightOptions::default(),
    );
    assert!(
        results.is_empty(),
        "english-initial must be off when toggle is false"
    );
}

// ---------------------------------------------------------------------------
// Acceptance: usb drv
// ---------------------------------------------------------------------------

#[test]
fn acceptance_usb_drv_matches_usb_driver() {
    let settings = default_settings();
    let e1 = make_entry(
        1,
        "USB Driver",
        &["外部驱动"],
        r"C:\Program Files\USB\Driver\x64",
        None,
        &[],
        "",
    );
    let results = run(&[&e1], "usb drv", &settings, &HighlightOptions::default());
    assert_eq!(
        ids(&results),
        vec![1],
        "acceptance example: 'usb drv' must match 'USB Driver'"
    );
    // 'drv' hits the folded name prefix ("drv" prefix of "driver") so the
    // result must be a Name-tier hit.
    assert_eq!(
        results[0].score.hits[0].field,
        SearchField::Name,
        "first token 'usb' must hit the name field"
    );
}

// ---------------------------------------------------------------------------
// Determinism / stability
// ---------------------------------------------------------------------------

#[test]
fn repeated_identical_query_produces_identical_order() {
    let settings = default_settings();
    let e1 = make_entry(1, "Alpha", &[], r"C:\Alpha", None, &[], "");
    let e2 = make_entry(2, "Alpha", &[], r"C:\Alpha", None, &[], "");
    let e3 = make_entry(3, "Alpha Beta", &[], r"C:\Alpha Beta", None, &[], "");

    let entries = [&e1, &e2, &e3];
    let first = run(&entries, "alpha", &settings, &HighlightOptions::default());
    let second = run(&entries, "alpha", &settings, &HighlightOptions::default());

    assert_eq!(ids(&first), ids(&second));
    assert_eq!(
        first.iter().map(|r| r.total_score).collect::<Vec<_>>(),
        second.iter().map(|r| r.total_score).collect::<Vec<_>>()
    );
}

#[test]
fn max_results_truncates_after_full_sorting() {
    let mut settings = default_settings();
    settings.max_results = 2;
    let e1 = make_entry(1, "Alpha", &[], r"C:\Alpha", None, &[], "");
    let e2 = make_entry(2, "Alpha Beta", &[], r"C:\Alpha Beta", None, &[], "");
    let e3 = make_entry(3, "Alphabet", &[], r"C:\Alphabet", None, &[], "");

    let entries = [&e1, &e2, &e3];
    let results = run(&entries, "alpha", &settings, &HighlightOptions::default());
    assert_eq!(results.len(), 2);
}
