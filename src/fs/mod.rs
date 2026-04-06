pub(crate) mod format;
pub(crate) mod item_count;
pub(crate) mod places;
pub(crate) mod search;
pub(crate) mod watch;

mod directory;
mod directory_stats;
mod sort;

fn is_hidden(file_name: &std::ffi::OsStr) -> bool {
    file_name.to_string_lossy().starts_with('.')
}

/// Returns `true` if the directory entry should be treated as hidden.
///
/// On all platforms, entries whose names begin with `.` are hidden.
/// On Windows, entries with the `FILE_ATTRIBUTE_HIDDEN` attribute are also hidden.
fn is_hidden_entry(entry: &std::fs::DirEntry) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        if entry
            .metadata()
            .is_ok_and(|m| m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0)
        {
            return true;
        }
    }
    is_hidden(entry.file_name().as_os_str())
}

#[cfg(test)]
pub(crate) use directory::set_open_in_system_capture_for_test;
pub(crate) use directory::{
    DirectoryFingerprint, DirectorySnapshot, detached_open_command, load_directory_snapshot,
    open_in_system, restore_trash_item, scan_directory_fingerprint,
};
#[cfg(target_os = "macos")]
pub(crate) use directory::save_restore_origins;
pub(crate) use directory_stats::{DirectoryStats, DirectoryStatsScanResult, scan_directory_stats};
pub(crate) use format::{
    describe_io_error, format_item_count, format_size, format_time_ago, rect_contains,
    sanitize_terminal_text,
};
pub(crate) use item_count::count_directory_items;
pub(crate) use places::{build_sidebar_rows, home_dir, trash_dir};
pub(crate) use sort::natural_cmp;
pub(crate) use watch::{
    DirectoryWatchEvent, DirectoryWatcher, directory_watch_debounce, event_affects_visible_entries,
    start_directory_watcher,
};
