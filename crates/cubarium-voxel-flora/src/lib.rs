//! Producers on the voxel strip. Two species — bloomcrown, a sun producer of ridges
//! and terraces, and umbrellafrond, a shade-and-wet producer of hollows — live as
//! **stands** on support faces of a [`cubarium_voxel::World`], compete for light
//! through their own canopies and for pore water through the core's bounded
//! withdrawal, and spread by paid propagules.
//!
//! This crate never draws and never reads the clock. It reads the world through
//! [`cubarium_voxel::VoxelView`] and changes it only through
//! [`cubarium_voxel::Command`]s; the two ledgers stay separate (water in the core,
//! material and energy here). A stand's location is a **support voxel**: a solid whose
//! top face is exposed. Never a column's skyline.
//!
//! Ecology rules carry over from `design/ecology-v1-contract.md` §4 (stands `P`/`W`/`Q`,
//! paid maintenance and growth, dieback, death, paid propagules) with light from
//! geometry and water from the simulation instead of noise fields. See
//! `design/voxel-ecology-sketch-2026-09-16.md` §2 and §4.

#![forbid(unsafe_code)]

mod step;

use cubarium_voxel::{VoxelView, World};
use serde::{Deserialize, Serialize};

pub use cubarium_voxel::{DT, TICK_HZ};

/// The two producers of the first coupled experiment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Species {
    /// Sun producer: high light, shallow roots, tolerates dry, dies in standing water.
    Bloomcrown,
    /// Shade/wet producer: tolerates low light, deep roots, wants wet soil, taller crown.
    Umbrellafrond,
}

impl Species {
    pub const ALL: [Species; 2] = [Species::Bloomcrown, Species::Umbrellafrond];

    pub fn name(self) -> &'static str {
        match self {
            Species::Bloomcrown => "bloomcrown",
            Species::Umbrellafrond => "umbrellafrond",
        }
    }

    pub fn parse(s: &str) -> Option<Species> {
        Species::ALL.into_iter().find(|sp| sp.name() == s)
    }
}

/// Life stage of a stand. A site with no stand is **bare**.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    /// `0 < W < W_min`: propagule material, frozen. No income, no maintenance, no growth.
    Establishing,
    /// `W >= W_min`: runs the full plant model every tick.
    Alive,
}

/// A support voxel: solid, with its top face exposed to void. `x` is stored wrapped
/// into `0..width`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Site {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

/// One stand: a plant of one species rooted on one support face.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stand {
    pub site: Site,
    pub species: Species,
    pub stage: Stage,
    /// `W`: living wood, material units.
    pub wood: f64,
    /// `P`: foliage, material units.
    pub foliage: f64,
    /// `Q`: reserve, material units.
    pub reserve: f64,
    /// Light this stand received last tick as a fraction of open sky, `0..=1`, after
    /// terrain sky visibility and canopy attenuation. For inspection and drawing.
    pub light: f64,
    /// Moisture factor `μ` this stand saw last tick, `0..=1`. Zero means wilting.
    pub moisture: f64,
    /// Pore water this stand actually got last tick, cubic metres: its share of what the
    /// core accepted, which is less than it asked for when the soil ran out or when
    /// another stand's roots reached the same voxels. Income reads `moisture`, not this —
    /// the one water read happens before any withdrawal — so this is for inspection,
    /// drawing and tests.
    pub water_m3: f64,
}

/// Non-living stocks that sit on a site: mineral nutrient and the stand's own detritus.
/// One entry per site that has ever held a stand or received litter.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ground {
    pub site: Site,
    /// `N`: mineral nutrient available to a stand on this site.
    pub nutrient: f64,
    /// `D`: leaf litter, material.
    pub litter: f64,
    /// `De`: energy retained in the litter.
    pub litter_energy: f64,
    /// `Wd`: dead wood, material. Keeps its identity; nothing eats it this round.
    pub dead_wood: f64,
}

