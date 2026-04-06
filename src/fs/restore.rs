use anyhow::Context;
use std::{fs, path::{Path, PathBuf}};

// ---------------------------------------------------------------------------
// Restore from trash
// ---------------------------------------------------------------------------

/// Restores a trashed item to its original location.
///
/// Two backends are supported:
///
/// - **FreeDesktop trash** (Linux, BSD, and any macOS installation that uses
///   XDG tools): `entry_path` must be inside a `Trash/files/` directory and a
///   sibling `Trash/info/<name>.trashinfo` file must exist.  The original path
///   is read from that file and the item is moved back.
///
/// - **macOS `~/.Trash`**: Finder records the original location internally
///   when an item is trashed.  The `osascript` "put back" command asks Finder
///   to use that metadata and move the item back — exactly what the Finder
///   "Put Back" menu item does.
///
/// The FreeDesktop path is tried first (it works even on macOS if the XDG
/// layout happens to be present), then the macOS path, then an unsupported
/// error for any other layout (e.g. Windows Recycle Bin).
pub(crate) fn restore_trash_item(entry_path: &Path) -> anyhow::Result<()> {
    // FreeDesktop trash layout: the entry lives inside a `files/` directory,
    // and a sibling `info/` directory holds the `.trashinfo` metadata.
    //
    // Both conditions are required.  Checking only for `info/` two levels up
    // is insufficient: on macOS, `~/.Trash/foo` would compute `~/info`, and
    // if the user happens to have a `~/info` directory for any reason the
    // function would take the FreeDesktop path and fail to find a `.trashinfo`
    // instead of correctly falling through to the Finder backend.
    let parent = entry_path.parent();
    let in_files_dir = parent
        .and_then(|p| p.file_name())
        .is_some_and(|name| name == "files");
    let info_dir = parent
        .and_then(|p| p.parent())
        .map(|trash_root| trash_root.join("info"));

    if in_files_dir && info_dir.as_deref().is_some_and(|d| d.is_dir()) {
        return restore_trash_item_freedesktop(entry_path, info_dir.unwrap());
    }

    // macOS: no .trashinfo metadata, but Finder tracks the original location
    // internally.  Ask it to "put back" the item via osascript.
    #[cfg(target_os = "macos")]
    return restore_trash_item_macos(entry_path);

    // Any other layout (e.g. Windows Recycle Bin) is not supported.
    #[cfg(not(target_os = "macos"))]
    anyhow::bail!("restore is not supported for this trash location")
}

/// FreeDesktop-specific restore: reads the `.trashinfo` sidecar and moves the
/// item back to its original path.
fn restore_trash_item_freedesktop(
    entry_path: &Path,
    info_dir: PathBuf,
) -> anyhow::Result<()> {
    let name = entry_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("cannot determine file name for {:?}", entry_path))?;

    let info_path = info_dir.join(format!("{name}.trashinfo"));
    let content =
        fs::read_to_string(&info_path).with_context(|| format!("cannot read {:?}", info_path))?;

    let original = parse_trashinfo_original_path(&content)
        .ok_or_else(|| anyhow::anyhow!("cannot parse original path from {:?}", info_path))?;

    if original.exists() {
        anyhow::bail!("destination already exists: {:?}", original);
    }

    if let Some(parent) = original.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create parent dir {:?}", parent))?;
    }

    fs::rename(entry_path, &original)
        .with_context(|| format!("cannot move {:?} to {:?}", entry_path, original))?;

    let _ = fs::remove_file(&info_path);

    Ok(())
}

// ---------------------------------------------------------------------------
// macOS restore-origins store
// ---------------------------------------------------------------------------
// Elio trashes files via the `trash` crate, which calls
// NSWorkspace.recycleURLs.  That API stores the original path in a private
// system database that Finder reads for "Put Back" — it does NOT reliably
// write ptbL/ptbN records to ~/.Trash/.DS_Store the way Finder's own drag-
// to-trash action does.  Parsing .DS_Store therefore fails for any file Elio
// trashed, even though Finder's own "Put Back" works fine for those files.
//
// To work around this, whenever Elio trashes a file it immediately records
// the original path in its own JSON store at
//   ~/Library/Application Support/elio/trash-origins.json
// keyed by the expected filename in ~/.Trash.  Restore checks this store
// first.  The DS_Store parser is kept as a fallback for files trashed
// directly by Finder (which does write ptbL).

