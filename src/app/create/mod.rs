mod bulk_rename;
mod editing;
mod new_file;
mod rename;
mod restore;
mod trash;
mod validation;

#[cfg(test)]
mod tests {
    use super::super::{App, state::DirectoryLoadCompletion};
    use super::rename;
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    /// Drive background jobs until both the trash worker and the subsequent
    /// directory reload have both completed.  Checking only `trash_progress`
    /// is not enough: a single `process_background_jobs` call can consume
    /// the `Trash(done=true)` result *and* the immediately-queued
    /// `Directory` reload in the same batch (a tiny directory scan completes
    /// before the loop's next `try_recv`).  Driving until `pending_load` is
    /// also gone guarantees that `app.status_message()` holds the final
    /// status in all cases.
    fn wait_for_trash_and_reload(app: &mut App) {
        for _ in 0..500 {
            let _ = app.process_background_jobs();
            if app.trash_progress.is_none() && app.directory_runtime.pending_load.is_none() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for trash and directory reload to complete");
    }

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-create-{label}-{unique}"))
    }

    fn take_pending_status(app: &mut App) -> (String, Option<PathBuf>) {
        let load = app
            .directory_runtime
            .pending_load
            .take()
            .expect("expected queued directory load");
        let status = match load.completion {
            DirectoryLoadCompletion::Status(status) => status,
            DirectoryLoadCompletion::Keep => {
                panic!("expected status completion, got keep")
            }
            DirectoryLoadCompletion::Clear => {
                panic!("expected status completion, got clear")
            }
        };
        (status, load.reselect_path)
    }

    fn encode_trashinfo_path(path: &Path) -> String {
        path.to_string_lossy()
            .replace('%', "%25")
            .replace(' ', "%20")
    }

    fn create_fake_trash_file(label: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        let root = temp_path(label);
        let originals_dir = root.join("originals");
        let trash_files = root.join("Trash/files");
        let trash_info = root.join("Trash/info");
        fs::create_dir_all(&originals_dir).expect("failed to create originals dir");
        fs::create_dir_all(&trash_files).expect("failed to create trash files dir");
        fs::create_dir_all(&trash_info).expect("failed to create trash info dir");

        let original_path = originals_dir.join("restore-target.txt");
        fs::write(&original_path, "restore me").expect("failed to write original file");

        let trashed_path = trash_files.join("restore-target.txt");
        fs::rename(&original_path, &trashed_path).expect("failed to move file into fake trash");
        fs::write(
            trash_info.join("restore-target.txt.trashinfo"),
            format!(
                "[Trash Info]\nPath={}\nDeletionDate=2026-03-21T00:00:00\n",
                encode_trashinfo_path(&original_path)
            ),
        )
        .expect("failed to write trashinfo");

        (root, trash_files, original_path, trashed_path)
    }

