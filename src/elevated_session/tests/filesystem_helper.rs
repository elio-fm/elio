use super::*;
use std::os::unix::ffi::OsStrExt;

#[test]
fn protocol_round_trips_non_utf8_paths() {
    let path = PathBuf::from(OsString::from_vec(b"/tmp/a\xffb".to_vec()));
    let request = Request::Trash(vec![path.clone()]);
    let mut bytes = Vec::new();
    write_request(&mut bytes, &request).unwrap();
    let Request::Trash(paths) = read_request(bytes.as_slice()).unwrap() else {
        panic!("wrong request type");
    };
    assert_eq!(paths[0].as_os_str().as_bytes(), path.as_os_str().as_bytes());
}

#[test]
fn protocol_rejects_oversized_frame() {
    let mut bytes = b"EU\x01".to_vec();
    bytes.push(2);
    bytes.extend_from_slice(&((MAX_PATH_BYTES as u32) + 1).to_le_bytes());
    assert_eq!(
        read_request(bytes.as_slice()).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn protocol_rejects_unknown_version() {
    let bytes = b"EU\x02\x02";
    assert_eq!(
        read_request(bytes.as_slice()).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn request_rejects_too_many_items_before_writing() {
    let request = Request::Trash(vec![PathBuf::from("/tmp/a"); MAX_ITEMS + 1]);
    let mut bytes = Vec::new();
    assert_eq!(
        write_request(&mut bytes, &request).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    assert!(bytes.is_empty());
}

#[test]
fn response_truncates_error_without_losing_completion() {
    let response = Response {
        completed: 7,
        error: Some("x".repeat(MAX_PATH_BYTES + 1)),
        warning: None,
    };
    let mut bytes = Vec::new();
    write_response(&mut bytes, &response).unwrap();
    let decoded = read_response(bytes.as_slice()).unwrap();
    assert_eq!(decoded.completed, 7);
    assert_eq!(decoded.error.unwrap().len(), MAX_ERROR_BYTES);
    assert!(decoded.warning.is_none());
}

#[test]
fn response_round_trips_warning_separately_from_error() {
    let response = Response {
        completed: 2,
        error: None,
        warning: Some("restore metadata is unavailable".to_string()),
    };
    let mut bytes = Vec::new();

    write_response(&mut bytes, &response).unwrap();
    let decoded = read_response(bytes.as_slice()).unwrap();

    assert_eq!(decoded.completed, 2);
    assert!(decoded.error.is_none());
    assert_eq!(
        decoded.warning.as_deref(),
        Some("restore metadata is unavailable")
    );
}

#[test]
fn response_rejects_error_and_warning_together_before_writing() {
    let response = Response {
        completed: 1,
        error: Some("failure".to_string()),
        warning: Some("warning".to_string()),
    };
    let mut bytes = Vec::new();

    assert_eq!(
        write_response(&mut bytes, &response).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    assert!(bytes.is_empty());
}

#[test]
fn restore_origin_names_are_bounded() {
    assert!(validate_restore_origin_names(&["report.pdf".to_string()]).is_ok());
    assert_eq!(
        validate_restore_origin_names(&vec!["name".to_string(); MAX_ITEMS + 1])
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
    assert_eq!(
        validate_restore_origin_names(&["x".repeat(MAX_PATH_BYTES + 1)])
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
}

#[test]
fn checked_restore_distinguishes_metadata_warning_from_restore_failure() {
    let warning = checked_restore_response(Ok(Some(anyhow::anyhow!("store is read-only"))));
    assert_eq!(warning.completed, 1);
    assert_eq!(
        warning.warning.as_deref(),
        Some("could not update Trash restore metadata: store is read-only")
    );
    assert!(warning.error.is_none());

    let failure = checked_restore_response(Err(anyhow::anyhow!("permission denied")));
    assert_eq!(failure.completed, 0);
    assert_eq!(failure.error.as_deref(), Some("permission denied"));
    assert!(failure.warning.is_none());
}

#[cfg(target_os = "macos")]
#[test]
fn remove_restore_origins_round_trips_names() {
    let request = Request::RemoveRestoreOrigins(vec![
        "report.pdf".to_string(),
        "report 11.53.48.pdf".to_string(),
    ]);
    let mut bytes = Vec::new();
    write_request(&mut bytes, &request).unwrap();
    let Request::RemoveRestoreOrigins(names) = read_request(bytes.as_slice()).unwrap() else {
        panic!("wrong request type");
    };
    assert_eq!(
        names,
        vec!["report.pdf".to_string(), "report 11.53.48.pdf".to_string()]
    );
}
