use super::*;

#[test]
fn c_preview_uses_code_renderer() {
    let root = temp_path("c");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.c");
    fs::write(
        &path,
        "#include <stdio.h>\nint main(void) {\n    printf(\"hello\\n\");\n}\n",
    )
    .expect("failed to write c source");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some_and(|detail| detail.contains('C')));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("printf"))
    );
    assert_ne!(span_color(&preview.lines[0], "#"), Some(code_palette.fg));
    assert_ne!(span_color(&preview.lines[1], "int"), Some(code_palette.fg));
    assert_ne!(
        span_color(&preview.lines[2], "printf"),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn python_preview_uses_code_renderer_with_colors() {
    let root = temp_path("python");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.py");
    fs::write(
            &path,
            "@decorator\nclass Greeter:\n    async def greet(self, name: str) -> str:\n        \"\"\"Return greeting.\"\"\"\n        return f\"hi {name}\"\n",
        )
        .expect("failed to write python source");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .is_some_and(|detail| detail.contains("Python"))
    );
    assert_ne!(
        span_color(&preview.lines[1], "class"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[1], "Greeter"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[2], "async"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[2], "greet"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[4], "return"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[4], "f\"hi {name}\""),
        Some(code_palette.fg)
    );
    assert!(line_text(&preview.lines[3]).contains("Return greeting."));
    assert!(line_text(&preview.lines[4]).contains("f\"hi {name}\""));

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn javascript_preview_uses_code_renderer_with_colors() {
    let root = temp_path("javascript");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.js");
    fs::write(
        &path,
        "export class Greeter {\n  greet(name) { return console.log(`hi ${name}`); }\n}\n",
    )
    .expect("failed to write javascript source");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .is_some_and(|detail| detail.contains("JavaScript"))
    );
    assert_ne!(
        span_color(&preview.lines[0], "export"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[0], "Greeter"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[1], "return"),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn nix_preview_uses_curated_syntect_support() {
    let root = temp_path("nix");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("flake.nix");
    fs::write(
            &path,
            "{ description = \"elio\"; outputs = { self }: { packages.x86_64-linux.default = self; }; }\n",
        )
        .expect("failed to write nix source");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(preview.detail.is_some_and(|detail| detail.contains("Nix")));
    assert_ne!(
        span_color(&preview.lines[0], "description"),
        Some(code_palette.fg)
    );
    assert!(line_has_color(&preview.lines[0], code_palette.string));
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("description"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn cmake_preview_uses_curated_syntect_support() {
    let root = temp_path("cmake");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("CMakeLists.txt");
    fs::write(
        &path,
        "cmake_minimum_required(VERSION 3.28)\nproject(elio)\nadd_executable(elio main.cpp)\n",
    )
    .expect("failed to write cmake source");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .is_some_and(|detail| detail.contains("CMake"))
    );
    assert_ne!(
        span_color(&preview.lines[2], "add_executable"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[1], "project"),
        Some(code_palette.fg)
    );
    assert!(
        preview
            .lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("add_executable"))
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn powershell_preview_uses_curated_syntect_support() {
    let root = temp_path("powershell");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("build.ps1");
    fs::write(
        &path,
        "function Invoke-Greeting([string]$Name) {\n  Write-Host \"Hello $Name\"\n}\n",
    )
    .expect("failed to write powershell script");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("PowerShell"))
    );
    assert_eq!(
        span_color(&preview.lines[0], "function"),
        Some(code_palette.keyword)
    );
    assert_eq!(
        span_color(&preview.lines[0], "Invoke-Greeting"),
        Some(code_palette.function)
    );
    assert_eq!(
        span_color(&preview.lines[0], "[string]"),
        Some(code_palette.r#type)
    );
    assert_eq!(
        span_color(&preview.lines[1], "\"Hello "),
        Some(code_palette.string)
    );
    assert_eq!(
        span_color(&preview.lines[1], "$Name"),
        Some(code_palette.string)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn typescript_preview_uses_code_renderer() {
    let root = temp_path("typescript");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("main.ts");
    fs::write(&path, "export const value: number = 1;\n").expect("failed to write ts");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("TypeScript"))
    );
    assert_ne!(
        span_color(&preview.lines[0], "export"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[0], "const"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[0], "number"),
        Some(code_palette.fg)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn tsx_preview_uses_code_renderer() {
    let root = temp_path("tsx");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("App.tsx");
    fs::write(
        &path,
        "export function App() { return <div className=\"greeting\">Hello</div>; }\n",
    )
    .expect("failed to write tsx");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("TSX"))
    );
    assert_ne!(
        span_color(&preview.lines[0], "export"),
        Some(code_palette.fg)
    );
    assert_ne!(
        span_color(&preview.lines[0], "return"),
        Some(code_palette.fg)
    );
    assert_eq!(span_color(&preview.lines[0], "div"), Some(code_palette.tag));
    assert_eq!(
        span_color(&preview.lines[0], "className"),
        Some(code_palette.parameter)
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn curated_syntect_languages_render_with_theme_colors() {
    let root = temp_path("curated-syntect");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let code_palette = theme::code_preview_palette();

    for (name, contents, detail, token) in [
        (
            "schema.sql",
            "SELECT name FROM users WHERE id = 1;\n",
            "SQL",
            "SELECT",
        ),
        (
            "Dockerfile",
            "FROM rust:1.87\nRUN cargo build --release\n",
            "Docker build file",
            "FROM",
        ),
        (
            "main.tf",
            "terraform { required_version = \">= 1.7\" }\n",
            "Terraform module",
            "terraform",
        ),
        (
            "terraform.hcl",
            "server { listen = \"127.0.0.1\" }\n",
            "HCL config",
            "server",
        ),
        (
            "build.gradle",
            "plugins { id 'java' }\n",
            "Gradle build script",
            "'java'",
        ),
        (
            "build.sbt",
            "lazy val root = (project in file(\".\"))\n",
            "sbt build definition",
            "lazy",
        ),
        (
            "script.pl",
            "sub greet { print \"hi\\n\"; }\n",
            "Perl",
            "sub",
        ),
        (
            "Main.hs",
            "module Main where\nmain = putStrLn \"elio\"\n",
            "Haskell",
            "module",
        ),
        (
            "main.jl",
            "function greet(name)\n  return name\nend\n",
            "Julia",
            "function",
        ),
        (
            "analysis.r",
            "library(ggplot2)\nprint(\"elio\")\n",
            "R",
            "library",
        ),
        ("Justfile", "build:\n  cargo test\n", "Just", "build"),
        (
            "styles.scss",
            "$fg: #fff;\n.button { color: $fg; }\n",
            "SCSS",
            "$fg",
        ),
        (
            "theme.sass",
            "$fg: #fff\n.button\n  color: $fg\n",
            "Sass",
            "$fg",
        ),
        (
            "theme.less",
            "@fg: #fff;\n.button { color: @fg; }\n",
            "Less",
            "@fg",
        ),
        (
            "Program.cs",
            "public class Greeter { public string Greet(string name) => name; }\n",
            "C#",
            "public",
        ),
        (
            "main.dart",
            "class Greeter { String greet(String name) => name; }\n",
            "Dart",
            "class",
        ),
        (
            "solver.f90",
            "program elio\n  implicit none\n  print *, \"hello\"\nend program elio\n",
            "Fortran",
            "program",
        ),
        (
            "ledger.cbl",
            "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. ELIOTEST.\n       PROCEDURE DIVISION.\n           DISPLAY \"HELLO\".\n",
            "COBOL",
            "IDENTIFICATION",
        ),
        (
            "main.zig",
            "const std = @import(\"std\");\npub fn main() void {}\n",
            "Zig",
            "@import",
        ),
        (
            "main.kt",
            "class Greeter { fun greet(name: String): String = name }\n",
            "Kotlin",
            "fun",
        ),
        (
            "main.swift",
            "struct Greeter { func greet(name: String) -> String { name } }\n",
            "Swift",
            "func",
        ),
        (
            "main.exs",
            "defmodule Greeter do\n  def greet(name), do: \"hi #{name}\"\nend\n",
            "Elixir",
            "defmodule",
        ),
        (
            "core.clj",
            "(ns elio.core)\n(defn greet [name] (str \"hi \" name))\n",
            "Clojure",
            "defn",
        ),
        (
            "build.ps1",
            "function Invoke-Greeting([string]$Name) {\n  Write-Host \"Hello $Name\"\n}\n",
            "PowerShell",
            "function",
        ),
    ] {
        let path = root.join(name);
        fs::write(&path, contents).expect("failed to write curated syntax fixture");
        let preview = build_preview(&file_entry(path));

        assert_eq!(preview.kind, PreviewKind::Code);
        assert!(
            preview
                .detail
                .as_deref()
                .is_some_and(|rendered| rendered.contains(detail)),
            "expected preview detail to mention {detail}"
        );
        assert_ne!(
            span_color(&preview.lines[0], token),
            Some(code_palette.fg),
            "expected {name} to highlight {token}"
        );
    }

    fs::remove_dir_all(root).expect("failed to remove temp root");
}

#[test]
fn diff_preview_uses_curated_syntect_support() {
    let root = temp_path("diff");
    fs::create_dir_all(&root).expect("failed to create temp root");
    let path = root.join("changes.diff");
    fs::write(
        &path,
        "diff --git a/src/main.rs b/src/main.rs\nindex 1111111..2222222 100644\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-fn old() {}\n+fn new() {}\n",
    )
    .expect("failed to write diff fixture");

    let preview = build_preview(&file_entry(path));
    let code_palette = theme::code_preview_palette();

    assert_eq!(preview.kind, PreviewKind::Code);
    assert!(
        preview
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Diff"))
    );
    assert!(
        preview.lines.iter().any(|line| {
            line.spans.iter().any(|span| {
                span.content.trim() != "│"
                    && !span.content.trim().is_empty()
                    && span.style.fg.is_some()
                    && span.style.fg != Some(code_palette.fg)
            })
        }),
        "expected diff preview to contain at least one highlighted token",
    );

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
