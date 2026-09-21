//! The seeded example habitat for `cubarium voxel`.
//!
//! A fresh `cubarium voxel` run starts with an empty ecology: a stand appears only from
//! an `f` stdin line and an animal only from a `g` line. That is the right contract for a
//! harness and the wrong one for `--sink gpu`, whose whole job is to put a living world on
//! a screen without anyone typing at it. This module is the smallest thing that closes the
//! gap: one deterministic founder habitat built entirely out of the flora and fauna
//! crates' own commands, with no model rule changed and no new mechanic.
//!
//! Since the live-founders package the animals it places are the two **sensed founder
//! lineages** — `Founder::Blind` (littershredder) and `Founder::Browser` (frondgrazer
//! founder) — introduced hungry through `Command::IntroduceFounder` with their own
//! observation-only heuristics installed. The legacy `Species::Frondgrazer` animal is
//! still in the fauna crate and still reachable from the `g` stdin line; this seeder
//! stopped placing it. Those heuristics are the seeder's floor: the ambient run installs
//! the trained P3-C centres over them by default (`crates/cubarium/assets/policies`),
//! and `--founder-heuristic` is what leaves this floor standing.
//!
//! **It is a stage-1 dev scene, not a tuned ecology.** The species are placed by simple
//! environment proxies — standing water, rock or soil, height band — every founder is
//! half-grown so a ground browser can reach a canopy, and the counts are chosen for a
//! populated picture. Nothing here is a balance claim, nothing here persists, and the
//! world it makes is the ordinary disposable development world. `--empty` asks for the
//! bare world back.

use std::sync::Arc;

use cubarium_voxel::{Material, VoxelView, World};
use cubarium_voxel_fauna::{
    BlindForager, BrowserForager, Command as FaunaCommand, Controller, Fauna, Founder,
    StartingStores,
};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraView, Site, Species, highest_support,
};

/// World ticks run before anything is planted. The authored fixture fills its pool to a
/// level and the first ticks redistribute that water across the basin; planting into the
/// pre-settle surface drowns low founders as the pool finds its level. Waiting for it to
/// settle is an ordering fix, not an ecological one — ten seconds of a disposable world.
const SETTLE_TICKS: u32 = 400;

/// A founder starts at half its own `wood_max`, which is `donor_min` for every preset: a
/// full-grown crown is above a ground browser's reach, and a half-grown patch is the
/// meadow this launch exists to show.
const FOUNDER_FRACTION: f64 = 0.5;

/// Dead wood laid under each glowcap: twice the fungus's `establish_substrate_min`, enough
/// for one grove to establish and spread along its log.
const LOG_ORGANIC: f64 = 0.4;

/// The five producers, and how many founders to spread over the skyline sites their
/// environment proxy claims. Counts are chosen for a populated picture, not a balance.
const PRODUCERS: [(Species, usize); 5] = [
    (Species::Springturf, 16),
    (Species::Bloomcrown, 14),
    (Species::Velvetpad, 10),
    (Species::Umbrellafrond, 8),
    (Species::Stonecushion, 6),
];

/// How many glowcaps the decomposer grove holds.
const GLOWCAPS: usize = 8;

/// How many **sensed founder bodies** of each lineage the habitat starts with: eight
/// littershredders ([`Founder::Blind`]) and eight frondgrazer founders
/// ([`Founder::Browser`]), the same count the legacy `Species::Frondgrazer`
/// introductions used, now split across the two lineages. The legacy species stays in
/// the fauna crate and is still reachable from the `g` stdin line; this seeder simply
/// stops placing it.
const SHREDDERS: usize = 8;
const BROWSERS: usize = 8;

