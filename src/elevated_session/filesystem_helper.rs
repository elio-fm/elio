#[cfg(unix)]
use super::{InvokingUser, prepare};
#[cfg(unix)]
use std::{
    ffi::OsString,
    io::{self, Read, Write},
    os::unix::ffi::OsStringExt,
    path::PathBuf,
    process::{Command, Stdio},
};

#[cfg(unix)]
const MAX_ITEMS: usize = 16_384;
#[cfg(unix)]
const MAX_PATH_BYTES: usize = 1024 * 1024;
#[cfg(unix)]
const MAX_ERROR_BYTES: usize = 16 * 1024;
#[cfg(any(target_os = "macos", all(test, unix)))]
const MAX_TOTAL_NAME_BYTES: usize = 16 * 1024 * 1024;

#[cfg(unix)]
#[derive(Debug)]
pub(crate) enum Request {
    Trash(Vec<PathBuf>),
    Restore(PathBuf),
    #[cfg(target_os = "macos")]
    RemoveRestoreOrigins(Vec<String>),
}

#[cfg(unix)]
pub(crate) struct Response {
    pub(crate) completed: usize,
    pub(crate) error: Option<String>,
    pub(crate) warning: Option<String>,
}

#[cfg(unix)]
pub(crate) fn run_as_invoking_user(
    user: &InvokingUser,
    request: &Request,
) -> Result<Response, String> {
    validate_request(request).map_err(|error| format!("invalid invoking-user request: {error}"))?;
    let mut command = Command::new(std::env::current_exe().map_err(|error| error.to_string())?);
    command
        .arg("--internal-user-fs-helper")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("HOME", &user.home)
        .env("USER", &user.name)
        .env("LOGNAME", &user.name)
        .env("ELIO_HELPER_UID", user.uid.to_string())
        .env("ELIO_HELPER_GID", user.gid.to_string())
        .env(
            "ELIO_HELPER_GROUPS",
            user.groups
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
    prepare(&mut command, user, Some(&user.home))
        .map_err(|error| format!("could not prepare invoking-user helper: {error}"))?;

    let mut child = command
        .spawn()
        .map_err(|error| format!("could not start invoking-user helper: {error}"))?;
    let write_result = child
        .stdin
        .take()
        .ok_or_else(|| "invoking-user helper has no stdin".to_string())
        .and_then(|mut stdin| {
            write_request(&mut stdin, request).map_err(|error| error.to_string())?;
            stdin.flush().map_err(|error| error.to_string())
        });
    if let Err(error) = write_result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!("could not send invoking-user request: {error}"));
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("could not wait for invoking-user helper: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        return Err(if detail.is_empty() {
            "invoking-user helper failed".to_string()
        } else {
            format!("invoking-user helper failed: {detail}")
        });
    }
    read_response(output.stdout.as_slice())
        .map_err(|error| format!("invalid invoking-user helper response: {error}"))
}

#[cfg(unix)]
pub(crate) fn validate_request(request: &Request) -> io::Result<()> {
    match request {
        Request::Trash(paths) => {
            if paths.len() > MAX_ITEMS {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "too many paths",
                ));
            }
            for path in paths {
                validate_path(path)?;
            }
        }
        Request::Restore(path) => validate_path(path)?,
        #[cfg(target_os = "macos")]
        Request::RemoveRestoreOrigins(names) => validate_restore_origin_names(names)?,
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn write_request(mut writer: impl Write, request: &Request) -> io::Result<()> {
    validate_request(request)?;
    writer.write_all(b"EU\x01")?;
    match request {
        Request::Trash(paths) => {
            writer.write_all(&[1])?;
            write_u32(&mut writer, paths.len())?;
            for path in paths {
                write_path(&mut writer, path)?;
            }
        }
        Request::Restore(path) => {
            writer.write_all(&[2])?;
            write_path(&mut writer, path)?;
        }
        #[cfg(target_os = "macos")]
        Request::RemoveRestoreOrigins(names) => {
            writer.write_all(&[3])?;
            write_u32(&mut writer, names.len())?;
            for name in names {
                write_bytes(&mut writer, name.as_bytes())?;
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn read_request(mut reader: impl Read) -> io::Result<Request> {
    let mut header = [0; 3];
    reader.read_exact(&mut header)?;
    if header != *b"EU\x01" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported helper protocol",
        ));
    }
    let mut opcode = [0];
    reader.read_exact(&mut opcode)?;
    match opcode[0] {
        1 => {
            let count = read_u32(&mut reader)?;
            if count > MAX_ITEMS {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "too many paths"));
            }
            let mut paths = Vec::with_capacity(count);
            for _ in 0..count {
                paths.push(read_path(&mut reader)?);
            }
            Ok(Request::Trash(paths))
        }
        2 => Ok(Request::Restore(read_path(&mut reader)?)),
        #[cfg(target_os = "macos")]
        3 => {
            let count = read_u32(&mut reader)?;
            if count > MAX_ITEMS {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "too many names"));
            }
            let mut names = Vec::with_capacity(count);
            let mut total_bytes = 0usize;
            for _ in 0..count {
                let bytes = read_bytes(&mut reader)?;
                total_bytes = total_bytes.checked_add(bytes.len()).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "names are too large")
                })?;
                if total_bytes > MAX_TOTAL_NAME_BYTES {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "names are too large",
                    ));
                }
                let name = String::from_utf8(bytes)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "name is not UTF-8"))?;
                names.push(name);
            }
            Ok(Request::RemoveRestoreOrigins(names))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unknown request",
        )),
    }
}

