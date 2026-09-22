//! What a landscape is made of, in metres: the generation recipe and the choice of
//! generator.
//!
//! A [`Recipe`] holds **generation-only** parameters — wavelengths, relief, persistence,
//! lacunarity, octave counts, layer thicknesses — and names one seed stream per pass.
//! Runtime water and ecology settings stay in [`Config`]; nothing in here is read after
//! the world exists.
//!
//! Every length is a length. Feature size is fixed by the recipe, never by `width` or
//! `voxel_m`: doubling the width adds landforms of the same size, halving `voxel_m`
//! resolves the same landforms more finely, and octaves finer than
//! [`Recipe::min_feature_voxels`] voxels are dropped rather than aliased onto the grid.
//!
//! The named streams are why a pass can be retuned on its own. Each one is xored into
//! [`Config::seed`], so moving the ridged noise leaves the broad relief, the rocky mask
//! and the strata exactly where they were.

use serde::{Deserialize, Serialize};

use crate::Config;
use std::f64::consts::TAU;

use crate::noise::{Ladder, fbm, resolved_octaves, ridged, ring_cells, ring_noise, smoothstep};

/// Which generator builds the terrain.
///
/// [`Landform::Ridge`] is the original feature-guided generator — one broad ridge, one
/// receiving basin, a weak correlated wobble — and stays the default, so every fixture
/// and harness that never mentions a landform keeps the world it had.
// A `Recipe` is a couple of hundred bytes beside a unit variant. It lives in `Config`,
// which is cloned once per world and never in a hot loop, so the size is not worth an
// indirection that every generator call would then have to follow.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Landform {
    /// The original generator. Unchanged, down to the byte.
    #[default]
    Ridge,
    /// Staged periodic relief in physical units: broad multi-octave relief, ridged noise
    /// warped into rocky regions, and restrained fine detail, voxelised through the same
    /// layering and camera pass.
    Staged(Recipe),
}

/// One deterministic stream per generation pass. Distinct constants, xored into the
/// world seed: retuning one pass does not reshuffle the others.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Streams {
    /// Broad multi-octave relief.
    pub relief: u64,
    /// Ridged noise in the rocky regions.
    pub ridge: u64,
    /// The domain warp applied to the ridged noise.
    pub warp: u64,
    /// Which stretches of the ring are rocky.
    pub rocky: u64,
    /// The rock hardness field: strata warp and the slow regional variation.
    pub hardness: u64,
    /// The soil pockets.
    pub material: u64,
}

impl Default for Streams {
    fn default() -> Streams {
        Streams {
            relief: 0x_5354_4147_5F52_454C,
            ridge: 0x_5354_4147_5F52_4447,
            warp: 0x_5354_4147_5F57_5250,
            rocky: 0x_5354_4147_5F52_4B59,
            hardness: 0x_5354_4147_5F48_5244,
            material: 0x_5354_4147_5F4D_4154,
        }
    }
}

/// The erosion budget. Geological time is [`Erosion::iterations`], never ticks: it is a
/// work budget for a solver, not a duration anything in the world experiences.
///
/// The model water each iteration rains, routes and discards is a modelling tool. It is
/// never the live world's inventory; slice 3 fills the basins from the water budget.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Erosion {
    /// How many iterations the solver runs. `0` is the identity.
    pub iterations: u32,
    /// Model rain per sample column per iteration.
    pub rain: f64,
    /// Sediment one unit of discharge carries per unit of slope. This and `rain` set the
    /// scale together; only their product means anything.
    pub capacity: f64,
    /// Fraction of the carrying deficit taken off the bed each iteration.
    pub erode: f64,
    /// Fraction of the excess load dropped each iteration.
    pub deposit: f64,
    /// How much slower hardness `1` bedrock yields than loose sediment.
    pub bedrock_resistance: f64,
    /// Deepest one iteration may cut at a column whose bed is **soft** — hardness at or
    /// below `hollows.soft_hardness` — in metres.
    ///
    /// This pair is what makes the landscape layered rather than merely rough. A single
    /// cap would let a channel lower everything it runs over at the same rate, and a
    /// hard band over a soft one would never become a bank. Five to one, and the soft
    /// rock under a cap goes while the cap stays: knickpoints, rocky shoulders, steps
    /// with something to undercut.
    pub max_cut_soft_m: f64,
    /// The same, at a column whose bed is hard. Also the stability limit the single cap
    /// used to be: without it a steep column cuts through the floor in one pass.
    pub max_cut_hard_m: f64,
    /// Tangent of the angle of repose for loose sediment.
    pub repose: f64,
    /// Relaxation sweeps of the repose rule per iteration.
    pub repose_sweeps: u32,
}

impl Default for Erosion {
    fn default() -> Erosion {
        Erosion::DEFAULT
    }
}

impl Erosion {
    /// The budget the `default` and `wide` presets run.
    pub const DEFAULT: Erosion = Erosion {
        iterations: 100,
        rain: 1.0,
        capacity: 0.0025,
        erode: 0.5,
        deposit: 0.4,
        bedrock_resistance: 6.0,
        max_cut_soft_m: 0.25,
        max_cut_hard_m: 0.05,
        repose: 0.8,
        repose_sweeps: 2,
    };

    /// The `small` preset's budget.
    ///
    /// Half the cell size, so less cut per iteration — and less again than that: at the
    /// 0.15 m the cell size alone would ask for, incision left the front rough enough
    /// that the skyline pass had to lower 7.8 % of the ring's columns, over the 5 % the
    /// diorama is allowed. The bound is the bound; the cap came down instead.
    pub const SMALL: Erosion = Erosion {
        max_cut_soft_m: 0.05,
        max_cut_hard_m: 0.01,
        ..Erosion::DEFAULT
    };

    /// No erosion at all: the identity.
    pub const NONE: Erosion = Erosion {
        iterations: 0,
        ..Erosion::DEFAULT
    };
}

/// Structural benches: where a hard stratum outcrops in a rocky region, the bedrock
/// surface follows the top of the band instead of the smooth relief.
///
/// Cliffs come from geology, not from the solver. Slice 2c measured what an erosion
/// model can do here — 0.34 m of incision over a whole ring in a hundred iterations, and
/// a surface step the angle of repose caps at less than a voxel — and the answer was
/// nothing. A stratum that outcrops is a ledge because it is harder than what is under
/// it, not because water wore a step into it, and that is a fact about the rock the
/// generator already has.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Benches {
    /// How far the bedrock surface is pulled onto the band top under it, `0..=1`,
    /// weighted by the rocky mask. `0` is the smooth relief slice 1 made; `1` makes the
    /// rocky ground a staircase of band tops, with faces one [`Recipe::strata_m`] tall.
    pub strength: f64,
    /// How much of the rocky mask's own edge is given over to ramping the pull in,
    /// metres. A bench has to end in a ramp, not in a wall dropped across the strip;
    /// this is what keeps the ring walkable where a rocky region stops.
    pub ramp_m: f64,
}

impl Default for Benches {
    fn default() -> Benches {
        Benches::NONE
    }
}

