//! **Habitat zones**: the classes the sim's habitat rules actually distinguish, built from
//! face censuses ([`super::census`]) of one world.
//!
//! A face's zone is fixed by what does not move: its material, whether it is its column's
//! highest face (seeds land only there, `seeds.rs:1299`), its terrain sky in bands cut at
//! the species' light gates (`establish_light_min`: 0.05 pad, 0.35 lanternberry, 0.6
//! bloomcrown, 0.75 springturf, 0.9 vaulttree) and its water class — **wet** when
//! springturf's aeration or flooding gate fails in at least half the snapshots, which is
//! where the soak's `wet` cohort sits. Moisture is not fixed per face: every snapshot of
//! the world is one **weather state**, and a zone's state per species is the mean over
//! its faces of the species' own readings (`μ` ramp, aeration target, gate pass, cure
//! dryness) in that snapshot.
//!
//! **Aggregation.** A zone is mean field: the faces inside it are exchangeable, so the
//! spatial spread of light, water and mineral inside a zone is lost (the cohort's light
//! class and the per-occupant mineral pools carry the two that matter most).

use std::collections::HashMap;

use cubarium_voxel_flora::{FloraConfig, Species};

use super::census::{Face, PHOTO};
use super::stand::aeration_target;

/// Sky bands, cut at the species' light gates.
pub const BANDS: [(f64, &str); 6] = [(0.05, "dark"), (0.35, "dim"), (0.6, "mid"), (0.75, "bright"), (0.9, "open"), (f64::INFINITY, "summit")];

/// A face's zone key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key {
    pub soil: bool,
    pub under_roof: bool,
    pub band: u8,
    pub wet: bool,
}

impl Key {
    pub fn name(&self) -> String {
        if self.under_roof {
            return format!("{}roof-{}{}", if self.soil { "" } else { "rock-" }, BANDS[self.band as usize].1, if self.wet { "-wet" } else { "" });
        }
        format!("{}{}-{}", if self.soil { "" } else { "rock-" }, BANDS[self.band as usize].1, if self.wet { "wet" } else { "drained" })
    }
}

fn band(sky: f64) -> u8 {
    BANDS.iter().position(|&(hi, _)| sky < hi).unwrap_or(BANDS.len() - 1) as u8
}

/// One species' reading of a zone in one weather state: means over the zone's faces.
#[derive(Clone, Copy, Debug, Default)]
pub struct Reading {
    /// Mean `μ` (the species' ramp of each face's root-box available water).
    pub mu: f64,
    /// Mean root-box available water.
    pub available: f64,
    /// Mean saturated fraction (what aeration stress closes on), and the mean level it asks.
    pub saturated: f64,
    pub stress_target: f64,
    /// Share of faces whose water and terrain-light gates pass.
    pub gate: f64,
}

/// One zone.
#[derive(Clone, Debug)]
pub struct Zone {
    pub key: Key,
    pub area: f64,
    /// Mean terrain sky.
    pub sky: f64,
    /// The glowcap's mean darkness, and the share of faces a fungus can hold (held one, or
    /// passed its gates, in any snapshot).
    pub darkness: f64,
    pub fungal_space: f64,
    /// Per weather state (snapshot), per species ([`Species::ALL`] order).
    pub weather: Vec<[Reading; Species::COUNT]>,
}

impl Zone {
    /// The time mean over the weather states: the **frozen water** reading.
    pub fn frozen(&self) -> [Reading; Species::COUNT] {
        let n = self.weather.len().max(1) as f64;
        let mut out = [Reading::default(); Species::COUNT];
        for w in &self.weather {
            for (o, r) in out.iter_mut().zip(w.iter()) {
                o.mu += r.mu / n;
                o.available += r.available / n;
                o.saturated += r.saturated / n;
                o.stress_target += r.stress_target / n;
                o.gate += r.gate / n;
            }
        }
        out
    }
}

/// The zones of one world, and each face's zone.
#[derive(Clone, Debug)]
pub struct Zones {
    pub zones: Vec<Zone>,
    pub of_face: HashMap<(u32, u32, u32), usize>,
}

