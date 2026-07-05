use std::path::PathBuf;

use base64::{
    Engine,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD},
};

use super::protocol::{DndOperation, URI_LIST_MIME};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum KittyDndEvent {
    DropOffer {
        mime_index: u8,
        operation: DndOperation,
        final_drop: bool,
    },
    DropLeave,
    DropData {
        mime_index: u8,
        paths: Vec<PathBuf>,
        unsupported_schemes: Vec<String>,
    },
    DropDataError {
        mime_index: Option<u8>,
        message: String,
    },
    DropUnsupported {
        final_drop: bool,
    },
    DragOffer {
        x: u16,
        y: u16,
    },
    DragStarted,
    DragAccepted {
        mime_index: u8,
    },
    DragActionChanged {
        operation: DndOperation,
    },
    DragDropped,
    DragDataRequested {
        mime_index: u8,
    },
    DragEnded {
        cancelled: bool,
    },
    DragError {
        message: String,
    },
}

#[derive(Default)]
pub(in crate::runtime) struct Osc72State {
    fields: DndFields,
    payload: Vec<u8>,
    active: bool,
}

impl Osc72State {
    fn reset(&mut self) {
        self.fields = DndFields::default();
        self.payload.clear();
        self.active = false;
    }
}

#[cfg(test)]
pub(in crate::runtime) fn parse_osc72(sequence: &[u8]) -> Option<KittyDndEvent> {
    let mut state = Osc72State::default();
    parse_osc72_with_state(sequence, &mut state)
}

pub(in crate::runtime) fn parse_osc72_with_state(
    sequence: &[u8],
    state: &mut Osc72State,
) -> Option<KittyDndEvent> {
    let body = strip_osc72(sequence)?;
    let (metadata, payload) =
        SplitOnceByte::split_once(body, |byte| *byte == b';').unwrap_or((body, &[]));
    let fields = DndFields::parse(metadata);
    let payload_for_event;
    let fields_for_event;
    let ty = if state.active {
        state.fields.ty
    } else {
        fields.ty
    };
    let has_more = fields.more == Some(true) || ty == Some(b'r') && !payload.is_empty();

    if state.active {
        state.payload.extend_from_slice(payload);
        if has_more {
            return None;
        }
        fields_for_event = state.fields.clone();
        payload_for_event = std::mem::take(&mut state.payload);
        state.reset();
        return event_from_parts(fields_for_event, &payload_for_event);
    }

    if has_more {
        state.fields = fields;
        state.payload.extend_from_slice(payload);
        state.active = true;
        return None;
    }

    event_from_parts(fields, payload)
}

fn event_from_parts(fields: DndFields, payload: &[u8]) -> Option<KittyDndEvent> {
    match fields.ty? {
        b'm' => {
            if fields.x == Some(-1) && fields.y == Some(-1) {
                return Some(KittyDndEvent::DropLeave);
            }
            drop_offer_from_parts(fields.op, payload, false)
        }
        b'M' => drop_offer_from_parts(fields.op, payload, true),
        b'r' => {
            let mime_index = fields.x.and_then(|x| u8::try_from(x).ok())?;
            if payload.is_empty() && fields.more == Some(false) {
                return None;
            }
            let data = decode_base64(payload)?;
            let payload = parse_uri_list(&String::from_utf8(data).ok()?);
            Some(KittyDndEvent::DropData {
                mime_index,
                paths: payload.paths,
                unsupported_schemes: payload.unsupported_schemes,
            })
        }
        b'R' => Some(KittyDndEvent::DropDataError {
            mime_index: fields.x.and_then(|x| u8::try_from(x).ok()),
            message: String::from_utf8_lossy(payload).into_owned(),
        }),
        b'o' => Some(KittyDndEvent::DragOffer {
            x: fields.x.and_then(|x| u16::try_from(x).ok())?,
            y: fields.y.and_then(|y| u16::try_from(y).ok())?,
        }),
        b'e' if fields.x == Some(1) => Some(KittyDndEvent::DragAccepted {
            mime_index: fields.y.and_then(|y| u8::try_from(y).ok())?,
        }),
        b'e' if fields.x == Some(2) => Some(KittyDndEvent::DragActionChanged {
            operation: fields.op.and_then(DndOperation::from_protocol)?,
        }),
        b'e' if fields.x == Some(3) => Some(KittyDndEvent::DragDropped),
        b'e' if fields.x == Some(4) => Some(KittyDndEvent::DragEnded {
            cancelled: fields.y.unwrap_or(0) != 0,
        }),
        b'e' if fields.x == Some(5) => Some(KittyDndEvent::DragDataRequested {
            mime_index: fields.y.and_then(|y| u8::try_from(y).ok())?,
        }),
        b'E' if payload == b"OK" => Some(KittyDndEvent::DragStarted),
        b'E' => Some(KittyDndEvent::DragError {
            message: String::from_utf8_lossy(payload).into_owned(),
        }),
        _ => None,
    }
}

