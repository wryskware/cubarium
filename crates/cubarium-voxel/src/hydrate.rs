//! Water already in place: one inventory, split four ways, poured into the geometry.
//!
//! `design/terrain-generation-plan-2026-09-21.md` §4. A fresh staged world is charged a
//! single total — [`Water::inventory_m`] metres over its footprint — and the atmosphere,
//! the aquifer, the pore water and the pools are **shares of that one number**, never
//! four independent targets. What the geometry cannot hold ends up in the aquifer, so
//! the four stores sum to the inventory exactly.
//!
//! Pools come from the geometry, not from a level: [`basins`] finds every place in the
//! **void** where water cannot get any lower, which is a surface depression, a valley
//! floor and the bowl floor of a grotto alike, and each one fills to one head. The
//! shares follow each basin's own catchment — the ground that drains into it — so a
//! ring does not put its whole inventory in its single lowest hollow and leave every
//! depression and every grotto dry.
//!
//! Nothing here is a claim of hydrological equilibrium. [`crate::World::settle`] runs
//! the world's own solver afterwards and measures what it did.

use crate::World;
use crate::recipe::Water;

/// One place water collects: a connected void region with a floor it cannot leave, and
/// the level it fills to before it spills somewhere lower.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Basin {
    /// Void cells it can hold water in, ascending in `y`. At `spill_y` it overflows.
    pub cells: Vec<usize>,
    /// Lowest cell's `y`.
    pub floor_y: u32,
    /// The level it spills at: it holds cells with `y < spill_y`.
    pub spill_y: u32,
    /// Ground cells — void directly over a solid — whose runoff reaches this basin.
    pub catchment: usize,
}

/// What [`hydrate`] put where, in cubic metres.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hydrated {
    /// Free water standing in the basins.
    pub pooled_m3: f64,
    /// Pore water wetted into the soil.
    pub pore_m3: f64,
    /// The aquifer store, the requested head plus whatever the geometry refused.
    pub aquifer_m3: f64,
    /// Water held aloft for the cycle.
    pub atmosphere_m3: f64,
    /// Basins found, and how many took water.
    pub basins: usize,
    pub basins_filled: usize,
    /// Of `aquifer_m3`, the part that is inventory the surface could not hold.
    pub spare_m3: f64,
}

impl Hydrated {
    /// Every store, which is the inventory.
    pub fn total_m3(&self) -> f64 {
        self.pooled_m3 + self.pore_m3 + self.aquifer_m3 + self.atmosphere_m3
    }
}

