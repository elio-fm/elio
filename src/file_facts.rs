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
pub(crate) enum FallbackSyntax {
    JsLike,
    CLike,
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

impl FallbackSyntax {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::JsLike => "TypeScript",
            Self::CLike => "C",
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
    const fn detail_label(self) -> &'static str {
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
    if let Some(facts) = inspect_archive_name(&name) {
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
            preview: PreviewSpec::source(Some("bash"), Some(FallbackSyntax::Shell), None),
        }),
        "makefile" | "gnumakefile" | "bsdmakefile" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Makefile"),
            preview: PreviewSpec::source(Some("make"), Some(FallbackSyntax::Make), None),
        }),
        "cmakelists.txt" => Some(FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("CMake project"),
            preview: PreviewSpec::forced_fallback(FallbackSyntax::CMake),
        }),
        ".bashrc" | ".bash_profile" | ".bash_login" | ".bash_logout" | ".bash_aliases" => {
            Some(shell_file_facts(FileClass::Config, "Bash config", "bash"))
        }
        ".profile" | ".xprofile" | ".xsessionrc" | ".envrc" => {
            Some(shell_file_facts(FileClass::Config, "Shell config", "sh"))
        }
        ".zshrc" | ".zprofile" | ".zshenv" | ".zlogin" | ".zlogout" => {
            Some(shell_file_facts(FileClass::Config, "Zsh config", "zsh"))
        }
        ".kshrc" | ".mkshrc" => Some(shell_file_facts(
            FileClass::Config,
            "KornShell config",
            "ksh",
        )),
        "cargo.lock" | "poetry.lock" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: None,
            preview: PreviewSpec::source(
                Some("toml"),
                Some(FallbackSyntax::Toml),
                Some(StructuredFormat::Toml),
            ),
        }),
        "uv.lock" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: Some("Lockfile"),
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
        "composer.lock" | "pipfile.lock" | "flake.lock" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: Some("Lockfile"),
            preview: PreviewSpec::source(
                Some("json"),
                Some(FallbackSyntax::Json),
                Some(StructuredFormat::Json),
            ),
        }),
        "gemfile.lock" | "bun.lock" => Some(FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: Some("Lockfile"),
            preview: PreviewSpec::source(None, Some(FallbackSyntax::Ini), None),
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

fn inspect_archive_name(name: &str) -> Option<FileFacts> {
    let detail = if let Some(kind) = inspect_compound_archive_name(name) {
        Some(kind.detail_label())
    } else if name.ends_with(".zip") {
        Some("ZIP archive")
    } else if name.ends_with(".7z") {
        Some("7z archive")
    } else if name.ends_with(".jar") {
        Some("Java archive")
    } else if name.ends_with(".apk") {
        Some("Android package")
    } else if name.ends_with(".aab") {
        Some("Android App Bundle")
    } else if name.ends_with(".apkg") {
        Some("Anki package")
    } else {
        None
    }?;

    Some(plain(FileClass::Archive, Some(detail)))
}

pub(crate) fn inspect_compound_archive_name(name: &str) -> Option<CompoundArchiveKind> {
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        return Some(CompoundArchiveKind::TarGzip);
    }
    if name.ends_with(".tar.xz") || name.ends_with(".txz") {
        return Some(CompoundArchiveKind::TarXz);
    }
    if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") || name.ends_with(".tbz") {
        return Some(CompoundArchiveKind::TarBzip2);
    }
    if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
        return Some(CompoundArchiveKind::TarZstd);
    }

    inspect_compressed_disk_image_name(name)
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
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" => {
            js_like_file_facts(FileClass::Code, None)
        }
        "nix" => nix_file_facts(FileClass::Config, "Nix expression"),
        "cmake" => cmake_file_facts(FileClass::Config, "CMake script"),
        "lock" => FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: Some("Lockfile"),
            preview: PreviewSpec::source(None, Some(FallbackSyntax::Ini), None),
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
        "raw" => disk_image_file_facts(DiskImageKind::Raw),
        "img" => disk_image_file_facts(DiskImageKind::Img),
        "qcow2" => disk_image_file_facts(DiskImageKind::Qcow2),
        "vmdk" => disk_image_file_facts(DiskImageKind::Vmdk),
        "vdi" => disk_image_file_facts(DiskImageKind::Vdi),
        "vhd" => disk_image_file_facts(DiskImageKind::Vhd),
        "vhdx" => disk_image_file_facts(DiskImageKind::Vhdx),
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
        "dll" => plain(FileClass::File, Some("Windows DLL")),
        "sys" => plain(FileClass::File, Some("Windows system driver")),
        "msi" => plain(FileClass::File, Some("Windows Installer package")),
        "so" => plain(FileClass::File, Some("Shared library")),
        "dylib" => plain(FileClass::File, Some("Dynamic library")),
        "o" => plain(FileClass::File, Some("Object file")),
        "a" => plain(FileClass::File, Some("Static library")),
        "lib" => plain(FileClass::File, Some("Library file")),
        "jar" => plain(FileClass::Archive, Some("Java archive")),
        "c" => c_like_file_facts(FileClass::Code, "C source file", "c"),
        "h" => c_like_file_facts(FileClass::Code, "C header", "c"),
        "cpp" | "cc" | "cxx" => c_like_file_facts(FileClass::Code, "C++ source file", "cpp"),
        "hpp" | "hh" | "hxx" => c_like_file_facts(FileClass::Code, "C++ header", "cpp"),
        "mk" | "mak" => source_only(FileClass::Config, Some("Makefile"), Some("make"))
            .with_fallback(FallbackSyntax::Make),
        "sh" => shell_file_facts(FileClass::Code, "Shell script", "sh"),
        "bash" => shell_file_facts(FileClass::Code, "Bash script", "bash"),
        "zsh" => shell_file_facts(FileClass::Code, "Zsh script", "zsh"),
        "ksh" => shell_file_facts(FileClass::Code, "KornShell script", "ksh"),
        "fish" => shell_file_facts(FileClass::Code, "Fish script", "fish"),
        "py" | "pyi" | "pyw" | "pyx" => python_file_facts(FileClass::Code, None),
        "rs" | "go" | "java" | "lua" | "php" | "rb" | "swift" | "kt" => {
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
        "docm" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("DOCM document"),
            preview: PreviewSpec::document(DocumentFormat::Docm),
        },
        "odt" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("ODT document"),
            preview: PreviewSpec::document(DocumentFormat::Odt),
        },
        "ods" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("ODS spreadsheet"),
            preview: PreviewSpec::document(DocumentFormat::Ods),
        },
        "odp" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("ODP presentation"),
            preview: PreviewSpec::document(DocumentFormat::Odp),
        },
        "pptx" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("PPTX presentation"),
            preview: PreviewSpec::document(DocumentFormat::Pptx),
        },
        "pptm" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("PPTM presentation"),
            preview: PreviewSpec::document(DocumentFormat::Pptm),
        },
        "xlsx" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("XLSX spreadsheet"),
            preview: PreviewSpec::document(DocumentFormat::Xlsx),
        },
        "xlsm" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("XLSM spreadsheet"),
            preview: PreviewSpec::document(DocumentFormat::Xlsm),
        },
        "pages" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("Pages document"),
            preview: PreviewSpec::document(DocumentFormat::Pages),
        },
        "epub" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("EPUB ebook"),
            preview: PreviewSpec::document(DocumentFormat::Epub),
        },
        "pdf" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("PDF document"),
            preview: PreviewSpec::document(DocumentFormat::Pdf),
        },
        "txt" | "rst" => plain(FileClass::Document, None),
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

