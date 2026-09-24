//! Stamps a build ID into every result row, so a replay can say which binary produced it.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CUBARIUM_SEARCH_BUILD");
    // Without these, Cargo reuses the stamp until build.rs itself changes, so every recorded
    // build ID drifts to whatever commit was checked out when the crate was last rebuilt.
    for path in git_watch_paths() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let id = std::env::var("CUBARIUM_SEARCH_BUILD")
        .ok()
        .unwrap_or_else(git_id);
    println!("cargo:rustc-env=CUBARIUM_SEARCH_BUILD={id}");
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
    println!("cargo:rustc-env=CUBARIUM_SEARCH_FLAGS={}", build_flags());
}

/// How this build was compiled, for a remote worker's connect check
/// (`es::voxel::remote`): profile, optimisation level, target, the rustflags, and a
/// digest of the target features they enabled (`-C target-cpu=znver5` shows there).
fn build_flags() -> String {
    let var = |k: &str| std::env::var(k).unwrap_or_default();
    let rustflags = var("CARGO_ENCODED_RUSTFLAGS")
        .split('\x1f')
        .filter(|f| !f.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let mut features: Vec<String> = var("CARGO_CFG_TARGET_FEATURE")
        .split(',')
        .map(str::to_string)
        .collect();
    features.sort();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in features.join(",").bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!(
        "profile={} opt={} target={} rustflags=[{rustflags}] features={}:{h:016x}",
        var("PROFILE"),
        var("OPT_LEVEL"),
        var("TARGET"),
        features.len(),
    )
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

/// The files whose change means `HEAD` or the index moved: `HEAD` itself, the branch ref it
/// points at (loose and packed), and the index for the dirty flag. Unstaged edits do not
/// retrigger; that is the one case the stamp can still lag, and the `-dirty` suffix covers it
/// only at the next rebuild.
fn git_watch_paths() -> Vec<std::path::PathBuf> {
    let Some(git_dir) = Command::new("git")
        .args(["rev-parse", "--absolute-git-dir"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| std::path::PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
    else {
        return Vec::new();
    };
    let mut paths = vec![
        git_dir.join("HEAD"),
        git_dir.join("index"),
        git_dir.join("packed-refs"),
    ];
    if let Ok(head) = std::fs::read_to_string(git_dir.join("HEAD"))
        && let Some(r) = head.trim().strip_prefix("ref: ")
    {
        paths.push(git_dir.join(r));
    }
    paths.into_iter().filter(|p| p.exists()).collect()
}
