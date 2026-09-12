//! Independent tests for the three strata of the live art mode, written from
//! `design/stratified-world.md` "Presentation" and the public doc comments of
//! `cubarium::art_present` rather than from the implementation.
//!
//! Everything here goes through the public API. Where the expected image is needed it is
//! rebuilt here from the normative rules — the night floor and the producer ramp through
//! `cubarium::present`'s own helpers, the soil ground from its stated formula and
//! `cubarium_surface`'s seam neighbors — so an implementation that quietly changed a
//! ramp, a filter or a weight has to survive a second, independently written copy of the
//! rule rather than agreeing with itself.

use cube_proto::{FACE_SIZE, Face};
use cubarium::art::ArtPack;
use cubarium::art_present::{
    ArtPresenter, Band, CANOPY_STAGES, HORIZON, SOIL_HIGH_SRGB, SOIL_LOW_SRGB,
    SOIL_MAX_BRIGHTNESS, SOIL_MIN_BRIGHTNESS, SOIL_SCALE, SOIL_TOP, band_of, band_of_height,
    cell_band, height_of, next_stage, plant_density, rank_cap_of, soil_weight,
    stage_thresholds, w_soil,
};
use cubarium::present::{
    self, DETRITUS_SCALE, DETRITUS_THRESHOLD, PALETTE, PRODUCER_SATURATION, srgb_linear,
};
use cubarium_core::view::RenderView;
use cubarium_render::{Canvas, draw_field};
use cubarium_surface::{
    CELL_COUNT, CELLS_PER_FACE_EDGE, CellId, Edge, ScalarField, SurfacePoint, Vec2, cell_of,
    pixel_neighbor,
};

use std::path::Path;

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

const PRODUCER_MAX: f64 = 10.0;

fn pack() -> ArtPack {
    ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
        .expect("the baked pack at assets/atelier must load")
}

fn saturation() -> f64 {
    PRODUCER_MAX * PRODUCER_SATURATION
}

fn view(producer: Vec<f64>, detritus: Vec<f64>) -> RenderView {
    assert_eq!(producer.len(), CELL_COUNT);
    assert_eq!(detritus.len(), CELL_COUNT);
    RenderView {
        tick: 0,
        producer,
        detritus,
        fruit: vec![0.0; CELL_COUNT],
        water: vec![0.0; CELL_COUNT],
        rain: vec![0.0; CELL_COUNT],
        producer_max: PRODUCER_MAX,
        organisms: Vec::new(),
    }
}

fn flat(v: f64) -> Vec<f64> {
    vec![v; CELL_COUNT]
}

fn draw(v: &RenderView) -> Canvas {
    let mut canvas = Canvas::new();
    ArtPresenter::new(pack()).draw(v, 0.0, &mut canvas);
    canvas
}

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL.into_iter().flat_map(|face| {
        (0..FACE_SIZE as u8).flat_map(move |y| (0..FACE_SIZE as u8).map(move |x| (face, x, y)))
    })
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

/// The four faces that are not the canopy.
fn side_faces() -> impl Iterator<Item = Face> {
    Face::ALL.into_iter().filter(|&f| f != Face::Top)
}

// ---------------------------------------------------------------------------
// the expected image, rebuilt from the normative rules
// ---------------------------------------------------------------------------

/// The seam-filtered value of a field at a pixel: the pixel's own cell with weight 4 and
/// each existing pixel-neighbor's cell with weight 1, normalized over the neighbors that
/// exist. Written out again here rather than borrowed from the renderer.
fn filtered(field: &ScalarField, face: Face, x: u8, y: u8) -> f64 {
    let mut sum = field.get(cell_of(&SurfacePoint::pixel_center(face, x, y))) * 4.0;
    let mut divisor = 4.0;
    for edge in Edge::ALL {
        if let Some((nf, nx, ny)) = pixel_neighbor(face, x, y, edge) {
            sum += field.get(cell_of(&SurfacePoint::pixel_center(nf, nx, ny)));
            divisor += 1.0;
        }
    }
    sum / divisor
}

