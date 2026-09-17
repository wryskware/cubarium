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
    Ok(env.world)
}
