//! Kitty-protocol thumbnails for the grid file browser view.
//!
//! Grid view traditionally only shows a file-type icon. This module renders a
//! real image preview inside every visible grid tile when the terminal speaks
//! the Kitty graphics protocol (Kitty, Ghostty, Rio). Thumbnails are generated
//! off the UI thread through the same preparation pipeline used by the preview
//! pane, so resizing, EXIF orientation, SVG/HEIF rendering and on-disk caching
//! behave identically.
//!
//! Rendering strategy: images are uploaded once and then displayed through
//! Kitty's unicode placeholder extension. Placeholder cells are ordinary
//! terminal cells, which means:
//!
//! * ratatui can keep compositing its buffer normally (the image never covers
//!   the filename band or popups), and
//! * erasing the cells before a draw removes the image without a full-screen
//!   repaint.
//!
//! The preview pane keeps its own image id, so clearing the preview overlay no
//! longer wipes the grid's thumbnails.

use crate::app::App;
use crate::file_browser::ViewMode;
use crate::file_classification::FileClass;
use crate::filesystem::Entry;
use crate::preview::images::{ImagePrepareRequest, prepare_static_image_asset};
use crate::terminal_images::{
    self, ImageProtocol, RenderedImageDimensions, area_pixel_size, fit_image_area, intersect_rect,
    read_png_dimensions,
};
use ratatui::{layout::Rect, style::Color};
use std::{
    collections::{HashMap, HashSet, VecDeque, hash_map::DefaultHasher},
    env, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

const THUMBNAIL_WORKERS: usize = 2;
/// Upper bound on terminal-side image ids. Evicting an id only costs one
/// re-upload if that file becomes visible again.
const MAX_LIVE_IMAGES: usize = 1024;
const MIN_THUMBNAIL_WIDTH: u16 = 5;
const MIN_THUMBNAIL_HEIGHT: u16 = 2;
const VIDEO_THUMBNAIL_CACHE_VERSION: usize = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ThumbnailSource {
    Image,
    Video,
}

fn thumbnail_source(entry: &Entry) -> Option<ThumbnailSource> {
    match crate::file_classification::inspect_entry_cached(entry).builtin_class {
        FileClass::Image => Some(ThumbnailSource::Image),
        FileClass::Video => Some(ThumbnailSource::Video),
        _ => None,
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ThumbnailKey {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    target_width_px: u32,
    target_height_px: u32,
}

impl ThumbnailKey {
    fn new(entry: &Entry, target_width_px: u32, target_height_px: u32) -> Self {
        Self {
            path: entry.path.clone(),
            size: entry.size,
            modified: entry.modified,
            target_width_px: target_width_px.max(1),
            target_height_px: target_height_px.max(1),
        }
    }
}

struct ReadyThumbnail {
    display_path: PathBuf,
    dimensions: RenderedImageDimensions,
}

enum ThumbnailStatus {
    Pending,
    Ready(ReadyThumbnail),
    Failed,
}

struct ThumbnailJob {
    key: ThumbnailKey,
    source: ThumbnailSource,
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    target_width_px: u32,
    target_height_px: u32,
}

struct ThumbnailDone {
    key: ThumbnailKey,
    ready: Option<ReadyThumbnail>,
}

struct ThumbnailWorkers {
    submit: Sender<ThumbnailJob>,
    done: Receiver<ThumbnailDone>,
}

impl ThumbnailWorkers {
    fn start() -> Self {
        let (submit, job_queue) = mpsc::channel::<ThumbnailJob>();
        let job_queue = Arc::new(Mutex::new(job_queue));
        let (done_sender, done) = mpsc::channel::<ThumbnailDone>();

        for index in 0..THUMBNAIL_WORKERS {
            let job_queue = Arc::clone(&job_queue);
            let done_sender = done_sender.clone();
            let spawned = thread::Builder::new()
                .name(format!("elio-grid-thumb-{index}"))
                .spawn(move || {
                    loop {
                        let job = match job_queue.lock() {
                            Ok(queue) => match queue.recv() {
                                Ok(job) => job,
                                Err(_) => break,
                            },
                            Err(_) => break,
                        };
                        let key = job.key.clone();
                        let ready = build_thumbnail(job);
                        if done_sender.send(ThumbnailDone { key, ready }).is_err() {
                            break;
                        }
                    }
                });
            if spawned.is_err() {
                break;
            }
        }

        Self { submit, done }
    }
}

fn build_thumbnail(job: ThumbnailJob) -> Option<ReadyThumbnail> {
    match job.source {
        ThumbnailSource::Image => build_image_thumbnail(job),
        ThumbnailSource::Video => build_video_thumbnail(&job),
    }
}

fn build_image_thumbnail(job: ThumbnailJob) -> Option<ReadyThumbnail> {
    let request = ImagePrepareRequest {
        path: job.path,
        size: job.size,
        modified: job.modified,
        target_width_px: job.target_width_px,
        target_height_px: job.target_height_px,
        ffmpeg_available: terminal_images::command_exists("ffmpeg"),
        resvg_available: terminal_images::command_exists("resvg"),
        magick_available: terminal_images::command_exists("magick"),
        force_render_to_cache: true,
        prepare_inline_payload: false,
        sixel_prepare: None,
    };
    let asset = prepare_static_image_asset(&request, || false)?;
    if asset.dimensions.width_px == 0 || asset.dimensions.height_px == 0 {
        return None;
    }
    Some(ReadyThumbnail {
        display_path: asset.display_path,
        dimensions: asset.dimensions,
    })
}

fn build_video_thumbnail(job: &ThumbnailJob) -> Option<ReadyThumbnail> {
    if !terminal_images::command_exists("ffmpeg") {
        return None;
    }
    let cache_path = video_thumbnail_cache_path(job)?;
    if !cache_path.exists() {
        let temp_path = cache_path.with_extension(format!("tmp-{}.png", std::process::id()));
        if !render_video_frame(
            &job.path,
            &temp_path,
            job.target_width_px,
            job.target_height_px,
        ) {
            let _ = fs::remove_file(&temp_path);
            return None;
        }
        // Publish atomically: another worker (or a later session) may be
        // reading the cache while this one finishes.
        if fs::rename(&temp_path, &cache_path).is_err() {
            let _ = fs::remove_file(&temp_path);
            return None;
        }
    }
    let dimensions = read_png_dimensions(&cache_path)?;
    Some(ReadyThumbnail {
        display_path: cache_path,
        dimensions,
    })
}

fn render_video_frame(path: &Path, output: &Path, max_width: u32, max_height: u32) -> bool {
    let filter = format!(
        "scale={}:{}:force_original_aspect_ratio=decrease",
        max_width.max(1),
        max_height.max(1)
    );
    // One second in is usually more representative than the first frame, which
    // is often black. Very short clips fall back to the very beginning.
    for timestamp in ["1", "0"] {
        let status = Command::new("ffmpeg")
            .args(["-loglevel", "error", "-y", "-ss", timestamp, "-i"])
            .arg(path)
            .args(["-frames:v", "1", "-vf", &filter])
            .arg(output)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if status.is_ok_and(|status| status.success()) && output.exists() {
            return true;
        }
        let _ = fs::remove_file(output);
    }
    false
}

fn video_thumbnail_cache_path(job: &ThumbnailJob) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    VIDEO_THUMBNAIL_CACHE_VERSION.hash(&mut hasher);
    job.path.hash(&mut hasher);
    job.size.hash(&mut hasher);
    job.modified
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|elapsed| elapsed.as_nanos())
        .hash(&mut hasher);
    job.target_width_px.hash(&mut hasher);
    job.target_height_px.hash(&mut hasher);

    let cache_dir = env::temp_dir().join(format!(
        "elio-grid-video-thumb-v{VIDEO_THUMBNAIL_CACHE_VERSION}"
    ));
    fs::create_dir_all(&cache_dir).ok()?;
    Some(cache_dir.join(format!("thumb-{:016x}.png", hasher.finish())))
}

pub(crate) struct GridThumbnails {
    workers: Option<ThumbnailWorkers>,
    statuses: HashMap<ThumbnailKey, ThumbnailStatus>,
    ids: HashMap<ThumbnailKey, u32>,
    id_order: VecDeque<ThumbnailKey>,
    transmitted: HashSet<u32>,
    pending_deletes: Vec<u32>,
    /// Tile bodies that currently contain placeholder cells, together with the
    /// background colour they were drawn on, so they can be erased before the
    /// next ratatui draw without a colour jump.
    displayed_areas: Vec<(Rect, Color)>,
    next_id: u32,
}

impl Default for GridThumbnails {
    fn default() -> Self {
        Self {
            workers: None,
            statuses: HashMap::new(),
            ids: HashMap::new(),
            id_order: VecDeque::new(),
            transmitted: HashSet::new(),
            pending_deletes: Vec::new(),
            displayed_areas: Vec::new(),
            // Keep the first id away from 0, which some terminals treat as "unset".
            next_id: 1,
        }
    }
}

impl GridThumbnails {
    fn ensure_workers(&mut self) {
        if self.workers.is_none() {
            self.workers = Some(ThumbnailWorkers::start());
        }
    }

    fn request(&mut self, key: &ThumbnailKey, entry: &Entry, source: ThumbnailSource) -> bool {
        if let Some(ThumbnailStatus::Ready(_)) = self.statuses.get(key) {
            return true;
        }
        if self.statuses.contains_key(key) {
            return false;
        }

        self.ensure_workers();
        let job = ThumbnailJob {
            key: key.clone(),
            source,
            path: entry.path.clone(),
            size: entry.size,
            modified: entry.modified,
            target_width_px: key.target_width_px,
            target_height_px: key.target_height_px,
        };
        self.statuses.insert(key.clone(), ThumbnailStatus::Pending);
        let sent = self
            .workers
            .as_ref()
            .is_some_and(|workers| workers.submit.send(job).is_ok());
        if !sent {
            self.statuses.insert(key.clone(), ThumbnailStatus::Failed);
        }
        false
    }

    fn ready(&self, key: &ThumbnailKey) -> Option<&ReadyThumbnail> {
        match self.statuses.get(key) {
            Some(ThumbnailStatus::Ready(ready)) => Some(ready),
            _ => None,
        }
    }

    fn id_for(&mut self, key: &ThumbnailKey) -> u32 {
        if let Some(id) = self.ids.get(key) {
            return *id;
        }

        let preview_id = terminal_images::kitty_preview_image_id();
        let id = loop {
            let candidate = self.next_id;
            self.next_id = if self.next_id >= 0xff_ffff {
                1
            } else {
                self.next_id + 1
            };
            if candidate != 0 && candidate != preview_id {
                break candidate;
            }
        };

        self.ids.insert(key.clone(), id);
        self.id_order.push_back(key.clone());
        self.evict_old_ids();
        id
    }

    fn evict_old_ids(&mut self) {
        while self.id_order.len() > MAX_LIVE_IMAGES {
            let Some(key) = self.id_order.pop_front() else {
                break;
            };
            if let Some(id) = self.ids.remove(&key) {
                self.transmitted.remove(&id);
                self.pending_deletes.push(id);
            }
        }
    }

    fn take_deletes(&mut self) -> Vec<u8> {
        let ids = std::mem::take(&mut self.pending_deletes);
        ids.into_iter()
            .flat_map(terminal_images::delete_kitty_thumbnail)
            .collect()
    }

    fn resolve_results(&mut self) -> bool {
        let Some(workers) = self.workers.as_ref() else {
            return false;
        };
        let mut results = Vec::new();
        while let Ok(done) = workers.done.try_recv() {
            results.push(done);
        }
        let changed = !results.is_empty();
        for done in results {
            self.statuses.insert(
                done.key,
                match done.ready {
                    Some(ready) => ThumbnailStatus::Ready(ready),
                    None => ThumbnailStatus::Failed,
                },
            );
        }
        changed
    }
}

impl App {
    fn grid_thumbnails_visible(&self) -> bool {
        self.preview.terminal_images.protocol == ImageProtocol::KittyGraphics
            && self.file_browser.view_mode == ViewMode::Grid
            && !self.preview.fullscreen
    }

    /// Whether the tile body should reserve its space for an image instead of
    /// metadata text. Called from the renderer, so it must stay cheap and
    /// side-effect free.
    pub(crate) fn grid_thumbnail_ready_for(&self, entry: &Entry, body: Rect) -> bool {
        if !self.grid_thumbnails_visible() || !thumbnail_area_big_enough(body) {
            return false;
        }
        if thumbnail_source(entry).is_none() {
            return false;
        }
        let Some(window) = self.cached_terminal_window() else {
            return false;
        };
        let (target_width_px, target_height_px) = area_pixel_size(body, window);
        let key = ThumbnailKey::new(entry, target_width_px, target_height_px);
        self.grid_thumbnails.ready(&key).is_some()
    }

    /// Processes finished thumbnail jobs. Returns true when the grid needs a
    /// redraw to show newly available images.
    pub(crate) fn process_grid_thumbnail_results(&mut self) -> bool {
        self.grid_thumbnails.resolve_results()
    }

    /// Erases placeholder cells from the previous frame so ratatui's
    /// differential renderer cannot leave stale images behind. Must be written
    /// before `terminal.draw()`.
    pub(crate) fn grid_thumbnail_pre_draw_erase(&mut self) -> Vec<u8> {
        if self.preview.terminal_images.protocol != ImageProtocol::KittyGraphics {
            self.grid_thumbnails.displayed_areas.clear();
            return Vec::new();
        }
        let areas = std::mem::take(&mut self.grid_thumbnails.displayed_areas);
        if areas.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (area, background) in areas {
            out.extend(erase_cells_with_background(area, background));
        }
        out
    }

    /// Uploads missing thumbnails and writes their placeholder cells. Must be
    /// written after `terminal.draw()`.
    pub(crate) fn present_grid_thumbnails(&mut self) -> Vec<u8> {
        if !self.grid_thumbnails_visible() {
            self.grid_thumbnails.displayed_areas.clear();
            return Vec::new();
        }
        let Some(window) = self.cached_terminal_window() else {
            self.grid_thumbnails.displayed_areas.clear();
            return Vec::new();
        };
        let popups = self.collect_popup_rects();
        let hits = self.input.screen_regions.entry_hits.clone();
        let entries = &self.file_browser.entries;
        let selected = self.file_browser.selected;
        let palette = crate::theme::palette();
        let grid = &mut self.grid_thumbnails;

        let mut out = Vec::new();
        let mut displayed_areas = Vec::new();

        for hit in hits {
            let Some(entry) = entries.get(hit.index) else {
                continue;
            };
            let body = thumbnail_body_rect(hit.rect);
            let Some(source) = thumbnail_source(entry).filter(|_| thumbnail_area_big_enough(body))
            else {
                continue;
            };
            if popups
                .iter()
                .any(|popup| intersect_rect(*popup, body).is_some())
            {
                continue;
            }

            let content_bg = if hit.index == selected {
                crate::ui::focused_tile_content_bg(palette.selected_bg)
            } else {
                palette.surface
            };
            let (target_width_px, target_height_px) = area_pixel_size(body, window);
            let key = ThumbnailKey::new(entry, target_width_px, target_height_px);
            if !grid.request(&key, entry, source) {
                continue;
            }
            let Some(ready) = grid.ready(&key) else {
                continue;
            };
            let aspect_ratio =
                ready.dimensions.width_px as f32 / ready.dimensions.height_px.max(1) as f32;
            let placement = fit_image_area(body, window, aspect_ratio);
            let display_path = ready.display_path.clone();

            let id = grid.id_for(&key);
            if !grid.transmitted.contains(&id) {
                match terminal_images::upload_kitty_thumbnail(&display_path, id, placement) {
                    Ok(bytes) => {
                        out.extend(bytes);
                        grid.transmitted.insert(id);
                    }
                    Err(_) => {
                        grid.statuses.insert(key, ThumbnailStatus::Failed);
                        grid.transmitted.remove(&id);
                        continue;
                    }
                }
            }
            out.extend(terminal_images::kitty_thumbnail_placeholders(
                id,
                placement,
                &popups,
                Some(content_bg),
            ));
            displayed_areas.push((body, content_bg));
        }

        grid.displayed_areas = displayed_areas;
        out.extend(grid.take_deletes());
        out
    }
}

fn thumbnail_body_rect(tile: Rect) -> Rect {
    Rect {
        x: tile.x,
        y: tile.y.saturating_add(1),
        width: tile.width,
        height: tile.height.saturating_sub(1),
    }
}

fn thumbnail_area_big_enough(area: Rect) -> bool {
    area.width >= MIN_THUMBNAIL_WIDTH && area.height >= MIN_THUMBNAIL_HEIGHT
}

fn erase_cells_with_background(area: Rect, background: Color) -> Vec<u8> {
    use std::fmt::Write as _;

    let mut out = String::new();
    match background {
        Color::Rgb(red, green, blue) => {
            let _ = write!(out, "\x1b[0;48;2;{red};{green};{blue}m");
        }
        _ => {
            let _ = write!(out, "\x1b[0m");
        }
    }
    let blank_row = " ".repeat(usize::from(area.width));
    for row in 0..area.height {
        let _ = write!(
            out,
            "\x1b[{};{}H{}",
            area.y.saturating_add(1).saturating_add(row),
            area.x.saturating_add(1),
            blank_row
        );
    }
    let _ = write!(out, "\x1b[0m");
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, size: u64) -> Entry {
        Entry {
            path: PathBuf::from(path),
            size,
            ..Entry::default()
        }
    }

    #[test]
    fn key_separates_versions_and_target_sizes() {
        let base = entry("/tmp/a.png", 10);
        let key = ThumbnailKey::new(&base, 100, 50);
        assert_ne!(key, ThumbnailKey::new(&entry("/tmp/a.png", 11), 100, 50));
        assert_ne!(key, ThumbnailKey::new(&base, 200, 50));
    }

    #[test]
    fn body_rect_excludes_the_filename_band() {
        let body = thumbnail_body_rect(Rect {
            x: 4,
            y: 7,
            width: 20,
            height: 5,
        });
        assert_eq!(
            body,
            Rect {
                x: 4,
                y: 8,
                width: 20,
                height: 4
            }
        );
    }

    #[test]
    fn tiny_tiles_do_not_reserve_space_for_thumbnails() {
        assert!(!thumbnail_area_big_enough(Rect {
            x: 0,
            y: 0,
            width: 4,
            height: 3
        }));
        assert!(!thumbnail_area_big_enough(Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 1
        }));
        assert!(thumbnail_area_big_enough(Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 2
        }));
    }

    fn video_job(path: &str, width_px: u32) -> ThumbnailJob {
        let entry = entry(path, 42);
        ThumbnailJob {
            key: ThumbnailKey::new(&entry, width_px, 50),
            source: ThumbnailSource::Video,
            path: entry.path,
            size: entry.size,
            modified: None,
            target_width_px: width_px,
            target_height_px: 50,
        }
    }

    #[test]
    fn video_cache_path_is_stable_and_layout_dependent() {
        let first =
            video_thumbnail_cache_path(&video_job("/tmp/movie.mp4", 120)).expect("temp cache path");
        let same =
            video_thumbnail_cache_path(&video_job("/tmp/movie.mp4", 120)).expect("temp cache path");
        let larger =
            video_thumbnail_cache_path(&video_job("/tmp/movie.mp4", 240)).expect("temp cache path");
        assert_eq!(first, same);
        assert_ne!(first, larger);
    }

    #[test]
    fn eviction_queues_a_delete_for_the_evicted_image() {
        let mut grid = GridThumbnails::default();
        for index in 0..=MAX_LIVE_IMAGES {
            let key = ThumbnailKey::new(&entry(&format!("/tmp/{index}.png"), 1), 10, 10);
            let id = grid.id_for(&key);
            grid.transmitted.insert(id);
        }
        let deletes = grid.take_deletes();
        let text = String::from_utf8(deletes).expect("kitty delete bytes are utf8");
        assert!(text.contains("a=d,d=I"));
        assert_eq!(grid.ids.len(), MAX_LIVE_IMAGES);
    }
}
