//! Independent tests for the **authored growth expansion** of the art image (pack v5),
//! generic over every plant and every `grow<from><to>` row the pack carries.
//!
//! Where `tests/art_growth_clip.rs` pins the *pilot* — the lanternstalk's `grow01` — this file
//! states the same properties as a contract over `pack.plants[*].transitions`, so it passes on
//! the pack that carries one transition and gains coverage, with no edit, for every row the art
//! adds: shape, the endpoint convention as alpha-plane equality with the neighbouring stages'
//! phase-0 samples, the planted root, the extent and wind budgets, the derived 60 fps per-frame
//! motion bound, the documented three-layer playback at a windy and a calm instant in every
//! band, reversal, the reveal-mask fallback, and the loader's row placement.
//!
//! Written from the public doc comments of `cubarium::art` (`Clip::sample`, `Plant`,
//! `Plant::transition`, `Transition`, `ArtPack::plant_frames`), `cubarium::art_present`
//! (`GrowthStep`, `growth_step`, `GROW_BLEND`, `growth_weights`, `advance_growth`,
//! `growth_between`, `slot_wind`, `plant_bend`, `plant_bend_budget`, `effective_tip`,
//! `WIND_RESPONSE`, `WIND_PEAK_TICK`, `WIND_QUIET_TICK`, and the "A stage step in flight"
//! section of `ArtPresenter::draw_with_fruit`), `cubarium_render::sprite` (`Sprite::texel`,
//! `Sprite::extent`, `Pose::extent`, `Bend`, `Mask`, `stamp_layers_bent`), `art/PLANTS.md`
//! ("The stage and sway contract", "Pack v5: growth transitions"), `art/README.md` ("Live
//! world", "Authored growth (the pilot)", "Wind") and the Package 2 decisions of
//! `design/7_Research/living-world-next-brief-2026-09-13.md` — never from their bodies.
//!
//! Every expected image is rebuilt here a second, independent way: a presenter drawn from a
//! pack with its plants removed supplies the background, and the step's stamp is hand-built on
//! top through the public API. Where no such image exists the assertion is a property a wrong
//! clip would break: a root that skates, a berry that shows up mid-growth, a pose that narrows
//! the admitted wind, a frame that jumps, a fallback that stopped being the old picture.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::ptr;

use cube_proto::{FACE_SIZE, Face};
use cubarium::art::{ArtPack, Band, Clip, Plant};
use cubarium::art_present::{
    ArtPresenter, GROW_BLEND, GrowthStep, PLANT_REVEAL_PX, REED_DEPTH, REED_SCALE, SOIL_SCALE,
    STAGE_GROW_SECONDS, WIND_PEAK_TICK, WIND_PERIOD, WIND_QUIET_TICK, WIND_RESPONSE,
    WIND_SLOT_VARIATION, band_of, band_opacity, effective_tip, growth_between,
    growth_step, growth_weights, next_stage, plant_bend_budget, plant_cap, plant_phase_of,
    present_seconds, slot_of, slot_wind, species_of, stage_opacity, stage_thresholds, up_of,
    wind_response, wind_strength,
};
use cubarium::clock::DT;
use cubarium::present::PRODUCER_SATURATION;
use cubarium_core::view::RenderView;
use cubarium_render::{Bend, Canvas, Mask, Pose, Sprite, stamp_layers_bent, stamp_pose};
use cubarium_surface::{CELL_COUNT, CellId, Vec2};

// ---------------------------------------------------------------------------
// fixtures (the patterns of `art_growth_clip.rs` and `art_water.rs`, copied so this file
// stands alone)
// ---------------------------------------------------------------------------

const PRODUCER_MAX: f64 = 10.0;
/// Render frames per simulated tick at 60 fps with a 20 Hz clock.
const FRAMES_PER_TICK: u64 = 3;

/// The measured bend budgets of the shipped pack, as `art/README.md` and Package 2
/// decision 6 of the brief record them (px, two decimals). The new art may not *narrow*
/// the wind these admit.
const OLD_BUDGET: [(&str, f64); 7] = [
    ("glowcap", 2.40),
    ("rootveil", 5.43),
    ("lanternstalk", 3.23),
    ("tendrilfan", 0.31),
    ("reedspire", 4.38),
    ("umbrellafrond", 0.33),
    ("bloomcrown", 2.07),
];
/// Half a unit in the last place of the two-decimal table above: the most a *recorded*
/// budget can differ from the measured one without the art having changed at all.
const BUDGET_ROUNDING: f64 = 0.005;

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

// ---------------------------------------------------------------------------
// sites: one cell per (band, species), driven through its own band's field
// ---------------------------------------------------------------------------

/// One cell this file drives, and everything a fixture needs to know about it.
#[derive(Clone, Copy, Debug)]
struct Site {
    cell: CellId,
    band: Band,
    species: &'static str,
}

/// A cell whose 16-px tile stays well inside one face: the anchor is within ±1 px of the
/// cell center and a stamp reaches at most 9 px, so 3..=12 in both cell axes keeps every
/// painted pixel on the cell's own face and the pixel windows below meaningful.
fn interior(cell: CellId) -> bool {
    (3..=12).contains(&cell.cx()) && (3..=12).contains(&cell.cy())
}

/// A rank-2 slot of `species` in `band`, interior to one face.
///
/// Soil and foliage are the cell's own geometry on a side face, so their plants stand up
/// toward the canopy and reveal along the stalk. Canopy is the **top** face, the radial
/// case. Water is made by *flooding* an interior side-face cell (`art_water.rs`'s recipe),
/// which is how a cell's band becomes [`Band::Water`] at all.
fn site_for(band: Band, species: &'static str) -> Site {
    let cell = CellId::all()
        .find(|&c| {
            interior(c)
                && plant_cap(band, c) == Some(2)
                && species_of(band, c) == species
                && match band {
                    Band::Canopy => c.face() == Face::Top && band_of(c) == Band::Canopy,
                    Band::Water => c.face() != Face::Top,
                    other => c.face() != Face::Top && band_of(c) == other,
                }
        })
        .unwrap_or_else(|| {
            panic!("the cube has no interior rank-2 {species} slot in the {} band", band.name())
        });
    Site { cell, band, species }
}

/// Every site this file drives: both plants of every band.
fn sites() -> Vec<Site> {
    let mut out = Vec::new();
    for (band, species) in [
        (Band::Soil, "glowcap"),
        (Band::Soil, "rootveil"),
        (Band::Foliage, "lanternstalk"),
        (Band::Foliage, "tendrilfan"),
        (Band::Canopy, "umbrellafrond"),
        (Band::Canopy, "bloomcrown"),
        (Band::Water, "reedspire"),
    ] {
        out.push(site_for(band, species));
    }
    out
}

/// The site of a plant, by its band and asset name.
fn site_of(plant: &Plant) -> Site {
    let species = WIND_RESPONSE
        .iter()
        .map(|(n, _)| *n)
        .find(|n| *n == plant.name.as_str())
        .unwrap_or_else(|| panic!("{} has no wind response, so no site is defined", plant.name));
    site_for(plant.band, species)
}

/// A density, as a fraction of the band's scale, that warrants exactly `stage` — strictly
/// past that stage's threshold and short of the next one, with room for the hysteresis on
/// the way back down. Asserted against [`next_stage`] so a threshold retune surfaces here
/// rather than as a mysterious image mismatch.
fn density_for(band: Band, stage: u8) -> f64 {
    let th = stage_thresholds(band);
    let t = match stage {
        0 => (th[0] + th[1]) / 2.0,
        1 => (th[1] + th[2]) / 2.0,
        _ => th[2] + 0.15,
    };
    assert_eq!(
        next_stage(None, t, &th, 2),
        Some(stage),
        "{band:?}: density {t} does not warrant stage {stage}"
    );
    assert_eq!(
        next_stage(Some(2), t, &th, 2),
        Some(stage),
        "{band:?}: density {t} does not pull a full-grown plant back to stage {stage}"
    );
    if band == Band::Water {
        assert!(
            t * REED_SCALE > REED_DEPTH,
            "{t} of the reed scale is not deep enough to make the cell a pool"
        );
    }
    t
}

