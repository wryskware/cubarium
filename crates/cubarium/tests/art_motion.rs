//! Independent tests for the *motion* of the art image: clip sampling, paced plant and
//! column growth, the rain, the body cross-fade, the fruit accent and the crown cap.
//!
//! Written from the public doc comments of `cubarium::art`, `cubarium::art_present` and
//! `cubarium_render::sprite` — never from their bodies. Where an expected image is needed
//! it is rebuilt here through the public API a second, independent way: a presenter drawn
//! from a pack with one layer removed supplies the background, and the layer under test is
//! hand-stamped on top. Where no such image exists the assertion is a property a wrong
//! implementation would break — growth that steps once per tick instead of per frame, a
//! column that pops a whole tile, a fade that cuts, an accent that lingers after the food
//! is gone.

use std::path::{Path, PathBuf};
use std::ptr;

use cube_proto::{FACE_SIZE, Face};
use cubarium::art::{ArtPack, Band, Clip, CROWN_TAIL_MIN_ROW, TallPlant};
use cubarium::art_present::{
    ArtPresenter, BODY_FADE_SECONDS, BodyMemory, FRUIT_FADE_SECONDS, FRUIT_SHOW, Growth,
    MAX_STEP_SECONDS, MOTIF_OPACITY, RAIN_BLINK, RAIN_PERIOD, REED_DEPTH, SOIL_SCALE,
    STAGE_GROW_SECONDS, STAGE_WILT_SECONDS, TALL_GROW_PX_PER_S, TALL_MAX_SEGMENTS, TALL_PLANTS,
    TALL_WILT_PX_PER_S, TILE_ROWS, TallColumn, TallGrowth, advance_growth, advance_tall, band_of,
    cell_band, clip_time, column_density, fruit_stage, growth_between, phase_of, placement_of,
    plant_bend_budget, plant_cap, plant_phase_of, present_seconds, rain_blink, rain_blink_on,
    rain_fall, rain_marks, rain_origin, rig_of, slot_of, slot_wind, species_of, stage_thresholds,
    state_of, tall_anchor, tall_anchor_at, tall_between, tall_columns, tall_grown_px, tall_heading,
    tall_target, up_of,
};
use cubarium::clock::DT;
use cubarium::present::{PRODUCER_SATURATION, interpolate};
use cubarium_core::OrganismId;
use cubarium_core::organism::Mode;
use cubarium_core::view::{OrganismView, RenderView};
use cubarium_render::{Canvas, Mask, Pose, Sprite, stamp_layers, stamp_layers_bent, stamp_pose};
use cubarium_surface::{CELL_COUNT, CellId, PixelImage, SurfacePoint, Vec2, cell_of};

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

const PRODUCER_MAX: f64 = 10.0;
/// Render frames per simulated tick at 60 fps with a 20 Hz clock.
const FRAMES_PER_TICK: u64 = 3;

fn atelier() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")
}

fn pack() -> ArtPack {
    ArtPack::load(&atelier()).expect("the baked pack at assets/atelier must load")
}

/// The pack without its tall plants: the same plants, ground and bodies, no columns. Growth
/// pacing of the small plants is independent of it, and a column takes 24 s to reach the rim
/// where a plant takes 12, so leaving the columns out is what lets a plant test compare a
/// paced image against a snapped one.
fn pack_without(tall: bool, plants: bool, ground: bool) -> ArtPack {
    let mut art = pack();
    if tall {
        art.tall.clear();
    }
    if plants {
        art.plants.clear();
    }
    if ground {
        art.ground.clear();
    }
    art
}

fn saturation() -> f64 {
    PRODUCER_MAX * PRODUCER_SATURATION
}

fn bare_view(tick: u64) -> RenderView {
    RenderView {
        tick,
        producer: vec![0.0; CELL_COUNT],
        detritus: vec![0.0; CELL_COUNT],
        fruit: vec![0.0; CELL_COUNT],
        water: vec![0.0; CELL_COUNT],
        rain: vec![0.0; CELL_COUNT],
        producer_max: PRODUCER_MAX,
        organisms: Vec::new(),
    }
}

/// Producers saturated and litter rich in every cell: every slot's target is its rank cap.
fn rich_view(tick: u64) -> RenderView {
    let mut v = bare_view(tick);
    v.producer.fill(saturation());
    v.detritus.fill(SOIL_SCALE);
    v
}

/// A world rich in exactly one cell, so one plant is the only thing that moves.
fn one_cell_view(tick: u64, cell: CellId, density: f64) -> RenderView {
    let mut v = bare_view(tick);
    v.producer[cell.index()] = saturation() * density;
    v
}

/// A foliage slot allowed to reach stage 2, far enough from the face edges that its tile
/// stays on one face.
fn full_foliage_cell() -> CellId {
    CellId::all()
        .find(|&c| {
            c.face() == Face::Front
                && band_of(c) == Band::Foliage
                && plant_cap(Band::Foliage, c) == Some(2)
                && (4..=7).contains(&c.cy())
                && (4..=11).contains(&c.cx())
        })
        .expect("the cube has a rank-2 foliage slot in the middle of Front")
}

// ---------------------------------------------------------------------------
// canvas helpers
// ---------------------------------------------------------------------------

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL.into_iter().flat_map(|face| {
        (0..FACE_SIZE as u8).flat_map(move |y| (0..FACE_SIZE as u8).map(move |x| (face, x, y)))
    })
}

/// The pixels a 16-px tile anchored in `cell` can reach: a tile's footprint is under the
/// nine-pixel budget and a slot sits within a pixel of the cell centre, so everything it
/// touches is within 11 px of that centre on the cell's own face.
fn near(cell: CellId) -> Vec<(Face, u8, u8)> {
    let centre = cell.center();
    (0..FACE_SIZE as u8)
        .flat_map(|y| (0..FACE_SIZE as u8).map(move |x| (cell.face(), x, y)))
        .filter(|&(_, x, y)| {
            (f64::from(x) + 0.5 - centre.u).hypot(f64::from(y) + 0.5 - centre.v) <= 12.0
        })
        .collect()
}

fn max_diff_at(a: &Canvas, b: &Canvas, pixels: &[(Face, u8, u8)]) -> f32 {
    pixels
        .iter()
        .flat_map(|&(f, x, y)| {
            let (p, q) = (a.get(f, x, y), b.get(f, x, y));
            (0..3).map(move |c| (p[c] - q[c]).abs())
        })
        .fold(0.0, f32::max)
}

fn max_diff(a: &Canvas, b: &Canvas) -> f32 {
    every_pixel()
        .flat_map(|(f, x, y)| {
            let (p, q) = (a.get(f, x, y), b.get(f, x, y));
            (0..3).map(move |c| (p[c] - q[c]).abs())
        })
        .fold(0.0, f32::max)
}

fn differing(a: &Canvas, b: &Canvas) -> Vec<(Face, u8, u8)> {
    every_pixel().filter(|&(f, x, y)| a.get(f, x, y) != b.get(f, x, y)).collect()
}

fn assert_same_canvas(a: &Canvas, b: &Canvas, what: &str) {
    let diff = differing(a, b);
    assert!(
        diff.is_empty(),
        "{what}: {} pixels differ, first at {:?} ({:?} vs {:?})",
        diff.len(),
        diff[0],
        a.get(diff[0].0, diff[0].1, diff[0].2),
        b.get(diff[0].0, diff[0].1, diff[0].2),
    );
}

fn assert_close_canvas(a: &Canvas, b: &Canvas, tolerance: f32, what: &str) {
    let d = max_diff(a, b);
    assert!(d <= tolerance, "{what}: images differ by {d}, over {tolerance}");
}

/// Observe then draw, the host's per-tick contract.
fn observe_draw(p: &mut ArtPresenter, v: &RenderView, f: f64) -> Canvas {
    p.observe(v);
    draw(p, v, f)
}

fn draw(p: &mut ArtPresenter, v: &RenderView, f: f64) -> Canvas {
    let mut canvas = Canvas::new();
    p.draw(v, f, &mut canvas);
    canvas
}

fn draw_fruit(p: &mut ArtPresenter, v: &RenderView, f: f64, fruit: Option<&[f64]>) -> Canvas {
    let mut canvas = Canvas::new();
    p.draw_with_fruit(v, f, &mut canvas, fruit);
    canvas
}

// ---------------------------------------------------------------------------
// 7. Clip::sample
// ---------------------------------------------------------------------------

/// A discrete player holds `at()`; `sample()` must bracket the same instant and carry the
/// fraction between the two samples. A looping clip that did not wrap its last sample back
/// into its first would jerk once per period; a bud clip that wrapped would restart its
/// birth.
#[test]
fn clip_sample_brackets_each_instant_and_wraps_a_loop_without_a_step() {
    let art = pack();
    let clip = &art.plant("lanternstalk").expect("lanternstalk").stages[2];
    let n = clip.frames.len();
    assert!(clip.looping && n >= 2, "the fixture must be a real looping clip");

    // The wrap: just below the length the pose runs from the last sample into the first.
    let wrap = clip.sample(clip.seconds - 1e-6);
    assert!(ptr::eq(wrap.first, clip.frames.last().unwrap()), "the wrap does not start last");
    assert!(ptr::eq(wrap.second, &clip.frames[0]), "the wrap does not end at the first sample");
    assert!(wrap.mix > 0.9999 && wrap.mix <= 1.0, "mix {} at the wrap", wrap.mix);
    assert!(ptr::eq(wrap.first, clip.at(clip.seconds - 1e-6)), "first is not at()'s frame");

    // On a sample: no blend at all, and the same frame a discrete player holds.
    for k in 0..n {
        let seconds = clip.seconds * k as f64 / n as f64;
        let pose = clip.sample(seconds);
        assert_eq!(pose.mix, 0.0, "sample {k} at {seconds} s must not blend");
        assert!(ptr::eq(pose.first, &clip.frames[k]), "sample {k} brackets the wrong frame");
        assert!(ptr::eq(pose.first, clip.at(seconds)), "sample {k} disagrees with at()");
        // Half a sample later it is halfway between this sample and the next.
        let half = clip.sample(seconds + clip.seconds / (2.0 * n as f64));
        assert!((half.mix - 0.5).abs() < 1e-6, "mix {} halfway through sample {k}", half.mix);
        assert!(ptr::eq(half.first, &clip.frames[k]));
        assert!(ptr::eq(half.second, &clip.frames[(k + 1) % n]));
    }
    // The loop repeats: a whole period later is the same pose.
    for k in [0usize, 5, 17] {
        let seconds = clip.seconds * k as f64 / n as f64 + 0.017;
        let here = clip.sample(seconds);
        let later = clip.sample(seconds + clip.seconds);
        assert!(ptr::eq(here.first, later.first) && ptr::eq(here.second, later.second));
        assert!((here.mix - later.mix).abs() < 1e-5, "{} vs {}", here.mix, later.mix);
    }

    // The non-looping bud clip clamps at both ends and never wraps.
    let bud = &art.clips[3];
    assert!(!bud.looping);
    let m = bud.frames.len();
    for seconds in [-1.0, -0.0, 0.0] {
        let pose = bud.sample(seconds);
        assert!(ptr::eq(pose.first, &bud.frames[0]), "{seconds} s is not the first sample");
        assert_eq!(pose.mix, 0.0);
    }
    for seconds in [bud.seconds, bud.seconds * 4.0] {
        let pose = bud.sample(seconds);
        assert!(ptr::eq(pose.first, bud.frames.last().unwrap()), "{seconds} s is not the last");
        assert!(ptr::eq(pose.second, bud.frames.last().unwrap()));
        assert_eq!(pose.mix, 0.0, "a finished bud must hold its last sample, not blend out of it");
    }
    let middle = bud.sample(bud.seconds * 0.5);
    assert!(ptr::eq(middle.first, &bud.frames[(m - 1) / 2]), "the midpoint brackets wrongly");

    // Nonsense time is the first sample, held.
    for clip in [clip, bud] {
        for seconds in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let pose = clip.sample(seconds);
            assert!(ptr::eq(pose.first, &clip.frames[0]), "{seconds} must read as the first sample");
            assert!(ptr::eq(pose.second, &clip.frames[0]));
            assert_eq!(pose.mix, 0.0);
        }
    }
}

