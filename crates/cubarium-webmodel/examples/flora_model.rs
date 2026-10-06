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
        Some("elig") => elig(&args[2..]),
        Some("hetero") => hetero(&args[2..]),
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

/// `hetero DIR SPECIES MINUTES`: every stand of SPECIES in a save stepped by the stand law
/// for MINUTES on its own face's light, water and mineral (held), against the same stands on
/// their zone's means (one input at a time and all together): how much the mean field's
/// averaging (Jensen) moves the foliage built.
fn hetero(a: &[String]) {
    use cubarium_voxel_flora::Species;
    use cubarium_webmodel::flora::{census::PHOTO, climate::DayModel, stand, zones};
    let p = config::resolve(Path::new("config/scenarios/terrarium.toml"), &[]).unwrap();
    let fc = config::flora_of(&p);
    let dir = Path::new(&a[0]);
    let sp = Species::parse(&a[1]).unwrap();
    let minutes: f64 = a[2].parse().unwrap();
    let k = PHOTO.iter().position(|&x| x == sp).unwrap();
    let sc = fc.species(sp);
    let (world, flora) = census::read_save(dir).unwrap();
    let faces = census::census(&world, &flora, Some(&fc));
    let zs = zones::build(&fc, &[faces.clone()]);
    let minute0: f64 = std::fs::read_to_string(dir.join("minute")).unwrap().trim().parse().unwrap();
    let dm = DayModel::of(&p.world, fc.upkeep_q10);
    // (zone, light_eff, available, saturated, mineral, stand)
    let mut st: Vec<(usize, [f64; 4], stand::StandState)> = Vec::new();
    for f in &faces {
        let Some((p2, r)) = f.plant else { continue };
        if p2 != sp {
            continue;
        }
        let z = zs.of_face[&(f.x, f.y, f.z)];
        st.push((z, [r.light, f.water[k].unwrap_or(0.0), f.saturated[k], f.mineral], stand::StandState { wood: r.wood, foliage: r.foliage, reserve: r.reserve, aeration_stress: r.aeration_stress, cured: r.cured, ..Default::default() }));
    }
    let nz = zs.zones.len();
    let mut mean = vec![[0.0f64; 5]; nz];
    for (z, x, _) in &st {
        for i in 0..4 {
            mean[*z][i] += x[i];
        }
        mean[*z][4] += 1.0;
    }
    for m in &mut mean {
        for i in 0..4 {
            m[i] /= m[4].max(1.0);
        }
    }
    let names = ["own", "mean-all", "mean-light", "mean-water", "mean-mineral"];
    for (v, name) in names.iter().enumerate() {
        let (mut built, mut fol, mut dead, mut wood) = (0.0, 0.0, 0.0, 0.0);
        for (z, x, s0) in &st {
            let mut inp = *x;
            match v {
                1 => inp = [mean[*z][0], mean[*z][1], mean[*z][2], mean[*z][3]],
                2 => inp[0] = mean[*z][0],
                3 => {
                    inp[1] = mean[*z][1];
                    inp[2] = mean[*z][2]
                }
                4 => inp[3] = mean[*z][3],
                _ => {}
            }
            let site = stand::Site { light_eff: inp[0], available: inp[1], saturated: inp[2], mineral: inp[3], darkness: 0.0, substrate_taken: 0.0, substrate_gross: 0.0 };
            let mut s1 = *s0;
            let dt = 10.0;
            let mut alive = true;
            for i in 0..(minutes * 6.0) as usize {
                let c = dm.at(minute0 * 60.0 + i as f64 * dt, dt);
                let (fl, d) = stand::grow(sc, &mut s1, site, c);
                built += fl.built;
                if d.is_some() {
                    alive = false;
                    break;
                }
                stand::fund(sc, &mut s1, dt);
            }
            if alive {
                fol += s1.foliage;
                wood += s1.wood;
            } else {
                dead += 1.0;
            }
        }
        println!("hetero,{},{name},stands={},built={built:.0},foliage_end={fol:.0},wood_end={wood:.1},died={dead:.0}", sp.name(), st.len());
    }
}

