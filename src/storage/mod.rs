//! Versioned document codecs and future storage boundary.
//!
//! M01-A keeps this module free of filesystem side effects. M01-B will add
//! atomic, recoverable persistence behind a separate repository boundary.

pub mod codec;
pub mod io;
pub mod location;
pub mod repository;
pub mod schema;

#[cfg(test)]
mod repository_tests;
#[cfg(test)]
mod tests;

pub trait SettingsRepository {
    type Error;

    fn flush(&mut self) -> Result<(), Self::Error>;
}
