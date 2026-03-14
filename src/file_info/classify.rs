use super::{
    FileFacts, HighlightLanguage, PreviewSpec, archives::inspect_archive_name,
    extensions::inspect_extension, license::sniff_license_file_type, names::inspect_exact_name,
    types::shell_file_facts,
};
use crate::app::{EntryKind, FileClass};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;
use std::{fs::File, io::Read};

const CONFIG_SNIFF_BYTE_LIMIT: usize = 16 * 1024;
const CONFIG_SNIFF_LINE_LIMIT: usize = 80;
const CONFIG_HINT_LINE_LIMIT: usize = 10;
const STRONG_INI_THRESHOLD: u8 = 4;
const STRONG_SHELL_THRESHOLD: u8 = 4;
const SCORE_MARGIN: u8 = 2;

pub(crate) fn inspect_path(path: &Path, kind: EntryKind) -> FileFacts {
    if kind == EntryKind::Directory {
        return FileFacts {
            builtin_class: FileClass::Directory,
            specific_type_label: None,
            preview: PreviewSpec::plain_text(),
        };
    }

    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(normalize_key)
        .unwrap_or_default();
    if let Some(facts) = inspect_exact_name(&name) {
        return facts;
    }
    if let Some(facts) = inspect_archive_name(&name) {
        return facts;
    }

    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(normalize_key)
        .unwrap_or_default();
    let mut facts = inspect_extension(&ext);
    if ext.is_empty() {
        facts = sniff_extensionless_file_type(path).unwrap_or(facts);
    } else if matches!(ext.as_str(), "conf" | "cfg") {
        facts = sniff_config_file_type(path).unwrap_or(facts);
    }
    sniff_license_file_type(path, &name, &ext, facts).unwrap_or(facts)
}

/// Cached variant of [`inspect_path`] that avoids repeated file I/O for the same file version.
/// The cache is keyed on path, kind, file size, and modification time, so stale results are not
/// served when a file changes on disk.
pub(crate) fn inspect_path_cached(
    path: &Path,
    kind: EntryKind,
    size: u64,
    modified: Option<SystemTime>,
) -> FileFacts {
    #[derive(Eq, Hash, PartialEq)]
    struct CacheKey {
        path: PathBuf,
        is_dir: bool,
        size: u64,
        mtime: Option<(u64, u32)>,
    }

    fn facts_cache() -> &'static Mutex<HashMap<CacheKey, FileFacts>> {
        static CACHE: OnceLock<Mutex<HashMap<CacheKey, FileFacts>>> = OnceLock::new();
        CACHE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    let mtime = modified
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| (d.as_secs(), d.subsec_nanos()));
    let key = CacheKey {
        path: path.to_path_buf(),
        is_dir: kind == EntryKind::Directory,
        size,
        mtime,
    };

    if let Some(&facts) = facts_cache()
        .lock()
        .expect("file facts cache lock")
        .get(&key)
    {
        return facts;
    }

    let facts = inspect_path(path, kind);
    facts_cache()
        .lock()
        .expect("file facts cache lock")
        .insert(key, facts);
    facts
}

fn normalize_key(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

fn sniff_extensionless_file_type(path: &Path) -> Option<FileFacts> {
    let mut file = File::open(path).ok()?;
    let mut buffer = [0_u8; 512];
    let bytes_read = file.read(&mut buffer).ok()?;
    let prefix = &buffer[..bytes_read];
    sniff_image_type(prefix).or_else(|| sniff_shebang_script_type(prefix))
}

fn sniff_image_type(buffer: &[u8]) -> Option<FileFacts> {
    if buffer.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some(image_facts("PNG image"));
    }
    if buffer.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some(image_facts("JPEG image"));
    }
    if buffer.starts_with(b"GIF87a") || buffer.starts_with(b"GIF89a") {
        return Some(image_facts("GIF image"));
    }
    if buffer.len() >= 12 && &buffer[..4] == b"RIFF" && &buffer[8..12] == b"WEBP" {
        return Some(image_facts("WebP image"));
    }

    let text = std::str::from_utf8(buffer).ok()?;
    let trimmed = text.trim_start_matches(|ch: char| ch.is_ascii_whitespace() || ch == '\u{feff}');
    if trimmed.starts_with("<svg") || (trimmed.starts_with("<?xml") && trimmed.contains("<svg")) {
        return Some(image_facts("SVG image"));
    }

    None
}

fn image_facts(label: &'static str) -> FileFacts {
    FileFacts {
        builtin_class: FileClass::Image,
        specific_type_label: Some(label),
        preview: PreviewSpec::plain_text(),
    }
}

