//! Water: prescribed rain and evaporation, a conservative free-water solver over the
//! void voxels (fall, then equalize every connected water region to one surface level),
//! infiltration into soil pores, drainage to the aquifer, spring discharge where head
//! exceeds the spring cell, and one named outlet that exports.
//!
//! # One tick
//!
//! Every rate below is per second and is applied over one [`crate::DT`]. Only step 3 is
//! subdivided; the re-levelling inside it has no rate in it, so nothing there is divided
//! by `water_substeps`, but infiltration does have one and it is applied over the
//! substep's own `DT / water_substeps`. Every interface is limited at both ends as well —
//! by the donor's water and the receiver's room — and the material named with it is the
//! one whose constants set the rate.
//!
//! 1. **Rain**: `rain_m_per_s * DT` metres of water onto each column's sky-exposed
//!    void cell, limited by that cell's room. A roof shadows what is under it: the rain
//!    lands on the roof.
//! 2. **Evaporation**: `evaporation_m_per_s * DT` metres off each column's sky-exposed
//!    free-water surface, limited by the water there. Water under a roof does not
//!    evaporate.
//! 3. **`water_substeps` free-water substeps**, each in this order: *infiltrate* (free
//!    water into the porous cell directly below it, the **receiving** cell's material
//!    setting the rate at `permeability_per_s * sub_dt` of its own `pore_capacity`, per
//!    voxel volume), then *fall* (every void cell hands its water to the void cell below
//!    while that has room, one cell per substep), then *equalize* (every connected water
//!    region settles to one surface level).
//! 4. **Drainage**: pore water above the cell's own `field_capacity` moves down — into
//!    the pore space below, into the aquifer where a porous cell sits on bedrock or on
//!    the foundation, or as a drip into free water where a porous cell roofs a void. A
//!    voxel inside the saturated zone is skipped: it has nowhere lower to go. The
//!    **donor** cell's material sets both the threshold and the rate
//!    (`permeability_per_s * DT` of its own pore capacity). This is field-capacity
//!    drainage, not "saturated soil only": rock above its field capacity drips too, just
//!    very slowly, and soil stops draining at `field_capacity` rather than at zero —
//!    that fraction is the retained water ecology gets to read.
//! 5. **Water table**: the aquifer saturates the pores of every permeable voxel whose
//!    centre lies at or below its head, and seeps free water into the void cells the
//!    head reaches over saturated ground. `drain` leaves the saturated zone alone. See
//!    [`water_table`] — this is what makes low ground wetter than high ground.
//! 6. **Spring**: out of the aquifer into the spring cell. See below.
//! 7. **Outlet**: while open, the one named outlet cell exports up to
//!    `outlet_m3_per_s * DT` of its free water into `Ledger::outlet_out`. This is the
//!    separate named export and has nothing to do with the spring.
//!
//! # Runoff is what infiltration refuses
//!
//! Infiltration is offered **before** anything moves the water, and inside the substep
//! rather than once at the end of the tick, because that is the physical statement: rain
//! reaches the ground, the ground takes what it can take, and what is left over runs
//! off. The other order — flow first, infiltrate with whatever is still standing at the
//! end of the tick — makes a slope shed water it never refused, since `equalize` carries
//! a film downhill within the same tick it landed in and the soil under it is never
//! asked. It is not a small effect: on the eight-column staircase in `tests/core.rs`,
//! under rain at a seventh of what the soil could absorb, the flat foot of the stairs
//! reached field capacity (pore 0.25) while the top step held 0.0001 — the whole
//! staircase was dry in proportion to its slope, and a plant layer reading pore water
//! would have found no soil moisture anywhere but in the hollows.
//!
//! What is left standing at the end of a tick is therefore genuine transit water: runoff
//! from cells that could not take it (rock, saturated soil) on its way to somewhere it
//! can rest. It is thin. On the default generated world under rain, the film over a
//! skyline support face went from 0.26 mm to 0.0025 mm when infiltration moved into the
//! substep — but it is still there, and a reader that treats *any* free water as
//! standing water will see it, because each substep ends with `equalize` rather than
//! with something that absorbs. Ending the substep with `fall` instead does not remove
//! it (a film resting on the ground has nowhere to fall to) and does move a
//! noticeably larger share of the world's water into the soil, so the order stays
//! fall-then-equalize.
//!
//! # The spring, and what it is not
//!
//! `h_spring = y * voxel_m`: the elevation of the spring cell's own floor, in metres
//! above `y = 0`, from that cell's `y`. `head` is [`crate::Config::aquifer_head_m`] of
//! the current store — the aquifer is a single number, not a field. The drive is
//! `max(head - h_spring, 0)`, so a spring cell at or above the head discharges nothing
//! at all, and
//!
//! ```text
//! Q = spring_k_m2_per_s * max(head - h_spring, 0)
//! ```
//!
//! The volume that actually leaves the aquifer in one tick is the smallest of `Q * DT`,
//! the aquifer's whole stock, and the room at the destination. The seep emerges at the
//! named cell and, if that is brim full, in the void cells above it, so a spring under
//! standing water still reaches the surface; a solid stops it there. Whatever it could
//! not push out **stays in the aquifer**: a full or roofed spring cell means zero
//! discharge and an unchanged store, never water lost.
//!
//! **Limitation, plainly: discharge is one-way this wave.** There is no submerged
//! backpressure. A pool standing over the spring does not push back on it — only the
//! destination's *room* limits the flow — and water never runs from the cell back into
//! the aquifer. So this is a room-limited source, not a groundwater equilibrium, and the
//! only path back into the aquifer is drainage (step 5).
//!
//! # Connectivity: what a region is
//!
//! A region is grown from a seed void cell that holds water. Its candidates are the void
//! cells 6-connected to it, and they are admitted **a whole row of one `y` at a time**:
//! a row joins only if the level the region would settle to after taking that whole row
//! in still lies strictly above the highest cell in the region, the row included. So the
//! region is always a body of water whose own surface submerges every cell it reaches
//! through — under roofs as readily as in the open, since nothing in the rule looks at
//! the sky.
//!
//! ## Row order, and why a row is all or nothing
//!
//! Seeds are taken lowest-water-surface first, then in index order. Rows are taken in
//! this order: the region's own surface row first, then upwards, and only then the rows
//! *below* it, lowest first. So the body spreads out at its own level before it looks for
//! a way down, and by the time it does, every way down at a given `y` is in hand and they
//! are judged together. A row refused while the level was still low is offered again in
//! the next round, since taking other water in can lift the level within reach of it.
//!
//! Judging a row whole is what keeps the answer off the order the cells were reached in.
//! Two identical hollows either side of a perched shelf are one row: admitting both would
//! drop the level below the shelf, so *neither* is admitted, and the shelf drains into
//! both by falling, a cell per substep, at the same rate. Admitting one of them — which is
//! what judging candidates one at a time did — made the shelf pick the side the walk
//! happened to reach first and left the other hollow short for good.
//!
//! Index order inside a row is a tie-break for determinism only, and the fill picks no
//! direction either: a region fills bottom-up and every cell at the surface level gets
//! the *same* share. What is actually checked is three fixtures in `tests/core.rs` — the
//! seam shift, the mirrored shelf and the symmetric spill — each settling to the
//! translated or mirrored answer: exactly where they reach a common level, and to within
//! a few times 1e-8 in a spill, whose stopping substep turns on a float comparison. That
//! is three fixtures, not a proof that every geometry is order independent.
//!
//! The level rule is what the U-tube and roofed-passage tests pin down. A shaft on the far
//! side of a bottom connection, or of a roofed passage, joins the region only when the
//! common level would stand above the connection, which is exactly when real water
//! would push through it — so both sides rise together and end level, and neither the
//! roof nor the passage floor stops them. The converse holds too: a barrier whose top
//! is above the level is never crossed, and a spill over a low sill moves only the
//! water that stands above the sill, a substep at a time, which leaves the two sides
//! unequal afterwards the way real water does.
//!
//! # Limits, plainly
//!
//! - **No inertia.** Water has no momentum; nothing sloshes, overshoots or oscillates.
//! - **No current.** There is no velocity field and no flow direction: water that is
//!   in one region is simply re-levelled. A waterfall is a column of cells that each
//!   hand their water down one cell per substep, not a jet.
//! - **Settling is instantaneous within a region, per substep.** A lake 100 cells wide
//!   levels in one substep, however far the water has to travel. Set
//!   `Config::free_transfer_cap` above zero to cap how much one cell's fill may change
//!   per substep and watch a fill spread instead; the default of zero leaves it
//!   instantaneous.
//! - **Water above the level is not part of the region.** A film running down a slope
//!   descends a cell per substep rather than arriving at once.
//! - **`f64` stores.** `free` and `pore` are `f64` fractions, so an internal transfer
//!   debits its source exactly what it credited its destination. There is no
//!   quantization term: the ledger residual `stored - initial_stored - net_in` is the
//!   raw conservation error and nothing corrects it.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::{Command, Config, Material, World, DT};

