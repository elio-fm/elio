use super::{
    content_comparison::{DuplicateHashCacheKey, duplicate_hash_cache_key},
    duplicate_scanning::DuplicateScanStats,
};
use anyhow::{Context, Result};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::{Path, PathBuf},
};

const DUPLICATE_NODE_VISIT_LIMIT: usize = 5_000_000;
const SCAN_YIELD_NODES: usize = 512;
const SCAN_SLEEP_NODES: usize = 16_384;
pub(super) const SMALL_FILE_FULL_HASH_LIMIT: u64 = 256 * 1024;

#[derive(Clone, Debug)]
pub(super) struct CandidateFile {
    pub(super) path: PathBuf,
    pub(super) name: String,
    pub(super) relative: String,
    pub(super) size: u64,
    pub(super) modified: Option<std::time::SystemTime>,
    pub(super) cache_key: Option<DuplicateHashCacheKey>,
}

pub(super) fn collect_size_candidates(
    cwd: &Path,
    show_hidden: bool,
    is_canceled: &impl Fn() -> bool,
) -> Result<(HashMap<u64, Vec<CandidateFile>>, DuplicateScanStats)> {
    let mut queue = VecDeque::from([cwd.to_path_buf()]);
    let mut stats = DuplicateScanStats::default();
    let mut size_groups: HashMap<u64, Vec<CandidateFile>> = HashMap::new();

    while let Some(dir) = queue.pop_front() {
        if is_canceled() {
            break;
        }
        if stats.visited_nodes >= DUPLICATE_NODE_VISIT_LIMIT {
            stats.node_limit_reached = true;
            break;
        }
        let read_dir = match fs::read_dir(&dir) {
            Ok(read_dir) => read_dir,
            Err(error) if dir == cwd => {
                return Err(error).with_context(|| format!("failed to read {}", cwd.display()));
            }
            Err(_) => continue,
        };
        let mut nodes = Vec::new();
        for entry in read_dir {
            if is_canceled() || stats.visited_nodes >= DUPLICATE_NODE_VISIT_LIMIT {
                stats.node_limit_reached |= stats.visited_nodes >= DUPLICATE_NODE_VISIT_LIMIT;
                break;
            }
            let Ok(entry) = entry else {
                continue;
            };
            if !show_hidden && crate::fs::is_hidden_entry(&entry) {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let name_key = name.to_lowercase();
            stats.visited_nodes += 1;
            breathe_after_node(stats.visited_nodes);
            if file_type.is_dir() {
                if !should_prune_dir(&name_key) {
                    nodes.push((name_key, path, true));
                }
                continue;
            }
            if file_type.is_symlink() || !file_type.is_file() {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let size = metadata.len();
            if size == 0 {
                continue;
            }
            stats.scanned_files += 1;
            let Ok(relative_path) = path.strip_prefix(cwd) else {
                continue;
            };
            let relative = relative_path.to_string_lossy().replace('\\', "/");
            size_groups.entry(size).or_default().push(CandidateFile {
                path: path.clone(),
                name,
                relative,
                size,
                modified: metadata.modified().ok(),
                cache_key: duplicate_hash_cache_key(&path, &metadata),
            });
        }
        nodes.sort_by(|a, b| crate::fs::natural_cmp(&a.0, &b.0));
        for (_, path, is_dir) in nodes {
            if is_dir {
                queue.push_back(path);
            }
        }
    }
    size_groups.retain(|_, files| files.len() > 1);
    Ok((size_groups, stats))
}

pub(super) fn breathe_after_node(count: usize) {
    if count.is_multiple_of(SCAN_SLEEP_NODES) {
        std::thread::sleep(std::time::Duration::from_millis(1));
    } else if count.is_multiple_of(SCAN_YIELD_NODES) {
        std::thread::yield_now();
    }
}

pub(super) fn compare_candidate_buckets(
    left: &[CandidateFile],
    right: &[CandidateFile],
) -> std::cmp::Ordering {
    candidate_bucket_priority(left).cmp(&candidate_bucket_priority(right))
}

fn candidate_bucket_priority(
    files: &[CandidateFile],
) -> (
    u8,
    std::cmp::Reverse<usize>,
    u64,
    std::cmp::Reverse<u64>,
    u64,
) {
    let size = files.first().map_or(0, |file| file.size);
    let class = if size <= SMALL_FILE_FULL_HASH_LIMIT {
        0
    } else if size <= 64 * 1024 * 1024 && files.len() >= 3 {
        1
    } else if size <= 512 * 1024 * 1024 {
        2
    } else {
        3
    };
    (
        class,
        std::cmp::Reverse(files.len()),
        candidate_bucket_read_bytes(files),
        std::cmp::Reverse(duplicate_candidate_bytes(files)),
        size,
    )
}

fn candidate_bucket_read_bytes(files: &[CandidateFile]) -> u64 {
    files
        .first()
        .map_or(0, |file| file.size.saturating_mul(files.len() as u64))
}

fn duplicate_candidate_bytes(files: &[CandidateFile]) -> u64 {
    files.first().map_or(0, |file| {
        file.size
            .saturating_mul(files.len().saturating_sub(1) as u64)
    })
}

fn should_prune_dir(name_key: &str) -> bool {
    matches!(name_key, ".git" | "node_modules" | "target")
}

#[cfg(test)]
#[path = "tests/candidate_collection.rs"]
mod tests;