// ---------------------------------------------------------------------------
// 8. temporal continuity of a stamped clip
// ---------------------------------------------------------------------------

/// Stamp one tile over black, where the composite is exactly linear in the sampled pose.
fn stamp_alone(pose: Pose<'_>, anchor: SurfacePoint) -> Canvas {
    let mut canvas = Canvas::new();
    stamp_pose(
        &mut canvas,
        anchor,
        Vec2::new(1.0, 0.0),
        pose,
        1.0,
        1.0,
        Mask::None,
        &mut Vec::new(),
    );
    canvas
}

/// The whole point of `Clip::sample`: at 60 fps the image moves by a fraction of one
/// sample step per frame instead of jumping a whole sample 8 times a second.
///
/// **The bound is derived, not measured.** Over black the stamp is linear in the pose's
/// sample, so within one sample interval a frame moves a pixel by `Δu · |Fⱼ₊₁ − Fⱼ|`, and
/// across a boundary by `(δ + δ') · max |Fⱼ₊₁ − Fⱼ|` — in both cases at most `Δu` times the
/// largest adjacent-sample difference, which is exactly what the discrete path jumps by.
/// With a 24-sample 3 s clip at 60 fps, `Δu = (1/60) / (3/24) = 0.1333…`, so the absolute
/// bound on the interpolated path's per-frame change is `0.1334 · max_discrete_step`.
#[test]
fn interpolated_stamping_moves_less_per_frame_than_discrete_frame_stepping() {
    let art = pack();
    let clip = &art.plant("lanternstalk").unwrap().stages[2];
    let n = clip.frames.len();
    let anchor = SurfacePoint::new(Face::Front, 32.0, 32.0);
    let frames = (clip.seconds * 60.0).round() as usize;
    assert!(frames > n, "the sweep must cross every sample boundary");
    let step = (1.0 / 60.0) / (clip.seconds / n as f64);

    let mut smooth = 0.0f32;
    let mut discrete = 0.0f32;
    let mut last_smooth = stamp_alone(clip.sample(0.0), anchor);
    let mut last_discrete = stamp_alone(Pose::still(clip.at(0.0)), anchor);
    for i in 1..=frames {
        let seconds = i as f64 / 60.0;
        let now_smooth = stamp_alone(clip.sample(seconds), anchor);
        let now_discrete = stamp_alone(Pose::still(clip.at(seconds)), anchor);
        smooth = smooth.max(max_diff(&now_smooth, &last_smooth));
        discrete = discrete.max(max_diff(&now_discrete, &last_discrete));
        last_smooth = now_smooth;
        last_discrete = now_discrete;
    }
    assert!(discrete > 0.0, "the fixture clip does not move at all");
    assert!(
        smooth < discrete,
        "interpolated stamping moved {smooth} per frame, the discrete path {discrete}"
    );
    println!("smooth {smooth} per frame, discrete {discrete}, bound {}", step * f64::from(discrete));
    assert!(
        f64::from(smooth) <= step * f64::from(discrete) + 1e-6,
        "interpolated stamping moved {smooth} per frame, over the derived bound {}",
        step * f64::from(discrete)
    );
}

// ---------------------------------------------------------------------------
// 9. advance_growth
// ---------------------------------------------------------------------------

/// How much of `stage` the growth is showing: `from` at `1 − g` and `to` at `g`. This is
/// orientation-independent, so a reversal that swaps the pair is recognised as the same
/// picture.
fn weight_of(g: Growth, stage: Option<u8>) -> f64 {
    let progress = if g.g.is_finite() { g.g.clamp(0.0, 1.0) } else { 0.0 };
    let mut w = 0.0;
    if g.from == stage {
        w += 1.0 - progress;
    }
    if g.to == stage {
        w += progress;
    }
    w
}

const STAGES: [Option<u8>; 4] = [None, Some(0), Some(1), Some(2)];

fn assert_same_visual(a: Growth, b: Growth, what: &str) {
    for stage in STAGES {
        let (wa, wb) = (weight_of(a, stage), weight_of(b, stage));
        assert!(
            (wa - wb).abs() < 1e-9,
            "{what}: stage {stage:?} is shown at {wa} but should be {wb} ({a:?} vs {b:?})"
        );
    }
}

#[test]
fn advance_growth_climbs_one_stage_at_a_time_at_the_documented_pace() {
    let mut g = Growth::snapped(None, false);
    assert_eq!(g, Growth { from: None, to: None, g: 1.0, target: None, fruit: 0.0 });

    // None -> Some(2) runs None -> 0 -> 1 -> 2, one step at a time, never skipping.
    let mut idle_at: Vec<(usize, Option<u8>)> = Vec::new();
    let mut seen_in_flight: Vec<(Option<u8>, Option<u8>)> = Vec::new();
    for call in 1..=400usize {
        g = advance_growth(g, Some(2), false, DT);
        assert_eq!(g.target, Some(2), "call {call} lost the target");
        assert!((0.0..=1.0).contains(&g.g), "call {call}: g is {}", g.g);
        if g.from == g.to {
            if idle_at.last().map(|e| e.1) != Some(g.from) {
                idle_at.push((call, g.from));
            }
        } else {
            if seen_in_flight.last() != Some(&(g.from, g.to)) {
                seen_in_flight.push((g.from, g.to));
            }
            // One stage at a time: the step always spans neighbouring ranks.
            let rank = |s: Option<u8>| s.map_or(-1i32, i32::from);
            assert_eq!(
                (rank(g.to) - rank(g.from)).abs(),
                1,
                "call {call} stepped from {:?} to {:?}",
                g.from,
                g.to
            );
        }
        if g.from == Some(2) && g.to == Some(2) {
            break;
        }
    }
    assert_eq!(
        seen_in_flight,
        vec![(None, Some(0)), (Some(0), Some(1)), (Some(1), Some(2))],
        "the climb must pass through every stage in order"
    );
    assert_eq!(
        idle_at.iter().map(|e| e.1).collect::<Vec<_>>(),
        vec![Some(0), Some(1), Some(2)],
        "every step must complete into idle before the next starts"
    );

    // Each step takes `STAGE_GROW_SECONDS` of accumulated `dt`, to within the one call that
    // f64 summation of `DT / STAGE_GROW_SECONDS` costs (80 increments of 0.0125 fall a few
    // ulps short of 1, so a 4 s step completes on call 81).
    let per_step = STAGE_GROW_SECONDS / DT;
    for (i, &(call, stage)) in idle_at.iter().enumerate() {
        let want = per_step * (i + 1) as f64;
        // One tick of slack per step: `g` is a running sum of `dt / STAGE_GROW_SECONDS`, and
        // 80 f64 increments of 0.0125 land a few ulps *under* 1, so each 4 s step costs one
        // tick more than `STAGE_GROW_SECONDS / DT` calls and the slack accumulates.
        assert!(
            (call as f64 - want).abs() <= (i + 1) as f64,
            "stage {stage:?} completed on call {call}, not within {} ticks of {want}",
            i + 1
        );
    }
    let total = idle_at.last().unwrap().0 as f64 * DT;
    assert!(
        (3.0 * STAGE_GROW_SECONDS - 2.0 * DT..=3.0 * STAGE_GROW_SECONDS + 3.0 * DT)
            .contains(&total),
        "three stages took {total} s, not about {} s",
        3.0 * STAGE_GROW_SECONDS
    );
    // The call after a step completes starts the next one, from the stage just reached.
    let mut g = Growth::snapped(None, false);
    for _ in 0..idle_at[0].0 {
        g = advance_growth(g, Some(2), false, DT);
    }
    assert_eq!((g.from, g.to), (Some(0), Some(0)));
    g = advance_growth(g, Some(2), false, DT);
    assert_eq!((g.from, g.to), (Some(0), Some(1)), "the next step must start from stage 0");
    assert!(g.g < 0.05, "a fresh step starts at 0, not {}", g.g);
}

#[test]
fn advance_growth_reverses_in_flight_and_needs_time_to_start_a_step() {
    // A target beyond `to` finishes the current step first.
    let mut g = Growth::snapped(None, false);
    for call in 1..=40 {
        g = advance_growth(g, Some(2), false, DT);
        assert_eq!(g.to, Some(0), "call {call} skipped ahead to {:?}", g.to);
        assert_eq!(g.from, None);
    }
    assert!((g.g - 0.5).abs() < 1e-9, "half a step is g = {}", g.g);

    // A reversal mid-flight goes back out of the pose it had reached, in half a wilt.
    let reversed = advance_growth(g, None, false, 0.0);
    assert_same_visual(reversed, g, "a reversal with no time must not move the picture");
    assert_eq!((reversed.from, reversed.to), (Some(0), None), "the step must turn around");
    assert!((reversed.g - 0.5).abs() < 1e-9);
    let mut back = g;
    let mut calls = 0;
    while !(back.from == None && back.to == None) {
        back = advance_growth(back, None, false, DT);
        calls += 1;
        assert!(calls < 200, "the reversal never reached bare ground");
        assert!(
            weight_of(back, Some(0)) <= 0.5 + 1e-9,
            "the plant grew while wilting: {back:?}"
        );
    }
    let wilted = calls as f64 * DT;
    assert!(
        (0.5 * STAGE_WILT_SECONDS - DT..=0.5 * STAGE_WILT_SECONDS + DT).contains(&wilted),
        "half a step wilted in {wilted} s, not about {} s",
        0.5 * STAGE_WILT_SECONDS
    );
    assert!(STAGE_WILT_SECONDS < STAGE_GROW_SECONDS, "wilting must be the quicker direction");

    // `dt = 0` records the target and nothing else: a repeated observe of one tick must not
    // start a step, and must not advance one.
    let idle = Growth::snapped(Some(1), false);
    for dt in [0.0, -0.0, -1.0, f64::NAN, f64::NEG_INFINITY] {
        let still = advance_growth(idle, Some(2), false, dt);
        assert_eq!(
            still,
            Growth { target: Some(2), ..idle },
            "dt {dt} must only record the target"
        );
        let held = advance_growth(g, Some(2), false, dt);
        assert_eq!(held.g, g.g, "dt {dt} advanced an in-flight step");
        assert_eq!((held.from, held.to), (g.from, g.to));
    }
    // But a retarget that reverses an in-flight step happens even with no time: the pair
    // swaps and the progress is measured the other way round, so the picture is unchanged.
    let turned = advance_growth(g, None, false, 0.0);
    assert_eq!((turned.from, turned.to), (Some(0), None));
    assert!(
        (turned.g - (1.0 - g.g)).abs() < 1e-12,
        "a reversal with no time must not also advance: {} vs {}",
        turned.g,
        1.0 - g.g
    );
    assert_same_visual(turned, g, "a reversal with no time");
}

