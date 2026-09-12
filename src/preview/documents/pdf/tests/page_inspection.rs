use super::*;

#[test]
fn parse_pdfinfo_page_count_reads_page_field() {
    assert_eq!(
        parse_pdfinfo_page_count("Title: demo\nPages: 18\nProducer: test\n"),
        Some(18)
    );
}

#[test]
fn parse_pdfinfo_page_dimensions_reads_global_and_per_page_sizes() {
    assert_eq!(
        parse_pdfinfo_page_dimensions("Page size: 595.276 x 841.89 pts (A4)\n"),
        Some(PdfPageDimensions {
            width_pts: 595.276,
            height_pts: 841.89,
        })
    );
    assert_eq!(
        parse_pdfinfo_page_dimensions("Page    2 size: 300 x 144 pts\n"),
        Some(PdfPageDimensions {
            width_pts: 300.0,
            height_pts: 144.0,
        })
    );
}
