//! Independent tests for the plants of the live art mode, written from the wiring brief
//! and the public doc comments of `cubarium::art_present` rather than from the
//! implementation. Where the expected image is needed it is rebuilt here from the rules
//! through the public API — species by band, slot placement, rank, stage, opacity, sway
//! phase — so the presenter has to agree with a second, independently written copy of
//! the rule rather than with itself.

use std::path::Path;

use cube_proto::{FACE_SIZE, Face};
use cubarium::art::{ArtPack, Band};
use cubarium::art_present::{
    ArtPresenter, CANOPY_PLANTS, FOLIAGE_PLANTS, FOLIAGE_STAGES, FRUIT_SHOW, HEADING_JITTER_DEG,
    MOTIF_OPACITY, RANK_FULL, RANK_MID, REED_DEPTH, REED_STAGES, SOIL_PLANT_OPACITY,
    SOIL_PLANTS, SOIL_SCALE, SOIL_STAGES, STAGE_HYST, WATER_PLANT, band_of, band_opacity,
    cell_band, fruit_stage, next_stage, placement_of, plant_density, plant_phase_of,
    plant_cap, rank_cap_of, slot_of, soil_weight, species_of, stage_opacity, stage_thresholds,
    stalk_heading, up_of, column_density, ground_frame, ground_opacity, ground_phase_of,
    ground_points, ground_weight, tall_anchor, tall_columns, tall_heading,
    tall_phase_of, tall_target, water_brightness, water_color, water_coverage, water_phase,
    TALL_PLANTS, VINE_PLANT,
};
use cubarium::clock::DT;
use cubarium::present::{
    self, DETRITUS_SCALE, DETRITUS_THRESHOLD, PALETTE, PRODUCER_SATURATION, srgb_linear,
};
use cubarium_core::view::RenderView;
use cubarium_render::{Canvas, draw_field, stamp_sprite};
use cubarium_surface::{
    CELL_COUNT, CellId, Edge, PixelImage, ScalarField, SurfacePoint, Vec2, cell_of,
    pixel_neighbor,
};

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

fn flat(v: f64) -> Vec<f64> {
    vec![v; CELL_COUNT]
}

fn view(tick: u64, producer: Vec<f64>, detritus: Vec<f64>, water: Vec<f64>) -> RenderView {
    RenderView {
        tick,
        producer,
        detritus,
        fruit: vec![0.0; CELL_COUNT],
        water,
        rain: vec![0.0; CELL_COUNT],
        producer_max: PRODUCER_MAX,
        organisms: Vec::new(),
    }
}

fn draw(p: &mut ArtPresenter, v: &RenderView, fruit: Option<&[f64]>) -> Canvas {
    let mut canvas = Canvas::new();
    p.observe(v);
    p.draw_with_fruit(v, 0.0, &mut canvas, fruit);
    canvas
}

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL
        .into_iter()
        .flat_map(|f| (0..FACE_SIZE as u8).flat_map(move |y| (0..FACE_SIZE as u8).map(move |x| (f, x, y))))
}

fn differing(a: &Canvas, b: &Canvas) -> Vec<(Face, u8, u8)> {
    every_pixel().filter(|&(f, x, y)| a.get(f, x, y) != b.get(f, x, y)).collect()
}

fn assert_same_canvas(a: &Canvas, b: &Canvas, what: &str) {
    let diff = differing(a, b);
    assert!(
        diff.is_empty(),
        "{what}: {} pixels differ, first at {:?}: {:?} vs {:?}",
        diff.len(),
        diff[0],
        a.get(diff[0].0, diff[0].1, diff[0].2),
        b.get(diff[0].0, diff[0].1, diff[0].2),
    );
}

// ---------------------------------------------------------------------------
// the expected image, rebuilt from the rules
// ---------------------------------------------------------------------------

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

