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

/// Most of the inventory left after the lake that may go into the water table. The table
/// has to reach the lake floor for the lake to hold, and on these rings that is the
/// largest single charge in the budget; this keeps it from taking the pore and the other
/// pools with it.
const LAKE_TABLE_SHARE: f64 = 0.85;

/// The ring's **lake**: the lowest basin the sky reaches all of, and the water standing
/// in it. Derived geometry, like [`basins`] — read off the material and free arrays, so
/// it is true of any world at any time and nothing stores it.
///
/// A basin qualifies only when **every** one of its floor cells has open sky over it. A
/// gallery bowl cut below the lowest ground reaches the sky through its mouth and is the
/// ring's deepest basin, so it takes the catchment and hides every drop under rock; that
/// is what the deployed panel did (Wrysk, 2026-09-21). Its floor is roofed, so it is not
/// a lake.
#[derive(Clone, Debug, PartialEq)]
pub struct Pool {
    /// Its wet cells.
    pub cells: Vec<usize>,
    /// Its **open-water surface**: a wet cell with void, and no water, directly over it.
    pub surface_cells: Vec<usize>,
    /// The share of that surface the camera can read, square metres — and in cells, which
    /// is what a gate about *readability* wants: one cell is one cell at any voxel size.
    pub visible_m2: f64,
    pub visible_cells: usize,
    /// The level the surface stands at: one above the highest wet cell, or the floor of
    /// an empty lake.
    pub level_y: u32,
    /// The basin's floor.
    pub floor_y: u32,
    /// Water standing in it, cubic metres.
    pub volume_m3: f64,
}

/// The ring's **lake**: the lowest pool, which is the one the outlet's datum is seated
/// on. Empty geometry when no basin has sky over the whole of its floor — a world with
/// nowhere for open water to be.
pub fn lake(world: &World) -> Pool {
    let empty = Pool {
        cells: Vec::new(),
        surface_cells: Vec::new(),
        visible_m2: 0.0,
        visible_cells: 0,
        level_y: 0,
        floor_y: 0,
        volume_m3: 0.0,
    };
    let Some(basin) = basins(world).into_iter().find(|b| open_to_sky(world, b)) else {
        return empty;
    };
    fill_of(world, &basin)
}

/// The water standing in one basin, and how much of its surface the camera reads.
fn fill_of(world: &World, basin: &Basin) -> Pool {
    let c = world.config();
    let plane = c.width as usize * c.depth as usize;
    let cells: Vec<usize> = basin
        .cells
        .iter()
        .copied()
        .filter(|&i| world.free[i] > 0.0)
        .collect();
    let volume_m3 = cells.iter().map(|&i| world.free[i]).sum::<f64>() * c.voxel_volume();
    // The **top of the water in each column**, which is what the camera reads: the
    // highest wet cell whose own roof is open. Not "the cell above is dry" — a pool with
    // a waterfall landing in it has water in the cell above every one of its own, and
    // that pool has a surface like any other. A cell under rock is not a surface at all.
    let mut top: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    for &i in &cells {
        let col = i % plane;
        let higher = top.get(&col).is_none_or(|&best| i > best);
        if higher {
            top.insert(col, i);
        }
    }
    let mut surface_cells: Vec<usize> = top
        .into_values()
        .filter(|&i| {
            let up = i + plane;
            up >= c.cells() || !world.material[up].is_solid()
        })
        .collect();
    surface_cells.sort_unstable();
    let visible_cells = surface_cells
        .iter()
        .filter(|&&i| {
            let (x, y, z) = c.coords(i);
            crate::hollows::floor_is_visible(world, i64::from(x), y, z)
        })
        .count();
    let visible_m2 = visible_cells as f64 * c.cell_area();
    let level_y = cells
        .iter()
        .map(|&i| c.coords(i).1)
        .max()
        .map_or(basin.floor_y, |y| y + 1);
    Pool {
        cells,
        surface_cells,
        visible_m2,
        visible_cells,
        level_y,
        floor_y: basin.floor_y,
        volume_m3,
    }
}

/// Every open-sky pool in the world that is actually holding water, lowest first.
///
/// The lake is the first of them; the rest are the tiers' pools above it. A basin with no
/// water in it is not a pool, and a roofed one is never a pool however deep — the same
/// rule [`lake`] uses, for the same reason: the camera cannot read what is under rock.
pub fn pools(world: &World) -> Vec<Pool> {
    basins(world)
        .into_iter()
        .filter(|b| open_to_sky(world, b))
        .map(|b| fill_of(world, &b))
        .filter(|p| p.volume_m3 > 0.0)
        .collect()
}

/// How many of `pools` stand **above** `lake` and show enough of themselves to read: the
/// tier-pool half of the host's seed gate. Four surface cells is a quarter of a square
/// metre at 0.25 m — under that a pool is a glint, not a water feature.
pub fn tier_pools(pools: &[Pool], lake_level_y: u32) -> usize {
    pools
        .iter()
        .filter(|p| p.level_y > lake_level_y && p.visible_cells >= MIN_POOL_CELLS)
        .count()
}

/// Visible surface cells a pool needs before the gate counts it.
pub const MIN_POOL_CELLS: usize = 4;

