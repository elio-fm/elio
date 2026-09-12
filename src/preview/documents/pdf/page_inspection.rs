use anyhow::{Context, Result};
use std::{path::Path, process::Command};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PdfPageDimensions {
    pub(crate) width_pts: f32,
    pub(crate) height_pts: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct PdfProbeResult {
    pub(crate) total_pages: Option<usize>,
    pub(crate) width_pts: Option<f32>,
    pub(crate) height_pts: Option<f32>,
}

pub(crate) fn probe_pdf_page(path: &Path, page: usize) -> Result<PdfProbeResult> {
    let output = Command::new("pdfinfo")
        .arg("-f")
        .arg(page.to_string())
        .arg("-l")
        .arg(page.to_string())
        .arg(path)
        .output()
        .context("failed to start pdfinfo")?;
    if !output.status.success() {
        anyhow::bail!("pdfinfo exited with {}", output.status);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let dimensions = parse_pdfinfo_page_dimensions(&stdout);
    Ok(PdfProbeResult {
        total_pages: parse_pdfinfo_page_count(&stdout),
        width_pts: dimensions.map(|dimensions| dimensions.width_pts),
        height_pts: dimensions.map(|dimensions| dimensions.height_pts),
    })
}

fn parse_pdfinfo_page_count(output: &str) -> Option<usize> {
    output.lines().find_map(|line| {
        let (label, value) = line.split_once(':')?;
        (label.trim() == "Pages")
            .then_some(value.trim())
            .and_then(|value| value.parse().ok())
    })
}

fn parse_pdfinfo_page_dimensions(output: &str) -> Option<PdfPageDimensions> {
    output.lines().find_map(|line| {
        let (label, value) = line.split_once(':')?;
        let label = label.trim();
        if !(label == "Page size" || label.starts_with("Page ") && label.ends_with(" size")) {
            return None;
        }

        let mut parts = value.split_whitespace();
        let width_pts = parts.next()?.parse().ok()?;
        let _separator = parts.next()?;
        let height_pts = parts.next()?.parse().ok()?;
        Some(PdfPageDimensions {
            width_pts,
            height_pts,
        })
    })
}

#[cfg(test)]
#[path = "tests/page_inspection.rs"]
mod tests;
