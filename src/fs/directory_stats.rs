use std::{fs, io, path::Path};

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
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-directory-stats-{label}-{unique}"))
    }

    #[test]
    fn recursive_directory_stats_include_nested_entries_and_sizes() {
        let root = temp_path("recursive");
        let nested = root.join("nested");
        fs::create_dir_all(&nested).expect("failed to create nested dir");
        fs::write(root.join("a.txt"), vec![b'a'; 500]).expect("failed to write file");
        fs::write(nested.join("b.txt"), vec![b'b'; 700]).expect("failed to write nested file");

        let result = scan_directory_stats(&root, &|| false);

        assert_eq!(
            result,
            DirectoryStatsScanResult::Complete(DirectoryStats {
                item_count: 3,
                folder_count: 1,
                file_count: 2,
                total_size_bytes: 1_200,
            })
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[cfg(unix)]
    #[test]
    fn recursive_directory_stats_do_not_follow_symlinked_directories() {
        let root = temp_path("symlink-dir");
        let nested = root.join("nested");
        let linked = root.join("linked");
        fs::create_dir_all(&nested).expect("failed to create nested dir");
        fs::write(nested.join("inside.txt"), vec![b'x'; 900]).expect("failed to write file");
        symlink(&nested, &linked).expect("failed to create symlink");

        let result = scan_directory_stats(&root, &|| false);
        let symlink_size = fs::symlink_metadata(&linked)
            .expect("failed to stat symlink")
            .len();

        assert_eq!(
            result,
            DirectoryStatsScanResult::Complete(DirectoryStats {
                item_count: 3,
                folder_count: 1,
                file_count: 2,
                total_size_bytes: 900 + symlink_size,
            })
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
