use super::*;

#[test]
fn completed_helper_restore_preserves_metadata_warning() {
    let warning = restore_helper_response(crate::elevated_session::Response {
        completed: 1,
        error: None,
        warning: Some("could not update Trash restore metadata".to_string()),
    })
    .unwrap();
    assert_eq!(
        warning.as_deref(),
        Some("could not update Trash restore metadata")
    );
}

#[test]
fn failed_helper_restore_remains_an_error() {
    let error = restore_helper_response(crate::elevated_session::Response {
        completed: 0,
        error: Some("permission denied".to_string()),
        warning: None,
    })
    .unwrap_err();
    assert_eq!(error.to_string(), "permission denied");
}
