use crate::search::SearchCandidate;
use anyhow::{Context, Result, bail};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use std::{
    cmp::Ordering,
    collections::HashMap,
    env,
    ffi::OsStr,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant, SystemTime},
};

const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(450);
const WHEEL_SCROLL_INTERVAL_HORIZONTAL: Duration = Duration::from_millis(36);
const WHEEL_SCROLL_INTERVAL_VERTICAL: Duration = Duration::from_millis(42);
const WHEEL_SCROLL_INTERVAL_SEARCH: Duration = Duration::from_millis(38);
const WHEEL_SCROLL_QUEUE_LIMIT: isize = 8;
const PREVIEW_LIMIT_BYTES: usize = 8 * 1024;
const PREVIEW_MAX_LINES: usize = 24;
const SEARCH_MATCH_LIMIT: usize = 250;
const SEARCH_CACHE_LIMIT: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewMode {
    Grid,
    List,
}

impl ViewMode {
    pub fn toggle(self) -> Self {
        match self {
            Self::Grid => Self::List,
            Self::List => Self::Grid,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Grid => "Grid",
            Self::List => "List",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryKind {
    Directory,
    File,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub name_key: String,
    pub kind: EntryKind,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub readonly: bool,
    pub hidden: bool,
}

impl Entry {
    pub fn is_dir(&self) -> bool {
        self.kind == EntryKind::Directory
    }

    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            EntryKind::Directory => "Folder",
            EntryKind::File => "File",
        }
    }

    pub fn detail_label(&self) -> &'static str {
        match self.kind {
            EntryKind::Directory => "Folder",
            EntryKind::File => match extension_class(&self.path) {
                "code" => "Code file",
                "image" => "Image file",
                "audio" => "Audio file",
                "video" => "Video file",
                "archive" => "Archive file",
                "text" => "Text file",
                _ => "File",
            },
        }
    }

