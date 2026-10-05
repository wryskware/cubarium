//! Diagnosis (plants-flowering-vines 2026-10-03, Part B 1): which big open walls carry
//! latticevine, and why the others do not.
//!
//! ```text
//! vine_wall_diag <save-dir> [min_faces]
//! ```
//!
//! Reads a `voxel_census save=` directory (`world.bin`, `flora.bin`), groups the eligible
//! side faces into planar walls (same direction, joined up, down and sideways in the
//! wall's plane), and prints one row per wall of at least `min_faces` faces (default 40):
//! its direction and extent, its mean face light, its feet (the voxel under the air in
//! front of a bottom face: soil, rock, air or other), its founder candidates (a soil
//! foot or a soil lip, as `seed_vine_founders` takes them), and its cover.

use std::collections::VecDeque;

use cubarium_voxel::{Material, World};
use cubarium_voxel_flora::{Face, FaceDir, Site, cover::face_eligible, cover::face_light};
use rustc_hash::{FxHashMap, FxHashSet};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = std::path::PathBuf::from(args.get(1).expect("usage: vine_wall_diag <save-dir> [min_faces]"));
    let min: usize = args.get(2).map_or(40, |a| a.parse().expect("min_faces"));
    let read = |n: &str| std::fs::read(dir.join(n)).unwrap_or_else(|e| panic!("{n}: {e}"));
    let world = World::load(&read("world.bin")).expect("world");
    let flora = cubarium_voxel_flora::snapshot::decode(&read("flora.bin")).expect("flora");
    let view = world.view();
    let c = view.config;
    let vc = &flora.config().latticevine;
    let fv = flora.view();
    let owned: FxHashMap<Face, u64> = fv.cover.faces().iter().map(|f| (f.face, f.owner)).collect();
    let roots: Vec<(Site, Face)> = fv.cover.vines().iter().map(|v| (v.root, v.root_face)).collect();

    let step = |x: u32, y: u32, z: u32, d: (i64, i64, i64)| -> Option<(u32, u32, u32)> {
        let ny = y as i64 + d.1;
        let nz = z as i64 + d.2;
        if ny < 0 || nz < 0 || ny >= c.height as i64 || nz >= c.depth as i64 {
            return None;
        }
        let nx = (x as i64 + d.0).rem_euclid(c.width as i64);
        Some((nx as u32, ny as u32, nz as u32))
    };
    let front = |f: Face| step(f.x, f.y, f.z, f.dir.offset());
    let sky = |f: Face| -> f64 {
        match front(f) {
            Some((x, y, z)) if y >= 1 => view.sky_visibility(x as i64, y - 1, z),
            _ => 0.0,
        }
    };

    let mut seen: FxHashSet<Face> = FxHashSet::default();
    let mut rows = Vec::new();
    for d in FaceDir::SIDES {
        let o = d.offset();
        // In-plane steps: up, down, and the horizontal axis perpendicular to the normal.
        let side = if o.0 != 0 { (0, 0, 1) } else { (1, 0, 0) };
        let plane = [(0, 1, 0), (0, -1, 0), side, (-side.0, 0, -side.2)];
        for z in 0..c.depth {
            for y in 0..c.height {
                for x in 0..c.width {
                    let f0 = Face::new(x, y, z, d);
                    if seen.contains(&f0) || !face_eligible(&view, f0) {
                        continue;
                    }
                    let mut comp = Vec::new();
                    let mut q = VecDeque::from([f0]);
                    seen.insert(f0);
                    while let Some(f) = q.pop_front() {
                        comp.push(f);
                        for s in plane {
                            if let Some((nx, ny, nz)) = step(f.x, f.y, f.z, s) {
                                let n = Face::new(nx, ny, nz, d);
                                if !seen.contains(&n) && face_eligible(&view, n) {
                                    seen.insert(n);
                                    q.push_back(n);
                                }
                            }
                        }
                    }
                    if comp.len() >= min {
                        rows.push((d, comp));
                    }
                }
            }
        }
    }
    rows.sort_by_key(|r| std::cmp::Reverse(r.1.len()));
    println!(
        "dir,faces,x0,x1,y0,y1,z0,z1,light_mean,light_min,light_max,bottom_faces,foot_soil,foot_rock,foot_air,foot_other,soil_foot_cands,soil_lip_cands,covered,owners,roots_on_wall"
    );
    for (d, comp) in rows {
        let set: FxHashSet<Face> = comp.iter().copied().collect();
        let (mut x0, mut x1, mut y0, mut y1, mut z0, mut z1) = (u32::MAX, 0, u32::MAX, 0, u32::MAX, 0);
        let mut lights = Vec::with_capacity(comp.len());
        let (mut bottom, mut fs, mut fr, mut fa, mut fo, mut cand_foot, mut cand_lip) = (0, 0, 0, 0, 0, 0, 0);
        let mut owners: FxHashSet<u64> = FxHashSet::default();
        let mut covered = 0;
        for &f in &comp {
            x0 = x0.min(f.x);
            x1 = x1.max(f.x);
            y0 = y0.min(f.y);
            y1 = y1.max(f.y);
            z0 = z0.min(f.z);
            z1 = z1.max(f.z);
            lights.push(face_light(vc, d, sky(f)));
            if let Some(o) = owned.get(&f) {
                covered += 1;
                owners.insert(*o);
            }
            let below_in_wall = f.y >= 1 && set.contains(&Face::new(f.x, f.y - 1, f.z, d));
            if !below_in_wall {
                bottom += 1;
                match front(f) {
                    Some((x, y, z)) if y >= 1 => {
                        let m = view.material_at(x as i64, y - 1, z);
                        match m {
                            Material::Soil => {
                                fs += 1;
                                if view.is_support(x as i64, y - 1, z) {
                                    cand_foot += 1;
                                }
                            }
                            Material::Air => fa += 1,
                            m if m.is_solid() => fr += 1,
                            _ => fo += 1,
                        }
                    }
                    _ => fo += 1,
                }
            }
            let above_in_wall = set.contains(&Face::new(f.x, f.y + 1, f.z, d));
            if !above_in_wall
                && view.material_at(f.x as i64, f.y, f.z) == Material::Soil
                && view.is_support(f.x as i64, f.y, f.z)
            {
                cand_lip += 1;
            }
        }
        let n = lights.len() as f64;
        let mean = lights.iter().sum::<f64>() / n;
        let lo = lights.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = lights.iter().cloned().fold(0.0, f64::max);
        let on_wall = roots.iter().filter(|(_, rf)| set.contains(rf)).count();
        println!(
            "{d:?},{},{x0},{x1},{y0},{y1},{z0},{z1},{mean:.3},{lo:.3},{hi:.3},{bottom},{fs},{fr},{fa},{fo},{cand_foot},{cand_lip},{covered},{},{on_wall}",
            comp.len(),
            owners.len()
        );
    }
    // The vines: root, its material, first face, faces owned, water, dormancy.
    eprintln!(
        "vines: {} (dormancies so far {}, deaths {}, sisters {})",
        fv.cover.vines().len(),
        fv.ledger.vine_dormancies,
        fv.ledger.vine_deaths,
        fv.ledger.vine_sisters
    );
    for v in fv.cover.vines() {
        let faces = fv.cover.faces_of(v.id);
        let n = faces.len();
        let leaf: f64 = faces.iter().map(|f| f.leafiness).sum();
        // Mean face light over (up to 200 of) its faces, and the share below the light
        // at which a full face's income, half the day dark, pays its upkeep.
        let pay = 2.0 * (vc.leaf_upkeep + vc.wood_upkeep) / vc.assimilation;
        let stride = n.div_ceil(200).max(1);
        let ls: Vec<f64> = faces.iter().step_by(stride).map(|f| face_light(vc, f.face.dir, sky(f.face))).collect();
        let light = ls.iter().sum::<f64>() / ls.len().max(1) as f64;
        let dark = ls.iter().filter(|&&l| l < pay).count() as f64 / ls.len().max(1) as f64;
        // The drink's ask at full moisture against what the root got last plant step.
        let ask = vc.transpiration_m3_per_s
            * leaf
            * cubarium_voxel::DT
            * f64::from(flora.config().step_every.max(1));
        eprintln!(
            "vine {} lineage {} root ({},{},{}) {:?} root_face {:?} faces {} light {:.2} below_pay {:.2} leaf {:.1} drank/ask {:.3} water {:.3} reserve {:.4} dormant {}",
            v.id,
            v.lineage,
            v.root.x,
            v.root.y,
            v.root.z,
            view.material_at(v.root.x as i64, v.root.y, v.root.z),
            v.root_face.dir,
            n,
            light,
            dark,
            leaf,
            if ask > 0.0 { v.drank_m3 / ask } else { f64::NAN },
            v.water,
            v.reserve,
            v.dormant
        );
    }
}
