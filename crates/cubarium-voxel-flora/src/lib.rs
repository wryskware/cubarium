//! Producers on the voxel strip. Five species — bloomcrown, the light-demanding producer
//! of sunny aerated soil; umbrellafrond, the wetland producer of hollows; springturf, the
//! pioneer turf of open moist soil; stonecushion, the cushion of bare rock with a soil
//! pocket in reach; and velvetpad, the moist aerated understory pad — live as **stands**
//! on support faces of a [`cubarium_voxel::World`], compete for light through their own
//! canopies and for pore water through the core's bounded withdrawal, and spread by paid
//! propagules.
//!
//! Each species is a **role first** and a set of numbers second: the role is one sentence
//! of ecology in the preset's own doc comment, and every number encoding it is a
//! placeholder listed in `design/backlog.md` §1. Nothing here is tuned.
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
//!
//! # Transfers to a consumer layer, and the producer response (round 5a)
//!
//! A consumer eats through three **bounded withdrawals** and puts material back through one
//! **deposit**, all applied between ticks the way [`Command`]s are:
//! [`Flora::take_foliage`], [`Flora::take_dead_wood`], [`Flora::take_litter`] and
//! [`Flora::deposit`], with [`FloraView::reachable_foliage`] as the geometry of what a
//! ground browser can actually get at. Each of them is a named boundary flow of the ledger
//! ([`FloraLedger::consumed_organic_out`] and [`FloraLedger::deposited_organic_in`] and
//! their mineral and energy siblings), so the three residuals hold to the bit with a
//! consumer present. There is no animal body here, no movement, no population and no
//! carrying capacity: this layer says only what may be taken and what a deposit becomes.
//!
//! **The producer response needs no new rule, and none was added.** A grazed stand's
//! foliage drops; its income drops with it, because income is
//! `assimilation · L_eff · μ · (1 − stress) · P · monod` and `P` is exactly what was taken;
//! and the foliage comes back through the `foliage_rate` growth and the
//! reflush-from-reserve rules that were already there. A stand grazed faster than it can
//! reflush pays the difference out of its reserve, and one held at zero foliage fixes
//! nothing at all and lives on reserve until dieback. "Recovery" is therefore a
//! **measurement** of those rules (`design/7_Research/voxel-round3-experiment-2026-09-16.md`,
//! "Round 5a") and not a mechanic of its own.

#![forbid(unsafe_code)]

mod layers;
pub mod snapshot;
mod step;

use cubarium_voxel::{Material, VoxelView, World};
use serde::{Deserialize, Serialize};

pub use cubarium_voxel::{DT, TICK_HZ};
pub use layers::{
    Layer, LayerKind, MAX_FOLIAGE_LAYERS, MIN_LAYER_AREA_M2, Profile, StandLayer, disc_offset,
    trunk_offsets,
};
/// The germination predicate the model itself uses, for a caller outside a tick: a
/// harness picking founder columns, a diagnosis of which gate is shut. There is one
/// predicate, and this is it.
pub use step::can_establish;
/// The same predicate, gate by gate, for a caller that needs to know **which** gate shut:
/// `Gates::passes()` is exactly `can_establish`.
pub use step::{Gates, establishment_gates, establishment_gates_on_substrate};
/// The same predicate with the geometric sky reading supplied by a caller that already has
/// it — the batch observation path, [`FloraView::establishment_gates_over`], where one ray
/// per site is shared across species.
pub use step::{adult_light_cover, establishment_gates_with_sky};

/// The **stands** of the voxel ecology, each one a role: see the preset that carries its
/// numbers ([`SpeciesConfig::bloomcrown`] and the five after it) for the sentence of
/// ecology the numbers encode.
///
/// Five of the six are producers. The sixth, [`Species::Glowcap`], is a
/// [`Trophic::Saprotroph`]: the same stand, the same lifecycle, and dead organic matter —
/// dead wood and litter both — where the light was. "Species" is therefore the crate's word for a kind of stand and not a claim
/// that they are all plants.
///
/// The first two are the pair of the first coupled experiment and keep slots 0 and 1, so
/// that a [`FloraLedger`] array read by index still means what it meant in round 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Species {
    /// The light-demanding producer of sunny, aerated soil: high light, shallow roots,
    /// tolerates dry, dies in standing water.
    Bloomcrown,
    /// The wetland producer: tolerates low light, deep roots, wants sustained wetness,
    /// saturation-immune, taller crown.
    Umbrellafrond,
    /// The pioneer turf of open, moist soil: shallow, sun-demanding, fast, short-lived,
    /// wide hop, a crown one cell tall.
    Springturf,
    /// The cushion of bare rock whose roots reach a soil pocket: drought-tolerant, slow,
    /// tiny.
    Stonecushion,
    /// The moist, aerated understory pad: shade-tolerant, damp but not waterlogged soil,
    /// low and broad.
    Velvetpad,
    /// The wood fungus of the decomposer grove: **not a plant**. It earns nothing from
    /// light, eats the dead wood under and around it, and fruits one cap.
    Glowcap,
}

impl Species {
    /// How many species there are, and the length of every per-species array in
    /// [`FloraLedger`]. Derived from [`Species::ALL`] so the two can never disagree.
    pub const COUNT: usize = Species::ALL.len();

    pub const ALL: [Species; 6] = [
        Species::Bloomcrown,
        Species::Umbrellafrond,
        Species::Springturf,
        Species::Stonecushion,
        Species::Velvetpad,
        Species::Glowcap,
    ];

    /// This species' slot in the per-species arrays of [`FloraLedger`], and the same index
    /// [`Species::ALL`] holds it at. A fixed order, never an iteration order.
    pub fn index(self) -> usize {
        match self {
            Species::Bloomcrown => 0,
            Species::Umbrellafrond => 1,
            Species::Springturf => 2,
            Species::Stonecushion => 3,
            Species::Velvetpad => 4,
            Species::Glowcap => 5,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Species::Bloomcrown => "bloomcrown",
            Species::Umbrellafrond => "umbrellafrond",
            Species::Springturf => "springturf",
            Species::Stonecushion => "stonecushion",
            Species::Velvetpad => "velvetpad",
            Species::Glowcap => "glowcap",
        }
    }

    pub fn parse(s: &str) -> Option<Species> {
        Species::ALL.into_iter().find(|sp| sp.name() == s)
    }
}

/// Life stage of a stand. A site with no stand is **bare**; propagule material waiting on
/// a site is a [`SeedCohort`] in that site's [`Ground`], never a stand.
///
/// Round 3 deleted `Establishing`. A frozen sub-`W_min` stand was not a seed bank: it
/// could not die, it never aged, and it needed one donor's whole attention for 300 s to
/// cross `alive_min`. One variant is left on purpose — a stand is alive or it is not
/// there — so that code which matches on a stage keeps saying which it means.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    /// Runs the full plant model every tick.
    Alive,
}

/// One dormant seed cohort on a site: paid propagule material of one species that landed
/// in one **arrival-time bin**.
///
/// Not a stand. It has no income, no maintenance and no growth — only attrition, an age
/// limit, and the chance to germinate when the site passes its species' establishment
/// predicate and the bank is big enough to build a living stand out of.
///
/// A cohort is a bin and not a delivery: its age is measured from
/// [`SeedCohort::bin_start_tick`], the first tick of the window it landed in, so a later
/// landing that joins the same bin adds material without moving the age. Round 3b's
/// correction (Astra R4.1): the old rule merged a cohort of age 1 with one of age 0 and
/// kept age 0, so `seed_max_age_s` measured the time since the bank's **last delivery**
/// and an arbitrarily small fresh arrival could retain old material for ever.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeedCohort {
    pub species: Species,
    /// Organic matter in the cohort.
    pub organic: f64,
    /// The mineral that came with it, from the donor's own tissue.
    pub mineral: f64,
    /// The first tick of the arrival bin this material landed in: a multiple of the
    /// species' bin width, `seed_max_age_s / seed_cohorts_max`. Never moves once the bin
    /// exists, so the bin's age never decreases.
    pub bin_start_tick: u64,
}

impl SeedCohort {
    /// Ticks since this bin opened, at tick `now`. Monotone in `now` by construction:
    /// nothing a later landing does can make it smaller.
    pub fn age_ticks(&self, now: u64) -> u64 {
        now.saturating_sub(self.bin_start_tick)
    }

    /// The same age in seconds, which is what `seed_max_age_s` is compared against.
    pub fn age_s(&self, now: u64) -> f64 {
        self.age_ticks(now) as f64 * DT
    }
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
    /// This stand's identity, from [`FloraLedger::births`]: every stand this layer has
    /// ever created has its own, founders included, and it never changes or repeats.
    ///
    /// Round 3b (Astra R4.7): a harness that told founders from descendants by watching
    /// each founder's **site** could not see a founder die and its own species germinate
    /// on that site in the same tick, which steps 6 and 8 of the tick permit. A
    /// descendant count by identity sees it.
    pub id: u64,
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
    /// the stand's material.
    ///
    /// It is an **inventory, not a reusable internal nutrient reserve**, and that is a
    /// stated limitation of this round rather than a physiological claim (Astra R4.3).
    /// Income reads the *site's* pool: `assimilation`'s Michaelis–Menten term, the
    /// `nutrient_draw_max` rate cap and the `mineral / n_tissue` stock cap all read
    /// `Ground::mineral`, so a stand rich in mineral fixes **zero** on a bare pool at full
    /// light and full moisture and has to burn reserve to stand still
    /// (`tests/round3.rs`, `a_bare_mineral_pool_stops_the_income_and_not_just_the_growth`
    /// pins it). Moving `mineral / n_tissue` alone would not change that: the other two
    /// terms zero the income independently. When nutrient physiology matters, carbon
    /// fixation and maintenance have to be separated from mineral-funded tissue
    /// construction, and the nutrient response pointed at whatever stock is then the
    /// usable one. Until then this number is what the stand is *made of*, not what it can
    /// spend.
    pub mineral: f64,
    /// Waterlogging of the root zone, `0..=1`. It relaxes toward the level its root box's
    /// saturated fraction asks for — nothing at or below the species'
    /// `establish_saturated_max`, everything at a wholly saturated box, in proportion
    /// between — and it multiplies income by `1 − aeration_stress`, so a stand in a root
    /// zone wetter than it tolerates earns that much less and a drowned one earns nothing
    /// and diebacks on unpaid maintenance. That is the "roots drowned" death the sketch
    /// wanted, with no new kill switch: standing-water drowning (`drown_depth_m`) is a
    /// separate and much cruder thing.
    pub aeration_stress: f64,
    /// Organic matter this stand has paid out of its reserve for reproduction and is
    /// **saving**: a propagule package under construction. It is no longer the stand's
    /// tissue — it earns nothing, it is not `W`, `P` or `Q`, and it cannot be spent on
    /// maintenance — but it is still inside the system, so it counts in
    /// [`FloraView::organic`] through [`Stand::material`] and it goes to litter when the
    /// stand dies.
    ///
    /// Round 3b (Astra R4.4): the old rule gave the donor a budget of
    /// `propagule_rate · dt · recipient_count` and split it across every recipient, so
    /// each recipient's share was a rate *per recipient* paid out of one scarce reserve,
    /// and no bank ever came near the germination threshold while its donor was stressed —
    /// bloomcrown's best bank sat at a seventh of it for 2,000 s. A donor now saves one
    /// recipient's worth per tick until it holds one whole minimum package and sends
    /// **that**, to one recipient. The mineral of the parcel stays in the stand until the
    /// package leaves, and travels with it by the fraction rule.
    pub parcel: f64,
    /// **Where this stand's foliage is**: one stock per foliage-bearing layer of the
    /// profile stage `profile_stage` names, bottom-up, in organic units. Entries past
    /// that stage's layer count are zero.
    ///
    /// The invariant, and the reason this exists at all: `layer_stock[..n]` sums to
    /// [`Stand::foliage`] at all times, and `foliage` stays the scalar the ledgers, the
    /// water rule and the mineral rule read. A scalar total cannot tell a plant grazed
    /// from below apart from one grazed evenly — taking 0.10 from a 0.25 / 0.75 stand
    /// leaves 0.15 / 0.75, and re-deriving fixed shares of the remaining 0.90 would give
    /// 0.225 / 0.675 and move a twelfth of the crown into reachable tissue with no growth
    /// (`design/7_Research/organism-systems-audit-2026-09-21.md` §5, "Persistent lower
    /// depletion"). So the amounts are a state and not a derivation.
    ///
    /// A fixed array rather than a `Vec` because a [`Stand`] is `Copy` and is copied by
    /// value throughout the crate and the host.
    #[serde(default)]
    pub layer_stock: [f64; MAX_FOLIAGE_LAYERS],
    /// Which entry of the species' `profile` the stocks above are binned for. When the
    /// stand's wood crosses a stage threshold, the tick re-bins the **existing** total
    /// into the new stage bottom-up and updates this; nothing is created
    /// (`design/handoffs/voxel-organism-decisions-2026-09-21.md` §4).
    #[serde(default)]
    pub profile_stage: u8,
}

impl Stand {
    /// `W + P + Q`: the organic matter this stand's **tissue** holds. Not the parcel: that
    /// is paid-out material in transit, and a stand cannot live on it.
    pub fn organic(&self) -> f64 {
        self.wood + self.foliage + self.reserve
    }

    /// The stocks that are live under `count` foliage layers.
    pub fn layer_stocks(&self, count: usize) -> &[f64] {
        &self.layer_stock[..count.min(MAX_FOLIAGE_LAYERS)]
    }

    /// The same, mutably.
    pub(crate) fn layer_stocks_mut(&mut self, count: usize) -> &mut [f64] {
        &mut self.layer_stock[..count.min(MAX_FOLIAGE_LAYERS)]
    }

    /// Put `foliage` into the stage `sc` gives this stand's wood, bottom-up, and record
    /// the stage. Used wherever a stand is **created** — a founder, a germination — and
    /// by the stage-transition re-bin.
    pub(crate) fn bin_foliage(&mut self, sc: &SpeciesConfig, voxel_m: f64) {
        let index = sc.profile_index(self.wood);
        let caps = sc.layer_capacities(self.wood);
        self.layer_stock = [0.0; MAX_FOLIAGE_LAYERS];
        let n = caps.len();
        let total = self.foliage;
        layers::rebin(self.layer_stocks_mut(n), &caps, total);
        self.profile_stage = index as u8;
        let _ = voxel_m;
    }

    /// Re-bin **only if** the stand has crossed a stage threshold since the stocks were
    /// last binned. Called after every tick's wood change.
    pub(crate) fn resync_layers(&mut self, sc: &SpeciesConfig) {
        let index = sc.profile_index(self.wood);
        if index as u8 != self.profile_stage {
            let caps = sc.layer_capacities(self.wood);
            let n = caps.len();
            let total = self.foliage;
            self.layer_stock = [0.0; MAX_FOLIAGE_LAYERS];
            layers::rebin(self.layer_stocks_mut(n), &caps, total);
            self.profile_stage = index as u8;
        }
    }

    /// Bin this stand's `foliage` into its species' profile, bottom-up, and record the
    /// stage. A fixture that builds a [`Stand`] by hand calls this so its layers are
    /// consistent with its scalar; everything inside the crate is binned already.
    pub fn bin_layers(&mut self, config: &FloraConfig) {
        let sc = config.species(self.species);
        self.bin_foliage(sc, config.voxel_m);
    }

    /// Settle the stocks against the scalar after an arithmetic step has moved
    /// `foliage`: the residue of `foliage - sum(stocks)` — float dust, because the two
    /// were moved by the same amounts in a different order — is pushed into the largest
    /// stock, which can always absorb it. Without this the invariant would drift a few
    /// ulps per tick and stop being an invariant.
    pub(crate) fn settle_layers(&mut self, count: usize) {
        let n = count.min(MAX_FOLIAGE_LAYERS);
        if n == 0 {
            return;
        }
        let sum: f64 = self.layer_stock[..n].iter().sum();
        let residue = self.foliage - sum;
        if residue == 0.0 {
            return;
        }
        let mut best = 0;
        for i in 1..n {
            if self.layer_stock[i] > self.layer_stock[best] {
                best = i;
            }
        }
        self.layer_stock[best] = (self.layer_stock[best] + residue).max(0.0);
    }

    /// `W + P + Q + parcel`: every unit of organic matter this stand holds, which is what
    /// the ledger has to account for and what the stand's mineral is a stock against.
    pub fn material(&self) -> f64 {
        self.organic() + self.parcel
    }
}