    pub fn badge(&self) -> &'static str {
        match self.kind {
            EntryKind::Directory => "󰉋 DIR",
            EntryKind::File => match extension_class(&self.path) {
                "code" => "󰆍 CODE",
                "image" => "󰋩 IMG",
                "audio" => "󰎆 AUDIO",
                "video" => "󰈫 VIDEO",
                "archive" => "󰗄 ARC",
                "text" => "󰈙 TXT",
                _ => "󰈔 FILE",
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct SidebarItem {
    pub title: String,
    pub icon: &'static str,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default)]
pub struct FrameState {
    pub sidebar_hits: Vec<PathHit>,
    pub entry_hits: Vec<EntryHit>,
    pub search_hits: Vec<SearchHit>,
    pub search_panel: Option<Rect>,
    pub back_button: Option<Rect>,
    pub forward_button: Option<Rect>,
    pub parent_button: Option<Rect>,
    pub refresh_button: Option<Rect>,
    pub hidden_button: Option<Rect>,
    pub view_button: Option<Rect>,
    pub metrics: ViewMetrics,
    pub search_rows_visible: usize,
}

#[derive(Clone, Debug)]
pub struct PathHit {
    pub rect: Rect,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct EntryHit {
    pub rect: Rect,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub rect: Rect,
    pub index: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct ViewMetrics {
    pub cols: usize,
    pub rows_visible: usize,
}

impl Default for ViewMetrics {
    fn default() -> Self {
        Self {
            cols: 1,
            rows_visible: 1,
        }
    }
}

#[derive(Clone, Debug)]
struct ClickState {
    path: PathBuf,
    at: Instant,
}

#[derive(Clone, Debug)]
struct ScrollLane {
    pending: isize,
    last_step_at: Option<Instant>,
}

#[derive(Clone, Debug)]
struct ScrollState {
    horizontal: ScrollLane,
    vertical: ScrollLane,
    search: ScrollLane,
}

#[derive(Clone, Debug)]
struct SearchOverlay {
    scope: SearchScope,
    query: String,
    candidates: Arc<Vec<SearchCandidate>>,
    matches: Vec<usize>,
    cached_matches: HashMap<String, Vec<usize>>,
    selected: usize,
    scroll: usize,
    loading: bool,
    error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchScope {
    Folders,
    Files,
}

impl SearchScope {
    pub fn label(self) -> &'static str {
        match self {
            Self::Folders => "Folders",
            Self::Files => "Files",
        }
    }

    fn accepts(self, candidate: &SearchCandidate) -> bool {
        match self {
            Self::Folders => candidate.is_dir,
            Self::Files => !candidate.is_dir,
        }
    }

    pub fn empty_label(self) -> &'static str {
        match self {
            Self::Folders => "No matching folders in this tree",
            Self::Files => "No matching files in this tree",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SearchRow {
    pub index: usize,
    pub name: String,
    pub relative: String,
    pub is_dir: bool,
    pub selected: bool,
}

#[derive(Clone, Debug)]
struct SearchCache {
    cwd: PathBuf,
    show_hidden: bool,
    candidates: Arc<Vec<SearchCandidate>>,
}

#[derive(Debug)]
struct SearchBuild {
    token: u64,
    cwd: PathBuf,
    show_hidden: bool,
    result: Result<Arc<Vec<SearchCandidate>>, String>,
}

#[derive(Debug)]
struct SearchRequest {
    token: u64,
    cwd: PathBuf,
    show_hidden: bool,
}

pub struct App {
    pub cwd: PathBuf,
    pub entries: Vec<Entry>,
    pub sidebar: Vec<SidebarItem>,
    pub selected: usize,
    pub scroll_row: usize,
    pub view_mode: ViewMode,
    pub zoom_level: u8,
    pub sort_mode: SortMode,
    pub show_hidden: bool,
    pub status: String,
    pub help_open: bool,
    pub should_quit: bool,
    back_history: Vec<PathBuf>,
    forward_history: Vec<PathBuf>,
    preview_cache: Vec<Line<'static>>,
    frame_state: FrameState,
    search: Option<SearchOverlay>,
    search_cache: Option<SearchCache>,
    search_loading: bool,
    search_token: u64,
    search_request_tx: mpsc::Sender<SearchRequest>,
    search_rx: mpsc::Receiver<SearchBuild>,
    last_click: Option<ClickState>,
    wheel_scroll: ScrollState,
}

impl App {
    pub fn new() -> Result<Self> {
        let cwd = env::current_dir().context("failed to read current directory")?;
        Self::new_at(cwd)
    }

    pub fn new_at(cwd: PathBuf) -> Result<Self> {
        let (search_request_tx, search_request_rx) = mpsc::channel();
        let (search_tx, search_rx) = mpsc::channel();
        spawn_search_worker(search_request_rx, search_tx);
        let mut app = Self {
            cwd,
            entries: Vec::new(),
            sidebar: Vec::new(),
            selected: 0,
            scroll_row: 0,
            view_mode: ViewMode::Grid,
            zoom_level: 1,
            sort_mode: SortMode::Name,
            show_hidden: false,
            status: String::new(),
            help_open: false,
            should_quit: false,
            back_history: Vec::new(),
            forward_history: Vec::new(),
            preview_cache: Vec::new(),
            frame_state: FrameState::default(),
            search: None,
            search_cache: None,
            search_loading: false,
            search_token: 0,
            search_request_tx,
            search_rx,
            last_click: None,
            wheel_scroll: ScrollState {
                horizontal: ScrollLane {
                    pending: 0,
                    last_step_at: None,
                },
                vertical: ScrollLane {
                    pending: 0,
                    last_step_at: None,
                },
                search: ScrollLane {
                    pending: 0,
                    last_step_at: None,
                },
            },
        };
        app.reload()?;
        Ok(app)
    }

    pub fn set_frame_state(&mut self, frame_state: FrameState) -> bool {
        self.frame_state = frame_state;
        self.sync_scroll() | self.sync_search_scroll()
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.entries.get(self.selected)
    }

    pub fn reload(&mut self) -> Result<()> {
        let previous_name = self.selected_entry().map(|entry| entry.name.clone());
        self.entries = read_entries(&self.cwd, self.show_hidden)?;
        sort_entries(&mut self.entries, self.sort_mode);
        self.sidebar = build_sidebar_items();

        self.selected = match previous_name {
            Some(name) => self
                .entries
                .iter()
                .position(|entry| entry.name == name)
                .unwrap_or(0),
            None => 0,
        };
        self.clamp_selection();
        self.sync_scroll();
        self.refresh_preview();
        self.prewarm_search_index();
        self.clear_wheel_scroll();
        Ok(())
    }

    pub fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            Event::Resize(_, _) | Event::FocusGained | Event::FocusLost | Event::Paste(_) => Ok(()),
        }
    }

    pub fn process_background_jobs(&mut self) -> bool {
        let mut dirty = false;

        while let Ok(build) = self.search_rx.try_recv() {
            if build.token != self.search_token
                || build.cwd != self.cwd
                || build.show_hidden != self.show_hidden
            {
                continue;
            }

            self.search_loading = false;
            dirty = true;

            match build.result {
                Ok(candidates) => {
                    self.search_cache = Some(SearchCache {
                        cwd: build.cwd,
                        show_hidden: build.show_hidden,
                        candidates: candidates.clone(),
                    });
                    if let Some(search) = &mut self.search {
                        search.candidates = candidates;
                        search.cached_matches = HashMap::from([(
                            String::new(),
                            search_scope_indices(&search.candidates, search.scope),
                        )]);
                        search.loading = false;
                        search.error = None;
                    }
                    self.refresh_search_matches("");
                }
                Err(error) => {
                    self.search_cache = None;
                    if let Some(search) = &mut self.search {
                        search.candidates = Arc::new(Vec::new());
                        search.matches.clear();
                        search.cached_matches = HashMap::from([(String::new(), Vec::new())]);
                        search.selected = 0;
                        search.scroll = 0;
                        search.loading = false;
                        search.error = Some(error);
                    }
                }
            }
        }

        dirty
    }

    pub fn process_pending_scroll(&mut self) -> bool {
        let mut dirty = false;

        if self.search.is_some() {
            self.wheel_scroll.vertical.pending = 0;
            self.wheel_scroll.horizontal.pending = 0;
            dirty |= self.flush_search_scroll();
        } else {
            self.wheel_scroll.search.pending = 0;
            dirty |= self.flush_entry_vertical_scroll();
            if self.view_mode == ViewMode::Grid {
                dirty |= self.flush_entry_horizontal_scroll();
            } else {
                self.wheel_scroll.horizontal.pending = 0;
            }
        }

        dirty
    }

    pub fn has_pending_scroll(&self) -> bool {
        self.wheel_scroll.vertical.pending != 0
            || self.wheel_scroll.horizontal.pending != 0
            || self.wheel_scroll.search.pending != 0
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.search.is_some() {
            return self.handle_search_key(key);
        }

        if self.help_open {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c'))
            {
                self.help_open = false;
                return Ok(());
            }
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') => self.help_open = false,
                _ => {}
            }
            return Ok(());
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.should_quit = true;
            return Ok(());
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('f') => {
                    if let Err(error) = self.open_fuzzy_finder(SearchScope::Files) {
                        self.status = format!("Search unavailable: {error}");
                    }
                    return Ok(());
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    self.adjust_zoom(1);
                    return Ok(());
                }
                KeyCode::Char('-') | KeyCode::Char('_') => {
                    self.adjust_zoom(-1);
                    return Ok(());
                }
                KeyCode::Char('r') => {
                    self.reload()?;
                    self.status = "Refreshed".to_string();
                    return Ok(());
                }
                KeyCode::Char('0') => {
                    self.reset_zoom();
                    return Ok(());
                }
                _ => {}
            }
        }

        if key.modifiers.contains(KeyModifiers::ALT) {
            match key.code {
                KeyCode::Left => return self.go_back(),
                KeyCode::Right => return self.go_forward(),
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('?') => {
                self.clear_wheel_scroll();
                self.help_open = true;
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_vertical(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_vertical(1),
            KeyCode::Left => {
                if self.view_mode == ViewMode::Grid {
                    self.move_by(-1);
                } else {
                    self.go_parent()?;
                }
            }
            KeyCode::Char('h') => self.go_home()?,
            KeyCode::Right | KeyCode::Char('l') => {
                if self.view_mode == ViewMode::Grid {
                    self.move_by(1);
                } else {
                    self.open_selected()?;
                }
            }
            KeyCode::PageUp => self.page(-1),
            KeyCode::PageDown => self.page(1),
            KeyCode::Home => self.select_index(0),
            KeyCode::End => self.select_last(),
            KeyCode::Char('g') => self.select_index(0),
            KeyCode::Char('G') => self.select_last(),
            KeyCode::Enter => self.open_selected()?,
            KeyCode::Backspace => self.go_parent()?,
            KeyCode::Char('v') => {
                self.clear_wheel_scroll();
                self.view_mode = self.view_mode.toggle();
                self.sync_scroll();
                self.status = format!("Switched to {} view", self.view_mode.label());
            }
            KeyCode::Char('s') => {
                self.sort_mode = self.sort_mode.cycle();
                self.reload()?;
                self.status = format!("Sort: {}", self.sort_mode.label());
            }
            KeyCode::Char('.') => {
                self.show_hidden = !self.show_hidden;
                self.reload()?;
                self.status = if self.show_hidden {
                    "Hidden files shown".to_string()
                } else {
                    "Hidden files hidden".to_string()
                };
            }
            KeyCode::Char('r') => {
                self.reload()?;
                self.status = "Refreshed".to_string();
            }
            KeyCode::Char('f') => {
                if let Err(error) = self.open_fuzzy_finder(SearchScope::Folders) {
                    self.status = format!("Search unavailable: {error}");
                }
            }
            KeyCode::Char('o') => self.open_in_system()?,
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        if self.search.is_some() {
            return self.handle_search_mouse(mouse);
        }

        if self.help_open {
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                self.clear_wheel_scroll();
                self.help_open = false;
            }
            return Ok(());
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(rect) = self.frame_state.back_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    return self.go_back();
                }
                if let Some(rect) = self.frame_state.forward_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    return self.go_forward();
                }
                if let Some(rect) = self.frame_state.parent_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    return self.go_parent();
                }
                if let Some(rect) = self.frame_state.refresh_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    self.reload()?;
                    self.status = "Refreshed".to_string();
                    return Ok(());
                }
                if let Some(rect) = self.frame_state.hidden_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    self.show_hidden = !self.show_hidden;
                    self.reload()?;
                    self.status = if self.show_hidden {
                        "Hidden files shown".to_string()
                    } else {
                        "Hidden files hidden".to_string()
                    };
                    return Ok(());
                }
                if let Some(rect) = self.frame_state.view_button
                    && rect_contains(rect, mouse.column, mouse.row)
                {
                    self.clear_wheel_scroll();
                    self.view_mode = self.view_mode.toggle();
                    self.sync_scroll();
                    self.status = format!("Switched to {} view", self.view_mode.label());
                    return Ok(());
                }

                if let Some(target) = self
                    .frame_state
                    .sidebar_hits
                    .iter()
                    .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                    .cloned()
                {
                    return self.set_dir(target.path);
                }

                if let Some(hit) = self
                    .frame_state
                    .entry_hits
                    .iter()
                    .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                    .cloned()
                {
                    self.select_index(hit.index);
                    let path = self.entries[hit.index].path.clone();
                    if self.is_double_click(&path) {
                        self.open_selected()?;
                    }
                    self.last_click = Some(ClickState {
                        path,
                        at: Instant::now(),
                    });
                }
            }
            MouseEventKind::ScrollDown => {
                if self.view_mode == ViewMode::Grid {
                    if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                        Self::queue_scroll(&mut self.wheel_scroll.horizontal, 1);
                    } else {
                        Self::queue_scroll(&mut self.wheel_scroll.vertical, 1);
                    }
                } else {
                    Self::queue_scroll(&mut self.wheel_scroll.vertical, 1);
                }
            }
            MouseEventKind::ScrollUp => {
                if self.view_mode == ViewMode::Grid {
                    if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                        Self::queue_scroll(&mut self.wheel_scroll.horizontal, -1);
                    } else {
                        Self::queue_scroll(&mut self.wheel_scroll.vertical, -1);
                    }
                } else {
                    Self::queue_scroll(&mut self.wheel_scroll.vertical, -1);
                }
            }
            MouseEventKind::ScrollLeft if self.view_mode == ViewMode::Grid => {
                Self::queue_scroll(&mut self.wheel_scroll.horizontal, -1);
            }
            MouseEventKind::ScrollRight if self.view_mode == ViewMode::Grid => {
                Self::queue_scroll(&mut self.wheel_scroll.horizontal, 1);
            }
            _ => {}
        }
        Ok(())
    }

    pub fn open_selected(&mut self) -> Result<()> {
        let Some(entry) = self.selected_entry() else {
            return Ok(());
        };
        if entry.is_dir() {
            self.set_dir(entry.path.clone())
        } else {
            self.open_in_system()
        }
    }

    pub fn preview_lines(&self, max_lines: usize) -> Vec<Line<'static>> {
        self.preview_cache
            .iter()
            .take(max_lines)
            .cloned()
            .collect::<Vec<_>>()
    }

    fn refresh_preview(&mut self) {
        self.preview_cache = match self.selected_entry() {
            Some(entry) => build_preview(entry),
            None => vec![Line::from("No selection")],
        };
    }

    pub fn selection_summary(&self) -> String {
        match self.selected_entry() {
            Some(entry) => format!(
                "{} of {} selected  •  {}  •  {}",
                self.selected.saturating_add(1),
                self.entries.len(),
                entry.kind_label(),
                entry.name,
            ),
            None => format!("0 items  •  {}", self.cwd.display()),
        }
    }

    pub fn status_message(&self) -> &str {
        &self.status
    }

    pub fn search_is_open(&self) -> bool {
        self.search.is_some()
    }

    pub fn search_query(&self) -> &str {
        self.search
            .as_ref()
            .map(|search| search.query.as_str())
            .unwrap_or("")
    }

    pub fn search_match_count(&self) -> usize {
        self.search
            .as_ref()
            .map(|search| search.matches.len())
            .unwrap_or(0)
    }

    pub fn search_candidate_count(&self) -> usize {
        self.search
            .as_ref()
            .and_then(|search| search.cached_matches.get("").map(Vec::len))
            .unwrap_or(0)
    }

    pub fn search_scope(&self) -> Option<SearchScope> {
        self.search.as_ref().map(|search| search.scope)
    }

    pub fn search_is_loading(&self) -> bool {
        self.search.as_ref().is_some_and(|search| search.loading)
    }

    pub fn search_error(&self) -> Option<&str> {
        self.search
            .as_ref()
            .and_then(|search| search.error.as_deref())
    }

    pub fn search_rows(&self, max_rows: usize) -> Vec<SearchRow> {
        let Some(search) = &self.search else {
            return Vec::new();
        };

        let end = (search.scroll + max_rows).min(search.matches.len());
        (search.scroll..end)
            .map(|visible_index| {
                let candidate = &search.candidates[search.matches[visible_index]];
                SearchRow {
                    index: visible_index,
                    name: candidate.name.clone(),
                    relative: candidate.relative.clone(),
                    is_dir: candidate.is_dir,
                    selected: visible_index == search.selected,
                }
            })
            .collect()
    }

    pub fn can_go_back(&self) -> bool {
        !self.back_history.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward_history.is_empty()
    }

    fn set_selected(&mut self, index: usize) {
        let next = index.min(self.entries.len().saturating_sub(1));
        if next != self.selected {
            self.selected = next;
            self.refresh_preview();
        } else {
            self.selected = next;
        }
        self.sync_scroll();
    }

    fn set_selected_last(&mut self) {
        if !self.entries.is_empty() {
            let last = self.entries.len() - 1;
            self.set_selected(last);
        }
    }

    fn set_selected_delta(&mut self, delta: isize) {
        if self.entries.is_empty() {
            self.selected = 0;
            self.preview_cache = vec![Line::from("No selection")];
            return;
        }

        let max_index = self.entries.len().saturating_sub(1) as isize;
        let next = (self.selected as isize + delta).clamp(0, max_index) as usize;
        self.set_selected(next);
    }

    fn page(&mut self, direction: isize) {
        let rows = self.frame_state.metrics.rows_visible.max(1) as isize;
        if self.view_mode == ViewMode::Grid {
            self.move_grid_vertical(direction * rows);
        } else {
            self.set_selected_delta(direction * rows);
        }
    }

    fn move_vertical(&mut self, rows: isize) {
        if self.view_mode == ViewMode::Grid {
            self.move_grid_vertical(rows);
        } else {
            self.set_selected_delta(rows);
        }
    }

    fn move_by(&mut self, delta: isize) {
        self.set_selected_delta(delta);
    }

    fn move_grid_vertical(&mut self, rows: isize) {
        if self.entries.is_empty() {
            self.selected = 0;
            return;
        }

        let cols = self.frame_state.metrics.cols.max(1);
        let current_row = self.selected / cols;
        let current_col = self.selected % cols;
        let total_rows = self.entries.len().div_ceil(cols);
        let target_row = current_row as isize + rows;

        if target_row < 0 || target_row >= total_rows as isize {
            return;
        }

        let target_index = target_row as usize * cols + current_col;
        if target_index >= self.entries.len() {
            return;
        }

        self.set_selected(target_index);
    }

    fn open_fuzzy_finder(&mut self, scope: SearchScope) -> Result<()> {
        self.clear_wheel_scroll();
        self.help_open = false;
        let cached = self
            .search_cache
            .as_ref()
            .filter(|cache| cache.cwd == self.cwd && cache.show_hidden == self.show_hidden)
            .map(|cache| cache.candidates.clone());
        let candidates = cached.clone().unwrap_or_else(|| Arc::new(Vec::new()));
        let base_matches = search_scope_indices(&candidates, scope);
        let matches = base_matches
            .iter()
            .copied()
            .take(SEARCH_MATCH_LIMIT)
            .collect::<Vec<_>>();
        let loading = self.search_loading || cached.is_none();
        if cached.is_none() && !self.search_loading {
            self.prewarm_search_index();
        }
        self.search = Some(SearchOverlay {
            scope,
            query: String::new(),
            candidates,
            matches,
            cached_matches: HashMap::from([(String::new(), base_matches)]),
            selected: 0,
            scroll: 0,
            loading,
            error: None,
        });
        self.status.clear();
        Ok(())
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.search = None;
            self.clear_wheel_scroll();
            self.status.clear();
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                self.search = None;
                self.clear_wheel_scroll();
                self.status.clear();
            }
            KeyCode::Enter => self.confirm_search_selection()?,
            KeyCode::Up | KeyCode::Char('k') => self.move_search_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_search_selection(1),
            KeyCode::PageUp => self.page_search(-1),
            KeyCode::PageDown => self.page_search(1),
            KeyCode::Home => self.select_search_index(0),
            KeyCode::End => self.select_last_search_result(),
            KeyCode::Backspace => {
                let previous_query = self
                    .search
                    .as_ref()
                    .map(|search| search.query.clone())
                    .unwrap_or_default();
                if let Some(search) = &mut self.search {
                    search.query.pop();
                }
                self.refresh_search_matches(&previous_query);
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                let previous_query = self
                    .search
                    .as_ref()
                    .map(|search| search.query.clone())
                    .unwrap_or_default();
                if let Some(search) = &mut self.search {
                    search.query.push(ch);
                }
                self.refresh_search_matches(&previous_query);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_search_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(hit) = self
                    .frame_state
                    .search_hits
                    .iter()
                    .find(|hit| rect_contains(hit.rect, mouse.column, mouse.row))
                    .cloned()
                {
                    self.select_search_index(hit.index);
                    self.confirm_search_selection()?;
                } else if self
                    .frame_state
                    .search_panel
                    .is_none_or(|rect| !rect_contains(rect, mouse.column, mouse.row))
                {
                    self.search = None;
                    self.clear_wheel_scroll();
                    self.status.clear();
                }
            }
            MouseEventKind::ScrollDown => Self::queue_scroll(&mut self.wheel_scroll.search, 1),
            MouseEventKind::ScrollUp => Self::queue_scroll(&mut self.wheel_scroll.search, -1),
            _ => {}
        }
        Ok(())
    }

    fn refresh_search_matches(&mut self, previous_query: &str) {
        let Some(search) = &mut self.search else {
            return;
        };

        let next_query = search.query.clone();
        if let Some(cached) = search.cached_matches.get(&next_query).cloned() {
            search.matches = cached;
        } else {
            let pool = if !previous_query.is_empty() && next_query.starts_with(previous_query) {
                search.matches.clone()
            } else {
                search
                    .cached_matches
                    .get("")
                    .cloned()
                    .unwrap_or_else(|| (0..search.candidates.len()).collect::<Vec<_>>())
            };

            search.matches = crate::search::filter_candidates_in(
                &search.candidates,
                pool,
                &next_query,
                SEARCH_MATCH_LIMIT,
            );
            prune_search_cache(&mut search.cached_matches, &next_query);
            search
                .cached_matches
                .insert(next_query.clone(), search.matches.clone());
        }

        if search.matches.is_empty() {
            search.selected = 0;
            search.scroll = 0;
            return;
        }

        search.selected = search.selected.min(search.matches.len().saturating_sub(1));
        self.sync_search_scroll();
    }

    fn move_search_selection(&mut self, delta: isize) {
        let Some(search) = &mut self.search else {
            return;
        };
        if search.matches.is_empty() {
            search.selected = 0;
            search.scroll = 0;
            return;
        }

        let max_index = search.matches.len().saturating_sub(1) as isize;
        search.selected = (search.selected as isize + delta).clamp(0, max_index) as usize;
        self.sync_search_scroll();
    }

    fn page_search(&mut self, direction: isize) {
        let visible = self.frame_state.search_rows_visible.max(1) as isize;
        self.move_search_selection(direction * visible);
    }

    fn select_search_index(&mut self, index: usize) {
        let Some(search) = &mut self.search else {
            return;
        };
        if search.matches.is_empty() {
            search.selected = 0;
            search.scroll = 0;
            return;
        }
        search.selected = index.min(search.matches.len().saturating_sub(1));
        self.sync_search_scroll();
    }

    fn select_last_search_result(&mut self) {
        let Some(search) = &self.search else {
            return;
        };
        let last = search.matches.len().saturating_sub(1);
        self.select_search_index(last);
    }

    fn confirm_search_selection(&mut self) -> Result<()> {
        let Some(path) = self.search.as_ref().and_then(|search| {
            search
                .matches
                .get(search.selected)
                .copied()
                .map(|index| search.candidates[index].path.clone())
        }) else {
            return Ok(());
        };

        self.search = None;
        self.reveal_path(path)
    }

    fn sync_search_scroll(&mut self) -> bool {
        let Some(search) = &mut self.search else {
            return false;
        };
        if search.matches.is_empty() {
            let changed = search.scroll != 0;
            search.scroll = 0;
            return changed;
        }

        let previous = search.scroll;
        let rows_visible = self.frame_state.search_rows_visible.max(1);
        if search.selected < search.scroll {
            search.scroll = search.selected;
        } else if search.selected >= search.scroll + rows_visible {
            search.scroll = search.selected + 1 - rows_visible;
        }
        let max_scroll = search.matches.len().saturating_sub(rows_visible);
        search.scroll = search.scroll.min(max_scroll);
        previous != search.scroll
    }

    fn prewarm_search_index(&mut self) {
        self.search_token = self.search_token.wrapping_add(1);
        self.search_loading = true;
        self.search_cache = None;
        let request = SearchRequest {
            token: self.search_token,
            cwd: self.cwd.clone(),
            show_hidden: self.show_hidden,
        };
        if self.search_request_tx.send(request).is_err() {
            self.search_loading = false;
            if let Some(search) = &mut self.search {
                search.loading = false;
                search.error = Some("Search worker unavailable".to_string());
            }
        }
    }

    fn reveal_path(&mut self, path: PathBuf) -> Result<()> {
        if path.is_dir() {
            self.set_dir(path)?;
            self.status = "Opened folder from search".to_string();
            return Ok(());
        }

        let Some(parent) = path.parent() else {
            return Ok(());
        };

        let file_name = path
            .file_name()
            .and_then(OsStr::to_str)
            .map(str::to_string)
            .unwrap_or_default();
        self.set_dir(parent.to_path_buf())?;
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.name == file_name)
        {
            self.select_index(index);
        }
        self.status = format!("Located {}", file_name);
        Ok(())
    }

    fn adjust_zoom(&mut self, delta: i8) {
        let next = (self.zoom_level as i8 + delta).clamp(0, 2) as u8;
        if next == self.zoom_level {
            self.status = format!("Directory zoom limit: {}", self.zoom_level);
            return;
        }
        self.zoom_level = next;
        self.status = format!("Directory zoom set to {}", self.zoom_level);
        self.sync_scroll();
    }

    fn reset_zoom(&mut self) {
        self.zoom_level = 1;
        self.status = format!("Directory zoom reset to {}", self.zoom_level);
        self.sync_scroll();
    }

    fn queue_scroll(lane: &mut ScrollLane, delta: isize) {
        lane.pending = (lane.pending + delta).clamp(-WHEEL_SCROLL_QUEUE_LIMIT, WHEEL_SCROLL_QUEUE_LIMIT);
    }

    fn consume_scroll_step(lane: &mut ScrollLane, cooldown: Duration) -> Option<isize> {
        let now = Instant::now();
        if lane.pending == 0 {
            return None;
        }
        if lane
            .last_step_at
            .is_some_and(|at| now.duration_since(at) < cooldown)
        {
            return None;
        }

        let step = lane.pending.signum();
        lane.pending -= step;
        lane.last_step_at = Some(now);
        Some(step)
    }

    fn flush_entry_vertical_scroll(&mut self) -> bool {
        let Some(step) = Self::consume_scroll_step(
            &mut self.wheel_scroll.vertical,
            WHEEL_SCROLL_INTERVAL_VERTICAL,
        ) else {
            return false;
        };

        let previous = self.selected;
        self.move_vertical(step);
        previous != self.selected
    }

    fn flush_entry_horizontal_scroll(&mut self) -> bool {
        let Some(step) = Self::consume_scroll_step(
            &mut self.wheel_scroll.horizontal,
            WHEEL_SCROLL_INTERVAL_HORIZONTAL,
        ) else {
            return false;
        };

        let previous = self.selected;
        self.move_by(step);
        previous != self.selected
    }

    fn flush_search_scroll(&mut self) -> bool {
        let Some(step) = Self::consume_scroll_step(
            &mut self.wheel_scroll.search,
            WHEEL_SCROLL_INTERVAL_SEARCH,
        ) else {
            return false;
        };

        let previous = self
            .search
            .as_ref()
            .map(|search| search.selected)
            .unwrap_or(0);
        self.move_search_selection(step);
        self.search
            .as_ref()
            .map(|search| search.selected != previous)
            .unwrap_or(false)
    }

    fn clear_wheel_scroll(&mut self) {
        self.wheel_scroll.vertical.pending = 0;
        self.wheel_scroll.vertical.last_step_at = None;
        self.wheel_scroll.horizontal.pending = 0;
        self.wheel_scroll.horizontal.last_step_at = None;
        self.wheel_scroll.search.pending = 0;
        self.wheel_scroll.search.last_step_at = None;
    }

    fn select_index(&mut self, index: usize) {
        self.set_selected(index);
    }

    fn select_last(&mut self) {
        self.set_selected_last();
    }

    fn clamp_selection(&mut self) {
        if self.entries.is_empty() {
            self.selected = 0;
            self.scroll_row = 0;
            self.preview_cache = vec![Line::from("No selection")];
        } else if self.selected >= self.entries.len() {
            self.selected = self.entries.len() - 1;
        }
    }

    fn sync_scroll(&mut self) -> bool {
        let previous = self.scroll_row;
        if self.entries.is_empty() {
            self.scroll_row = 0;
            return previous != self.scroll_row;
        }

        let cols = self.frame_state.metrics.cols.max(1);
        let rows_visible = self.frame_state.metrics.rows_visible.max(1);
        let selected_row = self.selected / cols;
        if selected_row < self.scroll_row {
            self.scroll_row = selected_row;
        } else if selected_row >= self.scroll_row + rows_visible {
            self.scroll_row = selected_row + 1 - rows_visible;
        }
        self.scroll_row = self.scroll_row.min(self.max_scroll_row());
        previous != self.scroll_row
    }

    fn max_scroll_row(&self) -> usize {
        if self.entries.is_empty() {
            return 0;
        }

        let cols = self.frame_state.metrics.cols.max(1);
        let rows_visible = self.frame_state.metrics.rows_visible.max(1);
        let total_rows = self.entries.len().div_ceil(cols);
        total_rows.saturating_sub(rows_visible)
    }

    fn is_double_click(&self, path: &Path) -> bool {
        self.last_click
            .as_ref()
            .is_some_and(|click| click.path == path && click.at.elapsed() <= DOUBLE_CLICK_WINDOW)
    }
}