/// The whole ground of the art image — floor, the foliage/canopy layers faded out by the
/// horizon, and the soil ground faded in by it — with no motifs and no bodies.
fn expected_ground(v: &RenderView) -> Canvas {
    let mut canvas = Canvas::new();
    present::draw_floor(&mut canvas);

    let producer = field_from(&v.producer);
    let mut flecks = field_from(&v.detritus);
    for value in flecks.values.iter_mut() {
        if *value <= DETRITUS_THRESHOLD {
            *value = 0.0;
        }
    }
    let detritus = field_from(&v.detritus);

    // The decided producer ramp and the decided flecks, each scaled by `1 - w_soil`.
    let mut layer = Canvas::new();
    present::draw_ramp_field(
        &mut layer,
        &producer,
        v.producer_max * PRODUCER_SATURATION,
        PALETTE.producer_low,
        PALETTE.producer_high,
        true,
    );
    add_scaled(&mut canvas, &layer, |f, x, y| 1.0 - soil_weight(f, x, y));
    let mut layer = Canvas::new();
    draw_field(&mut layer, &flecks, DETRITUS_SCALE, PALETTE.detritus, false);
    add_scaled(&mut canvas, &layer, |f, x, y| 1.0 - soil_weight(f, x, y));

    // The soil ground: a seam-filtered detritus ramp, linear brightness, scaled by
    // `w_soil`, and painting even at zero detritus.
    let (low, high) = (srgb_linear(SOIL_LOW_SRGB), srgb_linear(SOIL_HIGH_SRGB));
    for (face, x, y) in every_pixel() {
        let w = soil_weight(face, x, y);
        if w <= 0.0 {
            continue;
        }
        let t = (filtered(&detritus, face, x, y) / SOIL_SCALE).clamp(0.0, 1.0) as f32;
        let c = present::mix(low, high, t);
        let b = (SOIL_MIN_BRIGHTNESS + (SOIL_MAX_BRIGHTNESS - SOIL_MIN_BRIGHTNESS) * t) * w;
        canvas.add(face, x, y, [c[0] * b, c[1] * b, c[2] * b]);
    }
    canvas
}

fn field_from(values: &[f64]) -> ScalarField {
    let mut field = ScalarField::zeros();
    field.values.copy_from_slice(values);
    field
}

fn add_scaled(canvas: &mut Canvas, layer: &Canvas, weight: impl Fn(Face, u8, u8) -> f32) {
    for (face, x, y) in every_pixel() {
        let k = weight(face, x, y);
        if k <= 0.0 {
            continue;
        }
        let c = layer.get(face, x, y);
        if c == [0.0; 3] {
            continue;
        }
        canvas.add(face, x, y, [c[0] * k, c[1] * k, c[2] * k]);
    }
}

// ---------------------------------------------------------------------------
// where the bands are
// ---------------------------------------------------------------------------

