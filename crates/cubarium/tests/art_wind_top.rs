//! Independent tests for the **reed on the top face** rule: a `reedspire` standing in a
//! flooded top-face cell is not radial — it is the same side-view tile lying along its own
//! heading — so it *bends* along that tile's horizontal axis exactly as it would on a side
//! face, rooted at its ripple row, while the two genuinely radial canopy species keep turning
//! in place.
//!
//! Written from the public doc comments of `cubarium::art_present` (`slot_wind` — "a slot on
//! the **top face** whose species is *radial* (`response.spin_deg > 0`) turns in place …
//! any other slot bends … this includes a reed standing in a flooded top-face cell",
//! `plant_bend`, `canopy_heading`, `PLANT_BEND_ROOT`, `PLANT_BEND_LENGTH`, `effective_tip`,
//! `plant_bend_budget`, `wind_at`, `wind_chart`, `wind_strength`, `WIND_RESPONSE`,
//! `WIND_PEAK_TICK`, `WIND_QUIET_TICK`, `Slot`, `slot_of`, `cell_band`, `REED_DEPTH`), of
//! `cubarium_render::sprite` (`Bend`, `Bend::displacement`, `Bend::is_identity`,
//! `stamp_layers_bent`) and from `art/README.md` "Wind" and the "Reed on the top face"
//! section of `design/7_Research/living-world-next-brief-2026-09-13.md` — never from their
//! bodies.
//!
//! The flooded cell is built the way `tests/art_water.rs` builds one: water deeper than
//! [`REED_DEPTH`] in exactly one cell, which is what makes that cell's band
//! [`Band::Water`] and its species `reedspire` at all. Every expected image is rebuilt a
//! second way — a presenter drawn from a pack with its plants removed supplies the floor, the
//! ground cover and the *water*, and the reed is hand-stamped on top through the public API —
//! so a difference between two of these images can only be the plant.

use std::path::{Path, PathBuf};

use cube_proto::{FACE_SIZE, Face};
use cubarium::art::{ArtPack, Band};
use cubarium::art_present::{
    ArtPresenter, PLANT_BEND_LENGTH, PLANT_BEND_ROOT, REED_DEPTH, REED_SCALE, REED_STAGES,
    WATER_PLANT, WIND_CHART_MAX, WIND_PEAK_TICK, WIND_QUIET_TICK, WIND_SLOT_VARIATION,
    WIND_TRAVEL_SECONDS, band_of, band_opacity, canopy_heading, cell_band, effective_tip,
    next_stage, placement_of, plant_bend, plant_bend_budget, plant_cap, plant_phase_of,
    present_seconds, slot_of, slot_wind, species_of, stage_opacity, stage_thresholds, up_of,
    wind_at, wind_chart, wind_phase, wind_response, wind_strength,
};
use cubarium::present::PRODUCER_SATURATION;
use cubarium_core::view::RenderView;
use cubarium_render::{Bend, Canvas, Mask, stamp_layers_bent};
use cubarium_surface::{CELL_COUNT, CellId, SurfacePoint, Vec2};

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

const PRODUCER_MAX: f64 = 10.0;
/// Deep enough to be a pool ([`REED_DEPTH`]) and past [`REED_STAGES`]`[2]`, so the slot's
/// band is [`Band::Water`] and its target stage is 2.
const POOL: f64 = 1.5;
/// The producer density that warrants a full-grown canopy plant.
const CANOPY_FULL: f64 = 0.70;

fn atelier() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")
}

fn pack() -> ArtPack {
    ArtPack::load(&atelier()).expect("the baked pack at assets/atelier must load")
}

/// The shipped pack without its tall plants: a column is not what these tests are about.
fn plants_only() -> ArtPack {
    let mut art = pack();
    art.tall.clear();
    art
}

/// The same pack with no plants at all: the background a hand-built stamp goes on top of.
/// It still draws the floor, the ramp, the soil, the ground cover and — the point, here —
/// the **water**, so a difference between this image and a drawn one is the plant alone.
fn no_plants() -> ArtPack {
    let mut art = plants_only();
    art.plants.clear();
    art
}

