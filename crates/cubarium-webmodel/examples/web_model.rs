//! **The closed loop, run** (`design/webmodel/loop.md`).
//!
//! ```text
//! web_model refarea                                 the patch areas the responses were fitted on
//! web_model budget   [map=terrarium|patch] [seed=5] [days=30] [from=D] [to=E] [config=FILE] [set=P=V]..
//!                                                   the budget sheet over a window (default: day 0-1 and the last day)
//! web_model scenario [base=terrarium|patch] [seed=5] [days=30] [twins=10] [browse=PER_DAY] [lj=N] [config=FILE] [set=P=V]..
//!                                                   a knob or lever to N days: trajectory, end sheet, end state, twin risks
//! web_model soak     [seeds=5,6,1] [wseed=1]        closed-loop validation on soak v2 (m370 -> m1570)
//! web_model d2b      [gen=DIR] [lj=DIR]             closed-loop validation on D2b (patch-hunters, 1 vs 3 hunters)
//! web_model fitheld  [rounds=N] [check]             the held lineages' fits on soak v2 (animal half alone); `check` runs the recorded
//! web_model fitcarrion [rounds=N] [half=H] [a=A]    the shredder's carrion response, closed loop on soak v2 s5+s6
//! ```
//!
//! The soak runs read the soak's config under the soak build's behaviour (`soak_params`).
//!
//! Data (`data=DIR`, default the session scratchpad): `wmflora/ckpt/s{1,5,6}-m*` (soak saves),
//! `wml/f370/s*.csv` (the m370 animal censuses, printed with the soak build's own binary),
//! `soak2/v2/runs/plants-v2-s*.txt` (the soak's logs), `wmflora2/patch/` (patch saves),
//! `gen/` and `lj2/` (D1b and D2b).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cubarium_voxel_fauna::{Food, Founder};
use cubarium_voxel_flora::Species;
use cubarium_webmodel::fauna::fits::Response;
use cubarium_webmodel::fauna::{Cohort, FaunaModel, Fits, LineageLedger};
use cubarium_webmodel::web::sheet::{lineage_name, sheet};
use cubarium_webmodel::web::start::{self, FaunaCensus, STEPPED};
use cubarium_webmodel::web::{Snap, Web};

const DEFAULT_DATA: &str = "/tmp/claude-1000/-home-wrysk-wryskware-cubarium/0183970f-d99c-4145-9cdc-fd35b3ab6476/scratchpad";

fn arg<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(k))
}

fn sets(args: &[String]) -> Vec<String> {
    args.iter().filter_map(|a| a.strip_prefix("set=").map(str::to_string)).collect()
}

fn data(args: &[String]) -> PathBuf {
    PathBuf::from(arg(args, "data=").unwrap_or(DEFAULT_DATA))
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("fitheld") => fitheld(&args),
        Some("fitcarrion") => fitcarrion(&args),
        Some("refarea") => refarea(&args),
        Some("budget") => budget(&args),
        Some("scenario") => scenario(&args),
        Some("soak") => soak(&args),
        Some("d2b") => d2b(&args),
        _ => eprintln!("usage: web_model refarea | budget | scenario | soak | d2b (see the module doc)"),
    }
}

// ---------------------------------------------------------------------------------------
// Maps.

/// **The areas the animal half's responses were fitted on**: patch (D1b, the `patch`
/// scenario), the faces each food class stood on, m² (`web_model refarea`: the plant half on
/// the patch foundings f1 and f3 at minute 120, the mean; 2026-10-05). Litter and carrion are
/// the whole floor, so their ratio is the floor ratio.
/// In [`Food::ALL`] order: litter, caps, carrion, foliage, film, nectar, fruit (f1 / f3: litter
/// 72.5 / 75.9, caps 29.9 / 31.4, foliage 31.3 / 34.1, film 15.6 / 14.6; no nectar or fruit
/// stands yet, so those fall back to the floor ratio).
const PATCH_REF_AREA: [f64; Food::COUNT] = [74.19, 30.66, 74.19, 32.69, 15.09, 0.0, 0.0];
/// Patch's support faces (the floor the hunt's encounter rates and the nest crowding were
/// fitted on), from the same run (4,638 / 4,858).
const PATCH_FACES: f64 = 4748.0;

/// Seeds' saves: (start, the weather snapshots), the producer half's own sets (`long.sh`).
fn soak_saves(d: &Path, seed: u32) -> (PathBuf, Vec<PathBuf>) {
    let c = d.join("wmflora/ckpt");
    let snaps: &[&str] = match seed {
        5 => &["s5-m250", "s5-m310", "s5-m1460", "s5-m1520", "s5-m1570"],
        6 => &["s6-m250", "s6-m1460", "s6-m1570"],
        _ => &["s1-m980", "s1-m1100"],
    };
    (c.join(format!("s{seed}-m370")), snaps.iter().map(|s| c.join(s)).collect())
}

fn patch_saves(d: &Path, seed: u32) -> (PathBuf, Vec<PathBuf>) {
    let p = d.join("wmflora2/patch");
    let s = if seed == 3 { 3 } else { 1 };
    (p.join(format!("f{s}/m120")), [150, 180, 210, 240, 270, 300, 330, 360].iter().map(|m| p.join(format!("s{s}/m{m}"))).collect())
}

fn faces(w: &cubarium_webmodel::flora::model::Model) -> f64 {
    w.zones.zones.iter().map(|z| z.area).sum()
}

/// The animal half's scales for a world of `world_faces` support faces.
fn scale(fauna: &mut FaunaModel, world_faces: f64) {
    if PATCH_FACES > 0.0 {
        fauna.area_scale = world_faces / PATCH_FACES;
        fauna.ref_area = PATCH_REF_AREA;
    }
}

/// **The soak's parameters**: soak v2's own config (`soak2/v2/config/scenarios/terrarium.toml`)
/// under the soak build's behaviour (18ca6e81): the knobs added since at the values that
/// reproduce it, as `config/scenarios/hunters/main.toml` records them — the patrol off (the P1
/// crown-seeking ambush), the lanternjaw's and the Veilhound's prey lists of that build, the
/// lanternjaw's physiology of that day.
fn soak_params(d: &Path, sets: &[String]) -> cubarium_voxel_sim::params::Params {
    start::params_with_overlay(&d.join("soak2/v2/config/scenarios/terrarium.toml"), &repo().join("config/scenarios/hunters/main.toml"), sets).unwrap_or_else(|e| panic!("{e}"))
}

/// **The terrarium at soak v2's start**: seed `seed`'s minute-370 save, its census.
fn terrarium(d: &Path, seed: u32, p: &cubarium_voxel_sim::params::Params, wseed: u64) -> Web {
    let t = std::time::Instant::now();
    let p = p.clone();
    let (s0, snaps) = soak_saves(d, seed);
    let flora = start::flora(&p, &s0, &snaps, wseed).unwrap_or_else(|e| panic!("{e}"));
    let census = FaunaCensus::read(&d.join(format!("wml/f370/s{seed}.csv"))).unwrap_or_else(|e| panic!("{e}"));
    let mut fauna = start::fauna(&p, start::closed_fits(), &census, &STEPPED, 1.0);
    scale(&mut fauna, faces(&flora));
    eprintln!("terrarium s{seed}: setup {:.1} s, {} zones, {:.0} faces, area scale {:.2}", t.elapsed().as_secs_f64(), flora.z.len(), faces(&flora), fauna.area_scale);
    Web::new(flora, fauna)
}

// ---------------------------------------------------------------------------------------
// refarea

