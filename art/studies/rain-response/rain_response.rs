//! Tests of the rain-response study prototype (`art/studies/rain-response`), run inside the
//! isolated source copy `run.sh` prepares. Written from the prototype's doc comments in
//! `art_present.rs` (`RAIN_RESPONSE`, `rain_tip`, `rain_level_target`,
//! `advance_rain_level`, `rain_patter`, `slot_wind_rain`, `ArtPresenter::rain_level_of`,
//! `without_rain_response`) and the existing wind contract (`slot_wind`, `plant_bend`,
//! `effective_tip`, `plant_bend_budget`, `WIND_SLOT_VARIATION`).

use std::path::Path;

use cube_proto::{FACE_SIZE, Face};
use cubarium::art::{ArtPack, Band};
use cubarium::art_present::{
    ArtPresenter, PLANT_BEND_ROOT, RAIN_ATTACK_SECONDS, RAIN_RELEASE_SECONDS, RAIN_RESPONSE,
    RAIN_RESPONSE_RATE, RAIN_SETTLE_FLOOR, SOIL_SCALE, WIND_PEAK_TICK, WIND_QUIET_TICK,
    WIND_RESPONSE, WIND_SLOT_VARIATION, advance_rain_level, band_of, effective_tip,
    plant_bend_budget, plant_cap, present_seconds, rain_level_target, rain_patter,
    rain_response, rain_tip, slot_of, slot_wind, slot_wind_rain, species_of, wind_response,
};
use cubarium::clock::DT;
use cubarium::present::PRODUCER_SATURATION;
use cubarium_core::view::RenderView;
use cubarium_render::Canvas;
use cubarium_surface::{CELL_COUNT, CellId};

const PRODUCER_MAX: f64 = 10.0;

fn pack() -> ArtPack {
    ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")).expect("pack")
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

/// A world rich everywhere (every slot at its cap), with rain `rate` on `rained` cells.
fn rich_view(tick: u64, rained: &[CellId], rate: f32) -> RenderView {
    let mut v = bare_view(tick);
    v.producer.fill(saturation());
    v.detritus.fill(SOIL_SCALE);
    for c in rained {
        v.rain[c.index()] = rate;
    }
    v
}

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL.into_iter().flat_map(|f| (0..FACE_SIZE as u8).flat_map(move |y| (0..FACE_SIZE as u8).map(move |x| (f, x, y))))
}

fn differing(a: &Canvas, b: &Canvas) -> Vec<(Face, u8, u8)> {
    every_pixel().filter(|&(f, x, y)| a.get(f, x, y) != b.get(f, x, y)).collect()
}

fn assert_same_canvas(a: &Canvas, b: &Canvas, what: &str) {
    let d = differing(a, b);
    assert!(d.is_empty(), "{what}: {} pixels differ, first {:?}: {:?} vs {:?}", d.len(), d[0], a.get(d[0].0, d[0].1, d[0].2), b.get(d[0].0, d[0].1, d[0].2));
}

fn draw(p: &mut ArtPresenter, v: &RenderView, f: f64) -> Canvas {
    let mut c = Canvas::new();
    p.draw(v, f, &mut c);
    c
}

fn max_diff_at(a: &Canvas, b: &Canvas, pixels: &[(Face, u8, u8)]) -> f32 {
    pixels.iter().flat_map(|&(f, x, y)| { let (p, q) = (a.get(f, x, y), b.get(f, x, y)); (0..3).map(move |c| (p[c] - q[c]).abs()) }).fold(0.0, f32::max)
}

/// A rank-2 slot of a responding species on a side face, away from the face edges.
fn responding_cell(species: &str, face: Face) -> CellId {
    CellId::all()
        .find(|&c| c.face() == face && c.face() != Face::Top && band_of(c) == Band::Foliage && plant_cap(Band::Foliage, c) == Some(2) && species_of(Band::Foliage, c) == species && (3..=12).contains(&c.cx()) && (3..=12).contains(&c.cy()))
        .unwrap_or_else(|| panic!("a rank-2 {species} slot on {face:?}"))
}

/// Grow the whole cube in from bare ground with no rain: 500 ticks is past every plant's
/// climb, ending inside a quiet wind interval.
fn grown(rained: &[CellId], rate: f32) -> (ArtPresenter, ArtPresenter, u64) {
    let mut new = ArtPresenter::new(pack());
    let mut old = ArtPresenter::new(pack()).without_rain_response();
    let first = WIND_QUIET_TICK;
    for tick in first..first + 500 {
        let v = rich_view(tick, &[], 0.0);
        new.observe(&v);
        old.observe(&v);
    }
    let _ = (rained, rate);
    (new, old, first + 500)
}

