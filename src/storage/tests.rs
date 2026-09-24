use std::path::Path;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    domain::{
        document::AppData,
        folder::{
            Category, FolderColor, FolderEntry, MAX_ALIAS_LEN, MAX_ALIASES_PER_FOLDER,
            MAX_CATEGORY_NAME_LEN, MAX_FAVORITES, MAX_MANUAL_WEIGHT, MAX_NOTE_LEN,
            MAX_TAG_NAME_LEN, MIN_MANUAL_WEIGHT, Tag,
        },
        ids::{CategoryId, FolderId, TagId},
        settings::{
            AppSettings, EmptyQueryStrategy, MAX_EDIT_DISTANCE, MAX_MAX_RESULTS, MAX_WINDOW_WIDTH,
            MIN_MAX_RESULTS, MIN_WINDOW_WIDTH, ThemePreference,
        },
    },
    storage::{
        codec::{StorageErrorKind, decode, encode},
        schema::StoredDocumentV1,
    },
};

const SENSITIVE_PATH: &str = r"\\sensitive-server\private-share\机密 文件夹\nested\very-long-path";
const SENSITIVE_JSON_MARKER: &str = "DO-NOT-LEAK-JSON-CONTENT";

fn utc(value: &str) -> DateTime<Utc> {
    value.parse().expect("fixed RFC3339 fixture must parse")
}

fn fixture_document() -> StoredDocumentV1 {
    let category_id = CategoryId::from_uuid(Uuid::from_u128(1));
    let first_tag_id = TagId::from_uuid(Uuid::from_u128(2));
    let second_tag_id = TagId::from_uuid(Uuid::from_u128(3));

    StoredDocumentV1::new(AppData {
        settings: AppSettings::default(),
        folders: vec![FolderEntry {
            id: FolderId::from_uuid(Uuid::from_u128(4)),
            display_name: "项目 文档 🗂️".to_owned(),
            aliases: vec![
                "项目资料".to_owned(),
                "docs".to_owned(),
                "项目资料".to_owned(),
            ],
            path: format!(r"{SENSITIVE_PATH}\非常长的目录\emoji-📁"),
            enabled: true,
            favorite: true,
            pinned: true,
            manual_weight: 25,
            category_id: Some(category_id),
            tag_ids: vec![first_tag_id, second_tag_id, first_tag_id],
            note: "DO-NOT-LEAK-JSON-CONTENT".to_owned(),
            color: Some(FolderColor(0x2563_EBFF)),
            sort_order: -7,
            created_at: utc("2026-09-21T00:00:00Z"),
            updated_at: utc("2026-09-21T00:01:00Z"),
            last_opened_at: Some(utc("2026-09-21T00:02:00Z")),
            open_count: 4,
        }],
        categories: vec![Category {
            id: category_id,
            name: "工作".to_owned(),
            color: Some(FolderColor(0x2563_EBFF)),
        }],
        tags: vec![
            Tag {
                id: first_tag_id,
                name: "重要".to_owned(),
            },
            Tag {
                id: second_tag_id,
                name: "客户 A".to_owned(),
            },
        ],
        revision: 1,
    })
}

#[test]
fn settings_defaults_match_product_baseline() {
    let settings = AppSettings::default();

    assert_eq!(settings.theme, ThemePreference::System);
    assert_eq!(settings.max_results, 8);
    assert_eq!(settings.window_width, 600);
    assert!(settings.search_paths);
    assert!(settings.search_categories);
    assert!(settings.search_tags);
    assert!(!settings.search_notes);
    assert!(settings.fuzzy_matching);
    assert!(settings.search_pinyin);
    assert!(settings.search_english_initials);
    assert_eq!(settings.max_edit_distance, 1);
    assert!(settings.hide_after_open);
    assert!(settings.clear_after_open);
    assert!(settings.hide_on_focus_loss);
    assert!(!settings.launch_at_login);
    assert_eq!(
        settings.empty_query_strategy,
        EmptyQueryStrategy::FavoritesFirst
    );
    assert!(!settings.remember_last_filter);
    assert!(settings.validate().is_ok());
}

