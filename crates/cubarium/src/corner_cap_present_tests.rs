//! The retained final-position cap owner ([`TallPlant::corner_cap_owner`]), ported from the
//! independently reviewed growth-corner study (`art/studies/growth-corner`, review
//! `design/7_Research/fable-growth-corner-review-2026-09-13.md`). The original ownership
//! pop is kept reproducible through the unflagged path; the flagged path must be exactly the
//! original below the handoff and at maturity, continuous at the handoff and at the reported
//! boundary for every authored frame and midframe on every near-corner slot of every side
//! at ± the full family wind budget, physically supported within nine pixels, and must
//! change nothing about atlas texels, growth or any interior column.
use super::*;
use crate::art::{ArtPack, CORNER_CAP_OWNER_HOSTS, CORNER_CAP_OWNER_V1};
use cubarium_surface::unfold;
use cube_proto::Frame;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

fn atelier() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")
}
fn pack() -> ArtPack {
    ArtPack::load(&atelier()).unwrap()
}
/// The shipped pack with the capability cleared in memory: the original cap path.
fn legacy_pack() -> ArtPack {
    let mut art = pack();
    for p in art.tall.iter_mut() {
        p.corner_cap_owner = false;
    }
    art
}
fn draw(art: &ArtPack, column: &TallColumn, height: f64, seconds: f64, amplitude: f64) -> Canvas {
    let mut c = Canvas::new();
    draw_column(
        &mut c,
        column,
        height,
        art.tall_plant(TALL_PLANTS[column.pick]).unwrap(),
        art.tall_plant(VINE_PLANT),
        seconds,
        amplitude,
        &mut Vec::new(),
    );
    c
}
fn rgb8(c: &Canvas) -> Frame {
    let mut f = Frame::black();
    c.encode(&mut f);
    f
}
/// Largest linear channel difference and where it is.
fn worst(a: &Canvas, b: &Canvas) -> (f32, Option<(Face, u8, u8)>) {
    let mut best = 0.0f32;
    let mut at = None;
    for face in Face::ALL {
        for y in 0..64 {
            for x in 0..64 {
                let d = a
                    .get(face, x, y)
                    .into_iter()
                    .zip(b.get(face, x, y))
                    .map(|(p, q)| (p - q).abs())
                    .fold(0.0f32, f32::max);
                if d > best {
                    best = d;
                    at = Some((face, x, y));
                }
            }
        }
    }
    (best, at)
}
fn light(c: &Canvas) -> f64 {
    Face::ALL
        .into_iter()
        .flat_map(|f| (0..64).flat_map(move |y| (0..64).map(move |x| (f, x, y))))
        .map(|(f, x, y)| c.get(f, x, y).into_iter().map(f64::from).sum::<f64>())
        .sum()
}
const H_BOUNDARY: f64 = 53.0 / 6.0;
const PHASE: f64 = 49.625;
/// The two forced corner spires the vine review reported (not selected by the hash) and
/// every selected near-corner column.
fn corner_columns() -> Vec<TallColumn> {
    let mut v = vec![
        TallColumn {
            face: Face::Front,
            cx: 0,
            pick: 0,
            vine: true,
        },
        TallColumn {
            face: Face::Right,
            cx: 15,
            pick: 0,
            vine: true,
        },
    ];
    v.extend(tall_columns().into_iter().filter(|c| c.cx < 2 || c.cx > 13));
    v
}
fn all_corner_slots() -> Vec<TallColumn> {
    Face::ALL
        .into_iter()
        .filter(|f| *f != Face::Top)
        .flat_map(|face| {
            [0u8, 1, 14, 15].into_iter().flat_map(move |cx| {
                (0..2).map(move |pick| TallColumn {
                    face,
                    cx,
                    pick,
                    vine: true,
                })
            })
        })
        .collect()
}

