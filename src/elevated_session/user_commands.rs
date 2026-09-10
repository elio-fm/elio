#[cfg(unix)]
use super::{
    InvocationContext, InvokingUser, SESSION_ENVIRONMENT_KEYS, context, user_environment_value,
};
#[cfg(unix)]
use std::{
    ffi::{CString, OsStr},
    io,
    os::unix::{ffi::OsStrExt, process::CommandExt},
    path::{Path, PathBuf},
    process::Command,
};

/// Applies the identity policy for user-facing external applications.
///
/// Normal launches are left untouched. Elevated launches run as the invoking
/// user, use `cwd` when the action has an explicit directory, and otherwise
/// start from that user's home. Unresolved elevated identity fails closed.
#[cfg(unix)]
pub(crate) fn prepare_external(command: &mut Command, cwd: Option<&Path>) -> io::Result<()> {
    prepare_external_for_context(command, context(), cwd)
}

#[cfg(unix)]
fn prepare_external_for_context(
    command: &mut Command,
    context: &InvocationContext,
    cwd: Option<&Path>,
) -> io::Result<()> {
    match context {
        InvocationContext::Normal | InvocationContext::RootSession => Ok(()),
        InvocationContext::Elevated(user) => {
            let cwd = cwd.unwrap_or(&user.home);
            prepare(command, user, Some(cwd))
        }
        InvocationContext::ElevatedUnresolved => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "could not resolve invoking user",
        )),
    }
}

#[cfg(unix)]
pub(crate) fn prepare(
    command: &mut Command,
    user: &InvokingUser,
    cwd: Option<&Path>,
) -> io::Result<()> {
    let cwd = cwd
        .map(|path| CString::new(path.as_os_str().as_bytes()))
        .transpose()
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "working directory contains NUL",
            )
        })?;

    apply_user_environment(command, user);
    if let Some(path) = cwd.as_ref() {
        command.env("PWD", OsStr::from_bytes(path.as_bytes()));
    }

    let uid = user.uid;
    let gid = user.gid;
    let groups = user.groups.clone();
    unsafe {
        command.pre_exec(move || {
            drop_privileges(uid, gid, &groups)?;
            match &cwd {
                Some(cwd) if libc::chdir(cwd.as_ptr()) != 0 => {
                    return Err(io::Error::last_os_error());
                }
                _ => {}
            }
            Ok(())
        });
    }
    Ok(())
}

#[cfg(unix)]
fn apply_user_environment(command: &mut Command, user: &InvokingUser) {
    command
        .env("HOME", &user.home)
        .env("USER", &user.name)
        .env("USERNAME", &user.name)
        .env("LOGNAME", &user.name)
        .env("SHELL", &user.shell)
        .env_remove("MAIL")
        .env_remove("OLDPWD")
        .env_remove("SUDO_COMMAND")
        .env_remove("SUDO_GID")
        .env_remove("SUDO_UID")
        .env_remove("SUDO_USER")
        .env_remove("DOAS_USER");

    for name in SESSION_ENVIRONMENT_KEYS {
        command.env_remove(name);
    }
    for (name, value) in &user.session_environment {
        if !matches!(
            name.to_str(),
            Some(
                "XAUTHORITY"
                    | "XDG_CACHE_HOME"
                    | "XDG_CONFIG_HOME"
                    | "XDG_DATA_HOME"
                    | "XDG_RUNTIME_DIR"
            )
        ) {
            command.env(name, value);
        }
    }

    if let Some(path) = &user.xdg_config_home {
        command.env("XDG_CONFIG_HOME", path);
    }
    if let Some(path) = &user.xdg_data_home {
        command.env("XDG_DATA_HOME", path);
    }
    set_owned_absolute_env(command, user, "XDG_CACHE_HOME");
    set_owned_path_env(command, user, "XAUTHORITY");
    if let Some(path) = user_environment_value(user, "XDG_RUNTIME_DIR")
        && valid_runtime_dir(Some(path), user.uid)
    {
        command.env("XDG_RUNTIME_DIR", path);
    }
}

#[cfg(unix)]
fn set_owned_absolute_env(command: &mut Command, user: &InvokingUser, name: &str) {
    if let Some(path) = user_environment_value(user, name)
        .map(PathBuf::from)
        .filter(|path| valid_owned_absolute_path(path, user.uid))
    {
        command.env(name, path);
    }
}

#[cfg(unix)]
fn set_owned_path_env(command: &mut Command, user: &InvokingUser, name: &str) {
    use std::os::unix::fs::MetadataExt;

    if let Some(path) = user_environment_value(user, name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .filter(|path| std::fs::metadata(path).is_ok_and(|metadata| metadata.uid() == user.uid))
    {
        command.env(name, path);
    }
}

#[cfg(unix)]
fn valid_owned_absolute_path(path: &Path, uid: libc::uid_t) -> bool {
    use std::os::unix::fs::MetadataExt;

    path.is_absolute()
        && path
            .ancestors()
            .find_map(|ancestor| match std::fs::metadata(ancestor) {
                Ok(metadata) => Some(metadata.is_dir() && metadata.uid() == uid),
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(_) => Some(false),
            })
            == Some(true)
}

#[cfg(unix)]
fn valid_runtime_dir(path: Option<&OsStr>, uid: libc::uid_t) -> bool {
    use std::os::unix::fs::MetadataExt;

    let Some(path) = path.map(Path::new).filter(|path| path.is_absolute()) else {
        return false;
    };
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_dir() && metadata.uid() == uid)
}

#[cfg(unix)]
fn drop_privileges(uid: libc::uid_t, gid: libc::gid_t, groups: &[libc::gid_t]) -> io::Result<()> {
    if unsafe { set_supplementary_groups(groups) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::setgid(gid) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::setuid(uid) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if !credentials_match(uid, gid) {
        return Err(io::Error::from_raw_os_error(libc::EPERM));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
unsafe fn set_supplementary_groups(groups: &[libc::gid_t]) -> libc::c_int {
    unsafe { libc::setgroups(groups.len(), groups.as_ptr()) }
}

#[cfg(all(unix, not(target_os = "linux")))]
unsafe fn set_supplementary_groups(groups: &[libc::gid_t]) -> libc::c_int {
    let Ok(count) = libc::c_int::try_from(groups.len()) else {
        return -1;
    };
    unsafe { libc::setgroups(count, groups.as_ptr()) }
}

#[cfg(unix)]
fn credentials_match(uid: libc::uid_t, gid: libc::gid_t) -> bool {
    if unsafe { libc::getuid() } != uid
        || unsafe { libc::geteuid() } != uid
        || unsafe { libc::getgid() } != gid
        || unsafe { libc::getegid() } != gid
    {
        return false;
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        let (mut real_uid, mut effective_uid, mut saved_uid) = (0, 0, 0);
        let (mut real_gid, mut effective_gid, mut saved_gid) = (0, 0, 0);
        if unsafe { libc::getresuid(&mut real_uid, &mut effective_uid, &mut saved_uid) } != 0
            || unsafe { libc::getresgid(&mut real_gid, &mut effective_gid, &mut saved_gid) } != 0
        {
            return false;
        }
        if (real_uid, effective_uid, saved_uid) != (uid, uid, uid)
            || (real_gid, effective_gid, saved_gid) != (gid, gid, gid)
        {
            return false;
        }
    }

    true
}

#[cfg(all(test, unix))]
#[path = "tests/user_commands.rs"]
mod tests;