fn build_preview(entry: &Entry) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            entry.badge().to_string(),
            Style::default()
                .fg(folder_color(entry))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            entry.name.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(format!("Type: {}", entry.kind_label())));
    lines.push(Line::from(format!("Size: {}", format_size(entry.size))));
    lines.push(Line::from(format!(
        "Modified: {}",
        entry
            .modified
            .map(format_time_ago)
            .unwrap_or_else(|| "unknown".to_string())
    )));
    lines.push(Line::from(format!(
        "Permissions: {}",
        if entry.readonly {
            "readonly"
        } else {
            "read/write"
        }
    )));
    lines.push(Line::from(format!(
        "Hidden: {}",
        if entry.hidden { "yes" } else { "no" }
    )));
    lines.push(Line::from(String::new()));

    if entry.is_dir() {
        lines.push(Line::from(Span::styled(
            "Contents",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        match fs::read_dir(&entry.path) {
            Ok(children) => {
                let mut count = 0usize;
                for child in children
                    .flatten()
                    .take(PREVIEW_MAX_LINES.saturating_sub(lines.len()))
                {
                    count += 1;
                    let name = child.file_name().to_string_lossy().to_string();
                    lines.push(Line::from(format!("• {}", name)));
                }
                if count == 0 {
                    lines.push(Line::from("Folder is empty"));
                }
            }
            Err(_) => lines.push(Line::from("Folder preview unavailable")),
        }
        return lines;
    }

    lines.push(Line::from(Span::styled(
        "Preview",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    match read_text_preview(&entry.path) {
        Ok(Some(text)) => {
            for line in text
                .lines()
                .take(PREVIEW_MAX_LINES.saturating_sub(lines.len()))
            {
                lines.push(Line::from(line.to_string()));
            }
            if lines.len() <= 7 {
                lines.push(Line::from("File is empty"));
            }
        }
        Ok(None) => lines.push(Line::from("Binary file or unsupported text encoding")),
        Err(_) => lines.push(Line::from("Preview unavailable")),
    }
    lines
}

impl App {
    fn set_dir(&mut self, path: PathBuf) -> Result<()> {
        self.set_dir_with_history(path, true)
    }

    fn set_dir_with_history(&mut self, path: PathBuf, record_history: bool) -> Result<()> {
        if !path.is_dir() {
            bail!("{} is not a directory", path.display());
        }
        let normalized = path.canonicalize().unwrap_or(path);
        if normalized == self.cwd {
            self.status = format!("Already in {}", self.cwd.display());
            return Ok(());
        }
        if record_history {
            self.back_history.push(self.cwd.clone());
            self.forward_history.clear();
        }
        self.cwd = normalized;
        self.selected = 0;
        self.scroll_row = 0;
        self.reload()?;
        self.status.clear();
        Ok(())
    }

    fn go_parent(&mut self) -> Result<()> {
        let Some(parent) = self.cwd.parent() else {
            self.status = "Already at filesystem root".to_string();
            return Ok(());
        };
        self.set_dir(parent.to_path_buf())
    }

    fn go_home(&mut self) -> Result<()> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/"));
        self.set_dir(home)
    }

    fn go_back(&mut self) -> Result<()> {
        let Some(previous) = self.back_history.pop() else {
            self.status = "No previous folder".to_string();
            return Ok(());
        };
        self.forward_history.push(self.cwd.clone());
        self.set_dir_with_history(previous, false)
    }

    fn go_forward(&mut self) -> Result<()> {
        let Some(next) = self.forward_history.pop() else {
            self.status = "No next folder".to_string();
            return Ok(());
        };
        self.back_history.push(self.cwd.clone());
        self.set_dir_with_history(next, false)
    }

    fn open_in_system(&mut self) -> Result<()> {
        let Some(entry) = self.selected_entry() else {
            return Ok(());
        };

        let target = entry.path.clone();
        for (program, args) in [("gio", vec!["open"]), ("xdg-open", Vec::new())] {
            match detached_open(program, &args, &target) {
                Ok(()) => {
                    self.status = format!("Opened {}", target.display());
                    return Ok(());
                }
                Err(error) => {
                    self.status = format!(
                        "Failed to open {} with {}: {}",
                        target.display(),
                        program,
                        error
                    );
                }
            }
        }
        Ok(())
    }
}

pub fn rect_contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

pub fn format_size(size: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = size as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", size, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn format_time_ago(time: SystemTime) -> String {
    let Ok(age) = SystemTime::now().duration_since(time) else {
        return "just now".to_string();
    };
    let seconds = age.as_secs();
    match seconds {
        0..=59 => format!("{seconds}s ago"),
        60..=3599 => format!("{}m ago", seconds / 60),
        3600..=86_399 => format!("{}h ago", seconds / 3600),
        86_400..=2_592_000 => format!("{}d ago", seconds / 86_400),
        _ => format!("{}mo ago", seconds / 2_592_000),
    }
}

pub fn folder_color(entry: &Entry) -> Color {
    match entry.kind {
        EntryKind::Directory => Color::Rgb(65, 143, 222),
        EntryKind::File => match extension_class(&entry.path) {
            "image" => Color::Rgb(86, 156, 214),
            "audio" => Color::Rgb(138, 110, 214),
            "video" => Color::Rgb(204, 112, 79),
            "archive" => Color::Rgb(191, 142, 74),
            "code" => Color::Rgb(76, 152, 120),
            _ => Color::Rgb(98, 109, 122),
        },
    }
}

fn build_sidebar_items() -> Vec<SidebarItem> {
    let mut items = Vec::new();
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));

    items.push(SidebarItem {
        title: "Home".to_string(),
        icon: "󰋜",
        path: home.clone(),
    });

    for (title, folder, icon) in [
        ("Desktop", "Desktop", "󰟀"),
        ("Documents", "Documents", "󰈙"),
        ("Downloads", "Downloads", "󰉍"),
        ("Pictures", "Pictures", "󰉏"),
        ("Music", "Music", "󱍙"),
        ("Videos", "Videos", "󰕧"),
    ] {
        let path = home.join(folder);
        if path.exists() {
            items.push(SidebarItem {
                title: title.to_string(),
                icon,
                path,
            });
        }
    }

    items.push(SidebarItem {
        title: "Root".to_string(),
        icon: "󰋊",
        path: PathBuf::from("/"),
    });

    items
}

fn read_entries(dir: &Path, show_hidden: bool) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for item in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let item = item?;
        let path = item.path();
        let file_name = item.file_name();
        let name = file_name.to_string_lossy().to_string();
        let name_key = name.to_lowercase();
        let hidden = is_hidden(file_name.as_os_str());
        if hidden && !show_hidden {
            continue;
        }

        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to read metadata for {}", path.display()))?;
        let kind = if metadata.is_dir() {
            EntryKind::Directory
        } else {
            EntryKind::File
        };
        let size = if metadata.is_file() {
            metadata.len()
        } else {
            0
        };
        entries.push(Entry {
            path,
            name,
            name_key,
            kind,
            size,
            modified: metadata.modified().ok(),
            readonly: metadata.permissions().readonly(),
            hidden,
        });
    }
    Ok(entries)
}

