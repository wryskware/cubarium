//! **NECTAR-SCENT** (2026-10-08): a flower's nectar scent is a plume in the air around its
//! bloom, not a deposit on the soil under the plant. Hungry bellwings awake in the dark
//! followed the old ground scent to its peak at a bloomcrown's rooting face and circled
//! there, a metre off the flower, all night. A child of `step`, sharing the predation
//! fixture.
//!
//! Every case builds its scene by hand and checks a mechanism; no default, ratio or
//! population is asserted.

use super::predation_fixture::*;
use crate::light::BeaconKind;
use crate::scent::{Plume, nectar_plume, nectar_sources};
use crate::{Activity, Command, Fauna, FaunaConfig, Founder, MobilityMode, Pose, StartingStores};
use cubarium_voxel::World;
use cubarium_voxel::weather::DayClock;
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species};

const BW: Founder = Founder::Bellwing;

/// A bloomcrown in flower on a plain 24 rows tall at `hour`, its nectar on: `(world, flora,
/// the bloom (x, y, z) as the light layer's beacon has it)`.
fn flower_scene(hour: f64, seed: u64) -> (World, Flora, (f64, f64, f64)) {
    let mut world = plain_of_height(DayClock { start_hour: hour, ..DayClock::default() }, seed, 24);
    let mut fc = FloraConfig::default();
    fc.bloomcrown.nectar_per_s = 1e-3;
    fc.bloomcrown.nectar_cap = 2e-3;
    let mut flora = Flora::new(fc);
    let wood = flora.config().species(Species::Bloomcrown).wood_max;
    assert!(flora.apply(&world, FloraCommand::Seed { x: 24, z: 6, species: Species::Bloomcrown, wood }));
    for _ in 0..60 {
        flora.step(&mut world);
    }
    assert!(flora.view().stands.iter().any(|s| s.nectar > 0.0), "fixture: the flower holds nectar");
    let mut fauna = Fauna::new(FaunaConfig::default());
    fauna.beacons.refresh(&world.view(), &flora.view(), fauna.config.scent.nectar_full);
    let b = *fauna.beacons.list().iter().find(|b| b.kind == BeaconKind::Flower).expect("fixture: the flower is a beacon");
    assert!(b.y_m > SURFACE_M + 0.5, "fixture: the flower head stands over the ground: {} vs {SURFACE_M}", b.y_m);
    (world, flora, (b.x, b.y_m, b.z))
}

/// The plume at `p` from every source in the scene.
fn smell_at(world: &World, flora: &Flora, p: (f64, f64, f64)) -> Plume {
    let mut fauna = Fauna::new(FaunaConfig::default());
    let (view, fv) = (world.view(), flora.view());
    fauna.beacons.refresh(&view, &fv, fauna.config.scent.nectar_full);
    let sources = nectar_sources(&fauna, &view, None);
    nectar_plume(&view, &fauna.config, &sources, p, 0, 0.0)
}

/// **The plume falls with distance from the bloom in 3D, not from the root**: at the flower
/// head it is stronger than on the ground under the plant; at the bloom's height it falls
/// with distance from it; a metre off, it is strongest at the bloom's height (weaker below
/// and above); and its gradient there points at the bloom — toward it level with it, and
/// up from under it.
#[test]
fn the_nectar_plume_falls_with_3d_distance_from_the_bloom_not_from_the_root() {
    let (world, flora, (bx, by, bz)) = flower_scene(12.0, 801);
    let at = |dx: f64, dy: f64| smell_at(&world, &flora, (bx + dx, by + dy, bz));
    let root = smell_at(&world, &flora, (bx, SURFACE_M + 0.05, bz));
    let bloom = at(0.0, 0.0);
    assert!(bloom.level > root.level, "stronger at the flower head than on the soil under it: {bloom:?} vs {root:?}");
    let off = [0.5, 1.0, 2.0, 3.0].map(|d| at(d, 0.0).level);
    assert!(off.windows(2).all(|w| w[0] > w[1]) && bloom.level > off[0], "falls with distance at its height: {} {off:?}", bloom.level);
    let side = at(1.0, 0.0);
    let (below, above) = (at(1.0, -(by - SURFACE_M - 0.05)), at(1.0, 1.0));
    assert!(side.level > below.level && side.level > above.level, "a metre off, strongest at its height: {side:?} {below:?} {above:?}");
    assert!(side.gx < 0.0 && side.gx.abs() > 10.0 * side.gy.abs(), "level with it, the gradient points at it: {side:?}");
    let under = at(0.5, -0.5);
    assert!(under.gy > 0.0 && under.gx < 0.0, "below and beside it, up and toward it: {under:?}");
    assert_eq!(side.source, Some((bx, by, bz)), "its source is the bloom");
}

