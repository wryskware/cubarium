//! **Plant anatomy: a stand is a profile of layers, not a lollipop.**
//!
//! Before this module every plant was one crown disc, one cell thick, holding all of
//! its foliage: there was no low foliage and no high foliage, so a low browser either
//! ate the whole plant or none of it, and shade under a tiered frond was
//! all-or-nothing (`design/organism-anatomy-2026-09-21.md` §1).
//!
//! A species now carries a **profile** staged by `wood / wood_max`: an ordered list of
//! [`Layer`]s, each a band of the crown's height with a radius, a kind and — for the
//! foliage-bearing kinds — a share of the stand's foliage **capacity**. A stand holds
//! one stock per foliage-bearing layer of its current profile, and those stocks sum to
//! the scalar `foliage` every ledger, water and mineral rule already reads
//! (`design/handoffs/voxel-organism-decisions-2026-09-21.md` §4).
//!
//! **Bands are the physical truth; cells are a conversion at the edge.** A band is a
//! pair of fractions of the stand's crown height, measured from the support face, so
//! `[0, 1.0]` is the whole plant and `[0, 0.15]` is the first slice above the ground.
//! Everything continuous — shade, the mouth's metre band — reads the band. Everything
//! discrete — the cone, the presenter, the mouth's own cell scan — reads the layer's
//! one **disc cell**, `round(band_top × crown_height)`, never zero, which for a
//! single-layer `[0, 1.0]` profile is `crown_voxels(wood)` itself: exactly the cell the
//! lollipop model drew its disc in. That is what keeps a single-layer species'
//! geometry unchanged by this module.

use serde::{Deserialize, Serialize};

/// The most foliage-bearing layers any one profile stage may declare.
///
/// A fixed ceiling rather than a `Vec` on the stand, because [`crate::Stand`] is `Copy`
/// and is copied by value all over the crate and the host. Three is the most any of the
/// six live species uses (umbrellafrond's adult tiers); four leaves room for the
/// vaulttree's drape when that species arrives.
pub const MAX_FOLIAGE_LAYERS: usize = 4;

/// What a layer is made of. Only [`LayerKind::Trunk`] bears no foliage; the other three
/// are the anatomy document's names for foliage that hangs, mats or stands
/// (`design/organism-anatomy-2026-09-21.md` §2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LayerKind {
    /// Structure: wood, stems, limbs. An occluder to an eye, never food, no share.
    Trunk,
    /// Leaves standing in a band: a crown, a tier, a rosette.
    Foliage,
    /// Foliage hanging below its attachment: filaments, curtains.
    Drape,
    /// Foliage lying on the surface: turf, a pad, a cushion's skirt.
    Mat,
}

impl LayerKind {
    /// Whether this kind holds a share of the stand's foliage. Everything but
    /// [`LayerKind::Trunk`] does.
    pub const fn bears_foliage(self) -> bool {
        !matches!(self, LayerKind::Trunk)
    }
}

/// One band of a plant's volume.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub kind: LayerKind,
    /// `[from, to]` as fractions of the stand's physical crown height, `0` at the
    /// support face.
    pub band: [f64; 2],
    /// Fraction of the stand's physical crown radius.
    pub radius: f64,
    /// Fraction of the stand's foliage **capacity** this layer holds. Zero on a
    /// [`LayerKind::Trunk`]; the foliage-bearing layers of a stage sum to one.
    pub share: f64,
    /// `0` is a solid disc, `1` casts no shade. **A light property only**: the cone
    /// sees any foliage-bearing layer cell holding stock as foliage, however porous
    /// (`design/voxel-encounter-contract-2026-09-21.md` §8).
    pub porosity: f64,
}

/// One stage of a species' anatomy: the layers that apply while
/// `wood / wood_max <= wood_fraction_max`, and an optional physical height ceiling for
/// the stage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// The first stage whose threshold is at or above `wood / wood_max` applies. The
    /// last stage of a species must be at or above `1.0`.
    pub wood_fraction_max: f64,
    /// A ceiling on the stand's physical crown height **in metres** while this stage
    /// applies, overriding the interpolated height. Decisions §5: a woody seedling is a
    /// ground rosette no taller than 0.125 m, whatever the wood interpolation says.
    ///
    /// On a species' **first** stage it makes the stage a capped seedling (package L,
    /// [`crate::SpeciesConfig::capped_seedling`]): the ceiling bounds the crown's radius
    /// too, and growth after the seedling starts from it rather than from the range's
    /// minimum. On a later stage it is read by nothing.
    pub height_m_max: Option<f64>,
    /// Bottom-up. Regrowth fills the foliage-bearing ones in this order.
    pub layers: Vec<Layer>,
}