const fn shell_file_facts(
    class: FileClass,
    specific_type_label: &'static str,
    syntax_hint: &'static str,
) -> FileFacts {
    source_only(class, Some(specific_type_label), Some(syntax_hint))
        .with_fallback(FallbackSyntax::Shell)
        .prefer_fallback()
}

const fn c_like_file_facts(
    class: FileClass,
    specific_type_label: &'static str,
    syntax_hint: &'static str,
) -> FileFacts {
    source_only(class, Some(specific_type_label), Some(syntax_hint))
        .with_fallback(FallbackSyntax::CLike)
        .prefer_fallback()
}

const fn python_file_facts(
    class: FileClass,
    specific_type_label: Option<&'static str>,
) -> FileFacts {
    source_only(class, specific_type_label, Some("python"))
        .with_fallback(FallbackSyntax::Python)
        .prefer_fallback()
}

const fn js_like_file_facts(
    class: FileClass,
    specific_type_label: Option<&'static str>,
) -> FileFacts {
    source_only(class, specific_type_label, None)
        .with_fallback(FallbackSyntax::JsLike)
        .prefer_fallback()
}

const fn nix_file_facts(class: FileClass, specific_type_label: &'static str) -> FileFacts {
    source_only(class, Some(specific_type_label), Some("nix"))
        .with_fallback(FallbackSyntax::Nix)
        .prefer_fallback()
}

