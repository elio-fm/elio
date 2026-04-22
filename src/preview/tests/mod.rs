use super::{appearance as theme, *};
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

mod archives;
mod audio;
mod binaries;
mod code;
mod data;
mod documents;
mod fonts;
mod helpers;
mod images;
mod markdown;
mod structured;
mod text;
mod videos;

use self::helpers::*;