/// The pack with the reed removed and every other plant left in place: what the brief calls
/// "the same view with a pack whose `reedspire` plant is removed". A slot whose species the
/// pack does not carry draws nothing.
fn without_reed() -> ArtPack {
    let mut art = plants_only();
    art.plants.retain(|p| p.name != WATER_PLANT);
    assert!(art.plant(WATER_PLANT).is_none(), "the reed must be gone");
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

/// A world whose only pool is `cell`, `POOL` deep: exactly one reed in the picture.
fn flooded_view(tick: u64, cell: CellId) -> RenderView {
    let mut v = bare_view(tick);
    v.water[cell.index()] = POOL;
    v
}

/// A world whose only fed cell is `cell`, at `CANOPY_FULL` of the producer saturation.
fn lit_view(tick: u64, cell: CellId) -> RenderView {
    let mut v = bare_view(tick);
    v.producer[cell.index()] = CANOPY_FULL * saturation();
    v
}

/// A presenter snapped to `view`: the first observe takes the fields' targets as they are, so
/// the plant is idle and full-grown rather than part-way up a step.
fn snapped(art: ArtPack, view: &RenderView) -> ArtPresenter {
    let mut p = ArtPresenter::new(art);
    p.observe(view);
    p
}

fn draw(p: &mut ArtPresenter, v: &RenderView, f: f64) -> Canvas {
    let mut canvas = Canvas::new();
    p.draw(v, f, &mut canvas);
    canvas
}

/// The image at the same instant with no plant in it at all.
fn background(v: &RenderView, f: f64) -> Canvas {
    draw(&mut snapped(no_plants(), v), v, f)
}

// ---------------------------------------------------------------------------
// pixels
// ---------------------------------------------------------------------------

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL.into_iter().flat_map(|face| {
        (0..FACE_SIZE as u8).flat_map(move |y| (0..FACE_SIZE as u8).map(move |x| (face, x, y)))
    })
}

