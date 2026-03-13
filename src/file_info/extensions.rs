use super::types::{
    c_like_file_facts, cmake_file_facts, disk_image_file_facts, js_like_file_facts, nix_file_facts,
    plain, python_file_facts, shell_file_facts, source_only,
};
use super::{
    DiskImageKind, DocumentFormat, FileFacts, HighlightLanguage, PreviewSpec, StructuredFormat,
};
use crate::app::FileClass;

pub(super) fn inspect_extension(ext: &str) -> FileFacts {
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
                Some(HighlightLanguage::Json),
                Some(StructuredFormat::Json),
            ),
        },
        "jsonc" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("JSON with comments"),
            preview: PreviewSpec::source(
                Some("jsonc"),
                Some(HighlightLanguage::Jsonc),
                Some(StructuredFormat::Jsonc),
            ),
        },
        "json5" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("JSON5 file"),
            preview: PreviewSpec::source(
                Some("javascript"),
                Some(HighlightLanguage::Jsonc),
                Some(StructuredFormat::Json5),
            ),
        },
        "toml" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: None,
            preview: PreviewSpec::source(
                Some("toml"),
                Some(HighlightLanguage::Toml),
                Some(StructuredFormat::Toml),
            ),
        },
        "yaml" | "yml" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: None,
            preview: PreviewSpec::source(
                Some("yaml"),
                Some(HighlightLanguage::Yaml),
                Some(StructuredFormat::Yaml),
            ),
        },
        "html" | "htm" | "xhtml" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("HTML document"),
            preview: PreviewSpec::source(Some("html"), Some(HighlightLanguage::Markup), None),
        },
        "xml" | "xsd" | "xsl" | "xslt" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("XML document"),
            preview: PreviewSpec::source(Some("xml"), Some(HighlightLanguage::Markup), None),
        },
        "css" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Stylesheet"),
            preview: PreviewSpec::source(Some("css"), Some(HighlightLanguage::Css), None),
        },
        "scss" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("SCSS stylesheet"),
            preview: PreviewSpec::source(Some("scss"), Some(HighlightLanguage::Css), None),
        },
        "sass" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Sass stylesheet"),
            preview: PreviewSpec::source(Some("sass"), Some(HighlightLanguage::Css), None),
        },
        "less" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Less stylesheet"),
            preview: PreviewSpec::source(Some("css"), Some(HighlightLanguage::Css), None),
        },
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" => {
            js_like_file_facts(FileClass::Code, None)
        }
        "nix" => nix_file_facts(FileClass::Config, "Nix expression"),
        "cmake" => cmake_file_facts(FileClass::Config, "CMake script"),
        "lock" => FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: Some("Lockfile"),
            preview: PreviewSpec::source(None, Some(HighlightLanguage::Ini), None),
        },
        "ini" | "conf" | "cfg" | "keys" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: match ext {
                "keys" => Some("Keys file"),
                _ => None,
            },
            preview: PreviewSpec::source(None, Some(HighlightLanguage::Ini), None),
        },
        "env" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Environment file"),
            preview: PreviewSpec::source(
                None,
                Some(HighlightLanguage::Ini),
                Some(StructuredFormat::Dotenv),
            ),
        },
        "desktop" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Desktop Entry"),
            preview: PreviewSpec::highlighted_source(None, HighlightLanguage::DesktopEntry),
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
                Some(HighlightLanguage::Log),
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
            .with_highlight_language(HighlightLanguage::Make),
        "sh" => shell_file_facts(FileClass::Code, "Shell script", "sh"),
        "bash" => shell_file_facts(FileClass::Code, "Bash script", "bash"),
        "zsh" => shell_file_facts(FileClass::Code, "Zsh script", "zsh"),
        "ksh" => shell_file_facts(FileClass::Code, "KornShell script", "ksh"),
        "fish" => shell_file_facts(FileClass::Code, "Fish script", "fish"),
        "py" | "pyi" | "pyw" | "pyx" => python_file_facts(FileClass::Code, None),
        "rs" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Rust source file"),
            preview: PreviewSpec::source(Some("rust"), Some(HighlightLanguage::CLike), None),
        },
        "go" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Go source file"),
            preview: PreviewSpec::source(Some("go"), Some(HighlightLanguage::CLike), None),
        },
        "java" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Java source file"),
            preview: PreviewSpec::source(Some("java"), Some(HighlightLanguage::CLike), None),
        },
        "php" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("PHP script"),
            preview: PreviewSpec::source(Some("php"), Some(HighlightLanguage::CLike), None),
        },
        "swift" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Swift source file"),
            preview: PreviewSpec::source(Some("swift"), Some(HighlightLanguage::CLike), None),
        },
        "kt" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Kotlin source file"),
            preview: PreviewSpec::source(Some("kotlin"), Some(HighlightLanguage::CLike), None),
        },
        "rb" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Ruby script"),
            preview: PreviewSpec::source(Some("ruby"), Some(HighlightLanguage::Python), None),
        },
        "lua" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Lua script"),
            preview: PreviewSpec::source(None, None, None),
        },
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
            preview: PreviewSpec::source(Some("xml"), Some(HighlightLanguage::Markup), None),
        },
        "png" => plain(FileClass::Image, Some("PNG image")),
        "jpg" | "jpeg" => plain(FileClass::Image, Some("JPEG image")),
        "gif" => plain(FileClass::Image, Some("GIF image")),
        "webp" => plain(FileClass::Image, Some("WebP image")),
        "avif" => plain(FileClass::Image, Some("AVIF image")),
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => plain(FileClass::Audio, None),
        "mp4" | "mkv" | "mov" | "webm" | "avi" => plain(FileClass::Video, None),
        "zip" | "tar" | "gz" | "xz" | "bz2" | "7z" => plain(FileClass::Archive, None),
        "ttf" | "otf" | "woff" | "woff2" => plain(FileClass::Font, None),
        _ => plain(FileClass::File, None),
    }
}
