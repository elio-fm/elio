use super::*;

#[test]
fn immutable_uri_handles_windows_drive_and_unc_paths() {
    assert_eq!(
        immutable_uri(Path::new(r"C:\dbs\a #1%.db")).as_deref(),
        Some("file:/C:/dbs/a%20%231%25.db?immutable=1")
    );
    assert_eq!(
        immutable_uri(Path::new(r"\\server\share\a.db")).as_deref(),
        Some("file:////server/share/a.db?immutable=1")
    );
    assert!(immutable_uri(Path::new(r"C:relative.db")).is_none());
}
