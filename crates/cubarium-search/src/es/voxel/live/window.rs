//! **One world's window**: every living body of a lineage gets a variant at the start,
//! twins of a pair on neighbouring bodies; a newborn gets one when it appears, twin to its
//! nearest neighbour's; each body's score counts only its own time in this window.
//!
//! **The score is the body's net energy** (Wrysk 2026-10-01,
//! `design/handoffs/cotrain-stage0-2026-10-01.md` §1): `(E_end − E_start + E_given) /
//! body_ref`, with `E` the body's tissue, reserve and gestation escrow (organic units, the
//! units intake is counted in), `E_given` what it passed to offspring in the window, and
//! `E_end = 0` for a body that died in it — whatever killed it. Breeding is neither a cost
//! nor a loss; starving loses the body, not only an empty reserve. Intake and motor are
//! still read by the frozen driver's own [`BodyMeter`], for the readout.

use std::time::Instant;

use cubarium_voxel_fauna::Founder;
use cubarium_voxel_sim::Sim;
use rustc_hash::{FxHashMap, FxHashSet};

use super::curiosity::{Curiosity, Geometry, Memory};
use super::ecology::EcologyParams;
use super::readout::{self, Ctx, LineageStats, Pool, Scan};
use super::variants::VariantTable;
use super::world::FoundInfo;
use crate::es::voxel::score::{BodyMeter, credit_unseen_intake};

/// The window both training CLIs default to, simulated minutes.
pub const DEFAULT_WINDOW_MIN: f64 = 10.0;

/// Simulated ticks in one minute.
pub const TICKS_PER_MIN: u64 = 60 * cubarium_voxel_fauna::TICK_HZ as u64;

/// Moving means over this many body lengths per second, averaged over a body-minute.
pub const MOVING_BL_PER_S: f64 = 0.1;

/// One persistent training world.
pub struct LiveWorld {
    /// Its slot in the run.
    pub id: usize,
    pub sim: Sim,
    pub ecology: EcologyParams,
    pub info: FoundInfo,
    /// Windows this world has run since it was founded or replaced.
    pub windows: u32,
    /// Its bodies' familiarity, carried from window to window (training side only).
    pub memory: Memory,
}

/// One body's result for one window.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct BodyScore {
    pub world: usize,
    pub body_id: u64,
    /// Both energy potentials, regardless of which is ranked.
    pub delta_body_normalized: f64,
    pub delta_stomach_aware_normalized: f64,
    pub founder: Founder,
    pub candidate: usize,
    pub score: NetEnergy,
    /// Whether the body was born inside the window.
    pub newborn: bool,
}

/// One body's metered result for one window, whether a variant drove it or not (§11.7:
/// heuristic relatives are metered too). [`WindowResult::scores`] keeps only the bodies
/// with a variant, as before; this keeps every body that could have lived in the window.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct MeteredBody {
    pub body_id: u64,
    pub founder: Founder,
    /// Its variant, `None` for a body its lineage's driver (a heuristic, a centre) ran.
    pub candidate: Option<usize>,
    pub newborn: bool,
    /// The score of the kind the window ranks by (body or stomach-aware), curiosity
    /// included; a death is scored with `E_end = 0` as for every scored body.
    pub score: NetEnergy,
}

impl MeteredBody {
    /// Its own base score, the energy part only: `(E_end − E_start + E_given)/body_ref`,
    /// without the curiosity term. What a relative contributes to its kin (§10.3).
    pub fn base(&self) -> f64 {
        self.score.delta_normalized + self.score.given_normalized
    }
}

/// What one window of one world produced.
pub struct WindowResult {
    pub world: usize,
    pub scores: Vec<BodyScore>,
    /// Every body metered in the window, variant or not ([`MeteredBody`]).
    pub metered: Vec<MeteredBody>,
    /// The window's reward roster: fixed at its start, newborns joined at birth.
    pub roster: crate::es::voxel::kin::Roster,
    pub lineages: [LineageStats; Founder::COUNT],
    pub readout: Vec<(String, Pool, f64)>,
    pub ticks: u64,
    pub wall_s: f64,
    /// Bodies whose measured intake came out negative beyond rounding (clamped to zero).
    pub intake_anomalies: u64,
    /// The living bodies' familiarity, for the next window of this world.
    pub memory: Memory,
}