const fn cmake_file_facts(class: FileClass, specific_type_label: &'static str) -> FileFacts {
    source_only(class, Some(specific_type_label), Some("cmake"))
        .with_fallback(FallbackSyntax::CMake)
        .prefer_fallback()
}

const fn disk_image_file_facts(kind: DiskImageKind) -> FileFacts {
    plain(FileClass::File, Some(kind.detail_label()))
}

impl FileFacts {
    const fn with_fallback(mut self, fallback_syntax: FallbackSyntax) -> Self {
        self.preview.fallback_syntax = Some(fallback_syntax);
        self
    }

    const fn prefer_fallback(mut self) -> Self {
        self.preview.force_fallback = true;
        self
    }
}

fn is_env_name(name: &str) -> bool {
    name == ".env" || name.starts_with(".env.")
}

fn inspect_compressed_disk_image_name(name: &str) -> Option<CompoundArchiveKind> {
    compressed_disk_image_kind(name, ".gz", CompressionKind::Gzip)
        .or_else(|| compressed_disk_image_kind(name, ".xz", CompressionKind::Xz))
        .or_else(|| compressed_disk_image_kind(name, ".bz2", CompressionKind::Bzip2))
        .or_else(|| compressed_disk_image_kind(name, ".zst", CompressionKind::Zstd))
}

fn compressed_disk_image_kind(
    name: &str,
    compression_suffix: &str,
    compression: CompressionKind,
) -> Option<CompoundArchiveKind> {
    name.strip_suffix(compression_suffix)
        .and_then(disk_image_kind_from_name)
        .map(|image| CompoundArchiveKind::CompressedDiskImage { image, compression })
}

