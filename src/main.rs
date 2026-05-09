mod bump;
mod changelog;
mod cli;
mod config;
mod date;
mod git;
mod hooks;
mod release;

use std::process::ExitCode;

fn main() -> ExitCode {
    match cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}
