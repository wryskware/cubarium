//! Independent tests for the shared breeze of the art image: the global wind sampler, the
//! seam-compatible chart field, the measured amplitude budgets, and what the presenter does
//! with them.
//!
//! Written from the public doc comments of `cubarium::art_present`'s Wind section and of
//! `cubarium_render::{Bend, Mask, Sprite::bend_headroom, stamp_layers_bent}` — never from their
//! bodies. Every normative formula used as an expectation (the Hermite smoothstep, the chart
//! field, the packet envelope's slope bound, the column's row ownership) is recomputed here
//! from the doc comment that states it, and every image expectation is rebuilt a second way
//! through the public API: a presenter drawn from a pack with the plants removed supplies the
//! background, and the layer under test is hand-stamped over it.
//!
//! This file also carries the two sweeps the renderer's own `tests/bend.rs` cannot host,
//! because they need the shipped pack and the presenter's documented column geometry:
//! the per-asset footprint sweep and the synthetic striped column.

use std::path::{Path, PathBuf};

use cube_proto::{FACE_SIZE, Face};
use cubarium::art::{ArtPack, Band, Clip, TallPlant};
use cubarium::art_present::{
    ArtPresenter, Growth, MOTIF_OPACITY, PLANT_BEND_LENGTH, PLANT_BEND_ROOT, SOIL_SCALE,
    TALL_BEND_LENGTH, TALL_BEND_ROOT, TALL_FIRST_JOIN, TALL_JOIN, TALL_MAX_SEGMENTS, TALL_OPACITY,
    TALL_PLANTS, TALL_STRIP_FLOOR, TALL_STRIP_TOP, TALL_VINE_FLOOR, TALL_VINE_TOP, TILE_ROWS,
    TallColumn, VINE_PLANT,
    WIND_CHART_MAX, WIND_FALL, WIND_FLUTTER, WIND_FLUTTER_SECONDS, WIND_HOLD, WIND_PEAK_SECONDS,
    WIND_PEAK_VARY, WIND_PERIOD, WIND_QUIET_SECONDS, WIND_RISE, WIND_SLOT_VARIATION,
    WIND_QUIET_TICK, WIND_TRAVEL_SECONDS, band_of, canopy_heading, effective_tip, placement_of, plant_bend,
    plant_bend_budget, plant_cap, plant_phase_of, present_seconds, slot_of, slot_wind,
    species_of, tall_amplitude, tall_anchor_at, tall_bend_base, tall_bend_budget, tall_columns,
    tall_grown_px, tall_heading, tall_wind_of, trunk_strip, wind_at, wind_chart, wind_phase,
    wind_response, wind_strength,
};
use cubarium::clock::DT;
use cubarium::present::PRODUCER_SATURATION;
use cubarium_core::OrganismId;
use cubarium_core::organism::Mode;
use cubarium_core::view::{OrganismView, RenderView};
use cubarium_render::{
    Bend, Canvas, Mask, Pose, Sprite, srgb_decode, stamp_layers_bent,
    stamp_layers_bent_with_radius,
};
use cubarium_surface::{CELL_COUNT, CellId, SurfacePoint, Vec2, travel};

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

const PRODUCER_MAX: f64 = 10.0;

fn atelier() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")
}

fn pack() -> ArtPack {
    ArtPack::load(&atelier()).expect("the baked pack at assets/atelier must load")
}

/// The pack with whole layers removed, so a presenter can supply a background that contains
/// everything *except* the layer under test.
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

fn organism(id: OrganismId, at: SurfacePoint) -> OrganismView {
    OrganismView {
        id,
        pos: at,
        heading: Vec2::new(1.0, 0.0),
        lobes: vec![(0.0, 0.0, 2.0)],
        hue: 0.5,
        mode: Mode::Resting,
        fed: false,
        juvenile: false,
        gestation: None,
        form: u8::MAX,
        moved: Vec::new(),
    }
}

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
    max_diff_at(a, b, &every_pixel().collect::<Vec<_>>())
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

/// A presenter freshly snapped onto `v`, so nothing is mid-growth and only the sway, the
/// shimmer and the wind still move.
fn snapped(art: ArtPack, v: &RenderView) -> ArtPresenter {
    let mut p = ArtPresenter::new(art);
    p.observe(v);
    p
}

/// The tick whose frame at `f = 0` shows presentation second `seconds`:
/// `present_seconds(tick, 0) = (tick − 1) · DT`.
fn tick_at(seconds: f64) -> u64 {
    let tick = (seconds / DT).round() as u64 + 1;
    assert!(
        (present_seconds(tick, 0.0) - seconds).abs() < 1e-9,
        "{seconds} s is not a whole tick"
    );
    tick
}

/// One 16×16 stamp, the way every plant and column tile is drawn.
fn stamp(
    canvas: &mut Canvas,
    sprite: &Sprite,
    anchor: SurfacePoint,
    heading: Vec2,
    opacity: f32,
    mask: Mask,
    bend: Bend,
) {
    stamp_layers_bent(
        canvas,
        anchor,
        heading,
        &[(Pose::still(sprite), 1.0)],
        1.0,
        opacity,
        mask,
        bend,
        &mut Vec::new(),
    );
}

// ---------------------------------------------------------------------------
// the normative formulas, recomputed here from the doc comments
// ---------------------------------------------------------------------------

/// The seconds a packet is doing anything at all; the rest of the period is exact calm.
fn active_seconds() -> f64 {
    WIND_RISE + WIND_HOLD + WIND_FALL
}

/// The documented bend profile, `smoothstep(clamp((H − root) / length, 0, 1))` with the
/// Hermite `t²(3 − 2t)`.
fn profile(root: f64, length: f64, h: f64) -> f64 {
    if length <= 0.0 || length.is_nan() {
        return 0.0;
    }
    let t = ((h - root) / length).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// [`wind_chart`], recomputed from its doc comment: `a = u/32 − 1`, `b = v/32 − 1`, side faces
/// carry `(−(1 − a²), 0)` and Top carries `(−b(1 − a²), a(1 − b²))`.
fn chart_field(face: Face, u: f64, v: f64) -> Vec2 {
    let (a, b) = (u / 32.0 - 1.0, v / 32.0 - 1.0);
    if face == Face::Top {
        Vec2::new(-b * (1.0 - a * a), a * (1.0 - b * b))
    } else {
        Vec2::new(-(1.0 - a * a), 0.0)
    }
}

// ---------------------------------------------------------------------------
// 1. the global sampler
// ---------------------------------------------------------------------------

/// **The global sampler** ([`wind_strength`]), not the delayed per-root one. Its whole contract
/// in one sweep at 1 ms: inside `[0, 1]`, **exactly** 0 through the quiet interval (so the cube
/// draws the windless image bit for bit and at the windless cost), never a step at any of the
/// four packet boundaries — the flutter is multiplied through the whole active interval, so a
/// piecewise implementation that only fluttered the hold would jump twice — zero slope at both
/// outer edges, its maximum only inside a hold, and 0 for a non-finite time.
#[test]
fn the_shared_gust_stays_in_range_rests_exactly_and_never_jumps_at_a_boundary() {
    // A packet that never rested would have a negative quiet interval, which the sampler
    // would clip rather than honour, and this whole suite would be measuring the wrong thing.
    assert!(
        WIND_QUIET_SECONDS > 0.0 && WIND_QUIET_SECONDS == WIND_PERIOD - active_seconds(),
        "the tuned constants leave {WIND_QUIET_SECONDS} s of calm"
    );

    // A bound on |d strength / dt| from the three documented factors: the envelope's
    // smoothstep (max slope 1.5 over its edge), the flutter (depth × π / its period) and the
    // slow peak modulation (likewise). Each factor is in [0, 1], so the product rule makes the
    // sum of their slopes a bound, and the mean value theorem turns it into a step bound.
    let slope = 1.5 / WIND_RISE.min(WIND_FALL)
        + WIND_FLUTTER * std::f64::consts::PI / WIND_FLUTTER_SECONDS
        + WIND_PEAK_VARY * std::f64::consts::PI / WIND_PEAK_SECONDS;
    let step = 0.001;

    let mut previous = wind_strength(0.0);
    let mut best = (f64::NAN, -1.0f64);
    let samples = 1_000_000;
    for i in 1..=samples {
        let seconds = f64::from(i) * step;
        let s = wind_strength(seconds);
        assert!(
            (0.0..=1.0).contains(&s),
            "the gust left [0, 1] at {seconds}: {s} — every measured budget is sized for 1"
        );
        assert!(
            (s - previous).abs() <= slope * step + 1e-12,
            "the gust stepped by {} at {seconds}, over the {} its own factors allow",
            (s - previous).abs(),
            slope * step
        );
        let u = seconds % WIND_PERIOD;
        if u >= active_seconds() {
            assert_eq!(s, 0.0, "the quiet interval is not exactly quiet at {seconds} (u = {u})");
        }
        if s > best.1 {
            best = (u, s);
        }
        previous = s;
    }
    assert!(best.1 > 0.99, "the gust never reached full strength: {}", best.1);
    assert!(
        best.0 >= WIND_RISE && best.0 <= WIND_RISE + WIND_HOLD,
        "the gust peaked at u = {}, outside the hold",
        best.0
    );

    // Value *and* slope are 0 at both outer edges: a millisecond into the packet the smoothstep
    // has moved by about (0.001/rise)² · 3, which is a ten-millionth, and a millisecond before
    // the end likewise. A linear ramp would already be at 2e-4 there.
    for packet in 0..4 {
        for edge in [0.0, active_seconds()] {
            let t = f64::from(packet) * WIND_PERIOD + edge;
            for side in [-step, step] {
                let s = wind_strength(t + side);
                assert!(
                    s < 1e-6,
                    "the packet edge at {t} has a slope: {s} a millisecond away ({side})"
                );
            }
        }
    }

    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(wind_strength(bad), 0.0, "a non-finite time must be calm");
    }
}

// ---------------------------------------------------------------------------
// 2. the chart field joins across the seams
// ---------------------------------------------------------------------------

