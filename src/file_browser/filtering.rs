#[derive(Clone, Debug, Default)]
pub(crate) struct LocalFilter {
    pub(crate) active: bool,
    pub(crate) query: String,
    pub(crate) cursor: usize,
}

impl LocalFilter {
    pub(crate) fn is_editing(&self) -> bool {
        self.active
    }

    pub(crate) fn query(&self) -> &str {
        &self.query
    }

    pub(crate) fn has_query(&self) -> bool {
        !self.query.trim().is_empty()
    }

    pub(crate) fn cursor(&self) -> usize {
        self.cursor.min(self.query.chars().count())
    }

    pub(crate) fn activate(&mut self) {
        self.active = true;
        self.cursor = self.query.chars().count();
    }

    pub(crate) fn finish_editing(&mut self) {
        self.active = false;
    }

    pub(crate) fn move_cursor(&mut self, delta: isize) {
        let max = self.query.chars().count() as isize;
        self.cursor = (self.cursor as isize + delta).clamp(0, max) as usize;
    }
}

impl super::FileBrowserState {
    pub(crate) fn apply_local_filter(&mut self) {
        let query = self.local_filter.query.trim();
        if query.is_empty() {
            self.entries = self.unfiltered_entries.clone();
            return;
        }

        let query = query.to_ascii_lowercase();
        self.entries = self
            .unfiltered_entries
            .iter()
            .filter(|entry| entry.name.to_ascii_lowercase().contains(&query))
            .cloned()
            .collect();
    }

    pub(crate) fn apply_local_filter_preserving_selection(&mut self) {
        let selected_path = self.selected_entry().map(|entry| entry.path.clone());
        self.apply_local_filter();
        self.selected = selected_path
            .and_then(|path| self.entries.iter().position(|entry| entry.path == path))
            .unwrap_or(0);
    }
}
