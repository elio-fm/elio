use super::format::{ExtractFormat, unique_destination};
use anyhow::{Context, Result, anyhow, bail};
use flate2::read::GzDecoder;
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExtractPlan {
    pub(crate) archive_path: PathBuf,
    pub(crate) dest_dir: PathBuf,
    pub(crate) format: ExtractFormat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExtractProgress {
    pub(crate) completed: usize,
    pub(crate) total: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExtractSummary {
    pub(crate) dest_dir: PathBuf,
    pub(crate) completed: usize,
    pub(crate) total: Option<usize>,
}

pub(crate) fn plan_extract(path: &Path) -> Result<ExtractPlan> {
    let format = ExtractFormat::detect(path)
        .ok_or_else(|| anyhow!("Extraction supports ZIP, TAR, and TAR.GZ"))?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Cannot determine archive parent directory"))?;
    let stem = ExtractFormat::stem_for_destination(path)
        .ok_or_else(|| anyhow!("Cannot determine extraction folder name"))?;
    Ok(ExtractPlan {
        archive_path: path.to_path_buf(),
        dest_dir: unique_destination(parent, &stem),
        format,
    })
}

pub(crate) fn extract_archive<F, C>(
    plan: &ExtractPlan,
    mut progress: F,
    cancelled: C,
) -> Result<ExtractSummary>
where
    F: FnMut(ExtractProgress),
    C: FnMut() -> bool,
{
    fs::create_dir(&plan.dest_dir)
        .with_context(|| format!("Could not create {}", plan.dest_dir.display()))?;
    match plan.format {
        ExtractFormat::Zip => extract_zip(plan, &mut progress, cancelled),
        ExtractFormat::Tar => extract_tar(plan, &mut progress, cancelled),
        ExtractFormat::TarGzip => extract_tar_gzip(plan, &mut progress, cancelled),
    }
}

fn extract_zip<F, C>(
    plan: &ExtractPlan,
    progress: &mut F,
    mut cancelled: C,
) -> Result<ExtractSummary>
where
    F: FnMut(ExtractProgress),
    C: FnMut() -> bool,
{
    let file = File::open(&plan.archive_path)
        .with_context(|| format!("Could not open {}", plan.archive_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("Could not read ZIP archive")?;
    let total = archive.len();
    let mut completed = 0usize;
    progress(ExtractProgress {
        completed,
        total: Some(total),
    });

    for index in 0..total {
        if cancelled() {
            break;
        }
        let mut entry = archive
            .by_index(index)
            .context("Could not read ZIP entry")?;
        let Some(enclosed) = entry.enclosed_name() else {
            bail!("Archive entry escapes the destination: {}", entry.name());
        };
        let out_path = checked_output_path(&plan.dest_dir, enclosed.as_ref())?;
        if entry.is_dir() {
            fs::create_dir_all(&out_path)
                .with_context(|| format!("Could not create {}", out_path.display()))?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Could not create {}", parent.display()))?;
            }
            let mut out = File::create(&out_path)
                .with_context(|| format!("Could not create {}", out_path.display()))?;
            io::copy(&mut entry, &mut out)
                .with_context(|| format!("Could not write {}", out_path.display()))?;
            #[cfg(unix)]
            if let Some(mode) = entry.unix_mode() {
                use std::os::unix::fs::PermissionsExt;
                let safe_mode = mode & 0o777;
                let _ = fs::set_permissions(&out_path, fs::Permissions::from_mode(safe_mode));
            }
        }
        completed += 1;
        progress(ExtractProgress {
            completed,
            total: Some(total),
        });
    }

    Ok(ExtractSummary {
        dest_dir: plan.dest_dir.clone(),
        completed,
        total: Some(total),
    })
}

fn extract_tar<F, C>(plan: &ExtractPlan, progress: &mut F, cancelled: C) -> Result<ExtractSummary>
where
    F: FnMut(ExtractProgress),
    C: FnMut() -> bool,
{
    let count_file = File::open(&plan.archive_path)
        .with_context(|| format!("Could not open {}", plan.archive_path.display()))?;
    let total = count_tar_entries(count_file).context("Could not read TAR archive")?;
    let file = File::open(&plan.archive_path)
        .with_context(|| format!("Could not open {}", plan.archive_path.display()))?;
    extract_tar_reader(plan, file, Some(total), progress, cancelled)
}

fn extract_tar_gzip<F, C>(
    plan: &ExtractPlan,
    progress: &mut F,
    cancelled: C,
) -> Result<ExtractSummary>
where
    F: FnMut(ExtractProgress),
    C: FnMut() -> bool,
{
    let count_file = File::open(&plan.archive_path)
        .with_context(|| format!("Could not open {}", plan.archive_path.display()))?;
    let total =
        count_tar_entries(GzDecoder::new(count_file)).context("Could not read TAR.GZ archive")?;
    let file = File::open(&plan.archive_path)
        .with_context(|| format!("Could not open {}", plan.archive_path.display()))?;
    extract_tar_reader(plan, GzDecoder::new(file), Some(total), progress, cancelled)
}

fn count_tar_entries<R: Read>(reader: R) -> Result<usize> {
    let mut archive = tar::Archive::new(reader);
    let mut total = 0usize;
    for entry in archive.entries()? {
        entry?;
        total += 1;
    }
    Ok(total)
}

fn extract_tar_reader<R, F, C>(
    plan: &ExtractPlan,
    reader: R,
    total: Option<usize>,
    progress: &mut F,
    mut cancelled: C,
) -> Result<ExtractSummary>
where
    R: Read,
    F: FnMut(ExtractProgress),
    C: FnMut() -> bool,
{
    let mut archive = tar::Archive::new(reader);
    let mut completed = 0usize;
    progress(ExtractProgress { completed, total });
    for entry in archive.entries().context("Could not read TAR entries")? {
        if cancelled() {
            break;
        }
        let mut entry = entry.context("Could not read TAR entry")?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            completed += 1;
            progress(ExtractProgress { completed, total });
            continue;
        }
        let path = entry.path().context("Could not read TAR entry path")?;
        let out_path = checked_output_path(&plan.dest_dir, path.as_ref())?;
        if entry_type.is_dir() {
            fs::create_dir_all(&out_path)
                .with_context(|| format!("Could not create {}", out_path.display()))?;
        } else if entry_type.is_file() {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Could not create {}", parent.display()))?;
            }
            entry
                .unpack(&out_path)
                .with_context(|| format!("Could not extract {}", out_path.display()))?;
        }
        completed += 1;
        progress(ExtractProgress { completed, total });
    }
    Ok(ExtractSummary {
        dest_dir: plan.dest_dir.clone(),
        completed,
        total,
    })
}

fn checked_output_path(dest_dir: &Path, entry_path: &Path) -> Result<PathBuf> {
    let mut relative = PathBuf::new();
    for component in entry_path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!(
                    "Archive entry escapes the destination: {}",
                    entry_path.display()
                );
            }
        }
    }
    if relative.as_os_str().is_empty() {
        bail!("Archive entry has an empty path");
    }
    let out = dest_dir.join(relative);
    if !out.starts_with(dest_dir) {
        bail!(
            "Archive entry escapes the destination: {}",
            entry_path.display()
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("elio-archive-extract-{label}-{unique}"))
    }

    #[test]
    fn rejects_escaping_paths() {
        let dest = Path::new("/tmp/out");
        assert!(checked_output_path(dest, Path::new("../evil")).is_err());
        assert!(checked_output_path(dest, Path::new("/evil")).is_err());
        assert_eq!(
            checked_output_path(dest, Path::new("ok/file.txt")).unwrap(),
            dest.join("ok/file.txt")
        );
    }

    #[test]
    fn extracts_zip_archive() {
        let root = temp_path("zip");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.zip");
        {
            let file = File::create(&archive_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            zip.add_directory("dir/", options).unwrap();
            zip.start_file("dir/file.txt", options).unwrap();
            zip.write_all(b"hello").unwrap();
            zip.finish().unwrap();
        }
        let plan = plan_extract(&archive_path).unwrap();
        let summary = extract_archive(&plan, |_| {}, || false).unwrap();
        assert_eq!(summary.completed, 2);
        assert_eq!(
            fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
            "hello"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn extracts_tar_archive() {
        let root = temp_path("tar");
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source");
        fs::create_dir_all(source.join("dir")).unwrap();
        fs::write(source.join("dir/file.txt"), "hello").unwrap();
        let archive_path = root.join("sample.tar");
        {
            let file = File::create(&archive_path).unwrap();
            let mut tar = tar::Builder::new(file);
            tar.append_dir("dir", source.join("dir")).unwrap();
            tar.append_path_with_name(source.join("dir/file.txt"), "dir/file.txt")
                .unwrap();
            tar.finish().unwrap();
        }
        let plan = plan_extract(&archive_path).unwrap();
        let summary = extract_archive(&plan, |_| {}, || false).unwrap();
        assert_eq!(summary.completed, 2);
        assert_eq!(
            fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
            "hello"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn extracts_tar_gzip_archive() {
        let root = temp_path("tgz");
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source");
        fs::create_dir_all(source.join("dir")).unwrap();
        fs::write(source.join("dir/file.txt"), "hello").unwrap();
        let archive_path = root.join("sample.tar.gz");
        {
            let file = File::create(&archive_path).unwrap();
            let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            let mut tar = tar::Builder::new(enc);
            tar.append_dir("dir", source.join("dir")).unwrap();
            tar.append_path_with_name(source.join("dir/file.txt"), "dir/file.txt")
                .unwrap();
            tar.finish().unwrap();
        }
        let plan = plan_extract(&archive_path).unwrap();
        let summary = extract_archive(&plan, |_| {}, || false).unwrap();
        assert_eq!(summary.completed, 2);
        assert_eq!(
            fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
            "hello"
        );
        let _ = fs::remove_dir_all(root);
    }
}