/// The claim the field exists for: it "joins across every seam under the real tangent
/// transport". This checks it in *three dimensions* — each side of the seam embeds its own
/// chart vector with [`SurfacePoint::embed_tangent`] and the two must be the same vector of the
/// cube — using [`travel`] only to find the same surface point in the neighbouring chart. A
/// naive 3D swirl projected face by face fails here on Top, where it keeps an edge-normal
/// component the adjacent side face does not have.
#[test]
fn the_chart_breeze_agrees_in_three_dimensions_on_both_sides_of_every_seam() {
    let epsilon = 1e-6;
    let mut seams = 0;
    for face in Face::ALL {
        for edge in cube_proto::Edge::ALL {
            // The open bottom rim is a reflection boundary, not a seam between two charts.
            if face != Face::Top && edge == cube_proto::Edge::Bottom {
                continue;
            }
            for along in [4.0, 17.0, 32.0, 48.5, 60.0] {
                let (u, v, delta) = match edge {
                    cube_proto::Edge::Top => (along, epsilon, Vec2::new(0.0, -2.0 * epsilon)),
                    cube_proto::Edge::Right => {
                        (64.0 - epsilon, along, Vec2::new(2.0 * epsilon, 0.0))
                    }
                    cube_proto::Edge::Bottom => {
                        (along, 64.0 - epsilon, Vec2::new(0.0, 2.0 * epsilon))
                    }
                    cube_proto::Edge::Left => (epsilon, along, Vec2::new(-2.0 * epsilon, 0.0)),
                };
                let here = SurfacePoint::new(face, u, v);
                let crossed = travel(here, delta);
                assert_eq!(crossed.crossings, 1, "the fixture must cross a real seam");
                assert_eq!(crossed.reflections, 0);
                let there = crossed.end;
                let mine = here.embed_tangent(wind_chart(face, u, v));
                let theirs = there.embed_tangent(wind_chart(there.face, there.u, there.v));
                for axis in 0..3 {
                    assert!(
                        (mine[axis] - theirs[axis]).abs() < 1e-5,
                        "the breeze is a different vector of the cube either side of \
                         {face:?}/{edge:?} at {along}: {mine:?} vs {theirs:?}"
                    );
                }
                // And the field really is this polynomial, recomputed from the doc comment.
                let want = chart_field(face, u, v);
                assert!(
                    (wind_chart(face, u, v) - want).length() < 1e-12,
                    "{face:?} at ({u}, {v}) is not the documented field"
                );
                seams += 1;
            }
        }
    }
    assert!(seams >= 60, "only {seams} seam samples");
}

/// The field's deliberate calm regions, exactly zero so that there is nothing for two charts
/// to disagree about, and its magnitude bound — which is what makes [`WIND_CHART_MAX`] the
/// divisor a budget may rely on. A normalized field, or one with a per-face phase, breaks all
/// of this.
#[test]
fn the_chart_breeze_is_exactly_calm_at_the_side_seams_the_top_vertices_and_the_top_centre() {
    for face in Face::ALL {
        if face == Face::Top {
            continue;
        }
        for v in [0.0, 13.0, 32.0, 47.5, 64.0] {
            for u in [0.0, 64.0] {
                assert_eq!(
                    wind_chart(face, u, v),
                    Vec2::ZERO,
                    "{face:?} at the side/side seam u = {u} is not calm"
                );
            }
        }
    }
    for u in [0.0, 64.0] {
        for v in [0.0, 64.0] {
            assert_eq!(
                wind_chart(Face::Top, u, v),
                Vec2::ZERO,
                "the Top vertex ({u}, {v}) is not calm"
            );
        }
    }
    assert_eq!(wind_chart(Face::Top, 32.0, 32.0), Vec2::ZERO, "Top's centre is not calm");

    // |W| ≤ WIND_CHART_MAX everywhere, and it is reached (so the constant is not slack).
    let mut most = 0.0f64;
    for face in Face::ALL {
        for i in 0..=128 {
            for j in 0..=128 {
                let (u, v) = (f64::from(i) * 0.5, f64::from(j) * 0.5);
                let w = wind_chart(face, u, v).length();
                assert!(w <= WIND_CHART_MAX + 1e-12, "{face:?} at ({u}, {v}) blows {w}");
                most = most.max(w);
            }
        }
    }
    assert!((most - WIND_CHART_MAX).abs() < 1e-12, "the largest chart wind is {most}");

    for bad in [f64::NAN, f64::INFINITY] {
        assert_eq!(wind_chart(Face::Front, bad, 32.0), Vec2::ZERO);
        assert_eq!(wind_chart(Face::Top, 32.0, bad), Vec2::ZERO);
    }
}

// ---------------------------------------------------------------------------
// 3. the delayed per-root sampler
// ---------------------------------------------------------------------------

/// **The delayed per-root sampler** ([`wind_at`]), whose quiet interval each point enters and
/// leaves a little early or late: the spatial phase is in `[−1, 1]` and the lag at most the
/// largest in the species table, so a shared quiet interval of
/// `WIND_QUIET_SECONDS − WIND_TRAVEL_SECONDS − max lag` remains, and through it every point of
/// the cube is *exactly* calm. Tested on a grid of every face, every lag in the table, and one
/// with the phase's own extremes.
#[test]
fn the_delayed_per_root_breeze_is_exactly_zero_through_the_shared_quiet_interval() {
    let lags: Vec<f64> = cubarium::art_present::WIND_RESPONSE
        .iter()
        .map(|(_, r)| r.lag_seconds)
        .collect();
    let worst_lag = lags.iter().cloned().fold(0.0f64, f64::max);
    assert!(worst_lag > 0.0 && worst_lag <= 0.5, "the table's lags are {lags:?}");
    let margin = WIND_TRAVEL_SECONDS + worst_lag;
    assert!(
        WIND_QUIET_SECONDS > 2.0 * margin,
        "the travel and lag leave no shared quiet interval"
    );

    let points: Vec<SurfacePoint> = Face::ALL
        .into_iter()
        .flat_map(|face| {
            [1.5f64, 16.0, 32.0, 47.5, 62.5].into_iter().flat_map(move |u| {
                [1.5f64, 16.0, 32.0, 47.5, 62.5].into_iter().map(move |v| {
                    SurfacePoint::new(face, u, v)
                })
            })
        })
        .collect();
    for point in &points {
        assert!(wind_phase(*point).abs() <= 1.0 + 1e-12, "the spatial phase left [−1, 1]");
    }

    for packet in 0..4 {
        let quiet = f64::from(packet) * WIND_PERIOD + active_seconds();
        // The travel term and the lag shift each point's own quiet interval by up to `margin`
        // either way, so the *shared* one is the documented interval less that at both ends.
        let mut seconds = quiet + margin;
        let mut samples = 0;
        while seconds < quiet + WIND_QUIET_SECONDS - margin {
            for point in &points {
                for &lag in &lags {
                    assert_eq!(
                        wind_at(*point, seconds, lag),
                        Vec2::ZERO,
                        "{point:?} still feels the breeze at {seconds} with lag {lag}"
                    );
                }
            }
            samples += 1;
            seconds += 0.25;
        }
        assert!(samples > 20, "only {samples} quiet samples in packet {packet}");
    }

    // Outside a quiet interval the breeze is the chart field scaled by a factor in [0, 1]: the
    // direction is the chart's, never reversed, and the magnitude never exceeds the bound the
    // budgets rely on.
    let mut moved = false;
    for packet in 0..3 {
        for offset in [0.5, 2.0, 6.0, 9.0, 12.5, 15.0, 17.0] {
            let seconds = f64::from(packet) * WIND_PERIOD + offset;
            for point in &points {
                let w = wind_at(*point, seconds, 0.1);
                let chart = wind_chart(point.face, point.u, point.v);
                assert!(w.length() <= WIND_CHART_MAX + 1e-12);
                if chart.length() > 1e-9 {
                    let scale = w.length() / chart.length();
                    assert!((0.0..=1.0 + 1e-12).contains(&scale));
                    assert!(
                        (w - chart * scale).length() < 1e-9,
                        "the breeze at {point:?} is not the chart field scaled: {w:?} vs {chart:?}"
                    );
                    if scale > 0.2 {
                        moved = true;
                    }
                }
            }
        }
    }
    assert!(moved, "the sweep never found any wind at all");
}

// ---------------------------------------------------------------------------
// 4. budgets
// ---------------------------------------------------------------------------

/// "The budget is divided by the largest variation a slot can draw, so that
/// `effective_tip(..) · variation ≤ budget` for **every** variation the hash can produce" — the
/// nine-pixel footprint is a hard bound and the slot that happened to hash a `+10 %` must not
/// be the one that clips. With the documented edge cases: a non-finite tip and a `NaN` budget
/// are 0, and an infinite budget leaves the desired tip alone.
#[test]
fn a_familys_admitted_tip_leaves_room_for_the_largest_slot_variation() {
    let most = 1.0 + WIND_SLOT_VARIATION;
    for budget in [0.0, 0.01, 0.05, 0.2, 0.55, 0.62, 3.8, 7.7] {
        for tip in [0.0, 0.12, 0.45, 0.5, 0.7, 0.9, 12.0] {
            let admitted = effective_tip(tip, budget);
            assert!(admitted >= 0.0, "an admitted tip is never negative: {admitted}");
            assert!(admitted <= tip + 1e-12, "{admitted} is more than the desired {tip}");
            assert!(
                admitted * most <= budget + 1e-12,
                "tip {tip} on budget {budget} admits {admitted}, which a +{}% slot turns into {}",
                WIND_SLOT_VARIATION * 100.0,
                admitted * most
            );
            assert!(
                (admitted - tip.min(budget / most)).abs() < 1e-12,
                "the admitted tip is not min(tip, budget / (1 + variation))"
            );
        }
        assert_eq!(effective_tip(f64::NAN, budget), 0.0);
        assert_eq!(effective_tip(f64::INFINITY, budget), 0.0);
    }
    for tip in [0.0, 0.45, 0.9] {
        assert_eq!(effective_tip(tip, f64::INFINITY), tip, "nothing measured bounds this asset");
        assert_eq!(effective_tip(tip, f64::NAN), 0.0);
        assert_eq!(effective_tip(tip, -1.0), 0.0);
    }
}

