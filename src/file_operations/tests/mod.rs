mod bulk_rename;
mod clipboard_test_support;
mod copy_move;
mod copy_to_clipboard;
mod create;
#[cfg(unix)]
mod editor_bulk_rename;
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
mod mutation_test_support;
mod rename;
mod restore;
#[cfg(unix)]
mod symlink_creation;
mod trash_delete;
