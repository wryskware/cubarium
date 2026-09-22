//! Periodic gradient noise for terrain generation, in metres.
//!
//! One rule governs everything in here: **the lattice period is the ring circumference
//! in metres**. A field is evaluated at a metre coordinate reduced modulo the
//! circumference before it ever touches the lattice, so `f(x)` and `f(x + C)` are the
//! same float — not two nearly-equal floats joined by a crossfade. `x` therefore has no
//! seam for any later pass to see, including a domain warp, which is itself periodic and
//! so cannot smuggle one in.
//!
//! Depth is not periodic: `z = 0` is the viewing plane and `z = depth-1` the back wall,
//! both no-flow. The z lattice simply continues, at the same cell size as x, so features
//! are the same size in both directions before the recipe's depth anisotropy.
//!
//! Feature size is in metres and nothing else. An octave asks for a wavelength in metres;
//! the ring is divided into the nearest whole number of lattice cells, so the realised
//! wavelength is `circumference / cells` — a landform, not a fraction of the array.
//! Widening the world therefore adds cells and adds landforms; shrinking `voxel_m`
//! changes no wavelength at all, it only samples the same field more finely.

use std::f64::consts::TAU;

/// One octave ladder: the coarsest wavelength in metres and how the rest follow it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ladder {
    /// Wavelength of octave 0, metres.
    pub wavelength_m: f64,
    /// How many octaves the ladder asks for. Octaves finer than the resolution limit are
    /// dropped, never aliased, so the number *resolved* can be smaller.
    pub octaves: u32,
    /// Amplitude of each octave relative to the one before.
    pub persistence: f64,
    /// Frequency step between octaves.
    pub lacunarity: f64,
}

/// Lattice cells around the ring for one wavelength: the nearest whole number, at least
/// one. This is what makes the field exactly periodic, and it is why the realised
/// wavelength of an octave is `circumference_m / cells`.
pub fn ring_cells(circumference_m: f64, wavelength_m: f64) -> u32 {
    if !(wavelength_m > 0.0 && circumference_m > 0.0) {
        return 1;
    }
    (circumference_m / wavelength_m).round().clamp(1.0, 1e6) as u32
}

/// Octaves of `ladder` whose wavelength is at least `min_wavelength_m`. Finer octaves are
/// dropped: below about two voxels an octave is noise on the quantization, not terrain.
pub fn resolved_octaves(ladder: &Ladder, min_wavelength_m: f64) -> u32 {
    let mut wl = ladder.wavelength_m;
    let mut n = 0;
    for _ in 0..ladder.octaves {
        if wl < min_wavelength_m {
            break;
        }
        n += 1;
        wl /= ladder.lacunarity.max(1.000_001);
    }
    n
}

