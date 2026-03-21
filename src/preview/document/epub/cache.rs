use super::super::{common::read_zip_entry, metadata::DocumentMetadata};
use super::{
    EPUB_PACKAGE_CACHE_LIMIT,
    assets::{EpubAssetDescriptor, build_epub_asset_descriptor},
    parse::{parse_epub_package_document, parse_epub_rootfile_path, resolve_epub_cover_item},
    system_time_key,
    toc::{EpubSection, build_epub_sections},
};
use std::{
    collections::{HashMap, VecDeque},
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};
use zip::ZipArchive;

#[derive(Clone, Debug)]
pub(super) struct CachedEpubPackage {
    pub(super) metadata: DocumentMetadata,
    pub(super) sections: Vec<EpubSection>,
    pub(super) cover_asset: Option<EpubAssetDescriptor>,
}

#[derive(Debug, Default)]
struct EpubPackageCache {
    packages: HashMap<EpubPackageCacheKey, Arc<CachedEpubPackage>>,
    order: VecDeque<EpubPackageCacheKey>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct EpubPackageCacheKey {
    path: PathBuf,
    size: u64,
    modified: Option<(u64, u32)>,
}

pub(super) fn load_epub_package<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    path: &Path,
) -> Option<Arc<CachedEpubPackage>> {
    let cache_key = epub_package_cache_key(path);
    if let Some(cache_key) = cache_key.as_ref()
        && let Some(cached) = cached_epub_package(cache_key)
    {
        return Some(cached);
    }

    let container_xml = read_zip_entry(archive, "META-INF/container.xml")?;
    let package_path = parse_epub_rootfile_path(&container_xml)?;
    let package_xml = read_zip_entry(archive, &package_path)?;
    #[cfg(test)]
    record_epub_package_parse(path);
    let package = parse_epub_package_document(&package_xml);
    let sections = build_epub_sections(archive, &package, &package_path);
    let cover_asset = resolve_epub_cover_item(&package)
        .and_then(|item| build_epub_asset_descriptor(&package_path, item));
    let cached = Arc::new(CachedEpubPackage {
        metadata: package.metadata,
        sections,
        cover_asset,
    });
    if let Some(cache_key) = cache_key {
        cache_epub_package(cache_key, Arc::clone(&cached));
    }
    Some(cached)
}

fn epub_package_cache() -> &'static Mutex<EpubPackageCache> {
    static CACHE: OnceLock<Mutex<EpubPackageCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(EpubPackageCache::default()))
}

fn epub_package_cache_key(path: &Path) -> Option<EpubPackageCacheKey> {
    let metadata = std::fs::metadata(path).ok()?;
    Some(EpubPackageCacheKey {
        path: path.to_path_buf(),
        size: metadata.len(),
        modified: metadata.modified().ok().and_then(system_time_key),
    })
}

fn cached_epub_package(key: &EpubPackageCacheKey) -> Option<Arc<CachedEpubPackage>> {
    let mut cache = epub_package_cache()
        .lock()
        .expect("epub package cache lock");
    let package = cache.packages.get(key).cloned();
    if package.is_some() {
        cache.order.retain(|cached| cached != key);
        cache.order.push_back(key.clone());
    }
    package
}

fn cache_epub_package(key: EpubPackageCacheKey, package: Arc<CachedEpubPackage>) {
    let mut cache = epub_package_cache()
        .lock()
        .expect("epub package cache lock");
    cache.packages.insert(key.clone(), package);
    cache.order.retain(|cached| cached != &key);
    cache.order.push_back(key);
    while cache.order.len() > EPUB_PACKAGE_CACHE_LIMIT {
        if let Some(stale_key) = cache.order.pop_front() {
            cache.packages.remove(&stale_key);
        }
    }
}

#[cfg(test)]
fn epub_package_parse_counts() -> &'static Mutex<HashMap<PathBuf, usize>> {
    static COUNTS: OnceLock<Mutex<HashMap<PathBuf, usize>>> = OnceLock::new();
    COUNTS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
fn record_epub_package_parse(path: &Path) {
    let mut counts = epub_package_parse_counts()
        .lock()
        .expect("epub package parse count lock");
    *counts.entry(path.to_path_buf()).or_insert(0) += 1;
}

#[cfg(test)]
pub(super) fn reset_epub_package_parse_count(path: &Path) {
    epub_package_parse_counts()
        .lock()
        .expect("epub package parse count lock")
        .remove(path);
}

#[cfg(test)]
pub(super) fn epub_package_parse_count(path: &Path) -> usize {
    epub_package_parse_counts()
        .lock()
        .expect("epub package parse count lock")
        .get(path)
        .copied()
        .unwrap_or(0)
}

#[cfg(test)]
pub(super) fn clear_epub_package_cache() {
    let mut cache = epub_package_cache()
        .lock()
        .expect("epub package cache lock");
    cache.packages.clear();
    cache.order.clear();
}
