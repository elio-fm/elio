use super::{
    candidate_collection::{collect_size_candidates, compare_candidate_buckets},
    content_comparison::{DuplicateHashCache, verified_groups_for_same_size},
};
use anyhow::Result;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const DUPLICATE_BATCH_GROUP_SIZE: usize = 128;
const DUPLICATE_BATCH_MAX_LATENCY: Duration = Duration::from_millis(120);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DuplicateFile {
    pub path: PathBuf,
    pub name: String,
    pub relative: String,
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DuplicateGroup {
    pub id: u64,
    pub size: u64,
    pub files: Vec<DuplicateFile>,
}

impl DuplicateGroup {
    pub(crate) fn duplicate_bytes(&self) -> u64 {
        self.size
            .saturating_mul(self.files.len().saturating_sub(1) as u64)
    }
}

pub(crate) fn sort_duplicate_groups(groups: &mut [DuplicateGroup]) {
    groups.sort_by(compare_groups);
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum DuplicateScanPhase {
    #[default]
    Walking,
    SizeGrouping,
    ContentChecking,
    Complete,
}

impl DuplicateScanPhase {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Walking => "walking",
            Self::SizeGrouping => "grouping",
            Self::ContentChecking => "checking",
            Self::Complete => "complete",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DuplicateScanStats {
    pub(crate) phase: DuplicateScanPhase,
    pub(crate) visited_nodes: usize,
    pub(crate) scanned_files: usize,
    pub(crate) candidate_files: usize,
    pub(crate) checked_candidates: usize,
    pub(crate) hashed_files: usize,
    pub(crate) cached_hashes: usize,
    pub(crate) processed_bytes: u64,
    pub(crate) verified_files: usize,
    pub(crate) groups: usize,
    pub(crate) duplicate_bytes: u64,
    pub(crate) node_limit_reached: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct DuplicateScanBatch {
    pub(crate) groups: Vec<DuplicateGroup>,
    pub(crate) stats: DuplicateScanStats,
}

#[derive(Clone, Debug)]
pub(crate) struct DuplicateScanResult {
    pub(crate) groups: Vec<DuplicateGroup>,
    pub(crate) stats: DuplicateScanStats,
}

#[cfg(test)]
pub(crate) fn scan_duplicates_streaming(
    cwd: &Path,
    show_hidden: bool,
    is_canceled: impl Fn() -> bool,
    emit_batch: impl FnMut(DuplicateScanBatch) -> bool,
) -> Result<DuplicateScanResult> {
    let mut cache = DuplicateHashCache::default();
    scan_duplicates_streaming_with_cache(cwd, show_hidden, &mut cache, is_canceled, emit_batch)
}

pub(crate) fn scan_duplicates_streaming_with_cache(
    cwd: &Path,
    show_hidden: bool,
    cache: &mut DuplicateHashCache,
    is_canceled: impl Fn() -> bool,
    mut emit_batch: impl FnMut(DuplicateScanBatch) -> bool,
) -> Result<DuplicateScanResult> {
    let (size_groups, mut stats) = collect_size_candidates(cwd, show_hidden, &is_canceled)?;
    stats.phase = DuplicateScanPhase::SizeGrouping;
    stats.candidate_files = size_groups.values().map(Vec::len).sum();
    let mut size_buckets = size_groups.into_values().collect::<Vec<_>>();
    size_buckets.sort_by(|left, right| compare_candidate_buckets(left, right));

    let mut groups = Vec::new();
    let mut next_id = 1u64;
    let mut batch_emitter = DuplicateBatchEmitter::new(&mut emit_batch);
    if !batch_emitter.emit_progress(stats) {
        return Ok(DuplicateScanResult { groups, stats });
    }
    stats.phase = DuplicateScanPhase::ContentChecking;
    if !batch_emitter.emit_progress(stats) {
        return Ok(DuplicateScanResult { groups, stats });
    }

    for candidates in size_buckets {
        if is_canceled() {
            break;
        }
        if candidates.len() < 2 {
            continue;
        }
        let verified = verified_groups_for_same_size(
            candidates,
            cache,
            &mut stats,
            &mut batch_emitter,
            &is_canceled,
        )?;
        for files in verified {
            if files.len() < 2 {
                continue;
            }
            let size = files[0].size;
            let group = DuplicateGroup {
                id: next_id,
                size,
                files,
            };
            next_id = next_id.wrapping_add(1);
            stats.groups += 1;
            stats.duplicate_bytes = stats
                .duplicate_bytes
                .saturating_add(group.duplicate_bytes());
            if !batch_emitter.push(group.clone(), stats) {
                groups.push(group);
                groups.sort_by(compare_groups);
                return Ok(DuplicateScanResult { groups, stats });
            }
            groups.push(group);
        }
    }
    stats.phase = DuplicateScanPhase::Complete;
    let _ = batch_emitter.flush(stats);
    groups.sort_by(compare_groups);
    Ok(DuplicateScanResult { groups, stats })
}

pub(super) struct DuplicateBatchEmitter<'a, F>
where
    F: FnMut(DuplicateScanBatch) -> bool,
{
    pending: Vec<DuplicateGroup>,
    last_emit: Instant,
    last_phase: DuplicateScanPhase,
    emit_batch: &'a mut F,
}

impl<'a, F> DuplicateBatchEmitter<'a, F>
where
    F: FnMut(DuplicateScanBatch) -> bool,
{
    fn new(emit_batch: &'a mut F) -> Self {
        Self {
            pending: Vec::with_capacity(DUPLICATE_BATCH_GROUP_SIZE),
            last_emit: Instant::now() - DUPLICATE_BATCH_MAX_LATENCY,
            last_phase: DuplicateScanPhase::Walking,
            emit_batch,
        }
    }

    fn push(&mut self, group: DuplicateGroup, stats: DuplicateScanStats) -> bool {
        self.pending.push(group);
        if self.pending.len() >= DUPLICATE_BATCH_GROUP_SIZE
            || self.last_emit.elapsed() >= DUPLICATE_BATCH_MAX_LATENCY
        {
            return self.flush(stats);
        }
        true
    }

    pub(super) fn emit_progress(&mut self, stats: DuplicateScanStats) -> bool {
        if stats.phase == self.last_phase && self.last_emit.elapsed() < DUPLICATE_BATCH_MAX_LATENCY
        {
            return true;
        }
        self.last_emit = Instant::now();
        self.last_phase = stats.phase;
        (self.emit_batch)(DuplicateScanBatch {
            groups: Vec::new(),
            stats,
        })
    }

    fn flush(&mut self, stats: DuplicateScanStats) -> bool {
        if self.pending.is_empty() {
            return true;
        }
        let mut groups = std::mem::replace(
            &mut self.pending,
            Vec::with_capacity(DUPLICATE_BATCH_GROUP_SIZE),
        );
        groups.sort_by(compare_groups);
        self.last_emit = Instant::now();
        self.last_phase = stats.phase;
        (self.emit_batch)(DuplicateScanBatch { groups, stats })
    }
}

fn compare_groups(left: &DuplicateGroup, right: &DuplicateGroup) -> std::cmp::Ordering {
    right
        .size
        .cmp(&left.size)
        .then_with(|| right.duplicate_bytes().cmp(&left.duplicate_bytes()))
        .then_with(|| {
            left.files
                .first()
                .map(|file| &file.relative)
                .cmp(&right.files.first().map(|file| &file.relative))
        })
}

#[cfg(test)]
#[path = "tests/duplicate_scanning.rs"]
mod tests;
