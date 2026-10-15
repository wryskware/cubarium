//! **The decision log** (PG-PILOT): one lineage's bodies in one world's window, driven by
//! the centre with an exploring top ([`Explore`]) and read every tick.
//!
//! - At the window's start every body of the lineage gets a fresh brain on the centre with
//!   its **memory carried over** from the controller it had (its GRU state, hold and
//!   decision credit continue), sampling from then on. A newborn gets one the tick it
//!   appears, from the birth factory's brain the same way.
//! - [`Recorder::before_step`] sets the shared clock to the window tick the step starts
//!   from: a decision is stamped with it. [`Recorder::after_step`] reads every body's energy
//!   ([`super::PgBody::series`]): `(E + E_given)/body_ref`, `E` as the window's own score reads
//!   it (body, reserve, fuel and escrow; plus the digestible stomach on the stomach-aware
//!   reward), the given counter from its value at the start, and `E = 0` from a death on;
//!   and its drives and life stage ([`super::judge`], SCORE-V2: bookkeeping for the reward
//!   only, never read by behaviour). A newcomer born to a recorded mother notes her.
//! - The sampler is seeded per body from the generation's seed, the body's id and where it
//!   stands when its brain is installed.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use cubarium_voxel_fauna::{Animal, Founder, FounderPhysiology};
use cubarium_voxel_sim::Sim;
use rustc_hash::FxHashMap;

use super::judge::{DRIVES, LifeStage, drives, life_stage};
use super::reward::{PgBody, PgWorld};
use crate::es::voxel::two_layer::{Explore, IntentArbitration, TopDecision, TwoLayerNet};

/// What a window's exploring lineage runs: the centre, its holding knobs and the
/// generation's sampling seed.
#[derive(Clone, Debug)]
pub struct ExplorePlan {
    pub net: Arc<TwoLayerNet>,
    pub arbitration: IntentArbitration,
    pub seed: u64,
}

struct Body {
    id: u64,
    start: u64,
    given0: f64,
    given_last: f64,
    series: Vec<f64>,
    drives: Vec<[f64; DRIVES]>,
    stage: LifeStage,
    born_to: Option<u64>,
    alive: bool,
    died: bool,
    seen: u64,
    log: Arc<Mutex<Vec<TopDecision>>>,
}

/// One lineage's decision log over one world's window.
pub struct Recorder {
    founder: Founder,
    plan: ExplorePlan,
    clock: Arc<AtomicU64>,
    reference: f64,
    phys: FounderPhysiology,
    bodies: Vec<Body>,
    index: FxHashMap<u64, usize>,
    /// Stomach-aware energy (the window's experimental reward), read at each step.
    stomach_aware: bool,
}

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Recorder {
    /// Put every body of `founder` on an exploring brain and read its starting energy.
    pub fn begin(sim: &mut Sim, founder: Founder, plan: ExplorePlan, stomach_aware: bool) -> Recorder {
        let mut r = Recorder {
            founder,
            clock: Arc::new(AtomicU64::new(0)),
            reference: founder.manifest().body_reference,
            phys: *sim.fauna().config().founder(founder),
            plan,
            bodies: Vec::new(),
            index: FxHashMap::default(),
            stomach_aware,
        };
        let ids: Vec<u64> = sim
            .fauna()
            .view()
            .animals
            .iter()
            .filter(|a| a.founder == Some(founder))
            .map(|a| a.id)
            .collect();
        for id in ids {
            r.join(sim, id, 0);
        }
        r
    }

    pub fn founder(&self) -> Founder {
        self.founder
    }

    fn energy(&self, a: &Animal) -> f64 {
        let e = crate::es::voxel::live::window::body_energy(a);
        if self.stomach_aware {
            e + crate::es::voxel::live::targets::digestible_stomach(a, &self.phys)
        } else {
            e
        }
    }

    /// Install body `id`'s exploring brain, its memory carried over, and start its series
    /// at window tick `tick`.
    fn join(&mut self, sim: &mut Sim, id: u64, tick: u64) {
        let Some(a) = sim.fauna().view().animal(id).copied() else { return };
        let period = self.founder.manifest().controller_period_s;
        let mut brain = self.plan.net.brain(self.plan.arbitration, period);
        if let Some(m) = sim.fauna_mut().take_controller(id).and_then(|c| c.memory()) {
            // A memory of another kind of brain (a heuristic has none) starts fresh.
            let _ = brain.restore_record(&m);
        }
        let log = Arc::new(Mutex::new(Vec::new()));
        let seed = mix(self.plan.seed ^ mix(id.wrapping_add(0x51_7CC1_B727_220A)) ^ mix(a.pose.x.to_bits()));
        brain.explore_with(Explore::new(seed, Arc::clone(&self.clock), Arc::clone(&log)));
        sim.fauna_mut().set_controller(id, Box::new(brain));
        let given0 = a.reproduction.given;
        let s = self.energy(&a) / self.reference;
        let now = sim.fauna().tick();
        // A birth: a native newcomer whose mother is recorded (a body standing at the start,
        // or introduced, is not one; a hatchling records no mother).
        let born_to = a.mother.filter(|m| tick > 0 && a.reproduction.native && self.index.contains_key(m));
        self.index.insert(id, self.bodies.len());
        self.bodies.push(Body {
            id,
            start: tick,
            given0,
            given_last: given0,
            series: vec![s],
            drives: vec![drives(&a, &self.phys, now)],
            stage: life_stage(&a, &self.phys),
            born_to,
            alive: true,
            died: false,
            seen: tick,
            log,
        });
    }

    /// Before the sim step that runs from window tick `tick`.
    pub fn before_step(&self, tick: u64) {
        self.clock.store(tick, Ordering::Relaxed);
    }

    /// After the step that brought the window to tick `tick`: every body's reading, a
    /// newcomer's brain, a death.
    pub fn after_step(&mut self, sim: &mut Sim, tick: u64) {
        let mut newcomers = Vec::new();
        let now = sim.fauna().tick();
        for a in sim.fauna().view().animals.iter().filter(|a| a.founder == Some(self.founder)) {
            match self.index.get(&a.id) {
                Some(&k) => {
                    let e = self.energy(a);
                    let (d, stage) = (drives(a, &self.phys, now), life_stage(a, &self.phys));
                    let b = &mut self.bodies[k];
                    b.seen = tick;
                    b.given_last = a.reproduction.given;
                    b.series.push((e + a.reproduction.given - b.given0) / self.reference);
                    b.drives.push(d);
                    b.stage = stage;
                }
                None => newcomers.push(a.id),
            }
        }
        let reference = self.reference;
        for b in self.bodies.iter_mut().filter(|b| b.alive && b.seen != tick) {
            // Gone: its energy is zero from here, what it gave is kept.
            b.alive = false;
            b.died = true;
            b.series.push((b.given_last - b.given0) / reference);
            let last = b.drives.last().copied().unwrap_or([0.0; DRIVES]);
            b.drives.push(last);
        }
        for id in newcomers {
            self.join(sim, id, tick);
        }
    }

    /// Close the record: every body's series and decisions.
    pub fn finish(self) -> PgWorld {
        PgWorld {
            founder: self.founder,
            bodies: self
                .bodies
                .into_iter()
                .map(|b| {
                    let decisions = std::mem::take(&mut *b.log.lock().expect("decision log"));
                    PgBody {
                        id: b.id,
                        start: b.start,
                        series: b.series,
                        died: b.died,
                        decisions,
                        drives: b.drives,
                        stage: b.stage,
                        born_to: b.born_to,
                    }
                })
                .collect(),
        }
    }
}
