use super::*;

#[test]
fn xhtml_preserves_unicode_text_and_cdata() {
    let blocks = extract_xhtml_text_blocks(
        "<html><body><p>café 日本語</p><p><![CDATA[naïve 日本語]]></p></body></html>",
    );
    assert_eq!(blocks, vec!["café 日本語", "naïve 日本語"]);
}
