//! **HUNGRY-SEARCH** (`rove`): a hungry forager with nothing in sense goes looking.
//! Hand-built inputs, every number read from the config the case sets — no default, ratio or
//! population is asserted.
//!
//! - A hungry body whose answers have been its own search past its patience sets out on a
//!   leg away from the ground it has covered, even when it faces that ground.
//! - Before its patience is up its own search runs untouched.
//! - A fed body never searches.
//! - A body that senses food goes to it as before; food at its mouth starts its patience
//!   over, a lead it steers by does not.
//! - Something in its way, or a later layer overriding the leg, ends it, and the next leg
//!   turns at least square to it.
//! - A body on a leg in a world turns toward the leg's heading (the tick wires it in).
//! - A body on a stem or a terrain face is left to its climb.
//! - `search_leg_bl` 0 turns the search off.
//! - LEADS-PAY: a lead followed for its `lead_give_up_bl` budget without a taste becomes a
//!   leg turned at least square from it, held over the heuristic's leads until it runs out;
//!   a taste starts the count over, so a lead that ends in food is never given up.

use super::*;
use crate::step::predation_fixture::*;
use crate::{Fauna, FaunaConfig, Founder, StartingStores, Steer};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig};

const GRAZER: Founder = Founder::Browser;

/// The wander a browser answers with on open ground.
const WANDER: Actions = Actions { forward: 1.0, turn: 0.6, feed: 0.0 };

fn config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    let t = &mut c.teachers;
    t.search_patience_s = 2.0;
    t.search_leg_bl = 4.0;
    t.search_cell_bl = 1.0;
    t.search_memory_half_life_s = 600.0;
    c
}

struct Case {
    fauna: Fauna,
    world: World,
    id: u64,
}

impl Case {
    fn new(cfg: FaunaConfig, hungry: bool) -> Case {
        let world = clock_off(11);
        let mut fauna = Fauna::new(cfg);
        let id = introduce(&mut fauna, &world, GRAZER, StartingStores { body: 1.0, reserve: 0.3 });
        place(&mut fauna, id, 3.0, 1.5, EAST);
        let a = animal_mut(&mut fauna, id);
        a.eating = hungry;
        a.emergency = false;
        Case { fauna, world, id }
    }

    fn period_s(&self) -> f64 {
        GRAZER.manifest().cadence_ticks().max(1) as f64 * DT
    }

    fn ring(&self) -> f64 {
        f64::from(self.world.config().width) * self.world.config().voxel_m
    }

    fn length(&self) -> f64 {
        crate::body_of(&self.fauna.config, &animal(&self.fauna, self.id)).expect("a body").length_m
    }

    /// One controller sample: the layer's answer to `seek`, then the clock moves a period.
    fn sample(&mut self, seek: Seek) -> Option<Actions> {
        let i = self.fauna.animals.iter().position(|a| a.id == self.id).expect("alive");
        let (period_s, ring) = (self.period_s(), self.ring());
        let out = search(&mut self.fauna, i, WANDER, seek, period_s, ring);
        self.fauna.tick += GRAZER.manifest().cadence_ticks().max(1);
        out
    }

    /// Samples covering at least `s` seconds.
    fn samples(&self, s: f64) -> usize {
        (s / self.period_s()).ceil() as usize + 1
    }

    /// Walk east along the lane a step per sample, its answers its own search, for `n`
    /// samples; returns the answers.
    fn walk_east(&mut self, n: usize, step_m: f64) -> Vec<Option<Actions>> {
        (0..n)
            .map(|_| {
                let p = animal(&self.fauna, self.id).pose;
                place(&mut self.fauna, self.id, p.x + step_m, p.z, EAST);
                self.sample(Seek::Search)
            })
            .collect()
    }
}

fn unit(h: f64) -> (f64, f64) {
    (h.sin(), h.cos())
}

#[test]
fn a_hungry_searcher_sets_out_away_from_ground_it_has_covered() {
    let cfg = config();
    let mut c = Case::new(cfg.clone(), true);
    let before = c.samples(cfg.teachers.search_patience_s) - 2;
    // Covering ground eastward, its patience not yet up: its own search runs.
    let step = 0.5 * c.length();
    assert!(c.walk_east(before, step).iter().all(Option::is_none), "its own search before its patience is up");
    // Turned round to face the ground it covered, patience up: it sets out away from it.
    let p = animal(&c.fauna, c.id).pose;
    place(&mut c.fauna, c.id, p.x, p.z, WEST);
    let mut go = None;
    for _ in 0..4 {
        go = go.or(c.sample(Seek::Search));
    }
    let go = go.expect("a hungry body past its patience searches");
    let (heading, _) = c.fauna.search_leg(c.id).expect("on a leg");
    let (hx, _) = unit(heading);
    assert!(hx > 0.0, "the leg leaves the covered ground behind (west): heading {heading}");
    assert_eq!(go.feed, WANDER.feed, "the search keeps its heuristic's feed");
}