impl Benches {
    /// No benching: the smooth relief.
    pub const NONE: Benches = Benches {
        strength: 0.0,
        ramp_m: 2.0,
    };

    /// `default` and `wide`. Full strength: a stratum that outcrops is a ledge, not a
    /// suggestion of one, and a partial pull leaves a face too shallow for the undercut
    /// pass to find. How *many* benches a ring gets is not this — it is how often the
    /// relief crosses a band inside a rocky region, which is a handful.
    pub const ON: Benches = Benches {
        strength: 1.0,
        ramp_m: 2.0,
    };

    /// `small`. Full strength, like the others: a partial pull on a 1.4 m band leaves a
    /// face too short for a grotto to fit under, and a ledge you cannot get under is
    /// just a slope with a step in it.
    pub const SMALL: Benches = Benches {
        strength: 1.0,
        ramp_m: 0.4,
    };

    pub fn validate(&self) -> anyhow::Result<()> {
        for (name, v) in [("strength", self.strength), ("ramp_m", self.ramp_m)] {
            anyhow::ensure!(
                v.is_finite() && v >= 0.0,
                "benches.{name} must be finite and not negative, not {v}"
            );
        }
        Ok(())
    }
}

/// The rock banding of one column: everything about the hardness field that does not
/// depend on height, so a scan up and down a column evaluates the noise once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strata {
    warp_m: f64,
    region: f64,
    soft: f64,
    span: f64,
    strata_m: f64,
    core_m: f64,
}

impl Strata {
    /// Hardness at a height in this column, `0..=1`.
    pub fn hardness(&self, y_m: f64) -> f64 {
        let band = 0.5 - 0.5 * (TAU * (y_m + self.warp_m) / self.strata_m).cos();
        let layered = self.soft + self.span * (0.72 * band + 0.28 * self.region);
        let core = 1.0 - smoothstep(self.core_m, self.core_m + 0.5, y_m);
        layered.max(core).clamp(0.0, 1.0)
    }

    /// Top of the highest hard band at or below `y_m`, or `None` where the column has no
    /// hard bands at all — all of it hard, or none of it.
    ///
    /// Solved rather than searched. The banding is one cosine in height and everything
    /// else in the column is constant, so the band edges are an arc-cosine away.
    pub fn hard_band_top_m(&self, y_m: f64, threshold: f64) -> Option<f64> {
        if self.span.is_nan() || self.span <= 0.0 {
            return None;
        }
        // Hard is `band >= t`, with the region's share already taken out.
        let t = ((threshold - self.soft) / self.span - 0.28 * self.region) / 0.72;
        if !(0.0..1.0).contains(&t) {
            return None;
        }
        let half = (1.0 - 2.0 * t).clamp(-1.0, 1.0).acos() / TAU;
        let top_phase = 1.0 - half;
        let phase = (y_m + self.warp_m) / self.strata_m;
        let k = (phase - top_phase).floor();
        Some(self.strata_m * (k + top_phase) - self.warp_m)
    }
}

/// Carved hollows: undercuts and grottos, galleries with mouths and skylights, and the
/// shelves that fall out of both (`design/caves-and-hollows-plan-2026-09-21.md`).
///
/// Both densities default to zero, so a recipe that says nothing about hollows gets the
/// solid ring it had before they existed. Lengths are lengths, as everywhere here: the
/// same section on a finer grid carves the same grotto out of more voxels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Hollows {
    /// Void a hollow must have above a floor face before anyone can stand under it,
    /// metres. Less than this is drainage, not habitat, and is not listed as a hollow.
    pub clearance_m: f64,
    /// How hard the rock under a cap has to be *not* to be, for the water to have taken
    /// it: at or below this hardness a band is soft enough to notch out or to hold a
    /// gallery.
    pub soft_hardness: f64,

    /// How steep the ground has to fall toward a neighbour for a column to count as a
    /// bank, in metres of drop per metre along.
    pub bank_slope: f64,
    /// Thickness of the hard cap over an undercut, metres.
    pub cap_thickness_m: f64,
    /// How far below a hard bank a neighbour has to be cut for
    /// [`crate::Heightfield::hard_cap`] to flag it. The second source of undercut sites,
    /// beside the hardness geometry.
    pub cap_drop_m: f64,
    /// How far back into the bank the soft band is notched, metres.
    pub undercut_depth_m: f64,
    /// Share of eligible banks that are notched, `0..=1`. Zero carves none.
    ///
    /// High, because the geology is now what limits this: a bank has to carry a hard cap
    /// over a soft band thick enough for a body, with rock cut below the notch floor
    /// beside it, and a ring offers a dozen or so places like that. The patch field
    /// below is there to keep a long bank from becoming one continuous slot, not to be
    /// the thing that decides how many grottos there are.
    pub undercut_density: f64,
    /// Size of the patches the undercuts come in, metres. Without it every eligible
    /// column of a long bank is notched and the result is a slot, not a few grottos.
    pub grotto_wavelength_m: f64,
    /// How strongly hollows are pulled toward the front cut, `0..=1`. The camera is a
    /// section through `z = 0`: a grotto at the back is a grotto nobody sees.
    pub front_bias: f64,

    /// Share of the soft rock at depth that opens into galleries, `0..=1`. Zero carves
    /// none.
    pub gallery_density: f64,
    /// Size of the galleries, metres.
    pub gallery_wavelength_m: f64,
    /// How far the gallery noise is stretched vertically, `0..1`. Galleries follow the
    /// soft strata, which are a couple of metres thick and tens of metres long; a field
    /// that varies as fast up as along carves bubbles, not passages, and a bubble two
    /// voxels tall is drainage.
    pub gallery_flatten: f64,
    /// How far under the surface a gallery has to start, metres, so it is a passage in
    /// the rock and not a hole in the ground.
    pub gallery_min_depth_m: f64,
    /// How far a mouth may be cut through rock to open a gallery, metres.
    pub mouth_reach_m: f64,
    /// How close a gallery's ceiling has to come to the surface for a skylight instead,
    /// metres. A gallery that gets neither is filled.
    pub skylight_m: f64,

    /// This section's own seed stream.
    pub stream: u64,
}

impl Default for Hollows {
    fn default() -> Hollows {
        Hollows::NONE
    }
}

impl Hollows {
    /// No hollows at all: the solid ring.
    pub const NONE: Hollows = Hollows {
        clearance_m: 0.75,
        soft_hardness: 0.45,
        bank_slope: 1.2,
        cap_thickness_m: 0.4,
        cap_drop_m: 0.75,
        undercut_depth_m: 0.75,
        undercut_density: 0.0,
        grotto_wavelength_m: 6.0,
        front_bias: 0.5,
        gallery_density: 0.0,
        gallery_wavelength_m: 3.0,
        gallery_flatten: 0.3,
        gallery_min_depth_m: 0.6,
        mouth_reach_m: 1.0,
        skylight_m: 1.0,
        stream: 0x_5354_4147_5F48_4C57,
    };

    /// A handful of grottos on the `default` and `wide` rings.
    pub const GROTTOS: Hollows = Hollows {
        undercut_density: 0.70,
        gallery_density: 0.28,
        ..Hollows::NONE
    };

