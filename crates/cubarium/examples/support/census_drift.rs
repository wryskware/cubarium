//! Run-local drift over (FROM, TO], in absolute census sim-minutes.
//! Population is integrated every tick; fullness pools body samples at 1 Hz.
//! Energy terms use the ledger's organic-equivalent units, per body-hour.
use cubarium_voxel_fauna::{Departure, FaunaLedger, Founder, TICK_HZ};
use cubarium_voxel_flora::{Species, Trophic};
use cubarium_voxel_sim::Sim;
use serde_json::{Value, json};
use std::path::PathBuf;

const TPM: u64 = 60 * TICK_HZ as u64;

#[derive(Clone, Debug)]
pub struct Config {
    from: u64,
    to: u64,
    out: Option<PathBuf>,
}
impl Config {
    pub fn parse(args: &[String]) -> Result<Option<Self>, String> {
        let window = args.iter().find_map(|a| a.strip_prefix("drift="));
        let out = args.iter().find_map(|a| a.strip_prefix("drift_out="));
        let Some(window) = window else {
            return if out.is_some() { Err("drift_out= requires drift=FROM:TO".into()) } else { Ok(None) };
        };
        let (from, to) = window.split_once(':').ok_or("drift=FROM:TO (sim-minutes)")?;
        let ticks = |s: &str| -> Result<u64, String> {
            let n: f64 = s.parse().map_err(|_| "drift boundaries need numbers")?;
            if !n.is_finite() || n < 0.0 || n * TPM as f64 >= u64::MAX as f64 {
                return Err("drift boundaries must be finite, nonnegative sim-minutes".into());
            }
            Ok((n * TPM as f64).round() as u64)
        };
        let (from, to) = (ticks(from)?, ticks(to)?);
        if to <= from { return Err("drift TO must exceed FROM by at least one tick".into()); }
        if out == Some("") { return Err("drift_out= needs a file path".into()); }
        Ok(Some(Self { from, to, out: out.map(PathBuf::from) }))
    }
    pub fn validate(&self, start: u64, end: u64) -> Result<(), String> {
        if self.from < start || self.to > end {
            return Err(format!("drift window {}:{} is outside this run's {}:{} sim-minutes", self.from as f64 / TPM as f64, self.to as f64 / TPM as f64, start as f64 / TPM as f64, end as f64 / TPM as f64));
        }
        Ok(())
    }
}

struct Boundary {
    n: [u64; Founder::COUNT],
    born: [u64; Founder::COUNT],
    ledger: Box<FaunaLedger>,
    produced: f64,
    eaten: f64,
}
impl Boundary {
    fn read(sim: &Sim) -> Self {
        let mut n = [0; Founder::COUNT];
        for a in sim.fauna().view().animals {
            if let Some(f) = a.founder { n[f.index()] += 1; }
        }
        let flora = sim.flora().view();
        let leaves = |book: &[f64; Species::COUNT]| Species::ALL.into_iter()
            .filter(|&s| flora.config.species(s).trophic == Trophic::Photo)
            .map(|s| book[s.index()]).sum();
        Self { n, born: sim.fauna().born_by_founder(), ledger: Box::new(sim.fauna().view().ledger.clone()),
            produced: leaves(&flora.ledger.foliage_built), eaten: leaves(&flora.ledger.foliage_taken) }
    }
}