#[test]
fn soil_is_the_bottom_five_cell_rows_of_every_side_face_and_no_top_cell() {
    // The geometry: a side face's cell row `cy` has its center at h = 1 - (cy + 0.5)/8,
    // so rows 11..=15 are below -0.33 and rows 0..=10 are not. Five of sixteen.
    for face in side_faces() {
        let mut soil_rows = Vec::new();
        for cy in 0..CELLS_PER_FACE_EDGE as u8 {
            let mut band = None;
            for cx in 0..CELLS_PER_FACE_EDGE as u8 {
                let cell = CellId::new(face, cx, cy);
                let h = height_of(cell);
                // Every cell in a row of a side face sits at the same height.
                let expected = 1.0 - (f64::from(cy) + 0.5) / 8.0;
                assert!((h - expected).abs() < 1e-12, "{cell:?}: h = {h}, expected {expected}");
                let b = band_of(cell);
                assert_eq!(*band.get_or_insert(b), b, "{face:?} row {cy} is not one band");
                assert_ne!(b, Band::Canopy, "{cell:?} is not on the top face");
            }
            if band == Some(Band::Soil) {
                soil_rows.push(cy);
            }
        }
        assert_eq!(
            soil_rows,
            vec![11, 12, 13, 14, 15],
            "{face:?}: SOIL_TOP = {SOIL_TOP} must put exactly the bottom five of sixteen \
             cell rows in the soil"
        );
    }

    // And the top face is the canopy, all of it, at exactly h = 1.
    for cell in CellId::all().filter(|c| c.face() == Face::Top) {
        assert_eq!(height_of(cell), 1.0, "{cell:?}");
        assert_eq!(band_of(cell), Band::Canopy, "{cell:?}");
    }

    // The band rule itself, at its two edges.
    assert_eq!(band_of_height(SOIL_TOP - 1e-9), Band::Soil);
    assert_eq!(band_of_height(SOIL_TOP), Band::Foliage, "the soil rule is strict");
    assert_eq!(band_of_height(0.9999), Band::Foliage);
    assert_eq!(band_of_height(1.0), Band::Canopy);
}

#[test]
fn w_soil_is_one_below_the_horizon_zero_above_it_and_monotone_between() {
    assert_eq!(w_soil(SOIL_TOP - HORIZON), 1.0, "the bottom of the blend is wholly soil");
    assert_eq!(w_soil(SOIL_TOP + HORIZON), 0.0, "the top of the blend is wholly foliage");
    assert_eq!(w_soil(-1.0), 1.0, "the rim is wholly soil");
    assert_eq!(w_soil(1.0), 0.0, "the canopy has no soil in it");
    assert!((w_soil(SOIL_TOP) - 0.5).abs() < 1e-12, "the blend is centered on SOIL_TOP");
    assert_eq!(w_soil(f64::NAN), 0.0, "a NaN height is not soil");

    // Monotone (non-increasing) in h over the whole cube, and strictly decreasing
    // somewhere inside the blend.
    let mut previous = 1.0;
    let mut moved = false;
    for k in 0..=4000 {
        let h = -1.0 + 2.0 * f64::from(k) / 4000.0;
        let w = w_soil(h);
        assert!((0.0..=1.0).contains(&w), "w_soil({h}) = {w}");
        assert!(w <= previous + 1e-15, "w_soil rose from {previous} to {w} at h = {h}");
        if w < previous {
            moved = true;
        }
        previous = w;
    }
    assert!(moved, "w_soil never changed: the horizon is not a blend at all");
}

#[test]
fn the_horizon_is_the_same_row_on_all_four_side_faces_and_absent_from_the_top() {
    for y in 0..FACE_SIZE as u8 {
        let front = soil_weight(Face::Front, 0, y);
        for face in side_faces() {
            for x in 0..FACE_SIZE as u8 {
                assert_eq!(
                    soil_weight(face, x, y),
                    front,
                    "{face:?} ({x}, {y}) has a different soil weight than Front row {y}: \
                     the horizon is not one line around the cube"
                );
            }
            // And it is the pixel's own height, not its cell's.
            let h = SurfacePoint::pixel_center(face, 0, y).embed()[1];
            assert_eq!(front, w_soil(h) as f32, "row {y} of {face:?}");
        }
        for x in 0..FACE_SIZE as u8 {
            assert_eq!(soil_weight(Face::Top, x, y), 0.0, "the canopy has no soil");
        }
    }

    // The blend really is per pixel: the transition takes several pixel rows, and the
    // rows it spans are strictly inside one cell row's worth of the image.
    let partial: Vec<u8> = (0..FACE_SIZE as u8)
        .filter(|&y| {
            let w = soil_weight(Face::Front, 0, y);
            w > 0.0 && w < 1.0
        })
        .collect();
    assert!(
        partial.len() >= 2,
        "the horizon is a hard edge, not a blend: partial rows {partial:?}"
    );
    // Everything above the blend is foliage and everything below it is soil.
    let (first, last) = (partial[0], partial[partial.len() - 1]);
    assert!((0..first).all(|y| soil_weight(Face::Front, 0, y) == 0.0));
    assert!((last + 1..FACE_SIZE as u8).all(|y| soil_weight(Face::Front, 0, y) == 1.0));
}

