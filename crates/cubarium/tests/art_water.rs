//! Independent tests for the water layer, the rain, the ground cover and the tall plants
//! of the live art mode, written from `design/water.md`, the presenter brief and the
//! public doc comments of `cubarium::art_present`. Expected images are rebuilt here from
//! the rules through the public API rather than borrowed from the implementation.

use std::path::Path;

use cube_proto::{FACE_SIZE, Face};
use cubarium::art::{ArtPack, Band};
use cubarium::art_present::{
    ArtPresenter, FOLIAGE_STAGES, GROUND_LATTICE, GROUND_OPACITY, MOTIF_OPACITY, RAIN_BLINK,
    RAIN_DENSITY, RAIN_MAX_STREAKS, RAIN_PERIOD, RAIN_SPEED, TALL_COLUMN_P, TALL_MAX_SEGMENTS,
    TALL_PLANTS, TALL_STEP, WATER_FILM, WATER_SHIMMER_SECONDS, band_of, column_density,
    foliage_rows, ground_frame, ground_opacity, ground_phase_of, ground_points, ground_weight,
    TALL_HYST, next_tall, rain_blink_on, tall_rise, rain_fall, rain_marks, rain_origin, rain_streaks, soil_weight,
    stage_thresholds, tall_anchor, tall_column_of, tall_columns, tall_target, up_of,
    water_brightness, water_coverage,
};
use cubarium::clock::DT;
use cubarium::present::PRODUCER_SATURATION;
use cubarium_core::view::RenderView;
use cubarium_render::{Canvas, stamp_sprite};
use cubarium::art_present::{band_opacity, next_stage, placement_of, plant_density, plant_phase_of, rank_cap_of, species_of, stage_opacity};
use cubarium_surface::{CELL_COUNT, CELLS_PER_FACE_EDGE, CellId, PixelImage, SurfacePoint, Vec2, cell_of};

const PRODUCER_MAX: f64 = 10.0;

fn pack() -> ArtPack {
    ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")).expect("pack")
}

fn saturation() -> f64 {
    PRODUCER_MAX * PRODUCER_SATURATION
}

fn flat(v: f64) -> Vec<f64> {
    vec![v; CELL_COUNT]
}

fn view(tick: u64, producer: Vec<f64>, detritus: Vec<f64>, water: Vec<f64>, rain: Vec<f32>) -> RenderView {
    RenderView {
        tick,
        producer,
        detritus,
        fruit: vec![0.0; CELL_COUNT],
        water,
        rain,
        producer_max: PRODUCER_MAX,
        organisms: Vec::new(),
    }
}

fn draw_at(p: &mut ArtPresenter, v: &RenderView, f: f64) -> Canvas {
    let mut canvas = Canvas::new();
    p.observe(v);
    p.draw(v, f, &mut canvas);
    canvas
}

fn draw(v: &RenderView) -> Canvas {
    draw_at(&mut ArtPresenter::new(pack()), v, 0.0)
}

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL
        .into_iter()
        .flat_map(|f| (0..FACE_SIZE as u8).flat_map(move |y| (0..FACE_SIZE as u8).map(move |x| (f, x, y))))
}

fn differing(a: &Canvas, b: &Canvas) -> Vec<(Face, u8, u8)> {
    every_pixel().filter(|&(f, x, y)| a.get(f, x, y) != b.get(f, x, y)).collect()
}

fn side_faces() -> impl Iterator<Item = Face> {
    Face::ALL.into_iter().filter(|&f| f != Face::Top)
}

// ---------------------------------------------------------------------------
// water
// ---------------------------------------------------------------------------

#[test]
fn coverage_follows_the_film_rule_and_a_dry_cell_has_none() {
    assert_eq!(water_coverage(0.0), 0.0);
    assert_eq!(water_coverage(-1.0), 0.0);
    assert_eq!(water_coverage(f64::NAN), 0.0);
    let a = water_coverage(WATER_FILM);
    assert!((f64::from(a) - (1.0 - (-1.0f64).exp())).abs() < 1e-6, "{a}");
    assert!((f64::from(water_coverage(0.05)) - (1.0 - (-0.05 / WATER_FILM).exp())).abs() < 1e-6);
    assert!(water_coverage(5.0) > 0.99);
    assert!(water_coverage(0.5) > water_coverage(0.2));
}

