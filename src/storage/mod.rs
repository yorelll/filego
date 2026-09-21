//! Storage boundary.
//!
//! M01 will implement versioned JSON loading and atomic, recoverable writes.

pub trait SettingsRepository {
    type Error;

    fn flush(&mut self) -> Result<(), Self::Error>;
}
