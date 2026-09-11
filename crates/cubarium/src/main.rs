//! Cubarium host binary. M1 scope: fixed clock, geometry fixture scenes, preview window,
//! shim output, PNG captures. See `crates/cubarium/README.md` for the command contract.

use std::process::ExitCode;

use clap::Parser;
use cubarium::cli::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cubarium::run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("cubarium: {e:#}");
            ExitCode::FAILURE
        }
    }
}
