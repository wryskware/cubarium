//! Independent tests for the live art mode, written from the brief and the public doc
//! comments rather than from the implementation. Everything here goes through the public
//! API only, which is the point of an integration test: a wrong implementation that still
//! type-checks has to survive these.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use cube_proto::{FACE_SIZE, Face};
use cubarium::art::{ArtPack, Clip, STATES};
use cubarium::art_present::{
    ArtPresenter, MOTIF_FULL, MOTIF_THRESHOLD, clip_time, form_of, phase_of, state_of,
};
use cubarium::clock::DT;
use cubarium::present::{JUVENILE_SCALE, PRODUCER_SATURATION, Presenter, interpolate};
use cubarium::sink::web::WebSink;
use cubarium_core::OrganismId;
use cubarium_core::organism::Mode;
use cubarium_core::view::{OrganismView, RenderView};
use cubarium_render::{Canvas, Sprite, stamp_sprite};
use cubarium_surface::{CELL_COUNT, CellId, PixelImage, SurfacePoint, Vec2};

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

/// The baked pack, resolved from the manifest so the test does not depend on the
/// working directory the runner happens to pick.
fn pack_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")
}

fn pack() -> ArtPack {
    ArtPack::load(&pack_dir()).expect("the baked pack at assets/atelier must load")
}

fn presenter() -> ArtPresenter {
    ArtPresenter::new(pack())
}

/// A `P_max` big enough that the interesting densities are not near zero.
const PRODUCER_MAX: f64 = 10.0;

/// The producer value at which a cell's density `t` is exactly `d`.
fn producer_for_density(d: f64) -> f64 {
    d * PRODUCER_MAX * PRODUCER_SATURATION
}

fn view(tick: u64, producer: Vec<f64>, detritus: Vec<f64>, organisms: Vec<OrganismView>) -> RenderView {
    assert_eq!(producer.len(), CELL_COUNT);
    assert_eq!(detritus.len(), CELL_COUNT);
    RenderView { tick, producer, detritus, producer_max: PRODUCER_MAX, organisms }
}

/// A varied producer field whose every cell stays under [`MOTIF_THRESHOLD`], so no motif
/// may be stamped anywhere while the ramp still has real structure in it.
fn quiet_producer() -> Vec<f64> {
    let ceiling = producer_for_density(MOTIF_THRESHOLD);
    (0..CELL_COUNT).map(|i| ceiling * (i % 20) as f64 / 20.0).collect()
}

/// A detritus field with cells above and below the fleck threshold.
fn mixed_detritus() -> Vec<f64> {
    (0..CELL_COUNT).map(|i| (i % 13) as f64 * 0.15).collect()
}

fn flat(value: f64) -> Vec<f64> {
    vec![value; CELL_COUNT]
}

fn organism(id: OrganismId, hue: f32, mode: Mode, pos: SurfacePoint) -> OrganismView {
    OrganismView {
        id,
        pos,
        heading: Vec2::new(1.0, 0.0),
        lobes: vec![(0.0, 0.0, 2.0)],
        hue,
        mode,
        fed: false,
        juvenile: false,
        gestation: None,
        moved: Vec::new(),
    }
}

fn id(slot: u32, generation: u32) -> OrganismId {
    OrganismId { slot, generation }
}

/// The clip the pack holds for `form` in `state`, by the documented species-major layout.
fn clip_of(pack: &ArtPack, form: usize, state: usize) -> &Clip {
    &pack.clips[form * 4 + state]
}

// ---------------------------------------------------------------------------
// canvas helpers
// ---------------------------------------------------------------------------

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL.into_iter().flat_map(|face| {
        (0..FACE_SIZE as u8).flat_map(move |y| (0..FACE_SIZE as u8).map(move |x| (face, x, y)))
    })
}

/// Pixels where the two canvases are not bit-for-bit the same value.
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

