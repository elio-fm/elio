use anyhow::{Context, Result};
use std::{
    env,
    path::{Path, PathBuf},
};

pub(crate) const SUPPORTED_SHELLS: &str = "bash, zsh, fish, nu, pwsh";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Shell {
    Bash,
    Zsh,
    Fish,
    Nu,
    Pwsh,
}

impl Shell {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "bash" => Ok(Self::Bash),
            "zsh" => Ok(Self::Zsh),
            "fish" => Ok(Self::Fish),
            "nu" | "nushell" => Ok(Self::Nu),
            "pwsh" => Ok(Self::Pwsh),
            shell => Err(format!(
                "error: unsupported shell '{shell}'

supported shells: {SUPPORTED_SHELLS}"
            )),
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
            Self::Nu => "nu",
            Self::Pwsh => "pwsh",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ShellIntegrationAction {
    Install,
    Uninstall,
}

impl ShellIntegrationAction {
    fn command(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Uninstall => "uninstall",
        }
    }

    fn active_shell_description(self) -> &'static str {
        match self {
            Self::Install => "installs integration for",
            Self::Uninstall => "removes integration from",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum ShellDetection {
    Supported(Shell),
    Unsupported(String),
    Unknown,
}

/// The shell `elio shell install`/`uninstall` targets when none is named.
pub(crate) struct DetectedShell {
    pub(crate) shell: Shell,
    /// Executable of the parent process the shell was detected from, when the
    /// platform reports it.
    pub(crate) executable: Option<PathBuf>,
}

struct ParentShell {
    detection: ShellDetection,
    executable: Option<PathBuf>,
}

pub(crate) fn detect_shell(action: ShellIntegrationAction) -> Result<DetectedShell> {
    let parent = detect_parent_shell();
    match parent.detection {
        ShellDetection::Supported(shell) => {
            if matches!(action, ShellIntegrationAction::Install) {
                ensure_detected_pwsh_path(shell, parent.executable.as_deref(), cfg!(windows))?;
            }
            Ok(DetectedShell {
                shell,
                executable: parent.executable,
            })
        }
        ShellDetection::Unsupported(shell) => Err(anyhow::anyhow!(
            unsupported_active_shell_message(action, &shell)
        )),
        ShellDetection::Unknown => Ok(DetectedShell {
            shell: detect_shell_from_environment(action)?,
            executable: None,
        }),
    }
}

/// Installing for a detected `pwsh` version-checks that exact executable. Where
/// the platform reports the parent's path (Windows) but did not for this
/// process, refuse rather than checking a possibly different `pwsh` from PATH.
pub(super) fn ensure_detected_pwsh_path(
    shell: Shell,
    executable: Option<&Path>,
    platform_reports_path: bool,
) -> Result<()> {
    if shell == Shell::Pwsh && platform_reports_path && executable.is_none() {
        anyhow::bail!(
            "error: could not determine which PowerShell launched elio\n\nWindows did not report the path of the parent pwsh.exe, so its version cannot be checked.\nRun this explicitly to install for the pwsh on your PATH:\n  elio shell install pwsh"
        );
    }
    Ok(())
}

fn detect_shell_from_environment(action: ShellIntegrationAction) -> Result<Shell> {
    let shell = env::var("SHELL").with_context(|| {
        format!(
            "error: could not detect your shell from the parent process or $SHELL\n\n{}",
            explicit_shell_guidance(action)
        )
    })?;
    let name = shell_name_from_command(&shell).unwrap_or(shell);

    Shell::parse(&name).map_err(anyhow::Error::msg)
}

#[cfg(unix)]
fn detect_parent_shell() -> ParentShell {
    ParentShell {
        detection: detect_parent_shell_from_ps(),
        executable: None,
    }
}

#[cfg(unix)]
fn detect_parent_shell_from_ps() -> ShellDetection {
    let parent_pid = unsafe { libc::getppid() }.to_string();
    let output = std::process::Command::new("ps")
        .args(["-p", &parent_pid, "-o", "comm="])
        .output()
        .ok();
    let Some(output) = output else {
        return ShellDetection::Unknown;
    };

    if !output.status.success() {
        return ShellDetection::Unknown;
    }

    let Ok(command) = String::from_utf8(output.stdout) else {
        return ShellDetection::Unknown;
    };

    detect_shell_from_command(&command)
}

#[cfg(windows)]
fn detect_parent_shell() -> ParentShell {
    match parent_process() {
        Some(parent) => ParentShell {
            detection: detect_shell_from_command(&parent.name),
            executable: process_executable(&parent),
        },
        None => ParentShell {
            detection: ShellDetection::Unknown,
            executable: None,
        },
    }
}

#[cfg(not(any(unix, windows)))]
fn detect_parent_shell() -> ParentShell {
    ParentShell {
        detection: ShellDetection::Unknown,
        executable: None,
    }
}

/// The process that launched elio. A launcher shim carrying elio's own name
/// (Scoop installs one) is stepped over so the shell behind it is reported
/// instead.
#[cfg(windows)]
fn parent_process() -> Option<ProcessInfo> {
    let processes = process_snapshot()?;
    let find = |process_id: u32| processes.iter().find(|process| process.id == process_id);

    let current = find(std::process::id())?;
    let parent = find(current.parent_id)?;
    if parent.name.eq_ignore_ascii_case(&current.name) {
        return find(parent.parent_id).cloned();
    }
    Some(parent.clone())
}

/// Full path of `process`'s executable. The path is dropped when its file name
/// no longer matches the snapshot, which means the process id was reused.
#[cfg(windows)]
fn process_executable(process: &ProcessInfo) -> Option<PathBuf> {
    let path = process_executable_path(process.id)?;
    let file_name = path.file_name()?.to_str()?;
    file_name
        .eq_ignore_ascii_case(&process.name)
        .then_some(path)
}

#[cfg(windows)]
pub(super) fn process_executable_path(process_id: u32) -> Option<PathBuf> {
    use std::{ffi::OsString, ffi::c_void, os::windows::ffi::OsStringExt};

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const MAX_LONG_PATH: usize = 32_768;

    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit_handle: i32, process_id: u32) -> *mut c_void;
        fn QueryFullProcessImageNameW(
            process: *mut c_void,
            flags: u32,
            exe_name: *mut u16,
            size: *mut u32,
        ) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    // SAFETY: the process handle is checked before use and closed exactly once.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process.is_null() {
        return None;
    }

