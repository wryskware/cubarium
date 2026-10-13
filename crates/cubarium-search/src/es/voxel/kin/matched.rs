//! **Matched copies** (§11.7, topic 10): a scene runs twice from one start; one focal body
//! carries `+ε` in one copy and `−ε` in the other, everything else identical. The
//! difference in the focal's kin-weighted score estimates its own variant's effect,
//! relatives included. The focal and the sign rotate ([`rotate`]).
//!
//! **Generic over the scene** ([`Scene`]): a staged encounter ([`EngageScene`], ENGAGE's
//! `cubarium_voxel_sim::engagement`) and a whole live window from a save ([`SavedScene`]) are
//! the same machinery on different scenes, so matched live windows are a cost choice, not
//! new code. The scores are the live windows' own energy terms: every copy is metered by a
//! [`Window`] with no variant table, the focal's controller installed by hand.
//!
//! **What a scene must offer**: a fresh copy of its start, as many times as asked, every
//! body already on its scene driver ([`Scene::open`]); one tick at a time, saying when the
//! episode has ended ([`SceneCopy::step`]); its `Sim` to read and to install the focal's
//! controller on. Scores are read by the runner, not the scene.
//!
//! **Determinism.** Matched copies work only if two copies with identical brains score
//! alike: [`null_pair`] measures the gap. Both shipped scenes run their sim on one thread
//! with the chained (non-overlapped) schedule.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use cubarium_voxel_fauna::{BrainMemory, Controller, Decision, DecisionInput, Founder, Layer, Response, Seek};
use cubarium_voxel_sim::Sim;
use serde::{Deserialize, Serialize};

use super::compare::PairScores;
use super::config::KinConfig;
use super::score::{KinParts, kin_scores};
use crate::es::voxel::EpisodeDriver;
use crate::es::voxel::live::curiosity::{Curiosity, Memory};
use crate::es::voxel::live::variants::VariantTable;
use crate::es::voxel::live::window::Window;

/// A scene the matched runner can copy.
pub trait Scene: Sync {
    type Copy: SceneCopy + Send;
    /// A fresh copy of the start, every body on its scene driver. Two calls give two
    /// copies of one start.
    fn open(&self) -> Result<Self::Copy, String>;
    /// The longest episode, ticks (the window the copy is metered over).
    fn ticks(&self) -> u64;
    /// The bodies of `lineage` that may be focal, in a fixed order. Default: every living
    /// body of the lineage at the start, by id.
    fn focals(&self, copy: &Self::Copy, lineage: Founder) -> Vec<u64> {
        let mut ids: Vec<u64> = copy
            .sim()
            .fauna()
            .view()
            .animals
            .iter()
            .filter(|a| a.founder == Some(lineage))
            .map(|a| a.id)
            .collect();
        ids.sort_unstable();
        ids
    }
}

/// One running copy of a scene.
pub trait SceneCopy {
    fn sim(&self) -> &Sim;
    fn sim_mut(&mut self) -> &mut Sim;
    /// Run one tick; `false` (without stepping) once the episode has ended.
    fn step(&mut self) -> bool;
}

/// Lineages driven by a centre in a scene (the rest keep their heuristics).
pub type Centres = Vec<(Founder, Vec<f64>)>;

fn install(sim: &mut Sim, centres: &Centres) -> Result<(), String> {
    let drivers: Vec<(Founder, EpisodeDriver)> = centres
        .iter()
        .map(|(f, theta)| EpisodeDriver::gru(theta, *f).map(|d| (*f, d)))
        .collect::<Result<_, _>>()?;
    crate::es::voxel::live::install_centres(sim.fauna_mut(), &drivers);
    Ok(())
}

// ---- the staged encounter (ENGAGE) ----

/// One staged encounter (`cubarium_voxel_sim::engagement`): the seed alone fixes the start,
/// so every copy is founded afresh from it.
pub struct EngageScene {
    pub config: cubarium_voxel_sim::engagement::Config,
    pub arm: cubarium_voxel_sim::engagement::Arm,
    pub seed: u64,
    /// Lineages put on a centre after founding (empty: every body on its heuristic).
    pub centres: Centres,
}

impl SceneCopy for cubarium_voxel_sim::engagement::Episode {
    fn sim(&self) -> &Sim {
        &self.sim
    }
    fn sim_mut(&mut self) -> &mut Sim {
        &mut self.sim
    }
    fn step(&mut self) -> bool {
        cubarium_voxel_sim::engagement::Episode::step(self)
    }
}

