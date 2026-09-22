//! Hollows: roofed void a body can stand inside. Derived geometry, not generation state.
//!
//! [`carve`] makes them; [`find`] reports them. Nothing is stored: a hollow is read back
//! out of the material array the way [`crate::generate::isolated_voids`] is, so the list
//! is still true after an edit, after the skyline pass has lowered a column, and later,
//! when the founders come looking for a floor under a roof.
//!
//! Three kinds, two mechanisms (`design/caves-and-hollows-plan-2026-09-21.md`).
//! **Undercuts and grottos** are notched out of the soft band under a hard cap on a
//! steep bank — no 3D noise, just the hardness layering the erosion exposed.
//! **Galleries** are carved from periodic 3D noise inside soft strata at depth and then
//! connected to the sky by a mouth or a skylight, or filled. **Shelves and terraces**
//! are what the first two leave behind where a notch meets a lower support; they have no
//! mechanism of their own.
//!
//! The camera is a 30° elevated orthographic section through `z = 0`, so a hollow nobody
//! can see is not kept. [`floor_is_visible`] is the same arithmetic as
//! [`crate::generate::visibility_pass`], read the other way round.

use crate::generate::{Heightfield, Volume, allowed_drop};
use crate::noise::{ring_cells, ring_noise, ring_noise_3d};
use crate::recipe::{Hollows, Landform, Recipe};
use crate::{Config, Material, World};

/// One roofed void.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hollow {
    /// Its void cells, in index order.
    pub cells: Vec<usize>,
    /// Solid cells inside it whose top face is exposed with the recipe's clearance of
    /// room above: where something can stand.
    pub floors: Vec<usize>,
    /// Its own cells that touch open sky — where the light and the runoff get in.
    pub mouths: Vec<usize>,
    /// Whether it reaches the front cut, where the camera sees it in section.
    pub meets_front: bool,
    /// Whether the camera can see any of its floor.
    pub visible: bool,
}

/// What one carve produced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Carved {
    /// Banks notched under their cap.
    pub undercuts: usize,
    /// Gallery bodies kept, having found a mouth or a skylight.
    pub galleries: usize,
    /// Gallery bodies filled again for want of either.
    pub dropped: usize,
}

/// Every hollow in the world, in index order.
///
/// A hollow is a connected run of **roofed** void — void with solid over it somewhere in
/// its own column — that has at least one floor a body could stand on: a solid cell with
/// the recipe's [`Hollows::clearance_m`] of void above it. Less room than that is
/// drainage, not habitat, and is not listed. `x` wraps, so a grotto across the seam is
/// one grotto.
pub fn find(world: &World) -> Vec<Hollow> {
    let clearance_m = section(world.config()).clearance_m;
    scan(world.config(), &world.material, clearance_m)
}

/// Whether the camera can see the floor at `(x, y, z)`.
///
/// The same arithmetic as [`crate::generate::visibility_pass`], read the other way
/// round: that pass lowers a nearer column until it stops hiding the skyline behind it,
/// and this asks whether any nearer column is still standing high enough to hide *this*
/// cell. A cell at the cut, `z = 0`, has nothing in front of it and is drawn in section.
pub fn floor_is_visible(world: &World, x: i64, y: u32, z: u32) -> bool {
    visible_in(world.config(), &world.material, x, y, z)
}

fn visible_in(c: &Config, material: &[Material], x: i64, y: u32, z: u32) -> bool {
    (0..z).all(|near| match skyline(c, material, x, near) {
        Some(top) => top as i32 <= y as i32 + allowed_drop(z - near),
        None => true,
    })
}

/// Carve the hollows a recipe asks for. Runs on the volume, after voxelisation and
/// before habitat preparation: undercuts follow the layering the erosion exposed, and
/// galleries follow the same soft strata.
pub fn carve(volume: &mut Volume, field: &Heightfield, r: &Recipe, seed: u64) -> Carved {
    let h = r.hollows;
    if !h.any() {
        return Carved::default();
    }
    let undercuts = notch_banks(volume, field, &h, seed);
    let candidates = gallery_candidates(volume, field, &h, seed);
    let bodies = components(&volume.config, &candidates);
    connect_or_fill(volume, &h);
    seal_unreadable_shafts(volume);
    let kept = bodies
        .iter()
        .filter(|body| body.iter().any(|&i| !volume.material[i].is_solid()))
        .count();
    Carved {
        undercuts,
        galleries: kept,
        dropped: bodies.len() - kept,
    }
}