#[test]
fn a_dry_world_draws_exactly_the_dry_image_whether_water_is_empty_or_zero() {
    let producer = flat(saturation() * 0.5);
    let zero = view(9, producer.clone(), flat(0.4), flat(0.0), vec![0.0; CELL_COUNT]);
    let mut empty = view(9, producer, flat(0.4), Vec::new(), Vec::new());
    empty.water = Vec::new();
    empty.rain = Vec::new();
    let a = draw(&zero);
    let b = draw(&empty);
    assert!(differing(&a, &b).is_empty(), "an empty water vector must draw as dry");
}

#[test]
fn deep_water_is_blue_cyan_brighter_than_the_ground_and_local() {
    let dry = view(3, flat(0.0), flat(0.0), flat(0.0), vec![0.0; CELL_COUNT]);
    let mut water = flat(0.0);
    let cell = CellId::new(Face::Front, 8, 5);
    water[cell.index()] = 5.0;
    let wet = view(3, flat(0.0), flat(0.0), water, vec![0.0; CELL_COUNT]);
    let a = draw(&dry);
    let b = draw(&wet);
    let (cx, cy) = (cell.cx() as i32, cell.cy() as i32);
    let center = b.get(Face::Front, (cx * 4 + 1) as u8, (cy * 4 + 1) as u8);
    let under = a.get(Face::Front, (cx * 4 + 1) as u8, (cy * 4 + 1) as u8);
    assert!(center[2] > center[1] && center[1] > center[0], "not blue-cyan: {center:?}");
    assert!(center.iter().sum::<f32>() > under.iter().sum::<f32>(), "water darker than bare ground");
    // The water itself spreads one pixel (the filter); the cell, now a pool, also grows a
    // reed whose 16-px tile reaches up to 9 px from its anchor within ±1 px of the center.
    let center = cell.center();
    for (f, x, y) in differing(&a, &b) {
        assert_eq!(f, Face::Front, "water leaked onto {f:?}");
        let p = SurfacePoint::pixel_center(f, x, y);
        let d = ((p.u - center.u).powi(2) + (p.v - center.v).powi(2)).sqrt();
        assert!(d <= 11.5, "({x}, {y}) is {d:.1} px from the wet cell");
    }
}

#[test]
fn a_thin_film_changes_the_ground_by_its_coverage() {
    let dry = view(3, flat(0.0), flat(0.0), flat(0.0), vec![0.0; CELL_COUNT]);
    let wet = view(3, flat(0.0), flat(0.0), flat(0.05), vec![0.0; CELL_COUNT]);
    let a = draw(&dry);
    let b = draw(&wet);
    // Far from the horizon and away from seams, the filtered depth is exactly 0.05.
    let (x, y) = (20u8, 20u8);
    let cover = f64::from(water_coverage(0.05));
    assert!((cover - (1.0 - (-0.05 / WATER_FILM).exp())).abs() < 1e-6, "{cover}");
    assert!(cover > 0.05 && cover < 0.25, "a 0.05 film should be faint but visible: {cover}");
    let before = a.get(Face::Front, x, y);
    let after = b.get(Face::Front, x, y);
    // The ground component is scaled by (1 − a): each channel is at most the ground plus
    // the water term, and the blue channel rose.
    assert!(after[2] > before[2]);
    for i in 0..3 {
        assert!(after[i] >= before[i] * (1.0 - cover as f32) - 1e-6);
    }
}