/// The ground as `tests/art_bands.rs` derives it: floor, the decided ramp and flecks
/// faded out by the horizon, the soil ramp faded in by it.
fn expected_ground(v: &RenderView) -> Canvas {
    use cubarium::art_present::{
        SOIL_HIGH_SRGB, SOIL_LOW_SRGB, SOIL_MAX_BRIGHTNESS, SOIL_MIN_BRIGHTNESS,
    };
    let mut canvas = Canvas::new();
    present::draw_floor(&mut canvas);
    let mut layer = Canvas::new();
    present::draw_ramp_field(
        &mut layer,
        &field_from(&v.producer),
        v.producer_max * PRODUCER_SATURATION,
        PALETTE.producer_low,
        PALETTE.producer_high,
        true,
    );
    add_scaled(&mut canvas, &layer, |f, x, y| 1.0 - soil_weight(f, x, y));
    let mut flecks = field_from(&v.detritus);
    for value in flecks.values.iter_mut() {
        if *value <= DETRITUS_THRESHOLD {
            *value = 0.0;
        }
    }
    let mut layer = Canvas::new();
    draw_field(&mut layer, &flecks, DETRITUS_SCALE, PALETTE.detritus, false);
    add_scaled(&mut canvas, &layer, |f, x, y| 1.0 - soil_weight(f, x, y));
    let detritus = field_from(&v.detritus);
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
    // Ground cover: the band's tile on the 8-px lattice, fading in from the band's first
    // stage threshold, cross-faded through the horizon.
    let pack = pack();
    let mut scratch: Vec<PixelImage> = Vec::new();
    for face in Face::ALL {
        for (face, x, y) in ground_points(face) {
            let point = SurfacePoint::pixel_center(face, x, y);
            let cell = cell_of(&point);
            let band = band_of(cell);
            let Some(tile) = pack.ground_for(band) else { continue };
            let t = plant_density(v, cell.index(), band);
            let opacity = ground_opacity(t, band) * ground_weight(face, x, y, band);
            if opacity <= 0.0 {
                continue;
            }
            let frame = ground_frame(tile, v.tick as f64 * DT + ground_phase_of(face, x, y, tile.seconds));
            stamp_sprite(&mut canvas, point, Vec2::new(1.0, 0.0), frame, 1.0, opacity, &mut scratch);
        }
    }
    // Water: filtered depth, coverage 1 − exp(−w/film), source-over, shimmering.
    let water = field_from(&v.water);
    for (face, x, y) in every_pixel() {
        let w = filtered(&water, face, x, y);
        let a = water_coverage(w);
        if a <= 0.0 {
            continue;
        }
        let c = water_color(w);
        let b = water_brightness(v.tick, water_phase(face, x, y));
        let under = canvas.get(face, x, y);
        canvas.set(face, x, y, [
            c[0] * b * a + under[0] * (1.0 - a),
            c[1] * b * a + under[1] * (1.0 - a),
            c[2] * b * a + under[2] * (1.0 - a),
        ]);
    }
    canvas
}

/// The tall columns the rules predict from bare ground (one tick, no history): base,
/// `n` trunks, crown, then the vine's trunks at the odd positions.
fn stamp_tall(canvas: &mut Canvas, v: &RenderView, pack: &ArtPack) {
    let mut scratch: Vec<PixelImage> = Vec::new();
    for column in tall_columns() {
        let n = tall_target(column_density(v, column.face, column.cx));
        if n == 0 {
            continue;
        }
        let plant = pack.tall_plant(TALL_PLANTS[column.pick]).expect("tall species");
        let heading = tall_heading(column.face, column.cx);
        let mut stamp = |clip: &cubarium::art::Clip, i: u8| {
            let sprite = clip.at(v.tick as f64 * DT + tall_phase_of(column.face, column.cx, clip.seconds));
            stamp_sprite(canvas, tall_anchor(column.face, column.cx, i), heading, sprite, 1.0, MOTIF_OPACITY, &mut scratch);
        };
        if let Some(base) = &plant.base {
            stamp(base, 0);
        }
        for i in 1..=n {
            stamp(&plant.trunk, i);
        }
        if let Some(crown) = &plant.crown {
            stamp(crown, n + 1);
        }
        if column.vine {
            let vine = pack.tall_plant(VINE_PLANT).expect("vine");
            for i in (1..=n).filter(|i| i % 2 == 1) {
                stamp(&vine.trunk, i);
            }
        }
    }
}

