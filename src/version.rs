pub const PRODUCT_NAME: &str = "QuickFolder";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn display() -> String {
    format!("{PRODUCT_NAME} {VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_uses_cargo_package_version() {
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
        assert_eq!(display(), "QuickFolder 0.0.1");
    }
}
