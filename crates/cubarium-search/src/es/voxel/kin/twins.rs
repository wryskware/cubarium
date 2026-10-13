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
//! **Half-and-half twins** (TWIN-MIX; [`TwinKnobs::carrier_share`] below 1, the default):
//! only a drawn subset of the lineage carries `±ε_p` and the score separates what the
//! variant does for its carrier from what it does to the lineage's other bodies; see
//! [`estimate`] for the estimator. `carrier_share = 1` is the all-sign twin above, scored
//! by the same code as before.
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
    /// Half-and-half twins: the start's bodies as the estimator reads them (the same in both
    /// copies) and this copy's side; `None` on all-sign twins.
    pub mix: Option<(Vec<MixBody>, MixCopy)>,
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

/// Run one copy. All-sign (`carriers` `None`): every body of `lineage` at the start, and
/// its newborns, on `candidate`. Half-and-half (`Some`): only those bodies on `candidate`,
/// and the newborns of a body on it from the tick after their birth
/// ([`AssignOptions::inherit`]); every other body of the lineage on the centre.
pub fn run_copy(
    start: &TwinStart,
    ticks: u64,
    lineage: Founder,
    table: &VariantTable,
    candidate: usize,
    carriers: Option<&[u64]>,
    metering: &Metering,
) -> Result<CopyResult, String> {
    let t0 = std::time::Instant::now();
    let ids = &start.bodies[lineage.index()];
    let on = carriers.unwrap_or(ids);
    let assign: Vec<(u64, usize)> = on.iter().map(|&id| (id, candidate)).collect();
    let options = AssignOptions {
        curiosity: metering.curiosity.clone(),
        memory: start.memory.clone(),
        stomach_aware_reward: metering.stomach_aware_reward,
        newborns: carriers.is_none().then_some((lineage, candidate)),
        inherit: carriers.is_some().then_some((lineage, candidate)),
    };
    let mut trace = Vec::with_capacity(ticks as usize);
    let o = run_assigned_with(&start.scene(ticks), &assign, table, &metering.kin, options, &mut |sim| {
        trace.push(world_hash(sim))
    })?;
    let mut r = CopyResult {
        ticks: o.ticks,
        score: None,
        bodies: on.len(),
        newborns: 0,
        own: 0.0,
        relatives: 0.0,
        combined: 0.0,
        metered: 0,
        by_body: o.scores.iter().map(|(id, k)| (*id, k.combined)).collect(),
        mix: None,
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
    match carriers {
        None => {
            if !ids.is_empty() {
                r.score = Some(r.combined / ids.len() as f64);
            }
        }
        Some(c) => {
            let (bodies, copy) = mix_side(ids, c, lineage, &o, &metering.kin);
            // Readout only: the carriers' mean own score (the update reads [`estimate`]).
            if !c.is_empty() {
                let own: f64 = bodies.iter().zip(&copy.own).filter(|(b, _)| b.carrier).map(|(_, o)| o).sum();
                r.score = Some(own / c.len() as f64);
            }
            r.mix = Some((bodies, copy));
        }
    }
    r.wall_s = t0.elapsed().as_secs_f64();
    Ok(r)
}

// ---- half-and-half twins (TWIN-MIX) ----

/// How many of `n` bodies carry at `share`: `round(share·n)`, at least 1, and at most
/// `n − 1` below a share of 1 so a twin always keeps a non-carrier to read the social
/// effect from (one body alone carries).
pub fn carrier_count(n: usize, share: f64) -> usize {
    if n == 0 {
        return 0;
    }
    if share >= 1.0 || n == 1 {
        return n;
    }
    ((share * n as f64).round() as usize).clamp(1, n - 1)
}

fn splitmix(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A generation's carrier seed for one lineage: from the run's training seed, the
/// generation and the lineage, so a resumed run draws what it drew.
pub fn job_seed(train_seed: u64, generation: u32, lineage: Founder) -> u64 {
    let mut x = train_seed ^ 0x7477_696e_2d6d_6978; // "twin-mix"
    let a = splitmix(&mut x);
    let mut y = a ^ (u64::from(generation) << 8) ^ lineage.index() as u64;
    splitmix(&mut y)
}

/// One twin's seed: the job's, the pair and the start it is measured on (nulls use pair
/// indices past the table's).
pub fn twin_seed(job: u64, pair: usize, start: usize) -> u64 {
    let mut x = job ^ (pair as u64).wrapping_mul(0x0000_0100_0000_01b3) ^ ((start as u64) << 40);
    splitmix(&mut x)
}

/// **The carriers** of one twin: [`carrier_count`] of `ids` (the lineage's bodies at the
/// start, sorted), drawn without replacement by `seed` (a partial Fisher–Yates shuffle);
/// returned sorted. Both copies of a twin take the same draw.
pub fn carriers(ids: &[u64], share: f64, seed: u64) -> Vec<u64> {
    let m = carrier_count(ids.len(), share);
    let mut pool = ids.to_vec();
    let mut x = seed;
    for i in 0..m {
        let j = i + (splitmix(&mut x) % (pool.len() - i) as u64) as usize;
        pool.swap(i, j);
    }
    pool.truncate(m);
    pool.sort_unstable();
    pool
}

/// One body of the lineage at a twin's start, as [`estimate`] reads it (the same in both
/// copies: the roster is fixed at the start and the carriers are drawn once per twin).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct MixBody {
    pub carrier: bool,
    /// `a_k`: carriers related to it (itself excluded).
    pub related_carriers: u32,
    /// `ρ_k`: the kin shares those carriers give it, `Σ r(c, k)` over them (`r(c, k)`: the
    /// share for how `k` is related to `c`).
    pub kin_weight: f64,
}

/// One copy's side of a half-and-half twin.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct MixCopy {
    /// Per body at the start, in the start's id order: its own score (energy, plus
    /// curiosity where configured) and its base score (`(E_end − E_start + E_given)/ref`).
    pub own: Vec<f64>,
    pub base: Vec<f64>,
    /// The lineage's newborns in this copy: `Σ base`, and `Σ (ρ_k / a_k)·base` over those
    /// related to a carrier (their kin, split among the carriers they are related to).
    pub newborn_base: f64,
    pub newborn_kin: f64,
}

