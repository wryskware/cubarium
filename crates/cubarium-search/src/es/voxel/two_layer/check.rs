//! **A brain's check** (BRAIN-FIX step 4; `design/behaviour-ai-2026-10-11.md` §10.4,
//! "Readouts every check"): one lineage's brain beside its heuristic, on matched starts.
//!
//! - **Energy on matched seeds**: each seed's world founded once per arm from one seed, the
//!   lineage on its heuristic or on the brain, every other body as founded; the lineage's
//!   energy (bodies and stores) at the start and end, its deaths and bodies, and the share of
//!   its body-seconds with the eating latch on (whether the window gave it anything to win).
//! - **Holds**: from each arm's own intent records (the brain's hold, or the rules'), per
//!   intent the share of decision samples held, the holds that ended and their mean length,
//!   and the switches per body-minute.
//! - **Per-mode competence** on the lineage's staged scenes ([`super::scenes::competence`]),
//!   both arms on identical starts.
//!
//! Readouts only: nothing here touches a training world, an optimizer or a random stream.

use std::collections::BTreeMap;

use cubarium_voxel_fauna::Founder;
use cubarium_voxel_sim::Sim;
use cubarium_voxel_sim::mode_scenes::{self, ModeScene};
use serde::Serialize;

use crate::es::voxel::EpisodeDriver;

/// What a check runs (`--check-*` on `voxel-live-train`; the `brain_check` example).
#[derive(Clone, Debug, PartialEq)]
pub struct CheckSpec {
    /// Run a check after every this many generations (and on the starting centre).
    pub every: u32,
    /// The matched live seeds (founding seed bases).
    pub seeds: Vec<u64>,
    /// Simulated seconds per live arm.
    pub seconds: u64,
    /// The staged scenes' seeds.
    pub scene_seeds: Vec<u64>,
    /// Worker threads (one world or scene at a time each).
    pub threads: usize,
}

/// One arm on one live seed.
#[derive(Clone, Debug, Serialize)]
pub struct LiveArm {
    pub seed: u64,
    pub bodies_start: usize,
    pub bodies_end: usize,
    pub deaths: u64,
    pub energy_start: f64,
    pub energy_end: f64,
    /// The share of the lineage's body-seconds with its eating latch on.
    pub hungry_share: f64,
}

/// One intent's holds in one arm.
#[derive(Clone, Debug, Default, Serialize)]
pub struct IntentHolds {
    pub intent: String,
    /// Share of the lineage's decision samples that held it.
    pub share: f64,
    /// Holds of it that ended inside the run, and their mean length, seconds.
    pub ended: usize,
    pub mean_hold_s: f64,
}

/// One arm's holds over every live seed.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Holds {
    pub decision_samples: usize,
    pub switches_per_body_min: f64,
    pub intents: Vec<IntentHolds>,
}

/// One scene's competence, both arms.
#[derive(Clone, Debug, Serialize)]
pub struct SceneCheck {
    pub scene: &'static str,
    pub intent: &'static str,
    pub heuristic: f64,
    pub brain: f64,
    pub mode_share_heuristic: f64,
    pub mode_share_brain: f64,
}

/// One check of one lineage's brain.
#[derive(Clone, Debug, Serialize)]
pub struct CheckReport {
    pub lineage: String,
    pub generation: u32,
    pub heuristic: Vec<LiveArm>,
    pub brain: Vec<LiveArm>,
    /// Mean over the matched seeds of the lineage's energy change per starting body, each
    /// arm, and the brain's minus the heuristic's (paired by seed): the "beats its
    /// heuristic" read.
    pub gain_per_body_heuristic: f64,
    pub gain_per_body_brain: f64,
    pub gain_per_body_brain_minus_heuristic: f64,
    pub deaths_heuristic: u64,
    pub deaths_brain: u64,
    pub holds_heuristic: Holds,
    pub holds_brain: Holds,
    pub scenes: Vec<SceneCheck>,
    /// Seeds or scenes the founding refused, with the reason.
    pub skipped: Vec<String>,
    pub wall_s: f64,
}

fn lineage(sim: &Sim, f: Founder) -> (usize, f64, u64) {
    let v = sim.fauna().view();
    let body: f64 = v.animals.iter().filter(|a| a.founder == Some(f)).map(|a| a.body).sum();
    let energy = body + crate::es::voxel::live::balance::stores(v.animals, sim.fauna().config(), f);
    let deaths = v.ledger.deaths_by_founder_cause[f.index()].iter().sum();
    (v.animals.iter().filter(|a| a.founder == Some(f)).count(), energy, deaths)
}

