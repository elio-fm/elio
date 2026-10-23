use super::super::x11::{
    atoms_from_property_for_test, exported_target_names_for_test, next_incr_chunk_end_for_test,
    selection_notify_event_for_test,
};
use x11rb::protocol::xproto::{GetPropertyReply, SELECTION_NOTIFY_EVENT};

#[test]
fn target_atoms_require_a_32_bit_x11_property() {
    let property = GetPropertyReply {
        format: 32,
        ..Default::default()
    };
    assert_eq!(atoms_from_property_for_test(&property), Some(vec![]));

    let malformed_property = GetPropertyReply {
        format: 8,
        ..Default::default()
    };
    assert_eq!(atoms_from_property_for_test(&malformed_property), None);
}

#[test]
fn file_export_advertises_kde_copy_formats_without_legacy_text_targets() {
    let targets = exported_target_names_for_test(vec![
        ("text/uri-list".into(), b"file:///tmp/a\r\n".to_vec()),
        (
            "x-special/gnome-copied-files".into(),
            b"copy\nfile:///tmp/a\n".to_vec(),
        ),
        (
            "application/x-kde4-urilist".into(),
            b"file:///tmp/a\r\n".to_vec(),
        ),
        ("application/x-kde-cutselection".into(), b"0".to_vec()),
        ("text/plain;charset=utf-8".into(), b"/tmp/a".to_vec()),
    ]);

    for atom in [
        "text/uri-list",
        "x-special/gnome-copied-files",
        "application/x-kde4-urilist",
        "application/x-kde-cutselection",
        "text/plain;charset=utf-8",
        "text/plain",
        "UTF8_STRING",
    ] {
        assert!(targets.contains(&atom));
    }
    assert!(!targets.contains(&"TEXT"));
    assert!(!targets.contains(&"STRING"));
}

#[test]
fn incr_payload_is_chunked_and_terminated_for_large_selections() {
    const CHUNK: usize = 60 * 1024;

    assert_eq!(next_incr_chunk_end_for_test(0, CHUNK + 7), Some(CHUNK));
    assert_eq!(
        next_incr_chunk_end_for_test(CHUNK, CHUNK + 7),
        Some(CHUNK + 7)
    );
    assert_eq!(next_incr_chunk_end_for_test(CHUNK + 7, CHUNK + 7), None);
}

#[test]
fn synthetic_selection_notify_uses_the_x11_selection_notify_opcode() {
    assert_eq!(
        selection_notify_event_for_test().response_type,
        SELECTION_NOTIFY_EVENT
    );
}
