use crate::{
    file_classification::{self, FileClass},
    filesystem::{Entry, EntryKind},
    terminal_images::{ImageProtocol, TerminalWindowSize},
};
use std::{fs, path::PathBuf, time::SystemTime};

#[derive(Default)]
pub(crate) struct PreviewMetadata {
    pub(crate) document_author: Option<String>,
    pub(crate) document_page_count: Option<String>,
    pub(crate) comic_writer: Option<String>,
    pub(crate) comic_penciller: Option<String>,
    pub(crate) audio_artist: Option<String>,
    pub(crate) audio_album: Option<String>,
    pub(crate) media_duration: Option<String>,
    pub(crate) media_dimensions: Option<String>,
    pub(crate) media_codec: Option<String>,
    pub(crate) media_frame_rate: Option<String>,
    pub(crate) archive_entry_count: Option<usize>,
    pub(crate) archive_unpacked_size: Option<u64>,
    pub(crate) archive_password_protected: Option<bool>,
}

pub(crate) struct ItemDetails {
    pub(crate) entry: Entry,
    pub(crate) kind_label: String,
    pub(crate) language: Option<&'static str>,
    pub(crate) path: PathBuf,
    pub(crate) icon: &'static str,
    pub(crate) icon_png_path: Option<PathBuf>,
    pub(crate) created: Option<SystemTime>,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) original_location: Option<PathBuf>,
    pub(crate) trashed_at: Option<SystemTime>,
    pub(crate) image_dimensions: Option<(u32, u32)>,
    pub(crate) document_author: Option<String>,
    pub(crate) document_page_count: Option<String>,
    pub(crate) comic_writer: Option<String>,
    pub(crate) comic_penciller: Option<String>,
    pub(crate) audio_artist: Option<String>,
    pub(crate) audio_album: Option<String>,
    pub(crate) media_duration: Option<String>,
    pub(crate) media_dimensions: Option<String>,
    pub(crate) media_codec: Option<String>,
    pub(crate) media_frame_rate: Option<String>,
    pub(crate) archive_entry_count: Option<usize>,
    pub(crate) archive_unpacked_size: Option<u64>,
    pub(crate) archive_password_protected: Option<bool>,
}

impl ItemDetails {
    pub(crate) fn from_entry(
        entry: &Entry,
        protocol: ImageProtocol,
        window_size: Option<TerminalWindowSize>,
        preview: PreviewMetadata,
    ) -> Self {
        let facts = file_classification::inspect_entry_fast(entry);
        let appearance = crate::theme::resolve_entry(entry);
        let panel_color = crate::theme::palette().panel;
        let icon_png_path = match protocol {
            ImageProtocol::None => None,
            ImageProtocol::KittyGraphics => super::icon_renderer::render_icon_png(
                appearance.icon,
                appearance.color,
                panel_color,
            ),
            _ => window_size.and_then(|window| {
                let cell_width = window.pixels_width / u32::from(window.cells_width.max(1));
                let cell_height = window.pixels_height / u32::from(window.cells_height.max(1));
                super::icon_renderer::render_icon_png_for_cell_area(
                    appearance.icon,
                    appearance.color,
                    panel_color,
                    cell_width.saturating_mul(4),
                    cell_height.saturating_mul(3),
                )
            }),
        };
        let kind_label = entry
            .symlink
            .as_ref()
            .map(|link| match link.target_kind {
                Some(EntryKind::Directory) => "Symbolic link to folder",
                Some(EntryKind::File) => "Symbolic link to file",
                None => "Broken symbolic link",
            })
            .or(facts.specific_type_label)
            .unwrap_or_else(|| default_kind_label(facts.builtin_class))
            .to_owned();
        let file_metadata = fs::symlink_metadata(&entry.path).ok();
        let created = file_metadata
            .as_ref()
            .and_then(|metadata| metadata.created().ok());
        let modified = file_metadata
            .as_ref()
            .and_then(|metadata| metadata.modified().ok())
            .or(entry.modified);
        let trash_metadata =
            crate::filesystem::trash_item_metadata(&entry.path).unwrap_or_default();
        let image_dimensions = (facts.builtin_class == FileClass::Image)
            .then(|| {
                image::ImageReader::open(&entry.path)
                    .ok()?
                    .with_guessed_format()
                    .ok()?
                    .into_dimensions()
                    .ok()
            })
            .flatten();

        Self {
            entry: entry.clone(),
            kind_label,
            language: facts
                .specific_type_label
                .is_none()
                .then(|| {
                    facts.preview.language_hint.map(|hint| {
                        file_classification::code_languages::display_label_for_code_syntax(hint)
                            .unwrap_or(hint)
                    })
                })
                .flatten(),
            path: entry
                .path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or(&entry.path)
                .to_path_buf(),
            icon: appearance.icon,
            icon_png_path,
            created,
            modified,
            original_location: trash_metadata.original_path,
            trashed_at: trash_metadata.trashed_at,
            image_dimensions,
            document_author: preview.document_author,
            document_page_count: preview.document_page_count,
            comic_writer: preview.comic_writer,
            comic_penciller: preview.comic_penciller,
            audio_artist: preview.audio_artist,
            audio_album: preview.audio_album,
            media_duration: preview.media_duration,
            media_dimensions: preview.media_dimensions,
            media_codec: preview.media_codec,
            media_frame_rate: preview.media_frame_rate,
            archive_entry_count: preview.archive_entry_count,
            archive_unpacked_size: preview.archive_unpacked_size,
            archive_password_protected: preview.archive_password_protected,
        }
    }

    pub(crate) fn is_directory(&self) -> bool {
        self.entry.kind == EntryKind::Directory
    }
}

fn default_kind_label(class: FileClass) -> &'static str {
    match class {
        FileClass::Directory | FileClass::SymlinkDirectory => "Folder",
        FileClass::Code => "Code",
        FileClass::Config => "Configuration file",
        FileClass::Document => "Text file",
        FileClass::Image => "Image",
        FileClass::Audio => "Audio file",
        FileClass::Video => "Video",
        FileClass::Archive => "Archive",
        FileClass::Font => "Font file",
        FileClass::License => "License",
        FileClass::BrokenSymlink => "Broken symbolic link",
        FileClass::Data => "Data file",
        FileClass::File => "File",
    }
}

impl Drop for ItemDetails {
    fn drop(&mut self) {
        if let Some(path) = &self.icon_png_path {
            let _ = fs::remove_file(path);
        }
    }
}