    /// The `small` ring: half the vertical room, so shallower notches and shorter reach.
    /// The clearance is not halved — a body is the size it is, whatever the voxel is.
    pub const SMALL: Hollows = Hollows {
        cap_thickness_m: 0.25,
        cap_drop_m: 0.5,
        undercut_depth_m: 0.5,
        grotto_wavelength_m: 4.0,
        gallery_wavelength_m: 2.0,
        gallery_min_depth_m: 0.4,
        mouth_reach_m: 0.7,
        skylight_m: 0.7,
        ..Hollows::GROTTOS
    };

    /// Whether anything is carved at all.
    pub fn any(&self) -> bool {
        self.undercut_density > 0.0 || self.gallery_density > 0.0
    }

    /// Refuse a section no hollow can be carved from.
    pub fn validate(&self) -> anyhow::Result<()> {
        for (name, v) in [
            ("clearance_m", self.clearance_m),
            ("grotto_wavelength_m", self.grotto_wavelength_m),
            ("gallery_wavelength_m", self.gallery_wavelength_m),
        ] {
            anyhow::ensure!(
                v.is_finite() && v > 0.0,
                "hollows.{name} must be positive and finite, not {v}"
            );
        }
        for (name, v) in [
            ("soft_hardness", self.soft_hardness),
            ("bank_slope", self.bank_slope),
            ("cap_thickness_m", self.cap_thickness_m),
            ("cap_drop_m", self.cap_drop_m),
            ("undercut_depth_m", self.undercut_depth_m),
            ("undercut_density", self.undercut_density),
            ("front_bias", self.front_bias),
            ("gallery_density", self.gallery_density),
            ("gallery_flatten", self.gallery_flatten),
            ("gallery_min_depth_m", self.gallery_min_depth_m),
            ("mouth_reach_m", self.mouth_reach_m),
            ("skylight_m", self.skylight_m),
        ] {
            anyhow::ensure!(
                v.is_finite() && v >= 0.0,
                "hollows.{name} must be finite and not negative, not {v}"
            );
        }
        Ok(())
    }
}

/// The water a fresh world starts with, as one inventory and its split.
///
/// **Inventory first** (`design/terrain-generation-plan-2026-09-21.md` §4.1): the world
/// is charged a single total, `inventory_m` metres of water over its footprint, and the
/// four stores — atmosphere, aquifer, pore and pools — are shares of that one number.
/// Nothing is charged independently to a target of its own, so a world cannot quietly
/// start with more water than its inventory says.
///
/// `inventory_m = 0`, the default, is the dry world every fixture and every `Ridge`
/// config had before: [`crate::hydrate::hydrate`] does nothing at all.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Water {
    /// Total water, metres over the world footprint. Zero is a dry world.
    pub inventory_m: f64,
    /// Share of the inventory held aloft for the cycle, `0..=1`.
    pub atmosphere_fraction: f64,
    /// Water table at creation, metres above `y = 0`. Charged out of the inventory, so
    /// a head the inventory cannot pay for is truncated rather than conjured.
    pub aquifer_head_m: f64,

    /// Whether the world runs the **closed** cycle: route B of
    /// `design/handoffs/voxel-water-cycle-2026-09-20.md` — evaporation, transpiration and
    /// the outlet's export go into the lumped atmosphere store and come back as showers,
    /// and nothing leaves. False keeps the open flow-through budget every fixture has.
    pub closed_cycle: bool,
    /// Rain onto exposed top surfaces while a shower is falling, metres per second.
    pub rain_m_per_s: f64,
    /// Evaporation from open free-water surfaces, metres per second.
    pub evaporation_m_per_s: f64,
    /// Share of the world's total water the atmosphere must hold before a shower starts.
    pub shower_trigger_fraction: f64,
    /// How much one shower delivers, cubic metres.
    pub shower_volume_m3: f64,
    /// Shortest and longest gap between showers, seconds of simulated time. `0 / 0` is
    /// no schedule: showers fire whenever the store crosses the trigger.
    pub shower_interval_min_s: f64,
    pub shower_interval_max_s: f64,

    /// How deep the ring's **lake** — the lowest open-sky basin — stands before it
    /// overflows into the outlet, metres above its floor. The outlet sits at this level on
    /// the lake's rim (not on its floor), so the lake keeps its level and only surplus
    /// leaves; and no hollow is carved with its floor below it, so nothing hidden can act
    /// as a sump. Zero puts the outlet on the floor, the pre-lake behaviour.
    pub lake_depth_m: f64,
    /// Smallest open-water surface the camera must be able to read, square metres, for a
    /// generated world to be accepted. The host draws another seed below it. Zero accepts
    /// any world.
    pub min_lake_m2: f64,
}

impl Default for Water {
    fn default() -> Water {
        Water::DRY
    }
}

impl Water {
    /// No water at all: what a `Ridge` world and every fixture has.
    /// No water at all, and no cycle: what a `Ridge` world and every fixture has. The
    /// five runtime values are [`Config::default`]'s own, so applying this recipe's water
    /// to a config changes nothing.
    pub const DRY: Water = Water {
        inventory_m: 0.0,
        atmosphere_fraction: 0.0,
        aquifer_head_m: 0.0,
        closed_cycle: false,
        rain_m_per_s: 0.0,
        evaporation_m_per_s: 0.0,
        shower_trigger_fraction: 0.02,
        shower_volume_m3: 5.0,
        shower_interval_min_s: 0.0,
        shower_interval_max_s: 0.0,
        lake_depth_m: 0.0,
        min_lake_m2: 0.0,
    };

    /// The staged presets' inventory: half a metre of water over the footprint, six per
    /// cent of it aloft (nothing at all under the **open** budget, which has no
    /// atmosphere store), and a metre of water table.
    ///
    /// A metre of head costs `aquifer_porosity` metres of the inventory -- 0.35 -- and
    /// bringing every soil voxel to field capacity costs another 0.025 to 0.035 on these
    /// rings, so this leaves roughly 0.09 m for the pools: on `default` that is 16 m³ in
    /// the basins against the 142 m³ it would take to fill every one of them to its spill.
    /// Ponds in the low ground, not a flooded ring.
    /// The weather, chosen against package W3's hour-long study on all three presets.
    ///
    /// **When** it rains is the schedule: 300 to 900 s, Wrysk's 5-to-15-minute cadence,
    /// drawn per gap from the world's own seed. The **trigger is no longer a trigger**:
    /// at 0.01 of the world's water it is an availability floor a healthy store clears
    /// easily, so the calendar decides and the store only ever vetoes. It sits well under
    /// `atmosphere_fraction` 0.08, which is what the world starts aloft.
    ///
    /// **Rain rate and shower volume are one choice.** A shower lasts
    /// `shower_volume_m3 / (rain_m_per_s × sky area)`, so 3.5e-5 m/s (126 mm/h) against
    /// 0.4 m³ is a minute of visible rain on `default`'s 192 m². The volume is therefore
    /// **per preset**, scaled to footprint ([`Water::SMALL`], [`Water::WIDE`]), so the
    /// same rate gives every ring the same minute.
    ///
    /// Evaporation is as high as the rule allows — it must stay below the shower rate,
    /// because `evaporate` runs straight after `rain` and would otherwise lift fresh rain
    /// before it infiltrates. It is also the only engine this crate has, and a weak one:
    /// see the study. Transpiration, which only the live world has, is the other.
    pub const DEFAULT: Water = Water {
        inventory_m: 1.5,
        atmosphere_fraction: 0.08,
        aquifer_head_m: 1.0,
        closed_cycle: true,
        rain_m_per_s: 3.5e-5,
        evaporation_m_per_s: 3.0e-5,
        shower_trigger_fraction: 0.01,
        shower_volume_m3: 0.4,
        shower_interval_min_s: 300.0,
        shower_interval_max_s: 900.0,
        // Three voxels at 0.25 m. The lake holds the rows **below** the datum — the outlet
        // sits in the datum row, on a rim column whose ground tops out just under it — so
        // this is a two-voxel, half-metre pond. Two voxels of ask leaves one voxel of
        // water, which reads as nothing: measured over eight seeds of `default`, one
        // voxel is a median of 0.8 m² visible and two is 9.8 m².
        //
        // It is not free. No hollow floor may sit under the waterline, so a deeper lake is
        // a drier cave system: `default` goes from 7 habitable hollows over eight seeds to
        // 6, `wide` from 18 to 15. Three voxels is where the water becomes readable for
        // the least of that.
        lake_depth_m: 0.75,
        min_lake_m2: 6.0,
    };

