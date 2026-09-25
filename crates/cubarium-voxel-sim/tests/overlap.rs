//! **The overlapped tick** (`design/handoffs/voxel-phase-overlap-2026-09-24.md`): water
//! runs beside plants and animals, and plants and animals read the world as it stood when
//! the tick began — after the previous tick's water. That one-tick lag is the only rule
//! it changes. The plants' drink is buffered and applied at the barrier, where a cell the
//! water emptied in the same tick gives only what it still holds and the shortfall comes
//! off the stand that asked, so the water ledger still closes.
//!
//! Tiny tests: a few ticks each, one of about two hundred.

use bevy_ecs::prelude::{IntoScheduleConfigs, Res, ResMut, Resource};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{Command as FaunaCommand, Fauna, FaunaConfig, Species as Beast};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species};
use cubarium_voxel_sim::{Sim, SimConfig, TickPhase, VoxelWorld};

fn config(threads: usize, overlap: bool) -> SimConfig {
    SimConfig { threads, overlap }
}

fn rel(a: f64, b: f64) -> f64 {
    let scale = a.abs().max(b.abs()).max(1e-300);
    (a - b).abs() / scale
}

/// A world of one soil slab `width` columns wide, `depth` deep: bedrock at `y = 0`, soil at
/// `y = 1..=2` holding `pore` of its pore capacity, air above, so every support face is
/// `y = 2` in open sky. No rain, no evaporation, no aquifer: nothing but the plants and the
/// test's own hooks moves pore water.
fn slab(width: u32, depth: u32, pore: f64) -> World {
    let config = VoxelConfig {
        width,
        height: 8,
        depth,
        voxel_m: 1.0,
        seed: 5,
        ..VoxelConfig::default()
    };
    let mut w = World::empty(config);
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=2u32 {
                let want = pore * Material::Soil.pore_capacity() * w.config().voxel_volume();
                let got = w.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z,
                    volume_m3: want,
                });
                assert!((got - want).abs() < 1e-12, "the void took {got} of {want}");
                w.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    w
}

/// The water residual as the schedule tests judge it: absolute, on at least a cubic metre.
fn water_residual(world: &World) -> f64 {
    let v = world.view();
    (v.stored_m3() - v.ledger.expected_stored()).abs() / v.stored_m3().max(1.0)
}

/// Every skyline face of a generated world, lowest first.
fn skyline_of(world: &World) -> Vec<Site> {
    let (width, depth) = (world.config().width, world.config().depth);
    let mut out: Vec<Site> = Vec::new();
    for z in 0..depth {
        for x in 0..i64::from(width) {
            if let Some(site) = cubarium_voxel_flora::highest_support(&world.view(), x, z) {
                out.push(site);
            }
        }
    }
    out.sort_by_key(|s| (s.y, s.x, s.z));
    out
}

/// A generated world under steady rain with every species of plant drinking from it and
/// two grazers: the schedule tests' condition, shorter.
fn rained_on() -> (World, Flora, Fauna) {
    let dry = VoxelConfig {
        seed: 1,
        rain_m_per_s: 0.0002,
        ..VoxelConfig::default()
    };
    let floor = World::new(dry.clone())
        .outlet_cell()
        .map_or(0.0, |(_, y, _)| f64::from(y))
        * dry.voxel_m;
    let mut world = World::new(VoxelConfig {
        initial_aquifer_head_m: floor + 1.0,
        ..dry
    });
    world.apply(WorldCommand::SetOutlet { open: true });
    for _ in 0..60 {
        world.step_with(1);
    }
    let mut flora = Flora::new(FloraConfig::default());
    let skyline = skyline_of(&world);
    let mut taken: Vec<Site> = Vec::new();
    for species in Species::ALL {
        let pool: Vec<Site> = skyline
            .iter()
            .copied()
            .filter(|s| {
                !taken.contains(s) && flora.view().can_establish(&world.view(), *s, species)
            })
            .collect();
        let wood = 0.5 * flora.config().species(species).wood_max;
        let stride = (pool.len() / 6).max(1);
        for site in pool.iter().step_by(stride).take(6) {
            if flora.apply(
                &world,
                FloraCommand::Seed {
                    x: i64::from(site.x),
                    z: site.z,
                    species,
                    wood,
                },
            ) {
                taken.push(*site);
            }
        }
    }
    let mut fauna = Fauna::new(FaunaConfig::default());
    let body = fauna.config().species(Beast::Frondgrazer).body_max;
    for site in skyline.iter().step_by(skyline.len() / 2).take(2) {
        fauna.apply(
            &world,
            FaunaCommand::Introduce {
                x: i64::from(site.x),
                z: site.z,
                species: Beast::Frondgrazer,
                body,
            },
        );
    }
    (world, flora, fauna)
}