/// A world whose only fed cell is the site's, at `density` of its band's scale — so one
/// plant is the only thing that grows and a flooded site is the only pool.
fn fed_view(tick: u64, site: Site, density: f64) -> RenderView {
    let mut v = bare_view(tick);
    let i = site.cell.index();
    match site.band {
        Band::Soil => v.detritus[i] = density * SOIL_SCALE,
        Band::Foliage | Band::Canopy => v.producer[i] = density * saturation(),
        Band::Water => v.water[i] = density * REED_SCALE,
    }
    v
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
/// flecks, the soil, the water or the ground cover reads a plant's growth.
fn background(v: &RenderView, f: f64) -> Canvas {
    let mut bare = ArtPresenter::new(no_plants());
    bare.observe(v);
    draw(&mut bare, v, f)
}

// ---------------------------------------------------------------------------
// sprite helpers
// ---------------------------------------------------------------------------

fn distinct_frames(clip: &Clip) -> usize {
    clip.frames.iter().map(|s| format!("{s:?}")).collect::<HashSet<_>>().len()
}

/// The alpha plane of a sprite: one premultiplied-linear alpha per texel, row-major.
fn alpha_plane(s: &Sprite) -> Vec<f32> {
    let mut out = Vec::with_capacity(s.width() * s.height());
    for y in 0..s.height() as i32 {
        for x in 0..s.width() as i32 {
            out.push(s.texel(x, y)[3]);
        }
    }
    out
}

/// The premultiplied-linear alpha of texel `i`, row-major, of a 16-wide sprite.
fn alpha_of(s: &Sprite, i: usize) -> f32 {
    s.texel((i % s.width()) as i32, (i / s.width()) as i32)[3]
}

/// How many texels two same-sized sprites' alpha planes differ in, beyond `1e-6`.
fn alpha_texels_differing(a: &Sprite, b: &Sprite) -> usize {
    let (x, y) = (alpha_plane(a), alpha_plane(b));
    assert_eq!(x.len(), y.len(), "two sprites of different sizes");
    (0..x.len()).filter(|&i| (x[i] - y[i]).abs() > 1e-6).count()
}

/// Whether a sprite paints anything in tile row `y`.
fn paints_row(s: &Sprite, y: usize) -> bool {
    (0..s.width() as i32).any(|x| s.texel(x, y as i32)[3] > 0.0)
}

/// The lowest painted tile row of a sprite, `None` for an empty one.
fn lowest_painted_row(s: &Sprite) -> Option<usize> {
    (0..s.height()).rev().find(|&y| paints_row(s, y))
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

/// What the shared breeze does to a site's slot at a presentation instant: the one
/// `(Bend, heading)` every stamp of that slot takes that frame.
fn wind_of(site: Site, budget: f64, seconds: f64) -> (Bend, Vec2) {
    slot_wind(&slot_of(site.cell), site.species, budget, seconds)
}

/// The step the frame at `f` draws, `None` when the frame is idle.
fn drawn_step(p: &ArtPresenter, cell: CellId, f: f64) -> Option<GrowthStep> {
    growth_step(growth_between(p.growth_prev_of(cell), p.growth_of(cell), f))
}

/// Ticks in one wind packet, so a fixture tick can be moved a whole period without leaving
/// the part of the packet it was chosen for.
fn wind_period_ticks() -> u64 {
    let ticks = (WIND_PERIOD / DT).round() as u64;
    assert!(
        (ticks as f64 * DT - WIND_PERIOD).abs() < 1e-9,
        "a wind packet is not a whole number of ticks"
    );
    ticks
}

/// [`WIND_PEAK_TICK`] or [`WIND_QUIET_TICK`] pushed forward by whole wind packets until it
/// leaves `room` ticks of history in front of it, so a step that takes longer to reach can
/// still be drawn at the same place in a packet.
fn fixture_tick(base: u64, room: u64) -> u64 {
    let period = wind_period_ticks();
    let mut tick = base;
    while tick < room {
        tick += period;
    }
    tick
}

/// Observe bare ground, then `observes` ticks `gap` apart with the site fed at `density`,
/// the last of them exactly at `tick`; return the view the presenter has just seen.
///
/// A `gap` of 20 ticks or more is one whole `MAX_STEP_SECONDS` (1 s) of simulated time per
/// observe, so a rising step advances by exactly `1 / STAGE_GROW_SECONDS` each time (an exact
/// binary fraction) while presentation time runs `gap · DT`.
fn drive_to(
    p: &mut ArtPresenter,
    site: Site,
    density: f64,
    gap: u64,
    observes: u64,
    tick: u64,
) -> RenderView {
    assert!(gap * 20 >= 20, "a gap of {gap} ticks is under one MAX_STEP_SECONDS");
    assert!(tick >= observes * gap, "tick {tick} leaves no room for {observes} observes");
    let first = tick - observes * gap;
    p.observe(&bare_view(first));
    let mut last = bare_view(first);
    for k in 1..=observes {
        last = fed_view(first + k * gap, site, density);
        p.observe(&last);
    }
    last
}

/// How many 1 s observes it takes to *complete* every step below `from`: the climb walks one
/// stage at a time and each step is four observes at `1 / STAGE_GROW_SECONDS` a piece, so
/// `None → 0` closes at observe 4, `0 → 1` at 8 and `1 → 2` at 12.
fn observes_before(from: u8) -> u64 {
    4 * (u64::from(from) + 1)
}

/// The documented image of one in-flight step of a site, hand-built on `background`.
///
/// The two branches are the two the doc comment names: an authored clip is **one** stamp of
/// three layers at [`growth_weights`] with [`Mask::None`] and a linearly crossing opacity;
/// every other pair keeps the reveal masks — the lower stage whole at `opacity · (1 − t)` and
/// the upper stage revealed, along the stalk on a side face
/// ([`Mask::Axial`] to `t · PLANT_REVEAL_PX`) and outward from the pivot on the top face
/// ([`Mask::Radial`]). `PLANT_REVEAL_PX` is 16.5 because `Mask::Axial` is
/// [`Mask::None`] at `reveal ≥ height + 0.5` for a 16-row tile; the radial analogue of "the
/// reveal that reaches `Mask::None` exactly at `t = 1`" is therefore `extent + 0.5`.
fn expected_step(
    art: &ArtPack,
    background: &Canvas,
    site: Site,
    density: f64,
    seconds: f64,
    wind: (Bend, Vec2),
    step: GrowthStep,
) -> Canvas {
    let plant = art.plant(site.species).expect(site.species);
    let cell = site.cell;
    let slot = slot_of(cell);
    let (bend, heading) = wind;
    let thresholds = stage_thresholds(site.band);
    let ceiling = band_opacity(site.band);
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
            let mask = match up_of(cell) {
                Some(_) => Mask::Axial { reveal: step.t * PLANT_REVEAL_PX },
                None => Mask::Radial { reveal: step.t * (layers[0].0.extent() + 0.5) },
            };
            stamp_layers_bent(
                &mut canvas,
                slot.at,
                heading,
                &layers,
                1.0,
                opacity_of(step.upper),
                mask,
                bend,
                scratch,
            );
        }
    }
    canvas
}

/// The idle image of one whole stage of a site, hand-built the same way: the stage's own
/// sway pose, whole, with no mask.
fn expected_idle(
    art: &ArtPack,
    background: &Canvas,
    site: Site,
    density: f64,
    seconds: f64,
    wind: (Bend, Vec2),
    stage: u8,
) -> Canvas {
    let plant = art.plant(site.species).expect(site.species);
    let slot = slot_of(site.cell);
    let (bend, heading) = wind;
    let thresholds = stage_thresholds(site.band);
    let opacity = stage_opacity(stage, density, &thresholds, band_opacity(site.band));
    let mut canvas = background.clone();
    stamp_layers_bent(
        &mut canvas,
        slot.at,
        heading,
        &[(idle_pose(plant, stage, site.cell, seconds), 1.0)],
        1.0,
        opacity,
        Mask::None,
        bend,
        &mut Vec::new(),
    );
    canvas
}

// ---------------------------------------------------------------------------
// 1. shape
// ---------------------------------------------------------------------------

/// Every row the pack loaded as a transition is one stage step up, non-looping, the pack's own
/// sample count, long enough to move, and reachable by exactly the pair it declares. A looping
/// transition would snap the plant back to a sprout at the top of every step; a clip found by
/// the wrong pair would play the wrong growth.
#[test]
fn every_authored_transition_is_one_non_looping_step_of_the_packs_own_sample_count() {
    let art = pack();
    let frames = art.plant_frames();
    assert!(frames >= 2, "a pack with {frames} plant samples cannot blend");
    let mut total = 0;
    for plant in &art.plants {
        let pairs: Vec<(u8, u8)> = plant.transitions.iter().map(|t| (t.from, t.to)).collect();
        println!("  {:<14} transitions {pairs:?}", plant.name);
        for (i, transition) in plant.transitions.iter().enumerate() {
            let (from, to) = (transition.from, transition.to);
            let what = format!("{} grow{from}{to}", plant.name);
            assert_eq!(to, from + 1, "{what}: a transition is one stage step up");
            assert!(from < 2, "{what}: no stage above 2 to grow into");
            let clip = &transition.clip;
            assert!(!clip.looping, "{what}: a growth transition never loops");
            assert_eq!(clip.frames.len(), frames, "{what}: samples");
            assert!(clip.seconds > 0.0 && clip.seconds.is_finite(), "{what}: {} s", clip.seconds);
            let distinct = distinct_frames(clip);
            assert!(distinct >= 8, "{what}: only {distinct} distinct frames of {frames}");

            // `Plant::transition` finds exactly this clip, and no other pair finds it.
            let found = plant.transition(from, to).unwrap_or_else(|| panic!("{what}: not found"));
            assert!(ptr::eq(found, clip), "{what}: `transition` returned a different clip");
            assert_eq!(
                pairs.iter().filter(|p| **p == (from, to)).count(),
                1,
                "{what}: two rows claim the same pair"
            );

            // Inclusive sampling: the endpoints are held, nothing wraps, nonsense is the
            // first frame.
            let first = clip.sample(0.0);
            assert!(ptr::eq(first.first, &clip.frames[0]), "{what}: sample(0) is not frame 0");
            assert_eq!(first.mix, 0.0, "{what}: sample(0) must not blend");
            let last = clip.sample(clip.seconds);
            let tail = clip.frames.last().unwrap();
            assert!(ptr::eq(last.first, tail), "{what}: sample(seconds) is not the last frame");
            assert!(ptr::eq(last.second, tail), "{what}: the end wrapped into the start");
            assert_eq!(last.mix, 0.0, "{what}: sample(seconds) must hold the last frame");
            for seconds in [clip.seconds * 1.5, clip.seconds + 1.0] {
                assert!(ptr::eq(clip.sample(seconds).first, tail), "{what}: {seconds} s");
            }
            for seconds in [-1.0, 0.0, f64::NAN] {
                assert!(ptr::eq(clip.sample(seconds).first, &clip.frames[0]), "{what}: {seconds} s");
            }
            assert_eq!(i, pairs.iter().position(|p| *p == (from, to)).unwrap(), "{what}: order");
            total += 1;
        }
        // No pair the plant does not declare resolves to a clip, and a transition never runs
        // downward or skips a stage.
        for (from, to) in [(0u8, 1u8), (1, 2), (0, 2), (1, 0), (2, 1), (0, 0), (2, 3)] {
            assert_eq!(
                plant.transition(from, to).is_some(),
                pairs.contains(&(from, to)),
                "{} {from} → {to}",
                plant.name
            );
        }
    }
    println!("  {total} authored transition(s) over {} plants", art.plants.len());
    assert!(total >= 1, "the pack carries no authored growth at all");
}