/// Open every sealed void to the sky or fill it in: a mouth cut to the nearest open void
/// within [`Hollows::mouth_reach_m`], else a skylight where the ceiling is within
/// [`Hollows::skylight_m`] of the surface, else back to rock.
///
/// Returns how many sealed bodies were opened and how many were filled. A hollow nobody
/// and nothing can get into is not a hollow; it is a bubble.
pub fn connect_or_fill(volume: &mut Volume, h: &Hollows) -> (usize, usize) {
    let c = volume.config.clone();
    let vm = c.voxel_m;
    let reach_v = (h.mouth_reach_m / vm).round().max(0.0) as u32;
    let sky_v = (h.skylight_m / vm).round().max(0.0) as u32;
    let (mut kept, mut dropped) = (0, 0);
    let mut scratch = Scratch::new(c.cells());
    // Filling a body can seal a neighbour of it, so go round until nothing is left; in
    // practice that is two passes.
    for _ in 0..4 {
        let mut open = sky_reachable(&c, &volume.material);
        let sealed: Vec<usize> = (0..c.cells())
            .filter(|&i| !volume.material[i].is_solid() && !open[i])
            .collect();
        if sealed.is_empty() {
            break;
        }
        for body in components(&c, &sealed) {
            // An earlier opening in this same pass may already have let it out.
            if body.iter().any(|&i| open[i]) {
                continue;
            }
            let opened = cut_mouth(volume, &body, &open, reach_v, &mut scratch)
                .filter(|cut| skyline_survives(volume, cut))
                .or_else(|| cut_skylight(volume, &body, sky_v));
            match opened {
                Some(cut) => {
                    kept += 1;
                    let from: Vec<usize> = body
                        .iter()
                        .chain(cut.iter().map(|(i, _)| i))
                        .copied()
                        .collect();
                    spread_open(&c, &volume.material, &mut open, &from);
                }
                None => {
                    for &i in &body {
                        volume.material[i] = Material::Rock;
                    }
                    dropped += 1;
                }
            }
        }
    }
    (kept, dropped)
}

/// Fill in every shaft that broke the surface and left the ring unreadable.
///
/// [`skyline_survives`] checks a cut the moment it is made, and that is not enough on its
/// own: a shaft that looked safe while a neighbouring gallery was still open stops being
/// safe when that gallery is filled back in and its column's skyline goes back up. So
/// the landform rule is asked once more at the end, of every column a cut broke through,
/// and a shaft that now hides the terrain behind it is closed. Closing one raises a
/// skyline, which can condemn another, so this goes round until nothing changes.
fn seal_unreadable_shafts(volume: &mut Volume) -> usize {
    let c = volume.config.clone();
    let (w, d) = (c.width as usize, c.depth as usize);
    let mut sealed = 0;
    for _ in 0..4 {
        let mut closing = Vec::new();
        for z in 0..d {
            for x in 0..w {
                let terrain = volume.surface[z * w + x];
                let Some(top) = skyline(&c, &volume.material, x as i64, z as u32) else {
                    continue;
                };
                if (top as i32) >= terrain {
                    continue;
                }
                if !visible_in(&c, &volume.material, x as i64, top, z as u32) {
                    closing.push((x, z, top, terrain));
                }
            }
        }
        if closing.is_empty() {
            break;
        }
        for (x, z, top, terrain) in closing {
            for y in top + 1..=terrain.max(0) as u32 {
                volume.material[c.index(x as i64, y, z as u32)] = Material::Rock;
            }
            sealed += 1;
        }
    }
    sealed
}

/// Mark the void an opening just joined to the sky, so the bodies after it in the same
/// pass know they are out.
fn spread_open(c: &Config, material: &[Material], open: &mut [bool], from: &[usize]) {
    let mut stack: Vec<usize> = from
        .iter()
        .copied()
        .filter(|&i| !material[i].is_solid() && !open[i])
        .collect();
    for &i in &stack {
        open[i] = true;
    }
    while let Some(i) = stack.pop() {
        for nb in neighbours(c, i) {
            if !material[nb].is_solid() && !open[nb] {
                open[nb] = true;
                stack.push(nb);
            }
        }
    }
}

/// Reusable working room for the mouth search, so one pass over a ring's sealed bodies
/// does not allocate a world-sized array for each of them.
struct Scratch {
    from: Vec<usize>,
    depth: Vec<u32>,
    touched: Vec<usize>,
}

impl Scratch {
    fn new(cells: usize) -> Scratch {
        Scratch {
            from: vec![usize::MAX; cells],
            depth: vec![u32::MAX; cells],
            touched: Vec::new(),
        }
    }
    fn clear(&mut self) {
        for &i in &self.touched {
            self.from[i] = usize::MAX;
            self.depth[i] = u32::MAX;
        }
        self.touched.clear();
    }
}

/// Whether the landform rule still holds for every column an opening cut into.
///
/// A mouth or a skylight that breaks the surface lowers that column's skyline, and a
/// column the camera cannot see past is a column that hides the terrain behind it. This
/// is the same inequality [`crate::generate::visibility_pass`] enforces on the skyline,
/// asked of the new top; a lowered column can only stop hiding things farther back, so
/// nothing else has to be checked. An opening that fails it is put back and the body it
/// would have opened is filled: a skylight nobody can look down is not a skylight.
fn skyline_survives(volume: &mut Volume, cut: &[(usize, Material)]) -> bool {
    let c = volume.config.clone();
    let mut columns: Vec<(u32, u32)> = cut
        .iter()
        .map(|&(i, _)| {
            let (x, _, z) = c.coords(i);
            (x, z)
        })
        .collect();
    columns.sort_unstable();
    columns.dedup();
    let ok = columns.iter().all(|&(x, z)| {
        skyline(&c, &volume.material, x as i64, z)
            .is_none_or(|top| visible_in(&c, &volume.material, x as i64, top, z))
    });
    if !ok {
        for &(i, was) in cut {
            volume.material[i] = was;
        }
    }
    ok
}