/// Non-living stocks that sit on a site: the mineral pool and the stand's own detritus.
/// One entry per site that has ever held a stand or received litter.
///
/// Organic matter and mineral nutrient are **two** stocks here, not one number as they
/// were in ecology v1: respiring a kilogram of wood does not produce a kilogram of
/// fertilizer. Litter and dead wood carry organic matter and the mineral that was in the
/// tissue; the site's `mineral` is what a stand can actually draw on to build with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// The site's seed bank, **sorted by species then `bin_start_tick`**, oldest first.
    /// One entry per species per arrival bin: a landing joins the bin whose window covers
    /// the current tick, creating it if it is not there, so a site a donor feeds every
    /// tick holds one cohort per species per bin and not one per tick. The bank is bounded
    /// by construction — a bin's age never decreases and a bin older than
    /// `seed_max_age_s` leaves whole, so at most `seed_cohorts_max + 1` bins of one
    /// species can be alive at once.
    pub seeds: Vec<SeedCohort>,
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
    /// `C`: **carrion** — organic matter a consumer deposited here, a corpse or a part of
    /// one ([`Flora::deposit`] with [`DepositKind::Carrion`]). Its own pool and not litter,
    /// because it decomposes at its own rate ([`FloraConfig::carrion_decomposition`]) and
    /// because a picture will want to draw remains as remains.
    ///
    /// No plant ever creates it: dead plant tissue is litter and dead wood, as it was. This
    /// pool exists so that a consumer layer has somewhere to put a body, and it is emptied
    /// by the same `decompose` phase on the same two flows — organic matter respired out of
    /// the system, mineral released to `mineral` at the stock's own fraction.
    pub carrion: f64,
    /// The mineral held in the carrion, released to `mineral` as it decomposes.
    pub carrion_mineral: f64,
    /// Energy in the carrion, as the depositing consumer handed it over. Held and released
    /// as heat at the stock's current density as the carrion decomposes — dead wood's rule,
    /// and **not** litter's: there is no `e_d_max` cap on this pool, because a deposit's
    /// energy comes from a consumer's own books rather than from a species'
    /// `energy_density`, and a cap here would be a second knob with nothing measuring it.
    pub carrion_energy: f64,
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
            seeds: Vec::new(),
            dead_wood: 0.0,
            dead_wood_mineral: 0.0,
            dead_wood_energy: 0.0,
            carrion: 0.0,
            carrion_mineral: 0.0,
            carrion_energy: 0.0,
        }
    }

    /// The organic matter this site's seed bank holds of one species: what germination
    /// pools and measures against `alive_min / w_frac`.
    pub fn seed_organic(&self, species: Species) -> f64 {
        self.seeds
            .iter()
            .filter(|c| c.species == species)
            .map(|c| c.organic)
            .sum()
    }

    /// The mineral the same cohorts hold.
    pub fn seed_mineral(&self, species: Species) -> f64 {
        self.seeds
            .iter()
            .filter(|c| c.species == species)
            .map(|c| c.mineral)
            .sum()
    }

    /// Which species' cohorts hold the most organic matter here, for a picture with one
    /// glyph per site. Ties go to the earlier species in [`Species::ALL`], which is a
    /// fixed order and not an iteration order.
    pub fn seed_species(&self) -> Option<Species> {
        let mut best: Option<(Species, f64)> = None;
        for species in Species::ALL {
            let o = self.seed_organic(species);
            if o > 0.0 && best.is_none_or(|(_, b)| o > b) {
                best = Some((species, o));
            }
        }
        best.map(|(s, _)| s)
    }
}

/// **How a species earns.** One number's worth of ecology, and the only thing in this crate
/// that changes which income rule a stand runs.
///
/// Everything else about a stand — where it may establish, how it grows, how it diebacks,
/// how it dies, how it reproduces — is one set of rules for both modes. A saprotroph is not
/// a second model: it is the same stand with its income line replaced, which is what
/// `design/theoretical-biosphere-2026-09-16.md` §6 asks for ("reuse stand
/// location/lifecycle structure, replace income and substrate rules; no light income").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Trophic {
    /// **A plant.** Income is light: `assimilation · L_eff · μ · (1 − stress) · P · monod`,
    /// capped by the mineral pool, and the organic matter is created at the boundary
    /// ([`FloraLedger::fixed_in`]).
    #[default]
    Photo,
    /// **A saprotroph.** Income is dead organic matter — **dead wood and litter both**:
    /// at most [`SpeciesConfig::substrate_uptake_per_s`]` · W · μ · dt` taken pro rata
    /// from the dead-wood **and litter** pools of the sites in its **mycelium box**, of
    /// which [`SpeciesConfig::substrate_yield`] becomes tissue and the rest is respired at
    /// once. The yield is applied to each pool's withdrawal separately, because the two
    /// carry their own mineral and energy per unit. No light gate, no light income, and
    /// nothing is created at the boundary: the organic matter was already in the system,
    /// in the log or in the leaf fall. Carrion is not substrate: a corpse is a consumer's
    /// pool and out of scope.
    Saprotroph,
}

/// The plant model of one species: `design/ecology-v1-contract.md` §4 parameters, plus
/// the terrain couplings that replace the old noise fields.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SpeciesConfig {
    /// Which income rule this species runs: [`Trophic::Photo`] for the five plants,
    /// [`Trophic::Saprotroph`] for a wood fungus. Nothing else in the config changes
    /// meaning with it — the three `substrate_*` fields below are simply inert on a
    /// `Photo` species, and the light fields are inert on a `Saprotroph`.
    pub trophic: Trophic,
    /// **Saprotroph only.** Organic matter a mycelium withdraws from the dead wood **and
    /// litter** in its box per second per unit of `W`, at full moisture. The uptake is
    /// `substrate_uptake_per_s · W · μ · dt` against the **sum** of the two, bounded again
    /// by what the pools actually hold, so a drying log and a bare face both starve the
    /// fungus. **Placeholder**
    /// (`design/backlog.md` §1); inert on a [`Trophic::Photo`] species.
    pub substrate_uptake_per_s: f64,
    /// **Saprotroph only.** The fraction of the organic matter taken that becomes tissue;
    /// the rest is respired at once (`respired_out`, heat). The brief's `yield`, spelled out
    /// because `yield` is a reserved word. **Placeholder**; inert on a `Photo` species.
    pub substrate_yield: f64,
    /// **Saprotroph only.** Substrate — dead wood **and** litter — the sites of the
    /// mycelium box must hold, in total, before a spore cohort may germinate there. **Placeholder**; inert on a `Photo`
    /// species, whose substrate gate is open by construction.
    pub establish_substrate_min: f64,
    /// **Saprotroph only.** How many support-face rows **up and down** the mycelium box
    /// reaches from the stand's own face: the vertical reach of *substrate access*, and
    /// nothing to do with water. **Placeholder 1** (`design/backlog.md` §1); inert on a
    /// [`Trophic::Photo`] species, which never walks the box.
    ///
    /// Astra R9.3, and a rule decision rather than a repair. The box used to borrow
    /// `rooting_depth`'s geometry, which reaches only **down**, so at `rooting_depth` 1 the
    /// box was the stand's own row alone: a spore landing one voxel above or below a full
    /// log had no wood in its box, and the round-5b `community` run refused all three of
    /// its landings on exactly that (`design/7_Research/voxel-round3-experiment-2026-09-16.md`,
    /// "Round 5b"). Mycelium in a log is not a root in soil and has no reason to share the
    /// root box's downward asymmetry, so substrate access now has its own **symmetric**
    /// reach, and the soil-water box is left exactly as it was — raising `rooting_depth`
    /// would have moved the water too and still only repaired one direction.
    ///
    /// Sideways the box is still `rooting_radius`, and blind paid landing plus
    /// germination-time selection are unchanged: a spore lands on the highest support face
    /// of a column inside its `hop` and the gates are read *there*, so a grove still ends
    /// where the wood does.
    pub substrate_reach_up_down: u32,
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
    /// `W_min`: a stand with less wood than this dies. Germination needs a seed bank
    /// holding at least `alive_min / w_frac`, so that the stand it builds is born alive.
    pub alive_min: f64,
    /// `W_est`: wood a stand needs before it can send propagules.
    pub donor_min: f64,
    /// `q_prop`: the fraction of `Q_max` a donor keeps for itself.
    pub donor_reserve_floor: f64,
    /// `k_est`: the **gross** organic matter a donor sets aside for reproduction per
    /// second — one recipient's worth, not one per neighbour (round 3b, Astra R4.4/R5.6).
    ///
    /// The implemented contract, for a preset author:
    ///
    /// 1. Every tick, a stand whose wood is at least `donor_min` asks for
    ///    `propagule_rate · dt` of its reserve and is funded out of whatever that reserve
    ///    holds **above** `donor_reserve_floor · reserve_cap · wood`. A stand that cannot
    ///    pay asks anyway, and the gap between the two is
    ///    [`FloraLedger::propagule_requested`] against [`FloraLedger::propagule_funded`].
    /// 2. What it can pay is charged construction respiration at once — of a gross `gross`
    ///    taken out of the reserve, `gross − gross / (1 + build)` leaves as `respired_out`
    ///    and heat (Astra R6.3: not "`c_g` of the gross", which is the larger
    ///    `c_g · gross`) — and the remaining `1 / (1 + build)` of it is saved
    ///    in [`Stand::parcel`]. So a whole package costs `(1 + build)` times its own size:
    ///    0.06 of reserve for 0.05 of package at the placeholders.
    /// 3. When the parcel holds one whole **package** — `alive_min / propagule_split[0]`,
    ///    the material a germination needs to build a stand at exactly `alive_min` of wood
    ///    — that package lands on **one** support face inside `hop`, drawn from the donor's
    ///    own keyed stream, and the remainder keeps saving. Nothing lands before the parcel
    ///    is full; there is no trickle to every neighbour.
    /// 4. The recipient is drawn without habitat screening and may be occupied: the
    ///    predicate is germination's test, not landing's, and a bank waits for a gap. It is
    ///    never the donor's own site.
    ///
    /// At the placeholders one package is therefore 300 s of a fully funded donor's entire
    /// reproductive output, wherever `hop` reaches. Raising this rate raises what a donor
    /// *asks* for; it creates no income, and on a stand that is already reserve-limited it
    /// changes nothing at all.
    pub propagule_rate: f64,
    /// `w_frac, p_frac, q_frac`: how a landed propagule splits into wood, starter foliage
    /// and starter reserve. Sums to one.
    pub propagule_split: [f64; 3],
    /// `e_v`: energy per unit of organic matter in every plant tissue. Light is the source.
    pub energy_density: f64,
    /// Fraction of a seed cohort that falls to litter each second, with its mineral:
    /// paid decay, not deletion. **Placeholder**.
    pub seed_attrition_per_s: f64,
    /// A bin whose start is older than this many seconds falls to litter whole, with its
    /// mineral and its energy. **Placeholder**.
    pub seed_max_age_s: f64,
    /// How many arrival bins one species' bank is divided into: the bin width is
    /// `seed_max_age_s / seed_cohorts_max`, at least one tick, and a landing joins the bin
    /// whose window covers the current tick. So this is the **age resolution** of a bank,
    /// and the bound on its size follows from it: a bin's age never decreases and a bin
    /// past `seed_max_age_s` leaves whole, so a site holds at most `seed_cohorts_max + 1`
    /// bins of one species however a donor comes and goes. **Placeholder** — 4 is "a few",
    /// which is what [`Ground::seeds`] always claimed.
    ///
    /// Round 3b replaced two rules with this one (Astra R4.1). The first merged cohorts
    /// whose ages were within one tick and kept the **younger** age, which made
    /// `seed_max_age_s` the time since the bank's last delivery rather than a seed's
    /// lifetime: continuous arrivals, however small, retained old material for ever, and
    /// `tests/round3.rs` blessed it. The second capped the count by merging the two
    /// **oldest** cohorts at the older age, which bounded the `Vec` but swept nearly every
    /// old deposit into one bucket that could then kill much younger material at the next
    /// expiry. Fixed bins do the bounding without either effect.
    pub seed_cohorts_max: usize,
    /// `n_tissue`: mineral nutrient per unit of organic matter this species builds. Wood,
    /// foliage and reserve share it this round. Growing `ΔO` draws `n_tissue · ΔO` from
    /// the site's mineral pool, and the pool caps income through `mineral / n_tissue`.
    /// **Placeholder**; nothing here is tuned.
    ///
    /// Which of the three nutrient limits actually binds, at the placeholders (Astra
    /// R4.3): the stock cap `N / n_tissue` is `50 · N` of income per tick, the
    /// `nutrient_draw_max` rate cap is `f_max · N · dt` = `0.0005 · N`, and the
    /// Michaelis–Menten factor is `N / (N + 0.5)`. The **rate cap is five orders of
    /// magnitude tighter**, so the stock cap cannot be the binding one for any positive
    /// `N`: `n_tissue` sets the stoichiometry of the draw and the density of the tissue,
    /// and not the ceiling on income.
    ///
    /// For a [`Trophic::Saprotroph`] it **is** a ceiling on growth (Astra R9.1): a fungus's
    /// income carries its own mineral out of the log, and what it may build in a tick is
    /// `(arriving mineral + the site's pool) / n_tissue`. Unfunded income is respired where
    /// every other unspent unit is. `step`'s §4.4a is the rule.
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
    /// A seed cohort may germinate only on a site whose root-box mean pore fraction is at
    /// least this and whose sky visibility is at least `establish_light_min`.
    pub establish_pore_min: f64,
    pub establish_light_min: f64,
    /// Pore fraction (0..1 of capacity) at and above which a root voxel counts as
    /// **saturated** — no air in it for a root. **Placeholder**.
    pub saturated_pore: f64,
    /// How fast `Stand::aeration_stress` closes the gap to the level its root box asks
    /// for — a fraction of the remaining gap per second, rising with `stress_rate_per_s`
    /// and falling with `relax_rate_per_s`. The level itself is
    /// `(f − establish_saturated_max) / (1 − establish_saturated_max)`, clamped to
    /// `0..=1`, where `f` is the saturated fraction of the root box: a box no wetter than
    /// the species would germinate on asks for no stress, a wholly saturated one asks for
    /// all of it, and anything between settles between. These two rates therefore set how
    /// fast a stand reaches its level and not which level it reaches. **Placeholders**.
    pub stress_rate_per_s: f64,
    pub relax_rate_per_s: f64,
    /// A cohort may germinate only where the site's *current* saturated root fraction is
    /// at most this — and the same number is where `aeration_stress` starts to bite, since
    /// a site a species may germinate on is a site it does not stress on. The tolerant
    /// species has a high ceiling, and at 1.0 it has no saturation stress at all; the
    /// intolerant one has a low ceiling and a fast `stress_rate`. **Placeholder**.
    ///
    /// That double duty is an **assumption**, stated as one (Astra R4.6): "can germinate
    /// here" does not logically imply "pays no stress here as an adult", and a role whose
    /// seedlings are fussier than its adults, or whose adults pay something even where
    /// their seeds establish, needs the two traits separated. They are one number in this
    /// slice because nothing has measured them apart, not because they are the same
    /// quantity. Two different hypotheses come out of splitting them: a germination
    /// ceiling below the adult tolerance, and an adult response with an explicit subunit
    /// maximum or a funded cost of tolerance. Neither needs an oxygen solver.
    pub establish_saturated_max: f64,
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
    /// shades is exactly what is drawn: crown top above the support face and crown
    /// half-width, both **in metres**, `[min, max]` (package L,
    /// `design/handoffs/voxel-ladder-growth-2026-09-23.md` §1). They are physical sizes
    /// and are **not** rescaled per voxel size: every grid grows the same plant, and
    /// the voxel readings ([`SpeciesConfig::crown_height`]) divide by the world's cell.
    /// How wood moves a stand between the two ends is [`SpeciesConfig::crown_height_m_at`].
    pub crown_height_m: [f64; 2],
    pub crown_radius_m: [f64; 2],
    /// **What is in that volume**: the species' anatomy, staged by `wood / wood_max`.
    /// The first entry whose `wood_fraction_max` is at or above the fraction applies;
    /// the last must catch a full-grown stand. Its foliage-bearing layers are the
    /// stand's [`Stand::layer_stock`] entries, bottom-up.
    ///
    /// The six live species' entries are the anatomy document's §3 tables with
    /// decisions §5's corrections, and they are **authored placeholders**
    /// (`design/backlog.md` §1): nothing here has been measured or tuned.
    pub profile: Vec<Profile>,
}

impl SpeciesConfig {
    /// A foliage layer of the anatomy document's four fields.
    pub fn foliage_layer(band: [f64; 2], radius: f64, share: f64, porosity: f64) -> Layer {
        Layer {
            kind: LayerKind::Foliage,
            band,
            radius,
            share,
            porosity,
        }
    }

    /// A mat: foliage lying on the surface.
    pub fn mat_layer(band: [f64; 2], radius: f64, share: f64, porosity: f64) -> Layer {
        Layer {
            kind: LayerKind::Mat,
            ..SpeciesConfig::foliage_layer(band, radius, share, porosity)
        }
    }

    /// A trunk: structure, no share.
    pub fn trunk_layer(band: [f64; 2], radius: f64, porosity: f64) -> Layer {
        Layer {
            kind: LayerKind::Trunk,
            band,
            radius,
            share: 0.0,
            porosity,
        }
    }

    /// One stage holding the whole life: what a species with no lifecycle shape needs.
    pub fn one_stage(layers: Vec<Layer>) -> Vec<Profile> {
        vec![Profile {
            wood_fraction_max: f64::INFINITY,
            height_m_max: None,
            layers,
        }]
    }

    /// Which entry of [`SpeciesConfig::profile`] a stand of this wood is in: the first
    /// whose threshold is at or above `wood / wood_max`, and the last if none is.
    pub fn profile_index(&self, wood: f64) -> usize {
        if self.profile.is_empty() {
            return 0;
        }
        let t = (wood / self.wood_max).clamp(0.0, 1.0);
        self.profile
            .iter()
            .position(|p| p.wood_fraction_max >= t)
            .unwrap_or(self.profile.len() - 1)
    }

    /// The stage itself.
    pub fn profile_at(&self, wood: f64) -> &Profile {
        &self.profile[self.profile_index(wood).min(self.profile.len() - 1)]
    }

    /// How many foliage stocks a stand of this wood holds.
    pub fn foliage_layer_count(&self, wood: f64) -> usize {
        self.profile_at(wood)
            .foliage_layer_count()
            .min(MAX_FOLIAGE_LAYERS)
    }

    /// Each foliage layer's capacity, bottom-up: its `share` of the growth model's own
    /// foliage cap, `alpha · W` (`step`'s `p_cap`). Not a new number.
    pub fn layer_capacities(&self, wood: f64) -> Vec<f64> {
        let cap = self.alpha * wood.max(0.0);
        self.profile_at(wood)
            .foliage_layers()
            .take(MAX_FOLIAGE_LAYERS)
            .map(|(_, l)| l.share * cap)
            .collect()
    }

