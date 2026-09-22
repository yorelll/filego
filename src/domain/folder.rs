use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    error::{ValidationError, ValidationErrorKind},
    ids::{CategoryId, FolderId, TagId},
};

pub const MAX_DISPLAY_NAME_LEN: usize = 255;
pub const MAX_PATH_LEN: usize = 32_767;
pub const MAX_ALIASES_PER_FOLDER: usize = 20;
pub const MAX_ALIAS_LEN: usize = 255;
pub const MIN_MANUAL_WEIGHT: i16 = -100;
pub const MAX_MANUAL_WEIGHT: i16 = 100;
pub const MAX_FAVORITES: usize = 5;
pub const MAX_NOTE_LEN: usize = 4_096;
pub const MAX_CATEGORY_NAME_LEN: usize = 128;
pub const MAX_TAG_NAME_LEN: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FolderColor(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Category {
    pub id: CategoryId,
    pub name: String,
    pub color: Option<FolderColor>,
}

impl Category {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_named_value(
            &self.name,
            MAX_CATEGORY_NAME_LEN,
            ValidationErrorKind::CategoryNameTooLong,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    pub id: TagId,
    pub name: String,
}

impl Tag {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_named_value(
            &self.name,
            MAX_TAG_NAME_LEN,
            ValidationErrorKind::TagNameTooLong,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderEntry {
    pub id: FolderId,
    pub display_name: String,
    pub aliases: Vec<String>,
    pub path: String,
    pub enabled: bool,
    pub favorite: bool,
    pub pinned: bool,
    pub manual_weight: i16,
    pub category_id: Option<CategoryId>,
    pub tag_ids: Vec<TagId>,
    pub note: String,
    pub color: Option<FolderColor>,
    pub sort_order: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_opened_at: Option<DateTime<Utc>>,
    pub open_count: u64,
}

impl FolderEntry {
    pub fn validate_and_normalize(&mut self) -> Result<(), ValidationError> {
        if self.display_name.trim().is_empty() {
            return Err(ValidationError::new(ValidationErrorKind::EmptyDisplayName));
        }

        if self.display_name.chars().count() > MAX_DISPLAY_NAME_LEN {
            return Err(ValidationError::new(
                ValidationErrorKind::DisplayNameTooLong,
            ));
        }

        if self.path.trim().is_empty() {
            return Err(ValidationError::new(ValidationErrorKind::EmptyPath));
        }

        if self.path.chars().count() > MAX_PATH_LEN {
            return Err(ValidationError::new(ValidationErrorKind::PathTooLong));
        }

        normalize_aliases(&mut self.aliases)?;

        if self.aliases.len() > MAX_ALIASES_PER_FOLDER {
            return Err(ValidationError::new(ValidationErrorKind::TooManyAliases));
        }

        if !(MIN_MANUAL_WEIGHT..=MAX_MANUAL_WEIGHT).contains(&self.manual_weight) {
            return Err(ValidationError::new(
                ValidationErrorKind::ManualWeightOutOfRange,
            ));
        }

        if self.note.chars().count() > MAX_NOTE_LEN {
            return Err(ValidationError::new(ValidationErrorKind::NoteTooLong));
        }

        deduplicate_tag_ids(&mut self.tag_ids);
        Ok(())
    }
}

fn validate_named_value(
    value: &str,
    maximum_length: usize,
    too_long_kind: ValidationErrorKind,
) -> Result<(), ValidationError> {
    if value.trim().is_empty() || value.chars().count() > maximum_length {
        return Err(ValidationError::new(too_long_kind));
    }

    Ok(())
}

fn normalize_aliases(aliases: &mut Vec<String>) -> Result<(), ValidationError> {
    let mut deduplicated = Vec::with_capacity(aliases.len());
    for alias in aliases.drain(..) {
        let normalized = alias.trim().to_owned();
        if normalized.is_empty() {
            return Err(ValidationError::new(ValidationErrorKind::EmptyAlias));
        }
        if normalized.chars().count() > MAX_ALIAS_LEN {
            return Err(ValidationError::new(ValidationErrorKind::AliasTooLong));
        }
        if !deduplicated.contains(&normalized) {
            deduplicated.push(normalized);
        }
    }
    *aliases = deduplicated;
    Ok(())
}

fn deduplicate_tag_ids(tag_ids: &mut Vec<TagId>) {
    let mut deduplicated = Vec::with_capacity(tag_ids.len());
    for tag_id in tag_ids.iter().copied() {
        if !deduplicated.contains(&tag_id) {
            deduplicated.push(tag_id);
        }
    }
    *tag_ids = deduplicated;
}