pub struct Recorder {
    config: Config,
    opening: Option<Boundary>,
    previous_n: [u64; Founder::COUNT],
    body_ticks: [f64; Founder::COUNT],
    fullness: [Vec<f64>; Founder::COUNT],
    result: Option<Value>,
}
impl Recorder {
    pub fn new(config: Config) -> Self {
        Self { config, opening: None, previous_n: [0; Founder::COUNT], body_ticks: [0.0; Founder::COUNT],
            fullness: std::array::from_fn(|_| Vec::new()), result: None }
    }
    pub fn validate(&self, start: u64, end: u64) -> Result<(), String> { self.config.validate(start, end) }
    /// Called at the run start (before edits), then after each tick.
    pub fn poll(&mut self, sim: &mut Sim, tick: u64) {
        if self.result.is_some() || tick < self.config.from { return; }
        if self.opening.is_none() {
            assert_eq!(tick, self.config.from);
            let boundary = Boundary::read(sim);
            self.previous_n = boundary.n;
            self.opening = Some(boundary);
            sim.fauna_mut().set_census_readout(true);
            return;
        }
        for (sum, n) in self.body_ticks.iter_mut().zip(self.previous_n) { *sum += n as f64; }
        self.previous_n = Boundary::read_counts(sim);
        if tick % u64::from(TICK_HZ) == 0 || tick == self.config.to { self.sample(sim); }
        if tick == self.config.to { self.close(sim, tick, false); }
    }
    /// Edits at FROM are fluxes, while subsequent residence uses the edited population.
    pub fn after_edits(&mut self, sim: &Sim) {
        if self.opening.is_some() { self.previous_n = Boundary::read_counts(sim); }
    }
    fn sample(&mut self, sim: &Sim) {
        for a in sim.fauna().view().animals {
            if let Some(f) = a.founder {
                let row = cubarium_observatory::query::animal_row(sim.fauna(), a);
                self.fullness[f.index()].push(row.fullness);
            }
        }
    }
    fn close(&mut self, sim: &mut Sim, tick: u64, clipped: bool) {
        let opening = self.opening.as_ref().expect("drift began");
        let end = Boundary::read(sim);
        let ticks = tick - self.config.from;
        let hours = ticks as f64 / (3600.0 * f64::from(TICK_HZ));
        let readout = sim.fauna().census_readout().expect("active drift readout");
        let lineages: Vec<Value> = Founder::ALL.into_iter().map(|f| {
            let i = f.index();
            let births = end.born[i] - opening.born[i];
            let hatchings = end.ledger.eggs_hatched_by_founder[i] - opening.ledger.eggs_hatched_by_founder[i];
            let departures: Vec<_> = Departure::ALL.into_iter().map(|c| (c, end.ledger.departed_founder(f, c) - opening.ledger.departed_founder(f, c))).collect();
            let deaths: u64 = departures.iter().filter(|(c, _)| c.is_death()).map(|(_, n)| n).sum();
            let removed = departures[Departure::Removed.index()].1;
            let introduced = readout.introduced[i];
            let residual = end.n[i] as i128 - opening.n[i] as i128 - births as i128 + deaths as i128 - introduced as i128 + removed as i128;
            assert_eq!(residual, 0, "drift bookkeeping for {}", f.name());
            let assimilated = end.ledger.assimilated_by_founder[i] - opening.ledger.assimilated_by_founder[i];
            let upkeep = end.ledger.upkeep_by_founder[i] - opening.ledger.upkeep_by_founder[i];
            let motor = end.ledger.motor_by_founder[i] - opening.ledger.motor_by_founder[i];
            let mean_n = self.body_ticks[i] / ticks as f64;
            let body_hours = mean_n * hours;
            let causes: serde_json::Map<String, Value> = departures.into_iter().map(|(c, n)| (c.name().into(), json!(n))).collect();
            let mut fullness = self.fullness[i].clone();
            fullness.sort_unstable_by(f64::total_cmp);
            json!({"lineage":f.name(), "n_from":opening.n[i], "n_to":end.n[i], "mean_n":mean_n,
                "live_births":births - hatchings, "hatchings":hatchings,
                "eggs_laid":end.ledger.eggs_laid_by_founder[i] - opening.ledger.eggs_laid_by_founder[i],
                "independent":readout.independent[i], "adult":end.ledger.matured_by_founder[i] - opening.ledger.matured_by_founder[i],
                "deaths":causes, "introduced":introduced, "removed":removed, "bookkeeping_residual":residual as i64,
                "assimilated":assimilated, "upkeep":upkeep, "motor":motor, "organic_given":readout.organic_given[i],
                "rho":ratio(births as f64 - deaths as f64, body_hours), "rho_E":ratio(assimilated - upkeep - motor, body_hours),
                "fullness_p10":quantile(&fullness, 0.1), "fullness_p50":quantile(&fullness, 0.5), "fullness_p90":quantile(&fullness, 0.9)})
        }).collect();
        let mut hunts = Vec::new();
        for predator in Founder::ALL {
            for prey in Founder::ALL {
                let events: Vec<_> = readout.predation.iter().filter(|e| e.predator == predator && e.prey == prey)
                    .map(|e| json!({"kind":if e.kill {"kill"} else {"hunt"}, "prey_body":e.prey_body})).collect();
                let count = |kind: &str| events.iter().filter(|e| e["kind"] == kind).count();
                let (h, k) = (count("hunt"), count("kill"));
                if events.is_empty() { continue; }
                hunts.push(json!({"predator":predator.name(), "prey":prey.name(), "hunts":h, "kills":k,
                    "hunts_per_hour":h as f64 / hours, "kills_per_hour":k as f64 / hours, "events":events}));
            }
        }
        let produced = end.produced - opening.produced;
        let eaten = end.eaten - opening.eaten;
        self.result = Some(json!({"from_minute":self.config.from as f64 / TPM as f64, "to_minute":tick as f64 / TPM as f64,
            "requested_to_minute":self.config.to as f64 / TPM as f64, "hours":hours, "clipped":clipped,
            "recruits":"live_births + hatchings", "energy_unit":"organic-equivalent per body-hour", "fullness_sampling":"pooled bodies at 1 Hz; endpoint included",
            "hunt_definition":"strikes begun", "lineages":lineages,
            "world":{"leaf_produced":produced, "leaf_eaten":eaten, "leaf_offtake":ratio(eaten, produced), "hunts":hunts}}));
        sim.fauna_mut().set_census_readout(false);
    }
    pub fn finish(&mut self, sim: &mut Sim, tick: u64) -> Result<(), String> {
        if self.result.is_none() {
            if self.opening.is_none() || tick <= self.config.from { return Err("run stopped before drift window; no readout".into()); }
            if tick % u64::from(TICK_HZ) != 0 { self.sample(sim); }
            self.close(sim, tick, true);
        }
        let result = self.result.as_ref().expect("closed drift");
        for lineage in result["lineages"].as_array().expect("lineages") {
            eprintln!("drift: from={} to={} clipped={} {}", result["from_minute"], result["to_minute"], result["clipped"], serde_json::to_string(lineage).expect("serialize drift"));
        }
        eprintln!("drift_world: from={} to={} hours={} clipped={} {}", result["from_minute"], result["to_minute"], result["hours"], result["clipped"], serde_json::to_string(&result["world"]).expect("serialize world drift"));
        if let Some(path) = &self.config.out {
            let bytes = serde_json::to_vec_pretty(result).map_err(|e| e.to_string())?;
            std::fs::write(path, bytes).map_err(|e| format!("drift_out {}: {e}", path.display()))?;
        }
        Ok(())
    }
}
impl Boundary {
    fn read_counts(sim: &Sim) -> [u64; Founder::COUNT] {
        let mut n = [0; Founder::COUNT];
        for a in sim.fauna().view().animals { if let Some(f) = a.founder { n[f.index()] += 1; } }
        n
    }
}
fn ratio(n: f64, d: f64) -> Option<f64> { (d > 0.0).then(|| n / d) }
fn quantile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() { return None; }
    let at = p * (sorted.len() - 1) as f64;
    let lo = at.floor() as usize;
    let hi = at.ceil() as usize;
    Some(sorted[lo] + (sorted[hi] - sorted[lo]) * (at - lo as f64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_voxel::{Command, Config as WorldConfig, Material, World};
    use cubarium_voxel_fauna::{BodySpec, Fauna, FaunaConfig, Placed};
    use cubarium_voxel_flora::{Flora, FloraConfig};
    use cubarium_voxel_sim::{SimConfig, edit::At};

    fn empty() -> Sim {
        let mut world = World::empty(WorldConfig { width: 32, height: 12, depth: 16, voxel_m: 0.25, ..Default::default() });
        for z in 0..16 { for x in 0..32 {
            world.apply(Command::SetMaterial { x, y: 1, z, material: Material::Soil });
        } }
        Sim::ambient(world, Flora::new(FloraConfig::for_voxel_size(0.25)), Fauna::new(FaunaConfig::default()), SimConfig::with_threads(1))
    }
    fn place(sim: &mut Sim, x: i64) -> u64 {
        match sim.edit(|e| e.place_animal(Founder::Browser, At::Column { x, z: 8 }, BodySpec::default())).expect("place") {
            Placed::Body { id, .. } => id,
            other => panic!("{other:?}"),
        }
    }
    #[test]
    fn window_validation_and_undefined_quantiles() {
        let parse = |s: &str| Config::parse(&[format!("drift={s}")]);
        for s in ["2:1", "1:1", "NaN:2", "0:inf", "-1:2", "bad"] { assert!(parse(s).is_err(), "{s}"); }
        let c = parse("10:20").unwrap().unwrap();
        assert!(c.validate(0, 19 * TPM).is_err());
        assert!(c.validate(11 * TPM, 30 * TPM).is_err());
        assert!(c.validate(0, 20 * TPM).is_ok());
        assert_eq!(quantile(&[], 0.5), None);
        assert_eq!(quantile(&[0.0, 1.0], 0.1), Some(0.1));
        assert_eq!(ratio(1.0, 0.0), None);
    }
    #[test]
    fn edits_close_and_energy_terms_are_the_ledgers_window_deltas() {
        let mut sim = empty();
        let mut r = Recorder::new(Config { from: 0, to: 20, out: None });
        r.poll(&mut sim, 0);
        let id = place(&mut sim, 8);
        place(&mut sim, 20);
        sim.edit(|e| e.remove_animal(id)).unwrap();
        r.after_edits(&sim);
        let opening = sim.fauna().view().ledger.clone();
        for tick in 1..=20 { sim.step(); r.poll(&mut sim, tick); }
        let result = r.result.as_ref().unwrap();
        let row = result["lineages"].as_array().unwrap().iter().find(|row| row["lineage"] == Founder::Browser.name()).unwrap();
        assert_eq!(row["n_from"], 0);
        assert_eq!(row["n_to"], 1);
        assert_eq!(row["introduced"], 2);
        assert_eq!(row["removed"], 1);
        assert_eq!(row["bookkeeping_residual"], 0);
        assert_eq!(row["mean_n"], 1.0);
        let ledger = sim.fauna().view().ledger;
        let i = Founder::Browser.index();
        for (name, want) in [("assimilated", ledger.assimilated_by_founder[i] - opening.assimilated_by_founder[i]),
            ("upkeep", ledger.upkeep_by_founder[i] - opening.upkeep_by_founder[i]),
            ("motor", ledger.motor_by_founder[i] - opening.motor_by_founder[i])] {
            assert_eq!(row[name].as_f64().unwrap(), want);
        }
        let expected = (row["assimilated"].as_f64().unwrap() - row["upkeep"].as_f64().unwrap() - row["motor"].as_f64().unwrap()) / result["hours"].as_f64().unwrap();
        assert_eq!(row["rho_E"].as_f64().unwrap(), expected);
        assert!(sim.fauna().census_readout().is_none());
    }
    #[test]
    fn early_stop_is_clipped_and_empty_lineages_have_null_rates() {
        let mut sim = empty();
        let mut r = Recorder::new(Config { from: 0, to: 40, out: None });
        r.poll(&mut sim, 0);
        for tick in 1..=10 { sim.step(); r.poll(&mut sim, tick); }
        r.close(&mut sim, 10, true);
        let result = r.result.unwrap();
        assert_eq!(result["clipped"], true);
        assert_eq!(result["to_minute"], 10.0 / TPM as f64);
        for row in result["lineages"].as_array().unwrap() {
            assert!(row["rho"].is_null());
            assert!(row["rho_E"].is_null());
        }
    }
}
