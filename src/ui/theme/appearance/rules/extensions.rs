use super::super::types::RuleOverride;
use super::shared::{
    rgb, rule_class, rule_document_file, rule_presentation_file, rule_spreadsheet_file,
};
use crate::core::FileClass;
use std::collections::HashMap;

pub(super) fn default_extension_rules() -> HashMap<String, RuleOverride> {
    HashMap::from([
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
        (
            "cbr".to_string(),
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
        ("sqlite3".to_string(), rule_class(FileClass::Data)),
        ("db3".to_string(), rule_class(FileClass::Data)),
        ("db".to_string(), rule_class(FileClass::Data)),
        ("sqlite-wal".to_string(), rule_class(FileClass::Data)),
        ("sqlite-shm".to_string(), rule_class(FileClass::Data)),
        ("sqlite-journal".to_string(), rule_class(FileClass::Data)),
        ("db-wal".to_string(), rule_class(FileClass::Data)),
        ("db-shm".to_string(), rule_class(FileClass::Data)),
        ("db-journal".to_string(), rule_class(FileClass::Data)),
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
    ])
}
