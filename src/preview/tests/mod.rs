use super::{appearance as theme, *};
#[cfg(unix)]
use crate::fs::{EntryKind, SymlinkInfo};
use image::ImageFormat;
use ratatui::{style::Modifier, text::Line};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    fs::File,
    io::Write,
    process::Command,
    sync::{Arc, Barrier},
    thread,
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

mod audio;
mod directory;
mod font;
mod helpers;
mod markdown;
mod plain_text;
mod torrent;
mod video;

#[path = "../archives/tests/mod.rs"]
mod archives;
#[path = "../binary/tests/mod.rs"]
mod binary;
#[path = "../code/tests/mod.rs"]
mod code;
#[path = "../documents/tests/mod.rs"]
mod documents;
#[path = "../structured_text/tests/mod.rs"]
mod structured_text;
#[path = "../tabular_data/tests/mod.rs"]
mod tabular_data;

use self::helpers::*;