#[test]
fn a_fed_body_does_not_search() {
    let cfg = config();
    let mut c = Case::new(cfg.clone(), false);
    let n = 3 * c.samples(cfg.teachers.search_patience_s);
    assert!(c.walk_east(n, 0.1).iter().all(Option::is_none), "a fed body's own walk runs");
    assert!(c.fauna.search_leg(c.id).is_none(), "and it sets out on no leg");
}

#[test]
fn a_body_that_senses_food_goes_to_it_and_a_taste_starts_its_patience_over() {
    let cfg = config();
    let mut c = Case::new(cfg.clone(), true);
    let n = c.samples(cfg.teachers.search_patience_s) + 2;
    assert!(c.walk_east(n, 0.1).iter().any(Option::is_some), "searching past its patience");
    assert!(c.sample(Seek::Lead).is_none(), "food in sense: its heuristic steers by it");
    assert!(c.sample(Seek::Search).is_some(), "a lead does not start the patience over");
    assert!(c.sample(Seek::Taste).is_none(), "food at its mouth: its heuristic feeds");
    assert!(c.fauna.search_leg(c.id).is_none(), "the leg is dropped");
    let again = c.samples(cfg.teachers.search_patience_s) - 2;
    assert!(c.walk_east(again, 0.1).iter().all(Option::is_none), "a taste starts its patience over");
}

#[test]
fn a_leg_a_later_layer_overrides_ends_and_the_next_turns_from_it() {
    let cfg = config();
    let mut c = Case::new(cfg.clone(), true);
    let n = c.samples(cfg.teachers.search_patience_s) + 2;
    c.walk_east(n, 0.1);
    let (first, _) = c.fauna.search_leg(c.id).expect("on a leg");
    overridden(&mut c.fauna, c.id);
    assert!(c.fauna.search_leg(c.id).is_none(), "the overridden leg ends");
    assert!(c.sample(Seek::Search).is_some(), "it searches on");
    let (next, _) = c.fauna.search_leg(c.id).expect("a new leg");
    let (a, b) = (unit(first), unit(next));
    assert!(a.0 * b.0 + a.1 * b.1 <= 1e-9, "the next leg turns at least square to it: {first} → {next}");
}

#[test]
fn something_in_the_way_ends_the_leg_and_the_next_leaves_it_behind() {
    let cfg = config();
    let mut c = Case::new(cfg.clone(), true);
    let n = c.samples(cfg.teachers.search_patience_s) + 2;
    c.walk_east(n, 0.1);
    let (east, _) = c.fauna.search_leg(c.id).expect("on a leg");
    // Facing along the leg, something is in the way: its heuristic's answer stands.
    let p = animal(&c.fauna, c.id).pose;
    place(&mut c.fauna, c.id, p.x, p.z, east);
    assert!(c.sample(Seek::Obstacle).is_none(), "the wall rules answer an obstacle");
    assert!(c.fauna.search_leg(c.id).is_none(), "the obstacle ends the leg");
    assert!(c.sample(Seek::Search).is_some(), "it searches on");
    let (next, _) = c.fauna.search_leg(c.id).expect("a new leg");
    let (a, b) = (unit(east), unit(next));
    assert!(a.0 * b.0 + a.1 * b.1 < 1.0 - 1e-6, "the next leg turns off the obstacle: {east} → {next}");
}

