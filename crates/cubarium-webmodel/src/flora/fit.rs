//! **The fitted rates** (spatial, emergent from where stands and seeds sit, never a sim
//! knob), each from censuses of the soak's own saves, with its spread over snapshots.

use cubarium_voxel_flora::{FloraConfig, Species, Trophic};

use super::census::Face;
use super::model::{NS, crown_of};
use super::zones::Zones;

/// A fitted number and its spread (min, max over the snapshots fitted one by one).
#[derive(Clone, Copy, Debug, Default)]
pub struct Fitted {
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub n: usize,
}

/// The crown area index (adult non-cover crowns, faces of crown over faces) per zone of one
/// census.
pub fn cai(config: &FloraConfig, zones: &Zones, faces: &[Face]) -> Vec<f64> {
    let mut sum = vec![0.0; zones.zones.len()];
    for f in faces {
        if let Some((sp, r)) = f.plant {
            let sc = config.species(sp);
            if sc.trophic == Trophic::Photo && !sc.ground_cover && r.wood >= sc.donor_min {
                sum[zones.of_face[&(f.x, f.y, f.z)]] += crown_of(config, sp, r.wood, r.foliage).0;
            }
        }
    }
    sum.iter().zip(&zones.zones).map(|(s, z)| s / z.area.max(1.0)).collect()
}

/// **Light** per species and stage: ordinary least squares of a stand's relative light
/// `L / sky` on the zone's crown area index, `L/sky = a − k · CAI` (fitted on the light
/// itself, not its log, so the mean light is kept), `L` each stand's last-tick
/// light (`Stand::light`, its response inverted) and `sky` its face's terrain sky. The
/// intercept `a` is the shade a stand's own neighbourhood casts at no zone-wide canopy
/// (seeds fall near their parents, so stands stand in clumps); `k` is the canopy's.
/// Returns `[a, k]` per species and stage, each with its spread over the snapshots.
pub fn light(config: &FloraConfig, zones: &Zones, snaps: &[Vec<Face>]) -> [[[Fitted; 2]; 2]; NS] {
    #[derive(Clone, Copy, Default)]
    struct Acc {
        n: f64,
        x: f64,
        y: f64,
        xx: f64,
        xy: f64,
    }
    impl Acc {
        fn fit(&self) -> Option<(f64, f64)> {
            if self.n < 5.0 {
                return None;
            }
            let d = self.n * self.xx - self.x * self.x;
            let k = if d > 1e-12 { (-(self.n * self.xy - self.x * self.y) / d).max(0.0) } else { 0.0 };
            let a = ((self.y + k * self.x) / self.n).clamp(0.0, 1.0);
            Some((a, k))
        }
    }
    let mut out = [[[Fitted::default(); 2]; 2]; NS];
    let mut per: Vec<[[Acc; 2]; NS]> = Vec::new();
    for faces in snaps {
        let c = cai(config, zones, faces);
        let mut acc = [[Acc::default(); 2]; NS];
        for f in faces {
            let Some((sp, r)) = f.plant else { continue };
            let sc = config.species(sp);
            if sc.trophic != Trophic::Photo || f.sky < 0.05 || r.light <= 0.0 {
                continue;
            }
            let h = sc.light_half;
            let raw = if 1.0 + h - r.light > 1e-9 { h * r.light / (1.0 + h - r.light) } else { f.sky };
            let y = (raw / f.sky).clamp(0.0, 1.0);
            let x = c[zones.of_face[&(f.x, f.y, f.z)]];
            let j = usize::from(r.wood >= sc.donor_min);
            let a = &mut acc[sp.index()][j];
            a.n += 1.0;
            a.x += x;
            a.y += y;
            a.xx += x * x;
            a.xy += x * y;
        }
        per.push(acc);
    }
    for s in 0..NS {
        for j in 0..2 {
            let mut all = Acc::default();
            for p in &per {
                let q = p[s][j];
                all.n += q.n;
                all.x += q.x;
                all.y += q.y;
                all.xx += q.xx;
                all.xy += q.xy;
            }
            let each: Vec<(f64, f64)> = per.iter().filter_map(|p| p[s][j].fit()).collect();
            let (a, k) = all.fit().unwrap_or((0.0, 0.0));
            let spread = |v: Vec<f64>| (v.iter().copied().fold(f64::INFINITY, f64::min), v.iter().copied().fold(f64::NEG_INFINITY, f64::max));
            let (amin, amax) = spread(each.iter().map(|e| e.0).collect());
            let (kmin, kmax) = spread(each.iter().map(|e| e.1).collect());
            out[s][j] = [
                Fitted { value: a, min: amin, max: amax, n: each.len() },
                Fitted { value: k, min: kmin, max: kmax, n: each.len() },
            ];
        }
    }
    let _ = Species::ALL;
    out
}

/// The share of open faces holding at least one whole seed when the zone banks `m` seeds
/// per face, under a negative-binomial seed shadow of clumping `k` (`k → ∞` is Poisson):
/// `1 − (1 + m/k)^(−k)`.
pub fn seeded_share(m: f64, k: f64) -> f64 {
    if !(m > 0.0) {
        return 0.0;
    }
    if !k.is_finite() || k > 1e6 {
        return 1.0 - (-m).exp();
    }
    1.0 - (1.0 + m / k).powf(-k)
}

