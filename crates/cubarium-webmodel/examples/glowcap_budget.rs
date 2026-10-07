//! Whole-web hourly budget from a current study save, rather than the old soak layout.
use cubarium_voxel_fauna::Founder;
use cubarium_voxel_flora::Species;
use cubarium_webmodel::web::{Web, sheet::sheet, start};
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |key: &str| args.iter().find_map(|s| s.strip_prefix(key));
    let dir = Path::new(arg("load=").expect("load=DIR"));
    let sets: Vec<String> = args
        .iter()
        .filter_map(|s| s.strip_prefix("set=").map(str::to_owned))
        .collect();
    let p = start::params(
        Path::new(arg("config=").unwrap_or("config/desktop/terrarium.toml")),
        &sets,
    )
    .unwrap();
    let snaps: Vec<PathBuf> = arg("snaps=")
        .unwrap_or("")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect();
    let flora = start::flora(&p, dir, &snaps, 1).unwrap();
    let save = cubarium_voxel_sim::saved::load(dir, &[]).unwrap();
    let fv = save.fauna.view();
    let mut census = start::FaunaCensus {
        tick: fv.tick,
        ..Default::default()
    };
    for a in fv.animals {
        if let Some(f) = a.founder {
            let host = a
                .attached
                .and_then(|r| fv.animal(r.host))
                .and_then(|h| h.founder);
            census
                .bodies
                .push((f, a.body, a.reserve, a.age_ticks, host));
        }
    }
    for c in fv.clutches {
        census.clutches.push((
            c.lineage,
            c.count,
            c.organic,
            c.laid_tick,
            c.dormancy.is_some(),
        ));
    }
    let mut fauna = start::fauna(&p, start::closed_fits(), &census, &Founder::ALL, 1.0);
    cubarium_webmodel::web::to_whole(&mut fauna);
    fauna.area_scale = flora.zones.zones.iter().map(|z| z.area).sum::<f64>() / 4748.0;
    let mut w = Web::new(flora, fauna);
    let m0 = w.minute();
    let hours: usize = arg("hours=").unwrap_or("6").parse().unwrap();
    for hour in 1..=hours {
        let a = w.snap();
        w.run_to(m0 + hour as f64 * 60.0);
        let b = w.snap();
        println!("{}", sheet(&a, &b, w.day_s()));
        let s = w.flora.stands();
        let glow = s[Species::Glowcap.index()];
        let caps = w
            .flora
            .z
            .iter()
            .flat_map(|z| &z.cohorts[Species::Glowcap.index()])
            .map(|c| c.n * c.st.foliage)
            .sum::<f64>();
        let spores = w
            .flora
            .z
            .iter()
            .map(|z| z.bank[Species::Glowcap.index()] * w.flora.config.glowcap.propagule_package())
            .sum::<f64>();
        let fungal = w
            .flora
            .z
            .iter()
            .flat_map(|z| &z.cohorts[Species::Glowcap.index()])
            .map(|c| c.n * c.st.material())
            .sum::<f64>()
            + spores;
        println!(
            "hour,{hour},glowcap={},caps={caps},glow_organic={fungal},glow_spores={spores},world_organic={},share={}",
            glow.0,
            w.organic(),
            fungal / w.organic()
        );
    }
}
