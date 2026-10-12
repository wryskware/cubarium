//! **The staged mode scenes for cloning** (BRAIN-NEXT steps 3–5): the scenes of
//! [`cubarium_voxel_sim::mode_scenes`] as teacher recordings, as closed-loop runs of a brain
//! with the dry-run teacher beside it, and as matched starts for the per-mode competence
//! checks ([`super::super::kin::matched::Scene`]).

use cubarium_voxel_fauna::{DryScope, Founder};
use cubarium_voxel_sim::mode_scenes::{self, ModeScene};

use super::clone::TrajectoryRecord;
use super::record::{closed_loop_records, records_from_tap};
use super::shape::TwoLayerShape;
use crate::es::voxel::EpisodeDriver;
use crate::es::voxel::kin::matched::{Scene, SceneCopy};

/// One staged scene as a matched start: every copy is founded afresh from the seed, the
/// scene's lineage on `driver` (its heuristic when `None`), with the dry-run teacher beside
/// its brain-driven bodies and the tap on when asked.
pub struct StagedScene {
    pub config: mode_scenes::Config,
    pub scene: ModeScene,
    pub seed: u64,
    pub driver: Option<EpisodeDriver>,
    /// The dry-run teacher beside the lineage's brain-driven bodies.
    pub dry: bool,
    /// Record the tap.
    pub tap: bool,
}

impl SceneCopy for mode_scenes::Episode {
    fn sim(&self) -> &cubarium_voxel_sim::Sim {
        &self.sim
    }
    fn sim_mut(&mut self) -> &mut cubarium_voxel_sim::Sim {
        &mut self.sim
    }
    fn step(&mut self) -> bool {
        mode_scenes::Episode::step(self)
    }
}

impl Scene for StagedScene {
    type Copy = mode_scenes::Episode;
    fn open(&self) -> Result<mode_scenes::Episode, String> {
        let mut e = mode_scenes::found(&self.config, self.scene, self.seed)?;
        let f = self.scene.founder();
        if let Some(d) = &self.driver {
            if d.founder() != f {
                return Err(format!("a {} driver for a {} scene", d.founder().name(), f.name()));
            }
            crate::es::voxel::live::install_centres(e.sim.fauna_mut(), &[(f, d.clone())]);
        }
        if self.dry {
            e.sim.fauna_mut().dry_teach(f, Some(DryScope::Brains));
        }
        if self.tap {
            e.sim.fauna_mut().start_tap();
        }
        Ok(e)
    }
    fn ticks(&self) -> u64 {
        (self.config.duration_s * f64::from(cubarium_voxel_fauna::TICK_HZ)).ceil() as u64
    }
    fn focals(&self, copy: &mode_scenes::Episode, _lineage: Founder) -> Vec<u64> {
        copy.focal.clone()
    }
}

/// Run a copy to its end and hand back its tap.
fn run(scene: &StagedScene) -> Result<cubarium_voxel_fauna::ActionTap, String> {
    let mut e = scene.open()?;
    while e.step() {}
    e.sim.fauna_mut().take_tap().ok_or_else(|| "the scene ran without its tap".into())
}

/// **The teacher's records from one scene** (step 3): the scene under its heuristics, every
/// body of its lineage recorded as [`super::clone::RecordSource::Teacher`] stretches.
pub fn record(config: &mode_scenes::Config, scene: ModeScene, seed: u64, shape: &TwoLayerShape, min_samples: usize) -> Result<Vec<TrajectoryRecord>, String> {
    let s = StagedScene { config: config.clone(), scene, seed, driver: None, dry: false, tap: true };
    let tap = run(&s)?;
    Ok(records_from_tap(&tap, scene.founder(), shape, &|_, _| false, min_samples).into_iter().map(|(_, r)| r).collect())
}

/// **Closed-loop records from one scene** (step 4): the lineage on `driver`, the dry-run
/// teacher labelling the states it reaches ([`closed_loop_records`]).
pub fn closed_loop(
    config: &mode_scenes::Config,
    scene: ModeScene,
    seed: u64,
    driver: &EpisodeDriver,
    shape: &TwoLayerShape,
    min_samples: usize,
) -> Result<Vec<TrajectoryRecord>, String> {
    let s = StagedScene { config: config.clone(), scene, seed, driver: Some(driver.clone()), dry: true, tap: true };
    let tap = run(&s)?;
    Ok(closed_loop_records(&tap, scene.founder(), shape, min_samples).into_iter().map(|(_, r)| r).collect())
}

