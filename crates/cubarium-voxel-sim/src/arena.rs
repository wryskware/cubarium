//! **The static phase-one arena**: a 32 × 16 × 12 voxel world at 0.25 m/voxel, prepared
//! once from a layout seed, with a finite consumable resource layout and the founder body
//! placed off it.
//!
//! This is the fixture `design/voxel-senses-phase1-plan.md`'s "Frozen arena contract"
//! describes. Terrain and water are laid down by [`Arena::build`] and then never evolve:
//! the arena is run through [`Sim::new_static`], whose schedule has no `Water` and no
//! `Flora` leg. What still runs is the production fauna tick — the same `sys_fauna` the
//! live schedule runs — so an animal placed here ages, pays maintenance and dies by the
//! real rules, and P1-B/P1-C add the real motion, contacts, senses and feeding behind the
//! same interfaces.
//!
//! # The two layouts
//!
//! [`Founder::Blind`] gets **litter** deposited on the ground: the finite stock the litter
//! feeder will eat. [`Founder::Browser`] gets **springturf** stands, each with full
//! foliage: the finite stock the browser will crop. Both are finite because neither
//! decomposition nor growth runs in the static schedule. [`Arena::resource_stock`] reads
//! the live total, so a test can watch a bite lower it.
//!
//! # What P1-A does *not* do
//!
//! The placed founder is idle. It has a continuous pose and a support face, but no
//! forward/turn/feed resolution, no sensory sample and no field; those are P1-B and P1-C.
//! P1-A's acceptance is that the world holds still and the body does not.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{Command as FaunaCommand, Fauna, FaunaConfig, Founder, Pose};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, Site, Species as Plant,
    Taken,
};

use crate::Sim;

/// Arena width in voxels; `x` wraps.
pub const ARENA_WIDTH: u32 = 32;
/// Arena height in voxels.
pub const ARENA_HEIGHT: u32 = 16;
/// Arena depth in voxels.
pub const ARENA_DEPTH: u32 = 12;
/// Edge length of one voxel, metres.
pub const ARENA_VOXEL_M: f64 = 0.25;
/// The `y` of the all-soil ground every column supports at.
pub const GROUND_Y: u32 = 4;

/// Fraction of a void cell a prepared pond cell is filled to.
const POND_FILL: f64 = 0.8;
/// Organic matter in one litter tile.
const LITTER_PER_TILE: f64 = 0.2;
/// Litter mineral fraction (a plant tissue's order of magnitude).
const LITTER_MINERAL_FRACTION: f64 = 0.02;
/// Litter retained-energy density, at the litter energy cap.
const LITTER_ENERGY_DENSITY: f64 = 2.0;
/// How many resource tiles a default layout lays.
const RESOURCE_TILES: usize = 6;

/// Deterministic scalar stream, splitmix64. No clock, no thread state.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }
}

/// A built arena and the founder it placed.
pub struct Arena {
    /// Which founder this arena feeds.
    pub founder: Founder,
    /// The layout seed the terrain, pond and resources were drawn from.
    pub layout_seed: u64,
    pub world: World,
    pub flora: Flora,
    pub fauna: Fauna,
    /// The finite resource sites, in column order: litter tiles or foliage stands.
    pub resources: Vec<Site>,
    /// The id of the idle founder body placed at build time, if it landed.
    pub animal_id: Option<u64>,
}

