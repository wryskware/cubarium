//! Water: the store arithmetic and the **local exchange**, per column.
//!
//! From `cubarium-voxel`'s `water.rs` (retrain 169ae89, by the single-source trial): the store primitives'
//! arithmetic (`add_free`, `take_free`, `add_pore`, `take_pore`, `transfer`) and the
//! exchange that was `exchange_inner_with_masks` — heads from the packed full stack
//! (package H), head through submerged water ([`HEAD_PASSES`] Jacobi passes), offers
//! across four faces and down with displacement up the neighbour's void run, package P's
//! per-stack caps and Jacobi division, and the destination's acceptance. The rule is
//! unchanged; the **data layout is restructured** so that each phase is a function of
//! one column:
//!
//! - the wet cells are per-column row masks ([`Field::wet`], maintained by the host's wet
//!   set) instead of an ascending list of cells, so no phase walks the grid in index
//!   order and no phase needs a sort;
//! - a column's givers are grouped into their **stacks** inside the column (a stack is a
//!   run of wet rows that share one head, package P), and a stack's totals are computed
//!   by walking its rows twice instead of storing every giver's offers, so a phase keeps
//!   nothing per giver;
//! - what a cell receives is the sum of the proposals into it times its acceptance, and a
//!   giver's loss is recomputed from its own proposals and its destinations' acceptance,
//!   so no phase writes another column's cells except by an atomic add of a proposal.
//!
//! The phases, each over columns, separated by a barrier: [`column_heads`] (A), then
//! [`column_drive_pass`] up to [`HEAD_PASSES`] times, then [`column_edges`] into the
//! proposals (B), [`column_delta`] (C), and the commit ([`commit`], D). The host drivers
//! are in [`host`]; the trial's CUDA kernels (`crates/cubarium-rules-ptx`) stayed on the
//! `rules-trial` branch.
//!
//! The phases that move water only up and down a column — fall, infiltration, drainage
//! and the water table — are per-column rules too, in [`column`](mod@column), with their drivers in
//! [`host`] (`design/handoffs/voxel-water-parallel-2026-09-24.md`).
//!
//! **Parity is statistical.** The received volume is `Σq × accept` instead of `Σ(q ×
//! accept)`, and proposals into one cell are summed in column order (or in whatever order
//! the atomics land), so a trajectory leaves the old one in the last place. Conservation
//! holds to rounding: every volume one cell loses another gains.

pub mod column;
#[cfg(not(target_arch = "nvptx64"))]
pub mod host;

/// The fraction of a head difference that crosses one face in one substep. Placeholder.
pub const FLOW_PER_SUBSTEP: f64 = 0.5;

/// A cell with less than this much room left counts as full.
pub const ROOM_EPS: f64 = 1e-12;

/// How many local passes carry head through submerged water in one substep. Placeholder.
pub const HEAD_PASSES: u32 = 4;

/// The tallest world the per-column row masks cover.
pub const MASK_ROWS: usize = 128;

// ---------------------------------------------------------------- store arithmetic

/// Credit `vol` m³ to a store holding the fraction `before` of `unit` m³, up to full:
/// the store's new fraction and the volume it actually took. `add_free` and `add_pore`.
#[inline]
pub fn fill(before: f64, vol: f64, unit: f64) -> (f64, f64) {
    let after = (before + vol / unit).min(1.0);
    (after, (after - before).max(0.0) * unit)
}

/// Debit `vol` m³ from a store holding the fraction `before` of `unit` m³, down to
/// empty: the new fraction and the volume actually taken. `take_free` and `take_pore`.
#[inline]
pub fn empty(before: f64, vol: f64, unit: f64) -> (f64, f64) {
    let after = (before - vol / unit).max(0.0);
    (after, (before - after).max(0.0) * unit)
}

/// What a `transfer` of `vol` may move: no more than the source has and the destination
/// has room for.
#[inline]
pub fn transfer_want(vol: f64, have: f64, room: f64) -> f64 {
    vol.min(have).min(room)
}

// ---------------------------------------------------------------- geometry

/// The grid's extents. A cell is `y * plane + col`, a column `z * width + x`: the
/// world's own index order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    pub width: usize,
    pub depth: usize,
    pub height: usize,
}

/// No neighbour (past a `z` wall).
pub const NONE: usize = usize::MAX;

impl Grid {
    #[inline]
    pub fn plane(&self) -> usize {
        self.width * self.depth
    }

    #[inline]
    pub fn cells(&self) -> usize {
        self.plane() * self.height
    }