// ---------------------------------------------------------------------------
// 2. the endpoint convention (brief decision 2)
// ---------------------------------------------------------------------------

/// **Decision 2.** Frame 0 of every growth clip is the source stage's RESET-neutral pose and
/// the last frame the target's, so the presenter's 12 % edge blends have only a *colour*
/// mismatch to absorb: the silhouette at each end of the step is one the idle stage itself
/// shows. A clip authored from a different silhouette — a sprout an extra pixel tall, a stem
/// that starts above the root, a berry left visible — breaks this and nothing else would
/// catch it.
///
/// The comparison is the **alpha plane** — colour may differ, because a stage's sway pulse is
/// a `self_modulate` colour multiply and a growth clip is baked neutral, while alpha may not,
/// a translucent halo keeping its own alpha under that multiply.
///
/// The reference is *not* the stage's phase-0 sample. The RESET-neutral pose has **every**
/// pivot at rotation 0 at once, and a stage clip whose pivots sway out of phase with each
/// other never samples that pose at all: `art/PLANTS.md` already notes that the pilot's
/// neutral is stage 1's *half-period* sample rather than its phase-0 one, and for `tendrilfan`
/// and `reedspire` no single sample carries it. So the properties asserted are the two that
/// hold whatever the keying:
///
/// * **the stationary material.** A texel whose alpha is the same in every sample of the stage
///   clip belongs to something the sway does not move — a base, a ripple, an unrotated stem.
///   The endpoint's alpha there must equal it exactly. Where *every* texel is stationary (the
///   sway is a pure colour pulse) this is the exact phase-0 equality, and that stricter branch
///   is required to be exercised by at least three species so it cannot quietly evaporate.
/// * **the silhouette envelope.** Every texel painted in *every* sample of the stage clip must
///   be painted by the endpoint, and every texel the endpoint paints must be painted in *some*
///   sample: the endpoint lies inside the stage's own swept silhouette and covers its core. A
///   sprout an extra pixel tall, a stem one row short, a berry left visible — anything outside
///   the stage's sweep — fails.
///
/// And the **join**: both clips of one plant claim to meet on stage 1's neutral pose, so
/// `grow01`'s last frame must be byte-identical to `grow12`'s first and a plant climbing
/// 0 → 1 → 2 shows no jump at the handover. That one is exact and needs no reference sample.
#[test]
fn each_growth_clip_starts_and_ends_on_its_neighbouring_stages_neutral_pose() {
    let art = pack();
    let mut strict: HashSet<String> = HashSet::new();
    for plant in &art.plants {
        for transition in &plant.transitions {
            let (from, to) = (transition.from, transition.to);
            let what = format!("{} grow{from}{to}", plant.name);
            let clip = &transition.clip;
            for (end, frame, stage) in [
                ("first", clip.frames.first().unwrap(), from),
                ("last", clip.frames.last().unwrap(), to),
            ] {
                let stage_clip = &plant.stages[usize::from(stage)];
                let planes: Vec<Vec<f32>> =
                    stage_clip.frames.iter().map(|s| alpha_plane(s)).collect();
                let e = alpha_plane(frame);
                let n = e.len();
                assert!(planes.iter().all(|p| p.len() == n), "{what}: {end} frame is a size apart");

                // (a) The stationary material: same alpha in every sample of the loop.
                let stationary: Vec<usize> = (0..n)
                    .filter(|&i| planes.iter().all(|p| (p[i] - planes[0][i]).abs() <= 1e-6))
                    .collect();
                for &i in &stationary {
                    assert!(
                        (e[i] - planes[0][i]).abs() <= 1e-6,
                        "{what}: the {end} frame's alpha at texel ({}, {}) is {} where every \
                         sample of stage {stage} has {} — that material does not move in the \
                         sway, so the endpoint is not stage {stage}'s neutral pose",
                        i % 16,
                        i / 16,
                        e[i],
                        planes[0][i],
                    );
                }

                // (b) The silhouette envelope: the stage's core is covered and nothing is
                // painted outside its sweep.
                let mut always = 0;
                for i in 0..n {
                    if planes.iter().all(|p| p[i] > 0.0) {
                        always += 1;
                        assert!(
                            e[i] > 0.0,
                            "{what}: the {end} frame leaves texel ({}, {}) empty where every \
                             sample of stage {stage} paints it",
                            i % 16,
                            i / 16,
                        );
                    }
                    if e[i] > 0.0 {
                        assert!(
                            planes.iter().any(|p| p[i] > 0.0),
                            "{what}: the {end} frame paints texel ({}, {}), which no sample of \
                             stage {stage} paints at all — the endpoint is outside the stage's \
                             own silhouette",
                            i % 16,
                            i / 16,
                        );
                    }
                }

                // (c) The strict branch, where the whole alpha plane is stationary.
                let phase0 = alpha_texels_differing(frame, &stage_clip.frames[0]);
                if stationary.len() == n {
                    assert_eq!(
                        phase0, 0,
                        "{what}: stage {stage}'s samples share one alpha plane, so the {end} \
                         frame must equal its phase-0 sample exactly"
                    );
                    strict.insert(plant.name.clone());
                }
                println!(
                    "  {what:<22} {end:5} vs stage{stage}: {} of {n} texels stationary ({}), \
                     {always} always painted, phase-0 residual {phase0}",
                    stationary.len(),
                    if stationary.len() == n { "strict" } else { "swept" },
                );
            }
            // The clip has to have grown *something*: the two endpoints differ in colour or
            // alpha, or the row is not a transition at all.
            assert_ne!(
                format!("{:?}", clip.frames.first().unwrap()),
                format!("{:?}", clip.frames.last().unwrap()),
                "{what}: the plant never grew"
            );
        }
        // The handover. Both clips claim to end and begin on stage 1's neutral pose, so they
        // must be the very same image — a plant that climbs two steps in a row shows no jump.
        if let (Some(up), Some(on)) = (plant.transition(0, 1), plant.transition(1, 2)) {
            assert_eq!(
                format!("{:?}", up.frames.last().unwrap()),
                format!("{:?}", on.frames.first().unwrap()),
                "{}: grow01 ends on a different stage-1 pose than grow12 begins on, so a plant \
                 climbing 0 → 1 → 2 jumps at the handover",
                plant.name
            );
            println!("  {:<14} grow01 and grow12 join on one stage-1 image", plant.name);
        }
        // Decision 4, as far as pixel arithmetic can separate it: the texels only the `fruit`
        // clip ever paints. A growing part legitimately sweeps through positions no stage
        // holds, so this is reported rather than asserted — a mid-clip berry is a job for the
        // 8× strip, not for the atlas.
        if let Some(fruit) = &plant.fruit {
            let stage_painted = |i: usize| {
                plant.stages.iter().any(|c| c.frames.iter().any(|s| alpha_of(s, i) > 0.0))
            };
            let only: Vec<usize> = (0..256)
                .filter(|&i| fruit.frames.iter().all(|s| alpha_of(s, i) > 0.0) && !stage_painted(i))
                .collect();
            let leaks: usize = plant
                .transitions
                .iter()
                .flat_map(|t| t.clip.frames.iter())
                .map(|s| only.iter().filter(|&&i| alpha_of(s, i) > 0.0).count())
                .sum();
            println!(
                "  {:<14} {} fruit-only texel(s), touched by growth frames {leaks} time(s)",
                plant.name,
                only.len()
            );
        }
    }
    let mut names: Vec<&String> = strict.iter().collect();
    names.sort();
    println!("  the strict phase-0 branch was exercised by {names:?}");
    assert!(
        strict.len() >= 3,
        "only {} species have a stage clip whose samples share one alpha plane, so the strict \
         branch of this test has stopped being exercised: {names:?}",
        strict.len()
    );
}

// ---------------------------------------------------------------------------
// 3. roots stay planted (brief decision 3)
// ---------------------------------------------------------------------------

