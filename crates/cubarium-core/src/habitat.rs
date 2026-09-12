//! Static habitat and slow weather, both continuous functions of the embedded position.

use serde::{Deserialize, Serialize};

use cubarium_surface::{CELL_COUNT, CellId};

use crate::config::{HabitatConfig, WeatherConfig};
use crate::rng::{Stream, unit};

/// Simulated ticks in one minute (`TICK_HZ · 60`).
const TICKS_PER_MINUTE: u64 = crate::TICK_HZ as u64 * 60;

/// The counter block reserved for weather initialization, used instead of a real minute.
/// Four counters per blob: center `z`, center azimuth, axis `z`, axis azimuth.
const WEATHER_INIT_COUNTER: u64 = u64::MAX - 8;

/// The offset applied to the sample position for the moisture noise field.
const MOISTURE_NOISE_SHIFT: [f64; 3] = [0.37, 0.11, 0.23];

/// The offset applied to the sample position for the terrain basin noise (`design/water.md`).
const BASIN_NOISE_SHIFT: [f64; 3] = [0.53, 0.29, 0.71];

/// Per-cell static base light and moisture, computed once from the seed.
#[derive(Clone, Debug, PartialEq)]
pub struct Habitat {
    pub light_base: Box<[f64; CELL_COUNT]>,
    pub moisture_base: Box<[f64; CELL_COUNT]>,
    /// Cell-center unit-cube positions, cached for weather sampling.
    pub positions: Box<[[f64; 3]; CELL_COUNT]>,
    /// Terrain height for standing water: `z = h + basin_gain · n_b(p + (0.53, 0.29, 0.71))`
    /// with `h = p.y` (Top = 1, rim = −1). Hollows a fraction of a cell deep, so the level
    /// top face and the bottom row of the sides have places for pools (`design/water.md`).
    pub terrain: Box<[f64; CELL_COUNT]>,
}

/// One cosine wave of the patch noise.
struct Wave {
    /// `2π d / λ`, so the wave contributes `cos(p · scaled_dir + phase)`.
    scaled_dir: [f64; 3],
    phase: f64,
}

impl Habitat {
    /// Patch noise `n(p) = (1/k) Σ_i cos(2π p·d_i / λ_i + φ_i)` with `k` waves whose
    /// directions `d_i` (uniform on the sphere), wavelengths `λ_i` (uniform in the configured
    /// range), and phases `φ_i` come from `Stream::Habitat` draws (key = i, counters 0..4).
    /// Moisture uses the same waves evaluated at `p + (0.37, 0.11, 0.23)`, the terrain
    /// basins at `p + (0.53, 0.29, 0.71)`.
    /// `L₀ = clamp(light_base + light_height_gain · y + light_noise_gain · n, 0, 1)`,
    /// `W₀ = clamp(moisture_base + moisture_height_gain · y + moisture_noise_gain · n', moisture_min, 1)`.
    pub fn new(cfg: &HabitatConfig, seed: u64) -> Habitat {
        let waves: Vec<Wave> = (0..u64::from(cfg.noise_waves))
            .map(|i| {
                let dir = sphere_direction(seed, Stream::Habitat, i, 0);
                let [lo, hi] = cfg.noise_wavelength;
                let lambda = lo + (hi - lo) * unit(seed, Stream::Habitat, i, 2);
                let phase = std::f64::consts::TAU * unit(seed, Stream::Habitat, i, 3);
                let k = std::f64::consts::TAU / lambda;
                Wave { scaled_dir: [dir[0] * k, dir[1] * k, dir[2] * k], phase }
            })
            .collect();

        let noise = |p: [f64; 3]| -> f64 {
            if waves.is_empty() {
                return 0.0;
            }
            let sum: f64 = waves
                .iter()
                .map(|w| {
                    (p[0] * w.scaled_dir[0] + p[1] * w.scaled_dir[1] + p[2] * w.scaled_dir[2]
                        + w.phase)
                        .cos()
                })
                .sum();
            sum / waves.len() as f64
        };

        let mut light_base = Box::new([0.0f64; CELL_COUNT]);
        let mut moisture_base = Box::new([0.0f64; CELL_COUNT]);
        let mut positions = Box::new([[0.0f64; 3]; CELL_COUNT]);
        let mut terrain = Box::new([0.0f64; CELL_COUNT]);
        for cell in CellId::all() {
            let p = cell.center().embed();
            let shifted = [
                p[0] + MOISTURE_NOISE_SHIFT[0],
                p[1] + MOISTURE_NOISE_SHIFT[1],
                p[2] + MOISTURE_NOISE_SHIFT[2],
            ];
            let basin = [
                p[0] + BASIN_NOISE_SHIFT[0],
                p[1] + BASIN_NOISE_SHIFT[1],
                p[2] + BASIN_NOISE_SHIFT[2],
            ];
            let y = p[1];
            let l = cfg.light_base + cfg.light_height_gain * y + cfg.light_noise_gain * noise(p);
            let w = cfg.moisture_base
                + cfg.moisture_height_gain * y
                + cfg.moisture_noise_gain * noise(shifted);
            let i = cell.index();
            light_base[i] = l.clamp(0.0, 1.0);
            moisture_base[i] = w.clamp(cfg.moisture_min, 1.0);
            positions[i] = p;
            // `basin_gain = 0` must give exactly `y`: skip the noise term rather than add 0·n.
            terrain[i] = if cfg.basin_gain != 0.0 { y + cfg.basin_gain * noise(basin) } else { y };
        }
        Habitat { light_base, moisture_base, positions, terrain }
    }
}

