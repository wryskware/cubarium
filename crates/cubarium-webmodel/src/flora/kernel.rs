//! **Where seeds land**, zone to zone: each species' dispersal kernel (`seeds::landing`,
//! `seeds.rs:1168`) sampled over the world's own faces, so the share of a zone's seed rain
//! that reaches each zone is geometry and not a fit.
//!
//! The kernels are restated from `seeds.rs`: Drop (`drop_target`, :1299) uniform over the
//! columns within `hop`; Wind (`wind_offset`, :1337) a Lomax distance at scale
//! `wind_scale_hops · hop`, shape `wind_shape`; Wald (`wald_offset`, :1284) the inverse
//! Gaussian; Spores (`spore_target`, :1418) up to `spore_tries` columns within `hop`,
//! landing where the face's ground gates pass, else lost as litter; the glowcap's dark
//! spores (`dark_spore_target`, :1464) weigh a face `1 + bias · darkness` and a roofed face
//! `bias · darkness`; Water (siphonreed) is sampled as a Drop. Every landing is on a
//! column's highest support face, `x` wrapped, `z` reflected (`reflect`, :1353).
//!
//! **Aggregation.** Donors are drawn uniformly over a zone's faces, so the matrix knows
//! the zone's geometry but not where in it the donors stand: clumping of donors near their
//! own kind is not in it.

use std::collections::HashMap;

use cubarium_voxel_flora::{Dispersal, FloraConfig, Species};

use super::census::{Face, PHOTO};
use super::zones::Zones;

/// splitmix64.
#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.unit() * n as f64) as usize % n.max(1)
    }
    pub fn normal(&mut self) -> f64 {
        let u1 = self.unit().max(1e-300);
        let u2 = self.unit();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

/// The landing matrix of one species: `land[z][z']` is the share of zone `z`'s packages
/// banked in `z'`; `lost[z]` the share that falls as litter in `z`.
#[derive(Clone, Debug)]
pub struct Kernel {
    pub land: Vec<Vec<f64>>,
    pub lost: Vec<f64>,
}

/// A runner's neighbourhood: the mean count of candidate faces (within `runner_reach`
/// columns, within a voxel of the parent's height) per zone, and the share in the donor's
/// own zone.
#[derive(Clone, Copy, Debug, Default)]
pub struct RunnerBox {
    pub faces: f64,
    pub same_zone: f64,
}

struct Geo<'a> {
    width: i64,
    depth: i64,
    /// Column → its faces (indices into `faces`), highest first.
    columns: HashMap<(i64, i64), Vec<usize>>,
    faces: &'a [Face],
    zone: Vec<usize>,
    /// Per face, per species ([`Species::ALL`] order): the share of snapshots its ground
    /// gates passed (every gate but light), what a spore's landing reads.
    ground_pass: Vec<[f64; Species::COUNT]>,
}

impl Geo<'_> {
    fn highest(&self, x: i64, z: i64) -> Option<usize> {
        let x = x.rem_euclid(self.width);
        self.columns.get(&(x, z)).and_then(|v| v.first().copied())
    }
}

fn reflect(z: i64, depth: i64) -> i64 {
    if depth <= 1 {
        return 0;
    }
    let period = 2 * (depth - 1);
    let m = z.rem_euclid(period);
    if m < depth { m } else { period - m }
}

fn wald(rng: &mut Rng, mean: f64, shape: f64) -> f64 {
    if !(mean > 0.0 && shape > 0.0) {
        return 0.0;
    }
    let nu = rng.normal();
    let y = nu * nu;
    let mu = mean;
    let x = mu + mu * mu * y / (2.0 * shape) - mu / (2.0 * shape) * (4.0 * mu * shape * y + mu * mu * y * y).sqrt();
    if rng.unit() <= mu / (mu + x) { x } else { mu * mu / x }
}