#[test]
fn advance_growth_fades_fruit_in_only_while_idle_at_stage_two_and_in_fruit() {
    let full = Growth::snapped(Some(2), false);
    assert_eq!(full.fruit, 0.0);
    assert_eq!(Growth::snapped(Some(2), true).fruit, 1.0, "a full plant in fruit snaps shown");
    for stage in [None, Some(0), Some(1)] {
        assert_eq!(Growth::snapped(stage, true).fruit, 0.0, "{stage:?} has no fruit to show");
    }

    let mut g = full;
    let mut calls = 0;
    let mut last = 0.0;
    while g.fruit < 1.0 {
        g = advance_growth(g, Some(2), true, DT);
        calls += 1;
        assert!(calls < 200, "the fruit accent never arrived");
        assert!(g.fruit > last, "the accent stalled at {}", g.fruit);
        assert!(g.fruit <= 1.0, "the accent overshot to {}", g.fruit);
        last = g.fruit;
    }
    let seconds = calls as f64 * DT;
    assert!(
        (FRUIT_FADE_SECONDS - DT..=FRUIT_FADE_SECONDS + DT).contains(&seconds),
        "the accent faded in over {seconds} s, not {FRUIT_FADE_SECONDS}"
    );

    // It leaves at once when the fruit does, and when the plant stops being full-grown.
    assert_eq!(advance_growth(g, Some(2), false, DT).fruit, 0.0, "the accent lingered");
    assert_eq!(advance_growth(g, Some(2), false, 0.0).fruit, 0.0, "the accent lingered at dt 0");
    let wilting = advance_growth(g, Some(1), true, DT);
    assert_ne!((wilting.from, wilting.to), (Some(2), Some(2)));
    assert_eq!(wilting.fruit, 0.0, "a plant leaving stage 2 kept its accent");
    // And it never starts below stage 2.
    let mut sprout = Growth::snapped(Some(1), false);
    for _ in 0..10 {
        sprout = advance_growth(sprout, Some(1), true, DT);
        assert_eq!(sprout.fruit, 0.0, "a stage-1 plant grew fruit");
    }
}

// ---------------------------------------------------------------------------
// 10. growth_between
// ---------------------------------------------------------------------------

/// The five things a tick can do to a cell, each named in the doc comment. A frame at `f`
/// must leave `prev`'s picture at 0 and arrive at `cur`'s at 1; anything else makes growth
/// step at 20 Hz or jump at a tick boundary.
#[test]
fn growth_between_runs_from_prev_to_cur_for_every_documented_case() {
    let g = |from, to, progress, fruit| Growth { from, to, g: progress, target: Some(2), fruit };
    let cases: [(&str, Growth, Growth); 5] = [
        ("the same step in flight", g(None, Some(0), 0.2, 0.0), g(None, Some(0), 0.45, 0.0)),
        ("a step started from idle", g(Some(0), Some(0), 1.0, 0.0), g(Some(0), Some(1), 0.3, 0.0)),
        ("a step completed into idle", g(Some(0), Some(1), 0.6, 0.0), g(Some(1), Some(1), 1.0, 0.0)),
        ("a reversal", g(None, Some(0), 0.5, 0.0), g(Some(0), None, 0.6, 0.0)),
        ("a reversal completed back", g(None, Some(0), 0.5, 0.0), g(None, None, 1.0, 0.0)),
    ];
    for (what, prev, cur) in cases {
        assert_same_visual(growth_between(prev, cur, 0.0), prev, &format!("{what} at f = 0"));
        assert_same_visual(growth_between(prev, cur, 1.0), cur, &format!("{what} at f = 1"));
        for f in [f64::NAN, f64::INFINITY, -1.0] {
            assert_same_visual(growth_between(prev, cur, f), prev, &format!("{what} at f = {f}"));
        }
        assert_same_visual(growth_between(prev, cur, 2.0), cur, &format!("{what} past f = 1"));
        // Continuous in between: no stage's share of the picture jumps.
        let mut last = growth_between(prev, cur, 0.0);
        let mut moved = 0.0f64;
        for step in 1..=50 {
            let now = growth_between(prev, cur, f64::from(step) / 50.0);
            assert_eq!(now.target, cur.target, "{what}: the target is always this tick's");
            for stage in STAGES {
                let jump = (weight_of(now, stage) - weight_of(last, stage)).abs();
                assert!(jump <= 0.05 + 1e-9, "{what}: stage {stage:?} jumped by {jump}");
                moved += jump;
            }
            last = now;
        }
        assert!(moved > 0.05, "{what}: the fixture does not move at all");
    }

    // The fruit accent is a linear mix whatever the pair did.
    let prev = g(Some(2), Some(2), 1.0, 0.2);
    let cur = g(Some(2), Some(2), 1.0, 0.8);
    for f in [0.0, 0.25, 0.5, 1.0] {
        let want = 0.2 + 0.6 * f;
        assert!(
            (growth_between(prev, cur, f).fruit - want).abs() < 1e-9,
            "fruit at f = {f} is {}",
            growth_between(prev, cur, f).fruit
        );
    }

    // A snap or a band change is not a transition at all: the frame shows this tick.
    let before = g(Some(2), Some(2), 1.0, 0.0);
    let after = g(None, Some(0), 0.3, 0.0);
    for f in [0.0, 0.3, 0.7, 1.0] {
        let drawn = growth_between(before, after, f);
        assert_eq!(
            (drawn.from, drawn.to, drawn.g),
            (after.from, after.to, after.g),
            "an unrelated pair must draw the new state at f = {f}"
        );
    }
}

// ---------------------------------------------------------------------------
// 11. the presenter's pacing
// ---------------------------------------------------------------------------

#[test]
fn a_fresh_presenter_snaps_a_rich_world_instead_of_replaying_its_growth() {
    let mut p = ArtPresenter::new(pack());
    let v = rich_view(500);
    let first = observe_draw(&mut p, &v, 0.5);
    for cell in CellId::all() {
        let growth = p.growth_of(cell);
        assert_eq!(
            growth,
            Growth::snapped(p.stage_of(cell), false),
            "{cell:?} did not snap to its target"
        );
        assert_eq!(p.growth_prev_of(cell), growth, "a snap must leave no transition behind");
    }
    for i in 0..p.columns().len() {
        assert_eq!(
            p.tall_growth_of(i),
            TallGrowth {
                height: f64::from(p.segments_of(i)),
                target: p.segments_of(i)
            },
            "column {i} did not snap"
        );
    }
    assert!(
        CellId::all().any(|c| p.stage_of(c) == Some(2)),
        "the fixture must grow full plants"
    );
    assert!((0..p.columns().len()).any(|i| p.segments_of(i) > 0), "no column grew");

    // The same tick, observed again and again, is no simulated time at all.
    for _ in 0..20 {
        p.observe(&v);
    }
    assert_same_canvas(&first, &draw(&mut p, &v, 0.5), "20 more observes of the same tick");
}

#[test]
fn a_cell_that_turns_rich_grows_through_every_stage_at_the_documented_pace() {
    let cell = full_foliage_cell();
    let mut p = ArtPresenter::new(pack_without(true, false, false));
    p.observe(&bare_view(0));
    assert_eq!(p.growth_of(cell), Growth::snapped(None, false));

    // One tick of rich world: the target is already stage 2, the picture has barely left bare.
    p.observe(&rich_view(1));
    assert_eq!(p.stage_of(cell), Some(2), "the target is not paced");
    let first = p.growth_of(cell);
    assert_eq!((first.from, first.to), (None, Some(0)), "the visual must start from bare");
    assert!(first.g < 0.05);

    let mut images: Vec<(u64, Canvas)> = vec![(1, draw(&mut p, &rich_view(1), 0.0))];
    let grow_ticks = (STAGE_GROW_SECONDS / DT).round() as u64;
    let at_stage_one = grow_ticks + 2;
    let idle_at_two = 3 * grow_ticks + 3;
    for tick in 2..=idle_at_two {
        let v = rich_view(tick);
        p.observe(&v);
        if tick == at_stage_one {
            let growth = p.growth_of(cell);
            assert_eq!(
                (growth.from, growth.to),
                (Some(0), Some(1)),
                "after {} s the sprout must be growing into stage 1",
                tick as f64 * DT
            );
        }
        if [at_stage_one, 2 * grow_ticks, idle_at_two].contains(&tick) {
            images.push((tick, draw(&mut p, &v, 0.0)));
        }
    }
    let grown = p.growth_of(cell);
    assert_eq!(
        (grown.from, grown.to, grown.g),
        (Some(2), Some(2), 1.0),
        "after {} s the plant must be idle at stage 2",
        idle_at_two as f64 * DT
    );
    for pair in images.windows(2) {
        assert!(
            !differing(&pair[0].1, &pair[1].1).is_empty(),
            "ticks {} and {} drew the same image",
            pair[0].0,
            pair[1].0
        );
    }

    // Caught up, the paced presenter draws exactly what a presenter that snapped draws.
    let v = rich_view(idle_at_two);
    let mut fresh = ArtPresenter::new(pack_without(true, false, false));
    let snapped = observe_draw(&mut fresh, &v, 0.0);
    assert_same_canvas(&images.last().unwrap().1, &snapped, "a caught-up world vs a snapped one");
}

