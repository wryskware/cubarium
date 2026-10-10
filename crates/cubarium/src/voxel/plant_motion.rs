//! `[plants]`: how plants move when the GPU renderer draws them as models
//! (`[figures] plants = "models"`, `cubarium_gpu::plants`). Presentation only: nothing here
//! reaches the simulation, and with `plants = "stamped"` (the default) none of it is read
//! but `turn`.
//!
//! - **Sway**: each plant's top moves sideways in a shared, travelling, gusty breeze
//!   ([`WindConfig`]); a cell moves by its plant's tip times `(height / plant height)^p`,
//!   so the root never moves and a short plant barely does.
//! - **Grow-in**: when a plant appears, grows a step, changes foliage state or goes, its
//!   voxels arrive (or leave) one by one: each scales from its own centre over `grow_s`,
//!   at a moment staggered along the plant in [`GrowOrder`] with some jitter, the whole
//!   change taking `VoxelConfig::foliage_fade_s`.
//!
//! Every number is a knob; the defaults are a starting point for Wrysk's viewing.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Which way a plant's voxels arrive: from the root up (and leave from the top down), or
/// from the plant's axis outward (and leave from the rim in).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrowOrder {
    #[default]
    RootUp,
    CentreOut,
}

/// `[plants.wind]`: the breeze (`cubarium_gpu::plants::Breeze`). One gust packet a
/// period, travelling round the ring; exactly calm between packets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WindConfig {
    /// Seconds (sim time) from one gust packet to the next.
    pub period_s: f64,
    /// Seconds a packet eases in, holds and eases out; the rest of the period is calm.
    pub rise_s: f64,
    pub hold_s: f64,
    pub fall_s: f64,
    /// How deep the flutter inside a gust dips it (0..1), and its period in seconds.
    pub flutter: f64,
    pub flutter_s: f64,
    /// How far a packet's peak may fall below full (0..1), and the period that varies it.
    pub peak_vary: f64,
    pub peak_s: f64,
    /// Whole packet periods the gust front takes to go once round the ring (0: everywhere
    /// at once); negative travels toward −x.
    pub sweeps: i32,
    /// Whole flutter waves round the ring.
    pub flutter_waves: i32,
    /// What a shower and a downpour add to the breeze, each at full rain.
    pub shower_gain: f64,
    pub storm_gain: f64,
}

impl Default for WindConfig {
    fn default() -> WindConfig {
        let b = cubarium_gpu::plants::Breeze::default();
        WindConfig {
            period_s: b.period_s,
            rise_s: b.rise_s,
            hold_s: b.hold_s,
            fall_s: b.fall_s,
            flutter: b.flutter,
            flutter_s: b.flutter_s,
            peak_vary: b.peak_vary,
            peak_s: b.peak_s,
            sweeps: b.sweeps,
            flutter_waves: b.flutter_waves,
            shower_gain: b.shower_gain,
            storm_gain: b.storm_gain,
        }
    }
}

impl WindConfig {
    pub fn breeze(&self) -> cubarium_gpu::plants::Breeze {
        cubarium_gpu::plants::Breeze {
            period_s: self.period_s,
            rise_s: self.rise_s,
            hold_s: self.hold_s,
            fall_s: self.fall_s,
            flutter: self.flutter,
            flutter_s: self.flutter_s,
            peak_vary: self.peak_vary,
            peak_s: self.peak_s,
            sweeps: self.sweeps,
            flutter_waves: self.flutter_waves,
            shower_gain: self.shower_gain,
            storm_gain: self.storm_gain,
        }
    }
}

