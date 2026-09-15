//! One episode on one layout, through the **real** core tick loop.
//!
//! There is no second simulator and no copied neural forward pass. An episode is exactly:
//!
//! 1. [`Layout::build`] — `World::new`, stage the fields and the grazer, `World::from_state`.
//! 2. For a candidate, `World::attach_neural_policy(id, policy)` — the single explicit door
//!    into the recurrent extension, which gives the animal fresh zero `hidden`, `held` and
//!    `feedback` at the current tick.
//! 3. `World::set_scripted_intents` with `bud: Some(false)` — the diagnostic seam's only use
//!    in a candidate rollout, applied identically to every arm, so births are disabled without
//!    touching any reproduction rule.
//! 4. `World::step()` until the horizon or until the organism is gone.
//!
//! Nothing carries over between episodes: every candidate gets a fresh isolated world and
//! private state, and no experience, hidden state or field is reused.
//!
//! # What is measured
//!
//! Survival ticks and usable terminal stores are the *score* (§4 of the brief). Everything
//! else here is a **diagnostic**, reported separately and never summed into the ordering:
//! actual mouth intake from the world's own [`cubarium_core::IntakeDiagnostics`], paid upkeep
//! and paid motion priced from the **resolved** motion through `MotorBill`, and the four
//! behavioural descriptors the brief lists (body-length displacement, distinct cells, time in
//! the opening patch, turn sweep). No mode label and no field-stock delta is used as a proxy
//! for intake.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};

use cubarium_core::diagnostic::ScriptedIntent;
use cubarium_core::motor::MotorBill;
use cubarium_core::neural::Policy;
use cubarium_core::{DT, World};
use cubarium_surface::{CellId, Vec2, cell_of};
use serde::{Deserialize, Serialize};

use super::fixture::Layout;

/// How often the shared cancellation flag is read inside an episode. Every 128 ticks is about
/// 4 ms of simulated time and well under a millisecond of wall time, so a cancelled generation
/// stops promptly without putting an atomic load on the hot path.
pub const CANCEL_CHECK_TICKS: u64 = 128;

/// What drives the body for the whole episode.
#[derive(Clone, Debug)]
pub enum Driver {
    /// A candidate policy, through the world's own recurrent dispatch.
    Policy(Box<Policy>),
    /// A disclosed diagnostic script. **Never** used for a candidate rollout.
    Control(Control),
}

/// The three controls the fixture protocol requires (brief §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Control {
    /// Stand still, take nothing: what the starting stores alone buy.
    NoIntake,
    /// Stand still on the opening patch and graze continuously: the legacy `feed_min` gate
    /// bypassed, so this is the most favourable stationary strategy that exists.
    StationaryGrazing,
    /// The disclosed mobile script: tour the layout's food cells in the declared route order,
    /// dwelling on a cell until its `P` falls below the world's own `feed_min`, then walking to
    /// the next one on ordinary paid motion through `motor::resolve`.
    MobileScript,
}

impl Control {
    pub fn name(self) -> &'static str {
        match self {
            Control::NoIntake => "no-intake",
            Control::StationaryGrazing => "stationary-grazing",
            Control::MobileScript => "mobile-script",
        }
    }
}

/// How much per-tick accounting an episode does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    /// Survival and terminal stores only: what the score needs. Used for candidate rollouts.
    Score,
    /// Everything in [`Episode`]. Used for the controls and the smoke.
    Full,
}

/// The outcome of one episode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub layout: String,
    /// Ticks actually simulated. Equal to the horizon when the body survived it; the tick the
    /// body was found gone otherwise. Ordinary death is a completed episode, not an error.
    pub ticks: u64,
    pub alive: bool,
    /// `E + e_r·R` at the end, or 0 for a dead animal.
    pub terminal_stores: f64,
    /// `E_max + e_r·R_max`: the fixed body capacity the normalisation divides by.
    pub store_capacity: f64,
    // --- diagnostics, never part of the ordering ---
    pub intake_producer: f64,
    pub intake_fruit: f64,
    pub intake_detritus: f64,
    pub upkeep_paid: f64,
    pub motion_paid: f64,
    /// `E + e_r·R` at the first tick. With `terminal_stores`, the intake columns and the two
    /// paid columns this closes the episode's own energy box: what the body started with, what
    /// it actually took through its mouth, what it actually paid, and what it had left.
    pub store_start: f64,
    pub travelled_px: f64,
    pub body_lengths: f64,
    pub distinct_cells: usize,
    pub ticks_in_opening: u64,
    pub turn_sweep_rad: f64,
    pub route_p_start: f64,
    pub route_p_end: f64,
    pub route_p_grown: f64,
}

