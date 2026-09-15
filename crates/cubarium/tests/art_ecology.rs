//! Ecology v1's presentation slice, tested from `design/ecology-v1-contract.md` §3, §11 and
//! §12 and from the public doc comments of `cubarium::art_present` — not from the
//! implementation. Three claims are on trial:
//!
//! 1. **Living structure persists when foliage is stripped.** A cell with `W > 0` and
//!    `P ≈ 0` is visibly a plant, not empty soil.
//! 2. **Foliage loss and recovery happen on that structure**, following the stocks
//!    continuously, without flicker and without regrowth that masks depletion.
//! 3. **Dead wood** (`Wd > 0`, `W = 0`) is distinct from living structure and from soil,
//!    and fades as its stock decays.
//!
//! The reference stands are the implementation note's measured ones
//! (`design/7_Research/ecology-v1-implementation-2026-09-15.md`, Run 3, row B0):
//! average light `P = 0.0977, W = 0.1050`, bright `P = 0.4789, W = 0.3934`, against
//! `W_max = 0.6`, `W_min = 0.02`, `α = 2`.

use std::path::Path;

use cube_proto::{FACE_SIZE, Face};
use cubarium::art::{ArtPack, Band};
use cubarium::art_present::{
    ArtPresenter, CANOPY_STAGES, DEAD_WOOD_OPACITY, FOLIAGE_FULL, FOLIAGE_PER_WOOD,
    FOLIAGE_STAGES, MOTIF_OPACITY, SOIL_SCALE, STAGE_HYST, WOOD_SHAPE, band_of, dead_wood_density,
    dead_wood_tone, foliage_fullness, foliage_ramp, litter_density, next_stage, plant_cap,
    living_wood_tone, plant_density, rank_cap_of, stage_thresholds, structural, wood_density,
    wood_for_density, wood_fraction,
};
use cubarium_core::view::RenderView;
use cubarium_render::Canvas;
use cubarium_surface::{CELL_COUNT, CellId};

// ---------------------------------------------------------------------------
// the measured stands of B0, and the contract's constants
// ---------------------------------------------------------------------------

const W_MAX: f64 = 0.6;
const W_MIN: f64 = 0.02;
/// B0, average light: `P = 0.0977, W = 0.1050`.
const AVERAGE: (f64, f64) = (0.0977, 0.1050);
/// B0, bright: `P = 0.4789, W = 0.3934`.
const BRIGHT: (f64, f64) = (0.4789, 0.3934);

fn pack() -> ArtPack {
    ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
        .expect("the baked pack at assets/atelier must load")
}

fn empty_view(tick: u64) -> RenderView {
    RenderView {
        tick,
        producer: vec![0.0; CELL_COUNT],
        detritus: vec![0.0; CELL_COUNT],
        fruit: vec![0.0; CELL_COUNT],
        wood: vec![0.0; CELL_COUNT],
        plant_reserve: vec![0.0; CELL_COUNT],
        dead_wood: vec![0.0; CELL_COUNT],
        carrion: vec![0.0; CELL_COUNT],
        water: vec![0.0; CELL_COUNT],
        rain: vec![0.0; CELL_COUNT],
        producer_max: 1.5,
        wood_max: W_MAX,
        organisms: Vec::new(),
    }
}

/// One cell holding a stand: living wood `w`, foliage `p`, dead wood `wd`.
fn stand(tick: u64, cell: CellId, w: f64, p: f64, wd: f64) -> RenderView {
    let mut v = empty_view(tick);
    v.wood[cell.index()] = w;
    v.producer[cell.index()] = p;
    v.plant_reserve[cell.index()] = 0.5 * w;
    v.dead_wood[cell.index()] = wd;
    v
}