#[test]
fn settings_forward_compat_new_fields_default_when_absent() {
    // schema-v1 JSON WITHOUT the M02-B fields must still decode: the fields
    // carry #[serde(default)] so old documents keep appending defaultValue.
    let raw = r#"{
        "schema_version": 1,
        "revision": 1,
        "settings": {
            "theme": "system",
            "max_results": 8,
            "window_width": 600,
            "search_paths": true,
            "search_categories": true,
            "search_tags": true,
            "search_notes": false,
            "fuzzy_matching": true,
            "search_pinyin": true,
            "search_english_initials": true,
            "max_edit_distance": 1,
            "hide_after_open": true,
            "clear_after_open": true,
            "hide_on_focus_loss": true,
            "launch_at_login": false
        },
        "folders": [],
        "categories": [],
        "tags": []
    }"#;
    let decoded = decode(raw.as_bytes()).expect("old settings without new fields must decode");
    assert_eq!(
        decoded.data.settings.empty_query_strategy,
        EmptyQueryStrategy::FavoritesFirst
    );
    assert!(!decoded.data.settings.remember_last_filter);
}

#[test]
fn settings_forward_compat_new_fields_decode_when_present() {
    let raw = r#"{
        "schema_version": 1,
        "revision": 1,
        "settings": {
            "theme": "dark",
            "max_results": 12,
            "window_width": 680,
            "search_paths": true,
            "search_categories": true,
            "search_tags": true,
            "search_notes": false,
            "fuzzy_matching": true,
            "search_pinyin": true,
            "search_english_initials": true,
            "max_edit_distance": 1,
            "hide_after_open": true,
            "clear_after_open": true,
            "hide_on_focus_loss": true,
            "launch_at_login": false,
            "empty_query_strategy": "pinned_only",
            "remember_last_filter": true
        },
        "folders": [],
        "categories": [],
        "tags": []
    }"#;
    let decoded = decode(raw.as_bytes()).expect("settings with new fields must decode");
    assert_eq!(
        decoded.data.settings.empty_query_strategy,
        EmptyQueryStrategy::PinnedOnly
    );
    assert!(decoded.data.settings.remember_last_filter);
}

#[test]
fn settings_outside_allowed_range_are_rejected() {
    let settings = AppSettings {
        max_results: MAX_MAX_RESULTS + 1,
        ..AppSettings::default()
    };

    assert!(settings.validate().is_err());

    let settings = AppSettings {
        max_edit_distance: 3,
        ..AppSettings::default()
    };
    assert!(settings.validate().is_err());
}

#[test]
fn settings_range_boundaries_are_enforced_inclusively() {
    // max_results: MIN-1 rejected; MIN accepted; MAX accepted; MAX+1 rejected.
    {
        let below = AppSettings {
            max_results: MIN_MAX_RESULTS - 1,
            ..AppSettings::default()
        };
        assert!(below.validate().is_err(), "max_results below MIN must fail");
        assert_eq!(
            encode(&StoredDocumentV1::new(AppData {
                settings: below,
                ..fixture_document().data
            }))
            .expect_err("invalid settings document must fail")
            .kind(),
            StorageErrorKind::InvalidDocument
        );

        let at_min = AppSettings {
            max_results: MIN_MAX_RESULTS,
            ..AppSettings::default()
        };
        assert!(at_min.validate().is_ok());

        let at_max = AppSettings {
            max_results: MAX_MAX_RESULTS,
            ..AppSettings::default()
        };
        assert!(at_max.validate().is_ok());

        let above = AppSettings {
            max_results: MAX_MAX_RESULTS + 1,
            ..AppSettings::default()
        };
        assert!(above.validate().is_err());
    }

    // window_width: MIN-1 rejected; MIN accepted; MAX accepted; MAX+1 rejected.
    {
        let below = AppSettings {
            window_width: MIN_WINDOW_WIDTH - 1,
            ..AppSettings::default()
        };
        assert!(below.validate().is_err());

        let at_min = AppSettings {
            window_width: MIN_WINDOW_WIDTH,
            ..AppSettings::default()
        };
        assert!(at_min.validate().is_ok());

        let at_max = AppSettings {
            window_width: MAX_WINDOW_WIDTH,
            ..AppSettings::default()
        };
        assert!(at_max.validate().is_ok());

        let above = AppSettings {
            window_width: MAX_WINDOW_WIDTH + 1,
            ..AppSettings::default()
        };
        assert!(above.validate().is_err());
    }

    // max_edit_distance: 0..=MAX accepted; MAX+1 rejected.
    for distance in 0..=MAX_EDIT_DISTANCE {
        let settings = AppSettings {
            max_edit_distance: distance,
            ..AppSettings::default()
        };
        assert!(
            settings.validate().is_ok(),
            "max_edit_distance {distance} within range must be accepted"
        );
    }
    let above = AppSettings {
        max_edit_distance: MAX_EDIT_DISTANCE + 1,
        ..AppSettings::default()
    };
    assert!(above.validate().is_err());
}