/// Leaf litter laid under each littershredder, in organic-matter units.
///
/// A fresh world has **no litter at all** — litterfall is senescence, and nothing has
/// senesced before the first tick — so a blind litter feeder placed on bare soil would
/// start with nothing to smell and nothing to eat. This is the same move the glowcap
/// grove already makes with its log: lay the substrate the founder's own gate needs. At
/// 0.2 the tile saturates the cue's emission term (`min(litter / 0.05, 1)`) exactly as a
/// Stage-A arena tile does, so the founder starts on a signal it has actually been seen
/// to follow. It is a dev-scene starter, not a claim about a standing litter layer.
const LITTER_ORGANIC: f64 = 0.2;
/// Mineral and retained-energy densities of that starter litter: a plant tissue's order
/// of magnitude, and the litter energy cap. The same pair the frozen arena deposits at.
const LITTER_MINERAL_FRACTION: f64 = 0.02;
const LITTER_ENERGY_DENSITY: f64 = 2.0;

/// What [`seed`] put into the world.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Seeded {
    pub stands: usize,
    pub logs: usize,
    /// Founder bodies placed, by [`Founder::index`]: littershredders, then frondgrazer
    /// founders.
    pub founders: [usize; Founder::COUNT],
    /// Litter tiles laid for the littershredders.
    pub litter_tiles: usize,
}

impl Seeded {
    /// Every animal the seeder introduced.
    pub fn animals(&self) -> usize {
        self.founders.iter().sum()
    }
}