/// **One per-mode competence check** (step 5; §11.3 "Competence before freezing"): one
/// scene, one seed, one arm, closed loop.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Competence {
    pub scene: &'static str,
    pub intent: &'static str,
    pub seed: u64,
    /// Whether the scene did what the mode is for, 0–1 ([`competence`]'s per-scene note).
    pub competence: f64,
    /// The share of the focal bodies' decision samples that held the scene's intent (their
    /// own records: the brain's hold, or the rules').
    pub mode_share: f64,
    /// Deaths of the scene's lineage.
    pub deaths: u64,
    /// The lineage's energy (bodies and stores) at the start and end.
    pub energy_start: f64,
    pub energy_end: f64,
    /// The null pair's largest score gap over every body (two copies, one arm): 0 means the
    /// arms' differences are theirs alone (`kin::matched::null_pair`).
    pub null_gap: f64,
}

fn tally(sim: &cubarium_voxel_sim::Sim, f: Founder) -> (f64, u64) {
    let v = sim.fauna().view();
    let body: f64 = v.animals.iter().filter(|a| a.founder == Some(f)).map(|a| a.body).sum();
    let energy = body + crate::es::voxel::live::balance::stores(v.animals, sim.fauna().config(), f);
    (energy, v.ledger.deaths_by_founder_cause[f.index()].iter().sum())
}

fn up_a_stand(a: &cubarium_voxel_fauna::Animal) -> bool {
    use cubarium_voxel_fauna::{MobilityMode, PerchAnchor};
    match a.mobility.mode {
        MobilityMode::Perched => matches!(a.mobility.perch.map(|p| p.anchor), Some(PerchAnchor::Stand { .. })),
        MobilityMode::Climb => !a.mobility.falling,
        _ => false,
    }
}