/// `[plants]`: see the module.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlantConfig {
    /// Layer samples per raster pixel axis, 1..=4: a swaying edge moves by this fraction of
    /// a pixel at a time, blended by coverage. Startup only.
    pub supersample: u32,
    /// Seconds one voxel takes to grow in from nothing, or shrink to nothing.
    pub grow_s: f64,
    /// Which way a plant's voxels arrive ([`GrowOrder`]).
    pub order: GrowOrder,
    /// How much of each voxel's moment is a per-voxel random draw rather than its place in
    /// the order, 0..1.
    pub jitter: f64,
    /// Voxels a plant's top moves at full breeze for each voxel of the plant's height, up to
    /// `max_tip_voxels`: a tall plant sways more, a tuft barely.
    pub bend: f64,
    pub max_tip_voxels: f64,
    /// A cell moves by its plant's tip times `(height above the root / plant height)` to
    /// this power: 1 a straight lean, 2 a bend that leaves the stem near the root alone.
    pub bend_exponent: f64,
    /// Seconds a plant answers the breeze late for each voxel of its height.
    pub lag_s_per_voxel: f64,
    /// Which way the breeze leans plants: 1 toward +x, −1 toward −x.
    pub direction: f64,
    /// Per-species multipliers on the sway, by species name (missing: 1).
    pub species: BTreeMap<String, f64>,
    pub wind: WindConfig,
    /// Whether each plant model is drawn turned by a quarter its stand's id picks
    /// (`model::plant_quarter`), so stands of one species do not all face one way. Read
    /// by the stamp whichever way plants are drawn. Startup only.
    pub turn: bool,
}

impl Default for PlantConfig {
    fn default() -> PlantConfig {
        PlantConfig {
            supersample: 2,
            grow_s: 0.35,
            order: GrowOrder::RootUp,
            jitter: 0.3,
            bend: 0.025,
            max_tip_voxels: 0.75,
            bend_exponent: 2.0,
            lag_s_per_voxel: 0.03,
            direction: 1.0,
            // A glowcap is a fungus in a cave: no breeze reaches it.
            species: BTreeMap::from([("glowcap".to_string(), 0.0)]),
            wind: WindConfig::default(),
            turn: true,
        }
    }
}

