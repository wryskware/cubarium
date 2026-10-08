//! **Staged runs** (WORKING_POLICY "Staged runs and scenario maps", Wrysk 2026-10-04):
//! no study runs for hours unattended. A run advances in segments, saves a checkpoint at
//! every boundary, can be paused, resumed and branched, and stops itself with `FAILED`
//! when a scripted liveness check says the world it is measuring is not alive.
//!
//! The study tools (`succession_study`, `voxel_census`) drive a [`Runner`]: it is told
//! every tick and every sim minute, and says when to stop. Arguments:
//!
//! - `run=DIR`: the run directory. Without it a tool runs unstaged, as before.
//! - `segment_min=N` (30): sim minutes between checkpoints.
//! - `check_min=N` (3): the first segment after any load or founding, at whose end the
//!   full liveness check runs.
//! - `keep=N` (3): checkpoints kept; older ones are deleted.
//! - `expect=a,b`: ledger counters the arm's mechanisms must move during the check, by
//!   field name in the plant or animal ledger (`burrs_picked`, `seeds_landed`, ...), or
//!   `flora.NAME` / `fauna.NAME`; an array counter is summed.
//! - `still=lineage,..`: lineages exempt from the movement check (an ambusher that may
//!   sit a whole check out).
//! - `residual_tol=X` (1e-6): the largest relative ledger residual accepted.
//! - The water step is the tool's [`crate::SimConfig::experiment`] (`water_every=` on
//!   the run line, 4 by default); the status line reports the world's.
//! - `resume=DIR`: continue `DIR` from its latest checkpoint with its own arguments
//!   ([`resume_args`]). Branching from any checkpoint is `load=DIR/ckpt/mNNNNNN`.
//!
//! Files in `DIR`: `args` (the arguments, one a line), `end_minute`, `status` (one line,
//! rewritten every sim minute: minute, end, wall speed, ms/tick, the last check and the
//! state), `ckpt/mNNNNNN/` (the checkpoints, [`crate::saved`]'s layout), and at most one
//! of `DONE`, `PAUSED` or `FAILED` (its reason inside) once the run ends. `STOP` in `DIR`,
//! made by anyone, pauses the run at its next boundary.
//!
//! **The liveness check.** Right after the load: every founder body has a controller and
//! every lineage with a body a birth factory ([`crate::saved::driven`]), and the ledgers
//! conserve ([`Residuals`]). At the end of the first segment, over that segment:
//! - each lineage present moved (not in `still=`, and awake for at least a quarter of
//!   the window's body-seconds):
//!   its bodies' summed path > 0 and at least two bodies (one, for a lineage of one)
//!   ended half a voxel or more from where they started — or, standing still, it fed
//!   (a body with no controller does neither);
//!   a lineage asleep or resting nearly all the window passes as idle by choice only while
//!   every body is driven, its resting bodies were not hungry (eating and emergency latches
//!   off) for most of their resting samples, and no body was hungry and held still at every
//!   sample (FROZEN-PREY); such frozen bodies are named on a lineage that passes otherwise;
//! - the grazers (every lineage whose diet takes stand tissue) bit plants, counting only
//!   the lineages with a body that wanted food in the window (its eating latch on while
//!   awake): founders arrive fed (`founding.arrival_stomach`) and a sated body is right not
//!   to eat;
//! - every `expect=` counter moved;
//! - the ledgers conserve, and every body is still driven.
//!
//! At every later boundary the ledgers and the drivers are checked again.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use cubarium_voxel::World;
use cubarium_voxel_fauna::{Fauna, Food, Founder, Pose, State, TICK_HZ};
use cubarium_voxel_flora::Flora;
use serde::{Deserialize, Serialize};

use crate::Sim;

/// Ticks in a sim minute.
pub const TICKS_PER_MIN: u64 = 60 * TICK_HZ as u64;

/// **The ledgers' residuals**, each relative to its stock: water (stored against the
/// world ledger's expectation), and organic, mineral and energy in the plant and animal
/// layers against theirs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Residuals {
    pub water: f64,
    pub flora: [f64; 3],
    pub fauna: [f64; 3],
}

