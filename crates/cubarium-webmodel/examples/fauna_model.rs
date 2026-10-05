//! **The animal half of the whole-web model**, run (`design/webmodel/fauna.md`).
//!
//! ```text
//! fauna_model laws    [config=FILE]                   every founder's numbers as the model reads them
//! fauna_model young   [founder=littershredder] [hours=10] [set=P=V ...]
//!                     one hatchling at the gut ceiling (the young_budget probe's plain)
//! fauna_model fit     gen=DIR lj=DIR                  the fitted rates from the D1b and D2b logs
//! fauna_model d1b     gen=DIR [twin=N]                open-loop validation against D1b
//! fauna_model d2b     lj=DIR gen=DIR [twin=N]         open-loop validation against D2b
//! fauna_model answers gen=DIR lj=DIR                  generation time; hunter carrying capacity
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cubarium_voxel_fauna::{Food, Founder};
use cubarium_webmodel::exchange::Supply;
use cubarium_webmodel::fauna::{config, fits, laws, model, Cohort, Draws, FaunaModel, Fits, Rng};

fn arg<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(k))
}

fn sets(args: &[String]) -> Vec<String> {
    args.iter().filter_map(|a| a.strip_prefix("set=").map(str::to_string)).collect()
}

fn founder(name: &str) -> Founder {
    *Founder::ALL.iter().find(|f| f.name() == name).unwrap_or_else(|| panic!("no founder {name}"))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("laws") => laws_cmd(&args),
        Some("young") => young(&args),
        Some("fit") => fit_cmd(&args),
        Some("d1b") => d1b_cmd(&args),
        Some("d2b") => d2b_cmd(&args),
        Some("answers") => answers_cmd(&args),
        Some(other) => panic!("unknown command {other}"),
        None => panic!("usage: fauna_model laws|young|fit|d1b|d2b|answers"),
    }
}

/// Every founder's numbers, as the model reads them from the config.
fn laws_cmd(args: &[String]) {
    let cfg = arg(args, "config=").unwrap_or("config/scenarios/patch.toml");
    let p = config::scenario(cfg, &sets(args)).expect("config");
    for f in Founder::ALL {
        let ph = p.fauna.founder(f);
        let sc = ph.core;
        let r = sc.reproduction;
        let diet: Vec<&str> = ph.diet.foods().map(|x| x.name()).collect();
        let main_food = ph.diet.foods().next().unwrap_or(Food::Litter);
        let ceil_adult = laws::gut_ceiling_per_s(ph, sc.body_max, main_food) * 7200.0 / sc.body_max;
        let ceil_hatch = laws::gut_ceiling_per_s(ph, sc.body_min, main_food) * 7200.0 / sc.body_min;
        let need = sc.maintenance_per_s * 7200.0;
        println!(
            "{},active={:?},diet={diet:?},body_min={:.4},birth_body={:.4},body_max={:.4},reserve_cap={},growth_max_day={:.4},need_day={:.4},motor_day={:.4},ceiling_per_body_day(adult/hatchling,{})={:.3}/{:.3},mode={:?},egg={:.4},floor={:.4},clutch_max={},brood_share={},hold_s={:.0},interval_s={:.0},incubation_s={:.0},gestation_s={:.0},nest={}m/{},lifespan_h={:.1},founding={}",
            f.name(),
            ph.active,
            sc.body_min,
            sc.birth_body,
            sc.body_max,
            sc.reserve_cap,
            sc.growth_max_per_s * 7200.0,
            need,
            ph.motor_respiration_per_s * 7200.0,
            main_food.name(),
            ceil_adult,
            ceil_hatch,
            r.mode,
            r.egg_organic,
            r.surplus_floor,
            r.clutch_max,
            r.brood_share,
            r.surplus_hold_s,
            r.birth_interval_s,
            r.incubation_s,
            r.gestation_s,
            r.nest_radius_m,
            r.nest_crowd_max,
            sc.lifespan_median_s / 3600.0,
            ph.founding_count,
        );
    }
}

/// **The individual at its gut ceiling**: one hatchling, clock off, food always under its
/// mouth (ψ = 1), its eggs never hatching — the sim's `young_budget` probe in the model.
fn young(args: &[String]) {
    let f = founder(arg(args, "founder=").unwrap_or("littershredder"));
    let hours: f64 = arg(args, "hours=").map_or(10.0, |h| h.parse().unwrap());
    let cfg = arg(args, "config=").unwrap_or("config/scenarios/patch.toml");
    let mut p = config::scenario(cfg, &sets(args)).expect("config");
    p.world.day.mode = cubarium_voxel::weather::DayMode::Off;
    {
        let ph = &mut p.fauna.founders[f.index()];
        ph.core.reproduction.incubation_s = 1e12;
        ph.core.reproduction.nest_radius_m = 0.0;
        ph.core.lifespan_median_s = 0.0;
    }
    let mut m = FaunaModel::new(&p, Fits::unfitted(), 1.0);
    m.fits.of_mut(f).motor_effort = 0.0;
    for g in Founder::ALL {
        m.enabled[g.index()] = g == f;
    }
    let sc = p.fauna.founder(f).core;
    m.add(f, Cohort { native: true, ..Cohort::new(1.0, sc.body_min, (sc.reproduction.egg_organic - sc.body_min).max(0.0)) });
    let food = p.fauna.founder(f).diet.foods().next().unwrap();
    let mut supply = Supply::default();
    match food {
        Food::Litter => supply.litter = 1e9,
        Food::Film => supply.film = 1e9,
        Food::Carrion => supply.carrion = 1e9,
        _ => supply.foliage = [1e9; cubarium_voxel_flora::Species::COUNT],
    }
    let steps = (hours * 3600.0) as u64;
    let mut clutches = 0.0;
    for t in 1..=steps {
        m.step(&supply);
        let l = &m.ledger[f.index()];
        if l.clutches > clutches {
            clutches = l.clutches;
            let c = &m.pops[f.index()][0];
            println!("clutch,{clutches:.0},min={:.1},eggs={:.0},body={:.4}", t as f64 / 60.0, l.laid, c.b.body);
        }
        if t % 600 == 0 {
            let c = &m.pops[f.index()][0];
            let cap = sc.reserve_of(c.b.body);
            println!(
                "t,{:.0},body={:.4},reserve={:.5},fullness={:.3},brood={:.5},stomach={:.2},assim={:.5},upkeep={:.5},growth={:.5},eggs={:.5}",
                t as f64 / 60.0,
                c.b.body,
                c.b.reserve,
                c.b.reserve / cap,
                c.b.brood,
                c.b.stomach_total() / p.fauna.founder(f).gut.capacity(c.b.body),
                l.flows.assimilated,
                l.flows.upkeep,
                l.flows.growth,
                l.eggs_paid,
            );
        }
    }
    let l = &m.ledger[f.index()];
    println!(
        "milestones,maturation_min={:.1},first_clutch_min={:.1},clutches={:.0},eggs={:.0}",
        if l.matured > 0.0 { l.maturation_age_s / l.matured / 60.0 } else { f64::NAN },
        if l.first_clutches > 0.0 { l.first_clutch_age_s / l.first_clutches / 60.0 } else { f64::NAN },
        l.clutches,
        l.laid
    );
}

// ---------------------------------------------------------------------------------------
// The logs.

/// One run's log: the `det,` / `prey,` lines per (minute, lineage), the `lj,` and `stocks,`
/// lines per minute, each a map of its `key=value` fields.
#[derive(Default, Debug)]
struct RunLog {
    arm: String,
    seed: u32,
    det: BTreeMap<(u32, usize), BTreeMap<String, f64>>,
    lj: BTreeMap<u32, BTreeMap<String, f64>>,
    stocks: BTreeMap<u32, BTreeMap<String, f64>>,
}

fn kv_into(m: &mut BTreeMap<String, f64>, field: &str) {
    for part in field.split(' ') {
        if let Some((k, v)) = part.split_once('=') {
            m.insert(k.to_string(), v.parse().unwrap_or(f64::NAN));
        }
    }
}

fn read_log(path: &Path) -> RunLog {
    let name = path.file_stem().unwrap().to_string_lossy().to_string();
    let (arm, seed) = name.rsplit_once("_s").or_else(|| name.rsplit_once("-s")).unwrap();
    let mut log = RunLog { arm: arm.to_string(), seed: seed.parse().unwrap_or(0), ..RunLog::default() };
    let text = std::fs::read_to_string(path).unwrap_or_default();
    for line in text.lines() {
        let p: Vec<&str> = line.split(',').collect();
        if p.len() < 4 {
            continue;
        }
        let Ok(minute) = p[2].parse::<u32>() else { continue };
        match p[0] {
            "det" | "prey" => {
                let Some(f) = Founder::ALL.iter().find(|f| f.name() == p[3]) else { continue };
                let m = log.det.entry((minute, f.index())).or_default();
                for x in &p[4..] {
                    kv_into(m, x);
                }
            }
            "lj" => {
                let m = log.lj.entry(minute).or_default();
                for x in &p[3..] {
                    kv_into(m, x);
                }
            }
            "stocks" => {
                let m = log.stocks.entry(minute).or_default();
                for x in &p[3..] {
                    kv_into(m, x);
                }
            }
            _ => {}
        }
    }
    log
}

/// Every `*-sN.txt` / `*_sN.out` under `dir` whose arm is `arm`.
fn logs_of(dir: &Path, arm: &str) -> Vec<RunLog> {
    let mut v: Vec<RunLog> = std::fs::read_dir(dir)
        .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy().to_string();
            (n.ends_with(".txt") || n.ends_with(".out"))
                && (n.starts_with(&format!("{arm}-s")) || n.starts_with(&format!("{arm}_s")))
        })
        .map(|p| read_log(&p))
        .collect();
    v.sort_by_key(|l| l.seed);
    v
}

