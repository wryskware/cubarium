//! The herding prototype (`design/handoffs/voxel-browser-movement-2026-09-28.md`, 3): the
//! heuristic browser plus cohesion toward the bodies its cone already sees, and a little
//! separation. Observation-only, no sense or contract change.

use cubarium_voxel_fauna::{BrowserForager, Controller, Founder, Response};

use super::controller::{EpisodeKind, HerdingBrowser, VoxelControl};
use super::live::search::brain_driver;

/// The cone module's offset in the browser's observation.
fn cone() -> usize {
    Founder::Browser
        .manifest()
        .modules
        .iter()
        .find(|m| m.name.starts_with("Cone"))
        .expect("the browser has a cone")
        .offset
}

/// Flat open ground, a working motor, nothing to eat or taste: the plain forager walks
/// on at full effort.
fn open_ground() -> Vec<f64> {
    let mut o = vec![0.0; Founder::Browser.manifest().inputs()];
    o[7] = 1.0; // motor delivery
    let c = cone();
    for k in 0..3 {
        o[c + 6 * k] = 0.6; // clear, as on flat ground
        o[c + 6 * k + 1] = 0.83; // the floor rows' proximity
    }
    o[c + 18] = 1.0; // valid
    o
}

fn with_body(mut o: Vec<f64>, sector: usize, proximity: f64) -> Vec<f64> {
    let c = cone();
    o[c + 6 * sector + 4] = 0.3;
    o[c + 6 * sector + 5] = proximity;
    o
}

fn turn(r: Response) -> (f64, f64) {
    match r {
        Response::Bounded(a) => (a.forward, a.turn),
        other => panic!("a heuristic answers bounded actions, not {other:?}"),
    }
}

/// With no food in view, a body seen at a distance draws the herder toward it (sector 0
/// is left, a negative turn; sector 2 right, positive).
#[test]
fn cohesion_turns_toward_a_body_seen_far_off() {
    let mut h = HerdingBrowser::new();
    let (f, t) = turn(h.drive(&with_body(open_ground(), 2, 0.2)));
    assert!(
        f > 0.9 && t > 0.5,
        "toward the right: forward {f}, turn {t}"
    );
    let mut h = HerdingBrowser::new();
    let (_, t) = turn(h.drive(&with_body(open_ground(), 0, 0.2)));
    assert!(t < -0.5, "toward the left: {t}");
}

/// A body very close turns the herder away: separation.
#[test]
fn separation_turns_away_from_a_body_too_close() {
    let mut h = HerdingBrowser::new();
    let (_, t) = turn(h.drive(&with_body(open_ground(), 2, 0.9)));
    assert!(t < -0.5, "away from the right: {t}");
}

/// Food in view wins: with foliage seen, the herder forages exactly as the plain
/// heuristic does, whatever bodies are about.
#[test]
fn food_in_view_is_the_plain_foragers_answer() {
    let c = cone();
    let mut o = with_body(open_ground(), 2, 0.2);
    o[c + 2] = 0.5; // foliage in sector 0
    o[c + 3] = 0.5;
    let (mut h, mut p) = (HerdingBrowser::new(), BrowserForager::new());
    let (a, b) = (turn(h.drive(&o)), turn(p.drive(&o)));
    assert_eq!(a, b);
}

/// With nothing seen at all, the herder is the plain forager.
#[test]
fn nothing_seen_is_the_plain_foragers_answer() {
    let (mut h, mut p) = (HerdingBrowser::new(), BrowserForager::new());
    for _ in 0..6 {
        assert_eq!(turn(h.drive(&open_ground())), turn(p.drive(&open_ground())));
    }
}

/// The check takes `herding` for the browser, and refuses it for a lineage with no cone.
#[test]
fn herding_is_a_browser_brain_option() {
    let g = brain_driver(Founder::Browser, "herding").expect("herding");
    assert!(matches!(
        g.kind(),
        EpisodeKind::Control(VoxelControl::Herding)
    ));
    assert!(brain_driver(Founder::Blind, "herding").is_err());
}
