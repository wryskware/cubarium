//! Diagnosis (plants-flowering-vines 2026-10-03, Part B 2): where each plant species can
//! start and where it stands, from a `voxel_census save=` directory.
//!
//! ```text
//! stand_niche_diag <save-dir>
//! ```
//!
//! Every soil support face is read once: its sky (terrain only) and, through velvetpad's
//! root box, its mean available water and saturated fraction, binned into dry (< 0.4),
//! moist (0.4 to 1), wet (≥ 1, above field capacity) and soggy (saturated fraction ≥
//! 0.6, velvetpad's own ceiling), and lit (sky ≥ 0.6) or shaded. Prints, per bin, how many
//! sites, how many each species' establishment gates pass, and which species holds the
//! site now; then per species its stands' mean light, moisture factor and aeration
//! stress.

use cubarium_voxel::{Material, World};
use cubarium_voxel_flora::{Site, Species, establishment_gates_with_sky};

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("usage: stand_niche_diag <save-dir>"));
    let read = |n: &str| std::fs::read(dir.join(n)).unwrap_or_else(|e| panic!("{n}: {e}"));
    let world = World::load(&read("world.bin")).expect("world");
    let flora = cubarium_voxel_flora::snapshot::decode(&read("flora.bin")).expect("flora");
    let view = world.view();
    let c = view.config;
    let fv = flora.view();
    let cfg = flora.config();
    let photo: Vec<Species> = Species::ALL.into_iter().filter(|&s| s != Species::Glowcap).collect();
    const BINS: [&str; 8] = [
        "dry/lit", "dry/shade", "moist/lit", "moist/shade", "wet/lit", "wet/shade", "soggy/lit", "soggy/shade",
    ];
    let n = Species::ALL.len();
    let mut sites = [0usize; 8];
    let mut pass = vec![[0usize; 8]; n];
    let mut held = vec![[0usize; 8]; n];
    for z in 0..c.depth {
        for x in 0..c.width {
            for y in view.supports_in_column(x as i64, z) {
                if view.material_at(x as i64, y, z) != Material::Soil {
                    continue;
                }
                let site = Site { x, y, z };
                let sky = view.sky_visibility(x as i64, y, z);
                let g = establishment_gates_with_sky(&view, site, cfg.species(Species::Velvetpad), sky, 0.0, 0.0);
                let w = g.mean_water.unwrap_or(0.0);
                let class = if g.saturated_fraction >= 0.6 {
                    3
                } else if w >= 1.0 {
                    2
                } else if w >= 0.4 {
                    1
                } else {
                    0
                };
                let b = class * 2 + usize::from(sky < 0.6);
                sites[b] += 1;
                for &s in &photo {
                    if establishment_gates_with_sky(&view, site, cfg.species(s), sky, 0.0, 0.0).passes() {
                        pass[s as usize][b] += 1;
                    }
                }
                if let Some(st) = fv.stand_at(site) {
                    held[st.species as usize][b] += 1;
                }
            }
        }
    }
    print!("{:<14}", "bin");
    for b in BINS {
        print!("{b:>12}");
    }
    println!();
    print!("{:<14}", "sites");
    for v in sites {
        print!("{v:>12}");
    }
    println!();
    for &s in &photo {
        print!("{:<14}", format!("pass {}", &s.name()[..s.name().len().min(9)]));
        for v in pass[s as usize] {
            print!("{v:>12}");
        }
        println!();
    }
    for s in Species::ALL {
        print!("{:<14}", format!("held {}", &s.name()[..s.name().len().min(9)]));
        for v in held[s as usize] {
            print!("{v:>12}");
        }
        println!();
    }
    println!("species,stands,mean_light,mean_moisture,mean_aeration_stress,mean_wood_over_max");
    for s in Species::ALL {
        let st: Vec<_> = fv.stands.iter().filter(|x| x.species == s).collect();
        if st.is_empty() {
            continue;
        }
        let k = st.len() as f64;
        let wm = cfg.species(s).wood_max;
        println!(
            "{},{},{:.3},{:.3},{:.3},{:.3}",
            s.name(),
            st.len(),
            st.iter().map(|x| x.light).sum::<f64>() / k,
            st.iter().map(|x| x.moisture).sum::<f64>() / k,
            st.iter().map(|x| x.aeration_stress).sum::<f64>() / k,
            st.iter().map(|x| x.wood / wm).sum::<f64>() / k
        );
    }
}
