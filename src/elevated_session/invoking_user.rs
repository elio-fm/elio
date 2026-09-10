#[cfg(unix)]
use std::{
    env,
    ffi::{CStr, CString, OsStr, OsString},
    mem,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::PathBuf,
    ptr,
    sync::OnceLock,
};

#[cfg(target_os = "linux")]
use std::io::Read;

#[cfg(unix)]
pub(crate) const SESSION_ENVIRONMENT_KEYS: &[&str] = &[
    "DBUS_SESSION_BUS_ADDRESS",
    "DESKTOP_SESSION",
    "DISPLAY",
    "EDITOR",
    "ELIO_ZOXIDE_OPTS",
    "FZF_DEFAULT_OPTS",
    "PATH",
    "VISUAL",
    "WAYLAND_DISPLAY",
    "XAUTHORITY",
    "XDG_CACHE_HOME",
    "XDG_CONFIG_DIRS",
    "XDG_CONFIG_HOME",
    "XDG_CURRENT_DESKTOP",
    "XDG_DATA_DIRS",
    "XDG_DATA_HOME",
    "XDG_RUNTIME_DIR",
    "XDG_SESSION_DESKTOP",
    "XDG_SESSION_TYPE",
    "_ZO_DATA_DIR",
    "_ZO_ECHO",
    "_ZO_EXCLUDE_DIRS",
    "_ZO_MAXAGE",
    "_ZO_RESOLVE_SYMLINKS",
];

#[cfg(unix)]
#[derive(Debug)]
pub(crate) struct InvokingUser {
    pub(crate) uid: libc::uid_t,
    pub(crate) gid: libc::gid_t,
    pub(crate) name: OsString,
    pub(crate) home: PathBuf,
    pub(crate) shell: OsString,
    pub(crate) groups: Vec<libc::gid_t>,
    pub(crate) session_environment: Vec<(OsString, OsString)>,
    pub(crate) xdg_config_home: Option<PathBuf>,
    pub(crate) xdg_data_home: Option<PathBuf>,
}

#[cfg(unix)]
#[derive(Debug)]
pub(crate) enum InvocationContext {
    Normal,
    RootSession,
    Elevated(InvokingUser),
    ElevatedUnresolved,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IdentityClaim {
    Absent,
    Root,
    User(libc::uid_t),
    Invalid,
}

#[cfg(unix)]
static INVOCATION_CONTEXT: OnceLock<InvocationContext> = OnceLock::new();

#[cfg(unix)]
pub(crate) fn context() -> &'static InvocationContext {
    INVOCATION_CONTEXT.get_or_init(detect_context)
}

#[cfg(unix)]
fn detect_context() -> InvocationContext {
    let sudo_uid = env::var_os("SUDO_UID");
    let sudo_user = env::var_os("SUDO_USER");
    let doas_user = env::var_os("DOAS_USER");
    let has_other_sudo_metadata = ["SUDO_COMMAND", "SUDO_GID"]
        .into_iter()
        .any(|name| env::var_os(name).is_some());
    invocation_context(
        unsafe { libc::geteuid() },
        sudo_uid.as_deref(),
        sudo_user.as_deref(),
        has_other_sudo_metadata,
        doas_user.as_deref(),
    )
}

#[cfg(unix)]
pub(crate) fn env_var(name: &str) -> Option<OsString> {
    env_var_for_context(context(), name, env::var_os(name))
}

#[cfg(unix)]
fn env_var_for_context(
    context: &InvocationContext,
    name: &str,
    normal_value: Option<OsString>,
) -> Option<OsString> {
    match context {
        InvocationContext::Normal | InvocationContext::RootSession => normal_value,
        InvocationContext::Elevated(user) => {
            user_environment_value(user, name).map(OsStr::to_os_string)
        }
        InvocationContext::ElevatedUnresolved => None,
    }
}

#[cfg(not(unix))]
pub(crate) fn env_var(name: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(name)
}

