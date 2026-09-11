use super::*;
use crate::{file_classification::FileClass, fs::EntryKind};
use std::path::Path;

use crate::preview::archives as archive_preview;

#[path = "../comics/tests/mod.rs"]
mod comics;
mod iso;
mod java_archives;
mod standard_archives;