/// **No scent through rock**: a wall of soil between the smeller and the flower, as tall
/// as the band it smells in, and the plume is not smelled; the same place with no wall
/// smells it.
#[test]
fn the_plume_does_not_pass_through_rock() {
    let (mut world, flora, (bx, by, bz)) = flower_scene(12.0, 802);
    let p = (bx - 2.0, by, bz);
    let open = smell_at(&world, &flora, p);
    assert!(open.level > 0.0, "fixture: smelled in the open");
    let wx = ((bx - 1.0) / VOXEL_M).floor() as i64;
    for z in 0..world.config().depth {
        for y in SURFACE_Y + 1..world.config().height {
            world.apply(cubarium_voxel::Command::SetMaterial { x: wx, y, z, material: cubarium_voxel::Material::Soil });
        }
    }
    let walled = smell_at(&world, &flora, p);
    assert_eq!(walled.level, 0.0, "behind a wall: {walled:?}");
}

/// **A hungry flyer at the old scent peak finds the bloom in the dark**: a bellwing on its
/// own heuristic, awake at midnight (the flower a dark near-IR reflector: no light to approach),
/// hovering at its search height a metre and a quarter off a bloomcrown in flower and facing
/// away from it — where the ground scent held bellwings circling. Led by the plume, it sips
/// within half a minute.
#[test]
fn a_hungry_flyer_at_the_scent_peak_finds_the_bloom_in_the_dark() {
    let (world, mut flora, (bx, _, bz)) = flower_scene(0.0, 803);
    let mut c = FaunaConfig::default();
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c.scent.nectar_full = flora.config().bloomcrown.nectar_cap;
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let cruise = fauna.config.teachers.bellwing_cruise_altitude * fauna.config.founder(BW).flight.expect("a flyer").band_max_m;
    assert!(fauna.apply(
        &world,
        Command::IntroduceAloft { pose: Pose { x: bx - 1.25, z: bz, heading_rad: -std::f64::consts::FRAC_PI_2, y_m: SURFACE_M + cruise }, founder: BW, stores: StartingStores::FULL },
    ));
    let bw = fauna.view().ledger.births - 1;
    animal_mut(&mut fauna, bw).eating = true;
    assert_eq!(animal(&fauna, bw).mobility.mode, MobilityMode::Fly, "fixture: aloft");
    let teachers = fauna.config.teachers;
    fauna.controllers.set(bw, crate::controller::heuristic(BW, teachers));
    let sips0: u64 = fauna.ledger.sips_by_bloom.iter().sum();
    let sipped = step_until(&mut fauna, &world, &mut flora, cubarium_voxel::ticks_at_least_one(30.0) as u32, |f| {
        f.ledger.sips_by_bloom.iter().sum::<u64>() > sips0
    });
    let led = fauna.ledger.scent_led_samples[BW.index()];
    let l = fauna.percept(bw).map(|p| p.light);
    assert!(sipped.is_some(), "no sip in 30 s ({led} scent-led samples, light {l:?}, at {:?})", animal(&fauna, bw).pose);
    assert!(led > 0, "fixture: the scent led it");
}

/// **A flower smells by the nectar it holds**: two flowers the same distance either side of
/// the smeller, one holding a full bloomcrown head's nectar and one a tenth of it: the plume
/// leads to the full one, and each alone gives off in proportion to what it holds.
#[test]
fn a_flower_smells_by_the_nectar_it_holds() {
    let world = clock_off(804);
    let flora = Flora::new(FloraConfig::default());
    let (view, fv) = (world.view(), flora.view());
    let flower = |x: f64| crate::light::Beacon { kind: BeaconKind::Flower, x, z: LANE_Z, y_m: SURFACE_M + 0.5, sky: 1.0, power: 1.0, moving: false, body: 0 };
    let p = (6.0, SURFACE_M + 0.5, LANE_Z);
    let smell = |list: Vec<(crate::light::Beacon, f64)>| {
        let mut fauna = Fauna::new(FaunaConfig::default());
        fauna.beacons.fixed_with_scent(&view, &fv, list);
        let sources = nectar_sources(&fauna, &view, None);
        nectar_plume(&view, &fauna.config, &sources, p, 0, 0.0)
    };
    let both = smell(vec![(flower(4.5), 1.0), (flower(7.5), 0.1)]);
    assert!(both.gx < 0.0 && both.source == Some((4.5, SURFACE_M + 0.5, LANE_Z)), "led to the full flower: {both:?}");
    let (full, tenth) = (smell(vec![(flower(4.5), 1.0)]).level, smell(vec![(flower(4.5), 0.1)]).level);
    assert!(full > 0.0 && (tenth / full - 0.1).abs() < 1e-9, "in proportion to its nectar: {tenth} of {full}");
}

