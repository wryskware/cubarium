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
//! # Transfers to a consumer layer, and the producer response
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
//! **measurement** of those rules and not a mechanic of its own.

#![forbid(unsafe_code)]

pub mod cover;
mod registry;
pub use registry::registry;
mod layers;
#[cfg(test)]
mod seed_marks_tests;
#[cfg(test)]
mod overtop_covers_tests;
#[cfg(test)]
mod carrion_tests;
#[cfg(test)]
mod spread_tests;
#[cfg(test)]
mod shared_face_tests;
#[cfg(test)]
mod canopy_establish_tests;
mod film;
#[cfg(test)]
mod film_tests;
mod seeds;
pub use seeds::{AttemptTrace, Attempts};
pub mod snapshot;
mod step;
#[doc(hidden)]
pub mod study;

use cubarium_voxel::{Material, VoxelView, World};
use serde::{Deserialize, Serialize};

pub use cubarium_voxel::{DT, TICK_HZ};

/// The latticevine face cover (D15 Revision 2): see [`cover`].
pub use cover::{
    Cover, CoveredFace, Face, FaceDir, FaceDraw, SpurPhase, Vine, VineConfig, VineId, VineSeed,
    face_eligible, face_light, vine_seed_package,
};

pub use layers::{
    Layer, LayerKind, MAX_FOLIAGE_LAYERS, Profile, StandLayer, disc_offset, trunk_offsets,
};
pub use step::upkeep_factor;
/// The germination predicate the model itself uses, gate by gate, for a caller outside a
/// tick (a harness picking founder columns, a diagnosis of which gate is shut):
/// `Gates::passes()` is the predicate. `establishment_gates_with_sky` is the one function
/// every form calls, for a caller that already has the sky reading and the substrate;
/// `establishment_gates` reads the sky off the view and assumes no substrate; and
/// [`FloraView::establishment_gates`] and [`FloraView::establishment_gates_over`] read the
/// substrate off this layer's ground.
pub use step::{Gates, establishment_gates};
pub use step::{adult_light_cover, establishment_gates_with_sky};
/// Where a falling stand's wood lands, for a diagnostic.
pub use step::fall_line;
/// The overlapped tick's planned drink, between the plant step and the barrier.
pub use step::{DrinkPlan, PlantStep};
/// What one cell of ground offers a root: the scale `μ` and the establishment
/// water gate read.
pub use step::available_water;

/// The **stands** of the voxel ecology, each one a role: see the preset that carries its
/// numbers ([`SpeciesConfig::bloomcrown`] and the five after it) for the sentence of
/// ecology the numbers encode.
///
/// Five of the six are producers. The sixth, [`Species::Glowcap`], is a
/// [`Trophic::Saprotroph`]: the same stand, the same lifecycle, and dead organic matter —
/// dead wood and litter both — where the light was. "Species" is therefore the crate's word for a kind of stand and not a claim
/// that they are all plants.
///
/// Slots are stable: a new species is appended, so a [`FloraLedger`] array read by index
/// keeps its meaning.
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
    /// The canopy builder of deep aerated soil: establishes only in an open
    /// gap, slow, long-lived, a large reserve; at death its wood falls as a line of dead
    /// wood as long as the crown was wide.
    Vaulttree,
    /// The fruiting shrub of grove edges and moist bright gaps: a fan of
    /// stems under low, wide foliage that a ground browser crops below its band.
    Lanternberry,
    /// The emergent of saturated soil and shallow standing water: needs
    /// settled standing water on or beside its site and pays for its aeration.
    Siphonreed,
}

impl Species {
    /// How many species there are, and the length of every per-species array in
    /// [`FloraLedger`]. Derived from [`Species::ALL`] so the two can never disagree.
    pub const COUNT: usize = Species::ALL.len();

    /// New species are **appended**, so every earlier ledger slot keeps its meaning.
    pub const ALL: [Species; 9] = [
        Species::Bloomcrown,
        Species::Umbrellafrond,
        Species::Springturf,
        Species::Stonecushion,
        Species::Velvetpad,
        Species::Glowcap,
        Species::Vaulttree,
        Species::Lanternberry,
        Species::Siphonreed,
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
            Species::Vaulttree => 6,
            Species::Lanternberry => 7,
            Species::Siphonreed => 8,
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
            Species::Vaulttree => "vaulttree",
            Species::Lanternberry => "lanternberry",
            Species::Siphonreed => "siphonreed",
        }
    }

    pub fn parse(s: &str) -> Option<Species> {
        Species::ALL.into_iter().find(|sp| sp.name() == s)
    }
}

/// A species list written as names (`["velvetpad", "stonecushion"]`), the way the
/// registry and the presets name species: [`Species::name`] out, [`Species::parse`] in,
/// an unknown name refused.
mod species_names {
    use super::Species;
    use serde::{Deserialize, Deserializer, Serializer, de::Error, ser::SerializeSeq};

    pub fn serialize<S: Serializer>(list: &[Species], s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(list.len()))?;
        for sp in list {
            seq.serialize_element(sp.name())?;
        }
        seq.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Species>, D::Error> {
        let names: Vec<String> = Vec::deserialize(d)?;
        names
            .iter()
            .map(|n| Species::parse(n).ok_or_else(|| D::Error::custom(format!("unknown species `{n}`"))))
            .collect()
    }
}

/// Life stage of a stand. A site with no stand is **bare**; propagule material waiting on
/// a site is a [`SeedCohort`] in that site's [`Ground`], never a stand.
///
/// One variant on purpose — a stand is alive or it is not there — so that code which
/// matches on a stage keeps saying which it means.
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
/// landing that joins the same bin adds material without moving the age, so a small fresh
/// arrival cannot keep old material alive.
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
    /// ever created has its own, founders included, and it never changes or repeats. A
    /// founder can die and its own species germinate on its site in the same tick, so a
    /// descendant count reads identities, not sites.
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
    /// stated limitation of this round rather than a physiological claim.
    /// Income reads the *site's* pool: `assimilation`'s Michaelis–Menten term, the
    /// `nutrient_draw_max` rate cap and the `mineral / n_tissue` stock cap all read
    /// `Ground::mineral`, so a stand rich in mineral fixes **zero** on a bare pool at full
    /// light and full moisture and has to burn reserve to stand still
    /// (`tests/life_cycle.rs`, `a_bare_mineral_pool_stops_the_income_and_not_just_the_growth`
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
    /// A donor saves one recipient's worth per tick until it holds one whole minimum
    /// package and sends **that**, to one recipient. The mineral of the parcel stays in
    /// the stand until the package leaves, and travels with it by the fraction rule.
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
    /// **Induced grazing defence**, `0..1`: raised by every bite
    /// ([`SpeciesConfig::defence_per_bite`]), decaying at the species' half-life once
    /// grazing stops. A bite takes `(1 − defence)` of what was asked.
    #[serde(default)]
    pub defence: f64,
    /// Which entry of the species' `profile` the stocks above are binned for. When the
    /// stand's wood crosses a stage threshold, the tick re-bins the **existing** total
    /// into the new stage bottom-up and updates this; nothing is created
    /// (`design/handoffs/voxel-organism-decisions-2026-09-21.md` §4).
    #[serde(default)]
    pub profile_stage: u8,
    /// **Nectar** held in the stand's flowers, organic units (F1, §3.7): paid from the
    /// reserve in daylight up to the species' `nectar_cap`, taken by
    /// [`Flora::take_stand_nectar`]. Not the seed parcel. Like the parcel it is no longer
    /// tissue (it earns and pays nothing) but it is still the stand's material
    /// ([`Stand::material`]): its energy is the species' density, its mineral stays in
    /// [`Stand::mineral`] until it is taken, and it falls to litter when the stand dies.
    #[serde(default)]
    pub nectar: f64,
    /// **Fruit** hanging on the stand, organic units (F3, the seedporter's food;
    /// predation-and-flight §3.7): paid from the reserve in daylight up to the species'
    /// `fruit_cap` on an adult stand, taken by [`Flora::take_stand_fruit`], falling to
    /// litter at the stand's site when left uneaten past `fruit_drop_s`. Like nectar, the
    /// stand's material, not its tissue: its energy is the species' density, its mineral
    /// stays in [`Stand::mineral`] until it is taken or falls, and it falls to litter when
    /// the stand dies.
    #[serde(default)]
    pub fruit: f64,
    /// Seconds the current crop has hung: counted from the tick the stock goes from empty
    /// to holding fruit, back to zero when it is eaten out or falls. When it reaches the
    /// species' `fruit_drop_s` the whole hanging crop falls to the litter (F3).
    #[serde(default)]
    pub fruit_age_s: f64,
    /// Seconds left of the stand's rest after an uneaten crop fell: no fruit is made while
    /// it is positive ([`SpeciesConfig::fruit_rest_s`]).
    #[serde(default)]
    pub fruit_rest_s: f64,
    /// **Cured** (drought dormancy, [`SpeciesConfig::cure_after_s`]): its foliage dropped
    /// to the litter, it fixes nothing, pays a fraction of its maintenance out of its
    /// reserve and waits for water. The presenter draws it in the species' cured colour.
    #[serde(default)]
    pub cured: bool,
    /// Seconds the stand has been dry below [`SpeciesConfig::cure_water`] while green, and
    /// seconds it has been dormant once cured.
    #[serde(default)]
    pub dry_s: f64,
    /// **A runner daughter's support** from its parent ([`SpeciesConfig::runner_support_s`]):
    /// `Some` while the parent still feeds it, `None` for every other stand and once the
    /// daughter pays its own upkeep, the time runs out, or the parent is gone.
    #[serde(default)]
    pub support: Option<Support>,
}

/// A runner daughter's tie to the parent that feeds it ([`Stand::support`]): the parent's
/// id and site, so the parent is found by the stands' own key and checked by its id, and
/// the seconds of support left.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Support {
    pub parent: u64,
    pub parent_site: Site,
    pub left_s: f64,
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

    /// `W + P + Q + parcel + nectar + fruit`: every unit of organic matter this stand
    /// holds, which is what the ledger has to account for and what the stand's mineral is
    /// a stock against.
    pub fn material(&self) -> f64 {
        self.organic() + self.parcel + self.nectar + self.fruit
    }
}

/// A dead animal's record on a ground pool (`design/predation-and-flight-2026-09-28.md`
/// §2.5): which lineage it was, which way it lay, and the organic matter it died with. A
/// drawing aid and nothing else — the carrion pool holds the matter.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Carcass {
    /// The dead body's lineage code: the fauna's `Founder::index`.
    pub lineage: u8,
    /// Its heading when it died, radians.
    pub heading_rad: f64,
    /// The organic matter the body went into the pool with.
    pub organic: f64,
}

impl Carcass {
    /// Whether the pool holding `carrion` is **remains** rather than the body: below
    /// [`FloraConfig::carcass_remains_fraction`] (30 % shipped) of what the body died with
    /// (dossier D2's "collapsed body, then bone").
    pub fn remains(&self, carrion: f64, config: &FloraConfig) -> bool {
        carrion < config.carcass_remains_fraction * self.organic
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
    /// The **carcass record** (`design/predation-and-flight-2026-09-28.md` §2.5): the dead
    /// animal whose body lies in `carrion` here, so the presenter can draw it — its model
    /// lying flat, then remains below 30 % ([`Carcass::remains`]). It carries **no
    /// mass**: the carrion pool does. Laid by a kill ([`Flora::lay_carcass`]) and cleared
    /// once the pool has fallen below [`FloraConfig::carcass_gone_fraction`] of what the body
    /// died with.
    pub carcass: Option<Carcass>,
    /// The species and tick of the latest seed **landing** here: what the D5
    /// seed mark reads as "recent" ([`Ground::seed_mark`]). A cohort's `bin_start_tick`
    /// cannot say this — a bin is hours wide — so the landing path writes it.
    pub last_landing: Option<(Species, u64)>,
    /// The species this site's bank will **sprout at its next check**: its
    /// gates passed and it won the lottery at the last check, and it germinates at the
    /// next one if they still pass. A one-check delay, so the presenter reads a flag and
    /// evaluates no gate per frame.
    ///
    /// The **plant's** arm ([`FaceLayer::Plant`]); a fungus has its own,
    /// [`Ground::sprouting_fungus`], since each place runs its own lottery.
    pub sprouting: Option<Species>,
    /// [`Ground::sprouting`] for the face's **fungus** place ([`FaceLayer::Fungus`]).
    #[serde(default)]
    pub sprouting_fungus: Option<Species>,
    /// **Glassfilm** (`design/handoffs/voxel-roster-2026-10-01.md`, R2a): the living film
    /// on a shallow submerged face, organic matter. A fourth pool beside litter, dead wood
    /// and carrion, stepped by `film.rs`: it grows from light on the site's `mineral`,
    /// spreads to less-filmed wet neighbours, dies to this site's litter when the site
    /// dries or deepens, and is grazed with [`Flora::take_film`].
    pub film: f64,
    /// The mineral held in the film: drawn from `mineral` as it grows, returned to it as
    /// it respires.
    pub film_mineral: f64,
    /// The energy held in the film, respired as heat at the stock's current density.
    pub film_energy: f64,
    /// **Latticevine seeds** voided here by an animal (roster mechanics fix 3): the vine's
    /// seed bank on this face, beside the stands' [`Ground::seeds`]. Tested at the site's
    /// bank check by the lanternberry's rule — one whole seed and a site it can establish
    /// on — and founds a vine there ([`VineSeedLot`]).
    pub vine_seeds: Option<VineSeedLot>,
}

/// Latticevine seed on one face (roster mechanics fix 3): organic matter and the mineral
/// that came with it, banked by [`Flora::deposit_vine_seeds`]. Its energy is the vine's
/// `energy_density` times its organic matter. Its age runs from the first landing, a bin
/// that later landings join without moving it, as a [`SeedCohort`]'s does.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct VineSeedLot {
    pub organic: f64,
    pub mineral: f64,
    pub landed_tick: u64,
}

impl Ground {
    /// Drop the carcass record once its pool has gone below `gone_fraction`
    /// ([`FloraConfig::carcass_gone_fraction`]) of what the body died with.
    pub(crate) fn clear_spent_carcass(&mut self, gone_fraction: f64) {
        if let Some(c) = self.carcass {
            if self.carrion <= gone_fraction * c.organic {
                self.carcass = None;
            }
        }
    }

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
            carcass: None,
            last_landing: None,
            sprouting: None,
            sprouting_fungus: None,
            film: 0.0,
            film_mineral: 0.0,
            film_energy: 0.0,
            vine_seeds: None,
        }
    }

    /// The arm of `place`'s lottery ([`Ground::sprouting`], [`Ground::sprouting_fungus`]).
    pub fn arm(&self, place: FaceLayer) -> Option<Species> {
        match place {
            FaceLayer::Plant => self.sprouting,
            FaceLayer::Fungus => self.sprouting_fungus,
        }
    }

    pub(crate) fn arm_mut(&mut self, place: FaceLayer) -> &mut Option<Species> {
        match place {
            FaceLayer::Plant => &mut self.sprouting,
            FaceLayer::Fungus => &mut self.sprouting_fungus,
        }
    }

    /// The latticevine seed banked here, organic matter.
    pub fn vine_seed_organic(&self) -> f64 {
        self.vine_seeds.map_or(0.0, |l| l.organic)
    }

    /// The species whose D5 seed mark this site shows at `tick`, if any ("draw only recent
    /// seeds and about to sprout ones"): the species about
    /// to sprout, or else the species of a landing within
    /// [`FloraConfig::seed_mark_recent_s`] — and in either case only while the bank still
    /// holds that species' seed. A bank that is simply waiting shows nothing.
    pub fn seed_mark(&self, tick: u64, config: &FloraConfig) -> Option<Species> {
        let held = |sp: Species| {
            self.seeds
                .iter()
                .any(|c| c.species == sp && c.organic > 0.0)
        };
        // The plant's arm first, then the fungus's.
        if let Some(sp) = self.sprouting.filter(|&sp| held(sp)).or(self.sprouting_fungus.filter(|&sp| held(sp))) {
            return Some(sp);
        }
        let recent = (config.seed_mark_recent_s * f64::from(TICK_HZ)).round() as u64;
        match self.last_landing {
            Some((sp, at)) if tick.saturating_sub(at) <= recent && held(sp) => Some(sp),
            _ => None,
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

/// **A stand's place on its face** (2026-10-05, Wrysk: "glowcaps and plants can coincide
/// ... like normal mushrooms grow under trees"): a support face holds at most **one plant
/// and one fungus**. The place is the stand's [`Trophic`] mode, never a config number of
/// its own: a [`Trophic::Photo`] stand is the face's plant, a [`Trophic::Saprotroph`] its
/// fungus.
///
/// The two places share everything the face has — its ground (mineral pool, litter, dead
/// wood, carrion), its root water and its seed bank — and nothing else: every way onto a
/// face (the seed lottery, a runner, a founder) asks only its own place, so a plant comes
/// up on a glowcap's face through its own gates and a glowcap comes up under a plant
/// through its own, and neither displaces the other. Overtopping and runner displacement
/// are plant against plant. The two stands of one face never shade each other: the cap
/// grows under the plant's leaves (a glowcap's crown top is above a bloomcrown's basal
/// rosette, so without this it would shade it), and a fungus earns nothing from light.
///
/// [`FloraView::stands`] is sorted by `(site, place)`, the plant first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FaceLayer {
    /// The face's photosynthetic stand: what [`FloraView::stand_at`] answers.
    Plant,
    /// The face's saprotroph: what [`FloraView::fungus_at`] answers.
    Fungus,
}

impl FaceLayer {
    pub const ALL: [FaceLayer; 2] = [FaceLayer::Plant, FaceLayer::Fungus];

    /// The place a stand of this trophic mode takes.
    pub fn of(trophic: Trophic) -> FaceLayer {
        match trophic {
            Trophic::Photo => FaceLayer::Plant,
            Trophic::Saprotroph => FaceLayer::Fungus,
        }
    }

    /// `0` for the plant, `1` for the fungus: an array slot.
    pub fn index(self) -> usize {
        self as usize
    }
}

/// **How a species' seeds travel.** Every mode lands one whole seed — one package — per delivery; what differs is where.
/// `seeds.rs`'s `landing` is the rule, and [`SpeciesConfig::hop`] is each mode's distance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dispersal {
    /// Uniform over the highest support faces within `hop`, habitat unscreened: the bank
    /// waits for the gap.
    #[default]
    Drop,
    /// A fat-tailed, isotropic distance at a scale of `hop`: 79 % within three hops, 4 %
    /// past ten, capped at the world, habitat unscreened.
    Wind,
    /// Lands only on a face whose ground passes the species' gates — every gate but light
    /// — within a wide radius `hop`; the rain that finds none is litter. A spore rain
    /// mostly falls where nothing grows, and only what lands where it could is modelled.
    Spores,
    /// Lands only on a standing-water margin within a wide radius `hop`, downhill first;
    /// with no margin in reach it is litter under the parent.
    Water,
    /// **Wind, on the WALD kernel** (springturf spread, 2026-10-04): an inverse-Gaussian
    /// distance with mean [`SpeciesConfig::wald_mean_m`] and shape
    /// [`SpeciesConfig::wald_shape_m`], in metres, in a uniform direction, wrapped round
    /// the ring and reflected off the walls like [`Dispersal::Wind`], onto the column's
    /// highest support face; habitat unscreened.
    ///
    /// Why WALD and not [`Dispersal::Wind`]'s Lomax: the WALD is the mechanistic wind
    /// kernel (Katul et al. 2005, Am. Nat. 166: 368–381) — the first-passage distance of a
    /// seed released at height `H`, carried at wind speed `U` and falling at `F` through
    /// turbulence — so its mean `H·U/F` is a physical "wind" knob of its own, per species,
    /// and its variance `μ³/λ` is finite, with a right tail that fattens as the wind
    /// rises. The Lomax shipped for stonecushion has its scale and shape as two
    /// layer-wide knobs in integer `hop`s (a mean of `2.5 · hop` columns cannot be 6) and
    /// infinite variance at its shape of 2. The exponential has no tail to speak of and
    /// the 2Dt is a phenomenological fit with no wind in it.
    Wald,
}

/// The shipped defence rise per bite (`defence_per_bite`). A browser at a crown bites on
/// 15-20 % of ticks (once per controller sample, `examples/defence_probe.rs`), 3-4 bites
/// a second, so continuous grazing moves the defence with a time constant of 80-110 s:
/// 0.8 within 2.5-3.5 min, where the bite keeps a fifth.
pub const DEFENCE_PER_BITE: f64 = 3e-3;
/// The shipped defence half-life once grazing stops: 20 min (`defence_half_life_s`),
/// "relaxes over tens of minutes".
pub const DEFENCE_HALF_LIFE_S: f64 = 1200.0;

/// [`SpeciesConfig::night_bloom_scale`]'s default: 0.3 of the day rate (1b's candidate).
fn default_night_bloom_scale() -> f64 {
    0.3
}

/// [`SpeciesConfig::night_floor`]'s default: a fifth of the reserve cap kept for dawn.
fn default_night_floor() -> f64 {
    0.2
}

/// [`SpeciesConfig::night_min_s`]'s default: ten minutes.
fn default_night_min_s() -> f64 {
    600.0
}

/// [`SpeciesConfig::establish_light_margin`]'s default: the break-even itself.
fn default_establish_light_margin() -> f64 {
    1.0
}

/// [`SpeciesConfig::runner_support_s`]'s default: an hour.
fn default_runner_support_s() -> f64 {
    3600.0
}

