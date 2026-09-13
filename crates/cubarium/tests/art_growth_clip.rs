//! Independent tests for the **authored growth clip** of the art image (pack v5): the
//! lanternstalk's `grow01`, the three-layer stamp that plays it, the blends at its ends, and
//! the reveal-mask fallback every other step keeps.
//!
//! Written from the public doc comments of `cubarium::art` (`Clip::sample`, `Plant::transition`,
//! `Transition`), `cubarium::art_present` (`GrowthStep`, `growth_step`, `GROW_BLEND`,
//! `growth_weights`, `slot_wind`, and the "A stage step in flight" section of
//! `ArtPresenter::draw_with_fruit`), `cubarium_render::sprite` (`stamp_layers_bent`, `Bend`,
//! `Mask`), `art/README.md` "Live world" and `art/PLANTS.md` "Pack v5: growth transitions" —
//! never from their bodies.
//!
//! Where an expected image is needed it is rebuilt here a second, independent way: a presenter
//! drawn from a pack with its plants removed supplies the background, and the step's stamp is
//! hand-built on top through the public API (`slot_of`, `slot_wind`, `growth_weights`,
//! `Clip::sample`, `plant_phase_of`, `stage_opacity`, `stamp_layers_bent`). Where no such image
//! exists the assertion is a property a wrong implementation would break: a clip that restarts
//! on a draw, a step that cuts at its ends, a reversal that replays the wrong progress, a
//! fallback that stopped being the old picture, a root that skates.

use std::path::{Path, PathBuf};
use std::ptr;

use cube_proto::{FACE_SIZE, Face};
use cubarium::art::{ArtPack, Band, Plant};
use cubarium::art_present::{
    ArtPresenter, GROW_BLEND, Growth, GrowthStep, PLANT_BEND_LENGTH, PLANT_BEND_ROOT,
    PLANT_REVEAL_PX, STAGE_GROW_SECONDS, band_of, band_opacity, growth_between, growth_step,
    growth_weights, plant_bend_budget, plant_cap, plant_phase_of, present_seconds, slot_of,
    slot_wind, species_of, stage_opacity, stage_thresholds, wind_strength,
};
use cubarium::clock::DT;
use cubarium::present::PRODUCER_SATURATION;
use cubarium_core::view::RenderView;
use cubarium_render::{Bend, Canvas, Mask, Pose, stamp_layers_bent, stamp_pose};
use cubarium_surface::{CELL_COUNT, CellId, SurfacePoint, Vec2};

// ---------------------------------------------------------------------------
// fixtures (the patterns of `art_motion.rs`, copied so this file stands alone)
// ---------------------------------------------------------------------------

const PRODUCER_MAX: f64 = 10.0;
/// Render frames per simulated tick at 60 fps with a 20 Hz clock.
const FRAMES_PER_TICK: u64 = 3;
/// The species that carries the pilot clip.
const PILOT: &str = "lanternstalk";
/// A foliage density that warrants stage 1 and nothing more, and that is past
/// `FOLIAGE_STAGES[1]` so [`stage_opacity`] of stage 0 and of stage 1 are *both* the band's
/// ceiling: the step's opacity is then constant and every image comparison below is about the
/// art, not about a ramp.
const DENSITY: f64 = 0.60;
/// A density that warrants stage 2, for the `1 → 2` step (authored since 2026-09-13, so the
/// fallback sweep below treats it like the pilot's own step).
const FULL_DENSITY: f64 = 0.85;

fn atelier() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")
}

fn pack() -> ArtPack {
    ArtPack::load(&atelier()).expect("the baked pack at assets/atelier must load")
}

/// The shipped pack without its tall plants: one rich cell would otherwise also raise the
/// column over it, and a column is not what these tests are about.
fn plants_only() -> ArtPack {
    let mut art = pack();
    art.tall.clear();
    art
}

/// The same pack with no plants at all: the background a hand-built stamp goes on top of.
fn no_plants() -> ArtPack {
    let mut art = plants_only();
    art.plants.clear();
    art
}

/// The pack a v1–v4 host would have loaded: every authored growth clip removed, everything
/// else untouched.
fn without_transitions() -> ArtPack {
    let mut art = plants_only();
    for plant in &mut art.plants {
        plant.transitions.clear();
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

/// A world rich in exactly one cell, so one plant is the only thing that moves.
fn one_cell_view(tick: u64, cell: CellId, density: f64) -> RenderView {
    let mut v = bare_view(tick);
    v.producer[cell.index()] = saturation() * density;
    v
}

/// The cell this file drives: a lanternstalk slot allowed to reach stage 2, in the middle of
/// Front so its tile stays on one face and its plant is a stalk, not a radial crown.
fn pilot_cell() -> CellId {
    CellId::all()
        .find(|&c| {
            c.face() == Face::Front
                && band_of(c) == Band::Foliage
                && plant_cap(Band::Foliage, c) == Some(2)
                && species_of(Band::Foliage, c) == PILOT
                && (4..=7).contains(&c.cy())
                && (4..=11).contains(&c.cx())
        })
        .expect("the cube has a rank-2 lanternstalk slot in the middle of Front")
}

// ---------------------------------------------------------------------------
// canvas helpers
// ---------------------------------------------------------------------------

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL.into_iter().flat_map(|face| {
        (0..FACE_SIZE as u8).flat_map(move |y| (0..FACE_SIZE as u8).map(move |x| (face, x, y)))
    })
}

/// The pixels a 16-px tile anchored in `cell` can reach.
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

/// The image with no plant in it at all, at the same instant: the background a hand-built
/// stamp goes on. It is a pure function of (view, `f`) — nothing in the floor, the ramp, the
/// flecks, the soil or the ground cover reads a plant's growth.
fn background(v: &RenderView, f: f64) -> Canvas {
    let mut bare = ArtPresenter::new(no_plants());
    bare.observe(v);
    draw(&mut bare, v, f)
}

// ---------------------------------------------------------------------------
// growth helpers
// ---------------------------------------------------------------------------

/// The **idle sway pose** of one stage, exactly as the docs define it: that stage's own
/// looping clip sampled at `seconds + plant_phase_of(cell, clip.seconds)`.
fn idle_pose<'a>(plant: &'a Plant, stage: u8, cell: CellId, seconds: f64) -> Pose<'a> {
    let clip = &plant.stages[usize::from(stage)];
    clip.sample(seconds + plant_phase_of(cell, clip.seconds))
}

