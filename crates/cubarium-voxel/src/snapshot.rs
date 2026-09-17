//! Save/load as postcard bytes behind a schema tag. A different tag is refused; there
//! is no migration, ever.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::World;

pub const SCHEMA: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Envelope {
    schema: u32,
    world: World,
}

pub fn encode(world: &World) -> Vec<u8> {
    postcard::to_stdvec(&Envelope { schema: SCHEMA, world: world.clone() })
        .expect("a World always serializes")
}

pub fn decode(bytes: &[u8]) -> anyhow::Result<World> {
    let tag: u32 = postcard::from_bytes(bytes).context("not a voxel world snapshot")?;
    if tag != SCHEMA {
        bail!("voxel snapshot schema {tag} is not {SCHEMA}; start a fresh world");
    }
    let env: Envelope = postcard::from_bytes(bytes).context("corrupt voxel world snapshot")?;
    env.world.validate_loaded().context("invalid voxel world snapshot")?;
    Ok(env.world)
}

#[cfg(test)]
mod tests {
    use crate::{Config, World};

    fn fixture() -> World {
        World::empty(Config { width: 4, height: 4, depth: 1, ..Config::default() })
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
}