#[cfg(unix)]
pub(crate) fn write_response(mut writer: impl Write, response: &Response) -> io::Result<()> {
    if response.error.is_some() && response.warning.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "response cannot contain both an error and a warning",
        ));
    }
    writer.write_all(&(response.completed as u64).to_le_bytes())?;
    match (&response.error, &response.warning) {
        (Some(_), Some(_)) => unreachable!("response fields were validated"),
        (Some(error), None) => {
            writer.write_all(&[1])?;
            let bytes = error.as_bytes();
            write_bytes(&mut writer, &bytes[..bytes.len().min(MAX_ERROR_BYTES)])?;
        }
        (None, Some(warning)) => {
            writer.write_all(&[2])?;
            let bytes = warning.as_bytes();
            write_bytes(&mut writer, &bytes[..bytes.len().min(MAX_ERROR_BYTES)])?;
        }
        (None, None) => writer.write_all(&[0])?,
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn read_response(mut reader: impl Read) -> io::Result<Response> {
    let mut completed = [0; 8];
    reader.read_exact(&mut completed)?;
    let mut message_kind = [0];
    reader.read_exact(&mut message_kind)?;
    let (error, warning) = match message_kind[0] {
        0 => (None, None),
        1 => (
            Some(String::from_utf8_lossy(&read_bytes(&mut reader)?).into_owned()),
            None,
        ),
        2 => (
            None,
            Some(String::from_utf8_lossy(&read_bytes(&mut reader)?).into_owned()),
        ),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid response",
            ));
        }
    };
    Ok(Response {
        completed: u64::from_le_bytes(completed) as usize,
        error,
        warning,
    })
}

#[cfg(unix)]
fn write_path(writer: impl Write, path: &std::path::Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    write_bytes(writer, path.as_os_str().as_bytes())
}

#[cfg(unix)]
fn validate_path(path: &std::path::Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    if path.as_os_str().as_bytes().len() > MAX_PATH_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path too large",
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "macos", all(test, unix)))]
fn validate_restore_origin_names(names: &[String]) -> io::Result<()> {
    if names.len() > MAX_ITEMS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "too many names",
        ));
    }
    let total_bytes = names.iter().try_fold(0usize, |total, name| {
        if name.len() > MAX_PATH_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "name too large",
            ));
        }
        total
            .checked_add(name.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "names are too large"))
    })?;
    if total_bytes > MAX_TOTAL_NAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "names are too large",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn read_path(reader: impl Read) -> io::Result<PathBuf> {
    Ok(PathBuf::from(OsString::from_vec(read_bytes(reader)?)))
}

#[cfg(unix)]
fn write_u32(mut writer: impl Write, value: usize) -> io::Result<()> {
    let value = u32::try_from(value)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "value too large"))?;
    writer.write_all(&value.to_le_bytes())
}

#[cfg(unix)]
fn read_u32(mut reader: impl Read) -> io::Result<usize> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes) as usize)
}

#[cfg(unix)]
fn write_bytes(mut writer: impl Write, bytes: &[u8]) -> io::Result<()> {
    if bytes.len() > MAX_PATH_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "frame too large",
        ));
    }
    write_u32(&mut writer, bytes.len())?;
    writer.write_all(bytes)
}