fn refarea(args: &[String]) {
    let d = data(args);
    let cfg = repo().join("config/scenarios/patch.toml");
    let p = start::params(&cfg, &sets(args)).unwrap();
    let mut acc = [0.0f64; Food::COUNT];
    let mut fc = 0.0;
    for seed in [1u32, 3] {
        let (s0, snaps) = patch_saves(&d, seed);
        let flora = start::flora(&p, &s0, &snaps, 1).unwrap();
        let o = flora.offer();
        let mut m = FaunaModel::new(&p, start::closed_fits(), 1.0);
        m.enabled = [false; Founder::COUNT];
        let a = cubarium_webmodel::fauna::model::areas_of(&o);
        println!("patch f{seed} m{:.0}: faces {:.0}, areas {}", flora.minute(), faces(&flora), Food::ALL.iter().map(|f| format!("{:?}={:.2}", f, a[f.index()])).collect::<Vec<_>>().join(" "));
        println!("  stocks: litter {:.1} caps {:.1} film {:.2} foliage {:.1} carrion {:.2}", o.litter, o.foliage[Species::Glowcap.index()], o.film, o.foliage.iter().enumerate().filter(|(k, _)| *k != Species::Glowcap.index()).map(|(_, v)| v).sum::<f64>(), o.carrion);
        for k in 0..Food::COUNT {
            acc[k] += a[k] / 2.0;
        }
        fc += faces(&flora) / 2.0;
    }
    println!("PATCH_AREA = {:?}; PATCH_FACES = {fc:.1}", acc);
}

// ---------------------------------------------------------------------------------------
// budget and scenario

/// **The gut-throughput knob rows** (`gut=LINEAGE:S,..`): the animal half's own Q1 knob
/// (`fauna_model`'s `knob_sets`): every diet food's `digestion_s` over `S` and
/// `bite_per_body_s` times `S`, as `path=value` sets on the resolved config.
fn gut_sets(cfg: &Path, base: &[String], spec: &str) -> Vec<String> {
    let p = start::params(cfg, base).unwrap_or_else(|e| panic!("{e}"));
    let mut v = Vec::new();
    for part in spec.split(',') {
        let (name, x) = part.split_once(':').expect("gut=LINEAGE:SCALE");
        let s: f64 = x.parse().unwrap();
        let f = *Founder::ALL.iter().find(|f| f.name() == name).unwrap_or_else(|| panic!("no lineage {name}"));
        let ph = p.fauna.founder(f);
        let pre = format!("fauna.founders.{name}");
        for g in ph.diet.foods() {
            v.push(format!("{pre}.gut.digestion_s.{}={}", g.name(), ph.gut.digestion_s[g.index()] / s));
        }
        v.push(format!("{pre}.gut.bite_per_body_s={}", ph.gut.bite_per_body_s * s));
    }
    v
}

fn world_of(args: &[String]) -> Web {
    let d = data(args);
    let map = arg(args, "map=").or_else(|| arg(args, "base=")).unwrap_or("terrarium");
    let seed: u32 = arg(args, "seed=").map_or(5, |s| s.parse().unwrap());
    let wseed: u64 = arg(args, "wseed=").map_or(1, |s| s.parse().unwrap());
    let cfg = arg(args, "config=").map(PathBuf::from).unwrap_or_else(|| repo().join(format!("config/scenarios/{map}.toml")));
    let mut sets = sets(args);
    if let Some(g) = arg(args, "gut=") {
        let extra = gut_sets(&cfg, &sets, g);
        eprintln!("gut sets: {}", extra.join(" "));
        sets.extend(extra);
    }
    let mut w = match map {
        "patch" => patch_world(&d, seed, &cfg, &sets, wseed),
        _ => terrarium(&d, seed, &start::params(&cfg, &sets).unwrap_or_else(|e| panic!("{e}")), wseed),
    };
    if let Some(b) = arg(args, "browse=") {
        let per_day: f64 = b.parse().unwrap();
        for sp in [Species::Bloomcrown, Species::Lanternberry, Species::Vaulttree] {
            w.browse[sp.index()] = per_day / w.day_s();
        }
    }
    if let Some(n) = arg(args, "lj=") {
        set_hunters(&mut w, n.parse().unwrap());
    }
    if let Some(only) = arg(args, "only=") {
        let keep: Vec<&str> = only.split(',').collect();
        for f in Founder::ALL {
            w.fauna.enabled[f.index()] = w.fauna.enabled[f.index()] && keep.contains(&f.name());
        }
    }
    w
}

/// Replace the free lanternjaws with `n` adults at the arrival stores (riders and the bank kept;
/// on patch, as `patch-hunters` introduces them, at minute 120).
fn set_hunters(w: &mut Web, n: f64) {
    let lj = Founder::Lanternjaw;
    w.fauna.pops[lj.index()].retain(|c| c.host.is_some());
    if n > 0.0 {
        let c = Cohort::founders(&w.fauna.config, lj, n);
        w.fauna.add(lj, c);
    }
    w.fauna.mineral = w.fauna.mineral_wanted();
    *w = Web::new(w.flora.clone(), w.fauna.clone());
}

/// Patch at minute 120 of a founding: the plant half on the patch save (f1, or f3 for seed
/// 3), the patch founders run from minute 0 on its minute-120 offer (the animal half alone,
/// as D2b's start), then joined. The lanternjaws come with `lj=` (none on `patch`).
fn patch_world(d: &Path, seed: u32, cfg: &Path, sets: &[String], wseed: u64) -> Web {
    let p = start::params(cfg, sets).unwrap_or_else(|e| panic!("{e}"));
    let (s0, snaps) = patch_saves(d, seed);
    let flora = start::flora(&p, &s0, &snaps, wseed).unwrap_or_else(|e| panic!("{e}"));
    let mut fauna = FaunaModel::new(&p, start::closed_fits(), 1.0);
    for f in Founder::ALL {
        fauna.enabled[f.index()] = STEPPED.contains(&f);
    }
    scale(&mut fauna, faces(&flora));
    // Every founder of the scenario's table but the hunters (the lanternjaws come with `lj=`).
    let counts = p.fauna.founder_counts();
    for f in Founder::ALL {
        let n = counts[f.index()] as f64;
        if n > 0.0 && f != Founder::Lanternjaw {
            fauna.add(f, Cohort::founders(&p.fauna, f, n));
        }
    }
    let offer = flora.offer();
    while fauna.t_s < flora.t_s - 1e-9 {
        fauna.step(&offer);
    }
    fauna.t_s = flora.t_s;
    fauna.mineral = fauna.mineral_wanted();
    Web::new(flora, fauna)
}

fn census_line(w: &Web) -> String {
    let st = w.flora.stands();
    let o = w.flora.offer();
    let mut v = vec![format!("m{:.0}", w.minute())];
    for f in Founder::ALL {
        let c = w.fauna.census(f);
        if c.n > 0.0 || c.bank > 0.0 {
            v.push(format!("{}={:.1}", lineage_name(f), c.n));
        }
    }
    v.push(format!("litter={:.0} caps={:.0} film={:.2} nectar={:.2} fruit={:.1} carrion={:.1} foliage={:.0}", o.litter, o.foliage[Species::Glowcap.index()], o.film, o.nectar, o.fruit, o.carrion, o.foliage.iter().enumerate().filter(|(k, _)| *k != Species::Glowcap.index()).map(|(_, v)| v).sum::<f64>()));
    for sp in Species::ALL {
        if st[sp.index()].0 >= 0.5 {
            v.push(format!("{}={:.0}", sp.name(), st[sp.index()].0));
        }
    }
    v.join(" ")
}

fn budget(args: &[String]) {
    let mut w = world_of(args);
    let days: f64 = arg(args, "days=").map_or(30.0, |s| s.parse().unwrap());
    let from: f64 = arg(args, "from=").map_or(days - 1.0, |s| s.parse().unwrap());
    let to: f64 = arg(args, "to=").map_or(days, |s| s.parse().unwrap());
    let m0 = w.minute();
    let dm = w.day_s() / 60.0;
    let s0 = w.snap();
    w.run_to(m0 + dm);
    let s1 = w.snap();
    println!("{}", sheet(&s0, &s1, w.day_s()));
    w.run_to(m0 + from * dm);
    let a = w.snap();
    w.run_to(m0 + to * dm);
    let b = w.snap();
    println!("{}", sheet(&a, &b, w.day_s()));
}