#[test]
fn the_shimmer_repeats_on_its_period_and_is_still_within_a_tick() {
    let ticks = (WATER_SHIMMER_SECONDS / DT).round() as u64;
    assert!((ticks as f64 * DT - WATER_SHIMMER_SECONDS).abs() < 1e-9);
    // Half a unit deep: solid water, but under the reed line so nothing else animates.
    let water = flat(0.5);
    let v0 = view(40, flat(0.0), flat(0.0), water.clone(), vec![0.0; CELL_COUNT]);
    let v1 = view(40 + ticks, flat(0.0), flat(0.0), water.clone(), vec![0.0; CELL_COUNT]);
    let vh = view(40 + ticks / 2, flat(0.0), flat(0.0), water, vec![0.0; CELL_COUNT]);
    let mut p = ArtPresenter::new(pack());
    let a = draw_at(&mut p, &v0, 0.0);
    let a_again = draw_at(&mut p, &v0, 0.7);
    let b = draw_at(&mut p, &v1, 0.0);
    let h = draw_at(&mut p, &vh, 0.0);
    assert!(differing(&a, &a_again).is_empty(), "the frame fraction must not move the water");
    assert!(differing(&a, &b).is_empty(), "a whole shimmer period later the water differs");
    assert!(!differing(&a, &h).is_empty(), "half a period later nothing shimmered");
    let phase = 1.0;
    assert!((water_brightness(0, phase) - water_brightness(ticks, phase)).abs() < 1e-6);
}

#[test]
fn reeds_leave_half_of_a_pool_open() {
    use cubarium::art_present::{plant_cap, rank_cap_of};
    let mut bare = 0;
    let mut grow = 0;
    for cell in CellId::all() {
        match plant_cap(Band::Water, cell) {
            None => {
                bare += 1;
                assert_eq!(rank_cap_of(cell), 0);
            }
            Some(cap) => {
                grow += 1;
                assert_eq!(cap, rank_cap_of(cell));
                assert!(cap >= 1);
            }
        }
        for band in [Band::Soil, Band::Foliage, Band::Canopy] {
            assert_eq!(plant_cap(band, cell), Some(rank_cap_of(cell)));
        }
    }
    let share = bare as f64 / CELL_COUNT as f64;
    assert!((0.4..0.6).contains(&share), "{bare} bare of {}", bare + grow);
    // A flooded floor row grows reeds only in its growing slots.
    let mut water = flat(0.0);
    for cx in 0..CELLS_PER_FACE_EDGE as u8 {
        water[CellId::new(Face::Right, cx, 15).index()] = 1.5;
    }
    let v = view(3, flat(0.0), flat(0.0), water, vec![0.0; CELL_COUNT]);
    let mut p = ArtPresenter::new(pack());
    p.observe(&v);
    for cx in 0..CELLS_PER_FACE_EDGE as u8 {
        let cell = CellId::new(Face::Right, cx, 15);
        assert_eq!(p.stage_of(cell).is_some(), plant_cap(Band::Water, cell).is_some(), "{cell:?}");
    }
}

// ---------------------------------------------------------------------------
// rain
// ---------------------------------------------------------------------------

#[test]
fn streak_count_follows_the_rate_and_caps() {
    assert_eq!(rain_streaks(0.0), 0);
    assert_eq!(rain_streaks(-1.0), 0);
    assert_eq!(rain_streaks(f32::NAN), 0);
    assert_eq!(rain_streaks(0.1), 1);
    assert_eq!(rain_streaks(1.0), RAIN_DENSITY.ceil() as usize);
    assert_eq!(rain_streaks(100.0), RAIN_MAX_STREAKS);
}

#[test]
fn a_streak_falls_downhill_and_wraps_within_its_cell() {
    let cell = CellId::new(Face::Front, 5, 6);
    let up = up_of(cell).expect("side face");
    let (dx, dy) = rain_origin(cell, 0);
    assert!(dx < 4 && dy < 4);
    let head_at = |seconds: f64| rain_marks(cell, 0, seconds)[0];
    let (x0, y0) = head_at(0.0);
    assert_eq!(x0, cell.cx() * 4 + dx);
    assert_eq!(y0, cell.cy() * 4 + dy);
    // One pixel of fall later the head moved one pixel downhill (−up), still in the cell.
    let one_px = 1.0 / RAIN_SPEED;
    let (x1, y1) = head_at(one_px + 1e-9);
    let expected_y = i32::from(cell.cy()) * 4 + (i32::from(dy) + if up.y < 0.0 { 1 } else { -1 }).rem_euclid(4);
    assert_eq!((i32::from(x1), i32::from(y1)), (i32::from(x0), expected_y), "the streak did not fall one pixel downhill");
    // Every mark stays inside the cell's column and the face; the fall wraps.
    for k in 0..RAIN_MAX_STREAKS {
        for step in 0..40 {
            let seconds = step as f64 * RAIN_PERIOD / 40.0 + 7.0;
            let marks = rain_marks(cell, k, seconds);
            assert!(!marks.is_empty() && marks.len() <= 2);
            let head = marks[0];
            assert_eq!(head.0 / 4, cell.cx(), "head left its cell column");
            assert_eq!(head.1 / 4, cell.cy(), "head left its cell row");
        }
    }
    assert!((rain_fall(RAIN_PERIOD + 0.1) - RAIN_SPEED * 0.1).abs() < 1e-9, "the fall wraps every period");
}