#[cfg(unix)]
fn read_bytes(mut reader: impl Read) -> io::Result<Vec<u8>> {
    let len = read_u32(&mut reader)?;
    if len > MAX_PATH_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let mut bytes = vec![0; len];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

#[cfg(unix)]
pub fn run() -> anyhow::Result<()> {
    validate_credentials()?;
    let request = read_request(io::stdin().lock())?;
    let response = match request {
        Request::Trash(paths) => crate::app::run_user_trash_helper(&paths),
        Request::Restore(path) => restore_response(&path),
        #[cfg(target_os = "macos")]
        Request::RemoveRestoreOrigins(names) => {
            let names_ref = names.iter().map(String::as_str).collect::<Vec<_>>();
            match crate::file_operations::remove_restore_origins_checked(&names_ref) {
                Ok(()) => Response {
                    completed: names.len(),
                    error: None,
                    warning: None,
                },
                Err(error) => Response {
                    completed: 0,
                    error: Some(error.to_string()),
                    warning: None,
                },
            }
        }
    };
    write_response(io::stdout().lock(), &response)?;
    Ok(())
}

#[cfg(unix)]
fn restore_response(path: &std::path::Path) -> Response {
    #[cfg(target_os = "macos")]
    {
        return checked_restore_response(
            crate::file_operations::restore_trash_item_checked_metadata(path),
        );
    }

    #[cfg(not(target_os = "macos"))]
    match crate::file_operations::restore_trash_item(path) {
        Ok(()) => Response {
            completed: 1,
            error: None,
            warning: None,
        },
        Err(error) => Response {
            completed: 0,
            error: Some(error.to_string()),
            warning: None,
        },
    }
}

#[cfg(any(target_os = "macos", all(test, unix)))]
fn checked_restore_response(result: anyhow::Result<Option<anyhow::Error>>) -> Response {
    match result {
        Ok(None) => Response {
            completed: 1,
            error: None,
            warning: None,
        },
        Ok(Some(error)) => Response {
            completed: 1,
            error: None,
            warning: Some(format!("could not update Trash restore metadata: {error}")),
        },
        Err(error) => Response {
            completed: 0,
            error: Some(error.to_string()),
            warning: None,
        },
    }
}

#[cfg(unix)]
fn validate_credentials() -> anyhow::Result<()> {
    let expected_uid = std::env::var("ELIO_HELPER_UID")?.parse::<libc::uid_t>()?;
    let expected_gid = std::env::var("ELIO_HELPER_GID")?.parse::<libc::gid_t>()?;
    let mut expected_groups = std::env::var("ELIO_HELPER_GROUPS")?
        .split(',')
        .map(str::parse::<libc::gid_t>)
        .collect::<Result<Vec<_>, _>>()?;
    let (uid, euid, gid, egid) = unsafe {
        (
            libc::getuid(),
            libc::geteuid(),
            libc::getgid(),
            libc::getegid(),
        )
    };
    anyhow::ensure!(
        expected_uid != 0 && uid == expected_uid && euid == expected_uid,
        "invalid helper uid"
    );
    anyhow::ensure!(
        gid == expected_gid && egid == expected_gid,
        "invalid helper gid"
    );
    let mut group_count = unsafe { libc::getgroups(0, std::ptr::null_mut()) };
    anyhow::ensure!(group_count >= 0, "cannot read helper groups");
    let mut groups = vec![0; group_count as usize];
    group_count = unsafe { libc::getgroups(group_count, groups.as_mut_ptr()) };
    anyhow::ensure!(group_count >= 0, "cannot read helper groups");
    groups.truncate(group_count as usize);
    groups.sort_unstable();
    groups.dedup();
    expected_groups.sort_unstable();
    expected_groups.dedup();
    anyhow::ensure!(groups == expected_groups, "invalid helper groups");
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        let (mut real_uid, mut effective_uid, mut saved_uid) = (0, 0, 0);
        let (mut real_gid, mut effective_gid, mut saved_gid) = (0, 0, 0);
        anyhow::ensure!(
            unsafe { libc::getresuid(&mut real_uid, &mut effective_uid, &mut saved_uid) } == 0,
            "cannot read helper uid state"
        );
        anyhow::ensure!(
            unsafe { libc::getresgid(&mut real_gid, &mut effective_gid, &mut saved_gid) } == 0,
            "cannot read helper gid state"
        );
        anyhow::ensure!(
            real_uid == expected_uid && effective_uid == expected_uid && saved_uid == expected_uid,
            "invalid saved helper uid"
        );
        anyhow::ensure!(
            real_gid == expected_gid && effective_gid == expected_gid && saved_gid == expected_gid,
            "invalid saved helper gid"
        );
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn run() -> anyhow::Result<()> {
    anyhow::bail!("user filesystem helper is unsupported on this platform")
}

#[cfg(all(test, unix))]
#[path = "tests/filesystem_helper.rs"]
mod tests;
