use super::FileOperationsState;
use anyhow::{Result, anyhow};
use base64::Engine as _;
use std::{
    env,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Clone, Debug)]
enum CopyValue {
    Text(String),
    Items(Vec<PathBuf>),
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
const GNOME_COPIED_FILES_MIME: &str = "x-special/gnome-copied-files";

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
const URI_LIST_MIME: &str = "text/uri-list";

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
const PLAIN_TEXT_MIME: &str = "text/plain;charset=utf-8";

#[derive(Clone, Debug)]
pub(crate) struct CopyOverlayRow {
    pub(crate) shortcut: char,
    pub(crate) label: String,
    pub(crate) status_label: String,
    value: CopyValue,
}

#[derive(Clone, Debug)]
pub(crate) struct CopyOverlay {
    pub(crate) title: String,
    pub(crate) rows: Vec<CopyOverlayRow>,
}

impl FileOperationsState {
    pub(crate) fn copy_items_to_system_clipboard(&mut self, paths: &[PathBuf]) -> String {
        match write_items_to_system_clipboard(paths) {
            Ok(()) => format!(
                "Copied {} {}",
                paths.len(),
                if paths.len() == 1 { "item" } else { "items" }
            ),
            Err(error) => item_clipboard_status_message(&error),
        }
    }

    pub(crate) fn copy_overlay(&self) -> Option<&CopyOverlay> {
        self.copy.as_ref()
    }

    pub(crate) fn dismiss_copy(&mut self) {
        self.copy = None;
    }

    pub(crate) fn open_copy_overlay(&mut self, cwd: &Path, paths: &[PathBuf]) -> bool {
        if paths.is_empty() {
            return false;
        }
        self.copy = Some(build_copy_overlay(cwd, paths));
        true
    }

    pub(crate) fn confirm_copy_index(&mut self, index: usize) -> Option<String> {
        let (value, status_label) = self.copy.as_ref().and_then(|overlay| {
            overlay
                .rows
                .get(index)
                .map(|row| (row.value.clone(), row.status_label.clone()))
        })?;

        let is_item_copy = matches!(value, CopyValue::Items(_));
        let result = match value {
            CopyValue::Text(value) => write_text_to_system_clipboard(&value),
            CopyValue::Items(paths) => write_items_to_system_clipboard(&paths),
        };

        Some(match result {
            Ok(()) => {
                self.copy = None;
                format!("Copied {status_label}")
            }
            Err(error) if is_item_copy => item_clipboard_status_message(&error),
            Err(error) => clipboard_status_message(&error),
        })
    }
}

#[cfg(all(
    test,
    any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
mod item_clipboard_tests {
    use super::*;