#[test]
fn the_render_fraction_and_repeated_draws_never_move_the_growth() {
    let mut thirds = ArtPresenter::new(pack_without(true, true, true));
    let mut halves = ArtPresenter::new(pack_without(true, true, true));
    let cells: Vec<CellId> = CellId::all().collect();
    for tick in 0..=40u64 {
        let v = if tick == 0 { bare_view(0) } else { rich_view(tick) };
        thirds.observe(&v);
        halves.observe(&v);
        let mut canvas = Canvas::new();
        for frame in 0..FRAMES_PER_TICK {
            thirds.draw(&v, frame as f64 / FRAMES_PER_TICK as f64, &mut canvas);
        }
        halves.draw(&v, 0.5, &mut canvas);
        for &cell in &cells {
            assert_eq!(
                thirds.growth_of(cell),
                halves.growth_of(cell),
                "{cell:?} diverged at tick {tick} because of the render fraction"
            );
        }
    }

    // Fifty draws of one frame are one frame, pixel for pixel.
    let mut p = ArtPresenter::new(pack_without(true, false, false));
    p.observe(&bare_view(0));
    let v = rich_view(1);
    p.observe(&v);
    let before: Vec<Growth> = cells.iter().map(|&c| p.growth_of(c)).collect();
    let first = draw(&mut p, &v, 0.25);
    for _ in 0..49 {
        assert_same_canvas(&first, &draw(&mut p, &v, 0.25), "a repeated draw");
    }
    assert_eq!(cells.iter().map(|&c| p.growth_of(c)).collect::<Vec<_>>(), before);
    // And an observe of a tick already seen is no simulated time at all: `dt = 0` starts no
    // step and advances none, so the growth does not move, the previous-tick copy the
    // frames interpolate from stays what the previous *tick* left (it is not folded
    // forward to this tick's state), and every frame inside the tick — not only its end —
    // draws the same picture as before the repeated observe.
    let prev_before: Vec<Growth> = cells.iter().map(|&c| p.growth_prev_of(c)).collect();
    let end_of_tick = draw(&mut p, &v, 1.0);
    p.observe(&v);
    assert_eq!(cells.iter().map(|&c| p.growth_of(c)).collect::<Vec<_>>(), before);
    assert_eq!(cells.iter().map(|&c| p.growth_prev_of(c)).collect::<Vec<_>>(), prev_before);
    assert_same_canvas(&end_of_tick, &draw(&mut p, &v, 1.0), "a repeated observe of one tick");
    assert_same_canvas(&first, &draw(&mut p, &v, 0.25), "a mid-tick frame after a repeated observe");
}

#[test]
fn a_backwards_tick_snaps_and_draws_what_a_fresh_presenter_draws() {
    let art = || pack_without(true, false, false);
    let mut used = ArtPresenter::new(art());
    for tick in 0..=30u64 {
        used.observe(&rich_view(500 + tick));
    }
    let rewound = rich_view(7);
    let mut fresh = ArtPresenter::new(art());
    let expected = observe_draw(&mut fresh, &rewound, 0.4);
    let actual = observe_draw(&mut used, &rewound, 0.4);
    assert_same_canvas(&actual, &expected, "a rewound world must be drawn as a new one");
    for cell in CellId::all() {
        assert_eq!(used.growth_of(cell), fresh.growth_of(cell), "{cell:?}");
    }
}

#[test]
fn growth_interrupted_mid_stage_wilts_back_without_reaching_the_stage_it_was_climbing_to() {
    let cell = full_foliage_cell();
    let mut p = ArtPresenter::new(pack_without(true, true, true));
    p.observe(&bare_view(0));
    let mut tick = 1u64;
    // Grow until the visual is in flight from stage 1 toward stage 2.
    while !(p.growth_of(cell).from == Some(1) && p.growth_of(cell).to == Some(2)) {
        p.observe(&rich_view(tick));
        tick += 1;
        assert!(tick < 400, "the fixture never started its last stage");
    }
    while p.growth_of(cell).g < 0.4 {
        p.observe(&rich_view(tick));
        tick += 1;
    }
    let reached = weight_of(p.growth_of(cell), Some(2));
    assert!(reached > 0.3 && reached < 0.6, "the fixture interrupts at {reached} of stage 2");

    let mut highest = reached;
    while p.growth_of(cell) != Growth::snapped(None, false) {
        p.observe(&bare_view(tick));
        tick += 1;
        assert!(tick < 1200, "the interrupted plant never returned to bare ground");
        let growth = p.growth_of(cell);
        let now = weight_of(growth, Some(2));
        assert!(
            now <= highest + 1e-9,
            "the plant grew toward stage 2 after being cut off: {now} over {highest}"
        );
        highest = highest.min(now);
        assert!(
            !(growth.from == Some(2) && growth.to == Some(2)),
            "the plant completed the stage it was interrupted in: {growth:?}"
        );
    }
    assert_eq!(p.stage_of(cell), None);
}

#[test]
fn flooding_a_cell_starts_its_reed_from_bare_ground_and_paces_it() {
    let cell = full_foliage_cell();
    let mut p = ArtPresenter::new(pack_without(true, false, true));
    // A full-grown foliage plant, snapped.
    let mut dry = rich_view(100);
    p.observe(&dry);
    assert_eq!(p.band_at(cell), Band::Foliage);
    assert_eq!(p.growth_of(cell), Growth::snapped(Some(2), false));

    let mut flooded = rich_view(101);
    flooded.water[cell.index()] = 1.2;
    assert_eq!(cell_band(cell, Some(1.2)), Band::Water);
    p.observe(&flooded);
    assert_eq!(p.band_at(cell), Band::Water, "the cell must be drawn as water");
    assert_eq!(species_of(Band::Water, cell), "reedspire");
    let after = p.growth_of(cell);
    assert_eq!(after.from, None, "a cell that changed band must start over from bare ground");
    assert_eq!(after.to, Some(0), "and then grow paced, not snap to its reed");
    assert!(after.g < 0.05, "the reed appeared at {} of its first stage", after.g);
    assert_eq!(p.stage_of(cell), Some(2), "the pool warrants a full reed");

    // Paced: it takes the documented stage time to get there, not one tick.
    let mut tick = 102u64;
    while p.growth_of(cell) != Growth::snapped(Some(2), false) {
        let mut v = rich_view(tick);
        v.water[cell.index()] = 1.2;
        p.observe(&v);
        tick += 1;
        assert!(tick < 102 + 400, "the reed never finished growing");
    }
    let seconds = (tick - 101) as f64 * DT;
    assert!(
        seconds > 2.0 * STAGE_GROW_SECONDS,
        "the reed grew three stages in {seconds} s, which is not paced"
    );
    // The water line is strict: at exactly REED_DEPTH the cell keeps its own band.
    dry.tick = tick;
    dry.water[cell.index()] = REED_DEPTH;
    p.observe(&dry);
    assert_eq!(p.band_at(cell), Band::Foliage);
}

/// At 60 fps growth must move by a fraction of a stage per frame. The tolerance is derived
/// from two independent measurements through the same public API: `sway`, the largest
/// per-frame change a *snapped* presenter shows on the same frames (the clip's own motion,
/// which an in-flight cell pays for twice because it draws two stages at once), and
/// `whole_plant`, the difference between bare ground and the grown plant. A stage takes
/// `STAGE_GROW_SECONDS · 60` frames, so paced growth may contribute at most
/// `whole_plant / frames_per_stage` per frame; the factor of 4 is slack for the cross-fade
/// drawing two layers and for the reveal mask's ramp. Growth stepped once per tick would
/// move a whole frame's worth of the plant at a tick boundary and blow through this.
#[test]
fn growth_drawn_at_60_fps_never_steps_a_stage_in_one_frame() {
    let cell = full_foliage_cell();
    let window = near(cell);
    let art = || pack_without(true, false, true);
    let rich = |tick: u64| one_cell_view(tick, cell, 1.0);

    // The frames this test walks: the first second of growth and the two stage boundaries.
    let ticks: Vec<u64> = (1..=12).chain(70..=92).chain(152..=174).collect();

    let thresholds = stage_thresholds(Band::Foliage);
    let mut sway = 0.0f32;
    for density in [thresholds[1] - 1e-9, thresholds[2] - 1e-9, 1.0] {
        let mut control = ArtPresenter::new(art());
        let mut last: Option<Canvas> = None;
        let mut previous_tick = 0;
        for &tick in &ticks {
            let v = one_cell_view(tick, cell, density);
            control.observe(&v);
            for frame in 0..FRAMES_PER_TICK {
                let image = draw(&mut control, &v, frame as f64 / FRAMES_PER_TICK as f64);
                if let Some(last) = &last {
                    if previous_tick + 1 == tick || frame > 0 {
                        sway = sway.max(max_diff_at(&image, last, &window));
                    }
                }
                last = Some(image);
            }
            previous_tick = tick;
        }
        assert!(sway > 0.0, "the control plant at density {density} does not move at all");
    }

    let mut p = ArtPresenter::new(art());
    p.observe(&bare_view(0));
    let bare = draw(&mut p, &bare_view(0), 0.0);
    let mut grown = ArtPresenter::new(art());
    let whole_plant = max_diff_at(&observe_draw(&mut grown, &rich(200), 0.0), &bare, &window);
    assert!(whole_plant > 0.05, "the fixture's plant is invisible");
    let frames_per_stage = STAGE_GROW_SECONDS * 60.0;
    let bound = 2.0 * sway + 4.0 * whole_plant / frames_per_stage as f32;
    println!("sway {sway}, whole plant {whole_plant}, bound {bound}");

    let mut last: Option<Canvas> = None;
    let mut previous_tick = 0u64;
    let mut worst = 0.0f32;
    let mut tick_iter = ticks.iter().peekable();
    while let Some(&tick) = tick_iter.next() {
        let v = rich(tick);
        p.observe(&v);
        for frame in 0..FRAMES_PER_TICK {
            let f = frame as f64 / FRAMES_PER_TICK as f64;
            let image = draw(&mut p, &v, f);
            if let Some(last) = &last {
                if previous_tick + 1 == tick || frame > 0 {
                    let moved = max_diff_at(&image, last, &window);
                    worst = worst.max(moved);
                    assert!(
                        moved <= bound,
                        "tick {tick} frame {frame} moved a pixel by {moved}, over the derived \
                         bound {bound} (sway {sway}, whole plant {whole_plant})"
                    );
                }
            }
            last = Some(image);
        }
        previous_tick = tick;
        // The end of a tick meets the start of the next: the same simulated instant, the
        // same growth, so the same picture.
        if tick_iter.peek() == Some(&&(tick + 1)) {
            let end = draw(&mut p, &v, 1.0);
            let next = rich(tick + 1);
            p.observe(&next);
            let start = draw(&mut p, &next, 0.0);
            assert_close_canvas(&end, &start, 1e-3, &format!("the tick {tick} boundary"));
            last = Some(start);
            previous_tick = tick + 1;
            tick_iter.next();
        }
    }
    println!("the worst growing frame moved {worst}");
    assert!(worst > 0.0, "the growing plant never moved");
}

// ---------------------------------------------------------------------------
// 12. tall columns
// ---------------------------------------------------------------------------

/// The column's own axis: the chart direction in which its face's height rises, recovered
/// from the documented heading rule (`stalk_heading(up) = (−up.y, up.x)`).
fn column_up(face: Face, cx: u8) -> Vec2 {
    let heading = tall_heading(face, cx);
    Vec2::new(heading.y, -heading.x)
}