impl Scene for EngageScene {
    type Copy = cubarium_voxel_sim::engagement::Episode;
    fn open(&self) -> Result<Self::Copy, String> {
        let mut e = cubarium_voxel_sim::engagement::found(&self.config, self.seed, &self.arm)?;
        install(&mut e.sim, &self.centres)?;
        Ok(e)
    }
    fn ticks(&self) -> u64 {
        (self.config.duration_s * f64::from(cubarium_voxel_fauna::TICK_HZ)).ceil() as u64
    }
    /// The family (adults, then the calf) for the loftstrider, the pack for the Veilhound.
    fn focals(&self, copy: &Self::Copy, lineage: Founder) -> Vec<u64> {
        match lineage {
            Founder::Loftstrider => copy.adults.iter().copied().chain(copy.calf).collect(),
            Founder::Chorister => copy.hounds.clone(),
            _ => Vec::new(),
        }
    }
}

// ---- any saved start (a whole live window) ----

/// A saved start: the three layers' bytes and the lineages its save says a policy drives.
/// Brain memory travels inside the fauna save (CONTRACTS' `Controller::memory`, restored
/// into whatever controller the body has at its next sample), so a focal's variant and
/// every other brain continue from the same state in both copies.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimStart {
    pub world: Vec<u8>,
    pub flora: Vec<u8>,
    pub fauna: Vec<u8>,
    pub policy: Vec<Founder>,
}

impl SimStart {
    /// Capture `sim` as it stands. `policy` names the lineages a centre drives in it.
    pub fn capture(sim: &Sim, policy: &[Founder]) -> SimStart {
        let (w, fl, fa) = sim.layers();
        SimStart {
            world: w.save(),
            flora: cubarium_voxel_flora::snapshot::encode(fl),
            fauna: fa.save(),
            policy: policy.to_vec(),
        }
    }

    /// A save directory ([`cubarium_voxel_sim::saved::save`]'s layout).
    pub fn read(dir: &std::path::Path, policy: &[Founder]) -> Result<SimStart, String> {
        let read = |n: &str| std::fs::read(dir.join(n)).map_err(|e| format!("{}: {e}", dir.join(n).display()));
        let world_file = if dir.join("world.bin").is_file() { "world.bin" } else { "world.voxel" };
        Ok(SimStart {
            world: read(world_file)?,
            flora: read("flora.bin")?,
            fauna: read("fauna.bin")?,
            policy: policy.to_vec(),
        })
    }

    /// A fresh `Sim` from the start on `threads` (1: one thread, chained schedule).
    pub fn open(&self, threads: usize, water_every: u32) -> Result<Sim, String> {
        let (world, flora, fauna) =
            cubarium_voxel_sim::saved::decode(&self.world, &self.flora, &self.fauna, &self.policy)?;
        let mut sim = Sim::ambient(
            world,
            flora,
            fauna,
            cubarium_voxel_sim::SimConfig { threads, overlap: threads > 1, water_every: 1 },
        );
        sim.world_mut().set_water_every(water_every);
        Ok(sim)
    }
}

/// A whole live window from a saved start.
pub struct SavedScene {
    pub start: SimStart,
    pub ticks: u64,
    /// Lineages put on a centre after loading (every lineage the save names as
    /// policy-driven must be here).
    pub centres: Centres,
    /// Sim threads per copy: 1 is the deterministic schedule.
    pub threads: usize,
    pub water_every: u32,
}

/// One copy of a [`SavedScene`].
pub struct SavedCopy {
    sim: Sim,
    left: u64,
}

impl SceneCopy for SavedCopy {
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

impl Scene for SavedScene {
    type Copy = SavedCopy;
    fn open(&self) -> Result<SavedCopy, String> {
        let mut sim = self.start.open(self.threads, self.water_every)?;
        install(&mut sim, &self.centres)?;
        Ok(SavedCopy { sim, left: self.ticks })
    }
    fn ticks(&self) -> u64 {
        self.ticks
    }
}

// ---- the runner ----

/// What one copy gives its focal: a variant of the table, or nothing (the scene's own
/// driver). Every other body keeps its scene driver in every copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct CopyPlan {
    pub focal: u64,
    /// The table's candidate the focal runs; `None`: the scene's driver.
    pub candidate: Option<usize>,
}

