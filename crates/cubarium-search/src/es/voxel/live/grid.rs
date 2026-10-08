//! **The big-lever grid** (`design/handoffs/lever-grid-tool-2026-10-03.md`): a matrix of
//! master scales (animal metabolism `m` × plant productivity `p` by default, any registry
//! override as a further axis) × fixed worlds, each cell × seed run once and booked to
//! `cells.jsonl`, then summarised per cell × lineage (median and range over seeds) into
//! `summary.tsv` and a compact `summary.md` of median `B` / `R̂` per lineage.
//!
//! - **`m`** multiplies every lineage's daily need, `fauna.founders.*.core.maintenance_per_s`;
//!   walking (`motor_respiration_per_s`) follows it through the fauna's
//!   `motor_follows_upkeep` tie, as the search ties it.
//! - **`p`** multiplies plant production ([`P_PATHS`]): fixation (`assimilation`, the vine's
//!   too; the glowcap's substrate and seep uptake, its only income), the growth caps
//!   (`foliage_rate`, `wood_rate`, the vine's `regrow_rate`) and what the reserve is moved
//!   out into (`nectar_per_s`, `fruit_per_s`, `propagule_rate`), so nectar, fruit and seed
//!   follow production instead of staying capped at today's rates. Upkeep, senescence,
//!   caps and the mineral side are not production and stay put.
//! - Each cell is a genome over the world file's `[flora]`/`[fauna]` (its `[base]`), one
//!   gene per axis level, decoded by the search's own decode, ties included.
//! - The worlds are picked once under the x1 ecology: seed `b` is used when the founding
//!   loop accepts it as itself (not a neighbour it moved on to), so the worlds are
//!   distinct; every cell then founds exactly those worlds (`Seeds::Asked`).

use std::collections::{BTreeMap, HashSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use cubarium_voxel_fauna::{Departure, Founder};
use cubarium_voxel_flora::Species;
use serde::{Deserialize, Serialize};

use super::ecology::EcologyParams;
use super::search::{EvalRun, EvalSpec, brain_driver, check_mass};
use super::threads::{ThreadBudget, parallel_shared};
use super::trainer::parallel;
use super::window::TICKS_PER_MIN;
use super::world;
use crate::es::voxel::EpisodeDriver;

/// What `m` multiplies.
pub const M_PATHS: [&str; 1] = ["fauna.founders.*.core.maintenance_per_s"];
/// What follows `m` by the fauna's tie (not a gene path: one gene on both is refused).
pub const M_FOLLOWS: [&str; 1] = ["fauna.founders.*.motor_respiration_per_s"];
/// What `p` multiplies (module docs).
/// `flora.*.assimilation` covers the vine's own (`flora.latticevine.assimilation`):
/// listing it again would multiply it twice.
pub const P_PATHS: [&str; 9] = [
    "flora.*.assimilation",
    "flora.*.substrate_uptake_per_s",
    "flora.*.dark_uptake_per_s",
    "flora.*.foliage_rate",
    "flora.*.wood_rate",
    "flora.*.nectar_per_s",
    "flora.*.fruit_per_s",
    "flora.*.propagule_rate",
    "flora.latticevine.regrow_rate",
];
/// The default levels of `m` and `p`.
pub const DEFAULT_LEVELS: &str = "0.5,0.71,1,1.41,2";
/// The grid's window, simulated minutes: alive counts are read at window ends.
pub const WINDOW_MIN: u64 = 30;

/// The paths a named scale multiplies.
pub fn named_scale(name: &str) -> Option<Vec<String>> {
    let paths: &[&str] = match name {
        "m" => &M_PATHS,
        "p" => &P_PATHS,
        _ => return None,
    };
    Some(paths.iter().map(|s| s.to_string()).collect())
}

/// One change a level makes.
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// Multiply every leaf the globs match.
    Scale { paths: Vec<String>, by: f64 },
    /// Set one path (a glob) to a value.
    Set { path: String, to: f64 },
}

/// One level of an axis.
#[derive(Clone, Debug, PartialEq)]
pub struct Level {
    pub label: String,
    pub edits: Vec<Edit>,
}

/// One axis of the grid.
#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    pub name: String,
    pub levels: Vec<Level>,
}

/// `0.5,1,2` as numbers.
pub fn parse_values(s: &str) -> Result<Vec<f64>, String> {
    s.split(',')
        .map(|v| v.trim().parse::<f64>().map_err(|e| format!("level {v}: {e}")))
        .collect()
}

fn label(v: f64) -> String {
    format!("{v}")
}

impl Axis {
    /// A named scale (`m`, `p`) at these multipliers.
    pub fn named(name: &str, values: &[f64]) -> Result<Axis, String> {
        let paths = named_scale(name).ok_or_else(|| format!("no named scale `{name}` (m, p)"))?;
        Ok(Axis {
            name: name.into(),
            levels: values
                .iter()
                .map(|&by| Level { label: label(by), edits: vec![Edit::Scale { paths: paths.clone(), by }] })
                .collect(),
        })
    }

