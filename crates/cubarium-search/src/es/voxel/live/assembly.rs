//! **The E0 and E1 start recipes** (`design/community-assembly-2026-10-03.md` R2,
//! revision 3 N2; `design/handoffs/assembly-step1-2026-10-03.md`): fixed fixtures, the same
//! for every candidate, written as start directories ([`super::start`]).
//!
//! - **E0** ([`e0_start`]): today's founding of the seed under the start's own (x1)
//!   ecology, saved at once.
//! - **E1, constructed rich** ([`e1_start`]): a plants-only founding grown for a fixed
//!   number of hours under the start's own parameters (the simulation books every gram),
//!   then a constructed fauna ([`construct_fauna`]): about three times today's walking
//!   founders in a mix of adults and small, never gravid juveniles, one chorister pack
//!   sized to the prey placed by a recorded rule, and the lanternjaw's egg bank as
//!   today's founding. Every injection goes through the layers' booked introduction and
//!   deposit paths; bodies and attempts are capped; a placement that fails is counted,
//!   never retried into another count.
//!
//! [`generate_library`] writes one directory per seed and skips a seed whose manifest is
//! already there (resumable).

use std::path::PathBuf;
use std::time::Instant;

use cubarium_voxel::{Config, World};
use cubarium_voxel_fauna::{Fauna, Founder, Founding, StartingStores};
use cubarium_voxel_flora::{Flora, Site, Species};
use cubarium_voxel_sim::habitat;
use cubarium_voxel_sim::{Sim, SimConfig, found};
use serde::{Deserialize, Serialize};

use super::ecology::EcologyParams;
use super::start::{Identity, MANIFEST, Manifest, RECIPE_FILE, params_hash, seed_dir, write_start, world_hash};
use super::trainer::parallel;
use super::window::TICKS_PER_MIN;

/// **The E1 recipe's numbers**: fixture settings, not ecology (no candidate sees them),
/// recorded whole in every start's `recipe.toml`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct E1Recipe {
    /// Hours of plants-only growth, simulated, under the start's own parameters, before
    /// the fauna is constructed.
    pub plant_hours: f64,
    /// Bodies asked for per today's founder of each body-founded lineage.
    pub consumer_multiplier: f64,
    /// The share of each lineage's bodies that arrive as juveniles, spread through the
    /// placement order.
    pub juvenile_share: f64,
    /// A juvenile's structure, as the fraction of the way from `body_min` to
    /// `birth_body` (below `birth_body`, so it cannot breed or gestate).
    pub juvenile_body: f64,
    /// The pack rule: one chorister adult per this many prey bodies placed.
    pub prey_per_chorister: f64,
    /// The pack's adults are clamped to `pack_min ..= pack_max`.
    pub pack_min: usize,
    pub pack_max: usize,
    /// At most this many walking bodies, over every lineage (the pack and the eggs aside).
    pub max_bodies: usize,
    /// At most this many introduction attempts, over every lineage.
    pub max_attempts: usize,
}

impl Default for E1Recipe {
    fn default() -> E1Recipe {
        E1Recipe {
            plant_hours: 4.0,
            consumer_multiplier: 3.0,
            juvenile_share: 0.25,
            juvenile_body: 0.5,
            prey_per_chorister: 10.0,
            pack_min: 2,
            pack_max: 6,
            max_bodies: 400,
            max_attempts: 800,
        }
    }
}

/// One body-founded lineage's construction.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub lineage: String,
    /// `round(consumer_multiplier × today's founders)`.
    pub wanted: usize,
    /// Introductions tried.
    pub attempted: usize,
    pub placed: usize,
    /// Tried and refused by the layer (or no starter tile).
    pub failed: usize,
    /// No face the lineage's founding rule would take (habitat, food cover, room).
    pub unplaceable: usize,
    /// Over the body or attempt cap.
    pub capped: usize,
    /// Placed bodies under `birth_body`.
    pub juveniles: usize,
}

