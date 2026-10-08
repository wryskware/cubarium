//! LOAD-CLIMB (2026-10-08): a body caught mid-climb by a save keeps climbing after the one
//! safe load ([`crate::saved::decode`]) on its reinstalled heuristic, as the same world
//! continuing unsaved does. Hand-built fixtures, a few hundred ticks; no tuned numbers.

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config, Material, World};
use cubarium_voxel_fauna::{Command as FaunaCommand, Fauna, FaunaConfig, Founder, MobilityMode, StartingStores, State};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species as Plant};

use crate::habitat;

const V: f64 = 0.25;
/// Facing `+x`.
const EAST: f64 = std::f64::consts::FRAC_PI_2;

/// A soil plain (`y = 1..=2`, at field capacity), day clock off.
fn plain(width: u32, height: u32, depth: u32) -> World {
    let mut w = World::empty(Config {
        width,
        height,
        depth,
        voxel_m: V,
        seed: 7,
        day: DayClock {
            mode: DayMode::Off,
            ..DayClock::default()
        },
        ..Config::default()
    });
    let m = cubarium_voxel::Materials::DEFAULT;
    let water = m.field_capacity(Material::Soil) * m.pore_capacity(Material::Soil) * w.config().voxel_volume();
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=2 {
                w.apply(WorldCommand::AddWater { x, y, z, volume_m3: water });
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

/// Every founder body driven by its lineage's heuristic, as a founding seeds them.
fn drive(fauna: &mut Fauna) {
    habitat::reinstall_heuristics(fauna);
}

/// The one safe load, in memory: the three layers through their save bytes.
fn round_trip(world: &World, flora: &Flora, fauna: &Fauna) -> (World, Flora, Fauna) {
    crate::saved::decode(&world.save(), &cubarium_voxel_flora::snapshot::encode(flora), &fauna.save(), &[])
        .expect("the save loads")
}

fn body(fauna: &Fauna, id: u64) -> cubarium_voxel_fauna::Animal {
    *fauna.view().animal(id).expect("alive")
}

/// Step until `done`, at most `cap` ticks; whether it got there.
fn until(world: &World, flora: &mut Flora, fauna: &mut Fauna, cap: usize, done: impl Fn(&Fauna) -> bool) -> bool {
    for _ in 0..cap {
        if done(fauna) {
            return true;
        }
        fauna.step(world, flora);
    }
    done(fauna)
}

/// **A seedporter saved halfway up a stem finishes the climb after a load.** A seedporter on
/// its own heuristic walks east into a full-grown lanternberry with no fruit (nothing to stop
/// for) and starts up the stem; caught a few ticks into the climb, the world is saved and
/// loaded. The loaded body keeps going up and leaves the climb (perched in the crown, or
/// off it) within a few hundred ticks, as the unsaved twin does.
#[test]
fn a_seedporter_saved_mid_climb_keeps_climbing_after_a_load() {
    let world = plain(48, 24, 8);
    let mut fc = FloraConfig::default();
    fc.lanternberry.fruit_per_s = 0.0;
    fc.lanternberry.propagule_rate = 0.0;
    let mut flora = Flora::new(fc);
    let wood = flora.config().lanternberry.wood_max;
    assert!(flora.apply(&world, FloraCommand::Seed { x: 12, z: 4, species: Plant::Lanternberry, wood }));
    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 9,
            z: 4,
            founder: Founder::Seedporter,
            stores: StartingStores::FULL,
            heading_rad: EAST,
        },
    ));
    let id = fauna.view().ledger.births - 1;
    drive(&mut fauna);
    let climbing = |f: &Fauna| body(f, id).mobility.mode == MobilityMode::Climb;
    assert!(until(&world, &mut flora, &mut fauna, 400, climbing), "fixture: it walks into the stem and climbs");
    for _ in 0..3 {
        fauna.step(&world, &mut flora);
    }
    assert!(climbing(&fauna), "fixture: still on the stem when saved");
    let saved_y = body(&fauna, id).pose.y_m;

    let (lworld, mut lflora, mut loaded) = round_trip(&world, &flora, &fauna);
    let left = |f: &Fauna| body(f, id).mobility.mode != MobilityMode::Climb;
    let mut climbed = false;
    for _ in 0..400 {
        loaded.step(&lworld, &mut lflora);
        let a = body(&loaded, id);
        climbed |= a.pose.y_m > saved_y;
        if left(&loaded) {
            break;
        }
    }
    assert!(climbed, "the loaded body went on up the stem from {saved_y}");
    assert!(left(&loaded), "the loaded body finished its climb: {:?}", body(&loaded, id).mobility);
    assert!(until(&world, &mut flora, &mut fauna, 400, left), "the unsaved twin finishes its climb too");
}

