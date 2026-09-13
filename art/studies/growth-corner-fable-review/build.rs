// The same extraction the growth-corner study performs (art/studies/growth-corner/build.rs):
// the real private `draw_column` from the host source **as it was at `a9eb064`** (read from
// git, not the working tree), plus a copy whose near-corner cap owner is held at the
// column's final cap position. Pinned because the working tree's `draw_column` now carries
// the capability itself; extracting it live would make "original" mean "flagged" and the
// historical original-vs-candidate numbers dishonest. The probe still links against the
// current crates, so the two extracted bodies see today's helpers.
fn main() {
    let source = "../../../crates/cubarium/src/art_present.rs";
    println!("cargo:rerun-if-changed=build.rs");
    let show = std::process::Command::new("git")
        .args(["show", "a9eb064:crates/cubarium/src/art_present.rs"])
        .current_dir(std::path::Path::new(source).parent().unwrap())
        .output()
        .expect("git show of the pinned presenter source");
    assert!(show.status.success(), "git show a9eb064 failed: {}", String::from_utf8_lossy(&show.stderr));
    let text = String::from_utf8(show.stdout).unwrap();
    let start = text.find("fn draw_column(\n").expect("private column entry");
    let end = text[start..].find("\n/// The layers one stage").expect("column end") + start;
    let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("column.rs");
    let original = &text[start..end];
    let needle = "let at = tall_anchor_at(column.face, column.cx, i);";
    assert_eq!(original.matches(needle).count(), 1);
    let candidate = original.replace("fn draw_column(", "fn draw_column_candidate(").replace(needle,
        "let at = tall_anchor_at(column.face, column.cx, i);\nlet owner = if plant.cap.as_ref().is_some_and(|cap| std::ptr::eq(cap, clip)) && (column.cx < 2 || column.cx > 13) && at.v < 10.0 { SurfacePoint::new(at.face, at.u, 2.0) } else { at };")
        .replace("stamp_layers_bent(\n            canvas,", "candidate_stamp(\n            canvas,\n            owner,");
    std::fs::write(path, format!("{original}\n{candidate}")).unwrap();
}
