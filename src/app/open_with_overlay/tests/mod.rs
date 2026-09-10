use crate::opening::open_with::OpenWithApplication;
use std::time::{SystemTime, UNIX_EPOCH};

mod input;
mod overlay;

fn temp_dir_path(label: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-open-with-{label}-{unique}"))
}

fn fake_open_with_app(display_name: &str) -> OpenWithApplication {
    OpenWithApplication {
        display_name: display_name.to_string(),
        application_id: None,
        program: "fake".to_string(),
        args: vec!["--arg".to_string()],
        is_default: true,
        requires_terminal: false,
    }
}

fn fake_terminal_app(display_name: &str) -> OpenWithApplication {
    OpenWithApplication {
        display_name: display_name.to_string(),
        application_id: None,
        program: "nvim".to_string(),
        args: vec!["/tmp/file.txt".to_string()],
        is_default: false,
        requires_terminal: true,
    }
}
