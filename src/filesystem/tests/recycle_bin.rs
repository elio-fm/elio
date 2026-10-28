use super::*;

/// Builds a synthetic `$I` sidecar body for the given version and path.
fn info_bytes(version: u64, size: u64, filetime: u64, path: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&filetime.to_le_bytes());

    let mut units: Vec<u16> = path.encode_utf16().collect();
    units.push(0); // NUL terminator

    match version {
        1 => {
            units.resize(V1_PATH_CODE_UNITS, 0);
        }
        _ => {
            out.extend_from_slice(&(units.len() as u32).to_le_bytes());
        }
    }
    for unit in units {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out
}

// 2024-03-15T10:30:00Z as a FILETIME.
const SAMPLE_FILETIME: u64 = (1_710_498_600 + FILETIME_TO_UNIX_SECS) * 10_000_000;

#[test]
fn parses_version_2_sidecar() {
    let bytes = info_bytes(
        2,
        4096,
        SAMPLE_FILETIME,
        r"C:\Users\brett\Reports\report.pdf",
    );
    let info = parse_info(&bytes).expect("v2 sidecar should parse");

    assert_eq!(
        info.original_path,
        PathBuf::from(r"C:\Users\brett\Reports\report.pdf")
    );
    assert_eq!(info.original_size, 4096);
    assert_eq!(info.original_name().as_deref(), Some("report.pdf"));
    assert_eq!(
        info.deleted_at,
        Some(UNIX_EPOCH + Duration::from_secs(1_710_498_600))
    );
}

#[test]
fn parses_version_1_fixed_width_sidecar() {
    let bytes = info_bytes(1, 12, SAMPLE_FILETIME, r"D:\data\notes.txt");
    let info = parse_info(&bytes).expect("v1 sidecar should parse");

    assert_eq!(info.original_path, PathBuf::from(r"D:\data\notes.txt"));
    assert_eq!(info.original_name().as_deref(), Some("notes.txt"));
    assert_eq!(info.original_size, 12);
}

#[test]
fn parses_paths_with_non_ascii_characters() {
    let bytes = info_bytes(2, 1, SAMPLE_FILETIME, r"C:\Users\brett\Résumé — final.docx");
    let info = parse_info(&bytes).expect("sidecar should parse");

    assert_eq!(info.original_name().as_deref(), Some("Résumé — final.docx"));
}

#[test]
fn rejects_truncated_and_unknown_version_sidecars() {
    assert_eq!(parse_info(&[]), None, "empty buffer");
    assert_eq!(parse_info(&[0u8; 20]), None, "short header");

    let unknown = info_bytes(7, 1, SAMPLE_FILETIME, r"C:\x.txt");
    assert_eq!(parse_info(&unknown), None, "unknown version");

    // Version 2 claiming more code units than are present.
    let mut lying = info_bytes(2, 1, SAMPLE_FILETIME, r"C:\x.txt");
    lying[24..28].copy_from_slice(&9999u32.to_le_bytes());
    assert_eq!(parse_info(&lying), None, "truncated path buffer");
}

#[test]
fn rejects_sidecar_with_empty_path() {
    let bytes = info_bytes(2, 0, SAMPLE_FILETIME, "");
    assert_eq!(parse_info(&bytes), None);
}

#[test]
fn tolerates_filetime_before_unix_epoch() {
    let bytes = info_bytes(2, 1, 0, r"C:\x.txt");
    let info = parse_info(&bytes).expect("path should still parse");
    assert_eq!(info.deleted_at, None, "unrepresentable time is dropped");
    assert_eq!(info.original_name().as_deref(), Some("x.txt"));
}

#[test]
fn maps_content_paths_to_their_info_sidecars() {
    let bin = Path::new(r"C:\$Recycle.Bin\S-1-5-21-1");

    assert_eq!(
        info_path_for(&bin.join("$R0A5E05.txt")),
        Some(bin.join("$I0A5E05.txt"))
    );
    // Extensionless entries (typically directories) pair the same way.
    assert_eq!(
        info_path_for(&bin.join("$R0A5E05")),
        Some(bin.join("$I0A5E05"))
    );
    // Anything that is not a $R entry has no sidecar.
    assert_eq!(info_path_for(&bin.join("desktop.ini")), None);
    assert_eq!(info_path_for(&bin.join("$I0A5E05.txt")), None);
}

#[test]
fn only_treats_dollar_r_entries_directly_in_the_bin_as_recycle_bin_entries() {
    let bin = Path::new(r"C:\$Recycle.Bin\S-1-5-21-1");

    assert!(is_recycle_bin_entry_in(&bin.join("$R0A5E05.txt"), bin));
    assert!(
        is_recycle_bin_entry_in(&bin.join("$r0a5e05"), bin),
        "case-insensitive"
    );

    // Sidecars and stray files in the bin are not restorable content.
    assert!(!is_recycle_bin_entry_in(&bin.join("$I0A5E05.txt"), bin));
    assert!(!is_recycle_bin_entry_in(&bin.join("desktop.ini"), bin));
    // A $R-looking name elsewhere on disk must not qualify, or permanently
    // deleting it would also delete an unrelated `$I…` neighbour.
    assert!(!is_recycle_bin_entry_in(
        Path::new(r"C:\elsewhere\$R0A5E05.txt"),
        bin
    ));
    // Nested paths inside a recycled directory are not top-level entries.
    assert!(!is_recycle_bin_entry_in(
        &bin.join("$R0A5E05").join("$RNested.txt"),
        bin
    ));
}