#[cfg(unix)]
pub(crate) fn home_dir() -> Option<PathBuf> {
    match context() {
        InvocationContext::Elevated(user) => Some(user.home.clone()),
        InvocationContext::Normal
        | InvocationContext::RootSession
        | InvocationContext::ElevatedUnresolved => dirs::home_dir(),
    }
}

#[cfg(not(unix))]
pub(crate) fn home_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir()
}

pub(crate) fn trash_home_dir() -> Option<std::path::PathBuf> {
    #[cfg(unix)]
    return trash_home_for_context(context());
    #[cfg(not(unix))]
    return dirs::home_dir();
}

#[cfg(unix)]
fn trash_home_for_context(context: &InvocationContext) -> Option<PathBuf> {
    match context {
        InvocationContext::Normal | InvocationContext::RootSession => dirs::home_dir(),
        InvocationContext::Elevated(user) => Some(user.home.clone()),
        InvocationContext::ElevatedUnresolved => None,
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn trash_data_dir() -> Option<PathBuf> {
    match context() {
        InvocationContext::Normal | InvocationContext::RootSession => dirs::data_dir(),
        InvocationContext::Elevated(user) => user
            .xdg_data_home
            .clone()
            .or_else(|| Some(user.home.join(".local/share"))),
        InvocationContext::ElevatedUnresolved => None,
    }
}

#[cfg(unix)]
fn invocation_context(
    effective_uid: libc::uid_t,
    sudo_uid: Option<&OsStr>,
    sudo_user: Option<&OsStr>,
    has_other_sudo_metadata: bool,
    doas_user: Option<&OsStr>,
) -> InvocationContext {
    if effective_uid != 0 {
        return InvocationContext::Normal;
    }

    let uid = match agreed_invoking_uid(
        sudo_identity_claim(sudo_uid, sudo_user, has_other_sudo_metadata),
        doas_identity_claim(doas_user),
    ) {
        Ok(None) => return InvocationContext::RootSession,
        Ok(Some(uid)) => uid,
        Err(()) => return InvocationContext::ElevatedUnresolved,
    };
    let user = passwd_by_uid(uid);
    let Some(mut user) = user else {
        return InvocationContext::ElevatedUnresolved;
    };
    let Some(groups) = supplementary_groups(&user.name, user.gid) else {
        return InvocationContext::ElevatedUnresolved;
    };
    user.groups = groups;
    user.session_environment = invoking_user_environment(user.uid);
    user.xdg_config_home =
        validated_xdg_home(user_environment_value(&user, "XDG_CONFIG_HOME"), &user);
    #[cfg(not(target_os = "macos"))]
    {
        user.xdg_data_home =
            validated_xdg_home(user_environment_value(&user, "XDG_DATA_HOME"), &user);
    }
    InvocationContext::Elevated(user)
}

#[cfg(unix)]
fn sudo_identity_claim(
    sudo_uid: Option<&OsStr>,
    sudo_user: Option<&OsStr>,
    has_other_sudo_metadata: bool,
) -> IdentityClaim {
    if sudo_uid.is_none() && sudo_user.is_none() && !has_other_sudo_metadata {
        return IdentityClaim::Absent;
    }
    let Some(uid) = parse_uid(sudo_uid) else {
        return IdentityClaim::Invalid;
    };
    if sudo_user.and_then(uid_by_name) != Some(uid) {
        return IdentityClaim::Invalid;
    }
    identity_claim(uid)
}

#[cfg(unix)]
fn doas_identity_claim(doas_user: Option<&OsStr>) -> IdentityClaim {
    match doas_user {
        None => IdentityClaim::Absent,
        Some(user) => uid_by_name(user)
            .map(identity_claim)
            .unwrap_or(IdentityClaim::Invalid),
    }
}

#[cfg(unix)]
fn identity_claim(uid: libc::uid_t) -> IdentityClaim {
    if uid == 0 {
        IdentityClaim::Root
    } else {
        IdentityClaim::User(uid)
    }
}

#[cfg(unix)]
fn agreed_invoking_uid(
    sudo: IdentityClaim,
    doas: IdentityClaim,
) -> Result<Option<libc::uid_t>, ()> {
    use IdentityClaim::{Absent, Root, User};

    match (sudo, doas) {
        (Absent, Absent | Root) | (Root, Absent | Root) => Ok(None),
        (User(uid), Absent) | (Absent, User(uid)) => Ok(Some(uid)),
        (User(sudo_uid), User(doas_uid)) if sudo_uid == doas_uid => Ok(Some(sudo_uid)),
        _ => Err(()),
    }
}

#[cfg(unix)]
fn parse_uid(value: Option<&OsStr>) -> Option<libc::uid_t> {
    value?.to_str()?.parse().ok()
}

#[cfg(unix)]
fn passwd_by_uid(uid: libc::uid_t) -> Option<InvokingUser> {
    passwd(|record, buffer, len, result| unsafe {
        libc::getpwuid_r(uid, record, buffer, len, result)
    })
}

#[cfg(unix)]
fn uid_by_name(name: &OsStr) -> Option<libc::uid_t> {
    let bytes = name.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let name = CString::new(bytes).ok()?;
    passwd(|record, buffer, len, result| unsafe {
        libc::getpwnam_r(name.as_ptr(), record, buffer, len, result)
    })
    .map(|user| user.uid)
}

#[cfg(unix)]
fn passwd(
    mut lookup: impl FnMut(
        *mut libc::passwd,
        *mut libc::c_char,
        usize,
        *mut *mut libc::passwd,
    ) -> libc::c_int,
) -> Option<InvokingUser> {
    let mut buffer = vec![0_u8; passwd_buffer_size()];
    loop {
        let mut record = unsafe { mem::zeroed::<libc::passwd>() };
        let mut result = ptr::null_mut();
        let status = lookup(
            &mut record,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        );
        if status == libc::ERANGE && buffer.len() < 1024 * 1024 {
            buffer.resize(buffer.len() * 2, 0);
            continue;
        }
        if status != 0 || result.is_null() || record.pw_dir.is_null() || record.pw_name.is_null() {
            return None;
        }
        let home = unsafe { CStr::from_ptr(record.pw_dir) }.to_bytes();
        let name = unsafe { CStr::from_ptr(record.pw_name) }.to_bytes();
        let shell = if record.pw_shell.is_null() {
            &[][..]
        } else {
            unsafe { CStr::from_ptr(record.pw_shell) }.to_bytes()
        };
        if home.is_empty() || name.is_empty() {
            return None;
        }
        return Some(InvokingUser {
            uid: record.pw_uid,
            gid: record.pw_gid,
            name: OsString::from_vec(name.to_vec()),
            home: PathBuf::from(OsString::from_vec(home.to_vec())),
            shell: if shell.is_empty() {
                OsString::from("/bin/sh")
            } else {
                OsString::from_vec(shell.to_vec())
            },
            groups: vec![record.pw_gid],
            session_environment: Vec::new(),
            xdg_config_home: None,
            xdg_data_home: None,
        });
    }
}

#[cfg(unix)]
fn validated_xdg_home(value: Option<&OsStr>, user: &InvokingUser) -> Option<PathBuf> {
    use std::os::unix::fs::MetadataExt;

    value
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .filter(|path| {
            path.ancestors()
                .find_map(|ancestor| match std::fs::metadata(ancestor) {
                    Ok(metadata) => Some(metadata.is_dir() && metadata.uid() == user.uid),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(_) => Some(false),
                })
                == Some(true)
        })
}

#[cfg(unix)]
pub(crate) fn user_environment_value<'a>(user: &'a InvokingUser, name: &str) -> Option<&'a OsStr> {
    user.session_environment
        .iter()
        .find_map(|(key, value)| (key == name).then_some(value.as_os_str()))
}

