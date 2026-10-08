//! **The detritus web study** (package D1, `design/lanternjaw-detritus-web-proposal-2026-10-05.md`
//! §4 with `design/7_Research/astra-detritus-web-review-2026-10-05.md`): do the small
//! detritivores (littershredder, capgnawer, ripple-snail) breed and replace themselves, and
//! does their crowding brake bite before food runs short?
//!
//! ```text
//! detritus_study found seed=N save=DIR [minutes=120] [config=FILE] [set=P=V ...] [threads=T]
//! detritus_study run   load=DIR/mM arm=NAME hours=H [every=10] [config=FILE] [set=...] [run=DIR ...]
//! ```
//!
//! - `found`: the config's world from founding to `minutes` with its founding table of
//!   animals, saved at the end (`DIR/mM`): the shared start every arm branches from.
//! - `run`: branch a save under the resolved config — **both** layers' configs are
//!   replaced by the arm's and the heuristics reinstalled on the arm's teacher numbers —
//!   and run `hours`, staged with `run=DIR` (`cubarium_voxel_sim::staged`).
//!
//! Config layers as `succession_study`: code defaults, the file's `[world]`, `[flora]`,
//! `[fauna]` (default `config/scenarios/patch.toml`), then `set=` lines and a `sets=` file.
//!
//! **Readouts**, every `every` minutes, one `det,` line per detritivore lineage over the
//! window just ended: bodies, adults (free, at `birth_body`), juveniles, the minimum of both
//! so far, mean reserve fullness, the adults' mean brood (D1b: the share of the reserve cap
//! earmarked for a clutch) and body; intake (assimilated) and `g` (the net specific
//! growth, (assimilated − upkeep − motor) per body-day); the **awake share** (awake over
//! alive ticks), the **dark-awake share** (awake ticks owed to a dark face by day), access
//! (eating over wanting ticks), the **sated share** (awake ticks not wanting food: the gut
//! full), `B`; stay-on-food samples; clutches, eggs laid, hatched and
//! lost, the mean clutch; nest and ceiling refusals; **replacement** (in-world-born bodies
//! laying their first clutch) and their mean hatch-to-first-clutch minutes; maturation
//! (bodies growing through `birth_body`) and its mean age; recruitment (hatched) per adult
//! per world day; and the **local crowding** — the mean count of the lineage's own bodies
//! within its nest radius (1 m if it has none) of each adult. A `stocks,` line carries
//! litter, film, glowcap stands (dark and lit) and their cap tissue and mean fullness, the
//! glowcap spores landed so far, the photosynthetic stands' edible foliage and it plus the
//! litter, and ms/tick. At the end, a `sum,`
//! line per lineage over the whole run.

use std::path::Path;

use cubarium_search::es::voxel::live::balance;
use cubarium_voxel_fauna::{Fauna, FaunaLedger, Founder, TICK_HZ};
use cubarium_voxel_flora::Species;
use cubarium_voxel_sim::params::{Layers, Params};
use cubarium_voxel_sim::staged;
use cubarium_voxel_sim::{Sim, SimConfig};

const DETRITIVORES: [Founder; 3] = [Founder::Blind, Founder::Capgnawer, Founder::RippleSnail];

fn arg<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(k))
}

fn resolve(args: &[String]) -> Params {
    let path = arg(args, "config=").unwrap_or("config/scenarios/patch.toml");
    let cfg = cubarium::voxel::load_config(Path::new(path)).unwrap_or_else(|e| panic!("config {path}: {e}"));
    let mut file = toml::Table::new();
    if !cfg.sim.flora.is_empty() {
        file.insert("flora".into(), toml::Value::Table(cfg.sim.flora));
    }
    if !cfg.sim.fauna.is_empty() {
        file.insert("fauna".into(), toml::Value::Table(cfg.sim.fauna));
    }
    let mut sets: Vec<String> = args.iter().filter_map(|a| a.strip_prefix("set=").map(str::to_string)).collect();
    if let Some(f) = arg(args, "sets=") {
        let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("sets {f}: {e}"));
        sets.extend(text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_string));
    }
    for s in &sets {
        eprintln!("set {s}");
    }
    Params::resolve_over(Params { world: cfg.sim.world, ..Params::defaults() }, &Layers { file: Some(file), sets, ..Layers::default() })
        .unwrap_or_else(|e| panic!("the parameter set: {e}"))
}