impl Residuals {
    pub fn of(world: &World, flora: &Flora, fauna: &Fauna) -> Residuals {
        let rel = |stock: f64, expected: f64| (stock - expected) / stock.abs().max(expected.abs()).max(1.0);
        let w = world.view();
        let fl = flora.view();
        let fa = fauna.view();
        Residuals {
            water: rel(w.stored_m3(), w.ledger.expected_stored()),
            flora: [
                rel(fl.organic(), fl.ledger.expected_organic()),
                rel(fl.mineral(), fl.ledger.expected_mineral()),
                rel(fl.energy(), fl.ledger.expected_energy()),
            ],
            fauna: [
                rel(fa.organic(), fa.ledger.expected_organic()),
                rel(fa.mineral(), fa.ledger.expected_mineral()),
                rel(fa.energy(), fa.ledger.expected_energy()),
            ],
        }
    }

    /// The largest residual, absolute.
    pub fn worst(&self) -> f64 {
        std::iter::once(self.water)
            .chain(self.flora)
            .chain(self.fauna)
            .map(f64::abs)
            .fold(0.0, f64::max)
    }

    /// `None` when every residual is within `tol`, else which ledger is out and by how much.
    pub fn out_of(&self, tol: f64) -> Option<String> {
        let names = ["water", "flora organic", "flora mineral", "flora energy", "fauna organic", "fauna mineral", "fauna energy"];
        let all = std::iter::once(self.water).chain(self.flora).chain(self.fauna);
        let out: Vec<String> = names
            .iter()
            .zip(all)
            .filter(|(_, r)| !(r.abs() <= tol))
            .map(|(n, r)| format!("{n} residual {r:.2e}"))
            .collect();
        (!out.is_empty()).then(|| format!("ledgers do not conserve ({} > {tol:.0e})", out.join(", ")))
    }
}

/// How a run is staged: the `run=`-family arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct StageConfig {
    pub dir: PathBuf,
    pub segment_min: u64,
    pub check_min: u64,
    pub keep: usize,
    pub expect: Vec<String>,
    pub still: Vec<Founder>,
    pub residual_tol: f64,
    /// Ticks in one of the run's minutes: [`TICKS_PER_MIN`]; a test shortens it so a
    /// staged run fits in a few hundred ticks. Not an argument.
    pub ticks_per_min: u64,
}

fn arg<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(k))
}

impl StageConfig {
    /// The staging arguments, or `None` with no `run=` (an unstaged run).
    pub fn from_args(args: &[String]) -> Result<Option<StageConfig>, String> {
        let Some(dir) = arg(args, "run=") else {
            for k in ["segment_min=", "check_min=", "keep=", "expect=", "still=", "residual_tol="] {
                if arg(args, k).is_some() {
                    return Err(format!("{k} stages a run: it needs run=DIR"));
                }
            }
            return Ok(None);
        };
        let num = |k: &str, d: u64| -> Result<u64, String> { arg(args, k).map_or(Ok(d), |v| v.parse().map_err(|e| format!("{k}{v}: {e}"))) };
        let list = |k: &str| -> Vec<String> {
            arg(args, k).map_or(Vec::new(), |v| v.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect())
        };
        let mut still = Vec::new();
        for name in list("still=") {
            still.push(Founder::ALL.into_iter().find(|f| f.name() == name).ok_or_else(|| format!("still={name}: no such lineage"))?);
        }
        let cfg = StageConfig {
            dir: PathBuf::from(dir),
            segment_min: num("segment_min=", 30)?,
            check_min: num("check_min=", 3)?,
            keep: num("keep=", 3)? as usize,
            expect: list("expect="),
            still,
            residual_tol: arg(args, "residual_tol=").map_or(Ok(1e-6), |v| v.parse().map_err(|e| format!("residual_tol={v}: {e}")))?,
            ticks_per_min: TICKS_PER_MIN,
        };
        if cfg.segment_min == 0 || cfg.check_min == 0 || cfg.keep == 0 {
            return Err("segment_min=, check_min= and keep= must be positive".into());
        }
        Ok(Some(cfg))
    }
}