fn scenario(args: &[String]) {
    let t = std::time::Instant::now();
    let w0 = world_of(args);
    let days: f64 = arg(args, "days=").map_or(30.0, |s| s.parse().unwrap());
    let twins: usize = arg(args, "twins=").map_or(10, |s| s.parse().unwrap());
    let every: f64 = arg(args, "every=").map_or(2.0, |s| s.parse().unwrap());
    let dm = w0.day_s() / 60.0;
    let m0 = w0.minute();
    let mut w = w0.clone();
    let a = w.snap();
    let mut d = 0.0;
    println!("trajectory (mean field), every {every} world days:");
    println!("  day 0 {}", census_line(&w));
    let mut last_day_start = a.clone();
    let mut offers = Vec::new();
    while d < days - 1e-9 {
        d = (d + every).min(days);
        if d >= days - 1.0 - 1e-9 && last_day_start.minute < m0 + (days - 1.0) * dm - 1e-6 {
            w.run_to_recording(m0 + (days - 1.0) * dm, &mut offers);
            last_day_start = w.snap();
        }
        w.run_to_recording(m0 + d * dm, &mut offers);
        println!("  day {d:.0} {}", census_line(&w));
    }
    let b = w.snap();
    println!("\nmean-field run {:.1} s", t.elapsed().as_secs_f64());
    println!("{}", sheet(&last_day_start, &b, w.day_s()));
    println!("{}", sheet(&a, &b, w.day_s()));
    if twins > 0 {
        let mut gone = [0usize; Founder::COUNT];
        let mut ends: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
        for k in 0..twins {
            let tw = w0.twin_replay(1000 + k as u64, &offers);
            for f in STEPPED {
                let c = tw.census(f);
                ends.entry(f.index()).or_default().push(c.n);
                if c.n + c.eggs + c.bank < 0.5 && w0.fauna.census(f).n > 0.0 {
                    gone[f.index()] += 1;
                }
            }
        }
        println!("stochastic twin ({twins} runs on the mean field's supply): P(lineage gone by day {days:.0}) and bodies at the end [min..max]");
        for f in STEPPED {
            let v = &ends[&f.index()];
            let (lo, hi) = (v.iter().cloned().fold(f64::INFINITY, f64::min), v.iter().cloned().fold(0.0, f64::max));
            println!("  {:<14} P(gone) {:.2}  end {:.0} [{lo:.0}..{hi:.0}]", lineage_name(f), gone[f.index()] as f64 / twins as f64, v.iter().sum::<f64>() / v.len() as f64);
        }
    }
    println!("total {:.1} s", t.elapsed().as_secs_f64());
}

// ---------------------------------------------------------------------------------------
// Validation.

/// The soak's log, as the sim printed it (`succession_study plants`, build 18ca6e81).
#[derive(Default)]
struct SoakLog {
    /// minute -> key -> value: `litter`, `edible`, `<species>` (stands), `<species>.ed`
    /// (edible), `capgnawer` (bodies).
    at: BTreeMap<u32, BTreeMap<String, f64>>,
    /// (from, to, foliage taken, litter taken) per flux window.
    flux: Vec<(f64, f64, f64, f64)>,
    /// The run's own books per lineage over the whole run (`balance,v2,all`): born, and deaths
    /// by cause.
    balance: BTreeMap<String, BTreeMap<String, f64>>,
    /// The same books per phase (`early` m370-970, `late` m970-1570, `all`) and lineage: `assim`,
    /// `body_days`, `born`, and deaths by cause.
    phase: BTreeMap<(String, String), BTreeMap<String, f64>>,
}

fn read_soak(path: &Path) -> SoakLog {
    let mut log = SoakLog::default();
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    for line in text.lines() {
        let p: Vec<&str> = line.split(',').collect();
        if p.len() < 4 {
            continue;
        }
        match p[0] {
            "world" => {
                let Ok(m) = p[2].parse::<u32>() else { continue };
                let e = log.at.entry(m).or_default();
                let (head, tail) = line.split_once(",stands/donors/edible: ").unwrap_or((line, ""));
                for kv in head.split(',') {
                    if let Some((k, v)) = kv.split_once('=') {
                        if let Ok(x) = v.parse::<f64>() {
                            e.insert(k.to_string(), x);
                        }
                    }
                }
                for kv in tail.split_whitespace() {
                    if let Some((k, v)) = kv.split_once('=') {
                        let a: Vec<f64> = v.split('/').filter_map(|x| x.parse().ok()).collect();
                        if a.len() == 3 {
                            e.insert(k.to_string(), a[0]);
                            e.insert(format!("{k}.ed"), a[2]);
                        }
                    }
                }
            }
            "capgnawer" => {
                let Ok(m) = p[2].parse::<u32>() else { continue };
                if let Some(n) = p[3].strip_prefix("n=").and_then(|x| x.parse::<f64>().ok()) {
                    log.at.entry(m).or_default().insert("a.capgnawer".into(), n);
                }
            }
            "bodies" => {
                let m = match (p[1], p[2]) {
                    ("mid", x) => 370 + x.parse::<u32>().unwrap_or(0),
                    (_, "end") => 1570,
                    _ => continue,
                };
                let e = log.at.entry(m).or_default();
                for kv in p[3].split_whitespace() {
                    if let Some((k, v)) = kv.split_once('=') {
                        let n: f64 = v.split('/').next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
                        e.insert(format!("a.{k}"), n);
                    }
                }
            }
            "stocks" => {
                let m = match (p[1], p[2]) {
                    ("mid", x) => 370 + x.parse::<u32>().unwrap_or(0),
                    (_, "end") => 1570,
                    _ => continue,
                };
                let e = log.at.entry(m).or_default();
                for kv in p[3].split_whitespace() {
                    if let Some((k, v)) = kv.split_once('=') {
                        if let Ok(x) = v.parse::<f64>() {
                            e.insert(format!("s.{k}"), x);
                        }
                    }
                }
            }
            "balance" => {
                let ph = log.phase.entry((p[2].to_string(), p[3].to_string())).or_default();
                for kv in line.split(',').skip(4) {
                    if let Some((k, v)) = kv.split_once('=') {
                        if let Ok(x) = v.parse::<f64>() {
                            ph.insert(k.to_string(), x);
                        }
                    }
                    if let Some(v) = kv.strip_prefix("deaths=[") {
                        for d in v.trim_end_matches(']').split_whitespace() {
                            if let Some((k, x)) = d.split_once('=') {
                                ph.insert(k.to_string(), x.parse().unwrap_or(0.0));
                            }
                        }
                    }
                    if let Some(v) = kv.strip_prefix("bites=[") {
                        for d in v.trim_end_matches(']').split_whitespace() {
                            if let Some((k, x)) = d.split_once('=') {
                                ph.insert(format!("bites.{k}"), x.parse().unwrap_or(0.0));
                            }
                        }
                    }
                }
                if p[2] != "all" {
                    continue;
                }
                let e = log.balance.entry(p[3].to_string()).or_default();
                for kv in line.split(',').skip(4) {
                    if let Some(v) = kv.strip_prefix("born=") {
                        e.insert("born".into(), v.parse().unwrap_or(0.0));
                    }
                    if let Some(v) = kv.strip_prefix("deaths=[") {
                        for d in v.trim_end_matches(']').split_whitespace() {
                            if let Some((k, x)) = d.split_once('=') {
                                e.insert(k.to_string(), x.parse().unwrap_or(0.0));
                            }
                        }
                    }
                }
            }
            "flux" => {
                let Some((a, b)) = p[2].split_once('-') else { continue };
                let get = |k: &str| p.iter().find_map(|x| x.split(" |").next()?.strip_prefix(k)).and_then(|v| v.parse::<f64>().ok());
                if let (Ok(a), Ok(b), Some(f), Some(l)) = (a.parse::<f64>(), b.parse::<f64>(), get("foliage_taken="), get("litter_taken=")) {
                    log.flux.push((a, b, f, l));
                }
            }
            _ => {}
        }
    }
    log
}

