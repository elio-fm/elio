mod directory;
mod duplicate_finder;
mod fuzzy_finder;
mod goto;
mod navigation;
pub(in crate::app) mod open_with;

use super::*;
use anyhow::{Result, anyhow, bail};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

#[cfg(test)]
mod tests;
