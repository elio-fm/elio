use super::super::{PlaceKind, linux_devices::*};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    path::{Path, PathBuf},
};

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

    let items = linux_device_items_from_mounts(&mounts, home, &labels, &removable, &pinned_paths);

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].title, "photos");
    assert_eq!(items[0].path, PathBuf::from("/home/regueiro/mnt/photos"));
    assert_eq!(items[1].title, "Vacation");
    assert_eq!(items[1].path, PathBuf::from("/run/media/regueiro/My USB"));
    assert_eq!(items[1].kind, PlaceKind::Device { removable: true });
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

    let items =
        linux_device_items_from_mounts(&mounts, home, &HashMap::new(), &removable, &pinned_paths);

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "data");
    assert_eq!(items[0].path, PathBuf::from("/data"));
}

#[test]
fn decode_linux_label_name_unescapes_hex_sequences() {
    let decoded = decode_linux_label_name(OsStr::new("New\\x20vol\\x23A"));
    assert_eq!(decoded, "New vol#A");
}
