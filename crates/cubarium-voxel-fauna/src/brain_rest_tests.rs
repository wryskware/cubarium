//! **BRAIN-REST** (`design/handoffs/brain-rest-2026-10-12.md`): a brain's granted rest lasts
//! one bout (`rest.brain_bout_s`), then its ask clears and the brain is sampled again; a wake
//! or an emergency ends the bout sooner; the ask and the bout survive a save; a heuristic
//! body's sleep is untouched. A child of `step` beside the predation fixture.
//!
//! Every case sets the knobs it relies on and reads the bout back from the config; no shipped
//! number is asserted.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::predation_fixture::*;
use crate::calls::Heed;
use crate::controller::{Actions, Controller, Decision, DecisionInput, Response};
use crate::{Activity, Fauna, FaunaConfig, Founder, State};
use cubarium_voxel::World;
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel_flora::{Flora, FloraConfig};

const LS: Founder = Founder::Loftstrider;

/// A menu brain that answers "stand still" and asks to rest while `rest` is set; `once`
/// clears `rest` after its first ask. Counts its samples and records the tick of each.
struct Asker {
    rest: Arc<AtomicBool>,
    once: bool,
    samples: Arc<AtomicUsize>,
}

impl Controller for Asker {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        Response::Bounded(Actions::REST)
    }
    fn reset(&mut self) {}
    fn decide(&mut self, _o: &[f64], _input: &DecisionInput<'_>) -> (Response, Option<Decision>) {
        self.samples.fetch_add(1, Ordering::Relaxed);
        let rest = self.rest.load(Ordering::Relaxed);
        if rest && self.once {
            self.rest.store(false, Ordering::Relaxed);
        }
        (Response::Bounded(Actions::REST), Some(Decision { rest, ..Decision::default() }))
    }
}

/// A heuristic-layer body holding still (its rest decisions are the rhythm's).
struct StillHeuristic(Arc<AtomicUsize>);

impl Controller for StillHeuristic {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        self.0.fetch_add(1, Ordering::Relaxed);
        Response::Bounded(Actions::REST)
    }
    fn reset(&mut self) {}
    fn layer(&self) -> crate::Layer {
        crate::Layer::Heuristic
    }
}

fn world(seed: u64) -> World {
    plain_of_height(DayClock { mode: DayMode::Off, ..DayClock::default() }, seed, 16)
}

/// Every lineage awake by its rhythm; the loftstrider's rest without deep bouts (so a bout's
/// end is never deferred) and its brain bout set to a few controller periods.
fn config() -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    let r = &mut c.founders[LS.index()].rest;
    r.deep_per_min = 0.0;
    r.need_s = 0.0;
    r.brain_bout_s = 5.0 * LS.manifest().controller_period_s;
    c
}

fn bout_ticks(f: &Fauna) -> u64 {
    cubarium_voxel::ticks_at_least_one(f.config.founder(LS).rest.brain_bout_s)
}

fn asker(f: &mut Fauna, w: &World, x: f64, once: bool) -> (u64, Arc<AtomicBool>, Arc<AtomicUsize>) {
    let id = introduce(f, w, LS, crate::StartingStores::FULL);
    place(f, id, x, LANE_Z, EAST);
    let rest = Arc::new(AtomicBool::new(true));
    let samples = Arc::new(AtomicUsize::new(0));
    assert!(f.set_controller(id, Box::new(Asker { rest: rest.clone(), once, samples: samples.clone() })));
    (id, rest, samples)
}

fn asleep(f: &Fauna, id: u64) -> bool {
    animal(f, id).state == State::Asleep
}

/// Step until the body falls asleep (at most `max` ticks); the tick count it took.
fn until_asleep(f: &mut Fauna, w: &World, flora: &mut Flora, id: u64, max: u32) -> u32 {
    step_until(f, w, flora, max, |f| asleep(f, id)).expect("the brain's ask is granted")
}

/// The run of asleep ticks from now, stepping at most `max`.
fn asleep_run(f: &mut Fauna, w: &World, flora: &mut Flora, id: u64, max: u64) -> u64 {
    let mut n = 0;
    while asleep(f, id) && n < max {
        f.step_with(w, flora, 1);
        n += 1;
    }
    n
}

