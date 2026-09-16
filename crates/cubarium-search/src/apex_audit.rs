//! The apex **opportunity** audit: why no two introduced adults ever mate.
//!
//! The calibration screen ran 180 apex-bearing runs and recorded zero matings, zero births,
//! zero emergences and zero final survivors, then named the 10 px mating radius as the reason.
//! The predicate has six other terms. The screen recorded no minimum pair distance, no
//! simultaneously-ready time and no failure count, so early death, never being ready and never
//! meeting are one undivided outcome (`design/7_Research/ecology-v1-next-review-2026-09-15.md`,
//! finding 5 and next step 5). Asking the owner to move a constant on that evidence would be
//! asking them to guess.
//!
//! This command re-runs **exactly the two-apex arm the screen ran** — the same candidate
//! configuration, the same seeds, the same deterministic placements, the same introduction at
//! tick 6,000, never restocked — with the world's own
//! [`cubarium_core::encounter::ApexOpportunity`] counters on, and reports what each predicate
//! actually did.
//!
//! # Reading the result
//!
//! - `ticks_two_ready == 0` in every run: **no radius can matter.** Two adults were never
//!   simultaneously able to reproduce, so the distance between them was never consulted.
//! - `ticks_two_ready > 0` and `ticks_ready_pair_within_radius == 0`: a ready pair existed and
//!   never closed. The radius, or an encounter policy that brings adults together, is then a
//!   real choice, and `min_ready_distance_px` says how large a change would have to be.
//! - `fail_radius > 0` with `ticks_two_ready > 0`: the pass formed candidate pairs and refused
//!   them on distance alone. The same conclusion, reached from the other side.
//!
//! A candidate pair only exists when the two adults sensed each other; the census counts do
//! not require that. Both are reported, because a world where two ready adults never even
//! entered each other's sensing is a different world from one where they did and stayed 12 px
//! apart.
//!
//! # What this run is not
//!
//! It is not a calibration stage, it scores nothing, it writes no `evals.jsonl`, and it moves
//! no parameter. It changes nothing about the arm it re-runs: the counters are read-only
//! diagnostics with a hash test behind them (`cubarium-core/tests/body_budget.rs`).

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use cubarium_core::encounter::ApexOpportunity;
use cubarium_core::hunter::FixedHunterProfile;
use cubarium_core::{OrganismId, World, WorldConfig};
use serde::{Deserialize, Serialize};

use crate::calibrate;
use crate::es::fixture::Ecology;
use crate::evaluate::{self, BUILD_ID};
use crate::search::HELDOUT_SEEDS;

type Boxed = Box<dyn std::error::Error>;

/// How often a running audit re-checks the world's own invariants. The calibration samples
/// every 500 ticks; this run has nothing to sample, so the check is the only reason to stop.
pub const VALIDATE_EVERY: u64 = 5_000;

/// One introduced adult's life in the run that introduced it.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ApexLife {
    pub id: OrganismId,
    pub introduced_tick: u64,
    /// The first tick the id no longer resolved. `None` if it outlived the horizon.
    pub gone_tick: Option<u64>,
    /// Ticks it was alive inside this run.
    pub lived_ticks: u64,
}

/// One `(configuration, seed)` audit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditRow {
    pub config: String,
    pub config_hash: String,
    /// Whether the loaded configuration is bit-for-bit the declared screen candidate of the
    /// same name at this seed. `None` when no candidate carries that name.
    pub matches_screen_candidate: Option<bool>,
    pub seed: u64,
    pub apex_founders: u32,
    pub introduce_tick: u64,
    pub horizon_ticks: u64,
    /// Ticks actually simulated: the horizon, or fewer if the world emptied.
    pub ticks: u64,
    pub collapsed_at: Option<u64>,
    pub apex_material_in: f64,
    pub apex_energy_in: f64,
    pub lives: Vec<ApexLife>,
    pub opportunity: ApexOpportunity,
    /// The world's own conservation residuals at the end, so a run that drifted is visible.
    pub mass_residual: f64,
    pub final_population: usize,
    pub elapsed_ms: u64,
    /// The profile terms `hunter::may_reproduce` tests, carried on the row so a reader can see
    /// what "never ready" was measured against without opening the source.
    pub reproduce_min_age_ticks: u64,
    pub reproduce_reserve_fraction: f64,
    pub reproduce_energy_fraction: f64,
    pub mating_radius_px: f64,
}

impl AuditRow {
    /// Did any two ready adults ever stand within the mating radius?
    pub fn ready_pair_ever_met(&self) -> bool {
        self.opportunity.ticks_ready_pair_within_radius > 0
    }