impl SoakLog {
    /// `key` at `minute`, linear between the logged minutes; `None` outside them.
    fn lerp(&self, key: &str, minute: f64) -> Option<f64> {
        let pts: Vec<(f64, f64)> = self.at.iter().filter_map(|(m, e)| e.get(key).map(|v| (f64::from(*m), *v))).collect();
        let first = pts.first()?;
        if minute <= first.0 {
            return Some(first.1);
        }
        let last = pts.last()?;
        if minute >= last.0 {
            return Some(last.1);
        }
        let k = pts.iter().position(|p| p.0 > minute)?;
        let (a, b) = (pts[k - 1], pts[k]);
        Some(a.1 + (b.1 - a.1) * (minute - a.0) / (b.0 - a.0))
    }
}

/// The readouts compared, model side: stands per species, the takeable foliage, litter (with
/// dead wood, as the soak's `litter` + `dead_wood`), caps, and bodies per lineage.
fn readout(w: &Web) -> BTreeMap<String, f64> {
    let mut r = BTreeMap::new();
    let st = w.flora.stands();
    let o = w.flora.offer();
    for sp in Species::ALL {
        r.insert(sp.name().to_string(), st[sp.index()].0);
    }
    r.insert("edible".into(), o.foliage.iter().sum());
    r.insert("litter".into(), o.litter);
    r.insert("glowcap.ed".into(), o.foliage[Species::Glowcap.index()]);
    for f in Founder::ALL {
        r.insert(format!("a.{}", f.name()), w.fauna.census(f).n);
    }
    r
}

/// **The plant half alone** on the soak (the producer half's open loop): its take clamped to
/// the log's flux windows, foliage over the species by edible stock, litter from the pools
/// (`flora_model run log=`). Stepped by itself; the animals frozen.
fn flora_open(w: &mut Web, log: &SoakLog) {
    let m = w.minute();
    let dt = w.flora.p.dt;
    if let Some(&(a0, b0, fol, lit)) = log.flux.iter().find(|x| m >= x.0 && m < x.1) {
        let per = dt / ((b0 - a0) * 60.0);
        let sup = w.flora.offer();
        let tot: f64 = sup.foliage.iter().enumerate().filter(|(k, _)| *k != Species::Glowcap.index()).map(|(_, v)| v).sum();
        for k in 0..Species::COUNT {
            w.flora.take.foliage[k] = if k == Species::Glowcap.index() || tot <= 0.0 { 0.0 } else { fol * per * sup.foliage[k] / tot };
        }
        w.flora.take.litter = lit * per;
    } else {
        w.flora.take = Default::default();
    }
    w.flora.step();
}

/// **The animal half alone** on the soak: its supply the log's stocks (litter,
/// the caps, the photosynthetic edible foliage), film the soak's mid-run stock, the areas the
/// closed loop's plant half offers at the start.
fn fauna_open(w: &mut Web, log: &SoakLog, areas: &cubarium_webmodel::exchange::Areas) {
    let m = w.minute();
    let mut s = cubarium_webmodel::exchange::Supply::default();
    s.litter = log.lerp("litter", m).unwrap_or(0.0);
    let caps = log.lerp("glowcap.ed", m).unwrap_or(0.0);
    s.foliage[Species::Glowcap.index()] = caps;
    s.foliage[Species::Springturf.index()] = (log.lerp("edible", m).unwrap_or(0.0) - caps).max(0.0);
    s.film = log.lerp("s.film", m).unwrap_or(0.0);
    s.nectar = log.lerp("s.nectar", m).unwrap_or(0.0);
    s.fruit = log.lerp("s.fruit", m).unwrap_or(0.0);
    s.area = areas.clone();
    s.area.foliage = [0.0; Species::COUNT];
    s.area.foliage[Species::Glowcap.index()] = areas.foliage[Species::Glowcap.index()];
    s.area.foliage[Species::Springturf.index()] = areas.foliage.iter().enumerate().filter(|(k, _)| *k != Species::Glowcap.index()).map(|(_, v)| v).sum();
    for _ in 0..w.sub {
        w.fauna.step(&s);
    }
    w.flora.t_s += w.flora.p.dt;
}

fn ratio_verdict(sim: f64, model: f64) -> String {
    if !(sim > 0.0) {
        return if model < 0.5 { "ok(0)".into() } else { format!("sim 0, model {model:.0}") };
    }
    let r = model / sim;
    let ok = (0.5..=2.0).contains(&r) || (model - sim).abs() <= 1.0;
    format!("x{r:.2}{}", if ok { "" } else { " FAIL" })
}

fn soak(args: &[String]) {
    let d = data(args);
    let p = soak_params(&d, &sets(args));
    let seeds: Vec<u32> = arg(args, "seeds=").unwrap_or("5,6,1").split(',').map(|x| x.parse().unwrap()).collect();
    let wseed: u64 = arg(args, "wseed=").map_or(1, |s| s.parse().unwrap());
    let keys: Vec<String> = ["springturf", "velvetpad", "lanternberry", "vaulttree", "stonecushion", "bloomcrown", "umbrellafrond", "siphonreed", "glowcap", "edible", "glowcap.ed", "litter"]
        .iter()
        .map(|s| s.to_string())
        .chain(Founder::ALL.iter().map(|f| format!("a.{}", f.name())))
        .collect();
    for seed in seeds {
        let log = read_soak(&d.join(format!("soak2/v2/runs/plants-v2-s{seed}.txt")));
        let end = *log.at.keys().max().unwrap();
        let mins: Vec<u32> = [610u32, 970, 1090, 1330, 1570].into_iter().filter(|m| *m <= end).collect();
        let t = std::time::Instant::now();
        let w0 = terrarium(&d, seed, &p, wseed);
        let areas = w0.flora.offer().area;
        let (mut closed, mut fo, mut fa) = (w0.clone(), w0.clone(), w0.clone());
        let mut rows: BTreeMap<(u32, String), (f64, f64, f64)> = BTreeMap::new();
        let mut traj = Vec::new();
        for &m in &mins {
            closed.run_to(f64::from(m));
            while fo.minute() < f64::from(m) - 1e-9 {
                flora_open(&mut fo, &log);
            }
            while fa.minute() < f64::from(m) - 1e-9 {
                fauna_open(&mut fa, &log, &areas);
            }
            let (rc, rf, ra) = (readout(&closed), readout(&fo), readout(&fa));
            for k in &keys {
                let open = if k.starts_with("a.") { ra[k] } else { rf[k] };
                rows.insert((m, k.clone()), (closed_val(&rc, k), open, f64::NAN));
            }
            traj.push(format!("  m{m}: {}", census_line(&closed)));
        }
        let cl = closed.closure();
        println!("=== soak v2 s{seed} (closed loop {:.1} s; organic residual {:.1e}, mineral residual {:.1e})", t.elapsed().as_secs_f64(), (cl.organic_held - cl.organic_expected) / cl.organic_held, (cl.mineral_held - cl.mineral_expected) / cl.mineral_held);
        for l in &traj {
            println!("{l}");
        }
        println!("  {:<22}{}", "stock", mins.iter().map(|m| format!(" | m{m}: sim / closed / open")).collect::<String>());
        for k in &keys {
            let held = k.strip_prefix("a.").is_some_and(|x| !STEPPED.iter().any(|f| f.name() == x));
            let mut line = format!("  {:<22}", k.strip_prefix("a.").map_or(k.clone(), |x| format!("{} ({})", if x == "chorister" { "veilhound" } else { x }, if held { "held" } else { "bodies" })));
            let mut any = false;
            for &m in &mins {
                let (c, o, _) = rows[&(m, k.clone())];
                let at_m = log.at.get(&m).and_then(|e| e.get(k)).copied();
                match at_m {
                    Some(sv) => {
                        any = true;
                        line += &format!(" | {sv:.0} / {c:.0} {} / {o:.0} {}", ratio_verdict(sv, c), ratio_verdict(sv, o));
                    }
                    None => line += " | -",
                }
            }
            if any {
                println!("{line}");
            }
        }
        // The run's flows by lineage over the whole run, where the log has its books.
        if !log.balance.is_empty() {
            println!("  flows m370-{end}, sim / closed: births | aged | starved | killed by the lanternjaw | killed by the Veilhound");
            for f in STEPPED {
                let b = log.balance.get(f.name());
                let g = |k: &str| b.and_then(|m| m.get(k)).copied().unwrap_or(0.0);
                let l = &closed.fauna.ledger[f.index()];
                println!(
                    "    {:<14} {:.0} / {:.1} | {:.0} / {:.1} | {:.0} / {:.1} | {:.0} / {:.1} | {:.0} / {:.1}",
                    lineage_name(f),
                    g("born"),
                    l.hatched + l.born,
                    g("aged"),
                    l.aged,
                    g("starved"),
                    l.starved,
                    g("killed_by_lanternjaw"),
                    l.killed_by[Founder::Lanternjaw.index()],
                    g("killed_by_chorister"),
                    l.killed_by[Founder::Chorister.index()],
                );
            }
        }
    }
}