    /// A column's four horizontal neighbours in the exchange's face order — `x - 1`,
    /// `x + 1`, `z - 1`, `z + 1` — with [`NONE`] past a `z` wall. `x` wraps.
    #[inline]
    pub fn neighbours(&self, col: usize) -> [usize; 4] {
        let w = self.width;
        let z = col / w;
        let x = col - z * w;
        let row = z * w;
        [
            row + (x + w - 1) % w,
            row + (x + 1) % w,
            if z > 0 { col - w } else { NONE },
            if z + 1 < self.depth { col + w } else { NONE },
        ]
    }
}

/// Rows `y .. y + len` of a row mask.
#[inline]
pub fn run_bits(y: usize, len: usize) -> u128 {
    if len >= MASK_ROWS {
        u128::MAX
    } else {
        ((1u128 << len) - 1) << y
    }
}

/// Whether a fill counts as full.
#[inline]
pub fn is_full(free: f64) -> bool {
    free >= 1.0 - ROOM_EPS
}

// ---------------------------------------------------------------- the exchange

/// What the exchange reads: the water and geometry at the start of the substep. Cells in
/// the world's index order, masks per column (bit `y` is row `y`).
#[derive(Clone, Copy)]
pub struct Field<'a> {
    pub grid: Grid,
    pub free: &'a [f64],
    /// Non-solid rows.
    pub void: &'a [u128],
    /// Rows holding free water (`free > 0`).
    pub wet: &'a [u128],
    /// Wet rows that are full ([`is_full`]).
    pub full: &'a [u128],
    /// A wet cell's head, in cell units (phase A).
    pub head: &'a [f64],
    /// A wet cell's driving head: its head raised through submerged water.
    pub drive: &'a [f64],
}

/// The two numbers the exchange takes from the world's config.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// Package C's spreading depth in cell units: a giver shallower than this makes no
    /// horizontal offers.
    pub min_spread: f64,
    /// `Config::free_transfer_cap`: the most one face may move in one substep, cell
    /// units; zero is uncapped.
    pub transfer_cap: f64,
}

#[inline]
fn cap_flux(cap: f64, q: f64) -> f64 {
    if cap > 0.0 { q.min(cap) } else { q }
}

/// **Phase A**, one column: every wet cell's head from the packed full stack it sits at
/// the foot of (package H) — per wet run, from the top down, the top cell carries its own
/// surface and each cell below carries the head of the cell above it if it is full, or
/// its own level if not. `head(cell, h)` for every wet cell; returns the column's full
/// mask.
///
/// **Head from the packed full stack** (package H, 2026-09-22): with `s` the first row at
/// or above a wet cell that is not full, its head is `s + free[s]` when row `s` is wet —
/// the partial cell capping the stack — so a partial cell is its own level. A full stack
/// capped by a dry cell, a roof or the world top keeps its top cell's own surface, the old
/// run-top answer. What H changed is a **hollow** run, wet cells that are not full stacked
/// on each other: until then every cell of it carried the run top's surface, so the
/// bottom film of a falling stack pushed at a dry neighbour with a drop the height of the
/// stack and displaced slivers up the neighbour's column (the census at 382ef3a: 78 % of
/// a shower's active wet cells, 96 % of its proposals).
#[inline]
pub fn column_heads(grid: Grid, col: usize, wet: u128, free: &[f64], mut head: impl FnMut(usize, f64)) -> u128 {
    let plane = grid.plane();
    let mut full = 0u128;
    let mut m = wet;
    while m != 0 {
        let y = m.trailing_zeros() as usize;
        let run = (m >> y).trailing_ones() as usize;
        let top = y + run - 1;
        let mut at = top * plane + col;
        let f = free[at];
        let mut surface = top as f64 + f;
        if is_full(f) {
            full |= 1u128 << top;
        }
        head(at, surface);
        let mut r = top;
        while r > y {
            r -= 1;
            at -= plane;
            let f = free[at];
            if is_full(f) {
                full |= 1u128 << r;
            } else {
                surface = r as f64 + f;
            }
            head(at, surface);
        }
        m &= !run_bits(y, run);
    }
    full
}

/// **One pass of head through submerged water**, one column: every **full** wet cell
/// takes the highest driving head among itself and its wet horizontal neighbours on its
/// row, read from `drive_in`. `out(cell, drive)` for the full cells only (the others keep
/// their head, which the caller has in both buffers). Returns whether any drive rose.
#[inline]
pub fn column_drive_pass(
    grid: Grid,
    col: usize,
    wet: &[u128],
    full: &[u128],
    drive_in: &[f64],
    mut out: impl FnMut(usize, f64),
) -> bool {
    let plane = grid.plane();
    let nb = grid.neighbours(col);
    let mut raised = false;
    let mut m = full[col];
    while m != 0 {
        let y = m.trailing_zeros() as usize;
        m &= m - 1;
        let base = y * plane;
        let i = base + col;
        let own = drive_in[i];
        let mut h = own;
        for &n in &nb {
            if n == NONE || (wet[n] >> y) & 1 == 0 {
                continue;
            }
            let d = drive_in[base + n];
            if d > h {
                h = d;
            }
        }
        if h > own {
            raised = true;
        }
        out(i, h);
    }
    raised
}

