use super::super::common::{
    read_zip_entry_bytes_limited, resolve_zip_entry_path, strip_fragment_identifier,
};
use super::parse::EpubManifestItem;
use super::{EPUB_ASSET_CACHE_VERSION, system_time_key};
use std::{
    collections::hash_map::DefaultHasher,
    env,
    fs::{self, File},
    hash::{Hash, Hasher},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};
use zip::ZipArchive;

#[derive(Clone, Debug)]
pub(super) struct EpubAssetDescriptor {
    pub(super) zip_path: String,
    pub(super) extension: String,
}

#[derive(Clone)]
pub(super) struct ExtractedEpubAsset {
    pub(super) path: PathBuf,
    pub(super) size: u64,
    pub(super) modified: Option<SystemTime>,
}

pub(super) fn build_epub_asset_descriptor(
    package_path: &str,
    item: &EpubManifestItem,
) -> Option<EpubAssetDescriptor> {
    Some(EpubAssetDescriptor {
        zip_path: resolve_zip_entry_path(package_path, &item.href),
        extension: epub_cover_extension(item)?.to_string(),
    })
}

pub(super) fn extract_epub_asset<R: Read + std::io::Seek>(
    source_path: &Path,
    archive: &mut ZipArchive<R>,
    asset_path: &str,
    limit_bytes: usize,
) -> Option<ExtractedEpubAsset> {
    let extension = epub_asset_extension(asset_path)?;
    let descriptor = EpubAssetDescriptor {
        zip_path: asset_path.to_string(),
        extension: extension.to_string(),
    };
    extract_epub_asset_descriptor(source_path, archive, &descriptor, limit_bytes)
}

pub(super) fn extract_epub_asset_descriptor<R: Read + std::io::Seek>(
    source_path: &Path,
    archive: &mut ZipArchive<R>,
    asset: &EpubAssetDescriptor,
    limit_bytes: usize,
) -> Option<ExtractedEpubAsset> {
    let cache_path = epub_asset_cache_path(source_path, &asset.zip_path, &asset.extension)?;
    if cache_path.exists() {
        return extracted_epub_asset_from_path(cache_path);
    }

    let bytes = read_zip_entry_bytes_limited(archive, &asset.zip_path, limit_bytes)?;
    write_bytes_atomically(&cache_path, &bytes)?;
    extracted_epub_asset_from_path(cache_path)
}

fn extracted_epub_asset_from_path(path: PathBuf) -> Option<ExtractedEpubAsset> {
    let metadata = fs::metadata(&path).ok()?;
    Some(ExtractedEpubAsset {
        path,
        size: metadata.len(),
        modified: metadata.modified().ok(),
    })
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

fn epub_cover_extension(item: &EpubManifestItem) -> Option<&'static str> {
    match item.media_type.as_deref() {
        Some("image/png") => Some("png"),
        Some("image/jpeg") => Some("jpg"),
        Some("image/gif") => Some("gif"),
        Some("image/webp") => Some("webp"),
        Some("image/svg+xml") => Some("svg"),
        _ => {
            let href = strip_fragment_identifier(&item.href).to_ascii_lowercase();
            if href.ends_with(".png") {
                Some("png")
            } else if href.ends_with(".jpg") || href.ends_with(".jpeg") {
                Some("jpg")
            } else if href.ends_with(".gif") {
                Some("gif")
            } else if href.ends_with(".webp") {
                Some("webp")
            } else if href.ends_with(".svg") {
                Some("svg")
            } else {
                None
            }
        }
    }
}

fn epub_asset_extension(asset_path: &str) -> Option<&str> {
    Path::new(strip_fragment_identifier(asset_path))
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            if extension.eq_ignore_ascii_case("jpeg") {
                "jpg"
            } else {
                extension
            }
        })
}

fn epub_asset_cache_path(source_path: &Path, asset_path: &str, extension: &str) -> Option<PathBuf> {
    let metadata = fs::metadata(source_path).ok();
    let modified = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(system_time_key);
    let mut hasher = DefaultHasher::new();
    EPUB_ASSET_CACHE_VERSION.hash(&mut hasher);
    source_path.hash(&mut hasher);
    asset_path.hash(&mut hasher);
    metadata
        .as_ref()
        .map(|metadata| metadata.len())
        .hash(&mut hasher);
    modified.hash(&mut hasher);
    let cache_dir = env::temp_dir().join(format!("elio-epub-asset-v{EPUB_ASSET_CACHE_VERSION}"));
    fs::create_dir_all(&cache_dir).ok()?;
    Some(cache_dir.join(format!("{:016x}.{extension}", hasher.finish())))
}
