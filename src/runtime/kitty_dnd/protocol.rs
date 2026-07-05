use std::path::{Path, PathBuf};

use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};

pub(super) const URI_LIST_MIME: &str = "text/uri-list";
const DRAG_PAYLOAD_CHUNK_SIZE: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum DndOperation {
    Copy,
    Move,
    Either,
}

impl DndOperation {
    pub(super) fn from_protocol(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Copy),
            2 => Some(Self::Move),
            3 => Some(Self::Either),
            _ => None,
        }
    }

    fn protocol_value(self) -> u8 {
        match self {
            Self::Copy => 1,
            Self::Move => 2,
            Self::Either => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum DropFinish {
    Copy,
    Move,
    Reject,
}

/// Enable the local Kitty DND slice elio currently supports: accepting file URI
/// drops and offering local file URI drags.
pub(in crate::runtime) fn startup_sequence(machine_id: Option<&str>) -> String {
    format!(
        "{}{}",
        enable_drop_sequence(),
        enable_drag_sequence(machine_id.unwrap_or(""))
    )
}

/// Disable both halves before suspending or restoring the terminal.
pub(in crate::runtime) fn disable_sequence() -> &'static str {
    "\x1b]72;t=A\x1b\\\x1b]72;t=o:x=2\x1b\\"
}

fn enable_drop_sequence() -> String {
    format!("\x1b]72;t=a;{URI_LIST_MIME}\x1b\\")
}

fn enable_drag_sequence(machine_id: &str) -> String {
    format!("\x1b]72;t=o:x=1;{machine_id}\x1b\\")
}

pub(in crate::runtime) fn accept_drop_sequence(operation: DndOperation) -> String {
    let operation = match operation {
        DndOperation::Copy | DndOperation::Either => DndOperation::Copy,
        DndOperation::Move => DndOperation::Move,
    };
    format!(
        "\x1b]72;t=m:o={};{URI_LIST_MIME}\x1b\\",
        operation.protocol_value()
    )
}

pub(in crate::runtime) fn reject_drop_sequence() -> &'static str {
    "\x1b]72;t=m:o=0\x1b\\"
}

pub(in crate::runtime) fn request_drop_data_sequence(mime_index: u8) -> String {
    format!("\x1b]72;t=r:x={mime_index}\x1b\\")
}

pub(in crate::runtime) fn finish_drop_sequence(finish: DropFinish) -> &'static str {
    match finish {
        DropFinish::Copy => "\x1b]72;t=r:o=1\x1b\\",
        DropFinish::Move => "\x1b]72;t=r:o=2\x1b\\",
        DropFinish::Reject => "\x1b]72;t=r:o=0\x1b\\",
    }
}

pub(in crate::runtime) fn agree_drag_sequence(operation: DndOperation) -> String {
    format!(
        "\x1b]72;t=o:o={};{URI_LIST_MIME}\x1b\\",
        operation.protocol_value()
    )
}

pub(in crate::runtime) fn start_drag_sequence() -> &'static str {
    "\x1b]72;t=P:x=-1\x1b\\"
}

pub(in crate::runtime) fn present_drag_data_sequence(mime_index: i8, data: &[u8]) -> String {
    present_drag_payload_sequence(&format!("t=p:x={mime_index}"), data, true)
}

pub(in crate::runtime) fn send_drag_data_sequence(mime_index: u8, data: &[u8]) -> String {
    present_drag_payload_sequence(&format!("t=e:y={mime_index}"), data, true)
}

pub(in crate::runtime) fn drag_data_error_sequence(mime_index: u8, error: &str) -> String {
    format!("\x1b]72;t=E:y={mime_index};{error}\x1b\\")
}

pub(in crate::runtime) fn cancel_drag_sequence() -> &'static str {
    "\x1b]72;t=E:y=-1\x1b\\"
}

pub(in crate::runtime) fn present_drag_icon_sequence(label: &str) -> String {
    // Present a compact text icon payload accepted by Kitty's DND protocol.
    present_drag_payload_sequence("t=p:x=-1:y=0:X=6:Y=4:o=0", label.as_bytes(), false)
}

fn present_drag_payload_sequence(metadata: &str, data: &[u8], finish: bool) -> String {
    let encoded = STANDARD_NO_PAD.encode(data);
    let mut sequence = String::new();
    let chunks = encoded.as_bytes().chunks(DRAG_PAYLOAD_CHUNK_SIZE).count();
    for (index, chunk) in encoded
        .as_bytes()
        .chunks(DRAG_PAYLOAD_CHUNK_SIZE)
        .enumerate()
    {
        let more = usize::from(index + 1 < chunks);
        let chunk = std::str::from_utf8(chunk).expect("base64 output is valid UTF-8");
        if index == 0 {
            sequence.push_str(&format!("\x1b]72;{metadata}:m={more};{chunk}\x1b\\"));
        } else {
            sequence.push_str(&format!("\x1b]72;m={more};{chunk}\x1b\\"));
        }
    }
    if finish {
        sequence.push_str(&format!("\x1b]72;{metadata}:m=0;\x1b\\"));
    }
    sequence
}

pub(in crate::runtime) fn uri_list_payload(paths: &[PathBuf]) -> Vec<u8> {
    paths
        .iter()
        .filter(|path| path.is_absolute())
        .map(|path| {
            let mut encoded = percent_encode_path(path);
            if path.is_dir() && !encoded.ends_with('/') {
                encoded.push('/');
            }
            format!("file://{encoded}")
        })
        .collect::<Vec<_>>()
        .join("\r\n")
        .into_bytes()
}

fn percent_encode_path(path: &Path) -> String {
    let mut encoded = String::new();
    for byte in path.to_string_lossy().as_bytes() {
        encoded.push_str(&percent_encode_byte(*byte));
    }
    encoded
}

fn percent_encode_byte(byte: u8) -> String {
    match byte {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
            char::from(byte).to_string()
        }
        _ => format!("%{byte:02X}"),
    }
}

#[cfg(test)]
mod tests {
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
            "\x1b]72;t=p:x=-1:y=0:X=6:Y=4:o=0:m=0;MSBzZWxlY3RlZCBmaWxlKHMp\x1b\\"
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
}
