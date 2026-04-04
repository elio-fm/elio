use super::{ImageProtocol, TerminalIdentity};
use std::{env, fs, path::Path};

pub(super) fn pdf_preview_tools_available() -> bool {
    command_exists("pdfinfo") && command_exists("pdftocairo")
}

pub(in crate::app) fn detect_terminal_identity() -> TerminalIdentity {
    let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
    let term_program = env::var("TERM_PROGRAM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let kitty_window_id = env::var_os("KITTY_WINDOW_ID").is_some();

    if kitty_window_id || term.contains("xterm-kitty") || term_program == "kitty" {
        TerminalIdentity::Kitty
    } else if term.contains("ghostty") || term_program == "ghostty" {
        TerminalIdentity::Ghostty
    } else if term.contains("wezterm") || term_program == "wezterm" {
        TerminalIdentity::WezTerm
    } else if term_program.contains("warp") || env::var_os("WARP_SESSION_ID").is_some() {
        TerminalIdentity::Warp
    } else if term.contains("alacritty")
        || term_program.contains("alacritty")
        || env::var_os("ALACRITTY_SOCKET").is_some()
    {
        TerminalIdentity::Alacritty
    } else {
        TerminalIdentity::Other
    }
}

pub(in crate::app) fn select_image_protocol(
    identity: TerminalIdentity,
    image_previews_override: bool,
) -> ImageProtocol {
    match identity {
        TerminalIdentity::Kitty => ImageProtocol::KittyGraphics,
        TerminalIdentity::Ghostty => ImageProtocol::KittyGraphics,
        TerminalIdentity::Warp => ImageProtocol::KittyGraphics,
        TerminalIdentity::WezTerm => ImageProtocol::ItermInline,
        // ELIO_IMAGE_PREVIEWS=1 force-enables KittyGraphics on unrecognised terminals
        // for testing. Alacritty is excluded — it does not support image protocols.
        TerminalIdentity::Other if image_previews_override => ImageProtocol::KittyGraphics,
        TerminalIdentity::Alacritty | TerminalIdentity::Other => ImageProtocol::None,
    }
}

pub(in crate::app) fn command_exists(program: &str) -> bool {
    if program.is_empty() {
        return false;
    }

    let program_path = Path::new(program);
    if program_path.components().count() > 1 {
        return executable_file_exists(program_path);
    }

    env::var_os("PATH").is_some_and(|paths| {
        env::split_paths(&paths).any(|dir| executable_file_exists(&dir.join(program)))
    })
}

fn executable_file_exists(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
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

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-inline-image-{label}-{unique}"))
    }

    #[test]
    fn select_image_protocol_kitty_always_enabled() {
        assert_eq!(
            select_image_protocol(TerminalIdentity::Kitty, false),
            ImageProtocol::KittyGraphics
        );
        assert_eq!(
            select_image_protocol(TerminalIdentity::Kitty, true),
            ImageProtocol::KittyGraphics
        );
    }

    #[test]
    fn select_image_protocol_ghostty_always_enabled() {
        assert_eq!(
            select_image_protocol(TerminalIdentity::Ghostty, false),
            ImageProtocol::KittyGraphics
        );
        assert_eq!(
            select_image_protocol(TerminalIdentity::Ghostty, true),
            ImageProtocol::KittyGraphics
        );
    }

    #[test]
    fn select_image_protocol_wezterm_always_enabled() {
        assert_eq!(
            select_image_protocol(TerminalIdentity::WezTerm, false),
            ImageProtocol::ItermInline
        );
        assert_eq!(
            select_image_protocol(TerminalIdentity::WezTerm, true),
            ImageProtocol::ItermInline
        );
    }

    #[test]
    fn select_image_protocol_warp_always_enabled() {
        assert_eq!(
            select_image_protocol(TerminalIdentity::Warp, false),
            ImageProtocol::KittyGraphics
        );
        assert_eq!(
            select_image_protocol(TerminalIdentity::Warp, true),
            ImageProtocol::KittyGraphics
        );
    }

    #[test]
    fn select_image_protocol_alacritty_disabled_and_other_override_enabled() {
        assert_eq!(
            select_image_protocol(TerminalIdentity::Alacritty, true),
            ImageProtocol::None
        );
        assert_eq!(
            select_image_protocol(TerminalIdentity::Other, false),
            ImageProtocol::None
        );
        assert_eq!(
            select_image_protocol(TerminalIdentity::Other, true),
            ImageProtocol::KittyGraphics
        );
    }

    #[cfg(unix)]
    #[test]
    fn command_exists_checks_direct_executable_paths_without_shelling_out() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_root("command-exists-direct-path");
        fs::create_dir_all(&root).expect("failed to create temp root");

        let executable = root.join("demo-tool");
        fs::write(&executable, b"#!/bin/sh\nexit 0\n").expect("failed to write test executable");

        let mut permissions = fs::metadata(&executable)
            .expect("test executable metadata should exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).expect("failed to mark test executable");

        assert!(command_exists(
            executable.to_str().expect("path should be valid utf-8")
        ));

        let not_executable = root.join("demo-data");
        fs::write(&not_executable, b"plain data").expect("failed to write plain file");
        assert!(!command_exists(
            not_executable.to_str().expect("path should be valid utf-8")
        ));
    }
}
