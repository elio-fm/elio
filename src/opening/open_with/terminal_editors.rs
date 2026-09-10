use std::{fs, path::Path};

#[cfg(all(unix, not(target_os = "macos")))]
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::{collections::HashSet, env};

use super::OpenWithApplication;

#[cfg(all(unix, not(target_os = "macos")))]
pub(super) fn append_environment_editors(
    applications: &mut Vec<OpenWithApplication>,
    path: &Path,
    require_compatible_file: bool,
) {
    if require_compatible_file && !super::available_applications::is_editor_compatible(path) {
        return;
    }

    let mut insert_index = environment_editor_insert_index(applications);
    for variable in ["VISUAL", "EDITOR"] {
        let Some(application) = environment_editor_for_variable(variable, path) else {
            continue;
        };
        insert_index = promote_environment_editor(applications, application, insert_index);
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
#[cfg_attr(test, allow(dead_code))]
pub(super) fn environment_editor_for(path: &Path) -> Option<OpenWithApplication> {
    if !super::available_applications::is_editor_compatible(path) {
        return None;
    }

    ["VISUAL", "EDITOR"]
        .into_iter()
        .find_map(|variable| environment_editor_for_variable(variable, path))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn environment_editor_for_variable(
    variable: &'static str,
    path: &Path,
) -> Option<OpenWithApplication> {
    let value =
        crate::elevated_session::env_var(variable).and_then(|value| value.into_string().ok())?;
    environment_editor_from_command(variable, &value, path)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn environment_editor_from_command(
    variable: &str,
    command: &str,
    path: &Path,
) -> Option<OpenWithApplication> {
    let path_str = path.to_str()?;
    let mut tokens = tokenize_editor_command(command);
    if tokens.is_empty() {
        return None;
    }

    let program = tokens.remove(0);
    let resolved = resolve_executable(&program)?;
    let program_name = resolved
        .file_name()
        .and_then(|name| name.to_str())
        .or_else(|| {
            Path::new(&program)
                .file_name()
                .and_then(|name| name.to_str())
        })?;
    let display_name = linux_bsd_editor_name(program_name);

    tokens.push(path_str.to_string());
    Some(OpenWithApplication {
        display_name: format!("{display_name} (${variable})"),
        application_id: None,
        program,
        args: tokens,
        is_default: false,
        requires_terminal: true,
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn promote_environment_editor(
    applications: &mut Vec<OpenWithApplication>,
    application: OpenWithApplication,
    insert_index: usize,
) -> usize {
    let Some(position) = matching_application_index(&application, applications) else {
        let index = insert_index.min(applications.len());
        applications.insert(index, application);
        return index + 1;
    };

    if applications[position].is_default
        || is_environment_editor_label(&applications[position].display_name)
    {
        if !is_environment_editor_label(&applications[position].display_name) {
            applications[position].display_name = application.display_name;
        }
        return insert_index;
    }

    let mut existing = applications.remove(position);
    existing.display_name = application.display_name;
    let index = if position < insert_index {
        insert_index.saturating_sub(1)
    } else {
        insert_index
    }
    .min(applications.len());
    applications.insert(index, existing);
    index + 1
}

#[cfg(all(unix, not(target_os = "macos")))]
fn environment_editor_insert_index(applications: &[OpenWithApplication]) -> usize {
    applications
        .iter()
        .take_while(|application| application.is_default)
        .count()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn matching_application_index(
    editor: &OpenWithApplication,
    applications: &[OpenWithApplication],
) -> Option<usize> {
    let editor_program = program_key(&editor.program);
    applications
        .iter()
        .position(|application| program_key(&application.program) == editor_program)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn is_environment_editor_label(display_name: &str) -> bool {
    display_name.contains("($VISUAL)") || display_name.contains("($EDITOR)")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn program_key(program: &str) -> Option<String> {
    let path = resolve_executable(program).unwrap_or_else(|| PathBuf::from(program));
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn resolve_executable(program: &str) -> Option<PathBuf> {
    if program.is_empty() {
        return None;
    }

    let program_path = Path::new(program);
    if program_path.components().count() > 1 {
        return executable_file_exists(program_path).then(|| canonical_path(program_path));
    }

    crate::elevated_session::env_var("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|directory| directory.join(program))
            .find(|path| executable_file_exists(path))
            .map(|path| canonical_path(&path))
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn canonical_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_bsd_editor_name(program_name: &str) -> &str {
    match program_name.to_ascii_lowercase().as_str() {
        "nvim" => "Neovim",
        "vim" => "Vim",
        "vi" => "Vi",
        "helix" => "Helix",
        "micro" => "Micro",
        "nano" => "Nano",
        "emacs" => "Emacs",
        "kak" | "kakoune" => "Kakoune",
        _ => program_name,
    }
}

#[cfg(target_os = "macos")]
pub(super) fn macos_applications(path: &Path) -> Vec<OpenWithApplication> {
    let Some(path_str) = path.to_str() else {
        return Vec::new();
    };
    let mut applications = Vec::new();
    let mut seen_editors = HashSet::new();

    for variable in ["VISUAL", "EDITOR"] {
        let Some(value) = env::var_os(variable).and_then(|value| value.into_string().ok()) else {
            continue;
        };
        let Some(application) = macos_editor_from_command(variable, &value, path_str) else {
            continue;
        };
        let key = macos_editor_key(&application.program);
        if seen_editors.insert(key) {
            applications.push(application);
        }
    }

    for &(program, display_name) in COMMON_MACOS_EDITORS {
        if !seen_editors.insert(macos_editor_key(program)) || !macos_command_exists(program) {
            continue;
        }
        applications.push(OpenWithApplication {
            display_name: display_name.to_string(),
            application_id: None,
            program: program.to_string(),
            args: vec![path_str.to_string()],
            is_default: false,
            requires_terminal: true,
        });
    }

    applications
}

#[cfg(target_os = "macos")]
const COMMON_MACOS_EDITORS: &[(&str, &str)] = &[
    ("nvim", "Neovim"),
    ("vim", "Vim"),
    ("vi", "Vi"),
    ("hx", "Helix"),
    ("helix", "Helix"),
    ("micro", "Micro"),
    ("nano", "Nano"),
    ("emacs", "Emacs"),
    ("kak", "Kakoune"),
    ("kakoune", "Kakoune"),
];

#[cfg(target_os = "macos")]
fn macos_editor_from_command(
    variable: &str,
    command: &str,
    path_str: &str,
) -> Option<OpenWithApplication> {
    let mut tokens = tokenize_editor_command(command);
    if tokens.is_empty() {
        return None;
    }

    let program = tokens.remove(0);
    if !macos_command_exists(&program) {
        return None;
    }

    let program_name = Path::new(&program)
        .file_name()
        .and_then(|name| name.to_str())?
        .to_ascii_lowercase();
    let display_name = macos_editor_name(&program_name)?;

    tokens.push(path_str.to_string());
    Some(OpenWithApplication {
        display_name: format!("{display_name} (${variable})"),
        application_id: None,
        program,
        args: tokens,
        is_default: false,
        requires_terminal: true,
    })
}

#[cfg(target_os = "macos")]
fn macos_editor_name(program_name: &str) -> Option<&'static str> {
    match program_name {
        "nvim" => Some("Neovim"),
        "vim" => Some("Vim"),
        "vi" => Some("Vi"),
        "hx" | "helix" => Some("Helix"),
        "micro" => Some("Micro"),
        "nano" => Some("Nano"),
        "emacs" => Some("Emacs"),
        "kak" | "kakoune" => Some("Kakoune"),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn macos_editor_key(program: &str) -> String {
    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program)
        .to_ascii_lowercase()
}

#[cfg(target_os = "macos")]
fn macos_command_exists(program: &str) -> bool {
    if program.is_empty() {
        return false;
    }

    let program_path = Path::new(program);
    if program_path.components().count() > 1 {
        return executable_file_exists(program_path);
    }

    env::var_os("PATH").is_some_and(|paths| {
        env::split_paths(&paths).any(|directory| executable_file_exists(&directory.join(program)))
    })
}

fn executable_file_exists(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

fn tokenize_editor_command(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = command.chars().peekable();

    while let Some(character) = chars.next() {
        match character {
            '"' => in_quotes = !in_quotes,
            '\\' => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(character),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

#[cfg(all(test, unix, not(target_os = "macos")))]
#[path = "tests/terminal_editors.rs"]
mod tests;
