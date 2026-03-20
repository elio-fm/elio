use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use std::sync::OnceLock;
use syntect::{
    easy::ScopeRangeIterator,
    parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet},
};

// Enable only language families that have been validated against the current bundled syntax set.
const ENABLED_SYNTAXES: &[&str] = &[
    "javascript",
    "jsx",
    "typescript",
    "tsx",
    "rust",
    "go",
    "c",
    "cpp",
    "java",
    "php",
    "python",
    "ruby",
    "lua",
    "make",
    "sh",
    "bash",
    "zsh",
    "ksh",
    "fish",
    "html",
    "xml",
    "css",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SemanticRole {
    Fg,
    Comment,
    String,
    Constant,
    Keyword,
    Function,
    Type,
    Parameter,
    Tag,
    Operator,
    Macro,
    Invalid,
}

struct ScopeSelectors {
    comment: [Scope; 1],
    string: [Scope; 1],
    constant: [Scope; 2],
    keyword: [Scope; 2],
    function: [Scope; 3],
    type_name: [Scope; 4],
    parameter: [Scope; 3],
    tag: [Scope; 3],
    operator: [Scope; 3],
    macro_name: [Scope; 4],
    invalid: [Scope; 2],
    variable_readwrite: [Scope; 1],
}

pub(in crate::preview::code) fn is_enabled(code_syntax: &str) -> bool {
    ENABLED_SYNTAXES.contains(&code_syntax)
}

pub(in crate::preview::code) fn render_syntect_code_preview<F>(
    code_syntax: &str,
    text: &str,
    line_numbers: bool,
    line_limit: usize,
    canceled: &F,
) -> Result<Vec<Line<'static>>, ()>
where
    F: Fn() -> bool,
{
    let syntax_set = syntax_set();
    let Some(syntax) = find_syntax(syntax_set, code_syntax) else {
        return Err(());
    };

    let source_lines = crate::preview::collect_preview_lines_with_limit(
        text,
        crate::preview::clamp_code_preview_line_limit(line_limit),
    );
    let number_width = crate::preview::line_number_width(source_lines.len());
    let code_palette = crate::ui::theme::code_preview_palette();
    let mut parse_state = ParseState::new(syntax);
    let mut scope_stack = ScopeStack::new();
    let mut rendered = Vec::new();

    for (index, line) in source_lines.iter().enumerate() {
        if canceled() {
            break;
        }

        let mut spans = Vec::new();
        if line_numbers {
            spans.push(crate::preview::line_number_span(index + 1, number_width));
        } else {
            spans.push(Span::styled(
                "│ ",
                Style::default().fg(code_palette.line_number),
            ));
        }

        let ops = parse_state.parse_line(line, syntax_set).map_err(|_| ())?;
        for (range, op) in ScopeRangeIterator::new(&ops, line) {
            scope_stack.apply(op).map_err(|_| ())?;
            let text = &line[range];
            if text.is_empty() {
                continue;
            }
            spans.push(syntect_span(text, scope_stack.as_slice(), code_palette));
        }
        rendered.push(Line::from(spans));
    }

    if rendered.is_empty() && !canceled() {
        rendered.push(Line::from("File is empty"));
    }

    Ok(rendered)
}

fn syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn scope_selectors() -> &'static ScopeSelectors {
    static SELECTORS: OnceLock<ScopeSelectors> = OnceLock::new();
    SELECTORS.get_or_init(|| {
        let scope = |value| Scope::new(value).expect("valid syntect scope selector");
        ScopeSelectors {
            comment: [scope("comment")],
            string: [scope("string")],
            constant: [scope("constant"), scope("support.constant")],
            keyword: [scope("keyword"), scope("storage")],
            function: [
                scope("entity.name.function"),
                scope("support.function"),
                scope("variable.function"),
            ],
            type_name: [
                scope("entity.name.type"),
                scope("entity.name.class"),
                scope("support.type"),
                scope("support.class"),
            ],
            parameter: [
                scope("variable.parameter"),
                scope("entity.other.attribute-name"),
                scope("variable.other.readwrite.assignment"),
            ],
            tag: [
                scope("entity.name.tag"),
                scope("meta.tag"),
                scope("punctuation.definition.tag"),
            ],
            operator: [
                scope("keyword.operator"),
                scope("punctuation.separator.key-value"),
                scope("punctuation.accessor"),
            ],
            macro_name: [
                scope("entity.name.function.preprocessor"),
                scope("support.function.preprocessor"),
                scope("meta.preprocessor"),
                scope("keyword.directive"),
            ],
            invalid: [scope("invalid"), scope("invalid.deprecated")],
            variable_readwrite: [scope("variable.other.readwrite")],
        }
    })
}

