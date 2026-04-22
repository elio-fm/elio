use super::PreviewContent;
use super::PreviewKind;
use super::appearance as theme;
use super::process::run_command_capture_stdout_cancellable;
use crate::core::Entry;
use anyhow::{Context, Result};
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use std::{
    fs,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::OnceLock,
};

const FC_SCAN_FORMAT: &str = "%{family}\n%{style}\n%{postscriptname}\n%{fontformat}\n%{fontwrapper}\n%{spacing}\n%{variable}\n";
const WOFF_MAGIC: &[u8; 4] = b"wOFF";
const WOFF2_MAGIC: &[u8; 4] = b"wOF2";
const OTTO_MAGIC: &[u8; 4] = b"OTTO";
const TRUE_TYPE_MAGIC: [u8; 4] = [0x00, 0x01, 0x00, 0x00];
const APPLE_TRUE_TYPE_MAGIC: &[u8; 4] = b"true";
const TYPE_1_MAGIC: &[u8; 4] = b"typ1";

#[derive(Debug, Eq, PartialEq)]
struct FontMetadata {
    family: Option<String>,
    style: Option<String>,
    postscript: Option<String>,
    format: String,
    monospace: bool,
    variable: bool,
    file_size: String,
}

#[derive(Debug, Eq, PartialEq)]
struct FcScanMetadata {
    family: Option<String>,
    style: Option<String>,
    postscript: Option<String>,
    font_format: Option<String>,
    wrapper: Option<String>,
    monospace: bool,
    variable: bool,
}

pub(super) fn build_font_preview<F>(
    entry: &Entry,
    type_detail: Option<&'static str>,
    canceled: &F,
) -> Result<PreviewContent>
where
    F: Fn() -> bool,
{
    let detail = type_detail.unwrap_or("Font");
    let metadata = fs::metadata(&entry.path)
        .with_context(|| format!("failed to read metadata for {}", entry.path.display()))?;
    let byte_size = metadata.len();
    let header = read_font_header(&entry.path)?;
    let fallback_format = detect_font_format_from_header(&header, type_detail);
    let scan_metadata = (byte_size >= 12)
        .then(|| fc_scan_metadata(&entry.path, canceled))
        .flatten();
    let preview_metadata = FontMetadata {
        family: scan_metadata.as_ref().and_then(|scan| scan.family.clone()),
        style: scan_metadata.as_ref().and_then(|scan| scan.style.clone()),
        postscript: scan_metadata
            .as_ref()
            .and_then(|scan| scan.postscript.clone()),
        format: scan_metadata
            .as_ref()
            .and_then(|scan| {
                normalize_format(
                    scan.wrapper.as_deref(),
                    scan.font_format.as_deref(),
                    type_detail,
                )
            })
            .unwrap_or(fallback_format),
        monospace: scan_metadata.as_ref().is_some_and(|scan| scan.monospace),
        variable: scan_metadata.as_ref().is_some_and(|scan| scan.variable),
        file_size: crate::fs::format_size(byte_size),
    };

    Ok(render_font_preview(detail, preview_metadata))
}

fn fc_scan_metadata<F>(path: &Path, canceled: &F) -> Option<FcScanMetadata>
where
    F: Fn() -> bool,
{
    if canceled() || !fc_scan_available() {
        return None;
    }

    let mut command = Command::new("fc-scan");
    command.arg("--format").arg(FC_SCAN_FORMAT).arg(path);
    let output = run_command_capture_stdout_cancellable(command, "preview-font-fc-scan", canceled)?;
    parse_fc_scan_output(&String::from_utf8_lossy(&output))
}

fn fc_scan_available() -> bool {
    static FC_SCAN_AVAILABLE: OnceLock<bool> = OnceLock::new();
    *FC_SCAN_AVAILABLE.get_or_init(|| {
        Command::new("fc-scan")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    })
}