/// Returns the path to the restore-origins metadata store.
#[cfg(target_os = "macos")]
fn restore_origins_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("elio").join("trash-origins.json"))
}

/// Records `(trash_name, original_path)` pairs in the restore-origins store.
/// `trash_name` is the filename as it will appear in `~/.Trash` (= the
/// original filename when there is no collision).  Best-effort: silently
/// ignored on any I/O error.
#[cfg(target_os = "macos")]
pub(crate) fn save_restore_origins(items: &[(String, PathBuf)]) {
    let Some(path) = restore_origins_path() else {
        return;
    };
    let mut map: std::collections::HashMap<String, String> = fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();

    for (name, original) in items {
        if let Some(s) = original.to_str() {
            map.insert(name.clone(), s.to_owned());
        }
    }

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_vec_pretty(&map) {
        let _ = fs::write(&path, json);
    }
}

/// Removes entries for the given `trash_names` from the restore-origins store.
/// For each name, first tries an exact key match, then strips any macOS
/// collision suffix (` 2`, ` 3`, …) and tries again — so "foo 2.txt"
/// correctly removes the "foo.txt" key that was saved at trash time.
/// Best-effort: silently ignores any I/O error.
#[cfg(target_os = "macos")]
pub(crate) fn remove_restore_origins(trash_names: &[&str]) {
    let Some(path) = restore_origins_path() else {
        return;
    };
    let bytes = match fs::read(&path) {
        Ok(b) => b,
        Err(_) => return,
    };
    let mut map: std::collections::HashMap<String, String> =
        match serde_json::from_slice(&bytes) {
            Ok(m) => m,
            Err(_) => return,
        };
    if remove_from_origins_map(&mut map, trash_names) {
        if let Ok(json) = serde_json::to_vec_pretty(&map) {
            let _ = fs::write(&path, json);
        }
    }
}

/// Core map-mutation logic for [`remove_restore_origins`]: removes each name
/// in `trash_names` from `map`, trying the exact key first then the
/// collision-stripped base name.  Returns `true` if the map was modified.
#[cfg(target_os = "macos")]
fn remove_from_origins_map(
    map: &mut std::collections::HashMap<String, String>,
    trash_names: &[&str],
) -> bool {
    let mut changed = false;
    for &name in trash_names {
        if map.remove(name).is_some() {
            changed = true;
            continue;
        }
        // Collision case: the file was stored under its original name (e.g.
        // "foo.txt") but appears in trash as "foo 2.txt".  Strip the suffix
        // and try again.
        let p = Path::new(name);
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            if let Some(base_stem) = strip_macos_collision_suffix(stem) {
                let ext = p.extension().and_then(|e| e.to_str());
                let base_name = match ext {
                    Some(e) => format!("{base_stem}.{e}"),
                    None => base_stem.to_owned(),
                };
                if map.remove(&base_name).is_some() {
                    changed = true;
                }
            }
        }
    }
    changed
}

/// Looks up the original path for a file currently named `trash_name`.
/// Also tries stripping macOS collision suffixes (` 2`, ` 3`, …) from the
/// stem, so files renamed on collision can still be matched.
#[cfg(target_os = "macos")]
fn load_restore_origin(trash_name: &str) -> Option<PathBuf> {
    let path = restore_origins_path()?;
    let map: std::collections::HashMap<String, String> =
        serde_json::from_slice(&fs::read(&path).ok()?).ok()?;

    if let Some(orig) = map.get(trash_name) {
        return Some(PathBuf::from(orig));
    }

    // Collision case: "foo 2.txt" → look up "foo.txt".
    let p = Path::new(trash_name);
    let stem = p.file_stem().and_then(|s| s.to_str())?;
    let ext = p.extension().and_then(|e| e.to_str());
    let base_stem = strip_macos_collision_suffix(stem)?;
    let base_name = match ext {
        Some(e) => format!("{base_stem}.{e}"),
        None => base_stem.to_owned(),
    };
    map.get(&base_name).map(|s| PathBuf::from(s))
}