    /// The **capped seedling** this species opens with, if it does: `(w0, cap)`, where
    /// `w0` is stage 0's `wood_fraction_max` and `cap` its `height_m_max`. Decisions §5's
    /// woody seedling (today bloomcrown and umbrellafrond). `None` for a species whose
    /// first stage has no ceiling, or whose first stage is its whole life.
    pub fn capped_seedling(&self) -> Option<(f64, f64)> {
        let first = self.profile.first()?;
        let cap = first.height_m_max?;
        let w0 = first.wood_fraction_max;
        (cap.is_finite() && cap >= 0.0 && w0.is_finite() && w0 > 0.0 && w0 < 1.0)
            .then_some((w0, cap))
    }

    /// One crown dimension in metres at `wood`, from its `[min, max]` range: package L's
    /// growth rule (`design/handoffs/voxel-ladder-growth-2026-09-23.md` §2).
    ///
    /// - No capped seedling: linear, `min + t · (max − min)`, `t = W / W_max`.
    /// - A capped seedling `(w0, cap)`: while `t ≤ w0` (stage 0) the dimension is at
    ///   most `cap` — the seedling's **height and radius** both, so a seedling is a small
    ///   rosette and not a flat star as wide as the adult; after it, growth starts from
    ///   the seedling, `cap + (max − cap) · (t − w0) / (1 − w0)`, and the range's `min`
    ///   is unused.
    fn crown_dimension_m(&self, wood: f64, range: [f64; 2]) -> f64 {
        let t = (wood / self.wood_max).clamp(0.0, 1.0);
        let linear = range[0] + t * (range[1] - range[0]);
        match self.capped_seedling() {
            Some((_, cap)) if self.profile_index(wood) == 0 => linear.min(cap),
            Some((w0, cap)) => {
                let from = cap.min(range[1]);
                let s = ((t - w0) / (1.0 - w0)).clamp(0.0, 1.0);
                from + s * (range[1] - from)
            }
            None => linear,
        }
    }

    /// Height of the crown top above the support face, **in metres**, seedling cap and
    /// growth rule applied ([`SpeciesConfig::crown_dimension_m`]).
    pub fn crown_height_m_at(&self, wood: f64) -> f64 {
        self.crown_dimension_m(wood, self.crown_height_m)
    }

    /// Half-width of the crown, **in metres**, seedling cap and growth rule applied.
    pub fn crown_radius_m_at(&self, wood: f64) -> f64 {
        self.crown_dimension_m(wood, self.crown_radius_m)
    }

    /// Height of the crown top above the support face, in voxels of `voxel_m`.
    pub fn crown_height(&self, wood: f64, voxel_m: f64) -> f64 {
        self.crown_height_m_at(wood) / voxel_m
    }

    /// Half-width of the crown, in voxels of `voxel_m`.
    pub fn crown_radius(&self, wood: f64, voxel_m: f64) -> f64 {
        self.crown_radius_m_at(wood) / voxel_m
    }

    /// How many voxels above its support face the crown's **cells** sit:
    /// `round(crown_height)`, never zero. The crown is one disc of cells at that level, so
    /// a species whose crown height rounds to one is a crown sitting straight on the
    /// ground with no stem — which is what a one-voxel plant is — and the wood of a taller
    /// one is the cells below it.
    ///
    /// [`SpeciesConfig::crown_height`] is the float the **shade** model uses, and this is
    /// the same number quantised to whole voxels, which is what a query about *cells* — is
    /// this crown in reach, which cell does the picture paint — has to ask. The presenter's
    /// `cubarium::voxel::stand::crown_height_voxels` is the same rounding, and this is the
    /// model-side statement of it.
    pub fn crown_voxels(&self, wood: f64, voxel_m: f64) -> u32 {
        let h = self.crown_height(wood, voxel_m);
        if !h.is_finite() {
            return 1;
        }
        (h.round().max(1.0) as u32).min(u32::from(u16::MAX))
    }
}

/// How far a ground consumer can get at food from the face it is standing on: sideways in
/// whole voxels, and up in whole voxels.
///
/// Pure geometry, and deliberately crude — there is **no line of sight**, no body, no
/// posture and no cost. `design/theoretical-biosphere-2026-09-16.md` §6's point is only
/// that food above reach does not feed a ground browser, and a box is enough to say that.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Reach {
    /// Voxels sideways, in wrapped `x` and in `z`, each measured on its own: a box, not a
    /// disc.
    pub horizontal: u32,
    /// Voxels **above** the eater's own support face that it can still reach. The only
    /// vertical bound there is: a crown level with the face, or below it, is in reach
    /// whatever this says.
    pub up: u32,
}

impl SpeciesConfig {
    fn v1_base() -> SpeciesConfig {
        SpeciesConfig {
            // A plant, with the three saprotroph numbers inert. A `Photo` species never
            // reads them, and they are zero rather than absent so that a preset which
            // switches `trophic` and forgets them earns nothing and says so, instead of
            // inheriting a rate from the base.
            trophic: Trophic::Photo,
            substrate_uptake_per_s: 0.0,
            substrate_yield: 0.0,
            establish_substrate_min: 0.0,
            // One row up and one down, for a species that ever walks the box. Inert here:
            // a `Photo` species never does.
            substrate_reach_up_down: 1,
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
            seed_attrition_per_s: 0.001,
            seed_max_age_s: 600.0,
            seed_cohorts_max: 4,
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
            saturated_pore: 0.95,
            stress_rate_per_s: 0.05,
            relax_rate_per_s: 0.05,
            establish_saturated_max: 0.5,
            drown_depth_m: 0.0,
            hop: 1,
            crown_height_m: [0.25, 0.75],
            crown_radius_m: [0.125, 0.375],
            // The v1 base carries no anatomy of its own: every preset states one.
            profile: SpeciesConfig::one_stage(vec![SpeciesConfig::foliage_layer(
                [0.0, 1.0],
                1.0,
                1.0,
                0.0,
            )]),
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
            // The sun producer of ridges and terraces: it will not germinate on a site
            // that is even a quarter waterlogged, and past that quarter it stresses
            // quickly toward what the root box asks for and lets go of the stress ten
            // times more slowly. Placeholders, in the direction the sketch asks for.
            stress_rate_per_s: 0.2,
            relax_rate_per_s: 0.02,
            establish_saturated_max: 0.25,
            // A fifth of a voxel: bloomcrown dies in a pool and shrugs off a shower.
            // This was 0.0, which is not "dies in standing water" but "dies in any
            // water at all": measured on the default generated world under rain, every
            // bloomcrown founder died within three ticks in 0.3 to 2 mm of transit
            // water, so the species could not exist anywhere it rained. A wrong
            // placeholder, not a tuned one.
            drown_depth_m: 0.05,
            hop: 2,
            crown_height_m: [0.375, 1.0],
            crown_radius_m: [0.125, 0.3125],
            // Anatomy document §3, with decisions §5's corrections. A seedling is a
            // ground rosette capped at 0.125 m — not a small adult, and reachable
            // whatever the interpolated crown height says. The **adult keeps its basal
            // rosette at 0.25 of its foliage for life**: that is the floor food that
            // makes a grazed meadow possible, and the reason a low browser can eat a
            // mature bloomcrown at all.
            profile: vec![
                Profile {
                    wood_fraction_max: 0.2,
                    height_m_max: Some(0.125),
                    layers: vec![SpeciesConfig::foliage_layer([0.0, 1.0], 1.0, 1.0, 0.4)],
                },
                Profile {
                    wood_fraction_max: 0.5,
                    height_m_max: None,
                    layers: vec![
                        SpeciesConfig::trunk_layer([0.0, 0.4], 0.15, 0.0),
                        SpeciesConfig::foliage_layer([0.0, 0.25], 0.7, 0.4, 0.4),
                        SpeciesConfig::foliage_layer([0.5, 1.0], 1.0, 0.6, 0.3),
                    ],
                },
                Profile {
                    wood_fraction_max: f64::INFINITY,
                    height_m_max: None,
                    layers: vec![
                        SpeciesConfig::trunk_layer([0.0, 0.5], 0.15, 0.0),
                        SpeciesConfig::foliage_layer([0.0, 0.15], 0.6, 0.25, 0.4),
                        SpeciesConfig::foliage_layer([0.55, 1.0], 1.0, 0.75, 0.3),
                    ],
                },
            ],
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
            // The wet producer of hollows: no aeration bound on establishment at all,
            // which — since the same number is the stress target's tolerance — is also no
            // waterlogging stress at all. A wholly waterlogged hollow is its habitat, and
            // bloomcrown's `0.25` is what shuts the sun producer out of it. Its two rates
            // are therefore inert at the placeholders and kept only because a lower
            // ceiling would need them. Placeholders, in the direction the sketch asks
            // for, and the asymmetry Chesson's test needs somewhere to bite.
            //
            // 1.0 makes this species explicitly **saturation-immune**, and that is the
            // role it is standing in for: a wetland producer, not the biosphere's
            // moist-but-aerated understory role, which is a different niche and will need
            // its own preset (Astra R4.6). Do **not** lower it slightly as a repair: for
            // *every* tolerance below 1, a wholly saturated root box targets stress 1 and
            // eventually takes all of the species' assimilation again, which is the
            // failure package I measured. A partial cost at full saturation is a different
            // rule — an explicit subunit maximum on the response, or a funded cost of
            // tolerance — and not a smaller number here.
            stress_rate_per_s: 0.01,
            relax_rate_per_s: 0.05,
            establish_saturated_max: 1.0,
            drown_depth_m: 0.5,
            hop: 1,
            crown_height_m: [1.0, 2.0],
            crown_radius_m: [0.375, 0.75],
            // Anatomy document §3: three tiers whose lowest is the widest, so the
            // shade under an adult is layered. Decisions §5: the frond **escapes at the
            // seedling → juvenile transition** — the juvenile's lowest tier is already
            // over a low browser's band — and its seedling is a capped ground rosette.
            profile: vec![
                Profile {
                    wood_fraction_max: 0.15,
                    height_m_max: Some(0.125),
                    layers: vec![SpeciesConfig::foliage_layer([0.0, 1.0], 1.0, 1.0, 0.4)],
                },
                Profile {
                    wood_fraction_max: 0.5,
                    height_m_max: None,
                    layers: vec![
                        SpeciesConfig::trunk_layer([0.0, 0.5], 0.15, 0.0),
                        SpeciesConfig::foliage_layer([0.5, 0.65], 1.0, 0.6, 0.4),
                        SpeciesConfig::foliage_layer([0.85, 1.0], 0.6, 0.4, 0.5),
                    ],
                },
                Profile {
                    wood_fraction_max: f64::INFINITY,
                    height_m_max: None,
                    layers: vec![
                        SpeciesConfig::trunk_layer([0.0, 0.4], 0.2, 0.0),
                        SpeciesConfig::foliage_layer([0.4, 0.55], 1.0, 0.4, 0.4),
                        SpeciesConfig::foliage_layer([0.65, 0.8], 0.8, 0.35, 0.4),
                        SpeciesConfig::foliage_layer([0.9, 1.0], 0.5, 0.25, 0.5),
                    ],
                },
            ],
            ..SpeciesConfig::v1_base()
        }
    }

    /// **Springturf — the pioneer turf of open, moist soil.** "It wins the first years on
    /// bare moist ground and loses under a canopy" is **intended succession and untested**
    /// (Astra R7.3): round 4 measured neither half of it, and the numbers below encode the
    /// sentence rather than evidencing it. What is measured is the boundaries — a paid
    /// birth, one income window, one failing neighbour (`tests/round4.rs`). The role as the
    /// numbers state it: shallow roots that only see the support row, a light need above
    /// bloomcrown's, a moisture floor at the fraction drained soil actually settles to, a
    /// fast cheap body that turns income into packages quickly, and a maintenance bill that
    /// kills it as soon as the light or the water goes.
    ///
    /// Every number is a **placeholder** (`design/backlog.md` §1), chosen to encode that
    /// sentence and nothing else:
    ///
    /// - **Water, the three thresholds together.** `establish_pore_min` 0.25 is soil's own
    ///   retained fraction, so springturf germinates on ordinary drained soil and no
    ///   drier. That is **germination permission and not a positive newborn budget**, and
    ///   the two are separate claims (Astra R7.3): a newborn on passing soil at pore 0.26,
    ///   with ordinary preset stocks, fixes `0.008 · 1 · 0.36667 · 0.006 · (2/3) =
    ///   1.17333e-5` organic per second against maintenance `0.002 · 0.006 = 1.2e-5` and
    ///   pays the difference out of reserve — 1.06667e-5 at the 0.25 floor itself
    ///   (`tests/round4.rs`,
    ///   `springturf_germinates_on_retained_water_soil_and_is_maintenance_deficient_there`).
    ///   The species' own income fixture runs at pore **0.6**, so what it establishes is
    ///   solvency on *ample* water, not a viable pioneer on retained-water soil. Whether
    ///   recruitment has to be solvent on drained soil is a deliberate contract question
    ///   for a later round and explicitly **not** a reason to raise `assimilation`.
    ///   `wilt_pore` 0.15 is twice bloomcrown's 0.08, so it is the species that
    ///   gives up first on a dry ridge; `sat_pore` 0.45 is where it reaches full moisture,
    ///   just above the germination floor, because a turf's shallow roots either have
    ///   water in the top row or do not. `establish_saturated_max` 0.3 keeps it out of a
    ///   waterlogged hollow, and `drown_depth_m` 0.03 is under a voxel of pool but well
    ///   over the millimetres of transit water the harness's rain leaves on a face.
    /// - **Light.** `light_half` 1.0 and `establish_light_min` 0.75 are both above
    ///   bloomcrown's 0.8 and 0.6: it needs open sky to germinate and earns badly in
    ///   shade. Note that a *living crown* cannot shut that gate — germination light is
    ///   geometric sky visibility with no canopy in it (`step.rs`'s `Gates`) — so "loses
    ///   under a canopy" is an adult-income statement here and a terrain-shade statement
    ///   at germination. Making it a germination statement would be an explicit rule
    ///   addition (Astra R6.2), and this round does not make it.
    /// - **Fast and cheap.** `wood_max` 0.06 is a tenth of bloomcrown's and `alive_min`
    ///   0.006 with it, so one package is 0.015 rather than 0.05; `donor_min` 0.03 is half
    ///   its own `wood_max`, so it can reproduce at half size, where bloomcrown's 0.3 is
    ///   half of 0.6; `wood_rate` 0.01 and `foliage_rate` 0.02 are ten times the base, and
    ///   `propagule_rate` 0.002 ten times, so a funded donor fills a package in about nine
    ///   seconds against bloomcrown's three hundred. `assimilation` 0.008 is twice the
    ///   base, and the honest reason is **faster growth, not survival** (Astra R7.6): at
    ///   full foliage, full light and moisture, mineral 1 and no stress the *base* 0.004
    ///   already supplies `0.004 · α · monod = 0.0053333 · W` per second against
    ///   maintenance plus foliage replacement `0.002 · W + 1.2 · 0.001 · 2 · W =
    ///   0.0044 · W`, a 1.21× margin — so ten times the base maintenance is solvent on the
    ///   base assimilation. Doubling it takes that margin to 2.42×, which is what funds the
    ///   ten-times growth and propagule rates above rather than what keeps the species
    ///   alive.
    /// - **Short-lived.** `maintenance` 0.002 is ten times the base, so a springturf whose
    ///   income stops burns its whole reserve in 250 s and diebacks, where bloomcrown
    ///   waits 2,500 s.
    /// - **Shape.** `hop` 3 is the wide hop of a pioneer; the crown is `[0.125, 0.1875]` m
    ///   tall and `[0.125, 0.25]` m in radius (package L's ladder): a mat on the ground
    ///   with no stem at all.
    pub fn springturf() -> SpeciesConfig {
        SpeciesConfig {
            light_half: 1.0,
            rooting_depth: 1,
            rooting_radius: 1,
            wilt_pore: 0.15,
            sat_pore: 0.45,
            establish_pore_min: 0.25,
            establish_light_min: 0.75,
            stress_rate_per_s: 0.2,
            relax_rate_per_s: 0.05,
            establish_saturated_max: 0.3,
            drown_depth_m: 0.03,
            hop: 3,
            wood_max: 0.06,
            alive_min: 0.006,
            donor_min: 0.03,
            maintenance: 0.002,
            foliage_rate: 0.02,
            wood_rate: 0.01,
            propagule_rate: 0.002,
            assimilation: 0.008,
            crown_height_m: [0.125, 0.1875],
            crown_radius_m: [0.125, 0.25],
            // Anatomy document §3: a mat, no trunk at any size, entirely inside a
            // grazer's reach; cropping it is the mat thinning.
            profile: SpeciesConfig::one_stage(vec![SpeciesConfig::mat_layer(
                [0.0, 1.0],
                1.0,
                1.0,
                0.3,
            )]),
            ..SpeciesConfig::v1_base()
        }
    }