    /// `NAME:LEVEL|LEVEL|…`. A level is a number (NAME a named scale), or `;`-separated
    /// registry overrides: `PATH=V` sets, `PATH=xV` multiplies (`fauna.founders.*.gut.k=x2`).
    pub fn parse(spec: &str) -> Result<Axis, String> {
        let (name, levels) = spec
            .split_once(':')
            .ok_or_else(|| format!("--axis takes NAME:LEVEL|LEVEL…, not {spec}"))?;
        let name = name.trim();
        let mut out = Vec::new();
        for lv in levels.split('|') {
            let lv = lv.trim();
            if let Ok(by) = lv.parse::<f64>() {
                let paths = named_scale(name).ok_or_else(|| {
                    format!("axis `{name}`: a bare number needs a named scale (m, p); write PATH=xV")
                })?;
                out.push(Level { label: label(by), edits: vec![Edit::Scale { paths, by }] });
                continue;
            }
            let mut edits = Vec::new();
            for e in lv.split(';') {
                let (path, v) = e
                    .split_once('=')
                    .ok_or_else(|| format!("axis `{name}`: `{e}` is not PATH=VALUE"))?;
                let (path, v) = (path.trim().to_string(), v.trim());
                if let Some(by) = v.strip_prefix('x') {
                    let by = by.parse::<f64>().map_err(|err| format!("{e}: {err}"))?;
                    edits.push(Edit::Scale { paths: vec![path], by });
                } else {
                    let to = v.parse::<f64>().map_err(|err| format!("{e}: {err}"))?;
                    edits.push(Edit::Set { path, to });
                }
            }
            out.push(Level { label: lv.to_string(), edits });
        }
        if out.is_empty() {
            return Err(format!("axis `{name}` has no levels"));
        }
        Ok(Axis { name: name.into(), levels: out })
    }
}

/// One cell: a level of every axis.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    /// `m=0.5 p=1`.
    pub label: String,
    /// `(axis, level label)`, in axis order.
    pub levels: Vec<(String, String)>,
    /// `(axis, its level's edits)`.
    pub edits: Vec<(String, Vec<Edit>)>,
}

/// The cartesian product, the first axis outermost.
pub fn cells(axes: &[Axis]) -> Vec<Cell> {
    let mut out = vec![Cell { label: String::new(), levels: Vec::new(), edits: Vec::new() }];
    for a in axes {
        let mut next = Vec::new();
        for c in &out {
            for l in &a.levels {
                let mut c = c.clone();
                c.levels.push((a.name.clone(), l.label.clone()));
                c.edits.push((a.name.clone(), l.edits.clone()));
                next.push(c);
            }
        }
        out = next;
    }
    for c in &mut out {
        c.label = c.levels.iter().map(|(a, l)| format!("{a}={l}")).collect::<Vec<_>>().join(" ");
    }
    out
}

/// The search-file text of a cell over `base` (a config file whose `[flora]`/`[fauna]`
/// it decodes over; the code defaults without).
pub fn cell_search_text(cell: &Cell, base: Option<&Path>) -> Result<String, String> {
    let mut doc = toml::Table::new();
    if let Some(b) = base {
        let mut t = toml::Table::new();
        t.insert("file".into(), toml::Value::String(b.display().to_string()));
        doc.insert("base".into(), toml::Value::Table(t));
    }
    let mut genes = Vec::new();
    for (axis, edits) in &cell.edits {
        for (k, e) in edits.iter().enumerate() {
            let mut g = toml::Table::new();
            let name = if edits.len() == 1 { axis.clone() } else { format!("{axis}_{k}") };
            g.insert("name".into(), toml::Value::String(name));
            g.insert("tie".into(), toml::Value::String("all".into()));
            match e {
                Edit::Scale { paths, by } => {
                    if !(*by > 0.0 && by.is_finite()) {
                        return Err(format!("axis {axis}: a multiplier must be positive, not {by}"));
                    }
                    g.insert(
                        "path".into(),
                        toml::Value::Array(paths.iter().map(|p| toml::Value::String(p.clone())).collect()),
                    );
                    g.insert("range".into(), toml::Value::Array(vec![(by.min(1.0) * 0.5).into(), (by.max(1.0) * 2.0).into()]));
                    g.insert("value".into(), (*by).into());
                }
                Edit::Set { path, to } => {
                    g.insert("path".into(), toml::Value::String(path.clone()));
                    g.insert("mode".into(), toml::Value::String("value".into()));
                    let w = 1.0 + to.abs();
                    g.insert("range".into(), toml::Value::Array(vec![(to - w).into(), (to + w).into()]));
                    g.insert("value".into(), (*to).into());
                }
            }
            genes.push(toml::Value::Table(g));
        }
    }
    if genes.is_empty() {
        return Err("a cell with no edits (give at least one axis)".into());
    }
    doc.insert("gene".into(), toml::Value::Array(genes));
    toml::to_string(&doc).map_err(|e| format!("cannot write the cell's genome: {e}"))
}

