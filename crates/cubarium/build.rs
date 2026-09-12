//! Stamps the git short hash into the binary so snapshots record which build wrote them.
//! Failure is never fatal: without git (a source tarball, a sandbox) the hash is `unknown`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=CUBARIUM_GIT_HASH={hash}");
}