fn get(m: Option<&BTreeMap<String, f64>>, k: &str) -> f64 {
    m.and_then(|m| m.get(k)).copied().unwrap_or(f64::NAN)
}

fn median(v: &mut [f64]) -> f64 {
    let mut w: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if w.is_empty() {
        return f64::NAN;
    }
    w.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = w.len();
    if n % 2 == 1 { w[n / 2] } else { 0.5 * (w[n / 2 - 1] + w[n / 2]) }
}

fn quantile(v: &[f64], q: f64) -> f64 {
    let mut w: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if w.is_empty() {
        return f64::NAN;
    }
    w.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let i = ((w.len() - 1) as f64 * q).round() as usize;
    w[i]
}

// ---------------------------------------------------------------------------------------
// The fits.

/// **The gut's realized share of its ceiling at a bite share ψ** (`r(ψ)`): one hungry body of
/// `body`, clock off, food under its mouth at ψ, its assimilation over 30 min against the
/// full-stomach ceiling. The model's own gut, so the inversion r → ψ is the model's.
fn gut_r(p: &cubarium_voxel_sim::params::Params, f: Founder, food: Food, body: f64, psi: f64) -> f64 {
    let mut p = p.clone();
    p.world.day.mode = cubarium_voxel::weather::DayMode::Off;
    p.fauna.founders[f.index()].core.reproduction.incubation_s = 1e12;
    let mut fits = Fits::unfitted();
    fits.of_mut(f).response[food.index()] = Some(fits::Response::holling(psi, 0.0));
    for g in Food::ALL {
        if g != food {
            fits.of_mut(f).response[g.index()] = Some(fits::Response::holling(0.0, 0.0));
        }
    }
    fits.of_mut(f).motor_effort = 0.0;
    let mut m = FaunaModel::new(&p, fits, 1.0);
    for g in Founder::ALL {
        m.enabled[g.index()] = g == f;
    }
    let ph = *p.fauna.founder(f);
    let mut c = Cohort::new(1.0, body, 0.1 * ph.core.reserve_of(body));
    c.b.stomach[food.index()] = 0.9 * ph.gut.capacity(body);
    m.add(f, c);
    let mut s = Supply::default();
    match food {
        Food::Litter => s.litter = 1e9,
        Food::Film => s.film = 1e9,
        Food::CapTissue => s.foliage[cubarium_voxel_flora::Species::Glowcap.index()] = 1e9,
        Food::Carrion => s.carrion = 1e9,
        _ => s.foliage = [1e9; cubarium_voxel_flora::Species::COUNT],
    }
    for _ in 0..300 {
        m.step(&s);
    }
    let a0 = m.ledger[f.index()].flows.assimilated;
    let mut ceil = 0.0;
    for _ in 0..1800 {
        let b = m.pops[f.index()][0].b.body;
        ceil += laws::gut_ceiling_per_s(&ph, b, food);
        m.step(&s);
    }
    (m.ledger[f.index()].flows.assimilated - a0) / ceil
}

/// ψ for an observed `r` by inverting [`gut_r`] on a grid.
fn psi_of_r(table: &[(f64, f64)], r: f64) -> f64 {
    if r <= table[0].1 {
        return table[0].0;
    }
    for w in table.windows(2) {
        if r <= w[1].1 {
            let (p0, r0) = w[0];
            let (p1, r1) = w[1];
            return p0 + (p1 - p0) * (r - r0) / (r1 - r0).max(1e-12);
        }
    }
    table.last().unwrap().0
}

/// Least squares of `ψ = psi_max · E / (E + half)` over `(E, ψ)` points, by grid on `half`
/// (log-spaced) with `psi_max` closed-form. Returns (psi_max, half, rms).
fn fit_holling(pts: &[(f64, f64)]) -> (f64, f64, f64) {
    let mut best = (0.0, 0.0, f64::INFINITY);
    for k in 0..=200 {
        let half = if k == 0 { 0.0 } else { 10f64.powf(-3.0 + 7.0 * k as f64 / 200.0) };
        let g: Vec<f64> = pts.iter().map(|(e, _)| e / (e + half)).collect();
        let num: f64 = pts.iter().zip(&g).map(|((_, y), g)| y * g).sum();
        let den: f64 = g.iter().map(|g| g * g).sum();
        let a = (num / den.max(1e-300)).clamp(0.0, 1.0);
        let rms = (pts.iter().zip(&g).map(|((_, y), g)| (y - a * g).powi(2)).sum::<f64>() / pts.len() as f64).sqrt();
        if rms < best.2 {
            best = (a, half, rms);
        }
    }
    best
}

/// The D1b windows usable for fitting: every arm and seed, minute ≥ 20 (past the founders'
/// first clutch), with bodies awake.
fn d1b_logs(gdir: &Path) -> Vec<RunLog> {
    let mut v = Vec::new();
    for arm in ["branch", "rate", "main"] {
        v.extend(logs_of(&gdir.join("runs"), arm));
    }
    for arm in ["gut", "brood"] {
        v.extend(logs_of(&gdir.join("eid"), arm));
    }
    v
}

/// The arm's config, as the D1b and D2b launchers ran it.
fn arm_config(arm: &str) -> (&'static str, Vec<String>) {
    match arm {
        "branch" => ("config/scenarios/generation/branch.toml", vec![]),
        "rate" => ("config/scenarios/generation/rate.toml", vec![]),
        "gut" => ("config/scenarios/generation/gut.toml", vec![]),
        "brood" => ("config/scenarios/generation/brood.toml", vec![]),
        // D1b's `main` is main before D1b (c65800b9): no mass exponent, no brood, no full-clutch
        // wait for the shredder and snail (the capgnawer kept its own).
        "main" => (
            "config/scenarios/patch.toml",
            ["littershredder", "capgnawer", "ripple-snail"]
                .iter()
                .flat_map(|f| {
                    let mut v = vec![format!("fauna.founders.{f}.gut.mass_exponent=1.0"), format!("fauna.founders.{f}.core.reproduction.brood_share=0.0")];
                    if *f != "capgnawer" {
                        v.push(format!("fauna.founders.{f}.core.reproduction.clutch_hold_max_s=0.0"));
                    }
                    v
                })
                .collect(),
        ),
        "d2" => ("config/scenarios/hunters/d2.toml", vec![]),
        "d2b" | "d2b-1" => ("config/scenarios/patch-hunters.toml", vec![]),
        "d2b-open" => ("config/scenarios/patch-hunters.toml", vec!["fauna.hunt.strike_chosen_only=false".into()]),
        "d2-chosen" => ("config/scenarios/hunters/d2.toml", vec!["fauna.hunt.strike_chosen_only=true".into()]),
        "lj-main" => ("config/scenarios/hunters/main.toml", vec![]),
        _ => ("config/scenarios/patch.toml", vec![]),
    }
}

fn params_of(arm: &str) -> cubarium_voxel_sim::params::Params {
    let (c, s) = arm_config(arm);
    config::scenario(c, &s).unwrap_or_else(|e| panic!("{arm}: {e}"))
}

const DETRITIVORES: [Founder; 3] = [Founder::Blind, Founder::Capgnawer, Founder::RippleSnail];

/// The food a detritivore's response is fitted on: its one stocked food on patch.
fn fit_food(f: Founder) -> Food {
    match f {
        Founder::Capgnawer => Food::CapTissue,
        Founder::RippleSnail => Food::Film,
        _ => Food::Litter,
    }
}

fn stock_of(m: Option<&BTreeMap<String, f64>>, food: Food) -> f64 {
    match food {
        Food::Litter => get(m, "litter"),
        Food::Film => get(m, "film"),
        Food::CapTissue => get(m, "cap_tissue"),
        _ => f64::NAN,
    }
}

/// **Fit every behaviour-emergent rate** from the D1b and D2b logs, print each with its
/// source and spread, and return them.
fn fit_all(gdir: &Path, lj: Option<&Path>, quiet: bool) -> Fits {
    fit_all_with(gdir, lj, quiet, false)
}