#[cfg(unix)]
fn invoking_user_environment(uid: libc::uid_t) -> Vec<(OsString, OsString)> {
    #[cfg(target_os = "linux")]
    {
        // Do not fall back to root Elio's environment: if the trusted
        // same-user ancestor cannot be verified, omit session values.
        linux_ancestor_environment(uid).unwrap_or_default()
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = uid;
        SESSION_ENVIRONMENT_KEYS
            .iter()
            .filter_map(|name| env::var_os(name).map(|value| (OsString::from(name), value)))
            .collect()
    }
}

#[cfg(target_os = "linux")]
fn linux_ancestor_environment(uid: libc::uid_t) -> Option<Vec<(OsString, OsString)>> {
    // sudo/doas may strip desktop-session variables before Elio starts. Walk
    // only through privileged ancestors to the first process wholly owned by
    // the already-resolved invoking UID, then recover only the fixed allowlist.
    let mut pid = unsafe { libc::getppid() };
    let mut elevation_environment = None;
    for _ in 0..32 {
        if pid <= 1 {
            return None;
        }
        let (parent, process_uids) = linux_process_identity(pid)?;
        if process_uids.iter().all(|process_uid| *process_uid == uid) {
            let mut environment = linux_process_environment(pid)?;
            let (_, verified_uids) = linux_process_identity(pid)?;
            if !verified_uids.iter().all(|process_uid| *process_uid == uid) {
                return None;
            }
            if let Some(elevation_environment) = elevation_environment {
                environment = merge_linux_environments(environment, elevation_environment);
            }
            return Some(environment);
        }
        let privileged_bridge = process_uids
            .iter()
            .all(|process_uid| *process_uid == 0 || *process_uid == uid)
            && process_uids.contains(&0);
        if !privileged_bridge || parent == pid {
            return None;
        }
        if elevation_environment.is_none() {
            elevation_environment = linux_elevation_environment(pid, process_uids);
        }
        pid = parent;
    }
    None
}

