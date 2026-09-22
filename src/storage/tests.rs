use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    domain::{
        document::AppData,
        folder::{Category, FolderColor, FolderEntry, MAX_FAVORITES, Tag},
        ids::{CategoryId, FolderId, TagId},
        settings::{AppSettings, MAX_MAX_RESULTS, ThemePreference},
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
    assert!(settings.validate().is_ok());
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
