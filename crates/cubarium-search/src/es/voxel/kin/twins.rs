//! **All-sign twins** (LIVE-TWINS; `design/handoffs/live-twins-2026-10-13.md`, the
//! comparison switch of §11.7 and topic 10): a lineage on [`Comparison::Twins`] is scored
//! on matched copies of its own live worlds instead of on the live window.
//!
//! - At each world's window boundary the world is captured ([`TwinStart::capture`]): its
//!   three layers' bytes, every trained lineage's current centre, the bodies' familiarity
//!   and each lineage's bodies at that moment.
//! - A **twin** is two copies of one start, every body of the lineage on `+ε_p` in one and
//!   on `−ε_p` in the other, newborns included ([`super::matched::run_assigned_with`]);
//!   every other body runs its lineage's centre or heuristic, as in the live world. A copy's
//!   score is the lineage's summed kin-weighted score over the bodies put on the variant at
//!   the start ([`Comparison::Twins`] says why), its **measurement** `d = score(+) −
//!   score(−)`.
//! - [`layout`] spreads `pairs × starts` measurements over the boundary starts; a pair's
//!   `d̄` is the mean over its measurements. The update ranks the twin differences, not the
//!   copies' raw scores: pair `p` enters [`super::ascend_pairs`] as `(d̄/2, −d̄/2)`, so a
//!   start's shared level (which world, how fed its bodies were) never enters the ranks,
//!   only what the sign changed (as `per_body` z-scores within a world before pooling).
//!   A measurement on a start with none of the lineage's bodies is missing, never a zero;
//!   a pair with no measurement is left out ([`PairScores::missing`]).
//! - **Null twins** ([`TwinKnobs::nulls`]): both copies on candidate 0; their gap is the
//!   determinism floor and must be 0 (a readout, never in the update).
//!
//! **Determinism.** Every copy runs its sim on one thread with the chained schedule from
//! the same bytes; copies run in parallel ([`crate::es::voxel::live::trainer::parallel`])
//! and every result is gathered in item order, so the scores and the update do not depend
//! on the thread count or the schedule. With a null table (σ 0) the two copies of a twin
//! are bit-identical and the update is zero.

use cubarium_voxel_fauna::Founder;
use cubarium_voxel_sim::Sim;
use serde::Serialize;

use super::compare::PairScores;
use super::config::{Comparison, KinConfig, TwinKnobs};
use super::matched::{AssignOptions, Scene, SceneCopy, SimStart, run_assigned_with};
use crate::es::voxel::EpisodeDriver;
use crate::es::voxel::live::curiosity::{Curiosity, Memory};
use crate::es::voxel::live::variants::VariantTable;

/// One world's start at a generation boundary.
pub struct TwinStart {
    /// The world's slot in the run.
    pub world: usize,
    pub start: SimStart,
    /// Every trained lineage's centre at the boundary, put back on every copy.
    pub drivers: Vec<(Founder, EpisodeDriver)>,
    pub water_every: u32,
    /// The bodies' familiarity at the boundary (the live window's, for curiosity).
    pub memory: Memory,
    /// Each lineage's living bodies at the boundary, by id, in [`Founder::ALL`] order.
    pub bodies: Vec<Vec<u64>>,
}

impl TwinStart {
    /// Capture `sim` as it stands at a boundary (nothing in it changes). `drivers` are the
    /// trained lineages' centres, the policy every copy restores.
    pub fn capture(world: usize, sim: &Sim, drivers: &[(Founder, EpisodeDriver)], water_every: u32, memory: &Memory) -> TwinStart {
        let policy: Vec<Founder> = drivers.iter().map(|d| d.0).collect();
        let mut bodies = vec![Vec::new(); Founder::COUNT];
        for a in sim.fauna().view().animals.iter() {
            if let Some(f) = a.founder {
                bodies[f.index()].push(a.id);
            }
        }
        for b in &mut bodies {
            b.sort_unstable();
        }
        TwinStart {
            world,
            start: SimStart::capture(sim, &policy),
            drivers: drivers.to_vec(),
            water_every,
            memory: memory.clone(),
            bodies,
        }
    }

    /// This start as a scene of `ticks`.
    pub fn scene(&self, ticks: u64) -> TwinScene<'_> {
        TwinScene { start: self, ticks }
    }
}

/// A start run for one twin window: the saved layers, every centre put back.
pub struct TwinScene<'a> {
    pub start: &'a TwinStart,
    pub ticks: u64,
}

/// One copy of a [`TwinScene`].
pub struct TwinCopy {
    sim: Sim,
    left: u64,
}

