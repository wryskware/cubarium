//! Producers on the voxel strip. Two species — bloomcrown, a sun producer of ridges
//! and terraces, and umbrellafrond, a shade-and-wet producer of hollows — live as
//! **stands** on support faces of a [`cubarium_voxel::World`], compete for light
//! through their own canopies and for pore water through the core's bounded
//! withdrawal, and spread by paid propagules.
//!
//! This crate never draws and never reads the clock. It reads the world through
//! [`cubarium_voxel::VoxelView`] and changes it only through
//! [`cubarium_voxel::Command`]s; the ledgers stay separate (water in the core, organic
//! matter, mineral and energy here). A stand's location is a **support voxel**: a solid
//! whose top face is exposed. Never a column's skyline.
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
    /// The mineral nutrient this stand's tissue holds. A **stock**, not `n_tissue · O`:
    /// new tissue draws `n_tissue` per unit from the site's pool, but respiration takes
    /// organic matter away and leaves the mineral where it is, so a starving stand ends
    /// up mineral-rich per unit of what is left. Every transfer out of a stand — litter,
    /// dead wood, a propagule package — takes the same fraction of this as it takes of
    /// the stand's organic matter.
    pub mineral: f64,
}

impl Stand {
    /// `W + P + Q`: the organic matter this stand holds.
    pub fn organic(&self) -> f64 {
        self.wood + self.foliage + self.reserve
    }
}

/// Non-living stocks that sit on a site: the mineral pool and the stand's own detritus.
/// One entry per site that has ever held a stand or received litter.
///
/// Organic matter and mineral nutrient are **two** stocks here, not one number as they
/// were in ecology v1: respiring a kilogram of wood does not produce a kilogram of
/// fertilizer. Litter and dead wood carry organic matter and the mineral that was in the
/// tissue; the site's `mineral` is what a stand can actually draw on to build with.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ground {
    pub site: Site,
    /// `N`: the mineral pool a stand on this site draws on to build tissue. Never
    /// organic matter: nothing respires into it.
    pub mineral: f64,
    /// `D`: leaf litter, organic matter.
    pub litter: f64,
    /// `De`: energy retained in the litter.
    pub litter_energy: f64,
    /// The mineral held in the litter, released to `mineral` as the litter decomposes.
    pub litter_mineral: f64,
    /// `Wd`: dead wood, organic matter. Keeps its identity; nothing eats it this round.
    pub dead_wood: f64,
    /// The mineral held in the dead wood, released to `mineral` as it decomposes.
    pub dead_wood_mineral: f64,
    /// Energy in the dead wood, `e_v` per unit at the moment it died. Held, not
    /// respired: a standing dead trunk is energy-dense and unavailable, and it releases
    /// its energy as heat only as the wood decomposes. Kept as its own stock rather than
    /// as `e_v · dead_wood` because two species with different `energy_density` can
    /// leave dead wood on one site and `Ground` has no species.
    pub dead_wood_energy: f64,
}