#[cfg(target_os = "linux")]
fn linux_elevation_environment(
    pid: libc::pid_t,
    expected_uids: [libc::uid_t; 4],
) -> Option<Vec<(OsString, OsString)>> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let executable = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    let name = executable.file_name()?.as_bytes();
    if name != b"sudo" && name != b"doas" {
        return None;
    }
    let metadata = std::fs::metadata(&executable).ok()?;
    if metadata.uid() != 0 || metadata.permissions().mode() & 0o4000 == 0 {
        return None;
    }
    let environment = linux_process_environment(pid)?;
    let (_, verified_uids) = linux_process_identity(pid)?;
    (verified_uids == expected_uids).then_some(environment)
}

#[cfg(target_os = "linux")]
fn merge_linux_environments(
    base: Vec<(OsString, OsString)>,
    elevation: Vec<(OsString, OsString)>,
) -> Vec<(OsString, OsString)> {
    let mut environment: std::collections::BTreeMap<_, _> = base.into_iter().collect();
    environment.extend(elevation);
    environment.into_iter().collect()
}

#[cfg(target_os = "linux")]
fn linux_process_identity(pid: libc::pid_t) -> Option<(libc::pid_t, [libc::uid_t; 4])> {
    let status = std::fs::read(format!("/proc/{pid}/status")).ok()?;
    parse_linux_process_identity(&status)
}

#[cfg(target_os = "linux")]
fn parse_linux_process_identity(status: &[u8]) -> Option<(libc::pid_t, [libc::uid_t; 4])> {
    let text = std::str::from_utf8(status).ok()?;
    let parent = text.lines().find_map(|line| {
        line.strip_prefix("PPid:")
            .and_then(|value| value.trim().parse().ok())
    })?;
    let uids = text.lines().find_map(|line| {
        let mut values = line.strip_prefix("Uid:")?.split_whitespace();
        Some([
            values.next()?.parse().ok()?,
            values.next()?.parse().ok()?,
            values.next()?.parse().ok()?,
            values.next()?.parse().ok()?,
        ])
    })?;
    Some((parent, uids))
}