/// The checkpoints in a run directory, oldest first: `(minute, dir)`.
pub fn checkpoints(run: &Path) -> Vec<(u64, PathBuf)> {
    let mut out: Vec<(u64, PathBuf)> = std::fs::read_dir(run.join("ckpt"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let m = name.strip_prefix('m')?.parse().ok()?;
            e.path().join("fauna.bin").is_file().then(|| (m, e.path()))
        })
        .collect();
    out.sort();
    out
}

/// **`resume=DIR`**: the run's own arguments (`DIR/args`), with `load=` its latest
/// checkpoint, `run=DIR` and a `resumed=1` marker ([`resumed`]); every argument that only
/// shapes a fresh world (`drop`: prefixes the tool names, such as `seed=` or `preset=`)
/// is left out, because the checkpoint carries the world. `threads=` given beside
/// `resume=` replaces the saved one; nothing else may be given. Arguments without
/// `resume=` are returned as they are.
pub fn resume_args(args: &[String], drop: &[&str]) -> Result<Vec<String>, String> {
    let Some(dir) = arg(args, "resume=") else {
        return Ok(args.to_vec());
    };
    let dir = PathBuf::from(dir);
    for a in args.iter().skip(1) {
        if !(a.starts_with("resume=") || a.starts_with("threads=")) {
            return Err(format!("resume= continues a run with its own arguments; `{a}` cannot be changed (branch with load=DIR/ckpt/mN instead)"));
        }
    }
    let saved = std::fs::read_to_string(dir.join("args")).map_err(|e| format!("{}: {e}", dir.join("args").display()))?;
    let (_, latest) = checkpoints(&dir).pop().ok_or_else(|| format!("{} holds no checkpoint to resume from", dir.display()))?;
    let threads = arg(args, "threads=");
    let mut out: Vec<String> = Vec::new();
    for a in saved.lines() {
        let a = a.trim_end();
        let dropped = drop.iter().any(|d| a == *d || a.starts_with(d))
            || ["load=", "run=", "resumed=", "resume="].iter().any(|d| a.starts_with(d))
            || (threads.is_some() && a.starts_with("threads="));
        if !dropped {
            out.push(a.to_string());
        }
    }
    if let Some(t) = threads {
        out.push(format!("threads={t}"));
    }
    out.push(format!("load={}", latest.display()));
    out.push(format!("run={}", dir.display()));
    out.push("resumed=1".into());
    let _ = std::fs::remove_file(dir.join("STOP"));
    let _ = std::fs::remove_file(dir.join("PAUSED"));
    Ok(out)
}

/// Whether these arguments continue a run ([`resume_args`] marks them): a tool skips
/// whatever it does once at an arm's start, such as introducing animals.
pub fn resumed(args: &[String]) -> bool {
    arg(args, "resumed=") == Some("1")
}

/// How a staged run ended, when the runner says stop.
#[derive(Clone, Debug, PartialEq)]
pub enum Ended {
    /// Reached its end minute (`DONE`).
    Done,
    /// A `STOP` file paused it at a boundary (`PAUSED`); `resume=DIR` continues it.
    Paused,
    /// A check failed (`FAILED`, with the reason).
    Failed(String),
}

/// What one check window watches: the first segment after a load.
pub struct Window {
    start: HashMap<u64, (Founder, Pose)>,
    last: HashMap<u64, Pose>,
    path_m: [f64; Founder::COUNT],
    moved: HashMap<u64, ()>,
    awake: [bool; Founder::COUNT],
    flora: serde_json::Value,
    fauna: serde_json::Value,
    bites: [[u64; Food::COUNT]; Founder::COUNT],
    /// Awake ticks with the eating latch on, per lineage, at the window's start: a lineage
    /// that never wanted food in the window is not asked to have bitten a plant.
    wanting: [u64; Founder::COUNT],
    /// Each lineage's bodies by state, sampled once a second: what a frozen lineage was
    /// doing instead.
    states: [std::collections::BTreeMap<String, u64>; Founder::COUNT],
    /// FROZEN-PREY: each lineage's sampled body-seconds standing in `State::Resting` with
    /// the eating or the emergency latch on: hungry, so not a rest it chose.
    resting_hungry: [u64; Founder::COUNT],
    /// FROZEN-PREY: per founder body, its lineage, its samples in the window and those in
    /// which it was [`hungry_held`]: hungry at every one is a body frozen by a bug.
    held: HashMap<u64, (Founder, u64, u64)>,
    ticks: u64,
}