// ---------------------------------------------------------------------------
// the ground
// ---------------------------------------------------------------------------

#[test]
fn a_saturated_world_with_no_detritus_still_has_a_bare_soil_band() {
    // Producers everywhere at the ramp's saturation point, no detritus at all.
    let v = view(flat(saturation()), flat(0.0));
    let canvas = draw(&v);

    // What the soil should be: the floor plus the dark plum at SOIL_MIN_BRIGHTNESS, and
    // nothing else — no producer ramp, however rich the cell is.
    let low = srgb_linear(SOIL_LOW_SRGB);
    let bare_soil = [
        PALETTE.floor[0] + low[0] * SOIL_MIN_BRIGHTNESS,
        PALETTE.floor[1] + low[1] * SOIL_MIN_BRIGHTNESS,
        PALETTE.floor[2] + low[2] * SOIL_MIN_BRIGHTNESS,
    ];

    let mut soil_px = 0;
    for (face, x, y) in every_pixel() {
        if soil_weight(face, x, y) != 1.0 || !plant_free(&v, face, x, y) {
            continue;
        }
        let got = canvas.get(face, x, y);
        for i in 0..3 {
            assert!(
                (got[i] - bare_soil[i]).abs() <= 1e-7,
                "{face:?} ({x}, {y}) channel {i}: {} vs the bare soil {}; a producer lawn \
                 is being drawn in the soil",
                got[i],
                bare_soil[i],
            );
        }
        soil_px += 1;
    }
    assert!(soil_px > 1_000, "only {soil_px} pixels are wholly soil and motif-free");

    // And above the horizon the ground is the decided image: floor, ramp, flecks. With
    // producers saturated everywhere every foliage slot grows a plant, so the comparison
    // uses a second view whose producers sit under the foliage's first stage threshold
    // (the canopy, with its lower thresholds, still grows, so only side-face pixels
    // well away from the top seam are compared).
    let quiet = view(flat(saturation() * 0.2), flat(0.0));
    assert!(0.2 < stage_thresholds(Band::Foliage)[0] && 0.2 > CANOPY_STAGES[0]);
    let quiet_canvas = draw(&quiet);
    let ground = expected_ground(&quiet);
    let mut foliage_px = 0;
    for (face, x, y) in deep_foliage() {
        assert_eq!(soil_weight(face, x, y), 0.0);
        assert!(plant_free(&quiet, face, x, y), "{face:?} ({x}, {y}) is within a plant's reach");
        assert_eq!(
            quiet_canvas.get(face, x, y),
            ground.get(face, x, y),
            "{face:?} ({x}, {y}) above the horizon is not the decided ground"
        );
        foliage_px += 1;
    }
    assert!(foliage_px > 1_000, "only {foliage_px} foliage pixels were compared");

    // The soil is genuinely darker than the lit foliage above it: that is the picture.
    let mut ramp_only = Canvas::new();
    present::draw_floor(&mut ramp_only);
    present::draw_ramp_field(
        &mut ramp_only,
        &ScalarField::constant(saturation()),
        saturation(),
        PALETTE.producer_low,
        PALETTE.producer_high,
        true,
    );
    let light = |c: [f32; 3]| f64::from(c[0]) + f64::from(c[1]) + f64::from(c[2]);
    assert!(
        light(bare_soil) < light(ramp_only.get(Face::Front, 32, 8)) * 0.5,
        "the soil band is not visibly darker than a saturated foliage band"
    );
}

