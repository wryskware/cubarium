use anyhow::{Context, ensure};
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
    /// Seed for the landform's final weak correlated wobble alone. Zero — the default —
    /// draws it from the one `seed` stream exactly as before; any other value draws the
    /// wobble from its own stream while the main stream advances identically, so the
    /// ridge phase, the strata warp and the soil pockets do not move. That is what makes
    /// the terrain-coupling experiment possible: re-draw the noise, keep the landform.
    pub noise_seed: u64,
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
    ///
    /// It must be at least soil's own pore capacity (0.35) for the water table to hold:
    /// a metre of head is `footprint * aquifer_porosity` cubic metres, and if that is
    /// less than one row of saturated soil holds, the aquifer collapses while trying to
    /// fill the ground it is supposed to be holding up. Not tuned; a consistency floor.
    pub aquifer_porosity: f64,
    /// Level of the water table at creation, in metres above `y = 0`. The aquifer is
    /// charged to the volume whose head is this, and that water is counted in
    /// `Ledger::initial_stored` like any other water the world starts with.
    ///
    /// Zero, the default, is a dry aquifer and the behaviour every world had before the
    /// water table existed. Set it to a little above the basin floor and the basin's
    /// soil is saturated from below and seeps a pond, while ground above the level
    /// drains to its field capacity as usual.
    pub initial_aquifer_head_m: f64,
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
            depth: 24,
            voxel_m: 0.25,
            seed: 1,
            noise_seed: 0,
            rain_m_per_s: 0.0,
            evaporation_m_per_s: 0.0,
            water_substeps: 4,
            spring_k_m2_per_s: 0.02,
            aquifer_porosity: 0.35,
            initial_aquifer_head_m: 0.0,
            outlet_m3_per_s: 0.05,
            free_transfer_cap: 0.0,
        }
    }
}

impl Config {
    /// Refuse a config no world can be built from: zero dimensions, a cell count that
    /// does not fit a `usize`, a voxel size or aquifer porosity that is not positive and
    /// finite, a rate that is negative or not finite, or fewer than one water substep.
    ///
    /// [`crate::World::new`] and [`crate::World::empty`] panic on a failure — building a
    /// world from nonsense is a programming error — and [`crate::World::load`] returns
    /// it, because a snapshot is input. Everything past this point may divide by a
    /// dimension and index with [`Config::index`] without checking again.
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.width > 0 && self.height > 0 && self.depth > 0,
            "a voxel world needs nonzero dimensions, not {}x{}x{}",
            self.width,
            self.height,
            self.depth
        );
        (self.width as usize)
            .checked_mul(self.height as usize)
            .and_then(|n| n.checked_mul(self.depth as usize))
            .with_context(|| {
                format!(
                    "{}x{}x{} voxels overflows a cell count",
                    self.width, self.height, self.depth
                )
            })?;
        for (name, value) in [
            ("voxel_m", self.voxel_m),
            ("aquifer_porosity", self.aquifer_porosity),
        ] {
            ensure!(
                value.is_finite() && value > 0.0,
                "{name} must be positive and finite, not {value}"
            );
        }
        for (name, rate) in [
            ("rain_m_per_s", self.rain_m_per_s),
            ("evaporation_m_per_s", self.evaporation_m_per_s),
            ("spring_k_m2_per_s", self.spring_k_m2_per_s),
            ("outlet_m3_per_s", self.outlet_m3_per_s),
            ("free_transfer_cap", self.free_transfer_cap),
            ("initial_aquifer_head_m", self.initial_aquifer_head_m),
        ] {
            ensure!(
                rate.is_finite() && rate >= 0.0,
                "{name} must be finite and not negative, not {rate}"
            );
        }
        ensure!(
            self.water_substeps >= 1,
            "water_substeps must be at least 1, not 0"
        );
        Ok(())
    }

    /// Number of voxels. Valid configs cannot overflow it; see [`Config::validate`].
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
        aquifer_m3 / self.aquifer_pore_m3()
    }

    /// The aquifer store whose head is `head_m`: the inverse of
    /// [`Config::aquifer_head_m`].
    pub fn aquifer_volume_for_head(&self, head_m: f64) -> f64 {
        self.aquifer_pore_m3() * head_m.max(0.0)
    }

    /// Cubic metres of aquifer pore space per metre of head.
    fn aquifer_pore_m3(&self) -> f64 {
        let area = self.width as f64 * self.depth as f64 * self.cell_area();
        (area * self.aquifer_porosity).max(1e-12)
    }
}