/// Where a body stands around the ring, metres, and its id.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub id: u64,
    pub x_m: f64,
}

/// A body's energy: tissue, reserve, fuel and the gestation escrow it is filling (still its own
/// until the birth), organic units.
pub fn body_energy(a: &cubarium_voxel_fauna::Animal) -> f64 {
    a.body + a.reserve + a.fuel + a.reproduction.escrow.map_or(0.0, |e| e.organic)
}

/// One body's score for one window and its parts, each over the lineage's
/// `body_reference`.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NetEnergy {
    /// `(E_end − E_start) / ref`, with `E_end = 0` for a death.
    pub delta_normalized: f64,
    /// `E_given / ref`.
    pub given_normalized: f64,
    /// Settled intake and paid motor over the window (readout only, not in the score).
    pub intake_normalized: f64,
    pub motor_loss_normalized: f64,
    /// Whether it died in the window.
    pub died: bool,
    /// Ground new to the body this window, over the cells a walk at cruise would enter
    /// ([`super::curiosity`]); earned before a death is kept.
    #[serde(default)]
    pub novelty: f64,
    /// The curiosity term, `weight × novelty` (0 without a curiosity file).
    #[serde(default)]
    pub curiosity: f64,
    /// What the ES ranks by: `delta + given + curiosity`.
    pub score: f64,
}

impl NetEnergy {
    /// The score from a window's measurements: `E` at the start and the end (the end of a
    /// death is read as zero, whatever is passed), what it gave, and the intake and motor
    /// it is reported beside.
    pub fn of(
        e_start: f64,
        e_end: f64,
        given: f64,
        died: bool,
        intake: f64,
        motor: f64,
        body_reference: f64,
    ) -> NetEnergy {
        assert!(e_start.is_finite() && e_end.is_finite() && given.is_finite());
        assert!(body_reference.is_finite() && body_reference > 0.0);
        let end = if died { 0.0 } else { e_end };
        let delta_normalized = (end - e_start) / body_reference;
        let given_normalized = given.max(0.0) / body_reference;
        NetEnergy {
            delta_normalized,
            given_normalized,
            intake_normalized: intake / body_reference,
            motor_loss_normalized: motor / body_reference,
            died,
            novelty: 0.0,
            curiosity: 0.0,
            score: delta_normalized + given_normalized,
        }
    }

    /// The score with the curiosity term: `delta + given + weight × novelty`. Weight 0 is
    /// the energy score unchanged.
    pub fn with_curiosity(self, novelty: f64, weight: f64) -> NetEnergy {
        let curiosity = weight * novelty;
        NetEnergy {
            novelty,
            curiosity,
            score: self.delta_normalized + self.given_normalized + curiosity,
            ..self
        }
    }
}

/// One body's energy over a window: [`body_energy`] and its `given` counter when it was
/// first seen and when it was last seen alive.
#[derive(Clone, Copy, Debug)]
pub struct EnergyMeter {
    e_start: f64,
    d_start: f64,
    d_last: f64,
    given_start: f64,
    e_last: f64,
    given_last: f64,
    died: bool,
}

impl EnergyMeter {
    pub fn new(a: &cubarium_voxel_fauna::Animal) -> EnergyMeter {
        let e = body_energy(a);
        EnergyMeter {
            e_start: e,
            d_start: 0.0,
            d_last: 0.0,
            given_start: a.reproduction.given,
            e_last: e,
            given_last: a.reproduction.given,
            died: false,
        }
    }

    pub fn new_with_stomach(a: &cubarium_voxel_fauna::Animal, p: &cubarium_voxel_fauna::FounderPhysiology) -> EnergyMeter {
        let d = super::targets::digestible_stomach(a, p);
        Self { d_start: d, d_last: d, ..Self::new(a) }
    }