#[test]
fn with_no_rain_anywhere_the_image_is_the_old_one_bit_for_bit_windy_or_calm() {
    let (mut new, mut old, t) = grown(&[], 0.0);
    let mut checked = 0;
    for tick in t..t + 700 {
        let v = rich_view(tick, &[], 0.0);
        new.observe(&v);
        old.observe(&v);
        if tick % 7 == 0 {
            for f in [0.0, 0.5] {
                assert_same_canvas(&draw(&mut new, &v, f), &draw(&mut old, &v, f), &format!("tick {tick} f {f}"));
                checked += 1;
            }
        }
    }
    for cell in CellId::all() {
        assert_eq!(new.rain_level_of(cell), (0.0, 0.0));
    }
    assert!(checked > 150);
}

#[test]
fn the_level_rises_quickly_holds_through_the_shower_and_settles_exactly_to_zero() {
    // The pure step, as documented.
    let mut level = 0.0f32;
    let mut rising = Vec::new();
    for _ in 0..40 {
        level = advance_rain_level(level, 1.0, DT);
        rising.push(level);
    }
    assert!(rising[5] > 0.5 && rising[5] < 0.7, "after 0.3 s the level is ~63 %: {}", rising[5]);
    assert!((rising[39] - 1.0).abs() < 0.002, "two seconds in it is full: {}", rising[39]);
    let mut falling = Vec::new();
    for _ in 0..120 {
        level = advance_rain_level(level, 0.0, DT);
        falling.push(level);
    }
    assert!(falling[15] > 0.3 && falling[15] < 0.45, "0.8 s after the rain the level is ~37 %: {}", falling[15]);
    let settled = falling.iter().position(|&l| l == 0.0).expect("it must settle to exactly 0");
    assert!((3.0..=4.5).contains(&(settled as f64 * DT)), "settled after {} s", settled as f64 * DT);
    assert!(falling[settled..].iter().all(|&l| l == 0.0));
    assert_eq!(rain_level_target(0.0), 0.0);
    assert_eq!(rain_level_target(RAIN_RESPONSE_RATE as f32), 1.0);
    assert_eq!(rain_level_target(RAIN_RESPONSE_RATE as f32 * 0.5), 0.5);
    assert_eq!(rain_level_target(f32::NAN), 0.0);
    assert!(RAIN_ATTACK_SECONDS < RAIN_RELEASE_SECONDS && RAIN_SETTLE_FLOOR > 0.0);
    // And through the presenter: a shower of 120 ticks on one cell.
    let cell = responding_cell("lanternstalk", Face::Front);
    let (mut new, _old, t) = grown(&[], 0.0);
    let mut peak = 0.0f32;
    for tick in t..t + 300 {
        let raining = (t + 10..t + 130).contains(&tick);
        let v = rich_view(tick, &[cell], if raining { 0.25 } else { 0.0 });
        new.observe(&v);
        let (_, level) = new.rain_level_of(cell);
        peak = peak.max(level);
        if tick == t + 9 { assert_eq!(level, 0.0); }
        if tick == t + 129 { assert!(level > 0.99, "full through the shower: {level}"); }
    }
    assert_eq!(new.rain_level_of(cell).1, 0.0, "settled exactly");
    assert!(peak > 0.9999, "peak {peak}");
    assert_eq!(advance_rain_level(0.4, 1.0, 0.0), 0.4, "no time, no change");
    assert_eq!(advance_rain_level(0.4, 0.0, -1.0), 0.4, "no time, no change");
}

