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
//!    setting the rate at its own `conductivity_m_per_s` across the cell face,
//!    `K * cell_area * sub_dt`), then *fall* (every void cell hands its water to the void cell below
//!    while that has room, one cell per substep), then *[`exchange`]* (every wet cell
//!    offers water to its four horizontal neighbours and the cell below, driven by the
//!    difference in column head).
//! 4. **Drainage**: pore water above the cell's own `field_capacity` moves down — into
//!    the pore space below, into the aquifer where a porous cell sits on bedrock or on
//!    the foundation, or as a drip into free water where a porous cell roofs a void. A
//!    voxel inside the saturated zone is skipped: it has nowhere lower to go. The
//!    **donor** cell's material sets both the threshold and the rate (its own
//!    `conductivity_m_per_s` across the cell face). This is field-capacity
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
//! is the difference in **column head**: the surface level `s + free[s]` of the packed
//! full stack the cell sits at the foot of, `s` being the first row at or above it that is
//! not full — a partial cell's own level, a full cell's the partial cell capping its stack
//! (package H; until then it was the top of the whole wet run, packed or not). A dry
//! cell's head is its own floor, so water runs into an empty neighbour and off a ledge; a
//! deep column's bottom cell carries its whole column's head, so it pushes hard sideways;
//! and a **full** cell carries the
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
//! the U-tube's own levels — come out the same. A **closed, surcharged** passage used to be
//! the one case that did not settle flat: two columns facing each other over several full
//! rows each sent half their head difference per row, so they swapped levels every substep
//! and the surface above a flooded roof stayed uneven by a few tenths of a cell. Package P
//! caps what a stack sends across one face at one row's worth, and that fixture now levels
//! (`a_roofed_passage_pushes_the_far_shaft_above_the_roof`).
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
//! - **Still water settles by relaxation, not at once.** Two columns side by side level in
//!   one substep; a pool six columns wide agrees to the last bit after a few hundred
//!   (package P). There is no pressure solve: head reaches `HEAD_PASSES` cells a substep.
//! - **Water above the level is not carried down at once.** A film running down a slope
//!   descends a cell per substep.
//! - **`f64` stores.** `free` and `pore` are `f64` fractions, so an internal transfer
//!   debits its source exactly what it credited its destination. There is no
//!   quantization term: the ledger residual `stored - initial_stored - net_in` is the
//!   raw conservation error and nothing corrects it.

use std::cell::RefCell;

use crate::material::REFERENCE_VOXEL_M;
use crate::world::{MASK_ROWS, VoidRun, run_bits};
use crate::{Command, Config, DT, Material, World};

/// The fraction of a head difference that crosses one face in one substep.
///
/// **Placeholder** (`design/backlog.md`): 0.5 is the largest coefficient that cannot make
/// a pair overshoot — moving half of a difference leaves both ends level — and nothing
/// measured it. It is the only number in the local exchange.
const FLOW_PER_SUBSTEP: f64 = 0.5;

/// **Minimum spreading depth** (package C, agreed by Wrysk 2026-09-22): free water
/// shallower than this — metres of depth in its own cell, so `free < MIN_SPREAD_DEPTH_M /
/// voxel_m` — makes no **horizontal** offers in the exchange. It stays where it is and still
/// infiltrates, evaporates, falls and offers downward; nothing is deleted. It is the
/// shallow-water solvers' wet/dry threshold. The test is on the giver's **own** `free`,
/// never its head or drive, so the thin surface cell of a lake does not stop the full
/// cells under it from pushing.
///
/// Without it every film spreads into every empty neighbour on its row, because an empty
/// cell's head is its own floor, and rain multiplies near-empty wet cells.
///
/// **Placeholder** (`design/backlog.md`). 0.125 µm, a millionth of a 0.125 m cell: the
/// smallest decade past which the shower stops getting steeply cheaper on the desktop
/// world (census, 2026-09-22 — proposals per tick 303 k with no threshold, 171 k here,
/// 135 k and 125 k at ten and a hundred times it). The next decade is also where the
/// shipped `small` landform's spring-route fixture fails, because what it counts at the
/// spring is invisible spray; see the package C commit.
const MIN_SPREAD_DEPTH_M: f64 = 1.25e-7;

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

/// The volume one cell face passes in `dt` at the material's hydraulic conductivity:
/// the solver's one transport rate (package 1c,
/// `design/handoffs/voxel-water-units-2026-09-22.md`).
///
/// **The rule.** A flux between two cells is a conductivity in metres per second times
/// the shared face area in m² times the timestep, `K · A · dt`, with `K =
/// Material::conductivity_m_per_s()`. What stood here before was `permeability_per_s ·
/// dt · pore_capacity · voxel_volume` — a *fraction of a cell* per tick, so the physical
/// flux was `permeability · pore_capacity · voxel_m` metres per second and halved with
/// the cell: on the panel's 0.125 m ring the ground drained at half the speed the 0.25 m
/// ring's did, and the first shower stood 0.03–0.08 m deep and drowned half the stands
/// (D5, `design/handoffs/voxel-small-collapse-2026-09-22.md`).
///
/// **Why the factors are in this order.** Mathematically this is `K · A · dt`. It is
/// written as `(permeability · dt) · pore_capacity · (A · REFERENCE_VOXEL_M)` because on
/// the 0.25 m reference grid `A · 0.25` *is* `voxel_volume()` exactly — both are powers
/// of two times the cell, so no rounding enters — and the whole expression then
/// reproduces the pre-1c one bit for bit rather than to within an ulp. Reassociating
/// `permeability · dt · pore_capacity` into `(permeability · pore_capacity) · dt` is what
/// costs that last place; the reference grid is meant to be untouched, so it is not paid.
/// `units_tests` pins both halves: bit identity against the old expression on 0.25 m, and
/// agreement with `K · A · dt` to within one ulp on both grids.
///
/// Storage terms — `pore_room_m3`, the `room` a seepage target has, a cell's `unit` of
/// pore capacity — stay volumes and are **not** this. Rain and evaporation were already
/// per area (`rain_m_per_s · DT · cell_area()`) and are untouched.
#[inline]
fn pore_flux_m3(m: Material, c: &Config, dt: f64) -> f64 {
    m.permeability_per_s() * dt * m.pore_capacity() * (c.cell_area() * REFERENCE_VOXEL_M)
}

fn free_m3(w: &World, i: usize) -> f64 {
    if w.material[i].is_solid() {
        0.0
    } else {
        w.free[i] * voxel(w)
    }
}

fn free_room_m3(w: &World, i: usize) -> f64 {
    if w.material[i].is_solid() {
        0.0
    } else {
        (1.0 - w.free[i]).max(0.0) * voxel(w)
    }
}

fn pore_m3(w: &World, i: usize) -> f64 {
    w.pore[i] * voxel(w) * w.material[i].pore_capacity()
}

