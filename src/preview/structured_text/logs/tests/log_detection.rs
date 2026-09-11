use super::super::render_log_preview;

#[test]
fn unstructured_logs_return_none_for_structured_rendering() {
    assert!(render_log_preview("starting application\nloading configuration\nready\n").is_none());
}
