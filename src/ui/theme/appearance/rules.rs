use super::types::{ClassStyle, CodePreviewPalette, Palette, PreviewTheme, RuleOverride, Theme};
use crate::app::FileClass;
use ratatui::style::Color;
use std::collections::HashMap;

impl Theme {
    pub(super) fn base_theme() -> Self {
        let mut classes = HashMap::new();
        classes.insert(
            FileClass::Directory,
            ClassStyle {
                icon: "󰉋".to_string(),
                color: rgb(65, 143, 222),
            },
        );
        classes.insert(
            FileClass::Code,
            ClassStyle {
                icon: "󰆍".to_string(),
                color: rgb(87, 196, 155),
            },
        );
        classes.insert(
            FileClass::Config,
            ClassStyle {
                icon: "󰒓".to_string(),
                color: rgb(121, 188, 255),
            },
        );
        classes.insert(
            FileClass::Document,
            ClassStyle {
                icon: "󰈙".to_string(),
                color: rgb(112, 182, 117),
            },
        );
        classes.insert(
            FileClass::License,
            ClassStyle {
                icon: "󰿃".to_string(),
                color: rgb(245, 216, 91),
            },
        );
        classes.insert(
            FileClass::Image,
            ClassStyle {
                icon: "󰋩".to_string(),
                color: rgb(86, 156, 214),
            },
        );
        classes.insert(
            FileClass::Audio,
            ClassStyle {
                icon: "󰎆".to_string(),
                color: rgb(138, 110, 214),
            },
        );
        classes.insert(
            FileClass::Video,
            ClassStyle {
                icon: "".to_string(),
                color: rgb(204, 112, 79),
            },
        );
        classes.insert(
            FileClass::Archive,
            ClassStyle {
                icon: "󰗄".to_string(),
                color: rgb(207, 111, 63),
            },
        );
        classes.insert(
            FileClass::Font,
            ClassStyle {
                icon: "󰛖".to_string(),
                color: rgb(196, 148, 92),
            },
        );
        classes.insert(
            FileClass::Data,
            ClassStyle {
                icon: "󰆼".to_string(),
                color: rgb(92, 192, 201),
            },
        );
        classes.insert(
            FileClass::File,
            ClassStyle {
                icon: "󰈔".to_string(),
                color: rgb(98, 109, 122),
            },
        );

        let extensions = HashMap::from([
            ("rs".to_string(), rule_class(FileClass::Code)),
            ("js".to_string(), rule_class(FileClass::Code)),
            ("ts".to_string(), rule_class(FileClass::Code)),
            ("tsx".to_string(), rule_class(FileClass::Code)),
            ("jsx".to_string(), rule_class(FileClass::Code)),
            (
                "sql".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(92, 192, 201)),
                },
            ),
            (
                "diff".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(255, 184, 107)),
                },
            ),
            (
                "patch".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(255, 184, 107)),
                },
            ),
            (
                "hcl".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "tf".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "tfvars".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "tfbackend".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "groovy".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(112, 182, 117)),
                },
            ),
            (
                "gvy".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(112, 182, 117)),
                },
            ),
            (
                "gradle".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(112, 182, 117)),
                },
            ),
            (
                "scala".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(232, 90, 90)),
                },
            ),
            (
                "sbt".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(232, 90, 90)),
                },
            ),
            (
                "pl".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(125, 176, 255)),
                },
            ),
            (
                "pm".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(125, 176, 255)),
                },
            ),
            (
                "pod".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(125, 176, 255)),
                },
            ),
            (
                "hs".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "lhs".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "jl".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(193, 120, 255)),
                },
            ),
            (
                "r".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰟔".to_string()),
                    color: Some(rgb(95, 153, 219)),
                },
            ),
            (
                "just".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(255, 184, 107)),
                },
            ),
            (
                "ziggy".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(245, 173, 64)),
                },
            ),
            ("py".to_string(), rule_class(FileClass::Code)),
            ("go".to_string(), rule_class(FileClass::Code)),
            ("c".to_string(), rule_class(FileClass::Code)),
            ("cpp".to_string(), rule_class(FileClass::Code)),
            ("h".to_string(), rule_class(FileClass::Code)),
            ("hpp".to_string(), rule_class(FileClass::Code)),
            (
                "cs".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰌛".to_string()),
                    color: Some(rgb(104, 179, 120)),
                },
            ),
            (
                "csx".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰌛".to_string()),
                    color: Some(rgb(104, 179, 120)),
                },
            ),
            (
                "dart".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(56, 213, 255)),
                },
            ),
            (
                "f".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󱈚".to_string()),
                    color: Some(rgb(115, 79, 150)),
                },
            ),
            (
                "for".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󱈚".to_string()),
                    color: Some(rgb(115, 79, 150)),
                },
            ),
            (
                "f90".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󱈚".to_string()),
                    color: Some(rgb(115, 79, 150)),
                },
            ),
            (
                "f95".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󱈚".to_string()),
                    color: Some(rgb(115, 79, 150)),
                },
            ),
            (
                "f03".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󱈚".to_string()),
                    color: Some(rgb(115, 79, 150)),
                },
            ),
            (
                "f08".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󱈚".to_string()),
                    color: Some(rgb(115, 79, 150)),
                },
            ),
            (
                "fpp".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󱈚".to_string()),
                    color: Some(rgb(115, 79, 150)),
                },
            ),
            (
                "cbl".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(0, 92, 165)),
                },
            ),
            (
                "cob".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(0, 92, 165)),
                },
            ),
            (
                "cobol".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(0, 92, 165)),
                },
            ),
            (
                "cpy".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(0, 92, 165)),
                },
            ),
            ("java".to_string(), rule_class(FileClass::Code)),
            ("lua".to_string(), rule_class(FileClass::Code)),
            ("php".to_string(), rule_class(FileClass::Code)),
            ("rb".to_string(), rule_class(FileClass::Code)),
            (
                "ex".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(155, 143, 199)),
                },
            ),
            (
                "exs".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(155, 143, 199)),
                },
            ),
            (
                "clj".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(128, 176, 92)),
                },
            ),
            (
                "cljs".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(128, 176, 92)),
                },
            ),
            (
                "cljc".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(128, 176, 92)),
                },
            ),
            (
                "edn".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(128, 176, 92)),
                },
            ),
            ("swift".to_string(), rule_class(FileClass::Code)),
            ("kt".to_string(), rule_class(FileClass::Code)),
            (
                "ps1".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰨊".to_string()),
                    color: Some(rgb(95, 153, 219)),
                },
            ),
            (
                "psm1".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰨊".to_string()),
                    color: Some(rgb(95, 153, 219)),
                },
            ),
            (
                "psd1".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("󰨊".to_string()),
                    color: Some(rgb(95, 153, 219)),
                },
            ),
            (
                "sh".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(214, 222, 240)),
                },
            ),
            (
                "bash".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(214, 222, 240)),
                },
            ),
            (
                "zsh".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(214, 222, 240)),
                },
            ),
            (
                "fish".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("".to_string()),
                    color: Some(rgb(214, 222, 240)),
                },
            ),
            (
                "json".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(125, 176, 255)),
                },
            ),
            (
                "jsonc".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(125, 176, 255)),
                },
            ),
            (
                "json5".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(125, 176, 255)),
                },
            ),
            (
                "toml".to_string(),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: None,
                },
            ),
            ("yaml".to_string(), rule_class(FileClass::Config)),
            ("yml".to_string(), rule_class(FileClass::Config)),
            ("ini".to_string(), rule_class(FileClass::Config)),
            ("conf".to_string(), rule_class(FileClass::Config)),
            ("cfg".to_string(), rule_class(FileClass::Config)),
            ("desktop".to_string(), rule_class(FileClass::Config)),
            ("ron".to_string(), rule_class(FileClass::Config)),
            ("env".to_string(), rule_class(FileClass::Config)),
            (
                "xml".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰗀".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "xsd".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰗀".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "xsl".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰗀".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "xslt".to_string(),
                RuleOverride {
                    class: Some(FileClass::Code),
                    icon: Some("󰗀".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                "md".to_string(),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("".to_string()),
                    color: Some(rgb(211, 170, 124)),
                },
            ),
            (
                "markdown".to_string(),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("".to_string()),
                    color: Some(rgb(211, 170, 124)),
                },
            ),
            (
                "mdown".to_string(),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("".to_string()),
                    color: Some(rgb(211, 170, 124)),
                },
            ),
            (
                "mkd".to_string(),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("".to_string()),
                    color: Some(rgb(211, 170, 124)),
                },
            ),
            (
                "mdx".to_string(),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("".to_string()),
                    color: Some(rgb(211, 170, 124)),
                },
            ),
            (
                "txt".to_string(),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("".to_string()),
                    color: Some(rgb(174, 184, 199)),
                },
            ),
            ("rst".to_string(), rule_class(FileClass::Document)),
            (
                "lock".to_string(),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(89, 222, 148)),
                },
            ),
            ("pdf".to_string(), rule_class(FileClass::Document)),
            (
                "epub".to_string(),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("󱗖".to_string()),
                    color: Some(rgb(211, 170, 124)),
                },
            ),
            (
                "cbz".to_string(),
                RuleOverride {
                    class: Some(FileClass::Archive),
                    icon: Some("󱗖".to_string()),
                    color: Some(rgb(211, 170, 124)),
                },
            ),
            ("doc".to_string(), rule_document_file()),
            ("docx".to_string(), rule_document_file()),
            ("docm".to_string(), rule_document_file()),
            ("odt".to_string(), rule_document_file()),
            ("ods".to_string(), rule_spreadsheet_file()),
            ("xlsx".to_string(), rule_spreadsheet_file()),
            ("xlsm".to_string(), rule_spreadsheet_file()),
            ("odp".to_string(), rule_presentation_file()),
            ("pptx".to_string(), rule_presentation_file()),
            ("pptm".to_string(), rule_presentation_file()),
            ("pages".to_string(), rule_document_file()),
            ("png".to_string(), rule_class(FileClass::Image)),
            ("jpg".to_string(), rule_class(FileClass::Image)),
            ("jpeg".to_string(), rule_class(FileClass::Image)),
            ("gif".to_string(), rule_class(FileClass::Image)),
            ("svg".to_string(), rule_class(FileClass::Image)),
            ("webp".to_string(), rule_class(FileClass::Image)),
            ("avif".to_string(), rule_class(FileClass::Image)),
            ("xcf".to_string(), rule_class(FileClass::Image)),
            ("ico".to_string(), rule_class(FileClass::Image)),
            ("mp3".to_string(), rule_class(FileClass::Audio)),
            ("wav".to_string(), rule_class(FileClass::Audio)),
            ("flac".to_string(), rule_class(FileClass::Audio)),
            ("ogg".to_string(), rule_class(FileClass::Audio)),
            ("m4a".to_string(), rule_class(FileClass::Audio)),
            ("mp4".to_string(), rule_class(FileClass::Video)),
            ("mkv".to_string(), rule_class(FileClass::Video)),
            ("mov".to_string(), rule_class(FileClass::Video)),
            ("webm".to_string(), rule_class(FileClass::Video)),
            ("avi".to_string(), rule_class(FileClass::Video)),
            ("zip".to_string(), rule_class(FileClass::Archive)),
            ("tar".to_string(), rule_class(FileClass::Archive)),
            ("gz".to_string(), rule_class(FileClass::Archive)),
            ("xz".to_string(), rule_class(FileClass::Archive)),
            ("bz2".to_string(), rule_class(FileClass::Archive)),
            ("7z".to_string(), rule_class(FileClass::Archive)),
            ("iso".to_string(), rule_class(FileClass::Archive)),
            ("rpm".to_string(), rule_class(FileClass::Archive)),
            ("deb".to_string(), rule_class(FileClass::Archive)),
            ("apk".to_string(), rule_class(FileClass::Archive)),
            ("aab".to_string(), rule_class(FileClass::Archive)),
            ("apkg".to_string(), rule_class(FileClass::Archive)),
            ("zst".to_string(), rule_class(FileClass::Archive)),
            ("jar".to_string(), rule_class(FileClass::Archive)),
            ("zest".to_string(), rule_class(FileClass::Archive)),
            ("appimage".to_string(), rule_class(FileClass::Archive)),
            ("ttf".to_string(), rule_class(FileClass::Font)),
            ("otf".to_string(), rule_class(FileClass::Font)),
            ("woff".to_string(), rule_class(FileClass::Font)),
            ("woff2".to_string(), rule_class(FileClass::Font)),
            ("csv".to_string(), rule_class(FileClass::Data)),
            ("tsv".to_string(), rule_class(FileClass::Data)),
            ("sqlite".to_string(), rule_class(FileClass::Data)),
            ("db".to_string(), rule_class(FileClass::Data)),
            ("parquet".to_string(), rule_class(FileClass::Data)),
            ("torrent".to_string(), rule_class(FileClass::Data)),
            ("hash".to_string(), rule_class(FileClass::Data)),
            ("sha1".to_string(), rule_class(FileClass::Data)),
            ("sha256".to_string(), rule_class(FileClass::Data)),
            ("sha512".to_string(), rule_class(FileClass::Data)),
            ("md5".to_string(), rule_class(FileClass::Data)),
            ("log".to_string(), rule_class(FileClass::Document)),
            ("srt".to_string(), rule_class(FileClass::Document)),
            ("keys".to_string(), rule_class(FileClass::Config)),
            ("p12".to_string(), rule_class(FileClass::Config)),
            ("pfx".to_string(), rule_class(FileClass::Config)),
            ("pem".to_string(), rule_class(FileClass::Config)),
            ("crt".to_string(), rule_class(FileClass::Config)),
            ("cer".to_string(), rule_class(FileClass::Config)),
            ("csr".to_string(), rule_class(FileClass::Config)),
            ("key".to_string(), rule_class(FileClass::Config)),
            ("exe".to_string(), rule_class(FileClass::File)),
        ]);

        let files = HashMap::from([
            (
                normalize_key("Cargo.lock"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: None,
                },
            ),
            (
                normalize_key("package.json"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(226, 180, 80)),
                },
            ),
            (
                normalize_key("package-lock.json"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(210, 146, 89)),
                },
            ),
            (
                normalize_key("pnpm-lock.yaml"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(255, 184, 107)),
                },
            ),
            (
                normalize_key("yarn.lock"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(36, 217, 184)),
                },
            ),
            (
                normalize_key("bun.lock"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(247, 200, 94)),
                },
            ),
            (
                normalize_key("bun.lockb"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(247, 200, 94)),
                },
            ),
            (
                normalize_key("poetry.lock"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(141, 223, 109)),
                },
            ),
            (
                normalize_key("Pipfile.lock"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(89, 222, 148)),
                },
            ),
            (
                normalize_key("uv.lock"),
                RuleOverride {
                    class: Some(FileClass::Data),
                    icon: Some("󰈡".to_string()),
                    color: Some(rgb(89, 222, 148)),
                },
            ),
            (
                normalize_key("Dockerfile"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("󰡨".to_string()),
                    color: Some(rgb(94, 162, 227)),
                },
            ),
            (
                normalize_key("Containerfile"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("󰡨".to_string()),
                    color: Some(rgb(94, 162, 227)),
                },
            ),
            (
                normalize_key("compose.yml"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("󰡨".to_string()),
                    color: Some(rgb(94, 162, 227)),
                },
            ),
            (
                normalize_key("compose.yaml"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("󰡨".to_string()),
                    color: Some(rgb(94, 162, 227)),
                },
            ),
            (
                normalize_key(".terraform.lock.hcl"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(179, 140, 255)),
                },
            ),
            (
                normalize_key("build.gradle"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(112, 182, 117)),
                },
            ),
            (
                normalize_key("settings.gradle"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(112, 182, 117)),
                },
            ),
            (
                normalize_key("init.gradle"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(112, 182, 117)),
                },
            ),
            (
                normalize_key("build.sbt"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(232, 90, 90)),
                },
            ),
            (
                normalize_key(".rprofile"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("󰟔".to_string()),
                    color: Some(rgb(95, 153, 219)),
                },
            ),
            (
                normalize_key("project.clj"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(128, 176, 92)),
                },
            ),
            (
                normalize_key("deps.edn"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(128, 176, 92)),
                },
            ),
            (
                normalize_key("bb.edn"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(128, 176, 92)),
                },
            ),
            (
                normalize_key("shadow-cljs.edn"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(128, 176, 92)),
                },
            ),
            (
                normalize_key("Justfile"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(255, 184, 107)),
                },
            ),
            (
                normalize_key(".justfile"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(255, 184, 107)),
                },
            ),
            (
                normalize_key("build.zig.zon"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(245, 173, 64)),
                },
            ),
            (
                normalize_key("README.md"),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("".to_string()),
                    color: Some(rgb(211, 170, 124)),
                },
            ),
            (
                normalize_key("AUTHORS"),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("󰭘".to_string()),
                    color: Some(rgb(155, 143, 199)),
                },
            ),
            (
                normalize_key("AUTHORS.md"),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("󰭘".to_string()),
                    color: Some(rgb(155, 143, 199)),
                },
            ),
            (
                normalize_key("AUTHORS.txt"),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("󰭘".to_string()),
                    color: Some(rgb(155, 143, 199)),
                },
            ),
            (
                normalize_key("CONTRIBUTORS"),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("󰭘".to_string()),
                    color: Some(rgb(155, 143, 199)),
                },
            ),
            (
                normalize_key("CONTRIBUTORS.md"),
                RuleOverride {
                    class: Some(FileClass::Document),
                    icon: Some("󰭘".to_string()),
                    color: Some(rgb(155, 143, 199)),
                },
            ),
            (
                normalize_key(".gitignore"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("󰊢".to_string()),
                    color: Some(rgb(232, 153, 88)),
                },
            ),
            (
                normalize_key(".env"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("󰒓".to_string()),
                    color: Some(rgb(144, 192, 121)),
                },
            ),
            (
                normalize_key("PKGBUILD"),
                RuleOverride {
                    class: Some(FileClass::Config),
                    icon: Some("".to_string()),
                    color: Some(rgb(102, 187, 255)),
                },
            ),
        ]);

        Self {
            palette: Palette {
                bg: rgb(2, 5, 12),
                chrome: rgb(7, 13, 22),
                chrome_alt: rgb(11, 18, 32),
                panel: rgb(9, 16, 27),
                panel_alt: rgb(6, 11, 20),
                surface: rgb(16, 25, 42),
                elevated: rgb(21, 32, 54),
                border: rgb(53, 80, 111),
                text: rgb(237, 244, 255),
                muted: rgb(142, 162, 191),
                accent: rgb(126, 196, 255),
                accent_soft: rgb(20, 54, 87),
                accent_text: rgb(234, 245, 255),
                selected_bg: rgb(32, 64, 100),
                selected_border: rgb(149, 211, 255),
                selection_bar: rgb(255, 178, 86),
                sidebar_active: rgb(27, 56, 88),
                button_bg: rgb(14, 23, 38),
                button_disabled_bg: rgb(8, 16, 27),
                path_bg: rgb(12, 19, 32),
            },
            preview: PreviewTheme {
                code: CodePreviewPalette {
                    fg: rgb(215, 227, 244),
                    bg: rgb(10, 13, 18),
                    selection_bg: rgb(18, 42, 63),
                    selection_fg: rgb(242, 247, 255),
                    caret: rgb(18, 210, 255),
                    line_highlight: rgb(16, 21, 31),
                    line_number: rgb(123, 144, 167),
                    comment: rgb(111, 131, 153),
                    string: rgb(121, 231, 213),
                    constant: rgb(255, 166, 87),
                    keyword: rgb(255, 120, 198),
                    function: rgb(54, 215, 255),
                    r#type: rgb(179, 140, 255),
                    parameter: rgb(255, 216, 102),
                    tag: rgb(89, 222, 148),
                    operator: rgb(138, 231, 255),
                    r#macro: rgb(255, 143, 64),
                    invalid: rgb(255, 133, 133),
                },
            },
            classes,
            extensions,
            files,
            directories: HashMap::new(),
        }
    }
}