// ---------------------------------------------------------------- store primitives
//
// Every one of these returns the volume the store *actually* moved, so the ledger can
// be written from what happened rather than from what was asked for.

#[inline]
fn voxel(w: &World) -> f64 {
    w.config.voxel_volume()
}

fn free_m3(w: &World, i: usize) -> f64 {
    if w.material[i].is_solid() { 0.0 } else { w.free[i] * voxel(w) }
}

fn free_room_m3(w: &World, i: usize) -> f64 {
    if w.material[i].is_solid() { 0.0 } else { (1.0 - w.free[i]).max(0.0) * voxel(w) }
}

fn pore_m3(w: &World, i: usize) -> f64 {
    w.pore[i] * voxel(w) * w.material[i].pore_capacity()
}

fn pore_room_m3(w: &World, i: usize) -> f64 {
    let cap = w.material[i].pore_capacity();
    if cap <= 0.0 { 0.0 } else { (1.0 - w.pore[i]).max(0.0) * voxel(w) * cap }
}

fn add_free(w: &mut World, i: usize, vol: f64) -> f64 {
    if vol <= 0.0 || w.material[i].is_solid() {
        return 0.0;
    }
    let v = voxel(w);
    let before = w.free[i];
    w.free[i] = (before + vol / v).min(1.0);
    (w.free[i] - before).max(0.0) * v
}

