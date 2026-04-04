use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SidebarItem {
    pub kind: SidebarItemKind,
    pub title: String,
    pub icon: String,
    pub path: PathBuf,
}

impl SidebarItem {
    pub fn new(
        kind: SidebarItemKind,
        title: impl Into<String>,
        icon: impl Into<String>,
        path: PathBuf,
    ) -> Self {
        Self {
            kind,
            title: title.into(),
            icon: icon.into(),
            path,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SidebarItemKind {
    Home,
    Desktop,
    Documents,
    Downloads,
    Pictures,
    Music,
    Videos,
    Root,
    Trash,
    Custom,
    Device { removable: bool },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SidebarRow {
    Section { title: &'static str },
    Item(SidebarItem),
}

impl SidebarRow {
    pub fn item(&self) -> Option<&SidebarItem> {
        match self {
            Self::Item(item) => Some(item),
            Self::Section { .. } => None,
        }
    }
}