    /// `small`'s ring is 60 m² of footprint against `default`'s 192, and both the store's
    /// refill and one tick of rain scale with that area. So the **shower scales with it
    /// too**: the same rain rate then puts the same minute of rain on every preset, and a
    /// shower costs the same share of what the interval lifted.
    /// What a **hand-built fixture** is charged with: the cycle and the rates, at the
    /// inventory the presets carried before the lake. A preset's inventory is now sized
    /// to lift a whole ring's water table up to its lake floor, and pouring that into the
    /// authored scene's 24 m² floods it — nine stands instead of sixteen, measured. The
    /// fixture has its own authored pool and no lake datum, so it needs none of it.
    pub const AUTHORED: Water = Water {
        inventory_m: 0.5,
        ..Water::DEFAULT
    };

    pub const SMALL: Water = Water {
        shower_volume_m3: 0.125,
        // Three voxels at 0.125 m: a two-voxel, quarter-metre pond. Deeper reads better —
        // four voxels lifts the median from 5.2 to 7.1 m² — but four is where this ring's
        // grottos start dying: 5 habitable hollows over eight seeds becomes 3, and the
        // seeds with none go from 5 to 6. At three voxels the lake costs **no** hollows at
        // all against the same ring with no lake, so the bar comes down instead of the
        // water going up.
        lake_depth_m: 0.375,
        // 3 m² on a 60 m² ring is a pond twenty voxels across at 4 px each: plainly a
        // water feature on the panel, and six of eight seeds clear it at this depth
        // against three of eight at 6 m². (`small`'s five barren seeds are the preset's
        // own — it has them at every depth, lake or no lake.)
        min_lake_m2: 3.0,
        ..Water::DEFAULT
    };

    /// `wide` is 384 m², twice `default`.
    pub const WIDE: Water = Water {
        shower_volume_m3: 0.8,
        ..Water::DEFAULT
    };

    pub fn validate(&self) -> anyhow::Result<()> {
        for (name, v) in [
            ("water.inventory_m", self.inventory_m),
            ("water.atmosphere_fraction", self.atmosphere_fraction),
            ("water.aquifer_head_m", self.aquifer_head_m),
            ("water.rain_m_per_s", self.rain_m_per_s),
            ("water.evaporation_m_per_s", self.evaporation_m_per_s),
            (
                "water.shower_trigger_fraction",
                self.shower_trigger_fraction,
            ),
            ("water.shower_volume_m3", self.shower_volume_m3),
            ("water.shower_interval_min_s", self.shower_interval_min_s),
            ("water.shower_interval_max_s", self.shower_interval_max_s),
            ("water.lake_depth_m", self.lake_depth_m),
            ("water.min_lake_m2", self.min_lake_m2),
            ("water.lake_depth_m", self.lake_depth_m),
            ("water.min_lake_m2", self.min_lake_m2),
        ] {
            anyhow::ensure!(
                v.is_finite() && v >= 0.0,
                "{name} must be finite and not negative, not {v}"
            );
        }
        anyhow::ensure!(
            self.atmosphere_fraction <= 1.0,
            "water.atmosphere_fraction is a share of the inventory, not {}",
            self.atmosphere_fraction
        );
        anyhow::ensure!(
            self.shower_trigger_fraction <= 1.0,
            "water.shower_trigger_fraction is a share of the world's water, not {}",
            self.shower_trigger_fraction
        );
        Ok(())
    }

    /// Write this recipe's runtime water settings onto the config the world will run on.
    ///
    /// **The recipe decides the cycle** (slice 3's decision, applied here): a landscape
    /// states its own water inventory and the weather that keeps it moving, and the host
    /// TOML no longer writes rain, evaporation or a water table beside it. [`Water::DRY`]
    /// holds [`Config::default`]'s own five values, so a `Ridge` world is untouched.
    pub fn cycle_into(&self, c: &mut Config) {
        c.closed_water_budget = self.closed_cycle;
        c.rain_m_per_s = self.rain_m_per_s;
        c.evaporation_m_per_s = self.evaporation_m_per_s;
        c.shower_trigger_fraction = self.shower_trigger_fraction;
        c.shower_volume_m3 = self.shower_volume_m3;
        c.shower_interval_min_s = self.shower_interval_min_s;
        c.shower_interval_max_s = self.shower_interval_max_s;
    }
}