fn take_free(w: &mut World, i: usize, vol: f64) -> f64 {
    if vol <= 0.0 || w.material[i].is_solid() {
        return 0.0;
    }
    let v = voxel(w);
    let before = w.free[i];
    w.free[i] = (before - vol / v).max(0.0);
    (before - w.free[i]).max(0.0) * v
}

fn add_pore(w: &mut World, i: usize, vol: f64) -> f64 {
    let cap = w.material[i].pore_capacity();
    if vol <= 0.0 || cap <= 0.0 {
        return 0.0;
    }
    let unit = voxel(w) * cap;
    let before = w.pore[i];
    w.pore[i] = (before + vol / unit).min(1.0);
    (w.pore[i] - before).max(0.0) * unit
}

fn take_pore(w: &mut World, i: usize, vol: f64) -> f64 {
    let cap = w.material[i].pore_capacity();
    if vol <= 0.0 || cap <= 0.0 {
        return 0.0;
    }
    let unit = voxel(w) * cap;
    let before = w.pore[i];
    w.pore[i] = (before - vol / unit).max(0.0);
    (before - w.pore[i]).max(0.0) * unit
}

/// Which store a transfer touches at one end.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Store {
    Free,
    Pore,
}

/// Move at most `vol` from one cell's store to another's, crediting the destination
/// with what it gained and debiting the source with exactly that same volume.
fn transfer(w: &mut World, from: (usize, Store), to: (usize, Store), vol: f64) -> f64 {
    let have = match from {
        (i, Store::Free) => free_m3(w, i),
        (i, Store::Pore) => pore_m3(w, i),
    };
    let room = match to {
        (i, Store::Free) => free_room_m3(w, i),
        (i, Store::Pore) => pore_room_m3(w, i),
    };
    let want = vol.min(have).min(room);
    if want <= 0.0 {
        return 0.0;
    }
    let got = match to {
        (i, Store::Free) => add_free(w, i, want),
        (i, Store::Pore) => add_pore(w, i, want),
    };
    let _ = match from {
        (i, Store::Free) => take_free(w, i, got),
        (i, Store::Pore) => take_pore(w, i, got),
    };
    got
}

// ---------------------------------------------------------------- geometry helpers

/// The six face neighbours. `x` wraps; `y` and `z` stop at the walls.
fn neighbours(c: &Config, i: usize) -> [Option<usize>; 6] {
    let (x, y, z) = c.coords(i);
    let x = x as i64;
    let mut out = [None; 6];
    out[0] = Some(c.index(x - 1, y, z));
    out[1] = Some(c.index(x + 1, y, z));
    if y > 0 {
        out[2] = Some(c.index(x, y - 1, z));
    }
    if y + 1 < c.height {
        out[3] = Some(c.index(x, y + 1, z));
    }
    if z > 0 {
        out[4] = Some(c.index(x, y, z - 1));
    }
    if z + 1 < c.depth {
        out[5] = Some(c.index(x, y, z + 1));
    }
    out
}

/// The void cell in column `(x, z)` that sees the sky: the first cell above whatever
/// the sky meets looking down, or that cell itself while it still has room. `None` when
/// the column is packed to the ceiling.
fn sky_cell(w: &World, x: i64, z: u32) -> Option<usize> {
    let c = &w.config;
    for y in (0..c.height).rev() {
        let i = c.index(x, y, z);
        if w.material[i].is_solid() || w.free[i] >= 1.0 {
            return if y + 1 < c.height { Some(c.index(x, y + 1, z)) } else { None };
        }
        if w.free[i] > 0.0 {
            return Some(i);
        }
    }
    Some(c.index(x, 0, z))
}