/// One moving raised-cosine cap on the unit sphere.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blob {
    /// Unit direction of the cap center.
    pub center: [f64; 3],
    /// Orbit axis (unit) and angular speed in radians per tick; the center rotates about the axis.
    pub axis: [f64; 3],
    pub rate: f64,
}

/// Checkpointed weather state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Weather {
    pub light: Vec<Blob>,
    pub moisture: Vec<Blob>,
    /// Simulated minute of the last random-walk step.
    pub last_walk_minute: u64,
}

impl Weather {
    /// Initial blobs: centers and axes from `Stream::Weather` (key = blob index, counters
    /// 0..6 at "minute" `u64::MAX` are reserved for initialization), periods cycled from
    /// the config. Light blobs use keys `0..n`, moisture blobs `n..2n`. Concretely the
    /// initialization block is `u64::MAX - 8 + k` for `k` in `0..4`: center `z`, center
    /// azimuth, axis `z`, axis azimuth. The axis is made perpendicular to the center by
    /// Gram–Schmidt, and `rate = 2π / (period_min · 60 · TICK_HZ)` radians per tick.
    pub fn new(cfg: &WeatherConfig, seed: u64) -> Weather {
        let n = u64::from(cfg.blobs_per_channel);
        let make = |key: u64, within_channel: usize| -> Blob {
            let center = sphere_direction(seed, Stream::Weather, key, WEATHER_INIT_COUNTER);
            let raw = sphere_direction(seed, Stream::Weather, key, WEATHER_INIT_COUNTER + 2);
            let axis = orthonormalize(raw, center);
            let period_min = if cfg.periods_min.is_empty() {
                0.0
            } else {
                cfg.periods_min[within_channel % cfg.periods_min.len()]
            };
            let ticks = period_min * 60.0 * f64::from(crate::TICK_HZ);
            let rate = if ticks > 0.0 { std::f64::consts::TAU / ticks } else { 0.0 };
            Blob { center, axis, rate }
        };
        Weather {
            light: (0..n).map(|b| make(b, b as usize)).collect(),
            moisture: (0..n).map(|b| make(n + b, b as usize)).collect(),
            last_walk_minute: 0,
        }
    }