/// A foliage-band cell on Front whose rank lets it reach stage 2 and whose tile stays well
/// inside one face, so what is drawn is one plant and nothing else.
fn pilot() -> CellId {
    CellId::all()
        .find(|&c| {
            c.face() == Face::Front
                && band_of(c) == Band::Foliage
                && rank_cap_of(c) == 2
                && (3..=7).contains(&c.cy())
                && (4..=11).contains(&c.cx())
        })
        .expect("a rank-2 front foliage slot away from the edges")
}

/// A canopy (top-face) cell whose rank lets it reach stage 2.
fn canopy_pilot() -> CellId {
    CellId::all()
        .find(|&c| {
            band_of(c) == Band::Canopy
                && rank_cap_of(c) == 2
                && (5..=10).contains(&c.cx())
                && (5..=10).contains(&c.cy())
        })
        .expect("a rank-2 canopy slot away from the edges")
}

/// A presenter snapped to `v` (its first observe), drawn at `f = 0`.
fn snapped(v: &RenderView) -> Canvas {
    let mut p = ArtPresenter::new(pack());
    p.observe(v);
    let mut canvas = Canvas::new();
    p.draw(v, 0.0, &mut canvas);
    canvas
}

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL.into_iter().flat_map(|f| {
        (0..FACE_SIZE as u8).flat_map(move |y| (0..FACE_SIZE as u8).map(move |x| (f, x, y)))
    })
}

/// The pixels where two images differ at all.
fn differing(a: &Canvas, b: &Canvas) -> Vec<(Face, u8, u8)> {
    every_pixel()
        .filter(|&(f, x, y)| a.get(f, x, y) != b.get(f, x, y))
        .collect()
}

/// The largest absolute per-channel difference between two images.
fn max_diff(a: &Canvas, b: &Canvas) -> f32 {
    every_pixel()
        .flat_map(|(f, x, y)| {
            let (p, q) = (a.get(f, x, y), b.get(f, x, y));
            (0..3).map(move |c| (p[c] - q[c]).abs())
        })
        .fold(0.0, f32::max)
}

/// Total light in an image: what "how much is painted here" means with no reference image.
fn total(c: &Canvas) -> f64 {
    every_pixel()
        .map(|(f, x, y)| c.get(f, x, y).iter().map(|&v| f64::from(v)).sum::<f64>())
        .sum()
}

/// The light one cell's plant adds over the same world with that cell bare.
fn painted(v: &RenderView, bare: &Canvas) -> f64 {
    total(&snapped(v)) - total(bare)
}

// ---------------------------------------------------------------------------
// 1. the mapping itself
// ---------------------------------------------------------------------------

#[test]
fn the_structural_read_is_the_cube_root_of_the_wood_fraction_and_is_total() {
    for w in [0.0, W_MIN, 0.05, 0.1, AVERAGE.1, BRIGHT.1, W_MAX] {
        let want = (w / W_MAX).powf(WOOD_SHAPE);
        assert!(
            (wood_fraction(w, W_MAX) - want).abs() < 1e-12,
            "W = {w} read {} not {want}",
            wood_fraction(w, W_MAX)
        );
    }
    // Above the maximum the fraction saturates rather than running away.
    assert_eq!(wood_fraction(10.0 * W_MAX, W_MAX), 1.0);
    // Nonsense is bare ground, never a panic and never a NaN density.
    for (value, max) in [
        (f64::NAN, W_MAX),
        (-1.0, W_MAX),
        (0.3, 0.0),
        (0.3, -1.0),
        (0.3, f64::NAN),
        (f64::INFINITY, W_MAX),
    ] {
        let d = wood_fraction(value, max);
        assert!(d == 0.0, "wood_fraction({value}, {max}) = {d}");
    }
}