/// The plant model of one species: `design/ecology-v1-contract.md` §4 parameters, plus
/// the terrain couplings that replace the old noise fields.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SpeciesConfig {
    /// `α`: foliage the structure can carry, `P_cap = α · W`.
    pub alpha: f64,
    /// `W_max`: the most living wood one stand can hold.
    pub wood_max: f64,
    /// `q_cap`: reserve per unit of wood, `Q_max = q_cap · W`.
    pub reserve_cap: f64,
    /// `m_w`: wood maintenance per second.
    pub maintenance: f64,
    /// `m_p`: foliage senescence per second. Shed foliage becomes litter.
    pub senescence: f64,
    /// `r_p`: foliage regrowth per second per unit of `W`.
    pub foliage_rate: f64,
    /// `r_w`: wood growth per second per unit of `W`.
    pub wood_rate: f64,
    /// `c_g`: construction respiration; growing one unit costs `1 + c_g`.
    pub build: f64,
    /// `κ`: wood lost per unit of unpaid maintenance.
    pub dieback: f64,
    /// `W_min`: alive at or above this much wood; establishing below it.
    pub alive_min: f64,
    /// `W_est`: wood a stand needs before it can send propagules.
    pub donor_min: f64,
    /// `q_prop`: the fraction of `Q_max` a donor keeps for itself.
    pub donor_reserve_floor: f64,
    /// `k_est`: propagule material per second per establishing-or-bare neighbour site.
    pub propagule_rate: f64,
    /// `w_frac, p_frac, q_frac`: how a landed propagule splits into wood, starter foliage
    /// and starter reserve. Sums to one.
    pub propagule_split: [f64; 3],
    /// `e_v`: energy per material unit of every plant tissue. Light is the source.
    pub energy_density: f64,
    /// `q_share`: share of every tick's surplus that goes to the reserve first.
    pub reserve_share: f64,
    /// `p_reflush`: reserve is spent on foliage only while `P < p_reflush · P_cap`.
    pub reflush_below: f64,
    /// `g`: gross assimilation per second per unit foliage at full light and moisture.
    pub assimilation: f64,
    /// `K_N`: nutrient half-saturation of assimilation.
    pub nutrient_half: f64,
    /// `f_max`: the fastest a stand can draw its site's nutrient, per second.
    pub nutrient_draw_max: f64,
    /// Light response: `L_eff = L / (L + light_half) · (1 + light_half)`, so a species with
    /// a small `light_half` earns nearly full income in shade and one with a large value
    /// needs open sky.
    pub light_half: f64,
    /// Roots reach `rooting_depth` voxels down from the support (the support voxel
    /// counts as the first) and `rooting_radius` voxels sideways in `x` and `z`; only
    /// soil voxels in that box hold water for the stand.
    pub rooting_depth: u32,
    pub rooting_radius: u32,
    /// Pore fraction (0..1 of capacity) at and below which `μ = 0` (wilting), and at and
    /// above which `μ = 1`; linear between. Read as the water-volume-weighted mean over
    /// the root box.
    pub wilt_pore: f64,
    pub sat_pore: f64,
    /// Water transpired per second per unit foliage at `μ = 1`, cubic metres. Withdrawn
    /// from the root box through `Command::WithdrawPore`, never by reading `pore` twice.
    pub transpiration_m3_per_s: f64,
    /// A propagule may land only on a site whose root-box mean pore fraction is at least
    /// this and whose sky visibility is at least `establish_light_min`.
    pub establish_pore_min: f64,
    pub establish_light_min: f64,
    /// Standing water over the support face deeper than this (metres) kills the stand.
    /// Zero for a species that cannot stand in water at all.
    pub drown_depth_m: f64,
    /// How many support sites away, in `x` and `z`, a propagule may land.
    pub hop: u32,
    /// Crown geometry from wood, shared by the shade model and the presenter so what
    /// shades is exactly what is drawn: crown top above the support face, in voxels,
    /// and crown half-width, in voxels, both linear in `W / W_max` between the two ends.
    pub crown_height_voxels: [f64; 2],
    pub crown_radius_voxels: [f64; 2],
}