const FOUNDING_ONLY: &[&str] = &["seed=", "minutes=", "save="];

fn main() {
    let raw: Vec<String> = std::env::args().collect();
    let args = staged::resume_args(&raw, FOUNDING_ONLY).unwrap_or_else(|e| panic!("{e}"));
    cubarium_voxel_sim::experiment_args(&args);
    if let Some(n) = arg(&args, "threads=").and_then(|s| s.parse().ok()) {
        cubarium_voxel::set_thread_override(n);
    }
    match args.get(1).map(String::as_str) {
        Some("found") => found(&args),
        Some("run") => run(&args),
        _ => panic!("usage: detritus_study found|run [...]"),
    }
}

fn stage(args: &[String], sim: &mut Sim, minute0: u64, end: u64) -> Option<staged::Runner> {
    let cfg = staged::StageConfig::from_args(args).unwrap_or_else(|e| panic!("{e}"))?;
    let r = staged::Runner::start(cfg, args, sim, minute0, end).unwrap_or_else(|e| {
        eprintln!("staged: the load check failed — {e}");
        std::process::exit(2)
    });
    eprintln!("staged run {} to minute {}", r.dir().display(), r.end_minute());
    Some(r)
}

fn stop_at(runner: &mut Option<staged::Runner>, sim: &Sim, minute: u64) -> bool {
    let Some(r) = runner.as_mut() else { return false };
    match r.minute(sim, minute) {
        None => false,
        Some(ended) => {
            eprintln!("staged: {ended:?} at minute {minute}");
            ended != staged::Ended::Done
        }
    }
}

fn found(args: &[String]) {
    let params = resolve(args);
    let seed: u64 = arg(args, "seed=").map_or(1, |s| s.parse().unwrap());
    let minutes: u64 = arg(args, "minutes=").map_or(120, |m| m.parse().unwrap());
    let dir = Path::new(arg(args, "save=").expect("save=DIR"));
    let wc = params.world.clone();
    let founded = cubarium::voxel::found(
        &wc,
        &cubarium::voxel::LayerConfig::new(&params, wc.voxel_m),
        params.fauna.founder_counts(),
        cubarium::voxel::Seeds::from(seed),
        false,
    );
    eprintln!("founded seed {} ({}): stands {}", founded.seed, if founded.accepted { "accepted" } else { "NOT accepted" }, founded.seeded.stands);
    println!("founded,{seed},{}", counts(&founded.fauna));
    let mut sim = Sim::ambient(founded.world, founded.flora, founded.fauna, SimConfig::experiment());
    let per_min = 60 * u64::from(TICK_HZ);
    let mut runner = stage(args, &mut sim, 0, minutes);
    for minute in 1..=minutes {
        for _ in 0..per_min {
            sim.step();
            if let Some(r) = runner.as_mut() {
                r.tick(&sim);
            }
        }
        if stop_at(&mut runner, &sim, minute) {
            return;
        }
    }
    let at = dir.join(format!("m{minutes}"));
    let (w, fl, fa) = sim.layers();
    cubarium_voxel_sim::saved::save(&at, w, fl, fa, minutes).unwrap_or_else(|e| panic!("save: {e}"));
    println!("saved,{seed},{},{}", at.display(), counts(sim.fauna()));
}

