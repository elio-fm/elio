use super::super::*;
use super::helpers::*;

#[test]
fn background_job_processing_yields_after_a_burst_of_results() {
    let root = temp_path("result-burst-budget");
    fs::create_dir_all(&root).expect("failed to create temp root");

    let mut app = App::new_at(root.clone()).expect("failed to create app");
    wait_for_directory_load(&mut app);

    for index in 0..20 {
        app.jobs
            .scheduler
            .defer_result(JobResult::PreviewLineCount(PreviewLineCountBuild {
                path: root.join(format!("item-{index}.txt")),
                size: index as u64 + 1,
                modified: None,
                total_lines: Some(index + 1),
            }));
    }

    let _ = app.process_background_jobs();
    assert!(!app.preview.state.line_count_cache.is_empty());
    assert!(app.preview.state.line_count_cache.len() < 20);
    assert!(app.has_pending_background_work());

    for _ in 0..10 {
        let _ = app.process_background_jobs();
        if !app.has_pending_background_work() {
            break;
        }
    }

    assert_eq!(app.preview.state.line_count_cache.len(), 20);
    assert!(!app.has_pending_background_work());

    fs::remove_dir_all(root).expect("failed to remove temp root");
}