/// What a relative `relation` (read from `a`'s side) is, read from the other side.
fn reverse(relation: super::Relation) -> super::Relation {
    use super::Relation::*;
    match relation {
        Calf => Mother,
        Mother => Calf,
        r => r,
    }
}

/// `(a_k, ρ_k)` for body `id`: carriers related to it and the shares they give it.
fn related(id: u64, carrier: &dyn Fn(u64) -> bool, roster: &super::Roster, shares: &super::KinShares) -> (u32, f64) {
    let (mut a, mut rho) = (0u32, 0.0);
    for (other, relation) in roster.relatives(id) {
        if *other != id && carrier(*other) {
            a += 1;
            // `relation` is `other`'s relation to `id`; the carrier `other` weighs `id` by
            // how `id` is related to it.
            rho += shares.of(reverse(*relation));
        }
    }
    (a, rho)
}

/// One copy's [`MixBody`]s and [`MixCopy`] from its outcome: `ids` the lineage's bodies at
/// the start and `carriers` the twin's draw, both sorted.
pub fn mix_side(
    ids: &[u64],
    carriers: &[u64],
    lineage: Founder,
    o: &super::matched::AssignedOutcome,
    kin: &KinConfig,
) -> (Vec<MixBody>, MixCopy) {
    let shares = &kin.of(lineage).shares;
    let is_carrier = |id: u64| carriers.binary_search(&id).is_ok();
    let bodies = ids
        .iter()
        .map(|&id| {
            let (a, rho) = related(id, &is_carrier, &o.roster, shares);
            MixBody { carrier: is_carrier(id), related_carriers: a, kin_weight: rho }
        })
        .collect();
    let mut copy = MixCopy::default();
    for &id in ids {
        let m = o.metered.iter().find(|m| m.body_id == id);
        copy.own.push(o.scores.get(&id).map_or(0.0, |k| k.own));
        copy.base.push(m.map_or(0.0, |m| m.base()));
    }
    for m in o.metered.iter().filter(|m| m.founder == lineage && ids.binary_search(&m.body_id).is_err()) {
        let b = m.base();
        copy.newborn_base += b;
        let (a, rho) = related(m.body_id, &is_carrier, &o.roster, shares);
        if a > 0 {
            copy.newborn_kin += rho / f64::from(a) * b;
        }
    }
    (bodies, copy)
}

