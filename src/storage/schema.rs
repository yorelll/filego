use serde::{Deserialize, Serialize};

use crate::domain::document::AppData;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredDocumentV1 {
    pub schema_version: u32,
    #[serde(flatten)]
    pub data: AppData,
}

impl StoredDocumentV1 {
    pub const fn new(data: AppData) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            data,
        }
    }
}
