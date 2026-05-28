use anyhow::{Context, Result};
use std::{
    env, fs, io,
    io::Write,
    path::{Path, PathBuf},
};

use super::scripts::{init_script, nu_string_literal, shell_quote};
use super::{InstallReport, Shell, UninstallReport};

pub(crate) const MANAGED_START: &str = "# >>> elio shell integration >>>";
pub(crate) const MANAGED_END: &str = "# <<< elio shell integration <<<";

pub(crate) fn install(shell: Shell, binary: &str) -> Result<InstallReport> {
    let script = managed_script(shell, binary);
    let path = integration_path(shell)?;

    match shell {
        Shell::Fish | Shell::Nu => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            if let Some(existing) = read_utf8_if_exists(&path)?
                && !has_managed_block(&existing)?
            {
                anyhow::bail!(
                    "error: refusing to overwrite {} because it is not managed by elio",
                    path.display()
                );
            }
            write_text_atomic(&path, &script)?;
        }
        Shell::Bash | Shell::Zsh => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            let existing = read_utf8_if_exists(&path)?.unwrap_or_default();
            let updated = upsert_managed_block(&existing, &script)?;
            write_text_atomic(&path, &updated)?;
        }
    }

    Ok(InstallReport {
        shell,
        reload_command: reload_command(shell, &path),
        path,
    })
}

pub(crate) fn uninstall(shell: Shell) -> Result<UninstallReport> {
    let path = integration_path(shell)?;
    let changed = match shell {
        Shell::Fish | Shell::Nu => uninstall_managed_file(&path)?,
        Shell::Bash | Shell::Zsh => uninstall_posix(&path)?,
    };

    Ok(UninstallReport {
        shell,
        reload_command: uninstall_reload_command(shell),
        path,
        changed,
        removed_file: matches!(shell, Shell::Fish | Shell::Nu) && changed,
    })
}

fn integration_path(shell: Shell) -> Result<PathBuf> {
    match shell {
        Shell::Fish => Ok(config_home_for_shell("fish")?.join("fish/conf.d/elio.fish")),
        Shell::Nu => Ok(config_home_for_shell("nu")?.join("nushell/autoload/elio.nu")),
        Shell::Bash => Ok(home_dir()?.join(".bashrc")),
        Shell::Zsh => Ok(zsh_config_dir()?.join(".zshrc")),
    }
}

fn config_home_for_shell(shell: &str) -> Result<PathBuf> {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .with_context(|| {
            format!("error: could not find $XDG_CONFIG_HOME or $HOME for {shell} integration")
        })
}

fn zsh_config_dir() -> Result<PathBuf> {
    match env::var_os("ZDOTDIR") {
        Some(zdotdir) if !zdotdir.is_empty() => Ok(PathBuf::from(zdotdir)),
        _ => home_dir(),
    }
}

fn home_dir() -> Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .context("error: could not find $HOME for shell integration")
}

fn reload_command(shell: Shell, path: &Path) -> String {
    match shell {
        Shell::Fish => format!("source {}", shell_quote(path)),
        Shell::Bash | Shell::Zsh => format!("source {}", shell_quote(path)),
        Shell::Nu => format!("source {}", nu_string_literal(path)),
    }
}

pub(super) fn uninstall_reload_command(shell: Shell) -> String {
    match shell {
        Shell::Bash => "unset -f elio".to_string(),
        Shell::Zsh => "unfunction elio 2>/dev/null || true".to_string(),
        Shell::Fish => "functions --erase elio".to_string(),
        Shell::Nu => "hide elio".to_string(),
    }
}

pub(super) fn managed_script(shell: Shell, binary: &str) -> String {
    format!(
        "{MANAGED_START}\n{}\n{MANAGED_END}\n",
        init_script(shell, binary).trim_end()
    )
}

pub(super) fn upsert_managed_block(existing: &str, block: &str) -> Result<String> {
    if let Some(start) = existing.find(MANAGED_START) {
        if let Some(relative_end) = existing[start..].find(MANAGED_END) {
            let end = start + relative_end + MANAGED_END.len();
            let after = remove_managed_blocks(&existing[end..])?
                .unwrap_or_else(|| existing[end..].to_string());
            let mut updated = String::new();
            updated.push_str(existing[..start].trim_end_matches('\n'));
            if !updated.is_empty() {
                updated.push_str("\n\n");
            }
            updated.push_str(block.trim_end());
            updated.push_str(after.trim_start_matches('\n'));
            if !updated.ends_with('\n') {
                updated.push('\n');
            }
            return Ok(updated);
        }

        anyhow::bail!("error: found elio shell integration start marker without end marker");
    }

    let mut updated = existing.trim_end_matches('\n').to_string();
    if !updated.is_empty() {
        updated.push_str("\n\n");
    }
    updated.push_str(block.trim_end());
    updated.push('\n');
    Ok(updated)
}

fn uninstall_posix(path: &Path) -> Result<bool> {
    let Some(existing) = read_utf8_if_exists(path)? else {
        return Ok(false);
    };

    let Some(updated) = remove_managed_blocks(&existing)? else {
        return Ok(false);
    };

    write_text_atomic(path, &updated)?;
    Ok(true)
}