fn find_syntax<'a>(syntax_set: &'a SyntaxSet, code_syntax: &str) -> Option<&'a SyntaxReference> {
    let lookup_token = syntect_lookup_token(code_syntax);
    syntax_set
        .find_syntax_by_token(lookup_token)
        .or_else(|| syntax_set.find_syntax_by_extension(lookup_token))
}

fn syntect_lookup_token(code_syntax: &str) -> &str {
    match code_syntax {
        // The stock syntect dump includes JavaScript, but not dedicated TypeScript / TSX syntaxes.
        // Route the whole JS family through the JavaScript grammar until a curated bundle is added.
        "javascript" | "jsx" | "typescript" | "tsx" => "js",
        "rust" => "rs",
        "kotlin" => "kt",
        "ruby" => "rb",
        "python" => "py",
        "bash" => "bash",
        "zsh" | "ksh" => "sh",
        "make" => "makefile",
        _ => code_syntax,
    }
}

fn syntect_span(
    text: &str,
    scope_stack: &[Scope],
    palette: crate::ui::theme::CodePreviewPalette,
) -> Span<'static> {
    let role = semantic_role_for_token(text, scope_stack);
    let mut rendered_style = Style::default().fg(role_color(role, palette));
    if role == SemanticRole::Invalid {
        rendered_style = rendered_style.add_modifier(Modifier::UNDERLINED);
    }

    Span::styled(crate::preview::expand_tabs(text), rendered_style)
}

fn semantic_role_for_token(text: &str, scope_stack: &[Scope]) -> SemanticRole {
    let selectors = scope_selectors();

    if scope_stack_matches(scope_stack, &selectors.invalid) {
        SemanticRole::Invalid
    } else if scope_stack_matches(scope_stack, &selectors.comment) {
        SemanticRole::Comment
    } else if scope_stack_matches(scope_stack, &selectors.string) {
        SemanticRole::String
    } else if scope_stack_matches(scope_stack, &selectors.macro_name) {
        SemanticRole::Macro
    } else if scope_stack_matches(scope_stack, &selectors.parameter) {
        SemanticRole::Parameter
    } else if scope_stack_matches(scope_stack, &selectors.tag) {
        SemanticRole::Tag
    } else if scope_stack_matches(scope_stack, &selectors.function) {
        SemanticRole::Function
    } else if scope_stack_matches(scope_stack, &selectors.type_name) {
        SemanticRole::Type
    } else if scope_stack_matches(scope_stack, &selectors.variable_readwrite)
        && text.chars().next().is_some_and(char::is_uppercase)
    {
        SemanticRole::Type
    } else if scope_stack_matches(scope_stack, &selectors.keyword) {
        SemanticRole::Keyword
    } else if scope_stack_matches(scope_stack, &selectors.operator) {
        SemanticRole::Operator
    } else if scope_stack_matches(scope_stack, &selectors.constant) {
        SemanticRole::Constant
    } else {
        SemanticRole::Fg
    }
}

fn scope_stack_matches(scope_stack: &[Scope], selectors: &[Scope]) -> bool {
    scope_stack.iter().rev().any(|scope| {
        selectors
            .iter()
            .any(|selector| selector.is_prefix_of(*scope))
    })
}

