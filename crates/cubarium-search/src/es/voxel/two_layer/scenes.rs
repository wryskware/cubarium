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