/// The plant stage the rules give a cell of this view from bare ground (one tick, no
/// history), with its band.
fn stage_from_bare(v: &RenderView, cell: CellId) -> (Band, Option<u8>) {
    let band = cell_band(cell, v.water.get(cell.index()).copied());
    let t = plant_density(v, cell.index(), band);
    let stage = plant_cap(band, cell).and_then(|cap| next_stage(None, t, &stage_thresholds(band), cap));
    (band, stage)
}

/// The image the rules predict for a view seen from bare ground: the ground plus, per
/// cell that grows, the species' stage clip (or its fruit clip) at the sway time,
/// stamped at the slot in `CellId` order.
fn expected_image(v: &RenderView, pack: &ArtPack, fruit: Option<&[f64]>) -> Canvas {
    let mut canvas = expected_ground(v);
    let mut scratch: Vec<PixelImage> = Vec::new();
    for cell in CellId::all() {
        let (band, Some(stage)) = stage_from_bare(v, cell) else { continue };
        let Some(plant) = pack.plant(species_of(band, cell)) else { continue };
        let t = plant_density(v, cell.index(), band);
        let opacity = stage_opacity(stage, t, &stage_thresholds(band), band_opacity(band));
        if opacity <= 0.0 {
            continue;
        }
        let f = fruit.and_then(|f| f.get(cell.index()).copied());
        let clip = match (&plant.fruit, stage == 2 && fruit_stage(f)) {
            (Some(clip), true) => clip,
            _ => &plant.stages[usize::from(stage)],
        };
        let sprite = clip.at(v.tick as f64 * DT + plant_phase_of(cell, clip.seconds));
        let (at, heading) = placement_of(cell);
        stamp_sprite(&mut canvas, at, heading, sprite, 1.0, opacity, &mut scratch);
    }
    stamp_tall(&mut canvas, v, pack);
    canvas
}

// ---------------------------------------------------------------------------
// species, slots, ranks
// ---------------------------------------------------------------------------

#[test]
fn species_follow_the_band_and_a_wet_cell_grows_reeds() {
    let mut seen = std::collections::HashMap::<&str, usize>::new();
    for cell in CellId::all() {
        let band = band_of(cell);
        let name = species_of(band, cell);
        let allowed: &[&str] = match band {
            Band::Soil => &SOIL_PLANTS,
            Band::Foliage => &FOLIAGE_PLANTS,
            Band::Canopy => &CANOPY_PLANTS,
            Band::Water => unreachable!("a dry cell is never water"),
        };
        assert!(allowed.contains(&name), "{cell:?} in {band:?} picked {name}");
        *seen.entry(name).or_default() += 1;
        // Wet, the same cell grows reeds whatever its band.
        assert_eq!(cell_band(cell, Some(REED_DEPTH + 0.01)), Band::Water);
        assert_eq!(cell_band(cell, Some(REED_DEPTH)), band, "the depth rule is strict");
        assert_eq!(cell_band(cell, None), band);
        assert_eq!(species_of(Band::Water, cell), WATER_PLANT);
    }
    for name in SOIL_PLANTS.iter().chain(&FOLIAGE_PLANTS).chain(&CANOPY_PLANTS) {
        assert!(seen[name] > 60, "{name} is picked by only {} cells", seen[name]);
    }
    // Every named species exists in the pack.
    let art = pack();
    for name in SOIL_PLANTS.iter().chain(&FOLIAGE_PLANTS).chain(&CANOPY_PLANTS).chain([&WATER_PLANT]) {
        assert!(art.plant(name).is_some(), "the pack has no plant named {name}");
    }
}

