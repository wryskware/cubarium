//! The producer half's driver (`design/webmodel/flora.md`).
use std::io::Write;
use std::path::Path;

use cubarium_webmodel::flora::{census, config};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("dump") => {
            let path = args.get(2).map_or("config/scenarios/terrarium.toml", String::as_str);
            let p = config::resolve(Path::new(path), &[]).unwrap();
            println!("{}", toml::to_string(&config::flora_of(&p)).unwrap());
        }
        Some("census") => {
            let dir = Path::new(args.get(2).expect("census DIR"));
            let (world, flora) = census::read_save(dir).unwrap();
            let faces = census::census(&world, &flora, None);
            let mut out = std::io::BufWriter::new(std::io::stdout().lock());
            writeln!(out, "{}", census::csv_header()).unwrap();
            for f in &faces {
                writeln!(out, "{}", census::to_csv(f)).unwrap();
            }
            eprintln!("{} faces, minute {}", faces.len(), std::fs::read_to_string(dir.join("minute")).unwrap_or_default().trim());
        }
        Some("physio") => physio(&args[2..]),
        Some("run") => run(&args[2..]),
        _ => eprintln!("usage: flora_model dump [CONFIG] | census DIR | physio SPECIES L WATER SAT N DAYS newborn|adult"),
    }
}

/// One stand under held conditions, by the sim's own probe and by the model's law.
fn physio(a: &[String]) {
    use cubarium_voxel_flora::Species;
    use cubarium_voxel_flora::study::{ClimateSample, Held, Start, probe, start_stand};
    use cubarium_webmodel::flora::{climate::DayModel, stand};
    let sp = Species::parse(&a[0]).unwrap();
    let f = |i: usize| a[i].parse::<f64>().unwrap();
    let (l, water, sat, n, days) = (f(1), f(2), f(3), f(4), f(5));
    let start = if a[6] == "adult" { Start::Adult } else { Start::Newborn };
    let p = config::resolve(Path::new("config/scenarios/terrarium.toml"), &[]).unwrap();
    let fc = config::flora_of(&p);
    let sc = fc.species(sp);
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    let ticks = (days * dm.day_s() / cubarium_voxel::DT) as usize;
    let t0 = 0.0;
    let clim: Vec<ClimateSample> = (0..ticks)
        .map(|k| {
            let c = dm.at(t0 + k as f64 * cubarium_voxel::DT, cubarium_voxel::DT);
            ClimateSample { daylight: c.daylight, upkeep: c.upkeep, night: c.night, to_dawn_s: c.to_dawn_s }
        })
        .collect();
    let s0 = start_stand(&fc, sp, start);
    let o = probe(&fc, s0, &clim, |_| Held { water, saturated: sat, light: l, mineral: n });
    println!("probe: alive {} died_s {:?} W {:.4}->{:.4} P {:.4}->{:.4} Q {:.4}->{:.4} fixed {:.4} funded {:.4} packages {} donor_s {:?}", o.alive, o.died_s, o.wood[0], o.wood[1], o.foliage[0], o.foliage[1], o.reserve[0], o.reserve[1], o.fixed, o.funded, o.packages, o.donor_s);
    for dt in [cubarium_voxel::DT, 1.0, 5.0, 10.0, 30.0] {
        let mut st = stand::StandState { wood: s0.wood, foliage: s0.foliage, reserve: s0.reserve, ..Default::default() };
        let site = stand::Site { light_eff: stand::light_response(sc, l), available: water, saturated: sat, mineral: n, darkness: 0.0, substrate_taken: 0.0, substrate_gross: 0.0 };
        let steps = (days * dm.day_s() / dt) as usize;
        let (mut fixed, mut pk, mut died) = (0.0, 0, None);
        for k in 0..steps {
            let c = dm.at(t0 + k as f64 * dt, dt);
            let (fl, d) = stand::grow(sc, &mut st, site, c);
            fixed += fl.income;
            if d.is_some() { died = Some(k as f64 * dt); break; }
            pk += stand::fund(sc, &mut st, dt).0;
        }
        println!("model dt {dt:>5}: died {:?} W {:.4} P {:.4} Q {:.4} fixed {:.4} packages {}", died, st.wood, st.foliage, st.reserve, fixed, pk);
    }
}

fn kv<'a>(a: &'a [String], k: &str) -> Option<&'a str> {
    a.iter().find_map(|x| x.strip_prefix(k))
}

