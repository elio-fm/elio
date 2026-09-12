#[cfg(unix)]
use anyhow::{Context, anyhow};
use anyhow::{Result, bail};
use std::path::{Component, Path, PathBuf};
#[cfg(unix)]
use std::{fs, io};

pub(super) struct DeferredSymlink {
    pub(super) path: PathBuf,
    pub(super) target: PathBuf,
}

pub(super) fn safe_relative_link_target(dest_dir: &Path, link_path: &Path, target: &Path) -> bool {
    if target.is_absolute() {
        return false;
    }
    let Some(parent) = link_path.parent() else {
        return false;
    };
    let Ok(parent) = parent.strip_prefix(dest_dir) else {
        return false;
    };
    let mut resolved = parent.to_path_buf();
    for component in target.components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    return false;
                }
            }
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    !resolved.as_os_str().is_empty()
}

pub(super) fn extract_deferred_symlinks(
    dest_dir: &Path,
    symlinks: Vec<DeferredSymlink>,
) -> Result<usize> {
    let mut skipped = 0usize;
    for link in symlinks {
        if create_safe_symlink(dest_dir, &link).is_err() {
            skipped += 1;
        }
    }
    Ok(skipped)
}

#[cfg(unix)]
fn create_safe_symlink(dest_dir: &Path, link: &DeferredSymlink) -> Result<()> {
    use std::os::unix::fs::symlink;

    let parent = link
        .path
        .parent()
        .ok_or_else(|| anyhow!("Could not determine symlink parent"))?;
    if parent_contains_symlink(dest_dir, parent)? || fs::symlink_metadata(&link.path).is_ok() {
        bail!("Unsafe TAR symlink");
    }
    fs::create_dir_all(parent).with_context(|| format!("Could not create {}", parent.display()))?;
    symlink(&link.target, &link.path)
        .with_context(|| format!("Could not create symlink {}", link.path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn create_safe_symlink(_dest_dir: &Path, link: &DeferredSymlink) -> Result<()> {
    let _ = (&link.path, &link.target);
    bail!("Archive symlinks are not supported on this platform")
}

#[cfg(unix)]
fn parent_contains_symlink(dest_dir: &Path, parent: &Path) -> Result<bool> {
    let relative = parent.strip_prefix(dest_dir).with_context(|| {
        format!(
            "Archive entry escapes the destination: {}",
            parent.display()
        )
    })?;
    let mut cursor = dest_dir.to_path_buf();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            continue;
        };
        cursor.push(part);
        match fs::symlink_metadata(&cursor) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(false)
}

pub(super) fn checked_output_path(dest_dir: &Path, entry_path: &Path) -> Result<PathBuf> {
    let mut relative = PathBuf::new();
    for component in entry_path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!(
                    "Archive entry escapes the destination: {}",
                    entry_path.display()
                );
            }
        }
    }
    if relative.as_os_str().is_empty() {
        bail!("Archive entry has an empty path");
    }
    let out = dest_dir.join(relative);
    if !out.starts_with(dest_dir) {
        bail!(
            "Archive entry escapes the destination: {}",
            entry_path.display()
        );
    }
    Ok(out)
}

pub(super) fn checked_output_name(dest_dir: &Path, entry_name: &str) -> Result<PathBuf> {
    let normalized = entry_name.replace('\\', "/");
    checked_output_path(dest_dir, Path::new(&normalized))
}