#[test]
fn slots_stay_in_their_cell_stand_up_on_the_sides_and_face_freely_on_top() {
    for cell in CellId::all() {
        let slot = slot_of(cell);
        assert_eq!(cell_of(&slot.at), cell, "{cell:?} anchored its plant elsewhere");
        assert!((slot.heading.length() - 1.0).abs() < 1e-9);
        assert!(slot.pick < 2);
        assert!(slot.rank_cap <= 2);
        assert_eq!(placement_of(cell), (slot.at, slot.heading));
        assert_eq!(rank_cap_of(cell), slot.rank_cap);
        match up_of(cell) {
            Some(up) => {
                assert_ne!(cell.face(), Face::Top);
                // Up really is up: a step along it raises the embedded height.
                let here = cell.center();
                let there = SurfacePoint::new(here.face, here.u + up.x, here.v + up.y);
                assert!(there.embed()[1] > here.embed()[1], "{cell:?}: {up:?} is not up");
                // Tiles stand along their −y, which the renderer lays along the heading
                // turned a quarter turn; so the heading is up turned the other way.
                let stand = stalk_heading(up);
                let off = (slot.heading.screen_angle() - stand.screen_angle()).abs();
                let off = off.min(std::f64::consts::TAU - off).to_degrees();
                assert!(off <= HEADING_JITTER_DEG + 1e-9, "{cell:?} leans {off:.1}°");
                // And that quarter turn really does put tile −y along up: a tile point one
                // unit up its own −y lands one unit higher on the cube.
                let side = Vec2::new(-stand.y, stand.x);
                let tile_up = Vec2::new(-side.x, -side.y);
                assert!((tile_up.x - up.x).abs() < 1e-12 && (tile_up.y - up.y).abs() < 1e-12);
            }
            None => assert_eq!(cell.face(), Face::Top, "{cell:?} has no up"),
        }
    }
}

#[test]
fn the_rank_split_is_about_a_third_each() {
    let mut counts = [0usize; 3];
    for cell in CellId::all() {
        counts[usize::from(rank_cap_of(cell))] += 1;
    }
    let share = |n: usize| n as f64 / CELL_COUNT as f64;
    assert!((share(counts[2]) - RANK_FULL).abs() < 0.05, "stage-2 share {counts:?}");
    assert!((share(counts[1]) - (RANK_MID - RANK_FULL)).abs() < 0.05, "stage-1 share {counts:?}");
    assert!((share(counts[0]) - (1.0 - RANK_MID)).abs() < 0.05, "sprout share {counts:?}");
}

// ---------------------------------------------------------------------------
// stages and hysteresis
// ---------------------------------------------------------------------------

#[test]
fn stages_rise_at_the_threshold_and_fall_only_below_the_hysteresis() {
    for band in [Band::Soil, Band::Foliage, Band::Canopy, Band::Water] {
        let th = stage_thresholds(band);
        assert!(th[0] < th[1] && th[1] < th[2], "{band:?} thresholds are not ordered");
        for (i, &t) in th.iter().enumerate() {
            let stage = i as u8;
            assert_eq!(next_stage(None, t, &th, 2), stage.checked_sub(1), "{band:?} at th[{i}] exactly");
            assert_eq!(next_stage(None, t + 1e-9, &th, 2), Some(stage), "{band:?} just over th[{i}]");
            // Held just under the threshold, released below threshold − hysteresis.
            assert_eq!(next_stage(Some(stage), t - STAGE_HYST / 2.0, &th, 2), Some(stage));
            assert_eq!(
                next_stage(Some(stage), t - STAGE_HYST - 1e-9, &th, 2),
                stage.checked_sub(1),
                "{band:?} falls out of stage {stage}"
            );
        }
        // The cap.
        assert_eq!(next_stage(None, 10.0, &th, 0), Some(0));
        assert_eq!(next_stage(None, 10.0, &th, 1), Some(1));
        assert_eq!(next_stage(None, 10.0, &th, 2), Some(2));
    }
    const { assert!(REED_STAGES[0] > REED_DEPTH, "reeds stand only in pools deeper than the water line") };
}