    let mut buffer = vec![0u16; MAX_LONG_PATH];
    let mut length = buffer.len() as u32;
    // SAFETY: `length` carries the buffer's capacity in and the path length out.
    let queried =
        unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) } != 0;
    // SAFETY: `process` is a valid handle owned by this function.
    unsafe { CloseHandle(process) };

    queried.then(|| PathBuf::from(OsString::from_wide(&buffer[..length as usize])))
}

#[cfg(windows)]
#[derive(Clone)]
struct ProcessInfo {
    id: u32,
    parent_id: u32,
    name: String,
}

#[cfg(windows)]
fn process_snapshot() -> Option<Vec<ProcessInfo>> {
    use std::ffi::c_void;

    const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
    const INVALID_HANDLE_VALUE: *mut c_void = usize::MAX as *mut c_void;
    const MAX_PATH: usize = 260;

    #[repr(C)]
    struct ProcessEntry32W {
        size: u32,
        usage: u32,
        process_id: u32,
        default_heap_id: usize,
        module_id: u32,
        threads: u32,
        parent_process_id: u32,
        priority_class_base: i32,
        flags: u32,
        exe_file: [u16; MAX_PATH],
    }

    unsafe extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> *mut c_void;
        fn Process32FirstW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
        fn Process32NextW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    // SAFETY: the snapshot handle is checked before use and closed exactly once.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot.is_null() || snapshot == INVALID_HANDLE_VALUE {
        return None;
    }

    let mut entry = ProcessEntry32W {
        size: size_of::<ProcessEntry32W>() as u32,
        usage: 0,
        process_id: 0,
        default_heap_id: 0,
        module_id: 0,
        threads: 0,
        parent_process_id: 0,
        priority_class_base: 0,
        flags: 0,
        exe_file: [0; MAX_PATH],
    };
    let mut processes = Vec::new();

    // SAFETY: `entry` mirrors the Win32 PROCESSENTRY32W layout and carries its
    // own size, as the Toolhelp API requires.
    let mut has_entry = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while has_entry {
        let length = entry
            .exe_file
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(MAX_PATH);
        processes.push(ProcessInfo {
            id: entry.process_id,
            parent_id: entry.parent_process_id,
            name: String::from_utf16_lossy(&entry.exe_file[..length]),
        });
        // SAFETY: same snapshot handle and entry buffer as above.
        has_entry = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }

    // SAFETY: `snapshot` is a valid handle owned by this function.
    unsafe { CloseHandle(snapshot) };
    Some(processes)
}

pub(super) fn detect_shell_from_command(command: &str) -> ShellDetection {
    let Some(name) = shell_name_from_command(command) else {
        return ShellDetection::Unknown;
    };

    match Shell::parse(&name) {
        Ok(shell) => ShellDetection::Supported(shell),
        Err(_) if is_known_unsupported_shell(&name) => ShellDetection::Unsupported(name),
        Err(_) => ShellDetection::Unknown,
    }
}

fn is_known_unsupported_shell(name: &str) -> bool {
    matches!(
        name,
        "sh" | "dash"
            | "ash"
            | "ksh"
            | "mksh"
            | "pdksh"
            | "yash"
            | "csh"
            | "tcsh"
            | "xonsh"
            | "elvish"
            | "ion"
            | "oil"
            | "osh"
            | "powershell"
            | "cmd"
    )
}

fn unsupported_active_shell_message(action: ShellIntegrationAction, shell: &str) -> String {
    let note = if shell == "powershell" {
        "\nWindows PowerShell is not supported; use PowerShell 7.4+ (pwsh) instead."
    } else {
        ""
    };
    format!(
        "error: unsupported active shell '{shell}'\n\n`elio shell {}` {} the active shell.\nsupported shells: {SUPPORTED_SHELLS}{note}\n\n{}",
        action.command(),
        action.active_shell_description(),
        explicit_shell_guidance(action)
    )
}

fn explicit_shell_guidance(action: ShellIntegrationAction) -> String {
    let command = action.command();
    format!(
        "Run one of these explicitly if you want to target another shell:\n  elio shell {command} fish\n  elio shell {command} bash\n  elio shell {command} zsh\n  elio shell {command} nu\n  elio shell {command} pwsh"
    )
}

pub(super) fn shell_name_from_command(command: &str) -> Option<String> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }

    let first_word = command.split_whitespace().next()?;
    let without_login_prefix = first_word.strip_prefix('-').unwrap_or(first_word);
    let file_name = Path::new(without_login_prefix)
        .file_name()
        .and_then(|name| name.to_str())?;
    let name = file_name.strip_prefix('-').unwrap_or(file_name);
    Some(match strip_exe_suffix(name) {
        Some(stem) => stem.to_ascii_lowercase(),
        None => name.to_string(),
    })
}

fn strip_exe_suffix(name: &str) -> Option<&str> {
    let split = name.len().checked_sub(4)?;
    let (stem, suffix) = (name.get(..split)?, name.get(split..)?);
    (!stem.is_empty() && suffix.eq_ignore_ascii_case(".exe")).then_some(stem)
}