fn parse_fc_scan_output(output: &str) -> Option<FcScanMetadata> {
    let mut lines = output.lines();
    let family = primary_name_value(lines.next().unwrap_or_default());
    let style = primary_name_value(lines.next().unwrap_or_default());
    let postscript = clean_fc_scan_value(lines.next().unwrap_or_default());
    let font_format = clean_fc_scan_value(lines.next().unwrap_or_default());
    let wrapper = clean_fc_scan_value(lines.next().unwrap_or_default());
    let monospace = is_monospace_spacing(lines.next().unwrap_or_default());
    let variable = clean_fc_scan_value(lines.next().unwrap_or_default())
        .is_some_and(|value| value.eq_ignore_ascii_case("true"));

    let metadata = FcScanMetadata {
        family,
        style,
        postscript,
        font_format,
        wrapper,
        monospace,
        variable,
    };
    if metadata.family.is_none()
        && metadata.style.is_none()
        && metadata.postscript.is_none()
        && metadata.font_format.is_none()
        && metadata.wrapper.is_none()
        && !metadata.monospace
        && !metadata.variable
    {
        None
    } else {
        Some(metadata)
    }
}

fn clean_fc_scan_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value == "(null)" {
        None
    } else {
        Some(value.to_string())
    }
}

fn primary_name_value(value: &str) -> Option<String> {
    value
        .split(',')
        .map(str::trim)
        .find(|family| !family.is_empty())
        .map(str::to_string)
}

fn is_monospace_spacing(value: &str) -> bool {
    matches!(value.trim(), "100" | "110")
}

fn read_font_header(path: &Path) -> Result<Vec<u8>> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut header = [0u8; 8];
    let read = file
        .read(&mut header)
        .with_context(|| format!("failed to read {}", path.display()))?;
    Ok(header[..read].to_vec())
}

fn detect_font_format_from_header(header: &[u8], type_detail: Option<&'static str>) -> String {
    if header.starts_with(WOFF_MAGIC) {
        return woff_wrapper_format("WOFF", header.get(4..8));
    }
    if header.starts_with(WOFF2_MAGIC) {
        return woff_wrapper_format("WOFF2", header.get(4..8));
    }
    if header.starts_with(OTTO_MAGIC) {
        return "OpenType (CFF)".to_string();
    }
    if is_true_type_flavor(header) {
        return true_type_outline_format(type_detail).to_string();
    }
    format_from_type_detail(type_detail).to_string()
}

fn woff_wrapper_format(wrapper: &str, flavor: Option<&[u8]>) -> String {
    match flavor {
        Some(flavor) if flavor == OTTO_MAGIC => format!("{wrapper} (CFF)"),
        Some(flavor) if is_true_type_flavor(flavor) => format!("{wrapper} (TrueType)"),
        _ => wrapper.to_string(),
    }
}

fn normalize_format(
    wrapper: Option<&str>,
    font_format: Option<&str>,
    type_detail: Option<&'static str>,
) -> Option<String> {
    let wrapper = wrapper.map(str::trim).filter(|value| !value.is_empty());
    let font_format = font_format.map(str::trim).filter(|value| !value.is_empty());

    match (wrapper, font_format) {
        (Some("WOFF"), Some("CFF")) => Some("WOFF (CFF)".to_string()),
        (Some("WOFF"), Some("TrueType")) => Some("WOFF (TrueType)".to_string()),
        (Some("WOFF2"), Some("CFF")) => Some("WOFF2 (CFF)".to_string()),
        (Some("WOFF2"), Some("TrueType")) => Some("WOFF2 (TrueType)".to_string()),
        (Some("WOFF"), _) => Some("WOFF".to_string()),
        (Some("WOFF2"), _) => Some("WOFF2".to_string()),
        (Some("SFNT"), Some("CFF")) => Some("OpenType (CFF)".to_string()),
        (Some("SFNT"), Some("TrueType")) => Some(true_type_outline_format(type_detail).to_string()),
        (Some("SFNT"), _) => Some(format_from_type_detail(type_detail).to_string()),
        (Some(wrapper), Some(format)) if wrapper.eq_ignore_ascii_case(format) => {
            Some(wrapper.to_string())
        }
        (Some(wrapper), Some(format)) => Some(format!("{wrapper} ({format})")),
        (Some(wrapper), None) => Some(wrapper.to_string()),
        (None, Some("CFF")) => Some("OpenType (CFF)".to_string()),
        (None, Some("TrueType")) => Some(true_type_outline_format(type_detail).to_string()),
        (None, Some(format)) => Some(format.to_string()),
        (None, None) => None,
    }
}

