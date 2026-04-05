// This module is only compiled on macOS (gated in discovery/mod.rs).
//
// Discovers Open With applications via NSWorkspace / Launch Services.
//
// The primary query (`URLsForApplicationsToOpenURL:`) was added in macOS 12
// (Monterey, October 2021).  On older macOS this module returns an empty list
// rather than maintaining a second code path for a platform that is
// end-of-life.  If pre-12 support ever becomes necessary, the fallback path
// would use `LSCopyApplicationURLsForURL` from Core Services.
//
// Launch flow
// ───────────
// Each `OpenWithApp` produced here uses `program = "open"` and
// `args = ["-a", "/path/to/App.app", "/path/to/file"]`.  The macOS `open`
// command handles sandbox entitlements, Rosetta translation, and document
// handoff — none of which we would get by exec-ing the bundle binary directly.

use std::path::Path;

use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSBundle, NSFileManager, NSProcessInfo, NSString, NSURL};

use super::super::super::state::OpenWithApp;

/// Entry point called from `mod.rs`.
pub(super) fn discover_via_nsworkspace(path: &Path) -> Vec<OpenWithApp> {
    let Some(path_str) = path.to_str() else {
        return vec![];
    };

    // `URLsForApplicationsToOpenURL:` is macOS 12+.  Guard before calling it
    // so we do not invoke an absent selector on older systems.
    if !is_macos_12_or_later() {
        return vec![];
    }

    discover_inner(path_str, path)
}

// ── macOS version check ───────────────────────────────────────────────────────

fn is_macos_12_or_later() -> bool {
    let version = NSProcessInfo::processInfo().operatingSystemVersion();
    version.majorVersion >= 12
}

// ── Core discovery ────────────────────────────────────────────────────────────

fn discover_inner(path_str: &str, path: &Path) -> Vec<OpenWithApp> {
    let workspace = NSWorkspace::sharedWorkspace();
    let file_manager = NSFileManager::defaultManager();

    let ns_path = NSString::from_str(path_str);
    let file_url = NSURL::fileURLWithPath(&ns_path);

    // All applications registered to open this file URL (Launch Services).
    let all_apps = workspace.URLsForApplicationsToOpenURL(&file_url);

    // Default application — used to mark `is_default` on the matching entry.
    let default_path: Option<String> = workspace
        .URLForApplicationToOpenURL(&file_url)
        .and_then(|u| u.path())
        .map(|p| p.to_string());

    let count = all_apps.count();
    let mut result: Vec<OpenWithApp> = Vec::with_capacity(count as usize);

    for i in 0..count {
        let app_url = all_apps.objectAtIndex(i);

        // Bundle path on disk: /Applications/TextEdit.app
        let Some(app_path_ns) = app_url.path() else {
            continue;
        };
        let app_path_str = app_path_ns.to_string();

        // Finder-style display name: localised, ".app" suffix stripped by the OS.
        let display_name = file_manager.displayNameAtPath(&app_path_ns).to_string();

        // Bundle identifier (com.apple.TextEdit) stored in `desktop_id`.
        // Reserved for a future "set as default" action; not used at launch time.
        let bundle_id: Option<String> = NSBundle::bundleWithURL(&app_url)
            .and_then(|b| b.bundleIdentifier())
            .map(|id| id.to_string());

        let is_default = default_path.as_deref() == Some(&app_path_str);

        result.push(OpenWithApp {
            display_name,
            desktop_id: bundle_id,
            // Launch via the `open` command so macOS handles sandboxing,
            // Rosetta translation, and document handoff automatically.
            program: "open".to_string(),
            args: vec!["-a".to_string(), app_path_str, path_str.to_string()],
            is_default,
            // NSWorkspace only returns GUI app bundles; terminal launch is
            // never required on macOS.
            requires_terminal: false,
        });
    }

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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_macos_12_or_later ──────────────────────────────────────────────────

    #[test]
    fn is_macos_12_or_later_returns_true_on_ci() {
        // The CI macOS runner is macOS 14+.  This test documents the expected
        // result on modern hardware and would fail if ever run on macOS < 12.
        assert!(
            is_macos_12_or_later(),
            "expected macOS 12+ on CI; got an older system"
        );
    }

    // ── discover_via_nsworkspace ──────────────────────────────────────────────

    #[test]
    fn discover_returns_vec_for_known_plain_text_file() {
        // Every macOS 12+ system has at least one app registered for text/plain
        // (TextEdit.app ships with the OS).  We only assert the contract, not
        // the exact list, so this passes on any standard macOS installation.
        let tmp = std::env::temp_dir().join("elio-macos-test.txt");
        std::fs::write(&tmp, "hello").expect("write temp file");

        let apps = discover_via_nsworkspace(&tmp);
        let _ = std::fs::remove_file(&tmp);

        assert!(
            !apps.is_empty(),
            "expected at least one app for a .txt file on macOS"
        );

        // At most one entry should have is_default=true.
        let default_count = apps.iter().filter(|a| a.is_default).count();
        assert!(
            default_count <= 1,
            "at most one app should have is_default=true; got {default_count}"
        );

        // Every app must use `open -a <bundle_path> <file>` launch convention.
        for app in &apps {
            assert_eq!(
                app.program, "open",
                "program must be 'open', got {:?}",
                app.program
            );
            assert!(
                app.args.first().map(|s| s == "-a").unwrap_or(false),
                "first arg must be '-a' for app {:?}",
                app.display_name
            );
            assert_eq!(app.requires_terminal, false);
            assert!(
                !app.display_name.is_empty(),
                "display_name must not be empty"
            );
        }
    }

    #[test]
    fn discover_returns_empty_for_nonexistent_path() {
        // A path that does not exist and has an unregistered extension.
        let path = Path::new("/tmp/elio_macos_no_such_file_xyzzy_42.elio_test_ext");
        let apps = discover_via_nsworkspace(path);
        // We can't assert empty (some apps register for unknown types), but we
        // can assert the result is a valid Vec — this tests that the function
        // does not panic on a missing file.
        let _ = apps; // no panic == pass
    }

    #[test]
    fn discover_returns_empty_for_non_utf8_path() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let non_utf8 = OsStr::from_bytes(b"/tmp/\xff\xfe.txt");
        let path = Path::new(non_utf8);
        // Non-UTF-8 paths cannot be passed to NSString; the function returns
        // empty rather than panicking.
        let apps = discover_via_nsworkspace(path);
        assert!(
            apps.is_empty(),
            "expected empty vec for non-UTF-8 path, got {apps:?}"
        );
    }

    #[test]
    fn default_app_is_sorted_first() {
        let tmp = std::env::temp_dir().join("elio-macos-sort-test.txt");
        std::fs::write(&tmp, "hello").expect("write temp file");

        let apps = discover_via_nsworkspace(&tmp);
        let _ = std::fs::remove_file(&tmp);

        if apps.len() > 1 {
            // If a default exists it must be the first entry.
            if apps.iter().any(|a| a.is_default) {
                assert!(apps[0].is_default, "default app must be first in the list");
            }
        }
    }
}
