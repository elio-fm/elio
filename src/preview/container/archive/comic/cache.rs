use super::super::common::system_time_key;
use super::backends::parse_comic_archive;
use super::types::{CachedComicArchive, ComicArchiveCache, ComicArchiveCacheKey};
use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};

const COMIC_ARCHIVE_CACHE_LIMIT: usize = 16;
static COMIC_ARCHIVE_CACHE: OnceLock<Mutex<ComicArchiveCache>> = OnceLock::new();

pub(super) fn load_comic_archive<F>(path: &Path, canceled: &F) -> Option<Arc<CachedComicArchive>>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }

    let key = comic_archive_cache_key(path)?;
    if let Some(cached) = comic_archive_cache()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .archives
        .get(&key)
        .cloned()
    {
        return Some(cached);
    }

    let parsed = Arc::new(parse_comic_archive(path, canceled)?);
    if canceled() {
        return None;
    }
    let mut cache = comic_archive_cache()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if let Some(existing) = cache.archives.get(&key).cloned() {
        return Some(existing);
    }
    cache.order.retain(|cached_key| cached_key != &key);
    cache.order.push_back(key.clone());
    cache.archives.insert(key.clone(), Arc::clone(&parsed));
    while cache.order.len() > COMIC_ARCHIVE_CACHE_LIMIT {
        if let Some(stale_key) = cache.order.pop_front() {
            cache.archives.remove(&stale_key);
        }
    }
    Some(parsed)
}

fn comic_archive_cache() -> &'static Mutex<ComicArchiveCache> {
    COMIC_ARCHIVE_CACHE.get_or_init(|| Mutex::new(ComicArchiveCache::default()))
}

fn comic_archive_cache_key(path: &Path) -> Option<ComicArchiveCacheKey> {
    let metadata = fs::metadata(path).ok()?;
    Some(ComicArchiveCacheKey {
        path: path.to_path_buf(),
        size: metadata.len(),
        modified: metadata.modified().ok().and_then(system_time_key),
    })
}
