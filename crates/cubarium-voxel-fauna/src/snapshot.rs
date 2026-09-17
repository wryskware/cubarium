//! Save/load as postcard bytes behind a schema tag, the way
//! [`cubarium_voxel::snapshot`] does it. A different tag is refused; there is no
//! migration, ever.
//!
//! This is the standing rule and not a choice of this round: `always-fresh-never-migrate`
//! says a new schema refuses old worlds and nothing is synthesized or re-anchored. A saved
//! animal layer whose [`crate::FaunaConfig`] no animal can live under is refused too, for
//! the reason [`crate::Fauna::new`] panics on one: it is not a runtime condition.
//!
//! **What this does not do yet, and why.** The round's brief asks for the fauna to be
//! serialized *beside the flora*, and there is nothing to sit beside: the plant layer has
//! no snapshot at all. `cubarium_voxel_flora::Flora` does not implement `Serialize`, and
//! the host's `w`/`l` stdin commands write and read `World::save()` alone
//! (`crates/cubarium/src/voxel/mod.rs`), so a host-side envelope carrying the world, the
//! plants and the animals cannot be written without adding the derive to the flora crate —
//! which is another package's file this round. This module is therefore the animal layer's
//! own always-fresh snapshot, ready for that envelope, and the host wiring is reported as
//! owed.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::Fauna;

/// Schema 2: schema 1 was the first animal layer; the per-plant arrays in the ledger are
/// sized by the flora's species count, which the glowcap merge raised to six, so the
/// postcard layout changed with it (round 5b/5c merge). Postcard is not
/// self-describing, so any new field is a new format and earlier tags are refused.
pub const SCHEMA: u32 = 2;

#[derive(Serialize, Deserialize)]
struct Envelope {
    schema: u32,
    fauna: Fauna,
}

pub fn encode(fauna: &Fauna) -> Vec<u8> {
    postcard::to_stdvec(&Envelope { schema: SCHEMA, fauna: fauna.clone() })
        .expect("a Fauna always serializes")
}

pub fn decode(bytes: &[u8]) -> anyhow::Result<Fauna> {
    let tag: u32 = postcard::from_bytes(bytes).context("not a voxel fauna snapshot")?;
    if tag != SCHEMA {
        bail!("voxel fauna snapshot schema {tag} is not {SCHEMA}; start a fresh world");
    }
    let env: Envelope = postcard::from_bytes(bytes).context("corrupt voxel fauna snapshot")?;
    if let Err(e) = env.fauna.config().validate() {
        bail!("invalid voxel fauna snapshot — {e}");
    }
    Ok(env.fauna)
}