fn glow_index() -> usize {
    Species::Glowcap.index()
}

/// Build the zones from `snapshots` (censuses of one world, any order), under `config`.
pub fn build(config: &FloraConfig, snapshots: &[Vec<Face>]) -> Zones {
    let first = &snapshots[0];
    let n = snapshots.len();
    // Each face's key, with the wet vote over the snapshots.
    let mut index: HashMap<(u32, u32, u32), Vec<&Face>> = HashMap::new();
    for snap in snapshots {
        for f in snap {
            index.entry((f.x, f.y, f.z)).or_default().push(f);
        }
    }
    let turf = PHOTO.iter().position(|&s| s == Species::Springturf).unwrap();
    let tsc = config.species(Species::Springturf);
    let mut keys: HashMap<(u32, u32, u32), Key> = HashMap::new();
    for f in first {
        let all = &index[&(f.x, f.y, f.z)];
        let wet = all.iter().filter(|g| g.saturated[turf] > tsc.establish_saturated_max || g.depth_m > tsc.drown_depth_m).count();
        keys.insert(
            (f.x, f.y, f.z),
            Key {
                soil: f.soil,
                under_roof: !f.highest,
                band: if f.highest { band(f.sky) } else { band(f.sky).min(1) },
                wet: 2 * wet >= all.len(),
            },
        );
    }
    let mut order: Vec<Key> = keys.values().copied().collect();
    order.sort();
    order.dedup();
    let zi: HashMap<Key, usize> = order.iter().enumerate().map(|(i, k)| (*k, i)).collect();
    let mut zones: Vec<Zone> = order
        .iter()
        .map(|&key| Zone {
            key,
            area: 0.0,
            sky: 0.0,
            darkness: 0.0,
            fungal_space: 0.0,
            weather: vec![[Reading::default(); Species::COUNT]; n],
        })
        .collect();
    let mut of_face = HashMap::new();
    let gsc = config.species(Species::Glowcap);
    for f in first {
        let z = zi[&keys[&(f.x, f.y, f.z)]];
        of_face.insert((f.x, f.y, f.z), z);
        let zone = &mut zones[z];
        zone.area += 1.0;
        zone.sky += f.sky;
        zone.darkness += f.darkness;
        let all = &index[&(f.x, f.y, f.z)];
        if all.iter().any(|g| g.fungus.is_some() || g.glow_ok) {
            zone.fungal_space += 1.0;
        }
    }
    for (si, snap) in snapshots.iter().enumerate() {
        for f in snap {
            let z = zi[&keys[&(f.x, f.y, f.z)]];
            let w = &mut zones[z].weather[si];
            for (k, sp) in PHOTO.iter().enumerate() {
                let sc = config.species(*sp);
                let a = f.water[k].unwrap_or(0.0);
                let r = &mut w[sp.index()];
                r.mu += sc.moisture_at(a);
                r.available += a;
                r.saturated += f.saturated[k];
                r.stress_target += aeration_target(f.saturated[k], sc);
                r.gate += f64::from(u8::from(f.water_ok[k] && f.light_ok[k]));
            }
            // The glowcap reads its own box through its gates; the census keeps its pass,
            // and its moisture through the cave damp is at least `dark_damp · darkness`.
            let r = &mut w[glow_index()];
            let a = f.turf_water.unwrap_or(0.0).max(gsc.dark_damp * f.darkness);
            r.mu += gsc.moisture_at(a);
            r.available += a;
            r.gate += f64::from(u8::from(f.glow_ok));
        }
    }
    for zone in &mut zones {
        let a = zone.area.max(1.0);
        zone.sky /= a;
        zone.darkness /= a;
        zone.fungal_space /= a;
        for w in &mut zone.weather {
            for r in w.iter_mut() {
                r.mu /= a;
                r.available /= a;
                r.saturated /= a;
                r.stress_target /= a;
                r.gate /= a;
            }
        }
    }
    Zones { zones, of_face }
}
