//! Save/load the way [`cubarium_voxel::snapshot`] does it: the plant layer's **state** as
//! postcard bytes behind a schema tag, its **config** beside it as a named-field document
//! ([`cubarium_voxel::savedoc`]). A different state tag is refused as another version
//! (`always-fresh-never-migrate` holds for the state and for meaning); a config knob added or
//! dropped since the save is not a new version.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::{Flora, FloraConfig};

/// The plant layer's state format.
///
/// **What needs a bump** (Wrysk, 2026-10-08): a change to the *state's* layout (a serialized
/// field of `Flora` or anything it holds, other than its config, added, removed, reordered or
/// retyped), or a change to what a saved field or knob *means*. **What does not:** adding,
/// removing or retuning a config knob (`FloraConfig` and everything inside it) — the document
/// carries names, so an older save takes the new knob's default.
///
/// Before 39 the config sat inside the positional state and every knob was a bump:
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
// 31: CONTEST — `Stand::place` (a face's two plants, `FaceLayer::Under`) and the
//     ledger's `recruits`, `escapes`, `under_deaths`.
// 32: DORMANCY — `SpeciesConfig::cure_cover` (curing on the budget); a cured stand's
//     death on its stores is `DeathCause::Drought`.
// 33: SNAIL-FIX — glassfilm on wet ground: `FloraConfig::{film_land_moisture_min,
//     film_land_growth_per_s, film_land_capacity_per_m2}`.
// 34: SNAIL-LAND — the shore band and the wet capacity: `FloraConfig::{film_shore_band_m,
//     film_shore_moisture_min, film_land_capacity_wet_per_m2}`.
// 35: GLOWCAP — demand-bound substrate, required carbon substrate, mycelium grazing,
//     and pool-specific intake/decomposition diagnostics.
// 36: GLOWCAP-2 — the sun on a saprotroph: `SpeciesConfig::{sun_shade_max,
//     sun_cap_loss_per_s, sun_mycelium_loss_per_s}`, `FloraLedger::sun_dried` and
//     `DeathCause::Desiccated`.
// 37: SUN-SHELTER — `SpeciesConfig::sun_dark_shelter`: its dark habitat shelters it from the sun.
// 38: FOLIAGE STATES — `SpeciesConfig::{foliage_state_below, foliage_state_hysteresis}`
//     (presentation knobs carried with the species).
// 39: SAVE-EXTENSIBLE — the config moved out of the positional state into a named-field
//     document beside it.
// 40: FLOWER-FRUIT — `Stand::crop` (the crop's flowering, visits and set), `CoveredFace::visits`,
//     `FloraLedger::{flower_visits, crops_set, fruit_set, fruit_overhead}`; a crop eaten out
//     rests like a fallen one.
// 41: DECAY-GLOWCAP — Ground::fragmented_s; mycelium drying withdraws against its
//     full footprint (wood_max) and exposed tissue does not regrow in daylight.
pub const SCHEMA: u32 = 41;

#[derive(Serialize)]
struct EnvelopeOut<'a> {
    schema: u32,
    config: Vec<u8>,
    flora: &'a Flora,
}

#[derive(Deserialize)]
struct EnvelopeIn {
    _schema: u32,
    config: Vec<u8>,
    flora: Flora,
}

pub fn encode(flora: &Flora) -> Vec<u8> {
    postcard::to_stdvec(&EnvelopeOut {
        schema: SCHEMA,
        config: cubarium_voxel::savedoc::to_doc(&flora.config),
        flora,
    })
    .expect("a Flora always serializes")
}