/// A cell's ecology.
pub fn cell_ecology(cell: &Cell, base: Option<&Path>) -> Result<EcologyParams, String> {
    EcologyParams::parse(&cell_search_text(cell, base)?).map_err(|e| format!("cell {}: {e}", cell.label))
}

/// The x1 ecology over `base` (the worlds are picked under it).
pub fn neutral_ecology(base: Option<&Path>) -> Result<EcologyParams, String> {
    let cell = &cells(&[Axis::named("m", &[1.0])?])[0];
    cell_ecology(cell, base)
}

/// One lineage's result in one world.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LineageRecord {
    pub name: String,
    pub founded: usize,
    pub alive_60: Option<usize>,
    pub alive_120: Option<usize>,
    pub alive_end: usize,
    pub births: u64,
    pub starved: u64,
    /// The balance measures (`live/balance.rs`) over the run; `None` where there was
    /// nothing to measure (or `B` infinite). `days_to_empty` is `None` at `B ≥ 1` too.
    pub b: Option<f64>,
    pub days_to_empty: Option<f64>,
    pub surplus: Option<f64>,
    pub r_hat: Option<f64>,
    pub access: Option<f64>,
    pub sated_share: Option<f64>,
    /// Departures since the baseline by cause (`starved`, `drowned`, `aged`,
    /// `killed_by_<lineage>`, `removed`), the nonzero ones.
    #[serde(default)]
    pub deaths: Vec<(String, u64)>,
}

/// One plant species in one world.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SpeciesRecord {
    pub name: String,
    pub stands_start: usize,
    pub stands_end: usize,
    pub foliage_start: f64,
    pub foliage_end: f64,
}

/// One cell × seed: a line of `cells.jsonl`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CellRecord {
    #[serde(default)]
    pub lost_required: usize,
    #[serde(default)]
    pub lost_required_names: Vec<String>,
    #[serde(default)]
    pub required_roster: super::accounting::RequiredRoster,
    #[serde(default)]
    pub presence_end: super::accounting::Presence,
    pub cell: String,
    pub levels: Vec<(String, String)>,
    pub world_seed: u64,
    /// Whether this cell's ecology still passes the founding's acceptance on this world.
    pub accepted: bool,
    pub hours: f64,
    pub minutes_run: f64,
    pub stopped: Option<String>,
    pub wall_s: f64,
    pub lineages: Vec<LineageRecord>,
    pub species: Vec<SpeciesRecord>,
    /// The start set it ran on ([`super::start::StartSet::label`]; empty is `e0`).
    #[serde(default)]
    pub start: String,
    /// How it started: burn-in, frozen genes, ledger residuals.
    #[serde(default)]
    pub start_record: Option<super::start::StartRecord>,
    /// The ledger baselines: the burn-in's end and every window boundary.
    #[serde(default)]
    pub windows: Vec<super::start::Baseline>,
}

impl CellRecord {
    /// The start set it ran on, `e0` for a record from before starts.
    pub fn start_label(&self) -> &str {
        if self.start.is_empty() { "e0" } else { &self.start }
    }
}

fn finite(x: f64) -> Option<f64> {
    x.is_finite().then_some(x)
}

/// The seed list: picked worlds and the seeds skipped.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SeedList {
    pub world: String,
    /// The first seed tried.
    pub first: u64,
    pub seeds: Vec<u64>,
    /// `(seed, why)` for each seed the generator refused or moved off.
    pub skipped: Vec<(u64, String)>,
}

/// Pick `n` distinct worlds from `first` on under `eco`: a seed is used when the founding
/// loop accepts it as itself. Gives up after `8 n` tries with what it has.
pub fn pick_seeds(
    cfg: &cubarium_voxel::Config,
    eco: &EcologyParams,
    founders: [usize; Founder::COUNT],
    n: usize,
    first: u64,
    threads: usize,
    world_name: &str,
) -> SeedList {
    let mut list = SeedList { world: world_name.into(), first, ..Default::default() };
    let mut next = first;
    let limit = first + 8 * n as u64;
    while list.seeds.len() < n && next < limit {
        // No more founding probes at once than worlds still wanted (or threads).
        let width = threads.max(1).min(n - list.seeds.len()) as u64;
        let batch: Vec<u64> = (next..(next + width).min(limit)).collect();
        next += batch.len() as u64;
        let got = parallel(batch.clone(), threads, |b| {
            let (_, info) = world::found_world(cfg, b, founders, eco);
            (info.seed, info.accepted)
        });
        for (b, (seed, accepted)) in batch.into_iter().zip(got) {
            if list.seeds.len() >= n {
                break;
            }
            if seed != b {
                list.skipped.push((b, format!("refused; the founding moved to seed {seed}")));
            } else if !accepted {
                list.skipped.push((b, "refused by the habitat check".into()));
            } else {
                list.seeds.push(b);
            }
        }
    }
    list
}