/// `run start=DIR snaps=DIR,DIR,.. [minutes=1200] [weather=replay|frozen] [period=60] [wseed=1] [config=FILE] [set=P=V]..`
fn run(a: &[String]) {
    use cubarium_voxel_flora::Species;
    use cubarium_webmodel::flora::{climate::DayModel, kernel, model, zones};
    let cfg = kv(a, "config=").unwrap_or("config/scenarios/terrarium.toml");
    let sets: Vec<String> = a.iter().filter_map(|x| x.strip_prefix("set=").map(str::to_string)).collect();
    let p = config::resolve(Path::new(cfg), &sets).unwrap();
    let fc = config::flora_of(&p);
    let start = Path::new(kv(a, "start=").expect("start=DIR"));
    let mut dirs: Vec<std::path::PathBuf> = vec![start.to_path_buf()];
    if let Some(s) = kv(a, "snaps=") {
        dirs.extend(s.split(',').filter(|x| !x.is_empty()).map(std::path::PathBuf::from));
    }
    let t = std::time::Instant::now();
    let mut snaps = Vec::new();
    let mut dims = (0, 0);
    for d in &dirs {
        let (world, flora) = census::read_save(d).unwrap();
        dims = (world.config().width, world.config().depth);
        snaps.push(census::census(&world, &flora, Some(&fc)));
    }
    let minute0: f64 = std::fs::read_to_string(start.join("minute")).unwrap().trim().parse().unwrap();
    let zs = zones::build(&fc, &snaps);
    let (kern, rb) = kernel::build(&fc, dims.0, dims.1, &zs, &snaps, 3000, 7);
    eprintln!("setup {:.1} s: {} zones", t.elapsed().as_secs_f64(), zs.zones.len());
    for (i, z) in zs.zones.iter().enumerate() {
        eprintln!("  zone {i} {} area {} sky {:.2} dark {:.2} fungal {:.2}", z.key.name(), z.area, z.sky, z.darkness, z.fungal_space);
    }
    let mut mp = model::Params::default();
    let lk = cubarium_webmodel::flora::fit::light(&fc, &zs, &snaps);
    for sp in Species::ALL {
        let k = sp.index();
        mp.light_k[k] = [[lk[k][0][0].value, lk[k][0][1].value], [lk[k][1][0].value, lk[k][1][1].value]];
        let f = |x: &cubarium_webmodel::flora::fit::Fitted| format!("{:.3} [{:.3},{:.3}]", x.value, x.min, x.max);
        eprintln!("light {} juv a {} k {} | adult a {} k {}", sp.name(), f(&lk[k][0][0]), f(&lk[k][0][1]), f(&lk[k][1][0]), f(&lk[k][1][1]));
    }
    let period = kv(a, "period=").map_or(60.0, |x| x.parse().unwrap()) * 60.0;
    let wseed = kv(a, "wseed=").map_or(1, |x| x.parse().unwrap());
    mp.weather = if kv(a, "weather=") == Some("frozen") { model::Weather::Frozen } else { model::Weather::Replay { period_s: period, seed: wseed } };
    if let Some(dt) = kv(a, "dt=") {
        mp.dt = dt.parse().unwrap();
    }
    if let Some(h) = kv(a, "browse=") {
        // per day, on the woody juveniles
        for sp in [Species::Bloomcrown, Species::Lanternberry, Species::Vaulttree] {
            mp.juvenile_browse[sp.index()] = h.parse::<f64>().unwrap() / DayModel::of(&p.world, fc.upkeep_q10).day_s();
        }
    }
    let minutes: f64 = kv(a, "minutes=").map_or(1200.0, |x| x.parse().unwrap());
    let every: f64 = kv(a, "every=").map_or(30.0, |x| x.parse().unwrap());
    let mut m = model::Model::new(fc.clone(), DayModel::of(&p.world, fc.upkeep_q10), zs, kern, rb, &snaps[0], minute0, mp);
    // The animals' take, clamped to the run's logged flux windows (`flux,ARM,A-B,...,
    // foliage_taken=X,litter_taken=Y`): spread evenly over each window, the foliage over
    // the species by their edible stock (the log has no per-species split).
    let clamp: Vec<(f64, f64, f64, f64)> = kv(a, "log=")
        .map(|f| {
            std::fs::read_to_string(f)
                .unwrap()
                .lines()
                .filter(|l| l.starts_with("flux,"))
                .filter_map(|l| {
                    let p: Vec<&str> = l.split(',').collect();
                    let (a, b) = p[2].split_once('-')?;
                    let get = |k: &str| p.iter().find_map(|x| x.split(" |").next()?.strip_prefix(k)).and_then(|v| v.parse::<f64>().ok());
                    Some((a.parse().ok()?, b.parse().ok()?, get("foliage_taken=")?, get("litter_taken=")?))
                })
                .collect()
        })
        .unwrap_or_default();
    let t = std::time::Instant::now();
    let end = minute0 + minutes;
    let mut next = minute0;
    loop {
        if m.minute() + 1e-9 >= next {
            line(&m);
            next += every;
        }
        if m.minute() >= end - 1e-9 {
            break;
        }
        if let Some(&(a0, b0, fol, lit)) = clamp.iter().find(|w| m.minute() >= w.0 && m.minute() < w.1) {
            let per = m.p.dt / ((b0 - a0) * 60.0);
            let sup = if m.cohorts() > 0 && (m.minute() as u64) % 10 == 0 { Some(m.supply()) } else { None };
            if let Some(sup) = sup {
                let tot: f64 = sup.foliage.iter().enumerate().filter(|(k, _)| *k != Species::Glowcap.index()).map(|(_, v)| v).sum();
                for k in 0..Species::COUNT {
                    m.take.foliage[k] = if k == Species::Glowcap.index() || tot <= 0.0 { 0.0 } else { fol * per * sup.foliage[k] / tot };
                }
            }
            m.take.litter = lit * per;
        } else {
            m.take = Default::default();
        }
        m.step();
    }
    eprintln!("ran {minutes} min in {:.2} s ({} cohorts)", t.elapsed().as_secs_f64(), m.cohorts());
    let c = &m.c;
    let n = |v: &[f64; 9]| Species::ALL.iter().map(|s| format!("{}={:.0}", s.name(), v[s.index()])).collect::<Vec<_>>().join(" ");
    println!("births_seed {}", n(&c.births_seed));
    println!("births_runner {}", n(&c.births_runner));
    println!("starved {}", n(&c.starved));
    println!("drought {}", n(&c.drought));
    println!("overtopped {}", n(&c.overtopped));
    println!("displaced {}", n(&c.displaced));
    println!("browsed {}", n(&c.browsed));
    let pv = Species::Velvetpad.index();
    let pt = Species::Springturf.index();
    let woody = [Species::Bloomcrown.index(), Species::Lanternberry.index(), Species::Vaulttree.index()];
    println!(
        "flows pad_to_turf={:.0} pad_to_woody={:.0} turf_to_woody={:.0} built={:.0} shed={:.0} substrate={:.0}",
        c.pairs[pv][pt],
        woody.iter().map(|&w| c.pairs[pv][w]).sum::<f64>(),
        woody.iter().map(|&w| c.pairs[pt][w]).sum::<f64>(),
        c.built.iter().sum::<f64>(),
        c.shed.iter().sum::<f64>(),
        c.substrate
    );
    for (i, z) in m.zones.zones.iter().enumerate() {
        let occ: Vec<String> = Species::ALL.iter().filter(|s| m.z[i].count(s.index()) >= 0.5).map(|s| format!("{}={:.0}", &s.name()[..5], m.z[i].count(s.index()))).collect();
        println!("zone {} {} cover {:.2} {}", i, z.key.name(), m.z[i].cover, occ.join(" "));
    }
}

