//! Platform-neutral access to the operating system clipboard.
//!
//! This facade defines the values consumed by the application and delegates
//! runtime backend selection to a private dispatcher. Platform protocols and
//! Freedesktop file-clipboard formats remain implementation details.

use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FileClipboardOperation {
    Copy,
    #[cfg(any(
        windows,
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    Cut,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileClipboard {
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) operation: FileClipboardOperation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClipboardError {
    Unavailable,
    WriteFailed,
}

impl ClipboardError {
    pub(crate) fn item_status(self) -> &'static str {
        match self {
            Self::Unavailable | Self::WriteFailed => "File clipboard unavailable",
        }
    }
    pub(crate) fn text_status(self) -> &'static str {
        match self {
            Self::Unavailable => "Clipboard unavailable",
            Self::WriteFailed => "Clipboard write failed",
        }
    }
}

mod dispatcher;
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
mod freedesktop;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
mod wayland;
#[cfg(windows)]
mod windows;
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
mod x11;

pub(crate) fn read_files() -> Option<FileClipboard> {
    dispatcher::read_files()
}
pub(crate) fn read_text() -> Option<String> {
    dispatcher::read_text()
}
pub(crate) fn write_files(paths: &[PathBuf]) -> Result<(), ClipboardError> {
    dispatcher::write_files(paths)
}
pub(crate) fn write_text(text: &str) -> Result<(), ClipboardError> {
    dispatcher::write_text(text)
}

#[cfg(test)]
mod tests;
