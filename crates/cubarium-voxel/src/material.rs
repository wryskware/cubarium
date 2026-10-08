use serde::{Deserialize, Serialize};

/// What a voxel is made of. Solid materials block bodies and free water; only their
/// pore space holds water. `Air` is empty space free water can occupy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, bytemuck::NoUninit)]
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
/// appears exactly once, in [`Materials::conductivity_m_per_s`], as the factor that
/// turns the authored fraction into metres per second. It is **not** a tuning knob
/// and not a world parameter: changing it would rescale every material's
/// conductivity, which is what `permeability_per_s` is for.
pub const REFERENCE_VOXEL_M: f64 = 0.25;

/// The numbers one porous material carries (`[world.materials.rock]`,
/// `[world.materials.soil]`). Air and the bedrock foundation hold no water and have none.
///
/// Authored on the reference 0.25 m cell ([`REFERENCE_VOXEL_M`]); transport reads them
/// through [`Materials::conductivity_m_per_s`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialProps {
    /// Fraction of the voxel's volume that can hold pore water.
    pub pore_capacity: f64,
    /// Fraction of the voxel's pore capacity that can move per second **on the reference
    /// cell**.
    ///
    /// This is a raw authored constant, not a flux and not a rate anything may be
    /// scaled by directly: on its own it says nothing about how fast water actually
    /// moves through a metre of ground, because "the voxel" is the discretisation
    /// and not the physics. Use [`Materials::conductivity_m_per_s`] for every
    /// transport term (`design/handoffs/voxel-water-units-2026-09-22.md`, package
    /// 1c); the only other legitimate reading of this number is as the constant
    /// `conductivity_m_per_s` is authored from.
    pub permeability_per_s: f64,
    /// Fraction of the pore capacity the material holds against gravity. Only pore
    /// water above this drains downward; the rest stays put until it evaporates or a
    /// plant takes it.
    ///
    /// Soil **0.65**: drained soil keeps 0.65 × 0.35 = 22.75 % of its volume in water,
    /// the middle of the 20–25 % a loam holds
    /// (`design/handoffs/voxel-plant-viability-2026-09-23.md`, W). It was 0.25, 8.75 % —
    /// sand — and plants starved in full light on drained ground. Rock keeps its crevice
    /// 0.5: nothing measured argues for moving it.
    pub field_capacity: f64,
    /// Fraction of the pore capacity held too tightly for roots to draw. Pore water
    /// between this and [`MaterialProps::field_capacity`] is what drained ground offers a
    /// plant.
    ///
    /// Soil **0.28**: 0.28 × 0.35 = 9.8 % of the volume is held below it, the ~10 % a
    /// loam keeps from roots, which leaves 13 % of the volume — 0.37 of the pores — for
    /// plants between a shower and the wilting point. Rock's 0.2 is the provisional
    /// crevice number, under its field capacity as the rule needs.
    pub wilting_point: f64,
}

impl MaterialProps {
    /// Air and bedrock: no pore space, no flow.
    pub const NONE: MaterialProps = MaterialProps {
        pore_capacity: 0.0,
        permeability_per_s: 0.0,
        field_capacity: 0.0,
        wilting_point: 0.0,
    };
    /// Hard rock: solid, tiny pore capacity, near-zero permeability.
    pub const ROCK: MaterialProps = MaterialProps {
        pore_capacity: 0.02,
        permeability_per_s: 0.001,
        field_capacity: 0.5,
        wilting_point: 0.2,
    };
    /// Soil: solid to bodies, holds pore water, lets water infiltrate and drain.
    pub const SOIL: MaterialProps = MaterialProps {
        pore_capacity: 0.35,
        permeability_per_s: 0.2,
        field_capacity: 0.65,
        wilting_point: 0.28,
    };
}

/// `[world.materials]`: what the two porous materials hold and how fast they pass water.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Materials {
    /// Hard rock.
    pub rock: MaterialProps,
    /// Soil.
    pub soil: MaterialProps,
}

impl Materials {
    /// The numbers every world had before they were knobs.
    pub const DEFAULT: Materials = Materials {
        rock: MaterialProps::ROCK,
        soil: MaterialProps::SOIL,
    };

    /// One material's numbers (`Air` and `Bedrock` have none).
    #[inline]
    pub fn props(&self, m: Material) -> &MaterialProps {
        match m {
            Material::Air | Material::Bedrock => &MaterialProps::NONE,
            Material::Rock => &self.rock,
            Material::Soil => &self.soil,
        }
    }

    /// Fraction of the voxel's volume that can hold pore water.
    #[inline]
    pub fn pore_capacity(&self, m: Material) -> f64 {
        self.props(m).pore_capacity
    }

    /// See [`MaterialProps::permeability_per_s`].
    #[inline]
    pub fn permeability_per_s(&self, m: Material) -> f64 {
        self.props(m).permeability_per_s
    }

    /// See [`MaterialProps::field_capacity`].
    #[inline]
    pub fn field_capacity(&self, m: Material) -> f64 {
        self.props(m).field_capacity
    }

    /// See [`MaterialProps::wilting_point`].
    #[inline]
    pub fn wilting_point(&self, m: Material) -> f64 {
        self.props(m).wilting_point
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
    #[inline]
    pub fn conductivity_m_per_s(&self, m: Material) -> f64 {
        self.permeability_per_s(m) * self.pore_capacity(m) * REFERENCE_VOXEL_M
    }
}

impl Default for Materials {
    fn default() -> Materials {
        Materials::DEFAULT
    }
}

impl Material {
    pub fn is_solid(self) -> bool {
        !matches!(self, Material::Air)
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
        for m in ALL
            .into_iter()
            .filter(|m| Materials::DEFAULT.pore_capacity(*m) > 0.0)
        {
            let (wp, fc) = (
                Materials::DEFAULT.wilting_point(m),
                Materials::DEFAULT.field_capacity(m),
            );
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
        let retained = Materials::DEFAULT.field_capacity(s) * Materials::DEFAULT.pore_capacity(s);
        let locked = Materials::DEFAULT.wilting_point(s) * Materials::DEFAULT.pore_capacity(s);
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