#[test]
fn detritus_brightens_the_soil_and_keeps_it_violet() {
    let bare = draw(&view(flat(0.0), flat(0.0)));
    let rich = draw(&view(flat(0.0), flat(SOIL_SCALE)));

    // A pixel deep in the soil, far from the horizon and from any seam.
    let (face, x, y) = (Face::Front, 32, 60);
    assert_eq!(soil_weight(face, x, y), 1.0);
    assert_eq!(band_of(cell_of(&SurfacePoint::pixel_center(face, x, y))), Band::Soil);

    let poor = bare.get(face, x, y);
    let full = rich.get(face, x, y);
    let light = |c: [f32; 3]| f64::from(c[0]) + f64::from(c[1]) + f64::from(c[2]);
    assert!(
        light(full) > light(poor),
        "detritus must brighten the soil: {full:?} against {poor:?}"
    );
    assert!(full[2] > full[1], "rich soil must be violet, not green: {full:?}");
    assert!(full[0] < full[2], "rich soil must be violet, not red: {full:?}");
    assert!(poor[2] > poor[1] && poor[0] < poor[2], "bare soil must be plum: {poor:?}");

    // And an empty world is the ground rule as written, in both bands at once; a
    // detritus-rich soil also grows plants, which `tests/art_plants.rs` covers.
    assert_same_canvas(&bare, &expected_ground(&view(flat(0.0), flat(0.0))), "an empty world");
}

#[test]
fn the_foliage_ground_is_the_decided_image_and_the_soil_never_flecks() {
    // A mixed world: producers everywhere but under every plant threshold, and detritus
    // everywhere above the fleck threshold (so the soil grows plants where it is rich;
    // those pixels are excluded, the soil plants are tested in `tests/art_plants.rs`).
    let producer: Vec<f64> =
        (0..CELL_COUNT).map(|i| saturation() * CANOPY_STAGES[0] * (i % 5) as f64 / 8.0).collect();
    let detritus: Vec<f64> = (0..CELL_COUNT).map(|i| 0.06 + (i % 7) as f64 * 0.2).collect();
    let v = view(producer, detritus.clone());
    let canvas = draw(&v);

    // Above the horizon, the ground is exactly floor + ramp + flecks, drawn with the
    // `present` helpers the M2 image uses.
    let mut foliage = Canvas::new();
    present::draw_floor(&mut foliage);
    present::draw_ramp_field(
        &mut foliage,
        &field_from(&v.producer),
        saturation(),
        PALETTE.producer_low,
        PALETTE.producer_high,
        true,
    );
    let mut flecks = field_from(&detritus);
    for value in flecks.values.iter_mut() {
        if *value <= DETRITUS_THRESHOLD {
            *value = 0.0;
        }
    }
    draw_field(&mut foliage, &flecks, DETRITUS_SCALE, PALETTE.detritus, false);

    let mut compared = 0;
    for (face, x, y) in every_pixel() {
        // Only pixels no plant can reach: the plants are tested separately.
        if soil_weight(face, x, y) != 0.0 || !plant_free(&v, face, x, y) {
            continue;
        }
        assert_eq!(
            canvas.get(face, x, y),
            foliage.get(face, x, y),
            "{face:?} ({x}, {y}) above the horizon is not the decided image"
        );
        compared += 1;
    }
    assert!(compared > 1_000, "only {compared} foliage pixels were actually compared");
}