/// **1.** Two hundred overlapped ticks on the multi-threaded executor, with rain falling,
/// a shower pulse every fifty ticks and plants drinking: the water ledger closes, and what
/// the plants booked as transpired is what the world booked as transpiration.
#[test]
fn the_overlapped_tick_closes_the_water_ledger() {
    let (world, flora, fauna) = rained_on();
    let mut sim = Sim::new(world, flora, fauna, config(4, true), None);
    assert!(sim.config().overlap);
    for t in 0..200u64 {
        if t % 50 == 0 {
            sim.world_mut()
                .apply(WorldCommand::RainPulse { volume_m3: 2.0 });
        }
        sim.step();
        let r = water_residual(sim.world());
        assert!(r <= 1e-9, "tick {t}: water residual {r:.3e}");
    }
    let transpired = sim.flora().view().ledger.transpired_m3;
    let booked = sim.world().view().ledger.transpiration_out;
    assert!(transpired > 0.0, "the plants drank nothing");
    assert!(
        rel(transpired, booked) <= 1e-12,
        "flora transpired {transpired} against the world's {booked}"
    );
}

/// What the water-leg hook of test 2 does: on one tick, drain every soil cell's pore water
/// down to `left` cubic metres above its wilting point.
#[derive(Resource)]
struct Drain {
    on_tick: u64,
    left_m3: f64,
    taken_m3: f64,
}

fn drain_hook(mut w: ResMut<VoxelWorld>, mut d: ResMut<Drain>) {
    if w.0.tick() != d.on_tick {
        return;
    }
    let c = w.0.config().clone();
    let volume = c.voxel_volume();
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            for y in 0..c.height {
                let v = w.0.view();
                let m = v.material_at(x, y, z);
                if m != Material::Soil {
                    continue;
                }
                let drinkable =
                    (v.pore_at(x, y, z) - m.wilting_point()).max(0.0) * m.pore_capacity() * volume;
                let take = drinkable - d.left_m3;
                if take > 0.0 {
                    d.taken_m3 -= w.0.apply(WorldCommand::WithdrawPore {
                        x,
                        y,
                        z,
                        volume_m3: take,
                    });
                }
            }
        }
    }
}

/// **2.** A plant drinking from cells the water emptied in the same tick gets only what is
/// left, the shortfall comes off its stand, and the ledger is exact: the world booked the
/// hook's withdrawals plus the plant's, and the plant booked only its own.
#[test]
fn a_drink_from_a_cell_emptied_this_tick_gets_what_is_left() {
    for threads in [1usize, 2] {
        let world = slab(8, 1, 0.6);
        let mut flora = Flora::new(FloraConfig::default());
        let wood = 0.5 * flora.config().species(Species::Springturf).wood_max;
        assert!(flora.apply(
            &world,
            FloraCommand::Seed {
                x: 3,
                z: 0,
                species: Species::Springturf,
                wood,
            },
        ));
        let fauna = Fauna::new(FaunaConfig::default());
        let mut sim = Sim::new(world, flora, fauna, config(threads, true), None);

        // One undisturbed tick: what the stand asks for from full cells.
        sim.step();
        let asked = sim.flora().view().stands[0].water_m3;
        assert!(asked > 0.0, "{threads} threads: the stand drank nothing");

        // The next tick's water leg drains every soil cell to a hundredth of the ask.
        let cells = 8 * 2;
        let left = asked / (100.0 * f64::from(cells));
        let on_tick = sim.world().tick();
        sim.ecs().insert_resource(Drain {
            on_tick,
            left_m3: left,
            taken_m3: 0.0,
        });
        sim.add_live_systems(|| drain_hook.in_set(TickPhase::Water));
        let world_before = sim.world().view().ledger.transpiration_out;
        let flora_before = sim.flora().view().ledger.transpired_m3;
        sim.step();

        let hook = sim.ecs().resource::<Drain>().taken_m3;
        assert!(hook > 0.0, "{threads} threads: the hook drained nothing");
        let got = sim.flora().view().stands[0].water_m3;
        let flora_d = sim.flora().view().ledger.transpired_m3 - flora_before;
        let world_d = sim.world().view().ledger.transpiration_out - world_before;
        assert!(
            got > 0.0 && got <= f64::from(cells) * left * (1.0 + 1e-6),
            "{threads} threads: the stand got {got}, the cells held at most {}",
            f64::from(cells) * left
        );
        assert!(
            rel(got, flora_d) <= 1e-9,
            "{threads} threads: the stand's {got} is not the flora ledger's {flora_d}"
        );
        assert!(
            (world_d - hook - flora_d).abs() <= 1e-12 * world_d.abs(),
            "{threads} threads: world booked {world_d}, hook {hook} + plant {flora_d}"
        );
        assert!(water_residual(sim.world()) <= 1e-12);
    }
}