/// **Hungry and held still** at a sample (FROZEN-PREY): awake and not resting by choice
/// (no den rest, not hunkered, perched or carried), the eating or the emergency latch on,
/// not biting (`State::Cropping`), and still for at least the last second. A flee layer
/// that never lets go, or a walk that never goes, holds a body like this.
pub(crate) fn hungry_held(a: &cubarium_voxel_fauna::Animal) -> bool {
    a.state != State::Asleep
        && a.state != State::Cropping
        && a.rest.is_none()
        && a.attached.is_none()
        && !a.hunkered
        && a.mobility.mode != cubarium_voxel_fauna::MobilityMode::Perched
        && (a.eating || a.emergency)
        && a.still_ticks >= u64::from(TICK_HZ)
}

impl Window {
    pub fn open(sim: &Sim) -> Window {
        let v = sim.fauna().view();
        let mut start = HashMap::new();
        let mut last = HashMap::new();
        for a in v.animals {
            if let Some(f) = a.founder {
                start.insert(a.id, (f, a.pose));
                last.insert(a.id, a.pose);
            }
        }
        let mut w = Window {
            start,
            last,
            path_m: [0.0; Founder::COUNT],
            moved: HashMap::new(),
            awake: [false; Founder::COUNT],
            flora: counters_of(sim.flora().view().ledger),
            fauna: counters_of(v.ledger),
            bites: v.ledger.bites_by_founder_food,
            wanting: v.ledger.wanting_ticks_by_founder,
            states: std::array::from_fn(|_| std::collections::BTreeMap::new()),
            resting_hungry: [0; Founder::COUNT],
            held: HashMap::new(),
            ticks: 0,
        };
        w.watch(sim);
        w
    }

    /// One tick's movement: each founder body's step along its path, and whether it is
    /// awake.
    pub fn watch(&mut self, sim: &Sim) {
        let voxel_m = sim.world().config().voxel_m;
        let ring_m = f64::from(sim.world().config().width) * voxel_m;
        let sample = self.ticks % u64::from(TICK_HZ) == 0;
        self.ticks += 1;
        for a in sim.fauna().view().animals {
            let Some(f) = a.founder else { continue };
            if sample {
                *self.states[f.index()].entry(format!("{:?}", a.state)).or_default() += 1;
                if a.state == State::Resting && (a.eating || a.emergency) {
                    self.resting_hungry[f.index()] += 1;
                }
                let h = self.held.entry(a.id).or_insert((f, 0, 0));
                h.1 += 1;
                h.2 += u64::from(hungry_held(a));
            }
            if a.state != State::Asleep {
                self.awake[f.index()] = true;
            }
            if let Some(prev) = self.last.insert(a.id, a.pose) {
                self.path_m[f.index()] += pose_distance(prev, a.pose, ring_m);
            }
            if let Some((_, p0)) = self.start.get(&a.id)
                && pose_distance(*p0, a.pose, ring_m) > MOVED_MIN_VOXELS * voxel_m
            {
                self.moved.insert(a.id, ());
            }
        }
    }

    /// The share of a lineage's sampled body-seconds it was not asleep (0 if never seen
    /// awake).
    fn awake_share(&self, f: Founder) -> f64 {
        if !self.awake[f.index()] {
            return 0.0;
        }
        let seen: u64 = self.states[f.index()].values().sum();
        let asleep = self.states[f.index()].get("Asleep").copied().unwrap_or(0);
        1.0 - asleep as f64 / seen.max(1) as f64
    }

    /// The share of a lineage's sampled body-seconds spent asleep or resting.
    fn idle_share(&self, f: Founder) -> f64 {
        let s = &self.states[f.index()];
        let seen: u64 = s.values().sum();
        let idle: u64 = ["Asleep", "Resting"].iter().map(|k| s.get(*k).copied().unwrap_or(0)).sum();
        idle as f64 / seen.max(1) as f64
    }