/// Seed the example habitat. The world is stepped briefly first so its initial water has
/// settled; then every unit enters through the ordinary founder, deposit and introduction
/// inflows the layers already name. Deterministic for a given world.
pub fn seed(world: &mut World, flora: &mut Flora, fauna: &mut Fauna) -> Seeded {
    for _ in 0..SETTLE_TICKS {
        world.step();
    }

    let skyline = skyline_of(world);
    let Some(highest_y) = skyline.last().map(|s| s.y) else {
        return Seeded::default();
    };
    let view = world.view();
    let mut taken: Vec<Site> = Vec::new();
    let mut seeded = Seeded::default();

    // The five producers, each on the settled sites its environment proxy claims and its
    // own drowning limit allows. The proxies are terrain sectors, so the picture comes out
    // banded by landform rather than sprinkled evenly.
    for (species, want) in PRODUCERS {
        let drown = flora.config().species(species).drown_depth_m;
        let pool: Vec<Site> = skyline
            .iter()
            .copied()
            .filter(|s| {
                if taken.contains(s) || species_for(&view, *s, highest_y) != species {
                    return false;
                }
                let depth = view.water_depth_m(i64::from(s.x), s.y, s.z);
                if species == Species::Umbrellafrond {
                    // The wetland producer stands in the shallow water its proxy names.
                    depth > 0.0 && depth <= drown
                } else {
                    // Everything else is placed on settled dry ground: a thin film deeper
                    // than the low drowning limits would take a founder in its first ticks.
                    depth <= 0.0
                }
            })
            .collect();
        let wood = FOUNDER_FRACTION * flora.config().species(species).wood_max;
        for site in strided(&pool, want) {
            if flora.apply(
                world,
                FloraCommand::Seed {
                    x: i64::from(site.x),
                    z: site.z,
                    species,
                    wood,
                },
            ) {
                taken.push(site);
                seeded.stands += 1;
            }
        }
    }

    // The decomposer grove: a log first, then the fungus that eats it. A fresh world has
    // no dead wood at all, so the log is the habitat and glowcap's own substrate gate
    // could not pass before it is laid.
    let (glowcap_n_tissue, glowcap_energy_density, glowcap_wood_max) = {
        let sc = flora.config().species(Species::Glowcap);
        (sc.n_tissue, sc.energy_density, sc.wood_max)
    };
    let pool: Vec<Site> = skyline
        .iter()
        .copied()
        .filter(|s| {
            !taken.contains(s)
                && view.material_at(i64::from(s.x), s.y, s.z) == Material::Soil
                && view.water_depth_m(i64::from(s.x), s.y, s.z) <= 0.0
        })
        .collect();
    for site in strided(&pool, GLOWCAPS) {
        if flora.deposit(
            site,
            Deposit {
                kind: DepositKind::DeadWood,
                organic: LOG_ORGANIC,
                mineral: glowcap_n_tissue * LOG_ORGANIC,
                energy: glowcap_energy_density * LOG_ORGANIC,
            },
        ) {
            seeded.logs += 1;
        }
        let wood = FOUNDER_FRACTION * glowcap_wood_max;
        if flora.apply(
            world,
            FloraCommand::Seed {
                x: i64::from(site.x),
                z: site.z,
                species: Species::Glowcap,
                wood,
            },
        ) {
            taken.push(site);
            seeded.stands += 1;
        }
    }

    // The two **sensed founder lineages**, which is what walks this habitat now. Both
    // arrive hungry (`StartingStores::HUNGRY`, the arenas' P2-C start): a body placed
    // full has nowhere to put what it eats, so eating would be worth nothing to it.
    // The seeder installs each lineage's own observation-only heuristic through the
    // fauna layer's ordinary controller boundary, and registers it as that lineage's
    // **birth factory** too, so a founder born here is handed a fresh controller of its
    // parent's kind instead of resting for ever while paying upkeep.
    //
    // That heuristic is the seeder's floor, not the live run's default: the ambient run
    // replaces both the factory and the standing bodies' controllers afterwards with the
    // trained P3-C centre built into the binary (`crates/cubarium/assets/policies`),
    // unless `--founder-heuristic` asks for this floor as the disclosed control or
    // `--founder-policy` names another centre. A caller that seeds a habitat without
    // going through `cubarium voxel` gets the heuristics and nothing else.
    install_heuristics(fauna);

    // Littershredders on litter-bearing soil: dry open soil away from the stands, each
    // with its own starter tile of leaf litter laid under it first, so the founder
    // begins on a cue it can smell and a stock it can bite.
    let blind_drown = fauna.config().founder(Founder::Blind).core.drown_depth_m;
    let pool: Vec<Site> = skyline
        .iter()
        .copied()
        .filter(|s| {
            !taken.contains(s)
                && view.material_at(i64::from(s.x), s.y, s.z) == Material::Soil
                && view.water_depth_m(i64::from(s.x), s.y, s.z) <= 0.0
                && view.water_depth_m(i64::from(s.x), s.y, s.z) <= blind_drown
        })
        .collect();
    for (k, site) in strided(&pool, SHREDDERS).into_iter().enumerate() {
        if !flora.deposit(
            site,
            Deposit {
                kind: DepositKind::Litter,
                organic: LITTER_ORGANIC,
                mineral: LITTER_ORGANIC * LITTER_MINERAL_FRACTION,
                energy: LITTER_ORGANIC * LITTER_ENERGY_DENSITY,
            },
        ) {
            continue;
        }
        seeded.litter_tiles += 1;
        if introduce_founder(
            world,
            fauna,
            Founder::Blind,
            site,
            spread_heading(k, SHREDDERS),
        ) {
            taken.push(site);
            seeded.founders[Founder::Blind.index()] += 1;
        }
    }

    // Frondgrazer founders on open soil in the meadow. "Open soil" is the legacy
    // grazers' pool; the meadow part is the founder's own limit and not a route — it
    // browses through a 2 m cone and a mouth that reaches a quarter of a body length,
    // with no target search at all, so a body dropped on empty ground has nothing its
    // senses can act on. `browser_faces` returns the dry soil faces a **half-grown crown
    // already covers at the body's own layer**, which is the same geometry the frozen
    // arena gives its browser. It falls back to plain open soil if this landform grew no
    // reachable crown, rather than placing nothing.
    let meadow = browser_faces(&view, &flora.view(), fauna);
    let browser_drown = fauna.config().founder(Founder::Browser).core.drown_depth_m;
    let pool: Vec<Site> = if meadow.is_empty() {
        skyline
            .iter()
            .copied()
            .filter(|s| {
                view.material_at(i64::from(s.x), s.y, s.z) == Material::Soil
                    && view.water_depth_m(i64::from(s.x), s.y, s.z) <= browser_drown
            })
            .collect()
    } else {
        meadow
    };
    for (k, site) in strided(&pool, BROWSERS).into_iter().enumerate() {
        if introduce_founder(
            world,
            fauna,
            Founder::Browser,
            site,
            spread_heading(k, BROWSERS),
        ) {
            seeded.founders[Founder::Browser.index()] += 1;
        }
    }

    seeded
}

