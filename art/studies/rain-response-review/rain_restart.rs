//! Presenter reconstruction under rain (`art/studies/rain-response-review`): what a host
//! restart shows, given that presentation history (the per-cell rain level) is not
//! persisted and a fresh presenter snaps to the rain it first sees. Runs inside a frozen
//! source copy carrying the rain-response prototype (v2 or v3): the review's `run.sh test`.
//!
//! A restart is modelled as a *brand-new* presenter observing the live view for the first
//! time (it snaps growth and level). Every difference is measured against the same
//! brand-new presenter with the response off, so what is attributed to the rain response
//! is only what the response adds; whatever a restart already changes about growth or
//! wind is reported separately and not blamed on the rain.
//!
//! Three claims, each measured rather than assumed:
//! 1. Restarted *while it rains* at a saturating rate, the fresh presenter holds the same
//!    full level as the continuous one and its rain contribution to the frame is the same.
//! 2. Restarted while the level is still *rising* (a weak rate that does not saturate),
//!    the fresh presenter snaps ahead of the continuous one by the unelapsed attack, a
//!    bounded difference that closes within a few attack constants.
//! 3. Restarted *just after* the rain ends, the decay tail is dropped: the fresh presenter
//!    is already still while the continuous one settles; the difference is bounded,
//!    shrinks as the tail settles, and vanishes once the continuous level reaches the
//!    settle floor. Restart is therefore not exact in the tail; it is a cut to the still
//!    image, never a pop upward.

use std::path::Path;

