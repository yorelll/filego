//! Platform-independent QuickFolder business rules.
//!
//! Folder records, settings, and search models are introduced in M01 and M02.

pub const PRODUCT_SCOPE: &str = "user-maintained folder shortcuts";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_does_not_claim_file_indexing() {
        assert!(!PRODUCT_SCOPE.contains("index"));
    }
}
