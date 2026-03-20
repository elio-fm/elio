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

    pub(super) const fn source(language_hint: Option<&'static str>) -> Self {
        Self {
            kind: PreviewKind::Source,
            language_hint,
            code_syntax: language_hint,
            code_backend: CodeBackend::Plain,
            structured_format: None,
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
        preview: PreviewSpec::source(language_hint),
    }
}

pub(super) const fn disk_image_file_facts(kind: DiskImageKind) -> FileFacts {
    plain(FileClass::File, Some(kind.detail_label()))
}
