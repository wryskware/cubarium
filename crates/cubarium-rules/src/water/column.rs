//! The water phases that move water **only up and down a column**: fall, infiltration,
//! drainage and the water table (`design/handoffs/voxel-water-parallel-2026-09-24.md`,
//! W2). From `cubarium-voxel`'s `water.rs` (main a2cee21), the rules unchanged.
//!
//! Every transfer these phases make is between a cell and the cell directly above or below
//! it, or between a cell and the aquifer, so a column's result depends on nothing but that
//! column. The old phases walked the whole world's active set in ascending index order,
//! which is bottom-up; walking one column's rows bottom-up visits the same cells in the
//! same relative order, so the per-column rule is the old rule, and the columns can run
//! in any order or at once. The one thing columns share is the **aquifer**: what drainage
//! hands it is summed per column (a reassociated sum), and what the water table takes out
//! of it is a scarce stock shared in index order **by rule** — see [`saturate`].
//!
//! Stores are fractions of a cell, as in the world, and each primitive here is the
//! world's own ([`Cells::add_free`] is `add_free`, and so on) in the same arithmetic, so
//! the per-column rule moves the same volumes the old phase did. The active sets are kept
//! through [`Sets`], which the host implements as a column's row masks or as its own
//! sparse sets.

use super::{empty, fill, transfer_want};

/// A per-cell `f64` array the column phases read and write, in the world's index order:
/// a slice on one thread; on a pool, a pointer each task writes only its own columns
/// through (the host's drivers).
pub trait Store {
    fn get(&self, i: usize) -> f64;
    fn set(&mut self, i: usize, v: f64);
}

impl Store for [f64] {
    #[inline(always)]
    fn get(&self, i: usize) -> f64 {
        self[i]
    }
    #[inline(always)]
    fn set(&mut self, i: usize, v: f64) {
        self[i] = v;
    }
}

/// The world's three active sets, as the column phases edit them: the cells holding free
/// water (`wet`), pore water (`damp`), and pore water over their field capacity
/// (`drainable`). `i` is the cell, `y` its row. Called with the membership the cell has
/// just been given, whether or not it changed.
pub trait Sets {
    fn wet(&mut self, i: usize, y: usize, member: bool);
    fn damp(&mut self, i: usize, y: usize, member: bool);
    fn drainable(&mut self, i: usize, y: usize, member: bool);
}

/// One column's three set words, and how many members each gained (negative: lost).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Words {
    pub wet: u128,
    pub damp: u128,
    pub drainable: u128,
    pub delta: [isize; 3],
}

#[inline(always)]
fn flip(word: &mut u128, count: &mut isize, y: usize, member: bool) {
    let bit = 1u128 << y;
    if member {
        if *word & bit == 0 {
            *word |= bit;
            *count += 1;
        }
    } else if *word & bit != 0 {
        *word &= !bit;
        *count -= 1;
    }
}

impl Sets for Words {
    #[inline(always)]
    fn wet(&mut self, _: usize, y: usize, member: bool) {
        flip(&mut self.wet, &mut self.delta[0], y, member);
    }
    #[inline(always)]
    fn damp(&mut self, _: usize, y: usize, member: bool) {
        flip(&mut self.damp, &mut self.delta[1], y, member);
    }
    #[inline(always)]
    fn drainable(&mut self, _: usize, y: usize, member: bool) {
        flip(&mut self.drainable, &mut self.delta[2], y, member);
    }
}

/// What the column phases need to know about a cell's material.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ground {
    /// Solid to free water: everything but air.
    pub solid: bool,
    /// The impermeable foundation: what drains onto it joins the aquifer.
    pub bedrock: bool,
    /// The fraction of the cell that holds pore water; zero for air and bedrock.
    pub capacity: f64,
    /// Whether water moves through its pores at all (`permeability_per_s > 0`).
    pub permeable: bool,
    /// The pore fill it drains down to.
    pub field_capacity: f64,
    /// What one face passes in the phase's time step, m³ (`pore_flux_m3`): the host
    /// builds its table with the step the phase runs over.
    pub flux: f64,
}

/// A cell as the primitives take it: its index, its row, and its ground.
pub type At = (usize, usize, Ground);

/// The rows of a column mask, lowest first.
#[derive(Clone, Copy)]
pub struct Rows(pub u128);

impl Iterator for Rows {
    type Item = usize;
    #[inline(always)]
    fn next(&mut self) -> Option<usize> {
        if self.0 == 0 {
            return None;
        }
        let y = self.0.trailing_zeros() as usize;
        self.0 &= self.0 - 1;
        Some(y)
    }
}