/// The pixels a 16-px tile anchored at `at` can reach, on `at`'s own face.
fn window(at: SurfacePoint) -> Vec<(Face, u8, u8)> {
    (0..FACE_SIZE as u8)
        .flat_map(|y| (0..FACE_SIZE as u8).map(move |x| (at.face, x, y)))
        .filter(|&(_, x, y)| {
            (f64::from(x) + 0.5 - at.u).hypot(f64::from(y) + 0.5 - at.v) <= 12.0
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

fn differing(a: &Canvas, b: &Canvas) -> Vec<(Face, u8, u8)> {
    every_pixel().filter(|&(f, x, y)| a.get(f, x, y) != b.get(f, x, y)).collect()
}

fn differing_at(a: &Canvas, b: &Canvas, pixels: &[(Face, u8, u8)]) -> Vec<(Face, u8, u8)> {
    pixels.iter().copied().filter(|&(f, x, y)| a.get(f, x, y) != b.get(f, x, y)).collect()
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

/// Tile coordinates of a face pixel centre for a 16×16 tile stamped at `at` with `heading`:
/// the tile's `+x` lies along the heading and its `+y` along the heading turned a quarter
/// turn, and the pivot — the anchor — is the tile centre `(8, 8)`. Only meaningful for
/// pixels on `at`'s own face, which is why every fixture cell here is interior to Top.
fn tile_at(at: SurfacePoint, heading: Vec2, x: u8, y: u8) -> Vec2 {
    let d = Vec2::new(f64::from(x) + 0.5 - at.u, f64::from(y) + 0.5 - at.v);
    let side = Vec2::new(-heading.y, heading.x);
    Vec2::new(heading.dot(d) + 8.0, side.dot(d) + 8.0)
}

/// A cell whose 16-px tile stays well inside one face: the anchor is within ±1 px of the cell
/// centre and a stamp reaches at most 9 px, so 3..=12 in both cell axes keeps every painted
/// pixel on the cell's own face — which is what makes [`tile_at`] and [`window`] meaningful.
fn interior(cell: CellId) -> bool {
    (3..=12).contains(&cell.cx()) && (3..=12).contains(&cell.cy())
}

// ---------------------------------------------------------------------------
// instants
// ---------------------------------------------------------------------------

/// The presentation instant of [`WIND_PEAK_TICK`]: inside a packet's hold, so the shared
/// breeze is at full envelope.
fn peak_seconds() -> f64 {
    let s = present_seconds(WIND_PEAK_TICK, 0.0);
    assert!(wind_strength(s) > 0.5, "{WIND_PEAK_TICK} is not inside a gust ({})", wind_strength(s));
    s
}

/// The presentation instant of [`WIND_QUIET_TICK`]: inside a packet's quiet interval, where
/// the sampler is **exactly** 0 for every root on the cube — the identity path.
fn quiet_seconds() -> f64 {
    let s = present_seconds(WIND_QUIET_TICK, 0.0);
    assert_eq!(wind_strength(s), 0.0, "{WIND_QUIET_TICK} is not exactly calm");
    s
}

// ---------------------------------------------------------------------------
// the fixture cells
// ---------------------------------------------------------------------------

/// The top-face cell this file floods: a rank-2 slot, interior to Top, whose reed feels as
/// much of the gust *along its own tile's horizontal axis* as any — the amplitude is the
/// breeze **projected onto the heading**, so a slot whose hashed heading happens to lie
/// across the local wind would prove nothing however hard it blew.
///
/// Top's centre, its four vertices and its side seams are exactly calm by construction
/// ([`wind_chart`]), so the search maximizes rather than assumes.
fn reed_cell() -> CellId {
    let art = pack();
    let plant = art.plant(WATER_PLANT).expect("the pack carries the reed");
    let budget = plant_bend_budget(plant);
    let response = wind_response(WATER_PLANT);
    let seconds = peak_seconds();
    let best = CellId::all()
        .filter(|&c| {
            c.face() == Face::Top && interior(c) && plant_cap(Band::Water, c) == Some(2)
        })
        .map(|c| {
            let slot = slot_of(c);
            let w = wind_at(slot.at, seconds, response.lag_seconds);
            let tip = effective_tip(response.tip_px, budget) * slot.wind;
            (plant_bend(tip, w, slot.heading).amplitude.abs(), c)
        })
        .fold((0.0f64, None), |m, v| if v.0 > m.0 { (v.0, Some(v.1)) } else { m });
    let cell = best.1.expect("the cube has an interior rank-2 slot on Top");
    println!("  reed at {cell:?}: |amplitude| {:.4} px at the gust peak", best.0);
    assert!(
        best.0 > 0.2,
        "the windiest interior Top slot bends by only {:.4} px, which cannot move a texel",
        best.0
    );
    // The flooding really does make it a reed whose rank allows stage 2.
    assert_eq!(cell_band(cell, Some(POOL)), Band::Water, "the pool must change the band");
    assert_eq!(species_of(Band::Water, cell), WATER_PLANT, "a pool grows a reed");
    assert_eq!(
        next_stage(None, POOL / REED_SCALE, &stage_thresholds(Band::Water), 2),
        Some(2),
        "{POOL} deep must warrant a full-grown reed"
    );
    assert!(POOL > REED_DEPTH, "{POOL} is not deeper than the reed line");
    assert_eq!(REED_STAGES, stage_thresholds(Band::Water), "the water band's thresholds");
    assert!(up_of(cell).is_none(), "a top-face cell has no 'up toward the canopy'");
    cell
}

/// A rank-2 slot of a *radial* species, interior to Top, that feels as much of the breeze as
/// any: the canopy control for the reed.
fn canopy_cell(species: &str) -> CellId {
    let response = wind_response(species);
    assert!(response.spin_deg > 0.0, "{species} is not a radial species");
    let seconds = peak_seconds();
    let best = CellId::all()
        .filter(|&c| {
            c.face() == Face::Top
                && interior(c)
                && band_of(c) == Band::Canopy
                && plant_cap(Band::Canopy, c) == Some(2)
                && species_of(Band::Canopy, c) == species
        })
        .map(|c| {
            let slot = slot_of(c);
            (wind_at(slot.at, seconds, response.lag_seconds).length(), c)
        })
        .fold((0.0f64, None), |m, v| if v.0 > m.0 { (v.0, Some(v.1)) } else { m });
    let cell = best.1.unwrap_or_else(|| panic!("no interior rank-2 {species} slot on Top"));
    println!("  {species} at {cell:?}: |w| {:.4} at the gust peak", best.0);
    assert!(best.0 > 0.05, "{species}'s windiest interior Top slot feels only {:.4}", best.0);
    cell
}

// ---------------------------------------------------------------------------
// hand-built stamps
// ---------------------------------------------------------------------------

/// The documented image of one idle full-grown plant of `species` in `cell`, hand-built on
/// `bg`: the stage's own sway pose at the slot's phase, whole, unmasked, at the band's
/// opacity ceiling, with the slot's `(Bend, heading)`.
fn expected_plant(
    art: &ArtPack,
    bg: &Canvas,
    cell: CellId,
    band: Band,
    species: &str,
    density: f64,
    seconds: f64,
    wind: (Bend, Vec2),
) -> Canvas {
    let plant = art.plant(species).unwrap_or_else(|| panic!("{species} is in the pack"));
    let slot = slot_of(cell);
    let (at, heading) = placement_of(cell);
    assert_eq!(at, slot.at, "the slot's anchor is where the plant stands");
    assert_eq!(heading, slot.heading, "the slot's heading is the plant's authored one");
    let clip = &plant.stages[2];
    let pose = clip.sample(seconds + plant_phase_of(cell, clip.seconds));
    let opacity =
        stage_opacity(2, density, &stage_thresholds(band), band_opacity(band));
    assert_eq!(opacity, band_opacity(band), "a full-grown plant is at its band's ceiling");
    let mut canvas = bg.clone();
    stamp_layers_bent(
        &mut canvas,
        slot.at,
        wind.1,
        &[(pose, 1.0)],
        1.0,
        opacity,
        Mask::None,
        wind.0,
        &mut Vec::new(),
    );
    canvas
}

/// The reed's own stamp over a plant-free background, at `wind`.
fn expected_reed(
    art: &ArtPack,
    bg: &Canvas,
    cell: CellId,
    seconds: f64,
    wind: (Bend, Vec2),
) -> Canvas {
    expected_plant(art, bg, cell, Band::Water, WATER_PLANT, POOL / REED_SCALE, seconds, wind)
}

// ---------------------------------------------------------------------------
// 1. the rule itself
// ---------------------------------------------------------------------------

/// **The rule.** A top-face `reedspire` has a tip (0.70 px) and **no spin**, so [`slot_wind`]
/// takes the bending branch, not the radial one: the returned bend is exactly
/// [`plant_bend`]`(effective_tip(tip, budget) · slot.wind, wind_at(slot.at, seconds, lag),
/// slot.heading)` — rooted at [`PLANT_BEND_ROOT`] over [`PLANT_BEND_LENGTH`], base 0 — and the
/// heading is the slot's own, unrotated, bit for bit. In the quiet interval the sampler is
/// exactly 0, so the amplitude is exactly 0 and the stamp takes the renderer's identity path.
///
/// Every number is recomputed here from the doc comments (`wind_chart`'s formula, `wind_at`'s
/// lag and travel, `effective_tip`'s `min(tip, budget / (1 + WIND_SLOT_VARIATION))`), so an
/// implementation that quietly fell back to a canopy rotation, forgot the slot's variation, or
/// projected onto the wrong axis would fail here rather than only in an image.
#[test]
fn slot_wind_bends_a_top_face_reed_along_its_own_heading_and_rests_exactly_when_the_packet_does() {
    let art = pack();
    let plant = art.plant(WATER_PLANT).expect("the pack carries the reed");
    let budget = plant_bend_budget(plant);
    let response = wind_response(WATER_PLANT);
    assert!(response.tip_px > 0.0, "a reed that wants no tip travel cannot bend");
    assert_eq!(response.spin_deg, 0.0, "a reed is not radial; only a spin turns in place");

    let cell = reed_cell();
    let slot = slot_of(cell);
    assert_eq!(slot.at.face, Face::Top, "the fixture reed must stand on the top face");

    // At the gust peak: the documented bend, and the slot's own heading.
    let seconds = peak_seconds();
    let (bend, heading) = slot_wind(&slot, WATER_PLANT, budget, seconds);
    let w = wind_at(slot.at, seconds, response.lag_seconds);
    // `wind_at`, recomputed from its own doc comment: the chart field at the root, times the
    // shared sampler at a time shifted by the species' lag and the root's spatial phase.
    let when = seconds - response.lag_seconds - WIND_TRAVEL_SECONDS * wind_phase(slot.at);
    let want_w = wind_chart(slot.at.face, slot.at.u, slot.at.v) * wind_strength(when);
    assert!(
        (w - want_w).length() < 1e-12,
        "wind_at is {w:?}, not the documented {want_w:?}"
    );
    assert!(w.length() <= WIND_CHART_MAX + 1e-12, "the breeze escaped its chart maximum");
    let tip = effective_tip(response.tip_px, budget) * slot.wind;
    assert_eq!(
        effective_tip(response.tip_px, budget),
        response.tip_px.min(budget / (1.0 + WIND_SLOT_VARIATION)).max(0.0),
        "the admitted family tip is not the documented min"
    );
    assert_eq!(bend, plant_bend(tip, w, slot.heading), "the reed did not take the plant bend");
    assert_eq!(bend.root, PLANT_BEND_ROOT, "a reed is rooted at its ripple row");
    assert_eq!(bend.length, PLANT_BEND_LENGTH, "a reed takes the small-plant bend length");
    assert_eq!(bend.base, 0.0, "a small plant's tile stands on the root line");
    assert!(
        (bend.amplitude - tip * w.dot(slot.heading)).abs() < 1e-15,
        "the amplitude is not the breeze projected onto the tile's own horizontal axis"
    );
    assert!(!bend.is_identity(), "the reed must really bend at the gust peak: {bend:?}");
    assert!(bend.amplitude.abs() > 0.2, "the reed bends only {} px", bend.amplitude);
    assert_eq!(heading, slot.heading, "a bending reed is never turned; it is not radial");
    // And the root line really is fixed while the tip really moves.
    assert_eq!(
        bend.displacement(16.0, 16.0 - PLANT_BEND_ROOT),
        0.0,
        "the root row must not be displaced at all"
    );
    assert!(
        bend.displacement(16.0, 16.0 - PLANT_BEND_ROOT - 4.0).abs() > 0.0,
        "four pixels above the root nothing moves, so the fixture proves nothing"
    );
    println!("  peak: bend {bend:?}, heading {heading:?}");

    // In the quiet interval: the identity, and the authored heading unchanged.
    let calm = quiet_seconds();
    let (bend, heading) = slot_wind(&slot, WATER_PLANT, budget, calm);
    assert_eq!(
        wind_at(slot.at, calm, response.lag_seconds),
        Vec2::ZERO,
        "the shared quiet interval must reach this root too"
    );
    assert!(bend.is_identity(), "a calm reed must take the identity path, not {bend:?}");
    assert_eq!(bend.amplitude, 0.0, "a calm reed's amplitude must be exactly 0");
    assert_eq!(bend.displacement(16.0, 0.0), 0.0, "the identity displaces nothing anywhere");
    assert_eq!(heading, slot.heading, "a calm reed keeps its authored heading bit for bit");
    println!("  quiet: bend {bend:?}, heading {heading:?}");
}

// ---------------------------------------------------------------------------
// 2. the picture
// ---------------------------------------------------------------------------

/// **The picture.** At the gust peak the reed's pixels *move* — the drawn cube is not the
/// windless one — and it is exactly the hand-built bent stamp over the plant-free image; the
/// pixels along its **root line** are bit-identical to the windless stamp, because the bend's
/// profile is exactly 0 at and below [`PLANT_BEND_ROOT`] and a root displaced by a hundredth
/// of a pixel would still resample and skate; and nothing is painted below the root at all.
///
/// On the top face the "bottom rows" of the tile are not the low rows of the *face*: the tile
/// lies along the slot's own heading, so the root line is derived from
/// [`Slot::at`](cubarium::art_present::Slot::at), [`Slot::heading`] and the tile's geometry —
/// the destination pixels whose own tile row lies within the bilinear support of rows 14 and
/// 15, i.e. 1.5 px of the tile's bottom edge.
#[test]
fn a_flooded_top_face_reed_moves_at_a_gust_but_its_root_line_is_bit_identical() {
    let art = plants_only();
    let plant = art.plant(WATER_PLANT).expect("the reed");
    let budget = plant_bend_budget(plant);
    let cell = reed_cell();
    let slot = slot_of(cell);
    let near = window(slot.at);

    let v = flooded_view(WIND_PEAK_TICK, cell);
    let f = 0.0;
    let seconds = present_seconds(v.tick, f);
    assert_eq!(seconds, peak_seconds(), "the fixture instant must be the gust peak");
    let (bend, heading) = slot_wind(&slot, WATER_PLANT, budget, seconds);
    assert!(!bend.is_identity(), "the fixture instant must be windy");

    let mut p = snapped(plants_only(), &v);
    assert!(
        p.stage_of(cell) == Some(2),
        "the snapped fixture must be a full-grown reed, not {:?}",
        p.stage_of(cell)
    );
    let actual = draw(&mut p, &v, f);
    let bg = background(&v, f);

    // The drawn cube *is* the documented stamp: one bent stage-2 pose over the plant-free
    // image, which already carries the water, the ground cover and the floor.
    let windy = expected_reed(&art, &bg, cell, seconds, (bend, heading));
    assert_same_canvas(&actual, &windy, "the bent reed on the top face");

    // The windless image at the same instant, so only the bend differs.
    let calm = expected_reed(&art, &bg, cell, seconds, (Bend::NONE, slot.heading));
    assert!(
        max_diff_at(&actual, &calm, &near) > 0.0,
        "the reed looks the same windy and calm: the top-face rule moves nothing"
    );
    let moved = differing_at(&actual, &calm, &near);
    println!("  the gust moved {} of the reed's {} window pixels", moved.len(), near.len());
    assert!(moved.len() >= 4, "only {} pixels moved; that is not a bend", moved.len());

    // The root line: destination pixels whose tile row lies in 14.5..15.5, whose whole
    // bilinear support is at or below the root line and whose displacement is therefore
    // exactly zero. And the rows below the plant, which nothing may paint.
    let root_line: Vec<(Face, u8, u8)> = near
        .iter()
        .copied()
        .filter(|&(_, x, y)| (14.5..15.5).contains(&tile_at(slot.at, slot.heading, x, y).y))
        .collect();
    let below: Vec<(Face, u8, u8)> = near
        .iter()
        .copied()
        .filter(|&(_, x, y)| tile_at(slot.at, slot.heading, x, y).y >= 15.5)
        .collect();
    assert!(root_line.len() >= 8, "the fixture found only {} root-line pixels", root_line.len());
    assert!(below.len() >= 8, "the fixture found only {} pixels below the reed", below.len());
    for &(fc, x, y) in &root_line {
        let h = 16.0 - tile_at(slot.at, slot.heading, x, y).y;
        assert_eq!(
            bend.profile(h),
            0.0,
            "({fc:?}, {x}, {y}) is {h} above the root line and is not fixed"
        );
    }
    let skated = max_diff_at(&actual, &calm, &root_line);
    assert!(
        skated == 0.0,
        "the reed's root line differs from the windless image by {skated} — the contact is \
         being displaced along the top face"
    );
    assert!(
        differing_at(&actual, &calm, &root_line).is_empty(),
        "the reed's root line is not bit-identical windy or calm"
    );
    // The root line is *painted*, or the assertion above is about empty water.
    let painted = differing_at(&actual, &bg, &root_line);
    assert!(
        !painted.is_empty(),
        "the reed paints nothing on its root line, so 'the root does not skate' is vacuous"
    );
    println!("  {} root-line pixels painted, all bit-identical windy or calm", painted.len());
    let leak = max_diff_at(&actual, &bg, &below);
    assert!(leak == 0.0, "the reed painted {leak} below its own root line");
}

/// The gust moves the **reed** and nothing else: every pixel that differs between the drawn
/// windy cube and the windless one is inside one stamp's reach of the reed's own anchor, so
/// the water film, the shimmer, the ground cover and the floor of that cell — and every other
/// cell of the cube — are the same picture windy or calm.
///
/// The comparison is against a pack whose `reedspire` plant is **removed**, which is the
/// brief's own recipe for "the same view with the reed's slot excluded": that image contains
/// the water of the flooded cell and everything else the frame draws, and differs from the
/// drawn one only where the reed paints.
#[test]
fn the_gust_moves_only_the_reeds_own_pixels_not_the_water_or_the_ground_under_it() {
    let art = plants_only();
    let budget = plant_bend_budget(art.plant(WATER_PLANT).expect("the reed"));
    let cell = reed_cell();
    let slot = slot_of(cell);
    let near = window(slot.at);

    let v = flooded_view(WIND_PEAK_TICK, cell);
    let f = 0.0;
    let seconds = present_seconds(v.tick, f);
    let (bend, heading) = slot_wind(&slot, WATER_PLANT, budget, seconds);
    assert!(!bend.is_identity(), "the fixture instant must be windy");

    let actual = draw(&mut snapped(plants_only(), &v), &v, f);
    let reedless = draw(&mut snapped(without_reed(), &v), &v, f);
    let bg = background(&v, f);
    // Removing the one plant the picture has is the same as removing them all, which is what
    // makes the plant-free background a legitimate stand-in for the reed's own exclusion.
    assert_same_canvas(&reedless, &bg, "a pack without the reed drew some other plant");
    // The flooded cell really does have water in it under the reed.
    let dry = bare_view(v.tick);
    let dry_image = draw(&mut snapped(no_plants(), &dry), &dry, f);
    assert!(
        !differing_at(&bg, &dry_image, &near).is_empty(),
        "the fixture cell has no water in it, so there is nothing for the wind to leave alone"
    );

    let calm = expected_reed(&art, &bg, cell, seconds, (Bend::NONE, slot.heading));
    let moved = differing(&actual, &calm);
    assert!(!moved.is_empty(), "the gust moved nothing at all");
    for &(fc, x, y) in &moved {
        assert_eq!(fc, slot.at.face, "the gust changed a pixel on {fc:?}, off the reed's face");
        let d = (f64::from(x) + 0.5 - slot.at.u).hypot(f64::from(y) + 0.5 - slot.at.v);
        assert!(
            d <= 9.5,
            "the gust changed ({x}, {y}), {d:.2} px from the reed's anchor — outside one \
             stamp's nine-pixel reach, so something other than the plant moved"
        );
    }
    // The water and ground the reed does not cover are the plant-free image in *both*, so the
    // breeze reached neither: count them so the claim is not about an empty set.
    let untouched: Vec<(Face, u8, u8)> = near
        .iter()
        .copied()
        .filter(|&(fc, x, y)| {
            actual.get(fc, x, y) == bg.get(fc, x, y) && calm.get(fc, x, y) == bg.get(fc, x, y)
        })
        .collect();
    assert!(
        untouched.len() > near.len() / 2,
        "only {} of {} window pixels are untouched water and ground",
        untouched.len(),
        near.len()
    );
    println!(
        "  {} pixels moved, all within 9.5 px of the anchor on {:?}; {} window pixels of water \
         and ground untouched",
        moved.len(),
        slot.at.face,
        untouched.len()
    );
    assert_eq!(heading, slot.heading, "the reed's heading must not have turned");
}

/// A calm instant is the image from **before** this rule existed, bit for bit: the reed takes
/// the renderer's identity path, so the whole cube is the windless picture — the hand-built
/// stamp with [`Bend::NONE`] and the slot's authored heading.
#[test]
fn a_calm_instant_draws_the_windless_top_face_reed_bit_for_bit() {
    let art = plants_only();
    let budget = plant_bend_budget(art.plant(WATER_PLANT).expect("the reed"));
    let cell = reed_cell();
    let slot = slot_of(cell);

    let v = flooded_view(WIND_QUIET_TICK, cell);
    let f = 0.0;
    let seconds = present_seconds(v.tick, f);
    assert_eq!(seconds, quiet_seconds(), "the fixture instant must be exactly calm");
    let (bend, heading) = slot_wind(&slot, WATER_PLANT, budget, seconds);
    assert!(bend.is_identity(), "the fixture instant must be calm, not {bend:?}");
    assert_eq!(heading, slot.heading);

    let actual = draw(&mut snapped(plants_only(), &v), &v, f);
    let bg = background(&v, f);
    let windless = expected_reed(&art, &bg, cell, seconds, (Bend::NONE, slot.heading));
    assert_same_canvas(&actual, &windless, "the calm top-face reed");
    assert!(
        max_diff_at(&actual, &bg, &window(slot.at)) > 0.02,
        "the calm fixture draws no reed at all"
    );
}

// ---------------------------------------------------------------------------
// 3. the canopy control
// ---------------------------------------------------------------------------

/// The two genuinely **radial** species on the top face are untouched by the reed rule: they
/// still answer with [`Bend::NONE`] and a heading turned by `θ = spin_deg · slot.wind · |w| /
/// `[`WIND_CHART_MAX`] radians about the *stationary* tile centre. Turning in place, never
/// translated: the drawn cube is the same pose stamped at the **same anchor** with the turned
/// heading, which a plant carried sideways — by a bend, or by a moved anchor — cannot be.
///
/// (The tile centre is the pivot and the anchor, which is what "no translation" means here.
/// Its *nearest destination pixel* is not bit-identical and must not be asserted to be: that
/// pixel sits up to 0.71 px from the pivot, so a 2° turn still resamples it by about a
/// hundredth of a pixel. What cannot move is the anchor, and that is what is checked.)
#[test]
fn the_two_canopy_species_on_top_still_turn_in_place_and_are_never_carried_sideways() {
    let art = plants_only();
    for species in ["umbrellafrond", "bloomcrown"] {
        let response = wind_response(species);
        let plant = art.plant(species).unwrap_or_else(|| panic!("{species} is in the pack"));
        let budget = plant_bend_budget(plant);
        let cell = canopy_cell(species);
        let slot = slot_of(cell);
        assert!(up_of(cell).is_none(), "{species}: a top-face cell is radial");

        // At the gust peak: no bend, a turned heading, the documented angle.
        let v = lit_view(WIND_PEAK_TICK, cell);
        let f = 0.0;
        let seconds = present_seconds(v.tick, f);
        let (bend, heading) = slot_wind(&slot, species, budget, seconds);
        let w = wind_at(slot.at, seconds, response.lag_seconds);
        assert_eq!(bend, Bend::NONE, "{species}: a radial plant must never be bent");
        assert_eq!(
            heading,
            canopy_heading(slot.heading, response.spin_deg * slot.wind, w),
            "{species}: not the documented rotation"
        );
        assert!((heading.length() - 1.0).abs() < 1e-12, "{species}: the heading lost its length");
        let turned = heading.screen_angle() - slot.heading.screen_angle();
        let want = (response.spin_deg * slot.wind * w.length() / WIND_CHART_MAX).to_radians();
        assert!(
            (turned.rem_euclid(std::f64::consts::TAU) - want.rem_euclid(std::f64::consts::TAU))
                .abs()
                < 1e-9,
            "{species}: turned {turned} rad, not the documented {want}"
        );
        assert!(want > 1e-4, "{species}: the fixture barely turns ({want} rad)");
        assert_ne!(heading, slot.heading, "{species}: the gust turned the crown by nothing");

        // The drawn cube: the same pose, at the same anchor, with the turned heading. A
        // translated crown — or a bent one — differs from this.
        let mut p = snapped(plants_only(), &v);
        assert_eq!(p.stage_of(cell), Some(2), "{species}: the fixture must be full-grown");
        let actual = draw(&mut p, &v, f);
        let bg = background(&v, f);
        let expected = expected_plant(
            &art,
            &bg,
            cell,
            Band::Canopy,
            species,
            CANOPY_FULL,
            seconds,
            (Bend::NONE, heading),
        );
        assert_same_canvas(&actual, &expected, &format!("{species} turning in place on Top"));
        let unturned = expected_plant(
            &art,
            &bg,
            cell,
            Band::Canopy,
            species,
            CANOPY_FULL,
            seconds,
            (Bend::NONE, slot.heading),
        );
        assert!(
            !differing(&actual, &unturned).is_empty(),
            "{species}: the gust turned nothing visible, so the fixture proves nothing"
        );
        println!(
            "  {species}: turned {:.5} rad, {} pixels differ from the unturned crown",
            turned,
            differing(&actual, &unturned).len()
        );

        // And at a calm instant the authored heading comes back bit for bit.
        let quiet = lit_view(WIND_QUIET_TICK, cell);
        let calm_seconds = present_seconds(quiet.tick, f);
        let (bend, heading) = slot_wind(&slot, species, budget, calm_seconds);
        assert_eq!(bend, Bend::NONE, "{species}: still never bent");
        assert_eq!(
            heading, slot.heading,
            "{species}: a calm crown must keep its authored heading bit for bit"
        );
        assert_eq!(
            canopy_heading(slot.heading, response.spin_deg, Vec2::ZERO),
            slot.heading,
            "{species}: an exactly-zero angle must return the heading itself"
        );
        let actual = draw(&mut snapped(plants_only(), &quiet), &quiet, f);
        let bg = background(&quiet, f);
        let windless = expected_plant(
            &art,
            &bg,
            cell,
            Band::Canopy,
            species,
            CANOPY_FULL,
            calm_seconds,
            (Bend::NONE, slot.heading),
        );
        assert_same_canvas(&actual, &windless, &format!("{species} at exact calm"));
    }
}