/// The measured table, recomputed here from [`Sprite::bend_headroom`] over exactly the clip sets
/// and bases the doc comments name: every stage, the fruit clip and every authored growth
/// transition at the small-plant shape; a column's base at `tall_bend_base(0)`, its trunk and
/// vine at the highest integer tile and its **cap** (never the raw crown) at the highest
/// continuous one. A budget measured from the pose on screen instead of the whole family would
/// disagree here the moment a plant came into fruit or a growth clip reached its widest.
#[test]
fn every_measured_budget_matches_an_independent_sweep_of_bend_headroom() {
    let art = pack();
    let presenter = ArtPresenter::new(pack());

    let over = |clip: &Clip, base: f64| {
        clip.frames
            .iter()
            .map(|f| f.bend_headroom(PLANT_BEND_ROOT, PLANT_BEND_LENGTH, base))
            .fold(f64::INFINITY, f64::min)
    };
    for plant in &art.plants {
        let mut want = f64::INFINITY;
        for stage in &plant.stages {
            want = want.min(over(stage, 0.0));
        }
        if let Some(fruit) = &plant.fruit {
            want = want.min(over(fruit, 0.0));
        }
        for transition in &plant.transitions {
            want = want.min(over(&transition.clip, 0.0));
        }
        assert_eq!(
            plant_bend_budget(plant),
            want,
            "{}'s budget is not the smallest headroom of its whole family",
            plant.name
        );
        assert_eq!(presenter.bend_budget(&plant.name), want, "{} is not in the table", plant.name);
        assert!(want > 0.0, "{} cannot move at all", plant.name);
    }

    let column_over = |clip: &Clip, base: f64| {
        clip.frames
            .iter()
            .map(|f| f.bend_headroom(TALL_BEND_ROOT, TALL_BEND_LENGTH, base))
            .fold(f64::INFINITY, f64::min)
    };
    let trunk_base = tall_bend_base(f64::from(TALL_MAX_SEGMENTS));
    let cap_base = tall_bend_base(f64::from(TALL_MAX_SEGMENTS) + 1.0);
    assert_eq!(tall_bend_base(0.0), -8.0, "a base tile's bottom edge is 8 px below the horizon");
    assert_eq!(trunk_base, 4.0 * f64::from(TALL_MAX_SEGMENTS) - 8.0);
    for plant in &art.tall {
        // Opted-in vines render a derived pair, not the original authored trunk.
        // Retain the independent raw-clip check too by explicitly testing legacy mode.
        let mut want = if let Some(vine) = &plant.vine_strips {
            column_over(&vine.trunk, trunk_base).min(column_over(&vine.endpoint, cap_base))
        } else { column_over(&plant.trunk, trunk_base) };
        if let Some(base) = &plant.base {
            want = want.min(column_over(base, tall_bend_base(0.0)));
        }
        if let Some(cap) = &plant.cap {
            want = want.min(column_over(cap, cap_base));
        }
        assert_eq!(
            tall_bend_budget(plant),
            want,
            "{}'s column budget is not the smallest headroom of its rendered parts",
            plant.name
        );
        assert_eq!(presenter.bend_budget(&plant.name), want, "{} is not in the table", plant.name);
        assert!(want > 0.0, "{} cannot move at all", plant.name);
    }
    let mut legacy = pack();
    let vine = legacy.tall.iter_mut().find(|p| p.name == VINE_PLANT).unwrap();
    assert!(vine.vine_strips.take().is_some());
    assert_eq!(tall_bend_budget(vine), column_over(&vine.trunk, trunk_base));

    // The table covers the whole pack and nothing else, and an unmeasured name stands still.
    assert_eq!(presenter.bend_budgets().len(), art.plants.len() + art.tall.len());
    assert_eq!(presenter.bend_budget("no such plant"), 0.0);

    // A column shares one budget with its vine, because both bend by the same amplitude.
    let vine = presenter.bend_budget(VINE_PLANT);
    for (pick, name) in TALL_PLANTS.iter().enumerate() {
        let own = presenter.bend_budget(name);
        let bare = TallColumn { face: Face::Front, cx: 3, pick, vine: false };
        let vined = TallColumn { face: Face::Front, cx: 3, pick, vine: true };
        assert_eq!(presenter.column_budget(&bare), own);
        assert_eq!(presenter.column_budget(&vined), own.min(vine));
    }
}

// ---------------------------------------------------------------------------
// 5. the footprint, on the shipped art
// ---------------------------------------------------------------------------

/// The sweep the nine-pixel bound needs: every frame of every shipped plant and column part,
/// at `±` its own family budget, at a chart-interior anchor, a seam anchor and a vertex anchor,
/// drawn at the footprint the renderer computes for itself and again at a deliberately larger
/// radius. The two images must be identical — nothing the stamp would have painted lay outside
/// the footprint — and the stamp must still be there. A clipped filter tail is invisible
/// otherwise: it is a texel that quietly stops being drawn when the wind blows.
#[test]
fn every_shipped_plant_and_column_frame_draws_completely_at_its_own_budget() {
    let art = pack();
    let anchors = [
        ("a chart interior", SurfacePoint::new(Face::Front, 32.25, 40.25)),
        ("a side seam", SurfacePoint::new(Face::Front, 63.5, 32.5)),
        ("a top vertex", SurfacePoint::new(Face::Top, 0.25, 0.25)),
    ];
    // Every sixth baked sample of each 24-frame clip, which is a different pose of the sway.
    const STRIDE: usize = 6;

    let mut checked = 0;
    let mut sweep = |name: &str, clip: &Clip, budget: f64, root: f64, length: f64, base: f64| {
        let admitted = if budget.is_finite() {
            budget
        } else {
            effective_tip(wind_response(name).tip_px, budget) * (1.0 + WIND_SLOT_VARIATION)
        };
        for frame in clip.frames.iter().step_by(STRIDE) {
            for sign in [-1.0, 1.0] {
                let bend = Bend { amplitude: sign * admitted, base, root, length };
                assert!(
                    frame.bend_headroom(root, length, base) >= admitted - 1e-12,
                    "{name}: the family budget {admitted} is over this frame's own headroom"
                );
                for (what, anchor) in anchors {
                    let mut tight = Canvas::new();
                    stamp(&mut tight, frame, anchor, Vec2::new(0.8, -0.6), 1.0, Mask::None, bend);
                    let mut wide = Canvas::new();
                    stamp_layers_bent_with_radius(
                        &mut wide,
                        anchor,
                        Vec2::new(0.8, -0.6),
                        &[(Pose::still(frame), 1.0)],
                        1.0,
                        1.0,
                        Mask::None,
                        bend,
                        14.0,
                        &mut Vec::new(),
                    );
                    assert_same_canvas(
                        &tight,
                        &wide,
                        &format!("{name} at {what}, amplitude {}", bend.amplitude),
                    );
                    let painted = every_pixel().any(|(f, x, y)| tight.get(f, x, y) != [0.0; 3]);
                    assert!(painted, "{name} vanished at {what} under {}", bend.amplitude);
                    checked += 1;
                }
            }
        }
    };

    for plant in &art.plants {
        let budget = plant_bend_budget(plant);
        for stage in &plant.stages {
            sweep(&plant.name, stage, budget, PLANT_BEND_ROOT, PLANT_BEND_LENGTH, 0.0);
        }
        if let Some(fruit) = &plant.fruit {
            sweep(&plant.name, fruit, budget, PLANT_BEND_ROOT, PLANT_BEND_LENGTH, 0.0);
        }
        for transition in &plant.transitions {
            sweep(&plant.name, &transition.clip, budget, PLANT_BEND_ROOT, PLANT_BEND_LENGTH, 0.0);
        }
    }
    for plant in &art.tall {
        let budget = tall_bend_budget(plant);
        let trunk_base = tall_bend_base(f64::from(TALL_MAX_SEGMENTS));
        if let Some(base) = &plant.base {
            sweep(&plant.name, base, budget, TALL_BEND_ROOT, TALL_BEND_LENGTH, tall_bend_base(0.0));
        }
        if let Some(vine) = &plant.vine_strips {
            sweep(&plant.name, &vine.trunk, budget, TALL_BEND_ROOT, TALL_BEND_LENGTH, trunk_base);
            sweep(&plant.name, &vine.endpoint, budget, TALL_BEND_ROOT, TALL_BEND_LENGTH,
                tall_bend_base(f64::from(TALL_MAX_SEGMENTS) + 1.0));
            // Also preserve the legacy raw-image footprint check at its own admission;
            // the opt-in intentionally no longer tries to bend this unrendered image.
            let raw_budget = plant.trunk.frames.iter()
                .map(|f| f.bend_headroom(TALL_BEND_ROOT, TALL_BEND_LENGTH, trunk_base))
                .fold(f64::INFINITY, f64::min);
            sweep(&plant.name, &plant.trunk, raw_budget, TALL_BEND_ROOT, TALL_BEND_LENGTH, trunk_base);
        } else {
            sweep(&plant.name, &plant.trunk, budget, TALL_BEND_ROOT, TALL_BEND_LENGTH, trunk_base);
        }
        if let Some(cap) = &plant.cap {
            sweep(
                &plant.name,
                cap,
                budget,
                TALL_BEND_ROOT,
                TALL_BEND_LENGTH,
                tall_bend_base(f64::from(TALL_MAX_SEGMENTS) + 1.0),
            );
        }
    }
    assert!(checked > 500, "the sweep only drew {checked} stamps");
}

// ---------------------------------------------------------------------------
// 6. a synthetic striped column
// ---------------------------------------------------------------------------