/// One half-and-half twin's estimate (all per carrier, `+` minus `−`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct MixEstimate {
    /// What the update ranks: `direct + stranger_weight·social + kin`.
    pub score: f64,
    /// `δ̂`: the variant's effect on its carrier.
    pub direct: f64,
    /// The carriers' effect on every other body of the lineage, per carrier.
    pub social: f64,
    /// The kin-weighted effect on relatives, per carrier (0 with every share 0).
    pub kin: f64,
    /// `ê_S`, `ê_R`: one carrier's effect on one unrelated body, on one related body.
    pub stranger_pair: f64,
    pub kin_pair: f64,
    pub carriers: usize,
    /// Non-carriers related to no carrier (`U`) and to some carrier (`R`).
    pub strangers: usize,
    pub relatives: usize,
}

/// **The half-and-half estimator** (TWIN-MIX). Per twin, `Δx = x(+) − x(−)` for each body
/// `k` of the lineage at the start; `C` the `m` carriers, `a_k` the carriers related to `k`,
/// `u_k = m − a_k − [k ∈ C]` the carriers unrelated to it, `ρ_k` the kin shares those
/// related carriers give it ([`MixBody`]).
///
/// **The model.** Additive effects: `Δb_k = [k ∈ C]·δ + a_k·e_R + u_k·e_S`: a carrier's
/// own change (`δ`, its direct effect) plus what each related carrier did to it (`e_R`)
/// and what each unrelated carrier did to it (`e_S`). Carriers are a random draw, so a
/// carrier and a non-carrier are alike in expectation but for the variant.
///
/// - **Strangers' effect** `ê_S = Σ_{k∈U} Δb_k / Σ_{k∈U} u_k` over `U`, the non-carriers
///   related to no carrier: their whole change is what the `m` unrelated carriers did.
/// - **Relatives' effect** `ê_R = Σ_{k∈R} (Δb_k − u_k·ê_S) / Σ_{k∈R} a_k` over `R`, the
///   non-carriers related to some carrier, once the strangers' share is taken out.
/// - **Direct** `δ̂ = (1/m) Σ_{c∈C} (Δown_c − u_c·ê_S − a_c·ê_R)`: a carrier's own change
///   (curiosity included, as its own score has it) less what the other carriers did to it.
/// - **Lineage total** `L = Σ_{c∈C} Δown_c + Σ_{k∉C} Δb_k + Δ(Σ newborns' base)`: what
///   the variant did to the lineage, every body counted once; its social part per carrier
///   is `social = L/m − δ̂`.
/// - **Kin** `K = (1/m) [Σ_{k∈R} (ρ_k/a_k)·(Δb_k − u_k·ê_S) + Σ_{c∈C} ρ_c·ê_R +
///   Δ(Σ newborns' (ρ/a)·base)]`: each relative's change caused by its related carriers,
///   split evenly among them and weighted by their shares (a carrier relative's by
///   `ê_R`, since its own change also holds its direct effect; a newborn's whole value,
///   since it exists by what its mother did). Every share 0 makes it 0 exactly.
/// - **Score** `δ̂ + w·social + K` (`w` = [`TwinKnobs::stranger_weight`]). At `w = 0`:
///   own effect plus kin-weighted relatives (topic 2). At `w = 1`: `L/m + K`, the lineage
///   total over the carriers, where the estimates cancel; with every carrier (`m = n`) and
///   every share 0 that is the all-sign score, which is why `w` weighs the effect on every
///   other body, relatives too (a relative counts `w + r`). `carrier_share = 1` itself is
///   scored by the all-sign code, not here.
///
/// **Bias.** Exact under the additive model in expectation over the draw. Where effects do
/// not add (two carriers chasing one prey), every term is the effect at saturation
/// `carrier_share`, not at all-sign. `U` and `R` are chosen by kinship, not at random: if
/// relatives of carriers are placed unlike strangers (a mother with her calf), `ê_S`
/// and `ê_R` are the effects on the bodies that happen to be in those groups, and a
/// carrier's correction borrows them. With no `U` (every non-carrier related to a carrier)
/// `ê_S` is 0 and `δ̂` keeps the strangers' effect on carriers; with no `R`, `ê_R` is 0.
///
/// **Why a fresh draw per twin.** The pair's measurement must estimate the variant's
/// effect on any body, not on one subset's circumstances: drawing per twin (from
/// [`twin_seed`]) makes carriers and non-carriers exchangeable over the pair's
/// measurements, so the subset's own luck (a hungry carrier, a carrier beside prey)
/// averages out instead of entering every measurement of a start alike; the same draw in
/// both copies keeps the twin difference the sign's alone.
///
/// `None` without a carrier (a start with none of the lineage).
pub fn estimate(bodies: &[MixBody], plus: &MixCopy, minus: &MixCopy, stranger_weight: f64) -> Option<MixEstimate> {
    let m = bodies.iter().filter(|b| b.carrier).count();
    if m == 0 {
        return None;
    }
    let mf = m as f64;
    let u = |b: &MixBody| (m as u32 - b.related_carriers - u32::from(b.carrier)) as f64;
    let d_own = |k: usize| plus.own[k] - minus.own[k];
    let d_base = |k: usize| plus.base[k] - minus.base[k];
    let mut e = MixEstimate { carriers: m, ..Default::default() };
    let (mut su, mut su_n) = (0.0, 0.0);
    for (k, b) in bodies.iter().enumerate() {
        if !b.carrier && b.related_carriers == 0 {
            su += d_base(k);
            su_n += u(b);
            e.strangers += 1;
        }
    }
    e.stranger_pair = if su_n > 0.0 { su / su_n } else { 0.0 };
    let (mut sr, mut sr_n) = (0.0, 0.0);
    for (k, b) in bodies.iter().enumerate() {
        if !b.carrier && b.related_carriers > 0 {
            sr += d_base(k) - u(b) * e.stranger_pair;
            sr_n += f64::from(b.related_carriers);
            e.relatives += 1;
        }
    }
    e.kin_pair = if sr_n > 0.0 { sr / sr_n } else { 0.0 };
    let (mut direct, mut total, mut kin) = (0.0, 0.0, 0.0);
    for (k, b) in bodies.iter().enumerate() {
        if b.carrier {
            direct += d_own(k) - u(b) * e.stranger_pair - f64::from(b.related_carriers) * e.kin_pair;
            total += d_own(k);
            if b.kin_weight != 0.0 {
                kin += b.kin_weight * e.kin_pair;
            }
        } else {
            total += d_base(k);
            if b.related_carriers > 0 && b.kin_weight != 0.0 {
                kin += b.kin_weight / f64::from(b.related_carriers) * (d_base(k) - u(b) * e.stranger_pair);
            }
        }
    }
    total += plus.newborn_base - minus.newborn_base;
    kin += plus.newborn_kin - minus.newborn_kin;
    e.direct = direct / mf;
    e.social = total / mf - e.direct;
    e.kin = kin / mf;
    e.score = e.direct + stranger_weight * e.social + e.kin;
    Some(e)
}

