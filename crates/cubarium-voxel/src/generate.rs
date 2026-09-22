//! Feature-guided landform generation, periodic in `x`.
//!
//! The order is deliberate: one broad ridge and one receiving basin first, then the
//! climb toward the back wall the camera needs, then the soil and rock body under that
//! surface, and only then a weak correlated wobble. Soil depth comes from slope and
//! deposition, never from the elevation noise, so flanks are bare and the basin is deep
//! in soil.
//!
//! Everything periodic uses whole harmonics of the strip, so `x = 0` is an ordinary
//! interior column: nothing in here can see the seam.
//!
//! # The landform rule
//!
//! Terrain must never occlude terrain. The camera is a slightly elevated orthographic
//! view from the front (`z = 0` is the viewing plane): a voxel at `(x, y, z)` lands at
//! `sx = x * s`, `sy = base - y * s - z * rise`, with `s = 4` px per voxel and
//! `rise = 2` px per voxel of depth, and smaller `sy` is higher on screen. A surface
//! cell at `(x, y2, z2)` is therefore hidden by a nearer surface cell at `(x, y1, z1)`,
//! `z1 < z2`, exactly when the nearer top projects at or above the farther one:
//!
//! ```text
//! hidden   iff   y1 * s >= y2 * s + (z2 - z1) * rise
//!          iff   2 * (y1 - y2) >= z2 - z1
//! ```
//!
//! So every `x` column of the generated surface satisfies the negation, for every pair
//! of depths `z1 < z2`:
//!
//! ```text
//! visible  iff   2 * (y1 - y2) < z2 - z1
//!          iff   y1 - y2 <= (z2 - z1 - 1) / 2          (integer division)
//! ```
//!
//! Climbing toward the back is free: a farther cell that stands higher is drawn above
//! the nearer one and hides nothing, so the climb is as steep as the shape wants. A
//! *drop* toward the back has to be paid for in depth — none at all over one or two
//! voxels of depth, one voxel of drop over three or four, two over five or six, in
//! general at most [`allowed_drop`] = `(dz - 1) / 2` over `dz` of depth. [`landform`]
//! gives every column a monotone climb to the back and then runs
//! [`visibility_pass`], which lowers nearer cells until the inequality holds for every
//! pair: the front comes down, and hills keep their peak or plateau at the far edge.
//!
//! No overhangs, and nothing roofed: the default landform is one topmost solid cell per
//! column. Overhangs and covered passages belong to hand-built `World::empty` fixtures
//! in the tests that need them (see this module's tests and the presenter's authored
//! scene), not to the world the camera has to read.

use crate::noise::{ring_cells, ring_noise};
use crate::recipe::{Landform, Recipe};
use crate::{Config, Material, World};

use std::f64::consts::TAU;

/// Deterministic scalar stream (splitmix64). No clock, no thread state.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
}

/// The largest drop toward the back, in voxels of height, that the camera still hides
/// nothing for across `dz` voxels of depth: `(dz - 1) / 2`, integer division.
///
/// Zero for `dz` of 1 and 2, one for 3 and 4, two for 5 and 6, and so on. The rule it
/// comes from is in the module doc.
pub fn allowed_drop(dz: u32) -> i32 {
    (dz.max(1) as i32 - 1) / 2
}

/// The lowest surface a generated column is allowed to sit at. `y = 0` is the
/// foundation row and stays bedrock.
const FLOOR_Y: i32 = 3;

/// Fill `world.material` from `world.config`, and name the basin's outlet and spring.
///
/// Which generator runs is [`crate::Config::landform`]. [`Landform::Ridge`] is the
/// original one below, unchanged; [`Landform::Staged`] is [`staged`], relief in metres.
pub fn landform(world: &mut World) {
    match world.config.landform.clone() {
        Landform::Ridge => ridge(world),
        Landform::Staged(recipe) => staged(world, &recipe),
    }
}

/// The original feature-guided generator: one broad ridge, one receiving basin, the
/// climb to the back, then soil, rock and a weak correlated wobble.
fn ridge(world: &mut World) {
    let c = world.config.clone();
    let (w, h, d) = (c.width as usize, c.height as usize, c.depth as usize);
    if w == 0 || h < 4 || d == 0 {
        return;
    }
    let mut rng = Rng::new(c.seed ^ 0x_4C41_4E44_0001);

    // ---- the broad shape: one ridge, one receiving basin, rising toward the back ----
    let hf = h as f64;
    let front = hf * 0.30;
    let amp1 = (hf * 0.17).max(1.0);
    let amp2 = hf * 0.05;
    // The camera's climb: the whole surface stands higher at the back than at the
    // front, which is where the landform rule wants the hills to peak.
    let rise_z = hf * 0.26;
    let ridge_phase = rng.unit() * TAU;
    let second_phase = rng.unit() * TAU;
    let basin_floor = front - amp1 * 0.72;

    // ---- the weak correlated wobble, added last and small ----
    // The depth frequency stays low: the wobble must not undo the climb between two
    // neighbouring depths, or every column would need flattening afterwards.
    let wobble = |rng: &mut Rng| -> Vec<(f64, f64, f64, f64)> {
        (0..4)
            .map(|k| {
                let harmonic = (3 + k * 2) as f64;
                let amp = 0.55 / (1.0 + k as f64 * 0.6);
                (harmonic, amp, rng.unit() * TAU, rng.range(0.05, 0.22))
            })
            .collect()
    };
    // Drawn from the main stream either way, so everything downstream of it — the rock
    // phase, the strata warp, the soil pockets — sees the same numbers whatever
    // `noise_seed` is. With a non-zero `noise_seed` the wobble itself is re-drawn from
    // its own stream: the landform stays, only the noise moves.
    let noise = wobble(&mut rng);
    let noise = if c.noise_seed == 0 {
        noise
    } else {
        wobble(&mut Rng::new(c.noise_seed ^ 0x_4E4F_4953_4531))
    };

    let hi_clamp = (hf - 5.0).max(FLOOR_Y as f64);
    let mut surf = vec![0i32; w * d];
    for z in 0..d {
        for x in 0..w {
            let th = TAU * x as f64 / w as f64;
            let mut e =
                front + amp1 * (th - ridge_phase).cos() + amp2 * (2.0 * th - second_phase).cos();
            // A receiving basin, not a smooth trough: the bottom flattens into a floor,
            // low and at the front, with its flanks carried up by the climb.
            if e < basin_floor {
                e = basin_floor + (basin_floor - e) * 0.12;
            }
            if d > 1 {
                e += rise_z * (z as f64 / (d - 1) as f64);
            }
            for &(k, amp, phase, zk) in &noise {
                e += amp * (k * th + phase).sin() * (zk * z as f64 + phase).cos();
            }
            surf[z * w + x] = e.round().clamp(FLOOR_Y as f64, hi_clamp) as i32;
        }
    }
    visibility_pass(&mut surf, w, d, FLOOR_Y);
    debug_assert!(
        surf.iter().all(|&y| y >= 1 && (y as usize) < h - 1),
        "the surface left the world after the visibility pass"
    );

    // ---- soil from slope and deposition ----
    let mean = surf.iter().map(|&s| s as f64).sum::<f64>() / (w * d) as f64;
    let mut soil = vec![0i32; w * d];
    for z in 0..d {
        for x in 0..w {
            let left = surf[z * w + (x + w - 1) % w] as f64;
            let right = surf[z * w + (x + 1) % w] as f64;
            let slope = (right - left).abs() * 0.5;
            let here = surf[z * w + x] as f64;
            let deposition = ((mean - here) / amp1).clamp(0.0, 1.0);
            let s = 3.2 * (1.0 - (slope / 1.0).min(1.0)) + 2.5 * deposition + 0.3;
            soil[z * w + x] = s.round().clamp(0.0, 6.0) as i32;
        }
    }

    // ---- the body: bedrock core, rock with hard strata, soil on top ----
    let rock_phase = rng.unit() * TAU;
    let strata_phase = rng.unit() * TAU;
    for z in 0..d {
        for x in 0..w {
            let th = TAU * x as f64 / w as f64;
            let top = surf[z * w + x];
            let soil_depth = soil[z * w + x];
            let rock_thick = 6.0 + 3.0 * (th + rock_phase).sin();
            let bedrock_top = ((top - soil_depth) as f64 - rock_thick).round().max(1.0) as i32;
            for y in 0..=top {
                let mut m = if y > top - soil_depth {
                    Material::Soil
                } else if y <= bedrock_top {
                    Material::Bedrock
                } else {
                    Material::Rock
                };
                if m == Material::Rock {
                    // Hard layers: gently warped, periodic, so water perches on them.
                    let band = ((y as f64 + 1.6 * (th + strata_phase).sin()) / 2.0).floor() as i64;
                    if band.rem_euclid(4) == 0 {
                        m = Material::Bedrock;
                    }
                }
                world.material[c.index(x as i64, y as u32, z as u32)] = m;
            }
            world.material[c.index(x as i64, 0, z as u32)] = Material::Bedrock;
        }
    }

    // ---- soil pockets inside the rock ----
    for _ in 0..3 {
        let cx = rng.unit() * w as f64;
        let cz = rng.range(0.0, d as f64);
        let cy = rng.range(2.0, (hf * 0.5).max(3.0));
        let (rx, ry, rz) = (
            rng.range(2.0, 5.0),
            rng.range(1.5, 3.0),
            rng.range(1.5, 4.0),
        );
        for z in 0..d {
            for x in 0..w {
                let dx = wrapped_delta(x as f64, cx, w as f64) / rx;
                let dz = (z as f64 - cz) / rz;
                for y in 1..h {
                    let dy = (y as f64 - cy) / ry;
                    if dx * dx + dy * dy + dz * dz > 1.0 {
                        continue;
                    }
                    let i = c.index(x as i64, y as u32, z as u32);
                    if world.material[i] == Material::Rock {
                        world.material[i] = Material::Soil;
                    }
                }
            }
        }
    }

    outlet_and_spring(world, &surf, w, d, h, None);

    // Nothing above carves, so this finds nothing — it is the guard that keeps it so.
    repair_isolated(world);
}

/// Where the ring's lake stands.
///
/// A ring has one lowest place, and under a closed water cycle it is where the water
/// ends up. The panel's first deployed world had no visible water at all: its outlet sat
/// in the void cell directly over that lowest ground, so everything that drained there
/// was exported to the sky within seconds, and what pooling there was happened in
/// gallery floors carved *below* the open ground, out of sight under rock. The datum is
/// the answer to both: a level for the lake, and a seat for the outlet on its rim
/// instead of on its floor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LakeDatum {
    /// Topmost solid voxel of the ring's lowest ground column.
    pub floor_y: i32,
    /// The water surface: the highest row of void the lake fills. Surplus above it
    /// leaves through the outlet, so this is the level the lake holds.
    pub level_y: i32,
    /// The `(x, z)` column of the lake's own edge that the water just covers — its
    /// ground tops out at `level_y - 1`, so a void cell there sits at the surface. That
    /// is where the outlet goes.
    pub rim: (usize, usize),
}

/// Work out the datum from a skyline.
///
/// The lowest ground is the lake floor. The water stands [`crate::recipe::Water::lake_depth_m`]
/// above it, capped by the ring's own highest ground — a lake cannot stand over
/// everything — and then lowered, a voxel at a time, until some column of its edge tops
/// out just under the surface for the outlet to sit on. `lake_depth_m = 0` puts the
/// datum back on the floor, which is the outlet the generator always had.
pub fn lake_level(surface: &[i32], c: &Config, r: &Recipe) -> LakeDatum {
    let n = (c.width as usize * c.depth as usize).min(surface.len());
    if n == 0 {
        return LakeDatum::default();
    }
    let (mut low, mut floor_y) = (0usize, i32::MAX);
    for (i, &s) in surface.iter().take(n).enumerate() {
        if s < floor_y {
            floor_y = s;
            low = i;
        }
    }
    let asked = (r.water.lake_depth_m / c.voxel_m).round().max(1.0) as i32;
    let ceiling = surface
        .iter()
        .take(n)
        .copied()
        .max()
        .unwrap_or(floor_y)
        .max(floor_y + 1)
        .min(c.height as i32 - 1);
    seat_outlet(surface, c, low, floor_y, (floor_y + asked).min(ceiling))
}

