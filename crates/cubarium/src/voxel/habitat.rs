//! The seeded example habitat for `cubarium voxel`.
//!
//! A fresh `cubarium voxel` run starts with an empty ecology: a stand appears only from
//! an `f` stdin line and an animal only from a `g` line. That is the right contract for a
//! harness and the wrong one for `--sink gpu`, whose whole job is to put a living world on
//! a screen without anyone typing at it. This module is the smallest thing that closes the
//! gap: one deterministic founder habitat built entirely out of the flora and fauna
//! crates' own commands, with no model rule changed and no new mechanic.
//!
//! **It is a stage-1 dev scene, not a tuned ecology.** The species are placed by simple
//! environment proxies — standing water, rock or soil, height band — every founder is
//! half-grown so a ground browser can reach a canopy, and the counts are chosen for a
//! populated picture. Nothing here is a balance claim, nothing here persists, and the
//! world it makes is the ordinary disposable development world. `--empty` asks for the
//! bare world back.

use cubarium_voxel::{Material, VoxelView, World};
use cubarium_voxel_fauna::{Command as FaunaCommand, Fauna, Species as Beast};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, Site, Species, highest_support,
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

/// How many glowcaps the decomposer grove holds, and how many browsers walk the meadow.
const GLOWCAPS: usize = 8;
const GRAZERS: usize = 8;

/// What [`seed`] put into the world.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Seeded {
    pub stands: usize,
    pub logs: usize,
    pub animals: usize,
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

    // Ground browsers on open soil, spread so several patches are being grazed. They
    // sense and walk to the nearest foliage within their own radius.
    let grazer_drown = fauna.config().species(Beast::Frondgrazer).drown_depth_m;
    let pool: Vec<Site> = skyline
        .iter()
        .copied()
        .filter(|s| {
            view.material_at(i64::from(s.x), s.y, s.z) == Material::Soil
                && view.water_depth_m(i64::from(s.x), s.y, s.z) <= grazer_drown
        })
        .collect();
    let body = fauna.config().species(Beast::Frondgrazer).body_max;
    for site in strided(&pool, GRAZERS) {
        if fauna.apply(
            world,
            FaunaCommand::Introduce {
                x: i64::from(site.x),
                z: site.z,
                species: Beast::Frondgrazer,
                body,
            },
        ) {
            seeded.animals += 1;
        }
    }

    seeded
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
        assert!(a.animals > 0, "something moves: {a:?}");
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
