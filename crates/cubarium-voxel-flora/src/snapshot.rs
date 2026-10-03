//! Save/load as postcard bytes behind a schema tag, the way
//! [`cubarium_voxel::snapshot`] and `cubarium_voxel_fauna::snapshot` do it. A different
//! tag is refused; there is no migration, ever.
//!
//! `always-fresh-never-migrate`: a new schema refuses old worlds and nothing is
//! synthesized or re-anchored. Nothing in the host writes one of these yet.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::Flora;

/// The plant layer's snapshot format. Bumped whenever a serialized field, the config
/// inside the snapshot, or what a saved field means changes. Postcard is not
/// self-describing, so other tags are refused, never migrated.
// 13: F1 (tests-first stubs) — `Stand::nectar`, `SpeciesConfig::{nectar_per_s, nectar_cap}`.
// 14: F1 — bloomcrown's nectar is made and taken; `SpeciesConfig::nectar_mineral_fraction`.
// 15: F3 (tests-first stubs) — `Stand::fruit`, `SpeciesConfig::{fruit_per_s, fruit_cap,
// fruit_drop_s, fruit_mineral_fraction}`.
// 16: F3 — lanternberry fruit is made, falls and is taken; `Stand::fruit_age_s`.
// 17: merge with main's glowcap dark habitat (main's schema 11).
// 18: glassfilm drying — `FloraConfig::{film_wet_min_depth_m, film_gone_fraction}`.
pub const SCHEMA: u32 = 18;

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
    let mut flora = envelope.flora;
    // The crowns are derived and not saved.
    flora.refresh_crowns(None);
    Ok(flora)
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