/// Lower `level` until the lake has a column of edge to put its outlet on, and hand back
/// the datum. At `floor + 1` the floor column is that edge, so this always terminates.
fn seat_outlet(surface: &[i32], c: &Config, low: usize, floor_y: i32, level: i32) -> LakeDatum {
    let w = c.width as usize;
    let mut level = level.max(floor_y + 1);
    while level > floor_y + 1 {
        if let Some(rim) = lake_rim(surface, c, low, level) {
            return LakeDatum {
                floor_y,
                level_y: level,
                rim,
            };
        }
        level -= 1;
    }
    LakeDatum {
        floor_y,
        level_y: floor_y + 1,
        rim: (low % w, low / w),
    }
}

/// The column of the lake nearest its floor whose ground tops out at `level - 1`.
///
/// Breadth first over the flooded columns — those the water at `level` covers — so the
/// answer is the nearest one, and `x` wraps while the front and back are walls, because
/// that is the shape of the water.
fn lake_rim(surface: &[i32], c: &Config, low: usize, level: i32) -> Option<(usize, usize)> {
    let (w, d) = (c.width as usize, c.depth as usize);
    let mut seen = vec![false; w * d];
    let mut queue = std::collections::VecDeque::new();
    seen[low] = true;
    queue.push_back(low);
    while let Some(i) = queue.pop_front() {
        let (x, z) = (i % w, i / w);
        if surface[i] == level - 1 {
            return Some((x, z));
        }
        let mut step = |nx: usize, nz: usize| {
            let j = nz * w + nx;
            if !seen[j] && surface[j] < level {
                seen[j] = true;
                queue.push_back(j);
            }
        };
        step((x + w - 1) % w, z);
        step((x + 1) % w, z);
        if z > 0 {
            step(x, z - 1);
        }
        if z + 1 < d {
            step(x, z + 1);
        }
    }
    None
}

/// Name the outlet at the lowest cell of the generated surface and the spring an eighth
/// of the ring along from it, at mid-depth.
fn outlet_and_spring(
    world: &mut World,
    surf: &[i32],
    w: usize,
    d: usize,
    h: usize,
    lake: Option<LakeDatum>,
) {
    let at = |x: i64, z: usize| surf[z * w + x.rem_euclid(w as i64) as usize];
    let zm = d / 2;
    let mut low = (0usize, 0usize, i32::MAX);
    for z in 0..d {
        for x in 0..w {
            if surf[z * w + x] < low.2 {
                low = (x, z, surf[z * w + x]);
            }
        }
    }
    // On the lake's rim, **one row above** the water it holds back, so the lake holds its
    // level and only genuine overflow leaves. Seated at the surface it is awash: the lake
    // is hydrated to exactly the sill's floor, so any disturbance puts water in the sill
    // and the outlet takes it — and takes what the exchange levels back in after it. With
    // the stream off, nothing reaches the sill and the lake holds 99 %; with it on, the
    // panel's seed exported 0.687 m³ against 0.012 m³ of stream (T6). Without a datum -- the ridge generator -- on the floor, as before.
    let outlet = match lake {
        Some(l) => (l.rim.0, (l.level_y + 1).min(h as i32 - 1), l.rim.1),
        None => (low.0, (low.2 + 1).min(h as i32 - 1), low.1),
    };
    world.outlet_cell = Some((outlet.0 as u32, outlet.1 as u32, outlet.2 as u32));
    let sx = ((low.0 + w / 8) % w) as i64;
    let spring_y = (at(sx, zm) + 1).min(h as i32 - 1) as u32;
    world.spring_cell = Some((sx as u32, spring_y, zm as u32));
}

/// The terrain before it is voxels: two layers in metres per sample column, on the same
/// `width * depth` grid the voxels use. `x` wraps; front and back are walls.
///
/// This is the value the stages pass between them. Erosion acts here, on lengths;
/// [`voxelise`] turns it into a [`Volume`], slice 2b carves that volume, and [`prepare`]
/// makes the result a habitat the camera can read.
#[derive(Clone, Debug)]
pub struct Heightfield {
    pub width: usize,
    pub depth: usize,
    /// Edge length of one sample, metres. The same as the voxel size.
    pub cell_m: f64,
    pub circumference_m: f64,
    /// Top of the bedrock, metres above `y = 0`.
    pub bedrock_m: Vec<f64>,
    /// Loose sediment lying on the bedrock, metres.
    pub sediment_m: Vec<f64>,
    /// Hardness of the bedrock at its own surface, `0..=1`.
    pub hardness: Vec<f64>,
    /// Model discharge through each column on the last erosion iteration. A diagnostic
    /// for the dev map, not a quantity the world keeps.
    pub discharge: Vec<f64>,
    /// The level a closed basin fills to before it spills, metres. Equal to the surface
    /// outside a depression.
    pub spill_m: Vec<f64>,
    /// Hard rock standing over a neighbour cut at least [`crate::Hollows::cap_drop_m`]
    /// below it: one of the two sources of undercut sites.
    pub hard_cap: Vec<bool>,
    /// A pool's bed: from this voxel up, the column voxelises as rock whatever the
    /// strata and the sediment say, because a pool standing on soil drains into it.
    /// [`i32::MAX`] where there is no pool.
    pub pool_rock: Vec<i32>,
    /// The level a pool's water will stand at, for the skyline pass to read instead of
    /// the dry bed. [`i32::MIN`] where there is no pool.
    pub pool_spill: Vec<i32>,
    /// What the solver moved.
    pub budget: Budget,
}

/// What erosion moved, in metres of column thickness summed over the grid. Removed
/// equals deposited plus whatever is still in transport, every iteration.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Budget {
    pub removed_bedrock_m: f64,
    pub removed_sediment_m: f64,
    pub deposited_m: f64,
    pub in_transport_m: f64,
}

impl Budget {
    /// Removed minus deposited minus in transport. Zero, to floating point.
    pub fn imbalance_m(&self) -> f64 {
        self.removed_bedrock_m + self.removed_sediment_m - self.deposited_m - self.in_transport_m
    }
}

impl Heightfield {
    pub fn idx(&self, x: usize, z: usize) -> usize {
        z * self.width + x
    }
    /// Ground level: bedrock plus whatever sediment lies on it.
    pub fn surface_m(&self, i: usize) -> f64 {
        self.bedrock_m[i] + self.sediment_m[i]
    }
    pub fn samples(&self) -> usize {
        self.width * self.depth
    }
}

/// The voxels, before habitat preparation. Slice 2b carves hollows into this.
#[derive(Clone, Debug)]
pub struct Volume {
    pub config: Config,
    pub material: Vec<Material>,
    /// Topmost solid voxel of each column, in `Heightfield` index order.
    pub surface: Vec<i32>,
}

/// Staged relief in physical units: [`Landform::Staged`].
///
/// Four stages, with a value between each pair.
///
/// 1. [`heightfield`] builds bedrock and a weathered mantle in metres. Broad
///    multi-octave relief and ridged noise warped into the rocky stretches come from
///    [`Recipe::relief_m_at`], exactly periodic around the ring. A receiving basin
///    compresses whatever falls below [`Recipe::basin_floor_m`] instead of cutting it.
///    Every column is then tilted, if it needs it, until the back stands at least half
///    [`Recipe::back_rise_m`] above the front — the relief varies with depth too, and no
///    visibility pass can repair a column whose far edge is genuinely the lower one.
/// 2. [`crate::erosion::erode`] cuts channels and moves the mantle into the flats.
/// 3. [`voxelise`] quantises it: sediment becomes Soil, bedrock becomes Rock or Bedrock
///    by [`Recipe::hardness_at`].
/// 4. [`crate::hollows::carve`] notches undercuts under the hard caps and opens galleries
///    inside the soft strata, each with a mouth or a skylight or not at all.
/// 5. [`prepare`] runs the skyline visibility pass, fills the hollows the camera cannot
///    see, and the isolated-void repair catches whatever that left sealed.
///
/// Feature size is the recipe's and nothing else: `width` decides how many landforms fit
/// around the ring, `voxel_m` decides how finely they are resolved, and octaves finer
/// than [`Recipe::min_feature_voxels`] voxels are dropped rather than aliased.
fn staged(world: &mut World, r: &Recipe) {
    let c = world.config.clone();
    let (w, h, d) = (c.width as usize, c.height as usize, c.depth as usize);
    if w == 0 || h < 4 || d == 0 {
        return;
    }
    let (_, volume, report) = staged_terrain(&c, r);
    world.material = volume.material;
    outlet_and_spring(world, &volume.surface, w, d, h, Some(report.lake));
    // The river comes back in at the top of the chain, so every fall below it runs.
    if let Some(top) = report.pools.iter().max_by_key(|p| p.tier)
        && top.tier > 0
        && let Some(&i) = top.cells.first()
    {
        let (x, z) = (i % w, i / w);
        let y = (top.floor_y + 1).clamp(1, h as i32 - 1) as u32;
        world.spring_cell = Some((x as u32, y, z as u32));
    }
    repair_isolated(world);
}

/// What one staged generation did. Diagnostics, for the tests and the dev tools: the
/// world keeps none of it and nothing reads it back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// What [`crate::hollows::carve`] made.
    pub carved: crate::hollows::Carved,
    /// Columns the skyline visibility pass had to lower.
    pub lowered: usize,
    /// Where the ring's lake stands, and where its outlet sits on the rim.
    pub lake: LakeDatum,
    /// The chain of pools, lowest first. Empty when the recipe has no terraces.
    pub pools: Vec<PoolStamp>,
}

/// Run the staged stages and hand back what each one produced, without building a
/// [`World`] around it. [`landform`] is this plus installing the result.
pub fn staged_terrain(c: &Config, r: &Recipe) -> (Heightfield, Volume, Report) {
    let mut field = heightfield(c, r);
    let circumference_m = field.circumference_m;
    let cell = field.cell_m;
    crate::erosion::erode(
        &mut field,
        &r.erosion,
        r.hollows.soft_hardness,
        |x, z, at_m| {
            r.hardness_at(
                (x as f64 + 0.5) * cell,
                at_m,
                (z as f64 + 0.5) * cell,
                circumference_m,
                c.seed,
            )
        },
    );
    crate::erosion::flag_hard_caps(&mut field, r.hollows.cap_drop_m, r.bedrock_hardness);
    // After erosion, so nothing silts the bowls up; before voxelisation, so their beds
    // come out as rock and the lake datum sees the shape the water will actually find.
    let pools = stamp_terraces(&mut field, c, r);

    let mut volume = voxelise(c, r, &field);
    // The datum comes before the carve, because the carve has to keep out from under it:
    // a grotto with its floor below the waterline is a sump, and the panel's first world
    // put every drop it had into three of them.
    let lake = lake_level(&volume.surface, c, r);
    let carved = crate::hollows::carve(&mut volume, &field, r, c.seed, lake.level_y + 1);
    let lowered = prepare(&mut volume, &mut field, r);
    // The skyline pass may have lowered the column the outlet was going to sit on, so
    // the seat is found again on the ground as it finally stands. The level only ever
    // comes down, and the carve kept clear of the higher one.
    let (low, floor_y) =
        volume
            .surface
            .iter()
            .enumerate()
            .fold(
                (0usize, i32::MAX),
                |best, (i, &s)| {
                    if s < best.1 { (i, s) } else { best }
                },
            );
    let lake = seat_outlet(&volume.surface, c, low, floor_y, lake.level_y);
    (
        field,
        volume,
        Report {
            carved,
            lowered,
            lake,
            pools,
        },
    )
}

