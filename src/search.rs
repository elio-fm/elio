use anyhow::{Context, Result};
use std::{
    collections::VecDeque,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

const SEARCH_NODE_VISIT_LIMIT: usize = 250_000;

#[derive(Clone, Debug)]
pub struct SearchCandidate {
    pub path: PathBuf,
    pub name: String,
    pub name_key: String,
    pub relative: String,
    pub relative_key: String,
    pub is_dir: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchCandidateScope {
    Files,
    Folders,
}

pub fn collect_candidates(
    cwd: &Path,
    show_hidden: bool,
    scope: SearchCandidateScope,
) -> Result<Vec<SearchCandidate>> {
    collect_candidates_with_limits(cwd, show_hidden, scope, usize::MAX, SEARCH_NODE_VISIT_LIMIT)
}

fn collect_candidates_with_limits(
    cwd: &Path,
    show_hidden: bool,
    scope: SearchCandidateScope,
    candidate_limit: usize,
    node_visit_limit: usize,
) -> Result<Vec<SearchCandidate>> {
    let mut queue = VecDeque::from([cwd.to_path_buf()]);
    let mut visited_nodes = 0usize;
    let mut candidates = Vec::new();

    while let Some(dir) = queue.pop_front() {
        if visited_nodes >= node_visit_limit {
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
            if visited_nodes >= node_visit_limit {
                break;
            }

            let Ok(entry) = entry else {
                continue;
            };
            let file_name = entry.file_name();
            if !show_hidden && is_hidden(file_name.as_os_str()) {
                continue;
            }

            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            let file_type = metadata.file_type();
            if file_type.is_symlink() {
                continue;
            }

            let is_dir = file_type.is_dir();
            let is_file = file_type.is_file();
            if !is_dir && !is_file {
                continue;
            }

            visited_nodes += 1;

            let Ok(relative_path) = path.strip_prefix(cwd) else {
                continue;
            };
            let relative = relative_path.to_string_lossy().replace('\\', "/");
            let name = file_name.to_string_lossy().to_string();
            let name_key = name.to_lowercase();
            let relative_key = relative.to_lowercase();
            nodes.push(SearchCandidate {
                path,
                name,
                name_key,
                relative,
                relative_key,
                is_dir,
            });
        }

        nodes.sort_by(|left, right| {
            left.name_key
                .cmp(&right.name_key)
                .then_with(|| left.relative_key.cmp(&right.relative_key))
        });

        for node in nodes {
            if node.is_dir && !should_prune_dir(&node.name_key) {
                queue.push_back(node.path.clone());
            }
            if should_include_candidate(node.is_dir, scope) {
                candidates.push(node);
            }
        }
    }

    if candidates.len() > candidate_limit {
        candidates.truncate(candidate_limit);
    }
    Ok(candidates)
}

pub fn filter_candidates_in<I>(
    candidates: &[SearchCandidate],
    pool: I,
    query: &str,
    limit: usize,
) -> Vec<usize>
where
    I: IntoIterator<Item = usize>,
{
    if query.trim().is_empty() {
        return pool.into_iter().take(limit).collect();
    }

    let query_key = query.to_lowercase();
    let needle = query_key.as_bytes();
    let mut top = Vec::<(usize, i64, usize)>::with_capacity(limit.min(64));

    for index in pool {
        let candidate = &candidates[index];
        let exact_name_bonus = (candidate.name_key == query_key) as i64 * 220;
        let name_score = fuzzy_score_bytes(needle, candidate.name_key.as_bytes()).map(|score| {
            score + 80 + i64::from(candidate.is_dir) * 12 + exact_name_bonus
        });
        let path_score = fuzzy_score_bytes(needle, candidate.relative_key.as_bytes());
        let score = match (name_score, path_score) {
            (Some(name), Some(path)) => name.max(path),
            (Some(name), None) => name,
            (None, Some(path)) => path,
            (None, None) => continue,
        };

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

    top.into_iter().map(|(index, _, _)| index).collect()
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
            candidates[left.0]
                .relative_key
                .cmp(&candidates[right.0].relative_key)
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

fn is_hidden(file_name: &OsStr) -> bool {
    file_name.to_string_lossy().starts_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-search-{label}-{unique}"))
    }

    #[test]
    fn fuzzy_filter_prefers_tighter_name_match() {
        let candidates = vec![
            SearchCandidate {
                path: PathBuf::from("/tmp/src/main.rs"),
                name: "main.rs".to_string(),
                name_key: "main.rs".to_string(),
                relative: "src/main.rs".to_string(),
                relative_key: "src/main.rs".to_string(),
                is_dir: false,
            },
            SearchCandidate {
                path: PathBuf::from("/tmp/docs/readme.md"),
                name: "readme.md".to_string(),
                name_key: "readme.md".to_string(),
                relative: "docs/readme.md".to_string(),
                relative_key: "docs/readme.md".to_string(),
                is_dir: false,
            },
        ];

        let matches = filter_candidates_in(&candidates, 0..candidates.len(), "mn", 10);
        assert_eq!(matches.first().copied(), Some(0));
    }

    #[test]
    fn collect_candidates_respects_hidden_toggle() {
        let root = temp_path("hidden-toggle");
        fs::create_dir_all(root.join(".hidden-root/needle")).expect("failed to create hidden dir");
        fs::create_dir_all(root.join("projects/needle")).expect("failed to create visible dir");

        let hidden_off = collect_candidates_with_limits(
            &root,
            false,
            SearchCandidateScope::Folders,
            100,
            1_000,
        )
        .expect("failed to collect visible candidates");
        assert!(hidden_off.iter().any(|candidate| candidate.relative == "projects"));
        assert!(hidden_off
            .iter()
            .any(|candidate| candidate.relative == "projects/needle"));
        assert!(!hidden_off
            .iter()
            .any(|candidate| candidate.relative == ".hidden-root/needle"));

        let hidden_on = collect_candidates_with_limits(
            &root,
            true,
            SearchCandidateScope::Folders,
            100,
            1_000,
        )
        .expect("failed to collect hidden candidates");
        assert!(hidden_on
            .iter()
            .any(|candidate| candidate.relative == ".hidden-root/needle"));

        fs::remove_dir_all(root).expect("failed to remove temp tree");
    }

    #[test]
    fn collect_candidates_follow_stable_breadth_first_order_under_limit() {
        let root = temp_path("breadth-first-order");
        fs::create_dir_all(root.join(".hidden-root/needle")).expect("failed to create target dir");
        fs::create_dir_all(root.join("alpha")).expect("failed to create alpha dir");
        fs::create_dir_all(root.join("beta")).expect("failed to create beta dir");
        fs::create_dir_all(root.join("gamma")).expect("failed to create gamma dir");

        let candidates = collect_candidates_with_limits(
            &root,
            true,
            SearchCandidateScope::Folders,
            6,
            1_000,
        )
        .expect("failed to collect candidates");

        assert_eq!(candidates[0].relative, ".hidden-root");
        assert_eq!(candidates[1].relative, "alpha");
        assert_eq!(candidates[2].relative, "beta");
        assert_eq!(candidates[3].relative, "gamma");
        assert!(candidates
            .iter()
            .any(|candidate| candidate.relative == ".hidden-root/needle"));

        fs::remove_dir_all(root).expect("failed to remove temp tree");
    }
}
