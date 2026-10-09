//! **Making a world to run**: found a fresh one, or resume a saved one, then seed it and
//! install its animals' drivers. One copy, used by `cubarium voxel` (from its command
//! line) and by a session's candidates and loads.

use std::path::Path;

use anyhow::Result;
use cubarium_voxel::{Command as VoxelCommand, World};
use cubarium_voxel_fauna::{Fauna, Founder, Senses};
use cubarium_voxel_flora::Flora;
use cubarium_voxel_sim::found::{LAKE_SEED_TRIES, LayerConfig, Seeds, found, generate_with_a_lake};
use cubarium_voxel_sim::params::Params;
use cubarium_voxel_sim::{Sim, SimConfig, habitat, scene};
use cubarium_search::es::voxel::EpisodeDriver;

use crate::founders::{FounderRecipe, install_founder_controllers};
use crate::store;

/// Which world to build when none is resumed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// The hand-authored fixture.
    Authored,
    /// A generated, founded world.
    Generated,
}

/// What a run starts from.
#[derive(Clone, Debug)]
pub struct StartSpec<'a> {
    /// Resume this world file or state directory (`--load`); a directory with nothing
    /// loadable in it founds a fresh world ([`store::resume`]).
    pub load: Option<&'a Path>,
    pub scene: Scene,
    /// The terrain seed asked for; `None` draws one.
    pub seed: Option<u64>,
    /// A bare world: no stands, no logs, no animals.
    pub empty: bool,
    /// How many of each lineage a fresh world founds.
    pub founder_counts: [usize; Founder::COUNT],
}

/// A fresh generated world's founding: the layers the founding loop seeded it with.
pub type Seeding = (Flora, Fauna, habitat::Seeded);

/// The layers a run starts with, when they do not have to be seeded by the caller.
pub enum StartLayers {
    /// A fresh generated world, seeded and judged by the founding loop.
    Founded(Seeding),
    /// A resumed world's own plants and animals, saved beside it.
    Restored(Flora, Fauna),
}

/// Generate a random 64-bit seed from `/dev/urandom` or fallback system time entropy.
pub fn random_seed() -> u64 {
    let mut buf = [0u8; 8];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        if f.read_exact(&mut buf).is_ok() {
            let s = u64::from_le_bytes(buf);
            if s != 0 {
                return s;
            }
        }
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x5EED_0001)
}

/// Resume from a file or directory, or create a fresh world on `params`.
///
/// A **fresh generated** world that is going to be seeded goes through the founding loop
/// ([`found`]) and comes back with its layers, already seeded; every other world comes
/// back bare, `None`, for the caller to seed. Returns the world, its label, whether it
/// was resumed, and its layers when they came with it.
pub fn load_or_create_world(
    params: &Params,
    spec: &StartSpec<'_>,
) -> Result<(World, String, bool, Option<StartLayers>)> {
    if let Some(path) = spec.load
        && let Some((world, label, resumed, layers)) = store::resume(path)?
    {
        let layers = layers.map(|(flora, fauna)| StartLayers::Restored(flora, fauna));
        return Ok((world, label, resumed, layers));
    }
    let world_cfg = params.world.clone();
    let (world, label) = match spec.scene {
        Scene::Authored => (scene::authored(world_cfg), "authored".to_string()),
        Scene::Generated => {
            if spec.empty {
                let (world, seed, rejected) =
                    generate_with_a_lake(&world_cfg, spec.seed, LAKE_SEED_TRIES, random_seed);
                let label = if rejected > 0 {
                    format!("generated (seed {seed}, {rejected} rejected)")
                } else {
                    format!("generated (seed {seed})")
                };
                (world, label)
            } else {
                let founded = found(
                    &world_cfg,
                    &LayerConfig::new(params, world_cfg.voxel_m),
                    spec.founder_counts,
                    spec.seed.map_or(Seeds::Drawn(random_seed), Seeds::Asked),
                    false,
                );
                if let Some(why) = &founded.terrain_rejection {
                    anyhow::bail!("terrain seed {} rejected: {why}", founded.seed);
                }
                let rejected = founded.lake_rejected + founded.habitat_rejected;
                let label = if rejected > 0 {
                    format!("generated (seed {}, {rejected} rejected)", founded.seed)
                } else {
                    format!("generated (seed {})", founded.seed)
                };
                return Ok((
                    founded.world,
                    label,
                    false,
                    Some(StartLayers::Founded((founded.flora, founded.fauna, founded.seeded))),
                ));
            }
        }
    };
    Ok((world, label, false, None))
}

/// How the layers of a world in hand are seeded and driven.
pub struct Populate<'a> {
    pub recipes: &'a [FounderRecipe],
    /// `--introduce`: adult lanternjaws or chorister packs where the prey live.
    pub introductions: &'a [(Founder, usize)],
    pub empty: bool,
    pub founder_counts: [usize; Founder::COUNT],
    /// Only said: the founding was asked to leave out the lanternjaw egg bank.
    pub no_egg_bank: bool,
}

