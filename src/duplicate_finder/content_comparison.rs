use super::{
    candidate_collection::{CandidateFile, SMALL_FILE_FULL_HASH_LIMIT, breathe_after_node},
    duplicate_scanning::{
        DuplicateBatchEmitter, DuplicateFile, DuplicateScanBatch, DuplicateScanStats,
    },
};
use anyhow::{Context, Result};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(not(unix))]
use std::path::PathBuf;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufReader, Read},
    path::Path,
    time::Duration,
};

const HASH_CHUNK_SIZE: usize = 1024 * 1024;
const PARTIAL_CHUNK_SIZE: usize = 64 * 1024;
const HASH_YIELD_CHUNKS: usize = 16;
const HASH_SLEEP_CHUNKS: usize = 128;
#[derive(Clone, Debug, Default)]
pub(crate) struct DuplicateHashCache {
    hashes: HashMap<DuplicateHashCacheKey, blake3::Hash>,
}

impl DuplicateHashCache {
    fn get(&self, key: &DuplicateHashCacheKey) -> Option<blake3::Hash> {
        self.hashes.get(key).copied()
    }

    fn insert(&mut self, key: DuplicateHashCacheKey, hash: blake3::Hash) {
        self.hashes.insert(key, hash);
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct DuplicateHashCacheKey {
    identity: DuplicateHashCacheIdentity,
    size: u64,
    modified: Option<std::time::SystemTime>,
    changed: Option<std::time::SystemTime>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum DuplicateHashCacheIdentity {
    #[cfg(unix)]
    Unix { dev: u64, ino: u64 },
    #[cfg(not(unix))]
    Path(PathBuf),
}

pub(super) fn duplicate_hash_cache_key(
    path: &Path,
    metadata: &fs::Metadata,
) -> Option<DuplicateHashCacheKey> {
    let identity = duplicate_hash_cache_identity(path, metadata);
    Some(DuplicateHashCacheKey {
        identity,
        size: metadata.len(),
        modified: metadata.modified().ok(),
        changed: metadata_changed_time(metadata),
    })
}

#[cfg(unix)]
fn duplicate_hash_cache_identity(
    _path: &Path,
    metadata: &fs::Metadata,
) -> DuplicateHashCacheIdentity {
    DuplicateHashCacheIdentity::Unix {
        dev: metadata.dev(),
        ino: metadata.ino(),
    }
}

#[cfg(not(unix))]
fn duplicate_hash_cache_identity(
    path: &Path,
    _metadata: &fs::Metadata,
) -> DuplicateHashCacheIdentity {
    DuplicateHashCacheIdentity::Path(path.to_path_buf())
}

#[cfg(unix)]
fn metadata_changed_time(metadata: &fs::Metadata) -> Option<std::time::SystemTime> {
    let secs = metadata.ctime();
    let nanos = metadata.ctime_nsec();
    if secs < 0 || nanos < 0 {
        return None;
    }
    Some(
        std::time::UNIX_EPOCH
            + Duration::from_secs(secs as u64)
            + Duration::from_nanos(nanos as u64),
    )
}

#[cfg(not(unix))]
fn metadata_changed_time(_metadata: &fs::Metadata) -> Option<std::time::SystemTime> {
    None
}

pub(super) fn verified_groups_for_same_size<F>(
    candidates: Vec<CandidateFile>,
    cache: &mut DuplicateHashCache,
    stats: &mut DuplicateScanStats,
    batch_emitter: &mut DuplicateBatchEmitter<'_, F>,
    is_canceled: &impl Fn() -> bool,
) -> Result<Vec<Vec<DuplicateFile>>>
where
    F: FnMut(DuplicateScanBatch) -> bool,
{
    let candidates = if candidates
        .first()
        .is_some_and(|candidate| candidate.size <= SMALL_FILE_FULL_HASH_LIMIT)
    {
        candidates
    } else {
        partial_duplicate_candidates(candidates, stats, is_canceled)
    };

    let mut by_hash: HashMap<blake3::Hash, Vec<CandidateFile>> = HashMap::new();
    for candidate in candidates {
        if is_canceled() {
            break;
        }
        match content_hash(&candidate, cache, stats, batch_emitter, is_canceled) {
            Ok(Some(hash)) => {
                stats.checked_candidates += 1;
                stats.hashed_files += 1;
                by_hash.entry(hash).or_default().push(candidate);
            }
            Ok(None) => break,
            Err(_) => {
                stats.checked_candidates += 1;
            }
        }
    }

    let mut groups = Vec::new();
    for same_hash in by_hash.into_values().filter(|files| files.len() > 1) {
        let mut representatives: Vec<Vec<CandidateFile>> = Vec::new();
        'candidate: for candidate in same_hash {
            if is_canceled() {
                break;
            }
            stats.verified_files += 1;
            for group in &mut representatives {
                if files_equal(&candidate.path, &group[0].path).unwrap_or(false) {
                    group.push(candidate);
                    continue 'candidate;
                }
            }
            representatives.push(vec![candidate]);
        }
        for group in representatives.into_iter().filter(|group| group.len() > 1) {
            let mut files = group
                .into_iter()
                .map(|file| DuplicateFile {
                    path: file.path,
                    name: file.name,
                    relative: file.relative,
                    size: file.size,
                    modified: file.modified,
                })
                .collect::<Vec<_>>();
            files.sort_by(|left, right| {
                crate::filesystem::natural_cmp(
                    &left.relative.to_lowercase(),
                    &right.relative.to_lowercase(),
                )
                .then_with(|| left.relative.cmp(&right.relative))
            });
            groups.push(files);
        }
    }
    Ok(groups)
}

fn partial_duplicate_candidates(
    candidates: Vec<CandidateFile>,
    stats: &mut DuplicateScanStats,
    is_canceled: &impl Fn() -> bool,
) -> Vec<CandidateFile> {
    let mut by_partial: HashMap<blake3::Hash, Vec<CandidateFile>> = HashMap::new();
    let mut processed = 0usize;
    for (index, candidate) in candidates.into_iter().enumerate() {
        if is_canceled() {
            break;
        }
        processed += 1;
        breathe_after_node(index + 1);
        if let Ok(fingerprint) = partial_content_fingerprint(&candidate.path, candidate.size) {
            by_partial.entry(fingerprint).or_default().push(candidate);
        }
    }
    let survivors = by_partial
        .into_values()
        .filter(|files| files.len() > 1)
        .flatten()
        .collect::<Vec<_>>();
    stats.checked_candidates = stats
        .checked_candidates
        .saturating_add(processed.saturating_sub(survivors.len()));
    survivors
}

fn partial_content_fingerprint(path: &Path, size: u64) -> Result<blake3::Hash> {
    let mut file =
        File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&size.to_le_bytes());

    let mut first = vec![0u8; PARTIAL_CHUNK_SIZE.min(size as usize)];
    let first_read = file.read(&mut first)?;
    hasher.update(&first[..first_read]);

    if size > PARTIAL_CHUNK_SIZE as u64 {
        use std::io::{Seek, SeekFrom};
        file.seek(SeekFrom::Start(
            size.saturating_sub(PARTIAL_CHUNK_SIZE as u64),
        ))?;
        let mut last = vec![0u8; PARTIAL_CHUNK_SIZE];
        let last_read = file.read(&mut last)?;
        hasher.update(&last[..last_read]);
    }

    Ok(hasher.finalize())
}

fn content_hash<F>(
    candidate: &CandidateFile,
    cache: &mut DuplicateHashCache,
    stats: &mut DuplicateScanStats,
    batch_emitter: &mut DuplicateBatchEmitter<'_, F>,
    is_canceled: &impl Fn() -> bool,
) -> Result<Option<blake3::Hash>>
where
    F: FnMut(DuplicateScanBatch) -> bool,
{
    if let Some(cache_key) = &candidate.cache_key
        && let Some(hash) = cache.get(cache_key)
    {
        stats.cached_hashes += 1;
        return Ok(Some(hash));
    }

    let hash = content_hash_uncached(&candidate.path, stats, batch_emitter, is_canceled)?;
    if let Some(hash) = hash
        && let Some(cache_key) = &candidate.cache_key
    {
        cache.insert(cache_key.clone(), hash);
    }
    Ok(hash)
}

fn content_hash_uncached<F>(
    path: &Path,
    stats: &mut DuplicateScanStats,
    batch_emitter: &mut DuplicateBatchEmitter<'_, F>,
    is_canceled: &impl Fn() -> bool,
) -> Result<Option<blake3::Hash>>
where
    F: FnMut(DuplicateScanBatch) -> bool,
{
    let mut reader = BufReader::new(
        File::open(path).with_context(|| format!("failed to open {}", path.display()))?,
    );
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; HASH_CHUNK_SIZE];
    let mut chunks = 0usize;
    loop {
        if is_canceled() {
            return Ok(None);
        }
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        stats.processed_bytes = stats.processed_bytes.saturating_add(read as u64);
        let _ = batch_emitter.emit_progress(*stats);
        chunks += 1;
        breathe_after_hash_chunk(chunks);
    }
    Ok(Some(hasher.finalize()))
}

fn files_equal(left: &Path, right: &Path) -> Result<bool> {
    let mut left = BufReader::new(File::open(left)?);
    let mut right = BufReader::new(File::open(right)?);
    let mut left_buf = vec![0u8; HASH_CHUNK_SIZE];
    let mut right_buf = vec![0u8; HASH_CHUNK_SIZE];
    let mut chunks = 0usize;
    loop {
        let left_read = left.read(&mut left_buf)?;
        let right_read = right.read(&mut right_buf)?;
        if left_read != right_read {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
        if left_buf[..left_read] != right_buf[..right_read] {
            return Ok(false);
        }
        chunks += 1;
        breathe_after_hash_chunk(chunks);
    }
}

fn breathe_after_hash_chunk(chunks: usize) {
    if chunks.is_multiple_of(HASH_SLEEP_CHUNKS) {
        std::thread::sleep(std::time::Duration::from_millis(1));
    } else if chunks.is_multiple_of(HASH_YIELD_CHUNKS) {
        std::thread::yield_now();
    }
}

#[cfg(test)]
#[path = "tests/content_comparison.rs"]
mod tests;
