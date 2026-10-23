use super::*;
use crossterm::event::{Event, KeyCode, KeyModifiers};

#[test]
fn bracketed_paste_is_one_terminal_paste_event() {
    let (sender, receiver) = mpsc::channel();
    let input: Vec<u16> = "\x1b[200~C:\\Users\\Miguel\\caf\u{00e9}.txt\x1b[201~"
        .encode_utf16()
        .collect();
    let mut buffer = Vec::new();
    let mut pending = None;

    for unit in input {
        append_utf16_as_utf8(&mut buffer, &mut pending, &[unit]);
        parse_buffer(&mut buffer, &sender, true);
    }

    assert!(buffer.is_empty());
    assert_eq!(
        receiver.try_recv().unwrap().unwrap(),
        InputEvent::Terminal(Event::Paste(
            "C:\\Users\\Miguel\\caf\u{00e9}.txt".to_string()
        ))
    );
    assert!(receiver.try_recv().is_err());
}

#[test]
fn utf16_input_keeps_unicode_across_read_boundaries() {
    let mut bytes = Vec::new();
    let mut pending = None;
    let units: Vec<u16> = "\u{1f4c2}".encode_utf16().collect();

    append_utf16_as_utf8(&mut bytes, &mut pending, &units[..1]);
    assert!(bytes.is_empty());
    append_utf16_as_utf8(&mut bytes, &mut pending, &units[1..]);

    assert_eq!(String::from_utf8(bytes).unwrap(), "\u{1f4c2}");
}

#[test]
fn ordinary_vt_key_input_stays_a_key_event() {
    let (sender, receiver) = mpsc::channel();
    let mut buffer = b"j".to_vec();

    parse_buffer(&mut buffer, &sender, false);

    assert_eq!(
        receiver.try_recv().unwrap().unwrap(),
        InputEvent::Terminal(Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('j'),
            KeyModifiers::NONE,
        )))
    );
}
