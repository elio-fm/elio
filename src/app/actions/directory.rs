use super::*;

impl App {
    pub fn reload(&mut self) -> Result<()> {
        self.queue_directory_reload(false)
    }

    pub fn process_auto_reload(&mut self) -> Result<bool> {
        while let Ok(event) = self.directory_runtime.watch_rx.try_recv() {
            match event {
                crate::fs::DirectoryWatchEvent::Changed(paths)
                    if !crate::fs::event_affects_visible_entries(&paths, self.show_hidden) => {}
                _ => {
                    self.directory_runtime.pending_reload_at =
                        Some(Instant::now() + crate::fs::directory_watch_debounce());
                }
            }
        }

        if let Some(deadline) = self.directory_runtime.pending_reload_at {
            if Instant::now() < deadline {
                return Ok(false);
            }
            self.directory_runtime.pending_reload_at = None;
            return self.reload_if_directory_changed();
        }

        if !self.directory_runtime.use_polling_reload {
            return Ok(false);
        }

        if self.directory_runtime.last_auto_reload_at.elapsed() < AUTO_RELOAD_INTERVAL {
            return Ok(false);
        }
        self.directory_runtime.last_auto_reload_at = Instant::now();
        self.reload_if_directory_changed()
    }

    pub(in crate::app) fn queue_directory_load(
        &mut self,
        mut load: PendingDirectoryLoad,
    ) -> Result<()> {
        self.directory_token = self.directory_token.wrapping_add(1);
        load.token = self.directory_token;
        let request = jobs::DirectoryRequest {
            token: load.token,
            cwd: load.target_cwd.clone(),
            show_hidden: self.show_hidden,
            sort_mode: self.sort_mode,
        };
        if !self.scheduler.submit_directory(request) {
            bail!("Directory worker unavailable");
        }
        self.directory_runtime.pending_load = Some(load);
        Ok(())
    }

    fn queue_directory_reload(&mut self, refresh_search: bool) -> Result<()> {
        self.queue_directory_load(PendingDirectoryLoad {
            token: 0,
            target_cwd: self.cwd.clone(),
            previous_cwd: self.cwd.clone(),
            previous_selected_path: self.selected_entry().map(|entry| entry.path.clone()),
            previous_selection_name: self.selected_entry().map(|entry| entry.name.clone()),
            reselect_path: None,
            history_mode: DirectoryHistoryMode::None,
            refresh_search,
            completion: DirectoryLoadCompletion::Keep,
        })
    }

    pub(in crate::app) fn apply_directory_snapshot(
        &mut self,
        load: PendingDirectoryLoad,
        snapshot: crate::fs::DirectorySnapshot,
    ) {
        let cwd_changed = load.target_cwd != self.cwd;
        let remembered_view = self.remembered_view_for(&load.target_cwd);
        self.cwd = load.target_cwd.clone();
        self.entries = snapshot.entries;
        self.sidebar = crate::fs::build_sidebar_items();
        self.directory_runtime.fingerprint = snapshot.fingerprint;
        self.directory_runtime.last_auto_reload_at = Instant::now();

        self.selected = if let Some(path) = &load.reselect_path {
            self.entries
                .iter()
                .position(|entry| entry.path == *path)
                .unwrap_or(0)
        } else if let Some(path) = remembered_view
            .as_ref()
            .and_then(|view| view.selected_path.as_ref())
        {
            self.entries
                .iter()
                .position(|entry| entry.path == *path)
                .unwrap_or(0)
        } else if let Some(name) = &load.previous_selection_name {
            self.entries
                .iter()
                .position(|entry| entry.name == *name)
                .unwrap_or(0)
        } else {
            0
        };
        self.scroll_row = remembered_view.map_or(0, |view| view.scroll_row);
        self.last_selection_change_at = Instant::now();
        self.clamp_selection();
        self.sync_scroll();
        self.remember_current_directory_view();
        self.refresh_preview();
        self.clear_wheel_scroll();

        if cwd_changed {
            self.reset_directory_watch();
        }

        match load.history_mode {
            DirectoryHistoryMode::None => {}
            DirectoryHistoryMode::PushCurrent => {
                self.navigation_history.back.push(HistoryEntry {
                    cwd: load.previous_cwd,
                    selected_path: load.previous_selected_path,
                });
                self.navigation_history.forward.clear();
            }
            DirectoryHistoryMode::GoBack => {
                if !self.navigation_history.back.is_empty() {
                    self.navigation_history.back.pop();
                }
                self.navigation_history.forward.push(HistoryEntry {
                    cwd: load.previous_cwd,
                    selected_path: load.previous_selected_path,
                });
            }
            DirectoryHistoryMode::GoForward => {
                if !self.navigation_history.forward.is_empty() {
                    self.navigation_history.forward.pop();
                }
                self.navigation_history.back.push(HistoryEntry {
                    cwd: load.previous_cwd,
                    selected_path: load.previous_selected_path,
                });
            }
        }

        if load.refresh_search {
            self.refresh_search_after_directory_reload();
        }

        match load.completion {
            DirectoryLoadCompletion::Keep => {}
            DirectoryLoadCompletion::Clear => self.status.clear(),
            DirectoryLoadCompletion::Status(status) => self.status = status,
        }
    }