/// Strips a macOS collision suffix (` 2`, ` 3`, …) from a file stem.
/// Returns `Some(base)` if a suffix was stripped, `None` otherwise.
#[cfg(target_os = "macos")]
fn strip_macos_collision_suffix(stem: &str) -> Option<&str> {
    let (base, suffix) = stem.rsplit_once(' ')?;
    let n: u64 = suffix.parse().ok()?;
    (n >= 2).then_some(base)
}

/// Moves `entry_path` to `original_path`, creating parent directories as
/// needed.  Shared by both restore paths (our store and DS_Store fallback).
#[cfg(target_os = "macos")]
fn perform_restore(entry_path: &Path, original_path: &Path) -> anyhow::Result<()> {
    if original_path.exists() {
        anyhow::bail!("destination already exists: {:?}", original_path);
    }
    if let Some(parent) = original_path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create parent dir {:?}", parent))?;
    }
    fs::rename(entry_path, original_path)
        .with_context(|| format!("cannot move {:?} to {:?}", entry_path, original_path))
}

/// macOS-specific restore.  Checks the Elio restore-origins store first
/// (populated whenever Elio trashes a file), then falls back to parsing
/// `.DS_Store` for files trashed directly by Finder.
#[cfg(target_os = "macos")]
fn restore_trash_item_macos(entry_path: &Path) -> anyhow::Result<()> {
    let file_name = entry_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("cannot determine file name for {:?}", entry_path))?;
    let trash_dir = entry_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("cannot determine trash dir for {:?}", entry_path))?;
    let ds_store_path = trash_dir.join(".DS_Store");

    // Guard: never treat the metadata file itself as the item to restore.
    if entry_path == ds_store_path {
        anyhow::bail!("cannot restore \".DS_Store\" — it is a system metadata file");
    }

    // ── Primary: our own restore-origins store ──────────────────────────────
    if let Some(original_path) = load_restore_origin(file_name) {
        return perform_restore(entry_path, &original_path);
    }

    // ── Fallback: parse .DS_Store written by Finder ─────────────────────────
    if !ds_store_path.exists() {
        anyhow::bail!(
            "no Put Back metadata found for \"{file_name}\" \
             (the file was not trashed via Finder or Elio)"
        );
    }

    let data = fs::read(&ds_store_path)
        .with_context(|| format!("cannot read {:?}", ds_store_path))?;

    let (parent_dir, original_name) =
        macos_ds_store_find_ptb(&data, file_name).ok_or_else(|| {
            anyhow::anyhow!(
                "no Put Back metadata found for \"{file_name}\" \
                 (the file was not trashed via Finder or Elio)"
            )
        })?;

    // ptbL stores a volume-relative path without a leading slash.
    let original_path = if parent_dir.is_empty() {
        PathBuf::from(format!("/{original_name}"))
    } else {
        PathBuf::from(format!("/{parent_dir}/{original_name}"))
    };

    perform_restore(entry_path, &original_path)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// macOS DS_Store parser
// ---------------------------------------------------------------------------
// When Finder moves a file to the Trash it writes `ptbL` (original parent
// directory, volume-relative, no leading slash) and `ptbN` (original file
// name, when renamed on collision) into the `.DS_Store` file in `~/.Trash`.
// These are the same records that Finder's "Put Back" command consults.
//
// DS_Store uses a buddy allocator to store a B-tree of
// (filename, property, type, value) records.  We parse just enough to locate
// ptbL/ptbN for the target filename without pulling in an external dependency.
// ---------------------------------------------------------------------------