/// Register each lineage's own observation-only heuristic as its birth factory: every
/// founder **born** in this layer is driven by one of these unless a caller replaces
/// them. Introduced bodies get theirs from [`introduce_founder`], out of the same
/// recipes.
pub fn install_heuristics(fauna: &mut Fauna) {
    fauna.set_founder_factory(
        Founder::Blind,
        Arc::new(|| -> Box<dyn Controller> { Box::new(BlindForager::new()) }),
    );
    fauna.set_founder_factory(
        Founder::Browser,
        Arc::new(|| -> Box<dyn Controller> { Box::new(BrowserForager::new()) }),
    );
}

/// One founder body on a column's highest support face, hungry, with its own heuristic
/// installed. Returns whether the layer accepted it.
fn introduce_founder(
    world: &World,
    fauna: &mut Fauna,
    founder: Founder,
    site: Site,
    heading_rad: f64,
) -> bool {
    if !fauna.apply(
        world,
        FaunaCommand::IntroduceFounder {
            x: i64::from(site.x),
            z: site.z,
            founder,
            stores: StartingStores::HUNGRY,
            heading_rad,
        },
    ) {
        return false;
    }
    let id = fauna.view().ledger.births - 1;
    fauna.install_founder_controller(id, founder)
}

/// Headings spread evenly around the circle, one per body. Deterministic, and nothing
/// about the world reaches it: a founder is not aimed at its food.
fn spread_heading(k: usize, of: usize) -> f64 {
    std::f64::consts::TAU * (k as f64) / (of.max(1) as f64)
}

/// The dry soil faces a seeded crown already reaches at the body's own layer: a support
/// face `s` such that some standing crown occupies the column of `s` at height
/// `s.y + 1`, which is exactly the contact [`cubarium_voxel_fauna`]'s browser mouth
/// tests for. Sorted low to high, like the skyline the other pools come from.
fn browser_faces(view: &VoxelView<'_>, fv: &FloraView<'_>, fauna: &Fauna) -> Vec<Site> {
    let drown = fauna.config().founder(Founder::Browser).core.drown_depth_m;
    let width = i64::from(view.config.width);
    let depth = i64::from(view.config.depth);
    let mut out: Vec<Site> = Vec::new();
    for stand in fv.stands.iter().filter(|s| s.foliage > 0.0) {
        let sc = fv.config.species(stand.species);
        let crown_y = i64::from(stand.site.y) + i64::from(sc.crown_voxels(stand.wood));
        let radius = sc.crown_radius(stand.wood).max(0.0);
        let span = radius.floor() as i64;
        let r2 = radius * radius;
        for dz in -span..=span {
            for dx in -span..=span {
                if (dx * dx + dz * dz) as f64 > r2 {
                    continue;
                }
                let cx = (i64::from(stand.site.x) + dx).rem_euclid(width);
                let cz = i64::from(stand.site.z) + dz;
                if cz < 0 || cz >= depth {
                    continue;
                }
                let Some(face) = highest_support(view, cx, cz as u32) else {
                    continue;
                };
                if i64::from(face.y) + 1 != crown_y {
                    continue;
                }
                if view.material_at(cx, face.y, face.z) != Material::Soil {
                    continue;
                }
                if view.water_depth_m(cx, face.y, face.z) > drown {
                    continue;
                }
                if !out.contains(&face) {
                    out.push(face);
                }
            }
        }
    }
    out.sort_by_key(|s| (s.y, s.x, s.z));
    out
}

