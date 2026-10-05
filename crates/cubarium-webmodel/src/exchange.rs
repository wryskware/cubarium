//! The interface between the producer half ([`crate::flora`]) and the animal half
//! ([`crate::fauna`]). World totals in the sim's own tissue units, per the model's step.
//! Changes to these types go through the parent session, since both halves build on them.

use cubarium_voxel_flora::Species;

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
    /// Fruit: latticevine fruit and lanternberry stock.
    pub fruit: f64,
}

/// What the animals took from a [`Supply`] over one step; same fields, same units.
pub type Take = Supply;

/// What the animals give back to the pools over one step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Returns {
    /// To the litter pool (dung, shed tissue, if the sim routes any there).
    pub litter: f64,
    /// To the carrion pool: bodies that died and were not eaten whole.
    pub carrion: f64,
}
