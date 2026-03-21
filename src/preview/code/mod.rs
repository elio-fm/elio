mod backends;
pub(crate) mod custom;
pub(crate) mod registry;
mod render;
mod syntax_manifest;

pub(crate) use self::render::render_code_preview;
