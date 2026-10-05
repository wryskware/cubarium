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

/// **Light extinction** per species and stage: ordinary least squares of `−ln(L / sky)` on
/// the zone's crown area index, `−ln(L/sky) = a + k · CAI`, `L` each stand's last-tick
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
            let k = if d > 1e-12 { ((self.n * self.xy - self.x * self.y) / d).max(0.0) } else { 0.0 };
            let a = ((self.y - k * self.x) / self.n).max(0.0);
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
            let y = -(raw / f.sky).clamp(1e-3, 1.0).ln();
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