/// Charge a world with its recipe's water inventory.
///
/// `inventory_m = 0` does nothing whatever, which is what every `Ridge` world and every
/// fixture gets. [`crate::World::new`] calls it once the terrain is installed.
///
/// Everything it adds is **booked** — `Ledger::user_in` for the in-world stores,
/// `atmosphere_in` for the store aloft — so it is safe on a world that already holds
/// water: a store is raised *to* its target and never lowered, and the ledger explains
/// the difference. `World::new` clears that booking and records the result as the world's
/// initial stores instead, because for a fresh world the inventory **is** what it began
/// with.
pub fn hydrate(world: &mut World, w: &Water) -> Hydrated {
    let c = world.config().clone();
    let footprint = c.width as f64 * c.depth as f64 * c.cell_area();
    let total = w.inventory_m * footprint;
    if !(total > 0.0) {
        return Hydrated::default();
    }
    // The share aloft exists only where there is a sky to hold it: an **open**-budget
    // world has no atmosphere store at all (`Config::closed_water_budget`, and
    // `World::validate_loaded` refuses one that pretends otherwise), so its share stays in
    // the ground and the inventory still adds up.
    let atmosphere = if c.closed_water_budget {
        total * w.atmosphere_fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let charged = c
        .aquifer_volume_for_head(w.aquifer_head_m)
        .min(total - atmosphere);
    let available = total - atmosphere - charged;

    // **Pore first, then pools.** Water wets the ground before it stands on it: soil that
    // has been rained on and has drained holds its field capacity, which is the retention
    // rule the solver itself uses, and soil under the water table is saturated. Pools get
    // what is left.
    let pore = wet_soil(world, w.aquifer_head_m, available);
    let list = basins(world);
    let pooled = fill_basins(world, &list, available - pore);

    let spare = (available - pore - pooled).max(0.0);
    world.aquifer_m3 += charged + spare;
    world.atmosphere_m3 += atmosphere;
    world.ledger.user_in += pore + pooled + charged + spare;
    world.ledger.atmosphere_in += atmosphere;
    world.ledger.user_atmosphere_in += atmosphere;
    world.rebuild_active_sets();
    Hydrated {
        pooled_m3: pooled,
        pore_m3: pore,
        aquifer_m3: charged + spare,
        atmosphere_m3: atmosphere,
        basins: list.len(),
        basins_filled: list
            .iter()
            .filter(|b| b.cells.iter().any(|&i| world.free[i] > 0.0))
            .count(),
        spare_m3: spare,
    }
}

/// Every basin in the world, lowest floor first.
///
/// A basin's floor is a connected sheet of void cells with **no void directly under
/// any of them**: water there cannot get lower, whatever is beside it. The pool then
/// grows upward a level at a time while the void it would spread into still drains
/// nowhere but back into this basin; the first level that can reach lower ground is
/// the spill. A grotto's bowl is a basin like any other: its spill is its mouth.
pub fn basins(world: &World) -> Vec<Basin> {
    let c = world.config();
    let (w, h, d) = (c.width as usize, c.height as usize, c.depth as usize);
    let plane = w * d;
    let material = &world.material;
    let void = |i: usize| !material[i].is_solid();

    // One pass upward. At each level the void is cut into **sheets** — connected in
    // `(x, z)`, `x` wrapping — and each sheet learns the lowest `y` anything on it can
    // reach by going down or sideways, which is the floor of the basin it drains into.
    let mut sheet_of = vec![u32::MAX; plane * h];
    let mut sheet_cells: Vec<Vec<usize>> = Vec::new();
    let mut sheet_low: Vec<u32> = Vec::new();
    let mut sheet_basin: Vec<u32> = Vec::new();
    let mut floors: Vec<u32> = Vec::new();
    let mut comp = vec![u32::MAX; plane];
    for y in 0..h {
        comp.fill(u32::MAX);
        let first = sheet_cells.len();
        for col in 0..plane {
            if !void(y * plane + col) || comp[col] != u32::MAX {
                continue;
            }
            let sid = sheet_cells.len();
            comp[col] = sid as u32;
            let mut cells = vec![col];
            let mut k = 0;
            while k < cells.len() {
                let cur = cells[k];
                k += 1;
                let (x, z) = (cur % w, cur / w);
                let mut around = vec![((x + 1) % w, z), ((x + w - 1) % w, z)];
                if z + 1 < d {
                    around.push((x, z + 1));
                }
                if z > 0 {
                    around.push((x, z - 1));
                }
                for (nx, nz) in around {
                    let nc = nz * w + nx;
                    if comp[nc] == u32::MAX && void(y * plane + nc) {
                        comp[nc] = sid as u32;
                        cells.push(nc);
                    }
                }
            }
            sheet_cells.push(cells.into_iter().map(|col| y * plane + col).collect());
            sheet_low.push(y as u32);
            sheet_basin.push(u32::MAX);
        }
        for sid in first..sheet_cells.len() {
            // What lies under this sheet? The lowest thing it drains to, and whose basin.
            let mut best: Option<(u32, u32)> = None;
            if y > 0 {
                for &i in &sheet_cells[sid] {
                    let b = i - plane;
                    if void(b) {
                        let bs = sheet_of[b] as usize;
                        let key = (sheet_low[bs], sheet_basin[bs]);
                        if best.is_none_or(|cur| key < cur) {
                            best = Some(key);
                        }
                    }
                }
            }
            let (l, basin) = best.unwrap_or_else(|| {
                // Nothing under any of it: water here cannot get lower. A basin floor.
                floors.push(sid as u32);
                (y as u32, floors.len() as u32 - 1)
            });
            sheet_low[sid] = l;
            sheet_basin[sid] = basin;
            for &i in &sheet_cells[sid] {
                sheet_of[i] = sid as u32;
            }
        }
    }

    // The ground each basin catches: void resting directly on solid, by where it drains.
    let mut catchment = vec![0usize; floors.len()];
    for i in 0..plane * h {
        if void(i) && (i < plane || material[i - plane].is_solid()) {
            let b = sheet_basin[sheet_of[i] as usize];
            if (b as usize) < catchment.len() {
                catchment[b as usize] += 1;
            }
        }
    }

    // Grow each pool upward while the void above it still drains nowhere lower. The
    // first level that can reach lower ground is the spill. The ring's **lowest** basin
    // has nothing lower anywhere, so it has no rim: it is the ring's own lake, and only
    // the inventory stops it.
    let mut out: Vec<Basin> = Vec::with_capacity(floors.len());
    for (b, &sid0) in floors.iter().enumerate() {
        let floor_y = sheet_low[sid0 as usize];
        let mut cells = sheet_cells[sid0 as usize].clone();
        let mut level = cells.clone();
        let mut y = floor_y as usize;
        let spill_y = loop {
            if y + 1 >= h {
                break h as u32;
            }
            let mut above: Vec<u32> = Vec::new();
            for &i in &level {
                let up = i + plane;
                if void(up) {
                    let sid = sheet_of[up];
                    if !above.contains(&sid) {
                        above.push(sid);
                    }
                }
            }
            if above.is_empty() || above.iter().any(|&s| sheet_low[s as usize] < floor_y) {
                break (y + 1) as u32;
            }
            level = above
                .iter()
                .flat_map(|&s| sheet_cells[s as usize].iter().copied())
                .collect();
            cells.extend_from_slice(&level);
            y += 1;
        };
        out.push(Basin {
            cells,
            floor_y,
            spill_y,
            catchment: catchment[b].max(1),
        });
    }
    out.sort_by_key(|b| (b.floor_y, b.cells.first().copied().unwrap_or(0)));
    // Two pools that overlap are one pool as far as the water is concerned; give each
    // cell to the lowest basin that claims it, so no volume is counted twice.
    let mut claimed = vec![false; plane * h];
    for b in &mut out {
        b.cells
            .retain(|&i| !std::mem::replace(&mut claimed[i], true));
    }
    out.retain(|b| !b.cells.is_empty());
    out
}

/// Share the pool budget between the basins by catchment, capped at what each one holds
/// below its spill, and write one flat head into each. Returns the volume placed.
fn fill_basins(world: &mut World, list: &[Basin], budget: f64) -> f64 {
    if !(budget > 0.0) || list.is_empty() {
        return 0.0;
    }
    let vol = world.config().voxel_volume();
    let cap: Vec<f64> = list.iter().map(|b| b.cells.len() as f64 * vol).collect();
    let mut share = vec![0.0f64; list.len()];
    let mut left = budget;
    // Water-filling: proportional to catchment, redistributing what a full basin refuses,
    // lowest basin first when the last drop has to go somewhere.
    for _ in 0..8 {
        let open: Vec<usize> = (0..list.len()).filter(|&i| share[i] < cap[i]).collect();
        let weight: f64 = open.iter().map(|&i| list[i].catchment as f64).sum();
        if left <= 1e-15 || open.is_empty() || weight <= 0.0 {
            break;
        }
        let mut used = 0.0;
        for &i in &open {
            let want = left * list[i].catchment as f64 / weight;
            let take = want.min(cap[i] - share[i]);
            share[i] += take;
            used += take;
        }
        left -= used;
        if used <= 1e-15 {
            break;
        }
    }
    let mut placed = 0.0;
    for (b, &want) in list.iter().zip(share.iter()) {
        if !(want > 0.0) {
            continue;
        }
        // One flat head: whole levels fill, and whatever is left spreads evenly over the
        // level it stops on rather than standing in a column at one end of the basin.
        let mut cells = b.cells.clone();
        cells.sort_by_key(|&i| world.config().coords(i).1);
        let mut rest = want;
        let mut k = 0;
        while k < cells.len() && rest > 0.0 {
            let y = world.config().coords(cells[k]).1;
            let end = cells[k..]
                .iter()
                .position(|&i| world.config().coords(i).1 != y)
                .map_or(cells.len(), |n| k + n);
            let n = (end - k) as f64;
            let fill = (rest / (n * vol)).min(1.0);
            for &i in &cells[k..end] {
                // Raised *to* the head, never lowered: a world that already holds water
                // keeps it, and only the difference is charged.
                let add = (fill - world.free[i]).max(0.0);
                world.free[i] += add;
                rest -= add * vol;
                placed += add * vol;
            }
            k = end;
        }
    }
    placed
}

/// Wet the ground: every porous voxel to its material's **field capacity**, and every one
/// under the water table to saturation. Returns the volume it took.
///
/// The retention rule is the material's own ([`crate::Material::field_capacity`], the same
/// number the solver's drainage stops at), so this is the state a soil that has been
/// rained on and has drained is already in — not a moisture model of this module's own
/// invention. When the inventory cannot pay for all of it the whole field is scaled down
/// by one factor, so the ground comes out uniformly drier rather than half wet and half
/// bone dry in index order.
fn wet_soil(world: &mut World, head_m: f64, budget: f64) -> f64 {
    let c = world.config().clone();
    if !(budget > 0.0) {
        return 0.0;
    }
    let unit = c.voxel_volume();
    let target = |world: &World, i: usize| -> f64 {
        let m = world.material[i];
        if m.pore_capacity() <= 0.0 {
            return 0.0;
        }
        let top = f64::from(c.coords(i).1 + 1) * c.voxel_m;
        if top <= head_m {
            1.0
        } else {
            m.field_capacity()
        }
    };
    let demand: f64 = (0..c.cells())
        .map(|i| {
            (target(world, i) - world.pore[i]).max(0.0) * world.material[i].pore_capacity() * unit
        })
        .sum();
    if demand <= 0.0 {
        return 0.0;
    }
    let scale = (budget / demand).min(1.0);
    let mut used = 0.0;
    for i in 0..c.cells() {
        let add = (target(world, i) - world.pore[i]).max(0.0) * scale;
        if add > 0.0 {
            world.pore[i] += add;
            used += add * world.material[i].pore_capacity() * unit;
        }
    }
    used
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, Material, SETTLE_WINDOW};

    /// A flat ring of rock `top` voxels deep, with air over it.
    fn flat(width: u32, depth: u32, top: u32) -> World {
        let config = Config {
            width,
            height: 12,
            depth,
            voxel_m: 0.25,
            ..Config::default()
        };
        let mut world = World::empty(config.clone());
        for z in 0..depth {
            for x in 0..width as i64 {
                for y in 1..=top {
                    world.material[config.index(x, y, z)] = Material::Soil;
                }
            }
        }
        world
    }

    fn dig(world: &mut World, xs: std::ops::Range<i64>, floor: u32) {
        let c = world.config().clone();
        for x in xs {
            for z in 0..c.depth {
                for y in floor..c.height {
                    if world.material[c.index(x, y, z)].is_solid() {
                        world.material[c.index(x, y, z)] = Material::Air;
                    }
                }
            }
        }
    }

    /// Two pits in a flat ring, one a voxel deeper than the other. The inventory is the
    /// whole of what the world has: pools, pore, aquifer and atmosphere sum to it, and
    /// each pit stands at one level.
    #[test]
    fn the_inventory_is_every_store_and_each_basin_holds_one_head() {
        let mut world = flat(24, 4, 6);
        dig(&mut world, 2..6, 3); // deep pit: floor at y = 3
        dig(&mut world, 14..18, 5); // shallow pit: floor at y = 5
        world.rebuild_active_sets();

        let list = basins(&world);
        assert_eq!(list.len(), 2, "two pits, two basins: {list:?}");
        assert_eq!(list[0].floor_y, 3, "the deeper pit is listed first");
        assert_eq!(list[1].floor_y, 5);
        assert_eq!(
            list[1].spill_y, 7,
            "the shallow pit spills over its rim into the deep one"
        );
        assert_eq!(
            list[0].spill_y,
            world.config().height,
            "the ring's lowest basin has no rim: only the inventory stops it"
        );

        let w = Water {
            inventory_m: 0.2,
            atmosphere_fraction: 0.1,
            aquifer_head_m: 0.1,
        };
        let out = hydrate(&mut world, &w);
        let c = world.config().clone();
        let footprint = c.width as f64 * c.depth as f64 * c.cell_area();
        let total = w.inventory_m * footprint;
        assert!(
            (out.total_m3() - total).abs() <= 1e-9 * total,
            "the four stores are the inventory: {out:?} against {total}"
        );
        assert!(out.pooled_m3 > 0.0, "some of it is standing water");
        assert_eq!(out.basins_filled, 2, "both pits caught their own runoff");

        for i in 0..c.cells() {
            if world.free[i] > 0.0 {
                let (_, y, _) = c.coords(i);
                assert!(y < 7, "nothing stands above a rim at y = {y}");
            }
        }
        for b in &list {
            let mut levels: Vec<(u32, f64)> = b
                .cells
                .iter()
                .filter(|&&i| world.free[i] > 0.0)
                .map(|&i| (c.coords(i).1, world.free[i]))
                .collect();
            levels.sort_by_key(|&(y, _)| y);
            let top = levels.last().copied().expect("a filled basin").0;
            for &(y, f) in &levels {
                if y < top {
                    assert!((f - 1.0).abs() < 1e-9, "one flat head, not a staircase");
                }
            }
        }
    }

    /// Water wets the ground before it stands on it. An inventory too thin to bring the
    /// soil to field capacity has nothing left to pool, and says so rather than putting a
    /// pond on dry ground.
    #[test]
    fn a_thin_inventory_wets_the_ground_and_pools_nothing() {
        let mut world = flat(24, 4, 6);
        dig(&mut world, 2..6, 3);
        dig(&mut world, 14..18, 5);
        world.rebuild_active_sets();
        let out = hydrate(
            &mut world,
            &Water {
                inventory_m: 0.05,
                atmosphere_fraction: 0.0,
                aquifer_head_m: 0.0,
            },
        );
        assert_eq!(out.pooled_m3, 0.0, "nothing to spare for a pond: {out:?}");
        assert!(out.pore_m3 > 0.0, "the ground took it: {out:?}");
        assert!(world.free.iter().all(|&f| f == 0.0));
        let total = 0.05 * 24.0 * 4.0 * world.config().cell_area();
        assert!((out.total_m3() - total).abs() <= 1e-9 * total, "{out:?}");
    }

    /// A dry recipe leaves the world exactly as it was.
    #[test]
    fn no_inventory_is_no_water() {
        let mut world = flat(8, 4, 4);
        let out = hydrate(&mut world, &Water::DRY);
        assert_eq!(out, Hydrated::default());
        assert!(world.free.iter().all(|&f| f == 0.0));
    }
}
