use serde::{Deserialize, Serialize};

use super::{
    error::{ValidationError, ValidationErrorKind},
    folder::{Category, FolderEntry, MAX_FAVORITES, Tag},
    ids::{CategoryId, TagId},
    settings::AppSettings,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppData {
    pub settings: AppSettings,
    pub folders: Vec<FolderEntry>,
    pub categories: Vec<Category>,
    pub tags: Vec<Tag>,
    pub revision: u64,
}

impl AppData {
    pub fn validate_and_normalize(&mut self) -> Result<(), ValidationError> {
        if self.revision == 0 {
            return Err(ValidationError::new(ValidationErrorKind::InvalidRevision));
        }

        self.settings.validate()?;

        let mut category_ids = Vec::with_capacity(self.categories.len());
        for category in &self.categories {
            category.validate()?;
            if category_ids.contains(&category.id) {
                return Err(ValidationError::new(
                    ValidationErrorKind::DuplicateCategoryId,
                ));
            }
            category_ids.push(category.id);
        }

        let mut tag_ids = Vec::with_capacity(self.tags.len());
        for tag in &self.tags {
            tag.validate()?;
            if tag_ids.contains(&tag.id) {
                return Err(ValidationError::new(ValidationErrorKind::DuplicateTagId));
            }
            tag_ids.push(tag.id);
        }

        let mut folder_ids = Vec::with_capacity(self.folders.len());
        for folder in &mut self.folders {
            if folder_ids.contains(&folder.id) {
                return Err(ValidationError::new(ValidationErrorKind::DuplicateFolderId));
            }
            folder_ids.push(folder.id);
            folder.validate_and_normalize()?;

            if folder
                .category_id
                .is_some_and(|category_id| !category_ids.contains(&category_id))
            {
                return Err(ValidationError::new(
                    ValidationErrorKind::UnknownCategoryReference,
                ));
            }

            if folder
                .tag_ids
                .iter()
                .any(|tag_id| !tag_ids.contains(tag_id))
            {
                return Err(ValidationError::new(
                    ValidationErrorKind::UnknownTagReference,
                ));
            }
        }

        if self.folders.iter().filter(|folder| folder.favorite).count() > MAX_FAVORITES {
            return Err(ValidationError::new(ValidationErrorKind::TooManyFavorites));
        }

        Ok(())
    }

    pub fn next_revision(&self) -> Result<u64, ValidationError> {
        self.revision
            .checked_add(1)
            .ok_or_else(|| ValidationError::new(ValidationErrorKind::RevisionOverflow))
    }

    pub fn remove_category(&mut self, category_id: CategoryId) -> bool {
        let initial_count = self.categories.len();
        self.categories
            .retain(|category| category.id != category_id);

        for folder in &mut self.folders {
            if folder.category_id == Some(category_id) {
                folder.category_id = None;
            }
        }

        self.categories.len() != initial_count
    }

    pub fn remove_tag(&mut self, tag_id: TagId) -> bool {
        let initial_count = self.tags.len();
        self.tags.retain(|tag| tag.id != tag_id);

        for folder in &mut self.folders {
            folder
                .tag_ids
                .retain(|folder_tag_id| *folder_tag_id != tag_id);
        }

        self.tags.len() != initial_count
    }
}
