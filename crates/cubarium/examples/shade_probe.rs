//! `shade_probe` (SHADE-REAL, 2026-10-10): a saved world's light under this build's shade
//! law, against the light the save recorded under the law it ran. No tick is run: the
//! stands, crowns and terrain are the save's, and only the light is recomputed
//! ([`Flora::refresh_founder_light`]).
//!
//! Reports, per photosynthetic species, the recorded and recomputed light and the share
//! of stands that could not pay their upkeep at the new light with a full crown and full
//! water and mineral; vaulttrees by profile stage; the ground's transmission and a vaulttree
//! newborn's canopy gate under adult vaulttree crowns and in the gaps beside them; and a
//! static thinning of the adults (the one that pays least dies, the light is recomputed,
//! until every adult pays `1 / water_mineral`) with the survivors' spacing.
//!
//! ```text
//! cargo run --release -p cubarium --example shade_probe -- --save DIR [--config FILE] [--wm 0.6]
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use cubarium::voxel::VoxelConfig;
use cubarium_voxel_flora::{Flora, Site, Species, Trophic, break_even_cover};

fn q(v: &mut Vec<f64>) -> String {
    v.retain(|x| x.is_finite());
    if v.is_empty() {
        return "-".into();
    }
    v.sort_by(f64::total_cmp);
    let p = |f: f64| v[((v.len() - 1) as f64 * f) as usize];
    format!("p10 {:.3} p25 {:.3} med {:.3} p75 {:.3} p90 {:.3} (n {})", p(0.1), p(0.25), p(0.5), p(0.75), p(0.9), v.len())
}

