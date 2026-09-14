//! Stamps a build ID into every result row, so a replay can say which binary produced it.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CUBARIUM_SEARCH_BUILD");
    let id = std::env::var("CUBARIUM_SEARCH_BUILD").ok().unwrap_or_else(git_id);
    println!("cargo:rustc-env=CUBARIUM_SEARCH_BUILD={id}");
}

fn git_id() -> String {
    let rev = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let dirty = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);
    if dirty { format!("{rev}-dirty") } else { rev }
}