fn format_from_type_detail(type_detail: Option<&'static str>) -> &'static str {
    match type_detail {
        Some("TrueType font") => "TrueType",
        Some("OpenType font") => "OpenType",
        Some("WOFF font") => "WOFF",
        Some("WOFF2 font") => "WOFF2",
        _ => "Font",
    }
}

fn true_type_outline_format(type_detail: Option<&'static str>) -> &'static str {
    match type_detail {
        Some("OpenType font") => "OpenType (TrueType)",
        _ => "TrueType",
    }
}

fn is_true_type_flavor(value: &[u8]) -> bool {
    value == TRUE_TYPE_MAGIC || value == APPLE_TRUE_TYPE_MAGIC || value == TYPE_1_MAGIC
}

fn render_font_preview(detail: &str, metadata: FontMetadata) -> PreviewContent {
    let palette = theme::palette();
    let mut fields = Vec::new();
    if let Some(family) = metadata.family {
        fields.push(("Family", family));
    }
    if let Some(style) = metadata.style {
        fields.push(("Style", style));
    }
    if let Some(postscript) = metadata.postscript {
        fields.push(("PostScript", postscript));
    }
    fields.push(("Format", metadata.format));
    if metadata.monospace {
        fields.push(("Monospace", "Yes".to_string()));
    }
    if metadata.variable {
        fields.push(("Variable", "Yes".to_string()));
    }
    fields.push(("File Size", metadata.file_size));

    let label_width = fields
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(8);
    let mut lines = vec![preview_section_line("Details", palette)];
    for (label, value) in fields {
        lines.push(preview_field_line(label, &value, label_width, palette));
    }

    PreviewContent::new(PreviewKind::Font, lines).with_detail(detail)
}

fn preview_section_line(title: &str, palette: theme::Palette) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default().fg(palette.accent),
    ))
}

fn preview_field_line(
    label: &str,
    value: &str,
    label_width: usize,
    palette: theme::Palette,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<width$} ", width = label_width + 1),
            Style::default().fg(palette.muted),
        ),
        Span::styled(value.to_string(), Style::default().fg(palette.text)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fc_scan_output_extracts_clean_metadata() {
        let output = "\
JetBrainsMono Nerd Font,JetBrainsMono NF
Regular
JetBrainsMonoNF-Regular
TrueType
SFNT
100
True
";

        let metadata = parse_fc_scan_output(output).expect("expected parsed fc-scan metadata");

        assert_eq!(metadata.family.as_deref(), Some("JetBrainsMono Nerd Font"));
        assert_eq!(metadata.style.as_deref(), Some("Regular"));
        assert_eq!(
            metadata.postscript.as_deref(),
            Some("JetBrainsMonoNF-Regular")
        );
        assert_eq!(metadata.font_format.as_deref(), Some("TrueType"));
        assert_eq!(metadata.wrapper.as_deref(), Some("SFNT"));
        assert!(metadata.monospace);
        assert!(metadata.variable);
    }

    #[test]
    fn normalize_format_prefers_wrapper_specific_labels() {
        assert_eq!(
            normalize_format(Some("WOFF2"), Some("TrueType"), Some("WOFF2 font")),
            Some("WOFF2 (TrueType)".to_string())
        );
        assert_eq!(
            normalize_format(Some("SFNT"), Some("CFF"), Some("OpenType font")),
            Some("OpenType (CFF)".to_string())
        );
        assert_eq!(
            normalize_format(Some("SFNT"), Some("TrueType"), Some("OpenType font")),
            Some("OpenType (TrueType)".to_string())
        );
    }

    #[test]
    fn parse_fc_scan_output_keeps_only_primary_style_name() {
        let output = "\
TypoGraphica
Regular ,Normal, obyéejné, Standard, Kavov ika, Normaali
TypoGraphica
TrueType
SFNT

False
";

        let metadata = parse_fc_scan_output(output).expect("expected parsed fc-scan metadata");

        assert_eq!(metadata.style.as_deref(), Some("Regular"));
    }

    #[test]
    fn detect_font_format_from_header_uses_magic_wrappers() {
        assert_eq!(
            detect_font_format_from_header(b"wOFF\x00\x01\x00\x00", Some("WOFF font")),
            "WOFF (TrueType)"
        );
        assert_eq!(
            detect_font_format_from_header(b"wOF2OTTO", Some("WOFF2 font")),
            "WOFF2 (CFF)"
        );
        assert_eq!(
            detect_font_format_from_header(b"OTTOrest", Some("OpenType font")),
            "OpenType (CFF)"
        );
        assert_eq!(
            detect_font_format_from_header(b"\x00\x01\x00\x00rest", Some("TrueType font")),
            "TrueType"
        );
    }
}
