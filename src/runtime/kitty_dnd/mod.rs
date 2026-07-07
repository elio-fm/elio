#[cfg(unix)]
mod capability;
#[cfg(unix)]
mod drag_image;
#[cfg(unix)]
mod event;
#[cfg(unix)]
mod protocol;

#[cfg(unix)]
pub(super) use capability::{KittyDndRuntime, detect_kitty_dnd_runtime};
#[cfg(unix)]
pub(in crate::runtime) use drag_image::{prewarm_drag_image_renderer, render_drag_image};
#[cfg(unix)]
pub(in crate::runtime) use event::{KittyDndEvent, Osc72State, parse_osc72_with_state};
#[cfg(unix)]
pub(super) use protocol::{
    DndOperation, DropFinish, accept_drop_sequence, agree_drag_sequence, cancel_drag_sequence,
    disable_sequence, drag_data_error_sequence, finish_drop_sequence, present_drag_data_sequence,
    present_drag_icon_png_sequence, present_drag_icon_sequence, reject_drop_sequence,
    request_drop_data_sequence, send_drag_data_sequence, start_drag_sequence, startup_sequence,
    uri_list_payload,
};

#[cfg(not(unix))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct KittyDndRuntime;

#[cfg(not(unix))]
impl KittyDndRuntime {
    pub(in crate::runtime) const fn is_enabled(&self) -> bool {
        false
    }

    pub(in crate::runtime) const fn drag_machine_id(&self) -> Option<&str> {
        None
    }
}

#[cfg(not(unix))]
pub(in crate::runtime) const fn detect_kitty_dnd_runtime() -> KittyDndRuntime {
    KittyDndRuntime
}

#[cfg(not(unix))]
pub(in crate::runtime) const fn disable_sequence() -> &'static str {
    ""
}

#[cfg(not(unix))]
pub(in crate::runtime) const fn startup_sequence(_machine_id: Option<&str>) -> &'static str {
    ""
}