/// Observe bare ground, then `observes` ticks `gap` apart with `cell` fed at `density`, and
/// return the view the presenter has just seen.
///
/// A `gap` of 20 ticks or more is one whole [`cubarium::art_present::MAX_STEP_SECONDS`] of
/// simulated time per observe, so the growth advances by exactly `1 / STAGE_GROW_SECONDS` of a
/// step each time (an exact binary fraction) while presentation time runs `gap · DT` — which
/// is how a mid-step frame can be placed at a *calm* instant as well as a windy one.
fn drive(
    p: &mut ArtPresenter,
    cell: CellId,
    density: f64,
    gap: u64,
    observes: u64,
) -> RenderView {
    p.observe(&bare_view(0));
    let mut last = bare_view(0);
    for k in 1..=observes {
        last = one_cell_view(k * gap, cell, density);
        p.observe(&last);
    }
    last
}

/// What the shared breeze does to the pilot cell's slot at a presentation instant: the one
/// `(Bend, heading)` every stamp of that slot takes that frame.
fn wind_of(cell: CellId, budget: f64, seconds: f64) -> (Bend, Vec2) {
    slot_wind(&slot_of(cell), PILOT, budget, seconds)
}

/// The step the frame at `f` draws, `None` when the frame is idle.
fn drawn_step(p: &ArtPresenter, cell: CellId, f: f64) -> Option<GrowthStep> {
    growth_step(growth_between(p.growth_prev_of(cell), p.growth_of(cell), f))
}

/// The documented image of one in-flight step of the pilot cell, hand-built on `background`.
///
/// The two branches are the two the doc comment names: an authored clip is **one** stamp of
/// three layers at [`growth_weights`] with [`Mask::None`] and a linearly crossing opacity;
/// every other pair keeps the reveal masks — the lower stage whole at `opacity · (1 − t)` and
/// the upper stage through `Mask::Axial { reveal: t · PLANT_REVEAL_PX }`.
fn expected_step(
    art: &ArtPack,
    background: &Canvas,
    cell: CellId,
    density: f64,
    seconds: f64,
    wind: (Bend, Vec2),
    step: GrowthStep,
) -> Canvas {
    let plant = art.plant(PILOT).expect(PILOT);
    let slot = slot_of(cell);
    let (bend, heading) = wind;
    let thresholds = stage_thresholds(Band::Foliage);
    let ceiling = band_opacity(Band::Foliage);
    let opacity_of = |stage: u8| stage_opacity(stage, density, &thresholds, ceiling);
    let mut canvas = background.clone();
    let scratch = &mut Vec::new();
    let clip = step.lower.and_then(|low| plant.transition(low, step.upper).map(|c| (low, c)));
    match clip {
        Some((low, clip)) => {
            let under = opacity_of(low);
            let opacity = under + (opacity_of(step.upper) - under) * step.t as f32;
            let [w_from, w_grow, w_to] = growth_weights(step.t);
            let layers = [
                (idle_pose(plant, low, cell, seconds), w_from),
                (clip.sample(step.t * clip.seconds), w_grow),
                (idle_pose(plant, step.upper, cell, seconds), w_to),
            ];
            stamp_layers_bent(
                &mut canvas,
                slot.at,
                heading,
                &layers,
                1.0,
                opacity,
                Mask::None,
                bend,
                scratch,
            );
        }
        None => {
            if let Some(low) = step.lower {
                let layers = [(idle_pose(plant, low, cell, seconds), 1.0)];
                stamp_layers_bent(
                    &mut canvas,
                    slot.at,
                    heading,
                    &layers,
                    1.0,
                    opacity_of(low) * (1.0 - step.t) as f32,
                    Mask::None,
                    bend,
                    scratch,
                );
            }
            let layers = [(idle_pose(plant, step.upper, cell, seconds), 1.0)];
            stamp_layers_bent(
                &mut canvas,
                slot.at,
                heading,
                &layers,
                1.0,
                opacity_of(step.upper),
                Mask::Axial { reveal: step.t * PLANT_REVEAL_PX },
                bend,
                scratch,
            );
        }
    }
    canvas
}

/// The idle image of one whole stage of the pilot cell, hand-built the same way: the stage's
/// own sway pose, whole, with no mask.
fn expected_idle(
    art: &ArtPack,
    background: &Canvas,
    cell: CellId,
    density: f64,
    seconds: f64,
    wind: (Bend, Vec2),
    stage: u8,
) -> Canvas {
    let plant = art.plant(PILOT).expect(PILOT);
    let slot = slot_of(cell);
    let (bend, heading) = wind;
    let thresholds = stage_thresholds(Band::Foliage);
    let opacity = stage_opacity(stage, density, &thresholds, band_opacity(Band::Foliage));
    let mut canvas = background.clone();
    stamp_layers_bent(
        &mut canvas,
        slot.at,
        heading,
        &[(idle_pose(plant, stage, cell, seconds), 1.0)],
        1.0,
        opacity,
        Mask::None,
        bend,
        &mut Vec::new(),
    );
    canvas
}

// ---------------------------------------------------------------------------
// 1. the shipped pack
// ---------------------------------------------------------------------------

/// The pilot is exactly one clip on exactly one plant, it does not loop, and it is sampled
/// *inclusively*: `sample(0)` is its first frame and `sample(seconds)` its last, so the
/// endpoints are the two stages' own neutral poses and nothing wraps the end back into the
/// beginning. A looping transition would jump the plant back to a sprout at the top of every
/// step.
#[test]
fn the_shipped_pack_carries_the_lanternstalks_growth_transitions_and_the_pilot_is_0_to_1() {
    let art = pack();
    let plant = art.plant(PILOT).expect("the pack must carry the pilot plant");
    assert_eq!(
        plant.transitions.iter().map(|t| (t.from, t.to)).collect::<Vec<_>>(),
        vec![(0u8, 1u8), (1, 2)],
        "the pilot's 0 → 1 clip and the 1 → 2 clip authored on 2026-09-13"
    );
    let clip = plant.transition(0, 1).expect("pack v5 carries the growth pilot");
    assert!(!clip.looping, "a growth transition never loops");
    assert!(!plant.transitions[0].clip.looping, "`Transition::clip` is always non-looping");
    assert!(plant.transition(1, 2).is_some(), "1 → 2 is authored too");
    assert!(plant.transition(0, 2).is_none(), "a transition is one stage step");
    assert!(plant.transition(1, 0).is_none(), "a transition only runs upward");

    // Inclusive endpoints: the first and last baked samples, held, with no blend.
    let first = clip.sample(0.0);
    assert!(ptr::eq(first.first, &clip.frames[0]), "sample(0) is not the first frame");
    assert_eq!(first.mix, 0.0, "sample(0) must not blend");
    let last = clip.sample(clip.seconds);
    assert!(
        ptr::eq(last.first, clip.frames.last().unwrap()),
        "sample(seconds) is not the last frame"
    );
    assert!(ptr::eq(last.second, clip.frames.last().unwrap()), "the end wrapped into the start");
    assert_eq!(last.mix, 0.0, "sample(seconds) must hold the last frame");
    // Past the end it stays there, and before the start it stays at the first.
    for seconds in [clip.seconds, clip.seconds * 1.5, clip.seconds + 1.0] {
        assert!(ptr::eq(clip.sample(seconds).first, clip.frames.last().unwrap()), "{seconds} s");
    }
    for seconds in [-1.0, 0.0, f64::NAN] {
        assert!(ptr::eq(clip.sample(seconds).first, &clip.frames[0]), "{seconds} s");
    }
    assert!(clip.frames.len() >= 2, "a clip needs two samples to blend");
    assert!(clip.seconds > 0.0);

    // Every side species carries both steps (2026-09-13); the two radial canopy species carry
    // none, so their steps keep the reveal mask until a top-down opening is authored.
    for plant in &art.plants {
        let pairs = plant.transitions.iter().map(|t| (t.from, t.to)).collect::<Vec<_>>();
        if plant.band == Band::Canopy {
            assert!(pairs.is_empty(), "{} carries {pairs:?}; canopy keeps the masks", plant.name);
            for (from, to) in [(0u8, 1u8), (1, 2)] {
                assert!(plant.transition(from, to).is_none(), "{} {from} → {to}", plant.name);
            }
        } else {
            assert_eq!(pairs, vec![(0u8, 1u8), (1, 2)], "{} growth clips", plant.name);
        }
    }
}