/// The chorister pack's construction.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PackPlacement {
    pub lineage: String,
    /// Bodies of its prey lineages placed before it.
    pub prey_bodies: usize,
    /// The rule, with its numbers.
    pub rule: String,
    pub size: usize,
    /// Adults placed.
    pub placed: usize,
    pub failed: Option<String>,
}

/// Matter brought in from outside, all three currencies: the layers' booked
/// introductions (bodies, reserves, stomachs, eggs) and deposits (starter litter).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Injected {
    pub organic: f64,
    pub mineral: f64,
    pub energy: f64,
}

/// What [`construct_fauna`] did.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FaunaReport {
    pub bodies: Vec<Placement>,
    pub pack: PackPlacement,
    /// Eggs buried in the bank, and why none were if none were.
    pub eggs: usize,
    pub eggs_failed: Option<String>,
    pub injected: Injected,
    /// Introduction attempts, all lineages.
    pub attempts: usize,
}

/// Everything booked in from outside so far: the animals' introductions and the plants'
/// deposits and seedings.
fn imported(flora: &Flora, fauna: &Fauna) -> [f64; 3] {
    let (fl, fa) = (flora.view().ledger, fauna.view().ledger);
    [
        fa.introduced_organic_in + fl.deposited_organic_in + fl.seeded_organic_in,
        fa.introduced_mineral_in + fl.deposited_mineral_in + fl.seeded_mineral_in,
        fa.introduced_energy_in + fl.deposited_energy_in + fl.seeded_energy_in,
    ]
}

/// Whether body `k` of a lineage is one of its juveniles: `share` of them, spread evenly.
fn is_juvenile(k: usize, share: f64) -> bool {
    let share = share.clamp(0.0, 1.0);
    ((k + 1) as f64 * share).floor() > (k as f64 * share).floor()
}

