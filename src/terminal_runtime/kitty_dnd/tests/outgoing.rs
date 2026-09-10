use super::*;

#[test]
fn startup_enables_drop_and_drag_without_machine_id() {
    assert_eq!(
        startup_sequence(None),
        "\x1b]72;t=a;text/uri-list\x1b\\\x1b]72;t=o:x=1;\x1b\\"
    );
}

#[test]
fn startup_enables_drop_and_drag_with_machine_id() {
    assert_eq!(
        startup_sequence(Some("host")),
        "\x1b]72;t=a;text/uri-list\x1b\\\x1b]72;t=o:x=1;host\x1b\\"
    );
}

#[test]
fn disable_turns_off_drop_and_drag() {
    assert_eq!(disable_sequence(), "\x1b]72;t=A\x1b\\\x1b]72;t=o:x=2\x1b\\");
}

#[test]
fn drop_reply_sequences_match_protocol_shape() {
    assert_eq!(
        accept_drop_sequence(DndOperation::Copy),
        "\x1b]72;t=m:o=1;text/uri-list\x1b\\"
    );
    assert_eq!(
        accept_drop_sequence(DndOperation::Move),
        "\x1b]72;t=m:o=2;text/uri-list\x1b\\"
    );
    assert_eq!(
        accept_drop_sequence(DndOperation::Either),
        "\x1b]72;t=m:o=1;text/uri-list\x1b\\"
    );
    assert_eq!(reject_drop_sequence(), "\x1b]72;t=m:o=0\x1b\\");
    assert_eq!(request_drop_data_sequence(2), "\x1b]72;t=r:x=2\x1b\\");
    assert_eq!(
        finish_drop_sequence(DropFinish::Copy),
        "\x1b]72;t=r:o=1\x1b\\"
    );
    assert_eq!(
        finish_drop_sequence(DropFinish::Move),
        "\x1b]72;t=r:o=2\x1b\\"
    );
    assert_eq!(
        finish_drop_sequence(DropFinish::Reject),
        "\x1b]72;t=r:o=0\x1b\\"
    );
}

#[test]
fn drag_reply_sequences_match_protocol_shape() {
    assert_eq!(
        agree_drag_sequence(DndOperation::Copy),
        "\x1b]72;t=o:o=1;text/uri-list\x1b\\"
    );
    assert_eq!(
        agree_drag_sequence(DndOperation::Move),
        "\x1b]72;t=o:o=2;text/uri-list\x1b\\"
    );
    assert_eq!(
        agree_drag_sequence(DndOperation::Either),
        "\x1b]72;t=o:o=3;text/uri-list\x1b\\"
    );
    assert_eq!(start_drag_sequence(), "\x1b]72;t=P:x=-1\x1b\\");
}

#[test]
fn uri_list_payload_encodes_absolute_local_paths_without_trailing_crlf() {
    assert_eq!(
        uri_list_payload(&[
            PathBuf::from("/tmp/a b.txt"),
            PathBuf::from("relative"),
            PathBuf::from("/tmp/é.txt"),
        ]),
        b"file:///tmp/a%20b.txt\r\nfile:///tmp/%C3%A9.txt".to_vec()
    );
}

#[test]
fn present_drag_data_encodes_and_finishes_payload() {
    assert_eq!(
        present_drag_data_sequence(0, b"file:///tmp/a.txt"),
        "\x1b]72;t=p:x=0:m=0;ZmlsZTovLy90bXAvYS50eHQ\x1b\\\x1b]72;t=p:x=0:m=0;\x1b\\"
    );
}

#[test]
fn send_drag_data_responds_to_terminal_request_without_restarting_drag() {
    assert_eq!(
        send_drag_data_sequence(0, b"file:///tmp/a.txt"),
        "\x1b]72;t=e:y=0:m=0;ZmlsZTovLy90bXAvYS50eHQ\x1b\\\x1b]72;t=e:y=0:m=0;\x1b\\"
    );
}

#[test]
fn drag_data_error_reports_requested_index() {
    assert_eq!(
        drag_data_error_sequence(2, "ENOENT"),
        "\x1b]72;t=E:y=2;ENOENT\x1b\\"
    );
}

#[test]
fn cancel_drag_aborts_the_pending_source_drag() {
    assert_eq!(cancel_drag_sequence(), "\x1b]72;t=E:y=-1\x1b\\");
}

#[test]
fn present_drag_icon_uses_text_payload_without_finish_marker() {
    assert_eq!(
        present_drag_icon_sequence("1 selected file(s)"),
        "\x1b]72;t=p:x=-1:y=0:X=6:Y=4:o=1024:m=0;MSBzZWxlY3RlZCBmaWxlKHMp\x1b\\"
    );
}

#[test]
fn uri_list_payload_appends_slash_for_directories() {
    let root = std::env::temp_dir().join(format!(
        "elio-dnd-dir-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();

    let payload = uri_list_payload(std::slice::from_ref(&root));
    let text = String::from_utf8(payload).unwrap();
    assert!(text.ends_with('/'));

    std::fs::remove_dir_all(root).ok();
}