#[test]
fn the_shipped_pack_flags_exactly_the_two_host_caps_and_the_flag_changes_no_texel() {
    let art = pack();
    for p in &art.tall {
        assert_eq!(
            p.corner_cap_owner,
            CORNER_CAP_OWNER_HOSTS.contains(&p.name.as_str()),
            "{}",
            p.name
        );
        assert_eq!(
            p.corner_cap_owner,
            p.name == "spiretree" || p.name == "glasscane"
        );
        if p.corner_cap_owner {
            assert!(p.cap.is_some() && p.crown.is_some());
        }
    }
    assert!(!art.tall_plant(VINE_PLANT).unwrap().corner_cap_owner);
    // A pack whose crown rows carry no selector loads the same texels, tail rows and vine
    // pieces; only the flag differs.
    let mut fixture = Fixture::new();
    for row in fixture.rows() {
        row.as_object_mut().unwrap().remove("corner_cap_owner");
    }
    let plain = fixture.load().unwrap();
    assert!(plain.tall.iter().all(|p| !p.corner_cap_owner));
    assert_eq!(plain.tall.len(), art.tall.len());
    for (a, b) in art.tall.iter().zip(&plain.tall) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.tail_row, b.tail_row);
        assert_eq!(a.vine_strips.is_some(), b.vine_strips.is_some());
        let clips = |p: &TallPlant| {
            [
                p.base.as_ref(),
                Some(&p.trunk),
                p.crown.as_ref(),
                p.cap.as_ref(),
            ]
            .map(|c| c.cloned_frames())
        };
        for (x, y) in clips(a).into_iter().zip(clips(b)) {
            assert_eq!(x, y, "{}", a.name);
        }
    }
}

/// Frames as premultiplied texel vectors, for exact comparison without `Clone` on `Clip`.
trait ClonedFrames {
    fn cloned_frames(self) -> Option<Vec<Vec<[f32; 4]>>>;
}
impl ClonedFrames for Option<&Clip> {
    fn cloned_frames(self) -> Option<Vec<Vec<[f32; 4]>>> {
        self.map(|c| {
            c.frames
                .iter()
                .map(|f| {
                    (0..f.height() as i32)
                        .flat_map(|y| (0..f.width() as i32).map(move |x| f.texel(x, y)))
                        .collect()
                })
                .collect()
        })
    }
}

#[test]
fn the_unflagged_path_still_reproduces_the_original_ownership_pop() {
    // The red case is retained deliberately: with the capability cleared, the selected
    // Left0 glasscane+vine column and the two forced corner spires still switch charts at
    // height 53/6, and removing the cap removes the switch (causal control).
    let legacy = legacy_pack();
    // The study's three reproducers: the selected Left0 column and the two forced spires.
    // The other selected corner columns (Front14, Back14) did not switch at 53/6 in the
    // study either; they are covered by the continuity tests, not asserted to pop.
    let reproducers = [
        tall_column_of(Face::Left, 0).unwrap(),
        TallColumn {
            face: Face::Front,
            cx: 0,
            pick: 0,
            vine: true,
        },
        TallColumn {
            face: Face::Right,
            cx: 15,
            pick: 0,
            vine: true,
        },
    ];
    assert_eq!(reproducers[0].pick, 1, "Left0 is the glasscane+vine column");
    for col in reproducers {
        let a = draw(&legacy, &col, H_BOUNDARY - 1e-8, PHASE, 0.0);
        let b = draw(&legacy, &col, H_BOUNDARY + 1e-8, PHASE, 0.0);
        let (d, at) = worst(&a, &b);
        assert!(
            d > 0.15,
            "{col:?}: the original pop is gone from the unflagged path ({d} at {at:?})"
        );
        assert_eq!(
            at.map(|p| p.0),
            Some(Face::Top),
            "{col:?}: the pop is a top-face ownership switch"
        );
    }
    let mut capless = legacy_pack();
    for p in capless.tall.iter_mut() {
        p.cap = None;
    }
    let col = tall_column_of(Face::Left, 0).unwrap();
    let (d, _) = worst(
        &draw(&capless, &col, H_BOUNDARY - 1e-8, PHASE, 0.0),
        &draw(&capless, &col, H_BOUNDARY + 1e-8, PHASE, 0.0),
    );
    assert!(d < 1e-6, "without the cap there is no switch: {d}");
}

#[test]
fn flagged_columns_are_exact_below_the_handoff_and_on_every_interior_column() {
    let art = pack();
    let legacy = legacy_pack();
    let interior: Vec<TallColumn> = tall_columns()
        .into_iter()
        .filter(|c| c.cx >= 2 && c.cx <= 13)
        .collect();
    assert!(!interior.is_empty());
    for seconds in [0.0, 0.9375, 2.99999, PHASE] {
        for col in corner_columns() {
            for h in [0.0, 0.1, 1.0, 4.5, 6.99999, 7.0] {
                let (d, at) = worst(
                    &draw(&art, &col, h, seconds, 0.0),
                    &draw(&legacy, &col, h, seconds, 0.0),
                );
                assert_eq!(
                    d, 0.0,
                    "{col:?} h{h} t{seconds}: lower growth must be the original image ({at:?})"
                );
            }
        }
        for col in &interior {
            for h in [4.5, 7.5, 8.0, H_BOUNDARY, 9.0] {
                let (d, _) = worst(
                    &draw(&art, col, h, seconds, 0.0),
                    &draw(&legacy, col, h, seconds, 0.0),
                );
                assert_eq!(
                    d, 0.0,
                    "{col:?} h{h}: an interior column never takes the corner path"
                );
            }
        }
    }
}

