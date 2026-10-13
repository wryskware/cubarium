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
//! - **Diet and prey numbers** (topic 6, "prey switching"): per arm, what the lineage ate by
//!   food (each prey lineage's kills and carrion, each plant or pool food class) with its
//!   share by kills and by matter eaten, and every animal lineage's live body count at the
//!   start, each simulated minute and the end. Read from the fauna ledger's own counters
//!   (the hunt funnel's kills, `eaten_by_founder_food`, `eaten_carrion_by_founder_prey`).
//! - **Per-mode competence** on the lineage's staged scenes ([`super::scenes::competence`]),
//!   both arms on identical starts.
//!
//! Readouts only: nothing here touches a training world, an optimizer or a random stream.

use std::collections::BTreeMap;

use cubarium_voxel_fauna::{FaunaLedger, Food, Founder};
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
    /// What the lineage ate over the arm, by food ([`diet`]).
    pub diet: Vec<DietItem>,
    /// The simulated seconds at which `numbers` were read: the start, each minute, the end.
    pub numbers_at_s: Vec<u64>,
    /// Every animal lineage's live founder bodies at `numbers_at_s` (lineages with no body
    /// at any of them left out).
    pub numbers: Vec<LineageCount>,
}

/// One food in a lineage's diet over an arm (or summed over a check's arms).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct DietItem {
    /// A prey lineage's name (its kills and the carrion of its carcasses), `carrion` for
    /// carrion with no carcass record, or a non-animal food class's name (`foliage`,
    /// `litter`, `larva`, ...).
    pub food: String,
    /// Kills of that prey lineage by the lineage (zero for a food class).
    pub kills: u64,
    /// Raw organic matter withdrawn, before bite spillage: the unit the check's energy reads in.
    pub eaten: f64,
    /// This food's share of the lineage's kills, and of its matter eaten (each zero when
    /// the lineage killed or ate nothing; otherwise each sums to 1 over the diet).
    pub kill_share: f64,
    pub eaten_share: f64,
}

/// One lineage's live founder bodies over an arm.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct LineageCount {
    pub lineage: String,
    pub counts: Vec<usize>,
}

/// One lineage's live founder bodies at the start and end, summed over a check's seeds.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct LineageNumbers {
    pub lineage: String,
    pub start: usize,
    pub end: usize,
}

/// **Lineage `f`'s diet between two ledger readings**: per prey lineage its kills (the hunt
/// funnel's killed stage) and the raw carrion of that lineage's carcasses it ate; carrion
/// with no carcass record as `carrion`; every other food class it ate by name. Shares by
/// kills and by matter eaten, each summing to 1 when anything was killed or eaten.
pub fn diet(before: &FaunaLedger, after: &FaunaLedger, f: Founder) -> Vec<DietItem> {
    let i = f.index();
    let killed = cubarium_voxel_fauna::perception::KILLED;
    let mut items = Vec::new();
    let mut carrion_known = 0.0;
    for g in Founder::ALL {
        let kills = after.funnel[i][g.index()][killed].saturating_sub(before.funnel[i][g.index()][killed]);
        let eaten = (after.eaten_carrion_by_founder_prey[i][g.index()] - before.eaten_carrion_by_founder_prey[i][g.index()]).max(0.0);
        carrion_known += eaten;
        if kills > 0 || eaten > 0.0 {
            items.push(DietItem { food: g.name().to_string(), kills, eaten, ..Default::default() });
        }
    }
    for food in Food::ALL {
        let raw = (after.eaten_by_founder_food[i][food.index()] - before.eaten_by_founder_food[i][food.index()]).max(0.0);
        // Carrion already booked to a carcass's lineage is not counted twice; what is left
        // beyond the rounding of the same sums had no carcass record.
        let eaten = if food == Food::Carrion { raw - carrion_known } else { raw };
        if eaten > 1e-9 * raw {
            items.push(DietItem { food: food.name().to_string(), eaten, ..Default::default() });
        }
    }
    shares(&mut items);
    items
}

/// Fill each item's shares of the items' kills and matter eaten.
fn shares(items: &mut [DietItem]) {
    let kills: u64 = items.iter().map(|d| d.kills).sum();
    let eaten: f64 = items.iter().map(|d| d.eaten).sum();
    for d in items {
        d.kill_share = if kills > 0 { d.kills as f64 / kills as f64 } else { 0.0 };
        d.eaten_share = if eaten > 0.0 { d.eaten / eaten } else { 0.0 };
    }
}

/// Several arms' diets summed by food, shares recomputed.
fn sum_diets<'a>(diets: impl IntoIterator<Item = &'a [DietItem]>) -> Vec<DietItem> {
    let mut out: Vec<DietItem> = Vec::new();
    for d in diets.into_iter().flatten() {
        match out.iter_mut().find(|o| o.food == d.food) {
            Some(o) => {
                o.kills += d.kills;
                o.eaten += d.eaten;
            }
            None => out.push(DietItem { food: d.food.clone(), kills: d.kills, eaten: d.eaten, ..Default::default() }),
        }
    }
    shares(&mut out);
    out
}