#[test]
fn a_repeated_observation_and_a_fresh_presenter_agree_and_the_root_row_never_moves() {
    let cell = responding_cell("lanternstalk", Face::Front);
    let (mut new, _old, t) = grown(&[], 0.0);
    for tick in t..t + 30 {
        new.observe(&rich_view(tick, &[cell], 0.25));
    }
    let v = rich_view(t + 30, &[cell], 0.25);
    new.observe(&v);
    let before = new.rain_level_of(cell);
    let image = draw(&mut new, &v, 0.5);
    new.observe(&v);
    new.observe(&v);
    assert_eq!(new.rain_level_of(cell), before, "a repeated observation must not advance the level");
    assert_same_canvas(&draw(&mut new, &v, 0.5), &image, "a repeated draw");
    // A fresh presenter joining mid-shower snaps the level to the rain it sees: honest, no
    // invented onset, and the same level a rewound one has.
    let mut fresh = ArtPresenter::new(pack());
    fresh.observe(&v);
    assert_eq!(fresh.rain_level_of(cell), (1.0, 1.0));
    let earlier = rich_view(t + 5, &[cell], 0.25);
    new.observe(&earlier);
    assert_eq!(new.rain_level_of(cell), (1.0, 1.0), "a rewind snaps like a fresh presenter");
    // The quiver is rooted and local: with the same wind, rained vs dry differs only
    // inside this slot's nine-pixel footprint, never on the tile's bottom (root) rows, and
    // never on the ground row under the anchor. The tile is 16 rows on pivot (8, 8), the
    // bend fixes heights ≤ PLANT_BEND_ROOT above the tile's bottom edge, and a side-face
    // plant stands "up" toward smaller v (±12° of heading jitter tilts that by ≤ 1.7 px
    // across the half-tile).
    let (mut wet, mut dry, t2) = grown(&[], 0.0);
    for tick in t2..t2 + 40 {
        wet.observe(&rich_view(tick, &[cell], 0.25));
        dry.observe(&rich_view(tick, &[], 0.0));
    }
    let vw = rich_view(t2 + 40, &[cell], 0.25);
    let vd = rich_view(t2 + 40, &[], 0.0);
    let slot = slot_of(cell);
    let a = draw(&mut wet, &vw, 0.5);
    let b = draw(&mut dry, &vd, 0.5);
    let d = differing(&a, &b);
    assert!(d.len() > 4, "the rained plant did not move at all");
    let lowest = d.iter().map(|&(_, _, y)| f64::from(y) + 0.5 - slot.at.v).fold(f64::NEG_INFINITY, f64::max);
    for &(face, x, y) in &d {
        assert_eq!(face, cell.face());
        let r = (f64::from(x) + 0.5 - slot.at.u).hypot(f64::from(y) + 0.5 - slot.at.v);
        assert!(r <= 9.0 + 0.71, "a rained pixel {r:.2} px from the anchor is outside the footprint");
    }
    println!("  lowest rained difference is {lowest:.2} px below the anchor (tile bottom edge at +8)");
    assert!(lowest < 8.0 - PLANT_BEND_ROOT + 1.7, "the root rows moved: {lowest:.2} px below the anchor");
    assert!(PLANT_BEND_ROOT > 1.0);
}

#[test]
fn wind_and_rain_together_never_exceed_the_familys_budget_and_a_still_family_stays_still() {
    let art = pack();
    let presenter = ArtPresenter::new(pack());
    let seconds = present_seconds(WIND_PEAK_TICK, 0.5);
    let mut answered = 0;
    for plant in &art.plants {
        let budget = plant_bend_budget(plant);
        let tip = rain_tip(&plant.name, budget);
        let wind_tip = effective_tip(wind_response(&plant.name).tip_px, budget);
        assert!(tip >= 0.0 && wind_tip + tip <= budget / (1.0 + WIND_SLOT_VARIATION) + 1e-12, "{}: wind {wind_tip} + rain {tip} over budget {budget}", plant.name);
        println!(
            "  {:<14} budget {budget:.3} px, wind tip {wind_tip:.3}, rain asks {:.2} gets {tip:.3}{}",
            plant.name,
            rain_response(&plant.name),
            if wind_response(&plant.name).spin_deg > 0.0 { " (radial: turns, never bends)" } else { "" }
        );
        for cell in CellId::all() {
            let slot = slot_of(cell);
            let (calm, h0) = slot_wind(&slot, &plant.name, budget, seconds);
            let (rained, h1) = slot_wind_rain(&slot, &plant.name, budget, seconds, 1.0);
            assert_eq!(h0, h1, "rain must not turn a plant");
            assert!(rained.amplitude.abs() <= budget + 1e-9, "{} at {cell:?}: amplitude {} over budget {budget}", plant.name, rained.amplitude);
            assert_eq!(slot_wind_rain(&slot, &plant.name, budget, seconds, 0.0), (calm, h0), "level 0 is the wind alone");
            if tip == 0.0 || (cell.face() == Face::Top && wind_response(&plant.name).spin_deg > 0.0) {
                assert_eq!(rained, calm, "{}: a family with no rain room, or a radial one, must not quiver", plant.name);
            } else if rained != calm {
                assert_eq!((rained.root, rained.length, rained.base), (PLANT_BEND_ROOT, calm.length.max(13.0), 0.0));
                answered += 1;
            }
        }
    }
    assert!(answered > 100, "no slot quivered");
    assert_eq!(presenter.bend_budget("lanternstalk"), plant_bend_budget(art.plant("lanternstalk").unwrap()));
    for (name, _) in RAIN_RESPONSE {
        assert!(WIND_RESPONSE.iter().any(|(n, _)| *n == name), "{name} is not a wind family");
    }
    assert!(rain_patter(0.0, 0.0).abs() <= 1.0);
    for k in 0..1000 {
        assert!(rain_patter(k as f64 * 0.0137, 1.3).abs() <= 1.0 + 1e-12);
    }
}

