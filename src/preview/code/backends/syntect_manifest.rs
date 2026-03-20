#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxSource {
    BundledDefault,
    Vendored,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CuratedSyntax {
    pub canonical_id: &'static str,
    pub lookup_token: &'static str,
    pub source: SyntaxSource,
}

pub const CURATED_SYNTAXES: &[CuratedSyntax] = &[
    CuratedSyntax {
        canonical_id: "html",
        lookup_token: "html",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "xml",
        lookup_token: "xml",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "css",
        lookup_token: "css",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "scss",
        lookup_token: "scss",
        source: SyntaxSource::Vendored,
    },
    CuratedSyntax {
        canonical_id: "sass",
        lookup_token: "sass",
        source: SyntaxSource::Vendored,
    },
    CuratedSyntax {
        canonical_id: "less",
        lookup_token: "less",
        source: SyntaxSource::Vendored,
    },
    CuratedSyntax {
        canonical_id: "javascript",
        lookup_token: "js",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "jsx",
        lookup_token: "jsx",
        source: SyntaxSource::Vendored,
    },
    CuratedSyntax {
        canonical_id: "typescript",
        lookup_token: "typescript",
        source: SyntaxSource::Vendored,
    },
    CuratedSyntax {
        canonical_id: "tsx",
        lookup_token: "tsx",
        source: SyntaxSource::Vendored,
    },
    CuratedSyntax {
        canonical_id: "rust",
        lookup_token: "rs",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "go",
        lookup_token: "go",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "c",
        lookup_token: "c",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "cpp",
        lookup_token: "cpp",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "cs",
        lookup_token: "cs",
        source: SyntaxSource::Vendored,
    },
    CuratedSyntax {
        canonical_id: "java",
        lookup_token: "java",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "dart",
        lookup_token: "dart",
        source: SyntaxSource::Vendored,
    },
    CuratedSyntax {
        canonical_id: "zig",
        lookup_token: "zig",
        source: SyntaxSource::Vendored,
    },
    CuratedSyntax {
        canonical_id: "php",
        lookup_token: "php",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "swift",
        lookup_token: "swift",
        source: SyntaxSource::Vendored,
    },
    CuratedSyntax {
        canonical_id: "kotlin",
        lookup_token: "kotlin",
        source: SyntaxSource::Vendored,
    },
    CuratedSyntax {
        canonical_id: "elixir",
        lookup_token: "elixir",
        source: SyntaxSource::Vendored,
    },
    CuratedSyntax {
        canonical_id: "ruby",
        lookup_token: "rb",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "python",
        lookup_token: "py",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "lua",
        lookup_token: "lua",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "make",
        lookup_token: "makefile",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "sh",
        lookup_token: "sh",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "bash",
        lookup_token: "bash",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "zsh",
        lookup_token: "sh",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "ksh",
        lookup_token: "sh",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "fish",
        lookup_token: "fish",
        source: SyntaxSource::BundledDefault,
    },
    CuratedSyntax {
        canonical_id: "nix",
        lookup_token: "nix",
        source: SyntaxSource::Vendored,
    },
    CuratedSyntax {
        canonical_id: "cmake",
        lookup_token: "cmake",
        source: SyntaxSource::Vendored,
    },
];

#[allow(dead_code)]
pub fn curated_syntax(code_syntax: &str) -> Option<&'static CuratedSyntax> {
    CURATED_SYNTAXES
        .iter()
        .find(|syntax| syntax.canonical_id == code_syntax)
}
