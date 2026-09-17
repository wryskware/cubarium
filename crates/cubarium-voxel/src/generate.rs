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
pub fn landform(world: &mut World) {
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
    let noise: Vec<(f64, f64, f64, f64)> = (0..4)
        .map(|k| {
            let harmonic = (3 + k * 2) as f64;
            let amp = 0.55 / (1.0 + k as f64 * 0.6);
            (harmonic, amp, rng.unit() * TAU, rng.range(0.05, 0.22))
        })
        .collect();

    let hi_clamp = (hf - 5.0).max(FLOOR_Y as f64);
    let mut surf = vec![0i32; w * d];
    for z in 0..d {
        for x in 0..w {
            let th = TAU * x as f64 / w as f64;
            let mut e = front + amp1 * (th - ridge_phase).cos() + amp2 * (2.0 * th - second_phase).cos();
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
        let (rx, ry, rz) = (rng.range(2.0, 5.0), rng.range(1.5, 3.0), rng.range(1.5, 4.0));
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

    // ---- outlet and spring, from the basin we just built ----
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

    // Nothing above carves, so this finds nothing — it is the guard that keeps it so.
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
    (0..n).filter(|&i| !world.material[i].is_solid() && !seen[i]).collect()
}

/// Fill every void the sky cannot reach with rock. Run after every carve.
pub fn repair_isolated(world: &mut World) -> usize {
    let pockets = isolated_voids(world);
    for &i in &pockets {
        world.material[i] = Material::Rock;
        world.free[i] = 0.0;
    }
    pockets.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;

    const SEEDS: [u64; 3] = [1, 2, 77];

    /// Surface heights front to back for one column, as the camera sees them.
    fn column(world: &World, x: i64) -> Vec<i32> {
        let v = world.view();
        (0..v.config.depth)
            .map(|z| v.surface_y(x, z).expect("every column has ground") as i32)
            .collect()
    }

    #[test]
    fn no_surface_cell_is_hidden_by_a_nearer_one() {
        for seed in SEEDS {
            let world = World::new(Config { seed, ..Config::default() });
            for x in 0..world.config().width as i64 {
                let ys = column(&world, x);
                for z1 in 0..ys.len() {
                    for z2 in z1 + 1..ys.len() {
                        let dz = (z2 - z1) as i32;
                        assert!(
                            2 * (ys[z1] - ys[z2]) < dz,
                            "seed {seed}, x {x}: y{z1}={} hides y{z2}={} across {dz} of depth",
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
        for seed in SEEDS {
            let world = World::new(Config { seed, ..Config::default() });
            let w = world.config().width as i64;
            let mut total = 0i64;
            for x in 0..w {
                let ys = column(&world, x);
                let (front, back) = (ys[0], ys[ys.len() - 1]);
                assert!(front < back, "seed {seed}, x {x}: front {front} not below back {back}");
                total += (back - front) as i64;
            }
            let mean = total as f64 / w as f64;
            assert!(mean >= 4.0, "seed {seed}: the surface only climbs {mean:.1} voxels on average");
        }
    }

    #[test]
    fn the_default_landform_has_no_overhang() {
        for seed in SEEDS {
            let world = World::new(Config { seed, ..Config::default() });
            let v = world.view();
            for z in 0..v.config.depth {
                for x in 0..v.config.width as i64 {
                    let top = v.surface_y(x, z).expect("every column has ground");
                    for y in 0..top {
                        assert!(
                            v.material_at(x, y, z).is_solid(),
                            "seed {seed}: void at ({x}, {y}, {z}) under the surface at {top}"
                        );
                    }
                }
            }
        }
    }

    /// The overhang and the covered passage the generator used to carve live here now,
    /// hand-built: a shaft down to a roofed slot stays, a sealed pocket is filled.
    #[test]
    fn a_reachable_roofed_slot_survives_repair_and_a_sealed_pocket_does_not() {
        let config = Config { width: 8, height: 8, depth: 2, ..Config::default() };
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

        assert_eq!(isolated_voids(&world).len(), config.depth as usize, "only the pocket is sealed");
        assert_eq!(repair_isolated(&mut world), config.depth as usize);
        for z in 0..config.depth {
            assert_eq!(world.material[config.index(4, 3, z)], Material::Air, "the roofed slot stays");
            assert!(world.material[config.index(4, 4, z)].is_solid(), "its roof stays");
            assert_eq!(world.material[config.index(1, 2, z)], Material::Rock, "the pocket is filled");
        }
        assert!(isolated_voids(&world).is_empty());
    }

    #[test]
    fn the_allowed_drop_is_the_camera_s_own_arithmetic() {
        // hidden iff 2 * (y1 - y2) >= dz, so the drop the camera hides is (dz - 1) / 2.
        for dz in 1..12u32 {
            let d = allowed_drop(dz);
            assert!(2 * d < dz as i32, "a drop of {d} over {dz} would already be hidden");
            assert!(2 * (d + 1) >= dz as i32, "a drop of {} over {dz} is still visible", d + 1);
        }
    }
}