    /// **Stonecushion — the cushion of bare rock.** Its support face is rock or bedrock and
    /// its roots reach a soil pocket beside or just under it; it asks almost nothing of the
    /// water in that pocket and grows at a crawl.
    ///
    /// **No rule addition was needed for the rock support, and none was made.** The gates
    /// never read the support voxel's material: `root_box` collects the `Material::Soil`
    /// voxels of the box and the pore gate reads their capacity-weighted mean, so a rock
    /// support with one soil voxel inside `rooting_radius` already passes and a rock
    /// support with none already fails on `pore_ok` (`tests/round4.rs`,
    /// `a_paid_stonecushion_birth_on_a_rock_ledge_beside_a_soil_pocket`). Water is drawn
    /// only from those soil voxels, as it always was. The `rock_support` flag the round-4
    /// brief held in reserve — accept a rock face *provided* the root box holds soil — is
    /// therefore the behaviour the model already has, and adding a flag would only have
    /// been a way of switching it off for the other four.
    ///
    /// Every number is a **placeholder** (`design/backlog.md` §1):
    ///
    /// - **Water, the three thresholds together.** `establish_pore_min` 0.05 and
    ///   `wilt_pore` 0.02 are the lowest of the five by a factor of four: a pocket holding
    ///   a twentieth of its capacity is enough to start on and a fiftieth is still not
    ///   wilting. `sat_pore` 0.35 is also the lowest, because the point of the role is that
    ///   a little water is *enough* — the cushion is at full moisture on a pocket the other
    ///   four would call dry. `establish_saturated_max` 0.4 and `drown_depth_m` 0.02: a
    ///   rock face does not hold a pool, and a cushion under one is finished.
    /// - **Roots.** `rooting_depth` 2 and `rooting_radius` 1: the 3 × 2 × 3 box around and
    ///   under the rock face, which is where a crack with soil in it is.
    /// - **Light.** `light_half` 0.3 and `establish_light_min` 0.4: an exposed rock face has
    ///   open sky by definition, so the role needs little light *beyond* being in the open
    ///   — a low gate, and a response efficient enough that the open sky it has is plenty.
    /// - **Slow.** `wood_rate` 0.0002 and `foliage_rate` 0.0005 are a fifth and a quarter of
    ///   the base, so a newborn needs about 11,500 s to fill its `wood_max` 0.1;
    ///   `maintenance` 0.00005 is a quarter of the base, which is what lets something that
    ///   slow stay solvent; `propagule_rate` 0.00005 is a quarter of the base, and its
    ///   package is half bloomcrown's — `alive_min` 0.01 over `propagule_split[0]` 0.4 is
    ///   0.025 against 0.05 — so one package takes **600 s** of a fully funded donor's
    ///   saving, `0.025 / (0.00005 / 1.2)`: *twice* bloomcrown's already long 300 s and
    ///   not four times, because the smaller package cancels half of the slower rate
    ///   (Astra R7.6). `alive_min` 0.01 against `wood_max` 0.1 and `donor_min` 0.05.
    /// - **Shape.** `hop` 1; the crown is `[0.125, 0.1875]` m tall — a cushion has no stem
    ///   at any size — and `[0.125, 0.15625]` m in radius (package L's ladder).
    pub fn stonecushion() -> SpeciesConfig {
        SpeciesConfig {
            light_half: 0.3,
            rooting_depth: 2,
            rooting_radius: 1,
            wilt_pore: 0.02,
            sat_pore: 0.35,
            establish_pore_min: 0.05,
            establish_light_min: 0.4,
            stress_rate_per_s: 0.1,
            relax_rate_per_s: 0.02,
            establish_saturated_max: 0.4,
            drown_depth_m: 0.02,
            hop: 1,
            wood_max: 0.1,
            alive_min: 0.01,
            donor_min: 0.05,
            maintenance: 0.00005,
            senescence: 0.0003,
            foliage_rate: 0.0005,
            wood_rate: 0.0002,
            propagule_rate: 0.00005,
            crown_height_m: [0.125, 0.1875],
            crown_radius_m: [0.125, 0.15625],
            // Anatomy document §3: a dome in two foliage bands, no trunk. Decisions
            // §3 keeps it on the browser's menu, so there is no diet flag here.
            profile: SpeciesConfig::one_stage(vec![
                SpeciesConfig::foliage_layer([0.0, 0.6], 1.0, 0.7, 0.2),
                SpeciesConfig::foliage_layer([0.6, 1.0], 0.6, 0.3, 0.2),
            ]),
            ..SpeciesConfig::v1_base()
        }
    }

    /// **Velvetpad — the moist, aerated understory pad.** This is the niche Astra's R4.6
    /// separated from umbrellafrond's wetland: damp soil that still has air in it, under
    /// somebody else's crown. Umbrellafrond is saturation-immune and wants a hollow;
    /// velvetpad wants the floor beside it and pays for a wholly saturated root box.
    ///
    /// Every number is a **placeholder** (`design/backlog.md` §1):
    ///
    /// - **Water, the three thresholds together.** `establish_pore_min` 0.3, `wilt_pore`
    ///   0.2, `sat_pore` 0.6: damp, and consistently so — it germinates a little above
    ///   drained soil's own 0.25, wilts just below it, and is at full moisture at 0.6,
    ///   which is *below* umbrellafrond's own 0.8 (Astra R7.6). The difference from the
    ///   wetland role is
    ///   entirely the ceiling: `establish_saturated_max` 0.6 against umbrellafrond's 1.0,
    ///   so a wholly saturated box refuses a velvetpad cohort and targets an adult's
    ///   `aeration_stress` at 1 — waterlogging costs this species something, which is the
    ///   whole of what "aerated" means here. `stress_rate_per_s` 0.1 and
    ///   `relax_rate_per_s` 0.05: it closes on that stress twice as fast as it lets go.
    ///   `drown_depth_m` 0.1 is more than three times springturf's 0.03 — a forest floor
    ///   takes a puddle — and a fifth of umbrellafrond's 0.5.
    /// - **Light.** `light_half` 0.1 is the smallest of the five, so at a fifth of open
    ///   sky velvetpad's light response `L (1 + h) / (L + h)` is **0.7333** where
    ///   bloomcrown's is **0.36** (Astra R7.6 — the earlier 83 %/49 % was wrong);
    ///   `establish_light_min` 0.05 lets a cohort start almost anywhere the terrain does
    ///   not roof over. `senescence` 0.0005, half the base, is the other half of being a
    ///   shade plant: long-lived leaves. The break-even it buys, **under stated
    ///   conditions** — full foliage (`P = α · W`), site mineral 1 so the Monod factor is
    ///   2/3, no aeration stress and no growth — is where income equals maintenance plus
    ///   the construction cost of replacing senesced foliage:
    ///   `L_eff · μ = (maintenance + (1 + c_g) · senescence · α) / (assimilation · α ·
    ///   monod)`. At the base senescence 0.001 that is
    ///   `(0.0002 + 1.2·0.001·2) / (0.004·2·(2/3)) = 0.0026 / 0.0053333` = **0.4875**,
    ///   which is not shade at all; at 0.0005 it is `0.0014 / 0.0053333` = **0.2625**.
    ///   At 4 % of open sky `L_eff` is 0.3143, so 4 % clears 0.2625 only with `μ` at or
    ///   above 0.835 — sufficient moisture, not any moisture.
    /// - **Size and rates.** `wood_max` 0.2, `alive_min` 0.015, `donor_min` 0.1 — a third
    ///   of bloomcrown's body; `wood_rate` 0.002 and `foliage_rate` 0.004 are twice the
    ///   base and `propagule_rate` 0.0005 is two and a half times, a pad that fills in
    ///   quickly once it is under cover.
    /// - **Shape.** `hop` 1; the crown is 0.125 m tall at every size and `[0.1875, 0.375]` m
    ///   in radius (package L's ladder) — low and broad, a mat on the floor rather than a
    ///   stem, and the widest ground cover relative to its height.
    pub fn velvetpad() -> SpeciesConfig {
        SpeciesConfig {
            light_half: 0.1,
            rooting_depth: 2,
            rooting_radius: 1,
            wilt_pore: 0.2,
            sat_pore: 0.6,
            establish_pore_min: 0.3,
            establish_light_min: 0.05,
            stress_rate_per_s: 0.1,
            relax_rate_per_s: 0.05,
            establish_saturated_max: 0.6,
            drown_depth_m: 0.1,
            hop: 1,
            wood_max: 0.2,
            alive_min: 0.015,
            donor_min: 0.1,
            senescence: 0.0005,
            foliage_rate: 0.004,
            wood_rate: 0.002,
            propagule_rate: 0.0005,
            crown_height_m: [0.125, 0.125],
            crown_radius_m: [0.1875, 0.375],
            // Anatomy document §3: a sheet, no trunk. Decisions §3 keeps it on the
            // browser's menu (removing it would have cut 58 % of what the D3 browser
            // ate), so there is no diet flag here either.
            profile: SpeciesConfig::one_stage(vec![SpeciesConfig::mat_layer(
                [0.0, 1.0],
                1.0,
                1.0,
                0.6,
            )]),
            ..SpeciesConfig::v1_base()
        }
    }

    /// **Glowcap — the wood fungus of the decomposer grove.** The biosphere's §5 branch 2
    /// and its §6 substrate request: "non-photosynthetic stand metabolism: reuse stand
    /// location/lifecycle structure, replace income and substrate rules; no light income".
    /// A mycelium in a log, a cap on top of it, and no leaves anywhere: `wood` is mycelium,
    /// `foliage` is fruiting caps, and the reserve is the reserve.
    ///
    /// The role as the numbers state it: it lives on the dead wood in its own box and
    /// nothing else, it earns nothing from light and needs none to start, it wants a log
    /// that is damp but not drowned, it is small and cheap, and it spreads one hop — along
    /// the log, not across the world.
    ///
    /// **Every number is an untuned placeholder** (`design/backlog.md` §1), chosen to
    /// encode that sentence and nothing else:
    ///
    /// - **Income.** `substrate_uptake_per_s` 0.02 per unit of mycelium per second, against
    ///   the **dead wood and the litter** of its box together, and `substrate_yield` 0.4:
    ///   a fungus at full moisture earns `0.4 · 0.02 · W = 0.008 · W`
    ///   of tissue per second against maintenance plus cap replacement
    ///   `0.0002 · W + 1.2 · 0.001 · 2 · W = 0.0026 · W`, a **3.08× margin** — solvent on a
    ///   log with wood in it, and starving the moment the log or the moisture runs out,
    ///   which is the whole of what the role claims. The yield is the one number with a
    ///   literature shape to it (a microbial growth yield is a fraction, not a fifth and not
    ///   nine tenths) and it is still a placeholder: nothing here measured it.
    ///   `assimilation` is **0.0**, so the species earns nothing from light even if some
    ///   future caller reaches the `Photo` branch with it.
    /// - **The substrate gate.** `establish_substrate_min` 0.02 is about one spore package's
    ///   worth of wood (`alive_min / w_frac` = 0.025): a box has to hold roughly what the
    ///   stand it would feed is made of, counting its litter as well as its wood. Below it
    ///   there is nothing there to eat and the gate shuts, which is how a decomposer grove
    ///   *ends* — on a floor with litter on it, later than it used to.
    /// - **Water, the three thresholds together.** `establish_pore_min` 0.1 and
    ///   `establish_saturated_max` 0.5 are the brief's "pore between the species' floor and
    ///   its saturation ceiling", read in the model's own terms — the existing pore gate is
    ///   the floor and the existing aeration gate is the ceiling, and **no new rule was
    ///   added**. `wilt_pore` 0.1 and `sat_pore` 0.4: full uptake on ordinary drained soil,
    ///   nothing at all on a dry one, because `μ` multiplies uptake exactly as it multiplies
    ///   assimilation. `drown_depth_m` 0.05 — a cap under a pool is finished.
    ///
    ///   **A stated limitation, and the one this preset is most likely to be wrong about.**
    ///   The ceiling is a *germination* ceiling: a spore will not take a waterlogged log,
    ///   while the mycelium already in one pays nothing, because a saprotroph's uptake reads
    ///   `μ` and not `1 − aeration_stress` (the brief names only `μ`, and `step`'s `feed`
    ///   says so). That is exactly the germination-versus-adult-tolerance assumption
    ///   `establish_saturated_max`'s own doc records for the plants (Astra R4.6), pointing
    ///   the other way for this species. Its `stress_rate_per_s` 0.1 and `relax_rate_per_s`
    ///   0.05 are therefore **inert on the income** and kept only because the stress is
    ///   still tracked and drawn.
    /// - **The box.** `substrate_reach_up_down` 1 and `rooting_radius` 1: the mycelium box
    ///   is the twenty-seven support faces within one row of its own — its own level, the
    ///   row above and the row below — so a log one voxel up or down a step is in reach of
    ///   the mycelium and a log two rows away is not (Astra R9.3; before it the box was the
    ///   nine faces of its own row alone, and a grove could not cross a one-voxel step in
    ///   either direction). Water is read off
    ///   the *soil voxels* of the `rooting_depth` root box, as every stand's is, which is why
    ///   a glowcap on bare rock has `μ = 0` and starves however much wood is on the rock:
    ///   one water read for every stand is the model's rule and a fungus is not exempted
    ///   from it here. **Bare rock means no soil voxel anywhere in that box** — a rock face
    ///   with a soil pocket inside the box reads that pocket's water and is not bare.
    ///   `transpiration_m3_per_s` **0.0** — it reads the moisture and withdraws nothing, so
    ///   a fungus takes no water away from the plants it lives among.
    /// - **Small and cheap.** `wood_max` 0.1, `alive_min` 0.01, `donor_min` 0.05 —
    ///   stonecushion's body; `foliage_rate` 0.004 and `wood_rate` 0.002 are twice the base,
    ///   a mycelium that fills a log fairly quickly; `propagule_rate` 0.0005 is two and a
    ///   half times the base, so one 0.025 package is **60 s** of a fully funded donor's
    ///   saving — the fastest of the six, because a fruiting body's whole job is spores.
    /// - **Spread.** `hop` 1: the eight faces around it. A grove follows its log.
    /// - **Shape.** `crown_height_m` `[0.125, 0.25]` and `crown_radius_m`
    ///   `[0.0625, 0.0625]` (package L's ladder): a cap one or two small cells tall on the
    ///   face above its support, a single column wide. That is
    ///   the interim glyph and not a design — the art direction of the voxel world is its
    ///   own thread, and `crates/cubarium/src/voxel/stand.rs` names the palette interim too.
    ///   One consequence in the model: a crown top of `y + 0.5` is the lowest of the six, so
    ///   a glowcap shades nothing at all, and everything shades it — which costs it nothing,
    ///   because it does not eat light.
    pub fn glowcap() -> SpeciesConfig {
        SpeciesConfig {
            trophic: Trophic::Saprotroph,
            substrate_uptake_per_s: 0.02,
            substrate_yield: 0.4,
            establish_substrate_min: 0.02,
            // No light income at all, and no light gate (`step`'s `gates` opens `light_ok`
            // for a saprotroph whatever this says; 0.0 keeps the reported number honest).
            assimilation: 0.0,
            establish_light_min: 0.0,
            rooting_depth: 1,
            rooting_radius: 1,
            wilt_pore: 0.1,
            sat_pore: 0.4,
            establish_pore_min: 0.1,
            establish_saturated_max: 0.5,
            stress_rate_per_s: 0.1,
            relax_rate_per_s: 0.05,
            drown_depth_m: 0.05,
            transpiration_m3_per_s: 0.0,
            hop: 1,
            wood_max: 0.1,
            alive_min: 0.01,
            donor_min: 0.05,
            foliage_rate: 0.004,
            wood_rate: 0.002,
            propagule_rate: 0.0005,
            crown_height_m: [0.125, 0.25],
            crown_radius_m: [0.0625, 0.0625],
            // Anatomy document §3: the fruiting body on the face. `foliage` is cap
            // tissue, which decisions §3 makes shredder food and not browser food; the
            // diet lives on the consumer, not here.
            profile: SpeciesConfig::one_stage(vec![SpeciesConfig::foliage_layer(
                [0.0, 1.0],
                1.0,
                1.0,
                0.5,
            )]),
            ..SpeciesConfig::v1_base()
        }
    }