/// Run one cell on one world.
pub fn run_cell(
    cell: &Cell,
    eco: &EcologyParams,
    seed: u64,
    hours: f64,
    drivers: &[(Founder, EpisodeDriver)],
    spec: &EvalSpec,
    budget: Option<&std::sync::Arc<ThreadBudget>>,
) -> CellRecord {
    let mut run = EvalRun::begin(eco, seed, drivers, spec);
    if let Some(b) = budget {
        run.share_threads(b.clone());
    }
    let starved = |r: &EvalRun| -> [u64; Founder::COUNT] {
        let l = r.sim().fauna().view().ledger;
        std::array::from_fn(|i| l.deaths_by_founder_cause[i][Departure::Starved.index()])
    };
    let plants = |r: &EvalRun| -> ([usize; Species::COUNT], [f64; Species::COUNT]) {
        let (mut n, mut f) = ([0usize; Species::COUNT], [0.0; Species::COUNT]);
        for s in r.sim().flora().view().stands {
            n[s.species.index()] += 1;
            f[s.species.index()] += s.foliage;
        }
        (n, f)
    };
    let departed = |r: &EvalRun| r.sim().fauna().view().ledger.deaths_by_founder_cause;
    let departed0 = departed(&run);
    let starved0 = starved(&run);
    let (stands0, foliage0) = plants(&run);
    let accepted = run.info().accepted;
    let tables = vec![None; Founder::COUNT];
    let zeros = vec![0usize; Founder::COUNT];
    let mut alive: BTreeMap<u64, [usize; Founder::COUNT]> = BTreeMap::new();
    while !run.done() {
        run.window(&tables, &zeros, 0);
        let t = run.elapsed();
        if t.is_multiple_of(TICKS_PER_MIN) {
            alive.insert(t / TICKS_PER_MIN, run.population());
        }
    }
    let starved1 = starved(&run);
    let departed1 = departed(&run);
    let (stands1, foliage1) = plants(&run);
    let minutes_run = run.elapsed() as f64 / TICKS_PER_MIN as f64;
    let e = run.finish();
    let lineages = e
        .lineages
        .iter()
        .map(|o| {
            let i = o.founder.index();
            let m = &o.balance;
            LineageRecord {
                name: o.lineage.into(),
                founded: o.founded,
                alive_60: alive.get(&60).map(|p| p[i]),
                alive_120: alive.get(&120).map(|p| p[i]),
                alive_end: o.alive_end,
                births: o.births,
                starved: starved1[i] - starved0[i],
                b: finite(m.energy_balance),
                days_to_empty: m.days_to_empty.and_then(finite),
                surplus: finite(m.surplus_per_adult_day),
                r_hat: finite(m.r_hat),
                access: finite(m.access),
                sated_share: finite(m.sated_share),
                deaths: Departure::ALL
                    .iter()
                    .map(|d| (d.name().to_string(), departed1[i][d.index()] - departed0[i][d.index()]))
                    .filter(|(_, n)| *n > 0)
                    .collect(),
            }
        })
        .collect();
    let species = Species::ALL
        .iter()
        .filter(|s| stands0[s.index()] + stands1[s.index()] > 0)
        .map(|s| {
            let i = s.index();
            SpeciesRecord {
                name: s.name().into(),
                stands_start: stands0[i],
                stands_end: stands1[i],
                foliage_start: foliage0[i],
                foliage_end: foliage1[i],
            }
        })
        .collect();
    CellRecord {
        lost_required: e.lost_required,
        lost_required_names: e.lost_required_names,
        required_roster: e.required_roster,
        presence_end: e.presence_end,
        cell: cell.label.clone(),
        levels: cell.levels.clone(),
        world_seed: seed,
        accepted,
        hours,
        minutes_run,
        stopped: e.stopped,
        wall_s: e.wall_s,
        lineages,
        species,
        start: e.start.set.clone(),
        start_record: Some(e.start),
        windows: e.baselines,
    }
}

/// Every record in `cells.jsonl` (none if it is missing; a torn last line is dropped).
pub fn read_records(path: &Path) -> Vec<CellRecord> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// The `(cell, seed)` jobs `done` has not booked at `hours`.
pub fn pending(cells: &[Cell], seeds: &[u64], hours: f64, done: &[CellRecord]) -> Vec<(usize, u64)> {
    let have: HashSet<(String, u64)> = done
        .iter()
        .filter(|r| r.hours == hours)
        .map(|r| (r.cell.clone(), r.world_seed))
        .collect();
    (0..cells.len())
        .flat_map(|c| seeds.iter().map(move |&s| (c, s)))
        .filter(|(c, s)| !have.contains(&(cells[*c].label.clone(), *s)))
        .collect()
}

