use serde::{Deserialize, Serialize};

/// Everything a world is generated and stepped from. Physical units: metres and
/// seconds; volumes in cubic metres. Depth is a free choice; the full world is deep.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
}
