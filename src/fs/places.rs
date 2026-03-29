use crate::app::{SidebarItem, SidebarItemKind, SidebarRow};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

/// Returns the current user's home directory.
///
/// Delegates to the [`dirs`] crate, which reads `$HOME` on Unix and
/// `%USERPROFILE%` / `{FOLDERID_Profile}` on Windows. Returns `None` only in
/// the unlikely event that none of the relevant system APIs succeed.
pub(crate) fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

pub(crate) fn build_sidebar_rows() -> Vec<SidebarRow> {
    let home = home_dir().unwrap_or_else(|| {
        #[cfg(windows)]
        return PathBuf::from("C:\\");
        #[cfg(not(windows))]
        return PathBuf::from("/");
    });
    let pinned_items = build_pinned_sidebar_items(&home);
    let pinned_paths = pinned_items
        .iter()
        .map(|item| item.path.clone())
        .collect::<HashSet<_>>();
    let mut rows = pinned_items
        .into_iter()
        .map(SidebarRow::Item)
        .collect::<Vec<_>>();
    let device_items = mounted_device_items(&home, &pinned_paths);
    if !device_items.is_empty() {
        rows.push(SidebarRow::Section { title: "Devices" });
        rows.extend(device_items.into_iter().map(SidebarRow::Item));
    }
    rows
}

fn build_pinned_sidebar_items(home: &Path) -> Vec<SidebarItem> {
    let mut items = Vec::new();

    items.push(SidebarItem::new(
        SidebarItemKind::Home,
        "Home",
        "󰋜",
        home.to_path_buf(),
    ));

    // dirs::*_dir() reads XDG_*_DIR on Linux, system folders on macOS/Windows.
    for (kind, title, path, icon) in [
        (
            SidebarItemKind::Desktop,
            "Desktop",
            dirs::desktop_dir(),
            "󰍹",
        ),
        (
            SidebarItemKind::Documents,
            "Documents",
            dirs::document_dir(),
            "󰲃",
        ),
        (
            SidebarItemKind::Downloads,
            "Downloads",
            dirs::download_dir(),
            "󰉍",
        ),
        (
            SidebarItemKind::Pictures,
            "Pictures",
            dirs::picture_dir(),
            "󰉏",
        ),
        (SidebarItemKind::Music, "Music", dirs::audio_dir(), "󱍙"),
        (
            SidebarItemKind::Videos,
            if cfg!(target_os = "macos") {
                "Movies"
            } else {
                "Videos"
            },
            dirs::video_dir(),
            "󰕧",
        ),
    ] {
        if let Some(path) = path
            && path.exists()
        {
            items.push(SidebarItem::new(kind, title, icon, path));
        }
    }

    #[cfg(unix)]
    items.push(SidebarItem::new(
        SidebarItemKind::Root,
        "Root",
        "󰋊",
        PathBuf::from("/"),
    ));

    if let Some(trash) = trash_dir(home) {
        items.push(SidebarItem::new(
            SidebarItemKind::Trash,
            "Trash",
            "󰩺",
            trash,
        ));
    }

    items
}

/// Returns the path to the user's trash directory, or `None` if it cannot be determined.
///
/// - **Linux / BSD (freedesktop):** `$XDG_DATA_HOME/Trash/files`, falling back to
///   `~/.local/share/Trash/files`. The `files/` subdirectory holds the actual items;
///   the sibling `info/` directory holds `.trashinfo` metadata used for restore.
/// - **macOS:** `~/.Trash`
/// - **Windows:** always returns `None`. The Recycle Bin is a virtual shell folder
///   that is not practically accessible as a regular filesystem path.
pub(crate) fn trash_dir(home: &Path) -> Option<PathBuf> {
    // dirs::data_dir() honours $XDG_DATA_HOME on Linux/BSD, returns
    // ~/Library/Application Support on macOS, and %APPDATA% on Windows.
    if let Some(data_dir) = dirs::data_dir() {
        let xdg_trash = data_dir.join("Trash/files");
        if xdg_trash.exists() {
            return Some(xdg_trash);
        }
    }

    // macOS: ~/.Trash (freedesktop path above won't exist there)
    let mac_trash = home.join(".Trash");
    if mac_trash.exists() {
        return Some(mac_trash);
    }

    None
}

