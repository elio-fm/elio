mod candidate_collection;
mod content_comparison;
mod duplicate_results;
mod duplicate_scanning;
mod result_selection;

pub(crate) use content_comparison::DuplicateHashCache;
pub use duplicate_results::DuplicateRow;
pub(crate) use duplicate_results::{DuplicateFinderSession, DuplicateFinderState};
#[cfg(test)]
pub(crate) use duplicate_scanning::DuplicateScanPhase;
pub(crate) use duplicate_scanning::{
    DuplicateFile, DuplicateGroup, DuplicateScanBatch, DuplicateScanResult, DuplicateScanStats,
    scan_duplicates_streaming_with_cache, sort_duplicate_groups,
};