/// **The E1 fauna**, on a grown plants-only world: the body-founded lineages at
/// `consumer_multiplier ×` today's counts (`today`, the start's own founding table)
/// through the seeder's own placement rules ([`habitat::place_bodies`]), one pack of each
/// pack lineage sized to its prey, and each egg-bank lineage's clutches as today. `sites`
/// are the support faces and `wettest` the deepest water each has stood under.
pub fn construct_fauna(
    world: &World,
    flora: &mut Flora,
    fauna: &mut Fauna,
    recipe: &E1Recipe,
    today: [usize; Founder::COUNT],
    sites: &[Site],
    wettest: &[f64],
) -> FaunaReport {
    let before = imported(flora, fauna);
    let mut report = FaunaReport::default();
    let mut taken: Vec<Site> = flora.view().stands.iter().map(|s| s.site).collect();
    let mut bodies_left = recipe.max_bodies;
    let mut attempts_left = recipe.max_attempts;
    let arrival = habitat::founder_arrival(&fauna.config().founding);
    for founder in Founder::ALL {
        let phys = *fauna.config().founder(founder);
        if phys.founding != Founding::Bodies || today[founder.index()] == 0 {
            continue;
        }
        let wanted = (recipe.consumer_multiplier * today[founder.index()] as f64).round() as usize;
        let tried = wanted.min(bodies_left);
        let core = phys.core;
        let juvenile = StartingStores {
            body: (core.body_min + recipe.juvenile_body.clamp(0.0, 0.999) * (core.birth_body - core.body_min))
                / core.body_max,
            reserve: arrival.reserve,
        };
        let stores: Vec<StartingStores> = (0..tried)
            .map(|k| if is_juvenile(k, recipe.juvenile_share) { juvenile } else { arrival })
            .collect();
        let p = habitat::place_bodies(world, flora, fauna, founder, &stores, sites, wettest, &mut taken, attempts_left);
        let juveniles = p
            .ids
            .iter()
            .filter(|id| fauna.view().animal(**id).is_some_and(|a| a.body < core.birth_body))
            .count();
        let capped_attempts = p.chosen - p.attempted.min(p.chosen);
        report.bodies.push(Placement {
            lineage: founder.name().into(),
            wanted,
            attempted: p.attempted,
            placed: p.placed,
            failed: p.attempted - p.placed,
            unplaceable: tried - p.chosen,
            capped: (wanted - tried) + capped_attempts,
            juveniles,
        });
        bodies_left -= p.placed;
        attempts_left -= p.attempted;
        report.attempts += p.attempted;
    }
    // One pack of each pack lineage, sized to its prey: `clamp(floor(prey / k), min, max)`.
    for founder in Founder::ALL {
        let phys = *fauna.config().founder(founder);
        if phys.founding != Founding::Pack {
            continue;
        }
        let Some(h) = phys.hunter else { continue };
        let prey = fauna.view().animals.iter().filter(|a| a.founder.is_some_and(|f| h.hunts(f))).count();
        let by_rule = (prey as f64 / recipe.prey_per_chorister.max(1e-9)).floor() as usize;
        let size = by_rule.clamp(recipe.pack_min, recipe.pack_max.max(recipe.pack_min));
        let mut pack = PackPlacement {
            lineage: founder.name().into(),
            prey_bodies: prey,
            rule: format!(
                "adults = clamp(floor({prey} prey bodies / {}), {}, {}) = {size}",
                recipe.prey_per_chorister, recipe.pack_min, recipe.pack_max
            ),
            size,
            ..PackPlacement::default()
        };
        match habitat::introduce_packs_sized(world, flora, fauna, founder, 1, size) {
            Ok(ids) => {
                pack.placed = fauna
                    .view()
                    .animals
                    .iter()
                    .filter(|a| a.pack.is_some_and(|m| ids.contains(&m.pack)))
                    .count();
                if pack.placed < size {
                    pack.failed = Some(format!("{} of {size} adults placed on the den", pack.placed));
                }
            }
            Err(why) => pack.failed = Some(why),
        }
        report.pack = pack;
    }
    // The egg bank, as today's founding.
    for founder in Founder::ALL {
        let phys = *fauna.config().founder(founder);
        if phys.founding != Founding::Clutches || today[founder.index()] == 0 {
            continue;
        }
        match habitat::bury_founding_clutches(world, flora, fauna, founder, today[founder.index()]) {
            Ok(eggs) => report.eggs += eggs,
            Err(why) => report.eggs_failed = Some(why),
        }
    }
    let after = imported(flora, fauna);
    report.injected = Injected {
        organic: after[0] - before[0],
        mineral: after[1] - before[1],
        energy: after[2] - before[2],
    };
    report
}

/// A start built in memory.
pub struct Built {
    pub manifest: Manifest,
    pub world: World,
    pub flora: Flora,
    pub fauna: Fauna,
}

/// **E0**: today's founding of `seed` (asked, kept whatever its verdict) under `eco`.
pub fn e0_start(world: &Config, eco: &EcologyParams, seed: u64, policy: Vec<String>) -> Result<Built, String> {
    eco.params()?;
    let f = found::found(world, &eco.layers(world.voxel_m), eco.fauna().founder_counts(), found::Seeds::Asked(seed), false);
    let identity = Identity::this_build(world, policy);
    let manifest = Manifest::of("e0", seed, f.accepted, identity, &f.world, &f.flora, &f.fauna);
    Ok(Built { manifest, world: f.world, flora: f.flora, fauna: f.fauna })
}

/// The plants at one moment of an E1 grow.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GrowthRow {
    pub minute: u64,
    pub stands: usize,
    pub vaulttree_stands: usize,
    /// Vaulttrees at or past `donor_min` (sending propagules).
    pub vaulttree_donors: usize,
    pub vines: usize,
    pub vine_faces: usize,
    pub plant_organic: f64,
}