/// Fill every hollow the camera cannot see. Returns how many.
pub fn fill_invisible(volume: &mut Volume, clearance_m: f64) -> usize {
    let doomed: Vec<Vec<usize>> = scan(&volume.config, &volume.material, clearance_m)
        .into_iter()
        .filter(|hollow| !hollow.visible)
        .map(|hollow| hollow.cells)
        .collect();
    for cells in &doomed {
        for &i in cells {
            volume.material[i] = Material::Rock;
        }
    }
    doomed.len()
}

/// The hollows section a config carves by, or none for the ridge generator.
pub fn section(config: &Config) -> Hollows {
    match &config.landform {
        Landform::Ridge => Hollows::NONE,
        Landform::Staged(r) => r.hollows,
    }
}

// --- the scan ------------------------------------------------------------------------

/// Highest solid voxel in a column.
fn skyline(c: &Config, material: &[Material], x: i64, z: u32) -> Option<u32> {
    (0..c.height)
        .rev()
        .find(|&y| material[c.index(x, y, z)].is_solid())
}

/// The six face neighbours, `x` wrapped, `y` and `z` clipped to the world.
fn neighbours(c: &Config, i: usize) -> Vec<usize> {
    let (x, y, z) = c.coords(i);
    let x = x as i64;
    let mut out = vec![c.index(x - 1, y, z), c.index(x + 1, y, z)];
    if y > 0 {
        out.push(c.index(x, y - 1, z));
    }
    if y + 1 < c.height {
        out.push(c.index(x, y + 1, z));
    }
    if z > 0 {
        out.push(c.index(x, y, z - 1));
    }
    if z + 1 < c.depth {
        out.push(c.index(x, y, z + 1));
    }
    out
}

/// Void cells the sky can reach, from the top row down.
fn sky_reachable(c: &Config, material: &[Material]) -> Vec<bool> {
    let mut seen = vec![false; c.cells()];
    let mut stack = Vec::new();
    for z in 0..c.depth {
        for x in 0..c.width as i64 {
            let i = c.index(x, c.height - 1, z);
            if !material[i].is_solid() && !seen[i] {
                seen[i] = true;
                stack.push(i);
            }
        }
    }
    while let Some(i) = stack.pop() {
        for nb in neighbours(c, i) {
            if !material[nb].is_solid() && !seen[nb] {
                seen[nb] = true;
                stack.push(nb);
            }
        }
    }
    seen
}

/// Connected runs of the given cells, each in index order.
fn components(c: &Config, cells: &[usize]) -> Vec<Vec<usize>> {
    let mut member = vec![false; c.cells()];
    for &i in cells {
        member[i] = true;
    }
    let mut seen = vec![false; c.cells()];
    let mut out = Vec::new();
    for &start in cells {
        if seen[start] {
            continue;
        }
        let mut body = Vec::new();
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            body.push(i);
            for nb in neighbours(c, i) {
                if member[nb] && !seen[nb] {
                    seen[nb] = true;
                    stack.push(nb);
                }
            }
        }
        body.sort_unstable();
        out.push(body);
    }
    out
}

fn scan(c: &Config, material: &[Material], clearance_m: f64) -> Vec<Hollow> {
    let clearance = (clearance_m / c.voxel_m).ceil().max(1.0) as u32;
    let mut tops = vec![0i64; c.width as usize * c.depth as usize];
    for z in 0..c.depth {
        for x in 0..c.width as i64 {
            tops[z as usize * c.width as usize + x as usize] =
                skyline(c, material, x, z).map_or(-1, |y| y as i64);
        }
    }
    let top_of = |x: u32, z: u32| tops[z as usize * c.width as usize + x as usize];
    let roofed: Vec<usize> = (0..c.cells())
        .filter(|&i| {
            let (x, y, z) = c.coords(i);
            !material[i].is_solid() && (y as i64) < top_of(x, z)
        })
        .collect();

    let mut out = Vec::new();
    for body in components(c, &roofed) {
        let mut mouths = Vec::new();
        let mut floors = Vec::new();
        let mut meets_front = false;
        for &i in &body {
            let (x, y, z) = c.coords(i);
            if z == 0 {
                meets_front = true;
            }
            // A mouth: a face that opens onto the sky's own air.
            if neighbours(c, i).into_iter().any(|nb| {
                let (nx, ny, nz) = c.coords(nb);
                !material[nb].is_solid() && (ny as i64) >= top_of(nx, nz)
            }) {
                mouths.push(i);
            }
            // A floor: the solid under this cell, with room over it for a body.
            if y > 0 {
                let under = c.index(x as i64, y - 1, z);
                if material[under].is_solid()
                    && (0..clearance).all(|k| {
                        y + k < c.height && !material[c.index(x as i64, y + k, z)].is_solid()
                    })
                {
                    floors.push(under);
                }
            }
        }
        if floors.is_empty() {
            continue;
        }
        floors.sort_unstable();
        floors.dedup();
        let visible = meets_front
            || floors.iter().any(|&f| {
                let (x, y, z) = c.coords(f);
                visible_in(c, material, x as i64, y, z)
            });
        out.push(Hollow {
            cells: body,
            floors,
            mouths,
            meets_front,
            visible,
        });
    }
    out
}