// ---------------------------------------------------------------------------
// 2. the presenter's two paths
// ---------------------------------------------------------------------------

/// A cell fed from bare ground first *reveals* its sprout — the `None → 0` step has no lower
/// stage to blend from, so it keeps the mask — and then plays the authored clip for `0 → 1`,
/// as one stamp of the three documented layers at `growth_weights(t)`, at the documented
/// opacity, under the very bend and heading `slot_wind` gives every other stamp of that slot.
///
/// Both instants are checked because the bend is the one thing a growth stamp shares with the
/// rest of the frame: at the windy instant the slot's bend is a real displacement, at the calm
/// one it is exactly the identity, and the hand-built image must match bit for bit either way.
#[test]
fn a_cell_fed_from_bare_ground_reveals_its_sprout_and_then_plays_the_authored_clip() {
    let cell = pilot_cell();
    let art = plants_only();
    let budget = plant_bend_budget(art.plant(PILOT).unwrap());

    for (what, gap, observes, windy) in
        [("a windy instant", 20u64, 6u64, true), ("a calm instant", 100, 5, false)]
    {
        let mut p = ArtPresenter::new(plants_only());
        assert_eq!(
            p.bend_budget(PILOT),
            budget,
            "the presenter's measured budget must be `plant_bend_budget` of the same plant"
        );
        let v = drive(&mut p, cell, DENSITY, gap, observes);
        let f = 1.0;
        let seconds = present_seconds(v.tick, f);
        let step = drawn_step(&p, cell, f).expect("the fixture must be in flight");
        assert_eq!((step.lower, step.upper), (Some(0), 1), "{what}: not the 0 → 1 step");
        assert!(
            step.t > GROW_BLEND && step.t < 1.0 - GROW_BLEND,
            "{what}: t = {} is inside an edge blend, not the clip alone",
            step.t
        );
        // The fixture really is windy / really is calm.
        let (bend, _) = wind_of(cell, budget, seconds);
        assert_eq!(
            !bend.is_identity(),
            windy,
            "{what}: the slot's bend is {bend:?} at {seconds} s (wind {})",
            wind_strength(seconds)
        );
        if windy {
            assert!(bend.amplitude.abs() > 1e-6, "{what}: the breeze must displace something");
        } else {
            assert_eq!(wind_strength(seconds), 0.0, "{what}: the packet must be quiet");
        }

        let actual = draw(&mut p, &v, f);
        let expected = expected_step(
            &art,
            &background(&v, f),
            cell,
            DENSITY,
            seconds,
            wind_of(cell, budget, seconds),
            step,
        );
        assert_same_canvas(&actual, &expected, &format!("the 0 → 1 clip step at {what}"));
        // The fixture is not comparing two empty images.
        assert!(
            max_diff_at(&actual, &background(&v, f), &near(cell)) > 0.05,
            "{what}: the growth stamp paints nothing"
        );
    }

    // The first appearance, `None → 0`, has no lower stage and therefore no clip: it is the
    // reveal mask, the same picture a v4 pack drew.
    let mut p = ArtPresenter::new(plants_only());
    let v = drive(&mut p, cell, DENSITY, 20, 2);
    let f = 0.5;
    let step = drawn_step(&p, cell, f).expect("in flight out of bare ground");
    assert_eq!((step.lower, step.upper), (None, 0), "the first step must come out of bare ground");
    let seconds = present_seconds(v.tick, f);
    let actual = draw(&mut p, &v, f);
    let expected = expected_step(
        &art,
        &background(&v, f),
        cell,
        DENSITY,
        seconds,
        wind_of(cell, budget, seconds),
        step,
    );
    assert_same_canvas(&actual, &expected, "the None → 0 reveal");
    assert!(
        max_diff_at(&actual, &background(&v, f), &near(cell)) > 0.02,
        "the revealed sprout paints nothing"
    );
}

// ---------------------------------------------------------------------------
// 3. the edges of the step
// ---------------------------------------------------------------------------