    /// Whether this preset can produce a living stand at all, checked where the numbers
    /// enter the system rather than where they first go wrong (Astra R6.2 asks for exactly
    /// this list: a finite nonnegative split summing to one, a positive wood fraction,
    /// `alive_min <= wood_max`, and finite rates).
    ///
    /// `name` is only for the message. The checks are the ones a *silent* failure would
    /// otherwise produce: a split that does not sum to one moves organic matter into or out
    /// of the system at every germination; a zero `w_frac` makes
    /// [`crate::SpeciesConfig::propagule_rate`]'s package size infinite, so the species can
    /// never germinate and never says why; `alive_min > wood_max` makes every newborn die
    /// on its first growth tick; a non-finite rate turns one stand's stocks into `NaN` and
    /// then the whole ledger.
    ///
    /// It is deliberately **not** a plausibility check. Nothing here says a rate is a good
    /// one, and nothing here refuses a preset that starves: `design/backlog.md` §1 lists
    /// the values that are known to be wrong, and a validator that refused them would be a
    /// tuning rule wearing a validator's clothes.
    pub fn validate(&self, name: &str) -> Result<(), String> {
        let fail = |what: &str| Err(format!("{name}: {what}"));

        // The anatomy, before anything else reads it: a stand's layer stocks are binned
        // against this the moment it is created, so a malformed profile is a world that
        // cannot conserve its own foliage rather than a number that looks odd later.
        if self.profile.is_empty() {
            return fail("profile is empty: every species states its anatomy");
        }
        let mut previous = 0.0f64;
        for (i, stage) in self.profile.iter().enumerate() {
            if !(stage.wood_fraction_max >= previous) {
                return fail(&format!(
                    "profile stage {i} threshold {} is below stage {}'s {previous}: the stages                      are read in order",
                    stage.wood_fraction_max,
                    i.saturating_sub(1)
                ));
            }
            previous = stage.wood_fraction_max;
            if stage.layers.is_empty() {
                return fail(&format!("profile stage {i} has no layers"));
            }
            if stage.foliage_layer_count() > MAX_FOLIAGE_LAYERS {
                return fail(&format!(
                    "profile stage {i} has {} foliage layers, over the {MAX_FOLIAGE_LAYERS} a                      stand can hold stocks for",
                    stage.foliage_layer_count()
                ));
            }
            let mut share = 0.0;
            for (j, layer) in stage.layers.iter().enumerate() {
                let [lo, hi] = layer.band;
                if !(lo.is_finite() && hi.is_finite() && lo >= 0.0 && hi <= 1.0 && hi > lo) {
                    return fail(&format!(
                        "profile stage {i} layer {j} band {:?} is not a rising interval inside                          [0, 1] of the crown height",
                        layer.band
                    ));
                }
                if !(layer.radius.is_finite() && layer.radius > 0.0 && layer.radius <= 1.0) {
                    return fail(&format!(
                        "profile stage {i} layer {j} radius is {}, not a fraction of the crown                          radius in (0, 1]",
                        layer.radius
                    ));
                }
                if !(layer.porosity.is_finite() && (0.0..=1.0).contains(&layer.porosity)) {
                    return fail(&format!(
                        "profile stage {i} layer {j} porosity is {}, not in [0, 1]",
                        layer.porosity
                    ));
                }
                if layer.kind.bears_foliage() {
                    if !(layer.share.is_finite() && layer.share >= 0.0) {
                        return fail(&format!(
                            "profile stage {i} layer {j} share is {}, not finite and \
                             nonnegative",
                            layer.share
                        ));
                    }
                    share += layer.share;
                } else if layer.share != 0.0 {
                    return fail(&format!(
                        "profile stage {i} layer {j} is a trunk holding a share of {}: structure                          is not foliage",
                        layer.share
                    ));
                }
            }
            if (share - 1.0).abs() > 1e-12 {
                return fail(&format!(
                    "profile stage {i} foliage shares sum to {share}, not one: a stand's layers                      would not sum to its foliage"
                ));
            }
            if let Some(cap) = stage.height_m_max {
                if !(cap.is_finite() && cap > 0.0) {
                    return fail(&format!(
                        "profile stage {i} height_m_max is {cap}, not finite and positive"
                    ));
                }
            }
        }
        if !(self
            .profile
            .last()
            .expect("a non-empty profile")
            .wood_fraction_max
            >= 1.0)
        {
            return fail("the last profile stage must catch a full-grown stand (threshold >= 1)");
        }

        let [w_frac, p_frac, q_frac] = self.propagule_split;
        for (label, v) in [("w_frac", w_frac), ("p_frac", p_frac), ("q_frac", q_frac)] {
            if !v.is_finite() || v < 0.0 {
                return fail(&format!(
                    "propagule_split {label} is {v}, not finite and nonnegative"
                ));
            }
        }
        let sum = w_frac + p_frac + q_frac;
        if (sum - 1.0).abs() > 1e-12 {
            return fail(&format!(
                "propagule_split {:?} sums to {sum}, not one — a germination would create or \
                 destroy organic matter",
                self.propagule_split
            ));
        }
        if !(w_frac > 0.0) {
            return fail(&format!(
                "propagule_split w_frac is {w_frac}: a species that puts nothing into wood has \
                 no package size and can never germinate"
            ));
        }
        if !(self.wood_max > 0.0 && self.wood_max.is_finite()) {
            return fail(&format!(
                "wood_max is {}, not finite and positive",
                self.wood_max
            ));
        }
        if !(self.alive_min.is_finite() && self.alive_min >= 0.0) {
            return fail(&format!(
                "alive_min is {}, not finite and nonnegative",
                self.alive_min
            ));
        }
        if self.alive_min > self.wood_max {
            return fail(&format!(
                "alive_min {} is over wood_max {}: every newborn would die on its first growth \
                 tick",
                self.alive_min, self.wood_max
            ));
        }

        // A yield over one would build more tissue than the substrate it came out of held,
        // which creates organic matter inside the system with no boundary flow to name it.
        // Checked as a bound and not as a rate because it is a fraction, and checked for
        // every species — the field is inert on a `Photo` preset, and a wrong value there
        // is still a wrong value waiting for a `trophic` switch.
        if !(self.substrate_yield.is_finite()
            && self.substrate_yield >= 0.0
            && self.substrate_yield <= 1.0)
        {
            return fail(&format!(
                "substrate_yield is {}, not a fraction in 0..=1 — a yield over one would \
                 build tissue out of nothing",
                self.substrate_yield
            ));
        }

        // Every scalar the tick multiplies a stock by. A `NaN` or an infinity in any of
        // them reaches the ledger within one step, and none of them has a meaning below
        // zero — a negative rate would run a flow backwards past its own `min` guard.
        let rates: [(&str, f64); 27] = [
            ("substrate_uptake_per_s", self.substrate_uptake_per_s),
            ("establish_substrate_min", self.establish_substrate_min),
            ("alpha", self.alpha),
            ("reserve_cap", self.reserve_cap),
            ("maintenance", self.maintenance),
            ("senescence", self.senescence),
            ("foliage_rate", self.foliage_rate),
            ("wood_rate", self.wood_rate),
            ("build", self.build),
            ("dieback", self.dieback),
            ("donor_min", self.donor_min),
            ("donor_reserve_floor", self.donor_reserve_floor),
            ("propagule_rate", self.propagule_rate),
            ("energy_density", self.energy_density),
            ("seed_attrition_per_s", self.seed_attrition_per_s),
            ("seed_max_age_s", self.seed_max_age_s),
            ("n_tissue", self.n_tissue),
            ("reserve_share", self.reserve_share),
            ("reflush_below", self.reflush_below),
            ("assimilation", self.assimilation),
            ("nutrient_half", self.nutrient_half),
            ("nutrient_draw_max", self.nutrient_draw_max),
            ("light_half", self.light_half),
            ("transpiration_m3_per_s", self.transpiration_m3_per_s),
            ("stress_rate_per_s", self.stress_rate_per_s),
            ("relax_rate_per_s", self.relax_rate_per_s),
            ("drown_depth_m", self.drown_depth_m),
        ];
        for (label, v) in rates {
            if !v.is_finite() || v < 0.0 {
                return fail(&format!("{label} is {v}, not finite and nonnegative"));
            }
        }
        // The water and aeration thresholds and the crown geometry: fractions and lengths,
        // read as bounds rather than multiplied by a stock, so only finiteness is checked.
        let bounds: [(&str, f64); 7] = [
            ("wilt_pore", self.wilt_pore),
            ("sat_pore", self.sat_pore),
            ("establish_pore_min", self.establish_pore_min),
            ("establish_light_min", self.establish_light_min),
            ("saturated_pore", self.saturated_pore),
            ("establish_saturated_max", self.establish_saturated_max),
            ("alive_min", self.alive_min),
        ];
        for (label, v) in bounds {
            if !v.is_finite() {
                return fail(&format!("{label} is {v}, not finite"));
            }
        }
        for (label, pair) in [
            ("crown_height_m", self.crown_height_m),
            ("crown_radius_m", self.crown_radius_m),
        ] {
            if !pair.iter().all(|v| v.is_finite() && *v >= 0.0) {
                return fail(&format!("{label} is {pair:?}, not finite and nonnegative"));
            }
        }
        Ok(())
    }
}

impl Default for SpeciesConfig {
    fn default() -> SpeciesConfig {
        SpeciesConfig::v1_base()
    }
}

/// **When** a site's [`FloraConfig::initial_mineral`] is provisioned: lazily, the first
/// time anything lands on the site, or eagerly on **every support face of the world** when
/// the layer is built.
///
/// The two are the same model. Nothing in `step` reads this: a provisioned site is a site
/// with a [`Ground`] on it holding `initial_mineral`, which is exactly what a first landing
/// creates, and both bookings are the same `seeded_mineral_in` inflow. What changes is
/// **when** the inflow happens, and therefore whether [`FloraLedger::expected_mineral`]
/// moves while the plants are still spreading.
///
/// [`Provision::Lazy`] is the default, so nothing that does not ask for the other one
/// changes at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Provision {
    /// **Provision previously unrepresented ground** (Astra R4.3): a site gets its
    /// `initial_mineral` the first time it holds anything — a founder, a landed package, a
    /// deposit — and that import is booked as `seeded_mineral_in` then. The mineral
    /// inventory therefore **grows with the number of sites the plants have reached**, so
    /// two arms of one study that spread differently hold different totals, and a fertility
    /// comparison between them is confounded by its own colonisation.
    #[default]
    Lazy,
    /// **A fixed per-site inventory, laid down once at world creation** (Astra R5.4): every
    /// support face in the world gets a [`Ground`] holding `initial_mineral` when
    /// [`Flora::in_world`] builds the layer, and the whole of it is booked as
    /// `seeded_mineral_in` in that one call. `expected_mineral` is then a constant of the
    /// world for the rest of the run — no later landing can import any — so every arm
    /// branched from one conditioned state has the **same** mineral inventory however far
    /// its own plants spread. That is matched fertility, and it is what a
    /// resource-competition study needs; lazy colonisation imports are not it.
    ///
    /// It is **not** fertilisation: the per-site amount is unchanged, and a site that is
    /// never reached simply holds what it was given and does nothing with it. The costs are
    /// stated rather than hidden: the layer carries one [`Ground`] per support face from
    /// tick zero, so the tick-start snapshot and the decomposition pass walk every face of
    /// the world instead of the reached ones, and `seeded_mineral_in` is large from the
    /// start.
    AtCreation,
}

/// Everything the plant layer runs from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FloraConfig {
    pub bloomcrown: SpeciesConfig,
    pub umbrellafrond: SpeciesConfig,
    pub springturf: SpeciesConfig,
    pub stonecushion: SpeciesConfig,
    pub velvetpad: SpeciesConfig,
    pub glowcap: SpeciesConfig,
    /// Canopy attenuation, **per square metre**: a taller stand whose crown covers a
    /// site multiplies the light reaching that site by
    /// `exp(-shade_k_per_m2 · P / crown_area_m2)`, where the crown's area is
    /// `π · (radius_cells · voxel_m)²` floored at one reference cell, `(0.25 m)²`.
    ///
    /// **It used to be per cell²** (`shade_k`, 1.5), which made optical depth a
    /// function of the grid: the same plant with the same foliage shaded four times
    /// less when the cell halved (the audit's §1 "shade-area units bug";
    /// `design/handoffs/voxel-body-anchors-2026-09-22.md`). The shipped value is the
    /// old one converted, `1.5 × (0.25 m)² = 0.09375`, so the 0.25 m reference world is
    /// numerically identical — including the floor, which is one reference cell either
    /// way — and only finer grids move, to the physically right value. A config written
    /// against the old field name is refused (`deny_unknown_fields`) rather than
    /// reinterpreted 16× too dark.
    pub shade_k_per_m2: f64,
    /// `k_d`: litter decomposition per second. Its organic matter is respired out of the
    /// system (`respired_out`, energy to heat) and its mineral is released to the site's
    /// pool at the same fraction.
    pub decomposition: f64,
    /// `k_w`: dead-wood decomposition per second, the same two flows, slow.
    pub wood_decomposition: f64,
    /// `k_c`: **carrion** decomposition per second ([`Ground::carrion`]), the same two
    /// flows. **A placeholder, and nothing has measured it**
    /// (`design/backlog.md` §1): 0.005 /s is five times `decomposition`, which is the only
    /// thing it claims — a corpse goes faster than a leaf, an e-folding in 200 s against
    /// litter's 1,000 s and dead wood's 10,000 s. It is not tuned and no result in this
    /// round depends on its value.
    pub carrion_decomposition: f64,
    /// `e_d_max`: retained energy cap per unit of litter.
    pub litter_energy_cap: f64,
    /// Mineral a site starts with the first time anything lands on it. Booked as
    /// `seeded_mineral_in`: it comes from outside the closed system, so it is a named
    /// inflow and not a residual.
    ///
    /// It is applied **lazily**, when a site first gets a `Ground` — a founder, a landed
    /// package, litter — so what it represents is *provisioning previously unrepresented
    /// ground*, not fertilizing it (Astra R4.3). Colonization therefore imports mineral
    /// into the world, explicitly booked, and the mineral inventory is not closed while
    /// first landings are still happening: `expected_mineral` grows with the number of
    /// sites the plants have reached. A fertility comparison wants a **fixed per-site
    /// inventory laid down at world creation** instead, so that the total is the same in
    /// every arm however far the plants spread: that is [`Provision::AtCreation`], asked
    /// for through [`FloraConfig::provision`] and applied by [`Flora::in_world`].
    pub initial_mineral: f64,
    /// Whether `initial_mineral` is provisioned lazily on a site's first landing or
    /// eagerly on every support face at creation. [`Provision::Lazy`] by default, which is
    /// the only rule that existed before the replacement study and changes nothing.
    pub provision: Provision,
    /// The world's cell size in metres, recorded so that geometry authored in **metres** can
    /// be applied to a grid stored in **voxels**: every crown is in metres since package
    /// L, and the voxel readings ([`SpeciesConfig::crown_height`]) divide by this. Set by
    /// [`FloraConfig::for_voxel_size`]; the reference 0.25 m by default.
    pub voxel_m: f64,
}

impl Default for FloraConfig {
    fn default() -> FloraConfig {
        FloraConfig {
            bloomcrown: SpeciesConfig::bloomcrown(),
            umbrellafrond: SpeciesConfig::umbrellafrond(),
            springturf: SpeciesConfig::springturf(),
            stonecushion: SpeciesConfig::stonecushion(),
            velvetpad: SpeciesConfig::velvetpad(),
            glowcap: SpeciesConfig::glowcap(),
            shade_k_per_m2: 0.09375,
            decomposition: 0.001,
            wood_decomposition: 0.0001,
            carrion_decomposition: 0.005,
            litter_energy_cap: 2.0,
            initial_mineral: 1.0,
            provision: Provision::Lazy,
            voxel_m: 0.25,
        }
    }
}

impl FloraConfig {
    /// The authored default flora, with spatial geometry expressed for `voxel_m`.
    ///
    /// The presets were authored on 0.25 m voxels. Finer grids multiply every spatial
    /// distance stored in voxel units so roots, mycelium access and propagule travel
    /// retain their approximate physical size. Crowns are stated in metres and are not
    /// touched. The ecological rates and stores
    /// are copied unchanged. Reference-sized and coarser grids deliberately retain the
    /// historical defaults, as do invalid sizes (which the world config rejects at its
    /// own input boundary).
    pub fn for_voxel_size(voxel_m: f64) -> FloraConfig {
        const REFERENCE_VOXEL_M: f64 = 0.25;

        let mut config = FloraConfig::default();
        if !voxel_m.is_finite() || !(voxel_m > 0.0) {
            return config;
        }
        // The cell size is recorded whatever it is, because a metre rule has to be
        // converted on a coarse grid too; only the authored *voxel* geometry is left
        // alone at or above the reference size.
        config.voxel_m = voxel_m;
        if voxel_m >= REFERENCE_VOXEL_M {
            return config;
        }
        let scale = REFERENCE_VOXEL_M / voxel_m;
        for species in Species::ALL {
            let sc = config.species_mut(species);
            sc.rooting_depth = scaled_voxel_distance(sc.rooting_depth, scale);
            sc.rooting_radius = scaled_voxel_distance(sc.rooting_radius, scale);
            sc.substrate_reach_up_down = scaled_voxel_distance(sc.substrate_reach_up_down, scale);
            sc.hop = scaled_voxel_distance(sc.hop, scale);
            // The crown is in metres (package L) and needs no rescale: it is the same
            // physical plant on every grid.
        }
        config
    }

    /// Every species reduced to **one** `[0, 1.0]` foliage layer at the full crown
    /// radius with no porosity: the lollipop the model was before plants had layers.
    ///
    /// For a **fixture whose subject is not the anatomy** — the shade exponent's units,
    /// the strictness of the occlusion inequality, the mouth's physical band — where a
    /// tiered, porous, rosette-bearing plant would be measuring something else. It is
    /// not a shipped configuration and nothing in the tick uses it; what the authored
    /// profiles do is `cubarium-voxel-flora/tests/layers.rs`'s and
    /// `cubarium-voxel-fauna/tests/plant_layers.rs`'s subject.
    pub fn one_layer_species(mut self) -> FloraConfig {
        for species in Species::ALL {
            self.species_mut(species).profile =
                SpeciesConfig::one_stage(vec![SpeciesConfig::foliage_layer(
                    [0.0, 1.0],
                    1.0,
                    1.0,
                    0.0,
                )]);
        }
        self
    }

    /// Every crown — both ranges and each stage's seedling ceiling — multiplied by `k`.
    ///
    /// For a **fixture on a coarse grid** whose subject is a rule and not a plant's
    /// size: the 1 m-cell fixtures were written when crowns were stated in 0.25 m
    /// reference cells and read as cells, so a 1 m fixture's crowns were four times the
    /// ladder. `crowns_scaled(4.0)` states that geometry in metres now crowns are metres
    /// (package L). Not a shipped configuration; nothing in the tick uses it.
    pub fn crowns_scaled(mut self, k: f64) -> FloraConfig {
        for species in Species::ALL {
            let sc = self.species_mut(species);
            sc.crown_height_m = sc.crown_height_m.map(|v| v * k);
            sc.crown_radius_m = sc.crown_radius_m.map(|v| v * k);
            for stage in &mut sc.profile {
                if let Some(cap) = &mut stage.height_m_max {
                    *cap *= k;
                }
            }
        }
        self
    }

    pub fn species(&self, s: Species) -> &SpeciesConfig {
        match s {
            Species::Bloomcrown => &self.bloomcrown,
            Species::Umbrellafrond => &self.umbrellafrond,
            Species::Springturf => &self.springturf,
            Species::Stonecushion => &self.stonecushion,
            Species::Velvetpad => &self.velvetpad,
            Species::Glowcap => &self.glowcap,
        }
    }

    /// The same table, mutably: a harness or a fixture that has a [`Species`] in hand and
    /// wants to change that species' preset, without a sixth copy of the match.
    pub fn species_mut(&mut self, s: Species) -> &mut SpeciesConfig {
        match s {
            Species::Bloomcrown => &mut self.bloomcrown,
            Species::Umbrellafrond => &mut self.umbrellafrond,
            Species::Springturf => &mut self.springturf,
            Species::Stonecushion => &mut self.stonecushion,
            Species::Velvetpad => &mut self.velvetpad,
            Species::Glowcap => &mut self.glowcap,
        }
    }

