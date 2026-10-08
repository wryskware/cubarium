//! Save/load: the world's **state** as postcard bytes behind a schema tag, and its **config**
//! beside it as a named-field document ([`crate::savedoc`]). A different state tag is refused
//! as another version; a config knob added or dropped since the save is not.
//!
//! The envelope is `(schema, config document, state)`. The config is read over
//! [`Config::default`]: a knob the save lacks takes its default, one this build does not
//! know is dropped with a note.

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::{Config, World};

/// Bumped to 2 for the plant boundary (`Ledger::transpiration_out`,
/// `World::terrain_version`) and to 3 for the water table
/// (`Config::initial_aquifer_head_m`, which sits inside the serialized world), and to 4
/// for the closed water budget (`World::atmosphere_m3`, `World::shower_left_m3`, the four
/// new `Ledger` terms and its shower count, and the `Config` switch with its two shower
/// knobs), and to 5 for the terrain generation recipe (`Config::landform`, which carries
/// a whole `Recipe` inside the serialized world), and to 6 for erosion and the hardness
/// field (the `Recipe` gained its `erosion` and `hollows` sections and its hardness
/// parameters, and lost the slope-derived soil the staged generator no longer uses).
/// and to 7 for carved hollows (the `Recipe`'s `hollows` section grew from an empty
/// marker into the shape the carve reads), and to 8 for layer-aware incision (the
/// `Erosion` section's single per-iteration cap became a pair, chosen by the hardness of
/// the bed being cut), and to 9 for structural benches (the `Recipe` gained its
/// `benches` section), and to 10 for the water inventory (`Recipe.water`), and to 11 for
/// the closed cycle the recipe turns on (five more `Water` fields), and to 12 for the
/// shower schedule (`World::next_shower_tick`, and the interval pair in both `Config` and
/// `Water`), and to 13 for the lake's recipe fields, and to **14** for
/// `Ledger::reentry_in`, the closed cycle's second return path — a stream at the spring
/// cell, and to **16** for the lake's wet shore (`Tiers::shore_m`, `shore_slope` and
/// `shore_lip_m`, package SW), and to **18** for the weather model (WX1: `World::weather`
/// replaces the shower allowance and schedule; `Config` gains `day` and `climate` and
/// loses the four shower knobs; `Ledger::showers` is `rain_events`), and to **19** for WX1b (`Water` and `Config`
/// lose `reentry_floor_fraction`: the stream's floor is RH 0.5), and to **20** for R2-world (every ecological number a knob: `Config` gains `materials`, `standing_support_fill`, `outlet_weir_per_tick` and `Climate` / `DayClock` / `Water` gain the numbers that were consts), and to **21** for the scenario maps' seed gate (`Water::{min_stream_m, min_halls, min_groves}`), and to **22** for the dry season (`Climate` gains `dry_season` and its four knobs). Postcard is not self-describing, so a new field is
/// a new format: earlier tags are refused, never migrated.
///
/// **23: SAVE-EXTENSIBLE** — the config moved out of the positional state into a named-field
/// document beside it.
///
/// **What needs a bump** (Wrysk, 2026-10-08): a change to the *state's* layout (a serialized
/// field of `World` or anything it holds, other than its config, added, removed, reordered or
/// retyped), or a change to what a saved field or knob *means*. **What does not:** adding,
/// removing or retuning a config knob (`Config` and everything inside it) — the document
/// carries names, so an older save takes the new knob's default.
pub const SCHEMA: u32 = 23;

/// **The one-version legacy reader's tag** ([`decode_legacy`]): schema 22, the last whose
/// config sat positionally inside the state, kept so the 2026-10-08 training starts load.
/// Retire it (delete this, [`decode_legacy`] and its arm in [`decode`]) once those saves are
/// rewritten in the current format (`voxel_resave`) or no longer wanted; it can read them only
/// while `Config`'s positional layout is still schema 22's, so the first knob added to the
/// world config after SAVE-EXTENSIBLE retires it in practice.
pub const LEGACY_SCHEMA: u32 = 22;

#[derive(Serialize)]
struct EnvelopeOut<'a> {
    schema: u32,
    config: Vec<u8>,
    world: &'a World,
}

#[derive(Deserialize)]
struct EnvelopeIn {
    _schema: u32,
    config: Vec<u8>,
    world: World,
}

pub fn encode(world: &World) -> Vec<u8> {
    postcard::to_stdvec(&EnvelopeOut {
        schema: SCHEMA,
        config: crate::savedoc::to_doc(&world.config),
        world,
    })
    .expect("a World always serializes")
}

/// A snapshot this build cannot read because the format moved on.
///
/// Its own type, so a caller can tell **stale** from **damaged** without reading the
/// message: a world of an older format holds nothing that can be recovered — the standing
/// rule is fresh, never migrate — while a truncated or corrupt one is a damaged world and
/// has to be named rather than replaced. The text is unchanged; only the type is new.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaMismatch {
    /// The schema the file claims.
    pub found: u32,
    /// The schema this build writes and reads.
    pub expected: u32,
}

impl std::fmt::Display for SchemaMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "voxel snapshot schema {} is not {}: made by another version of Cubarium whose world \
             state has another layout; start a fresh world",
            self.found, self.expected
        )
    }
}

impl std::error::Error for SchemaMismatch {}

/// The schema tag at the front of any saved layer (world, plants or animals), if it has one.
pub fn schema_tag(bytes: &[u8]) -> Option<u32> {
    postcard::take_from_bytes::<u32>(bytes).ok().map(|(t, _)| t)
}