/// A synthetic column whose art is 4-periodic in `y` and whose every row carries the same
/// amount of green, so the image itself says whether each row of the column was composited
/// exactly **once**: a gap reads as a dark row, a doubled row as a bright one, and a
/// half-tile registration error as the wrong stripe colour. Built by hand from the documented
/// geometry — the base drawn whole, trunk tile `i` owning `Strip { floor: TALL_FIRST_JOIN or
/// TALL_STRIP_FLOOR, reveal: min(TALL_STRIP_TOP, grown − (4i − 8)) }`, the cap gliding at the
/// fractional index
/// `height + 1` — and stamped with one shared amplitude and [`tall_bend_base`] per tile.
///
/// The rows are checked at fractional heights and at both wind extrema, which is where a
/// per-tile bend (a different amplitude, or a bend rooted in each tile instead of at the
/// horizon) would open a join.
#[test]
fn a_synthetic_striped_column_composites_every_row_exactly_once_under_one_bend() {
    let column = TallColumn { face: Face::Front, cx: 7, pick: 0, vine: false };
    let base_anchor = tall_anchor_at(column.face, column.cx, 0.0);
    let heading = tall_heading(column.face, column.cx);
    assert_eq!(heading, Vec2::new(1.0, 0.0), "the fixture needs a column whose tile axes are the face's");
    assert_eq!(base_anchor.v.fract(), 0.0, "the fixture needs an integer anchor row");
    let v0 = base_anchor.v;

    let plant = striped_tall();
    let budget = tall_bend_budget(&plant);
    assert!(budget.is_finite() && budget > 0.05, "the synthetic column's budget is {budget}");

    // How tall the column may be drawn before its cap leaves this face: a row at global height
    // `H` is face row `v0 − 0.5 − H`, and the cap's top painted row is at `4·height + 11.5`.
    let tallest = ((v0 - 13.0) / 4.0).floor().min(f64::from(TALL_MAX_SEGMENTS));
    assert!(tallest >= 3.0, "the fixture's column can only reach {tallest} segments");

    let green = f64::from(srgb_decode(STRIPE_GREEN)) * f64::from(TALL_OPACITY) * STRIPE_WIDTH as f64;
    for height in [3.0, 3.25, 3.5, 3.75, tallest] {
        for amplitude in [0.0, budget, -budget] {
            let image = draw_striped_column(&plant, &column, height, amplitude, false);
            let grown = tall_grown_px(height);
            let whole = height.fract() == 0.0;
            // Everything the column can paint: the base tile's lowest row up to the cap's top.
            let span = -7.5..=(4.0 * height + 11.5);
            let mut exact = 0;
            for y in 0..FACE_SIZE as u8 {
                let h = v0 - 0.5 - f64::from(y);
                if !span.contains(&h) {
                    continue;
                }
                let (mut red, mut lit, mut blue) = (0.0f64, 0.0f64, 0.0f64);
                for x in 0..FACE_SIZE as u8 {
                    let p = image.get(column.face, x, y);
                    red += f64::from(p[0]);
                    lit += f64::from(p[1]);
                    blue += f64::from(p[2]);
                }
                assert!(
                    lit <= green + 1e-5,
                    "row at H = {h} carries {lit} green, over the {green} of one stamp \
                     (height {height}, amplitude {amplitude}): a row was composited twice"
                );
                // Rows safely below the growing edge are owned outright by the base or one
                // trunk strip; with a whole height the cap's rows are aligned too, so the
                // whole column is exact.
                let interior = h <= grown - 1.5 || (whole && h >= grown + 0.5);
                if !interior {
                    continue;
                }
                exact += 1;
                assert!(
                    (lit - green).abs() < 1e-5,
                    "row at H = {h} carries {lit} green, not the {green} of exactly one stamp \
                     (height {height}, amplitude {amplitude}): a gap or a double"
                );
                // And it is the stripe the global height asks for: the trunk is 4-periodic, so
                // every tile that could own this row paints the same colour into it.
                let k = stripe_of(v0, y);
                for (channel, want) in [(red, STRIPE_RED[k]), (blue, STRIPE_BLUE[k])] {
                    let expect = f64::from(srgb_decode(want))
                        * f64::from(TALL_OPACITY)
                        * STRIPE_WIDTH as f64;
                    assert!(
                        (channel - expect).abs() < 1e-5,
                        "row at H = {h} shows {channel} where stripe {k} is {expect} \
                         (height {height}, amplitude {amplitude}): the column slipped a row"
                    );
                }
            }
            assert!(exact > 12, "only {exact} rows were checked exactly at height {height}");
        }
    }
}

/// A vine "shares its supporting column's displacement; do not let it slide against the
/// trunk": the same amplitude and the same [`Bend`] shape, differing only in the tile's own
/// base. Drawn over the trunk it can only *add* — its rows are its own — and the gap between
/// its stripe and the trunk's must be the authored one in every row and at both extrema. A vine
/// driven by its own (still) response, or by a bend rooted in its own tile, drifts here.
#[test]
fn a_vine_takes_its_host_columns_amplitude_and_only_adds_light() {
    let column = TallColumn { face: Face::Front, cx: 7, pick: 0, vine: true };
    let anchor = tall_anchor_at(column.face, column.cx, 0.0);
    let plant = striped_tall();
    let budget = tall_bend_budget(&plant);
    // The vine carries no response of its own: it would stand perfectly still.
    assert_eq!(wind_response(VINE_PLANT).tip_px, 0.0);

    for (height, amplitude) in [(3.0, 0.0), (3.0, budget), (3.0, -budget), (3.5, budget), (3.5, -budget)]
    {
        let bare = draw_striped_column(&plant, &column, height, amplitude, false);
        let vined = draw_striped_column(&plant, &column, height, amplitude, true);
        // The trunk's own pixels are untouched: the vine paints its own columns only. A tile
        // column `tx` lands on face column `u0 + tx − 8` plus the subpixel bend, so the trunk's
        // three columns stay below this split and the vine's one stays above it.
        let split = anchor.u as u8 + (VINE_COLUMN - TRUNK_COLUMN) - 1;
        let mut vine_rows = 0;
        for y in 0..FACE_SIZE as u8 {
            for x in 0..FACE_SIZE as u8 {
                let (a, b) = (bare.get(column.face, x, y), vined.get(column.face, x, y));
                if x < split {
                    assert_eq!(a, b, "the vine moved a trunk pixel at ({x}, {y})");
                } else {
                    for c in 0..3 {
                        assert!(b[c] >= a[c] - 1e-6, "the vine took light at ({x}, {y})");
                    }
                }
            }
            // In every row the vine paints, its stripe sits exactly the authored distance from
            // the trunk's: both were displaced by the same `D` at the same height.
            let trunk = centroid(&bare, column.face, y, 0..split);
            let vine = centroid(&vined, column.face, y, split..FACE_SIZE as u8);
            if let (Some(trunk), Some(vine)) = (trunk, vine) {
                assert!(
                    (vine - trunk - f64::from(VINE_COLUMN - TRUNK_COLUMN)).abs() < 2e-5,
                    "row {y}: the vine sits {} from the trunk, not the authored {}",
                    vine - trunk,
                    VINE_COLUMN - TRUNK_COLUMN
                );
                vine_rows += 1;
            }
        }
        assert!(vine_rows >= 8, "the fixture's vine painted only {vine_rows} rows");
    }
}

// --- the synthetic column's art and geometry -------------------------------

/// Tile columns the synthetic trunk paints, centred on the tile's pivot.
const TRUNK_COLUMN: u8 = 8;
const STRIPE_WIDTH: usize = 3;
/// The tile column the synthetic vine paints, far enough from the trunk that a subpixel bend
/// never mixes the two.
const VINE_COLUMN: u8 = 12;
/// The green every stripe shares, so a row's green *is* how often it was composited.
const STRIPE_GREEN: u8 = 200;
const STRIPE_RED: [u8; 4] = [40, 100, 160, 220];
const STRIPE_BLUE: [u8; 4] = [230, 170, 110, 50];

/// The stripe a face row must show: the trunk repeats every 4 px and every tile of the column
/// is stamped 4 px apart, so the source row of *any* tile that could own this face row has the
/// same index modulo 4.
fn stripe_of(v0: f64, y: u8) -> usize {
    (f64::from(y) + 8.0 - v0).rem_euclid(4.0) as usize
}

fn striped_tile(rows: std::ops::Range<usize>, column: u8, width: usize) -> Sprite {
    let mut bytes = vec![0u8; 16 * 16 * 4];
    for ty in rows {
        for dx in 0..width {
            let tx = column as usize + dx - width / 2;
            let k = ty % 4;
            let at = (ty * 16 + tx) * 4;
            bytes[at..at + 4].copy_from_slice(&[STRIPE_RED[k], STRIPE_GREEN, STRIPE_BLUE[k], 255]);
        }
    }
    Sprite::from_rgba(16, 16, Vec2::new(8.0, 8.0), &bytes).unwrap()
}

fn one_frame(sprite: Sprite) -> Clip {
    Clip { frames: vec![sprite], seconds: 3.0, looping: true }
}

/// The synthetic family: a base whose art stops where the first trunk's strip begins
/// (`TALL_FIRST_JOIN − 4` above the horizon, as the pack's own base tiles do), a 4-periodic
/// trunk, and a cap whose trunk-joining tail is already cleared.
fn striped_tall() -> TallPlant {
    TallPlant {
        name: "stripes".into(),
        base: Some(one_frame(striped_tile(3..16, TRUNK_COLUMN, STRIPE_WIDTH))),
        trunk: one_frame(striped_tile(0..16, TRUNK_COLUMN, STRIPE_WIDTH)),
        crown: None,
        cap: Some(one_frame(striped_tile(0..4, TRUNK_COLUMN, STRIPE_WIDTH))),
        tail_row: 16,
        vine_strips: None,
    }
}

fn striped_vine() -> Sprite {
    // Exactly the rows a vine tile owns: `TALL_VINE_FLOOR .. TALL_VINE_TOP` above the tile's
    // bottom edge, which are the tile rows whose centres lie in that strip.
    let floor = (TILE_ROWS - TALL_VINE_TOP) as usize;
    let top = (TILE_ROWS - TALL_VINE_FLOOR) as usize;
    striped_tile(floor..top, VINE_COLUMN, 1)
}

/// One column stamped by hand from the documented geometry: nothing but the parts, their
/// strips, their opacity and one shared amplitude.
fn draw_striped_column(
    plant: &TallPlant,
    column: &TallColumn,
    height: f64,
    amplitude: f64,
    vine: bool,
) -> Canvas {
    let mut canvas = Canvas::new();
    let heading = tall_heading(column.face, column.cx);
    let grown = tall_grown_px(height);
    let bend_at = |i: f64| Bend {
        amplitude,
        base: tall_bend_base(i),
        root: TALL_BEND_ROOT,
        length: TALL_BEND_LENGTH,
    };
    let mut part = |sprite: &Sprite, i: f64, mask: Mask| {
        stamp(
            &mut canvas,
            sprite,
            tall_anchor_at(column.face, column.cx, i),
            heading,
            TALL_OPACITY,
            mask,
            bend_at(i),
        );
    };
    part(&plant.base.as_ref().unwrap().frames[0], 0.0, Mask::None);
    let (strip_floor, strip_top) = trunk_strip(plant);
    for i in 1..=(height.ceil() as u8) {
        let floor = if i == 1 { TALL_FIRST_JOIN } else { strip_floor };
        let top = if i == TALL_MAX_SEGMENTS { TILE_ROWS } else { strip_top };
        let reveal = top.min(grown - tall_bend_base(f64::from(i)));
        if reveal <= floor {
            continue;
        }
        part(&plant.trunk.frames[0], f64::from(i), Mask::Strip { floor, reveal });
    }
    part(&plant.cap.as_ref().unwrap().frames[0], height + 1.0, Mask::None);
    if vine {
        let sprite = striped_vine();
        let mut i = 1u8;
        while f64::from(i) <= height.ceil() {
            let top = if i + 2 > TALL_MAX_SEGMENTS { TILE_ROWS } else { TALL_VINE_TOP };
            let reveal = top.min(grown - tall_bend_base(f64::from(i)));
            if reveal > TALL_VINE_FLOOR {
                part(&sprite, f64::from(i), Mask::Strip { floor: TALL_VINE_FLOOR, reveal });
            }
            i += 2;
        }
    }
    canvas
}

