//! What the schedule has to satisfy: it is the three-call sequence, and the thread count
//! is not a rule.
//!
//! Short function tests only. Nothing here pins a hash, asserts a bit-identical world or
//! runs a long simulation: the claims are **conservation** (a residual that closes) and
//! **agreement** (two ways of running the same tick land in the same place to a stated
//! relative tolerance), which is the contract
//! `design/handoffs/voxel-schedule-brief-2026-09-18.md` sets.
//!
//! On the thread counts: `bevy_tasks`'s compute pool is process-global, so the first arm
//! in this process fixes how many worker threads exist. What the later arms still vary is
//! the **partitioning** — how many chunks the wet columns and the animals are cut into —
//! which is the only thing that could reassociate a sum. That is what is being tested.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{Command as FaunaCommand, Fauna, FaunaConfig, Species as Beast};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species};
use cubarium_voxel_sim::{Sim, SimConfig};

/// The bench's own conditioning rain, under the outlet's capacity.
const RAIN_M_PER_S: f64 = 0.0002;
/// Short: enough water in the world for the exchange to have wet columns to split.
const WARMUP_TICKS: u64 = 200;
const FOUNDERS_PER_SPECIES: usize = 4;
const GRAZERS: usize = 2;

/// A small world with water in it, six species of founder and two grazers: the same
/// condition three times over, so the only difference between arms is how the tick ran.
fn conditioned() -> (World, Flora, Fauna) {
    let dry = VoxelConfig {
        seed: 1,
        rain_m_per_s: RAIN_M_PER_S,
        ..VoxelConfig::default()
    };
    let floor = World::new(dry.clone())
        .outlet_cell()
        .map_or(0.0, |(_, y, _)| y as f64)
        * dry.voxel_m;
    let config = VoxelConfig {
        initial_aquifer_head_m: floor + 1.0,
        ..dry
    };
    let mut world = World::new(config);
    world.apply(WorldCommand::SetOutlet { open: true });
    for _ in 0..WARMUP_TICKS {
        world.step();
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
        for site in strided(&pool, FOUNDERS_PER_SPECIES) {
            if flora.apply(
                &world,
                FloraCommand::Seed {
                    x: i64::from(site.x),
                    z: site.z,
                    species,
                    wood,
                },
            ) {
                taken.push(site);
            }
        }
    }

    let mut fauna = Fauna::new(FaunaConfig::default());
    let pool: Vec<Site> = skyline
        .into_iter()
        .filter(|s| {
            world.view().material_at(i64::from(s.x), s.y, s.z) == Material::Soil
                && flora
                    .view()
                    .can_establish(&world.view(), *s, Species::Springturf)
        })
        .collect();
    let body = fauna.config().species(Beast::Frondgrazer).body_max;
    for site in strided(&pool, GRAZERS) {
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

fn skyline_of(world: &World) -> Vec<Site> {
    let (width, depth) = (world.config().width, world.config().depth);
    let mut out: Vec<Site> = Vec::new();
    for z in 0..depth {
        for x in 0..width as i64 {
            if let Some(site) = cubarium_voxel_flora::highest_support(&world.view(), x, z) {
                out.push(site);
            }
        }
    }
    out.sort_by_key(|s| (s.y, s.x, s.z));
    out
}

fn strided<T: Copy>(pool: &[T], want: usize) -> Vec<T> {
    if pool.is_empty() || want == 0 {
        return Vec::new();
    }
    let stride = (pool.len() / want).max(1);
    pool.iter().step_by(stride).take(want).copied().collect()
}

/// What a run is judged by: stored water, the aquifer, the three conservation residuals,
/// the populations, and the soil moisture each species actually reads.
#[derive(Clone, Debug)]
struct Reading {
    stored_m3: f64,
    head_m: f64,
    water_residual: f64,
    organic_residual: f64,
    mineral_residual: f64,
    stands: usize,
    animals: usize,
    /// Every animal's id and the face it stands on, in the layer's own id order: what
    /// `sense` decided, read directly. The one check that a split `sense` put the animals
    /// where an unsplit one would.
    animal_faces: Vec<(u64, u32, u32, u32)>,
    /// Their bodies, same order.
    animal_body: Vec<f64>,
    /// Mean pore fraction over each species' stands' root boxes, in `Species::ALL` order;
    /// `0.0` for a species with no stands left.
    root_pore: Vec<f64>,
}

fn read(world: &World, flora: &Flora, fauna: &Fauna) -> Reading {
    let v = world.view();
    let fv = flora.view();
    let mut root_pore = Vec::new();
    for species in Species::ALL {
        let mut sum = 0.0;
        let mut n = 0u32;
        for stand in fv.stands.iter().filter(|s| s.species == species) {
            let sc = fv.config.species(species);
            let r = i64::from(sc.rooting_radius);
            let depth = i64::from(sc.rooting_depth);
            for dz in -r..=r {
                let z = i64::from(stand.site.z) + dz;
                if z < 0 || z >= i64::from(v.config.depth) {
                    continue;
                }
                for dx in -r..=r {
                    for dy in 0..depth {
                        let y = i64::from(stand.site.y) - dy;
                        if y < 0 {
                            continue;
                        }
                        let (x, y, z) = (i64::from(stand.site.x) + dx, y as u32, z as u32);
                        if v.material_at(x, y, z) == Material::Soil {
                            sum += v.pore_at(x, y, z);
                            n += 1;
                        }
                    }
                }
            }
        }
        root_pore.push(if n == 0 { 0.0 } else { sum / f64::from(n) });
    }
    Reading {
        stored_m3: v.stored_m3(),
        head_m: world.aquifer_head_m(),
        water_residual: v.stored_m3() - v.ledger.expected_stored(),
        organic_residual: fv.organic() - fv.ledger.expected_organic(),
        mineral_residual: fv.mineral() - fv.ledger.expected_mineral(),
        stands: fv.stands.len(),
        animals: fauna.view().animals.len(),
        animal_faces: fauna
            .view()
            .animals
            .iter()
            .map(|a| (a.id, a.site.x, a.site.y, a.site.z))
            .collect(),
        animal_body: fauna.view().animals.iter().map(|a| a.body).collect(),
        root_pore,
    }
}

/// The three-call sequence, which is still what the core fixtures drive.
fn three_calls(ticks: u64) -> Reading {
    let (mut world, mut flora, mut fauna) = conditioned();
    for _ in 0..ticks {
        world.step();
        flora.step(&mut world);
        fauna.step(&world, &mut flora);
    }
    read(&world, &flora, &fauna)
}

/// The schedule, at a thread count.
fn schedule(ticks: u64, threads: usize) -> Reading {
    let (world, flora, fauna) = conditioned();
    let mut sim = Sim::new(world, flora, fauna, SimConfig { threads });
    for _ in 0..ticks {
        sim.step();
    }
    let (world, flora, fauna) = sim.layers();
    read(world, flora, fauna)
}

fn rel(a: f64, b: f64) -> f64 {
    let scale = a.abs().max(b.abs()).max(1e-12);
    (a - b).abs() / scale
}

fn agree(name: &str, a: &Reading, b: &Reading, tol: f64) {
    assert_eq!(a.stands, b.stands, "{name}: stand count");
    assert_eq!(a.animals, b.animals, "{name}: animal count");
    assert_eq!(
        a.animal_faces, b.animal_faces,
        "{name}: the faces the animals ended up on"
    );
    for (i, (x, y)) in a.animal_body.iter().zip(b.animal_body.iter()).enumerate() {
        assert!(
            rel(*x, *y) <= tol,
            "{name}: animal {i} body {x} against {y}"
        );
    }
    assert!(
        rel(a.stored_m3, b.stored_m3) <= tol,
        "{name}: stored water {} against {} ({:.3e} relative, tolerance {tol:.0e})",
        a.stored_m3,
        b.stored_m3,
        rel(a.stored_m3, b.stored_m3)
    );
    assert!(
        rel(a.head_m, b.head_m) <= tol,
        "{name}: aquifer head {} against {}",
        a.head_m,
        b.head_m
    );
    for (i, species) in Species::ALL.into_iter().enumerate() {
        assert!(
            rel(a.root_pore[i], b.root_pore[i]) <= tol,
            "{name}: {} mean root-box pore {} against {} ({:.3e} relative)",
            species.name(),
            a.root_pore[i],
            b.root_pore[i],
            rel(a.root_pore[i], b.root_pore[i])
        );
    }
}

/// Every residual the three layers keep closes at the tolerances they were already judged
/// at, whatever the thread count. Conservation is the contract; a hash is not.
fn conserves(name: &str, r: &Reading) {
    assert!(
        r.water_residual.abs() / r.stored_m3.max(1.0) < 1e-9,
        "{name}: water residual {:.3e} m3 on {:.3} m3 stored",
        r.water_residual,
        r.stored_m3
    );
    assert!(
        r.organic_residual.abs() < 1e-9,
        "{name}: flora organic residual {:.3e}",
        r.organic_residual
    );
    assert!(
        r.mineral_residual.abs() < 1e-9,
        "{name}: flora mineral residual {:.3e}",
        r.mineral_residual
    );
}

/// The schedule is the three-call sequence. Not asserted bit for bit — the claim is that
/// the two agree to well inside anything the ecology reads.
#[test]
fn the_schedule_is_the_three_call_sequence() {
    let expected = three_calls(60);
    let got = schedule(60, 1);
    conserves("three calls", &expected);
    conserves("schedule, 1 thread", &got);
    agree("schedule against three calls", &expected, &got, 1e-12);
}

/// The brief's own check: stored water and per-species mean root-box pore after 200
/// coupled ticks agree across thread counts within 1e-6 relative, and every residual
/// still closes. There is no reduction inside either parallel phase — a column's scan and
/// an animal's plan are independent, and nothing is summed across chunks — so the
/// agreement observed is exact; the tolerance is what is *asserted*, because a future
/// parallel phase with a fold in it may only manage that much.
#[test]
fn the_thread_count_does_not_move_the_soil() {
    let one = schedule(200, 1);
    conserves("1 thread", &one);
    for threads in [4usize, 16] {
        let got = schedule(200, threads);
        conserves(&format!("{threads} threads"), &got);
        agree(&format!("{threads} threads against 1"), &one, &got, 1e-6);
    }
}

/// The tick counter moves exactly once per scheduled tick, and the `Advance` phase is
/// what moves it: a world stepped by the schedule reads the same tick a world stepped by
/// hand does.
#[test]
fn the_schedule_advances_the_clock_once_per_tick() {
    let mut sim = Sim::from_configs(
        VoxelConfig::default(),
        FloraConfig::default(),
        FaunaConfig::default(),
        SimConfig { threads: 1 },
    );
    assert_eq!(sim.world().tick(), 0);
    for expected in 1..=5u64 {
        sim.step();
        assert_eq!(sim.world().tick(), expected);
        assert_eq!(sim.flora().tick(), expected);
        assert_eq!(sim.fauna().tick(), expected);
    }
}

/// A caller can still reach all three layers mutably at once, which is what the command
/// plumbing needs: seeding a stand reads the world it stands in.
#[test]
fn a_caller_can_hold_all_three_layers_at_once() {
    let mut sim = Sim::from_configs(
        VoxelConfig::default(),
        FloraConfig::default(),
        FaunaConfig::default(),
        SimConfig { threads: 1 },
    );
    let seeded = sim.with_layers_mut(|world, flora, _fauna| {
        // `Command::Seed` is the declared founder and does not ask the establishment
        // gates, so any support face will do: what is under test is the access, not the
        // ecology.
        let Some(site) = skyline_of(world).into_iter().next() else {
            return false;
        };
        let wood = 0.5 * flora.config().species(Species::Springturf).wood_max;
        flora.apply(
            world,
            FloraCommand::Seed {
                x: i64::from(site.x),
                z: site.z,
                species: Species::Springturf,
                wood,
            },
        )
    });
    assert!(seeded, "a declared founder lands on the first skyline face");
    assert_eq!(sim.flora().view().stands.len(), 1);
    sim.step();
    assert_eq!(sim.world().tick(), 1);
}

/// A caller's sampler runs once per tick, in [`cubarium_voxel_sim::TickPhase::Sample`],
/// after every layer has stepped: what it reads is the tick that just finished.
#[test]
fn a_sampler_runs_once_per_tick_after_the_layers() {
    use bevy_ecs::prelude::{Res, ResMut, Resource};
    use cubarium_voxel_sim::VoxelWorld;

    #[derive(Resource, Default)]
    struct Seen(Vec<u64>);

    fn observe(w: Res<VoxelWorld>, mut seen: ResMut<Seen>) {
        seen.0.push(w.0.tick());
    }

    let mut sim = Sim::from_configs(
        VoxelConfig::default(),
        FloraConfig::default(),
        FaunaConfig::default(),
        SimConfig { threads: 1 },
    );
    sim.ecs().insert_resource(Seen::default());
    sim.add_samplers(observe);
    for _ in 0..3 {
        sim.step();
    }
    assert_eq!(sim.ecs().resource::<Seen>().0, vec![1, 2, 3]);
}