    #[test]
    fn wayland_offer_includes_file_operation_and_portable_uri_targets() {
        let payloads = item_clipboard_payloads(&[PathBuf::from("/tmp/résumé #1")]);
        assert_eq!(
            payloads
                .iter()
                .map(|(mime, _)| mime.as_str())
                .collect::<Vec<_>>(),
            [
                "x-special/gnome-copied-files",
                "text/uri-list",
                "text/plain;charset=utf-8",
            ]
        );
        assert_eq!(
            String::from_utf8_lossy(&payloads[0].1),
            "copy\nfile:///tmp/r%C3%A9sum%C3%A9%20%231"
        );
        assert_eq!(
            String::from_utf8_lossy(&payloads[1].1),
            "file:///tmp/r%C3%A9sum%C3%A9%20%231"
        );
    }
}

fn clipboard_status_message(error: &anyhow::Error) -> String {
    let detail = error.to_string();
    if detail.contains("no clipboard tool succeeded") || detail.contains("os error 2") {
        "Clipboard helper not found".to_string()
    } else if detail.contains("osc52") {
        "Clipboard unavailable".to_string()
    } else {
        "Clipboard write failed".to_string()
    }
}

fn item_clipboard_status_message(error: &anyhow::Error) -> String {
    let detail = error.to_string();
    if detail.contains("no item clipboard tool succeeded") || detail.contains("os error 2") {
        "File clipboard helper not found".to_string()
    } else {
        "File clipboard unavailable".to_string()
    }
}

fn build_copy_overlay(cwd: &Path, paths: &[PathBuf]) -> CopyOverlay {
    let absolute_paths = paths
        .iter()
        .map(|path| absolute_path_for(path, cwd))
        .collect::<Vec<_>>();
    let directory_paths = absolute_paths
        .iter()
        .map(|path| directory_path_for(path))
        .collect::<Vec<_>>();
    let file_name_values = absolute_paths
        .iter()
        .map(|path| file_name_for_path(path))
        .collect::<Vec<_>>();
    let stem_values = absolute_paths
        .iter()
        .map(|path| name_without_extension_for_path(path))
        .collect::<Vec<_>>();
    let target_count = absolute_paths.len();
    let rows = vec![
        build_copy_row(
            'c',
            if target_count == 1 {
                "Copy file name"
            } else {
                "Copy file names"
            },
            if target_count == 1 {
                "file name"
            } else {
                "file names"
            },
            &file_name_values,
        ),
        build_copy_row(
            'n',
            if target_count == 1 {
                "Name without extension"
            } else {
                "Names without extension"
            },
            if target_count == 1 {
                "name without extension"
            } else {
                "names without extension"
            },
            &stem_values,
        ),
        build_copy_row(
            'p',
            if target_count == 1 {
                "File path"
            } else {
                "File paths"
            },
            if target_count == 1 {
                "file path"
            } else {
                "file paths"
            },
            &absolute_paths
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>(),
        ),
        build_copy_row(
            'd',
            if target_count == 1 {
                "Directory path"
            } else {
                "Directory paths"
            },
            if target_count == 1 {
                "directory path"
            } else {
                "directory paths"
            },
            &directory_paths
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>(),
        ),
        CopyOverlayRow {
            shortcut: 'i',
            label: "Item".to_string(),
            status_label: if target_count == 1 {
                "item".to_string()
            } else {
                "items".to_string()
            },
            value: CopyValue::Items(absolute_paths.clone()),
        },
    ];

    CopyOverlay {
        title: "Copy to clipboard".to_string(),
        rows,
    }
}

fn build_copy_row(
    shortcut: char,
    label: &str,
    status_label: &str,
    values: &[String],
) -> CopyOverlayRow {
    CopyOverlayRow {
        shortcut,
        label: label.to_string(),
        status_label: status_label.to_string(),
        value: CopyValue::Text(values.join("\n")),
    }
}

/// Publishes local file references in native desktop clipboard formats. This
/// deliberately has no OSC52 or text-path fallback: neither can represent a
/// file-manager copy operation.
fn write_items_to_system_clipboard(paths: &[PathBuf]) -> Result<()> {
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    {
        if wayland_session() {
            return crate::file_operations::wayland_file_clipboard::offer(item_clipboard_payloads(
                paths,
            ));
        }

        let (mime_type, payload) = item_clipboard_x11_payload(paths);
        let mut errors = Vec::new();
        for (program, args) in item_clipboard_command_candidates(&mime_type) {
            match run_clipboard_command(&program, &args, &payload) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(format!("{program}: {error}")),
            }
        }
        Err(anyhow!(
            "no item clipboard tool succeeded ({})",
            errors.join("; ")
        ))
    }

