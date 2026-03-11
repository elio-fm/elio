use crate::app::{EntryKind, FileClass};
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreviewKind {
    Markdown,
    Source,
    PlainText,
    Iso,
    Torrent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DocumentFormat {
    Doc,
    Docx,
    Odt,
    Pages,
}

impl DocumentFormat {
    pub(crate) fn detail_label(self) -> &'static str {
        match self {
            Self::Doc => "DOC document",
            Self::Docx => "DOCX document",
            Self::Odt => "ODT document",
            Self::Pages => "Pages document",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FallbackSyntax {
    JsLike,
    Markup,
    Css,
    Toml,
    Json,
    Jsonc,
    Yaml,
    Log,
    Ini,
    DesktopEntry,
}

impl FallbackSyntax {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::JsLike => "TypeScript",
            Self::Markup => "Markup",
            Self::Css => "CSS",
            Self::Toml => "TOML",
            Self::Json => "JSON",
            Self::Jsonc => "JSONC",
            Self::Yaml => "YAML",
            Self::Log => "Log",
            Self::Ini => "INI",
            Self::DesktopEntry => "Desktop Entry",
        }
    }

    pub(crate) fn detail_label(self) -> String {
        format!("{} (best-effort)", self.label())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StructuredFormat {
    Json,
    Jsonc,
    Json5,
    Toml,
    Yaml,
    Dotenv,
    Log,
}

impl StructuredFormat {
    pub(crate) fn detail_label(self) -> &'static str {
        match self {
            Self::Json => "JSON (structured)",
            Self::Jsonc => "JSONC (structured)",
            Self::Json5 => "JSON5 (structured)",
            Self::Toml => "TOML (structured)",
            Self::Yaml => "YAML (structured)",
            Self::Dotenv => ".env (structured)",
            Self::Log => "Log (structured)",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreviewSpec {
    pub kind: PreviewKind,
    pub syntax_hint: Option<&'static str>,
    pub fallback_syntax: Option<FallbackSyntax>,
    pub structured_format: Option<StructuredFormat>,
    pub document_format: Option<DocumentFormat>,
    pub force_fallback: bool,
}

impl PreviewSpec {
    const fn plain_text() -> Self {
        Self {
            kind: PreviewKind::PlainText,
            syntax_hint: None,
            fallback_syntax: None,
            structured_format: None,
            document_format: None,
            force_fallback: false,
        }
    }

    const fn markdown() -> Self {
        Self {
            kind: PreviewKind::Markdown,
            syntax_hint: None,
            fallback_syntax: None,
            structured_format: None,
            document_format: None,
            force_fallback: false,
        }
    }

    const fn iso() -> Self {
        Self {
            kind: PreviewKind::Iso,
            syntax_hint: None,
            fallback_syntax: None,
            structured_format: None,
            document_format: None,
            force_fallback: false,
        }
    }

    const fn torrent() -> Self {
        Self {
            kind: PreviewKind::Torrent,
            syntax_hint: None,
            fallback_syntax: None,
            structured_format: None,
            document_format: None,
            force_fallback: false,
        }
    }

    const fn source(
        syntax_hint: Option<&'static str>,
        fallback_syntax: Option<FallbackSyntax>,
        structured_format: Option<StructuredFormat>,
    ) -> Self {
        Self {
            kind: PreviewKind::Source,
            syntax_hint,
            fallback_syntax,
            structured_format,
            document_format: None,
            force_fallback: false,
        }
    }

    const fn document(document_format: DocumentFormat) -> Self {
        Self {
            kind: PreviewKind::PlainText,
            syntax_hint: None,
            fallback_syntax: None,
            structured_format: None,
            document_format: Some(document_format),
            force_fallback: false,
        }
    }

    const fn forced_fallback(fallback_syntax: FallbackSyntax) -> Self {
        Self {
            kind: PreviewKind::Source,
            syntax_hint: None,
            fallback_syntax: Some(fallback_syntax),
            structured_format: None,
            document_format: None,
            force_fallback: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileFacts {
    pub builtin_class: FileClass,
    pub specific_type_label: Option<&'static str>,
    pub preview: PreviewSpec,
}

pub(crate) fn inspect_path(path: &Path, kind: EntryKind) -> FileFacts {
    if kind == EntryKind::Directory {
        return FileFacts {
            builtin_class: FileClass::Directory,
            specific_type_label: None,
            preview: PreviewSpec::plain_text(),
        };
    }

    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(normalize_key)
        .unwrap_or_default();
    if let Some(facts) = inspect_exact_name(&name) {
        return facts;
    }

    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(normalize_key)
        .unwrap_or_default();
    inspect_extension(&ext)
}

fn inspect_exact_name(name: &str) -> Option<FileFacts> {
    match name {
        "pkgbuild" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Arch build script"),
            preview: PreviewSpec::source(Some("bash"), None, None),
        }),
        "cargo.lock" | "poetry.lock" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: None,
            preview: PreviewSpec::source(
                Some("toml"),
                Some(FallbackSyntax::Toml),
                Some(StructuredFormat::Toml),
            ),
        }),
        "package.json" | "tsconfig.json" | "deno.json" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: None,
            preview: PreviewSpec::source(
                Some("json"),
                Some(FallbackSyntax::Json),
                Some(StructuredFormat::Json),
            ),
        }),
        "package-lock.json" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: None,
            preview: PreviewSpec::source(
                Some("json"),
                Some(FallbackSyntax::Json),
                Some(StructuredFormat::Json),
            ),
        }),
        "deno.jsonc" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("JSON with comments"),
            preview: PreviewSpec::source(
                Some("jsonc"),
                Some(FallbackSyntax::Jsonc),
                Some(StructuredFormat::Jsonc),
            ),
        }),
        "compose.yml"
        | "compose.yaml"
        | "docker-compose.yml"
        | "docker-compose.yaml"
        | "pnpm-lock.yaml"
        | "pnpm-workspace.yaml" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: None,
            preview: PreviewSpec::source(
                Some("yaml"),
                Some(FallbackSyntax::Yaml),
                Some(StructuredFormat::Yaml),
            ),
        }),
        _ if is_env_name(name) => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Environment file"),
            preview: PreviewSpec::source(
                None,
                Some(FallbackSyntax::Ini),
                Some(StructuredFormat::Dotenv),
            ),
        }),
        _ => None,
    }
}