/// splitmix64 over a lattice point and a stream seed. No clock, no thread state.
fn hash2(ix: i64, iz: i64, seed: u64) -> u64 {
    let mut z = seed
        ^ (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (iz as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Unit gradient at a lattice point.
fn gradient(ix: i64, iz: i64, seed: u64) -> (f64, f64) {
    let a = (hash2(ix, iz, seed) >> 11) as f64 / (1u64 << 53) as f64 * TAU;
    (a.cos(), a.sin())
}

/// Perlin's quintic fade: zero first and second derivative at both ends, so octaves join
/// without a visible lattice.
fn quintic(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Gradient noise, roughly `-1..1`, periodic in `x` with exactly `cells` lattice cells
/// around `circumference_m`. `z` runs on the same lattice and does not wrap.
pub fn ring_noise(x_m: f64, z_m: f64, circumference_m: f64, cells: u32, seed: u64) -> f64 {
    let cells = cells.max(1);
    let cell_m = circumference_m / cells as f64;
    if !(cell_m > 0.0 && cell_m.is_finite()) {
        return 0.0;
    }
    // Reduce in metres *first*: `x` and `x + circumference` become the same float here,
    // which is what makes the seam invisible rather than merely small.
    let fx = x_m.rem_euclid(circumference_m) / cell_m;
    let fz = z_m / cell_m;
    let (ix, iz) = (fx.floor(), fz.floor());
    let (tx, tz) = (fx - ix, fz - iz);
    let (i0, j0) = (ix as i64, iz as i64);
    let (ux, uz) = (quintic(tx), quintic(tz));
    let n = cells as i64;
    let mut acc = 0.0;
    for (dx, dz) in [(0i64, 0i64), (1, 0), (0, 1), (1, 1)] {
        let (gx, gz) = gradient((i0 + dx).rem_euclid(n), j0 + dz, seed);
        let (ox, oz) = (tx - dx as f64, tz - dz as f64);
        let wx = if dx == 0 { 1.0 - ux } else { ux };
        let wz = if dz == 0 { 1.0 - uz } else { uz };
        acc += (gx * ox + gz * oz) * wx * wz;
    }
    acc * GAIN
}

/// Scales gradient noise to about `-1..1` *in practice*. Two-dimensional gradient noise
/// can reach `sqrt(2)/2` in principle and does not: over a ring of samples it stays
/// inside about `+/-0.41`. Normalising by the unreachable bound would make every recipe's
/// relief four tenths of what it asked for, so this is the measured figure, and callers
/// clamp the rare excursion past it rather than shrinking everything to fit it.
const GAIN: f64 = 2.4;

/// Unit gradient at a lattice point of the three-dimensional lattice.
fn gradient3(ix: i64, iy: i64, iz: i64, seed: u64) -> (f64, f64, f64) {
    let a = hash2(
        ix,
        iz,
        seed ^ (iy as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
    );
    // A direction on the sphere from two independent angles.
    let theta = (a >> 11) as f64 / (1u64 << 53) as f64 * TAU;
    let cos_phi = ((a << 11) >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0;
    let sin_phi = (1.0 - cos_phi * cos_phi).max(0.0).sqrt();
    (sin_phi * theta.cos(), cos_phi, sin_phi * theta.sin())
}

/// Gradient noise in three dimensions, roughly `-1..1`, periodic in `x` with exactly
/// `cells` lattice cells around `circumference_m`. `y` and `z` run on the same lattice
/// and do not wrap: up is up and the habitat has two walls.
///
/// Same rule as [`ring_noise`]: the metre coordinate is reduced modulo the circumference
/// before it reaches the lattice, so a gallery that runs off one end of the ring arrives
/// at the other as the same gallery.
pub fn ring_noise_3d(
    x_m: f64,
    y_m: f64,
    z_m: f64,
    circumference_m: f64,
    cells: u32,
    seed: u64,
) -> f64 {
    let cells = cells.max(1);
    let cell_m = circumference_m / cells as f64;
    if !(cell_m > 0.0 && cell_m.is_finite()) {
        return 0.0;
    }
    let fx = x_m.rem_euclid(circumference_m) / cell_m;
    let (fy, fz) = (y_m / cell_m, z_m / cell_m);
    let (ix, iy, iz) = (fx.floor(), fy.floor(), fz.floor());
    let (tx, ty, tz) = (fx - ix, fy - iy, fz - iz);
    let (i0, j0, k0) = (ix as i64, iy as i64, iz as i64);
    let (ux, uy, uz) = (quintic(tx), quintic(ty), quintic(tz));
    let n = cells as i64;
    let mut acc = 0.0;
    for (dx, dy, dz) in [
        (0i64, 0i64, 0i64),
        (1, 0, 0),
        (0, 1, 0),
        (1, 1, 0),
        (0, 0, 1),
        (1, 0, 1),
        (0, 1, 1),
        (1, 1, 1),
    ] {
        let (gx, gy, gz) = gradient3((i0 + dx).rem_euclid(n), j0 + dy, k0 + dz, seed);
        let (ox, oy, oz) = (tx - dx as f64, ty - dy as f64, tz - dz as f64);
        let wx = if dx == 0 { 1.0 - ux } else { ux };
        let wy = if dy == 0 { 1.0 - uy } else { uy };
        let wz = if dz == 0 { 1.0 - uz } else { uz };
        acc += (gx * ox + gy * oy + gz * oz) * wx * wy * wz;
    }
    acc * GAIN
}

/// Walk the resolved octaves of a ladder, handing each one its cell count and amplitude.
/// Returns the amplitude sum, so a caller can normalise.
fn walk(
    ladder: &Ladder,
    circumference_m: f64,
    min_wavelength_m: f64,
    mut each: impl FnMut(u32, u32, f64),
) -> f64 {
    let mut wl = ladder.wavelength_m;
    let mut amp = 1.0;
    let mut norm = 0.0;
    for k in 0..ladder.octaves {
        if wl < min_wavelength_m {
            break;
        }
        each(k, ring_cells(circumference_m, wl), amp);
        norm += amp;
        wl /= ladder.lacunarity.max(1.000_001);
        amp *= ladder.persistence;
    }
    norm
}

/// One octave's own stream, so adding an octave does not reshuffle the ones below it.
fn octave_seed(seed: u64, k: u32) -> u64 {
    seed ^ (k as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// Fractal Brownian motion over the resolved octaves, normalised to roughly `-1..1`.
pub fn fbm(
    x_m: f64,
    z_m: f64,
    circumference_m: f64,
    ladder: &Ladder,
    min_wavelength_m: f64,
    seed: u64,
) -> f64 {
    let mut acc = 0.0;
    let norm = walk(
        ladder,
        circumference_m,
        min_wavelength_m,
        |k, cells, amp| {
            acc += amp * ring_noise(x_m, z_m, circumference_m, cells, octave_seed(seed, k));
        },
    );
    if norm > 0.0 {
        (acc / norm).clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

/// Ridged multifractal over the resolved octaves, normalised to roughly `-1..1` with the
/// crests at the top: `1 - 2|n|` per octave, which turns the noise's zero crossings into
/// sharp ridges instead of smooth swells.
pub fn ridged(
    x_m: f64,
    z_m: f64,
    circumference_m: f64,
    ladder: &Ladder,
    min_wavelength_m: f64,
    seed: u64,
) -> f64 {
    let mut acc = 0.0;
    let norm = walk(
        ladder,
        circumference_m,
        min_wavelength_m,
        |k, cells, amp| {
            let n = ring_noise(x_m, z_m, circumference_m, cells, octave_seed(seed, k));
            acc += amp * (1.0 - 2.0 * n.abs().min(1.0));
        },
    );
    if norm > 0.0 {
        (acc / norm).clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

/// Hermite blend, clamped: `0` at or below `lo`, `1` at or above `hi`.
pub fn smoothstep(lo: f64, hi: f64, v: f64) -> f64 {
    if hi.is_nan() || lo.is_nan() || hi <= lo {
        return if v >= hi { 1.0 } else { 0.0 };
    }
    let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LADDER: Ladder = Ladder {
        wavelength_m: 16.0,
        octaves: 6,
        persistence: 0.5,
        lacunarity: 2.0,
    };

    /// The seam is not a crossfade: the same float comes back one circumference along.
    #[test]
    fn the_field_repeats_bit_exactly_after_one_circumference() {
        let circ = 32.0;
        // Column centres of a 0.25 m ring: `x + circ` is exact, so a difference here is
        // the field's and not the caller's rounding.
        for i in 0..128 {
            let x = i as f64 * 0.25;
            for z in [0.0, 1.5, 5.75] {
                let here = fbm(x, z, circ, &LADDER, 0.5, 9);
                for laps in [1.0, 2.0, -3.0] {
                    let there = fbm(x + laps * circ, z, circ, &LADDER, 0.5, 9);
                    assert_eq!(here.to_bits(), there.to_bits(), "x {x} z {z} laps {laps}");
                }
            }
            let r = ridged(x, 0.0, circ, &LADDER, 0.5, 9);
            assert_eq!(
                r.to_bits(),
                ridged(x + circ, 0.0, circ, &LADDER, 0.5, 9).to_bits()
            );
        }
    }

    /// Doubling the ring keeps the wavelength in metres and doubles the lattice: more
    /// geography, not a stretched hill.
    #[test]
    fn a_wider_ring_holds_more_landforms_of_the_same_size() {
        assert_eq!(ring_cells(32.0, 16.0), 2);
        assert_eq!(ring_cells(64.0, 16.0), 4);
        assert_eq!(ring_cells(20.0, 10.0), 2);
        // Under one cell per ring still has a period; it does not divide by zero.
        assert_eq!(ring_cells(8.0, 40.0), 1);
    }

    /// Octaves finer than the voxel limit are dropped, not aliased.
    #[test]
    fn the_resolution_limit_drops_octaves_it_cannot_carry() {
        assert_eq!(resolved_octaves(&LADDER, 0.5), 6);
        assert_eq!(resolved_octaves(&LADDER, 1.0), 5);
        assert_eq!(
            resolved_octaves(&LADDER, 0.25),
            6,
            "the ladder still only asks for 6"
        );
        assert_eq!(resolved_octaves(&LADDER, 64.0), 0);
    }

    /// The three-dimensional field repeats around the ring too, so a gallery running off
    /// one end arrives at the other as the same gallery.
    #[test]
    fn the_three_dimensional_field_repeats_around_the_ring() {
        let circ = 32.0;
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for i in 0..128 {
            let x = i as f64 * 0.25;
            for y in [0.0, 1.25, 4.5] {
                let here = ring_noise_3d(x, y, 1.5, circ, 8, 4);
                let there = ring_noise_3d(x + circ, y, 1.5, circ, 8, 4);
                assert_eq!(here.to_bits(), there.to_bits(), "x {x} y {y}");
                lo = lo.min(here);
                hi = hi.max(here);
            }
        }
        assert!(hi - lo > 0.8, "the field barely moves: {lo:.3}..{hi:.3}");
        assert!(
            lo > -1.4 && hi < 1.4,
            "the field left its range: {lo:.3}..{hi:.3}"
        );
    }

    /// Bounded and not constant: a field that always returns zero would pass every seam
    /// check above.
    #[test]
    fn the_field_varies_and_stays_in_range() {
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for i in 0..2000 {
            let v = fbm(
                i as f64 * 0.017,
                (i % 7) as f64 * 0.4,
                32.0,
                &LADDER,
                0.5,
                3,
            );
            lo = lo.min(v);
            hi = hi.max(v);
        }
        assert!(hi - lo > 0.8, "the field barely moves: {lo:.3}..{hi:.3}");
        assert!(
            lo >= -1.0 && hi <= 1.0,
            "the field left its range: {lo:.3}..{hi:.3}"
        );
    }
}
