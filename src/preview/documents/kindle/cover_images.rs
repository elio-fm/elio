use super::kindle_metadata::{KindleDatabase, KindleHeader, read_record_bytes};
use crate::preview::{PreviewVisual, PreviewVisualKind, PreviewVisualLayout};
use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    fs::File,
    hash::{Hash, Hasher},
    io::Write,
    path::{Path, PathBuf},
    time::SystemTime,
};

const MAX_KINDLE_COVER_RECORD_BYTES: usize = 8 * 1024 * 1024;
const KINDLE_COVER_CACHE_VERSION: usize = 1;
const EXTH_COVER_OFFSET: u32 = 201;
const EXTH_THUMB_OFFSET: u32 = 202;

#[derive(Clone, Copy, Debug)]
enum KindleCoverFormat {
    Gif,
    Jpeg,
    Png,
    Webp,
}

impl KindleCoverFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Gif => "gif",
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
        }
    }
}

pub(super) fn extract_kindle_cover_visual(
    path: &Path,
    file: &mut File,
    database: &KindleDatabase,
    header: &KindleHeader,
) -> Option<PreviewVisual> {
    if header.encrypted {
        return None;
    }

    let first_image_record_index = usize::try_from(header.first_image_record_index?).ok()?;
    for relative_cover_index in kindle_cover_record_offsets(header) {
        let Some(cover_record_index) = usize::try_from(relative_cover_index)
            .ok()
            .and_then(|index| first_image_record_index.checked_add(index))
        else {
            continue;
        };
        if cover_record_index >= database.record_offsets.len() {
            continue;
        }
        if let Some(visual) =
            extract_kindle_cover_record_visual(path, file, database, cover_record_index)
        {
            return Some(visual);
        }
    }

    None
}

fn extract_kindle_cover_record_visual(
    path: &Path,
    file: &mut File,
    database: &KindleDatabase,
    cover_record_index: usize,
) -> Option<PreviewVisual> {
    let cache_path =
        if let Some(cache_path) = cached_kindle_cover_path(path, database, cover_record_index) {
            cache_path
        } else {
            let bytes = read_record_bytes(
                file,
                database,
                cover_record_index,
                MAX_KINDLE_COVER_RECORD_BYTES,
                false,
            )?;
            let format = sniff_kindle_cover_format(&bytes)?;
            let cache_path =
                kindle_cover_cache_path(path, database, cover_record_index, format.extension())?;
            if !cache_path.exists() {
                write_bytes_atomically(&cache_path, &bytes)?;
            }
            cache_path
        };

    kindle_cover_visual_from_path(cache_path)
}

fn cached_kindle_cover_path(
    path: &Path,
    database: &KindleDatabase,
    record_index: usize,
) -> Option<PathBuf> {
    for extension in ["jpg", "png", "gif", "webp"] {
        let cache_path = kindle_cover_cache_path(path, database, record_index, extension)?;
        if cache_path.exists() {
            return Some(cache_path);
        }
    }
    None
}

fn kindle_cover_record_offsets(header: &KindleHeader) -> Vec<u32> {
    let mut offsets = Vec::new();
    for kind in [EXTH_COVER_OFFSET, EXTH_THUMB_OFFSET] {
        if let Some(offset) = header
            .exth_records
            .get(&kind)
            .into_iter()
            .flat_map(|values| values.iter())
            .find_map(|value| exth_integer(value))
            && !offsets.contains(&offset)
        {
            offsets.push(offset);
        }
    }
    offsets
}

fn sniff_kindle_cover_format(bytes: &[u8]) -> Option<KindleCoverFormat> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some(KindleCoverFormat::Png);
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some(KindleCoverFormat::Jpeg);
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(KindleCoverFormat::Gif);
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some(KindleCoverFormat::Webp);
    }
    None
}

fn kindle_cover_visual_from_path(path: PathBuf) -> Option<PreviewVisual> {
    let metadata = fs::metadata(&path).ok()?;
    Some(PreviewVisual {
        kind: PreviewVisualKind::Cover,
        layout: PreviewVisualLayout::LargeInline,
        path,
        size: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

fn kindle_cover_cache_path(
    path: &Path,
    database: &KindleDatabase,
    record_index: usize,
    extension: &str,
) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    KINDLE_COVER_CACHE_VERSION.hash(&mut hasher);
    path.hash(&mut hasher);
    database.file_len.hash(&mut hasher);
    database
        .modified
        .and_then(system_time_key)
        .hash(&mut hasher);
    record_index.hash(&mut hasher);
    let cache_dir = kindle_cover_cache_dir()?;
    Some(cache_dir.join(format!("cover-{:016x}.{extension}", hasher.finish())))
}

fn kindle_cover_cache_dir() -> Option<PathBuf> {
    let cache_dir =
        env::temp_dir().join(format!("elio-kindle-cover-v{KINDLE_COVER_CACHE_VERSION}"));
    fs::create_dir_all(&cache_dir).ok()?;
    Some(cache_dir)
}

fn write_bytes_atomically(path: &Path, bytes: &[u8]) -> Option<()> {
    let parent = path.parent()?;
    fs::create_dir_all(parent).ok()?;

    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temp_name = format!(
        ".{}.tmp-{}-{}",
        path.file_name()?.to_string_lossy(),
        std::process::id(),
        unique
    );
    let temp_path = parent.join(temp_name);

    let mut file = File::create(&temp_path).ok()?;
    file.write_all(bytes).ok()?;
    file.sync_all().ok()?;
    match fs::rename(&temp_path, path) {
        Ok(()) => Some(()),
        Err(_) if path.exists() => {
            let _ = fs::remove_file(&temp_path);
            Some(())
        }
        Err(_) => {
            let _ = fs::remove_file(&temp_path);
            None
        }
    }
}

fn system_time_key(time: SystemTime) -> Option<(u64, u32)> {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|duration| (duration.as_secs(), duration.subsec_nanos()))
}

fn exth_integer(bytes: &[u8]) -> Option<u32> {
    match bytes.len() {
        1 => Some(bytes[0] as u32),
        2 => Some(u16::from_be_bytes([bytes[0], bytes[1]]) as u32),
        4 => Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])),
        _ => None,
    }
}
