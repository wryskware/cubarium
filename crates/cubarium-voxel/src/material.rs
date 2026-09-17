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

    /// Fraction of the voxel's pore capacity that can move per second: infiltration
    /// from free water above and drainage downward into soil or the aquifer.
    pub fn permeability_per_s(self) -> f64 {
        match self {
            Material::Air | Material::Bedrock => 0.0,
            Material::Rock => 0.001,
            Material::Soil => 0.2,
        }
    }
}
