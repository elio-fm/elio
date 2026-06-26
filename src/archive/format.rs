use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExtractFormat {
    Zip,
    Tar,
    TarGzip,
    TarXz,
    TarBzip2,
    TarZstd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExtractBackend {
    NativeZip,
    NativeTar(ExtractFormat),
}

impl ExtractFormat {
    pub(crate) const SUPPORTED_MESSAGE: &'static str =
        "Extraction supports ZIP, TAR, TAR.GZ, TAR.XZ, TAR.BZ2, and TAR.ZST";

    pub(crate) fn detect(path: &Path) -> Option<Self> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())?
            .to_ascii_lowercase();
        if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            return Some(Self::TarGzip);
        }
        if name.ends_with(".tar.xz") || name.ends_with(".txz") {
            return Some(Self::TarXz);
        }
        if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") || name.ends_with(".tbz") {
            return Some(Self::TarBzip2);
        }
        if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
            return Some(Self::TarZstd);
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
        let stem = [
            ".tar.bz2", ".tar.zst", ".tar.gz", ".tar.xz", ".tbz2", ".tzst", ".tgz", ".txz", ".tbz",
            ".zip", ".tar",
        ]
        .iter()
        .find_map(|suffix| {
            lower
                .ends_with(suffix)
                .then(|| &name[..name.len() - suffix.len()])
        })?;
        let trimmed = stem.trim();
        Some(if trimmed.is_empty() {
            "archive".to_string()
        } else {
            trimmed.to_string()
        })
    }

    pub(crate) fn backend(self) -> ExtractBackend {
        match self {
            Self::Zip => ExtractBackend::NativeZip,
            Self::Tar | Self::TarGzip | Self::TarXz | Self::TarBzip2 | Self::TarZstd => {
                ExtractBackend::NativeTar(self)
            }
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Zip => "ZIP",
            Self::Tar => "TAR",
            Self::TarGzip => "TAR.GZ",
            Self::TarXz => "TAR.XZ",
            Self::TarBzip2 => "TAR.BZ2",
            Self::TarZstd => "TAR.ZST",
        }
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
        assert_eq!(
            ExtractFormat::detect(Path::new("app.tar.xz")),
            Some(ExtractFormat::TarXz)
        );
        assert_eq!(
            ExtractFormat::detect(Path::new("app.txz")),
            Some(ExtractFormat::TarXz)
        );
        assert_eq!(
            ExtractFormat::detect(Path::new("app.tar.bz2")),
            Some(ExtractFormat::TarBzip2)
        );
        assert_eq!(
            ExtractFormat::detect(Path::new("app.tbz2")),
            Some(ExtractFormat::TarBzip2)
        );
        assert_eq!(
            ExtractFormat::detect(Path::new("app.tbz")),
            Some(ExtractFormat::TarBzip2)
        );
        assert_eq!(
            ExtractFormat::detect(Path::new("app.tar.zst")),
            Some(ExtractFormat::TarZstd)
        );
        assert_eq!(
            ExtractFormat::detect(Path::new("app.tzst")),
            Some(ExtractFormat::TarZstd)
        );
    }

    #[test]
    fn maps_formats_to_native_backends() {
        assert_eq!(ExtractFormat::Zip.backend(), ExtractBackend::NativeZip);
        assert_eq!(
            ExtractFormat::Tar.backend(),
            ExtractBackend::NativeTar(ExtractFormat::Tar)
        );
        assert_eq!(
            ExtractFormat::TarGzip.backend(),
            ExtractBackend::NativeTar(ExtractFormat::TarGzip)
        );
        assert_eq!(
            ExtractFormat::TarXz.backend(),
            ExtractBackend::NativeTar(ExtractFormat::TarXz)
        );
        assert_eq!(
            ExtractFormat::TarBzip2.backend(),
            ExtractBackend::NativeTar(ExtractFormat::TarBzip2)
        );
        assert_eq!(
            ExtractFormat::TarZstd.backend(),
            ExtractBackend::NativeTar(ExtractFormat::TarZstd)
        );
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
        assert_eq!(
            ExtractFormat::stem_for_destination(Path::new("app.tar.xz")).as_deref(),
            Some("app")
        );
        assert_eq!(
            ExtractFormat::stem_for_destination(Path::new("app.txz")).as_deref(),
            Some("app")
        );
        assert_eq!(
            ExtractFormat::stem_for_destination(Path::new("app.tar.bz2")).as_deref(),
            Some("app")
        );
        assert_eq!(
            ExtractFormat::stem_for_destination(Path::new("app.tbz2")).as_deref(),
            Some("app")
        );
        assert_eq!(
            ExtractFormat::stem_for_destination(Path::new("app.tbz")).as_deref(),
            Some("app")
        );
        assert_eq!(
            ExtractFormat::stem_for_destination(Path::new("app.tar.zst")).as_deref(),
            Some("app")
        );
        assert_eq!(
            ExtractFormat::stem_for_destination(Path::new("app.tzst")).as_deref(),
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