/// How far up the column, in pixels above the horizon cell's centre, a pixel centre sits.
fn up_px(face: Face, cx: u8, x: u8, y: u8) -> f64 {
    let origin = tall_anchor(face, cx, 0).chart();
    let up = column_up(face, cx);
    let d = Vec2::new(f64::from(x) + 0.5 - origin.x, f64::from(y) + 0.5 - origin.y);
    up.dot(d)
}

/// A view whose only rich cells are one column's foliage cells, so that column is the only
/// thing on the cube that grows.
fn column_view(tick: u64, face: Face, cx: u8) -> RenderView {
    let mut v = bare_view(tick);
    for cell in CellId::all() {
        if cell.face() == face && cell.cx() == cx && band_of(cell) == Band::Foliage {
            v.producer[cell.index()] = saturation();
        }
    }
    v
}

/// The tall column this test drives, and a pack that draws nothing but columns.
fn a_column(vine: bool) -> TallColumn {
    tall_columns()
        .into_iter()
        .find(|c| c.vine == vine)
        .expect("the cube carries columns with and without vines")
}

#[test]
fn a_tall_column_rises_and_falls_at_its_documented_rate_and_glides_its_cap() {
    let column = a_column(true);
    let index = tall_columns().iter().position(|c| *c == column).unwrap();
    let mut p = ArtPresenter::new(pack_without(false, true, true));
    p.observe(&bare_view(0));
    assert_eq!(p.tall_growth_of(index).height, 0.0);

    let v = column_view(1, column.face, column.cx);
    let target = tall_target(column_density(&v, column.face, column.cx));
    assert!(target >= 4, "the fixture column must grow tall, not {target}");
    assert!(target <= TALL_MAX_SEGMENTS);

    let rate = TALL_GROW_PX_PER_S / 4.0;
    let mut tick = 1u64;
    while tick <= 600 {
        let v = column_view(tick, column.face, column.cx);
        p.observe(&v);
        assert_eq!(p.segments_of(index), target, "tick {tick} lost the target");
        let height = p.tall_growth_of(index).height;
        let want = (tick as f64 * DT * rate).min(f64::from(target));
        assert!(
            (height - want).abs() < 1e-9,
            "tick {tick}: the column is {height} segments tall, not {want}"
        );
        assert!(height <= f64::from(target) + 1e-12, "the column overshot its target");
        if height >= f64::from(target) {
            break;
        }
        tick += 1;
    }
    assert_eq!(p.tall_growth_of(index).height, f64::from(target));

    // Decline is the faster rate.
    let full_at = tick;
    let mut tick = full_at + 1;
    for step in 1..=10u64 {
        p.observe(&bare_view(tick));
        assert_eq!(p.segments_of(index), 0, "a bare column keeps a target");
        let want = f64::from(target) - step as f64 * DT * TALL_WILT_PX_PER_S / 4.0;
        assert!(
            (p.tall_growth_of(index).height - want).abs() < 1e-9,
            "the column fell to {} instead of {want}",
            p.tall_growth_of(index).height
        );
        tick += 1;
    }
    assert!(TALL_WILT_PX_PER_S > TALL_GROW_PX_PER_S, "declining must be the quicker direction");

    // The cap glides: the topmost lit row of the column climbs as the column grows.
    let mut p = ArtPresenter::new(pack_without(false, true, true));
    let mut bare = ArtPresenter::new(pack_without(true, true, true));
    p.observe(&bare_view(0));
    let background = observe_draw(&mut bare, &column_view(1, column.face, column.cx), 0.0);
    let mut tops: Vec<(u64, f64)> = Vec::new();
    for tick in 1..=400u64 {
        let v = column_view(tick, column.face, column.cx);
        p.observe(&v);
        if tick % 100 == 0 {
            let image = draw(&mut p, &v, 0.0);
            let top = differing(&image, &background)
                .into_iter()
                .filter(|&(f, _, _)| f == column.face)
                .map(|(_, x, y)| up_px(column.face, column.cx, x, y))
                .fold(f64::NEG_INFINITY, f64::max);
            assert!(top.is_finite(), "the column drew nothing at tick {tick}");
            tops.push((tick, top));
        }
    }
    for pair in tops.windows(2) {
        assert!(
            pair[1].1 > pair[0].1 + 2.0,
            "the column's top went from {:.2} px at tick {} to {:.2} px at tick {}: the cap is \
             not gliding with it",
            pair[0].1,
            pair[0].0,
            pair[1].1,
            pair[1].0
        );
    }
}

#[test]
fn a_completed_trunk_segment_is_identical_while_the_column_is_still_growing() {
    let column = a_column(false);
    let index = tall_columns().iter().position(|c| *c == column).unwrap();
    let art = || pack_without(false, true, true);
    let mut growing = ArtPresenter::new(art());
    growing.observe(&bare_view(0));
    let tick = 267u64;
    for t in 1..=tick {
        growing.observe(&column_view(t, column.face, column.cx));
    }
    let v = column_view(tick, column.face, column.cx);
    let height = growing.tall_growth_of(index).height;
    assert!(
        (4.5..6.0).contains(&height),
        "the fixture must be mid-column, not {height} segments"
    );
    let partial = draw(&mut growing, &v, 0.0);

    // The same tick, so the same sway, with the column at its full height.
    let mut finished = ArtPresenter::new(art());
    finished.observe(&v);
    assert_eq!(finished.tall_growth_of(index).height, f64::from(finished.segments_of(index)));
    let whole = draw(&mut finished, &v, 0.0);

    // Rows below the growing cap's tile and below the grown height belong to trunk tiles
    // that are finished in both frames, so they must be the same pixels.
    let ceiling = tall_grown_px(height).min(4.0 * (height + 1.0) - 8.0) - 2.0;
    assert!(ceiling > 8.0, "the fixture leaves no settled rows to compare");
    let settled: Vec<(Face, u8, u8)> = (0..FACE_SIZE as u8)
        .flat_map(|y| (0..FACE_SIZE as u8).map(move |x| (column.face, x, y)))
        .filter(|&(_, x, y)| up_px(column.face, column.cx, x, y) <= ceiling)
        .collect();
    let d = max_diff_at(&partial, &whole, &settled);
    assert!(d < 1e-6, "a settled trunk row differs by {d} between a growing and a full column");

    let mut bare = ArtPresenter::new(pack_without(true, true, true));
    let background = observe_draw(&mut bare, &v, 0.0);
    assert!(
        max_diff_at(&partial, &background, &settled) > 0.05,
        "the fixture paints no trunk in the rows it compares"
    );
    // And above the cap the two frames are of course different pictures.
    assert!(!differing(&partial, &whole).is_empty(), "the two heights drew the same column");
}

#[test]
fn a_tall_column_crossing_a_whole_segment_does_not_pop_a_tile() {
    let column = a_column(false);
    let index = tall_columns().iter().position(|c| *c == column).unwrap();
    let mut p = ArtPresenter::new(pack_without(false, true, true));
    p.observe(&bare_view(0));
    for t in 1..=140u64 {
        p.observe(&column_view(t, column.face, column.cx));
    }

    // Walk 60 fps frames across the height reaching a whole number of segments.
    let mut previous = p.tall_growth_of(index).height;
    let mut last: Option<Canvas> = None;
    let mut crossings: Vec<f32> = Vec::new();
    let mut elsewhere: Vec<f32> = Vec::new();
    for tick in 141..=180u64 {
        let v = column_view(tick, column.face, column.cx);
        p.observe(&v);
        let current = p.tall_growth_of(index).height;
        for frame in 0..FRAMES_PER_TICK {
            let f = frame as f64 / FRAMES_PER_TICK as f64;
            let height = tall_between(
                TallGrowth { height: previous, target: 0 },
                TallGrowth { height: current, target: 0 },
                f,
            )
            .height;
            let image = draw(&mut p, &v, f);
            if let Some(last) = &last {
                let moved = max_diff(&image, last);
                // `height` is this frame's; the previous frame was 1/180 segment lower.
                let before = height - (current - previous) / FRAMES_PER_TICK as f64;
                if before.floor() != height.floor() {
                    crossings.push(moved);
                } else {
                    elsewhere.push(moved);
                }
            }
            last = Some(image);
        }
        previous = current;
    }
    assert!(!crossings.is_empty(), "the sweep did not cross a whole segment");
    assert!(elsewhere.len() > 50);
    let ordinary = elsewhere.iter().copied().fold(0.0, f32::max);
    let popped = crossings.iter().copied().fold(0.0, f32::max);
    println!("segment crossing moved {popped}, ordinary frames {ordinary}");
    assert!(ordinary > 0.0, "nothing moved at all in the sweep");
    assert!(
        popped <= 1.5 * ordinary + 0.01,
        "the frame that completed a segment moved a pixel by {popped}, against {ordinary} on \
         every other frame: a tile popped in"
    );
}