    pub fn observe_with_stomach(&mut self, a: &cubarium_voxel_fauna::Animal, p: &cubarium_voxel_fauna::FounderPhysiology) {
        self.observe(a);
        self.d_last = super::targets::digestible_stomach(a, p);
    }

    pub fn stomach_score(&self, intake: f64, motor: f64, body_reference: f64) -> NetEnergy {
        NetEnergy::of(self.e_start + self.d_start, self.e_last + self.d_last,
            self.given_last - self.given_start, self.died, intake, motor, body_reference)
    }

    /// Fold in one post-step reading of the living body.
    pub fn observe(&mut self, a: &cubarium_voxel_fauna::Animal) {
        self.e_last = body_energy(a);
        self.given_last = a.reproduction.given;
    }

    /// It is gone.
    pub fn die(&mut self) {
        self.died = true;
    }

    pub fn died(&self) -> bool {
        self.died
    }

    /// The window's score, with the intake and motor it is reported beside.
    pub fn score(&self, intake: f64, motor: f64, body_reference: f64) -> NetEnergy {
        NetEnergy::of(
            self.e_start,
            self.e_last,
            self.given_last - self.given_start,
            self.died,
            intake,
            motor,
            body_reference,
        )
    }
}

/// **Twin placement.** Bodies sorted around the ring; each consecutive two share a pair,
/// the first `+`, the second `−`, pairs taken in turn from `first_pair` (wrapping at
/// `pairs`). So a pair's twins are neighbours wherever there are two bodies to pair, and
/// a lone last body takes the `+` of the next pair. Returns `(id, candidate)` with
/// candidate `2·pair + sign`.
pub fn place_twins(bodies: &[Placed], pairs: usize, first_pair: usize) -> Vec<(u64, usize)> {
    assert!(pairs > 0, "a window needs pairs");
    let mut sorted = bodies.to_vec();
    sorted.sort_by(|a, b| a.x_m.total_cmp(&b.x_m).then(a.id.cmp(&b.id)));
    sorted
        .iter()
        .enumerate()
        .map(|(k, b)| {
            let pair = (first_pair + k / 2) % pairs;
            (b.id, 2 * pair + k % 2)
        })
        .collect()
}

/// **A newborn's variant**: the pair of the nearest body of its lineage already assigned
/// in this world (ring distance), with the sign that pair has fewer bodies on here — the
/// opposite of the neighbour's on a tie — so twins stay neighbours and pairs stay
/// balanced. With nobody to be near, the least-used candidate.
pub fn newborn_candidate(
    x_m: f64,
    ring_m: f64,
    assigned: &[(f64, usize)],
    counts: &[u32],
) -> usize {
    let ring = |a: f64, b: f64| {
        let d = (a - b).rem_euclid(ring_m);
        d.min(ring_m - d)
    };
    let Some(&(_, near)) = assigned
        .iter()
        .min_by(|a, b| ring(a.0, x_m).total_cmp(&ring(b.0, x_m)))
    else {
        return (0..counts.len())
            .min_by_key(|&c| (counts[c], c))
            .unwrap_or(0);
    };
    let pair = near / 2;
    let (plus, minus) = (2 * pair, 2 * pair + 1);
    match counts[plus].cmp(&counts[minus]) {
        std::cmp::Ordering::Less => plus,
        std::cmp::Ordering::Greater => minus,
        std::cmp::Ordering::Equal => near ^ 1,
    }
}

/// One body followed through one window.
struct Tracked {
    id: u64,
    founder: Founder,
    candidate: Option<usize>,
    newborn: bool,
    meter: BodyMeter,
    energy: EnergyMeter,
    /// Ticks it could have lived here (assignment to window end) and did.
    possible: u64,
    alive_ticks: u64,
    alive: bool,
    seen: u64,
    x_m: f64,
    visited: FxHashSet<u32>,
    minute_bl: f64,
    minute_ticks: u64,
    /// Cells' worth of new ground it entered this window ([`Memory::observe`]).
    earned: f64,
}