/// Per-body hold bookkeeping across one arm.
#[derive(Default)]
struct HoldTally {
    samples: Vec<usize>,
    ended: Vec<usize>,
    ended_s: Vec<f64>,
    switches: usize,
    body_seconds: f64,
    current: BTreeMap<u64, (u8, u64)>,
}

impl HoldTally {
    fn new(menu: usize) -> HoldTally {
        HoldTally { samples: vec![0; menu], ended: vec![0; menu], ended_s: vec![0.0; menu], ..Default::default() }
    }

    fn observe(&mut self, sim: &Sim, f: Founder) {
        let fauna = sim.fauna();
        let now = fauna.tick();
        for a in fauna.view().animals.iter().filter(|a| a.founder == Some(f)) {
            if a.intent.sample_tick != now {
                continue;
            }
            let held = a.intent.held;
            if let Some(s) = self.samples.get_mut(usize::from(held)) {
                *s += 1;
            }
            match self.current.get(&a.id).copied() {
                Some((was, since)) if was != held => {
                    self.switches += 1;
                    if let (Some(n), Some(t)) = (self.ended.get_mut(usize::from(was)), self.ended_s.get_mut(usize::from(was))) {
                        *n += 1;
                        *t += now.saturating_sub(since) as f64 / f64::from(cubarium_voxel_fauna::TICK_HZ);
                    }
                    self.current.insert(a.id, (held, now));
                }
                Some(_) => {}
                None => {
                    self.current.insert(a.id, (held, now));
                }
            }
        }
    }

    fn finish(&self, menu: &cubarium_voxel_fauna::Menu) -> Holds {
        let total: usize = self.samples.iter().sum();
        Holds {
            decision_samples: total,
            switches_per_body_min: if self.body_seconds > 0.0 { self.switches as f64 / (self.body_seconds / 60.0) } else { 0.0 },
            intents: (0..menu.named.len())
                .map(|k| IntentHolds {
                    intent: menu.name(k).to_string(),
                    share: if total > 0 { self.samples[k] as f64 / total as f64 } else { 0.0 },
                    ended: self.ended[k],
                    mean_hold_s: if self.ended[k] > 0 { self.ended_s[k] / self.ended[k] as f64 } else { 0.0 },
                })
                .collect(),
        }
    }
}

/// Run one arm on one founded world: the lineage on `driver` (its heuristic when `None`).
fn live_arm(mut sim: Sim, seed: u64, f: Founder, driver: Option<&EpisodeDriver>, seconds: u64, holds: &mut HoldTally) -> LiveArm {
    if let Some(d) = driver {
        crate::es::voxel::live::install_centres(sim.fauna_mut(), &[(f, d.clone())]);
    }
    let (bodies_start, energy_start, deaths_start) = lineage(&sim, f);
    let hz = u64::from(cubarium_voxel_fauna::TICK_HZ);
    let (mut hungry, mut present) = (0usize, 0usize);
    for t in 0..seconds * hz {
        sim.step();
        holds.observe(&sim, f);
        if t % hz == 0 {
            for a in sim.fauna().view().animals.iter().filter(|a| a.founder == Some(f)) {
                present += 1;
                hungry += usize::from(a.eating);
            }
        }
    }
    holds.body_seconds += present as f64;
    let (bodies_end, energy_end, deaths_end) = lineage(&sim, f);
    LiveArm {
        seed,
        bodies_start,
        bodies_end,
        deaths: deaths_end.saturating_sub(deaths_start),
        energy_start,
        energy_end,
        hungry_share: if present > 0 { hungry as f64 / present as f64 } else { 0.0 },
    }
}

