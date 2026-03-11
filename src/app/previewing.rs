use super::*;
use std::{collections::HashMap, sync::{Arc, mpsc}, thread};

impl App {
    pub fn process_background_jobs(&mut self) -> bool {
        let mut dirty = false;

        while let Ok(build) = self.search_rx.try_recv() {
            if build.token != self.search_token || build.cwd != self.cwd {
                continue;
            }

            self.search_loading = false;
            dirty = true;

            match build.result {
                Ok(candidates) => {
                    self.search_cache = Some(SearchCache {
                        cwd: build.cwd,
                        scope: build.scope,
                        candidates: candidates.clone(),
                    });
                    if let Some(search) = &mut self.search
                        && search.scope == build.scope
                    {
                        search.candidates = candidates;
                        search.cached_matches = HashMap::from([(
                            String::new(),
                            (0..search.candidates.len()).collect(),
                        )]);
                        search.loading = false;
                        search.error = None;
                    }
                    self.refresh_search_matches("");
                }
                Err(error) => {
                    self.search_cache = None;
                    if let Some(search) = &mut self.search
                        && search.scope == build.scope
                    {
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

        while let Ok(build) = self.preview_rx.try_recv() {
            self.cache_preview_result(&build.entry, &build.result);
            let is_current_entry = self
                .selected_entry()
                .map(|entry| {
                    entry.path == build.entry.path
                        && entry.modified == build.entry.modified
                        && entry.size == build.entry.size
                })
                .unwrap_or(false);
            if build.token != self.preview_token || !is_current_entry {
                continue;
            }

            self.preview_cache = build.result;
            dirty = true;
        }

        dirty
    }
}

pub(super) fn spawn_preview_worker(
    request_rx: mpsc::Receiver<PreviewRequest>,
    result_tx: mpsc::Sender<PreviewBuild>,
) {
    thread::spawn(move || {
        while let Ok(mut request) = request_rx.recv() {
            while let Ok(next_request) = request_rx.try_recv() {
                request = next_request;
            }

            let PreviewRequest { token, entry } = request;
            let result = preview::build_preview(&entry);
            if result_tx
                .send(PreviewBuild {
                    token,
                    entry,
                    result,
                })
                .is_err()
            {
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        fs::File,
        io::Write,
        path::{Path, PathBuf},
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-preview-worker-{label}-{unique}"))
    }

    fn write_zip_entries(path: &Path, entries: &[(&str, &str)]) {
        let file = File::create(path).expect("failed to create zip");
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        for (name, contents) in entries {
            zip.start_file(name, options)
                .expect("failed to start zip entry");
            zip.write_all(contents.as_bytes())
                .expect("failed to write zip entry");
        }

        zip.finish().expect("failed to finish zip");
    }

    fn wait_for_background_preview(app: &mut App) {
        for _ in 0..100 {
            if app.process_background_jobs() {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for background preview");
    }

    #[test]
    fn archive_preview_loads_in_background() {
        let root = temp_path("archive-background");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let archive = root.join("bundle.zip");
        write_zip_entries(&archive, &[("docs/readme.txt", "hello")]);

        let mut app = App::new_at(root.clone()).expect("failed to create app");

        assert_eq!(app.preview_section_label(), "Preview");
        assert_eq!(app.preview_header_detail(10).as_deref(), Some("ZIP archive"));
        assert!(
            app.preview_lines()
                .iter()
                .any(|line| line.to_string().contains("Loading preview"))
        );

        wait_for_background_preview(&mut app);

        assert_eq!(app.preview_section_label(), "Archive");
        assert_eq!(app.preview_header_detail(10).as_deref(), Some("ZIP archive"));
        assert!(
            app.preview_lines()
                .iter()
                .any(|line| line.to_string().contains("docs/"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn stale_archive_preview_result_is_ignored_after_selection_changes() {
        let root = temp_path("archive-stale");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let archive = root.join("a.zip");
        write_zip_entries(&archive, &[("docs/readme.txt", "hello")]);
        let text = root.join("z.txt");
        fs::write(&text, "plain text").expect("failed to write text file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        assert_eq!(
            app.selected_entry().map(|entry| entry.name.as_str()),
            Some("a.zip")
        );

        app.set_selected(1);
        assert_eq!(
            app.selected_entry().map(|entry| entry.name.as_str()),
            Some("z.txt")
        );

        thread::sleep(Duration::from_millis(50));
        let dirty = app.process_background_jobs();

        assert_eq!(app.preview_section_label(), "Text");
        assert!(!dirty);
        assert!(
            app.preview_lines()
                .iter()
                .any(|line| line.to_string().contains("plain text"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn archive_preview_is_reused_from_cache_on_reselection() {
        let root = temp_path("archive-cache");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let archive = root.join("a.zip");
        write_zip_entries(&archive, &[("docs/readme.txt", "hello")]);
        let text = root.join("z.txt");
        fs::write(&text, "plain text").expect("failed to write text file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        wait_for_background_preview(&mut app);

        app.set_selected(1);
        assert_eq!(app.preview_section_label(), "Text");

        app.set_selected(0);
        assert_eq!(app.preview_section_label(), "Archive");
        assert_eq!(app.preview_header_detail(10).as_deref(), Some("ZIP archive"));
        assert!(
            app.preview_lines()
                .iter()
                .any(|line| line.to_string().contains("docs/"))
        );
        assert!(
            app.preview_lines()
                .iter()
                .all(|line| !line.to_string().contains("Loading preview"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