/// One window in progress on one world: [`Window::begin`], [`Window::step`] once per
/// tick, [`Window::finish`]. [`run_window`] is the three in a row.
pub struct Window {
    /// Training-side experiment; false retains the current tissue/stores reward.
    pub stomach_aware_reward: bool,
    tables: Vec<Option<VariantTable>>,
    ticks: u64,
    elapsed: u64,
    started: Instant,
    voxel_m: f64,
    width: u32,
    depth: u32,
    ring_m: f64,
    cell_area: f64,
    phys: Vec<cubarium_voxel_fauna::FounderPhysiology>,
    deaths_start: [[u64; cubarium_voxel_fauna::Departure::COUNT]; Founder::COUNT],
    /// The ledger's assimilation per lineage at the window's start: a lineage's intake is
    /// the ledger's, not the meters' sum (the minimal gut digests before the controller
    /// stage, [`credit_unseen_intake`]).
    assimilated_start: [f64; Founder::COUNT],
    /// The ledger at the window's start: the balance measures' books.
    ledger_start: Box<cubarium_voxel_fauna::FaunaLedger>,
    recruitment_start: super::accounting::Recruitment,
    tracked: Vec<Tracked>,
    index: FxHashMap<u64, usize>,
    counts: Vec<Vec<u32>>,
    assigned: Vec<Vec<(f64, usize)>>,
    stats: [LineageStats; Founder::COUNT],
    newcomers: Vec<(u64, Founder, f64, BodyMeter, EnergyMeter)>,
    curiosity: Curiosity,
    geometry: Vec<Geometry>,
    memory: Memory,
    roster: crate::es::voxel::kin::Roster,
}

impl Window {
    /// Open a window of `ticks` on `sim`: every living body of a trained lineage gets
    /// its variant, twins as neighbours. `tables[f]` is lineage `f`'s variant table for
    /// this generation (`None`: that lineage is not trained and keeps what drives it);
    /// `first_pair[f]` is where this world's twin placement starts in it.
    pub fn begin(
        sim: &mut Sim,
        tables: &[Option<VariantTable>],
        first_pair: &[usize],
        ticks: u64,
    ) -> Window {
        Window::begin_with(sim, tables, first_pair, ticks, &Curiosity::default(), Memory::default())
    }