    /// Was there ever a moment at which two adults could both have reproduced?
    pub fn readiness_overlap(&self) -> bool {
        self.opportunity.ticks_two_ready > 0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditReport {
    pub build: String,
    pub configs: Vec<String>,
    pub seeds: Vec<u64>,
    pub apex_founders: u32,
    pub introduce_tick: u64,
    pub horizon_ticks: u64,
    pub workers: usize,
    pub wall_seconds: f64,
    /// The one sentence the audit exists to produce.
    pub verdict: String,
    pub rows: Vec<AuditRow>,
}

/// Run one `(configuration, seed)` arm of the audit.
fn run_one(
    eco: &Ecology,
    seed: u64,
    apex: u32,
    horizon: u64,
    introduce_tick: u64,
) -> Result<AuditRow, String> {
    let start = Instant::now();
    let mut config: WorldConfig = (*eco.base).clone();
    config.seed = seed;
    // The host-side event log is off in every headless run; leaving it on would grow a queue
    // this command never reads over 180,000 ticks.
    config.capacity.event_log = false;
    let hash = calibrate::config_hash(&config);
    // Provenance: is this the declared screen candidate of the same name, at this seed?
    let matches = calibrate::candidate(&eco.label)
        .map(|c| c.config(seed).map(|c| calibrate::config_hash(&c) == hash).unwrap_or(false));

    // The profile is derived from the **base** configuration, exactly as the screen derives it,
    // so the apex genome does not shift under a candidate's parameters.
    let profile = FixedHunterProfile::lanternjaw_trial(&evaluate::base_config(seed));
    profile.validate().map_err(|e| format!("hunter profile rejected: {e}"))?;
    config.validate().map_err(|e| format!("config rejected: {e}"))?;
    let mut world = World::new(config).map_err(|e| format!("world creation refused: {e}"))?;

    let mut lives: Vec<ApexLife> = Vec::new();
    let (mut material_in, mut energy_in) = (0.0, 0.0);
    let mut introduced = false;
    let mut collapsed_at = None;
    let mut ticks = 0;

    for _ in 0..horizon {
        world.step();
        ticks = world.tick();
        // The queues are drained every tick rather than accumulated: this command reads none
        // of them and a 180,000-tick run would otherwise hold every event it ever emitted.
        world.drain_events();
        world.drain_hunter_events();
        world.drain_apex_dormancy_events();
        world.drain_apex_encounter_events();
        world.drain_quiet_events();

        if !introduced && apex > 0 && world.tick() == introduce_tick {
            let targets = evaluate::apex_targets(seed, apex);
            let receipts = world
                .introduce_hunters(profile.clone(), &targets)
                .map_err(|e| format!("apex introduction refused: {e}"))?;
            material_in = receipts.iter().map(|r| r.material_in).sum();
            energy_in = receipts.iter().map(|r| r.energy_in).sum();
            lives = receipts
                .iter()
                .map(|r| ApexLife {
                    id: r.id,
                    introduced_tick: world.tick(),
                    gone_tick: None,
                    lived_ticks: 0,
                })
                .collect();
            introduced = true;
        }
        for life in lives.iter_mut() {
            if life.gone_tick.is_some() {
                continue;
            }
            if world.state.organisms.get(life.id).is_some() {
                life.lived_ticks += 1;
            } else {
                life.gone_tick = Some(world.tick());
            }
        }

        if world.tick().is_multiple_of(VALIDATE_EVERY)
            && let Err(e) = world.check_invariants()
        {
            return Err(format!("invariant violated at tick {}: {e}", world.tick()));
        }
        if world.population() == 0 {
            collapsed_at = Some(world.tick());
            break;
        }
    }

    Ok(AuditRow {
        reproduce_min_age_ticks: (profile.reproduce_min_age_seconds / cubarium_core::DT) as u64,
        reproduce_reserve_fraction: profile.reproduce_reserve_fraction,
        reproduce_energy_fraction: profile.reproduce_energy_fraction,
        mating_radius_px: cubarium_core::encounter::MATING_RADIUS_PX,
        config: eco.label.clone(),
        config_hash: format!("{hash:016x}"),
        matches_screen_candidate: matches,
        seed,
        apex_founders: apex,
        introduce_tick,
        horizon_ticks: horizon,
        ticks,
        collapsed_at,
        apex_material_in: material_in,
        apex_energy_in: energy_in,
        lives,
        opportunity: world.drain_apex_opportunity(),
        mass_residual: world.mass_residual(),
        final_population: world.population(),
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

/// The one sentence the audit produces, from the rows it produced.
fn verdict(rows: &[AuditRow]) -> String {
    let with_two_adults = rows.iter().filter(|r| r.opportunity.ticks_two_adults > 0).count();
    let with_overlap = rows.iter().filter(|r| r.readiness_overlap()).count();
    let with_meeting = rows.iter().filter(|r| r.ready_pair_ever_met()).count();
    let candidates: u64 = rows.iter().map(|r| r.opportunity.pair_candidates).sum();
    let radius_failures: u64 = rows.iter().map(|r| r.opportunity.fail_radius).sum();
    if with_overlap == 0 {
        // When no member is ever ready, say what the readiness gate it never passed actually
        // is. The oldest apex any run produced against the age `may_reproduce` demands is a
        // comparison the rows already carry, and it is the difference between "the radius is
        // the wrong knob" and "the radius is the wrong knob and here is the right one".
        let oldest = rows
            .iter()
            .flat_map(|r| r.lives.iter().map(|l| l.lived_ticks))
            .max()
            .unwrap_or(0);
        let min_age = rows.iter().map(|r| r.reproduce_min_age_ticks).max().unwrap_or(0);
        let age = if min_age > 0 && oldest < min_age {
            format!(
                " No member ever became eligible at all: the oldest apex in any run reached \
                 {oldest} ticks and `may_reproduce` requires {min_age}, so every member died at \
                 {:.0}% of its own minimum reproduction age.",
                100.0 * oldest as f64 / min_age as f64
            )
        } else {
            String::new()
        };
        format!(
            "Readiness overlap is zero in all {} runs: two adults were alive together in {} of \
             them, but never simultaneously able to reproduce, so the {} candidate pair(s) the \
             pass formed never reached the distance test as the deciding term. Changing the 10 \
             px mating radius cannot produce a mating in this arm.{age}",
            rows.len(),
            with_two_adults,
            candidates
        )
    } else if with_meeting == 0 {
        format!(
            "Readiness overlap exists in {with_overlap} of {} runs and no ready pair ever closed \
             to the 10 px mating radius ({radius_failures} candidate pair(s) were refused on \
             distance). The radius, or an encounter policy that brings ready adults together, \
             is a real owner-facing choice here.",
            rows.len()
        )
    } else {
        format!(
            "A ready pair stood inside the mating radius in {with_meeting} of {} runs, so \
             neither readiness nor distance is the whole explanation; read the per-run failure \
             histogram for what refused them.",
            rows.len()
        )
    }
}

/// Run the audit over every configuration and seed and write its record.
#[allow(clippy::too_many_arguments)]
pub fn run(
    configs: Vec<PathBuf>,
    seeds: usize,
    apex: u32,
    horizon: u64,
    introduce_tick: u64,
    workers: usize,
    wall_seconds: u64,
    out: PathBuf,
) -> Result<(), Boxed> {
    if configs.is_empty() {
        return Err("--config must name at least one world configuration TOML".into());
    }
    if seeds == 0 || seeds > HELDOUT_SEEDS.len() {
        return Err(format!("--seeds must be between 1 and {}", HELDOUT_SEEDS.len()).into());
    }
    let ecologies: Vec<Ecology> = configs
        .iter()
        .map(|p| Ecology::load(p))
        .collect::<Result<_, _>>()?;
    let seeds: Vec<u64> = HELDOUT_SEEDS[..seeds].to_vec();

    println!("# apex opportunity audit");
    println!("# build {BUILD_ID}, {apex} adults introduced at tick {introduce_tick}, never restocked");
    println!("# horizon {horizon} ticks, held-out seeds {seeds:?}, {workers} workers");
    for e in &ecologies {
        println!("# config {} (hash {})", e.label, e.hex());
    }

    let jobs: Vec<(usize, u64)> = ecologies
        .iter()
        .enumerate()
        .flat_map(|(i, _)| seeds.iter().map(move |s| (i, *s)))
        .collect();
    let cursor = AtomicUsize::new(0);
    let rows: Mutex<Vec<AuditRow>> = Mutex::new(Vec::new());
    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let started = Instant::now();

    std::thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                loop {
                    let i = cursor.fetch_add(1, Ordering::SeqCst);
                    let Some(&(e, seed)) = jobs.get(i) else { return };
                    if started.elapsed().as_secs() >= wall_seconds {
                        failures
                            .lock()
                            .expect("failures")
                            .push(format!("{}/{seed}: not started inside the wall cap", ecologies[e].label));
                        continue;
                    }
                    match run_one(&ecologies[e], seed, apex, horizon, introduce_tick) {
                        Ok(row) => rows.lock().expect("rows").push(row),
                        Err(err) => failures
                            .lock()
                            .expect("failures")
                            .push(format!("{}/{seed}: {err}", ecologies[e].label)),
                    }
                }
            });
        }
    });

