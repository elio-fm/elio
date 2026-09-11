mod archive_cache;
mod archive_reading;
mod comic_preview;
mod embedded_metadata;
mod filename_metadata;
mod page_extraction;

pub(super) use self::comic_preview::build_comic_archive_preview;

#[cfg(test)]
use self::archive_reading::{
    ComicArchiveBackend, ComicArchiveSignature, parse_comic_archive_from_7z_output,
    parse_unrar_archive_comment, parse_zip_comic_archive, sniff_comic_archive_signature,
};
#[cfg(test)]
use self::embedded_metadata::parse_comic_book_info_comment;

#[cfg(test)]
#[path = "tests/archive_reading.rs"]
mod tests;