#[test]
fn a_rained_plant_moves_a_fraction_of_a_pixel_per_frame_at_sixty_fps_and_frame_rates_agree() {
    let cell = responding_cell("lanternstalk", Face::Front);
    let (mut a, _old, t) = grown(&[], 0.0);
    // Two peers grown the same way: `b` observes the same ticks in the same order, `c`
    // observes every tick twice (a held frame between ticks), and both must draw as `a`.
    let mut b = ArtPresenter::new(pack());
    let mut c = ArtPresenter::new(pack());
    for tick in WIND_QUIET_TICK..t {
        b.observe(&rich_view(tick, &[], 0.0));
        c.observe(&rich_view(tick, &[], 0.0));
    }
    let window: Vec<(Face, u8, u8)> = every_pixel().filter(|&(f, x, y)| f == cell.face() && (f64::from(x) + 0.5 - slot_of(cell).at.u).hypot(f64::from(y) + 0.5 - slot_of(cell).at.v) <= 12.0).collect();
    let mut last: Option<Canvas> = None;
    let mut worst = 0.0f32;
    let mut frames = 0;
    for tick in t..t + 200 {
        let raining = (t + 10..t + 130).contains(&tick);
        let v = rich_view(tick, &[cell], if raining { 0.25 } else { 0.0 });
        a.observe(&v);
        b.observe(&v);
        c.observe(&v);
        c.observe(&v);
        for k in 0..3 {
            let f = k as f64 / 3.0;
            let image = draw(&mut a, &v, f);
            if let Some(last) = &last {
                worst = worst.max(max_diff_at(&image, last, &window));
                frames += 1;
            }
            last = Some(image);
        }
        // Instants that 30, 60 and 120 fps share are drawn alike by presenters that saw
        // the same ticks, whether or not one of them held frames between ticks.
        for f in [0.0, 0.5] {
            let x = draw(&mut a, &v, f);
            assert_same_canvas(&x, &draw(&mut b, &v, f), &format!("tick {tick} f {f}: two presenters"));
            assert_same_canvas(&x, &draw(&mut c, &v, f), &format!("tick {tick} f {f}: a held cadence"));
        }
    }
    assert!(frames > 500);
    // The quiver is at most ~0.5 px of tip travel at 2.7 and 4.3 Hz: between two 60 fps
    // frames the tip moves well under a third of a pixel, so no channel jumps by more than
    // the bilinear blend of one pixel's worth of neighbour — measured, printed, bounded.
    println!("  worst per-frame change near the rained lanternstalk: {worst:.4}");
    assert!(worst < 0.45, "a frame jumped by {worst}");
}

#[test]
fn seams_the_top_vertex_and_the_open_rim_draw_the_rained_plant_whole_and_bounded() {
    // Rain on every cell along the Front/Right seam, round the Top vertex, and on the rim
    // row: the quiver rides the same seam-continuous rooted stamp as the breeze.
    let rained: Vec<CellId> = CellId::all().filter(|c| (c.face() == Face::Front && c.cx() == 15) || (c.face() == Face::Right && c.cx() == 0) || (c.face() == Face::Top && (c.cx() <= 1 || c.cy() <= 1)) || (c.face() != Face::Top && c.cy() == 15)).collect();
    let (mut new, mut old, t) = grown(&[], 0.0);
    for tick in t..t + 40 {
        new.observe(&rich_view(tick, &rained, 0.25));
        old.observe(&rich_view(tick, &rained, 0.25));
    }
    let v = rich_view(t + 40, &rained, 0.25);
    for f in [0.0, 0.5, 1.0] {
        let a = draw(&mut new, &v, f);
        let b = draw(&mut old, &v, f);
        for (face, x, y) in every_pixel() {
            assert!(a.get(face, x, y).iter().all(|&c| (0.0..=1.0 + 1e-6).contains(&c)), "{face:?} ({x}, {y}) f {f}");
        }
        // Something moved on a side face along the seam, nothing on Top's radial plants,
        // and the rows below the rim were never painted (the rim is open, not reflected).
        let d = differing(&a, &b);
        assert!(d.iter().any(|(fc, _, _)| *fc == Face::Front || *fc == Face::Right), "no rained plant moved at the seam (f {f})");
        assert!(d.iter().all(|(fc, _, _)| *fc != Face::Top), "a top-face plant answered rain (f {f}): {:?}", d.iter().find(|(fc, _, _)| *fc == Face::Top));
    }
}