#[test]
fn top_face_streaks_blink_instead_of_falling() {
    let cell = CellId::new(Face::Top, 3, 3);
    assert!(up_of(cell).is_none());
    let (dx, dy) = rain_origin(cell, 1);
    let on = rain_marks(cell, 1, 0.01);
    assert!(rain_blink_on(0.01));
    assert_eq!(on, vec![(cell.cx() * 4 + dx, cell.cy() * 4 + dy)]);
    let off_time = RAIN_BLINK + 0.01;
    assert!(!rain_blink_on(off_time));
    assert!(rain_marks(cell, 1, off_time).is_empty());
}

#[test]
fn rain_adds_a_few_pixels_in_its_cell_and_nothing_without_rain() {
    let dry = view(12, flat(0.0), flat(0.0), flat(0.0), vec![0.0; CELL_COUNT]);
    let mut rain = vec![0.0f32; CELL_COUNT];
    let cell = CellId::new(Face::Front, 6, 4);
    rain[cell.index()] = 1.0;
    let raining = view(12, flat(0.0), flat(0.0), flat(0.0), rain);
    let mut p = ArtPresenter::new(pack());
    let a = draw_at(&mut p, &dry, 0.0);
    let b = draw_at(&mut p, &raining, 0.0);
    let diff = differing(&a, &b);
    assert!(!diff.is_empty(), "rain drew nothing");
    assert!(diff.len() <= 2 * RAIN_DENSITY.ceil() as usize, "{} pixels for one raining cell", diff.len());
    for (f, x, y) in &diff {
        assert_eq!(*f, Face::Front);
        let c = cell_of(&SurfacePoint::pixel_center(*f, *x, *y));
        assert!(
            (i32::from(c.cx()) - i32::from(cell.cx())).abs() <= 0 && (i32::from(c.cy()) - i32::from(cell.cy())).abs() <= 1,
            "({x}, {y}) is not in the raining cell's column"
        );
    }
    // Time moves the streaks: at 20 px/s a head falls one pixel per tick, so the next tick
    // draws a different image, and the frame fraction advances the fall continuously.
    let mut next = raining.clone();
    next.tick += 1;
    assert!(!differing(&b, &draw_at(&mut p, &next, 0.0)).is_empty(), "a tick later the rain has not moved");
    let t = 12.0 * DT;
    assert!((rain_fall(t + 0.5 * DT) - rain_fall(t) - 0.5 * RAIN_SPEED * DT).abs() < 1e-9);
    // Zero rain draws exactly the dry image.
    let none = view(12, flat(0.0), flat(0.0), flat(0.0), vec![0.0; CELL_COUNT]);
    assert!(differing(&a, &draw_at(&mut p, &none, 0.0)).is_empty());
}

// ---------------------------------------------------------------------------
// ground cover
// ---------------------------------------------------------------------------

#[test]
fn the_ground_lattice_is_the_eight_pixel_grid() {
    for face in Face::ALL {
        let points: Vec<_> = ground_points(face).collect();
        assert_eq!(points.len(), 64);
        for (f, x, y) in points {
            assert_eq!(f, face);
            assert_eq!(x % GROUND_LATTICE, GROUND_LATTICE / 2);
            assert_eq!(y % GROUND_LATTICE, GROUND_LATTICE / 2);
        }
    }
}

