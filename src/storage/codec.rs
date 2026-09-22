use std::fmt;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::domain::{
    document::AppData,
    error::ValidationError,
    folder::{Category, FolderColor, FolderEntry, Tag},
    ids::{CategoryId, FolderId, TagId},
    settings::AppSettings,
};

use super::schema::{CURRENT_SCHEMA_VERSION, StoredDocumentV1};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageErrorKind {
    InvalidJson,
    UnsupportedFutureSchema,
    MigrationRequired,
    InvalidDocument,
    EncodeFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageError {
    kind: StorageErrorKind,
    schema_version: Option<u32>,
}

impl StorageError {
    const fn new(kind: StorageErrorKind) -> Self {
        Self {
            kind,
            schema_version: None,
        }
    }

    const fn with_schema_version(kind: StorageErrorKind, schema_version: u32) -> Self {
        Self {
            kind,
            schema_version: Some(schema_version),
        }
    }

    pub const fn kind(&self) -> StorageErrorKind {
        self.kind
    }

    pub const fn schema_version(&self) -> Option<u32> {
        self.schema_version
    }
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.kind, self.schema_version) {
            (StorageErrorKind::InvalidJson, _) => {
                formatter.write_str("stored data is not valid JSON")
            }
            (StorageErrorKind::UnsupportedFutureSchema, Some(version)) => {
                write!(
                    formatter,
                    "stored data uses unsupported future schema version {version}"
                )
            }
            (StorageErrorKind::MigrationRequired, Some(version)) => {
                write!(
                    formatter,
                    "stored data schema version {version} requires migration"
                )
            }
            (StorageErrorKind::InvalidDocument, _) => {
                formatter.write_str("stored data violates document validation rules")
            }
            (StorageErrorKind::EncodeFailed, _) => {
                formatter.write_str("stored data could not be encoded")
            }
            (_, _) => formatter.write_str("stored data could not be processed"),
        }
    }
}

impl std::error::Error for StorageError {}

#[derive(Debug, Deserialize)]
struct SchemaProbe {
    schema_version: u32,
}

pub fn encode(document: &StoredDocumentV1) -> Result<Vec<u8>, StorageError> {
    let mut validated = document.clone();
    validate_current_document(&mut validated)?;

    serde_json::to_vec_pretty(&validated)
        .map_err(|_| StorageError::new(StorageErrorKind::EncodeFailed))
}

pub fn decode(bytes: &[u8]) -> Result<StoredDocumentV1, StorageError> {
    let probe: SchemaProbe = serde_json::from_slice(bytes)
        .map_err(|_| StorageError::new(StorageErrorKind::InvalidJson))?;

    migrate_to_current(bytes, probe.schema_version)
}

fn migrate_to_current(bytes: &[u8], schema_version: u32) -> Result<StoredDocumentV1, StorageError> {
    if schema_version > CURRENT_SCHEMA_VERSION {
        return Err(StorageError::with_schema_version(
            StorageErrorKind::UnsupportedFutureSchema,
            schema_version,
        ));
    }

    if schema_version < CURRENT_SCHEMA_VERSION {
        return Err(StorageError::with_schema_version(
            StorageErrorKind::MigrationRequired,
            schema_version,
        ));
    }

    let wire: WireDocumentV1 = serde_json::from_slice(bytes)
        .map_err(|_| StorageError::new(StorageErrorKind::InvalidJson))?;
    let mut document = wire.into_domain();
    validate_current_document(&mut document)?;
    Ok(document)
}

fn validate_current_document(document: &mut StoredDocumentV1) -> Result<(), StorageError> {
    if document.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(StorageError::with_schema_version(
            StorageErrorKind::MigrationRequired,
            document.schema_version,
        ));
    }

    document
        .data
        .validate_and_normalize()
        .map_err(validation_error_to_storage_error)
}

fn validation_error_to_storage_error(_: ValidationError) -> StorageError {
    StorageError::new(StorageErrorKind::InvalidDocument)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireDocumentV1 {
    schema_version: u32,
    settings: AppSettings,
    folders: Vec<WireFolderEntry>,
    categories: Vec<WireCategory>,
    tags: Vec<WireTag>,
    revision: u64,
}

impl WireDocumentV1 {
    fn into_domain(self) -> StoredDocumentV1 {
        StoredDocumentV1 {
            schema_version: self.schema_version,
            data: AppData {
                settings: self.settings,
                folders: self
                    .folders
                    .into_iter()
                    .map(WireFolderEntry::into_domain)
                    .collect(),
                categories: self
                    .categories
                    .into_iter()
                    .map(WireCategory::into_domain)
                    .collect(),
                tags: self.tags.into_iter().map(WireTag::into_domain).collect(),
                revision: self.revision,
            },
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireFolderEntry {
    id: Uuid,
    display_name: String,
    aliases: Vec<String>,
    path: String,
    enabled: bool,
    favorite: bool,
    manual_weight: i16,
    category_id: Option<Uuid>,
    tag_ids: Vec<Uuid>,
    note: String,
    color: Option<FolderColor>,
    sort_order: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    last_opened_at: Option<DateTime<Utc>>,
    open_count: u64,
}

impl WireFolderEntry {
    fn into_domain(self) -> FolderEntry {
        FolderEntry {
            id: FolderId::from_uuid(self.id),
            display_name: self.display_name,
            aliases: self.aliases,
            path: self.path,
            enabled: self.enabled,
            favorite: self.favorite,
            manual_weight: self.manual_weight,
            category_id: self.category_id.map(CategoryId::from_uuid),
            tag_ids: self.tag_ids.into_iter().map(TagId::from_uuid).collect(),
            note: self.note,
            color: self.color,
            sort_order: self.sort_order,
            created_at: self.created_at,
            updated_at: self.updated_at,
            last_opened_at: self.last_opened_at,
            open_count: self.open_count,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCategory {
    id: Uuid,
    name: String,
    color: Option<FolderColor>,
}

impl WireCategory {
    fn into_domain(self) -> Category {
        Category {
            id: CategoryId::from_uuid(self.id),
            name: self.name,
            color: self.color,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireTag {
    id: Uuid,
    name: String,
}

impl WireTag {
    fn into_domain(self) -> Tag {
        Tag {
            id: TagId::from_uuid(self.id),
            name: self.name,
        }
    }
}
