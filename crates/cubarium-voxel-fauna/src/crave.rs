//! **SP-CRAVE: a body craves what it is short of** (Wrysk, 2026-10-14, asked whether a
//! seedporter short of nitrogen should check trees even with plain fruit in sight: "yes on
//! seedporter question. i mean animals normally with have different cravings based on
//! needs.").
//!
//! - **The cravings** ([`cravings`]): one function per body reads its need for nitrogen and
//!   its need for energy off state it already has, with the glider's `crave_*` knobs
//!   ([`crate::Glider`]):
//!   - **nitrogen** (`crave_nitrogen`) while it builds tissue past its own upkeep — a growing
//!     young (under its `birth_body`), a gestating mother (an escrow), a nursing mother (a
//!     dependent young of hers alive), a brood under its target — or while its fuel is full
//!     (`crave_fuel_full`) and its N-funded reserve lags (`crave_reserve_lag`): fuel is the
//!     N-free share, so a full tank over a lagging reserve says N is what binds;
//!   - **energy**, rising from 0 at `crave_energy_low` of its combined fuel and reserve room
//!     to 1 empty, and 1 with its emergency latch on.
//!
//!   The larger wins ([`Need`]); none, and the choice is today's (the nearest food).
//! - **What a food is worth to it** ([`Fare`], [`worth`]): the usable gain for the craved need
//!   per organic eaten, from the figures `assimilate` uses (`step.rs`): tissue it can fund,
//!   `min(yield, mineral fraction / n_tissue)`, for nitrogen; all it absorbs, `yield`, for
//!   energy; both less the share that is seeds riding in the gut. The worth of a source is
//!   that gain over a meal there (its stock, at most the stomach's room) per second of walking
//!   to it at its own cruise plus eating it at the gut's bite times the food's handling.
//!
//! A heuristic's sense of its own body, not a reward: nothing scores or pays the behaviour,
//! and no observation channel changes.

use crate::{Animal, Fauna, Food, FounderPhysiology};
use cubarium_voxel_flora::{FloraView, Species, Stand};

/// A body's cravings, each in `[0, 1]`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cravings {
    pub nitrogen: f64,
    pub energy: f64,
}

/// The need its cravings make: the larger craving (energy on a tie), or none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
    None,
    Nitrogen,
    Energy,
}

impl Cravings {
    pub fn need(&self) -> Need {
        if self.nitrogen > self.energy {
            Need::Nitrogen
        } else if self.energy > 0.0 {
            Need::Energy
        } else {
            Need::None
        }
    }
}

/// `x / cap` in `[0, 1]`; a cap of zero reads as full.
fn fullness(x: f64, cap: f64) -> f64 {
    if cap > 0.0 { (x / cap).clamp(0.0, 1.0) } else { 1.0 }
}

/// Whether body `i` is a mother with a dependent young of hers alive ([`crate::nurse::dependent`]).
fn nursing(fauna: &Fauna, i: usize) -> bool {
    let id = fauna.animals[i].id;
    (0..fauna.animals.len()).any(|k| crate::mother_body(&fauna.animals[k]) == Some(id) && crate::nurse::dependent(fauna, k))
}

/// **The cravings of body `i`** (module note). Zero for a body without a glider table.
pub fn cravings(fauna: &Fauna, i: usize) -> Cravings {
    let a = &fauna.animals[i];
    let Some(founder) = a.founder else { return Cravings::default() };
    let phys = fauna.config.founder(founder);
    let Some(g) = phys.glider else { return Cravings::default() };
    let sc = phys.core;
    let (reserve_cap, fuel_cap) = (sc.reserve_of(a.body), sc.fuel_cap * a.body);
    let energy = if !(g.crave_energy_low > 0.0) {
        0.0
    } else if a.emergency {
        1.0
    } else {
        let stores = fullness(a.reserve + a.fuel, reserve_cap + fuel_cap);
        ((g.crave_energy_low - stores) / g.crave_energy_low).clamp(0.0, 1.0)
    };
    let rule = sc.reproduction;
    let builds = a.body < sc.birth_body
        || a.reproduction.escrow.is_some()
        || (a.body >= sc.birth_body && a.reproduction.brood < rule.brood_target(&sc, a.body))
        || nursing(fauna, i);
    let binds = fullness(a.fuel, fuel_cap) >= g.crave_fuel_full && fullness(a.reserve, reserve_cap) < g.crave_reserve_lag;
    let nitrogen = if builds || binds { g.crave_nitrogen.clamp(0.0, 1.0) } else { 0.0 };
    Cravings { nitrogen, energy }
}

/// **The usable gain per organic eaten** of a food class `food` carrying `mineral` per
/// organic, for an eater of lineage `phys`, with `seeds` of it riding in the gut:
/// `(tissue it funds, all it absorbs)`. `assimilate`'s figures (`step.rs`): the yield,
/// as far as the mineral pays for tissue at `n_tissue`; the rest of the yield is N-free fuel.
pub fn per_organic(phys: &FounderPhysiology, food: Food, mineral: f64, seeds: f64) -> (f64, f64) {
    let y = phys.yield_for(food).max(0.0);
    let n = phys.core.n_tissue;
    let funded = if n > 0.0 { mineral.max(0.0) / n } else { f64::INFINITY };
    let keep = (1.0 - seeds).clamp(0.0, 1.0);
    (y.min(funded) * keep, y * keep)
}

