use super::super::{dispatcher, freedesktop, wayland::ReadResult};

#[test]
fn current_wayland_image_selection_is_definitively_empty() {
    // An offered image has a current native selection, but no supported item
    // format. Its Empty result must not make stale XWayland content eligible.
    assert_eq!(
        dispatcher::native_result_for_test::<String>(ReadResult::Empty),
        None
    );
    assert!(!dispatcher::native_fallback_allowed_for_test::<String>(
        &ReadResult::Empty
    ));
}

#[test]
fn current_wayland_value_wins_over_xwayland() {
    assert_eq!(
        dispatcher::native_result_for_test(ReadResult::Value("native".to_string())),
        Some("native".to_string())
    );
}

#[test]
fn unavailable_wayland_is_the_only_fallback_case() {
    assert!(dispatcher::native_fallback_allowed_for_test(
        &ReadResult::<String>::Unavailable
    ));
}

#[test]
fn native_text_publication_advertises_standard_text_mimes() {
    let payloads = freedesktop::text_payloads("hello");
    assert_eq!(
        payloads
            .iter()
            .map(|(mime, _)| mime.as_str())
            .collect::<Vec<_>>(),
        ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING"]
    );
    assert!(payloads.iter().all(|(_, payload)| payload == b"hello"));
}