fn inspect_extension(ext: &str) -> FileFacts {
    match ext {
        "md" | "markdown" | "mdown" | "mkd" | "mdx" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: None,
            preview: PreviewSpec::markdown(),
        },
        "iso" => FileFacts {
            builtin_class: FileClass::Archive,
            specific_type_label: Some("ISO disk image"),
            preview: PreviewSpec::iso(),
        },
        "torrent" => FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: Some("BitTorrent file"),
            preview: PreviewSpec::torrent(),
        },
        "json" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: None,
            preview: PreviewSpec::source(
                Some("json"),
                Some(FallbackSyntax::Json),
                Some(StructuredFormat::Json),
            ),
        },
        "jsonc" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("JSON with comments"),
            preview: PreviewSpec::source(
                Some("jsonc"),
                Some(FallbackSyntax::Jsonc),
                Some(StructuredFormat::Jsonc),
            ),
        },
        "json5" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("JSON5 file"),
            preview: PreviewSpec::source(
                Some("javascript"),
                Some(FallbackSyntax::Jsonc),
                Some(StructuredFormat::Json5),
            ),
        },
        "toml" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: None,
            preview: PreviewSpec::source(
                Some("toml"),
                Some(FallbackSyntax::Toml),
                Some(StructuredFormat::Toml),
            ),
        },
        "yaml" | "yml" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: None,
            preview: PreviewSpec::source(
                Some("yaml"),
                Some(FallbackSyntax::Yaml),
                Some(StructuredFormat::Yaml),
            ),
        },
        "html" | "htm" | "xhtml" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("HTML document"),
            preview: PreviewSpec::source(Some("html"), Some(FallbackSyntax::Markup), None),
        },
        "xml" | "xsd" | "xsl" | "xslt" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("XML document"),
            preview: PreviewSpec::source(Some("xml"), Some(FallbackSyntax::Markup), None),
        },
        "css" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Stylesheet"),
            preview: PreviewSpec::source(Some("css"), Some(FallbackSyntax::Css), None),
        },
        "scss" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("SCSS stylesheet"),
            preview: PreviewSpec::source(Some("scss"), Some(FallbackSyntax::Css), None),
        },
        "sass" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Sass stylesheet"),
            preview: PreviewSpec::source(Some("sass"), Some(FallbackSyntax::Css), None),
        },
        "less" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Less stylesheet"),
            preview: PreviewSpec::source(Some("css"), Some(FallbackSyntax::Css), None),
        },
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: None,
            preview: PreviewSpec::source(None, Some(FallbackSyntax::JsLike), None),
        },
        "ini" | "conf" | "cfg" | "keys" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: match ext {
                "keys" => Some("Keys file"),
                _ => None,
            },
            preview: PreviewSpec::source(None, Some(FallbackSyntax::Ini), None),
        },
        "env" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Environment file"),
            preview: PreviewSpec::source(
                None,
                Some(FallbackSyntax::Ini),
                Some(StructuredFormat::Dotenv),
            ),
        },
        "desktop" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Desktop Entry"),
            preview: PreviewSpec::forced_fallback(FallbackSyntax::DesktopEntry),
        },
        "log" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("Log file"),
            preview: PreviewSpec::source(
                None,
                Some(FallbackSyntax::Log),
                Some(StructuredFormat::Log),
            ),
        },
        "xcf" => plain(FileClass::Image, Some("GIMP image")),
        "ico" => plain(FileClass::Image, Some("Icon image")),
        "rpm" => plain(FileClass::Archive, Some("RPM package")),
        "hash" => plain(FileClass::Data, Some("Hash file")),
        "sha1" => plain(FileClass::Data, Some("SHA-1 checksum")),
        "sha256" => plain(FileClass::Data, Some("SHA-256 checksum")),
        "sha512" => plain(FileClass::Data, Some("SHA-512 checksum")),
        "md5" => plain(FileClass::Data, Some("MD5 checksum")),
        "srt" => plain(FileClass::Document, Some("SubRip subtitles")),
        "p12" | "pfx" => plain(FileClass::Config, Some("PKCS#12 certificate")),
        "pem" => plain(FileClass::Config, Some("PEM certificate")),
        "crt" | "cer" => plain(FileClass::Config, Some("Certificate")),
        "csr" => plain(FileClass::Config, Some("Certificate signing request")),
        "key" => plain(FileClass::Config, Some("Private key")),
        "deb" => plain(FileClass::Archive, Some("Debian package")),
        "apk" => plain(FileClass::Archive, Some("Android package")),
        "aab" => plain(FileClass::Archive, Some("Android App Bundle")),
        "apkg" => plain(FileClass::Archive, Some("Anki package")),
        "zst" => plain(FileClass::Archive, Some("Zstandard archive")),
        "zest" => plain(FileClass::Archive, Some("Zest archive")),
        "appimage" => plain(FileClass::Archive, Some("AppImage bundle")),
        "exe" => plain(FileClass::File, Some("Windows executable")),
        "jar" => plain(FileClass::Archive, Some("Java archive")),
        "rs" | "py" | "go" | "c" | "cpp" | "h" | "hpp" | "java" | "lua" | "php" | "rb"
        | "swift" | "kt" | "sh" | "bash" | "zsh" | "fish" => {
            source_only(FileClass::Code, None, None)
        }
        "ron" => source_only(FileClass::Config, None, None),
        "csv" | "tsv" | "sql" | "sqlite" | "db" | "parquet" => {
            source_only(FileClass::Data, None, None)
        }
        "doc" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("DOC document"),
            preview: PreviewSpec::document(DocumentFormat::Doc),
        },
        "docx" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("DOCX document"),
            preview: PreviewSpec::document(DocumentFormat::Docx),
        },
        "odt" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("ODT document"),
            preview: PreviewSpec::document(DocumentFormat::Odt),
        },
        "pages" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("Pages document"),
            preview: PreviewSpec::document(DocumentFormat::Pages),
        },
        "txt" | "rst" | "pdf" => plain(FileClass::Document, None),
        "svg" => FileFacts {
            builtin_class: FileClass::Image,
            specific_type_label: Some("SVG image"),
            preview: PreviewSpec::source(Some("xml"), Some(FallbackSyntax::Markup), None),
        },
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" => plain(FileClass::Image, None),
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => plain(FileClass::Audio, None),
        "mp4" | "mkv" | "mov" | "webm" | "avi" => plain(FileClass::Video, None),
        "zip" | "tar" | "gz" | "xz" | "bz2" | "7z" => plain(FileClass::Archive, None),
        "ttf" | "otf" | "woff" | "woff2" => plain(FileClass::Font, None),
        _ => plain(FileClass::File, None),
    }
}