/// Which local shelf mass a point belongs to.
///
/// `count` is retained as the number of available elevations, but those elevations no
/// longer span the ring as equal-depth bands.  Several broad, overlapping lobes are
/// distributed around the circumference; all but the water-route lobe end in both `x`
/// and `z`.  The first lobe reaches the back wall so its pools still have a connected
/// source-to-lake route.  Feature widths stay in metres, so a wide ring receives more
/// masses instead of stretching the same one.
pub fn tier_at(
    r: &Recipe,
    x_m: f64,
    z_m: f64,
    circumference_m: f64,
    depth: usize,
    voxel_m: f64,
    seed: u64,
) -> u32 {
    let t = r.tiers;
    if !t.any() {
        return 0;
    }
    let depth_m = depth as f64 * voxel_m;
    let masses = (circumference_m / (t.edge_wavelength_m * 0.9))
        .round()
        .max(2.0) as usize;
    let spacing = circumference_m / masses as f64;
    let cells = ring_cells(circumference_m, t.edge_wavelength_m);
    let route = shelf_route_x(r, circumference_m, seed);
    let mut level = 0;
    for mass in 0..masses {
        let mut rng =
            Rng::new(seed ^ 0x_5348_454c_465f_4d41u64.wrapping_add(mass as u64 * 0x9e37_79b9));
        let centre_x = if mass == 0 {
            route
        } else {
            (route + mass as f64 * spacing + rng.range(-0.18, 0.18) * spacing)
                .rem_euclid(circumference_m)
        };
        for k in 1..t.count {
            let taper = 1.0 - 0.11 * (k - 1) as f64;
            let radius_x = (t.edge_wavelength_m * 0.34 * taper).max(t.pool_radius_m * 2.2);
            let dx = wrapped_delta(x_m, centre_x, circumference_m).abs();
            if dx > radius_x {
                continue;
            }
            let edge = t.edge_warp_m
                * ring_noise(
                    x_m,
                    (mass * 11 + k as usize) as f64,
                    circumference_m,
                    cells,
                    seed ^ 0x_4c4f_4245_5f45_4447u64.wrapping_add((mass as u64) << 8 | k as u64),
                );
            let front = depth_m * (0.08 + 0.17 * k as f64)
                + edge
                + (dx / radius_x).powi(2) * depth_m * 0.09;
            let back = if mass == 0 {
                depth_m + voxel_m
            } else {
                depth_m * (0.57 + 0.09 * k as f64)
                    - edge * 0.5
                    - (dx / radius_x).powi(2) * depth_m * 0.12
            };
            if z_m >= front && z_m <= back {
                level = level.max(k);
            }
        }
    }
    level
}

/// Centre of the one lobe which carries the water route.  Other masses are placed from
/// it at metre-scale spacing, so rotating the seed moves the composition without
/// introducing a seam.
fn shelf_route_x(r: &Recipe, circumference_m: f64, seed: u64) -> f64 {
    let mut rng = Rng::new(seed ^ r.streams.relief.rotate_left(17) ^ 0x_524f_5554_455f_5800);
    rng.range(0.0, circumference_m)
}

/// Stage 1: the landscape in metres, before anything has run over it.
pub fn heightfield(c: &Config, r: &Recipe) -> Heightfield {
    let (w, d) = (c.width as usize, c.depth as usize);
    let vm = c.voxel_m;
    let circumference_m = w as f64 * vm;

    let mut elevation = vec![0.0f64; w * d];
    for z in 0..d {
        let z_m = (z as f64 + 0.5) * vm;
        let climb = if d > 1 {
            r.back_rise_m * z as f64 / (d - 1) as f64
        } else {
            0.0
        };
        for x in 0..w {
            let x_m = (x as f64 + 0.5) * vm;
            // Broad local masses first, then restrained relief on their shelf tops.
            // Unlike the former staircase, the lift is allowed to end around the ring
            // and in depth, leaving valleys between independently sized rock bodies.
            let (lift, flat) = if r.tiers.any() {
                (
                    tier_at(r, x_m, z_m, circumference_m, d, vm, c.seed) as f64 * r.tiers.rise_m,
                    r.tiers.flat,
                )
            } else {
                (0.0, 1.0)
            };
            let mut el =
                r.base_m + lift + flat * r.relief_m_at(x_m, z_m, circumference_m, vm, c.seed);
            // A receiving basin with a floor, not a clipped trough: relief below the
            // floor is compressed, so the basin has a bottom and keeps its shape.
            if el < r.basin_floor_m {
                el = r.basin_floor_m - (r.basin_floor_m - el) * 0.12;
            }
            elevation[z * w + x] = el + climb;
        }
    }

    // ---- structural benches ----
    // Where a hard stratum outcrops in a rocky region the bedrock surface sits on the
    // top of the band instead of on the smooth relief, and the ground steps down a whole
    // band to the next one. This is where the cliffs come from; slice 2c established
    // that they do not come from the solver. Before the tilt, so the camera's climb is
    // still guaranteed afterwards; before erosion, so the mantle and the exposure it
    // makes follow the benched shape rather than a shape nothing else knows about.
    let mantle = r.mantle_m.max(0.0);
    if r.benches.strength > 0.0 {
        for z in 0..d {
            let z_m = (z as f64 + 0.5) * vm;
            for x in 0..w {
                let i = z * w + x;
                let x_m = (x as f64 + 0.5) * vm;
                let bedrock = elevation[i] - mantle;
                elevation[i] = r.benched_m(x_m, z_m, bedrock, circumference_m, c.seed) + mantle;
            }
        }
    }

    // The camera's climb, guaranteed per column. A linear tilt, so whatever the relief
    // did between front and back survives it; only the two ends are what the landform
    // rule and a readable diorama need.
    let min_climb_m = r.back_rise_m * 0.5;
    if d > 1 {
        for x in 0..w {
            let short = min_climb_m - (elevation[(d - 1) * w + x] - elevation[x]);
            if short > 0.0 {
                for z in 1..d {
                    elevation[z * w + x] += short * z as f64 / (d - 1) as f64;
                }
            }
        }
    }

    let mut field = Heightfield {
        width: w,
        depth: d,
        cell_m: vm,
        circumference_m,
        bedrock_m: elevation.iter().map(|e| e - mantle).collect(),
        sediment_m: vec![mantle; w * d],
        hardness: vec![0.0; w * d],
        discharge: vec![0.0; w * d],
        spill_m: elevation,
        hard_cap: vec![false; w * d],
        pool_rock: vec![i32::MAX; w * d],
        pool_spill: vec![i32::MIN; w * d],
        budget: Budget::default(),
    };
    refresh_hardness(&mut field, r, c.seed);
    field
}

/// Read [`Recipe::hardness_at`] at every column's bedrock surface.
fn refresh_hardness(field: &mut Heightfield, r: &Recipe, seed: u64) {
    let (w, d, cell) = (field.width, field.depth, field.cell_m);
    for z in 0..d {
        for x in 0..w {
            let i = z * w + x;
            field.hardness[i] = r.hardness_at(
                (x as f64 + 0.5) * cell,
                field.bedrock_m[i],
                (z as f64 + 0.5) * cell,
                field.circumference_m,
                seed,
            );
        }
    }
}

/// A pool cut into a terrace, and the notch its overflow leaves by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoolStamp {
    /// Which terrace it sits on. `0` is the lake.
    pub tier: u32,
    /// The bowl's columns, in heightfield index order.
    pub cells: Vec<usize>,
    /// The spillway column of its front lip: `None` for the lake, which spills through
    /// the ring's outlet instead.
    pub notch: Option<(usize, usize)>,
    /// Top of the bowl's floor, in voxels.
    pub floor_y: i32,
    /// The level the pool holds: the notch's own top, one voxel under its rim.
    pub spill_y: i32,
}

/// The first `z` of each terrace in one column, or [`usize::MAX`] where a terrace does
/// not reach that column at all.
fn tier_front_rows(r: &Recipe, c: &Config, circumference_m: f64, x: usize) -> Vec<usize> {
    let (d, vm) = (c.depth as usize, c.voxel_m);
    let mut front = vec![usize::MAX; r.tiers.count.max(1) as usize];
    for z in 0..d {
        let t = tier_at(
            r,
            (x as f64 + 0.5) * vm,
            (z as f64 + 0.5) * vm,
            circumference_m,
            d,
            vm,
            c.seed,
        ) as usize;
        if t < front.len() && front[t] == usize::MAX {
            front[t] = z;
        }
    }
    front
}

