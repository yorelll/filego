//! Platform-independent FileGo business rules.

pub mod document;
pub mod error;
pub mod folder;
pub mod ids;
pub mod path_semantics;
pub mod settings;

pub const PRODUCT_SCOPE: &str = "user-maintained folder shortcuts";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_does_not_claim_file_indexing() {
        assert!(!PRODUCT_SCOPE.contains("index"));
    }
}