    /// FROZEN-PREY: the lineage's bodies that were [`hungry_held`] at every sample of the
    /// window (two at least): frozen by a behaviour, whatever their state says.
    fn frozen_bodies(&self, f: Founder) -> usize {
        self.held.values().filter(|&&(g, n, h)| g == f && n >= 2 && h == n).count()
    }

    /// Idle by choice: asleep or resting for all but [`AWAKE_SHARE_MIN`] of the window
    /// (a pack lying up at its den by day) with every body of the lineage still driven. A
    /// body with no controller is never idle: it is the frozen-world failure. FROZEN-PREY:
    /// nor is a lineage whose resting bodies were hungry (eating or emergency latch on) for
    /// most of their resting samples, or with a body hungry and held still the whole window.
    fn idle_by_choice(&self, f: Founder, fauna: &cubarium_voxel_fauna::Fauna) -> Option<f64> {
        let idle = self.idle_share(f);
        let driven = fauna
            .view()
            .animals
            .iter()
            .filter(|a| a.founder == Some(f))
            .all(|a| fauna.has_controller(a.id));
        let resting = self.states[f.index()].get("Resting").copied().unwrap_or(0);
        let fed_rest = 2 * self.resting_hungry[f.index()] <= resting;
        (idle > 1.0 - AWAKE_SHARE_MIN && driven && fed_rest && self.frozen_bodies(f) == 0).then_some(idle)
    }

    /// The full check over the window, as one verdict per item; `Err` lists the failures.
    pub fn verdict(&self, sim: &Sim, cfg: &StageConfig) -> Result<String, String> {
        let mut fail = Vec::new();
        let mut notes = Vec::new();
        let fauna = sim.fauna();
        let phys = |f: Founder| fauna.config().founder(f).diet;
        let now = fauna.view().ledger.bites_by_founder_food;
        let wanting_now = fauna.view().ledger.wanting_ticks_by_founder;
        // Movement, per lineage present at the window's start: it moved, or (still) it fed.
        let mut bodies = [0usize; Founder::COUNT];
        let mut moved = [0usize; Founder::COUNT];
        for (id, (f, _)) in &self.start {
            bodies[f.index()] += 1;
            if self.moved.contains_key(id) {
                moved[f.index()] += 1;
            }
        }
        for f in Founder::ALL {
            let n = bodies[f.index()];
            if n == 0 {
                continue;
            }
            if cfg.still.contains(&f) {
                notes.push(format!("{} still-exempt", f.name()));
                continue;
            }
            let awake_share = self.awake_share(f);
            if awake_share < AWAKE_SHARE_MIN {
                notes.push(format!("{} asleep ({:.0}% awake)", f.name(), 100.0 * awake_share));
                continue;
            }
            if let Some(idle) = self.idle_by_choice(f, fauna) {
                notes.push(format!("{} idle ({:.0}% asleep or resting), driven", f.name(), 100.0 * idle));
                continue;
            }
            let need = n.min(2);
            let path = self.path_m[f.index()];
            let ate: u64 = Food::ALL.iter().map(|food| now[f.index()][food.index()] - self.bites[f.index()][food.index()]).sum();
            let frozen = self.frozen_bodies(f);
            if frozen > 0 {
                notes.push(format!("{} {frozen}/{n} bodies frozen (hungry and held still all window)", f.name()));
            }
            if path > 0.0 && moved[f.index()] >= need {
                notes.push(format!("{} moved {path:.1} m, {}/{n} bodies off their start", f.name(), moved[f.index()]));
            } else if ate > 0 {
                // Still but feeding: a sated browser cropping one crown. A body with no
                // controller cannot feed, so this is not the frozen-world failure.
                notes.push(format!("{} stayed put but took {ate} bites", f.name()));
            } else {
                let seen: u64 = self.states[f.index()].values().sum();
                let states: Vec<String> = self.states[f.index()]
                    .iter()
                    .map(|(k, c)| format!("{k} {:.0}%", 100.0 * *c as f64 / seen.max(1) as f64))
                    .collect();
                fail.push(format!(
                    "{} frozen: {path:.2} m walked, {}/{n} bodies off their start (need {need}), no bites; states {}",
                    f.name(),
                    moved[f.index()],
                    states.join(" ")
                ));
            }
        }
        // Grazing: the lineages that take stand tissue, together.
        let stand_foods: Vec<Food> = Food::ALL.into_iter().filter(|f| !f.is_pool()).collect();
        let mut grazers = Vec::new();
        let mut grazed = 0u64;
        for f in Founder::ALL {
            if bodies[f.index()] == 0
                || !phys(f).eats_stands()
                || self.awake_share(f) < AWAKE_SHARE_MIN
                || self.idle_by_choice(f, fauna).is_some()
            {
                continue;
            }
            if wanting_now[f.index()] == self.wanting[f.index()] {
                notes.push(format!("{} sated (never wanted food)", f.name()));
                continue;
            }
            let b: u64 = stand_foods.iter().map(|food| now[f.index()][food.index()] - self.bites[f.index()][food.index()]).sum();
            grazers.push(format!("{} {b}", f.name()));
            grazed += b;
        }
        if !grazers.is_empty() {
            if grazed > 0 {
                notes.push(format!("plant bites: {}", grazers.join(", ")));
            } else {
                fail.push(format!("no grazer bit a plant ({})", grazers.join(", ")));
            }
        }
        // The arm's own mechanisms.
        let flora_now = counters_of(sim.flora().view().ledger);
        let fauna_now = counters_of(fauna.view().ledger);
        for name in &cfg.expect {
            match counter_delta(name, (&self.flora, &flora_now), (&self.fauna, &fauna_now)) {
                Ok(d) if d > 0.0 => notes.push(format!("{name} +{d}")),
                Ok(_) => fail.push(format!("{name} did not move")),
                Err(e) => fail.push(e),
            }
        }
        if fail.is_empty() { Ok(notes.join("; ")) } else { Err(fail.join("; ")) }
    }
}