/// **Check `driver` against `f`'s heuristic.** `found(seed)` founds one live world (the
/// caller's training world; `Err` for a refused seed), called once per arm and seed.
pub fn check(
    f: Founder,
    driver: &EpisodeDriver,
    generation: u32,
    spec: &CheckSpec,
    found: &(dyn Fn(u64) -> Result<Sim, String> + Sync),
) -> CheckReport {
    let started = std::time::Instant::now();
    let menu = cubarium_voxel_fauna::menu_of(f);
    let mut skipped = Vec::new();
    // Live arms: every (seed, arm) on its own worker.
    let jobs: Vec<(u64, bool)> = spec.seeds.iter().flat_map(|&s| [(s, false), (s, true)]).collect();
    let runs: Vec<Result<(LiveArm, HoldTally), String>> = crate::es::voxel::live::trainer::parallel(jobs.clone(), spec.threads, |(seed, brain)| {
        let sim = found(seed)?;
        let mut holds = HoldTally::new(menu.named.len());
        let arm = live_arm(sim, seed, f, brain.then_some(driver), spec.seconds, &mut holds);
        Ok((arm, holds))
    });
    let (mut h_arms, mut b_arms) = (Vec::new(), Vec::new());
    let (mut h_holds, mut b_holds) = (HoldTally::new(menu.named.len()), HoldTally::new(menu.named.len()));
    for ((seed, brain), r) in jobs.into_iter().zip(runs) {
        match r {
            Ok((arm, holds)) => {
                let into = if brain { &mut b_holds } else { &mut h_holds };
                for k in 0..menu.named.len() {
                    into.samples[k] += holds.samples[k];
                    into.ended[k] += holds.ended[k];
                    into.ended_s[k] += holds.ended_s[k];
                }
                into.switches += holds.switches;
                into.body_seconds += holds.body_seconds;
                if brain { b_arms.push(arm) } else { h_arms.push(arm) }
            }
            Err(why) => skipped.push(format!("seed {seed} ({}): {why}", if brain { "brain" } else { "heuristic" })),
        }
    }
    let gain = |a: &LiveArm| (a.energy_end - a.energy_start) / a.bodies_start.max(1) as f64;
    let mean = |xs: Vec<f64>| if xs.is_empty() { f64::NAN } else { xs.iter().sum::<f64>() / xs.len() as f64 };
    let paired: Vec<f64> = b_arms
        .iter()
        .filter_map(|b| h_arms.iter().find(|h| h.seed == b.seed).map(|h| gain(b) - gain(h)))
        .collect();
    // Scenes: both arms on identical starts.
    let cfg = mode_scenes::Config::default();
    let scene_jobs: Vec<(ModeScene, u64)> = ModeScene::of(f).into_iter().flat_map(|m| spec.scene_seeds.iter().map(move |&s| (m, s))).collect();
    let got = crate::es::voxel::live::trainer::parallel(scene_jobs.clone(), spec.threads, |(m, s)| {
        Ok::<_, String>((super::scenes::competence(&cfg, m, s, None)?, super::scenes::competence(&cfg, m, s, Some(driver))?))
    });
    let mut scenes = Vec::new();
    for m in ModeScene::of(f) {
        let runs: Vec<_> = scene_jobs
            .iter()
            .zip(&got)
            .filter(|((sm, _), _)| *sm == m)
            .filter_map(|((_, s), r)| match r {
                Ok(v) => Some(v.clone()),
                Err(why) => {
                    skipped.push(format!("scene {} seed {s}: {why}", m.name()));
                    None
                }
            })
            .collect();
        scenes.push(SceneCheck {
            scene: m.name(),
            intent: m.intent(),
            heuristic: mean(runs.iter().map(|(h, _)| h.competence).collect()),
            brain: mean(runs.iter().map(|(_, b)| b.competence).collect()),
            mode_share_heuristic: mean(runs.iter().map(|(h, _)| h.mode_share).collect()),
            mode_share_brain: mean(runs.iter().map(|(_, b)| b.mode_share).collect()),
        });
    }
    CheckReport {
        lineage: f.name().to_string(),
        generation,
        gain_per_body_heuristic: mean(h_arms.iter().map(gain).collect()),
        gain_per_body_brain: mean(b_arms.iter().map(gain).collect()),
        gain_per_body_brain_minus_heuristic: mean(paired),
        deaths_heuristic: h_arms.iter().map(|a| a.deaths).sum(),
        deaths_brain: b_arms.iter().map(|a| a.deaths).sum(),
        heuristic: h_arms,
        brain: b_arms,
        holds_heuristic: h_holds.finish(menu),
        holds_brain: b_holds.finish(menu),
        scenes,
        skipped,
        wall_s: started.elapsed().as_secs_f64(),
    }
}

/// One compact console line for a check.
pub fn summary_line(r: &CheckReport) -> String {
    let scenes: Vec<String> = r.scenes.iter().map(|s| format!("{} {:.2}/{:.2}", s.intent, s.brain, s.heuristic)).collect();
    format!(
        "# check {} gen {}: gain/body brain {:+.5} heuristic {:+.5} (paired {:+.5}); deaths {}/{}; hungry {:.2}; switches/body-min {:.2}/{:.2}; scenes brain/rules {}",
        r.lineage,
        r.generation,
        r.gain_per_body_brain,
        r.gain_per_body_heuristic,
        r.gain_per_body_brain_minus_heuristic,
        r.deaths_brain,
        r.deaths_heuristic,
        r.brain.iter().map(|a| a.hungry_share).sum::<f64>() / r.brain.len().max(1) as f64,
        r.holds_brain.switches_per_body_min,
        r.holds_heuristic.switches_per_body_min,
        scenes.join(", ")
    )
}