/// **Decision 3.** For every species that stands on the tile's bottom edge — everything but
/// the two radial canopy plants — the lowest painted row of every growth frame lies between
/// the two stages' own lowest rows, row 15 is never painted, and the root moves *at most
/// once*, monotonically toward the target's row. A clip whose foot wandered would make the
/// plant skate on the ground while it grew, and a clip that painted row 15 would grow
/// downward out of its slot (that row also has no bend headroom: the bend is rooted 1.5 px
/// above the tile's bottom edge, i.e. in the middle of row 14).
#[test]
fn a_growth_clip_keeps_its_root_between_the_two_stages_rows_and_moves_it_at_most_once() {
    let art = pack();
    let mut checked = 0;
    for plant in &art.plants {
        for transition in &plant.transitions {
            let (from, to) = (transition.from, transition.to);
            let what = format!("{} grow{from}{to}", plant.name);
            let low_src = lowest_painted_row(&plant.stages[usize::from(from)].frames[0])
                .unwrap_or_else(|| panic!("{}: stage {from} paints nothing", plant.name));
            let low_dst = lowest_painted_row(&plant.stages[usize::from(to)].frames[0])
                .unwrap_or_else(|| panic!("{}: stage {to} paints nothing", plant.name));
            let lows: Vec<usize> = transition
                .clip
                .frames
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    lowest_painted_row(s)
                        .unwrap_or_else(|| panic!("{what}: frame {i} paints nothing at all"))
                })
                .collect();
            println!("  {what:<22} stage roots {low_src} → {low_dst}, clip roots {lows:?}");
            if plant.band == Band::Canopy {
                // A radial plant has no root row: its anchor is the tile **centre**, so the
                // planted-root rule becomes a centred one. Every frame paints the centre
                // pixel (the plant never lifts off its pivot), its painted footprint stays
                // centred on that pixel to within one pixel in each axis (an even-sized
                // piece sliding out, or one of an opposite pair leading the other, can be a
                // pixel ahead; a whole plant drifting off its pivot cannot hide in that), and
                // its painted reach lies between the two stages' own, so nothing shrinks
                // below the sprout or pokes past the target's tips.
                let reach = |s: &Sprite| -> f64 {
                    let mut r: f64 = 0.0;
                    for y in 0..s.height() as i32 {
                        for x in 0..s.width() as i32 {
                            if s.texel(x, y)[3] > 0.0 {
                                r = r.max((f64::from(x) + 0.5 - 8.0).hypot(f64::from(y) + 0.5 - 8.0));
                            }
                        }
                    }
                    r
                };
                let bounds = |s: &Sprite| -> (i32, i32, i32, i32) {
                    let (mut x0, mut x1, mut y0, mut y1) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
                    for y in 0..s.height() as i32 {
                        for x in 0..s.width() as i32 {
                            if s.texel(x, y)[3] > 0.0 {
                                (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
                            }
                        }
                    }
                    (x0, x1, y0, y1)
                };
                let lo = reach(&plant.stages[usize::from(from)].frames[0]);
                let hi = reach(&plant.stages[usize::from(to)].frames[0]);
                assert!(lo < hi, "{what}: stage {to} reaches no further than stage {from}");
                let mut reaches = Vec::with_capacity(lows.len());
                for (i, frame) in transition.clip.frames.iter().enumerate() {
                    assert!(
                        frame.texel(8, 8)[3] > 0.0,
                        "{what}: frame {i} leaves the tile centre unpainted — the plant lifted \
                         off its pivot"
                    );
                    let (x0, x1, y0, y1) = bounds(frame);
                    assert!(
                        (x0 + x1 - 15).abs() <= 1 && (y0 + y1 - 15).abs() <= 1,
                        "{what}: frame {i}'s footprint {x0}..={x1} × {y0}..={y1} is off the tile \
                         centre"
                    );
                    let r = reach(frame);
                    assert!(
                        r >= lo - 1e-9 && r <= hi + 1e-9,
                        "{what}: frame {i} reaches {r} px from the centre, outside the stages' \
                         {lo}..={hi}"
                    );
                    reaches.push(r);
                }
                println!(
                    "  {what:<22} radial: centre painted, footprint centred, reach {lo:.2} → \
                     {hi:.2} px (frames {:.2}..{:.2})",
                    reaches.iter().cloned().fold(f64::INFINITY, f64::min),
                    reaches.iter().cloned().fold(0.0, f64::max)
                );
                checked += 1;
                continue;
            }
            let (lo, hi) = (low_src.min(low_dst), low_src.max(low_dst));
            for (i, &low) in lows.iter().enumerate() {
                assert!(
                    (lo..=hi).contains(&low),
                    "{what}: frame {i}'s lowest painted row is {low}, outside the stages' \
                     {lo}..={hi}"
                );
                assert!(
                    !paints_row(&transition.clip.frames[i], 15),
                    "{what}: frame {i} paints tile row 15, below the plant's root"
                );
            }
            // Monotone toward the target, and it changes at most once.
            let descending = low_dst > low_src;
            for pair in lows.windows(2) {
                if descending {
                    assert!(
                        pair[1] >= pair[0],
                        "{what}: the root rose from {} to {} while the target is lower ({low_dst})",
                        pair[0],
                        pair[1]
                    );
                } else {
                    assert!(
                        pair[1] <= pair[0],
                        "{what}: the root fell from {} to {} while the target is higher \
                         ({low_dst})",
                        pair[0],
                        pair[1]
                    );
                }
            }
            let changes = lows.windows(2).filter(|p| p[0] != p[1]).count();
            assert!(
                changes <= 1,
                "{what}: the root moved {changes} times ({lows:?}); the stages differ by \
                 {} row(s), so it may move once, with the stem's extension",
                hi - lo
            );
            checked += 1;
        }
    }
    println!("  {checked} transition(s) checked (side species by root row, canopy by centre)");
    assert_eq!(checked, art.plants.len() * 2, "every plant's two steps must be checked");
}

// ---------------------------------------------------------------------------
// 4. extents and the wind budgets (brief decision 6)
// ---------------------------------------------------------------------------

/// Every growth frame fits the nine-pixel stamp footprint (the loader guarantees it; a pack
/// built another way would not), and — **decision 6** — no new pose may *narrow* the wind the
/// shipped pack admitted. The assertion is on the **effective tips**, because that is what a
/// viewer sees: a budget that shrank from 3.23 to 3.0 changes nothing for a lanternstalk
/// asking for 0.45, and a budget that shrank from 0.31 to 0.2 halves the tendrilfan's motion.
///
/// Two forms, because the recorded table is rounded to two decimals: a family whose desired
/// tip is *inside* its old headroom must still get exactly what it asks for (an exact
/// comparison — the rounding cannot hide anything there), and a family the pack clamps must
/// not lose more than the table's own half-a-last-place.
#[test]
fn every_growth_frame_fits_the_footprint_and_no_pose_narrows_the_admitted_wind() {
    let art = pack();
    let presenter = ArtPresenter::new(pack());
    for plant in &art.plants {
        for transition in &plant.transitions {
            for (i, frame) in transition.clip.frames.iter().enumerate() {
                let extent = frame.extent();
                assert!(
                    extent > 0.0 && extent <= 9.0,
                    "{} grow{}{}: frame {i} has extent {extent}",
                    plant.name,
                    transition.from,
                    transition.to
                );
            }
        }
    }
    println!("  asset          old budget   new budget   desired   old eff   new eff");
    let mut clamped = Vec::new();
    for plant in &art.plants {
        let old = OLD_BUDGET
            .iter()
            .find(|(n, _)| *n == plant.name.as_str())
            .map(|(_, b)| *b)
            .unwrap_or_else(|| panic!("{}: the brief records no shipped budget", plant.name));
        let new = plant_bend_budget(plant);
        assert_eq!(
            new,
            presenter.bend_budget(&plant.name),
            "{}: the presenter's table must be `plant_bend_budget` of the same plant",
            plant.name
        );
        let desired = wind_response(&plant.name).tip_px;
        let old_eff = effective_tip(desired, old);
        let new_eff = effective_tip(desired, new);
        println!(
            "  {:<14} {old:>10.4}   {new:>10.4}   {desired:>7.2}   {old_eff:>7.4}   {new_eff:>7.4}",
            plant.name
        );
        assert!(
            new >= old - BUDGET_ROUNDING,
            "{}: the measured budget fell from {old} to {new}; a mid-clip pose is wider than \
             the stages, which decision 6 forbids — narrow the pose",
            plant.name
        );
        if old_eff >= desired {
            assert_eq!(
                new_eff, desired,
                "{}: the family asked for {desired} px and the shipped pack gave it; the new \
                 art admits only {new_eff}",
                plant.name
            );
        } else {
            clamped.push(plant.name.clone());
            assert!(
                new_eff >= old_eff - BUDGET_ROUNDING / (1.0 + WIND_SLOT_VARIATION),
                "{}: the admitted tip fell from {old_eff} to {new_eff} px",
                plant.name
            );
        }
    }
    println!("  clamped by the pack's own art: {clamped:?}");
    assert!(!clamped.is_empty(), "no species is bounded by the pack, so the rule is untested");
}

// ---------------------------------------------------------------------------
// 5. continuity at 60 fps
// ---------------------------------------------------------------------------

