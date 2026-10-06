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
//! ```
//!
//! Data (`data=DIR`, default the session scratchpad): `wmflora/ckpt/s{1,5,6}-m*` (soak saves),
//! `wml/f370/s*.csv` (the m370 animal censuses, printed with the soak build's own binary),
//! `soak2/v2/runs/plants-v2-s*.txt` (the soak's logs), `wmflora2/patch/` (patch saves),
//! `gen/` and `lj2/` (D1b and D2b).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cubarium_voxel_fauna::{Food, Founder};
use cubarium_voxel_flora::Species;
use cubarium_webmodel::fauna::{Cohort, FaunaModel};
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

/// **The terrarium at soak v2's start**: seed `seed`'s minute-370 save, its census.
fn terrarium(d: &Path, seed: u32, cfg: &Path, sets: &[String], wseed: u64) -> Web {
    let t = std::time::Instant::now();
    let p = start::params(cfg, sets).unwrap_or_else(|e| panic!("{e}"));
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

fn world_of(args: &[String]) -> Web {
    let d = data(args);
    let map = arg(args, "map=").or_else(|| arg(args, "base=")).unwrap_or("terrarium");
    let seed: u32 = arg(args, "seed=").map_or(5, |s| s.parse().unwrap());
    let wseed: u64 = arg(args, "wseed=").map_or(1, |s| s.parse().unwrap());
    let cfg = arg(args, "config=").map(PathBuf::from).unwrap_or_else(|| repo().join(format!("config/scenarios/{map}.toml")));
    let mut w = match map {
        "patch" => patch_world(&d, seed, &cfg, &sets(args), wseed),
        _ => terrarium(&d, seed, &cfg, &sets(args), wseed),
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

/// Replace the free lanternjaws with `n` adults at the arrival stores (riders and the bank kept).
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

/// Patch at minute 120 of a founding: the plant half on the patch save, the animals the
/// patch founders run to minute 120 on it (closed).
fn patch_world(d: &Path, seed: u32, cfg: &Path, sets: &[String], wseed: u64) -> Web {
    let p = start::params(cfg, sets).unwrap_or_else(|e| panic!("{e}"));
    let (s0, snaps) = patch_saves(d, seed);
    let flora = start::flora(&p, &s0, &snaps, wseed).unwrap_or_else(|e| panic!("{e}"));
    let mut fauna = FaunaModel::new(&p, start::closed_fits(), 1.0);
    for f in Founder::ALL {
        fauna.enabled[f.index()] = STEPPED.contains(&f);
    }
    let counts = p.fauna.founder_counts();
    for f in STEPPED {
        let n = counts[f.index()] as f64;
        if n > 0.0 && f != Founder::Lanternjaw {
            fauna.add(f, Cohort::founders(&p.fauna, f, n));
        }
    }
    fauna.t_s = flora.t_s;
    fauna.mineral = fauna.mineral_wanted();
    scale(&mut fauna, faces(&flora));
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
    v.push(format!("litter={:.0} caps={:.0} film={:.2} carrion={:.1} foliage={:.0}", o.litter, o.foliage[Species::Glowcap.index()], o.film, o.carrion, o.foliage.iter().enumerate().filter(|(k, _)| *k != Species::Glowcap.index()).map(|(_, v)| v).sum::<f64>()));
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
    while d < days - 1e-9 {
        d = (d + every).min(days);
        if d >= days - 1.0 - 1e-9 && last_day_start.minute < m0 + (days - 1.0) * dm - 1e-6 {
            w.run_to(m0 + (days - 1.0) * dm);
            last_day_start = w.snap();
        }
        w.run_to(m0 + d * dm);
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
            let mut tw = w0.twin(1000 + k as u64);
            tw.run_to(m0 + days * dm);
            for f in STEPPED {
                let c = tw.fauna.census(f);
                ends.entry(f.index()).or_default().push(c.n);
                if c.n + c.eggs + c.bank < 0.5 && w0.fauna.census(f).n > 0.0 {
                    gone[f.index()] += 1;
                }
            }
        }
        println!("stochastic twin ({twins} runs, plants mean field): P(lineage gone by day {days:.0}) and bodies at the end [min..max]");
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
            "balance" if p[2] == "all" => {
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
    let cfg = arg(args, "config=").map(PathBuf::from).unwrap_or_else(|| d.join("soak2/v2/config/scenarios/terrarium.toml"));
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
        let w0 = terrarium(&d, seed, &cfg, &sets(args), wseed);
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
                    "    {:<14} {:.0} / {:.1} | {:.0} / {:.1} | {:.0} / {:.1} | {:.0} / {:.1} | {:.0} / -",
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
