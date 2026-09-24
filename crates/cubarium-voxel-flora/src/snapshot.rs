//! Save/load as postcard bytes behind a schema tag, the way
//! [`cubarium_voxel::snapshot`] and `cubarium_voxel_fauna::snapshot` do it. A different
//! tag is refused; there is no migration, ever.
//!
//! This is the standing rule and not a choice of this package: `always-fresh-never-migrate`
//! says a new schema refuses old worlds and nothing is synthesized or re-anchored.
//!
//! **Why schema 1 and not a bump.** The plant layer had no snapshot at all before this
//! package: `Flora` did not implement `Serialize`, and the host's `w`/`l` commands write
//! and read `World::save()` alone, so a restart has always rebuilt the plants from the
//! seeder (`crates/cubarium-voxel-fauna/src/snapshot.rs`, "What this does not do yet").
//! The layers package is the one that gives a stand state a seeder cannot re-derive — a
//! plant grazed at the base is a different plant from one grazed evenly, and only its
//! `layer_stock` says which — so the plant layer gets its own always-fresh envelope
//! here, ready for the host envelope that carries the world, the plants and the animals
//! together. **The host wiring is still owed**; nothing writes one of these yet.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::Flora;

/// Schema 1: the first flora snapshot there has ever been, and the first format that
/// could not be re-derived from a seed — [`crate::Stand::layer_stock`] and
/// [`crate::Stand::profile_stage`], the per-layer tissue of
/// `design/handoffs/voxel-plant-layers-2026-09-22.md`. Postcard is not
/// self-describing, so a new field is a new format: other tags are refused, never
/// migrated.
///
/// Schema 2: package L (`design/handoffs/voxel-ladder-growth-2026-09-23.md`) states every
/// crown in metres (`SpeciesConfig::crown_height_m` / `crown_radius_m`) at the size
/// ladder, with the capped-seedling growth rule. The serialised config's fields changed,
/// and a schema-1 stand's wood would grow a different plant: refused, not migrated.
///
/// Schema 3: package N (`design/handoffs/voxel-new-plants-2026-09-23.md`) appends three
/// species (the per-species ledger arrays grow to nine) and two config fields
/// (`water_depth_min_m`, `falls`): refused, not migrated.
///
/// Schema 4: the plant-viability packages (`design/handoffs/voxel-plant-viability-2026-09-23.md`).
/// G adds `SpeciesConfig::graze_refuge` to the serialised config; F moves every species'
/// water thresholds from pore fraction to available water (`wilt_water`, `full_water`,
/// `establish_water_min`), same bytes, new meaning. Refused, not migrated.
pub const SCHEMA: u32 = 4;

#[derive(Serialize, Deserialize)]
struct Envelope {
    schema: u32,
    flora: Flora,
}

pub fn encode(flora: &Flora) -> Vec<u8> {
    postcard::to_stdvec(&Envelope {
        schema: SCHEMA,
        flora: flora.clone(),
    })
    .expect("a Flora always serializes")
}

/// Read a plant layer back. A payload tagged with any other schema is refused with the
/// tag it carries: start a fresh world.
pub fn decode(bytes: &[u8]) -> anyhow::Result<Flora> {
    let (tag, _): (u32, &[u8]) =
        postcard::take_from_bytes(bytes).context("reading the flora snapshot's schema tag")?;
    if tag != SCHEMA {
        bail!(
            "this flora snapshot is schema {tag} and this build reads schema {SCHEMA}: \
             plant layers are never migrated, start a fresh world"
        );
    }
    let envelope: Envelope =
        postcard::from_bytes(bytes).context("decoding the flora snapshot's payload")?;
    envelope.flora.config().validate().map_err(|e| {
        anyhow::anyhow!("the saved flora config is not one a plant can live under: {e}")
    })?;
    Ok(envelope.flora)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Flora, FloraConfig};

    #[test]
    fn a_round_trip_keeps_the_layers_and_a_foreign_tag_is_refused() {
        let flora = Flora::new(FloraConfig::default());
        let bytes = encode(&flora);
        let back = decode(&bytes).expect("its own format");
        assert_eq!(back.view().stands.len(), flora.view().stands.len());

        let mut wrong = postcard::to_stdvec(&(SCHEMA + 1)).expect("a tag");
        wrong.extend_from_slice(&bytes[wrong.len().min(bytes.len())..]);
        assert!(decode(&wrong).is_err(), "a foreign schema must be refused");
    }
}