impl Episode {
    /// `clip(terminal usable stores / body capacity, 0, 1)`; zero for a dead animal.
    pub fn normalized_stores(&self) -> f64 {
        if !self.alive || self.store_capacity <= 0.0 {
            return 0.0;
        }
        (self.terminal_stores / self.store_capacity).clamp(0.0, 1.0)
    }

    pub fn seconds(&self) -> f64 {
        self.ticks as f64 * DT
    }
}

/// A run that was stopped by the shared cancellation flag. An incomplete generation never
/// updates the centre, so this is a distinct outcome rather than a zero score.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cancelled;

/// Run one episode.
pub fn run(
    layout: &Layout,
    driver: &Driver,
    horizon: u64,
    detail: Detail,
    cancel: &AtomicBool,
) -> Result<Episode, Cancelled> {
    let (mut world, id) = layout.build().expect("a frozen layout builds");
    let cfg = world.config().clone();
    let e_r = cfg.organism.reserve_energy_density;
    let leave_below = cfg.drives.feed_min;
    let route = layout.route();
    let opening: BTreeSet<u16> =
        layout.patches[0].cells(layout.face()).iter().map(|c| c.0).collect();

    let (extent, capacity) = {
        let o = world.state.organisms.get(id).expect("the grazer");
        (
            o.phenotype.extent,
            o.phenotype.energy_max + e_r * o.phenotype.reserve_max,
        )
    };

    let start_heading = layout.heading_vec();
    match driver {
        Driver::Policy(policy) => {
            world
                .attach_neural_policy(id, (**policy).clone())
                .expect("a validated policy attaches to an ordinary body");
            // The only scripted field a candidate rollout uses: births off, equally, in every
            // arm. `apply` is `d.bud = d.bud && false`, so it can suppress a request and can
            // never create one.
            world.set_scripted_intents(vec![(id, ScriptedIntent {
                bud: Some(false),
                ..ScriptedIntent::default()
            })]);
        }
        Driver::Control(Control::NoIntake) => {
            world.set_scripted_intents(vec![(id, ScriptedIntent {
                heading: Some(start_heading),
                effort: Some(0.0),
                graze_effort: Some(0.0),
                fruit_effort: Some(0.0),
                scavenge_effort: Some(0.0),
                mode: None,
                bud: Some(false),
            })]);
        }
        Driver::Control(Control::StationaryGrazing) => {
            world.set_scripted_intents(vec![(id, ScriptedIntent {
                heading: Some(start_heading),
                effort: Some(0.0),
                graze_effort: Some(1.0),
                fruit_effort: Some(0.0),
                scavenge_effort: Some(0.0),
                mode: None,
                bud: Some(false),
            })]);
        }
        Driver::Control(Control::MobileScript) => {}
    }

    let route_p_start: f64 = route.iter().map(|c| world.state.fields.p[c.index()]).sum();
    let mut episode = Episode {
        layout: layout.name.clone(),
        ticks: 0,
        alive: true,
        terminal_stores: 0.0,
        store_capacity: capacity,
        intake_producer: 0.0,
        intake_fruit: 0.0,
        intake_detritus: 0.0,
        upkeep_paid: 0.0,
        motion_paid: 0.0,
        store_start: 0.0,
        travelled_px: 0.0,
        body_lengths: 0.0,
        distinct_cells: 0,
        ticks_in_opening: 0,
        turn_sweep_rad: 0.0,
        route_p_start,
        route_p_end: 0.0,
        route_p_grown: 0.0,
    };

    episode.store_start = {
        let o = world.state.organisms.get(id).expect("the grazer");
        o.energy + e_r * o.reserve
    };
    let mut visited: BTreeSet<u16> = BTreeSet::new();
    let mut target = 0usize;

    for tick in 0..horizon {
        if tick % CANCEL_CHECK_TICKS == 0 && cancel.load(Ordering::Relaxed) {
            return Err(Cancelled);
        }
        let Some(o) = world.state.organisms.get(id) else {
            episode.alive = false;
            episode.ticks = tick;
            break;
        };
        let here = cell_of(&o.pos);
        let heading_before = o.heading;
        let chart_before = o.pos.chart();
        let face_before = o.pos.face;
        let bill = MotorBill::of(o, &cfg);

        if let Driver::Control(Control::MobileScript) = driver {
            // The disclosed rule, in full: dwell and crop until the cell falls below the
            // world's own `feed_min`, then face the next route cell's centre and walk. The
            // heading is a *request*; `motor::resolve` decides how much of the turn and how
            // much of the walk the body can pay for this tick.
            let goal = route[target % route.len()];
            let intent = if here == goal {
                ScriptedIntent {
                    heading: Some(heading_before),
                    effort: Some(0.0),
                    graze_effort: Some(1.0),
                    fruit_effort: Some(0.0),
                    scavenge_effort: Some(0.0),
                    mode: None,
                    bud: Some(false),
                }
            } else {
                let toward = toward(&world, goal, chart_before, face_before)
                    .unwrap_or(heading_before);
                ScriptedIntent {
                    heading: Some(toward),
                    effort: Some(1.0),
                    graze_effort: Some(1.0),
                    fruit_effort: Some(0.0),
                    scavenge_effort: Some(0.0),
                    mode: None,
                    bud: Some(false),
                }
            };
            world.set_scripted_intents(vec![(id, intent)]);
        }

        if detail == Detail::Full {
            visited.insert(here.0);
            if opening.contains(&here.0) {
                episode.ticks_in_opening += 1;
            }
        }

        world.step();
        world.drain_events();
        episode.ticks = tick + 1;

        let Some(o) = world.state.organisms.get(id) else {
            episode.alive = false;
            break;
        };

        if detail == Detail::Full {
            let travelled: f64 = world
                .moved_segments(id)
                .iter()
                .map(cubarium_surface::PathSegment::length)
                .sum();
            let turn = if o.pos.face == face_before {
                signed_turn(heading_before, o.heading).abs()
            } else {
                // A seam crossing changes the chart; that is transport, not a physical turn.
                0.0
            };
            episode.travelled_px += travelled;
            episode.turn_sweep_rad += turn;
            episode.upkeep_paid += bill.upkeep(DT);
            episode.motion_paid += bill.motor_cost(travelled / DT, extent * turn / DT, DT);
        }

        if let Driver::Control(Control::MobileScript) = driver {
            let goal = route[target % route.len()];
            if cell_of(&o.pos) == goal && world.state.fields.p[goal.index()] < leave_below {
                target += 1;
            }
        }
    }

    if let Some(o) = world.state.organisms.get(id) {
        episode.alive = true;
        episode.terminal_stores = o.energy + e_r * o.reserve;
    } else {
        episode.alive = false;
        episode.terminal_stores = 0.0;
    }

    let diag = world.intake_diagnostics();
    episode.intake_producer = diag.producer_eaten;
    episode.intake_fruit = diag.fruit_eaten;
    episode.intake_detritus = diag.detritus_eaten;
    episode.route_p_grown = diag.producer_growth;
    episode.route_p_end = route.iter().map(|c| world.state.fields.p[c.index()]).sum();
    episode.distinct_cells = visited.len();
    episode.body_lengths = if extent > 0.0 { episode.travelled_px / extent } else { 0.0 };
    Ok(episode)
}

