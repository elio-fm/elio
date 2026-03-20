#[cfg(test)]
use super::syntect_manifest::CURATED_SYNTAXES;
#[cfg(test)]
use super::syntect_manifest::CuratedSyntax;
use super::syntect_manifest::curated_syntax;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use std::sync::OnceLock;
use syntect::{
    dumps::from_uncompressed_data,
    easy::ScopeRangeIterator,
    parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet},
};

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
    shell_variable: [Scope; 4],
    tag: [Scope; 3],
    operator: [Scope; 3],
    macro_name: [Scope; 4],
    invalid: [Scope; 2],
    variable_readwrite: [Scope; 1],
    shell_source: [Scope; 1],
}

pub(in crate::preview::code) fn is_enabled(code_syntax: &str) -> bool {
    curated_syntax(code_syntax).is_some()
}

#[cfg(test)]
pub(in crate::preview::code) fn supported_syntaxes() -> &'static [CuratedSyntax] {
    CURATED_SYNTAXES
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
    SYNTAX_SET.get_or_init(|| {
        from_uncompressed_data(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/elio-curated-syntaxes.packdump"
        )))
        .expect("embedded curated syntect syntax dump should deserialize")
    })
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
            shell_variable: [
                scope("meta.group.expansion.parameter"),
                scope("punctuation.definition.variable"),
                scope("variable.other.readwrite.shell"),
                scope("variable.language.shell"),
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
            shell_source: [scope("source.shell")],
        }
    })
}

fn find_syntax<'a>(syntax_set: &'a SyntaxSet, code_syntax: &str) -> Option<&'a SyntaxReference> {
    let lookup_token = curated_syntax(code_syntax)?.lookup_token;
    syntax_set
        .find_syntax_by_token(lookup_token)
        .or_else(|| syntax_set.find_syntax_by_extension(lookup_token))
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
    } else if scope_stack_matches(scope_stack, &selectors.shell_variable) {
        SemanticRole::Parameter
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
    } else if let Some(role) = shell_semantic_role_from_heuristics(text, scope_stack, selectors) {
        role
    } else {
        SemanticRole::Fg
    }
}

fn shell_semantic_role_from_heuristics(
    text: &str,
    scope_stack: &[Scope],
    selectors: &ScopeSelectors,
) -> Option<SemanticRole> {
    if !scope_stack_matches(scope_stack, &selectors.shell_source) {
        return None;
    }

    let token = text.trim();
    if token.is_empty() {
        return None;
    }

    if matches!(
        token,
        "if" | "then"
            | "fi"
            | "for"
            | "do"
            | "done"
            | "case"
            | "esac"
            | "while"
            | "until"
            | "in"
            | "elif"
            | "else"
            | "select"
    ) {
        return Some(SemanticRole::Keyword);
    }

    if matches!(token, "[" | "]" | "test" | "echo" | "printf") {
        return Some(SemanticRole::Function);
    }

    if token.starts_with('$') || token.starts_with("${") || token.starts_with("$(") {
        return Some(SemanticRole::Parameter);
    }

    if looks_like_shell_assignment_name(token) {
        return Some(SemanticRole::Parameter);
    }

    None
}

