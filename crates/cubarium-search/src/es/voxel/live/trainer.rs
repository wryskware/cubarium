//! **The live-training loop**: many persistent worlds, every lineage with a centre at
//! once, one antithetic ES update per lineage per window.
//!
//! - One world per worker, one simulation thread each. Worlds are founded once and are
//!   **not reset** between generations.
//! - A generation is a window of simulated time ([`LiveSpec::window_ticks`]). Each
//!   lineage's `2·pairs` variants are placed on its living bodies at the window's start,
//!   twins on neighbours ([`super::window::place_twins`]), and on its newborns as they
//!   appear ([`super::window::newborn_candidate`]). Pair numbering runs on from one
//!   world to the next, so every pair is placed somewhere before any is placed twice.
//! - **Normalised within the world before pooling**: a body's score is z-scored against
//!   its own world's bodies of its lineage, a candidate's pooled score is the mean z of
//!   its bodies over all worlds (zero with none), and the lineage's update is the frozen
//!   trainer's own centred-rank antithetic gradient and Adam step over those scores.
//! - **Population bounds, training only**: a world that has lost a trained lineage, or
//!   holds more animals than [`LiveSpec::population_cap`], is replaced by a freshly
//!   founded one on a new seed. Every replacement is counted and logged.
//!
//! Nothing is hard-wired to two lineages: centres are given per [`Founder`], a lineage
//! without one is not trained (its bodies keep what the seeder gave them), and every
//! table is indexed over [`Founder::ALL`].

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use cubarium_voxel_fauna::Founder;
use cubarium_voxel_sim::Sim;
use serde::{Deserialize, Serialize};

use super::ecology::EcologyParams;
use super::readout;
use super::variants::VariantTable;
use super::window::{self, BodyScore, LiveWorld, WindowResult};
use super::world;
use crate::es::optimizer::{self, Adam};
use crate::es::rng::perturbation;
use crate::es::voxel::kin::stage::Perturber;
use crate::es::voxel::store::VoxelPolicyFile;
use crate::es::voxel::two_layer::{BrainFile, TwoLayerPolicyFile};

/// The protocol label written into the centres this loop saves.
pub const LIVE_PROTOCOL: &str = "live-world-1";

/// How far apart two worlds' founding seed bases start (the landscapes' own stride).
pub const WORLD_SEED_STRIDE: u64 = 1_000;

/// One run.
#[derive(Clone, Debug)]
pub struct LiveSpec {
    /// The starting centre per trained lineage.
    pub centres: Vec<(Founder, PathBuf)>,
    pub worlds: usize,
    /// Worker threads (one world at a time each).
    pub threads: usize,
    pub pairs: usize,
    /// One generation's window, ticks.
    pub window_ticks: u64,
    pub generations: u32,
    /// Stop after the generation that crosses this.
    pub wall_seconds: f64,
    /// Calibration deadline for the first boundary, wall seconds including setup.
    pub first_generation_seconds: f64,
    pub sigma: f64,
    pub learning_rate: f64,
    pub train_seed: u64,
    /// The first world's founding seed base; world `i` starts at `+ i·1000`.
    pub world_seed: u64,
    /// Training-only coarse water ([`cubarium_voxel::World::set_water_every`]).
    pub water_every: u32,
    /// A world holding more animals than this is replaced.
    pub population_cap: usize,
    /// Write the centres every this many generations (and at the end).
    pub save_every: u32,
    pub out: PathBuf,
    /// Restore centres, Adam, persistent worlds and curiosity from this run.
    pub resume: Option<PathBuf>,
    /// The ecology every world is founded under (training only; defaults: the shipped
    /// world).
    pub ecology: EcologyParams,
    /// The world every training world is founded from: a host file's `[world]`
    /// (`--world`, e.g. the desktop terrarium), or the still `small` preset when `None`.
    pub world: Option<cubarium_voxel::Config>,
    /// Each lineage's curiosity (`--curiosity`; weight 0 everywhere without).
    pub curiosity: super::curiosity::Curiosity,
    /// Experimental training reward, false keeps tissue/reserve/escrow accounting.
    pub stomach_aware_reward: bool,
    /// Each lineage's kin knobs (`--kin`; KIN-TRAIN). The default is today's training.
    pub kin: crate::es::voxel::kin::KinConfig,
    /// **The brain checks** (BRAIN-FIX step 4, `--check-every`): every two-layer lineage's
    /// centre beside its heuristic on matched starts, on the starting centre and every
    /// `every` generations, one JSON line each in `checks.jsonl`. Readouts only: `None`
    /// (the default) and every plain lineage train exactly as before.
    pub check: Option<crate::es::voxel::two_layer::check::CheckSpec>,
    /// **Training top-ups** (BRAIN-FIX step 4, `--top-up`): lineages brought up to a count
    /// of grown bodies after every founding (the lanternjaw, which the training worlds found
    /// none of). Empty (the default): every world founds exactly as before.
    pub top_up: Vec<world::TopUp>,
}

/// One lineage's side of a generation.
#[derive(Clone, Debug, Serialize)]
pub struct LineageUpdate {
    pub lineage: String,
    pub bodies: usize,
    pub newborn_bodies: usize,
    /// Candidates (of `2·pairs`) that had at least one body.
    pub candidates_placed: usize,
    pub mean_raw_score: f64,
    pub gradient_norm: f64,
    pub update_rms: f64,
    /// Pooled (within-world z) score per candidate, in candidate order.
    pub candidate_scores: Vec<f64>,
    /// The comparison it was scored under (`per_body`, `family`).
    pub comparison: crate::es::voxel::kin::Comparison,
    /// Pairs left out of the update for a missing sign (`family`; `per_body` keeps today's
    /// zero for an unplaced candidate and reports 0 here).
    pub pairs_missing: usize,
    /// Mean raw own, relatives' and combined score over its scored bodies (§11.7).
    pub mean_own: f64,
    pub mean_relatives: f64,
    pub mean_combined: f64,
    /// The `twins` comparison's readout (LIVE-TWINS); absent for every other lineage, so
    /// their records are written as before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twins: Option<crate::es::voxel::kin::twins::TwinsReadout>,
}

/// One world replaced after a generation.
#[derive(Clone, Debug, Serialize)]
pub struct Replacement {
    pub world: usize,
    pub reason: String,
    /// `copy of world N` or `fresh seed S`.
    pub with: String,
}

/// One generation's record (one JSON line in `generations.jsonl`).
#[derive(Clone, Debug, Serialize)]
pub struct GenerationRecord {
    pub generation: u32,
    pub wall_s: f64,
    pub elapsed_s: f64,
    pub ticks: u64,
    pub body_windows: usize,
    pub body_scores: Vec<BodyScore>,
    pub lineages: Vec<LineageUpdate>,
    pub replaced: Vec<Replacement>,
    pub intake_anomalies: u64,
    pub readout: Vec<(String, f64)>,
    /// Wall seconds of the generation's twin copies (LIVE-TWINS); absent without a lineage
    /// on the `twins` comparison.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twins_wall_s: Option<f64>,
}

