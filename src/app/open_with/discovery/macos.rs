// This module is only compiled on macOS (gated in discovery/mod.rs).
//
// App discovery uses the Launch Services C API (LSCopyApplicationURLsForURL
// from CoreServices.framework) — the same API that powers Finder's "Open With"
// menu.  This is the canonical, battle-tested path: it works on every macOS
// version, covers all registered handlers, and matches what Finder shows.
//
// NSWorkspace.urlsForApplicationsToOpenURL (macOS 12+) was tried first but
// returned empty results in practice even when Finder showed handlers; the
// lower-level LS API is more reliable.
//
// Launch flow
// ───────────
// Each OpenWithApp produced here uses `program = "open"` with
// `args = ["-a", "/path/to/App.app", "/path/to/file"]`.  Using the `open`
// command lets macOS handle sandbox entitlements, Rosetta translation, and
// document handoff automatically.

use std::{
    collections::HashSet,
    env,
    ffi::{CStr, c_void},
    fs,
    path::Path,
};

use objc2_foundation::{NSBundle, NSFileManager, NSString, NSURL};

use super::super::super::state::OpenWithApp;
use super::super::path_is_text_like;
use super::exec::tokenize_exec;

// ── CoreServices / CoreFoundation C types and functions ───────────────────────

/// Opaque CF types — represented as `*const c_void` for toll-free bridge casts.
type CFTypeRef = *const c_void;
type CFURLRef = *const c_void;
type CFArrayRef = *const c_void;
type CFStringRef = *const c_void;
type CFIndex = isize;
type CFStringEncoding = u32;

const LS_ROLES_ALL: u32 = 0xFFFF_FFFF;
const LS_ROLES_VIEWER: u32 = 1 << 1;
const LS_ROLES_EDITOR: u32 = 1 << 2;
const CF_URL_POSIX_PATH_STYLE: CFIndex = 0;
const CF_STRING_ENCODING_UTF8: CFStringEncoding = 0x0800_0100;

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    /// Returns all application URLs registered to handle `url`, or NULL if none.
    /// Available since macOS 10.3.  Caller must CFRelease the result.
    fn LSCopyApplicationURLsForURL(url: CFURLRef, role_mask: u32) -> CFArrayRef;
    /// Returns the default application URL for `url`, or NULL.
    /// Available since macOS 10.10.  Caller must CFRelease the result.
    fn LSCopyDefaultApplicationURLForURL(
        url: CFURLRef,
        role_mask: u32,
        error: *mut CFTypeRef,
    ) -> CFURLRef;
    /// Returns bundle identifiers for every application registered to handle
    /// `content_type` for the given role mask. Caller must CFRelease the result.
    fn LSCopyAllRoleHandlersForContentType(content_type: CFStringRef, role_mask: u32)
    -> CFArrayRef;
    /// Returns the default bundle identifier for `content_type`, or NULL.
    fn LSCopyDefaultRoleHandlerForContentType(
        content_type: CFStringRef,
        role_mask: u32,
    ) -> CFStringRef;
    /// Resolves a bundle identifier into one or more application bundle URLs.
    /// Returns `0` on success and writes a retained CFArrayRef into `out_app_urls`.
    fn LSCopyApplicationURLsForBundleIdentifier(
        bundle_id: CFStringRef,
        out_app_urls: *mut CFArrayRef,
    ) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFArrayGetCount(array: CFArrayRef) -> CFIndex;
    fn CFArrayGetValueAtIndex(array: CFArrayRef, idx: CFIndex) -> *const c_void;
    fn CFURLCopyFileSystemPath(url: CFURLRef, path_style: CFIndex) -> CFStringRef;
    fn CFStringGetLength(s: CFStringRef) -> CFIndex;
    fn CFStringGetMaximumSizeForEncoding(len: CFIndex, enc: CFStringEncoding) -> CFIndex;
    fn CFStringGetCString(
        s: CFStringRef,
        buf: *mut i8,
        buf_size: CFIndex,
        enc: CFStringEncoding,
    ) -> bool;
    fn CFRelease(cf: CFTypeRef);
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub(super) fn discover_via_nsworkspace(path: &Path) -> Vec<OpenWithApp> {
    let Some(path_str) = path.to_str() else {
        return vec![];
    };

    let mut apps = discover_file_url_handlers(path_str);
    if path_is_text_like(path) {
        merge_unique_apps(&mut apps, discover_generic_editor_apps(path_str, path));
        merge_unique_apps(&mut apps, discover_terminal_editor_apps(path_str));
    }
    sort_open_with_apps(&mut apps);
    apps
}