/// A landscape in metres. [`Recipe::default`] is the `default` preset.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Recipe {
    /// Elevation the front edge is built around, metres above `y = 0`.
    pub base_m: f64,
    /// How much higher the back wall stands than the front. The camera looks slightly
    /// down from the front, so this is what keeps the far terrain readable.
    pub back_rise_m: f64,
    /// Relief below this is compressed rather than cut: a receiving basin with a floor,
    /// not a clipped trough.
    pub basin_floor_m: f64,
    /// Depth anisotropy. The habitat is a few metres deep against tens of metres around,
    /// so features are stretched into `z` by this factor; `1.0` is isotropic.
    pub depth_scale: f64,

    /// Wavelength of the coarsest relief octave, metres. This is the landform size, and
    /// how many fit is `circumference / wavelength`.
    pub relief_wavelength_m: f64,
    /// Amplitude of the whole relief ladder, metres.
    pub relief_m: f64,
    /// Octaves the relief ladder asks for; the resolution limit may drop the finest.
    pub relief_octaves: u32,
    /// Amplitude ratio between successive octaves. Below 0.5 the fine detail is
    /// restrained, which is what a 0.25 m voxel can actually show.
    pub persistence: f64,
    /// Frequency ratio between successive octaves.
    pub lacunarity: f64,

    /// Wavelength of the coarsest ridged octave, metres.
    pub ridge_wavelength_m: f64,
    /// Amplitude of the ridged noise where the rocky mask is full, metres.
    pub ridge_relief_m: f64,
    /// Octaves the ridged ladder asks for.
    pub ridge_octaves: u32,

    /// Wavelength of the field that decides which stretches of ring are rocky, metres.
    pub rocky_wavelength_m: f64,
    /// Roughly what share of the ring the ridged noise reaches, `0..=1`.
    pub rocky_fraction: f64,

    /// Wavelength of the domain warp, metres.
    pub warp_wavelength_m: f64,
    /// How far the domain warp displaces the ridged noise, metres. Modest on purpose:
    /// it is there to bend parallel ridges, not to fold the landscape.
    pub warp_m: f64,

    /// The finest octave the generator will keep, in voxels of wavelength. Two voxels is
    /// the Nyquist limit of the grid the terrain is quantised onto.
    pub min_feature_voxels: f64,

    /// Loose weathered sediment the landscape starts mantled in, metres. Erosion moves
    /// it; it is not added to the relief, the bedrock top sits this far below the
    /// surface. With no erosion at all this is the soil everywhere.
    pub mantle_m: f64,
    /// Deepest sediment a column may carry into the voxels, metres.
    pub soil_max_m: f64,

    /// One soft-to-hard cycle of the strata, metres of height.
    pub strata_m: f64,
    /// How far the strata are warped around the ring, metres.
    pub strata_warp_m: f64,
    /// Hardness of the softest and of the hardest rock, `0..=1`.
    pub hardness_soft: f64,
    pub hardness_hard: f64,
    /// Wavelength of the slow regional variation in hardness, metres: the same band is
    /// not equally hard all the way around.
    pub hardness_region_m: f64,
    /// At or above this hardness the rock voxelises as [`crate::Material::Bedrock`], and
    /// a bank standing over a cut counts as a hard cap.
    pub bedrock_hardness: f64,
    /// Everything below this height is the impermeable core, hardness `1`.
    pub core_m: f64,

    /// Soil pockets inside the rock.
    pub pockets: u32,

    /// The erosion budget.
    pub erosion: Erosion,
    /// Structural benches: the ledges a hard stratum makes where it outcrops.
    pub benches: Benches,
    /// Carved hollows.
    pub hollows: Hollows,
    /// The water the world starts with.
    pub water: Water,

    /// The per-pass seed streams.
    pub streams: Streams,
}

impl Default for Recipe {
    fn default() -> Recipe {
        Recipe::DEFAULT
    }
}

impl Recipe {
    /// The `default` ring: 128 x 48 x 24 at 0.25 m, 32 m around, 12 m of headroom.
    /// Six resolved octaves from 16 m down to 0.5 m; two broad landforms.
    pub const DEFAULT: Recipe = Recipe {
        base_m: 3.0,
        back_rise_m: 3.1,
        basin_floor_m: 1.3,
        depth_scale: 0.5,
        relief_wavelength_m: 16.0,
        relief_m: 2.5,
        relief_octaves: 6,
        persistence: 0.5,
        lacunarity: 2.0,
        ridge_wavelength_m: 8.0,
        ridge_relief_m: 1.4,
        ridge_octaves: 3,
        rocky_wavelength_m: 21.0,
        rocky_fraction: 0.45,
        warp_wavelength_m: 18.0,
        warp_m: 1.6,
        min_feature_voxels: 2.0,
        mantle_m: 0.25,
        soil_max_m: 1.5,
        strata_m: 2.0,
        strata_warp_m: 0.4,
        hardness_soft: 0.25,
        hardness_hard: 0.95,
        hardness_region_m: 18.0,
        bedrock_hardness: 0.7,
        core_m: 2.0,
        pockets: 3,
        erosion: Erosion::DEFAULT,
        benches: Benches::ON,
        hollows: Hollows::GROTTOS,
        water: Water::DEFAULT,
        streams: Streams {
            relief: 0x_5354_4147_5F52_454C,
            ridge: 0x_5354_4147_5F52_4447,
            warp: 0x_5354_4147_5F57_5250,
            rocky: 0x_5354_4147_5F52_4B59,
            hardness: 0x_5354_4147_5F48_5244,
            material: 0x_5354_4147_5F4D_4154,
        },
    };

    /// The `small` ring the Tachyon panel runs: 160 x 48 x 24 at 0.125 m, 20 m around,
    /// 6 m tall and 3 m deep, drawn at 4 px per voxel on a 640 x 360 raster.
    ///
    /// Designed from that picture rather than scaled down from `default`. Two broad
    /// landforms around the ring and three rocky regions across them, so there are
    /// benched, grotto-bearing headlands with soil valleys between. Bands of 1.4 m,
    /// which is what a grotto needs: eleven voxels of face for a two-voxel cap over
    /// seven of soft rock with the shared 0.75 m of clearance under it. The fields
    /// barely vary with depth, so a rocky region is a stripe across the whole strip and
    /// its benches are terraces the camera looks along.
    ///
    /// Measured at seeds 1, 7 and 77: 4 to 11 undercuts, 25 to 26 bench faces, a closed
    /// basin with its spill above its floor on every seed, a fifth to a third of the
    /// columns bare rock, the skyline pass under 4.5 %, and every ring walkable.
    pub const SMALL: Recipe = Recipe {
        // 0.375 m of floor and 5.375 m of ceiling is five metres of room, and the bench
        // pull spends up to one band of it, so the smooth relief lives in the top three
        // and a half and the benches cut down into the rest.
        base_m: 1.6,
        back_rise_m: 1.5,
        basin_floor_m: 0.9,
        // Three metres of habitat against twenty around: the fields barely change with
        // depth, so a rocky region is a stripe across the whole strip and its benches
        // are terraces the camera looks along, not patches that step down behind what
        // is in front of them.
        depth_scale: 0.2,
        relief_wavelength_m: 10.0,
        relief_m: 1.7,
        relief_octaves: 5,
        ridge_wavelength_m: 5.0,
        ridge_relief_m: 0.55,
        // Three rocky regions around twenty metres, with soft ground between them.
        rocky_wavelength_m: 7.0,
        rocky_fraction: 0.4,
        warp_wavelength_m: 11.0,
        warp_m: 0.9,
        mantle_m: 0.18,
        soil_max_m: 0.9,
        // A grotto needs a cap of 2 voxels over 7 of soft rock and a neighbour cut below
        // its floor: nine voxels of face, 1.125 m. A 1.4 m band gives eleven.
        strata_m: 1.4,
        strata_warp_m: 0.25,
        // Four cells of regional variation around twenty metres, not two: on a ring
        // this short a slower field is one value per seed, and a seed whose value sat
        // low had no hard bands anywhere and so no ledges at all.
        hardness_region_m: 5.0,
        // A thin cap over a thick soft band. At 0.125 m a grotto wants two voxels of
        // bedrock over seven of rock, and the default's threshold makes the hard part of
        // a 1.4 m band four voxels thick -- a roof so deep the notch never starts.
        bedrock_hardness: 0.78,
        core_m: 0.8,
        erosion: Erosion::SMALL,
        benches: Benches::SMALL,
        hollows: Hollows::SMALL,
        water: Water::SMALL,
        ..Recipe::DEFAULT
    };

