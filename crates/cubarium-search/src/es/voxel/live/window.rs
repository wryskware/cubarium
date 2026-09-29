//! **One world's window**: every living body of a lineage gets a variant at the start,
//! twins of a pair on neighbouring bodies; a newborn gets one when it appears, twin to its
//! nearest neighbour's; each body's score counts only its own time in this window.
//!
//! The score keeps the frozen trainer's form so the numbers compare
//! ([`crate::es::voxel::score`]): `(intake − motor)/body_ref + 0.25·survived`, with
//! intake and motor read off the body's own feedback accumulator exactly as the frozen
//! driver reads them, and `survived` the share of the ticks the body could have lived in
//! this window (from its assignment to the window's end) that it did.

use std::time::Instant;

use cubarium_voxel_fauna::{Animal, Founder};
use cubarium_voxel_sim::Sim;
use rustc_hash::{FxHashMap, FxHashSet};

use super::ecology::EcologyParams;
use super::readout::{self, Ctx, LineageStats, Pool, Scan};
use super::variants::VariantTable;
use super::world::FoundInfo;
use crate::es::voxel::score::ScoreComponents;

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
}

/// One body's result for one window.
#[derive(Clone, Copy, Debug)]
pub struct BodyScore {
    pub founder: Founder,
    pub candidate: usize,
    pub score: ScoreComponents,
    /// Whether the body was born inside the window.
    pub newborn: bool,
}

/// What one window of one world produced.
pub struct WindowResult {
    pub world: usize,
    pub scores: Vec<BodyScore>,
    pub lineages: [LineageStats; Founder::COUNT],
    pub readout: Vec<(String, Pool, f64)>,
    pub ticks: u64,
    pub wall_s: f64,
    /// Bodies whose measured intake came out negative beyond rounding (clamped to zero).
    pub intake_anomalies: u64,
}

/// Where a body stands around the ring, metres, and its id.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub id: u64,
    pub x_m: f64,
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
    founder: Founder,
    candidate: Option<usize>,
    newborn: bool,
    cadence: u64,
    /// The feedback accumulator after the previous tick: intake, motor, delivered forward.
    prev: [f64; 3],
    intake: f64,
    motor: f64,
    /// Ticks it could have lived here (assignment to window end) and did.
    possible: u64,
    alive_ticks: u64,
    alive: bool,
    seen: u64,
    x_m: f64,
    visited: FxHashSet<u32>,
    minute_bl: f64,
    minute_ticks: u64,
}

fn feedback(a: &Animal) -> [f64; 3] {
    let fb = a.founder_state.feedback;
    [fb.intake, fb.motor_respiration, fb.delivered_forward]
}

