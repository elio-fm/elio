use super::types::{disk_image_file_facts, plain, source_only};
use super::{DiskImageKind, DocumentFormat, FileFacts, PreviewSpec};
use crate::{app::FileClass, preview::code::registry};

fn preview_for_extension(ext: &str) -> PreviewSpec {
    registry::language_for_extension(ext)
        .expect("extension registry entry should exist for code preview")
        .preview_spec()
}

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
            preview: preview_for_extension(ext),
        },
        "jsonc" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("JSON with comments"),
            preview: preview_for_extension(ext),
        },
        "json5" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("JSON5 file"),
            preview: preview_for_extension(ext),
        },
        "toml" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: None,
            preview: preview_for_extension(ext),
        },
        "yaml" | "yml" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: None,
            preview: preview_for_extension(ext),
        },
        "html" | "htm" | "xhtml" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("HTML document"),
            preview: preview_for_extension(ext),
        },
        "xml" | "xsd" | "xsl" | "xslt" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("XML document"),
            preview: preview_for_extension(ext),
        },
        "css" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Stylesheet"),
            preview: preview_for_extension(ext),
        },
        "scss" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("SCSS stylesheet"),
            preview: preview_for_extension(ext),
        },
        "sass" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Sass stylesheet"),
            preview: preview_for_extension(ext),
        },
        "less" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Less stylesheet"),
            preview: preview_for_extension(ext),
        },
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: None,
            preview: preview_for_extension(ext),
        },
        "sql" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("SQL script"),
            preview: preview_for_extension(ext),
        },
        "diff" | "patch" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some(match ext {
                "patch" => "Patch file",
                _ => "Diff file",
            }),
            preview: preview_for_extension(ext),
        },
        "nix" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Nix expression"),
            preview: preview_for_extension(ext),
        },
        "hcl" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("HCL config"),
            preview: preview_for_extension(ext),
        },
        "tf" | "tfvars" | "tfbackend" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some(match ext {
                "tfvars" => "Terraform variables",
                "tfbackend" => "Terraform backend config",
                _ => "Terraform module",
            }),
            preview: preview_for_extension(ext),
        },
        "cmake" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("CMake script"),
            preview: preview_for_extension(ext),
        },
        "lock" => FileFacts {
            builtin_class: FileClass::Data,
            specific_type_label: Some("Lockfile"),
            preview: preview_for_extension(ext),
        },
        "ini" | "keys" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: match ext {
                "keys" => Some("Keys file"),
                _ => None,
            },
            preview: preview_for_extension(ext),
        },
        "conf" | "cfg" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: None,
            preview: preview_for_extension(ext),
        },
        "env" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Environment file"),
            preview: preview_for_extension(ext),
        },
        "desktop" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Desktop Entry"),
            preview: preview_for_extension(ext),
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
            preview: preview_for_extension(ext),
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
        "c" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("C source file"),
            preview: preview_for_extension(ext),
        },
        "h" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("C header"),
            preview: preview_for_extension(ext),
        },
        "cpp" | "cc" | "cxx" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("C++ source file"),
            preview: preview_for_extension(ext),
        },
        "hpp" | "hh" | "hxx" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("C++ header"),
            preview: preview_for_extension(ext),
        },
        "mk" | "mak" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Makefile"),
            preview: preview_for_extension(ext),
        },
        "sh" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Shell script"),
            preview: preview_for_extension(ext),
        },
        "bash" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Bash script"),
            preview: preview_for_extension(ext),
        },
        "zsh" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Zsh script"),
            preview: preview_for_extension(ext),
        },
        "ksh" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("KornShell script"),
            preview: preview_for_extension(ext),
        },
        "fish" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Fish script"),
            preview: preview_for_extension(ext),
        },
        "ps1" | "psm1" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some(match ext {
                "psm1" => "PowerShell module",
                _ => "PowerShell script",
            }),
            preview: preview_for_extension(ext),
        },
        "psd1" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("PowerShell data file"),
            preview: preview_for_extension(ext),
        },
        "py" | "pyi" | "pyw" | "pyx" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: None,
            preview: preview_for_extension(ext),
        },
        "rs" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Rust source file"),
            preview: preview_for_extension(ext),
        },
        "go" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Go source file"),
            preview: preview_for_extension(ext),
        },
        "java" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Java source file"),
            preview: preview_for_extension(ext),
        },
        "php" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("PHP script"),
            preview: preview_for_extension(ext),
        },
        "swift" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Swift source file"),
            preview: preview_for_extension(ext),
        },
        "kt" | "kts" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some(match ext {
                "kts" => "Kotlin script",
                _ => "Kotlin source file",
            }),
            preview: preview_for_extension(ext),
        },
        "cs" | "csx" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some(match ext {
                "csx" => "C# script",
                _ => "C# source file",
            }),
            preview: preview_for_extension(ext),
        },
        "dart" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Dart source file"),
            preview: preview_for_extension(ext),
        },
        "zig" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Zig source file"),
            preview: preview_for_extension(ext),
        },
        "groovy" | "gvy" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Groovy source file"),
            preview: preview_for_extension(ext),
        },
        "gradle" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("Gradle build script"),
            preview: preview_for_extension(ext),
        },
        "scala" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Scala source file"),
            preview: preview_for_extension(ext),
        },
        "sbt" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("sbt build definition"),
            preview: preview_for_extension(ext),
        },
        "pl" | "pm" | "pod" | "t" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some(match ext {
                "pm" => "Perl module",
                "pod" => "Perl POD file",
                "t" => "Perl test script",
                _ => "Perl script",
            }),
            preview: preview_for_extension(ext),
        },
        "hs" | "lhs" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some(match ext {
                "lhs" => "Literate Haskell source file",
                _ => "Haskell source file",
            }),
            preview: preview_for_extension(ext),
        },
        "jl" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Julia source file"),
            preview: preview_for_extension(ext),
        },
        "r" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("R script"),
            preview: preview_for_extension(ext),
        },
        "ex" | "exs" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some(match ext {
                "exs" => "Elixir script",
                _ => "Elixir source file",
            }),
            preview: preview_for_extension(ext),
        },
        "clj" | "cljs" | "cljc" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some(match ext {
                "cljs" => "ClojureScript source file",
                "cljc" => "Portable Clojure source file",
                _ => "Clojure source file",
            }),
            preview: preview_for_extension(ext),
        },
        "edn" => FileFacts {
            builtin_class: FileClass::Config,
            specific_type_label: Some("EDN file"),
            preview: preview_for_extension(ext),
        },
        "rb" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Ruby script"),
            preview: preview_for_extension(ext),
        },
        "lua" => FileFacts {
            builtin_class: FileClass::Code,
            specific_type_label: Some("Lua script"),
            preview: preview_for_extension(ext),
        },
        "ron" => source_only(FileClass::Config, None, None),
        "csv" | "tsv" | "sqlite" | "db" | "parquet" => source_only(FileClass::Data, None, None),
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
        "cbz" => plain(FileClass::Archive, Some("Comic ZIP archive")),
        "cbr" => plain(FileClass::Archive, Some("Comic RAR archive")),
        "pdf" => FileFacts {
            builtin_class: FileClass::Document,
            specific_type_label: Some("PDF document"),
            preview: PreviewSpec::document(DocumentFormat::Pdf),
        },
        "txt" | "rst" => plain(FileClass::Document, None),
        "svg" => FileFacts {
            builtin_class: FileClass::Image,
            specific_type_label: Some("SVG image"),
            preview: preview_for_extension(ext),
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