/// One matched pair to run: `focal` on pair `pair`, `+` in copy 0 when `plus_first`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct MatchedPlan {
    pub focal: u64,
    pub pair: usize,
    pub plus_first: bool,
}

impl MatchedPlan {
    /// The two copies' plans: identical but for the focal's sign.
    pub fn copies(&self) -> [CopyPlan; 2] {
        let (plus, minus) = (2 * self.pair, 2 * self.pair + 1);
        let (a, b) = if self.plus_first { (plus, minus) } else { (minus, plus) };
        [
            CopyPlan { focal: self.focal, candidate: Some(a) },
            CopyPlan { focal: self.focal, candidate: Some(b) },
        ]
    }
}

/// **The rotation**: `rounds` matched pairs over `focals` and `pairs`. Round `k` takes
/// pair `k mod pairs` and focal `k mod focals`; which copy carries `+` alternates each
/// time a focal comes round again, so neither copy slot is always the `+`.
pub fn rotate(focals: &[u64], pairs: usize, rounds: usize) -> Vec<MatchedPlan> {
    if focals.is_empty() || pairs == 0 {
        return Vec::new();
    }
    (0..rounds)
        .map(|k| MatchedPlan {
            focal: focals[k % focals.len()],
            pair: k % pairs,
            plus_first: (k / focals.len()) % 2 == 0,
        })
        .collect()
}

/// One copy's result.
#[derive(Clone, Debug, Serialize)]
pub struct CopyOutcome {
    pub plan: CopyPlan,
    /// Ticks the copy ran.
    pub ticks: u64,
    /// Every metered body's kin-weighted score.
    pub scores: BTreeMap<u64, KinParts>,
    /// The bodies whose controller the runner replaced, with what (only the focal).
    pub installed: Vec<(u64, Option<usize>)>,
    /// The focal's decisions, one per controller sample ([`decision`]).
    pub decisions: Vec<u64>,
}

impl CopyOutcome {
    pub fn focal(&self) -> Option<KinParts> {
        self.scores.get(&self.plan.focal).copied()
    }
}

/// A controller that records its inner controller's decisions and forwards every call.
struct Recording {
    inner: Box<dyn Controller>,
    log: Arc<Mutex<Vec<u64>>>,
}

/// A response's decision: which of its outputs are positive (logits), or which bounded
/// actions are positive; a bitmask. Two copies "decide alike" at a sample when equal. A
/// menu brain's decision sample is read by its held intent and subject instead
/// ([`intent_decision`]).
pub fn decision(r: &Response) -> u64 {
    let bits = |xs: &mut dyn Iterator<Item = f64>| xs.take(64).enumerate().fold(0u64, |m, (i, x)| m | (u64::from(x > 0.0) << i));
    match r {
        Response::Logits(l) => bits(&mut l.iter().copied()),
        other => {
            // Bounded actions: read through the Debug form's numbers, sign only.
            let text = format!("{other:?}");
            bits(&mut text
                .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e'))
                .filter_map(|t| t.parse::<f64>().ok()))
        }
    }
}

/// A menu brain's decision: its held intent and subject row, packed.
pub fn intent_decision(d: &Decision) -> u64 {
    (1 << 63) | ((d.intent as u64) << 32) | d.subject.map_or(0xffff_ffff, |s| s as u64)
}

impl Controller for Recording {
    fn drive(&mut self, observation: &[f64]) -> Response {
        let r = self.inner.drive(observation);
        if let Ok(mut log) = self.log.lock() {
            log.push(decision(&r));
        }
        r
    }
    fn decide(&mut self, observation: &[f64], input: &DecisionInput<'_>) -> (Response, Option<Decision>) {
        let (r, d) = self.inner.decide(observation, input);
        if let Ok(mut log) = self.log.lock() {
            log.push(d.as_ref().map_or_else(|| decision(&r), intent_decision));
        }
        (r, d)
    }
    fn memory(&self) -> Option<BrainMemory> {
        self.inner.memory()
    }
    fn restore(&mut self, memory: &BrainMemory) -> bool {
        self.inner.restore(memory)
    }
    fn reset(&mut self) {
        self.inner.reset();
    }
    fn taste_only(&self) -> bool {
        self.inner.taste_only()
    }
    fn layer(&self) -> Layer {
        self.inner.layer()
    }
    fn at_carcass(&self) -> bool {
        self.inner.at_carcass()
    }
    fn seek(&self) -> Seek {
        self.inner.seek()
    }
    fn reads_tail(&self) -> bool {
        self.inner.reads_tail()
    }
}