/// Whether the sky reaches **every** floor cell of this basin: nothing solid anywhere
/// above it in its own column. One roofed floor cell and the basin is a sump.
fn open_to_sky(world: &World, b: &Basin) -> bool {
    let c = world.config();
    let plane = c.width as usize * c.depth as usize;
    b.cells
        .iter()
        .filter(|&&i| c.coords(i).1 == b.floor_y)
        .all(|&i| {
            let mut up = i + plane;
            while up < c.cells() {
                if world.material[up].is_solid() {
                    return false;
                }
                up += plane;
            }
            true
        })
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
    /// The lake: what went into it, how many cells it fills, and how much of its surface
    /// the camera can read.
    pub lake_m3: f64,
    pub lake_cells: usize,
    pub lake_visible_m2: f64,
    /// The water table the pore rule was run against. Below the recipe's
    /// `aquifer_head_m` when the inventory could not pay for it.
    pub aquifer_head_m: f64,
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
    let available = total - atmosphere;

    // **The lake first.** A ring with no water anybody can see is not a habitat, whatever
    // its ledger says, and the panel proved that the deepest basin is not the one to fill:
    // fill order was catchment share, and a roofed gallery bowl below the lowest ground
    // took it all (Wrysk, 2026-09-21). So the lowest basin the sky reaches all of is
    // filled first, to one flat head at the datum the outlet marks, and only then does the
    // rest of the inventory go where it used to.
    let list = basins(world);
    // No datum, no lake: a world whose generator never named an outlet — every hand-built
    // fixture — keeps the catchment-share fill it had, with its lowest basin an ordinary
    // basin like any other.
    let level_y = world.outlet_cell.map(|(_, y, _)| y);
    let lake_at = level_y.and_then(|_| list.iter().position(|b| open_to_sky(world, b)));
    let (lake_m3, lake_floor_y) = match (lake_at, level_y) {
        (Some(k), Some(level)) => (
            fill_to_level(world, &list[k], level, available),
            list[k].floor_y,
        ),
        _ => (0.0, 0),
    };

    // **The bed under the lake is saturated**, so the lake does not soak away into dry
    // soil the moment it is poured. Locally, under the lake's own columns — *not* by
    // raising the world's water table to the lake floor, which is what the plan's words
    // suggest and what the arithmetic refuses: a metre of head costs
    // `footprint x porosity` cubic metres over the **whole ring**, so a lake floor two
    // metres up would want more than the entire inventory and leave nothing for pore or
    // for any other pool (measured: `default` at 48 wide took every drop and came out
    // dry-locked). The lake's own bed is a few square metres and costs almost nothing.
    let after_lake = (available - lake_m3).max(0.0);
    let bed = match lake_at {
        Some(k) if lake_m3 > 0.0 => seal_lake_bed(world, &list[k], after_lake),
        _ => 0.0,
    };

    // **The water table is raised to the lake floor**, because nothing else holds a lake
    // up. Saturating the bed is not enough on its own: soil between the table and the
    // lake drains downward whatever its own fill, so a lake standing five voxels over the
    // table empties into the aquifer through its own bed — measured on `default` seed 1,
    // where 2.33 m³ of visible lake was gone inside ten minutes and the pore stores never
    // moved.
    //
    // A metre of head costs `footprint x aquifer_porosity` over the **whole ring**, so
    // this is the expensive line in the budget and the reason the presets carry the
    // inventory they do. It is capped so it can never starve the pore and the other
    // pools: if the lake floor is out of reach, the recipe's own head stands and the lake
    // is reported as the shallow thing it will become.
    let table_ceiling = LAKE_TABLE_SHARE * (after_lake - bed);
    let lake_head_m = f64::from(lake_floor_y) * c.voxel_m;
    let want_head = if lake_m3 > 0.0 && c.aquifer_volume_for_head(lake_head_m) <= table_ceiling {
        w.aquifer_head_m.max(lake_head_m)
    } else {
        w.aquifer_head_m
    };
    let charged = c.aquifer_volume_for_head(want_head).min(after_lake - bed);
    let head_used = c.aquifer_head_m(charged);

    // **Pore, then the other pools.** Water wets the ground before it stands on it: soil
    // that has been rained on and has drained holds its field capacity, which is the
    // retention rule the solver itself uses, and soil under the water table is saturated.
    let pore = wet_soil(world, head_used, after_lake - bed - charged);
    let others: Vec<Basin> = list
        .iter()
        .enumerate()
        .filter(|(k, _)| Some(*k) != lake_at)
        .map(|(_, b)| b.clone())
        .collect();
    let pooled = fill_basins(world, &others, after_lake - bed - charged - pore);

    // **The surplus goes aloft, not underground.** Whatever the geometry would not take
    // has to go somewhere, and the aquifer is the wrong somewhere: its head is what the
    // lake stands on, so surplus buried there lifts the water table above the lake floor
    // and the lake rises with it — on `small` at a 1.2 m inventory that flooded half the
    // ring, a 30 m² lake standing nine voxels over its own datum. In the sky it is the
    // cycle's own store, it rains back on schedule, and the table stays where the lake
    // needs it. An open-budget world has no sky to put it in, so there it still sinks.
    let spare = (after_lake - bed - charged - pore - pooled).max(0.0);
    let (to_aquifer, to_sky) = if c.closed_water_budget {
        (0.0, spare)
    } else {
        (spare, 0.0)
    };
    world.aquifer_m3 += charged + to_aquifer;
    world.atmosphere_m3 += atmosphere + to_sky;
    world.ledger.user_in += lake_m3 + bed + pore + pooled + charged + spare;
    world.ledger.atmosphere_in += atmosphere + to_sky;
    world.ledger.user_atmosphere_in += atmosphere + to_sky;
    world.rebuild_active_sets();
    let l = lake(world);
    Hydrated {
        pooled_m3: pooled + lake_m3,
        pore_m3: pore + bed,
        aquifer_m3: charged + to_aquifer,
        atmosphere_m3: atmosphere + to_sky,
        basins: list.len(),
        basins_filled: list
            .iter()
            .filter(|b| b.cells.iter().any(|&i| world.free[i] > 0.0))
            .count(),
        spare_m3: spare,
        lake_m3,
        lake_cells: l.cells.len(),
        lake_visible_m2: l.visible_m2,
        aquifer_head_m: c.aquifer_head_m(world.aquifer_m3),
    }
}

