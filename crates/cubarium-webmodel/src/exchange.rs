//! The interface between the producer half ([`crate::flora`]) and the animal half
//! ([`crate::fauna`]). World totals in the sim's own tissue units, per the model's step.
//! Changes to these types go through the parent session, since both halves build on them.
//!
//! **The parent's closed-loop decisions** (2026-10-05, `design/webmodel/loop.md`):
//! 1. [`Supply`] and [`Take`] stay **world totals** per food class, with foliage per species,
//!    plus the **area each class is spread over** ([`Areas`], m²). The animal half reads a
//!    density as stock / area against the area its responses were fitted on (patch).
//! 2. The plant half spreads a [`Take`] over its zones **pro rata to each zone's edible stock**
//!    of that class (mass-action foraging).
//! 3. [`Returns`] carries the **mineral** in dung and carcasses.
//! 4. [`Take`] carries **seedling browse** per species, zero unless a scenario sets a browser.

use cubarium_voxel_flora::Species;

/// The area (m²) each food class of a [`Supply`] is spread over: the faces its stock stands
/// on. Zero means "not given" (an open-loop clamp), and the animal half then reads its
/// single floor scale.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Areas {
    /// Per plant species ([`Species::ALL`] order): the faces its stands hold. The glowcap's
    /// entry is the faces its caps stand on.
    pub foliage: [f64; Species::COUNT],
    pub litter: f64,
    pub carrion: f64,
    /// The film's eligible faces (shallow standing water).
    pub film: f64,
    pub nectar: f64,
    pub fruit: f64,
}

/// What the producers and the detritus pools hold for the animals: standing edible stock.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Supply {
    /// Edible foliage per plant species, in [`Species::ALL`] order. The glowcap's entry is
    /// its cap tissue (the capgnawer's food), not a browser's.
    pub foliage: [f64; Species::COUNT],
    /// The ground litter pool.
    pub litter: f64,
    /// The ground carrion pool.
    pub carrion: f64,
    /// The glassfilm pool (the ripple-snail's food).
    pub film: f64,
    /// Nectar: latticevine flowers and bloomcrown stock.
    pub nectar: f64,
    /// Mineral per nectar organic unit, weighted over the supplied products. An open-loop
    /// supply may omit this and use the configured producing sources' conservative density.
    pub nectar_mineral_fraction: Option<f64>,
    /// Fruit: latticevine fruit and lanternberry stock.
    pub fruit: f64,
    /// Mineral per fruit organic unit, weighted over the supplied products.
    pub fruit_mineral_fraction: Option<f64>,
    /// Seedlings a browser could take: stands under `donor_min` per species (count).
    pub seedlings: [f64; Species::COUNT],
    /// The area each class is spread over.
    pub area: Areas,
}

/// What the animals took from a [`Supply`] over one step, the same classes and units.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Take {
    pub foliage: [f64; Species::COUNT],
    pub litter: f64,
    pub carrion: f64,
    pub film: f64,
    pub nectar: f64,
    pub fruit: f64,
    /// Seedlings browsed per species, stands (whole juveniles, `W < donor_min`). Zero unless
    /// a scenario sets a seedling browser: no lineage of the animal half eats them.
    pub seedlings: [f64; Species::COUNT],
}

impl Take {
    /// The organic matter of the food classes (seedlings are counted in stands, not here).
    pub fn organic(&self) -> f64 {
        self.foliage.iter().sum::<f64>() + self.litter + self.carrion + self.film + self.nectar + self.fruit
    }
}

/// What the animals give back to the pools over one step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Returns {
    /// To the litter pool (dung, bite spill, dead bank eggs).
    pub litter: f64,
    /// To the carrion pool: bodies that died and were not eaten whole, lost eggs, the kill's
    /// share beyond the hunter's reach, carcasses left.
    pub carrion: f64,
    /// **Mineral** in the dung and the carcasses, back to the site pools where the animals fed.
    /// The plant half keeps dead pools mineral-free (a dead pool's mineral is in the site pool
    /// from the moment it is shed), so this is the mineral the animals took in with living
    /// tissue (foliage, caps, nectar, fruit) beyond what their own tissue holds.
    pub mineral: f64,
}