/// A lineage being trained: its centre's weights and optimizer, and the file the centre
/// came from (BRAIN-NEXT step 1). A plain file trains exactly as before; a two-layer file
/// ([`TwoLayerPolicyFile`]) builds its variants with the two-layer brain, perturbed at the
/// lineage's stage (`kin.<lineage>.stage`; stage 1: only the top, [`Perturber`]), draws the
/// update from the same restricted stream, and is written back as a two-layer file.
struct Lineage {
    founder: Founder,
    theta: Vec<f64>,
    adam: Adam,
    template: BrainFile,
}

impl Lineage {
    /// A centre file as `founder`'s lineage: a plain file exactly as before (a `transfer:`
    /// path too), a two-layer file through its own validation.
    fn load(spec: &LiveSpec, founder: Founder, path: &Path) -> Result<Lineage, String> {
        let template = if is_two_layer(path) {
            let file = TwoLayerPolicyFile::load(path)?;
            let declared = crate::es::voxel::parse_founder(&file.founder)?;
            if declared != founder {
                return Err(format!(
                    "{} declares the {} lineage, not {}",
                    path.display(),
                    declared.name(),
                    founder.name()
                ));
            }
            BrainFile::TwoLayer(file)
        } else {
            let file = VoxelPolicyFile::load_or_transfer(path)?;
            let declared = file.founder()?;
            if declared != founder {
                return Err(format!(
                    "{} declares the {} lineage, not {}",
                    path.display(),
                    declared.name(),
                    founder.name()
                ));
            }
            BrainFile::Plain(file)
        };
        let theta = match &template {
            BrainFile::Plain(f) => f.theta.clone(),
            BrainFile::TwoLayer(f) => f.theta.clone(),
        };
        let mut adam = Adam::new(theta.len());
        adam.learning_rate = spec.learning_rate;
        let l = Lineage { founder, theta, adam, template };
        // Validates length, finiteness and this build's digest, and the world's mass.
        super::search::check_mass(&[(founder, l.driver(spec)?)], &spec.ecology)?;
        Ok(l)
    }

    /// The perturbation the variants and the update draw: today's for a plain file, the
    /// lineage's stage for a two-layer one.
    fn perturber(&self, spec: &LiveSpec) -> Perturber {
        match &self.template {
            BrainFile::Plain(_) => Perturber::plain(),
            BrainFile::TwoLayer(f) => Perturber::two_layer(&f.layout(), spec.kin.of(self.founder).stage),
        }
    }

    /// The two-layer file at the current centre.
    fn two_layer_at(&self, f: &TwoLayerPolicyFile) -> TwoLayerPolicyFile {
        TwoLayerPolicyFile { theta: self.theta.clone(), ..f.clone() }
    }

    /// The holding knobs a two-layer lineage's brains run by: the training worlds' own
    /// `fauna.founders.<lineage>.brain`.
    fn arbitration(&self, spec: &LiveSpec) -> crate::es::voxel::two_layer::IntentArbitration {
        spec.ecology.fauna().founder(self.founder).brain
    }

    /// The current centre as a driver (the birth factory's brain).
    fn driver(&self, spec: &LiveSpec) -> Result<crate::es::voxel::EpisodeDriver, String> {
        match &self.template {
            BrainFile::Plain(_) => crate::es::voxel::EpisodeDriver::gru(&self.theta, self.founder),
            BrainFile::TwoLayer(f) => Ok(crate::es::voxel::EpisodeDriver::two_layer(
                self.two_layer_at(f).net()?,
                self.arbitration(spec),
                self.founder,
            )),
        }
    }

    /// Whether the lineage is on the `twins` comparison (LIVE-TWINS).
    fn twins(&self, spec: &LiveSpec) -> bool {
        spec.kin.of(self.founder).comparison == crate::es::voxel::kin::Comparison::Twins
    }

    /// Its pairs per generation: the run's, or its twins layout's.
    fn pairs(&self, spec: &LiveSpec) -> usize {
        if self.twins(spec) { spec.kin.of(self.founder).twins.pairs } else { spec.pairs }
    }

    /// This generation's `2·pairs` variants.
    fn table(&self, spec: &LiveSpec, generation: u32) -> Result<VariantTable, String> {
        let pairs = self.pairs(spec);
        match &self.template {
            BrainFile::Plain(_) => VariantTable::antithetic(
                self.founder,
                &self.theta,
                spec.train_seed,
                spec.sigma,
                generation,
                pairs,
            ),
            BrainFile::TwoLayer(f) => VariantTable::antithetic_two_layer(
                &self.two_layer_at(f),
                spec.kin.of(self.founder).stage,
                self.arbitration(spec),
                spec.train_seed,
                spec.sigma,
                generation,
                pairs,
            ),
        }
    }
}

/// Whether a centre path is a two-layer file (by its schema token); anything else,
/// including a `transfer:` path, loads as a plain file exactly as before.
fn is_two_layer(path: &Path) -> bool {
    if path.to_string_lossy().starts_with("transfer:") {
        return false;
    }
    #[derive(Deserialize)]
    struct Token {
        schema: String,
    }
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Token>(&b).ok())
        .is_some_and(|t| t.schema == crate::es::voxel::two_layer::TWO_LAYER_POLICY_SCHEMA)
}

/// A lineage's template in the checkpoint: untagged, so a plain lineage's checkpoint is
/// byte-for-byte today's (`shape` is required by the two-layer file, which a plain file has
/// not, so a plain template never reads as two-layer).
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum SavedTemplate {
    TwoLayer(TwoLayerPolicyFile),
    Plain(VoxelPolicyFile),
}

impl From<&BrainFile> for SavedTemplate {
    fn from(b: &BrainFile) -> SavedTemplate {
        match b {
            BrainFile::Plain(f) => SavedTemplate::Plain(f.clone()),
            BrainFile::TwoLayer(f) => SavedTemplate::TwoLayer(f.clone()),
        }
    }
}

impl From<SavedTemplate> for BrainFile {
    fn from(s: SavedTemplate) -> BrainFile {
        match s {
            SavedTemplate::Plain(f) => BrainFile::Plain(f),
            SavedTemplate::TwoLayer(f) => BrainFile::TwoLayer(f),
        }
    }
}

/// Z-score each body within its own world and lineage, then pool: a candidate's score is
/// the mean z of its bodies across worlds, zero for a candidate no body ran.
///
/// Returns `(scores by candidate, bodies by candidate)`.
pub fn pooled_scores(per_world: &[Vec<(usize, f64)>], candidates: usize) -> (Vec<f64>, Vec<usize>) {
    let mut sum = vec![0.0; candidates];
    let mut n = vec![0usize; candidates];
    for bodies in per_world {
        if bodies.is_empty() {
            continue;
        }
        let m = bodies.iter().map(|b| b.1).sum::<f64>() / bodies.len() as f64;
        let var = bodies.iter().map(|b| (b.1 - m).powi(2)).sum::<f64>() / bodies.len() as f64;
        let sd = var.sqrt();
        for &(c, s) in bodies {
            let z = if sd > 1e-12 { (s - m) / sd } else { 0.0 };
            sum[c] += z;
            n[c] += 1;
        }
    }
    let scores = sum
        .iter()
        .zip(&n)
        .map(|(s, &k)| if k > 0 { s / k as f64 } else { 0.0 })
        .collect();
    (scores, n)
}