impl SceneCopy for TwinCopy {
    fn sim(&self) -> &Sim {
        &self.sim
    }
    fn sim_mut(&mut self) -> &mut Sim {
        &mut self.sim
    }
    fn step(&mut self) -> bool {
        if self.left == 0 {
            return false;
        }
        self.sim.step();
        self.left -= 1;
        true
    }
}

impl Scene for TwinScene<'_> {
    type Copy = TwinCopy;
    fn open(&self) -> Result<TwinCopy, String> {
        let mut sim = self.start.start.open(1, self.start.water_every)?;
        crate::es::voxel::live::install_centres(sim.fauna_mut(), &self.start.drivers);
        Ok(TwinCopy { sim, left: self.ticks })
    }
    fn ticks(&self) -> u64 {
        self.ticks
    }
}

/// The twin window in ticks.
pub fn window_ticks(knobs: &TwinKnobs) -> u64 {
    (knobs.window_s * f64::from(cubarium_voxel_fauna::TICK_HZ)).round().max(1.0) as u64
}

/// One twin measurement: pair `pair` on start `start` (an index into the boundary starts).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Measurement {
    pub pair: usize,
    pub start: usize,
}

/// **The layout**: `pairs × starts` measurements over `worlds` boundary starts. Pair `p`'s
/// `k`-th measurement is on start `(p·starts + k) mod worlds`, so a pair's starts are
/// distinct worlds and every start carries an equal share (±1). Refused when a pair asks
/// for more starts than there are worlds.
pub fn layout(knobs: &TwinKnobs, worlds: usize) -> Result<Vec<Measurement>, String> {
    if knobs.pairs == 0 || knobs.starts == 0 {
        return Err("twins: pairs and starts must be positive".into());
    }
    if worlds == 0 || knobs.starts > worlds {
        return Err(format!(
            "twins: {} starts per pair need as many worlds; the run has {worlds}",
            knobs.starts
        ));
    }
    Ok((0..knobs.pairs * knobs.starts)
        .map(|j| Measurement { pair: j / knobs.starts, start: j % worlds })
        .collect())
}

/// One copy's result.
#[derive(Clone, Debug)]
pub struct CopyResult {
    pub ticks: u64,
    /// The lineage's summed kin-weighted score over the bodies put on the variant at the
    /// start; `None` when the start held none of the lineage.
    pub score: Option<f64>,
    /// Bodies put on the variant at the start, and newborns the copy metered.
    pub bodies: usize,
    pub newborns: usize,
    /// Sums over every body of the lineage the copy metered: own, relatives', combined.
    pub own: f64,
    pub relatives: f64,
    pub combined: f64,
    pub metered: usize,
    /// Every body's kin-weighted score, by id (the null twins' gap).
    pub by_body: Vec<(u64, f64)>,
    /// A hash of every animal's pose after each tick (where twins first differ).
    pub trace: Vec<u64>,
    pub wall_s: f64,
}

fn mix(h: u64, x: u64) -> u64 {
    (h ^ x).wrapping_mul(0x0100_0000_01b3).rotate_left(23)
}

fn world_hash(sim: &Sim) -> u64 {
    let mut h = 0x9e37_79b9_7f4a_7c15u64;
    for a in sim.fauna().view().animals.iter() {
        h = mix(h, a.id);
        for v in [a.pose.x, a.pose.z, a.pose.heading_rad, a.pose.y_m] {
            h = mix(h, v.to_bits());
        }
    }
    h
}

/// What the copies are metered by beyond the kin knobs: the live window's curiosity and
/// reward switch.
#[derive(Clone, Default)]
pub struct Metering {
    pub kin: KinConfig,
    pub curiosity: Curiosity,
    pub stomach_aware_reward: bool,
}

/// Run one copy: every body of `lineage` at the start, and its newborns, on `candidate`.
pub fn run_copy(
    start: &TwinStart,
    ticks: u64,
    lineage: Founder,
    table: &VariantTable,
    candidate: usize,
    metering: &Metering,
) -> Result<CopyResult, String> {
    let t0 = std::time::Instant::now();
    let ids = &start.bodies[lineage.index()];
    let assign: Vec<(u64, usize)> = ids.iter().map(|&id| (id, candidate)).collect();
    let options = AssignOptions {
        curiosity: metering.curiosity.clone(),
        memory: start.memory.clone(),
        stomach_aware_reward: metering.stomach_aware_reward,
        newborns: Some((lineage, candidate)),
    };
    let mut trace = Vec::with_capacity(ticks as usize);
    let o = run_assigned_with(&start.scene(ticks), &assign, table, &metering.kin, options, &mut |sim| {
        trace.push(world_hash(sim))
    })?;
    let mut r = CopyResult {
        ticks: o.ticks,
        score: None,
        bodies: ids.len(),
        newborns: 0,
        own: 0.0,
        relatives: 0.0,
        combined: 0.0,
        metered: 0,
        by_body: o.scores.iter().map(|(id, k)| (*id, k.combined)).collect(),
        trace,
        wall_s: 0.0,
    };
    for m in o.metered.iter().filter(|m| m.founder == lineage) {
        let Some(k) = o.scores.get(&m.body_id) else { continue };
        r.own += k.own;
        r.relatives += k.relatives;
        r.combined += k.combined;
        r.metered += 1;
        r.newborns += usize::from(m.newborn);
    }
    if !ids.is_empty() {
        r.score = Some(r.combined / ids.len() as f64);
    }
    r.wall_s = t0.elapsed().as_secs_f64();
    Ok(r)
}