fn sort_entries(entries: &mut [Entry], mode: SortMode) {
    entries.sort_by(|left, right| match (left.is_dir(), right.is_dir()) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => match mode {
            SortMode::Name => left.name_key.cmp(&right.name_key),
            SortMode::Modified => right
                .modified
                .cmp(&left.modified)
                .then_with(|| left.name_key.cmp(&right.name_key)),
            SortMode::Size => right
                .size
                .cmp(&left.size)
                .then_with(|| left.name_key.cmp(&right.name_key)),
        },
    });
}

fn spawn_search_worker(
    request_rx: mpsc::Receiver<SearchRequest>,
    result_tx: mpsc::Sender<SearchBuild>,
) {
    thread::spawn(move || {
        while let Ok(mut request) = request_rx.recv() {
            while let Ok(next_request) = request_rx.try_recv() {
                request = next_request;
            }

            let SearchRequest {
                token,
                cwd,
                show_hidden,
            } = request;
            let result = crate::search::collect_candidates(&cwd, show_hidden)
                .map(Arc::new)
                .map_err(|error| error.to_string());
            if result_tx
                .send(SearchBuild {
                    token,
                    cwd,
                    show_hidden,
                    result,
                })
                .is_err()
            {
                break;
            }
        }
    });
}

fn prune_search_cache(cached_matches: &mut HashMap<String, Vec<usize>>, active_query: &str) {
    if cached_matches.len() < SEARCH_CACHE_LIMIT {
        return;
    }

    cached_matches.retain(|query, _| {
        query.is_empty() || active_query.starts_with(query) || query.starts_with(active_query)
    });

    while cached_matches.len() >= SEARCH_CACHE_LIMIT {
        let Some(stale_key) = cached_matches
            .keys()
            .filter(|query| !query.is_empty())
            .max_by_key(|query| query.len())
            .cloned()
        else {
            break;
        };
        cached_matches.remove(&stale_key);
    }
}