/// One window in progress on one world: [`Window::begin`], [`Window::step`] once per
/// tick, [`Window::finish`]. [`run_window`] is the three in a row.
pub struct Window {
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
    cadence: Vec<u64>,
    deaths_start: [[u64; cubarium_voxel_fauna::Departure::COUNT]; Founder::COUNT],
    tracked: Vec<Tracked>,
    index: FxHashMap<u64, usize>,
    counts: Vec<Vec<u32>>,
    assigned: Vec<Vec<(f64, usize)>>,
    stats: [LineageStats; Founder::COUNT],
    newcomers: Vec<(u64, Founder, f64, [f64; 3])>,
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
        assert_eq!(tables.len(), Founder::COUNT);
        let voxel_m = sim.world().config().voxel_m;
        let (width, depth) = (sim.world().config().width, sim.world().config().depth);
        let pairs: Vec<usize> = tables
            .iter()
            .map(|t| t.as_ref().map_or(0, |t| t.len() / 2))
            .collect();
        let mut w = Window {
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
            cadence: Founder::ALL
                .iter()
                .map(|f| f.manifest().cadence_ticks().max(1))
                .collect(),
            deaths_start: sim.fauna().view().ledger.deaths_by_founder_cause,
            tracked: Vec::new(),
            index: FxHashMap::default(),
            counts: pairs.iter().map(|&p| vec![0u32; 2 * p]).collect(),
            assigned: vec![Vec::new(); Founder::COUNT],
            stats: Default::default(),
            newcomers: Vec::new(),
        };
        let bodies: Vec<(u64, Founder, f64, [f64; 3])> = sim
            .fauna()
            .view()
            .animals
            .iter()
            .filter_map(|a| a.founder.map(|f| (a.id, f, a.pose.x, feedback(a))))
            .collect();
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
                    founder: f,
                    candidate,
                    newborn: false,
                    cadence: w.cadence[fi],
                    prev: b.3,
                    intake: 0.0,
                    motor: 0.0,
                    possible: ticks,
                    alive_ticks: 0,
                    alive: true,
                    seen: 0,
                    x_m: b.2,
                    visited: FxHashSet::default(),
                    minute_bl: 0.0,
                    minute_ticks: 0,
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
        sim.step();
        self.elapsed += 1;
        let stamp = self.elapsed;
        self.newcomers.clear();
        {
            let fv = sim.fauna().view();
            for a in fv.animals.iter() {
                let Some(f) = a.founder else { continue };
                let Some(&k) = self.index.get(&a.id) else {
                    self.newcomers.push((a.id, f, a.pose.x, feedback(a)));
                    continue;
                };
                let b = &mut self.tracked[k];
                b.seen = stamp;
                let now = feedback(a);
                let sampled = a.age_ticks.is_multiple_of(b.cadence);
                let d: [f64; 3] =
                    std::array::from_fn(|i| if sampled { now[i] } else { now[i] - b.prev[i] });
                b.prev = now;
                b.intake += d[0];
                b.motor += d[1];
                let len = self.phys[f.index()].body_at(a.body).length_m;
                if len > 0.0 {
                    b.minute_bl += d[2] / len;
                }
                {
                    let s = &mut self.stats[f.index()];
                    s.walked_m += d[2];
                }
                b.minute_ticks += 1;
                b.alive_ticks += 1;
                b.x_m = a.pose.x;
                if let Some((cx, cz)) = a.pose.column(self.voxel_m, self.depth) {
                    b.visited
                        .insert(cx.rem_euclid(i64::from(self.width)) as u32 * self.depth + cz);
                }
            }
        }
        for b in self
            .tracked
            .iter_mut()
            .filter(|b| b.alive && b.seen != stamp)
        {
            b.alive = false;
        }
        for k in 0..self.newcomers.len() {
            let (id, f, x, fb) = self.newcomers[k];
            let fi = f.index();
            self.stats[fi].births += 1;
            let candidate = self.tables[fi].as_ref().map(|table| {
                let c = newborn_candidate(x, self.ring_m, &self.assigned[fi], &self.counts[fi]);
                sim.fauna_mut().set_controller(id, table.controller(c));
                c
            });
            if let Some(c) = candidate {
                self.counts[fi][c] += 1;
                self.assigned[fi].push((x, c));
            }
            self.index.insert(id, self.tracked.len());
            self.tracked.push(Tracked {
                founder: f,
                candidate,
                newborn: true,
                cadence: self.cadence[fi],
                prev: fb,
                intake: 0.0,
                motor: 0.0,
                possible: self.ticks.saturating_sub(stamp),
                alive_ticks: 0,
                alive: true,
                seen: stamp,
                x_m: x,
                visited: FxHashSet::default(),
                minute_bl: 0.0,
                minute_ticks: 0,
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
        let mut anomalies = 0u64;
        let deaths_end = sim.fauna().view().ledger.deaths_by_founder_cause;
        for f in Founder::ALL {
            let fi = f.index();
            for (d, (end, start)) in deaths_end[fi].iter().zip(self.deaths_start[fi]).enumerate() {
                self.stats[fi].deaths[d] = end - start;
            }
        }
        for b in &self.tracked {
            let s = &mut self.stats[b.founder.index()];
            s.body_ticks += b.alive_ticks;
            s.unique_m2 += b.visited.len() as f64 * self.cell_area;
            s.intake += b.intake;
            s.walked_bl += b.minute_bl;
            let (Some(c), true) = (b.candidate, b.possible > 0) else {
                continue;
            };
            if b.intake < -1e-9 {
                anomalies += 1;
            }
            let survived = (b.alive_ticks as f64 / b.possible as f64).clamp(0.0, 1.0);
            let score = ScoreComponents::of(
                b.intake.max(0.0),
                b.motor.max(0.0),
                survived,
                b.founder.manifest().body_reference,
            );
            s.scored += 1;
            s.mean_score += score.score;
            scores.push(BodyScore {
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
            lineages: self.stats,
            readout,
            ticks: self.elapsed,
            wall_s,
            intake_anomalies: anomalies,
        }
    }
}

/// Run one whole window of `ticks` on `world` ([`Window`] begin, step, finish).
pub fn run_window(
    world: &mut LiveWorld,
    tables: &[Option<VariantTable>],
    first_pair: &[usize],
    ticks: u64,
) -> WindowResult {
    let mut w = Window::begin(&mut world.sim, tables, first_pair, ticks);
    while !w.done() {
        w.step(&mut world.sim);
    }
    world.windows += 1;
    w.finish(&world.sim, world.id)
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