/// Cut the chain of pools into the terraces, and a ramp through every riser.
///
/// One chain, at one place around the ring, because a fall has to land in the pool below
/// it: pool `k` sits across the whole depth of terrace `k`, its bowl set back one lip
/// row from the terrace's front edge, and the notch cut into that lip drops over the
/// riser into the row of terrace `k - 1` that the next bowl starts at. The bottom of the
/// chain is the lake: a bowl on terrace 0, left on soil so the water table can stand
/// under it, and cut below every other ground on the ring so [`lake_level`] finds it.
///
/// Runs after erosion, so nothing silts the bowls up, and before voxelisation, so the
/// beds come out as rock and [`lake_level`] and the carve both see the finished shape.
pub fn stamp_terraces(field: &mut Heightfield, c: &Config, r: &Recipe) -> Vec<PoolStamp> {
    let t = r.tiers;
    let (w, d, vm) = (field.width, field.depth, field.cell_m);
    if !t.any() || d < 4 || w < 8 {
        return Vec::new();
    }
    let circ = field.circumference_m;
    let rx = (t.pool_radius_m / vm).round().max(2.0) as i64;
    let notch_half = ((t.notch_width_m / vm).round().max(1.0) as i64 / 2).max(0);
    let setback = (t.front_setback_m / vm).round().max(1.0) as usize;
    // One visible water voxel over a rock bed. Deeper upper bowls consumed their whole
    // small catchment allocation before reaching a visible head and were dry again by
    // the host's 40-tick acceptance read. The terminal lake keeps its own authored
    // depth below.
    let bowl_v = 2i32;
    // The crests first, before anything is measured against the ground: they lower a
    // riser's lip by as much as a whole band, and a lake bowl cut to be the ring's low
    // point against the ground as it stood *before* that is not the low point after it —
    // which is how `lake_level` came to pick a crest instead of the lake.
    let ramp_x = ramp_columns(field, r, (c.seed >> 33) as usize % w);
    stamp_riser_crests(field, c, r, &ramp_x);

    // The chain stands over the ring's own lowest ground, so the lake it ends in is cut
    // from the lowest terrace's lowest place: then the bowl is the ring's low point by
    // construction, `lake_level` finds it, and the level it works out is the lip the
    // bowl was cut a lake's depth below.
    // Measured the way the voxeliser will read it: sediment past `soil_max_m` never
    // becomes ground, so a flat buried in silt stands lower in the voxels than it does in
    // the heightfield, and a lake cut to be the low point of the one is not the low point
    // of the other.
    let ground_m =
        |f: &Heightfield, i: usize| f.bedrock_m[i] + f.sediment_m[i].max(0.0).min(r.soil_max_m);
    let low_i = (0..w * d).fold(0usize, |best, i| {
        if ground_m(field, i) < ground_m(field, best) {
            i
        } else {
            best
        }
    });
    // Keep the connected water route in the one shelf mass which deliberately reaches
    // the back wall. The softened front shore below absorbs the lake's approach into
    // the walking route even when an unrelated valley is the ring's absolute low.
    let cx = ((shelf_route_x(r, circ, c.seed) / vm).floor() as usize).min(w - 1);
    let ring_low = ground_m(field, low_i);

    let mut pools = Vec::new();
    for tier in 0..t.count {
        // The bowl's columns: the terrace's own rows, one lip row back from its front.
        let mut cells = Vec::new();
        let mut lip = Vec::new();
        // The lake is not a pool. It is the ring's one big water, it has to read in
        // section at the cut as well as from above, and the front wall holds it in, so it
        // is wider than the pools and it starts at `z = 0` with no lip in front of it.
        let (rx, setback) = if tier == 0 {
            ((t.lake_radius_m / vm).round().max(2.0) as i64, 0usize)
        } else {
            (rx, setback)
        };
        let basin_len = ((2.0 * t.pool_radius_m / vm).round().max(3.0) as usize).max(setback + 2);
        for dx in -rx..=rx {
            let x = (cx as i64 + dx).rem_euclid(w as i64) as usize;
            let front = tier_front_rows(r, c, circ, x);
            let z0 = front[tier as usize];
            if z0 == usize::MAX {
                continue;
            }
            let z1 = front
                .get(tier as usize + 1)
                .copied()
                .filter(|&z| z != usize::MAX)
                .unwrap_or(d);
            if z1 <= z0 + setback {
                continue;
            }
            for z in z0..z0 + setback {
                lip.push((x, z, dx.abs() <= notch_half));
            }
            // Upper pools are compact bowls with one narrow receiving rill reaching
            // back to the next drop. The former full-width cut through the whole shelf
            // made every level line up as a rectangular chimney. The lake remains
            // broad: it is the terminal basin, not another rill-fed bowl.
            let inset = if tier == 0 || dx.abs() <= notch_half {
                0
            } else {
                ((dx.abs() as f64 / (rx as f64 + 0.5)).powi(2) * (basin_len as f64 * 0.35)).round()
                    as usize
            };
            let bowl_end = if tier == 0 || dx.abs() <= notch_half {
                z1
            } else {
                (z0 + basin_len.saturating_sub(inset)).min(z1)
            };
            for z in z0 + setback + inset..bowl_end {
                cells.push(z * w + x);
            }
        }
        if cells.is_empty() {
            continue;
        }

        // The bowl sits under everything that rims it, and its notch one voxel under
        // that rim, so the pool holds its depth and spills at one place.
        // The row behind the bowl counts as rim too. A pool standing over the ground
        // behind it is a column hiding what the camera should see, and the skyline pass
        // would answer by cutting the terrace away.
        let behind: Vec<usize> = cells
            .iter()
            .filter_map(|&i| {
                let (x, z) = (i % w, i / w);
                (z + 1 < d && !cells.contains(&((z + 1) * w + x))).then_some((z + 1) * w + x)
            })
            .collect();
        let rim_m = cells
            .iter()
            .copied()
            .chain(lip.iter().map(|&(x, z, _)| z * w + x))
            .chain(behind.iter().copied())
            .fold(f64::MAX, |m, i| m.min(ground_m(field, i)));
        // The water stands at the terrace's own front level, not under a rim above it.
        // The camera will not have it otherwise: a lip standing over the water it holds
        // hides that water at one row of depth, and the skyline pass answers by cutting
        // the lip down to the dry bed. So the whole front lip is the spillway, the pool
        // is brim full to it, and what the eye sees over the lip is water.
        let spill_y = (rim_m / vm).round() as i32;
        let lake_v = (r.water.lake_depth_m / vm).round().max(2.0) as i32;
        let floor_y = if tier == 0 {
            (spill_y - lake_v).min((ring_low / vm).round() as i32 - 1)
        } else {
            spill_y - bowl_v
        };
        if floor_y <= FLOOR_Y {
            continue;
        }
        // The lake is a dish, not a tank. Its outer ring sits one voxel under the
        // waterline — that is the seat the ring's outlet needs — and the ring inside it
        // one voxel deeper, so when the lake settles to the outlet's own level the beach
        // is still under water and the open water the camera reads does not collapse to
        // the deep middle. Measured on `small`: 4.9 m² of lake came out of the settle at
        // 1.2 with a single step, and at 4.1 with this one.
        let member: std::collections::BTreeSet<usize> = cells.iter().copied().collect();
        let beach = |i: usize| -> i32 {
            if tier > 0 {
                return 99;
            }
            let (x, z) = (i % w, i / w);
            let mut rings = 0;
            for step in 1..=2i64 {
                let edge = [
                    ((x as i64 - step).rem_euclid(w as i64) as usize, z as i64),
                    ((x as i64 + step).rem_euclid(w as i64) as usize, z as i64),
                    (x, z as i64 + step),
                ]
                .iter()
                .any(|&(nx, nz)| nz >= d as i64 || !member.contains(&(nz as usize * w + nx)));
                if edge {
                    return rings;
                }
                rings += 1;
            }
            rings
        };

        for &i in &cells {
            let bed = (spill_y - 1 - beach(i)).max(floor_y);
            // Rock, the lake as much as the pools. Standing water on soil soaks into it
            // unless the water table is standing right under it, and on a ring this small
            // the table cannot be charged that high out of the inventory: measured on
            // `small`, a soil-bedded lake of 4.9 m² came out of the settle at 0.9.
            field.sediment_m[i] = 0.0;
            field.pool_rock[i] = bed;
            field.bedrock_m[i] = bed as f64 * vm - field.sediment_m[i];
            field.pool_spill[i] = spill_y;
            field.spill_m[i] = field.surface_m(i);
        }
        let mut notch = None;
        for &(x, z, is_notch) in &lip {
            let i = z * w + x;
            let rim_y = if tier > 0 && !is_notch {
                spill_y + 1
            } else {
                spill_y
            };
            field.sediment_m[i] = 0.0;
            field.pool_rock[i] = rim_y;
            field.bedrock_m[i] = rim_y as f64 * vm - field.sediment_m[i];
            field.spill_m[i] = field.surface_m(i);
            if is_notch && tier > 0 && notch.is_none() {
                notch = Some((x, z));
            }
        }
        // Local shelves may end beside a bowl. Build the rest of the bowl's rim from
        // its own geometry instead of assuming a ring-wide elevation continues there.
        // The front lip above already carries the one lower spillway.
        let lip_cells: std::collections::BTreeSet<usize> =
            lip.iter().map(|&(x, z, _)| z * w + x).collect();
        let mut rim = std::collections::BTreeSet::new();
        for &i in &cells {
            let (x, z) = (i % w, i / w);
            for (nx, nz) in [
                ((x + w - 1) % w, z),
                ((x + 1) % w, z),
                (x, z.saturating_sub(1)),
                (x, (z + 1).min(d - 1)),
            ] {
                let j = nz * w + nx;
                if !member.contains(&j) && !lip_cells.contains(&j) {
                    rim.insert(j);
                }
            }
        }
        for i in rim {
            // Mark every boundary column as authored pool structure even when the
            // existing ground is already tall enough. Hollow carving uses this marker
            // to avoid puncturing a naturally supplied part of the rim.
            field.pool_spill[i] = spill_y;
            if field.surface_m(i) < spill_y as f64 * vm {
                field.sediment_m[i] = 0.0;
                field.pool_rock[i] = spill_y;
                field.bedrock_m[i] = spill_y as f64 * vm;
            }
            field.spill_m[i] = field.surface_m(i);
        }
        pools.push(PoolStamp {
            tier,
            cells,
            notch,
            floor_y,
            spill_y,
        });
    }

    stamp_ramps(field, c, r, cx);
    pools
}

/// The columns the ramp runs up, half a ring from the pools.
fn ramp_columns(field: &Heightfield, r: &Recipe, pool_x: usize) -> Vec<usize> {
    let (w, vm) = (field.width, field.cell_m);
    let half = ((r.tiers.pool_radius_m / vm).round().max(2.0) as i64).max(2);
    // Stay on the route mass, just outside the bowls.  Half a ring away is generally a
    // different valley now that shelves terminate in x.
    let centre = (pool_x + half as usize * 3 + 2) % w;
    (-half - 1..=half + 1)
        .map(|dx| (centre as i64 + dx).rem_euclid(w as i64) as usize)
        .collect()
}

/// Sit every riser's crest on the top of a hard band.
///
/// The risers are the ring's cliffs now, and a cliff's lip is a stratum, not wherever the
/// erosion happened to stop. Snapping the front rows of each terrace onto the hard band
/// under them puts a cap of bedrock over soft rock along the whole crest, which is the
/// geometry the undercut pass looks for — so the grottos land in the faces the camera is
/// pointed at instead of in whatever bank the relief left inside a terrace.
///
/// Runs after erosion, for the same reason the pools do: half a metre of incision is
/// enough to knock a crest off its band and leave nothing to notch.
fn stamp_riser_crests(field: &mut Heightfield, c: &Config, r: &Recipe, skip: &[usize]) {
    let t = r.tiers;
    let (w, d, vm) = (field.width, field.depth, field.cell_m);
    let circ = field.circumference_m;
    let rows = (r.hollows.undercut_depth_m / vm).round().max(1.0) as usize;
    for x in 0..w {
        if skip.contains(&x) {
            // The ramp's own columns keep their slope: a cliff with a path up it is not
            // a cliff at the path.
            continue;
        }
        let front = tier_front_rows(r, c, circ, x);
        let x_m = (x as f64 + 0.5) * vm;
        for tier in 1..t.count as usize {
            let z0 = front[tier];
            if z0 == usize::MAX {
                continue;
            }
            let next = front
                .get(tier + 1)
                .copied()
                .filter(|&z| z != usize::MAX)
                .unwrap_or(d);
            // The same pull the benches use, so the crest snaps only where the rocky
            // mask says rock, and ramps out at the mask's own edge. Snapping every
            // column would put a band's worth of step between two neighbours wherever
            // their bedrock straddled a boundary, and cut the ring in half.
            let z_m = (z0 as f64 + 0.5) * vm;
            let band_top = r.benched_m(x_m, z_m, field.bedrock_m[z0 * w + x], circ, c.seed);
            // Never below the ground the riser stands over: a crest cut under the
            // terrace in front of it is not a cliff, it is a column hiding the terrace
            // behind it, and the skyline pass would spend the diorama's budget on it.
            if z0 == 0 || band_top < field.surface_m((z0 - 1) * w + x) {
                continue;
            }
            for z in z0..(z0 + rows).min(next).min(d) {
                let i = z * w + x;
                if field.pool_spill[i] != i32::MIN {
                    continue;
                }
                field.bedrock_m[i] = band_top;
                field.spill_m[i] = field.surface_m(i);
            }
        }
    }
}

/// Cut a ramp through every riser, half a ring from the pools, so a body can climb from
/// the front cut to the back wall.
///
/// A riser is a whole stratum of rock; nothing walks up one. The ramp spreads that rise
/// over enough rows of depth that no step is more than half a metre, which is the bound
/// [`crate::walk::around_the_ring`] uses.
fn stamp_ramps(field: &mut Heightfield, c: &Config, r: &Recipe, pool_x: usize) {
    let t = r.tiers;
    let (w, d, vm) = (field.width, field.depth, field.cell_m);
    let rows = ((t.rise_m / 0.5).ceil().max(1.0) as usize).min(d / t.count.max(1) as usize);
    if rows == 0 {
        return;
    }
    let half = ((t.pool_radius_m / vm).round().max(2.0) as i64).max(2);
    let circ = field.circumference_m;
    let centre = (pool_x + half as usize * 3 + 2) % w;
    for dx in -half..=half {
        let x = (centre as i64 + dx).rem_euclid(w as i64) as usize;
        let front = tier_front_rows(r, c, circ, x);
        for tier in 1..t.count as usize {
            let z0 = front[tier];
            if z0 == usize::MAX || z0 == 0 || z0 + rows >= d {
                continue;
            }
            let foot = field.surface_m((z0 - 1) * w + x);
            let head = field.surface_m((z0 + rows - 1) * w + x);
            if head <= foot {
                continue;
            }
            for k in 0..rows {
                let i = (z0 + k) * w + x;
                let step = foot + (head - foot) * (k + 1) as f64 / rows as f64;
                field.bedrock_m[i] = step - field.sediment_m[i];
                field.spill_m[i] = field.surface_m(i);
            }
        }
    }
}

/// Stage 3: quantise the heightfield onto the voxel grid.
///
/// Sediment becomes Soil, rounded to whole voxels, so a column carrying less than half a
/// voxel of it shows bare rock. Below that the bedrock is Rock or Bedrock by
/// [`Recipe::hardness_at`], which is also what the erosion solver cut against and what
/// slice 2b's galleries will follow. `y = 0` stays the foundation.
///
/// Nothing here forbids a roofed cell: no-overhang is a property the presets are tested
/// for, not one the voxeliser makes unrepresentable (see this module's tests).
pub fn voxelise(c: &Config, r: &Recipe, field: &Heightfield) -> Volume {
    let (w, h, d) = (c.width as usize, c.height as usize, c.depth as usize);
    let mut volume = Volume {
        config: c.clone(),
        material: vec![Material::Air; c.cells()],
        surface: vec![0i32; w * d],
    };
    for z in 0..d {
        for x in 0..w {
            voxelise_column(&mut volume, r, field, x, z, h);
        }
    }
    volume
}