/// **3.** With `overlap` off the tick is the old chained one — water, then plants, then
/// animals, each reading the world the phase before it left — which is the three-call
/// sequence the core fixtures drive.
#[test]
fn overlap_off_is_the_chained_tick() {
    let (world, flora, fauna) = rained_on();
    let (mut w, mut fl, mut fa) = (world.clone(), flora.clone(), fauna.clone());
    let mut sim = Sim::new(world, flora, fauna, config(1, false), None);
    assert!(!sim.config().overlap);
    for _ in 0..30 {
        w.step_with(1);
        fl.step(&mut w);
        fa.step(&w, &mut fl);
        sim.step();
    }
    let (sw, sf, sa) = sim.layers();
    assert_eq!(sf.view().stands.len(), fl.view().stands.len());
    assert_eq!(sa.view().animals.len(), fa.view().animals.len());
    let stored = (sw.view().stored_m3(), w.view().stored_m3());
    assert!(rel(stored.0, stored.1) <= 1e-12, "stored {stored:?}");
    let transpired = (
        sf.view().ledger.transpired_m3,
        fl.view().ledger.transpired_m3,
    );
    assert!(
        rel(transpired.0, transpired.1) <= 1e-12,
        "transpired {transpired:?}"
    );
    for (a, b) in sf.view().stands.iter().zip(fl.view().stands.iter()) {
        assert_eq!(a.site, b.site);
        assert!(rel(a.water_m3, b.water_m3) <= 1e-12, "{:?}", a.site);
        assert!(rel(a.moisture, b.moisture) <= 1e-12, "{:?}", a.site);
    }
    for (a, b) in sa.view().animals.iter().zip(fa.view().animals.iter()) {
        assert_eq!((a.id, a.site), (b.id, b.site));
        assert!(rel(a.body, b.body) <= 1e-12);
    }
}

/// Test 4's hook: on one tick, flood every column two cells deep.
#[derive(Resource)]
struct Flood {
    on_tick: u64,
}

fn flood_hook(mut w: ResMut<VoxelWorld>, f: Res<Flood>) {
    if w.0.tick() != f.on_tick {
        return;
    }
    let c = w.0.config().clone();
    let volume = c.voxel_volume();
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            for y in 3..=4u32 {
                w.0.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z,
                    volume_m3: volume,
                });
            }
        }
    }
}

/// **4.** An animal in tick `t` sees the water depth from the end of tick `t − 1`: a flood
/// the water leg makes in tick `t` drowns an animal (the instant rule, deeper than its
/// `drown_depth_m`) in tick `t + 1` under overlap, and in tick `t` itself on the chained
/// tick.
#[test]
fn an_animal_sees_the_water_of_the_tick_before() {
    for (overlap, threads) in [(true, 1usize), (true, 2), (false, 1)] {
        let world = slab(6, 3, 0.4);
        let flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::default());
        let body = fauna.config().species(Beast::Frondgrazer).body_max;
        assert!(fauna.apply(
            &world,
            FaunaCommand::Introduce {
                x: 3,
                z: 1,
                species: Beast::Frondgrazer,
                body,
            },
        ));
        let mut sim = Sim::new(world, flora, fauna, config(threads, overlap), None);
        sim.ecs().insert_resource(Flood { on_tick: 0 });
        sim.add_live_systems(|| flood_hook.in_set(TickPhase::Water));
        let name = format!("overlap {overlap}, {threads} threads");

        sim.step();
        let site = cubarium_voxel_flora::highest_support(&sim.world().view(), 3, 1)
            .expect("the slab's face");
        let depth = sim
            .world()
            .view()
            .water_depth_m(i64::from(site.x), site.y, site.z);
        assert!(depth > 1.0, "{name}: the flood is {depth} m deep");
        let alive = sim.fauna().view().animals.len();
        if overlap {
            assert_eq!(alive, 1, "{name}: drowned in the tick the water rose");
            sim.step();
            assert_eq!(
                sim.fauna().view().animals.len(),
                0,
                "{name}: still alive a tick into the flood"
            );
        } else {
            assert_eq!(alive, 0, "{name}: the chained tick reads this tick's water");
        }
    }
}