fn line(m: &cubarium_webmodel::flora::model::Model) {
    use cubarium_voxel_flora::Species;
    let st = m.stands();
    let sup = m.supply();
    let (pool, inv, bank) = m.mineral();
    let parts: Vec<String> = Species::ALL.iter().map(|s| format!("{}={:.0}/{:.0}/{:.1}", s.name(), st[s.index()].0, st[s.index()].1, sup.foliage[s.index()])).collect();
    let litter: f64 = m.z.iter().map(|z| z.litter).sum();
    let mut diag = String::new();
    for sp in [Species::Springturf, Species::Velvetpad, Species::Lanternberry, Species::Vaulttree, Species::Stonecushion, Species::Bloomcrown] {
        let k = sp.index();
        let sc = m.config.species(sp);
        let bank: f64 = m.z.iter().map(|z| z.bank[k]).sum();
        let (mut jn, mut jw, mut jcov, mut an) = (0.0, 0.0, 0.0, 0.0);
        for z in &m.z {
            for c in &z.cohorts[k] {
                if c.st.wood < sc.donor_min {
                    jn += c.n;
                    jw += c.n * c.st.wood;
                    if c.covered { jcov += c.n; }
                } else {
                    an += c.n;
                }
            }
        }
        diag += &format!(" {}:bank={:.0},juv={:.0}@{:.3},cov={:.2},ad={:.0}", &sp.name()[..5], bank, jn, jw / jn.max(1e-9), jcov / jn.max(1e-9), an);
    }
    println!("diag,{:.0},{}", m.minute(), diag);
    println!(
        "model,{:.0},edible={:.1},litter={:.1},mature_turf_n={:.0},mineral={:.1}+{:.1}+{:.1},stands/donors/edible: {}",
        m.minute(),
        sup.foliage.iter().sum::<f64>(),
        litter,
        st[Species::Springturf.index()].1,
        pool,
        inv,
        bank,
        parts.join(" ")
    );
}
