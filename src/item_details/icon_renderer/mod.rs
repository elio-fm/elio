#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
mod fontconfig;
mod renderer;
#[cfg(windows)]
mod windows;

pub(crate) use renderer::prewarm_icon_renderer;
pub(crate) use renderer::{render_icon_png, render_icon_png_for_cell_area};

#[cfg(test)]
pub(in crate::item_details) use renderer::{is_private_use_character, write_png};