#[test]
fn flagged_columns_are_continuous_at_the_handoff_and_the_reported_boundary_with_the_vine() {
    let art = pack();
    for col in corner_columns() {
        for seconds in [0.0, 0.9375, 2.99999, PHASE] {
            for h in [7.0, H_BOUNDARY, 8.5, 8.75] {
                let (d, at) = worst(
                    &draw(&art, &col, h - 1e-8, seconds, 0.0),
                    &draw(&art, &col, h + 1e-8, seconds, 0.0),
                );
                assert!(d < 1e-6, "{col:?} h{h} t{seconds}: {d} at {at:?}");
            }
        }
    }
}

#[test]
fn the_handoff_holds_for_every_authored_frame_and_midframe_on_all_corner_slots_all_sides_and_full_wind_budgets_and_the_endpoint_is_the_original()
 {
    let art = pack();
    let legacy = legacy_pack();
    let mut checked = 0;
    for col in all_corner_slots() {
        let plant = art.tall_plant(TALL_PLANTS[col.pick]).unwrap();
        assert!(plant.corner_cap_owner);
        let clip = plant.cap.as_ref().unwrap();
        let budget = tall_bend_budget(plant);
        assert!(budget > 0.0);
        for k in 0..clip.frames.len() * 2 {
            let seconds = k as f64 * clip.seconds / (clip.frames.len() * 2) as f64
                - tall_phase_of(col.face, col.cx, clip.seconds);
            for amplitude in [-budget, 0.0, budget] {
                let (d, at) = worst(
                    &draw(&art, &col, 7.0 - 1e-8, seconds, amplitude),
                    &draw(&art, &col, 7.0 + 1e-8, seconds, amplitude),
                );
                assert!(
                    d < 1e-6,
                    "handoff {col:?} phase {k} amp {amplitude}: {d} at {at:?}"
                );
                let end = draw(&art, &col, 9.0, seconds, amplitude);
                let (e, at) = worst(&end, &draw(&legacy, &col, 9.0, seconds, amplitude));
                assert_eq!(e, 0.0, "endpoint {col:?} phase {k} amp {amplitude}: {at:?}");
                checked += 1;
            }
        }
    }
    assert!(checked >= 4 * 4 * 2 * 48 * 3);
}

#[test]
fn the_mature_endpoint_keeps_the_original_light_on_every_selected_corner_column() {
    let art = pack();
    let legacy = legacy_pack();
    for col in tall_columns().into_iter().filter(|c| c.cx < 2 || c.cx > 13) {
        for seconds in [0.0, PHASE] {
            let a = draw(&art, &col, 9.0, seconds, 0.0);
            let b = draw(&legacy, &col, 9.0, seconds, 0.0);
            assert_eq!(light(&a), light(&b), "{col:?}");
            assert_eq!(worst(&a, &b).0, 0.0);
        }
    }
}

