use super::renderer::{ResolvedFont, is_private_use_character};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

pub(super) fn font_for_character(ch: char) -> Option<ResolvedFont> {
    let charset = format!("charset={:x}", ch as u32);
    let path = if is_private_use_character(ch) {
        fontconfig_match(
            &format!("Symbols Nerd Font:{charset}"),
            Some("Symbols Nerd Font"),
        )
        .or_else(|| nerd_font_fallback(&charset))
        .or_else(|| fontconfig_match(&format!(":{charset}"), None))?
    } else {
        fontconfig_match(&format!(":{charset}"), None)?
    };
    Some(ResolvedFont {
        path,
        collection_index: 0,
    })
}

fn nerd_font_fallback(charset: &str) -> Option<PathBuf> {
    let output = Command::new("fc-match")
        .arg("-s")
        .arg("-f")
        .arg("%{family}\n%{file}\n")
        .arg(format!(":{charset}"))
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let output = String::from_utf8_lossy(&output.stdout);
    let mut lines = output.lines();
    while let (Some(family), Some(path)) = (lines.next(), lines.next()) {
        if family.contains("Nerd Font") {
            let path = PathBuf::from(path);
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
}

fn fontconfig_match(query: &str, required_family: Option<&str>) -> Option<PathBuf> {
    let output = Command::new("fc-match")
        .arg("-f")
        .arg("%{family}\n%{file}\n")
        .arg(query)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let output = String::from_utf8_lossy(&output.stdout);
    let mut lines = output.lines();
    let family = lines.next()?;
    if required_family.is_some_and(|required| !family.contains(required)) {
        return None;
    }
    let path = PathBuf::from(lines.next()?);
    path.exists().then_some(path)
}