fn is_hidden(file_name: &OsStr) -> bool {
    file_name.to_string_lossy().starts_with('.')
}

fn detached_open(program: &str, args: &[&str], target: &Path) -> std::io::Result<()> {
    let mut command = Command::new(program);
    command.args(args);
    command.arg(target);
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    command.spawn()?;
    Ok(())
}

fn extension_class(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" | "js" | "ts" | "tsx" | "jsx" | "py" | "go" | "c" | "cpp" | "h" | "java" | "json"
        | "toml" | "yaml" | "yml" | "md" | "sh" => "code",
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "avif" => "image",
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => "audio",
        "mp4" | "mkv" | "mov" | "webm" | "avi" => "video",
        "zip" | "tar" | "gz" | "xz" | "bz2" | "7z" => "archive",
        "txt" | "log" | "ini" | "csv" => "text",
        _ => "file",
    }
}

fn read_text_preview(path: &Path) -> Result<Option<String>> {
    let mut file = File::open(path)?;
    let mut buffer = vec![0; PREVIEW_LIMIT_BYTES];
    let count = file.read(&mut buffer)?;
    buffer.truncate(count);

    if buffer.is_empty() {
        return Ok(Some(String::new()));
    }
    if buffer.contains(&0) {
        return Ok(None);
    }

    match String::from_utf8(buffer) {
        Ok(text) => Ok(Some(text)),
        Err(_) => Ok(None),
    }
}