/// Searches a `.DS_Store` binary for the `ptbL` (original parent directory)
/// and `ptbN` (original file name) records associated with `file_name`.
///
/// Returns `(parent_dir, original_name)` on success, where `parent_dir` is
/// volume-relative (no leading slash).  Returns `None` if the records are not
/// found or the binary cannot be parsed.
#[cfg(target_os = "macos")]
fn macos_ds_store_find_ptb(data: &[u8], file_name: &str) -> Option<(String, String)> {
    // ── Buddy-allocator header ──────────────────────────────────────────────
    // data[0..4]  — alignment marker \x00\x00\x00\x01
    // data[4..8]  — "Bud1" magic
    // data[8..12] — info_offset (u32 BE, relative to data[4..])
    // data[12..16]— info_size   (u32 BE)
    if data.len() < 36 || &data[4..8] != b"Bud1" {
        return None;
    }
    let info_offset = u32::from_be_bytes(data[8..12].try_into().ok()?) as usize;
    let info_size = u32::from_be_bytes(data[12..16].try_into().ok()?) as usize;

    let info_start = 4usize.checked_add(info_offset)?;
    let info_end = info_start.checked_add(info_size)?;
    if info_end > data.len() || info_end < info_start + 8 {
        return None;
    }
    let info = &data[info_start..info_end];

    // ── Offset table ────────────────────────────────────────────────────────
    // info[0..4]  — num_offsets (u32 BE)
    // info[4..8]  — 0x00000000 (padding)
    // info[8..]   — num_offsets × u32 BE block addresses
    let num_offsets = u32::from_be_bytes(info[0..4].try_into().ok()?) as usize;
    let table_bytes = num_offsets.checked_mul(4)?;
    let table_end = 8usize.checked_add(table_bytes)?;
    if table_end > info.len() {
        return None;
    }
    let mut offsets = Vec::with_capacity(num_offsets);
    for i in 0..num_offsets {
        let o = 8 + i * 4;
        offsets.push(u32::from_be_bytes(info[o..o + 4].try_into().ok()?));
    }

    // Pad offset table to next 256-entry boundary.
    let pad = (256usize.wrapping_sub(num_offsets % 256)) % 256;
    let toc_start = table_end.checked_add(pad.checked_mul(4)?)?;

    // ── Table of Contents ───────────────────────────────────────────────────
    // toc[0..4]  — num_entries (u32 BE)
    // toc[4..]   — entries: name_len (u8) + name + block_id (u32 BE)
    if toc_start + 4 > info.len() {
        return None;
    }
    let num_toc =
        u32::from_be_bytes(info[toc_start..toc_start + 4].try_into().ok()?) as usize;

    let mut pos = toc_start + 4;
    let mut dsdb_block_id: Option<u32> = None;
    for _ in 0..num_toc {
        if pos >= info.len() {
            return None;
        }
        let name_len = info[pos] as usize;
        pos += 1;
        let name_end = pos.checked_add(name_len)?;
        if name_end + 4 > info.len() {
            return None;
        }
        let toc_name = std::str::from_utf8(&info[pos..name_end]).ok()?;
        let block_id =
            u32::from_be_bytes(info[name_end..name_end + 4].try_into().ok()?);
        if toc_name == "DSDB" {
            dsdb_block_id = Some(block_id);
        }
        pos = name_end + 4;
    }

    // ── DSDB block → root B-tree node ───────────────────────────────────────
    let dsdb_block = ds_store_block(data, &offsets, dsdb_block_id?)?;
    if dsdb_block.len() < 4 {
        return None;
    }
    let root_node = u32::from_be_bytes(dsdb_block[0..4].try_into().ok()?);

    // ── Traverse B-tree ─────────────────────────────────────────────────────
    let mut ptbl: Option<String> = None;
    let mut ptbn: Option<String> = None;
    let mut visited = std::collections::HashSet::new();
    ds_store_traverse(
        data,
        &offsets,
        root_node,
        file_name,
        &mut ptbl,
        &mut ptbn,
        &mut visited,
    )?;

    match (ptbl, ptbn) {
        (Some(l), Some(n)) => Some((l, n)),
        // ptbN is absent when the file name was not changed on trashing.
        (Some(l), None) => Some((l, file_name.to_owned())),
        _ => None,
    }
}