impl GrowthRow {
    fn of(minute: u64, flora: &Flora) -> GrowthRow {
        let v = flora.view();
        let donor = v.config.species(Species::Vaulttree).donor_min;
        let trees: Vec<_> = v.stands.iter().filter(|s| s.species == Species::Vaulttree).collect();
        GrowthRow {
            minute,
            stands: v.stands.len(),
            vaulttree_stands: trees.len(),
            vaulttree_donors: trees.iter().filter(|s| s.wood >= donor).count(),
            vines: v.cover.vines().len(),
            vine_faces: v.cover.faces().len(),
            plant_organic: v.organic(),
        }
    }
}

/// **A start's provenance** (`recipe.toml`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RecipeRecord {
    pub recipe: String,
    pub seed: u64,
    /// The build that made it (`BUILD_ID`).
    pub commit: String,
    /// The world config named, and its hash; the start's own parameters' hash.
    pub world: String,
    pub world_config: String,
    pub params: String,
    pub accepted: bool,
    /// E1: hours of plants-only growth.
    pub plant_hours: f64,
    pub e1: Option<E1Recipe>,
    pub fauna: Option<FaunaReport>,
    /// E1: the plants every half hour of the grow, and at its end.
    pub growth: Vec<GrowthRow>,
    pub wall_s: f64,
}

/// **E1**: found `seed` with no animals, grow its plants `recipe.plant_hours`, construct
/// the fauna. `log` hears each growth row.
pub fn e1_start(
    world_cfg: &Config,
    eco: &EcologyParams,
    seed: u64,
    recipe: &E1Recipe,
    policy: Vec<String>,
    sim_threads: usize,
    log: &(dyn Fn(&str) + Sync),
) -> Result<(Sim, Manifest, RecipeRecord), String> {
    eco.params()?;
    let started = Instant::now();
    let f = found::found(world_cfg, &eco.layers(world_cfg.voxel_m), [0; Founder::COUNT], found::Seeds::Asked(seed), false);
    let accepted = f.accepted;
    let mut sim = Sim::ambient(f.world, f.flora, f.fauna, SimConfig { threads: sim_threads.max(1), overlap: false });
    let sites = habitat::support_sites(sim.world());
    let depth = |w: &World, wettest: &mut [f64]| {
        let view = w.view();
        for (d, s) in wettest.iter_mut().zip(&sites) {
            *d = d.max(view.water_depth_m(i64::from(s.x), s.y, s.z));
        }
    };
    let mut wettest = vec![0.0; sites.len()];
    depth(sim.world(), &mut wettest);
    let minutes = (recipe.plant_hours * 60.0).round() as u64;
    let mut growth = vec![GrowthRow::of(0, sim.flora())];
    for minute in 1..=minutes {
        for _ in 0..TICKS_PER_MIN {
            sim.step();
        }
        depth(sim.world(), &mut wettest);
        if minute % 30 == 0 || minute == minutes {
            let row = GrowthRow::of(minute, sim.flora());
            log(&format!(
                "e1 seed {seed} minute {minute}: stands {} vaulttrees {} donors {} vines {} vine faces {} ({:.0} s)",
                row.stands,
                row.vaulttree_stands,
                row.vaulttree_donors,
                row.vines,
                row.vine_faces,
                started.elapsed().as_secs_f64()
            ));
            growth.push(row);
        }
    }
    let today = eco.fauna().founder_counts();
    let report = sim.with_layers_mut(|w, fl, fa| construct_fauna(w, fl, fa, recipe, today, &sites, &wettest));
    let (w, fl, fa) = sim.layers();
    let identity = Identity::this_build(world_cfg, policy);
    let manifest = Manifest::of("e1", seed, accepted, identity, w, fl, fa);
    let record = RecipeRecord {
        recipe: "e1".into(),
        seed,
        commit: crate::evaluate::BUILD_ID.into(),
        world: String::new(),
        world_config: world_hash(world_cfg),
        params: params_hash(&super::start::layers_of(fl, fa)),
        accepted,
        plant_hours: recipe.plant_hours,
        e1: Some(recipe.clone()),
        fauna: Some(report),
        growth,
        wall_s: started.elapsed().as_secs_f64(),
    };
    Ok((sim, manifest, record))
}

