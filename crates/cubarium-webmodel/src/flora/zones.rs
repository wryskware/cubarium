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
    /// The same readings on the faces this species **holds** (its stands sit where their
    /// own gates passed, a microhabitat a zone mean dilutes): `μ`, aeration target and
    /// available water, means over its stands in the zone; the zone's own when it holds
    /// fewer than [`HELD_MIN`] there.
    pub mu_held: f64,
    pub stress_held: f64,
    pub available_held: f64,
}

/// One face where a species' newborn could come up in one weather state (its water and
/// terrain-light gates pass, and no stand but a ground cover holds it): what the canopy gate
/// (`step::canopy_pays`, `step.rs:3899`) reads there besides the site mineral.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    /// Terrain sky.
    pub sky: f32,
    /// `μ` of the root box.
    pub mu: f32,
    /// Optical depth over the newborn's crown top, per shading species ([`Species::ALL`]).
    pub tau: [f32; Species::COUNT],
    /// Whether the species' water and terrain-light gates pass here in this state.
    pub gate: bool,
    /// The face's site mineral over the mean of its occupant's faces in the zone then
    /// (bare faces against bare): seeds fall near their parents, on ground those parents
    /// and their neighbours have drained.
    pub n_rel: f32,
    /// Its occupant then ([`Species::index`], `Species::COUNT` bare).
    pub occ: u8,
    /// Root-box available water.
    pub avail: f32,
}

/// The most [`Sample`]s kept per zone, weather state and species (an even stride over the
/// faces that qualify).
pub const SAMPLES_MAX: usize = 48;