/// Nothing in any authored step may jump. **The bound is derived, not tuned**, exactly as
/// `art_growth_clip.rs` derives it for the pilot, and re-derived per clip because a clip's own
/// sample spacing and its largest adjacent-sample difference are its own.
///
/// A clip of `n` samples over `seconds` s, non-looping, has its samples `seconds / (n − 1)`
/// apart and [`Clip::sample`] blends between the two bracketing ones. At 60 fps one frame
/// advances the clip by `Δu = (1/60) / (seconds / (n − 1))` of a sample interval and — the
/// stamp being an exact lerp in the sampled pose — moves any pixel by at most `Δu` times the
/// largest adjacent-sample difference `jump`, measured here by stamping the clip's own samples
/// alone over black at this slot's anchor, heading and opacity. Composited source-over a
/// background the same change can be doubled (`out = src + (1 − a)·bg`), hence `2 · Δu ·
/// jump`. Everything else in the window moves too — the two idle sway clips the edges blend
/// with, the wind bend, the ground cover's own breath — measured independently as `sway`, the
/// largest per-frame change a presenter *snapped* to either stage shows over the same frames;
/// an in-flight cell pays for it twice because it draws two layers. So
///
/// ```text
/// bound = 2 · sway + 2 · Δu · jump
/// ```
///
/// A clip stepped once per baked sample would move `jump` in one frame — ten times the `Δu ·
/// jump` term — and one stepped once per tick would move three frames' worth at once.
#[test]
fn every_authored_step_moves_less_than_a_fraction_of_a_baked_sample_per_frame_at_60_fps() {
    let art = plants_only();
    let mut checked = 0;
    for plant in &art.plants {
        if plant.transitions.is_empty() {
            continue;
        }
        let site = site_of(plant);
        let window = near(site.cell);
        let slot = slot_of(site.cell);
        let opacity = band_opacity(site.band);
        for transition in &plant.transitions {
            let (from, to) = (transition.from, transition.to);
            let what = format!("{} grow{from}{to}", plant.name);
            let clip = &transition.clip;
            let n = clip.frames.len();
            let du = (1.0 / 60.0) / (clip.seconds / (n - 1) as f64);

            // `jump`: the largest adjacent-sample difference, stamped alone over black at
            // this slot's own anchor, heading and opacity.
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
            assert!(jump > 0.0, "{what}: the clip does not move at all between samples");

            // The frames the sweep walks: the whole climb to the target stage, at 60 fps.
            // Each step is `STAGE_GROW_SECONDS` and one tick advances it by `DT`, so the
            // step from `from` runs over ticks `80·(from+1)+1 ..= 80·(from+2)`.
            let per_step = (STAGE_GROW_SECONDS / DT).round() as u64;
            let ticks: Vec<u64> = (1..=per_step * (u64::from(from) + 2) + 30).collect();
            let density = density_for(site.band, to);

            // `sway`: the same frames, with the plant snapped idle at each stage it blends
            // with.
            let mut sway = 0.0f32;
            for stage in [from, to] {
                let held = density_for(site.band, stage);
                let mut control = ArtPresenter::new(plants_only());
                control.observe(&fed_view(0, site, held));
                let mut last: Option<Canvas> = None;
                for &tick in &ticks {
                    let v = fed_view(tick, site, held);
                    control.observe(&v);
                    for frame in 0..FRAMES_PER_TICK {
                        let image = draw(&mut control, &v, frame as f64 / FRAMES_PER_TICK as f64);
                        if let Some(last) = &last {
                            sway = sway.max(max_diff_at(&image, last, &window));
                        }
                        last = Some(image);
                    }
                }
            }
            assert!(sway > 0.0, "{what}: the control plant does not move at all");
            let bound = 2.0 * sway + 2.0 * du as f32 * jump;

            let mut p = ArtPresenter::new(plants_only());
            p.observe(&bare_view(0));
            let mut last: Option<Canvas> = None;
            let mut worst = 0.0f32;
            let mut lowest = 1.0f64;
            let mut highest = 0.0f64;
            let mut frames = 0;
            for &tick in &ticks {
                let v = fed_view(tick, site, density);
                p.observe(&v);
                for frame in 0..FRAMES_PER_TICK {
                    let f = frame as f64 / FRAMES_PER_TICK as f64;
                    let step = drawn_step(&p, site.cell, f);
                    let image = draw(&mut p, &v, f);
                    let inside = step.is_some_and(|s| s.lower == Some(from) && s.upper == to);
                    if let (Some(last), true) = (&last, inside) {
                        let moved = max_diff_at(&image, last, &window);
                        worst = worst.max(moved);
                        assert!(
                            moved <= bound,
                            "{what}: tick {tick} frame {frame} (t = {:?}) moved a pixel by \
                             {moved}, over the derived bound {bound} (sway {sway}, sample jump \
                             {jump}, du {du})",
                            step.map(|s| s.t)
                        );
                        lowest = lowest.min(step.unwrap().t);
                        highest = highest.max(step.unwrap().t);
                        frames += 1;
                    }
                    last = Some(image);
                }
            }
            println!(
                "  {what:<22} sway {sway:.5} jump {jump:.5} du {du:.5} bound {bound:.5}; worst \
                 frame moved {worst:.5} over {frames} frames, t {lowest:.4}..{highest:.4}"
            );
            assert!(worst > 0.0, "{what}: the step never moved at all");
            assert!(lowest < 0.02, "{what}: the sweep missed the start (lowest t {lowest})");
            assert!(highest > 0.98, "{what}: the sweep missed the end (highest t {highest})");
            assert!(
                frames > 200,
                "{what}: a {STAGE_GROW_SECONDS} s step at 60 fps is 240 frames, not {frames}"
            );
            checked += 1;
        }
    }
    println!("  {checked} authored step(s) measured");
    assert!(checked >= 1, "no authored step was measured");
}

// ---------------------------------------------------------------------------
// 6. playback
// ---------------------------------------------------------------------------

/// Every authored step is drawn as the documented **three-layer stamp** — the lower stage's
/// running sway clip, the growth clip at `t · clip.seconds`, the upper stage's running sway
/// clip, at [`growth_weights`]`(t)`, unmasked, at the linearly crossing opacity, under the one
/// `(Bend, heading)` [`slot_wind`] gives every other stamp of that slot — and it *enters and
/// leaves on the neighbouring idle stage's own image*.
///
/// Both a windy and a calm instant, because the bend is the one thing a growth stamp shares
/// with the rest of the frame: at [`WIND_PEAK_TICK`] the slot's bend is a real displacement, at
/// [`WIND_QUIET_TICK`] it is exactly the identity, and the hand-built image must match bit for
/// bit either way. The ticks are pushed forward by whole [`WIND_PERIOD`]s where a step needs
/// more history to reach than the base tick leaves.
#[test]
fn the_presenter_plays_every_authored_step_as_the_documented_three_layer_stamp() {
    let art = plants_only();
    let mut checked = 0;
    let mut bent: HashSet<String> = HashSet::new();
    let mut spun: HashSet<String> = HashSet::new();
    for plant in &art.plants {
        if plant.transitions.is_empty() {
            continue;
        }
        let site = site_of(plant);
        let budget = plant_bend_budget(plant);
        for transition in &plant.transitions {
            let (from, to) = (transition.from, transition.to);
            let what = format!("{} grow{from}{to}", plant.name);
            let density = density_for(site.band, to);
            // Two observes into the step, so `t` is nowhere near an edge blend.
            let observes = observes_before(from) + 2;

            for (label, base, windy) in
                [("a windy instant", WIND_PEAK_TICK, true), ("a calm instant", WIND_QUIET_TICK, false)]
            {
                let tick = fixture_tick(base, observes * 20);
                let f = 0.5;
                let mut p = ArtPresenter::new(plants_only());
                let v = drive_to(&mut p, site, density, 20, observes, tick);
                let seconds = present_seconds(v.tick, f);
                let step = drawn_step(&p, site.cell, f)
                    .unwrap_or_else(|| panic!("{what} at {label}: the fixture is not in flight"));
                assert_eq!(
                    (step.lower, step.upper),
                    (Some(from), to),
                    "{what} at {label}: the fixture is in the wrong step"
                );
                assert!(
                    step.t > GROW_BLEND && step.t < 1.0 - GROW_BLEND,
                    "{what} at {label}: t = {} is inside an edge blend, not the clip alone",
                    step.t
                );
                // The fixture really is windy / really is calm.
                let wind = wind_of(site, budget, seconds);
                // A species the pack admits no tip travel to (`rootveil` asks for none;
                // another could be clamped to none by its own footprint) takes the identity
                // path at *every* instant, so there is no windy instant for it to be drawn
                // at. Its images are still compared — against the hand-built `Bend::NONE`
                // stamp, which is the whole of what the breeze does to it.
                let admitted = effective_tip(wind_response(site.species).tip_px, budget);
                if !windy {
                    assert_eq!(
                        wind_strength(seconds),
                        0.0,
                        "{what} at {label}: the packet must be quiet"
                    );
                }
                let radial = site.cell.face() == Face::Top
                    && wind_response(site.species).spin_deg > 0.0;
                if radial {
                    // A radial (top-face) species turns in place: never a bend, and at a
                    // windy instant its heading is spun off the slot's own — that spin is
                    // the whole of what the breeze does to a canopy plant, growing or not.
                    assert_eq!(
                        wind.0,
                        Bend::NONE,
                        "{what} at {label}: a radial species must never bend"
                    );
                    assert_eq!(
                        wind.1 != slot_of(site.cell).heading,
                        windy,
                        "{what} at {label}: the crown's heading is {:?} against the slot's {:?}",
                        wind.1,
                        slot_of(site.cell).heading
                    );
                    if windy {
                        spun.insert(plant.name.clone());
                    }
                } else if admitted == 0.0 {
                    assert_eq!(
                        wind,
                        (Bend::NONE, slot_of(site.cell).heading),
                        "{what} at {label}: a species admitted no tip travel must take the \
                         identity path"
                    );
                } else {
                    assert_eq!(
                        !wind.0.is_identity(),
                        windy,
                        "{what} at {label}: the slot's bend is {:?} at {seconds} s (wind {})",
                        wind.0,
                        wind_strength(seconds)
                    );
                    if windy {
                        assert!(
                            wind.0.amplitude.abs() > 1e-6,
                            "{what} at {label}: the breeze must displace something"
                        );
                        bent.insert(plant.name.clone());
                    }
                }

                let bg = background(&v, f);
                let actual = draw(&mut p, &v, f);
                let expected = expected_step(&art, &bg, site, density, seconds, wind, step);
                assert_same_canvas(&actual, &expected, &format!("{what} at {label}"));
                assert!(
                    max_diff_at(&actual, &bg, &near(site.cell)) > 0.02,
                    "{what} at {label}: the growth stamp paints nothing, so the fixture proves \
                     nothing"
                );
                println!("  {what:<22} {label:<16} t = {:.4}", step.t);
            }

            // Entering and leaving. The weights are a partition whose ends are `[1, 0, 0]`
            // and `[0, 0, 1]`, so the drawn frame there is the neighbouring idle stage's own
            // image; just inside each end the residual is the growth layer's own weight and
            // nothing more. A step that cut to and from the clip would step the sway phase
            // and the sprout's pulse at both ends of every step.
            assert_eq!(growth_weights(0.0), [1.0, 0.0, 0.0], "t = 0 is the lower stage alone");
            assert_eq!(growth_weights(1.0), [0.0, 0.0, 1.0], "t = 1 is the upper stage alone");
            for (label, extra, f, stage, want) in [
                ("entering", 1u64, 0.04, from, GROW_BLEND / 4.0),
                ("leaving", 4, 0.98, to, GROW_BLEND / 8.0),
            ] {
                let observes = observes_before(from) + extra;
                let tick = fixture_tick(WIND_PEAK_TICK, observes * 20);
                let mut p = ArtPresenter::new(plants_only());
                let v = drive_to(&mut p, site, density, 20, observes, tick);
                let seconds = present_seconds(v.tick, f);
                let step = drawn_step(&p, site.cell, f)
                    .unwrap_or_else(|| panic!("{what} {label}: not in flight at f = {f}"));
                assert_eq!((step.lower, step.upper), (Some(from), to), "{what} {label}");
                let edge = if extra == 1 { step.t } else { 1.0 - step.t };
                assert!(edge < want, "{what} {label}: t = {} is not in the edge blend", step.t);
                let bg = background(&v, f);
                let wind = wind_of(site, budget, seconds);
                let idle = expected_idle(&art, &bg, site, density, seconds, wind, stage);
                let actual = draw(&mut p, &v, f);
                let d = max_diff(&actual, &idle);
                let w_grow = f64::from(growth_weights(step.t)[1]);
                assert!(
                    f64::from(d) <= w_grow + 1e-6,
                    "{what} {label}: at t = {} the frame is {d} from the idle stage-{stage} \
                     image, over the growth layer's own weight {w_grow}",
                    step.t
                );
                assert!(
                    d < 0.01,
                    "{what} {label}: at t = {} the frame is still {d} from the idle stage-{stage} \
                     image — the step {label} with a cut",
                    step.t
                );
                println!("  {what:<22} {label:<16} t = {:.4}, {d:.5} from idle", step.t);
            }
            checked += 1;
        }
    }
    assert!(checked >= 1, "no authored step was played");
    let mut names: Vec<&String> = bent.iter().collect();
    names.sort();
    println!("  the windy branch was exercised by {names:?}");
    assert!(
        bent.len() >= 3,
        "only {} species were drawn with a live bend, so the windy branch of this test has \
         stopped being exercised: {names:?}",
        bent.len()
    );
    let mut names: Vec<&String> = spun.iter().collect();
    names.sort();
    println!("  the radial (spun) branch was exercised by {names:?}");
    assert!(
        spun.len() >= 2,
        "only {} species were drawn turning in place mid-step; both canopy species carry \
         clips: {names:?}",
        spun.len()
    );
}