/// The share of a fruit meal that rides in `phys`'s gut as seeds, when the fruit's seeds do.
fn seed_share(phys: &FounderPhysiology, ride: bool) -> f64 {
    match phys.gut_seeds {
        Some(g) if ride => g.seeds_per_fruit_organic.clamp(0.0, 1.0),
        _ => 0.0,
    }
}

/// **What a source offers** a body (module note), summed over the foods it holds there:
/// the stock, and per food its stock × its tissue and absorbed gain per organic, and its
/// stock over its handling (the eating time at a unit bite rate).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fare {
    pub stock: f64,
    pub tissue: f64,
    pub absorbed: f64,
    pub handled: f64,
}

impl Fare {
    fn add(&mut self, stock: f64, (tissue, absorbed): (f64, f64), handling: f64) {
        if !(stock > 0.0) {
            return;
        }
        self.stock += stock;
        self.tissue += stock * tissue;
        self.absorbed += stock * absorbed;
        self.handled += if handling > 0.0 { stock / handling } else { f64::INFINITY };
    }

    /// A stand's hanging fruit, `stock` of it, for `phys`.
    pub fn stand_fruit(&mut self, fv: &FloraView<'_>, phys: &FounderPhysiology, stand: &Stand, stock: f64) {
        let sc = fv.config.species(stand.species);
        let gain = per_organic(phys, Food::Fruit, sc.fruit_mineral_fraction, seed_share(phys, sc.fruit_seeds_in_gut));
        self.add(stock, gain, sc.fruit_handling.clamp(0.0, 1.0));
    }

    /// A latticevine's bead fruit, `stock` of it, for `phys`.
    pub fn vine_fruit(&mut self, fv: &FloraView<'_>, phys: &FounderPhysiology, stock: f64) {
        let gain = per_organic(phys, Food::Fruit, fv.config.latticevine.mineral_fraction, seed_share(phys, true));
        self.add(stock, gain, 1.0);
    }

    /// Larvae of `lineage`, `tissue` of them, for `phys` (their mineral is the lineage's
    /// `n_tissue` of their tissue, `larva.rs`).
    pub fn larvae(&mut self, fauna: &Fauna, phys: &FounderPhysiology, lineage: crate::Founder, tissue: f64) {
        let gain = per_organic(phys, Food::Larva, fauna.config.founder(lineage).core.n_tissue, 0.0);
        self.add(tissue, gain, 1.0);
    }

    /// Tissue it funds per organic eaten here, stock-weighted.
    pub fn tissue_per_organic(&self) -> f64 {
        if self.stock > 0.0 { self.tissue / self.stock } else { 0.0 }
    }
}

/// **The worth to body `a` of lineage `phys`** of a source offering `fare` at `d_m` metres
/// for `need` (module note): the gain over a meal there per second of walking and eating;
/// 0 with no need (the caller keeps today's order then) or nothing on offer.
pub fn worth(fauna: &Fauna, a: &Animal, phys: &FounderPhysiology, need: Need, fare: &Fare, d_m: f64) -> f64 {
    let per = match need {
        Need::None => return 0.0,
        Need::Nitrogen => fare.tissue,
        Need::Energy => fare.absorbed,
    };
    if !(fare.stock > 0.0) {
        return 0.0;
    }
    let length = crate::body_of(&fauna.config, a).map_or(phys.adult_length_m, |b| b.length_m);
    let cruise = phys.cruise_body_lengths_per_s * length;
    let walk_s = if cruise > 0.0 { d_m.max(0.0) / cruise } else { f64::INFINITY };
    let rate = phys.gut.bite_per_body_s * phys.gut.throughput_factor(a.body, phys.core.body_max) * a.body;
    let room = phys.gut.capacity(a.body) - a.stomach.organic();
    // The meal there: its stock, at most the stomach's room.
    let share = if room > 0.0 { (room / fare.stock).min(1.0) } else { 1.0 };
    let eat_s = if rate > 0.0 { share * fare.handled / rate } else { f64::INFINITY };
    let t = walk_s + eat_s;
    if t > 0.0 && t.is_finite() { share * per / t } else { 0.0 }
}

/// **The most tissue per organic a check could find** for an eater of lineage `phys`: a
/// species' nuts (fruit seen only from close by, [`crate::treecheck::may_hold_hidden_food`])
/// and, for an eater that seeks larvae, a laying lineage's larvae. A food in sight funding
/// less than this is plain to a body craving nitrogen.
pub fn hidden_tissue_per_organic(fauna: &Fauna, fv: &FloraView<'_>, phys: &FounderPhysiology) -> f64 {
    let nuts = Species::ALL
        .iter()
        .map(|&s| fv.config.species(s))
        .filter(|sc| sc.fruit_per_s > 0.0 && sc.fruit_sight_m > 0.0)
        .map(|sc| per_organic(phys, Food::Fruit, sc.fruit_mineral_fraction, seed_share(phys, sc.fruit_seeds_in_gut)).0)
        .fold(0.0, f64::max);
    let larvae = if crate::treecheck::seeks_larvae(fauna, phys) {
        crate::Founder::ALL
            .iter()
            .filter(|&&f| fauna.config.founder(f).larva.is_some())
            .map(|&f| per_organic(phys, Food::Larva, fauna.config.founder(f).core.n_tissue, 0.0).0)
            .fold(0.0, f64::max)
    } else {
        0.0
    };
    nuts.max(larvae)
}