#[test]
fn tall_grown_px_adds_four_pixels_a_segment_and_is_continuous_at_the_first() {
    for n in 1..=i32::from(TALL_MAX_SEGMENTS) {
        assert_eq!(
            tall_grown_px(f64::from(n)),
            f64::from(4 * n + 8),
            "a whole column of {n} segments"
        );
    }
    // `tall_anchor_at` is the same ladder at a continuous index: four pixels up the face per
    // segment, which is what lets the cap glide instead of stepping a whole cell.
    let column = a_column(false);
    let (face, cx) = (column.face, column.cx);
    for i in 0..=TALL_MAX_SEGMENTS {
        let whole = tall_anchor(face, cx, i);
        let same = tall_anchor_at(face, cx, f64::from(i));
        assert_eq!((whole.face, whole.u, whole.v), (same.face, same.u, same.v), "tile {i}");
    }
    for step in 0..=40 {
        let i = f64::from(step) / 4.0;
        let here = tall_anchor_at(face, cx, i);
        let up = column_up(face, cx);
        let origin = tall_anchor(face, cx, 0).chart();
        let along = up.dot(Vec2::new(here.u - origin.x, here.v - origin.y));
        assert!((along - 4.0 * i).abs() < 1e-9, "tile {i} sits {along} px up, not {}", 4.0 * i);
    }

    // `advance_tall` is what the presenter paces with, and it never overshoots in either
    // direction whatever the step.
    let mut g = TallGrowth { height: 0.0, target: 0 };
    for _ in 0..1000 {
        g = advance_tall(g, TALL_MAX_SEGMENTS, DT);
        assert!(g.height <= f64::from(TALL_MAX_SEGMENTS) + 1e-12, "{} segments", g.height);
        assert_eq!(g.target, TALL_MAX_SEGMENTS);
    }
    assert_eq!(g.height, f64::from(TALL_MAX_SEGMENTS));
    for dt in [0.0, -1.0, f64::NAN] {
        assert_eq!(advance_tall(g, 0, dt).height, g.height, "dt {dt} moved the column");
        assert_eq!(advance_tall(g, 0, dt).target, 0, "dt {dt} must still record the target");
    }
    let mut g = advance_tall(g, 0, 1.0);
    assert!((g.height - (f64::from(TALL_MAX_SEGMENTS) - TALL_WILT_PX_PER_S / 4.0)).abs() < 1e-12);
    for _ in 0..1000 {
        g = advance_tall(g, 0, DT);
        assert!(g.height >= 0.0, "the column fell through the floor to {}", g.height);
    }
    assert_eq!(g.height, 0.0);
    // The first segment grows out of the base's five pixels, continuously at both ends.
    let base = tall_grown_px(0.0);
    assert_eq!(base, 5.0, "the base paints the trunk pattern up to 5 px");
    for height in [-1.0, -0.0, f64::NAN, f64::NEG_INFINITY] {
        assert_eq!(tall_grown_px(height), base, "height {height} must read as bare");
    }
    assert!((tall_grown_px(1e-9) - base).abs() < 1e-6, "the first segment pops out of the base");
    let just_below = tall_grown_px(1.0 - 1e-9);
    assert!(
        (just_below - tall_grown_px(1.0)).abs() < 1e-6,
        "the grown height jumps at one segment: {just_below} then {}",
        tall_grown_px(1.0)
    );
    // Monotone over the whole range, and never faster than the first segment's own slope:
    // it runs from the base's 5 px to TALL_JOIN's 12 px over one segment (7 px per segment),
    // and adds 4 px per segment above that.
    let mut last = base;
    let mut height = 0.0;
    while height <= f64::from(TALL_MAX_SEGMENTS) {
        let now = tall_grown_px(height);
        assert!(now >= last - 1e-12, "the grown height fell at {height}");
        assert!(now - last <= 7.0 * 0.05 + 1e-9, "the grown height jumped at {height}");
        last = now;
        height += 0.05;
    }
}

// ---------------------------------------------------------------------------
// 13. rain
// ---------------------------------------------------------------------------

#[test]
fn a_falling_streak_keeps_constant_light_and_its_head_inside_its_cell() {
    let cell = CellId::new(Face::Front, 7, 5);
    let up = up_of(cell).expect("a side-face cell has an up");
    let down = Vec2::new(-up.x, -up.y);
    let k = 0;
    let origin = rain_origin(cell, k);
    assert!(origin.0 < 4 && origin.1 < 4, "a sub-cell origin is {origin:?}");

    let mut seen_heads = std::collections::HashSet::new();
    let mut steps = 0;
    let mut seconds = 0.0;
    while seconds < 3.0 * RAIN_PERIOD {
        let marks = rain_marks(cell, k, seconds);
        assert_eq!(marks.len(), 3, "a mid-face streak at {seconds} s is {marks:?}");
        let sum: f64 = marks.iter().map(|&(_, w)| f64::from(w)).sum();
        assert!(
            (sum - 2.0).abs() < 1e-6,
            "the streak's light is {sum}, not 2, at {seconds} s"
        );
        let phase = rain_fall(seconds).fract();
        let weights = [1.0 - phase, 1.0, phase];
        for (i, (&(at, w), want)) in marks.iter().zip(weights).enumerate() {
            assert!(
                (f64::from(w) - want).abs() < 1e-6,
                "mark {i} at {seconds} s weighs {w}, not {want}"
            );
            let expected = (
                (f64::from(marks[0].0 .0) + down.x * i as f64).round() as i64,
                (f64::from(marks[0].0 .1) + down.y * i as f64).round() as i64,
            );
            assert_eq!(
                (i64::from(at.0), i64::from(at.1)),
                expected,
                "mark {i} is not {i} pixels downhill of the head at {seconds} s"
            );
        }
        let head = marks[0].0;
        assert_eq!(
            cell_of(&SurfacePoint::pixel_center(cell.face(), head.0, head.1)),
            cell,
            "the head left its cell at {seconds} s"
        );
        seen_heads.insert(head);

        // Continuity: a millisecond of fall moves no pixel's share of the light far.
        let later = rain_marks(cell, k, seconds + 1e-3);
        for at in marks.iter().map(|m| m.0).chain(later.iter().map(|m| m.0)) {
            let before: f32 = marks.iter().filter(|m| m.0 == at).map(|m| m.1).sum();
            let after: f32 = later.iter().filter(|m| m.0 == at).map(|m| m.1).sum();
            assert!(
                (after - before).abs() < 0.05,
                "the light at {at:?} jumped from {before} to {after} over a millisecond at \
                 {seconds} s"
            );
        }
        steps += 1;
        seconds += 1.0 / 60.0;
    }
    assert!(steps > 50);
    assert_eq!(seen_heads.len(), 4, "the head must visit all four of its cell's pixels");
}

#[test]
fn a_top_face_sparkle_swells_to_one_and_dies_inside_its_window() {
    assert!(RAIN_BLINK < RAIN_PERIOD, "the blink must fit inside the period");
    assert!((rain_blink(RAIN_BLINK / 2.0) - 1.0).abs() < 1e-9, "the sparkle must peak at 1");
    assert!((rain_blink(RAIN_PERIOD + RAIN_BLINK / 2.0) - 1.0).abs() < 1e-9, "it must repeat");
    for seconds in [0.0, RAIN_BLINK, RAIN_BLINK + 0.01, RAIN_PERIOD - 1e-9, RAIN_PERIOD] {
        let u = seconds % RAIN_PERIOD;
        if u >= RAIN_BLINK {
            assert_eq!(rain_blink(seconds), 0.0, "the sparkle is lit at {u} s into its period");
        }
    }
    // Continuous at both edges of the window, and nowhere above 1.
    assert!(rain_blink(1e-9) < 1e-4, "the sparkle switches on: {}", rain_blink(1e-9));
    assert!(
        rain_blink(RAIN_BLINK - 1e-9) < 1e-4,
        "the sparkle switches off: {}",
        rain_blink(RAIN_BLINK - 1e-9)
    );
    let mut peak = 0.0f32;
    let mut seconds = 0.0;
    while seconds < 2.0 * RAIN_PERIOD {
        let blink = rain_blink(seconds);
        assert!((0.0..=1.0).contains(&blink), "blink {blink} at {seconds} s");
        assert_eq!(rain_blink_on(seconds), blink > 0.0, "rain_blink_on disagrees at {seconds} s");
        peak = peak.max(blink);
        seconds += 1e-4;
    }
    assert!(peak > 0.999);
    for seconds in [f64::NAN, f64::INFINITY] {
        assert_eq!(rain_blink(seconds), 0.0, "{seconds} must not sparkle");
        assert!(!rain_blink_on(seconds));
    }

    // On the top face a streak is that sparkle, at one pixel, and nothing when it is dark.
    let cell = CellId::new(Face::Top, 9, 6);
    for &seconds in &[RAIN_BLINK / 2.0, RAIN_BLINK / 4.0, RAIN_BLINK + 0.02, RAIN_PERIOD / 2.0] {
        let marks = rain_marks(cell, 1, seconds);
        if rain_blink_on(seconds) {
            assert_eq!(marks.len(), 1, "a top-face streak is one pixel: {marks:?}");
            assert_eq!(marks[0].1, rain_blink(seconds), "the sparkle's weight is its blink");
            assert_eq!(
                cell_of(&SurfacePoint::pixel_center(
                    Face::Top,
                    marks[0].0 .0,
                    marks[0].0 .1
                )),
                cell,
                "the sparkle left its cell"
            );
        } else {
            assert!(marks.is_empty(), "a dark sparkle still marked {marks:?}");
        }
    }
}

// ---------------------------------------------------------------------------
// 14. bodies
// ---------------------------------------------------------------------------

fn organism(id: OrganismId, mode: Mode) -> OrganismView {
    OrganismView {
        id,
        pos: SurfacePoint::new(Face::Front, 30.0, 30.0),
        heading: Vec2::new(1.0, 0.0),
        lobes: vec![(0.0, 0.0, 2.0)],
        hue: 0.5,
        mode,
        fed: false,
        juvenile: false,
        gestation: None,
        form: u8::MAX,
        moved: Vec::new(),
    }
}

fn body_view(tick: u64, modes: &[(OrganismId, Mode)]) -> RenderView {
    let mut v = bare_view(tick);
    v.organisms = modes.iter().map(|&(id, mode)| organism(id, mode)).collect();
    v
}

/// The body the presenter must be drawing: the states its memory says are on screen, each
/// sampled from its own clip at this frame's presentation time, in one `stamp_layers`.
fn expected_body(
    art: &ArtPack,
    background: &Canvas,
    body: &OrganismView,
    layers: &[(usize, f32)],
    tick: u64,
    f: f64,
) -> Canvas {
    let rig = rig_of(body.form, body.hue, art.creature_count());
    let seconds = present_seconds(tick, f);
    let poses: Vec<(Pose, f32)> = layers
        .iter()
        .map(|&(state, weight)| {
            let clip = &art.clips[rig * 4 + state];
            let phase = phase_of(body.id, clip.seconds);
            (clip.sample(clip_time(clip, seconds, phase, body.gestation)), weight)
        })
        .collect();
    let (anchor, facing) = interpolate(&body.moved, body.pos, body.heading, f);
    let mut canvas = background.clone();
    let mut scratch: Vec<PixelImage> = Vec::new();
    stamp_layers(
        &mut canvas,
        anchor,
        facing,
        &poses,
        1.0,
        1.0,
        Mask::None,
        &mut scratch,
    );
    canvas
}