pub fn decode(bytes: &[u8]) -> anyhow::Result<World> {
    let (tag, rest): (u32, &[u8]) =
        postcard::take_from_bytes(bytes).context("not a voxel world snapshot")?;
    let mut world = match tag {
        SCHEMA => {
            let env: EnvelopeIn = postcard::from_bytes(bytes).with_context(|| {
                format!(
                    "the voxel world state does not read as schema {SCHEMA} (a damaged file, or a \
                     build that changed the state layout without bumping the schema)"
                )
            })?;
            let (config, ignored) = crate::savedoc::from_doc(&env.config, &Config::default())
                .context("the voxel world snapshot's config")?;
            crate::savedoc::note_ignored("voxel world", &ignored);
            let mut world = env.world;
            world.config = config;
            world
        }
        LEGACY_SCHEMA => decode_legacy(rest)?,
        found => {
            return Err(SchemaMismatch {
                found,
                expected: SCHEMA,
            }
            .into());
        }
    };
    world
        .validate_loaded()
        .context("invalid voxel world snapshot")?;
    // The water active sets are not serialized — they are a cache of the arrays — so a
    // decoded world builds them before anything can iterate them.
    world.rebuild_active_sets();
    Ok(world)
}

/// **One-version legacy reader** ([`LEGACY_SCHEMA`]): a schema-22 world, whose config sat
/// positionally at the head of the state. Postcard frames nothing, so it is the config's
/// bytes followed by exactly the state the current envelope carries.
fn decode_legacy(rest: &[u8]) -> anyhow::Result<World> {
    let (config, state): (Config, &[u8]) = postcard::take_from_bytes(rest).context(
        "a schema-22 (legacy) world whose config this build no longer reads positionally; \
         rewrite it with voxel_resave from a build before the world config changed",
    )?;
    let mut world: World = postcard::from_bytes(state)
        .context("the state of a schema-22 (legacy) world does not read")?;
    world.config = config;
    Ok(world)
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
        assert!(format!("{err:#}").contains("world.width"), "{err:#}");
    }

    /// The envelope with a hand-made config document in it.
    fn with_doc(world: &World, doc: Vec<u8>) -> Vec<u8> {
        postcard::to_stdvec(&super::EnvelopeOut {
            schema: super::SCHEMA,
            config: doc,
            world,
        })
        .unwrap()
    }

    /// The config document as a CBOR map, edited by `f`.
    fn edited_doc(config: &Config, f: impl FnOnce(&mut Vec<(ciborium::Value, ciborium::Value)>)) -> Vec<u8> {
        let mut v = ciborium::Value::serialized(config).unwrap();
        let ciborium::Value::Map(m) = &mut v else { panic!("a config is a map") };
        f(m);
        let mut doc = Vec::new();
        ciborium::into_writer(&v, &mut doc).unwrap();
        doc
    }

    #[test]
    fn a_round_trip_keeps_every_config_value() {
        let mut world = fixture();
        world.config.seed = u64::MAX - 7;
        world.config.rain_m_per_s = 0.1 + 0.2;
        let back = World::load(&world.save()).expect("its own format");
        assert_eq!(back.config, world.config);
        assert_eq!(back, world);
    }

    /// A save written before a knob existed: the knob takes its default. Every knob of the
    /// shipped world config, one at a time (`savedoc::knobs_a_save_cannot_lack`), and one
    /// through the whole envelope.
    #[test]
    fn a_save_without_a_knob_takes_its_default() {
        let shipped = crate::Preset::find("default").unwrap().config();
        assert_eq!(crate::savedoc::knobs_a_save_cannot_lack(&shipped), Vec::<String>::new());
        assert_eq!(crate::savedoc::knobs_a_save_cannot_lack(&Config::default()), Vec::<String>::new());

        let world = fixture();
        let mut dropped = String::new();
        let doc = edited_doc(&world.config, |m| {
            let (k, _) = m.pop().unwrap();
            dropped = k.as_text().unwrap().to_string();
        });
        let back = World::load(&with_doc(&world, doc)).expect("a save without a knob loads");
        assert_eq!(back.config, world.config, "`{dropped}` took its default");
    }

    #[test]
    fn a_save_with_an_unknown_knob_loads() {
        let world = fixture();
        let doc = edited_doc(&world.config, |m| {
            m.push(("a_knob_from_another_build".into(), 3.5.into()));
        });
        let back = World::load(&with_doc(&world, doc)).expect("an unknown knob is dropped");
        assert_eq!(back.config, world.config);
    }

    /// Another state schema is another version, by type and in words; a state that does not
    /// read under this schema's tag is named as that, never as a bare decode error.
    #[test]
    fn a_state_layout_mismatch_refuses_as_another_version() {
        let world = fixture();
        let bytes = world.save();
        let mut other = postcard::to_stdvec(&(super::SCHEMA + 1)).unwrap();
        other.extend_from_slice(&bytes[other.len()..]);
        let err = World::load(&other).expect_err("another schema");
        assert!(err.downcast_ref::<super::SchemaMismatch>().is_some(), "{err:#}");
        assert!(format!("{err:#}").contains("another version"), "{err:#}");

        let mut short = bytes.clone();
        short.truncate(bytes.len() / 2);
        let err = format!("{:#}", World::load(&short).expect_err("half a state"));
        assert!(err.contains("does not read as schema"), "{err}");
    }

    /// The one-version legacy reader: a schema-22 world (its config positional at the head
    /// of the state) loads whole.
    #[test]
    fn the_legacy_reader_loads_a_positional_config_world() {
        let mut world = fixture();
        world.config.seed = 99;
        let legacy = postcard::to_stdvec(&(super::LEGACY_SCHEMA, &world.config, &world)).unwrap();
        let back = World::load(&legacy).expect("a schema-22 world");
        assert_eq!(back, world);
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