#[test]
fn the_tick_steers_a_hungry_heuristic_body_along_its_leg() {
    let mut cfg = config();
    cfg.teachers.search_patience_s = 0.0;
    cfg.founders[GRAZER.index()].senses.live = false;
    let world = clock_off(11);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(cfg);
    let id = introduce(&mut fauna, &world, GRAZER, StartingStores { body: 1.0, reserve: 0.3 });
    place(&mut fauna, id, 6.0, 1.5, EAST);
    animal_mut(&mut fauna, id).eating = true;
    let teachers = fauna.config.teachers;
    fauna.controllers.set(id, crate::controller::heuristic(GRAZER, teachers));
    let period = GRAZER.manifest().cadence_ticks().max(1);
    let mut searched = false;
    for _ in 0..(8 * period) {
        animal_mut(&mut fauna, id).eating = true;
        fauna.step_with(&world, &mut flora, 1);
        if fauna.steer_of(id) == Some(Steer::Search) {
            searched = true;
            let a = animal(&fauna, id);
            let (heading, _) = fauna.search_leg(id).expect("on a leg");
            let off = (heading - a.pose.heading_rad + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
            let held = a.founder_state.held;
            assert!(off.abs() < 1e-9 || held.turn.signum() == off.signum(), "turns toward its leg: off {off}, {held:?}");
        }
    }
    assert!(searched, "a hungry heuristic body with nothing in sense searched");
}

#[test]
fn a_body_on_a_stem_or_a_face_is_left_to_its_climb() {
    let cfg = config();
    for mode in [crate::MobilityMode::Climb, crate::MobilityMode::Wall] {
        let mut c = Case::new(cfg.clone(), true);
        animal_mut(&mut c.fauna, c.id).mobility.mode = mode;
        let n = 3 * c.samples(cfg.teachers.search_patience_s);
        assert!(c.walk_east(n, 0.1).iter().all(Option::is_none), "{mode:?}: its own climb runs");
    }
}

#[test]
fn a_zero_leg_turns_the_search_off() {
    let mut cfg = config();
    cfg.teachers.search_leg_bl = 0.0;
    let mut c = Case::new(cfg.clone(), true);
    let n = 3 * c.samples(cfg.teachers.search_patience_s);
    assert!(c.walk_east(n, 0.1).iter().all(Option::is_none), "off");
    assert!(c.fauna.rove.is_empty(), "and it keeps no memory");
}

/// A config whose lead budget runs out on the grazer's `samples`-th sample of leading.
fn lead_config(samples: f64) -> FaunaConfig {
    let mut c = config();
    let period_s = GRAZER.manifest().cadence_ticks().max(1) as f64 * DT;
    c.teachers.lead_give_up_bl = (samples - 0.5) * period_s * c.founder(GRAZER).cruise_body_lengths_per_s;
    c
}

#[test]
fn a_lead_that_never_brings_a_taste_becomes_a_leg_away_from_it() {
    let budget = 4usize;
    let cfg = lead_config(budget as f64);
    let mut c = Case::new(cfg.clone(), true);
    for k in 0..budget - 1 {
        assert!(c.sample(Seek::Lead).is_none(), "sample {k}: its heuristic steers by the lead");
    }
    let go = c.sample(Seek::Lead).expect("its budget spent: the lead is given up for a leg");
    assert_eq!(c.fauna.lead_give_ups()[GRAZER.index()], 1, "counted");
    let (heading, _) = c.fauna.search_leg(c.id).expect("on a leg");
    let (a, b) = (unit(EAST), unit(heading));
    assert!(a.0 * b.0 + a.1 * b.1 <= 1e-9, "the leg turns at least square from the way it was led (east): {heading}");
    assert_eq!(go.feed, WANDER.feed, "at its heuristic's own feed");
    assert!(c.sample(Seek::Lead).is_some(), "the leg holds over the lead while it runs");
    assert_eq!(c.fauna.search_leg(c.id).map(|l| l.0), Some(heading), "the same leg");
}

#[test]
fn a_taste_starts_the_lead_count_over() {
    let budget = 4usize;
    let cfg = lead_config(budget as f64);
    let mut c = Case::new(cfg.clone(), true);
    for round in 0..4 {
        for k in 0..budget - 1 {
            assert!(c.sample(Seek::Lead).is_none(), "round {round} sample {k}: led");
        }
        assert!(c.sample(Seek::Taste).is_none(), "round {round}: it feeds");
    }
    assert_eq!(c.fauna.lead_give_ups()[GRAZER.index()], 0, "a lead that ends in food is never given up");
    assert!(c.fauna.search_leg(c.id).is_none());
}

#[test]
fn a_zero_lead_budget_follows_a_lead_on() {
    let mut cfg = config();
    cfg.teachers.lead_give_up_bl = 0.0;
    let mut c = Case::new(cfg.clone(), true);
    for k in 0..40 {
        assert!(c.sample(Seek::Lead).is_none(), "sample {k}: led");
    }
    assert_eq!(c.fauna.lead_give_ups()[GRAZER.index()], 0);
}