    /// Every preset through [`SpeciesConfig::validate`], plus the shared rates, with the
    /// species named in the message. [`Flora::new`] calls this, so a config that cannot
    /// produce a living stand is refused where it enters rather than as a `NaN` in a ledger
    /// ten thousand ticks later.
    pub fn validate(&self) -> Result<(), String> {
        for species in Species::ALL {
            self.species(species).validate(species.name())?;
        }
        for (label, v) in [
            ("shade_k_per_m2", self.shade_k_per_m2),
            ("decomposition", self.decomposition),
            ("wood_decomposition", self.wood_decomposition),
            ("carrion_decomposition", self.carrion_decomposition),
            ("litter_energy_cap", self.litter_energy_cap),
            ("initial_mineral", self.initial_mineral),
        ] {
            if !v.is_finite() || v < 0.0 {
                return Err(format!("flora: {label} is {v}, not finite and nonnegative"));
            }
        }
        Ok(())
    }
}

fn scaled_voxel_distance(distance: u32, scale: f64) -> u32 {
    if distance == 0 {
        return 0;
    }
    (f64::from(distance) * scale)
        .round()
        .clamp(1.0, f64::from(u32::MAX)) as u32
}

#[cfg(test)]
mod voxel_scale_tests {
    use super::*;

    #[test]
    fn half_size_voxels_double_only_the_authored_spatial_geometry() {
        let reference = FloraConfig::default();
        let scaled = FloraConfig::for_voxel_size(0.125);

        for species in Species::ALL {
            let before = reference.species(species);
            let after = scaled.species(species);
            assert_eq!(after.rooting_depth, before.rooting_depth * 2);
            assert_eq!(after.rooting_radius, before.rooting_radius * 2);
            assert_eq!(
                after.substrate_reach_up_down,
                before.substrate_reach_up_down * 2
            );
            assert_eq!(after.hop, before.hop * 2);
            // Crowns are metres since package L: the same on every grid.
            assert_eq!(after.crown_height_m, before.crown_height_m);
            assert_eq!(after.crown_radius_m, before.crown_radius_m);
        }

        let mut geometry_reset = scaled;
        for species in Species::ALL {
            let before = reference.species(species);
            let after = geometry_reset.species_mut(species);
            after.rooting_depth = before.rooting_depth;
            after.rooting_radius = before.rooting_radius;
            after.substrate_reach_up_down = before.substrate_reach_up_down;
            after.hop = before.hop;
        }
        // The recorded cell size is not authored geometry: it is the world's, and it
        // is what a rule written in metres is converted with
        // ([`SpeciesConfig::crown_height`]).
        assert_eq!(geometry_reset.voxel_m, 0.125);
        geometry_reset.voxel_m = reference.voxel_m;
        assert_eq!(geometry_reset, reference, "rates and stores stay authored");
    }

    #[test]
    fn reference_and_coarser_voxels_keep_the_historical_defaults() {
        let reference = FloraConfig::default();
        assert_eq!(FloraConfig::for_voxel_size(0.25), reference);
        let mut coarse = FloraConfig::for_voxel_size(1.0);
        // Everything but the recorded cell size, which is always the world's own: a
        // 1 m world still has to convert decisions §5's 0.125 m seedling ceiling.
        assert_eq!(coarse.voxel_m, 1.0);
        coarse.voxel_m = reference.voxel_m;
        assert_eq!(coarse, reference);
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
    /// Organic matter, mineral and energy a `Seed` command — or a site's
    /// `initial_mineral`, whether it was provisioned on its first landing or with every
    /// other support face at creation ([`Provision`]) — created.
    pub seeded_organic_in: f64,
    pub seeded_mineral_in: f64,
    pub seeded_energy_in: f64,
    /// Organic matter, mineral and energy removed because a terrain edit buried or
    /// removed the support of a site, or a `Clear` command removed a stand, or a
    /// [`Command::ClearBank`] removed one species' seed bank. Reported, never hidden.
    pub removed_organic_out: f64,
    pub removed_mineral_out: f64,
    pub removed_energy_out: f64,
    /// Organic matter, mineral and energy a consumer **took** — [`Flora::take_foliage`],
    /// [`Flora::take_dead_wood`], [`Flora::take_litter`] — cumulative over every
    /// withdrawal. A named boundary flow exactly like `removed_*_out`: this layer holds no
    /// animal, so material a consumer eats leaves the plant layer's accounting the moment
    /// it is handed over, and the consumer layer that received the [`Taken`] owes the same
    /// three numbers on its own books.
    ///
    /// The three are the sum of the [`Taken`]s the withdrawals returned, to the bit: a
    /// caller that books what it was given cannot disagree with this.
    pub consumed_organic_out: f64,
    pub consumed_mineral_out: f64,
    pub consumed_energy_out: f64,
    /// The same boundary, **entering**: what [`Flora::deposit`] put back as carrion or as
    /// litter — a consumer's corpse, or its droppings. Not `seeded_*`, which is material
    /// this layer created out of nothing for a founder or for a new site's pool: a deposit
    /// is material a consumer is handing over, and the consumer's own books are where it
    /// came from. A consumer layer's outflow and this inflow are the same transfer seen
    /// from the two sides, so the two ledgers close together.
    ///
    /// A site's lazy `initial_mineral` is **not** in here even when a deposit is what first
    /// provisioned the site: that is `seeded_mineral_in`, as it is for a founder and a
    /// landed package.
    pub deposited_organic_in: f64,
    pub deposited_mineral_in: f64,
    pub deposited_energy_in: f64,
    /// Water withdrawn through `Command::WithdrawPore`, cubic metres, for cross-checking
    /// against the core ledger's `transpiration_out`.
    pub transpired_m3: f64,
    pub establishments: u64,
    pub deaths: u64,
    /// Every stand this layer has created, founders and germinations alike: a counter, not
    /// a flux, and the source of [`Stand::id`]. `births − establishments` is the number of
    /// founders a run was given.
    pub births: u64,
    /// Reproductive flux per species, indexed by [`Species::index`], cumulative organic
    /// matter **net of construction** so that the three are one currency and comparable:
    ///
    /// - `propagule_requested`: what the rate asks for — `propagule_rate · dt / (1 + c_g)`
    ///   for every stand over `donor_min`, whether or not it can pay.
    /// - `propagule_funded`: what the reserves actually paid into parcels, above each
    ///   donor's own reserve floor. `funded / requested` is how much of the advertised
    ///   reproductive effort the world can afford.
    /// - `propagule_landed`: what left donors as whole packages and arrived in seed banks.
    ///
    /// Astra's R4.4 asked for exactly this split: "distinguish requested, funded and
    /// landed reproductive flux in the diagnosis", because raising a rate that is already
    /// not the binding constraint creates no income. `funded − landed` is the material
    /// standing in parcels, plus whatever parcels have gone to litter with their donors.
    pub propagule_requested: [f64; Species::COUNT],
    pub propagule_funded: [f64; Species::COUNT],
    pub propagule_landed: [f64; Species::COUNT],
    /// Organic matter the saprotrophs have withdrawn from dead wood and litter, per
    /// species, indexed by [`Species::index`]: a **diagnostic flux and not a boundary flow**, exactly like
    /// the three `propagule_*` arrays, and therefore in none of the three
    /// `expected_*` totals.
    ///
    /// The reason it cannot be a boundary term is what a saprotroph is. `consumed_*_out`
    /// means "a consumer outside this layer took this, and owes it on its own books"; a
    /// glowcap is a stand *inside* this layer, so its uptake moves organic matter from a
    /// ground stock to a stand and crosses nothing. Booking it out and back in again would
    /// keep the residuals and destroy the meaning of `consumed_organic_out` — a harvest
    /// study could no longer tell what an animal ate from what the fungi digested. What
    /// leaves the system out of that uptake is the part the fungus does not keep, and that
    /// is `respired_out` like every other respiration.
    pub substrate_uptake: [f64; Species::COUNT],
}

impl FloraLedger {
    pub fn expected_organic(&self) -> f64 {
        self.seeded_organic_in + self.fixed_in + self.deposited_organic_in
            - self.respired_out
            - self.removed_organic_out
            - self.consumed_organic_out
    }

    pub fn expected_mineral(&self) -> f64 {
        self.seeded_mineral_in + self.deposited_mineral_in
            - self.removed_mineral_out
            - self.consumed_mineral_out
    }

    pub fn expected_energy(&self) -> f64 {
        self.seeded_energy_in + self.light_in + self.deposited_energy_in
            - self.heat_out
            - self.removed_energy_out
            - self.consumed_energy_out
    }
}

/// What one withdrawal actually took out of the plant layer: the three currencies of the
/// ledger, in the same units [`FloraView::organic`], [`FloraView::mineral`] and
/// [`FloraView::energy`] report.
///
/// A withdrawal is **bounded by the stock it reads** — never more than the stand's foliage,
/// never the wood or the reserve, never more than a dead pool holds — so this is what a
/// consumer got and not what it asked for. Booking exactly these three numbers is what
/// keeps a consumer layer's own accounting and this one's `consumed_*` agreeing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Taken {
    /// Organic matter, material units.
    pub organic: f64,
    /// The mineral that was in it: the same **fraction** of the source's mineral stock as
    /// the organic matter was of its organic stock ([`Stand::mineral`]'s fraction rule for
    /// a stand, the stock's own current density for a dead pool).
    pub mineral: f64,
    /// The energy that was in it: the tissue's `energy_density` for foliage, and the dead
    /// pool's own retained energy pro rata for litter and dead wood.
    pub energy: f64,
}

/// What a **layer-bounded** withdrawal took: the ledger entry, and the organic matter
/// that left each of the stand's foliage layers, indexed bottom-up as
/// [`Stand::layer_stock`] is.
///
/// The per-layer breakdown is the point: a caller that books a bite against a scalar
/// cannot say whether the plant is now leaf-poor at the base or evenly thinned, and
/// those are different plants to the next mouth and to the light
/// (`design/7_Research/organism-systems-audit-2026-09-21.md` §5).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TakenFoliage {
    pub taken: Taken,
    pub per_layer: [f64; MAX_FOLIAGE_LAYERS],
}

/// A caller-owned cache of the **geometric** sky visibility of sites, for a study that
/// asks the establishment predicate over a skyline many times.
///
/// One hemisphere-ray calculation per site, shared across the six species and reused across
/// observations for as long as the terrain is unchanged. It caches
/// [`VoxelView::sky_visibility`] and nothing else: pore water, saturation, standing-water
/// depth and dead wood are read afresh by [`FloraView::establishment_gates_over`] on every
/// call, so an observation is still the instantaneous reading it has always been and no
/// gate outcome is carried across ticks.
///
/// **Owned by the study, never by the process and never by the model.** A study holds one
/// for as long as it holds the world it reads and drops it with that world, so its lifetime
/// and its memory are bounded to that study — no process-global accumulating cache. It
/// carries the world's [`VoxelView::terrain_version`], its grid dimensions **and** a
/// fingerprint of the material array, so neither a decoded, reset, cloned or matched-arm
/// world that shares a version number nor a differently-shaped world with the same bytes can
/// hand a reading for one floor plan to another.
#[derive(Clone, Debug, Default)]
pub struct SkyCache {
    /// Sorted by site: one hemisphere reading per site, terrain geometry only.
    entries: Vec<(Site, f64)>,
    /// The terrain the entries were read against.
    key: Option<TerrainKey>,
}

/// What a cached reading is valid for: the world's terrain revision, its shape, and a
/// fingerprint of its material. The shape is in the key because the flat index layout and
/// the ray march both read `Config`, so identical bytes under different dimensions are
/// different geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TerrainKey {
    version: u64,
    width: u32,
    height: u32,
    depth: u32,
    material: u64,
}

impl SkyCache {
    /// An empty cache. It fills lazily on the first site asked.
    pub fn new() -> SkyCache {
        SkyCache::default()
    }

    /// Drop every reading. A study that means to start a new world calls this; a study that
    /// keeps its own world does not need to.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.key = None;
    }

    /// How many sites are cached: the whole of this cache's memory, and nothing else.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Point the cache at `view`'s terrain, dropping every reading if this is not the
    /// terrain the cache last held. The fingerprint is over the material array and the key
    /// also names the shape, so two worlds that share a `terrain_version` are told apart
    /// whether their difference is a floor plan or a grid.
    fn sync(&mut self, view: &VoxelView<'_>) {
        let key = TerrainKey {
            version: view.terrain_version,
            width: view.config.width,
            height: view.config.height,
            depth: view.config.depth,
            material: terrain_fingerprint(view.material),
        };
        if self.key != Some(key) {
            self.entries.clear();
            self.key = Some(key);
        }
    }

    /// The sky visibility of `site`: from the cache when this terrain has it, and from the
    /// model's own hemisphere rays when it does not.
    fn visibility(&mut self, view: &VoxelView<'_>, site: Site) -> f64 {
        match self.entries.binary_search_by_key(&site, |e| e.0) {
            Ok(i) => self.entries[i].1,
            Err(i) => {
                let value = view.sky_visibility(site.x as i64, site.y, site.z);
                self.entries.insert(i, (site, value));
                value
            }
        }
    }
}

