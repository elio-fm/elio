use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
};

const CACHE_LIMIT: usize = 2048;

#[derive(Default)]
pub(crate) struct FolderSizes {
    pub(crate) token: u64,
    pub(crate) pending: usize,
    pub(crate) current: HashMap<PathBuf, u64>,
    requested: HashSet<PathBuf>,
    cache: HashMap<PathBuf, u64>,
    order: VecDeque<PathBuf>,
}

#[cfg(test)]
#[path = "tests/folder_sizes.rs"]
mod tests;

impl FolderSizes {
    pub(crate) fn begin(&mut self, token: u64, paths: &[PathBuf]) {
        self.token = token;
        self.requested = paths.iter().cloned().collect();
        self.pending = self.requested.len();
        self.current = paths
            .iter()
            .filter_map(|path| self.cache.get(path).map(|size| (path.clone(), *size)))
            .collect();
    }

    pub(crate) fn apply(&mut self, token: u64, path: PathBuf, size: Option<u64>) -> bool {
        if token != self.token || !self.requested.remove(&path) {
            return false;
        }
        self.pending = self.requested.len();
        self.order.retain(|cached| cached != &path);
        match size {
            Some(size) => {
                self.current.insert(path.clone(), size);
                self.cache.insert(path.clone(), size);
                self.order.push_back(path);
                while self.order.len() > CACHE_LIMIT {
                    if let Some(old) = self.order.pop_front() {
                        self.cache.remove(&old);
                    }
                }
            }
            None => {
                self.current.remove(&path);
                self.cache.remove(&path);
            }
        }
        true
    }
}