/// One lineage's twins for one generation: its table, its layout.
pub struct TwinJob<'a> {
    pub lineage: Founder,
    pub table: &'a VariantTable,
    pub knobs: TwinKnobs,
}

/// One copy to run.
#[derive(Clone, Copy, Debug)]
struct Item {
    job: usize,
    start: usize,
    candidate: usize,
}

/// One lineage's twins, measured.
#[derive(Clone, Debug)]
pub struct TwinsResult {
    pub lineage: Founder,
    pub scores: PairScores,
    pub readout: TwinsReadout,
    /// Variant body-windows (bodies at the starts over every twin copy) and newborns.
    pub bodies: usize,
    pub newborns: usize,
    /// Means over every body of the lineage the twin copies metered.
    pub mean_own: f64,
    pub mean_relatives: f64,
    pub mean_combined: f64,
}

/// One lineage's twins readout, one per generation (`twins` in its [`LineageUpdate`]).
///
/// [`LineageUpdate`]: crate::es::voxel::live::trainer::LineageUpdate
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct TwinsReadout {
    pub window_ticks: u64,
    pub pairs: usize,
    pub starts_per_pair: usize,
    pub measurements: usize,
    /// Measurements on a start with none of the lineage (left out, never zero).
    pub measurements_missing: usize,
    pub pairs_scored: usize,
    /// Over the scored measurements: mean, mean |d|, sd of `d`, and the share exactly 0.
    pub mean_d: f64,
    pub mean_abs_d: f64,
    pub sd_d: f64,
    pub share_d_zero: f64,
    /// Share of twins whose worlds differ at some tick, and the median second they first
    /// differ (over those that do).
    pub share_diverged: f64,
    pub median_first_diff_s: Option<f64>,
    /// Null twins run, how many were bit-identical (every tick's world and every body's
    /// score), and the largest score gap over their bodies.
    pub nulls: usize,
    pub nulls_identical: usize,
    pub null_max_gap: f64,
    /// Mean raw copy score on `+` and on `−` (readout; the update ranks the differences).
    pub mean_plus: f64,
    pub mean_minus: f64,
    pub copies: usize,
    /// Sum of the copies' own wall time, thread-seconds.
    pub copy_thread_s: f64,
}

fn first_diff(a: &[u64], b: &[u64]) -> Option<usize> {
    a.iter().zip(b).position(|(x, y)| x != y).or_else(|| (a.len() != b.len()).then(|| a.len().min(b.len())))
}

fn identical(a: &CopyResult, b: &CopyResult) -> bool {
    a.trace == b.trace
        && a.by_body.len() == b.by_body.len()
        && a.by_body.iter().zip(&b.by_body).all(|(x, y)| x.0 == y.0 && x.1.to_bits() == y.1.to_bits())
}