fn fit_all_with(gdir: &Path, lj: Option<&Path>, quiet: bool, obs_only: bool) -> Fits {
    let logs = d1b_logs(gdir);
    let base = params_of("branch");
    let probe = FaunaModel::new(&base, Fits::unfitted(), 1.0);
    let mut fits = Fits::unfitted();
    macro_rules! say { ($($t:tt)*) => { if !quiet { println!($($t)*); } } }
    for f in DETRITIVORES {
        if obs_only {
            break;
        }
        let i = f.index();
        let ph = *base.fauna.founder(f);
        let mut r_pts: Vec<(f64, f64, f64)> = Vec::new(); // (stock, r, body)
        let mut q_dark = Vec::new();
        let mut effort = Vec::new();
        let mut crowd = Vec::new();
        let (mut laid, mut lost) = (0.0, 0.0);
        for log in &logs {
            let p = params_of(&log.arm);
            let pf = *p.fauna.founder(f);
            let mins: Vec<u32> = log.det.keys().filter(|(_, j)| *j == i).map(|(m, _)| *m).collect();
            for w in mins.windows(2) {
                let (m0, m1) = (w[0], w[1]);
                let d = log.det.get(&(m1, i));
                let n = get(d, "n");
                let bd = get(d, "body_days");
                let intake = get(d, "intake");
                let aw = get(d, "awake");
                let body = get(d, "body");
                laid += get(d, "laid").max(0.0);
                lost += get(d, "lost").max(0.0);
                let adults = get(d, "adults");
                let cr = get(d, "crowd");
                if n > 1.5 && adults > 0.0 && cr.is_finite() && m1 >= 20 {
                    crowd.push(cr / (n - 1.0));
                }
                if m1 < 20 || !(n > 0.0) || !(bd > 0.0) || !(aw > 0.05) || !body.is_finite() {
                    continue;
                }
                // Day share of the window, and the dark-awake share of daytime.
                let mut day = 0.0;
                let steps = 60;
                for k in 0..steps {
                    let t = (f64::from(m0) + (f64::from(m1 - m0)) * (k as f64 + 0.5) / steps as f64) * 60.0;
                    if probe.daylight_at(t) >= base.fauna.life.day_line {
                        day += 1.0 / steps as f64;
                    }
                }
                let da = get(d, "dark_awake");
                if day > 0.5 && da.is_finite() {
                    q_dark.push(da * aw / day);
                }
                // Motor effort: spend less the upkeep law, over the motor rate awake.
                let ratio = get(d, "B");
                if ratio.is_finite() && ratio > 0.0 {
                    let spend = intake / ratio;
                    let upkeep = pf.core.maintenance_per_s * 7200.0 * bd * (aw + (1.0 - aw) * pf.sleep_upkeep);
                    let motor = spend - upkeep;
                    effort.push(motor / (pf.motor_respiration_per_s * 7200.0 * bd * aw));
                }
                // Realized share of the gut ceiling while awake (the capgnawer's litter share
                // is taken off below).
                let food = fit_food(f);
                let ceil = laws::gut_ceiling_per_s(&pf, body, food) / body * 7200.0 * bd * aw;
                let s0 = stock_of(log.stocks.get(&m0), food);
                let s1 = stock_of(log.stocks.get(&m1), food);
                let stock = 0.5 * (s0 + s1);
                if stock.is_finite() && ceil > 0.0 {
                    let litter = 0.5 * (get(log.stocks.get(&m0), "litter") + get(log.stocks.get(&m1), "litter"));
                    r_pts.push((stock, intake / ceil, body + 0.0 * litter));
                }
            }
        }
        let lf = fits.of_mut(f);
        lf.day_dark_awake = median(&mut q_dark.clone()).clamp(0.0, 1.0);
        lf.motor_effort = median(&mut effort.clone()).max(0.0);
        lf.crowd_per_body = median(&mut crowd.clone()).max(0.0);
        lf.egg_loss = if laid > 0.0 { lost / laid } else { 0.0 };
        say!(
            "fit,{},day_dark_awake={:.3}[{:.3}..{:.3}],motor_effort={:.3}[{:.3}..{:.3}],crowd_per_body={:.3}[{:.3}..{:.3}],egg_loss={:.4}({lost:.0}/{laid:.0}),windows={}",
            f.name(),
            lf.day_dark_awake,
            quantile(&q_dark, 0.25),
            quantile(&q_dark, 0.75),
            lf.motor_effort,
            quantile(&effort, 0.25),
            quantile(&effort, 0.75),
            lf.crowd_per_body,
            quantile(&crowd, 0.25),
            quantile(&crowd, 0.75),
            lf.egg_loss,
            r_pts.len()
        );
        // r → ψ through the model's own gut at the windows' median body.
        let food = fit_food(f);
        let mb = median(&mut r_pts.iter().map(|x| x.2).collect::<Vec<_>>());
        let table: Vec<(f64, f64)> = (0..=50).map(|k| {
            let psi = k as f64 / 50.0;
            (psi, gut_r(&base, f, food, mb, psi))
        }).collect();
        say!("gut_r,{},body={mb:.4},{}", f.name(), table.iter().step_by(5).map(|(p, r)| format!("{p:.1}:{r:.3}")).collect::<Vec<_>>().join(" "));
        let mut pts: Vec<(f64, f64)> = Vec::new();
        if f == Founder::Capgnawer {
            // The capgnawer eats caps (yield y_c) and litter (y_l): r is in cap-ceiling units.
            // Its litter share ψ_l is read where the caps are nearly gone (stock < 1.5), as a
            // constant; the caps' response is fitted on what is left, the gut being linear
            // below its saturation (r ≈ 2ψ, the table above).
            let (yc, yl) = (ph.yield_for(Food::CapTissue), ph.yield_for(Food::Litter));
            let mut low: Vec<f64> = r_pts.iter().filter(|x| x.0 < 1.5).map(|x| psi_of_r(&table, x.1 * yc / yl)).collect();
            let psi_l = median(&mut low).max(0.0);
            let r_l = table.iter().find(|x| x.0 >= psi_l).map_or(0.0, |x| x.1) * yl / yc;
            fits.of_mut(f).response[Food::Litter.index()] = Some(fits::Response::holling(psi_l, 0.0));
            say!("fit,capgnawer,response[litter]: constant psi={psi_l:.3} from {} windows with caps < 1.5 (r_litter in cap units {r_l:.3})", low.len());
            for (stock, r, _) in &r_pts {
                pts.push((*stock, psi_of_r(&table, (r - r_l).max(0.0))));
            }
        } else {
            for (stock, r, _) in &r_pts {
                pts.push((*stock, psi_of_r(&table, *r)));
            }
        }
        let (a, h, rms) = fit_holling(&pts);
        let ratio: Vec<f64> = pts.iter().map(|(e, y)| y / (a * e / (e + h)).max(1e-9)).collect();
        fits.of_mut(f).response[food.index()] = Some(fits::Response::holling(a, h));
        say!("fit,{},first-pass (window r inverted through the gut) psi_max={a:.3} half={h:.3}", f.name());
        let _ = rms;
        let _ = ratio;
        continue;
        #[allow(unreachable_code)]
        say!(
            "fit,{},response[{}]: psi=psi_max*E/(E+half), psi_max={a:.3}, half={h:.3}, rms={rms:.3}, obs/fit IQR [{:.2}..{:.2}], stock range [{:.3}..{:.3}], n={}",
            f.name(),
            food.name(),
            quantile(&ratio, 0.25),
            quantile(&ratio, 0.75),
            quantile(&pts.iter().map(|x| x.0).collect::<Vec<_>>(), 0.0),
            quantile(&pts.iter().map(|x| x.0).collect::<Vec<_>>(), 1.0),
            pts.len()
        );
        fits.fitted[i] = true;
    }
    // **The responses by simulation** (pattern-oriented): the per-window intake of each
    // detritivore across every D1b arm and seed, the model run alone on the seed's clamped
    // stocks, matched in log space over a grid. The first pass above inverts each window's
    // realized share of the gut ceiling, which misreads who is awake (the logs' awake share
    // counts bodies, not body mass: hungry juveniles wake on the emergency latch while the
    // adults sleep), so it only seeds the grid.
    let runs: Vec<(cubarium_voxel_sim::params::Params, Clamp, Vec<(Founder, f64)>, &RunLog)> = logs
        .iter()
        .map(|l| (params_of(&l.arm), Clamp::of(l), DETRITIVORES.iter().map(|&f| (f, get(l.det.get(&(0, f.index())), "n"))).collect(), l))
        .collect();
    let minutes: Vec<u32> = (0..=24).map(|k| k * 10).collect();
    if obs_only {
        fits = Fits::recorded();
        for f in DETRITIVORES {
            fits.of_mut(f).nest_aggregation = 1.0;
        }
    }
    for f in DETRITIVORES {
        if obs_only {
            break;
        }
        let i = f.index();
        let food = fit_food(f);
        let score = |fits: &Fits| -> (f64, Vec<f64>) {
            let chunks: Vec<Vec<(f64, f64)>> = std::thread::scope(|sc| {
                let handles: Vec<_> = runs
                    .chunks(runs.len().div_ceil(4))
                    .map(|part| {
                        let minutes = &minutes;
                        sc.spawn(move || {
                            let mut v = Vec::new();
                            for (p, clamp, n0, log) in part {
                                let out = run_d1b_only(p, clamp, n0, fits, minutes, Draws::Mean, Some(f));
                                for w in minutes.windows(2) {
                                    if w[1] < 20 {
                                        continue;
                                    }
                                    let sim = get(log.det.get(&(w[1], i)), "intake");
                                    if !sim.is_finite() {
                                        continue;
                                    }
                                    let a = out.get(&(w[1], i)).map_or(0.0, |r| r.assim) - out.get(&(w[0], i)).map_or(0.0, |r| r.assim);
                                    v.push((a, sim));
                                }
                            }
                            v
                        })
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            let e = 1e-4;
            let mut err = 0.0;
            let mut ratios = Vec::new();
            for (a, sim) in chunks.into_iter().flatten() {
                err += ((a + e).ln() - (sim + e).ln()).powi(2);
                ratios.push((a + e) / (sim + e));
            }
            (err, ratios)
        };
        let stocks: Vec<f64> = runs.iter().flat_map(|(_, c, _, _)| c.pts.iter().map(|p| match food { Food::Litter => p.1, Food::Film => p.2, _ => p.3 })).collect();
        let s_med = quantile(&stocks, 0.5);
        let mut best = (f64::INFINITY, fits.of(f).clone(), Vec::new());
        let halves: Vec<f64> = [0.0, 0.1, 0.3, 1.0, 3.0].iter().map(|k| k * s_med).collect();
        let litter_psi: Vec<f64> = if f == Founder::Capgnawer { vec![0.0, 0.1, 0.2, 0.3] } else { vec![-1.0] };
        let mut try_one = |pl: f64, h: f64, ifr: f64, psi_max: f64, best: &mut (f64, fits::LineageFit, Vec<f64>)| {
            let mut cand = fits.clone();
            cand.of_mut(f).response[food.index()] = Some(fits::Response { psi_max, half: h, interference: ifr });
            if pl >= 0.0 {
                cand.of_mut(f).response[Food::Litter.index()] = Some(fits::Response::holling(pl, 0.0));
            }
            let (e, r) = score(&cand);
            if e < best.0 {
                *best = (e, *cand.of(f), r);
            }
        };
        // Coarse, then fine around the best.
        let interferences: Vec<f64> = [0.0, 0.01, 0.03, 0.1, 0.3].iter().map(|k| k * s_med).collect();
        for &pl in &litter_psi {
            for &h in &halves {
                for &ifr in &interferences {
                    for k in 1..=12 {
                        try_one(pl, h, ifr, k as f64 * 0.05, &mut best);
                    }
                }
            }
        }
        let b = best.1;
        let (c_psi, c_half, c_if) = b.response[food.index()].map_or((0.5, 0.0, 0.0), |r| (r.psi_max, r.half, r.interference));
        let c_lit = if f == Founder::Capgnawer { b.response[Food::Litter.index()].map_or(0.0, |r| r.psi_max) } else { -1.0 };
        let lits: Vec<f64> = if c_lit >= 0.0 { (-2..=2).map(|k| (c_lit + 0.025 * f64::from(k)).max(0.0)).collect() } else { vec![-1.0] };
        for &pl in &lits {
            for k in -5..=5 {
                let pm = c_psi + 0.01 * f64::from(k);
                if pm > 0.0 {
                    try_one(pl, c_half, c_if, pm.min(1.0), &mut best);
                }
            }
        }
        *fits.of_mut(f) = best.1;
        let r = &best.2;
        say!(
            "fit,{},response by simulation: {}, model/sim window intake median {:.2} IQR [{:.2}..{:.2}] over {} windows (all D1b arms and seeds, m20-240), stock median {s_med:.3}",
            f.name(),
            Food::ALL
                .iter()
                .filter_map(|g| fits.of(f).response[g.index()].map(|x| format!("{}: psi_max={:.3} half={:.3} interference={:.4}", g.name(), x.psi_max, x.half, x.interference)))
                .collect::<Vec<_>>()
                .join("; "),
            quantile(r, 0.5),
            quantile(r, 0.25),
            quantile(r, 0.75),
            r.len()
        );
    }
    // **Nest-site aggregation** on the 8 h observation's training seeds (4 5 7 8; seeds 9 12
    // 13 15 are held out for validation): the branch arm from founding to minute 720, each
    // lineage alone, `nest_aggregation` on a grid, matched on the lineage's count and its
    // cumulative hatched at 360-720 in log space.
    let starts = logs_of(&gdir.join("runs"), "branch");
    let obs = logs_of(&gdir.join("obs"), "branch");
    let train: Vec<(Clamp, Vec<(Founder, f64)>, &RunLog, &RunLog)> = starts
        .iter()
        .filter(|l| TRAIN_SEEDS.contains(&l.seed))
        .filter_map(|st| {
            let o = obs.iter().find(|o| o.seed == st.seed)?;
            let mut c = Clamp::of(st);
            c.extend(&Clamp::of(o));
            Some((c, DETRITIVORES.iter().map(|&f| (f, get(st.det.get(&(0, f.index())), "n"))).collect(), st, o))
        })
        .collect();
    let pb = params_of("branch");
    let mins: Vec<u32> = vec![0, 360, 480, 600, 720];
    for f in DETRITIVORES {
        let i = f.index();
        let food = fit_food(f);
        let base_resp = fits.of(f).response[food.index()].unwrap_or(fits::Response::ALWAYS);
        // The 4 h windows' median stock and count: interference is added keeping ψ there.
        let e_med = quantile(&runs.iter().flat_map(|(_, c, _, _)| c.pts.iter().filter(|p| p.0 <= 240.0).map(|p| match food { Food::Litter => p.1, Food::Film => p.2, _ => p.3 })).collect::<Vec<_>>(), 0.5);
        let n_med = quantile(&runs.iter().flat_map(|(_, _, _, l)| l.det.iter().filter(|((m, j), _)| *j == i && *m >= 20 && *m <= 240).map(|(_, w)| get(Some(w), "n"))).collect::<Vec<_>>(), 0.5);
        let ifr_grid: Vec<f64> = if base_resp.interference > 0.0 { vec![base_resp.interference] } else { [0.0, 0.003, 0.01, 0.03, 0.1, 0.3].iter().map(|k| k * e_med).collect() };
        let mut best = (f64::INFINITY, 1.0, base_resp);
        let mut line = String::new();
        for &ifr in &ifr_grid {
            let resp = if ifr == base_resp.interference {
                base_resp
            } else {
                let k = (e_med + base_resp.half + ifr * (n_med - 1.0).max(0.0)) / (e_med + base_resp.half);
                fits::Response { psi_max: (base_resp.psi_max * k).min(1.0), half: base_resp.half, interference: ifr }
            };
            for kappa in [1.0, 1.5, 2.0, 3.0, 4.0] {
                let mut cand = fits.clone();
                cand.of_mut(f).nest_aggregation = kappa;
                cand.of_mut(f).response[food.index()] = Some(resp);
                let mut err = 0.0;
                for (clamp, n0, st, o) in &train {
                    let out = run_d1b_only(&pb, clamp, n0, &cand, &mins, Draws::Mean, Some(f));
                    for &mi in &mins[1..] {
                        let d = o.det.get(&(mi, i));
                        let (sim_n, sim_a) = (get(d, "n"), get(d, "adults"));
                        let mut hat = 0.0;
                        for ((m2, j), w) in &st.det {
                            if *j == i && *m2 <= 240 && *m2 > 0 {
                                hat += get(Some(w), "hatched").max(0.0);
                            }
                        }
                        for ((m2, j), w) in &o.det {
                            if *j == i && *m2 > 240 && *m2 <= mi {
                                hat += get(Some(w), "hatched").max(0.0);
                            }
                        }
                        let r = out.get(&(mi, i)).copied().unwrap_or_default();
                        if sim_n.is_finite() {
                            let l = |x: f64| (x + 1.0).ln();
                            err += (l(r.n) - l(sim_n)).powi(2) + (l(r.hatched) - l(hat)).powi(2) + (l(r.adults) - l(sim_a)).powi(2) + (l(r.juveniles) - l(sim_n - sim_a)).powi(2);
                        }
                    }
                }
                line += &format!(" i={ifr:.3},k={kappa}:{err:.2}");
                if err < best.0 {
                    best = (err, kappa, resp);
                }
            }
        }
        fits.of_mut(f).nest_aggregation = best.1;
        fits.of_mut(f).response[food.index()] = Some(best.2);
        say!(
            "fit,{},observation (training seeds {:?}, branch to m720; count, adults, juveniles, hatched): nest_aggregation={}, response psi_max={:.3} half={:.3} interference={:.4} (4 h median stock {e_med:.2}, count {n_med:.0}); errors:{line}",
            f.name(),
            TRAIN_SEEDS,
            best.1,
            best.2.psi_max,
            best.2.half,
            best.2.interference
        );
    }
    let _ = lj;
    fits
}

/// The observation's training seeds; the other four are held out.
const TRAIN_SEEDS: [u32; 4] = [4, 5, 7, 8];

fn fit_cmd(args: &[String]) {
    let gdir = PathBuf::from(arg(args, "gen=").expect("gen=DIR"));
    let lj = arg(args, "lj=").map(PathBuf::from);
    fit_all_with(&gdir, lj.as_deref(), false, args.iter().any(|a| a == "obs-only"));
}

// ---------------------------------------------------------------------------------------
// Open-loop runs.

/// A clamped stock series: (minute, litter, film, cap_tissue), interpolated linearly.
#[derive(Clone, Default)]
struct Clamp {
    pts: Vec<(f64, f64, f64, f64)>,
}

impl Clamp {
    fn of(log: &RunLog) -> Clamp {
        let mut pts: Vec<(f64, f64, f64, f64)> = log
            .stocks
            .iter()
            .map(|(m, s)| (f64::from(*m), get(Some(s), "litter"), get(Some(s), "film"), get(Some(s), "cap_tissue")))
            .filter(|p| p.1.is_finite())
            .collect();
        pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        Clamp { pts }
    }

    fn extend(&mut self, other: &Clamp) {
        let last = self.pts.last().map_or(-1.0, |p| p.0);
        self.pts.extend(other.pts.iter().filter(|p| p.0 > last).copied());
    }

    fn at(&self, minute: f64) -> Supply {
        let mut s = Supply::default();
        if self.pts.is_empty() {
            return s;
        }
        let lerp = |a: &(f64, f64, f64, f64), b: &(f64, f64, f64, f64)| {
            let w = if b.0 > a.0 { ((minute - a.0) / (b.0 - a.0)).clamp(0.0, 1.0) } else { 0.0 };
            (a.1 + w * (b.1 - a.1), a.2 + w * (b.2 - a.2), a.3 + w * (b.3 - a.3))
        };
        let (l, fm, c) = if minute <= self.pts[0].0 {
            let p = self.pts[0];
            (p.1, p.2, p.3)
        } else if minute >= self.pts.last().unwrap().0 {
            let p = *self.pts.last().unwrap();
            (p.1, p.2, p.3)
        } else {
            let k = self.pts.iter().position(|p| p.0 > minute).unwrap();
            lerp(&self.pts[k - 1], &self.pts[k])
        };
        s.litter = l;
        s.film = fm;
        s.foliage[cubarium_voxel_flora::Species::Glowcap.index()] = c;
        s
    }
}

/// What a model run reads out at a minute, per lineage, cumulative where the sim's is.
#[derive(Clone, Copy, Default, Debug)]
struct Readout {
    n: f64,
    adults: f64,
    juveniles: f64,
    matured: f64,
    maturation_min: f64,
    first_clutches: f64,
    first_clutch_min: f64,
    clutch_mean: f64,
    laid: f64,
    hatched: f64,
    refusals: f64,
    killed: f64,
    fullness: f64,
    assim: f64,
    alive_s: f64,
    awake_s: f64,
    body_s: f64,
}

fn readout(m: &FaunaModel, f: Founder) -> Readout {
    let c = m.census(f);
    let l = &m.ledger[f.index()];
    Readout {
        n: c.n,
        adults: c.adults,
        juveniles: c.juveniles,
        matured: l.matured,
        maturation_min: if l.matured > 0.0 { l.maturation_age_s / l.matured / 60.0 } else { f64::NAN },
        first_clutches: l.first_clutches,
        first_clutch_min: if l.first_clutches > 0.0 { l.first_clutch_age_s / l.first_clutches / 60.0 } else { f64::NAN },
        clutch_mean: if l.clutches > 0.0 { l.laid / l.clutches } else { f64::NAN },
        laid: l.laid,
        hatched: l.hatched,
        refusals: l.nest_refusals,
        killed: l.killed_by.iter().sum(),
        fullness: c.mean_fullness,
        assim: l.flows.assimilated,
        alive_s: l.alive_s,
        awake_s: l.awake_s,
        body_s: l.body_s,
    }
}

/// **The D1b run in the model**: the founders of the log's minute 0, under the arm's config,
/// the stocks clamped to the seed's own, read out at each logged minute.
fn run_d1b(arm: &str, clamp: &Clamp, n0: &[(Founder, f64)], fits: &Fits, minutes: &[u32], draws: Draws) -> BTreeMap<(u32, usize), Readout> {
    run_d1b_only(&params_of(arm), clamp, n0, fits, minutes, draws, None)
}

fn run_d1b_only(
    p: &cubarium_voxel_sim::params::Params,
    clamp: &Clamp,
    n0: &[(Founder, f64)],
    fits: &Fits,
    minutes: &[u32],
    draws: Draws,
    only: Option<Founder>,
) -> BTreeMap<(u32, usize), Readout> {
    let mut m = FaunaModel::new(p, fits.clone(), 1.0);
    m.draws = draws;
    for g in Founder::ALL {
        m.enabled[g.index()] = DETRITIVORES.contains(&g) && only.is_none_or(|o| o == g);
    }
    for &(f, n) in n0 {
        if n > 0.0 && m.enabled[f.index()] {
            m.add(f, Cohort::founders(&p.fauna, f, n));
        }
    }
    let mut out = BTreeMap::new();
    let end = minutes.iter().copied().max().unwrap_or(0);
    let mut next = minutes.iter().copied().collect::<std::collections::BTreeSet<u32>>();
    for f in DETRITIVORES {
        out.insert((0, f.index()), readout(&m, f));
    }
    next.remove(&0);
    while m.t_s < f64::from(end) * 60.0 - 1e-9 {
        let s = clamp.at(m.t_s / 60.0);
        m.step(&s);
        let minute = (m.t_s / 60.0).round() as u32;
        if (m.t_s - f64::from(minute) * 60.0).abs() < 0.5 && next.remove(&minute) {
            for f in DETRITIVORES {
                out.insert((minute, f.index()), readout(&m, f));
            }
        }
    }
    out
}

fn fmt_spread(v: &[f64]) -> String {
    let w: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if w.is_empty() {
        return "-".into();
    }
    format!("{:.1} [{:.1}..{:.1}]", quantile(&w, 0.5), quantile(&w, 0.0), quantile(&w, 1.0))
}

/// The bar: the model's median within the sim seeds' [min..max], or within `factor` of the
/// sim median.
fn verdict(sim: &[f64], model: &[f64], factor: f64) -> &'static str {
    let s: Vec<f64> = sim.iter().copied().filter(|x| x.is_finite()).collect();
    let mo: Vec<f64> = model.iter().copied().filter(|x| x.is_finite()).collect();
    if s.is_empty() && mo.is_empty() {
        return "pass(none)";
    }
    if s.is_empty() || mo.is_empty() {
        return "FAIL(one side empty)";
    }
    let med = quantile(&mo, 0.5);
    let (lo, hi) = (quantile(&s, 0.0), quantile(&s, 1.0));
    let sm = quantile(&s, 0.5);
    // Counts are whole bodies: within one body of the seeds' range passes.
    if med >= lo - 1.0 && med <= hi + 1.0 {
        "pass"
    } else if sm > 0.0 && med > 0.0 && (med / sm).max(sm / med) <= factor {
        "pass(factor)"
    } else {
        "FAIL"
    }
}

fn d1b_cmd(args: &[String]) {
    let gdir = PathBuf::from(arg(args, "gen=").expect("gen=DIR"));
    let twins: usize = arg(args, "twin=").map_or(0, |t| t.parse().unwrap());
    let fits = if args.iter().any(|a| a == "refit") { fit_all(&gdir, None, true) } else { Fits::recorded() };
    let factor = 1.5;
    let fields: [(&str, fn(&Readout) -> f64, &str); 9] = [
        ("N", |r| r.n, "n"),
        ("adults", |r| r.adults, "adults"),
        ("juveniles", |r| r.juveniles, "juveniles"),
        ("matured(cum)", |r| r.matured, "matured"),
        ("maturation_min", |r| r.maturation_min, "maturation_min"),
        ("first_clutch_min", |r| r.first_clutch_min, "first_clutch_min"),
        ("clutch_mean", |r| r.clutch_mean, "clutch_mean"),
        ("hatched(cum)", |r| r.hatched, "hatched"),
        ("refusals(cum)", |r| r.refusals, "nest_refusals"),
    ];
    let mut tally: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
    for (arm, dir, minutes) in [
        ("main", "runs", vec![60u32, 120, 180, 240]),
        ("gut", "eid", vec![60, 120, 180, 240]),
        ("brood", "eid", vec![60, 120, 180, 240]),
        ("branch", "runs", vec![60, 120, 180, 240]),
        ("rate", "runs", vec![60, 120, 180, 240]),
        ("branch-obs", "obs", vec![360, 480, 600, 720]),
        ("branch-obs-holdout", "obs", vec![360, 480, 600, 720]),
    ] {
        let holdout = arm.ends_with("-holdout");
        let arm = arm.trim_end_matches("-holdout");
        let base_arm = arm.trim_end_matches("-obs");
        let mut logs = logs_of(&gdir.join(dir), base_arm);
        if holdout {
            logs.retain(|l| !TRAIN_SEEDS.contains(&l.seed));
        }
        let starts = if arm == "branch-obs" { logs_of(&gdir.join("runs"), "branch") } else { Vec::new() };
        println!("=== {arm}{}: {} seeds", if holdout { " (held-out seeds)" } else { "" }, logs.len());
        // Per seed: the sim's cumulative readouts summed over windows; the model's.
        let mut sim: BTreeMap<(u32, usize, &str), Vec<f64>> = BTreeMap::new();
        let mut model: BTreeMap<(u32, usize, &str), Vec<f64>> = BTreeMap::new();
        for log in &logs {
            let (clamp, n0, cum_src): (Clamp, Vec<(Founder, f64)>, Vec<&RunLog>) = if arm == "branch-obs" {
                let st = starts.iter().find(|l| l.seed == log.seed).expect("the seed's first 4 h");
                let mut c = Clamp::of(st);
                c.extend(&Clamp::of(log));
                (c, DETRITIVORES.iter().map(|&f| (f, get(st.det.get(&(0, f.index())), "n"))).collect(), vec![st, log])
            } else {
                (Clamp::of(log), DETRITIVORES.iter().map(|&f| (f, get(log.det.get(&(0, f.index())), "n"))).collect(), vec![log])
            };
            let mut all_minutes = minutes.clone();
            all_minutes.push(0);
            // The sim: counts at the minute; cumulative matured, first clutches (count-weighted
            // mean age), clutch mean, summed over every window up to the minute.
            for &mi in &minutes {
                for f in DETRITIVORES {
                    let i = f.index();
                    let d = cum_src.iter().find_map(|l| l.det.get(&(mi, i)));
                    let (mut mat, mut mat_age, mut fc, mut fc_age, mut cl, mut laid) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
                    let (mut hat, mut refu) = (0.0, 0.0);
                    for (k, l) in cum_src.iter().enumerate() {
                        for ((m2, j), w) in l.det.range((0, 0)..=(mi, Founder::COUNT)) {
                            if *j != i || *m2 == 0 || *m2 > mi {
                                continue;
                            }
                            // The observation continues the first 4 h: its windows after 240.
                            if arm == "branch-obs" && ((k == 0 && *m2 > 240) || (k == 1 && *m2 <= 240)) {
                                continue;
                            }
                            let mt = get(Some(w), "matured");
                            if mt > 0.0 {
                                mat += mt;
                                mat_age += mt * get(Some(w), "maturation_min");
                            }
                            let r = get(Some(w), "replacement");
                            if r > 0.0 {
                                fc += r;
                                fc_age += r * get(Some(w), "first_clutch_min");
                            }
                            cl += get(Some(w), "clutches").max(0.0);
                            hat += get(Some(w), "hatched").max(0.0);
                            refu += get(Some(w), "nest_refusals").max(0.0);
                            laid += get(Some(w), "laid").max(0.0);
                        }
                    }
                    let push = |k: &'static str, v: f64, sim: &mut BTreeMap<(u32, usize, &str), Vec<f64>>| sim.entry((mi, i, k)).or_default().push(v);
                    push("N", get(d, "n"), &mut sim);
                    push("adults", get(d, "adults"), &mut sim);
                    push("juveniles", get(d, "juveniles"), &mut sim);
                    push("matured(cum)", mat, &mut sim);
                    push("maturation_min", if mat > 0.0 { mat_age / mat } else { f64::NAN }, &mut sim);
                    push("first_clutch_min", if fc > 0.0 { fc_age / fc } else { f64::NAN }, &mut sim);
                    push("clutch_mean", if cl > 0.0 { laid / cl } else { f64::NAN }, &mut sim);
                    push("hatched(cum)", hat, &mut sim);
                    push("refusals(cum)", refu, &mut sim);
                }
            }
            let runs = if twins > 0 { twins } else { 1 };
            for k in 0..runs {
                let draws = if twins > 0 { Draws::Twin(Rng::new(u64::from(log.seed) * 1000 + k as u64)) } else { Draws::Mean };
                let out = run_d1b(base_arm, &clamp, &n0, &fits, &all_minutes, draws);
                for &mi in &minutes {
                    for f in DETRITIVORES {
                        let r = out.get(&(mi, f.index())).copied().unwrap_or_default();
                        for (name, get_, _) in &fields {
                            model.entry((mi, f.index(), *name)).or_default().push(get_(&r));
                        }
                    }
                }
            }
        }
        for f in DETRITIVORES {
            println!("  {}", f.name());
            for (name, _, _) in &fields {
                let mut line = format!("    {name:<17}");
                for &mi in &minutes {
                    let sv = sim.get(&(mi, f.index(), *name)).cloned().unwrap_or_default();
                    let mv = model.get(&(mi, f.index(), *name)).cloned().unwrap_or_default();
                    let v = verdict(&sv, &mv, factor);
                    let e = tally.entry(name).or_default();
                    e.1 += 1;
                    if v.starts_with("pass") {
                        e.0 += 1;
                    }
                    line += &format!(" | m{mi}: sim {} model {} {v}", fmt_spread(&sv), fmt_spread(&mv));
                }
                println!("{line}");
            }
        }
    }
    println!("=== tally (pass / cells), factor {factor}:");
    for (k, (p, n)) in tally {
        println!("  {k}: {p}/{n}");
    }
}

// ---------------------------------------------------------------------------------------
// D2b: patch-hunters.

const D2B_PREY: [Founder; 4] = [Founder::Blind, Founder::Capgnawer, Founder::RippleSnail, Founder::Browser];

/// The median D1b `branch` stocks per logged minute (litter, film, caps, and the
/// photosynthetic `edible` foliage): the patch's founding stocks, for the two hours before a
/// D2b log starts, and its foliage throughout (D2b does not log foliage).
fn median_stocks(gdir: &Path) -> BTreeMap<u32, (f64, f64, f64, f64)> {
    let logs = logs_of(&gdir.join("runs"), "branch");
    let mut out = BTreeMap::new();
    let minutes: std::collections::BTreeSet<u32> = logs.iter().flat_map(|l| l.stocks.keys().copied()).collect();
    for m in minutes {
        let col = |k: &str| median(&mut logs.iter().map(|l| get(l.stocks.get(&m), k)).collect::<Vec<_>>());
        out.insert(m, (col("litter"), col("film"), col("cap_tissue"), col("edible")));
    }
    out
}

fn lerp_map(map: &BTreeMap<u32, (f64, f64, f64, f64)>, minute: f64) -> (f64, f64, f64, f64) {
    let k = minute.max(0.0).floor() as u32;
    let lo = map.range(..=k).next_back();
    let hi = map.range(k + 1..).next();
    match (lo, hi) {
        (Some((a, x)), Some((b, y))) => {
            let w = (minute - f64::from(*a)) / f64::from(b - a).max(1e-9);
            (x.0 + w * (y.0 - x.0), x.1 + w * (y.1 - x.1), x.2 + w * (y.2 - x.2), x.3 + w * (y.3 - x.3))
        }
        (Some((_, x)), None) => *x,
        (None, Some((_, y))) => *y,
        _ => (0.0, 0.0, 0.0, 0.0),
    }
}

/// A D2b supply at `minute`: the seed's own logged litter, film and caps from minute 120, the
/// D1b medians before; foliage always the D1b median (put on the springturf slot: the
/// browser's food is the photosynthetic total).
fn d2b_supply(med: &BTreeMap<u32, (f64, f64, f64, f64)>, seed: &Clamp, minute: f64) -> Supply {
    let m = lerp_map(med, minute.min(240.0));
    let mut s = if minute >= 120.0 && !seed.pts.is_empty() { seed.at(minute) } else {
        let mut s = Supply::default();
        s.litter = m.0;
        s.film = m.1;
        s.foliage[cubarium_voxel_flora::Species::Glowcap.index()] = m.2;
        s
    };
    s.foliage[cubarium_voxel_flora::Species::Springturf.index()] = m.3;
    s
}

/// **The prey at minute 120** of a patch-hunters founding, in the model: the patch founders
/// (the D1b minute-0 counts) run on the median founding stocks, then each lineage's adults
/// and juveniles scaled to the seed's logged census.
fn d2b_start(p: &cubarium_voxel_sim::params::Params, fits: &Fits, med: &BTreeMap<u32, (f64, f64, f64, f64)>, gdir: &Path, log: &RunLog) -> FaunaModel {
    let mut m = FaunaModel::new(p, fits.clone(), 1.0);
    for g in Founder::ALL {
        m.enabled[g.index()] = D2B_PREY.contains(&g) || g == Founder::Lanternjaw;
    }
    let d1 = logs_of(&gdir.join("runs"), "branch");
    for f in D2B_PREY {
        let n0 = if f == Founder::Browser {
            median(&mut d1.iter().map(|l| get(l.stocks.get(&0), "frondgrazer")).collect::<Vec<_>>())
        } else {
            median(&mut d1.iter().map(|l| get(l.det.get(&(0, f.index())), "n")).collect::<Vec<_>>())
        };
        if n0 > 0.0 {
            m.add(f, Cohort::founders(&p.fauna, f, n0));
        }
    }
    let empty = Clamp::default();
    while m.t_s < 120.0 * 60.0 - 1e-9 {
        let s = d2b_supply(med, &empty, m.t_s / 60.0);
        m.step(&s);
    }
    for f in D2B_PREY {
        let d = log.det.get(&(120, f.index()));
        let (n, adults) = (get(d, "n"), get(d, "adults"));
        if !n.is_finite() {
            continue;
        }
        let c = m.census(f);
        let sc = p.fauna.founder(f).core;
        let (ka, kj) = (
            if c.adults > 0.0 { adults / c.adults } else { 0.0 },
            if c.juveniles > 0.0 { (n - adults) / c.juveniles } else { 0.0 },
        );
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
    // The hunters introduced at minute 120 (`introduce_lanternjaws`: arrival stores), the
    // logged riders (newly hatched, on the grazers) and the logged bank.
    let lj = log.lj.get(&120);
    let lp = p.fauna.founder(Founder::Lanternjaw).core;
    let free = get(lj, "adults");
    if free > 0.0 {
        m.add(Founder::Lanternjaw, Cohort::founders(&p.fauna, Founder::Lanternjaw, free));
    }
    let riders = get(lj, "riders");
    if riders > 0.0 {
        let mut r = Cohort::new(riders, lp.body_min, (lp.reproduction.egg_organic - lp.body_min).max(0.0));
        r.native = true;
        r.host = Some(Founder::Browser);
        m.add(Founder::Lanternjaw, r);
    }
    let bank = get(lj, "bank");
    if bank > 0.0 {
        m.eggs.push(model::EggCohort { lineage: Founder::Lanternjaw, n: bank, organic: lp.reproduction.egg_organic, age_s: 0.0, dormant: true });
    }
    m.t_s = 120.0 * 60.0;
    m
}

/// One D2b arm and seed in the model from minute 120: prey and hunters at the logged minutes.
fn run_d2b(p: &cubarium_voxel_sim::params::Params, fits: &Fits, med: &BTreeMap<u32, (f64, f64, f64, f64)>, gdir: &Path, log: &RunLog, obs: Option<&RunLog>, end: u32, draws: Draws) -> BTreeMap<u32, BTreeMap<String, f64>> {
    let mut m = d2b_start(p, fits, med, gdir, log);
    m.draws = draws;
    let mut seed = Clamp::of(log);
    if let Some(o) = obs {
        seed.extend(&Clamp::of(o));
    }
    let mut out = BTreeMap::new();
    let mut last = 120u32;
    let a0 = m.ledger[Founder::Lanternjaw.index()].flows.assimilated;
    while m.t_s < f64::from(end) * 60.0 - 1e-9 {
        let s = d2b_supply(med, &seed, m.t_s / 60.0);
        m.step(&s);
        let minute = (m.t_s / 60.0).round() as u32;
        if minute % 10 == 0 && minute != last && (m.t_s - f64::from(minute) * 60.0).abs() < 0.5 {
            last = minute;
            let mut r = BTreeMap::new();
            for f in D2B_PREY {
                let c = m.census(f);
                r.insert(format!("{}.n", f.name()), c.n);
                r.insert(format!("{}.adults", f.name()), c.adults);
                r.insert(format!("{}.killed", f.name()), m.ledger[f.index()].killed_by[Founder::Lanternjaw.index()]);
            }
            let c = m.census(Founder::Lanternjaw);
            let l = &m.ledger[Founder::Lanternjaw.index()];
            r.insert("lj.n".into(), c.n);
            r.insert("lj.free".into(), c.n - c.riders);
            r.insert("lj.riders".into(), c.riders);
            r.insert("lj.bank".into(), c.bank);
            r.insert("lj.fullness".into(), c.mean_fullness);
            r.insert("lj.kills".into(), l.kills);
            r.insert("lj.intake".into(), l.flows.assimilated - a0);
            r.insert("lj.laid".into(), l.laid);
            r.insert("lj.hatched".into(), l.hatched);
            r.insert("lj.awake".into(), if l.alive_s > 0.0 { l.awake_s / l.alive_s } else { f64::NAN });
            r.insert("small".into(), [Founder::Blind, Founder::Capgnawer, Founder::RippleSnail].iter().map(|f| m.census(*f).n).sum());
            out.insert(minute, r);
        }
    }
    out
}

/// The sim's readouts of a D2b log at `minute`, in the model's keys: counts now, kills summed
/// over the windows since minute 120.
fn d2b_sim(log: &RunLog, obs: Option<&RunLog>, minute: u32) -> BTreeMap<String, f64> {
    let mut r = BTreeMap::new();
    let srcs: Vec<&RunLog> = std::iter::once(log).chain(obs).collect();
    let at = |m: u32| srcs.iter().find_map(|l| if l.lj.contains_key(&m) { Some(*l) } else { None });
    let Some(src) = at(minute) else { return r };
    for f in D2B_PREY {
        let d = src.det.get(&(minute, f.index()));
        r.insert(format!("{}.n", f.name()), get(d, "n"));
        r.insert(format!("{}.adults", f.name()), get(d, "adults"));
        let mut k = 0.0;
        for (i, l) in srcs.iter().enumerate() {
            for ((m2, j), w) in &l.det {
                if *j == f.index() && *m2 > 120 && *m2 <= minute && (i == 0 && *m2 <= 240 || i == 1 && *m2 > 240) {
                    k += get(Some(w), "kills").max(0.0);
                }
            }
        }
        r.insert(format!("{}.killed", f.name()), k);
    }
    let lj = src.lj.get(&minute);
    r.insert("lj.n".into(), get(lj, "n"));
    r.insert("lj.free".into(), get(lj, "adults") + get(lj, "juveniles"));
    r.insert("lj.riders".into(), get(lj, "riders"));
    r.insert("lj.bank".into(), get(lj, "bank"));
    r.insert("lj.fullness".into(), get(lj, "fullness"));
    let mut kills = 0.0;
    let mut laid = 0.0;
    let mut hatched = 0.0;
    let mut intake = 0.0;
    for (i, l) in srcs.iter().enumerate() {
        for (m2, w) in &l.lj {
            if *m2 > 120 && *m2 <= minute && (i == 0 && *m2 <= 240 || i == 1 && *m2 > 240) {
                kills += get(Some(w), "kills").max(0.0);
                intake += get(Some(w), "intake").max(0.0);
                laid += get(Some(w), "laid").max(0.0);
                hatched += get(Some(w), "hatched").max(0.0);
            }
        }
    }
    r.insert("lj.kills".into(), kills);
    r.insert("lj.intake".into(), intake);
    r.insert("lj.laid".into(), laid);
    r.insert("lj.hatched".into(), hatched);
    r.insert("small".into(), [Founder::Blind, Founder::Capgnawer, Founder::RippleSnail].iter().map(|f| get(src.det.get(&(minute, f.index())), "n")).sum());
    r
}

/// **The kill rates by simulation**: each prey lineage's `kill_rate` scaled until the model's
/// kills of it over minutes 120-240, summed over the `d2` arm's seeds, equal the sim's.
fn fit_kills(fits: &mut Fits, med: &BTreeMap<u32, (f64, f64, f64, f64)>, gdir: &Path, ljdir: &Path, quiet: bool) {
    let p = params_of("d2");
    let logs = logs_of(&ljdir.join("mech"), "d2");
    // Strikes per kill, from the arm's own lines.
    let (mut strikes, mut kills) = (0.0, 0.0);
    for l in &logs {
        for w in l.lj.values() {
            strikes += get(Some(w), "strikes").max(0.0);
            kills += get(Some(w), "kills").max(0.0);
        }
    }
    fits.strikes_per_kill = if kills > 0.0 { strikes / kills } else { 1.0 };
    let prey = [Founder::Blind, Founder::Capgnawer, Founder::RippleSnail, Founder::Browser];
    let sim_k: Vec<f64> = prey.iter().map(|f| logs.iter().map(|l| d2b_sim(l, None, 240).get(&format!("{}.killed", f.name())).copied().unwrap_or(0.0)).sum()).collect();
    for f in prey {
        if fits.of(f).kill_rate <= 0.0 {
            fits.of_mut(f).kill_rate = 1e-4;
        }
    }
    for round in 0..8 {
        let mut mod_k = vec![0.0; prey.len()];
        for l in &logs {
            let out = run_d2b(&p, fits, med, gdir, l, None, 240, Draws::Mean);
            for (i, f) in prey.iter().enumerate() {
                mod_k[i] += out.get(&240).and_then(|r| r.get(&format!("{}.killed", f.name()))).copied().unwrap_or(0.0);
            }
        }
        if !quiet {
            println!("fit,kills,round {round}: {}", prey.iter().enumerate().map(|(i, f)| format!("{} sim {:.0} model {:.1} rate {:.3e}", f.name(), sim_k[i], mod_k[i], fits.of(*f).kill_rate)).collect::<Vec<_>>().join("; "));
        }
        for (i, f) in prey.iter().enumerate() {
            let k = if mod_k[i] > 1e-9 { (sim_k[i] / mod_k[i]).clamp(0.2, 5.0) } else { 3.0 };
            fits.of_mut(*f).kill_rate *= k;
        }
    }
    if !quiet {
        println!("fit,strikes_per_kill={:.2} ({strikes:.0} strikes / {kills:.0} kills, d2 m120-240, 8 seeds)", fits.strikes_per_kill);
    }
}

fn d2b_cmd(args: &[String]) {
    let gdir = PathBuf::from(arg(args, "gen=").expect("gen=DIR"));
    let ljdir = PathBuf::from(arg(args, "lj=").expect("lj=DIR"));
    let twins: usize = arg(args, "twin=").map_or(0, |t| t.parse().unwrap());
    let mut fits = if args.iter().any(|a| a == "refit") { fit_all(&gdir, None, true) } else { Fits::recorded() };
    let med = median_stocks(&gdir);
    fit_kills(&mut fits, &med, &gdir, &ljdir, false);
    let keys = ["small", "littershredder.n", "capgnawer.n", "ripple-snail.n", "frondgrazer.n", "lj.kills", "littershredder.killed", "capgnawer.killed", "ripple-snail.killed", "frondgrazer.killed", "lj.intake", "lj.fullness", "lj.laid", "lj.bank", "lj.hatched"];
    for (arm, cfg_arm, obs_dir, minutes) in [
        ("d2", "d2", None, vec![150u32, 180, 210, 240]),
        ("d2b", "d2b", None, vec![150, 180, 210, 240]),
        ("d2b-1", "d2b-1", None, vec![150, 180, 210, 240]),
        ("d2b-open", "d2b-open", None, vec![240]),
        ("d2-chosen", "d2-chosen", None, vec![240]),
        ("main", "lj-main", None, vec![240]),
        ("d2", "d2", Some("obs"), vec![300, 360, 480, 600]),
        ("d2b", "d2b", Some("obs"), vec![300, 360, 480, 600]),
    ] {
        let p = params_of(cfg_arm);
        let logs = logs_of(&ljdir.join("mech"), arm);
        let obs: Vec<RunLog> = obs_dir.map_or(Vec::new(), |d| logs_of(&ljdir.join("mech").join(d), arm));
        println!("=== {arm}{}: {} seeds", if obs_dir.is_some() { " (observation)" } else { "" }, logs.len());
        let end = *minutes.iter().max().unwrap();
        let mut sim: BTreeMap<(u32, &str), Vec<f64>> = BTreeMap::new();
        let mut model: BTreeMap<(u32, &str), Vec<f64>> = BTreeMap::new();
        for log in &logs {
            let o = obs.iter().find(|x| x.seed == log.seed);
            if obs_dir.is_some() && o.is_none() {
                continue;
            }
            for &mi in &minutes {
                let sr = d2b_sim(log, o, mi);
                for k in keys {
                    sim.entry((mi, k)).or_default().push(sr.get(k).copied().unwrap_or(f64::NAN));
                }
            }
            let runs = twins.max(1);
            for t in 0..runs {
                let draws = if twins > 0 { Draws::Twin(Rng::new(u64::from(log.seed) * 7919 + t as u64)) } else { Draws::Mean };
                let out = run_d2b(&p, &fits, &med, &gdir, log, o, end, draws);
                for &mi in &minutes {
                    for k in keys {
                        model.entry((mi, k)).or_default().push(out.get(&mi).and_then(|r| r.get(k)).copied().unwrap_or(f64::NAN));
                    }
                }
            }
        }
        for k in keys {
            let mut line = format!("  {k:<22}");
            for &mi in &minutes {
                let sv = sim.get(&(mi, k)).cloned().unwrap_or_default();
                let mv = model.get(&(mi, k)).cloned().unwrap_or_default();
                line += &format!(" | m{mi}: sim {} model {} {}", fmt_spread(&sv), fmt_spread(&mv), verdict(&sv, &mv, 1.5));
            }
            println!("{line}");
        }
    }
}

// ---------------------------------------------------------------------------------------
// First answers.

/// One hatchling's life history under `p`, `fits`, fixed stocks and `n_own − 1` neighbours of
/// its lineage, minutes from hatching: maturation (through `birth_body`), the first tick it is
/// in surplus, its first clutch; the clutches after it and their eggs, to `horizon_min`. With
/// `ceiling`, the clock is off and food is under its mouth (ψ = 1): the young_budget probe.
#[derive(Clone, Copy, Debug, Default)]
struct History {
    mature: f64,
    eligible: f64,
    first: f64,
    clutches: f64,
    eggs: f64,
    eggs_first: f64,
    last: f64,
    intake_raw_per_day: f64,
    died: f64,
}

fn history(p: &cubarium_voxel_sim::params::Params, fits: &Fits, f: Founder, supply: &Supply, n_own: f64, ceiling: bool, horizon_min: f64) -> History {
    let mut p = p.clone();
    {
        let ph = &mut p.fauna.founders[f.index()];
        ph.core.reproduction.incubation_s = 1e12;
        ph.core.lifespan_median_s = 0.0;
    }
    let mut fits = fits.clone();
    if ceiling {
        p.world.day.mode = cubarium_voxel::weather::DayMode::Off;
        for g in Food::ALL {
            fits.of_mut(f).response[g.index()] = Some(fits::Response::holling(1.0, 0.0));
        }
        fits.of_mut(f).motor_effort = 0.0;
    }
    let mut m = FaunaModel::new(&p, fits, 1.0);
    for g in Founder::ALL {
        m.enabled[g.index()] = g == f;
    }
    m.extra_bodies[f.index()] = (n_own - 1.0).max(0.0);
    let sc = p.fauna.founder(f).core;
    // Hatched ten minutes into the world's first day, as the founders' first clutches were.
    m.t_s = 600.0;
    m.add(f, Cohort { native: true, ..Cohort::new(1.0, sc.body_min, (sc.reproduction.egg_organic - sc.body_min).max(0.0)) });
    let t0 = m.t_s;
    let mut h = History { mature: f64::NAN, eligible: f64::NAN, first: f64::NAN, died: f64::NAN, ..History::default() };
    let rule = sc.reproduction;
    while m.t_s - t0 < horizon_min * 60.0 {
        m.step(supply);
        let min = (m.t_s - t0) / 60.0;
        let Some(c) = m.pops[f.index()].first() else {
            h.died = min;
            break;
        };
        if h.mature.is_nan() && c.b.body >= sc.birth_body {
            h.mature = min;
        }
        if h.eligible.is_nan() && c.b.body >= sc.birth_body && c.b.reserve >= rule.surplus_reserve(&sc) {
            h.eligible = min;
        }
        let l = &m.ledger[f.index()];
        if l.clutches > h.clutches + 1e-9 {
            if h.first.is_nan() {
                h.first = min;
                h.eggs_first = l.laid;
            }
            h.clutches = l.clutches;
            h.eggs = l.laid;
            h.last = min;
        }
    }
    let l = &m.ledger[f.index()];
    let days = (m.t_s - t0) / 7200.0;
    h.intake_raw_per_day = l.swallowed.iter().sum::<f64>() / days.max(1e-9);
    h
}

/// The generation time: an egg's incubation plus its hatchling's age at its first clutch.
fn generation(p: &cubarium_voxel_sim::params::Params, f: Founder, h: &History) -> f64 {
    p.fauna.founder(f).core.reproduction.incubation_s / 60.0 + h.first
}

/// The median D1b stocks over minutes 20-240 (all arms): the supply a field probe stands in.
fn field_supply(gdir: &Path) -> Supply {
    let med = median_stocks(gdir);
    let col = |k: usize| median(&mut med.iter().filter(|(m, _)| **m >= 20).map(|(_, v)| [v.0, v.1, v.2, v.3][k]).collect::<Vec<_>>());
    let mut s = Supply::default();
    s.litter = col(0);
    s.film = col(1);
    s.foliage[cubarium_voxel_flora::Species::Glowcap.index()] = col(2);
    s.foliage[cubarium_voxel_flora::Species::Springturf.index()] = col(3);
    s
}

/// A knob scaled by `s`, as `path=value` sets on patch.
fn knob_sets(p: &cubarium_voxel_sim::params::Params, f: Founder, knob: &str, s: f64) -> Vec<String> {
    let ph = p.fauna.founder(f);
    let sc = ph.core;
    let n = f.name();
    let pre = format!("fauna.founders.{n}");
    let diet: Vec<Food> = ph.diet.foods().collect();
    match knob {
        "gut_throughput" => {
            let mut v: Vec<String> = diet.iter().map(|g| format!("{pre}.gut.digestion_s.{}={}", g.name(), ph.gut.digestion_s[g.index()] / s)).collect();
            v.push(format!("{pre}.gut.bite_per_body_s={}", ph.gut.bite_per_body_s * s));
            v
        }
        "yield" => diet.iter().map(|g| format!("{pre}.yield_by_food.{}={}", g.name(), (ph.yield_for(*g) * s).min(1.0))).collect(),
        "stomach_k" => vec![format!("{pre}.gut.stomach_k={}", ph.gut.stomach_k * s), format!("{pre}.gut.bite_per_body_s={}", ph.gut.bite_per_body_s * s)],
        "birth_body" => vec![format!("{pre}.core.birth_body={}", sc.birth_body * s)],
        "hatchling" => vec![
            format!("{pre}.core.body_min={}", sc.body_min * s),
            format!("{pre}.core.reproduction.egg_organic={}", sc.reproduction.egg_organic * s),
        ],
        "growth_max" => vec![format!("{pre}.core.growth_max_per_s={}", sc.growth_max_per_s * s)],
        "mass_exponent" => vec![format!("{pre}.gut.mass_exponent={}", (ph.gut.mass_exponent * s).clamp(0.05, 1.0))],
        _ => vec![],
    }
}

fn answers_cmd(args: &[String]) {
    let gdir = PathBuf::from(arg(args, "gen=").expect("gen=DIR"));
    let ljdir = PathBuf::from(arg(args, "lj=").expect("lj=DIR"));
    let mut fits = Fits::recorded();
    let med = median_stocks(&gdir);
    fit_kills(&mut fits, &med, &gdir, &ljdir, true);
    let supply = field_supply(&gdir);
    let base = params_of("branch");
    println!("=== Q1: generation time (minutes); field stocks litter {:.0}, film {:.2}, caps {:.1}; neighbours 12", supply.litter, supply.film, supply.foliage[cubarium_voxel_flora::Species::Glowcap.index()]);
    for f in DETRITIVORES {
        let ceil = history(&base, &fits, f, &supply, 1.0, true, 900.0);
        let field = history(&base, &fits, f, &supply, 12.0, false, 900.0);
        let sc = base.fauna.founder(f).core;
        let inc = sc.reproduction.incubation_s / 60.0;
        for (name, h) in [("ceiling", ceil), ("field", field)] {
            println!(
                "{},{name}: maturation {:.0} + to surplus {:.0} + hold/wait to first clutch {:.0} + incubation {:.1} = generation {:.0}; clutches after: every {:.0} min, eggs first {:.0} / mean {:.2}; raw intake {:.3}/day",
                f.name(),
                h.mature,
                h.eligible - h.mature,
                h.first - h.eligible,
                inc,
                generation(&base, f, &h),
                if h.clutches > 1.0 { (h.last - h.first) / (h.clutches - 1.0) } else { f64::NAN },
                h.eggs_first,
                if h.clutches > 0.0 { h.eggs / h.clutches } else { f64::NAN },
                h.intake_raw_per_day,
            );
        }
        // The knob table: the scale that brings the field generation to 120 and to 60 min.
        for knob in ["gut_throughput", "yield", "stomach_k", "birth_body", "hatchling", "growth_max", "mass_exponent"] {
            let g_at = |s: f64| -> (f64, History) {
                let mut sets = knob_sets(&base, f, knob, s);
                sets.extend(arm_config("branch").1);
                match config::scenario(arm_config("branch").0, &sets) {
                    Ok(p) => {
                        let h = history(&p, &fits, f, &supply, 12.0, false, 900.0);
                        (generation(&p, f, &h), h)
                    }
                    Err(_) => (f64::NAN, History::default()),
                }
            };
            let grid: Vec<f64> = (0..=36).map(|k| 0.1 * 1.2f64.powi(k)).collect();
            let vals: Vec<(f64, f64)> = grid.iter().map(|&s| (s, g_at(s).0)).collect();
            let mut line = format!("{},knob {knob}:", f.name());
            for target in [120.0, 60.0] {
                // The scale nearest 1 at which the generation crosses the target.
                let mut found: Option<f64> = None;
                for w in vals.windows(2) {
                    let ((s0, g0), (s1, g1)) = (w[0], w[1]);
                    if !(g0.is_finite() && g1.is_finite()) {
                        continue;
                    }
                    if (g0 - target) * (g1 - target) <= 0.0 {
                        let (mut lo, mut hi) = (s0, s1);
                        for _ in 0..20 {
                            let mid = (lo * hi).sqrt();
                            let gm = g_at(mid).0;
                            if (g0 - target) * (gm - target) <= 0.0 { hi = mid } else { lo = mid }
                        }
                        let s = (lo * hi).sqrt();
                        if found.is_none_or(|x: f64| (s.ln()).abs() < (x.ln()).abs()) {
                            found = Some(s);
                        }
                    }
                }
                match found {
                    Some(s) => {
                        let (g, h) = g_at(s);
                        line += &format!(" | G={target:.0}: x{s:.2} ({}) gives {g:.0} min, maturation {:.0}, first clutch {:.0} eggs, raw intake {:.3}/day", knob_sets(&base, f, knob, s).join(" "), h.mature, h.eggs_first, h.intake_raw_per_day);
                    }
                    None => {
                        let best = vals.iter().filter(|v| v.1.is_finite()).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                        line += &format!(" | G={target:.0}: unreachable (fastest {:.0} min at x{:.2})", best.map_or(f64::NAN, |b| b.1), best.map_or(f64::NAN, |b| b.0));
                    }
                }
            }
            println!("{line}");
        }
    }
    let _ = ljdir;
}

#[allow(dead_code)]
fn unused(_: &Path, _: PathBuf, _: BTreeMap<u8, u8>, _: Rng, _: Draws, _: fits::Response, _: model::Census) {}
