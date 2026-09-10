use std::path::{Path, PathBuf};

use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};

pub(super) const URI_LIST_MIME: &str = "text/uri-list";
const DRAG_PAYLOAD_CHUNK_SIZE: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::terminal_runtime) enum DndOperation {
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
pub(in crate::terminal_runtime) enum DropFinish {
    Copy,
    Move,
    Reject,
}

/// Enable the local Kitty DND slice elio currently supports: accepting file URI
/// drops and offering local file URI drags.
pub(in crate::terminal_runtime) fn startup_sequence(machine_id: Option<&str>) -> String {
    format!(
        "{}{}",
        enable_drop_sequence(),
        enable_drag_sequence(machine_id.unwrap_or(""))
    )
}

/// Disable both halves before suspending or restoring the terminal.
pub(in crate::terminal_runtime) fn disable_sequence() -> &'static str {
    "\x1b]72;t=A\x1b\\\x1b]72;t=o:x=2\x1b\\"
}

fn enable_drop_sequence() -> String {
    format!("\x1b]72;t=a;{URI_LIST_MIME}\x1b\\")
}

fn enable_drag_sequence(machine_id: &str) -> String {
    format!("\x1b]72;t=o:x=1;{machine_id}\x1b\\")
}

pub(in crate::terminal_runtime) fn accept_drop_sequence(operation: DndOperation) -> String {
    let operation = match operation {
        DndOperation::Copy | DndOperation::Either => DndOperation::Copy,
        DndOperation::Move => DndOperation::Move,
    };
    format!(
        "\x1b]72;t=m:o={};{URI_LIST_MIME}\x1b\\",
        operation.protocol_value()
    )
}

pub(in crate::terminal_runtime) fn reject_drop_sequence() -> &'static str {
    "\x1b]72;t=m:o=0\x1b\\"
}

pub(in crate::terminal_runtime) fn request_drop_data_sequence(mime_index: u8) -> String {
    format!("\x1b]72;t=r:x={mime_index}\x1b\\")
}

pub(in crate::terminal_runtime) fn finish_drop_sequence(finish: DropFinish) -> &'static str {
    match finish {
        DropFinish::Copy => "\x1b]72;t=r:o=1\x1b\\",
        DropFinish::Move => "\x1b]72;t=r:o=2\x1b\\",
        DropFinish::Reject => "\x1b]72;t=r:o=0\x1b\\",
    }
}

pub(in crate::terminal_runtime) fn agree_drag_sequence(operation: DndOperation) -> String {
    format!(
        "\x1b]72;t=o:o={};{URI_LIST_MIME}\x1b\\",
        operation.protocol_value()
    )
}

pub(in crate::terminal_runtime) fn start_drag_sequence() -> &'static str {
    "\x1b]72;t=P:x=-1\x1b\\"
}

pub(in crate::terminal_runtime) fn present_drag_data_sequence(
    mime_index: i8,
    data: &[u8],
) -> String {
    present_drag_payload_sequence(&format!("t=p:x={mime_index}"), data, true)
}

pub(in crate::terminal_runtime) fn send_drag_data_sequence(mime_index: u8, data: &[u8]) -> String {
    present_drag_payload_sequence(&format!("t=e:y={mime_index}"), data, true)
}

pub(in crate::terminal_runtime) fn drag_data_error_sequence(mime_index: u8, error: &str) -> String {
    format!("\x1b]72;t=E:y={mime_index};{error}\x1b\\")
}

pub(in crate::terminal_runtime) fn cancel_drag_sequence() -> &'static str {
    "\x1b]72;t=E:y=-1\x1b\\"
}

pub(in crate::terminal_runtime) fn present_drag_icon_sequence(label: &str) -> String {
    // Present a compact text icon payload accepted by Kitty's DND protocol.
    present_drag_payload_sequence("t=p:x=-1:y=0:X=6:Y=4:o=1024", label.as_bytes(), false)
}

pub(in crate::terminal_runtime) fn present_drag_icon_png_sequence(
    width: u32,
    height: u32,
    png: &[u8],
) -> String {
    present_drag_payload_sequence(&format!("t=p:x=-1:y=100:X={width}:Y={height}"), png, false)
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

pub(in crate::terminal_runtime) fn uri_list_payload(paths: &[PathBuf]) -> Vec<u8> {
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
#[path = "tests/outgoing.rs"]
mod tests;
