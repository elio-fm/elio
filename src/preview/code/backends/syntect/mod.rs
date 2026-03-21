mod bundle;
mod render;
mod semantics;
mod shell;

#[cfg(test)]
use crate::preview::code::syntax_manifest::CURATED_SYNTAXES;
#[cfg(test)]
use crate::preview::code::syntax_manifest::CuratedSyntax;
use crate::preview::code::syntax_manifest::curated_syntax;
use ratatui::text::Line;

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
    if shell::is_shell_like_syntax(code_syntax) {
        return Ok(shell::render_shell_code_preview(
            text,
            line_numbers,
            line_limit,
            canceled,
        ));
    }

    let syntax_set = bundle::syntax_set();
    let Some(syntax) = bundle::find_syntax(syntax_set, code_syntax) else {
        return Err(());
    };

    render::render_syntect_code_preview(
        text,
        syntax_set,
        syntax,
        line_numbers,
        line_limit,
        canceled,
    )
}

#[cfg(test)]
mod tests {
    use super::bundle::{find_syntax, syntax_set};
    use super::semantics::{SemanticRole, semantic_role_for_token};
    use super::*;
    use crate::ui::theme;
    use std::str::FromStr;
    use syntect::{
        easy::ScopeRangeIterator,
        parsing::{ParseState, ScopeStack},
    };

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
            ("dockerfile", "FROM rust:1.87\nRUN cargo build --release\n"),
            ("hcl", "server { listen = \"127.0.0.1\" enabled = true }\n"),
            (
                "terraform",
                "terraform { required_version = \">= 1.7\" }\nresource \"null_resource\" \"example\" {}\n",
            ),
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
                "fortran",
                "program elio\n  implicit none\n  print *, \"hello\"\nend program elio\n",
            ),
            (
                "cobol",
                "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. ELIOTEST.\n       PROCEDURE DIVISION.\n           DISPLAY \"HELLO\".\n           STOP RUN.\n",
            ),
            ("julia", "function greet(name)\n  return name\nend\n"),
            ("just", "build:\n  cargo test\n"),
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
    fn sh_plain_commands_and_functions_map_to_semantic_roles() {
        let palette = theme::code_preview_palette();
        let sample =
            "deploy() {\n  grep -q \"$HOME\" /etc/profile\n  my_tool --flag \"$NAME\"\n}\n";
        let rendered = render_syntect_code_preview("sh", sample, true, 20, &|| false)
            .expect("sh syntax should render through syntect");
        let scopes = token_scopes("sh", sample);

        assert_ne!(
            span_color(&rendered[0], "deploy"),
            Some(palette.fg),
            "sh function name fell back to fg; scopes: {scopes:#?}"
        );
        assert_ne!(
            span_color(&rendered[1], "grep"),
            Some(palette.fg),
            "sh command fell back to fg; scopes: {scopes:#?}"
        );
        assert_ne!(
            span_color(&rendered[1], "-q"),
            Some(palette.fg),
            "sh option fell back to fg; scopes: {scopes:#?}"
        );
        assert_ne!(
            span_color(&rendered[2], "my_tool"),
            Some(palette.fg),
            "sh custom command fell back to fg; scopes: {scopes:#?}"
        );
        assert_ne!(
            span_color(&rendered[2], "--flag"),
            Some(palette.fg),
            "sh long option fell back to fg; scopes: {scopes:#?}"
        );
    }

    #[test]
    fn sh_common_builtins_and_redirections_map_to_semantic_roles() {
        let palette = theme::code_preview_palette();
        let sample = "#!/bin/sh\nset -e\ncd /tmp\ntrap 'cleanup' EXIT\nexport PATH=\"$HOME/bin:$PATH\"\nsource ./env.sh\nread -r NAME\nexec \"$NAME\" > /tmp/out.log\n";
        let rendered = render_syntect_code_preview("sh", sample, true, 20, &|| false)
            .expect("sh syntax should render through shell-aware renderer");

        assert_eq!(span_color(&rendered[0], "#!"), Some(palette.r#macro));
        assert_ne!(span_color(&rendered[1], "set"), Some(palette.fg));
        assert_ne!(span_color(&rendered[2], "cd"), Some(palette.fg));
        assert_ne!(span_color(&rendered[3], "trap"), Some(palette.fg));
        assert_ne!(span_color(&rendered[4], "export"), Some(palette.fg));
        assert_ne!(span_color(&rendered[5], "source"), Some(palette.fg));
        assert_ne!(span_color(&rendered[6], "read"), Some(palette.fg));
        assert_ne!(span_color(&rendered[7], "exec"), Some(palette.fg));
        assert_ne!(span_color(&rendered[7], ">"), Some(palette.fg));
    }

    #[test]
    fn sh_heredoc_markers_and_pipeline_commands_map_to_semantic_roles() {
        let palette = theme::code_preview_palette();
        let sample = "cat <<EOF | grep -i \"$needle\"\n";
        let rendered = render_syntect_code_preview("sh", sample, true, 20, &|| false)
            .expect("sh syntax should render through shell-aware renderer");

        assert_eq!(span_color(&rendered[0], "cat"), Some(palette.function));
        assert_eq!(span_color(&rendered[0], "<<"), Some(palette.operator));
        assert_eq!(span_color(&rendered[0], "EOF"), Some(palette.parameter));
        assert_eq!(span_color(&rendered[0], "|"), Some(palette.operator));
        assert_eq!(span_color(&rendered[0], "grep"), Some(palette.function));
        assert_eq!(span_color(&rendered[0], "-i"), Some(palette.parameter));
        assert_eq!(span_color(&rendered[0], "$needle"), Some(palette.parameter));
    }

    #[test]
    fn sh_export_like_builtins_keep_assignment_context_for_following_names() {
        let palette = theme::code_preview_palette();
        let sample = "export PATH=\"$HOME/bin\"\nreadonly NAME=elio\nlocal count=1\n";
        let rendered = render_syntect_code_preview("sh", sample, true, 20, &|| false)
            .expect("sh syntax should render through shell-aware renderer");

        assert_eq!(span_color(&rendered[0], "export"), Some(palette.function));
        assert_eq!(span_color(&rendered[0], "PATH"), Some(palette.parameter));
        assert_eq!(span_color(&rendered[0], "="), Some(palette.operator));
        assert_eq!(span_color(&rendered[1], "readonly"), Some(palette.function));
        assert_eq!(span_color(&rendered[1], "NAME"), Some(palette.parameter));
        assert_eq!(span_color(&rendered[1], "="), Some(palette.operator));
        assert_eq!(span_color(&rendered[2], "local"), Some(palette.function));
        assert_eq!(span_color(&rendered[2], "count"), Some(palette.parameter));
        assert_eq!(span_color(&rendered[2], "="), Some(palette.operator));
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

        let stack = ScopeStack::from_str("source.shell.bash meta.function-call.shell").unwrap();
        assert_eq!(
            semantic_role_for_token("grep", stack.as_slice()),
            SemanticRole::Function
        );

        let stack =
            ScopeStack::from_str("source.shell.bash meta.function-call.arguments.shell").unwrap();
        assert_eq!(
            semantic_role_for_token("--flag", stack.as_slice()),
            SemanticRole::Parameter
        );
    }

    #[test]
    fn semantic_role_classifier_keeps_shell_heuristics_scoped_to_shell_sources() {
        let rust_stack =
            ScopeStack::from_str("source.rust meta.function-call.arguments.rust").unwrap();
        assert_eq!(
            semantic_role_for_token("--flag", rust_stack.as_slice()),
            SemanticRole::Fg
        );

        let shell_stack =
            ScopeStack::from_str("source.shell.bash meta.function-call.arguments.shell").unwrap();
        assert_eq!(
            semantic_role_for_token("--flag", shell_stack.as_slice()),
            SemanticRole::Parameter
        );
    }
}
