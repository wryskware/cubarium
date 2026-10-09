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
        Some("export") => export(&args[2..]),
        Some("physio") => physio(&args[2..]),
        Some("elig") => elig(&args[2..]),
        Some("hetero") => hetero(&args[2..]),
        Some("run") => run(&args[2..]),
        _ => eprintln!(
            "usage: flora_model dump [CONFIG] | census DIR | export SAVE OUT [config=FILE] [overlay=FILE] [set=P=V].. | physio SPECIES L WATER SAT N DAYS newborn|adult [trace=S] [set=P=V].. | elig DIR.. | hetero DIR SPECIES MINUTES\n  \
             run start=DIR snaps=DIR,.. [log=SOAKLOG] [minutes=1200] [every=30] [weather=frozen] [wseed=N] [config=FILE] [set=P=V].. \
             [browse=PER_DAY] [hold=SPECIES,..] [zocc=1] [prod=1] [rcheck=SPECIES[:seed]] [zfunnel=SPECIES] [zdeaths=SPECIES] [trace=ZONE:SPECIES]\n  \
             meadow levers (design/webmodel/meadow.md): [take_hold=1] [take_const=FOLIAGE_PER_2H[:LITTER]] [crop=K] [cropw=SHARE] [dry=EVERY_D:LEN_D:WATER[:PHASE_D]] [drywet=1] \
             [thatch=X] [draw=X] [contest=ESCAPE_SHARE[:TAU] | contest=sim] [life=DAYS | life=SPECIES:DAYS,..] [soilmin=SPECIES:N,..] [soilhist=1] [rare=SPECIES:KEEP[:AT_DAY]] [rain=SPECIES:SEEDS_PER_FACE_DAY,..]\n  \
             diagnosis (meadow round 2): [diag=SPECIES,..] (per-zone ledger lines `tz` at every print) [gates=SPECIES] (per-zone gate shares in each snapshot)"
        ),
    }
}

/// `export SAVE OUT [config=FILE] [overlay=FILE] [set=P=V]..`: the census of a save, read
/// with this build (the one that shares the save's schemas) under the config's plant layer,
/// written to `OUT/` as a census file (`census::write_census`): the model's start state from
/// then on, whatever schema later builds read. `overlay=` lays a scenario file's tables over
/// the config, as the closed loop's soak validation does (`web::start::params_with_overlay`).
fn export(a: &[String]) {
    let save = Path::new(a.first().expect("export SAVE OUT"));
    let out = Path::new(a.get(1).expect("export SAVE OUT"));
    let cfg = kv(a, "config=").unwrap_or("config/scenarios/terrarium.toml");
    let sets: Vec<String> = a.iter().filter_map(|x| x.strip_prefix("set=").map(str::to_string)).collect();
    let p = match kv(a, "overlay=") {
        Some(o) => cubarium_webmodel::web::start::params_with_overlay(Path::new(cfg), Path::new(o), &sets).unwrap(),
        None => config::resolve(Path::new(cfg), &sets).unwrap(),
    };
    let fc = config::flora_of(&p);
    let (world, flora) = census::read_save(save).unwrap_or_else(|e| panic!("{e}"));
    let minute: f64 = std::fs::read_to_string(save.join("minute")).unwrap().trim().parse().unwrap();
    let c = census::Census { width: world.config().width, depth: world.config().depth, minute, faces: census::census(&world, &flora, Some(&fc)) };
    let note = format!("save {} config {cfg}{} sets [{}]", save.display(), kv(a, "overlay=").map(|o| format!(" overlay {o}")).unwrap_or_default(), sets.join(" "));
    census::write_census(out, &c, &toml::to_string(&fc).unwrap(), &note).unwrap();
    eprintln!("{}: {} faces, minute {}", out.display(), c.faces.len(), c.minute);
}

