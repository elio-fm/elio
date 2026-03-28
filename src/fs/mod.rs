pub(crate) mod format;
pub(crate) mod item_count;
pub(crate) mod places;
pub(crate) mod search;
pub(crate) mod watch;

mod directory;
mod sort;

fn is_hidden(file_name: &std::ffi::OsStr) -> bool {
    file_name.to_string_lossy().starts_with('.')
}

pub(crate) use directory::{
    DirectoryFingerprint, DirectorySnapshot, detached_open, load_directory_snapshot,
    restore_trash_item, scan_directory_fingerprint,
};
pub(crate) use format::{
    describe_io_error, format_item_count, format_size, format_time_ago, rect_contains,
    sanitize_terminal_text,
};
pub(crate) use item_count::count_directory_items;
pub(crate) use places::{build_sidebar_rows, trash_dir};
pub(crate) use sort::natural_cmp;
pub(crate) use watch::{
    DirectoryWatchEvent, DirectoryWatcher, directory_watch_debounce, event_affects_visible_entries,
    start_directory_watcher,
};
