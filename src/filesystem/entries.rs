use std::{path::PathBuf, time::SystemTime};

fn is_hidden(file_name: &std::ffi::OsStr) -> bool {
    file_name.to_string_lossy().starts_with('.')
}

/// Returns `true` if the directory entry should be treated as hidden.
///
/// On all platforms, entries whose names begin with `.` are hidden.
/// On Windows, entries with the `FILE_ATTRIBUTE_HIDDEN` attribute are also hidden.
pub(crate) fn is_hidden_entry(entry: &std::fs::DirEntry) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        if entry
            .metadata()
            .is_ok_and(|m| m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0)
        {
            return true;
        }
    }
    is_hidden(entry.file_name().as_os_str())
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EntryKind {
    Directory,
    File,
}

/// Symbolic link metadata: the stored link target and the resolved target kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymlinkInfo {
    pub target: Option<PathBuf>,
    pub target_kind: Option<EntryKind>,
}

impl SymlinkInfo {
    /// Returns true when the target cannot be resolved, including missing
    /// targets, permission failures, and symlink cycles.
    pub fn is_broken(&self) -> bool {
        self.target_kind.is_none()
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub name_key: String,
    pub kind: EntryKind,
    pub symlink: Option<SymlinkInfo>,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub readonly: bool,
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            name: String::new(),
            name_key: String::new(),
            kind: EntryKind::File,
            symlink: None,
            size: 0,
            modified: None,
            readonly: false,
        }
    }
}

impl Entry {
    pub fn is_dir(&self) -> bool {
        self.kind == EntryKind::Directory
    }

    pub fn is_symlink(&self) -> bool {
        self.symlink.is_some()
    }

    pub fn is_broken_symlink(&self) -> bool {
        self.symlink.as_ref().is_some_and(SymlinkInfo::is_broken)
    }
}