/// [`SpeciesConfig::runner_support_share`]'s default: the whole shortfall.
fn default_runner_support_share() -> f64 {
    1.0
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
    /// Mycelium in a log is not a root in soil and has no reason to share the root box's
    /// downward asymmetry, so substrate access has its own **symmetric** reach: a spore
    /// landing one voxel above or below a log still has the wood in its box. The soil-water
    /// box is separate and reaches only down.
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
    /// second — one recipient's worth, not one per neighbour.
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
    ///    and heat (not `c_g · gross`, which is larger) — and the remaining `1 / (1 + build)` of it is saved
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
    /// **Spores come from the caps** (the capgnawer package, 2026-10-05): the share of
    /// [`SpeciesConfig::propagule_rate`] that follows the stand's **fullness** — its
    /// foliage (a fungus's cap tissue) over the full foliage its wood carries,
    /// `min(P / (alpha · W), 1)`. A donor asks `propagule_rate · (1 − s + s · fullness)`,
    /// so at 1 a stand grazed to half its caps saves spores at half the rate and a capless
    /// one at none: the fruiting body is the reproductive organ, and a grazer that eats
    /// caps eats recruitment. 0 (the base, every plant) leaves the rate alone, fraction.
    #[serde(default)]
    pub propagule_cap_share: f64,
    /// `w_frac, p_frac, q_frac`: how a landed propagule splits into wood, starter foliage
    /// and starter reserve. Sums to one.
    pub propagule_split: [f64; 3],
    /// `e_v`: energy per unit of organic matter in every plant tissue. Light is the source.
    pub energy_density: f64,
    /// Per-seed death rate in the bank, per second: at each of its site's checks a seed
    /// survives the time since the last one with probability `exp(−rate · t)`, and a dead
    /// seed is litter with its mineral — whole seeds, paid decay, never deletion.
    /// Every preset sets it to `1 / seed_max_age_s`, an e-folding time equal to the
    /// lifetime: about a third of a cohort that never finds a gap is still there to go to
    /// litter at its age limit, and attrition has taken the rest along the way.
    /// **Placeholder**.
    pub seed_attrition_per_s: f64,
    /// **Dormancy.** A bin whose start is older than this many seconds falls to litter
    /// whole, with its mineral and its energy, at its site's next check. Hours, not
    /// minutes: the base is 8 h, a species' own preset says why it differs.
    /// **Placeholder**.
    pub seed_max_age_s: f64,
    /// How many arrival bins one species' bank is divided into: the bin width is
    /// `seed_max_age_s / seed_cohorts_max`, at least one tick, and a landing joins the bin
    /// whose window covers the current tick. So this is the **age resolution** of a bank,
    /// and the bound on its size follows from it: a bin's age never decreases and a bin
    /// past `seed_max_age_s` leaves whole, so a site holds at most `seed_cohorts_max + 1`
    /// bins of one species however a donor comes and goes. **Placeholder** — 4 is "a few",
    /// which is what [`Ground::seeds`] always claimed.
    ///
    /// Fixed bins bound the count without letting small fresh arrivals keep old material
    /// alive and without sweeping old deposits into one bucket that expires young
    /// material with it.
    pub seed_cohorts_max: usize,
    /// `n_tissue`: mineral nutrient per unit of organic matter this species builds. Wood,
    /// foliage and reserve share it this round. Growing `ΔO` draws `n_tissue · ΔO` from
    /// the site's mineral pool, and the pool caps income through `mineral / n_tissue`.
    /// **Placeholder**; nothing here is tuned.
    ///
    /// Which of the three nutrient limits actually binds, at the placeholders: the stock cap `N / n_tissue` is `50 · N` of income per tick, the
    /// `nutrient_draw_max` rate cap is `f_max · N · dt` = `0.0005 · N`, and the
    /// Michaelis–Menten factor is `N / (N + 0.5)`. The **rate cap is five orders of
    /// magnitude tighter**, so the stock cap cannot be the binding one for any positive
    /// `N`: `n_tissue` sets the stoichiometry of the draw and the density of the tissue,
    /// and not the ceiling on income.
    ///
    /// For a [`Trophic::Saprotroph`] it **is** a ceiling on growth: a fungus's
    /// income carries its own mineral out of the log, and what it may build in a tick is
    /// `(arriving mineral + the site's pool) / n_tissue`. Unfunded income is respired where
    /// every other unspent unit is. `step`'s §4.4a is the rule.
    pub n_tissue: f64,
    /// `q_share`: share of every tick's surplus that goes to the reserve first.
    pub reserve_share: f64,
    /// `p_reflush`: reserve is spent on foliage only while `P < p_reflush · P_cap`.
    pub reflush_below: f64,
    /// A stand is a **seedling** while its wood is below this multiple of `alive_min`:
    /// its reflush answers to the leaf its seed gave it rather than to `alpha · W`
    /// ([`SpeciesConfig::reflush_cap`]), so the adult foliage capacity never decides
    /// whether a newborn spends its seed reserve on leaves. Multiple of `alive_min`; at 1
    /// or below no living stand is a seedling. **Placeholder**: 2, a seedling until it has
    /// doubled its seed's wood.
    pub seedling_below: f64,
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
    /// **Available water** ([`available_water`]) at and below which `μ = 0` (wilting), and
    /// at and above which `μ = 1`; linear between ([`SpeciesConfig::moisture_at`]). Read as
    /// the root box's mean available water, each cell on its own material's wilting point
    /// and field capacity.
    ///
    /// The scale is `0` at the wilting point, `1` at field capacity — what drained ground
    /// holds — and `(1 − wp)/(fc − wp)` at saturation. So an upland plant is comfortable
    /// through the top half of what drained ground offers (`full_water` 0.5), and only a
    /// wetland plant needs ground wetter than drained (`wilt_water` 1.0).
    pub wilt_water: f64,
    pub full_water: f64,
    /// Water transpired per second per unit foliage at `μ = 1`, cubic metres. Withdrawn
    /// from the root box through `Command::WithdrawPore`, never by reading `pore` twice,
    /// and never below a cell's wilting point: water under it is not the plant's to take.
    pub transpiration_m3_per_s: f64,
    /// A seed cohort may germinate only on a site whose root box's mean **available
    /// water** is at least this — the same scale as `wilt_water` and `full_water` — and
    /// whose sky visibility is at least `establish_light_min`.
    pub establish_water_min: f64,
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
    /// That double duty is an **assumption**, stated as one: "can germinate
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
    /// **Settled standing water**, metres, the species needs on its own support face or
    /// on a 4-neighbouring one to establish and to stay healthy. Read with [`cubarium_voxel::VoxelView::standing_depth_m`], which does
    /// not count water in transit, and not with `water_depth_m`, which does. `0.0` is no
    /// requirement, and is what every species but the siphonreed carries.
    ///
    /// A neighbour counts when its face is at or below this site's and its standing water
    /// reaches up to this site's support voxel — water *beside* the site, not a pool at
    /// the foot of a cliff (`step`'s `standing_water_beside`). A stand whose water has
    /// gone reads `μ = 0`: it earns nothing, pays its upkeep out of reserve and then
    /// diebacks, which is what "stays healthy only there" means in this model.
    pub water_depth_min_m: f64,
    /// **The fall**: at death this species' wood is laid
    /// as dead wood along a line of `round(crown_radius / voxel)` sites in a direction
    /// hashed from its site, each an equal share (`step`'s `fall_line`), so the log a
    /// glowcap colonises is as long as the crown was wide. `false` — the wood falls on
    /// the site — for every species but the vaulttree.
    pub falls: bool,
    /// How many support sites away, in `x` and `z`, a propagule may land: the radius of a
    /// [`Dispersal::Drop`], the kernel scale of a [`Dispersal::Wind`], and the wide radius a
    /// [`Dispersal::Spores`] or [`Dispersal::Water`] seed searches. In reference 0.25 m
    /// voxels, scaled by [`FloraConfig::for_voxel_size`].
    pub hop: u32,
    /// How this species' seeds travel ([`Dispersal`]). [`Dispersal::Drop`] at the base.
    pub dispersal: Dispersal,
    /// The share of a donor's whole packages that leave as a **runner or rhizome** — a
    /// daughter stand on a free neighbouring face within a voxel of the parent's height
    /// that passes the gates, built from the package, nothing banked — instead of as a
    /// seed. Drawn per package from the donor's keyed stream; a runner with nowhere to go
    /// leaves as a seed. `0..=1`, zero at the base. **Placeholder**.
    pub clonal_share: f64,
    /// A [`Dispersal::Spores`] package's tries at a face whose ground passes before the
    /// rain falls as litter. 8 at
    /// the base — under a tenth of the rain lost where a quarter of the ground passes —
    /// and each try is one gate reading. Inert on every other mode. At least 1.
    pub spore_tries: u32,
    /// **Seed mass**, in minimum packages: one seed is
    /// `seed_mass · alive_min / w_frac` of organic matter, the parent pays for all of it,
    /// and the seedling it starts is that much bigger (wood `w_frac` of the seed, not the
    /// bare `alive_min`). The gap lottery weighs **count × mass**, so a bank weighs what
    /// it cost: a shrub or tree carries fewer, bigger seeds at the same weight per unit
    /// of material, and its seedling starts ahead. 1 — the minimum package — at the base
    /// and for the ground covers. At least 1.
    pub seed_mass: u32,
    /// **A ground cover**: a stand a seedling with a positive
    /// [`SpeciesConfig::overtop_per_check`] can come up through. The role, not a name:
    /// springturf, velvetpad and stonecushion carry it.
    pub ground_cover: bool,
    /// **Overtopping**: the chance, per check of its site, that this species' banked seed
    /// comes up through a [`SpeciesConfig::ground_cover`] holding the site — one its
    /// [`SpeciesConfig::overtop_covers`] allows. The site's gates must pass for it as if
    /// the site were free, its bank must win the gap lottery among the banks allowed
    /// there, and then this draw; on a success the cover dies an *overtopped* death
    /// ([`FloraLedger::overtopped`]) and the seedling establishes as usual. Zero —
    /// cannot — at the base; `0..=1`. The woody species carry it at 0.1; a ground cover
    /// may carry it too, restricted by `overtop_covers` (plant succession, 2026-10-03:
    /// springturf through velvetpad).
    pub overtop_per_check: f64,
    /// **Which ground covers** this species' seedlings can come up through, by species
    /// name (`overtop_covers = ["velvetpad"]`). **Empty means any ground cover** — the
    /// woody species' rule. Inert while `overtop_per_check` is zero. The occupant is read
    /// when the site is armed, again when the seedling sprouts, and once more at the
    /// replacement itself, so an arm never outlives a change to a cover it may not take.
    #[serde(with = "species_names")]
    pub overtop_covers: Vec<Species>,
    /// Crown geometry from wood, shared by the shade model and the presenter so what
    /// shades is exactly what is drawn: crown top above the support face and crown
    /// half-width, both **in metres**, `[min, max]`. They are physical sizes
    /// and are **not** rescaled per voxel size: every grid grows the same plant, and
    /// the voxel readings ([`SpeciesConfig::crown_height`]) divide by the world's cell.
    /// How wood moves a stand between the two ends is [`SpeciesConfig::crown_height_m_at`].
    pub crown_height_m: [f64; 2],
    pub crown_radius_m: [f64; 2],
    /// **The grazing refuge**: the share of the foliage its wood carries that no bite can take — basal
    /// buds, tissue pressed to the ground, leaves too tough or too few to be worth a mouth.
    /// Each foliage layer keeps `graze_refuge ×` its own capacity (`share · alpha · wood`),
    /// so the floors sum to `graze_refuge · alpha · wood`; [`StandLayer::edible`] is what
    /// stands above it, and it is the one reading every consumer takes and the one
    /// withdrawals are bounded by. A floor is not a height refuge: the lanternberry's browse
    /// line and the tall crowns keep theirs in their profiles, on top of this.
    ///
    /// **0.15** in the base, for every species the brief does not name: enough stock left
    /// on a stripped plant to refoliate from without making it inedible. Springturf and
    /// velvetpad **0.3** (their growth points sit at the base, under a mouth), stonecushion
    /// **0.4** (a cushion is mostly skirt pressed to rock). Glowcap keeps the base 0.15: its
    /// income is substrate, not cap, so its refuge only protects the tissue that fruits
    /// again, and the shredder has litter to fall back on.
    pub graze_refuge: f64,
    /// **Induced grazing defence**, the rise: every bite that takes anything moves the
    /// stand's defence this fraction of the way to 1
    /// (`design/handoffs/voxel-lifespan-defence-2026-09-25.md`, B). A bite takes
    /// `(1 − defence)` of what was asked, so a crown grazed without pause gives less and
    /// less. Zero is no defence. The same for every species at the defaults.
    pub defence_per_bite: f64,
    /// The defence's **half-life** once grazing stops, seconds. Zero or less: no decay.
    pub defence_half_life_s: f64,
    /// **Nectar** (F1, predation-and-flight §3.7, decision 6): organic matter an adult
    /// stand moves from its reserve into its [`Stand::nectar`] stock per second, in
    /// daylight only, until the stock reaches [`SpeciesConfig::nectar_cap`]. Zero makes
    /// none: every species but bloomcrown. "Adult" is past the seedling profile
    /// (`profile_stage >= 1`); "daylight" is the world's `life_daylight` above zero.
    pub nectar_per_s: f64,
    /// The most nectar an adult stand holds, organic units. Unvisited nectar stays.
    pub nectar_cap: f64,
    /// The **mineral** a unit of nectar carries, a fraction of its organic matter: pollen
    /// lumped in until pollen exists (§3.7). Small but not zero, because the animals'
    /// assimilation rule `min(yield · organic, mineral / n_tissue)` would starve a pure
    /// sugar eater. Drawn from the stand's own mineral when the nectar is taken (bounded
    /// by what it holds). Latticevine nectar does not read this: it is mineral-free.
    #[serde(default)]
    pub nectar_mineral_fraction: f64,
    /// **Fruit** (F3, the roster brief's Brief F3; predation-and-flight §3.7): organic
    /// matter an adult stand moves from its reserve into its [`Stand::fruit`] stock per
    /// second, in daylight only, until the stock reaches [`SpeciesConfig::fruit_cap`].
    /// Zero makes none: every species but lanternberry. "Adult" is past the seedling
    /// profile (`profile_stage >= 1`); "daylight" is the world's `life_daylight` above zero.
    #[serde(default)]
    pub fruit_per_s: f64,
    /// The most fruit an adult stand holds, organic units.
    #[serde(default)]
    pub fruit_cap: f64,
    /// Ripe fruit left uneaten this long, seconds, falls to the litter at the stand's site.
    #[serde(default)]
    pub fruit_drop_s: f64,
    /// **The mean rest after an uneaten crop falls**, seconds (plants-flowering-vines
    /// 2026-10-03): the stand hangs no fruit for a random time, exponential with this
    /// mean, keyed by the stand and the tick, so the bushes of a grove fruit out of step
    /// and some stand bare. A crop eaten out refills at once, so what a feeder can take is
    /// unchanged. 0: fruits again at once.
    #[serde(default)]
    pub fruit_rest_s: f64,
    /// The **mineral** a unit of fruit carries, a fraction of its organic matter: small
    /// but not zero (energy-rich fruit). Drawn from the stand's own mineral when taken.
    #[serde(default)]
    pub fruit_mineral_fraction: f64,
    /// **The night budget** (`design/ecology-timescales-2026-10-02.md` 1b): while the sun
    /// is down on the ecological clock, a photosynthetic stand's reserve funds the leaf
    /// reflush (below [`SpeciesConfig::reflush_below`]), then nectar, then fruit, out of
    /// one spendable budget per second,
    /// `max(0, reserve − maintenance to dawn − night_floor · reserve cap) / max(time to
    /// dawn, night_min_s)`. This is nectar and fruit production at night, a fraction of
    /// the day rate ([`SpeciesConfig::nectar_per_s`], [`SpeciesConfig::fruit_per_s`]).
    /// 0.3, a tunable candidate.
    #[serde(default = "default_night_bloom_scale")]
    pub night_bloom_scale: f64,
    /// The night budget's **dawn floor**: the reserve a stand keeps for dawn, a fraction of
    /// its reserve cap (`reserve_cap · W`). Nothing at night is spent from below it.
    #[serde(default = "default_night_floor")]
    pub night_floor: f64,
    /// The night budget's **minimum time to dawn**, seconds: the budget is spread over at
    /// least this long, so it stays bounded as dawn nears.
    #[serde(default = "default_night_min_s")]
    pub night_min_s: f64,
    /// **A runner's reach**: how many columns sideways, in `x` and `z`, a runner or rhizome
    /// daughter may root from the parent (the square ring around the home column); the
    /// support face is then the one at the parent's height or a voxel above or below. A
    /// count of voxels, **not** rescaled with the voxel size. Only read when
    /// `clonal_share > 0`.
    pub runner_reach: u32,
    /// **Runners root through covers** (springturf spread, 2026-10-04): a runner may root on
    /// a face held by a [`SpeciesConfig::ground_cover`] its [`SpeciesConfig::overtop_covers`]
    /// allows (empty: any ground cover) as well as on a free one, when this species' own
    /// gates pass there. The daughter is the parent's paid package, as on a free face, and
    /// the cover it displaces dies the ordinary death, booked as an overtopped one
    /// ([`FloraLedger::overtopped`], [`FloraLedger::runner_displaced`]). `false` at the
    /// base: runners take free faces only. Not a number, so its registry row is a switch.
    pub runners_overtop: bool,
    /// **The canopy gate's margin** (turf-establish, 2026-10-05): a photosynthetic seed
    /// comes up, and a runner roots, only where its newborn's income at full daylight —
    /// under the light that reaches its own layers through every other stand's taller
    /// foliage (the light model's own sweep), at the site's moisture and mineral — is at
    /// least this multiple of its upkeep (maintenance plus the construction of replacing
    /// its senescing leaves): the break-even of the stand's own carbon budget, read off its
    /// own config ([`crate::step::break_even_cover`]). Below it a seed stays in the bank,
    /// waiting for a gap, and a runner tries another face or stays in its parent's
    /// parcel. A multiple of the upkeep; 0 opens the gate. Saprotrophs have no light gate
    /// and never read it. **Placeholder**: 1, the break-even itself.
    #[serde(default = "default_establish_light_margin")]
    pub establish_light_margin: f64,
    /// **How long a runner daughter is fed by its parent**, seconds: from its birth the
    /// parent pays [`SpeciesConfig::runner_support_share`] of the daughter's shortfall
    /// (its upkeep less its own income, each tick) out of the parent's reserve above its
    /// donor floor, until this runs out or the daughter's own income pays its upkeep. 0: no
    /// support. Photosynthetic species only. **Placeholder**: an hour.
    #[serde(default = "default_runner_support_s")]
    pub runner_support_s: f64,
    /// **The share of a supported runner daughter's shortfall its parent pays**, each
    /// tick ([`SpeciesConfig::runner_support_s`]). Fraction; 1 pays all of it.
    /// **Placeholder**: 1.
    #[serde(default = "default_runner_support_share")]
    pub runner_support_share: f64,
    /// **The WALD wind kernel's mean distance**, metres ([`Dispersal::Wald`]). The
    /// inverse-Gaussian `μ`, which in the mechanistic WALD model (Katul et al. 2005) is
    /// `H · U / F` — release height times wind speed over the seed's terminal velocity —
    /// so this is the "wind" knob: more wind is a longer mean. In metres and not
    /// rescaled with the voxel size. Inert on every other mode.
    pub wald_mean_m: f64,
    /// **The WALD kernel's shape** `λ`, metres: `(H / σ)²` in the mechanistic model, the
    /// release height against the turbulent spread. It does not change with the wind, so a
    /// stronger wind (a larger mean) also fattens the tail relatively: the coefficient of
    /// variation is `√(μ / λ)`. Inert on every other mode.
    pub wald_shape_m: f64,
    /// **Burrs** (epizoochory): the chance, per contact, that a donor stand's seed latches
    /// onto an animal of coat factor 1 that steps onto its face; scaled by the animal's
    /// own coat factor (`cubarium_voxel_fauna`'s `burr_coat`). The seed is one whole
    /// package paid out of the donor's propagule budget — its parcel, then its reserve
    /// above `donor_reserve_floor`, at the construction cost a parcel pays — so a donor
    /// that cannot fund a package sheds none. 0, no burrs, at the base.
    pub burr_pickup: f64,
    /// **Burr retention: mean distance**, metres the carrier walks before the seed falls
    /// off. The retention clock runs in **distance moved**, not time: what pulls a burr
    /// off is the coat brushing through vegetation and the animal grooming as it goes, so a
    /// resting animal does not shed (Mouissie et al. 2005 measured adhesive retention
    /// against distance walked). Metres, not rescaled.
    pub burr_retention_mean_m: f64,
    /// **Burr retention: shape** `k` of the Weibull ("power exponential", the stretched
    /// exponential) retention curve `S(d) = exp(−(d / scale)^k)`, scale set by the mean.
    /// `k = 1` is the constant-detachment exponential; `k < 1` a detachment rate that
    /// falls the longer a seed has held on. Bullock et al. 2011 (Oikos 120: 1201–1208), over
    /// 52 retention data sets on sheep, cattle, deer, goats and mice, found the exponential
    /// adequate for short records and this power exponential, with the dropping rate
    /// decreasing in time, best for long ones, and slower dropping for seeds with
    /// adhesive structures. Dimensionless, positive.
    pub burr_retention_shape: f64,
    /// **Curing** (drought dormancy): a stand whose root box's mean **available water**
    /// stays below this for [`SpeciesConfig::cure_after_s`] cures — drops
    /// [`SpeciesConfig::cure_shed`] of its foliage to the litter and goes dormant. On the
    /// available-water scale (0 at the wilting point, 1 at field capacity).
    pub cure_water: f64,
    /// Seconds below [`SpeciesConfig::cure_water`] before a stand cures. **0: never cures**
    /// (every species but springturf).
    pub cure_after_s: f64,
    /// The share of its foliage a curing stand drops to the litter at once (booked as
    /// shed foliage). The rest stands as straw: no photosynthesis, no senescence.
    pub cure_shed: f64,
    /// A cured stand's maintenance, as a fraction of its waking rate: the root crowns
    /// alive on the reserve. Its wood still diebacks on maintenance it cannot pay.
    pub cure_upkeep: f64,
    /// Available water at and above which a cured stand wakes and regrows its foliage from
    /// its reserve (the ordinary reflush). Above `cure_water`: a passing shower is not
    /// the end of a drought.
    pub cure_resume_water: f64,
    /// Seconds a stand may stay cured before the drought kills it. 0: no limit (it dies
    /// only when its reserve runs out and its wood diebacks).
    pub cure_max_s: f64,
    /// **Look, not ecology**: the sRGB colour the plain-voxel draw gives a cured stand's
    /// crown (`0x00RRGGBB`). Wrysk judges it; 0 keeps the species' own palette.
    pub cured_srgb: u32,
    /// **Founding** (the habitat seeder, `cubarium-voxel-sim`): founders per square metre
    /// of habitat this species can actually live in (the faces where its own gates pass).
    /// Zero founds none of this species.
    pub founder_per_m2: f64,
    /// **Founding**: the size of one founder colony, in stands; colonies grow out from
    /// their own anchor. At least 1.
    pub founder_colony: u32,
    /// **Founding**: the cap a world puts on founders of this species, in stands.
    pub founder_cap: u32,
    /// **The dark habitat** (`design/handoffs/plants-glowcap-vine-2026-10-01.md` B): terrain
    /// sky visibility ([`cubarium_voxel::VoxelView::sky_visibility`], no canopy in it) at and
    /// above which a site is not dark at all. Darkness is
    /// `clamp((dark_sky_max − sky) / (dark_sky_max − dark_sky_full), 0, 1)`
    /// ([`SpeciesConfig::darkness`]). **0 turns the dark habitat off**: every site reads
    /// darkness 0 and every `dark_*` knob below is inert. Off for every species but the
    /// glowcap.
    pub dark_sky_max: f64,
    /// Sky visibility at and below which a site is **fully** dark (darkness 1). At most
    /// `dark_sky_max`.
    pub dark_sky_full: f64,
    /// **Seep income**: organic matter per unit of mycelium (wood) per second at darkness 1
    /// and `μ` 1 — drip water carrying dissolved organic matter down a cave wall. It adds to
    /// the substrate income, is capped at what the stand can spend this tick, brings its
    /// own mineral at `n_tissue`, and enters the ledger as `FloraLedger::seep_*_in`.
    /// Organic/s per unit wood; 0 is none.
    pub dark_uptake_per_s: f64,
    /// **Cave damp**, on the available-water scale (1 = field capacity): a dark site reads
    /// at least `dark_damp · darkness` of available water, in the germination gates and in
    /// `μ`, even with no soil in its root box — cave air is saturated and rock weeps. It
    /// creates no water: it is a reading, and a species with no transpiration draws
    /// nothing. 0 is none.
    pub dark_damp: f64,
    /// Darkness at and above which the **substrate gate opens** without dead wood or
    /// litter. 0 is off (the gate needs substrate as before).
    pub dark_establish_min: f64,
    /// **Spore landings favour the dark**: a candidate face weighs `1 + dark_spore_bias ·
    /// darkness` among a package's tries, and a dark face under a roof within `hop`
    /// rows of the donor weighs `dark_spore_bias · darkness` (`seeds`' `spore_target`).
    /// 0 gives the undarkened picks exactly. Dimensionless.
    pub dark_spore_bias: f64,
    /// **Founding**: the share of this species' founders the habitat seeder places on the
    /// darkest faces that pass its gates with no substrate laid (no log under them). 0 is
    /// none: every founder is founded as before. A fraction.
    pub dark_founder_share: f64,
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

    /// The foliage the **reflush** answers to at `wood` (`step` §4.4: reserve is spent on
    /// foliage only while `P < reflush_below · reflush_cap`, and up to that level).
    ///
    /// A stand past its seedling size — wood at least `seedling_below · alive_min` —
    /// answers to its foliage cap `alpha · W`, so a grazed or cured stand regrows from its
    /// reserve as it always has. A **seedling** answers to the leaf its seed gave it: the
    /// newborn's foliage per unit wood, `propagule_split[1] / propagule_split[0]`, times its
    /// wood, and never more than `alpha · W`. A seedling's reserve is its seed's, and
    /// whether it spends it on leaves is set by the seed it came from, not by how much
    /// foliage the species carries when grown: at an `alpha` that put the threshold above
    /// the newborn's own leaf, every newborn spent its reserve within seconds and died at
    /// `alive_min` on its first unpaid upkeep (`turf_trace`, 2026-10-04).
    pub fn reflush_cap(&self, wood: f64) -> f64 {
        let cap = self.alpha * wood.max(0.0);
        if wood >= self.seedling_below * self.alive_min {
            return cap;
        }
        let [w_frac, p_frac, _] = self.propagule_split;
        if w_frac > 0.0 {
            (p_frac.max(0.0) / w_frac * wood.max(0.0)).min(cap)
        } else {
            cap
        }
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

    /// The one minimum propagule package a donor saves before it sends one (the
    /// tick's own `package_of`). Read-only: the presenter shows a stand whose `parcel`
    /// holds half of this as ripe (`cubarium::voxel::model`).
    pub fn propagule_package(&self) -> f64 {
        seeds::package_of(self)
    }

    /// One crown dimension in metres at `wood`, from its `[min, max]` range.
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

/// Water: what the ground offers this species.
impl SpeciesConfig {
    /// How dark a site with terrain sky visibility `sky` is for this species, `0..=1`:
    /// 0 at and above `dark_sky_max`, 1 at and below `dark_sky_full`, linear between.
    /// Always 0 when `dark_sky_max` is 0 (no dark habitat).
    pub fn darkness(&self, sky: f64) -> f64 {
        if !(self.dark_sky_max > 0.0) {
            return 0.0;
        }
        1.0 - crate::step::ramp(sky, self.dark_sky_full, self.dark_sky_max)
    }

    /// The available water a root box reads once the cave damp is in it: at least
    /// `dark_damp · darkness`, even for a box with no soil (`mean` `None`). Unchanged —
    /// the same `Option`, to the bit — where there is no damp.
    pub fn damp_available(&self, mean: Option<f64>, darkness: f64) -> Option<f64> {
        let damp = self.dark_damp * darkness;
        if damp > 0.0 {
            Some(mean.map_or(damp, |m| m.max(damp)))
        } else {
            mean
        }
    }

    /// `μ` at root-box available water `a`: zero at and below `wilt_water`, one at and
    /// above `full_water`, linear between. A `full_water` at or below `wilt_water` is a
    /// step at `full_water`.
    pub fn moisture_at(&self, available: f64) -> f64 {
        crate::step::ramp(available, self.wilt_water, self.full_water)
    }

    /// `establish_water_min` as a pore fraction of `material` under the world's
    /// `materials`: `wp + a · (fc − wp)`. For a caller that thinks in pore fractions — the
    /// host's water-cycle report bands soil columns by it — and not for the gate itself,
    /// which reads available water.
    pub fn establish_pore_min_on(
        &self,
        materials: &cubarium_voxel::Materials,
        material: Material,
    ) -> f64 {
        let (wp, fc) = (
            materials.wilting_point(material),
            materials.field_capacity(material),
        );
        wp + self.establish_water_min * (fc - wp)
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
            propagule_cap_share: 0.0,
            propagule_split: [0.4, 0.4, 0.2],
            energy_density: 2.0,
            // Eight hours, e-folding at the lifetime (the field docs): the middle of the
            // brief's 6–12 h for seeds; each preset below says why it moves.
            seed_attrition_per_s: 1.0 / 28_800.0,
            seed_max_age_s: 28_800.0,
            seed_cohorts_max: 4,
            n_tissue: 0.02,
            reserve_share: 0.2,
            reflush_below: 0.25,
            seedling_below: 2.0,
            assimilation: 0.004,
            nutrient_half: 0.5,
            nutrient_draw_max: 0.01,
            light_half: 0.5,
            rooting_depth: 2,
            rooting_radius: 1,
            // Package F's upland scale (available water, 1 = field capacity): wilts only
            // in the bottom tenth of what drained ground offers, full through its top
            // half, germinates on a fifth of it.
            wilt_water: 0.1,
            full_water: 0.5,
            // Package F measurement (a v1_base number: proposed, Fable's call). At 2e-5 a
            // founder drew its root box from field capacity to the wilting point in
            // 10–30 min, while a default-preset shower (0.4 m³ over 576 m², ~0.7 mm)
            // gives a 3×3 root footprint ~1/20 of that demand back: bloomcrown,
            // vaulttree and lanternberry starved in full light at μ ≈ 0.1 on either
            // soil. At 1e-6 a full-grown bloomcrown (P 1.2) asks ~2× the mean rain on
            // its footprint, so boxes dry between showers and under dense cover only,
            // and every upland species holds its donors for 3 h (nofauna census).
            transpiration_m3_per_s: 1e-6,
            establish_water_min: 0.2,
            establish_light_min: 0.3,
            saturated_pore: 0.95,
            stress_rate_per_s: 0.05,
            relax_rate_per_s: 0.05,
            establish_saturated_max: 0.5,
            drown_depth_m: 0.0,
            water_depth_min_m: 0.0,
            falls: false,
            hop: 1,
            dispersal: Dispersal::Drop,
            clonal_share: 0.0,
            spore_tries: 8,
            seed_mass: 1,
            ground_cover: false,
            overtop_per_check: 0.0,
            overtop_covers: Vec::new(),
            crown_height_m: [0.25, 0.75],
            crown_radius_m: [0.125, 0.375],
            // The refuge a plant the brief does not name keeps (`graze_refuge`'s doc).
            graze_refuge: 0.15,
            defence_per_bite: DEFENCE_PER_BITE,
            defence_half_life_s: DEFENCE_HALF_LIFE_S,
            // F1: no species but bloomcrown makes nectar ([`SpeciesConfig::bloomcrown`]).
            nectar_per_s: 0.0,
            nectar_cap: 0.0,
            nectar_mineral_fraction: 0.0,
            // F3: no species but lanternberry makes fruit ([`SpeciesConfig::lanternberry`]).
            fruit_per_s: 0.0,
            fruit_cap: 0.0,
            fruit_drop_s: 0.0,
            fruit_rest_s: 0.0,
            fruit_mineral_fraction: 0.0,
            // 1b: the night budget's knobs, plausible defaults (not tuned).
            night_bloom_scale: default_night_bloom_scale(),
            night_floor: default_night_floor(),
            night_min_s: default_night_min_s(),
            // A runner roots on the next column over (the eight neighbours).
            runner_reach: 1,
            runners_overtop: false,
            // turf-establish: the canopy gate at break-even, and a runner fed for an hour.
            establish_light_margin: default_establish_light_margin(),
            runner_support_s: default_runner_support_s(),
            runner_support_share: default_runner_support_share(),
            // No WALD kernel, no burrs, no curing: springturf's preset says why it has them.
            wald_mean_m: 0.0,
            wald_shape_m: 0.0,
            burr_pickup: 0.0,
            burr_retention_mean_m: 0.0,
            burr_retention_shape: 1.0,
            cure_water: 0.0,
            cure_after_s: 0.0,
            cure_shed: 0.0,
            cure_upkeep: 1.0,
            cure_resume_water: 0.0,
            cure_max_s: 0.0,
            cured_srgb: 0,
            // No founders: every preset that is seeded says so.
            founder_per_m2: 0.0,
            founder_colony: 1,
            founder_cap: 0,
            // No dark habitat: only the glowcap has one.
            dark_sky_max: 0.0,
            dark_sky_full: 0.0,
            dark_uptake_per_s: 0.0,
            dark_damp: 0.0,
            dark_establish_min: 0.0,
            dark_spore_bias: 0.0,
            dark_founder_share: 0.0,
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
            founder_per_m2: 0.25,
            founder_colony: 3,
            founder_cap: 64,
            light_half: 0.8,
            rooting_depth: 2,
            rooting_radius: 1,
            // Upland: the most drought-hardy of the three upland plants —
            // wilts only in the bottom twentieth of drained ground's water, full through
            // its top half, germinates on a tenth of it.
            wilt_water: 0.05,
            full_water: 0.5,
            establish_water_min: 0.1,
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
            // Seeds: dropped within `hop` 2 — animal vectors are a later
            // package — into the base's 8 h bank. A meadow forb's seed that waits a
            // working day for a gap, not a pioneer's.
            hop: 2,
            // Package SU, the woody roles. A seed of **2** minimum packages: a forb-shrub's
            // seed, each seedling twice the material. `propagule_rate` is scaled by the
            // same 2 (0.0002 → 0.0004), so seeds per unit time are what they were and the
            // parent invests twice as much in each: a large woody plant puts more absolute
            // mass into reproduction than a turf does. It comes up through a ground cover
            // at **0.1 per check**: a mean of 10 checks, 5 min under a mat, against at most
            // one check (30 s) in a bare gap — harder, and well inside even the shortest
            // woody bank (6 h). 0.02 (25 min) left the terrarium's woody share falling at
            // 2 h (Fable's tuning round).
            seed_mass: 2,
            overtop_per_check: 0.1,
            propagule_rate: 0.0004,
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
            // F1 (predation-and-flight §3.7): the flower's nectar, paid from the reserve in
            // daylight. 1e-4 a second is about 2 % of a full-grown stand's gross income at
            // full light (assimilation 0.004/s × foliage 1.2); the cap, 0.02, is 200 s of
            // it and under a tenth of the reserve cap (0.3), so an unvisited flower costs
            // the plant a bounded, small store. The mineral share 0.01 is half the
            // bellwing's tissue mineral (0.02): a sip funds half its organic matter.
            nectar_per_s: 1e-4,
            nectar_cap: 0.02,
            nectar_mineral_fraction: 0.01,
            ..SpeciesConfig::v1_base()
        }
    }

    pub fn umbrellafrond() -> SpeciesConfig {
        SpeciesConfig {
            founder_per_m2: 0.15,
            founder_colony: 3,
            founder_cap: 40,
            light_half: 0.15,
            rooting_depth: 4,
            rooting_radius: 1,
            // Wetland: needs ground wetter than drained. Wilts at field
            // capacity (1.0) and is full at 1.5: pore 0.84 on a loam of fc 0.65 / wp
            // 0.28, and short of saturation (≥ 1.67) everywhere in the soil-retention
            // window, so saturated ground is always full moisture. Germinates only
            // where the ground is clearly wetter than drained (1.2).
            wilt_water: 1.0,
            full_water: 1.5,
            establish_water_min: 1.2,
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
            // its own preset. Do **not** lower it slightly as a repair: for
            // *every* tolerance below 1, a wholly saturated root box targets stress 1 and
            // eventually takes all of the species' assimilation again, which is the
            // failure package I measured. A partial cost at full saturation is a different
            // rule — an explicit subunit maximum on the response, or a funded cost of
            // tolerance — and not a smaller number here.
            stress_rate_per_s: 0.01,
            relax_rate_per_s: 0.05,
            establish_saturated_max: 1.0,
            drown_depth_m: 0.5,
            // Seeds: **spores to wet ground** within a wide `hop` 6 (1.5 m at
            // the reference voxel) — the water gate is what "wet" means here — and
            // **rhizomes** for half the packages, which is how a frond fills the hollow it
            // is already in while the spores find the next one. Spores are short-lived:
            // 4 h, the top of the brief's 2–4 h, because a hollow can take a shower or two
            // to come back.
            hop: 6,
            dispersal: Dispersal::Spores,
            clonal_share: 0.5,
            seed_max_age_s: 14_400.0,
            seed_attrition_per_s: 1.0 / 14_400.0,
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
    /// bare moist ground and loses under a canopy" is **intended succession and untested**:
    /// the numbers below encode the sentence rather than evidencing it. The role as the
    /// numbers state it: shallow roots that only see the support row, a light need above
    /// bloomcrown's, a moisture floor at the fraction drained soil actually settles to, a
    /// fast cheap body that turns income into packages quickly, and a maintenance bill that
    /// kills it as soon as the light or the water goes.
    ///
    /// Every number is a **placeholder** (`design/backlog.md` §1), chosen to encode that
    /// sentence and nothing else:
    ///
    /// - **Water, the three thresholds together**, on the available-water scale
    ///   (0 at the wilting point, 1 at field capacity). `wilt_water` 0.1 and `full_water`
    ///   0.5 are the upland pair: drained ground is full moisture, and a turf only wilts
    ///   in the bottom tenth of what it offers. `wilt_water` is twice bloomcrown's 0.05,
    ///   so it is still the upland species that gives up first on a dry ridge.
    ///   `establish_water_min` 0.3 is the highest upland germination floor: a turf's
    ///   shallow roots see only the support row, so a seed waits for ground that holds a
    ///   third of drained soil's water there. That is **germination permission and not a
    ///   positive newborn budget**. `establish_saturated_max` 0.3 keeps it out of a waterlogged hollow, and
    ///   `drown_depth_m` 0.03 is under a voxel of pool but well over the millimetres of
    ///   transit water the harness's rain leaves on a face.
    /// - **Light.** `light_half` 1.0 and `establish_light_min` 0.75 are both above
    ///   bloomcrown's 0.8 and 0.6: it needs open sky to germinate and earns badly in
    ///   shade. Note that a *living crown* cannot shut that gate — germination light is
    ///   geometric sky visibility with no canopy in it (`step.rs`'s `Gates`) — so "loses
    ///   under a canopy" is an adult-income statement here and a terrain-shade statement
    ///   at germination. Making it a germination statement would be an explicit rule
    ///   addition, and this round does not make it.
    /// - **Fast and cheap.** `wood_max` 0.06 is a tenth of bloomcrown's and `alive_min`
    ///   0.006 with it, so one package is 0.015 rather than 0.05; `donor_min` 0.03 is half
    ///   its own `wood_max`, so it can reproduce at half size, where bloomcrown's 0.3 is
    ///   half of 0.6; `wood_rate` 0.01 and `foliage_rate` 0.02 are ten times the base, and
    ///   `propagule_rate` 0.002 ten times, so a funded donor fills a package in about nine
    ///   seconds against bloomcrown's three hundred. `assimilation` 0.008 is twice the
    ///   base, and the honest reason is **faster growth, not survival**: at
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
    ///   tall and `[0.125, 0.25]` m in radius: a mat on the ground
    ///   with no stem at all.
    ///
    /// **Succession redesign (2026-10-03, Wrysk: "a wild grass ... rolling hills of
    /// california ... drought resistant").** The numbers above are the earlier turf's
    /// where these replace them; the crown, profile and colours are unchanged (the look is
    /// Wrysk's). The presets carry the same relative changes (`alpha` 1.285 → 4,
    /// `foliage_rate` × 3, `senescence` 0.00156 → 0.0026, the bank × 2).
    /// - **Productive.** `wood_max` 0.3 (× 5); `alpha` was 6.2 here, 1.5 in the spread
    ///   package (below) and 3.15 since its alpha sweep: a grazed mature stand carries about
    ///   the original (alpha 4) velvetpad's edible foliage, about 1.9× today's.
    ///   `foliage_rate` 0.06 (× 3) and `graze_refuge` 0.4 (basal buds) regrow a cropped
    ///   sward fast. `senescence` 0.00167 (× 1.67 the base): heavy thatch.
    /// - **Drought-tolerant.** `wilt_water` 0 and `full_water` 0.3, `rooting_depth` 2: it
    ///   earns on any water its deeper box holds (the probe: an adult and a newborn live
    ///   through 55,000 s at available water 0.2, longer than the bank, and regrow).
    /// - **Not a bog plant.** `establish_saturated_max` 0.05: a saturated share of the
    ///   (deeper) box over a twentieth stresses it, and a half-saturated box takes its net
    ///   below zero (the viability grid), so soggy ground goes back to velvetpad.
    /// - **Perennial upkeep.** `maintenance` 0.0002, twice velvetpad's, replaces the
    ///   short-lived bullet above (2026-10-05; the measurement is where it is set).
    /// - **Seeds.** The bank lives 24 h (`seed_max_age_s` 86,400); a large adult funds
    ///   many packages. It comes up through velvetpad (`overtop_covers` velvetpad).
    ///
    /// **Spread (2026-10-04, `design/handoffs/springturf-spread-2026-10-04.md`).** Wind on
    /// the WALD kernel, burrs on passing animals, runners that root through velvetpad, and
    /// curing in drought; each number is stated where it is set.
    pub fn springturf() -> SpeciesConfig {
        SpeciesConfig {
            founder_per_m2: 0.6,
            founder_colony: 6,
            founder_cap: 160,
            light_half: 1.0,
            rooting_depth: 2,
            rooting_radius: 1,
            wilt_water: 0.0,
            full_water: 0.3,
            establish_water_min: 0.3,
            establish_light_min: 0.75,
            stress_rate_per_s: 0.2,
            relax_rate_per_s: 0.05,
            establish_saturated_max: 0.05,
            drown_depth_m: 0.03,
            // Seeds: dropped within the pioneer's wide `hop` 3, and **runners**
            // for 0.3 of the packages (package SU: 0.5 let the turf close every patch it
            // touched before anything else could seed in), so a turf still closes its own
            // patch while most of its seed goes looking. The longest bank, 12 h, the top
            // of the brief's 6–12 h: a pioneer's seed bank is the persistent one, waiting
            // under whatever replaced it for the next bare ground. A ground cover: a seedling
            // with a positive overtop (the woody species) can come up through it.
            hop: 3,
            ground_cover: true,
            overtop_covers: vec![Species::Velvetpad],
            // **Spread** (2026-10-04, Wrysk: "springturf not spreading is the problem"):
            // - comes up through velvetpad at the succession study's pick, 0.03 a check;
            dispersal: Dispersal::Wald,
            overtop_per_check: 0.03,
            // - wind: a WALD kernel with a mean of 3.0 m (24 voxels at 0.125 m, the long
            //   end of Astra's 6/12/24 arms: the spread rerun on `patch`, 8 seeds × 2 h,
            //   2026-10-04, put the most of the S1 cohort under turf at 24) and a shape of
            //   0.5 m, of the order of a grass head's release height: most seed lands within
            //   a few decimetres and the tail crosses the patch gaps (CV √6 at this mean);
            wald_mean_m: 3.0,
            wald_shape_m: 0.5,
            // - runners: a sward spreads by stolons and rhizomes, so 0.4 of the packages
            //   (was 0.3) root up to two columns out (25 cm at 0.125 m, a stolon's
            //   internode or two), and through velvetpad as well as onto bare ground;
            clonal_share: 0.4,
            runner_reach: 2,
            runners_overtop: true,
            // - burrs: an awned seed head catches a passing coat a tenth of the time at
            //   coat factor 1, and stays on for a mean 3 m walked (a Weibull of shape 0.7:
            //   most fall off in the first metre, the hooked ones ride far).
            burr_pickup: 0.1,
            burr_retention_mean_m: 3.0,
            burr_retention_shape: 0.7,
            // **Curing** (Wrysk: "grass can turn brown when its dried out and produce a lot
            // of litter ... then it regrows from reserves in the roots"): below available
            // water 0.1 — where the drought probe's turf began to die — for half an hour,
            // it drops 0.8 of its foliage to the litter and goes dormant on a tenth of its
            // maintenance; it wakes at 0.25 and reflushes from the reserve; six hours
            // dormant (three days of the 2 h clock) kills it. Drawn straw brown.
            cure_water: 0.1,
            cure_after_s: 1800.0,
            cure_shed: 0.8,
            cure_upkeep: 0.1,
            cure_resume_water: 0.25,
            cure_max_s: 21_600.0,
            cured_srgb: 0x00C2_A15A,
            seed_max_age_s: 86_400.0,
            seed_attrition_per_s: 1.0 / 86_400.0,
            wood_max: 0.3,
            alive_min: 0.006,
            donor_min: 0.03,
            // Upkeep (2026-10-05, Wrysk: "why is grass upkeep so much higher?"): twice
            // velvetpad's 0.0001, the base. The earlier 0.002 (ten times the base) belonged
            // to the short-lived pioneer turf above, which was meant to die as soon as its
            // light or water went; a perennial grass that cures and regrows from its roots
            // can't burn its reserve in 250 s. On `patch` (8 seeds, 4 h from m120) the old
            // upkeep shrank mature turf 500 → 392 and turf on drained ground 616 → 405;
            // at twice velvetpad's, both held (553, 612). The presets carry the same 2×.
            maintenance: 0.0002,
            // Production (2026-10-04, Wrysk: "grass should have more foliage than moss"):
            // a mature stand carries `(1 − graze_refuge) · alpha · wood_max` above its
            // floor, 0.6 · 3.15 · 0.3 = 0.57 against velvetpad's 0.7 · 1.0 · 0.2 = 0.14 on
            // paper (the presets: 6.3 against 2.0, the same ratio). Grazed and young, the
            // alpha sweep on `patch` (8 seeds, 2026-10-04) measured a mature stand at 0.92×
            // the original velvetpad (preset alpha 4) and 1.7× today's, with the world's
            // edible + litter at 1.07× the original plants; 5.4 and 6.3 gave the same stand,
            // 6.3 the more food. Was 1.8 (presets 3.6), 1.5 (3.0), before that 6.2 (4.0).
            alpha: 3.15,
            senescence: 0.00167,
            foliage_rate: 0.06,
            wood_rate: 0.01,
            propagule_rate: 0.002,
            assimilation: 0.008,
            crown_height_m: [0.125, 0.1875],
            crown_radius_m: [0.125, 0.25],
            // Grazing refuge: a grass's basal buds and crowns (`graze_refuge`'s doc).
            graze_refuge: 0.4,
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
    /// support with none already fails on `pore_ok` (`tests/small_plant_births.rs`,
    /// `a_paid_stonecushion_birth_on_a_rock_ledge_beside_a_soil_pocket`). Water is drawn
    /// only from those soil voxels. Accepting a rock face *provided* the root box holds
    /// soil is therefore the behaviour the model already has, with no flag.
    ///
    /// Every number is a **placeholder** (`design/backlog.md` §1):
    ///
    /// - **Water, the three thresholds together**, on the available-water scale
    ///   (0 at the wilting point, 1 at field capacity). `wilt_water` 0 — it earns on any
    ///   water a root can draw at all — `full_water` 0.2 and `establish_water_min` 0.05 are
    ///   the lowest of every species, because the point of the role is that a little water
    ///   is *enough*: a pocket holding a twentieth of drained soil's water is enough to
    ///   start on, and one holding a fifth is full moisture. `establish_saturated_max` 0.4
    ///   and `drown_depth_m` 0.02: a rock face does not hold a pool, and a cushion under
    ///   one is finished.
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
    ///  . `alive_min` 0.01 against `wood_max` 0.1 and `donor_min` 0.05.
    /// - **Shape.** `hop` 1; the crown is `[0.125, 0.1875]` m tall — a cushion has no stem
    ///   at any size — and `[0.125, 0.15625]` m in radius.
    pub fn stonecushion() -> SpeciesConfig {
        SpeciesConfig {
            founder_per_m2: 0.2,
            founder_colony: 3,
            founder_cap: 64,
            light_half: 0.3,
            rooting_depth: 2,
            rooting_radius: 1,
            wilt_water: 0.0,
            full_water: 0.2,
            establish_water_min: 0.05,
            establish_light_min: 0.4,
            stress_rate_per_s: 0.1,
            relax_rate_per_s: 0.02,
            establish_saturated_max: 0.4,
            drown_depth_m: 0.02,
            // Seeds: **wind**, at a kernel scale of `hop` 1 — the cushion is
            // tiny and its seed is dust — so most stay within a metre and a few percent
            // cross the rock to the next crack. 12 h, the top of the range: a seed on
            // bare rock waits for a crack to wet. A ground cover.
            hop: 1,
            dispersal: Dispersal::Wind,
            ground_cover: true,
            seed_max_age_s: 43_200.0,
            seed_attrition_per_s: 1.0 / 43_200.0,
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
            // Grazing refuge: a cushion is mostly skirt pressed to the rock (`graze_refuge`'s doc).
            graze_refuge: 0.4,
            // Anatomy document §3: a dome in two foliage bands, no trunk. Decisions
            // §3 keeps it on the browser's menu, so there is no diet flag here.
            profile: SpeciesConfig::one_stage(vec![
                SpeciesConfig::foliage_layer([0.0, 0.6], 1.0, 0.7, 0.2),
                SpeciesConfig::foliage_layer([0.6, 1.0], 0.6, 0.3, 0.2),
            ]),
            ..SpeciesConfig::v1_base()
        }
    }

    /// **Velvetpad — the moist, aerated understory pad.** The niche apart from
    /// umbrellafrond's wetland: damp soil that still has air in it, under
    /// somebody else's crown. Umbrellafrond is saturation-immune and wants a hollow;
    /// velvetpad wants the floor beside it and pays for a wholly saturated root box.
    ///
    /// Every number is a **placeholder** (`design/backlog.md` §1):
    ///
    /// - **Water, the three thresholds together**, on the available-water scale
    ///   (0 at the wilting point, 1 at field capacity). `wilt_water` 0.25, `full_water`
    ///   0.9, `establish_water_min` 0.4: a damp-lover — it wilts in the bottom quarter of
    ///   what drained ground offers and needs nearly all of it to be at full moisture, so
    ///   a pad on a drying ridge earns badly long before an upland plant does, while
    ///   drained ground itself is still full moisture. The difference from the wetland
    ///   role (umbrellafrond wilts at 1.0) is the floor, and above all the ceiling:
    ///   `establish_saturated_max` 0.6 against umbrellafrond's 1.0, so a wholly saturated box refuses a velvetpad cohort and targets an adult's
    ///   `aeration_stress` at 1 — waterlogging costs this species something, which is the
    ///   whole of what "aerated" means here. `stress_rate_per_s` 0.1 and
    ///   `relax_rate_per_s` 0.05: it closes on that stress twice as fast as it lets go.
    ///   `drown_depth_m` 0.1 is more than three times springturf's 0.03 — a forest floor
    ///   takes a puddle — and a fifth of umbrellafrond's 0.5.
    /// - **Light.** `light_half` 0.1 is the smallest of the five, so at a fifth of open
    ///   sky velvetpad's light response `L (1 + h) / (L + h)` is **0.7333** where
    ///   bloomcrown's is **0.36**;
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
    ///   in radius — low and broad, a mat on the floor rather than a
    ///   stem, and the widest ground cover relative to its height.
    ///
    /// **Succession redesign (2026-10-03, Wrysk: "a mossy undergrowth / early
    /// colonizer").** `alpha` 1.0, half the base (the presets: 4 → 2): a thin moss, half
    /// the foliage and so half the litter per unit wood; still positive down to L 0.05 on
    /// damp ground and through a half-saturated box (the viability grid). Its spores and
    /// runners are unchanged — it still colonises bare ground first — and springturf now
    /// comes up through it on open drained ground.
    pub fn velvetpad() -> SpeciesConfig {
        SpeciesConfig {
            founder_per_m2: 0.4,
            founder_colony: 6,
            founder_cap: 120,
            light_half: 0.1,
            rooting_depth: 2,
            rooting_radius: 1,
            wilt_water: 0.25,
            full_water: 0.9,
            establish_water_min: 0.4,
            establish_light_min: 0.05,
            stress_rate_per_s: 0.1,
            relax_rate_per_s: 0.05,
            // Waterlogged ground (2026-10-04, Wrysk: velvetpad may sprout there): a moss
            // takes a saturated, unflooded box — 1.0 is no saturation ceiling and no
            // aeration stress — and drowning under more than `drown_depth_m` of standing
            // water stays the separate limit. Was 0.6.
            establish_saturated_max: 1.0,
            drown_depth_m: 0.1,
            // Seeds: **spores to damp ground** within a wide `hop` 6, and
            // **runners** for half the packages — a pad spreads across the floor it is on
            // and spores reach the next damp patch. 3 h, the middle of the spores' 2–4 h.
            // Package SU: **2** spore tries, not 8 — with eight the pad's rain found
            // almost every damp face in reach and it took the terrarium's damp ground
            // (121 → 659 stands in 3 h, springturf 54 → 4); with two, the half of the rain
            // that misses a patchy floor is lost as it would be. Runners 0.3 of the
            // packages, springturf's share (was 0.5; Fable's tuning round: the pad still
            // held ~60 % of the terrarium's stands on two tries). A ground cover.
            hop: 6,
            dispersal: Dispersal::Spores,
            clonal_share: 0.3,
            spore_tries: 2,
            ground_cover: true,
            seed_max_age_s: 10_800.0,
            seed_attrition_per_s: 1.0 / 10_800.0,
            wood_max: 0.2,
            alive_min: 0.015,
            donor_min: 0.1,
            alpha: 1.0,
            senescence: 0.0005,
            foliage_rate: 0.004,
            wood_rate: 0.002,
            propagule_rate: 0.0005,
            crown_height_m: [0.125, 0.125],
            crown_radius_m: [0.1875, 0.375],
            // Grazing refuge: basal buds under a grazer's mouth (`graze_refuge`'s doc).
            graze_refuge: 0.3,
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
    ///   *ends* — on a floor with litter on it.
    /// - **Water, the three thresholds together.** `establish_water_min` 0.2 and
    ///   `establish_saturated_max` 0.5 are the brief's "pore between the species' floor and
    ///   its saturation ceiling", read in the model's own terms — the existing pore gate is
    ///   the floor and the existing aeration gate is the ceiling, and **no new rule was
    ///   added**. `wilt_water` 0.2 and `full_water` 0.9 (the available-water scale,
    ///   1 = field capacity; the damp-lovers' pair): full uptake on ordinary drained soil,
    ///   nothing at all on a dry one, because `μ` multiplies uptake exactly as it
    ///   multiplies assimilation. The germination floor sits at the wilt threshold. `drown_depth_m` 0.05 — a cap under a pool is finished.
    ///
    ///   **A stated limitation, and the one this preset is most likely to be wrong about.**
    ///   The ceiling is a *germination* ceiling: a spore will not take a waterlogged log,
    ///   while the mycelium already in one pays nothing, because a saprotroph's uptake reads
    ///   `μ` and not `1 − aeration_stress` (the brief names only `μ`, and `step`'s `feed`
    ///   says so). That is exactly the germination-versus-adult-tolerance assumption
    ///   `establish_saturated_max`'s own doc records for the plants, pointing
    ///   the other way for this species. Its `stress_rate_per_s` 0.1 and `relax_rate_per_s`
    ///   0.05 are therefore **inert on the income** and kept only because the stress is
    ///   still tracked and drawn.
    /// - **The box.** `substrate_reach_up_down` 1 and `rooting_radius` 1: the mycelium box
    ///   is the twenty-seven support faces within one row of its own — its own level, the
    ///   row above and the row below — so a log one voxel up or down a step is in reach of
    ///   the mycelium and a log two rows away is not. Water is read off
    ///   the *soil voxels* of the `rooting_depth` root box, as every stand's is, which is why
    ///   a glowcap on bare rock has `μ = 0` and starves however much wood is on the rock:
    ///   one water read for every stand is the model's rule and a fungus is not exempted
    ///   from it here. **Bare rock means no soil voxel anywhere in that box** — a rock face
    ///   with a soil pocket inside the box reads that pocket's water and is not bare.
    ///   `transpiration_m3_per_s` **0.0** — it reads the moisture and withdraws nothing, so
    ///   a fungus takes no water away from the plants it lives among.
    /// - **The dark** (2026-10-01): a cave floor or the face under an overhang is the one
    ///   exception to the two rules above. There the cave damp stands in for soil water and
    ///   the seep for wood (`dark_*`, [`SpeciesConfig::darkness`]), so a bare dark rock
    ///   face is habitat and a lit one is not.
    /// - **Small and cheap.** `wood_max` 0.1, `alive_min` 0.01, `donor_min` 0.05 —
    ///   stonecushion's body; `foliage_rate` 0.004 and `wood_rate` 0.002 are twice the base,
    ///   a mycelium that fills a log fairly quickly; `propagule_rate` 0.0005 is two and a
    ///   half times the base, so one 0.025 package is **60 s** of a fully funded donor's
    ///   saving — the fastest of the six, because a fruiting body's whole job is spores.
    /// - **Spread.** `hop` 1: the eight faces around it. A grove follows its log.
    /// - **Shape.** `crown_height_m` `[0.125, 0.25]` and `crown_radius_m`
    ///   `[0.0625, 0.0625]`: a cap one or two small cells tall on the
    ///   face above its support, a single column wide. That is
    ///   the interim glyph and not a design — the art direction of the voxel world is its
    ///   own thread, and `crates/cubarium/src/voxel/stand.rs` names the palette interim too.
    ///   One consequence in the model: a crown top of `y + 0.5` is the lowest of the six, so
    ///   a glowcap shades nothing at all, and everything shades it — which costs it nothing,
    ///   because it does not eat light.
    pub fn glowcap() -> SpeciesConfig {
        SpeciesConfig {
            founder_per_m2: 0.15,
            founder_colony: 3,
            founder_cap: 40,
            // The dark habitat (`design/handoffs/plants-glowcap-vine-2026-10-01.md` B):
            // dark below sky 0.45, fully dark at 0.1; the seep pays 0.008 per unit of
            // mycelium at darkness 1 — a log at full moisture's `0.4 · 0.02`, so the same
            // ~3× margin over upkeep; a dark site reads at least 0.8 of field capacity; the
            // substrate gate opens at darkness 0.5; spores favour the dark 1 + 4·darkness;
            // half the founders go to the darkest faces, with no log laid.
            dark_sky_max: 0.45,
            dark_sky_full: 0.1,
            dark_uptake_per_s: 0.008,
            dark_damp: 0.8,
            dark_establish_min: 0.5,
            dark_spore_bias: 4.0,
            dark_founder_share: 0.5,
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
            wilt_water: 0.2,
            full_water: 0.9,
            establish_water_min: 0.2,
            establish_saturated_max: 0.5,
            stress_rate_per_s: 0.1,
            relax_rate_per_s: 0.05,
            drown_depth_m: 0.05,
            transpiration_m3_per_s: 0.0,
            // Spores: land only on a face whose box holds the substrate (and
            // the damp) it needs, within a wide `hop` 6 — the next log over, not only its
            // own. The shortest bank, 2 h, the bottom of the spores' range: a grove's
            // spore rain is continuous, and a fungal spore on bare ground does not last.
            hop: 6,
            dispersal: Dispersal::Spores,
            seed_max_age_s: 7_200.0,
            seed_attrition_per_s: 1.0 / 7_200.0,
            wood_max: 0.1,
            alive_min: 0.01,
            donor_min: 0.05,
            // Cap turnover (2026-10-05, the capgnawer package): a fruiting body takes about
            // a world day to come back and lasts about as long, not minutes. The earlier
            // 0.004 (the presets' 0.00656) regrew a grazed cap in about 260 s, so one
            // glowcap fed about a hundred capgnawers and grazing never reached the caps.
            // Regrowth and `senescence` are slowed together, by the same 1/27: ungrazed caps
            // stand where `foliage_rate · W = senescence · P` puts them, as before. Slowing
            // the regrowth alone (c65800b9) left caps at about 7 % of capacity, below the
            // graze refuge, and capgnawers found nothing to eat (24 h terrarium check-in).
            // On `patch` (8 seeds, founded under each config): edible caps 0.15 with
            // regrowth alone, 164 with the old rates, 145 and still filling at minute 240
            // with both slowed, grazed by capgnawers from minute 180.
            foliage_rate: 0.00015,
            senescence: 0.0000375,
            wood_rate: 0.002,
            propagule_rate: 0.0005,
            // The fruiting body is the spore source (the capgnawer package, 2026-10-05):
            // spore saving follows the caps standing, so grazed caps recruit less.
            propagule_cap_share: 1.0,
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

    /// **Vaulttree — the canopy builder of deep aerated soil** (dossier D11). Establishes
    /// only in
    /// an open gap, grows slowly into a large, long-lived crown on a large reserve, and at
    /// death lays its wood as a line of dead wood (`step`'s `die`).
    ///
    /// Every number is a **placeholder** (`design/backlog.md` §1):
    ///
    /// - **Light.** `establish_light_min` 0.9, the brief's gap gate. Germination light is
    ///   geometric sky with **no living canopy in it** (`step`'s `Gates`), so "needs a
    ///   gap" is "needs open sky" here: a vault seed under another stand's crown in open
    ///   terrain still passes. A canopy-aware gate is a separate rule.
    ///   `light_half` 0.6: a sun canopy, under bloomcrown's 0.8.
    /// - **Water.** Deep, wide roots (`rooting_depth` 6, `rooting_radius` 2 at 0.25 m),
    ///   upland thresholds on the available-water scale (`establish_water_min`
    ///   0.2, `wilt_water` 0.1, `full_water` 0.5: full moisture through the top half of
    ///   what drained ground offers), and an aerated root zone: `establish_saturated_max` 0.2 with a **slow**
    ///   `stress_rate_per_s` 0.005, so it stresses on long saturation and shrugs off a
    ///   wet week. `drown_depth_m` 0.1.
    /// - **Slow, long-lived, large reserve.** `wood_max` 5.0 (bloomcrown 0.6),
    ///   `wood_rate` 0.0003 and `foliage_rate` 0.001, `maintenance` 0.0001 and
    ///   `senescence` 0.0005, half the base; `reserve_cap` 1.5, three times the base.
    ///   `alive_min` 0.02 (a 0.05 minimum package; a seed is four of them at
    ///   `propagule_rate` 0.0008, still 300 s of a funded donor), `donor_min` 2.5, `hop` 4.
    /// - **Shape.** Crown `[2.25, 3.5]` m × `[0.625, 1.0]` m, grown from the capped seedling;
    ///   the profile is anatomy §3's table.
    pub fn vaulttree() -> SpeciesConfig {
        SpeciesConfig {
            founder_per_m2: 0.1,
            founder_colony: 1,
            founder_cap: 12,
            light_half: 0.6,
            establish_light_min: 0.9,
            rooting_depth: 6,
            rooting_radius: 2,
            wilt_water: 0.1,
            full_water: 0.5,
            establish_water_min: 0.2,
            establish_saturated_max: 0.2,
            stress_rate_per_s: 0.005,
            relax_rate_per_s: 0.02,
            drown_depth_m: 0.1,
            falls: true,
            // Seeds: a **winged seed on the wind** at a kernel scale of `hop` 4
            // (1 m): most land under or beside the crown and a few percent 10 m out. 6 h,
            // the bottom of the range: a large tree seed does not keep, and the tree's
            // bet is a seedling in the next gap, not a long bank. Package SU: a seed of
            // **4** minimum packages (0.2 of material), the biggest seedling of the nine;
            // `propagule_rate` ×4 with it (0.0002 → 0.0008), so a funded donor still sends
            // a seed every 300 s, and bloomcrown's 0.1 per check through a ground cover.
            hop: 4,
            dispersal: Dispersal::Wind,
            seed_mass: 4,
            overtop_per_check: 0.1,
            seed_max_age_s: 21_600.0,
            seed_attrition_per_s: 1.0 / 21_600.0,
            wood_max: 5.0,
            alive_min: 0.02,
            donor_min: 2.5,
            reserve_cap: 1.5,
            maintenance: 0.0001,
            senescence: 0.0005,
            foliage_rate: 0.001,
            wood_rate: 0.0003,
            propagule_rate: 0.0008,
            crown_height_m: [2.25, 3.5],
            crown_radius_m: [0.625, 1.0],
            // Anatomy document §3. The limbs are a sparse Trunk (structure, no foliage);
            // the lobes' sky gaps are porosity 0.45; the drape hangs under the limbs.
            // Bands follow the artbook pick (`art/gen/artbook/vaulttree.png`, Wrysk
            // 2026-10-01): its shelves span 0.55–1.0 of height (the lowest shelf's
            // underside at ~0.53) and the drape's bulk hangs from the lowest shelves to
            // ~0.25 (a few strands reach ~0.1).
            profile: vec![
                Profile {
                    wood_fraction_max: 0.1,
                    height_m_max: Some(0.125),
                    layers: vec![SpeciesConfig::foliage_layer([0.0, 1.0], 1.0, 1.0, 0.4)],
                },
                Profile {
                    wood_fraction_max: 0.4,
                    height_m_max: None,
                    layers: vec![
                        SpeciesConfig::trunk_layer([0.0, 0.55], 0.15, 0.0),
                        SpeciesConfig::foliage_layer([0.55, 1.0], 1.0, 1.0, 0.4),
                    ],
                },
                Profile {
                    wood_fraction_max: f64::INFINITY,
                    height_m_max: None,
                    layers: vec![
                        SpeciesConfig::trunk_layer([0.0, 0.45], 0.15, 0.0),
                        SpeciesConfig::trunk_layer([0.45, 0.55], 0.6, 0.85),
                        Layer {
                            kind: LayerKind::Drape,
                            ..SpeciesConfig::foliage_layer([0.25, 0.55], 0.7, 0.15, 0.9)
                        },
                        SpeciesConfig::foliage_layer([0.55, 1.0], 1.0, 0.85, 0.45),
                    ],
                },
            ],
            ..SpeciesConfig::v1_base()
        }
    }

    /// **Lanternberry — the fruiting shrub of grove edges and moist bright gaps** (package
    /// N; dossier D12). Its fruit is the ordinary parcel for now: a donor saves only out of
    /// reserve above a high floor, so each crop waits for the reserve to recover.
    ///
    /// Every number is a **placeholder** (`design/backlog.md` §1):
    ///
    /// - **Light** between umbrellafrond and bloomcrown: `light_half` 0.4,
    ///   `establish_light_min` 0.35.
    /// - **Moist to germinate, upland once grown** (the available-water scale, 1 = field
    ///   capacity). `establish_water_min` 0.4: a seed still waits for moist ground, which
    ///   is where the shrub of moist gaps starts. `wilt_water` 0.1 and `full_water` 0.5
    ///   are the upland ramp: a damp-lover's ramp empties its donors as the root box dries
    ///   between showers. Its break-even is bloomcrown's (`L · μ` ≈ 0.49 at full foliage).
    ///   `establish_saturated_max` 0.5, `drown_depth_m` 0.1.
    /// - **Repeated paid fruit**: `donor_reserve_floor` 0.7 (base 0.5) and `propagule_rate`
    ///   0.0004 per minimum package — 0.0012 for its three-package fruit stone;
    ///   `wood_max` 1.0, `alive_min` 0.02, `donor_min` 0.4, `hop` 2.
    /// - **Shape.** Crown `[0.625, 1.125]` m × `[0.25, 0.4375]` m from the capped seedling.
    ///
    /// **The browse line.** Anatomy §3 gives the adult one foliage layer `0.3–1.0`. A
    /// layer is reached by the one disc cell at its band's top, so as one layer a grown
    /// shrub's whole foliage would sit above a 0.5 m mouth and nothing would be cropped —
    /// not the brief's accepted browse line. The layer is therefore **cut in two at 0.45**
    /// of the height, same kind, radius and porosity, shares in proportion to band length
    /// (3/14 and 11/14): the same anatomy, with a lower skirt whose top is 0.51 m on a
    /// full-grown 1.125 m shrub, inside the browser's band on both grids, and an upper
    /// crown above it. A young adult is inside the band entirely.
    pub fn lanternberry() -> SpeciesConfig {
        SpeciesConfig {
            founder_per_m2: 0.25,
            founder_colony: 3,
            founder_cap: 40,
            light_half: 0.4,
            establish_light_min: 0.35,
            rooting_depth: 3,
            rooting_radius: 1,
            wilt_water: 0.1,
            full_water: 0.5,
            establish_water_min: 0.4,
            establish_saturated_max: 0.5,
            stress_rate_per_s: 0.1,
            relax_rate_per_s: 0.05,
            drown_depth_m: 0.1,
            // Seeds: the fruit **drops beneath** the crown, `hop` 2 (the
            // seedporter that carries it further is a later package), into the base's 8 h
            // bank. Package SU: a fruit stone of **3** minimum packages, `propagule_rate`
            // ×3 with it (0.0004 → 0.0012) so the crop's seed count holds, and
            // bloomcrown's 0.1 per check through a ground cover.
            hop: 2,
            seed_mass: 3,
            overtop_per_check: 0.1,
            // F3 (the seedporter's food, predation-and-flight §3.7): fruit hung from the
            // reserve in daylight. 1e-4 a second is bloomcrown's nectar rate, about 1 % of
            // a full-grown shrub's gross income at full light (assimilation 0.004/s ×
            // foliage 2.0); the cap, 0.02, is 200 s of it and under a twentieth of the
            // reserve cap (0.5), so an unvisited crop costs the plant a small, bounded
            // store. Uneaten, the crop falls to the litter after half an hour (1800 s).
            // The mineral share 0.01 is nectar's: small, not zero. The shrub's own seed
            // parcel (`propagule_rate`, dropped beneath it) stays; fruit is a second,
            // carried crop.
            fruit_per_s: 1e-4,
            fruit_cap: 0.02,
            fruit_drop_s: 1800.0,
            // Plants-flowering-vines 2026-10-03: after an uneaten crop falls, a rest of 15
            // minutes on average (exponential), so an unvisited bush hangs fruit about
            // two thirds of the time (200 s to fill, 1,800 s hanging, 900 s resting) and
            // a grove's bushes fruit out of step. Eaten out, it refills at once.
            fruit_rest_s: 900.0,
            fruit_mineral_fraction: 0.01,
            wood_max: 1.0,
            alive_min: 0.02,
            donor_min: 0.4,
            donor_reserve_floor: 0.7,
            propagule_rate: 0.0012,
            crown_height_m: [0.625, 1.125],
            crown_radius_m: [0.25, 0.4375],
            profile: vec![
                Profile {
                    wood_fraction_max: 0.2,
                    height_m_max: Some(0.125),
                    layers: vec![SpeciesConfig::foliage_layer([0.0, 1.0], 1.0, 1.0, 0.5)],
                },
                Profile {
                    wood_fraction_max: f64::INFINITY,
                    height_m_max: None,
                    layers: vec![
                        SpeciesConfig::trunk_layer([0.0, 0.3], 0.4, 0.7),
                        SpeciesConfig::foliage_layer([0.3, 0.45], 1.0, 3.0 / 14.0, 0.5),
                        SpeciesConfig::foliage_layer([0.45, 1.0], 1.0, 11.0 / 14.0, 0.5),
                    ],
                },
            ],
            ..SpeciesConfig::v1_base()
        }
    }

    /// **Siphonreed — the emergent of saturated soil and shallow standing water**
    /// (dossier D13). It needs settled standing water on or beside its site
    /// ([`SpeciesConfig::water_depth_min_m`]) and tolerates a waterlogged root box at a
    /// cost.
    ///
    /// Every number is a **placeholder** (`design/backlog.md` §1):
    ///
    /// - **Water.** `water_depth_min_m` 0.1: a pool, not a rain film (bloomcrown's 0.05
    ///   drown depth is the precedent for "a pool"). `drown_depth_m` 0.5: it stands in the
    ///   water it needs, up to a third of a full-grown reed. Wetland thresholds on package
    ///   F's available-water scale (1 = field capacity), umbrellafrond's: `wilt_water`
    ///   1.0, `full_water` 1.5, `establish_water_min` 1.2 — ground wetter than drained.
    /// - **Aeration at a cost.** `establish_saturated_max` 1.0, like umbrellafrond — and
    ///   for the same reason not "slightly below 1" (its doc: every tolerance below 1
    ///   drives a wholly saturated box to stress 1). The cost is **upkeep** instead:
    ///   `maintenance` 0.0004, twice the base, which is the aerenchyma it pays for.
    /// - **Light** high: `light_half` 0.8, `establish_light_min` 0.6.
    /// - **Local paid spread**: `hop` 1, `wood_max` 0.3, `alive_min` 0.02, `donor_min`
    ///   0.15, `propagule_rate` 0.0003.
    /// - **Shape.** Crown `[0.75, 1.5]` m × `[0.0625, 0.125]` m, one porous column for life.
    pub fn siphonreed() -> SpeciesConfig {
        SpeciesConfig {
            founder_per_m2: 4.0,
            founder_colony: 6,
            founder_cap: 64,
            light_half: 0.8,
            establish_light_min: 0.6,
            rooting_depth: 2,
            rooting_radius: 1,
            wilt_water: 1.0,
            full_water: 1.5,
            establish_water_min: 1.2,
            establish_saturated_max: 1.0,
            stress_rate_per_s: 0.01,
            relax_rate_per_s: 0.05,
            drown_depth_m: 0.5,
            water_depth_min_m: 0.1,
            maintenance: 0.0004,
            // Seeds: carried by **water** to a standing-water margin within a
            // wide `hop` 6, downhill first, and **rhizomes** for 0.7 of the packages — a
            // reed bed is mostly one clone. 10 h: a wetland seed waits for the water level
            // to suit it.
            hop: 6,
            dispersal: Dispersal::Water,
            clonal_share: 0.7,
            seed_max_age_s: 36_000.0,
            seed_attrition_per_s: 1.0 / 36_000.0,
            wood_max: 0.3,
            alive_min: 0.02,
            donor_min: 0.15,
            propagule_rate: 0.0003,
            crown_height_m: [0.75, 1.5],
            crown_radius_m: [0.0625, 0.125],
            profile: SpeciesConfig::one_stage(vec![SpeciesConfig::foliage_layer(
                [0.0, 1.0],
                1.0,
                1.0,
                0.7,
            )]),
            ..SpeciesConfig::v1_base()
        }
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
    /// **Provision previously unrepresented ground**: a site gets its
    /// `initial_mineral` the first time it holds anything — a founder, a landed package, a
    /// deposit — and that import is booked as `seeded_mineral_in` then. The mineral
    /// inventory therefore **grows with the number of sites the plants have reached**, so
    /// two arms of one study that spread differently hold different totals, and a fertility
    /// comparison between them is confounded by its own colonisation.
    #[default]
    Lazy,
    /// **A fixed per-site inventory, laid down once at world creation**: every
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
    pub vaulttree: SpeciesConfig,
    pub lanternberry: SpeciesConfig,
    pub siphonreed: SpeciesConfig,
    /// The latticevine, which is not a stand: a vine rooted on a support face covering rock
    /// faces ([`cover`]).
    pub latticevine: VineConfig,
    /// Canopy attenuation, **per square metre**: a taller stand whose crown covers a
    /// site multiplies the light reaching that site by
    /// `exp(-shade_k_per_m2 · P / crown_area_m2)`, where the crown's area is
    /// `π · (radius_cells · voxel_m)²` floored at one reference cell, `(0.25 m)²`.
    ///
    /// Per square metre and not per cell, so optical depth does not depend on the grid.
    /// A config with an unknown field is refused (`deny_unknown_fields`).
    pub shade_k_per_m2: f64,
    /// `k_d`: litter decomposition per second. Its organic matter is respired out of the
    /// system (`respired_out`, energy to heat) and its mineral is released to the site's
    /// pool at the same fraction.
    pub decomposition: f64,
    /// `k_w`: dead-wood decomposition per second, the same two flows, slow.
    pub wood_decomposition: f64,
    /// `k_c`: **carrion** decomposition per second ([`Ground::carrion`]), the same two
    /// flows. **A placeholder** (`design/backlog.md` §1): 0.00028 /s, an e-folding of
    /// about an hour — half a day on the 2 h clock — so one kill feeds its killer, other
    /// lanternjaws and shredders over time (`design/predation-and-flight-2026-09-28.md`,
    /// decision 2). It was 0.005 /s (200 s), which nothing had measured and which spent a
    /// grazer before a lanternjaw had finished its first mouthful.
    pub carrion_decomposition: f64,
    /// `e_d_max`: retained energy cap per unit of litter.
    pub litter_energy_cap: f64,
    /// Mineral a site starts with the first time anything lands on it. Booked as
    /// `seeded_mineral_in`: it comes from outside the closed system, so it is a named
    /// inflow and not a residual.
    ///
    /// It is applied **lazily**, when a site first gets a `Ground` — a founder, a landed
    /// package, litter — so what it represents is *provisioning previously unrepresented
    /// ground*, not fertilizing it. Colonization therefore imports mineral
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
    /// **How often a banked site is tested**, seconds: germination, attrition
    /// and age are read at each site's own check, staggered across this period, and at the
    /// end of every shower — never in between. **30 s**, the fast end of the brief's
    /// "~30–60 s": a gap waits at most that long for its bank, which is nothing against
    /// hours of dormancy, and the bank costs `banked sites / 600` site visits a tick.
    /// Rounded to whole ticks, at least one; a fixture that wants the old every-tick bank
    /// sets it to `DT` ([`FloraConfig::drop_seeds_checked_each_tick`]).
    pub seed_check_s: f64,
    /// **The plant clock**, in world ticks: the plant layer steps once every
    /// `step_every` ticks, one step of `step_every · DT` seconds, at the same per-second
    /// rates — every flux is a rate times the step, every decay `exp(−r·dt)`, and every
    /// staggered check (the seed bank's wheel, the vines' contest and spread wheels, the
    /// film's second) still fires once per its own period, on the tick it falls on inside
    /// the step. `1` is the per-tick layer. Animals still graze, deposit and carry burrs
    /// every tick, between plant steps. A scheduling numeric, not ecology.
    ///
    /// The steps fall on the world ticks that are multiples of `step_every`, the phase the
    /// coarse water keeps (`World::water_due`): with the experiments' `water_every` equal
    /// to it, the plant step lands on the coarse water's tick and the overlapped schedule
    /// runs the two long phases side by side. **4**, the experiments' `water_every`
    /// (2026-10-04; it was 5): one plant step per 0.2 s.
    pub step_every: u32,
    /// **Whether a passing bank waits one check before it sprouts**: `true`,
    /// the shipped rule — the check that finds the gates passing draws the lottery and
    /// **arms** the site ([`Ground::sprouting`], what the D5 seed mark reads as "about to
    /// sprout"), and the next check germinates the armed species if it still passes.
    /// `false` germinates at the check that finds it passing, the pre-SM rule, for a
    /// fixture whose subject is not the bank's clock
    /// ([`FloraConfig::drop_seeds_checked_each_tick`]).
    pub sprout_after_arming: bool,
    /// **Q10 of plant upkeep**: maintenance and the vines' upkeep are multiplied by
    /// `upkeep_q10^((T - T_mean) / 10 °C)` ([`upkeep_factor`]), `T` the air temperature
    /// and `T_mean` the climate's mean. Dimensionless; 2.0 doubles it per 10 °C.
    pub upkeep_q10: f64,
    /// **Wind seed: tries.** A wind package's draws at columns before the donor keeps its
    /// package for the next tick; only a void column refuses one. Count, at least 1.
    pub wind_tries: u32,
    /// **Wind seed: kernel shape** `k` of the Lomax (Pareto II) distance
    /// `P(R > r) = (1 + r/a)^-k`. Dimensionless; at 2 the median seed lands 1.04 hops
    /// out, 79 % within 3 hops and 4 % past 10. A larger `k` is a thinner tail.
    pub wind_shape: f64,
    /// **Wind seed: kernel scale** `a`, in the species' `hop`s (distances in `hop`
    /// multiples, so it follows the voxel rescale through `hop`).
    pub wind_scale_hops: f64,
    /// **Burrs: the most seeds one animal carries** at once ([`SpeciesConfig::burr_pickup`]);
    /// a full coat picks up no more until one falls off. Count.
    pub burr_cap: u32,
    /// A carcass is drawn as **remains** rather than the body once the carrion pool is
    /// below this fraction of the organic matter the body died with. Fraction.
    pub carcass_remains_fraction: f64,
    /// A carcass record is dropped once its carrion pool has fallen to this fraction of
    /// what the body died with: the remains have gone into the soil. Fraction.
    pub carcass_gone_fraction: f64,
    /// How long a seed landing keeps its site's D5 mark, simulated seconds: long enough
    /// to see a donor's fall of seed arrive, short enough that a bank waiting hours for
    /// its gap is not drawn for hours.
    pub seed_mark_recent_s: f64,
    /// The floor on a foliage layer's area in the shade exponent, square metres: one
    /// reference 0.25 m cell, which is what the pre-layer crown model floored at.
    pub min_layer_area_m2: f64,
    /// **Founding** (the habitat seeder, `cubarium-voxel-sim`): a founder's starting wood as
    /// fractions of its species' `wood_max`, cycled in placement order, so the first
    /// colony is a mix of low growth, middle-height clumps and a few canopy anchors.
    /// Fractions of `wood_max`.
    pub founder_wood_fractions: [f64; 5],
    /// **Founding**: how much of a site's sky a crown standing over it takes away while
    /// the seeder is still placing founders. A placement proxy and not the light model.
    /// Fraction of open sky.
    pub founder_canopy_shade: f64,
    /// **Founding**: dead wood laid under each glowcap founder so its substrate gate can
    /// pass, in organic-matter units (twice the fungus's `establish_substrate_min`).
    pub founder_log_organic: f64,
    /// **Founding**: the least distance between two founder patch centres, metres. Colonies
    /// grow out from their own anchor, so this keeps gaps between them.
    pub founder_patch_spacing_m: f64,
    /// The world's cell size in metres, recorded so that geometry authored in **metres** can
    /// be applied to a grid stored in **voxels**: every crown is in metres since package
    /// L, and the voxel readings ([`SpeciesConfig::crown_height`]) divide by this. Set by
    /// [`FloraConfig::for_voxel_size`]; the reference 0.25 m by default.
    pub voxel_m: f64,
    /// **Glassfilm** (R2a): a site is wet-eligible while its standing water is at least
    /// `film_wet_min_depth_m` and at most this deep, metres.
    pub film_max_depth_m: f64,
    /// Glassfilm: light under water is the sky light times `exp(-this · depth)`, per metre.
    pub film_extinction_per_m: f64,
    /// Glassfilm: logistic growth rate per second at full light,
    /// `rate · light · film · (1 − film / capacity)`, with no construction cost.
    pub film_growth_per_s: f64,
    /// Glassfilm: carrying capacity per square metre of face, organic matter.
    pub film_capacity_per_m2: f64,
    /// Glassfilm: upkeep per second per unit film, respired, on every site holding film,
    /// wet or dry.
    pub film_maintenance_per_s: f64,
    /// Glassfilm: fragments a less-filmed eligible site gains per second per unit of each
    /// eligible 4-neighbour's film, paid by the neighbour.
    pub film_spread_per_s: f64,
    /// Glassfilm: loss per second on a dry or too-deep site, into its own litter.
    pub film_dry_death_per_s: f64,
    /// Glassfilm: the shallowest standing water a site is wet-eligible under, metres.
    /// Shallower water is a **residue** (what a drained shower or a stopped film leaves
    /// on a ledge, often under an overhang where nothing evaporates it) and the site
    /// counts as dry: no growth, dry death, no spread either way.
    pub film_wet_min_depth_m: f64,
    /// Glassfilm: on a dry or too-deep site, film that would fall below this share of the
    /// site's capacity dies whole into its litter that step, so a dried site ends at
    /// exactly no film rather than an exponential sliver.
    pub film_gone_fraction: f64,
    /// Glassfilm: what [`Flora::seed_film`] puts on every eligible site, organic matter
    /// per square metre.
    pub film_seed_per_m2: f64,
    /// **The film's grazing refuge** (the detritus web, Astra §3.1F): the share of a
    /// site's film capacity a grazer cannot take — film in crevices and pores a rasping
    /// mouth does not reach — so grazing leaves every site a seed stock to regrow from
    /// (its growth is proportional to the film standing). 0 lets a grazer strip a site.
    #[serde(default = "default_film_graze_refuge")]
    pub film_graze_refuge: f64,
    /// Glassfilm: mineral per unit of film tissue, drawn from the site's pool as it grows
    /// (springturf's `n_tissue` by default).
    pub film_n_tissue: f64,
    /// Glassfilm: energy per unit of film tissue (springturf's `energy_density` by
    /// default).
    pub film_energy_density: f64,
    /// **Glassfilm at founding** (roster R2b): when `true`, the habitat founding calls
    /// [`Flora::seed_film`] on the pre-rolled world before the animals are founded.
    /// Default **false**, so co-training worlds do not change; the live presets set it.
    /// Not a number, so it has no registry row.
    pub seed_film_at_founding: bool,
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
            vaulttree: SpeciesConfig::vaulttree(),
            lanternberry: SpeciesConfig::lanternberry(),
            siphonreed: SpeciesConfig::siphonreed(),
            latticevine: VineConfig::default(),
            shade_k_per_m2: 0.09375,
            decomposition: 0.001,
            wood_decomposition: 0.0001,
            carrion_decomposition: 0.00028,
            litter_energy_cap: 2.0,
            initial_mineral: 1.0,
            provision: Provision::Lazy,
            seed_check_s: 30.0,
            // Plants live on minute-to-hour clocks: a step every quarter second is
            // invisible on the display and a fifth of the per-tick cost.
            step_every: 4,
            sprout_after_arming: true,
            upkeep_q10: 2.0,
            wind_tries: 4,
            wind_shape: 2.0,
            wind_scale_hops: 2.5,
            // A handful: a coat holds many burrs, but one burr here is a whole package.
            burr_cap: 6,
            carcass_remains_fraction: 0.3,
            carcass_gone_fraction: 0.02,
            seed_mark_recent_s: 120.0,
            min_layer_area_m2: 0.25 * 0.25,
            founder_wood_fractions: [0.30, 0.42, 0.55, 0.70, 0.85],
            founder_canopy_shade: 0.6,
            founder_log_organic: 0.4,
            founder_patch_spacing_m: 1.5,
            voxel_m: 0.25,
            film_max_depth_m: 0.6,
            film_extinction_per_m: 0.8,
            // Plausible, untuned (brief R2a): a fresh film at 5 % of capacity fills to 90 % in
            // about an hour of full light (net 1.4e-3 /s), a dried film is 95 % gone in
            // about ten minutes (ln 20 / 0.005 s), dark upkeep halves it in about two
            // hours, and spread is a fraction of growth.
            film_growth_per_s: 0.0015,
            film_capacity_per_m2: 0.4,
            film_graze_refuge: default_film_graze_refuge(),
            film_maintenance_per_s: 0.0001,
            film_spread_per_s: 0.0002,
            film_dry_death_per_s: 0.005,
            // Plausible, untuned (film-dry, 2026-10-02): on the desktop terrarium's
            // founding (seed 1) the wet faces split into residues under 1e-3 m (108 open,
            // 836 covered, nearly all under 1e-6 m) and pools from 0.03 m up (a half-cell
            // margin is ~0.06 m, a one-cell one ~0.11 m), with 32 faces between. 1 cm sits
            // in that gap: ten times the residues, a sixth of a one-cell margin.
            film_wet_min_depth_m: 0.01,
            // Under the render's first drawn level (1/16 of capacity): a mark fades out
            // before the film is cut, and a dried full film is gone in ln 50 / 0.005 s,
            // about 13 minutes.
            film_gone_fraction: 0.02,
            // Half the capacity: a live world founds an established pond, not a fresh
            // film (roster R2b repair). Read only when `seed_film_at_founding` is on.
            film_seed_per_m2: 0.2,
            film_n_tissue: SpeciesConfig::springturf().n_tissue,
            film_energy_density: SpeciesConfig::springturf().energy_density,
            seed_film_at_founding: false,
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
        FloraConfig::default().rescaled_to(voxel_m)
    }

    /// This config, authored on the reference 0.25 m voxel, expressed for `voxel_m`: the
    /// cell size recorded, and on a finer grid every row the parameter registry tags with
    /// a voxel scale multiplied by it ([`registry`]: each species' root box, mycelium
    /// reach and hop by `0.25 / voxel_m`, rounded and never below one voxel; the
    /// latticevine's per-face amounts by the face's area against the reference face's).
    /// Reference-sized and coarser grids keep the authored geometry, as do invalid sizes
    /// (which the world config rejects at its own input boundary).
    pub fn rescaled_to(mut self, voxel_m: f64) -> FloraConfig {
        const REFERENCE_VOXEL_M: f64 = 0.25;
        if !voxel_m.is_finite() || !(voxel_m > 0.0) {
            return self;
        }
        // The cell size is recorded whatever it is, because a metre rule has to be
        // converted on a coarse grid too; only the authored *voxel* geometry is left
        // alone at or above the reference size.
        self.voxel_m = voxel_m;
        if voxel_m >= REFERENCE_VOXEL_M {
            return self;
        }
        registry()
            .rescale(&mut self, REFERENCE_VOXEL_M / voxel_m)
            .expect("every scale-tagged flora field rescales");
        self
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

    /// Every species' seeds **dropped** within its `hop` — no wind, spores, water or
    /// runners — every banked site tested **every tick**, and a passing bank sprouting at
    /// the check that finds it passing, with no arming check.
    ///
    /// For a **fixture whose subject is not dispersal or the bank's clock** — an
    /// establishment gate, a birth's stoichiometry, the lottery's weights. Not a shipped
    /// configuration; nothing in the tick uses it.
    pub fn drop_seeds_checked_each_tick(mut self) -> FloraConfig {
        let scale = (0.25 / self.voxel_m).max(1.0);
        for species in Species::ALL {
            let sc = self.species_mut(species);
            sc.dispersal = Dispersal::Drop;
            sc.clonal_share = 0.0;
            if matches!(
                species,
                Species::Umbrellafrond | Species::Velvetpad | Species::Glowcap | Species::Siphonreed
            ) {
                sc.hop = (scale.round() as u32).max(1);
            }
        }
        self.seed_check_s = DT;
        self.sprout_after_arming = false;
        self
    }

    /// Every species with **no grazing refuge** (`graze_refuge` 0): a stand can be
    /// withdrawn to nothing. For a fixture whose patches are eaten down to nothing. Not a
    /// shipped configuration: the live world's plants keep theirs.
    pub fn without_graze_refuge(mut self) -> FloraConfig {
        for species in Species::ALL {
            self.species_mut(species).graze_refuge = 0.0;
        }
        self
    }

    /// Every species' seed back to **one minimum package** at the rate that paid for one
    /// (`propagule_rate` divided by `seed_mass`), and no seedling coming up through
    /// ground cover.
    ///
    /// For a **fixture whose subject is a package's arithmetic or the lottery's**: one seed
    /// is exactly the material of a stand at `alive_min`. Not a shipped configuration;
    /// nothing in the tick uses it.
    pub fn minimum_seeds(mut self) -> FloraConfig {
        for species in Species::ALL {
            let sc = self.species_mut(species);
            sc.propagule_rate /= f64::from(sc.seed_mass.max(1));
            sc.seed_mass = 1;
            sc.overtop_per_check = 0.0;
        }
        self
    }

    /// Every crown — both ranges and each stage's seedling ceiling — multiplied by `k`.
    ///
    /// For a **fixture on a coarse grid** whose subject is a rule and not a plant's
    /// size: `crowns_scaled(4.0)` gives a 1 m-cell fixture the crowns, in cells, that a
    /// 0.25 m grid has. Not a shipped configuration; nothing in the tick uses it.
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
            Species::Vaulttree => &self.vaulttree,
            Species::Lanternberry => &self.lanternberry,
            Species::Siphonreed => &self.siphonreed,
        }
    }

    /// The place on a face a stand of `species` takes ([`FaceLayer`]), by its trophic mode.
    #[inline]
    pub fn face_layer(&self, species: Species) -> FaceLayer {
        FaceLayer::of(self.species(species).trophic)
    }

    /// `stand`'s sort key in [`FloraView::stands`]: its site, then its place.
    #[inline]
    pub(crate) fn stand_key(&self, stand: &Stand) -> (Site, FaceLayer) {
        (stand.site, self.face_layer(stand.species))
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
            Species::Vaulttree => &mut self.vaulttree,
            Species::Lanternberry => &mut self.lanternberry,
            Species::Siphonreed => &mut self.siphonreed,
        }
    }

    /// Every number inside its row's range in the parameter registry ([`registry`]), and
    /// the cross-field rules (the anatomy, the propagule split, `alive_min <= wood_max`,
    /// the latticevine's thresholds), with the path named in the message. [`Flora::new`]
    /// calls this, so a config that cannot produce a living stand is refused where it
    /// enters rather than as a `NaN` in a ledger ten thousand ticks later. **Not** a
    /// plausibility check.
    pub fn validate(&self) -> Result<(), String> {
        registry().check(self)?;
        registry::cross_rules(self)
    }
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
        // The latticevine's root box is voxel geometry like a stand's.
        let (vb, va) = (&reference.latticevine, &mut geometry_reset.latticevine);
        assert_eq!(va.rooting_depth, vb.rooting_depth * 2);
        assert_eq!(va.rooting_radius, vb.rooting_radius * 2);
        va.rooting_depth = vb.rooting_depth;
        va.rooting_radius = vb.rooting_radius;
        // A half-size face is a quarter of the area: every per-face amount is a quarter.
        assert_eq!(va.leaf_mass, vb.leaf_mass / 4.0);
        assert_eq!(va.transpiration_m3_per_s, vb.transpiration_m3_per_s / 4.0);
        *va = vb.clone();
        // The recorded cell size is not authored geometry: it is the world's, and it
        // is what a rule written in metres is converted with
        // ([`SpeciesConfig::crown_height`]).
        assert_eq!(geometry_reset.voxel_m, 0.125);
        geometry_reset.voxel_m = reference.voxel_m;
        assert_eq!(geometry_reset, reference, "rates and stores stay authored");
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
/// `Seed`, a new site's `initial_mineral`, a removal, or a dark site's seep
/// (`seep_mineral_in`, which arrives with its organic matter).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FloraLedger {
    /// Energy fixed from light: `e_v · A`, the energy side of `fixed_in`.
    pub light_in: f64,
    /// Energy respired.
    pub heat_out: f64,
    /// Organic matter assimilation created from light: the CO2 supply, entering.
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
    /// **Seep**: organic matter, mineral and energy a dark site's drip water brought a
    /// saprotroph ([`SpeciesConfig::dark_uptake_per_s`]) — an external input beside
    /// `fixed_in` / `light_in`, entering with its own mineral at `n_tissue` and its energy
    /// at `energy_density`.
    pub seep_organic_in: f64,
    pub seep_mineral_in: f64,
    pub seep_energy_in: f64,
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
    /// - `propagule_landed`: what left donors as whole packages — into a seed bank, a
    ///   runner daughter, or (a spore or water seed that found no ground) the litter.
    ///
    /// Requested, funded and landed are kept apart because raising a rate that is already
    /// not the binding constraint creates no income. `funded − landed` is the material
    /// standing in parcels, plus whatever parcels have gone to litter with their donors.
    pub propagule_requested: [f64; Species::COUNT],
    pub propagule_funded: [f64; Species::COUNT],
    pub propagule_landed: [f64; Species::COUNT],
    /// **The night budget's spend** (1b), per species, indexed by [`Species::index`]:
    /// reserve spent while the sun was down on `[reflush, nectar, fruit]`, organic units,
    /// the reflush with its construction respiration. A diagnostic flux inside the stand
    /// (reserve into foliage, nectar and fruit, and the respiration already booked in
    /// `respired_out`), so in none of the `expected_*` totals.
    #[serde(default)]
    pub night_spend: [[f64; 3]; Species::COUNT],
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
    /// **Seed counts per species**, indexed by [`Species::index`], cumulative:
    /// diagnostics like the `propagule_*` arrays, in no `expected_*` total.
    ///
    /// - `seeds_landed`: seeds that arrived in a bank.
    /// - `seeds_germinated`: seeds that became stands — recruits from seed.
    /// - `seeds_died`: seeds attrition killed or age sent to litter.
    /// - `seeds_lost`: spore and water seeds that found no ground and fell as litter.
    /// - `clonal_births`: runner and rhizome daughters — recruits without a seed.
    ///
    /// `establishments` is `seeds_germinated + clonal_births` summed over species.
    pub seeds_landed: [u64; Species::COUNT],
    pub seeds_germinated: [u64; Species::COUNT],
    pub seeds_died: [u64; Species::COUNT],
    pub seeds_lost: [u64; Species::COUNT],
    pub clonal_births: [u64; Species::COUNT],
    /// **The canopy gate** ([`SpeciesConfig::establish_light_margin`]), per species, as
    /// cumulative diagnostics in no `expected_*` total: seed-bank checks at which a
    /// species' seeds passed every other gate and the light through the taller foliage
    /// could not pay its newborn's upkeep (the seeds stayed banked); runner faces it
    /// refused; runner packages that found no face and stayed in the parent's parcel.
    #[serde(default)]
    pub canopy_refused: [u64; Species::COUNT],
    #[serde(default)]
    pub runner_canopy_refused: [u64; Species::COUNT],
    #[serde(default)]
    pub runner_kept: [u64; Species::COUNT],
    /// **Runner support** ([`SpeciesConfig::runner_support_s`]), per species: organic
    /// matter (and its mineral) parents moved from their reserves into their runner
    /// daughters' — an internal transfer, in no `expected_*` total — and supports that
    /// ended because the daughter paid its own upkeep (weaned) or because the time ran out
    /// or the parent was gone (ended).
    #[serde(default)]
    pub runner_support_organic: [f64; Species::COUNT],
    #[serde(default)]
    pub runner_support_mineral: [f64; Species::COUNT],
    #[serde(default)]
    pub runner_weaned: [u64; Species::COUNT],
    #[serde(default)]
    pub runner_support_ended: [u64; Species::COUNT],
    /// **Latticevine counters** ([`cover`]), cumulative diagnostics in no `expected_*`
    /// total: founders and hand-seeded vines, sisters rooted, vines dead, faces covered by
    /// spreading, faces lost (terrain, starvation), faces that changed hands in a contest,
    /// and dormancies begun.
    pub vine_births: u64,
    pub vine_sisters: u64,
    pub vine_deaths: u64,
    pub vine_faces_spread: u64,
    pub vine_faces_lost: u64,
    pub vine_faces_contested: u64,
    pub vine_dormancies: u64,
    /// Organic matter the vines sent to litter: leaf fall, spent fruit, dropped runners
    /// and dead vines' reserves. A diagnostic; the litter itself is in the ground totals.
    pub vine_litter: f64,
    /// Ground covers killed by a seedling coming up through them, per
    /// **cover** species: a death, counted in `deaths` too, with its tissue booked to
    /// dead wood and litter like any other.
    pub overtopped: [u64; Species::COUNT],
    /// **Replacement events** by pair, `[cover][seedling]`, both indexed by
    /// [`Species::index`]: each overtopped death above, under the species that came up
    /// through it (velvetpad → springturf, velvetpad → a woody plant, springturf → a woody
    /// plant). A diagnostic counter in no `expected_*` total; each row sums to
    /// `overtopped[cover]`.
    pub overtop_pairs: [[u64; Species::COUNT]; Species::COUNT],
    /// Of the `overtopped` deaths, those a **runner** caused, rooting on the cover's face
    /// ([`SpeciesConfig::runners_overtop`]), per **cover** species. Counted under their
    /// pair in `overtop_pairs` as well.
    #[serde(default)]
    pub runner_displaced: [u64; Species::COUNT],
    /// **Deaths by cause** (`[cause][species]`, [`DeathCause::index`]): every stand death
    /// in `deaths`, once, under what killed it. A diagnostic in no `expected_*` total.
    #[serde(default)]
    pub deaths_by_cause: [[u64; Species::COUNT]; DeathCause::COUNT],
    /// **Burrs**, per seed species, cumulative counts and organic matter — diagnostics in
    /// no `expected_*` total, since the carried seed is still this layer's material
    /// ([`Flora::burrs`]):
    ///
    /// - `burrs_picked`: seeds that latched onto a passing animal, paid by their donor;
    /// - `burrs_dropped`: seeds that fell off a live carrier into the bank where it stood;
    /// - `burrs_dropped_at_carcass`: seeds whose carrier died or left the world, dropped
    ///   into the bank at its last face.
    ///
    /// `picked = carried + dropped + dropped_at_carcass`, seed for seed and organic unit
    /// for organic unit.
    #[serde(default)]
    pub burrs_picked: [u64; Species::COUNT],
    #[serde(default)]
    pub burrs_dropped: [u64; Species::COUNT],
    #[serde(default)]
    pub burrs_dropped_at_carcass: [u64; Species::COUNT],
    #[serde(default)]
    pub burr_picked_organic: [f64; Species::COUNT],
    #[serde(default)]
    pub burr_dropped_organic: [f64; Species::COUNT],
    #[serde(default)]
    pub burr_carcass_organic: [f64; Species::COUNT],
    /// **Curing**, per species: stands that cured, cured stands that woke, and the foliage
    /// curing dropped to the litter (also in `shed_organic`).
    #[serde(default)]
    pub cures: [u64; Species::COUNT],
    #[serde(default)]
    pub cure_wakes: [u64; Species::COUNT],
    #[serde(default)]
    pub cure_shed_organic: [f64; Species::COUNT],
    /// **Food-side diagnostics** per species, indexed by [`Species::index`], cumulative
    /// organic matter, in no `expected_*` total (plant succession, Astra finding 5):
    ///
    /// - `foliage_built`: foliage constructed (growth and reflush) — the regrowth.
    /// - `shed_organic`: foliage senescence sent to the litter — the new shedding.
    /// - `overtopped_organic`: the whole material of the ground covers that died
    ///   overtopped, per **cover** species — the replacement's one-time pulse to dead
    ///   wood and litter.
    /// - `foliage_taken`: foliage the consumers took ([`Flora::take_foliage`]).
    ///
    /// And `litter_taken`, the litter the consumers took ([`Flora::take_litter`]).
    pub foliage_built: [f64; Species::COUNT],
    pub shed_organic: [f64; Species::COUNT],
    pub overtopped_organic: [f64; Species::COUNT],
    pub foliage_taken: [f64; Species::COUNT],
    pub litter_taken: f64,
    /// **Glassfilm budget** (R2a), organic matter, cumulative diagnostics: what
    /// [`Flora::seed_film`] introduced (also in `seeded_organic_in`), what the film grew
    /// (also in `fixed_in`), what it respired (also in `respired_out`), what dying sent to
    /// litter (internal), and what [`Flora::take_film`] handed out (also in
    /// `consumed_organic_out`). Spread moves film between sites and books nothing.
    pub film_seeded: f64,
    pub film_produced: f64,
    pub film_respired: f64,
    pub film_to_litter: f64,
    pub film_consumed: f64,
}

impl FloraLedger {
    pub fn expected_organic(&self) -> f64 {
        self.seeded_organic_in + self.fixed_in + self.deposited_organic_in + self.seep_organic_in
            - self.respired_out
            - self.removed_organic_out
            - self.consumed_organic_out
    }

    pub fn expected_mineral(&self) -> f64 {
        self.seeded_mineral_in + self.deposited_mineral_in + self.seep_mineral_in
            - self.removed_mineral_out
            - self.consumed_mineral_out
    }

    pub fn expected_energy(&self) -> f64 {
        self.seeded_energy_in + self.light_in + self.deposited_energy_in + self.seep_energy_in
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

/// The run of `stands` — sorted by `(site, place)` — on `site`: none, one or two.
pub(crate) fn on_site(stands: &[Stand], site: Site) -> std::ops::Range<usize> {
    let a = stands.partition_point(|s| s.site < site);
    let b = a + stands[a..].iter().take_while(|s| s.site == site).count();
    a..b
}

/// Where the stand holding `layer` on `site` is in `stands`, or where it would go.
pub(crate) fn find_stand(
    config: &FloraConfig,
    stands: &[Stand],
    site: Site,
    layer: FaceLayer,
) -> Result<usize, usize> {
    stands.binary_search_by(|s| config.stand_key(s).cmp(&(site, layer)))
}

/// Read-only access for drawing and inspection.
#[derive(Clone, Copy, Debug)]
pub struct FloraView<'a> {
    pub config: &'a FloraConfig,
    pub tick: u64,
    /// Sorted by `(site, place)` ([`FaceLayer`]): at most one plant and one fungus per
    /// site, the plant first.
    pub stands: &'a [Stand],
    /// Sorted by site. At most one entry per site.
    pub ground: &'a [Ground],
    pub ledger: &'a FloraLedger,
    /// The latticevines and their covered faces: what the tile layer draws from
    /// ([`Cover::draw`]).
    pub cover: &'a Cover,
    /// The stands' cached crown geometry and the columns each can reach
    /// ([`CrownCache`]). Read through [`FloraView::cached_crown`], which checks each entry
    /// against its stand, and [`FloraView::stands_near`]; a view built by hand passes
    /// [`CrownCache::none`] and computes every crown.
    pub crowns: &'a CrownCache,
    /// The seeds riding animals' coats ([`Burr`]).
    pub burrs: &'a [Burr],
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
    layer_iter(config, stand, voxel_m).collect()
}

/// [`layers_of`], one layer at a time and without its vector: the per-tick readers
/// (a mouth scanning every stand in reach, every body, every tick) iterate a stand's
/// layers far more often than they keep them, and the vector was the tick's one
/// allocation per stand read (cache study C,
/// `design/handoffs/voxel-cache-and-pinning-2026-09-24.md`). The same arithmetic in the
/// same order, so the same layers to the bit.
pub fn layer_iter<'c>(config: &'c FloraConfig, stand: &Stand, voxel_m: f64) -> LayerIter<'c> {
    let sc = config.species(stand.species);
    let stage = sc.profile_at(stand.wood);
    LayerIter {
        graze_refuge: sc.graze_refuge,
        min_area_m2: config.min_layer_area_m2,
        layers: stage.layers.iter().enumerate(),
        height_v: sc.crown_height(stand.wood, voxel_m),
        radius_v: sc.crown_radius(stand.wood, voxel_m).max(0.0),
        cap: sc.alpha * stand.wood.max(0.0),
        site_y: stand.site.y,
        layer_stock: stand.layer_stock,
        voxel_m,
        foliage_index: 0,
        crown: None,
    }
}

/// The iterator [`layer_iter`] returns.
#[derive(Clone, Debug)]
pub struct LayerIter<'c> {
    graze_refuge: f64,
    /// [`FloraConfig::min_layer_area_m2`].
    min_area_m2: f64,
    layers: std::iter::Enumerate<std::slice::Iter<'c, Layer>>,
    height_v: f64,
    radius_v: f64,
    cap: f64,
    site_y: u32,
    layer_stock: [f64; MAX_FOLIAGE_LAYERS],
    voxel_m: f64,
    foliage_index: usize,
    /// The stand's cached crown, whose cell offsets replace the per-layer rounding.
    crown: Option<&'c Crown>,
}

/// The most profile layers, trunks included, whose cell offsets a [`Crown`] holds. A
/// stage with more is read uncached.
pub const CROWN_LAYERS: usize = 8;

/// A stand's **crown geometry**, cached where the flora keeps its stands
/// (`design/handoffs/voxel-hot-path-geometry-2026-09-24.md`, item 2).
///
/// Everything about a stand's layers that follows from its species, its wood and the
/// voxel size alone — the profile stage, the crown's height and radius in voxels, the
/// foliage cap `α · W`, [`SpeciesConfig::crown_voxels`], and every layer's disc cell and
/// trunk run as offsets over the site — and nothing that is state: the stocks are read
/// off the stand every time. A mouth scanning every stand for every body on every tick
/// read all of this from scratch, rounding included, although wood changes slowly (and
/// never in a frozen episode).
///
/// [`Crown::of`] is the one computation, with [`layer_iter`]'s own arithmetic in its own
/// order, so a layer read through a crown is [`layers_of`]' layer to the bit. The flora
/// keeps one per stand ([`Flora`]'s `crowns`) and refreshes an entry whenever the
/// stand's wood, species or site no longer match it: at the end of every tick and every
/// command — growth, dieback, a death, a fall, a germination, a founder — and after a
/// snapshot's decode. A reader checks the match itself ([`FloraView::cached_crown`]), so
/// a stale entry is never read, only recomputed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crown {
    site: Site,
    species: Species,
    /// The key: the wood and voxel size this was computed for, as bits.
    wood_bits: u64,
    voxel_bits: u64,
    /// The profile stage's index in the species' `profile`.
    stage: u16,
    /// The stage's layer count, trunks included; the offsets below hold them all when it
    /// is at most [`CROWN_LAYERS`].
    count: u16,
    crown_voxels: u32,
    /// The widest disc of any layer, in whole cells: `floor` of the largest layer
    /// radius in voxels ([`Crown::max_disc_span`]).
    max_disc_span: i64,
    height_v: f64,
    radius_v: f64,
    cap: f64,
    /// Per profile layer: its disc cell's offset over the site (`disc_offset` of its band
    /// top), and a trunk's run of offsets (`trunk_offsets`; zero for foliage).
    disc: [u32; CROWN_LAYERS],
    trunk: [(u32, u32); CROWN_LAYERS],
}

impl Crown {
    /// The crown of `stand` at cells of `voxel_m`, computed from scratch.
    pub fn of(config: &FloraConfig, stand: &Stand, voxel_m: f64) -> Crown {
        let sc = config.species(stand.species);
        let stage = sc.profile_index(stand.wood).min(sc.profile.len().saturating_sub(1));
        let layers = sc.profile.get(stage).map_or(&[][..], |p| &p.layers[..]);
        let height_v = sc.crown_height(stand.wood, voxel_m);
        let radius_v = sc.crown_radius(stand.wood, voxel_m).max(0.0);
        let mut disc = [0u32; CROWN_LAYERS];
        let mut trunk = [(0u32, 0u32); CROWN_LAYERS];
        for (i, layer) in layers.iter().take(CROWN_LAYERS).enumerate() {
            disc[i] = layers::disc_offset(layer.band[1], height_v);
            if !layer.kind.bears_foliage() {
                trunk[i] = layers::trunk_offsets(layer.band, height_v);
            }
        }
        // A layer's disc radius is `layer_iter`'s `(radius · radius_v).max(0)`, and its
        // span in cells that radius floored.
        let max_disc_span = layers
            .iter()
            .map(|l| (l.radius * radius_v).max(0.0).floor() as i64)
            .max()
            .unwrap_or(0);
        Crown {
            site: stand.site,
            species: stand.species,
            wood_bits: stand.wood.to_bits(),
            voxel_bits: voxel_m.to_bits(),
            stage: stage as u16,
            count: layers.len().min(usize::from(u16::MAX)) as u16,
            crown_voxels: sc.crown_voxels(stand.wood, voxel_m),
            max_disc_span,
            height_v,
            radius_v,
            cap: sc.alpha * stand.wood.max(0.0),
            disc,
            trunk,
        }
    }

    /// Whether this is `stand`'s crown: its site, species and wood (to the bit).
    #[inline]
    pub fn matches(&self, stand: &Stand) -> bool {
        self.site == stand.site
            && self.species == stand.species
            && self.wood_bits == stand.wood.to_bits()
    }

    /// [`SpeciesConfig::crown_voxels`] of the stand's wood.
    #[inline]
    pub fn crown_voxels(&self) -> u32 {
        self.crown_voxels
    }

    /// The widest of the crown's layer discs in whole cells, `floor` of its largest
    /// layer radius in voxels: no layer's disc covers a column more than this many cells
    /// from the stand's own in `x` or in `z`. A reader asking which columns a crown
    /// touches can rule out a stand with it before reading a layer.
    #[inline]
    pub fn max_disc_span(&self) -> i64 {
        self.max_disc_span
    }

    /// The crown's height in voxels, [`SpeciesConfig::crown_height`].
    pub fn height_v(&self) -> f64 {
        self.height_v
    }

    /// The crown's half-width in voxels, [`SpeciesConfig::crown_radius`], never negative.
    pub fn radius_v(&self) -> f64 {
        self.radius_v
    }

    /// Whether every layer's offsets are held (the stage has at most [`CROWN_LAYERS`]).
    fn complete(&self) -> bool {
        usize::from(self.count) <= CROWN_LAYERS
    }

    /// Every layer of `stand` — which must be the stand this crown [`matches`] —
    /// bottom-up, trunks included: [`layer_iter`] with the geometry read from here and
    /// the stocks from the stand.
    ///
    /// [`matches`]: Crown::matches
    #[inline]
    pub fn layers<'c>(&'c self, config: &'c FloraConfig, stand: &Stand) -> LayerIter<'c> {
        debug_assert!(self.matches(stand));
        let voxel_m = f64::from_bits(self.voxel_bits);
        if !self.complete() {
            return layer_iter(config, stand, voxel_m);
        }
        let sc = config.species(stand.species);
        LayerIter {
            graze_refuge: sc.graze_refuge,
            min_area_m2: config.min_layer_area_m2,
            layers: sc.profile[usize::from(self.stage)].layers.iter().enumerate(),
            height_v: self.height_v,
            radius_v: self.radius_v,
            cap: self.cap,
            site_y: stand.site.y,
            layer_stock: stand.layer_stock,
            voxel_m,
            foliage_index: 0,
            crown: Some(self),
        }
    }
}

/// A flora's cached crowns: one [`Crown`] per stand, entry for entry, and a **reach
/// index** — per column of the world, every stand whose widest layer disc
/// ([`Crown::max_disc_span`]) boxes it — so a mouth asks for the stands near its columns
/// instead of scanning them all (`design/handoffs/voxel-hot-path-geometry-2026-09-24.md`,
/// items 2 and 4).
///
/// Derived, never saved, refreshed with the crowns: an entry is recomputed when its stand
/// no longer [`matches`](Crown::matches) it, and the index rebuilt when a stand is added
/// or removed or a crown's site or widest span moves. The index needs the world's width
/// and depth, so it is built by a tick or a command and is absent after a decode until
/// the next of either; [`FloraView::stands_near`] says so, and the caller scans.
#[derive(Clone, Debug, Default)]
pub struct CrownCache {
    crowns: Vec<Crown>,
    /// The index's world, and the stand count it was built for.
    width: u32,
    depth: u32,
    indexed: Option<usize>,
    /// Per column `z * width + x`, its first entry in `near`; `plane + 1` entries.
    col_start: Vec<u32>,
    /// Stand indices, ascending within each column.
    near: Vec<u32>,
}

impl CrownCache {
    /// No crowns and no index, for a view built by hand: every reader computes.
    pub fn none() -> &'static CrownCache {
        static NONE: CrownCache = CrownCache {
            crowns: Vec::new(),
            width: 0,
            depth: 0,
            indexed: None,
            col_start: Vec::new(),
            near: Vec::new(),
        };
        &NONE
    }

    /// Bring the entries up to `stands`, reusing every entry that still matches and
    /// computing the rest (both lists are sorted by `(site, place)`, so an insertion or a removal in
    /// the middle moves no other entry out of reach), and rebuild the reach index for a
    /// world of `dims` if anything it records moved. `dims` is `None` where no world is in
    /// hand, and the index is then dropped.
    fn refresh(&mut self, config: &FloraConfig, stands: &[Stand], dims: Option<(u32, u32)>) {
        let voxel_bits = config.voxel_m.to_bits();
        let current = |c: &Crown, s: &Stand| c.matches(s) && c.voxel_bits == voxel_bits;
        let mut moved = self.crowns.len() != stands.len();
        if moved || !self.crowns.iter().zip(stands).all(|(c, s)| current(c, s)) {
            let old = std::mem::take(&mut self.crowns);
            let mut k = 0;
            self.crowns.reserve(stands.len());
            for (i, stand) in stands.iter().enumerate() {
                // Both lists in `(site, place)` order: a face's fungus finds its own entry.
                let key = config.stand_key(stand);
                while k < old.len() && (old[k].site, config.face_layer(old[k].species)) < key {
                    k += 1;
                }
                let crown = match old.get(k) {
                    Some(c) if current(c, stand) => *c,
                    _ => Crown::of(config, stand, config.voxel_m),
                };
                // The index records each stand's site and widest span, by position.
                moved |= old
                    .get(i)
                    .is_none_or(|o| o.site != crown.site || o.max_disc_span != crown.max_disc_span);
                self.crowns.push(crown);
            }
        }
        match dims {
            None => self.indexed = None,
            Some((w, d)) => {
                if moved || self.indexed != Some(stands.len()) || (self.width, self.depth) != (w, d) {
                    self.build_index(w, d);
                }
            }
        }
    }

    /// Every stand's box of columns, `site ± max_disc_span` in wrapped `x` and in `z`.
    fn build_index(&mut self, width: u32, depth: u32) {
        let (w, d) = (i64::from(width), i64::from(depth));
        let plane = width as usize * depth as usize;
        // A column list per stand: its box's distinct columns.
        let each = |c: &Crown, f: &mut dyn FnMut(usize)| {
            if w == 0 || d == 0 {
                return;
            }
            let span = c.max_disc_span.clamp(0, w.max(d));
            let (sx, sz) = (i64::from(c.site.x), i64::from(c.site.z));
            // The box's distinct columns, `x` wrapped, in the order they were listed.
            let (x0, xn) = if 2 * span + 1 >= w {
                (0, w)
            } else {
                (sx - span, 2 * span + 1)
            };
            for z in (sz - span).max(0)..=(sz + span).min(d - 1) {
                for x in x0..x0 + xn {
                    f(z as usize * width as usize + x.rem_euclid(w) as usize);
                }
            }
        };
        let mut count = vec![0u32; plane + 1];
        for c in &self.crowns {
            each(c, &mut |col| count[col + 1] += 1);
        }
        for k in 0..plane {
            count[k + 1] += count[k];
        }
        let mut fill = count.clone();
        let mut near = vec![0u32; count[plane] as usize];
        for (i, c) in self.crowns.iter().enumerate() {
            each(c, &mut |col| {
                near[fill[col] as usize] = i as u32;
                fill[col] += 1;
            });
        }
        self.width = width;
        self.depth = depth;
        self.col_start = count;
        self.near = near;
        self.indexed = Some(self.crowns.len());
    }
}

impl Iterator for LayerIter<'_> {
    type Item = StandLayer;

    fn next(&mut self) -> Option<StandLayer> {
        let voxel_m = self.voxel_m;
        let (height_v, radius_v, cap) = (self.height_v, self.radius_v, self.cap);
        let base = f64::from(self.site_y);
        for (index, layer) in self.layers.by_ref() {
            let is_foliage = layer.kind.bears_foliage();
            let fi = if is_foliage {
                let i = self.foliage_index;
                self.foliage_index += 1;
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
            let (disc, trunk) = match self.crown {
                Some(c) => (c.disc[index], c.trunk[index]),
                None => (
                    layers::disc_offset(layer.band[1], height_v),
                    if is_foliage {
                        (0, 0)
                    } else {
                        layers::trunk_offsets(layer.band, height_v)
                    },
                ),
            };
            let cell = i64::from(self.site_y) + i64::from(disc);
            let cells = if is_foliage {
                (cell, cell)
            } else {
                let (lo, hi) = trunk;
                (
                    i64::from(self.site_y) + i64::from(lo),
                    i64::from(self.site_y) + i64::from(hi),
                )
            };
            return Some(StandLayer {
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
                stock: fi.map_or(0.0, |i| self.layer_stock[i]),
                floor: if is_foliage {
                    self.graze_refuge * layer.share * cap
                } else {
                    0.0
                },
                porosity: layer.porosity,
                area_m2: (std::f64::consts::PI * r_m * r_m).max(self.min_area_m2),
            });
        }
        None
    }
}

impl<'a> FloraView<'a> {
    /// The **foliage-bearing** layers of `stand`, bottom-up: the contract's
    /// `FloraView::layers(stand)`, yielding each layer's band in metres, radius in
    /// metres, kind, stock and porosity. Trunk layers are not here — ask
    /// [`layers_of`] for the whole profile, which the presenter and the cone do.
    ///
    /// The stocks it yields **sum to the stand's `foliage`**, always.
    ///
    /// Read through the stand's cached [`Crown`] when the view holds a current one.
    pub fn layers(&self, stand: &Stand) -> impl Iterator<Item = StandLayer> + use<'a> {
        self.layer_iter_of(stand)
            .filter(|l| l.kind.bears_foliage())
    }

    /// Every layer of `stand`, through its cached crown when there is a current one.
    fn layer_iter_of(&self, stand: &Stand) -> LayerIter<'a> {
        match self.cached_crown(stand) {
            Some(crown) => crown.layers(self.config, stand),
            None => layer_iter(self.config, stand, self.config.voxel_m),
        }
    }

    /// The **current** cached crown of `stand` — which must be one of this view's own
    /// stands, borrowed from [`FloraView::stands`], not a copy — or `None` when the view
    /// holds none for it (a copy, a hand-built view, an entry the flora has not refreshed).
    #[inline]
    pub fn cached_crown(&self, stand: &Stand) -> Option<&'a Crown> {
        // Where `stand` sits in `stands`, by address: a reader iterating the view's stands
        // hands back the view's own references, and anything else is not found.
        let base = self.stands.as_ptr() as usize;
        let at = (stand as *const Stand as usize).checked_sub(base)?;
        let size = std::mem::size_of::<Stand>();
        if at % size != 0 {
            return None;
        }
        let i = at / size;
        if i >= self.stands.len() {
            return None;
        }
        let crown = self.crowns.crowns.get(i)?;
        (crown.matches(stand) && crown.voxel_bits == self.config.voxel_m.to_bits())
            .then_some(crown)
    }

    /// The indices in [`FloraView::stands`] of every stand whose crown could reach the
    /// column `(x, z)` of a world `width × depth` — every stand whose widest layer disc
    /// ([`Crown::max_disc_span`]) boxes that column, ascending — or `None` when the view
    /// holds no current index for that world, and the caller has to scan every stand. A
    /// column off the strip (`x` outside `0..width`, `z` past the end) has none.
    #[inline]
    pub fn stands_near(&self, x: i64, z: u32, width: u32, depth: u32) -> Option<&'a [u32]> {
        let cache = self.crowns;
        if cache.indexed != Some(self.stands.len()) || (cache.width, cache.depth) != (width, depth) {
            return None;
        }
        if !(0..i64::from(width)).contains(&x) || z >= depth {
            return Some(&[]);
        }
        let col = z as usize * width as usize + x as usize;
        let (a, b) = (cache.col_start[col] as usize, cache.col_start[col + 1] as usize);
        Some(&cache.near[a..b])
    }

    /// [`SpeciesConfig::crown_voxels`] of `stand`'s wood at the view's voxel size, read
    /// from its cached crown when there is a current one.
    pub fn crown_voxels(&self, stand: &Stand) -> u32 {
        match self.cached_crown(stand) {
            Some(crown) => crown.crown_voxels(),
            None => self
                .config
                .species(stand.species)
                .crown_voxels(stand.wood, self.config.voxel_m),
        }
    }

    /// [`FloraView::layers`] of the **plant** on `site`; empty if there is none.
    pub fn layers_at(&self, site: Site) -> impl Iterator<Item = StandLayer> + use<> {
        self.layers_in(site, FaceLayer::Plant)
    }

    /// [`FloraView::layers`] of the stand holding `layer` on `site`; empty if none does.
    pub fn layers_in(&self, site: Site, layer: FaceLayer) -> impl Iterator<Item = StandLayer> + use<> {
        let stand = self.stand_in(site, layer).copied();
        stand
            .map(|s| layers_of(self.config, &s, self.config.voxel_m))
            .unwrap_or_default()
            .into_iter()
            .filter(|l| l.kind.bears_foliage())
    }

    /// Every layer of `stand`, trunks included, bottom-up.
    pub fn profile_layers(&self, stand: &Stand) -> Vec<StandLayer> {
        self.layer_iter_of(stand).collect()
    }

    /// [`FloraView::profile_layers`] appended to `out`, without a vector per stand.
    pub fn profile_layers_into(&self, stand: &Stand, out: &mut Vec<StandLayer>) {
        out.extend(self.layer_iter_of(stand));
    }

    /// The **plant** on `site` ([`FaceLayer::Plant`]): what a face's occupant has always
    /// meant to its readers. A glowcap under it is [`FloraView::fungus_at`].
    pub fn stand_at(&self, site: Site) -> Option<&'a Stand> {
        self.stand_in(site, FaceLayer::Plant)
    }

    /// The **fungus** on `site` ([`FaceLayer::Fungus`]).
    pub fn fungus_at(&self, site: Site) -> Option<&'a Stand> {
        self.stand_in(site, FaceLayer::Fungus)
    }

    /// The stand holding `layer` on `site`.
    pub fn stand_in(&self, site: Site, layer: FaceLayer) -> Option<&'a Stand> {
        find_stand(self.config, self.stands, site, layer)
            .ok()
            .map(|i| &self.stands[i])
    }

    /// Every stand on `site` — none, one or two — the plant first.
    pub fn stands_on(&self, site: Site) -> &'a [Stand] {
        &self.stands[on_site(self.stands, site)]
    }

    pub fn ground_at(&self, site: Site) -> Option<&'a Ground> {
        self.ground
            .binary_search_by_key(&site, |g| g.site)
            .ok()
            .map(|i| &self.ground[i])
    }

    /// The glassfilm stock on `site`, organic matter; 0 where there is none (R2a).
    pub fn film_at(&self, site: Site) -> f64 {
        self.ground_at(site).map_or(0.0, |g| g.film)
    }

    /// Every site holding film, with its stock, in site order (R2a).
    pub fn film_sites(&self) -> impl Iterator<Item = (Site, f64)> + use<'a> {
        self.ground
            .iter()
            .filter(|g| g.film > 0.0)
            .map(|g| (g.site, g.film))
    }

    /// Organic matter in every living and dead stock, the seed banks and the latticevines
    /// included. The site's mineral pool is not organic matter and is not in here.
    pub fn organic(&self) -> f64 {
        self.burrs.iter().map(|b| b.organic).sum::<f64>()
            + self.cover.organic(&self.config.latticevine)
            + self.stands.iter().map(|s| s.material()).sum::<f64>()
            + self
                .ground
                .iter()
                .map(|g| {
                    g.litter
                        + g.dead_wood
                        + g.carrion
                        + g.film
                        + g.vine_seed_organic()
                        + g.seeds.iter().map(|c| c.organic).sum::<f64>()
                })
                .sum::<f64>()
    }

    /// Mineral in every stock: the sites' pools, the mineral held in litter, dead wood
    /// and the seed banks, and the mineral standing in living tissue.
    pub fn mineral(&self) -> f64 {
        self.burrs.iter().map(|b| b.mineral).sum::<f64>()
            + self.stands.iter().map(|s| s.mineral).sum::<f64>()
            + self.cover.vines().iter().map(|v| v.mineral).sum::<f64>()
            + self
                .ground
                .iter()
                .map(|g| {
                    g.mineral
                        + g.litter_mineral
                        + g.dead_wood_mineral
                        + g.carrion_mineral
                        + g.film_mineral
                        + g.vine_seeds.map_or(0.0, |l| l.mineral)
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
            // Since the grazing refuge the sum is of what stands above each layer's floor
            // ([`StandLayer::edible`]), which is all a mouth can take.
            let mut reachable = 0.0;
            let mut radius = 0.0f64;
            for layer in self.layers(stand) {
                let edible = layer.edible();
                if layer.cell > ceiling || !(edible > 0.0) {
                    continue;
                }
                reachable += edible;
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
    /// own species field and not the soil-water root box's.
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

    /// The germination predicate for **any** species, gate by gate, with the substrate
    /// read off this layer's own ground (a `VoxelView` holds no dead wood and no litter),
    /// so this is the form a harness or a diagnosis wants once a saprotroph is in the
    /// world.
    pub fn establishment_gates(
        &self,
        world: &VoxelView<'_>,
        site: Site,
        species: Species,
    ) -> Gates {
        let sc = self.config.species(species);
        let (dead_wood, litter) = step::substrate_pools_in_box(world, self.ground, site, sc);
        let sky_visibility = world.sky_visibility(site.x as i64, site.y, site.z);
        step::establishment_gates_with_sky(world, site, sc, sky_visibility, dead_wood, litter)
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

    /// Energy in every living and dead stock, the latticevines included.
    pub fn energy(&self) -> f64 {
        self.burrs
            .iter()
            .map(|b| self.config.species(b.species).energy_density * b.organic)
            .sum::<f64>()
            + self.config.latticevine.energy_density * self.cover.organic(&self.config.latticevine)
            + self
                .stands
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
                        + g.film_energy
                        + self.config.latticevine.energy_density * g.vine_seed_organic()
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
    /// [`Ground::dead_wood`]: a **log**. A saprotroph eats dead wood, and a **fresh world
    /// has none**: the harness has to lay declared logs before a fungus can be introduced at
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

/// What killed a stand ([`FloraLedger::deaths_by_cause`], [`DeathRecord`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeathCause {
    /// Its wood fell below `alive_min`: unpaid maintenance, dieback (light, water, a
    /// grazer's crop, waterlogging — whatever starved it).
    Starved,
    /// Standing water over its face deeper than its `drown_depth_m`.
    Drowned,
    /// A seedling came up through it (a ground cover).
    Overtopped,
    /// A runner rooted on its face (a ground cover, [`SpeciesConfig::runners_overtop`]).
    Displaced,
    /// Cured past its species' `cure_max_s`: the drought outlasted the dormancy.
    Drought,
}

impl DeathCause {
    pub const ALL: [DeathCause; 5] = [
        DeathCause::Starved,
        DeathCause::Drowned,
        DeathCause::Overtopped,
        DeathCause::Displaced,
        DeathCause::Drought,
    ];
    pub const COUNT: usize = DeathCause::ALL.len();

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            DeathCause::Starved => "starved",
            DeathCause::Drowned => "drowned",
            DeathCause::Overtopped => "overtopped",
            DeathCause::Displaced => "displaced",
            DeathCause::Drought => "drought",
        }
    }
}

/// One stand's death, as the tick saw it: **observation, not state**, like
/// [`DeliveryReceipt`] — cleared at the start of every [`Flora::step`], never saved. A
/// stand that leaves without one of these was removed (terrain, a command), not killed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeathRecord {
    pub id: u64,
    pub site: Site,
    pub species: Species,
    pub cause: DeathCause,
}

/// **A burr**: one seed package riding an animal's coat ([`Flora::carry_burrs`]). Still
/// this layer's organic matter, mineral and energy (the species' `energy_density`), in
/// [`FloraView::organic`] and its siblings, until it falls into a seed bank.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Burr {
    /// The carrier's animal id.
    pub carrier: u64,
    pub species: Species,
    pub organic: f64,
    pub mineral: f64,
    /// The donor stand's id and face.
    pub donor: u64,
    pub from: Site,
    pub picked_tick: u64,
    /// The distance it holds on for, metres walked, drawn at pickup from the species'
    /// retention curve; and the distance walked so far.
    pub hold_m: f64,
    pub walked_m: f64,
}

/// One animal as the burr pass sees it ([`Flora::carry_burrs`]): who it is, the face it
/// stands over, where its body is (metres, `x` along the ring), whether it is on the
/// ground (a flyer in the air, a climber on a stem or a rider picks nothing up), and its
/// coat factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Carrier {
    pub id: u64,
    pub site: Site,
    pub x_m: f64,
    pub z_m: f64,
    pub grounded: bool,
    pub coat: f64,
}