fn closed_val(r: &BTreeMap<String, f64>, k: &str) -> f64 {
    r.get(k).copied().unwrap_or(f64::NAN)
}

/// A D2b log (`patch_hunters`): per minute, `prey` lines per lineage and the `lj` line.
#[derive(Default)]
struct D2bLog {
    prey: BTreeMap<(u32, String), BTreeMap<String, f64>>,
    lj: BTreeMap<u32, BTreeMap<String, f64>>,
}

fn read_d2b(path: &Path) -> D2bLog {
    let mut log = D2bLog::default();
    for line in std::fs::read_to_string(path).unwrap_or_default().lines() {
        let p: Vec<&str> = line.split(',').collect();
        if p.len() < 4 {
            continue;
        }
        let Ok(m) = p[2].parse::<u32>() else { continue };
        let (target, rest) = match p[0] {
            "prey" | "det" => (log.prey.entry((m, p[3].to_string())).or_default(), &p[4..]),
            "lj" => (log.lj.entry(m).or_default(), &p[3..]),
            _ => continue,
        };
        for kv in rest {
            if let Some((k, v)) = kv.split_once('=') {
                target.insert(k.to_string(), v.parse().unwrap_or(f64::NAN));
            }
        }
    }
    log
}

const SMALL: [Founder; 3] = [Founder::Blind, Founder::Capgnawer, Founder::RippleSnail];
const D2B_PREY: [Founder; 4] = [Founder::Blind, Founder::Capgnawer, Founder::RippleSnail, Founder::Browser];

/// **D2b's start, closed**: the plant half on the patch save at minute 120; the prey the
/// patch founders run from minute 0 on the plant half's minute-120 offer (the animal half
/// alone, as its own `d2b_start` runs them on D1b's stocks), then each lineage's adults and
/// juveniles scaled to the seed's logged census; the hunters, riders and bank from the log.
fn d2b_start(d: &Path, p: &cubarium_voxel_sim::params::Params, log: &D2bLog, seed: u32) -> Web {
    let (s0, snaps) = patch_saves(d, if seed % 2 == 1 { 1 } else { 3 });
    let flora = start::flora(p, &s0, &snaps, u64::from(seed)).unwrap_or_else(|e| panic!("{e}"));
    let mut m = FaunaModel::new(p, start::closed_fits(), 1.0);
    for f in Founder::ALL {
        m.enabled[f.index()] = STEPPED.contains(&f);
    }
    scale(&mut m, faces(&flora));
    let counts = p.fauna.founder_counts();
    for f in D2B_PREY {
        let n = counts[f.index()] as f64;
        if n > 0.0 {
            m.add(f, Cohort::founders(&p.fauna, f, n));
        }
    }
    let offer = flora.offer();
    while m.t_s < 120.0 * 60.0 - 1e-9 {
        m.step(&offer);
    }
    for f in D2B_PREY {
        let Some(e) = log.prey.get(&(120, f.name().to_string())) else { continue };
        let (n, adults) = (e.get("n").copied().unwrap_or(0.0), e.get("adults").copied().unwrap_or(0.0));
        let c = m.census(f);
        let sc = p.fauna.founder(f).core;
        let (ka, kj) = (if c.adults > 0.0 { adults / c.adults } else { 0.0 }, if c.juveniles > 0.0 { (n - adults) / c.juveniles } else { 0.0 });
        for k in m.pops[f.index()].iter_mut() {
            k.n *= if k.b.body >= sc.birth_body { ka } else { kj };
        }
        if c.adults <= 0.0 && adults > 0.0 {
            m.add(f, Cohort::founders(&p.fauna, f, adults));
        }
        if c.juveniles <= 0.0 && n - adults > 0.0 {
            let mut j = Cohort::new(n - adults, sc.body_min * 2.0, 0.0);
            j.native = true;
            m.add(f, j);
        }
    }
    m.eggs.clear();
    let lj = log.lj.get(&120);
    let get = |k: &str| lj.and_then(|e| e.get(k)).copied().unwrap_or(0.0);
    let lp = p.fauna.founder(Founder::Lanternjaw).core;
    if get("adults") > 0.0 {
        m.add(Founder::Lanternjaw, Cohort::founders(&p.fauna, Founder::Lanternjaw, get("adults")));
    }
    if get("riders") > 0.0 {
        let mut r = Cohort::new(get("riders"), lp.body_min, (lp.reproduction.egg_organic - lp.body_min).max(0.0));
        r.native = true;
        r.host = Some(Founder::Browser);
        m.add(Founder::Lanternjaw, r);
    }
    if get("bank") > 0.0 {
        m.eggs.push(cubarium_webmodel::fauna::EggCohort { lineage: Founder::Lanternjaw, n: get("bank"), organic: lp.reproduction.egg_organic, age_s: 0.0, dormant: true });
    }
    m.t_s = flora.t_s;
    m.mineral = m.mineral_wanted();
    Web::new(flora, m)
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if v.is_empty() { f64::NAN } else if v.len() % 2 == 1 { v[v.len() / 2] } else { 0.5 * (v[v.len() / 2 - 1] + v[v.len() / 2]) }
}

fn spread(v: &[f64]) -> String {
    let mut v = v.to_vec();
    let m = median(&mut v);
    format!("{m:.1} [{:.1}..{:.1}]", v.first().copied().unwrap_or(f64::NAN), v.last().copied().unwrap_or(f64::NAN))
}

/// **D2b closed**: the arms `d2b` (three hunters) and `d2b-1` (one), 8 seeds each, from minute
/// 120 to 240 (and the observation to 600 where logged), both halves free.
fn d2b(args: &[String]) {
    let d = data(args);
    let ljd = arg(args, "lj=").map(PathBuf::from).unwrap_or_else(|| d.join("lj2"));
    let p = start::params(&repo().join("config/scenarios/patch-hunters.toml"), &sets(args)).unwrap();
    let mut summary: BTreeMap<(String, u32, &str), (Vec<f64>, Vec<f64>)> = BTreeMap::new();
    for arm in ["d2b", "d2b-1"] {
        for seed in 1..=8u32 {
            let log = read_d2b(&ljd.join(format!("mech/{arm}_s{seed}.out")));
            let obs = read_d2b(&ljd.join(format!("mech/obs/{arm}_s{seed}.out")));
            if log.lj.is_empty() {
                continue;
            }
            let mut w = d2b_start(&d, &p, &log, seed);
            for m in [240u32, 360, 600] {
                let src = if m <= 240 { &log } else { &obs };
                if !src.lj.contains_key(&m) {
                    continue;
                }
                w.run_to(f64::from(m));
                let small_sim: f64 = SMALL.iter().map(|f| src.prey.get(&(m, f.name().to_string())).and_then(|e| e.get("n")).copied().unwrap_or(0.0)).sum();
                let small_model: f64 = SMALL.iter().map(|f| w.fauna.census(*f).n).sum();
                let kills_sim: f64 = [&log, &obs]
                    .iter()
                    .flat_map(|l| l.lj.iter())
                    .filter(|(mm, _)| **mm > 120 && **mm <= m)
                    .map(|(_, e)| e.get("kills").copied().unwrap_or(0.0).max(0.0))
                    .sum::<f64>();
                let kills_model = w.fauna.ledger[Founder::Lanternjaw.index()].kills;
                let fg_sim = src.prey.get(&(m, "frondgrazer".to_string())).and_then(|e| e.get("n")).copied().unwrap_or(f64::NAN);
                let lj_sim = src.lj.get(&m).and_then(|e| e.get("n")).copied().unwrap_or(f64::NAN);
                for (k, sv, mv) in [("small prey", small_sim, small_model), ("LJ kills", kills_sim, kills_model), ("frondgrazer", fg_sim, w.fauna.census(Founder::Browser).n), ("lanternjaw", lj_sim, w.fauna.census(Founder::Lanternjaw).n)] {
                    let e = summary.entry((arm.to_string(), m, k)).or_default();
                    e.0.push(sv);
                    e.1.push(mv);
                }
            }
            let cl = w.closure();
            eprintln!("{arm} s{seed}: organic residual {:.1e}, mineral {:.1e}; {}", (cl.organic_held - cl.organic_expected) / cl.organic_held, (cl.mineral_held - cl.mineral_expected) / cl.mineral_held, census_line(&w));
        }
    }
    println!("D2b closed loop (patch-hunters, both halves free), median [min..max] over seeds: sim | model");
    for ((arm, m, k), (sv, mv)) in &summary {
        let ok = {
            let (mut a, mut b) = (sv.clone(), mv.clone());
            let (x, y) = (median(&mut a), median(&mut b));
            if x > 0.0 { format!("x{:.2}", y / x) } else { String::new() }
        };
        println!("  {arm:<6} m{m:<4} {k:<12} sim {} | model {} {ok}", spread(sv), spread(mv));
    }
}

