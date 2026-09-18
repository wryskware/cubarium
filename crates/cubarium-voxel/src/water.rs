//! Water: prescribed rain and evaporation, a conservative free-water solver over the
//! void voxels (fall, then a **local head-driven exchange** between neighbouring cells),
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
//!    while that has room, one cell per substep), then *[`exchange`]* (every wet cell
//!    offers water to its four horizontal neighbours and the cell below, driven by the
//!    difference in column head).
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
//! # The local exchange: what moves water sideways
//!
//! There is no region search and no connected-component solve. Each substep, every **wet**
//! cell offers water across its four horizontal faces and to the cell below, and the drive
//! is the difference in **column head**: the surface level `y + free` at the top of the
//! contiguous water column the cell belongs to. A dry cell's head is its own floor, so
//! water runs into an empty neighbour and off a ledge; a deep column's bottom cell carries
//! its whole column's head, so it pushes hard sideways; and a **full** cell carries the
//! highest head that reaches it across its horizontal faces ([`HEAD_PASSES`] local passes
//! per substep), which is how the weight of one column arrives at the foot of another
//! through a flooded passage.
//!
//! A push against a full neighbour is **displaced**, not refused: the water enters the
//! lowest cells with room at or above that neighbour inside its own void run, bottom first
//! ([`offer_up_the_run`]). That is incompressibility, locally, and it is what lifts the far
//! shaft of a U-tube — nothing ever flows upward against its own head, and there is no
//! separate up-push, which would only shuffle a column against `fall`.
//!
//! Three caps make it stable and conservative: a cell's offers are **divided by the number
//! of faces** it offers across (ordinary Jacobi damping — `FLOW_PER_SUBSTEP` is safe for
//! one pair, and a cell with two low neighbours would otherwise hand each of them half its
//! head and empty itself), then **scaled to what it holds**, and then every proposal into
//! one destination is **scaled by that destination's room**. Both ends of every proposal
//! are read from the water as it stood at the start of the substep and the whole of it is
//! applied at the end, so no cell's result depends on the order the active set was walked
//! in, and the phase is parallelisable by construction.
//!
//! Only the wet cells and the columns around them are ever looked at: the active sets in
//! [`crate::World`] are maintained by the store primitives, so dry rock and dry air cost
//! nothing at all (`design/7_Research/voxel-tick-profile-2026-09-18.md`).
//!
//! ## What this model is, and what the old one was
//!
//! The old solver grew every connected water region and set it to one surface level in a
//! single substep. It was exact and it was 45 % of the tick. This one relaxes toward the
//! same answer at half a head difference per substep per face: a U-tube levels in about a
//! second, a ten-column roofed passage in a few, and the answers the fixtures in
//! `tests/core.rs` pin — the spill thresholds, the mirrored shelf, the symmetric spill,
//! the U-tube's own levels — come out the same. A **closed, surcharged** passage is the one
//! case that does not settle flat: a local rule with no pressure solve leaves the surface
//! above a flooded roof uneven by a few tenths of a cell, which
//! `a_roofed_passage_pushes_the_far_shaft_above_the_roof` states rather than asserts away.
//!
//! Both numbers in it — `FLOW_PER_SUBSTEP` and `HEAD_PASSES` — are **placeholders**
//! (`design/backlog.md`), and so is `water_substeps` 4, which is unchanged.
//!
//! # Limits, plainly
//!
//! - **No inertia.** Water has no momentum; nothing sloshes, overshoots or oscillates.
//! - **No current.** There is no velocity field: a flux is a head difference across one
//!   face and nothing remembers it. A waterfall is a column of cells that each hand their
//!   water down one cell per substep, not a jet.
//! - **Settling travels.** A lake 100 cells wide no longer levels in one substep: head
//!   crosses `HEAD_PASSES` cells per substep and a face moves `FLOW_PER_SUBSTEP` of its own
//!   difference, so a wide body relaxes over ticks. `Config::free_transfer_cap` above zero
//!   caps what one face may pass and slows it further without changing where the water ends
//!   up.
//! - **A surcharged closed passage does not settle flat.** See above: the one case where
//!   the local rule visibly differs from the old region solver.
//! - **Water above the level is not carried down at once.** A film running down a slope
//!   descends a cell per substep.
//! - **`f64` stores.** `free` and `pore` are `f64` fractions, so an internal transfer
//!   debits its source exactly what it credited its destination. There is no
//!   quantization term: the ledger residual `stored - initial_stored - net_in` is the
//!   raw conservation error and nothing corrects it.

