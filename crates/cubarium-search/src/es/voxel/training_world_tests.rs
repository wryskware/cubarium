//! The audit round's step 5 (`design/handoffs/voxel-audit-round-2026-09-28.md` §5): the
//! frozen trainer's arenas and landscapes are the live world — graze refuge, defence, a
//! night, and the ecology vector. No ticks.

use cubarium_voxel_fauna::{FaunaConfig, Founder};
use cubarium_voxel_flora::FloraConfig;
use cubarium_voxel_sim::arena::{ArenaGrid, ArenaLayers, arena_start_hour};

use super::live::ecology::EcologyParams;
use super::task::{PreparedArena, Stage};
use super::training_world::{arena_layers_for, landscape_config, landscape_layers_for};

fn heavy() -> EcologyParams {
    let mut e = EcologyParams::defaults();
    e.set("founder.mass_scale@frondgrazer", 100.0).expect("in range");
    e.set("flora.foliage_rate", 2.0).expect("in range");
    e
}

/// The default vector's layers are the live ones; another vector's are its own configs.
#[test]
fn the_training_layers_are_the_ecology_vectors() {
    for grid in [ArenaGrid::Standard, ArenaGrid::Fine] {
        assert_eq!(
            arena_layers_for(&EcologyParams::defaults(), grid),
            ArenaLayers::live(grid)
        );
        let e = heavy();
        let layers = arena_layers_for(&e, grid);
        assert_eq!(layers.fauna, e.fauna_config());
        assert_eq!(
            layers.flora,
            e.flora_config(FloraConfig::for_voxel_size(grid.voxel_m()))
        );
        assert_ne!(layers.fauna, FaunaConfig::default());
    }
    let (flora, fauna) = landscape_layers_for(&heavy(), 0.125);
    assert_eq!(fauna, heavy().fauna_config());
    assert_eq!(flora, heavy().flora_config(FloraConfig::for_voxel_size(0.125)));
}

/// A prepared arena is built on the layers it is handed, and rebuilds on them too.
#[test]
fn a_prepared_arena_is_built_on_its_layers() {
    let grid = ArenaGrid::Standard;
    let layers = arena_layers_for(&heavy(), grid);
    let prepared = PreparedArena::build_stage_with(
        Founder::Browser,
        1,
        Stage::A,
        super::task::Band::Landed,
        grid,
        layers.clone(),
    );
    let arena = prepared.episode_arena();
    assert_eq!(arena.fauna.config(), &layers.fauna);
    assert_eq!(arena.flora.config(), &layers.flora);
    assert_eq!(prepared.rebuilt().fauna.config(), &layers.fauna);
}

/// A landscape's world starts its day at an hour drawn from its seed base, the arenas'
/// draw, so about a third of the landscapes are founded into a night.
#[test]
fn a_landscape_starts_its_day_at_a_drawn_hour() {
    let c = landscape_config("small", 101).expect("a preset");
    assert_eq!(c.day.start_hour, arena_start_hour(101));
    assert_ne!(c.day.mode, cubarium_voxel::weather::DayMode::Off);
    assert!(landscape_config("no-such-preset", 1).is_err());
}
