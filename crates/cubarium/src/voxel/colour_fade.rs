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
//!   falling from 1 to 0, every frame on the lit tier. A third tile arriving mid-way waits
//!   for that ramp to end and then fades in the same way (round 3: it used to start from
//!   whichever of the two showed more, a jump of up to half).
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

/// A change of colour no larger than this in every channel of every one of a style's
/// colours, in sRGB (0..=1), is drawn at once rather than faded (round 3): below one
/// visible step, and what a continuously drifting source (the style path's wilt) moves by
/// between packs. Fading those kept every such cell re-packed every pack.
pub const SNAP_SRGB: f32 = 1.5 / 255.0;

fn srgb(c: f32) -> f32 {
    let c = c.max(0.0);
    if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

/// Whether `a` and `b` are within [`SNAP_SRGB`] of each other in every channel.
pub fn close(a: Style, b: Style) -> bool {
    [(a.wood, b.wood), (a.crown, b.crown), (a.heart, b.heart)]
        .iter()
        .all(|(x, y)| (0..3).all(|k| (srgb(x[k]) - srgb(y[k])).abs() <= SNAP_SRGB))
}

/// The plant cells part way through a style swap: see the module.
///
/// **The style table** (round 3): every cell of one *group* (the same old and new colours,
/// started the same pack) draws the same blend, one style slot a pack; a group moving on
/// to its next blend holds its old slot and its new one for that pack. The packer gives
/// [`StyleFades::begin`] the slots free of everything else, and no group starts or turns
/// toward a new target unless its two slots fit beside the groups running: a swap that
/// does not fit waits (the cell keeps the colours it shows, [`StyleFades::draw`]) and is
/// tried again next pack, so a meadow curing at once fades in waves rather than running
/// the table out (round 2's blends did, and the cells that missed a slot sat at their old
/// colours until their fade ended, then stepped).
#[derive(Debug, Default)]
pub struct StyleFades {
    fade_s: f32,
    ramps: FxHashMap<u32, StyleRamp>,
    /// Cells holding their colours until a swap fits, each since when.
    waits: FxHashMap<u32, f64>,
    /// The groups started this pack, and how many more may start.
    started: rustc_hash::FxHashSet<u64>,
    room: usize,
    last: Option<f64>,
}

/// A cell waits for room at most this many fades; then (the table full of other styles
/// for that long) its swap shows at once rather than never.
pub const WAIT_FADES: f64 = 8.0;

impl StyleRamp {
    /// Its group: ramps with equal `from`, `to` and `start` draw the same blends.
    fn group(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = rustc_hash::FxHasher::default();
        for c in [self.from, self.to] {
            for v in [c.wood, c.crown, c.heart] {
                for x in v {
                    x.to_bits().hash(&mut h);
                }
            }
        }
        self.start.to_bits().hash(&mut h);
        h.finish()
    }
}

impl StyleFades {
    pub fn new(fade_s: f32) -> StyleFades {
        StyleFades { fade_s: fade_s.max(0.0), room: usize::MAX, ..StyleFades::default() }
    }

    /// Whether swaps fade at all (`colour_fade_s` above 0).
    pub fn on(&self) -> bool {
        self.fade_s > 0.0
    }

    /// A pack at presenter time `now` begins, with `free` style slots free of everything
    /// but this pack's blends. A clock that went back or jumped further than [`JUMP_S`]
    /// (a load, a seek) ends every ramp, so each cell draws its colours as they stand.
    pub fn begin(&mut self, now: f64, free: usize) {
        if self.last.is_some_and(|last| now < last || now - last > JUMP_S) {
            self.ramps.clear();
            self.waits.clear();
        }
        self.last = Some(now);
        self.started.clear();
        // Each running group moves to a new blend this pack: a second slot for a pack.
        let mut running = rustc_hash::FxHashSet::default();
        for r in self.ramps.values() {
            running.insert(r.group());
        }
        self.room = free.saturating_sub(running.len());
    }

    /// Whether ramp `r` may start this pack: its group started already, or there is room
    /// for one more (two slots: its blend now and its next).
    fn fits(&mut self, r: &StyleRamp) -> bool {
        let g = r.group();
        if self.started.contains(&g) {
            return true;
        }
        if self.room < 2 {
            return false;
        }
        self.room -= 2;
        self.started.insert(g);
        true
    }

    /// What `cell` draws at `now`, given the style it should draw (`to`) and what it drew
    /// last pack (`held`; `None` when it drew nothing comparable). A cell whose target moved
    /// starts a ramp from what it showed, unless the move is too small to see ([`close`]);
    /// a ramp that reaches its target ends. With no room ([`StyleFades::begin`]) a ramp
    /// keeps its old target and a new one waits, the cell keeping what it showed, until
    /// one fits or [`WAIT_FADES`] fades have passed.
    pub fn draw(&mut self, cell: u32, held: Option<Style>, to: Style, now: f64) -> Style {
        if !self.on() {
            return to;
        }
        let fade = f64::from(self.fade_s);
        let mut held = held;
        if let Some(&r) = self.ramps.get(&cell) {
            let done = now - r.start >= fade;
            if r.to == to {
                if done {
                    self.ramps.remove(&cell);
                    return to;
                }
                return r.at(now, fade);
            }
            let turn = StyleRamp { from: r.at(now, fade), to, start: now };
            if self.fits(&turn) {
                self.ramps.insert(cell, turn);
                return turn.from;
            }
            if !done {
                // No room to turn: on toward the old target, and turn when there is.
                return r.at(now, fade);
            }
            self.ramps.remove(&cell);
            held = Some(r.to);
        }
        match held {
            Some(h) if h != to && !close(h, to) => {
                let r = StyleRamp { from: h, to, start: now };
                if self.fits(&r) {
                    self.waits.remove(&cell);
                    self.ramps.insert(cell, r);
                    return h;
                }
                let since = *self.waits.entry(cell).or_insert(now);
                if now - since <= WAIT_FADES * fade {
                    return h;
                }
                self.waits.remove(&cell);
                to
            }
            _ => {
                self.waits.remove(&cell);
                to
            }
        }
    }

    /// `cell` no longer draws a plant: its ramp, if any, ends.
    pub fn forget(&mut self, cell: u32) {
        self.ramps.remove(&cell);
        self.waits.remove(&cell);
    }

    /// Whether `cell` is waiting for room to start a swap.
    pub fn waiting(&self, cell: u32) -> bool {
        self.waits.contains_key(&cell)
    }

    /// Whether `cell` is part way through a swap.
    pub fn fading(&self, cell: u32) -> bool {
        self.ramps.contains_key(&cell)
    }

    /// Every cell part way through a swap or waiting to start one, in no order: each is
    /// packed again next pack.
    pub fn cells(&self) -> impl Iterator<Item = u32> + '_ {
        self.ramps.keys().chain(self.waits.keys()).copied()
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
    /// A third tile that came while this ramp ran (round 3): the cell keeps drawing `to`
    /// over `from` until the ramp ends, then fades from `to` into this one. Never shown
    /// before then, so a fourth tile simply takes its place.
    next: Option<Tile>,
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
    /// where it is: back toward its old tile without a jump when `new` is that tile; a
    /// third tile waits for the running ramp to end and then fades in from the tile that
    /// ramp ends on ([`VineFades::drawn`] is the tile the cell's texel carries meanwhile).
    pub fn switch(&mut self, cell: u32, old: Tile, new: Tile, now: f64) {
        if !self.on() || old == new {
            return;
        }
        let fade = f64::from(self.fade_s);
        let r = match self.ramps.get(&cell) {
            None => VineRamp { from: old, to: new, start: now, next: None },
            Some(r) => {
                let s = 1.0 - f64::from(progress(now, r.start, fade));
                if new == r.to {
                    VineRamp { next: None, ..*r }
                } else if new == r.from {
                    // The old tile shows `s`: the new ramp's old tile (`r.to`) shows `1 − s`.
                    VineRamp { from: r.to, to: new, start: now - s * fade, next: None }
                } else {
                    VineRamp { next: Some(new), ..*r }
                }
            }
        };
        self.ramps.insert(cell, r);
    }

    /// The tile `cell`'s texel carries in place of its own while a third tile waits
    /// ([`VineFades::switch`]): the running ramp's new tile. `None`: its own.
    pub fn drawn(&self, cell: u32) -> Option<Tile> {
        self.ramps.get(&cell).filter(|r| r.next.is_some()).map(|r| r.to)
    }

    /// `cell` no longer carries a vine tile: its ramp, if any, ends.
    pub fn forget(&mut self, cell: u32) {
        self.ramps.remove(&cell);
    }

    /// Forget the ramps finished by `now`. A finished ramp with a tile waiting starts the
    /// next from the tile it ended on; `started` is told each such cell, whose texel now
    /// carries its own tile again and must be packed.
    pub fn retire(&mut self, now: f64, mut started: impl FnMut(u32)) {
        let fade = f64::from(self.fade_s);
        self.ramps.retain(|&cell, r| {
            if now - r.start < fade {
                return true;
            }
            match r.next.take() {
                Some(n) => {
                    *r = VineRamp { from: r.to, to: n, start: now, next: None };
                    started(cell);
                    true
                }
                None => false,
            }
        });
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