/// The horizontal centre of mass of one face row's green over `columns`, in face pixels, or
/// `None` where the row is dark.
fn centroid(image: &Canvas, face: Face, y: u8, columns: std::ops::Range<u8>) -> Option<f64> {
    let (mut light, mut moment) = (0.0f64, 0.0f64);
    for x in columns {
        let v = f64::from(image.get(face, x, y)[1]);
        light += v;
        moment += v * (f64::from(x) + 0.5);
    }
    (light > 1e-3).then_some(moment / light)
}

// ---------------------------------------------------------------------------
// 7. the presenter
// ---------------------------------------------------------------------------

/// Through a quiet interval nothing on the cube is bent at all: every slot of every cell gets
/// `Bend::NONE` and its own untouched heading, and every column's amplitude is exactly 0. With
/// the renderer's identity path (`tests/bend.rs`) that is the whole zero-wind guarantee — the
/// slice-1 image bit for bit, at the slice-1 cost — and it is the one thing a stray flutter
/// term or an un-clamped envelope would break.
#[test]
fn a_quiet_tick_bends_nothing_anywhere_on_the_cube() {
    let presenter = ArtPresenter::new(pack());
    let quiet = active_seconds() + WIND_TRAVEL_SECONDS + 1.0;
    assert_eq!(wind_strength(quiet), 0.0);
    let mut checked = 0;
    for cell in CellId::all() {
        let slot = slot_of(cell);
        for band in [Band::Soil, Band::Foliage, Band::Canopy, Band::Water] {
            let name = species_of(band, cell);
            let (bend, heading) =
                slot_wind(&slot, name, presenter.bend_budget(name), quiet);
            assert!(bend.is_identity(), "{name} in {cell:?} is bent in a quiet interval");
            assert_eq!(heading, slot.heading, "{name} in {cell:?} turned in a quiet interval");
            checked += 1;
        }
    }
    assert_eq!(checked, CELL_COUNT * 4);
    for column in tall_columns() {
        assert_eq!(
            tall_amplitude(&column, presenter.column_budget(&column), quiet),
            0.0,
            "{column:?} bends in a quiet interval"
        );
    }
    // And the breeze really does blow at other times, so this is not vacuous.
    let windy = WIND_RISE + WIND_HOLD * 0.5;
    let moving = tall_columns()
        .into_iter()
        .filter(|c| tall_amplitude(c, presenter.column_budget(c), windy).abs() > 1e-6)
        .count();
    assert!(moving > 3, "only {moving} columns move at full wind");
}

/// The wind moves plants and columns and nothing else. Both images are drawn at instants 48 s
/// apart — a whole number of periods of every clip in the pack (3 s and 6 s), of the creature
/// clips (4, 1.6 and 2 s) and of the rain (0.4 s), but not of the 30 s wind packet — so the
/// ground, the bodies and the rain are in exactly the same phase in both, one instant is inside
/// a packet's hold and the other inside a quiet interval, and every pixel that differs must be
/// a plant pixel. The plant pixels are identified independently, as the pixels a pack with no
/// plants and no columns draws differently.
///
/// (The water shimmer's 2.5 s period has no common multiple with the clips that is not also a
/// multiple of `WIND_PERIOD`, so this fixture leaves the water dry; a flooded cell's reed is
/// covered by the root and budget sweeps instead.)
#[test]
fn the_rising_breeze_moves_plant_pixels_and_leaves_the_ground_bodies_and_rain_alone() {
    let calm = 24.0;
    let windy = calm + 48.0;
    assert_eq!(wind_strength(calm), 0.0, "the calm instant must be inside a quiet interval");
    assert!(wind_strength(windy) > 0.5, "the windy instant must be inside a packet");

    let view_at = |seconds: f64| {
        let mut v = rich_view(tick_at(seconds));
        v.rain.fill(0.6);
        v.organisms = vec![
            organism(OrganismId { slot: 1, generation: 1 }, SurfacePoint::new(Face::Front, 20.0, 40.0)),
            organism(OrganismId { slot: 2, generation: 1 }, SurfacePoint::new(Face::Left, 33.0, 22.0)),
            organism(OrganismId { slot: 3, generation: 1 }, SurfacePoint::new(Face::Top, 30.0, 30.0)),
        ];
        v
    };
    let image = |art: ArtPack, seconds: f64| {
        let v = view_at(seconds);
        let mut p = snapped(art, &v);
        draw(&mut p, &v, 0.0)
    };

    let bare_calm = image(pack_without(true, true, false), calm);
    let bare_windy = image(pack_without(true, true, false), windy);
    assert_same_canvas(
        &bare_calm,
        &bare_windy,
        "the fixture's two instants must show the same ground, bodies and rain",
    );

    let full_calm = image(pack(), calm);
    let full_windy = image(pack(), windy);
    let mut plant_pixels: Vec<(Face, u8, u8)> = differing(&full_calm, &bare_calm);
    plant_pixels.extend(differing(&full_windy, &bare_windy));
    let moved = differing(&full_calm, &full_windy);
    assert!(moved.len() > 40, "the breeze moved only {} pixels", moved.len());
    for pixel in &moved {
        assert!(
            plant_pixels.contains(pixel),
            "the breeze moved {pixel:?}, which no plant or column paints"
        );
    }
}

/// "A root that moved by a hundredth of a pixel would still resample and skate": every row at
/// or below [`PLANT_BEND_ROOT`] must be *identical* windy and calm, for every stage and the
/// fruit clip of every species that bends, at the largest amplitude its family and a `+10 %`
/// slot can produce, both ways. The authored root contact is tile row 14, whose centre is
/// exactly 1.5 px above the tile's bottom edge, so this is exact rather than close.
#[test]
fn the_painted_root_row_of_every_side_species_is_identical_windy_and_calm() {
    let art = pack();
    let anchor = SurfacePoint::new(Face::Front, 32.0, 32.0);
    // With a `(8, 8)` pivot at an integer chart position, tile row `ty` lands on face row
    // `ty + 24`; the rows at or below the root are those with `16 − (ty + 0.5) ≤ root`.
    let root_rows: Vec<u8> = (0..16u8)
        .filter(|&ty| TILE_ROWS - (f64::from(ty) + 0.5) <= PLANT_BEND_ROOT)
        .map(|ty| ty + 24)
        .collect();
    assert_eq!(root_rows, vec![38, 39], "the root rows of a 16-row tile");

    let mut species = 0;
    for plant in &art.plants {
        let response = wind_response(&plant.name);
        if response.tip_px <= 0.0 {
            // A canopy species turns instead of bending, and `rootveil` does neither.
            assert!(
                response.spin_deg > 0.0 || response == cubarium::art_present::WindResponse::STILL,
                "{} has no response at all",
                plant.name
            );
            continue;
        }
        species += 1;
        let tip = effective_tip(response.tip_px, plant_bend_budget(plant))
            * (1.0 + WIND_SLOT_VARIATION);
        assert!(tip > 0.0, "{} is admitted no movement at all", plant.name);
        let clips: Vec<&Clip> = plant
            .stages
            .iter()
            .chain(plant.fruit.iter())
            .chain(plant.transitions.iter().map(|t| &t.clip))
            .collect();
        for (which, clip) in clips.iter().enumerate() {
            for frame in clip.frames.iter().step_by(6) {
                let mut calm = Canvas::new();
                stamp(&mut calm, frame, anchor, Vec2::new(1.0, 0.0), MOTIF_OPACITY, Mask::None, Bend::NONE);
                let painted = (24..40u8)
                    .any(|x| calm.get(Face::Front, x, root_rows[0]) != [0.0; 3]);
                for sign in [-1.0, 1.0] {
                    let bend = plant_bend(tip, Vec2::new(sign, 0.0), Vec2::new(1.0, 0.0));
                    assert!((bend.amplitude.abs() - tip).abs() < 1e-12, "the fixture's bend must be at full tip");
                    let mut windy = Canvas::new();
                    stamp(&mut windy, frame, anchor, Vec2::new(1.0, 0.0), MOTIF_OPACITY, Mask::None, bend);
                    for &y in &root_rows {
                        for x in 20..44u8 {
                            assert_eq!(
                                windy.get(Face::Front, x, y),
                                calm.get(Face::Front, x, y),
                                "{} clip {which} skated its root row {y} at x {x}, wind {sign}",
                                plant.name
                            );
                        }
                    }
                    assert!(
                        max_diff(&calm, &windy) > 1e-4 || tip < 1e-3,
                        "{} clip {which} did not move at all under {tip}",
                        plant.name
                    );
                }
                if which < 3 {
                    assert!(painted, "{} stage {which} paints no root row", plant.name);
                }
            }
        }
    }
    assert!(species >= 4, "only {species} bending species were swept");
}