/// Total linear light in the canvas, accumulated in `f64` so the comparison is not
/// limited by the order `f32` partial sums happen to fall in. On the current tree the
/// seam and centre totals agree exactly; the 1e-4 tolerance in the seam test is the
/// brief's, kept so the test is not hostage to a different FMA schedule.
fn total_light(c: &Canvas) -> f64 {
    every_pixel()
        .map(|(f, x, y)| c.get(f, x, y).iter().map(|&v| f64::from(v)).sum::<f64>())
        .sum()
}

fn draw_art(p: &mut ArtPresenter, v: &RenderView, f: f64) -> Canvas {
    let mut canvas = Canvas::new();
    p.observe(v);
    p.draw(v, f, &mut canvas);
    canvas
}

// ---------------------------------------------------------------------------
// form_of
// ---------------------------------------------------------------------------

/// The spec formula, evaluated in `f32` exactly as the doc comment writes it. Only valid
/// for hues in `[0, 1]`, where `floor(hue x 3)` cannot go negative.
fn expected_form(hue: f32) -> usize {
    (hue * 3.0).floor().min(2.0) as usize
}

#[test]
fn form_of_follows_min_two_floor_hue_times_three() {
    // A dense sweep of the whole unit interval, expectation computed from the formula.
    for k in 0..=1000u32 {
        let hue = k as f32 / 1000.0;
        assert_eq!(
            form_of(hue),
            expected_form(hue),
            "hue {hue} (hue*3 = {})",
            hue * 3.0
        );
    }
    // The exact tercile boundaries, and their immediate f32 neighbours on both sides:
    // these are where an off-by-an-epsilon implementation shows up.
    for &boundary in &[1.0f32 / 3.0, 2.0f32 / 3.0] {
        let below = f32::from_bits(boundary.to_bits() - 1);
        let above = f32::from_bits(boundary.to_bits() + 1);
        for hue in [below, boundary, above] {
            assert_eq!(form_of(hue), expected_form(hue), "hue {hue:?} near a tercile");
        }
    }
    // Named cases from the brief.
    assert_eq!(form_of(0.0), 0);
    assert_eq!(form_of(0.34), 1);
    assert_eq!(form_of(0.99), 2);
    assert_eq!(form_of(1.0), 2, "hue 1 is the top tercile, not a fourth rig");
    assert_eq!(form_of(f32::NAN), 0, "a NaN hue is form 0");
}

#[test]
fn form_of_never_leaves_the_three_rigs() {
    let hostile = [
        -0.0f32,
        -1e-30,
        -0.5,
        -1.0,
        -12345.678,
        -f32::MAX,
        f32::NEG_INFINITY,
        1.0000001,
        2.0,
        1e30,
        f32::MAX,
        f32::INFINITY,
        f32::MIN_POSITIVE,
        f32::NAN,
    ];
    for hue in hostile {
        let form = form_of(hue);
        assert!(form <= 2, "hue {hue:?} selected rig {form}, but the pack has 3 rigs");
    }
    // And the pack must actually be able to answer for every rig the function can name.
    let pack = pack();
    for form in 0..=2 {
        let _ = pack.creature(form, 0, 0.0);
    }
}

// ---------------------------------------------------------------------------
// state_of
// ---------------------------------------------------------------------------

#[test]
fn state_of_maps_each_mode_to_its_documented_clip() {
    let at = SurfacePoint::new(Face::Front, 32.0, 32.0);
    for (mode, want, name) in [
        (Mode::Resting, 0usize, "rest"),
        (Mode::Seeking, 1, "move"),
        (Mode::Feeding, 2, "feed"),
    ] {
        let o = organism(id(1, 1), 0.5, mode, at);
        assert_eq!(state_of(&o), want, "{mode:?}");
        assert_eq!(STATES[state_of(&o)], name, "{mode:?} must select the {name} clip");
    }
}

