use serde::{Deserialize, Serialize};

/// What a voxel is made of. Solid materials block bodies and free water; only their
/// pore space holds water. `Air` is empty space free water can occupy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Material {
    Air = 0,
    /// The impermeable foundation and the deep core. Holds no water.
    Bedrock = 1,
    /// Hard rock: solid, tiny pore capacity, near-zero permeability.
    Rock = 2,
    /// Soil: solid to bodies, holds pore water, lets water infiltrate and drain.
    Soil = 3,
}

/// The cell the material constants were authored on.
///
/// `permeability_per_s` is a fraction of a cell per second, so reading it as a
/// physical speed only ever made sense at one cell size; this is that size, and it
/// appears exactly once, in [`Material::conductivity_m_per_s`], as the factor that
/// turns the authored fraction into metres per second. It is **not** a tuning knob
/// and not a world parameter: changing it would rescale every material's
/// conductivity, which is what `permeability_per_s` is for.
pub const REFERENCE_VOXEL_M: f64 = 0.25;

impl Material {
    pub fn is_solid(self) -> bool {
        !matches!(self, Material::Air)
    }

    /// Fraction of the voxel's volume that can hold pore water.
    pub fn pore_capacity(self) -> f64 {
        match self {
            Material::Air | Material::Bedrock => 0.0,
            Material::Rock => 0.02,
            Material::Soil => 0.35,
        }
    }

    /// Fraction of the voxel's pore capacity that can move per second **on the
    /// reference cell**.
    ///
    /// This is a raw authored constant, not a flux and not a rate anything may be
    /// scaled by directly: on its own it says nothing about how fast water actually
    /// moves through a metre of ground, because "the voxel" is the discretisation
    /// and not the physics. Use [`Material::conductivity_m_per_s`] for every
    /// transport term (`design/handoffs/voxel-water-units-2026-09-22.md`, package
    /// 1c); the only other legitimate reading of this number is as the constant
    /// `conductivity_m_per_s` is authored from.
    pub fn permeability_per_s(self) -> f64 {
        match self {
            Material::Air | Material::Bedrock => 0.0,
            Material::Rock => 0.001,
            Material::Soil => 0.2,
        }
    }

    /// **Hydraulic conductivity in metres per second**: how fast water moves through
    /// this material, with no cell in it.
    ///
    /// Every permeability-driven flux in the solver is `K · A · dt` — a conductivity
    /// times the shared face area in m² times the timestep. Before package 1c the
    /// solver instead moved `permeability_per_s · dt · pore_capacity · voxel_volume`,
    /// a *fraction of a cell* per tick, so the physical speed of infiltration,
    /// drainage and the aquifer exchange halved when the cell halved and the panel's
    /// 0.125 m ring drained at half the rate the 0.25 m one did (D5,
    /// `design/handoffs/voxel-small-collapse-2026-09-22.md`). Decision §7: authored
    /// geometry and transport are in metres and the cell is the discretisation.
    ///
    /// The conversion is `permeability_per_s · pore_capacity · REFERENCE_VOXEL_M`,
    /// which is the same number the old form produced on a 0.25 m cell — chosen so
    /// the reference grid is unchanged, digit for digit, and nothing is retuned.
    /// Soil is 0.0175 m/s, Rock 5e-6 m/s.
    pub fn conductivity_m_per_s(self) -> f64 {
        self.permeability_per_s() * self.pore_capacity() * REFERENCE_VOXEL_M
    }

    /// Fraction of the pore capacity the material holds against gravity. Only pore
    /// water above this drains downward; the rest stays put until it evaporates or a
    /// plant takes it.
    pub fn field_capacity(self) -> f64 {
        match self {
            Material::Air | Material::Bedrock => 0.0,
            Material::Rock => 0.5,
            Material::Soil => 0.25,
        }
    }

    /// Fraction of the pore capacity held too tightly for roots to draw. Pore water
    /// between this and [`Material::field_capacity`] is what drained ground offers a
    /// plant. Provisional numbers (sand-like, a bit under half of the retained water);
    /// the soil-retention package sets them.
    pub fn wilting_point(self) -> f64 {
        match self {
            Material::Air | Material::Bedrock => 0.0,
            Material::Rock => 0.2,
            Material::Soil => 0.1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Material; 4] = [
        Material::Air,
        Material::Bedrock,
        Material::Rock,
        Material::Soil,
    ];

    /// Every material that holds water holds some of it against gravity, and roots can
    /// draw some of what it holds: `0 < wilting_point < field_capacity < 1`.
    #[test]
    fn every_porous_material_orders_its_wilting_point_under_its_field_capacity() {
        for m in ALL.into_iter().filter(|m| m.pore_capacity() > 0.0) {
            let (wp, fc) = (m.wilting_point(), m.field_capacity());
            assert!(
                0.0 < wp && wp < fc && fc < 1.0,
                "{m:?}: wilting point {wp}, field capacity {fc}"
            );
        }
    }

    /// Drained soil holds loam-like water: 20–25 % of its volume after drainage, about
    /// 10 % of it below the wilting point (the plant-viability brief's decision,
    /// `design/handoffs/voxel-plant-viability-2026-09-23.md`, W). Sand holds ~9 %.
    #[test]
    fn drained_soil_holds_a_loam_share_of_its_volume() {
        let s = Material::Soil;
        let retained = s.field_capacity() * s.pore_capacity();
        let locked = s.wilting_point() * s.pore_capacity();
        assert!(
            (0.20..=0.25).contains(&retained),
            "drained soil holds {retained} of its volume"
        );
        assert!(
            (0.08..=0.12).contains(&locked),
            "{locked} of its volume is held below the wilting point"
        );
    }
}