/// The weights are a partition of the stamp, the growth layer rises smoothly out of nothing at
/// each end, and the *image* at each end is the neighbouring idle stage's own image: a step
/// that cut to and from the clip would step the sway phase and the sprout's pulse at both ends
/// of every step.
#[test]
fn the_step_is_a_partition_of_three_weights_and_enters_and_leaves_on_the_idle_stage_images() {
    assert!(GROW_BLEND > 0.0 && GROW_BLEND < 0.5, "the two edge blends would overlap");

    // The weights, as a pure function: a partition, the reach of each blend, zero slope at
    // both edges, and nonsense held on the lower stage.
    let mut sampled = Vec::new();
    for i in 0..=2000 {
        let t = f64::from(i) / 2000.0;
        let [from, grow, to] = growth_weights(t);
        for (name, w) in [("w_from", from), ("w_grow", grow), ("w_to", to)] {
            assert!((0.0..=1.0).contains(&w), "t = {t}: {name} is {w}");
        }
        assert!((from + grow + to - 1.0).abs() < 1e-6, "t = {t}: the weights sum to {}", from + grow + to);
        assert!(!(from > 0.0 && to > 0.0), "t = {t}: both edge blends are live");
        assert_eq!(from > 0.0, t < GROW_BLEND, "t = {t}: the entry blend's reach");
        assert_eq!(to > 0.0, t > 1.0 - GROW_BLEND, "t = {t}: the exit blend's reach");
        sampled.push((t, grow));
    }
    assert_eq!(growth_weights(0.0), [1.0, 0.0, 0.0], "t = 0 is the lower stage alone");
    assert_eq!(growth_weights(1.0), [0.0, 0.0, 1.0], "t = 1 is the upper stage alone");
    assert_eq!(growth_weights(0.5), [0.0, 1.0, 0.0], "the middle is the clip alone");
    assert_eq!(growth_weights(f64::NAN), [1.0, 0.0, 0.0], "nonsense holds the lower stage");
    // The growth layer rises monotonically out of 0 over the entry blend, with zero slope at 0.
    let entry: Vec<(f64, f32)> =
        sampled.iter().copied().filter(|&(t, _)| t <= GROW_BLEND).collect();
    for pair in entry.windows(2) {
        assert!(pair[1].1 >= pair[0].1 - 1e-9, "the growth layer dipped at t = {}", pair[1].0);
    }
    let slope = |t: f64| f64::from((growth_weights(t + 0.0005)[1] - growth_weights(t)[1]).abs());
    assert!(
        slope(0.0) < slope(GROW_BLEND / 2.0) / 10.0,
        "a kink where the growth layer starts: {} vs {}",
        slope(0.0),
        slope(GROW_BLEND / 2.0)
    );
    assert!(
        slope(1.0 - 0.0005) < slope(1.0 - GROW_BLEND / 2.0) / 10.0,
        "a kink where the growth layer ends"
    );

    // And the images: the closer `t` gets to an end, the closer the drawn frame is to that
    // end's idle stage image, which is exactly what the blend promises. The residual is the
    // growth layer's own weight — `1 − smoothstep(t / GROW_BLEND) ≈ 3 (t / GROW_BLEND)²` at the
    // entry — times at most one premultiplied channel, so it must fall off *quadratically*.
    let cell = pilot_cell();
    let art = plants_only();
    let budget = plant_bend_budget(art.plant(PILOT).unwrap());

    let mut entry_diffs: Vec<(f64, f32)> = Vec::new();
    let mut p = ArtPresenter::new(plants_only());
    p.observe(&bare_view(0));
    // One tick at a time, so `t` lands just inside the entry blend.
    let mut tick = 1u64;
    let mut seen = 0;
    while tick < 400 && seen < 3 {
        let v = one_cell_view(tick, cell, DENSITY);
        p.observe(&v);
        for frame in 0..FRAMES_PER_TICK {
            let f = frame as f64 / FRAMES_PER_TICK as f64;
            let Some(step) = drawn_step(&p, cell, f) else { continue };
            if step.lower != Some(0) || step.t <= 0.0 || step.t >= GROW_BLEND / 4.0 {
                continue;
            }
            let seconds = present_seconds(v.tick, f);
            let bg = background(&v, f);
            let idle =
                expected_idle(&art, &bg, cell, DENSITY, seconds, wind_of(cell, budget, seconds), 0);
            let actual = draw(&mut p, &v, f);
            entry_diffs.push((step.t, max_diff(&actual, &idle)));
            seen += 1;
        }
        tick += 1;
    }
    assert!(
        entry_diffs.len() >= 3,
        "the sweep found only {} frames inside the entry blend",
        entry_diffs.len()
    );
    println!("entry: {entry_diffs:?}");
    for &(t, d) in &entry_diffs {
        let w_grow = f64::from(growth_weights(t)[1]);
        assert!(
            f64::from(d) <= w_grow + 1e-6,
            "t = {t}: the frame is {d} from the idle stage-0 image, over the growth layer's own \
             weight {w_grow}"
        );
    }
    let smallest = entry_diffs.iter().copied().fold((1.0, f32::MAX), |a, b| if b.0 < a.0 { b } else { a });
    assert!(
        smallest.1 < 0.01,
        "at t = {} the frame is still {} from the idle stage-0 image: the step enters with a cut",
        smallest.0,
        smallest.1
    );

    // The exit, the same way, against stage 1's idle image: every frame of the last sliver of
    // the step, and then the very last one before the step completes.
    let mut exit_diffs: Vec<(f64, f32)> = Vec::new();
    let mut p = ArtPresenter::new(plants_only());
    p.observe(&bare_view(0));
    for tick in 1..400u64 {
        let v = one_cell_view(tick, cell, DENSITY);
        p.observe(&v);
        for frame in 0..FRAMES_PER_TICK {
            let f = frame as f64 / FRAMES_PER_TICK as f64;
            let Some(step) = drawn_step(&p, cell, f) else { continue };
            if step.lower != Some(0) || step.t <= 1.0 - GROW_BLEND / 8.0 || step.t >= 1.0 {
                continue;
            }
            let seconds = present_seconds(v.tick, f);
            let bg = background(&v, f);
            let idle =
                expected_idle(&art, &bg, cell, DENSITY, seconds, wind_of(cell, budget, seconds), 1);
            let actual = draw(&mut p, &v, f);
            exit_diffs.push((step.t, max_diff(&actual, &idle)));
        }
    }
    assert!(exit_diffs.len() >= 3, "the sweep found only {} frames in the exit blend", exit_diffs.len());
    println!("exit: {exit_diffs:?}");
    for &(t, d) in &exit_diffs {
        let w_grow = f64::from(growth_weights(t)[1]);
        assert!(
            f64::from(d) <= w_grow + 1e-6,
            "t = {t}: the frame is {d} from the idle stage-1 image, over the growth layer's own \
             weight {w_grow}"
        );
    }
    let last = exit_diffs.iter().copied().fold((0.0, f32::MAX), |a, b| if b.0 > a.0 { b } else { a });
    assert!(
        last.1 < 0.01,
        "at t = {} the frame is still {} from the idle stage-1 image: the step leaves with a cut",
        last.0,
        last.1
    );
}

// ---------------------------------------------------------------------------
// 4. reversal
// ---------------------------------------------------------------------------

