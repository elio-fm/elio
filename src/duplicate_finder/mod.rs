mod candidate_collection;
mod content_comparison;
mod duplicate_scanning;

pub(crate) use content_comparison::DuplicateHashCache;
#[cfg(test)]
pub(crate) use duplicate_scanning::DuplicateScanPhase;
pub(crate) use duplicate_scanning::{
    DuplicateFile, DuplicateGroup, DuplicateScanBatch, DuplicateScanResult, DuplicateScanStats,
    scan_duplicates_streaming_with_cache, sort_duplicate_groups,
};