fn disk_image_kind_from_name(name: &str) -> Option<DiskImageKind> {
    if name.ends_with(".raw") {
        Some(DiskImageKind::Raw)
    } else if name.ends_with(".img") {
        Some(DiskImageKind::Img)
    } else if name.ends_with(".iso") {
        Some(DiskImageKind::Iso)
    } else if name.ends_with(".qcow2") {
        Some(DiskImageKind::Qcow2)
    } else if name.ends_with(".vmdk") {
        Some(DiskImageKind::Vmdk)
    } else if name.ends_with(".vdi") {
        Some(DiskImageKind::Vdi)
    } else if name.ends_with(".vhd") {
        Some(DiskImageKind::Vhd)
    } else if name.ends_with(".vhdx") {
        Some(DiskImageKind::Vhdx)
    } else {
        None
    }
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
    fn lockfile_variants_get_targeted_preview_support() {
        let uv = inspect_path(Path::new("uv.lock"), EntryKind::File);
        let flake = inspect_path(Path::new("flake.lock"), EntryKind::File);
        let gem = inspect_path(Path::new("Gemfile.lock"), EntryKind::File);
        let generic = inspect_path(Path::new("deps.lock"), EntryKind::File);

        assert_eq!(uv.preview.structured_format, Some(StructuredFormat::Toml));
        assert_eq!(uv.preview.fallback_syntax, Some(FallbackSyntax::Toml));

        assert_eq!(
            flake.preview.structured_format,
            Some(StructuredFormat::Json)
        );
        assert_eq!(flake.preview.fallback_syntax, Some(FallbackSyntax::Json));

        assert_eq!(gem.specific_type_label, Some("Lockfile"));
        assert_eq!(gem.preview.fallback_syntax, Some(FallbackSyntax::Ini));

        assert_eq!(generic.specific_type_label, Some("Lockfile"));
        assert_eq!(generic.preview.fallback_syntax, Some(FallbackSyntax::Ini));
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
    fn nix_and_cmake_files_use_code_preview_support() {
        let nix = inspect_path(Path::new("flake.nix"), EntryKind::File);
        let cmake = inspect_path(Path::new("toolchain.cmake"), EntryKind::File);
        let cmakelists = inspect_path(Path::new("CMakeLists.txt"), EntryKind::File);

        assert_eq!(nix.builtin_class, FileClass::Config);
        assert_eq!(nix.specific_type_label, Some("Nix expression"));
        assert_eq!(nix.preview.syntax_hint, Some("nix"));

        assert_eq!(cmake.builtin_class, FileClass::Config);
        assert_eq!(cmake.specific_type_label, Some("CMake script"));
        assert_eq!(cmake.preview.fallback_syntax, Some(FallbackSyntax::CMake));
        assert!(cmake.preview.force_fallback);

        assert_eq!(cmakelists.builtin_class, FileClass::Config);
        assert_eq!(cmakelists.specific_type_label, Some("CMake project"));
        assert_eq!(
            cmakelists.preview.fallback_syntax,
            Some(FallbackSyntax::CMake)
        );
        assert!(cmakelists.preview.force_fallback);
    }

    #[test]
    fn make_and_c_files_get_targeted_preview_support() {
        let makefile = inspect_path(Path::new("Makefile"), EntryKind::File);
        let c_source = inspect_path(Path::new("main.c"), EntryKind::File);
        let c_header = inspect_path(Path::new("app.h"), EntryKind::File);

        assert_eq!(makefile.builtin_class, FileClass::Config);
        assert_eq!(makefile.specific_type_label, Some("Makefile"));
        assert_eq!(makefile.preview.syntax_hint, Some("make"));
        assert_eq!(makefile.preview.fallback_syntax, Some(FallbackSyntax::Make));

        assert_eq!(c_source.builtin_class, FileClass::Code);
        assert_eq!(c_source.specific_type_label, Some("C source file"));
        assert_eq!(c_source.preview.syntax_hint, Some("c"));
        assert_eq!(
            c_source.preview.fallback_syntax,
            Some(FallbackSyntax::CLike)
        );
        assert!(c_source.preview.force_fallback);

        assert_eq!(c_header.builtin_class, FileClass::Code);
        assert_eq!(c_header.specific_type_label, Some("C header"));
        assert_eq!(c_header.preview.syntax_hint, Some("c"));
        assert_eq!(
            c_header.preview.fallback_syntax,
            Some(FallbackSyntax::CLike)
        );
        assert!(c_header.preview.force_fallback);
    }

    #[test]
    fn shell_files_and_dotfiles_get_targeted_preview_support() {
        let shell = inspect_path(Path::new("deploy.sh"), EntryKind::File);
        let bashrc = inspect_path(Path::new(".bashrc"), EntryKind::File);
        let zsh = inspect_path(Path::new("prompt.zsh"), EntryKind::File);
        let fish = inspect_path(Path::new("config.fish"), EntryKind::File);
        let zshrc = inspect_path(Path::new(".zshrc"), EntryKind::File);

        assert_eq!(shell.builtin_class, FileClass::Code);
        assert_eq!(shell.specific_type_label, Some("Shell script"));
        assert_eq!(shell.preview.syntax_hint, Some("sh"));
        assert_eq!(shell.preview.fallback_syntax, Some(FallbackSyntax::Shell));
        assert!(shell.preview.force_fallback);

        assert_eq!(bashrc.builtin_class, FileClass::Config);
        assert_eq!(bashrc.specific_type_label, Some("Bash config"));
        assert_eq!(bashrc.preview.syntax_hint, Some("bash"));
        assert_eq!(bashrc.preview.fallback_syntax, Some(FallbackSyntax::Shell));
        assert!(bashrc.preview.force_fallback);

        assert_eq!(zsh.builtin_class, FileClass::Code);
        assert_eq!(zsh.specific_type_label, Some("Zsh script"));
        assert_eq!(zsh.preview.syntax_hint, Some("zsh"));
        assert_eq!(zsh.preview.fallback_syntax, Some(FallbackSyntax::Shell));
        assert!(zsh.preview.force_fallback);

        assert_eq!(fish.builtin_class, FileClass::Code);
        assert_eq!(fish.specific_type_label, Some("Fish script"));
        assert_eq!(fish.preview.syntax_hint, Some("fish"));
        assert_eq!(fish.preview.fallback_syntax, Some(FallbackSyntax::Shell));
        assert!(fish.preview.force_fallback);

        assert_eq!(zshrc.builtin_class, FileClass::Config);
        assert_eq!(zshrc.specific_type_label, Some("Zsh config"));
        assert_eq!(zshrc.preview.syntax_hint, Some("zsh"));
        assert_eq!(zshrc.preview.fallback_syntax, Some(FallbackSyntax::Shell));
        assert!(zshrc.preview.force_fallback);
    }

    #[test]
    fn js_like_files_prefer_targeted_fallback_support() {
        let js = inspect_path(Path::new("main.js"), EntryKind::File);
        let tsx = inspect_path(Path::new("App.tsx"), EntryKind::File);

        assert_eq!(js.builtin_class, FileClass::Code);
        assert_eq!(js.preview.fallback_syntax, Some(FallbackSyntax::JsLike));
        assert!(js.preview.force_fallback);

        assert_eq!(tsx.builtin_class, FileClass::Code);
        assert_eq!(tsx.preview.fallback_syntax, Some(FallbackSyntax::JsLike));
        assert!(tsx.preview.force_fallback);
    }

    #[test]
    fn python_family_files_prefer_targeted_fallback_support() {
        let py = inspect_path(Path::new("main.py"), EntryKind::File);
        let pyi = inspect_path(Path::new("types.pyi"), EntryKind::File);

        assert_eq!(py.builtin_class, FileClass::Code);
        assert_eq!(py.preview.syntax_hint, Some("python"));
        assert_eq!(py.preview.fallback_syntax, Some(FallbackSyntax::Python));
        assert!(py.preview.force_fallback);

        assert_eq!(pyi.builtin_class, FileClass::Code);
        assert_eq!(pyi.preview.syntax_hint, Some("python"));
        assert_eq!(pyi.preview.fallback_syntax, Some(FallbackSyntax::Python));
        assert!(pyi.preview.force_fallback);
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
        let docm = inspect_path(Path::new("report.docm"), EntryKind::File);
        let odt = inspect_path(Path::new("report.odt"), EntryKind::File);
        let ods = inspect_path(Path::new("budget.ods"), EntryKind::File);
        let odp = inspect_path(Path::new("deck.odp"), EntryKind::File);
        let pptx = inspect_path(Path::new("deck.pptx"), EntryKind::File);
        let xlsx = inspect_path(Path::new("budget.xlsx"), EntryKind::File);
        let pages = inspect_path(Path::new("proposal.pages"), EntryKind::File);
        let epub = inspect_path(Path::new("novel.epub"), EntryKind::File);
        let pdf = inspect_path(Path::new("manual.pdf"), EntryKind::File);

        assert_eq!(doc.builtin_class, FileClass::Document);
        assert_eq!(doc.preview.document_format, Some(DocumentFormat::Doc));
        assert_eq!(doc.specific_type_label, Some("DOC document"));

        assert_eq!(docx.builtin_class, FileClass::Document);
        assert_eq!(docx.preview.document_format, Some(DocumentFormat::Docx));
        assert_eq!(docx.specific_type_label, Some("DOCX document"));

        assert_eq!(docm.builtin_class, FileClass::Document);
        assert_eq!(docm.preview.document_format, Some(DocumentFormat::Docm));
        assert_eq!(docm.specific_type_label, Some("DOCM document"));

        assert_eq!(odt.builtin_class, FileClass::Document);
        assert_eq!(odt.preview.document_format, Some(DocumentFormat::Odt));
        assert_eq!(odt.specific_type_label, Some("ODT document"));

        assert_eq!(ods.builtin_class, FileClass::Document);
        assert_eq!(ods.preview.document_format, Some(DocumentFormat::Ods));
        assert_eq!(ods.specific_type_label, Some("ODS spreadsheet"));

        assert_eq!(odp.builtin_class, FileClass::Document);
        assert_eq!(odp.preview.document_format, Some(DocumentFormat::Odp));
        assert_eq!(odp.specific_type_label, Some("ODP presentation"));

        assert_eq!(pptx.builtin_class, FileClass::Document);
        assert_eq!(pptx.preview.document_format, Some(DocumentFormat::Pptx));
        assert_eq!(pptx.specific_type_label, Some("PPTX presentation"));

        assert_eq!(xlsx.builtin_class, FileClass::Document);
        assert_eq!(xlsx.preview.document_format, Some(DocumentFormat::Xlsx));
        assert_eq!(xlsx.specific_type_label, Some("XLSX spreadsheet"));

        assert_eq!(pages.builtin_class, FileClass::Document);
        assert_eq!(pages.preview.document_format, Some(DocumentFormat::Pages));
        assert_eq!(pages.specific_type_label, Some("Pages document"));

        assert_eq!(epub.builtin_class, FileClass::Document);
        assert_eq!(epub.preview.document_format, Some(DocumentFormat::Epub));
        assert_eq!(epub.specific_type_label, Some("EPUB ebook"));

        assert_eq!(pdf.builtin_class, FileClass::Document);
        assert_eq!(pdf.preview.document_format, Some(DocumentFormat::Pdf));
        assert_eq!(pdf.specific_type_label, Some("PDF document"));
    }

    #[test]
    fn archive_suffixes_keep_specific_labels_for_common_multi_part_formats() {
        let tgz = inspect_path(Path::new("release.tar.gz"), EntryKind::File);
        let txz = inspect_path(Path::new("release.tar.xz"), EntryKind::File);
        let tbz2 = inspect_path(Path::new("release.tar.bz2"), EntryKind::File);
        let zip = inspect_path(Path::new("release.zip"), EntryKind::File);
        let seven_zip = inspect_path(Path::new("release.7z"), EntryKind::File);

        assert_eq!(tgz.builtin_class, FileClass::Archive);
        assert_eq!(tgz.specific_type_label, Some("TAR.GZ archive"));
        assert_eq!(txz.specific_type_label, Some("TAR.XZ archive"));
        assert_eq!(tbz2.specific_type_label, Some("TAR.BZ2 archive"));
        assert_eq!(zip.specific_type_label, Some("ZIP archive"));
        assert_eq!(seven_zip.specific_type_label, Some("7z archive"));
    }

    #[test]
    fn compressed_disk_images_get_specific_labels() {
        let raw_xz = inspect_path(Path::new("fedora.aarch64.raw.xz"), EntryKind::File);
        let iso_zst = inspect_path(Path::new("installer.iso.zst"), EntryKind::File);
        let qcow2_gz = inspect_path(Path::new("vm.qcow2.gz"), EntryKind::File);
        let vmdk_bz2 = inspect_path(Path::new("appliance.vmdk.bz2"), EntryKind::File);

        assert_eq!(raw_xz.builtin_class, FileClass::Archive);
        assert_eq!(
            raw_xz.specific_type_label,
            Some("XZ-compressed raw disk image")
        );
        assert_eq!(
            iso_zst.specific_type_label,
            Some("Zstandard-compressed ISO disk image")
        );
        assert_eq!(
            qcow2_gz.specific_type_label,
            Some("Gzip-compressed QCOW2 disk image")
        );
        assert_eq!(
            vmdk_bz2.specific_type_label,
            Some("Bzip2-compressed VMDK disk image")
        );
    }

    #[test]
    fn common_disk_image_extensions_keep_specific_labels_without_archive_mode() {
        let raw = inspect_path(Path::new("disk.raw"), EntryKind::File);
        let img = inspect_path(Path::new("disk.img"), EntryKind::File);
        let qcow2 = inspect_path(Path::new("vm.qcow2"), EntryKind::File);
        let vhdx = inspect_path(Path::new("backup.vhdx"), EntryKind::File);

        assert_eq!(raw.builtin_class, FileClass::File);
        assert_eq!(raw.specific_type_label, Some("Raw disk image"));
        assert_eq!(img.builtin_class, FileClass::File);
        assert_eq!(img.specific_type_label, Some("Disk image"));
        assert_eq!(qcow2.builtin_class, FileClass::File);
        assert_eq!(qcow2.specific_type_label, Some("QCOW2 disk image"));
        assert_eq!(vhdx.builtin_class, FileClass::File);
        assert_eq!(vhdx.specific_type_label, Some("VHDX disk image"));
    }

    #[test]
    fn executable_and_library_extensions_keep_specific_labels() {
        let dll = inspect_path(Path::new("plugin.dll"), EntryKind::File);
        let sys = inspect_path(Path::new("driver.sys"), EntryKind::File);
        let so = inspect_path(Path::new("libelio.so"), EntryKind::File);
        let dylib = inspect_path(Path::new("libelio.dylib"), EntryKind::File);
        let object = inspect_path(Path::new("main.o"), EntryKind::File);

        assert_eq!(dll.specific_type_label, Some("Windows DLL"));
        assert_eq!(sys.specific_type_label, Some("Windows system driver"));
        assert_eq!(so.specific_type_label, Some("Shared library"));
        assert_eq!(dylib.specific_type_label, Some("Dynamic library"));
        assert_eq!(object.specific_type_label, Some("Object file"));
    }
}