/// Returns the payload slice for the given block ID, or `None` on any error.
///
/// Block address encoding: `offset = addr & !0x1f` (absolute in `data`),
/// `size = 1 << (addr & 0x1f)`.  The 4 bytes at `data[offset..]` are a
/// block size header; the payload starts at `data[offset + 4..]`.
#[cfg(target_os = "macos")]
fn ds_store_block<'a>(data: &'a [u8], offsets: &[u32], id: u32) -> Option<&'a [u8]> {
    let addr = *offsets.get(id as usize)?;
    if addr == 0 {
        return None;
    }
    let offset = (addr & !0x1f) as usize;
    let size = 1usize << (addr & 0x1f);
    let start = offset.checked_add(4)?;
    let end = start.checked_add(size)?;
    if end > data.len() {
        return None;
    }
    Some(&data[start..end])
}

/// Recursively traverses a B-tree node, collecting `ptbL`/`ptbN` values for
/// `target_name`.  Returns `None` on any parse error.
#[cfg(target_os = "macos")]
fn ds_store_traverse(
    data: &[u8],
    offsets: &[u32],
    node_id: u32,
    target_name: &str,
    ptbl: &mut Option<String>,
    ptbn: &mut Option<String>,
    visited: &mut std::collections::HashSet<u32>,
) -> Option<()> {
    // Guard against cycles in corrupt DS_Store files — skip silently, don't abort.
    if !visited.insert(node_id) {
        return Some(());
    }

    let block = ds_store_block(data, offsets, node_id)?;
    let mut cur = DsStoreCursor::new(block);

    let pair_count = cur.read_u32()?;

    if pair_count == 0 {
        // Leaf node: record count then records.
        let record_count = cur.read_u32()?;
        for _ in 0..record_count {
            // Unknown type in a record means we can't determine its size and
            // must stop reading this node, but don't abort the whole traversal.
            if ds_store_read_record(&mut cur, target_name, ptbl, ptbn).is_none() {
                break;
            }
        }
    } else {
        // Internal node: alternating child_id and record, then one final child.
        for _ in 0..pair_count {
            let child_id = cur.read_u32()?;
            // Child failures don't corrupt our cursor — skip and continue.
            ds_store_traverse(data, offsets, child_id, target_name, ptbl, ptbn, visited);
            // Record failure means we can't find the boundary of this record,
            // so we can't safely continue reading this node.
            if ds_store_read_record(&mut cur, target_name, ptbl, ptbn).is_none() {
                return Some(());
            }
        }
        let last_child = cur.read_u32()?;
        ds_store_traverse(data, offsets, last_child, target_name, ptbl, ptbn, visited);
    }

    Some(())
}

