use anyhow::Result;
use std::{
    fs as std_fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

pub(super) fn write_cwd_file_if_requested(cwd_file: Option<&Path>, final_cwd: &Path) -> Result<()> {
    let Some(cwd_file) = cwd_file else {
        return Ok(());
    };

    write_cwd_file(cwd_file, final_cwd)
}

#[cfg(unix)]
fn write_cwd_file(cwd_file: &Path, final_cwd: &Path) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;

    std_fs::write(cwd_file, final_cwd.as_os_str().as_bytes())?;
    Ok(())
}

#[cfg(not(unix))]
fn write_cwd_file(cwd_file: &Path, final_cwd: &Path) -> Result<()> {
    std_fs::write(cwd_file, final_cwd.to_string_lossy().as_bytes())?;
    Ok(())
}

pub(super) fn write_chooser_file_if_requested(
    chooser_file: Option<&Path>,
    paths: &[PathBuf],
) -> Result<()> {
    let Some(chooser_file) = chooser_file else {
        return Ok(());
    };

    write_chooser_file(chooser_file, paths)
}

fn write_chooser_file(chooser_file: &Path, paths: &[PathBuf]) -> Result<()> {
    let bytes = chooser_output_bytes(paths);
    if chooser_file_is_stdout(chooser_file) {
        let mut stdout = io::stdout().lock();
        stdout.write_all(&bytes)?;
        stdout.flush()?;
    } else {
        std_fs::write(chooser_file, bytes)?;
    }
    Ok(())
}

fn chooser_file_is_stdout(chooser_file: &Path) -> bool {
    chooser_file == Path::new("-")
}

#[cfg(unix)]
fn chooser_output_bytes(paths: &[PathBuf]) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    let mut bytes = Vec::new();
    for path in paths {
        bytes.extend_from_slice(path.as_os_str().as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

#[cfg(not(unix))]
fn chooser_output_bytes(paths: &[PathBuf]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for path in paths {
        bytes.extend_from_slice(path.to_string_lossy().as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::{
        chooser_file_is_stdout, write_chooser_file_if_requested, write_cwd_file_if_requested,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-lib-{label}-{unique}"))
    }

    #[test]
    fn cwd_file_is_not_written_when_absent() {
        write_cwd_file_if_requested(None, Path::new("/tmp"))
            .expect("absent cwd file should be a no-op");
    }

    #[test]
    fn cwd_file_writes_path_without_trailing_newline() {
        let root = temp_path("cwd-file");
        fs::create_dir_all(&root).expect("temp directory should be created");
        let cwd_file = root.join("cwd");
        let final_cwd = root.join("nested");
        fs::create_dir_all(&final_cwd).expect("nested temp directory should be created");

        write_cwd_file_if_requested(Some(&cwd_file), &final_cwd)
            .expect("cwd file should be written");

        let bytes = fs::read(&cwd_file).expect("cwd file should be readable");
        assert!(!bytes.ends_with(b"\n"));
        assert_eq!(String::from_utf8_lossy(&bytes), final_cwd.to_string_lossy());

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn chooser_file_is_not_written_when_absent() {
        write_chooser_file_if_requested(None, &[PathBuf::from("/tmp/example")])
            .expect("absent chooser file should be a no-op");
    }

    #[test]
    fn chooser_file_hyphen_targets_stdout() {
        assert!(chooser_file_is_stdout(Path::new("-")));
        assert!(!chooser_file_is_stdout(Path::new("./-")));
        assert!(!chooser_file_is_stdout(Path::new("/dev/stdout")));
    }

    #[test]
    fn chooser_file_writes_paths_with_trailing_newline() {
        let root = temp_path("chooser-file");
        fs::create_dir_all(&root).expect("temp directory should be created");
        let chooser_file = root.join("selection");
        let alpha = root.join("alpha.txt");
        let beta = root.join("beta.txt");

        write_chooser_file_if_requested(Some(&chooser_file), &[alpha.clone(), beta.clone()])
            .expect("chooser file should be written");

        let bytes = fs::read(&chooser_file).expect("chooser file should be readable");
        let expected = format!("{}\n{}\n", alpha.to_string_lossy(), beta.to_string_lossy());
        assert_eq!(String::from_utf8_lossy(&bytes), expected);

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }

    #[test]
    fn chooser_file_truncates_on_empty_confirmation() {
        let root = temp_path("chooser-empty");
        fs::create_dir_all(&root).expect("temp directory should be created");
        let chooser_file = root.join("selection");
        fs::write(&chooser_file, "stale\n").expect("chooser file should be primed");

        write_chooser_file_if_requested(Some(&chooser_file), &[])
            .expect("empty chooser confirmation should be written");

        let bytes = fs::read(&chooser_file).expect("chooser file should be readable");
        assert!(bytes.is_empty());

        fs::remove_dir_all(root).expect("temp directory should be removed");
    }
}