/// **The longest runs first**, so the short ones fill the cores at the end instead of a few
/// long ones holding a near-empty machine (the tail): the plant scale `p` highest first, then
/// the metabolism scale `m` lowest first (more plants and more animals step slower: on a
/// c7g, 2 h at p 0.5/1/2 took 1.1/1.5/2.8 h, and m 6 ran ~20 % faster than m 1). A grid
/// without those axes keeps its order; the sort is stable, so seeds stay in order.
pub fn longest_first(cells: &[Cell], jobs: &mut [(usize, u64)]) {
    let level = |c: usize, axis: &str| {
        cells[c].levels.iter().find(|(a, _)| a == axis).and_then(|(_, l)| l.parse::<f64>().ok())
    };
    jobs.sort_by(|a, b| {
        let p = |c| level(c, "p").unwrap_or(1.0);
        let m = |c| level(c, "m").unwrap_or(1.0);
        p(b.0).total_cmp(&p(a.0)).then(m(a.0).total_cmp(&m(b.0)))
    });
}

/// The median and range of the present values; `None` without any.
pub fn median_range(mut v: Vec<f64>) -> Option<(usize, f64, f64, f64)> {
    v.retain(|x| x.is_finite());
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    let med = if n % 2 == 1 { v[n / 2] } else { 0.5 * (v[n / 2 - 1] + v[n / 2]) };
    Some((n, med, v[0], v[n - 1]))
}

/// The lineage metrics the summary carries, by name.
pub const LINEAGE_METRICS: [&str; 13] = [
    "founded", "alive_60", "alive_120", "alive_end", "births", "starved", "deaths", "B", "days_to_empty", "surplus",
    "r_hat", "access", "sated_share",
];
/// The plant metrics.
pub const SPECIES_METRICS: [&str; 4] = ["stands_start", "stands_end", "foliage_start", "foliage_end"];

fn lineage_metric(l: &LineageRecord, m: &str) -> Option<f64> {
    match m {
        "founded" => Some(l.founded as f64),
        "deaths" => Some(l.deaths.iter().filter(|(c, _)| c != "removed").map(|(_, n)| *n as f64).sum()),
        "alive_60" => l.alive_60.map(|x| x as f64),
        "alive_120" => l.alive_120.map(|x| x as f64),
        "alive_end" => Some(l.alive_end as f64),
        "births" => Some(l.births as f64),
        "starved" => Some(l.starved as f64),
        "B" => l.b,
        "days_to_empty" => l.days_to_empty,
        "surplus" => l.surplus,
        "r_hat" => l.r_hat,
        "access" => l.access,
        "sated_share" => l.sated_share,
        _ => None,
    }
}

fn species_metric(s: &SpeciesRecord, m: &str) -> Option<f64> {
    match m {
        "stands_start" => Some(s.stands_start as f64),
        "stands_end" => Some(s.stands_end as f64),
        "foliage_start" => Some(s.foliage_start),
        "foliage_end" => Some(s.foliage_end),
        _ => None,
    }
}

/// One summary row: a cell's median and range of one metric of one lineage or species.
#[derive(Clone, Debug, PartialEq)]
pub struct SummaryRow {
    pub cell: String,
    pub levels: Vec<(String, String)>,
    /// `lineage` or `species`.
    pub kind: &'static str,
    pub name: String,
    pub metric: &'static str,
    pub n: usize,
    pub median: f64,
    pub min: f64,
    pub max: f64,
}

/// Every cell's rows, cells in `cells` order (only cells with records), lineages in
/// [`Founder::ALL`] order and species in [`Species::ALL`] order.
pub fn summarise(cells: &[Cell], records: &[CellRecord]) -> Vec<SummaryRow> {
    let mut out = Vec::new();
    for c in cells {
        let mine: Vec<&CellRecord> = records.iter().filter(|r| r.cell == c.label).collect();
        if mine.is_empty() {
            continue;
        }
        for f in Founder::ALL {
            let ls: Vec<&LineageRecord> =
                mine.iter().flat_map(|r| r.lineages.iter().filter(|l| l.name == f.name())).collect();
            if ls.is_empty() {
                continue;
            }
            for m in LINEAGE_METRICS {
                let v = ls.iter().filter_map(|l| lineage_metric(l, m)).collect();
                let (n, median, min, max) = median_range(v).unwrap_or((0, f64::NAN, f64::NAN, f64::NAN));
                out.push(SummaryRow {
                    cell: c.label.clone(),
                    levels: c.levels.clone(),
                    kind: "lineage",
                    name: f.name().into(),
                    metric: m,
                    n,
                    median,
                    min,
                    max,
                });
            }
        }
        for s in Species::ALL {
            let ss: Vec<&SpeciesRecord> =
                mine.iter().flat_map(|r| r.species.iter().filter(|x| x.name == s.name())).collect();
            if ss.is_empty() {
                continue;
            }
            for m in SPECIES_METRICS {
                let v = ss.iter().filter_map(|x| species_metric(x, m)).collect();
                let (n, median, min, max) = median_range(v).unwrap_or((0, f64::NAN, f64::NAN, f64::NAN));
                out.push(SummaryRow {
                    cell: c.label.clone(),
                    levels: c.levels.clone(),
                    kind: "species",
                    name: s.name().into(),
                    metric: m,
                    n,
                    median,
                    min,
                    max,
                });
            }
        }
    }
    out
}