fn looks_like_shell_assignment_name(token: &str) -> bool {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_uppercase() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
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

    fn token_scopes(code_syntax: &str, text: &str) -> Vec<(String, String)> {
        let syntax_set = syntax_set();
        let syntax = find_syntax(syntax_set, code_syntax).expect("syntax should exist");
        let mut parse_state = ParseState::new(syntax);
        let mut scope_stack = ScopeStack::new();
        let mut tokens = Vec::new();

        for line in text.lines() {
            let ops = parse_state
                .parse_line(line, syntax_set)
                .expect("line should parse");
            for (range, op) in ScopeRangeIterator::new(&ops, line) {
                scope_stack.apply(op).expect("scope op should apply");
                let token = &line[range];
                if !token.is_empty() {
                    tokens.push((token.to_string(), scope_stack.to_string()));
                }
            }
        }

        tokens
    }

    #[test]
    fn bundled_syntaxes_cover_initial_canaries() {
        let syntax_set = syntax_set();

        for syntax in supported_syntaxes() {
            assert!(
                find_syntax(syntax_set, syntax.canonical_id).is_some(),
                "missing syntect syntax for {}",
                syntax.canonical_id
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
    fn unsupported_syntaxes_return_errors_for_safe_fallback() {
        for code_syntax in ["haskell", "brainfuck", "totally-unknown-syntax"] {
            assert!(
                render_syntect_code_preview(code_syntax, "sample\n", true, 20, &|| false).is_err(),
                "expected {code_syntax} to fall back safely"
            );
        }
    }

    #[test]
    fn enabled_syntaxes_are_routed_to_syntect() {
        for syntax in supported_syntaxes() {
            assert!(
                is_enabled(syntax.canonical_id),
                "expected {} to be enabled",
                syntax.canonical_id
            );
        }
    }

    #[test]
    fn curated_bundle_supports_newly_vendored_languages() {
        for (code_syntax, snippet) in [
            (
                "typescript",
                "export type User = { name: string }\nconst greet = (user: User) => user.name;\n",
            ),
            (
                "tsx",
                "export function App() { return <button className=\"cta\">Hi</button>; }\n",
            ),
            (
                "jsx",
                "export function App() { return <button className=\"cta\">Hi</button>; }\n",
            ),
            (
                "nix",
                "{ description = \"elio\"; outputs = { self }: { packages.default = self; }; }\n",
            ),
            (
                "cmake",
                "cmake_minimum_required(VERSION 3.28)\nproject(elio)\nadd_executable(elio main.cpp)\n",
            ),
            (
                "scss",
                "$fg: #fff;\n.button { color: $fg; @include hover { color: red; } }\n",
            ),
            ("sass", "$fg: #fff\n.button\n  color: $fg\n"),
            ("less", "@fg: #fff;\n.button { color: @fg; }\n"),
            (
                "cs",
                "public class Greeter { public string Greet(string name) => name; }\n",
            ),
            (
                "dart",
                "class Greeter { String greet(String name) => name; }\n",
            ),
            (
                "zig",
                "const std = @import(\"std\");\npub fn main() void {}\n",
            ),
            (
                "kotlin",
                "class Greeter { fun greet(name: String): String = name }\n",
            ),
            (
                "swift",
                "struct Greeter { func greet(name: String) -> String { name } }\n",
            ),
            (
                "elixir",
                "defmodule Greeter do\n  def greet(name), do: \"hi #{name}\"\nend\n",
            ),
            (
                "powershell",
                "function Invoke-Greeting([string]$Name) {\n  Write-Host \"Hello $Name\"\n}\n",
            ),
        ] {
            let rendered = render_syntect_code_preview(code_syntax, snippet, true, 20, &|| false)
                .expect("vendored syntax should render through syntect");
            assert!(
                rendered
                    .iter()
                    .flat_map(|line| line.spans.iter())
                    .any(|span| span.style.fg.is_some()),
                "expected {code_syntax} to produce styled output"
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
    fn powershell_tokens_map_to_semantic_roles() {
        let palette = theme::code_preview_palette();
        let sample = "function Invoke-Greeting([string]$Name) {\n  if ($Name) { Write-Host \"Hello $Name\" }\n}\n";
        let rendered = render_syntect_code_preview("powershell", sample, true, 20, &|| false)
            .expect("powershell syntax should render through syntect");

        assert_eq!(span_color(&rendered[0], "function"), Some(palette.keyword));
        assert_eq!(
            span_color(&rendered[0], "Invoke-Greeting"),
            Some(palette.function)
        );
        assert_eq!(span_color(&rendered[0], "[string]"), Some(palette.r#type));
        assert_eq!(span_color(&rendered[0], "$Name"), Some(palette.parameter));
        assert_ne!(span_color(&rendered[1], "Write-Host"), Some(palette.fg));
        assert_eq!(span_color(&rendered[1], "\"Hello "), Some(palette.string));
        assert_eq!(span_color(&rendered[1], "$Name"), Some(palette.parameter));
    }

    #[test]
    fn sh_tokens_map_to_semantic_roles() {
        let palette = theme::code_preview_palette();
        let sample = "NAME=elio\nif [ -n \"$HOME\" ]; then\n  echo \"$NAME\"\nfi # done\n";
        let rendered = render_syntect_code_preview("sh", sample, true, 20, &|| false)
            .expect("sh syntax should render through syntect");
        let scopes = token_scopes("sh", sample);

        assert_ne!(
            span_color(&rendered[0], "NAME"),
            Some(palette.fg),
            "sh assignment fell back to fg; scopes: {scopes:#?}"
        );
        assert_ne!(
            span_color(&rendered[1], "if"),
            Some(palette.fg),
            "sh keyword fell back to fg; scopes: {scopes:#?}"
        );
        assert_ne!(
            span_color(&rendered[1], "$"),
            Some(palette.fg),
            "sh variable marker fell back to fg; scopes: {scopes:#?}"
        );
        assert_ne!(
            span_color(&rendered[1], "HOME"),
            Some(palette.fg),
            "sh variable fell back to fg; scopes: {scopes:#?}"
        );
        assert_ne!(
            span_color(&rendered[2], "echo"),
            Some(palette.fg),
            "sh builtin fell back to fg; scopes: {scopes:#?}"
        );
        assert_eq!(
            span_color(&rendered[3], "#"),
            Some(palette.comment),
            "sh comment marker did not map to comment color; scopes: {scopes:#?}"
        );
        assert_eq!(
            span_color(&rendered[3], " done"),
            Some(palette.comment),
            "sh comment did not map to comment color; scopes: {scopes:#?}"
        );
    }

    #[test]
    fn bash_tokens_map_to_semantic_roles() {
        let palette = theme::code_preview_palette();
        let sample = "NAME=elio\nif [ -n \"$HOME\" ]; then\n  echo \"$NAME\"\nfi # done\n";
        let rendered = render_syntect_code_preview("bash", sample, true, 20, &|| false)
            .expect("bash syntax should render through syntect");
        let scopes = token_scopes("bash", sample);

        assert_ne!(
            span_color(&rendered[0], "NAME"),
            Some(palette.fg),
            "bash assignment fell back to fg; scopes: {scopes:#?}"
        );
        assert_ne!(
            span_color(&rendered[1], "if"),
            Some(palette.fg),
            "bash keyword fell back to fg; scopes: {scopes:#?}"
        );
        assert_ne!(
            span_color(&rendered[1], "$"),
            Some(palette.fg),
            "bash variable marker fell back to fg; scopes: {scopes:#?}"
        );
        assert_ne!(
            span_color(&rendered[1], "HOME"),
            Some(palette.fg),
            "bash variable fell back to fg; scopes: {scopes:#?}"
        );
        assert_ne!(
            span_color(&rendered[2], "echo"),
            Some(palette.fg),
            "bash builtin fell back to fg; scopes: {scopes:#?}"
        );
        assert_eq!(
            span_color(&rendered[3], "#"),
            Some(palette.comment),
            "bash comment marker did not map to comment color; scopes: {scopes:#?}"
        );
        assert_eq!(
            span_color(&rendered[3], " done"),
            Some(palette.comment),
            "bash comment did not map to comment color; scopes: {scopes:#?}"
        );
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

        let stack = ScopeStack::from_str("source.shell.bash").unwrap();
        assert_eq!(
            semantic_role_for_token("then", stack.as_slice()),
            SemanticRole::Keyword
        );

        let stack = ScopeStack::from_str("source.shell.bash").unwrap();
        assert_eq!(
            semantic_role_for_token("printf", stack.as_slice()),
            SemanticRole::Function
        );

        let stack = ScopeStack::from_str("source.shell.bash").unwrap();
        assert_eq!(
            semantic_role_for_token("$HOME", stack.as_slice()),
            SemanticRole::Parameter
        );
    }
}