/// The layers of a world [`load_or_create_world`] returned, seeded (unless resumed or
/// empty) and with every lineage's driver installed. Says on stderr what it did.
pub fn populate(
    world: World,
    resumed: bool,
    founded: Option<StartLayers>,
    params: &Params,
    how: &Populate<'_>,
) -> Result<(World, Flora, Fauna)> {
    let mut world = world;
    let drivers: Vec<(Founder, EpisodeDriver)> = how
        .recipes
        .iter()
        .filter_map(|r| r.driver.clone().map(|d| (r.founder, d)))
        .collect();
    // A fresh generated world comes out of the founding loop **already seeded** and
    // judged; an authored or resumed one is pre-rolled and seeded here — a resumed one
    // without the opening shower, which it has had, and never redrawn.
    let restored = matches!(founded, Some(StartLayers::Restored(..)));
    let (flora, mut fauna, seeded) = match founded {
        Some(StartLayers::Founded((flora, fauna, seeded))) => (flora, fauna, Some(seeded)),
        // Resumed with its own plants and animals: nothing to seed.
        Some(StartLayers::Restored(flora, fauna)) => (flora, fauna, None),
        None => {
            let (mut flora, mut fauna) = LayerConfig::new(params, world.config().voxel_m).layers();
            let seeded = (!how.empty).then(|| {
                let pre = habitat::pre_roll(&mut world, !resumed);
                habitat::seed_pre_rolled(&mut world, &mut flora, &mut fauna, &pre, how.founder_counts)
            });
            (flora, fauna, seeded)
        }
    };
    if let Some(seeded) = seeded {
        report_seeding(&world, &seeded, how.no_egg_bank);
    } else if restored {
        // A restored layer's controllers are not saved: every standing body gets its
        // lineage's heuristic back, and the policy-driven lineages are re-installed over
        // it just below.
        habitat::reinstall_heuristics(&mut fauna);
    } else {
        // An empty world grows its founders by hand; the lineages still need their
        // recipes registered so anything born is driven.
        habitat::install_heuristics(&mut fauna);
    }
    install_founder_controllers(&mut fauna, &drivers)?;
    // `--introduce lanternjaw:N` (predation §2.9): adults where the prey live, or the
    // reason there are none. `--introduce chorister:N` (roster R3): N packs, each on a
    // covered den near the prey.
    for &(founder, n) in how.introductions {
        anyhow::ensure!(
            matches!(founder, Founder::Lanternjaw | Founder::Chorister),
            "--introduce takes lanternjaw:N or chorister:N; the {} lineage is founded by the habitat",
            founder.name()
        );
        if founder == Founder::Chorister {
            match habitat::introduce_packs(&world, &flora, &mut fauna, founder, n) {
                Ok(ids) => eprintln!(
                    "cubarium voxel: introduced {} chorister packs where the prey live",
                    ids.len()
                ),
                Err(why) => eprintln!("cubarium voxel: --introduce chorister:{n} refused — {why}"),
            }
            continue;
        }
        match habitat::introduce_lanternjaws(&world, &flora, &mut fauna, n) {
            Ok(ids) => eprintln!(
                "cubarium voxel: introduced {} adult lanternjaws where the prey live",
                ids.len()
            ),
            Err(why) => eprintln!("cubarium voxel: --introduce lanternjaw:{n} refused — {why}"),
        }
    }
    // Say what is actually driving each lineage, every run: the default is a trained
    // centre, and a trained animal and a reflex animal are told apart from outside only
    // by being told apart here.
    for recipe in how.recipes {
        eprintln!(
            "cubarium voxel: {} founders are driven by {} ({})",
            recipe.founder.name(),
            match &recipe.driver {
                Some(driver) => driver.name(),
                None => "the observation-only heuristic".to_string(),
            },
            recipe.source,
        );
    }
    Ok((world, flora, fauna))
}

/// The coupled tick over three layers: the live schedule's own sensory state, settled
/// against the layers as they stand before the first tick — the ambient world's founders
/// smell the litter this flora actually drops and has eaten, at the field's own cadence.
/// An empty world has no litter, so the field is empty and its update is a walk over
/// nothing.
pub fn ready_sim(world: World, flora: Flora, fauna: Fauna, config: SimConfig) -> Sim {
    let mut senses = Senses::with_cue(fauna.config().cue);
    senses.settle(&world.view(), &flora.view());
    Sim::new(world, flora, fauna, config, Some(senses))
}