/// Every column's highest support face, sorted low to high, so a strided sample spreads
/// over the whole strip rather than one end of it.
fn skyline_of(world: &World) -> Vec<Site> {
    let view = world.view();
    let (width, depth) = (world.config().width, world.config().depth);
    let mut skyline: Vec<Site> = Vec::new();
    for z in 0..depth {
        for x in 0..width as i64 {
            if let Some(site) = highest_support(&view, x, z) {
                skyline.push(site);
            }
        }
    }
    skyline.sort_by_key(|s| (s.y, s.x, s.z));
    skyline
}

/// Which producer belongs on a support face: the wetland one where water stands on it, the
/// rock cushion on bare rock, then one of the soil species by height band. A dev-scene
/// proxy for the model's own establishment gates, not a claim about the real niches.
fn species_for(view: &VoxelView<'_>, site: Site, highest_y: u32) -> Species {
    let x = i64::from(site.x);
    if view.water_depth_m(x, site.y, site.z) > 0.0 {
        return Species::Umbrellafrond;
    }
    if view.material_at(x, site.y, site.z) == Material::Rock {
        return Species::Stonecushion;
    }
    let rel = f64::from(site.y) / f64::from(highest_y.max(1));
    if rel > 0.62 {
        Species::Bloomcrown
    } else if rel > 0.38 {
        Species::Springturf
    } else {
        Species::Velvetpad
    }
}

