use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExtractFormat {
    Zip,
    Tar,
    TarGzip,
}

impl ExtractFormat {
    pub(crate) fn detect(path: &Path) -> Option<Self> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())?
            .to_ascii_lowercase();
        if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            return Some(Self::TarGzip);
        }
        match path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("zip") => Some(Self::Zip),
            Some("tar") => Some(Self::Tar),
            _ => None,
        }
    }

    pub(crate) fn stem_for_destination(path: &Path) -> Option<String> {
        let name = path.file_name()?.to_string_lossy();
        let lower = name.to_ascii_lowercase();
        let stem = if lower.ends_with(".tar.gz") {
            &name[..name.len().saturating_sub(7)]
        } else if lower.ends_with(".tgz") {
            &name[..name.len().saturating_sub(4)]
        } else if lower.ends_with(".zip") || lower.ends_with(".tar") {
            let cut = name.rfind('.')?;
            &name[..cut]
        } else {
            return None;
        };
        let trimmed = stem.trim();
        Some(if trimmed.is_empty() {
            "archive".to_string()
        } else {
            trimmed.to_string()
        })
    }
}

pub(crate) fn unique_destination(parent: &Path, stem: &str) -> PathBuf {
    let first = parent.join(stem);
    if std::fs::symlink_metadata(&first).is_err() {
        return first;
    }
    for index in 1u32.. {
        let candidate = parent.join(format!("{stem}_{index}"));
        if std::fs::symlink_metadata(&candidate).is_err() {
            return candidate;
        }
    }
    unreachable!("unique destination search should not overflow")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("elio-archive-format-{label}-{unique}"))
    }

    #[test]
    fn detects_v1_formats() {
        assert_eq!(
            ExtractFormat::detect(Path::new("app.zip")),
            Some(ExtractFormat::Zip)
        );
        assert_eq!(
            ExtractFormat::detect(Path::new("app.tar")),
            Some(ExtractFormat::Tar)
        );
        assert_eq!(
            ExtractFormat::detect(Path::new("app.tar.gz")),
            Some(ExtractFormat::TarGzip)
        );
        assert_eq!(
            ExtractFormat::detect(Path::new("app.tgz")),
            Some(ExtractFormat::TarGzip)
        );
        assert_eq!(ExtractFormat::detect(Path::new("app.tar.xz")), None);
    }

    #[test]
    fn derives_destination_stems() {
        assert_eq!(
            ExtractFormat::stem_for_destination(Path::new("app.zip")).as_deref(),
            Some("app")
        );
        assert_eq!(
            ExtractFormat::stem_for_destination(Path::new("app.tar")).as_deref(),
            Some("app")
        );
        assert_eq!(
            ExtractFormat::stem_for_destination(Path::new("app.tar.gz")).as_deref(),
            Some("app")
        );
        assert_eq!(
            ExtractFormat::stem_for_destination(Path::new("app.tgz")).as_deref(),
            Some("app")
        );
    }

    #[test]
    fn unique_destination_uses_paste_style_suffix() {
        let root = temp_path("unique");
        fs::create_dir_all(root.join("app")).unwrap();
        fs::create_dir_all(root.join("app_1")).unwrap();
        assert_eq!(unique_destination(&root, "app"), root.join("app_2"));
        let _ = fs::remove_dir_all(root);
    }
}