impl Profile {
    /// The foliage-bearing layers, in profile (bottom-up) order.
    pub fn foliage_layers(&self) -> impl Iterator<Item = (usize, &Layer)> {
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, l)| l.kind.bears_foliage())
    }

    /// How many stocks a stand in this stage holds.
    pub fn foliage_layer_count(&self) -> usize {
        self.foliage_layers().count()
    }

    /// `share` of each foliage-bearing layer, bottom-up.
    pub fn shares(&self) -> Vec<f64> {
        self.foliage_layers().map(|(_, l)| l.share).collect()
    }
}

/// A layer of a **particular stand**, with its geometry resolved and its stock read.
///
/// Heights are in voxels **above the world floor**, as `step`'s crown model measures
/// them (`site.y + fraction × crown_height`), and in metres where a metre is what the
/// consumer speaks. The `cell` is the one layer of cells the disc occupies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StandLayer {
    /// Index in the stage's `layers`.
    pub index: usize,
    /// Index among the stage's foliage-bearing layers, which is the index into the
    /// stand's `layer_stock`. `None` for a [`LayerKind::Trunk`].
    pub foliage_index: Option<usize>,
    pub kind: LayerKind,
    /// `[bottom, top]` in **voxels above the world floor**.
    pub band_v: [f64; 2],
    /// `[bottom, top]` in **metres above the world floor**.
    pub band_m: [f64; 2],
    /// The layer's radius in voxels, and in metres.
    pub radius_v: f64,
    pub radius_m: f64,
    /// The single cell layer this layer's disc occupies, absolute: `site.y + offset`.
    pub cell: i64,
    /// For a [`LayerKind::Trunk`], the inclusive run of cells it fills; for a foliage
    /// layer, `(cell, cell)`.
    pub cells: (i64, i64),
    /// This layer's share of the stand's foliage capacity.
    pub share: f64,
    /// `share × alpha × wood`: what this layer holds when the stand is full.
    pub capacity: f64,
    /// What it holds now. Zero for a [`LayerKind::Trunk`].
    pub stock: f64,
    /// The **grazing floor**: `graze_refuge × capacity`, what no bite may take from this
    /// layer ([`crate::SpeciesConfig::graze_refuge`]). Zero for a [`LayerKind::Trunk`].
    pub floor: f64,
    pub porosity: f64,
    /// The layer's horizontal area in m², floored at one reference cell — the divisor
    /// of the shade exponent.
    pub area_m2: f64,
}

impl StandLayer {
    /// **Edible foliage**: what this layer holds above its grazing floor, never negative.
    ///
    /// The one reading of "food" in a plant. Every consumer that asks how much a mouth
    /// could take — a bite, the mouth's taste, the cone's foliage-or-stripped class, a
    /// heuristic's reach and smell — reads this and not [`StandLayer::stock`], and
    /// [`crate::Flora::take_foliage_in_layers`] withdraws no more than it reports, so a
    /// plant at its floor is bare to every eye and gives every mouth nothing.
    pub fn edible(&self) -> f64 {
        let above = self.stock - self.floor;
        // A layer taken to its floor keeps the float residue `settle_layers` hands back
        // to the lowest layer a bite touched; that dust is not food.
        if above > EDIBLE_DUST { above } else { 0.0 }
    }
}

/// Organic matter above a grazing floor that still reads as none: the float residue of
/// a withdrawal, some ten orders under the smallest bite a mouth asks for.
pub const EDIBLE_DUST: f64 = 1e-12;

/// The floor on a layer's area in the shade exponent: one 0.25 m reference cell, which
/// is what the pre-layer crown model floored at.
pub const MIN_LAYER_AREA_M2: f64 = 0.25 * 0.25;

/// The cell offset above the support face a band's top occupies: `round(top × height)`,
/// never below one. For a `[0, 1.0]` band this is `crown_voxels(wood)` exactly.
pub fn disc_offset(band_top: f64, crown_height_v: f64) -> u32 {
    let h = band_top * crown_height_v;
    if !h.is_finite() {
        return 1;
    }
    (h.round().max(1.0) as u32).min(u32::from(u16::MAX))
}

/// The inclusive run of cell offsets a **trunk** band fills: from the cell its bottom
/// falls in to the cell its top rounds to.
pub fn trunk_offsets(band: [f64; 2], crown_height_v: f64) -> (u32, u32) {
    let lo = (band[0] * crown_height_v).floor().max(0.0) as u32 + 1;
    let hi = disc_offset(band[1], crown_height_v);
    (lo, hi.max(lo))
}

