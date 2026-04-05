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

use std::ffi::{CStr, c_void};
use std::path::Path;

use objc2_foundation::{NSBundle, NSFileManager, NSString, NSURL};

use super::super::super::state::OpenWithApp;

// ── CoreServices / CoreFoundation C types and functions ───────────────────────

/// Opaque CF types — represented as `*const c_void` for toll-free bridge casts.
type CFTypeRef = *const c_void;
type CFURLRef = *const c_void;
type CFArrayRef = *const c_void;
type CFStringRef = *const c_void;
type CFIndex = isize;
type CFStringEncoding = u32;

const LS_ROLES_ALL: u32 = 0xFFFF_FFFF;
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
    discover_inner(path_str)
}

// ── Core discovery ────────────────────────────────────────────────────────────

fn discover_inner(path_str: &str) -> Vec<OpenWithApp> {
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

// ── Helpers ───────────────────────────────────────────────────────────────────

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