/// One force-of-habit hash over the terrain material — FNV-1a — used for nothing but a
/// cache validity check, where all that matters is that two floor plans differ.
fn terrain_fingerprint(material: &[Material]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &m in material {
        h ^= u64::from(m as u8);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
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

/// Every layer of `stand`, bottom-up, with its geometry resolved and its stock read.
///
/// The one place a layer's geometry is computed. `config` supplies the species'
/// anatomy; `voxel_m` is the **world's** cell size, which is what turns a radius in
/// cells into an area in m² and a band into metres. Nothing else about the world
/// enters: a layer is a property of the plant and not of the terrain under it.
///
/// In a live world `config.voxel_m` **is** `voxel_m` — [`FloraConfig::for_voxel_size`]
/// records the world's own size — and [`FloraView::layers`] passes it, because a view
/// has no world in hand. The tick passes the world's directly, so that a fixture whose
/// config and world disagree shades exactly as it did before layers existed.
pub fn layers_of(config: &FloraConfig, stand: &Stand, voxel_m: f64) -> Vec<StandLayer> {
    let sc = config.species(stand.species);
    let stage = sc.profile_at(stand.wood);
    let height_v = sc.crown_height(stand.wood, voxel_m);
    let radius_v = sc.crown_radius(stand.wood, voxel_m).max(0.0);
    let cap = sc.alpha * stand.wood.max(0.0);
    let base = f64::from(stand.site.y);
    let mut out = Vec::with_capacity(stage.layers.len());
    let mut foliage_index = 0usize;
    for (index, layer) in stage.layers.iter().enumerate() {
        let is_foliage = layer.kind.bears_foliage();
        let fi = if is_foliage {
            let i = foliage_index;
            foliage_index += 1;
            (i < MAX_FOLIAGE_LAYERS).then_some(i)
        } else {
            None
        };
        if is_foliage && fi.is_none() {
            continue;
        }
        let lo_v = base + layer.band[0] * height_v;
        let hi_v = base + layer.band[1] * height_v;
        let r_v = (layer.radius * radius_v).max(0.0);
        let r_m = r_v * voxel_m;
        let cell =
            i64::from(stand.site.y) + i64::from(layers::disc_offset(layer.band[1], height_v));
        let cells = if is_foliage {
            (cell, cell)
        } else {
            let (lo, hi) = layers::trunk_offsets(layer.band, height_v);
            (
                i64::from(stand.site.y) + i64::from(lo),
                i64::from(stand.site.y) + i64::from(hi),
            )
        };
        out.push(StandLayer {
            index,
            foliage_index: fi,
            kind: layer.kind,
            band_v: [lo_v, hi_v],
            band_m: [lo_v * voxel_m, hi_v * voxel_m],
            radius_v: r_v,
            radius_m: r_m,
            cell,
            cells,
            share: if is_foliage { layer.share } else { 0.0 },
            capacity: if is_foliage { layer.share * cap } else { 0.0 },
            stock: fi.map_or(0.0, |i| stand.layer_stock[i]),
            porosity: layer.porosity,
            area_m2: (std::f64::consts::PI * r_m * r_m).max(MIN_LAYER_AREA_M2),
        });
    }
    out
}

impl<'a> FloraView<'a> {
    /// The **foliage-bearing** layers of `stand`, bottom-up: the contract's
    /// `FloraView::layers(stand)`, yielding each layer's band in metres, radius in
    /// metres, kind, stock and porosity. Trunk layers are not here — ask
    /// [`layers_of`] for the whole profile, which the presenter and the cone do.
    ///
    /// The stocks it yields **sum to the stand's `foliage`**, always.
    pub fn layers(&self, stand: &Stand) -> impl Iterator<Item = StandLayer> + use<> {
        layers_of(self.config, stand, self.config.voxel_m)
            .into_iter()
            .filter(|l| l.kind.bears_foliage())
    }

    /// [`FloraView::layers`] of whatever stands on `site`; empty if nothing does.
    pub fn layers_at(&self, site: Site) -> impl Iterator<Item = StandLayer> + use<> {
        let stand = self.stand_at(site).copied();
        stand
            .map(|s| layers_of(self.config, &s, self.config.voxel_m))
            .unwrap_or_default()
            .into_iter()
            .filter(|l| l.kind.bears_foliage())
    }

    /// Every layer of `stand`, trunks included, bottom-up.
    pub fn profile_layers(&self, stand: &Stand) -> Vec<StandLayer> {
        layers_of(self.config, stand, self.config.voxel_m)
    }

    pub fn stand_at(&self, site: Site) -> Option<&'a Stand> {
        self.stands
            .binary_search_by_key(&site, |s| s.site)
            .ok()
            .map(|i| &self.stands[i])
    }

    pub fn ground_at(&self, site: Site) -> Option<&'a Ground> {
        self.ground
            .binary_search_by_key(&site, |g| g.site)
            .ok()
            .map(|i| &self.ground[i])
    }

    /// Organic matter in every living and dead stock, the seed banks included. The site's
    /// mineral pool is not organic matter and is not in here.
    pub fn organic(&self) -> f64 {
        self.stands.iter().map(|s| s.material()).sum::<f64>()
            + self
                .ground
                .iter()
                .map(|g| {
                    g.litter
                        + g.dead_wood
                        + g.carrion
                        + g.seeds.iter().map(|c| c.organic).sum::<f64>()
                })
                .sum::<f64>()
    }

    /// Mineral in every stock: the sites' pools, the mineral held in litter, dead wood
    /// and the seed banks, and the mineral standing in living tissue.
    pub fn mineral(&self) -> f64 {
        self.stands.iter().map(|s| s.mineral).sum::<f64>()
            + self
                .ground
                .iter()
                .map(|g| {
                    g.mineral
                        + g.litter_mineral
                        + g.dead_wood_mineral
                        + g.carrion_mineral
                        + g.seeds.iter().map(|c| c.mineral).sum::<f64>()
                })
                .sum::<f64>()
    }

    /// Every stand whose **crown** a consumer standing on `from` can get at, with the
    /// foliage each of them holds, sorted by site. Stands with no foliage left are not
    /// listed: there is nothing there to eat.
    ///
    /// A stand qualifies when
    ///
    /// 1. **at least one of its crown cells** is within `reach.horizontal` voxels of
    ///    `from` in wrapped `x` **and** within the same in `z` — the crown's cells are the
    ///    integer disc of [`SpeciesConfig::crown_radius`] around its own column, which is
    ///    the set the shade model covers and the presenter paints; and
    /// 2. its crown's **lowest cell** — the one level at
    ///    `site.y + `[`SpeciesConfig::crown_voxels`] — is at most `reach.up` voxels above
    ///    `from.y`.
    ///
    /// `world` is read for one thing only, the world's width, because `x` wraps: a crown
    /// one step across the seam is one step away. Nothing else about the world enters, so
    /// this is geometry and not a path — **no line of sight**, no terrain between the eater
    /// and the food, no check that `from` is even a support face. The caller stands where
    /// it says it stands.
    ///
    /// Sorted by site because [`FloraView::stands`] is, and this walks it in order: a
    /// consumer's choice among what it can reach is therefore its own rule and never a
    /// storage order.
    pub fn reachable_foliage(
        &self,
        world: &VoxelView<'_>,
        from: Site,
        reach: Reach,
    ) -> Vec<(Site, f64)> {
        let width = world.config.width.max(1) as i64;
        let horizontal = i64::from(reach.horizontal);
        let ceiling = i64::from(from.y) + i64::from(reach.up);
        let mut out = Vec::new();
        for stand in self.stands {
            if !(stand.foliage > 0.0) {
                continue;
            }
            // Since layers, what is in reach is a **sum over the stand's layers** whose
            // disc cell is at or below the ceiling, not the whole plant or none of it:
            // a low browser at an adult bloomcrown reaches its basal rosette and never
            // its crown. For a single-layer species the sum is the whole `foliage` and
            // the disc cell is `crown_voxels(wood)`, so nothing about this call moved.
            let mut reachable = 0.0;
            let mut radius = 0.0f64;
            for layer in self.layers(stand) {
                if layer.cell > ceiling || !(layer.stock > 0.0) {
                    continue;
                }
                reachable += layer.stock;
                radius = radius.max(layer.radius_v);
            }
            if !(reachable > 0.0) {
                continue;
            }
            let span = radius.floor() as i64;
            let r2 = radius * radius;
            let mut within = false;
            for dz in -span..=span {
                for dx in -span..=span {
                    if (dx * dx + dz * dz) as f64 > r2 {
                        continue;
                    }
                    let cell_x = i64::from(stand.site.x) + dx;
                    let d = (cell_x - i64::from(from.x)).rem_euclid(width);
                    let wrapped = d.min(width - d);
                    let dz_abs = (i64::from(stand.site.z) + dz - i64::from(from.z)).abs();
                    if wrapped <= horizontal && dz_abs <= horizontal {
                        within = true;
                        break;
                    }
                }
                if within {
                    break;
                }
            }
            if within {
                out.push((stand.site, reachable));
            }
        }
        out
    }

    /// The **substrate** the sites of a species' **mycelium box** around `from` hold, in
    /// total: what [`SpeciesConfig::establish_substrate_min`] is compared against, and the
    /// stock a saprotroph's income is drawn from. **Zero for a [`Trophic::Photo`]
    /// species**, which never asks.
    ///
    /// It is the **sum of dead wood and litter** — a fungus eats both, and the one rate
    /// and the one gate apply to the sum. [`FloraView::dead_wood_in_box`] and
    /// [`FloraView::litter_in_box`] are the two halves, for a diagnosis that wants to say
    /// which of them a site holds. Carrion is not in it.
    ///
    /// The box is `substrate_reach_up_down` rows up **and** down and `rooting_radius`
    /// sideways, `x` wrapped and `z` clipped, read as **support sites** rather than as soil
    /// voxels, because a ground stock lives one per support face. Its vertical reach is its
    /// own species field and not the soil-water root box's (Astra R9.3).
    pub fn substrate_in_box(&self, world: &VoxelView<'_>, from: Site, sc: &SpeciesConfig) -> f64 {
        step::substrate_in_box(world, self.ground, from, sc)
    }

    /// The dead wood alone of [`FloraView::substrate_in_box`]'s box: half the substrate,
    /// and the half this method has always reported.
    pub fn dead_wood_in_box(&self, world: &VoxelView<'_>, from: Site, sc: &SpeciesConfig) -> f64 {
        step::substrate_pools_in_box(world, self.ground, from, sc).0
    }

    /// The litter alone of [`FloraView::substrate_in_box`]'s box: the other half, which a
    /// saprotroph has eaten since a litter diet was given to it.
    pub fn litter_in_box(&self, world: &VoxelView<'_>, from: Site, sc: &SpeciesConfig) -> f64 {
        step::substrate_pools_in_box(world, self.ground, from, sc).1
    }

    /// The one establishment predicate for **any** species, gate by gate, with the
    /// substrate read off this layer's own ground: [`can_establish`] cannot do that,
    /// because a `VoxelView` holds no dead wood and no litter, so this is the form a
    /// harness or a diagnosis wants once a saprotroph is in the world.
    pub fn establishment_gates(
        &self,
        world: &VoxelView<'_>,
        site: Site,
        species: Species,
    ) -> Gates {
        let sc = self.config.species(species);
        let (dead_wood, litter) = step::substrate_pools_in_box(world, self.ground, site, sc);
        step::establishment_gates_on_substrate(world, site, sc, dead_wood, litter)
    }

    /// `passes()` on [`FloraView::establishment_gates`]: the predicate itself, for any
    /// species, and the one the tick runs.
    pub fn can_establish(&self, world: &VoxelView<'_>, site: Site, species: Species) -> bool {
        self.establishment_gates(world, site, species).passes()
    }

    /// [`FloraView::establishment_gates`] for a whole skyline at once, with the geometric
    /// sky visibility read **once per site** through `sky` and shared across species and
    /// observations.
    ///
    /// The predicate, the thresholds and every other reading are the same function as the
    /// per-site form; only the ray's result is supplied. The cache holds terrain geometry
    /// alone, so pore water, saturation, standing water and the substrate of the species'
    /// mycelium box are read afresh here on every call: the result is the instantaneous
    /// reading it has always been, and no gate outcome is carried across ticks.
    ///
    /// The gates come back in the order the sites were given. A caller that wants the
    /// eligible members reads `passes()` on each, exactly as it would on the per-site form.
    pub fn establishment_gates_over(
        &self,
        world: &VoxelView<'_>,
        sites: &[Site],
        species: Species,
        sky: &mut SkyCache,
    ) -> Vec<Gates> {
        sky.sync(world);
        let sc = self.config.species(species);
        sites
            .iter()
            .copied()
            .map(|site| {
                let sky_visibility = sky.visibility(world, site);
                let (dead_wood, litter) =
                    step::substrate_pools_in_box(world, self.ground, site, sc);
                step::establishment_gates_with_sky(
                    world,
                    site,
                    sc,
                    sky_visibility,
                    dead_wood,
                    litter,
                )
            })
            .collect()
    }

    /// Energy in every living and dead stock.
    pub fn energy(&self) -> f64 {
        self.stands
            .iter()
            .map(|s| self.config.species(s.species).energy_density * s.material())
            .sum::<f64>()
            + self
                .ground
                .iter()
                .map(|g| {
                    g.litter_energy
                        + g.dead_wood_energy
                        + g.carrion_energy
                        + g.seeds
                            .iter()
                            .map(|c| self.config.species(c.species).energy_density * c.organic)
                            .sum::<f64>()
                })
                .sum::<f64>()
    }
}

/// A frontend or a test changes the plant layer only through these.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Plant a founder stand on the highest support face of column `(x, z)`: alive, with
    /// `wood`, full foliage `α·W` and full reserve. Refused if the column has no
    /// support, the site already holds a stand, or `wood` is below the species'
    /// `alive_min`.
    Seed {
        x: i64,
        z: u32,
        species: Species,
        wood: f64,
    },
    /// Plant a founder stand on **exactly this support face**, which may be a hollow's
    /// floor or the ground under a shelf: [`Command::Seed`] with the face named rather
    /// than resolved to the column's skyline. The seeder uses it so that the face it
    /// checked is the face it plants (`design/handoffs/voxel-startup-acceptance-2026-09-22.md`
    /// item 2). Refused if `site` is not a support face, already holds a stand, or `wood`
    /// is below the species' `alive_min`.
    SeedOnFace {
        site: Site,
        species: Species,
        wood: f64,
    },
    /// Remove the stand on the highest support face of column `(x, z)`, booking its
    /// material and energy as removed. Refused if there is none.
    Clear { x: i64, z: u32 },
    /// Remove **one species' whole seed bank** from the highest support face of column
    /// `(x, z)` — every arrival bin of it — booking its organic matter, its mineral and its
    /// `energy_density · organic` as removed, exactly as [`Command::Clear`] books a stand
    /// and as `prune_unsupported` books a bank whose face went away. Refused, booking
    /// nothing, if the column has no support, the site has no [`Ground`], or that species
    /// has no cohort there.
    ///
    /// Nothing else on the site is touched: the mineral pool, the litter, the dead wood,
    /// the carrion and the **other** species' cohorts stay as they were. That is what the
    /// replacement study's exclusion arm needs — a resident removed from a conditioned
    /// state with its water, its litter and its soil mineral retained and matched — and a
    /// `Clear` alone cannot do it, because a cleared stand's bank goes on germinating.
    ClearBank { x: i64, z: u32, species: Species },
}

/// Which of a site's dead pools a [`Deposit`] joins. Three, because a consumer has two
/// things to leave behind — a body, and what passed through it — and because something
/// outside the plant layer can put **wood** on the ground.
///
/// Dung is **litter** this round and not a pool of its own, which is a simplification
/// stated as one: droppings and shed leaves decompose at one rate here, and a separate
/// dung pool with its own rate is a later contract question, not a missing line of code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DepositKind {
    /// [`Ground::carrion`]: a corpse, on its own pool at its own rate.
    Carrion,
    /// [`Ground::litter`], through the same `e_d_max` cap plant litter goes through. It is
    /// **saprotroph food** as well as a shredder's: a deposit here feeds any fungus whose
    /// mycelium box reaches the site, on the same terms as a log.
    Litter,
    /// [`Ground::dead_wood`]: a **log**. Round 5a did not have this kind, because nothing
    /// in that round could put wood on the ground that a plant had not grown there
    /// (Astra's round 8 closes on the same point: "package N may add dead wood with the
    /// same booking/removal checks").
    ///
    /// Round 5b needs it twice over. A saprotroph eats dead wood, and a **fresh world has
    /// none**: the harness has to lay declared logs before a fungus can be introduced at
    /// all — a fungus also eats litter now, but a fresh world has none of that either. And a consumer that kills a stand and leaves its trunk standing is putting wood
    /// back, not litter and not a corpse.
    ///
    /// No energy cap, for carrion's reason: a deposit's energy comes from the depositor's
    /// own books and not from a species' `energy_density`, and dead wood has never had an
    /// `e_d_max`. A log laid without energy — `energy` 0 against a positive `organic` — is
    /// therefore a log with nothing in it to eat, and a saprotroph on it earns nothing and
    /// respires everything it takes (`step`'s `feed`). A harness laying a log should hand
    /// over `e_v · organic`, which is what a dead trunk holds.
    DeadWood,
}

/// Material a consumer hands back to the plant layer at one site: a corpse, droppings, or a
/// log.
///
/// The three currencies are given explicitly and none of them is derived from the other
/// two — there is no species here and no `energy_density` to read, because the consumer
/// that is depositing knows what its own body held. A deposit is the mirror of a
/// [`Taken`], and a consumer that deposits exactly what it took moves material without
/// creating any.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Deposit {
    pub kind: DepositKind,
    pub organic: f64,
    pub mineral: f64,
    pub energy: f64,
}

/// One package leaving one donor for one site, exactly as `propagate` sent it:
/// **observation, not state**.
///
/// Astra's R10.3. A harness watching the seed banks cannot tell who sent what where: two
/// donors delivering in one tick, a bank emptied by germination between two deliveries, and
/// attrition and expiry all change the same number, so a "largest bank increase" is an
/// inference and sometimes a fabrication. The model knows the answer while it is sending, so
/// it says so here.
///
/// **Transient.** [`Flora::step`] clears the list at the start of every tick, so it holds at
/// most one tick's deliveries and a caller that never reads it costs nothing. It is not part
/// of the layer's state in any sense that matters: nothing in `step` reads it, it is not
/// serialised (the plant layer itself is not), no conservation total includes it — the
/// material it describes is booked in [`FloraLedger::propagule_landed`] and in the recipient's
/// own cohort — and two runs that differ only in whether the receipts were read are the same
/// run. A clone carries whatever the last tick left, which is equally inert.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeliveryReceipt {
    /// The tick the package left, which is [`Flora::tick`] at the time.
    pub tick: u64,
    /// [`Stand::id`] of the donor. The identity a harness needs, because a founder donating
    /// repeatedly is not its descendants reproducing.
    pub donor: u64,
    pub species: Species,
    /// The support face the package landed on, as `dispersal_target` drew it — **never the
    /// donor's own site**, and with no habitat screening: it may be occupied, and it may be a
    /// site that will never germinate it.
    pub recipient: Site,
    /// The package's organic matter, `alive_min / w_frac`.
    pub organic: f64,
    /// The mineral that travelled with it, by the donor's own fraction rule.
    pub mineral: f64,
}

/// The plant layer. Owns its stands and ground stocks; borrows the world per call.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    /// This tick's [`DeliveryReceipt`]s, in the order `propagate` sent them. Cleared at the
    /// start of every tick: **observation, not state** (see [`DeliveryReceipt`]).
    deliveries: Vec<DeliveryReceipt>,
}

impl Flora {
    /// A plant layer on a **valid** config. Panics with the offending field named if
    /// [`FloraConfig::validate`] refuses it: a preset that cannot build a living stand is a
    /// programming error in the preset, not a runtime condition, and every caller in the
    /// workspace builds its config in code. [`Flora::try_new`] is the same thing for a
    /// caller that has a config from outside — a future `[flora]` TOML table — and wants to
    /// print the message instead.
    pub fn new(config: FloraConfig) -> Flora {
        match Flora::try_new(config) {
            Ok(flora) => flora,
            Err(e) => panic!("invalid FloraConfig — {e}"),
        }
    }

    /// [`Flora::new`] without the panic.
    pub fn try_new(config: FloraConfig) -> Result<Flora, String> {
        config.validate()?;
        Ok(Flora::unchecked(config))
    }

    /// A plant layer on a world, honouring [`FloraConfig::provision`]: identical to
    /// [`Flora::new`] under [`Provision::Lazy`], and under [`Provision::AtCreation`] it
    /// provisions **every support face of `world`** with [`FloraConfig::initial_mineral`]
    /// before it returns, booking the whole of it as `seeded_mineral_in` in this one call.
    ///
    /// Every support face and not every skyline column: a [`Ground`] can sit on any support
    /// face — `prune_unsupported` keeps exactly the supported ones — so this is the set of
    /// sites the layer could ever hold stocks on, read through the core's own
    /// `VoxelView::supports_in_column`. A world whose terrain later changes keeps the rule
    /// the model already has: a face that goes away has its stocks booked out as
    /// `removed_*`, and a face that appears is provisioned lazily like any other, because
    /// this call happens once and cannot see the future.
    ///
    /// Panics on an invalid config, like [`Flora::new`].
    pub fn in_world(world: &World, config: FloraConfig) -> Flora {
        match Flora::try_in_world(world, config) {
            Ok(flora) => flora,
            Err(e) => panic!("invalid FloraConfig — {e}"),
        }
    }

    /// [`Flora::in_world`] without the panic.
    pub fn try_in_world(world: &World, config: FloraConfig) -> Result<Flora, String> {
        let mut flora = Flora::try_new(config)?;
        if flora.config.provision == Provision::AtCreation {
            flora.provision_every_support_face(world);
        }
        Ok(flora)
    }