/// Run one copy of `scene` under `plan`: the focal on its candidate of `table` (or its
/// scene driver), everything else as the scene left it, metered over the episode.
pub fn run_copy<S: Scene>(
    scene: &S,
    plan: CopyPlan,
    table: Option<&VariantTable>,
    kin: &KinConfig,
) -> Result<CopyOutcome, String> {
    let mut copy = scene.open()?;
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut installed = Vec::new();
    let inner = match (plan.candidate, table) {
        (Some(c), Some(t)) => {
            installed.push((plan.focal, Some(c)));
            Some(t.controller(c))
        }
        (Some(_), None) => return Err("a candidate needs a variant table".into()),
        (None, _) => copy.sim_mut().fauna_mut().take_controller(plan.focal),
    };
    let inner = inner.ok_or_else(|| format!("focal {} has no controller in the scene", plan.focal))?;
    if !copy
        .sim_mut()
        .fauna_mut()
        .set_controller(plan.focal, Box::new(Recording { inner, log: Arc::clone(&log) }))
    {
        return Err(format!("focal {} is not in the scene", plan.focal));
    }
    let tables = vec![None; Founder::COUNT];
    let mut w = Window::begin_kin(
        copy.sim_mut(),
        &tables,
        &[0; Founder::COUNT],
        scene.ticks(),
        &Curiosity::default(),
        Memory::default(),
        kin,
    );
    while !w.done() {
        let mark = w.before_step(copy.sim());
        if !copy.step() {
            break;
        }
        w.after_step(copy.sim_mut(), mark);
    }
    let r = w.finish(copy.sim(), 0);
    let scores = kin_scores(&r.metered, &r.roster, kin).into_iter().collect();
    let decisions = std::mem::take(&mut *log.lock().map_err(|_| "decision log poisoned")?);
    Ok(CopyOutcome { plan, ticks: r.ticks, scores, installed, decisions })
}

/// One copy run by [`run_assigned`].
#[derive(Clone, Debug, Serialize)]
pub struct AssignedOutcome {
    /// Ticks the copy ran.
    pub ticks: u64,
    /// Every metered body's kin-weighted score.
    pub scores: BTreeMap<u64, KinParts>,
    /// Every metered body's window score and its parts (intake, motor, death).
    pub metered: Vec<crate::es::voxel::live::window::MeteredBody>,
    /// Who was whose relative over the copy (TWIN-MIX's split of the social effect).
    #[serde(skip)]
    pub roster: super::Roster,
}

/// **Several bodies on variants in one copy** (LJ-TWIN's probe): run one copy of `scene`
/// with every body of `assign` on its candidate of `table` and every other body on its
/// scene driver, metered over the episode as [`run_copy`] meters it; `observe` reads the
/// sim after every tick. One entry is [`run_copy`]'s copy without the decision log.
pub fn run_assigned<S: Scene>(
    scene: &S,
    assign: &[(u64, usize)],
    table: &VariantTable,
    kin: &KinConfig,
    observe: &mut dyn FnMut(&Sim),
) -> Result<AssignedOutcome, String> {
    run_assigned_with(scene, assign, table, kin, AssignOptions::default(), observe)
}

/// What [`run_assigned_with`] meters a copy by beyond [`run_assigned`]'s defaults (no
/// curiosity, empty familiarity, the tissue reward, newborns on the scene's factory).
#[derive(Clone, Default)]
pub struct AssignOptions {
    /// Each lineage's curiosity, as the live window has it.
    pub curiosity: Curiosity,
    /// The bodies' familiarity at the start (the live world's, for a live start).
    pub memory: Memory,
    /// The live window's stomach-aware reward switch.
    pub stomach_aware_reward: bool,
    /// A lineage whose newborns get a candidate of the table the tick they are born (the
    /// birth factory is replaced for the copy); `None`: newborns take the scene's factory.
    pub newborns: Option<(Founder, usize)>,
    /// **Inherited variants** (TWIN-MIX): a newborn of this lineage whose recorded mother
    /// runs this candidate (a body of the assignment, or a newborn put on it earlier) is
    /// put on it from the tick after its birth; every other newborn keeps the scene's
    /// factory. The birth tick itself is the factory's (the fauna's factory cannot see the
    /// mother), so the leak is one tick of the centre, never a variant on a non-carrier's
    /// calf. `None`: no newborn is touched.
    pub inherit: Option<(Founder, usize)>,
}