use std::cell::RefCell;

use crate::{Command, Config, Material, World, DT};

/// The fraction of a head difference that crosses one face in one substep.
///
/// **Placeholder** (`design/backlog.md`): 0.5 is the largest coefficient that cannot make
/// a pair overshoot — moving half of a difference leaves both ends level — and nothing
/// measured it. It is the only number in the local exchange.
const FLOW_PER_SUBSTEP: f64 = 0.5;

/// A cell with less than this much room left counts as full, so a float hair of room
/// cannot make the displacement target a cell that cannot actually take anything.
const ROOM_EPS: f64 = 1e-12;

/// How many local passes carry head through **submerged** water in one substep: how far
/// pressure travels sideways before anything moves.
///
/// **Placeholder** (`design/backlog.md`). Four passes per substep and four substeps means
/// pressure reaches sixteen cells a tick, so a wide lake levels over a few ticks instead of
/// instantly, and a U-tube whose connection is two cells long feels the far column's weight
/// on the first substep. It is the one number that replaces the old region search.
const HEAD_PASSES: u32 = 4;

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
    // The active set is maintained here because this is one of the two places free water
    // is written: a phase that iterates the wet set can only be right if the primitives
    // keep it right.
    w.wet.set(i, w.free[i] > 0.0);
    (w.free[i] - before).max(0.0) * v
}

