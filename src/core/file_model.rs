use std::{path::PathBuf, time::SystemTime};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SortMode {
    Name,
    Modified,
    Size,
}

impl SortMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::Name => Self::Modified,
            Self::Modified => Self::Size,
            Self::Size => Self::Name,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Modified => "Modified",
            Self::Size => "Size",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EntryKind {
    Directory,
    File,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum FileClass {
    Directory,
    Code,
    Config,
    Document,
    License,
    Image,
    Audio,
    Video,
    Archive,
    Font,
    Data,
    File,
}

/// Symbolic link metadata: the stored link target and the resolved target kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymlinkInfo {
    pub target: Option<PathBuf>,
    pub target_kind: Option<EntryKind>,
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
}