#[test]
fn state_of_lets_gestation_beat_every_mode() {
    let at = SurfacePoint::new(Face::Front, 32.0, 32.0);
    for mode in [Mode::Resting, Mode::Seeking, Mode::Feeding] {
        for progress in [0.0f32, 0.25, 0.5, 0.999, 1.0] {
            let mut o = organism(id(4, 2), 0.1, mode, at);
            o.gestation = Some(progress);
            assert_eq!(
                state_of(&o),
                3,
                "{mode:?} with gestation {progress} must still be budding"
            );
            assert_eq!(STATES[state_of(&o)], "bud");
        }
    }
}

// ---------------------------------------------------------------------------
// phase_of
// ---------------------------------------------------------------------------

#[test]
fn phase_of_is_deterministic_for_the_same_id() {
    for slot in 0..200u32 {
        let a = phase_of(id(slot, 7), 4.0);
        let b = phase_of(id(slot, 7), 4.0);
        assert_eq!(a, b, "slot {slot} phase must not move between calls");
    }
}

#[test]
fn phase_of_stays_inside_the_clip() {
    for seconds in [0.001f64, 1.6, 2.0, 4.0, 5.0, 1.0e9] {
        for slot in 0..400u32 {
            for generation in [0u32, 1, 9999] {
                let p = phase_of(id(slot, generation), seconds);
                assert!(
                    p.is_finite() && (0.0..seconds).contains(&p),
                    "phase {p} for {slot}/{generation} is outside [0, {seconds})"
                );
            }
        }
    }
}

#[test]
fn phase_of_separates_ids_that_differ_only_in_generation() {
    for slot in 0..256u32 {
        let a = phase_of(id(slot, 0), 4.0);
        let b = phase_of(id(slot, 1), 4.0);
        assert_ne!(a, b, "slot {slot}: generation must change the phase");
    }
}

#[test]
fn phase_of_separates_ids_that_differ_only_in_slot() {
    for generation in 0..64u32 {
        let a = phase_of(id(0, generation), 4.0);
        let b = phase_of(id(1, generation), 4.0);
        assert_ne!(a, b, "generation {generation}: slot must change the phase");
    }
}

#[test]
fn phase_of_spreads_a_population_over_the_whole_clip() {
    // Not a distribution test so much as a check that the hash is a hash: 512 bodies
    // must not land in a handful of buckets, or the cube animates in lockstep anyway.
    let seconds = 4.0;
    let mut buckets = [0usize; 8];
    for slot in 0..512u32 {
        let p = phase_of(id(slot, 3), seconds);
        buckets[((p / seconds) * 8.0) as usize % 8] += 1;
    }
    for (i, &n) in buckets.iter().enumerate() {
        assert!(n > 0, "no organism phased into eighth {i} of the clip: {buckets:?}");
    }
}

