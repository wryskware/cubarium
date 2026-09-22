use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};

use crate::recipe::Landform;

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
    /// Whether the world runs a **closed** water budget: evaporation, transpiration and
    /// the outlet's export are deposited into [`crate::World::atmosphere_m3`] instead of
    /// leaving the world, and rain falls only as showers drawn back out of that store.
    /// Nothing but [`crate::Ledger::displaced_out`] leaves a closed world.
    ///
    /// False — the default — is the **open** flow-through budget every world had before:
    /// prescribed rain every tick from nowhere, evaporation and the outlet to nowhere.
    /// The flora study harness and every water fixture run on it, unchanged.
    pub closed_water_budget: bool,
    /// Water aloft at creation, cubic metres. A closed world with a dry atmosphere and a
    /// dry outlet cell has nothing to start a shower with; this is the water the cycle
    /// begins holding. Counted in [`crate::Ledger::initial_atmosphere`], never in
    /// `initial_stored`. Ignored under the open budget.
    pub initial_atmosphere_m3: f64,
    /// Closed budget: the fraction of the world's **total** water the atmosphere store
    /// must hold before a shower starts. Between showers no rain falls at all.
    ///
    /// Not tuned. It is the one knob that decides whether a world cycles or locks dry:
    /// set it above the share of the world's water the return flows can actually lift,
    /// and the last shower is the last shower.
    pub shower_trigger_fraction: f64,
    /// Closed budget: how much water one shower delivers, cubic metres, falling at
    /// [`Config::rain_m_per_s`]. The shower ends when this much has fallen, when the
    /// store empties, or when a brim-full world stops accepting any of it.
    pub shower_volume_m3: f64,
    /// Closed budget: the shortest and longest gap between showers, in **seconds of
    /// simulated time**. `0 / 0`, the default, is **no scheduler**: showers fire whenever
    /// the store crosses the trigger, which is what every fixture had before.
    ///
    /// With a positive maximum the world draws its next shower time from its own seed —
    /// `end of the last shower + U[min, max]` — and
    /// [`Config::shower_trigger_fraction`] stops being a *trigger* and becomes an
    /// **availability floor**: a due shower falls if the store can pay for it and waits
    /// for the first later tick it can otherwise. A timer decides *when* it rains; the
    /// store still decides *whether* it can. An empty sky never rains, which is the
    /// drought lock kept as a feature.
    pub shower_interval_min_s: f64,
    pub shower_interval_max_s: f64,
    /// Closed budget: water returned from the atmosphere store as a **stream at the spring
    /// cell**, cubic metres per second, while the store stands above the shower floor.
    /// The river that leaves at the outlet re-enters upstream. Zero — the default — is
    /// showers only. Ignored under the open budget.
    pub reentry_m3_per_s: f64,
    /// Largest change in one cell's `free` fraction that a single equalization substep
    /// may apply, so a filling region can be watched travelling. Zero (the default)
    /// disables the cap and a region settles to its level in one substep.
    pub free_transfer_cap: f64,
    /// Which generator builds the terrain, and — for
    /// [`crate::Landform::Staged`] — the recipe it builds from, in metres.
    ///
    /// [`crate::Landform::Ridge`], the default, is the original generator, byte for
    /// byte: a config that never mentions a landform gets the world it always had.
    pub landform: Landform,
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
            closed_water_budget: false,
            initial_atmosphere_m3: 0.0,
            shower_trigger_fraction: 0.02,
            shower_volume_m3: 5.0,
            shower_interval_min_s: 0.0,
            shower_interval_max_s: 0.0,
            reentry_m3_per_s: 0.0,
            free_transfer_cap: 0.0,
            landform: Landform::Ridge,
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
            ("initial_atmosphere_m3", self.initial_atmosphere_m3),
            ("shower_trigger_fraction", self.shower_trigger_fraction),
            ("shower_volume_m3", self.shower_volume_m3),
            ("shower_interval_min_s", self.shower_interval_min_s),
            ("shower_interval_max_s", self.shower_interval_max_s),
        ] {
            ensure!(
                rate.is_finite() && rate >= 0.0,
                "{name} must be finite and not negative, not {rate}"
            );
        }
        ensure!(
            self.shower_trigger_fraction <= 1.0,
            "shower_trigger_fraction is a fraction of the world's water, not {}",
            self.shower_trigger_fraction
        );
        ensure!(
            self.water_substeps >= 1,
            "water_substeps must be at least 1, not 0"
        );
        ensure!(
            self.shower_interval_min_s <= self.shower_interval_max_s,
            "the shortest gap between showers cannot be longer than the longest: {} > {}",
            self.shower_interval_min_s,
            self.shower_interval_max_s
        );
        if let Landform::Staged(recipe) = &self.landform {
            recipe.validate().context("the staged landform recipe")?;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The shower interval is a range, so it has to be one: no negatives, nothing
    /// backwards, and zero to zero is the "no schedule" every fixture runs on.
    #[test]
    fn the_shower_interval_must_be_a_range() {
        let with = |min, max| Config {
            shower_interval_min_s: min,
            shower_interval_max_s: max,
            ..Config::default()
        };
        with(0.0, 0.0)
            .validate()
            .expect("no schedule is a schedule");
        with(300.0, 900.0)
            .validate()
            .expect("five to fifteen minutes");
        with(600.0, 600.0).validate().expect("a fixed gap");

        let err = format!("{:#}", with(900.0, 300.0).unwrap_err_msg());
        assert!(err.contains("cannot be longer"), "{err}");
        assert!(
            with(-1.0, 300.0).validate().is_err(),
            "a negative shortest gap"
        );
        assert!(
            with(300.0, -1.0).validate().is_err(),
            "a negative longest gap"
        );
        assert!(
            with(f64::NAN, 300.0).validate().is_err(),
            "a gap that is not a number"
        );
    }

    trait UnwrapErrMsg {
        fn unwrap_err_msg(self) -> anyhow::Error;
    }
    impl UnwrapErrMsg for Config {
        fn unwrap_err_msg(self) -> anyhow::Error {
            self.validate().expect_err("this config is invalid")
        }
    }
}
