//! `vault_spacing` (brief VAULT-SHAPE, 2026-10-10): how far apart vaulttrees stand in a
//! fresh world, so a stand of candidate crowns can be drawn at that spacing.
//!
//! ```text
//! cargo run --release -p cubarium --example vault_spacing -- [CONFIG] [SEEDS] [TICKS]
//! cargo run --release -p cubarium --example vault_spacing -- --flora FLORA.bin WIDTH VOXEL_M
//! ```
//!
//! `--flora` reads a saved world's flora snapshot (a save slot's `flora-<tick>.bin`)
//! instead, for the spacing a long-running world has reached; WIDTH is the ring's width in
//! voxels (the slot's `meta.json` `size[0]`).
//!
//! Founds CONFIG (default `config/desktop/terrarium.toml`, the app's own) for seeds
//! 1..=SEEDS (default 8) as `founding_probe` does, optionally steps TICKS, and prints for
//! each seed the vaulttrees' wood fractions and each one's horizontal distance to its
//! nearest vaulttree neighbour (all of them, and among the adult-stage ones, wood above
//! the profile's adult threshold), in metres across the ring.

use cubarium::voxel::{LayerConfig, Seeds, found, load_config, params_of};
use cubarium_voxel_flora::Species;
use cubarium_voxel_sim::{Sim, SimConfig};

fn nearest(pts: &[(f64, f64)], ring_m: f64) -> Vec<f64> {
    pts.iter()
        .enumerate()
        .map(|(i, a)| {
            pts.iter()
                .enumerate()
                .filter(|&(j, _)| j != i)
                .map(|(_, b)| {
                    let dx = (a.0 - b.0).abs();
                    let dx = dx.min(ring_m - dx);
                    (dx * dx + (a.1 - b.1).powi(2)).sqrt()
                })
                .fold(f64::INFINITY, f64::min)
        })
        .collect()
}

fn summary(label: &str, d: &mut Vec<f64>) {
    d.retain(|v| v.is_finite());
    d.sort_by(f64::total_cmp);
    if d.is_empty() {
        println!("  {label}: none");
        return;
    }
    let med = d[d.len() / 2];
    let mean = d.iter().sum::<f64>() / d.len() as f64;
    println!(
        "  {label}: n={} min={:.2} median={:.2} mean={:.2} max={:.2} m",
        d.len(),
        d[0],
        med,
        mean,
        d[d.len() - 1]
    );
}

fn report(flora: &cubarium_voxel_flora::Flora, voxel_m: f64, ring_m: f64, label: &str) -> (Vec<f64>, Vec<f64>) {
    let sc = flora.config().species(Species::Vaulttree);
    let adult_wf = sc.profile.get(1).map_or(0.4, |p| p.wood_fraction_max);
    let vt: Vec<_> = flora.view().stands.iter().filter(|s| s.species == Species::Vaulttree).copied().collect();
    let pts: Vec<(f64, f64)> =
        vt.iter().map(|s| ((f64::from(s.site.x) + 0.5) * voxel_m, (f64::from(s.site.z) + 0.5) * voxel_m)).collect();
    let d = nearest(&pts, ring_m);
    let adult: Vec<(f64, f64)> =
        vt.iter().zip(&pts).filter(|(s, _)| s.wood / sc.wood_max > adult_wf).map(|(_, p)| *p).collect();
    let da = nearest(&adult, ring_m);
    println!("{label}: {} vaulttrees, {} adult (wood > {adult_wf} max)", vt.len(), adult.len());
    for (s, (p, dn)) in vt.iter().zip(pts.iter().zip(&d)) {
        println!(
            "  x={:.2} z={:.2} y={} wf={:.2} nearest={:.2} m layer fill {:?}",
            p.0,
            p.1,
            s.site.y,
            s.wood / sc.wood_max,
            dn,
            cubarium_voxel_flora::layer_iter(flora.config(), s, voxel_m)
                .filter(|l| l.capacity > 0.0)
                .map(|l| (l.stock / l.capacity * 100.0).round() / 100.0)
                .collect::<Vec<_>>()
        );
    }
    println!("  nearest (adult) m: {:?}", da.iter().map(|v| (v * 100.0).round() / 100.0).collect::<Vec<_>>());
    (d, da)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--flora") {
        let bytes = std::fs::read(&args[2]).expect("FLORA.bin");
        let width: f64 = args[3].parse().expect("WIDTH");
        let voxel_m: f64 = args[4].parse().expect("VOXEL_M");
        let flora = cubarium_voxel_flora::snapshot::decode(&bytes).expect("flora snapshot");
        let (mut d, mut da) = report(&flora, voxel_m, width * voxel_m, &args[2]);
        summary("all vaulttrees", &mut d);
        summary("adult vaulttrees", &mut da);
        return;
    }
    let path = args.get(1).map_or("config/desktop/terrarium.toml", String::as_str);
    let seeds: u64 = args.get(2).map_or(8, |a| a.parse().expect("SEEDS"));
    let ticks: u64 = args.get(3).map_or(0, |a| a.parse().expect("TICKS"));
    let cfg = load_config(std::path::Path::new(path)).expect("config");
    let params = params_of(&cfg, &[]).expect("params");
    let world_cfg = params.world.clone();
    let voxel_m = world_cfg.voxel_m;
    let ring_m = f64::from(world_cfg.width) * voxel_m;
    let counts = params.fauna.founder_counts();
    let (mut all_d, mut adult_d) = (Vec::new(), Vec::new());
    for seed in 1..=seeds {
        let founded = found(
            &world_cfg,
            &LayerConfig::new(&params, voxel_m),
            counts.clone(),
            Seeds::from(seed),
            false,
        );
        let mut sim = Sim::ambient(founded.world, founded.flora, founded.fauna, SimConfig::experiment());
        for _ in 0..ticks {
            sim.step();
        }
        let (d, da) = report(sim.flora(), voxel_m, ring_m, &format!("seed {seed}"));
        all_d.extend(d);
        adult_d.extend(da);
    }
    println!("pooled over {seeds} seeds, {ticks} ticks:");
    summary("all vaulttrees", &mut all_d);
    summary("adult vaulttrees", &mut adult_d);
}