/// The sky-exposed free-water surface of column `(x, z)`, if it has one. A solid met
/// first means the column is dry on top or its water is roofed.
fn open_water_cell(w: &World, x: i64, z: u32) -> Option<usize> {
    let c = &w.config;
    for y in (0..c.height).rev() {
        let i = c.index(x, y, z);
        if w.material[i].is_solid() {
            return None;
        }
        if w.free[i] > 0.0 {
            return Some(i);
        }
    }
    None
}

// ---------------------------------------------------------------- the tick

pub fn step(world: &mut World) {
    rain(world);
    evaporate(world);
    let substeps = world.config.water_substeps.max(1);
    let sub_dt = DT / substeps as f64;
    for _ in 0..substeps {
        // Infiltration first, and inside the substep: water standing on a permeable
        // cell is offered to it before anything moves the water somewhere else. See
        // the module doc on why runoff is what infiltration refuses.
        infiltrate(world, sub_dt);
        fall(world);
        equalize(world);
    }
    drain(world);
    water_table(world);
    spring(world);
    outlet(world);
}

fn rain(w: &mut World) {
    let per_column = w.config.rain_m_per_s * DT * w.config.cell_area();
    if per_column <= 0.0 {
        return;
    }
    let (width, depth) = (w.config.width as i64, w.config.depth);
    let mut credited = 0.0;
    for z in 0..depth {
        for x in 0..width {
            if let Some(i) = sky_cell(w, x, z) {
                credited += add_free(w, i, per_column);
            }
        }
    }
    w.ledger.rain_in += credited;
}

fn evaporate(w: &mut World) {
    let per_column = w.config.evaporation_m_per_s * DT * w.config.cell_area();
    if per_column <= 0.0 {
        return;
    }
    let (width, depth) = (w.config.width as i64, w.config.depth);
    let mut debited = 0.0;
    for z in 0..depth {
        for x in 0..width {
            if let Some(i) = open_water_cell(w, x, z) {
                debited += take_free(w, i, per_column);
            }
        }
    }
    w.ledger.evaporation_out += debited;
}

/// Every void cell hands what it can to the void cell below. Purely vertical, so no
/// horizontal direction is picked; a column compacts by one cell per substep.
fn fall(w: &mut World) {
    let c = w.config.clone();
    for z in 0..c.depth {
        for x in 0..c.width as i64 {
            for y in 1..c.height {
                let i = c.index(x, y, z);
                if w.free[i] <= 0.0 || w.material[i].is_solid() {
                    continue;
                }
                let below = c.index(x, y - 1, z);
                let want = free_m3(w, i).min(free_room_m3(w, below));
                if want > 0.0 {
                    transfer(w, (i, Store::Free), (below, Store::Free), want);
                }
            }
        }
    }
}

/// Surface level of a region, in cell units, given how many of its cells sit at each
/// `y` and how much water it holds (also in cell units: one full cell is `1.0`).
fn level(counts: &[u32], mut rem: f64) -> f64 {
    let mut top = 0.0;
    for (y, &k) in counts.iter().enumerate() {
        if k == 0 {
            continue;
        }
        let cap = k as f64;
        if rem >= cap {
            rem -= cap;
            top = (y + 1) as f64;
        } else {
            return y as f64 + (rem / cap).max(0.0);
        }
    }
    top
}

/// Whether the centre of row `y` lies at or below the water table `table`, in metres
/// above `y = 0`. The centre, not the floor: a voxel counts as part of the saturated
/// zone once the table has reached the middle of it.
#[inline]
fn submerged(c: &Config, y: u32, table: f64) -> bool {
    (y as f64 + 0.5) * c.voxel_m <= table
}