    /// [`Window::begin`] with each lineage's `curiosity` and the world's bodies'
    /// familiarity carried from its last window (`memory`; empty for a fresh world).
    /// [`WindowResult::memory`] hands it on.
    pub fn begin_with(
        sim: &mut Sim,
        tables: &[Option<VariantTable>],
        first_pair: &[usize],
        ticks: u64,
        curiosity: &Curiosity,
        mut memory: Memory,
    ) -> Window {
        assert_eq!(tables.len(), Founder::COUNT);
        let voxel_m = sim.world().config().voxel_m;
        let (width, depth) = (sim.world().config().width, sim.world().config().depth);
        let pairs: Vec<usize> = tables
            .iter()
            .map(|t| t.as_ref().map_or(0, |t| t.len() / 2))
            .collect();
        let mut w = Window {
            stomach_aware_reward: false,
            tables: tables.to_vec(),
            ticks,
            elapsed: 0,
            started: Instant::now(),
            voxel_m,
            width,
            depth,
            ring_m: f64::from(width) * voxel_m,
            cell_area: sim.world().config().cell_area(),
            phys: Founder::ALL
                .iter()
                .map(|f| *sim.fauna().config().founder(*f))
                .collect(),
            deaths_start: sim.fauna().view().ledger.deaths_by_founder_cause,
            assimilated_start: sim.fauna().view().ledger.assimilated_by_founder,
            ledger_start: Box::new(*sim.fauna().view().ledger),
            recruitment_start: super::accounting::Recruitment::of(sim.fauna()),
            tracked: Vec::new(),
            index: FxHashMap::default(),
            counts: pairs.iter().map(|&p| vec![0u32; 2 * p]).collect(),
            assigned: vec![Vec::new(); Founder::COUNT],
            stats: Default::default(),
            newcomers: Vec::new(),
            curiosity: curiosity.clone(),
            geometry: Founder::ALL
                .iter()
                .map(|f| Geometry::of(sim.fauna().config().founder(*f), curiosity.of(*f)))
                .collect(),
            memory: Memory::default(),
            roster: crate::es::voxel::kin::Roster::fixed(
                &sim.fauna()
                    .view()
                    .animals
                    .iter()
                    .filter(|a| a.founder.is_some())
                    .map(crate::es::voxel::kin::KinRecord::of)
                    .collect::<Vec<_>>(),
            ),
        };
        let bodies: Vec<(u64, Founder, f64, BodyMeter, EnergyMeter)> = sim
            .fauna()
            .view()
            .animals
            .iter()
            .filter_map(|a| {
                a.founder
                    .map(|f| (a.id, f, a.pose.x, BodyMeter::new(f, a), EnergyMeter::new_with_stomach(a, sim.fauna().config().founder(f))))
            })
            .collect();
        // Only the living are remembered; a body met for the first time starts empty.
        let living: FxHashSet<u64> = bodies.iter().map(|b| b.0).collect();
        memory.retain(|id| living.contains(&id));
        {
            let ring_m = w.ring_m;
            for a in sim.fauna().view().animals.iter() {
                if let Some(f) = a.founder {
                    memory.observe(
                        a.id,
                        &w.geometry[f.index()],
                        a.pose.x.rem_euclid(ring_m),
                        a.pose.z,
                    );
                }
            }
        }
        w.memory = memory;
        for f in Founder::ALL {
            let fi = f.index();
            let mine: Vec<Placed> = bodies
                .iter()
                .filter(|b| b.1 == f)
                .map(|b| Placed { id: b.0, x_m: b.2 })
                .collect();
            let placed: FxHashMap<u64, usize> = match &w.tables[fi] {
                Some(_) if !mine.is_empty() => place_twins(&mine, pairs[fi], first_pair[fi])
                    .into_iter()
                    .collect(),
                _ => FxHashMap::default(),
            };
            for b in bodies.iter().filter(|b| b.1 == f) {
                let candidate = placed.get(&b.0).copied();
                if let (Some(c), Some(t)) = (candidate, &w.tables[fi]) {
                    sim.fauna_mut().set_controller(b.0, t.controller(c));
                    w.counts[fi][c] += 1;
                    w.assigned[fi].push((b.2, c));
                }
                w.index.insert(b.0, w.tracked.len());
                w.tracked.push(Tracked {
                    id: b.0,
                    founder: f,
                    candidate,
                    newborn: false,
                    meter: b.3.clone(),
                    energy: b.4,
                    possible: ticks,
                    alive_ticks: 0,
                    alive: true,
                    seen: 0,
                    x_m: b.2,
                    visited: FxHashSet::default(),
                    minute_bl: 0.0,
                    minute_ticks: 0,
                    earned: 0.0,
                });
            }
        }
        w
    }

    /// Whether every tick of the window has run.
    pub fn done(&self) -> bool {
        self.elapsed >= self.ticks
    }

