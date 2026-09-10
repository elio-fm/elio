use super::*;

#[test]
fn parses_drop_offer_for_uri_list() {
    assert_eq!(
        parse_osc72(b"\x1b]72;t=M:x=12:y=5:o=1;text/plain text/uri-list\x1b\\"),
        Some(KittyDndEvent::DropOffer {
            mime_index: 2,
            operation: DndOperation::Copy,
            final_drop: true,
        })
    );
}

#[test]
fn parses_unsupported_drop_offer_with_phase() {
    assert_eq!(
        parse_osc72(b"\x1b]72;t=m:x=12:y=5:o=1;text/plain\x1b\\"),
        Some(KittyDndEvent::DropUnsupported { final_drop: false })
    );
    assert_eq!(
        parse_osc72(b"\x1b]72;t=M:x=12:y=5:o=2;text/plain\x1b\\"),
        Some(KittyDndEvent::DropUnsupported { final_drop: true })
    );
}

#[test]
fn parses_drop_offer_operation() {
    assert_eq!(
        parse_osc72(b"\x1b]72;t=m:x=12:y=5:o=2;text/uri-list\x1b\\"),
        Some(KittyDndEvent::DropOffer {
            mime_index: 1,
            operation: DndOperation::Move,
            final_drop: false,
        })
    );
    assert_eq!(
        parse_osc72(b"\x1b]72;t=m:x=12:y=5:o=3;text/uri-list\x1b\\"),
        Some(KittyDndEvent::DropOffer {
            mime_index: 1,
            operation: DndOperation::Either,
            final_drop: false,
        })
    );
}

#[test]
fn rejects_drop_offer_without_valid_operation() {
    assert_eq!(
        parse_osc72(b"\x1b]72;t=m:x=12:y=5;text/uri-list\x1b\\"),
        None
    );
    assert_eq!(
        parse_osc72(b"\x1b]72;t=m:x=12:y=5:o=9;text/uri-list\x1b\\"),
        None
    );
}

#[test]
fn reports_unsupported_uri_schemes_in_drop_data() {
    let data = STANDARD_NO_PAD.encode("trash:///foo\nsmb://server/share/file\nfile:///tmp/a.txt");
    let sequence = format!("\x1b]72;t=r:x=1;{data}\x1b\\");
    let mut state = Osc72State::default();
    assert_eq!(
        parse_osc72_with_state(sequence.as_bytes(), &mut state),
        None
    );

    assert_eq!(
        parse_osc72_with_state(b"\x1b]72;t=r:x=1:m=0;\x1b\\", &mut state),
        Some(KittyDndEvent::DropData {
            mime_index: 1,
            paths: vec![PathBuf::from("/tmp/a.txt")],
            unsupported_schemes: vec!["trash".to_string(), "smb".to_string()],
        })
    );
}

#[test]
fn parses_drop_data_error() {
    assert_eq!(
        parse_osc72(b"\x1b]72;t=R:x=1;EPERM:cannot drop into self window\x1b\\"),
        Some(KittyDndEvent::DropDataError {
            mime_index: Some(1),
            message: "EPERM:cannot drop into self window".to_string(),
        })
    );
}

#[test]
fn decodes_local_file_uri_list() {
    let payload = STANDARD.encode("file:///tmp/a%20b.txt\r\n# comment\r\nfile://remote/tmp/no\r\n");
    let sequence = format!("\x1b]72;t=r:x=1;{payload}\x1b\\");
    let mut state = Osc72State::default();
    assert_eq!(
        parse_osc72_with_state(sequence.as_bytes(), &mut state),
        None
    );
    assert_eq!(
        parse_osc72_with_state(b"\x1b]72;t=r:x=1:m=0;\x1b\\", &mut state),
        Some(KittyDndEvent::DropData {
            mime_index: 1,
            paths: vec![PathBuf::from("/tmp/a b.txt")],
            unsupported_schemes: vec!["file".to_string()],
        })
    );
}

#[test]
fn parses_drag_offer() {
    assert_eq!(
        parse_osc72(b"\x1b]72;t=o:x=12:y=5\x1b\\"),
        Some(KittyDndEvent::DragOffer { x: 12, y: 5 })
    );
}

#[test]
fn parses_drag_data_request_and_end() {
    assert_eq!(
        parse_osc72(b"\x1b]72;t=e:x=1:y=0\x1b\\"),
        Some(KittyDndEvent::DragAccepted { mime_index: 0 })
    );
    assert_eq!(
        parse_osc72(b"\x1b]72;t=e:x=2:o=1\x1b\\"),
        Some(KittyDndEvent::DragActionChanged {
            operation: DndOperation::Copy,
        })
    );
    assert_eq!(
        parse_osc72(b"\x1b]72;t=e:x=3\x1b\\"),
        Some(KittyDndEvent::DragDropped)
    );
    assert_eq!(
        parse_osc72(b"\x1b]72;t=e:x=5:y=0\x1b\\"),
        Some(KittyDndEvent::DragDataRequested { mime_index: 0 })
    );
    assert_eq!(
        parse_osc72(b"\x1b]72;t=e:x=4:y=1\x1b\\"),
        Some(KittyDndEvent::DragEnded { cancelled: true })
    );
}

#[test]
fn parses_drag_start_acknowledgement() {
    assert_eq!(
        parse_osc72(b"\x1b]72;t=E;OK\x1b\\"),
        Some(KittyDndEvent::DragStarted)
    );
    assert_eq!(
        parse_osc72(b"\x1b]72;t=E;EPERM\x1b\\"),
        Some(KittyDndEvent::DragError {
            message: "EPERM".to_string()
        })
    );
}

#[test]
fn ignores_empty_end_of_data_marker() {
    assert_eq!(parse_osc72(b"\x1b]72;t=r:x=1:m=0;\x1b\\"), None);
}

#[test]
fn decodes_unpadded_base64_uri_list() {
    let payload = STANDARD_NO_PAD.encode("file:///tmp/a.txt");
    let sequence = format!("\x1b]72;t=r:x=1;{payload}\x1b\\");
    let mut state = Osc72State::default();
    assert_eq!(
        parse_osc72_with_state(sequence.as_bytes(), &mut state),
        None
    );
    assert_eq!(
        parse_osc72_with_state(b"\x1b]72;t=r:x=1:m=0;\x1b\\", &mut state),
        Some(KittyDndEvent::DropData {
            mime_index: 1,
            paths: vec![PathBuf::from("/tmp/a.txt")],
            unsupported_schemes: Vec::new(),
        })
    );
}

#[test]
fn assembles_chunked_drop_data() {
    let mut state = Osc72State::default();
    assert_eq!(
        parse_osc72_with_state(b"\x1b]72;t=r:x=1:m=1;ZmlsZTov\x1b\\", &mut state),
        None
    );
    assert_eq!(
        parse_osc72_with_state(b"\x1b]72;m=1;Ly90bXAv\x1b\\", &mut state),
        None
    );
    assert_eq!(
        parse_osc72_with_state(b"\x1b]72;m=1;YS50eHQ\x1b\\", &mut state),
        None
    );
    assert_eq!(
        parse_osc72_with_state(b"\x1b]72;t=r:x=1:m=0;\x1b\\", &mut state),
        Some(KittyDndEvent::DropData {
            mime_index: 1,
            paths: vec![PathBuf::from("/tmp/a.txt")],
            unsupported_schemes: Vec::new(),
        })
    );
}