#[test]
fn a_body_changing_state_cross_fades_from_the_pose_that_is_on_screen() {
    let art = pack();
    let id = OrganismId { slot: 12, generation: 3 };
    let mut p = ArtPresenter::new(pack());
    let t = 400u64;

    // A body first seen is drawn with no fade at all.
    let mut fresh = ArtPresenter::new(pack());
    let seeking = body_view(t, &[(id, Mode::Seeking)]);
    let entered = observe_draw(&mut fresh, &seeking, 0.0);
    let memory = fresh.body_of(id).expect("the presenter must remember a body it has drawn");
    assert_eq!(memory, BodyMemory::entered(state_of(&seeking.organisms[0])));
    assert_eq!(memory.fade_at(present_seconds(t, 0.0)), 1.0, "a snapped body must not fade");
    let background = {
        let mut empty = ArtPresenter::new(pack());
        observe_draw(&mut empty, &bare_view(t), 0.0)
    };
    assert_same_canvas(
        &entered,
        &expected_body(&art, &background, &seeking.organisms[0], &[(1, 1.0)], t, 0.0),
        "a body entering with no fade is its own clip alone",
    );

    // Resting at T, seeking at T + 1: the first frame after the change is still the rest clip.
    p.observe(&body_view(t, &[(id, Mode::Resting)]));
    let moving = body_view(t + 1, &[(id, Mode::Seeking)]);
    p.observe(&moving);
    let body = &moving.organisms[0];
    let start = draw(&mut p, &moving, 0.0);
    let memory = p.body_of(id).unwrap();
    assert_eq!(memory.state, 1);
    assert_eq!(memory.fade_at(present_seconds(t + 1, 0.0)), 0.0, "the fade must start at 0");
    let rest_alone = expected_body(&art, &background, body, &[(0, 1.0)], t + 1, 0.0);
    assert_same_canvas(&start, &rest_alone, "the frame the state changed on");

    // Midway through the fade it is a weighted mix of both clips, each still swaying.
    let midway = 3u64;
    for tick in t + 2..=t + midway + 1 {
        p.observe(&body_view(tick, &[(id, Mode::Seeking)]));
    }
    let mid_view = body_view(t + midway + 1, &[(id, Mode::Seeking)]);
    let seconds = present_seconds(t + midway + 1, 0.0);
    let fade = p.body_of(id).unwrap().fade_at(seconds);
    assert!(
        fade > 0.2 && fade < 0.8,
        "the fixture must sit inside the fade, not at {fade}"
    );
    let layers = p.body_of(id).unwrap().layers_at(seconds);
    assert_eq!(layers.len(), 2, "a fade shows two clips: {layers:?}");
    let mid = draw(&mut p, &mid_view, 0.0);
    assert_same_canvas(
        &mid,
        &expected_body(&art, &background, &mid_view.organisms[0], &layers, t + midway + 1, 0.0),
        "a cross-fade is one weighted stamp of two sampled poses",
    );
    let rest_now = expected_body(&art, &background, &mid_view.organisms[0], &[(0, 1.0)], t + midway + 1, 0.0);
    let move_now = expected_body(&art, &background, &mid_view.organisms[0], &[(1, 1.0)], t + midway + 1, 0.0);
    assert!(!differing(&mid, &rest_now).is_empty(), "the fade is still the rest clip");
    assert!(!differing(&mid, &move_now).is_empty(), "the fade is already the move clip");

    // Once the fade is over, the new clip alone.
    let done = t + 1 + (BODY_FADE_TICKS + 2);
    for tick in t + midway + 2..=done {
        p.observe(&body_view(tick, &[(id, Mode::Seeking)]));
    }
    let done_view = body_view(done, &[(id, Mode::Seeking)]);
    let layers = p.body_of(id).unwrap().layers_at(present_seconds(done, 0.0));
    assert_eq!(layers, vec![(1usize, 1.0f32)], "the fade must finish: {layers:?}");
    assert_same_canvas(
        &draw(&mut p, &done_view, 0.0),
        &expected_body(&art, &background, &done_view.organisms[0], &[(1, 1.0)], done, 0.0),
        "after the fade the body is its new clip alone",
    );
}

/// Ticks a whole body fade takes, rounded up.
const BODY_FADE_TICKS: u64 = (BODY_FADE_SECONDS / DT) as u64 + 1;

#[test]
fn a_body_that_changes_state_twice_inside_one_fade_stays_on_the_pose_it_is_showing() {
    let art = pack();
    let id = OrganismId { slot: 5, generation: 1 };
    let t = 300u64;
    let background = {
        let mut empty = ArtPresenter::new(pack());
        observe_draw(&mut empty, &bare_view(t), 0.0)
    };

    for (second, third, what) in [(Mode::Resting, 0usize, "back to the state it left"), (Mode::Feeding, 2, "on to a third state")] {
        let mut p = ArtPresenter::new(pack());
        p.observe(&body_view(t, &[(id, Mode::Resting)]));
        p.observe(&body_view(t + 1, &[(id, Mode::Seeking)]));
        let half = t + 1 + BODY_FADE_TICKS / 2;
        for tick in t + 2..half {
            p.observe(&body_view(tick, &[(id, Mode::Seeking)]));
        }
        // The frame the switch lands on, drawn from the memory just before and just after it.
        let switch_view = body_view(half, &[(id, second)]);
        let seconds = present_seconds(half, 0.0);
        let fade = p.body_of(id).unwrap().fade_at(seconds);
        assert!(fade > 0.1 && fade < 0.9, "the fixture must switch mid-fade, not at {fade}");
        let before = draw(&mut p, &switch_view, 0.0);
        p.observe(&switch_view);
        let after = draw(&mut p, &switch_view, 0.0);
        let memory = p.body_of(id).unwrap();
        assert_eq!(memory.state, third, "{what}: the new state was not recorded");
        assert_eq!(memory.fade_at(seconds), 0.0, "{what}: the new fade must start at 0");

        let rest = expected_body(&art, &background, &switch_view.organisms[0], &[(0, 1.0)], half, 0.0);
        let moving = expected_body(&art, &background, &switch_view.organisms[0], &[(1, 1.0)], half, 0.0);
        let cut = max_diff(&rest, &moving);
        assert!(cut > 0.05, "the fixture's two clips look alike");
        let step = max_diff(&before, &after);
        println!("{what}: switch step {step}, a cut would be {cut}");
        assert!(
            step <= 0.5 * cut,
            "{what}: the switch moved the body by {step}, half a cut being {}",
            0.5 * cut
        );
        // It is genuinely a blend, not one of the endpoints.
        assert!(!differing(&after, &rest).is_empty() && !differing(&after, &moving).is_empty());
    }
}

#[test]
fn a_body_the_view_drops_is_forgotten() {
    let id = OrganismId { slot: 9, generation: 2 };
    let other = OrganismId { slot: 10, generation: 2 };
    let mut p = ArtPresenter::new(pack_without(false, true, true));
    p.observe(&body_view(50, &[(id, Mode::Resting), (other, Mode::Seeking)]));
    assert!(p.body_of(id).is_some() && p.body_of(other).is_some());
    p.observe(&body_view(51, &[(other, Mode::Seeking)]));
    assert!(p.body_of(id).is_none(), "a dead body must be forgotten");
    assert!(p.body_of(other).is_some(), "a living body must not be");
    // And a recycled slot is a new body: its memory enters fresh rather than fading.
    let reborn = OrganismId { slot: 9, generation: 3 };
    p.observe(&body_view(52, &[(other, Mode::Seeking), (reborn, Mode::Feeding)]));
    assert_eq!(p.body_of(reborn), Some(BodyMemory::entered(2)));
}

// ---------------------------------------------------------------------------
// 15. the fruit accent
// ---------------------------------------------------------------------------

/// A rank-2 lanternstalk slot: the foliage plant with a fruit clip.
fn lanternstalk_cell() -> CellId {
    CellId::all()
        .find(|&c| {
            c.face() == Face::Back
                && band_of(c) == Band::Foliage
                && plant_cap(Band::Foliage, c) == Some(2)
                && species_of(Band::Foliage, c) == "lanternstalk"
                && (4..=7).contains(&c.cy())
                && (4..=11).contains(&c.cx())
        })
        .expect("the cube has a rank-2 lanternstalk slot in the middle of Back")
}

#[test]
fn the_fruit_accent_fades_in_over_its_seconds_and_leaves_the_moment_the_food_does() {
    let art = pack();
    let cell = lanternstalk_cell();
    let plant = art.plant("lanternstalk").unwrap();
    let fruit_clip = plant.fruit.as_ref().expect("lanternstalk fruits");
    let window = near(cell);

    let view_at = |tick: u64| one_cell_view(tick, cell, 1.0);
    let dry = vec![0.0; CELL_COUNT];
    let mut ripe = vec![0.0; CELL_COUNT];
    ripe[cell.index()] = FRUIT_SHOW * 2.5;
    assert!(fruit_stage(Some(ripe[cell.index()])));

    let mut p = ArtPresenter::new(pack_without(true, false, false));
    p.observe_with_fruit(&view_at(0), Some(&dry));
    assert_eq!(p.growth_of(cell), Growth::snapped(Some(2), false), "the plant must start full");

    // The accent fades in over its own seconds, not at once.
    let mut tick = 1u64;
    let fade_ticks = (FRUIT_FADE_SECONDS / DT).round() as u64;
    let mut midway = None;
    while p.growth_of(cell).fruit < 1.0 {
        p.observe_with_fruit(&view_at(tick), Some(&ripe));
        if midway.is_none() && p.growth_of(cell).fruit > 0.45 {
            midway = Some(tick);
        }
        tick += 1;
        assert!(tick < fade_ticks * 3, "the accent never arrived");
    }
    assert!(
        (tick as f64 - 1.0 - fade_ticks as f64).abs() <= 1.0,
        "the accent took {} ticks, not about {fade_ticks}",
        tick - 1
    );

    // Fully in fruit: the fruit clip stamped alone where the stage clip would be.
    let shown = tick - 1 + 4;
    for t in tick..=shown {
        p.observe_with_fruit(&view_at(t), Some(&ripe));
    }
    let v = view_at(shown);
    let expected = |clip: &Clip| {
        let mut background = {
            let mut bare = ArtPresenter::new(pack_without(true, true, false));
            observe_draw(&mut bare, &v, 0.0)
        };
        let pose = clip.sample(
            present_seconds(shown, 0.0)
                + plant_phase_of(cell, clip.seconds),
        );
        let (at, _) = placement_of(cell);
        // The slot's share of the shared breeze at this instant, which every stamp of the
        // slot carries: at this tick the packet is rising, so the plant is leaning.
        let (bend, heading) = slot_wind(
            &slot_of(cell),
            &plant.name,
            plant_bend_budget(plant),
            present_seconds(shown, 0.0),
        );
        assert!(!bend.is_identity(), "the fixture tick must be windy");
        stamp_layers_bent(
            &mut background,
            at,
            heading,
            &[(pose, 1.0)],
            1.0,
            MOTIF_OPACITY,
            Mask::None,
            bend,
            &mut Vec::new(),
        );
        background
    };
    let fruity = expected(fruit_clip);
    let plain = expected(&plant.stages[2]);
    assert!(max_diff_at(&fruity, &plain, &window) > 0.02, "the two clips look alike");
    assert_same_canvas(
        &draw_fruit(&mut p, &v, 0.0, Some(&ripe)),
        &fruity,
        "a plant in fruit draws its fruit clip",
    );

    // The field gates the accent at draw time, whatever the paced value says.
    assert_same_canvas(
        &draw_fruit(&mut p, &v, 0.0, None),
        &plain,
        "a world that publishes no fruit must show none",
    );
    let mut under = vec![0.0; CELL_COUNT];
    under[cell.index()] = FRUIT_SHOW;
    assert_same_canvas(
        &draw_fruit(&mut p, &v, 0.0, Some(&under)),
        &plain,
        "fruit exactly at the threshold is not enough to show",
    );

    // Halfway in, the image is exactly the blend of the two, strictly between them.
    let mut p = ArtPresenter::new(pack_without(true, false, false));
    p.observe_with_fruit(&view_at(0), Some(&dry));
    let half = midway.expect("the sweep must pass the middle of the fade");
    for t in 1..=half {
        p.observe_with_fruit(&view_at(t), Some(&ripe));
    }
    let v = view_at(half);
    let w = p.growth_prev_of(cell).fruit as f32;
    assert!(w > 0.2 && w < 0.8, "the fixture must be mid-fade, not at {w}");
    let blended = draw_fruit(&mut p, &v, 0.0, Some(&ripe));
    let mut worst = (0.0f32, (Face::Top, 0u8, 0u8));
    let plain_half = {
        let mut q = ArtPresenter::new(pack_without(true, false, false));
        q.observe_with_fruit(&view_at(0), Some(&dry));
        for t in 1..=half {
            q.observe_with_fruit(&view_at(t), Some(&dry));
        }
        draw_fruit(&mut q, &v, 0.0, Some(&dry))
    };
    let fruity_half = {
        let mut q = ArtPresenter::new(pack_without(true, false, false));
        // A snap with fruit: the accent is fully on from the first view.
        q.observe_with_fruit(&view_at(half), Some(&ripe));
        draw_fruit(&mut q, &v, 0.0, Some(&ripe))
    };
    for &(f, x, y) in &window {
        let d = (plain_half.get(f, x, y)[0] - fruity_half.get(f, x, y)[0]).abs();
        if d > worst.0 {
            worst = (d, (f, x, y));
        }
    }
    assert!(worst.0 > 0.02, "the two clips do not differ anywhere near the plant");
    let (f, x, y) = worst.1;
    let (lo, hi) = (plain_half.get(f, x, y)[0], fruity_half.get(f, x, y)[0]);
    let mid = blended.get(f, x, y)[0];
    assert!(
        mid > lo.min(hi) + 1e-4 && mid < lo.max(hi) - 1e-4,
        "a half-faded accent drew {mid}, not between {lo} and {hi}"
    );
    let want = lo + (hi - lo) * w;
    assert!(
        (mid - want).abs() < 1e-3,
        "a half-faded accent is not the weighted mix: {mid} vs {want}"
    );

    // The moment the cell stops holding fruit, the accent is gone.
    let next = view_at(half + 1);
    p.observe_with_fruit(&next, Some(&dry));
    assert_eq!(p.growth_of(cell).fruit, 0.0, "the accent lingered in the history");
    let plain_next = {
        let mut q = ArtPresenter::new(pack_without(true, false, false));
        q.observe_with_fruit(&view_at(half + 1), Some(&dry));
        draw_fruit(&mut q, &next, 0.0, Some(&dry))
    };
    assert!(
        max_diff_at(&draw_fruit(&mut p, &next, 0.0, Some(&dry)), &plain_next, &window) < 1e-6,
        "the accent outlived the fruit"
    );
}