// --- package i: undercuts and grottos ------------------------------------------------

/// Notch the soft band back under a hard cap on a steep bank.
///
/// The site test is the hardness field's own geometry, read off the voxels it produced:
/// a column whose ground falls toward a neighbour faster than [`Hollows::bank_slope`],
/// whose rock is capped by [`Hollows::cap_thickness_m`] of bedrock, and which has soft
/// rock under that cap thick enough for a body. [`Heightfield::hard_cap`] — hard rock
/// standing over a neighbour cut [`Hollows::cap_drop_m`] below — is the second source,
/// and admits a site the slope alone would not.
///
/// The notch runs *into* the hill, opposite the fall, so the cap is left as a roof and
/// the opening faces the low ground. Where it meets a lower support it leaves a shelf.
fn notch_banks(volume: &mut Volume, field: &Heightfield, h: &Hollows, seed: u64) -> usize {
    let c = volume.config.clone();
    let (w, d) = (c.width as usize, c.depth as usize);
    let vm = c.voxel_m;
    let clearance = (h.clearance_m / vm).ceil().max(1.0) as i32;
    let cap_v = (h.cap_thickness_m / vm).round().max(1.0) as i32;
    let depth_v = (h.undercut_depth_m / vm).round().max(1.0) as usize;
    let circumference_m = field.circumference_m;
    let patch_cells = ring_cells(circumference_m, h.grotto_wavelength_m);
    let mut notched = 0;

    for z in 0..d {
        for x in 0..w {
            let i = z * w + x;
            let top = volume.surface[i];
            // The steepest fall to a neighbour, and the way back into the hill.
            let mut fall: Option<(usize, i64, i64, f64)> = None;
            for (dx, dz) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                let nz = z as i64 + dz;
                if nz < 0 || nz >= d as i64 {
                    continue;
                }
                let nx = (x as i64 + dx).rem_euclid(w as i64) as usize;
                let nb = nz as usize * w + nx;
                let slope = (top - volume.surface[nb]) as f64;
                if slope > 0.0 && fall.is_none_or(|(_, _, _, best)| slope > best) {
                    fall = Some((nb, dx, dz, slope));
                }
            }
            let Some((nb, dx, dz, slope)) = fall else {
                continue;
            };
            if slope < h.bank_slope && !field.hard_cap[i] {
                continue;
            }

            // The cap, under whatever soil is lying on it, and the soft band below it.
            let mut cap_top = top;
            while cap_top > 0
                && volume.material[c.index(x as i64, cap_top as u32, z as u32)] == Material::Soil
            {
                cap_top -= 1;
            }
            let hard = |y: i32| {
                y > 0 && volume.material[c.index(x as i64, y as u32, z as u32)] == Material::Bedrock
            };
            let soft = |y: i32| {
                y > 0 && volume.material[c.index(x as i64, y as u32, z as u32)] == Material::Rock
            };
            if !(0..cap_v).all(|k| hard(cap_top - k)) {
                continue;
            }
            let band_top = cap_top - cap_v;
            if !soft(band_top) {
                continue;
            }
            let mut band_lo = band_top;
            while band_lo > 1 && soft(band_lo - 1) {
                band_lo -= 1;
            }
            // Two to four voxels of headroom, never the whole band: a grotto, not a mine.
            let void_cells = (band_top - band_lo).min(clearance + 1);
            if void_cells < clearance {
                continue;
            }
            let void_lo = band_top - void_cells + 1;
            // The mouth is a hole in rock, so what has to be out of the way is the
            // neighbour's *rock*, not the talus lying against it: a grotto with a metre
            // of loose sediment across its threshold is a grotto with a sill, and the
            // sediment is the first thing the next shower moves.
            let (nx, nz) = ((x as i64 + dx).rem_euclid(w as i64), (z as i64 + dz) as u32);
            let mut outside = volume.surface[nb];
            while outside > 0 && volume.material[c.index(nx, outside as u32, nz)] == Material::Soil
            {
                outside -= 1;
            }
            if void_lo <= 1 || outside >= void_lo {
                continue;
            }

            // A few grottos, near the front, not a slot along every bank.
            let (x_m, z_m) = ((x as f64 + 0.5) * vm, (z as f64 + 0.5) * vm);
            let patch =
                0.5 + 0.5 * ring_noise(x_m, z_m, circumference_m, patch_cells, seed ^ h.stream);
            let front = 1.0 - h.front_bias * z as f64 / (d.max(2) - 1) as f64;
            if patch * front < 1.0 - h.undercut_density {
                continue;
            }

            for k in 0..depth_v {
                let k = k as i64;
                let cz = z as i64 - k * dz;
                if cz < 0 || cz >= d as i64 {
                    break;
                }
                let cx = x as i64 - k * dx;
                for y in void_lo..=band_top {
                    let cell = c.index(cx, y as u32, cz as u32);
                    if volume.material[cell] == Material::Rock {
                        volume.material[cell] = Material::Air;
                    }
                }
            }
            notched += 1;
        }
    }
    notched
}

