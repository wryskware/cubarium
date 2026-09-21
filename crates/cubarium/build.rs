//! Stamps the git short hash into the binary so snapshots record which build wrote them.
//! Git-free deployments can supply CUBARIUM_BUILD_REV; otherwise failure yields `unknown`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CUBARIUM_BUILD_REV");
    // Follow both HEAD and its resolved ref: HEAD itself does not change on a
    // normal commit. Ask git for paths so linked worktrees work as well.
    for name in ["HEAD", "packed-refs"] {
        watch_git_path(name);
    }
    if let Some(reference) = git_output(&["symbolic-ref", "-q", "HEAD"]) {
        watch_git_path(&reference);
    }
    let hash = std::env::var("CUBARIUM_BUILD_REV")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| git_output(&["rev-parse", "--short", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=CUBARIUM_GIT_HASH={hash}");
}

fn git_output(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn watch_git_path(name: &str) {
    if let Some(path) = git_output(&["rev-parse", "--git-path", name]) {
        println!("cargo:rerun-if-changed={path}");
    }
}