#[allow(dead_code)]
fn snap_line(s: &Snap) -> String {
    format!("m{:.0}", s.minute)
}

// ---------------------------------------------------------------------------------------
// The held lineages: fits on the soak's own books, the animal half alone (the plant stocks
// clamped to the log, `fauna_open`).

/// One soak seed, set up for the animal half alone.
struct OpenSeed {
    seed: u32,
    log: SoakLog,
    w0: Web,
    areas: cubarium_webmodel::exchange::Areas,
    end: u32,
}

/// What the open loop booked at each phase boundary (minutes 370, 970, 1570 as logged).
struct OpenOut {
    at: Vec<(u32, Vec<LineageLedger>, Vec<cubarium_webmodel::fauna::Census>)>,
}

fn open_seed(d: &Path, p: &cubarium_voxel_sim::params::Params, seed: u32) -> OpenSeed {
    let log = read_soak(&d.join(format!("soak2/v2/runs/plants-v2-s{seed}.txt")));
    let end = *log.at.keys().max().unwrap();
    let w0 = terrarium(d, seed, p, 1);
    let areas = w0.flora.offer().area;
    OpenSeed { seed, log, w0, areas, end }
}

fn open_run(o: &OpenSeed, fits: &Fits) -> OpenOut {
    open_run_clamped(o, fits, &[])
}

/// The logged count of lineage `f` at `minute`: the m370 census, then the `bodies` lines at
/// m970 and m1570, linear between.
fn logged_count(o: &OpenSeed, f: Founder, minute: f64) -> f64 {
    let mut pts = vec![(370.0, o.w0.fauna.census(f).n)];
    for m in [970u32, 1570] {
        if let Some(v) = o.log.at.get(&m).and_then(|e| e.get(&format!("a.{}", f.name()))) {
            pts.push((f64::from(m), *v));
        }
    }
    if minute <= pts[0].0 {
        return pts[0].1;
    }
    for w in pts.windows(2) {
        if minute <= w[1].0 {
            return w[0].1 + (w[1].1 - w[0].1) * (minute - w[0].0) / (w[1].0 - w[0].0);
        }
    }
    pts.last().unwrap().1
}

/// The open loop with the lineages in `clamp` held to their logged counts (each step's cohorts
/// rescaled after the step): a hunter's rates fitted on the prey the sim had, not on the
/// model's own prey trajectory.
fn open_run_clamped(o: &OpenSeed, fits: &Fits, clamp: &[Founder]) -> OpenOut {
    open_run_only(o, fits, clamp, None)
}

/// The open loop, optionally with one lineage alone stepped (`only`): a lineage no other one
/// eats or feeds (the soak build's bellwing) fits in a fraction of the time.
fn open_run_only(o: &OpenSeed, fits: &Fits, clamp: &[Founder], only: Option<Founder>) -> OpenOut {
    let mut w = o.w0.clone();
    w.fauna.fits = fits.clone();
    if let Some(f) = only {
        for g in Founder::ALL {
            w.fauna.enabled[g.index()] = g == f;
        }
    }
    let snap = |w: &Web| (w.fauna.ledger.to_vec(), Founder::ALL.iter().map(|f| w.fauna.census(*f)).collect::<Vec<_>>());
    let (l, c) = snap(&w);
    let mut at = vec![(370u32, l, c)];
    for m in [970u32, 1570] {
        if m > o.end {
            break;
        }
        while w.minute() < f64::from(m) - 1e-9 {
            fauna_open(&mut w, &o.log, &o.areas);
            for &f in clamp {
                let want = logged_count(o, f, w.minute());
                let have = w.fauna.census(f).n - w.fauna.census(f).riders;
                if have > 1e-9 {
                    for c in w.fauna.pops[f.index()].iter_mut().filter(|c| c.host.is_none()) {
                        c.n *= want / have;
                    }
                } else if want > 0.0 {
                    w.fauna.add(f, Cohort::founders(&w.fauna.config, f, want));
                }
            }
        }
        let (l, c) = snap(&w);
        at.push((m, l, c));
    }
    OpenOut { at }
}

/// A phase's books, model side: `(assimilated, body-days, born, aged, starved, killed by each
/// hunter)` of lineage `f` between boundaries `a` and `b`.
fn model_phase(o: &OpenOut, a: usize, b: usize, f: Founder, day_s: f64) -> BTreeMap<String, f64> {
    let mut r = BTreeMap::new();
    if b >= o.at.len() {
        return r;
    }
    let (x, y) = (&o.at[a].1[f.index()], &o.at[b].1[f.index()]);
    r.insert("assim".into(), y.flows.assimilated - x.flows.assimilated);
    r.insert("body_days".into(), (y.alive_s - x.alive_s) / day_s);
    r.insert("born".into(), y.hatched + y.born - x.hatched - x.born);
    r.insert("aged".into(), y.aged - x.aged);
    r.insert("starved".into(), y.starved - x.starved);
    for h in [Founder::Lanternjaw, Founder::Chorister] {
        r.insert(format!("killed_by_{}", h.name()), y.killed_by[h.index()] - x.killed_by[h.index()]);
    }
    r
}

fn sim_phase<'a>(log: &'a SoakLog, phase: &str, f: Founder) -> Option<&'a BTreeMap<String, f64>> {
    log.phase.get(&(phase.to_string(), f.name().to_string()))
}

const PHASES: [(&str, usize, usize); 2] = [("early", 0, 1), ("late", 1, 2)];

/// Sum of `key` over the phases and seeds given, sim side and model side.
fn sum_both(seeds: &[&OpenSeed], outs: &[OpenOut], f: Founder, key: &str, day_s: f64) -> (f64, f64) {
    let (mut s, mut m) = (0.0, 0.0);
    for (o, out) in seeds.iter().zip(outs) {
        for (ph, a, b) in PHASES {
            let Some(sv) = sim_phase(&o.log, ph, f) else { continue };
            let mv = model_phase(out, a, b, f, day_s);
            if mv.is_empty() {
                continue;
            }
            s += sv.get(key).copied().unwrap_or(0.0);
            m += mv.get(key).copied().unwrap_or(0.0);
        }
    }
    (s, m)
}

fn run_all(seeds: &[&OpenSeed], fits: &Fits) -> Vec<OpenOut> {
    run_all_clamped(seeds, fits, &[])
}

fn run_all_clamped(seeds: &[&OpenSeed], fits: &Fits, clamp: &[Founder]) -> Vec<OpenOut> {
    std::thread::scope(|sc| {
        let hs: Vec<_> = seeds.iter().map(|o| sc.spawn(move || open_run_clamped(o, fits, clamp))).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    })
}

