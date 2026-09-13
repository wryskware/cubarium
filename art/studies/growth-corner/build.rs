// Keep the historical failure reproducible after its production fix. Only the private
// column body is frozen; this study still uses the current surface/render dependencies.
fn main() {
    let source = "../../../crates/cubarium/src/art_present.rs";
    println!("cargo:rerun-if-changed=build.rs");
    let show = std::process::Command::new("git")
        .args(["show", "a9eb064:crates/cubarium/src/art_present.rs"])
        .current_dir(std::path::Path::new(source).parent().unwrap())
        .output()
        .expect("git show of the pinned pre-fix presenter");
    assert!(show.status.success(), "git show a9eb064 failed: {}",
        String::from_utf8_lossy(&show.stderr));
    let text = String::from_utf8(show.stdout).unwrap();
    let start = text
        .find("fn draw_column(\n")
        .expect("private column entry");
    let end = text[start..]
        .find("\n/// The layers one stage")
        .expect("column end")
        + start;
    let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("column.rs");
    let original = &text[start..end];
    let needle = "let at = tall_anchor_at(column.face, column.cx, i);";
    assert_eq!(original.matches(needle).count(), 1);
    let candidate = original.replace("fn draw_column(", "fn draw_column_candidate(").replace(needle,
        "let at = tall_anchor_at(column.face, column.cx, i);\nlet owner = if plant.cap.as_ref().is_some_and(|cap| std::ptr::eq(cap, clip)) && (column.cx < 2 || column.cx > 13) && at.v < 10.0 { SurfacePoint::new(at.face, at.u, 2.0) } else { at };")
        .replace("stamp_layers_bent(\n            canvas,", "candidate_stamp(\n            canvas,\n            owner,");
    std::fs::write(path, format!("{original}\n{candidate}")).unwrap();
}
