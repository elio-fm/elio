mod archive_formats;
mod archive_preview;
mod archive_reading;
pub(in crate::preview) mod archive_rendering;
mod comics;
mod external_tools;
mod iso;
mod zip_manifest;

pub(super) use self::archive_preview::build_archive_preview;
pub(super) use self::iso::build_iso_preview;

#[cfg(test)]
pub(super) use self::archive_rendering::normalize_archive_entries;

#[cfg(test)]
pub(in crate::preview) use self::iso::{
    ISO_BOOT_SYSTEM_ID, ISO_DESCRIPTOR_START_SECTOR, ISO_SECTOR_SIZE, IsoMetadata,
};
#[cfg(test)]
pub(super) use self::iso::{parse_iso_metadata, render_iso_preview};