    /// Advance one tick: rotate each center about its axis by `rate` (when `moving`), and at
    /// each new simulated minute apply a random-walk tilt of `walk_deg_per_min` in a
    /// direction drawn from `Stream::Weather` (key = blob index, counter = minute).
    pub fn advance(&mut self, cfg: &WeatherConfig, seed: u64, tick: u64) {
        if !cfg.moving {
            // Static mode freezes the blob centers entirely (spec: "Habitat and weather").
            return;
        }
        for blob in self.light.iter_mut().chain(self.moisture.iter_mut()) {
            blob.center = normalize_or(rodrigues(blob.center, blob.axis, blob.rate), blob.center);
        }
        let minute = tick / TICKS_PER_MINUTE;
        if minute > self.last_walk_minute {
            let step = cfg.walk_deg_per_min.to_radians();
            for (i, blob) in self.light.iter_mut().chain(self.moisture.iter_mut()).enumerate() {
                // Keys match `Weather::new`: light blobs 0..n, moisture blobs n..2n.
                let key = i as u64;
                let azimuth = std::f64::consts::TAU * unit(seed, Stream::Weather, key, minute);
                // A perpendicular basis anchored on the orbit axis keeps the walk direction
                // a deterministic function of the drawn azimuth alone.
                let e1 = blob.axis;
                let e2 = cross(blob.center, e1);
                let tangent = [
                    e1[0] * azimuth.cos() + e2[0] * azimuth.sin(),
                    e1[1] * azimuth.cos() + e2[1] * azimuth.sin(),
                    e1[2] * azimuth.cos() + e2[2] * azimuth.sin(),
                ];
                let (c, s) = (step.cos(), step.sin());
                let tilted = [
                    blob.center[0] * c + tangent[0] * s,
                    blob.center[1] * c + tangent[1] * s,
                    blob.center[2] * c + tangent[2] * s,
                ];
                blob.center = normalize_or(tilted, blob.center);
                blob.axis = orthonormalize(blob.axis, blob.center);
            }
            self.last_walk_minute = minute;
        }
    }

    /// Fill `light[c] = clamp(L₀ + Σ amp · cap(angle(center, p_c)), 0, 1)` and likewise
    /// moisture with its own minimum, where `cap(θ) = 0.5 (1 + cos(π θ / radius))` for
    /// `θ ≤ radius` else 0. `rain_source[c]` receives the bare moisture blob sum
    /// `B_c = Σ amp · cap(…)` over the moisture blobs, before the static habitat is added
    /// and before clamping: rain (`design/water.md`) is driven by the moving showers, not
    /// by how wet the ground already is.
    pub fn sample(
        &self,
        cfg: &WeatherConfig,
        habitat: &Habitat,
        light: &mut [f64; CELL_COUNT],
        moisture: &mut [f64; CELL_COUNT],
        rain_source: &mut [f64; CELL_COUNT],
        moisture_min: f64,
    ) {
        let radius = cfg.blob_radius_deg.to_radians();
        for i in 0..CELL_COUNT {
            let dir = normalize_or(habitat.positions[i], [0.0, 1.0, 0.0]);
            let sum = |blobs: &[Blob]| -> f64 {
                blobs.iter().map(|b| cfg.amplitude * cap(dot(b.center, dir), radius)).sum()
            };
            let wet = sum(&self.moisture);
            light[i] = (habitat.light_base[i] + sum(&self.light)).clamp(0.0, 1.0);
            moisture[i] = (habitat.moisture_base[i] + wet).clamp(moisture_min, 1.0);
            rain_source[i] = wet;
        }
    }
}

/// The raised-cosine cap value for a center–sample dot product and an angular radius.
fn cap(dot: f64, radius: f64) -> f64 {
    if radius <= 0.0 {
        return 0.0;
    }
    let theta = dot.clamp(-1.0, 1.0).acos();
    if theta > radius {
        0.0
    } else {
        0.5 * (1.0 + (std::f64::consts::PI * theta / radius).cos())
    }
}