#[test]
fn the_calibration_puts_the_measured_stands_where_the_brief_asks() {
    let th = FOLIAGE_STAGES;
    let stage = |w: f64| next_stage(None, wood_fraction(w, W_MAX), &th, 2);
    // A stand at the alive threshold is a plant, not bare soil — the whole of claim 1.
    assert_eq!(stage(W_MIN), Some(0), "a just-alive stand shows nothing");
    // And so is every living cell: the stage-0 entry sits *below* `W_min`.
    let entry = W_MAX * th[0].powi(3);
    assert!(
        entry < W_MIN,
        "stage 0 starts at W = {entry}, above the alive threshold {W_MIN}"
    );
    // The two measured classes separate, and the brighter one is full-grown.
    assert_eq!(stage(AVERAGE.1), Some(1), "the average-light stand");
    assert_eq!(stage(BRIGHT.1), Some(2), "the bright stand");
    assert_eq!(stage(W_MAX), Some(2));
    // Nothing at all where there is no wood.
    assert_eq!(stage(0.0), None);
    // The canopy's lower thresholds never read *less* grown than the foliage's.
    for w in [W_MIN, 0.05, AVERAGE.1, BRIGHT.1, W_MAX] {
        let t = wood_fraction(w, W_MAX);
        let f = next_stage(None, t, &FOLIAGE_STAGES, 2).map_or(-1, i32::from);
        let c = next_stage(None, t, &CANOPY_STAGES, 2).map_or(-1, i32::from);
        assert!(c >= f, "canopy read below foliage at W = {w}");
    }
}

#[test]
fn both_ungrazed_classes_read_as_a_full_canopy_and_a_stripped_one_reads_as_none() {
    let mut v = empty_view(0);
    let cell = pilot();
    for (name, (p, w)) in [("average", AVERAGE), ("bright", BRIGHT)] {
        v.wood[cell.index()] = w;
        v.producer[cell.index()] = p;
        let f = foliage_fullness(&v, cell.index());
        assert!(
            f >= FOLIAGE_FULL,
            "{name}: fullness {f} is below the shoulder {FOLIAGE_FULL}"
        );
        assert_eq!(foliage_ramp(f), 1.0, "{name} is not drawn as a full canopy");
    }
    // `k` is what the brief names it: the foliage per wood a whole canopy carries.
    v.wood[cell.index()] = 0.2;
    v.producer[cell.index()] = FOLIAGE_PER_WOOD * 0.2;
    assert_eq!(foliage_fullness(&v, cell.index()), 1.0);
    // Stripped, and structurally capped: `P_cap = α·W` is 2, and the ratio clamps at 1.
    v.producer[cell.index()] = 0.0;
    assert_eq!(foliage_fullness(&v, cell.index()), 0.0);
    assert_eq!(foliage_ramp(0.0), 0.0);
    v.producer[cell.index()] = 2.0 * 0.2;
    assert_eq!(foliage_fullness(&v, cell.index()), 1.0);
    // No wood is no canopy, whatever the foliage field says.
    v.wood[cell.index()] = 0.0;
    assert_eq!(foliage_fullness(&v, cell.index()), 0.0);
}

#[test]
fn the_fullness_ramp_is_monotone_flat_at_both_ends_and_total() {
    let mut prev = 0.0f32;
    for i in 0..=1000 {
        let f = i as f64 / 1000.0;
        let a = foliage_ramp(f);
        assert!(a >= prev, "the ramp fell at f = {f}");
        assert!((0.0..=1.0).contains(&a), "the ramp left [0, 1] at f = {f}");
        prev = a;
    }
    assert_eq!(foliage_ramp(0.0), 0.0);
    assert_eq!(foliage_ramp(FOLIAGE_FULL), 1.0);
    assert_eq!(foliage_ramp(1.0), 1.0);
    assert_eq!(foliage_ramp(f64::NAN), 0.0);
    // Zero slope at both ends: an ungrazed or a stripped stand cannot breathe.
    assert!(foliage_ramp(0.01) < 0.001, "the empty end is not flat");
    assert!(
        1.0 - foliage_ramp(0.98 * FOLIAGE_FULL) < 0.002,
        "the full end is not flat"
    );
}