#[test]
fn ground_opacity_starts_at_the_first_threshold_and_saturates() {
    for band in [Band::Soil, Band::Foliage, Band::Canopy] {
        let t0 = stage_thresholds(band)[0];
        assert_eq!(ground_opacity(0.0, band), 0.0);
        assert_eq!(ground_opacity(t0, band), 0.0);
        assert_eq!(ground_opacity(t0 - 0.01, band), 0.0);
        let mid = ground_opacity((t0 + 1.0) / 2.0, band);
        assert!((mid - GROUND_OPACITY / 2.0).abs() < 1e-6, "{band:?}: {mid}");
        assert_eq!(ground_opacity(1.0, band), GROUND_OPACITY);
        assert_eq!(ground_opacity(7.0, band), GROUND_OPACITY);
        assert_eq!(ground_opacity(f64::NAN, band), 0.0);
    }
}

#[test]
fn each_band_lays_its_own_tile_at_the_lattice_point() {
    let art = pack();
    // A lattice point deep in the soil of Front, one on the canopy.
    let soil_point = ground_points(Face::Front).find(|&(_, _, y)| y >= 56).expect("a soil point");
    let canopy_point = ground_points(Face::Top).find(|&(_, x, y)| x == 28 && y == 28).expect("a canopy point");
    for (point, band, detritus, producer) in [
        (soil_point, Band::Soil, 1.5, 0.0),
        (canopy_point, Band::Canopy, 0.0, saturation()),
    ] {
        let (face, x, y) = point;
        let cell = cell_of(&SurfacePoint::pixel_center(face, x, y));
        assert_eq!(band_of(cell), band);
        let mut d = flat(0.0);
        let mut pr = flat(0.0);
        d[cell.index()] = detritus;
        pr[cell.index()] = producer;
        let rich = view(31, pr, d, flat(0.0), vec![0.0; CELL_COUNT]);
        // The rich ground alone: the same presenter with a pack that has no plants, tall
        // plants or ground tiles draws floor, ramps and soil and nothing on them.
        let mut ground_only = pack();
        ground_only.plants.clear();
        ground_only.tall.clear();
        ground_only.ground.clear();
        let a = draw_at(&mut ArtPresenter::new(ground_only), &rich, 0.0);
        let b = draw(&rich);
        // Expected from the rules: the rich ground, then the band's tile at the lattice
        // point (full opacity times the band weight), then the cell's own plant on top.
        let tile = art.ground_for(band).expect("tile");
        let frame = ground_frame(tile, rich.tick as f64 * DT + ground_phase_of(face, x, y, tile.seconds));
        let opacity = GROUND_OPACITY * ground_weight(face, x, y, band);
        assert!(opacity > 0.0);
        let mut expected = a.clone();
        let mut scratch: Vec<PixelImage> = Vec::new();
        stamp_sprite(&mut expected, SurfacePoint::pixel_center(face, x, y), Vec2::new(1.0, 0.0), frame, 1.0, opacity, &mut scratch);
        assert!(!differing(&a, &expected).is_empty(), "{band:?} tile painted nothing");
        let t = plant_density(&rich, cell.index(), band);
        if let Some(stage) = next_stage(None, t, &stage_thresholds(band), rank_cap_of(cell)) {
            let plant = art.plant(species_of(band, cell)).expect("plant");
            let clip = &plant.stages[usize::from(stage)];
            let sprite = clip.at(rich.tick as f64 * DT + plant_phase_of(cell, clip.seconds));
            let (at, heading) = placement_of(cell);
            let po = stage_opacity(stage, t, &stage_thresholds(band), band_opacity(band));
            stamp_sprite(&mut expected, at, heading, sprite, 1.0, po, &mut scratch);
        }
        let diff = differing(&b, &expected);
        assert!(diff.is_empty(), "{band:?}: {} pixels differ from tile-then-plant, first {:?}", diff.len(), diff.first());
        // The other bands' tiles at the same point would be a different image.
        let other = art.ground_for(if band == Band::Soil { Band::Canopy } else { Band::Soil }).unwrap();
        let mut wrong = a.clone();
        let wf = ground_frame(other, rich.tick as f64 * DT + ground_phase_of(face, x, y, other.seconds));
        stamp_sprite(&mut wrong, SurfacePoint::pixel_center(face, x, y), Vec2::new(1.0, 0.0), wf, 1.0, opacity, &mut scratch);
        assert!(!differing(&wrong, &expected).is_empty() || differing(&a, &wrong).is_empty(), "the two tiles draw alike");
    }
}

