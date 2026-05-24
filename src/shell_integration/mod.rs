use anyhow::{Context, Result};
use std::{
    env, fs, io,
    io::Write,
    path::{Path, PathBuf},
};

const MANAGED_START: &str = "# >>> elio shell integration >>>";
const MANAGED_END: &str = "# <<< elio shell integration <<<";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Shell {
    Bash,
    Zsh,
    Fish,
}

impl Shell {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "bash" => Ok(Self::Bash),
            "zsh" => Ok(Self::Zsh),
            "fish" => Ok(Self::Fish),
            shell => Err(format!(
                "error: unsupported shell '{shell}'\n\nsupported shells: bash, zsh, fish"
            )),
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
        }
    }
}

pub(crate) struct InstallReport {
    pub(crate) shell: Shell,
    pub(crate) path: PathBuf,
    pub(crate) reload_command: String,
}

pub(crate) struct UninstallReport {
    pub(crate) shell: Shell,
    pub(crate) path: PathBuf,
    pub(crate) reload_command: String,
    pub(crate) changed: bool,
    pub(crate) removed_file: bool,
}

pub(crate) fn detect_shell() -> Result<Shell> {
    if let Some(shell) = detect_parent_shell() {
        return Ok(shell);
    }

    detect_shell_from_environment()
}

fn detect_shell_from_environment() -> Result<Shell> {
    let shell = env::var("SHELL").context(
        "error: could not detect your shell from the parent process or $SHELL\n\nRun 'elio shell install fish', 'elio shell install bash', or 'elio shell install zsh' instead.",
    )?;
    let name = shell_name_from_command(&shell).unwrap_or(shell);

    Shell::parse(&name).map_err(anyhow::Error::msg)
}

#[cfg(unix)]
fn detect_parent_shell() -> Option<Shell> {
    let parent_pid = unsafe { libc::getppid() }.to_string();
    let output = std::process::Command::new("ps")
        .args(["-p", &parent_pid, "-o", "comm="])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let command = String::from_utf8(output.stdout).ok()?;
    let name = shell_name_from_command(&command)?;
    Shell::parse(&name).ok()
}

#[cfg(not(unix))]
fn detect_parent_shell() -> Option<Shell> {
    None
}

fn shell_name_from_command(command: &str) -> Option<String> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }

    let first_word = command.split_whitespace().next()?;
    let without_login_prefix = first_word.strip_prefix('-').unwrap_or(first_word);
    let file_name = Path::new(without_login_prefix)
        .file_name()
        .and_then(|name| name.to_str())?;
    Some(file_name.strip_prefix('-').unwrap_or(file_name).to_string())
}

pub(crate) fn binary_command(invocation: Option<&str>, executable: &Path) -> String {
    let Some(invocation) = invocation else {
        return shell_quote(executable);
    };

    if invocation.contains('/') || invocation.contains('\\') || invocation.starts_with('.') {
        shell_quote(executable)
    } else {
        "command elio".to_string()
    }
}

pub(crate) fn init_script(shell: Shell, binary: &str) -> String {
    match shell {
        Shell::Bash | Shell::Zsh => posix_init_script(binary),
        Shell::Fish => fish_init_script(binary),
    }
}

pub(crate) fn install(shell: Shell, binary: &str) -> Result<InstallReport> {
    let script = managed_script(shell, binary);
    let path = integration_path(shell)?;

    match shell {
        Shell::Fish => {
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
        Shell::Fish => uninstall_fish(&path)?,
        Shell::Bash | Shell::Zsh => uninstall_posix(&path)?,
    };

    Ok(UninstallReport {
        shell,
        reload_command: uninstall_reload_command(shell),
        path,
        changed,
        removed_file: shell == Shell::Fish && changed,
    })
}

fn integration_path(shell: Shell) -> Result<PathBuf> {
    match shell {
        Shell::Fish => {
            let config_home = env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
                .context("error: could not find $XDG_CONFIG_HOME or $HOME for fish integration")?;
            Ok(config_home.join("fish/conf.d/elio.fish"))
        }
        Shell::Bash => Ok(home_dir()?.join(".bashrc")),
        Shell::Zsh => Ok(zsh_config_dir()?.join(".zshrc")),
    }
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
    }
}

fn uninstall_reload_command(shell: Shell) -> String {
    match shell {
        Shell::Bash => "unset -f elio".to_string(),
        Shell::Zsh => "unfunction elio 2>/dev/null || true".to_string(),
        Shell::Fish => "functions --erase elio".to_string(),
    }
}

fn managed_script(shell: Shell, binary: &str) -> String {
    format!(
        "{MANAGED_START}\n{}\n{MANAGED_END}\n",
        init_script(shell, binary).trim_end()
    )
}

fn upsert_managed_block(existing: &str, block: &str) -> Result<String> {
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

fn uninstall_fish(path: &Path) -> Result<bool> {
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

fn write_text_atomic(path: &Path, contents: &str) -> Result<()> {
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

fn resolve_write_path(path: &Path) -> Result<PathBuf> {
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

fn remove_managed_blocks(existing: &str) -> Result<Option<String>> {
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

fn posix_init_script(executable: &str) -> String {
    format!(
        r#"elio() {{
    case "${{1-}}" in
        shell|-*)
            {executable} "$@"
            return $?
            ;;
    esac

    local tmp cwd status_code
    tmp="$(mktemp -t "elio-cwd.XXXXXX")" || return
    {executable} --cwd-file "$tmp" "$@"
    status_code=$?

    if [ -s "$tmp" ]; then
        cwd="$(cat -- "$tmp")"
        rm -f -- "$tmp"
        if [ -n "$cwd" ] && [ "$cwd" != "$PWD" ] && [ -d "$cwd" ]; then
            cd -- "$cwd" || return $?
        fi
    else
        rm -f -- "$tmp"
    fi

    return "$status_code"
}}
"#
    )
}

fn fish_init_script(executable: &str) -> String {
    format!(
        r#"function elio
    switch "$argv[1]"
        case shell '-*'
            {executable} $argv
            return $status
    end

    set -l tmp (mktemp -t "elio-cwd.XXXXXX")
    or return

    {executable} --cwd-file "$tmp" $argv
    set -l status_code $status

    if test -s "$tmp"
        set -l cwd (string collect < "$tmp")
        rm -f "$tmp"
        if test -n "$cwd"; and test "$cwd" != "$PWD"; and test -d "$cwd"
            cd "$cwd"; or return $status
        end
    else
        rm -f "$tmp"
    end

    return $status_code
end
"#
    )
}

fn shell_quote(path: &Path) -> String {
    let value = path.to_string_lossy();
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests;