/// Where a carrier was last tick, kept by the burr pass to measure the distance walked
/// and to see a step onto a new face.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CarrierMark {
    pub id: u64,
    pub site: Site,
    pub x_m: f64,
    pub z_m: f64,
}

/// How a package reached its recipient ([`DeliveryReceipt::via`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Via {
    /// A seed landed by the species' dispersal mode.
    Seed,
    /// A runner daughter rooted.
    Runner,
    /// A burr fell off a carrier (or its carcass).
    Burr,
}

/// One package leaving one donor for one site, exactly as `propagate` sent it:
/// **observation, not state**.
///
/// A harness watching the seed banks cannot tell who sent what where: two
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
    /// The donor's face, so a reader can measure how far the package went.
    pub origin: Site,
    /// How it travelled.
    pub via: Via,
}

/// The plant layer. Owns its stands and ground stocks; borrows the world per call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Flora {
    config: FloraConfig,
    tick: u64,
    stands: Vec<Stand>,
    ground: Vec<Ground>,
    ledger: FloraLedger,
    /// The latticevines and their covered faces ([`cover`]).
    cover: Cover,
    /// Sky visibility per site, sorted by site: pure terrain geometry, so it is dropped
    /// whole when the world's `terrain_version` moves and refilled lazily. A `Vec` with
    /// a binary search, never a `HashMap`: this layer iterates nothing unordered.
    sky: Vec<(Site, f64)>,
    /// The `terrain_version` `sky` was filled against, `None` before the first tick.
    sky_version: Option<u64>,
    /// This tick's [`DeliveryReceipt`]s, in the order `propagate` sent them. Cleared at the
    /// start of every tick: **observation, not state** (see [`DeliveryReceipt`]).
    deliveries: Vec<DeliveryReceipt>,
    /// The seed bank's **check wheel** (`seeds.rs`): the banked sites, one bucket per phase
    /// of the check period, so a tick visits only the sites due on it. Derived from
    /// `ground` and rebuilt from it whenever it is missing, so it is not saved.
    #[serde(skip)]
    pub(crate) bank_wheel: Vec<Vec<Site>>,
    /// Whether it was raining at the last seed-bank phase: the tick it stops is the
    /// shower's end, when every bank is tested at once.
    #[serde(default)]
    pub(crate) was_raining: bool,
    /// A study's record of one species' germination attempts, site by site
    /// ([`Flora::trace_attempts`]). Observation only, off unless asked for, never saved.
    #[serde(skip)]
    pub(crate) attempts: Option<Box<seeds::AttemptTrace>>,
    /// Every stand's crown geometry, entry for entry with `stands` ([`Crown`]). Derived
    /// and not saved: refreshed at the end of every tick and command and after a decode.
    #[serde(skip)]
    crowns: CrownCache,
    /// The drink's per-cell scratch, kept between ticks so a tick does not allocate and
    /// zero a world-sized array, and — under the overlapped tick — holding the drink this
    /// tick **planned** against a read copy until [`Flora::settle_drink`] applies it
    /// to the live world. Not state: not saved, and a clone starts empty.
    #[serde(skip)]
    pub(crate) drink: step::DrinkSlot,
    /// **Burrs** riding animals' coats ([`Flora::carry_burrs`]), in pickup order: this
    /// layer's material in transit, saved with it.
    #[serde(default)]
    pub(crate) burrs: Vec<Burr>,
    /// Every carrier's place at the last burr pass, sorted by id.
    #[serde(default)]
    pub(crate) burr_marks: Vec<CarrierMark>,
    /// This tick's stand deaths ([`DeathRecord`]): **observation, not state**, cleared at
    /// the start of every tick, never saved.
    #[serde(skip)]
    pub(crate) death_log: Vec<DeathRecord>,
    /// World ticks since the last plant step ([`FloraConfig::step_every`]): the next step
    /// covers them all, so its `dt` is exact across a load or a changed `step_every`.
    #[serde(default)]
    pub(crate) since_step: u32,
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
            cover: Cover::default(),
            sky: Vec::new(),
            sky_version: None,
            deliveries: Vec::new(),
            bank_wheel: Vec::new(),
            was_raining: false,
            attempts: None,
            crowns: CrownCache::default(),
            drink: step::DrinkSlot::default(),
            burrs: Vec::new(),
            burr_marks: Vec::new(),
            death_log: Vec::new(),
            since_step: 0,
        }
    }

    /// Bring every stand's cached [`Crown`] up to date, and the reach index for `world`'s
    /// columns: after anything that can add, remove, move or regrow a stand. With no
    /// world in hand (a decode) the index is dropped until the next tick or command.
    pub(crate) fn refresh_crowns(&mut self, world: Option<&World>) {
        let dims = world.map(|w| (w.config().width, w.config().depth));
        self.crowns.refresh(&self.config, &self.stands, dims);
    }

    pub fn config(&self) -> &FloraConfig {
        &self.config
    }

    /// **A study's configuration replacement** on a loaded layer: `config` replaces the
    /// saved one whole, after [`FloraConfig::validate`], and every stand's tissue is
    /// re-binned into its (possibly new) profile and the crowns refreshed. For a diagnostic
    /// that continues a saved world under other parameters; no host calls it.
    #[doc(hidden)]
    /// Record, site by site, every seed-bank check at which `species` held a whole seed —
    /// what stopped it (a cover it may not take, a gate, the lottery, the overtopping
    /// draw, an arm lost before it sprouted) or that it was born — from now on; `None`
    /// stops and drops the record. A diagnostic for a study (plant succession's attempt
    /// classification): it changes nothing in the tick and is never saved.
    pub fn trace_attempts(&mut self, species: Option<Species>) {
        self.attempts = species.map(|species| {
            Box::new(seeds::AttemptTrace {
                species,
                sites: Default::default(),
            })
        });
    }

    /// The record [`Flora::trace_attempts`] keeps, if one is being kept.
    pub fn attempts(&self) -> Option<&seeds::AttemptTrace> {
        self.attempts.as_deref()
    }

    pub fn replace_config(&mut self, config: FloraConfig) -> Result<(), String> {
        config.validate()?;
        self.config = config;
        for s in &mut self.stands {
            s.resync_layers(self.config.species(s.species));
        }
        self.refresh_crowns(None);
        Ok(())
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
    /// seed banks afterwards.
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
            cover: &self.cover,
            crowns: &self.crowns,
            burrs: &self.burrs,
        }
    }

    /// **Burrs** (epizoochory, `seeds.rs`'s `carry_burrs`): the animal layer hands over
    /// every animal once per tick, after it has moved; carried seeds walk, fall off into
    /// the bank where their carrier stands (or where it died), and a carrier stepping onto
    /// a donor's face on the ground may pick one up. Applied between plant ticks, like a
    /// consumer's withdrawals; its receipts join this tick's [`Flora::deliveries`].
    pub fn carry_burrs(&mut self, world: &World, carriers: &[Carrier]) {
        seeds::carry_burrs(self, world, carriers);
    }

    /// The seeds riding animals' coats.
    pub fn burrs(&self) -> &[Burr] {
        &self.burrs
    }

    /// This tick's stand deaths, with their causes. Cleared at the start of the next
    /// [`Flora::step`]: **observation, not state** ([`DeathRecord`]).
    pub fn death_log(&self) -> &[DeathRecord] {
        &self.death_log
    }

    /// Advance one tick against the world's current state, as **one plant step of that
    /// tick** whatever [`FloraConfig::step_every`] says (a fixture's clock). Call after
    /// `World::step`. Withdraws root water through `Command::WithdrawPore`; that is the
    /// only way this layer changes the world. The order within the tick is `step`'s
    /// module doc. A world's schedule calls [`Flora::step_clocked`].
    pub fn step(&mut self, world: &mut World) {
        step::step(self, world, 1, false);
    }

    /// [`Flora::step`] with the per-stand reads of light and water split over up to
    /// `threads` workers of the `bevy_tasks` compute pool (with the `parallel` feature).
    /// The chunks are folded back in stand order, so every number is the serial step's.
    pub fn step_with(&mut self, world: &mut World, threads: usize) {
        step::step(self, world, threads, false);
    }

    /// **One world tick on the plant clock** ([`FloraConfig::step_every`]): the tick
    /// counter moves and the tick's receipts and death log clear every tick, and every
    /// `step_every`-th tick runs one plant step covering every tick since the last
    /// (`step_every · DT` seconds). Between plant steps the layer's stocks only change
    /// through what animals take and deposit. What the simulation's schedule calls.
    pub fn step_clocked(&mut self, world: &mut World, threads: usize) {
        step::step(self, world, threads, true);
    }

    /// [`Flora::step_planned`] on the plant clock: a tick that is not a plant step plans
    /// nothing, and [`Flora::take_drink_plan`] then returns `None`.
    pub fn step_planned_clocked(&mut self, world: &World, threads: usize) {
        step::step_planned(self, world, threads, true);
    }

    /// **[`Flora::step_planned_clocked`] in parts**, for a schedule that runs the plant
    /// step's reads beside the animals' (`design/7_Research/tick-stage-dependencies-2026-10-04.md`,
    /// proposal 2): `begin_planned_clocked`, then [`Flora::read_planned`] (through `&self`,
    /// so the animals' read side may hold the layer too), then [`Flora::plan_drink`] —
    /// after which [`Flora::take_drink_plan`] holds the plan — then
    /// [`Flora::finish_planned`]. Nothing else of this layer's may run between them except
    /// the animals' writes before the reads (their maintenance's voiding) and reads beside
    /// them; the parts in order are `step_planned_clocked`.
    ///
    /// This part: the clock, then the step's prep (terrain pruning, the cover's prep,
    /// drowning, the vines' drink wants). `None` on a tick that is not a plant step, which
    /// then only moved the clock.
    pub fn begin_planned_clocked(&mut self, world: &World, threads: usize) -> Option<PlantStep> {
        step::begin_planned(self, world, threads)
    }

    /// The plant step's reads: light and the drink's plan. Writes only `step`, which holds
    /// the layer's sky cache and drink scratch until [`Flora::plan_drink`]. Reads `stands`,
    /// the config and the world; a caller may read the layer beside it.
    pub fn read_planned(&self, step: &mut PlantStep, world: &World, threads: usize) {
        cubarium_voxel::voxel_phase!(FloraStep, { step.read(self, world, threads) });
    }

    /// The drink planned off the read: each stand's and vine's share credited, the sky
    /// cache and scratch back in the layer, and the plan held for
    /// [`Flora::take_drink_plan`].
    pub fn plan_drink(&mut self, step: &mut PlantStep, world: &World, threads: usize) {
        step::plan_drink(self, step, world, threads);
    }

    /// The rest of the step: defences relax, feed, growth, the cover, decomposition and
    /// the film, the seed bank, propagation and the crowns.
    pub fn finish_planned(&mut self, step: PlantStep, world: &World, threads: usize) {
        step::finish_planned(self, step, world, threads);
    }

    /// **The overlapped tick's plant step** (`design/handoffs/voxel-phase-overlap-2026-09-24.md`):
    /// [`Flora::step_with`] reading `world` — a read copy of the live world as it stood
    /// when the tick began — and writing nothing to it. The one write the plant layer makes
    /// to the world, the drink's `Command::WithdrawPore`s, is **planned** here, off the
    /// copy, exactly as [`Flora::step_with`] would take it, and held until
    /// [`Flora::settle_drink`] applies it to the live world at the tick's barrier. Every
    /// phase after the drink reads the copy, so it sees the pore water before this tick's
    /// withdrawal (the chained tick's later phases see it after).
    ///
    /// `transpired_m3` is booked at the settle, voxel by voxel, as the world books it.
    pub fn step_planned(&mut self, world: &World, threads: usize) {
        step::step_planned(self, world, threads, false);
    }

    /// Apply the drink [`Flora::step_planned`] planned to the live `world`: one
    /// `Command::WithdrawPore` per voxel for `min(planned, what the cell holds above its
    /// wilting point now)`, in ascending voxel order. What the water took out of a cell
    /// since the plan is a **shortfall**: it is never withdrawn, never booked as
    /// transpired, and comes off the observational `water_m3` (a stand) or `drank_m3` (a
    /// latticevine) of whoever asked, in proportion to what each asked — their income this
    /// tick was already earned on the plan, the one-tick lag. A no-op with nothing planned.
    pub fn settle_drink(&mut self, world: &mut World) {
        step::settle_drink(self, world);
    }

    /// Whether a planned drink is waiting for [`Flora::settle_drink`].
    pub fn drink_pending(&self) -> bool {
        self.drink.0.pending
    }

    /// [`Flora::settle_drink`] in three steps, for a schedule that lets the live world take
    /// the drink while the rest of the tick's reads still run: take the plan out of the
    /// layer (`None` with nothing planned), [`DrinkPlan::withdraw`] it from the world, then
    /// hand it back here. Between the take and the finish this layer's `transpired_m3`
    /// lacks this tick's drink, and the next plan needs the finish first.
    pub fn take_drink_plan(&mut self) -> Option<DrinkPlan> {
        step::take_plan(self)
    }

    /// Book a [`DrinkPlan`] the world has withdrawn: `transpired_m3` receipt by receipt in
    /// the world's order, each shortfall off whoever asked, and the scratch back in the
    /// layer.
    pub fn finish_drink(&mut self, plan: DrinkPlan) {
        step::finish(self, plan);
    }

    /// One tick of the **dead pools only**: litter, dead wood and carrion decompose at
    /// their own rates off the tick-start stocks, respired and released exactly as in
    /// [`Flora::step`]'s decomposition phase, booked in the same ledger lines. No clock,
    /// growth, shedding, seed bank or propagation. A frozen world that replays its
    /// production through [`Flora::deposit`] calls this each tick so what it was handed
    /// rots as it would have.
    pub fn decompose_dead_pools(&mut self) {
        step::decompose_only(self);
    }

    /// Apply a command now. Returns whether it was accepted.
    pub fn apply(&mut self, world: &World, command: Command) -> bool {
        let applied = self.apply_command(world, command);
        // A founder or a clear moves the stand list; the crowns follow it.
        self.refresh_crowns(Some(world));
        applied
    }

    /// [`Flora::apply`]'s commands.
    fn apply_command(&mut self, world: &World, command: Command) -> bool {
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
                // Both places: a `Clear` removes every stand the site held.
                let run = on_site(&self.stands, site);
                if run.is_empty() {
                    return false;
                }
                for s in self.stands.drain(run).collect::<Vec<_>>() {
                    // The parcel goes out with the stand: it is organic matter this layer
                    // holds, and a `Clear` removes everything the site held.
                    let organic = s.material();
                    self.ledger.removed_organic_out += organic;
                    self.ledger.removed_mineral_out += s.mineral;
                    self.ledger.removed_energy_out +=
                        self.config.species(s.species).energy_density * organic;
                }
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
        // Only its own place: a glowcap founder goes under a plant, a plant over a glowcap.
        let Err(at) = find_stand(&self.config, &self.stands, site, self.config.face_layer(species)) else {
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
            defence: 0.0,
            nectar: 0.0,
            fruit: 0.0,
            fruit_age_s: 0.0,
            fruit_rest_s: 0.0,
            cured: false,
            dry_s: 0.0,
            support: None,
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
    /// the stand's `P` holds above its grazing floor ([`SpeciesConfig::graze_refuge`],
    /// [`StandLayer::edible`]), never its wood and never its reserve.
    ///
    /// The mineral leaves by the same fraction rule every other outflow uses
    /// ([`Stand::mineral`]): a transfer of a fraction of the stand's material takes that
    /// fraction of its mineral stock. The energy is the tissue's own `energy_density` times
    /// the organic matter taken.
    ///
    /// `None`, booking nothing at all, when there is nothing to take: no stand on the site,
    /// a stand with no foliage left above its floor, or a `want` that is not a positive
    /// finite number.
    /// `Some` therefore always carries a strictly positive `organic`.
    ///
    /// Applied **between** ticks, like a [`Command`]: nothing here reads or moves the
    /// world, and the tick that follows sees the smaller `P` in its own income
    /// (`step`'s step 6). Taking a stand's whole canopy does not kill it — death is
    /// `wood < alive_min` and wood is untouched — it starves it, which is the producer
    /// response the crate doc describes.
    ///
    /// The stand is the site's **plant** ([`FaceLayer::Plant`]); a fungus's cap tissue is
    /// [`Flora::take_foliage_of`].
    pub fn take_foliage(&mut self, site: Site, want: f64) -> Option<Taken> {
        self.withdraw_foliage(site, FaceLayer::Plant, want, None).map(|t| t.taken)
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
    ///
    /// The stand is the site's **plant**; [`Flora::take_foliage_of`] names the place.
    pub fn take_foliage_in_layers(
        &mut self,
        site: Site,
        want: f64,
        layers: &std::ops::RangeInclusive<i64>,
    ) -> Option<TakenFoliage> {
        self.withdraw_foliage(site, FaceLayer::Plant, want, Some(layers))
    }

    /// [`Flora::take_foliage_in_layers`] off the stand holding `place` on `site`: a bite's
    /// food names its place — cap tissue is the fungus's, foliage the plant's — so a mouth
    /// on a face holding both takes from the one its food is.
    pub fn take_foliage_of(
        &mut self,
        site: Site,
        place: FaceLayer,
        want: f64,
        layers: &std::ops::RangeInclusive<i64>,
    ) -> Option<TakenFoliage> {
        self.withdraw_foliage(site, place, want, Some(layers))
    }

    /// The one foliage withdrawal: lowest layer first, each layer giving at most its
    /// [`StandLayer::edible`] stock, so no bite takes a layer below its grazing floor.
    /// Everything that takes foliage comes through here.
    fn withdraw_foliage(
        &mut self,
        site: Site,
        place: FaceLayer,
        want: f64,
        layers: Option<&std::ops::RangeInclusive<i64>>,
    ) -> Option<TakenFoliage> {
        if !(want > 0.0) || !want.is_finite() {
            return None;
        }
        let i = find_stand(&self.config, &self.stands, site, place).ok()?;
        let e_v = self.config.species(self.stands[i].species).energy_density;
        // The induced defence: the bite takes `(1 − defence)` of what was asked, and a
        // bite that took anything raises the defence (below, once it is known).
        let rise = self.config.species(self.stands[i].species).defence_per_bite;
        let want = want * (1.0 - self.stands[i].defence.clamp(0.0, 1.0));
        if !(want > 0.0) {
            return None;
        }
        // Which layers the mouth may take from, lowest first, and what each offers: its
        // edible stock, above the grazing floor, and never the floor itself.
        let offered: Vec<(usize, f64)> =
            layers_of(&self.config, &self.stands[i], self.config.voxel_m)
                .into_iter()
                .filter_map(|l| {
                    let fi = l.foliage_index?;
                    let inside = layers.is_none_or(|range| range.contains(&l.cell));
                    let edible = l.edible();
                    (inside && edible > 0.0).then_some((fi, edible))
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
        self.ledger.foliage_taken[self.stands[i].species.index()] += organic;
        if rise > 0.0 {
            let d = &mut self.stands[i].defence;
            *d += rise.min(1.0) * (1.0 - *d);
        }
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
        self.ledger.litter_taken += taken.organic;
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
        g.clear_spent_carcass(self.config.carcass_gone_fraction);
        Some(self.book_consumed(taken))
    }

    /// Eat up to `want` of the fruit hanging on the stand on `site` (F3, the seedporter's
    /// lanternberry fruit): `None` when there is no stand or it holds no fruit. Booked as
    /// `consumed_*_out`, with the mineral and energy the fruit carries: the species'
    /// `fruit_mineral_fraction` of the organic matter out of the stand's mineral (never
    /// more than it holds), and its `energy_density`.
    pub fn take_stand_fruit(&mut self, site: Site, want: f64) -> Option<Taken> {
        if !(want > 0.0) || !want.is_finite() {
            return None;
        }
        let i = find_stand(&self.config, &self.stands, site, FaceLayer::Plant).ok()?;
        let sc = self.config.species(self.stands[i].species);
        let (fraction, e_v) = (sc.fruit_mineral_fraction.max(0.0), sc.energy_density);
        let stand = &mut self.stands[i];
        if !(stand.fruit > 0.0) {
            return None;
        }
        let took = if want >= stand.fruit {
            stand.fruit_age_s = 0.0;
            std::mem::replace(&mut stand.fruit, 0.0)
        } else {
            stand.fruit -= want;
            want
        };
        let mineral = (fraction * took).min(stand.mineral.max(0.0));
        stand.mineral -= mineral;
        Some(self.book_consumed(Taken {
            organic: took,
            mineral,
            energy: e_v * took,
        }))
    }

    /// **Seeds voided by an animal** (F3, the seedporter's gut): `organic` and `mineral` of
    /// `species`' seed land in the bank of `site` (the arrival bin of this tick), booked as
    /// `deposited_*_in` with the energy the seeds hold, `energy_density · organic`; any
    /// difference between that and the `energy` the depositor hands over leaves as heat,
    /// so a depositor that carried the seeds at another density still closes its books.
    /// Refused (`false`, booking nothing) when a number is not finite or negative, or the
    /// organic matter is zero. A site with no ground entry is provisioned as
    /// [`Flora::deposit`] provisions one.
    pub fn deposit_seeds(&mut self, site: Site, species: Species, organic: f64, mineral: f64, energy: f64) -> bool {
        for v in [organic, mineral, energy] {
            if !v.is_finite() || v < 0.0 {
                return false;
            }
        }
        if !(organic > 0.0) {
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
        let sc = self.config.species(species);
        let held = sc.energy_density * organic;
        self.ledger.deposited_organic_in += organic;
        self.ledger.deposited_mineral_in += mineral;
        self.ledger.deposited_energy_in += energy;
        self.ledger.heat_out += energy - held;
        let fresh = self.ground[gi].seeds.is_empty();
        seeds::add_cohort(&mut self.ground[gi], species, organic, mineral, self.tick, sc);
        if fresh {
            seeds::schedule(self, site);
        }
        true
    }

    /// **Latticevine seeds voided by an animal** (roster mechanics fix 3): `organic` and
    /// `mineral` of vine seed land in the vine seed bank of `site`, booked as
    /// `deposited_*_in` with the energy the seed holds, the vine's `energy_density ·
    /// organic`; any difference from the `energy` handed over leaves as heat, as
    /// [`Flora::deposit_seeds`] books it. The site joins the bank's check wheel: at its
    /// check a whole seed ([`vine_seed_package`]) on a site a vine can root on founds one.
    /// Refused (`false`, booking nothing) when a number is not finite or negative, or the
    /// organic matter is zero.
    pub fn deposit_vine_seeds(&mut self, site: Site, organic: f64, mineral: f64, energy: f64) -> bool {
        for v in [organic, mineral, energy] {
            if !v.is_finite() || v < 0.0 {
                return false;
            }
        }
        if !(organic > 0.0) {
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
        let held = self.config.latticevine.energy_density * organic;
        self.ledger.deposited_organic_in += organic;
        self.ledger.deposited_mineral_in += mineral;
        self.ledger.deposited_energy_in += energy;
        self.ledger.heat_out += energy - held;
        let tick = self.tick;
        let lot = self.ground[gi].vine_seeds.get_or_insert(VineSeedLot {
            organic: 0.0,
            mineral: 0.0,
            landed_tick: tick,
        });
        lot.organic += organic;
        lot.mineral += mineral;
        seeds::schedule(self, site);
        true
    }

    /// Drink up to `want` of the nectar of the stand on `site` (F1, §3.7): `None` when
    /// there is no stand or it holds no nectar. Booked as `consumed_*_out`, with the
    /// mineral and energy the nectar carries: the species' `nectar_mineral_fraction` of
    /// the organic matter out of the stand's mineral (never more than it holds), and its
    /// `energy_density`.
    pub fn take_stand_nectar(&mut self, site: Site, want: f64) -> Option<Taken> {
        if !(want > 0.0) || !want.is_finite() {
            return None;
        }
        let i = find_stand(&self.config, &self.stands, site, FaceLayer::Plant).ok()?;
        let sc = self.config.species(self.stands[i].species);
        let (fraction, e_v) = (sc.nectar_mineral_fraction.max(0.0), sc.energy_density);
        let stand = &mut self.stands[i];
        if !(stand.nectar > 0.0) {
            return None;
        }
        let took = if want >= stand.nectar {
            std::mem::replace(&mut stand.nectar, 0.0)
        } else {
            stand.nectar -= want;
            want
        };
        let mineral = (fraction * took).min(stand.mineral.max(0.0));
        stand.mineral -= mineral;
        Some(self.book_consumed(Taken {
            organic: took,
            mineral,
            energy: e_v * took,
        }))
    }

    /// **Glassfilm founding** (R2a): put `film_seed_per_m2` of film on every wet-eligible
    /// support face of `world` (standing water from `film_wet_min_depth_m` to
    /// `film_max_depth_m`: a residue is not seeded),
    /// at `film_n_tissue` mineral and `film_energy_density` energy per unit, booked as
    /// introduced the way `Seed` is. A site with no [`Ground`] is provisioned with its
    /// `initial_mineral` as a deposit's is, under either [`Provision`]. Returns how many
    /// sites were seeded.
    pub fn seed_film(&mut self, world: &World) -> usize {
        let view = world.view();
        let c = view.config;
        let organic = self.config.film_seed_per_m2 * c.voxel_m * c.voxel_m;
        if !(organic > 0.0) {
            return 0;
        }
        let mineral = self.config.film_n_tissue * organic;
        let energy = self.config.film_energy_density * organic;
        let mut n = 0;
        for z in 0..c.depth {
            for x in 0..i64::from(c.width) {
                for y in 0..c.height {
                    if !view.is_support(x, y, z)
                        || !film::eligible(&view, &self.config, x, y, z)
                    {
                        continue;
                    }
                    let site = Site { x: x as u32, y, z };
                    let i = step::provisioned_slot(
                        &self.config,
                        &mut self.ground,
                        site,
                        &mut self.ledger,
                    );
                    let g = &mut self.ground[i];
                    g.film += organic;
                    g.film_mineral += mineral;
                    g.film_energy += energy;
                    self.ledger.seeded_organic_in += organic;
                    self.ledger.seeded_mineral_in += mineral;
                    self.ledger.seeded_energy_in += energy;
                    self.ledger.film_seeded += organic;
                    n += 1;
                }
            }
        }
        n
    }

    /// A grazer eats **film** off the site, up to `want`, pro rata —
    /// [`Flora::take_litter`]'s rule on the film pool, booked as consumed.
    pub fn take_film(&mut self, site: Site, want: f64) -> Option<Taken> {
        let i = self.ground.binary_search_by_key(&site, |g| g.site).ok()?;
        // The refuge ([`FloraConfig::film_graze_refuge`]) is out of a mouth's reach.
        let refuge = self.config.film_graze_refuge.clamp(0.0, 1.0)
            * self.config.film_capacity_per_m2
            * self.config.voxel_m
            * self.config.voxel_m;
        let g = &mut self.ground[i];
        // A bad `want` stays bad (refused below); a good one stops at the refuge.
        let want = if want.is_finite() { want.min(g.film - refuge) } else { want };
        let taken = take_pool(&mut g.film, &mut g.film_mineral, &mut g.film_energy, want)?;
        self.ledger.film_consumed += taken.organic;
        Some(self.book_consumed(taken))
    }

    /// Lay a **carcass record** on `site`'s ground pool (§2.5 of the predation note): the
    /// dead body the carrion there came from, for the presenter. Massless — nothing is
    /// booked — and it replaces any record already there. `false` when the site has no
    /// ground entry (the body's deposit makes one) or the record is not finite.
    pub fn lay_carcass(&mut self, site: Site, carcass: Carcass) -> bool {
        if !(carcass.heading_rad.is_finite() && carcass.organic.is_finite()) {
            return false;
        }
        match self.ground.binary_search_by_key(&site, |g| g.site) {
            Ok(i) => {
                self.ground[i].carcass = Some(carcass);
                true
            }
            Err(_) => false,
        }
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
    /// organic matter in it would hold that mineral for ever. It is still
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
    /// it, because it is not in this one's snapshot.
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
        // **A deposit with no organic matter in it is terminal at once**. A
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
fn default_film_graze_refuge() -> f64 {
    FILM_GRAZE_REFUGE
}

/// The shipped [`FloraConfig::film_graze_refuge`]: a tenth of a site's capacity, plausible
/// and untuned.
pub const FILM_GRAZE_REFUGE: f64 = 0.1;

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