/// True when no plant can reach this pixel, so its value is the ground alone.
///
/// A plant tile's footprint is under nine pixels from its anchor and an anchor is within
/// one pixel of its cell center, so nothing more than three cell rows away can reach it —
/// and staying twelve pixels inside the face keeps the plants of the neighbouring faces
/// out without having to reason about seams.
fn plant_free(v: &RenderView, face: Face, x: u8, y: u8) -> bool {
    const MARGIN: u8 = 12;
    if !(MARGIN..FACE_SIZE as u8 - MARGIN).contains(&x) || y < MARGIN {
        return false;
    }
    // The bottom edge of a side face is the open rim: there is no face beyond it, so the
    // margin there would only throw away the deepest soil, which is what this is for.
    if face == Face::Top && y >= FACE_SIZE as u8 - MARGIN {
        return false;
    }
    let here = cell_of(&SurfacePoint::pixel_center(face, x, y));
    for dy in -3i32..=3 {
        for dx in -3i32..=3 {
            let (cx, cy) = (i32::from(here.cx()) + dx, i32::from(here.cy()) + dy);
            if !(0..CELLS_PER_FACE_EDGE as i32).contains(&cx)
                || !(0..CELLS_PER_FACE_EDGE as i32).contains(&cy)
            {
                continue;
            }
            if would_grow(v, CellId::new(face, cx as u8, cy as u8)) {
                return false;
            }
        }
    }
    true
}

/// Whether the documented stage rule grows anything in a cell of this view, from bare
/// ground.
fn would_grow(v: &RenderView, cell: CellId) -> bool {
    let band = cell_band(cell, v.water.get(cell.index()).copied());
    let t = plant_density(v, cell.index(), band);
    next_stage(None, t, &stage_thresholds(band), rank_cap_of(cell)).is_some()
}

/// Side-face pixels well above the horizon and well inside the face, where the only
/// plants that could reach are the foliage's own.
fn deep_foliage() -> impl Iterator<Item = (Face, u8, u8)> {
    side_faces().flat_map(|face| (12..=33u8).flat_map(move |y| (12..52u8).map(move |x| (face, x, y))))
}

// ---------------------------------------------------------------------------
// cost
// ---------------------------------------------------------------------------

/// Not a correctness test: the number the brief asks for. Run with
/// `cargo test --release -p cubarium --test art_bands -- --ignored band_draw_cost`.
#[test]
#[ignore = "timing, not behaviour"]
fn band_draw_cost() {
    use cubarium_core::OrganismId;
    use cubarium_core::organism::Mode;
    use cubarium_core::view::OrganismView;

    let mut v = view(flat(PRODUCER_MAX), flat(SOIL_SCALE));
    v.organisms = (0..200u32)
        .map(|slot| OrganismView {
            id: OrganismId { slot, generation: 1 },
            pos: SurfacePoint::new(
                Face::ALL[slot as usize % 5],
                1.0 + f64::from(slot % 61),
                1.0 + f64::from((slot * 7) % 61),
            ),
            heading: Vec2::new(1.0, 0.0),
            lobes: vec![(0.0, 0.0, 1.4), (2.0, 0.0, 0.9)],
            hue: (slot % 100) as f32 / 100.0,
            mode: Mode::Seeking,
            fed: false,
            juvenile: slot % 3 == 0,
            gestation: (slot % 5 == 0).then_some(0.5),
            form: (slot % 4) as u8,
            moved: Vec::new(),
        })
        .collect();

    let mut presenter = ArtPresenter::new(pack());
    let mut canvas = Canvas::new();
    for _ in 0..5 {
        presenter.draw(&v, 0.0, &mut canvas);
    }
    let frames = 60u64;
    let t0 = std::time::Instant::now();
    for i in 0..frames {
        v.tick = i;
        presenter.draw(&v, 0.5, &mut canvas);
    }
    let per = t0.elapsed().as_secs_f64() / frames as f64;
    println!(
        "ArtPresenter::draw with bands: {:.3} ms/frame (1280 motifs, 200 organisms) = {:.0}% \
         of a 60 fps budget",
        per * 1e3,
        per / (1.0 / 60.0) * 100.0
    );
    assert!(per < 1.0 / 60.0, "draw took {:.3} ms, past the whole 60 fps budget", per * 1e3);
}
