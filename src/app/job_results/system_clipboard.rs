use super::super::App;
use crate::background_jobs::job_results::{SystemClipboardRead, SystemClipboardReadBuild};

impl App {
    pub(super) fn apply_system_clipboard_read_job_result(
        &mut self,
        build: SystemClipboardReadBuild,
    ) -> bool {
        if build.token != self.input.system_clipboard_read_token {
            return false;
        }
        match build.result {
            SystemClipboardRead::Files { cwd, clipboard } => {
                self.apply_system_clipboard_file_read(cwd, clipboard)
            }
            SystemClipboardRead::Text {
                target,
                input_generation,
                text,
            } => self.apply_system_clipboard_text_read(target, input_generation, text),
        }
    }
}