#[cfg(target_os = "macos")]
fn mounted_device_items(_home: &Path, pinned_paths: &HashSet<PathBuf>) -> Vec<SidebarItem> {
    use super::sort::natural_cmp;
    use std::os::unix::fs::MetadataExt;

    // Device ID of the root filesystem — used to skip the boot volume whether it
    // appears as a symlink (older macOS) or a firmlink/bind-mount (Big Sur+).
    let root_dev = fs::metadata("/").map(|m| m.dev()).ok();

    let Ok(entries) = fs::read_dir("/Volumes") else {
        return Vec::new();
    };

    let mut items = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();

        if pinned_paths.contains(&path) {
            continue;
        }
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if let Some(root_dev) = root_dev {
            if fs::metadata(&path).is_ok_and(|m| m.dev() == root_dev) {
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
        natural_cmp(
            &left.title.to_ascii_lowercase(),
            &right.title.to_ascii_lowercase(),
        )
        .then_with(|| left.path.cmp(&right.path))
    });

    items
}

#[cfg(windows)]
fn mounted_device_items(_home: &Path, pinned_paths: &HashSet<PathBuf>) -> Vec<SidebarItem> {
    let mut items = Vec::new();
    for letter in b'A'..=b'Z' {
        let path = PathBuf::from(format!("{}:\\", letter as char));
        if path.exists() && !pinned_paths.contains(&path) {
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
fn mounted_device_items(home: &Path, pinned_paths: &HashSet<PathBuf>) -> Vec<SidebarItem> {
    use super::sort::natural_cmp;

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

        if path == Path::new("/") || pinned_paths.contains(&path) {
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
            .and_then(|n| n.to_str())
            .filter(|n| !n.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                Path::new(source.as_ref())
                    .file_name()
                    .and_then(|n| n.to_str())
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

    items.sort_by(|a, b| {
        natural_cmp(&a.title.to_ascii_lowercase(), &b.title.to_ascii_lowercase())
            .then_with(|| a.path.cmp(&b.path))
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
fn mounted_device_items(_home: &Path, _pinned_paths: &HashSet<PathBuf>) -> Vec<SidebarItem> {
    Vec::new()
}

#[cfg(target_os = "linux")]
fn mounted_device_items(home: &Path, pinned_paths: &HashSet<PathBuf>) -> Vec<SidebarItem> {
    let mounts_content = match fs::read_to_string("/proc/mounts") {
        Ok(content) => content,
        Err(_) => return Vec::new(),
    };
    let mounts = parse_linux_mounts(&mounts_content);
    let labels = linux_device_labels();
    let removable = linux_removable_devices(&mounts);
    linux_device_items_from_mounts(&mounts, home, &labels, &removable, pinned_paths)
}

#[cfg(target_os = "linux")]
#[derive(Clone, Debug)]
struct LinuxMount {
    source: String,
    mount_point: PathBuf,
    fstype: String,
}

#[cfg(target_os = "linux")]
fn parse_linux_mounts(content: &str) -> Vec<LinuxMount> {
    let mut mounts = Vec::new();
    for line in content.lines() {
        let mut fields = line.split_whitespace();
        let Some(source) = fields.next() else {
            continue;
        };
        let Some(mount_point) = fields.next() else {
            continue;
        };
        let Some(fstype) = fields.next() else {
            continue;
        };
        mounts.push(LinuxMount {
            source: unmangle_proc_mount_field(source),
            mount_point: PathBuf::from(unmangle_proc_mount_field(mount_point)),
            fstype: unmangle_proc_mount_field(fstype),
        });
    }
    mounts
}

#[cfg(target_os = "linux")]
fn linux_device_items_from_mounts(
    mounts: &[LinuxMount],
    home: &Path,
    labels: &HashMap<PathBuf, String>,
    removable: &HashMap<String, bool>,
    pinned_paths: &HashSet<PathBuf>,
) -> Vec<SidebarItem> {
    use super::sort::natural_cmp;

    let mut seen_mount_points = HashSet::new();
    let mut items = Vec::new();

    for mount in mounts {
        let removable = linux_mount_removable(mount, removable);
        if !linux_mount_should_appear(mount, home, pinned_paths, removable) {
            continue;
        }
        if !seen_mount_points.insert(mount.mount_point.clone()) {
            continue;
        }

        items.push(SidebarItem::new(
            SidebarItemKind::Device { removable },
            linux_mount_title(mount, labels),
            if removable { "󰕓" } else { "󰋊" },
            mount.mount_point.clone(),
        ));
    }

    items.sort_by(|left, right| {
        let left_key = left.title.to_ascii_lowercase();
        let right_key = right.title.to_ascii_lowercase();
        natural_cmp(&left_key, &right_key).then_with(|| left.path.cmp(&right.path))
    });

    items
}

#[cfg(target_os = "linux")]
fn linux_mount_should_appear(
    mount: &LinuxMount,
    home: &Path,
    pinned_paths: &HashSet<PathBuf>,
    removable: bool,
) -> bool {
    if pinned_paths.contains(&mount.mount_point) || mount.mount_point == Path::new("/") {
        return false;
    }
    if linux_system_mount_type(&mount.fstype) || linux_hidden_mount_path(&mount.mount_point) {
        return false;
    }
    linux_user_visible_mount_path(&mount.mount_point, home)
        || linux_top_level_user_mount_path(&mount.mount_point)
        || removable
}

#[cfg(target_os = "linux")]
fn linux_system_mount_type(fstype: &str) -> bool {
    matches!(
        fstype,
        "autofs"
            | "aufs"
            | "binfmt_misc"
            | "bpf"
            | "cgroup"
            | "cgroup2"
            | "configfs"
            | "debugfs"
            | "devpts"
            | "devtmpfs"
            | "efivarfs"
            | "fuse.gvfsd-fuse"
            | "fuse.portal"
            | "fusectl"
            | "hugetlbfs"
            | "mqueue"
            | "nsfs"
            | "overlay"
            | "proc"
            | "pstore"
            | "ramfs"
            | "rpc_pipefs"
            | "securityfs"
            | "squashfs"
            | "sysfs"
            | "tmpfs"
            | "tracefs"
    )
}

#[cfg(target_os = "linux")]
fn linux_hidden_mount_path(path: &Path) -> bool {
    if path.starts_with("/run/media") {
        return false;
    }

    path.starts_with("/proc")
        || path.starts_with("/sys")
        || path.starts_with("/dev")
        || path.starts_with("/run")
        || path.starts_with("/snap")
        || path.starts_with("/var/lib")
}

#[cfg(target_os = "linux")]
fn linux_user_visible_mount_path(path: &Path, home: &Path) -> bool {
    path.starts_with(home)
        || path.starts_with("/media")
        || path.starts_with("/run/media")
        || path.starts_with("/mnt")
        || path.starts_with("/Volumes")
}

#[cfg(target_os = "linux")]
fn linux_top_level_user_mount_path(path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix("/") else {
        return false;
    };
    let mut components = relative.components();
    let Some(first) = components.next() else {
        return false;
    };
    if components.next().is_some() {
        return false;
    }
    let Some(name) = first.as_os_str().to_str() else {
        return false;
    };
    !matches!(
        name,
        "bin"
            | "boot"
            | "dev"
            | "etc"
            | "home"
            | "lib"
            | "lib32"
            | "lib64"
            | "lost+found"
            | "nix"
            | "opt"
            | "proc"
            | "root"
            | "run"
            | "sbin"
            | "snap"
            | "srv"
            | "sys"
            | "tmp"
            | "usr"
            | "var"
    )
}

#[cfg(target_os = "linux")]
fn linux_mount_title(mount: &LinuxMount, labels: &HashMap<PathBuf, String>) -> String {
    for key in linux_device_lookup_keys(&mount.source) {
        if let Some(label) = labels.get(&key)
            && !label.is_empty()
        {
            return label.clone();
        }
    }

    mount
        .mount_point
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            Path::new(&mount.source)
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| mount.mount_point.display().to_string())
}

#[cfg(target_os = "linux")]
fn linux_device_lookup_keys(source: &str) -> Vec<PathBuf> {
    let raw = PathBuf::from(source);
    let mut keys = Vec::new();
    if source.starts_with("/dev/")
        && let Ok(canonical) = fs::canonicalize(&raw)
    {
        keys.push(canonical);
    }
    keys.push(raw);
    keys
}

#[cfg(target_os = "linux")]
fn linux_device_labels() -> HashMap<PathBuf, String> {
    let mut labels = HashMap::new();
    let Ok(entries) = fs::read_dir("/dev/disk/by-label") else {
        return labels;
    };

    for entry in entries.flatten() {
        let label = decode_linux_label_name(&entry.file_name());
        if label.is_empty() {
            continue;
        }
        let Ok(target) = fs::canonicalize(entry.path()) else {
            continue;
        };
        labels.entry(target).or_insert(label);
    }

    labels
}

#[cfg(target_os = "linux")]
fn decode_linux_label_name(label: &std::ffi::OsStr) -> String {
    use std::os::unix::ffi::OsStrExt;

    let bytes = label.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'\\'
            && index + 3 < bytes.len()
            && bytes[index + 1] == b'x'
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 2]), hex_value(bytes[index + 3]))
        {
            decoded.push((high << 4) | low);
            index += 4;
            continue;
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&decoded).into_owned()
}

#[cfg(target_os = "linux")]
fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn linux_removable_devices(mounts: &[LinuxMount]) -> HashMap<String, bool> {
    let mut removable = HashMap::new();
    for mount in mounts {
        let Some(device_name) = linux_source_device_name(&mount.source) else {
            continue;
        };
        let Some(base_name) = linux_base_block_device_name(device_name) else {
            continue;
        };
        removable
            .entry(base_name.to_string())
            .or_insert_with(|| linux_block_device_is_removable(base_name));
    }
    removable
}

#[cfg(target_os = "linux")]
fn linux_mount_removable(mount: &LinuxMount, removable: &HashMap<String, bool>) -> bool {
    linux_source_device_name(&mount.source)
        .and_then(linux_base_block_device_name)
        .and_then(|name| removable.get(name).copied())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn linux_source_device_name(source: &str) -> Option<&str> {
    if !source.starts_with("/dev/") {
        return None;
    }
    Path::new(source).file_name()?.to_str()
}

#[cfg(target_os = "linux")]
fn linux_base_block_device_name(device_name: &str) -> Option<&str> {
    if device_name.len() < 3 {
        return None;
    }
    if device_name.starts_with("sd")
        || device_name.starts_with("hd")
        || device_name.starts_with("vd")
    {
        return Some(&device_name[..3]);
    }
    if device_name.starts_with("nvme") || device_name.starts_with("mmcblk") {
        return Some(
            device_name
                .split_once('p')
                .map_or(device_name, |(base, _)| base),
        );
    }
    if device_name.starts_with("loop") {
        return Some(device_name);
    }
    None
}

#[cfg(target_os = "linux")]
fn linux_block_device_is_removable(base_name: &str) -> bool {
    matches!(
        fs::read_to_string(format!("/sys/block/{base_name}/removable")),
        Ok(value) if value.trim() == "1"
    )
}

#[cfg(target_os = "linux")]
fn unmangle_proc_mount_field(value: &str) -> String {
    let mut value = value.to_string();
    for (from, to) in [
        (r"\011", "\t"),
        (r"\012", "\n"),
        (r"\040", " "),
        (r"\043", "#"),
        (r"\134", r"\"),
    ] {
        value = value.replace(from, to);
    }
    value
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn linux_device_items_filter_system_mounts_and_keep_user_visible_volumes() {
        let mounts = parse_linux_mounts(
            "proc /proc proc rw 0 0\n\
             tmpfs /run tmpfs rw 0 0\n\
             /dev/sda1 /boot ext4 rw 0 0\n\
             /dev/sdb1 /run/media/regueiro/My\\040USB exfat rw 0 0\n\
             /dev/sdc1 /home/regueiro/mnt/photos ext4 rw 0 0\n\
             server:/share /run/user/1000/gvfs fuse.gvfsd-fuse rw 0 0\n",
        );
        let home = Path::new("/home/regueiro");
        let pinned_paths = HashSet::from([home.to_path_buf(), PathBuf::from("/")]);
        let labels = HashMap::from([(PathBuf::from("/dev/sdb1"), "Vacation".to_string())]);
        let removable = HashMap::from([("sdb".to_string(), true), ("sdc".to_string(), false)]);

        let items =
            linux_device_items_from_mounts(&mounts, home, &labels, &removable, &pinned_paths);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "photos");
        assert_eq!(items[0].path, PathBuf::from("/home/regueiro/mnt/photos"));
        assert_eq!(items[1].title, "Vacation");
        assert_eq!(items[1].path, PathBuf::from("/run/media/regueiro/My USB"));
        assert_eq!(items[1].kind, SidebarItemKind::Device { removable: true });
    }

    #[test]
    fn linux_device_items_keep_custom_top_level_mounts_but_skip_system_roots() {
        let mounts = parse_linux_mounts(
            "/dev/sda2 /home ext4 rw 0 0\n\
             /dev/sda3 /var ext4 rw 0 0\n\
             /dev/sdb1 /data ext4 rw 0 0\n\
             /dev/loop0 /snap/core squashfs ro 0 0\n",
        );
        let home = Path::new("/home/regueiro");
        let pinned_paths = HashSet::from([home.to_path_buf(), PathBuf::from("/")]);
        let removable = HashMap::from([
            ("sda".to_string(), false),
            ("sdb".to_string(), false),
            ("loop0".to_string(), false),
        ]);

        let items = linux_device_items_from_mounts(
            &mounts,
            home,
            &HashMap::new(),
            &removable,
            &pinned_paths,
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "data");
        assert_eq!(items[0].path, PathBuf::from("/data"));
    }

    #[test]
    fn decode_linux_label_name_unescapes_hex_sequences() {
        let decoded = decode_linux_label_name(OsStr::new("New\\x20vol\\x23A"));
        assert_eq!(decoded, "New vol#A");
    }
}
