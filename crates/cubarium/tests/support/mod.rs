//! Shared helpers for the `run` integration tests: a scratch directory per test and a
//! way to drive the host through the library rather than a subprocess.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use cubarium::cli::{Cli, Command, Run};
use cubarium::runner::RunOutcome;

use clap::Parser;

/// A directory under the target tree that is removed on drop.
pub struct Scratch {
    path: PathBuf,
}

impl Scratch {
    pub fn new(name: &str) -> Scratch {
        let path = std::env::temp_dir()
            .join(format!("cubarium-run-tests-{}", std::process::id()))
            .join(name);
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("creating the scratch directory");
        Scratch { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn join(&self, child: &str) -> PathBuf {
        self.path.join(child)
    }

    /// Write a file inside the scratch directory and return its path.
    pub fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.join(name);
        std::fs::write(&path, contents).expect("writing a scratch file");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Parse a `cubarium run` command line exactly as the binary would.
pub fn parse(args: &[&str]) -> Run {
    let mut full = vec!["cubarium", "run"];
    full.extend_from_slice(args);
    match Cli::parse_from(full).command {
        Command::Run(r) => r,
        other => panic!("expected a run command, got {other:?}"),
    }
}

/// Drive one `cubarium run` through the library API.
pub fn run(args: &[&str]) -> RunOutcome {
    cubarium::run_world(&parse(args)).expect("the run must succeed")
}

/// Every JSON line of a telemetry file, parsed. Panics on a line that is not JSON.
pub fn telemetry_lines(path: &Path) -> Vec<serde_json::Value> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            serde_json::from_str(l).unwrap_or_else(|e| panic!("telemetry line is not JSON: {e}\n{l}"))
        })
        .collect()
}

/// Snapshot ticks present in a state directory, newest first.
pub fn snapshot_ticks(dir: &Path) -> Vec<u64> {
    cubarium::state::list_snapshots(dir).into_iter().map(|(t, _)| t).collect()
}