/// A canopy plant whose slot lies on the **rim** of the top face: its 16-px tile crosses the
/// seam onto a side face. An authored step there is still the one documented three-layer
/// stamp — hand-built through the same seam-continuous [`stamp_layers_bent`] — so the
/// crown opens as one body on two faces, every channel stays bounded, and the plant is lit
/// on both faces. Both clips of both radial species, at a windy instant (so the in-place
/// spin is live across the seam too).
#[test]
fn a_canopy_step_on_the_rim_of_the_top_face_opens_as_one_stamp_on_two_faces() {
    let art = plants_only();
    let mut checked = 0;
    for species in ["umbrellafrond", "bloomcrown"] {
        let plant = art.plant(species).expect(species);
        let cell = CellId::all()
            .find(|&c| {
                c.face() == Face::Top
                    && (c.cx() == 0 || c.cx() == 15 || c.cy() == 0 || c.cy() == 15)
                    && band_of(c) == Band::Canopy
                    && plant_cap(Band::Canopy, c) == Some(2)
                    && species_of(Band::Canopy, c) == species
            })
            .unwrap_or_else(|| panic!("the top face's rim has no rank-2 {species} slot"));
        let site = Site { cell, band: Band::Canopy, species };
        let budget = plant_bend_budget(plant);
        for transition in &plant.transitions {
            let (from, to) = (transition.from, transition.to);
            let what = format!("{species} grow{from}{to} at Top cell ({}, {})", cell.cx(), cell.cy());
            let density = density_for(Band::Canopy, to);
            // Three observes into the sprout's step (t = 0.75, the petals out and the ribs
            // long enough to reach a seam two pixels off), two into the wider one.
            let observes = observes_before(from) + if from == 0 { 3 } else { 2 };
            let tick = fixture_tick(WIND_PEAK_TICK, observes * 20);
            let f = 0.5;
            let mut p = ArtPresenter::new(plants_only());
            let v = drive_to(&mut p, site, density, 20, observes, tick);
            let seconds = present_seconds(v.tick, f);
            let step = drawn_step(&p, site.cell, f)
                .unwrap_or_else(|| panic!("{what}: the fixture is not in flight"));
            assert_eq!((step.lower, step.upper), (Some(from), to), "{what}: wrong step");
            assert!(step.t > GROW_BLEND && step.t < 1.0 - GROW_BLEND, "{what}: t = {}", step.t);
            let wind = wind_of(site, budget, seconds);
            assert_eq!(wind.0, Bend::NONE, "{what}: a radial species never bends");
            let bg = background(&v, f);
            let actual = draw(&mut p, &v, f);
            let expected = expected_step(&art, &bg, site, density, seconds, wind, step);
            assert_same_canvas(&actual, &expected, &what);
            let lit: HashSet<Face> = differing(&actual, &bg).into_iter().map(|(f, _, _)| f).collect();
            assert!(
                lit.contains(&Face::Top) && lit.len() >= 2,
                "{what}: the crown on the rim lights {lit:?}, not the top face and a side face"
            );
            for (face, x, y) in every_pixel() {
                let px = actual.get(face, x, y);
                assert!(
                    px.iter().all(|&c| (0.0..=1.0 + 1e-6).contains(&c)),
                    "{what}: pixel {face:?} ({x}, {y}) is {px:?}"
                );
            }
            println!("  {what:<44} t = {:.4}, lit {lit:?}", step.t);
            checked += 1;
        }
    }
    assert_eq!(checked, 4, "both clips of both canopy species must be drawn on the rim");
}

/// Native 64 px frames of both canopy species climbing sprout → stage 1 → stage 2 through
/// their authored clips and then wilting back through them, three frames per tick (60 fps
/// over the 20 Hz clock), on the presenter's own published-view path with the two cells
/// fed and nothing else. Ignored: writes files. `CANOPY_CAPTURE_DIR` names the directory.
#[test]
#[ignore = "review capture, not behaviour"]
fn capture_the_canopy_steps_as_native_frames() {
    use cubarium::sink::{FrameSink, PngSink};
    let dir = std::env::var("CANOPY_CAPTURE_DIR").unwrap_or_else(|_| {
        let d = std::env::temp_dir().join(format!("canopy-growth-{}", std::process::id()));
        d.to_string_lossy().into_owned()
    });
    std::fs::create_dir_all(&dir).unwrap();
    let art = plants_only();
    let sites: Vec<Site> = ["umbrellafrond", "bloomcrown"]
        .iter()
        .map(|s| site_of(art.plant(s).unwrap()))
        .collect();
    let view = |tick: u64, density: f64| {
        let mut v = bare_view(tick);
        for site in &sites {
            v.producer[site.cell.index()] = density * saturation();
        }
        v
    };
    let mut p = ArtPresenter::new(plants_only());
    let mut sink = PngSink::new(&dir, 1).unwrap();
    let mut frame = cube_proto::Frame::black();
    let mut manifest = String::new();
    p.observe(&view(0, 0.0));
    // 12 s up (three 4 s steps), 1 s held full-grown, then 6 s starved back to a sprout.
    let up = density_for(Band::Canopy, 2);
    let down = density_for(Band::Canopy, 0);
    for tick in 1..=400u64 {
        let v = view(tick, if tick <= 260 { up } else { down });
        p.observe(&v);
        for k in 0..FRAMES_PER_TICK {
            let f = k as f64 / FRAMES_PER_TICK as f64;
            let mut canvas = Canvas::new();
            p.draw(&v, f, &mut canvas);
            canvas.encode(&mut frame);
            sink.submit(&frame).unwrap();
            let steps: Vec<String> = sites
                .iter()
                .map(|s| match drawn_step(&p, s.cell, f) {
                    Some(step) => format!("{}:{:?}>{}@{:.3}", s.species, step.lower, step.upper, step.t),
                    None => format!("{}:idle", s.species),
                })
                .collect();
            manifest.push_str(&format!("tick {tick} f {f:.3} {}\n", steps.join(" ")));
        }
    }
    sink.finish().unwrap();
    std::fs::write(format!("{dir}/manifest.txt"), manifest).unwrap();
    let cells: Vec<String> = sites
        .iter()
        .map(|s| format!("{} at Top cell ({}, {}) anchor {:?}", s.species, s.cell.cx(), s.cell.cy(), slot_of(s.cell).at))
        .collect();
    std::fs::write(format!("{dir}/sites.txt"), cells.join("\n") + "\n").unwrap();
    eprintln!("frames in {dir}");
}

// ---------------------------------------------------------------------------
// 7. reversal, and the fruit accent
// ---------------------------------------------------------------------------