/// The stores one column phase works on: free and pore water, the active sets, the voxel
/// volume in m³ and the plane (columns per row), with the world's store primitives over
/// them. Each primitive returns the volume it actually moved, m³.
pub struct Cells<'a, F: ?Sized + Store, P: ?Sized + Store, S: Sets> {
    pub free: &'a mut F,
    pub pore: &'a mut P,
    pub sets: &'a mut S,
    pub voxel: f64,
    pub plane: usize,
}

impl<F: ?Sized + Store, P: ?Sized + Store, S: Sets> Cells<'_, F, P, S> {
    #[inline(always)]
    pub fn free_m3(&self, i: usize, g: Ground) -> f64 {
        if g.solid { 0.0 } else { self.free.get(i) * self.voxel }
    }

    #[inline(always)]
    pub fn free_room_m3(&self, i: usize, g: Ground) -> f64 {
        if g.solid {
            0.0
        } else {
            (1.0 - self.free.get(i)).max(0.0) * self.voxel
        }
    }

    #[inline(always)]
    pub fn pore_m3(&self, i: usize, g: Ground) -> f64 {
        self.pore.get(i) * self.voxel * g.capacity
    }

    #[inline(always)]
    pub fn pore_room_m3(&self, i: usize, g: Ground) -> f64 {
        if g.capacity <= 0.0 {
            0.0
        } else {
            (1.0 - self.pore.get(i)).max(0.0) * self.voxel * g.capacity
        }
    }

    /// Whether drainage can move pore water out of the cell: porous, permeable, and
    /// holding more than its field capacity. The world's `drains`.
    #[inline(always)]
    pub fn drains(&self, i: usize, g: Ground) -> bool {
        let cap = g.capacity;
        if cap <= 0.0 || self.pore.get(i) <= 0.0 || !g.permeable {
            return false;
        }
        let unit = cap * self.voxel;
        self.pore_m3(i, g) - g.field_capacity * unit > 0.0
    }

    #[inline(always)]
    pub fn add_free(&mut self, (i, y, g): At, vol: f64) -> f64 {
        if vol <= 0.0 || g.solid {
            return 0.0;
        }
        let (after, moved) = fill(self.free.get(i), vol, self.voxel);
        self.free.set(i, after);
        self.sets.wet(i, y, after > 0.0);
        moved
    }

    #[inline(always)]
    pub fn take_free(&mut self, (i, y, g): At, vol: f64) -> f64 {
        if vol <= 0.0 || g.solid {
            return 0.0;
        }
        let (after, moved) = empty(self.free.get(i), vol, self.voxel);
        self.free.set(i, after);
        self.sets.wet(i, y, after > 0.0);
        moved
    }

    #[inline(always)]
    pub fn add_pore(&mut self, (i, y, g): At, vol: f64) -> f64 {
        let cap = g.capacity;
        if vol <= 0.0 || cap <= 0.0 {
            return 0.0;
        }
        let (after, moved) = fill(self.pore.get(i), vol, self.voxel * cap);
        self.pore.set(i, after);
        self.sets.damp(i, y, after > 0.0);
        let d = self.drains(i, g);
        self.sets.drainable(i, y, d);
        moved
    }

    #[inline(always)]
    pub fn take_pore(&mut self, (i, y, g): At, vol: f64) -> f64 {
        let cap = g.capacity;
        if vol <= 0.0 || cap <= 0.0 {
            return 0.0;
        }
        let (after, moved) = empty(self.pore.get(i), vol, self.voxel * cap);
        self.pore.set(i, after);
        self.sets.damp(i, y, after > 0.0);
        let d = self.drains(i, g);
        self.sets.drainable(i, y, d);
        moved
    }

    /// Free water from `from` into the free water of `to`, at most `vol`: what the
    /// destination gained, debited from the source exactly.
    #[inline(always)]
    fn free_to_free(&mut self, from: At, to: At, vol: f64) -> f64 {
        let want = transfer_want(vol, self.free_m3(from.0, from.2), self.free_room_m3(to.0, to.2));
        if want <= 0.0 {
            return 0.0;
        }
        let got = self.add_free(to, want);
        self.take_free(from, got);
        got
    }

    #[inline(always)]
    fn free_to_pore(&mut self, from: At, to: At, vol: f64) -> f64 {
        let want = transfer_want(vol, self.free_m3(from.0, from.2), self.pore_room_m3(to.0, to.2));
        if want <= 0.0 {
            return 0.0;
        }
        let got = self.add_pore(to, want);
        self.take_free(from, got);
        got
    }

    #[inline(always)]
    fn pore_to_free(&mut self, from: At, to: At, vol: f64) -> f64 {
        let want = transfer_want(vol, self.pore_m3(from.0, from.2), self.free_room_m3(to.0, to.2));
        if want <= 0.0 {
            return 0.0;
        }
        let got = self.add_free(to, want);
        self.take_pore(from, got);
        got
    }

    #[inline(always)]
    fn pore_to_pore(&mut self, from: At, to: At, vol: f64) -> f64 {
        let want = transfer_want(vol, self.pore_m3(from.0, from.2), self.pore_room_m3(to.0, to.2));
        if want <= 0.0 {
            return 0.0;
        }
        let got = self.add_pore(to, want);
        self.take_pore(from, got);
        got
    }
}