// ── Core discovery ────────────────────────────────────────────────────────────

fn discover_file_url_handlers(path_str: &str) -> Vec<OpenWithApp> {
    let ns_path = NSString::from_str(path_str);
    let file_url = NSURL::fileURLWithPath(&ns_path);

    // Toll-free bridge: Retained<NSURL> → CFURLRef (same object in memory).
    let cf_file_url: CFURLRef = (&*file_url) as *const NSURL as *const c_void;

    // Query Launch Services for every application that can open this URL.
    // LSCopyApplicationURLsForURL returns NULL when no app is found.
    let apps_cf: CFArrayRef = unsafe { LSCopyApplicationURLsForURL(cf_file_url, LS_ROLES_ALL) };
    if apps_cf.is_null() {
        return vec![];
    }

    // Determine the default app so we can set is_default on the right entry.
    let default_path: Option<String> = {
        let def_cf: CFURLRef = unsafe {
            LSCopyDefaultApplicationURLForURL(cf_file_url, LS_ROLES_ALL, std::ptr::null_mut())
        };
        if def_cf.is_null() {
            None
        } else {
            let p = cf_url_to_path(def_cf);
            unsafe { CFRelease(def_cf) };
            p
        }
    };

    let file_manager = NSFileManager::defaultManager();
    let count = unsafe { CFArrayGetCount(apps_cf) };
    let mut result: Vec<OpenWithApp> = Vec::with_capacity(count as usize);

    for i in 0..count {
        let app_cf_url: CFURLRef = unsafe { CFArrayGetValueAtIndex(apps_cf, i) };

        let Some(app_path_str) = cf_url_to_path(app_cf_url) else {
            continue;
        };

        // Finder-style display name (localised, ".app" suffix stripped by the OS).
        let app_path_ns = NSString::from_str(&app_path_str);
        let display_name = file_manager.displayNameAtPath(&app_path_ns).to_string();

        // Bundle identifier (com.apple.TextEdit etc.) for the desktop_id field.
        let bundle_id: Option<String> = {
            // Toll-free bridge: CFURLRef → &NSURL (safe, same allocation).
            let ns_app_url: &NSURL = unsafe { &*(app_cf_url as *const NSURL) };
            NSBundle::bundleWithURL(ns_app_url)
                .and_then(|b| b.bundleIdentifier())
                .map(|id| id.to_string())
        };

        let is_default = default_path.as_deref() == Some(&app_path_str);

        result.push(OpenWithApp {
            display_name,
            desktop_id: bundle_id,
            // Launch via `open -a App.app file` so macOS handles sandboxing,
            // Rosetta translation, and document handoff automatically.
            program: "open".to_string(),
            args: vec!["-a".to_string(), app_path_str, path_str.to_string()],
            is_default,
            // LSCopyApplicationURLsForURL only returns GUI app bundles.
            requires_terminal: false,
        });
    }

    unsafe { CFRelease(apps_cf) };

    // Default first, then alphabetically by display name (case-insensitive).
    result.sort_unstable_by(|a, b| {
        b.is_default.cmp(&a.is_default).then_with(|| {
            a.display_name
                .to_ascii_lowercase()
                .cmp(&b.display_name.to_ascii_lowercase())
        })
    });

    result
}

