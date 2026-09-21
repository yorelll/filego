//! Presentation boundary between application state and Slint callbacks.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiCommand {
    Show,
    Hide,
    Toggle,
    Exit,
}
