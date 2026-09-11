use super::*;

#[test]
fn parse_fc_scan_output_extracts_clean_metadata() {
    let output = "\
JetBrainsMono Nerd Font,JetBrainsMono NF
Regular
TrueType
SFNT
100
True
";

    let metadata = parse_fc_scan_output(output).expect("expected parsed fc-scan metadata");

    assert_eq!(metadata.family.as_deref(), Some("JetBrainsMono Nerd Font"));
    assert_eq!(metadata.style.as_deref(), Some("Regular"));
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