/// **Run one scene closed loop on an arm** (`driver`; `None`: the heuristic) and read what
/// its mode is for, over the focal bodies:
/// - roost: the share up a crown (perched on a stand) at the end;
/// - flee to the trees: the share that reached a stand (perched on it or climbing) alive;
/// - groom: the share of seconds a groomer is in contact with the host
///   (`grooming.groom_radius_m`) or the larva is gone;
/// - follow mother: the share of seconds the young is within its lineage's nursing leash
///   (`nursing.leash_m`, 3D) of its mother;
/// - seedporter rest: the share of body-seconds resting or asleep;
/// - feed: the share of the seconds the lanternjaw wants food (its eating latch on) that it
///   takes a bite ([`cubarium_voxel_fauna::State::Cropping`]). Not the share of the carcass
///   eaten: the bite rate caps a 60 s scene at about an eighth of it and the full event stops
///   the meal long before the carcass is gone (BRAIN-FIX step 1), so that read capped every
///   arm, the rules included, at the same 0.125;
/// - give way: the share of the seconds after a hound first comes within
///   `predation.carcass_yield_radius_m` that the young is alive and keeps beyond
///   `predation.still_threat_near_m` of every hound (where even a still hound is a threat).
pub fn competence(config: &mode_scenes::Config, scene: ModeScene, seed: u64, driver: Option<&EpisodeDriver>) -> Result<Competence, String> {
    use crate::es::voxel::kin::matched::{CopyPlan, null_pair};
    let staged = StagedScene { config: config.clone(), scene, seed, driver: driver.cloned(), dry: false, tap: false };
    let mut e = staged.open()?;
    let f = scene.founder();
    let k = f.manifest().menu.index(scene.intent()).ok_or("the scene's intent is not on the menu")?;
    let ring = f64::from(config.width) * config.voxel_m;
    let dist = |a: &cubarium_voxel_fauna::Animal, b: &cubarium_voxel_fauna::Animal| {
        let dx = (b.pose.x - a.pose.x + ring / 2.0).rem_euclid(ring) - ring / 2.0;
        (dx.hypot(b.pose.z - a.pose.z)).hypot(b.pose.y_m - a.pose.y_m)
    };
    let rider = match scene {
        ModeScene::Groom => e.sim.fauna().view().animals.iter().find(|a| a.attached.map(|t| t.host) == e.others.first().copied()).map(|a| a.id),
        _ => None,
    };
    let (energy_start, deaths0) = tally(&e.sim, f);
    let leash = e.sim.fauna().config().founder(f).nursing.leash_m;
    let yield_m = e.sim.fauna().config().predation.carcass_yield_radius_m;
    let groom_m = e.sim.fauna().config().founder(f).grooming.groom_radius_m;
    let keep_m = e.sim.fauna().config().predation.still_threat_near_m;
    let (mut held, mut samples) = (0usize, 0usize);
    let (mut good, mut seconds) = (0usize, 0usize);
    let mut reached = std::collections::BTreeSet::new();
    let mut threatened = false;
    let per_s = u64::from(cubarium_voxel_fauna::TICK_HZ);
    while e.step() {
        let fauna = e.sim.fauna();
        let v = fauna.view();
        let now = fauna.tick();
        let focal: Vec<&cubarium_voxel_fauna::Animal> = e.focal.iter().filter_map(|id| v.animal(*id)).collect();
        for a in &focal {
            if a.intent.sample_tick == now {
                samples += 1;
                held += usize::from(usize::from(a.intent.held) == k);
            }
            if up_a_stand(a) {
                reached.insert(a.id);
            }
        }
        if e.ticks() % per_s != 0 {
            continue;
        }
        match scene {
            ModeScene::FollowMother => {
                let (young, mother) = (focal.first(), e.others.first().and_then(|m| v.animal(*m)));
                seconds += 1;
                if let (Some(y), Some(m)) = (young, mother) {
                    good += usize::from(dist(y, m) <= leash);
                }
            }
            ModeScene::Groom => {
                let host = e.others.first().and_then(|h| v.animal(*h));
                let gone = rider.is_some_and(|r| v.animal(r).and_then(|a| a.attached).is_none());
                seconds += 1;
                good += usize::from(gone || host.is_some_and(|h| focal.iter().any(|a| dist(a, h) <= groom_m)));
            }
            ModeScene::SeedporterRest => {
                for a in &focal {
                    seconds += 1;
                    good += usize::from(matches!(a.state, cubarium_voxel_fauna::State::Resting | cubarium_voxel_fauna::State::Asleep));
                }
            }
            ModeScene::Feed => {
                if let Some(a) = focal.first().filter(|a| a.eating) {
                    seconds += 1;
                    good += usize::from(a.state == cubarium_voxel_fauna::State::Cropping);
                }
            }
            ModeScene::GiveWay => {
                let hounds: Vec<_> = e.others.iter().filter_map(|h| v.animal(*h)).collect();
                let lj = e.focal.first().and_then(|id| v.animal(*id));
                let near = |a: &cubarium_voxel_fauna::Animal| hounds.iter().map(|h| dist(a, h)).fold(f64::INFINITY, f64::min);
                threatened |= lj.is_some_and(|a| near(a) <= yield_m);
                if threatened {
                    seconds += 1;
                    good += usize::from(lj.is_some_and(|a| near(a) > keep_m));
                }
            }
            _ => {}
        }
    }
    let v = e.sim.fauna().view();
    let n = e.focal.len().max(1) as f64;
    let share = |g: usize, s: usize| if s > 0 { g as f64 / s as f64 } else { 0.0 };
    let competence = match scene {
        ModeScene::Roost => e.focal.iter().filter_map(|id| v.animal(*id)).filter(|a| up_a_stand(a) && a.mobility.mode == cubarium_voxel_fauna::MobilityMode::Perched).count() as f64 / n,
        ModeScene::FleeToTrees => e.focal.iter().filter(|id| reached.contains(id) && v.animal(**id).is_some()).count() as f64 / n,
        ModeScene::Feed | ModeScene::Groom | ModeScene::FollowMother | ModeScene::SeedporterRest | ModeScene::GiveWay => share(good, seconds),
    };
    let (energy_end, deaths1) = tally(&e.sim, f);
    let focal = *e.focal.first().ok_or("a scene with no focal body")?;
    let np = null_pair(&staged, CopyPlan { focal, candidate: None }, None, &Default::default(), 2)?;
    Ok(Competence {
        scene: scene.name(),
        intent: scene.intent(),
        seed,
        competence,
        mode_share: share(held, samples),
        deaths: deaths1 - deaths0,
        energy_start,
        energy_end,
        null_gap: np.max_combined_gap.max(np.focal_gap),
    })
}
