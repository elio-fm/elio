mod archives;
mod classify;
mod extensions;
mod license;
mod names;
mod types;

#[cfg(test)]
mod tests;

pub(crate) use self::archives::inspect_compound_archive_name;
pub(crate) use self::classify::{inspect_path, inspect_path_cached};
pub(crate) use self::types::{
    CompoundArchiveKind, CompressionKind, DiskImageKind, DocumentFormat, FileFacts,
    HighlightLanguage, PreviewKind, PreviewSpec, StructuredFormat,
};