#[test]
fn flagged_cap_pixels_lie_within_nine_physical_pixels_of_the_centre_and_the_owner_query_within_the_surface_limit()
 {
    let art = pack();
    let mut capless = pack();
    for p in capless.tall.iter_mut() {
        p.cap = None;
    }
    let mut checked = 0;
    for col in all_corner_slots() {
        let plant = art.tall_plant(TALL_PLANTS[col.pick]).unwrap();
        let budget = tall_bend_budget(plant);
        for frame in &plant.cap.as_ref().unwrap().frames {
            // The in-chart stamp's query is `extent + |amplitude| + |owner − centre|`; the
            // owner is at most 8 px from the centre (v 10 → 2), the footprint 9.
            assert!(
                frame.extent() + budget + (CORNER_CAP_HANDOFF_V - CORNER_CAP_FINAL_V)
                    <= cubarium_surface::MAX_LOCAL_RADIUS
            );
        }
        for h in [7.0, 7.5, 8.0, H_BOUNDARY, 9.0] {
            for seconds in [0.0, 0.9375, 2.99999] {
                for amplitude in [-budget, 0.0, budget] {
                    let centre = tall_anchor_at(col.face, col.cx, h + 1.0);
                    let with = draw(&art, &col, h, seconds, amplitude);
                    let without = draw(&capless, &col, h, seconds, amplitude);
                    for f in Face::ALL {
                        for y in 0..64 {
                            for x in 0..64 {
                                if with.get(f, x, y) != without.get(f, x, y) {
                                    assert!(
                                        unfold(
                                            centre,
                                            SurfacePoint::pixel_center(f, x as u8, y as u8),
                                            9.0
                                        )
                                        .is_some(),
                                        "{col:?} h{h} t{seconds} amp{amplitude}: cap pixel {f:?} ({x},{y}) outside the footprint"
                                    );
                                    checked += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(checked > 10_000);
}

#[test]
fn the_top_corner_pixel_trace_of_the_selected_left_column_matches_the_review() {
    // From the review's per-frame trace (fixed phase, no wind): the original lights Top
    // (1,0), (2,0), (3,0) in single frames at heights 8.5, 8.75 and 53/6; the flagged path
    // reaches the same values, each pixel arriving as the crown's dim rim a quarter-height
    // earlier, and the two agree again at maturity.
    let art = pack();
    let legacy = legacy_pack();
    let col = tall_column_of(Face::Left, 0).unwrap();
    let top = |c: &Canvas, x: usize| rgb8(c).get(Face::Top, x, 0);
    let (a, l) = (
        draw(&art, &col, 8.0, PHASE, 0.0),
        draw(&legacy, &col, 8.0, PHASE, 0.0),
    );
    assert_eq!(worst(&a, &l).0, 0.0, "identical at height 8");
    let (a, l) = (
        draw(&art, &col, 8.5 - 1.0 / 120.0, PHASE, 0.0),
        draw(&legacy, &col, 8.5 - 1.0 / 120.0, PHASE, 0.0),
    );
    assert_eq!(
        top(&l, 1),
        [0, 0, 0],
        "original: (1,0) is black one frame before 8.5"
    );
    assert_ne!(
        top(&a, 1),
        [0, 0, 0],
        "flagged: (1,0) is already lit before 8.5"
    );
    let (a, l) = (
        draw(&art, &col, 8.5, PHASE, 0.0),
        draw(&legacy, &col, 8.5, PHASE, 0.0),
    );
    assert_eq!(top(&l, 1), [22, 115, 179], "original: (1,0) pops on at 8.5");
    assert_eq!(top(&a, 1), top(&l, 1), "flagged: same value at 8.5");
    assert_eq!(top(&l, 2), [0, 0, 0]);
    assert_eq!(
        top(&a, 2),
        [17, 12, 69],
        "flagged: (2,0) is the dim rim a quarter-height early"
    );
    let (a, l) = (
        draw(&art, &col, H_BOUNDARY, PHASE, 0.0),
        draw(&legacy, &col, H_BOUNDARY, PHASE, 0.0),
    );
    assert_eq!(top(&l, 3), [19, 69, 121]);
    assert_eq!(top(&a, 3), top(&l, 3));
    let (a, l) = (
        draw(&art, &col, 9.0, PHASE, 0.0),
        draw(&legacy, &col, 9.0, PHASE, 0.0),
    );
    assert_eq!(worst(&a, &l).0, 0.0, "identical at maturity");
    assert_eq!(top(&a, 2), [49, 146, 184]);
    // Every difference between the two paths over the sweep is on the Top corner row.
    for k in 0..=240 {
        let h = 7.0 + k as f64 / 120.0;
        let (a, l) = (
            draw(&art, &col, h, PHASE, 0.0),
            draw(&legacy, &col, h, PHASE, 0.0),
        );
        for f in Face::ALL {
            for y in 0..64 {
                for x in 0..64 {
                    if a.get(f, x, y) != l.get(f, x, y) {
                        assert!(
                            f == Face::Top && y == 0 && (1..=4).contains(&x),
                            "frame {k}: {f:?} ({x},{y})"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn the_flag_does_not_touch_growth_or_what_the_presenter_observes() {
    use crate::present::PRODUCER_SATURATION;
    use cubarium_core::view::RenderView;
    use cubarium_surface::CELL_COUNT;
    let mut flagged = ArtPresenter::new(pack());
    let mut plain = ArtPresenter::new(legacy_pack());
    for tick in WIND_QUIET_TICK..WIND_QUIET_TICK + 400 {
        let v = RenderView {
            tick,
            producer: vec![10.0 * PRODUCER_SATURATION; CELL_COUNT],
            detritus: vec![SOIL_SCALE; CELL_COUNT],
            fruit: vec![0.0; CELL_COUNT],
            water: vec![0.0; CELL_COUNT],
            rain: vec![0.0; CELL_COUNT],
            producer_max: 10.0,
            organisms: Vec::new(),
        };
        flagged.observe(&v);
        plain.observe(&v);
        for i in 0..flagged.columns().len() {
            assert_eq!(flagged.tall_growth_of(i), plain.tall_growth_of(i));
        }
        for cell in CellId::all() {
            assert_eq!(flagged.stage_of(cell), plain.stage_of(cell));
        }
    }
    assert_eq!(flagged.columns(), plain.columns());
}

// --- Loader: the selector is explicit, versioned and accepted only where studied. -----

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    path: PathBuf,
    meta: serde_json::Value,
}
impl Fixture {
    fn new() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../captures/corner-cap-loader-fixtures")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::create_dir(&path).unwrap();
        for entry in fs::read_dir(atelier()).unwrap() {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            fs::copy(entry.path(), path.join(entry.file_name())).unwrap();
        }
        let meta = serde_json::from_slice(&fs::read(path.join("pack.json")).unwrap()).unwrap();
        Self { path, meta }
    }
    fn rows(&mut self) -> impl Iterator<Item = &mut serde_json::Value> {
        self.meta["tall"].as_array_mut().unwrap().iter_mut()
    }
    fn row(&mut self, name: &str, part: &str) -> &mut serde_json::Value {
        self.rows()
            .find(|r| r["name"] == name && r["part"] == part)
            .unwrap()
    }
    fn load(&self) -> anyhow::Result<ArtPack> {
        fs::write(
            self.path.join("pack.json"),
            serde_json::to_vec(&self.meta).unwrap(),
        )
        .unwrap();
        ArtPack::load(&self.path)
    }
    fn reject(&self, reason: &str) {
        match self.load() {
            Ok(_) => panic!("accepted invalid {reason}"),
            Err(e) => assert!(
                format!("{e:#}").contains(reason),
                "wrong rejection: {e:#}; wanted {reason}"
            ),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).unwrap();
    }
}

#[test]
fn the_shipped_manifest_carries_the_versioned_selector_on_the_two_crown_rows_only() {
    let mut f = Fixture::new();
    let rows: Vec<(String, String, Option<String>)> = f
        .rows()
        .map(|r| {
            (
                r["name"].as_str().unwrap().to_string(),
                r["part"].as_str().unwrap().to_string(),
                r.get("corner_cap_owner")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            )
        })
        .collect();
    for (name, part, sel) in rows {
        let expected = (part == "crown" && CORNER_CAP_OWNER_HOSTS.contains(&name.as_str()))
            .then(|| CORNER_CAP_OWNER_V1.to_string());
        assert_eq!(sel, expected, "{name} {part}");
    }
    assert_eq!(CORNER_CAP_OWNER_V1, "final_position_v1");
}

#[test]
fn unknown_malformed_or_misplaced_selectors_are_rejected_and_absent_means_off() {
    let mut f = Fixture::new();
    f.row("spiretree", "crown")["corner_cap_owner"] = serde_json::json!("final_position_v2");
    f.reject("unsupported corner_cap_owner selector");
    let mut f = Fixture::new();
    f.row("glasscane", "crown")["corner_cap_owner"] = serde_json::json!(true);
    f.reject("unsupported corner_cap_owner selector");
    let mut f = Fixture::new();
    f.row("glasscane", "trunk")["corner_cap_owner"] = serde_json::json!(CORNER_CAP_OWNER_V1);
    f.reject("supported only on a crown row");
    let mut f = Fixture::new();
    f.row("vinecoil", "trunk")["corner_cap_owner"] = serde_json::json!(CORNER_CAP_OWNER_V1);
    f.reject("supported only on a crown row");
    // A custom host under another name is refused even on its crown row: the listing is
    // the validation record, and unlisted art keeps the legacy path or fails loudly.
    let mut f = Fixture::new();
    for r in f.rows() {
        if r["name"] == "glasscane" {
            r["name"] = serde_json::json!("customcane");
        }
    }
    f.reject("accepted only on the shipped host caps");
    let mut f = Fixture::new();
    for r in f.rows() {
        if r["name"] == "glasscane" {
            r["name"] = serde_json::json!("customcane");
            r.as_object_mut().unwrap().remove("corner_cap_owner");
        }
    }
    let custom = f.load().unwrap();
    assert!(!custom.tall_plant("customcane").unwrap().corner_cap_owner);
    assert!(custom.tall_plant("spiretree").unwrap().corner_cap_owner);
    let mut f = Fixture::new();
    f.row("spiretree", "crown")["loop"] = serde_json::json!(false);
    f.reject("corner_cap_owner must loop");
    let mut f = Fixture::new();
    f.row("spiretree", "crown")
        .as_object_mut()
        .unwrap()
        .remove("corner_cap_owner");
    let half = f.load().unwrap();
    assert!(!half.tall_plant("spiretree").unwrap().corner_cap_owner);
    assert!(half.tall_plant("glasscane").unwrap().corner_cap_owner);
}