fn sniff_shebang_script_type(buffer: &[u8]) -> Option<FileFacts> {
    let text = std::str::from_utf8(buffer).ok()?;
    let first_line = text.lines().next()?.trim_start_matches('\u{feff}');
    let interpreter = shebang_interpreter_name(first_line)?;

    match interpreter {
        "bash" => Some(shell_file_facts(FileClass::Code, "Bash script", "bash")),
        "zsh" => Some(shell_file_facts(FileClass::Code, "Zsh script", "zsh")),
        "ksh" => Some(shell_file_facts(FileClass::Code, "KornShell script", "ksh")),
        "sh" => Some(shell_file_facts(FileClass::Code, "Shell script", "sh")),
        _ => None,
    }
}

fn shebang_interpreter_name(first_line: &str) -> Option<&str> {
    let command = first_line.strip_prefix("#!")?.trim();
    if command.is_empty() {
        return None;
    }

    let mut tokens = command.split_whitespace();
    let program = shebang_basename(tokens.next()?)?;
    if program != "env" {
        return Some(program);
    }

    tokens
        .find(|token| !token.starts_with('-'))
        .and_then(shebang_basename)
}

fn shebang_basename(token: &str) -> Option<&str> {
    Path::new(token).file_name()?.to_str()
}

fn sniff_config_file_type(path: &Path) -> Option<FileFacts> {
    let prefix = read_text_prefix(path)?;
    if let Some(hint) = detect_config_hint(&prefix) {
        return Some(hint);
    }

    let (ini_score, shell_score) = score_config_prefix(&prefix);
    if ini_score >= STRONG_INI_THRESHOLD && ini_score >= shell_score.saturating_add(SCORE_MARGIN) {
        return Some(config_file_facts(Some("ini"), HighlightLanguage::Ini));
    }
    if shell_score >= STRONG_SHELL_THRESHOLD
        && shell_score >= ini_score.saturating_add(SCORE_MARGIN)
    {
        return Some(config_file_facts(Some("shell"), HighlightLanguage::Shell));
    }

    Some(config_file_facts(None, HighlightLanguage::DirectiveConf))
}

fn read_text_prefix(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut buffer = vec![0_u8; CONFIG_SNIFF_BYTE_LIMIT];
    let bytes_read = file.read(&mut buffer).ok()?;
    if bytes_read == 0 {
        return Some(String::new());
    }
    Some(String::from_utf8_lossy(&buffer[..bytes_read]).into_owned())
}

fn detect_config_hint(prefix: &str) -> Option<FileFacts> {
    prefix
        .lines()
        .take(CONFIG_HINT_LINE_LIMIT)
        .find_map(|line| extract_mode_hint(line).and_then(config_facts_from_hint))
}

fn extract_mode_hint(line: &str) -> Option<&str> {
    extract_emacs_mode_hint(line).or_else(|| extract_vim_mode_hint(line))
}

fn extract_emacs_mode_hint(line: &str) -> Option<&str> {
    let start = line.find("-*-")?;
    let rest = line.get(start + 3..)?;
    let end = rest.find("-*-")?;
    let payload = rest.get(..end)?.trim();
    if payload.is_empty() {
        return None;
    }

    payload
        .split(';')
        .find_map(|part| {
            let trimmed = part.trim();
            trimmed
                .strip_prefix("mode:")
                .or_else(|| trimmed.strip_prefix("Mode:"))
                .map(str::trim)
        })
        .or_else(|| {
            let token = payload.split_whitespace().next()?;
            (!token.contains(':')).then_some(token)
        })
}

fn extract_vim_mode_hint(line: &str) -> Option<&str> {
    let lower = line.to_ascii_lowercase();
    for needle in ["filetype=", "syntax=", "ft="] {
        if let Some(index) = lower.find(needle) {
            let token_start = index + needle.len();
            let token = line.get(token_start..)?;
            let token_end = token
                .find(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '+')))
                .unwrap_or(token.len());
            let token = token.get(..token_end)?.trim();
            if !token.is_empty() {
                return Some(token);
            }
        }
    }
    None
}