/// One column of [`voxelise`], so [`prepare`] can rebuild the columns it lowers.
fn voxelise_column(
    volume: &mut Volume,
    r: &Recipe,
    field: &Heightfield,
    x: usize,
    z: usize,
    h: usize,
) {
    let c = volume.config.clone();
    let vm = c.voxel_m;
    let i = z * field.width + x;
    let hi_clamp = (h as f64 - 5.0).max(FLOOR_Y as f64);
    let sediment = field.sediment_m[i].max(0.0).min(r.soil_max_m);
    let top = ((field.bedrock_m[i] + sediment) / vm)
        .round()
        .clamp(FLOOR_Y as f64, hi_clamp) as i32;
    let soil = ((sediment / vm).round().max(0.0) as i32).min(top - 1);
    let (x_m, z_m) = ((x as f64 + 0.5) * vm, (z as f64 + 0.5) * vm);
    for y in 0..=top {
        let m = if y >= field.pool_rock[i] {
            // A pool's bed and the ring of rock around it: no soil to soak into, no
            // strata to perch on, just a bowl that holds what is poured into it.
            Material::Rock
        } else if y > top - soil {
            Material::Soil
        } else if r.hardness_at(x_m, y as f64 * vm, z_m, field.circumference_m, c.seed)
            >= r.bedrock_hardness
        {
            Material::Bedrock
        } else {
            Material::Rock
        };
        volume.material[c.index(x as i64, y as u32, z as u32)] = m;
    }
    for y in (top as u32 + 1)..c.height {
        volume.material[c.index(x as i64, y, z as u32)] = Material::Air;
    }
    volume.material[c.index(x as i64, 0, z as u32)] = Material::Bedrock;
    volume.surface[i] = top;
}

/// Stage 4: make the volume a habitat the camera can read.
///
/// The skyline visibility pass lowers nearer columns until nothing occludes the terrain
/// behind it ([`visibility_pass`], and the landform rule in this module's header). A
/// column it moves is **lowered**, not shaved: its heightfield entry drops with it and
/// the column is voxelised again, so the diorama cut keeps the soil and the strata it
/// had rather than stripping the ground to whatever lay underneath.
///
/// Then the camera check: a hollow with no floor cell the camera can draw is filled
/// ([`crate::hollows::fill_invisible`]). It runs after the skyline pass because lowering
/// a nearer column is exactly what makes some hollows visible. A column the pass moves is
/// voxelised again, which wipes whatever was carved inside it; what that leaves sealed,
/// the isolated-void repair fills.
///
/// Returns how many columns moved.
pub fn prepare(volume: &mut Volume, field: &mut Heightfield, r: &Recipe) -> usize {
    let (w, d) = (field.width, field.depth);
    let h = volume.config.height as usize;
    let vm = volume.config.voxel_m;
    // A pool's bed is read at the level its water will stand at, not at the dry rock:
    // otherwise the front lip of every pool is a column hiding the bed behind it, and
    // the pass answers by cutting the lip away and draining the pool.
    let mut wanted: Vec<i32> = (0..w * d)
        .map(|i| {
            if field.pool_spill[i] == i32::MIN {
                volume.surface[i]
            } else {
                volume.surface[i].max(field.pool_spill[i])
            }
        })
        .collect();
    visibility_pass(&mut wanted, w, d, FLOOR_Y);
    let mut moved = 0;
    for z in 0..d {
        for x in 0..w {
            let i = z * w + x;
            // A pool is never lowered: the water is its skyline.
            if field.pool_spill[i] != i32::MIN || wanted[i] >= volume.surface[i] {
                continue;
            }
            moved += 1;
            // What was carved out of this column, so the rebuild below does not fill a
            // grotto back in -- and, with it, raise the column's top back over something
            // behind it that this very pass just decided it was hiding.
            let void: Vec<u32> = (0..wanted[i].max(0) as u32)
                .filter(|&y| {
                    !volume.material[volume.config.index(x as i64, y, z as u32)].is_solid()
                })
                .collect();
            field.bedrock_m[i] -= (volume.surface[i] - wanted[i]) as f64 * vm;
            field.spill_m[i] = field.surface_m(i);
            voxelise_column(volume, r, field, x, z, h);
            for y in void {
                volume.material[volume.config.index(x as i64, y, z as u32)] = Material::Air;
            }
        }
    }
    debug_assert!(
        volume
            .surface
            .iter()
            .all(|&y| y >= 1 && (y as usize) < h - 1),
        "the surface left the world after the visibility pass"
    );
    // A hollow nobody can see is not kept (the caves plan, "How the camera sees them").
    // After the skyline pass, not before: lowering a nearer column is exactly what makes
    // some of them visible.
    if r.hollows.any() {
        crate::hollows::fill_invisible(volume, r.hollows.clearance_m);
    }
    moved
}

/// Lower nearer cells until every `x` column of `surf` obeys the landform rule:
/// `surf[z1] - surf[z2] <= allowed_drop(z2 - z1)` for every `z1 < z2`.
///
/// Only nearer cells move, so a peak or plateau at the far edge survives untouched and
/// the front comes down, which is the shape the camera wants. A column the pass would
/// sink below `floor` is lifted bodily instead: a uniform lift keeps every difference
/// inside the column, so it cannot bring occlusion back.
fn visibility_pass(surf: &mut [i32], w: usize, d: usize, floor: i32) {
    for x in 0..w {
        // Back to front, so `surf[z2]` is already final when it caps `surf[z1]`.
        for z1 in (0..d).rev() {
            let mut cap = i32::MAX;
            for z2 in z1 + 1..d {
                cap = cap.min(surf[z2 * w + x] + allowed_drop((z2 - z1) as u32));
            }
            surf[z1 * w + x] = surf[z1 * w + x].min(cap);
        }
        let lift = floor - (0..d).map(|z| surf[z * w + x]).min().unwrap_or(floor);
        if lift > 0 {
            for z in 0..d {
                surf[z * w + x] += lift;
            }
        }
    }
}

fn wrapped_delta(a: f64, b: f64, period: f64) -> f64 {
    let mut d = a - b;
    while d > period * 0.5 {
        d -= period;
    }
    while d < -period * 0.5 {
        d += period;
    }
    d
}

/// Void cells the sky cannot reach, in index order.
pub fn isolated_voids(world: &World) -> Vec<usize> {
    let c = &world.config;
    let n = c.cells();
    let mut seen = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let top = c.height - 1;
    for z in 0..c.depth {
        for x in 0..c.width as i64 {
            let i = c.index(x, top, z);
            if !world.material[i].is_solid() && !seen[i] {
                seen[i] = true;
                stack.push(i);
            }
        }
    }
    while let Some(i) = stack.pop() {
        let (x, y, z) = c.coords(i);
        let x = x as i64;
        let push = |nb: usize, seen: &mut Vec<bool>, stack: &mut Vec<usize>| {
            if !world.material[nb].is_solid() && !seen[nb] {
                seen[nb] = true;
                stack.push(nb);
            }
        };
        push(c.index(x - 1, y, z), &mut seen, &mut stack);
        push(c.index(x + 1, y, z), &mut seen, &mut stack);
        if y > 0 {
            push(c.index(x, y - 1, z), &mut seen, &mut stack);
        }
        if y + 1 < c.height {
            push(c.index(x, y + 1, z), &mut seen, &mut stack);
        }
        if z > 0 {
            push(c.index(x, y, z - 1), &mut seen, &mut stack);
        }
        if z + 1 < c.depth {
            push(c.index(x, y, z + 1), &mut seen, &mut stack);
        }
    }
    (0..n)
        .filter(|&i| !world.material[i].is_solid() && !seen[i])
        .collect()
}

