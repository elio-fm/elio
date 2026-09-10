use anyhow::Result;
use std::path::Path;

pub(super) fn write_if_requested(cwd_file: Option<&Path>, final_cwd: &Path) -> Result<()> {
    let Some(cwd_file) = cwd_file else {
        return Ok(());
    };

    write(cwd_file, final_cwd)
}

#[cfg(unix)]
fn write(cwd_file: &Path, final_cwd: &Path) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;

    std::fs::write(cwd_file, final_cwd.as_os_str().as_bytes())?;
    Ok(())
}

#[cfg(not(unix))]
fn write(cwd_file: &Path, final_cwd: &Path) -> Result<()> {
    std::fs::write(cwd_file, final_cwd.to_string_lossy().as_bytes())?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/cd_on_exit.rs"]
mod tests;