/// [`run_assigned`] under `options` (LIVE-TWINS: a live start's copy metered as the live
/// window meters it, the whole lineage's newborns on the copy's candidate).
pub fn run_assigned_with<S: Scene>(
    scene: &S,
    assign: &[(u64, usize)],
    table: &VariantTable,
    kin: &KinConfig,
    options: AssignOptions,
    observe: &mut dyn FnMut(&Sim),
) -> Result<AssignedOutcome, String> {
    let mut copy = scene.open()?;
    for &(id, c) in assign {
        if c >= table.len() {
            return Err(format!("candidate {c} is not in a table of {}", table.len()));
        }
        if !copy.sim_mut().fauna_mut().set_controller(id, table.controller(c)) {
            return Err(format!("body {id} is not in the scene"));
        }
    }
    if let Some((f, c)) = options.newborns {
        if c >= table.len() {
            return Err(format!("candidate {c} is not in a table of {}", table.len()));
        }
        let t = table.clone();
        copy.sim_mut()
            .fauna_mut()
            .set_founder_factory(f, Arc::new(move || -> Box<dyn Controller> { t.controller(c) }));
    }
    let tables = vec![None; Founder::COUNT];
    let mut w = Window::begin_kin(
        copy.sim_mut(),
        &tables,
        &[0; Founder::COUNT],
        scene.ticks(),
        &options.curiosity,
        options.memory,
        kin,
    );
    w.stomach_aware_reward = options.stomach_aware_reward;
    // TWIN-MIX: the bodies on the inherited candidate, and every body seen so far.
    let mut heirs: Option<(Founder, usize, rustc_hash::FxHashSet<u64>, rustc_hash::FxHashSet<u64>)> =
        options.inherit.map(|(f, c)| {
            let on: rustc_hash::FxHashSet<u64> = assign.iter().filter(|a| a.1 == c).map(|a| a.0).collect();
            let seen = copy.sim().fauna().view().animals.iter().map(|a| a.id).collect();
            (f, c, on, seen)
        });
    if let Some((_, c, _, _)) = &heirs {
        if *c >= table.len() {
            return Err(format!("candidate {c} is not in a table of {}", table.len()));
        }
    }
    while !w.done() {
        let mark = w.before_step(copy.sim());
        if !copy.step() {
            break;
        }
        w.after_step(copy.sim_mut(), mark);
        if let Some((f, c, on, seen)) = &mut heirs {
            let born: Vec<(u64, bool)> = copy
                .sim()
                .fauna()
                .view()
                .animals
                .iter()
                .filter(|a| !seen.contains(&a.id))
                .map(|a| (a.id, a.founder == Some(*f) && a.mother.is_some_and(|m| on.contains(&m))))
                .collect();
            for (id, heir) in born {
                seen.insert(id);
                if heir {
                    copy.sim_mut().fauna_mut().set_controller(id, table.controller(*c));
                    on.insert(id);
                }
            }
        }
        observe(copy.sim());
    }
    let r = w.finish(copy.sim(), 0);
    let scores = kin_scores(&r.metered, &r.roster, kin).into_iter().collect();
    Ok(AssignedOutcome { ticks: r.ticks, scores, metered: r.metered, roster: r.roster })
}

/// One matched pair's result.
#[derive(Clone, Debug, Serialize)]
pub struct MatchedPair {
    pub plan: MatchedPlan,
    /// The focal's scores on `+` and on `−`; `None` where the copy did not meter it.
    pub plus: Option<KinParts>,
    pub minus: Option<KinParts>,
    /// Share of the focal's samples whose decision differs between the copies (samples
    /// past the shorter copy's end count as differing).
    pub decisions_changed: f64,
    pub ticks: [u64; 2],
}

impl MatchedPair {
    /// `combined(+) − combined(−)`, the estimate; `None` with a sign missing.
    pub fn difference(&self) -> Option<f64> {
        Some(self.plus?.combined - self.minus?.combined)
    }
}

