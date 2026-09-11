use super::*;

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
            "astro",
            "---\nconst title: string = \"Elio\";\n---\n<Layout title={title}><h1>{title}</h1></Layout>\n",
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
        (
            "qml",
            "import QtQuick\nItem {\n  id: root\n  property bool active: true\n  onActiveChanged: console.log(\"changed\")\n}\n",
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