/// A plant that starts to wilt retraces the pictures it came through. [`growth_step`] is the
/// rule: `t` is the *upper* stage's progress whichever way the step travels, and the clip is a
/// pure function of `t`. Drawn at the same simulated instant against the same fields, the
/// rising and the falling frame must be the same image — bit for bit, because nothing else
/// differs.
///
/// The progresses are exact binary fractions: an observe a whole `MAX_STEP_SECONDS` (1 s)
/// after the last advances a rising step by exactly `1 / STAGE_GROW_SECONDS = 0.25`, and a
/// half-second observe walks a falling one back by exactly `0.5 / STAGE_WILT_SECONDS = 0.25`,
/// so growing and wilting meet on the very same `t` and the comparison needs no tolerance.
///
/// The wilting view keeps the cell in its own band — a reed starved to bare ground would stop
/// being a reed, which is a band change and a snap, not a reversal.
#[test]
fn wilting_through_an_authored_step_replays_the_growing_pictures_backwards() {
    let art = plants_only();
    let mut checked = 0;
    let mut bent: HashSet<String> = HashSet::new();
    for plant in &art.plants {
        if plant.transitions.is_empty() {
            continue;
        }
        let site = site_of(plant);
        let budget = plant_bend_budget(plant);
        for transition in &plant.transitions {
            let (from, to) = (transition.from, transition.to);
            let what = format!("{} grow{from}{to}", plant.name);
            let up_density = density_for(site.band, to);
            let down_density = density_for(site.band, from);
            let n0 = observes_before(from);

            // The instant every frame below is drawn at: windy, so the bend is live through
            // the reversal too, and the same for both presenters.
            let shown = fed_view(fixture_tick(WIND_PEAK_TICK, 0), site, up_density);
            let f = 1.0;
            let seconds = present_seconds(shown.tick, f);
            let (bend, _) = wind_of(site, budget, seconds);
            if effective_tip(wind_response(site.species).tip_px, budget) == 0.0 {
                // `rootveil` answers the breeze with the identity at every instant; there is
                // no windy instant for it to be drawn at, and the reversal is compared on the
                // identity path instead.
                assert_eq!(bend, Bend::NONE, "{what}: a still species must not bend");
            } else {
                assert!(!bend.is_identity(), "{what}: the fixture instant must be windy");
                bent.insert(plant.name.clone());
            }

            for k in 1..=3u64 {
                let progress = 0.25 * k as f64;
                let mut growing = ArtPresenter::new(plants_only());
                drive_to(&mut growing, site, up_density, 20, n0 + k, (n0 + k) * 20);
                let up = drawn_step(&growing, site.cell, f)
                    .unwrap_or_else(|| panic!("{what}: the growing fixture is not in flight"));
                assert_eq!((up.lower, up.upper), (Some(from), to), "{what}: the rising pair");
                assert!(
                    (up.t - progress).abs() < 1e-15,
                    "{what}: growing t is {} not {progress}",
                    up.t
                );

                let mut wilting = ArtPresenter::new(plants_only());
                let last = (n0 + k + 1) * 20;
                drive_to(&mut wilting, site, up_density, 20, n0 + k + 1, last);
                // Starve it: half a second of the lower stage's own density turns the step
                // round and walks it back by exactly a quarter of the step.
                wilting.observe(&fed_view(last + 10, site, down_density));
                let growth = wilting.growth_of(site.cell);
                assert_eq!(
                    (growth.from, growth.to),
                    (Some(to), Some(from)),
                    "{what}: the starved plant must be wilting, not {growth:?}"
                );
                let down = drawn_step(&wilting, site.cell, f)
                    .unwrap_or_else(|| panic!("{what}: the wilting fixture is not in flight"));
                assert_eq!(
                    (down.lower, down.upper),
                    (up.lower, up.upper),
                    "{what}: the wilting step must be the same pair"
                );
                assert!(
                    (down.t - up.t).abs() < 1e-15,
                    "{what}: progress {progress}: wilting is at t = {}, growing at {}",
                    down.t,
                    up.t
                );

                let a = draw(&mut growing, &shown, f);
                let b = draw(&mut wilting, &shown, f);
                assert_same_canvas(&a, &b, &format!("{what}: wilting vs growing at t = {progress}"));
                assert!(
                    max_diff_at(&a, &background(&shown, f), &near(site.cell)) > 0.02,
                    "{what}: progress {progress}: the fixture draws no plant"
                );
            }
            println!("  {what:<22} replayed backwards at t = 0.25, 0.50, 0.75");
            checked += 1;
        }
    }
    assert!(checked >= 1, "no authored step was reversed");
    let mut names: Vec<&String> = bent.iter().collect();
    names.sort();
    println!("  the windy branch was exercised by {names:?}");
    assert!(
        bent.len() >= 3,
        "only {} species were reversed under a live bend: {names:?}",
        bent.len()
    );
}

/// An **authored** step never reads the `fruit` clip, whichever way it travels: the
/// three-layer stamp has no fruit layer at all and
/// [`cubarium::art_present::advance_growth`] holds the accent at 0 while a plant is in
/// flight. So a full-grown plant bursting with fruit that *wilts into* an authored 1 → 2 step
/// draws it exactly as a world with no fruit field does, every frame of the step.
///
/// The **fallback** (reveal-mask) step is only required to match once the accent the frames
/// interpolate has reached 0. `advance_growth` zeroes [`Growth::fruit`] the tick a step
/// starts, but [`growth_between`] mixes it linearly from the *previous* tick's value, so for
/// the one tick after a full-grown plant in fruit turns round, an unclipped step still blends
/// the accent into its two masked stamps. That is measured and printed here rather than
/// asserted away: an authored clip makes it impossible, and the doc comment's "a growth stamp
/// never reads the `fruit` clip" is exact only on the authored path.
#[test]
fn a_fruiting_plant_that_wilts_into_the_one_to_two_step_never_samples_its_fruit_clip() {
    let art = plants_only();
    let ripe = vec![1.0f64; CELL_COUNT];
    let mut checked = 0;
    for plant in &art.plants {
        if plant.fruit.is_none() {
            continue;
        }
        let site = site_of(plant);
        let authored = plant.transition(1, 2).is_some();
        let full = density_for(site.band, 2);
        let back = density_for(site.band, 1);

        let mut fruiting = ArtPresenter::new(plants_only());
        let mut plain = ArtPresenter::new(plants_only());
        // Snap both to a full-grown plant, then hold it there long enough for the accent to
        // be fully faded in, then starve it one stage.
        for p in [&mut fruiting, &mut plain] {
            p.observe_with_fruit(&fed_view(0, site, full), Some(&ripe));
        }
        for tick in 1..=40u64 {
            let v = fed_view(tick, site, full);
            fruiting.observe_with_fruit(&v, Some(&ripe));
            plain.observe_with_fruit(&v, None);
        }
        assert_eq!(
            fruiting.growth_of(site.cell).fruit,
            1.0,
            "{}: the fixture's full-grown plant never came into fruit",
            plant.name
        );

        // Non-vacuity: while the plant is *idle* and full-grown the accent really is the
        // difference between the two images, so the equality below is about the step in
        // flight and not about a fruit clip that never shows at all.
        {
            let v = fed_view(40, site, full);
            assert!(
                drawn_step(&fruiting, site.cell, 0.0).is_none(),
                "{}: the full-grown fixture must be idle",
                plant.name
            );
            let a = draw_fruit(&mut fruiting, &v, 0.0, Some(&ripe));
            let b = draw_fruit(&mut plain, &v, 0.0, None);
            assert!(
                !differing(&a, &b).is_empty(),
                "{}: the fruit clip changes nothing even on an idle full-grown plant",
                plant.name
            );
        }

        let mut in_flight = 0;
        let mut lingering = 0;
        for tick in 41..=120u64 {
            let v = fed_view(tick, site, back);
            fruiting.observe_with_fruit(&v, Some(&ripe));
            plain.observe_with_fruit(&v, None);
            for frame in 0..FRAMES_PER_TICK {
                let f = frame as f64 / FRAMES_PER_TICK as f64;
                // Only a frame *in flight* is required to match: an idle full-grown plant is
                // supposed to show its accent, which is what the block above checks.
                let Some(step) = drawn_step(&fruiting, site.cell, f) else { continue };
                assert_eq!(
                    fruiting.growth_of(site.cell).fruit,
                    0.0,
                    "{}: a plant in flight kept its fruit accent",
                    plant.name
                );
                // The accent the *frame* interpolates, which is the one the stamp can read.
                let blended = growth_between(
                    fruiting.growth_prev_of(site.cell),
                    fruiting.growth_of(site.cell),
                    f,
                )
                .fruit;
                if step.lower == Some(1) && step.upper == 2 {
                    in_flight += 1;
                }
                if !authored && blended > 0.0 {
                    lingering += 1;
                    continue;
                }
                let a = draw_fruit(&mut fruiting, &v, f, Some(&ripe));
                let b = draw_fruit(&mut plain, &v, f, None);
                assert_same_canvas(
                    &a,
                    &b,
                    &format!(
                        "{}: tick {tick} frame {frame}, step {:?} at t = {} (interpolated accent \
                         {blended}): the fruit field changed a step in flight",
                        plant.name,
                        (step.lower, step.upper),
                        step.t
                    ),
                );
            }
        }
        println!(
            "  {:<14} wilted 2 → 1 over {in_flight} frames (authored clip: {authored}); \
             {lingering} fallback frame(s) skipped while the interpolated accent fell to 0",
            plant.name
        );
        assert!(in_flight > 100, "{}: only {in_flight} frames of the step", plant.name);
        if authored {
            assert_eq!(
                lingering, 0,
                "{}: an authored step must match on every frame, accent or not",
                plant.name
            );
        } else {
            assert!(
                lingering <= FRAMES_PER_TICK as usize,
                "{}: the interpolated accent lingered for {lingering} frames, more than the one \
                 tick `growth_between` needs to mix it to 0",
                plant.name
            );
        }
        checked += 1;
    }
    assert!(checked >= 1, "no fruiting plant was wilted through its 1 → 2 step");
}

