//! Static habitat and slow weather, both continuous functions of the embedded position.

use serde::{Deserialize, Serialize};

use cubarium_surface::CELL_COUNT;

use crate::config::{HabitatConfig, WeatherConfig};

/// Per-cell static base light and moisture, computed once from the seed.
#[derive(Clone, Debug, PartialEq)]
pub struct Habitat {
    pub light_base: Box<[f64; CELL_COUNT]>,
    pub moisture_base: Box<[f64; CELL_COUNT]>,
    /// Cell-center unit-cube positions, cached for weather sampling.
    pub positions: Box<[[f64; 3]; CELL_COUNT]>,
}

impl Habitat {
    /// Patch noise `n(p) = (1/k) Σ_i cos(2π p·d_i / λ_i + φ_i)` with `k` waves whose
    /// directions `d_i` (uniform on the sphere), wavelengths `λ_i` (uniform in the configured
    /// range), and phases `φ_i` come from `Stream::Habitat` draws (key = i, counters 0..4).
    /// Moisture uses the same waves evaluated at `p + (0.37, 0.11, 0.23)`.
    /// `L₀ = clamp(light_base + light_height_gain · y + light_noise_gain · n, 0, 1)`,
    /// `W₀ = clamp(moisture_base + moisture_height_gain · y + moisture_noise_gain · n', moisture_min, 1)`.
    pub fn new(cfg: &HabitatConfig, seed: u64) -> Habitat {
        let _ = (cfg, seed);
        todo!("Habitat::new")
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
    /// the config. Light blobs use keys `0..n`, moisture blobs `n..2n`.
    pub fn new(cfg: &WeatherConfig, seed: u64) -> Weather {
        let _ = (cfg, seed);
        todo!("Weather::new")
    }

    /// Advance one tick: rotate each center about its axis by `rate` (when `moving`), and at
    /// each new simulated minute apply a random-walk tilt of `walk_deg_per_min` in a
    /// direction drawn from `Stream::Weather` (key = blob index, counter = minute).
    pub fn advance(&mut self, cfg: &WeatherConfig, seed: u64, tick: u64) {
        let _ = (cfg, seed, tick);
        todo!("Weather::advance")
    }

    /// Fill `light[c] = clamp(L₀ + Σ amp · cap(angle(center, p_c)), 0, 1)` and likewise
    /// moisture with its own minimum, where `cap(θ) = 0.5 (1 + cos(π θ / radius))` for
    /// `θ ≤ radius` else 0.
    pub fn sample(&self, cfg: &WeatherConfig, habitat: &Habitat, light: &mut [f64; CELL_COUNT], moisture: &mut [f64; CELL_COUNT], moisture_min: f64) {
        let _ = (cfg, habitat, light, moisture, moisture_min);
        todo!("Weather::sample")
    }
}
