use crate::core::{EntryKind, SymlinkInfo};
use anyhow::{Context, Result};
use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
};

const SEARCH_NODE_VISIT_LIMIT: usize = 5_000_000;
const SEARCH_CANDIDATE_BATCH_SIZE: usize = 512;
const SEARCH_PROGRESS_NODE_INTERVAL: usize = 2_048;

#[derive(Clone, Debug)]
pub(crate) struct SearchCandidate {
    pub path: PathBuf,
    pub name: String,
    pub name_key: String,
    pub relative: String,
    pub relative_key: String,
    pub is_dir: bool,
    pub symlink: Option<SymlinkInfo>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SearchIndexStats {
    pub(crate) visited_nodes: usize,
    pub(crate) node_limit_reached: bool,
    pub(crate) candidate_limit_reached: bool,
}

impl SearchIndexStats {
    pub(crate) fn is_limited(self) -> bool {
        self.node_limit_reached || self.candidate_limit_reached
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SearchIndex {
    pub(crate) candidates: Vec<SearchCandidate>,
    pub(crate) stats: SearchIndexStats,
}

#[derive(Clone, Debug)]
pub(crate) struct SearchIndexBatch {
    pub(crate) candidates: Vec<SearchCandidate>,
    pub(crate) stats: SearchIndexStats,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SearchCandidateScope {
    Files,
    Folders,
}

pub(crate) fn collect_candidates_streaming(
    cwd: &Path,
    show_hidden: bool,
    scope: SearchCandidateScope,
    is_canceled: impl Fn() -> bool,
    emit_batch: impl FnMut(SearchIndexBatch) -> bool,
) -> Result<SearchIndex> {
    collect_candidates_with_limits_and_emitter(
        cwd,
        show_hidden,
        scope,
        SearchCollectionLimits {
            candidate_limit: usize::MAX,
            node_visit_limit: SEARCH_NODE_VISIT_LIMIT,
            batch_size: SEARCH_CANDIDATE_BATCH_SIZE,
        },
        is_canceled,
        emit_batch,
    )
}

struct PendingSearchNode {
    path: PathBuf,
    name: String,
    name_key: String,
    relative: Option<String>,
    relative_key: Option<String>,
    is_dir: bool,
    symlink: Option<SymlinkInfo>,
    enqueue_children: bool,
}

#[derive(Clone, Copy)]
struct SearchCollectionLimits {
    candidate_limit: usize,
    node_visit_limit: usize,
    batch_size: usize,
}

impl PendingSearchNode {
    fn into_candidate(self) -> Option<SearchCandidate> {
        let relative = self.relative?;
        let relative_key = self.relative_key?;
        Some(SearchCandidate {
            path: self.path,
            name: self.name,
            name_key: self.name_key,
            relative,
            relative_key,
            is_dir: self.is_dir,
            symlink: self.symlink,
        })
    }
}

#[cfg(test)]
fn collect_candidates_with_limits(
    cwd: &Path,
    show_hidden: bool,
    scope: SearchCandidateScope,
    candidate_limit: usize,
    node_visit_limit: usize,
) -> Result<SearchIndex> {
    collect_candidates_with_limits_and_emitter(
        cwd,
        show_hidden,
        scope,
        SearchCollectionLimits {
            candidate_limit,
            node_visit_limit,
            batch_size: 0,
        },
        || false,
        |_| true,
    )
}

fn collect_candidates_with_limits_and_emitter(
    cwd: &Path,
    show_hidden: bool,
    scope: SearchCandidateScope,
    limits: SearchCollectionLimits,
    is_canceled: impl Fn() -> bool,
    mut emit_batch: impl FnMut(SearchIndexBatch) -> bool,
) -> Result<SearchIndex> {
    let mut queue = VecDeque::from([cwd.to_path_buf()]);
    let mut visited_nodes = 0usize;
    let mut node_limit_reached = false;
    let mut candidates = Vec::new();
    let emit_batches = limits.batch_size > 0;
    let mut pending_candidates = Vec::with_capacity(limits.batch_size);
    let mut next_progress_at = SEARCH_PROGRESS_NODE_INTERVAL;

    while let Some(dir) = queue.pop_front() {
        if is_canceled() {
            return Ok(build_search_index(
                candidates,
                visited_nodes,
                node_limit_reached,
                limits.candidate_limit,
            ));
        }
        if search_scan_limit_reached(visited_nodes, limits.node_visit_limit) {
            node_limit_reached = true;
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
            if is_canceled() {
                return Ok(build_search_index(
                    candidates,
                    visited_nodes,
                    node_limit_reached,
                    limits.candidate_limit,
                ));
            }
            if search_scan_limit_reached(visited_nodes, limits.node_visit_limit) {
                node_limit_reached = true;
                break;
            }

            let Ok(entry) = entry else {
                continue;
            };
            let file_name = entry.file_name();
            if !show_hidden && super::is_hidden_entry(&entry) {
                continue;
            }

            let Ok(file_type) = entry.file_type() else {
                continue;
            };

            let path = entry.path();
            let classified = classify_entry(&path, &file_type);
            let Some(classified) = classified else {
                continue;
            };
            let is_dir = classified.is_dir;
            let is_symlink_entry = classified.symlink.is_some();

            visited_nodes += 1;
            if emit_batches && visited_nodes >= next_progress_at {
                let stats = SearchIndexStats {
                    visited_nodes,
                    node_limit_reached: false,
                    candidate_limit_reached: false,
                };
                if !emit_search_batch(
                    &mut pending_candidates,
                    limits.batch_size,
                    stats,
                    true,
                    &mut emit_batch,
                ) {
                    return Ok(SearchIndex { candidates, stats });
                }
                next_progress_at = visited_nodes.saturating_add(SEARCH_PROGRESS_NODE_INTERVAL);
            }

            if is_canceled() {
                return Ok(build_search_index(
                    candidates,
                    visited_nodes,
                    node_limit_reached,
                    limits.candidate_limit,
                ));
            }

            let include_candidate = should_include_candidate(is_dir, scope);
            if !is_dir && !include_candidate {
                continue;
            }

            let name = file_name.to_string_lossy().to_string();
            let name_key = name.to_lowercase();
            let enqueue_children = is_dir && !is_symlink_entry && !should_prune_dir(&name_key);
            if !include_candidate && !enqueue_children {
                continue;
            }

            let (relative, relative_key) = if include_candidate {
                let Ok(relative_path) = path.strip_prefix(cwd) else {
                    continue;
                };
                let relative = relative_path.to_string_lossy().replace('\\', "/");
                let relative_key = relative.to_lowercase();
                (Some(relative), Some(relative_key))
            } else {
                (None, None)
            };

            nodes.push(PendingSearchNode {
                path,
                name,
                name_key,
                relative,
                relative_key,
                is_dir,
                symlink: classified.symlink,
                enqueue_children,
            });
        }

        nodes.sort_by(|left, right| {
            // Siblings share the same parent prefix, so sorting by name preserves
            // the same natural order as sorting by their relative paths.
            super::natural_cmp(&left.name_key, &right.name_key)
                .then_with(|| left.name.cmp(&right.name))
        });

        for node in nodes {
            if is_canceled() {
                return Ok(build_search_index(
                    candidates,
                    visited_nodes,
                    node_limit_reached,
                    limits.candidate_limit,
                ));
            }
            if node.enqueue_children {
                queue.push_back(node.path.clone());
            }
            if let Some(candidate) = node.into_candidate() {
                candidates.push(candidate);
                if emit_batches {
                    pending_candidates.push(
                        candidates
                            .last()
                            .expect("candidate was just pushed")
                            .clone(),
                    );
                    if pending_candidates.len() >= limits.batch_size {
                        let stats = SearchIndexStats {
                            visited_nodes,
                            node_limit_reached,
                            candidate_limit_reached: false,
                        };
                        if !emit_search_batch(
                            &mut pending_candidates,
                            limits.batch_size,
                            stats,
                            false,
                            &mut emit_batch,
                        ) {
                            return Ok(SearchIndex { candidates, stats });
                        }
                    }
                }
            }
        }
    }

    let index = build_search_index(
        candidates,
        visited_nodes,
        node_limit_reached,
        limits.candidate_limit,
    );
    if emit_batches {
        let _ = emit_search_batch(
            &mut pending_candidates,
            limits.batch_size,
            index.stats,
            false,
            &mut emit_batch,
        );
    }
    Ok(index)
}

fn build_search_index(
    mut candidates: Vec<SearchCandidate>,
    visited_nodes: usize,
    node_limit_reached: bool,
    candidate_limit: usize,
) -> SearchIndex {
    let candidate_limit_reached = candidates.len() > candidate_limit;
    if candidate_limit_reached {
        candidates.truncate(candidate_limit);
    }
    SearchIndex {
        candidates,
        stats: SearchIndexStats {
            visited_nodes,
            node_limit_reached,
            candidate_limit_reached,
        },
    }
}

fn emit_search_batch(
    pending_candidates: &mut Vec<SearchCandidate>,
    batch_size: usize,
    stats: SearchIndexStats,
    force_progress: bool,
    emit_batch: &mut impl FnMut(SearchIndexBatch) -> bool,
) -> bool {
    if pending_candidates.is_empty() && !force_progress {
        return true;
    }
    let candidates = std::mem::replace(pending_candidates, Vec::with_capacity(batch_size));
    emit_batch(SearchIndexBatch { candidates, stats })
}

fn search_scan_limit_reached(visited_nodes: usize, node_visit_limit: usize) -> bool {
    visited_nodes >= node_visit_limit
}

pub(crate) fn filter_candidates_in<I>(
    candidates: &[SearchCandidate],
    pool: I,
    query: &str,
    limit: usize,
) -> SearchFilterResult
where
    I: IntoIterator<Item = usize>,
{
    if query.trim().is_empty() {
        let pool = pool.into_iter().collect::<Vec<_>>();
        let matches = pool.iter().copied().take(limit).collect();
        return SearchFilterResult { pool, matches };
    }

    let query_key = query.to_lowercase();
    let needle = query_key.as_bytes();
    let mut filtered_pool = Vec::new();
    let mut top = Vec::<(usize, i64, usize)>::with_capacity(limit.min(64));

    for index in pool {
        let candidate = &candidates[index];
        let exact_name_bonus = (candidate.name_key == query_key) as i64 * 220;
        let name_score = fuzzy_score_bytes(needle, candidate.name_key.as_bytes())
            .map(|score| score + 80 + i64::from(candidate.is_dir) * 12 + exact_name_bonus);
        let path_score = fuzzy_score_bytes(needle, candidate.relative_key.as_bytes());
        let score = match (name_score, path_score) {
            (Some(name), Some(path)) => name.max(path),
            (Some(name), None) => name,
            (None, Some(path)) => path,
            (None, None) => continue,
        };

        filtered_pool.push(index);

        let entry = (index, score, candidate.relative.len());
        let insert_at = top
            .binary_search_by(|existing| compare_scored(candidates, existing, &entry))
            .unwrap_or_else(|slot| slot);

        if insert_at >= limit {
            continue;
        }

        top.insert(insert_at, entry);
        if top.len() > limit {
            top.pop();
        }
    }

    let matches = top.into_iter().map(|(index, _, _)| index).collect();
    SearchFilterResult {
        pool: filtered_pool,
        matches,
    }
}

pub(crate) struct SearchFilterResult {
    pub(crate) pool: Vec<usize>,
    pub(crate) matches: Vec<usize>,
}

struct ClassifiedSearchEntry {
    is_dir: bool,
    symlink: Option<SymlinkInfo>,
}

fn classify_entry(path: &Path, file_type: &fs::FileType) -> Option<ClassifiedSearchEntry> {
    if file_type.is_symlink() {
        let target = fs::read_link(path).ok();
        let target_kind = fs::metadata(path).ok().map(|metadata| {
            if metadata.is_dir() {
                EntryKind::Directory
            } else {
                EntryKind::File
            }
        });
        let is_dir = matches!(target_kind, Some(EntryKind::Directory));
        Some(ClassifiedSearchEntry {
            is_dir,
            symlink: Some(SymlinkInfo {
                target,
                target_kind,
            }),
        })
    } else if file_type.is_dir() {
        Some(ClassifiedSearchEntry {
            is_dir: true,
            symlink: None,
        })
    } else if file_type.is_file() {
        Some(ClassifiedSearchEntry {
            is_dir: false,
            symlink: None,
        })
    } else {
        None
    }
}

fn should_include_candidate(is_dir: bool, scope: SearchCandidateScope) -> bool {
    match scope {
        SearchCandidateScope::Files => !is_dir,
        SearchCandidateScope::Folders => is_dir,
    }
}

fn should_prune_dir(name_key: &str) -> bool {
    matches!(name_key, ".git" | "node_modules" | "target")
}

fn compare_scored(
    candidates: &[SearchCandidate],
    left: &(usize, i64, usize),
    right: &(usize, i64, usize),
) -> std::cmp::Ordering {
    right
        .1
        .cmp(&left.1)
        .then_with(|| left.2.cmp(&right.2))
        .then_with(|| {
            super::natural_cmp(
                &candidates[left.0].relative_key,
                &candidates[right.0].relative_key,
            )
            .then_with(|| {
                candidates[left.0]
                    .relative
                    .cmp(&candidates[right.0].relative)
            })
        })
}

fn fuzzy_score_bytes(query: &[u8], text: &[u8]) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    if text.is_empty() {
        return None;
    }

    let mut score = 0i64;
    let mut scan_at = 0usize;
    let mut last_match = None;
    let mut streak = 0i64;

    for &byte in query {
        let mut found = None;
        for (index, &candidate) in text.iter().enumerate().skip(scan_at) {
            if candidate == byte {
                found = Some(index);
                break;
            }
        }
        let index = found?;

        if index == 0
            || matches!(
                text[index.saturating_sub(1)],
                b'/' | b'-' | b'_' | b' ' | b'.'
            )
        {
            score += 18;
        }

        if let Some(previous) = last_match {
            if index == previous + 1 {
                streak += 1;
                score += 20 + streak * 6;
            } else {
                streak = 0;
                score -= (index - previous - 1) as i64;
            }
        } else {
            score += 12;
            score -= index as i64;
        }

        score += 10;
        scan_at = index + 1;
        last_match = Some(index);
    }

    score -= (text.len().saturating_sub(scan_at)) as i64 / 3;
    Some(score)
}

#[cfg(test)]
mod tests;