impl PlantConfig {
    /// Refuse what the plant pass cannot draw: a sampling factor outside 1..=4, a species
    /// name no species has, a negative or non-finite number where a length or a time goes.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            (1..=cubarium_gpu::plants::MAX_SUPERSAMPLE).contains(&self.supersample),
            "plants.supersample must be 1..={}, not {}",
            cubarium_gpu::plants::MAX_SUPERSAMPLE,
            self.supersample
        );
        let names: Vec<&str> = cubarium_voxel_flora::Species::ALL.iter().map(|s| s.name()).collect();
        for (name, k) in &self.species {
            anyhow::ensure!(
                names.contains(&name.as_str()),
                "plants.species: unknown species {name:?}; valid names: {}",
                names.join(", ")
            );
            anyhow::ensure!(k.is_finite(), "plants.species.{name} must be a finite number");
        }
        let w = &self.wind;
        for (name, v) in [
            ("grow_s", self.grow_s),
            ("jitter", self.jitter),
            ("bend", self.bend),
            ("max_tip_voxels", self.max_tip_voxels),
            ("bend_exponent", self.bend_exponent),
            ("lag_s_per_voxel", self.lag_s_per_voxel),
            ("wind.period_s", w.period_s),
            ("wind.rise_s", w.rise_s),
            ("wind.hold_s", w.hold_s),
            ("wind.fall_s", w.fall_s),
            ("wind.flutter", w.flutter),
            ("wind.flutter_s", w.flutter_s),
            ("wind.peak_vary", w.peak_vary),
            ("wind.peak_s", w.peak_s),
            ("wind.shower_gain", w.shower_gain),
            ("wind.storm_gain", w.storm_gain),
        ] {
            anyhow::ensure!(v.is_finite() && v >= 0.0, "plants.{name} must be a finite number >= 0, not {v}");
        }
        anyhow::ensure!(self.direction.is_finite(), "plants.direction must be finite");
        Ok(())
    }

    /// A species' sway multiplier ([`PlantConfig::species`]).
    pub fn species_gain(&self, species: cubarium_voxel_flora::Species) -> f64 {
        self.species.get(species.name()).copied().unwrap_or(1.0)
    }

    /// How far a plant `height` voxels tall moves its top at full breeze, signed by
    /// [`PlantConfig::direction`].
    pub fn tip_amplitude(&self, height: f64, species: cubarium_voxel_flora::Species) -> f64 {
        self.direction.signum() * (self.bend * height.max(0.0)).min(self.max_tip_voxels) * self.species_gain(species)
    }

    /// How much of its plant's tip a cell `h` voxels above the root of a plant whose top
    /// cell is `top` voxels above it moves by: 0 at the root, 1 at the top, growing
    /// with height.
    pub fn weight(&self, h: i64, top: i64) -> f32 {
        if h <= 0 || top <= 0 {
            return 0.0;
        }
        ((h.min(top) as f64 / top as f64).powf(self.bend_exponent.max(0.0))) as f32
    }

    /// [`Self::weight`] at a height between cells: a horizontal face's, `h` the face's
    /// height over the root (a cell's top face is at its height plus a half). A drape
    /// cord's join moves by it (`cubarium_gpu::cord`).
    pub fn face_weight(&self, h: f64, top: i64) -> f32 {
        if h <= 0.0 || top <= 0 {
            return 0.0;
        }
        ((h.min(top as f64) / top as f64).powf(self.bend_exponent.max(0.0))) as f32
    }

    /// The pass's fixed settings for the renderer.
    pub fn motion(&self) -> cubarium_gpu::plants::PlantMotion {
        cubarium_gpu::plants::PlantMotion {
            supersample: self.supersample.clamp(1, cubarium_gpu::plants::MAX_SUPERSAMPLE),
            grow_s: self.grow_s as f32,
            breeze: self.wind.breeze(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_face_weighs_between_the_two_cells_it_parts() {
        let cfg = PlantConfig::default();
        let top = 20;
        for h in 1..top {
            let face = cfg.face_weight(h as f64 + 0.5, top);
            assert!(cfg.weight(h, top) <= face && face <= cfg.weight(h + 1, top), "h {h}");
        }
        assert_eq!(cfg.face_weight(-0.5, top), 0.0, "under the root");
    }

    #[test]
    fn a_root_never_moves_and_the_bend_grows_with_height() {
        for exponent in [1.0, 2.0, 3.0] {
            let cfg = PlantConfig { bend_exponent: exponent, ..PlantConfig::default() };
            let top = 12;
            assert_eq!(cfg.weight(0, top), 0.0, "the root cell");
            assert_eq!(cfg.weight(top, top), 1.0, "the top cell");
            for h in 0..top {
                assert!(cfg.weight(h + 1, top) > cfg.weight(h, top), "exponent {exponent}: h {h} to {}", h + 1);
            }
        }
        let cfg = PlantConfig::default();
        assert_eq!(cfg.weight(0, 0), 0.0, "a one-cell plant is all root");
    }

    #[test]
    fn a_taller_plant_sways_more_up_to_the_cap_and_a_still_species_not_at_all() {
        let cfg = PlantConfig { species: BTreeMap::from([("glowcap".to_string(), 0.0)]), ..PlantConfig::default() };
        let tree = cubarium_voxel_flora::Species::Vaulttree;
        assert!(cfg.tip_amplitude(10.0, tree) > cfg.tip_amplitude(2.0, tree));
        assert_eq!(cfg.tip_amplitude(1.0e6, tree), cfg.max_tip_voxels, "capped");
        assert_eq!(cfg.tip_amplitude(10.0, cubarium_voxel_flora::Species::Glowcap), 0.0);
        let west = PlantConfig { direction: -1.0, ..cfg.clone() };
        assert_eq!(west.tip_amplitude(10.0, tree), -cfg.tip_amplitude(10.0, tree));
    }

    #[test]
    fn bad_plant_settings_are_refused_with_the_field_named() {
        let bad = PlantConfig { supersample: 0, ..PlantConfig::default() };
        assert!(bad.validate().unwrap_err().to_string().contains("plants.supersample"));
        let bad = PlantConfig { species: BTreeMap::from([("oak".to_string(), 1.0)]), ..PlantConfig::default() };
        assert!(bad.validate().unwrap_err().to_string().contains("oak"));
        let bad = PlantConfig { grow_s: -1.0, ..PlantConfig::default() };
        assert!(bad.validate().unwrap_err().to_string().contains("grow_s"));
        PlantConfig::default().validate().unwrap();
    }

    #[test]
    fn the_plants_section_reads_from_toml_and_rejects_unknown_keys() {
        let cfg: PlantConfig = toml::from_str("grow_s = 0.5\norder = \"centre_out\"\n[wind]\nsweeps = 2\n").unwrap();
        assert_eq!((cfg.grow_s, cfg.order, cfg.wind.sweeps), (0.5, GrowOrder::CentreOut, 2));
        assert!(toml::from_str::<PlantConfig>("sway = 1\n").is_err());
    }
}