/// Every lineage's live founder bodies, in [`Founder::ALL`] order.
pub fn lineage_counts(sim: &Sim) -> [usize; Founder::COUNT] {
    let mut n = [0; Founder::COUNT];
    for a in sim.fauna().view().animals {
        if let Some(g) = a.founder {
            n[g.index()] += 1;
        }
    }
    n
}

/// Several arms' numbers summed by lineage at their start and end.
fn sum_numbers<'a>(arms: impl IntoIterator<Item = &'a LiveArm>) -> Vec<LineageNumbers> {
    let mut out: Vec<LineageNumbers> = Vec::new();
    for c in arms.into_iter().flat_map(|a| &a.numbers) {
        let (start, end) = (c.counts.first().copied().unwrap_or(0), c.counts.last().copied().unwrap_or(0));
        match out.iter_mut().find(|o| o.lineage == c.lineage) {
            Some(o) => {
                o.start += start;
                o.end += end;
            }
            None => out.push(LineageNumbers { lineage: c.lineage.clone(), start, end }),
        }
    }
    out
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
    /// The lineage's diet summed over the matched seeds, each arm ([`diet`]): the topic 6
    /// watch on a brain overeating one prey.
    pub diet_heuristic: Vec<DietItem>,
    pub diet_brain: Vec<DietItem>,
    /// Every animal lineage's live founder bodies at each arm's start and end, summed over
    /// the matched seeds (per-minute counts are on each [`LiveArm`]).
    pub numbers_heuristic: Vec<LineageNumbers>,
    pub numbers_brain: Vec<LineageNumbers>,
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
    let ledger_start = *sim.fauna().view().ledger;
    let mut numbers_at_s = vec![0];
    let mut counts = vec![lineage_counts(&sim)];
    let hz = u64::from(cubarium_voxel_fauna::TICK_HZ);
    let (mut hungry, mut present) = (0usize, 0usize);
    for t in 0..seconds * hz {
        sim.step();
        holds.observe(&sim, f);
        let s = (t + 1) / hz;
        if (t + 1) % hz == 0 && s % 60 == 0 && s < seconds {
            numbers_at_s.push(s);
            counts.push(lineage_counts(&sim));
        }
        if t % hz == 0 {
            for a in sim.fauna().view().animals.iter().filter(|a| a.founder == Some(f)) {
                present += 1;
                hungry += usize::from(a.eating);
            }
        }
    }
    holds.body_seconds += present as f64;
    let (bodies_end, energy_end, deaths_end) = lineage(&sim, f);
    numbers_at_s.push(seconds);
    counts.push(lineage_counts(&sim));
    let numbers = Founder::ALL
        .into_iter()
        .filter(|g| counts.iter().any(|c| c[g.index()] > 0))
        .map(|g| LineageCount { lineage: g.name().to_string(), counts: counts.iter().map(|c| c[g.index()]).collect() })
        .collect();
    LiveArm {
        seed,
        bodies_start,
        bodies_end,
        deaths: deaths_end.saturating_sub(deaths_start),
        energy_start,
        energy_end,
        hungry_share: if present > 0 { hungry as f64 / present as f64 } else { 0.0 },
        diet: diet(&ledger_start, sim.fauna().view().ledger, f),
        numbers_at_s,
        numbers,
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
        diet_heuristic: sum_diets(h_arms.iter().map(|a| a.diet.as_slice())),
        diet_brain: sum_diets(b_arms.iter().map(|a| a.diet.as_slice())),
        numbers_heuristic: sum_numbers(&h_arms),
        numbers_brain: sum_numbers(&b_arms),
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
        "# check {} gen {}: gain/body brain {:+.5} heuristic {:+.5} (paired {:+.5}); deaths {}/{}; hungry {:.2}; switches/body-min {:.2}/{:.2}; scenes brain/rules {}; diet brain [{}] rules [{}]; prey brain [{}] rules [{}]",
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
        scenes.join(", "),
        diet_text(&r.diet_brain),
        diet_text(&r.diet_heuristic),
        prey_text(&r.diet_brain, &r.numbers_brain),
        prey_text(&r.diet_heuristic, &r.numbers_heuristic),
    )
}

/// `food k<kill share> e<eaten share>` per food, eaten share first in size.
fn diet_text(d: &[DietItem]) -> String {
    let mut d: Vec<&DietItem> = d.iter().collect();
    d.sort_by(|a, b| b.eaten_share.total_cmp(&a.eaten_share));
    d.iter().map(|x| format!("{} k{:.2} e{:.2}", x.food, x.kill_share, x.eaten_share)).collect::<Vec<_>>().join(", ")
}

/// `lineage start→end` for each animal lineage in the diet.
fn prey_text(d: &[DietItem], n: &[LineageNumbers]) -> String {
    n.iter()
        .filter(|c| d.iter().any(|x| x.food == c.lineage))
        .map(|c| format!("{} {}->{}", c.lineage, c.start, c.end))
        .collect::<Vec<_>>()
        .join(", ")
}