/// **A body that holds on without going anywhere is not climbing.** A seedporter on a stem
/// whose driver holds no forward effort stays where it is; its state is `Resting`, not
/// `Climbing` (the staged liveness check and the vitals read `Climbing` as moving), while its
/// mode stays `Climb`.
#[test]
fn a_seedporter_holding_still_on_a_stem_is_resting_not_climbing() {
    let world = plain(48, 24, 8);
    let mut fc = FloraConfig::default();
    fc.lanternberry.fruit_per_s = 0.0;
    let mut flora = Flora::new(fc);
    let wood = flora.config().lanternberry.wood_max;
    assert!(flora.apply(&world, FloraCommand::Seed { x: 12, z: 4, species: Plant::Lanternberry, wood }));
    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 9,
            z: 4,
            founder: Founder::Seedporter,
            stores: StartingStores::FULL,
            heading_rad: EAST,
        },
    ));
    let id = fauna.view().ledger.births - 1;
    let go = cubarium_voxel_fauna::Actions { forward: 1.0, turn: 0.0, feed: 0.0 };
    assert!(fauna.set_controller(id, Box::new(cubarium_voxel_fauna::Scripted::new(vec![go]))));
    let climbing = |f: &Fauna| body(f, id).mobility.mode == MobilityMode::Climb;
    assert!(until(&world, &mut flora, &mut fauna, 400, climbing), "fixture: it climbs");
    fauna.step(&world, &mut flora);
    assert_eq!(body(&fauna, id).state, State::Climbing, "going up the stem is climbing");
    let stop = cubarium_voxel_fauna::Actions { forward: 0.0, turn: 0.0, feed: 0.0 };
    assert!(fauna.set_controller(id, Box::new(cubarium_voxel_fauna::Scripted::new(vec![stop]))));
    // Its held action changes at its next controller sample.
    let period = Founder::Seedporter.manifest().cadence_ticks() as usize;
    for _ in 0..2 * period {
        fauna.step(&world, &mut flora);
    }
    let y = body(&fauna, id).pose.y_m;
    for _ in 0..period {
        fauna.step(&world, &mut flora);
        let a = body(&fauna, id);
        assert_eq!(a.mobility.mode, MobilityMode::Climb, "still on the stem");
        assert_eq!(a.pose.y_m, y, "it holds where it is");
        assert_eq!(a.state, State::Resting, "holding still on a stem is resting");
    }
}

