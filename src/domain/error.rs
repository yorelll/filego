use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationErrorKind {
    EmptyDisplayName,
    DisplayNameTooLong,
    EmptyPath,
    PathTooLong,
    EmptyAlias,
    AliasTooLong,
    TooManyAliases,
    ManualWeightOutOfRange,
    TooManyFavorites,
    NoteTooLong,
    CategoryNameTooLong,
    TagNameTooLong,
    DuplicateFolderId,
    DuplicateCategoryId,
    DuplicateTagId,
    UnknownCategoryReference,
    UnknownTagReference,
    InvalidRevision,
    RevisionOverflow,
    InvalidSetting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    kind: ValidationErrorKind,
}

impl ValidationError {
    pub const fn new(kind: ValidationErrorKind) -> Self {
        Self { kind }
    }

    pub const fn kind(&self) -> ValidationErrorKind {
        self.kind
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.kind {
            ValidationErrorKind::EmptyDisplayName => "a required display name is missing",
            ValidationErrorKind::DisplayNameTooLong => "a display name exceeds the allowed length",
            ValidationErrorKind::EmptyPath => "a required path is missing",
            ValidationErrorKind::PathTooLong => "a stored path exceeds the allowed length",
            ValidationErrorKind::EmptyAlias => "an alias is empty",
            ValidationErrorKind::AliasTooLong => "an alias exceeds the allowed length",
            ValidationErrorKind::TooManyAliases => "too many aliases are stored for one folder",
            ValidationErrorKind::ManualWeightOutOfRange => {
                "a manual ranking weight is outside its allowed range"
            }
            ValidationErrorKind::TooManyFavorites => "too many folders are marked as favorites",
            ValidationErrorKind::NoteTooLong => "a note exceeds the allowed length",
            ValidationErrorKind::CategoryNameTooLong => {
                "a category name exceeds the allowed length"
            }
            ValidationErrorKind::TagNameTooLong => "a tag name exceeds the allowed length",
            ValidationErrorKind::DuplicateFolderId => "a folder identifier is duplicated",
            ValidationErrorKind::DuplicateCategoryId => "a category identifier is duplicated",
            ValidationErrorKind::DuplicateTagId => "a tag identifier is duplicated",
            ValidationErrorKind::UnknownCategoryReference => {
                "a folder references an unknown category"
            }
            ValidationErrorKind::UnknownTagReference => "a folder references an unknown tag",
            ValidationErrorKind::InvalidRevision => "the document revision is invalid",
            ValidationErrorKind::RevisionOverflow => "the document revision cannot be incremented",
            ValidationErrorKind::InvalidSetting => "a setting is outside its allowed range",
        };

        formatter.write_str(message)
    }
}

impl std::error::Error for ValidationError {}