#[cfg(target_os = "linux")]
fn linux_process_environment(pid: libc::pid_t) -> Option<Vec<(OsString, OsString)>> {
    let file = std::fs::File::open(format!("/proc/{pid}/environ")).ok()?;
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 1024 * 1024 {
        return None;
    }
    Some(parse_allowlisted_environment(&bytes))
}

#[cfg(target_os = "linux")]
fn parse_allowlisted_environment(bytes: &[u8]) -> Vec<(OsString, OsString)> {
    let mut environment = std::collections::BTreeMap::new();
    for item in bytes.split(|byte| *byte == 0) {
        let Some(separator) = item.iter().position(|byte| *byte == b'=') else {
            continue;
        };
        let (name, value) = item.split_at(separator);
        if SESSION_ENVIRONMENT_KEYS
            .iter()
            .any(|candidate| candidate.as_bytes() == name)
        {
            environment.insert(
                OsString::from_vec(name.to_vec()),
                OsString::from_vec(value[1..].to_vec()),
            );
        }
    }
    environment.into_iter().collect()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn supplementary_groups(name: &OsStr, primary_gid: libc::gid_t) -> Option<Vec<libc::gid_t>> {
    let name = CString::new(name.as_bytes()).ok()?;
    let mut count: libc::c_int = 0;
    unsafe {
        libc::getgrouplist(name.as_ptr(), primary_gid, ptr::null_mut(), &mut count);
    }
    if count <= 0 {
        return None;
    }
    let mut groups = vec![primary_gid; count as usize];
    let status =
        unsafe { libc::getgrouplist(name.as_ptr(), primary_gid, groups.as_mut_ptr(), &mut count) };
    if status < 0 || count <= 0 {
        return None;
    }
    groups.truncate(count as usize);
    groups.push(primary_gid);
    groups.sort_unstable();
    groups.dedup();
    Some(groups)
}

#[cfg(target_os = "macos")]
fn supplementary_groups(name: &OsStr, primary_gid: libc::gid_t) -> Option<Vec<libc::gid_t>> {
    const INITIAL_CAPACITY: usize = 16;
    const MAX_CAPACITY: usize = 16 * 1024;

    let name = CString::new(name.as_bytes()).ok()?;
    let primary_group = libc::c_int::try_from(primary_gid).ok()?;
    let mut capacity = INITIAL_CAPACITY;

    loop {
        let mut native_groups = vec![primary_group; capacity];
        let mut count = libc::c_int::try_from(native_groups.len()).ok()?;
        let status = unsafe {
            libc::getgrouplist(
                name.as_ptr(),
                primary_group,
                native_groups.as_mut_ptr(),
                &mut count,
            )
        };

        if status == 0 {
            let returned = usize::try_from(count).ok()?;
            if returned == 0 || returned > native_groups.len() {
                return None;
            }
            let mut groups = native_groups
                .into_iter()
                .take(returned)
                .map(libc::gid_t::try_from)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            groups.push(primary_gid);
            groups.sort_unstable();
            groups.dedup();
            return Some(groups);
        }

        // Darwin does not support a null-buffer sizing probe, and on -1 the
        // returned count is only the number of entries that fit. Grow from
        // the previous capacity instead, with a fixed upper bound.
        if status != -1 || capacity >= MAX_CAPACITY {
            return None;
        }
        let next_capacity = capacity.checked_mul(2)?.min(MAX_CAPACITY);
        if next_capacity <= capacity {
            return None;
        }
        capacity = next_capacity;
    }
}

#[cfg(unix)]
fn passwd_buffer_size() -> usize {
    let size = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
    if size > 0 { size as usize } else { 16 * 1024 }
}

#[cfg(all(test, unix))]
#[path = "tests/invoking_user.rs"]
mod tests;