/// A lineage awake for less of the window than this (body-seconds) is not asked to have
/// moved: a diurnal lineage checked at night. A check threshold, not a model number; a
/// body with no controller is caught by [`crate::saved::driven`] whatever its phase.
const AWAKE_SHARE_MIN: f64 = 0.25;

/// A body counts as having moved once it stands this many voxels from where the window
/// found it. A check threshold, not a model number.
const MOVED_MIN_VOXELS: f64 = 0.5;

/// The distance between two poses, metres, the ring (`ring_m` around) wrapped in `x`.
fn pose_distance(a: Pose, b: Pose, ring_m: f64) -> f64 {
    let dx = (a.x - b.x).abs();
    let dx = dx.min(ring_m - dx);
    let dy = a.y_m - b.y_m;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn counters_of<T: Serialize>(ledger: &T) -> serde_json::Value {
    serde_json::to_value(ledger).unwrap_or(serde_json::Value::Null)
}

/// A ledger value as one number: an array is summed, at any depth.
fn total(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::Array(a) => a.iter().map(total).sum(),
        _ => None,
    }
}

/// How far the named counter moved between two ledger readings: `flora.NAME`,
/// `fauna.NAME`, or a bare `NAME` found in either (the plant ledger first).
pub fn counter_delta(
    name: &str,
    flora: (&serde_json::Value, &serde_json::Value),
    fauna: (&serde_json::Value, &serde_json::Value),
) -> Result<f64, String> {
    let (layers, field): (Vec<(&serde_json::Value, &serde_json::Value)>, &str) = if let Some(f) = name.strip_prefix("flora.") {
        (vec![flora], f)
    } else if let Some(f) = name.strip_prefix("fauna.") {
        (vec![fauna], f)
    } else {
        (vec![flora, fauna], name)
    };
    for (before, after) in layers {
        if let (Some(b), Some(a)) = (before.get(field).and_then(total), after.get(field).and_then(total)) {
            return Ok(a - b);
        }
    }
    Err(format!("expect={name}: no ledger counter by that name"))
}

/// Check that every founder body has its driver and the ledgers conserve within `tol`.
pub fn instant_check(sim: &Sim, tol: f64) -> Result<(), String> {
    let mut why = Vec::new();
    if let Err(e) = crate::saved::driven(sim.fauna()) {
        why.push(e);
    }
    let (w, fl, fa) = sim.layers();
    if let Some(e) = Residuals::of(w, fl, fa).out_of(tol) {
        why.push(e);
    }
    if why.is_empty() { Ok(()) } else { Err(why.join("; ")) }
}