#[test]
fn a_flickering_field_does_not_flicker_the_plant() {
    let mut p = ArtPresenter::new(pack());
    let cell = CellId::new(Face::Front, 5, 5);
    assert_eq!(band_of(cell), Band::Foliage);
    let th = FOLIAGE_STAGES;
    let cap = rank_cap_of(cell);
    let mut producer = flat(0.0);
    // Cross the first threshold, then oscillate within the hysteresis band.
    producer[cell.index()] = saturation() * (th[0] + 0.01);
    p.observe(&view(0, producer.clone(), flat(0.0), flat(0.0)));
    assert_eq!(p.stage_of(cell), Some(0));
    for tick in 1..40u64 {
        let wobble = if tick % 2 == 0 { th[0] + 0.01 } else { th[0] - STAGE_HYST / 2.0 };
        producer[cell.index()] = saturation() * wobble;
        p.observe(&view(tick, producer.clone(), flat(0.0), flat(0.0)));
        assert_eq!(p.stage_of(cell), Some(0), "tick {tick} flickered");
    }
    // Drop well below: gone.
    producer[cell.index()] = saturation() * (th[0] - STAGE_HYST - 0.01);
    p.observe(&view(40, producer.clone(), flat(0.0), flat(0.0)));
    assert_eq!(p.stage_of(cell), None);
    // Rich: capped by rank.
    producer[cell.index()] = saturation();
    p.observe(&view(41, producer.clone(), flat(0.0), flat(0.0)));
    assert_eq!(p.stage_of(cell), Some(cap));
}

#[test]
fn a_frame_without_an_observe_shows_the_ticks_plants_and_changes_nothing_else() {
    let v = view(3, flat(saturation()), flat(SOIL_SCALE), flat(0.0));
    let mut observed = ArtPresenter::new(pack());
    let mut canvas_a = Canvas::new();
    observed.observe(&v);
    observed.draw(&v, 0.0, &mut canvas_a);
    let mut unobserved = ArtPresenter::new(pack());
    let mut canvas_b = Canvas::new();
    unobserved.draw(&v, 0.0, &mut canvas_b);
    assert_same_canvas(&canvas_a, &canvas_b, "observe then draw vs draw alone");
    for cell in CellId::all() {
        assert_eq!(observed.stage_of(cell), unobserved.stage_of(cell));
    }
    // And a second draw of the same view is identical.
    let mut canvas_c = Canvas::new();
    observed.draw(&v, 0.0, &mut canvas_c);
    assert_same_canvas(&canvas_a, &canvas_c, "two draws of one view");
}

// ---------------------------------------------------------------------------
// determinism and sway
// ---------------------------------------------------------------------------

#[test]
fn two_presenters_agree_on_every_slot_and_phase() {
    let a = ArtPresenter::new(pack());
    let b = ArtPresenter::new(pack());
    for cell in CellId::all() {
        assert_eq!(slot_of(cell), slot_of(cell));
        for band in [Band::Soil, Band::Foliage, Band::Canopy, Band::Water] {
            assert_eq!(species_of(band, cell), species_of(band, cell));
            let (pa, pb) = (a.plant_for(band, cell), b.plant_for(band, cell));
            assert_eq!(pa.map(|p| &p.name), pb.map(|p| &p.name));
        }
        let ph = plant_phase_of(cell, 3.0);
        assert_eq!(ph, plant_phase_of(cell, 3.0));
        assert!((0.0..3.0).contains(&ph), "{cell:?}: {ph}");
        assert_eq!(plant_phase_of(cell, 0.0), 0.0);
        assert_eq!(plant_phase_of(cell, f64::NAN), 0.0);
    }
    // Phases are spread, not shared: at least half the cells differ from their neighbour.
    let phases: Vec<f64> = CellId::all().map(|c| plant_phase_of(c, 1.0)).collect();
    let distinct = phases.windows(2).filter(|w| (w[0] - w[1]).abs() > 1e-6).count();
    assert!(distinct > CELL_COUNT / 2, "{distinct} neighbouring cells differ in phase");
}