/// **Fall**, one column: every wet cell of the snapshot `rows` (the column's wet rows at
/// the start of the call, lowest first) hands what it can to the void cell below. A cell
/// wetted by the cell above it is not in the snapshot, so an arrival does not carry
/// farther down in the same call, while a wet cell above still gets its own turn and
/// makes room. The world's `fall`.
#[inline]
pub fn fall<F, P, S>(
    c: &mut Cells<'_, F, P, S>,
    col: usize,
    rows: impl Iterator<Item = usize>,
    ground: &impl Fn(usize) -> Ground,
) where
    F: ?Sized + Store,
    P: ?Sized + Store,
    S: Sets,
{
    for y in rows {
        // The bottom row has nowhere to fall to.
        if y == 0 {
            continue;
        }
        let i = y * c.plane + col;
        let g = ground(i);
        if g.solid || c.free.get(i) <= 0.0 {
            continue;
        }
        let below = i - c.plane;
        let gb = ground(below);
        let want = c.free_m3(i, g).min(c.free_room_m3(below, gb));
        if want > 0.0 {
            c.free_to_free((i, y, g), (below, y - 1, gb), want);
        }
    }
}

/// **Infiltration**, one column: each wet cell of the snapshot `rows` offers its free water
/// to the pores of the permeable cell directly below it, at that cell's own flux over the
/// phase's step (`Ground::flux`). The world's `infiltrate`.
#[inline]
pub fn infiltrate<F, P, S>(
    c: &mut Cells<'_, F, P, S>,
    col: usize,
    rows: impl Iterator<Item = usize>,
    ground: &impl Fn(usize) -> Ground,
) where
    F: ?Sized + Store,
    P: ?Sized + Store,
    S: Sets,
{
    for y in rows {
        if y == 0 {
            continue;
        }
        let i = y * c.plane + col;
        let g = ground(i);
        if g.solid || c.free.get(i) <= 0.0 {
            continue;
        }
        let below = i - c.plane;
        let gb = ground(below);
        if gb.capacity <= 0.0 || !gb.permeable {
            continue;
        }
        c.free_to_pore((i, y, g), (below, y - 1, gb), gb.flux);
    }
}

/// Whether the centre of row `y` lies at or below the water table `table` (metres above
/// `y = 0`).
#[inline(always)]
pub fn submerged(voxel_m: f64, y: usize, table: f64) -> bool {
    (y as f64 + 0.5) * voxel_m <= table
}

/// **Drainage**, one column: each cell of the snapshot `rows` (the column's drainable rows
/// at the start of the call, lowest first) moves pore water above its field capacity down,
/// at its own flux (`Ground::flux` over the tick): into the pores below, into free water
/// where it roofs a void, or into the aquifer where it sits on the foundation, on bedrock,
/// or on the saturated zone. A cell inside the saturated zone is skipped. Returns what the
/// aquifer gained, m³. The world's `drain`.
#[inline]
pub fn drain<F, P, S>(
    c: &mut Cells<'_, F, P, S>,
    col: usize,
    rows: impl Iterator<Item = usize>,
    ground: &impl Fn(usize) -> Ground,
    voxel_m: f64,
    table: f64,
) -> f64
where
    F: ?Sized + Store,
    P: ?Sized + Store,
    S: Sets,
{
    let mut gained = 0.0;
    for y in rows {
        if submerged(voxel_m, y, table) {
            continue;
        }
        let i = y * c.plane + col;
        let g = ground(i);
        let cap = g.capacity;
        if cap <= 0.0 || c.pore.get(i) <= 0.0 || !g.permeable {
            continue;
        }
        let unit = cap * c.voxel;
        let excess = c.pore_m3(i, g) - g.field_capacity * unit;
        if excess <= 0.0 {
            continue;
        }
        let want = excess.min(g.flux);
        let at = (i, y, g);
        if y == 0 {
            // Sitting on the foundation: what drains joins the aquifer.
            gained += c.take_pore(at, want);
            continue;
        }
        let below = i - c.plane;
        let gb = ground(below);
        if submerged(voxel_m, y - 1, table) && gb.capacity > 0.0 {
            // Reaching the water table: what drains recharges the aquifer.
            gained += c.take_pore(at, want);
        } else if gb.bedrock {
            gained += c.take_pore(at, want);
        } else if !gb.solid {
            c.pore_to_free(at, (below, y - 1, gb), want);
        } else {
            c.pore_to_pore(at, (below, y - 1, gb), want);
        }
    }
    gained
}