/// A unit heading from the body's chart position toward a goal cell's centre, when both are on
/// the same face. Returns `None` across a seam, where the caller keeps its current heading.
fn toward(world: &World, goal: CellId, from: Vec2, face: cubarium_surface::Face) -> Option<Vec2> {
    let _ = world;
    if goal.face() != face {
        return None;
    }
    (goal.center().chart() - from).normalized()
}

fn signed_turn(a: Vec2, b: Vec2) -> f64 {
    use std::f64::consts::{PI, TAU};
    let d = b.screen_angle() - a.screen_angle();
    let d = (d + PI).rem_euclid(TAU) - PI;
    if d.is_finite() { d } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::es::fixture::training_layouts;

    #[test]
    fn a_still_body_with_no_intake_eats_nothing_and_keeps_its_stores_falling() {
        let cancel = AtomicBool::new(false);
        let l = &training_layouts()[0];
        let e = run(l, &Driver::Control(Control::NoIntake), 400, Detail::Full, &cancel)
            .expect("not cancelled");
        assert_eq!(e.intake_producer, 0.0, "a closed mouth records exactly zero intake");
        assert_eq!(e.travelled_px, 0.0, "effort 0 is a genuine request for stillness");
        assert!(e.upkeep_paid > 0.0, "living is not free");
        assert!(e.alive, "400 ticks is well inside the starting stores");
        assert!(e.terminal_stores < e.store_capacity);
    }

    #[test]
    fn a_stationary_grazer_eats_from_its_own_cell_only() {
        let cancel = AtomicBool::new(false);
        let l = &training_layouts()[0];
        let e = run(l, &Driver::Control(Control::StationaryGrazing), 400, Detail::Full, &cancel)
            .expect("not cancelled");
        assert!(e.intake_producer > 0.0, "an open mouth on a fed cell eats");
        assert_eq!(e.travelled_px, 0.0);
        assert_eq!(e.distinct_cells, 1, "a stationary body visits one cell");
    }

    #[test]
    fn the_mobile_script_travels_and_visits_more_than_one_cell() {
        let cancel = AtomicBool::new(false);
        let l = &training_layouts()[0];
        let e = run(l, &Driver::Control(Control::MobileScript), 4_000, Detail::Full, &cancel)
            .expect("not cancelled");
        assert!(e.travelled_px > 0.0, "the script pays for real motion");
        assert!(e.distinct_cells > 1, "the script relocates");
        assert!(e.motion_paid > 0.0, "and pays for it");
        assert!(e.intake_producer > 0.0);
    }

    #[test]
    fn cancellation_stops_an_episode_without_a_score() {
        let cancel = AtomicBool::new(true);
        let l = &training_layouts()[0];
        let out = run(l, &Driver::Control(Control::NoIntake), 36_000, Detail::Score, &cancel);
        assert_eq!(out, Err(Cancelled));
    }

    #[test]
    fn a_zero_policy_runs_through_the_real_dispatch_and_holds_still() {
        use crate::es::tensor;
        let cancel = AtomicBool::new(false);
        let l = &training_layouts()[0];
        let policy = tensor::policy(&vec![0.0; tensor::PARAMS]).expect("valid");
        let e = run(l, &Driver::Policy(Box::new(policy)), 200, Detail::Full, &cancel)
            .expect("not cancelled");
        // A zero policy's head is its bias, which is zero: `sigma(0) = 0.5` on thrust, so it
        // is *not* still — what matters here is that the recurrent dispatch actually ran.
        assert!(e.alive);
        assert_eq!(e.ticks, 200);
        assert!(e.terminal_stores > 0.0);
    }

    #[test]
    fn two_runs_of_the_same_episode_agree_exactly() {
        use crate::es::tensor;
        let cancel = AtomicBool::new(false);
        let l = &training_layouts()[1];
        let policy = tensor::policy(&tensor::initial_center(5)).expect("valid");
        let a = run(l, &Driver::Policy(Box::new(policy.clone())), 600, Detail::Full, &cancel)
            .expect("ok");
        let b = run(l, &Driver::Policy(Box::new(policy)), 600, Detail::Full, &cancel).expect("ok");
        assert_eq!(a, b);
    }
}
