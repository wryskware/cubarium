//! Save/load as postcard bytes behind a schema tag. A different tag is refused; there
//! is no migration, ever.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::World;

/// Bumped to 2 for the plant boundary (`Ledger::transpiration_out`,
/// `World::terrain_version`) and to 3 for the water table
/// (`Config::initial_aquifer_head_m`, which sits inside the serialized world), and to 4
/// for the closed water budget (`World::atmosphere_m3`, `World::shower_left_m3`, the four
/// new `Ledger` terms and its shower count, and the `Config` switch with its two shower
/// knobs). Postcard
/// is not self-describing, so a new field is a new format: earlier tags are refused,
/// never migrated.
pub const SCHEMA: u32 = 4;

#[derive(Serialize, Deserialize)]
struct Envelope {
    schema: u32,
    world: World,
}

pub fn encode(world: &World) -> Vec<u8> {
    postcard::to_stdvec(&Envelope {
        schema: SCHEMA,
        world: world.clone(),
    })
    .expect("a World always serializes")
}

pub fn decode(bytes: &[u8]) -> anyhow::Result<World> {
    let tag: u32 = postcard::from_bytes(bytes).context("not a voxel world snapshot")?;
    if tag != SCHEMA {
        bail!("voxel snapshot schema {tag} is not {SCHEMA}; start a fresh world");
    }
    let mut env: Envelope = postcard::from_bytes(bytes).context("corrupt voxel world snapshot")?;
    env.world
        .validate_loaded()
        .context("invalid voxel world snapshot")?;
    // The water active sets are not serialized — they are a cache of the arrays — so a
    // decoded world builds them before anything can iterate them.
    env.world.rebuild_active_sets();
    Ok(env.world)
}

#[cfg(test)]
mod tests {
    use crate::{Config, World};

    fn fixture() -> World {
        World::empty(Config {
            width: 4,
            height: 4,
            depth: 1,
            ..Config::default()
        })
    }

    #[test]
    fn a_zero_width_config_is_refused() {
        let mut world = fixture();
        world.config.width = 0;
        let err = World::load(&world.save()).expect_err("a zero-width world is not a world");
        assert!(format!("{err:#}").contains("dimensions"), "{err:#}");
    }

    #[test]
    fn a_truncated_array_is_refused() {
        let mut world = fixture();
        world.free.truncate(3);
        let err = World::load(&world.save()).expect_err("a short store is not a world");
        assert!(format!("{err:#}").contains("free has 3 entries"), "{err:#}");
    }

    /// `0..=1` is not enough: the fraction has to be one the cell's own material can
    /// hold, or the view reports water the store accounting does not count.
    #[test]
    fn free_water_in_a_solid_is_refused() {
        let mut world = fixture();
        // `y = 0` is the bedrock foundation `World::empty` lays down.
        let i = world.config.index(1, 0, 0);
        world.free[i] = 1.0;
        let err = World::load(&world.save()).expect_err("bedrock holds no free water");
        assert!(
            format!("{err:#}").contains("holds no free water"),
            "{err:#}"
        );
    }

    #[test]
    fn pore_water_without_pore_space_is_refused() {
        let mut world = fixture();
        let i = world.config.index(1, 2, 0);
        assert!(
            !world.material[i].is_solid(),
            "the fixture's (1, 2, 0) must be air"
        );
        world.pore[i] = 1.0;
        let err = World::load(&world.save()).expect_err("air has no pore space");
        assert!(format!("{err:#}").contains("has no pore space"), "{err:#}");
    }
}