#[test]
fn sway_runs_on_simulated_time_and_repeats_after_a_clip_period() {
    let art = pack();
    let plant = art.plant(FOLIAGE_PLANTS[0]).expect("lanternstalk");
    let seconds = plant.stages[2].seconds;
    let ticks = |secs: f64| {
        let t = (secs / DT).round() as u64;
        assert!((t as f64 * DT - secs).abs() < 1e-9, "{secs} s is not a whole number of ticks");
        t
    };
    fn gcd(a: u64, b: u64) -> u64 { if b == 0 { a } else { gcd(b, a % b) } }
    let lcm = |a: u64, b: u64| a / gcd(a, b) * b;
    // Every simulated-time layer in a dry saturated world: the plants' sway, the ground
    // tiles' breath and the tall plants' pulse. The image repeats after their common period.
    let mut period = ticks(seconds);
    for tile in &art.ground {
        period = lcm(period, ticks(tile.seconds));
    }
    for tall in &art.tall {
        period = lcm(period, ticks(tall.trunk.seconds));
    }
    let plant_period = ticks(seconds);
    let mut p = ArtPresenter::new(pack());
    let v0 = view(100, flat(saturation()), flat(0.0), flat(0.0));
    let v1 = view(100 + period, flat(saturation()), flat(0.0), flat(0.0));
    let v_half = view(100 + plant_period / 2, flat(saturation()), flat(0.0), flat(0.0));
    let a = draw(&mut p, &v0, None);
    let b = draw(&mut p, &v1, None);
    let h = draw(&mut p, &v_half, None);
    assert_same_canvas(&a, &b, "one sway period later");
    assert!(!differing(&a, &h).is_empty(), "half a period later the plants have not moved");
}

// ---------------------------------------------------------------------------
// the image
// ---------------------------------------------------------------------------

#[test]
fn a_saturated_foliage_cell_draws_its_species_stage_at_its_cap() {
    let art = pack();
    // A cell whose rank lets it grow to stage 2, so the full sprite is what shows.
    let cell = CellId::all()
        .find(|&c| c.face() == Face::Front && band_of(c) == Band::Foliage && rank_cap_of(c) == 2 && c.cy() >= 3 && c.cy() <= 7)
        .expect("some front foliage cell has rank 2");
    let mut producer = flat(0.0);
    producer[cell.index()] = saturation();
    let v = view(17, producer, flat(0.0), flat(0.0));
    let mut p = ArtPresenter::new(pack());
    let actual = draw(&mut p, &v, None);
    assert_eq!(p.stage_of(cell), Some(2));

    let name = species_of(Band::Foliage, cell);
    let clip = &art.plant(name).unwrap().stages[2];
    let sprite = clip.at(v.tick as f64 * DT + plant_phase_of(cell, clip.seconds));
    let (at, heading) = placement_of(cell);
    let mut expected = expected_ground(&v);
    let mut scratch: Vec<PixelImage> = Vec::new();
    stamp_sprite(&mut expected, at, heading, sprite, 1.0, MOTIF_OPACITY, &mut scratch);
    assert_same_canvas(&actual, &expected, "one full foliage plant");
    assert!(!differing(&actual, &expected_ground(&v)).is_empty(), "it drew nothing");

    // The other foliage species at the same slot would be a different image.
    let other = FOLIAGE_PLANTS.iter().find(|&&n| n != name).unwrap();
    let wrong = &art.plant(other).unwrap().stages[2];
    let mut other_img = expected_ground(&v);
    stamp_sprite(&mut other_img, at, heading, wrong.at(v.tick as f64 * DT + plant_phase_of(cell, wrong.seconds)), 1.0, MOTIF_OPACITY, &mut scratch);
    assert!(!differing(&actual, &other_img).is_empty(), "the two foliage species draw alike");
}