fn pore_room_m3(w: &World, i: usize) -> f64 {
    let cap = w.material[i].pore_capacity();
    if cap <= 0.0 {
        0.0
    } else {
        (1.0 - w.pore[i]).max(0.0) * voxel(w) * cap
    }
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

/// Hand water a loss phase took out of the in-world stores to wherever this world's
/// budget sends it, and book the deposit.
///
/// Open budget: nowhere — it leaves, exactly as it always did. Closed budget: into the
/// lumped atmosphere, which is why `evaporation_out`, `transpiration_out` and
/// `outlet_out` read as flows *into* the store there. The phase books its own `*_out`
/// term either way, so [`crate::Ledger::expected_stored`] is right under both.
///
/// The outlet's export goes here too rather than into a second reserve behind the
/// spring. Reasons, in order: one store means one threshold, one residual line and one
/// thing for route C to replace; the spring is already fed by the aquifer, which
/// infiltration and the water table recharge from above, so a reserve would need its own
/// recharge rule and its own head; and the outlet is the only path that empties standing
/// water, so returning it to the sky is what keeps a full basin's water in the cycle
/// instead of piping it underground where the surface can never see it again.
fn release(w: &mut World, volume: f64) {
    if !w.config.closed_water_budget || !(volume > 0.0) {
        return;
    }
    w.atmosphere_m3 += volume;
    w.ledger.atmosphere_in += volume;
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

/// Per column, the highest **wet** row inside the run the sky reaches down into
/// ([`crate::world::VoidRuns::sky_floor`]), or `u32::MAX` where that run is dry or there
/// is none.
///
/// This is what rain and evaporation used to find by walking every column down from the
/// ceiling each tick, one cache line per row (2.0 ms a tick on the board, package PA).
/// The sky floor moves only with the terrain and is cached with the void runs; the water
/// part is read off the wet set, so the cost is the wet cells plus one entry per column.
/// Above the highest wet cell of the sky's run there is only dry air, which is exactly
/// what the walk stepped over.
fn sky_tops(w: &mut World) -> Vec<u32> {
    #[cfg(feature = "profile")]
    let _timer = crate::profile::start(crate::profile::Phase::SkyTops);
    // The wet set is a cache and may be stale between ticks (a decoded world, a command
    // before the first step); this reads it, so it must be true first.
    begin(w);
    w.ensure_void_runs();
    let plane = w.config.width as usize * w.config.depth as usize;
    let floor = &w.void_runs.sky_floor;
    let mut top = vec![u32::MAX; plane];
    for &i in w.wet.cells() {
        let y = i / plane;
        let col = i - y * plane;
        let y = y as u32;
        if y >= floor[col] && (top[col] == u32::MAX || y > top[col]) {
            top[col] = y;
        }
    }
    top
}

/// The void cell of column `col` that sees the sky: the first cell above whatever the sky
/// meets looking down — ground, or a brim-full cell — or the highest wet cell itself while
/// it still has room. `None` when the column is packed to the ceiling. `top` is that
/// column's entry of [`sky_tops`].
fn sky_cell_at(w: &World, col: usize, top: u32) -> Option<usize> {
    let plane = w.config.width as usize * w.config.depth as usize;
    let height = w.config.height as usize;
    if top == u32::MAX {
        let floor = w.void_runs.sky_floor[col] as usize;
        return (floor < height).then_some(floor * plane + col);
    }
    let i = top as usize * plane + col;
    if w.free[i] >= 1.0 {
        (top as usize + 1 < height).then_some(i + plane)
    } else {
        Some(i)
    }
}

/// The sky-exposed free-water surface of column `col`, if it has one: the highest wet
/// cell the sky reaches. Water under a roof is not in the sky's run and has none.
fn open_water_at(w: &World, col: usize, top: u32) -> Option<usize> {
    let plane = w.config.width as usize * w.config.depth as usize;
    (top != u32::MAX).then_some(top as usize * plane + col)
}

/// Every column's [`sky_cell_at`], in column order (`z`, then `x`): the cells rain lands on.
fn sky_cells(w: &mut World) -> Vec<usize> {
    let top = sky_tops(w);
    (0..top.len())
        .filter_map(|col| sky_cell_at(w, col, top[col]))
        .collect()
}

/// The walk [`sky_cell_at`] replaced, kept as its test oracle.
#[cfg(test)]
fn sky_cell_walk(w: &World, x: i64, z: u32) -> Option<usize> {
    let c = &w.config;
    for y in (0..c.height).rev() {
        let i = c.index(x, y, z);
        if w.material[i].is_solid() || w.free[i] >= 1.0 {
            return if y + 1 < c.height {
                Some(c.index(x, y + 1, z))
            } else {
                None
            };
        }
        if w.free[i] > 0.0 {
            return Some(i);
        }
    }
    Some(c.index(x, 0, z))
}

/// The walk [`open_water_at`] replaced, kept as its test oracle.
#[cfg(test)]
fn open_water_cell_walk(w: &World, x: i64, z: u32) -> Option<usize> {
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

/// Make a world's active sets trustworthy before any phase iterates them.
///
/// The sets are a cache of what the water arrays say, so a fresh, decoded or resized world
/// has to rebuild them first. Every entry point that runs phases — [`step`] here and the
/// schedule in `cubarium-voxel-sim` — calls this once before the first phase of a tick.
pub fn begin(world: &mut World) {
    crate::voxel_phase!(Begin, {
        if world.wet.needs_rebuild(world.config.cells())
            || world.damp.needs_rebuild(world.config.cells())
        {
            world.rebuild_active_sets();
        }
    });
}

/// One tick of water, as one call. **The phase order is the rule** and it is written out
/// once, here; `cubarium-voxel-sim`'s schedule chains the same public phases in the same
/// order and this stays as the three-call sequence's water leg for tests and warm-ups.
/// `threads` reaches only [`exchange`], which ignores it since package PA.
pub fn step(world: &mut World, threads: usize) {
    begin(world);
    crate::voxel_phase!(WorldStep, {
        rain(world);
        evaporate(world);
        let substeps = world.config.water_substeps.max(1);
        let sub_dt = DT / substeps as f64;
        crate::voxel_phase!(Substeps, {
            for _ in 0..substeps {
                // Infiltration first, and inside the substep: water standing on a permeable
                // cell is offered to it before anything moves the water somewhere else. See
                // the module doc on why runoff is what infiltration refuses.
                infiltrate(world, sub_dt);
                fall(world);
                exchange(world, threads);
            }
        });
        drain(world);
        water_table(world);
        spring(world);
        outlet(world);
    });
}

/// The tick's rain. Under the open budget it is the prescribed rate falling on every
/// sky-exposed column from nowhere; under the closed one it is [`shower`], which falls
/// only while a shower is running and only out of the atmosphere store.
pub fn rain(w: &mut World) {
    #[cfg(feature = "profile")]
    let census = crate::profile::census::before(w, crate::profile::census::Tag::Rain);
    if w.config.closed_water_budget {
        shower(w);
    } else {
        prescribed_rain(w);
    }
    #[cfg(feature = "profile")]
    crate::profile::census::after(w, census);
}

fn prescribed_rain(w: &mut World) {
    crate::voxel_phase!(Rain, {
        let per_column = w.config.rain_m_per_s * DT * w.config.cell_area();
        if per_column <= 0.0 {
            return;
        }
        let mut credited = 0.0;
        for i in sky_cells(w) {
            credited += add_free(w, i, per_column);
        }
        w.ledger.rain_in += credited;
    });
}

pub fn evaporate(w: &mut World) {
    // Measurement only. An early return inside the phase skips `after`, and every early
    // return in these phases comes before any water has moved, so nothing is missed.
    #[cfg(feature = "profile")]
    let census = crate::profile::census::before(w, crate::profile::census::Tag::Evaporate);
    crate::voxel_phase!(Evaporate, {
        let per_column = w.config.evaporation_m_per_s * DT * w.config.cell_area();
        if per_column <= 0.0 {
            return;
        }
        let top = sky_tops(w);
        let mut debited = 0.0;
        for col in 0..top.len() {
            if let Some(i) = open_water_at(w, col, top[col]) {
                debited += take_free(w, i, per_column);
            }
        }
        w.ledger.evaporation_out += debited;
        release(w, debited);
    });
    #[cfg(feature = "profile")]
    crate::profile::census::after(w, census);
}

/// The weather stream: splitmix64 over the world seed, a fixed token and the shower
/// count, so the same world draws the same weather however it is stepped and no clock
/// reaches it. The generator's own idiom (`generate.rs`, `noise.rs`).
const WEATHER_STREAM: u64 = 0x_5745_4154_4845_525F;

/// The tick the next shower is due: `from` plus a draw from the configured interval.
///
/// Zero — never due — when no schedule is configured, which is what every fixture and
/// every trigger-only world has. The draw is inclusive of both ends and uses the shower
/// count as its sequence index, so a world that has rained `n` times always draws its
/// `n + 1`th gap the same way.
pub(crate) fn next_shower_tick(c: &Config, from: u64, showers: u64) -> u64 {
    if !(c.shower_interval_max_s > 0.0) {
        return 0;
    }
    let hz = f64::from(crate::TICK_HZ);
    let lo = (c.shower_interval_min_s * hz).round().max(0.0) as u64;
    let hi = ((c.shower_interval_max_s * hz).round().max(0.0) as u64).max(lo);
    let mut z = WEATHER_STREAM
        ^ c.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ showers.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    from + lo + if hi > lo { z % (hi - lo + 1) } else { 0 }
}

/// The closed budget's rain: showers drawn out of the lumped atmosphere store.
///
/// **When** it rains is either the store's business or the calendar's. With no schedule
/// ([`crate::Config::shower_interval_max_s`] zero) a shower starts the moment the store
/// crosses [`crate::Config::shower_trigger_fraction`] of the world's total water, which
/// makes the period a function of the return flux and the intervals regular. With a
/// schedule the due tick is drawn from the world's seed and that same fraction becomes an
/// **availability floor**: a due shower falls if the store can pay for it, and waits for
/// the first later tick it can if it cannot. Either way an empty sky never rains.
///
/// A shower is allowed
/// [`crate::Config::shower_volume_m3`] (or the whole store, if that is less). While it
/// runs, every tick reserves the volume the rate asks for *before* distributing it —
/// withdrawn from the store, then spread over the sky-exposed cells, with whatever a full
/// cell refuses refunded to the store in the same call
/// (`design/terrain-and-ecosystem-proposal-2026-09-16.md`, "reserve water before
/// distributing a rain event"). Between showers no rain falls at all.
///
/// The allowance is drawn down by what actually **fell**, not by what was reserved, and a
/// tick that can place nothing at all ends the shower: a brim-full world stops raining
/// rather than spinning against a closed sky forever.
///
/// Spatially uniform, like the prescribed rain it replaces. Choosing *where* a shower
/// falls is route C's job, not this store's.
pub fn shower(w: &mut World) {
    crate::voxel_phase!(Rain, {
        let scheduled = w.config.shower_interval_max_s > 0.0;
        if w.shower_left_m3 <= 0.0 {
            // Not due yet: the calendar, not the store, is what is holding the rain.
            if scheduled && w.tick < w.next_shower_tick {
                return;
            }
            let floor = w.config.shower_trigger_fraction * w.ledger.expected_total();
            if !(w.atmosphere_m3 >= floor) || w.atmosphere_m3 <= 0.0 {
                return;
            }
            // The allowance decides whether a shower starts at all: a zero shower volume
            // is no weather, not a shower counted afresh on every tick forever.
            let allowance = w.config.shower_volume_m3.min(w.atmosphere_m3);
            if allowance <= 0.0 {
                return;
            }
            w.shower_left_m3 = allowance;
            w.ledger.showers += 1;
        }
        deliver(w);
        // A shower that has just run out books the next one. Drawn from where this one
        // ended, so the gap is a gap between showers and not between their starts.
        if scheduled && w.shower_left_m3 <= 0.0 {
            w.next_shower_tick = next_shower_tick(&w.config, w.tick, w.ledger.showers);
        }
    });
}

/// One tick of a shower already in progress: reserve, spread, refund what the world
/// refused, and draw the allowance down by what actually fell.
fn deliver(w: &mut World) {
    {
        let per_column = w.config.rain_m_per_s * DT * w.config.cell_area();
        if per_column <= 0.0 {
            return;
        }
        let targets = sky_cells(w);
        if targets.is_empty() {
            // No sky at all: this shower can never fall, so do not hold the store hostage.
            w.shower_left_m3 = 0.0;
            return;
        }

        // Reserve first: the volume leaves the store before a drop of it is placed.
        let reserved = (per_column * targets.len() as f64)
            .min(w.shower_left_m3)
            .min(w.atmosphere_m3);
        if reserved <= 0.0 {
            w.shower_left_m3 = 0.0;
            return;
        }
        w.atmosphere_m3 -= reserved;
        w.ledger.atmosphere_out += reserved;

        let mut left = reserved;
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
        // Refund what the world refused, so the reservation never invents or loses water.
        let refund = reserved - credited;
        if refund > 0.0 {
            w.atmosphere_m3 += refund;
            w.ledger.atmosphere_out -= refund;
        }

        if credited <= 0.0 {
            w.shower_left_m3 = 0.0;
        } else {
            w.shower_left_m3 = (w.shower_left_m3 - credited).max(0.0);
        }
        if w.atmosphere_m3 <= 0.0 {
            w.shower_left_m3 = 0.0;
        }
    }
}

/// Every void cell hands what it can to the void cell below. Purely vertical, so no
/// horizontal direction is picked; a column compacts by one cell per substep.
///
/// Iterated over a **snapshot of the wet cells taken at the start of this call**, ordered
/// bottom-up, instead of walking every wet column through its full height. Ascending world
/// index is bottom-up: `y` is the outer index, so every row is visited before the row above
/// it, and within one column the lower cell is always met first. The snapshot is what keeps
/// the rule: a cell wetted by a transfer from the cell above it is **not** in the snapshot,
/// so it does not carry that arrival farther down in the same call, while a wet cell above
/// still gets its own turn later and makes room. The snapshot is read back ascending through
/// a bitmap ([`crate::sparse::CellSet::sorted_into`]) rather than sorted, and that is
/// measured as part of the phase (`design/handoffs/voxel-sparse-fall-2026-09-18.md`).
pub fn fall(w: &mut World) {
    #[cfg(feature = "profile")]
    let census = crate::profile::census::before(w, crate::profile::census::Tag::Fall);
    crate::voxel_phase!(Fall, {
        let plane = w.config.width as usize * w.config.depth as usize;
        SCRATCH.with(|slot| {
            let sc = &mut *slot.borrow_mut();
            #[cfg(feature = "profile")]
            let set = crate::profile::start(crate::profile::Phase::FallSet);
            w.wet.sorted_into(&mut sc.bits, &mut sc.fall);
            #[cfg(feature = "profile")]
            drop(set);
            #[cfg(feature = "profile")]
            let (mut visited, mut moved) = (0u64, 0u64);
            for &i in &sc.fall {
                // The bottom row has nowhere to fall to; it is in the snapshot only because
                // the snapshot is the whole wet set.
                if i < plane || w.material[i].is_solid() {
                    continue;
                }
                #[cfg(feature = "profile")]
                {
                    visited += 1;
                }
                if w.free[i] <= 0.0 {
                    continue;
                }
                let below = i - plane;
                let want = free_m3(w, i).min(free_room_m3(w, below));
                if want > 0.0 {
                    let _got = transfer(w, (i, Store::Free), (below, Store::Free), want);
                    #[cfg(feature = "profile")]
                    if _got > 0.0 {
                        moved += 1;
                    }
                }
            }
            #[cfg(feature = "profile")]
            {
                crate::profile::add(crate::profile::Count::FallCells, visited);
                crate::profile::add(crate::profile::Count::FallMoved, moved);
            }
        });
    });
    #[cfg(feature = "profile")]
    crate::profile::census::after(w, census);
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
///    `conductivity_m_per_s` across the cell face, capped by the aquifer's stock.
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
pub fn water_table(w: &mut World) {
    // Measurement only; an early return skips `after` before anything has moved.
    #[cfg(feature = "profile")]
    let census = crate::profile::census::before(w, crate::profile::census::Tag::WaterTable);
    crate::voxel_phase!(WaterTable, {
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
            if rows <= 0.0 {
                0
            } else {
                (rows as u32).min(c.height)
            }
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
                    let rate = pore_flux_m3(m, &c, DT);
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
                    let rate = pore_flux_m3(m, &c, DT);
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
    });
    #[cfg(feature = "profile")]
    crate::profile::census::after(w, census);
}

/// Scratch buffers for [`exchange`], reused for the life of the thread so the phase
/// allocates nothing: four substeps a tick used to allocate and zero eight grid-sized
/// vectors each (`design/7_Research/voxel-tick-profile-2026-09-18.md`). Every entry is
/// written before it is read inside one call, so nothing here is state.
///
/// **Everything per cell is in the world's own index order** (`y`, then `z`, then `x`),
/// and the active set is walked ascending, so the phase reads `free`, `material` and these
/// buffers front to back. The column-major layout that existed to hand a parallel scan
/// disjoint spans is gone with that scan (package PA, 2026-09-22: the pool sat on the
/// board's little cores at an IPC of 0.1 and the scan was 1.7 ms of the tick).
#[derive(Default)]
struct Scratch {
    /// A wet cell's head, in cell units: the surface of the full stack it sits at the foot
    /// of (package H, [`scan_column_mask`]) — its own `y + free` if it is not full. Valid
    /// for the wet cells of this substep's active columns.
    head: Vec<f64>,
    /// Dense fallback only (worlds taller than [`MASK_ROWS`]): for every non-solid cell of
    /// an active column, the lowest cell **at or above** it, inside its own void run, that
    /// still has room — where a push against this cell actually displaces water to.
    /// `u32::MAX` when the run is full to its ceiling. The value is a world cell index.
    room_target: Vec<u32>,
    /// Dense fallback only: the top cell of each non-solid cell's own void run, where a
    /// displacement stops. A world cell index.
    run_top: Vec<u32>,
    /// Wet and effectively-full rows in each column, for the mask path.
    wet_mask: Vec<u128>,
    full_mask: Vec<u128>,
    /// One cell's offers this pass, `(destination, volume)`, before the giver's own stock
    /// scales them.
    offers: Vec<(usize, f64)>,
    /// Package P: every giver's offers this substep, `(destination, volume, face)` with
    /// face 0-3 horizontal and 4 down, kept until its stack's totals are known; and the
    /// givers themselves, in the active set's order.
    offer_list: Vec<(u32, f64, u8)>,
    givers: Vec<Giver>,
    /// Package P: each wet cell's stack this substep (an index into `stacks`), and each
    /// stack's per-face totals.
    stack_of: Vec<u32>,
    stacks: Vec<StackFaces>,
    /// The **driving** head: a cell's own surface head, raised to the highest head that
    /// reaches it through submerged water. A giver pushes with its `drive`; a receiver
    /// resists with its own `head`, because what a neighbour presents to the water arriving
    /// is its surface level and not the pressure passing through it.
    drive: Vec<f64>,
    /// The next `drive` while the current one is read: the propagation is Jacobi, two
    /// buffers, so it does not depend on the order the set is walked in.
    drive_next: Vec<f64>,
    /// The wet cells at the start of the substep, **ascending**, copied so the sets may be
    /// edited while the flux is applied; `rows` holds each one's `y`, found by walking the
    /// ascending list rather than by dividing.
    active: Vec<usize>,
    rows: Vec<u32>,
    /// The bitmap [`crate::sparse::CellSet::sorted_into`] sorts through; all zero between
    /// calls.
    bits: Vec<u64>,
    /// The wet cells at the start of a [`fall`] call, ascending, reused across substeps and
    /// ticks so the phase allocates nothing. `fall` and `exchange` never overlap, so they
    /// share this one scratch.
    fall: Vec<usize>,
    /// The **full** cells of the active set, as positions in `active`: the submerged ones,
    /// which are the only cells that carry another column's head.
    full: Vec<u32>,
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
    /// Per column: its four horizontal neighbours' columns in the exchange's face order
    /// — `x - 1`, `x + 1`, `z - 1`, `z + 1` — with `u32::MAX` past a `z` wall. The same
    /// row's neighbour cell is `y * plane + nbr[col][k]`: no `coords`, no `rem_euclid`.
    nbr: Vec<[u32; 4]>,
    nbr_dims: (usize, usize),
    /// Measurement only: the substep's stamp on every cell an edge actually moved water
    /// into or out of, for the census's no-edge count.
    #[cfg(feature = "profile")]
    moved_stamp: Vec<u32>,
}

impl Scratch {
    fn ensure(&mut self, n: usize, plane: usize, width: usize, depth: usize) {
        if self.head.len() != n {
            self.head = vec![0.0; n];
            self.drive = vec![0.0; n];
            self.drive_next = vec![0.0; n];
            self.room_target = vec![u32::MAX; n];
            self.run_top = vec![u32::MAX; n];
            self.stack_of = vec![0; n];
            self.touch_stamp = vec![0; n];
            self.proposed_in = vec![0.0; n];
            self.accept = vec![0.0; n];
            self.delta = vec![0.0; n];
            #[cfg(feature = "profile")]
            {
                self.moved_stamp = vec![0; n];
            }
            self.stamp = 0;
        }
        if self.col_stamp.len() != plane {
            self.col_stamp = vec![0; plane];
            self.wet_mask = vec![0; plane];
            self.full_mask = vec![0; plane];
            self.stamp = 0;
        }
        if self.nbr_dims != (width, depth) {
            self.nbr = neighbour_columns(width, depth);
            self.nbr_dims = (width, depth);
        }
    }

    /// A fresh stamp for this substep's dedup arrays, clearing them on the wrap.
    fn next_stamp(&mut self) {
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.col_stamp.fill(0);
            self.touch_stamp.fill(0);
            #[cfg(feature = "profile")]
            self.moved_stamp.fill(0);
            self.stamp = 1;
        }
    }
}

/// One giver's offers in [`Scratch::offer_list`] (`start..end`), and what it placed across
/// each horizontal face and down, before its stack's cap and the Jacobi division.
struct Giver {
    i: u32,
    #[cfg(feature = "profile")]
    y: u32,
    stack: u32,
    start: u32,
    end: u32,
    have: f64,
    placed: [f64; 4],
    down: f64,
}

/// A **stack**'s offers across each horizontal face (package P): the sum over its rows and
/// the most any single row placed, and whether its bottom cell offers down out of it.
#[derive(Default)]
struct StackFaces {
    placed: [f64; 4],
    best: [f64; 4],
    down: bool,
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

/// Every column's four horizontal neighbour columns, in the exchange's face order: `x`
/// wraps, `z` stops at the walls (`u32::MAX`). A column is `z * width + x`.
fn neighbour_columns(width: usize, depth: usize) -> Vec<[u32; 4]> {
    let mut out = Vec::with_capacity(width * depth);
    for z in 0..depth {
        for x in 0..width {
            let row = z * width;
            out.push([
                (row + (x + width - 1) % width) as u32,
                (row + (x + 1) % width) as u32,
                if z > 0 {
                    (row - width + x) as u32
                } else {
                    u32::MAX
                },
                if z + 1 < depth {
                    (row + width + x) as u32
                } else {
                    u32::MAX
                },
            ]);
        }
    }
    out
}

/// The `y` of every cell of an **ascending** index list, by walking the rows alongside it:
/// one comparison per cell and one addition per row instead of a division per cell.
fn rows_of(cells: &[usize], plane: usize, rows: &mut Vec<u32>) {
    rows.clear();
    let (mut y, mut end) = (0u32, plane);
    for &i in cells {
        while i >= end {
            end += plane;
            y += 1;
        }
        rows.push(y);
    }
}

/// **The local exchange: what replaced the region solver.**
///
/// Each substep every wet cell offers water to its four horizontal neighbours and to the
/// cell below, driven by the difference in **column head** — the surface level of the
/// packed full stack the cell sits at the foot of ([`scan_column_mask`]): its own
/// `y + free` if it is not full. That one
/// definition is what carries pressure without any connectivity search: the bottom cell of
/// a deep column has its whole column's head, so it pushes hard sideways; a full cell in a
/// submerged gap has the head of the body it is part of and passes the push along; and a
/// dry cell's head is its own floor, so water runs into an empty neighbour and off a ledge.
///
/// A push against a **full** neighbour is not refused, it is **displaced**: the water
/// enters the lowest cell with room at or above that neighbour, inside the neighbour's own
/// void run. That is incompressibility, locally — pushing at the bottom of a submerged
/// column lifts its surface — and it is what makes a U-tube level and a roofed passage push
/// the far shaft up without anything ever flowing upward against its own head.
///
/// The flux across one face is `FLOW_PER_SUBSTEP * (head_here - head_there)`, capped three
/// ways: by the room at the destination, by what the giver holds (its proposals are scaled
/// down together if they ask for more), and by the destination's total acceptance (every
/// proposal into one cell is scaled by the same factor when they overfill it together).
/// Both ends of every proposal are read from the **old** water and the whole of it is
/// applied at the end, so no cell's result depends on the order the set was walked in
/// beyond the rounding of the per-cell sums; the set is walked in ascending index order,
/// which is the world's memory order.
///
/// **What this is not.** There is no momentum, no velocity field and no free surface: a
/// lake no longer levels in one substep, it relaxes at half of its head difference per
/// substep, which is a travelling wave rather than an instant re-level. `FLOW_PER_SUBSTEP`
/// is a placeholder and the only number in the rule.
///
/// `threads` is unused since package PA: the scan it split is serial again (see
/// [`Scratch`]). It stays in the signature so the schedule's callers are unchanged.
pub fn exchange(w: &mut World, threads: usize) {
    #[cfg(feature = "profile")]
    let census = crate::profile::census::before(w, crate::profile::census::Tag::Exchange);
    crate::voxel_phase!(Exchange, { exchange_inner(w, threads) });
    #[cfg(feature = "profile")]
    crate::profile::census::after(w, census);
}

fn exchange_inner(w: &mut World, threads: usize) {
    exchange_inner_with_masks(w, threads, w.config.height as usize <= MASK_ROWS);
}

fn exchange_inner_with_masks(w: &mut World, _threads: usize, use_masks: bool) {
    if w.wet.len() == 0 {
        return;
    }
    let width = w.config.width as usize;
    let depth = w.config.depth as usize;
    let height = w.config.height as usize;
    let plane = width * depth;
    let n = plane * height;
    let transfer_cap = w.config.free_transfer_cap;
    // The spreading depth in this world's cell units (package C).
    let min_spread = MIN_SPREAD_DEPTH_M / w.config.voxel_m;
    // The void-run geometry the scan needs is terrain-only, so it is built once per
    // terrain version and not per substep.
    w.ensure_void_runs();
    SCRATCH.with(|slot| {
        let sc = &mut *slot.borrow_mut();
        #[cfg(feature = "profile")]
        let mut lap = crate::profile::start(crate::profile::Phase::ExchangeSet);
        sc.ensure(n, plane, width, depth);
        sc.next_stamp();
        let stamp = sc.stamp;
        // ---- the active set: the wet cells, ascending, and the columns they and their
        // horizontal neighbours live in. Dry rock and dry air are never looked at.
        w.wet.sorted_into(&mut sc.bits, &mut sc.active);
        rows_of(&sc.active, plane, &mut sc.rows);
        if use_masks {
            sc.wet_mask.fill(0);
            sc.full_mask.fill(0);
            for k in 0..sc.active.len() {
                let (i, y) = (sc.active[k], sc.rows[k] as usize);
                let col = i - y * plane;
                let bit = 1u128 << y;
                sc.wet_mask[col] |= bit;
                if w.free[i] >= 1.0 - ROOM_EPS {
                    sc.full_mask[col] |= bit;
                }
            }
        }
        for k in 0..sc.active.len() {
            let col = sc.active[k] - sc.rows[k] as usize * plane;
            sc.col_stamp[col] = stamp;
            for nb in sc.nbr[col] {
                if nb != u32::MAX {
                    sc.col_stamp[nb as usize] = stamp;
                }
            }
        }
        // Collected **ascending** by one pass over the stamp array rather than pushed in
        // walk order: the column scan writes disjoint cells, so its order cannot reach a
        // result, and one pass over `plane` columns is cheaper than a sort.
        sc.columns.clear();
        for col in 0..plane {
            if sc.col_stamp[col] == stamp {
                sc.columns.push(col);
            }
        }
        #[cfg(feature = "profile")]
        {
            crate::profile::add(crate::profile::Count::ExchangeWet, sc.active.len() as u64);
            crate::profile::add(
                crate::profile::Count::ExchangeColumns,
                sc.columns.len() as u64,
            );
        }

        // ---- one pass per active column: heads, and (dense fallback only) where a push
        // displaces to.
        #[cfg(feature = "profile")]
        lap.next(crate::profile::Phase::ExchangeScan);
        scan_columns(w, plane, use_masks, sc);
        #[cfg(feature = "profile")]
        lap.next(crate::profile::Phase::ExchangeHeads);

        // ---- head through submerged water. A full cell has no free surface of its own,
        // so it carries the highest head that reaches it across its horizontal faces: that
        // is how the weight of one column arrives at the foot of another through a flooded
        // passage, which is the whole of the U-tube and the roofed gap. The **driving**
        // head is what a giver pushes with; a receiver still resists with its own surface
        // head, or a push would cancel itself against the pressure it just transmitted.
        //
        // `max` only raises, so the passes converge; two buffers keep the answer off the
        // walk order, and the loop stops as soon as a pass raises nothing.
        sc.full.clear();
        for k in 0..sc.active.len() {
            let i = sc.active[k];
            sc.drive[i] = sc.head[i];
            if w.free[i] >= 1.0 - ROOM_EPS {
                sc.full.push(k as u32);
            }
        }
        #[cfg(feature = "profile")]
        let mut passes = 0u64;
        for _ in 0..HEAD_PASSES {
            #[cfg(feature = "profile")]
            {
                passes += 1;
            }
            let mut raised = false;
            for f in 0..sc.full.len() {
                let k = sc.full[f] as usize;
                let (i, y) = (sc.active[k], sc.rows[k] as usize);
                let base = y * plane;
                let mut h = sc.drive[i];
                for nb in sc.nbr[i - base] {
                    if nb == u32::MAX {
                        continue;
                    }
                    let j = base + nb as usize;
                    if w.material[j].is_solid() || w.free[j] <= 0.0 {
                        continue;
                    }
                    if sc.drive[j] > h {
                        h = sc.drive[j];
                    }
                }
                sc.drive_next[i] = h;
            }
            for f in 0..sc.full.len() {
                let i = sc.active[sc.full[f] as usize];
                if sc.drive_next[i] > sc.drive[i] {
                    sc.drive[i] = sc.drive_next[i];
                    raised = true;
                }
            }
            if !raised {
                break;
            }
        }
        #[cfg(feature = "profile")]
        {
            use crate::profile::{Count, add};
            let full = sc.full.len() as u64;
            add(Count::ExchangeFull, full);
            add(Count::HeadPasses, passes);
            add(Count::HeadPassCells, passes * full);
            lap.next(crate::profile::Phase::ExchangeOffers);
        }

        // ---- the proposals, all read from the old water.
        #[cfg(feature = "profile")]
        let census = crate::profile::census::open();
        #[cfg(feature = "profile")]
        let (mut raised, mut edge_depth, mut edge_depth_unpacked) = (
            0u64,
            [0u64; crate::profile::census::BINS],
            [0u64; crate::profile::census::BINS],
        );
        sc.edges.clear();
        sc.touched.clear();
        sc.offer_list.clear();
        sc.givers.clear();
        sc.stacks.clear();
        for k in 0..sc.active.len() {
            let i = sc.active[k];
            let have = w.free[i];
            if have <= 0.0 {
                continue;
            }
            let y = sc.rows[k] as usize;
            let base = y * plane;
            let col = i - base;
            let here = sc.drive[i];
            // **The stack** (package P): a wet cell continues the stack of the wet cell under
            // it when the two carry the same head — the full cells under a partial top share
            // that top's surface (package H) — and starts a stack of its own otherwise. The
            // set is walked ascending, so the cell below has its stack already.
            let continues = y > 0 && {
                let below = i - plane;
                w.free[below] > 0.0 && sc.head[below] == sc.head[i]
            };
            let stack = if continues {
                sc.stack_of[i - plane] as usize
            } else {
                sc.stacks.push(StackFaces::default());
                sc.stacks.len() - 1
            };
            sc.stack_of[i] = stack as u32;
            sc.offers.clear();
            let start = sc.offer_list.len() as u32;
            let mut placed_by_face = [0.0f64; 4];

            // The four horizontal faces. A push against a **full** neighbour is not
            // refused, it is displaced: the water goes into the lowest cells with room at
            // or above that neighbour, inside the neighbour's own void run, bottom first.
            // That is what lifts a submerged column's surface instead of throttling the
            // flow to the hair of room its floor cell has left.
            //
            // A giver shallower than the spreading depth offers across none of them
            // (package C): it keeps its water, and still offers downward below.
            let horizontal = if have < min_spread { 0 } else { 4 };
            for face in 0..horizontal {
                let nb = sc.nbr[col][face];
                if nb == u32::MAX {
                    continue;
                }
                let j = base + nb as usize;
                if w.material[j].is_solid() {
                    continue;
                }
                let there = if w.free[j] > 0.0 {
                    sc.head[j]
                } else {
                    y as f64
                };
                let drop = here - there;
                if drop <= 0.0 {
                    continue;
                }
                let q = cap_flux(transfer_cap, FLOW_PER_SUBSTEP * drop);
                let before = sc.offers.len();
                let placed = if use_masks {
                    offer_up_the_run_mask(w, plane, nb as usize, y, q, sc)
                } else {
                    offer_up_the_run(w, plane, j, q, sc)
                };
                for &(target, q) in &sc.offers[before..] {
                    sc.offer_list.push((target as u32, q, face as u8));
                }
                if placed > 0.0 {
                    placed_by_face[face] = placed;
                    let st = &mut sc.stacks[stack];
                    st.placed[face] += placed;
                    if placed > st.best[face] {
                        st.best[face] = placed;
                    }
                }
            }

            // And straight down, into the cell's own room below: the same rule, and the
            // reason a film keeps moving when `fall` has already taken what it can.
            let mut down = 0.0;
            if y > 0 {
                let below = i - plane;
                if !w.material[below].is_solid() {
                    let room = (1.0 - w.free[below]).max(0.0);
                    let there = if w.free[below] > 0.0 {
                        sc.head[below]
                    } else {
                        (y - 1) as f64
                    };
                    let drop = here - there;
                    if drop > 0.0 && room > 0.0 {
                        let q = cap_flux(transfer_cap, FLOW_PER_SUBSTEP * drop).min(room);
                        if q > 0.0 {
                            sc.offer_list.push((below as u32, q, 4));
                            down = q;
                            // Only a stack's bottom cell offers *out* of it downward; a
                            // sliver into the stack's own cell below is inside it.
                            if !continues {
                                sc.stacks[stack].down = true;
                            }
                        }
                    }
                }
            }

            // **There is no separate upward push**, and that is a rule and not an
            // omission: a column rises because water *arriving* at it is displaced up its
            // own run (`offer_up_the_run`), which is what lifts the far shaft of a U-tube.
            // Lifting a cell's own water into the cell above it instead only shuffles a
            // column against `fall` — the two fight, and a roofed passage never fills.

            let end = sc.offer_list.len() as u32;
            if end > start {
                sc.givers.push(Giver {
                    i: i as u32,
                    #[cfg(feature = "profile")]
                    y: y as u32,
                    stack: stack as u32,
                    start,
                    end,
                    have,
                    placed: placed_by_face,
                    down,
                });
            }
        }

        // ---- the stack's caps and the Jacobi division, then the edges.
        //
        // **One face, one row's worth** (package P, 2026-09-22). What a stack sends across
        // one horizontal face into one neighbouring column is capped at what the best single
        // row of it would send: its rows share that amount in proportion to their offers.
        // `FLOW_PER_SUBSTEP` is safe for one pair of cells — half a difference levels them
        // exactly — but a stack facing its neighbour over `k` rows used to send `k` halves,
        // so two columns side by side over two full rows swapped levels every substep and
        // never settled (the shimmer on still water). A one-row interface, a film and a
        // lone cell are unchanged.
        //
        // Then the ordinary Jacobi damping, **counted per stack**: every offer is divided by
        // the number of faces the stack offers across — its horizontal faces and, if its
        // bottom cell offers down out of it, that one — so a stack with two low neighbours
        // hands each half of what one would get, and relaxes instead of ringing. Finally a
        // cell cannot give more than it holds: its offers are scaled together if they still
        // ask for more, so no face is ever preferred.
        for gi in 0..sc.givers.len() {
            let g = &sc.givers[gi];
            let (i, have) = (g.i as usize, g.have);
            let st = &sc.stacks[g.stack as usize];
            let mut cap = [1.0f64; 4];
            let mut faces = usize::from(st.down);
            for face in 0..4 {
                if st.placed[face] > 0.0 {
                    faces += 1;
                    if st.placed[face] > st.best[face] {
                        cap[face] = st.best[face] / st.placed[face];
                    }
                }
            }
            let mut total = 0.0;
            for face in 0..4 {
                if g.placed[face] > 0.0 {
                    total += g.placed[face] * cap[face];
                }
            }
            total += g.down;
            if total <= 0.0 {
                continue;
            }
            let share = 1.0 / faces.max(1) as f64;
            let total = total * share;
            let scale = share * if total > have { have / total } else { 1.0 };
            #[cfg(feature = "profile")]
            let (y, base, edges_before) = (g.y as usize, g.y as usize * plane, sc.edges.len());
            for oi in g.start as usize..g.end as usize {
                let (target, q, face) = sc.offer_list[oi];
                let target = target as usize;
                let q = q * if face < 4 {
                    scale * cap[face as usize]
                } else {
                    scale
                };
                if q <= 0.0 {
                    continue;
                }
                #[cfg(feature = "profile")]
                if target >= base + plane {
                    raised += 1;
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
            #[cfg(feature = "profile")]
            if census {
                let col = i - base;
                let b = crate::profile::census::bin(have);
                let made = (sc.edges.len() - edges_before) as u64;
                edge_depth[b] += made;
                // Unpacked: not full, with a wet cell above it in its own column, so its
                // head is its run's top however little water lies between.
                let above = sc.wet_mask[col].checked_shr(y as u32 + 1).unwrap_or(0) & 1;
                if use_masks && have < 1.0 - ROOM_EPS && above != 0 {
                    edge_depth_unpacked[b] += made;
                }
            }
        }

        #[cfg(feature = "profile")]
        {
            crate::profile::add(crate::profile::Count::ExchangeEdges, sc.edges.len() as u64);
            crate::profile::add(crate::profile::Count::ExchangeRaised, raised);
            lap.next(crate::profile::Phase::ExchangeApply);
        }

        // ---- what each destination can actually take, whoever offered it.
        for ti in 0..sc.touched.len() {
            let cell = sc.touched[ti];
            let proposed = sc.proposed_in[cell];
            let room = (1.0 - w.free[cell]).max(0.0);
            sc.accept[cell] = if proposed > room && proposed > 0.0 {
                room / proposed
            } else {
                1.0
            };
        }

        // ---- apply: one subtraction and one addition per edge, accumulated per cell so
        // that a cell touched by several edges is written once.
        #[cfg(feature = "profile")]
        let (mut moved_edges, mut net_cells) = (0u64, 0u64);
        for ei in 0..sc.edges.len() {
            let (from, to, q) = sc.edges[ei];
            let moved = q * sc.accept[to as usize];
            if moved <= 0.0 {
                continue;
            }
            sc.delta[from as usize] -= moved;
            sc.delta[to as usize] += moved;
            #[cfg(feature = "profile")]
            {
                moved_edges += 1;
                if census {
                    sc.moved_stamp[from as usize] = stamp;
                    sc.moved_stamp[to as usize] = stamp;
                }
            }
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
                #[cfg(feature = "profile")]
                {
                    net_cells += 1;
                }
            }
        }
        #[cfg(feature = "profile")]
        {
            crate::profile::add(crate::profile::Count::ExchangeMoved, moved_edges);
            crate::profile::add(crate::profile::Count::ExchangeNet, net_cells);
            drop(lap);
            if census {
                // The substep's shape, for the census only: the active wet cells no
                // accepted edge touched; the wet runs and columns of its masks; and the
                // **unpacked** cells, wet and not full under a wet cell, whose head is
                // their run's top although the water between is not there.
                let no_edge = sc
                    .active
                    .iter()
                    .filter(|&&i| sc.moved_stamp[i] != stamp)
                    .count() as u64;
                let (mut runs, mut columns, mut unpacked) = (0u64, 0u64, 0u64);
                if use_masks {
                    for &col in &sc.columns {
                        let (m, f) = (sc.wet_mask[col], sc.full_mask[col]);
                        if m != 0 {
                            columns += 1;
                            runs += u64::from((m & !(m << 1)).count_ones());
                            unpacked += u64::from((m & !f & (m >> 1)).count_ones());
                        }
                    }
                }
                crate::profile::census::exchange_substep(crate::profile::census::Substep {
                    wet: sc.active.len() as u64,
                    no_edge,
                    runs,
                    columns,
                    unpacked,
                    edge_depth,
                    edge_depth_unpacked,
                });
            }
        }
    });
}

/// Place `q` into the cells with room at or above world cell `j`, inside that cell's own
/// void run, bottom first, recording each part as an offer. Returns how much of `q`
/// actually found room. The dense fallback's lookup, for worlds taller than
/// [`MASK_ROWS`].
///
/// This is the displacement rule: pushing against a submerged cell fills the stack above
/// it rather than stopping at whatever room that one cell has left. Without it a nearly
/// full column throttles the flow into it to its own remaining hair of room, and a U-tube
/// crawls toward its level geometrically instead of reaching it.
fn offer_up_the_run(w: &World, plane: usize, j: usize, q: f64, sc: &mut Scratch) -> f64 {
    let (at, top) = (sc.room_target[j], sc.run_top[j]);
    if at == u32::MAX || top == u32::MAX {
        return 0.0;
    }
    offer_from(w, plane, at as usize, top as usize, q, &mut sc.offers)
}

/// The walk both lookups share: from world cell `at` up to `top`, one row at a time,
/// every positive sliver of room takes what is left of `q`.
#[inline]
fn offer_from(
    w: &World,
    plane: usize,
    mut at: usize,
    top: usize,
    q: f64,
    offers: &mut Vec<(usize, f64)>,
) -> f64 {
    let mut left = q;
    let mut placed = 0.0;
    while left > 0.0 && at <= top {
        let room = (1.0 - w.free[at]).max(0.0);
        if room > 0.0 {
            let take = left.min(room);
            offers.push((at, take));
            placed += take;
            left -= take;
        }
        at += plane;
    }
    placed
}

/// Mask-backed displacement lookup for worlds no taller than [`MASK_ROWS`]. The first
/// target still uses `ROOM_EPS`; once found, the unchanged upward walk accepts every
/// positive sliver of room, including one smaller than `ROOM_EPS`.
fn offer_up_the_run_mask(
    w: &World,
    plane: usize,
    col: usize,
    y: usize,
    q: f64,
    sc: &mut Scratch,
) -> f64 {
    let void = w.void_runs.mask[col];
    let run_len = (void >> y).trailing_ones() as usize;
    if run_len == 0 {
        return 0.0;
    }
    let available = run_bits(y, run_len) & !sc.full_mask[col];
    if available == 0 {
        return 0.0;
    }
    let first_y = available.trailing_zeros() as usize;
    let top_y = y + run_len - 1;
    offer_from(
        w,
        plane,
        first_y * plane + col,
        top_y * plane + col,
        q,
        &mut sc.offers,
    )
}

/// `free_transfer_cap`, when the config sets one: the most one face may move in one
/// substep, in cell units. Zero leaves the flux uncapped, which is the default.
#[inline]
fn cap_flux(cap: f64, q: f64) -> f64 {
    if cap > 0.0 { q.min(cap) } else { q }
}

/// The column scan over every active column, serial. Worlds up to [`MASK_ROWS`] rows fill
/// heads from the wet-row masks; taller worlds take the dense walk over the cached void
/// runs, which also fills the displacement tables the mask path reads from its masks.
fn scan_columns(w: &World, plane: usize, use_masks: bool, sc: &mut Scratch) {
    let free = &w.free[..];
    if use_masks {
        for &col in &sc.columns {
            scan_column_mask(
                free,
                plane,
                col,
                sc.wet_mask[col],
                sc.full_mask[col],
                &mut sc.head,
            );
        }
        return;
    }
    let offset = &w.void_runs.offset[..];
    let runs = &w.void_runs.runs[..];
    for &col in &sc.columns {
        scan_column(
            free,
            plane,
            col,
            &runs[offset[col] as usize..offset[col + 1] as usize],
            &mut sc.head,
            &mut sc.room_target,
            &mut sc.run_top,
        );
    }
}

/// **Head from the packed full stack** (package H, 2026-09-22). A wet cell at row `y`
/// takes the surface of the full stack it sits at the foot of: with `s` the first row at
/// or above `y` that is not full, the head is `s + free[s]` when row `s` is wet — the
/// partial cell capping the stack — so a partial cell is its own `s` and its head is its
/// own level. A full stack capped by a dry cell, a roof or the world top keeps its top
/// cell's own surface, `top + free[top]`: the old run-top answer, which is `s` to within
/// `ROOM_EPS`. A packed run (full cells under one partial top) therefore has exactly the
/// head it always had, and a lake, a U-tube and a full-to-the-roof passage are unchanged.
///
/// What changed is a **hollow** run, wet cells that are not full stacked on each other:
/// until H every cell of it carried the run top's surface, so the bottom film of a falling
/// stack pushed at a dry neighbour with a drop the height of the stack and displaced
/// slivers up the neighbour's column (the census at 382ef3a: 78 % of a shower's active
/// wet cells, 96 % of its proposals).
///
/// Per wet run, from the top down: the top cell carries its own surface, and each cell
/// below carries the head of the cell above it if it is full, or its own level if it is
/// not. `full` is the exchange's own full mask (`free >= 1 - ROOM_EPS`). Writes `head` at
/// the column's own world indices.
#[inline]
fn scan_column_mask(
    free: &[f64],
    plane: usize,
    col: usize,
    mut wet: u128,
    full: u128,
    head: &mut [f64],
) {
    while wet != 0 {
        let y = wet.trailing_zeros() as usize;
        let run = (wet >> y).trailing_ones() as usize;
        let top = y + run - 1;
        let mut at = top * plane + col;
        let mut surface = top as f64 + free[at];
        head[at] = surface;
        for r in (y..top).rev() {
            at -= plane;
            if (full >> r) & 1 == 0 {
                surface = r as f64 + free[at];
            }
            head[at] = surface;
        }
        wet &= !run_bits(y, run);
    }
}

/// One pass over a column: the head of every wet cell, and the displacement target of
/// every non-solid cell. The dense fallback.
///
/// A **void run** is a maximal stack of non-solid cells; the displacement target of a cell
/// is the lowest cell with room at or above it *within its own run*, because a solid
/// ceiling is where a push stops. A **water run** is a maximal stack of wet cells inside a
/// void run; its heads follow [`scan_column_mask`]'s rule, from the packed full stack,
/// with the same operations in the same order so the two paths stay bit-identical.
///
/// The runs themselves are **static geometry**, handed in from [`World::void_runs`], so
/// this pass never rediscovers them from `material`; only the water-dependent values —
/// `room_target`, `run_top` and `head` — are written here, every substep, at the column's
/// own world indices.
fn scan_column(
    free: &[f64],
    plane: usize,
    col: usize,
    runs: &[VoidRun],
    head: &mut [f64],
    room_target: &mut [u32],
    run_top: &mut [u32],
) {
    for run in runs {
        let y = run.y0 as usize;
        let top = run.top as usize;
        // Displacement targets, from the ceiling down: the lowest cell with room seen so
        // far is the lowest cell with room at or above the cell being written.
        let mut best = u32::MAX;
        let top_cell = (top * plane + col) as u32;
        for k in (y..=top).rev() {
            let i = k * plane + col;
            if free[i] < 1.0 - ROOM_EPS {
                best = i as u32;
            }
            room_target[i] = best;
            run_top[i] = top_cell;
        }
        // Heads, one water run at a time.
        let mut k = y;
        while k <= top {
            let i = k * plane + col;
            if free[i] > 0.0 {
                let mut t = k;
                while t + 1 <= top && free[(t + 1) * plane + col] > 0.0 {
                    t += 1;
                }
                // From the top down: a full cell carries the head above it, a partial
                // cell its own level (package H).
                let mut at = t * plane + col;
                let mut surface = t as f64 + free[at];
                head[at] = surface;
                for m in (k..t).rev() {
                    at -= plane;
                    if free[at] < 1.0 - ROOM_EPS {
                        surface = m as f64 + free[at];
                    }
                    head[at] = surface;
                }
                k = t + 1;
            } else {
                k += 1;
            }
        }
    }
}

pub fn infiltrate(w: &mut World, dt: f64) {
    #[cfg(feature = "profile")]
    let census = crate::profile::census::before(w, crate::profile::census::Tag::Infiltrate);
    crate::voxel_phase!(Infiltrate, {
        let c = w.config.clone();
        let plane = c.width as usize * c.depth as usize;
        // Over the wet set: a cell with no free water in it has nothing to offer the ground,
        // and source and destination are disjoint (a void cell and the porous cell under it),
        // so the order the set is walked in cannot change the answer.
        // Ascending, read back through the shared scratch rather than copied fresh.
        SCRATCH.with(|slot| {
            let sc = &mut *slot.borrow_mut();
            #[cfg(feature = "profile")]
            let set = crate::profile::start(crate::profile::Phase::InfiltrateSet);
            w.wet.sorted_into(&mut sc.bits, &mut sc.fall);
            #[cfg(feature = "profile")]
            drop(set);
            #[cfg(feature = "profile")]
            crate::profile::add(crate::profile::Count::InfiltrateCells, sc.fall.len() as u64);
            for &i in &sc.fall {
                if i < plane || w.material[i].is_solid() || w.free[i] <= 0.0 {
                    continue;
                }
                let below = i - plane;
                let m = w.material[below];
                if m.pore_capacity() <= 0.0 || m.permeability_per_s() <= 0.0 {
                    continue;
                }
                let rate = pore_flux_m3(m, &c, dt);
                transfer(w, (i, Store::Free), (below, Store::Pore), rate);
            }
        });
    });
    #[cfg(feature = "profile")]
    crate::profile::census::after(w, census);
}

pub fn drain(w: &mut World) {
    #[cfg(feature = "profile")]
    let census = crate::profile::census::before(w, crate::profile::census::Tag::Drain);
    crate::voxel_phase!(Drain, {
        let c = w.config.clone();
        let plane = c.width as usize * c.depth as usize;
        // The table as it stands at the start of the step: a voxel inside the saturated
        // zone has nowhere lower to drain to, because the aquifer is what is holding it up.
        let table = c.aquifer_head_m(w.aquifer_m3);
        // Over the damp set — the cells that hold any pore water — instead of the grid
        // (`design/7_Research/voxel-tick-profile-2026-09-18.md`): a cell with no pore water
        // can never drain, so nothing is lost. The set is read back **ascending**, which is
        // bottom-up, so a stack of wet soil passes water down one cell a tick exactly as
        // the grid walk did (until package PA the walk was in swap-removal order and could
        // pass it several).
        SCRATCH.with(|slot| {
            let sc = &mut *slot.borrow_mut();
            w.damp.sorted_into(&mut sc.bits, &mut sc.fall);
            rows_of(&sc.fall, plane, &mut sc.rows);
            #[cfg(feature = "profile")]
            crate::profile::add(crate::profile::Count::DrainCells, sc.fall.len() as u64);
            #[cfg(feature = "profile")]
            let (mut over, mut moved) = (0u64, 0u64);
            for k in 0..sc.fall.len() {
                let (i, y) = (sc.fall[k], sc.rows[k]);
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
                let want = excess.min(pore_flux_m3(m, &c, DT));
                let _drained = if y == 0 {
                    // Sitting on the foundation: what drains joins the aquifer.
                    let lost = take_pore(w, i, want);
                    w.aquifer_m3 += lost;
                    lost
                } else {
                    let below = i - plane;
                    match w.material[below] {
                        Material::Bedrock => {
                            let lost = take_pore(w, i, want);
                            w.aquifer_m3 += lost;
                            lost
                        }
                        Material::Air => transfer(w, (i, Store::Pore), (below, Store::Free), want),
                        _ => transfer(w, (i, Store::Pore), (below, Store::Pore), want),
                    }
                };
                #[cfg(feature = "profile")]
                {
                    over += 1;
                    if _drained > 0.0 {
                        moved += 1;
                    }
                }
            }
            #[cfg(feature = "profile")]
            {
                crate::profile::add(crate::profile::Count::DrainOver, over);
                crate::profile::add(crate::profile::Count::DrainMoved, moved);
            }
        });
    });
    #[cfg(feature = "profile")]
    crate::profile::census::after(w, census);
}

/// What comes out of the spring cell: the aquifer's own head-driven seep, and — under the
/// closed cycle — the [`reentry`] stream on top of it.
///
/// One phase, not two, so `cubarium-voxel-sim`'s schedule stays the same chain of public
/// phases as [`step`] without having to learn a new one. They are the same cell and the
/// same emergence rule; only the store they draw on differs.
pub fn spring(w: &mut World) {
    #[cfg(feature = "profile")]
    let census = crate::profile::census::before(w, crate::profile::census::Tag::Spring);
    crate::voxel_phase!(Spring, {
        aquifer_seep(w);
        reentry(w);
    });
    #[cfg(feature = "profile")]
    crate::profile::census::after(w, census);
}

/// The head-driven part: water the aquifer pushes up while its table stands over the cell.
fn aquifer_seep(w: &mut World) {
    let Some((x, y, z)) = w.spring_cell else {
        return;
    };
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
    let got = emerge(w, (x, y, z), want);
    w.aquifer_m3 -= got;
}

/// Water pushed out at a named cell, and what it actually took.
///
/// It emerges at the cell and, if that is already brim full, in the cells above it: a
/// spring under standing water still reaches the surface. A **solid roof** over the seep
/// stops it there, and whatever could not be pushed out is not withdrawn — the caller
/// subtracts only the return value, so a blocked spring keeps its water in its store.
fn emerge(w: &mut World, (x, y, z): (u32, u32, u32), want: f64) -> f64 {
    let mut left = want;
    for at in y..w.config.height {
        let i = w.config.index(x as i64, at, z);
        if w.material[i].is_solid() {
            break;
        }
        let got = add_free(w, i, left);
        left -= got;
        if left <= 1e-15 {
            break;
        }
    }
    want - left
}

/// **River re-entry**: the closed cycle's second return path.
///
/// Route B lifts water into one lumped store and rains it back as showers. That is a
/// whole ring's weather arriving as weather, and it leaves the top of a tiered landscape
/// with nothing running through it. So the store also feeds a **stream at the spring
/// cell** — Wrysk, 2026-09-21: "water features on other levels, waterfalls possibly even
/// (works well if the only outlet is on the bottom most lake)". The ring is a slice of a
/// wider world: the river that leaves at the bottom lake re-enters at the top.
///
/// It draws [`crate::Config::reentry_m3_per_s`] per second while the store stands **above
/// the shower floor**, and never below it, so the stream can never eat the rain: the
/// drought lock stays exactly where [`shower`] put it. Booked as
/// [`crate::Ledger::reentry_in`] and `atmosphere_out`, so the conservation identity holds
/// term for term. The aquifer-driven [`spring`] is untouched and adds to it. Nothing here
/// is rain, so `is_raining` does not see it. The open budget has no store and is ignored.
pub fn reentry(w: &mut World) {
    {
        if !w.config.closed_water_budget || w.config.reentry_m3_per_s <= 0.0 {
            return;
        }
        let Some((x, y, z)) = w.spring_cell else {
            return;
        };
        if y >= w.config.height || z >= w.config.depth {
            return;
        }
        let floor = w.config.shower_trigger_fraction * w.ledger.expected_total();
        let spare = w.atmosphere_m3 - floor;
        if spare <= 0.0 {
            return;
        }
        let want = (w.config.reentry_m3_per_s * DT).min(spare);
        let got = emerge(w, (x, y, z), want);
        if got > 0.0 {
            w.atmosphere_m3 -= got;
            w.ledger.atmosphere_out += got;
            w.ledger.reentry_in += got;
        }
    }
}

/// Fraction of a brim-full sill cell a weir passes in one tick, falling off as the head
/// over the crest to the power of one and a half.
///
/// A full cell empties in about twenty ticks; a cell a tenth full passes a sixth of one
/// per cent of itself, a trickle rather than a siphon. Scale-free on purpose: it is a share
/// of the cell, so it means the same on a 0.125 m ring and a 0.25 m one.
const OUTLET_WEIR: f64 = 0.05;

/// Whether `CUBARIUM_OUTLET_TRACE` asked the outlet to say what it takes and what stands
/// around it. Read once: this sits in the tick.
///
/// The outlet is the hardest phase to watch from outside, because it runs **last** and
/// leaves its cell empty — 4000 ticks of the panel's lake draining looked, from any
/// sample taken between ticks, like a sill that never held a drop (T5). This is how T6
/// found out otherwise.
fn outlet_trace() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("CUBARIUM_OUTLET_TRACE").is_some())
}

pub fn outlet(w: &mut World) {
    // Measurement only; an early return skips `after` before anything has moved.
    #[cfg(feature = "profile")]
    let census = crate::profile::census::before(w, crate::profile::census::Tag::Outlet);
    crate::voxel_phase!(Outlet, {
        if !w.outlet_open {
            return;
        }
        let Some((x, y, z)) = w.outlet_cell else {
            return;
        };
        if y >= w.config.height || z >= w.config.depth {
            return;
        }
        let i = w.config.index(x as i64, y, z);
        let want = w.config.outlet_m3_per_s * DT;
        if outlet_trace() && w.free[i] > 0.0 {
            let c = &w.config;
            let plane = c.width as usize * c.depth as usize;
            let at = |j: usize| w.free.get(j).copied().unwrap_or(0.0);
            let solid_below = i >= plane && w.material[i - plane].is_solid();
            eprintln!(
                "TRACE tick {} sill ({x},{y},{z}) free {:.5} floor_solid {solid_below} | \
                 below {:.5} above {:.5} x- {:.5} x+ {:.5} z+ {:.5}",
                w.tick,
                w.free[i],
                if i >= plane { at(i - plane) } else { 0.0 },
                at(i + plane),
                at(c.index(x as i64 - 1, y, z)),
                at(c.index(x as i64 + 1, y, z)),
                if z + 1 < c.depth {
                    at(c.index(x as i64, y, z + 1))
                } else {
                    0.0
                },
            );
        }
        // **A weir, not a straw.** `outlet_m3_per_s` is a hard 0.05, which on `small` is
        // 1.28 voxels a tick: once any water reaches the sill it empties its own cell and
        // whatever the exchange levels back into it, tick after tick, and the lake goes
        // with it. Measured on the panel's seed: 0.687 m³ exported against 0.012 m³ of
        // stream, and with the stream off — no disturbance to reach the sill at all —
        // nothing left and the lake held 99 % (T6).
        //
        // So the sill passes what a weir passes: a discharge that falls away with the
        // head over it, `free^1.5`, which is a trickle for a film and a real flow for a
        // brim-full cell. `outlet_m3_per_s` stays as the ceiling, so nothing is ever
        // faster than it was. A sill the water does not reach still exports nothing,
        // which is the whole contract.
        let head = w.free[i].clamp(0.0, 1.0);
        let weir = OUTLET_WEIR * head * head.sqrt() * w.config.voxel_volume();
        let lost = take_free(w, i, want.min(weir));
        w.ledger.outlet_out += lost;
        release(w, lost);
    });
    #[cfg(feature = "profile")]
    crate::profile::census::after(w, census);
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
            #[cfg(feature = "profile")]
            let census = crate::profile::census::before(world, crate::profile::census::Tag::Rain);
            let got = rain_pulse(world, volume_m3);
            #[cfg(feature = "profile")]
            crate::profile::census::after(world, census);
            got
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
            release(world, got);
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
        Command::AddAtmosphere { volume_m3 } => {
            if !volume_m3.is_finite() || volume_m3 <= 0.0 || !world.config.closed_water_budget {
                return 0.0;
            }
            world.atmosphere_m3 += volume_m3;
            world.ledger.atmosphere_in += volume_m3;
            world.ledger.user_atmosphere_in += volume_m3;
            volume_m3
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
    let targets = sky_cells(w);
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

    let kept = if material.is_solid() {
        add_pore(w, i, water)
    } else {
        add_free(w, i, water)
    };
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

/// Tiny function fixtures for [`fall`] alone: the sparse snapshot must move water exactly
/// as the wet-column walk did, one cell per call, and keep the wet set honest. In-module so
/// the private `wet` set can be checked directly
/// (`design/handoffs/voxel-sparse-fall-2026-09-18.md`).
#[cfg(test)]
mod fall_tests {
    use super::fall;
    use crate::{Command, Config, Material, World};

    /// A one-column world of air over the default bedrock floor, so the floor itself is
    /// the solid boundary unless a fixture clears it.
    fn fixture(height: u32) -> World {
        World::empty(Config {
            width: 1,
            height,
            depth: 1,
            voxel_m: 1.0,
            seed: 7,
            ..Config::default()
        })
    }

    fn add(w: &mut World, x: i64, y: u32, volume_m3: f64) -> f64 {
        w.apply(Command::AddWater {
            x,
            y,
            z: 0,
            volume_m3,
        })
    }

    fn free_at(w: &World, x: i64, y: u32) -> f64 {
        w.view().free_at(x, y, 0)
    }

    fn residual(w: &World) -> f64 {
        w.view().stored_m3() - w.view().ledger.expected_stored()
    }

    /// The wet set is exactly the cells the arrays say hold free water: no stale member
    /// left by a transfer that emptied a cell, none missing after one filled it.
    fn assert_wet_set_is_true(w: &World) {
        let n = w.config.cells();
        let mut member = vec![false; n];
        for &i in w.wet.cells() {
            member[i] = true;
        }
        for (i, (&member, &free)) in member.iter().zip(w.free.iter()).enumerate() {
            assert_eq!(
                member,
                free > 0.0,
                "wet-set membership disagrees with free water at cell {i} (free {free})"
            );
        }
    }

    /// A falling droplet: one call moves it exactly one cell, not to the floor.
    #[test]
    fn fall_moves_a_droplet_one_cell() {
        let mut w = fixture(5);
        add(&mut w, 0, 3, 1.0);
        fall(&mut w);
        assert_eq!(free_at(&w, 0, 2), 1.0);
        assert_eq!(free_at(&w, 0, 1), 0.0, "the arrival fell a second cell");
        assert!(residual(&w).abs() < 1e-12);
        assert_wet_set_is_true(&w);
    }

    /// A stacked wet column shifts down by one cell, which needs the bottom-up visit:
    /// each cell meets the space the cell below just vacated.
    #[test]
    fn fall_shifts_a_stacked_column_one_cell() {
        let mut w = fixture(6);
        // Clear the bedrock floor so the column has somewhere to go.
        w.apply(Command::SetMaterial {
            x: 0,
            y: 0,
            z: 0,
            material: Material::Air,
        });
        for y in 1..=3 {
            add(&mut w, 0, y, 1.0);
        }
        fall(&mut w);
        for y in 0..=2 {
            assert_eq!(free_at(&w, 0, y), 1.0, "floor of the shift at {y}");
        }
        for y in 3..6 {
            assert_eq!(free_at(&w, 0, y), 0.0, "left behind at {y}");
        }
        assert!(residual(&w).abs() < 1e-12);
        assert_wet_set_is_true(&w);
    }

    /// A partly full receiver on the solid floor takes only its room; the donor keeps the
    /// rest and does not overfill it.
    #[test]
    fn fall_fills_a_partly_full_receiver_only_to_its_room() {
        let mut w = fixture(5);
        add(&mut w, 0, 1, 0.5); // receiver on the bedrock floor, half full
        add(&mut w, 0, 2, 1.0); // donor directly above
        fall(&mut w);
        assert!((free_at(&w, 0, 1) - 1.0).abs() < 1e-12, "receiver");
        assert!((free_at(&w, 0, 2) - 0.5).abs() < 1e-12, "donor");
        assert!(free_at(&w, 0, 0) == 0.0, "water entered the bedrock");
        assert!(residual(&w).abs() < 1e-12);
        assert_wet_set_is_true(&w);
    }

    /// A solid floor refuses the transfer and the water stays put.
    #[test]
    fn fall_stops_on_a_solid_floor() {
        let mut w = fixture(4);
        add(&mut w, 0, 1, 1.0);
        fall(&mut w);
        assert_eq!(free_at(&w, 0, 1), 1.0);
        assert_eq!(free_at(&w, 0, 0), 0.0);
        assert!(residual(&w).abs() < 1e-12);
        assert_wet_set_is_true(&w);
    }

    /// The bottom row is skipped rather than indexed below the world: a wet cell at
    /// `y = 0` is untouched and no subtraction underflows.
    #[test]
    fn fall_skips_the_bottom_row() {
        let mut w = fixture(4);
        w.apply(Command::SetMaterial {
            x: 0,
            y: 0,
            z: 0,
            material: Material::Air,
        });
        add(&mut w, 0, 0, 1.0);
        fall(&mut w);
        assert_eq!(free_at(&w, 0, 0), 1.0);
        assert!(residual(&w).abs() < 1e-12);
        assert_wet_set_is_true(&w);
    }

    /// An empty world falls through with nothing to do.
    #[test]
    fn fall_over_an_empty_world_does_nothing() {
        let mut w = fixture(4);
        fall(&mut w);
        assert!(w.wet.cells().is_empty());
        assert!(residual(&w).abs() < 1e-12);
    }

    /// A densely wet world: several columns, partial cells at every level, repeated
    /// calls. Conservation and wet-set membership hold every call.
    #[test]
    fn fall_keeps_conservation_and_the_wet_set_in_a_dense_world() {
        let mut w = World::empty(Config {
            width: 8,
            height: 8,
            depth: 1,
            voxel_m: 1.0,
            seed: 7,
            ..Config::default()
        });
        for x in 0..8 {
            for y in 1..7 {
                // A varying partial fill so cells have room and the order matters.
                add(&mut w, x, y, 0.4 + 0.05 * ((x + y as i64) % 4) as f64);
            }
        }
        assert_wet_set_is_true(&w);
        let before = w.view().stored_m3();
        for _ in 0..6 {
            fall(&mut w);
            assert_wet_set_is_true(&w);
            assert!((w.view().stored_m3() - before).abs() < 1e-9);
            assert!(residual(&w).abs() < 1e-9);
        }
        // Water reached the floor everywhere it could fall to.
        for x in 0..8 {
            assert!(free_at(&w, x, 1) > 0.0, "column {x} floor is dry");
        }
    }
}

/// The void-run geometry cache (`design/handoffs/voxel-exchange-geometry-2026-09-18.md`):
/// the cached scan must compute exactly what the old material walk did, the cache must
/// rebuild on a terrain edit and not on a water change, and it must belong to the world
/// rather than to the thread.
#[cfg(test)]
mod exchange_geometry_tests {
    use super::{
        MASK_ROWS, ROOM_EPS, SCRATCH, Scratch, exchange, exchange_inner_with_masks,
        offer_up_the_run, offer_up_the_run_mask, scan_column, scan_column_mask,
    };
    use crate::{Command, Config, Material, World};

    fn cfg(width: u32, height: u32) -> Config {
        Config {
            width,
            height,
            depth: 1,
            voxel_m: 1.0,
            seed: 7,
            ..Config::default()
        }
    }

    /// Three columns of different shape: an open run, a column split by a roof, and a
    /// column with a floor and a roof; water in an open run, a lower cavity and an upper
    /// run.
    fn varied_fixture() -> World {
        let mut w = World::empty(cfg(3, 6));
        w.apply(Command::SetMaterial {
            x: 1,
            y: 3,
            z: 0,
            material: Material::Rock,
        });
        w.apply(Command::SetMaterial {
            x: 2,
            y: 1,
            z: 0,
            material: Material::Rock,
        });
        w.apply(Command::SetMaterial {
            x: 2,
            y: 4,
            z: 0,
            material: Material::Rock,
        });
        for (x, y, volume_m3) in [(0, 1, 1.0), (1, 1, 0.5), (2, 5, 0.3)] {
            w.apply(Command::AddWater {
                x,
                y,
                z: 0,
                volume_m3,
            });
        }
        w
    }

    fn free_at(w: &World, x: i64, y: u32) -> f64 {
        w.view().free_at(x, y, 0)
    }

    fn residual(w: &World) -> f64 {
        w.view().stored_m3() - w.view().ledger.expected_stored()
    }

    /// The scan as it was before the cache: rediscover the runs from `material` every
    /// call, then write the same three arrays.
    fn uncached_scan(
        material: &[Material],
        free: &[f64],
        height: usize,
        plane: usize,
        col: usize,
    ) -> (Vec<f64>, Vec<usize>, Vec<usize>) {
        let mut head = vec![0.0; height];
        let mut room_target = vec![usize::MAX; height];
        let mut run_top = vec![usize::MAX; height];
        let mut y = 0usize;
        while y < height {
            if material[y * plane + col].is_solid() {
                y += 1;
                continue;
            }
            let mut top = y;
            while top + 1 < height && !material[(top + 1) * plane + col].is_solid() {
                top += 1;
            }
            let mut best = usize::MAX;
            let top_cell = top * plane + col;
            for k in (y..=top).rev() {
                let i = k * plane + col;
                if free[i] < 1.0 - ROOM_EPS {
                    best = i;
                }
                room_target[k] = best;
                run_top[k] = top_cell;
            }
            let mut k = y;
            while k <= top {
                let i = k * plane + col;
                if free[i] > 0.0 {
                    let mut t = k;
                    while t + 1 <= top && free[(t + 1) * plane + col] > 0.0 {
                        t += 1;
                    }
                    // Package H: from the top down, a full cell carries the head above
                    // it and a partial cell its own level.
                    let mut surface = t as f64 + free[t * plane + col];
                    head[t] = surface;
                    for m in (k..t).rev() {
                        let f = free[m * plane + col];
                        if f < 1.0 - ROOM_EPS {
                            surface = m as f64 + f;
                        }
                        head[m] = surface;
                    }
                    k = t + 1;
                } else {
                    k += 1;
                }
            }
            y = top + 1;
        }
        (head, room_target, run_top)
    }

    fn cached_scan(w: &World, col: usize) -> (Vec<f64>, Vec<usize>, Vec<usize>) {
        let plane = w.config.width as usize * w.config.depth as usize;
        let height = w.config.height as usize;
        let n = plane * height;
        let offset = &w.void_runs.offset;
        let runs = &w.void_runs.runs;
        let mut head = vec![0.0; n];
        let mut room_target = vec![u32::MAX; n];
        let mut run_top = vec![u32::MAX; n];
        scan_column(
            &w.free,
            plane,
            col,
            &runs[offset[col] as usize..offset[col + 1] as usize],
            &mut head,
            &mut room_target,
            &mut run_top,
        );
        // The scan writes the column at its world indices; read that column back out.
        let widen = |v: u32| {
            if v == u32::MAX {
                usize::MAX
            } else {
                v as usize
            }
        };
        let at = |y: usize| y * plane + col;
        (
            (0..height).map(|y| head[at(y)]).collect(),
            (0..height).map(|y| widen(room_target[at(y)])).collect(),
            (0..height).map(|y| widen(run_top[at(y)])).collect(),
        )
    }

    /// The cached scan is the uncached walk to the bit on a fixture with stacked cavities.
    #[test]
    fn cached_scan_matches_the_uncached_walk() {
        let mut w = varied_fixture();
        w.ensure_void_runs();
        let plane = w.config.width as usize * w.config.depth as usize;
        let height = w.config.height as usize;
        for col in 0..plane {
            let (head, room_target, run_top) = cached_scan(&w, col);
            let (rh, rr, rt) = uncached_scan(&w.material, &w.free, height, plane, col);
            assert_eq!(head, rh, "head, column {col}");
            assert_eq!(room_target, rr, "room_target, column {col}");
            assert_eq!(run_top, rt, "run_top, column {col}");
        }
    }

    /// A water change with no terrain edit must move the recomputed heads: only the void
    /// runs are cached, not the wet runs.
    #[test]
    fn water_changes_recompute_heads_without_a_terrain_edit() {
        let mut w = varied_fixture();
        w.ensure_void_runs();
        let version = w.terrain_version();
        let before = cached_scan(&w, 1).0[1];
        // Fill the lower cavity's last half-cell: its one-cell water run goes full.
        w.apply(Command::AddWater {
            x: 1,
            y: 1,
            z: 0,
            volume_m3: 0.5,
        });
        assert_eq!(
            w.terrain_version(),
            version,
            "adding water moved the terrain version"
        );
        let after = cached_scan(&w, 1).0[1];
        assert!(after > before, "head stayed {before} after the water rose");
    }

    /// Opening a roofed passage rebuilds the geometry, and the exchange then moves water
    /// through it; before the edit the roof holds the water up.
    #[test]
    fn a_terrain_edit_rebuilds_the_geometry_and_opens_the_passage() {
        let mut w = World::empty(cfg(1, 4));
        w.apply(Command::SetMaterial {
            x: 0,
            y: 2,
            z: 0,
            material: Material::Rock,
        });
        w.apply(Command::AddWater {
            x: 0,
            y: 3,
            z: 0,
            volume_m3: 1.0,
        });
        exchange(&mut w, 1);
        assert_eq!(free_at(&w, 0, 3), 1.0, "the roof did not hold the water");
        assert_eq!(free_at(&w, 0, 2), 0.0, "water crossed the solid roof");

        let before = w.terrain_version();
        w.apply(Command::SetMaterial {
            x: 0,
            y: 2,
            z: 0,
            material: Material::Air,
        });
        assert!(
            w.terrain_version() > before,
            "the edit did not bump the version"
        );
        exchange(&mut w, 1);
        assert!(
            free_at(&w, 0, 2) > 0.0,
            "the rebuilt geometry missed the new void"
        );
        assert_eq!(
            w.void_runs.runs.len(),
            1,
            "the roof still splits the column"
        );
        assert!(residual(&w).abs() < 1e-12);

        // Closing it again rebuilds too: the one run becomes two.
        w.apply(Command::SetMaterial {
            x: 0,
            y: 2,
            z: 0,
            material: Material::Rock,
        });
        w.ensure_void_runs();
        assert_eq!(
            w.void_runs.runs.len(),
            2,
            "closing the passage did not rebuild the geometry"
        );
    }

    /// Two worlds of the same size and terrain version have different floors. The cache
    /// belongs to the world, so neither borrows the other's runs (`SCRATCH`'s dimensions
    /// and version are not a valid key).
    #[test]
    fn two_worlds_with_one_terrain_version_do_not_share_geometry() {
        let mut a = World::empty(cfg(2, 5));
        let mut b = World::empty(cfg(2, 5));
        a.apply(Command::SetMaterial {
            x: 0,
            y: 2,
            z: 0,
            material: Material::Rock,
        });
        b.apply(Command::SetMaterial {
            x: 1,
            y: 2,
            z: 0,
            material: Material::Rock,
        });
        assert_eq!(a.terrain_version(), b.terrain_version());
        a.ensure_void_runs();
        b.ensure_void_runs();
        assert_ne!(
            a.void_runs.runs, b.void_runs.runs,
            "the two floors share a cache"
        );
        for w in [&mut a, &mut b] {
            w.apply(Command::AddWater {
                x: 0,
                y: 3,
                z: 0,
                volume_m3: 1.0,
            });
        }
        exchange(&mut a, 1);
        exchange(&mut b, 1);
        assert_ne!(
            a.free, b.free,
            "one world's water answered to the other's floor"
        );
    }

    /// A clone keeps the built geometry and behaves identically; a snapshot round trip
    /// comes back without it and rebuilds it to the same runs.
    #[test]
    fn clone_reuses_geometry_and_load_rebuilds_it() {
        let mut w = varied_fixture();
        w.ensure_void_runs();
        let snapshot = w.save();

        let mut cloned = w.clone();
        assert_eq!(
            cloned.void_runs.runs, w.void_runs.runs,
            "the clone dropped it"
        );

        let mut loaded = World::load(&snapshot).expect("the world round-trips");
        loaded.ensure_void_runs();
        assert_eq!(
            loaded.void_runs.runs, w.void_runs.runs,
            "the decoded world rebuilt different geometry"
        );

        for _ in 0..3 {
            exchange(&mut w, 1);
            exchange(&mut cloned, 1);
            exchange(&mut loaded, 1);
        }
        assert_eq!(w.free, cloned.free, "the clone diverged");
        assert_eq!(w.free, loaded.free, "the loaded world diverged");
        assert!(residual(&w).abs() < 1e-12);
    }

    fn mask_fixture(height: u32) -> World {
        let mut w = World::empty(cfg(4, height));
        for (x, y) in [(0, 17), (1, 9), (1, 31), (2, 22), (3, height - 2)] {
            w.apply(Command::SetMaterial {
                x,
                y,
                z: 0,
                material: Material::Rock,
            });
        }
        for (x, y, amount) in [
            (0, 1, 1.0),
            (0, 2, 1.0),
            (0, 3, 0.8),
            (1, 1, 0.5),
            // Effectively full: initial room lookup skips it, but the continuation from
            // the real room below must still accept its positive sliver.
            (1, 2, 1.0 - ROOM_EPS / 2.0),
            (1, 3, 0.4),
            (2, 23, 0.75),
            (2, 24, 1.0),
            (3, height - 1, 0.3),
        ] {
            w.apply(Command::AddWater {
                x,
                y,
                z: 0,
                volume_m3: amount,
            });
        }
        w
    }

    /// The word path preserves every floating-point operation and active-set edit across
    /// separated cavities, the bit-63 boundary, terrain-cache rebuilds and snapshot loads.
    #[test]
    fn mask_exchange_matches_the_reference_trajectory_at_height_64() {
        let mut fast = mask_fixture(64);
        let mut reference = fast.clone();
        for step in 0..200 {
            if step == 40 || step == 120 {
                let material = if step == 40 {
                    Material::Rock
                } else {
                    Material::Air
                };
                for w in [&mut fast, &mut reference] {
                    w.apply(Command::SetMaterial {
                        x: 2,
                        y: 30,
                        z: 0,
                        material,
                    });
                }
            }
            if step == 80 {
                fast = World::load(&fast.save()).expect("fast snapshot reloads");
                reference = World::load(&reference.save()).expect("reference snapshot reloads");
                assert!(!fast.wet.cells().is_empty(), "snapshot lost its wet set");
            }
            exchange_inner_with_masks(&mut fast, 1, true);
            exchange_inner_with_masks(&mut reference, 1, false);
            assert_eq!(
                fast.free, reference.free,
                "free water diverged at step {step}"
            );
            assert_eq!(fast.wet, reference.wet, "wet set diverged at step {step}");
        }
    }

    /// A world `depth` 3 deep and `height` tall, with water and roofs on both sides of
    /// row 64 — where the old one-word mask ended — and runs that cross it, so the wide
    /// mask's upper half, the `z` faces and the displacement walk all carry water.
    fn tall_fixture(height: u32) -> World {
        let mut w = World::empty(Config {
            depth: 3,
            ..cfg(5, height)
        });
        let rock = |w: &mut World, x: i64, y: u32, z: u32| {
            w.apply(Command::SetMaterial {
                x,
                y,
                z,
                material: Material::Rock,
            });
        };
        // A floor at y = 60 under every column but one, so water pools on both sides of
        // the word boundary and drains through the gap at (4, 1).
        for z in 0..3 {
            for x in 0..5 {
                if (x, z) != (4, 1) {
                    rock(&mut w, x, 60, z);
                }
            }
        }
        // A roof over one column just above the boundary and a pillar under the top row.
        rock(&mut w, 1, 66, 1);
        rock(&mut w, 3, height - 2, 2);
        rock(&mut w, 0, 17, 0);
        for (x, y, z, amount) in [
            // A run crossing rows 63 and 64, full below and partial on top.
            (0, 61, 0, 1.0),
            (0, 62, 0, 1.0),
            (0, 63, 0, 1.0),
            (0, 64, 0, 1.0),
            (0, 65, 0, 0.6),
            // Trapped under the roof, full, so a push against it displaces.
            (1, 61, 1, 1.0),
            (1, 62, 1, 1.0),
            (1, 63, 1, 1.0),
            (1, 64, 1, 1.0),
            (1, 65, 1, 1.0 - ROOM_EPS / 2.0),
            (2, 64, 2, 0.4),
            (3, height - 1, 2, 0.3),
            (4, 70, 1, 0.9),
            (2, 3, 0, 0.5),
        ] {
            w.apply(Command::AddWater {
                x,
                y,
                z,
                volume_m3: amount,
            });
        }
        w
    }

    /// The mask path and the dense walk step the same trajectory, bit for bit, on a world
    /// taller than one 64-bit word: the panel's 72 rows, and a full 128-row word. The two
    /// paths share every floating-point operation, so any difference is a lookup bug.
    #[test]
    fn mask_exchange_matches_the_reference_trajectory_at_heights_72_and_128() {
        for height in [72, 128] {
            let mut fast = tall_fixture(height);
            let mut reference = fast.clone();
            for step in 0..120 {
                if step == 30 || step == 80 {
                    // Open the roof, then close it again: both paths rebuild their geometry.
                    let material = if step == 30 {
                        Material::Air
                    } else {
                        Material::Rock
                    };
                    for w in [&mut fast, &mut reference] {
                        w.apply(Command::SetMaterial {
                            x: 1,
                            y: 66,
                            z: 1,
                            material,
                        });
                    }
                }
                exchange_inner_with_masks(&mut fast, 1, true);
                exchange_inner_with_masks(&mut reference, 1, false);
                assert_eq!(
                    fast.free, reference.free,
                    "free water diverged at height {height}, step {step}"
                );
                assert_eq!(fast.wet, reference.wet, "wet set diverged at step {step}");
            }
            assert!(
                fast.free[..]
                    .iter()
                    .enumerate()
                    .any(|(i, &f)| f > 0.0 && i / (fast.config.width as usize * 3) >= 64),
                "no water above row 64 at height {height}: the upper half went unexercised"
            );
        }
    }

    /// A world taller than the widest mask keeps the dense scan, and that is what the
    /// selector picks: same trajectory as the dense path, and no masks built.
    #[test]
    fn a_world_above_the_mask_width_uses_the_dense_fallback() {
        let height = MASK_ROWS as u32 + 1;
        let mut selected = tall_fixture(height);
        let mut reference = selected.clone();
        for step in 0..40 {
            exchange(&mut selected, 1);
            exchange_inner_with_masks(&mut reference, 1, false);
            assert_eq!(
                selected.free, reference.free,
                "fallback diverged at step {step}"
            );
        }
        selected.ensure_void_runs();
        assert!(selected.void_runs.mask.is_empty());
    }

    #[test]
    fn masks_cover_one_row_and_a_full_word() {
        for height in [1, 64, MASK_ROWS as u32] {
            let mut w = World::empty(cfg(1, height));
            w.apply(Command::SetMaterial {
                x: 0,
                y: 0,
                z: 0,
                material: Material::Air,
            });
            for y in 0..height {
                w.apply(Command::AddWater {
                    x: 0,
                    y,
                    z: 0,
                    volume_m3: if y + 1 == height { 0.25 } else { 1.0 },
                });
            }
            w.ensure_void_runs();
            let expected_mask = if height as usize == MASK_ROWS {
                u128::MAX
            } else {
                (1u128 << height) - 1
            };
            assert_eq!(w.void_runs.mask, [expected_mask]);
            let mut head = vec![0.0; height as usize];
            // Every row full but the top one.
            let full = expected_mask & !(1u128 << (height - 1));
            scan_column_mask(&w.free, 1, 0, expected_mask, full, &mut head);
            assert!(
                head.iter().all(|&h| h == f64::from(height - 1) + 0.25),
                "wrong head at height {height}: {head:?}"
            );
            if height >= 64 {
                let last = height as usize - 1;
                w.free.fill(1.0);
                for y in [0, 63, last] {
                    assert_offer_paths_match(&w, y, 0.75);
                }
                w.free[0] = 0.5;
                assert_offer_paths_match(&w, 0, 0.75);
                w.free[0] = 1.0;
                w.free[last] = 0.5;
                assert_offer_paths_match(&w, 0, 0.75);
                assert_offer_paths_match(&w, 63, 0.75);
                assert_offer_paths_match(&w, last, 0.75);
            }
        }
    }

    /// One column, `cfg(1, 8)`, with `fills` poured from row 1 up and rock at `roof`:
    /// its heads by the mask scan and by the dense walk, for the wet rows only.
    fn heads_both_ways(fills: &[f64], roof: Option<u32>) -> (Vec<f64>, Vec<f64>) {
        let mut w = World::empty(cfg(1, 8));
        if let Some(y) = roof {
            w.apply(Command::SetMaterial {
                x: 0,
                y,
                z: 0,
                material: Material::Rock,
            });
        }
        for (k, &volume_m3) in fills.iter().enumerate() {
            w.apply(Command::AddWater {
                x: 0,
                y: 1 + k as u32,
                z: 0,
                volume_m3,
            });
        }
        w.ensure_void_runs();
        // One column, so a world index is a row. The masks as the exchange builds them.
        let (mut wet, mut full) = (0u128, 0u128);
        for (y, &f) in w.free.iter().enumerate() {
            if f > 0.0 {
                wet |= 1 << y;
                if f >= 1.0 - ROOM_EPS {
                    full |= 1 << y;
                }
            }
        }
        let mut masked = vec![0.0; w.free.len()];
        scan_column_mask(&w.free, 1, 0, wet, full, &mut masked);
        let dense = cached_scan(&w, 0).0;
        let rows = 1..=fills.len();
        (masked[rows.clone()].to_vec(), dense[rows].to_vec())
    }

    /// **Head from the packed full stack** (package H). A wet cell's head is the surface
    /// of the full stack it sits at the foot of — `s + free[s]` for the first row `s` at
    /// or above it that is not full — so a partial cell is its own level and a full
    /// stack takes the partial cell capping it. A stack capped by a roof (or a dry cell,
    /// or the world top) keeps its top cell's own surface, the old answer. Masks and the
    /// dense walk agree on every column.
    #[test]
    fn a_wet_cell_takes_the_head_of_the_full_stack_it_is_under() {
        for (fills, roof, expected) in [
            // Hollow: films under a half cell. Each is its own level.
            (
                vec![1e-10, 1e-10, 0.5],
                None,
                vec![1.0 + 1e-10, 2.0 + 1e-10, 3.5],
            ),
            // Packed: the old run-top answer, unchanged.
            (vec![1.0, 1.0, 0.5], None, vec![3.5, 3.5, 3.5]),
            // Full to a roof at row 4: the roof row.
            (vec![1.0, 1.0, 1.0], Some(4), vec![4.0, 4.0, 4.0]),
            // Unpacked: a partial cell under a full stack capped by a partial one.
            (vec![0.3, 1.0, 1.0, 0.2], None, vec![1.3, 4.2, 4.2, 4.2]),
        ] {
            let (masked, dense) = heads_both_ways(&fills, roof);
            assert_eq!(masked, expected, "mask scan of {fills:?}");
            assert_eq!(dense, expected, "dense walk of {fills:?}");
        }
    }

    /// The defect the census found (382ef3a): the bottom film of a hollow stack used to
    /// push with the stack top's head and displace slivers up a dry neighbour's column.
    /// Now it offers only across its own row. Read off the proposals the exchange made.
    #[test]
    fn a_hollow_stack_does_not_push_its_bottom_film_up_a_dry_neighbour() {
        // Column 0 holds the stack, column 1 is dry, column 2 is rock to the top so the
        // stack has exactly one horizontal neighbour.
        let mut w = World::empty(cfg(3, 8));
        for y in 1..8 {
            w.apply(Command::SetMaterial {
                x: 2,
                y,
                z: 0,
                material: Material::Rock,
            });
        }
        // Films deeper than the spreading depth (package C), so the bottom one still
        // offers sideways and the test is about its head.
        for (y, volume_m3) in [(1, 1e-3), (2, 1e-3), (3, 0.5)] {
            w.apply(Command::AddWater {
                x: 0,
                y,
                z: 0,
                volume_m3,
            });
        }
        let plane = 3;
        let bottom = plane; // (0, 1)
        exchange(&mut w, 1);
        let from_bottom: Vec<(u32, u32, f64)> = SCRATCH.with(|s| {
            s.borrow()
                .edges
                .iter()
                .copied()
                .filter(|&(from, _, _)| from as usize == bottom)
                .collect()
        });
        assert!(
            from_bottom
                .iter()
                .any(|&(_, to, _)| to as usize == bottom + 1),
            "the bottom film offered nothing across its own row: {from_bottom:?}"
        );
        let raised: Vec<_> = from_bottom
            .iter()
            .filter(|&&(_, to, _)| to as usize >= 2 * plane)
            .collect();
        assert!(
            raised.is_empty(),
            "the bottom film pushed up the neighbour's column: {raised:?}"
        );
        assert!(residual(&w).abs() < 1e-12);
    }

    fn assert_offer_paths_match(w: &World, y: usize, q: f64) {
        let height = w.config.height as usize;
        let mut reference = Scratch::default();
        reference.ensure(height, 1, 1, 1);
        scan_column(
            &w.free,
            1,
            0,
            &w.void_runs.runs,
            &mut reference.head,
            &mut reference.room_target,
            &mut reference.run_top,
        );
        // One column, so its world index is its row.
        let reference_placed = offer_up_the_run(w, 1, y, q, &mut reference);

        let mut masked = Scratch::default();
        masked.ensure(height, 1, 1, 1);
        for row in 0..height {
            if w.free[row] >= 1.0 - ROOM_EPS {
                masked.full_mask[0] |= 1u128 << row;
            }
        }
        let masked_placed = offer_up_the_run_mask(w, 1, 0, y, q, &mut masked);
        assert_eq!(masked_placed, reference_placed, "placed from y={y}");
        assert_eq!(masked.offers, reference.offers, "offers from y={y}");
    }
}
/// **Still water settles** (package P): what one column's stack sends across one face into
/// one neighbouring column is capped at what that face would send from a single row, so a
/// deep interface levels instead of swapping the two columns' levels every substep.
#[cfg(test)]
mod settle_tests {
    use super::{SCRATCH, exchange, fall};
    use crate::{Command, Config, Material, World};

    fn cfg(width: u32, height: u32) -> Config {
        Config {
            width,
            height,
            depth: 1,
            voxel_m: 1.0,
            seed: 7,
            ..Config::default()
        }
    }

    fn rock_column(w: &mut World, x: i64) {
        for y in 1..w.config.height {
            w.apply(Command::SetMaterial {
                x,
                y,
                z: 0,
                material: Material::Rock,
            });
        }
    }

    /// Pour `units` into column `x` from the floor up, as settled water.
    fn pour(w: &mut World, x: i64, mut units: f64) {
        for y in 1..w.config.height {
            if units <= 0.0 {
                break;
            }
            let got = w.apply(Command::AddWater {
                x,
                y,
                z: 0,
                volume_m3: units.min(1.0),
            });
            units -= got;
        }
    }

    fn column(w: &World, x: i64) -> f64 {
        (1..w.config.height)
            .map(|y| w.view().free_at(x, y, 0))
            .sum()
    }

    fn residual(w: &World) -> f64 {
        w.view().stored_m3() - w.view().ledger.expected_stored()
    }

    /// Two columns side by side on a bedrock floor, walled on their other sides, with
    /// `k` full rows each under a partial top: 0.8 on the left, 0.2 on the right. They
    /// face each other across `k + 1` wet rows. The level is `k + 0.5` above the floor.
    fn pair(k: u32) -> World {
        let mut w = World::empty(cfg(4, k + 4));
        rock_column(&mut w, 2);
        rock_column(&mut w, 3);
        pour(&mut w, 0, f64::from(k) + 0.8);
        pour(&mut w, 1, f64::from(k) + 0.2);
        w
    }

    /// The two-column cycle the C tests found: joined over one row or four, the high
    /// column only ever loses water, the heads never cross, and the pair levels and stops.
    #[test]
    fn two_columns_joined_over_several_rows_level_without_swapping() {
        for k in [1, 2, 4] {
            let mut w = pair(k);
            let mut last = column(&w, 0);
            let mut still = None;
            for substep in 0..400 {
                fall(&mut w);
                exchange(&mut w, 1);
                let (a, b) = (column(&w, 0), column(&w, 1));
                assert!(
                    a - b >= -1e-12,
                    "k = {k}: the heads crossed at substep {substep}: {a} against {b}"
                );
                let change = a - last;
                assert!(
                    change <= 1e-15,
                    "k = {k}: the high column gained {change} at substep {substep}"
                );
                if change.abs() < 1e-12 {
                    still.get_or_insert(substep);
                } else {
                    still = None;
                }
                last = a;
            }
            let (a, b) = (column(&w, 0), column(&w, 1));
            assert!((a - b).abs() < 1e-9, "k = {k}: not level: {a} against {b}");
            assert!(
                (a - (f64::from(k) + 0.5)).abs() < 1e-9,
                "k = {k}: level {a}, not {}",
                f64::from(k) + 0.5
            );
            let still = still.expect("still moving at the end");
            assert!(still < 100, "k = {k}: still only from substep {still}");
            assert!(residual(&w).abs() < 1e-12);
        }
    }

    /// A pool six columns wide, two rows deep with a heap in one column, and nothing
    /// forcing it: it spreads, levels, and then the exchange proposes nothing at all,
    /// substep after substep. The slowest mode of a six-column pool relaxes by about 6 %
    /// a substep, so the columns agree to the last bit after roughly 550 substeps (under
    /// half a minute of world time); before package P two full rows swapped forever.
    #[test]
    fn a_pool_at_rest_proposes_nothing() {
        let mut w = World::empty(cfg(8, 6));
        rock_column(&mut w, 6);
        rock_column(&mut w, 7);
        for x in 0..6 {
            pour(&mut w, x, if x == 0 { 3.5 } else { 2.0 });
        }
        let mut quiet_from = None;
        for substep in 0..900 {
            fall(&mut w);
            exchange(&mut w, 1);
            let edges = SCRATCH.with(|s| s.borrow().edges.len());
            if edges == 0 {
                quiet_from.get_or_insert(substep);
            } else {
                assert!(
                    substep < 700,
                    "{edges} proposals at substep {substep}, after the pool should be still"
                );
                quiet_from = None;
            }
        }
        assert!(
            quiet_from.is_some_and(|s| s < 700),
            "never still: {quiet_from:?}"
        );
        let level = column(&w, 0);
        for x in 1..6 {
            assert!((column(&w, x) - level).abs() < 1e-9, "column {x}");
        }
        assert!(residual(&w).abs() < 1e-12);
    }
}

/// **Minimum spreading depth** (package C): a giver whose own free water is shallower than
/// [`MIN_SPREAD_DEPTH_M`] makes no horizontal offer. Everything else it does is unchanged,
/// and nothing is deleted. The fixtures are 1 m cells, so the threshold in cell units is
/// the constant itself.
#[cfg(test)]
mod spread_tests {
    use super::{MIN_SPREAD_DEPTH_M, evaporate, exchange, fall, infiltrate};
    use crate::{Command, Config, DT, Material, World};

    fn cfg(width: u32, height: u32) -> Config {
        Config {
            width,
            height,
            depth: 1,
            voxel_m: 1.0,
            seed: 7,
            ..Config::default()
        }
    }

    /// The threshold in cell units on a 1 m fixture.
    fn threshold() -> f64 {
        MIN_SPREAD_DEPTH_M / 1.0
    }

    fn rock(w: &mut World, x: i64, y: u32, material: Material) {
        w.apply(Command::SetMaterial {
            x,
            y,
            z: 0,
            material,
        });
    }

    fn add(w: &mut World, x: i64, y: u32, volume_m3: f64) {
        w.apply(Command::AddWater {
            x,
            y,
            z: 0,
            volume_m3,
        });
    }

    fn free_at(w: &World, x: i64, y: u32) -> f64 {
        w.view().free_at(x, y, 0)
    }

    fn residual(w: &World) -> f64 {
        w.view().stored_m3() - w.view().ledger.expected_stored()
    }

    /// A film on the bedrock of column 0; column 1 is dry and column 2 is rock, so the
    /// film has exactly one horizontal neighbour and nothing below it to fall into.
    fn film_beside_a_dry_cell(film: f64) -> World {
        let mut w = World::empty(cfg(3, 4));
        for y in 1..4 {
            rock(&mut w, 2, y, Material::Rock);
        }
        add(&mut w, 0, 1, film);
        w
    }

    #[test]
    fn a_film_below_the_threshold_stays_beside_a_dry_cell() {
        let film = 0.5 * threshold();
        let mut w = film_beside_a_dry_cell(film);
        assert_eq!(free_at(&w, 0, 1), film);
        exchange(&mut w, 1);
        assert_eq!(free_at(&w, 0, 1), film, "the film moved");
        assert_eq!(free_at(&w, 1, 1), 0.0, "the dry neighbour got water");
    }

    #[test]
    fn a_film_just_above_the_threshold_spreads_as_before() {
        let film = 1.01 * threshold();
        let mut w = film_beside_a_dry_cell(film);
        exchange(&mut w, 1);
        assert!(free_at(&w, 1, 1) > 0.0, "the film did not spread");
        assert!(free_at(&w, 0, 1) < film);
        assert!(residual(&w).abs() < 1e-15);
    }

    #[test]
    fn a_film_below_the_threshold_still_soaks_in_evaporates_and_falls() {
        let film = 0.5 * threshold();

        // Soaks in: the film on a soil cell.
        let mut w = World::empty(cfg(1, 4));
        rock(&mut w, 0, 1, Material::Soil);
        add(&mut w, 0, 2, film);
        infiltrate(&mut w, DT / 4.0);
        assert!(free_at(&w, 0, 2) < film, "the film did not soak in");

        // Evaporates: on the bedrock of an open column.
        let mut w = World::empty(Config {
            evaporation_m_per_s: 1e-6,
            ..cfg(1, 4)
        });
        add(&mut w, 0, 1, film);
        evaporate(&mut w);
        assert!(free_at(&w, 0, 1) < film, "the film did not evaporate");

        // Falls, and offers downward in the exchange: the film in the air over an empty
        // cell.
        let mut w = World::empty(cfg(1, 5));
        add(&mut w, 0, 3, film);
        fall(&mut w);
        assert_eq!(free_at(&w, 0, 2), film, "the film did not fall");
        exchange(&mut w, 1);
        assert_eq!(free_at(&w, 0, 1), film, "the exchange did not move it down");
        assert_eq!(free_at(&w, 0, 2), 0.0);
    }

    /// The thin top cell of a lake does not hold back the full cells under it: they push by
    /// their own `free`, and the lake levels against a lower pool.
    ///
    /// The lake and the pool are joined by a one-row passage, as the U-tube fixtures are.
    /// Two columns side by side over two or more full rows would not do: each row pushes
    /// half of the same head difference, so two rows move the whole of it and the pair
    /// swaps back and forth every substep — a 2-cycle the exchange has with or without the
    /// spreading depth, which this test is not about.
    #[test]
    fn a_lake_with_a_thin_surface_cell_still_levels_against_a_lower_pool() {
        // Column 0 is the lake shaft, column 1 a dry passage along row 1, column 2 the
        // pool's shaft and column 3 rock.
        let mut w = World::empty(cfg(4, 7));
        for y in 1..7 {
            rock(&mut w, 3, y, Material::Rock);
            if y >= 2 {
                rock(&mut w, 1, y, Material::Rock);
            }
        }
        for y in 1..4 {
            add(&mut w, 0, y, 1.0);
        }
        add(&mut w, 0, 4, 0.5 * threshold());
        add(&mut w, 2, 1, 0.5);
        exchange(&mut w, 1);
        assert!(
            free_at(&w, 1, 1) > 0.0,
            "the lake's full cells did not push into the passage"
        );
        for _ in 0..200 {
            w.step();
        }
        let column = |x: i64| (1..7).map(|y| free_at(&w, x, y)).sum::<f64>();
        assert!(
            (free_at(&w, 1, 1) - 1.0).abs() < 1e-9,
            "the passage is not full"
        );
        assert!(
            (column(0) - column(2)).abs() < 1e-3,
            "not level: {} against {}",
            column(0),
            column(2)
        );
        assert!(residual(&w).abs() < 1e-12);
    }

    /// Rain in films on a staircase of rock and soil — much of it shallower than the
    /// threshold, some of it deeper — conserves water.
    #[test]
    fn a_shower_of_films_conserves_water() {
        let mut w = World::empty(Config {
            evaporation_m_per_s: 1e-7,
            ..cfg(8, 6)
        });
        for x in 0..8i64 {
            for y in 1..=(x as u32 % 4) {
                rock(
                    &mut w,
                    x,
                    y,
                    if y == 1 {
                        Material::Soil
                    } else {
                        Material::Rock
                    },
                );
            }
        }
        for tick in 0..200 {
            let per_column = if tick % 5 == 0 { 30.0 } else { 0.3 } * threshold();
            w.apply(Command::RainPulse {
                volume_m3: 8.0 * per_column,
            });
            w.step();
        }
        let thin = w
            .free
            .iter()
            .filter(|&&f| f > 0.0 && f < threshold())
            .count();
        assert!(
            thin > 0,
            "no film under the threshold: the rule went unexercised"
        );
        assert!(residual(&w).abs() < 1e-12, "residual {}", residual(&w));
    }
}

/// The cached sky (package PA): the per-column sky floor from the terrain cache plus the
/// wet set give the same cells the ceiling-down walk found, on every kind of column and
/// across terrain edits; and a real shower on the shipped `small` ring conserves water.
#[cfg(test)]
mod sky_tests {
    use super::{open_water_at, open_water_cell_walk, sky_cell_at, sky_cell_walk, sky_tops};
    use crate::{Command, Config, Material, Preset, World};

    /// Every column's cached answers against the walk, and the sky floor against a walk of
    /// its own: the lowest row with nothing solid between it and the ceiling.
    fn assert_matches_the_walk(w: &mut World, when: &str) {
        let top = sky_tops(w);
        let (width, depth, height) = (w.config.width, w.config.depth, w.config.height);
        for z in 0..depth {
            for x in 0..width {
                let col = (z * width + x) as usize;
                let floor = (0..height)
                    .rev()
                    .take_while(|&y| !w.view().material_at(x as i64, y, z).is_solid())
                    .last()
                    .unwrap_or(height);
                assert_eq!(
                    w.void_runs.sky_floor[col], floor,
                    "sky floor of ({x}, {z}) {when}"
                );
                assert_eq!(
                    sky_cell_at(w, col, top[col]),
                    sky_cell_walk(w, x as i64, z),
                    "sky cell of ({x}, {z}) {when}"
                );
                assert_eq!(
                    open_water_at(w, col, top[col]),
                    open_water_cell_walk(w, x as i64, z),
                    "open water of ({x}, {z}) {when}"
                );
            }
        }
    }

    fn set(w: &mut World, x: i64, y: u32, z: u32, material: Material) {
        w.apply(Command::SetMaterial { x, y, z, material });
    }

    fn water(w: &mut World, x: i64, y: u32, z: u32, volume_m3: f64) {
        w.apply(Command::AddWater { x, y, z, volume_m3 });
    }

    /// Row `z = 0`, one column each: sky-open ground with a film; a roofed column with
    /// water under the roof; a grotto with water in the pocket and on the roof; a column
    /// packed to the ceiling; a brim-full cell with dry air over it; a brim-full top row.
    /// Row `z = 1` is dry air to the floor.
    fn columns() -> World {
        let mut w = World::empty(Config {
            width: 6,
            height: 10,
            depth: 2,
            voxel_m: 1.0,
            seed: 3,
            ..Config::default()
        });
        for y in 0..3 {
            set(&mut w, 0, y, 0, Material::Rock);
        }
        water(&mut w, 0, 3, 0, 0.3);
        set(&mut w, 1, 7, 0, Material::Rock);
        water(&mut w, 1, 0, 0, 1.0);
        water(&mut w, 1, 1, 0, 0.5);
        for y in [0, 1, 5] {
            set(&mut w, 2, y, 0, Material::Soil);
        }
        water(&mut w, 2, 2, 0, 0.7);
        water(&mut w, 2, 6, 0, 0.2);
        set(&mut w, 3, 9, 0, Material::Rock);
        water(&mut w, 3, 4, 0, 0.4);
        for y in 0..4 {
            set(&mut w, 4, y, 0, Material::Rock);
        }
        water(&mut w, 4, 4, 0, 1.0);
        for y in 0..9 {
            set(&mut w, 5, y, 0, Material::Rock);
        }
        water(&mut w, 5, 9, 0, 1.0);
        w
    }

    #[test]
    fn the_cached_sky_equals_the_walk_on_open_roofed_and_grotto_columns() {
        let mut w = columns();
        assert_matches_the_walk(&mut w, "as built");
        // The fixture really has the cases: open water on the ground, none under a roof,
        // the grotto's roof-top film seen and its pocket not, a packed column refused.
        let top = sky_tops(&mut w);
        assert!(open_water_at(&w, 0, top[0]).is_some());
        assert_eq!(
            open_water_at(&w, 1, top[1]),
            None,
            "roofed water is not open"
        );
        assert_eq!(
            sky_cell_at(&w, 3, top[3]),
            None,
            "a packed column has no sky"
        );
        assert_eq!(
            sky_cell_at(&w, 5, top[5]),
            None,
            "a full top row has no room"
        );
    }

    #[test]
    fn a_terrain_edit_that_opens_or_closes_a_column_moves_the_sky_floor() {
        let mut w = columns();
        assert_matches_the_walk(&mut w, "before the edits");
        // Open: the roofed column loses its roof, and its water is under the sky.
        set(&mut w, 1, 7, 0, Material::Air);
        assert_matches_the_walk(&mut w, "after opening the roof");
        let top = sky_tops(&mut w);
        assert!(open_water_at(&w, 1, top[1]).is_some());
        // Close: a lid over the open ground column and a ceiling cell over dry air.
        set(&mut w, 0, 8, 0, Material::Rock);
        set(&mut w, 0, 9, 1, Material::Rock);
        assert_matches_the_walk(&mut w, "after closing two columns");
        let top = sky_tops(&mut w);
        assert_eq!(open_water_at(&w, 0, top[0]), None);
        assert_eq!(sky_cell_at(&w, 6, top[6]), None);
        // And the water moving under a fixed terrain is read fresh every time.
        for tick in 0..20 {
            w.step();
            assert_matches_the_walk(&mut w, &format!("after tick {tick}"));
        }
    }

    /// A due shower on the shipped `small` ring: it starts, it rains, the cached sky is the
    /// walk on the real landform every tick, and the ledger closes.
    #[test]
    fn a_shower_on_the_small_ring_conserves_water() {
        let preset = Preset::find("small").expect("small is a shipped preset");
        let mut w = World::new(Config {
            seed: 1,
            ..preset.config()
        });
        assert!(w.config.closed_water_budget);
        let showers = w.ledger.showers;
        let rain_before = w.ledger.rain_in;
        w.next_shower_tick = w.tick;
        for tick in 0..40 {
            if tick % 10 == 0 {
                assert_matches_the_walk(&mut w, &format!("at shower tick {tick}"));
            }
            w.step();
            let v = w.view();
            let residual = v.stored_m3() - v.ledger.expected_stored();
            assert!(
                residual.abs() < 1e-9,
                "residual {residual:e} m3 at shower tick {tick}"
            );
        }
        assert_eq!(
            w.ledger.showers,
            showers + 1,
            "the due shower did not start"
        );
        assert!(w.ledger.rain_in > rain_before, "the shower placed no rain");
        assert!(
            w.shower_left_m3 > 0.0,
            "a minute's shower ended in two seconds"
        );
    }
}

/// The closed water budget: nothing leaves but `displaced_out`, and the arithmetic says
/// so. Every assertion here is conservation, never a pinned number
/// (`design/handoffs/voxel-water-cycle-2026-09-20.md`).
#[cfg(test)]
mod closed_budget_tests {
    use super::*;
    use crate::{Command, Config, Material};

    const TICKS: u32 = 200;

    fn config(closed: bool) -> Config {
        Config {
            width: 8,
            height: 8,
            depth: 2,
            voxel_m: 0.25,
            rain_m_per_s: 0.002,
            evaporation_m_per_s: 0.0004,
            outlet_m3_per_s: 0.001,
            closed_water_budget: closed,
            initial_atmosphere_m3: if closed { 0.2 } else { 0.0 },
            shower_trigger_fraction: 0.02,
            shower_volume_m3: 0.05,
            ..Config::default()
        }
    }

    /// Bedrock floor, one soil row on it, a puddle above that, and the outlet at the
    /// puddle's own row so the export has something to take.
    fn fixture(closed: bool) -> World {
        let c = config(closed);
        let (width, depth) = (c.width as i64, c.depth);
        let mut w = World::empty(c);
        for x in 0..width {
            for z in 0..depth {
                w.apply(Command::SetMaterial {
                    x,
                    y: 1,
                    z,
                    material: Material::Soil,
                });
            }
        }
        for x in 0..width {
            for z in 0..depth {
                w.apply(Command::AddWater {
                    x,
                    y: 2,
                    z,
                    volume_m3: 0.004,
                });
            }
        }
        w.set_outlet_cell(Some((0, 2, 0)));
        w.apply(Command::SetOutlet { open: true });
        w
    }

    fn run(w: &mut World, ticks: u32) {
        for t in 0..ticks {
            // One plant-sized withdrawal part way in, so transpiration is in the books
            // too and not only evaporation and the outlet.
            if t == ticks / 2 {
                w.apply(Command::WithdrawPore {
                    x: 3,
                    y: 1,
                    z: 0,
                    volume_m3: 0.001,
                });
            }
            w.step_with(1);
        }
    }

    #[test]
    fn a_closed_world_keeps_every_drop_it_started_with() {
        let mut w = fixture(true);
        let before = w.view().total_water_m3();
        run(&mut w, TICKS);
        let v = w.view();
        assert!(
            v.water_residual().abs() < 1e-9,
            "in-world residual {:e}",
            v.water_residual()
        );
        assert!(
            v.atmosphere_residual().abs() < 1e-9,
            "atmosphere residual {:e}",
            v.atmosphere_residual()
        );
        assert!(
            v.total_residual().abs() < 1e-9,
            "total residual {:e}",
            v.total_residual()
        );
        assert_eq!(v.ledger.displaced_out, 0.0, "nothing was displaced");
        assert!(
            (v.total_water_m3() - before).abs() < 1e-9,
            "a closed world with no user input holds what it held: {before} then {}",
            v.total_water_m3()
        );
    }

    /// The three loss terms are the store's income, to the drop. This is what makes
    /// `evaporation_out`, `transpiration_out` and `outlet_out` mean "into the store"
    /// rather than "out of the world".
    #[test]
    fn the_losses_are_the_stores_income() {
        let mut w = fixture(true);
        run(&mut w, TICKS);
        let l = w.view().ledger.clone();
        assert!(l.evaporation_out > 0.0, "the puddle must evaporate");
        assert!(l.outlet_out > 0.0, "the outlet must export");
        assert!(l.transpiration_out > 0.0, "the withdrawal must be booked");
        let deposited = l.evaporation_out + l.transpiration_out + l.outlet_out;
        assert!(
            (l.atmosphere_in - deposited).abs() < 1e-9,
            "atmosphere_in {} is not the three losses {deposited}",
            l.atmosphere_in
        );
        assert_eq!(l.user_atmosphere_in, 0.0, "no lever was pulled");
    }

    /// Rain in a closed world is the store spending itself: never more than the store
    /// ever held, and every drop of it withdrawn before it fell.
    #[test]
    fn rain_only_ever_comes_out_of_the_store() {
        let mut w = fixture(true);
        run(&mut w, TICKS);
        let l = w.view().ledger.clone();
        assert!(l.showers > 0, "the store never reached the trigger");
        assert!(
            l.rain_in > 0.0 && (l.rain_in - l.atmosphere_out).abs() < 1e-9,
            "rain {} is not what the store paid out {}",
            l.rain_in,
            l.atmosphere_out
        );
        assert!(
            l.atmosphere_out <= l.initial_atmosphere + l.atmosphere_in + 1e-9,
            "the store paid out {} of the {} it ever held",
            l.atmosphere_out,
            l.initial_atmosphere + l.atmosphere_in
        );
    }

    // ---- river re-entry ---------------------------------------------------------------

    /// A slab with a named spring cell on open ground and a charged sky.
    fn stream_slab(atmosphere_m3: f64, rate: f64, roofed: bool) -> World {
        let c = Config {
            width: 8,
            height: 8,
            depth: 2,
            voxel_m: 0.25,
            rain_m_per_s: 3.5e-5,
            evaporation_m_per_s: 0.0,
            closed_water_budget: true,
            initial_atmosphere_m3: atmosphere_m3,
            shower_trigger_fraction: 0.01,
            shower_volume_m3: 0.05,
            // No schedule and a floor the store starts over would rain at once and muddy
            // the arithmetic; the interval parks the shower far outside these windows.
            shower_interval_min_s: 600.0,
            shower_interval_max_s: 600.0,
            reentry_m3_per_s: rate,
            ..Config::default()
        };
        let mut w = World::empty(c.clone());
        for x in 0..c.width as i64 {
            for z in 0..c.depth {
                w.apply(Command::SetMaterial {
                    x,
                    y: 1,
                    z,
                    material: Material::Soil,
                });
            }
        }
        if roofed {
            // The seep's own cell is rock: `emerge` stops at the first solid, so there is
            // nowhere for the stream to come out and nothing is withdrawn.
            w.apply(Command::SetMaterial {
                x: 3,
                y: 2,
                z: 0,
                material: Material::Rock,
            });
        }
        w.set_spring_cell(Some((3, 2, 0)));
        w
    }

    /// The stream moves water out of the sky and into the world, and the books hold it
    /// term for term. It stops at the shower floor — it may never eat the rain — and a
    /// rate of zero is no stream at all.
    #[test]
    fn reentry_moves_store_to_spring_and_conserves() {
        let rate = 0.002;
        let mut w = stream_slab(0.5, rate, false);
        let before = w.atmosphere_m3();
        run(&mut w, 100);
        let v = w.view();
        let want = 100.0 * rate * DT;
        assert!(
            (v.ledger.reentry_in - want).abs() <= 1e-9 * want,
            "a hundred ticks of stream: {} against {want}",
            v.ledger.reentry_in
        );
        // The sky paid exactly that out. Its *net* change is smaller, because the
        // harness's one plant-sized withdrawal transpires back into it part way through —
        // which is the point of booking the two directions apart.
        assert!(
            (v.ledger.atmosphere_out - want).abs() <= 1e-9 * want,
            "the sky paid for every drop: out {} against {want}",
            v.ledger.atmosphere_out
        );
        assert!(
            (before - w.atmosphere_m3() - (want - v.ledger.transpiration_out)).abs() <= 1e-9,
            "and its net change is what it paid less what came back: {} -> {}, ledger {:?}",
            before,
            w.atmosphere_m3(),
            v.ledger
        );
        assert!(v.total_residual().abs() < 1e-9, "{}", v.total_residual());
        assert_eq!(v.ledger.showers, 0, "no shower muddied this window");

        // At the floor the stream stops: the drought lock is where `shower` put it.
        let mut starved = stream_slab(0.0, rate, false);
        run(&mut starved, 100);
        assert_eq!(starved.view().ledger.reentry_in, 0.0);

        // And no rate is no stream.
        let mut off = stream_slab(0.5, 0.0, false);
        run(&mut off, 100);
        assert_eq!(off.view().ledger.reentry_in, 0.0);
        assert_eq!(off.atmosphere_m3(), 0.5);
    }

    /// A spring under rock pushes nothing out, and keeps what it could not push.
    #[test]
    fn a_roofed_spring_keeps_the_store() {
        let mut w = stream_slab(0.5, 0.002, true);
        run(&mut w, 100);
        let v = w.view();
        assert_eq!(v.ledger.reentry_in, 0.0, "the roof stopped it");
        assert_eq!(w.atmosphere_m3(), 0.5, "and nothing was withdrawn");
        assert!(v.total_residual().abs() < 1e-9);
    }

    /// The open budget has no store to draw on, so there is nothing to return.
    #[test]
    fn the_open_budget_ignores_reentry() {
        let mut w = World::empty(Config {
            closed_water_budget: false,
            reentry_m3_per_s: 0.002,
            ..config(false)
        });
        w.set_spring_cell(Some((3, 2, 0)));
        run(&mut w, 100);
        assert_eq!(w.view().ledger.reentry_in, 0.0);
        assert_eq!(w.atmosphere_m3(), 0.0);
    }

    // ---- the shower schedule ---------------------------------------------------------
    /// **Rain is a shower falling, not a rate in the config.** Under the closed cycle
    /// `rain_m_per_s` is the rate a shower falls *at* and is always positive, so reading
    /// it as "is it raining" drew streaks on the panel every tick of a world that rained
    /// one minute in ten (Wrysk, 2026-09-21).
    #[test]
    fn closed_cycle_rain_is_only_the_shower() {
        let mut w = slab(0.0, 0.5, (0.0, 0.0));
        assert!(w.config().rain_m_per_s > 0.0);
        assert!(
            !w.view().is_raining(),
            "a closed world with a store under the floor is not raining"
        );

        let floor = 0.5 * w.view().ledger.expected_total();
        w.apply(Command::AddAtmosphere {
            volume_m3: floor * 2.0,
        });
        w.step();
        assert!(
            w.shower_left_m3() > 0.0 && w.view().is_raining(),
            "the shower fell"
        );
        for _ in 0..400 {
            w.step();
            if w.shower_left_m3() <= 0.0 {
                break;
            }
        }
        assert_eq!(
            w.shower_left_m3(),
            0.0,
            "the shower ran out inside 400 ticks"
        );
        assert!(!w.view().is_raining(), "and the sky is shut again");

        // The open budget is unchanged: its rain really is prescribed, every tick.
        let open = World::empty(Config {
            rain_m_per_s: 0.002,
            ..Config::default()
        });
        assert!(open.view().is_raining());
    }

    /// A dry soil slab with a wet aquifer under it: the fixture the viability test uses,
    /// with the schedule's own numbers on top.
    fn slab(atmosphere_m3: f64, floor: f64, interval_s: (f64, f64)) -> World {
        let c = Config {
            width: 8,
            height: 8,
            depth: 2,
            rain_m_per_s: 0.002,
            evaporation_m_per_s: 0.0,
            closed_water_budget: true,
            initial_atmosphere_m3: atmosphere_m3,
            shower_trigger_fraction: floor,
            shower_volume_m3: 0.0005,
            initial_aquifer_head_m: 0.4,
            shower_interval_min_s: interval_s.0,
            shower_interval_max_s: interval_s.1,
            ..Config::default()
        };
        let mut w = World::empty(c.clone());
        for x in 0..c.width as i64 {
            for z in 0..c.depth {
                w.apply(Command::SetMaterial {
                    x,
                    y: 1,
                    z,
                    material: Material::Soil,
                });
            }
        }
        w
    }

    /// The first gap is drawn from the world's own seed, inside the interval it was
    /// given, and it is the same draw every time that seed is used.
    #[test]
    fn the_first_due_tick_is_drawn_inside_the_interval() {
        let at = |seed: u64| {
            World::empty(Config {
                seed,
                closed_water_budget: true,
                initial_atmosphere_m3: 0.1,
                shower_interval_min_s: 300.0,
                shower_interval_max_s: 900.0,
                ..Config::default()
            })
            .next_shower_tick()
        };
        let drawn: Vec<u64> = (1..=4).map(at).collect();
        for (seed, t) in drawn.iter().enumerate() {
            assert!(
                (6000..=18000).contains(t),
                "seed {} drew {t}, outside 5 to 15 minutes",
                seed + 1
            );
        }
        assert!(
            drawn
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                >= 2,
            "four seeds drew the same gap every time: {drawn:?}"
        );
        assert_eq!(at(1), drawn[0], "the same seed draws the same weather");
    }

    /// The calendar decides when. Nothing falls before the due tick; the shower starts on
    /// it; and the gap that follows is measured from where that shower **ended**.
    #[test]
    fn a_due_shower_starts_on_its_tick_and_not_before() {
        // Two seconds either way, so the draw is exactly 40 ticks and the arithmetic is
        // readable. The floor is zero: this test is about the calendar alone.
        let mut w = slab(0.3, 0.0, (2.0, 2.0));
        assert_eq!(w.next_shower_tick(), 40);

        // `shower` runs before the tick counter moves, so the step that sees `tick == 39`
        // is the 40th, and the due shower falls on the 41st.
        for _ in 0..40 {
            w.step();
        }
        assert_eq!(w.tick(), 40);
        assert_eq!(
            w.view().ledger.showers,
            0,
            "no rain through tick 39: the water phase of step n sees tick n - 1"
        );
        w.step();
        assert_eq!(
            w.view().ledger.showers,
            1,
            "the due shower falls on the step whose water phase sees tick 40"
        );

        // It ends, and the next one is booked at least a whole interval after that.
        let mut ended_at = None;
        for _ in 0..200 {
            w.step();
            if w.shower_left_m3() <= 0.0 && ended_at.is_none() {
                ended_at = Some(w.tick());
                break;
            }
        }
        let ended_at = ended_at.expect("a 0.0005 m³ shower ends inside 200 ticks");
        // Same convention as the start: the water phase of the step that drained the
        // allowance saw `ended_at - 1`, and the gap is drawn from there.
        let next = w.next_shower_tick();
        assert_eq!(
            next,
            ended_at - 1 + 40,
            "the gap runs from the last tick it rained on, not from the start"
        );
        while w.tick() < next {
            w.step();
        }
        assert_eq!(
            w.view().ledger.showers,
            1,
            "still the one shower right up to the tick the next is due"
        );
        w.step();
        assert_eq!(
            w.view().ledger.showers,
            2,
            "and the next one falls on its own due tick"
        );
    }

    /// The store decides whether. A due shower with nothing to pay for it does not fall,
    /// does not count, and does not lose its turn: it falls on the first tick the floor
    /// holds.
    #[test]
    fn a_starved_sky_holds_the_shower_until_the_floor() {
        let mut w = slab(0.0, 0.01, (2.0, 2.0));
        for _ in 0..141 {
            w.step();
        }
        assert!(w.tick() > w.next_shower_tick(), "the shower is overdue");
        assert_eq!(
            w.view().ledger.showers,
            0,
            "an empty sky does not rain, however overdue"
        );

        let floor = 0.01 * w.view().ledger.expected_total();
        assert!(
            w.apply(Command::AddAtmosphere {
                volume_m3: floor * 4.0
            }) > 0.0
        );
        w.step();
        assert_eq!(
            w.view().ledger.showers,
            1,
            "the held shower falls on the first tick it can be paid for"
        );
        assert!(w.view().total_residual().abs() < 1e-9);
    }

    /// No interval is the trigger alone: the behaviour every fixture and every existing
    /// closed-budget test has, which those tests check unmodified.
    #[test]
    fn no_interval_means_the_trigger_alone() {
        let mut w = World::empty(Config {
            initial_atmosphere_m3: 0.0,
            shower_trigger_fraction: 1.0,
            shower_interval_min_s: 0.0,
            shower_interval_max_s: 0.0,
            ..config(true)
        });
        assert_eq!(w.next_shower_tick(), 0, "no schedule is drawn at all");
        w.apply(Command::AddWater {
            x: 0,
            y: 2,
            z: 0,
            volume_m3: 0.004,
        });
        run(&mut w, TICKS);
        let v = w.view();
        assert_eq!(v.ledger.showers, 0, "no shower may start under the trigger");
        assert_eq!(v.ledger.rain_in, 0.0, "and no rain may fall");
        assert!(v.total_residual().abs() < 1e-9);
    }

    /// Between showers the sky is shut: a closed world whose store cannot reach the
    /// trigger books no rain at all, however long it runs.
    #[test]
    fn a_store_under_the_trigger_never_rains() {
        let mut w = World::empty(Config {
            initial_atmosphere_m3: 0.0,
            shower_trigger_fraction: 1.0,
            ..config(true)
        });
        w.apply(Command::AddWater {
            x: 0,
            y: 2,
            z: 0,
            volume_m3: 0.004,
        });
        run(&mut w, TICKS);
        let v = w.view();
        assert_eq!(v.ledger.showers, 0, "no shower may start under the trigger");
        assert_eq!(v.ledger.rain_in, 0.0, "and no rain may fall");
        assert!(v.total_residual().abs() < 1e-9);
    }

    /// The open budget is untouched: water still leaves, the store stays empty, and the
    /// one residual every existing fixture checks is still zero.
    #[test]
    fn the_open_budget_still_flows_through() {
        let mut w = fixture(false);
        let before = w.view().total_water_m3();
        run(&mut w, TICKS);
        let v = w.view();
        assert_eq!(v.atmosphere_m3, 0.0, "an open world holds nothing aloft");
        assert_eq!(v.ledger.atmosphere_in, 0.0);
        assert_eq!(v.ledger.atmosphere_out, 0.0);
        assert_eq!(v.ledger.showers, 0);
        assert!(v.ledger.evaporation_out > 0.0 && v.ledger.outlet_out > 0.0);
        assert!(v.ledger.rain_in > 0.0, "the prescribed rain must fall");
        assert!(v.water_residual().abs() < 1e-9);
        // Flow-through, stated as arithmetic: the rain came from outside the stores
        // (nothing was drawn) and the losses went outside them (nothing was deposited),
        // and the change in the world's water is exactly those fluxes.
        let l = v.ledger;
        let flowed = l.rain_in - l.evaporation_out - l.outlet_out - l.transpiration_out;
        assert!(
            (v.total_water_m3() - before - flowed).abs() < 1e-9,
            "an open world's water is {before} plus the through-flow {flowed}, not {}",
            v.total_water_m3()
        );
    }

    /// A zero shower volume is no weather at all: no shower starts and the count stays
    /// put, rather than a fresh shower being booked on every tick forever.
    #[test]
    fn a_zero_shower_volume_starts_no_shower() {
        let mut w = World::empty(Config {
            shower_volume_m3: 0.0,
            ..config(true)
        });
        w.apply(Command::AddAtmosphere { volume_m3: 1.0 });
        run(&mut w, TICKS);
        let v = w.view();
        assert_eq!(v.ledger.showers, 0);
        assert_eq!(v.ledger.rain_in, 0.0);
        assert!(v.total_residual().abs() < 1e-9);
    }

    /// The user's lever: water aloft, the store's residual still zero, and the world's
    /// total up by exactly what the lever gave it. An open world refuses it.
    #[test]
    fn the_lever_puts_water_aloft_and_an_open_world_refuses_it() {
        let mut w = fixture(true);
        let before = w.view().total_water_m3();
        let took = w.apply(Command::AddAtmosphere { volume_m3: 0.5 });
        assert_eq!(took, 0.5);
        assert_eq!(w.apply(Command::AddAtmosphere { volume_m3: -1.0 }), 0.0);
        assert_eq!(
            w.apply(Command::AddAtmosphere {
                volume_m3: f64::NAN
            }),
            0.0
        );
        run(&mut w, TICKS);
        let v = w.view();
        assert_eq!(v.ledger.user_atmosphere_in, 0.5);
        assert!(v.atmosphere_residual().abs() < 1e-9);
        assert!(v.total_residual().abs() < 1e-9);
        assert!(
            (v.total_water_m3() - before - 0.5).abs() < 1e-9,
            "the lever added 0.5 to {before}, giving {}",
            v.total_water_m3()
        );

        let mut open = fixture(false);
        assert_eq!(open.apply(Command::AddAtmosphere { volume_m3: 0.5 }), 0.0);
        assert_eq!(open.view().ledger.atmosphere_in, 0.0);
    }

    /// The store is state, so it is saved; and a world claiming an atmosphere without
    /// the closed budget is not a world.
    #[test]
    fn the_store_survives_a_round_trip_and_an_open_world_may_not_hold_one() {
        let mut w = fixture(true);
        run(&mut w, TICKS / 2);
        let back = World::load(&w.save()).expect("a closed world round-trips");
        assert_eq!(back.atmosphere_m3(), w.atmosphere_m3());
        assert_eq!(back.shower_left_m3(), w.shower_left_m3());
        assert_eq!(back.view().ledger, w.view().ledger);

        // The calendar is state too: a world resumed from a snapshot owes its next shower
        // at the tick the saved one did, not a freshly drawn one.
        let mut scheduled = slab(0.3, 0.0, (2.0, 2.0));
        run(&mut scheduled, 50);
        let back = World::load(&scheduled.save()).expect("a scheduled world round-trips");
        assert_eq!(back.next_shower_tick(), scheduled.next_shower_tick());
        assert!(back.next_shower_tick() > 0);

        // And the stream's own books: `reentry_in` is an inflow like any other, so a
        // resumed world's conservation identity has to carry it.
        let mut stream = stream_slab(0.5, 0.002, false);
        run(&mut stream, 50);
        assert!(stream.view().ledger.reentry_in > 0.0);
        let back = World::load(&stream.save()).expect("a streaming world round-trips");
        assert_eq!(back.view().ledger, stream.view().ledger);
        assert!(back.view().total_residual().abs() < 1e-9);

        let mut bad = fixture(false);
        bad.atmosphere_m3 = 1.0;
        let err = World::load(&bad.save()).expect_err("an open world holds no atmosphere");
        assert!(format!("{err:#}").contains("no atmosphere"), "{err:#}");
    }
}

/// Package 1c's loud deterministic check, at the level of the rate expression itself:
/// the world's stores round-trip a volume through a cell fraction, so an end-to-end
/// measurement can only be right to a couple of ulps (`tests/water_units.rs` pins that
/// half). Here the arithmetic is compared directly, bit for bit.
#[cfg(test)]
mod units_tests {
    use super::{Config, DT, Material, pore_flux_m3};
    use crate::material::REFERENCE_VOXEL_M;

    /// What the solver computed before package 1c: a fraction of a cell per tick.
    fn pre_1c(m: Material, c: &Config, dt: f64) -> f64 {
        m.permeability_per_s() * dt * m.pore_capacity() * c.voxel_volume()
    }

    fn grid(voxel_m: f64) -> Config {
        Config {
            voxel_m,
            ..Config::default()
        }
    }

    /// On the 0.25 m reference grid the new expression is the old one **bit for bit**,
    /// for every material and for a substep's `dt` as well as a tick's. Nothing on the
    /// reference grid is retuned by this package, and the check is exact rather than
    /// approximate so a reassociation can never creep in unnoticed.
    #[test]
    fn the_reference_grid_is_identical_bit_for_bit() {
        let c = grid(REFERENCE_VOXEL_M);
        for m in [
            Material::Air,
            Material::Bedrock,
            Material::Rock,
            Material::Soil,
        ] {
            for dt in [DT, DT / 4.0, DT / 3.0] {
                let (got, want) = (pore_flux_m3(m, &c, dt), pre_1c(m, &c, dt));
                assert_eq!(
                    got.to_bits(),
                    want.to_bits(),
                    "{m:?} at dt {dt}: {got} is not the pre-1c {want}"
                );
            }
        }
    }

    /// And off the reference grid it is the *conductivity* form — `K · A · dt` — to
    /// within two ulps, which is the whole point: the flux per unit area no longer
    /// depends on the cell. Two and not zero because the two orderings round
    /// `permeability · dt · pore_capacity` and `(permeability · pore_capacity) · dt`
    /// differently; see `pore_flux_m3` for why this side is the one that gives.
    #[test]
    fn the_helper_is_the_conductivity_form() {
        for voxel_m in [0.25, 0.125, 0.5, 1.0, 0.1] {
            let c = grid(voxel_m);
            for m in [Material::Rock, Material::Soil] {
                let got = pore_flux_m3(m, &c, DT);
                let want = m.conductivity_m_per_s() * c.cell_area() * DT;
                assert!(
                    (got - want).abs() <= 2.0 * f64::EPSILON * want,
                    "{m:?} on {voxel_m} m cells: {got} against K·A·dt = {want}"
                );
            }
        }
    }

    /// The physical claim, stated on its own: the flux **per square metre** is the same
    /// on every grid, where before it was proportional to the cell.
    #[test]
    fn the_flux_per_unit_area_no_longer_depends_on_the_cell() {
        let m = Material::Soil;
        let reference = pore_flux_m3(m, &grid(0.25), DT) / grid(0.25).cell_area();
        for voxel_m in [0.125, 0.5, 1.0, 0.1] {
            let c = grid(voxel_m);
            let per_area = pore_flux_m3(m, &c, DT) / c.cell_area();
            assert!(
                (per_area - reference).abs() <= 1e-18,
                "{voxel_m} m cells pass {per_area} m per tick against {reference}"
            );
            // The bug this replaces, stated so it cannot come back silently.
            let pre = pre_1c(m, &c, DT) / c.cell_area();
            assert!(
                (pre - reference * (voxel_m / 0.25)).abs() <= 1e-18,
                "the pre-1c form was not proportional to the cell after all"
            );
        }
    }
}