/// A brain that asks to rest once sleeps one bout (the knob's length), then is awake and
/// sampled again on its cadence, and does not sleep again; one that keeps asking sleeps
/// another bout after its next sample.
#[test]
fn a_brain_that_asks_to_rest_once_sleeps_one_bout_then_is_sampled_again() {
    let w = world(61);
    for once in [true, false] {
        let mut f = Fauna::new(config());
        f.set_births_enabled(false);
        let mut flora = Flora::new(FloraConfig::default());
        let (id, _rest, samples) = asker(&mut f, &w, 3.0, once);
        let period = LS.manifest().cadence_ticks();
        until_asleep(&mut f, &w, &mut flora, id, 4 * period as u32);
        let bout = bout_ticks(&f);
        let before = samples.load(Ordering::Relaxed);
        let run = asleep_run(&mut f, &w, &mut flora, id, 4 * bout);
        assert_eq!(run, bout, "once={once}: the granted rest lasts one bout");
        assert_eq!(samples.load(Ordering::Relaxed), before, "an asleep brain is not sampled");
        assert!(!f.rest_asks.contains_key(&id), "the ask cleared at the bout's end");
        // Awake: the brain is sampled on its next cadence tick.
        assert!(step_until(&mut f, &w, &mut flora, period as u32 + 1, |_| samples.load(Ordering::Relaxed) > before).is_some(), "once={once}: sampled again");
        let slept_again = step_until(&mut f, &w, &mut flora, 2 * period as u32 + 2, |f| asleep(f, id)).is_some();
        assert_eq!(slept_again, !once, "once={once}: only a fresh ask sleeps again");
        if !once {
            assert_eq!(asleep_run(&mut f, &w, &mut flora, id, 4 * bout), bout, "the second ask sleeps another bout");
        }
    }
}

/// With the knob at 0 a brain's rest has no bout: it sleeps on until something wakes it.
#[test]
fn a_zero_bout_holds_the_ask_until_a_wake() {
    let w = world(62);
    let mut c = config();
    c.founders[LS.index()].rest.brain_bout_s = 0.0;
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let mut flora = Flora::new(FloraConfig::default());
    let (id, _rest, _samples) = asker(&mut f, &w, 3.0, true);
    until_asleep(&mut f, &w, &mut flora, id, 4 * LS.manifest().cadence_ticks() as u32);
    let span = 4 * cubarium_voxel::ticks_at_least_one(5.0 * LS.manifest().controller_period_s);
    assert_eq!(asleep_run(&mut f, &w, &mut flora, id, span), span);
}

/// A heeded call heard mid-bout wakes the body (its alert), ending the bout early; an
/// emergency does the same.
#[test]
fn a_wake_or_an_emergency_ends_a_brains_bout_early() {
    let w = world(63);
    // A wake: a neighbour's alarm call, heeded by everyone.
    let mut c = config();
    c.founders[LS.index()].rest.brain_bout_s = 200.0 * LS.manifest().controller_period_s;
    let t = &mut c.founders[LS.index()].calls;
    t.range_m = 5.0;
    t.loudness = [1.0, 1.0];
    t.heed = [Heed::Everyone, Heed::Adults];
    t.heed_s = [4.0, 4.0];
    t.keep_s = 10.0;
    let mut f = Fauna::new(c.clone());
    f.set_births_enabled(false);
    let mut flora = Flora::new(FloraConfig::default());
    let (id, rest, _samples) = asker(&mut f, &w, 3.0, false);
    let caller = introduce(&mut f, &w, LS, crate::StartingStores::FULL);
    place(&mut f, caller, 5.0, LANE_Z, WEST);
    drive(&mut f, caller, 0.0, false);
    until_asleep(&mut f, &w, &mut flora, id, 4 * LS.manifest().cadence_ticks() as u32);
    rest.store(false, Ordering::Relaxed);
    for _ in 0..3 {
        f.step_with(&w, &mut flora, 1);
    }
    assert!(asleep(&f, id) && f.brain_bouts.contains_key(&id), "mid-bout");
    let i = f.animals.iter().position(|a| a.id == caller).unwrap();
    assert!(crate::calls::emit(&mut f, i, 0), "the alarm is emitted");
    let woke = step_until(&mut f, &w, &mut flora, 5, |f| !asleep(f, id));
    assert!(woke.is_some(), "the call wakes it before its bout ends");
    assert!(!f.brain_bouts.contains_key(&id), "the wake ended the bout");

    // An emergency: hunger past its wake line mid-bout.
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let mut flora = Flora::new(FloraConfig::default());
    let (id, _rest, _samples) = asker(&mut f, &w, 3.0, false);
    until_asleep(&mut f, &w, &mut flora, id, 4 * LS.manifest().cadence_ticks() as u32);
    f.step_with(&w, &mut flora, 1);
    assert!(asleep(&f, id));
    f.config.founders[LS.index()].wake_hunger = 0.0;
    f.step_with(&w, &mut flora, 1);
    assert!(!asleep(&f, id), "the emergency wakes it");
    assert!(!f.brain_bouts.contains_key(&id), "the emergency ended the bout");
}