fn take_free(w: &mut World, i: usize, vol: f64) -> f64 {
    if vol <= 0.0 || w.material[i].is_solid() {
        return 0.0;
    }
    let v = voxel(w);
    let before = w.free[i];
    w.free[i] = (before - vol / v).max(0.0);
    w.wet.set(i, w.free[i] > 0.0);
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
    w.damp.set(i, w.pore[i] > 0.0);
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
    w.damp.set(i, w.pore[i] > 0.0);
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
    // The active sets are a cache of the arrays; a fresh, decoded or resized world has to
    // rebuild them before any phase may iterate them.
    if world.wet.needs_rebuild(world.config.cells())
        || world.damp.needs_rebuild(world.config.cells())
    {
        world.rebuild_active_sets();
    }
    crate::voxel_phase!(WorldStep, {
        crate::voxel_phase!(Rain, { rain(world) });
        crate::voxel_phase!(Evaporate, { evaporate(world) });
        let substeps = world.config.water_substeps.max(1);
        let sub_dt = DT / substeps as f64;
        crate::voxel_phase!(Substeps, {
            for _ in 0..substeps {
                // Infiltration first, and inside the substep: water standing on a permeable
                // cell is offered to it before anything moves the water somewhere else. See
                // the module doc on why runoff is what infiltration refuses.
                crate::voxel_phase!(Infiltrate, { infiltrate(world, sub_dt) });
                crate::voxel_phase!(Fall, { fall(world) });
                crate::voxel_phase!(Exchange, { exchange(world) });
            }
        });
        crate::voxel_phase!(Drain, { drain(world) });
        crate::voxel_phase!(WaterTable, { water_table(world) });
        crate::voxel_phase!(Spring, { spring(world) });
        crate::voxel_phase!(Outlet, { outlet(world) });
    });
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
///
/// Iterated over the **wet columns** and not the grid: a column is walked bottom-up the
/// moment one of its wet cells is met, so the rule — and its bottom-up order, which is
/// what makes a whole column shift down by one rather than only its floor cell — is
/// exactly what it was, over 2e3 columns instead of 147e3 cells
/// (`design/7_Research/voxel-tick-profile-2026-09-18.md`).
fn fall(w: &mut World) {
    let c = w.config.clone();
    let plane = c.width as usize * c.depth as usize;
    let columns = wet_columns(w, plane);
    #[cfg(feature = "profile")]
    crate::profile::add(
        crate::profile::Count::FallCells,
        columns.len() as u64 * u64::from(c.height.saturating_sub(1)),
    );
    for col in columns {
        for y in 1..c.height {
            let i = y as usize * plane + col;
            if w.free[i] <= 0.0 || w.material[i].is_solid() {
                continue;
            }
            let below = i - plane;
            let want = free_m3(w, i).min(free_room_m3(w, below));
            if want > 0.0 {
                transfer(w, (i, Store::Free), (below, Store::Free), want);
            }
        }
    }
}

/// The columns holding free water, deduplicated, in the order the wet set holds them.
/// Collected into a fresh `Vec` because the caller then writes the water arrays; the
/// dedup array itself is the reused scratch.
fn wet_columns(w: &World, plane: usize) -> Vec<usize> {
    SCRATCH.with(|slot| {
        let sc = &mut *slot.borrow_mut();
        sc.ensure(w.config.cells(), plane);
        sc.next_stamp();
        let stamp = sc.stamp;
        let mut out = Vec::with_capacity(sc.columns.len().max(64));
        for &i in w.wet.cells() {
            let col = i % plane;
            if sc.col_stamp[col] != stamp {
                sc.col_stamp[col] = stamp;
                out.push(col);
            }
        }
        out
    })
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
    // The saturated **band** and not the grid: `submerged` is monotone in `y`, so every
    // row above the table's own row can be skipped without looking at it at all
    // (`design/7_Research/voxel-tick-profile-2026-09-18.md`). Same rows, same rule.
    let band = {
        let rows = (table / c.voxel_m - 0.5).floor() + 1.0;
        if rows <= 0.0 { 0 } else { (rows as u32).min(c.height) }
    };
    #[cfg(feature = "profile")]
    crate::profile::add(
        crate::profile::Count::WaterTableCells,
        u64::from(c.depth) * u64::from(c.width) * u64::from(band),
    );
    // What the aquifer gives up this step, accumulated and debited once. Subtracting a
    // microlitre from a store of tens of cubic metres forty thousand times a tick is
    // forty thousand roundings against the store's own ulp; one subtraction is one.
    let mut taken = 0.0;
    let charged = w.aquifer_m3;

    for y in 0..band {
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

/// Scratch buffers for [`exchange`], reused for the life of the thread so the phase
/// allocates nothing: four substeps a tick used to allocate and zero eight grid-sized
/// vectors each (`design/7_Research/voxel-tick-profile-2026-09-18.md`). Every entry is
/// written before it is read inside one call, so nothing here is state.
#[derive(Default)]
struct Scratch {
    /// Surface level of the contiguous water column a wet cell belongs to, in cell units
    /// (`y + free` of the run's top cell). Valid for the cells of this substep's active
    /// columns.
    head: Vec<f64>,
    /// For every non-solid cell of an active column: the lowest cell **at or above** it,
    /// inside its own void run, that still has room — where a push against this cell
    /// actually displaces water to. `usize::MAX` when the run is full to its ceiling.
    room_target: Vec<usize>,
    /// The top cell of each non-solid cell's own void run: where a displacement stops.
    run_top: Vec<usize>,
    /// One cell's offers this pass, `(destination, volume)`, before the giver's own stock
    /// scales them.
    offers: Vec<(usize, f64)>,
    /// The **driving** head: a cell's own surface head, raised to the highest head that
    /// reaches it through submerged water. A giver pushes with its `drive`; a receiver
    /// resists with its own `head`, because what a neighbour presents to the water arriving
    /// is its surface level and not the pressure passing through it.
    drive: Vec<f64>,
    /// The next `drive` while the current one is read: the propagation is Jacobi, two
    /// buffers, so it does not depend on the order the set is walked in.
    drive_next: Vec<f64>,
    /// The wet cells at the start of the substep, copied so the sets may be edited while
    /// the flux is applied.
    active: Vec<usize>,
    /// The **full** cells of the active set: the submerged ones, which are the only cells
    /// that carry another column's head and the only ones that can push straight up.
    full: Vec<usize>,
    /// Columns holding or neighbouring water, deduplicated by `col_stamp`.
    columns: Vec<usize>,
    col_stamp: Vec<u32>,
    /// Cells named by an edge, deduplicated by `touch_stamp`.
    touched: Vec<usize>,
    touch_stamp: Vec<u32>,
    stamp: u32,
    /// The proposals: from, to, and the volume in cell units.
    edges: Vec<(u32, u32, f64)>,
    /// Per-cell accumulators: what was proposed into a cell, the fraction of it the cell
    /// accepts, and the net change applied at the end.
    proposed_in: Vec<f64>,
    accept: Vec<f64>,
    delta: Vec<f64>,
}

impl Scratch {
    fn ensure(&mut self, n: usize, plane: usize) {
        if self.head.len() != n {
            self.head = vec![0.0; n];
            self.drive = vec![0.0; n];
            self.drive_next = vec![0.0; n];
            self.room_target = vec![usize::MAX; n];
            self.run_top = vec![usize::MAX; n];
            self.touch_stamp = vec![0; n];
            self.proposed_in = vec![0.0; n];
            self.accept = vec![0.0; n];
            self.delta = vec![0.0; n];
            self.stamp = 0;
        }
        if self.col_stamp.len() != plane {
            self.col_stamp = vec![0; plane];
            self.stamp = 0;
        }
    }

    /// A fresh stamp for this substep's dedup arrays, clearing them on the wrap.
    fn next_stamp(&mut self) {
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.col_stamp.fill(0);
            self.touch_stamp.fill(0);
            self.stamp = 1;
        }
    }
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

/// **The local exchange: what replaced the region solver.**
///
/// Each substep every wet cell offers water to its four horizontal neighbours and to the
/// cell below, driven by the difference in **column head** — the surface level of the
/// contiguous water column the cell belongs to, `y + free` at the run's top. That one
/// definition is what carries pressure without any connectivity search: the bottom cell of
/// a deep column has its whole column's head, so it pushes hard sideways; a full cell in a
/// submerged gap has the head of the body it is part of and passes the push along; and a
/// dry cell's head is its own floor, so water runs into an empty neighbour and off a ledge.
///
/// A push against a **full** neighbour is not refused, it is **displaced**: the water
/// enters the lowest cell with room at or above that neighbour, inside the neighbour's own
/// void run (`Scratch::room_target`). That is incompressibility, locally — pushing at the
/// bottom of a submerged column lifts its surface — and it is what makes a U-tube level and
/// a roofed passage push the far shaft up without anything ever flowing upward against its
/// own head.
///
/// The flux across one face is `FLOW_PER_SUBSTEP * (head_here - head_there)`, capped three
/// ways: by the room at the destination, by what the giver holds (its proposals are scaled
/// down together if they ask for more), and by the destination's total acceptance (every
/// proposal into one cell is scaled by the same factor when they overfill it together).
/// Both ends of every proposal are read from the **old** water and the whole of it is
/// applied at the end, so no cell's result depends on the order the set was walked in and
/// the phase is parallelisable by construction.
///
/// **What this is not.** There is no momentum, no velocity field and no free surface: a
/// lake no longer levels in one substep, it relaxes at half of its head difference per
/// substep, which is a travelling wave rather than an instant re-level. `FLOW_PER_SUBSTEP`
/// is a placeholder and the only number in the rule.
fn exchange(w: &mut World) {
    let c = w.config.clone();
    let plane = c.width as usize * c.depth as usize;
    let n = c.cells();
    if w.wet.len() == 0 {
        return;
    }
    SCRATCH.with(|slot| {
        let sc = &mut *slot.borrow_mut();
        sc.ensure(n, plane);
        sc.next_stamp();
        let stamp = sc.stamp;
        if std::env::var("WATER_DEBUG").is_ok() {
            let mut v: Vec<(u32,u32,u32)> = w.wet.cells().iter().map(|&i| c.coords(i)).collect();
            v.sort();
            eprintln!("wet: {v:?}");
        }

        // ---- the active set: the wet cells, and the columns they and their horizontal
        // neighbours live in. Dry rock and dry air are never looked at.
        sc.active.clear();
        sc.active.extend_from_slice(w.wet.cells());
        sc.columns.clear();
        for &i in &sc.active {
            let (x, _, z) = c.coords(i);
            let x = x as i64;
            for (dx, dz) in [(0i64, 0i64), (-1, 0), (1, 0), (0, -1), (0, 1)] {
                let nz = z as i64 + dz;
                if nz < 0 || nz >= c.depth as i64 {
                    continue;
                }
                let col = c.index(x + dx, 0, nz as u32);
                if sc.col_stamp[col] != stamp {
                    sc.col_stamp[col] = stamp;
                    sc.columns.push(col);
                }
            }
        }
        #[cfg(feature = "profile")]
        {
            crate::profile::add(crate::profile::Count::ExchangeWet, sc.active.len() as u64);
            crate::profile::add(crate::profile::Count::ExchangeColumns, sc.columns.len() as u64);
        }

        // ---- one pass per active column: heads, and where a push displaces to.
        for k in 0..sc.columns.len() {
            let col = sc.columns[k];
            scan_column(w, &c, plane, col, sc);
        }

        // ---- head through submerged water. A full cell has no free surface of its own,
        // so it carries the highest head that reaches it across its horizontal faces: that
        // is how the weight of one column arrives at the foot of another through a flooded
        // passage, which is the whole of the U-tube and the roofed gap. The **driving**
        // head is what a giver pushes with; a receiver still resists with its own surface
        // head, or a push would cancel itself against the pressure it just transmitted.
        //
        // `max` only raises, so the passes converge; two buffers keep the answer off the
        // walk order, and the loop stops as soon as a pass raises nothing.
        for ai in 0..sc.active.len() {
            let i = sc.active[ai];
            sc.drive[i] = sc.head[i];
        }
        sc.full.clear();
        for ai in 0..sc.active.len() {
            let i = sc.active[ai];
            if w.free[i] >= 1.0 - ROOM_EPS {
                sc.full.push(i);
            }
        }
        for _ in 0..HEAD_PASSES {
            let mut raised = false;
            for fi in 0..sc.full.len() {
                let i = sc.full[fi];
                let (x, y, z) = c.coords(i);
                let x = x as i64;
                let mut h = sc.drive[i];
                for (dx, dz) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                    let nz = z as i64 + dz;
                    if nz < 0 || nz >= c.depth as i64 {
                        continue;
                    }
                    let j = c.index(x + dx, y, nz as u32);
                    if w.material[j].is_solid() || w.free[j] <= 0.0 {
                        continue;
                    }
                    if sc.drive[j] > h {
                        h = sc.drive[j];
                    }
                }
                sc.drive_next[i] = h;
            }
            for fi in 0..sc.full.len() {
                let i = sc.full[fi];
                if sc.drive_next[i] > sc.drive[i] {
                    sc.drive[i] = sc.drive_next[i];
                    raised = true;
                }
            }
            if !raised {
                break;
            }
        }

        // ---- the proposals, all read from the old water.
        sc.edges.clear();
        sc.touched.clear();
        for ai in 0..sc.active.len() {
            let i = sc.active[ai];
            let have = w.free[i];
            if have <= 0.0 {
                continue;
            }
            let (x, y, z) = c.coords(i);
            let x = x as i64;
            let here = sc.drive[i];
            sc.offers.clear();
            let mut total = 0.0;
            // How many faces this cell is offering across. Its offers are divided by that
            // count below: `FLOW_PER_SUBSTEP` is safe for **one** pair — half a difference
            // levels a pair exactly — and a cell with two low neighbours would otherwise
            // hand each of them half its head and empty itself, which flip-flops instead
            // of relaxing. This is the ordinary Jacobi damping and it is what makes the
            // exchange settle rather than ring.
            let mut faces = 0usize;

            // The four horizontal faces. A push against a **full** neighbour is not
            // refused, it is displaced: the water goes into the lowest cells with room at
            // or above that neighbour, inside the neighbour's own void run, bottom first.
            // That is what lifts a submerged column's surface instead of throttling the
            // flow to the hair of room its floor cell has left.
            for (dx, dz) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                let nz = z as i64 + dz;
                if nz < 0 || nz >= c.depth as i64 {
                    continue;
                }
                let j = c.index(x + dx, y, nz as u32);
                if w.material[j].is_solid() {
                    continue;
                }
                let there = if w.free[j] > 0.0 { sc.head[j] } else { (j / plane) as f64 };
                let drop = here - there;
                if drop <= 0.0 {
                    continue;
                }
                let placed =
                    offer_up_the_run(w, plane, j, cap_flux(&c, FLOW_PER_SUBSTEP * drop), sc);
                if placed > 0.0 {
                    faces += 1;
                    total += placed;
                }
            }

            // And straight down, into the cell's own room below: the same rule, and the
            // reason a film keeps moving when `fall` has already taken what it can.
            if y > 0 {
                let below = c.index(x, y - 1, z);
                if !w.material[below].is_solid() {
                    let room = (1.0 - w.free[below]).max(0.0);
                    let there = if w.free[below] > 0.0 {
                        sc.head[below]
                    } else {
                        (below / plane) as f64
                    };
                    let drop = here - there;
                    if drop > 0.0 && room > 0.0 {
                        let q = cap_flux(&c, FLOW_PER_SUBSTEP * drop).min(room);
                        if q > 0.0 {
                            sc.offers.push((below, q));
                            faces += 1;
                            total += q;
                        }
                    }
                }
            }

            // **There is no separate upward push**, and that is a rule and not an
            // omission: a column rises because water *arriving* at it is displaced up its
            // own run (`offer_up_the_run`), which is what lifts the far shaft of a U-tube.
            // Lifting a cell's own water into the cell above it instead only shuffles a
            // column against `fall` — the two fight, and a roofed passage never fills.

            if sc.offers.is_empty() || total <= 0.0 {
                continue;
            }
            // A cell cannot give more than it holds, and no one face may drain it: its
            // offers are divided by the faces it offers across and then scaled together if
            // they still ask for more than it has, so no face is ever preferred.
            let share = 1.0 / faces.max(1) as f64;
            let total = total * share;
            let scale = share * if total > have { have / total } else { 1.0 };
            for oi in 0..sc.offers.len() {
                let (target, q) = sc.offers[oi];
                let q = q * scale;
                if q <= 0.0 {
                    continue;
                }
                sc.edges.push((i as u32, target as u32, q));
                sc.proposed_in[target] += q;
                for cell in [i, target] {
                    if sc.touch_stamp[cell] != stamp {
                        sc.touch_stamp[cell] = stamp;
                        sc.touched.push(cell);
                    }
                }
            }
        }

        // ---- what each destination can actually take, whoever offered it.
        for ti in 0..sc.touched.len() {
            let cell = sc.touched[ti];
            let proposed = sc.proposed_in[cell];
            let room = (1.0 - w.free[cell]).max(0.0);
            sc.accept[cell] = if proposed > room && proposed > 0.0 { room / proposed } else { 1.0 };
        }

        // ---- apply: one subtraction and one addition per edge, accumulated per cell so
        // that a cell touched by several edges is written once.
        for ei in 0..sc.edges.len() {
            let (from, to, q) = sc.edges[ei];
            let moved = q * sc.accept[to as usize];
            if moved <= 0.0 {
                continue;
            }
            sc.delta[from as usize] -= moved;
            sc.delta[to as usize] += moved;
        }
        for ti in 0..sc.touched.len() {
            let cell = sc.touched[ti];
            let d = sc.delta[cell];
            sc.delta[cell] = 0.0;
            sc.proposed_in[cell] = 0.0;
            sc.accept[cell] = 0.0;
            if d != 0.0 {
                w.free[cell] = (w.free[cell] + d).clamp(0.0, 1.0);
                w.wet.set(cell, w.free[cell] > 0.0);
            }
        }
    });
}

/// Place `q` into the cells with room at or above `j`, inside `j`'s own void run, bottom
/// first, recording each part as an offer. Returns how much of `q` actually found room.
///
/// This is the displacement rule: pushing against a submerged cell fills the stack above
/// it rather than stopping at whatever room that one cell has left. Without it a nearly
/// full column throttles the flow into it to its own remaining hair of room, and a U-tube
/// crawls toward its level geometrically instead of reaching it.
fn offer_up_the_run(w: &World, plane: usize, j: usize, q: f64, sc: &mut Scratch) -> f64 {
    let mut left = q;
    let mut at = sc.room_target[j];
    let top = sc.run_top[j];
    if at == usize::MAX || top == usize::MAX {
        return 0.0;
    }
    let mut placed = 0.0;
    while left > 0.0 && at <= top {
        let room = (1.0 - w.free[at]).max(0.0);
        if room > 0.0 {
            let take = left.min(room);
            sc.offers.push((at, take));
            placed += take;
            left -= take;
        }
        at += plane;
    }
    placed
}

/// `free_transfer_cap`, when the config sets one: the most one face may move in one
/// substep, in cell units. Zero leaves the flux uncapped, which is the default.
#[inline]
fn cap_flux(c: &Config, q: f64) -> f64 {
    if c.free_transfer_cap > 0.0 { q.min(c.free_transfer_cap) } else { q }
}

/// One pass over a column: the head of every wet cell, and the displacement target of
/// every non-solid cell.
///
/// A **void run** is a maximal stack of non-solid cells; the displacement target of a cell
/// is the lowest cell with room at or above it *within its own run*, because a solid
/// ceiling is where a push stops. A **water run** is a maximal stack of wet cells inside a
/// void run, and every cell of it carries the run's own surface level.
fn scan_column(w: &World, c: &Config, plane: usize, col: usize, sc: &mut Scratch) {
    let height = c.height as usize;
    let mut y = 0usize;
    while y < height {
        if w.material[y * plane + col].is_solid() {
            y += 1;
            continue;
        }
        let mut top = y;
        while top + 1 < height && !w.material[(top + 1) * plane + col].is_solid() {
            top += 1;
        }
        // Displacement targets, from the ceiling down: the lowest cell with room seen so
        // far is the lowest cell with room at or above the cell being written.
        let mut best = usize::MAX;
        let top_cell = top * plane + col;
        for k in (y..=top).rev() {
            let i = k * plane + col;
            if w.free[i] < 1.0 - ROOM_EPS {
                best = i;
            }
            sc.room_target[i] = best;
            sc.run_top[i] = top_cell;
        }
        // Heads, one water run at a time.
        let mut k = y;
        while k <= top {
            let i = k * plane + col;
            if w.free[i] > 0.0 {
                let mut t = k;
                while t + 1 <= top && w.free[(t + 1) * plane + col] > 0.0 {
                    t += 1;
                }
                let surface = t as f64 + w.free[t * plane + col];
                for m in k..=t {
                    sc.head[m * plane + col] = surface;
                }
                k = t + 1;
            } else {
                k += 1;
            }
        }
        y = top + 1;
    }
}

fn infiltrate(w: &mut World, dt: f64) {
    let c = w.config.clone();
    let plane = c.width as usize * c.depth as usize;
    // Over the wet set: a cell with no free water in it has nothing to offer the ground,
    // and source and destination are disjoint (a void cell and the porous cell under it),
    // so the order the set is walked in cannot change the answer.
    let active: Vec<usize> = w.wet.cells().to_vec();
    #[cfg(feature = "profile")]
    crate::profile::add(crate::profile::Count::InfiltrateCells, active.len() as u64);
    for i in active {
        if i < plane || w.material[i].is_solid() || w.free[i] <= 0.0 {
            continue;
        }
        let below = i - plane;
        let m = w.material[below];
        if m.pore_capacity() <= 0.0 || m.permeability_per_s() <= 0.0 {
            continue;
        }
        let rate = m.permeability_per_s() * dt * m.pore_capacity() * c.voxel_volume();
        transfer(w, (i, Store::Free), (below, Store::Pore), rate);
    }
}

fn drain(w: &mut World) {
    let c = w.config.clone();
    let plane = c.width as usize * c.depth as usize;
    // The table as it stands at the start of the step: a voxel inside the saturated
    // zone has nowhere lower to drain to, because the aquifer is what is holding it up.
    let table = c.aquifer_head_m(w.aquifer_m3);
    // Over the damp set — the cells that hold any pore water — instead of the grid. Two
    // stated consequences (`design/7_Research/voxel-tick-profile-2026-09-18.md`): a cell
    // with no pore water can never drain, so nothing is lost; and the walk is no longer
    // bottom-up, so a stack of wet soil can pass water down more than one cell in a tick
    // where the grid walk passed it exactly one. It is a trickle either way — one tick's
    // drainage is `permeability_per_s * DT` of a cell's capacity — and the soil profile is
    // a statistical claim, not a per-cell one.
    let active: Vec<usize> = w.damp.cells().to_vec();
    #[cfg(feature = "profile")]
    crate::profile::add(crate::profile::Count::DrainCells, active.len() as u64);
    for i in active {
        let y = (i / plane) as u32;
        if submerged(&c, y, table) {
            continue;
        }
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
        let below = i - plane;
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
    // The one place outside the store primitives that writes the arrays, so the one place
    // that has to keep the active sets honest itself: an edited cell holds nothing until
    // the material below decides what it can keep.
    w.wet.remove(i);
    w.damp.remove(i);
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