/// Share of samples at which two decision streams differ.
pub fn changed_share(a: &[u64], b: &[u64]) -> f64 {
    let n = a.len().max(b.len());
    if n == 0 {
        return 0.0;
    }
    let same = a.iter().zip(b).filter(|(x, y)| x == y).count();
    (n - same) as f64 / n as f64
}

/// Run one matched pair: its two copies, on `threads` workers (1 or 2).
pub fn run_pair<S: Scene>(
    scene: &S,
    plan: MatchedPlan,
    table: &VariantTable,
    kin: &KinConfig,
    threads: usize,
) -> Result<MatchedPair, String> {
    let copies = plan.copies().to_vec();
    let mut out = crate::es::voxel::live::trainer::parallel(copies, threads.clamp(1, 2), |c| {
        run_copy(scene, c, Some(table), kin)
    })
    .into_iter();
    let (a, b) = (out.next().expect("copy 0")?, out.next().expect("copy 1")?);
    let (plus, minus) = if plan.plus_first { (&a, &b) } else { (&b, &a) };
    Ok(MatchedPair {
        plan,
        plus: plus.focal(),
        minus: minus.focal(),
        decisions_changed: changed_share(&a.decisions, &b.decisions),
        ticks: [a.ticks, b.ticks],
    })
}

/// The update's scores from matched pairs: per pair, the mean of the focal's combined
/// score over its `+` copies and over its `−` copies; a pair with no complete result is
/// missing, never zero.
pub fn pair_scores(results: &[MatchedPair], pairs: usize) -> PairScores {
    let mut sum = vec![(0.0, 0.0, 0usize); pairs];
    for r in results {
        if let (Some(p), Some(m)) = (r.plus, r.minus) {
            let s = &mut sum[r.plan.pair];
            s.0 += p.combined;
            s.1 += m.combined;
            s.2 += 1;
        }
    }
    let mut out = PairScores::default();
    for (p, (a, b, n)) in sum.into_iter().enumerate() {
        if n > 0 {
            out.pairs.push((p, a / n as f64, b / n as f64));
        } else {
            out.missing += 1;
        }
    }
    out
}

/// **The null pair**: two copies with identical brains (`plan` in both). How far apart
/// they score is the matched comparison's noise floor.
#[derive(Clone, Debug, Serialize)]
pub struct NullPair {
    pub focal: u64,
    /// `|combined(copy 0) − combined(copy 1)|` for the focal.
    pub focal_gap: f64,
    /// The largest gap over every body metered in both copies, own and combined.
    pub max_own_gap: f64,
    pub max_combined_gap: f64,
    pub bodies: usize,
    /// Bodies metered in one copy only (a birth or a body in one copy only).
    pub unmatched_bodies: usize,
    pub decisions_changed: f64,
    pub ticks: [u64; 2],
}

/// Run a null pair on `scene` under `plan` (the same in both copies).
pub fn null_pair<S: Scene>(
    scene: &S,
    plan: CopyPlan,
    table: Option<&VariantTable>,
    kin: &KinConfig,
    threads: usize,
) -> Result<NullPair, String> {
    let mut out = crate::es::voxel::live::trainer::parallel(vec![plan, plan], threads.clamp(1, 2), |c| {
        run_copy(scene, c, table, kin)
    })
    .into_iter();
    let (a, b) = (out.next().expect("copy 0")?, out.next().expect("copy 1")?);
    let mut np = NullPair {
        focal: plan.focal,
        focal_gap: match (a.focal(), b.focal()) {
            (Some(x), Some(y)) => (x.combined - y.combined).abs(),
            _ => f64::INFINITY,
        },
        max_own_gap: 0.0,
        max_combined_gap: 0.0,
        bodies: 0,
        unmatched_bodies: 0,
        decisions_changed: changed_share(&a.decisions, &b.decisions),
        ticks: [a.ticks, b.ticks],
    };
    for (id, x) in &a.scores {
        match b.scores.get(id) {
            Some(y) => {
                np.bodies += 1;
                np.max_own_gap = np.max_own_gap.max((x.own - y.own).abs());
                np.max_combined_gap = np.max_combined_gap.max((x.combined - y.combined).abs());
            }
            None => np.unmatched_bodies += 1,
        }
    }
    np.unmatched_bodies += b.scores.keys().filter(|id| !a.scores.contains_key(id)).count();
    Ok(np)
}