/// A plant that starts to wilt retraces the pictures it came through. `growth_step` is the
/// rule: `t` is the *upper* stage's progress whichever way the step travels, so a wilting
/// `1 → 0` at `g = 1 − p` is the same `t = p` as a growing `0 → 1` at `g = p`, and the clip is
/// a pure function of `t`. Drawn at the same simulated instant, against the same fields, the
/// two must be the same image — bit for bit, because nothing else differs.
///
/// The progresses are exact binary fractions: an observe a whole
/// `MAX_STEP_SECONDS` (1 s) after the last advances a rising step by exactly
/// `1 / STAGE_GROW_SECONDS = 0.25`, and a half-second observe advances a falling one by
/// exactly `0.5 / STAGE_WILT_SECONDS = 0.25`, so growing and wilting can meet on the very same
/// `t` and the comparison needs no tolerance.
#[test]
fn wilting_through_a_step_draws_the_growing_pictures_backwards_at_the_same_progress() {
    // First the rule itself, as a pure function of the two orientations.
    for i in 0..=100 {
        let p = f64::from(i) / 100.0;
        let up = Growth { from: Some(0), to: Some(1), g: p, target: Some(1), fruit: 0.0 };
        let down = Growth { from: Some(1), to: Some(0), g: 1.0 - p, target: None, fruit: 0.0 };
        assert_eq!(
            growth_step(up).map(|s| (s.lower, s.upper)),
            Some((Some(0), 1)),
            "p = {p}: the rising pair"
        );
        let (a, b) = (growth_step(up).unwrap(), growth_step(down).unwrap());
        assert_eq!((a.lower, a.upper), (b.lower, b.upper), "p = {p}: the pair must be ordered");
        assert!((a.t - b.t).abs() < 1e-15, "p = {p}: {} wilting vs {} growing", b.t, a.t);
    }

    let cell = pilot_cell();
    // The instant every frame below is drawn at: windy, so the bend is live through the
    // reversal too, and the same for both presenters.
    let shown = one_cell_view(120, cell, DENSITY);
    let f = 1.0;
    let seconds = present_seconds(shown.tick, f);
    let budget = plant_bend_budget(pack().plant(PILOT).unwrap());
    let (bend, _) = slot_wind(&slot_of(cell), PILOT, budget, seconds);
    assert!(!bend.is_identity(), "the fixture instant must be windy");

    // `grows` observes n ticks of rich world 20 apart; `wilts` grows one step further and is
    // then starved for half a second, which turns the step round at the same progress.
    for (progress, grown, extra) in [(0.25, 5u64, 6u64), (0.5, 6, 7), (0.75, 7, 8)] {
        let mut growing = ArtPresenter::new(plants_only());
        drive(&mut growing, cell, DENSITY, 20, grown);
        let up = drawn_step(&growing, cell, f).expect("the growing fixture is in flight");
        assert!((up.t - progress).abs() < 1e-15, "growing t is {} not {progress}", up.t);
        assert_eq!(growing.growth_of(cell).to, Some(1), "the growing plant must be climbing");

        let mut wilting = ArtPresenter::new(plants_only());
        drive(&mut wilting, cell, DENSITY, 20, extra);
        // Starve it: 10 ticks (half a second) of bare ground turns the step round and walks it
        // back by exactly a quarter of the step.
        wilting.observe(&bare_view(extra * 20 + 10));
        let growth = wilting.growth_of(cell);
        assert_eq!(
            (growth.from, growth.to),
            (Some(1), Some(0)),
            "the starved plant must be wilting, not {growth:?}"
        );
        let down = drawn_step(&wilting, cell, f).expect("the wilting fixture is in flight");
        assert_eq!(
            (down.lower, down.upper),
            (up.lower, up.upper),
            "the wilting step must be the same pair"
        );
        assert!(
            (down.t - up.t).abs() < 1e-15,
            "progress {progress}: wilting is at t = {}, growing at {}",
            down.t,
            up.t
        );

        let a = draw(&mut growing, &shown, f);
        let b = draw(&mut wilting, &shown, f);
        assert_same_canvas(&a, &b, &format!("wilting vs growing at t = {progress}"));
        assert!(
            max_diff_at(&a, &background(&shown, f), &near(cell)) > 0.05,
            "progress {progress}: the fixture draws no plant"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. purity and rate independence
// ---------------------------------------------------------------------------

/// The clip is a pure function of `t`: a draw neither advances it nor restarts it, so a
/// thousand draws of one frame are one frame, the growth state is untouched, and three hosts
/// running at 30, 60 and 120 fps that have drawn wildly different numbers of frames all show
/// the same picture at the same simulated instant. A clip resumed from a player's own cursor
/// would fail every one of these.
#[test]
fn a_growth_step_is_pure_and_independent_of_the_render_rate() {
    let cell = pilot_cell();
    let mut p = ArtPresenter::new(plants_only());
    let v = drive(&mut p, cell, DENSITY, 20, 6);
    let step = drawn_step(&p, cell, 0.25).expect("in flight");
    assert_eq!((step.lower, step.upper), (Some(0), 1));

    let before: Vec<Growth> = CellId::all().map(|c| p.growth_of(c)).collect();
    let before_prev: Vec<Growth> = CellId::all().map(|c| p.growth_prev_of(c)).collect();
    let first = draw(&mut p, &v, 0.25);
    for _ in 0..25 {
        assert_same_canvas(&first, &draw(&mut p, &v, 0.25), "a repeated draw of one frame");
    }
    assert_eq!(CellId::all().map(|c| p.growth_of(c)).collect::<Vec<_>>(), before, "a draw moved the growth");
    assert_eq!(
        CellId::all().map(|c| p.growth_prev_of(c)).collect::<Vec<_>>(),
        before_prev,
        "a draw moved the previous-tick copy the frames interpolate from"
    );
    // A wind packet arriving between two draws of the same frame is not a thing either: the
    // same (state, view, f) is the same instant, so the bend is the same too.
    assert_same_canvas(&first, &draw(&mut p, &v, 0.25), "one more draw, after 25 others");

    // Three render rates, one simulated history. Against a 20 Hz clock a 30 fps host draws
    // three frames every two ticks, a 60 fps host three a tick and a 120 fps host six, so by
    // the end they have drawn 120, 240 and 480 frames of the same 200 ticks. Wherever their
    // schedules meet — a frame whose own instant is a tick boundary, `f = 0` — the picture must
    // be identical, and so must the growth state a draw is forbidden to touch.
    const TICKS: u64 = 200;
    let rates: [u64; 3] = [30, 60, 120];
    let plans: Vec<Vec<Vec<f64>>> = rates.iter().map(|&rate| schedule(rate, TICKS)).collect();
    for (rate, plan) in rates.iter().zip(&plans) {
        let frames: usize = plan.iter().map(Vec::len).sum();
        assert!(frames > 0, "{rate} fps drew nothing");
    }
    let mut hosts: Vec<ArtPresenter> =
        rates.iter().map(|_| ArtPresenter::new(plants_only())).collect();
    for host in &mut hosts {
        host.observe(&bare_view(0));
    }
    let mut compared = 0;
    for tick in 1..=TICKS {
        let v = one_cell_view(tick, cell, DENSITY);
        let mut images: Vec<Option<Canvas>> = Vec::new();
        for ((host, plan), _) in hosts.iter_mut().zip(&plans).zip(&rates) {
            host.observe(&v);
            let mut at_boundary = None;
            for &f in &plan[tick as usize] {
                let image = draw(host, &v, f);
                if f < 1e-12 {
                    at_boundary = Some(image);
                }
            }
            images.push(at_boundary);
        }
        for host in &hosts[1..] {
            assert_eq!(
                host.growth_of(cell),
                hosts[0].growth_of(cell),
                "tick {tick}: the render rate moved the growth"
            );
        }
        // Every rate that drew the tick boundary this tick must have drawn the same picture.
        let drawn: Vec<(u64, &Canvas)> = rates
            .iter()
            .zip(&images)
            .filter_map(|(&rate, image)| image.as_ref().map(|c| (rate, c)))
            .collect();
        assert!(drawn.len() >= 2, "tick {tick}: only {} rate(s) hit the boundary", drawn.len());
        for &(rate, image) in &drawn[1..] {
            assert_same_canvas(
                drawn[0].1,
                image,
                &format!("tick {tick} at f = 0: {} fps vs {rate} fps", drawn[0].0),
            );
        }
        if drawn.len() == 3 && drawn_step(&hosts[0], cell, 0.0).is_some_and(|s| s.lower == Some(0)) {
            compared += 1;
        }
    }
    assert!(compared > 30, "all three rates met inside the clip step only {compared} times");
}

/// The `f` values a host running at `rate` frames a second draws inside each tick of a 20 Hz
/// clock: frame `i` shows presentation second `i / rate`, which [`present_seconds`] places at
/// `tick = floor(seconds / DT) + 1` and `f = fract(seconds / DT)`.
/// The arithmetic is integer — `seconds / DT = i · hz / rate` with `hz = 1 / DT` the clock's
/// 20 Hz — so a frame that lands exactly on a tick boundary is recognised as such instead of
/// falling a rounding error short of it.
fn schedule(rate: u64, ticks: u64) -> Vec<Vec<f64>> {
    let hz = (1.0 / DT).round() as u64;
    assert_eq!(1.0 / DT, hz as f64, "the clock must be a whole number of ticks a second");
    let mut out = vec![Vec::new(); ticks as usize + 1];
    for i in 0.. {
        let (whole, part) = ((i * hz) / rate, (i * hz) % rate);
        let tick = whole + 1;
        if tick > ticks {
            break;
        }
        out[tick as usize].push(part as f64 / rate as f64);
    }
    out
}

// ---------------------------------------------------------------------------
// 6. continuity at 60 fps
// ---------------------------------------------------------------------------

/// Nothing in the step may jump. **The bound is derived, not tuned.**
///
/// The pilot is 24 samples over 4 s, non-looping, so its samples sit `4 / 23 = 0.1739` s apart
/// and `Clip::sample` blends between the two bracketing ones. At 60 fps one frame therefore
/// advances the clip by `Δu = (1/60) / (4/23) = 0.0958` of a sample interval, and — the stamp
/// being an exact lerp in the sampled pose — moves any pixel by at most `Δu` times the largest
/// adjacent-sample difference, `jump`, which is measured here by stamping the clip's own
/// samples alone over black at this slot's anchor, heading and opacity. Composited source-over
/// a background the same change can be doubled (`out = src + (1 − a)·bg`, so a change in the
/// source's alpha also uncovers the background), hence `2 · Δu · jump`. Everything else in the
/// window moves too — the two idle sway clips the edges blend with, the wind bend, the ground
/// cover's own breath — and that is measured independently as `sway`, the largest per-frame
/// change a presenter *snapped* to a stage shows over the same frames; an in-flight cell pays
/// for it twice because it draws two layers. So:
///
/// ```text
/// bound = 2 · sway + 2 · Δu · jump
/// ```
///
/// A clip stepped once per baked sample would move `jump` in a single frame — ten times the
/// `Δu · jump` term — and a clip stepped once per tick would move three frames' worth at once.
#[test]
fn the_authored_step_moves_less_than_a_fraction_of_a_baked_sample_per_frame_at_60_fps() {
    let cell = pilot_cell();
    let window = near(cell);
    let art = plants_only();
    let plant = art.plant(PILOT).unwrap();
    let clip = plant.transition(0, 1).expect("the pilot clip");
    let n = clip.frames.len();
    assert_eq!((n, clip.seconds), (24, STAGE_GROW_SECONDS), "the derivation assumes 24 samples over 4 s");
    let du = (1.0 / 60.0) / (clip.seconds / (n - 1) as f64);

    // `jump`: the largest adjacent-sample difference of the clip, stamped alone over black at
    // this slot's own anchor, heading and opacity.
    let slot = slot_of(cell);
    let opacity = band_opacity(Band::Foliage);
    let stamp_alone = |sprite| {
        let mut canvas = Canvas::new();
        stamp_pose(
            &mut canvas,
            slot.at,
            slot.heading,
            Pose::still(sprite),
            1.0,
            opacity,
            Mask::None,
            &mut Vec::new(),
        );
        canvas
    };
    let mut jump = 0.0f32;
    for pair in clip.frames.windows(2) {
        jump = jump.max(max_diff(&stamp_alone(&pair[0]), &stamp_alone(&pair[1])));
    }
    assert!(jump > 0.0, "the pilot clip does not move at all between samples");

    // The frames the sweep walks: every tick of the 0 → 1 step, at 60 fps.
    let ticks: Vec<u64> = (1..=200).collect();

    // `sway`: the same frames, with the plant snapped idle at each stage it blends with.
    let mut sway = 0.0f32;
    for stage_density in [0.30, DENSITY] {
        let mut control = ArtPresenter::new(plants_only());
        control.observe(&one_cell_view(0, cell, stage_density));
        let mut last: Option<Canvas> = None;
        for &tick in &ticks {
            let v = one_cell_view(tick, cell, stage_density);
            control.observe(&v);
            for frame in 0..FRAMES_PER_TICK {
                let image = draw(&mut control, &v, frame as f64 / FRAMES_PER_TICK as f64);
                if let Some(last) = &last {
                    sway = sway.max(max_diff_at(&image, last, &window));
                }
                last = Some(image);
            }
        }
        assert!(sway > 0.0, "the control plant at density {stage_density} does not move at all");
    }
    let bound = 2.0 * sway + 2.0 * du as f32 * jump;
    println!("sway {sway}, sample jump {jump}, du {du}, bound {bound}");

    let mut p = ArtPresenter::new(plants_only());
    p.observe(&bare_view(0));
    let mut last: Option<Canvas> = None;
    let mut worst = 0.0f32;
    let mut lowest = 1.0f64;
    let mut highest = 0.0f64;
    let mut frames = 0;
    for &tick in &ticks {
        let v = one_cell_view(tick, cell, DENSITY);
        p.observe(&v);
        for frame in 0..FRAMES_PER_TICK {
            let f = frame as f64 / FRAMES_PER_TICK as f64;
            let step = drawn_step(&p, cell, f);
            let image = draw(&mut p, &v, f);
            // Only the clip step itself is measured against the derived bound.
            let inside = step.is_some_and(|s| s.lower == Some(0) && s.upper == 1);
            if let (Some(last), true) = (&last, inside) {
                let moved = max_diff_at(&image, last, &window);
                worst = worst.max(moved);
                assert!(
                    moved <= bound,
                    "tick {tick} frame {frame} (t = {:?}) moved a pixel by {moved}, over the \
                     derived bound {bound} (sway {sway}, sample jump {jump}, du {du})",
                    step.map(|s| s.t)
                );
                lowest = lowest.min(step.unwrap().t);
                highest = highest.max(step.unwrap().t);
                frames += 1;
            }
            last = Some(image);
        }
    }
    println!("the worst frame of the step moved {worst}; t ran {lowest} .. {highest} over {frames} frames");
    assert!(worst > 0.0, "the step never moved at all");
    assert!(lowest < 0.02, "the sweep missed the start of the step (lowest t {lowest})");
    assert!(highest > 0.98, "the sweep missed the end of the step (highest t {highest})");
    assert!(frames > 200, "a 4 s step at 60 fps is 240 frames, not {frames}");
}

// ---------------------------------------------------------------------------
// 7. the fallback
// ---------------------------------------------------------------------------

/// With the transitions cleared — a v1–v4 pack, or any pair the art has no clip for — the step
/// is the reveal mask again, bit for bit: the lower stage fading at `opacity · (1 − t)` under
/// the upper stage revealed along the stalk. And clearing them changes *nothing else*: the
/// `None → 0` step of the very same plant — the one step no clip can ever cover — draws
/// identically with and without the clips, while both authored steps differ mid-flight.
#[test]
fn a_pack_without_transitions_falls_back_to_the_reveal_masks_and_changes_no_other_step() {
    let cell = pilot_cell();
    let art = without_transitions();
    assert!(art.plant(PILOT).unwrap().transition(0, 1).is_none(), "the fixture keeps a clip");
    let budget = plant_bend_budget(art.plant(PILOT).unwrap());
    assert_eq!(
        budget,
        plant_bend_budget(pack().plant(PILOT).unwrap()),
        "the pilot clip's frames are not what bounds this plant's bend, so the two packs must \
         measure the same budget — otherwise the comparisons below would differ only by wind"
    );

    // The 0 → 1 step, hand-built as the mask path.
    let mut p = ArtPresenter::new(without_transitions());
    let v = drive(&mut p, cell, DENSITY, 20, 6);
    let f = 1.0;
    let step = drawn_step(&p, cell, f).expect("in flight");
    assert_eq!((step.lower, step.upper), (Some(0), 1));
    let seconds = present_seconds(v.tick, f);
    let actual = draw(&mut p, &v, f);
    let expected = expected_step(
        &art,
        &background(&v, f),
        cell,
        DENSITY,
        seconds,
        wind_of(cell, budget, seconds),
        step,
    );
    assert_same_canvas(&actual, &expected, "the 0 → 1 step of a pack with no transitions");

    // And it is a *different* picture from the clip's, or the fallback would be untested.
    let mut clipped = ArtPresenter::new(plants_only());
    drive(&mut clipped, cell, DENSITY, 20, 6);
    assert!(
        !differing(&actual, &draw(&mut clipped, &v, f)).is_empty(),
        "the authored clip and the reveal mask drew the same 0 → 1 picture"
    );

    // Every other pair: drive both packs identically to stage 2 and compare every frame. Only
    // the two authored steps may differ.
    let mut with = ArtPresenter::new(plants_only());
    let mut without = ArtPresenter::new(without_transitions());
    with.observe(&bare_view(0));
    without.observe(&bare_view(0));
    let mut seen: Vec<(Option<u8>, u8)> = Vec::new();
    let mut clip_differed = 0;
    for tick in 1..=320u64 {
        let v = one_cell_view(tick, cell, FULL_DENSITY);
        with.observe(&v);
        without.observe(&v);
        assert_eq!(
            with.growth_of(cell),
            without.growth_of(cell),
            "tick {tick}: the pacing must not depend on the pack's transitions"
        );
        for frame in 0..FRAMES_PER_TICK {
            let f = frame as f64 / FRAMES_PER_TICK as f64;
            let a = draw(&mut with, &v, f);
            let b = draw(&mut without, &v, f);
            let step = drawn_step(&with, cell, f);
            let pair = step.map(|s| (s.lower, s.upper));
            if let Some(pair) = pair {
                if seen.last() != Some(&pair) {
                    seen.push(pair);
                }
            }
            if matches!(pair, Some((Some(0), 1)) | Some((Some(1), 2))) {
                // The two paths necessarily *converge* at the ends of the step — both are the
                // neighbouring idle stage image there — so only the middle of the step is
                // required to differ.
                if (0.2..=0.8).contains(&step.unwrap().t) {
                    assert!(
                        !differing(&a, &b).is_empty(),
                        "tick {tick} frame {frame} at t = {}: the clip step drew the fallback \
                         picture",
                        step.unwrap().t
                    );
                    clip_differed += 1;
                }
            } else {
                assert_same_canvas(
                    &a,
                    &b,
                    &format!("tick {tick} frame {frame}, step {pair:?}: clearing the \
                              transitions changed a step that never had one"),
                );
            }
        }
    }
    assert_eq!(
        seen,
        vec![(None, 0u8), (Some(0), 1), (Some(1), 2)],
        "the sweep must walk all three steps of the climb"
    );
    assert!(clip_differed > 100, "only {clip_differed} frames of the clip step were compared");
}

// ---------------------------------------------------------------------------
// 8. the fruit accent
// ---------------------------------------------------------------------------

/// A growth stamp never reads the `fruit` clip: `advance_growth` holds the accent at 0 while a
/// plant is in flight, and the stamp has no fruit layer at all. A world whose every cell is
/// bursting with fruit must therefore draw the step exactly as a world with none.
#[test]
fn the_fruit_accent_takes_no_part_in_a_growth_step() {
    let cell = pilot_cell();
    assert!(
        pack().plant(PILOT).unwrap().fruit.is_some(),
        "the fixture plant must have a fruit clip for this to mean anything"
    );
    let ripe = vec![1.0f64; CELL_COUNT];
    let mut fruiting = ArtPresenter::new(plants_only());
    let mut plain = ArtPresenter::new(plants_only());
    fruiting.observe_with_fruit(&bare_view(0), Some(&ripe));
    plain.observe_with_fruit(&bare_view(0), None);
    let mut in_flight = 0;
    for tick in 1..=200u64 {
        let v = one_cell_view(tick, cell, DENSITY);
        fruiting.observe_with_fruit(&v, Some(&ripe));
        plain.observe_with_fruit(&v, None);
        assert_eq!(fruiting.growth_of(cell).fruit, 0.0, "tick {tick}: a plant in flight grew fruit");
        for frame in 0..FRAMES_PER_TICK {
            let f = frame as f64 / FRAMES_PER_TICK as f64;
            let Some(step) = drawn_step(&fruiting, cell, f) else { continue };
            let a = draw_fruit(&mut fruiting, &v, f, Some(&ripe));
            let b = draw_fruit(&mut plain, &v, f, None);
            assert_same_canvas(
                &a,
                &b,
                &format!("tick {tick} frame {frame} at t = {}: the fruit field changed a step", step.t),
            );
            if step.lower == Some(0) {
                in_flight += 1;
            }
        }
    }
    assert!(in_flight > 200, "the sweep only saw {in_flight} frames of the clip step");
}

// ---------------------------------------------------------------------------
// 9. the root contact
// ---------------------------------------------------------------------------

/// Tile coordinates of a face pixel centre for a 16×16 tile stamped at `at` with `heading`:
/// the tile's `+x` lies along the heading and its `+y` along the heading turned a quarter
/// turn, and the pivot — the anchor — is the tile centre `(8, 8)`.
fn tile_at(at: SurfacePoint, heading: Vec2, x: u8, y: u8) -> Vec2 {
    let d = Vec2::new(f64::from(x) + 0.5 - at.u, f64::from(y) + 0.5 - at.v);
    let side = Vec2::new(-heading.y, heading.x);
    Vec2::new(heading.dot(d) + 8.0, side.dot(d) + 8.0)
}

/// The plant's lowest painted row is tile row 14, whose centre sits exactly
/// [`PLANT_BEND_ROOT`] (1.5 px) above the tile's bottom edge, and through the whole step
///
/// * the destination pixels *at* that root height are bit-identical to the same picture drawn
///   with no bend at all — the breeze displaces them by exactly zero, so the contact pixel
///   cannot resample and skate while the rest of the plant leans;
/// * the painted footprint of that row does not change from frame to frame — the clip's art
///   keeps the contact where the stage art has it, so the plant never lifts off the ground or
///   widens its foot;
/// * and nothing at or below tile row 15 is ever painted, so the step never grows *downward*
///   out of its slot.
///
/// (The root row's *colour* does change slightly over the first and last [`GROW_BLEND`] of the
/// step, and must: the sprout's idle sway pulses its whole sprite and the edge blends mix that
/// pulse with the clip's neutral pose. What may not change is where the plant stands.)
#[test]
fn the_root_row_is_fixed_through_the_step_and_nothing_below_it_is_painted() {
    let cell = pilot_cell();
    let slot = slot_of(cell);
    let art = plants_only();
    let budget = plant_bend_budget(art.plant(PILOT).unwrap());

    // One windy instant, drawn again and again as the step advances: the growth is driven by
    // observing later ticks, but every frame is *drawn* against one fixed view, so the wind,
    // the fields and the background are identical and only `t` changes.
    let shown = one_cell_view(120, cell, DENSITY);
    let f = 1.0;
    let seconds = present_seconds(shown.tick, f);
    let (bend, heading) = wind_of(cell, budget, seconds);
    assert!(!bend.is_identity(), "the fixture instant must be windy");
    assert_eq!(
        bend.root, PLANT_BEND_ROOT,
        "the small-plant bend must be rooted at the painted root row"
    );
    assert_eq!(bend.length, PLANT_BEND_LENGTH);
    // A 16-row tile: `Bend::displacement(16, p_y)` reads `H = 16 − p_y`, so the root row's own
    // height is exactly fixed and the rows above it are not.
    assert_eq!(
        bend.displacement(16.0, 16.0 - PLANT_BEND_ROOT),
        0.0,
        "the root row must not be displaced at all"
    );
    assert!(
        bend.displacement(16.0, 16.0 - PLANT_BEND_ROOT - 4.0).abs() > 0.0,
        "the stem four pixels up must move, or the fixture proves nothing"
    );

    // The destination pixels whose own height is at or below the root line (displacement
    // exactly 0, bilinear support in rows 14 and 15), and the pixels whose support lies wholly
    // at or below row 15, which nothing may paint.
    let root_row: Vec<(Face, u8, u8)> = near(cell)
        .into_iter()
        .filter(|&(_, x, y)| (14.5..15.5).contains(&tile_at(slot.at, heading, x, y).y))
        .collect();
    let below: Vec<(Face, u8, u8)> = near(cell)
        .into_iter()
        .filter(|&(_, x, y)| tile_at(slot.at, heading, x, y).y >= 15.5)
        .collect();
    assert!(root_row.len() >= 8, "the fixture found only {} root-row pixels", root_row.len());
    assert!(below.len() >= 8, "the fixture found only {} pixels below the plant", below.len());

    let bg = background(&shown, f);
    let mut p = ArtPresenter::new(plants_only());
    p.observe(&bare_view(0));
    let mut footprint: Option<(f64, Vec<(Face, u8, u8)>)> = None;
    let mut seen = 0;
    for tick in 1..=200u64 {
        p.observe(&one_cell_view(tick, cell, DENSITY));
        let Some(step) = drawn_step(&p, cell, f) else { continue };
        if step.lower != Some(0) {
            continue;
        }
        let image = draw(&mut p, &shown, f);

        // Nothing at or below row 15, ever.
        let leak = max_diff_at(&image, &bg, &below);
        assert!(leak == 0.0, "t = {}: the step painted {leak} below the plant's lowest row", step.t);

        // The root row is the windless image, bit for bit, while the rest of the plant leans.
        let calm =
            expected_step(&art, &bg, cell, DENSITY, seconds, (Bend::NONE, heading), step);
        let skated = max_diff_at(&image, &calm, &root_row);
        assert!(
            skated == 0.0,
            "t = {}: the root row differs from the windless image by {skated} — the contact is \
             being displaced",
            step.t
        );
        assert!(
            max_diff_at(&image, &calm, &near(cell)) > 0.0,
            "t = {}: the breeze moved nothing at all, so the fixture proves nothing",
            step.t
        );

        // And the footprint of that row is the same every frame.
        let painted: Vec<(Face, u8, u8)> = root_row
            .iter()
            .copied()
            .filter(|&(fc, x, y)| image.get(fc, x, y) != bg.get(fc, x, y))
            .collect();
        assert!(!painted.is_empty(), "t = {}: the root row is not painted at all", step.t);
        match &footprint {
            None => footprint = Some((step.t, painted)),
            Some((first_t, first)) => assert_eq!(
                &painted, first,
                "the root row's footprint changed between t = {first_t} and t = {}",
                step.t
            ),
        }
        seen += 1;
    }
    assert!(seen > 60, "the sweep only saw {seen} ticks of the step");
}