#[test]
fn ground_cover_is_absent_in_a_quiet_world() {
    let quiet = flat(saturation() * (FOLIAGE_STAGES[0].min(stage_thresholds(Band::Canopy)[0]) - 0.01));
    let v = view(2, quiet, flat(0.0), flat(0.0), vec![0.0; CELL_COUNT]);
    for face in Face::ALL {
        for (face, x, y) in ground_points(face) {
            let cell = cell_of(&SurfacePoint::pixel_center(face, x, y));
            let band = band_of(cell);
            let t = cubarium::art_present::plant_density(&v, cell.index(), band);
            assert_eq!(ground_opacity(t, band), 0.0, "{face:?} ({x}, {y})");
        }
    }
    // Weight sanity: soil tile fades with the soil, the others with its complement.
    for (f, x, y) in every_pixel() {
        let w = soil_weight(f, x, y);
        assert!((ground_weight(f, x, y, Band::Soil) - w).abs() < 1e-7);
        assert!((ground_weight(f, x, y, Band::Foliage) - (1.0 - w)).abs() < 1e-7);
    }
}

// ---------------------------------------------------------------------------
// tall plants
// ---------------------------------------------------------------------------

#[test]
fn tall_columns_are_a_deterministic_fifth_of_the_side_columns_and_never_on_top() {
    let columns = tall_columns();
    let again = tall_columns();
    assert_eq!(columns, again);
    assert!(columns.iter().all(|c| c.face != Face::Top));
    for cx in 0..CELLS_PER_FACE_EDGE as u8 {
        assert!(tall_column_of(Face::Top, cx).is_none());
    }
    let share = columns.len() as f64 / (4 * CELLS_PER_FACE_EDGE) as f64;
    assert!((share - TALL_COLUMN_P).abs() <= 0.06, "{} columns = {share}", columns.len());
    assert!(columns.iter().any(|c| c.pick == 0) && columns.iter().any(|c| c.pick == 1));
}

#[test]
fn the_foliage_has_eleven_rows_and_the_horizon_row_is_ten() {
    for face in side_faces() {
        assert_eq!(foliage_rows(face), Some((0, 10)));
    }
    assert_eq!(foliage_rows(Face::Top), None);
}

#[test]
fn column_height_follows_the_foliage_density_with_hysteresis() {
    assert_eq!(tall_target(0.0), 0);
    assert_eq!(tall_target(FOLIAGE_STAGES[0]), 0);
    assert_eq!(tall_target(FOLIAGE_STAGES[0] + TALL_STEP * 0.4), 0);
    assert_eq!(tall_target(FOLIAGE_STAGES[0] + TALL_STEP * 0.6), 1);
    assert_eq!(tall_target(tall_rise(5)), 5);
    assert_eq!(tall_target(1.0), TALL_MAX_SEGMENTS);
    assert_eq!(tall_target(f64::NAN), 0);
    const { assert!(TALL_HYST > 0.5 * TALL_STEP && TALL_HYST < TALL_STEP, "one segment below holds, two let go") };
    // Rises at once, holds one segment below, falls at two, idempotent.
    let level = |n: u8| FOLIAGE_STAGES[0] + TALL_STEP * f64::from(n);
    assert_eq!(next_tall(0, level(5)), 5);
    assert_eq!(next_tall(5, level(4)), 5, "one below holds");
    assert_eq!(next_tall(5, level(3)), 4, "two below falls");
    assert_eq!(next_tall(4, level(3)), 4);
    assert_eq!(next_tall(next_tall(5, level(3)), level(3)), 4);
    assert_eq!(next_tall(7, 0.0), 0, "a bare column falls all the way in one tick");
    assert_eq!(next_tall(3, f64::NAN), 0);
    // Against a view: a saturated column reaches the max; a bare one nothing.
    let face = Face::Right;
    let rich = view(0, flat(saturation()), flat(0.0), flat(0.0), vec![0.0; CELL_COUNT]);
    assert!((column_density(&rich, face, 3) - 1.0).abs() < 1e-9);
    let bare = view(0, flat(0.0), flat(0.0), flat(0.0), vec![0.0; CELL_COUNT]);
    assert_eq!(column_density(&bare, face, 3), 0.0);
}

