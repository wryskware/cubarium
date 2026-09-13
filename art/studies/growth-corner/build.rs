// Instrument the real private function without changing its production visibility.
fn main() {
    let source = "../../../crates/cubarium/src/art_present.rs";
    println!("cargo:rerun-if-changed={source}");
    let text = std::fs::read_to_string(source).unwrap();
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
