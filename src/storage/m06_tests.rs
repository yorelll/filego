//! M06 repository-level tests: settings round-trip through a REAL file,
//! reset-vs-clear-all separation, `set_settings`/`set_data` semantics, and the
//! clear-all no-real-directory canary.

use chrono::{DateTime, Utc};
use tempfile::TempDir;
use uuid::Uuid;

use crate::{
    domain::{
        document::AppData,
        folder::{Category, FolderColor, FolderEntry, Tag},
        ids::{CategoryId, FolderId, TagId},
        settings::{AppSettings, LanguagePreference, MonitorStrategy, RowHeightPreference},
    },
    storage::{
        location::DocumentPaths,
        repository::{DocumentRepository, LoadOutcome},
        schema::StoredDocumentV1,
    },
};

fn utc(value: &str) -> DateTime<Utc> {
    value.parse().expect("fixed RFC3339 fixture must parse")
}

fn folder(id: u128, name: &str, path: &str) -> FolderEntry {
    let now = utc("2026-09-21T00:00:00Z");
    FolderEntry {
        id: FolderId::from_uuid(Uuid::from_u128(id)),
        display_name: name.to_owned(),
        aliases: Vec::new(),
        path: path.to_owned(),
        enabled: true,
        favorite: false,
        pinned: false,
        manual_weight: 0,
        category_id: None,
        tag_ids: Vec::new(),
        note: String::new(),
        color: None,
        sort_order: 0,
        created_at: now,
        updated_at: now,
        last_opened_at: None,
        open_count: 0,
    }
}

fn seed() -> StoredDocumentV1 {
    let category_id = CategoryId::from_uuid(Uuid::from_u128(1));
    let tag_a = TagId::from_uuid(Uuid::from_u128(2));
    StoredDocumentV1::new(AppData {
        settings: AppSettings::default(),
        folders: vec![folder(10, "Docs", r"C:\docs")],
        categories: vec![Category {
            id: category_id,
            name: "工作".to_owned(),
            color: Some(FolderColor(0x2563_EBFF)),
        }],
        tags: vec![Tag {
            id: tag_a,
            name: "重要".to_owned(),
        }],
        revision: 2,
    })
}

fn real_repo(base: &TempDir) -> DocumentRepository {
    DocumentRepository::new(DocumentPaths::from_base_dir(base.path()))
}

#[test]
fn overwrite_import_snapshot_restores_the_pre_import_document() {
    // M06 review M1: before an overwrite-mode import APPLIES, the adapter takes
    // an explicit `backup-before-import-*.json` snapshot of the CURRENT
    // (pre-import) document. Replicate that sequence against a real directory:
    // seed current -> plan/apply overwrite -> snapshot pre-import -> replace +
    // save; then assert a restorable backup exists that decodes to the exact
    // PRE-import document (not the post-import one).
    use crate::storage::{backup, import_export};
    let base = TempDir::new().expect("temp dir");
    let mut repo = real_repo(&base);
    repo.save(&seed()).expect("seed save");

    let mut repo = real_repo(&base);
    repo.load().expect("load");
    let pre = repo.document().expect("pre-import doc").clone();

    // The incoming document adds a record with a fresh path (no collision);
    // in Overwrite mode that is still an explicit user action that mutates the
    // document (a new record lands), so a pre-import snapshot must exist.
    let incoming_doc = seed();
    let incoming = AppData {
        folders: vec![folder(99, "Imported", r"D:\imported")],
        ..incoming_doc.data
    };
    let (applied, _) =
        import_export::apply_import(&pre.data, &incoming, import_export::ImportMode::Overwrite)
            .expect("apply");
    assert_eq!(applied.data.folders.len(), 2, "new record added");

    // The adapter creates the pre-import snapshot via the standard backup
    // mechanism under a recognizable before-import stamp, then applies.
    let stamp = format!("before-import-{}", Utc::now().format("%Y%m%d-%H%M%S"));
    backup::create_backup(base.path(), &pre, &stamp).expect("pre-import snapshot");
    repo.set_data(applied.data).expect("set_data");
    repo.save_at().expect("save after overwrite import");

    // Assert: a backup-before-import file exists, is listed, and decodes to
    // the EXACT pre-import document (the old record + old revision).
    let names = backup::list_backups(base.path()).expect("list");
    let snapshot = names
        .iter()
        .find(|name| name.starts_with("backup-before-import-"))
        .cloned()
        .expect("a before-import snapshot must exist after an overwrite import");
    let decoded = backup::read_backup(base.path(), &snapshot).expect("decode snapshot");
    assert_eq!(decoded, pre, "snapshot decodes to the pre-import document");
    assert_eq!(decoded.data.folders.len(), 1);
    assert_eq!(decoded.data.folders[0].path, r"C:\docs");
    assert_eq!(decoded.data.revision, 2);
    // The live document is the applied (post-import) one.
    let live = repo.document().expect("live doc");
    assert_eq!(live.data.folders.len(), 2, "imported record landed live");
    assert!(
        live.data
            .folders
            .iter()
            .any(|f| f.display_name == "Imported")
    );
    assert_ne!(live, &pre, "live document changed by the import");
}