/// At 60 fps a plant's pixels must never jump: through a packet's rise and its fall the largest
/// per-frame change of any pixel of the plant may exceed what the authored sway alone does
/// during a quiet interval only by what the wind can add in one frame. The amplitude moves by
/// at most `tip · max|d strength/dt| / 60` ≈ 0.01 px per frame, and a pixel's light changes by
/// at most one unit per pixel of motion, so the whole wind contribution is a hundredth; the
/// bound below is five times that. A wind sampler driven by wall time, or one that stepped its
/// packet per tick, would blow straight through it.
#[test]
fn a_plants_pixels_never_jump_between_two_frames_at_sixty_fps() {
    let cell = full_plant_cell("reedspire").or_else(|| full_plant_cell("lanternstalk")).expect(
        "the cube has a rank-2 slot of a bending species in the middle of a side face",
    );
    let window = near(cell);
    let art = pack_without(true, false, true);
    let name = species_of(band_of(cell), cell);
    let response = wind_response(name);
    assert!(response.tip_px > 0.0, "{name} does not bend");

    let sweep = |from: f64, to: f64| {
        let mut p = ArtPresenter::new(pack_without(true, false, true));
        let mut previous: Option<Canvas> = None;
        let mut worst = 0.0f32;
        let mut seconds = from;
        while seconds < to {
            let tick = tick_at((seconds / DT).round() * DT);
            let v = one_cell_view(tick, cell, 1.0);
            p.observe(&v);
            for third in 0..3 {
                let image = draw(&mut p, &v, f64::from(third) / 3.0);
                if let Some(before) = &previous {
                    worst = worst.max(max_diff_at(&image, before, &window));
                }
                previous = Some(image);
            }
            seconds += DT;
        }
        worst
    };
    // A whole sway period of quiet: the authored sway's own worst frame-to-frame step.
    let quiet_from = active_seconds() + 1.0;
    let sway = sweep(quiet_from, quiet_from + 3.0);
    assert!(sway > 0.0, "the fixture's plant does not even sway");
    assert!(
        sway < 0.15,
        "the authored sway already steps by {sway} a frame, so it would hide a wind jump"
    );
    assert_eq!(wind_strength(quiet_from), 0.0);

    // The wind's own share of a frame: the amplitude moves by at most
    // `tip · max|d strength/dt| / 60` ≈ 0.7 · 0.72 / 60 ≈ 0.008 px, and a pixel's light moves by
    // at most about one unit per pixel of motion. Measured (2026-09-12): the sway alone steps a
    // pixel by 0.065 and the windiest frame of a rise or a fall by 0.076, so the breeze is
    // costing 0.012 — this bound is under three times the derivation and well under a jump.
    const WIND_STEP: f32 = 0.03;
    for (what, from) in [("the rise", WIND_PERIOD), ("the fall", WIND_PERIOD + WIND_RISE + WIND_HOLD)] {
        let windy = sweep(from, from + WIND_RISE.min(WIND_FALL));
        assert!(
            windy <= sway + WIND_STEP,
            "through {what} a frame moved a pixel by {windy}, over the sway's own {sway} plus {WIND_STEP}"
        );
    }
    // Non-vacuity: over the same interval the plant really did lean.
    let mut p = ArtPresenter::new(art);
    let calm_v = one_cell_view(tick_at(quiet_from), cell, 1.0);
    p.observe(&calm_v);
    let calm = draw(&mut p, &calm_v, 0.0);
    let windy_tick = tick_at(WIND_PERIOD + WIND_RISE + WIND_HOLD * 0.5);
    let windy_v = one_cell_view(windy_tick, cell, 1.0);
    p.observe(&windy_v);
    let windy = draw(&mut p, &windy_v, 0.0);
    assert!(
        max_diff_at(&calm, &windy, &window) > 0.01,
        "the fixture's plant looks the same windy and calm"
    );
}

/// A rank-2 slot of `name` in the middle of a side face.
fn full_plant_cell(name: &str) -> Option<CellId> {
    CellId::all().find(|&c| {
        c.face() == Face::Front
            && c.cy() >= 4
            && c.cy() <= 8
            && (4..=11).contains(&c.cx())
            && plant_cap(band_of(c), c) == Some(2)
            && species_of(band_of(c), c) == name
    })
}

/// "Identical output for the same simulated time across 30/60/120 Hz histories": `draw` is pure,
/// so the number of frames a host asked for between two ticks cannot reach the image. A wind
/// sampler that advanced a phase per frame, or one that remembered its last amplitude, would
/// show three different images here.
#[test]
fn thirty_sixty_and_a_hundred_and_twenty_fps_draw_the_same_simulated_instant_alike() {
    let first = tick_at(WIND_PERIOD + WIND_RISE + 1.0);
    let ticks: Vec<u64> = (first..first + 8).collect();
    let mut images = Vec::new();
    for fps in [30u32, 60, 120] {
        let frames = fps / 20;
        let mut p = ArtPresenter::new(pack());
        for &tick in &ticks {
            let v = rich_view(tick);
            p.observe(&v);
            for frame in 0..frames {
                let _ = draw(&mut p, &v, f64::from(frame) / f64::from(frames));
            }
        }
        let last = rich_view(*ticks.last().unwrap());
        images.push((fps, draw(&mut p, &last, 0.5)));
    }
    for (fps, image) in &images[1..] {
        assert_same_canvas(&images[0].1, image, &format!("{fps} fps against 30 fps"));
    }
    assert!(
        max_diff(&images[0].1, &Canvas::new()) > 0.01,
        "the fixture drew an empty cube"
    );
}

/// A paused world holds its image, and two presenters given the same world draw the same cube:
/// the breeze is a pure function of the presentation instant, with no state and no wall time.
#[test]
fn a_paused_frame_and_a_second_presenter_draw_the_identical_windy_image() {
    let tick = tick_at(WIND_PERIOD + WIND_RISE + WIND_HOLD * 0.5);
    let v = rich_view(tick);
    assert!(wind_strength(present_seconds(tick, 0.5)) > 0.5, "the fixture tick must be windy");

    let mut p = snapped(pack(), &v);
    let held = draw(&mut p, &v, 0.5);
    for _ in 0..3 {
        assert_same_canvas(&draw(&mut p, &v, 0.5), &held, "a paused frame moved");
    }
    let mut other = snapped(pack(), &v);
    assert_same_canvas(&draw(&mut other, &v, 0.5), &held, "a second presenter drew differently");
    // And the next frame is a different image, so the pause is a pause and not a still cube.
    assert!(max_diff(&draw(&mut p, &v, 1.0), &held) > 0.0, "the cube never moves at all");
}

/// A canopy (top-face) plant "rotates instead of bending … about the stationary center", by
/// `θ = deg · |w| / WIND_CHART_MAX` degrees. The angle is recomputed here from the returned
/// heading rather than from `canopy_heading`, the bend is the identity, the heading stays a unit
/// vector, and the drawn cube is exactly the same sprite stamped at the *same anchor* with that
/// heading — a plant carried sideways, or one bent, differs from that hand stamp.
#[test]
fn a_canopy_plant_turns_about_its_pivot_and_is_never_carried_sideways() {
    let cell = CellId::all()
        .find(|&c| {
            c.face() == Face::Top
                && (5..=10).contains(&c.cx())
                && (5..=10).contains(&c.cy())
                && plant_cap(band_of(c), c) == Some(2)
                && wind_response(species_of(band_of(c), c)).spin_deg > 0.0
        })
        .expect("the cube has a rank-2 canopy slot in the middle of Top");
    let art = pack();
    let name = species_of(band_of(cell), cell);
    let plant = art.plant(name).expect("the canopy species is in the pack");
    let response = wind_response(name);
    let slot = slot_of(cell);
    let seconds = WIND_RISE + WIND_HOLD * 0.5;
    let w = wind_at(slot.at, seconds, response.lag_seconds);
    assert!(w.length() > 0.05, "the fixture's canopy cell must feel the breeze");

    let (bend, heading) = slot_wind(&slot, name, plant_bend_budget(plant), seconds);
    assert_eq!(bend, Bend::NONE, "a canopy plant must never be bent");
    assert!((heading.length() - 1.0).abs() < 1e-12, "the heading stopped being a unit vector");
    let turned = heading.screen_angle() - slot.heading.screen_angle();
    let want = (response.spin_deg * slot.wind * w.length() / WIND_CHART_MAX).to_radians();
    assert!(
        (turned.rem_euclid(std::f64::consts::TAU) - want.rem_euclid(std::f64::consts::TAU)).abs()
            < 1e-9,
        "the canopy turned {turned} rad, not the documented {want}"
    );
    assert_eq!(heading, canopy_heading(slot.heading, response.spin_deg * slot.wind, w));
    // Exactly calm, exactly the authored heading, bit for bit.
    assert_eq!(canopy_heading(slot.heading, response.spin_deg, Vec2::ZERO), slot.heading);

    // The drawn cube: the background a pack with no plants draws, plus this one stamp.
    let tick = tick_at((seconds / DT).round() * DT);
    let v = one_cell_view(tick, cell, 1.0);
    let now = present_seconds(tick, 0.0);
    let (bend, heading) = slot_wind(&slot, name, plant_bend_budget(plant), now);
    let clip = &plant.stages[2];
    let pose = clip.sample(now + plant_phase_of(cell, clip.seconds));
    let (at, _) = placement_of(cell);
    assert_eq!(at, slot.at, "the slot's anchor is where the plant stands");

    let mut expected = {
        let mut bare = ArtPresenter::new(pack_without(true, true, false));
        observe_draw(&mut bare, &v, 0.0)
    };
    let mut unturned = expected.clone();
    stamp_layers_bent(
        &mut expected,
        at,
        heading,
        &[(pose, 1.0)],
        1.0,
        MOTIF_OPACITY,
        Mask::None,
        bend,
        &mut Vec::new(),
    );
    stamp_layers_bent(
        &mut unturned,
        at,
        slot.heading,
        &[(pose, 1.0)],
        1.0,
        MOTIF_OPACITY,
        Mask::None,
        Bend::NONE,
        &mut Vec::new(),
    );
    let mut p = ArtPresenter::new(pack_without(true, false, false));
    let actual = observe_draw(&mut p, &v, 0.0);
    assert_eq!(p.growth_of(cell), Growth::snapped(Some(2), false), "the canopy plant must be full");
    assert_same_canvas(&actual, &expected, "a canopy plant at full wind");
    assert!(
        max_diff_at(&expected, &unturned, &near(cell)) > 1e-4,
        "the fixture's rotation is too small to see, so it proves nothing"
    );
}

