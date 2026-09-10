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

    // GUI entries must use the `open -a` launch convention.
    for app in apps.iter().filter(|a| !a.requires_terminal) {
        assert_eq!(app.program, "open");
        assert_eq!(app.args.first().map(String::as_str), Some("-a"));
        assert!(!app.display_name.is_empty());
    }
    // Terminal entries must have a non-empty display name and no GUI wrapper.
    for app in apps.iter().filter(|a| a.requires_terminal) {
        assert!(!app.display_name.is_empty());
        assert_ne!(app.program, "open");
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
