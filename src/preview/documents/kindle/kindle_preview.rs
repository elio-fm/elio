use super::{
    cover_images::extract_kindle_cover_visual,
    kindle_metadata::{
        MAX_KINDLE_RECORD0_BYTES, parse_kindle_database, parse_kindle_header, read_record_bytes,
        render_kindle_metadata,
    },
};
use crate::preview::documents::metadata_preview::{DocumentMetadata, render_document_preview};
use crate::{
    file_classification::DocumentFormat,
    preview::{PreviewContent, PreviewVisual},
};
use std::{fs::File, path::Path};

#[derive(Debug, Default)]
struct KindlePreviewData {
    metadata: DocumentMetadata,
    visual: Option<PreviewVisual>,
}

pub(in crate::preview::documents) fn build_kindle_preview(
    path: &Path,
    format: DocumentFormat,
) -> Option<PreviewContent> {
    File::open(path).ok()?;
    let preview = parse_kindle_preview_data(path).unwrap_or_default();
    let mut content = render_document_preview(format, preview.metadata);
    if let Some(visual) = preview.visual {
        content = content.with_preview_visual(visual);
    }
    Some(content)
}

fn parse_kindle_preview_data(path: &Path) -> Option<KindlePreviewData> {
    let mut file = File::open(path).ok()?;
    let database = parse_kindle_database(&mut file)?;
    let record0 = read_record_bytes(&mut file, &database, 0, MAX_KINDLE_RECORD0_BYTES, true)?;
    let header = parse_kindle_header(&record0).unwrap_or_default();
    let metadata = render_kindle_metadata(&database, &header);
    let visual = extract_kindle_cover_visual(path, &mut file, &database, &header);

    Some(KindlePreviewData { metadata, visual })
}
