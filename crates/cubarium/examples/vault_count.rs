//! `vault_count` (brief VAULT-COUNT, 2026-10-10): what a saved world's vaulttree count
//! is made of: stands by profile stage and foliage, their spacing, how many sit under a
//! bigger vaulttree's crown, how they clump, and how many cells each one draws once every
//! stand is stamped (the app's own model library and stamping order).
//!
//! ```text
//! cargo run --release -p cubarium --example vault_count -- --save DIR [--config FILE]
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use cubarium::voxel::VoxelConfig;
use cubarium::voxel::stand::{Part, Stands, total_foliage_fraction};
use cubarium_voxel_flora::Species;

fn q(v: &mut Vec<f64>) -> String {
    v.retain(|x| x.is_finite());
    if v.is_empty() {
        return "-".into();
    }
    v.sort_by(f64::total_cmp);
    let p = |f: f64| v[((v.len() - 1) as f64 * f) as usize];
    format!(
        "min {:.2} p25 {:.2} med {:.2} p75 {:.2} max {:.2}",
        p(0.0),
        p(0.25),
        p(0.5),
        p(0.75),
        p(1.0)
    )
}

fn main() -> Result<()> {
    let mut config = PathBuf::from("config/desktop/terrarium.toml");
    let mut save = None::<PathBuf>;
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        match k.as_str() {
            "--config" => config = it.next().context("--config FILE")?.into(),
            "--save" => save = Some(it.next().context("--save DIR")?.into()),
            _ => anyhow::bail!("unknown argument {k}"),
        }
    }
    let cfg: VoxelConfig = toml::from_str(&std::fs::read_to_string(&config)?)?;
    let save = save.context("--save DIR")?;
    let prepared = cubarium_session::prepare_loaded(&cfg.sim, &save).map_err(anyhow::Error::msg)?;
    let (world, flora) = (prepared.world, prepared.flora);
    let lib = cubarium::voxel::load_models(&cfg).context("models")?;
    let c = world.config();
    let voxel_m = c.voxel_m;
    let ring_m = f64::from(c.width) * voxel_m;
    let fv = flora.view();
    let sc = fv.config.species(Species::Vaulttree);
    println!(
        "world {}x{}x{} voxel {voxel_m} m, ring {ring_m} m; vaulttree crown_radius_m {:?} crown_height_m {:?} \
         wood_max {} hop {} establish_light_min {} establish_light_margin {} foliage_state_below {:?}",
        c.width,
        c.height,
        c.depth,
        sc.crown_radius_m,
        sc.crown_height_m,
        sc.wood_max,
        sc.hop,
        sc.establish_light_min,
        sc.establish_light_margin,
        sc.foliage_state_below
    );

    // Every stand stamped, as the app does, with owners.
    let mut stands = Stands::empty(c.width, c.height, c.depth);
    stands.track_owners();
    stands.rebuild_with(&world.view(), fv, &lib);
    let mut owned = vec![0u32; fv.stands.len()];
    for (x, y, z) in stands.cells() {
        if matches!(
            stands.at(i64::from(x), i64::from(y), z),
            Part::Trunk(_) | Part::Crown { .. }
        ) && let Some(o) = stands.owner(i64::from(x), y, z)
        {
            owned[o] += 1;
        }
    }
    // Each vaulttree stamped alone: the cells it would draw with nothing else there.
    let vt: Vec<usize> = (0..fv.stands.len())
        .filter(|&i| fv.stands[i].species == Species::Vaulttree)
        .collect();
    let mut solo = vec![0u32; fv.stands.len()];
    let base = {
        let v0 = cubarium_voxel_flora::FloraView {
            stands: &[],
            ground: &[],
            crowns: cubarium_voxel_flora::CrownCache::none(),
            ..fv
        };
        let mut s0 = Stands::empty(c.width, c.height, c.depth);
        s0.rebuild_with(&world.view(), v0, &lib);
        s0.cells()
            .filter(|&(x, y, z)| {
                matches!(
                    s0.at(i64::from(x), i64::from(y), z),
                    Part::Trunk(_) | Part::Crown { .. }
                )
            })
            .count() as u32
    };
    println!("cells drawn with no stand at all (vines): {base}");
    for &i in &vt {
        let one = std::slice::from_ref(&fv.stands[i]);
        let v1 = cubarium_voxel_flora::FloraView {
            stands: one,
            ground: &[],
            crowns: cubarium_voxel_flora::CrownCache::none(),
            ..fv
        };
        let mut s1 = Stands::empty(c.width, c.height, c.depth);
        s1.rebuild_with(&world.view(), v1, &lib);
        solo[i] = s1
            .cells()
            .filter(|&(x, y, z)| {
                matches!(
                    s1.at(i64::from(x), i64::from(y), z),
                    Part::Trunk(_) | Part::Crown { .. }
                )
            })
            .count() as u32
            - base;
    }

    let pos = |i: usize| {
        let s = &fv.stands[i];
        (
            (f64::from(s.site.x) + 0.5) * voxel_m,
            f64::from(s.site.y) * voxel_m,
            (f64::from(s.site.z) + 0.5) * voxel_m,
        )
    };
    let hdist = |a: (f64, f64, f64), b: (f64, f64, f64)| {
        let dx = (a.0 - b.0).abs();
        let dx = dx.min(ring_m - dx);
        (dx * dx + (a.2 - b.2).powi(2)).sqrt()
    };
    let stage = |i: usize| fv.stands[i].profile_stage as usize;
    if let Ok(path) = std::env::var("VAULT_CSV") {
        let mut out = String::from(
            "id,x_m,y_m,z_m,stage,wood_fraction,height_m,radius_m,foliage,light,cells_alone,cells_drawn\n",
        );
        for &i in &vt {
            let s = &fv.stands[i];
            let p = pos(i);
            out += &format!(
                "{},{:.3},{:.3},{:.3},{},{:.3},{:.3},{:.3},{:.3},{:.3},{},{}\n",
                s.id,
                p.0,
                p.1,
                p.2,
                stage(i),
                s.wood / sc.wood_max,
                sc.crown_height_m_at(s.wood),
                sc.crown_radius_m_at(s.wood),
                total_foliage_fraction(s, sc),
                s.light,
                solo[i],
                owned[i]
            );
        }
        std::fs::write(path, out)?;
    }
    let names = ["seedling (stage 0)", "sapling (stage 1)", "adult (stage 2)"];
    for st in 0..3 {
        let ids: Vec<usize> = vt.iter().copied().filter(|&i| stage(i) == st).collect();
        let f = |g: &dyn Fn(usize) -> f64| ids.iter().map(|&i| g(i)).collect::<Vec<f64>>();
        println!("\n{}: n={}", names[st], ids.len());
        println!(
            "  wood fraction  {}",
            q(&mut f(&|i| fv.stands[i].wood / sc.wood_max))
        );
        println!(
            "  total foliage  {}",
            q(&mut f(&|i| total_foliage_fraction(&fv.stands[i], sc)))
        );
        println!("  light          {}", q(&mut f(&|i| fv.stands[i].light)));
        println!(
            "  height m       {}",
            q(&mut f(&|i| sc.crown_height_m_at(fv.stands[i].wood)))
        );
        println!(
            "  radius m       {}",
            q(&mut f(&|i| sc.crown_radius_m_at(fv.stands[i].wood)))
        );
        println!("  cells alone    {}", q(&mut f(&|i| f64::from(solo[i]))));
        println!("  cells drawn    {}", q(&mut f(&|i| f64::from(owned[i]))));
        let buckets = [0u32, 1, 3, 10, 50, u32::MAX];
        let mut hist = String::new();
        for w in buckets.windows(2) {
            let n = ids
                .iter()
                .filter(|&&i| owned[i] >= w[0] && owned[i] < w[1])
                .count();
            hist += &format!(
                " [{}..{}):{n}",
                w[0],
                if w[1] == u32::MAX {
                    "inf".into()
                } else {
                    w[1].to_string()
                }
            );
        }
        println!("  drawn-cell histogram{hist}");
        let fol = [0.0, 0.2, 0.4, 0.6, 0.8, 1.01];
        let mut fh = String::new();
        for w in fol.windows(2) {
            let n = ids
                .iter()
                .filter(|&&i| {
                    let t = total_foliage_fraction(&fv.stands[i], sc);
                    t >= w[0] && t < w[1]
                })
                .count();
            fh += &format!(" [{:.1},{:.1}):{n}", w[0], w[1]);
        }
        println!("  foliage histogram{fh}");
        // Nearest vaulttree of any stage, and nearest of the same stage, horizontally.
        let mut any = Vec::new();
        let mut same = Vec::new();
        let mut under = 0;
        for &i in &ids {
            let p = pos(i);
            let mut da = f64::INFINITY;
            let mut ds = f64::INFINITY;
            let mut covered = false;
            for &j in &vt {
                if j == i {
                    continue;
                }
                let d = hdist(p, pos(j));
                da = da.min(d);
                if stage(j) == st {
                    ds = ds.min(d);
                }
                // Under a bigger vaulttree's crown: inside its radius, below its top.
                let o = &fv.stands[j];
                if o.wood > fv.stands[i].wood
                    && d <= sc.crown_radius_m_at(o.wood)
                    && p.1 < pos(j).1 + sc.crown_height_m_at(o.wood)
                    && p.1 + 0.5 >= pos(j).1
                {
                    covered = true;
                }
            }
            any.push(da);
            same.push(ds);
            under += usize::from(covered);
        }
        println!("  nearest any vaulttree m   {}", q(&mut any));
        println!("  nearest same stage m      {}", q(&mut same));
        println!("  under a bigger vaulttree's crown: {under}");
    }

    // Clumps: single linkage on horizontal distance (and within 1 m of height).
    for link in [0.5, 1.0, 2.0] {
        let n = vt.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut Vec<usize>, i: usize) -> usize {
            let mut r = i;
            while p[r] != r {
                r = p[r];
            }
            let mut k = i;
            while p[k] != r {
                let nx = p[k];
                p[k] = r;
                k = nx;
            }
            r
        }
        for a in 0..n {
            for b in a + 1..n {
                let (pa, pb) = (pos(vt[a]), pos(vt[b]));
                if hdist(pa, pb) <= link && (pa.1 - pb.1).abs() <= 1.0 {
                    let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
                    parent[ra] = rb;
                }
            }
        }
        let mut groups: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
        for a in 0..n {
            let r = find(&mut parent, a);
            groups.entry(r).or_default().push(vt[a]);
        }
        let mut sizes: Vec<usize> = groups.values().map(Vec::len).collect();
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        let with_adult = groups
            .values()
            .filter(|g| g.iter().any(|&i| stage(i) == 2))
            .count();
        let with_tall = groups
            .values()
            .filter(|g| {
                g.iter()
                    .any(|&i| sc.crown_height_m_at(fv.stands[i].wood) >= 1.5)
            })
            .count();
        let drawn = groups
            .values()
            .filter(|g| g.iter().any(|&i| owned[i] >= 10))
            .count();
        println!(
            "\nclumps at {link} m: {} clumps; with an adult {with_adult}; with a stand >= 1.5 m tall {with_tall}; \
             with a stand drawing >= 10 cells {drawn}; largest {:?}",
            groups.len(),
            &sizes[..sizes.len().min(12)]
        );
    }

    // Where: 10 m bins round the ring, and the floor levels.
    let mut bins = [[0usize; 3]; 8];
    for &i in &vt {
        let b = ((pos(i).0 / ring_m * 8.0) as usize).min(7);
        bins[b][stage(i)] += 1;
    }
    println!(
        "\nring bins of {} m [seedling, sapling, adult]: {bins:?}",
        ring_m / 8.0
    );
    let mut ys: std::collections::BTreeMap<u32, [usize; 3]> = Default::default();
    for &i in &vt {
        ys.entry(fv.stands[i].site.y / 8).or_default()[stage(i)] += 1;
    }
    println!("site y (per 1 m band) [seedling, sapling, adult]: {ys:?}");
    let bank: f64 = fv
        .ground
        .iter()
        .flat_map(|g| g.seeds.iter())
        .filter(|c| c.species == Species::Vaulttree)
        .map(|c| c.organic)
        .sum();
    let bank_sites = fv
        .ground
        .iter()
        .filter(|g| g.seeds.iter().any(|c| c.species == Species::Vaulttree))
        .count();
    let k = Species::Vaulttree.index();
    let l = fv.ledger;
    println!(
        "seed bank: {bank:.2} organic on {bank_sites} sites; ledger: landed {} germinated {} died {} lost {} canopy_refused {}",
        l.seeds_landed[k],
        l.seeds_germinated[k],
        l.seeds_died[k],
        l.seeds_lost[k],
        l.canopy_refused[k]
    );
    // The densest grove, for a picture: the 10 m bin with most adults.
    let (bi, _) = bins.iter().enumerate().max_by_key(|(_, b)| b[2]).unwrap();
    println!(
        "densest adult bin: x {}..{} m",
        bi as f64 * ring_m / 8.0,
        (bi + 1) as f64 * ring_m / 8.0
    );
    Ok(())
}
