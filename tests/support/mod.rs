use std::{
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) fn elio() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elio"))
}

pub(crate) fn temp_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("elio-cli-{label}-{unique}"))
}