/// The fauna reference controls emission independently of the plant's capacity.
#[test]
fn nectar_full_scales_flower_scent_and_nonpositive_uses_the_full_rate() {
    let (world, flora, _) = flower_scene(12.0, 805);
    let (view, fv) = (world.view(), flora.view());
    let held = fv.stands.iter().find(|s| s.nectar > 0.0).unwrap().nectar;
    let share = |full| {
        let (_, shares) = crate::light::Beacons::build(&view, &fv, full);
        shares[0]
    };
    assert!((share(held) - 1.0).abs() < 1e-9);
    assert!((share(held * 4.0) - 0.25).abs() < 1e-9);
    assert_eq!(share(0.0), 1.0);
    assert_eq!(share(-held), 1.0);
}

/// The shared tree gives the complete reader's result with different bloom heights,
/// rates, wrap distances, and floor cuts, including a floor above the top-six sum.
#[test]
fn indexed_plume_matches_complete_sources() {
    let mut world = plain_of_height(DayClock::default(), 806, 24);
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    fauna.config.scent.threshold = 0.0;
    let ring = f64::from(world.config().width) * world.config().voxel_m;
    let flowers: Vec<_> = (0..96)
        .map(|i| {
            (
                crate::light::Beacon {
                    kind: if i % 11 == 0 {
                        BeaconKind::Glowcap
                    } else {
                        BeaconKind::Flower
                    },
                    x: ((i * 37 % 96) as f64 + 0.5) / 96.0 * ring,
                    y_m: SURFACE_M + 0.5 + (i % 4) as f64 * 0.25,
                    z: LANE_Z + (i % 5) as f64 * 0.15,
                    sky: 1.0,
                    power: 1.0,
                    moving: false,
                    body: 0,
                },
                if i % 7 == 0 {
                    100.0
                } else {
                    0.1 + (i % 3) as f64
                },
            )
        })
        .collect();
    for wall in [false, true] {
        if wall {
            for y in SURFACE_Y + 1..world.config().height {
                for z in 0..world.config().depth {
                    world.apply(cubarium_voxel::Command::SetMaterial {
                        x: 1,
                        y,
                        z,
                        material: cubarium_voxel::Material::Soil,
                    });
                }
            }
        }
        let (view, fv) = (world.view(), flora.view());
        fauna.beacons.fixed_with_scent(&view, &fv, flowers.clone());
        let sources = nectar_sources(&fauna, &view, None);
        for x in [0.01, ring - 0.01, ring * 0.4] {
            for y in [SURFACE_M + 0.5, SURFACE_M + 1.5] {
                let p = (x, y, LANE_Z);
                let full = nectar_plume(&view, &fauna.config, &sources, p, 0, 0.0);
                for floor in [0.0, full.level * 1.5, full.level * 100.0] {
                    let expected = nectar_plume(&view, &fauna.config, &sources, p, 0, floor);
                    let actual = crate::scent::nectar_plume_indexed(&fauna, &view, p, 0, floor);
                    assert_eq!(actual.source, expected.source);
                    for (a, e) in [
                        (actual.level, expected.level),
                        (actual.gx, expected.gx),
                        (actual.gy, expected.gy),
                        (actual.gz, expected.gz),
                    ] {
                        assert!(
                            (a - e).abs() <= 1e-10 * e.abs().max(1.0),
                            "indexed {actual:?}, complete {expected:?}"
                        );
                    }
                }
            }
        }
    }
}