impl SpeciesConfig {
    /// Height of the crown top above the support face, in voxels.
    pub fn crown_height(&self, wood: f64) -> f64 {
        let t = (wood / self.wood_max).clamp(0.0, 1.0);
        self.crown_height_voxels[0] + t * (self.crown_height_voxels[1] - self.crown_height_voxels[0])
    }

    /// Half-width of the crown, in voxels.
    pub fn crown_radius(&self, wood: f64) -> f64 {
        let t = (wood / self.wood_max).clamp(0.0, 1.0);
        self.crown_radius_voxels[0] + t * (self.crown_radius_voxels[1] - self.crown_radius_voxels[0])
    }
}

impl SpeciesConfig {
    fn v1_base() -> SpeciesConfig {
        SpeciesConfig {
            alpha: 2.0,
            wood_max: 0.6,
            reserve_cap: 0.5,
            maintenance: 0.0002,
            senescence: 0.001,
            foliage_rate: 0.002,
            wood_rate: 0.001,
            build: 0.2,
            dieback: 1.0,
            alive_min: 0.02,
            donor_min: 0.3,
            donor_reserve_floor: 0.5,
            propagule_rate: 0.0002,
            propagule_split: [0.4, 0.4, 0.2],
            energy_density: 2.0,
            reserve_share: 0.2,
            reflush_below: 0.25,
            assimilation: 0.004,
            nutrient_half: 0.5,
            nutrient_draw_max: 0.01,
            light_half: 0.5,
            rooting_depth: 2,
            rooting_radius: 1,
            wilt_pore: 0.15,
            sat_pore: 0.6,
            transpiration_m3_per_s: 2e-5,
            establish_pore_min: 0.2,
            establish_light_min: 0.3,
            drown_depth_m: 0.0,
            hop: 1,
            crown_height_voxels: [1.0, 3.0],
            crown_radius_voxels: [0.5, 1.5],
        }
    }

    /// Placeholder values; nothing here is tuned. The two species differ only where the
    /// sketch says they must: light need, rooting, water tolerance, crown and hop.
    pub fn bloomcrown() -> SpeciesConfig {
        SpeciesConfig {
            light_half: 0.8,
            rooting_depth: 2,
            rooting_radius: 1,
            wilt_pore: 0.08,
            sat_pore: 0.5,
            establish_pore_min: 0.1,
            establish_light_min: 0.6,
            drown_depth_m: 0.0,
            hop: 2,
            crown_height_voxels: [1.0, 3.0],
            crown_radius_voxels: [0.5, 1.5],
            ..SpeciesConfig::v1_base()
        }
    }

    pub fn umbrellafrond() -> SpeciesConfig {
        SpeciesConfig {
            light_half: 0.15,
            rooting_depth: 4,
            rooting_radius: 1,
            wilt_pore: 0.3,
            sat_pore: 0.8,
            establish_pore_min: 0.45,
            establish_light_min: 0.1,
            drown_depth_m: 0.5,
            hop: 1,
            crown_height_voxels: [2.0, 5.0],
            crown_radius_voxels: [1.0, 2.5],
            ..SpeciesConfig::v1_base()
        }
    }
}

impl Default for SpeciesConfig {
    fn default() -> SpeciesConfig {
        SpeciesConfig::v1_base()
    }
}

/// Everything the plant layer runs from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FloraConfig {
    pub bloomcrown: SpeciesConfig,
    pub umbrellafrond: SpeciesConfig,
    /// Canopy attenuation: a taller stand whose crown covers a site multiplies the light
    /// reaching that site by `exp(-shade_k · P / crown_area)`.
    pub shade_k: f64,
    /// `k_d`: litter decomposition per second (`D → N`, energy to heat).
    pub decomposition: f64,
    /// `k_w`: dead-wood decomposition per second (`Wd → N`), slow.
    pub wood_decomposition: f64,
    /// `e_d_max`: retained energy cap per unit of litter.
    pub litter_energy_cap: f64,
    /// Nutrient a site starts with the first time anything lands on it.
    pub initial_nutrient: f64,
}

