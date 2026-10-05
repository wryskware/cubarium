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
// 19: roster mechanics — `Ground::vine_seeds` (the latticevine seed bank),
//     `VineConfig::seed_max_age_s`.
// 20: night survival 1b — `SpeciesConfig::{night_bloom_scale, night_floor, night_min_s}`,
//     `FloraLedger::night_spend`; a stand spends its reserve at night.
// 21: the minimal gut's repair 3 — `Vine::mineral`, `VineConfig::mineral_fraction`.
// 22: sparse, staggered flowering — `VineConfig::{spur_share, nectar_per_face_s,
//     fruit_per_face_s}` replace `{spur_window_s, spur_jitter_s, nectar, fruit}`, a bare
//     spur's timer is its random rest, `Cover`'s shower tick goes; `Stand::fruit_rest_s`,
//     `SpeciesConfig::fruit_rest_s`; founders rank walls by lit open area.
// 23: plant succession — `SpeciesConfig::overtop_covers`, `FloraLedger::{overtop_pairs,
//     foliage_built, shed_organic, overtopped_organic, foliage_taken, litter_taken}`.
// 24: springturf spread — `SpeciesConfig::{runners_overtop, wald_mean_m, wald_shape_m,
//     burr_pickup, burr_retention_mean_m, burr_retention_shape, cure_*, cured_srgb}`,
//     `Dispersal::Wald`, `FloraConfig::burr_cap`, `Stand::{cured, dry_s}`,
//     `Flora::{burrs, burr_marks}`, the ledger's runner, death-cause, burr and cure lines.
// 25: the plant clock — `FloraConfig::step_every`, `Flora::since_step`.
// 26: a seedling's reflush answers to its seed's leaf — `SpeciesConfig::seedling_below`.
// 27: a face holds one plant and one fungus (`FaceLayer`) — `Flora::stands` sorted by
//     `(site, place)`, `Ground::sprouting_fungus`.
// 28: the detritus web — `FloraConfig::film_graze_refuge`.
// 29: turf-establish — the canopy gate and runner support: `Stand::support`,
//     `SpeciesConfig::{establish_light_margin, runner_support_s, runner_support_share}`, the
//     ledger's canopy and support lines.
// 30: the capgnawer package — `SpeciesConfig::propagule_cap_share` (spores from caps).
pub const SCHEMA: u32 = 30;

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