/// **The held lineages' fits** (`web_model fitheld`): the Veilhound's kill rates and reach, the
/// bellwing's, loftstrider's and seedporter's food responses, by simulation on soak v2's own
/// books (`balance,v2,early|late`), the animal half alone with the plant stocks clamped to the
/// log. Training seeds 5 and 6; seed 1 is held out (its m970 census, and its books once logged).
/// Each round runs both seeds and scales each rate by sim / model on its target:
/// - each herbivore's `psi_max`: assimilation per body-day, pooled over the phases (the
///   loftstrider on seed 6 alone: seed 5 had one, for 0.7 body-days);
/// - the Veilhound's kill rate per prey: kills of that prey (the frondgrazer on seed 5 alone:
///   seed 6's packs killed none of the 97 at hand, a spatial refuge this rate cannot carry);
/// - its reach: its assimilation.
fn fitheld(args: &[String]) {
    let d = data(args);
    let p = soak_params(&d, &sets(args));
    let rounds: usize = arg(args, "rounds=").map_or(8, |s| s.parse().unwrap());
    let t = std::time::Instant::now();
    let train: Vec<OpenSeed> = [5u32, 6].iter().map(|&s| open_seed(&d, &p, s)).collect();
    let held = open_seed(&d, &p, 1);
    let day_s = train[0].w0.day_s();
    eprintln!("setup {:.1} s", t.elapsed().as_secs_f64());
    let mut fits = start::closed_fits();
    if args.iter().any(|a| a == "check") {
        held_check(&train.iter().chain(std::iter::once(&held)).collect::<Vec<_>>(), &fits, day_s);
        return;
    }
    start::held_guess(&mut fits);
    let vh = Founder::Chorister;
    let refs: Vec<&OpenSeed> = train.iter().collect();
    let s5: Vec<&OpenSeed> = train.iter().filter(|o| o.seed == 5).collect();
    let s6: Vec<&OpenSeed> = train.iter().filter(|o| o.seed == 6).collect();
    let herb = [(Founder::Bellwing, Food::Nectar, false), (Founder::Loftstrider, Food::Foliage, true), (Founder::Seedporter, Food::Fruit, false)];
    let prey = [Founder::Browser, Founder::Loftstrider, Founder::Lanternjaw, Founder::Seedporter];
    for round in 0..=rounds {
        let t = std::time::Instant::now();
        // The Veilhound's prey held to the log while its rates are fitted.
        let outs = run_all_clamped(&refs, &fits, &[Founder::Browser, Founder::Loftstrider, Founder::Lanternjaw, Founder::Seedporter]);
        let pick = |only6: bool, only5: bool| -> (Vec<&OpenSeed>, Vec<&OpenOut>) {
            let mut a = Vec::new();
            let mut b = Vec::new();
            for (o, out) in refs.iter().zip(&outs) {
                if (only6 && o.seed != 6) || (only5 && o.seed != 5) {
                    continue;
                }
                a.push(*o);
                b.push(out);
            }
            (a, b)
        };
        let mut line = format!("round {round} ({:.1} s):", t.elapsed().as_secs_f64());
        let sumk = |sel: (Vec<&OpenSeed>, Vec<&OpenOut>), f: Founder, key: &str| -> (f64, f64) {
            let outs: Vec<OpenOut> = sel.1.iter().map(|o| OpenOut { at: o.at.clone() }).collect();
            sum_both(&sel.0, &outs, f, key, day_s)
        };
        let mut upd: Vec<(String, f64)> = Vec::new();
        for (f, food, only6) in herb {
            let (sa, ma) = sumk(pick(only6, false), f, "assim");
            let (sb, mb) = sumk(pick(only6, false), f, "body_days");
            let (si, mi) = (sa / sb.max(1e-9), ma / mb.max(1e-9));
            let k = if mi > 0.0 { (si / mi).clamp(0.3, 3.0) } else { 3.0 };
            let r = fits.of(f).response[food.index()].unwrap_or(Response::holling(0.5, 0.0));
            line += &format!(" {} assim/bd sim {si:.4} model {mi:.4} psi_max {:.3};", lineage_name(f), r.psi_max);
            upd.push((format!("psi:{}", f.name()), k));
        }
        for f in prey {
            let only5 = f == Founder::Browser;
            let (sk, mk) = sumk(pick(false, only5), f, "killed_by_chorister");
            let k = if mk > 1e-6 { (sk / mk).clamp(0.2, 5.0) } else { 3.0 };
            let rate = fits.hunters[vh.index()].map_or(0.0, |h| h.kill[f.index()]);
            line += &format!(" VH kills {} sim {sk:.0} model {mk:.1} rate {rate:.2e};", lineage_name(f));
            upd.push((format!("kill:{}", f.name()), if sk > 0.0 { k } else { 0.0 }));
        }
        let (sa, ma) = sumk(pick(false, false), vh, "assim");
        let cap = fits.hunters[vh.index()].map_or(1.0, |h| h.meal_cap);
        line += &format!(" VH assim sim {sa:.2} model {ma:.2} meal cap {cap:.3}");
        upd.push(("meal".into(), if ma > 0.0 { (sa / ma).clamp(0.5, 2.0) } else { 2.0 }));
        println!("{line}");
        if round == rounds {
            break;
        }
        for (k, x) in upd {
            if let Some(name) = k.strip_prefix("psi:") {
                let (f, food, _) = herb.iter().find(|h| h.0.name() == name).copied().unwrap();
                let r = fits.of(f).response[food.index()].unwrap_or(Response::holling(0.5, 0.0));
                fits.of_mut(f).response[food.index()] = Some(Response { psi_max: (r.psi_max * x).min(20.0), ..r });
            } else if let Some(name) = k.strip_prefix("kill:") {
                let f = *Founder::ALL.iter().find(|g| g.name() == name).unwrap();
                if let Some(h) = fits.hunters[vh.index()].as_mut() {
                    h.kill[f.index()] *= x;
                }
            } else if let Some(h) = fits.hunters[vh.index()].as_mut() {
                h.meal_cap = (h.meal_cap * x).clamp(1e-3, 100.0);
            }
        }
    }
    fit_interference(&refs, &mut fits, day_s, Founder::Bellwing, Food::Nectar);
    let _ = (s5, s6);
    // The fitted values, to record.
    let h = fits.hunters[vh.index()].unwrap();
    println!(
        "fitted: veilhound kill {} meal cap {:.4} | {}",
        Founder::ALL.iter().filter(|f| h.kill[f.index()] > 0.0).map(|f| format!("{}={:.4e}", f.name(), h.kill[f.index()])).collect::<Vec<_>>().join(" "),
        h.meal_cap,
        herb.iter().map(|(f, food, _)| format!("{} {}: {:?}", f.name(), food.name(), fits.of(*f).response[food.index()])).collect::<Vec<_>>().join(" | ")
    );
    held_check(&train.iter().chain(std::iter::once(&held)).collect::<Vec<_>>(), &fits, day_s);
}