fn num(x: f64) -> String {
    if x.is_nan() {
        String::new()
    } else {
        format!("{x:.6}")
    }
}

/// `summary.tsv`: one row per cell × lineage-or-species × metric (long form).
pub fn summary_tsv(axes: &[Axis], rows: &[SummaryRow]) -> String {
    let mut s = String::from("cell");
    for a in axes {
        s.push_str(&format!("\t{}", a.name));
    }
    s.push_str("\tkind\tname\tmetric\tn\tmedian\tmin\tmax\n");
    for r in rows {
        s.push_str(&r.cell);
        for (_, l) in &r.levels {
            s.push_str(&format!("\t{l}"));
        }
        s.push_str(&format!(
            "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            r.kind,
            r.name,
            r.metric,
            r.n,
            num(r.median),
            num(r.min),
            num(r.max)
        ));
    }
    s
}

fn short(x: f64) -> String {
    if !x.is_finite() {
        "–".into()
    } else if x.abs() >= 100.0 {
        format!("{x:.0}")
    } else if x.abs() >= 10.0 {
        format!("{x:.1}")
    } else {
        format!("{x:.2}")
    }
}

/// `summary.md`: per lineage, median `B / R̂` over the first two axes (one matrix per
/// level of any further axes), then the plants' standing foliage at the end.
pub fn summary_md(axes: &[Axis], rows: &[SummaryRow], header: &str) -> String {
    let get = |cell: &str, kind: &str, name: &str, metric: &str| {
        rows.iter()
            .find(|r| r.cell == cell && r.kind == kind && r.name == name && r.metric == metric)
            .map_or(f64::NAN, |r| r.median)
    };
    let all = cells(axes);
    let (row_axis, col_axis) = (axes.first(), axes.get(1));
    let rest: Vec<Axis> = axes.iter().skip(2).cloned().collect();
    let rest_cells = if rest.is_empty() {
        vec![Cell { label: String::new(), levels: Vec::new(), edits: Vec::new() }]
    } else {
        cells(&rest)
    };
    // The cell label of (row level, col level, rest cell).
    let label_of = |r: Option<&Level>, c: Option<&Level>, rc: &Cell| -> String {
        let mut levels = Vec::new();
        if let (Some(a), Some(l)) = (row_axis, r) {
            levels.push(format!("{}={}", a.name, l.label));
        }
        if let (Some(a), Some(l)) = (col_axis, c) {
            levels.push(format!("{}={}", a.name, l.label));
        }
        levels.extend(rc.levels.iter().map(|(a, l)| format!("{a}={l}")));
        levels.join(" ")
    };
    let matrix = |title: &str, value: &dyn Fn(&str) -> String| -> String {
        let mut s = String::new();
        for rc in &rest_cells {
            if rest_cells.len() > 1 {
                s.push_str(&format!("\n{title}, {}\n\n", rc.label));
            } else {
                s.push_str(&format!("\n{title}\n\n"));
            }
            let rows_l: Vec<Option<&Level>> =
                row_axis.map_or(vec![None], |a| a.levels.iter().map(Some).collect());
            let cols_l: Vec<Option<&Level>> =
                col_axis.map_or(vec![None], |a| a.levels.iter().map(Some).collect());
            let corner = match (row_axis, col_axis) {
                (Some(r), Some(c)) => format!("{} \\ {}", r.name, c.name),
                (Some(r), None) => r.name.clone(),
                _ => String::new(),
            };
            s.push_str(&format!("| {corner} |"));
            for c in &cols_l {
                s.push_str(&format!(" {} |", c.map_or("", |l| l.label.as_str())));
            }
            s.push_str("\n|---|");
            for _ in &cols_l {
                s.push_str("---|");
            }
            s.push('\n');
            for r in &rows_l {
                s.push_str(&format!("| {} |", r.map_or("", |l| l.label.as_str())));
                for c in &cols_l {
                    let label = label_of(*r, *c, rc);
                    s.push_str(&format!(" {} |", value(&label)));
                }
                s.push('\n');
            }
        }
        s
    };
    let mut md = format!("# Lever grid\n\n{header}\n");
    let cells_with: HashSet<&str> = rows.iter().map(|r| r.cell.as_str()).collect();
    let _ = &all;
    for f in Founder::ALL {
        if !rows.iter().any(|r| r.kind == "lineage" && r.name == f.name()) {
            continue;
        }
        let name = f.name();
        md.push_str(&matrix(&format!("## {name}: median B / R̂"), &|cell| {
            if !cells_with.contains(cell) {
                return "·".into();
            }
            format!(
                "{} / {}",
                short(get(cell, "lineage", name, "B")),
                short(get(cell, "lineage", name, "r_hat"))
            )
        }));
    }
    md.push_str(&matrix("## Plants: median standing foliage at the end, all species", &|cell| {
        if !cells_with.contains(cell) {
            return "·".into();
        }
        let total: f64 = Species::ALL
            .iter()
            .map(|s| get(cell, "species", s.name(), "foliage_end"))
            .filter(|x| x.is_finite())
            .sum();
        short(total)
    }));
    md
}