    let failures = failures.into_inner().expect("failures");
    for f in &failures {
        println!("FAILED {f}");
    }
    let mut rows = rows.into_inner().expect("rows");
    rows.sort_by(|a, b| a.config.cmp(&b.config).then(a.seed.cmp(&b.seed)));
    if rows.is_empty() {
        return Err("no audit row completed".into());
    }

    println!();
    println!(
        "{:<12} {:>7} {:>9} {:>9} {:>9} {:>9} {:>10} {:>7} {:>7} {:>7}",
        "config", "seed", "life a", "life b", "2 adults", "2 ready", "min px", "cands", "radius", "ready",
    );
    for r in &rows {
        let life = |i: usize| r.lives.get(i).map_or(0, |l| l.lived_ticks);
        println!(
            "{:<12} {:>7} {:>9} {:>9} {:>9} {:>9} {:>10} {:>7} {:>7} {:>7}",
            r.config,
            r.seed,
            life(0),
            life(1),
            r.opportunity.ticks_two_adults,
            r.opportunity.ticks_two_ready,
            r.opportunity
                .min_ready_distance_px
                .map_or_else(|| ">32".to_string(), |d| format!("{d:.2}")),
            r.opportunity.pair_candidates,
            r.opportunity.fail_radius,
            r.opportunity.fail_ready_a + r.opportunity.fail_ready_b,
        );
    }