    /// Step `sim` one tick and follow every body; a newcomer gets its variant the tick it
    /// appears.
    pub fn step(&mut self, sim: &mut Sim) {
        let assimilated_before = sim.fauna().view().ledger.assimilated_by_founder;
        let mut seen = [0.0f64; Founder::COUNT];
        sim.step();
        self.elapsed += 1;
        self.memory.advance();
        let stamp = self.elapsed;
        self.newcomers.clear();
        {
            let fv = sim.fauna().view();
            for a in fv.animals.iter() {
                let Some(f) = a.founder else { continue };
                let Some(&k) = self.index.get(&a.id) else {
                    self.newcomers.push((
                        a.id,
                        f,
                        a.pose.x,
                        BodyMeter::new(f, a),
                        EnergyMeter::new_with_stomach(a, &self.phys[f.index()]),
                    ));
                    continue;
                };
                let b = &mut self.tracked[k];
                b.seen = stamp;
                let d = b.meter.observe(a);
                seen[f.index()] += d.intake;
                if super::balance::is_adult(a, fv.config) {
                    self.stats[f.index()].adult_ticks += 1;
                }
                b.energy.observe_with_stomach(a, &self.phys[f.index()]);
                let len = self.phys[f.index()].body_at(a.body).length_m;
                if len > 0.0 {
                    b.minute_bl += d.forward_m / len;
                }
                self.stats[f.index()].walked_m += d.forward_m;
                b.minute_ticks += 1;
                b.alive_ticks += 1;
                b.x_m = a.pose.x;
                b.earned += self.memory.observe(
                    a.id,
                    &self.geometry[f.index()],
                    a.pose.x.rem_euclid(self.ring_m),
                    a.pose.z,
                );
                if let Some((cx, cz)) = a.pose.column(self.voxel_m, self.depth) {
                    b.visited
                        .insert(cx.rem_euclid(i64::from(self.width)) as u32 * self.depth + cz);
                }
            }
        }
        // The sampled bodies' digestion the meters missed, per lineage, from the ledger.
        {
            let after = sim.fauna().view().ledger.assimilated_by_founder;
            for f in Founder::ALL {
                let fi = f.index();
                let residual = after[fi] - assimilated_before[fi] - seen[fi];
                credit_unseen_intake(
                    self.tracked
                        .iter_mut()
                        .filter(|b| b.founder == f && b.seen == stamp)
                        .map(|b| &mut b.meter),
                    residual,
                );
            }
        }
        for (id, &k) in &self.index {
            let b = &mut self.tracked[k];
            if b.alive && b.seen != stamp {
                // Gone: what it earned is kept, its memory is not.
                b.alive = false;
                b.energy.die();
                self.memory.forget(*id);
            }
        }
        for (id, f, x, meter, energy) in self.newcomers.drain(..) {
            let fi = f.index();

            let candidate = self.tables[fi].as_ref().map(|table| {
                let c = newborn_candidate(x, self.ring_m, &self.assigned[fi], &self.counts[fi]);
                sim.fauna_mut().set_controller(id, table.controller(c));
                c
            });
            if let Some(c) = candidate {
                self.counts[fi][c] += 1;
                self.assigned[fi].push((x, c));
            }
            // Its first sight is its spawn, not an entry: it earns nothing.
            if let Some(a) = sim.fauna().view().animals.iter().find(|a| a.id == id) {
                self.memory
                    .observe(id, &self.geometry[fi], x.rem_euclid(self.ring_m), a.pose.z);
                // A newborn joins its mother's roster at birth (§11.7).
                self.roster.join(crate::es::voxel::kin::KinRecord::of(a));
            }
            self.index.insert(id, self.tracked.len());
            self.tracked.push(Tracked {
                id,
                founder: f,
                candidate,
                newborn: true,
                meter,
                energy,
                possible: self.ticks.saturating_sub(stamp),
                alive_ticks: 0,
                alive: true,
                seen: stamp,
                x_m: x,
                visited: FxHashSet::default(),
                minute_bl: 0.0,
                minute_ticks: 0,
                earned: 0.0,
            });
        }
        if stamp.is_multiple_of(TICKS_PER_MIN) {
            // How clumped each lineage is: its bodies' mean nearest-neighbour distance.
            let ring_m = self.ring_m;
            for f in Founder::ALL {
                let points: Vec<(f64, f64)> = sim
                    .fauna()
                    .view()
                    .animals
                    .iter()
                    .filter(|a| a.founder == Some(f))
                    .map(|a| (a.pose.x, a.pose.z))
                    .collect();
                if let Some(nn) = mean_nearest_neighbour(&points, ring_m) {
                    let s = &mut self.stats[f.index()];
                    s.nn_sum_m += nn;
                    s.nn_samples += 1;
                }
            }
        }
        if stamp.is_multiple_of(TICKS_PER_MIN) || stamp == self.ticks {
            for b in self.tracked.iter_mut() {
                let s = &mut self.stats[b.founder.index()];
                if b.minute_ticks * 2 >= TICKS_PER_MIN {
                    s.body_minutes += 1;
                    let pace = b.minute_bl / (b.minute_ticks as f64 * cubarium_voxel::DT);
                    if pace > MOVING_BL_PER_S {
                        s.moving_minutes += 1;
                    }
                }
                s.walked_bl += b.minute_bl;
                b.minute_bl = 0.0;
                b.minute_ticks = 0;
            }
        }
    }