/// The grid's whole run.
pub struct GridSpec {
    pub world: cubarium_voxel::Config,
    /// What the world was read from (a config file whose `[flora]`/`[fauna]` the cells
    /// decode over, or a preset's name).
    pub world_name: String,
    pub base: Option<PathBuf>,
    pub axes: Vec<Axis>,
    pub seeds: usize,
    pub first_seed: u64,
    pub hours: f64,
    pub threads: usize,
    /// Cores the worlds share once the queue is empty ([`super::threads`]); `1` keeps every
    /// world on one thread throughout.
    pub tail_cores: usize,
    pub water_every: u32,
    pub abort_animals: usize,
    pub centres: Vec<(Founder, PathBuf)>,
    pub out: PathBuf,
    /// Where every cell starts ([`super::start`]): E0 (today's founding) or a library.
    pub start: super::start::StartSet,
    pub burn_in: super::start::BurnIn,
    /// These worlds exactly (each founded as asked), instead of picking `seeds` from
    /// `first_seed`. A library's own seeds are used with a library.
    pub seed_list: Option<Vec<u64>>,
}

/// Run the grid: pick (or reread) the worlds, run every pending cell × seed (one simulation
/// thread per world while jobs are queued, `threads` worlds at once; once the queue is
/// empty the worlds still running share `tail_cores`), then write the summaries.
pub fn grid(spec: &GridSpec) -> Result<(), String> {
    let out = &spec.out;
    std::fs::create_dir_all(out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    cubarium_voxel::world::set_thread_override(1);
    let all = cells(&spec.axes);
    let ecologies: Vec<EcologyParams> =
        all.iter().map(|c| cell_ecology(c, spec.base.as_deref())).collect::<Result<_, _>>()?;
    let neutral = neutral_ecology(spec.base.as_deref())?;
    let founders = neutral.fauna().founder_counts();
    let drivers: Vec<(Founder, EpisodeDriver)> = spec
        .centres
        .iter()
        .map(|(f, p)| Ok((*f, brain_driver(*f, &p.display().to_string())?)))
        .collect::<Result<_, String>>()?;
    for e in &ecologies {
        check_mass(&drivers, e)?;
    }
    // The worlds: a library's own seeds, a list asked for, or picked (reread when this run
    // asked for the same ones).
    let seeds_path = out.join("seeds.json");
    let reread: Option<SeedList> = std::fs::read_to_string(&seeds_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .filter(|l: &SeedList| l.world == spec.world_name && l.first == spec.first_seed && l.seeds.len() >= spec.seeds);
    let given: Option<Vec<u64>> = match (&spec.start, &spec.seed_list) {
        (super::start::StartSet::Library(lib), Some(list)) => {
            if let Some(s) = list.iter().find(|s| !lib.seeds.contains(s)) {
                return Err(format!("seed {s} is not in the start library {}", lib.dir.display()));
            }
            Some(list.clone())
        }
        (super::start::StartSet::Library(lib), None) => Some(lib.seeds.iter().copied().take(spec.seeds).collect()),
        (super::start::StartSet::E0, Some(list)) => Some(list.clone()),
        (super::start::StartSet::E0, None) => None,
    };
    let list = match (given, reread) {
        (Some(seeds), _) => SeedList { world: spec.world_name.clone(), first: 0, seeds, skipped: Vec::new() },
        (None, Some(mut l)) => {
            l.seeds.truncate(spec.seeds);
            l
        }
        (None, None) => {
            let l = pick_seeds(&spec.world, &neutral, founders, spec.seeds, spec.first_seed, spec.threads, &spec.world_name);
            let text = serde_json::to_string_pretty(&l).map_err(|e| e.to_string())?;
            std::fs::write(&seeds_path, text).map_err(|e| format!("cannot write {}: {e}", seeds_path.display()))?;
            l
        }
    };
    if list.seeds.len() < spec.seeds && spec.seed_list.is_none() && !spec.start.established() {
        eprintln!("grid: only {} distinct accepted worlds found", list.seeds.len());
    }
    for (s, why) in &list.skipped {
        println!("grid: seed {s} skipped: {why}");
    }
    let cells_path = out.join("cells.jsonl");
    let label = spec.start.label();
    let done: Vec<CellRecord> = read_records(&cells_path).into_iter().filter(|r| r.start_label() == label).collect();
    let mut jobs = pending(&all, &list.seeds, spec.hours, &done);
    longest_first(&all, &mut jobs);
    println!(
        "grid: {} cells x {} worlds ({:?}), {} to run, {} already booked",
        all.len(),
        list.seeds.len(),
        list.seeds,
        jobs.len(),
        all.len() * list.seeds.len() - jobs.len()
    );
    let horizon = (spec.hours * 3600.0 * f64::from(cubarium_voxel_fauna::TICK_HZ)).round() as u64;
    let mut eval = EvalSpec::new(spec.world.clone(), horizon, WINDOW_MIN * TICKS_PER_MIN);
    eval.founders = founders;
    eval.driver_sources = spec.centres.iter().map(|(f, p)| (*f, p.display().to_string())).collect();
    eval.required_roster = Some(super::accounting::RequiredRoster::declared(&founders, &neutral.flora(spec.world.voxel_m)));
    eval.water_every = spec.water_every;
    eval.abort_animals = spec.abort_animals;
    eval.stop_on_loss = false;
    eval.asked_seed = true;
    eval.start = spec.start.clone();
    eval.burn_in = spec.burn_in;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cells_path)
        .map_err(|e| format!("cannot open {}: {e}", cells_path.display()))?;
    let file = Mutex::new(file);
    let budget = ThreadBudget::new(spec.tail_cores);
    let results: Vec<Result<(), String>> = parallel_shared(jobs, spec.threads, &budget, |(c, seed), b| {
        let r = run_cell(&all[c], &ecologies[c], seed, spec.hours, &drivers, &eval, Some(b));
        let line = serde_json::to_string(&r).map_err(|e| e.to_string())?;
        let mut f = file.lock().expect("cells.jsonl");
        writeln!(f, "{line}").map_err(|e| e.to_string())?;
        f.flush().map_err(|e| e.to_string())?;
        println!("grid: {} seed {seed} done in {:.0} s", r.cell, r.wall_s);
        Ok(())
    });
    results.into_iter().collect::<Result<(), String>>()?;
    write_summaries(spec, &all, &list)
}

/// `summary.tsv` and `summary.md` from what `cells.jsonl` holds for this grid's cells,
/// worlds and hours.
pub fn write_summaries(spec: &GridSpec, all: &[Cell], list: &SeedList) -> Result<(), String> {
    let out = &spec.out;
    let records: Vec<CellRecord> = read_records(&out.join("cells.jsonl"))
        .into_iter()
        .filter(|r| r.hours == spec.hours && list.seeds.contains(&r.world_seed) && r.start_label() == spec.start.label())
        .collect();
    let rows = summarise(all, &records);
    let tsv = out.join("summary.tsv");
    std::fs::write(&tsv, summary_tsv(&spec.axes, &rows)).map_err(|e| format!("cannot write {}: {e}", tsv.display()))?;
    let unaccepted = records.iter().filter(|r| !r.accepted).count();
    let stopped = records.iter().filter(|r| r.stopped.is_some()).count();
    let frozen: std::collections::BTreeSet<String> = records
        .iter()
        .filter_map(|r| r.start_record.as_ref())
        .flat_map(|s| s.frozen.iter().map(|c| c.path.clone()))
        .collect();
    let start = match &spec.start {
        super::start::StartSet::E0 => "**Start: E0**, today's founding under each cell's own ecology (the disclosed \
             early-community diagnostic; chorister, lanternjaw and loftstrider are readouts here)"
            .to_string(),
        super::start::StartSet::Library(lib) => format!(
            "**Start: {}**, every cell on the same loaded starts, rate-only (the established setting: \
             every lineage scored){}",
            lib.label(),
            if frozen.is_empty() {
                String::new()
            } else {
                format!("; frozen at the start's values: {}", frozen.into_iter().collect::<Vec<_>>().join(", "))
            }
        ),
    };
    let burn = records.iter().filter_map(|r| r.start_record.as_ref()).map(|s| s.burn_in_ticks).collect::<Vec<_>>();
    let burn_note = match (burn.iter().min(), burn.iter().max()) {
        (Some(lo), Some(hi)) if *hi > 0 => format!(
            " Burn-in {:.0}–{:.0} min (at least {:.0}, ending on the {:.0}-min window grid of the day clock); scores from its end.",
            *lo as f64 / TICKS_PER_MIN as f64,
            *hi as f64 / TICKS_PER_MIN as f64,
            spec.burn_in.min_ticks as f64 / TICKS_PER_MIN as f64,
            spec.burn_in.align_ticks as f64 / TICKS_PER_MIN as f64,
        ),
        _ => " No burn-in: scores from the start.".to_string(),
    };
    let header = format!(
        "{start}.{burn_note}\n\nWorld `{}`, {} h, worlds {:?} (skipped: {}). {} cell x world runs; {} of them on a world \
         the cell's ecology would refuse; {} stopped early. Medians over worlds; `–` nothing \
         to measure, `·` not run.",
        spec.world_name,
        spec.hours,
        list.seeds,
        if list.skipped.is_empty() {
            "none".to_string()
        } else {
            list.skipped.iter().map(|(s, _)| s.to_string()).collect::<Vec<_>>().join(", ")
        },
        records.len(),
        unaccepted,
        stopped
    );
    let md = out.join("summary.md");
    std::fs::write(&md, summary_md(&spec.axes, &rows, &header)).map_err(|e| format!("cannot write {}: {e}", md.display()))?;
    println!("grid: wrote {}, {} and {}", out.join("cells.jsonl").display(), tsv.display(), md.display());
    Ok(())
}
