//! **GLOW-EASE round 2** (Wrysk, 2026-10-10: "i see patches of latticevine during the day
//! that darken. theres also springturf browning, and potentially other plants have similar
//! effects. these all need to be faded as well"): a plant cell never changes colour, and a
//! latticevine cell never changes tile, in one step.
//!
//! Two kinds of switch, each eased over `[light] colour_fade_s` of the presenter's clock:
//!
//! - **A style swap** ([`StyleFades`]): a plant cell keeps its glyph but its stand's colours
//!   change at a tick — springturf curing to straw and waking green, a model plant's wilt
//!   level, a fruit or bloom accent turning ripe, a glowcap lip's colony colour. The cell
//!   draws a straight line from the colours it showed to the new ones. The packer reads it
//!   once a pack, so on both GPU tiers it moves in rungs of one pack (20 Hz).
//! - **A vine tile swap** ([`VineFades`]): a latticevine cell's tile (its density row, full,
//!   thin or bare, a dry spell's dormancy; its accent, bud, flower or fruit; its shape) changes
//!   at a tick. The renderer draws both tiles, the old one's share of coverage and colour
//!   falling from 1 to 0, every frame on the lit tier.
//!
//! Either ramp started again mid-way starts from what the cell shows, so a reversal runs
//! back without a jump. Cells appearing or going (growth, grazing) are not switches here.

use rustc_hash::FxHashMap;

use super::glow_ease::JUMP_S;
use super::stand::Style;

fn lerp(a: [f32; 3], b: [f32; 3], p: f32) -> [f32; 3] {
    std::array::from_fn(|k| a[k] + (b[k] - a[k]) * p)
}

/// One style blended `p` of the way from `a` to `b` (`b` itself from 1 on).
pub fn blend(a: Style, b: Style, p: f32) -> Style {
    if p >= 1.0 {
        return b;
    }
    Style {
        wood: lerp(a.wood, b.wood, p),
        crown: lerp(a.crown, b.crown, p),
        heart: lerp(a.heart, b.heart, p),
    }
}

/// Progress `0..=1` of a ramp started at `start` at `now`.
fn progress(now: f64, start: f64, fade: f64) -> f32 {
    if fade > 0.0 { ((now - start) / fade).clamp(0.0, 1.0) as f32 } else { 1.0 }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct StyleRamp {
    from: Style,
    to: Style,
    start: f64,
}

impl StyleRamp {
    fn at(&self, now: f64, fade: f64) -> Style {
        blend(self.from, self.to, progress(now, self.start, fade))
    }
}

/// The plant cells part way through a style swap: see the module.
#[derive(Debug, Default)]
pub struct StyleFades {
    fade_s: f32,
    ramps: FxHashMap<u32, StyleRamp>,
    last: Option<f64>,
}

impl StyleFades {
    pub fn new(fade_s: f32) -> StyleFades {
        StyleFades { fade_s: fade_s.max(0.0), ..StyleFades::default() }
    }

    /// Whether swaps fade at all (`colour_fade_s` above 0).
    pub fn on(&self) -> bool {
        self.fade_s > 0.0
    }

    /// A pack at presenter time `now` begins: a clock that went back or jumped further than
    /// [`JUMP_S`] (a load, a seek) ends every ramp, so each cell draws its colours as they
    /// stand.
    pub fn begin(&mut self, now: f64) {
        if self.last.is_some_and(|last| now < last || now - last > JUMP_S) {
            self.ramps.clear();
        }
        self.last = Some(now);
    }

    /// What `cell` draws at `now`, given the style it should draw (`to`) and what it drew
    /// last pack (`held`; `None` when it drew nothing comparable). A cell whose target moved
    /// starts a ramp from what it showed; a ramp that reaches its target ends.
    pub fn draw(&mut self, cell: u32, held: Option<Style>, to: Style, now: f64) -> Style {
        if !self.on() {
            return to;
        }
        let fade = f64::from(self.fade_s);
        if let Some(r) = self.ramps.get_mut(&cell) {
            if r.to != to {
                *r = StyleRamp { from: r.at(now, fade), to, start: now };
            }
            if now - r.start >= fade {
                self.ramps.remove(&cell);
                return to;
            }
            return r.at(now, fade);
        }
        match held {
            Some(h) if h != to => {
                self.ramps.insert(cell, StyleRamp { from: h, to, start: now });
                h
            }
            _ => to,
        }
    }

    /// `cell` no longer draws a plant: its ramp, if any, ends.
    pub fn forget(&mut self, cell: u32) {
        self.ramps.remove(&cell);
    }

    /// Whether `cell` is part way through a swap.
    pub fn fading(&self, cell: u32) -> bool {
        self.ramps.contains_key(&cell)
    }

    /// Every cell part way through a swap, in no order: each is packed again next pack.
    pub fn cells(&self) -> impl Iterator<Item = u32> + '_ {
        self.ramps.keys().copied()
    }

    pub fn len(&self) -> usize {
        self.ramps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ramps.is_empty()
    }
}