/// Reads one B-tree record and, if it belongs to `target_name`, stores the
/// `ptbL` or `ptbN` value.  Returns `None` on any parse error.
#[cfg(target_os = "macos")]
fn ds_store_read_record(
    cur: &mut DsStoreCursor<'_>,
    target_name: &str,
    ptbl: &mut Option<String>,
    ptbn: &mut Option<String>,
) -> Option<()> {
    // Filename: u32 code-unit count + UTF-16BE data.
    let name_len = cur.read_u32()? as usize;
    let name_bytes = cur.read_bytes(name_len * 2)?;
    let name = decode_utf16be(name_bytes)?;

    // Property code and type code (4 ASCII bytes each).
    let prop4: [u8; 4] = cur.read_bytes(4)?.try_into().ok()?;
    let typ4: [u8; 4] = cur.read_bytes(4)?.try_into().ok()?;

    let is_target = name == target_name;
    let is_ptbl = prop4 == *b"ptbL";
    let is_ptbn = prop4 == *b"ptbN";

    match (&prop4, &typ4) {
        (_, b"ustr") => {
            let val_len = cur.read_u32()? as usize;
            let val_bytes = cur.read_bytes(val_len * 2)?;
            if is_target && (is_ptbl || is_ptbn) {
                let val = decode_utf16be(val_bytes)?;
                if is_ptbl {
                    *ptbl = Some(val);
                } else {
                    *ptbn = Some(val);
                }
            }
        }
        (_, b"bool") => {
            cur.skip(1)?;
        }
        (_, b"shor") => {
            cur.skip(2)?;
        }
        (_, b"long") | (_, b"type") => {
            cur.skip(4)?;
        }
        (_, b"comp") | (_, b"dutc") => {
            cur.skip(8)?;
        }
        // BKGD blob has no length prefix — it is always exactly 12 bytes.
        (b"BKGD", b"blob") => {
            cur.skip(12)?;
        }
        (_, b"blob") => {
            let len = cur.read_u32()? as usize;
            cur.skip(len)?;
        }
        _ => {
            // Unknown type — cannot determine record size, so abort traversal.
            return None;
        }
    }

    Some(())
}

/// Cursor over a `&[u8]` slice with big-endian integer reads.
#[cfg(target_os = "macos")]
struct DsStoreCursor<'a> {
    data: &'a [u8],
    pos: usize,
}

#[cfg(target_os = "macos")]
impl<'a> DsStoreCursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn skip(&mut self, n: usize) -> Option<()> {
        let end = self.pos.checked_add(n)?;
        if end > self.data.len() {
            return None;
        }
        self.pos = end;
        Some(())
    }

    fn read_u32(&mut self) -> Option<u32> {
        let b = self.read_bytes(4)?;
        Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        if end > self.data.len() {
            return None;
        }
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Some(slice)
    }
}