fn uninstall_managed_file(path: &Path) -> Result<bool> {
    let Some(existing) = read_utf8_if_exists(path)? else {
        return Ok(false);
    };

    if !has_managed_block(&existing)? {
        anyhow::bail!(
            "error: refusing to remove {} because it is not managed by elio",
            path.display()
        );
    }

    fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
    Ok(true)
}

fn read_utf8_if_exists(path: &Path) -> Result<Option<String>> {
    match fs::read(path) {
        Ok(bytes) => String::from_utf8(bytes)
            .with_context(|| format!("failed to read {} as UTF-8", path.display()))
            .map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

pub(super) fn write_text_atomic(path: &Path, contents: &str) -> Result<()> {
    let write_path = resolve_write_path(path)?;
    let parent = write_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let (temp_path, mut temp_file) = create_atomic_temp(&write_path, parent)?;

    let result = (|| -> Result<()> {
        match fs::metadata(&write_path) {
            Ok(metadata) => temp_file
                .set_permissions(metadata.permissions())
                .with_context(|| format!("failed to set permissions on {}", temp_path.display()))?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to inspect {}", write_path.display()));
            }
        }

        temp_file
            .write_all(contents.as_bytes())
            .with_context(|| format!("failed to write {}", temp_path.display()))?;
        temp_file
            .sync_all()
            .with_context(|| format!("failed to sync {}", temp_path.display()))?;
        drop(temp_file);

        replace_with_temp(&temp_path, &write_path)
            .with_context(|| format!("failed to replace {}", write_path.display()))?;
        sync_parent_dir(parent).with_context(|| format!("failed to sync {}", parent.display()))?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    result
}

pub(super) fn resolve_write_path(path: &Path) -> Result<PathBuf> {
    let mut current = path.to_path_buf();

    for _ in 0..16 {
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(current),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to inspect {}", current.display()));
            }
        };

        if !metadata.file_type().is_symlink() {
            return Ok(current);
        }

        let target = fs::read_link(&current)
            .with_context(|| format!("failed to read symlink {}", current.display()))?;
        current = if target.is_absolute() {
            target
        } else {
            current
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."))
                .join(target)
        };
    }

    anyhow::bail!(
        "failed to resolve shell integration symlink chain for {}",
        path.display()
    )
}

fn create_atomic_temp(path: &Path, parent: &Path) -> Result<(PathBuf, fs::File)> {
    let file_name = path
        .file_name()
        .context("failed to build temporary shell integration path")?
        .to_string_lossy();
    let process_id = std::process::id();

    for attempt in 0..100 {
        let temp_path = parent.join(format!(".{file_name}.elio-tmp-{process_id}-{attempt}"));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to create {}", temp_path.display()));
            }
        }
    }

    anyhow::bail!(
        "failed to create a temporary shell integration file beside {}",
        path.display()
    )
}

#[cfg(windows)]
fn replace_with_temp(temp_path: &Path, write_path: &Path) -> io::Result<()> {
    if write_path.exists() {
        fs::remove_file(write_path)?;
    }
    fs::rename(temp_path, write_path)
}

#[cfg(not(windows))]
fn replace_with_temp(temp_path: &Path, write_path: &Path) -> io::Result<()> {
    fs::rename(temp_path, write_path)
}

#[cfg(unix)]
fn sync_parent_dir(parent: &Path) -> io::Result<()> {
    match fs::File::open(parent)?.sync_all() {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::InvalidInput | io::ErrorKind::Unsupported
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

#[cfg(not(unix))]
fn sync_parent_dir(_parent: &Path) -> io::Result<()> {
    Ok(())
}

pub(super) fn remove_managed_blocks(existing: &str) -> Result<Option<String>> {
    let mut updated = existing.to_string();
    let mut changed = false;

    while let Some(next) = remove_first_managed_block(&updated)? {
        updated = next;
        changed = true;
    }

    Ok(changed.then_some(updated))
}

fn remove_first_managed_block(existing: &str) -> Result<Option<String>> {
    let Some(start) = existing.find(MANAGED_START) else {
        return Ok(None);
    };
    let Some(relative_end) = existing[start..].find(MANAGED_END) else {
        anyhow::bail!("error: found elio shell integration start marker without end marker");
    };

    let end = start + relative_end + MANAGED_END.len();
    let before = existing[..start].trim_end_matches('\n');
    let after = existing[end..].trim_start_matches('\n');
    let mut updated = String::new();
    updated.push_str(before);
    if !before.is_empty() && !after.is_empty() {
        updated.push_str("\n\n");
    }
    updated.push_str(after);
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    Ok(Some(updated))
}

fn has_managed_block(existing: &str) -> Result<bool> {
    let Some(start) = existing.find(MANAGED_START) else {
        return Ok(false);
    };
    if !existing[start..].contains(MANAGED_END) {
        anyhow::bail!("error: found elio shell integration start marker without end marker");
    }
    Ok(true)
}