    /// One [`Ground`] per support face, each holding `initial_mineral`, all of it booked as
    /// `seeded_mineral_in`. Built in column order and then sorted by site, because
    /// `ground` is searched by binary search and its order is the model's invariant.
    fn provision_every_support_face(&mut self, world: &World) {
        let view = world.view();
        let c = view.config;
        let mineral = self.config.initial_mineral;
        let mut ground: Vec<Ground> = Vec::new();
        for z in 0..c.depth {
            for x in 0..c.width as i64 {
                for y in view.supports_in_column(x, z) {
                    let site = Site {
                        x: x.rem_euclid(c.width as i64) as u32,
                        y,
                        z,
                    };
                    ground.push(Ground::new(site, mineral));
                    self.ledger.seeded_mineral_in += mineral;
                }
            }
        }
        ground.sort_unstable_by_key(|g| g.site);
        self.ground = ground;
    }

    fn unchecked(config: FloraConfig) -> Flora {
        Flora {
            config,
            tick: 0,
            stands: Vec::new(),
            ground: Vec::new(),
            ledger: FloraLedger::default(),
            sky: Vec::new(),
            sky_version: None,
            deliveries: Vec::new(),
        }
    }

    pub fn config(&self) -> &FloraConfig {
        &self.config
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// This tick's delivery receipts: who sent a package, where it landed and how much of
    /// each currency went with it. Cleared at the start of the next [`Flora::step`], so a
    /// caller reads them between ticks or not at all.
    ///
    /// **Observation, not state** — see [`DeliveryReceipt`]. A harness that wants the
    /// destination of a package has to read this: the recipient cannot be recovered from the
    /// seed banks afterwards (Astra R10.3).
    pub fn deliveries(&self) -> &[DeliveryReceipt] {
        &self.deliveries
    }

    /// [`Flora::deliveries`], taken rather than borrowed, for a caller that wants to keep
    /// them past the next tick.
    pub fn take_deliveries(&mut self) -> Vec<DeliveryReceipt> {
        std::mem::take(&mut self.deliveries)
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
            Command::Seed {
                x,
                z,
                species,
                wood,
            } => {
                let Some(site) = highest_support(&view, x, z) else {
                    return false;
                };
                self.seed_founder(site, species, wood)
            }
            Command::SeedOnFace {
                site,
                species,
                wood,
            } => {
                let c = view.config;
                if site.x >= c.width
                    || site.z >= c.depth
                    || site.y + 1 >= c.height
                    || !view.is_support(i64::from(site.x), site.y, site.z)
                {
                    return false;
                }
                self.seed_founder(site, species, wood)
            }
            Command::Clear { x, z } => {
                let Some(site) = highest_support(&view, x, z) else {
                    return false;
                };
                let Ok(at) = self.stands.binary_search_by_key(&site, |s| s.site) else {
                    return false;
                };
                let s = self.stands.remove(at);
                // The parcel goes out with the stand: it is organic matter this layer
                // holds, and a `Clear` removes everything the site held.
                let organic = s.material();
                self.ledger.removed_organic_out += organic;
                self.ledger.removed_mineral_out += s.mineral;
                self.ledger.removed_energy_out +=
                    self.config.species(s.species).energy_density * organic;
                true
            }
            Command::ClearBank { x, z, species } => {
                let Some(site) = highest_support(&view, x, z) else {
                    return false;
                };
                let Ok(gi) = self.ground.binary_search_by_key(&site, |g| g.site) else {
                    return false;
                };
                let (mut organic, mut mineral) = (0.0, 0.0);
                let before = self.ground[gi].seeds.len();
                self.ground[gi].seeds.retain(|c| {
                    if c.species != species {
                        return true;
                    }
                    organic += c.organic;
                    mineral += c.mineral;
                    false
                });
                if self.ground[gi].seeds.len() == before {
                    return false;
                }
                self.ledger.removed_organic_out += organic;
                self.ledger.removed_mineral_out += mineral;
                self.ledger.removed_energy_out +=
                    self.config.species(species).energy_density * organic;
                true
            }
        }
    }

    /// Plant a founder stand on `site`, a support face the caller has resolved: the one
    /// body [`Command::Seed`] and [`Command::SeedOnFace`] share.
    fn seed_founder(&mut self, site: Site, species: Species, wood: f64) -> bool {
        let sc = self.config.species(species);
        if !(wood.is_finite() && wood >= sc.alive_min) {
            return false;
        }
        let Err(at) = self.stands.binary_search_by_key(&site, |s| s.site) else {
            return false;
        };
        let mut stand = Stand {
            id: self.ledger.births,
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
            aeration_stress: 0.0,
            parcel: 0.0,
            layer_stock: [0.0; MAX_FOLIAGE_LAYERS],
            profile_stage: 0,
        };
        // A founder arrives full, so its layers arrive at their capacities.
        stand.bin_foliage(sc, self.config.voxel_m);
        self.ledger.births += 1;
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
            self.ground
                .insert(g, Ground::new(site, self.config.initial_mineral));
        }
        true
    }

    /// A consumer eats the **foliage** of the stand on `site`, up to `want`: at most what
    /// the stand's `P` holds, never its wood and never its reserve.
    ///
    /// The mineral leaves by the same fraction rule every other outflow uses
    /// ([`Stand::mineral`]): a transfer of a fraction of the stand's material takes that
    /// fraction of its mineral stock. The energy is the tissue's own `energy_density` times
    /// the organic matter taken.
    ///
    /// `None`, booking nothing at all, when there is nothing to take: no stand on the site,
    /// a stand with no foliage left, or a `want` that is not a positive finite number.
    /// `Some` therefore always carries a strictly positive `organic`.
    ///
    /// Applied **between** ticks, like a [`Command`]: nothing here reads or moves the
    /// world, and the tick that follows sees the smaller `P` in its own income
    /// (`step`'s step 6). Taking a stand's whole canopy does not kill it — death is
    /// `wood < alive_min` and wood is untouched — it starves it, which is the producer
    /// response the crate doc describes.
    pub fn take_foliage(&mut self, site: Site, want: f64) -> Option<Taken> {
        self.withdraw_foliage(site, want, None).map(|t| t.taken)
    }

    /// A consumer eats the foliage the **layers whose disc cells lie in `layers`** hold,
    /// lowest first, up to `want`.
    ///
    /// This is the bite a mouth takes: `layers` is the cell range the mouth's physical
    /// band selects over the face it stands on
    /// (`cubarium_voxel_fauna::band_crown_layers`), so a low browser at an adult
    /// bloomcrown withdraws from its basal rosette and the crown three cells up is not
    /// offered. What comes back reports the withdrawal **per layer**, so the caller can
    /// book exactly what left the stand rather than a share of a scalar.
    ///
    /// **A bite from below leaves the upper stock untouched.** Taking 0.10 from a
    /// 0.25 / 0.75 stand leaves 0.15 / 0.75; nothing anywhere re-derives the shares of
    /// what is left (`design/7_Research/organism-systems-audit-2026-09-21.md` §5).
    ///
    /// Everything else — the mineral fraction rule, the energy, the ledger entry, the
    /// `None` cases — is [`Flora::take_foliage`]'s, which is this with no layer bound.
    pub fn take_foliage_in_layers(
        &mut self,
        site: Site,
        want: f64,
        layers: &std::ops::RangeInclusive<i64>,
    ) -> Option<TakenFoliage> {
        self.withdraw_foliage(site, want, Some(layers))
    }

    fn withdraw_foliage(
        &mut self,
        site: Site,
        want: f64,
        layers: Option<&std::ops::RangeInclusive<i64>>,
    ) -> Option<TakenFoliage> {
        if !(want > 0.0) || !want.is_finite() {
            return None;
        }
        let i = self.stands.binary_search_by_key(&site, |s| s.site).ok()?;
        let e_v = self.config.species(self.stands[i].species).energy_density;
        // Which layers the mouth may take from, lowest first, and what each holds.
        let offered: Vec<(usize, f64)> =
            layers_of(&self.config, &self.stands[i], self.config.voxel_m)
                .into_iter()
                .filter_map(|l| {
                    let fi = l.foliage_index?;
                    let inside = layers.is_none_or(|range| range.contains(&l.cell));
                    (inside && l.stock > 0.0).then_some((fi, l.stock))
                })
                .collect();
        let mut per_layer = [0.0f64; MAX_FOLIAGE_LAYERS];
        let mut organic = 0.0f64;
        let mut left = want;
        for (fi, stock) in offered {
            if !(left > 0.0) {
                break;
            }
            let take = left.min(stock);
            per_layer[fi] = take;
            self.stands[i].layer_stock[fi] -= take;
            organic += take;
            left -= take;
        }
        if !(organic > 0.0) {
            return None;
        }
        // The fraction rule reads the stand's whole material, parcel included, exactly as
        // senescence, dieback, death and a leaving package do.
        let before = self.stands[i].material();
        self.stands[i].foliage -= organic;
        let mineral = step::pull_mineral(&mut self.stands[i], before, organic);
        // The stocks and the scalar were moved by the same amounts in a different
        // order; the float residue between them goes back into the lowest layer the
        // bite touched, so `sum(layer_stock) == foliage` holds to the ulp.
        let count = self
            .config
            .species(self.stands[i].species)
            .foliage_layer_count(self.stands[i].wood);
        self.stands[i].settle_layers(count);
        let taken = self.book_consumed(Taken {
            organic,
            mineral,
            energy: e_v * organic,
        });
        Some(TakenFoliage { taken, per_layer })
    }

    /// A consumer eats **dead wood** off the site's ground, up to `want`, with its mineral
    /// and its retained energy pro rata. `None` when the site has no ground entry, its
    /// dead-wood pool is empty, or `want` is not a positive finite number.
    ///
    /// Pro rata means at the stock's own **current density**, which is what `decompose`
    /// does: neither the mineral nor the energy density of what is left moves, so a pool
    /// half eaten is the same stuff it was.
    pub fn take_dead_wood(&mut self, site: Site, want: f64) -> Option<Taken> {
        let i = self.ground.binary_search_by_key(&site, |g| g.site).ok()?;
        let g = &mut self.ground[i];
        let taken = take_pool(
            &mut g.dead_wood,
            &mut g.dead_wood_mineral,
            &mut g.dead_wood_energy,
            want,
        )?;
        Some(self.book_consumed(taken))
    }

    /// A consumer eats **litter** off the site's ground, up to `want`, with its mineral and
    /// its retained energy pro rata — [`Flora::take_dead_wood`]'s rule on the other pool.
    pub fn take_litter(&mut self, site: Site, want: f64) -> Option<Taken> {
        let i = self.ground.binary_search_by_key(&site, |g| g.site).ok()?;
        let g = &mut self.ground[i];
        let taken = take_pool(
            &mut g.litter,
            &mut g.litter_mineral,
            &mut g.litter_energy,
            want,
        )?;
        Some(self.book_consumed(taken))
    }

    /// A consumer eats **carrion** off the site's ground, up to `want`, with its mineral
    /// and its retained energy pro rata — [`Flora::take_dead_wood`]'s rule on the third
    /// pool, and the withdrawal side of [`DepositKind::Carrion`].
    ///
    /// Carrion had decomposition and no consumer until decisions §3 made the shredder a
    /// detritivore with three foods (`design/handoffs/voxel-diets-2026-09-22.md`). What
    /// it takes is exactly what the pool holds — organic, mineral and energy at the
    /// stock's own current density — so nothing is invented for a corpse that a
    /// depositor's own books did not hand over, and what is left is the same stuff it
    /// was.
    pub fn take_carrion(&mut self, site: Site, want: f64) -> Option<Taken> {
        let i = self.ground.binary_search_by_key(&site, |g| g.site).ok()?;
        let g = &mut self.ground[i];
        let taken = take_pool(
            &mut g.carrion,
            &mut g.carrion_mineral,
            &mut g.carrion_energy,
            want,
        )?;
        Some(self.book_consumed(taken))
    }

    /// A consumer puts material **back** on `site`: a corpse into the carrion pool,
    /// droppings into the litter pool, or a log into the dead-wood pool. Returns whether it
    /// was accepted.
    ///
    /// Refused — booking nothing — when any of the three numbers is not finite or is
    /// negative, and when all three are zero: a deposit of nothing is not a deposit, and
    /// accepting one would provision a `Ground` (and import its `initial_mineral`) for no
    /// material at all.
    ///
    /// **A deposit whose `organic` is zero is accepted and settles at once**, whatever its
    /// kind: its mineral goes straight to the site's soluble pool and its energy leaves as
    /// heat, because a dead pool decomposes `rate · dt · organic` and a pool with no
    /// organic matter in it would hold that mineral for ever (Astra R8.2). It is still
    /// booked as `deposited_*_in` in full, and it still provisions a `Ground`.
    ///
    /// A deposit on a site with no [`Ground`] provisions one, and the lazy
    /// `initial_mineral` rule applies and is booked as `seeded_mineral_in` exactly as it
    /// is for a founder or a landed package. Nothing here reads the world, so a deposit on
    /// a site that is no longer a support face is accepted and then **booked out** as
    /// `removed_*` by step 1 of the next tick, like any other stock on a site the terrain
    /// took away.
    ///
    /// # Where this sits in the tick
    ///
    /// Applied **between** ticks, like a [`Command`] and like the withdrawals. `step`
    /// takes its decomposition snapshot as the very first thing it does, so material
    /// deposited between tick `t` and tick `t + 1` is in the snapshot of tick `t + 1` and
    /// decomposes in it — "a tick's decomposition sees the previous inter-tick's deposits",
    /// which is the tick-start snapshot rule (`step`'s module doc, step 7) and not a new
    /// one. A carrion deposit therefore starts respiring on the **next** step, which is
    /// **sooner** than what this tick's own senescence sheds: that waits for the step after
    /// it, because it is not in this one's snapshot (Astra R8.5 — the two were equated here,
    /// and they are one tick apart).
    pub fn deposit(&mut self, site: Site, deposit: Deposit) -> bool {
        let Deposit {
            kind,
            organic,
            mineral,
            energy,
        } = deposit;
        for v in [organic, mineral, energy] {
            if !v.is_finite() || v < 0.0 {
                return false;
            }
        }
        if organic <= 0.0 && mineral <= 0.0 && energy <= 0.0 {
            return false;
        }
        let gi = match self.ground.binary_search_by_key(&site, |g| g.site) {
            Ok(i) => i,
            Err(i) => {
                self.ledger.seeded_mineral_in += self.config.initial_mineral;
                self.ground
                    .insert(i, Ground::new(site, self.config.initial_mineral));
                i
            }
        };
        self.ledger.deposited_organic_in += organic;
        self.ledger.deposited_mineral_in += mineral;
        self.ledger.deposited_energy_in += energy;
        // **A deposit with no organic matter in it is terminal at once** (Astra R8.2). A
        // dead pool's decomposition is `rate · dt · organic`, so a pool holding mineral and
        // energy against zero organic matter releases nothing for ever: an exhausted
        // consumer whose respiration left only mineral behind would have created an inert
        // sink that waits for unrelated material to arrive and dilute itself into. The
        // conservation totals closed either way; the defect was the terminal state of a
        // public API. So the mineral goes straight to the site's soluble pool, where
        // decomposition would have put it, and the energy leaves as heat, where
        // decomposition would have sent it — with the full `deposited_*` booking above and
        // the same provisioning rule, whichever kind was asked for. The alternative,
        // refusing the triplet, was rejected because it leaves a consumer with nowhere to
        // put the mineral it is holding.
        if organic <= 0.0 {
            self.ground[gi].mineral += mineral;
            self.ledger.heat_out += energy;
            return true;
        }
        match kind {
            DepositKind::Carrion => {
                let g = &mut self.ground[gi];
                g.carrion += organic;
                g.carrion_mineral += mineral;
                g.carrion_energy += energy;
            }
            // Dead wood, on the pool a dieback and a death already fill, with no energy
            // cap — dead wood has never had one.
            DepositKind::DeadWood => {
                let g = &mut self.ground[gi];
                g.dead_wood += organic;
                g.dead_wood_mineral += mineral;
                g.dead_wood_energy += energy;
            }
            // The existing cap rule, with the existing consequence: energy over
            // `e_d_max · D` cannot be held by litter and leaves as heat at once, which the
            // `deposited_energy_in` above has already booked in, so the energy residual
            // holds whatever the cap refuses.
            DepositKind::Litter => step::add_litter_cap(
                self.config.litter_energy_cap,
                &mut self.ground[gi],
                organic,
                mineral,
                energy,
                &mut self.ledger,
            ),
        }
        true
    }

    /// One withdrawal on the boundary: the three named `consumed_*` flows, and the same
    /// [`Taken`] back to the caller, so what the ledger books and what the consumer
    /// receives are one value and cannot drift.
    fn book_consumed(&mut self, taken: Taken) -> Taken {
        self.ledger.consumed_organic_out += taken.organic;
        self.ledger.consumed_mineral_out += taken.mineral;
        self.ledger.consumed_energy_out += taken.energy;
        taken
    }
}

/// A bounded pro-rata draw on one of a site's dead pools: `want` of its organic matter at
/// most, with the mineral and the energy that were in it at the stock's **current
/// density**, which is `decompose`'s own rule. `None` when nothing can be taken, so the
/// caller books nothing.
fn take_pool(organic: &mut f64, mineral: &mut f64, energy: &mut f64, want: f64) -> Option<Taken> {
    if !(want > 0.0) || !want.is_finite() || !(*organic > 0.0) {
        return None;
    }
    let out = want.min(*organic);
    // `f` is 1.0 exactly when the pool is emptied, so an emptied pool hands over every
    // unit of its mineral and its energy and keeps no float dust claiming to be a stock.
    let f = out / *organic;
    let m = if f >= 1.0 {
        *mineral
    } else {
        (*mineral * f).min(*mineral)
    };
    let e = if f >= 1.0 {
        *energy
    } else {
        (*energy * f).min(*energy)
    };
    *organic -= out;
    *mineral -= m;
    *energy -= e;
    Some(Taken {
        organic: out,
        mineral: m,
        energy: e,
    })
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
    Some(Site {
        x: x.rem_euclid(c.width as i64) as u32,
        y,
        z,
    })
}