const fn plain(class: FileClass, specific_type_label: Option<&'static str>) -> FileFacts {
    FileFacts {
        builtin_class: class,
        specific_type_label,
        preview: PreviewSpec::plain_text(),
    }
}

const fn source_only(
    class: FileClass,
    specific_type_label: Option<&'static str>,
    syntax_hint: Option<&'static str>,
) -> FileFacts {
    FileFacts {
        builtin_class: class,
        specific_type_label,
        preview: PreviewSpec::source(syntax_hint, None, None),
    }
}

fn is_env_name(name: &str) -> bool {
    name == ".env" || name.starts_with(".env.")
}

fn normalize_key(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_lock_uses_one_shared_definition() {
        let facts = inspect_path(Path::new("package-lock.json"), EntryKind::File);

        assert_eq!(facts.builtin_class, FileClass::Data);
        assert_eq!(
            facts.preview.structured_format,
            Some(StructuredFormat::Json)
        );
        assert_eq!(facts.preview.fallback_syntax, Some(FallbackSyntax::Json));
    }

    #[test]
    fn dotenv_variants_are_classified_once() {
        let facts = inspect_path(Path::new(".env.local"), EntryKind::File);

        assert_eq!(facts.builtin_class, FileClass::Config);
        assert_eq!(facts.specific_type_label, Some("Environment file"));
        assert_eq!(
            facts.preview.structured_format,
            Some(StructuredFormat::Dotenv)
        );
    }

    #[test]
    fn json5_gets_parser_backed_preview_support() {
        let facts = inspect_path(Path::new("settings.json5"), EntryKind::File);

        assert_eq!(facts.builtin_class, FileClass::Config);
        assert_eq!(
            facts.preview.structured_format,
            Some(StructuredFormat::Json5)
        );
        assert_eq!(facts.preview.fallback_syntax, Some(FallbackSyntax::Jsonc));
    }

    #[test]
    fn html_and_css_files_use_code_preview_support() {
        let html = inspect_path(Path::new("index.html"), EntryKind::File);
        let css = inspect_path(Path::new("styles.css"), EntryKind::File);

        assert_eq!(html.builtin_class, FileClass::Code);
        assert_eq!(html.preview.syntax_hint, Some("html"));
        assert_eq!(html.preview.fallback_syntax, Some(FallbackSyntax::Markup));

        assert_eq!(css.builtin_class, FileClass::Code);
        assert_eq!(css.preview.syntax_hint, Some("css"));
        assert_eq!(css.preview.fallback_syntax, Some(FallbackSyntax::Css));
    }

    #[test]
    fn svg_keeps_image_identity_while_using_markup_preview() {
        let facts = inspect_path(Path::new("icon.svg"), EntryKind::File);

        assert_eq!(facts.builtin_class, FileClass::Image);
        assert_eq!(facts.specific_type_label, Some("SVG image"));
        assert_eq!(facts.preview.syntax_hint, Some("xml"));
        assert_eq!(facts.preview.fallback_syntax, Some(FallbackSyntax::Markup));
    }

    #[test]
    fn office_and_pages_documents_use_metadata_preview() {
        let doc = inspect_path(Path::new("legacy.doc"), EntryKind::File);
        let docx = inspect_path(Path::new("report.docx"), EntryKind::File);
        let odt = inspect_path(Path::new("report.odt"), EntryKind::File);
        let pages = inspect_path(Path::new("proposal.pages"), EntryKind::File);

        assert_eq!(doc.builtin_class, FileClass::Document);
        assert_eq!(doc.preview.document_format, Some(DocumentFormat::Doc));
        assert_eq!(doc.specific_type_label, Some("DOC document"));

        assert_eq!(docx.builtin_class, FileClass::Document);
        assert_eq!(docx.preview.document_format, Some(DocumentFormat::Docx));
        assert_eq!(docx.specific_type_label, Some("DOCX document"));

        assert_eq!(odt.builtin_class, FileClass::Document);
        assert_eq!(odt.preview.document_format, Some(DocumentFormat::Odt));
        assert_eq!(odt.specific_type_label, Some("ODT document"));

        assert_eq!(pages.builtin_class, FileClass::Document);
        assert_eq!(pages.preview.document_format, Some(DocumentFormat::Pages));
        assert_eq!(pages.specific_type_label, Some("Pages document"));
    }
}