/// How many rows of one stack [`column_edges`] keeps offers for between its two passes.
const KEPT_ROWS: usize = 8;

/// One giver's offers before its stack's caps: the raw flux and what found room across
/// each horizontal face, and what it offers down.
#[derive(Clone, Copy, Default)]
struct RowOffers {
    q: [f64; 4],
    placed: [f64; 4],
    down: f64,
}

/// **Displacement** up a neighbour's void run: `q` goes into the lowest cells with room at
/// or above row `y` of column `n`, inside its own void run, bottom first. The first target
/// skips rows that count as full ([`is_full`]); from there every positive sliver of room
/// takes what is left. `take(row, amount)` for each part; returns the total placed.
#[inline]
pub fn walk(f: &Field, n: usize, y: usize, q: f64, mut take: impl FnMut(usize, f64)) -> f64 {
    let run_len = (f.void[n] >> y).trailing_ones() as usize;
    if run_len == 0 {
        return 0.0;
    }
    let available = run_bits(y, run_len) & !f.full[n];
    if available == 0 {
        return 0.0;
    }
    let plane = f.grid.plane();
    let top = y + run_len - 1;
    let mut at = available.trailing_zeros() as usize;
    let mut left = q;
    let mut placed = 0.0;
    while left > 0.0 && at <= top {
        let room = (1.0 - f.free[at * plane + n]).max(0.0);
        if room > 0.0 {
            let t = left.min(room);
            take(at, t);
            placed += t;
            left -= t;
        }
        at += 1;
    }
    placed
}

#[inline]
fn row_offers(f: &Field, p: &Params, col: usize, nb: &[usize; 4], y: usize) -> RowOffers {
    let plane = f.grid.plane();
    let i = y * plane + col;
    let have = f.free[i];
    let here = f.drive[i];
    let mut o = RowOffers::default();
    // A giver shallower than the spreading depth offers across none of the four faces
    // (package C); it still offers downward.
    if have >= p.min_spread {
        for face in 0..4 {
            let n = nb[face];
            if n == NONE || (f.void[n] >> y) & 1 == 0 {
                continue;
            }
            let there = if (f.wet[n] >> y) & 1 != 0 {
                f.head[y * plane + n]
            } else {
                y as f64
            };
            let drop = here - there;
            if drop <= 0.0 {
                continue;
            }
            let q = cap_flux(p.transfer_cap, FLOW_PER_SUBSTEP * drop);
            o.q[face] = q;
            o.placed[face] = walk(f, n, y, q, |_, _| {});
        }
    }
    // Straight down into the cell's own room below. There is no upward push: a column
    // rises because water arriving at it is displaced up its run.
    if y > 0 && (f.void[col] >> (y - 1)) & 1 != 0 {
        let below = i - plane;
        let room = (1.0 - f.free[below]).max(0.0);
        let there = if (f.wet[col] >> (y - 1)) & 1 != 0 {
            f.head[below]
        } else {
            (y - 1) as f64
        };
        let drop = here - there;
        if drop > 0.0 && room > 0.0 {
            let q = cap_flux(p.transfer_cap, FLOW_PER_SUBSTEP * drop).min(room);
            if q > 0.0 {
                o.down = q;
            }
        }
    }
    o
}

