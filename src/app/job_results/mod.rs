mod background_job_processing;
mod browser_results;
mod file_operation_results;
mod preview_results;
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
mod system_clipboard;

#[cfg(test)]
mod tests;