/// The antithetic update from pooled candidate scores (`2p` = plus, `2p+1` = minus): the
/// frozen trainer's centred-rank gradient and Adam ascent. Returns `(‖g‖, update rms)`.
pub fn ascend(
    theta: &mut [f64],
    adam: &mut Adam,
    scores: &[f64],
    sigma: f64,
    train_seed: u64,
    generation: u32,
) -> (f64, f64) {
    ascend_with(theta, adam, scores, sigma, |i, out| {
        perturbation(train_seed, u64::from(generation), i as u64, out);
    })
}

/// [`ascend`] over any pair-keyed perturbation stream (`eps(pair, out)`): a two-layer
/// lineage's stage-restricted draw ([`Perturber::eps`]), so a stage-1 gradient is zero on
/// the bottom by construction.
pub fn ascend_with(
    theta: &mut [f64],
    adam: &mut Adam,
    scores: &[f64],
    sigma: f64,
    eps: impl Fn(usize, &mut [f64]) + Sync,
) -> (f64, f64) {
    let pairs = scores.len() / 2;
    let plus: Vec<f64> = (0..pairs).map(|p| scores[2 * p]).collect();
    let minus: Vec<f64> = (0..pairs).map(|p| scores[2 * p + 1]).collect();
    let g = optimizer::gradient(&plus, &minus, theta.len(), sigma, eps);
    let norm = g.iter().map(|x| x * x).sum::<f64>().sqrt();
    let rms = adam.ascend(theta, &g);
    (norm, rms)
}

/// This generation's variant tables, one per trained lineage, indexed by [`Founder::index`].
fn tables_for(spec: &LiveSpec, lineages: &[Lineage], generation: u32) -> Result<Vec<Option<VariantTable>>, String> {
    let mut tables: Vec<Option<VariantTable>> = vec![None; Founder::COUNT];
    for l in lineages {
        tables[l.founder.index()] = Some(l.table(spec, generation)?);
    }
    Ok(tables)
}

/// One lineage's update from every world's window: its comparison's scores, then the
/// antithetic step over the same perturbation stream its variants were drawn from.
fn update(spec: &LiveSpec, l: &mut Lineage, results: &[WindowResult], generation: u32) -> LineageUpdate {
    let candidates = 2 * spec.pairs;
    let comparison = spec.kin.of(l.founder).comparison;
    let perturber = l.perturber(spec);
    let two_layer = matches!(l.template, BrainFile::TwoLayer(_));
    let per_world: Vec<Vec<BodyScore>> = results
        .iter()
        .map(|r| r.scores.iter().filter(|b| b.founder == l.founder).copied().collect())
        .collect();
    let mine: Vec<&BodyScore> = per_world.iter().flatten().collect();
    let (scores, n, pairs_missing, (gradient_norm, update_rms)) = match comparison {
        crate::es::voxel::kin::Comparison::Family => {
            let fam: Vec<_> = per_world.iter().zip(results).map(|(b, r)| (b.as_slice(), &r.roster)).collect();
            let by_candidate = crate::es::voxel::kin::compare::family(&fam, candidates);
            let pairs = crate::es::voxel::kin::PairScores::from_candidates(&by_candidate);
            let mut n = vec![0usize; candidates];
            for b in &mine { n[b.candidate] += 1; }
            let train_seed = spec.train_seed;
            let step = if two_layer {
                crate::es::voxel::kin::ascend_pairs(&mut l.theta, &mut l.adam, &pairs, spec.sigma,
                    |p, out| perturber.eps(train_seed, generation, p, out))
            } else {
                let generation = u64::from(generation);
                crate::es::voxel::kin::ascend_pairs(&mut l.theta, &mut l.adam, &pairs, spec.sigma,
                    |p, out| perturbation(train_seed, generation, p as u64, out))
            };
            (by_candidate.iter().map(|s| s.unwrap_or(f64::NAN)).collect::<Vec<_>>(), n, pairs.missing, step)
        }
        // `matched` is refused at the start of the run; a `twins` lineage is updated from
        // its twin copies ([`update_twins`]), never from the live windows.
        _ => {
            let refs: Vec<&[BodyScore]> = per_world.iter().map(Vec::as_slice).collect();
            let (scores, n) = crate::es::voxel::kin::compare::per_body(&refs, candidates);
            // σ = 0 is the placement-noise control: every variant is the centre, so
            // there is no direction to estimate and the centre is left alone.
            let step = if mine.len() >= 2 && spec.sigma > 0.0 {
                if two_layer {
                    let train_seed = spec.train_seed;
                    ascend_with(&mut l.theta, &mut l.adam, &scores, spec.sigma,
                        |p, out| perturber.eps(train_seed, generation, p, out))
                } else {
                    ascend(&mut l.theta, &mut l.adam, &scores, spec.sigma, spec.train_seed, generation)
                }
            } else {
                (0.0, 0.0)
            };
            (scores, n, 0, step)
        }
    };
    let mean = |f: fn(&BodyScore) -> f64| mine.iter().map(|b| f(b)).sum::<f64>() / mine.len().max(1) as f64;
    LineageUpdate {
        lineage: l.founder.name().into(),
        bodies: mine.len(),
        newborn_bodies: mine.iter().filter(|b| b.newborn).count(),
        candidates_placed: n.iter().filter(|&&k| k > 0).count(),
        mean_raw_score: mine.iter().map(|b| b.score.score).sum::<f64>()
            / mine.len().max(1) as f64,
        gradient_norm,
        update_rms,
        candidate_scores: scores.clone(),
        comparison,
        pairs_missing,
        mean_own: mean(|b| b.kin.own),
        mean_relatives: mean(|b| b.kin.relatives),
        mean_combined: mean(|b| b.kin.combined),
        twins: None,
    }
}

/// A `twins` lineage's update from its twin copies (LIVE-TWINS): each scored pair's
/// `(d̄/2, −d̄/2)` through the same antithetic step and perturbation stream as `family`.
fn update_twins(spec: &LiveSpec, l: &mut Lineage, r: &crate::es::voxel::kin::twins::TwinsResult, generation: u32) -> LineageUpdate {
    let candidates = 2 * l.pairs(spec);
    let perturber = l.perturber(spec);
    let train_seed = spec.train_seed;
    let (gradient_norm, update_rms) = if matches!(l.template, BrainFile::TwoLayer(_)) {
        crate::es::voxel::kin::ascend_pairs(&mut l.theta, &mut l.adam, &r.scores, spec.sigma,
            |p, out| perturber.eps(train_seed, generation, p, out))
    } else {
        let generation = u64::from(generation);
        crate::es::voxel::kin::ascend_pairs(&mut l.theta, &mut l.adam, &r.scores, spec.sigma,
            |p, out| perturbation(train_seed, generation, p as u64, out))
    };
    LineageUpdate {
        lineage: l.founder.name().into(),
        bodies: r.bodies,
        newborn_bodies: r.newborns,
        candidates_placed: 2 * r.scores.pairs.len(),
        mean_raw_score: r.mean_own,
        gradient_norm,
        update_rms,
        candidate_scores: r.scores.candidates(candidates).iter().map(|s| s.unwrap_or(f64::NAN)).collect(),
        comparison: crate::es::voxel::kin::Comparison::Twins,
        pairs_missing: r.scores.missing,
        mean_own: r.mean_own,
        mean_relatives: r.mean_relatives,
        mean_combined: r.mean_combined,
        twins: Some(r.readout.clone()),
    }
}