#[test]
fn the_soil_band_reads_litter_plus_remains_and_the_structural_bands_do_not() {
    let cell = CellId::all()
        .find(|&c| band_of(c) == Band::Soil)
        .expect("a soil cell");
    let mut v = empty_view(0);
    v.detritus[cell.index()] = 0.3 * SOIL_SCALE;
    let litter_only = plant_density(&v, cell.index(), Band::Soil);
    v.carrion[cell.index()] = 0.2 * SOIL_SCALE;
    let with_remains = plant_density(&v, cell.index(), Band::Soil);
    assert!(
        with_remains > litter_only,
        "a carcass did not enrich the soil: {with_remains} vs {litter_only}"
    );
    assert!((with_remains - 0.5).abs() < 1e-12, "{with_remains}");
    assert!((litter_density(&v, cell.index()) - 0.5).abs() < 1e-12);
    // And the structural bands are wood and nothing else.
    assert!(!structural(Band::Soil) && !structural(Band::Water));
    assert!(structural(Band::Foliage) && structural(Band::Canopy));
    let f = pilot();
    v.detritus[f.index()] = 10.0;
    v.carrion[f.index()] = 10.0;
    v.producer[f.index()] = 10.0;
    assert_eq!(plant_density(&v, f.index(), Band::Foliage), 0.0);
    v.wood[f.index()] = BRIGHT.1;
    assert_eq!(
        plant_density(&v, f.index(), Band::Foliage),
        wood_density(&v, f.index())
    );
}

// ---------------------------------------------------------------------------
// 2. what the five states draw
// ---------------------------------------------------------------------------