fn config_facts_from_hint(token: &str) -> Option<FileFacts> {
    let token = normalize_key(token);
    let (language_hint, highlight_language) = match token.as_str() {
        "ini" | "dosini" => (Some("ini"), HighlightLanguage::Ini),
        "sh" | "shell" => (Some("shell"), HighlightLanguage::Shell),
        "bash" => (Some("bash"), HighlightLanguage::Shell),
        "zsh" => (Some("zsh"), HighlightLanguage::Shell),
        "ksh" => (Some("ksh"), HighlightLanguage::Shell),
        "fish" => (Some("fish"), HighlightLanguage::Shell),
        "kitty" => (Some("kitty"), HighlightLanguage::DirectiveConf),
        "mpv" => (Some("mpv"), HighlightLanguage::DirectiveConf),
        "btop" => (Some("btop"), HighlightLanguage::DirectiveConf),
        "conf" | "cfg" | "config" => (Some("config"), HighlightLanguage::DirectiveConf),
        "lua" => (Some("lua"), HighlightLanguage::Lua),
        "python" | "py" => (Some("python"), HighlightLanguage::Python),
        "nix" => (Some("nix"), HighlightLanguage::Nix),
        "cmake" => (Some("cmake"), HighlightLanguage::CMake),
        "css" => (Some("css"), HighlightLanguage::Css),
        "html" => (Some("html"), HighlightLanguage::Markup),
        "xml" | "svg" | "markup" => (Some("xml"), HighlightLanguage::Markup),
        "toml" => (Some("toml"), HighlightLanguage::Toml),
        "json" => (Some("json"), HighlightLanguage::Json),
        "jsonc" | "json5" => (Some("jsonc"), HighlightLanguage::Jsonc),
        "yaml" | "yml" => (Some("yaml"), HighlightLanguage::Yaml),
        "log" => (Some("log"), HighlightLanguage::Log),
        "desktop" => (Some("desktop"), HighlightLanguage::DesktopEntry),
        _ => {
            return HighlightLanguage::from_language_token(token.as_str())
                .map(|language| config_file_facts(None, language));
        }
    };
    Some(config_file_facts(language_hint, highlight_language))
}

fn config_file_facts(
    language_hint: Option<&'static str>,
    highlight_language: HighlightLanguage,
) -> FileFacts {
    FileFacts {
        builtin_class: FileClass::Config,
        specific_type_label: None,
        preview: PreviewSpec::source(language_hint, Some(highlight_language), None),
    }
}

fn score_config_prefix(prefix: &str) -> (u8, u8) {
    let mut ini_sections = 0_u8;
    let mut ini_assignments = 0_u8;
    let mut ini_semicolon_comments = 0_u8;
    let mut shell_expansions = 0_u8;
    let mut shell_controls = 0_u8;
    let mut shell_assignments = 0_u8;

    for line in prefix.lines().take(CONFIG_SNIFF_LINE_LIMIT) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with(';') {
            ini_semicolon_comments = ini_semicolon_comments.saturating_add(1);
            continue;
        }
        if looks_like_ini_section(trimmed) {
            ini_sections = ini_sections.saturating_add(1);
            continue;
        }
        if looks_like_ini_assignment(trimmed) {
            ini_assignments = ini_assignments.saturating_add(1);
        }
        if looks_like_shell_expansion(trimmed) {
            shell_expansions = shell_expansions.saturating_add(1);
        }
        if looks_like_shell_control(trimmed) {
            shell_controls = shell_controls.saturating_add(1);
        }
        if looks_like_shell_assignment(trimmed) {
            shell_assignments = shell_assignments.saturating_add(1);
        }
    }

    let ini_score = 4_u8.saturating_mul(ini_sections.min(1))
        + ini_assignments.min(2)
        + ini_semicolon_comments.min(2);
    let shell_score = 3_u8.saturating_mul(shell_expansions.min(1))
        + 3_u8.saturating_mul(shell_controls.min(1))
        + shell_assignments.min(2);

    (ini_score, shell_score)
}

fn looks_like_ini_section(line: &str) -> bool {
    line.starts_with('[') && line.ends_with(']') && line.len() > 2 && !line.contains('\n')
}

fn looks_like_ini_assignment(line: &str) -> bool {
    let Some((left, _right)) = line.split_once('=') else {
        return false;
    };
    let key = left.trim();
    !key.is_empty()
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

fn looks_like_shell_expansion(line: &str) -> bool {
    line.contains("${")
        || line.contains("$(")
        || line.contains("$((")
        || line.contains("`")
        || line.contains("&&")
        || line.contains("||")
        || line.contains("[[")
        || line.contains("]]")
}

fn looks_like_shell_control(line: &str) -> bool {
    line.starts_with("export ")
        || line.starts_with("if ")
        || line.starts_with("for ")
        || line.starts_with("while ")
        || line.starts_with("case ")
        || matches!(line, "then" | "do" | "done" | "fi" | "esac")
        || line.contains("; then")
        || line.contains("; do")
}

fn looks_like_shell_assignment(line: &str) -> bool {
    let Some((left, _right)) = line.split_once('=') else {
        return false;
    };
    !left.trim().is_empty()
        && !left.chars().any(char::is_whitespace)
        && left
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}