/// A world's window under this generation's main tables and its placement start.
type WindowFn<'a> = dyn Fn(&mut LiveWorld, &[Option<VariantTable>], &[usize]) -> Result<WindowResult, String> + Sync + 'a;

/// **One generation's measurements and updates** (steps 1–3 of the loop):
///
/// 1. every lineage's table, and where each world's placement starts;
/// 2. with a lineage on `twins` (LIVE-TWINS): its current centre is put on its bodies and
///    birth factory in every world, every world is captured at this boundary and the
///    lineage's twin copies run from those starts ([`crate::es::voxel::kin::twins`]);
/// 3. every world runs its window (`window`), a `twins` lineage on its centre (no table),
///    every other lineage on its variants as before;
/// 4. one update per lineage: `twins` from its copies, the rest from the windows.
///
/// Without a `twins` lineage this is the loop's old steps 1–3 exactly. Returns the window
/// results, the updates and the twin copies' wall seconds.
fn run_generation(
    spec: &LiveSpec,
    lineages: &mut [Lineage],
    worlds: &mut [LiveWorld],
    generation: u32,
    window: &WindowFn<'_>,
    check: &dyn Fn() -> Result<(), String>,
) -> Result<(Vec<WindowResult>, Vec<LineageUpdate>, Option<f64>), String> {
    // 1. This generation's tables, and where each world's placement starts.
    let tables = tables_for(spec, lineages, generation)?;
    let mut first: Vec<Vec<usize>> = Vec::with_capacity(worlds.len());
    let mut cursor = vec![0usize; Founder::COUNT];
    for w in worlds.iter() {
        first.push(cursor.clone());
        let pop = window::population(&w.sim);
        for f in Founder::ALL {
            cursor[f.index()] = (cursor[f.index()] + pop[f.index()].div_ceil(2)) % spec.pairs;
        }
    }

    // 2. The twins lineages' copies, from this boundary.
    let twinned: Vec<Founder> = lineages.iter().filter(|l| l.twins(spec)).map(|l| l.founder).collect();
    let mut twins = Vec::new();
    let mut twins_wall = None;
    let mut main_tables = None;
    if !twinned.is_empty() {
        let started = Instant::now();
        let drivers: Vec<(Founder, crate::es::voxel::EpisodeDriver)> =
            lineages.iter().map(|l| Ok((l.founder, l.driver(spec)?))).collect::<Result<_, String>>()?;
        let mine: Vec<_> = drivers.iter().filter(|d| twinned.contains(&d.0)).cloned().collect();
        let starts: Vec<crate::es::voxel::kin::twins::TwinStart> = worlds
            .iter_mut()
            .map(|w| {
                super::install_centres(w.sim.fauna_mut(), &mine);
                crate::es::voxel::kin::twins::TwinStart::capture(w.id, &w.sim, &drivers, spec.water_every, &w.memory)
            })
            .collect();
        let jobs: Vec<crate::es::voxel::kin::twins::TwinJob<'_>> = twinned
            .iter()
            .map(|f| crate::es::voxel::kin::twins::TwinJob {
                lineage: *f,
                table: tables[f.index()].as_ref().expect("a trained lineage has a table"),
                knobs: spec.kin.of(*f).twins,
            })
            .collect();
        let metering = crate::es::voxel::kin::twins::Metering {
            kin: spec.kin.clone(),
            curiosity: spec.curiosity.clone(),
            stomach_aware_reward: spec.stomach_aware_reward,
        };
        twins = crate::es::voxel::kin::twins::measure(&jobs, &starts, &metering, spec.threads)?;
        twins_wall = Some(started.elapsed().as_secs_f64());
        let mut t = tables.clone();
        for f in &twinned {
            t[f.index()] = None;
        }
        main_tables = Some(t);
        check()?;
    }

    // 3. Every world runs its window.
    let jobs: Vec<(&mut LiveWorld, Vec<usize>)> = worlds.iter_mut().zip(first).collect();
    let tables_ref = main_tables.as_ref().unwrap_or(&tables);
    let results: Vec<Result<WindowResult, String>> =
        parallel(jobs, spec.threads, |(w, first)| window(w, tables_ref, &first));
    let results: Vec<WindowResult> = results.into_iter().collect::<Result<_, _>>()?;
    check()?;

    // 4. One update per lineage, from the scores normalised within each world (or, on
    // `twins`, from its twin copies).
    let updates: Vec<LineageUpdate> = lineages
        .iter_mut()
        .map(|l| match twins.iter().find(|r| r.lineage == l.founder) {
            Some(r) => update_twins(spec, l, r, generation),
            None => update(spec, l, &results, generation),
        })
        .collect();
    Ok((results, updates, twins_wall))
}

/// Found training world `slot` from `base`. A base the founding refuses is redrawn
/// deterministically ([`world::found_world_redrawing`], noted in the run log); an accepted
/// first base founds exactly as before. Past the redraw cap this is a clean `Err`.
fn found(spec: &LiveSpec, slot: usize, base: u64, lineages: &[Lineage]) -> Result<LiveWorld, String> {
    let cfg = spec.world.clone().unwrap_or_else(world::training_config);
    let ecology = spec.ecology.clone();
    let (mut sim, info) = world::found_world_redrawing(
        &cfg,
        base,
        ecology.fauna().founder_counts_without_egg_bank(),
        &ecology,
        |note| println!("# world {slot}: {note}"),
    )
    .map_err(|why| format!("world {slot}: {why}"))?;
    for t in &spec.top_up {
        world::top_up(&mut sim, *t);
    }
    prepare(&mut sim, spec, lineages);
    Ok(LiveWorld {
        id: slot,
        sim,
        ecology,
        info,
        windows: 0,
        memory: Default::default(),
    })
}

/// The per-world setup a founded or copied world gets: coarse water, and every trained
/// lineage's current centre as its birth factory (newborns are reassigned a variant the
/// tick they appear, so this only fills the gap a newborn's first tick could leave).
fn prepare(sim: &mut Sim, spec: &LiveSpec, lineages: &[Lineage]) {
    sim.world_mut().set_water_every(spec.water_every);
    let drivers: Vec<_> = lineages
        .iter()
        .map(|l| (l.founder, l.driver(spec).expect("a validated centre")))
        .collect();
    super::install_centres(sim.fauna_mut(), &drivers);
}

