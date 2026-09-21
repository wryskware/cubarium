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

use crate::recipe::{Landform, Recipe};
use crate::{Material, World};

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

    outlet_and_spring(world, &surf, w, d, h);

    // Nothing above carves, so this finds nothing — it is the guard that keeps it so.
    repair_isolated(world);
}

/// Name the outlet at the lowest cell of the generated surface and the spring an eighth
/// of the ring along from it, at mid-depth.
fn outlet_and_spring(world: &mut World, surf: &[i32], w: usize, d: usize, h: usize) {
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
    let outlet_y = (low.2 + 1).min(h as i32 - 1) as u32;
    world.outlet_cell = Some((low.0 as u32, outlet_y, low.1 as u32));
    let sx = ((low.0 + w / 8) % w) as i64;
    let spring_y = (at(sx, zm) + 1).min(h as i32 - 1) as u32;
    world.spring_cell = Some((sx as u32, spring_y, zm as u32));
}

/// Staged relief in physical units: [`Landform::Staged`].
///
/// Four things, in order. Broad multi-octave relief and the ridged noise warped into the
/// rocky stretches come from [`Recipe::relief_m_at`], all of it in metres and all of it
/// exactly periodic around the ring. A receiving basin compresses whatever falls below
/// [`Recipe::basin_floor_m`] instead of cutting it. Every column is then tilted, if it
/// needs it, until the back stands at least half [`Recipe::back_rise_m`] above the front
/// — the relief varies with depth too, and no visibility pass can repair a column whose
/// far edge is genuinely the lower one. Only then is the field quantised onto the voxel
/// grid and handed to [`visibility_pass`], which lowers whatever still occludes the
/// terrain behind it, exactly as it does for the ridge generator.
///
/// Feature size is the recipe's and nothing else: `width` decides how many landforms fit
/// around the ring, `voxel_m` decides how finely they are resolved, and octaves finer
/// than [`Recipe::min_feature_voxels`] voxels are dropped rather than aliased.
///
/// Material layering is carried over from the ridge generator — soil from slope and
/// deposition, rock with hard periodic strata, a bedrock core, soil pockets — with every
/// thickness in metres. Slice 2 replaces the slope-derived soil with eroded sediment.
fn staged(world: &mut World, r: &Recipe) {
    let c = world.config.clone();
    let (w, h, d) = (c.width as usize, c.height as usize, c.depth as usize);
    if w == 0 || h < 4 || d == 0 {
        return;
    }
    let vm = c.voxel_m;
    let circumference_m = w as f64 * vm;
    let hi_clamp = (h as f64 - 5.0).max(FLOOR_Y as f64);

    // ---- the heightfield, in metres ----
    let mut e = vec![0.0f64; w * d];
    for z in 0..d {
        let z_m = (z as f64 + 0.5) * vm;
        let climb = if d > 1 {
            r.back_rise_m * z as f64 / (d - 1) as f64
        } else {
            0.0
        };
        for x in 0..w {
            let x_m = (x as f64 + 0.5) * vm;
            let mut el = r.base_m + r.relief_m_at(x_m, z_m, circumference_m, vm, c.seed);
            // A receiving basin with a floor, not a clipped trough: relief below the
            // floor is compressed, so the basin has a bottom and keeps its shape.
            if el < r.basin_floor_m {
                el = r.basin_floor_m - (r.basin_floor_m - el) * 0.12;
            }
            e[z * w + x] = el + climb;
        }
    }

    // ---- the camera's climb, guaranteed per column ----
    // A linear tilt, so whatever the relief did between front and back survives it; only
    // the two ends are what the landform rule and the readable diorama need.
    let min_climb_m = r.back_rise_m * 0.5;
    if d > 1 {
        for x in 0..w {
            let short = min_climb_m - (e[(d - 1) * w + x] - e[x]);
            if short > 0.0 {
                for z in 1..d {
                    e[z * w + x] += short * z as f64 / (d - 1) as f64;
                }
            }
        }
    }

    let mut surf = vec![0i32; w * d];
    for i in 0..w * d {
        surf[i] = (e[i] / vm).round().clamp(FLOOR_Y as f64, hi_clamp) as i32;
    }
    visibility_pass(&mut surf, w, d, FLOOR_Y);
    debug_assert!(
        surf.iter().all(|&y| y >= 1 && (y as usize) < h - 1),
        "the surface left the world after the visibility pass"
    );

    // ---- soil from slope and deposition, in metres ----
    let mean_m = surf.iter().map(|&s| s as f64).sum::<f64>() / (w * d) as f64 * vm;
    let soil_cap = (r.soil_max_m / vm).round().max(0.0);
    // Half a metre either side, whatever the voxel size: a gradient measured over a
    // length, so the same physical slope carries the same soil at any resolution and a
    // single voxel of quantisation does not strip a column bare.
    let span = ((0.5 / vm).round().max(1.0) as usize).min(w / 2).max(1);
    let mut soil = vec![0i32; w * d];
    for z in 0..d {
        for x in 0..w {
            let left = surf[z * w + (x + w - span) % w] as f64;
            let right = surf[z * w + (x + span) % w] as f64;
            let slope = (right - left).abs() / (2.0 * span as f64);
            let here_m = surf[z * w + x] as f64 * vm;
            let deposition = ((mean_m - here_m) / r.relief_m.max(1e-9)).clamp(0.0, 1.0);
            let s_m = r.soil_flat_m * (1.0 - slope.min(1.0))
                + r.soil_deposit_m * deposition
                + r.soil_floor_m;
            soil[z * w + x] = (s_m / vm).round().clamp(0.0, soil_cap) as i32;
        }
    }

    // ---- the body: bedrock core, rock with hard strata, soil on top ----
    let mut rng = Rng::new(c.seed ^ r.streams.material);
    let rock_phase = rng.unit() * TAU;
    let strata_phase = rng.unit() * TAU;
    let strata_m = r.strata_m.max(1e-9);
    for z in 0..d {
        for x in 0..w {
            let th = TAU * x as f64 / w as f64;
            let top = surf[z * w + x];
            let soil_depth = soil[z * w + x];
            let rock_thick = (r.rock_m + r.rock_swing_m * (th + rock_phase).sin()) / vm;
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
                    let warped = y as f64 * vm + r.strata_warp_m * (th + strata_phase).sin();
                    if (warped / strata_m).floor() as i64 % 4 == 0 {
                        m = Material::Bedrock;
                    }
                }
                world.material[c.index(x as i64, y as u32, z as u32)] = m;
            }
            world.material[c.index(x as i64, 0, z as u32)] = Material::Bedrock;
        }
    }

    // ---- soil pockets inside the rock, sized in metres ----
    for _ in 0..r.pockets {
        let cx = rng.unit() * w as f64;
        let cz = rng.range(0.0, d as f64);
        let cy = rng.range(0.5, (h as f64 * vm * 0.5).max(0.75)) / vm;
        let (rx, ry, rz) = (
            rng.range(0.5, 1.25) / vm,
            rng.range(0.375, 0.75) / vm,
            rng.range(0.375, 1.0) / vm,
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

    outlet_and_spring(world, &surf, w, d, h);
    repair_isolated(world);
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
    use crate::{Config, PRESETS, Preset};

    const SEEDS: [u64; 3] = [1, 2, 77];

    /// Every ring the camera rules have to hold for: the ridge default and the three
    /// staged presets, on three seeds each.
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
            for p in PRESETS {
                out.push((
                    format!("{} seed {seed}", p.name),
                    Config { seed, ..p.config() },
                ));
            }
        }
        out
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

    #[test]
    fn no_surface_cell_is_hidden_by_a_nearer_one() {
        for (name, config) in rings() {
            let world = World::new(config);
            for x in 0..world.config().width as i64 {
                let ys = column(&world, x);
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
                    front < back,
                    "{name}, x {x}: front {front} not below back {back}"
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

    #[test]
    fn the_default_landform_has_no_overhang() {
        for (name, config) in rings() {
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

    /// `x = 0` is an ordinary interior column: across the whole ring, no seam column pair
    /// steps further, or changes more of its material column, than the worst interior
    /// pair does. (Per depth row it would be a coin toss which pair holds the row's
    /// maximum; over the ring it is a statement about the seam.) And the field itself
    /// repeats bit for bit one circumference along — a periodic lattice, not a
    /// crossfade.
    #[test]
    fn a_staged_ring_has_no_seam() {
        for p in PRESETS {
            for seed in SEEDS {
                let world = World::new(Config { seed, ..p.config() });
                let v = world.view();
                let (w, h, d) = (v.config.width as i64, v.config.height, v.config.depth);
                let (mut worst_step, mut worst_change) = (0i32, 0usize);
                let (mut seam_step, mut seam_change) = (0i32, 0usize);
                for z in 0..d {
                    let top = |x: i64| v.surface_y(x, z).expect("ground") as i32;
                    let changed = |a: i64, b: i64| {
                        (0..h)
                            .filter(|&y| v.material_at(a, y, z) != v.material_at(b, y, z))
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
            for seed in SEEDS {
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
        let base = Preset::find("default").unwrap().config();
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

    /// Halving the voxel resolves the same landforms more finely: at every coarse column
    /// the fine world's surface agrees within one coarse voxel, in metres.
    #[test]
    fn halving_the_voxel_resolves_the_same_landforms_finer() {
        let base = Preset::find("default").unwrap().config();
        for seed in SEEDS {
            let coarse = World::new(Config {
                seed,
                ..base.clone()
            });
            // The same 32 m ring at half the cell size is twice the cells in every
            // direction: the same world resolved finer, not a shallower one.
            let fine = World::new(Config {
                seed,
                width: base.width * 2,
                height: base.height * 2,
                depth: base.depth * 2,
                voxel_m: base.voxel_m / 2.0,
                ..base.clone()
            });
            let (cv, fv) = (coarse.view(), fine.view());
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
                worst <= base.voxel_m,
                "seed {seed}: the fine ring differs by {worst:.3} m, more than one {:.3} m voxel",
                base.voxel_m
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
