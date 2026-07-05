pub(super) mod crossterm_compat;
mod kitty_dnd;
mod osc;

use std::io;

use crate::runtime::input::RuntimeInputEvent;

#[derive(Default)]
pub(super) struct Parser {
    kitty_dnd: kitty_dnd::Parser,
}

pub(super) fn parse_event(
    parser: &mut Parser,
    buffer: &mut Vec<u8>,
    input_available: bool,
) -> io::Result<Option<RuntimeInputEvent>> {
    let len_before_dnd = buffer.len();
    if let Some(event) = parser.kitty_dnd.parse(buffer) {
        return Ok(Some(RuntimeInputEvent::KittyDnd(event)));
    }
    if buffer.len() != len_before_dnd {
        return Ok(None);
    }
    if buffer.is_empty() {
        return Ok(None);
    }
    if osc::starts_with_osc(buffer) && osc::end(buffer).is_none() {
        return Ok(None);
    }
    match crossterm_compat::parse_event(buffer, input_available) {
        Ok(Some(crossterm_compat::InternalEvent::Event(event))) => {
            Ok(Some(RuntimeInputEvent::Terminal(event)))
        }
        Ok(Some(_)) => Ok(None),
        Ok(None) => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::kitty_dnd::{DndOperation, KittyDndEvent};

    #[test]
    fn routes_kitty_dnd_osc72_before_crossterm_compat() {
        let mut parser = Parser::default();
        let mut buffer = b"\x1b]72;t=M:x=1:y=2:o=1;text/uri-list\x1b\\".to_vec();

        assert_eq!(
            parse_event(&mut parser, &mut buffer, false).unwrap(),
            Some(RuntimeInputEvent::KittyDnd(KittyDndEvent::DropOffer {
                mime_index: 1,
                operation: DndOperation::Copy,
                final_drop: true,
            }))
        );
    }

    #[test]
    fn waits_for_complete_osc_sequence() {
        let mut parser = Parser::default();
        let mut buffer = b"\x1b]72;t=M:x=1:y=2;text/uri-list".to_vec();

        assert_eq!(parse_event(&mut parser, &mut buffer, true).unwrap(), None);
        assert!(!buffer.is_empty());
    }

    #[test]
    fn keeps_kitty_dnd_chunks_until_end_marker() {
        let mut parser = Parser::default();
        let mut buffer = b"\x1b]72;t=r:x=1:m=1;ZmlsZTov\x1b\\".to_vec();

        assert_eq!(parse_event(&mut parser, &mut buffer, false).unwrap(), None);
        assert!(buffer.is_empty());

        let mut buffer = b"\x1b]72;m=1;Ly90bXAvYS50eHQ\x1b\\".to_vec();
        assert_eq!(parse_event(&mut parser, &mut buffer, false).unwrap(), None);
        assert!(buffer.is_empty());

        let mut buffer = b"\x1b]72;t=r:x=1:m=0;\x1b\\".to_vec();
        assert_eq!(
            parse_event(&mut parser, &mut buffer, false).unwrap(),
            Some(RuntimeInputEvent::KittyDnd(KittyDndEvent::DropData {
                mime_index: 1,
                paths: vec![std::path::PathBuf::from("/tmp/a.txt")],
                unsupported_schemes: Vec::new(),
            }))
        );
    }

    #[test]
    fn preserves_following_bytes_after_kitty_dnd_sequence() {
        let mut parser = Parser::default();
        let mut buffer = b"\x1b]72;t=o:x=1:y=2\x1b\\\x1b]72;t=e:x=4:y=1\x1b\\".to_vec();

        assert_eq!(
            parse_event(&mut parser, &mut buffer, false).unwrap(),
            Some(RuntimeInputEvent::KittyDnd(KittyDndEvent::DragOffer {
                x: 1,
                y: 2,
            }))
        );
        assert_eq!(buffer, b"\x1b]72;t=e:x=4:y=1\x1b\\");
    }
}
