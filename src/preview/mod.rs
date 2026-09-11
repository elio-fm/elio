mod appearance;
mod archives;
mod audio;
mod binary;
pub(crate) mod code;
mod directory;
mod documents;
pub(crate) mod external_commands;
mod font;
mod images;
mod markdown;
mod plain_text;
mod preview_building;
mod preview_content;
mod structured_text;
mod tabular_data;
mod text_rendering;
mod torrent;
mod video;

pub(crate) use self::plain_text::count_total_text_lines;
use self::plain_text::{
    collect_preview_lines_with_limit, combine_preview_notes, count_source_lines,
    finalize_text_preview, finalize_text_preview_with_line_limit, read_text_preview,
    render_plain_text_preview, render_reflowed_text_preview, trim_trailing_line_endings,
    truncation_note, truncation_note_with_line_limit,
};
#[cfg(test)]
pub(crate) use self::preview_building::build_preview;
#[cfg(test)]
pub(crate) use self::preview_building::build_preview_with_options;
pub(crate) use self::preview_building::{
    build_preview_with_options_and_code_line_limit, loading_preview_for, preview_work_class,
    should_build_preview_in_background,
};
use self::preview_content::*;
pub(crate) use self::preview_content::{
    MIN_DYNAMIC_CODE_PREVIEW_LINE_LIMIT, PreviewContent, PreviewKind, PreviewLineCoverage,
    PreviewRequestOptions, PreviewVisual, PreviewVisualKind, PreviewVisualLayout, PreviewWorkClass,
    clamp_code_preview_line_limit, default_code_preview_line_limit,
};
pub(crate) use self::text_rendering::{expand_tabs, line_number_span, line_number_width};

#[cfg(test)]
mod tests;
