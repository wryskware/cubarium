//! **Knob wiring** (R2 integration): a layer's `fauna.teachers` reaches the reflex a
//! centre installed through [`install_centres`] runs — the live host's own path — and the
//! reflex an [`EpisodeDriver`] hands out. Before R2's integration every one of those
//! reflexes was built on the default `TeacherConfig`.

use cubarium_voxel_fauna::{FaunaConfig, Founder, Pose};
use cubarium_voxel_sim::{Arena, ArenaGrid, ArenaLayers, SimConfig};

use super::install_centres;
use crate::es::voxel::controller::{EpisodeDriver, VoxelControl};

/// The founder's pose at every tick of a short static arena run, its body driven by the
/// heuristic centre installed through [`install_centres`] on a layer of `fauna`.
fn walk(fauna: FaunaConfig) -> Vec<Pose> {
    let grid = ArenaGrid::Standard;
    let mut layers = ArenaLayers::live(grid);
    layers.fauna = fauna;
    let arena = Arena::build_on_with(Founder::Blind, 1, grid, layers);
    let id = arena.animal_id.expect("the arena placed its founder");
    let mut sim = arena.into_sim(SimConfig::with_threads(1));
    let driver = EpisodeDriver::control(VoxelControl::Heuristic, Founder::Blind);
    install_centres(sim.fauna_mut(), &[(Founder::Blind, driver)]);
    (0..400)
        .map(|_| {
            sim.step_static();
            sim.fauna().view().animal(id).map_or(Pose::ZERO, |a| a.pose)
        })
        .collect()
}

#[test]
fn a_layers_teacher_numbers_reach_the_centre_install_centres_puts_on_its_bodies() {
    let default = FaunaConfig::default();
    let mut still = default.clone();
    // A reflex that may not turn, and stands still when blocked or escaping.
    still.teachers.wander_turn = 0.0;
    still.teachers.blind_turn = 0.0;
    still.teachers.escape_creep = 0.0;
    still.teachers.blocked_forward = 0.0;
    let base = walk(default.clone());
    let again = walk(default);
    let moved = walk(still);
    assert_eq!(base, again, "the arena run repeats");
    assert_ne!(
        base, moved,
        "fauna.teachers must change how the installed reflex walks"
    );
}