fn main() -> Result<()> {
    let mut config = PathBuf::from("config/desktop/terrarium-wide.toml");
    let mut save = None::<PathBuf>;
    let mut wm = 1.0f64;
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        match k.as_str() {
            "--config" => config = it.next().context("--config FILE")?.into(),
            "--save" => save = Some(it.next().context("--save DIR")?.into()),
            "--wm" => wm = it.next().context("--wm X")?.parse()?,
            _ => anyhow::bail!("unknown argument {k}"),
        }
    }
    let cfg: VoxelConfig = toml::from_str(&std::fs::read_to_string(&config)?)?;
    let save = save.context("--save DIR")?;
    let prepared = cubarium_session::prepare_loaded(&cfg.sim, &save).map_err(anyhow::Error::msg)?;
    let (world, mut flora) = (prepared.world, prepared.flora);
    let c = world.config().clone();
    let voxel_m = c.voxel_m;
    let ring_m = f64::from(c.width) * voxel_m;
    let fc = flora.config().clone();
    println!(
        "world {}x{}x{} voxel {voxel_m}; light_extinction_k {}; water*mineral factor for the pay tests {wm}",
        c.width, c.height, c.depth, fc.light_extinction_k
    );
    let old: Vec<f64> = flora.view().stands.iter().map(|s| s.light).collect();
    flora.refresh_founder_light(&world);
    let pays = |f: &Flora, i: usize, l: f64| {
        // At full foliage (`α · W`): whether the light lets a whole crown pay, not whether a
        // browsed one does today.
        let s = &f.view().stands[i];
        let sc = f.config().species(s.species);
        break_even_cover(sc, s.wood, sc.alpha * s.wood, l, wm, 1.0)
    };

    println!("\nper species: light recorded -> recomputed; share that cannot pay at the new light");
    for sp in Species::ALL {
        let idx: Vec<usize> = (0..old.len()).filter(|&i| flora.view().stands[i].species == sp).collect();
        if idx.is_empty() {
            continue;
        }
        let photo = fc.species(sp).trophic == Trophic::Photo;
        let mut o: Vec<f64> = idx.iter().map(|&i| old[i]).collect();
        let mut n: Vec<f64> = idx.iter().map(|&i| flora.view().stands[i].light).collect();
        let short_old = idx.iter().filter(|&&i| photo && pays(&flora, i, old[i]) < 1.0).count();
        let short_new = idx.iter().filter(|&&i| photo && pays(&flora, i, flora.view().stands[i].light) < 1.0).count();
        println!("{:>13} n {:>5}  old {}\n{:>13}          new {}\n{:>13}          cannot pay: old {short_old} new {short_new}", sp.name(), idx.len(), q(&mut o), "", q(&mut n), "");
    }

    let vt: Vec<usize> = (0..old.len()).filter(|&i| flora.view().stands[i].species == Species::Vaulttree).collect();
    let sc = fc.species(Species::Vaulttree).clone();
    println!("\nvaulttree by stage: light recorded -> recomputed");
    for st in 0..3u8 {
        let ids: Vec<usize> = vt.iter().copied().filter(|&i| flora.view().stands[i].profile_stage == st).collect();
        let mut o: Vec<f64> = ids.iter().map(|&i| old[i]).collect();
        let mut n: Vec<f64> = ids.iter().map(|&i| flora.view().stands[i].light).collect();
        let short = ids.iter().filter(|&&i| pays(&flora, i, flora.view().stands[i].light) < 1.0).count();
        println!("  stage {st}: old {}\n           new {}  cannot pay {short}", q(&mut o), q(&mut n));
    }

    // Columns under an adult crown, and gaps within 6 m of an adult but under no vaulttree.
    let pos = |s: &cubarium_voxel_flora::Stand| ((f64::from(s.site.x) + 0.5) * voxel_m, (f64::from(s.site.z) + 0.5) * voxel_m);
    let hd = |a: (f64, f64), b: (f64, f64)| {
        let dx = (a.0 - b.0).abs();
        let dx = dx.min(ring_m - dx);
        (dx * dx + (a.1 - b.1).powi(2)).sqrt()
    };
    let crowns: Vec<((f64, f64), f64, u8)> = vt
        .iter()
        .map(|&i| {
            let s = &flora.view().stands[i];
            (pos(s), sc.crown_radius_m_at(s.wood), s.profile_stage)
        })
        .collect();
    let view = world.view();
    let (mut t_under, mut t_gap, mut g_under, mut g_gap) = (vec![], vec![], vec![], vec![]);
    let (mut pass_under, mut pass_gap) = (0usize, 0usize);
    for z in 0..c.depth {
        for x in 0..c.width {
            let Some(y) = (1..c.height).rev().find(|&y| view.is_support(i64::from(x), y, z)) else { continue };
            let site = Site { x, y, z };
            let p = ((f64::from(x) + 0.5) * voxel_m, (f64::from(z) + 0.5) * voxel_m);
            let under_adult = crowns.iter().any(|&(q, r, st)| st == 2 && hd(p, q) <= r);
            let under_any = crowns.iter().any(|&(q, r, _)| hd(p, q) <= r);
            let near_adult = crowns.iter().any(|&(q, _, st)| st == 2 && hd(p, q) <= 6.0);
            if !(under_adult || (near_adult && !under_any)) {
                continue;
            }
            let t = cubarium_voxel_flora::study::canopy_at(&flora.view(), c.width, c.depth, site, f64::from(y) + 1.0);
            let (l_eff, cover) = flora.newborn_canopy(&world, site, Species::Vaulttree);
            let ok = cover * wm >= sc.establish_light_margin;
            if under_adult {
                t_under.push(t);
                g_under.push(l_eff);
                pass_under += usize::from(ok);
            } else {
                t_gap.push(t);
                g_gap.push(l_eff);
                pass_gap += usize::from(ok);
            }
        }
    }
    let (nu, ng) = (t_under.len(), t_gap.len());
    println!("\nground transmission at 0.125 m (canopy only, no terrain sky):");
    println!("  under an adult crown  {}", q(&mut t_under));
    println!("  gap near an adult     {}", q(&mut t_gap));
    println!("vaulttree newborn L_eff (terrain sky x canopy, through its response):");
    println!("  under an adult crown  {}", q(&mut g_under));
    println!("  gap near an adult     {}", q(&mut g_gap));
    println!(
        "vaulttree newborn passes the canopy gate (cover x {wm} >= margin {}): under {pass_under}/{nu} ({:.0} %), gap {pass_gap}/{ng} ({:.0} %)",
        sc.establish_light_margin,
        100.0 * pass_under as f64 / nu.max(1) as f64,
        100.0 * pass_gap as f64 / ng.max(1) as f64
    );

    // Static thinning of the adults alone (shorter stands never shade a taller crown).
    let nn = |f: &Flora| -> Vec<f64> {
        let a: Vec<(f64, f64)> = f.view().stands.iter().filter(|s| s.species == Species::Vaulttree && s.profile_stage == 2).map(pos).collect();
        (0..a.len())
            .map(|i| (0..a.len()).filter(|&j| j != i).map(|j| hd(a[i], a[j])).fold(f64::INFINITY, f64::min))
            .collect()
    };
    println!("\nadult nearest adult, m, as saved: {}", q(&mut nn(&flora)));
    let mut removed = 0usize;
    loop {
        let worst = (0..flora.view().stands.len())
            .filter(|&i| {
                let s = &flora.view().stands[i];
                s.species == Species::Vaulttree && s.profile_stage == 2
            })
            .map(|i| (i, pays(&flora, i, flora.view().stands[i].light)))
            .filter(|&(_, cv)| cv < 1.0)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((i, _)) = worst else { break };
        let s = flora.view().stands[i];
        flora.apply(&world, cubarium_voxel_flora::Command::Clear { x: i64::from(s.site.x), z: s.site.z });
        flora.refresh_founder_light(&world);
        removed += 1;
    }
    let left = flora.view().stands.iter().filter(|s| s.species == Species::Vaulttree && s.profile_stage == 2).count();
    println!("static thinning of adults that cannot pay: {removed} die, {left} stand");
    println!("adult nearest adult, m, after: {}", q(&mut nn(&flora)));

    // Static thinning of every photosynthetic stand: each round the quarter that pays
    // least among those that cannot pay dies, the light is recomputed; to a fixed point.
    let count = |f: &Flora| -> Vec<usize> {
        Species::ALL.iter().map(|&sp| f.view().stands.iter().filter(|s| s.species == sp).count()).collect()
    };
    let before = count(&flora);
    let mut rounds = 0;
    loop {
        let mut short: Vec<(usize, f64)> = (0..flora.view().stands.len())
            .filter(|&i| fc.species(flora.view().stands[i].species).trophic == Trophic::Photo)
            .map(|i| (i, pays(&flora, i, flora.view().stands[i].light)))
            .filter(|&(_, cv)| cv < 1.0)
            .collect();
        if short.is_empty() || rounds >= 60 {
            break;
        }
        short.sort_by(|a, b| a.1.total_cmp(&b.1));
        let kill = (short.len() / 4).max(1);
        let sites: Vec<(Site, cubarium_voxel_flora::FaceLayer)> =
            short[..kill].iter().map(|&(i, _)| (flora.view().stands[i].site, flora.view().stands[i].place)).collect();
        flora.remove_stands(&sites);
        flora.refresh_founder_light(&world);
        rounds += 1;
    }
    let after = count(&flora);
    println!("\nstatic thinning of every photosynthetic stand that cannot pay ({rounds} rounds):");
    for (k, sp) in Species::ALL.iter().enumerate() {
        println!("  {:>13} {:>6} -> {:>6}", sp.name(), before[k], after[k]);
    }
    for st in 0..3u8 {
        let n = flora.view().stands.iter().filter(|s| s.species == Species::Vaulttree && s.profile_stage == st).count();
        println!("  vaulttree stage {st}: {n}");
    }
    println!("adult nearest adult, m, after all: {}", q(&mut nn(&flora)));
    let mut l: Vec<f64> = flora.view().stands.iter().filter(|s| s.species == Species::Springturf).map(|s| s.light).collect();
    println!("springturf light after all: {}", q(&mut l));
    Ok(())
}