fn search_scope_indices(candidates: &[SearchCandidate], scope: SearchScope) -> Vec<usize> {
    candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| scope.accepts(candidate).then_some(index))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        env::temp_dir().join(format!("elio-{label}-{unique}"))
    }

    #[test]
    fn reload_filters_hidden_files_by_default() {
        let root = temp_path("hidden");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("visible.txt"), "hello").expect("failed to write visible file");
        fs::write(root.join(".secret"), "hidden").expect("failed to write hidden file");

        let app = App::new_at(root.clone()).expect("failed to create app");
        assert_eq!(app.entries.len(), 1);
        assert_eq!(app.entries[0].name, "visible.txt");

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn sort_keeps_directories_before_files() {
        let mut entries = vec![
            Entry {
                path: PathBuf::from("beta.txt"),
                name: "beta.txt".to_string(),
                name_key: "beta.txt".to_string(),
                kind: EntryKind::File,
                size: 10,
                modified: None,
                readonly: false,
                hidden: false,
            },
            Entry {
                path: PathBuf::from("alpha"),
                name: "alpha".to_string(),
                name_key: "alpha".to_string(),
                kind: EntryKind::Directory,
                size: 0,
                modified: None,
                readonly: false,
                hidden: false,
            },
        ];

        sort_entries(&mut entries, SortMode::Name);
        assert!(entries[0].is_dir());
        assert!(!entries[1].is_dir());
    }

    #[test]
    fn size_format_is_human_readable() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2048), "2.0 KB");
    }

    #[test]
    fn zoom_is_clamped() {
        let root = temp_path("zoom");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("one.txt"), "hello").expect("failed to write temp file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        assert_eq!(app.zoom_level, 1);

        app.adjust_zoom(10);
        assert_eq!(app.zoom_level, 2);

        app.adjust_zoom(-10);
        assert_eq!(app.zoom_level, 0);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