/// Run `f` over every item on `threads` workers, results in item order.
pub(crate) fn parallel<T: Send, R: Send>(
    items: Vec<T>,
    threads: usize,
    f: impl Fn(T) -> R + Sync,
) -> Vec<R> {
    let n = items.len();
    let queue: Mutex<Vec<(usize, T)>> = Mutex::new(items.into_iter().enumerate().rev().collect());
    let out: Mutex<Vec<Option<R>>> = Mutex::new((0..n).map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1).min(n.max(1)) {
            scope.spawn(|| {
                loop {
                    let Some((i, item)) = queue.lock().expect("queue").pop() else {
                        return;
                    };
                    let r = f(item);
                    out.lock().expect("out")[i] = Some(r);
                }
            });
        }
    });
    out.into_inner()
        .expect("out")
        .into_iter()
        .map(|r| r.expect("every item ran"))
        .collect()
}

fn save_centres(spec: &LiveSpec, lineages: &[Lineage], generation: u32, tag: &str) -> Result<(), String> {
    let dir = spec.out.join("centres");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for l in lineages {
        let path = dir.join(format!("{tag}-{}.json", l.founder.name()));
        match &l.template {
            BrainFile::Plain(template) => {
                let file = VoxelPolicyFile {
                    build: crate::evaluate::BUILD_ID.into(),
                    train_seed: spec.train_seed,
                    generation: Some(u64::from(generation)),
                    score: None,
                    arena_protocol: LIVE_PROTOCOL.into(),
                    protocol_hash: None,
                    imitation: None,
                    theta: l.theta.clone(),
                    ..template.clone()
                };
                file.write(&path)?;
            }
            // Written back through its own format: shape and founder from the template,
            // this run's weights, seed and generation.
            BrainFile::TwoLayer(template) => {
                let file = TwoLayerPolicyFile {
                    build: crate::evaluate::BUILD_ID.into(),
                    train_seed: spec.train_seed,
                    generation: Some(u64::from(generation)),
                    score: None,
                    origin: LIVE_PROTOCOL.into(),
                    theta: l.theta.clone(),
                    ..template.clone()
                };
                file.validate(&path.display().to_string())?;
                file.write(&path)?;
            }
        }
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct TrainingCheckpoint {
    next_generation: u32,
    next_fresh_seed: u64,
    train_seed: u64,
    pairs: usize,
    window_ticks: u64,
    sigma: f64,
    curiosity: super::curiosity::Curiosity,
    stomach_aware_reward: bool,
    #[serde(default)]
    kin: crate::es::voxel::kin::KinConfig,
    lineages: Vec<(Founder, Vec<f64>, Adam, SavedTemplate)>,
    worlds: Vec<WorldState>,
}

#[derive(Serialize, Deserialize)]
struct WorldState {
    id: usize,
    windows: u32,
    info: world::FoundInfo,
    familiarity: super::curiosity::MemoryState,
}

fn save_checkpoint(spec: &LiveSpec, lineages: &[Lineage], worlds: &[LiveWorld], next_generation: u32, next_fresh_seed: u64) -> Result<(), String> {
    let pointer = spec.out.join("checkpoint.json");
    let old = std::fs::read_to_string(&pointer).ok();
    let slot = if old.as_deref().is_some_and(|s| s.trim() == "checkpoint-a") { "checkpoint-b" } else { "checkpoint-a" };
    let dir = spec.out.join(slot);
    if dir.exists() { std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?; }
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for world in worlds {
        super::staging::save_world(&dir.join(format!("world-{}", world.id)), &world.sim)?;
    }
    let state = TrainingCheckpoint {
        next_generation, next_fresh_seed, train_seed: spec.train_seed, pairs: spec.pairs,
        window_ticks: spec.window_ticks, sigma: spec.sigma, curiosity: spec.curiosity.clone(),
        stomach_aware_reward: spec.stomach_aware_reward, kin: spec.kin.clone(),
        lineages: lineages.iter().map(|l| (l.founder, l.theta.clone(), l.adam.clone(), SavedTemplate::from(&l.template))).collect(),
        worlds: worlds.iter().map(|w| WorldState { id: w.id, windows: w.windows, info: w.info.clone(), familiarity: w.memory.state() }).collect(),
    };
    super::staging::atomic_text(&dir.join("state.json"), &serde_json::to_string(&state).map_err(|e| e.to_string())?)?;
    // Commit only after every world and the matching optimizer/familiarity state exists.
    super::staging::atomic_text(&pointer, slot)?;
    if let Some(old) = old.filter(|s| s.trim() != slot) {
        let old = old.trim();
        if matches!(old, "checkpoint-a" | "checkpoint-b") {
            std::fs::remove_dir_all(spec.out.join(old)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn load_checkpoint(spec: &LiveSpec, dir: &Path, lineages: &mut Vec<Lineage>) -> Result<(Vec<LiveWorld>, u64, u32), String> {
    let slot = std::fs::read_to_string(dir.join("checkpoint.json")).map_err(|e| format!("training checkpoint: {e}"))?;
    if !matches!(slot.trim(), "checkpoint-a" | "checkpoint-b") { return Err("invalid training checkpoint pointer".into()); }
    let dir = dir.join(slot.trim());
    let state: TrainingCheckpoint = serde_json::from_str(&std::fs::read_to_string(dir.join("state.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if state.train_seed != spec.train_seed || state.pairs != spec.pairs || state.window_ticks != spec.window_ticks
        || state.sigma != spec.sigma || state.stomach_aware_reward != spec.stomach_aware_reward
        || state.kin != spec.kin
        || serde_json::to_string(&state.curiosity).map_err(|e| e.to_string())? != serde_json::to_string(&spec.curiosity).map_err(|e| e.to_string())?
        || state.worlds.len() != spec.worlds {
        return Err("checkpoint training settings differ (seed, pairs, window, sigma, reward, kin, curiosity or worlds)".into());
    }
    let policy: Vec<_> = lineages.iter().map(|l| l.founder).collect();
    if state.lineages.iter().map(|l| l.0).collect::<Vec<_>>() != policy { return Err("checkpoint trained lineages differ".into()); }
    *lineages = state.lineages.into_iter().map(|(founder, theta, adam, template)| Lineage { founder, theta, adam, template: template.into() }).collect();
    let mut worlds = Vec::new();
    for state in state.worlds {
        let saved = cubarium_voxel_sim::saved::load(&dir.join(format!("world-{}", state.id)), &policy)?;
        let mut sim = Sim::ambient(saved.world, saved.flora, saved.fauna, cubarium_voxel_sim::SimConfig { threads: 1, overlap: false, water_every: 1 });
        prepare(&mut sim, spec, lineages);
        worlds.push(LiveWorld { id: state.id, sim, ecology: spec.ecology.clone(), info: state.info,
            windows: state.windows, memory: super::curiosity::Memory::from_state(state.familiarity) });
    }
    Ok((worlds, state.next_fresh_seed, state.next_generation))
}

fn check_world(run: &super::staging::Run, world: &LiveWorld, window: Option<&cubarium_voxel_sim::staged::Window>) -> Result<String, String> {
    let result = super::staging::instant(&world.sim).and_then(|()| match window {
        Some(w) => w.verdict(&world.sim, &super::staging::check_config(&run.dir)),
        None => Ok("drivers and ledgers OK".into()),
    });
    if let Err(why) = result {
        let dir = run.dir.join("worlds").join(format!("world-{}", world.id));
        let save = super::staging::save_world(&dir, &world.sim);
        let why = format!("world {} seed {}: {why}{}", dir.display(), world.info.seed,
            save.err().map_or(String::new(), |e| format!("; checkpoint failed: {e}")));
        run.fail(why.clone());
        Err(why)
    } else { result }
}

#[allow(clippy::too_many_arguments)]
fn staged_window(world: &mut LiveWorld, tables: &[Option<VariantTable>], first: &[usize], spec: &LiveSpec,
    run: &super::staging::Run, generation: u32, full: bool) -> Result<WindowResult, String> {
    run.check()?;
    check_world(run, world, None)?;
    let started = Instant::now();
    let mut check = full.then(|| cubarium_voxel_sim::staged::Window::open(&world.sim));
    let memory = std::mem::take(&mut world.memory);
    let mut w = window::Window::begin_kin(&mut world.sim, tables, first, spec.window_ticks, &spec.curiosity, memory, &spec.kin);
    w.stomach_aware_reward = spec.stomach_aware_reward;
    while !w.done() {
        if let Err(why) = run.check() {
            let mut r = w.finish(&world.sim, world.id);
            world.memory = std::mem::take(&mut r.memory);
            return Err(why);
        }
        w.step(&mut world.sim);
        if let Some(check) = &mut check { check.watch(&world.sim); }
    }
    let mut result = w.finish(&world.sim, world.id);
    world.memory = std::mem::take(&mut result.memory);
    world.windows += 1;
    let note = check_world(run, world, check.as_ref())?;
    let dir = run.dir.join("worlds").join(format!("world-{}", world.id));
    super::staging::save_world(&dir, &world.sim).and_then(|()| run.status(generation, u64::from(world.windows),
        spec.window_ticks as f64 / started.elapsed().as_secs_f64().max(1e-9), &note, "RUNNING"))
        .inspect_err(|why| run.fail(format!("world {} seed {}: checkpoint/status failed: {why}", dir.display(), world.info.seed)))?;
    Ok(result)
}

/// Run the loop. Returns the records it wrote.
pub fn train(spec: &LiveSpec) -> Result<Vec<GenerationRecord>, String> {
    super::staging::execute(&spec.out, spec.resume.is_some(), spec.wall_seconds, spec.first_generation_seconds, |run| train_inner(spec, run))
}

fn train_inner(spec: &LiveSpec, run: &std::sync::Arc<super::staging::Run>) -> Result<Vec<GenerationRecord>, String> {
    let run_started = Instant::now();
    if spec.worlds == 0 || spec.pairs == 0 || spec.window_ticks == 0 { return Err("worlds, pairs and window length must be positive".into()); }
    for (f, _) in &spec.centres {
        if spec.kin.of(*f).comparison == crate::es::voxel::kin::Comparison::Matched {
            return Err(format!(
                "{}: the `matched` comparison runs through the matched-copies runner \
                 (es::voxel::kin::matched), not the live loop; matched live windows are a priced \
                 trial (design/behaviour-ai-2026-10-11.md §11.7); `twins` is the live loop's \
                 matched comparison",
                f.name()
            ));
        }
        if spec.kin.of(*f).comparison == crate::es::voxel::kin::Comparison::Twins {
            crate::es::voxel::kin::twins::layout(&spec.kin.of(*f).twins, spec.worlds)
                .map_err(|why| format!("{}: {why}", f.name()))?;
        }
    }
    cubarium_voxel::world::set_thread_override(1);
    println!("# pools: workers={} world-threads=1 tail-cores=1", spec.threads.max(1).min(spec.worlds));
    std::fs::create_dir_all(&spec.out)
        .map_err(|e| format!("cannot create {}: {e}", spec.out.display()))?;
    let mut lineages: Vec<Lineage> = Vec::new();
    for (founder, path) in &spec.centres {
        lineages.push(Lineage::load(spec, *founder, path)?);
    }
    if lineages.is_empty() {
        return Err("live training needs at least one centre".into());
    }
    lineages.sort_by_key(|l| l.founder.index());

    let (mut worlds, mut next_fresh_seed, first_generation) = if let Some(dir) = &spec.resume {
        load_checkpoint(spec, dir, &mut lineages)?
    } else {
        let bases: Vec<_> = (0..spec.worlds).map(|i| (i, spec.world_seed + i as u64 * WORLD_SEED_STRIDE)).collect();
        (parallel(bases, spec.threads, |(slot, base)| found(spec, slot, base, &lineages))
            .into_iter().collect::<Result<Vec<_>, String>>()?,
            spec.world_seed + spec.worlds as u64 * WORLD_SEED_STRIDE, 0)
    };
    for world in &worlds { check_world(run, world, None)?; }
    println!("# {} {} worlds at generation {first_generation}: seeds {:?}",
        if spec.resume.is_some() { "restored" } else { "founded" }, worlds.len(), worlds.iter().map(|w| w.info.seed).collect::<Vec<_>>());
    if spec.resume.is_some() {
        super::staging::repair_log(&spec.out.join("generations.jsonl"), |r| r["generation"].as_u64().is_some_and(|g| g < u64::from(first_generation)))?;
    }
    let mut log = std::fs::OpenOptions::new().create(true).write(true)
        .append(spec.resume.is_some()).truncate(spec.resume.is_none()).open(spec.out.join("generations.jsonl"))
        .map_err(|e| format!("cannot write the generation log: {e}"))?;
    save_centres(spec, &lineages, first_generation, "latest")?;
    save_checkpoint(spec, &lineages, &worlds, first_generation, next_fresh_seed)?;
    let mut checks = match &spec.check {
        Some(_) => Some(
            std::fs::OpenOptions::new().create(true).append(true).open(spec.out.join("checks.jsonl"))
                .map_err(|e| format!("cannot write the check log: {e}"))?,
        ),
        None => None,
    };
    if let Some(log) = checks.as_mut() {
        if spec.resume.is_none() {
            run_checks(spec, &lineages, first_generation, log)?;
        }
    }
    let mut last_gen_wall = 0.0;
    let mut next_generation = first_generation;

    let mut records = Vec::new();
    for generation in first_generation..spec.generations {
        if run.boundary(generation, last_gen_wall) { break; }
        run.check()?;
        run.status(generation, 0, 0.0, "drivers and ledgers OK", "RUNNING")?;
        let started = Instant::now();
        // 1–3. The tables, the twin copies (a `twins` lineage only), every world's window,
        // one update per lineage.
        let staged = |w: &mut LiveWorld, tables: &[Option<VariantTable>], first: &[usize]| {
            let full = generation == first_generation || w.windows == 0;
            staged_window(w, tables, first, spec, run, generation, full)
        };
        let (results, updates, twins_wall_s) =
            run_generation(spec, &mut lineages, &mut worlds, generation, &staged, &|| run.check())?;

        // 4. The readout, pooled.
        let per_world: Vec<_> = results.iter().map(|r| r.readout.clone()).collect();
        let pooled = readout::pool(&per_world);

        // 5. Population bounds.
        let mut replaced = Vec::new();
        let bad: Vec<(usize, String)> = worlds
            .iter()
            .enumerate()
            .filter_map(|(i, w)| {
                let pop = window::population(&w.sim);
                let total = w.sim.fauna().view().animals.len();
                if lineages.iter().any(|l| pop[l.founder.index()] == 0) {
                    Some((i, "lost a lineage".to_string()))
                } else if total > spec.population_cap {
                    Some((i, format!("{total} animals over the cap")))
                } else {
                    None
                }
            })
            .collect();
        // Every replacement is a fresh founding, in parallel, on its own seed. (Copying
        // the healthiest world, tried first, collapsed diversity: in the first trial the
        // worlds boomed past the cap within a generation of each other and one donor
        // ended up cloned into 11 of 16 slots.)
        let fresh: Vec<(usize, u64)> = bad
            .iter()
            .map(|(i, _)| {
                let seed = next_fresh_seed;
                next_fresh_seed += WORLD_SEED_STRIDE;
                (*i, seed)
            })
            .collect();
        let founded: Vec<LiveWorld> = parallel(fresh.clone(), spec.threads, |(slot, seed)| {
            found(spec, slot, seed, &lineages)
        }).into_iter().collect::<Result<_, String>>()?;
        for (((i, reason), (_, seed)), new) in bad.into_iter().zip(fresh).zip(founded) {
            check_world(run, &new, None)?;
            worlds[i] = new;
            replaced.push(Replacement {
                world: i,
                reason,
                with: format!("fresh seed {seed}"),
            });
        }

        let ticks: u64 = results.iter().map(|r| r.ticks).sum();
        let record = GenerationRecord {
            body_scores: results.iter().flat_map(|r| r.scores.iter().copied()).collect(),
            generation,
            wall_s: started.elapsed().as_secs_f64(),
            elapsed_s: run_started.elapsed().as_secs_f64(),
            ticks,
            body_windows: results.iter().map(|r| r.scores.len()).sum(),
            lineages: updates,
            replaced,
            intake_anomalies: results.iter().map(|r| r.intake_anomalies).sum(),
            readout: pooled,
            twins_wall_s,
        };
        println!("{}", summary_line(&record));
        writeln!(log, "{}", serde_json::to_string(&record).map_err(|e| e.to_string())?)
            .map_err(|e| format!("generation log: {e}"))?;
        log.flush().map_err(|e| format!("generation log flush: {e}"))?;
        next_generation = generation + 1;
        records.push(record);
        if spec.save_every > 0 && next_generation % spec.save_every == 0 {
            save_centres(spec, &lineages, next_generation, "latest")?;
            save_checkpoint(spec, &lineages, &worlds, next_generation, next_fresh_seed)?;
        }
        if let (Some(c), Some(log)) = (&spec.check, checks.as_mut()) {
            if c.every > 0 && next_generation % c.every == 0 {
                run_checks(spec, &lineages, next_generation, log)?;
            }
        }
        last_gen_wall = started.elapsed().as_secs_f64();
        // STOP/wall pauses always commit the entire current boundary, even between saves.
        if (next_generation < spec.generations || run.dir.join("STOP").exists()) && run.boundary(next_generation, last_gen_wall) {
            save_checkpoint(spec, &lineages, &worlds, next_generation, next_fresh_seed)?;
            break;
        }
    }
    save_centres(spec, &lineages, next_generation, "final")?;
    save_checkpoint(spec, &lineages, &worlds, next_generation, next_fresh_seed)?;
    Ok(records)
}

/// Run the brain checks of every two-layer lineage at `generation` and log them
/// ([`LiveSpec::check`]). The check's worlds are founded as the training worlds are, on
/// the check's own seeds, and never touch a training world.
fn run_checks(spec: &LiveSpec, lineages: &[Lineage], generation: u32, log: &mut std::fs::File) -> Result<(), String> {
    let Some(c) = &spec.check else { return Ok(()) };
    let cfg = spec.world.clone().unwrap_or_else(world::training_config);
    let found = |seed: u64| -> Result<Sim, String> {
        // The same deterministic redraw as the training worlds: both arms of a seed found
        // the same world, and a refused check seed (21) is redrawn, not skipped.
        let (mut sim, _) = world::found_world_redrawing(&cfg, seed, spec.ecology.fauna().founder_counts_without_egg_bank(),
            &spec.ecology, |note| println!("# check generation {generation} seed {seed}: {note}"))?;
        for t in &spec.top_up {
            world::top_up(&mut sim, *t);
        }
        sim.world_mut().set_water_every(spec.water_every);
        Ok(sim)
    };
    for l in lineages.iter().filter(|l| matches!(l.template, BrainFile::TwoLayer(_))) {
        let report = crate::es::voxel::two_layer::check::check(l.founder, &l.driver(spec)?, generation, c, &found);
        println!("{}", crate::es::voxel::two_layer::check::summary_line(&report));
        writeln!(log, "{}", serde_json::to_string(&report).map_err(|e| e.to_string())?).map_err(|e| format!("check log: {e}"))?;
        log.flush().map_err(|e| format!("check log flush: {e}"))?;
    }
    Ok(())
}

/// One compact line per generation for the console.
pub fn summary_line(r: &GenerationRecord) -> String {
    let get = |name: &str| {
        r.readout
            .iter()
            .find(|(n, _)| n == name)
            .map_or(f64::NAN, |(_, v)| *v)
    };
    let mut s = format!(
        "gen {:>4} {:>6.1}s bw={:<5}",
        r.generation, r.wall_s, r.body_windows
    );
    for f in Founder::ALL {
        let n = f.name();
        s.push_str(&format!(
            " | {n}: pop={:.0} mov={:.2} pace={:.3} b/h={:.1} st/h={:.1} dr/h={:.1} sc={:.3} \
             nov={:.3} cur={:.3}",
            get(&format!("{n}.population")),
            get(&format!("{n}.moving_share")),
            get(&format!("{n}.pace_bl_per_s")),
            get(&format!("{n}.births_per_h")),
            get(&format!("{n}.starved_per_h")),
            get(&format!("{n}.drowned_per_h")),
            get(&format!("{n}.mean_score")),
            get(&format!("{n}.mean_novelty")),
            get(&format!("{n}.mean_curiosity")),
        ));
    }
    s.push_str(&format!(
        " | stands={:.0} tick_ms={:.3} repl={}",
        get("plants.stands"),
        get("cost.tick_ms"),
        r.replaced.len()
    ));
    for l in &r.lineages {
        if let Some(t) = &l.twins {
            s.push_str(&format!(
                " | twins {}: {} pairs d={:+.4} |d|={:.4} sd={:.4} zero={:.2} div={:.2} null={}/{} g={:.3e} {:.0}s",
                l.lineage, t.pairs_scored, t.mean_d, t.mean_abs_d, t.sd_d, t.share_d_zero,
                t.share_diverged, t.nulls_identical, t.nulls, l.gradient_norm, r.twins_wall_s.unwrap_or(0.0),
            ));
        }
    }
    s
}

/// Where a run's final centre for `founder` is written.
pub fn final_centre(out: &Path, founder: Founder) -> PathBuf {
    out.join("centres")
        .join(format!("final-{}.json", founder.name()))
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    fn setup(name: &str) -> (LiveSpec, Vec<Lineage>, Vec<LiveWorld>) {
        let out = std::env::temp_dir().join(format!("cub-live-recovery-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        let path = crate::es::voxel::store::test_centre_path(Founder::Blind, name);
        let template = VoxelPolicyFile::load_or_transfer(&path).unwrap();
        let mut lineages = vec![Lineage { founder: Founder::Blind, theta: template.theta.clone(), adam: Adam::new(template.theta.len()), template: BrainFile::Plain(template) }];
        let cfg = cubarium_voxel::Config { width: 64, height: 24, depth: 6, ..Default::default() };
        let mut world = cubarium_voxel_sim::scene::authored(cfg.clone());
        let mut flora = cubarium_voxel_flora::Flora::new(Default::default());
        let mut fauna = cubarium_voxel_fauna::Fauna::new(cubarium_voxel_fauna::FaunaConfig::placeholder());
        let mut counts = [0; Founder::COUNT];
        counts[Founder::Blind.index()] = 1;
        cubarium_voxel_sim::habitat::seed_with_founder_counts(&mut world, &mut flora, &mut fauna, counts);
        let spec = LiveSpec { centres: vec![(Founder::Blind, path)], worlds: 1, threads: 1, pairs: 1,
            window_ticks: 100, generations: 2, wall_seconds: 60.0, first_generation_seconds: 60.0,
            sigma: 0.05, learning_rate: 0.01, train_seed: 3, world_seed: 7, water_every: 1,
            population_cap: 100, save_every: 1, out, resume: None, ecology: EcologyParams::defaults(),
            world: Some(cfg), curiosity: Default::default(), stomach_aware_reward: false, kin: Default::default(), check: None, top_up: Vec::new() };
        let mut sim = Sim::ambient(world, flora, fauna, cubarium_voxel_sim::SimConfig { threads: 1, overlap: false, water_every: 1 });
        prepare(&mut sim, &spec, &lineages);
        for _ in 0..10 { sim.step(); }
        let info = world::FoundInfo { seed: 7, accepted: true, lake_rejected: 0, habitat_rejected: 0,
            founders: counts, stands: sim.flora().view().stands.len() };
        let mut w = LiveWorld { id: 0, sim, ecology: spec.ecology.clone(), info, windows: 1, memory: Default::default() };
        let id = w.sim.fauna().view().animals[0].id;
        let c = super::super::curiosity::LineageCuriosity { weight: 1.0, memory_min: 2.0, cell_bl: 1.0 };
        let g = super::super::curiosity::Geometry::of(w.sim.fauna().config().founder(Founder::Blind), &c);
        w.memory.observe(id, &g, 0.0, 0.0);
        w.memory.advance();
        w.memory.observe(id, &g, 1.0, 0.0);
        // Nonzero optimizer history and a changed centre must survive too.
        let scores = [1.0, -1.0];
        let l = &mut lineages[0];
        ascend(&mut l.theta, &mut l.adam, &scores, spec.sigma, spec.train_seed, 0);
        (spec, lineages, vec![w])
    }

    #[test]
    fn pause_restores_same_ids_ticks_familiarity_and_optimizer() {
        let (spec, mut lineages, worlds) = setup("pause");
        let ids: Vec<_> = worlds[0].sim.fauna().view().animals.iter().map(|a| a.id).collect();
        let tick = worlds[0].sim.world().tick();
        let optimizer = serde_json::to_string(&lineages[0].adam).unwrap();
        let theta = lineages[0].theta.clone();
        super::super::staging::execute(&spec.out, false, 60.0, 60.0, |run| {
            save_checkpoint(&spec, &lineages, &worlds, 1, 1007)?;
            std::fs::write(spec.out.join("STOP"), "pause").unwrap();
            assert!(run.boundary(1, 0.01));
            Ok(())
        }).unwrap();
        assert!(spec.out.join("PAUSED").is_file());
        super::super::staging::execute(&spec.out, true, 60.0, 60.0, |_| {
            let (mut restored, seed, generation) = load_checkpoint(&spec, &spec.out, &mut lineages)?;
            assert_eq!((seed, generation), (1007, 1));
            assert_eq!(restored[0].sim.world().tick(), tick);
            assert_eq!(restored[0].sim.fauna().view().animals.iter().map(|a| a.id).collect::<Vec<_>>(), ids);
            assert_eq!(lineages[0].theta, theta);
            assert_eq!(serde_json::to_string(&lineages[0].adam).unwrap(), optimizer);
            assert_eq!(restored[0].memory.now(), worlds[0].memory.now());
            assert!(restored[0].memory.knows(ids[0]));
            let c = super::super::curiosity::LineageCuriosity { weight: 1.0, memory_min: 2.0, cell_bl: 1.0 };
            let g = super::super::curiosity::Geometry::of(restored[0].sim.fauna().config().founder(Founder::Blind), &c);
            let mut before = worlds[0].memory.clone();
            assert_eq!(restored[0].memory.observe(ids[0], &g, 0.0, 0.0), before.observe(ids[0], &g, 0.0, 0.0));
            cubarium_voxel_sim::saved::driven(restored[0].sim.fauna())?;
            // Continue the restored world, rather than refounding it.
            restored[0].sim.step();
            assert_eq!(restored[0].sim.world().tick(), tick + 1);
            Ok(())
        }).unwrap();
        assert!(spec.out.join("DONE").is_file());
        std::fs::remove_dir_all(&spec.out).unwrap();
    }

    #[test]
    fn failed_training_checkpoint_keeps_the_previous_boundary() {
        let (spec, mut lineages, worlds) = setup("write-failure");
        save_checkpoint(&spec, &lineages, &worlds, 1, 1007).unwrap();
        let old = std::fs::read_to_string(spec.out.join("checkpoint.json")).unwrap();
        // This is a deterministic unwritable destination even when the suite runs as root.
        let target = if old == "checkpoint-a" { "checkpoint-b" } else { "checkpoint-a" };
        std::fs::write(spec.out.join(target), "obstruction").unwrap();
        let err = super::super::staging::execute(&spec.out, false, 60.0, 60.0, |_| save_checkpoint(&spec, &lineages, &worlds, 2, 2007)).unwrap_err();
        assert!(err.contains(target), "{err}");
        assert!(spec.out.join("FAILED").is_file());
        assert_eq!(std::fs::read_to_string(spec.out.join("checkpoint.json")).unwrap(), old);
        assert_eq!(load_checkpoint(&spec, &spec.out, &mut lineages).unwrap().2, 1);
        std::fs::remove_dir_all(&spec.out).unwrap();
    }
}

#[cfg(test)]
#[path = "trainer_brain_tests.rs"]
mod brain_tests;

#[cfg(test)]
#[path = "trainer_twins_tests.rs"]
mod twins_tests;
