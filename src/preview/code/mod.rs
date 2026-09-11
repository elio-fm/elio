mod built_in_highlighting;
mod code_rendering;
mod highlighting_styles;
mod plain_code;
pub(in crate::preview) mod syntect_highlighting;

pub(crate) use self::code_rendering::render_code_preview;