    let verdict = verdict(&rows);
    println!();
    println!("{verdict}");
    let report = AuditReport {
        build: BUILD_ID.to_string(),
        configs: ecologies.iter().map(|e| e.label.clone()).collect(),
        seeds,
        apex_founders: apex,
        introduce_tick,
        horizon_ticks: horizon,
        workers,
        wall_seconds: started.elapsed().as_secs_f64(),
        verdict,
        rows,
    };
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&out, serde_json::to_string(&report)?)?;
    println!("wall {:.1} s; wrote {}", report.wall_seconds, out.display());
    if !failures.is_empty() {
        return Err(format!("{} arm(s) did not complete", failures.len()).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(two_adults: u64, two_ready: u64, within: u64, candidates: u64, radius: u64) -> AuditRow {
        AuditRow {
            config: "fast-leaf".into(),
            config_hash: "0".into(),
            matches_screen_candidate: Some(true),
            seed: 1,
            apex_founders: 2,
            introduce_tick: 6_000,
            horizon_ticks: 180_000,
            ticks: 180_000,
            collapsed_at: None,
            apex_material_in: 0.0,
            apex_energy_in: 0.0,
            lives: Vec::new(),
            opportunity: ApexOpportunity {
                ticks_two_adults: two_adults,
                ticks_two_ready: two_ready,
                ticks_ready_pair_within_radius: within,
                pair_candidates: candidates,
                fail_radius: radius,
                ..ApexOpportunity::default()
            },
            mass_residual: 0.0,
            final_population: 0,
            elapsed_ms: 0,
            reproduce_min_age_ticks: 24_000,
            reproduce_reserve_fraction: 0.8,
            reproduce_energy_fraction: 0.75,
            mating_radius_px: 10.0,
        }
    }

    /// The three verdicts are decided by the counters, not by the author: no readiness overlap
    /// means the radius is irrelevant, overlap without a meeting makes it a real choice, and a
    /// meeting means neither term alone explains the outcome.
    #[test]
    fn the_verdict_follows_the_counters() {
        let none = verdict(&[row(500, 0, 0, 3, 3)]);
        assert!(none.contains("cannot produce a mating"), "{none}");

        let overlap = verdict(&[row(500, 400, 0, 3, 3)]);
        assert!(overlap.contains("real owner-facing choice"), "{overlap}");

        let met = verdict(&[row(500, 400, 12, 3, 0)]);
        assert!(met.contains("neither readiness nor distance"), "{met}");
    }

    /// The two row predicates read the counters they claim to read.
    #[test]
    fn the_row_predicates_name_their_counters() {
        let r = row(500, 400, 0, 3, 3);
        assert!(r.readiness_overlap());
        assert!(!r.ready_pair_ever_met());
        let r = row(500, 400, 1, 3, 0);
        assert!(r.ready_pair_ever_met());
    }
}
