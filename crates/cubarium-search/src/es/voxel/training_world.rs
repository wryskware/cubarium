//! **The frozen trainer's worlds are the live world** (audit round step 5,
//! `design/handoffs/voxel-audit-round-2026-09-28.md` §5; audit §1.3.4): its arenas and
//! landscapes are built on the ecology vector's layers — the shipped plant layer with its
//! graze refuge and defence, and the vector's animal config — and their day clocks start
//! at an hour drawn per layout, so some of them are at night.
//!
//! One process-wide vector, [`training_ecology`]: the live defaults unless the command
//! line's `--training-ecology <file.toml>` set another before any fixture was built.
//! Remote evaluation workers build their fixtures from their own process's vector, so a
//! run that sets one must not use them ([`check_remote`]).

use std::sync::OnceLock;

use cubarium_voxel::Config;
use cubarium_voxel_fauna::FaunaConfig;
use cubarium_voxel_flora::FloraConfig;
use cubarium_voxel_sim::{ArenaGrid, ArenaLayers, arena_start_hour};

use super::live::ecology::EcologyParams;

static TRAINING: OnceLock<(EcologyParams, FaunaConfig)> = OnceLock::new();

/// Set the process's training ecology. Refused once any fixture has read it.
pub fn set_training_ecology(e: EcologyParams) -> Result<(), String> {
    let fauna = e.fauna_config();
    fauna.validate()?;
    TRAINING
        .set((e, fauna))
        .map_err(|_| "the training ecology was already fixed".to_string())
}

fn training() -> &'static (EcologyParams, FaunaConfig) {
    TRAINING.get_or_init(|| {
        let e = EcologyParams::defaults();
        let f = e.fauna_config();
        (e, f)
    })
}

/// The ecology vector the frozen trainer's worlds are built on.
pub fn training_ecology() -> &'static EcologyParams {
    &training().0
}

/// Whether the training ecology is the live defaults.
pub fn training_ecology_is_default() -> bool {
    *training_ecology() == EcologyParams::defaults()
}

/// The animal config every frozen-trainer body runs on (and is sized from).
pub fn training_fauna_config() -> FaunaConfig {
    training().1
}

/// An arena's layers on `grid` under `e`.
pub fn arena_layers_for(e: &EcologyParams, grid: ArenaGrid) -> ArenaLayers {
    ArenaLayers {
        flora: e.flora_config(FloraConfig::for_voxel_size(grid.voxel_m())),
        fauna: e.fauna_config(),
    }
}

/// An arena's layers on `grid` under the training ecology.
pub fn arena_layers(grid: ArenaGrid) -> ArenaLayers {
    arena_layers_for(training_ecology(), grid)
}

/// A landscape's `(plant, animal)` configs at cell size `voxel_m` under `e`.
pub fn landscape_layers_for(e: &EcologyParams, voxel_m: f64) -> (FloraConfig, FaunaConfig) {
    (
        e.flora_config(FloraConfig::for_voxel_size(voxel_m)),
        e.fauna_config(),
    )
}

/// A landscape's world config: the preset's, its day starting at the hour drawn from
/// `seed_base` (the arenas' draw, [`arena_start_hour`]).
pub fn landscape_config(preset: &str, seed_base: u64) -> Result<Config, String> {
    let mut cfg = cubarium_voxel::Preset::find(preset)
        .ok_or_else(|| format!("no landform preset is called {preset:?}"))?
        .config();
    cfg.day.start_hour = arena_start_hour(seed_base);
    Ok(cfg)
}

/// Refuse remote evaluation workers under a non-default training ecology: they would
/// build the live world's fixtures, not this one's.
pub fn check_remote(remotes: usize) -> Result<(), String> {
    if remotes > 0 && !training_ecology_is_default() {
        return Err(
            "--training-ecology is set, and remote workers would build the default training \
             world: run without --remote"
                .into(),
        );
    }
    Ok(())
}