#[test]
fn every_band_at_once_is_the_rule_as_written() {
    let art = pack();
    // Producers saturated, detritus rich, a pond on the top face and a puddle in the soil.
    let mut water = flat(0.0);
    for cell in CellId::all() {
        if cell.face() == Face::Top && cell.cx() >= 4 && cell.cx() <= 7 && cell.cy() >= 9 && cell.cy() <= 11 {
            water[cell.index()] = 1.2;
        }
        if cell.face() == Face::Right && cell.cy() == 15 && cell.cx() % 3 == 0 {
            water[cell.index()] = 0.5;
        }
    }
    let v = view(250, flat(saturation()), flat(SOIL_SCALE * 0.8), water);
    let mut p = ArtPresenter::new(pack());
    let actual = draw(&mut p, &v, None);
    assert_same_canvas(&actual, &expected_image(&v, &art, None), "a rich world in every band");

    // Non-vacuity: reeds stand in the pond, the soil has plants, the canopy is covered.
    // Reeds stand in the pond's slots that may grow at all; the sprout-only slots stay open
    // water, so the pond is not a fence.
    let pond_cells: Vec<CellId> = CellId::all()
        .filter(|c| c.face() == Face::Top && (4..=7).contains(&c.cx()) && (9..=11).contains(&c.cy()))
        .collect();
    for &pond in &pond_cells {
        assert_eq!(p.band_at(pond), Band::Water);
        assert_eq!(p.stage_of(pond).is_some(), plant_cap(Band::Water, pond).is_some(), "{pond:?}");
        assert_eq!(p.plant_for(Band::Water, pond).unwrap().name, WATER_PLANT);
    }
    assert!(pond_cells.iter().any(|&c| p.stage_of(c).is_some()), "no reed in the pond");
    assert!(pond_cells.iter().any(|&c| p.stage_of(c).is_none()), "the pond is a reed fence");
    let soil = CellId::new(Face::Left, 7, 14);
    assert_eq!(p.band_at(soil), Band::Soil);
    assert!(p.stage_of(soil).is_some(), "rich soil grows");
    let grown = CellId::all().filter(|&c| p.stage_of(c).is_some()).count();
    assert!(grown > CELL_COUNT * 8 / 10, "only {grown} cells grew in a saturated world");
}

#[test]
fn fruit_shows_on_a_full_plant_only_when_the_cell_holds_enough() {
    let art = pack();
    let cell = CellId::all()
        .find(|&c| c.face() == Face::Back && band_of(c) == Band::Foliage && rank_cap_of(c) == 2 && species_of(Band::Foliage, c) == "lanternstalk" && c.cy() >= 3 && c.cy() <= 7)
        .expect("a back-face lanternstalk slot of rank 2");
    let mut producer = flat(0.0);
    producer[cell.index()] = saturation();
    let v = view(60, producer, flat(0.0), flat(0.0));
    let mut fruit = flat(0.0);

    let mut p = ArtPresenter::new(pack());
    fruit[cell.index()] = 0.1;
    let fruiting = draw(&mut p, &v, Some(&fruit));
    fruit[cell.index()] = 0.02;
    let not_yet = draw(&mut p, &v, Some(&fruit));
    let none = draw(&mut p, &v, None);
    assert!(fruit_stage(Some(0.1)) && !fruit_stage(Some(0.02)) && !fruit_stage(None));
    assert!(!fruit_stage(Some(FRUIT_SHOW)), "the fruit rule is strict");

    assert_same_canvas(&not_yet, &none, "fruit under the threshold draws the plain stage");
    assert!(!differing(&fruiting, &none).is_empty(), "fruit changed nothing");

    let plant = art.plant("lanternstalk").unwrap();
    let clip = plant.fruit.as_ref().expect("lanternstalk has a fruit clip");
    let sprite = clip.at(v.tick as f64 * DT + plant_phase_of(cell, clip.seconds));
    let (at, heading) = placement_of(cell);
    let mut expected = expected_ground(&v);
    let mut scratch: Vec<PixelImage> = Vec::new();
    stamp_sprite(&mut expected, at, heading, sprite, 1.0, MOTIF_OPACITY, &mut scratch);
    assert_same_canvas(&fruiting, &expected, "the fruit clip at the slot");
    fruit[cell.index()] = 0.1;
    assert_same_canvas(&fruiting, &expected_image(&v, &art, Some(&fruit)), "the full rule with fruit");
}