impl Ground {
    /// An empty site holding `mineral` and nothing else.
    pub fn new(site: Site, mineral: f64) -> Ground {
        Ground {
            site,
            mineral,
            litter: 0.0,
            litter_energy: 0.0,
            litter_mineral: 0.0,
            dead_wood: 0.0,
            dead_wood_mineral: 0.0,
            dead_wood_energy: 0.0,
        }
    }
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
    /// `e_v`: energy per unit of organic matter in every plant tissue. Light is the source.
    pub energy_density: f64,
    /// `n_tissue`: mineral nutrient per unit of organic matter this species builds. Wood,
    /// foliage and reserve share it this round. Growing `ΔO` draws `n_tissue · ΔO` from
    /// the site's mineral pool, and the pool caps growth through `mineral / n_tissue`.
    /// **Placeholder**; nothing here is tuned.
    pub n_tissue: f64,
    /// `q_share`: share of every tick's surplus that goes to the reserve first.
    pub reserve_share: f64,
    /// `p_reflush`: reserve is spent on foliage only while `P < p_reflush · P_cap`.
    pub reflush_below: f64,
    /// `g`: gross assimilation per second per unit foliage at full light and moisture.
    pub assimilation: f64,
    /// `K_N`: mineral half-saturation of assimilation.
    pub nutrient_half: f64,
    /// `f_max`: the fastest a stand can turn its site's mineral pool into income, per
    /// second per unit of pool. A rate cap on `A`, not on the mineral draw itself.
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
    ///
    /// The threshold means a **pool**, not a rain film. What the core reports as water
    /// depth at the end of a tick includes runoff still on its way downhill — thin, but
    /// not zero, and over most of the world's support faces while it is raining — so a
    /// species whose limit is literally zero drowns everywhere it rains. Give even the
    /// least water-tolerant species a fraction of a voxel.
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
            n_tissue: 0.02,
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
            // A fifth of a voxel: bloomcrown dies in a pool and shrugs off a shower.
            // This was 0.0, which is not "dies in standing water" but "dies in any
            // water at all": measured on the default generated world under rain, every
            // bloomcrown founder died within three ticks in 0.3 to 2 mm of transit
            // water, so the species could not exist anywhere it rained. A wrong
            // placeholder, not a tuned one.
            drown_depth_m: 0.05,
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
    /// `k_d`: litter decomposition per second. Its organic matter is respired out of the
    /// system (`respired_out`, energy to heat) and its mineral is released to the site's
    /// pool at the same fraction.
    pub decomposition: f64,
    /// `k_w`: dead-wood decomposition per second, the same two flows, slow.
    pub wood_decomposition: f64,
    /// `e_d_max`: retained energy cap per unit of litter.
    pub litter_energy_cap: f64,
    /// Mineral a site starts with the first time anything lands on it. Booked as
    /// `seeded_mineral_in`: it comes from outside the closed system, so it is a named
    /// inflow and not a residual.
    pub initial_mineral: f64,
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
            initial_mineral: 1.0,
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

/// Cumulative external fluxes of the plant layer, in three currencies: **organic
/// matter**, **mineral** and **energy**. Internal transfers (growth, litterfall,
/// propagules, mineralization) never appear here. Water is neither: transpiration lives
/// in the core's water ledger.
///
/// Organic matter is not a closed stock — it enters from light and leaves as respiration
/// — so it has two named boundary flows, [`FloraLedger::fixed_in`] and
/// [`FloraLedger::respired_out`]. Mineral **is** closed: the only way in or out is a
/// `Seed`, a new site's `initial_mineral`, or a removal.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FloraLedger {
    /// Energy fixed from light: `e_v · A`, the energy side of `fixed_in`.
    pub light_in: f64,
    /// Energy respired.
    pub heat_out: f64,
    /// Organic matter assimilation created from light. The boundary Astra calls the CO2
    /// supply, entering.
    pub fixed_in: f64,
    /// Organic matter respiration removed from the system: maintenance, construction
    /// `c_g`, reflush, the leftover of a capped income, and decomposition of litter and
    /// dead wood. The same boundary, leaving. Mineral never crosses it.
    pub respired_out: f64,
    /// Organic matter, mineral and energy a `Seed` command — or a new site's
    /// `initial_mineral` — created.
    pub seeded_organic_in: f64,
    pub seeded_mineral_in: f64,
    pub seeded_energy_in: f64,
    /// Organic matter, mineral and energy removed because a terrain edit buried or
    /// removed the support of a site, or a `Clear` command removed a stand. Reported,
    /// never hidden.
    pub removed_organic_out: f64,
    pub removed_mineral_out: f64,
    pub removed_energy_out: f64,
    /// Water withdrawn through `Command::WithdrawPore`, cubic metres, for cross-checking
    /// against the core ledger's `transpiration_out`.
    pub transpired_m3: f64,
    pub establishments: u64,
    pub deaths: u64,
}

impl FloraLedger {
    pub fn expected_organic(&self) -> f64 {
        self.seeded_organic_in + self.fixed_in - self.respired_out - self.removed_organic_out
    }

    pub fn expected_mineral(&self) -> f64 {
        self.seeded_mineral_in - self.removed_mineral_out
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

    /// Organic matter in every living and dead stock. The site's mineral pool is not
    /// organic matter and is not in here.
    pub fn organic(&self) -> f64 {
        self.stands.iter().map(|s| s.organic()).sum::<f64>()
            + self.ground.iter().map(|g| g.litter + g.dead_wood).sum::<f64>()
    }

    /// Mineral in every stock: the sites' pools, the mineral held in litter and dead
    /// wood, and the mineral standing in living tissue.
    pub fn mineral(&self) -> f64 {
        self.stands.iter().map(|s| s.mineral).sum::<f64>()
            + self
                .ground
                .iter()
                .map(|g| g.mineral + g.litter_mineral + g.dead_wood_mineral)
                .sum::<f64>()
    }

    /// Energy in every living and dead stock.
    pub fn energy(&self) -> f64 {
        self.stands
            .iter()
            .map(|s| self.config.species(s.species).energy_density * s.organic())
            .sum::<f64>()
            + self.ground.iter().map(|g| g.litter_energy + g.dead_wood_energy).sum::<f64>()
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
                let mut stand = Stand {
                    site,
                    species,
                    stage: Stage::Alive,
                    wood,
                    foliage: sc.alpha * wood,
                    reserve: sc.reserve_cap * wood,
                    light: 0.0,
                    moisture: 0.0,
                    water_m3: 0.0,
                    mineral: 0.0,
                };
                let organic = stand.organic();
                // A founder arrives at the species' own tissue mineral content: it is
                // grown outside the system, so its mineral is seeded in with it.
                stand.mineral = sc.n_tissue * organic;
                self.ledger.seeded_organic_in += organic;
                self.ledger.seeded_mineral_in += stand.mineral;
                self.ledger.seeded_energy_in += sc.energy_density * organic;
                self.stands.insert(at, stand);
                if let Err(g) = self.ground.binary_search_by_key(&site, |g| g.site) {
                    self.ledger.seeded_mineral_in += self.config.initial_mineral;
                    self.ground.insert(g, Ground::new(site, self.config.initial_mineral));
                }
                true
            }
            Command::Clear { x, z } => {
                let Some(site) = highest_support(&view, x, z) else { return false };
                let Ok(at) = self.stands.binary_search_by_key(&site, |s| s.site) else { return false };
                let s = self.stands.remove(at);
                let organic = s.organic();
                self.ledger.removed_organic_out += organic;
                self.ledger.removed_mineral_out += s.mineral;
                self.ledger.removed_energy_out += self.config.species(s.species).energy_density * organic;
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
