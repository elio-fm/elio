mod directive_configs;
mod ini_files;
mod log_files;
mod preview_rendering;
mod shell_scripts;
mod structured_data;
mod text_scanning;

pub(super) use self::{
    preview_rendering::render_built_in_code_preview,
    shell_scripts::{is_shell_syntax, render_shell_script},
};
use self::{
    preview_rendering::styled_text,
    text_scanning::{
        looks_numeric, scan_quoted_segment, split_comment, split_jsonc_segments,
        split_unquoted_once,
    },
};

#[cfg(test)]
mod tests;
