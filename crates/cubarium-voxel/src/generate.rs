//! Feature-guided landform generation, periodic in `x`.
//!
//! The order is deliberate: one broad ridge and one receiving basin first, then the
//! soil and rock body under that surface, then the two carved features — one overhang
//! and one covered passage — and only then a weak correlated wobble. Soil depth comes
//! from slope and deposition, never from the elevation noise, so flanks are bare and
//! the basin is deep in soil. Every carve is rechecked for voids the sky cannot reach
//! and those are filled back in.
//!
//! Everything periodic uses whole harmonics of the strip, so `x = 0` is an ordinary
//! interior column: nothing in here can see the seam.

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

/// Fill `world.material` from `world.config`, and name the basin's outlet and spring.
pub fn landform(world: &mut World) {
    let c = world.config.clone();
    let (w, h, d) = (c.width as usize, c.height as usize, c.depth as usize);
    if w == 0 || h < 4 || d == 0 {
        return;
    }
    let mut rng = Rng::new(c.seed ^ 0x_4C41_4E44_0001);

    // ---- the broad shape: one ridge, one receiving basin, periodic in x ----
    let hf = h as f64;
    let base = hf * 0.46;
    let amp1 = (hf * 0.22).max(1.0);
    let amp2 = hf * 0.07;
    let tilt_z = hf * 0.06;
    let ridge_phase = rng.unit() * TAU;
    let second_phase = rng.unit() * TAU;
    let basin_floor = base - amp1 * 0.72;

    // ---- the weak correlated wobble, added last and small ----
    let noise: Vec<(f64, f64, f64, f64)> = (0..4)
        .map(|k| {
            let harmonic = (3 + k * 2) as f64;
            let amp = 0.55 / (1.0 + k as f64 * 0.6);
            (harmonic, amp, rng.unit() * TAU, rng.range(0.15, 0.6))
        })
        .collect();

    let hi_clamp = (hf - 5.0).max(3.0);
    let mut surf = vec![0i32; w * d];
    for z in 0..d {
        for x in 0..w {
            let th = TAU * x as f64 / w as f64;
            let mut e = base + amp1 * (th - ridge_phase).cos() + amp2 * (2.0 * th - second_phase).cos();
            // A receiving basin, not a smooth trough: the bottom flattens into a floor.
            if e < basin_floor {
                e = basin_floor + (basin_floor - e) * 0.12;
            }
            if d > 1 {
                e += tilt_z * (z as f64 / (d - 1) as f64 - 0.5);
            }
            for &(k, amp, phase, zk) in &noise {
                e += amp * (k * th + phase).sin() * (zk * z as f64 + phase).cos();
            }
            surf[z * w + x] = e.round().clamp(3.0, hi_clamp) as i32;
        }
    }

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

    let at = |x: i64, z: usize| surf[z * w + x.rem_euclid(w as i64) as usize];
    let zm = d / 2;
    let z_lo = zm.saturating_sub(1);
    let z_hi = (zm + 1).min(d - 1);

    // ---- one overhang: a slot bitten into the steepest descending face ----
    let span = (w as i64 / 8).clamp(1, 5);
    let mut best = (0i64, i32::MIN);
    for x in 0..w as i64 {
        let drop = at(x, zm) - at(x + span, zm);
        if drop > best.1 {
            best = (x, drop);
        }
    }
    let x_lo = best.0 + span;
    let ya = (at(x_lo, zm) + 1).max(1);
    if (ya as usize) + 1 < h {
        for step in 0..(span + 4) {
            let x = x_lo - step;
            if at(x, zm) < ya + 2 {
                continue;
            }
            for y in ya..=ya + 1 {
                for z in z_lo..=z_hi {
                    carve(world, x, y as u32, z as u32);
                }
            }
        }
        repair_isolated(world);
    }

    // ---- one covered passage: straight through the ridge, roofed all the way ----
    let mut crest = (0i64, i32::MIN);
    for x in 0..w as i64 {
        if at(x, zm) > crest.1 {
            crest = (x, at(x, zm));
        }
    }
    let yp = (crest.1 - 4).max(2);
    let cap = (w as i64 / 2 - 1).max(1);
    let mut left = 0i64;
    while left < cap && at(crest.0 - left, zm) >= yp {
        left += 1;
    }
    let mut right = 0i64;
    while right < cap && at(crest.0 + right, zm) >= yp {
        right += 1;
    }
    if left < cap && right < cap && (yp as usize) + 1 < h {
        for x in (crest.0 - left)..=(crest.0 + right) {
            for y in yp..=yp + 1 {
                for z in z_lo..=z_hi {
                    carve(world, x, y as u32, z as u32);
                }
            }
        }
        repair_isolated(world);
    }

    // ---- outlet and spring, from the basin we just built ----
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

    repair_isolated(world);
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

fn carve(world: &mut World, x: i64, y: u32, z: u32) {
    let i = world.config.index(x, y, z);
    if y == 0 {
        return;
    }
    world.material[i] = Material::Air;
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