/// **The held check**: every lineage on every seed, the animal half alone — bodies at the
/// logged minutes, and per phase the books (assimilation per body-day, births, deaths by
/// cause), sim / model.
fn held_check(seeds: &[&OpenSeed], fits: &Fits, day_s: f64) {
    let outs = run_all(seeds, fits);
    for (o, out) in seeds.iter().zip(&outs) {
        println!("=== s{} (animal half alone, plant stocks clamped to the log)", o.seed);
        for f in Founder::ALL {
            let mut line = format!("  {:<14}", lineage_name(f));
            for (m, _, c) in &out.at {
                let sv = o.log.at.get(m).and_then(|e| e.get(&format!("a.{}", f.name()))).copied();
                match (sv, *m) {
                    (Some(sv), _) => line += &format!(" m{m} {sv:.0}/{:.1}", c[f.index()].n),
                    (None, 370) => line += &format!(" m370 -/{:.0}", c[f.index()].n),
                    _ => line += &format!(" m{m} -/{:.1}", c[f.index()].n),
                }
            }
            for (ph, a, b) in PHASES {
                let Some(sv) = sim_phase(&o.log, ph, f) else { continue };
                let mv = model_phase(out, a, b, f, day_s);
                if mv.is_empty() {
                    continue;
                }
                let g = |k: &str| sv.get(k).copied().unwrap_or(0.0);
                let h = |k: &str| mv.get(k).copied().unwrap_or(0.0);
                line += &format!(
                    " | {ph}: bd {:.0}/{:.0} assim/bd {:.3}/{:.3} born {:.0}/{:.1} aged {:.0}/{:.1} starved {:.0}/{:.1} LJ-killed {:.0}/{:.1} VH-killed {:.0}/{:.1}",
                    g("body_days"),
                    h("body_days"),
                    g("assim") / g("body_days").max(1e-9),
                    h("assim") / h("body_days").max(1e-9),
                    g("born"),
                    h("born"),
                    g("aged"),
                    h("aged"),
                    g("starved"),
                    h("starved"),
                    g("killed_by_lanternjaw"),
                    h("killed_by_lanternjaw"),
                    g("killed_by_chorister"),
                    h("killed_by_chorister")
                );
            }
            println!("{line}");
        }
    }
}

/// **A herbivore's response shape**, on a grid of Hill exponent × half-saturation ×
/// interference, its `psi_max` refitted by ratio at each point (three rounds), the lineage
/// stepped alone: the open loop clamps the food stock, so only the response's shape makes
/// intake follow the stock across seeds and fall as the lineage grows. Scored on the training
/// seeds' books: `ln` error of assimilation per body-day per phase, and of the counts at m970
/// and m1570.
fn fit_interference(seeds: &[&OpenSeed], fits: &mut Fits, day_s: f64, f: Founder, food: Food) {
    let base = fits.of(f).response[food.index()].unwrap_or(Response::holling(0.5, 0.0));
    let mut best = (f64::INFINITY, base);
    let mut line = String::new();
    let score = |cand: &Fits| -> (f64, f64, f64) {
        let outs: Vec<OpenOut> = std::thread::scope(|sc| {
            let hs: Vec<_> = seeds.iter().map(|o| sc.spawn(move || open_run_only(o, cand, &[], Some(f)))).collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let (mut sa, mut ma, mut sb, mut mb, mut err) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (o, out) in seeds.iter().zip(&outs) {
            for (ph, a, b) in PHASES {
                let (Some(sv), mv) = (sim_phase(&o.log, ph, f), model_phase(out, a, b, f, day_s)) else { continue };
                if mv.is_empty() {
                    continue;
                }
                let (x, y) = (sv.get("assim").copied().unwrap_or(0.0), mv["assim"]);
                let (bx, by) = (sv.get("body_days").copied().unwrap_or(0.0), mv["body_days"]);
                sa += x;
                ma += y;
                sb += bx;
                mb += by;
                err += ((y / by.max(1e-9) + 1e-4) / (x / bx.max(1e-9) + 1e-4)).ln().powi(2);
            }
            for (m, _, c) in out.at.iter().skip(1) {
                if let Some(sv) = o.log.at.get(m).and_then(|e| e.get(&format!("a.{}", f.name()))) {
                    err += ((c[f.index()].n + 1.0) / (sv + 1.0)).ln().powi(2);
                }
            }
        }
        let k = if ma > 0.0 { ((sa / sb.max(1e-9)) / (ma / mb.max(1e-9))).clamp(0.3, 3.0) } else { 3.0 };
        (err, k, 0.0)
    };
    for hill in [1.0, 2.0, 3.0, 4.0] {
        for half in [0.5, 1.0, 2.0, 3.0] {
            for ifr in [0.0, 0.01, 0.1] {
                let mut r = Response { interference: ifr, half: if hill == 1.0 && half > 0.5 { continue } else if hill == 1.0 { 0.0 } else { half }, hill, ..base };
                let mut err = 0.0;
                for round in 0..4 {
                    let mut cand = fits.clone();
                    cand.of_mut(f).response[food.index()] = Some(r);
                    let (e, k, _) = score(&cand);
                    err = e;
                    if round < 3 {
                        r.psi_max = (r.psi_max * k).min(50.0);
                    }
                }
                line += &format!(" q={hill},h={},i={ifr}:{err:.1}", r.half);
                if err < best.0 {
                    best = (err, r);
                }
            }
        }
    }
    fits.of_mut(f).response[food.index()] = Some(best.1);
    println!("fit,{},{} response shape (grid q,half,interference: error, psi_max refitted at each):{line}; best {:?} (error {:.2})", lineage_name(f), food.name(), best.1, best.0);
}

/// **The shredder's carrion response** (`web_model fitcarrion`): the sim's shredder takes
/// whatever is richest at its mouth (`mouth_meal`, body.rs:2649-2673: the stock compared, not
/// the return), so it eats carrion when its mouth is over a corpse face whose carrion beats
/// that face's litter — a corpse almost always does on the terrarium, whose litter is a tenth
/// of a unit a face. Its carrion intake is then the share of its feeding spent on corpse
/// faces: in proportion to the carrion on the floor while that is sparse (mass action,
/// `ψ_c = a · C / (C + half)` with the half far above the stocks met). `a` scaled until the
/// model's carrion share of the shredder's swallowed food, over m370-1570 on seeds 5 and 6
/// pooled, equals the sim's share of its bites (`balance,v2,all`; bites, not organic: a
/// bite off a corpse is fuller than one off thin litter, so the sim's organic share is if
/// anything higher). The closed loop, both halves free: the carrion pool is the model's.
fn fitcarrion(args: &[String]) {
    let d = data(args);
    let p = soak_params(&d, &sets(args));
    let rounds: usize = arg(args, "rounds=").map_or(5, |s| s.parse().unwrap());
    let half: f64 = arg(args, "half=").map_or(100.0, |s| s.parse().unwrap());
    let seeds = [5u32, 6];
    let ws: Vec<(SoakLog, Web)> = seeds.iter().map(|&s| (read_soak(&d.join(format!("soak2/v2/runs/plants-v2-s{s}.txt"))), terrarium(&d, s, &p, 1))).collect();
    let shr = Founder::Blind;
    let mut a = arg(args, "a=").map_or(0.05, |s| s.parse().unwrap());
    let sim_share = {
        let (mut c, mut t) = (0.0, 0.0);
        for (log, _) in &ws {
            let b = log.phase.get(&("all".to_string(), shr.name().to_string()));
            let g = |k: &str| b.and_then(|m| m.get(k)).copied().unwrap_or(0.0);
            c += g("bites.carrion");
            t += g("bites.carrion") + g("bites.litter");
        }
        c / t.max(1.0)
    };
    for round in 0..=rounds {
        let t = std::time::Instant::now();
        let outs: Vec<(f64, f64, f64, f64)> = std::thread::scope(|sc| {
            let hs: Vec<_> = ws
                .iter()
                .map(|(log, w0)| {
                    sc.spawn(move || {
                        let mut w = w0.clone();
                        w.fauna.fits.of_mut(shr).response[Food::Carrion.index()] = Some(Response::holling(a, half));
                        let end = f64::from(*log.at.keys().max().unwrap());
                        w.run_to(end.min(1570.0));
                        let l = &w.fauna.ledger[shr.index()];
                        (l.swallowed[Food::Carrion.index()], l.swallowed[Food::Litter.index()], w.flora.carrion, w.fauna.census(shr).n)
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let (c, l): (f64, f64) = outs.iter().fold((0.0, 0.0), |acc, o| (acc.0 + o.0, acc.1 + o.1));
        let share = c / (c + l).max(1e-12);
        println!(
            "round {round} ({:.1} s): a={a:.4e} (half {half}) carrion share of the shredder's food: model {share:.4} sim (bites) {sim_share:.4}; per seed (carrion, litter swallowed, carrion pool at end, shredders): {:?}",
            t.elapsed().as_secs_f64(),
            outs
        );
        if round < rounds {
            a *= if share > 0.0 { (sim_share / share).clamp(0.2, 5.0) } else { 5.0 };
        }
    }
}
