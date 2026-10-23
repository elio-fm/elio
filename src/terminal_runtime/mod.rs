mod cd_on_exit;
#[cfg(any(unix, windows))]
mod crossterm_parser;
mod input_reader;
mod kitty_dnd;
mod shell_here;
mod tui_drawing;
mod tui_event_loop;
#[cfg(windows)]
mod windows_vt_input;
mod zoxide;

#[cfg(all(unix, any(target_os = "linux", target_os = "freebsd")))]
pub(crate) use tui_event_loop::run_portal_chooser;
pub(crate) use tui_event_loop::run_with_startup_state;
