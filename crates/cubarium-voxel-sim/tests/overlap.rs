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
use cubarium_voxel_fauna::{Command as FaunaCommand, Fauna, FaunaConfig, Founder, StartingStores};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species};
use cubarium_voxel_sim::{Sim, SimConfig, TickPhase, VoxelWorld};

fn config(threads: usize, overlap: bool) -> SimConfig {
    SimConfig { threads, overlap, water_every: 1 }
}

#[test]
fn one_thread_reads_in_place_like_the_copied_overlap_and_can_switch_threads() {
    let world = slab(8, 4, 0.6);
    let mut flora = Flora::new(FloraConfig {
        step_every: 1,
        ..FloraConfig::default()
    });
    for (x, z) in [(1, 1), (3, 2), (6, 1)] {
        let wood = flora.config().species(Species::Springturf).wood_max * 0.5;
        assert!(flora.apply(
            &world,
            FloraCommand::Seed {
                x,
                z,
                species: Species::Springturf,
                wood,
            }
        ));
    }
    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 3,
            z: 2,
            founder: Founder::Browser,
            stores: StartingStores::HUNGRY,
            heading_rad: 0.0,
        }
    ));
    let id = fauna.view().animals[0].id;
    let teachers = fauna.config().teachers;
    let mut direct = Sim::ambient(world.clone(), flora.clone(), fauna.clone(), config(1, true));
    let mut copied = Sim::ambient(world, flora, fauna, config(2, true));
    // Controllers are intentionally omitted by Fauna::clone; attach one to each arm.
    for sim in [&mut direct, &mut copied] {
        sim.with_layers_mut(|_, _, fauna| {
            assert!(fauna.set_controller(
                id,
                cubarium_voxel_fauna::heuristic(Founder::Browser, teachers)
            ));
        });
    }
    for tick in 0..12 {
        if tick == 4 {
            direct.set_threads(2);
        }
        if tick == 8 {
            direct.set_threads(1);
        }
        direct.step();
        copied.step();
        assert_eq!(direct.world(), copied.world());
        assert_eq!(
            format!("{:?}", direct.flora().view().stands),
            format!("{:?}", copied.flora().view().stands)
        );
        assert_eq!(
            format!("{:?}", direct.fauna().view().animals),
            format!("{:?}", copied.fauna().view().animals)
        );
        assert_eq!(direct.flora().view().ledger, copied.flora().view().ledger);
        assert_eq!(direct.fauna().view().ledger, copied.fauna().view().ledger);
    }
    assert!(direct.flora().view().ledger.transpired_m3 > 0.0);
    assert!(direct.fauna().view().ledger.eaten_organic_in > 0.0);
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
                let want = pore
                    * cubarium_voxel::Materials::DEFAULT.pore_capacity(Material::Soil)
                    * w.config().voxel_volume();
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
                !taken.contains(s) && flora.view().establishment_gates(&world.view(), *s, species).passes()
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
    for site in skyline.iter().step_by(skyline.len() / 2).take(2) {
        fauna.apply(
            &world,
            FaunaCommand::IntroduceFounder {
                x: i64::from(site.x),
                z: site.z,
                founder: Founder::Browser,
                stores: StartingStores::FULL,
                heading_rad: 0.0,
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
                let drinkable = (v.pore_at(x, y, z)
                    - cubarium_voxel::Materials::DEFAULT.wilting_point(m))
                .max(0.0)
                    * cubarium_voxel::Materials::DEFAULT.pore_capacity(m)
                    * volume;
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
        // A plant step every tick: the subject is one tick's drink, not the plant clock.
        let mut flora = Flora::new(FloraConfig { step_every: 1, ..FloraConfig::default() });
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
        cubarium_voxel::water::step(&mut w, 1);
        fl.step_clocked(&mut w, 1);
        fa.step(&w, &mut fl);
        w.advance_tick();
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
/// the water leg makes in tick `t` drowns an animal (its lineage's `drown_after_s` set to
/// zero, so one tick over its drown depth is enough) in tick `t + 1` under overlap, and in
/// tick `t` itself on the chained tick.
#[test]
fn an_animal_sees_the_water_of_the_tick_before() {
    for (overlap, threads) in [(true, 1usize), (true, 2), (false, 1)] {
        let world = slab(6, 3, 0.4);
        let flora = Flora::new(FloraConfig::default());
        let mut fauna_config = FaunaConfig::default();
        fauna_config.founders[Founder::Browser.index()].drown_after_s = 0.0;
        let mut fauna = Fauna::new(fauna_config);
        assert!(fauna.apply(
            &world,
            FaunaCommand::IntroduceFounder {
                x: 3,
                z: 1,
                founder: Founder::Browser,
                stores: StartingStores::FULL,
                heading_rad: 0.0,
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

/// **The coarse clocks conserve water.** A rained-on world with plants and grazers, the
/// lateral water once every `water_every` ticks and the plants once every `step_every`,
/// chained and overlapped: the world's water closes, and every drop the plants drank is
/// the world's transpiration.
#[test]
fn coarse_water_and_the_plant_clock_conserve_water_chained_and_overlapped() {
    for overlap in [false, true] {
        let (world, mut flora, fauna) = rained_on();
        let mut fc = flora.config().clone();
        fc.step_every = 5;
        flora.replace_config(fc).unwrap();
        let mut sim = Sim::new(world, flora, fauna, SimConfig { threads: 2, overlap, water_every: 4 }, None);
        assert_eq!(sim.world().water_every(), 4, "Sim::new applies the coarse water");
        for _ in 0..203 {
            sim.step();
        }
        assert!(water_residual(sim.world()) <= 1e-12, "overlap {overlap}: residual {}", water_residual(sim.world()));
        let (w, f, _) = sim.layers();
        assert!(f.view().ledger.transpired_m3 > 0.0, "overlap {overlap}: nothing drank");
        assert!(
            rel(f.view().ledger.transpired_m3, w.view().ledger.transpiration_out) <= 1e-12,
            "overlap {overlap}: plants drank {}, the world booked {}",
            f.view().ledger.transpired_m3,
            w.view().ledger.transpiration_out
        );
    }
}

/// The modes' water: the display's `Default` leaves the world's own; an experiment takes a
/// coarse one, whatever number the preset holds.
#[test]
fn the_display_runs_live_water_and_an_experiment_runs_its_preset() {
    assert_eq!(SimConfig::default().water_every, 1);
    assert_eq!(SimConfig::experiment().water_every, cubarium_voxel_sim::EXPERIMENT_WATER_EVERY);
    let (world, flora, fauna) = rained_on();
    let sim = Sim::new(world, flora, fauna, SimConfig::default(), None);
    assert_eq!(sim.world().water_every(), 1);
}

// ------------------------------------------------- the plant tick (overlap package)
//
// `design/7_Research/tick-stage-dependencies-2026-10-04.md`, proposals 1 and 2: the plant
// step keeps the coarse water's phase, and on its tick the plants' reads run beside the
// animals'. Nothing here pins a tuned number: the clock is read off the config, the
// observation is compared with a twin's, and the ledgers with themselves.

/// A soil slab `width × depth` carpeted in springturf at `share` of its largest wood.
fn carpet(width: u32, depth: u32, share: f64) -> (World, Flora) {
    let world = slab(width, depth, 0.6);
    let mut flora = Flora::new(FloraConfig::default());
    let wood = share * flora.config().species(Species::Springturf).wood_max;
    for z in 0..depth {
        for x in 0..i64::from(width) {
            assert!(flora.apply(&world, FloraCommand::Seed { x, z, species: Species::Springturf, wood }));
        }
    }
    (world, flora)
}

/// A plant step happened in the tick between `before` and `after`: every living stand
/// pays its upkeep in a plant step, and nothing between steps respires.
fn plant_stepped(before: &Flora, after: &Flora) -> bool {
    after.view().ledger.respired_out != before.view().ledger.respired_out
}

/// **Plants step every `step_every` ticks on the water's phase, whatever `water_every`
/// is**: on exactly the world ticks that are multiples of `step_every` — the ticks a
/// `water_every` of `step_every` runs the coarse water on — chained and overlapped.
#[test]
fn plants_step_every_step_every_ticks_on_the_water_phase_whatever_water_every_is() {
    let n = FloraConfig::default().step_every.max(1);
    for water_every in [1, 2, n, n + 1] {
        for overlap in [false, true] {
            let (world, flora) = carpet(8, 4, 0.5);
            let fauna = Fauna::new(FaunaConfig::default());
            let mut sim = Sim::new(world, flora, fauna, SimConfig { threads: 2, overlap, water_every }, None);
            let mut steps = 0;
            for _ in 0..3 * u64::from(n) + 2 {
                let t = sim.world().tick();
                let water_due = sim.world().water_due();
                let before = sim.flora().clone();
                sim.step();
                let stepped = plant_stepped(&before, sim.flora());
                assert_eq!(
                    stepped,
                    t % u64::from(n) == 0,
                    "water_every {water_every}, overlap {overlap}: world tick {t} on a clock of {n}"
                );
                if stepped {
                    steps += 1;
                    if water_every == n {
                        assert!(water_due, "water_every {water_every}: the plant tick {t} is the coarse water's");
                    }
                }
            }
            assert!(steps >= 3, "water_every {water_every}, overlap {overlap}: {steps} plant steps");
        }
    }
}

/// **The overlapped plant tick conserves every ledger.** A springturf carpet grazed by
/// browsers on their own heuristics, the coarse water on the plant clock's phase, four
/// threads, the settled litter field: three hundred ticks, the plants' reads beside the
/// animals' on every plant tick. The world's water, both layers' organic matter, mineral
/// and energy close, the plants' drink is the world's transpiration, and what one layer
/// booked handing to the other is what the other booked receiving.
#[test]
fn the_overlapped_plant_tick_conserves_every_ledger() {
    let (world, flora) = carpet(16, 8, 0.5);
    // The guts void every few ticks, so dung lands on the plants' ground on plant ticks too.
    let mut fc = FaunaConfig::default();
    fc.life.gut_void_s = 10.0 * cubarium_voxel::DT;
    let mut fauna = Fauna::new(fc);
    cubarium_voxel_sim::habitat::install_heuristics(&mut fauna);
    let teachers = fauna.config().teachers;
    for (x, z) in [(3, 2), (8, 5), (12, 1), (13, 6)] {
        assert!(fauna.apply(
            &world,
            FaunaCommand::IntroduceFounder { x, z, founder: Founder::Browser, stores: StartingStores::HUNGRY, heading_rad: 0.0 },
        ));
        let id = fauna.view().ledger.births - 1;
        assert!(fauna.set_controller(id, cubarium_voxel_fauna::heuristic(Founder::Browser, teachers)));
    }
    let n = flora.config().step_every;
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig { threads: 4, overlap: true, water_every: n });
    for t in 0..300u64 {
        sim.step();
        let r = water_residual(sim.world());
        assert!(r <= 1e-9, "tick {t}: water residual {r:.3e}");
    }
    let (w, fl, fa) = sim.layers();
    let (fv, av) = (fl.view(), fa.view());
    for (name, got, want) in [
        ("flora organic", fv.organic(), fv.ledger.expected_organic()),
        ("flora mineral", fv.mineral(), fv.ledger.expected_mineral()),
        ("flora energy", fv.energy(), fv.ledger.expected_energy()),
        ("fauna organic", av.organic(), av.ledger.expected_organic()),
        ("fauna mineral", av.mineral(), av.ledger.expected_mineral()),
        ("fauna energy", av.energy(), av.ledger.expected_energy()),
    ] {
        assert!((got - want).abs() <= 1e-9 * got.abs().max(1.0), "{name}: {got} against {want}");
    }
    assert!(fv.ledger.transpired_m3 > 0.0, "nothing drank");
    assert!(
        rel(fv.ledger.transpired_m3, w.view().ledger.transpiration_out) <= 1e-12,
        "plants drank {}, the world booked {}",
        fv.ledger.transpired_m3,
        w.view().ledger.transpiration_out
    );
    assert!(av.ledger.eaten_organic_in > 0.0, "nothing ate");
    assert!(av.ledger.deposited_organic_out > 0.0, "nothing was voided");
    for (name, plants, animals) in [
        ("eaten", fv.ledger.consumed_organic_out, av.ledger.eaten_organic_in),
        ("deposited", fv.ledger.deposited_organic_in, av.ledger.deposited_organic_out),
    ] {
        assert!(
            (plants - animals).abs() <= 1e-9 * plants.abs().max(1.0),
            "{name}: the plants booked {plants}, the animals {animals}"
        );
    }
}

/// **On a plant tick the animals sense the plants as the last plant step left them.** A
/// soil slab carpeted in young springturf and one browser standing on it, its controller
/// a recorder that holds still. Every tick runs twice from the same layers: as built, and
/// as a twin whose plant clock never comes round. On a tick where the plants step and the
/// browser is sampled, its observation is the twin's, to the bit: the plants' light,
/// drink and growth ran beside the controller stage and changed nothing it read. The
/// browser is introduced after a few ticks of plants alone, so its sampling phase meets
/// the plant clock's.
#[test]
fn on_a_plant_tick_the_animals_sense_the_plants_the_last_step_left() {
    use cubarium_voxel_fauna::{Actions, Controller, Response};
    use std::sync::{Arc, Mutex};

    /// Records the packet it is handed and holds still.
    struct Recorder(Arc<Mutex<Vec<Vec<f64>>>>);
    impl Controller for Recorder {
        fn drive(&mut self, observation: &[f64]) -> Response {
            self.0.lock().expect("the sink").push(observation.to_vec());
            Response::Bounded(Actions::REST)
        }
        fn reset(&mut self) {}
    }

    let cfg = SimConfig { threads: 4, overlap: true, water_every: 1 };
    // One tick of `(world, flora, fauna)` with the plant clock at `every`: what the browser
    // was handed, whether the plants stepped, and the layers after.
    let tick = |world: &World, flora: &Flora, fauna: &Fauna, id: u64, every: u32| {
        let mut flora = flora.clone();
        let mut fc = flora.config().clone();
        fc.step_every = every;
        flora.replace_config(fc).expect("a clock");
        let sink = Arc::new(Mutex::new(Vec::new()));
        let mut sim = Sim::new(world.clone(), flora.clone(), fauna.clone(), cfg, None);
        assert!(sim.fauna_mut().set_controller(id, Box::new(Recorder(Arc::clone(&sink)))));
        sim.step();
        let stepped = plant_stepped(&flora, sim.flora());
        let seen = sink.lock().expect("the sink").pop();
        let (w, fl, fa) = sim.layers();
        (seen, stepped, (w.clone(), fl.clone(), fa.clone()))
    };

    let mut checked = 0;
    for lead in 0..8u64 {
        let (world, flora) = carpet(10, 6, 0.1);
        let every = flora.config().step_every;
        let mut sim = Sim::new(world, flora, Fauna::new(FaunaConfig::default()), cfg, None);
        for _ in 0..lead {
            sim.step();
        }
        let (world, flora, _) = sim.layers();
        let (mut world, mut flora) = (world.clone(), flora.clone());
        let mut fauna = Fauna::new(FaunaConfig::default());
        assert!(fauna.apply(
            &world,
            FaunaCommand::IntroduceFounder {
                x: 5,
                z: 3,
                founder: Founder::Browser,
                stores: StartingStores::FULL,
                heading_rad: 0.0,
            },
        ));
        let id = fauna.view().ledger.births - 1;
        for _ in 0..3 * u64::from(every) {
            let (seen, stepped, next) = tick(&world, &flora, &fauna, id, every);
            if stepped && let Some(seen) = seen {
                let (twin_seen, twin_stepped, _) = tick(&world, &flora, &fauna, id, u32::MAX);
                assert!(!twin_stepped, "the twin's clock never comes round");
                assert_eq!(
                    Some(seen),
                    twin_seen,
                    "lead {lead}, world tick {}: the browser read the plants after their step",
                    world.tick()
                );
                checked += 1;
            }
            (world, flora, fauna) = next;
        }
    }
    assert!(checked > 0, "no plant tick met a sampled browser");
}
