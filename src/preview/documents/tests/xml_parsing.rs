use super::*;

#[test]
fn xml_attributes_preserve_unicode_and_normalize_references() {
    let mut reader = Reader::from_str("<item ns:title=\" 日本語\tA&amp;B&#10;C \"/>");
    let Event::Empty(event) = reader.read_event().unwrap() else {
        panic!("expected item")
    };
    assert_eq!(
        xml_attribute_value(&event, "title").as_deref(),
        Some("日本語 A&B\nC")
    );
    assert_eq!(xml_attribute_value(&event, "missing"), None);
}

#[test]
fn xml_text_fields_preserve_unicode_and_raw_line_endings() {
    let fields = parse_xml_text_fields(
        "<root><dc:title> 日本語\r\ncafé </dc:title><document-statistic pages=\"&#50;\"/></root>",
    );
    assert_eq!(
        fields.get("title").map(String::as_str),
        Some("日本語\r\ncafé")
    );
    assert_eq!(fields.get("pages").map(String::as_str), Some("2"));
}
