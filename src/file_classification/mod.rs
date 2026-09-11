mod archive_detection;
pub(crate) mod code_languages;
mod extensions;
mod file_inspection;
mod license_detection;
mod names;
mod types;

#[cfg(test)]
mod tests;

pub(crate) use self::archive_detection::inspect_compound_archive_name;
pub(crate) use self::file_inspection::{
    inspect_entry_cached, inspect_entry_fast, inspect_path, inspect_path_cached,
};
pub(crate) use self::license_detection::is_canonical_license_file_name;
pub(crate) use self::types::{
    CodeBackend, CompoundArchiveKind, CompressionKind, CustomCodeKind, DiskImageKind,
    DocumentFormat, FileClass, FileFacts, PreviewKind, PreviewSpec, StructuredFormat,
};