/// **Seed clumping per zone** and species: [`clumping`]'s fit over one zone's snapshots
/// alone (seeds pile under parents where nothing comes up, so a zone's shadow is its own),
/// `global` where the zone has no data.
pub fn clumping_by_zone(zones: &Zones, global: &[Fitted; NS]) -> Vec<[f64; NS]> {
    let grid: Vec<f64> = (0..=60).map(|i| 10f64.powf(-3.0 + 5.0 * f64::from(i) / 60.0)).collect();
    zones
        .zones
        .iter()
        .map(|z| {
            std::array::from_fn(|s| {
                // Snapshots where the zone banked seeds but none lay on a face the species
                // may take count too (the shadow misses those faces): least squares on the
                // share itself, which a zero can enter.
                let pts: Vec<(f64, f64)> = z
                    .seed_stats
                    .iter()
                    .filter(|st| st[s][0] >= 5.0 && st[s][2] > 0.0)
                    .map(|st| (st[s][2] / z.area.max(1.0), st[s][1] / st[s][0]))
                    .collect();
                if pts.is_empty() {
                    return global[s].value;
                }
                grid.iter()
                    .copied()
                    .map(|k| (k, pts.iter().map(|&(m, o)| (seeded_share(m, k) - o).powi(2)).sum::<f64>()))
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map_or(global[s].value, |x| x.0)
            })
        })
        .collect()
}

/// **Seed clumping** per species: the negative-binomial `k` whose [`seeded_share`] best
/// matches the open faces holding a seed against the zone's banked seeds per face, least
/// squares on the log share over every zone and snapshot with at least 20 open faces and a
/// seed; the spread is the same fit snapshot by snapshot. Seeds fall near their parents,
/// so a bank covers fewer faces than a well-mixed one would.
pub fn clumping(zones: &Zones) -> [Fitted; NS] {
    let grid: Vec<f64> = (0..=60).map(|i| 10f64.powf(-3.0 + 5.0 * f64::from(i) / 60.0)).collect();
    let best = |pts: &[(f64, f64)]| -> Option<f64> {
        if pts.is_empty() {
            return None;
        }
        grid.iter()
            .copied()
            .map(|k| (k, pts.iter().map(|&(m, o)| (seeded_share(m, k).max(1e-9).ln() - o.max(1e-9).ln()).powi(2)).sum::<f64>()))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|x| x.0)
    };
    let n_states = zones.zones.first().map_or(0, |z| z.seed_stats.len());
    std::array::from_fn(|s| {
        let pts_of = |st: Option<usize>| -> Vec<(f64, f64)> {
            let mut v = Vec::new();
            for z in &zones.zones {
                for (si, stats) in z.seed_stats.iter().enumerate() {
                    if st.is_some_and(|x| x != si) {
                        continue;
                    }
                    let [open, seeded, bank] = stats[s];
                    if open >= 20.0 && seeded > 0.0 && bank > 0.0 {
                        v.push((bank / z.area.max(1.0), seeded / open));
                    }
                }
            }
            v
        };
        let all = pts_of(None);
        let Some(value) = best(&all) else {
            return Fitted { value: f64::INFINITY, min: f64::INFINITY, max: f64::INFINITY, n: 0 };
        };
        let each: Vec<f64> = (0..n_states).filter_map(|si| best(&pts_of(Some(si)))).collect();
        Fitted { value, min: each.iter().copied().fold(f64::INFINITY, f64::min), max: each.iter().copied().fold(0.0, f64::max), n: all.len() }
    })
}

/// **Self-shade over the seed shadow** per species: the exponent `β` of the mean optical
/// depth a species' own crowns cast over the open faces holding its seeds against its
/// foliage per face in the zone, `τ_self ∝ F^β` (least squares on the logs over every zone
/// and snapshot with at least 3 seeded faces). Seeds fall under their parents, so the shade
/// over them grows with the parents, but less than in proportion (`β < 1`): new crowns
/// partly overlap old ones. `0.5` where fewer than 5 points.
pub fn self_shade_beta(zones: &Zones) -> [Fitted; NS] {
    use super::census::PHOTO;
    std::array::from_fn(|s| {
        let Some(k) = PHOTO.iter().position(|p| p.index() == s) else {
            return Fitted { value: 0.5, min: 0.5, max: 0.5, n: 0 };
        };
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for z in &zones.zones {
            for (st, smp) in z.seeded.iter().enumerate() {
                let v = &smp[k];
                let f = z.foliage_ref[st][s] / z.area.max(1.0);
                if v.len() < 3 || !(f > 0.0) {
                    continue;
                }
                let t = v.iter().map(|x| f64::from(x.tau[s])).sum::<f64>() / v.len() as f64;
                if t > 0.0 {
                    pts.push((f.ln(), t.ln()));
                }
            }
        }
        if pts.len() < 5 {
            return Fitted { value: 0.5, min: 0.5, max: 0.5, n: pts.len() };
        }
        let n = pts.len() as f64;
        let (mx, my) = (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n);
        let sxx: f64 = pts.iter().map(|p| (p.0 - mx).powi(2)).sum();
        if sxx <= 0.0 {
            return Fitted { value: 0.5, min: 0.5, max: 0.5, n: pts.len() };
        }
        let b = (pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>() / sxx).clamp(0.0, 1.0);
        // The spread: the slope's standard error, as a ± band.
        let resid: f64 = pts.iter().map(|p| (p.1 - my - b * (p.0 - mx)).powi(2)).sum();
        let se = (resid / (n - 2.0).max(1.0) / sxx).sqrt();
        Fitted { value: b, min: b - se, max: b + se, n: pts.len() }
    })
}