// --- package ii: galleries and skylights ---------------------------------------------

/// Carve the cells a gallery might run through: periodic 3D noise over the threshold,
/// inside soft rock, far enough under the surface to be a passage and not a hole.
/// Returns the cells carved; whether any of it survives is
/// [`connect_or_fill`]'s business.
fn gallery_candidates(
    volume: &mut Volume,
    field: &Heightfield,
    h: &Hollows,
    seed: u64,
) -> Vec<usize> {
    if h.gallery_density <= 0.0 {
        return Vec::new();
    }
    let c = volume.config.clone();
    let (w, d) = (c.width as usize, c.depth as usize);
    let vm = c.voxel_m;
    let min_depth = (h.gallery_min_depth_m / vm).round().max(1.0) as i32;
    let cells = ring_cells(field.circumference_m, h.gallery_wavelength_m);
    // A monotone gate, not a measured quantile: more density, lower bar.
    let gate = 0.6 - 1.2 * h.gallery_density.clamp(0.0, 1.0);
    let mut carved = Vec::new();
    for z in 0..d {
        let z_m = (z as f64 + 0.5) * vm;
        let front = 1.0 - h.front_bias * z as f64 / (d.max(2) - 1) as f64;
        for x in 0..w {
            let x_m = (x as f64 + 0.5) * vm;
            let top = volume.surface[z * w + x];
            for y in 1..=(top - min_depth).max(0) {
                let i = c.index(x as i64, y as u32, z as u32);
                if volume.material[i] != Material::Rock {
                    continue;
                }
                let n = ring_noise_3d(
                    x_m,
                    y as f64 * vm * h.gallery_flatten,
                    z_m,
                    field.circumference_m,
                    cells,
                    seed ^ h.stream.rotate_left(29),
                );
                if n * front <= gate {
                    continue;
                }
                volume.material[i] = Material::Air;
                carved.push(i);
            }
        }
    }
    carved
}

/// Cut the shortest passage through rock from a sealed body to air the sky already
/// reaches, if one is within `reach` voxels.
fn cut_mouth(
    volume: &mut Volume,
    body: &[usize],
    open: &[bool],
    reach: u32,
    scratch: &mut Scratch,
) -> Option<Vec<(usize, Material)>> {
    if reach == 0 {
        return None;
    }
    let c = volume.config.clone();
    scratch.clear();
    let mut queue = std::collections::VecDeque::new();
    for &i in body {
        scratch.depth[i] = 0;
        scratch.touched.push(i);
        queue.push_back(i);
    }
    let mut found = None;
    while let Some(i) = queue.pop_front() {
        if scratch.depth[i] >= reach {
            continue;
        }
        for nb in neighbours(&c, i) {
            if scratch.depth[nb] != u32::MAX {
                continue;
            }
            scratch.depth[nb] = scratch.depth[i] + 1;
            scratch.from[nb] = i;
            scratch.touched.push(nb);
            if !volume.material[nb].is_solid() {
                if open[nb] {
                    found = Some(nb);
                    break;
                }
                continue;
            }
            queue.push_back(nb);
        }
        if found.is_some() {
            break;
        }
    }
    // Walk the path back, opening the rock it went through.
    let mut step = scratch.from[found?];
    let mut cut = Vec::new();
    while step != usize::MAX && scratch.depth[step] > 0 {
        cut.push((step, volume.material[step]));
        volume.material[step] = Material::Air;
        step = scratch.from[step];
    }
    Some(cut)
}

