use super::ClipOp;
use super::FileOperationsState;
use anyhow::Result;
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;

pub(crate) struct SymlinkCompletion {
    pub(crate) created: bool,
    pub(crate) status: String,
}

impl FileOperationsState {
    pub(crate) fn create_symlinks(&self, cwd: &Path, relative: bool) -> Result<SymlinkCompletion> {
        let Some(clipboard) = &self.clipboard else {
            return Ok(SymlinkCompletion {
                created: false,
                status: "Nothing to link".to_string(),
            });
        };
        if clipboard.op != ClipOp::Yank {
            return Ok(SymlinkCompletion {
                created: false,
                status: "Yank items before linking".to_string(),
            });
        }

        let paths = clipboard.paths.clone();
        if paths.is_empty() {
            return Ok(SymlinkCompletion {
                created: false,
                status: "Nothing to link".to_string(),
            });
        }

        #[cfg(unix)]
        {
            let mut created = Vec::new();
            let mut first_error = None;
            for source in paths {
                let link_path = unique_link_dest(cwd, &source);
                let target = if relative {
                    relative_path(cwd, &source)
                } else {
                    source.clone()
                };
                match std::os::unix::fs::symlink(&target, &link_path) {
                    Ok(()) => created.push(link_path),
                    Err(error) => {
                        let name = source
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("item");
                        first_error = Some(format!("Could not link \"{name}\": {error}"));
                        break;
                    }
                }
            }

            let status = match (created.len(), first_error) {
                (0, Some(error)) => error,
                (1, None) => format!(
                    "Created symlink \"{}\"",
                    created[0]
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("item")
                ),
                (n, None) => format!("Created {n} symlinks"),
                (n, Some(error)) => format!("Created {n} symlinks; last error: {error}"),
            };
            Ok(SymlinkCompletion {
                created: !created.is_empty(),
                status,
            })
        }

        #[cfg(not(unix))]
        {
            let _ = (cwd, relative);
            Ok(SymlinkCompletion {
                created: false,
                status: "Symlinks are not supported on this platform".to_string(),
            })
        }
    }
}

#[cfg(unix)]
fn unique_link_dest(dest_dir: &Path, source: &Path) -> PathBuf {
    let name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "link".to_string());
    let candidate = dest_dir.join(&name);
    if std::fs::symlink_metadata(&candidate).is_err() {
        return candidate;
    }

    let source_path = Path::new(&name);
    let stem = source_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.clone());
    let ext = source_path
        .extension()
        .map(|ext| ext.to_string_lossy().into_owned());
    for index in 1u32.. {
        let next_name = match &ext {
            Some(ext) => format!("{stem}_{index}.{ext}"),
            None => format!("{stem}_{index}"),
        };
        let next = dest_dir.join(next_name);
        if std::fs::symlink_metadata(&next).is_err() {
            return next;
        }
    }
    candidate
}

#[cfg(unix)]
fn relative_path(from_dir: &Path, to: &Path) -> PathBuf {
    let from = from_dir.components().collect::<Vec<_>>();
    let to = to.components().collect::<Vec<_>>();
    let common = from
        .iter()
        .zip(to.iter())
        .take_while(|(left, right)| left == right)
        .count();

    let mut relative = PathBuf::new();
    for _ in common..from.len() {
        relative.push("..");
    }
    for component in &to[common..] {
        relative.push(component.as_os_str());
    }
    if relative.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        relative
    }
}