/// Build every species' landing matrix and runner box from `snapshots` of one world.
pub fn build(config: &FloraConfig, width: u32, depth: u32, zones: &Zones, snapshots: &[Vec<Face>], samples: usize, seed: u64) -> (Vec<Kernel>, Vec<RunnerBox>) {
    let faces = &snapshots[0];
    let mut columns: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, f) in faces.iter().enumerate() {
        columns.entry((i64::from(f.x), i64::from(f.z))).or_default().push(i);
    }
    for v in columns.values_mut() {
        v.sort_by_key(|&i| std::cmp::Reverse(faces[i].y));
    }
    let pos: HashMap<(u32, u32, u32), usize> = faces.iter().enumerate().map(|(i, f)| ((f.x, f.y, f.z), i)).collect();
    let mut ground_pass = vec![[0.0; Species::COUNT]; faces.len()];
    for snap in snapshots {
        for f in snap {
            let i = pos[&(f.x, f.y, f.z)];
            for (k, sp) in PHOTO.iter().enumerate() {
                ground_pass[i][sp.index()] += f64::from(u8::from(f.water_ok[k])) / snapshots.len() as f64;
            }
            ground_pass[i][Species::Glowcap.index()] += f64::from(u8::from(f.glow_ok)) / snapshots.len() as f64;
        }
    }
    let geo = Geo {
        width: i64::from(width),
        depth: i64::from(depth),
        columns,
        faces,
        zone: faces.iter().map(|f| zones.of_face[&(f.x, f.y, f.z)]).collect(),
        ground_pass,
    };
    let nz = zones.zones.len();
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); nz];
    for (i, &z) in geo.zone.iter().enumerate() {
        members[z].push(i);
    }
    let mut rng = Rng(seed);
    let mut kernels = Vec::new();
    let mut runners = Vec::new();
    let cap = f64::from(width.max(depth));
    for sp in Species::ALL {
        let sc = config.species(sp);
        let mut land = vec![vec![0.0; nz]; nz];
        let mut lost = vec![0.0; nz];
        let (mut rb_faces, mut rb_same, mut rb_n) = (0.0, 0.0, 0.0);
        for z in 0..nz {
            if members[z].is_empty() {
                continue;
            }
            let mut tally = vec![0.0; nz];
            let mut lost_n = 0.0;
            for _ in 0..samples {
                let d = members[z][rng.below(members[z].len())];
                let home = &faces[d];
                let (hx, hz) = (i64::from(home.x), i64::from(home.z));
                let hop = i64::from(sc.hop.max(1));
                let target: Option<usize> = match sc.dispersal {
                    Dispersal::Drop | Dispersal::Water => {
                        let mut t = None;
                        for _ in 0..16 {
                            let dx = rng.below((2 * hop + 1) as usize) as i64 - hop;
                            let dz = rng.below((2 * hop + 1) as usize) as i64 - hop;
                            let zz = hz + dz;
                            if zz < 0 || zz >= geo.depth {
                                continue;
                            }
                            if let Some(i) = geo.highest(hx + dx, zz)
                                && i != d
                            {
                                t = Some(i);
                                break;
                            }
                        }
                        t
                    }
                    Dispersal::Wind | Dispersal::Wald => {
                        let mut t = None;
                        for _ in 0..config.wind_tries.max(1) {
                            let r = if sc.dispersal == Dispersal::Wind {
                                let a = config.wind_scale_hops * f64::from(sc.hop.max(1));
                                (a * ((1.0 - rng.unit()).powf(-1.0 / config.wind_shape) - 1.0)).min(cap)
                            } else {
                                (wald(&mut rng, sc.wald_mean_m, sc.wald_shape_m) / config.voxel_m).min(cap)
                            };
                            let th = std::f64::consts::TAU * rng.unit();
                            let (s, c) = th.sin_cos();
                            let (mut dx, mut dz) = ((r * c).round() as i64, (r * s).round() as i64);
                            if (dx, dz) == (0, 0) {
                                dx = c.round() as i64;
                                dz = s.round() as i64;
                            }
                            let zz = reflect(hz + dz, geo.depth);
                            if let Some(i) = geo.highest(hx + dx, zz)
                                && i != d
                            {
                                t = Some(i);
                                break;
                            }
                        }
                        t
                    }
                    Dispersal::Spores => {
                        let dark = sc.dark_spore_bias > 0.0 && sc.dark_sky_max > 0.0;
                        let mut cands: Vec<(usize, f64)> = Vec::new();
                        let mut last = None;
                        let mut t = None;
                        for _ in 0..sc.spore_tries.max(1) {
                            let dx = rng.below((2 * hop + 1) as usize) as i64 - hop;
                            let dz = rng.below((2 * hop + 1) as usize) as i64 - hop;
                            let zz = hz + dz;
                            if (dx, dz) == (0, 0) || zz < 0 || zz >= geo.depth {
                                continue;
                            }
                            let col = geo.columns.get(&((hx + dx).rem_euclid(geo.width), zz));
                            let Some(col) = col else { continue };
                            for (k, &i) in col.iter().enumerate() {
                                if i == d || (k > 0 && !dark) {
                                    continue;
                                }
                                last = Some(i);
                                if rng.unit() < geo.ground_pass[i][sp.index()] {
                                    if dark {
                                        let dk = faces[i].darkness;
                                        let w = if k == 0 { 1.0 + sc.dark_spore_bias * dk } else { sc.dark_spore_bias * dk };
                                        if w > 0.0 {
                                            cands.push((i, w));
                                        }
                                    } else {
                                        t = Some(i);
                                        break;
                                    }
                                }
                            }
                            if t.is_some() {
                                break;
                            }
                        }
                        if dark && !cands.is_empty() {
                            let tot: f64 = cands.iter().map(|c| c.1).sum();
                            let mut u = rng.unit() * tot;
                            for (i, w) in &cands {
                                u -= w;
                                if u <= 0.0 {
                                    t = Some(*i);
                                    break;
                                }
                            }
                            t = t.or(Some(cands.last().unwrap().0));
                        }
                        if t.is_none() {
                            lost_n += 1.0;
                            let _ = last;
                            continue;
                        }
                        t
                    }
                };
                if let Some(i) = target {
                    tally[geo.zone[i]] += 1.0;
                }
            }
            let tot: f64 = tally.iter().sum::<f64>() + lost_n;
            if tot > 0.0 {
                for z2 in 0..nz {
                    land[z][z2] = tally[z2] / tot;
                }
                lost[z] = lost_n / tot;
            }
        }
        // The runner's box (`runner_target`, seeds.rs:1078), sampled over all faces.
        let reach = i64::from(sc.runner_reach);
        if reach > 0 {
            for _ in 0..samples.min(4000) {
                let d = rng.below(faces.len());
                let home = &faces[d];
                let mut n = 0.0;
                let mut same = 0.0;
                for dz in -reach..=reach {
                    let zz = i64::from(home.z) + dz;
                    if zz < 0 || zz >= geo.depth {
                        continue;
                    }
                    for dx in -reach..=reach {
                        if dx == 0 && dz == 0 {
                            continue;
                        }
                        if let Some(col) = geo.columns.get(&((i64::from(home.x) + dx).rem_euclid(geo.width), zz))
                            && let Some(&i) = col.iter().find(|&&i| (i64::from(faces[i].y) - i64::from(home.y)).abs() <= 1)
                        {
                            n += 1.0;
                            same += f64::from(u8::from(geo.zone[i] == geo.zone[d]));
                        }
                    }
                }
                rb_faces += n;
                rb_same += same;
                rb_n += 1.0;
            }
        }
        kernels.push(Kernel { land, lost });
        runners.push(RunnerBox {
            faces: if rb_n > 0.0 { rb_faces / rb_n } else { 0.0 },
            same_zone: if rb_faces > 0.0 { rb_same / rb_faces } else { 0.0 },
        });
    }
    (kernels, runners)
}
