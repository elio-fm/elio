use super::*;

#[test]
fn direct_syntect_rendering_supports_registry_canonical_ids() {
    let rendered = render_syntect_code_preview("rust", "fn main() {}\n", true, 20, &|| false)
        .expect("rust syntax should render through syntect");

    assert!(
        rendered[0]
            .spans
            .iter()
            .any(|span| span.content.contains("fn"))
    );
    assert_eq!(
        span_color(&rendered[0], "fn"),
        Some(theme::code_preview_palette().keyword)
    );
}

#[test]
fn clojure_support_renders_through_curated_syntect_bundle() {
    let rendered = render_syntect_code_preview(
        "clojure",
        "(ns elio.core)\n(defn greet [name] (str \"hi \" name))\n",
        true,
        20,
        &|| false,
    )
    .expect("clojure syntax should render through syntect");

    assert_eq!(
        span_color(&rendered[1], "defn"),
        Some(theme::code_preview_palette().keyword)
    );
}

#[test]
fn fortran_support_renders_through_curated_syntect_bundle() {
    let rendered = render_syntect_code_preview(
        "fortran",
        "program elio\n  implicit none\n  print *, \"hello\"\nend program elio\n",
        true,
        20,
        &|| false,
    )
    .expect("fortran syntax should render through syntect");

    assert_eq!(
        span_color(&rendered[0], "program"),
        Some(theme::code_preview_palette().keyword)
    );
}

#[test]
fn cobol_support_renders_through_curated_syntect_bundle() {
    let rendered = render_syntect_code_preview(
        "cobol",
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. ELIOTEST.\n       PROCEDURE DIVISION.\n           DISPLAY \"HELLO\".\n           STOP RUN.\n",
        true,
        20,
        &|| false,
    )
    .expect("cobol syntax should render through syntect");

    assert_eq!(
        span_color(&rendered[0], "IDENTIFICATION"),
        Some(theme::code_preview_palette().keyword)
    );
}

#[test]
fn unsupported_syntaxes_return_errors_for_safe_fallback() {
    for code_syntax in ["ziggy", "brainfuck", "totally-unknown-syntax"] {
        assert!(
            render_syntect_code_preview(code_syntax, "sample\n", true, 20, &|| false).is_err(),
            "expected {code_syntax} to fall back safely"
        );
    }
}