/// The display's water fidelity (`water_every`, live by default); a coarse one is said,
/// since it changes how fast water spreads on screen.
pub fn set_water_every(sim: &mut Sim, every: u32) {
    sim.world_mut().set_water_every(every.max(1));
    if sim.world().water_every() > 1 {
        eprintln!(
            "cubarium voxel: coarse water, the lateral phases once every {} ticks",
            sim.world().water_every()
        );
    }
}

/// Under a closed budget the outlet is not an export: what it takes goes into the
/// atmosphere store and comes back as a shower. It is the strongest of the two return
/// flows, so a closed world starts with it open — otherwise the cycle never starts and
/// the run reports a world that is only standing still.
pub fn open_closed_outlet(sim: &mut Sim) {
    if sim.world().config().closed_water_budget && !sim.world().outlet_open() {
        sim.world_mut().apply(VoxelCommand::SetOutlet { open: true });
        eprintln!(
            "cubarium voxel: closed water budget — the outlet is open as the return flow \
             into the atmosphere, not an export"
        );
    }
}

/// The settle report, once, at startup: how long the water took to stop moving and
/// whether it did. The ordinary display shows none of this.
fn report_seeding(world: &World, seeded: &habitat::Seeded, no_egg_bank: bool) {
    let st = seeded.settle;
    let wc = world.config().clone();
    let lake = cubarium_voxel::hydrate::lake(world);
    let above = cubarium_voxel::hydrate::tier_pools(&cubarium_voxel::hydrate::pools(world), lake.level_y);
    eprintln!(
        "cubarium voxel: lake {:.2} m³ over {:.1} m², {:.1} m² visible, \
         {above} pool(s) above it, stream {:.4} m³/s",
        lake.volume_m3,
        lake.surface_cells.len() as f64 * world.config().cell_area(),
        lake.visible_m2,
        world.config().reentry_m3_per_s,
    );
    eprintln!(
        "cubarium voxel: {} — water settled in {} ticks ({}), {:.2} m³ pooled \
         in {} cells, {:.2} m³ pore, drift {:.2e} m³/100 ticks{}",
        if wc.closed_water_budget {
            format!(
                "closed cycle, {:.2} m³ aloft, {}, {:.0} mm/h rain, \
                 {:.0} mm/h evaporation",
                world.atmosphere_m3(),
                format!(
                    "climate {}, RH {:.2}, dry gap {:.0}-{:.0} min",
                    wc.climate.preset.name(),
                    world.rh(),
                    wc.climate.dry_gap_min_s / 60.0,
                    wc.climate.dry_gap_max_s / 60.0,
                ),
                wc.rain_m_per_s * 3600.0 * 1000.0,
                wc.evaporation_m_per_s * 3600.0 * 1000.0,
            )
        } else {
            "open budget, no cycle to report".to_string()
        },
        st.ticks,
        if st.converged { "converged" } else { "still moving at the cap" },
        st.pooled_m3,
        st.free_cells,
        st.pore_m3,
        st.drift_m3_per_100,
        if st.dry_locked {
            " — DRY LOCKED: water aloft and no pool on the ground"
        } else {
            ""
        },
    );
    let bodies: Vec<String> = Founder::ALL
        .into_iter()
        .filter(|f| seeded.founders[f.index()] > 0)
        .map(|f| format!("{} {}", seeded.founders[f.index()], f.name()))
        .collect();
    eprintln!(
        "cubarium voxel: seeded the example habitat — {} stands, {} logs, \
         {} litter tiles, founders: {} (--empty for a bare world)",
        seeded.stands,
        seeded.logs,
        seeded.litter_tiles,
        bodies.join(", "),
    );
    match &seeded.founding_bank_refused {
        Some(why) => eprintln!("cubarium voxel: no lanternjaw egg bank — {why}"),
        None => eprintln!(
            "cubarium voxel: {} lanternjaw eggs buried where the prey live{}",
            seeded.founding_eggs,
            if no_egg_bank { " (--no-egg-bank)" } else { "" },
        ),
    }
    for founder in Founder::ALL {
        let short = seeded.shortfall[founder.index()];
        if short > 0 {
            eprintln!(
                "cubarium voxel: {} shortfall — {short} bodies not placed; \
                 this habitat holds no site the lineage can live on",
                founder.name()
            );
        }
    }
    let pr = seeded.pre_roll;
    eprintln!(
        "cubarium voxel: pre-roll {} ticks — {}; habitat {}",
        pr.ticks,
        if pr.opening_shower {
            format!(
                "opened with a {}-tick shower, drained in {} ticks ({}), lake \
                 at rest after {} more",
                pr.shower_ticks,
                pr.drain.ticks,
                if pr.drain.converged { "converged" } else { "at the cap" },
                pr.lake_rest_ticks,
            )
        } else {
            "no opening shower".to_string()
        },
        if seeded.acceptance.accepted {
            "accepted".to_string()
        } else {
            format!("NOT accepted: {}", seeded.acceptance.reasons())
        },
    );
}