/// **The staged run**: told every tick and every sim minute, it checks, checkpoints,
/// reports and says when to stop.
pub struct Runner {
    cfg: StageConfig,
    start_minute: u64,
    end_minute: u64,
    check_end: u64,
    window: Option<Window>,
    last_check: String,
    wall: Instant,
    minute_wall: Instant,
    ms_per_tick: f64,
    water_every: u32,
}

impl Runner {
    /// Start (or continue) the run in `cfg.dir` on a world just loaded or founded at
    /// `minute`. `args` are recorded for [`resume_args`]; `end_minute` is the run's end,
    /// unless the directory already records one (a resumed run keeps its own). Runs the
    /// load check; `Err` (with `FAILED` written) when it fails.
    pub fn start(cfg: StageConfig, args: &[String], sim: &mut Sim, minute: u64, end_minute: u64) -> Result<Runner, String> {
        let sim: &Sim = sim;
        std::fs::create_dir_all(cfg.dir.join("ckpt")).map_err(|e| format!("{}: {e}", cfg.dir.display()))?;
        for f in ["DONE", "PAUSED", "FAILED"] {
            let _ = std::fs::remove_file(cfg.dir.join(f));
        }
        let end_file = cfg.dir.join("end_minute");
        let end_minute = if resumed(args) {
            std::fs::read_to_string(&end_file).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(end_minute)
        } else {
            let keep: Vec<&str> = args.iter().skip(1).map(String::as_str).filter(|a| !a.starts_with("resumed=")).collect();
            let mut text = args.first().cloned().unwrap_or_default();
            for a in keep {
                text.push('\n');
                text.push_str(a);
            }
            std::fs::write(cfg.dir.join("args"), text + "\n").map_err(|e| e.to_string())?;
            std::fs::write(&end_file, end_minute.to_string()).map_err(|e| e.to_string())?;
            end_minute
        };
        let check_end = (minute + cfg.check_min).min(end_minute);
        let mut runner = Runner {
            start_minute: minute,
            end_minute,
            check_end,
            window: None,
            last_check: "load: pending".into(),
            wall: Instant::now(),
            minute_wall: Instant::now(),
            ms_per_tick: 0.0,
            water_every: sim.world().water_every(),
            cfg,
        };
        match instant_check(sim, runner.cfg.residual_tol) {
            Ok(()) => runner.last_check = format!("load m{minute}: driven, ledgers conserve"),
            Err(why) => {
                let why = format!("load m{minute}: {why}");
                runner.fail(minute, &why);
                return Err(why);
            }
        }
        runner.window = Some(Window::open(sim));
        runner.status(minute, "running");
        Ok(runner)
    }

    /// The minute the run ends at (a resumed run's own).
    pub fn end_minute(&self) -> u64 {
        self.end_minute
    }

    /// The run directory.
    pub fn dir(&self) -> &Path {
        &self.cfg.dir
    }

    /// After every tick.
    pub fn tick(&mut self, sim: &Sim) {
        if let Some(w) = self.window.as_mut() {
            w.watch(sim);
        }
    }

