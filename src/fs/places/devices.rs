#[cfg(any(
    target_os = "macos",
    windows,
    target_os = "freebsd",
    target_os = "openbsd"
))]
use super::resolution::path_identity_key;
use crate::app::SidebarItem;
#[cfg(any(
    target_os = "macos",
    windows,
    target_os = "freebsd",
    target_os = "openbsd"
))]
use crate::app::SidebarItemKind;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[cfg(target_os = "macos")]
use std::fs;

#[cfg(target_os = "linux")]
pub(super) fn mounted_device_items(
    home: &Path,
    pinned_paths: &HashSet<PathBuf>,
) -> Vec<SidebarItem> {
    super::linux::mounted_device_items(home, pinned_paths)
}

#[cfg(target_os = "macos")]
pub(super) fn mounted_device_items(
    _home: &Path,
    pinned_paths: &HashSet<PathBuf>,
) -> Vec<SidebarItem> {
    use std::os::unix::fs::MetadataExt;

    // Device ID of the root filesystem — used to skip the boot volume whether it
    // appears as a symlink (older macOS) or a firmlink/bind-mount (Big Sur+).
    let root_dev = fs::metadata("/").map(|metadata| metadata.dev()).ok();

    let Ok(entries) = fs::read_dir("/Volumes") else {
        return Vec::new();
    };

    let mut items = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();

        if pinned_paths.contains(&path_identity_key(&path)) {
            continue;
        }
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if let Some(root_dev) = root_dev {
            if fs::metadata(&path).is_ok_and(|metadata| metadata.dev() == root_dev) {
                continue;
            }
        }
        if !path.is_dir() {
            continue;
        }

        let Some(title) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };

        items.push(SidebarItem::new(
            SidebarItemKind::Device { removable: false },
            title,
            "󰋊",
            path,
        ));
    }

    items.sort_by(|left, right| {
        crate::fs::natural_cmp(
            &left.title.to_ascii_lowercase(),
            &right.title.to_ascii_lowercase(),
        )
        .then_with(|| left.path.cmp(&right.path))
    });

    items
}

#[cfg(windows)]
pub(super) fn mounted_device_items(
    _home: &Path,
    pinned_paths: &HashSet<PathBuf>,
) -> Vec<SidebarItem> {
    let mut items = Vec::new();
    for letter in b'A'..=b'Z' {
        let path = PathBuf::from(format!("{}:\\", letter as char));
        if path.exists() && !pinned_paths.contains(&path_identity_key(&path)) {
            items.push(SidebarItem::new(
                SidebarItemKind::Device { removable: false },
                format!("{}:", letter as char),
                "󰋊",
                path,
            ));
        }
    }
    items
}

// FreeBSD and OpenBSD share the same getmntinfo(3) interface and statfs field
// names, so one implementation covers both.
#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
pub(super) fn mounted_device_items(
    home: &Path,
    pinned_paths: &HashSet<PathBuf>,
) -> Vec<SidebarItem> {
    let mut mntbuf: *mut libc::statfs = std::ptr::null_mut();
    let count = unsafe { libc::getmntinfo(&mut mntbuf, libc::MNT_NOWAIT) };
    if count <= 0 || mntbuf.is_null() {
        return Vec::new();
    }

    let mounts = unsafe { std::slice::from_raw_parts(mntbuf, count as usize) };
    let mut items = Vec::new();

    for mount in mounts {
        let mount_point =
            unsafe { std::ffi::CStr::from_ptr(mount.f_mntonname.as_ptr()) }.to_string_lossy();
        let fstype =
            unsafe { std::ffi::CStr::from_ptr(mount.f_fstypename.as_ptr()) }.to_string_lossy();
        let source =
            unsafe { std::ffi::CStr::from_ptr(mount.f_mntfromname.as_ptr()) }.to_string_lossy();

        let path = PathBuf::from(mount_point.as_ref());

        if path == Path::new("/") || pinned_paths.contains(&path_identity_key(&path)) {
            continue;
        }
        if bsd_system_fstype(&fstype) || bsd_hidden_path(&path) {
            continue;
        }
        if !bsd_user_visible_path(&path, home) {
            continue;
        }

        let title = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                Path::new(source.as_ref())
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| path.display().to_string());

        items.push(SidebarItem::new(
            SidebarItemKind::Device { removable: false },
            title,
            "󰋊",
            path,
        ));
    }

    items.sort_by(|left, right| {
        crate::fs::natural_cmp(
            &left.title.to_ascii_lowercase(),
            &right.title.to_ascii_lowercase(),
        )
        .then_with(|| left.path.cmp(&right.path))
    });

    items
}

// Virtual/system filesystem types to suppress on FreeBSD and OpenBSD.
// The union of both sets is used so the filter is correct on either OS.
#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
fn bsd_system_fstype(fstype: &str) -> bool {
    matches!(
        fstype,
        // FreeBSD
        "devfs" | "fdescfs" | "linprocfs" | "linsysfs" | "nullfs" | "procfs" | "tmpfs"
            | "unionfs"
            // OpenBSD
            | "kernfs" | "mfs"
    )
}

#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
fn bsd_hidden_path(path: &Path) -> bool {
    path.starts_with("/dev")
        || path.starts_with("/proc")
        || path.starts_with("/kern")
        || path.starts_with("/compat")
}

#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
fn bsd_user_visible_path(path: &Path, home: &Path) -> bool {
    path.starts_with(home) || path.starts_with("/media") || path.starts_with("/mnt")
}

// NetBSD uses statvfs / getmntinfo with a different struct layout; other
// exotic Unices are similarly untested. Leave those as an empty list for now.
#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    windows
)))]
pub(super) fn mounted_device_items(
    _home: &Path,
    _pinned_paths: &HashSet<PathBuf>,
) -> Vec<SidebarItem> {
    Vec::new()
}
