mod file_model;
mod sidebar;

pub(crate) use self::file_model::FileClass;
pub use self::file_model::{Entry, EntryKind, SortMode};
pub use self::sidebar::{SidebarItem, SidebarItemKind, SidebarRow};
