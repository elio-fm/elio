use super::*;
use crate::terminal_runtime::kitty_dnd::DndOperation;

#[test]
fn unsupported_mouse_button_sequence_is_ignored() {
    let (sender, receiver) = mpsc::channel();
    let mut parser = EventParser::default();
    let mut buffer = b"\x1b[<128;10;5M".to_vec();

    parse_buffer(&mut parser, &mut buffer, &sender, false);

    assert!(buffer.is_empty());
    assert!(receiver.try_recv().is_err());
}

#[test]
fn parse_errors_from_real_io_are_still_errors() {
    let error = io::Error::other("different failure");
    assert!(!is_unsupported_input_sequence(&error));
}

#[test]
fn parse_buffer_keeps_concatenated_kitty_dnd_events() {
    let (sender, receiver) = mpsc::channel();
    let mut parser = EventParser::default();
    let mut buffer = b"\x1b]72;t=o:x=1:y=2\x1b\\\x1b]72;t=e:x=4:y=1\x1b\\".to_vec();

    parse_buffer(&mut parser, &mut buffer, &sender, false);

    assert!(buffer.is_empty());
    assert_eq!(
        receiver.try_recv().unwrap().unwrap(),
        InputEvent::KittyDnd(KittyDndEvent::DragOffer { x: 1, y: 2 })
    );
    assert_eq!(
        receiver.try_recv().unwrap().unwrap(),
        InputEvent::KittyDnd(KittyDndEvent::DragEnded { cancelled: true })
    );
    assert!(receiver.try_recv().is_err());
}

#[test]
fn routes_kitty_dnd_osc72_before_crossterm_parser() {
    let mut parser = EventParser::default();
    let mut buffer = b"\x1b]72;t=M:x=1:y=2:o=1;text/uri-list\x1b\\".to_vec();

    assert_eq!(
        parse_event(&mut parser, &mut buffer, false).unwrap(),
        Some(InputEvent::KittyDnd(KittyDndEvent::DropOffer {
            mime_index: 1,
            operation: DndOperation::Copy,
            final_drop: true,
        }))
    );
}

#[test]
fn waits_for_complete_osc_sequence() {
    let mut parser = EventParser::default();
    let mut buffer = b"\x1b]72;t=M:x=1:y=2;text/uri-list".to_vec();

    assert_eq!(parse_event(&mut parser, &mut buffer, true).unwrap(), None);
    assert!(!buffer.is_empty());
}

#[test]
fn keeps_kitty_dnd_chunks_until_end_marker() {
    let mut parser = EventParser::default();
    let mut buffer = b"\x1b]72;t=r:x=1:m=1;ZmlsZTov\x1b\\".to_vec();

    assert_eq!(parse_event(&mut parser, &mut buffer, false).unwrap(), None);
    assert!(buffer.is_empty());

    let mut buffer = b"\x1b]72;m=1;Ly90bXAvYS50eHQ\x1b\\".to_vec();
    assert_eq!(parse_event(&mut parser, &mut buffer, false).unwrap(), None);
    assert!(buffer.is_empty());

    let mut buffer = b"\x1b]72;t=r:x=1:m=0;\x1b\\".to_vec();
    assert_eq!(
        parse_event(&mut parser, &mut buffer, false).unwrap(),
        Some(InputEvent::KittyDnd(KittyDndEvent::DropData {
            mime_index: 1,
            paths: vec![std::path::PathBuf::from("/tmp/a.txt")],
            unsupported_schemes: Vec::new(),
        }))
    );
}

#[test]
fn preserves_following_bytes_after_kitty_dnd_sequence() {
    let mut parser = EventParser::default();
    let mut buffer = b"\x1b]72;t=o:x=1:y=2\x1b\\\x1b]72;t=e:x=4:y=1\x1b\\".to_vec();

    assert_eq!(
        parse_event(&mut parser, &mut buffer, false).unwrap(),
        Some(InputEvent::KittyDnd(KittyDndEvent::DragOffer {
            x: 1,
            y: 2,
        }))
    );
    assert_eq!(buffer, b"\x1b]72;t=e:x=4:y=1\x1b\\");
}