#[test]
fn phase_of_yields_zero_for_a_degenerate_clip_length() {
    for seconds in [0.0f64, -0.0, -1.0, -4.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for slot in [0u32, 1, 7, 12345] {
            assert_eq!(
                phase_of(id(slot, 2), seconds),
                0.0,
                "seconds {seconds} must yield phase 0 (slot {slot})"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// clip_time
// ---------------------------------------------------------------------------

#[test]
fn clip_time_drives_looping_clips_on_simulated_time() {
    let pack = pack();
    for form in 0..3 {
        for state in 0..3 {
            let clip = clip_of(&pack, form, state);
            assert!(clip.looping, "rest/move/feed must loop ({form}/{state})");
            let phase = phase_of(id(form as u32, state as u32), clip.seconds);
            for tick in [0u64, 1, 17, 400, 100_000] {
                assert_eq!(
                    clip_time(clip, tick, phase, None),
                    tick as f64 * DT + phase,
                    "form {form} state {state} tick {tick}"
                );
            }
        }
    }
}

#[test]
fn clip_time_ignores_wall_clock_delay() {
    let pack = pack();
    let clip = clip_of(&pack, 1, 0);
    let phase = phase_of(id(9, 4), clip.seconds);
    let first = clip_time(clip, 1234, phase, None);
    std::thread::sleep(Duration::from_millis(8));
    let second = clip_time(clip, 1234, phase, None);
    std::thread::sleep(Duration::from_millis(8));
    let third = clip_time(clip, 1234, phase, None);
    assert_eq!(first, second, "the same tick must sample the same instant of the clip");
    assert_eq!(second, third);
}

#[test]
fn clip_time_drives_the_bud_clip_on_gestation_progress() {
    let pack = pack();
    for form in 0..3 {
        let bud = clip_of(&pack, form, 3);
        assert!(!bud.looping, "the bud clip must not loop");
        let phase = phase_of(id(form as u32, 0), bud.seconds);
        for progress in [0.0f32, 0.5, 1.0] {
            for tick in [0u64, 7, 9_999] {
                assert_eq!(
                    clip_time(bud, tick, phase, Some(progress)),
                    f64::from(progress) * bud.seconds,
                    "form {form} progress {progress} tick {tick}: the bud clip is driven \
                     by gestation, not by the tick or the phase"
                );
            }
        }
    }
}

#[test]
fn clip_time_yields_zero_for_a_non_looping_clip_without_gestation() {
    let pack = pack();
    let bud = clip_of(&pack, 0, 3);
    assert_eq!(clip_time(bud, 500, 1.25, None), 0.0);
}

#[test]
fn clip_time_keeps_looping_clips_on_simulated_time_even_while_gestating() {
    // The doc comment is unconditional about this: "Every looping clip is driven by
    // simulated time". Only the non-looping bud clip reads the gestation progress.
    let pack = pack();
    let clip = clip_of(&pack, 2, 1);
    let phase = phase_of(id(3, 3), clip.seconds);
    assert_eq!(clip_time(clip, 40, phase, Some(0.5)), 40.0 * DT + phase);
}

// ---------------------------------------------------------------------------
// simulated-time honesty in draw
// ---------------------------------------------------------------------------

#[test]
fn draw_is_a_pure_function_of_the_view() {
    let mut p = presenter();
    let v = view(
        321,
        quiet_producer(),
        mixed_detritus(),
        vec![
            organism(id(2, 1), 0.1, Mode::Resting, SurfacePoint::new(Face::Front, 20.0, 20.0)),
            organism(id(5, 2), 0.5, Mode::Seeking, SurfacePoint::new(Face::Top, 32.0, 32.0)),
            organism(id(8, 3), 0.9, Mode::Feeding, SurfacePoint::new(Face::Left, 40.0, 12.0)),
        ],
    );
    let first = draw_art(&mut p, &v, 0.25);
    std::thread::sleep(Duration::from_millis(12));
    let second = draw_art(&mut p, &v, 0.25);
    assert_same_canvas(&first, &second, "two draws of the same view separated by wall time");
}

/// Draw one body of `hue` in `mode` at `tick`, on a bare floor.
fn pose_at(p: &mut ArtPresenter, body: &OrganismView, tick: u64) -> Canvas {
    draw_art(p, &view(tick, flat(0.0), flat(0.0), vec![body.clone()]), 0.0)
}

/// A slot whose clip phase sits comfortably inside a frame, so a period test measures the
/// period rather than f64 rounding at a frame boundary.
fn slot_phased_mid_frame(clip: &Clip, generation: u32) -> u32 {
    let frame_len = clip.seconds / clip.frames.len() as f64;
    (0..10_000u32)
        .find(|&slot| {
            let into_frame = phase_of(id(slot, generation), clip.seconds) % frame_len;
            into_frame > frame_len * 0.2 && into_frame < frame_len * 0.8
        })
        .expect("some slot phases into the middle of a frame")
}

fn whole_ticks(clip: &Clip) -> u64 {
    let ticks = (clip.seconds / DT).round() as u64;
    assert!(
        (ticks as f64 * DT - clip.seconds).abs() < 1e-12,
        "this test needs a clip whose period is a whole number of ticks; {} s is {} ticks",
        clip.seconds,
        clip.seconds / DT
    );
    ticks
}

#[test]
fn a_whole_clip_period_later_a_resting_body_holds_the_same_pose() {
    // Rig 1 (`sail`) is used because its baked rest clip is the only one of the three
    // with more than one distinct frame: on rigs 0 and 2 the assertion would hold for a
    // presenter that ignored time entirely.
    let hue = 0.5f32;
    let form = form_of(hue);
    assert_eq!(form, 1);
    let pack_ref = pack();
    let rest = clip_of(&pack_ref, form, 0);
    let period_ticks = whole_ticks(rest);
    let slot = slot_phased_mid_frame(rest, 1);
    let body = organism(id(slot, 1), hue, Mode::Resting, SurfacePoint::new(Face::Front, 30.0, 30.0));

    let mut p = presenter();
    let now = pose_at(&mut p, &body, 100);
    let later = pose_at(&mut p, &body, 100 + period_ticks);
    assert_same_canvas(&now, &later, "one whole clip period later");

    // Non-vacuity: somewhere strictly inside the period the pose must actually change.
    let moved = (1..period_ticks)
        .any(|offset| !differing(&now, &pose_at(&mut p, &body, 100 + offset)).is_empty());
    assert!(moved, "the rest clip never changed pose anywhere inside its period");
}

#[test]
fn a_feeding_body_repeats_on_its_own_period_and_not_before() {
    // Rig 0's feed clip is the one baked clip whose frames differ half a period apart, so
    // this is the sharpest available test that the period is the clip's and not something
    // shorter or longer.
    let hue = 0.1f32;
    let form = form_of(hue);
    assert_eq!(form, 0);
    let pack_ref = pack();
    let feed = clip_of(&pack_ref, form, 2);
    let period_ticks = whole_ticks(feed);
    let slot = slot_phased_mid_frame(feed, 6);
    let body = organism(id(slot, 6), hue, Mode::Feeding, SurfacePoint::new(Face::Left, 30.0, 30.0));

    let mut p = presenter();
    let now = pose_at(&mut p, &body, 0);
    assert_same_canvas(
        &now,
        &pose_at(&mut p, &body, period_ticks),
        "one whole feed period later",
    );
    assert_same_canvas(
        &now,
        &pose_at(&mut p, &body, 5 * period_ticks),
        "five whole feed periods later",
    );
    assert!(
        !differing(&now, &pose_at(&mut p, &body, period_ticks / 2)).is_empty(),
        "half a feed period later the pose must have moved"
    );
}

// ---------------------------------------------------------------------------
// rendering equivalence with the decided M2 image
// ---------------------------------------------------------------------------

#[test]
fn an_empty_quiet_world_draws_exactly_the_m2_image() {
    let v = view(77, quiet_producer(), mixed_detritus(), Vec::new());
    let ceiling = producer_for_density(MOTIF_THRESHOLD);
    assert!(
        v.producer.iter().all(|&p| p < ceiling),
        "the fixture must keep every cell under the motif threshold"
    );

    let mut art = presenter();
    let art_canvas = draw_art(&mut art, &v, 0.5);

    let mut old = Presenter::new();
    let mut old_canvas = Canvas::new();
    old.observe(&v);
    old.draw(&v, 0.5, &mut old_canvas);

    assert_same_canvas(
        &art_canvas,
        &old_canvas,
        "with no organisms and no eligible cell, the art image must be the M2 image",
    );
    assert!(total_light(&art_canvas) > 0.0, "the fixture must actually draw something");
}

#[test]
fn a_rich_cell_adds_a_motif_and_nothing_else() {
    let cell = CellId::new(Face::Front, 8, 8);
    let center = cell.center();
    let mut producer = quiet_producer();
    producer[cell.index()] = producer_for_density((MOTIF_FULL + 1.0) / 2.0);
    let v = view(9, producer, mixed_detritus(), Vec::new());

    let mut art = presenter();
    let art_canvas = draw_art(&mut art, &v, 0.0);

    let mut old = Presenter::new();
    let mut old_canvas = Canvas::new();
    old.observe(&v);
    old.draw(&v, 0.0, &mut old_canvas);

    let diff = differing(&art_canvas, &old_canvas);
    assert!(!diff.is_empty(), "a cell above MOTIF_FULL must grow a visible motif");

    // The motif is one sprite stamped at the cell centre with at most +-1 px of jitter
    // per axis, so everything it touches stays local to the cell. (Measured worst case on
    // the current pack: 7.38 px.)
    let mut worst = 0.0f64;
    for &(face, x, y) in &diff {
        assert_eq!(face, Face::Front, "the motif escaped its face at ({x}, {y})");
        let dx = f64::from(x) + 0.5 - center.u;
        let dy = f64::from(y) + 0.5 - center.v;
        worst = worst.max(dx.hypot(dy));
    }
    assert!(
        worst <= 10.0,
        "a pixel {worst:.2} px from the cell centre changed; the motif is not local"
    );
}

// ---------------------------------------------------------------------------
// gestation and scale
// ---------------------------------------------------------------------------

#[test]
fn a_full_gestation_draws_the_bud_clips_last_frame() {
    let hue = 0.9f32;
    let form = form_of(hue);
    assert_eq!(form, 2);
    let at = SurfacePoint::new(Face::Back, 28.0, 36.0);
    let heading = Vec2::new(1.0, 0.0);
    let mut body = organism(id(11, 5), hue, Mode::Resting, at);
    body.heading = heading;
    body.gestation = Some(1.0);

    let producer = quiet_producer();
    let detritus = mixed_detritus();
    let empty = view(640, producer.clone(), detritus.clone(), Vec::new());
    let peopled = view(640, producer, detritus, vec![body.clone()]);

    let mut p = presenter();
    let mut expected = draw_art(&mut p, &empty, 0.0);
    let actual = draw_art(&mut p, &peopled, 0.0);

    let bud_seconds = clip_of(p.pack(), form, 3).seconds;
    let phase = phase_of(body.id, bud_seconds);
    let t = clip_time(clip_of(p.pack(), form, 3), peopled.tick, phase, body.gestation);
    assert_eq!(t, bud_seconds, "progress 1 must land exactly on the end of the bud clip");

    let (anchor, facing) = interpolate(&body.moved, body.pos, body.heading, 0.0);
    let sprite: &Sprite = p.pack().creature(form, 3, t);
    let mut scratch: Vec<PixelImage> = Vec::new();
    stamp_sprite(&mut expected, anchor, facing, sprite, 1.0, 1.0, &mut scratch);

    assert_same_canvas(
        &actual,
        &expected,
        "a gestating body must be the bud clip stamped at the interpolated anchor",
    );
    assert!(
        !differing(&actual, &draw_art(&mut p, &empty, 0.0)).is_empty(),
        "the body must have drawn something at all"
    );
}

#[test]
fn a_juvenile_draws_a_smaller_footprint_than_an_adult() {
    let hue = 0.5f32;
    let at = SurfacePoint::new(Face::Top, 32.0, 32.0);
    let adult = organism(id(21, 1), hue, Mode::Seeking, at);
    let mut juvenile = adult.clone();
    juvenile.juvenile = true;
    const { assert!(JUVENILE_SCALE < 1.0, "the juvenile scale must actually shrink the body") };

    let empty = view(55, flat(0.0), flat(0.0), Vec::new());
    let mut p = presenter();
    let background = draw_art(&mut p, &empty, 0.0);

    let adult_canvas = draw_art(&mut p, &view(55, flat(0.0), flat(0.0), vec![adult]), 0.0);
    let juvenile_canvas =
        draw_art(&mut p, &view(55, flat(0.0), flat(0.0), vec![juvenile]), 0.0);

    let adult_px = differing(&adult_canvas, &background).len();
    let juvenile_px = differing(&juvenile_canvas, &background).len();
    assert!(adult_px > 0, "the adult drew nothing");
    assert!(juvenile_px > 0, "the juvenile drew nothing");
    assert!(
        juvenile_px < adult_px,
        "a juvenile lit {juvenile_px} px and the adult {adult_px}: the scale is not applied"
    );
}

// ---------------------------------------------------------------------------
// seams
// ---------------------------------------------------------------------------

#[test]
fn a_body_on_a_seam_spans_both_faces_and_conserves_light() {
    let hue = 0.1f32;
    let seam_body = organism(
        id(31, 2),
        hue,
        Mode::Resting,
        SurfacePoint::new(Face::Front, 63.0, 32.0),
    );
    let center_body = OrganismView {
        pos: SurfacePoint::new(Face::Front, 32.0, 32.0),
        ..seam_body.clone()
    };

    let empty = view(200, flat(0.0), flat(0.0), Vec::new());
    let mut p = presenter();
    let background = draw_art(&mut p, &empty, 0.0);
    let base_light = total_light(&background);

    let on_seam = draw_art(&mut p, &view(200, flat(0.0), flat(0.0), vec![seam_body]), 0.0);
    let at_center =
        draw_art(&mut p, &view(200, flat(0.0), flat(0.0), vec![center_body]), 0.0);

    let touched = differing(&on_seam, &background);
    let on_front = touched.iter().filter(|&&(f, _, _)| f == Face::Front).count();
    let on_right = touched.iter().filter(|&&(f, _, _)| f == Face::Right).count();
    assert!(on_front > 0, "the seam body lit nothing on Front");
    assert!(on_right > 0, "the seam body lit nothing on Right: it did not cross the seam");
    assert_eq!(
        on_front + on_right,
        touched.len(),
        "the seam body lit a face other than Front and Right: {touched:?}"
    );

    let added_seam = total_light(&on_seam) - base_light;
    let added_center = total_light(&at_center) - base_light;
    assert!(added_center > 0.0, "the reference body at the face centre lit nothing");
    assert!(
        (added_seam - added_center).abs() <= 1e-4,
        "light is not conserved across the seam: {added_seam} on the seam vs \
         {added_center} at the centre (difference {})",
        added_seam - added_center
    );
}

// ---------------------------------------------------------------------------
// web sink /note
// ---------------------------------------------------------------------------

/// One blocking HTTP/1.1 request over a real socket, returning the status line, the
/// lowercased head and the body. The sink closes the connection itself.
fn http_get(addr: SocketAddr, path: &str) -> (String, String, Vec<u8>) {
    let mut stream = TcpStream::connect(addr).expect("connect to the sink");
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
    write!(stream, "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .expect("write the request");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read the response");
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("a response with a complete head");
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let status = head.lines().next().unwrap_or_default().to_string();
    (status, head.to_lowercase(), raw[split + 4..].to_vec())
}

#[test]
fn web_sink_serves_the_hud_note_as_plain_text() {
    let note = "8× time";
    let sink = WebSink::with_note(0, note).expect("bind an ephemeral loopback port");
    assert_eq!(sink.note(), note);

    let (status, head, body) = http_get(sink.addr(), "/note");
    assert!(status.starts_with("HTTP/1.1 200"), "status was {status:?}");
    assert!(
        head.contains("content-type: text/plain; charset=utf-8"),
        "the note must be plain UTF-8 text; head was:\n{head}"
    );
    assert_eq!(
        String::from_utf8(body).expect("a UTF-8 body"),
        note,
        "the served note must be exactly what the host passed in"
    );
}

#[test]
fn web_sink_without_a_note_serves_an_empty_body() {
    let sink = WebSink::new(0).expect("bind an ephemeral loopback port");
    assert_eq!(sink.note(), "");

    let (status, _head, body) = http_get(sink.addr(), "/note");
    assert!(status.starts_with("HTTP/1.1 200"), "status was {status:?}");
    assert!(body.is_empty(), "expected no note, got {:?}", String::from_utf8_lossy(&body));
}