/// One stand under held conditions, by the sim's own probe and by the model's law.
fn physio(a: &[String]) {
    use cubarium_voxel_flora::Species;
    use cubarium_voxel_flora::study::{ClimateSample, Held, Start, start_stand};
    use cubarium_webmodel::flora::{climate::DayModel, stand};
    let sp = Species::parse(&a[0]).unwrap();
    let f = |i: usize| a[i].parse::<f64>().unwrap();
    let (l, water, sat, n, days) = (f(1), f(2), f(3), f(4), f(5));
    let start = if a[6] == "adult" { Start::Adult } else { Start::Newborn };
    let sets: Vec<String> = a.iter().filter_map(|x| x.strip_prefix("set=")).map(String::from).collect();
    let p = config::resolve(Path::new("config/scenarios/terrarium.toml"), &sets).unwrap();
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
    let every: Option<f64> = a.iter().find_map(|x| x.strip_prefix("trace=")).map(|v| v.parse().unwrap());
    let qcap = sc.reserve_cap;
    let o = cubarium_voxel_flora::study::probe_traced(&fc, s0, &clim, |_| Held { water, saturated: sat, light: l, mineral: n }, |t, s| {
        if let Some(e) = every {
            let k = (e / cubarium_voxel::DT).round().max(1.0) as usize;
            if (t + 1) % k == 0 {
                let c = clim[t];
                println!("t {:>8.0} day {:.2} dl {:.2} W {:.4} P/aW {:.3} Q/qcap {:.3} cured {} dry_s {:.0}", (t + 1) as f64 * cubarium_voxel::DT, (t + 1) as f64 * cubarium_voxel::DT / dm.day_s(), c.daylight, s.wood, s.foliage / (sc.alpha * s.wood), s.reserve / (qcap * s.wood), s.cured, s.dry_s);
            }
        }
    });
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
    let cs = census::load(dir, &fc).unwrap();
    let faces = cs.faces;
    let zs = zones::build(&fc, &[faces.clone()]);
    let minute0 = cs.minute;
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
        let faces = census::load(Path::new(d), &fc).unwrap().faces;
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
    let mut minute0 = 0.0;
    for (i, d) in dirs.iter().enumerate() {
        let cs = census::load(d, &fc).unwrap_or_else(|e| panic!("{e}"));
        dims = (cs.width, cs.depth);
        if i == 0 {
            minute0 = cs.minute;
        }
        snaps.push(cs.faces);
    }
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
    // Meadow levers (`design/webmodel/meadow.md`), scenario levers of the model.
    {
        let day_s = DayModel::of(&p.world, fc.upkeep_q10).day_s();
        let mw = &mut mp.meadow;
        if let Some(x) = kv(a, "crop=") {
            mw.crop_per_graze = x.parse().unwrap();
        }
        if let Some(x) = kv(a, "cropw=") {
            mw.crop_wood_max = x.parse().unwrap();
        }
        if let Some(x) = kv(a, "dry=") {
            let v: Vec<f64> = x.split(':').map(|y| y.parse().unwrap()).collect();
            mw.dry_every_s = v[0] * day_s;
            mw.dry_len_s = v[1] * day_s;
            mw.dry_water = v[2];
            mw.dry_phase_s = v.get(3).copied().unwrap_or(0.0) * day_s;
        }
        if kv(a, "drywet=") == Some("1") {
            mw.dry_wet_zones = true;
        }
        if let Some(x) = kv(a, "thatch=") {
            mw.sward_germ = x.parse().unwrap();
        }
        if let Some(x) = kv(a, "contest=") {
            // `contest=sim`: the sim's CONTEST rule — escape when the crown leaves its
            // seedling cap (above the sward), the sward's shade by the shade law (both -1).
            let v: Vec<f64> = x.split(':').map(|y| if y == "sim" { -1.0 } else { y.parse().unwrap() }).collect();
            mw.contest_escape = v[0];
            mw.contest_tau = v.get(1).copied().unwrap_or(if v[0] < 0.0 { -1.0 } else { 0.0 });
        }
        if let Some(x) = kv(a, "draw=") {
            mw.sward_draw = x.parse().unwrap();
        }
        if let Some(x) = kv(a, "life=") {
            for part in x.split(',') {
                match part.split_once(':') {
                    Some((sp, d)) => mw.lifespan_s[Species::parse(sp).unwrap().index()] = d.parse::<f64>().unwrap() * day_s,
                    None => {
                        for s in 0..Species::COUNT {
                            if mw.woody[s] {
                                mw.lifespan_s[s] = part.parse::<f64>().unwrap() * day_s;
                            }
                        }
                    }
                }
            }
        }
        if let Some(x) = kv(a, "rain=") {
            // Lever g: seed rain into the open-sky drained soil zones, seeds per face per day.
            for part in x.split(',') {
                let (sp, r) = part.split_once(':').expect("rain=SPECIES:SEEDS_PER_FACE_DAY");
                mw.rain_per_face_s[Species::parse(sp).unwrap().index()] = r.parse::<f64>().unwrap() / day_s;
            }
        }
        if let Some(x) = kv(a, "soilmin=") {
            for part in x.split(',') {
                let (sp, d) = part.split_once(':').expect("soilmin=SPECIES:N");
                mw.root_soil_min[Species::parse(sp).unwrap().index()] = d.parse().unwrap();
            }
        }
    }
    let diag: Vec<Species> = kv(a, "diag=").map(|x| x.split(',').map(|n| Species::parse(n).expect("diag=SPECIES")).collect()).unwrap_or_default();
    mp.diag = !diag.is_empty();
    if let Some(name) = kv(a, "gates=") {
        // Per zone and snapshot: the share of its faces where the species' water gates, its
        // terrain-light gate, and both pass; the mean root-box water; the share at or above
        // its `establish_water_min`; and the share with banked seed of it.
        let sp = Species::parse(name).unwrap();
        let k = cubarium_webmodel::flora::census::PHOTO.iter().position(|&p| p == sp).unwrap();
        let wmin = fc.species(sp).establish_water_min;
        for (st, snap) in snaps.iter().enumerate() {
            let mut h = vec![[0.0f64; 7]; zs.zones.len()];
            for f in snap {
                let Some(&zi) = zs.of_face.get(&(f.x, f.y, f.z)) else { continue };
                let x = &mut h[zi];
                x[0] += 1.0;
                x[1] += f64::from(u8::from(f.water_ok[k]));
                x[2] += f64::from(u8::from(f.light_ok[k]));
                x[3] += f64::from(u8::from(f.water_ok[k] && f.light_ok[k]));
                x[4] += f.water[k].unwrap_or(0.0);
                x[5] += f64::from(u8::from(f.water[k].unwrap_or(0.0) >= wmin));
                x[6] += f64::from(u8::from(f.seeds[sp.index()] > 0));
            }
            for (zi, z) in zs.zones.iter().enumerate() {
                let n = h[zi][0].max(1.0);
                println!("gates,{},{st},{zi},{},faces={:.0},water_ok={:.2},light_ok={:.2},both={:.2},water_mean={:.3},ge_wmin={:.2},seeded={:.2}", sp.name(), z.key.name(), h[zi][0], h[zi][1] / n, h[zi][2] / n, h[zi][3] / n, h[zi][4] / n, h[zi][5] / n, h[zi][6] / n);
            }
        }
    }
    if kv(a, "soilhist=") == Some("1") {
        // Per zone: the share of faces with at least d soil voxels under them.
        let mut h = vec![[0.0f64; 9]; zs.zones.len()];
        for f in &snaps[0] {
            let zi = zs.of_face[&(f.x, f.y, f.z)];
            for (i, d) in [1u32, 2, 3, 4, 6, 8, 12, 16].iter().enumerate() {
                if f.soil_depth >= *d {
                    h[zi][i] += 1.0;
                }
            }
            h[zi][8] += 1.0;
        }
        for (zi, z) in zs.zones.iter().enumerate() {
            let n = h[zi][8].max(1.0);
            println!("soilhist,{zi},{},area={},ge1/2/3/4/6/8/12/16={}", z.key.name(), z.area, (0..8).map(|i| format!("{:.2}", h[zi][i] / n)).collect::<Vec<_>>().join("/"));
        }
    }
    let minutes: f64 = kv(a, "minutes=").map_or(1200.0, |x| x.parse().unwrap());
    let every: f64 = kv(a, "every=").map_or(30.0, |x| x.parse().unwrap());
    let mut m = model::Model::new(fc.clone(), DayModel::of(&p.world, fc.upkeep_q10), zs, kern, rb, &snaps[0], minute0, mp);
    // A recovery-from-rarity start (`rare=SPECIES:KEEP`): the species thinned to KEEP of its
    // stands and seeds at the start.
    // `rare=SPECIES:KEEP:AT_DAY` thins it at that model day instead: invasion when rare into
    // the community the run has built by then (the Chesson test on a late state).
    let mut rare_at: Option<(usize, f64, f64)> = None;
    if let Some(x) = kv(a, "rare=") {
        let v: Vec<&str> = x.split(':').collect();
        let (sp, keep) = (Species::parse(v[0]).unwrap().index(), v.get(1).expect("rare=SPECIES:KEEP").parse::<f64>().unwrap());
        match v.get(2) {
            Some(d) => rare_at = Some((sp, keep, minute0 + d.parse::<f64>().unwrap() * m.day.day_s() / 60.0)),
            None => m.make_rare(sp, keep),
        }
    }
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
    // The take past the log (`take_hold=1`): the mean of its last three windows, per 2 h.
    // A log with no flux windows (`take_const=FOLIAGE[:LITTER]`, per 2 h): that take
    // throughout, e.g. from the run's logged grazer count times a per-grazer take.
    let held_take: Option<(f64, f64, f64, f64)> = if clamp.len() >= 3 {
        let tail = &clamp[clamp.len() - 3..];
        let (mut f, mut l) = (0.0, 0.0);
        for w in tail {
            f += w.2 * 120.0 / (w.1 - w.0);
            l += w.3 * 120.0 / (w.1 - w.0);
        }
        Some((0.0, 0.0, f / 3.0, l / 3.0))
    } else {
        kv(a, "take_const=").map(|x| {
            let v: Vec<f64> = x.split(':').map(|y| y.parse().unwrap()).collect();
            (0.0, 0.0, v[0], v.get(1).copied().unwrap_or(0.0))
        })
    };
    let mut was_dry = false;
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
            for sp in &diag {
                zdiag_lines(&m, *sp);
            }
            next += every;
        }
        if m.minute() >= end - 1e-9 {
            break;
        }
        if let Some((sp, keep, at)) = rare_at
            && m.minute() + 1e-9 >= at
        {
            m.make_rare(sp, keep);
            println!("rare,{:.0},{},keep={keep}", m.minute(), Species::ALL[sp].name());
            rare_at = None;
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
        } else if let (Some("1"), Some(&(_, _, fol, lit))) = (kv(a, "take_hold="), held_take.as_ref()) {
            // Past the log: the mean of its last three windows, spread as above.
            let per = m.p.dt / 7200.0;
            if m.cohorts() > 0 && (m.minute() as u64) % 10 == 0 {
                let sup = m.supply();
                let tot: f64 = sup.foliage.iter().enumerate().filter(|(k, _)| *k != Species::Glowcap.index()).map(|(_, v)| v).sum();
                for k in 0..Species::COUNT {
                    m.take.foliage[k] = if k == Species::Glowcap.index() || tot <= 0.0 { 0.0 } else { fol * per * sup.foliage[k] / tot };
                }
            }
            m.take.litter = lit * per;
        } else {
            m.take = Default::default();
        }
        // Meadow lever b: the spells' edges, with the turf and the woody juveniles then.
        let dry = (0..m.z.len()).any(|zi| m.dry_zone(zi));
        if dry != was_dry {
            let turf: f64 = m.z.iter().map(|z| z.count(Species::Springturf.index())).sum();
            let juv: f64 = m.z.iter().flat_map(|z| [Species::Bloomcrown, Species::Lanternberry, Species::Vaulttree].map(|sp| z.cohorts[sp.index()].iter().filter(|c| c.st.wood < m.config.species(sp).donor_min).map(|c| c.n).sum::<f64>())).sum();
            println!("dryedge,{:.0},{},turf={turf:.0},woody_juv={juv:.0}", m.minute(), if dry { "start" } else { "end" });
            was_dry = dry;
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
    println!("cropped {}", n(&c.cropped));
    println!("aged {}", n(&c.aged));
    println!("contested {}", n(&c.contested));
    println!("escaped {}", n(&c.escaped));
    println!("contest_died {}", n(&c.contest_died));
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

/// The diagnosis ledger of species `sp` per zone, cumulative since the start
/// (`model::ZoneDiag`): one `tz` line per zone with any of it, plus the zone's state now.
fn zdiag_lines(m: &cubarium_webmodel::flora::model::Model, sp: cubarium_voxel_flora::Species) {
    use cubarium_voxel_flora::Species;
    let k = sp.index();
    let sc = m.config.species(sp);
    let woody = [Species::Bloomcrown, Species::Lanternberry, Species::Vaulttree];
    for (zi, z) in m.zones.zones.iter().enumerate() {
        let d = &m.c.zdiag[zi];
        let zs = &m.z[zi];
        let held: f64 = Species::ALL.iter().filter(|s| **s != Species::Glowcap).map(|s| zs.count(s.index())).sum();
        let vacant = (z.area - held).max(0.0);
        let under: f64 = woody.iter().map(|w| zs.understorey(w.index())).sum();
        // Who took the vacant faces here (seed births on vacant, all species).
        let by: Vec<String> = Species::ALL.iter().filter(|s| d.born_vacant[s.index()] >= 0.5).map(|s| format!("{}:{:.0}", &s.name()[..4], d.born_vacant[s.index()])).collect();
        let occ: Vec<String> = Species::ALL.iter().filter(|s| **s != Species::Glowcap && zs.count(s.index()) >= 0.5).map(|s| format!("{}:{:.0}", &s.name()[..4], zs.count(s.index()))).collect();
        println!(
            "tz,{:.0},{},{zi},{},area={},vacant={:.1},open={:.1},under={:.1},donors={:.1},bank={:.1},packages={:.1},seeds_out={:.1},landed={:.1},landed_self={:.1},lost={:.1},bank_died={:.1},born_vacant={:.1},born_cover={:.1},born_runner={:.1},starved={:.1},drought={:.1},overtopped={:.1},displaced={:.1},under_starved={:.1},killed={:.1},f_time={:.0},f_vacant={:.0},f_seeded={:.0},f_gates={:.0},f_canopy={:.0},f_lottery={:.0},rained={:.1},vac_by=[{}],occ=[{}]",
            m.minute(),
            sp.name(),
            z.key.name(),
            z.area,
            vacant,
            zs.count(k),
            under,
            m.count_at_least(zi, k, sc.donor_min),
            zs.bank[k],
            d.packages[k],
            d.seeds_out[k],
            d.landed[k],
            d.landed_self[k],
            d.lost[k],
            d.bank_died[k],
            d.born_vacant[k],
            d.born_cover[k],
            d.born_runner[k],
            d.starved[k],
            d.drought[k],
            d.overtopped[k],
            d.displaced[k],
            d.under_starved[k],
            d.killed[k],
            d.f_time,
            d.f_vacant,
            d.f_seeded[k],
            d.f_gates[k],
            d.f_canopy[k],
            d.f_lottery[k],
            d.rained[k],
            by.join(" "),
            occ.join(" ")
        );
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
    // The meadow levers' readout: recruits standing in the turf now, and the counters.
    let woody = [Species::Bloomcrown, Species::Lanternberry, Species::Vaulttree];
    println!(
        "meadow,{:.0},dry={},in_turf={},contested={},escaped={},contest_died={},cropped={},aged={}",
        m.minute(),
        u8::from((0..m.z.len()).any(|zi| m.dry_zone(zi))),
        woody.iter().map(|s| format!("{:.0}", m.z.iter().map(|z| z.contested(s.index())).sum::<f64>())).collect::<Vec<_>>().join("/"),
        woody.iter().map(|s| format!("{:.0}", m.c.contested[s.index()])).collect::<Vec<_>>().join("/"),
        woody.iter().map(|s| format!("{:.0}", m.c.escaped[s.index()])).collect::<Vec<_>>().join("/"),
        woody.iter().map(|s| format!("{:.0}", m.c.contest_died[s.index()])).collect::<Vec<_>>().join("/"),
        woody.iter().map(|s| format!("{:.0}", m.c.cropped[s.index()])).collect::<Vec<_>>().join("/"),
        woody.iter().map(|s| format!("{:.0}", m.c.aged[s.index()])).collect::<Vec<_>>().join("/"),
    );
    // Lever f's understorey (CONTEST): turf standing under each woody canopy now, and the
    // turf's counters: gone under, starved there, left holding the face.
    let t = Species::Springturf.index();
    println!(
        "under,{:.0},turf_under={},turf_open={:.0},under_in={:.0},starved_under={:.0},under_freed={:.0}",
        m.minute(),
        woody.iter().map(|s| format!("{:.0}", m.z.iter().map(|z| z.understorey(s.index())).sum::<f64>())).collect::<Vec<_>>().join("/"),
        m.z.iter().map(|z| z.count(t)).sum::<f64>(),
        m.c.under_in[t],
        m.c.starved_under[t],
        m.c.under_freed[t],
    );
    // The sim's `sp` ledger line's counterpart, cumulative since the start.
    let c = &m.c;
    for sp in Species::ALL {
        let k = sp.index();
        let by: Vec<String> = Species::ALL.iter().filter(|n| c.pairs[k][n.index()] >= 0.5).map(|n| format!("{}={:.0}", n.name(), c.pairs[k][n.index()])).collect();
        println!(
            "spm,{:.0},{},landed={:.0},germinated={:.0},seeds_died={:.0},runner_births={:.0},deaths=[starved={:.0} drought={:.0} overtopped={:.0} displaced={:.0} browsed={:.0} cropped={:.0} aged={:.0}],overtopped_by=[{}],built={:.3},shed={:.3},cures={:.0},wakes={:.0},cured_now={:.0}",
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
            c.cropped[k],
            c.aged[k],
            by.join(" "),
            c.built[k],
            c.shed[k],
            c.cures[k],
            c.wakes[k],
            m.z.iter().map(|z| z.cohorts[k].iter().filter(|x| x.st.cured).map(|x| x.n).sum::<f64>()).sum::<f64>()
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