fn strip_osc72(sequence: &[u8]) -> Option<&[u8]> {
    let body = sequence.strip_prefix(b"\x1b]72;")?;
    body.strip_suffix(b"\x1b\\")
        .or_else(|| body.strip_suffix(b"\x07"))
}

fn decode_base64(payload: &[u8]) -> Option<Vec<u8>> {
    STANDARD
        .decode(payload)
        .or_else(|_| STANDARD_NO_PAD.decode(payload))
        .ok()
}

fn drop_offer_from_parts(
    operation: Option<i32>,
    payload: &[u8],
    final_drop: bool,
) -> Option<KittyDndEvent> {
    let operation = operation.and_then(DndOperation::from_protocol)?;
    offered_uri_list(payload).map_or(
        Some(KittyDndEvent::DropUnsupported { final_drop }),
        |mime_index| {
            Some(KittyDndEvent::DropOffer {
                mime_index,
                operation,
                final_drop,
            })
        },
    )
}

fn offered_uri_list(payload: &[u8]) -> Option<u8> {
    let text = std::str::from_utf8(payload).ok()?;
    text.split_whitespace()
        .position(|mime| mime == URI_LIST_MIME)
        // Kitty's data request index is 1-based, not Rust's 0-based `position()`.
        .and_then(|idx| u8::try_from(idx + 1).ok())
}

#[derive(Debug, Default, Eq, PartialEq)]
struct ParsedUriList {
    paths: Vec<PathBuf>,
    unsupported_schemes: Vec<String>,
}

fn parse_uri_list(text: &str) -> ParsedUriList {
    let mut parsed = ParsedUriList::default();
    for uri in text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        if let Some(path) = file_uri_to_path(uri) {
            parsed.paths.push(path);
            continue;
        }
        let scheme = uri
            .split_once(':')
            .map(|(scheme, _)| scheme)
            .filter(|scheme| !scheme.is_empty())
            .unwrap_or("unknown");
        if !parsed
            .unsupported_schemes
            .iter()
            .any(|known| known == scheme)
        {
            parsed.unsupported_schemes.push(scheme.to_string());
        }
    }
    parsed
}

fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let path = if let Some(local) = rest.strip_prefix("localhost/") {
        format!("/{local}")
    } else if rest.starts_with('/') {
        rest.to_string()
    } else {
        return None;
    };
    Some(PathBuf::from(percent_decode(&path)?))
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = bytes.get(index + 1).copied().and_then(hex_value)?;
            let low = bytes.get(index + 2).copied().and_then(hex_value)?;
            out.push((high << 4) | low);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[derive(Clone, Default)]
struct DndFields {
    ty: Option<u8>,
    x: Option<i32>,
    y: Option<i32>,
    op: Option<i32>,
    more: Option<bool>,
}

impl DndFields {
    fn parse(metadata: &[u8]) -> Self {
        let mut fields = Self::default();
        for field in metadata.split(|byte| *byte == b':') {
            let Some((key, value)) = SplitOnceByte::split_once(field, |byte| *byte == b'=') else {
                continue;
            };
            match key {
                b"t" => fields.ty = value.first().copied(),
                b"x" => fields.x = parse_i32(value),
                b"y" => fields.y = parse_i32(value),
                b"o" => fields.op = parse_i32(value),
                b"m" => fields.more = parse_i32(value).map(|value| value != 0),
                _ => {}
            }
        }
        fields
    }
}

fn parse_i32(value: &[u8]) -> Option<i32> {
    std::str::from_utf8(value).ok()?.parse().ok()
}

trait SplitOnceByte {
    fn split_once(&self, predicate: impl FnMut(&u8) -> bool) -> Option<(&[u8], &[u8])>;
}

impl SplitOnceByte for [u8] {
    fn split_once(&self, mut predicate: impl FnMut(&u8) -> bool) -> Option<(&[u8], &[u8])> {
        let index = self.iter().position(&mut predicate)?;
        Some((&self[..index], &self[index + 1..]))
    }
}

#[cfg(test)]
mod tests {
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
        let data =
            STANDARD_NO_PAD.encode("trash:///foo\nsmb://server/share/file\nfile:///tmp/a.txt");
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
        let payload =
            STANDARD.encode("file:///tmp/a%20b.txt\r\n# comment\r\nfile://remote/tmp/no\r\n");
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
}