/// Read a plant layer back. A payload tagged with another state schema is refused with the
/// tag it carries: start a fresh world.
pub fn decode(bytes: &[u8]) -> anyhow::Result<Flora> {
    let (tag, _): (u32, &[u8]) =
        postcard::take_from_bytes(bytes).context("reading the flora snapshot's schema tag")?;
    let mut flora = match tag {
        SCHEMA => {
            let env: EnvelopeIn = postcard::from_bytes(bytes).with_context(|| {
                format!(
                    "the flora state does not read as schema {SCHEMA} (a damaged file, or a build \
                     that changed the state layout without bumping the schema)"
                )
            })?;
            // The saved config is the one the layer ran, at the world's cell size; a knob
            // the save lacks takes the default at that same cell size.
            let voxel_m = cubarium_voxel::savedoc::field_f64(&env.config, "voxel_m")
                .unwrap_or(FloraConfig::default().voxel_m);
            let base = FloraConfig::default().rescaled_to(voxel_m);
            let (config, ignored) = cubarium_voxel::savedoc::from_doc(&env.config, &base)
                .context("the flora snapshot's config")?;
            cubarium_voxel::savedoc::note_ignored("flora", &ignored);
            let mut flora = env.flora;
            flora.config = config;
            flora
        }
        found => bail!(
            "this flora snapshot is schema {found} and this build reads schema {SCHEMA}: made by \
             another version of Cubarium whose plant state has another layout; start a fresh world"
        ),
    };
    flora.config().validate().map_err(|e| {
        anyhow::anyhow!("the saved flora config is not one a plant can live under: {e}")
    })?;
    // The crowns are derived and not saved.
    flora.refresh_crowns(None);
    Ok(flora)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_doc(flora: &Flora, doc: Vec<u8>) -> Vec<u8> {
        postcard::to_stdvec(&EnvelopeOut { schema: SCHEMA, config: doc, flora }).unwrap()
    }

    fn edited_doc(config: &FloraConfig, f: impl FnOnce(&mut Vec<(ciborium::Value, ciborium::Value)>)) -> Vec<u8> {
        let mut v = ciborium::Value::serialized(config).unwrap();
        let ciborium::Value::Map(m) = &mut v else { panic!("a config is a map") };
        f(m);
        let mut doc = Vec::new();
        ciborium::into_writer(&v, &mut doc).unwrap();
        doc
    }

    #[test]
    fn a_round_trip_keeps_the_layers_and_a_foreign_tag_is_refused() {
        let flora = Flora::new(FloraConfig::default());
        let bytes = encode(&flora);
        let back = decode(&bytes).expect("its own format");
        assert_eq!(back.view().stands.len(), flora.view().stands.len());

        let mut wrong = postcard::to_stdvec(&(SCHEMA + 1)).expect("a tag");
        wrong.extend_from_slice(&bytes[wrong.len().min(bytes.len())..]);
        let err = format!("{:#}", decode(&wrong).expect_err("a foreign schema must be refused"));
        assert!(err.contains("another version"), "{err}");

        let err = format!("{:#}", decode(&bytes[..bytes.len() / 2]).expect_err("half a state"));
        assert!(err.contains("does not read as schema"), "{err}");
    }

    /// Every value of a config at a fine cell size comes back, rescaled fields included.
    #[test]
    fn a_round_trip_keeps_every_config_value() {
        let config = FloraConfig::default().rescaled_to(0.125);
        let flora = Flora::new(config.clone());
        assert_eq!(decode(&encode(&flora)).unwrap().config(), &config);
    }

    /// A save written before a knob existed: the knob takes its default **at the save's
    /// cell size**. Every knob of the default config, one at a time, and one through the
    /// whole envelope.
    #[test]
    fn a_save_without_a_knob_takes_its_default() {
        assert_eq!(
            cubarium_voxel::savedoc::knobs_a_save_cannot_lack(&FloraConfig::default()),
            Vec::<String>::new()
        );
        let config = FloraConfig::default().rescaled_to(0.125);
        let flora = Flora::new(config.clone());
        let doc = edited_doc(&config, |m| {
            // A table of a species: drop one of its knobs.
            let (_, species) = m
                .iter_mut()
                .find(|(_, v)| matches!(v, ciborium::Value::Map(_)))
                .expect("a species table");
            let ciborium::Value::Map(knobs) = species else { unreachable!() };
            knobs.pop();
        });
        let back = decode(&with_doc(&flora, doc)).expect("a save without a knob loads");
        assert_eq!(back.config(), &config);
    }

    #[test]
    fn a_save_with_an_unknown_knob_loads() {
        let flora = Flora::new(FloraConfig::default());
        let doc = edited_doc(flora.config(), |m| {
            m.push(("a_knob_from_another_build".into(), 1.into()));
        });
        let back = decode(&with_doc(&flora, doc)).expect("an unknown knob is dropped");
        assert_eq!(back.config(), flora.config());
    }
}