/// **A start library's generation**: what to make and where.
pub struct StartsSpec {
    /// `e0` or `e1`.
    pub recipe: String,
    pub seeds: Vec<u64>,
    pub world: Config,
    /// What the world was read from (recorded).
    pub world_name: String,
    /// The start's own parameters (the run's x1).
    pub eco: EcologyParams,
    pub e1: E1Recipe,
    pub policy: Vec<String>,
    /// Seeds generated at once.
    pub threads: usize,
    /// Simulation threads per seed (E1's grow).
    pub sim_threads: usize,
    pub out: PathBuf,
}

/// Generate every seed not yet in `spec.out` (one with a manifest is whole and skipped).
pub fn generate_library(spec: &StartsSpec) -> Result<(), String> {
    std::fs::create_dir_all(&spec.out).map_err(|e| format!("cannot create {}: {e}", spec.out.display()))?;
    if !matches!(spec.recipe.as_str(), "e0" | "e1") {
        return Err(format!("no start recipe `{}` (e0, e1)", spec.recipe));
    }
    let todo: Vec<u64> = spec
        .seeds
        .iter()
        .copied()
        .filter(|s| !seed_dir(&spec.out, *s).join(MANIFEST).is_file())
        .collect();
    println!(
        "starts: recipe {} seeds {:?}: {} to make, {} already there",
        spec.recipe,
        spec.seeds,
        todo.len(),
        spec.seeds.len() - todo.len()
    );
    let log = |s: &str| println!("{s}");
    let results: Vec<Result<(), String>> = parallel(todo, spec.threads.max(1), |seed| {
        let dir = seed_dir(&spec.out, seed);
        let started = Instant::now();
        let (manifest, mut record, layers) = if spec.recipe == "e0" {
            let b = e0_start(&spec.world, &spec.eco, seed, spec.policy.clone())?;
            let record = RecipeRecord {
                recipe: "e0".into(),
                seed,
                commit: crate::evaluate::BUILD_ID.into(),
                world_config: b.manifest.identity.world_config.clone(),
                params: b.manifest.params.clone(),
                accepted: b.manifest.accepted,
                ..RecipeRecord::default()
            };
            (b.manifest.clone(), record, Layers::Built(b))
        } else {
            let (sim, m, r) = e1_start(&spec.world, &spec.eco, seed, &spec.e1, spec.policy.clone(), spec.sim_threads, &log)?;
            (m, r, Layers::Sim(Box::new(sim)))
        };
        record.world = spec.world_name.clone();
        record.wall_s = started.elapsed().as_secs_f64();
        std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        let text = toml::to_string(&record).map_err(|e| format!("cannot write the recipe record: {e}"))?;
        std::fs::write(dir.join(RECIPE_FILE), text).map_err(|e| format!("cannot write {}: {e}", dir.display()))?;
        let (w, fl, fa) = layers.refs();
        write_start(&dir, &manifest, w, fl, fa)?;
        let placed: Vec<String> = record
            .fauna
            .iter()
            .flat_map(|r| r.bodies.iter().map(|p| format!("{} {}/{}", p.lineage, p.placed, p.wanted)))
            .collect();
        println!(
            "starts: seed {seed} written to {} at tick {} ({:.0} s){}",
            dir.display(),
            manifest.tick,
            record.wall_s,
            if placed.is_empty() { String::new() } else { format!(": {}", placed.join(", ")) }
        );
        Ok(())
    });
    results.into_iter().collect()
}

/// A start's layers, built or inside a grown sim.
enum Layers {
    Built(Built),
    Sim(Box<Sim>),
}

impl Layers {
    fn refs(&self) -> (&World, &Flora, &Fauna) {
        match self {
            Layers::Built(b) => (&b.world, &b.flora, &b.fauna),
            Layers::Sim(s) => s.layers(),
        }
    }
}