#[test]
fn settings_round_trip_through_a_real_file() {
    let base = TempDir::new().expect("temp dir");
    let mut repo = real_repo(&base);
    repo.save(&seed()).expect("seed save");

    // M06 settings persisted through the repository and reloaded from disk:
    // UI command -> set_settings -> save_at -> fresh repo load -> same values.
    let mut repo = real_repo(&base);
    let loaded = repo.load().expect("load");
    match loaded {
        LoadOutcome::Found(document) => {
            assert_eq!(
                document.data.settings.language_preference,
                LanguagePreference::System
            );
            assert_eq!(
                document.data.settings.monitor_strategy,
                MonitorStrategy::Mouse
            );
            assert_eq!(
                document.data.settings.row_height_preference,
                RowHeightPreference::Compact
            );
        }
        other => panic!("expected Found, got {other:?}"),
    }

    let changed = AppSettings {
        language_preference: LanguagePreference::EnUS,
        monitor_strategy: MonitorStrategy::ActiveWindow,
        row_height_preference: RowHeightPreference::Standard,
        silent_start: false,
        show_main_window_at_startup: true,
        font_scale_percent: 120,
        settings_window_width: Some(1000),
        search_window_width: Some(700),
        recent_sort_first: true,
        ..AppSettings::default()
    };
    repo.set_settings(changed.clone()).expect("set_settings");
    let new_rev = repo.save_at().expect("save");

    let mut fresh = real_repo(&base);
    let reloaded = fresh.load().expect("load");
    match reloaded {
        LoadOutcome::Found(document) => {
            assert_eq!(document.data.settings, changed);
            assert!(document.data.revision > 2);
            assert!(document.data.settings.recent_sort_first);
            // The document data is unchanged (folders/categories/tags intact).
            assert_eq!(document.data.folders.len(), 1);
            assert_eq!(document.data.categories.len(), 1);
        }
        other => panic!("expected Found, got {other:?}"),
    }
    let _ = new_rev;
}

#[test]
fn set_data_replaces_the_whole_data_block_and_persists() {
    let base = TempDir::new().expect("temp dir");
    let mut repo = real_repo(&base);
    repo.save(&seed()).expect("seed save");

    let mut repo = real_repo(&base);
    repo.load().expect("load");
    let original_settings = repo.document().unwrap().data.settings.clone();

    // Replace the whole data block (backup restore / import apply path).
    let replaced = AppData {
        settings: original_settings.clone(),
        folders: vec![folder(99, "Imported", r"D:\imported")],
        categories: Vec::new(),
        tags: Vec::new(),
        revision: 5,
    };
    repo.set_data(replaced.clone()).expect("set_data");
    let rev = repo.save_at().expect("save");
    assert!(rev > 5);

    let mut fresh = real_repo(&base);
    match fresh.load().expect("load") {
        LoadOutcome::Found(document) => {
            assert_eq!(document.data.folders.len(), 1);
            assert_eq!(document.data.folders[0].display_name, "Imported");
            assert_eq!(document.data.categories.len(), 0);
            assert_eq!(document.data.settings, original_settings);
        }
        other => panic!("expected Found, got {other:?}"),
    }
}

#[test]
fn clear_all_records_keeps_every_real_directory_and_settings() {
    // The canary: clear-all removes only folder RECORDS; the real directories
    // and their contents survive byte-for-byte, and settings are untouched.
    let base = TempDir::new().expect("temp dir");
    let real_a = base.path().join("real A").to_path_buf();
    let real_b = base.path().join("真实 乙 🗂").to_path_buf();
    std::fs::create_dir(&real_a).expect("canary helper");
    std::fs::create_dir(&real_b).expect("canary helper");
    let inner = real_a.join("nested");
    std::fs::create_dir(&inner).expect("canary helper");
    std::fs::write(inner.join("keep.txt"), b"keep me").expect("canary helper");

    let mut doc = seed();
    doc.data.folders = vec![
        folder(10, "A", real_a.to_str().unwrap()),
        folder(11, "B", real_b.to_str().unwrap()),
    ];
    let settings = doc.data.settings.clone();
    let mut repo = real_repo(&base);
    repo.save(&doc).expect("seed save");

    let mut repo = real_repo(&base);
    repo.load().expect("load");
    assert_eq!(repo.folder_count(), 2);
    assert!(repo.clear_all_records());
    assert_eq!(repo.folder_count(), 0);
    repo.save_at().expect("save after clear");

    // Real directories + contents survive.
    assert!(real_a.exists());
    assert!(real_b.exists());
    assert!(inner.exists());
    assert!(inner.join("keep.txt").exists());

    // Settings survive clear-all (separate surface from records).
    let mut fresh = real_repo(&base);
    match fresh.load().expect("load") {
        LoadOutcome::Found(document) => {
            assert!(document.data.folders.is_empty());
            assert_eq!(document.data.settings, settings);
        }
        other => panic!("expected Found, got {other:?}"),
    }
}