    /// The `wide` ring: 256 x 48 x 24 at 0.25 m, 64 m around. The same landform sizes as
    /// `default` in a ring twice as long, so it holds twice the geography rather than
    /// the same hill stretched.
    pub const WIDE: Recipe = Recipe {
        relief_m: 2.7,
        rocky_fraction: 0.4,
        water: Water::WIDE,
        ..Recipe::DEFAULT
    };

    /// Refuse a recipe no landscape can be built from. [`Config::validate`] calls this
    /// for a staged config; nothing past it divides by a wavelength or a spacing again.
    pub fn validate(&self) -> anyhow::Result<()> {
        for (name, v) in [
            ("relief_wavelength_m", self.relief_wavelength_m),
            ("ridge_wavelength_m", self.ridge_wavelength_m),
            ("rocky_wavelength_m", self.rocky_wavelength_m),
            ("warp_wavelength_m", self.warp_wavelength_m),
            ("strata_m", self.strata_m),
            ("mantle_m", self.mantle_m),
            ("hardness_region_m", self.hardness_region_m),
            ("depth_scale", self.depth_scale),
            ("min_feature_voxels", self.min_feature_voxels),
            ("relief_m", self.relief_m),
            ("persistence", self.persistence),
        ] {
            anyhow::ensure!(
                v.is_finite() && v > 0.0,
                "{name} must be positive and finite, not {v}"
            );
        }
        for (name, v) in [
            ("base_m", self.base_m),
            ("back_rise_m", self.back_rise_m),
            ("basin_floor_m", self.basin_floor_m),
            ("ridge_relief_m", self.ridge_relief_m),
            ("rocky_fraction", self.rocky_fraction),
            ("warp_m", self.warp_m),
            ("soil_max_m", self.soil_max_m),
            ("strata_warp_m", self.strata_warp_m),
            ("core_m", self.core_m),
            ("hardness_soft", self.hardness_soft),
            ("hardness_hard", self.hardness_hard),
            ("bedrock_hardness", self.bedrock_hardness),
            ("erosion.rain", self.erosion.rain),
            ("erosion.capacity", self.erosion.capacity),
            ("erosion.erode", self.erosion.erode),
            ("erosion.deposit", self.erosion.deposit),
            (
                "erosion.bedrock_resistance",
                self.erosion.bedrock_resistance,
            ),
            ("erosion.max_cut_soft_m", self.erosion.max_cut_soft_m),
            ("erosion.max_cut_hard_m", self.erosion.max_cut_hard_m),
            ("erosion.repose", self.erosion.repose),
        ] {
            anyhow::ensure!(
                v.is_finite() && v >= 0.0,
                "{name} must be finite and not negative, not {v}"
            );
        }
        self.benches.validate()?;
        self.hollows.validate()?;
        self.water.validate()?;
        anyhow::ensure!(
            self.lacunarity.is_finite() && self.lacunarity > 1.0,
            "lacunarity steps the frequency up, so it must exceed 1, not {}",
            self.lacunarity
        );
        Ok(())
    }

    /// The relief ladder.
    pub fn relief_ladder(&self) -> Ladder {
        Ladder {
            wavelength_m: self.relief_wavelength_m,
            octaves: self.relief_octaves,
            persistence: self.persistence,
            lacunarity: self.lacunarity,
        }
    }

    /// The ridged ladder.
    pub fn ridge_ladder(&self) -> Ladder {
        Ladder {
            wavelength_m: self.ridge_wavelength_m,
            octaves: self.ridge_octaves,
            persistence: self.persistence,
            lacunarity: self.lacunarity,
        }
    }

    /// The finest wavelength this voxel size can carry, metres.
    pub fn min_wavelength_m(&self, voxel_m: f64) -> f64 {
        self.min_feature_voxels * voxel_m
    }

    /// Relief octaves that survive the resolution limit at this voxel size. The number
    /// the recipe *asks* for is [`Recipe::relief_octaves`]; this is what it gets.
    pub fn resolved_octaves(&self, voxel_m: f64) -> u32 {
        resolved_octaves(&self.relief_ladder(), self.min_wavelength_m(voxel_m))
    }

    /// Relief above [`Recipe::base_m`] at one point of the ring, metres. Exactly periodic
    /// in `x_m` at `circumference_m`, including the domain warp: two coordinates one lap
    /// apart return the same float, not two nearly-equal ones. Knows nothing about the
    /// voxel grid except the resolution limit.
    pub fn relief_m_at(
        &self,
        x_m: f64,
        z_m: f64,
        circumference_m: f64,
        voxel_m: f64,
        seed: u64,
    ) -> f64 {
        let min_wl = self.min_wavelength_m(voxel_m);
        // Reduce to the ring's canonical representative *before* the warp: a warp offset
        // added to `x` and to `x + circumference` rounds differently at the two
        // magnitudes, and one ulp of difference is a seam. Reduced first, two coordinates
        // one lap apart are the same float and everything after them is identical.
        let x_m = if circumference_m > 0.0 {
            x_m.rem_euclid(circumference_m)
        } else {
            x_m
        };
        let zs = z_m * self.depth_scale;
        let s = self.streams;

        let broad = self.relief_m
            * fbm(
                x_m,
                zs,
                circumference_m,
                &self.relief_ladder(),
                min_wl,
                seed ^ s.relief,
            );

        // The rocky regions: a slow field, thresholded softly so ridges fade in.
        let mask = self.rocky_mask_at(x_m, z_m, circumference_m, seed);
        if mask <= 0.0 {
            return broad;
        }

        // A modest periodic domain warp, so the ridges are not parallel and obviously
        // sinusoidal. Both offsets are periodic fields, so the warped field still is.
        let warp_cells = ring_cells(circumference_m, self.warp_wavelength_m);
        let wx = self.warp_m * ring_noise(x_m, zs, circumference_m, warp_cells, seed ^ s.warp);
        let wz = self.warp_m
            * ring_noise(
                x_m + 137.0,
                zs + 61.0,
                circumference_m,
                warp_cells,
                seed ^ s.warp.rotate_left(17),
            );
        let crest = ridged(
            x_m + wx,
            zs + wz,
            circumference_m,
            &self.ridge_ladder(),
            min_wl,
            seed ^ s.ridge,
        );
        broad + mask * self.ridge_relief_m * crest
    }
}

impl Recipe {
    /// How hard the rock is at one point of the world, `0..=1`.
    ///
    /// A field of `(x, y, z)` in metres, not a per-column formula: the erosion solver
    /// reads it at the bedrock surface, the voxeliser turns it into
    /// [`crate::Material::Bedrock`] or [`crate::Material::Rock`], and slice 2b's
    /// galleries will follow the same layers the picture shows. Periodic in `x`.
    ///
    /// Strata are one soft-to-hard cycle every [`Recipe::strata_m`] of height, warped
    /// around the ring so they are not flat, modulated slowly so the same band is not
    /// equally hard everywhere, and overridden by the impermeable core below
    /// [`Recipe::core_m`].
    pub fn hardness_at(
        &self,
        x_m: f64,
        y_m: f64,
        z_m: f64,
        circumference_m: f64,
        seed: u64,
    ) -> f64 {
        self.strata_at(x_m, z_m, circumference_m, seed)
            .hardness(y_m)
    }