/// A heuristic body put to sleep by its rhythm sleeps on past any brain bout; a brain's bout
/// never starts for it.
#[test]
fn a_heuristic_bodys_sleep_is_untouched() {
    let w = at_hour(0.0, 64);
    let mut c = config();
    c.founders[LS.index()].active = Activity::Diurnal;
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let mut flora = Flora::new(FloraConfig::default());
    let id = introduce(&mut f, &w, LS, crate::StartingStores::FULL);
    place(&mut f, id, 3.0, LANE_Z, EAST);
    let samples = Arc::new(AtomicUsize::new(0));
    assert!(f.set_controller(id, Box::new(StillHeuristic(samples))));
    f.step_with(&w, &mut flora, 1);
    assert!(asleep(&f, id), "its rhythm puts it to sleep at midnight");
    let span = 3 * bout_ticks(&f);
    assert_eq!(asleep_run(&mut f, &w, &mut flora, id, span), span);
    assert!(f.brain_bouts.is_empty() && f.rest_asks.is_empty());
}

/// **A save mid-bout continues identically**: a body asleep on its own ask, saved halfway
/// through its bout, wakes at the same tick in the loaded world as in the uninterrupted one,
/// and its brain is next sampled at the same tick.
#[test]
fn a_save_mid_bout_continues_identically() {
    let w = world(65);
    let mut c = config();
    c.founders[LS.index()].rest.brain_bout_s = 20.0 * LS.manifest().controller_period_s;
    let mut f = Fauna::new(c);
    f.set_births_enabled(false);
    let mut flora = Flora::new(FloraConfig::default());
    let (id, _rest, samples) = asker(&mut f, &w, 3.0, true);
    until_asleep(&mut f, &w, &mut flora, id, 4 * LS.manifest().cadence_ticks() as u32);
    let half = bout_ticks(&f) / 2;
    for _ in 0..half {
        f.step_with(&w, &mut flora, 1);
    }
    assert!(asleep(&f, id), "saved mid-bout");
    let mut g = Fauna::load(&f.save()).expect("loads");
    g.set_births_enabled(false);
    let g_samples = Arc::new(AtomicUsize::new(0));
    assert!(g.set_controller(id, Box::new(Asker { rest: Arc::new(AtomicBool::new(false)), once: true, samples: g_samples.clone() })));
    let mut flora_g = flora.clone();
    let before = samples.load(Ordering::Relaxed);
    let mut trace = (Vec::new(), Vec::new());
    for _ in 0..(bout_ticks(&f) + 2 * LS.manifest().cadence_ticks()) {
        f.step_with(&w, &mut flora, 1);
        g.step_with(&w, &mut flora_g, 1);
        trace.0.push((asleep(&f, id), samples.load(Ordering::Relaxed) - before));
        trace.1.push((asleep(&g, id), g_samples.load(Ordering::Relaxed)));
    }
    assert!(trace.0.iter().any(|t| !t.0) && trace.0.iter().any(|t| t.1 > 0), "it woke and was sampled");
    assert_eq!(trace.0, trace.1, "the loaded body sleeps out the same bout and is sampled at the same ticks");
}
