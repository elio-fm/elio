use crate::app::{SidebarItem, SidebarItemKind, SidebarRow};
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
};

pub(crate) fn build_sidebar_rows() -> Vec<SidebarRow> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));
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

    let xdg_dirs = read_xdg_user_dirs(home);

    for (kind, title, xdg_key, fallback, icon) in [
        (
            SidebarItemKind::Desktop,
            "Desktop",
            "XDG_DESKTOP_DIR",
            "Desktop",
            "󰍹",
        ),
        (
            SidebarItemKind::Documents,
            "Documents",
            "XDG_DOCUMENTS_DIR",
            "Documents",
            "󰲃",
        ),
        (
            SidebarItemKind::Downloads,
            "Downloads",
            "XDG_DOWNLOAD_DIR",
            "Downloads",
            "󰉍",
        ),
        (
            SidebarItemKind::Pictures,
            "Pictures",
            "XDG_PICTURES_DIR",
            "Pictures",
            "󰉏",
        ),
        (
            SidebarItemKind::Music,
            "Music",
            "XDG_MUSIC_DIR",
            "Music",
            "󱍙",
        ),
        (
            SidebarItemKind::Videos,
            "Videos",
            "XDG_VIDEOS_DIR",
            "Videos",
            "󰕧",
        ),
    ] {
        let path = xdg_dirs
            .get(xdg_key)
            .cloned()
            .unwrap_or_else(|| home.join(fallback));
        if path.exists() {
            items.push(SidebarItem::new(kind, title, icon, path));
        }
    }

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

/// Reads `$XDG_CONFIG_HOME/user-dirs.dirs` (default: `~/.config/user-dirs.dirs`) and returns a
/// map of XDG variable names (e.g. `"XDG_DOWNLOAD_DIR"`) to their resolved paths. On systems
/// without this file (e.g. macOS) or when the file cannot be read, returns an empty map and the
/// caller falls back to English directory names.
fn read_xdg_user_dirs(home: &Path) -> HashMap<String, PathBuf> {
    let config_home = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));

    let content = match fs::read_to_string(config_home.join("user-dirs.dirs")) {
        Ok(content) => content,
        Err(_) => return HashMap::new(),
    };

    let mut dirs = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        let path = if value == "$HOME" {
            home.to_path_buf()
        } else if let Some(relative) = value.strip_prefix("$HOME/") {
            home.join(relative)
        } else if value.starts_with('/') {
            PathBuf::from(value)
        } else {
            continue;
        };
        dirs.insert(key.trim().to_string(), path);
    }
    dirs
}

/// Returns the path to the user's trash directory, or `None` if it cannot be determined.
///
/// On freedesktop systems (Linux) the trash lives at `$XDG_DATA_HOME/Trash/files`.
/// On macOS it is `~/.Trash`. The `files` subdirectory is used on Linux because that is
/// where the actual deleted items are stored (the sibling `info/` directory holds metadata).
pub(crate) fn trash_dir(home: &Path) -> Option<PathBuf> {
    let data_home = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    let xdg_trash = data_home.join("Trash/files");
    if xdg_trash.exists() {
        return Some(xdg_trash);
    }

    let mac_trash = home.join(".Trash");
    if mac_trash.exists() {
        return Some(mac_trash);
    }

    None
}

#[cfg(not(target_os = "linux"))]
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