/// A vine cell's tile as its texel carries it: `(b, a)`, the column byte (mask and exits)
/// and the row byte (density row and accent), `VineCell::texel_bytes`.
pub type Tile = (u8, u8);

#[derive(Clone, Copy, Debug, PartialEq)]
struct VineRamp {
    from: Tile,
    to: Tile,
    start: f64,
}

/// The latticevine cells part way through a tile swap: see the module.
#[derive(Debug, Default)]
pub struct VineFades {
    fade_s: f32,
    ramps: FxHashMap<u32, VineRamp>,
    last: Option<f64>,
}

impl VineFades {
    pub fn new(fade_s: f32) -> VineFades {
        VineFades { fade_s: fade_s.max(0.0), ..VineFades::default() }
    }

    pub fn on(&self) -> bool {
        self.fade_s > 0.0
    }

    /// As [`StyleFades::begin`].
    pub fn begin(&mut self, now: f64) {
        if self.last.is_some_and(|last| now < last || now - last > JUMP_S) {
            self.ramps.clear();
        }
        self.last = Some(now);
    }

    /// `cell`'s tile changed from `old` to `new` at `now`. A cell already fading runs from
    /// where it is: back toward its old tile without a jump when `new` is that tile; to a
    /// third tile from whichever of its two shows more.
    pub fn switch(&mut self, cell: u32, old: Tile, new: Tile, now: f64) {
        if !self.on() || old == new {
            return;
        }
        let fade = f64::from(self.fade_s);
        let r = match self.ramps.get(&cell) {
            None => VineRamp { from: old, to: new, start: now },
            Some(r) => {
                let s = 1.0 - f64::from(progress(now, r.start, fade));
                if new == r.to {
                    *r
                } else if new == r.from {
                    // The old tile shows `s`: the new ramp's old tile (`r.to`) shows `1 − s`.
                    VineRamp { from: r.to, to: new, start: now - s * fade }
                } else {
                    VineRamp { from: if s >= 0.5 { r.from } else { r.to }, to: new, start: now }
                }
            }
        };
        self.ramps.insert(cell, r);
    }

    /// `cell` no longer carries a vine tile: its ramp, if any, ends.
    pub fn forget(&mut self, cell: u32) {
        self.ramps.remove(&cell);
    }

    /// Forget the ramps finished by `now`.
    pub fn retire(&mut self, now: f64) {
        let fade = f64::from(self.fade_s);
        self.ramps.retain(|_, r| now - r.start < fade);
    }

    /// `cell`'s old tile and the share of it still shown at `now` (`0..=1`), while it fades.
    pub fn old(&self, cell: u32, now: f64) -> Option<(Tile, f32)> {
        let r = self.ramps.get(&cell)?;
        let s = 1.0 - progress(now, r.start, f64::from(self.fade_s));
        (s > 0.0).then_some((r.from, s))
    }

    /// Every fading cell with its old tile and share at `now`, in no order.
    pub fn each(&self, now: f64, mut f: impl FnMut(u32, Tile, f32)) {
        for &c in self.ramps.keys() {
            if let Some((t, s)) = self.old(c, now) {
                f(c, t, s);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.ramps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ramps.is_empty()
    }
}

#[cfg(test)]
#[path = "colour_fade_tests.rs"]
mod tests;