/// The fewest stands whose faces stand for a species' own reading in a zone.
pub const HELD_MIN: f64 = 5.0;

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
    /// Per weather state, per photo species ([`PHOTO`] order): the newborn's faces (gates
    /// passed, open: bare or a ground cover).
    pub samples: Vec<Vec<Vec<Sample>>>,
    /// The same over the open faces **holding a seed** of the species (gates passed or
    /// not): seeds fall near their parents, so the seeded faces carry the parents' shade
    /// and their own water.
    pub seeded: Vec<Vec<Vec<Sample>>>,
    /// The same over the open faces **in a runner's reach of a donor** of the species
    /// (`runner_reach` columns, within a voxel of its height, `seeds.rs:1078`).
    pub runner: Vec<Vec<Vec<Sample>>>,
    /// Per weather state and species ([`Species::ALL`]): the faces its seed may take (bare,
    /// or a cover it comes up through), those holding at least one whole seed, and the whole
    /// seeds banked over the whole zone (the clumping fit's data, [`super::fit::clumping`]).
    pub seed_stats: Vec<[[f64; 3]; Species::COUNT]>,
    /// Per clonal species ([`Species::ALL`]) and occupant (`Species::COUNT` bare): how much
    /// more often a face in a donor's runner box is held by that occupant than a face of
    /// the zone at large (box share over zone share, pooled over the snapshots; 1 with no
    /// data). Runners crawl within a clump: a turf's box is mostly turf.
    pub box_enrich: [[f64; Species::COUNT + 1]; Species::COUNT],
    /// Per photo receiver ([`PHOTO`] order) and shading species ([`Species::ALL`]): the
    /// zone's mean optical depth over a newborn of the receiver per unit of the shader's
    /// foliage per face (ratio of sums over the snapshots). New foliage of a shader spreads
    /// its shade over the zone at this rate ([`super::model::Site`]).
    pub kappa: [[f64; Species::COUNT]; 8],
    /// Per weather state: the zone's foliage per species ([`Species::ALL`]) when it was
    /// read, which the samples' optical depths belong to.
    pub foliage_ref: Vec<[f64; Species::COUNT]>,
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
                o.mu_held += r.mu_held / n;
                o.stress_held += r.stress_held / n;
                o.available_held += r.available_held / n;
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
            samples: vec![vec![Vec::new(); PHOTO.len()]; n],
            seeded: vec![vec![Vec::new(); PHOTO.len()]; n],
            runner: vec![vec![Vec::new(); PHOTO.len()]; n],
            seed_stats: vec![[[0.0; 3]; Species::COUNT]; n],
            box_enrich: [[1.0; Species::COUNT + 1]; Species::COUNT],
            kappa: [[0.0; Species::COUNT]; 8],
            foliage_ref: vec![[0.0; Species::COUNT]; n],
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
    let mut held_n: Vec<Vec<[f64; Species::COUNT]>> = vec![vec![[0.0; Species::COUNT]; n]; zones.len()];
    let empty = || vec![vec![vec![Vec::new(); PHOTO.len()]; n]; zones.len()];
    // [zone][species][occupant]: box faces seen, and zone faces (species-independent).
    let mut box_n = vec![[[0.0f64; Species::COUNT + 1]; Species::COUNT]; zones.len()];
    let mut zone_n = vec![[0.0f64; Species::COUNT + 1]; zones.len()];
    // [zone][receiver][shader]: Σ τ over faces, over the snapshots.
    let mut tau_sum = vec![[[0.0f64; Species::COUNT]; 8]; zones.len()];
    let (mut qualifying, mut seeded, mut runner): (Vec<Vec<Vec<Vec<Sample>>>>, Vec<Vec<Vec<Vec<Sample>>>>, Vec<Vec<Vec<Vec<Sample>>>>) = (empty(), empty(), empty());
    for (si, snap) in snapshots.iter().enumerate() {
        // Donor columns per species: (x, z) → heights, for the runner reach.
        let mut donors: HashMap<(i64, i64), Vec<(usize, u32)>> = HashMap::new();
        for f in snap {
            if let Some((p, r)) = f.plant
                && r.wood >= config.species(p).donor_min
            {
                donors.entry((i64::from(f.x), i64::from(f.z))).or_default().push((p.index(), f.y));
            }
        }
        let width = snap.iter().map(|f| i64::from(f.x)).max().unwrap_or(0) + 1;
        {
            let mut at: HashMap<(i64, i64), Vec<&Face>> = HashMap::new();
            for f in snap {
                at.entry((i64::from(f.x), i64::from(f.z))).or_default().push(f);
                let z = zi[&keys[&(f.x, f.y, f.z)]];
                zone_n[z][f.plant.map_or(Species::COUNT, |(p, _)| p.index())] += 1.0;
            }
            for f in snap {
                let Some((p, r)) = f.plant else { continue };
                let sc = config.species(p);
                if sc.clonal_share <= 0.0 || r.wood < sc.donor_min {
                    continue;
                }
                let z = zi[&keys[&(f.x, f.y, f.z)]];
                let reach = i64::from(sc.runner_reach);
                for dx in -reach..=reach {
                    for dz in -reach..=reach {
                        if dx == 0 && dz == 0 {
                            continue;
                        }
                        let Some(v) = at.get(&((i64::from(f.x) + dx).rem_euclid(width), i64::from(f.z) + dz)) else { continue };
                        if let Some(g) = v.iter().filter(|g| (i64::from(g.y) - i64::from(f.y)).abs() <= 1).min_by_key(|g| (i64::from(g.y) - i64::from(f.y)).abs()) {
                            box_n[z][p.index()][g.plant.map_or(Species::COUNT, |(q, _)| q.index())] += 1.0;
                        }
                    }
                }
            }
        }
        // Mean site mineral per zone and occupant (bare last).
        let mut occ_min = vec![[[0.0f64; 2]; Species::COUNT + 1]; zones.len()];
        for f in snap {
            let z = zi[&keys[&(f.x, f.y, f.z)]];
            let o = f.plant.map_or(Species::COUNT, |(p, _)| p.index());
            occ_min[z][o][0] += f.mineral;
            occ_min[z][o][1] += 1.0;
        }
        for f in snap {
            let z = zi[&keys[&(f.x, f.y, f.z)]];
            if let Some((p, r)) = f.plant {
                zones[z].foliage_ref[si][p.index()] += r.foliage;
            }
            for k in 0..8 {
                for j in 0..Species::COUNT {
                    tau_sum[z][k][j] += f64::from(f.nb_tau[k][j]);
                }
            }
            let open = f.plant.is_none_or(|(p, _)| config.species(p).ground_cover);
            if open {
                for sp in Species::ALL {
                    // The faces this species' seed may take (bare, or a cover it comes up
                    // through), as the model's seeded share is applied to.
                    let sc = config.species(sp);
                    let may = f.plant.is_none_or(|(c, _)| c != sp && sc.overtop_per_check > 0.0 && (sc.overtop_covers.is_empty() || sc.overtop_covers.contains(&c)));
                    if !may {
                        continue;
                    }
                    let st = &mut zones[z].seed_stats[si][sp.index()];
                    st[0] += 1.0;
                    st[1] += f64::from(u8::from(f.seeds[sp.index()] > 0));
                }
            }
            for sp in Species::ALL {
                zones[z].seed_stats[si][sp.index()][2] += f64::from(f.seeds[sp.index()]);
            }
            for (k, sp) in PHOTO.iter().enumerate() {
                if !open {
                    continue;
                }
                let sc = config.species(*sp);
                // Only faces this species may take: bare, or a cover its seed may come up
                // through (`seeds::may_overtop`) or its runner root through
                // (`seeds::runner_may_take`); never its own kind's.
                let cover = f.plant.map(|(p, _)| p);
                let allowed = |runner: bool| match cover {
                    None => true,
                    Some(c) => {
                        c != *sp
                            && (sc.overtop_covers.is_empty() || sc.overtop_covers.contains(&c))
                            && if runner { sc.runners_overtop } else { sc.overtop_per_check > 0.0 }
                    }
                };
                let gate = f.water_ok[k] && f.light_ok[k];
                let mut tau = [0.0f32; Species::COUNT];
                tau.copy_from_slice(&f.nb_tau[k]);
                let o = f.plant.map_or(Species::COUNT, |(p, _)| p.index());
                let mean = occ_min[z][o][0] / occ_min[z][o][1].max(1.0);
                let n_rel = if mean > 0.0 { (f.mineral / mean) as f32 } else { 1.0 };
                let smp = Sample { sky: f.sky as f32, mu: sc.moisture_at(f.water[k].unwrap_or(0.0)) as f32, tau, gate, n_rel, occ: o as u8, avail: f.water[k].unwrap_or(0.0) as f32 };
                if gate && allowed(false) {
                    qualifying[z][si][k].push(smp);
                }
                if f.seeds[sp.index()] > 0 && allowed(false) {
                    seeded[z][si][k].push(smp);
                }
                if sc.clonal_share > 0.0 && allowed(true) {
                    let reach = i64::from(sc.runner_reach);
                    let near = (-reach..=reach).any(|dx| {
                        (-reach..=reach).any(|dz| {
                            donors.get(&((i64::from(f.x) + dx).rem_euclid(width), i64::from(f.z) + dz)).is_some_and(|v| {
                                v.iter().any(|&(p, y)| p == sp.index() && (i64::from(y) - i64::from(f.y)).abs() <= 1 && !(dx == 0 && dz == 0 && y == f.y))
                            })
                        })
                    });
                    if near {
                        runner[z][si][k].push(smp);
                    }
                }
            }
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
                if f.plant.is_some_and(|(p, _)| p == *sp) {
                    r.mu_held += sc.moisture_at(a);
                    r.stress_held += aeration_target(f.saturated[k], sc);
                    r.available_held += a;
                    held_n[z][si][sp.index()] += 1.0;
                }
            }
            // The glowcap reads its own box through its gates; the census keeps its pass,
            // and its moisture through the cave damp is at least `dark_damp · darkness`.
            let r = &mut w[glow_index()];
            let a = f.turf_water.unwrap_or(0.0).max(gsc.dark_damp * f.darkness);
            r.mu += gsc.moisture_at(a);
            r.available += a;
            r.gate += f64::from(u8::from(f.glow_ok));
            if f.fungus.is_some() {
                r.mu_held += gsc.moisture_at(a);
                r.available_held += a;
                held_n[z][si][glow_index()] += 1.0;
            }
        }
    }
    for (zi, zone) in zones.iter_mut().enumerate() {
        for k in 0..8 {
            for j in 0..Species::COUNT {
                let d: f64 = zone.foliage_ref.iter().map(|f| f[j]).sum::<f64>();
                if d > 1e-9 {
                    // Σ_snapshots mean τ / Σ_snapshots foliage per face = Σ τ / Σ foliage.
                    zone.kappa[k][j] = tau_sum[zi][k][j] / d;
                }
            }
        }
        let zt: f64 = zone_n[zi].iter().sum();
        for sp in 0..Species::COUNT {
            let bt: f64 = box_n[zi][sp].iter().sum();
            if bt < 20.0 {
                continue;
            }
            for o in 0..=Species::COUNT {
                let zs = zone_n[zi][o] / zt.max(1.0);
                if zs > 0.0 {
                    zone.box_enrich[sp][o] = (box_n[zi][sp][o] / bt) / zs;
                }
            }
        }
        let thin = |q: &Vec<Sample>| -> Vec<Sample> {
            let stride = q.len().div_ceil(SAMPLES_MAX).max(1);
            q.iter().step_by(stride).copied().collect()
        };
        for si in 0..n {
            for k in 0..PHOTO.len() {
                zone.samples[si][k] = thin(&qualifying[zi][si][k]);
                zone.seeded[si][k] = thin(&seeded[zi][si][k]);
                zone.runner[si][k] = thin(&runner[zi][si][k]);
            }
        }
        let a = zone.area.max(1.0);
        zone.sky /= a;
        zone.darkness /= a;
        zone.fungal_space /= a;
        for (si, w) in zone.weather.iter_mut().enumerate() {
            for (k, r) in w.iter_mut().enumerate() {
                r.mu /= a;
                r.available /= a;
                r.saturated /= a;
                r.stress_target /= a;
                r.gate /= a;
                let h = held_n[zi][si][k];
                if h >= HELD_MIN {
                    r.mu_held /= h;
                    r.stress_held /= h;
                    r.available_held /= h;
                } else {
                    r.mu_held = r.mu;
                    r.stress_held = r.stress_target;
                    r.available_held = r.available;
                }
            }
        }
    }
    Zones { zones, of_face }
}