pub(super) fn default_class_style(class: FileClass) -> ClassStyle {
    match class {
        FileClass::Directory => ClassStyle {
            icon: "󰉋".to_string(),
            color: rgb(65, 143, 222),
        },
        FileClass::Code => ClassStyle {
            icon: "󰆍".to_string(),
            color: rgb(87, 196, 155),
        },
        FileClass::Config => ClassStyle {
            icon: "󰒓".to_string(),
            color: rgb(121, 188, 255),
        },
        FileClass::Document => ClassStyle {
            icon: "󰈙".to_string(),
            color: rgb(112, 182, 117),
        },
        FileClass::License => ClassStyle {
            icon: "󰿃".to_string(),
            color: rgb(245, 216, 91),
        },
        FileClass::Image => ClassStyle {
            icon: "󰋩".to_string(),
            color: rgb(86, 156, 214),
        },
        FileClass::Audio => ClassStyle {
            icon: "󰎆".to_string(),
            color: rgb(138, 110, 214),
        },
        FileClass::Video => ClassStyle {
            icon: "".to_string(),
            color: rgb(204, 112, 79),
        },
        FileClass::Archive => ClassStyle {
            icon: "󰗄".to_string(),
            color: rgb(207, 111, 63),
        },
        FileClass::Font => ClassStyle {
            icon: "󰛖".to_string(),
            color: rgb(196, 148, 92),
        },
        FileClass::Data => ClassStyle {
            icon: "󰆼".to_string(),
            color: rgb(92, 192, 201),
        },
        FileClass::File => ClassStyle {
            icon: "󰈔".to_string(),
            color: rgb(98, 109, 122),
        },
    }
}

pub(super) fn rule_class(class: FileClass) -> RuleOverride {
    RuleOverride {
        class: Some(class),
        ..RuleOverride::default()
    }
}

pub(super) fn rule_document_file() -> RuleOverride {
    RuleOverride {
        class: Some(FileClass::Document),
        icon: Some("󰈬".to_string()),
        color: Some(rgb(88, 142, 255)),
    }
}

pub(super) fn rule_spreadsheet_file() -> RuleOverride {
    RuleOverride {
        class: Some(FileClass::Document),
        icon: Some("󱎏".to_string()),
        color: Some(rgb(78, 178, 116)),
    }
}

pub(super) fn rule_presentation_file() -> RuleOverride {
    RuleOverride {
        class: Some(FileClass::Document),
        icon: Some("󱎐".to_string()),
        color: Some(rgb(232, 139, 63)),
    }
}

pub(super) fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub(super) fn rgb(red: u8, green: u8, blue: u8) -> Color {
    Color::Rgb(red, green, blue)
}