    /// Close the window: every scored body's result, the lineage counts and the readout.
    pub fn finish(mut self, sim: &Sim, world_id: usize) -> WindowResult {
        let mut scores = Vec::new();
        let mut metered = Vec::new();
        let mut anomalies = 0u64;
        let events = super::accounting::Recruitment::of(sim.fauna()).since(&self.recruitment_start);
        let deaths_end = sim.fauna().view().ledger.deaths_by_founder_cause;
        for f in Founder::ALL {
            let fi = f.index();
            self.stats[fi].births = events.births[fi];
            self.stats[fi].live_births = events.live_births[fi];
            self.stats[fi].hatchings = events.hatchings[fi];
            self.stats[fi].eggs_laid = events.eggs_laid[fi];
            self.stats[fi].maturations = events.maturations[fi];
            for (d, (end, start)) in deaths_end[fi].iter().zip(self.deaths_start[fi]).enumerate() {
                self.stats[fi].deaths[d] = end - start;
            }
        }
        let assimilated_end = sim.fauna().view().ledger.assimilated_by_founder;
        let fv = sim.fauna().view();
        let days = super::balance::days(
            self.elapsed as f64 / f64::from(cubarium_voxel_fauna::TICK_HZ),
            fv.config,
        );
        for f in Founder::ALL {
            let fi = f.index();
            self.stats[fi].intake = assimilated_end[fi] - self.assimilated_start[fi];
            // The window's balance measures (the search rebase's addendum).
            let books = super::balance::Books::of(fv.ledger, f)
                .since(&super::balance::Books::of(&self.ledger_start, f));
            let adult_days = super::balance::days(
                self.stats[fi].adult_ticks as f64 / f64::from(cubarium_voxel_fauna::TICK_HZ),
                fv.config,
            );
            let ctx = super::balance::context(fv.animals, fv.config, f, adult_days, days);
            self.stats[fi].books = books;
            self.stats[fi].balance = super::balance::measures(&books, &ctx);
        }
        for b in &self.tracked {
            let s = &mut self.stats[b.founder.index()];
            s.body_ticks += b.alive_ticks;
            s.unique_m2 += b.visited.len() as f64 * self.cell_area;
            s.walked_bl += b.minute_bl;
            let fi = b.founder.index();
            let novelty = self.geometry[fi].novelty(b.earned, self.ticks);
            let weight = self.curiosity.of(b.founder).weight;
            if b.possible > 0 {
                s.novelty_bodies += 1;
                s.novelty_sum += novelty;
                s.curiosity_sum += weight * novelty;
            }
            if b.possible == 0 {
                continue;
            }
            let body_score = b
                .energy
                .score(
                    b.meter.intake.max(0.0),
                    b.meter.motor.max(0.0),
                    b.founder.manifest().body_reference,
                )
                .with_curiosity(novelty, weight);
            let stomach_score = b.energy.stomach_score(b.meter.intake.max(0.0), b.meter.motor.max(0.0),
                b.founder.manifest().body_reference).with_curiosity(novelty, weight);
            let score = if self.stomach_aware_reward { stomach_score } else { body_score };
            metered.push(MeteredBody {
                body_id: b.id,
                founder: b.founder,
                candidate: b.candidate,
                newborn: b.newborn,
                score,
            });
            let Some(c) = b.candidate else {
                continue;
            };
            if b.meter.intake < -1e-9 {
                anomalies += 1;
            }
            s.scored += 1;
            s.mean_score += score.score;
            scores.push(BodyScore {
                world: world_id,
                body_id: b.id,
                delta_body_normalized: body_score.delta_normalized,
                delta_stomach_aware_normalized: stomach_score.delta_normalized,
                founder: b.founder,
                candidate: c,
                score,
                newborn: b.newborn,
            });
        }
        for s in self.stats.iter_mut() {
            if s.scored > 0 {
                s.mean_score /= s.scored as f64;
            }
        }
        let wall_s = self.started.elapsed().as_secs_f64();
        let scan = Scan::of(sim);
        let readout = readout::read(&Ctx {
            sim,
            lineages: &self.stats,
            scan: &scan,
            hours: self.elapsed.max(1) as f64 / (TICKS_PER_MIN as f64 * 60.0),
            tick_ms: wall_s * 1e3 / self.elapsed.max(1) as f64,
        });
        WindowResult {
            world: world_id,
            scores,
            metered,
            roster: self.roster,
            lineages: self.stats,
            readout,
            ticks: self.elapsed,
            wall_s,
            intake_anomalies: anomalies,
            memory: self.memory,
        }
    }
}

