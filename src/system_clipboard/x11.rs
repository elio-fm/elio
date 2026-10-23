use super::{ClipboardError, FileClipboard, dispatcher::platform_command, freedesktop};
use std::path::PathBuf;

pub(super) fn read_files() -> Option<FileClipboard> {
    let types = targets()?;
    let refs = types.iter().map(String::as_str).collect::<Vec<_>>();
    freedesktop::read_file_clipboard(read_target, &refs)
}

pub(super) fn read_text() -> Option<String> {
    let types = targets()?;
    let refs = types.iter().map(String::as_str).collect::<Vec<_>>();
    freedesktop::read_text(read_target, &refs)
}

pub(super) fn write_files(paths: &[PathBuf]) -> Result<(), ClipboardError> {
    #[cfg(test)]
    if let Some(tool) = std::env::var_os("ELIO_TEST_ITEM_CLIPBOARD_TOOL") {
        return platform_command(&tool.to_string_lossy(), &[], &freedesktop::uri_list(paths));
    }
    platform_command(
        "xclip",
        &[
            "-selection",
            "clipboard",
            "-t",
            "text/uri-list",
            "-in",
            "-silent",
        ],
        &freedesktop::uri_list(paths),
    )
}

pub(super) fn write_text(text: &str) -> Result<(), ClipboardError> {
    #[cfg(test)]
    if let Some(tool) = std::env::var_os("ELIO_TEST_CLIPBOARD_TOOL") {
        return platform_command(&tool.to_string_lossy(), &[], text);
    }
    platform_command("xclip", &["-selection", "clipboard", "-in"], text)
        .or_else(|_| platform_command("xsel", &["--clipboard", "--input"], text))
}

fn targets() -> Option<Vec<String>> {
    let output = command("xclip", &["-selection", "clipboard", "-t", "TARGETS", "-o"])?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .map(str::to_owned)
            .collect()
    })
}

fn read_target(mime: &str) -> Option<Vec<u8>> {
    let output = command("xclip", &["-selection", "clipboard", "-t", mime, "-o"])?;
    output.status.success().then_some(output.stdout)
}

fn command(program: &str, args: &[&str]) -> Option<std::process::Output> {
    std::process::Command::new(program).args(args).output().ok()
}