/// Contact candidates include the crown extent, wrap across the seam, and retain
/// the complete geometry query's answer in an unindexed hand-built view.
#[test]
fn indexed_bloom_contact_matches_uncached_geometry() {
    let world = plain_of_height(DayClock::default(), 807, 24);
    let mut fc = FloraConfig::default();
    fc.bloomcrown.nectar_per_s = 1e-3;
    fc.bloomcrown.nectar_cap = 2e-3;
    let mut flora = Flora::new(fc);
    let wood = flora.config().species(Species::Bloomcrown).wood_max;
    for x in [0, 8, world.config().width - 1] {
        assert!(flora.apply(
            &world,
            FloraCommand::Seed {
                x: i64::from(x),
                z: 6,
                species: Species::Bloomcrown,
                wood
            }
        ));
    }
    let mut world = world;
    for _ in 0..60 {
        flora.step(&mut world);
    }
    let (view, fv) = (world.view(), flora.view());
    let empty = cubarium_voxel_flora::CrownCache::none();
    let uncached = cubarium_voxel_flora::FloraView {
        crowns: &empty,
        ..flora.view()
    };
    assert!(
        fv.stands_near(0, 6, view.config.width, view.config.depth)
            .is_some()
    );
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let (beacons, _) = crate::light::Beacons::build(&view, &fv, 1.0);
    let height = beacons.iter().find(|b| b.kind == BeaconKind::Flower).unwrap().y_m;
    let mut contacts = 0;
    for x in [0.01, ring - 0.01, 8.5 * v, 10.1 * v, 20.5 * v] {
        for y in [SURFACE_M, height - 0.05, height, height + v] {
            for reach in [0.0, 0.05, 0.4] {
                let p = (x, y, 6.5 * v);
                contacts += usize::from(crate::flight::bloom_in_reach(&view, &fv, p, reach).is_some());
                assert_eq!(
                    crate::flight::bloom_in_reach(&view, &fv, p, reach),
                    crate::flight::bloom_in_reach(&view, &uncached, p, reach)
                );
            }
        }
    }
    assert!(contacts > 0, "fixture: some tested points touch blooms");
}

/// Isolate query cost on one fixed set of flower positions/rates; this is an
/// explicitly requested performance probe, never a threshold on tuned ecology.
#[test]
#[ignore = "study: run by name"]
fn nectar_query_throughput() {
    use std::time::Instant;
    let world = World::new(cubarium_voxel::Config {
        width: 640, depth: 128, height: 32, voxel_m: 0.25,
        ..cubarium_voxel::Config::default()
    });
    let flora = Flora::new(FloraConfig::default());
    let (view, fv) = (world.view(), flora.view());
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let depth = f64::from(view.config.depth) * view.config.voxel_m;
    let positions: Vec<_> = (0..32).map(|i| (
        (i as f64 * 29.731).rem_euclid(ring),
        2.0 + (i % 5) as f64 * 0.6,
        (i as f64 * 5.37).rem_euclid(depth),
    )).collect();
    for count in [4096, 32768] {
        let mut fauna = Fauna::new(FaunaConfig::default());
        let flowers: Vec<_> = (0..count).map(|i| (crate::light::Beacon {
            kind: BeaconKind::Flower,
            x: (i as f64 * 31.731).rem_euclid(ring),
            z: (i as f64 * 7.171).rem_euclid(depth),
            y_m: 2.0 + (i % 5) as f64 * 0.5,
            sky: 1.0, power: 1.0, moving: false, body: 0,
        }, (1 + i % 7) as f64 / 7.0)).collect();
        for (label, share, floor) in [("normal", 1.0, 0.0), ("under-floor", 1.0, 1e8), ("weak", 1e-9, 1.0)] {
            let started = Instant::now();
            fauna.beacons.fixed_with_scent(&view, &fv, flowers.iter().map(|&(b, q)| (b, q * share)).collect());
            let build = started.elapsed();
            let sources = nectar_sources(&fauna, &view, None);
            for &p in &positions {
                let old = nectar_plume(&view, &fauna.config, &sources, p, 0, floor);
                let new = crate::scent::nectar_plume_indexed(&fauna, &view, p, 0, floor);
                assert_eq!(new.source, old.source);
                assert!((new.level - old.level).abs() < 1e-10 * old.level.max(1.0));
            }
            let started = Instant::now();
            for _ in 0..4 {
                for &p in &positions {
                    std::hint::black_box(nectar_plume(&view, &fauna.config, &sources, p, 0, floor));
                }
            }
            let complete = started.elapsed();
            let started = Instant::now();
            for _ in 0..4 {
                for &p in &positions {
                    std::hint::black_box(crate::scent::nectar_plume_indexed(&fauna, &view, p, 0, floor));
                }
            }
            let indexed = started.elapsed();
            eprintln!("nectar {count} {label}: build {:.3}ms; complete {:.3}ms; indexed {:.3}ms; {:.2}x (128 queries)",
                build.as_secs_f64() * 1000.0, complete.as_secs_f64() * 1000.0,
                indexed.as_secs_f64() * 1000.0, complete.as_secs_f64() / indexed.as_secs_f64());
        }
    }
}