/// Fill `stocks` bottom-up to `caps`, spending `amount` and returning what was placed.
///
/// Each layer takes what it has room for before the next one gets anything, which is
/// what makes a browsed plant refill its floor tissue first and stay leaf-poor at the
/// base while it is being grazed (`design/organism-anatomy-2026-09-21.md` §3,
/// bloomcrown). Any residue left when every layer is at capacity — float dust, or a
/// stand whose wood diebacked below what its foliage already holds — goes into the top
/// layer rather than vanishing, because the stocks must go on summing to `foliage`.
pub fn fill_bottom_up(stocks: &mut [f64], caps: &[f64], amount: f64) -> f64 {
    if !(amount > 0.0) || stocks.is_empty() {
        return 0.0;
    }
    let mut left = amount;
    for (i, stock) in stocks.iter_mut().enumerate() {
        if !(left > 0.0) {
            break;
        }
        let room = (caps.get(i).copied().unwrap_or(0.0) - *stock).max(0.0);
        let give = left.min(room);
        *stock += give;
        left -= give;
    }
    if left > 0.0
        && let Some(top) = stocks.last_mut()
    {
        *top += left;
    }
    amount
}

/// Take `amount` from `stocks`, **top layer first**, returning what was taken.
/// Senescence, dieback and crown loss shed from the top: a plant loses its canopy
/// before its rosette (`design/organism-anatomy-2026-09-21.md` §3).
pub fn shed_from_top(stocks: &mut [f64], amount: f64) -> f64 {
    if !(amount > 0.0) {
        return 0.0;
    }
    let mut left = amount;
    for stock in stocks.iter_mut().rev() {
        if !(left > 0.0) {
            break;
        }
        let take = left.min(*stock);
        *stock -= take;
        left -= take;
    }
    amount - left
}

/// Re-bin `total` into `caps` bottom-up, discarding whatever binning the stocks had.
///
/// This is a **stage transition**: the layer set changed under a stand that already
/// holds tissue, and the tissue has to go somewhere. It is placed the way regrowth
/// places it — the floor first, the crown last — so a plant that has just grown a stem
/// does not conjure a full crown out of a rosette, and it never creates tissue: what
/// goes in is exactly what came out (`decisions §4`).
pub fn rebin(stocks: &mut [f64], caps: &[f64], total: f64) {
    for stock in stocks.iter_mut() {
        *stock = 0.0;
    }
    fill_bottom_up(stocks, caps, total.max(0.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_band_top_of_one_is_the_old_crown_cell() {
        for height in [0.5, 1.0, 1.4, 1.5, 2.0, 3.0, 5.0, 16.0] {
            assert_eq!(
                disc_offset(1.0, height),
                (height.round().max(1.0) as u32),
                "height {height}"
            );
        }
    }

    #[test]
    fn filling_bottom_up_tops_each_layer_before_the_next() {
        let caps = [0.25, 0.75];
        let mut stocks = [0.0, 0.0];
        fill_bottom_up(&mut stocks, &caps, 0.30);
        assert!((stocks[0] - 0.25).abs() < 1e-15 && (stocks[1] - 0.05).abs() < 1e-15);
    }

    #[test]
    fn a_residue_lands_on_top_rather_than_disappearing() {
        let caps = [0.1, 0.1];
        let mut stocks = [0.0, 0.0];
        fill_bottom_up(&mut stocks, &caps, 1.0);
        let sum: f64 = stocks.iter().sum();
        assert!((sum - 1.0).abs() < 1e-15, "{stocks:?}");
    }

    #[test]
    fn shedding_takes_the_crown_before_the_rosette() {
        let mut stocks = [0.25, 0.75];
        assert!((shed_from_top(&mut stocks, 0.8) - 0.8).abs() < 1e-15);
        assert!(
            (stocks[0] - 0.2).abs() < 1e-15 && stocks[1] == 0.0,
            "{stocks:?}"
        );
    }

    #[test]
    fn a_rebin_conserves_the_total() {
        let mut stocks = [0.0, 1.0, 0.0];
        rebin(&mut stocks, &[0.4, 0.35, 0.25], 1.0);
        let sum: f64 = stocks.iter().sum();
        assert!((sum - 1.0).abs() < 1e-15, "{stocks:?}");
        assert!((stocks[0] - 0.4).abs() < 1e-15, "bottom-up: {stocks:?}");
    }
}