fn discover_generic_editor_apps(path_str: &str, path: &Path) -> Vec<OpenWithApp> {
    let file_manager = NSFileManager::defaultManager();
    let mut result = Vec::new();

    for content_type in generic_editor_content_types(path) {
        let role_mask = LS_ROLES_VIEWER | LS_ROLES_EDITOR;
        let default_bundle_id = default_role_handler_for_content_type(content_type, role_mask);
        let mut bundle_ids = role_handlers_for_content_type(content_type, role_mask);

        if let Some(default_bundle_id) = default_bundle_id.as_ref()
            && !bundle_ids
                .iter()
                .any(|bundle_id| bundle_id == default_bundle_id)
        {
            bundle_ids.insert(0, default_bundle_id.clone());
        }

        for bundle_id in bundle_ids {
            let app_urls = application_urls_for_bundle_identifier(&bundle_id);
            let display_name = app_urls
                .iter()
                .find_map(|app_path| {
                    let app_path_ns = NSString::from_str(app_path);
                    let display_name = file_manager.displayNameAtPath(&app_path_ns).to_string();
                    (!display_name.is_empty()).then_some(display_name)
                })
                .unwrap_or_else(|| bundle_id.clone());

            result.push(OpenWithApp {
                display_name,
                desktop_id: Some(bundle_id.clone()),
                program: "open".to_string(),
                args: vec!["-b".to_string(), bundle_id.clone(), path_str.to_string()],
                is_default: default_bundle_id.as_deref() == Some(bundle_id.as_str()),
                requires_terminal: false,
            });
        }
    }

    result
}

fn discover_terminal_editor_apps(path_str: &str) -> Vec<OpenWithApp> {
    let mut result = Vec::new();
    let mut seen_editors = HashSet::new();

    for var in ["VISUAL", "EDITOR"] {
        let Some(value) = env::var_os(var).and_then(|value| value.into_string().ok()) else {
            continue;
        };
        let Some(app) = terminal_editor_app_from_command(&value, path_str) else {
            continue;
        };
        let key = app.display_name.to_ascii_lowercase();
        if seen_editors.insert(key) {
            result.push(app);
        }
    }

    for &(program, display_name) in COMMON_TERMINAL_EDITORS {
        if !seen_editors.insert(display_name.to_ascii_lowercase()) {
            continue;
        }
        if !command_exists(program) {
            continue;
        }
        result.push(OpenWithApp {
            display_name: display_name.to_string(),
            desktop_id: None,
            program: program.to_string(),
            args: vec![path_str.to_string()],
            is_default: false,
            requires_terminal: true,
        });
    }

    result
}

// ── Helpers ───────────────────────────────────────────────────────────────────

const COMMON_TERMINAL_EDITORS: &[(&str, &str)] = &[
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

fn generic_editor_content_types(path: &Path) -> &'static [&'static str] {
    use crate::core::EntryKind;
    use crate::file_info::PreviewKind;

    match crate::file_info::inspect_path(path, EntryKind::File)
        .preview
        .kind
    {
        PreviewKind::Source => &["public.source-code", "public.plain-text"],
        PreviewKind::Markdown => &["net.daringfireball.markdown", "public.plain-text"],
        PreviewKind::PlainText | PreviewKind::Csv => &["public.plain-text"],
        _ => &[],
    }
}

fn role_handlers_for_content_type(content_type: &str, role_mask: u32) -> Vec<String> {
    let ns_content_type = NSString::from_str(content_type);
    let cf_content_type = (&*ns_content_type) as *const NSString as CFStringRef;
    let handlers = unsafe { LSCopyAllRoleHandlersForContentType(cf_content_type, role_mask) };
    let values = cf_array_to_strings(handlers);
    if !handlers.is_null() {
        unsafe { CFRelease(handlers) };
    }
    values
}

fn default_role_handler_for_content_type(content_type: &str, role_mask: u32) -> Option<String> {
    let ns_content_type = NSString::from_str(content_type);
    let cf_content_type = (&*ns_content_type) as *const NSString as CFStringRef;
    let handler = unsafe { LSCopyDefaultRoleHandlerForContentType(cf_content_type, role_mask) };
    if handler.is_null() {
        return None;
    }
    let value = cf_string_to_string(handler);
    unsafe { CFRelease(handler) };
    value
}