/// **Phase B**, one column: every proposal its wet cells make this substep, after package
/// P's caps — a stack sends across one face at most what its best single row would, its
/// rows sharing that in proportion to their offers — the Jacobi division by the number
/// of faces the stack offers across, and the giver's own stock.
/// `emit(from_row, to_col, to_row, q)`, `q` in cell units, only for `q > 0`.
#[inline]
pub fn column_edges(f: &Field, p: &Params, col: usize, mut emit: impl FnMut(usize, usize, usize, f64)) {
    let plane = f.grid.plane();
    let nb = f.grid.neighbours(col);
    let wet = f.wet[col];
    let mut m = wet;
    while m != 0 {
        // One stack: from its lowest row, up while the next row is wet and shares the head.
        let s0 = m.trailing_zeros() as usize;
        let mut s1 = s0;
        while s1 + 1 < MASK_ROWS
            && (wet >> (s1 + 1)) & 1 != 0
            && f.head[(s1 + 1) * plane + col] == f.head[s1 * plane + col]
        {
            s1 += 1;
        }
        m &= !run_bits(s0, s1 - s0 + 1);

        // The stack's totals per face, and whether its bottom cell offers down out of it.
        // Each row's offers are kept for the emitting pass while the stack is short, and
        // recomputed past that (a stack is one or two rows almost always).
        let mut kept = [RowOffers::default(); KEPT_ROWS];
        let mut placed = [0.0f64; 4];
        let mut best = [0.0f64; 4];
        let mut down = false;
        for y in s0..=s1 {
            let o = row_offers(f, p, col, &nb, y);
            if y - s0 < KEPT_ROWS {
                kept[y - s0] = o;
            }
            for face in 0..4 {
                let pl = o.placed[face];
                if pl > 0.0 {
                    placed[face] += pl;
                    if pl > best[face] {
                        best[face] = pl;
                    }
                }
            }
            if y == s0 && o.down > 0.0 {
                down = true;
            }
        }
        let mut cap = [1.0f64; 4];
        let mut faces = usize::from(down);
        for face in 0..4 {
            if placed[face] > 0.0 {
                faces += 1;
                if placed[face] > best[face] {
                    cap[face] = best[face] / placed[face];
                }
            }
        }
        let share = 1.0 / faces.max(1) as f64;

        // The rows again, now emitting.
        for y in s0..=s1 {
            let o = if y - s0 < KEPT_ROWS {
                kept[y - s0]
            } else {
                row_offers(f, p, col, &nb, y)
            };
            let have = f.free[y * plane + col];
            let mut total = 0.0;
            for face in 0..4 {
                if o.placed[face] > 0.0 {
                    total += o.placed[face] * cap[face];
                }
            }
            total += o.down;
            if total <= 0.0 {
                continue;
            }
            let total = total * share;
            let scale = share * if total > have { have / total } else { 1.0 };
            for face in 0..4 {
                if o.placed[face] > 0.0 {
                    let k = scale * cap[face];
                    let n = nb[face];
                    walk(f, n, y, o.q[face], |to, t| {
                        let q = t * k;
                        if q > 0.0 {
                            emit(y, n, to, q);
                        }
                    });
                }
            }
            if o.down > 0.0 {
                let q = o.down * scale;
                if q > 0.0 {
                    emit(y, col, y - 1, q);
                }
            }
        }
    }
}

/// The share of what was proposed into a cell that it accepts: all of it, or its room
/// over the proposals when they overfill it together.
#[inline]
pub fn accept(proposed: f64, free: f64) -> f64 {
    let room = (1.0 - free).max(0.0);
    if proposed > room && proposed > 0.0 {
        room / proposed
    } else {
        1.0
    }
}

/// What one proposal of `q` actually moves, once its destination's acceptance is known:
/// the debit of its giver and a share of its destination's credit. For a driver that
/// keeps phase B's proposals instead of recomputing them in phase C.
#[inline]
pub fn moved(q: f64, proposed_to: f64, free_to: f64) -> f64 {
    q * accept(proposed_to, free_to)
}

/// Phase C's **credits**, one column: every touched row's proposals times its
/// acceptance, `out(cell, credit)`.
#[inline]
pub fn column_credits(f: &Field, col: usize, touched: u128, proposed: &[f64], mut out: impl FnMut(usize, f64)) {
    let plane = f.grid.plane();
    let mut m = touched;
    while m != 0 {
        let y = m.trailing_zeros() as usize;
        m &= m - 1;
        let i = y * plane + col;
        let pr = proposed[i];
        if pr > 0.0 {
            out(i, pr * accept(pr, f.free[i]));
        }
    }
}

/// **Phase C**, one column: the net change of its cells. `out(cell, d)` is called with
/// every **credit** ([`column_credits`]) and every **debit** — each of the column's own
/// proposals, recomputed, times its destination's acceptance; the caller sums them per
/// cell. `touched` is the column's rows that were proposed into; `proposed` the
/// proposals, in the world's index order. A driver that kept phase B's proposals debits
/// them with [`moved`] instead of recomputing them.
#[inline]
pub fn column_delta(f: &Field, p: &Params, col: usize, touched: u128, proposed: &[f64], mut out: impl FnMut(usize, f64)) {
    let plane = f.grid.plane();
    column_credits(f, col, touched, proposed, &mut out);
    if f.wet[col] == 0 {
        return;
    }
    column_edges(f, p, col, |from, to_col, to_row, q| {
        let t = to_row * plane + to_col;
        let m = moved(q, proposed[t], f.free[t]);
        if m > 0.0 {
            out(from * plane + col, -m);
        }
    });
}

/// **Phase D**: a cell's new fill from its net change.
#[inline]
pub fn commit(free: f64, delta: f64) -> f64 {
    (free + delta).clamp(0.0, 1.0)
}