/// One lineage's twins for one generation: its table, its layout.
pub struct TwinJob<'a> {
    pub lineage: Founder,
    pub table: &'a VariantTable,
    pub knobs: TwinKnobs,
    /// The carrier draws' seed for this generation ([`job_seed`]); unread on all-sign.
    pub seed: u64,
}

/// One copy to run.
#[derive(Clone, Debug)]
struct Item {
    job: usize,
    start: usize,
    candidate: usize,
    /// Half-and-half: the twin's carriers (both copies share the draw); `None`: all-sign.
    carriers: Option<std::sync::Arc<Vec<u64>>>,
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
    /// Half-and-half twins (TWIN-MIX; all 0 on all-sign): the knobs, carriers per scored
    /// twin, the estimate's parts' means over the scored measurements ([`MixEstimate`]),
    /// the share of scored twins with no non-carrier stranger to every carrier (`ê_S` is
    /// then 0), and the largest |estimate| over the null twins (must be 0).
    pub carrier_share: f64,
    pub stranger_weight: f64,
    pub mean_carriers: f64,
    pub mean_direct: f64,
    pub mean_social: f64,
    pub mean_kin: f64,
    pub mean_stranger_pair: f64,
    pub share_no_strangers: f64,
    pub null_max_d: f64,
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
        job.knobs.check().map_err(|why| format!("{}: {why}", job.lineage.name()))?;
        let plan = layout(&job.knobs, starts.len())?;
        // Half-and-half: one carrier draw per twin, shared by its two copies.
        let draw = |pair: usize, start: usize| {
            (!job.knobs.all_sign()).then(|| {
                let ids = &starts[start].bodies[job.lineage.index()];
                std::sync::Arc::new(carriers(ids, job.knobs.carrier_share, twin_seed(job.seed, pair, start)))
            })
        };
        for m in &plan {
            let c = draw(m.pair, m.start);
            items.push(Item { job: j, start: m.start, candidate: 2 * m.pair, carriers: c.clone() });
            items.push(Item { job: j, start: m.start, candidate: 2 * m.pair + 1, carriers: c });
        }
        for n in 0..job.knobs.nulls {
            let c = draw(job.knobs.pairs + n, n % starts.len());
            for _ in 0..2 {
                items.push(Item { job: j, start: n % starts.len(), candidate: 0, carriers: c.clone() });
            }
        }
        layouts.push(plan);
    }
    let copies: Vec<Result<CopyResult, String>> = crate::es::voxel::live::trainer::parallel(items, threads, |it| {
        let job = &jobs[it.job];
        run_copy(&starts[it.start], window_ticks(&job.knobs), job.lineage, job.table, it.candidate, it.carriers.as_deref().map(Vec::as_slice), metering)
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
                carrier_share: job.knobs.carrier_share,
                stranger_weight: job.knobs.stranger_weight,
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
        let mut parts = Vec::new();
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
            let mixed = match (&a.mix, &b.mix) {
                (Some((bodies, x)), Some((_, y))) => Some(estimate(bodies, x, y, job.knobs.stranger_weight)),
                _ => None,
            };
            let measured = match (a.score, b.score, mixed) {
                (Some(p), Some(q), None) => Some((p, q, p - q)),
                (Some(p), Some(q), Some(Some(e))) => {
                    parts.push(e);
                    Some((p, q, e.score))
                }
                _ => None,
            };
            match measured {
                Some((p, q, d)) => {
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
            if let (Some((bodies, x)), Some((_, y))) = (&a.mix, &b.mix) {
                let d = estimate(bodies, x, y, job.knobs.stranger_weight).map_or(0.0, |e| e.score.abs());
                r.readout.null_max_d = r.readout.null_max_d.max(if d.is_nan() { f64::INFINITY } else { d });
            }
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
        if !parts.is_empty() {
            let n = parts.len() as f64;
            let mean = |f: &dyn Fn(&MixEstimate) -> f64| parts.iter().map(f).sum::<f64>() / n;
            r.readout.mean_carriers = mean(&|e| e.carriers as f64);
            r.readout.mean_direct = mean(&|e| e.direct);
            r.readout.mean_social = mean(&|e| e.social);
            r.readout.mean_kin = mean(&|e| e.kin);
            r.readout.mean_stranger_pair = mean(&|e| e.stranger_pair);
            r.readout.share_no_strangers = mean(&|e| f64::from(u8::from(e.strangers == 0)));
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