impl Arena {
    /// Build the frozen arena for `founder` from `layout_seed`, and place one idle founder
    /// body off the food. The world is not stepped: the pond is a direct `AddWater` and the
    /// ground is direct `SetMaterial`, so there is no settling cost and nothing to freeze
    /// afterwards.
    pub fn build(founder: Founder, layout_seed: u64) -> Arena {
        let config = VoxelConfig {
            width: ARENA_WIDTH,
            height: ARENA_HEIGHT,
            depth: ARENA_DEPTH,
            voxel_m: ARENA_VOXEL_M,
            seed: layout_seed,
            ..VoxelConfig::default()
        };
        let mut world = World::empty(config);
        world.apply(WorldCommand::SetOutlet { open: false });

        // Ground: soil 1..=GROUND_Y, bedrock already at 0. Wrap the x walk so the seam
        // column is an ordinary interior column.
        for z in 0..ARENA_DEPTH {
            for x in 0..ARENA_WIDTH as i64 {
                for y in 1..=GROUND_Y {
                    world.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }

        let mut rng = Rng::new(layout_seed);
        let voxel_volume = ARENA_VOXEL_M * ARENA_VOXEL_M * ARENA_VOXEL_M;
        let mut pond = vec![false; (ARENA_WIDTH * ARENA_DEPTH) as usize];
        // A shallow prepared pond: a rock-free patch of ground with free water standing on
        // it. One direct write, never solved, and the static schedule never moves it.
        let px = rng.below(ARENA_WIDTH as usize) as i64;
        let pz = rng.below(ARENA_DEPTH as usize) as u32;
        for dz in 0..2u32 {
            for dx in 0..4i64 {
                let x = px + dx;
                let z = pz + dz;
                if z >= ARENA_DEPTH {
                    continue;
                }
                let idx = (x.rem_euclid(ARENA_WIDTH as i64) as usize) * ARENA_DEPTH as usize
                    + z as usize;
                pond[idx] = true;
                let _ = world.apply(WorldCommand::AddWater {
                    x,
                    y: GROUND_Y + 1,
                    z,
                    volume_m3: POND_FILL * voxel_volume,
                });
            }
        }

        let mut flora = Flora::new(FloraConfig::default());

        // Candidate support faces: soil ground, not the pond, and — unless this layout is
        // the seam layout — at least two columns off the wrapped seam, so a default layout
        // does not accidentally test wrap geometry.
        let seam_layout = layout_seed % 4 == 3;
        let mut candidates: Vec<Site> = Vec::new();
        for z in 0..ARENA_DEPTH {
            for x in 0..ARENA_WIDTH as i64 {
                let idx = (x as usize) * ARENA_DEPTH as usize + z as usize;
                if pond[idx] {
                    continue;
                }
                if !seam_layout && (x < 2 || x >= ARENA_WIDTH as i64 - 2) {
                    continue;
                }
                candidates.push(Site {
                    x: x as u32,
                    y: GROUND_Y,
                    z,
                });
            }
        }
        // A seam layout explicitly lays one tile on the seam column, so the wrapped geometry
        // gets reused at least once. The pick below adds it after the interior tiles.

        let mut resources: Vec<Site> = Vec::new();
        let want = RESOURCE_TILES.min(candidates.len());
        for _ in 0..want {
            let pick = rng.below(candidates.len());
            resources.push(candidates.swap_remove(pick));
        }
        if seam_layout {
            resources.push(Site {
                x: 0,
                y: GROUND_Y,
                z: rng.below(ARENA_DEPTH as usize) as u32,
            });
        }
        resources.sort();

        for &site in &resources {
            match founder {
                Founder::Blind => {
                    let organic = LITTER_PER_TILE;
                    let accepted = flora.deposit(
                        site,
                        Deposit {
                            kind: DepositKind::Litter,
                            organic,
                            mineral: organic * LITTER_MINERAL_FRACTION,
                            energy: organic * LITTER_ENERGY_DENSITY,
                        },
                    );
                    debug_assert!(accepted, "a litter deposit on a support face");
                }
                Founder::Browser => {
                    let wood = 0.5 * flora.config().species(Plant::Springturf).wood_max;
                    let accepted = flora.apply(
                        &world,
                        FloraCommand::Seed {
                            x: i64::from(site.x),
                            z: site.z,
                            species: Plant::Springturf,
                            wood,
                        },
                    );
                    debug_assert!(accepted, "a springturf founder on a support face");
                }
            }
        }

        // An idle founder, off the food: pick the candidate site farthest from every
        // resource column, so "off-food" is geometry and not a claim.
        let start = candidates
            .iter()
            .copied()
            .filter(|s| !resources.contains(s))
            .max_by_key(|s| {
                resources
                    .iter()
                    .map(|r| {
                        let dx = (i64::from(s.x) - i64::from(r.x)).abs();
                        let wrapped = dx.min(ARENA_WIDTH as i64 - dx.min(ARENA_WIDTH as i64));
                        wrapped + (i64::from(s.z) - i64::from(r.z)).abs()
                    })
                    .min()
                    .unwrap_or(0)
            });

        let mut fauna = Fauna::new(FaunaConfig::default());
        // Isolated arenas disable paid births (plan, "Frozen arena contract").
        fauna.set_births_enabled(false);
        let manifest = founder.manifest();
        let mut animal_id = None;
        if let Some(site) = start {
            let body = manifest.body_reference;
            let heading_rad = (rng.next_u64() % 4) as f64 * std::f64::consts::FRAC_PI_2;
            if fauna.apply(
                &world,
                FaunaCommand::IntroduceFounder {
                    x: i64::from(site.x),
                    z: site.z,
                    founder,
                    body,
                    heading_rad,
                },
            ) {
                animal_id = Some(fauna.view().ledger.births - 1);
            }
        }

        Arena {
            founder,
            layout_seed,
            world,
            flora,
            fauna,
            resources,
            animal_id,
        }
    }

    /// The live finite stock in the resource layout: litter organic matter for a blind
    /// arena, stand foliage for a browser arena. This is what a bite lowers.
    pub fn resource_stock(&self) -> f64 {
        let fv = self.flora.view();
        self.resources
            .iter()
            .map(|&site| match self.founder {
                Founder::Blind => fv.ground_at(site).map_or(0.0, |g| g.litter),
                Founder::Browser => fv.stand_at(site).map_or(0.0, |s| s.foliage),
            })
            .sum()
    }

    /// The placed founder's pose, if it landed.
    pub fn animal_pose(&self) -> Option<Pose> {
        self.animal_id
            .and_then(|id| self.fauna.view().animal(id))
            .map(|a| a.pose)
    }

    /// Take up to `want` of the finite stock off one resource site, through the plant
    /// layer's production withdrawals — the same call P1-B's litter feeder and browser will
    /// make. `None` when the site holds nothing.
    pub fn take(&mut self, site: Site, want: f64) -> Option<Taken> {
        match self.founder {
            Founder::Blind => self.flora.take_litter(site, want),
            Founder::Browser => self.flora.take_foliage(site, want),
        }
    }

    /// Move the built layers into a [`Sim`] in [`crate::ScheduleMode::Static`].
    pub fn into_sim(self, config: crate::SimConfig) -> Sim {
        Sim::new_static(self.world, self.flora, self.fauna, config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ScheduleMode, SimConfig};

    const SEEDS: [u64; 3] = [1, 2, 3];

    #[test]
    fn the_arena_is_the_frozen_size_with_finite_food() {
        for seed in SEEDS {
            for founder in Founder::ALL {
                let arena = Arena::build(founder, seed);
                let c = arena.world.config();
                assert_eq!(
                    (c.width, c.height, c.depth, c.voxel_m),
                    (32, 16, 12, 0.25)
                );
                assert!(!arena.resources.is_empty(), "{founder:?} seed {seed}");
                assert!(
                    arena.resource_stock() > 0.0,
                    "{founder:?} seed {seed}: the layout is not empty"
                );
                assert!(
                    !arena.fauna.births_enabled(),
                    "arenas disable paid births"
                );
                let animal = arena
                    .animal_id
                    .and_then(|id| arena.fauna.view().animal(id))
                    .expect("an idle founder is placed");
                assert_eq!(animal.founder, Some(founder));
                assert!(arena.resources.iter().all(|r| *r != animal.site));
            }
        }
    }

    /// The static arena steps the world's and the fauna's clocks, ages and maintains the
    /// body, and **holds the terrain, water and unrelated plants still**. No motion, no
    /// feeding.
    #[test]
    fn a_static_arena_advances_time_and_freezes_the_world() {
        let arena = Arena::build(Founder::Browser, 7);
        let before_material = arena.world.view().material.to_vec();
        let before_free = arena.world.view().free.to_vec();
        let before_stands = arena.flora.view().stands.len();
        let before_stock = arena.resource_stock();
        let before_animal = *arena
            .fauna
            .view()
            .animal(arena.animal_id.unwrap())
            .unwrap();

        let mut sim = arena.into_sim(SimConfig { threads: 1 });
        assert_eq!(sim.mode(), ScheduleMode::Static);
        for _ in 0..10 {
            sim.step();
        }

        assert_eq!(sim.world().tick(), 10);
        assert_eq!(sim.fauna().tick(), 10);
        assert_eq!(sim.world().view().material, before_material, "terrain moved");
        assert_eq!(sim.world().view().free, before_free, "water moved");
        assert_eq!(sim.flora().view().stands.len(), before_stands, "a stand changed");

        let after = *sim
            .fauna()
            .view()
            .animal(before_animal.id)
            .expect("the idle body survives ten ticks");
        assert_eq!(after.age_ticks, before_animal.age_ticks + 10);
        assert!(
            after.organic() < before_animal.organic(),
            "maintenance was paid out of the body's own organic matter"
        );
        assert_eq!(after.site, before_animal.site, "the idle body moved");
        assert_eq!(after.pose, before_animal.pose, "the idle body's pose moved");
        assert_eq!(after.state, cubarium_voxel_fauna::State::Resting);

        let (_, flora, _) = sim.layers();
        let stock = match before_animal.founder {
            Some(Founder::Browser) => arena_stock(flora),
            _ => unreachable!("browser arena"),
        };
        assert_eq!(stock, before_stock, "the idle body ate nothing");
    }

    fn arena_stock(flora: &Flora) -> f64 {
        // The browser layout's stock: every springturf stand's foliage.
        flora
            .view()
            .stands
            .iter()
            .filter(|s| s.species == Plant::Springturf)
            .map(|s| s.foliage)
            .sum()
    }

    /// The finite stock is real and earnable through the production withdrawals.
    #[test]
    fn the_finite_food_is_actually_takeable() {
        let mut arena = Arena::build(Founder::Blind, 1);
        let site = arena.resources[0];
        let before = arena.resource_stock();
        let taken = arena.take(site, 0.05).expect("litter off the tile");
        assert!(taken.organic > 0.0);
        assert!(arena.resource_stock() < before);

        let mut browser = Arena::build(Founder::Browser, 1);
        let site = browser.resources[0];
        let before = browser.resource_stock();
        let taken = browser.take(site, 0.01).expect("foliage off the stand");
        assert!(taken.organic > 0.0);
        assert!(browser.resource_stock() < before);
    }

    /// The live schedule is still the default; only an explicit static construction moves
    /// off it.
    #[test]
    fn live_is_still_the_default_mode() {
        let sim = Sim::from_configs(
            VoxelConfig::default(),
            FloraConfig::default(),
            FaunaConfig::default(),
            SimConfig { threads: 1 },
        );
        assert_eq!(sim.mode(), ScheduleMode::Live);
    }
}