#[test]
fn hides_info_sidecars_and_desktop_ini_from_the_listing() {
    assert!(is_hidden_in_listing("$I0A5E05.txt"));
    assert!(is_hidden_in_listing("$i0a5e05"), "case-insensitive");
    assert!(is_hidden_in_listing("desktop.ini"));
    assert!(is_hidden_in_listing("Desktop.ini"), "case-insensitive");
    assert!(!is_hidden_in_listing("$R0A5E05.txt"));
    assert!(!is_hidden_in_listing("$"));
    assert!(!is_hidden_in_listing(""));
}

#[test]
fn original_name_splits_on_either_separator() {
    let info = |path: &str| RecycleInfo {
        original_path: PathBuf::from(path),
        original_size: 0,
        deleted_at: None,
    };

    assert_eq!(
        info(r"C:\a\b.txt").original_name().as_deref(),
        Some("b.txt")
    );
    assert_eq!(info("C:/a/b.txt").original_name().as_deref(), Some("b.txt"));
    assert_eq!(info(r"C:\a\").original_name(), None);
}

/// NTFS names may hold unpaired surrogates; they must survive into the path
/// restore moves the item back to.
#[test]
#[cfg(windows)]
fn keeps_unpaired_surrogates_in_the_original_path() {
    use std::os::windows::ffi::OsStrExt;

    let mut bytes = info_bytes(2, 1, SAMPLE_FILETIME, r"C:\x");
    // Replace the NUL terminator with a lone high surrogate, then terminate.
    let count = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) + 1;
    bytes[24..28].copy_from_slice(&count.to_le_bytes());
    bytes.truncate(bytes.len() - 2);
    bytes.extend_from_slice(&0xD800u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());

    let info = parse_info(&bytes).expect("sidecar should parse");
    let wide: Vec<u16> = info.original_path.as_os_str().encode_wide().collect();
    assert_eq!(wide.last(), Some(&0xD800));
}

/// End-to-end check against the real Recycle Bin: trash a file, find it via
/// its `$I` sidecar, restore it, and confirm both halves of the pair are gone.
///
/// Ignored by default because it mutates the machine's actual Recycle Bin and
/// depends on a writable `%TEMP%` on the system drive. Run explicitly with:
/// `cargo test --lib recycle_bin -- --ignored --nocapture`
#[test]
#[ignore = "mutates the real Recycle Bin"]
#[cfg(windows)]
fn roundtrips_a_real_file_through_the_recycle_bin() {
    // `std::env::temp_dir()` yields the 8.3 short path while Windows records
    // the long path in the sidecar, so match on the base name and take the
    // most recently deleted candidate rather than comparing full paths.
    let dir = std::env::temp_dir().join("elio-recycle-roundtrip");
    let _ = fs::create_dir_all(&dir);
    let name = "round trip \u{2014} caf\u{e9}.txt";
    let file = dir.join(name);
    fs::write(&file, b"payload").expect("write the fixture");

    trash::delete(&file).expect("send to recycle bin");
    assert!(!file.exists(), "file should be gone after trashing");

    let bin = recycle_bin_dir().expect("recycle bin should resolve");
    let (entry, info) = fs::read_dir(&bin)
        .expect("list recycle bin")
        .flatten()
        .map(|e| e.path())
        .filter_map(|p| read_info(&p).map(|i| (p, i)))
        .filter(|(_, i)| i.original_name().as_deref() == Some(name))
        .max_by_key(|(_, i)| i.deleted_at)
        .expect("the trashed item should be findable via its $I sidecar");

    assert_eq!(info.original_size, 7, "payload is 7 bytes");
    assert!(info.deleted_at.is_some(), "deletion time should parse");
    assert!(
        info.original_path.ends_with(name),
        "sidecar should record the full original path"
    );

    let sidecar = info_path_for(&entry).expect("sidecar path");
    assert!(sidecar.exists(), "sidecar should exist before restore");

    crate::filesystem::restore_trash_item(&entry).expect("restore should succeed");

    assert!(
        info.original_path.exists(),
        "file should be back at its original path"
    );
    assert_eq!(
        fs::read(&info.original_path).expect("read restored"),
        b"payload"
    );
    assert!(!entry.exists(), "$R content should be gone from the bin");
    assert!(!sidecar.exists(), "$I sidecar should be cleaned up");

    let _ = fs::remove_dir_all(&dir);
}

/// The listing hook hides `$I` sidecars and resolves every `$R` entry to its
/// original name. Ignored by default: it reads the machine's real Recycle Bin.
#[test]
#[ignore = "reads the real Recycle Bin"]
#[cfg(windows)]
fn lists_the_real_recycle_bin_with_original_names() {
    let Some(dir) = recycle_bin_dir() else {
        eprintln!("no recycle bin on this machine; nothing to check");
        return;
    };
    // Navigation loads the canonicalized path; it must still be recognized.
    let navigated = dir.canonicalize().expect("recycle bin should canonicalize");
    assert!(is_recycle_bin_dir(&navigated));

    let snapshot = crate::filesystem::load_directory_snapshot(
        &navigated,
        false,
        crate::filesystem::SortMode::Name,
        true,
    )
    .expect("recycle bin should list");

    assert!(
        !snapshot
            .entries
            .iter()
            .any(|e| is_hidden_in_listing(&e.name)),
        "$I sidecars and desktop.ini must never be shown as entries"
    );
    assert!(
        !snapshot.entries.iter().any(|e| e.name.starts_with("$R")),
        "every $R entry should be resolved to its original name"
    );
}