#[test]
fn folder_range_boundaries_are_enforced_inclusively() {
    let base = fixture_document().data;

    // manual_weight: MIN-1 rejected; MIN and MAX accepted; MAX+1 rejected.
    let mut below = base.clone();
    below.folders[0].manual_weight = MIN_MANUAL_WEIGHT - 1;
    assert_eq!(
        encode(&StoredDocumentV1::new(below))
            .expect_err("weight below MIN must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );

    let mut at_min = base.clone();
    at_min.folders[0].manual_weight = MIN_MANUAL_WEIGHT;
    encode(&StoredDocumentV1::new(at_min)).expect("weight at MIN must encode");

    let mut at_max = base.clone();
    at_max.folders[0].manual_weight = MAX_MANUAL_WEIGHT;
    encode(&StoredDocumentV1::new(at_max)).expect("weight at MAX must encode");

    let mut above = base.clone();
    above.folders[0].manual_weight = MAX_MANUAL_WEIGHT + 1;
    assert_eq!(
        encode(&StoredDocumentV1::new(above))
            .expect_err("weight above MAX must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );

    // Exactly MAX_ALIASES_PER_FOLDER distinct aliases accepted; one more rejected.
    let mut at_limit = base.clone();
    at_limit.folders[0].aliases = (0..MAX_ALIASES_PER_FOLDER)
        .map(|index| format!("alias-{index}"))
        .collect();
    encode(&StoredDocumentV1::new(at_limit.clone())).expect("exactly twenty aliases must encode");

    at_limit.folders[0]
        .aliases
        .push(format!("alias-{}", MAX_ALIASES_PER_FOLDER));
    assert_eq!(
        encode(&StoredDocumentV1::new(at_limit))
            .expect_err("more than twenty aliases must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );
}

#[test]
fn empty_alias_after_trim_is_rejected() {
    let mut document = fixture_document();
    document.data.folders[0].aliases = vec!["   ".to_owned()];

    assert_eq!(
        encode(&document)
            .expect_err("an alias that trims to empty must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );
}

#[test]
fn too_long_alias_is_rejected() {
    let mut document = fixture_document();
    document.data.folders[0].aliases = vec!["x".repeat(MAX_ALIAS_LEN + 1)];

    assert_eq!(
        encode(&document)
            .expect_err("an alias above the length limit must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );
}

#[test]
fn too_long_note_is_rejected() {
    let mut document = fixture_document();
    document.data.folders[0].note = "x".repeat(MAX_NOTE_LEN + 1);

    assert_eq!(
        encode(&document)
            .expect_err("a note above the length limit must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );
}

#[test]
fn too_long_category_and_tag_names_are_rejected() {
    let mut long_category = fixture_document();
    long_category.data.categories[0].name = "x".repeat(MAX_CATEGORY_NAME_LEN + 1);
    assert_eq!(
        encode(&long_category)
            .expect_err("a category name above the length limit must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );

    let mut long_tag = fixture_document();
    long_tag.data.tags[0].name = "x".repeat(MAX_TAG_NAME_LEN + 1);
    assert_eq!(
        encode(&long_tag)
            .expect_err("a tag name above the length limit must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );
}

#[test]
fn duplicate_category_and_tag_ids_are_rejected() {
    let mut duplicate_category = fixture_document();
    duplicate_category
        .data
        .categories
        .push(duplicate_category.data.categories[0].clone());
    assert_eq!(
        encode(&duplicate_category)
            .expect_err("a duplicated category ID must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );

    let mut duplicate_tag = fixture_document();
    duplicate_tag
        .data
        .tags
        .push(duplicate_tag.data.tags[0].clone());
    assert_eq!(
        encode(&duplicate_tag)
            .expect_err("a duplicated tag ID must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );
}

#[test]
fn unknown_category_reference_is_rejected() {
    let mut document = fixture_document();
    document.data.folders[0].category_id = Some(CategoryId::from_uuid(Uuid::from_u128(998)));

    assert_eq!(
        encode(&document)
            .expect_err("a folder referencing an unknown category must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );
}

#[test]
fn whitespace_only_folder_name_is_rejected() {
    let mut document = fixture_document();
    document.data.folders[0].display_name = " \t\n ".to_owned();

    let error = encode(&document).expect_err("blank display name must be rejected");
    assert_eq!(error.kind(), StorageErrorKind::InvalidDocument);
}

#[test]
fn duplicate_tags_are_deduplicated_in_first_seen_order() {
    let document = fixture_document();
    let encoded = encode(&document).expect("fixture must encode");
    let decoded = decode(&encoded).expect("encoded fixture must decode");
    let tags = &decoded.data.folders[0].tag_ids;

    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].as_uuid(), Uuid::from_u128(2));
    assert_eq!(tags[1].as_uuid(), Uuid::from_u128(3));
    assert_eq!(decoded.data.folders[0].aliases, ["项目资料", "docs"]);
}

#[test]
fn aliases_are_normalized_and_invalid_manual_weight_is_rejected() {
    let mut document = fixture_document();
    document.data.folders[0].aliases = vec!["  work  ".to_owned(), "work".to_owned()];
    let encoded = encode(&document).expect("normalized aliases must encode");
    let decoded = decode(&encoded).expect("normalized aliases must decode");
    assert_eq!(decoded.data.folders[0].aliases, ["work"]);

    document.data.folders[0].aliases = (0..=20).map(|index| format!("alias-{index}")).collect();
    assert_eq!(
        encode(&document)
            .expect_err("more than twenty unique aliases must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );

    document.data.folders[0].aliases = vec!["work".to_owned()];
    document.data.folders[0].manual_weight = 101;
    assert_eq!(
        encode(&document)
            .expect_err("out-of-range weight must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );
}

#[test]
fn favorite_limit_is_enforced() {
    let mut document = fixture_document();
    let original = document.data.folders[0].clone();
    for index in 1..=5 {
        let mut additional = original.clone();
        additional.id = FolderId::from_uuid(Uuid::from_u128(100 + index));
        additional.favorite = true;
        document.data.folders.push(additional);
    }

    assert_eq!(
        encode(&document)
            .expect_err("more than five favorites must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );
}

#[test]
fn pinned_and_favorite_are_independent_persisted_fields() {
    let document = fixture_document();
    let encoded = encode(&document).expect("fixture must encode");
    let decoded = decode(&encoded).expect("encoded fixture must decode");
    let folder = &decoded.data.folders[0];
    assert!(folder.favorite);
    assert!(folder.pinned);

    let mut folder = fixture_document().data.folders[0].clone();
    folder.pinned = true;
    folder.favorite = false;
    let mut document = fixture_document();
    document.data.folders[0] = folder;
    let decoded =
        decode(&encode(&document).expect("document must encode")).expect("document must decode");
    assert!(decoded.data.folders[0].pinned);
    assert!(!decoded.data.folders[0].favorite);

    let mut folder = fixture_document().data.folders[0].clone();
    folder.pinned = false;
    folder.favorite = true;
    let mut document = fixture_document();
    document.data.folders[0] = folder;
    let decoded =
        decode(&encode(&document).expect("document must encode")).expect("document must decode");
    assert!(!decoded.data.folders[0].pinned);
    assert!(decoded.data.folders[0].favorite);

    let mut many_pinned = fixture_document();
    let original = many_pinned.data.folders[0].clone();
    for index in 1..=10 {
        let mut additional = original.clone();
        additional.id = FolderId::from_uuid(Uuid::from_u128(200 + index));
        additional.pinned = true;
        additional.favorite = false;
        many_pinned.data.folders.push(additional);
    }
    let favorite_count = many_pinned
        .data
        .folders
        .iter()
        .filter(|folder| folder.favorite)
        .count();
    assert!(favorite_count <= MAX_FAVORITES);
    encode(&many_pinned).expect("uncapped pinned folders must remain valid");

    let mut six_favorites = fixture_document();
    let original = six_favorites.data.folders[0].clone();
    for index in 1..=5 {
        let mut additional = original.clone();
        additional.id = FolderId::from_uuid(Uuid::from_u128(300 + index));
        additional.favorite = true;
        six_favorites.data.folders.push(additional);
    }
    assert_eq!(
        encode(&six_favorites)
            .expect_err("six favorites must still be rejected")
            .kind(),
        StorageErrorKind::InvalidDocument
    );
}

#[test]
fn unknown_or_duplicate_references_are_rejected() {
    let mut duplicate_folder = fixture_document();
    duplicate_folder
        .data
        .folders
        .push(duplicate_folder.data.folders[0].clone());
    assert_eq!(
        encode(&duplicate_folder)
            .expect_err("duplicate folder ID must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );

    let mut unknown_tag = fixture_document();
    unknown_tag.data.folders[0]
        .tag_ids
        .push(TagId::from_uuid(Uuid::from_u128(999)));
    assert_eq!(
        encode(&unknown_tag)
            .expect_err("unknown tag reference must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );
}

#[test]
fn removing_category_and_tag_only_clears_references() {
    let mut data = fixture_document().data;
    let category_id = data.categories[0].id;
    let tag_id = data.tags[0].id;
    let folder_id = data.folders[0].id;

    assert!(data.remove_category(category_id));
    assert!(data.remove_tag(tag_id));

    assert_eq!(data.folders.len(), 1);
    assert_eq!(data.folders[0].id, folder_id);
    assert_eq!(data.folders[0].category_id, None);
    assert_eq!(data.folders[0].tag_ids.len(), 1);
    assert!(!data.remove_category(category_id));
    assert!(!data.remove_tag(tag_id));
}

#[test]
fn json_round_trip_is_stable_and_preserves_unicode() {
    let document = fixture_document();
    let first = encode(&document).expect("fixture must encode");
    let decoded = decode(&first).expect("fixture must decode");
    let second = encode(&decoded).expect("decoded fixture must encode");

    assert_eq!(first, second);
    assert_eq!(decoded.data.folders[0].display_name, "项目 文档 🗂️");
    assert!(decoded.data.folders[0].path.contains("机密 文件夹"));
    assert!(decoded.data.folders[0].path.contains("emoji-📁"));
}

#[test]
fn invalid_or_truncated_json_is_distinguished_without_content_leakage() {
    let bytes = format!(r#"{{"schema_version":1,"note":"{SENSITIVE_JSON_MARKER}""#).into_bytes();
    let error = decode(&bytes).expect_err("truncated JSON must fail");

    assert_eq!(error.kind(), StorageErrorKind::InvalidJson);
    assert!(!error.to_string().contains(SENSITIVE_JSON_MARKER));
    assert!(!error.to_string().contains(SENSITIVE_PATH));
}

#[test]
fn unknown_fields_are_rejected_without_silent_data_loss() {
    let encoded = encode(&fixture_document()).expect("fixture must encode");
    let mut value: serde_json::Value = serde_json::from_slice(&encoded).expect("fixture JSON");
    value["unexpected_field"] = serde_json::json!("ignored-data-is-not-allowed");
    let bytes = serde_json::to_vec(&value).expect("test JSON must encode");

    let error = decode(&bytes).expect_err("unknown data must not be silently discarded");
    assert_eq!(error.kind(), StorageErrorKind::InvalidJson);
    assert!(!error.to_string().contains("ignored-data-is-not-allowed"));
}

#[test]
fn future_schema_is_rejected_without_defaulting() {
    let error = decode(br#"{"schema_version":2}"#).expect_err("future schema must fail");

    assert_eq!(error.kind(), StorageErrorKind::UnsupportedFutureSchema);
    assert_eq!(error.schema_version(), Some(2));
}

#[test]
fn legacy_schema_requires_explicit_migration() {
    let error = decode(br#"{"schema_version":0}"#).expect_err("legacy schema must not default");

    assert_eq!(error.kind(), StorageErrorKind::MigrationRequired);
    assert_eq!(error.schema_version(), Some(0));
}

#[test]
fn invalid_revision_and_overflow_are_rejected() {
    let mut invalid = fixture_document();
    invalid.data.revision = 0;
    assert_eq!(
        encode(&invalid)
            .expect_err("zero revision must fail")
            .kind(),
        StorageErrorKind::InvalidDocument
    );

    let mut overflow = fixture_document().data;
    overflow.revision = u64::MAX;
    assert!(overflow.next_revision().is_err());
}

#[test]
fn validation_errors_do_not_expose_sensitive_path_or_json() {
    let mut document = fixture_document();
    document.data.folders[0].display_name = " ".to_owned();
    let error = encode(&document).expect_err("invalid document must fail");

    assert_eq!(error.kind(), StorageErrorKind::InvalidDocument);
    assert!(!error.to_string().contains(SENSITIVE_PATH));
    assert!(!error.to_string().contains(SENSITIVE_JSON_MARKER));
}

#[test]
fn domain_does_not_expose_real_directory_delete_api() {
    let source = include_str!("../domain/document.rs");
    assert!(!source.contains("remove_dir"));
    assert!(!source.contains("remove_file"));
    assert!(!source.contains("delete_directory"));
}

/// Recursive source scan of `src/storage/`, `src/domain/`, `src/presentation/`
/// and `src/platform/`: no described call may delete a real folder or the real
/// stored document.
///
/// - `remove_dir` / `remove_dir_all` / `delete_directory` must not appear in
///   any source under these trees — folder deletion is never represented at
///   this layer.
/// - `remove_file` is permitted only inside the `src/storage/io.rs` filesystem
///   adapter (used purely for best-effort cleanup of the repository's own
///   `data.json.tmp.*` siblings and the lock file); it must not appear in
///   `repository.rs`, anywhere else in `src/storage/`, or anywhere in
///   `src/domain/`, `src/presentation/`, or `src/platform/`.
///
/// M05 review M4: the original guard only scanned `src/storage/` + `src/domain/`.
/// The management milestone added large new presenters (`management.rs`,
/// `manager.rs`, `view_model.rs`) and platform adapters (`folder_picker.rs`,
/// `clipboard.rs`), so the structural scan now includes them — a future call to a
/// real-directory/real-file deletion API in those layers is caught at CI time
/// instead of being verified only by hand.
#[test]
fn source_under_storage_and_domain_has_no_fs_delete_api_calls() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    let mut pending = vec![
        source_root.join("storage"),
        source_root.join("domain"),
        source_root.join("presentation"),
        source_root.join("platform"),
    ];
    let mut seen_roots = std::collections::HashSet::new();
    while let Some(dir) = pending.pop() {
        if !seen_roots.insert(dir.clone()) {
            continue;
        }
        for entry in std::fs::read_dir(&dir).expect("src tree must be readable") {
            let entry = entry.expect("directory entry must be readable");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push(path);
            }
        }
    }
    assert!(
        sources.len() >= 2,
        "src/storage, src/domain, src/presentation and src/platform must contain sources"
    );

    for path in &sources {
        // Only implementation sources are scanned; the test files themselves
        // legitimately mention these tokens inside their assertions.
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if file_name.ends_with("tests.rs") {
            continue;
        }
        let source = std::fs::read_to_string(path).expect("source must be readable");
        for forbidden in ["remove_dir", "remove_dir_all", "delete_directory"] {
            assert!(
                !source.contains(forbidden),
                "{forbidden} must not appear in {}",
                path.display()
            );
        }
        let is_io_adapter = path
            .parent()
            .is_some_and(|parent| parent.ends_with("storage"))
            && path.file_name().is_some_and(|name| name == "io.rs");
        if !is_io_adapter {
            assert!(
                !source.contains("remove_file"),
                "remove_file must not appear outside the storage io.rs adapter: {}",
                path.display()
            );
        }
    }
}

/// M07.5 supply-chain/security source guard: no implementation source may spawn
/// a shell or command interpreter with user/derived strings, and no full user
/// path / query may be written to a file log.
///
/// The only file logging in the tree is the redacted panic log
/// (`src/diagnostics.rs`, `impl_log_panic`), which writes fixed anonymous lines
/// (`PANIC_LOG_*` constants), so the `write_flush_sync`-shaped disk writers in
/// the storage layer are the ONLY `std::fs::write`/`File::create`-style paths.
/// We deliberately scan the four non-test trees for:
///
/// - process-spawn / command-interpreter tokens. 0.0.1 opens folders purely via
///   `ShellExecuteExW(lpFile=path)` (no verb, no parameters — see
///   `platform/windows/tray_open.rs`), so `Command::new`, `std::process::Command`,
///   `CreateProcess`, `powershell`, and `cmd.exe` must never appear;
/// - a `set_hook`-adjacent logging pattern that could echo `eprintln!` with a
///   user value at a call site (guarded structurally by requiring every
///   `eprintln!`/`println!` argument list in implementation sources to contain
///   no `path`/`query`/`text` variable usages — too fragile to express as a
///   token check, so we rely on the anonymous-string review plus this scan for
///   the dangerous tokens instead).
#[test]
fn implementation_sources_do_not_invoke_a_shell_or_command_interpreter() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources = Vec::new();
    let mut pending_dirs = vec![
        source_root.join("storage"),
        source_root.join("domain"),
        source_root.join("presentation"),
        source_root.join("platform"),
    ];
    // Top-level module files (not under one of the above trees) are appended
    // explicitly so main.rs/app.rs/diagnostics.rs/lib.rs are scanned too.
    for file in ["app.rs", "diagnostics.rs", "main.rs", "lib.rs"] {
        sources.push(source_root.join(file));
    }
    let mut seen = std::collections::HashSet::new();
    while let Some(dir) = pending_dirs.pop() {
        if !seen.insert(dir.clone()) {
            continue;
        }
        let metadata = std::fs::metadata(&dir);
        if metadata.is_err() || !metadata.unwrap().is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&dir).expect("src tree must be readable") {
            let entry = entry.expect("directory entry must be readable");
            let path = entry.path();
            if path.is_dir() {
                pending_dirs.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                sources.push(path);
            }
        }
    }
    let forbidden = [
        "std::process::Command",
        "Command::new",
        "CreateProcess",
        "powershell",
        "cmd.exe",
        "cmd /c",
        "ShCreateProcess",
    ];
    for path in &sources {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if file_name.ends_with("tests.rs") {
            continue;
        }
        let source = std::fs::read_to_string(path).expect("source must be readable");
        for token in forbidden {
            assert!(
                !source.contains(token),
                "command-interpreter token {token:?} must not appear in {}",
                path.display()
            );
        }
    }
    // The ShellExecuteExW boundary is the one allowed shell entry; it must keep
    // passing the path solely via lpFile with NO verb and NO parameters, so a
    // user path can never be woven into a command line. The `shell_open.rs`
    // controller asserts the same contract at the API level.
    let tray_open = std::fs::read_to_string(source_root.join("platform/windows/tray_open.rs"))
        .expect("tray_open.rs present");
    assert!(
        tray_open.contains("ShellExecuteExW"),
        "shell boundary alive"
    );
    assert!(
        tray_open.contains("lpVerb: windows::core::PCWSTR::null()"),
        "the shell boundary must not pass a verb"
    );
    assert!(
        tray_open.contains("lpParameters: windows::core::PCWSTR::null()"),
        "the shell boundary must not pass parameters"
    );
}

/// M07.5 supply-chain guard: every GitHub Actions workflow keeps minimal
/// permissions (`contents: read`), never runs untrusted PR code with write
/// access, and pins every third-party `uses:` step to a full 40-character
/// commit SHA (not a floating branch/tag or a short prefix). The Windows CI and
/// release candidate workflows are the only authenticated shells into this
/// repo, so their security posture is structural and enforced here.
#[test]
fn workflows_pin_actions_and_use_minimal_permissions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows");
    let workflow_files = [
        root.join("ci.yml"),
        root.join("benchmark.yml"),
        root.join("release.yml"),
    ];
    for file in &workflow_files {
        assert!(file.exists(), "workflow {} must exist", file.display());
        let source = std::fs::read_to_string(file).expect("workflow must be readable");

        // Minimal permissions: require a `permissions:` block with
        // `contents: read` and never write.
        assert!(
            source.contains("permissions:"),
            "{} must declare a permissions block",
            file.display()
        );
        assert!(
            source.contains("contents: read"),
            "{} must have minimal contents: read",
            file.display()
        );
        assert!(
            !source.contains("contents: write"),
            "{} must not hold contents write",
            file.display()
        );

        // Every `uses:` reference must be pinned to a full 40-char SHA.
        for line in source.lines() {
            let line = line.trim();
            let Some(use_at) = line.find("uses:") else {
                continue;
            };
            let uses = &line[use_at + "uses:".len()..];
            let spec = uses.split('#').next().unwrap_or("").trim();
            assert!(
                spec.starts_with("actions/")
                    || spec.starts_with("dtolnay/")
                    || spec.starts_with("Swatinem/")
                    || spec.starts_with("taiki-e/"),
                "unexpected action provider in {}: {spec:?}",
                file.display(),
            );
            assert!(
                spec.len() > 40 && spec.contains('@'),
                "{}: action must be pinned to a full commit SHA, got {spec:?}",
                file.display()
            );
            let after_at = spec.split('@').nth(1).unwrap_or_default();
            assert!(
                after_at.len() >= 40,
                "{}: action pin must be a full 40-char SHA, got {after_at:?}",
                file.display()
            );
        }
    }
}

/// M07.5: the release-candidate workflow must not run untrusted PR code with a
/// write token. It is `workflow_dispatch`-only (no `pull_request_target`), the
/// checkout uses `persist-credentials: false`, and there is no release job.
#[test]
fn release_workflow_never_runs_untrusted_code_with_write() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows");
    let source = std::fs::read_to_string(root.join("release.yml")).expect("release.yml present");

    assert!(
        source.contains("workflow_dispatch") && !source.contains("pull_request_target"),
        "release.yml must be dispatch-only (never pull_request_target)"
    );
    assert!(
        source.contains("persist-credentials: false"),
        "release.yml checkout must not persist credentials"
    );
    assert!(
        !source.contains("permissions: write-all")
            && !source.contains("contents: write")
            && !source.contains("issues: write"),
        "release.yml must hold no write scopes"
    );
    assert!(
        !source.contains("gh release create") && !source.contains("on.release"),
        "release.yml must not create GitHub releases (separate authorized step)"
    );
}
