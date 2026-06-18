use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ComicArchiveBackend {
    Zip,
    SevenZip,
    Unrar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ComicArchiveSignature {
    Zip,
    Rar,
    SevenZip,
    Unknown,
}

#[derive(Clone, Debug)]
pub(super) struct ComicArchivePage {
    pub(super) entry_name: String,
    pub(super) sort_key: String,
    pub(super) extension: String,
}

#[derive(Clone, Debug)]
pub(super) struct CachedComicArchive {
    pub(super) backend: ComicArchiveBackend,
    pub(super) page_entries: Vec<ComicArchivePage>,
    pub(super) comic_info: Option<ComicInfoMetadata>,
    pub(super) derived_info: Option<ComicDerivedMetadata>,
}

#[derive(Clone, Debug)]
pub(super) struct ComicArchiveListing {
    pub(super) backend: ComicArchiveBackend,
    pub(super) page_entries: Vec<ComicArchivePage>,
    pub(super) metadata_entry: Option<ComicMetadataEntry>,
    pub(super) archive_comment: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub(super) struct ComicMetadataEntry {
    pub(super) name: String,
    pub(super) kind: ComicMetadataFileKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ComicMetadataFileKind {
    ComicInfo,
    MetronInfo,
    CoMet,
    Acbf,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ComicInfoMetadata {
    pub(super) title: Option<String>,
    pub(super) series: Option<String>,
    pub(super) number: Option<String>,
    pub(super) volume: Option<String>,
    pub(super) year: Option<String>,
    pub(super) publisher: Option<String>,
    pub(super) writer: Option<String>,
    pub(super) penciller: Option<String>,
    pub(super) genre: Option<String>,
}

impl ComicInfoMetadata {
    pub(super) fn has_visible_fields(&self) -> bool {
        self.title.is_some()
            || self.series.is_some()
            || self.number.is_some()
            || self.volume.is_some()
            || self.year.is_some()
            || self.publisher.is_some()
            || self.writer.is_some()
            || self.penciller.is_some()
            || self.genre.is_some()
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct ComicDerivedMetadata {
    pub(super) series: Option<String>,
    pub(super) volume: Option<String>,
    pub(super) number: Option<String>,
    pub(super) year: Option<String>,
    pub(super) publisher: Option<String>,
    pub(super) source: Option<String>,
    pub(super) chapters: Option<String>,
}

impl ComicDerivedMetadata {
    pub(super) fn has_visible_fields(&self) -> bool {
        self.series.is_some()
            && (self.volume.is_some()
                || self.number.is_some()
                || self.year.is_some()
                || self.publisher.is_some()
                || self.source.is_some()
                || self.chapters.is_some())
    }
}

#[derive(Debug, Default)]
pub(super) struct ComicArchiveCache {
    pub(super) archives:
        std::collections::HashMap<ComicArchiveCacheKey, std::sync::Arc<CachedComicArchive>>,
    pub(super) order: std::collections::VecDeque<ComicArchiveCacheKey>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct ComicArchiveCacheKey {
    pub(super) path: PathBuf,
    pub(super) size: u64,
    pub(super) modified: Option<(u64, u32)>,
}