/// **Measure every lineage's twins** over the boundary `starts` on `threads` workers: each
/// job's measurements ([`layout`]) and nulls, every copy one work item, results in item
/// order. The update's scores are each pair's `(d̄/2, −d̄/2)`.
pub fn measure(jobs: &[TwinJob<'_>], starts: &[TwinStart], metering: &Metering, threads: usize) -> Result<Vec<TwinsResult>, String> {
    let mut items = Vec::new();
    let mut layouts = Vec::new();
    for (j, job) in jobs.iter().enumerate() {
        if metering.kin.of(job.lineage).comparison != Comparison::Twins {
            return Err(format!("{}: twins measured for a lineage not on the twins comparison", job.lineage.name()));
        }
        if job.table.len() < 2 * job.knobs.pairs {
            return Err(format!("{}: a table of {} for {} pairs", job.lineage.name(), job.table.len(), job.knobs.pairs));
        }
        let plan = layout(&job.knobs, starts.len())?;
        for m in &plan {
            items.push(Item { job: j, start: m.start, candidate: 2 * m.pair });
            items.push(Item { job: j, start: m.start, candidate: 2 * m.pair + 1 });
        }
        for n in 0..job.knobs.nulls {
            for _ in 0..2 {
                items.push(Item { job: j, start: n % starts.len(), candidate: 0 });
            }
        }
        layouts.push(plan);
    }
    let copies: Vec<Result<CopyResult, String>> = crate::es::voxel::live::trainer::parallel(items, threads, |it| {
        let job = &jobs[it.job];
        run_copy(&starts[it.start], window_ticks(&job.knobs), job.lineage, job.table, it.candidate, metering)
    });
    let mut copies = copies.into_iter();
    let mut out = Vec::new();
    for (job, plan) in jobs.iter().zip(layouts) {
        let ticks = window_ticks(&job.knobs);
        let secs = |t: usize| (t + 1) as f64 / f64::from(cubarium_voxel_fauna::TICK_HZ);
        let mut r = TwinsResult {
            lineage: job.lineage,
            scores: PairScores::default(),
            readout: TwinsReadout {
                window_ticks: ticks,
                pairs: job.knobs.pairs,
                starts_per_pair: job.knobs.starts,
                measurements: plan.len(),
                nulls: job.knobs.nulls,
                ..Default::default()
            },
            bodies: 0,
            newborns: 0,
            mean_own: 0.0,
            mean_relatives: 0.0,
            mean_combined: 0.0,
        };
        let mut sum = vec![(0.0, 0usize); job.knobs.pairs];
        let (mut ds, mut first, mut diverged, mut twins) = (Vec::new(), Vec::new(), 0usize, 0usize);
        let (mut own, mut rel, mut comb, mut metered) = (0.0, 0.0, 0.0, 0usize);
        let (mut plus_sum, mut minus_sum) = (0.0, 0.0);
        for m in &plan {
            let (a, b) = (copies.next().expect("+ copy")?, copies.next().expect("- copy")?);
            r.readout.copies += 2;
            r.readout.copy_thread_s += a.wall_s + b.wall_s;
            twins += 1;
            if let Some(i) = first_diff(&a.trace, &b.trace) {
                diverged += 1;
                first.push(secs(i));
            }
            for c in [&a, &b] {
                r.bodies += c.bodies;
                r.newborns += c.newborns;
                own += c.own;
                rel += c.relatives;
                comb += c.combined;
                metered += c.metered;
            }
            match (a.score, b.score) {
                (Some(p), Some(q)) => {
                    let d = p - q;
                    ds.push(d);
                    plus_sum += p;
                    minus_sum += q;
                    sum[m.pair].0 += d;
                    sum[m.pair].1 += 1;
                }
                _ => r.readout.measurements_missing += 1,
            }
        }
        for _ in 0..job.knobs.nulls {
            let (a, b) = (copies.next().expect("null copy")?, copies.next().expect("null copy")?);
            r.readout.copies += 2;
            r.readout.copy_thread_s += a.wall_s + b.wall_s;
            r.readout.nulls_identical += usize::from(identical(&a, &b));
            for (x, y) in a.by_body.iter().zip(&b.by_body) {
                let gap = if x.0 == y.0 { (x.1 - y.1).abs() } else { f64::INFINITY };
                r.readout.null_max_gap = r.readout.null_max_gap.max(gap);
            }
            if a.by_body.len() != b.by_body.len() {
                r.readout.null_max_gap = f64::INFINITY;
            }
        }
        for (p, (s, n)) in sum.into_iter().enumerate() {
            if n > 0 {
                let d = s / n as f64;
                r.scores.pairs.push((p, d / 2.0, -d / 2.0));
            } else {
                r.scores.missing += 1;
            }
        }
        let k = ds.len();
        r.readout.pairs_scored = r.scores.pairs.len();
        if k > 0 {
            let mean = ds.iter().sum::<f64>() / k as f64;
            r.readout.mean_d = mean;
            r.readout.mean_abs_d = ds.iter().map(|d| d.abs()).sum::<f64>() / k as f64;
            r.readout.sd_d = (ds.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / k as f64).sqrt();
            r.readout.share_d_zero = ds.iter().filter(|d| **d == 0.0).count() as f64 / k as f64;
            r.readout.mean_plus = plus_sum / k as f64;
            r.readout.mean_minus = minus_sum / k as f64;
        }
        if twins > 0 {
            r.readout.share_diverged = diverged as f64 / twins as f64;
        }
        first.sort_by(f64::total_cmp);
        r.readout.median_first_diff_s = (!first.is_empty()).then(|| first[first.len() / 2]);
        let n = metered.max(1) as f64;
        (r.mean_own, r.mean_relatives, r.mean_combined) = (own / n, rel / n, comb / n);
        out.push(r);
    }
    Ok(out)
}

#[cfg(test)]
#[path = "twins_tests.rs"]
mod tests;
