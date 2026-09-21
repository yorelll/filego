pub const PRODUCT_NAME: &str = "FileGo";
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
        assert_eq!(display(), "FileGo 0.0.1");
    }
}