impl Default for FloraConfig {
    fn default() -> FloraConfig {
        FloraConfig {
            bloomcrown: SpeciesConfig::bloomcrown(),
            umbrellafrond: SpeciesConfig::umbrellafrond(),
            shade_k: 1.5,
            decomposition: 0.001,
            wood_decomposition: 0.0001,
            litter_energy_cap: 2.0,
            initial_nutrient: 1.0,
        }
    }
}

impl FloraConfig {
    pub fn species(&self, s: Species) -> &SpeciesConfig {
        match s {
            Species::Bloomcrown => &self.bloomcrown,
            Species::Umbrellafrond => &self.umbrellafrond,
        }
    }
}

/// Cumulative external material and energy fluxes of the plant layer. Internal
/// transfers (growth, litterfall, propagules, decomposition to nutrient) never appear
/// here. Water is not material: transpiration lives in the core's water ledger.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FloraLedger {
    /// Energy fixed from light.
    pub light_in: f64,
    /// Energy respired.
    pub heat_out: f64,
    /// Material and energy a `Seed` command created.
    pub seeded_material_in: f64,
    pub seeded_energy_in: f64,
    /// Material and energy removed because a terrain edit buried or removed the support
    /// of a site, or a `Clear` command removed a stand. Reported, never hidden.
    pub removed_material_out: f64,
    pub removed_energy_out: f64,
    /// Water withdrawn through `Command::WithdrawPore`, cubic metres, for cross-checking
    /// against the core ledger's `transpiration_out`.
    pub transpired_m3: f64,
    pub establishments: u64,
    pub deaths: u64,
}

impl FloraLedger {
    pub fn expected_material(&self) -> f64 {
        self.seeded_material_in - self.removed_material_out
    }

    pub fn expected_energy(&self) -> f64 {
        self.seeded_energy_in + self.light_in - self.heat_out - self.removed_energy_out
    }
}

/// Read-only access for drawing and inspection.
#[derive(Clone, Copy, Debug)]
pub struct FloraView<'a> {
    pub config: &'a FloraConfig,
    pub tick: u64,
    /// Sorted by site. At most one stand per site.
    pub stands: &'a [Stand],
    /// Sorted by site. At most one entry per site.
    pub ground: &'a [Ground],
    pub ledger: &'a FloraLedger,
}

impl<'a> FloraView<'a> {
    pub fn stand_at(&self, site: Site) -> Option<&'a Stand> {
        self.stands.binary_search_by_key(&site, |s| s.site).ok().map(|i| &self.stands[i])
    }

    pub fn ground_at(&self, site: Site) -> Option<&'a Ground> {
        self.ground.binary_search_by_key(&site, |g| g.site).ok().map(|i| &self.ground[i])
    }

    /// Material in every living and dead stock.
    pub fn material(&self) -> f64 {
        self.stands.iter().map(|s| s.wood + s.foliage + s.reserve).sum::<f64>()
            + self.ground.iter().map(|g| g.nutrient + g.litter + g.dead_wood).sum::<f64>()
    }

    /// Energy in every living and dead stock.
    pub fn energy(&self) -> f64 {
        self.stands
            .iter()
            .map(|s| self.config.species(s.species).energy_density * (s.wood + s.foliage + s.reserve))
            .sum::<f64>()
            + self.ground.iter().map(|g| g.litter_energy).sum::<f64>()
    }
}

/// A frontend or a test changes the plant layer only through these.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Plant a founder stand on the highest support face of column `(x, z)`: alive, with
    /// `wood`, full foliage `α·W` and full reserve. Refused if the column has no
    /// support, the site already holds a stand, or `wood` is below the species'
    /// `alive_min`.
    Seed { x: i64, z: u32, species: Species, wood: f64 },
    /// Remove the stand on the highest support face of column `(x, z)`, booking its
    /// material and energy as removed. Refused if there is none.
    Clear { x: i64, z: u32 },
}