    fn remembered_view_for(&self, cwd: &Path) -> Option<DirectoryViewMemory> {
        self.directory_view_memory.get(cwd).cloned()
    }

    pub(in crate::app) fn remember_current_directory_view(&mut self) {
        self.directory_view_memory.insert(
            self.cwd.clone(),
            DirectoryViewMemory {
                selected_path: self.selected_entry().map(|entry| entry.path.clone()),
                scroll_row: self.scroll_row,
            },
        );
    }

    pub(in crate::app) fn set_dir(&mut self, path: PathBuf) -> Result<()> {
        self.set_dir_transition(
            path,
            DirectoryHistoryMode::PushCurrent,
            None,
            DirectoryLoadCompletion::Clear,
        )
    }

    pub(in crate::app) fn set_dir_transition(
        &mut self,
        path: PathBuf,
        history_mode: DirectoryHistoryMode,
        reselect_path: Option<PathBuf>,
        completion: DirectoryLoadCompletion,
    ) -> Result<()> {
        let metadata = std::fs::metadata(&path).map_err(|error| {
            anyhow!(
                "Cannot open {}: {}",
                path.display(),
                crate::fs::describe_io_error(&error)
            )
        })?;
        if !metadata.is_dir() {
            bail!("{} is not a directory", path.display());
        }
        let normalized = path.canonicalize().unwrap_or(path);
        if normalized == self.cwd && self.directory_runtime.pending_load.is_none() {
            if let Some(path) = reselect_path.as_ref()
                && self.reselect_visible_entry(path)
            {
                self.apply_directory_completion(completion);
                return Ok(());
            }
            self.status = format!("Already in {}", self.cwd.display());
            return Ok(());
        }
        if self
            .directory_runtime
            .pending_load
            .as_ref()
            .is_some_and(|load| load.target_cwd == normalized)
        {
            if let Some(load) = self.directory_runtime.pending_load.as_mut() {
                if let Some(path) = reselect_path {
                    load.reselect_path = Some(path);
                }
                load.completion = completion;
            }
            self.status = format!("Already opening {}", normalized.display());
            return Ok(());
        }

        let reselect_path = reselect_path.or_else(|| {
            self.remembered_view_for(&normalized)
                .and_then(|view| view.selected_path)
        });
        self.status = format!("Opening {}", normalized.display());
        self.queue_directory_load(PendingDirectoryLoad {
            token: 0,
            target_cwd: normalized,
            previous_cwd: self.cwd.clone(),
            previous_selected_path: self.selected_entry().map(|entry| entry.path.clone()),
            previous_selection_name: None,
            reselect_path,
            history_mode,
            refresh_search: false,
            completion,
        })
    }

    fn reselect_visible_entry(&mut self, path: &Path) -> bool {
        let Some(index) = self.entries.iter().position(|entry| entry.path == path) else {
            return false;
        };
        self.set_selected(index);
        self.clear_wheel_scroll();
        true
    }

    fn apply_directory_completion(&mut self, completion: DirectoryLoadCompletion) {
        match completion {
            DirectoryLoadCompletion::Keep => {}
            DirectoryLoadCompletion::Clear => self.status.clear(),
            DirectoryLoadCompletion::Status(status) => self.status = status,
        }
    }

    pub(in crate::app) fn go_parent(&mut self) -> Result<()> {
        let current = self.cwd.clone();
        let Some(parent) = self.cwd.parent() else {
            self.status = "Already at filesystem root".to_string();
            return Ok(());
        };
        self.set_dir_transition(
            parent.to_path_buf(),
            DirectoryHistoryMode::PushCurrent,
            Some(current),
            DirectoryLoadCompletion::Clear,
        )
    }

    pub(in crate::app) fn reset_directory_watch(&mut self) {
        self.directory_runtime.watch = None;
        self.directory_runtime.pending_reload_at = None;
        while self.directory_runtime.watch_rx.try_recv().is_ok() {}

        match crate::fs::start_directory_watcher(&self.cwd, &self.directory_runtime.watch_tx) {
            Ok(watcher) => {
                self.directory_runtime.watch = Some(watcher);
                self.directory_runtime.use_polling_reload = false;
            }
            Err(_) => {
                self.directory_runtime.use_polling_reload = true;
            }
        }
    }

    fn reload_if_directory_changed(&mut self) -> Result<bool> {
        if self.directory_runtime.pending_load.is_some() {
            return Ok(false);
        }
        let fingerprint = match crate::fs::scan_directory_fingerprint(&self.cwd, self.show_hidden) {
            Ok(fingerprint) => fingerprint,
            Err(_) => return Ok(false),
        };
        if fingerprint == self.directory_runtime.fingerprint {
            return Ok(false);
        }

        self.queue_directory_reload(true)?;
        Ok(true)
    }

    fn refresh_search_after_directory_reload(&mut self) {
        let Some(scope) = self.search.as_ref().map(|search| search.scope) else {
            return;
        };

        if let Some(search) = &mut self.search {
            search.candidates = Arc::new(Vec::new());
            search.matches.clear();
            search.cached_matches = HashMap::from([(String::new(), Vec::new())]);
            search.selected = 0;
            search.scroll = 0;
            search.loading = true;
            search.error = None;
        }
        self.prewarm_search_index(scope);
    }
}