/// Pack v5's contract, both halves: a v4-style pack (no growth transitions at all) still loads,
/// still measures a budget and still draws, and the shipped lanternstalk's `grow01` clip is the
/// documented one — non-looping, `from + 1 == to`, and inclusive at both ends, so
/// `Clip::sample(0)` and `Clip::sample(seconds)` are exactly the two endpoint frames rather than
/// a wrap back into the first.
#[test]
fn a_v4_pack_still_draws_and_the_growth_clip_is_a_non_looping_pair_of_endpoints() {
    let art = pack();
    let plant = art.plant("lanternstalk").expect("lanternstalk is in the pack");
    let clip = plant.transition(0, 1).expect("the pilot's grow01 clip is baked");
    assert!(!clip.looping, "a growth transition must not loop");
    assert!(clip.seconds > 0.0 && clip.frames.len() >= 2);
    let transition = plant
        .transitions
        .iter()
        .find(|t| t.from == 0)
        .expect("the transition carries its own stages");
    assert_eq!(transition.to, transition.from + 1, "a transition is one stage step");
    assert!(!transition.clip.looping);
    assert!(
        plant.transitions.iter().all(|t| plant.transition(t.from, t.to).is_some()),
        "every authored transition is reachable by its pair"
    );
    assert!(plant.transition(1, 0).is_none(), "growth clips run one way");

    // Inclusive endpoints: the first sample is the first frame with no blend into a second, and
    // the last is the last frame. A clip sampled at `i / n` like a loop would never reach it.
    let start = clip.sample(0.0);
    assert!(std::ptr::eq(start.first, &clip.frames[0]));
    assert_eq!(start.weight(), 0.0, "the first sample blends into nothing");
    let end = clip.sample(clip.seconds);
    let last = clip.frames.last().unwrap();
    assert!(
        std::ptr::eq(end.first, last) || (std::ptr::eq(end.second, last) && end.weight() == 1.0),
        "the last sample is not the clip's last frame"
    );
    assert!(
        max_diff(&frame_image(&clip.frames[0]), &frame_image(last)) > 1e-3,
        "the growth clip's two endpoints are the same image"
    );

    // A v4 pack: the same art with every transition row dropped.
    let mut v4 = pack();
    for plant in &mut v4.plants {
        plant.transitions.clear();
    }
    let plain = v4.plant("lanternstalk").unwrap();
    assert!(plain.transition(0, 1).is_none(), "a v4 pack carries no transitions");
    assert!(plant_bend_budget(plain) >= plant_bend_budget(plant), "dropping a clip cannot tighten a budget");
    let tick = tick_at(WIND_PERIOD + WIND_RISE + WIND_HOLD * 0.5);
    let v = rich_view(tick);
    let mut p = snapped(v4, &v);
    let image = draw(&mut p, &v, 0.0);
    assert!(max_diff(&image, &Canvas::new()) > 0.05, "a v4 pack drew nothing");
}

/// One sprite stamped alone, for comparing two frames of a clip.
fn frame_image(sprite: &Sprite) -> Canvas {
    let mut canvas = Canvas::new();
    stamp(
        &mut canvas,
        sprite,
        SurfacePoint::new(Face::Front, 32.0, 32.0),
        Vec2::new(1.0, 0.0),
        1.0,
        Mask::None,
        Bend::NONE,
    );
    canvas
}

/// The bend shape a small plant's stamps carry, recomputed: the profile is still at
/// [`PLANT_BEND_ROOT`] and saturates at [`PLANT_BEND_ROOT`]` + `[`PLANT_BEND_LENGTH`], and the
/// amplitude is the breeze projected onto the tile's own horizontal axis. A plant turned away
/// from the wind answers less than its neighbour; one facing across it does not move at all.
// ---------------------------------------------------------------------------
// trunk strips: the end rows a tile never owns (the spiretree's earned wind room)
// ---------------------------------------------------------------------------

/// Paint the given rows of every trunk frame of a tall plant solid magenta, in place.
fn make_end_rows_loud(plant: &mut TallPlant, rows: &[usize]) {
    for frame in &mut plant.trunk.frames {
        let (w, h) = (frame.width(), frame.height());
        let mut pixels: Vec<[f32; 4]> = (0..h as i32)
            .flat_map(|y| (0..w as i32).map(move |x| (x, y)))
            .map(|(x, y)| frame.texel(x, y))
            .collect();
        // Only the trunk's own columns (those its pattern paints), so the loud tile keeps
        // the footprint the loader admits.
        let columns: Vec<usize> = (0..w).filter(|&x| (0..h).any(|y| pixels[y * w + x][3] > 0.0)).collect();
        for &row in rows {
            for &x in &columns {
                pixels[row * w + x] = [1.0, 0.0, 1.0, 1.0];
            }
        }
        *frame = Sprite::from_premultiplied(w, h, frame.pivot(), pixels).expect("a loud trunk");
    }
}

/// One real column hand-built from the documented geometry with an explicit strip rule:
/// base whole, trunk strips `floor2..top(i)` (the first segment from `TALL_FIRST_JOIN`), the
/// cap at the fractional index, one shared amplitude.
fn build_column(
    plant: &TallPlant,
    column: &TallColumn,
    height: f64,
    amplitude: f64,
    floor2: f64,
    top: &dyn Fn(u8) -> f64,
) -> Canvas {
    let mut canvas = Canvas::new();
    let heading = tall_heading(column.face, column.cx);
    let grown = tall_grown_px(height);
    let mut part = |sprite: &Sprite, i: f64, mask: Mask| {
        stamp(
            &mut canvas,
            sprite,
            tall_anchor_at(column.face, column.cx, i),
            heading,
            TALL_OPACITY,
            mask,
            Bend { amplitude, base: tall_bend_base(i), root: TALL_BEND_ROOT, length: TALL_BEND_LENGTH },
        );
    };
    part(&plant.base.as_ref().unwrap().frames[0], 0.0, Mask::None);
    for i in 1..=TALL_MAX_SEGMENTS {
        let floor = if i == 1 { TALL_FIRST_JOIN } else { floor2 };
        let reveal = top(i).min(grown - tall_bend_base(f64::from(i)));
        if reveal <= floor {
            break;
        }
        part(&plant.trunk.frames[0], f64::from(i), Mask::Strip { floor, reveal });
    }
    part(&plant.cap.as_ref().unwrap().frames[0], height + 1.0, Mask::None);
    canvas
}

/// A trunk tile's row 15 is **never drawn** for any family, at any height of a column: a
/// pack whose trunk tiles carry row 15 in solid magenta draws every calm column identically
/// to the shipped pack all the way up from bare ground to the rim. And on the shifted
/// strips a tile's row 0 is drawn only by the last possible segment: the spiretree column
/// hand-built on those strips (the art-derived rule cannot be forced through the presenter,
/// because painting row 0 is exactly what opts a family *out*) with its trunk's row 0 in
/// magenta is the shipped column, bit for bit and under a live bend, until the ninth segment
/// stands — where the loud row shows under the cap (non-vacuity). That is what lets the
/// spiretree leave those rows unpainted to earn wind room, with its dome over the top.
#[test]
fn a_trunk_tiles_end_rows_are_never_drawn_below_the_top_segment() {
    // Row 15, through the presenter, every family, growing from bare ground.
    let mut loud_bottom = pack();
    for plant in &mut loud_bottom.tall {
        make_end_rows_loud(plant, &[15]);
    }
    for (a, b) in loud_bottom.tall.iter().zip(&pack().tall) {
        assert!(
            max_diff(&frame_image(&a.trunk.frames[0]), &frame_image(&b.trunk.frames[0])) > 0.5,
            "{}: the loud row changed nothing",
            a.name
        );
    }
    let mut shipped = ArtPresenter::new(pack());
    let mut bottom = ArtPresenter::new(loud_bottom);
    let first = WIND_QUIET_TICK;
    shipped.observe(&bare_view(first));
    bottom.observe(&bare_view(first));
    let mut compared = 0;
    for tick in first + 1..=first + 720 {
        let v = rich_view(tick);
        shipped.observe(&v);
        bottom.observe(&v);
        if tick % 5 != 0 {
            continue;
        }
        for f in [0.0, 0.5] {
            // Calm at every column: the shared packet is delayed at each root, so ask each
            // column's own amplitude (any budget: zero wind is zero whatever the room).
            let seconds = present_seconds(tick, f);
            if tall_columns().iter().any(|c| tall_amplitude(c, 1.0, seconds) != 0.0) {
                continue;
            }
            let a = draw(&mut shipped, &v, f);
            let b = draw(&mut bottom, &v, f);
            assert_same_canvas(&a, &b, &format!("tick {tick} f {f}: a trunk row 15 was drawn"));
            compared += 1;
        }
    }
    assert!(compared > 100, "only {compared} calm frames were compared");
    for i in 0..tall_columns().len() {
        assert!(shipped.tall_growth_of(i).height >= f64::from(TALL_MAX_SEGMENTS) - 1e-9, "column {i} is not full");
    }

    // Row 0, hand-built on the shifted strips, the spiretree with and without a loud row 0.
    let art = pack();
    let spire = art.tall_plant("spiretree").expect("spiretree");
    let mut loud = pack();
    let loud_spire = loud.tall.iter_mut().find(|p| p.name == "spiretree").unwrap();
    make_end_rows_loud(loud_spire, &[0]);
    let column = tall_columns().into_iter().find(|c| !c.vine && TALL_PLANTS[c.pick] == "spiretree").expect("a bare spiretree column");
    let top = |i: u8| if i == TALL_MAX_SEGMENTS { TILE_ROWS } else { TALL_STRIP_TOP };
    let mut below = 0;
    for height in [0.5, 1.0, 1.3, 2.75, 4.5, 6.1, 7.9] {
        for amplitude in [0.0, 0.9, -0.9] {
            let a = build_column(spire, &column, height, amplitude, TALL_STRIP_FLOOR, &top);
            let b = build_column(loud_spire, &column, height, amplitude, TALL_STRIP_FLOOR, &top);
            assert_same_canvas(&a, &b, &format!("height {height}, amplitude {amplitude}: a trunk row 0 was drawn below the top"));
            below += 1;
        }
    }
    let a = build_column(spire, &column, 9.0, 0.0, TALL_STRIP_FLOOR, &top);
    let b = build_column(loud_spire, &column, 9.0, 0.0, TALL_STRIP_FLOOR, &top);
    assert!(!differing(&a, &b).is_empty(), "the ninth segment's row 0 is never drawn");
    println!("  {compared} calm frames: row 15 never drawn; row 0 hidden over {below} hand-built columns below the top, shown by the ninth segment");
}

