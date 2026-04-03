mod appearance;
mod audio;
mod binary;
pub(crate) mod code;
mod container;
mod data;
mod directory;
mod dispatch;
mod document;
mod markdown;
mod process;
mod structured;
mod text;
mod types;
mod video;

#[cfg(test)]
pub(crate) use self::dispatch::build_preview;
#[cfg(test)]
pub(crate) use self::dispatch::build_preview_with_options;
pub(crate) use self::dispatch::{
    build_preview_with_options_and_code_line_limit, loading_preview_for, preview_work_class,
    should_build_preview_in_background,
};
pub(crate) use self::text::count_total_text_lines;
use self::text::{
    collect_preview_lines_with_limit, combine_preview_notes, count_source_lines,
    finalize_text_preview, finalize_text_preview_with_line_limit, read_text_preview,
    render_plain_text_preview, render_reflowed_text_preview, trim_trailing_line_endings,
    truncation_note, truncation_note_with_line_limit,
};
use self::types::*;
pub(crate) use self::types::{
    MIN_DYNAMIC_CODE_PREVIEW_LINE_LIMIT, PreviewContent, PreviewKind, PreviewLineCoverage,
    PreviewRequestOptions, PreviewVisual, PreviewVisualKind, PreviewVisualLayout, PreviewWorkClass,
    clamp_code_preview_line_limit, default_code_preview_line_limit,
};

#[cfg(test)]
mod tests;