/// Decodes a UTF-16BE byte sequence into a `String`.
/// Returns `None` if the byte count is odd or the data is not valid UTF-16.
#[cfg(target_os = "macos")]
fn decode_utf16be(bytes: &[u8]) -> Option<String> {
    if bytes.len() % 2 != 0 {
        return None;
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16(&units).ok()
}

fn parse_trashinfo_original_path(content: &str) -> Option<PathBuf> {
    for line in content.lines() {
        if let Some(encoded) = line.trim().strip_prefix("Path=") {
            return Some(PathBuf::from(percent_decode(encoded)));
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_nibble(bytes[i + 1]), hex_nibble(bytes[i + 2]))
        {
            out.push(hi << 4 | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-{label}-{unique}"))
    }

    /// Builds a minimal FreeDesktop trash layout under `root`:
    ///   root/
    ///     files/<name>  ← the trashed item (a regular file)
    ///     info/<name>.trashinfo
    ///
    /// Returns `(trash_files_dir, trash_info_dir, item_path)`.
    #[cfg(unix)]
    fn make_freedesktop_trash(
        root: &PathBuf,
        name: &str,
        original: &Path,
    ) -> (PathBuf, PathBuf, PathBuf) {
        let files_dir = root.join("files");
        let info_dir = root.join("info");
        fs::create_dir_all(&files_dir).expect("failed to create trash files dir");
        fs::create_dir_all(&info_dir).expect("failed to create trash info dir");
        let item_path = files_dir.join(name);
        fs::write(&item_path, b"trashed content").expect("failed to write trashed item");
        let trashinfo = format!(
            "[Trash Info]\nPath={}\nDeletionDate=2024-01-01T00:00:00\n",
            original.to_str().unwrap()
        );
        fs::write(info_dir.join(format!("{name}.trashinfo")), trashinfo)
            .expect("failed to write trashinfo");
        (files_dir, info_dir, item_path)
    }

    #[test]
    #[cfg(unix)]
    fn restore_freedesktop_moves_item_to_original_path_and_removes_trashinfo() {
        let root = temp_path("restore-fd-ok");
        let restore_target = temp_path("restore-fd-ok-dest");
        fs::create_dir_all(&root).expect("failed to create trash root");
        fs::create_dir_all(&restore_target).expect("failed to create restore target dir");

        let original = restore_target.join("report.pdf");
        let (_, info_dir, item_path) =
            make_freedesktop_trash(&root, "report.pdf", &original);

        let result = restore_trash_item(&item_path);
        assert!(result.is_ok(), "restore should succeed: {:?}", result);
        assert!(original.exists(), "file should be at original location");
        assert!(!item_path.exists(), "trashed item should be gone");
        assert!(
            !info_dir.join("report.pdf.trashinfo").exists(),
            "trashinfo should be removed"
        );

        fs::remove_dir_all(&root).ok();
        fs::remove_dir_all(&restore_target).ok();
    }

    #[test]
    #[cfg(unix)]
    fn restore_freedesktop_fails_when_destination_already_exists() {
        let root = temp_path("restore-fd-conflict");
        let restore_target = temp_path("restore-fd-conflict-dest");
        fs::create_dir_all(&root).expect("failed to create trash root");
        fs::create_dir_all(&restore_target).expect("failed to create restore target dir");

        let original = restore_target.join("conflict.txt");
        fs::write(&original, b"already here").expect("failed to write blocking file");

        let (_, _, item_path) =
            make_freedesktop_trash(&root, "conflict.txt", &original);

        let err = restore_trash_item(&item_path).unwrap_err();
        assert!(
            err.to_string().contains("destination already exists"),
            "unexpected error: {err}"
        );

        fs::remove_dir_all(&root).ok();
        fs::remove_dir_all(&restore_target).ok();
    }

    #[test]
    #[cfg(unix)]
    fn restore_freedesktop_fails_when_trashinfo_is_missing() {
        let root = temp_path("restore-fd-no-info");
        let files_dir = root.join("files");
        let info_dir = root.join("info");
        fs::create_dir_all(&files_dir).expect("failed to create files dir");
        fs::create_dir_all(&info_dir).expect("failed to create info dir");

        let item_path = files_dir.join("orphan.txt");
        fs::write(&item_path, b"no metadata").expect("failed to write orphan item");
        // Deliberately do NOT write a .trashinfo file.

        let err = restore_trash_item(&item_path).unwrap_err();
        assert!(
            err.to_string().contains("orphan.txt.trashinfo"),
            "error should mention the missing trashinfo, got: {err}"
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn restore_fails_for_path_outside_any_known_trash_layout() {
        let tmp = temp_path("restore-unsupported");
        fs::create_dir_all(&tmp).expect("failed to create temp dir");
        let fake_item = tmp.join("item.txt");
        fs::write(&fake_item, b"content").expect("failed to write item");

        #[cfg(not(target_os = "macos"))]
        {
            let err = restore_trash_item(&fake_item).unwrap_err();
            assert!(
                err.to_string().contains("not supported"),
                "unexpected error: {err}"
            );
        }

        fs::remove_dir_all(&tmp).ok();
    }

    /// Regression test for false-positive FreeDesktop detection.
    ///
    /// On macOS, `~/.Trash/foo` computes `~/info` as the candidate info dir.
    /// If the user happens to have a `~/info` directory, the old code would
    /// take the FreeDesktop path and then fail looking for a `.trashinfo` file
    /// instead of falling through to the Finder backend.
    ///
    /// The fix requires the entry's immediate parent to be named `files` before
    /// treating the layout as FreeDesktop, so a `~/.Trash`-style path is never
    /// misidentified even when a coincidental `info/` exists nearby.
    #[test]
    #[cfg(not(target_os = "macos"))]
    fn restore_does_not_misdetect_freedesktop_when_info_dir_exists_at_wrong_level() {
        let root = temp_path("restore-false-positive");
        let trash_dir = root.join("Trash");
        let decoy_info = root.join("info");
        fs::create_dir_all(&trash_dir).expect("failed to create trash dir");
        fs::create_dir_all(&decoy_info).expect("failed to create decoy info dir");

        let item_path = trash_dir.join("foo.txt");
        fs::write(&item_path, b"content").expect("failed to write item");

        let err = restore_trash_item(&item_path).unwrap_err();
        assert!(
            err.to_string().contains("not supported"),
            "should bail as unsupported, not attempt FreeDesktop restore: {err}"
        );

        fs::remove_dir_all(&root).ok();
    }

    // ── macOS DS_Store restore helpers ────────────────────────────────────────

    #[test]
    #[cfg(target_os = "macos")]
    fn decode_utf16be_decodes_ascii_string() {
        let bytes = b"\x00H\x00i";
        assert_eq!(decode_utf16be(bytes), Some("Hi".to_string()));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn decode_utf16be_decodes_non_ascii() {
        let bytes = b"\x00\xe9";
        assert_eq!(decode_utf16be(bytes), Some("é".to_string()));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn decode_utf16be_rejects_odd_byte_count() {
        assert_eq!(decode_utf16be(b"\x00H\x00"), None);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn decode_utf16be_empty_slice_gives_empty_string() {
        assert_eq!(decode_utf16be(b""), Some(String::new()));
    }

    // ── remove_from_origins_map ───────────────────────────────────────────────

    #[test]
    #[cfg(target_os = "macos")]
    fn remove_from_origins_map_removes_exact_match() {
        let mut map = std::collections::HashMap::from([
            ("report.pdf".to_string(), "/home/user/report.pdf".to_string()),
            ("notes.txt".to_string(), "/home/user/notes.txt".to_string()),
        ]);
        let changed = remove_from_origins_map(&mut map, &["report.pdf"]);
        assert!(changed);
        assert!(!map.contains_key("report.pdf"), "target entry should be removed");
        assert!(map.contains_key("notes.txt"), "unrelated entry should be untouched");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn remove_from_origins_map_handles_collision_suffix_with_extension() {
        // "report.pdf" was stored as the key but macOS renamed it "report 2.pdf"
        // in the trash due to a collision.
        let mut map = std::collections::HashMap::from([
            ("report.pdf".to_string(), "/home/user/report.pdf".to_string()),
        ]);
        let changed = remove_from_origins_map(&mut map, &["report 2.pdf"]);
        assert!(changed);
        assert!(map.is_empty(), "collision-suffixed name should strip and remove base key");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn remove_from_origins_map_handles_collision_suffix_without_extension() {
        let mut map = std::collections::HashMap::from([
            ("notes".to_string(), "/home/user/notes".to_string()),
        ]);
        let changed = remove_from_origins_map(&mut map, &["notes 2"]);
        assert!(changed);
        assert!(map.is_empty());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn remove_from_origins_map_returns_false_when_key_not_found() {
        let mut map = std::collections::HashMap::from([
            ("other.txt".to_string(), "/home/user/other.txt".to_string()),
        ]);
        let changed = remove_from_origins_map(&mut map, &["missing.txt"]);
        assert!(!changed, "no match should return false and leave map untouched");
        assert_eq!(map.len(), 1);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn remove_from_origins_map_removes_multiple_names() {
        let mut map = std::collections::HashMap::from([
            ("a.txt".to_string(), "/home/user/a.txt".to_string()),
            ("b.txt".to_string(), "/home/user/b.txt".to_string()),
            ("c.txt".to_string(), "/home/user/c.txt".to_string()),
        ]);
        let changed = remove_from_origins_map(&mut map, &["a.txt", "c.txt"]);
        assert!(changed);
        assert!(!map.contains_key("a.txt"));
        assert!(map.contains_key("b.txt"), "untargeted entry must survive");
        assert!(!map.contains_key("c.txt"));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn remove_from_origins_map_no_op_on_empty_map() {
        let mut map = std::collections::HashMap::new();
        let changed = remove_from_origins_map(&mut map, &["foo.txt"]);
        assert!(!changed);
    }
}