/// A direction uniform on the unit sphere from two draws: `z` uniform in `[-1, 1]` at
/// `counter`, azimuth uniform in `[0, 2π)` at `counter + 1`.
fn sphere_direction(seed: u64, stream: Stream, key: u64, counter: u64) -> [f64; 3] {
    let z = 2.0 * unit(seed, stream, key, counter) - 1.0;
    let azimuth = std::f64::consts::TAU * unit(seed, stream, key, counter.wrapping_add(1));
    let r = (1.0 - z * z).max(0.0).sqrt();
    [r * azimuth.cos(), r * azimuth.sin(), z]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// Normalize, falling back to `fallback` when the vector is degenerate.
fn normalize_or(v: [f64; 3], fallback: [f64; 3]) -> [f64; 3] {
    let len = dot(v, v).sqrt();
    if len > 1e-12 && len.is_finite() { [v[0] / len, v[1] / len, v[2] / len] } else { fallback }
}

/// Gram–Schmidt: the unit component of `v` perpendicular to the unit vector `axis`, or an
/// arbitrary perpendicular when the two are parallel.
fn orthonormalize(v: [f64; 3], axis: [f64; 3]) -> [f64; 3] {
    let d = dot(v, axis);
    let perp = [v[0] - d * axis[0], v[1] - d * axis[1], v[2] - d * axis[2]];
    let len = dot(perp, perp).sqrt();
    if len > 1e-9 {
        [perp[0] / len, perp[1] / len, perp[2] / len]
    } else {
        any_perpendicular(axis)
    }
}

/// Some unit vector perpendicular to the unit vector `a`.
fn any_perpendicular(a: [f64; 3]) -> [f64; 3] {
    let seed = if a[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let p = cross(a, seed);
    normalize_or(p, [1.0, 0.0, 0.0])
}

/// Rotate `v` about the unit `axis` by `angle` (Rodrigues).
fn rodrigues(v: [f64; 3], axis: [f64; 3], angle: f64) -> [f64; 3] {
    let (s, c) = angle.sin_cos();
    let k = cross(axis, v);
    let d = dot(axis, v) * (1.0 - c);
    [v[0] * c + k[0] * s + axis[0] * d, v[1] * c + k[1] * s + axis[1] * d, v[2] * c + k[2] * s + axis[2] * d]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorldConfig;
    use cubarium_surface::FieldGraph;

    fn norm(v: [f64; 3]) -> f64 {
        dot(v, v).sqrt()
    }

    #[test]
    fn habitat_values_stay_inside_their_bounds() {
        let cfg = WorldConfig::default().habitat;
        for seed in [1u64, 2, 7, 12345] {
            let h = Habitat::new(&cfg, seed);
            for i in 0..CELL_COUNT {
                assert!((0.0..=1.0).contains(&h.light_base[i]), "light {}", h.light_base[i]);
                assert!(
                    h.moisture_base[i] >= cfg.moisture_min && h.moisture_base[i] <= 1.0,
                    "moisture {}",
                    h.moisture_base[i]
                );
                // Cell centers lie on the cube surface: one coordinate is ±1.
                let p = h.positions[i];
                assert!(p.iter().any(|c| (c.abs() - 1.0).abs() < 1e-12), "position {p:?}");
            }
        }
    }

    #[test]
    fn habitat_is_deterministic_and_seed_dependent() {
        let cfg = WorldConfig::default().habitat;
        assert_eq!(Habitat::new(&cfg, 4), Habitat::new(&cfg, 4));
        assert_ne!(Habitat::new(&cfg, 4).light_base, Habitat::new(&cfg, 5).light_base);
    }

    /// Across each of the eight seams a neighboring pair of cells must not be more
    /// different than neighboring pairs inside a face: the habitat is a function of the
    /// embedded position, so seams are invisible to it.
    #[test]
    fn habitat_is_continuous_across_every_seam() {
        let cfg = WorldConfig::default().habitat;
        let graph = FieldGraph::new();
        for seed in [1u64, 2, 3, 99] {
            let h = Habitat::new(&cfg, seed);
            let mut worst_seam = 0.0f64;
            let mut worst_inner = 0.0f64;
            let mut seams_seen = std::collections::HashSet::new();
            for &(a, b) in graph.edges() {
                let dl = (h.light_base[a.index()] - h.light_base[b.index()]).abs();
                let dw = (h.moisture_base[a.index()] - h.moisture_base[b.index()]).abs();
                let d = dl.max(dw);
                if a.face() == b.face() {
                    worst_inner = worst_inner.max(d);
                } else {
                    let mut pair = [a.face() as u8, b.face() as u8];
                    pair.sort_unstable();
                    seams_seen.insert(pair);
                    worst_seam = worst_seam.max(d);
                }
            }
            assert_eq!(seams_seen.len(), 8, "expected all eight seams, got {seams_seen:?}");
            println!(
                "seed {seed}: max seam-neighbor jump {worst_seam:.6}, max in-face jump {worst_inner:.6}"
            );
            assert!(
                worst_seam <= worst_inner,
                "seed {seed}: seam jump {worst_seam} exceeds in-face jump {worst_inner}"
            );
        }
    }

    #[test]
    fn weather_blobs_start_unit_and_orthogonal() {
        let cfg = WorldConfig::default().weather;
        let w = Weather::new(&cfg, 3);
        assert_eq!(w.light.len(), cfg.blobs_per_channel as usize);
        assert_eq!(w.moisture.len(), cfg.blobs_per_channel as usize);
        for (i, b) in w.light.iter().chain(w.moisture.iter()).enumerate() {
            assert!((norm(b.center) - 1.0).abs() < 1e-12, "blob {i} center {:?}", b.center);
            assert!((norm(b.axis) - 1.0).abs() < 1e-12, "blob {i} axis {:?}", b.axis);
            assert!(dot(b.center, b.axis).abs() < 1e-12, "blob {i} not orthogonal");
        }
        // Light and moisture blobs use different keys, so they differ.
        assert_ne!(w.light[0].center, w.moisture[0].center);
        // Periods are cycled per channel.
        let want = std::f64::consts::TAU / (20.0 * 60.0 * 20.0);
        assert!((w.light[0].rate - want).abs() < 1e-18);
        assert_eq!(w.light[0].rate, w.moisture[0].rate);
    }

    #[test]
    fn static_weather_never_moves() {
        let mut cfg = WorldConfig::default().weather;
        cfg.moving = false;
        let start = Weather::new(&cfg, 11);
        let mut w = start.clone();
        for tick in 0..5000 {
            w.advance(&cfg, 11, tick);
        }
        assert_eq!(w, start);
    }

    #[test]
    fn moving_weather_keeps_unit_centers_and_returns_after_one_period() {
        let mut cfg = WorldConfig::default().weather;
        cfg.walk_deg_per_min = 0.0;
        let start = Weather::new(&cfg, 21);
        let mut w = start.clone();
        // The first blob's period is 20 minutes = 24_000 ticks.
        let period_ticks = 20 * 60 * 20;
        let mut moved = 0.0f64;
        for tick in 1..=period_ticks {
            w.advance(&cfg, 21, tick);
            for b in w.light.iter().chain(w.moisture.iter()) {
                assert!((norm(b.center) - 1.0).abs() < 1e-9);
            }
            if tick == period_ticks / 4 {
                moved = (0..3)
                    .map(|k| (w.light[0].center[k] - start.light[0].center[k]).abs())
                    .fold(0.0, f64::max);
            }
        }
        assert!(moved > 0.1, "the center should have travelled by a quarter period: {moved}");
        for k in 0..3 {
            assert!(
                (w.light[0].center[k] - start.light[0].center[k]).abs() < 1e-6,
                "period mismatch on axis {k}: {:?} vs {:?}",
                w.light[0].center,
                start.light[0].center
            );
        }
    }

    #[test]
    fn the_random_walk_moves_centers_once_per_minute() {
        let cfg = WorldConfig::default().weather;
        let mut still = WeatherConfig { walk_deg_per_min: 0.0, ..cfg.clone() };
        still.walk_deg_per_min = 0.0;
        let mut walked = Weather::new(&cfg, 31);
        let mut straight = Weather::new(&still, 31);
        for tick in 1..=(3 * TICKS_PER_MINUTE) {
            walked.advance(&cfg, 31, tick);
            straight.advance(&still, 31, tick);
        }
        assert_eq!(walked.last_walk_minute, 3);
        assert_eq!(straight.last_walk_minute, 3);
        let angle = dot(walked.light[0].center, straight.light[0].center).clamp(-1.0, 1.0).acos();
        // Three minutes of 2° tilts, each in its own direction: within 0 and 6 degrees.
        assert!(angle > 1e-3 && angle <= 6.0f64.to_radians() + 1e-9, "walk angle {angle}");
        for b in walked.light.iter().chain(walked.moisture.iter()) {
            assert!((norm(b.center) - 1.0).abs() < 1e-12);
            assert!(dot(b.center, b.axis).abs() < 1e-9, "axis lost orthogonality");
        }
    }

    #[test]
    fn sample_is_bounded_and_reduces_to_the_base_without_amplitude() {
        let world = WorldConfig::default();
        let habitat = Habitat::new(&world.habitat, 5);
        let weather = Weather::new(&world.weather, 5);
        let mut light = Box::new([0.0f64; CELL_COUNT]);
        let mut moisture = Box::new([0.0f64; CELL_COUNT]);
        let mut source = Box::new([0.0f64; CELL_COUNT]);

        let flat = WeatherConfig { amplitude: 0.0, ..world.weather.clone() };
        weather.sample(&flat, &habitat, &mut light, &mut moisture, &mut source, world.habitat.moisture_min);
        assert_eq!(&light[..], &habitat.light_base[..]);
        assert_eq!(&moisture[..], &habitat.moisture_base[..]);
        assert!(source.iter().all(|&b| b == 0.0), "no amplitude, no rain source");

        weather.sample(
            &world.weather,
            &habitat,
            &mut light,
            &mut moisture,
            &mut source,
            world.habitat.moisture_min,
        );
        // The rain source is the bare blob sum: nonnegative, never above the sum of the
        // amplitudes, and equal to the lift of moisture wherever the clamp did not bite.
        let ceiling = world.weather.amplitude * f64::from(world.weather.blobs_per_channel);
        for i in 0..CELL_COUNT {
            assert!(source[i] >= 0.0 && source[i] <= ceiling + 1e-12, "source {}", source[i]);
            let lifted = habitat.moisture_base[i] + source[i];
            if lifted <= 1.0 && lifted >= world.habitat.moisture_min {
                assert!((moisture[i] - lifted).abs() < 1e-12);
            }
        }
        let mut lifted = 0;
        for i in 0..CELL_COUNT {
            assert!((0.0..=1.0).contains(&light[i]), "light {}", light[i]);
            assert!(
                moisture[i] >= world.habitat.moisture_min && moisture[i] <= 1.0,
                "moisture {}",
                moisture[i]
            );
            if light[i] > habitat.light_base[i] + 1e-12 {
                lifted += 1;
            }
        }
        assert!(lifted > 0, "weather blobs should raise light somewhere");
    }

    #[test]
    fn a_cap_centered_on_the_cell_is_the_full_amplitude() {
        assert_eq!(cap(1.0, 1.0), 1.0);
        assert!(cap(0.0_f64.cos(), 1.0) == 1.0);
        // Exactly on the radius the cap is zero, and beyond it stays zero.
        let r = 55.0f64.to_radians();
        assert!(cap(r.cos(), r).abs() < 1e-15);
        assert_eq!(cap((r + 0.1).cos(), r), 0.0);
        // Half way out, the raised cosine is one half.
        assert!((cap((r / 2.0).cos(), r) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn terrain_is_the_height_with_shallow_basins() {
        use cube_proto::Face;
        let cfg = WorldConfig::default().habitat;
        let h = Habitat::new(&cfg, 9);
        assert!(cfg.basin_gain > 0.0, "the default has basins");
        let mut hollows = 0;
        for cell in CellId::all() {
            let i = cell.index();
            let y = h.positions[i][1];
            let z = h.terrain[i];
            assert!((z - y).abs() <= cfg.basin_gain + 1e-12, "terrain {z} vs y {y}");
            if cell.face() == Face::Top {
                assert!((z - 1.0).abs() <= cfg.basin_gain + 1e-12);
                if z < 1.0 {
                    hollows += 1;
                }
            }
        }
        assert!(hollows > 0, "the level top face must have places for pools");
        // Down a side face the terrain falls on average: rows are 0.125 apart and the
        // basin noise is at most 0.06 either way.
        for face in [Face::Front, Face::Right, Face::Back, Face::Left] {
            let row_mean = |cy: u8| -> f64 {
                (0..16u8).map(|cx| h.terrain[CellId::new(face, cx, cy).index()]).sum::<f64>() / 16.0
            };
            for cy in 0..15u8 {
                assert!(row_mean(cy) > row_mean(cy + 1), "{face:?} row {cy} does not fall");
            }
        }
        // No basins: the terrain is the embedded height exactly.
        let flat = Habitat::new(&HabitatConfig { basin_gain: 0.0, ..cfg }, 9);
        for i in 0..CELL_COUNT {
            assert_eq!(flat.terrain[i], flat.positions[i][1]);
        }
    }
}