/// `elig DIR..`: per photo species, the faces of a save where a newborn could come up now,
/// by the occupant class (bare, a cover it may come up through): seeds banked, the water
/// and terrain-light gates passed, and the canopy gate passed on the newborn's own canopy
/// light (`nbc`) or on terrain sky alone (`open`), at the face's own mineral and water.
fn elig(a: &[String]) {
    use cubarium_voxel_flora::Species;
    use cubarium_webmodel::flora::{census::PHOTO, model::n_star, stand::light_response};
    let p = config::resolve(Path::new("config/scenarios/terrarium.toml"), &[]).unwrap();
    let fc = config::flora_of(&p);
    for d in a {
        let (world, flora) = census::read_save(Path::new(d)).unwrap();
        let faces = census::census(&world, &flora, Some(&fc));
        println!("== {d}");
        for (k, sp) in PHOTO.iter().enumerate() {
            let sc = fc.species(*sp);
            // [bare, cover]: faces with seed, gates ok, canopy ok on nbc, canopy ok open; seeds on eligible
            let mut n = [[0.0f64; 6]; 2];
            let mut tau = [0.0f64; 9];
            let (mut mins, mut nstars, mut all_min): (Vec<f64>, Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new(), Vec::new());
            for f in &faces {
                let cls = match f.plant {
                    None => 0,
                    Some((c, _)) => {
                        let cc = fc.species(c);
                        if cc.ground_cover && sc.overtop_per_check > 0.0 && (sc.overtop_covers.is_empty() || sc.overtop_covers.contains(&c)) { 1 } else { continue }
                    }
                };
                if !f.highest && false {
                    continue;
                }
                let seeds = f64::from(f.seeds[sp.index()]);
                n[cls][5] += 1.0;
                all_min.push(f.mineral);
                if seeds <= 0.0 {
                    continue;
                }
                n[cls][0] += 1.0;
                if !(f.water_ok[k] && f.light_ok[k]) {
                    continue;
                }
                n[cls][1] += 1.0;
                for j in 0..9 {
                    tau[j] += f64::from(f.nb_tau[k][j]);
                }
                let mu = f.water[k].map_or(0.0, |w| sc.moisture_at(w));
                let pays = |t: f64| f.mineral >= n_star(sc, light_response(sc, f.sky * t), mu);
                mins.push(f.mineral);
                nstars.push(n_star(sc, light_response(sc, f.sky * (-f.nb_tau[k].iter().map(|&v| f64::from(v)).sum::<f64>()).exp()), mu));
                if pays((-f.nb_tau[k].iter().map(|&v| f64::from(v)).sum::<f64>()).exp()) {
                    n[cls][2] += 1.0;
                    n[cls][4] += seeds;
                }
                if pays(1.0) {
                    n[cls][3] += 1.0;
                }
            }
            println!(
                "elig,{},bare:faces={:.0},seeded={:.0},gates={:.0},canopy={:.0},open={:.0},seeds={:.0} | cover:faces={:.0},seeded={:.0},gates={:.0},canopy={:.0},open={:.0},seeds={:.0}",
                sp.name(), n[0][5], n[0][0], n[0][1], n[0][2], n[0][3], n[0][4], n[1][5], n[1][0], n[1][1], n[1][2], n[1][3], n[1][4]
            );
            let q = |v: &mut Vec<f64>| -> String {
                v.sort_by(f64::total_cmp);
                if v.is_empty() { return "-".into(); }
                format!("{:.2}/{:.2}/{:.2}", v[v.len() / 4], v[v.len() / 2], v[3 * v.len() / 4])
            };
            println!("  quartiles: open-face mineral {} | seeded-gated mineral {} | their n* {}", q(&mut all_min), q(&mut mins), q(&mut nstars));
            let g = (n[0][1] + n[1][1]).max(1.0);
            println!("  mean tau over gate-passing seeded faces: {}", Species::ALL.iter().map(|x| format!("{}={:.2}", &x.name()[..5], tau[x.index()] / g)).collect::<Vec<_>>().join(" "));
        }
        // Runners: per clonal species, its donors and how many have a box face a daughter
        // may take now (open, its gates and canopy gate passing at the face's own mineral).
        let mut at: std::collections::HashMap<(i64, i64), Vec<usize>> = std::collections::HashMap::new();
        for (i, f) in faces.iter().enumerate() {
            at.entry((i64::from(f.x), i64::from(f.z))).or_default().push(i);
        }
        let width = faces.iter().map(|f| i64::from(f.x)).max().unwrap_or(0) + 1;
        for (k, sp) in PHOTO.iter().enumerate() {
            let sc = fc.species(*sp);
            if sc.clonal_share <= 0.0 {
                continue;
            }
            let (mut donors, mut with, mut box_faces, mut passing, mut open_faces) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for f in &faces {
                let Some((p, r)) = f.plant else { continue };
                if p != *sp || r.wood < sc.donor_min {
                    continue;
                }
                donors += 1.0;
                let reach = i64::from(sc.runner_reach);
                let mut any = false;
                for dx in -reach..=reach {
                    for dz in -reach..=reach {
                        if dx == 0 && dz == 0 {
                            continue;
                        }
                        let Some(v) = at.get(&((i64::from(f.x) + dx).rem_euclid(width), i64::from(f.z) + dz)) else { continue };
                        let Some(&gi) = v.iter().filter(|&&g| (i64::from(faces[g].y) - i64::from(f.y)).abs() <= 1).min_by_key(|&&g| (i64::from(faces[g].y) - i64::from(f.y)).abs()) else { continue };
                        let g = &faces[gi];
                        box_faces += 1.0;
                        let open = match g.plant {
                            None => true,
                            Some((c, _)) => sc.runners_overtop && c != *sp && fc.species(c).ground_cover && (sc.overtop_covers.is_empty() || sc.overtop_covers.contains(&c)),
                        };
                        if !open {
                            continue;
                        }
                        open_faces += 1.0;
                        if !(g.water_ok[k] && g.light_ok[k]) {
                            continue;
                        }
                        let mu = g.water[k].map_or(0.0, |w| sc.moisture_at(w));
                        let t = (-g.nb_tau[k].iter().map(|&v| f64::from(v)).sum::<f64>()).exp();
                        if g.mineral >= n_star(sc, light_response(sc, g.sky * t), mu) {
                            passing += 1.0;
                            any = true;
                        }
                    }
                }
                if any {
                    with += 1.0;
                }
            }
            println!("runner,{},donors={donors:.0},with_target={with:.0},box_faces={box_faces:.0},open={open_faces:.0},passing={passing:.0}", sp.name());
        }
        let _ = Species::ALL;
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
    let ck = cubarium_webmodel::flora::fit::clumping(&zs);
    if kv(a, "clump=") != Some("0") {
        for sp in Species::ALL {
            mp.seed_k[sp.index()] = ck[sp.index()].value;
        }
        if kv(a, "clump=") != Some("global") {
            mp.seed_k_zone = cubarium_webmodel::flora::fit::clumping_by_zone(&zs, &ck);
        }
    }
    eprintln!("clumping k: {}", Species::ALL.iter().map(|s| { let f = ck[s.index()]; format!("{}={:.3} [{:.3},{:.3}] n={}", s.name(), f.value, f.min, f.max, f.n) }).collect::<Vec<_>>().join(" "));
    let sb = cubarium_webmodel::flora::fit::self_shade_beta(&zs);
    if kv(a, "selfbeta=") != Some("0") {
        for sp in Species::ALL {
            mp.self_beta[sp.index()] = sb[sp.index()].value;
        }
    } else {
        mp.self_beta = [0.0; Species::COUNT];
    }
    eprintln!("self-shade beta: {}", Species::ALL.iter().map(|s| { let f = sb[s.index()]; format!("{}={:.2}±{:.2} n={}", s.name(), f.value, f.max - f.value, f.n) }).collect::<Vec<_>>().join(" "));
    let period = kv(a, "period=").map_or(60.0, |x| x.parse().unwrap()) * 60.0;
    let wseed = kv(a, "wseed=").map_or(1, |x| x.parse().unwrap());
    mp.weather = if kv(a, "weather=") == Some("frozen") { model::Weather::Frozen } else { model::Weather::Replay { period_s: period, seed: wseed } };
    // Scenario levers (not sim knobs): `hold=SPECIES,..` holds each named species' site
    // mineral, as its own stands read it, at the zone's start mean (no self-depletion).
    if let Some(h) = kv(a, "hold=") {
        for name in h.split(',') {
            mp.hold_mineral[Species::parse(name).unwrap().index()] = true;
        }
    }
    if kv(a, "wrank=") == Some("0") {
        mp.water_rank = false;
    }
    if kv(a, "nbl=") == Some("0") {
        mp.newborn_light = false;
    }
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
    for sp in Species::ALL {
        let f = m.funnel(sp.index());
        let rf = m.runner_funnel(sp.index());
        if rf[3] > 0.0 {
            eprintln!("rfunnel,{},faces={:.0},takeable={:.1},donors={:.0},donors_with_target={:.1},box={:.1}x{:.2}", sp.name(), rf[0], rf[1], rf[3], rf[2], m.runner_box[sp.index()].faces, m.runner_box[sp.index()].same_zone);
        }
        eprintln!("funnel,{},bare:faces={:.0},seeded={:.0},gates={:.0},canopy={:.1} | cover:faces={:.0},seeded={:.0},gates={:.0},canopy={:.1}", sp.name(), f[0][0], f[0][1], f[0][2], f[0][3], f[1][0], f[1][1], f[1][2], f[1][3]);
    }
    if let Some(name) = kv(a, "selftau=") {
        let sp = Species::parse(name).unwrap();
        let k = cubarium_webmodel::flora::census::PHOTO.iter().position(|&p| p == sp).unwrap();
        for (zi, z) in m.zones.zones.iter().enumerate() {
            for (st, smp) in z.seeded.iter().enumerate() {
                let v = &smp[k];
                if v.len() < 3 {
                    continue;
                }
                let t: f64 = v.iter().map(|x| f64::from(x.tau[sp.index()])).sum::<f64>() / v.len() as f64;
                let o: f64 = v.iter().map(|x| x.tau.iter().enumerate().filter(|(j, _)| *j != sp.index()).map(|(_, &y)| f64::from(y)).sum::<f64>()).sum::<f64>() / v.len() as f64;
                println!("selftau,{zi},{st},{},{:.4},{:.4},{:.4}", v.len(), z.foliage_ref[st][sp.index()] / z.area, t, o);
            }
        }
    }
    if let Some(name) = kv(a, "wstates=") {
        let k = Species::parse(name).unwrap().index();
        for (zi, z) in m.zones.zones.iter().enumerate() {
            let v: Vec<String> = z.weather.iter().map(|w| format!("{:.2}/{:.2}/{:.2}", w[k].mu_held, w[k].available_held, w[k].gate)).collect();
            eprintln!("ws z{zi} {} area {} : {}", z.key.name(), z.area, v.join(" "));
        }
    }
    if let Some(name) = kv(a, "rcheck=") {
        use cubarium_webmodel::flora::{census::PHOTO, model::n_star, stand::light_response};
        let (name, seedmode) = match name.strip_suffix(":seed") {
            Some(n) => (n, true),
            None => (name, false),
        };
        let sp = Species::parse(name).unwrap();
        let k = PHOTO.iter().position(|&p| p == sp).unwrap();
        let sc = fc.species(sp);
        let snap = &snaps[0];
        let mut donors: std::collections::HashMap<(i64, i64), Vec<u32>> = Default::default();
        for f in snap {
            if f.plant.is_some_and(|(p, r)| p == sp && r.wood >= sc.donor_min) {
                donors.entry((i64::from(f.x), i64::from(f.z))).or_default().push(f.y);
            }
        }
        let width = snap.iter().map(|f| i64::from(f.x)).max().unwrap() + 1;
        let mut per = vec![[0.0f64; 3]; m.zones.zones.len()];
        let mut split = [0.0f64; 2];
        for f in snap {
            if !f.plant.is_none_or(|(p, _)| fc.species(p).ground_cover) {
                continue;
            }
            let reach = i64::from(sc.runner_reach);
            let allowed = match f.plant {
                None => true,
                Some((c, _)) => c != sp && (sc.overtop_covers.is_empty() || sc.overtop_covers.contains(&c)) && if seedmode { sc.overtop_per_check > 0.0 } else { sc.runners_overtop },
            };
            if !allowed {
                continue;
            }
            let near = if seedmode {
                f.seeds[sp.index()] > 0
            } else {
                (-reach..=reach).any(|dx| (-reach..=reach).any(|dz| donors.get(&((i64::from(f.x) + dx).rem_euclid(width), i64::from(f.z) + dz)).is_some_and(|v| v.iter().any(|&y| (i64::from(y) - i64::from(f.y)).abs() <= 1 && !(dx == 0 && dz == 0 && y == f.y)))))
            };
            if !near {
                continue;
            }
            let zi = m.zones.of_face[&(f.x, f.y, f.z)];
            per[zi][0] += 1.0;
            if f.water_ok[k] && f.light_ok[k] {
                per[zi][1] += 1.0;
                let mu = f.water[k].map_or(0.0, |w| sc.moisture_at(w));
                let t = (-f.nb_tau[k].iter().map(|&v| f64::from(v)).sum::<f64>()).exp();
                if f.mineral >= n_star(sc, light_response(sc, f.sky * t), mu) {
                    per[zi][2] += 1.0;
                    split[usize::from(f.plant.is_some())] += 1.0;
                }
            }
        }
        println!("rsplit,{},bare={:.0},cover={:.0}", sp.name(), split[0], split[1]);
        for (zi, p) in per.iter().enumerate() {
            if p[0] > 0.0 {
                let mf = m.zone_funnel(zi, sp.index(), seedmode);
                eprintln!("rcheck z{zi} {} census: faces={:.0} gates={:.0} pass={:.0} | model: faces={:.1} gates={:.1} pass={:.1}", m.zones.zones[zi].key.name(), p[0], p[1], p[2], mf[0], mf[1], mf[2]);
            }
        }
    }
    if let Some(name) = kv(a, "zfunnel=") {
        let sp = Species::parse(name).unwrap();
        let k = cubarium_webmodel::flora::census::PHOTO.iter().position(|&p| p == sp).unwrap();
        for (zi, z) in m.zones.zones.iter().enumerate() {
            let st = z.seed_stats[0][sp.index()];
            let smp = &z.seeded[0][k];
            let g = smp.iter().filter(|x| x.gate).count();
            let bank = m.z[zi].bank[sp.index()];
            eprintln!("zf,{zi},{},open={:.0},seeded_census={:.0},bank_census={:.0},bank_model={:.0},seeded_model={:.1},sample_gate={}/{}", z.key.name(), st[0], st[1], st[2], bank, st[0] * cubarium_webmodel::flora::fit::seeded_share(bank / z.area.max(1.0), m.p.seed_k_zone.get(zi).map_or(m.p.seed_k[sp.index()], |v| v[sp.index()])), g, smp.len());
        }
    }
    let t = std::time::Instant::now();
    let end = minute0 + minutes;
    let mut next = minute0;
    loop {
        if m.minute() + 1e-9 >= next {
            line(&m);
            if kv(a, "zocc=") == Some("1") {
                for (i, z) in m.zones.zones.iter().enumerate() {
                    let occ: Vec<String> = Species::ALL.iter().map(|s| format!("{}={:.1}", s.name(), m.z[i].count(s.index()))).collect();
                    let don: Vec<String> = Species::ALL.iter().map(|s| format!("{:.1}", m.count_at_least(i, s.index(), m.config.species(*s).donor_min))).collect();
                    let pool: Vec<String> = (0..=Species::COUNT).map(|k| { let n = if k == Species::COUNT { (z.area - (0..Species::COUNT).filter(|&j| j != Species::Glowcap.index()).map(|j| m.z[i].count(j)).sum::<f64>()).max(0.0) } else { m.z[i].count(k) }; format!("{:.3}", if n > 0.0 { m.z[i].pool[k] / n } else { 0.0 }) }).collect();
                    println!("zocc,{:.0},{},{},{},{},donors={},N={},litter={:.2},dead_wood={:.2}", m.minute(), i, z.key.name(), z.area, occ.join(","), don.join("/"), pool.join("/"), m.z[i].litter, m.z[i].dead_wood);
                }
            }
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
        if let Some(tr) = kv(a, "trace=") {
            let (zs, sp) = tr.split_once(':').unwrap();
            let zi: usize = zs.parse().unwrap();
            let k = Species::parse(sp).unwrap().index();
            if (m.t_s as u64) % (kv(a, "trace_every=").map_or(600, |x| x.parse::<u64>().unwrap()) * 60) == 0 {
                let r = m.zones.zones[zi].weather[m.weather_state()][k];
                let co: Vec<String> = m.z[zi].cohorts[k].iter().filter(|c| c.n >= 0.5).map(|c| format!("{:.1}@W{:.3}P{:.3}Q{:.4}L{:.2}n{:.2}", c.n, c.st.wood, c.st.foliage, c.st.reserve, m.cohort_light(zi, k, c), c.site.n_rel)).collect();
                eprintln!("tr,{:.0},day={:.2},ws={},mu_h={:.2},av_h={:.2},str_h={:.2},gate={:.2},N={:.2} | {}", m.minute(), m.day.at(m.t_s, 10.0).daylight, m.weather_state(), r.mu_held, r.available_held, r.stress_held, r.gate, m.z[zi].pool[k] / m.z[zi].count(k).max(1e-9), co.join(" "));
            }
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
    if kv(a, "prod=") == Some("1") {
        // The producers' output per world day, per zone, over the run (no take after the
        // log's windows): what the animal half's detritivores and capgnawers can draw.
        let days = minutes / (m.day.day_s() / 60.0);
        let gi = Species::Glowcap.index();
        let mut tot = [0.0f64; 4];
        for (i, z) in m.zones.zones.iter().enumerate() {
            let p = m.c.prod_z[i];
            for k in 0..4 {
                tot[k] += p[k];
            }
            let caps_now: f64 = m.z[i].cohorts[gi].iter().map(|c| c.n * c.st.foliage).sum();
            let gsc = m.config.species(Species::Glowcap);
            let caps_cap: f64 = m.z[i].cohorts[gi].iter().map(|c| c.n * gsc.alpha * c.st.wood).sum();
            // The most caps the stands could regrow in a day if grazed to nothing and funded:
            // `foliage_rate · W` per stand (step.rs:3021), over the day.
            let caps_pot: f64 = m.z[i].cohorts[gi].iter().map(|c| c.n * gsc.foliage_rate * c.st.wood).sum::<f64>() * m.day.day_s();
            println!("prod,{},{},area={},litter_in={:.2},dead_wood_in={:.2},caps_built={:.3},foliage_built={:.1},litter_now={:.2},dead_wood_now={:.2},caps_now={:.2},caps_full={:.2},caps_potential={:.2},glowcap={:.0}", i, z.key.name(), z.area, p[0] / days, p[1] / days, p[2] / days, p[3] / days, m.z[i].litter, m.z[i].dead_wood, caps_now, caps_cap, caps_pot, m.z[i].count(gi));
        }
        println!("prodworld,days={days:.2},litter_in={:.1},dead_wood_in={:.1},caps_built={:.2},foliage_built={:.0},film_grown={:.3},film_upkeep={:.3},film_now={:.3},film_capacity={:.3},carrion_now={:.3}", tot[0] / days, tot[1] / days, tot[2] / days, tot[3] / days, m.c.film_grown / days, m.c.film_upkeep / days, m.film, m.film_capacity, m.carrion);
    }
    let zd = kv(a, "zdeaths=").and_then(Species::parse);
    for (i, z) in m.zones.zones.iter().enumerate() {
        if let Some(sp) = zd {
            let k = sp.index();
            let r = m.zones.zones[i].frozen()[k];
            println!("zd,{i},{},births={:.0},deaths={:.0},n={:.0},mu_held={:.2},stress_held={:.2},gate={:.2},sky={:.2},N={:.2}", z.key.name(), m.c.by_zone[i][0][k], m.c.by_zone[i][1][k], m.z[i].count(k), r.mu_held, r.stress_held, r.gate, z.sky, m.z[i].pool[k] / m.z[i].count(k).max(1e-9));
        }
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
    // The sim's `sp` ledger line's counterpart, cumulative since the start.
    let c = &m.c;
    for sp in Species::ALL {
        let k = sp.index();
        let by: Vec<String> = Species::ALL.iter().filter(|n| c.pairs[k][n.index()] >= 0.5).map(|n| format!("{}={:.0}", n.name(), c.pairs[k][n.index()])).collect();
        println!(
            "spm,{:.0},{},landed={:.0},germinated={:.0},seeds_died={:.0},runner_births={:.0},deaths=[starved={:.0} drought={:.0} overtopped={:.0} displaced={:.0} browsed={:.0}],overtopped_by=[{}],built={:.3},shed={:.3}",
            m.minute(),
            sp.name(),
            c.seeds_landed[k],
            c.births_seed[k],
            c.seeds_died[k],
            c.births_runner[k],
            c.starved[k],
            c.drought[k],
            c.overtopped[k],
            c.displaced[k],
            c.browsed[k],
            by.join(" "),
            c.built[k],
            c.shed[k]
        );
    }
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