// ---------------------------------------------------------------------------
// 8. the fallback
// ---------------------------------------------------------------------------

/// With the transitions cleared — a v1–v4 pack, or any pair the art has no clip for — every
/// step is the reveal mask again, bit for bit: the lower stage fading at `opacity · (1 − t)`
/// under the upper stage revealed along the stalk on a side face and outward from the pivot on
/// the top face. Hand-built for **both** steps of **every** site, so the axial and the radial
/// mask are each covered whatever the pack carries.
#[test]
fn a_pack_without_transitions_draws_the_documented_reveal_masks_in_every_band() {
    let art = without_transitions();
    for site in sites() {
        for from in 0u8..2 {
            let to = from + 1;
            let what = format!("{} {from} → {to}", site.species);
            let density = density_for(site.band, to);
            let observes = observes_before(from) + 2;
            let tick = fixture_tick(WIND_PEAK_TICK, observes * 20);
            let f = 0.5;
            let mut p = ArtPresenter::new(without_transitions());
            let v = drive_to(&mut p, site, density, 20, observes, tick);
            let seconds = present_seconds(v.tick, f);
            let step = drawn_step(&p, site.cell, f)
                .unwrap_or_else(|| panic!("{what}: the fixture is not in flight"));
            assert_eq!((step.lower, step.upper), (Some(from), to), "{what}: wrong step");
            let budget = plant_bend_budget(art.plant(site.species).expect(site.species));
            let bg = background(&v, f);
            let actual = draw(&mut p, &v, f);
            let expected = expected_step(
                &art,
                &bg,
                site,
                density,
                seconds,
                wind_of(site, budget, seconds),
                step,
            );
            let mask = if up_of(site.cell).is_some() { "axial" } else { "radial" };
            assert_same_canvas(&actual, &expected, &format!("{what}: the {mask} reveal"));
            assert!(
                max_diff_at(&actual, &bg, &near(site.cell)) > 0.02,
                "{what}: the reveal paints nothing"
            );
            println!("  {what:<28} {mask} reveal at t = {:.4}", step.t);
        }
    }
}

/// Clearing the transitions restores the old picture for the steps that had a clip and
/// changes **nothing else**: every other step of the same plant — the first appearance out of
/// bare ground, and any pair the pack never authored — draws identically with and without the
/// clips, and the pacing itself does not depend on them.
#[test]
fn clearing_the_transitions_changes_only_the_steps_that_had_a_clip() {
    let art = plants_only();
    let mut checked = 0;
    for plant in &art.plants {
        if plant.transitions.is_empty() {
            continue;
        }
        let authored: Vec<(Option<u8>, u8)> =
            plant.transitions.iter().map(|t| (Some(t.from), t.to)).collect();
        let site = site_of(plant);
        let density = density_for(site.band, 2);
        let mut with = ArtPresenter::new(plants_only());
        let mut without = ArtPresenter::new(without_transitions());
        with.observe(&bare_view(0));
        without.observe(&bare_view(0));
        let mut seen: Vec<(Option<u8>, u8)> = Vec::new();
        let mut differed = 0;
        for tick in 1..=320u64 {
            let v = fed_view(tick, site, density);
            with.observe(&v);
            without.observe(&v);
            assert_eq!(
                with.growth_of(site.cell),
                without.growth_of(site.cell),
                "{}: tick {tick}: the pacing must not depend on the pack's transitions",
                plant.name
            );
            for frame in 0..FRAMES_PER_TICK {
                let f = frame as f64 / FRAMES_PER_TICK as f64;
                let a = draw(&mut with, &v, f);
                let b = draw(&mut without, &v, f);
                let step = drawn_step(&with, site.cell, f);
                let pair = step.map(|s| (s.lower, s.upper));
                if let Some(pair) = pair {
                    if seen.last() != Some(&pair) {
                        seen.push(pair);
                    }
                }
                match pair {
                    Some(pair) if authored.contains(&pair) => {
                        // The two paths necessarily *converge* at the ends of a step — both
                        // are the neighbouring idle stage image there — so only the middle is
                        // required to differ.
                        if (0.2..=0.8).contains(&step.unwrap().t) {
                            assert!(
                                !differing(&a, &b).is_empty(),
                                "{}: tick {tick} frame {frame} at t = {}: the clip step drew the \
                                 fallback picture",
                                plant.name,
                                step.unwrap().t
                            );
                            differed += 1;
                        }
                    }
                    _ => assert_same_canvas(
                        &a,
                        &b,
                        &format!(
                            "{}: tick {tick} frame {frame}, step {pair:?}: clearing the \
                             transitions changed a step that never had one",
                            plant.name
                        ),
                    ),
                }
            }
        }
        assert_eq!(
            seen,
            vec![(None, 0u8), (Some(0), 1), (Some(1), 2)],
            "{}: the sweep must walk all three steps of the climb",
            plant.name
        );
        assert!(
            differed > 100,
            "{}: only {differed} frames of its authored step(s) were compared",
            plant.name
        );
        println!("  {:<14} {differed} mid-step frames differ from the fallback", plant.name);
        checked += 1;
    }
    assert!(checked >= 1, "no plant carries a clip to compare against the fallback");
}

// ---------------------------------------------------------------------------
// 9. the loader
// ---------------------------------------------------------------------------

/// The atlas stays **plant-major** and additive over v4: each plant's rows are contiguous and
/// in one block, its three stage rows come first in stage order, then its optional `fruit`
/// row, and only then its `grow` rows — so a v5 pack differs from a v4 one only by inserted
/// rows and every pre-existing tile keeps its place. Every such row is loaded as a transition,
/// in row order, with the row's own `from`, `to`, `seconds`, `frames` and `loop: false`.
#[test]
fn every_grow_row_sits_after_its_plants_own_rows_and_is_loaded_as_a_transition() {
    let art = pack();
    let meta: serde_json::Value = serde_json::from_reader(
        std::fs::File::open(atelier().join("pack.json")).expect("pack.json"),
    )
    .expect("pack.json must parse");
    assert!(meta["version"].as_u64().unwrap() >= 5, "a v5 pack is needed for growth rows");
    let rows = meta["plants"].as_array().expect("a v2+ pack has a plants array");

    // Contiguous, one block per plant, in the order the loader produced.
    let mut order: Vec<String> = Vec::new();
    for row in rows {
        let name = row["name"].as_str().expect("a plant row has a name").to_string();
        if order.last() != Some(&name) {
            assert!(!order.contains(&name), "{name}'s rows are not contiguous");
            order.push(name);
        }
    }
    assert_eq!(
        order,
        art.plants.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
        "the loaded plants are not the atlas's own plant-major order"
    );

    let mut grow_rows = 0;
    for plant in &art.plants {
        let mine: Vec<&serde_json::Value> =
            rows.iter().filter(|r| r["name"] == plant.name.as_str()).collect();
        let row_of = |r: &serde_json::Value| r["row"].as_u64().expect("a plant row has a row");
        let grows: Vec<&&serde_json::Value> =
            mine.iter().filter(|r| r["stage"] == "grow").collect();
        let others: Vec<&&serde_json::Value> =
            mine.iter().filter(|r| r["stage"] != "grow").collect();

        // Stage 0, 1, 2, then `fruit` — the v4 block, unchanged.
        let kinds: Vec<String> = others.iter().map(|r| r["stage"].to_string()).collect();
        let want: Vec<String> = ["0", "1", "2"]
            .iter()
            .map(|s| (*s).to_string())
            .chain(plant.fruit.is_some().then(|| "\"fruit\"".to_string()))
            .collect();
        assert_eq!(kinds, want, "{}: the v4 rows are out of order", plant.name);

        let last_other = others.iter().map(|r| row_of(r)).max().expect("three stage rows");
        for g in &grows {
            assert!(
                row_of(g) > last_other,
                "{}: growth row {} comes before one of its stage/fruit rows (row {last_other})",
                plant.name,
                row_of(g)
            );
        }

        assert_eq!(
            plant.transitions.iter().map(|t| (t.from, t.to)).collect::<Vec<_>>(),
            grows
                .iter()
                .map(|r| (
                    r["from"].as_u64().expect("a growth row has `from`") as u8,
                    r["to"].as_u64().expect("a growth row has `to`") as u8,
                ))
                .collect::<Vec<_>>(),
            "{}: the loaded transitions are not the atlas's growth rows in row order",
            plant.name
        );
        for (transition, row) in plant.transitions.iter().zip(&grows) {
            let what = format!("{} grow{}{}", plant.name, transition.from, transition.to);
            assert_eq!(row["loop"].as_bool(), Some(false), "{what}: must be marked loop: false");
            assert_eq!(row["band"], plant.band.name(), "{what}: band");
            assert_eq!(transition.clip.seconds, row["seconds"].as_f64().unwrap(), "{what}: seconds");
            assert_eq!(
                transition.clip.frames.len(),
                row["frames"].as_u64().unwrap() as usize,
                "{what}: frames"
            );
            grow_rows += 1;
        }
    }
    let declared = rows.iter().filter(|r| r["stage"] == "grow").count();
    assert_eq!(grow_rows, declared, "{declared} growth rows in the atlas, {grow_rows} loaded");
    println!("  {grow_rows} growth row(s) of {} plant rows loaded", rows.len());
}
