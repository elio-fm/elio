mod desktop_applications;
mod gio_applications;
mod mime_applications;
mod xdg_environment;

#[cfg_attr(test, allow(unused_imports))]
pub(super) use mime_applications::{applications_for, desktop_applications_for};