use cube_proto::{FACE_SIZE, Face};
use cubarium::art::{ArtPack, Band};
use cubarium::art_present::{
    ArtPresenter, RAIN_ATTACK_SECONDS, RAIN_RELEASE_SECONDS, RAIN_RESPONSE_RATE, RAIN_SETTLE_FLOOR,
    SOIL_SCALE, WIND_QUIET_TICK, band_of, plant_cap, species_of, slot_of,
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

fn rich_view(tick: u64, rained: &[CellId], rate: f32) -> RenderView {
    let mut v = RenderView {
        tick,
        producer: vec![PRODUCER_MAX * PRODUCER_SATURATION; CELL_COUNT],
        detritus: vec![SOIL_SCALE; CELL_COUNT],
        fruit: vec![0.0; CELL_COUNT],
        water: vec![0.0; CELL_COUNT],
        rain: vec![0.0; CELL_COUNT],
        producer_max: PRODUCER_MAX,
        organisms: Vec::new(),
    };
    for c in rained {
        v.rain[c.index()] = rate;
    }
    v
}

fn responding_cell(species: &str, face: Face) -> CellId {
    CellId::all()
        .find(|&c| c.face() == face && band_of(c) == Band::Foliage && plant_cap(Band::Foliage, c) == Some(2) && species_of(Band::Foliage, c) == species && (3..=12).contains(&c.cx()) && (3..=12).contains(&c.cy()))
        .unwrap_or_else(|| panic!("a rank-2 {species} slot on {face:?}"))
}

fn draw(p: &mut ArtPresenter, v: &RenderView, f: f64) -> Canvas {
    let mut c = Canvas::new();
    p.draw(v, f, &mut c);
    c
}

/// Largest channel difference between two canvases inside `radius` of `cell`'s anchor.
fn max_diff_near(a: &Canvas, b: &Canvas, cell: CellId, radius: f64) -> f32 {
    let at = slot_of(cell).at;
    let mut worst = 0.0f32;
    for y in 0..FACE_SIZE as u8 {
        for x in 0..FACE_SIZE as u8 {
            if (f64::from(x) + 0.5 - at.u).hypot(f64::from(y) + 0.5 - at.v) > radius {
                continue;
            }
            let (p, q) = (a.get(cell.face(), x, y), b.get(cell.face(), x, y));
            for c in 0..3 {
                worst = worst.max((p[c] - q[c]).abs());
            }
        }
    }
    worst
}

/// A continuous presenter (response on) and its twin (response off), both grown 500
/// rainless ticks from bare ground: past every plant's climb, in a quiet wind interval.
fn grown_pair() -> (ArtPresenter, ArtPresenter, u64) {
    let mut on = ArtPresenter::new(pack());
    let mut off = ArtPresenter::new(pack()).without_rain_response();
    for tick in WIND_QUIET_TICK..WIND_QUIET_TICK + 500 {
        let v = rich_view(tick, &[], 0.0);
        on.observe(&v);
        off.observe(&v);
    }
    (on, off, WIND_QUIET_TICK + 500)
}

/// The rain response's own contribution to a frame: the difference between a presenter
/// and its response-off twin that has seen exactly the same views.
fn rain_part(on: &mut ArtPresenter, off: &mut ArtPresenter, v: &RenderView, cell: CellId) -> f32 {
    max_diff_near(&draw(on, v, 0.5), &draw(off, v, 0.5), cell, 12.0)
}

#[test]
fn a_restart_in_a_grown_rainless_field_is_reported_not_blamed_on_the_rain() {
    // Whatever a brand-new presenter changes about a grown field with no rain is the
    // restart's own business (growth snapped to its target, no clip in flight). Measured
    // and printed so the numbers below can be read against it.
    let cell = responding_cell("lanternstalk", Face::Front);
    let (_on, mut off, t) = grown_pair();
    let v = rich_view(t, &[], 0.0);
    off.observe(&v);
    let mut fresh = ArtPresenter::new(pack()).without_rain_response();
    fresh.observe(&v);
    let d = max_diff_near(&draw(&mut off, &v, 0.5), &draw(&mut fresh, &v, 0.5), cell, 12.0);
    println!("  restart with no rain in a grown field: difference {d:.3} of 1.0 near the stalk (0 = the restart itself changes nothing here)");
}

#[test]
fn restarted_in_saturating_rain_the_fresh_presenter_holds_the_same_level_and_rain_part() {
    let cell = responding_cell("lanternstalk", Face::Front);
    let (mut on, mut off, t) = grown_pair();
    let rate = (RAIN_RESPONSE_RATE * 2.0) as f32;
    for tick in t..t + 40 {
        let v = rich_view(tick, &[cell], rate);
        on.observe(&v);
        off.observe(&v);
    }
    let v = rich_view(t + 40, &[cell], rate);
    on.observe(&v);
    off.observe(&v);
    let mut fresh_on = ArtPresenter::new(pack());
    let mut fresh_off = ArtPresenter::new(pack()).without_rain_response();
    fresh_on.observe(&v);
    fresh_off.observe(&v);
    assert_eq!(fresh_on.rain_level_of(cell), (1.0, 1.0));
    // Two seconds into the shower the continuous level is 1 − e^(−2/0.3) ≈ 0.9987.
    let (_, continuous) = on.rain_level_of(cell);
    assert!(continuous > 0.99, "continuous level {continuous}");
    let cont = rain_part(&mut on, &mut off, &v, cell);
    let fresh = rain_part(&mut fresh_on, &mut fresh_off, &v, cell);
    println!("  restart mid-shower: level continuous {continuous:.4} vs restarted 1.0; rain part continuous {cont:.3}, restarted {fresh:.3} (of 1.0)");
    assert!(cont > 0.0, "the shower must be visible at all for this to mean anything");
    assert!((cont - fresh).abs() < 0.02, "the restarted rain part must match the continuous one: {cont} vs {fresh}");
}

#[test]
fn restarted_while_the_level_is_still_rising_the_snap_is_bounded_and_closes_quickly() {
    let cell = responding_cell("lanternstalk", Face::Front);
    let (mut on, mut off, t) = grown_pair();
    let rate = (RAIN_RESPONSE_RATE * 0.5) as f32; // target level 0.5, never saturating
    let restart = t + 2;
    let mut fresh_on = None;
    let mut fresh_off = None;
    let mut first = None;
    let mut closed_at = None;
    for tick in t..t + 80 {
        let v = rich_view(tick, &[cell], rate);
        on.observe(&v);
        off.observe(&v);
        if tick == restart {
            fresh_on = Some(ArtPresenter::new(pack()));
            fresh_off = Some(ArtPresenter::new(pack()).without_rain_response());
        }
        if let (Some(fo), Some(ff)) = (fresh_on.as_mut(), fresh_off.as_mut()) {
            fo.observe(&v);
            ff.observe(&v);
            let (_, lc) = on.rain_level_of(cell);
            let (_, lf) = fo.rain_level_of(cell);
            assert!(lf >= lc - 1e-6, "the restarted presenter can only be ahead: {lf} vs {lc}");
            // The rain parts of the two: the restarted one at its snapped level, the
            // continuous one lagging, on otherwise identical frames.
            let d = (rain_part(&mut on, &mut off, &v, cell) - rain_part(fo, ff, &v, cell)).abs();
            if first.is_none() {
                first = Some((d, lf - lc));
            }
            if (lf - lc).abs() < 0.02 && closed_at.is_none() {
                closed_at = Some(tick);
            }
        }
    }
    let (d0, gap0) = first.unwrap();
    let closed = (closed_at.expect("the levels must meet") - restart) as f64 * DT;
    println!("  restart in the rise: level gap {gap0:.3} (snapped 0.5 vs continuous), rain-part difference {d0:.3} of 1.0; levels within 0.02 after {closed:.2} s");
    assert!(gap0 > 0.3 && gap0 <= 0.5, "the snap jumps by the unelapsed attack: {gap0}");
    assert!(d0 < 0.5, "a bounded difference, not a different picture: {d0}");
    assert!(closed <= 4.0 * RAIN_ATTACK_SECONDS + DT, "closed within four attack constants: {closed}");
}

#[test]
fn restarted_just_after_the_rain_the_tail_is_cut_to_the_still_image_and_they_meet_at_the_floor() {
    let cell = responding_cell("lanternstalk", Face::Front);
    let (mut on, mut off, t) = grown_pair();
    let rate = (RAIN_RESPONSE_RATE * 2.0) as f32;
    for tick in t..t + 60 {
        let v = rich_view(tick, &[cell], rate);
        on.observe(&v);
        off.observe(&v);
    }
    // The rain ends at t+60; the host restarts five ticks (0.25 s) later.
    let restart = t + 65;
    let mut fresh_on = None;
    let mut fresh_off = None;
    let mut tail = Vec::new();
    let mut identical_at = None;
    for tick in t + 60..t + 200 {
        let v = rich_view(tick, &[], 0.0);
        on.observe(&v);
        off.observe(&v);
        if tick == restart {
            fresh_on = Some(ArtPresenter::new(pack()));
            fresh_off = Some(ArtPresenter::new(pack()).without_rain_response());
        }
        if let (Some(fo), Some(ff)) = (fresh_on.as_mut(), fresh_off.as_mut()) {
            fo.observe(&v);
            ff.observe(&v);
            assert_eq!(fo.rain_level_of(cell), (0.0, 0.0), "no rain seen, no level: the tail is dropped");
            assert_eq!(rain_part(fo, ff, &v, cell), 0.0, "the restarted presenter IS its own still image");
            // The continuous presenter's remaining tail is exactly what the restart cuts.
            let d = rain_part(&mut on, &mut off, &v, cell);
            tail.push(d);
            if d == 0.0 && identical_at.is_none() {
                identical_at = Some(tick);
            }
        }
    }
    let level_at_restart = (-(5.0 * DT) / RAIN_RELEASE_SECONDS).exp();
    let worst = tail.iter().cloned().fold(0.0f32, f32::max);
    println!("  restart 0.25 s after the rain: continuous level ≈ {level_at_restart:.2}; the cut tail is at most {worst:.3} of 1.0 (first frame {:.3}); the two agree from +{:.2} s after the rain", tail[0], (identical_at.unwrap() - (t + 60)) as f64 * DT);
    assert!(worst > 0.0 && worst < 0.5, "a bounded cut, not a different picture: {worst}");
    // The cut shrinks: the running maximum over half-second windows must not grow.
    let windows: Vec<f32> = tail.chunks(10).map(|w| w.iter().cloned().fold(0.0, f32::max)).collect();
    for pair in windows.windows(2).take(4) {
        assert!(pair[1] <= pair[0] + 0.02, "the tail must settle: {windows:?}");
    }
    let settle = RAIN_RELEASE_SECONDS * (1.0 / f64::from(RAIN_SETTLE_FLOOR)).ln();
    assert!((identical_at.unwrap() - (t + 60)) as f64 * DT <= settle + 2.0 * DT, "identical once the continuous level hits the floor (≤ {settle:.2} s after the rain)");
}
