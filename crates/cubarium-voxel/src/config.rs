use serde::{Deserialize, Serialize};

/// Everything a world is generated and stepped from. Physical units: metres and
/// seconds; volumes in cubic metres. Depth is a free choice; the full world is deep.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Voxels along the strip. `x` wraps: `x = width` is `x = 0`.
    pub width: u32,
    /// Voxels of vertical extent. `y = 0` sits on the impermeable foundation.
    pub height: u32,
    /// Voxels into the habitat. Front (`z = 0`) and back are no-flow walls.
    pub depth: u32,
    /// Edge length of one voxel in metres.
    pub voxel_m: f64,
    /// Seed for every deterministic stream (landform, soil, water events).
    pub seed: u64,
    /// Prescribed rain onto exposed top surfaces, metres of water per second.
    pub rain_m_per_s: f64,
    /// Prescribed evaporation from exposed free-water surfaces, metres per second.
    pub evaporation_m_per_s: f64,
    /// Water substeps per tick for the free-water solver.
    pub water_substeps: u32,
    /// Spring conductance from the aquifer into the spring cell, square metres per
    /// second: `Q = spring_k_m2_per_s * max(head - h_spring, 0)`.
    pub spring_k_m2_per_s: f64,
    /// Fraction of the world footprint the aquifer store occupies, used to turn its
    /// volume into a head in metres above `y = 0`.
    pub aquifer_porosity: f64,
    /// Free water an open outlet exports, cubic metres per second.
    pub outlet_m3_per_s: f64,
    /// Largest change in one cell's `free` fraction that a single equalization substep
    /// may apply, so a filling region can be watched travelling. Zero (the default)
    /// disables the cap and a region settles to its level in one substep.
    pub free_transfer_cap: f64,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            width: 128,
            height: 48,
            depth: 16,
            voxel_m: 0.25,
            seed: 1,
            rain_m_per_s: 0.0,
            evaporation_m_per_s: 0.0,
            water_substeps: 4,
            spring_k_m2_per_s: 0.02,
            aquifer_porosity: 0.1,
            outlet_m3_per_s: 0.05,
            free_transfer_cap: 0.0,
        }
    }
}

impl Config {
    /// Number of voxels.
    pub fn cells(&self) -> usize {
        self.width as usize * self.height as usize * self.depth as usize
    }

    /// Flat index of a voxel. `x` is wrapped; `y` and `z` must be in range.
    #[inline]
    pub fn index(&self, x: i64, y: u32, z: u32) -> usize {
        debug_assert!(y < self.height && z < self.depth);
        let x = x.rem_euclid(self.width as i64) as usize;
        (y as usize * self.depth as usize + z as usize) * self.width as usize + x
    }

    /// Inverse of [`Config::index`]: `(x, y, z)`.
    #[inline]
    pub fn coords(&self, i: usize) -> (u32, u32, u32) {
        let w = self.width as usize;
        let d = self.depth as usize;
        let x = i % w;
        let rest = i / w;
        let z = rest % d;
        let y = rest / d;
        (x as u32, y as u32, z as u32)
    }

    /// Volume of one voxel in cubic metres.
    pub fn voxel_volume(&self) -> f64 {
        self.voxel_m * self.voxel_m * self.voxel_m
    }

    /// Footprint of one voxel column in square metres.
    pub fn cell_area(&self) -> f64 {
        self.voxel_m * self.voxel_m
    }

    /// Aquifer head in metres above `y = 0` for a given store, from the world footprint
    /// and [`Config::aquifer_porosity`].
    pub fn aquifer_head_m(&self, aquifer_m3: f64) -> f64 {
        let area = self.width as f64 * self.depth as f64 * self.cell_area();
        let pore = (area * self.aquifer_porosity).max(1e-12);
        aquifer_m3 / pore
    }
}