/// The water table: the aquifer and the world exchanging water according to head.
///
/// `Config::aquifer_head_m` turns the aquifer store into a level above `y = 0`, and that
/// level is a real boundary rather than just the spring's drive:
///
/// 1. **Saturation.** Every permeable voxel whose centre lies at or below the table has
///    its pores filled toward `pore = 1` from the aquifer, at that material's
///    `permeability_per_s * DT` of its own pore capacity, capped by the aquifer's stock.
///    `drain` leaves those voxels alone, so soil under the table sits saturated instead
///    of settling back to its field capacity.
/// 2. **Seepage.** A void voxel the table reaches, standing on a permeable voxel that is
///    itself saturated, takes free water from the aquifer up to the table's own level
///    inside that cell — `clamp((table - y * voxel_m) / voxel_m, 0, 1)` of the cell — at
///    the *support's* permeability rate, because the water has to come up through it.
///    Seepage never lifts a free surface above the table, and a cell already at or above
///    it takes nothing: from there the path back down is the ordinary
///    infiltrate-then-drain.
///
/// Both directions are internal transfers between the aquifer and a voxel, so the ledger
/// has nothing to say about them and the conservation residual must not move. The table
/// is read once per step and held fixed for the whole of it, so the order voxels are
/// filled in cannot change the answer; within a step the fill runs bottom-up in index
/// order, which is also the order a scarce stock is shared in.
///
/// The named spring is untouched and still one cell with its own conductance: it is the
/// world's one *feature*, while this is the ambient boundary the whole world sits on.
fn water_table(w: &mut World) {
    let c = w.config.clone();
    let table = c.aquifer_head_m(w.aquifer_m3);
    if !(table > 0.0) {
        return;
    }
    // What the aquifer gives up this step, accumulated and debited once. Subtracting a
    // microlitre from a store of tens of cubic metres forty thousand times a tick is
    // forty thousand roundings against the store's own ulp; one subtraction is one.
    let mut taken = 0.0;
    let charged = w.aquifer_m3;

    for y in 0..c.height {
        if !submerged(&c, y, table) {
            continue;
        }
        for z in 0..c.depth {
            for x in 0..c.width as i64 {
                let i = c.index(x, y, z);
                let m = w.material[i];
                if m.pore_capacity() <= 0.0 || m.permeability_per_s() <= 0.0 {
                    continue;
                }
                let rate = m.permeability_per_s() * DT * m.pore_capacity() * c.voxel_volume();
                let want = rate.min(pore_room_m3(w, i)).min((charged - taken).max(0.0));
                if want <= 0.0 {
                    continue;
                }
                taken += add_pore(w, i, want);
            }
        }
    }

    for y in 1..c.height {
        // Above the table there is nothing to seep into.
        if (y as f64) * c.voxel_m >= table {
            break;
        }
        let level = ((table - y as f64 * c.voxel_m) / c.voxel_m).clamp(0.0, 1.0);
        for z in 0..c.depth {
            for x in 0..c.width as i64 {
                let i = c.index(x, y, z);
                if w.material[i].is_solid() || w.free[i] >= level {
                    continue;
                }
                let below = c.index(x, y - 1, z);
                let m = w.material[below];
                if m.pore_capacity() <= 0.0 || m.permeability_per_s() <= 0.0 {
                    continue;
                }
                // Only saturated ground seeps: unsaturated soil takes the water itself.
                if w.pore[below] < 1.0 - 1e-9 {
                    continue;
                }
                let rate = m.permeability_per_s() * DT * m.pore_capacity() * c.voxel_volume();
                let room = (level - w.free[i]) * c.voxel_volume();
                let want = rate.min(room).min((charged - taken).max(0.0));
                if want <= 0.0 {
                    continue;
                }
                taken += add_free(w, i, want);
            }
        }
    }

    w.aquifer_m3 = (charged - taken).max(0.0);
}