/// The plant layer. Owns its stands and ground stocks; borrows the world per call.
#[derive(Clone, Debug)]
pub struct Flora {
    config: FloraConfig,
    tick: u64,
    stands: Vec<Stand>,
    ground: Vec<Ground>,
    ledger: FloraLedger,
    /// Sky visibility per site, sorted by site: pure terrain geometry, so it is dropped
    /// whole when the world's `terrain_version` moves and refilled lazily. A `Vec` with
    /// a binary search, never a `HashMap`: this layer iterates nothing unordered.
    sky: Vec<(Site, f64)>,
    /// The `terrain_version` `sky` was filled against, `None` before the first tick.
    sky_version: Option<u64>,
}

impl Flora {
    pub fn new(config: FloraConfig) -> Flora {
        Flora {
            config,
            tick: 0,
            stands: Vec::new(),
            ground: Vec::new(),
            ledger: FloraLedger::default(),
            sky: Vec::new(),
            sky_version: None,
        }
    }

    pub fn config(&self) -> &FloraConfig {
        &self.config
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn view(&self) -> FloraView<'_> {
        FloraView {
            config: &self.config,
            tick: self.tick,
            stands: &self.stands,
            ground: &self.ground,
            ledger: &self.ledger,
        }
    }

    /// Advance one tick against the world's current state. Call after `World::step`.
    /// Withdraws root water through `Command::WithdrawPore`; that is the only way this
    /// layer changes the world. The order within the tick is `step`'s module doc.
    pub fn step(&mut self, world: &mut World) {
        step::step(self, world);
    }

    /// Apply a command now. Returns whether it was accepted.
    pub fn apply(&mut self, world: &World, command: Command) -> bool {
        let view = world.view();
        match command {
            Command::Seed { x, z, species, wood } => {
                let Some(site) = highest_support(&view, x, z) else { return false };
                let sc = self.config.species(species);
                if !(wood.is_finite() && wood >= sc.alive_min) {
                    return false;
                }
                let Err(at) = self.stands.binary_search_by_key(&site, |s| s.site) else { return false };
                let stand = Stand {
                    site,
                    species,
                    stage: Stage::Alive,
                    wood,
                    foliage: sc.alpha * wood,
                    reserve: sc.reserve_cap * wood,
                    light: 0.0,
                    moisture: 0.0,
                    water_m3: 0.0,
                };
                let material = stand.wood + stand.foliage + stand.reserve;
                self.ledger.seeded_material_in += material;
                self.ledger.seeded_energy_in += sc.energy_density * material;
                self.stands.insert(at, stand);
                if let Err(g) = self.ground.binary_search_by_key(&site, |g| g.site) {
                    self.ledger.seeded_material_in += self.config.initial_nutrient;
                    self.ground.insert(
                        g,
                        Ground { site, nutrient: self.config.initial_nutrient, litter: 0.0, litter_energy: 0.0, dead_wood: 0.0 },
                    );
                }
                true
            }
            Command::Clear { x, z } => {
                let Some(site) = highest_support(&view, x, z) else { return false };
                let Ok(at) = self.stands.binary_search_by_key(&site, |s| s.site) else { return false };
                let s = self.stands.remove(at);
                let material = s.wood + s.foliage + s.reserve;
                self.ledger.removed_material_out += material;
                self.ledger.removed_energy_out += self.config.species(s.species).energy_density * material;
                true
            }
        }
    }
}

/// The highest support face in a column: the skyline solid, if the void above it is
/// inside the world. Only the founder and clear commands use the skyline; the model
/// itself works on any support face.
pub fn highest_support(view: &VoxelView<'_>, x: i64, z: u32) -> Option<Site> {
    let c = view.config;
    if z >= c.depth {
        return None;
    }
    let y = view.surface_y(x, z)?;
    if y + 1 >= c.height {
        return None;
    }
    Some(Site { x: x.rem_euclid(c.width as i64) as u32, y, z })
}
