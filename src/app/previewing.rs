use super::*;
use crate::app::jobs::JobResult;
use std::{collections::HashMap, sync::Arc};

impl App {
    pub fn process_background_jobs(&mut self) -> bool {
        let mut dirty = false;

        while let Ok(job) = self.scheduler.try_recv() {
            match job {
                JobResult::Directory(build) => {
                    let Some(load) = self.pending_directory_load.clone() else {
                        continue;
                    };
                    if build.token != self.directory_token
                        || build.token != load.token
                        || build.cwd != load.target_cwd
                    {
                        continue;
                    }

                    self.pending_directory_load = None;
                    dirty = true;

                    match build.result {
                        Ok(snapshot) => self.apply_directory_snapshot(load, snapshot),
                        Err(error) => {
                            self.status = format!("Cannot open {}: {}", build.cwd.display(), error);
                        }
                    }
                }
                JobResult::Search(build) => {
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
                                search.cached_matches =
                                    HashMap::from([(String::new(), Vec::new())]);
                                search.selected = 0;
                                search.scroll = 0;
                                search.loading = false;
                                search.error = Some(error);
                            }
                        }
                    }
                }
                JobResult::Preview(build) => {
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
                        self.preview_metrics.stale_results_dropped += 1;
                        continue;
                    }

                    self.preview_cache = build.result;
                    self.preview_metrics.applied_results += 1;
                    dirty = true;
                }
            }
        }

        dirty
    }
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

    fn write_docx_fixture(path: &Path) {
        write_zip_entries(
            path,
            &[
                (
                    "docProps/core.xml",
                    r#"<?xml version="1.0" encoding="UTF-8"?>
                    <cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                        xmlns:dc="http://purl.org/dc/elements/1.1/"
                        xmlns:dcterms="http://purl.org/dc/terms/">
                      <dc:title>Quarterly Report</dc:title>
                      <dc:creator>Regueiro</dc:creator>
                      <dcterms:created>2026-03-11T09:00:00Z</dcterms:created>
                    </cp:coreProperties>"#,
                ),
                (
                    "docProps/app.xml",
                    r#"<?xml version="1.0" encoding="UTF-8"?>
                    <Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
                      <Application>LibreOffice</Application>
                      <Pages>12</Pages>
                      <Words>4238</Words>
                    </Properties>"#,
                ),
            ],
        );
    }

    #[test]
    fn archive_preview_loads_in_background() {
        let root = temp_path("archive-background");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let archive = root.join("bundle.zip");
        write_zip_entries(&archive, &[("docs/readme.txt", "hello")]);

        let mut app = App::new_at(root.clone()).expect("failed to create app");

        assert_eq!(app.preview_section_label(), "Preview");
        assert_eq!(
            app.preview_header_detail(10).as_deref(),
            Some("ZIP archive")
        );
        assert!(
            app.preview_lines()
                .iter()
                .any(|line| line.to_string().contains("Loading preview"))
        );

        wait_for_background_preview(&mut app);

        assert_eq!(app.preview_section_label(), "Archive");
        assert_eq!(
            app.preview_header_detail(10).as_deref(),
            Some("ZIP archive")
        );
        assert!(
            app.preview_lines()
                .iter()
                .any(|line| line.to_string().contains("docs/"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn document_preview_loads_in_background() {
        let root = temp_path("document-background");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let document = root.join("a.docx");
        write_docx_fixture(&document);

        let mut app = App::new_at(root.clone()).expect("failed to create app");

        assert_eq!(app.preview_section_label(), "Preview");
        assert_eq!(
            app.preview_header_detail(10).as_deref(),
            Some("DOCX document")
        );
        assert!(app.preview_lines().iter().any(|line| {
            line.to_string()
                .contains("Extracting document metadata in background")
        }));

        wait_for_background_preview(&mut app);

        assert_eq!(app.preview_section_label(), "Document");
        assert_eq!(
            app.preview_header_detail(10).as_deref(),
            Some("DOCX document")
        );
        assert!(
            app.preview_lines()
                .iter()
                .any(|line| line.to_string().contains("Quarterly Report"))
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn nearby_archive_preview_is_prefetched_at_low_priority() {
        let root = temp_path("archive-prefetch");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let first = root.join("a.zip");
        let second = root.join("b.zip");
        write_zip_entries(&first, &[("docs/first.txt", "hello")]);
        write_zip_entries(&second, &[("docs/second.txt", "world")]);

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        for _ in 0..100 {
            let _ = app.process_background_jobs();
            if app.has_cached_preview_for_path(&second) {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(app.has_cached_preview_for_path(&second));
        let scheduler_metrics = app.scheduler_metrics();
        assert!(scheduler_metrics.preview_jobs_submitted_high >= 1);
        assert!(scheduler_metrics.preview_jobs_submitted_low >= 1);

        app.set_selected(1);
        assert_eq!(app.preview_section_label(), "Archive");
        assert!(
            app.preview_lines()
                .iter()
                .all(|line| !line.to_string().contains("Loading preview"))
        );
        assert!(
            app.preview_lines()
                .iter()
                .any(|line| line.to_string().contains("second.txt"))
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
        assert_eq!(
            app.preview_header_detail(10).as_deref(),
            Some("ZIP archive")
        );
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
        let metrics = app.preview_metrics();
        assert_eq!(metrics.cache_hits, 1);
        assert_eq!(metrics.cache_misses, 1);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn stale_preview_results_are_counted_in_metrics() {
        let root = temp_path("archive-stale-metrics");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let archive = root.join("a.zip");
        write_zip_entries(&archive, &[("docs/readme.txt", "hello")]);
        let text = root.join("z.txt");
        fs::write(&text, "plain text").expect("failed to write text file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.set_selected(1);

        thread::sleep(Duration::from_millis(50));
        let _ = app.process_background_jobs();

        let metrics = app.preview_metrics();
        assert!(metrics.stale_results_dropped >= 1);
        assert_eq!(metrics.applied_results, 0);

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