fn application_urls_for_bundle_identifier(bundle_id: &str) -> Vec<String> {
    let ns_bundle_id = NSString::from_str(bundle_id);
    let cf_bundle_id = (&*ns_bundle_id) as *const NSString as CFStringRef;
    let mut out_urls: CFArrayRef = std::ptr::null();
    let status = unsafe { LSCopyApplicationURLsForBundleIdentifier(cf_bundle_id, &mut out_urls) };
    if status != 0 || out_urls.is_null() {
        return Vec::new();
    }

    let values = cf_array_to_paths(out_urls);
    unsafe { CFRelease(out_urls) };
    values
}

fn cf_array_to_strings(array: CFArrayRef) -> Vec<String> {
    if array.is_null() {
        return Vec::new();
    }

    let count = unsafe { CFArrayGetCount(array) };
    let mut values = Vec::with_capacity(count as usize);
    for i in 0..count {
        let value = unsafe { CFArrayGetValueAtIndex(array, i) } as CFStringRef;
        if let Some(value) = cf_string_to_string(value) {
            values.push(value);
        }
    }
    values
}

fn cf_array_to_paths(array: CFArrayRef) -> Vec<String> {
    if array.is_null() {
        return Vec::new();
    }

    let count = unsafe { CFArrayGetCount(array) };
    let mut values = Vec::with_capacity(count as usize);
    for i in 0..count {
        let value = unsafe { CFArrayGetValueAtIndex(array, i) } as CFURLRef;
        if let Some(path) = cf_url_to_path(value) {
            values.push(path);
        }
    }
    values
}

/// Converts a CFURLRef to its POSIX file system path as a Rust String.
/// Returns None if the URL is null or the path cannot be extracted.
fn cf_url_to_path(url: CFURLRef) -> Option<String> {
    if url.is_null() {
        return None;
    }
    let cf_str: CFStringRef = unsafe { CFURLCopyFileSystemPath(url, CF_URL_POSIX_PATH_STYLE) };
    if cf_str.is_null() {
        return None;
    }
    let len = unsafe { CFStringGetLength(cf_str) };
    // +1 for the null terminator.
    let max_size = unsafe { CFStringGetMaximumSizeForEncoding(len, CF_STRING_ENCODING_UTF8) } + 1;
    let mut buf: Vec<i8> = vec![0; max_size as usize];
    let ok =
        unsafe { CFStringGetCString(cf_str, buf.as_mut_ptr(), max_size, CF_STRING_ENCODING_UTF8) };
    unsafe { CFRelease(cf_str) };
    if !ok {
        return None;
    }
    unsafe { CStr::from_ptr(buf.as_ptr()) }
        .to_str()
        .ok()
        .map(str::to_string)
}

fn cf_string_to_string(value: CFStringRef) -> Option<String> {
    if value.is_null() {
        return None;
    }

    let len = unsafe { CFStringGetLength(value) };
    let max_size = unsafe { CFStringGetMaximumSizeForEncoding(len, CF_STRING_ENCODING_UTF8) } + 1;
    let mut buf: Vec<i8> = vec![0; max_size as usize];
    let ok =
        unsafe { CFStringGetCString(value, buf.as_mut_ptr(), max_size, CF_STRING_ENCODING_UTF8) };
    if !ok {
        return None;
    }

    unsafe { CStr::from_ptr(buf.as_ptr()) }
        .to_str()
        .ok()
        .map(str::to_string)
}

fn merge_unique_apps(target: &mut Vec<OpenWithApp>, apps: Vec<OpenWithApp>) {
    let mut seen = target
        .iter()
        .map(open_with_app_identity_key)
        .collect::<HashSet<_>>();
    for app in apps {
        let key = open_with_app_identity_key(&app);
        if seen.insert(key) {
            target.push(app);
        }
    }
}

fn sort_open_with_apps(apps: &mut [OpenWithApp]) {
    apps.sort_unstable_by(|a, b| {
        b.is_default
            .cmp(&a.is_default)
            .then_with(|| a.requires_terminal.cmp(&b.requires_terminal))
            .then_with(|| {
                a.display_name
                    .to_ascii_lowercase()
                    .cmp(&b.display_name.to_ascii_lowercase())
            })
    });
}