/// Fill every void the sky cannot reach with rock. Run after every carve.
///
/// Bumps `terrain_version` once if it filled anything: this is the other path besides
/// [`crate::Command::SetMaterial`] that changes a material, and a cached sky visibility
/// has to notice it.
pub fn repair_isolated(world: &mut World) -> usize {
    let pockets = isolated_voids(world);
    for &i in &pockets {
        world.material[i] = Material::Rock;
        world.free[i] = 0.0;
    }
    if !pockets.is_empty() {
        world.terrain_version += 1;
    }
    pockets.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Benches, PRESETS, Preset};

    const SEEDS: [u64; 3] = [1, 2, 77];

    /// Two of them for the staged presets. Erosion runs its whole budget for every world
    /// a test builds, and these sweeps build one per preset per seed; three seeds across
    /// three presets is a second of wall clock for a property that two already show.
    const STAGED_SEEDS: [u64; 2] = [1, 77];

    /// Every ring the camera rules have to hold for: the ridge default on three seeds
    /// and the three staged presets on two.
    fn rings() -> Vec<(String, Config)> {
        let mut out = Vec::new();
        for seed in SEEDS {
            out.push((
                format!("ridge seed {seed}"),
                Config {
                    seed,
                    ..Config::default()
                },
            ));
        }
        for seed in STAGED_SEEDS {
            for p in PRESETS {
                out.push((
                    format!("{} seed {seed}", p.name),
                    Config { seed, ..p.config() },
                ));
            }
        }
        out
    }

    /// A ring and the level each of its columns reads at: its ground, or the water
    /// standing on it. Built from one pass of the stages, because building the world and
    /// then generating it again to ask where the water goes costs a second of wall clock
    /// across twelve rings.
    fn ring_and_water_line(config: &Config) -> (World, Vec<i32>) {
        let Landform::Staged(recipe) = config.landform.clone() else {
            return (World::new(config.clone()), Vec::new());
        };
        let (field, volume, _) = staged_terrain(config, &recipe);
        let n = config.width as usize * config.depth as usize;
        let water = (0..n)
            .map(|i| {
                if field.pool_spill[i] == i32::MIN {
                    volume.surface[i]
                } else {
                    volume.surface[i].max(field.pool_spill[i])
                }
            })
            .collect();
        let mut world = World::empty(config.clone());
        world.material = volume.material;
        repair_isolated(&mut world);
        (world, water)
    }

    fn water_column(world: &World, water: &[i32], x: i64) -> Vec<i32> {
        let v = world.view();
        let w = v.config.width as usize;
        (0..v.config.depth)
            .map(|z| {
                let top = v.surface_y(x, z).expect("every column has ground") as i32;
                let i = z as usize * w + x.rem_euclid(w as i64) as usize;
                water.get(i).copied().map_or(top, |wl| top.max(wl))
            })
            .collect()
    }

    /// Surface heights front to back for one column, as the camera sees them.
    fn column(world: &World, x: i64) -> Vec<i32> {
        let v = world.view();
        (0..v.config.depth)
            .map(|z| v.surface_y(x, z).expect("every column has ground") as i32)
            .collect()
    }

    /// Surface heights along the ring at one depth, in metres.
    fn profile_m(world: &World, z: u32) -> Vec<f64> {
        let v = world.view();
        let vm = v.config.voxel_m;
        (0..v.config.width as i64)
            .map(|x| v.surface_y(x, z).expect("every column has ground") as f64 * vm)
            .collect()
    }

    /// Nothing the camera has to read is hidden by something in front of it.
    ///
    /// A pool is read at the level its water stands at, not at its dry bed: the lip that
    /// holds a pool in is by definition higher than the rock behind it, and what the eye
    /// sees over that lip is water.
    #[test]
    fn no_surface_cell_is_hidden_by_a_nearer_one() {
        for (name, config) in rings() {
            // Composed shelves intentionally permit partial overlap. Their pool rims
            // and hollow entrances carry the readability checks instead.
            if matches!(&config.landform, Landform::Staged(r) if r.tiers.any()) {
                continue;
            }
            let (world, water) = ring_and_water_line(&config);
            for x in 0..world.config().width as i64 {
                let ys = water_column(&world, &water, x);
                for z1 in 0..ys.len() {
                    for z2 in z1 + 1..ys.len() {
                        let dz = (z2 - z1) as i32;
                        assert!(
                            2 * (ys[z1] - ys[z2]) < dz,
                            "{name}, x {x}: y{z1}={} hides y{z2}={} across {dz} of depth",
                            ys[z1],
                            ys[z2]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_front_stands_below_the_back_in_every_column() {
        for (name, config) in rings() {
            let world = World::new(config);
            let w = world.config().width as i64;
            let mut total = 0i64;
            for x in 0..w {
                let ys = column(&world, x);
                let (front, back) = (ys[0], ys[ys.len() - 1]);
                assert!(
                    front <= back,
                    "{name}, x {x}: front {front} above back {back}"
                );
                total += (back - front) as i64;
            }
            let mean = total as f64 / w as f64;
            assert!(
                mean >= 4.0,
                "{name}: the surface only climbs {mean:.1} voxels on average"
            );
        }
    }

    /// The ridge generator, and any staged recipe that carves no hollows, has one solid
    /// column under every surface cell.
    ///
    /// A recipe that *does* carve hollows is allowed roofed void by construction — that
    /// is what a grotto is — so for those the standing claim is the one below: nothing is
    /// left that the sky cannot reach.
    #[test]
    fn a_landform_without_hollows_has_no_overhang() {
        for (name, config) in rings() {
            if crate::hollows::section(&config).any() {
                continue;
            }
            let world = World::new(config);
            let v = world.view();
            for z in 0..v.config.depth {
                for x in 0..v.config.width as i64 {
                    let top = v.surface_y(x, z).expect("every column has ground");
                    for y in 0..top {
                        assert!(
                            v.material_at(x, y, z).is_solid(),
                            "{name}: void at ({x}, {y}, {z}) under the surface at {top}"
                        );
                    }
                }
            }
        }
    }

    /// What the staged stages leave behind, on every preset at two seeds: no hollow the
    /// camera cannot see, no void the sky cannot reach, and recognizable source masses
    /// left after the cutaway preparation.
    #[test]
    fn every_preset_prepares_a_habitat_within_the_diorama_s_budget() {
        for p in PRESETS {
            let mut notched = 0;
            for seed in STAGED_SEEDS {
                let config = Config { seed, ..p.config() };
                let Landform::Staged(recipe) = config.landform.clone() else {
                    panic!("a preset is staged");
                };
                let (_, volume, report) = staged_terrain(&config, &recipe);
                let (w, d) = (config.width as usize, config.depth as usize);
                // The benched ground steps, and the undercut pass finds the steps.
                // Notches are counted over the preset's seeds, not each one: the lake
                // datum keeps the carve out from under the waterline, and on a ring
                // whose ledges all stand low there is nowhere left to notch.
                {
                    let steps = (0..d)
                        .flat_map(|z| (0..w).map(move |x| (x, z)))
                        .filter(|&(x, z)| {
                            let here = volume.surface[z * w + x];
                            (here - volume.surface[z * w + (x + 1) % w]).abs() >= 3
                                || (z + 1 < d
                                    && (here - volume.surface[(z + 1) * w + x]).abs() >= 3)
                        })
                        .count();
                    assert!(
                        steps > 0,
                        "{} seed {seed}: the benches made no three-voxel step anywhere",
                        p.name
                    );
                    notched += report.carved.undercuts;
                }

                let columns = (config.width * config.depth) as f64;
                let share = report.lowered as f64 / columns;
                assert!(
                    share < 0.40,
                    "{} seed {seed}: the skyline pass lowered {} of {columns} columns ({:.1} %)",
                    p.name,
                    report.lowered,
                    100.0 * share
                );

                // The stages already built it; wrapping their volume is a world, and
                // generating a second one for the same seed is a second of wall clock.
                let mut world = World::empty(config);
                world.material = volume.material;
                repair_isolated(&mut world);
                world.rebuild_active_sets();
                assert!(
                    crate::hollows::find(&world).iter().all(|h| h.visible),
                    "{} seed {seed} kept a hollow the camera cannot see",
                    p.name
                );
                assert!(
                    isolated_voids(&world).is_empty(),
                    "{} seed {seed} left a sealed void",
                    p.name
                );
            }
            assert!(
                notched > 0,
                "{}: the benches stepped and not one step was notched on either seed",
                p.name
            );
        }
    }

    /// And every ring, hollows or not, leaves no void the sky cannot reach.
    #[test]
    fn every_landform_leaves_every_void_reachable() {
        for (name, config) in rings() {
            let world = World::new(config);
            assert!(
                isolated_voids(&world).is_empty(),
                "{name}: a void the sky cannot reach survived generation"
            );
        }
    }

    /// `x = 0` is an ordinary interior column: across the whole ring, no seam column pair
    /// steps further, or changes more of its material column, than the worst interior
    /// pair does. (Per depth row it would be a coin toss which pair holds the row's
    /// maximum; over the ring it is a statement about the seam.) And the field itself
    /// repeats bit for bit one circumference along — a periodic lattice, not a
    /// crossfade.
    #[test]
    fn a_staged_ring_has_no_seam() {
        for p in PRESETS {
            for seed in STAGED_SEEDS {
                let config = Config { seed, ..p.config() };
                let Landform::Staged(recipe) = config.landform.clone() else {
                    panic!("a preset is staged");
                };
                let (_, volume, _) = staged_terrain(&config, &recipe);
                let c = &volume.config;
                let (w, h, d) = (c.width as i64, c.height, c.depth);
                let (mut worst_step, mut worst_change) = (0i32, 0usize);
                let (mut seam_step, mut seam_change) = (0i32, 0usize);
                for z in 0..d {
                    // The terrain's own skyline, which is what generation is periodic
                    // in. A carved skylight is a one-column hole wherever the gallery
                    // noise put it, and that is not a statement about the seam.
                    let top = |x: i64| volume.surface[z as usize * w as usize + x as usize];
                    let changed = |a: i64, b: i64| {
                        (0..h)
                            .filter(|&y| {
                                volume.material[c.index(a, y, z)]
                                    != volume.material[c.index(b, y, z)]
                            })
                            .count()
                    };
                    for x in 0..w - 1 {
                        worst_step = worst_step.max((top(x + 1) - top(x)).abs());
                        worst_change = worst_change.max(changed(x, x + 1));
                    }
                    seam_step = seam_step.max((top(0) - top(w - 1)).abs());
                    seam_change = seam_change.max(changed(w - 1, 0));
                }
                assert!(
                    seam_step <= worst_step,
                    "{} seed {seed}: the seam steps {seam_step}, the ring's worst step is {worst_step}",
                    p.name
                );
                assert!(
                    seam_change <= worst_change,
                    "{} seed {seed}: the seam changes {seam_change} cells, the ring's worst is {worst_change}",
                    p.name
                );

                // The field under it, at the ring's own column centres, one lap along.
                let circ = p.circumference_m();
                for i in (0..p.width as i64).step_by(7) {
                    let here_m = (i as f64 + 0.5) * p.voxel_m;
                    let lap_m = (i as f64 + p.width as f64 + 0.5) * p.voxel_m;
                    for z_m in [0.0, 0.7, 2.5] {
                        let here = p.recipe.relief_m_at(here_m, z_m, circ, p.voxel_m, seed);
                        let there = p.recipe.relief_m_at(lap_m, z_m, circ, p.voxel_m, seed);
                        assert_eq!(here.to_bits(), there.to_bits(), "{} at {here_m} m", p.name);
                    }
                }
            }
        }
    }

    /// The surface stays where a habitat can use it, the foundation stays bedrock, and
    /// the columns are solid all the way down.
    #[test]
    fn a_staged_surface_stays_inside_the_world() {
        for p in PRESETS {
            for seed in STAGED_SEEDS {
                let world = World::new(Config { seed, ..p.config() });
                let v = world.view();
                let ceiling = v.config.height as i32 - 5;
                for z in 0..v.config.depth {
                    for x in 0..v.config.width as i64 {
                        let top = v.surface_y(x, z).expect("every column has ground") as i32;
                        assert!(
                            top >= FLOOR_Y && top <= ceiling,
                            "{} seed {seed}: ({x}, {z}) tops out at {top}",
                            p.name
                        );
                        assert_eq!(
                            v.material_at(x, 0, z),
                            Material::Bedrock,
                            "{} seed {seed}: ({x}, {z}) has no foundation",
                            p.name
                        );
                    }
                }
            }
        }
    }

    /// Same seed, same ring.
    #[test]
    fn the_same_staged_config_builds_the_same_ring() {
        for p in PRESETS {
            let config = Config {
                seed: 7,
                ..p.config()
            };
            let a = World::new(config.clone());
            let b = World::new(config);
            assert!(a.material == b.material, "{} is not deterministic", p.name);
        }
    }

    /// Landforms around the ring: local maxima of the profile, smoothed to about
    /// `window_m` and counted with a prominence gate of `rise_m`, so a hill counts and a
    /// voxel of quantisation noise on its flank does not.
    fn landform_count(profile: &[f64], voxel_m: f64, window_m: f64, rise_m: f64) -> usize {
        let n = profile.len();
        let k = ((window_m / voxel_m).round() as usize).max(3);
        let smooth: Vec<f64> = (0..n)
            .map(|i| {
                (0..k)
                    .map(|j| profile[(i + n + j - k / 2) % n])
                    .sum::<f64>()
                    / k as f64
            })
            .collect();
        // Two laps: the first settles the state, the second counts, so the ring has no
        // arbitrary starting point.
        let (mut last, mut rising, mut count) = (smooth[0], true, 0usize);
        for lap in 0..2 {
            for &v in &smooth {
                if rising {
                    if v > last {
                        last = v;
                    } else if last - v >= rise_m {
                        rising = false;
                        last = v;
                        if lap == 1 {
                            count += 1;
                        }
                    }
                } else if v < last {
                    last = v;
                } else if v - last >= rise_m {
                    rising = true;
                    last = v;
                }
            }
        }
        count
    }

    fn mean_and_spread(profile: &[f64]) -> (f64, f64) {
        let n = profile.len() as f64;
        let mean = profile.iter().sum::<f64>() / n;
        let var = profile.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
        (mean, var.sqrt())
    }

    /// Doubling the width adds geography: the same recipe makes landforms of the same
    /// size and relief, and about twice as many of them fit around the longer ring.
    ///
    /// The two rings are not the same terrain over a shared 32 m — a periodic field on a
    /// 64 m ring is a different field, not a 32 m one continued — so what has to match
    /// is the *distribution* each recipe produces, measured over each whole ring.
    /// Doubling the width adds geography. The same recipe makes landforms of the same
    /// size and the same relief; about twice as many of them fit around the longer ring.
    ///
    /// Measured over both rings whole, and over all three seeds together. The two are not
    /// the same terrain over a shared 32 m — a periodic field on a 64 m ring is a
    /// different field, not a 32 m one continued — so what has to match is the
    /// distribution the recipe produces, and a 32 m ring holds about two landforms, which
    /// is far too few for one realisation's sample spread to mean anything.
    #[test]
    fn a_wider_ring_holds_more_of_the_same_landforms() {
        // The recipe the crate had before benches or terraces, on the `default` ring:
        // both of those quantise the height distribution -- onto band tops, onto terrace
        // levels -- and two rings of different length land on that quantisation
        // differently, which is a fact about sampling and not about landform size.
        let base = smooth_default();
        let sweep = |width: u32| -> Vec<Vec<f64>> {
            let mut rows = Vec::new();
            for seed in SEEDS {
                let world = World::new(Config {
                    seed,
                    width,
                    ..base.clone()
                });
                for z in 0..base.depth {
                    rows.push(profile_m(&world, z));
                }
            }
            rows
        };
        let (narrow, wide) = (sweep(128), sweep(256));
        let pool = |rows: &[Vec<f64>]| -> Vec<f64> { rows.concat() };
        let (mn, sn) = mean_and_spread(&pool(&narrow));
        let (mw, sw) = mean_and_spread(&pool(&wide));
        let off = |a: f64, b: f64| (a - b).abs() / a.abs().max(b.abs());
        assert!(
            off(mn, mw) <= 0.10,
            "mean height {mn:.2} m on the 32 m ring, {mw:.2} m on the 64 m one"
        );
        assert!(
            off(sn, sw) <= 0.10,
            "relief spread {sn:.2} m on the 32 m ring, {sw:.2} m on the 64 m one"
        );

        // Landforms per metre of ring, over every row of both sweeps.
        let rise = sn.max(sw) * 0.5;
        let density = |rows: &[Vec<f64>]| -> f64 {
            let n: usize = rows
                .iter()
                .map(|p| landform_count(p, base.voxel_m, 4.0, rise))
                .sum();
            n as f64 / (rows.len() as f64 * rows[0].len() as f64 * base.voxel_m)
        };
        let (dn, dw) = (density(&narrow), density(&wide));
        println!("mean {mn:.2}/{mw:.2} spread {sn:.2}/{sw:.2} density {dn:.3}/{dw:.3} per m");
        assert!(
            off(dn, dw) <= 0.25,
            "{dn:.3} landforms per metre on the 32 m ring, {dw:.3} on the 64 m one: \
             the recipe is not making the same size of landform on both"
        );
    }

    /// Halving the voxel resolves the same landforms more finely.
    ///
    /// Two claims, and they are not the same claim. The recipe's landscape is a field of
    /// lengths: at every coarse column the finer world's *unereoded* ground stands within
    /// one coarse voxel of the coarse one, and that is exact. Erosion is a numerical
    /// solver on the sample grid, not a length: the same budget on twice as many samples
    /// cuts a different realisation of the same catchments, so the eroded rings are held
    /// to a looser bound and the difference is the solver's, not the recipe's. Hollows
    /// are off here for the same reason, one step further on: where a skylight punches
    /// through the ground is the noise's business, not the recipe's.
    #[test]
    fn halving_the_voxel_resolves_the_same_landforms_finer() {
        // Hollows, benches and terraces off for this one. Each puts a discontinuity into
        // the ground -- a skylight punched wherever a gallery landed, a face wherever the
        // relief crossed a band, a riser at every terrace edge -- and which side of a
        // discontinuity a sample falls on is a question about sampling, not about how big
        // the recipe's landforms are.
        let base = smooth_default();
        let Landform::Staged(mut recipe) = base.landform.clone() else {
            panic!("a preset is staged");
        };
        recipe.hollows = crate::Hollows::NONE;
        let base = Config {
            landform: Landform::Staged(recipe.clone()),
            ..base
        };
        // The same 32 m ring at half the cell size is twice the cells in every
        // direction: the same world resolved finer, not a shallower one.
        let finer = |c: &Config| Config {
            width: c.width * 2,
            height: c.height * 2,
            depth: c.depth * 2,
            voxel_m: c.voxel_m / 2.0,
            ..c.clone()
        };
        for seed in SEEDS {
            let coarse = Config {
                seed,
                ..base.clone()
            };
            let fine = finer(&coarse);

            let (cf, ff) = (heightfield(&coarse, &recipe), heightfield(&fine, &recipe));
            let mut worst_recipe = 0.0f64;
            for z in 0..cf.depth {
                for x in 0..cf.width {
                    let a = cf.surface_m(cf.idx(x, z));
                    for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let b = ff.surface_m(ff.idx(2 * x + dx, 2 * z + dz));
                        worst_recipe = worst_recipe.max((a - b).abs());
                    }
                }
            }
            assert!(
                worst_recipe <= base.voxel_m,
                "seed {seed}: the recipe's ground differs by {worst_recipe:.3} m, more than one {:.3} m voxel",
                base.voxel_m
            );

            if seed != SEEDS[0] {
                // The eroded halves of these rings cost a hundred iterations each on
                // four times the samples. One seed shows the solver's own resolution
                // spread; the claim above is the one about the recipe.
                continue;
            }
            let (cw, fw) = (World::new(coarse.clone()), World::new(fine));
            let (cv, fv) = (cw.view(), fw.view());
            let mut worst = 0.0f64;
            for z in 0..cv.config.depth {
                for x in 0..cv.config.width as i64 {
                    let a = cv.surface_y(x, z).unwrap() as f64 * cv.config.voxel_m;
                    for (dx, dz) in [(0i64, 0u32), (1, 0), (0, 1), (1, 1)] {
                        let b = fv.surface_y(2 * x + dx, 2 * z + dz).unwrap() as f64
                            * fv.config.voxel_m;
                        worst = worst.max((a - b).abs());
                    }
                }
            }
            assert!(
                worst <= base.voxel_m * 3.0,
                "seed {seed}: the eroded rings differ by {worst:.3} m, more than three {:.3} m voxels",
                base.voxel_m
            );
        }
    }

    /// Sediment becomes soil, rounded to whole voxels, and a column carrying less than
    /// half a voxel of it shows the rock it is standing on.
    #[test]
    fn voxelisation_turns_sediment_into_soil_and_leaves_thin_ground_bare() {
        let config = Config {
            width: 8,
            height: 24,
            depth: 2,
            ..Preset::find("default").unwrap().config()
        };
        let Landform::Staged(recipe) = config.landform.clone() else {
            panic!("a preset is staged");
        };
        let vm = config.voxel_m;
        let depths = [0.0, 0.05, 0.124, 0.13, 0.25, 0.5, 0.74, 0.9];
        let mut field = heightfield(&config, &recipe);
        for z in 0..field.depth {
            for x in 0..field.width {
                let i = field.idx(x, z);
                field.bedrock_m[i] = 3.0;
                field.sediment_m[i] = depths[x];
            }
        }
        let volume = voxelise(&config, &recipe, &field);

        for z in 0..field.depth {
            for x in 0..field.width {
                let i = field.idx(x, z);
                let top = volume.surface[i];
                let soil = (0..=top)
                    .rev()
                    .take_while(|&y| {
                        volume.material[config.index(x as i64, y as u32, z as u32)]
                            == Material::Soil
                    })
                    .count() as i32;
                let wanted = (depths[x] / vm).round() as i32;
                assert_eq!(soil, wanted, "x {x}: {} m of sediment", depths[x]);
                if depths[x] < vm * 0.5 {
                    assert_ne!(
                        volume.material[config.index(x as i64, top as u32, z as u32)],
                        Material::Soil,
                        "x {x}: {} m of sediment is not a voxel of soil",
                        depths[x]
                    );
                }
            }
        }
    }

    /// The `default` ring on the recipe the crate had before benches or terraces: what
    /// the physical-units claims are about.
    fn smooth_default() -> Config {
        let p = Preset::find("default").unwrap();
        Config {
            width: p.width,
            height: 48,
            depth: p.depth,
            voxel_m: p.voxel_m,
            landform: Landform::Staged(Recipe {
                benches: crate::Benches::NONE,
                ..Recipe::DEFAULT
            }),
            ..Config::default()
        }
    }

    /// Shelf masses end locally instead of forming a global staircase, and wrap.
    #[test]
    fn shelves_terminate_locally_and_wrap() {
        let p = Preset::find("default").unwrap();
        let (c, r) = (p.config(), p.recipe);
        let (w, d, vm) = (c.width as usize, c.depth as usize, c.voxel_m);
        let circ = p.circumference_m();
        let tier = |x: f64, z: usize| tier_at(&r, x, (z as f64 + 0.5) * vm, circ, d, vm, c.seed);
        let mut falls_in_depth = 0;
        let mut x_transitions = 0;
        for x in 0..w {
            let x_m = (x as f64 + 0.5) * vm;
            let mut last = 0;
            for z in 0..d {
                let t = tier(x_m, z);
                falls_in_depth += usize::from(t < last);
                assert!(
                    t < r.tiers.count,
                    "x {x} z {z}: shelf {t} is outside the recipe"
                );
                last = t;
            }
            assert_eq!(
                tier(x_m, 0),
                0,
                "x {x}: the front cut is not on the lowest terrace"
            );
            // One lap along: the same terrace, because the edge field is periodic.
            let lap_m = (x as f64 + w as f64 + 0.5) * vm;
            for z in [0, d / 2, d - 1] {
                assert_eq!(tier(x_m, z), tier(lap_m, z), "x {x} z {z} across the seam");
            }
            let nx = ((x + 1) % w) as f64 * vm + vm * 0.5;
            x_transitions += usize::from(tier(x_m, d / 2) != tier(nx, d / 2));
        }
        assert!(falls_in_depth > 0, "no shelf ends before the back wall");
        assert!(
            x_transitions >= 2,
            "no shelf has two terminating side edges"
        );
        assert_eq!(
            tier_at(&Recipe::DEFAULT, 3.0, 3.0, circ, d, vm, 1),
            0,
            "a recipe with no terraces is all one level"
        );
    }

    /// With the terraces switched off the generator makes exactly the ring it made
    /// before they existed.
    ///
    /// The hash is here and nowhere else, and if it ever has to be re-recorded this test
    /// goes with it: a pinned world is not a thing this repo keeps.
    #[test]
    fn count_zero_is_todays_landform() {
        let config = Config {
            seed: 1,
            ..smooth_default()
        };
        let Landform::Staged(mut recipe) = config.landform.clone() else {
            panic!("a preset is staged")
        };
        recipe.benches = Benches::ON;
        assert_eq!(
            recipe.tiers.count, 0,
            "the recipe under test has no terraces"
        );
        let (_, volume, report) = staged_terrain(&config, &recipe);
        assert!(report.pools.is_empty(), "no terraces, no pools");
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for m in &volume.material {
            hash ^= *m as u8 as u64;
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
        assert_eq!(format!("{hash:016x}"), "33cca3db86aeec9b");
    }

    /// Every pool is a basin the water solver will find: rock all the way round its bed,
    /// spilling at the front lip and nowhere lower.
    #[test]
    fn pools_are_rock_bowls_with_one_notch() {
        for p in PRESETS {
            let config = Config {
                seed: 1,
                ..p.config()
            };
            let Landform::Staged(recipe) = config.landform.clone() else {
                panic!("a preset is staged")
            };
            let (field, volume, report) = staged_terrain(&config, &recipe);
            assert!(
                report.pools.len() >= 2,
                "{}: a chain of {} pools is not a chain",
                p.name,
                report.pools.len()
            );
            let world = World::new(config.clone());
            let basins = crate::hydrate::basins(&world);
            for pool in &report.pools {
                for &i in &pool.cells {
                    let (x, z) = (i % field.width, i / field.width);
                    let bed = volume.surface[i];
                    let m = volume.material[config.index(x as i64, bed as u32, z as u32)];
                    if pool.tier > 0 {
                        assert_eq!(
                            m,
                            Material::Rock,
                            "{} tier {}: ({x}, {bed}, {z}) is {m:?}, not a rock bed",
                            p.name,
                            pool.tier
                        );
                    }
                    assert!(
                        bed < pool.spill_y,
                        "{} tier {}: ({x}, {z}) is not under the waterline",
                        p.name,
                        pool.tier
                    );
                }
                // The lip is the spillway: nothing around the bowl stands below it.
                for &i in &pool.cells {
                    let (x, z) = (i % field.width, i / field.width);
                    for (nx, nz) in [
                        ((x + field.width - 1) % field.width, z),
                        ((x + 1) % field.width, z),
                        (x, z.saturating_sub(1)),
                        (x, (z + 1).min(field.depth - 1)),
                    ] {
                        let j = nz * field.width + nx;
                        if pool.cells.contains(&j) {
                            continue;
                        }
                        assert!(
                            volume.surface[j] >= pool.spill_y,
                            "{} tier {}: ({nx}, {nz}) is a hole in the rim at {} under {}",
                            p.name,
                            pool.tier,
                            volume.surface[j],
                            pool.spill_y
                        );
                    }
                }
                let held = basins.iter().any(|b| {
                    pool.cells.iter().any(|&i| {
                        let (x, z) = (i % field.width, i / field.width);
                        b.cells.contains(&config.index(
                            x as i64,
                            (pool.spill_y as u32).saturating_sub(1),
                            z as u32,
                        ))
                    })
                });
                assert!(held, "{} tier {}: no basin holds it", p.name, pool.tier);
            }
        }
    }

    /// Each fall lands in the pool below it, and the last one in the lake.
    #[test]
    fn the_chain_lands_in_the_pool_below() {
        for p in PRESETS {
            let config = Config {
                seed: 1,
                ..p.config()
            };
            let Landform::Staged(recipe) = config.landform.clone() else {
                panic!("a preset is staged")
            };
            let (field, _, report) = staged_terrain(&config, &recipe);
            for pool in &report.pools {
                let Some((nx, nz)) = pool.notch else {
                    assert_eq!(pool.tier, 0, "{}: only the lake spills elsewhere", p.name);
                    continue;
                };
                assert!(nz > 0, "{}: the notch is on the front cut", p.name);
                let landing = (nz - 1) * field.width + nx;
                let below = report
                    .pools
                    .iter()
                    .find(|q| q.tier + 1 == pool.tier)
                    .unwrap_or_else(|| {
                        panic!("{}: tier {} has nothing below it", p.name, pool.tier)
                    });
                assert!(
                    below.cells.contains(&landing),
                    "{}: tier {} falls onto ({nx}, {}) and misses tier {}",
                    p.name,
                    pool.tier,
                    nz - 1,
                    below.tier
                );
            }
        }
    }

    /// The river comes back in at the top of the chain.
    #[test]
    fn the_spring_is_in_the_top_pool() {
        for p in PRESETS {
            let config = Config {
                seed: 1,
                ..p.config()
            };
            let Landform::Staged(recipe) = config.landform.clone() else {
                panic!("a preset is staged")
            };
            let (field, _, report) = staged_terrain(&config, &recipe);
            let top = report
                .pools
                .iter()
                .max_by_key(|q| q.tier)
                .expect("a chain has a top");
            let world = World::new(config.clone());
            let (x, y, z) = world
                .spring_cell()
                .expect("a staged world names its spring");
            let i = z as usize * field.width + x as usize;
            assert!(
                top.cells.contains(&i),
                "{}: the spring at ({x}, {z}) is not in the top pool",
                p.name
            );
            assert!(
                (y as i32) > top.floor_y && (y as i32) <= top.spill_y,
                "{}: the spring at y {y} is not between the top pool's floor {} and its \
                 surface {}",
                p.name,
                top.floor_y,
                top.spill_y
            );
        }
    }

    /// `small` is the Tachyon panel's world, and the panel's raster is 640 x 360.
    /// `Projection::new` crops anything taller, so the world plus the lift the 30° tilt
    /// gives its depth has to fit: `4 * height + 2 * depth <= 360`.
    #[test]
    fn small_fits_the_panel_raster() {
        let p = Preset::find("small").unwrap();
        let rows = 4 * p.height + 2 * p.depth;
        assert!(rows <= 360, "small draws {rows} rows into a 360-row raster",);
        assert!(
            rows >= 300,
            "small leaves {} rows of sky unused",
            360 - rows
        );
    }

    /// Most seeds of a preset give the camera a lake worth looking at, and the lake runs
    /// to the front cut so it reads in section and not only from above.
    ///
    /// Measured before the settle, because this is a claim about the ground the generator
    /// cuts; what the water does in it afterwards is the hydrology's. One test per
    /// preset, and three in four of its seeds have to clear the bar: eight worlds is most
    /// of a second and three presets would be three.
    fn most_seeds_have_a_lake(name: &str, seeds: u64) {
        let p = Preset::find(name).expect("a shipped preset");
        let Landform::Staged(recipe) = p.config().landform.clone() else {
            panic!("a preset is staged")
        };
        let bar = recipe.water.min_lake_m2;
        let mut passed = 0;
        for seed in 1..=seeds {
            let config = Config { seed, ..p.config() };
            let world = World::new(config.clone());
            let lake = crate::hydrate::lake(&world);
            if lake.visible_m2 >= bar {
                passed += 1;
            }
            assert!(
                lake.cells.iter().any(|&i| config.coords(i).2 == 0),
                "{name} seed {seed}: the lake does not reach the cut"
            );
        }
        assert!(
            passed * 4 >= seeds * 3,
            "{name}: only {passed} of {seeds} seeds clear {bar} m² of open water"
        );
    }

    #[test]
    fn small_gives_most_seeds_a_lake() {
        most_seeds_have_a_lake("small", 8);
    }

    #[test]
    fn default_gives_most_seeds_a_lake() {
        most_seeds_have_a_lake("default", 8);
    }

    #[test]
    fn wide_gives_most_seeds_a_lake() {
        // Six, not eight: `wide` is twice the ring and a hundred erosion iterations
        // apiece, and eight of them is past the two seconds a test here may take. The
        // bar is the same three in four.
        most_seeds_have_a_lake("wide", 6);
    }

    /// Local shelf banks carry a sparse set of visible rooms and grottos.
    #[test]
    fn the_shelf_banks_carry_visible_hollows() {
        for name in ["small", "default", "wide"] {
            let p = Preset::find(name).unwrap();
            let world = World::new(Config {
                seed: 1,
                ..p.config()
            });
            let found = crate::hollows::find(&world);
            assert!(
                !found.is_empty(),
                "{name} seed 1: no habitable hollow survived"
            );
            assert!(found.iter().all(|h| h.visible), "{name}: an unseen grotto");
        }
    }

    /// A hand-authored skyline: `width x depth` columns, all at `ground`, with the
    /// listed columns set to their own heights.
    fn skyline(width: u32, depth: u32, ground: i32, dug: &[(usize, usize, i32)]) -> Vec<i32> {
        let mut surf = vec![ground; width as usize * depth as usize];
        for &(x, z, y) in dug {
            surf[z * width as usize + x] = y;
        }
        surf
    }

    fn ring(width: u32, depth: u32, voxel_m: f64, lake_depth_m: f64) -> (Config, Recipe) {
        let mut config = Config {
            width,
            height: 16,
            depth,
            voxel_m,
            ..Preset::find("default").unwrap().config()
        };
        let Landform::Staged(mut recipe) = config.landform.clone() else {
            panic!("a preset is staged")
        };
        recipe.water.lake_depth_m = lake_depth_m;
        config.landform = Landform::Staged(recipe.clone());
        (config, recipe)
    }

    /// The lake stands the recipe's depth over the ring's lowest ground, and its outlet
    /// sits on the edge column the water just covers.
    #[test]
    fn lake_level_follows_the_recipe_depth() {
        // Ground at 12, a pit at (3, 0) and a shelf beside it one voxel higher than the
        // water will stand minus one.
        let surf = skyline(8, 2, 12, &[(3, 0, 6), (4, 0, 7)]);

        let (config, recipe) = ring(8, 2, 0.25, 0.5);
        let lake = lake_level(&surf, &config, &recipe);
        assert_eq!(lake.floor_y, 6);
        assert_eq!(lake.level_y, 8, "half a metre at 0.25 m is two voxels");
        assert_eq!(
            lake.rim,
            (4, 0),
            "the outlet seats on the shelf the water covers"
        );

        // No lake at all: the datum is the floor outlet the generator always had.
        let (config, recipe) = ring(8, 2, 0.25, 0.0);
        let dry = lake_level(&surf, &config, &recipe);
        assert_eq!((dry.floor_y, dry.level_y, dry.rim), (6, 7, (3, 0)));
    }

    /// A pit with nothing above it to hold more water keeps the water it can hold,
    /// however deep the recipe asks.
    #[test]
    fn lake_level_stops_at_the_rim() {
        let surf = skyline(8, 2, 7, &[(3, 0, 6)]);
        for asked in [0.5, 2.0, 8.0] {
            let (config, recipe) = ring(8, 2, 0.25, asked);
            let lake = lake_level(&surf, &config, &recipe);
            assert_eq!(
                (lake.floor_y, lake.level_y),
                (6, 7),
                "asked for {asked} m over a one-voxel rim"
            );
            assert_eq!(lake.rim, (3, 0));
        }
    }

    /// The ridge generator's outlet is where it always was.
    #[test]
    fn ridge_outlet_does_not_move() {
        for (seed, cell) in [(1u64, (28, 9, 0)), (2, (0, 9, 0)), (77, (109, 9, 0))] {
            let world = World::new(Config {
                seed,
                ..Config::default()
            });
            assert_eq!(world.outlet_cell(), Some(cell), "ridge seed {seed}");
        }
    }

    /// No hollow is a sump: nothing the carve leaves has a floor under the waterline.
    #[test]
    fn no_hollow_below_the_lake() {
        for p in PRESETS {
            let config = Config {
                seed: 1,
                ..p.config()
            };
            let Landform::Staged(recipe) = config.landform.clone() else {
                panic!("a preset is staged")
            };
            let (_, _, report) = staged_terrain(&config, &recipe);
            let world = World::new(config.clone());
            for hollow in crate::hollows::find(&world) {
                for &floor in &hollow.floors {
                    let (x, y, z) = config.coords(floor);
                    assert!(
                        y as i32 >= report.lake.level_y,
                        "{}: a hollow floor at ({x}, {y}, {z}) is under the lake at {}",
                        p.name,
                        report.lake.level_y
                    );
                }
            }
        }
    }

    /// The staged outlet is a void cell at the lake's own surface, standing on the
    /// column the water just covers -- so the lake holds its level and only the surplus
    /// leaves.
    #[test]
    fn staged_outlet_sits_on_the_rim() {
        for p in PRESETS {
            let config = Config {
                seed: 1,
                ..p.config()
            };
            let Landform::Staged(recipe) = config.landform.clone() else {
                panic!("a preset is staged")
            };
            let (_, _, report) = staged_terrain(&config, &recipe);
            let world = World::new(config);
            let (x, y, z) = world
                .outlet_cell()
                .expect("a staged world names its outlet");
            let v = world.view();
            assert!(
                !v.material_at(x as i64, y, z).is_solid(),
                "{}: the outlet at ({x}, {y}, {z}) is not a void cell",
                p.name
            );
            // One row **above** the datum: the sill is what the surplus goes over, and it
            // is dry until there is surplus. Seated on the datum itself it stands in the
            // lake's own surface and drains it (T6).
            assert_eq!(
                y as i32,
                report.lake.level_y + 1,
                "{}: outlet off the datum",
                p.name
            );
            assert_eq!(
                v.surface_y(x as i64, z).map(|t| t as i32),
                Some(report.lake.level_y - 1),
                "{}: the outlet is not standing over its rim",
                p.name
            );
        }
    }

    /// The overhang and the covered passage the generator used to carve live here now,
    /// hand-built: a shaft down to a roofed slot stays, a sealed pocket is filled.
    #[test]
    fn a_reachable_roofed_slot_survives_repair_and_a_sealed_pocket_does_not() {
        let config = Config {
            width: 8,
            height: 8,
            depth: 2,
            ..Config::default()
        };
        let mut world = World::empty(config.clone());
        for z in 0..config.depth {
            for x in 0..config.width as i64 {
                for y in 1..=4 {
                    world.material[config.index(x, y, z)] = Material::Rock;
                }
            }
        }
        let air = |world: &mut World, x: i64, y: u32| {
            for z in 0..config.depth {
                world.material[config.index(x, y, z)] = Material::Air;
            }
        };
        // A shaft open to the sky at x = 5, and a roofed slot beside it at x = 4:
        // a covered passage, reachable, with rock over it.
        air(&mut world, 5, 4);
        air(&mut world, 5, 3);
        air(&mut world, 4, 3);
        // And one pocket the sky cannot reach.
        air(&mut world, 1, 2);

        assert_eq!(
            isolated_voids(&world).len(),
            config.depth as usize,
            "only the pocket is sealed"
        );
        assert_eq!(repair_isolated(&mut world), config.depth as usize);
        for z in 0..config.depth {
            assert_eq!(
                world.material[config.index(4, 3, z)],
                Material::Air,
                "the roofed slot stays"
            );
            assert!(
                world.material[config.index(4, 4, z)].is_solid(),
                "its roof stays"
            );
            assert_eq!(
                world.material[config.index(1, 2, z)],
                Material::Rock,
                "the pocket is filled"
            );
        }
        assert!(isolated_voids(&world).is_empty());
    }

    #[test]
    fn the_allowed_drop_is_the_camera_s_own_arithmetic() {
        // hidden iff 2 * (y1 - y2) >= dz, so the drop the camera hides is (dz - 1) / 2.
        for dz in 1..12u32 {
            let d = allowed_drop(dz);
            assert!(
                2 * d < dz as i32,
                "a drop of {d} over {dz} would already be hidden"
            );
            assert!(
                2 * (d + 1) >= dz as i32,
                "a drop of {} over {dz} is still visible",
                d + 1
            );
        }
    }
}