fn counts(fauna: &Fauna) -> String {
    let v = fauna.view();
    Founder::ALL
        .iter()
        .map(|f| format!("{}={}", f.name(), v.animals.iter().filter(|a| a.founder == Some(*f)).count()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Per-lineage state the windows integrate.
#[derive(Clone, Copy, Default)]
struct Track {
    min_n: Option<usize>,
    min_adults: Option<usize>,
    body_ticks: u64,
    adult_ticks: u64,
    body_sum_ticks: f64,
}

fn run(args: &[String]) {
    let hours: f64 = arg(args, "hours=").map_or(2.0, |h| h.parse().unwrap());
    let every: u64 = arg(args, "every=").map_or(10, |m| m.parse().unwrap());
    let arm = arg(args, "arm=").unwrap_or("arm").to_string();
    let params = resolve(args);
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let (world, mut flora, mut fauna, minute0) = (s.world, s.flora, s.fauna, s.minute);
    // The arm's numbers on both layers, and the heuristics rebuilt on its teachers.
    flora.replace_config(params.flora_for_world()).expect("the arm's plant config validates");
    fauna.replace_config(params.fauna.clone()).expect("the arm's animal config validates");
    cubarium_voxel_sim::saved::restore(&mut fauna, &[]).expect("the heuristics go back on");
    eprintln!("{arm}: loaded {} at minute {minute0}: {}", dir.display(), counts(&fauna));
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let per_min = 60 * u64::from(TICK_HZ);
    let mut runner = stage(args, &mut sim, minute0, minute0 + (hours * 60.0).round() as u64);
    let end = runner.as_ref().map_or(minute0 + (hours * 60.0).round() as u64, staged::Runner::end_minute);
    let mut track = [Track::default(); Founder::COUNT];
    let mut win = [Track::default(); Founder::COUNT];
    let l0 = sim.fauna().view().ledger.clone();
    let mut lw = l0.clone();
    let mut t0 = std::time::Instant::now();
    let mut ticks_in_window = 0u64;
    sample(&sim, &params, &arm, minute0, &lw, &lw, &win, &mut track, 0.0);
    for minute in minute0 + 1..=end {
        for _ in 0..per_min {
            sim.step();
            ticks_in_window += 1;
            if let Some(r) = runner.as_mut() {
                r.tick(&sim);
            }
            for a in sim.fauna().view().animals {
                let Some(f) = a.founder else { continue };
                let i = f.index();
                win[i].body_ticks += 1;
                win[i].body_sum_ticks += a.body;
                if balance::is_adult(a, &params.fauna) {
                    win[i].adult_ticks += 1;
                }
            }
        }
        if (minute - minute0) % every == 0 || minute == end {
            let ms = t0.elapsed().as_secs_f64() * 1000.0 / ticks_in_window.max(1) as f64;
            let l = sim.fauna().view().ledger.clone();
            sample(&sim, &params, &arm, minute, &lw, &l, &win, &mut track, ms);
            for i in 0..Founder::COUNT {
                track[i].body_ticks += win[i].body_ticks;
                track[i].adult_ticks += win[i].adult_ticks;
                track[i].body_sum_ticks += win[i].body_sum_ticks;
            }
            win = [Track::default(); Founder::COUNT];
            lw = l;
            t0 = std::time::Instant::now();
            ticks_in_window = 0;
        }
        if stop_at(&mut runner, &sim, minute) {
            break;
        }
    }
    let l = sim.fauna().view().ledger.clone();
    let hz = f64::from(TICK_HZ);
    for f in DETRITIVORES {
        let i = f.index();
        let t = &track[i];
        let line = window_line(&l0, &l, f, &params, t.adult_ticks, t.body_sum_ticks, hz);
        println!(
            "sum,{arm},{},min_n={},min_adults={},{line}",
            f.name(),
            t.min_n.unwrap_or(0),
            t.min_adults.unwrap_or(0),
        );
    }
}

/// The window's ledger measures of `f`, `a` → `b`, as `k=v` pairs.
#[allow(clippy::too_many_arguments)]
fn window_line(a: &FaunaLedger, b: &FaunaLedger, f: Founder, params: &Params, adult_ticks: u64, body_sum_ticks: f64, hz: f64) -> String {
    let i = f.index();
    let d = |x: &[u64; Founder::COUNT], y: &[u64; Founder::COUNT]| y[i].saturating_sub(x[i]);
    let df = |x: &[f64; Founder::COUNT], y: &[f64; Founder::COUNT]| y[i] - x[i];
    let day = params.fauna.life.ecological_day_s;
    let body_days = body_sum_ticks / hz / day;
    let adult_days = adult_ticks as f64 / hz / day;
    let assim = df(&a.assimilated_by_founder, &b.assimilated_by_founder);
    let upkeep = df(&a.upkeep_by_founder, &b.upkeep_by_founder);
    let motor = df(&a.motor_by_founder, &b.motor_by_founder);
    let alive = d(&a.alive_ticks_by_founder, &b.alive_ticks_by_founder);
    let awake = d(&a.awake_ticks_by_founder, &b.awake_ticks_by_founder);
    let dark = d(&a.dark_awake_ticks_by_founder, &b.dark_awake_ticks_by_founder);
    let wanting = d(&a.wanting_ticks_by_founder, &b.wanting_ticks_by_founder);
    let eating = d(&a.eating_ticks_by_founder, &b.eating_ticks_by_founder);
    let clutches = d(&a.clutches_by_founder, &b.clutches_by_founder);
    let laid = d(&a.eggs_laid_by_founder, &b.eggs_laid_by_founder);
    let hatched = d(&a.eggs_hatched_by_founder, &b.eggs_hatched_by_founder);
    let lost = d(&a.eggs_lost_by_founder, &b.eggs_lost_by_founder);
    let first = d(&a.first_clutches_by_founder, &b.first_clutches_by_founder);
    let first_age = d(&a.first_clutch_age_ticks_by_founder, &b.first_clutch_age_ticks_by_founder);
    let matured = d(&a.matured_by_founder, &b.matured_by_founder);
    let mat_age = d(&a.maturation_ticks_by_founder, &b.maturation_ticks_by_founder);
    let ratio = |x: f64, y: f64| if y > 0.0 { x / y } else { f64::NAN };
    let mins = |ticks: u64, n: u64| if n > 0 { ticks as f64 / n as f64 / hz / 60.0 } else { f64::NAN };
    format!(
        "body_days={body_days:.3},intake={assim:.5},g={:.4},B={:.3},awake={:.3},dark_awake={:.3},access={:.3},sated={:.3},still={},clutches={clutches},laid={laid},hatched={hatched},lost={lost},clutch_mean={:.2},nest_refusals={},ceiling_refusals={},replacement={first},first_clutch_min={:.1},matured={matured},maturation_min={:.1},recruits_per_adult_day={:.3}",
        ratio(assim - upkeep - motor, body_days),
        ratio(assim, upkeep + motor),
        ratio(awake as f64, alive as f64),
        ratio(dark as f64, awake as f64),
        ratio(eating as f64, wanting as f64),
        ratio(awake.saturating_sub(wanting) as f64, awake as f64),
        d(&a.feed_still_ticks_by_founder, &b.feed_still_ticks_by_founder),
        ratio(laid as f64, clutches as f64),
        d(&a.nest_refusals_by_founder, &b.nest_refusals_by_founder),
        d(&a.ceiling_refusals_by_founder, &b.ceiling_refusals_by_founder),
        mins(first_age, first),
        mins(mat_age, matured),
        ratio(hatched as f64, adult_days),
    )
}

#[allow(clippy::too_many_arguments)]
fn sample(sim: &Sim, params: &Params, arm: &str, minute: u64, a: &FaunaLedger, b: &FaunaLedger, win: &[Track; Founder::COUNT], track: &mut [Track; Founder::COUNT], ms: f64) {
    let v = sim.fauna().view();
    let ring = f64::from(sim.world().config().width) * sim.world().config().voxel_m;
    let hz = f64::from(TICK_HZ);
    for f in DETRITIVORES {
        let p = params.fauna.founder(f);
        let mine: Vec<_> = v.animals.iter().filter(|x| x.founder == Some(f)).collect();
        let adults: Vec<_> = mine.iter().filter(|x| balance::is_adult(x, &params.fauna)).collect();
        let n = mine.len();
        let t = &mut track[f.index()];
        t.min_n = Some(t.min_n.map_or(n, |m| m.min(n)));
        t.min_adults = Some(t.min_adults.map_or(adults.len(), |m| m.min(adults.len())));
        let r = if p.core.reproduction.nest_radius_m > 0.0 { p.core.reproduction.nest_radius_m } else { 1.0 };
        let crowd = if adults.is_empty() {
            f64::NAN
        } else {
            let mut sum = 0usize;
            for x in &adults {
                sum += mine
                    .iter()
                    .filter(|y| {
                        if y.id == x.id {
                            return false;
                        }
                        let mut dx = (y.pose.x - x.pose.x).rem_euclid(ring);
                        if dx > ring / 2.0 {
                            dx -= ring;
                        }
                        let dz = y.pose.z - x.pose.z;
                        dx * dx + dz * dz <= r * r
                    })
                    .count();
            }
            sum as f64 / adults.len() as f64
        };
        let fullness = if n > 0 {
            mine.iter().map(|x| x.reserve / p.core.reserve_of(x.body).max(1e-300)).sum::<f64>() / n as f64
        } else {
            f64::NAN
        };
        let body = if n > 0 { mine.iter().map(|x| x.body).sum::<f64>() / n as f64 } else { f64::NAN };
        // The brood (D1b): the mean share of the adults' reserve cap earmarked for a clutch.
        let brood = if adults.is_empty() {
            f64::NAN
        } else {
            adults.iter().map(|x| x.reproduction.brood_held(x.reserve) / p.core.reserve_of(x.body).max(1e-300)).sum::<f64>() / adults.len() as f64
        };
        let w = &win[f.index()];
        println!(
            "det,{arm},{minute},{},n={n},adults={},juveniles={},min_n={},min_adults={},fullness={fullness:.3},brood={brood:.3},body={body:.4},crowd={crowd:.2},{}",
            f.name(),
            adults.len(),
            n - adults.len(),
            t.min_n.unwrap_or(0),
            t.min_adults.unwrap_or(0),
            window_line(a, b, f, params, w.adult_ticks, w.body_sum_ticks, hz),
        );
    }
    let fv = sim.flora().view();
    let litter: f64 = fv.ground.iter().map(|g| g.litter).sum();
    let film: f64 = fv.film_sites().map(|(_, f)| f).sum();
    let glow: Vec<_> = fv.stands.iter().filter(|s| s.species == Species::Glowcap).collect();
    let caps: f64 = glow.iter().map(|s| s.foliage).sum();
    // The capgnawer package's readouts: glowcap on dark terrain (its own darkness above
    // zero) against lit, the mean cap fullness P / (alpha W), the spores landed so far, and
    // the photosynthetic stands' edible foliage with and without the litter.
    let gsc = fv.config.species(Species::Glowcap);
    let view = sim.world().view();
    let dark = glow
        .iter()
        .filter(|s| gsc.darkness(view.sky_visibility(i64::from(s.site.x), s.site.y, s.site.z)) > 0.0)
        .count();
    let fullness = if glow.is_empty() {
        f64::NAN
    } else {
        glow.iter().map(|s| (s.foliage / (gsc.alpha * s.wood).max(1e-300)).min(1.0)).sum::<f64>() / glow.len() as f64
    };
    let spores = fv.ledger.seeds_landed[Species::Glowcap.index()];
    let edible: f64 = fv
        .stands
        .iter()
        .filter(|s| fv.config.species(s.species).trophic == cubarium_voxel_flora::Trophic::Photo)
        .map(|s| fv.layers(s).map(|l| l.edible()).sum::<f64>())
        .sum();
    let others = Founder::ALL
        .iter()
        .filter(|f| !DETRITIVORES.contains(f))
        .map(|f| format!("{}={}", f.name(), v.animals.iter().filter(|x| x.founder == Some(*f)).count()))
        .collect::<Vec<_>>()
        .join(" ");
    println!(
        "stocks,{arm},{minute},litter={litter:.3},film={film:.4},glowcap_stands={},cap_tissue={caps:.4},glowcap_dark={dark},glowcap_lit={},cap_fullness={fullness:.3},glowcap_spores_landed={spores},edible={edible:.3},edible_plus_litter={:.3},bodies={},ms_tick={ms:.3},{others}",
        glow.len(),
        glow.len() - dark,
        edible + litter,
        v.animals.len()
    );
}