/// **A wall climber saved on a face finishes the face after a load.** A littershredder and a
/// capgnawer walk east into a rock wall a metre high and go onto it; caught on the face, the
/// world is saved and loaded, and every body is on its own heuristic. Each loaded body comes
/// off the face (at the top, or back down) within a few hundred ticks, as the unsaved twin
/// on the same heuristic does.
#[test]
fn a_wall_climber_saved_on_a_face_comes_off_it_after_a_load() {
    for founder in [Founder::Blind, Founder::Capgnawer] {
        let mut world = plain(48, 24, 8);
        for z in 0..8 {
            for x in 14..=40 {
                for y in 3..=6 {
                    world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Rock });
                }
            }
        }
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::default());
        assert!(fauna.apply(
            &world,
            FaunaCommand::IntroduceFounder { x: 10, z: 4, founder, stores: StartingStores::FULL, heading_rad: EAST },
        ));
        let id = fauna.view().ledger.births - 1;
        assert!(founder.manifest().climbs_walls, "fixture: {founder:?} climbs walls");
        let go = cubarium_voxel_fauna::Actions { forward: 1.0, turn: 0.0, feed: 0.0 };
        assert!(fauna.set_controller(id, Box::new(cubarium_voxel_fauna::Scripted::new(vec![go]))));
        let on_face = |f: &Fauna| body(f, id).mobility.climb.is_some();
        assert!(until(&world, &mut flora, &mut fauna, 600, on_face), "fixture: {founder:?} goes onto the wall");
        fauna.step(&world, &mut flora);
        assert!(on_face(&fauna), "fixture: {founder:?} still on the face when saved");

        let (lworld, mut lflora, mut loaded) = round_trip(&world, &flora, &fauna);
        drive(&mut fauna);
        let off = |f: &Fauna| body(f, id).mobility.climb.is_none();
        assert!(until(&lworld, &mut lflora, &mut loaded, 600, off), "loaded {founder:?} still on the face: {:?}", body(&loaded, id));
        assert!(until(&world, &mut flora, &mut fauna, 600, off), "unsaved {founder:?} still on the face");
    }
}

/// **The one-version legacy reader on a real save** (SAVE-EXTENSIBLE): a checkpoint of the
/// 2026-10-08 training starts (world 22, flora 38, fauna 71) loads through the one safe load,
/// every animal driven, and [`crate::saved::resave`] rewrites it in the current format with
/// nothing changed. Needs the files:
/// `CUBARIUM_LEGACY_SAVE=runs/starts3-2026-10-08/heuristic/s4/run/ckpt/m000120 cargo test
/// --release -p cubarium-voxel-sim -- --ignored a_real_legacy_save`.
#[test]
#[ignore = "needs a real schema-71 checkpoint: set CUBARIUM_LEGACY_SAVE"]
fn a_real_legacy_save_loads_and_resaves() {
    let dir = std::path::PathBuf::from(
        std::env::var("CUBARIUM_LEGACY_SAVE").expect("CUBARIUM_LEGACY_SAVE names a checkpoint directory"),
    );
    let fauna_bytes = std::fs::read(dir.join("fauna.bin")).expect("fauna.bin");
    assert_eq!(
        cubarium_voxel::snapshot::schema_tag(&fauna_bytes),
        Some(cubarium_voxel_fauna::LEGACY_SCHEMA),
        "the checkpoint is a legacy one"
    );
    let old = crate::saved::load(&dir, &[]).expect("the legacy checkpoint loads");
    assert!(!old.fauna.view().animals.is_empty(), "a mature start has animals");
    crate::saved::driven(&old.fauna).expect("every animal is driven");

    let copy = std::env::temp_dir().join(format!("cubarium-resave-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&copy);
    std::fs::create_dir_all(&copy).unwrap();
    for f in ["world.bin", "flora.bin", "fauna.bin", "minute"] {
        std::fs::copy(dir.join(f), copy.join(f)).unwrap();
    }
    let done = crate::saved::resave(&copy).expect("resaves");
    assert_eq!(done.len(), 3, "{done:?}");
    assert!(crate::saved::resave(&copy).expect("a second pass").is_empty(), "already current");
    let new = crate::saved::load(&copy, &[]).expect("the resaved checkpoint loads");
    std::fs::remove_dir_all(&copy).unwrap();
    assert!(new.world == old.world, "the world comes back whole");
    assert_eq!(new.flora.config(), old.flora.config());
    assert_eq!(new.flora.view().stands, old.flora.view().stands);
    assert_eq!(new.fauna.config(), old.fauna.config());
    assert_eq!(new.fauna.view().animals, old.fauna.view().animals);
    assert_eq!(new.fauna.view().ledger, old.fauna.view().ledger);
    assert_eq!(new.minute, old.minute);
}