/// Settle every connected water region to one surface level. See the module doc for
/// what "connected" means here.
fn equalize(w: &mut World) {
    let c = w.config.clone();
    let n = c.cells();
    let plane = c.width as usize * c.depth as usize;
    let height = c.height as usize;

    let mut seeds: Vec<(f64, usize)> = Vec::new();
    for i in 0..n {
        if !w.material[i].is_solid() && w.free[i] > 0.0 {
            seeds.push(((i / plane) as f64 + w.free[i], i));
        }
    }
    if seeds.is_empty() {
        return;
    }
    seeds.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut done = vec![false; n];
    let mut stamp = vec![0u32; n];
    let mut queued = vec![0u32; n];
    let mut counts = vec![0u32; height];
    let mut fracs = vec![0f64; height];
    // Candidates waiting to be judged, filed under their own `y`, and the round in which
    // that row was last refused. `rows` is non-empty only at the `y`s listed in `touched`.
    let mut rows: Vec<Vec<usize>> = vec![Vec::new(); height];
    let mut refused = vec![0u32; height];
    let mut touched: Vec<usize> = Vec::new();
    let mut region: Vec<usize> = Vec::new();
    let mut heap: BinaryHeap<Reverse<(usize, u8, usize)>> = BinaryHeap::new();
    let mut epoch = 0u32;
    let mut round = 0u32;

    for (_, seed) in seeds {
        if done[seed] {
            continue;
        }
        epoch += 1;
        region.clear();
        heap.clear();
        stamp[seed] = epoch;
        queued[seed] = epoch;
        region.push(seed);
        let mut fill = w.free[seed];
        let mut y_max = seed / plane;
        counts[y_max] = 1;
        push_neighbours(&c, w, seed, epoch, &stamp, &done, &mut queued, &mut heap);

        // Growth takes a whole row of candidates or none of it, and it takes the region's
        // own surface row and the rows above it before any row below: the body spreads out
        // at its own level first and only then looks for a way down, so when there are
        // several ways down at one `y` they are judged together instead of one at a time.
        // That is what keeps two equal exits from being decided by which one the walk
        // reached first. A row refused while the level was low is offered again in the
        // next round, since taking other water in can lift the level within reach of it.
        for _ in 0..4 {
            round += 1;
            let mut committed = false;
            loop {
                // File everything reachable so far under its own `y`. `queued` keeps a
                // cell out of more than one row, so a row never counts one twice.
                while let Some(Reverse((y, _, i))) = heap.pop() {
                    if stamp[i] == epoch || done[i] {
                        continue;
                    }
                    if rows[y].is_empty() {
                        touched.push(y);
                    }
                    rows[y].push(i);
                }
                let Some(y) = (y_max..height)
                    .chain(0..y_max)
                    .find(|&y| refused[y] != round && !rows[y].is_empty())
                else {
                    break;
                };

                let batch = std::mem::take(&mut rows[y]);
                let taking = batch.len() as u32;
                let candidates: f64 = batch.iter().map(|&i| w.free[i]).sum();
                counts[y] += taking;
                let l = level(&counts, fill + candidates);
                let top = y_max.max(y) as f64;
                if l > top + 1e-12 {
                    fill += candidates;
                    y_max = y_max.max(y);
                    for &i in &batch {
                        stamp[i] = epoch;
                        region.push(i);
                    }
                    for &i in &batch {
                        push_neighbours(&c, w, i, epoch, &stamp, &done, &mut queued, &mut heap);
                    }
                    committed = true;
                } else {
                    counts[y] -= taking;
                    refused[y] = round;
                    // Not reachable at this level. Keep the row for the next round.
                    rows[y] = batch;
                }
            }
            if !committed {
                break;
            }
        }
        for &y in &touched {
            rows[y].clear();
        }
        touched.clear();

        // Fill the region bottom up, splitting what is left across the cells that sit
        // at the surface level.
        let mut rem = fill;
        for y in 0..height {
            if counts[y] == 0 {
                continue;
            }
            let cap = counts[y] as f64;
            if rem >= cap - 1e-12 {
                fracs[y] = 1.0;
                rem = (rem - cap).max(0.0);
            } else if rem > 0.0 {
                fracs[y] = rem / cap;
                rem = 0.0;
            } else {
                fracs[y] = 0.0;
            }
        }
        debug_assert!(rem < 1e-9, "a region cannot hold more water than its own cells");

        // Optional per-substep cap: relax every cell of the region toward its target by
        // the same factor, which keeps the total exactly.
        let mut relax = 1.0;
        if c.free_transfer_cap > 0.0 {
            let mut worst = 0.0f64;
            for &i in &region {
                let d = (fracs[i / plane] - w.free[i]).abs();
                worst = worst.max(d);
            }
            if worst > c.free_transfer_cap {
                relax = c.free_transfer_cap / worst;
            }
        }

        for &i in &region {
            let before = w.free[i];
            let target = before + relax * (fracs[i / plane] - before);
            w.free[i] = target.clamp(0.0, 1.0);
            done[i] = true;
        }
        for &i in &region {
            counts[i / plane] = 0;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn push_neighbours(
    c: &Config,
    w: &World,
    i: usize,
    epoch: u32,
    stamp: &[u32],
    done: &[bool],
    queued: &mut [u32],
    heap: &mut BinaryHeap<Reverse<(usize, u8, usize)>>,
) {
    let plane = c.width as usize * c.depth as usize;
    for nb in neighbours(c, i).into_iter().flatten() {
        if w.material[nb].is_solid() || stamp[nb] == epoch || done[nb] || queued[nb] == epoch {
            continue;
        }
        queued[nb] = epoch;
        let wet = if w.free[nb] > 0.0 { 0u8 } else { 1u8 };
        heap.push(Reverse((nb / plane, wet, nb)));
    }
}

fn infiltrate(w: &mut World, dt: f64) {
    let c = w.config.clone();
    for z in 0..c.depth {
        for x in 0..c.width as i64 {
            for y in 1..c.height {
                let i = c.index(x, y, z);
                if w.material[i].is_solid() || w.free[i] <= 0.0 {
                    continue;
                }
                let below = c.index(x, y - 1, z);
                let m = w.material[below];
                if m.pore_capacity() <= 0.0 || m.permeability_per_s() <= 0.0 {
                    continue;
                }
                let rate = m.permeability_per_s() * dt * m.pore_capacity() * c.voxel_volume();
                transfer(w, (i, Store::Free), (below, Store::Pore), rate);
            }
        }
    }
}

fn drain(w: &mut World) {
    let c = w.config.clone();
    // The table as it stands at the start of the step: a voxel inside the saturated
    // zone has nowhere lower to drain to, because the aquifer is what is holding it up.
    let table = c.aquifer_head_m(w.aquifer_m3);
    for z in 0..c.depth {
        for x in 0..c.width as i64 {
            for y in 0..c.height {
                if submerged(&c, y, table) {
                    continue;
                }
                let i = c.index(x, y, z);
                let m = w.material[i];
                let cap = m.pore_capacity();
                if cap <= 0.0 || w.pore[i] <= 0.0 || m.permeability_per_s() <= 0.0 {
                    continue;
                }
                let unit = cap * c.voxel_volume();
                let excess = pore_m3(w, i) - m.field_capacity() * unit;
                if excess <= 0.0 {
                    continue;
                }
                let want = excess.min(m.permeability_per_s() * DT * unit);
                if y == 0 {
                    // Sitting on the foundation: what drains joins the aquifer.
                    let lost = take_pore(w, i, want);
                    w.aquifer_m3 += lost;
                    continue;
                }
                let below = c.index(x, y - 1, z);
                match w.material[below] {
                    Material::Bedrock => {
                        let lost = take_pore(w, i, want);
                        w.aquifer_m3 += lost;
                    }
                    Material::Air => {
                        transfer(w, (i, Store::Pore), (below, Store::Free), want);
                    }
                    _ => {
                        transfer(w, (i, Store::Pore), (below, Store::Pore), want);
                    }
                }
            }
        }
    }
}

fn spring(w: &mut World) {
    let Some((x, y, z)) = w.spring_cell else { return };
    if w.aquifer_m3 <= 0.0 || w.config.spring_k_m2_per_s <= 0.0 {
        return;
    }
    if y >= w.config.height || z >= w.config.depth {
        return;
    }
    let head = w.config.aquifer_head_m(w.aquifer_m3);
    let h_spring = y as f64 * w.config.voxel_m;
    let drive = (head - h_spring).max(0.0);
    if drive <= 0.0 {
        return;
    }
    let want = (w.config.spring_k_m2_per_s * drive * DT).min(w.aquifer_m3);
    // The seep emerges at the named cell and, if that is already brim full, in the
    // cells above it: a spring under standing water still reaches the surface. A solid
    // roof over the seep blocks it, and the aquifer keeps what it could not push out.
    let mut left = want;
    for at in y..w.config.height {
        let i = w.config.index(x as i64, at, z);
        if w.material[i].is_solid() {
            break;
        }
        let got = add_free(w, i, left);
        w.aquifer_m3 -= got;
        left -= got;
        if left <= 1e-15 {
            break;
        }
    }
}

fn outlet(w: &mut World) {
    if !w.outlet_open {
        return;
    }
    let Some((x, y, z)) = w.outlet_cell else { return };
    if y >= w.config.height || z >= w.config.depth {
        return;
    }
    let i = w.config.index(x as i64, y, z);
    let want = w.config.outlet_m3_per_s * DT;
    let lost = take_free(w, i, want);
    w.ledger.outlet_out += lost;
}

// ---------------------------------------------------------------- commands

/// Apply one command, and return the receipt.
///
/// Commands take effect **now**, at the point of the call, whether or not a frontend
/// has the world paused: nothing is queued for the next tick. The receipt is the volume
/// in cubic metres the world actually accepted, signed: positive for water that went in,
/// negative for a `ChargeAquifer` or `WithdrawPore` withdrawal, and capped by what was
/// really there — the room in the cell, the reachable sky-exposed cells, the aquifer's
/// own stock, the pore water in the one voxel. What was asked for and not accepted is
/// the difference between the two, and it is simply refused, never stored elsewhere.
///
/// Non-water commands (`SetMaterial`, `SetOutlet`) always return zero.
///
/// A malformed amount — non-finite, or negative for anything but `ChargeAquifer` — is
/// refused whole: the receipt is zero and nothing at all is booked, so a refused rain
/// pulse or addition never reaches `Ledger::rain_in` or `Ledger::user_in`.
pub fn apply(world: &mut World, command: Command) -> f64 {
    match command {
        Command::RainPulse { volume_m3 } => {
            if !volume_m3.is_finite() || volume_m3 <= 0.0 {
                return 0.0;
            }
            rain_pulse(world, volume_m3)
        }
        Command::AddWater { x, y, z, volume_m3 } => {
            if !volume_m3.is_finite() || volume_m3 < 0.0 {
                return 0.0;
            }
            if y >= world.config.height || z >= world.config.depth {
                return 0.0;
            }
            let i = world.config.index(x, y, z);
            let got = add_free(world, i, volume_m3);
            world.ledger.user_in += got;
            got
        }
        Command::WithdrawPore { x, y, z, volume_m3 } => {
            if !volume_m3.is_finite() || volume_m3 < 0.0 {
                return 0.0;
            }
            if y >= world.config.height || z >= world.config.depth {
                return 0.0;
            }
            let i = world.config.index(x, y, z);
            // Capped by the stock in this one voxel: `take_pore` returns what it moved,
            // which is zero where there is no pore space at all.
            let got = take_pore(world, i, volume_m3);
            world.ledger.transpiration_out += got;
            -got
        }
        Command::SetMaterial { x, y, z, material } => {
            if y >= world.config.height || z >= world.config.depth {
                return 0.0;
            }
            let i = world.config.index(x, y, z);
            let changed = world.material[i] != material;
            set_material(world, i, material);
            if changed {
                world.terrain_version += 1;
            }
            0.0
        }
        Command::ChargeAquifer { volume_m3 } => {
            if !volume_m3.is_finite() {
                return 0.0;
            }
            // A withdrawal can only take what is there.
            let take = volume_m3.max(-world.aquifer_m3);
            world.aquifer_m3 += take;
            world.ledger.user_in += take;
            take
        }
        Command::SetOutlet { open } => {
            world.outlet_open = open;
            0.0
        }
    }
}

/// Spread one pulse over the sky-exposed cells, sharing it evenly and handing what a
/// full cell refuses back to the others.
fn rain_pulse(w: &mut World, volume_m3: f64) -> f64 {
    if volume_m3 <= 0.0 {
        return 0.0;
    }
    let (width, depth) = (w.config.width as i64, w.config.depth);
    let mut targets: Vec<usize> = Vec::new();
    for z in 0..depth {
        for x in 0..width {
            if let Some(i) = sky_cell(w, x, z) {
                targets.push(i);
            }
        }
    }
    if targets.is_empty() {
        return 0.0;
    }
    let mut left = volume_m3;
    let mut credited = 0.0;
    for _ in 0..4 {
        let share = left / targets.len() as f64;
        if share <= 0.0 {
            break;
        }
        let mut taken = 0.0;
        for &i in &targets {
            taken += add_free(w, i, share);
        }
        credited += taken;
        left -= taken;
        if taken <= 0.0 || left <= 1e-15 {
            break;
        }
    }
    w.ledger.rain_in += credited;
    credited
}

/// Replace a material, preserving the water volume the cell held.
///
/// The cell's free and pore water are converted to cubic metres *before* the edit, so
/// nothing carries over as a fraction of the wrong capacity. The new material then keeps
/// what fits it: air keeps free water up to its void, soil and rock keep pore water up
/// to their pore capacity, and a solid turned to air hands its pore water over as free
/// water. What does not fit is displaced to the nearest void with room, and only volume
/// with no reachable room at all is booked as [`crate::Ledger::displaced_out`].
fn set_material(w: &mut World, i: usize, material: Material) -> f64 {
    let water = free_m3(w, i) + pore_m3(w, i);
    w.material[i] = material;
    w.free[i] = 0.0;
    w.pore[i] = 0.0;
    if water <= 0.0 {
        return 0.0;
    }

    let kept = if material.is_solid() { add_pore(w, i, water) } else { add_free(w, i, water) };
    let mut left = water - kept;
    if left > 1e-15 {
        left -= spill_to_nearest_void(w, i, left);
    }
    if left > 1e-12 {
        w.ledger.displaced_out += left;
    }
    water - left.max(0.0)
}

/// Pour `volume_m3` into the void cells nearest `from`, and return what it placed.
///
/// "Nearest" is the wrapped face-adjacent **void** path distance: the search steps only
/// through void cells, so it never tunnels through a solid to reach space behind it, and
/// it wraps in `x` like everything else. Every cell at the same distance gets an equal
/// share; a cell with no room takes nothing and its share goes back to the others at
/// that same distance; only once a whole shell is full does the next shell get anything.
/// Equal shares are the tie rule — nothing here depends on index order.
fn spill_to_nearest_void(w: &mut World, from: usize, volume_m3: f64) -> f64 {
    let c = w.config.clone();
    let mut seen = vec![false; c.cells()];
    seen[from] = true;
    let mut shell: Vec<usize> = Vec::new();
    for nb in neighbours(&c, from).into_iter().flatten() {
        if !seen[nb] && !w.material[nb].is_solid() {
            seen[nb] = true;
            shell.push(nb);
        }
    }

    let mut left = volume_m3;
    let mut placed = 0.0;
    while !shell.is_empty() && left > 1e-15 {
        let mut open = shell.clone();
        while !open.is_empty() && left > 1e-15 {
            let share = left / open.len() as f64;
            let mut taken = 0.0;
            for &i in &open {
                taken += add_free(w, i, share);
            }
            placed += taken;
            left -= taken;
            if taken <= 1e-15 {
                break;
            }
            open.retain(|&i| free_room_m3(w, i) > 1e-15);
        }
        let mut next: Vec<usize> = Vec::new();
        for &i in &shell {
            for nb in neighbours(&c, i).into_iter().flatten() {
                if !seen[nb] && !w.material[nb].is_solid() {
                    seen[nb] = true;
                    next.push(nb);
                }
            }
        }
        shell = next;
    }
    placed
}