/// Saturate the porous ground directly under a lake, so the water it holds is not
/// immediately drawn down into dry soil. Returns the volume it took.
fn seal_lake_bed(world: &mut World, b: &Basin, budget: f64) -> f64 {
    let c = world.config().clone();
    let plane = c.width as usize * c.depth as usize;
    let unit = c.voxel_volume();
    let mut used = 0.0;
    let floor: Vec<usize> = b
        .cells
        .iter()
        .copied()
        .filter(|&i| c.coords(i).1 == b.floor_y)
        .collect();
    for i in floor {
        let mut below = i;
        while below >= plane {
            below -= plane;
            let cap = world.material[below].pore_capacity();
            if cap <= 0.0 {
                continue;
            }
            let add = ((1.0 - world.pore[below]).max(0.0) * cap * unit).min(budget - used);
            if add <= 0.0 {
                return used;
            }
            world.pore[below] += add / (cap * unit);
            used += add;
        }
    }
    used
}

/// Fill one basin to a flat head at `level_y` — it holds the cells below that line — out
/// of `budget`. Returns the volume placed. Raised *to* the level, never lowered.
fn fill_to_level(world: &mut World, b: &Basin, level_y: u32, budget: f64) -> f64 {
    let vol = world.config().voxel_volume();
    let mut cells: Vec<usize> = b
        .cells
        .iter()
        .copied()
        .filter(|&i| world.config().coords(i).1 < level_y.min(b.spill_y))
        .collect();
    cells.sort_by_key(|&i| world.config().coords(i).1);
    let mut rest = budget;
    let mut placed = 0.0;
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
            let add = (fill - world.free[i]).max(0.0);
            world.free[i] += add;
            rest -= add * vol;
            placed += add * vol;
        }
        k = end;
    }
    placed
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
    use crate::{Config, Material, SETTLE_REST, SETTLE_WINDOW};

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
            ..Water::DRY
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
                ..Water::DRY
            },
        );
        assert_eq!(out.pooled_m3, 0.0, "nothing to spare for a pond: {out:?}");
        assert!(out.pore_m3 > 0.0, "the ground took it: {out:?}");
        assert!(world.free.iter().all(|&f| f == 0.0));
        let total = 0.05 * 24.0 * 4.0 * world.config().cell_area();
        assert!((out.total_m3() - total).abs() <= 1e-9 * total, "{out:?}");
    }

    /// The roofed bowl of `hollows`' own fixture: `hydrate` finds it like any other
    /// basin, and the water is still in it after the solver has had its say.
    ///
    /// (Restored: this test and the settle one below were cut by an editing slip in
    /// `e260844` and are back with the behaviour they always asserted.)
    #[test]
    fn a_grotto_bowl_is_a_basin_and_keeps_what_hydrate_puts_in_it() {
        let config = Config {
            width: 12,
            height: 10,
            depth: 2,
            voxel_m: 0.25,
            ..Config::default()
        };
        let mut world = World::empty(config.clone());
        for z in 0..config.depth {
            for x in 0..config.width as i64 {
                for y in 1..=5 {
                    world.material[config.index(x, y, z)] = Material::Bedrock;
                }
            }
            // A bowl from x = 3 to 8, floor at y = 3, roofed at y = 7, mouth at x = 3.
            for x in 3..=8i64 {
                for y in 4..=6 {
                    world.material[config.index(x, y, z)] = Material::Air;
                }
            }
            for x in 4..=8i64 {
                world.material[config.index(x, 7, z)] = Material::Bedrock;
            }
        }
        world.rebuild_active_sets();

        let out = hydrate(
            &mut world,
            &Water {
                inventory_m: 0.05,
                ..Water::DRY
            },
        );
        assert!(out.pooled_m3 > 0.0, "the bowl took water: {out:?}");
        let roofed: Vec<usize> = (4..=8i64)
            .flat_map(|x| (0..config.depth).map(move |z| (x, z)))
            .map(|(x, z)| config.index(x, 4, z))
            .collect();
        assert!(
            roofed.iter().any(|&i| world.free[i] > 0.0),
            "the water went under the roof"
        );
        let settled = world.settle(300);
        let v = world.view();
        let held: f64 = (4..=8i64)
            .flat_map(|x| (0..config.depth).map(move |z| (x, z)))
            .map(|(x, z)| v.water_depth_m(x, 3, z))
            .sum();
        assert!(held > 0.0, "the bowl let the water out: {settled:?}");
    }

    /// Settling is measured. A world already at rest says so **before** the convergence
    /// window is even full; a world with a column of water standing over a pit finds its
    /// level and keeps the volume; a world with water aloft and no pool anywhere is
    /// stable *and* dry-locked, and one with a pool is not.
    #[test]
    fn settle_reports_rest_convergence_and_a_dry_lock() {
        let mut still = flat(8, 2, 4);
        still.rebuild_active_sets();
        let at_rest = still.settle(600);
        assert!(at_rest.converged, "a dry flat ring is already settled");
        assert!(
            at_rest.ticks < SETTLE_WINDOW,
            "rest is reported without stepping the whole window: {at_rest:?}"
        );
        assert!(at_rest.ticks <= SETTLE_REST + 1, "{at_rest:?}");

        let config = Config {
            width: 8,
            height: 10,
            depth: 2,
            voxel_m: 0.25,
            ..Config::default()
        };
        let mut world = World::empty(config.clone());
        for z in 0..config.depth {
            for x in 0..config.width as i64 {
                for y in 1..=4 {
                    world.material[config.index(x, y, z)] = Material::Bedrock;
                }
            }
            for x in 2..=4i64 {
                world.material[config.index(x, 4, z)] = Material::Air;
            }
        }
        world.rebuild_active_sets();
        let poured = 3.0 * config.voxel_volume();
        let taken = world.apply(crate::Command::AddWater {
            x: 3,
            y: 8,
            z: 0,
            volume_m3: poured,
        });
        assert!(taken > 0.0);
        let out = world.settle(600);
        assert!(out.converged && out.ticks < 600, "{out:?}");
        assert!(
            (out.pooled_m3 - taken).abs() <= 0.01 * taken,
            "the pit kept what fell into it: {out:?} against {taken}"
        );
        assert!(
            !out.dry_locked,
            "a world with a pool in it is not locked dry: {out:?}"
        );

        // Water aloft, nowhere for it to fall to that it can stay: stable and still not a
        // habitat. Rain is off, so the shower that starts delivers nothing.
        let mut dry = World::empty(Config {
            width: 8,
            height: 8,
            depth: 2,
            closed_water_budget: true,
            initial_atmosphere_m3: 1.0,
            ..Config::default()
        });
        dry.rebuild_active_sets();
        let locked = dry.settle(600);
        assert!(
            locked.converged && locked.dry_locked,
            "stable and dry is not a habitat: {locked:?}"
        );
    }

    /// A staged world runs the **closed** cycle its recipe asked for: the budget is on,
    /// the atmosphere holds exactly its share of the inventory, the snapshot survives the
    /// validator that refuses an open world holding water aloft, and the books balance
    /// after a minute of weather.
    #[test]
    fn a_staged_world_runs_the_closed_cycle_its_recipe_asked_for() {
        let recipe = crate::Recipe::DEFAULT;
        let mut world = World::new(Config {
            width: 24,
            height: 16,
            depth: 4,
            voxel_m: 0.25,
            seed: 1,
            landform: crate::Landform::Staged(recipe),
            ..Config::default()
        });
        let c = world.config().clone();
        assert!(c.closed_water_budget, "the recipe turned the cycle on");
        assert_eq!(c.rain_m_per_s, recipe.water.rain_m_per_s);
        assert_eq!(c.evaporation_m_per_s, recipe.water.evaporation_m_per_s);
        assert!(
            c.evaporation_m_per_s > 0.0 && c.evaporation_m_per_s < c.rain_m_per_s,
            "evaporation is on, and below the shower rate"
        );

        let footprint = c.width as f64 * c.depth as f64 * c.cell_area();
        // Its share, **and** whatever the ground would not take: the surplus goes aloft
        // rather than into the aquifer, where it would lift the table over the lake.
        let want = recipe.water.inventory_m * footprint * recipe.water.atmosphere_fraction;
        assert!(
            world.atmosphere_m3 >= want - 1e-9,
            "the atmosphere holds at least its share: {} against {want}",
            world.atmosphere_m3
        );
        assert!(
            world.atmosphere_m3 <= recipe.water.inventory_m * footprint,
            "and never more than the whole inventory"
        );
        // The books still add up with the surplus aloft: four stores, one inventory.
        let total = recipe.water.inventory_m * footprint;
        let stores = world.view().stored_m3() + world.atmosphere_m3;
        assert!(
            (stores - total).abs() <= 1e-9 * total,
            "every store is the inventory: {stores} against {total}"
        );

        let bytes = world.save();
        World::load(&bytes).expect("a closed staged world is a valid snapshot");

        for _ in 0..60 {
            world.step();
        }
        let v = world.view();
        assert!(
            v.total_residual().abs() <= 1e-9 * v.total_water_m3().max(1.0),
            "residual {:e} after a minute of weather",
            v.total_residual()
        );
    }

    /// **Study, not a test.** What does the lake cost in grottos? The datum forbids any
    /// hollow floor under the waterline, so a deeper lake is a drier cave system. Sweep
    /// the depth on eight seeds of each preset and read both sides of the trade at once.
    #[test]
    #[ignore = "study: run by name"]
    fn the_lake_depth_sweep() {
        for p in crate::PRESETS {
            let want = p.recipe.water.min_lake_m2;
            for depth_m in [0.25, 0.375, 0.5, 0.625, 0.75] {
                let mut recipe = p.recipe;
                recipe.water.lake_depth_m = depth_m;
                let (mut pass, mut hollows, mut barren) = (0usize, 0usize, 0usize);
                let mut areas: Vec<f64> = Vec::new();
                for seed in 1..=8u64 {
                    let world = World::new(Config {
                        seed,
                        landform: crate::Landform::Staged(recipe),
                        ..p.config()
                    });
                    let l = lake(&world);
                    pass += usize::from(l.visible_m2 >= want);
                    areas.push(l.visible_m2);
                    let h = crate::hollows::find(&world).len();
                    hollows += h;
                    barren += usize::from(h == 0);
                }
                areas.sort_by(f64::total_cmp);
                let at = |bar: f64| areas.iter().filter(|&&a| a >= bar).count();
                println!(
                    "{:7} depth {depth_m:5.3} m ({:2} vx): visible {:?}; pass at 2/3/4.5/6 m² \
                     = {}/{}/{}/{}; {hollows} hollows, {barren} barren; (bar {want}: {pass})",
                    p.name,
                    (depth_m / p.voxel_m).round() as i32,
                    areas
                        .iter()
                        .map(|a| (a * 10.0).round() / 10.0)
                        .collect::<Vec<_>>(),
                    at(2.0),
                    at(3.0),
                    at(4.5),
                    at(6.0),
                );
            }
        }
    }

    /// **Study, not a test.** Is there a lake, can the camera see it, and does it stay?
    /// Eight seeds of each preset: hydrate and settle, read the lake, judge it against
    /// the recipe's own bar, then open the outlet and run ten simulated minutes with the
    /// cycle on to see whether the level holds.
    #[test]
    #[ignore = "study: run by name"]
    fn the_lake_on_eight_seeds_of_every_preset() {
        for p in crate::PRESETS {
            let want = p.recipe.water.min_lake_m2;
            let (mut passed, mut worst_drift, mut areas) = (0, 0i64, Vec::new());
            let mut carved = (0usize, 0usize, 0usize);
            for seed in 1..=8u64 {
                let mut world = World::new(Config { seed, ..p.config() });
                let settled = world.settle(600);
                let l = lake(&world);
                let datum = world.outlet_cell().map_or(0, |(_, y, _)| y);
                let ok = l.visible_m2 >= want;
                passed += usize::from(ok);
                areas.push(l.visible_m2);
                let (_, _, report) = crate::generate::staged_terrain(world.config(), &p.recipe);
                let h = crate::hollows::find(&world).len();
                carved.0 += report.carved.undercuts;
                carved.1 += report.carved.galleries;
                carved.2 += h;

                world.apply(crate::Command::SetOutlet { open: true });
                for _ in 0..12_000 {
                    world.step();
                }
                let after = lake(&world);
                let drift = i64::from(after.level_y) - i64::from(l.level_y);
                // Only the seeds the gate would keep. A rejected seed has no lake to
                // drift: `lake` picks whatever basin is lowest and open, and on a ring
                // with no pond that is a different basin ten minutes later.
                if ok {
                    worst_drift = worst_drift.max(drift.abs());
                }
                println!(
                    "  {} seed {seed}: floor {} datum {datum} level {} -> {} ({drift:+}), \
                     {:.2} m³ over {:.1} m², {:.1} m² visible {} (need {want:.1}); \
                     settle {} ticks {}",
                    p.name,
                    l.floor_y,
                    l.level_y,
                    after.level_y,
                    l.volume_m3,
                    l.surface_cells.len() as f64 * p.voxel_m * p.voxel_m,
                    l.visible_m2,
                    if ok { "PASS" } else { "fail" },
                    settled.ticks,
                    if settled.converged {
                        "converged"
                    } else {
                        "at the cap"
                    },
                );
            }
            areas.sort_by(f64::total_cmp);
            println!(
                "=== {}: depth {} m, bar {want} m²: {passed}/8 pass, visible {:.1}..{:.1} m², \
                 worst level drift {worst_drift} voxel(s) in ten minutes (passing seeds); \
                 over 8 seeds \
                 {} undercuts / {} galleries carved, {} habitable hollows",
                p.name,
                p.recipe.water.lake_depth_m,
                areas.first().copied().unwrap_or(0.0),
                areas.last().copied().unwrap_or(0.0),
                carved.0,
                carved.1,
                carved.2,
            );
        }
    }

    /// **Study, not a test.** The weather, on every preset: settle, open the outlet the
    /// way the host does, then run an hour of simulated time and report every shower —
    /// when it was due, when it fell, whether the store held it back, what it delivered
    /// and how long it rained — plus what the store did and how fast it refills.
    #[test]
    #[ignore = "study: run by name"]
    fn an_hour_of_weather_on_every_preset() {
        const MINUTES: u64 = 60;
        let ticks = MINUTES * 60 * u64::from(crate::TICK_HZ);
        for p in crate::PRESETS {
            let mut world = World::new(Config {
                seed: 1,
                ..p.config()
            });
            let settled = world.settle(600);
            world.apply(crate::Command::SetOutlet { open: true });
            let start_tick = world.tick();
            let aloft0 = world.atmosphere_m3();
            let in0 = world.view().ledger.atmosphere_in;

            let (mut lo, mut hi) = (aloft0, aloft0);
            let mut showers: Vec<(u64, u64, bool, f64, f64)> = Vec::new();
            let (mut count, mut open_at, mut open_out, mut due_at) = (0u64, 0u64, 0.0, 0u64);
            let mut held = false;
            for _ in 0..ticks {
                let due = world.next_shower_tick();
                let overdue = world.tick() >= due && world.shower_left_m3() <= 0.0;
                world.step();
                let v = world.view();
                lo = lo.min(world.atmosphere_m3());
                hi = hi.max(world.atmosphere_m3());
                if v.ledger.showers > count {
                    count = v.ledger.showers;
                    open_at = world.tick();
                    open_out = v.ledger.atmosphere_out;
                    due_at = due;
                    held = false;
                } else if overdue && world.shower_left_m3() <= 0.0 {
                    held = true;
                }
                if open_at > 0 && world.shower_left_m3() <= 0.0 && world.tick() > open_at {
                    let secs = (world.tick() - open_at) as f64 / f64::from(crate::TICK_HZ);
                    showers.push((
                        due_at,
                        open_at,
                        held,
                        v.ledger.atmosphere_out - open_out,
                        secs,
                    ));
                    open_at = 0;
                }
            }

            let v = world.view();
            let minutes = (world.tick() - start_tick) as f64 / f64::from(crate::TICK_HZ) / 60.0;
            let gaps: Vec<f64> = showers
                .windows(2)
                .map(|w| (w[1].1 - w[0].1) as f64 / f64::from(crate::TICK_HZ) / 60.0)
                .collect();
            println!(
                "=== {} seed 1 — settle {} ticks, converged {}",
                p.name, settled.ticks, settled.converged
            );
            for (due, at, was_held, delivered, secs) in &showers {
                println!(
                    "  due {due}, fell {at} ({}), {delivered:.4} m³ over {secs:.0} s",
                    if *was_held {
                        "HELD by the floor"
                    } else {
                        "on time"
                    }
                );
            }
            println!(
                "  {} showers in {minutes:.0} min; gaps {:.1}..{:.1} min; store {lo:.2}..{hi:.2} m³; \
                 lift {:.4} m³/min; residual {:.2e} of {:.1} m³",
                showers.len(),
                gaps.iter().cloned().fold(f64::INFINITY, f64::min),
                gaps.iter().cloned().fold(0.0, f64::max),
                (v.ledger.atmosphere_in - in0) / minutes,
                v.total_residual().abs() / v.total_water_m3().max(1.0),
                v.total_water_m3(),
            );
        }
    }

    /// A ring with an **open-sky pit** and a **roofed sump lower than it** — the shape
    /// the deployed panel had, where every drop of pooled water ended up under rock
    /// (Wrysk, 2026-09-21). Ground to `y = 5`; the pit is open from `y = 4`; the cavity
    /// at `y = 2` is roofed and reaches the sky only through a shaft beside it.
    fn pit_and_sump() -> World {
        let c = Config {
            width: 16,
            height: 12,
            depth: 2,
            voxel_m: 0.25,
            ..Config::default()
        };
        let mut w = World::empty(c.clone());
        for z in 0..c.depth {
            for x in 0..c.width as i64 {
                for y in 1..=5 {
                    w.material[c.index(x, y, z)] = if y == 1 {
                        Material::Bedrock
                    } else {
                        Material::Soil
                    };
                }
            }
            // The pit: open sky over it, floor cells at y = 4, spilling at y = 6.
            for x in 3..=7i64 {
                for y in 4..=5 {
                    w.material[c.index(x, y, z)] = Material::Air;
                }
            }
            // The sump: two cells at y = 2 under rock, with a shaft at x = 12.
            for x in 11..=12i64 {
                w.material[c.index(x, 2, z)] = Material::Air;
            }
            for y in 3..=5 {
                w.material[c.index(12, y, z)] = Material::Air;
            }
        }
        w.rebuild_active_sets();
        // The datum package L will place: the pit's rim, two voxels over its floor.
        w.set_outlet_cell(Some((3, 6, 0)));
        w
    }

    /// The lake is filled **first**, and a sump does not get to be the ring's water
    /// feature just because it is deeper.
    #[test]
    fn the_lake_is_filled_first() {
        let mut w = pit_and_sump();
        let c = w.config().clone();
        let out = hydrate(
            &mut w,
            &Water {
                inventory_m: 0.06,
                atmosphere_fraction: 0.0,
                aquifer_head_m: 0.0,
                lake_depth_m: 0.5,
                ..Water::DRY
            },
        );
        assert!(out.lake_m3 > 0.0, "the pit took water: {out:?}");
        assert_eq!(out.lake_cells, 20, "five columns, two rows, two deep");

        for z in 0..c.depth {
            for x in 3..=7i64 {
                for y in 4..=5 {
                    assert!(
                        w.free[c.index(x, y, z)] > 0.0,
                        "the pit stands full at ({x}, {y}, {z})"
                    );
                }
            }
            for x in 11..=12i64 {
                assert_eq!(
                    w.free[c.index(x, 2, z)],
                    0.0,
                    "the sump is dry while the lake is being filled"
                );
            }
        }
        let total = 0.06 * 16.0 * 2.0 * c.cell_area();
        assert!((out.total_m3() - total).abs() <= 1e-9 * total, "{out:?}");
    }

    /// Only open water counts, and only what the camera can read of it.
    #[test]
    fn lake_reports_only_open_water() {
        let mut w = pit_and_sump();
        let c = w.config().clone();
        hydrate(
            &mut w,
            &Water {
                inventory_m: 0.2,
                atmosphere_fraction: 0.0,
                aquifer_head_m: 0.0,
                lake_depth_m: 0.5,
                ..Water::DRY
            },
        );
        let l = lake(&w);
        assert_eq!(l.floor_y, 4, "the lowest basin with sky over its floor");
        for &i in &l.surface_cells {
            let (x, y, z) = c.coords(i);
            assert!(
                (3..=7).contains(&x),
                "a surface cell of the pit, not of the sump: ({x}, {y}, {z})"
            );
            assert!(
                !w.material[i + c.width as usize * c.depth as usize].is_solid(),
                "an open-water surface has void over it"
            );
        }
        let readable = l
            .surface_cells
            .iter()
            .filter(|&&i| {
                let (x, y, z) = c.coords(i);
                crate::hollows::floor_is_visible(&w, i64::from(x), y, z)
            })
            .count();
        assert!(
            (l.visible_m2 - readable as f64 * c.cell_area()).abs() < 1e-12,
            "visible_m2 counts exactly the cells the camera reads"
        );
        assert!(l.visible_m2 > 0.0, "the pit is in shot: {l:?}");
    }

    /// The lake **keeps** its level. Soil under it drains, showers fall on it, and an
    /// hour later it is still a lake.
    #[test]
    fn the_lake_survives_ten_minutes() {
        let mut w = pit_and_sump();
        let c = w.config().clone();
        let level_y = w.outlet_cell().expect("the datum").1;
        hydrate(
            &mut w,
            &Water {
                inventory_m: 0.5,
                atmosphere_fraction: 0.06,
                aquifer_head_m: 1.0,
                closed_cycle: true,
                rain_m_per_s: 3.5e-5,
                evaporation_m_per_s: 3.0e-5,
                shower_trigger_fraction: 0.01,
                shower_volume_m3: 0.05,
                shower_interval_min_s: 60.0,
                shower_interval_max_s: 120.0,
                lake_depth_m: 0.5,
                min_lake_m2: 0.0,
                reentry_m3_per_s: 0.0,
                min_tier_pools: 0,
            },
        );
        let before = lake(&w).level_y;
        for _ in 0..12_000 {
            w.step();
        }
        let after = lake(&w);
        assert!(
            after.level_y.abs_diff(level_y) <= 1,
            "the lake stands within a voxel of its datum after ten minutes: \
             {} against {level_y} (it began at {before})",
            after.level_y
        );
        assert!(after.visible_m2 > 0.0, "and it is still visible: {after:?}");
        let v = w.view();
        assert!(v.total_residual().abs() <= 1e-9 * v.total_water_m3().max(1.0));
        let _ = c;
    }

    /// A rock pool on a shelf with a notch over a one-voxel drop into a lower basin that
    /// holds the outlet on its rim: the smallest thing that is a cascade. The spring sits
    /// on the upper pool's floor, where package T1 will put it.
    fn two_bowls(reentry: f64) -> World {
        let c = Config {
            width: 16,
            height: 14,
            depth: 2,
            voxel_m: 0.25,
            rain_m_per_s: 3.5e-5,
            evaporation_m_per_s: 0.0,
            closed_water_budget: true,
            initial_atmosphere_m3: 2.0,
            shower_trigger_fraction: 0.01,
            shower_volume_m3: 0.05,
            shower_interval_min_s: 6000.0,
            shower_interval_max_s: 6000.0,
            reentry_m3_per_s: reentry,
            ..Config::default()
        };
        let mut w = World::empty(c.clone());
        for z in 0..c.depth {
            for x in 0..c.width as i64 {
                // The lower ground, to y = 3; the shelf behind it, to y = 7.
                let top = if x >= 8 { 7 } else { 3 };
                for y in 1..=top {
                    w.material[c.index(x, y, z)] = Material::Rock;
                }
            }
            // The upper pool: a rock bowl on the shelf, x 10..13, floor at y = 8, with a
            // notch in its front rim at x = 10 one voxel below the rest.
            for x in 10..=13i64 {
                w.material[c.index(x, 8, z)] = Material::Rock;
            }
            for x in 11..=13i64 {
                w.material[c.index(x, 9, z)] = Material::Rock;
                w.material[c.index(x, 10, z)] = Material::Rock;
            }
            for x in 11..=12i64 {
                for y in 9..=10 {
                    w.material[c.index(x, y, z)] = Material::Air;
                }
            }
            // The notch: x = 10 tops out at y = 9, a voxel under the rim at 10.
            w.material[c.index(10, 9, z)] = Material::Rock;
            // The lower basin: a real bowl, x 2..5, floor at y = 1, rimmed at y = 3 by the
            // ground either side of it, so the lake has somewhere to be and a top to it.
            for x in 2..=5i64 {
                for y in 2..=3 {
                    w.material[c.index(x, y, z)] = Material::Air;
                }
            }
        }
        w.rebuild_active_sets();
        w.set_spring_cell(Some((12, 9, 0)));
        // The datum: the outlet sits in the lake's own surface row, as `LakeDatum` seats
        // it, so the lake holds its level and only the surplus leaves.
        w.set_outlet_cell(Some((2, 3, 0)));
        w
    }

    /// The stream runs: the upper pool stands at its notch, the fall column under it is
    /// wet on **every** tick, and the world's stored water stops changing even though the
    /// water in it never stops moving.
    #[test]
    fn a_stream_reaches_steady_state() {
        let mut w = two_bowls(0.0015);
        let c = w.config().clone();
        hydrate(
            &mut w,
            &Water {
                inventory_m: 0.04,
                atmosphere_fraction: 0.5,
                closed_cycle: true,
                rain_m_per_s: 3.5e-5,
                evaporation_m_per_s: 0.0,
                shower_trigger_fraction: 0.01,
                shower_volume_m3: 0.05,
                shower_interval_min_s: 6000.0,
                shower_interval_max_s: 6000.0,
                ..Water::DRY
            },
        );
        let settled = w.settle(600);
        assert!(
            !settled.dry_locked,
            "a running stream is not a drought: {settled:?}"
        );
        // The outlet is the stream's way home — the host opens it after seeding, and
        // without it the ring is a bath filling, not a river running.
        w.apply(crate::Command::SetOutlet { open: true });
        // Opening it is a transient of its own: the outlet's 0.05 m³/s is far more than
        // the stream's, so the lower pool draws down to the outlet's own lip before the
        // two rates can balance. Wait that out, then measure.
        for _ in 0..8000 {
            w.step();
        }

        // The fall: the void under the notch's outer lip, where the overflow drops.
        let fall = c.index(10, 10, 0);
        let mut wet = 0;
        let mut stored: Vec<f64> = Vec::new();
        for _ in 0..200 {
            w.step();
            if w.free[fall] > 0.0 {
                wet += 1;
            }
            stored.push(w.view().stored_m3());
        }
        assert_eq!(
            wet,
            200,
            "the fall column ran dry on {} of 200 ticks",
            200 - wet
        );

        let upper = pools(&w)
            .into_iter()
            .find(|p| p.floor_y >= 9)
            .expect("the upper pool holds water");
        assert!(
            upper.level_y >= 10,
            "the upper pool stands at its notch: {upper:?}"
        );

        // The brief's test: stored water within one per cent across the last hundred ticks.
        let last = &stored[stored.len() - 100..];
        let (a, b) = (last[0], last[last.len() - 1]);
        assert!(
            (b - a).abs() <= 0.01 * a.max(1e-9),
            "flow-through is a steady state: stored {a:.4} then {b:.4} over the last 100 ticks"
        );
        assert!(w.view().total_residual().abs() < 1e-9);
    }

    /// `pools` is the open water, tier by tier: the upper pool counts and the lake does
    /// not count as a tier above itself.
    #[test]
    fn pools_reports_only_open_water_above_the_lake() {
        // No stream: this is a question about reading water, not about moving it. And a
        // thin inventory, so the lake stays in its own bowl instead of drowning the shelf
        // the upper pool sits on.
        let mut w = two_bowls(0.0);
        hydrate(
            &mut w,
            &Water {
                inventory_m: 0.04,
                atmosphere_fraction: 0.2,
                closed_cycle: true,
                shower_interval_min_s: 6000.0,
                shower_interval_max_s: 6000.0,
                ..Water::DRY
            },
        );
        w.settle(600);
        // Fill the upper bowl by hand. A thin inventory puts its water in the lake, which
        // is right — this test is about *reading* pools, not about who gets filled, and a
        // hand-poured pool is the same geometry a tier's own spring will make.
        let c = w.config().clone();
        for x in 11..=12i64 {
            for z in 0..c.depth {
                w.apply(crate::Command::AddWater {
                    x,
                    y: 9,
                    z,
                    volume_m3: c.voxel_volume(),
                });
            }
        }
        for _ in 0..40 {
            w.step();
        }
        let list = pools(&w);
        let lake = lake(&w);
        assert!(
            list.iter().any(|p| p.floor_y == lake.floor_y),
            "the lake is one of the pools: {lake:?} in {list:?}"
        );
        assert_eq!(
            tier_pools(&list, lake.level_y),
            1,
            "one pool stands above the lake: {list:?}"
        );
        for p in &list {
            for &i in &p.surface_cells {
                let plane = w.config().width as usize * w.config().depth as usize;
                assert!(
                    i + plane >= w.config().cells() || !w.material[i + plane].is_solid(),
                    "every surface cell is open water"
                );
            }
        }
    }

    /// **Study, not a test.** The smallest stream that reads: sweep `reentry_m3_per_s` on
    /// the two-bowl fixture and report, per rate, how many of 200 ticks the fall column
    /// holds water and what the sky store did. The rate that keeps it wet on every tick is
    /// the one the presets want, scaled by voxel volume.
    #[test]
    #[ignore = "study: run by name"]
    fn the_smallest_stream_that_reads() {
        let vol = 0.25f64.powi(3);
        for rate in [1e-5, 3e-5, 1e-4, 3e-4, 5e-4, 1e-3, 1.5e-3, 3e-3] {
            let mut w = two_bowls(rate);
            let c = w.config().clone();
            hydrate(
                &mut w,
                &Water {
                    inventory_m: 0.3,
                    atmosphere_fraction: 0.5,
                    closed_cycle: true,
                    shower_interval_min_s: 6000.0,
                    shower_interval_max_s: 6000.0,
                    ..Water::DRY
                },
            );
            w.settle(600);
            w.apply(crate::Command::SetOutlet { open: true });
            let fall = c.index(10, 10, 0);
            let sky0 = w.atmosphere_m3();
            let (mut wet, mut min_fill) = (0, f64::INFINITY);
            for _ in 0..200 {
                w.step();
                if w.free[fall] > 0.0 {
                    wet += 1;
                    min_fill = min_fill.min(w.free[fall]);
                }
            }
            println!(
                "rate {rate:8.1e} m³/s ({:6.4} of a voxel per tick): fall wet {wet}/200, \
                 thinnest fill {:.2e}, sky {sky0:.3} -> {:.3} ({:+.1} %)",
                rate * crate::DT / vol,
                if min_fill.is_finite() { min_fill } else { 0.0 },
                w.atmosphere_m3(),
                100.0 * (w.atmosphere_m3() - sky0) / sky0.max(1e-9),
            );
        }
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
