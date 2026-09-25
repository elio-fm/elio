mod directory_scanning;
mod directory_statistics;
mod directory_watching;
mod entries;
mod item_display;
mod sort;
mod trash_metadata;
mod trash_restoration;

pub(crate) use directory_scanning::{
    DirectoryFingerprint, DirectorySnapshot, entry_from_path, load_directory_snapshot,
    load_directory_snapshot_cancellable, scan_directory_fingerprint_cancellable,
};
pub(crate) use directory_statistics::{
    DirectoryStats, DirectoryStatsScanResult, count_directory_items, scan_directory_stats,
};
pub(crate) use directory_watching::{
    DirectoryWatchEvent, DirectoryWatcher, directory_watch_debounce, event_affects_visible_entries,
    start_directory_watcher, start_git_head_watcher,
};
pub(crate) use entries::is_hidden_entry;
pub use entries::{Entry, EntryKind, SymlinkInfo};
pub(crate) use item_display::{
    describe_io_error, display_path, format_item_count, format_size, format_size_parts,
    format_time_ago, sanitize_terminal_text, symlink_target_display_label,
};
pub use sort::SortMode;
pub(crate) use sort::natural_cmp;
pub(crate) use trash_metadata::{original_basename_from_path_value, parse_original_path};
pub(crate) use trash_restoration::restore_trash_item;
#[cfg(target_os = "macos")]
pub(crate) use trash_restoration::{
    remove_restore_origins, remove_restore_origins_checked, restore_trash_item_checked_metadata,
    save_restore_origins_checked,
};