    #[cfg(not(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )))]
    {
        let _ = paths;
        Err(anyhow!(
            "native file clipboard is not supported on this platform"
        ))
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn item_clipboard_payloads(paths: &[PathBuf]) -> Vec<(String, Vec<u8>)> {
    let uris = item_clipboard_uris(paths);
    vec![
        (
            GNOME_COPIED_FILES_MIME.to_string(),
            gnome_copied_files_payload(&uris).into_bytes(),
        ),
        (URI_LIST_MIME.to_string(), uris.join("\r\n").into_bytes()),
        // This is an accompanying representation, not a fallback. The native
        // file-operation and URI targets above are always offered with it.
        (
            PLAIN_TEXT_MIME.to_string(),
            paths
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join("\n")
                .into_bytes(),
        ),
    ]
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn item_clipboard_uris(paths: &[PathBuf]) -> Vec<String> {
    paths.iter().map(|path| file_uri(path)).collect()
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn gnome_copied_files_payload(uris: &[String]) -> String {
    format!("copy\n{}", uris.join("\n"))
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn item_clipboard_x11_payload(paths: &[PathBuf]) -> (String, String) {
    let uris = item_clipboard_uris(paths);
    if desktop_prefers_gnome_file_clipboard() {
        (
            GNOME_COPIED_FILES_MIME.to_string(),
            gnome_copied_files_payload(&uris),
        )
    } else {
        (URI_LIST_MIME.to_string(), uris.join("\r\n"))
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn desktop_prefers_gnome_file_clipboard() -> bool {
    env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .any(|desktop| {
            matches!(
                desktop.trim().to_ascii_lowercase().as_str(),
                "gnome" | "ubuntu" | "cinnamon" | "pantheon" | "budgie"
            )
        })
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn wayland_session() -> bool {
    env::var_os("WAYLAND_DISPLAY").is_some()
        || env::var_os("XDG_SESSION_TYPE")
            .is_some_and(|value| value.to_string_lossy().eq_ignore_ascii_case("wayland"))
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn file_uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;

    let bytes = path.as_os_str().as_bytes();

    let mut uri = String::from("file://");
    for &byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'.' | b'_' | b'~') {
            uri.push(byte as char);
        } else {
            use std::fmt::Write as _;
            let _ = write!(uri, "%{byte:02X}");
        }
    }
    uri
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn item_clipboard_command_candidates(mime_type: &str) -> Vec<(String, Vec<String>)> {
    let mut commands = Vec::new();

    #[cfg(test)]
    if let Some(tool) = env::var_os("ELIO_TEST_ITEM_CLIPBOARD_TOOL") {
        commands.push((tool.to_string_lossy().into_owned(), Vec::new()));
        return commands;
    }

    let x11 = env::var_os("DISPLAY").is_some();

    if x11 {
        commands.push((
            "xclip".to_string(),
            vec![
                "-selection".to_string(),
                "clipboard".to_string(),
                "-t".to_string(),
                mime_type.to_string(),
                "-in".to_string(),
                "-silent".to_string(),
            ],
        ));
    }
    commands
}

fn absolute_path_for(path: &Path, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn file_name_for_path(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .or_else(|| {
            path.components()
                .next_back()
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| path.display().to_string())
}

fn name_without_extension_for_path(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_name_for_path(path))
}

fn directory_path_for(path: &Path) -> PathBuf {
    path.parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| path.to_path_buf())
}

fn write_text_to_system_clipboard(text: &str) -> Result<()> {
    let mut errors = Vec::new();

    if terminal_supports_osc52_clipboard() {
        match write_text_to_terminal_clipboard(text) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(format!("osc52: {error}")),
        }
    }

    for (program, args) in clipboard_command_candidates() {
        match run_clipboard_command(&program, &args, text) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(format!("{program}: {error}")),
        }
    }

    Err(anyhow!(
        "no clipboard tool succeeded ({})",
        errors.join("; ")
    ))
}

fn write_text_to_terminal_clipboard(text: &str) -> Result<()> {
    #[cfg(test)]
    if let Some(path) = env::var_os("ELIO_TEST_OSC52_CAPTURE") {
        std::fs::write(path, build_osc52_set_clipboard_sequence(text))
            .map_err(|error| anyhow!("failed to capture osc52 clipboard output: {error}"))?;
        return Ok(());
    }

    let mut stdout = io::stdout();
    if !stdout.is_terminal() {
        return Err(anyhow!("stdout is not a terminal"));
    }

    stdout
        .write_all(build_osc52_set_clipboard_sequence(text).as_bytes())
        .map_err(|error| anyhow!("failed to write osc52 clipboard escape: {error}"))?;
    stdout
        .flush()
        .map_err(|error| anyhow!("failed to flush osc52 clipboard escape: {error}"))?;
    Ok(())
}

fn build_osc52_set_clipboard_sequence(text: &str) -> String {
    let payload = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    format!("\x1b]52;c;{payload}\x1b\\")
}

fn terminal_supports_osc52_clipboard() -> bool {
    if env::var_os("ELIO_CLIPBOARD_OSC52").is_some() {
        return true;
    }

    if env::var_os("TMUX").is_some() && !tmux_accepts_application_osc52() {
        return false;
    }

    #[cfg(test)]
    if env::var_os("ELIO_TEST_OSC52_CAPTURE").is_some() {
        return true;
    }

    let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
    let term_program = env::var("TERM_PROGRAM")
        .unwrap_or_default()
        .to_ascii_lowercase();

    env::var_os("KITTY_WINDOW_ID").is_some()
        || term.contains("xterm-kitty")
        || term_program == "kitty"
        || term.contains("ghostty")
        || term_program == "ghostty"
        || term == "foot"
        || term == "foot-extra"
        || term.contains("wezterm")
        || term_program == "wezterm"
        || term_program == "iterm.app"
        || term_program.contains("warp")
        || env::var_os("WARP_SESSION_ID").is_some()
        || term.contains("alacritty")
        || term_program.contains("alacritty")
        || env::var_os("ALACRITTY_SOCKET").is_some()
        || env::var_os("VTE_VERSION").is_some()
        || env::var_os("WT_SESSION").is_some()
}

fn tmux_accepts_application_osc52() -> bool {
    #[cfg(test)]
    if let Some(value) = env::var_os("ELIO_TEST_TMUX_SET_CLIPBOARD") {
        return value.to_string_lossy().trim().eq_ignore_ascii_case("on");
    }

    Command::new("tmux")
        .args(["show-options", "-gqv", "set-clipboard"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("on"))
}

fn run_clipboard_command(program: &str, args: &[String], text: &str) -> Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| anyhow!("{error}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|error| anyhow!("failed to write stdin: {error}"))?;
    }

    let status = child
        .wait()
        .map_err(|error| anyhow!("failed to wait for command: {error}"))?;
    if status.success() {
        return Ok(());
    }

    Err(anyhow!("exited with {status}"))
}

fn clipboard_command_candidates() -> Vec<(String, Vec<String>)> {
    let mut commands = Vec::new();

    #[cfg(test)]
    if let Some(tool) = env::var_os("ELIO_TEST_CLIPBOARD_TOOL") {
        commands.push((tool.to_string_lossy().into_owned(), Vec::new()));
    }

    if cfg!(target_os = "macos") {
        commands.push(("pbcopy".to_string(), Vec::new()));
        return commands;
    }

    if cfg!(windows) {
        commands.push(("clip.exe".to_string(), Vec::new()));
        commands.push(("clip".to_string(), Vec::new()));
        return commands;
    }

    let wayland = env::var_os("WAYLAND_DISPLAY").is_some()
        || env::var_os("XDG_SESSION_TYPE")
            .is_some_and(|value| value.to_string_lossy().eq_ignore_ascii_case("wayland"));
    let x11 = env::var_os("DISPLAY").is_some();

    if wayland {
        commands.push((
            "wl-copy".to_string(),
            vec!["--type".to_string(), "text/plain;charset=utf-8".to_string()],
        ));
    }
    if x11 {
        commands.push((
            "xclip".to_string(),
            vec![
                "-selection".to_string(),
                "clipboard".to_string(),
                "-in".to_string(),
            ],
        ));
        commands.push((
            "xsel".to_string(),
            vec!["--clipboard".to_string(), "--input".to_string()],
        ));
    }
    if !wayland {
        commands.push((
            "wl-copy".to_string(),
            vec!["--type".to_string(), "text/plain;charset=utf-8".to_string()],
        ));
    }
    if !x11 {
        commands.push((
            "xclip".to_string(),
            vec![
                "-selection".to_string(),
                "clipboard".to_string(),
                "-in".to_string(),
            ],
        ));
        commands.push((
            "xsel".to_string(),
            vec!["--clipboard".to_string(), "--input".to_string()],
        ));
    }

    commands
}

impl FileOperationsState {
    pub fn copy_is_open(&self) -> bool {
        self.copy.is_some()
    }

    pub fn copy_title(&self) -> &str {
        self.copy
            .as_ref()
            .map(|overlay| overlay.title.as_str())
            .unwrap_or("")
    }

    pub fn copy_row_count(&self) -> usize {
        self.copy
            .as_ref()
            .map(|overlay| overlay.rows.len())
            .unwrap_or(0)
    }

    pub fn copy_row_label(&self, index: usize) -> &str {
        self.copy
            .as_ref()
            .and_then(|overlay| overlay.rows.get(index))
            .map(|row| row.label.as_str())
            .unwrap_or("")
    }

    pub fn copy_row_shortcut(&self, index: usize) -> Option<char> {
        self.copy
            .as_ref()
            .and_then(|overlay| overlay.rows.get(index))
            .map(|row| row.shortcut)
    }
}
