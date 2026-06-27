use super::format::{ExtractBackend, ExtractFormat, unique_destination};
use anyhow::{Context, Result, anyhow, bail};
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use sevenz_rust2::{ArchiveEntry as SevenZipEntry, ArchiveReader, Password};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};
use xz2::read::XzDecoder;
use zstd::stream::read::Decoder as ZstdDecoder;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExtractPlan {
    pub(crate) archive_path: PathBuf,
    pub(crate) dest_dir: PathBuf,
    pub(crate) backend: ExtractBackend,
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
    let format =
        ExtractFormat::detect(path).ok_or_else(|| anyhow!(ExtractFormat::SUPPORTED_MESSAGE))?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Cannot determine archive parent directory"))?;
    let stem = ExtractFormat::stem_for_destination(path)
        .ok_or_else(|| anyhow!("Cannot determine extraction folder name"))?;
    Ok(ExtractPlan {
        archive_path: path.to_path_buf(),
        dest_dir: unique_destination(parent, &stem),
        backend: format.backend(),
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
    match plan.backend {
        ExtractBackend::Zip => extract_zip(plan, &mut progress, cancelled),
        ExtractBackend::Tar(format) => extract_tar(format, plan, &mut progress, cancelled),
        ExtractBackend::SevenZip => extract_seven_zip(plan, &mut progress, cancelled),
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

fn extract_tar<F, C>(
    format: ExtractFormat,
    plan: &ExtractPlan,
    progress: &mut F,
    cancelled: C,
) -> Result<ExtractSummary>
where
    F: FnMut(ExtractProgress),
    C: FnMut() -> bool,
{
    match format {
        ExtractFormat::Tar => extract_tar_with(format, plan, Ok, progress, cancelled),
        ExtractFormat::TarGzip => extract_tar_with(
            format,
            plan,
            |file| Ok(GzDecoder::new(file)),
            progress,
            cancelled,
        ),
        ExtractFormat::TarXz => extract_tar_with(
            format,
            plan,
            |file| Ok(XzDecoder::new(file)),
            progress,
            cancelled,
        ),
        ExtractFormat::TarBzip2 => extract_tar_with(
            format,
            plan,
            |file| Ok(BzDecoder::new(file)),
            progress,
            cancelled,
        ),
        ExtractFormat::TarZstd => {
            extract_tar_with(format, plan, ZstdDecoder::new, progress, cancelled)
        }
        ExtractFormat::Zip | ExtractFormat::SevenZip => {
            unreachable!("non-TAR archives use their own native backends")
        }
    }
}

fn extract_seven_zip<F, C>(
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
    let mut archive =
        ArchiveReader::new(file, Password::empty()).context("Could not read 7z archive")?;
    let total = archive.archive().files.len();
    let mut completed = 0usize;
    let mut extract_error = None;

    progress(ExtractProgress {
        completed,
        total: Some(total),
    });
    archive
        .for_each_entries(|entry, reader| {
            if cancelled() {
                return Ok(false);
            }
            if let Err(error) = extract_seven_zip_entry(plan, entry, reader) {
                extract_error = Some(error);
                return Ok(false);
            }
            completed += 1;
            progress(ExtractProgress {
                completed,
                total: Some(total),
            });
            Ok(true)
        })
        .context("Could not read 7z entries")?;

    if let Some(error) = extract_error {
        return Err(error);
    }
    Ok(ExtractSummary {
        dest_dir: plan.dest_dir.clone(),
        completed,
        total: Some(total),
    })
}

fn extract_seven_zip_entry(
    plan: &ExtractPlan,
    entry: &SevenZipEntry,
    reader: &mut dyn Read,
) -> Result<()> {
    if entry.is_anti_item {
        return Ok(());
    }
    let out_path = checked_output_name(&plan.dest_dir, &entry.name)?;
    if entry.is_directory {
        fs::create_dir_all(&out_path)
            .with_context(|| format!("Could not create {}", out_path.display()))?;
    } else {
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Could not create {}", parent.display()))?;
        }
        let mut out = File::create(&out_path)
            .with_context(|| format!("Could not create {}", out_path.display()))?;
        let mut limited = reader.take(entry.size);
        io::copy(&mut limited, &mut out)
            .with_context(|| format!("Could not write {}", out_path.display()))?;
    }
    Ok(())
}

fn extract_tar_with<R, D, F, C>(
    format: ExtractFormat,
    plan: &ExtractPlan,
    decoder: D,
    progress: &mut F,
    cancelled: C,
) -> Result<ExtractSummary>
where
    R: Read,
    D: Fn(File) -> Result<R, std::io::Error>,
    F: FnMut(ExtractProgress),
    C: FnMut() -> bool,
{
    let count_file = File::open(&plan.archive_path)
        .with_context(|| format!("Could not open {}", plan.archive_path.display()))?;
    let total = count_tar_entries(decoder(count_file)?)
        .with_context(|| format!("Could not read {} archive", format.label()))?;
    let file = File::open(&plan.archive_path)
        .with_context(|| format!("Could not open {}", plan.archive_path.display()))?;
    extract_tar_reader(plan, decoder(file)?, Some(total), progress, cancelled)
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

fn checked_output_name(dest_dir: &Path, entry_name: &str) -> Result<PathBuf> {
    let normalized = entry_name.replace('\\', "/");
    checked_output_path(dest_dir, Path::new(&normalized))
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

    #[test]
    fn extracts_tar_xz_archive() {
        let root = temp_path("txz");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.tar.xz");
        write_compressed_tar(&archive_path, |file| {
            Ok(xz2::write::XzEncoder::new(file, 6))
        });

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
    fn extracts_tar_bzip2_archive() {
        let root = temp_path("tbz2");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.tar.bz2");
        write_compressed_tar(&archive_path, |file| {
            Ok(bzip2::write::BzEncoder::new(
                file,
                bzip2::Compression::default(),
            ))
        });

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
    fn extracts_tar_zstd_archive() {
        let root = temp_path("tzst");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.tar.zst");
        write_zstd_tar(&archive_path);

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
    fn extracts_seven_zip_archive() {
        let root = temp_path("7z");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.7z");
        write_seven_zip(
            &archive_path,
            &[("dir", None), ("dir/file.txt", Some(b"hello"))],
        );

        let plan = plan_extract(&archive_path).unwrap();
        let mut progress = Vec::new();
        let summary = extract_archive(&plan, |update| progress.push(update), || false).unwrap();

        assert_eq!(summary.completed, 2);
        assert_eq!(summary.total, Some(2));
        assert_eq!(
            progress
                .last()
                .map(|update| (update.completed, update.total)),
            Some((2, Some(2)))
        );
        assert_eq!(
            fs::read_to_string(root.join("sample/dir/file.txt")).unwrap(),
            "hello"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_escaping_seven_zip_paths() {
        let root = temp_path("7z-slip");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("sample.7z");
        write_seven_zip(&archive_path, &[("../evil.txt", Some(b"bad"))]);

        let plan = plan_extract(&archive_path).unwrap();
        let err = extract_archive(&plan, |_| {}, || false).unwrap_err();

        assert!(err.to_string().contains("escapes the destination"));
        assert!(!root.join("evil.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    fn write_compressed_tar<W, D>(archive_path: &Path, encoder: D)
    where
        W: Write,
        D: FnOnce(File) -> std::result::Result<W, std::io::Error>,
    {
        let root = archive_path.parent().unwrap();
        let source = root.join("source");
        fs::create_dir_all(source.join("dir")).unwrap();
        fs::write(source.join("dir/file.txt"), "hello").unwrap();

        let file = File::create(archive_path).unwrap();
        let writer = encoder(file).unwrap();
        let mut tar = tar::Builder::new(writer);
        tar.append_dir("dir", source.join("dir")).unwrap();
        tar.append_path_with_name(source.join("dir/file.txt"), "dir/file.txt")
            .unwrap();
        tar.finish().unwrap();
    }

    fn write_zstd_tar(archive_path: &Path) {
        let root = archive_path.parent().unwrap();
        let source = root.join("source");
        fs::create_dir_all(source.join("dir")).unwrap();
        fs::write(source.join("dir/file.txt"), "hello").unwrap();

        let mut tar_bytes = Vec::new();
        {
            let mut tar = tar::Builder::new(&mut tar_bytes);
            tar.append_dir("dir", source.join("dir")).unwrap();
            tar.append_path_with_name(source.join("dir/file.txt"), "dir/file.txt")
                .unwrap();
            tar.finish().unwrap();
        }
        let compressed = zstd::stream::encode_all(tar_bytes.as_slice(), 0).unwrap();
        fs::write(archive_path, compressed).unwrap();
    }

    fn write_seven_zip(archive_path: &Path, entries: &[(&str, Option<&[u8]>)]) {
        let mut writer = sevenz_rust2::ArchiveWriter::create(archive_path).unwrap();
        for (name, contents) in entries {
            let entry = if contents.is_some() {
                sevenz_rust2::ArchiveEntry::new_file(name)
            } else {
                sevenz_rust2::ArchiveEntry::new_directory(name)
            };
            match contents {
                Some(contents) => {
                    writer.push_archive_entry(entry, Some(*contents)).unwrap();
                }
                None => {
                    writer.push_archive_entry::<&[u8]>(entry, None).unwrap();
                }
            }
        }
        writer.finish().unwrap();
    }
}