#[test]
fn a_full_column_stacks_to_the_rim_and_its_crown_reaches_the_top_face() {
    let art = pack();
    let column = tall_columns().into_iter().find(|c| c.face == Face::Front).expect("a front column");
    let plant = art.tall_plant(TALL_PLANTS[column.pick]).expect("species");
    assert!(plant.crown.is_some());
    // The anchors climb the face 4 px at a time from the horizon cell's center.
    let base = tall_anchor(column.face, column.cx, 0);
    let horizon = CellId::new(column.face, column.cx, 10).center();
    assert!((base.u - horizon.u).abs() < 1e-9 && (base.v - horizon.v).abs() < 1e-9);
    for i in 1..=TALL_MAX_SEGMENTS + 1 {
        let a = tall_anchor(column.face, column.cx, i - 1);
        let b = tall_anchor(column.face, column.cx, i);
        let step = ((b.u - a.u).powi(2) + (b.v - a.v).powi(2)).sqrt();
        assert!((step - 4.0).abs() < 1e-9);
        assert!(b.embed()[1] > a.embed()[1], "tile {i} is not higher than tile {}", i - 1);
    }
    let crown = tall_anchor(column.face, column.cx, TALL_MAX_SEGMENTS + 1);
    let rim = CellId::new(column.face, column.cx, 0).center();
    assert!((crown.u - rim.u).abs() < 1e-9 && (crown.v - rim.v).abs() < 1e-9, "the crown sits on the rim cell");

    // Only this column's cells are rich, so only it grows; the crown lights the top face.
    let mut producer = flat(0.0);
    for cy in 0..=10u8 {
        producer[CellId::new(column.face, column.cx, cy).index()] = saturation();
    }
    let v = view(5, producer, flat(0.0), flat(0.0), vec![0.0; CELL_COUNT]);
    let mut p = ArtPresenter::new(pack());
    let with = draw_at(&mut p, &v, 0.0);
    let index = p.columns().iter().position(|c| *c == column).unwrap();
    assert_eq!(p.segments_of(index), TALL_MAX_SEGMENTS);
    let bare = draw(&view(5, flat(0.0), flat(0.0), flat(0.0), vec![0.0; CELL_COUNT]));
    let diff = differing(&bare, &with);
    assert!(diff.iter().any(|&(f, _, _)| f == Face::Top), "the crown did not reach the top face");
    assert!(diff.iter().any(|&(f, _, y)| f == column.face && y > 44), "the base did not reach below the horizon");
    // Nothing on the other side faces.
    assert!(diff.iter().all(|&(f, _, _)| f == Face::Top || f == column.face), "another face changed");
    // Two draws agree; a column period later the pulse repeats.
    let again = draw_at(&mut p, &v, 0.3);
    assert!(differing(&with, &again).is_empty());
    fn gcd(a: u64, b: u64) -> u64 { if b == 0 { a } else { gcd(b, a % b) } }
    let mut period = (plant.trunk.seconds / DT).round() as u64;
    for tile in &art.ground {
        let g = (tile.seconds / DT).round() as u64;
        period = period / gcd(period, g) * g;
    }
    for plant in &art.plants {
        let g = (plant.stages[2].seconds / DT).round() as u64;
        period = period / gcd(period, g) * g;
    }
    let later = view(5 + period, v.producer.clone(), flat(0.0), flat(0.0), vec![0.0; CELL_COUNT]);
    assert!(differing(&with, &draw_at(&mut p, &later, 0.0)).is_empty(), "the pulse did not repeat");
}