/// A host that fell a thousand ticks behind must not fast-forward every plant through its
/// whole growth in one observe: the step is capped, so the cell advances by one cap's worth
/// of a stage and no more.
#[test]
fn a_host_that_fell_behind_advances_by_at_most_the_step_cap() {
    let cell = full_foliage_cell();
    let mut p = ArtPresenter::new(pack_without(true, true, true));
    p.observe(&bare_view(0));
    p.observe(&rich_view(1000));
    assert_eq!(p.stage_of(cell), Some(2), "the target is never paced");
    let growth = p.growth_of(cell);
    assert_eq!((growth.from, growth.to), (None, Some(0)), "the visual must still be a sprout");
    let want = MAX_STEP_SECONDS / STAGE_GROW_SECONDS;
    assert!(
        (growth.g - want).abs() < 1e-9,
        "a 50 s gap advanced the sprout to {}, not the capped {want}",
        growth.g
    );
    // And an ordinary step is not capped.
    let mut p = ArtPresenter::new(pack_without(true, true, true));
    p.observe(&bare_view(0));
    p.observe(&rich_view(1));
    assert!((p.growth_of(cell).g - DT / STAGE_GROW_SECONDS).abs() < 1e-12);
    assert!(MAX_STEP_SECONDS > 10.0 * DT, "the cap must be worth more than a tick");
}

// ---------------------------------------------------------------------------
// 16. the crown's cap
// ---------------------------------------------------------------------------

/// The tile anchor that lands every texel of a 16×16 tile on exactly one face pixel.
fn tile_anchor() -> SurfacePoint {
    SurfacePoint::new(Face::Front, 32.0, 32.0)
}

fn tile_window() -> Vec<(Face, u8, u8)> {
    (16..48u8).flat_map(|y| (16..48u8).map(move |x| (Face::Front, x, y))).collect()
}

fn stamp_tile(sprite: &Sprite, mask: Mask, background: &Canvas) -> Canvas {
    let mut canvas = background.clone();
    stamp_pose(
        &mut canvas,
        tile_anchor(),
        Vec2::new(1.0, 0.0),
        Pose::still(sprite),
        1.0,
        1.0,
        mask,
        &mut Vec::new(),
    );
    canvas
}

#[test]
fn a_crown_cap_keeps_its_own_art_and_drops_the_trunk_rows_it_merely_repeats() {
    let art = pack();
    let white = {
        let mut canvas = Canvas::new();
        for (f, x, y) in every_pixel() {
            canvas.set(f, x, y, [1.0, 1.0, 1.0]);
        }
        canvas
    };
    let window = tile_window();
    let mut with_crowns = 0;
    for plant in &art.tall {
        let TallPlant { name, crown, cap, tail_row, trunk, .. } = plant;
        let Some(crown) = crown else {
            assert!(cap.is_none(), "{name} has a cap without a crown");
            assert_eq!(*tail_row, 16, "a plant with no crown has no tail");
            continue;
        };
        with_crowns += 1;
        let cap = cap.as_ref().expect("a crown must come with a cap");
        assert!(
            (CROWN_TAIL_MIN_ROW..16).contains(tail_row),
            "{name}'s tail starts at row {tail_row}"
        );
        assert_eq!(cap.frames.len(), crown.frames.len(), "{name}: the cap must run in step");
        assert_eq!(cap.seconds, crown.seconds);

        let tail = TILE_ROWS - *tail_row as f64;
        for (i, (cap_frame, crown_frame)) in cap.frames.iter().zip(&crown.frames).enumerate() {
            assert!(
                cap_frame.extent() <= crown_frame.extent(),
                "{name} frame {i}: the cap's extent {} exceeds the crown's {}",
                cap_frame.extent(),
                crown_frame.extent()
            );
            // Above the tail the cap is the crown, texel for texel.
            let rows_above = Mask::Strip { floor: tail, reveal: TILE_ROWS };
            assert!(
                max_diff_at(
                    &stamp_tile(cap_frame, rows_above, &white),
                    &stamp_tile(crown_frame, rows_above, &white),
                    &window
                ) < 1e-6,
                "{name} frame {i}: the cap lost the crown's own art above row {tail_row}"
            );
            // In the tail the cap paints nothing the trunk paints.
            let tail_rows = Mask::Strip { floor: 0.0, reveal: tail };
            let capped = stamp_tile(cap_frame, tail_rows, &white);
            let trunked = stamp_tile(&trunk.frames[i], tail_rows, &white);
            let mut shared = 0;
            for &(f, x, y) in &window {
                if trunked.get(f, x, y) != white.get(f, x, y) {
                    assert_eq!(
                        capped.get(f, x, y),
                        white.get(f, x, y),
                        "{name} frame {i}: the cap re-paints the trunk at ({x}, {y})"
                    );
                    shared += 1;
                }
            }
            assert!(shared > 4, "{name} frame {i}: the trunk paints nothing in its tail rows");
        }
        // Non-vacuity: the crown *does* repeat the trunk there, which is why the cap exists.
        let tail_rows = Mask::Strip { floor: 0.0, reveal: tail };
        let crowned = stamp_tile(&crown.frames[0], tail_rows, &white);
        let capped = stamp_tile(&cap.frames[0], tail_rows, &white);
        assert!(
            max_diff_at(&crowned, &capped, &window) > 0.01,
            "{name}: the cap is the crown, so nothing was subtracted"
        );
    }
    assert!(with_crowns >= 2, "the pack must carry crowned columns");
    for name in TALL_PLANTS {
        assert!(art.tall_plant(name).is_some(), "the pack is missing {name}");
    }
}

// ---------------------------------------------------------------------------
// 17. review capture
// ---------------------------------------------------------------------------

/// Not a correctness test: the review capture the brief asks for. Run with
/// `cargo test -p cubarium --test art_motion -- --ignored growth_sequence_capture`
/// and look at the PNGs in `$CUBARIUM_CAPTURE_DIR` (default `/tmp/cubarium-growth`).
#[test]
#[ignore = "review capture, not behaviour"]
fn growth_sequence_capture() {
    use cube_proto::Frame;
    use cubarium::net::net_rgb8;
    use cubarium::sink::png::write_net_png;

    let dir = std::env::var("CUBARIUM_CAPTURE_DIR")
        .unwrap_or_else(|_| "/tmp/cubarium-growth".to_string());
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("the capture directory must be writable");

    let foliage = full_foliage_cell();
    let canopy = CellId::all()
        .find(|&c| band_of(c) == Band::Canopy && plant_cap(Band::Canopy, c) == Some(2))
        .expect("a rank-2 canopy slot");
    let fruity = lanternstalk_cell();
    let column = a_column(true);

    let rich = |tick: u64| {
        let mut v = bare_view(tick);
        for cell in [foliage, canopy, fruity] {
            v.producer[cell.index()] = saturation();
        }
        for cell in CellId::all() {
            if cell.face() == column.face && cell.cx() == column.cx && band_of(cell) == Band::Foliage
            {
                v.producer[cell.index()] = saturation();
            }
        }
        v.fruit[fruity.index()] = FRUIT_SHOW * 2.0;
        v
    };

    let mut p = ArtPresenter::new(pack());
    let mut canvas = Canvas::new();
    let mut frame = Frame::black();
    let mut rgb = Vec::new();
    let seconds = 45.0;
    let frames = (seconds * 60.0) as u64;
    let mut written = 0;
    for i in 0..frames {
        let tick = i / FRAMES_PER_TICK + 1;
        let f = (i % FRAMES_PER_TICK) as f64 / FRAMES_PER_TICK as f64;
        let elapsed = i as f64 / 60.0;
        let v = if (5.0..30.0).contains(&elapsed) { rich(tick) } else { bare_view(tick) };
        if i % FRAMES_PER_TICK == 0 {
            p.observe(&v);
        }
        canvas.clear();
        p.draw(&v, f, &mut canvas);
        if i % 6 == 0 {
            canvas.encode(&mut frame);
            rgb.clear();
            net_rgb8(&frame, &mut rgb);
            write_net_png(&dir.join(format!("growth-{i:05}.png")), &rgb).expect("write the png");
            written += 1;
        }
    }
    println!("wrote {written} net PNGs to {}", dir.display());
}