fn role_color(
    role: SemanticRole,
    palette: crate::ui::theme::CodePreviewPalette,
) -> ratatui::style::Color {
    match role {
        SemanticRole::Fg => palette.fg,
        SemanticRole::Comment => palette.comment,
        SemanticRole::String => palette.string,
        SemanticRole::Constant => palette.constant,
        SemanticRole::Keyword => palette.keyword,
        SemanticRole::Function => palette.function,
        SemanticRole::Type => palette.r#type,
        SemanticRole::Parameter => palette.parameter,
        SemanticRole::Tag => palette.tag,
        SemanticRole::Operator => palette.operator,
        SemanticRole::Macro => palette.r#macro,
        SemanticRole::Invalid => palette.invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme;
    use std::str::FromStr;

    fn span_color(line: &Line<'_>, token: &str) -> Option<ratatui::style::Color> {
        line.spans
            .iter()
            .find(|span| span.content.contains(token))
            .and_then(|span| span.style.fg)
    }

    fn palette_colors() -> Vec<ratatui::style::Color> {
        let palette = theme::code_preview_palette();
        vec![
            palette.fg,
            palette.bg,
            palette.selection_bg,
            palette.selection_fg,
            palette.caret,
            palette.line_highlight,
            palette.line_number,
            palette.comment,
            palette.string,
            palette.constant,
            palette.keyword,
            palette.function,
            palette.r#type,
            palette.parameter,
            palette.tag,
            palette.operator,
            palette.r#macro,
            palette.invalid,
        ]
    }

    #[test]
    fn bundled_syntaxes_cover_initial_canaries() {
        let syntax_set = syntax_set();

        for code_syntax in ENABLED_SYNTAXES {
            assert!(
                find_syntax(syntax_set, code_syntax).is_some(),
                "missing syntect syntax for {code_syntax}"
            );
        }
    }

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
    fn missing_bundled_syntaxes_return_errors_for_safe_fallback() {
        for code_syntax in ["nix", "cmake", "kotlin", "swift"] {
            assert!(
                render_syntect_code_preview(code_syntax, "sample\n", true, 20, &|| false).is_err(),
                "expected {code_syntax} to fail until a curated syntect bundle is added"
            );
        }
    }

    #[test]
    fn enabled_syntaxes_are_routed_to_syntect() {
        for code_syntax in ENABLED_SYNTAXES {
            assert!(
                is_enabled(code_syntax),
                "expected {code_syntax} to be enabled"
            );
        }
    }

    #[test]
    fn rendered_syntect_colors_only_use_elio_code_palette() {
        let allowed = palette_colors();
        let rendered = render_syntect_code_preview(
            "rust",
            "fn main() {\n    let answer = 42;\n    println!(\"hi\"); // note\n}\n",
            true,
            20,
            &|| false,
        )
        .expect("rust syntax should render through syntect");

        for line in &rendered {
            for span in &line.spans {
                if let Some(color) = span.style.fg {
                    assert!(
                        allowed.contains(&color),
                        "found non-Elio syntect color {color:?} in span {:?}",
                        span.content
                    );
                }
            }
        }
    }

    #[test]
    fn rendered_syntect_tokens_map_to_elio_semantic_roles() {
        let palette = theme::code_preview_palette();
        let rust = render_syntect_code_preview(
            "rust",
            "fn main() {\n    let answer = 42;\n    println!(\"hi\"); // note\n}\n",
            true,
            20,
            &|| false,
        )
        .expect("rust syntax should render through syntect");
        assert_eq!(span_color(&rust[0], "fn"), Some(palette.keyword));
        assert_eq!(span_color(&rust[1], "42"), Some(palette.constant));
        assert!(
            rust[2]
                .spans
                .iter()
                .any(|span| span.style.fg == Some(palette.string)),
            "expected a string-colored span in {:?}",
            rust[2]
        );
        assert!(
            rust[2]
                .spans
                .iter()
                .any(|span| span.style.fg == Some(palette.comment)),
            "expected a comment-colored span in {:?}",
            rust[2]
        );

        let html = render_syntect_code_preview(
            "html",
            "<div class=\"app\">elio</div>\n",
            true,
            20,
            &|| false,
        )
        .expect("html syntax should render through syntect");
        assert_eq!(span_color(&html[0], "div"), Some(palette.tag));
        assert_eq!(span_color(&html[0], "class"), Some(palette.parameter));
    }

    #[test]
    fn semantic_role_classifier_covers_expected_scope_families() {
        let stack = ScopeStack::from_str("source.rust keyword.control.rust").unwrap();
        assert_eq!(
            semantic_role_for_token("if", stack.as_slice()),
            SemanticRole::Keyword
        );

        let stack =
            ScopeStack::from_str("text.html.basic meta.tag entity.other.attribute-name.html")
                .unwrap();
        assert_eq!(
            semantic_role_for_token("class", stack.as_slice()),
            SemanticRole::Parameter
        );

        let stack = ScopeStack::from_str(
            "source.c meta.preprocessor.include entity.name.function.preprocessor",
        )
        .unwrap();
        assert_eq!(
            semantic_role_for_token("include", stack.as_slice()),
            SemanticRole::Macro
        );

        let stack =
            ScopeStack::from_str("source.shell.bash variable.other.readwrite.assignment.shell")
                .unwrap();
        assert_eq!(
            semantic_role_for_token("MAKE", stack.as_slice()),
            SemanticRole::Parameter
        );

        let stack = ScopeStack::from_str("source.js variable.other.readwrite.js").unwrap();
        assert_eq!(
            semantic_role_for_token("Greeter", stack.as_slice()),
            SemanticRole::Type
        );
    }
}
