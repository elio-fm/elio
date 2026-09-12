use std::{fs, io, path::Path};

pub(crate) fn count_directory_items(dir: &Path, show_hidden: bool) -> io::Result<usize> {
    let mut count = 0usize;
    for item in fs::read_dir(dir)? {
        let item = match item {
            Ok(item) => item,
            Err(_) => continue,
        };
        if !show_hidden && super::is_hidden_entry(&item) {
            continue;
        }
        count += 1;
    }
    Ok(count)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DirectoryStats {
    pub item_count: usize,
    pub folder_count: usize,
    pub file_count: usize,
    pub total_size_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DirectoryStatsScanResult {
    Complete(DirectoryStats),
    Incomplete {
        partial: DirectoryStats,
        error: String,
    },
    Canceled,
}

pub(crate) fn scan_directory_stats(
    root: &Path,
    canceled: &dyn Fn() -> bool,
) -> DirectoryStatsScanResult {
    let mut stats = DirectoryStats::default();
    let mut first_error = None::<io::Error>;
    let mut pending = vec![root.to_path_buf()];

    while let Some(dir) = pending.pop() {
        if canceled() {
            return DirectoryStatsScanResult::Canceled;
        }

        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) => {
                first_error.get_or_insert(error);
                continue;
            }
        };

        for entry in entries {
            if canceled() {
                return DirectoryStatsScanResult::Canceled;
            }

            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    first_error.get_or_insert(error);
                    continue;
                }
            };

            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    first_error.get_or_insert(error);
                    continue;
                }
            };

            stats.item_count = stats.item_count.saturating_add(1);
            if metadata.file_type().is_dir() {
                stats.folder_count = stats.folder_count.saturating_add(1);
                pending.push(path);
            } else {
                stats.file_count = stats.file_count.saturating_add(1);
                stats.total_size_bytes = stats.total_size_bytes.saturating_add(metadata.len());
            }
        }
    }

    match first_error {
        Some(error) => DirectoryStatsScanResult::Incomplete {
            partial: stats,
            error: directory_stats_error_message(&error),
        },
        None => DirectoryStatsScanResult::Complete(stats),
    }
}

fn directory_stats_error_message(error: &io::Error) -> String {
    match error.kind() {
        io::ErrorKind::PermissionDenied => "Some entries unreadable".to_string(),
        io::ErrorKind::NotFound => "Folder changed while scanning".to_string(),
        _ => "Folder totals incomplete".to_string(),
    }
}

#[cfg(test)]
#[path = "tests/directory_statistics.rs"]
mod tests;