/// **Saturation**, one cell (the water table's first half): a permeable cell whose centre
/// is at or below the table has its pores filled toward full at its flux over the tick,
/// out of `budget` m³ of aquifer. Returns what it took. The caller walks the cells in
/// index order and draws `budget` down, which is how a scarce stock is shared **by rule**.
/// The caller only offers cells whose row is submerged.
#[inline(always)]
pub fn saturate<F, P, S>(c: &mut Cells<'_, F, P, S>, at: At, budget: f64) -> f64
where
    F: ?Sized + Store,
    P: ?Sized + Store,
    S: Sets,
{
    let (i, _, g) = at;
    if g.capacity <= 0.0 || !g.permeable {
        return 0.0;
    }
    let want = g.flux.min(c.pore_room_m3(i, g)).min(budget.max(0.0));
    if want <= 0.0 {
        return 0.0;
    }
    c.add_pore(at, want)
}

/// What [`saturate`] would take from an unlimited aquifer: the demand a driver sums to
/// learn whether the stock can bind at all.
#[inline(always)]
pub fn saturate_demand<F, P, S>(c: &Cells<'_, F, P, S>, i: usize, g: Ground) -> f64
where
    F: ?Sized + Store,
    P: ?Sized + Store,
    S: Sets,
{
    if g.capacity <= 0.0 || !g.permeable {
        return 0.0;
    }
    g.flux.min(c.pore_room_m3(i, g)).max(0.0)
}

/// **Seepage**, one cell (the water table's second half): a void cell the table reaches,
/// with no water standing over it and saturated permeable ground under it, takes free water
/// from the aquifer up to the table's level inside it, at the ground's flux, out of
/// `budget`. `level` is that level as a fill (`clamp((table − y·voxel_m) / voxel_m, 0,
/// 1)`), and `i` is at row 1 or above. Returns what it took.
#[inline(always)]
pub fn seep<F, P, S>(
    c: &mut Cells<'_, F, P, S>,
    i: usize,
    y: usize,
    level: f64,
    top_row: bool,
    ground: &impl Fn(usize) -> Ground,
    budget: f64,
) -> f64
where
    F: ?Sized + Store,
    P: ?Sized + Store,
    S: Sets,
{
    let want = seep_demand(c, i, level, top_row, ground).min(budget.max(0.0));
    if want <= 0.0 {
        return 0.0;
    }
    c.add_free((i, y, ground(i)), want)
}

/// What [`seep`] would take from an unlimited aquifer.
#[inline(always)]
pub fn seep_demand<F, P, S>(
    c: &Cells<'_, F, P, S>,
    i: usize,
    level: f64,
    top_row: bool,
    ground: &impl Fn(usize) -> Ground,
) -> f64
where
    F: ?Sized + Store,
    P: ?Sized + Store,
    S: Sets,
{
    let g = ground(i);
    if g.solid || c.free.get(i) >= level {
        return 0.0;
    }
    // Only a surface seeps: a cell with water standing over it is under that water's own
    // pressure.
    let above = i + c.plane;
    if !top_row && !ground(above).solid && c.free.get(above) > 1e-9 {
        return 0.0;
    }
    let below = i - c.plane;
    let gb = ground(below);
    if gb.capacity <= 0.0 || !gb.permeable {
        return 0.0;
    }
    // Only saturated ground seeps: unsaturated soil takes the water itself.
    if c.pore.get(below) < 1.0 - 1e-9 {
        return 0.0;
    }
    let room = (level - c.free.get(i)) * c.voxel;
    gb.flux.min(room).max(0.0)
}

/// The water table's rows: saturation covers rows `0..band`, seepage rows `1..seep_top`,
/// and `level(y)` is row `y`'s seepage level. From the table in metres; the world's own
/// arithmetic.
#[derive(Clone, Copy, Debug)]
pub struct Table {
    pub table: f64,
    pub voxel_m: f64,
    pub band: usize,
    pub seep_top: usize,
}

impl Table {
    pub fn new(table: f64, voxel_m: f64, height: usize) -> Table {
        let rows = crate::math::floor(table / voxel_m - 0.5) + 1.0;
        let band = if rows <= 0.0 { 0 } else { (rows as usize).min(height) };
        let mut seep_top = 1;
        while seep_top < height && (seep_top as f64) * voxel_m < table {
            seep_top += 1;
        }
        Table {
            table,
            voxel_m,
            band,
            seep_top,
        }
    }

    #[inline(always)]
    pub fn submerged(&self, y: usize) -> bool {
        submerged(self.voxel_m, y, self.table)
    }

    #[inline(always)]
    pub fn level(&self, y: usize) -> f64 {
        ((self.table - y as f64 * self.voxel_m) / self.voxel_m).clamp(0.0, 1.0)
    }
}