fn open_with_app_identity_key(app: &OpenWithApp) -> String {
    if let Some(desktop_id) = app.desktop_id.as_ref() {
        return format!("bundle:{desktop_id}");
    }
    format!(
        "program:{}:{}",
        app.program,
        if app.requires_terminal {
            "terminal"
        } else {
            "gui"
        }
    )
}

fn terminal_editor_app_from_command(command: &str, path_str: &str) -> Option<OpenWithApp> {
    let mut tokens = tokenize_exec(command);
    if tokens.is_empty() {
        return None;
    }

    let program = tokens.remove(0);
    if !command_exists(&program) {
        return None;
    }

    let program_name = Path::new(&program)
        .file_name()
        .and_then(|name| name.to_str())?
        .to_ascii_lowercase();
    let display_name = terminal_editor_display_name(&program_name)?;

    tokens.push(path_str.to_string());
    Some(OpenWithApp {
        display_name: display_name.to_string(),
        desktop_id: None,
        program,
        args: tokens,
        is_default: false,
        requires_terminal: true,
    })
}

fn terminal_editor_display_name(program_name: &str) -> Option<&'static str> {
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

fn command_exists(program: &str) -> bool {
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

    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── discover_via_nsworkspace ──────────────────────────────────────────────

    #[test]
    fn discover_returns_apps_for_plain_text_file() {
        // Every macOS system has at least one app registered for .txt
        // (TextEdit ships with the OS and handles plain text).
        let tmp = std::env::temp_dir().join("elio-macos-open-with-test.txt");
        std::fs::write(&tmp, "hello").expect("write temp file");

        let apps = discover_via_nsworkspace(&tmp);
        let _ = std::fs::remove_file(&tmp);

        assert!(
            !apps.is_empty(),
            "expected at least one app for a .txt file on macOS; got none"
        );

        // At most one entry should carry is_default=true.
        let defaults: Vec<_> = apps.iter().filter(|a| a.is_default).collect();
        assert!(
            defaults.len() <= 1,
            "at most one app may have is_default=true; got {}",
            defaults.len()
        );

        // Every entry must use the `open -a` launch convention.
        for app in &apps {
            assert_eq!(app.program, "open");
            assert_eq!(app.args.first().map(String::as_str), Some("-a"));
            assert!(!app.display_name.is_empty());
            assert!(!app.requires_terminal);
        }
    }

    #[test]
    fn default_app_is_sorted_first_when_present() {
        let tmp = std::env::temp_dir().join("elio-macos-sort-test.txt");
        std::fs::write(&tmp, "hello").expect("write temp file");
        let apps = discover_via_nsworkspace(&tmp);
        let _ = std::fs::remove_file(&tmp);

        if apps.iter().any(|a| a.is_default) {
            assert!(
                apps[0].is_default,
                "default app must appear first in the list"
            );
        }
    }

    #[test]
    fn discover_returns_empty_for_non_utf8_path() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let non_utf8 = OsStr::from_bytes(b"/tmp/\xff\xfe.txt");
        let apps = discover_via_nsworkspace(Path::new(non_utf8));
        assert!(
            apps.is_empty(),
            "expected empty vec for non-UTF-8 path, got {apps:?}"
        );
    }

    // ── cf_url_to_path ────────────────────────────────────────────────────────

    #[test]
    fn cf_url_to_path_returns_none_for_null() {
        assert!(cf_url_to_path(std::ptr::null()).is_none());
    }

    #[test]
    fn cf_url_to_path_round_trips_via_nsurl() {
        // Build a NSURL for a known path and verify the round-trip through CF.
        let ns_path = NSString::from_str("/Applications");
        let ns_url = NSURL::fileURLWithPath(&ns_path);
        let cf_url: CFURLRef = (&*ns_url) as *const NSURL as *const c_void;

        let result = cf_url_to_path(cf_url);
        assert_eq!(result.as_deref(), Some("/Applications"));
    }
}