    /// After the last tick of sim minute `minute`. `Some` means stop: the run ended.
    pub fn minute(&mut self, sim: &Sim, minute: u64) -> Option<Ended> {
        let elapsed = self.minute_wall.elapsed().as_secs_f64();
        self.ms_per_tick = elapsed * 1e3 / self.cfg.ticks_per_min as f64;
        self.minute_wall = Instant::now();
        let check_due = minute >= self.check_end && self.window.is_some();
        let boundary = minute == self.check_end
            || (minute > self.check_end && (minute - self.check_end) % self.cfg.segment_min == 0)
            || minute >= self.end_minute;
        let mut failure = None;
        if check_due {
            let window = self.window.take().expect("a window is open");
            let verdict = window
                .verdict(sim, &self.cfg)
                .and_then(|ok| instant_check(sim, self.cfg.residual_tol).map(|()| ok));
            match verdict {
                Ok(notes) => {
                    self.last_check = format!("check m{}-{minute} ok: {notes}", self.start_minute)
                }
                Err(why) => {
                    failure = Some(format!("check m{}-{minute}: {why}", self.start_minute));
                }
            }
        } else if boundary {
            if let Err(why) = instant_check(sim, self.cfg.residual_tol) {
                failure = Some(format!("boundary m{minute}: {why}"));
            }
        }
        if (boundary || failure.is_some())
            && let Err(why) = self.checkpoint(sim, minute)
        {
            failure = Some(match failure {
                Some(previous) => format!("{previous}; {why}"),
                None => why,
            });
        }
        if let Some(why) = failure {
            self.fail(minute, &why);
            return Some(Ended::Failed(why));
        }
        if minute >= self.end_minute {
            self.status(minute, "done");
            let _ = std::fs::write(self.cfg.dir.join("DONE"), format!("{minute}\n"));
            return Some(Ended::Done);
        }
        if boundary && self.cfg.dir.join("STOP").exists() {
            self.status(minute, "paused");
            let _ = std::fs::write(self.cfg.dir.join("PAUSED"), format!("{minute}\n"));
            return Some(Ended::Paused);
        }
        self.status(minute, "running");
        None
    }

    fn checkpoint(&self, sim: &Sim, minute: u64) -> Result<(), String> {
        let dir = self.cfg.dir.join("ckpt").join(format!("m{minute:06}"));
        let (w, fl, fa) = sim.layers();
        crate::saved::save(&dir, w, fl, fa, minute)
            .map_err(|e| format!("checkpoint {} failed: {e}", dir.display()))?;
        let all = checkpoints(&self.cfg.dir);
        for (_, old) in all.iter().take(all.len().saturating_sub(self.cfg.keep)) {
            let _ = std::fs::remove_dir_all(old);
        }
        Ok(())
    }

    fn fail(&mut self, minute: u64, why: &str) {
        eprintln!("staged: FAILED {why}");
        self.last_check = format!("FAILED {why}");
        let _ = std::fs::write(self.cfg.dir.join("FAILED"), format!("{why}\n"));
        self.status(minute, "failed");
    }

    fn status(&self, minute: u64, state: &str) {
        let sim_s = ((minute - self.start_minute) * self.cfg.ticks_per_min) as f64 / f64::from(TICK_HZ);
        let wall_s = self.wall.elapsed().as_secs_f64().max(1e-9);
        let line = format!(
            "minute={minute} end={} speed={:.2}x ms_tick={:.2} water_every={} state={state} check={}\n",
            self.end_minute,
            sim_s / wall_s,
            self.ms_per_tick,
            self.water_every,
            self.last_check.replace('\n', " ")
        );
        let tmp = self.cfg.dir.join("status.tmp");
        if std::fs::write(&tmp, line).is_ok() {
            let _ = std::fs::rename(&tmp, self.cfg.dir.join("status"));
        }
    }
}

/// A run's stepping, staged or not: `steps` one tick at a time from `minute0` to the
/// end (the runner's, when staged), calling `each_tick` after every tick and `each_minute`
/// after every sim minute. Returns how it ended (`Done` for an unstaged run).
pub fn drive(
    sim: &mut Sim,
    runner: &mut Option<Runner>,
    minute0: u64,
    end_minute: u64,
    mut each_tick: impl FnMut(&mut Sim),
    mut each_minute: impl FnMut(&mut Sim, u64),
) -> Ended {
    let end = runner.as_ref().map_or(end_minute, Runner::end_minute);
    let per_min = runner.as_ref().map_or(TICKS_PER_MIN, |r| r.cfg.ticks_per_min);
    for minute in minute0 + 1..=end {
        for _ in 0..per_min {
            sim.step();
            each_tick(sim);
            if let Some(r) = runner.as_mut() {
                r.tick(sim);
            }
        }
        each_minute(sim, minute);
        if let Some(r) = runner.as_mut()
            && let Some(ended) = r.minute(sim, minute)
        {
            return ended;
        }
    }
    Ended::Done
}

#[cfg(test)]
#[path = "staged_tests.rs"]
mod tests;