/// The shifted strips are a per-family opt-in read from the art: a trunk that paints its
/// tile row 0 (glasscane, the vine, the synthetic stripes) keeps the original
/// `TALL_JOIN..TILE_ROWS` strips, so its image is untouched growth and all; one that leaves
/// row 0 clear (the spiretree) is stacked on `TALL_STRIP_FLOOR..TALL_STRIP_TOP`. And the
/// shifted strips still composite every row of a column **exactly once**: the striped
/// synthetic column with its trunk's rows 0 and 15 cleared carries one stamp of green in
/// every interior row, the stripe the global height asks for, at fractional heights and
/// both wind extrema — the same check the original strips pass.
#[test]
fn the_shifted_strips_are_opted_into_by_the_art_and_still_composite_each_row_once() {
    let art = pack();
    assert_eq!(trunk_strip(art.tall_plant("glasscane").unwrap()), (TALL_JOIN, TILE_ROWS));
    assert_eq!(trunk_strip(art.tall_plant(VINE_PLANT).unwrap()), (TALL_JOIN, TILE_ROWS));
    assert_eq!(trunk_strip(art.tall_plant("spiretree").unwrap()), (TALL_STRIP_FLOOR, TALL_STRIP_TOP));
    assert_eq!((TALL_STRIP_FLOOR, TALL_STRIP_TOP), (TALL_JOIN - 1.0, TILE_ROWS - 1.0));
    assert_eq!(trunk_strip(&striped_tall()), (TALL_JOIN, TILE_ROWS), "the stripes paint row 0");

    let column = TallColumn { face: Face::Front, cx: 7, pick: 0, vine: false };
    let v0 = tall_anchor_at(column.face, column.cx, 0.0).v;
    let mut plant = striped_tall();
    plant.trunk = one_frame(striped_tile(1..15, TRUNK_COLUMN, STRIPE_WIDTH));
    assert_eq!(trunk_strip(&plant), (TALL_STRIP_FLOOR, TALL_STRIP_TOP), "row 0 clear opts in");
    let budget = tall_bend_budget(&plant);
    // The trunk itself earns room (the stripes' 4-row cap stays the column's bound).
    let top = tall_bend_base(f64::from(TALL_MAX_SEGMENTS));
    let room = |p: &TallPlant| p.trunk.frames[0].bend_headroom(TALL_BEND_ROOT, TALL_BEND_LENGTH, top);
    assert!(room(&plant) > room(&striped_tall()) + 1.0, "clearing the end rows must earn trunk room: {} vs {}", room(&plant), room(&striped_tall()));
    let tallest = ((v0 - 13.0) / 4.0).floor().min(f64::from(TALL_MAX_SEGMENTS));
    let green = f64::from(srgb_decode(STRIPE_GREEN)) * f64::from(TALL_OPACITY) * STRIPE_WIDTH as f64;
    let mut exact_rows = 0;
    for height in [1.0, 2.5, 3.0, 3.25, 3.5, 3.75, 4.0, tallest] {
        for amplitude in [0.0, budget, -budget] {
            let image = draw_striped_column(&plant, &column, height, amplitude, false);
            let grown = tall_grown_px(height);
            let whole = height.fract() == 0.0;
            let span = -7.5..=(4.0 * height + 11.5);
            for y in 0..FACE_SIZE as u8 {
                let h = v0 - 0.5 - f64::from(y);
                if !span.contains(&h) {
                    continue;
                }
                let (mut red, mut lit, mut blue) = (0.0f64, 0.0f64, 0.0f64);
                for x in 0..FACE_SIZE as u8 {
                    let p = image.get(column.face, x, y);
                    red += f64::from(p[0]);
                    lit += f64::from(p[1]);
                    blue += f64::from(p[2]);
                }
                assert!(lit <= green + 1e-5, "H = {h}: {lit} green, a row composited twice (height {height}, amplitude {amplitude})");
                let interior = h <= grown - 1.5 || (whole && h >= grown + 0.5);
                if !interior {
                    continue;
                }
                exact_rows += 1;
                assert!((lit - green).abs() < 1e-5, "H = {h}: {lit} green, not one stamp (height {height}, amplitude {amplitude}): a gap");
                let k = stripe_of(v0, y);
                for (channel, want) in [(red, STRIPE_RED[k]), (blue, STRIPE_BLUE[k])] {
                    let expect = f64::from(srgb_decode(want)) * f64::from(TALL_OPACITY) * STRIPE_WIDTH as f64;
                    assert!((channel - expect).abs() < 1e-5, "H = {h}: stripe {k} expected (height {height}, amplitude {amplitude}): the column slipped a row");
                }
            }
        }
    }
    assert!(exact_rows > 200, "only {exact_rows} rows were checked exactly");
    println!("  shifted strips: {exact_rows} interior rows composited exactly once, budget {budget:.3} vs {:.3}", tall_bend_budget(&striped_tall()));
}

/// The spiretree earns its authored tip. Its dome is centred on the pivot and its trunk
/// leaves the two never-drawn end rows unpainted, so the family's measured budget now
/// admits the whole desired 0.9 px even for the windiest slot, where the shipped art of
/// 2026-09-12 admitted 0.27 (cap bound 0.30, trunk bound 0.47). A vined column is still
/// held to the vine's own budget.
#[test]
fn the_spiretree_column_is_admitted_its_whole_desired_tip() {
    let art = pack();
    let spire = art.tall_plant("spiretree").expect("spiretree");
    let vine = art.tall_plant(VINE_PLANT).expect("vine");
    let desired = wind_response("spiretree").tip_px;
    assert_eq!(desired, 0.9, "the authored response this test is about");
    let budget = tall_bend_budget(spire);
    println!("  spiretree budget {budget:.3} px, vine {:.3} px", tall_bend_budget(vine));
    assert!(
        budget >= desired * (1.0 + WIND_SLOT_VARIATION),
        "the spiretree's budget {budget} still clips its desired {desired} px"
    );
    assert_eq!(effective_tip(desired, budget), desired);
    // Where the room comes from: the cap's headroom and the trunk's, each above the tip.
    let cap = spire.cap.as_ref().unwrap();
    let top = f64::from(TALL_MAX_SEGMENTS);
    for (what, clip, base) in [
        ("cap", cap, tall_bend_base(top + 1.0)),
        ("trunk", &spire.trunk, tall_bend_base(top)),
    ] {
        let room = clip
            .frames
            .iter()
            .map(|f| f.bend_headroom(TALL_BEND_ROOT, TALL_BEND_LENGTH, base))
            .fold(f64::INFINITY, f64::min);
        println!("  spiretree {what} headroom {room:.3} px");
        assert!(room >= desired * (1.0 + WIND_SLOT_VARIATION), "{what} headroom {room}");
    }
    // The trunk's end rows are unpainted and the rest is the 4-periodic pattern.
    for frame in &spire.trunk.frames {
        for x in 0..frame.width() as i32 {
            assert_eq!(frame.texel(x, 0)[3], 0.0, "trunk row 0 painted");
            assert_eq!(frame.texel(x, 15)[3], 0.0, "trunk row 15 painted");
            for y in 1..11 {
                assert_eq!(frame.texel(x, y), frame.texel(x, y + 4), "not periodic at ({x}, {y})");
            }
        }
    }
    let presenter = ArtPresenter::new(pack());
    for column in tall_columns().into_iter().filter(|c| TALL_PLANTS[c.pick] == "spiretree") {
        let want = if column.vine { budget.min(tall_bend_budget(vine)) } else { budget };
        assert_eq!(presenter.column_budget(&column), want, "{column:?}");
    }
}

#[test]
fn a_small_plants_bend_is_the_breeze_projected_onto_its_own_heading() {
    let w = Vec2::new(-0.8, 0.3);
    for tip in [0.0, 0.2, 0.55] {
        for heading in [
            Vec2::new(1.0, 0.0),
            Vec2::new(-1.0, 0.0),
            Vec2::new(0.6, -0.8),
            Vec2::new(0.6, 0.8),
        ] {
            let bend = plant_bend(tip, w, heading);
            assert_eq!(bend.root, PLANT_BEND_ROOT);
            assert_eq!(bend.length, PLANT_BEND_LENGTH);
            assert_eq!(bend.base, 0.0, "a small plant stands on the root line");
            assert!(
                (bend.amplitude - tip * w.dot(heading)).abs() < 1e-12,
                "the amplitude is not the projected breeze"
            );
            assert!(bend.amplitude.abs() <= tip * w.length() + 1e-12);
            // The profile: zero at and below the root, the whole amplitude a length above it.
            assert_eq!(bend.profile(PLANT_BEND_ROOT), 0.0);
            assert_eq!(bend.profile(PLANT_BEND_ROOT - 3.0), 0.0);
            assert_eq!(bend.profile(PLANT_BEND_ROOT + PLANT_BEND_LENGTH), 1.0);
            for h in [2.0, 5.0, 9.0, 13.0] {
                assert!(
                    (bend.profile(h) - profile(PLANT_BEND_ROOT, PLANT_BEND_LENGTH, h)).abs()
                        < 1e-12,
                    "the profile at {h} is not the documented Hermite"
                );
            }
        }
    }
    // A heading square to the breeze feels nothing at all, whatever the tip.
    let across = Vec2::new(0.6, 0.8);
    let square = Vec2::new(-across.y, across.x);
    assert_eq!(plant_bend(0.7, across * 0.5, square).amplitude, 0.0);

    // A column takes one sample at its base, and its per-tile base is the documented `4i − 8`,
    // so base, trunk and cap sit on one global height coordinate.
    for i in [0.0, 1.0, 4.5, f64::from(TALL_MAX_SEGMENTS) + 1.0] {
        assert_eq!(tall_bend_base(i), 4.0 * i - 8.0);
    }
    let presenter = ArtPresenter::new(pack());
    let seconds = WIND_RISE + WIND_HOLD * 0.5;
    for column in tall_columns() {
        let budget = presenter.column_budget(&column);
        let amplitude = tall_amplitude(&column, budget, seconds);
        assert!(
            amplitude.abs() <= budget + 1e-12,
            "{column:?} bends by {amplitude}, over its budget {budget}"
        );
        // The column's own share of the breeze is a scale in the documented range.
        let share = tall_wind_of(column.face, column.cx);
        assert!((1.0 - WIND_SLOT_VARIATION..1.0 + WIND_SLOT_VARIATION).contains(&share));
    }
}
