#[cfg(target_os = "windows")]
pub mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformCommand {
    ShowWindow,
    HideWindow,
    ToggleWindow,
    Exit,
}

pub trait PlatformEventSource {
    type Error;

    fn initialize(&mut self) -> Result<(), Self::Error>;
}