    /// The rock banding of one column, evaluated once for a whole scan up or down it.
    pub fn strata_at(&self, x_m: f64, z_m: f64, circumference_m: f64, seed: u64) -> Strata {
        let x_m = self.on_the_ring(x_m, circumference_m);
        let zs = z_m * self.depth_scale;
        let s = self.streams;
        let warp_m = self.strata_warp_m
            * ring_noise(
                x_m,
                zs,
                circumference_m,
                ring_cells(circumference_m, self.warp_wavelength_m),
                seed ^ s.hardness,
            );
        let region = (0.5
            + 0.5
                * ring_noise(
                    x_m,
                    zs,
                    circumference_m,
                    ring_cells(circumference_m, self.hardness_region_m),
                    seed ^ s.hardness.rotate_left(13),
                ))
        .clamp(0.0, 1.0);
        Strata {
            warp_m,
            region,
            soft: self.hardness_soft,
            span: self.hardness_hard - self.hardness_soft,
            strata_m: self.strata_m.max(1e-9),
            core_m: self.core_m,
        }
    }

    /// How rocky this point of the ring is, `0..=1`: the field the ridged noise and the
    /// benches are both weighted by.
    pub fn rocky_mask_at(&self, x_m: f64, z_m: f64, circumference_m: f64, seed: u64) -> f64 {
        let x_m = self.on_the_ring(x_m, circumference_m);
        let zs = z_m * self.depth_scale;
        let at = ring_noise(
            x_m,
            zs,
            circumference_m,
            ring_cells(circumference_m, self.rocky_wavelength_m),
            seed ^ self.streams.rocky,
        );
        let lo = 1.0 - 2.0 * self.rocky_fraction.clamp(0.0, 1.0);
        smoothstep(lo, lo + 0.5, at)
    }

    /// Where the bench pull puts the bedrock surface: on the top of the hard band under
    /// it, as far as [`Benches::strength`] and the rocky mask ask for.
    ///
    /// The pull is downward, onto the band top **at or below** the smooth surface, which
    /// is what makes the face point at the camera: the ground falls toward the front, so
    /// the step down onto the next band top falls that way too, and the flat of a bench
    /// is its band's own top. Soft ground, and the mask's own edge, are left alone, so a
    /// bench ramps out instead of ending in a wall.
    pub fn benched_m(
        &self,
        x_m: f64,
        z_m: f64,
        raw_m: f64,
        circumference_m: f64,
        seed: u64,
    ) -> f64 {
        let b = self.benches;
        if b.strength <= 0.0 {
            return raw_m;
        }
        let mask = self.rocky_mask_at(x_m, z_m, circumference_m, seed);
        // `ramp_m` is a length; the mask is a field with a wavelength. A quarter of that
        // wavelength is about the distance its smoothstep takes to rise, so this is the
        // share of the rise given over to the ramp.
        let edge = (4.0 * b.ramp_m / self.rocky_wavelength_m.max(1e-9)).clamp(0.05, 1.0);
        let pull = b.strength.clamp(0.0, 1.0) * smoothstep(0.0, edge, mask);
        if pull <= 0.0 {
            return raw_m;
        }
        match self
            .strata_at(x_m, z_m, circumference_m, seed)
            .hard_band_top_m(raw_m, self.bedrock_hardness)
        {
            Some(top) => raw_m + (top - raw_m) * pull,
            None => raw_m,
        }
    }

    /// The ring's canonical representative of a metre coordinate.
    fn on_the_ring(&self, x_m: f64, circumference_m: f64) -> f64 {
        if circumference_m > 0.0 {
            x_m.rem_euclid(circumference_m)
        } else {
            x_m
        }
    }
}

/// A shipped landscape: the recipe and the ring it was written for.
///
/// The extents belong to [`Config`], not to the recipe — the same recipe on a wider ring
/// is more geography, which is the point — but a preset that names no ring is not a
/// landscape anyone can look at, so each one carries the one it was tuned on.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    /// `small`, `default` or `wide`.
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub voxel_m: f64,
    pub recipe: Recipe,
}

/// Every shipped preset, in size order.
pub const PRESETS: [Preset; 3] = [
    Preset {
        name: "small",
        width: 160,
        height: 48,
        depth: 24,
        voxel_m: 0.125,
        recipe: Recipe::SMALL,
    },
    Preset {
        name: "default",
        width: 128,
        height: 48,
        depth: 24,
        voxel_m: 0.25,
        recipe: Recipe::DEFAULT,
    },
    Preset {
        name: "wide",
        width: 256,
        height: 48,
        depth: 24,
        voxel_m: 0.25,
        recipe: Recipe::WIDE,
    },
];

impl Preset {
    /// A shipped preset by name.
    pub fn find(name: &str) -> Option<&'static Preset> {
        PRESETS.iter().find(|p| p.name == name)
    }

    /// The ring this preset was written for, on its own recipe.
    pub fn config(&self) -> Config {
        Config {
            width: self.width,
            height: self.height,
            depth: self.depth,
            voxel_m: self.voxel_m,
            landform: Landform::Staged(self.recipe),
            ..Config::default()
        }
    }

    /// Circumference in metres.
    pub fn circumference_m(&self) -> f64 {
        self.width as f64 * self.voxel_m
    }
}

#[cfg(test)]
mod tests {

    /// A lake cannot be a negative depth, and a world cannot be asked for a negative
    /// area of visible water.
    #[test]
    fn the_lake_fields_must_be_real_lengths() {
        let with = |depth, area| Water {
            lake_depth_m: depth,
            min_lake_m2: area,
            ..Water::DEFAULT
        };
        with(0.5, 6.0).validate().expect("the shipped numbers");
        with(0.0, 0.0)
            .validate()
            .expect("no lake asked for is valid");
        assert!(with(-0.5, 6.0).validate().is_err(), "a negative lake depth");
        assert!(with(0.5, -1.0).validate().is_err(), "a negative lake area");
        assert!(
            with(f64::NAN, 6.0).validate().is_err(),
            "a depth that is not a number"
        );
    }

    /// The recipe decides the weather, so every number it holds has to arrive on the
    /// config the world runs on — the schedule included.
    #[test]
    fn a_recipes_water_writes_the_whole_cycle_onto_the_config() {
        let w = Water {
            closed_cycle: true,
            rain_m_per_s: 3e-5,
            evaporation_m_per_s: 1e-5,
            shower_trigger_fraction: 0.07,
            shower_volume_m3: 0.4,
            shower_interval_min_s: 300.0,
            shower_interval_max_s: 900.0,
            ..Water::DRY
        };
        let mut c = Config::default();
        w.cycle_into(&mut c);
        assert!(c.closed_water_budget);
        assert_eq!(c.rain_m_per_s, 3e-5);
        assert_eq!(c.evaporation_m_per_s, 1e-5);
        assert_eq!(c.shower_trigger_fraction, 0.07);
        assert_eq!(c.shower_volume_m3, 0.4);
        assert_eq!(c.shower_interval_min_s, 300.0);
        assert_eq!(c.shower_interval_max_s, 900.0);

        // And the dry recipe leaves a config exactly as it found it.
        let mut untouched = Config::default();
        Water::DRY.cycle_into(&mut untouched);
        assert_eq!(untouched, Config::default());
    }
    use super::*;