    #[test]
    fn confirm_create_creates_files_and_folders_and_reselects_last_created_path() {
        let root = temp_path("create-success");
        fs::create_dir_all(&root).expect("failed to create temp root");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.open_create_prompt();
        let overlay = app.create.as_mut().expect("create overlay should be open");
        overlay.lines = vec!["notes.txt".to_string(), "/docs/".to_string()];
        overlay.line_errors = vec![None; overlay.lines.len()];

        app.confirm_create().expect("create should succeed");

        assert!(app.create.is_none());
        assert!(root.join("notes.txt").is_file());
        assert!(root.join("docs").is_dir());

        let (status, reselect_path) = take_pending_status(&mut app);
        assert_eq!(status, "Created 1 file and 1 folder");
        assert_eq!(reselect_path, Some(root.join("docs")));

        app.directory_runtime.watch = None;
        drop(app);
        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn confirm_create_reports_duplicate_names_after_dir_marker_normalization() {
        let root = temp_path("create-duplicates");
        fs::create_dir_all(&root).expect("failed to create temp root");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.open_create_prompt();
        let overlay = app.create.as_mut().expect("create overlay should be open");
        overlay.lines = vec!["logs/".to_string(), "/logs".to_string()];
        overlay.line_errors = vec![None; overlay.lines.len()];

        app.confirm_create()
            .expect("create validation should succeed");

        let overlay = app
            .create
            .as_ref()
            .expect("create overlay should stay open");
        assert_eq!(overlay.cursor_line, 1);
        assert_eq!(
            overlay.line_errors[1].as_deref(),
            Some("\"logs\" appears more than once")
        );
        assert!(!root.join("logs").exists());
        assert!(app.directory_runtime.pending_load.is_none());

        app.directory_runtime.watch = None;
        drop(app);
        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn confirm_rename_renames_selected_entry_and_queues_reselect() {
        let root = temp_path("rename-success");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("report.txt"), "draft").expect("failed to write source file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.open_rename_prompt();
        let overlay = app.rename.as_mut().expect("rename overlay should be open");
        assert_eq!(overlay.original_name, "report.txt");
        assert_eq!(overlay.cursor_col, 6);
        overlay.input = "summary.txt".to_string();

        app.confirm_rename().expect("rename should succeed");

        assert!(app.rename.is_none());
        assert!(!root.join("report.txt").exists());
        assert!(root.join("summary.txt").is_file());

        let (status, reselect_path) = take_pending_status(&mut app);
        assert_eq!(status, "Renamed \"report.txt\" → \"summary.txt\"");
        assert_eq!(reselect_path, Some(root.join("summary.txt")));

        app.directory_runtime.watch = None;
        drop(app);
        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn cursor_before_extension_skips_hidden_file_prefix_dot() {
        assert_eq!(rename::cursor_before_extension(".env"), 4);
        assert_eq!(rename::cursor_before_extension("report.txt"), 6);
        assert_eq!(rename::cursor_before_extension("archive.tar.gz"), 11);
    }

    #[test]
    fn confirm_bulk_rename_renames_changed_entries_and_skips_unchanged_rows() {
        let root = temp_path("bulk-rename-success");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("alpha.txt"), "alpha").expect("failed to write alpha");
        fs::write(root.join("beta.txt"), "beta").expect("failed to write beta");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.selected_paths.insert(root.join("alpha.txt"));
        app.selected_paths.insert(root.join("beta.txt"));
        app.open_bulk_rename_prompt();

        let overlay = app
            .bulk_rename
            .as_mut()
            .expect("bulk rename overlay should be open");
        assert_eq!(overlay.new_names, vec!["alpha.txt", "beta.txt"]);
        overlay.new_names[0] = "gamma.txt".to_string();

        app.confirm_bulk_rename()
            .expect("bulk rename should succeed");

        assert!(app.bulk_rename.is_none());
        assert!(root.join("gamma.txt").is_file());
        assert!(root.join("beta.txt").is_file());
        assert!(!root.join("alpha.txt").exists());
        assert!(app.selected_paths.is_empty());

        let (status, reselect_path) = take_pending_status(&mut app);
        assert_eq!(status, "Renamed \"alpha.txt\" → \"gamma.txt\"");
        assert_eq!(reselect_path, Some(root.join("gamma.txt")));

        app.directory_runtime.watch = None;
        drop(app);
        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn confirm_bulk_rename_reports_duplicate_destination_names() {
        let root = temp_path("bulk-rename-duplicates");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("alpha.txt"), "alpha").expect("failed to write alpha");
        fs::write(root.join("beta.txt"), "beta").expect("failed to write beta");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.selected_paths.insert(root.join("alpha.txt"));
        app.selected_paths.insert(root.join("beta.txt"));
        app.open_bulk_rename_prompt();

        let overlay = app
            .bulk_rename
            .as_mut()
            .expect("bulk rename overlay should be open");
        overlay.new_names = vec!["shared.txt".to_string(), "shared.txt".to_string()];

        app.confirm_bulk_rename()
            .expect("bulk rename validation should succeed");

        let overlay = app
            .bulk_rename
            .as_ref()
            .expect("bulk rename overlay should stay open");
        assert_eq!(overlay.cursor_line, 1);
        assert_eq!(
            overlay.line_errors[1].as_deref(),
            Some("\"shared.txt\" appears more than once")
        );
        assert!(root.join("alpha.txt").is_file());
        assert!(root.join("beta.txt").is_file());
        assert!(app.directory_runtime.pending_load.is_none());

        app.directory_runtime.watch = None;
        drop(app);
        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn confirm_trash_permanently_deletes_selected_items_inside_trash() {
        let root = temp_path("trash-permanent");
        fs::create_dir_all(&root).expect("failed to create temp root");
        fs::write(root.join("gone.txt"), "bye").expect("failed to write file");

        let mut app = App::new_at(root.clone()).expect("failed to create app");
        app.in_trash = true;
        app.selected_paths.insert(root.join("gone.txt"));
        app.open_trash_prompt();

        assert_eq!(app.trash_title(), "Delete permanently 1 selected file?");
        app.confirm_trash().expect("trash should succeed");

        assert!(app.trash.is_none());
        assert!(app.selected_paths.is_empty());

        // Deletion is async — wait for the background worker *and* the
        // subsequent directory reload to both finish.
        wait_for_trash_and_reload(&mut app);

        assert!(!root.join("gone.txt").exists());
        // Status is set by apply_directory_snapshot once the reload completes.
        assert_eq!(app.status_message(), "Permanently deleted \"gone.txt\"");

        app.directory_runtime.watch = None;
        drop(app);
        fs::remove_dir_all(root).expect("failed to remove temp root");
    }

    #[test]
    fn confirm_restore_restores_file_from_trashinfo_and_queues_reload() {
        let (root, trash_files, original_path, trashed_path) = create_fake_trash_file("restore");

        let mut app = App::new_at(trash_files.clone()).expect("failed to create app");
        app.in_trash = true;
        app.open_restore_prompt();

        assert_eq!(app.restore_title(), "Restore 1 selected file?");
        app.confirm_restore().expect("restore should succeed");

        assert!(app.restore.is_none());
        assert!(original_path.is_file());
        assert!(!trashed_path.exists());
        assert!(app.selected_paths.is_empty());

        let (status, reselect_path) = take_pending_status(&mut app);
        assert_eq!(status, "Restored \"restore-target.txt\"");
        assert_eq!(reselect_path, None);

        app.directory_runtime.watch = None;
        drop(app);
        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
