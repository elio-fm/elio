#[test]
#[cfg(not(target_os = "macos"))]
fn fontconfig_charset_query_uses_lowercase_hex_codepoint() {
    assert_eq!(
        super::platform_fonts::fontconfig_charset_query('󰉋'),
        ":charset=f024b"
    );
    assert_eq!(
        super::platform_fonts::fontconfig_charset_query('A'),
        ":charset=41"
    );
}