/// Cut a shaft straight up to the surface from the body's ceiling.
///
/// Every cell of the body whose ceiling is within `reach` is a candidate, shallowest
/// first, and the first shaft the landform rule accepts is the one taken: a body is not
/// given up on because its shallowest point happened to be the one the camera could not
/// look down.
fn cut_skylight(volume: &mut Volume, body: &[usize], reach: u32) -> Option<Vec<(usize, Material)>> {
    let c = volume.config.clone();
    let mut candidates: Vec<(u32, usize)> = body
        .iter()
        .filter_map(|&i| {
            let (x, y, z) = c.coords(i);
            let top = skyline(&c, &volume.material, x as i64, z)?;
            (top > y && top - y <= reach).then_some((top - y, i))
        })
        .collect();
    candidates.sort_unstable();
    for (_, i) in candidates {
        let (x, y, z) = c.coords(i);
        let Some(top) = skyline(&c, &volume.material, x as i64, z) else {
            continue;
        };
        let mut cut = Vec::new();
        for yy in y + 1..=top {
            let cell = c.index(x as i64, yy, z);
            cut.push((cell, volume.material[cell]));
            volume.material[cell] = Material::Air;
        }
        if skyline_survives(volume, &cut) {
            return Some(cut);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::{isolated_voids, repair_isolated};
    use crate::{Command, PRESETS};

    /// A solid block of rock `height - 4` deep under open sky, in a tiny world.
    fn block(width: u32, height: u32, depth: u32) -> World {
        let config = Config {
            width,
            height,
            depth,
            voxel_m: 0.25,
            ..Config::default()
        };
        let mut world = World::empty(config.clone());
        for z in 0..depth {
            for x in 0..width as i64 {
                for y in 1..height - 4 {
                    world.material[config.index(x, y, z)] = Material::Rock;
                }
            }
        }
        world
    }

    fn air(world: &mut World, x: i64, y: u32, z: u32) {
        let i = world.config().index(x, y, z);
        world.material[i] = Material::Air;
    }

    /// A slot `cells` tall at `(x, y0, z)`, and the columns beside it left solid.
    fn slot(world: &mut World, x: i64, y0: u32, z: u32, cells: u32, run: i64) {
        for k in 0..run {
            for y in y0..y0 + cells {
                air(world, x + k, y, z);
            }
        }
    }

    /// A `Volume` holding a world's materials, so the carve functions can work on it.
    fn volume_of(world: &World) -> Volume {
        let c = world.config();
        let (w, d) = (c.width as usize, c.depth as usize);
        let mut surface = vec![0i32; w * d];
        for z in 0..d {
            for x in 0..w {
                surface[z * w + x] = world.view().surface_y(x as i64, z as u32).unwrap_or(0) as i32;
            }
        }
        Volume {
            config: c.clone(),
            material: world.material.clone(),
            surface,
        }
    }

    fn install(world: &mut World, volume: &Volume) {
        world.material = volume.material.clone();
        world.rebuild_active_sets();
    }

    // ---- 1. reachability ----------------------------------------------------------

    /// A sealed gallery near a face is opened; one buried out of reach is filled; and
    /// after the pass nothing in the world is a void the sky cannot get to.
    #[test]
    fn a_sealed_gallery_is_opened_or_filled_and_none_is_left_sealed() {
        let mut world = block(16, 12, 4);
        // One cell under the rock's own surface, at the cut, where a shaft through to
        // it has nothing standing in front of it: within a mouth's reach of the sky.
        slot(&mut world, 2, 6, 0, 1, 3);
        // Five cells down, in the middle: out of reach of anything.
        slot(&mut world, 10, 2, 1, 1, 3);
        assert_eq!(isolated_voids(&world).len(), 6, "both start sealed");

        let mut volume = volume_of(&world);
        let h = Hollows {
            mouth_reach_m: 0.6,
            skylight_m: 0.6,
            ..Hollows::GROTTOS
        };
        let (kept, dropped) = connect_or_fill(&mut volume, &h);
        install(&mut world, &volume);

        assert_eq!((kept, dropped), (1, 1), "one opened, one filled");
        assert!(
            isolated_voids(&world).is_empty(),
            "a void the sky cannot reach survived"
        );
        assert_eq!(
            world.view().material_at(3, 6, 0),
            Material::Air,
            "the shallow gallery is still there"
        );
        assert!(
            world.view().material_at(11, 2, 1).is_solid(),
            "the buried one is filled"
        );
    }

    // ---- 2. clearance -------------------------------------------------------------

    /// A hollow is somewhere a body fits. Every floor a hollow lists has the recipe's
    /// clearance of void over it, and a slot one voxel short of it is not a hollow.
    #[test]
    fn a_slot_under_the_clearance_is_drainage_and_is_not_listed() {
        let mut world = block(16, 12, 4);
        // Clearance is 0.75 m at 0.25 m voxels: three cells. One slot has them, the
        // other is one short.
        // The rock tops out at y = 7, so a roofed slot sits at y = 3 and the shaft that
        // opens it goes up through y = 7.
        slot(&mut world, 2, 3, 1, 3, 2);
        for y in 3..8u32 {
            air(&mut world, 4, y, 1);
        }
        slot(&mut world, 9, 3, 1, 2, 2);
        for y in 3..8u32 {
            air(&mut world, 11, y, 1);
        }
        repair_isolated(&mut world);

        let hollows = find(&world);
        assert_eq!(
            hollows.len(),
            1,
            "only the slot a body fits under is a hollow"
        );
        let c = world.config();
        let clearance = 3u32;
        for h in &hollows {
            assert!(!h.floors.is_empty(), "a hollow has somewhere to stand");
            for &f in &h.floors {
                let (x, y, z) = c.coords(f);
                for k in 1..=clearance {
                    assert!(
                        !world.view().material_at(x as i64, y + k, z).is_solid(),
                        "the floor at ({x}, {y}, {z}) has {k} of solid over it"
                    );
                }
            }
        }
    }

    // ---- 3. camera ----------------------------------------------------------------

    /// The camera check is the skyline rule read the other way round: a floor under a
    /// lip on a front-facing bank is drawn, the same floor behind a taller nearer column
    /// is not, and anything at the cut is drawn by definition.
    #[test]
    fn the_camera_sees_a_floor_under_a_lip_and_not_one_behind_a_wall() {
        let mut world = block(16, 14, 4);
        // Cut the front two rows down to y = 4, so the bank faces the camera.
        for z in 0..2u32 {
            for x in 0..16i64 {
                for y in 5..10 {
                    air(&mut world, x, y, z);
                }
            }
        }
        // A floor at y = 7 in the third row, under the lip the block still has there.
        assert!(
            floor_is_visible(&world, 3, 7, 2),
            "the lip does not hide it"
        );
        // Put a wall back up in front of it and it is gone.
        for y in 5..10u32 {
            let i = world.config().index(3, y, 0);
            world.material[i] = Material::Rock;
        }
        assert!(
            !floor_is_visible(&world, 3, 7, 2),
            "a nearer column standing above it should hide it"
        );
        // The cut itself is always drawn.
        assert!(floor_is_visible(&world, 3, 4, 0));
    }

    /// A hollow at the cut is visible; one the camera cannot see is filled.
    #[test]
    fn a_hollow_nobody_can_see_is_filled() {
        let mut world = block(16, 12, 4);
        slot(&mut world, 2, 3, 0, 3, 2);
        for y in 3..8u32 {
            air(&mut world, 4, y, 0);
        }
        repair_isolated(&mut world);
        let front = find(&world);
        assert_eq!(front.len(), 1);
        assert!(front[0].meets_front && front[0].visible, "the cut shows it");

        // The same slot at the back, behind three rows standing well above it.
        let mut buried = block(16, 12, 4);
        slot(&mut buried, 2, 3, 3, 3, 3);
        for y in 3..8u32 {
            air(&mut buried, 4, y, 3);
        }
        repair_isolated(&mut buried);
        let listed = find(&buried);
        assert_eq!(listed.len(), 1, "it is a hollow");
        assert!(!listed[0].visible, "and the camera cannot see it");

        let mut volume = volume_of(&buried);
        assert_eq!(fill_invisible(&mut volume, 0.75), 1);
        install(&mut buried, &volume);
        assert!(find(&buried).is_empty(), "so it is filled");
    }

    // ---- 4. seam ------------------------------------------------------------------

    /// `x = 0` is not an edge: a hollow that straddles it is one hollow, and rotating
    /// the world rotates the list.
    #[test]
    fn a_hollow_across_the_seam_is_one_hollow_and_the_list_rotates_with_the_world() {
        let build = |shift: i64| {
            let mut world = block(16, 12, 4);
            for k in 0..3i64 {
                for y in 4..7u32 {
                    air(&mut world, 14 + k + shift, y, 1);
                }
            }
            for y in 4..8u32 {
                air(&mut world, 17 + shift, y, 1);
            }
            repair_isolated(&mut world);
            world
        };
        let world = build(0);
        let hollows = find(&world);
        assert_eq!(hollows.len(), 1, "the seam does not cut it in two");
        let c = world.config();
        let xs: Vec<u32> = hollows[0].cells.iter().map(|&i| c.coords(i).0).collect();
        assert!(
            xs.contains(&15) && xs.contains(&0),
            "it spans the seam: {xs:?}"
        );

        let shifted = find(&build(5));
        assert_eq!(shifted.len(), 1);
        let want: std::collections::BTreeSet<(u32, u32, u32)> = hollows[0]
            .cells
            .iter()
            .map(|&i| {
                let (x, y, z) = c.coords(i);
                ((x + 5) % c.width, y, z)
            })
            .collect();
        let got: std::collections::BTreeSet<(u32, u32, u32)> =
            shifted[0].cells.iter().map(|&i| c.coords(i)).collect();
        assert_eq!(got, want, "the rotated world has the rotated hollow");
    }

    // ---- 5. undercuts -------------------------------------------------------------

    /// A hard band over a soft one on a bank is notched out; the same bank in uniform
    /// rock is not.
    #[test]
    fn a_capped_bank_is_notched_and_a_plain_one_is_not() {
        let carved = |layered: bool| -> (Carved, World) {
            let (w, h, d) = (16u32, 16u32, 4u32);
            let config = Config {
                width: w,
                height: h,
                depth: d,
                voxel_m: 0.25,
                ..Config::default()
            };
            let mut world = World::empty(config.clone());
            // A step: the left half stands at y = 10, the right half at y = 4.
            for z in 0..d {
                for x in 0..w as i64 {
                    let top = if x < 8 { 10 } else { 4 };
                    for y in 1..=top {
                        // Hard cap from y = 9 up, soft rock under it.
                        let hard = layered && (9..=10).contains(&y);
                        world.material[config.index(x, y, z)] = if hard || y < 2 {
                            Material::Bedrock
                        } else {
                            Material::Rock
                        };
                    }
                }
            }
            let mut volume = volume_of(&world);
            let field = flat_field(&config);
            let recipe = notch_recipe();
            let report = carve(&mut volume, &field, &recipe, 1);
            install(&mut world, &volume);
            repair_isolated(&mut world);
            (report, world)
        };

        let (layered, world) = carved(true);
        assert!(layered.undercuts > 0, "the capped bank was not notched");
        let hollows = find(&world);
        assert!(!hollows.is_empty(), "the notch is a hollow");
        let c = world.config();
        for h in &hollows {
            assert!(!h.floors.is_empty(), "it has a floor");
            for &cell in &h.cells {
                let (x, y, z) = c.coords(cell);
                let top = world.view().surface_y(x as i64, z).expect("a roof");
                assert!(top > y, "({x}, {y}, {z}) has nothing over it");
            }
        }

        let (plain, _) = carved(false);
        assert_eq!(plain.undercuts, 0, "uniform rock has no cap to notch under");
    }

    /// A heightfield of the right shape for a hand-built world. The carve reads its
    /// ring size and its `hard_cap` flags, both of which are the recipe's business and
    /// not this fixture's; the layering it looks at is in the materials.
    fn flat_field(config: &Config) -> Heightfield {
        crate::generate::heightfield(config, &Recipe::DEFAULT)
    }

    fn notch_recipe() -> Recipe {
        Recipe {
            hollows: Hollows {
                undercut_density: 1.0,
                gallery_density: 0.0,
                front_bias: 0.0,
                clearance_m: 0.5,
                cap_thickness_m: 0.5,
                undercut_depth_m: 0.75,
                ..Hollows::GROTTOS
            },
            ..Recipe::DEFAULT
        }
    }

    // ---- 6. galleries -------------------------------------------------------------

    /// Soft rock at depth opens into galleries, and every gallery that is kept is
    /// connected to the sky.
    #[test]
    fn soft_rock_at_depth_opens_into_connected_galleries() {
        let (w, h, d) = (32u32, 20u32, 4u32);
        let config = Config {
            width: w,
            height: h,
            depth: d,
            voxel_m: 0.25,
            ..Config::default()
        };
        let mut world = World::empty(config.clone());
        for z in 0..d {
            for x in 0..w as i64 {
                for y in 1..=12 {
                    // A soft band from y = 5 to 9 inside hard rock.
                    world.material[config.index(x, y, z)] = if (5..=9).contains(&y) {
                        Material::Rock
                    } else {
                        Material::Bedrock
                    };
                }
            }
        }
        let mut volume = volume_of(&world);
        let field = flat_field(&config);
        let recipe = Recipe {
            hollows: Hollows {
                undercut_density: 0.0,
                gallery_density: 0.5,
                gallery_wavelength_m: 1.5,
                gallery_min_depth_m: 0.5,
                front_bias: 0.0,
                clearance_m: 0.5,
                mouth_reach_m: 1.0,
                skylight_m: 1.0,
                ..Hollows::GROTTOS
            },
            ..Recipe::DEFAULT
        };
        let report = carve(&mut volume, &field, &recipe, 3);
        install(&mut world, &volume);
        assert!(report.galleries > 0, "no gallery survived: {report:?}");
        assert!(
            isolated_voids(&world).is_empty(),
            "a kept gallery is connected to the sky"
        );
    }

    // ---- 7. presets ---------------------------------------------------------------

    /// Every preset carves a handful of grottos, all of them visible, none of them
    /// sealed, and the skyline still reads.
    #[test]
    fn every_preset_carves_visible_hollows_and_leaves_nothing_sealed() {
        for p in PRESETS {
            for seed in [1u64, 77] {
                let world = World::new(Config { seed, ..p.config() });
                let hollows = find(&world);
                assert!(
                    !hollows.is_empty(),
                    "{} seed {seed} carved no hollow at all",
                    p.name
                );
                assert!(
                    hollows.iter().all(|h| h.visible),
                    "{} seed {seed} kept a hollow the camera cannot see",
                    p.name
                );
                assert!(
                    isolated_voids(&world).is_empty(),
                    "{} seed {seed} left a sealed void",
                    p.name
                );
            }
        }
    }

    // ---- 8. water in a bowl --------------------------------------------------------

    /// A bowl-floored hollow holds the water poured into it, and the books balance.
    #[test]
    fn a_bowl_floored_hollow_holds_the_water_poured_into_it() {
        let config = Config {
            width: 12,
            height: 10,
            depth: 2,
            voxel_m: 0.25,
            water_substeps: 4,
            ..Config::default()
        };
        let mut world = World::empty(config.clone());
        for z in 0..config.depth {
            for x in 0..config.width as i64 {
                for y in 1..=5 {
                    world.material[config.index(x, y, z)] = Material::Rock;
                }
            }
        }
        // A bowl from x = 3 to 8, floor at y = 3, roofed at y = 7, open at x = 3.
        for z in 0..config.depth {
            for x in 4..=8i64 {
                for y in 4..=6 {
                    world.material[config.index(x, y, z)] = Material::Air;
                }
            }
            for y in 4..=6 {
                world.material[config.index(3, y, z)] = Material::Air;
            }
            for x in 4..=8i64 {
                world.material[config.index(x, 7, z)] = Material::Rock;
            }
        }
        world.rebuild_active_sets();
        assert!(
            crate::generate::isolated_voids(&world).is_empty(),
            "the bowl is open at its mouth"
        );

        let poured = 4.0 * config.voxel_volume();
        let taken = world.apply(Command::AddWater {
            x: 6,
            y: 5,
            z: 0,
            volume_m3: poured,
        });
        assert!(taken > 0.0);
        for _ in 0..300 {
            world.step();
        }
        let v = world.view();
        let held: f64 = (4..=8i64)
            .flat_map(|x| (0..config.depth).map(move |z| (x, z)))
            .map(|(x, z)| v.water_depth_m(x, 3, z))
            .sum();
        assert!(held > 0.0, "the bowl did not hold the water");
        let stored = v.stored_m3();
        let expected = world.ledger.expected_stored();
        assert!(
            (stored - expected).abs() <= 1e-9 * expected.max(1.0),
            "stored {stored:.6} against the ledger's {expected:.6}"
        );
    }
}