#[test]
fn a_bare_column_draws_nothing_tall_and_hysteresis_holds_one_segment() {
    let column = tall_columns()[0];
    let mut p = ArtPresenter::new(pack());
    let index = 0;
    let level = |n: u8| FOLIAGE_STAGES[0] + TALL_STEP * f64::from(n);
    let column_view = |t: f64| {
        let mut producer = flat(0.0);
        for cy in 0..=10u8 {
            producer[CellId::new(column.face, column.cx, cy).index()] = saturation() * t;
        }
        view(1, producer, flat(0.0), flat(0.0), vec![0.0; CELL_COUNT])
    };
    p.observe(&column_view(0.0));
    assert_eq!(p.segments_of(index), 0);
    p.observe(&column_view(level(5)));
    assert_eq!(p.segments_of(index), 5);
    p.observe(&column_view(level(4)));
    assert_eq!(p.segments_of(index), 5, "one segment below holds");
    p.observe(&column_view(level(3)));
    assert_eq!(p.segments_of(index), 4, "two below falls by one");
    p.observe(&column_view(0.0));
    assert_eq!(p.segments_of(index), 0, "a bare column is gone in one tick");
    // A view of bare ground draws no tall pixels at all: the image equals the ground.
    let bare = column_view(0.0);
    let a = draw_at(&mut p, &bare, 0.0);
    let b = draw(&bare);
    assert!(differing(&a, &b).is_empty());
}

#[test]
fn tall_tiles_are_stamped_at_full_motif_opacity_in_the_column_heading() {
    // The doc says MOTIF_OPACITY; the constant is what the plants use, so a tree and a
    // plant of the same tile would read equally solid.
    const { assert!(MOTIF_OPACITY > 0.5 && MOTIF_OPACITY <= 1.0) };
    for face in side_faces() {
        let h = cubarium::art_present::tall_heading(face, 3);
        let up = up_of(CellId::new(face, 3, 10)).unwrap();
        // stalk_heading(up) = (−up.y, up.x)
        assert!((h.x + up.y).abs() < 1e-9 && (h.y - up.x).abs() < 1e-9);
    }
}

/// Not a correctness test: the draw-cost number for the report. Run with
/// `cargo test --release -p cubarium --test art_water -- --ignored everything_on_draw_cost --nocapture`.
#[test]
#[ignore]
fn everything_on_draw_cost() {
    use cubarium_core::OrganismId;
    use cubarium_core::organism::Mode;
    use cubarium_core::view::OrganismView;
    let organisms = (0..200u32)
        .map(|slot| OrganismView {
            id: OrganismId { slot, generation: 1 },
            pos: SurfacePoint::new(Face::ALL[(slot % 5) as usize], (slot % 13) as f64 * 4.5 + 3.0, (slot % 11) as f64 * 5.5 + 3.0),
            heading: Vec2::new(1.0, 0.0),
            lobes: vec![],
            hue: (slot % 7) as f32 / 7.0,
            mode: Mode::Seeking,
            fed: false,
            juvenile: slot % 3 == 0,
            gestation: if slot % 5 == 0 { Some(0.5) } else { None },
            form: (slot % 4) as u8,
            moved: Vec::new(),
        })
        .collect();
    let mut v = view(77, flat(saturation()), flat(1.5), flat(1.0), vec![1.0; CELL_COUNT]);
    v.organisms = organisms;
    let mut p = ArtPresenter::new(pack());
    let mut canvas = Canvas::new();
    p.observe(&v);
    p.draw(&v, 0.0, &mut canvas);
    let frames = 60;
    let start = std::time::Instant::now();
    for i in 0..frames {
        v.tick = 77 + i;
        p.observe(&v);
        p.draw(&v, 0.37, &mut canvas);
    }
    let ms = start.elapsed().as_secs_f64() * 1e3 / frames as f64;
    println!("ArtPresenter::draw everything on: {ms:.3} ms/frame (all wet, raining, rich, all columns tall, 200 organisms) = {:.0}% of a 60 fps budget", ms / (1000.0 / 60.0) * 100.0);
    assert!(ms < 1000.0 / 60.0, "{ms} ms/frame exceeds the 60 fps budget");
}