/// Run one whole window of `ticks` on `world` ([`Window`] begin, step, finish).
pub fn run_window(
    world: &mut LiveWorld,
    tables: &[Option<VariantTable>],
    first_pair: &[usize],
    ticks: u64,
    curiosity: &Curiosity,
) -> WindowResult {
    run_window_with_reward(world, tables, first_pair, ticks, curiosity, false)
}

pub fn run_window_with_reward(
    world: &mut LiveWorld, tables: &[Option<VariantTable>], first_pair: &[usize],
    ticks: u64, curiosity: &Curiosity, stomach_aware_reward: bool,
) -> WindowResult {
    let memory = std::mem::take(&mut world.memory);
    let mut w = Window::begin_with(&mut world.sim, tables, first_pair, ticks, curiosity, memory);
    w.stomach_aware_reward = stomach_aware_reward;
    while !w.done() {
        w.step(&mut world.sim);
    }
    world.windows += 1;
    let mut r = w.finish(&world.sim, world.id);
    world.memory = std::mem::take(&mut r.memory);
    r
}

/// Mean distance from each point to its nearest other point, metres, with `x` wrapping
/// on a ring of `ring_m` (the strip's `z` does not wrap). `None` with fewer than two.
pub fn mean_nearest_neighbour(points: &[(f64, f64)], ring_m: f64) -> Option<f64> {
    if points.len() < 2 {
        return None;
    }
    let sum: f64 = points
        .iter()
        .enumerate()
        .map(|(i, &(x, z))| {
            points
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, &(x2, z2))| {
                    let dx = (x - x2).rem_euclid(ring_m);
                    let dx = dx.min(ring_m - dx);
                    (dx * dx + (z - z2) * (z - z2)).sqrt()
                })
                .fold(f64::INFINITY, f64::min)
        })
        .sum();
    Some(sum / points.len() as f64)
}

/// How many bodies of each lineage `sim` holds now.
pub fn population(sim: &Sim) -> [usize; Founder::COUNT] {
    let mut n = [0usize; Founder::COUNT];
    for a in sim.fauna().view().animals.iter() {
        if let Some(f) = a.founder {
            n[f.index()] += 1;
        }
    }
    n
}

#[cfg(test)]
mod energy_accounting_tests {
    use super::*;

    #[test]
    fn digestion_transfers_potential_and_death_clears_both_stocks() {
        let mut meter = EnergyMeter {
            e_start: 10.0, e_last: 13.0, d_start: 3.0, d_last: 0.0,
            given_start: 1.0, given_last: 3.0, died: false,
        };
        let e = meter.score(3.0, 0.0, 2.0);
        let star = meter.stomach_score(3.0, 0.0, 2.0);
        assert_eq!(e.delta_normalized, 1.5);
        assert_eq!(star.delta_normalized, 0.0, "digestion moves a stock, not new intake");
        assert_eq!(e.given_normalized, star.given_normalized);
        meter.e_last = 10.0;
        meter.d_last = 6.0;
        assert_eq!(meter.stomach_score(0.0, 0.0, 2.0).delta_normalized, 1.5, "late food is in this window");
        meter.die();
        assert_eq!(meter.score(0.0, 0.0, 2.0).delta_normalized, -5.0);
        assert_eq!(meter.stomach_score(0.0, 0.0, 2.0).delta_normalized, -6.5);
        assert_eq!(meter.stomach_score(0.0, 0.0, 2.0).given_normalized, 1.0);
    }
}
