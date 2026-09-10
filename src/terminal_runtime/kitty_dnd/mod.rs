#[cfg(unix)]
mod crossterm_parser;
#[cfg(unix)]
mod drag_and_drop;
#[cfg(unix)]
mod drag_card;
#[cfg(unix)]
mod incoming;
#[cfg(unix)]
mod input_reader;
#[cfg(unix)]
mod outgoing;
#[cfg(unix)]
mod support;

#[cfg(unix)]
pub(super) use drag_and_drop::{PendingDragOut, PendingDropIn, handle_event};
#[cfg(unix)]
pub(in crate::terminal_runtime) use drag_card::{prewarm_drag_image_renderer, render_drag_image};
#[cfg(unix)]
pub(in crate::terminal_runtime) use incoming::{KittyDndEvent, Osc72State, parse_osc72_with_state};
#[cfg(unix)]
pub(in crate::terminal_runtime) use input_reader::InputReader;
#[cfg(unix)]
pub(super) use outgoing::{
    DndOperation, DropFinish, accept_drop_sequence, agree_drag_sequence, cancel_drag_sequence,
    disable_sequence, drag_data_error_sequence, finish_drop_sequence, present_drag_data_sequence,
    present_drag_icon_png_sequence, present_drag_icon_sequence, reject_drop_sequence,
    request_drop_data_sequence, send_drag_data_sequence, start_drag_sequence, startup_sequence,
    uri_list_payload,
};
#[cfg(unix)]
pub(super) use support::{KittyDndRuntime, detect_kitty_dnd_runtime};

#[cfg(not(unix))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_runtime) struct KittyDndRuntime;

#[cfg(not(unix))]
impl KittyDndRuntime {
    pub(in crate::terminal_runtime) const fn is_enabled(&self) -> bool {
        false
    }

    pub(in crate::terminal_runtime) const fn drag_machine_id(&self) -> Option<&str> {
        None
    }
}

#[cfg(not(unix))]
pub(in crate::terminal_runtime) const fn detect_kitty_dnd_runtime() -> KittyDndRuntime {
    KittyDndRuntime
}

#[cfg(not(unix))]
pub(in crate::terminal_runtime) const fn disable_sequence() -> &'static str {
    ""
}

#[cfg(not(unix))]
pub(in crate::terminal_runtime) const fn startup_sequence(
    _machine_id: Option<&str>,
) -> &'static str {
    ""
}