    /// The presets differ in resolved octaves and in how many landforms fit, never by
    /// squeezing one landscape into another ring.
    #[test]
    fn the_presets_resolve_the_octaves_their_rings_can_carry() {
        let small = Preset::find("small").unwrap();
        let default = Preset::find("default").unwrap();
        let wide = Preset::find("wide").unwrap();

        assert_eq!(default.recipe.resolved_octaves(default.voxel_m), 6);
        assert_eq!(small.recipe.resolved_octaves(small.voxel_m), 5);
        assert_eq!(wide.recipe.resolved_octaves(wide.voxel_m), 6);

        // Landforms per ring: the coarse octave's lattice.
        let count = |p: &Preset| ring_cells(p.circumference_m(), p.recipe.relief_wavelength_m);
        assert_eq!(count(small), 2);
        assert_eq!(count(default), 2);
        assert_eq!(count(wide), 4, "the wide ring holds twice the geography");
    }

    /// The vertical budget: nothing a preset can build reaches the clamps, or the
    /// landscape would be a plateau and the metre-for-metre invariance would be a lie.
    #[test]
    fn every_preset_fits_between_the_floor_and_the_sky() {
        for p in PRESETS {
            let r = p.recipe;
            let head_m = (p.height as f64 - 5.0) * p.voxel_m;
            let floor_m = 3.0 * p.voxel_m;
            let high = r.base_m + r.relief_m + r.ridge_relief_m + r.back_rise_m;
            let deep = r.base_m - r.relief_m - r.ridge_relief_m;
            let low = r.basin_floor_m - (r.basin_floor_m - deep).max(0.0) * 0.12;
            assert!(
                high < head_m,
                "{}: {high:.2} m reaches the {head_m:.2} m ceiling",
                p.name
            );
            assert!(
                low > floor_m,
                "{}: {low:.2} m reaches the {floor_m:.2} m floor",
                p.name
            );
        }
    }

    /// A recipe whose rocky mask is everywhere or nowhere, so a bench test can choose
    /// which side of it to stand on.
    fn benched_recipe(rocky: bool, strength: f64) -> Recipe {
        Recipe {
            rocky_fraction: if rocky { 1.0 } else { 0.0 },
            benches: Benches {
                strength,
                ramp_m: 0.01,
            },
            ..Recipe::DEFAULT
        }
    }

    /// A ramp of relief across one band thickness comes out of the bench pull as two
    /// flats on two band tops with one face between them; the same ramp in soft ground
    /// comes out as it went in.
    #[test]
    fn a_ramp_across_a_band_becomes_two_benches_and_a_face() {
        let (circ, seed) = (32.0, 5u64);
        let rocky = benched_recipe(true, 1.0);
        let strata = rocky.strata_at(4.0, 1.5, circ, seed);
        // Stand on a band top and walk down one whole band thickness.
        let top = strata
            .hard_band_top_m(6.0, rocky.bedrock_hardness)
            .expect("this column has hard bands");
        // Half open at the bottom: the sample exactly one band down stands on the next
        // band top, and that is a third flat, not a second face.
        let samples: Vec<f64> = (0..40)
            .map(|i| top - rocky.strata_m * i as f64 / 40.0)
            .collect();

        let benched: Vec<f64> = samples
            .iter()
            .map(|&raw| rocky.benched_m(4.0, 1.5, raw, circ, seed))
            .collect();
        let mut flats: Vec<f64> = benched.clone();
        flats.sort_by(f64::total_cmp);
        flats.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        assert_eq!(flats.len(), 2, "two band tops, not {flats:?}");
        let face = flats[1] - flats[0];
        assert!(
            face >= rocky.strata_m * 0.98,
            "the face is {face:.3} m, less than the {:.3} m band it should be",
            rocky.strata_m
        );
        assert!(
            benched
                .iter()
                .all(|b| flats.iter().any(|f| (f - b).abs() < 1e-9)),
            "the bench is flat between its faces"
        );

        let soft = benched_recipe(false, 1.0);
        for &raw in &samples {
            let out = soft.benched_m(4.0, 1.5, raw, circ, seed);
            assert!(
                (out - raw).abs() < 1e-12,
                "soft ground was benched: {raw:.3} -> {out:.3}"
            );
        }
    }

    /// A bench ends in a ramp, not in a wall.
    ///
    /// Out at the edge of a rocky region the pull has already faded to less than a
    /// voxel, so the benched ground meets the smooth ground there without a step — while
    /// inside the region the same recipe is stepping by whole bands. A hard switch at
    /// the mask's threshold would pass the second of those and fail the first.
    #[test]
    fn a_bench_ramps_out_where_the_rocky_mask_does() {
        let p = Preset::find("default").unwrap();
        let (r, circ, vm) = (p.recipe, p.circumference_m(), p.voxel_m);
        let edge = (4.0 * r.benches.ramp_m / r.rocky_wavelength_m).clamp(0.05, 1.0);
        let mut biggest_inside = 0.0f64;
        for seed in [1u64, 7, 77] {
            for z in [0usize, 11, 23] {
                let z_m = (z as f64 + 0.5) * vm;
                for x in 0..p.width as usize {
                    let x_m = (x as f64 + 0.5) * vm;
                    // A raw surface that does not itself step, so what is measured is
                    // the pull and nothing else.
                    let raw = 5.0 + 0.4 * (x as f64 * 0.11).sin();
                    let pull = (r.benched_m(x_m, z_m, raw, circ, seed) - raw).abs();
                    let mask = r.rocky_mask_at(x_m, z_m, circ, seed);
                    if mask <= edge * 0.2 {
                        assert!(
                            pull <= vm,
                            "seed {seed} z {z} x {x}: {pull:.3} m of bench at the mask's \
                             edge, more than one {vm:.3} m voxel"
                        );
                    }
                    if mask >= 0.9 {
                        biggest_inside = biggest_inside.max(pull);
                    }
                }
            }
        }
        assert!(
            biggest_inside >= r.strata_m * 0.5,
            "nothing is being benched anywhere: the biggest pull inside a rocky region \
             is {biggest_inside:.3} m"
        );
    }

    /// A staged config survives a snapshot round trip: postcard is not self-describing,
    /// so the enum has to be externally tagged, not internally tagged.
    #[test]
    fn a_staged_config_round_trips_through_postcard() {
        for p in PRESETS {
            let c = p.config();
            let bytes = postcard::to_stdvec(&c).unwrap();
            let back: Config = postcard::from_bytes(&bytes).unwrap();
            assert_eq!(c, back, "{}", p.name);
        }
    }
}
