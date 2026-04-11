use anyhow::Result;
use std::{env, process::ExitCode};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();

    match args.as_slice() {
        [] => elio::run(),
        [arg] if arg == "--version" || arg == "-V" => {
            print_version();
            Ok(())
        }
        [arg] if arg == "--help" || arg == "-h" => {
            print_help();
            Ok(())
        }
        [arg, unexpected, ..] if arg == "--version" || arg == "-V" => {
            Err(anyhow::anyhow!(unknown_argument_message(unexpected)))
        }
        [arg, unexpected, ..] if arg == "--help" || arg == "-h" => {
            Err(anyhow::anyhow!(unknown_argument_message(unexpected)))
        }
        [arg, ..] => Err(anyhow::anyhow!(unknown_argument_message(arg))),
    }
}

fn print_version() {
    println!("elio {}", env!("CARGO_PKG_VERSION"));
}

fn print_help() {
    println!("elio {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Usage: elio [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -h, --help     Print help");
    println!("  -V, --version  Print version");
}

fn unknown_argument_message(arg: &str) -> String {
    let mut message = format!("error: unexpected argument '{arg}' found");

    if arg != "--version" && arg != "-V" && ("--version".starts_with(arg) || "-V".starts_with(arg))
    {
        message.push_str("\n\n  tip: a similar argument exists: '--version'");
    } else if arg != "--help" && arg != "-h" && ("--help".starts_with(arg) || "-h".starts_with(arg))
    {
        message.push_str("\n\n  tip: a similar argument exists: '--help'");
    }

    message.push_str("\n\nUsage: elio [OPTIONS]");
    message.push_str("\n\nFor more information, try '--help'.");
    message
}
