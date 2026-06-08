mod cli;
mod shell_integration;

use std::process::ExitCode;

fn main() -> ExitCode {
    match cli::run() {
        Ok(exit_code) => exit_code,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