#[test]
fn clear_all_records_with_no_records_is_a_noop() {
    let base = TempDir::new().expect("temp dir");
    let mut empty = seed();
    empty.data.folders.clear();
    let mut repo = real_repo(&base);
    repo.save(&empty).expect("seed save");
    let mut repo = real_repo(&base);
    repo.load().expect("load");
    assert!(!repo.clear_all_records(), "no folders -> no change");
}

#[test]
fn reset_settings_keeps_folder_data_and_is_separate_from_clear_all() {
    // The settings controller keeps folder data on RestoreDefaultSettings; at
    // the repository level this is just `set_settings(defaults)` — verify the
    // two surfaces (settings vs records) never cross.
    let base = TempDir::new().expect("temp dir");
    let mut doc = seed();
    doc.data.settings.silent_start = false;
    doc.data.settings.font_scale_percent = 130;
    doc.data.settings.recent_sort_first = true;
    let mut repo = real_repo(&base);
    repo.save(&doc).expect("seed save");

    let mut repo = real_repo(&base);
    repo.load().expect("load");
    let folder_count = repo.folder_count();
    let defaults = AppSettings::default();
    repo.set_settings(defaults.clone()).expect("reset settings");
    repo.save_at().expect("save reset");

    let mut fresh = real_repo(&base);
    match fresh.load().expect("load") {
        LoadOutcome::Found(document) => {
            assert_eq!(document.data.settings, defaults);
            assert_eq!(document.data.settings.font_scale_percent, 100);
            assert!(!document.data.settings.recent_sort_first);
            assert_eq!(
                document.data.folders.len(),
                folder_count,
                "records untouched"
            );
            assert_eq!(document.data.categories.len(), 1);
            assert_eq!(document.data.tags.len(), 1);
        }
        other => panic!("expected Found, got {other:?}"),
    }
}

#[test]
fn old_document_without_m06_settings_fields_decodes_with_defaults() {
    // A schema-v1 document written before M06 (all pre-M06 settings present,
    // none of the M06 fields) must load with M06 defaults — never an error.
    let base = TempDir::new().expect("temp dir");
    let doc = seed();
    let base = base.path().join("data");
    let paths = DocumentPaths::from_base_dir(&base);
    std::fs::create_dir_all(&base).expect("mkdir");
    let mut repo = DocumentRepository::new(paths);
    repo.save(&doc).expect("seed save");
    drop(repo);

    // Rewrite the main bytes WITHOUT the M06 fields by encoding a document
    // built from AppSettings::default() (which now includes M06 fields by
    // serde) — instead simulate an old file by deleting the M06 keys from the
    // JSON. Use the serde_json Value round-trip.
    let main_path = crate::storage::location::main_document_path(&base);
    let bytes = std::fs::read(&main_path).expect("main bytes");
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let settings = value
        .as_object_mut()
        .and_then(|root| root.remove("settings"))
        .expect("settings");
    let mut clean = serde_json::Map::new();
    for (k, v) in settings.as_object().expect("settings object") {
        // Drop every M06 field; keep the pre-M06 set.
        if !matches!(
            k.as_str(),
            "silent_start"
                | "show_main_window_at_startup"
                | "launch_at_login_wired"
                | "monitor_strategy"
                | "language_preference"
                | "row_height_preference"
                | "search_window_width"
                | "settings_window_width"
                | "font_scale_percent"
                | "show_path_in_results"
                | "show_category_tag_in_results"
                | "highlight_results"
                | "search_aliases"
                | "recent_sort_first"
        ) {
            clean.insert(k.clone(), v.clone());
        }
    }
    value
        .as_object_mut()
        .unwrap()
        .insert("settings".into(), serde_json::Value::Object(clean));
    std::fs::write(&main_path, serde_json::to_vec(&value).unwrap()).expect("rewrite");

    let mut repo = DocumentRepository::new(DocumentPaths::from_base_dir(&base));
    let loaded = repo.load().expect("old doc must load");
    match loaded {
        LoadOutcome::Found(document) => {
            assert!(document.data.settings.silent_start, "M06 default true");
            assert!(!document.data.settings.show_main_window_at_startup);
            assert_eq!(
                document.data.settings.monitor_strategy,
                MonitorStrategy::Mouse
            );
        }
        other => panic!("expected Found, got {other:?}"),
    }
}