/// A fixed stride sample of `want` items out of `pool`: the whole pool spread over, never
/// the same end of it twice. Returns fewer when the pool is smaller than `want`.
fn strided<T: Copy>(pool: &[T], want: usize) -> Vec<T> {
    if pool.is_empty() || want == 0 {
        return Vec::new();
    }
    let stride = (pool.len() / want).max(1);
    pool.iter().step_by(stride).take(want).copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_voxel::Config;

    fn config() -> Config {
        Config {
            width: 64,
            height: 24,
            depth: 6,
            ..Config::default()
        }
    }

    fn seeded(world: World) -> (World, Flora, Fauna, Seeded) {
        let mut world = world;
        let mut flora = Flora::new(Default::default());
        let mut fauna = Fauna::new(Default::default());
        let summary = seed(&mut world, &mut flora, &mut fauna);
        (world, flora, fauna, summary)
    }

    /// The one thing the launch needs: a populated world, from the same world bytes twice.
    #[test]
    fn the_example_habitat_is_populated_and_deterministic() {
        let a = seeded(crate::voxel::scene::authored(config())).3;
        let b = seeded(crate::voxel::scene::authored(config())).3;
        assert_eq!(a, b, "the same world seeds the same habitat");
        assert!(a.stands >= 10, "a populated picture, not a specimen: {a:?}");
        assert!(a.logs > 0, "the glowcap grove has its wood: {a:?}");
        assert!(a.animals() > 0, "something moves: {a:?}");
        for founder in Founder::ALL {
            assert!(
                a.founders[founder.index()] > 0,
                "the habitat introduces {}: {a:?}",
                founder.name()
            );
        }
        assert!(
            a.litter_tiles >= a.founders[Founder::Blind.index()],
            "every littershredder got its starter tile: {a:?}"
        );
    }

    /// **Both founder lineages eat in the seeded habitat.** The bodies are the seeder's
    /// own — hungry, on the faces it chose, driven by the heuristics it installed — and
    /// they run through the live schedule with the live litter field, which is what the
    /// ambient run does. The evidence for a bite is the founder's own `Self`
    /// `assimilated_intake` channel: the prior interval's assimilated organic matter,
    /// read out of the packet the controller was handed. Nothing fixture-side reaches
    /// it, and nothing here reads a site or a stock.
    #[test]
    fn both_founder_kinds_take_a_bite_in_the_first_ten_seconds() {
        use cubarium_voxel_fauna::{Response, Senses};
        use cubarium_voxel_sim::{Sim, SimConfig};
        use std::sync::{Arc, Mutex};

        /// Passes the packet through untouched and remembers the largest intake channel
        /// it ever carried.
        struct Watched {
            inner: Box<dyn Controller>,
            intake: usize,
            best: Arc<Mutex<f64>>,
        }
        impl Controller for Watched {
            fn drive(&mut self, o: &[f64]) -> Response {
                let mut best = self.best.lock().expect("the sink");
                *best = best.max(o[self.intake]);
                drop(best);
                self.inner.drive(o)
            }
            fn reset(&mut self) {
                self.inner.reset();
            }
        }

        let (world, flora, mut fauna, seeded) = seeded(crate::voxel::scene::authored(config()));
        // One watcher per founder kind, wrapped around the controller the seeder
        // installed, so the heuristic under test is the one the habitat ships.
        let best: [Arc<Mutex<f64>>; Founder::COUNT] = std::array::from_fn(|_| Arc::default());
        let bodies: Vec<(u64, Founder)> = fauna
            .view()
            .animals
            .iter()
            .filter_map(|a| a.founder.map(|f| (a.id, f)))
            .collect();
        assert_eq!(bodies.len(), seeded.animals(), "every animal is a founder");
        for (id, founder) in bodies {
            let intake = founder
                .manifest()
                .modules
                .iter()
                .find(|m| m.name == "Self")
                .expect("every schema opens with Self")
                .offset
                + 4;
            let inner = fauna.take_controller(id).expect("the seeder installed one");
            assert!(fauna.set_controller(
                id,
                Box::new(Watched {
                    inner,
                    intake,
                    best: Arc::clone(&best[founder.index()]),
                }),
            ));
        }

        let mut senses = Senses::new();
        senses.settle(&world.view(), &flora.view());
        let mut sim = Sim::new(world, flora, fauna, SimConfig { threads: 1 }, Some(senses));
        for _ in 0..200 {
            sim.step();
        }
        for founder in Founder::ALL {
            let seen = *best[founder.index()].lock().expect("the sink");
            assert!(
                seen > 0.0,
                "no {} reported any assimilated intake in 200 ticks",
                founder.name()
            );
        }
    }

    /// A few coupled ticks, the fast-iteration bar: the seeded founders stand, the layers
    /// step, and the two ledgers stay closed. Not a study, and deliberately not long.
    #[test]
    fn the_seeded_habitat_steps_with_closed_ledgers() {
        let (mut world, mut flora, mut fauna, seeded) =
            seeded(crate::voxel::scene::authored(config()));
        let before: Vec<(u64, Species)> = flora
            .view()
            .stands
            .iter()
            .map(|s| (s.id, s.species))
            .collect();
        for _ in 0..40 {
            world.step();
            flora.step(&mut world);
            fauna.step(&world, &mut flora);
        }
        let fv = flora.view();
        let av = fauna.view();
        let now: std::collections::BTreeSet<u64> = fv.stands.iter().map(|s| s.id).collect();
        let died: Vec<(u64, &str)> = before
            .iter()
            .filter(|(id, _)| !now.contains(id))
            .map(|(id, sp)| (*id, sp.name()))
            .collect();
        assert!(
            fv.stands.len() == seeded.stands,
            "seeded founder(s) {died:?} died in the first two settled seconds; stands {} of {}",
            fv.stands.len(),
            seeded.stands
        );
        for (got, expected) in [
            (fv.organic() - fv.ledger.expected_organic(), "organic"),
            (fv.mineral() - fv.ledger.expected_mineral(), "mineral"),
            (fv.energy() - fv.ledger.expected_energy(), "energy"),
        ] {
            assert!(got.abs() < 1e-6, "flora {expected} residual {got:e}");
        }
        for (got, expected) in [
            (av.organic() - av.ledger.expected_organic(), "organic"),
            (av.mineral() - av.ledger.expected_mineral(), "mineral"),
            (av.energy() - av.ledger.expected_energy(), "energy"),
        ] {
            assert!(got.abs() < 1e-9, "fauna {expected} residual {got:e}");
        }
    }
}