/// The five states of one cell the brief names, at the bright stand's structure.
fn five_states(tick: u64, cell: CellId) -> [(&'static str, RenderView); 5] {
    let (p, w) = BRIGHT;
    [
        ("healthy", stand(tick, cell, w, p, 0.0)),
        ("half-grazed", stand(tick, cell, w, 0.5 * p, 0.0)),
        ("stripped", stand(tick, cell, w, 0.0, 0.0)),
        ("dead wood", stand(tick, cell, 0.0, 0.0, w)),
        ("empty", empty_view(tick)),
    ]
}

#[test]
fn the_five_states_of_a_cell_are_pairwise_distinguishable() {
    for cell in [pilot(), canopy_pilot()] {
        let states = five_states(7, cell);
        let drawn: Vec<(&str, Canvas)> = states
            .iter()
            .map(|(name, v)| (*name, snapped(v)))
            .collect();
        for i in 0..drawn.len() {
            for j in i + 1..drawn.len() {
                let d = differing(&drawn[i].1, &drawn[j].1);
                assert!(
                    d.len() >= 4,
                    "{cell:?}: {} and {} differ in only {} pixels",
                    drawn[i].0,
                    drawn[j].0,
                    d.len()
                );
                assert!(
                    max_diff(&drawn[i].1, &drawn[j].1) > 0.01,
                    "{cell:?}: {} and {} are within 0.01 everywhere",
                    drawn[i].0,
                    drawn[j].0
                );
            }
        }
        // And every one of the four living/dead states paints something the empty world
        // does not: none of them is bare soil.
        let bare = &drawn[4].1;
        for (name, image) in &drawn[..4] {
            assert!(
                !differing(image, bare).is_empty(),
                "{cell:?}: {name} drew nothing at all"
            );
        }
    }
}

#[test]
fn a_stripped_stand_paints_the_same_structure_the_healthy_one_paints_under_its_foliage() {
    for cell in [pilot(), canopy_pilot()] {
        let (p, w) = BRIGHT;
        let bare = snapped(&empty_view(7));
        let healthy = snapped(&stand(7, cell, w, p, 0.0));
        let stripped = snapped(&stand(7, cell, w, 0.0, 0.0));
        let a: Vec<_> = differing(&healthy, &bare);
        let b: Vec<_> = differing(&stripped, &bare);
        // The same sprite at the same stage at the same coverage, so the stripped stand
        // touches no pixel the healthy one does not: it is the same shape in another colour.
        assert!(
            b.iter().all(|px| a.contains(px)),
            "{cell:?}: the stripped stand painted outside the healthy one's footprint"
        );
        // It may touch *fewer*: the flat wood tone is darker than the brightest leaf, and at
        // the sprite's faintest edge texels the composite can round back onto the background
        // in f32 where a leaf would not have. The bound is that this is a fringe, not the
        // shape — at least 95 % of the healthy footprint is still painted.
        assert!(
            b.len() * 100 >= a.len() * 95,
            "{cell:?}: the stripped stand kept only {} of {} pixels",
            b.len(),
            a.len()
        );
        assert!(a.len() > 20, "{cell:?}: only {} pixels painted", a.len());
        // The stand did not get quieter, it got browner: coverage is held, not faded.
        assert!(
            max_diff(&healthy, &stripped) > 0.02,
            "{cell:?}: stripping changed no colour"
        );
    }
}

#[test]
fn foliage_is_monotone_in_the_stock_at_a_fixed_structure() {
    // Claim 2's core: lower `P` at the same `W` never draws *more* foliage. Measured as
    // the image's distance from the stripped stand — the pure structure — which rises with
    // `P` and is 0 at `P = 0`.
    for cell in [pilot(), canopy_pilot()] {
        let w = BRIGHT.1;
        let structure = snapped(&stand(7, cell, w, 0.0, 0.0));
        let mut prev = 0.0f32;
        let mut seen_full = 0.0f32;
        for i in 0..=40 {
            let p = i as f64 / 40.0 * (FOLIAGE_PER_WOOD * w);
            let image = snapped(&stand(7, cell, w, p, 0.0));
            let leafiness = max_diff(&image, &structure);
            assert!(
                leafiness >= prev - 1e-6,
                "{cell:?}: P = {p} drew more foliage than the step below it"
            );
            prev = leafiness;
            seen_full = seen_full.max(leafiness);
        }
        assert_eq!(
            max_diff(&snapped(&stand(7, cell, w, 0.0, 0.0)), &structure),
            0.0
        );
        assert!(seen_full > 0.02, "{cell:?}: the sweep never grew foliage");
    }
}

#[test]
fn growth_does_not_mask_depletion_the_stage_falls_with_the_wood_that_carries_it() {
    // A stand that is losing *wood* loses stage, and the picture cannot hide it behind a
    // full canopy: at every step the structure is smaller than the step before.
    let cell = pilot();
    let bare = snapped(&empty_view(7));
    let mut prev = f64::INFINITY;
    for w in [BRIGHT.1, 0.2, 0.1, 0.05, 0.03, W_MIN, 0.0] {
        // Foliage held at full fullness throughout: only the structure is shrinking.
        let light = painted(&stand(7, cell, w, FOLIAGE_PER_WOOD * w, 0.0), &bare);
        assert!(
            light <= prev + 1e-9,
            "W = {w} painted more ({light}) than the larger stand above it ({prev})"
        );
        prev = light;
    }
    assert!(prev.abs() < 1e-9, "a stand with no wood still paints {prev}");
}

// ---------------------------------------------------------------------------
// 3. dead wood
// ---------------------------------------------------------------------------

#[test]
fn dead_wood_fades_monotonically_with_its_stock_and_reaches_soil_at_zero() {
    for cell in [pilot(), canopy_pilot()] {
        let bare = snapped(&empty_view(7));
        let mut prev = f64::INFINITY;
        let sweep = [0.5, 0.39, 0.25, 0.15, 0.08, 0.04, 0.02, 0.01, 0.004, 0.0];
        let mut first = 0.0;
        for (i, wd) in sweep.into_iter().enumerate() {
            let light = painted(&stand(7, cell, 0.0, 0.0, wd), &bare);
            if i == 0 {
                first = light;
            }
            assert!(
                light <= prev + 1e-9,
                "{cell:?}: Wd = {wd} painted more ({light}) than the step above it ({prev})"
            );
            prev = light;
        }
        // Zero is exactly the empty world, not merely a dim one.
        assert!(
            differing(&snapped(&stand(7, cell, 0.0, 0.0, 0.0)), &bare).is_empty(),
            "{cell:?}: dead wood at zero still paints"
        );
        assert!(first > 0.0, "{cell:?}: the sweep never painted dead wood");
        // And the fade is a real one, not two flat steps.
        assert!(
            painted(&stand(7, cell, 0.0, 0.0, 0.05), &bare) < 0.6 * first,
            "{cell:?}: a decayed snag is as loud as a whole one"
        );
    }
}

#[test]
fn dead_wood_is_quieter_and_a_different_colour_from_the_living_structure_it_replaces() {
    let cell = pilot();
    let w = BRIGHT.1;
    let bare = snapped(&empty_view(7));
    let living = snapped(&stand(7, cell, w, 0.0, 0.0));
    let dead = snapped(&stand(7, cell, 0.0, 0.0, w));
    // The same shape, since the species pick is a pure function of the cell.
    let a = differing(&living, &bare);
    let b = differing(&dead, &bare);
    assert_eq!(a, b, "the dead stand is not the living stand's shape");
    // Quieter, by the documented factor's worth.
    let (lit, lost) = (painted(&stand(7, cell, w, 0.0, 0.0), &bare), painted(&stand(7, cell, 0.0, 0.0, w), &bare));
    assert!(
        lost < lit,
        "dead wood ({lost}) is not quieter than living structure ({lit})"
    );
    assert!(DEAD_WOOD_OPACITY < 1.0);
    // Cooler and less saturated than the living tone: the two tones are what they say.
    let spread = |c: [f32; 3]| {
        c.iter().cloned().fold(f32::MIN, f32::max) - c.iter().cloned().fold(f32::MAX, f32::min)
    };
    let (alive, gone) = (living_wood_tone(), dead_wood_tone());
    assert!(
        spread(gone) < 0.3 * spread(alive),
        "the dead tone is not much less saturated than the living one: {gone:?} vs {alive:?}"
    );
    assert!(gone[2] > gone[0], "the dead tone is not cool: {gone:?}");
    assert!(alive[0] > alive[2], "the living tone is not warm: {alive:?}");
    assert_eq!(
        dead_wood_density(&stand(7, cell, 0.0, 0.0, w), cell.index()),
        wood_fraction(w, W_MAX),
        "dead wood does not read through the same mapping"
    );
}

#[test]
fn a_dead_stand_takes_over_from_the_living_one_and_both_show_while_dieback_runs() {
    let cell = pilot();
    let bare = snapped(&empty_view(7));
    // Dieback: `W` falling while `Wd` rises. The cell is never empty on the way through.
    let mut previous: Option<Canvas> = None;
    for i in 0..=10 {
        let t = f64::from(i) / 10.0;
        let v = stand(7, cell, BRIGHT.1 * (1.0 - t), 0.0, BRIGHT.1 * t);
        let image = snapped(&v);
        assert!(
            !differing(&image, &bare).is_empty(),
            "the cell went bare at t = {t} of the handover"
        );
        if let Some(prev) = &previous {
            assert!(
                max_diff(prev, &image) < 0.35,
                "the handover jumped at t = {t}"
            );
        }
        previous = Some(image);
    }
}

// ---------------------------------------------------------------------------
// 4. no flicker, and the presenter's contract
// ---------------------------------------------------------------------------

/// How much *more* than a standing control a moving sequence may step between two
/// consecutive frames at 60 fps, per channel.
///
/// The absolute frame-to-frame change of any plant is dominated by things that were here
/// before ecology v1 — its authored sway clip and the wind bend, which move every frame —
/// so an absolute bound would measure those. What this suite bounds is the *excess*: what
/// stripping, reflushing, dying and decomposing add on top of a stand that is simply
/// standing there. Nothing in ecology v1's presentation is allowed a cut, and a cut would be
/// a step of the order of a plant's own brightness — 0.3 to 0.85 of full scale. 0.05 is a
/// sixth of the smallest of those and about twice what the wind alone already moves a pixel
/// in one frame, which is the scale of change a viewer reads as motion rather than as a
/// jump. Both sequences are run at three frames a tick, the real render rate.
const EXCESS_BOUND: f32 = 0.05;

/// Run a tick-by-tick sequence at three frames a tick and return the largest per-channel
/// change between two consecutive frames, plus the total distance travelled from the first
/// frame to the last (so a test can show the sequence actually did something).
fn worst_frame_step(states: &dyn Fn(u64) -> RenderView, ticks: u64) -> (f32, f32) {
    let mut p = ArtPresenter::new(pack());
    let mut canvas = Canvas::new();
    let mut previous: Option<Canvas> = None;
    let mut first: Option<Canvas> = None;
    let mut worst = 0.0f32;
    for tick in 0..ticks {
        let v = states(tick);
        p.observe(&v);
        for frame in 0..3 {
            p.draw(&v, f64::from(frame) / 3.0, &mut canvas);
            if let Some(prev) = &previous {
                worst = worst.max(max_diff(prev, &canvas));
            }
            if first.is_none() {
                first = Some(canvas.clone());
            }
            previous = Some(canvas.clone());
        }
    }
    let travelled = match (&first, &previous) {
        (Some(a), Some(b)) => max_diff(a, b),
        _ => 0.0,
    };
    (worst, travelled)
}

#[test]
fn stripping_and_reflushing_a_stand_adds_no_step_a_standing_one_does_not_have() {
    let cell = pilot();
    let w = BRIGHT.1;
    let full = FOLIAGE_PER_WOOD * w;
    // 60 ticks stripping the stand, 60 reflushing it.
    let (worst, travelled) = worst_frame_step(
        &|tick| {
            let phase = f64::from(u32::try_from(tick).unwrap());
            let p_now = if tick < 60 {
                full * (1.0 - phase / 59.0)
            } else {
                full * ((phase - 60.0) / 59.0)
            };
            stand(tick + 1, cell, w, p_now, 0.0)
        },
        120,
    );
    let (control, _) = worst_frame_step(&|tick| stand(tick + 1, cell, w, full, 0.0), 120);
    println!("strip and reflush: worst frame step {worst}, standing control {control}");
    assert!(
        worst <= control + EXCESS_BOUND,
        "stripping stepped by {worst} against a standing control's {control}"
    );
    assert!(
        travelled > 0.05,
        "the sequence never actually stripped the stand ({travelled})"
    );
}

#[test]
fn a_dying_and_decomposing_stand_adds_no_step_a_standing_one_does_not_have() {
    let cell = pilot();
    // Dieback into decomposition: `W` to nothing, `Wd` up and then away.
    let (worst, travelled) = worst_frame_step(
        &|tick| {
            let t = f64::from(u32::try_from(tick).unwrap()) / 159.0;
            let (w, wd) = if t < 0.5 {
                (BRIGHT.1 * (1.0 - 2.0 * t), BRIGHT.1 * 2.0 * t)
            } else {
                (0.0, BRIGHT.1 * (2.0 - 2.0 * t))
            };
            stand(tick + 1, cell, w, 0.0, wd)
        },
        160,
    );
    let (control, _) = worst_frame_step(&|tick| stand(tick + 1, cell, BRIGHT.1, 0.0, 0.0), 160);
    println!("die and decompose: worst frame step {worst}, standing control {control}");
    assert!(
        worst <= control + EXCESS_BOUND,
        "dying stepped by {worst} against a standing control's {control}"
    );
    assert!(travelled > 0.05, "the stand never died ({travelled})");
}

#[test]
fn drawing_mutates_nothing_and_re_observing_a_tick_is_idempotent() {
    let cell = pilot();
    let v = stand(9, cell, BRIGHT.1, 0.4 * BRIGHT.1, 0.1);
    let mut p = ArtPresenter::new(pack());
    p.observe(&stand(8, cell, 0.05, 0.02, 0.0));
    p.observe(&v);
    let state = |p: &ArtPresenter| {
        (
            p.growth_of(cell),
            p.growth_prev_of(cell),
            p.dead_growth_of(cell),
            p.dead_growth_prev_of(cell),
            (0..p.columns().len())
                .map(|i| (p.tall_growth_of(i), p.tall_dead_growth_of(i)))
                .collect::<Vec<_>>(),
        )
    };
    let before = state(&p);
    let mut a = Canvas::new();
    p.draw(&v, 0.5, &mut a);
    for _ in 0..8 {
        let mut b = Canvas::new();
        p.draw(&v, 0.5, &mut b);
        assert!(differing(&a, &b).is_empty(), "a repeated draw moved");
    }
    assert_eq!(before, state(&p), "a draw advanced the presenter");
    // Re-observing the same tick retargets but folds nothing forward.
    p.observe(&v);
    let once = state(&p);
    p.observe(&v);
    p.observe(&v);
    assert_eq!(once, state(&p), "re-observing a tick advanced it");
    let mut c = Canvas::new();
    p.draw(&v, 0.5, &mut c);
    assert!(
        differing(&a, &c).is_empty(),
        "re-observing a tick changed the frame"
    );
}

#[test]
fn a_cell_that_grows_nothing_grows_no_dead_wood_either() {
    // The rank cap is what keeps the cube from becoming a wall of sprites; dead wood must
    // obey the same rule or a stand's death would put a plant where none ever stood.
    let sprout_only = CellId::all()
        .find(|&c| band_of(c) == Band::Foliage && plant_cap(Band::Foliage, c).is_none())
        .expect("a sprout-only foliage slot");
    let bare = snapped(&empty_view(7));
    // `producer` stays at zero in both arms: the ground cover and the producer ramp read
    // it, and this test is about the plant slot, not the ground.
    for v in [
        stand(7, sprout_only, BRIGHT.1, 0.0, 0.0),
        stand(7, sprout_only, 0.0, 0.0, BRIGHT.1),
    ] {
        assert!(
            differing(&snapped(&v), &bare).is_empty(),
            "a capped-out slot drew a plant"
        );
    }
}

#[test]
fn the_structural_read_does_not_disturb_the_soil_or_the_water_band() {
    // Claim: soil and water keep exactly the image they had. A soil cell with wood and no
    // litter draws nothing; the same cell with litter draws its litter plant.
    let soil = CellId::all()
        .find(|&c| {
            band_of(c) == Band::Soil && rank_cap_of(c) == 2 && (4..=11).contains(&c.cx())
        })
        .expect("a rank-2 soil slot");
    let bare = snapped(&empty_view(7));
    let wooded = snapped(&stand(7, soil, BRIGHT.1, BRIGHT.0, BRIGHT.1));
    assert!(
        differing(&wooded, &bare).is_empty(),
        "wood grew a plant in the soil band"
    );
    let mut littered = empty_view(7);
    littered.detritus[soil.index()] = SOIL_SCALE;
    assert!(
        !differing(&snapped(&littered), &bare).is_empty(),
        "litter grew nothing in the soil band"
    );
    // The thresholds a soil cell is judged by are still its own.
    assert_eq!(stage_thresholds(Band::Soil).len(), 3);
    assert!(STAGE_HYST > 0.0);
    // And a wooded cell needs `wood_for_density` to hit a chosen structural reading.
    let w = wood_for_density(0.6, W_MAX);
    let v = stand(7, pilot(), w, w, 0.0);
    assert!((wood_density(&v, pilot().index()) - 0.6).abs() < 1e-9);
}