#[test]
fn nothing_grows_below_the_first_threshold_and_the_soil_ramp_is_untouched() {
    let producer: Vec<f64> = (0..CELL_COUNT)
        .map(|i| saturation() * stage_thresholds(Band::Canopy)[0] * (i % 9) as f64 / 9.0)
        .collect();
    let detritus: Vec<f64> = (0..CELL_COUNT).map(|i| SOIL_SCALE * SOIL_STAGES[0] * (i % 4) as f64 / 4.0).collect();
    let v = view(5, producer.clone(), detritus.clone(), flat(0.0));
    let mut p = ArtPresenter::new(pack());
    let actual = draw(&mut p, &v, None);
    for cell in CellId::all() {
        assert_eq!(p.stage_of(cell), None, "{cell:?} grew below its first threshold");
    }
    for i in 0..p.columns().len() {
        assert_eq!(p.segments_of(i), 0, "a quiet column grew a tall plant");
    }
    assert_same_canvas(&actual, &expected_ground(&v), "a quiet world is only ground");
    // Water exactly at the water line is drawn as water but grows no reed.
    let wet = view(5, producer, detritus, flat(REED_DEPTH));
    let mut p = ArtPresenter::new(pack());
    let _ = draw(&mut p, &wet, None);
    for cell in CellId::all() {
        assert_eq!(p.stage_of(cell), None, "{cell:?} grew a reed at the water line");
    }
}

#[test]
fn sprouts_fade_in_and_soil_plants_are_dimmer() {
    let th = FOLIAGE_STAGES;
    assert_eq!(stage_opacity(0, th[0] - STAGE_HYST, &th, MOTIF_OPACITY), 0.0);
    let mid = stage_opacity(0, (th[0] - STAGE_HYST + th[1]) / 2.0, &th, MOTIF_OPACITY);
    assert!((mid - MOTIF_OPACITY / 2.0).abs() < 1e-6, "{mid}");
    assert_eq!(stage_opacity(0, th[1], &th, MOTIF_OPACITY), MOTIF_OPACITY);
    assert_eq!(stage_opacity(1, 0.0, &th, MOTIF_OPACITY), MOTIF_OPACITY);
    assert_eq!(stage_opacity(2, 0.0, &th, MOTIF_OPACITY), MOTIF_OPACITY);
    assert_eq!(stage_opacity(0, f64::NAN, &th, MOTIF_OPACITY), 0.0);
    assert_eq!(band_opacity(Band::Soil), SOIL_PLANT_OPACITY);
    const { assert!(SOIL_PLANT_OPACITY < MOTIF_OPACITY, "soil plants must be dimmer") };
    for band in [Band::Foliage, Band::Canopy, Band::Water] {
        assert_eq!(band_opacity(band), MOTIF_OPACITY);
    }
}

/// Not a correctness test: the number the brief asks for. Run with
/// `cargo test --release -p cubarium --test art_plants -- --ignored plant_draw_cost`.
#[test]
#[ignore = "timing, not behaviour"]
fn plant_draw_cost() {
    use cubarium_core::OrganismId;
    use cubarium_core::organism::Mode;
    use cubarium_core::view::OrganismView;
    let mut v = view(0, flat(PRODUCER_MAX), flat(SOIL_SCALE), flat(0.0));
    v.organisms = (0..200u32)
        .map(|slot| OrganismView {
            id: OrganismId { slot, generation: 1 },
            pos: SurfacePoint::new(Face::ALL[slot as usize % 5], 1.0 + f64::from(slot % 61), 1.0 + f64::from((slot * 7) % 61)),
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
    let mut p = ArtPresenter::new(pack());
    let mut canvas = Canvas::new();
    p.observe(&v);
    for _ in 0..5 {
        p.draw(&v, 0.0, &mut canvas);
    }
    let grown = CellId::all().filter(|&c| p.stage_of(c).is_some()).count();
    let frames = 60;
    let t0 = std::time::Instant::now();
    for i in 0..frames {
        v.tick = i;
        p.draw(&v, 0.5, &mut canvas);
    }
    let per = t0.elapsed().as_secs_f64() / frames as f64;
    println!(
        "ArtPresenter::draw with plants: {:.3} ms/frame ({grown} plants, 200 organisms) = {:.0}% of a 60 fps budget",
        per * 1e3,
        per / (1.0 / 60.0) * 100.0
    );
    assert!(per < 1.0 / 60.0, "draw took {:.3} ms", per * 1e3);
}
