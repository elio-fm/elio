use crate::app::FileClass;

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
    Docm,
    Odt,
    Ods,
    Odp,
    Pptx,
    Pptm,
    Xlsx,
    Xlsm,
    Pages,
    Epub,
    Pdf,
}

impl DocumentFormat {
    pub(crate) fn detail_label(self) -> &'static str {
        match self {
            Self::Doc => "DOC document",
            Self::Docx => "DOCX document",
            Self::Docm => "DOCM document",
            Self::Odt => "ODT document",
            Self::Ods => "ODS spreadsheet",
            Self::Odp => "ODP presentation",
            Self::Pptx => "PPTX presentation",
            Self::Pptm => "PPTM presentation",
            Self::Xlsx => "XLSX spreadsheet",
            Self::Xlsm => "XLSM spreadsheet",
            Self::Pages => "Pages document",
            Self::Epub => "EPUB ebook",
            Self::Pdf => "PDF document",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CodeBackend {
    Plain,
    Syntect,
    Custom(CustomCodeKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CustomCodeKind {
    DirectiveConf,
    Ini,
    DesktopEntry,
    Json,
    Jsonc,
    Toml,
    Yaml,
    Log,
}

impl CustomCodeKind {
    pub(crate) const fn highlight_language(self) -> HighlightLanguage {
        match self {
            Self::DirectiveConf => HighlightLanguage::DirectiveConf,
            Self::Ini => HighlightLanguage::Ini,
            Self::DesktopEntry => HighlightLanguage::DesktopEntry,
            Self::Json => HighlightLanguage::Json,
            Self::Jsonc => HighlightLanguage::Jsonc,
            Self::Toml => HighlightLanguage::Toml,
            Self::Yaml => HighlightLanguage::Yaml,
            Self::Log => HighlightLanguage::Log,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HighlightLanguage {
    JsLike,
    CLike,
    DirectiveConf,
    Lua,
    Python,
    Make,
    Shell,
    Nix,
    CMake,
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

impl HighlightLanguage {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::JsLike => "JavaScript / TypeScript",
            Self::CLike => "C-style code",
            Self::DirectiveConf => "Directive config",
            Self::Lua => "Lua",
            Self::Python => "Python",
            Self::Make => "Makefile",
            Self::Shell => "Shell",
            Self::Nix => "Nix",
            Self::CMake => "CMake",
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
        self.label().to_string()
    }

    pub(crate) const fn code_backend(self) -> CodeBackend {
        match self {
            Self::JsLike
            | Self::CLike
            | Self::Lua
            | Self::Python
            | Self::Make
            | Self::Shell
            | Self::Nix
            | Self::CMake
            | Self::Markup
            | Self::Css => CodeBackend::Syntect,
            Self::DirectiveConf => CodeBackend::Custom(CustomCodeKind::DirectiveConf),
            Self::Toml => CodeBackend::Custom(CustomCodeKind::Toml),
            Self::Json => CodeBackend::Custom(CustomCodeKind::Json),
            Self::Jsonc => CodeBackend::Custom(CustomCodeKind::Jsonc),
            Self::Yaml => CodeBackend::Custom(CustomCodeKind::Yaml),
            Self::Log => CodeBackend::Custom(CustomCodeKind::Log),
            Self::Ini => CodeBackend::Custom(CustomCodeKind::Ini),
            Self::DesktopEntry => CodeBackend::Custom(CustomCodeKind::DesktopEntry),
        }
    }

    pub(crate) const fn default_code_syntax(self) -> Option<&'static str> {
        match self {
            Self::JsLike => Some("typescript"),
            Self::CLike => Some("c"),
            Self::DirectiveConf => Some("config"),
            Self::Lua => Some("lua"),
            Self::Python => Some("python"),
            Self::Make => Some("make"),
            Self::Shell => Some("shell"),
            Self::Nix => Some("nix"),
            Self::CMake => Some("cmake"),
            Self::Markup => Some("markup"),
            Self::Css => Some("css"),
            Self::Toml => Some("toml"),
            Self::Json => Some("json"),
            Self::Jsonc => Some("jsonc"),
            Self::Yaml => Some("yaml"),
            Self::Log => Some("log"),
            Self::Ini => Some("ini"),
            Self::DesktopEntry => Some("desktop"),
        }
    }

    pub(crate) fn from_language_token(token: &str) -> Option<Self> {
        crate::preview::code::registry::language_for_markdown_fence(token).and_then(|language| {
            match language.backend {
                CodeBackend::Plain => None,
                CodeBackend::Syntect => Self::from_code_syntax(language.canonical_id),
                CodeBackend::Custom(kind) => Some(kind.highlight_language()),
            }
        })
    }

    pub(crate) fn from_code_syntax(code_syntax: &str) -> Option<Self> {
        match code_syntax {
            "javascript" | "jsx" | "typescript" | "tsx" => Some(Self::JsLike),
            "c" | "cpp" | "rust" | "go" | "java" | "kotlin" | "php" | "swift" => Some(Self::CLike),
            "config" | "kitty" | "mpv" | "btop" => Some(Self::DirectiveConf),
            "lua" => Some(Self::Lua),
            "python" | "ruby" => Some(Self::Python),
            "make" => Some(Self::Make),
            "sh" | "bash" | "zsh" | "ksh" | "fish" => Some(Self::Shell),
            "nix" => Some(Self::Nix),
            "cmake" => Some(Self::CMake),
            "html" | "xml" => Some(Self::Markup),
            "css" | "scss" | "sass" | "less" => Some(Self::Css),
            "toml" => Some(Self::Toml),
            "json" => Some(Self::Json),
            "jsonc" | "json5" => Some(Self::Jsonc),
            "yaml" => Some(Self::Yaml),
            "log" => Some(Self::Log),
            "ini" | "dotenv" => Some(Self::Ini),
            "desktop" => Some(Self::DesktopEntry),
            _ => None,
        }
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
            Self::Json => "JSON",
            Self::Jsonc => "JSONC",
            Self::Json5 => "JSON5",
            Self::Toml => "TOML",
            Self::Yaml => "YAML",
            Self::Dotenv => ".env",
            Self::Log => "Log",
        }
    }

    pub(crate) const fn code_syntax(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Jsonc => "jsonc",
            Self::Json5 => "json5",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
            Self::Dotenv => "dotenv",
            Self::Log => "log",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompoundArchiveKind {
    TarGzip,
    TarXz,
    TarBzip2,
    TarZstd,
    CompressedDiskImage {
        image: DiskImageKind,
        compression: CompressionKind,
    },
}

impl CompoundArchiveKind {
    pub(crate) const fn detail_label(self) -> &'static str {
        match self {
            Self::TarGzip => "TAR.GZ archive",
            Self::TarXz => "TAR.XZ archive",
            Self::TarBzip2 => "TAR.BZ2 archive",
            Self::TarZstd => "TAR.ZST archive",
            Self::CompressedDiskImage { image, compression } => {
                compression.compressed_disk_image_label(image)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompressionKind {
    Gzip,
    Xz,
    Bzip2,
    Zstd,
}

impl CompressionKind {
    const fn compressed_disk_image_label(self, image: DiskImageKind) -> &'static str {
        match (self, image) {
            (Self::Gzip, DiskImageKind::Raw) => "Gzip-compressed raw disk image",
            (Self::Xz, DiskImageKind::Raw) => "XZ-compressed raw disk image",
            (Self::Bzip2, DiskImageKind::Raw) => "Bzip2-compressed raw disk image",
            (Self::Zstd, DiskImageKind::Raw) => "Zstandard-compressed raw disk image",
            (Self::Gzip, DiskImageKind::Img) => "Gzip-compressed disk image",
            (Self::Xz, DiskImageKind::Img) => "XZ-compressed disk image",
            (Self::Bzip2, DiskImageKind::Img) => "Bzip2-compressed disk image",
            (Self::Zstd, DiskImageKind::Img) => "Zstandard-compressed disk image",
            (Self::Gzip, DiskImageKind::Iso) => "Gzip-compressed ISO disk image",
            (Self::Xz, DiskImageKind::Iso) => "XZ-compressed ISO disk image",
            (Self::Bzip2, DiskImageKind::Iso) => "Bzip2-compressed ISO disk image",
            (Self::Zstd, DiskImageKind::Iso) => "Zstandard-compressed ISO disk image",
            (Self::Gzip, DiskImageKind::Qcow2) => "Gzip-compressed QCOW2 disk image",
            (Self::Xz, DiskImageKind::Qcow2) => "XZ-compressed QCOW2 disk image",
            (Self::Bzip2, DiskImageKind::Qcow2) => "Bzip2-compressed QCOW2 disk image",
            (Self::Zstd, DiskImageKind::Qcow2) => "Zstandard-compressed QCOW2 disk image",
            (Self::Gzip, DiskImageKind::Vmdk) => "Gzip-compressed VMDK disk image",
            (Self::Xz, DiskImageKind::Vmdk) => "XZ-compressed VMDK disk image",
            (Self::Bzip2, DiskImageKind::Vmdk) => "Bzip2-compressed VMDK disk image",
            (Self::Zstd, DiskImageKind::Vmdk) => "Zstandard-compressed VMDK disk image",
            (Self::Gzip, DiskImageKind::Vdi) => "Gzip-compressed VDI disk image",
            (Self::Xz, DiskImageKind::Vdi) => "XZ-compressed VDI disk image",
            (Self::Bzip2, DiskImageKind::Vdi) => "Bzip2-compressed VDI disk image",
            (Self::Zstd, DiskImageKind::Vdi) => "Zstandard-compressed VDI disk image",
            (Self::Gzip, DiskImageKind::Vhd) => "Gzip-compressed VHD disk image",
            (Self::Xz, DiskImageKind::Vhd) => "XZ-compressed VHD disk image",
            (Self::Bzip2, DiskImageKind::Vhd) => "Bzip2-compressed VHD disk image",
            (Self::Zstd, DiskImageKind::Vhd) => "Zstandard-compressed VHD disk image",
            (Self::Gzip, DiskImageKind::Vhdx) => "Gzip-compressed VHDX disk image",
            (Self::Xz, DiskImageKind::Vhdx) => "XZ-compressed VHDX disk image",
            (Self::Bzip2, DiskImageKind::Vhdx) => "Bzip2-compressed VHDX disk image",
            (Self::Zstd, DiskImageKind::Vhdx) => "Zstandard-compressed VHDX disk image",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DiskImageKind {
    Raw,
    Img,
    Iso,
    Qcow2,
    Vmdk,
    Vdi,
    Vhd,
    Vhdx,
}

impl DiskImageKind {
    pub(super) const fn detail_label(self) -> &'static str {
        match self {
            Self::Raw => "Raw disk image",
            Self::Img => "Disk image",
            Self::Iso => "ISO disk image",
            Self::Qcow2 => "QCOW2 disk image",
            Self::Vmdk => "VMDK disk image",
            Self::Vdi => "VDI disk image",
            Self::Vhd => "VHD disk image",
            Self::Vhdx => "VHDX disk image",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreviewSpec {
    pub kind: PreviewKind,
    pub language_hint: Option<&'static str>,
    pub code_syntax: Option<&'static str>,
    pub code_backend: CodeBackend,
    pub structured_format: Option<StructuredFormat>,
    pub document_format: Option<DocumentFormat>,
}

impl PreviewSpec {
    pub(super) const fn plain_text() -> Self {
        Self {
            kind: PreviewKind::PlainText,
            language_hint: None,
            code_syntax: None,
            code_backend: CodeBackend::Plain,
            structured_format: None,
            document_format: None,
        }
    }

    pub(super) const fn markdown() -> Self {
        Self {
            kind: PreviewKind::Markdown,
            language_hint: None,
            code_syntax: None,
            code_backend: CodeBackend::Plain,
            structured_format: None,
            document_format: None,
        }
    }

    pub(super) const fn iso() -> Self {
        Self {
            kind: PreviewKind::Iso,
            language_hint: None,
            code_syntax: None,
            code_backend: CodeBackend::Plain,
            structured_format: None,
            document_format: None,
        }
    }

    pub(super) const fn torrent() -> Self {
        Self {
            kind: PreviewKind::Torrent,
            language_hint: None,
            code_syntax: None,
            code_backend: CodeBackend::Plain,
            structured_format: None,
            document_format: None,
        }
    }

    pub(super) const fn source(
        language_hint: Option<&'static str>,
        highlight_language: Option<HighlightLanguage>,
        structured_format: Option<StructuredFormat>,
    ) -> Self {
        Self {
            kind: PreviewKind::Source,
            language_hint,
            code_syntax: resolve_code_syntax(language_hint, highlight_language, structured_format),
            code_backend: resolve_code_backend(highlight_language),
            structured_format,
            document_format: None,
        }
    }

    pub(crate) const fn code(
        code_syntax: &'static str,
        code_backend: CodeBackend,
        structured_format: Option<StructuredFormat>,
    ) -> Self {
        Self {
            kind: PreviewKind::Source,
            language_hint: Some(code_syntax),
            code_syntax: Some(code_syntax),
            code_backend,
            structured_format,
            document_format: None,
        }
    }

    pub(super) const fn document(document_format: DocumentFormat) -> Self {
        Self {
            kind: PreviewKind::PlainText,
            language_hint: None,
            code_syntax: None,
            code_backend: CodeBackend::Plain,
            structured_format: None,
            document_format: Some(document_format),
        }
    }

    pub(crate) fn highlight_language(self) -> Option<HighlightLanguage> {
        match self.code_backend {
            CodeBackend::Plain => None,
            CodeBackend::Syntect => self
                .code_syntax
                .and_then(HighlightLanguage::from_code_syntax),
            CodeBackend::Custom(kind) => Some(kind.highlight_language()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileFacts {
    pub builtin_class: FileClass,
    pub specific_type_label: Option<&'static str>,
    pub preview: PreviewSpec,
}

pub(super) const fn plain(
    class: FileClass,
    specific_type_label: Option<&'static str>,
) -> FileFacts {
    FileFacts {
        builtin_class: class,
        specific_type_label,
        preview: PreviewSpec::plain_text(),
    }
}

pub(super) const fn source_only(
    class: FileClass,
    specific_type_label: Option<&'static str>,
    language_hint: Option<&'static str>,
) -> FileFacts {
    FileFacts {
        builtin_class: class,
        specific_type_label,
        preview: PreviewSpec::source(language_hint, None, None),
    }
}

pub(super) const fn disk_image_file_facts(kind: DiskImageKind) -> FileFacts {
    plain(FileClass::File, Some(kind.detail_label()))
}

const fn resolve_code_backend(highlight_language: Option<HighlightLanguage>) -> CodeBackend {
    match highlight_language {
        Some(language) => language.code_backend(),
        None => CodeBackend::Plain,
    }
}

const fn resolve_code_syntax(
    language_hint: Option<&'static str>,
    highlight_language: Option<HighlightLanguage>,
    structured_format: Option<StructuredFormat>,
) -> Option<&'static str> {
    match structured_format {
        Some(format) => Some(format.code_syntax()),
        None => match language_hint {
            Some(language_hint) => Some(language_hint),
            None => match highlight_language {
                Some(language) => language.default_code_syntax(),
                None => None,
            },
        },
    }
}
