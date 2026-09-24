//! Package V, authored first and by a separate pass: the **one source of numbers**
//! (`design/handoffs/voxel-organism-models-2026-09-23.md` §1). The Blender bake reads a
//! JSON dump of the model's own numbers instead of keeping copies. This checks that the
//! dump reads back as exactly those numbers.
//!
//! **This does not compile until the API below exists.** It lives in the host crate
//! because the dump needs both the flora (species) and the fauna (founders), and only
//! `cubarium` depends on both. The example that writes `assets/voxel-models/source.json`
//! serialises this same value with `serde_json`. The test never reads that checked-in
//! file, because pinning a generated file is what `WORKING_POLICY.md` rules out; it
//! round-trips the in-memory dump.
//!
//! # The API this test assumes: `cubarium::voxel::model`
//!
//! ```ignore
//! #[derive(Serialize, Deserialize)]
//! pub struct SourceDump { pub species: Vec<SpeciesSource>, pub founders: Vec<FounderSource> }
//! impl SourceDump {
//!     pub fn from_model(flora: &FloraConfig, fauna: &FaunaConfig) -> SourceDump;
//! }
//! /// One per `Species::ALL`, `name` = `Species::name()`.
//! pub struct SpeciesSource {
//!     pub name: String,
//!     pub crown_height_m: [f64; 2],
//!     pub crown_radius_m: [f64; 2],
//!     pub profile: Vec<Stage>,
//! }
//! /// The stage's fields: flora's own `Profile` or a mirror of it with these names.
//! ///   wood_fraction_max: f64, height_m_max: Option<f64> (the seedling cap),
//! ///   layers: Vec<{ kind: LayerKind, band: [f64; 2], radius, share, porosity: f64 }>
//! /// One per `Founder::ALL`, `name` = `Founder::name()`.
//! pub struct FounderSource {
//!     pub name: String,
//!     pub adult_length_m: f64, pub adult_width_m: f64, pub adult_height_m: f64,
//! }
//! ```
//!
//! **A trap this test catches.** Every species' last stage has `wood_fraction_max =
//! f64::INFINITY` (`SpeciesConfig::one_stage` and the staged species alike), and
//! `serde_json` writes a non-finite `f64` as `null`, which does not read back as an
//! `f64`. The dump needs a representation that reads back as infinity. A plain
//! `serde_json` default may also be off by one ulp on long decimals; the
//! `float_roundtrip` feature fixes that if it ever bites.

use cubarium::voxel::model::SourceDump;
use cubarium_voxel_fauna::{FaunaConfig, Founder};
use cubarium_voxel_flora::{FloraConfig, Species};

/// For every species, including package N's vaulttree, lanternberry and siphonreed, the
/// dump reads back the crown ranges in metres and every stage (threshold, seedling cap,
/// and each layer's kind, band, radius, share and porosity), exactly. For every founder
/// it reads back the adult dimensions exactly.
#[test]
fn the_source_dump_reads_back_every_species_and_founder_number_exactly() {
    let flora = FloraConfig::default();
    let fauna = FaunaConfig::default();
    let json = serde_json::to_string_pretty(&SourceDump::from_model(&flora, &fauna))
        .expect("the dump serialises");
    let back: SourceDump = serde_json::from_str(&json).expect("the dump deserialises");

    assert_eq!(
        back.species.len(),
        Species::ALL.len(),
        "one entry per species"
    );
    for sp in Species::ALL {
        let name = sp.name();
        let sc = flora.species(sp);
        let got = back
            .species
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("{name} is missing from the dump"));
        assert_eq!(
            got.crown_height_m, sc.crown_height_m,
            "{name} crown_height_m"
        );
        assert_eq!(
            got.crown_radius_m, sc.crown_radius_m,
            "{name} crown_radius_m"
        );
        assert_eq!(got.profile.len(), sc.profile.len(), "{name} stage count");
        for (i, (g, m)) in got.profile.iter().zip(&sc.profile).enumerate() {
            assert_eq!(
                g.wood_fraction_max, m.wood_fraction_max,
                "{name} stage {i} wood_fraction_max"
            );
            assert_eq!(
                g.height_m_max, m.height_m_max,
                "{name} stage {i} seedling cap"
            );
            assert_eq!(
                g.layers.len(),
                m.layers.len(),
                "{name} stage {i} layer count"
            );
            for (j, (gl, ml)) in g.layers.iter().zip(&m.layers).enumerate() {
                let at = format!("{name} stage {i} layer {j}");
                assert_eq!(gl.kind, ml.kind, "{at} kind");
                assert_eq!(gl.band, ml.band, "{at} band");
                assert_eq!(gl.radius, ml.radius, "{at} radius");
                assert_eq!(gl.share, ml.share, "{at} share");
                assert_eq!(gl.porosity, ml.porosity, "{at} porosity");
            }
        }
    }
    for name in ["vaulttree", "lanternberry", "siphonreed"] {
        assert!(
            back.species.iter().any(|s| s.name == name),
            "package N's {name} is in the dump"
        );
    }
    assert!(
        back.species
            .iter()
            .any(|s| s.profile.first().is_some_and(|p| p.height_m_max.is_some())),
        "at least one species opens with a capped seedling, so the cap is exercised"
    );

    assert_eq!(
        back.founders.len(),
        Founder::ALL.len(),
        "one entry per founder"
    );
    for f in Founder::ALL {
        let name = f.name();
        let phys = fauna.founder(f);
        let got = back
            .founders
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("{name} is missing from the dump"));
        assert_eq!(got.adult_length_m, phys.adult_length_m, "{name} length");
        assert_eq!(got.adult_width_m, phys.adult_width_m, "{name} width");
        assert_eq!(got.adult_height_m, phys.adult_height_m, "{name} height");
    }
}
