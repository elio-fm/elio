use super::{PdfPageDimensions, PdfProbeResult, PdfRenderKey};
use anyhow::{Context, Result};
use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::SystemTime,
};

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

pub(crate) fn render_pdf_page_to_cache(
    path: &Path,
    size: u64,
    modified: Option<SystemTime>,
    page: usize,
    width_px: u32,
    height_px: u32,
) -> Result<Option<PathBuf>> {
    let key = PdfRenderKey {
        path: path.to_path_buf(),
        size,
        modified,
        page,
        width_px,
        height_px,
    };
    let cache_dir = pdf_render_cache_dir()?;
    let stem = cache_dir.join(pdf_render_cache_stem(&key));
    let png_path = stem.with_extension("png");
    if png_path.exists() {
        return Ok(Some(png_path));
    }

    let status = Command::new("pdftocairo")
        .arg("-png")
        .arg("-singlefile")
        .arg("-f")
        .arg(page.to_string())
        .arg("-l")
        .arg(page.to_string())
        .arg("-scale-to-x")
        .arg(width_px.to_string())
        .arg("-scale-to-y")
        .arg("-1")
        .arg(path)
        .arg(&stem)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to start pdftocairo")?;

    if !status.success() || !png_path.exists() {
        return Ok(None);
    }
    Ok(Some(png_path))
}

pub(super) fn parse_pdfinfo_page_count(output: &str) -> Option<usize> {
    output.lines().find_map(|line| {
        let (label, value) = line.split_once(':')?;
        (label.trim() == "Pages")
            .then_some(value.trim())
            .and_then(|value| value.parse().ok())
    })
}

pub(super) fn parse_pdfinfo_page_dimensions(output: &str) -> Option<PdfPageDimensions> {
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

fn pdf_render_cache_dir() -> Result<PathBuf> {
    let cache_dir = env::temp_dir().join("elio-pdf-preview");
    fs::create_dir_all(&cache_dir).context("failed to create PDF preview cache")?;
    Ok(cache_dir)
}

fn pdf_render_cache_stem(key: &PdfRenderKey) -> String {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    format!("page-{:016x}", hasher.finish())
}
