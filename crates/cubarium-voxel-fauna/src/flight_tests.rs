//! **F1, written before the rule** (`design/handoffs/voxel-flight-2026-09-28.md` Brief F1
//! "Tests first", with the roster brief's "F1 overrides"; the design note is
//! `design/predation-and-flight-2026-09-28.md` §3): the flight substrate — the pose's
//! `y_m`, the `Fly` and `Perched` modes, air motion, perches, the flyer fan, nectar —
//! with the bellwing as its first lineage.
//!
//! In-crate because the `altitude` / `ceiling` claim reads the observation vector
//! (`body::observation`), which is crate-private.
//!
//! Fixtures are hand-built at the 0.25 m reference voxel: soil at `y = 1..=2`, so a plain
//! column's support face is `y = 2` and its top, the **surface**, is 0.75 m. Bellwings are
//! placed in the air with [`Command::IntroduceAloft`] (a pose whose `y_m` is the body's
//! base height) and driven by [`Scripted`] with a constant [`Scripted::lifting`] effort.
//! The day clock is off unless a test needs daylight, so the diurnal bellwing is awake.
//! No world is stepped: the fauna reads the world and does not move it. Plants are
//! stepped only to grow a fixture's nectar or flowers. No run is longer than 200 ticks.
//!
//! Readings these tests fix where the brief leaves a choice (each named again where it
//! is used):
//! - **The ceiling** is `y_m` (the base) at most `band_max_m` over the top of the support
//!   face below the body's column.
//! - **The sphere** is radius `span_m / 2` about the body; a flyer pressed against a wall
//!   keeps its centre at least that far from the face plane.
//! - **An underside** is perched on by rising into it (lift up while touching the slab
//!   from below); a crown top by descending onto it. A flyer moving level through a crown
//!   passes through it at the foliage speed.
//! - **"Can't pay a takeoff"** is an empty reserve and a body at `body_min` (JUVENILE-FOOD:
//!   a takeoff is paid out of the reserve, then the body above its death line).
//! - **`ceiling`** in `Self` is the clear height above the body's base to the first solid
//!   or water cell in its column, over `band_max_m`, capped at 1; **`altitude`** is the
//!   base's height over the support face below, over `band_max_m`.

use cubarium_voxel::weather::{DayClock, DayMode, RainMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_flora::{
    Command as FloraCommand, Face, FaceDir, Flora, FloraConfig, Site, Species as Plant, SpurPhase,
    VineConfig, VineSeed,
};

use crate::{
    Actions, Animal, Bloom, Command, ConeHit, Diet, Fauna, FaunaConfig, Flight, Food, Founder,
    FounderPhysiology, Founding, Hunter, MobilityMode, PerchKind, Pose, Scripted, StartingStores,
    surface_m,
};

const V: f64 = 0.25;
const BW: Founder = Founder::Bellwing;
/// The top of a plain column's support face `y = 2`.
const SURFACE: f64 = 0.75;
/// Facing `+x`: `Pose::forward` is `(sin h, cos h)`.
const EAST: f64 = std::f64::consts::FRAC_PI_2;
/// A body that is perched sits on its perch within this, metres.
const ON: f64 = 0.02;

// ------------------------------------------------------------------ fixtures

fn clock_off() -> DayClock {
    DayClock {
        mode: DayMode::Off,
        ..DayClock::default()
    }
}

/// A day clock held at `hour` (the world is never stepped, so it stays there).
fn clock_at(hour: f64) -> DayClock {
    DayClock {
        start_hour: hour,
        ..DayClock::default()
    }
}

fn config(width: u32, height: u32, depth: u32, day: DayClock) -> VoxelConfig {
    VoxelConfig {
        width,
        height,
        depth,
        voxel_m: V,
        seed: 7,
        day,
        ..VoxelConfig::default()
    }
}

/// Soil at `y = 1..=2` everywhere, with every soil voxel at field capacity (the plants'
/// and the vines' fixtures need wet ground; nothing here needs it dry).
fn soil(world: &mut World) {
    let c = world.config().clone();
    let m = cubarium_voxel::Materials::DEFAULT;
    let fc = m.field_capacity(Material::Soil);
    let cap = m.pore_capacity(Material::Soil) * c.voxel_volume();
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            for y in 1..=2 {
                world.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z,
                    volume_m3: fc * cap,
                });
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
}

fn plain_with(c: VoxelConfig) -> World {
    let mut w = World::empty(c);
    soil(&mut w);
    w
}

/// A flat plain, day clock off.
fn plain(width: u32, height: u32, depth: u32) -> World {
    plain_with(config(width, height, depth, clock_off()))
}

/// Rock over columns `xs` (every `z`) at `ys`.
fn rock(world: &mut World, xs: std::ops::RangeInclusive<i64>, ys: std::ops::RangeInclusive<u32>) {
    let depth = world.config().depth;
    for z in 0..depth {
        for x in xs.clone() {
            for y in ys.clone() {
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Rock,
                });
            }
        }
    }
}

/// The centre of column `(x, z)` in metres.
fn centre(x: u32, z: u32) -> (f64, f64) {
    ((f64::from(x) + 0.5) * V, (f64::from(z) + 0.5) * V)
}

fn bellwing_flight(fauna: &Fauna) -> Flight {
    fauna
        .config()
        .founder(BW)
        .flight
        .expect("the bellwing has a flight table (F1)")
}

/// Put a bellwing in the air at `(x_m, y_m, z_m)`; returns its id.
fn aloft(fauna: &mut Fauna, world: &World, x_m: f64, z_m: f64, y_m: f64, heading_rad: f64, stores: StartingStores) -> u64 {
    assert!(
        fauna.apply(
            world,
            Command::IntroduceAloft {
                pose: Pose {
                    x: x_m,
                    z: z_m,
                    heading_rad,
                    y_m,
                },
                founder: BW,
                stores,
            },
        ),
        "the fixture places a bellwing aloft at ({x_m}, {y_m}, {z_m})"
    );
    fauna.view().ledger.births - 1
}

/// Put a founder on the highest support face of column `(x, z)`; returns its id.
fn on_ground(fauna: &mut Fauna, world: &World, founder: Founder, x: i64, z: u32, stores: StartingStores) -> u64 {
    assert!(
        fauna.apply(
            world,
            Command::IntroduceFounder {
                x,
                z,
                founder,
                stores,
                heading_rad: EAST,
            },
        ),
        "the fixture places a {founder:?} at ({x}, {z})"
    );
    fauna.view().ledger.births - 1
}

/// Hold `forward`, `feed` and `lift` every tick.
fn fly(fauna: &mut Fauna, id: u64, forward: f64, feed: f64, lift: f64) {
    assert!(fauna.set_controller(
        id,
        Box::new(
            Scripted::new(vec![Actions {
                forward,
                turn: 0.0,
                feed,
            }])
            .lifting(lift),
        ),
    ));
}

fn animal(fauna: &Fauna, id: u64) -> Animal {
    *fauna.view().animal(id).expect("the body is alive")
}

fn mode(fauna: &Fauna, id: u64) -> MobilityMode {
    animal(fauna, id).mobility.mode
}

fn perch_kind(fauna: &Fauna, id: u64) -> Option<PerchKind> {
    animal(fauna, id).mobility.perch.map(|p| p.kind)
}

fn steps(fauna: &mut Fauna, world: &World, flora: &mut Flora, n: usize) {
    for _ in 0..n {
        fauna.step(world, flora);
    }
}

/// Step until `done` or `cap` ticks; returns the ticks taken.
fn until(fauna: &mut Fauna, world: &World, flora: &mut Flora, cap: usize, done: impl Fn(&Fauna) -> bool) -> usize {
    let mut n = 0;
    while !done(fauna) && n < cap {
        fauna.step(world, flora);
        n += 1;
    }
    n
}

/// Respiration over `n` ticks beyond basal upkeep: what flight (or anything but
/// maintenance) cost. One body per layer in every caller, so it is that body's.
fn beyond_basal(fauna: &mut Fauna, world: &World, flora: &mut Flora, n: usize) -> f64 {
    let l = &fauna.view().ledger;
    let before = l.respired_out - l.respired_maintenance_out;
    steps(fauna, world, flora, n);
    let l = &fauna.view().ledger;
    l.respired_out - l.respired_maintenance_out - before
}

/// All respiration over `n` ticks.
fn respired(fauna: &mut Fauna, world: &World, flora: &mut Flora, n: usize) -> f64 {
    let before = fauna.view().ledger.respired_out;
    steps(fauna, world, flora, n);
    fauna.view().ledger.respired_out - before
}

/// Both layers' books close, and what the plant layer lost is what the animal booked
/// eating minus what it put back.
fn ledgers_close(fauna: &Fauna, flora: &Flora, flora_organic_before: f64) {
    let av = fauna.view();
    let fv = flora.view();
    for (name, got, want) in [
        ("fauna organic", av.organic(), av.ledger.expected_organic()),
        ("fauna mineral", av.mineral(), av.ledger.expected_mineral()),
        ("fauna energy", av.energy(), av.ledger.expected_energy()),
        ("flora organic", fv.organic(), fv.ledger.expected_organic()),
        ("flora mineral", fv.mineral(), fv.ledger.expected_mineral()),
        ("flora energy", fv.energy(), fv.ledger.expected_energy()),
    ] {
        assert!(
            (got - want).abs() <= 1e-9 * got.abs().max(1.0),
            "the {name} ledger must close: {got} against {want}"
        );
    }
    let lost = flora_organic_before - fv.organic();
    let net = av.ledger.eaten_organic_in - av.ledger.deposited_organic_out;
    assert!(
        (lost - net).abs() <= 1e-12 * lost.abs().max(1.0),
        "the plant layer lost {lost} and the animal booked {net}"
    );
}

/// Seed a full-grown stand of `species` on column `(x, z)`; returns its site.
fn full_stand(flora: &mut Flora, world: &World, species: Plant, x: i64, z: u32) -> Site {
    let wood = flora.config().species(species).wood_max;
    assert!(flora.apply(world, FloraCommand::Seed { x, z, species, wood }));
    flora
        .view()
        .stands
        .iter()
        .find(|s| i64::from(s.site.x) == x && s.site.z == z)
        .expect("the stand")
        .site
}

/// A stand's foliage layers as `(cell, layer)`, bottom-up.
fn foliage_layers(flora: &Flora, site: Site) -> Vec<cubarium_voxel_flora::StandLayer> {
    let mut v: Vec<_> = flora
        .view()
        .layers_at(site)
        .filter(|l| l.foliage_index.is_some())
        .collect();
    v.sort_by_key(|l| l.cell);
    v
}

/// The top of a stand's crown slab: the top of its highest foliage layer's cell.
fn crown_top(flora: &Flora, site: Site) -> f64 {
    let top = *foliage_layers(flora, site).last().expect("a crown");
    (top.cell + 1) as f64 * V
}

/// A flora config whose bloomcrowns make nectar fast enough for a fixture: 1e-3 a second
/// up to 2e-3 (full in 2 s of daylight). Explicit, so the implementer's defaults change
/// none of these results.
fn nectar_flora() -> FloraConfig {
    let mut c = FloraConfig::default();
    c.bloomcrown.nectar_per_s = 1e-3;
    c.bloomcrown.nectar_cap = 2e-3;
    c
}

/// A bloomcrown on `(x, z)` given `ticks` of daylight to make nectar (the world's clock
/// must be at day).
fn blooming(flora: &mut Flora, world: &mut World, x: i64, z: u32, ticks: usize) -> Site {
    let site = full_stand(flora, world, Plant::Bloomcrown, x, z);
    for _ in 0..ticks {
        flora.step(world);
    }
    site
}

fn stand_nectar(flora: &Flora, site: Site) -> f64 {
    flora.view().stand_at(site).expect("the stand").nectar
}

#[test]
fn indexed_mouth_reach_selects_the_same_flower_as_a_full_scan() {
    let mut world = plain_with(config(24, 24, 9, clock_at(12.0)));
    let mut flora = Flora::new(nectar_flora());
    let a = blooming(&mut flora, &mut world, 0, 4, 60);
    let b = blooming(&mut flora, &mut world, 22, 4, 60);
    let fv = flora.view();
    let view = world.view();
    assert!(stand_nectar(&flora, a) > 0.0 && stand_nectar(&flora, b) > 0.0);
    assert!(fv.stands_near(0, 4, view.config.width, view.config.depth).is_some());
    let ring = f64::from(view.config.width) * V;
    let top = crown_top(&flora, a);
    let mut found = false;
    // Crown edges, exact cell boundaries, the strip's edges, and both sides of the
    // wrapping seam. Several reach sizes include overlapping/equidistant flowers.
    for x in [-V, 0.0, V, 2.0 * V, ring - V, ring, ring + V] {
        for z in [0.0, 3.0 * V, 4.0 * V, 5.0 * V, 9.0 * V] {
            for y in [top - V, top, top + V] {
                for reach in [0.0, V, 2.0 * V] {
                    let p = (x, y, z);
                    let indexed = crate::flight::bloom_in_reach(&view, &fv, p, reach);
                    let scanned = crate::flight::bloom_in_reach_scan(&view, &fv, p, reach);
                    assert_eq!(indexed, scanned, "mouth {p:?}, reach {reach}");
                    found |= indexed.is_some();
                }
            }
        }
    }
    assert!(found);
}

// ------------------------------------------------------------------ the table

/// **The bellwing's table** (§3.9, the roster overrides): the specimen's name, a nectar
/// diet, diurnal, founded by bodies, on no hunter's list, and a flyer — the only one.
/// No tuned number is pinned. Its manifest adds `lift` to the three shared actions; every walker keeps three
/// (the lanternjaw its `lure`), so no existing action list moves.
#[test]
fn the_bellwing_table_is_a_diurnal_nectar_flyer() {
    assert_eq!(BW.name(), "bellwing", "the specimen key");
    assert!(Founder::ALL.contains(&BW));
    assert!(Food::ALL.contains(&Food::Nectar) && !Food::Nectar.is_pool(), "a flower product");
    assert_eq!(Food::Nectar.name(), "nectar");

    let bw = FounderPhysiology::frozen(BW);
    assert_eq!(bw.diet, Diet::only(Food::Nectar));
    // RHYTHMS-CADENCE: a leaning (night, some day, dawn and dusk peaks).
    assert_eq!(bw.active, crate::Activity::Leaning);
    assert_eq!(bw.founding, Founding::Bodies);
    assert_eq!(
        FaunaConfig::default().founder_counts()[BW.index()],
        bw.founding_count
    );
    assert!(bw.hunter.is_none());
    // The detritus web D2: on the lanternjaw's list, taken only at rest (hunt_tests).
    assert!(Hunter::LANTERNJAW.hunts(BW), "on the lanternjaw's list since D2");

    let f = bw.flight.expect("the bellwing flies");
    assert!(f.span_m > 0.0 && f.band_max_m > 0.0 && f.takeoff_s > 0.0);
    assert!(f.climb_work_per_m > 0.0, "lift work is paid going up");
    assert!(f.shower_rain_reference_m_per_s > 0.0);
    for other in Founder::ALL.into_iter().filter(|&o| o != BW) {
        assert!(FounderPhysiology::frozen(other).flight.is_none(), "{other:?} walks");
    }

    let m = BW.manifest();
    assert_eq!(m.actions.len(), 4);
    assert_eq!(m.actions[3].name, "lift_effort");
    assert_eq!((m.actions[3].low, m.actions[3].high), (-1.0, 1.0));
    assert_eq!(m.actions[3].transfer, crate::Transfer::Tanh);
    // Contract v3: no other lineage has a lift.
    for other in Founder::ALL.into_iter().filter(|&o| o != BW) {
        assert!(other.manifest().actions.iter().all(|a| a.name != "lift_effort"), "{other:?} lifts");
    }
}

// ------------------------------------------------------------------ 1. aloft

/// **1. A bellwing with no support under its feet stays aloft while paying the airborne
/// cost; a walker in the same spot is refused.** A bellwing placed 1 m over a plain and
/// holding a hover (forward 0, lift 0) for 40 ticks is flying, keeps its height, and
/// respires beyond its basal upkeep at least that upkeep again (hover is 5× basal, so the
/// excess is at least 4×). A shredder asked to stand at the same pose is refused.
/// Walkers are unchanged: an introduced shredder's `y_m` is the top of its support face,
/// and stays so as it walks.
#[test]
fn a_bellwing_stays_aloft_paying_and_a_walker_there_is_refused() {
    let world = plain(16, 20, 8);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let (x, z) = centre(8, 4);
    let y = SURFACE + 1.0;
    let id = aloft(&mut fauna, &world, x, z, y, EAST, StartingStores::FULL);
    fly(&mut fauna, id, 0.0, 0.0, 0.0);
    assert_eq!(mode(&fauna, id), MobilityMode::Fly, "placed in the air, it flies");

    let body = animal(&fauna, id).body;
    let basal = fauna.config().founder(BW).core.maintenance_per_s * body * 40.0 * DT;
    let extra = beyond_basal(&mut fauna, &world, &mut flora, 40);
    let a = animal(&fauna, id);
    assert_eq!(a.mobility.mode, MobilityMode::Fly, "still aloft");
    assert!((a.pose.y_m - y).abs() < 1e-9, "a hover holds its height: {}", a.pose.y_m);
    assert!(extra >= basal, "the airborne cost is paid: {extra} beyond a basal {basal}");

    // A walker at the same pose.
    let births = fauna.view().ledger.births;
    assert!(
        !fauna.apply(
            &world,
            Command::IntroduceAloft {
                pose: Pose { x, z, heading_rad: EAST, y_m: y },
                founder: Founder::Blind,
                stores: StartingStores::FULL,
            },
        ),
        "a walker cannot be put in the air"
    );
    assert_eq!(fauna.view().ledger.births, births, "and nothing was placed");

    // Walkers stand on their face.
    let mut walkers = Fauna::new(FaunaConfig::default());
    let s = on_ground(&mut walkers, &world, Founder::Blind, 4, 4, StartingStores::FULL);
    fly(&mut walkers, s, 1.0, 0.0, 0.0);
    for _ in 0..40 {
        let a = animal(&walkers, s);
        assert_eq!(a.mobility.mode, MobilityMode::Ground);
        assert!(
            (a.pose.y_m - surface_m(a.site.y, V)).abs() < 1e-12,
            "a walker's y_m is its support face's top: {} on {:?}",
            a.pose.y_m,
            a.site
        );
        walkers.step(&world, &mut flora);
    }
}

// ------------------------------------------------------------------ 2. air only

/// **2. It cannot enter rock or water, and cannot climb past the ceiling.**
/// - Flying east at full forward toward a rock wall (columns 10..=11, up to `y = 15`) for
///   200 ticks, its centre never comes nearer the wall's face than its sphere's radius,
///   and the cell at its base is never solid.
/// - Descending at full down-lift over a pond (one cell of water on every column) for 200
///   ticks, its base never goes below the water's surface.
/// - Climbing at full lift for 200 ticks, its base never stands more than `band_max_m`
///   over the support face below, and gets within a voxel of it.
#[test]
fn a_flyer_never_enters_rock_or_water_nor_climbs_past_its_ceiling() {
    // Rock.
    let mut world = plain(24, 20, 8);
    rock(&mut world, 10..=11, 3..=15);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let (x, z) = centre(4, 4);
    let y = SURFACE + 1.0;
    let id = aloft(&mut fauna, &world, x, z, y, EAST, StartingStores::FULL);
    fly(&mut fauna, id, 1.0, 0.0, 0.0);
    let r = bellwing_flight(&fauna).span_m / 2.0;
    let face = 10.0 * V;
    let mut nearest = f64::INFINITY;
    for t in 0..200 {
        fauna.step(&world, &mut flora);
        let a = animal(&fauna, id);
        nearest = nearest.min(face - a.pose.x);
        assert!(
            a.pose.x + r <= face + 1e-9,
            "tick {t}: the sphere crosses the rock face: x {} radius {r}",
            a.pose.x
        );
        let cell = ((a.pose.y_m / V).floor() as u32, (a.pose.x / V).floor() as i64);
        assert!(!world.view().is_solid(cell.1, cell.0, 4), "tick {t}: inside rock at {cell:?}");
    }
    assert!(nearest < r + 0.1, "it flew up to the wall: nearest {nearest}");

    // Water.
    let mut world = plain(16, 20, 8);
    let cell = world.config().voxel_volume();
    for z in 0..8 {
        for x in 0..16 {
            world.apply(WorldCommand::AddWater { x, y: 3, z, volume_m3: cell });
        }
    }
    let water_top = SURFACE + world.view().standing_depth_m(8, 2, 4);
    assert!((water_top - (SURFACE + V)).abs() < 1e-9, "fixture: a cell of water");
    let mut fauna = Fauna::new(FaunaConfig::default());
    let (x, z) = centre(8, 4);
    let id = aloft(&mut fauna, &world, x, z, SURFACE + 1.5, EAST, StartingStores::FULL);
    fly(&mut fauna, id, 0.0, 0.0, -1.0);
    let mut lowest = f64::INFINITY;
    for t in 0..200 {
        fauna.step(&world, &mut flora);
        let a = animal(&fauna, id);
        lowest = lowest.min(a.pose.y_m);
        assert!(a.pose.y_m >= water_top - 1e-9, "tick {t}: under the water at {}", a.pose.y_m);
    }
    assert!(lowest < water_top + 0.3, "it came down to the water: lowest {lowest}");

    // Ceiling.
    let world = plain(16, 24, 8);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = aloft(&mut fauna, &world, x, z, SURFACE + 1.5, EAST, StartingStores::FULL);
    fly(&mut fauna, id, 0.0, 0.0, 1.0);
    let band = bellwing_flight(&fauna).band_max_m;
    let mut highest = 0.0f64;
    for t in 0..200 {
        fauna.step(&world, &mut flora);
        let a = animal(&fauna, id);
        let over = a.pose.y_m - surface_m(a.site.y, V);
        highest = highest.max(over);
        assert!(over <= band + 1e-9, "tick {t}: {over} m over its support, ceiling {band}");
    }
    assert!(highest > band - V, "it climbed to the ceiling: {highest} of {band}");
}

// ------------------------------------------------------------------ 3. costs

/// **3. Hover costs more per second than cruise, which costs more than perching; lift
/// work is paid going up and not going down.** Five single-bellwing layers on one plain,
/// 10 ticks each, all starting 1.25 m up (the perched one on the ground): hover (forward
/// 0, lift 0), cruise (forward 1), perched (resting on its face), climbing (lift 1) and
/// descending (lift −1). Hover > cruise > perched, and cruise is at least 3.5× perched
/// (4× basal); the climb costs the hover plus `climb_work_per_m × body × rise` (within
/// 10 %); the descent costs no more than the hover.
#[test]
fn hover_costs_more_than_cruise_than_perching_and_only_climbing_pays_lift_work() {
    let world = plain(48, 20, 8);
    let mut flora = Flora::new(FloraConfig::default());
    let (x, z) = centre(8, 4);
    let y = SURFACE + 1.25;
    let run = |forward: f64, lift: f64, flora: &mut Flora| -> (f64, f64, f64) {
        let mut fauna = Fauna::new(FaunaConfig::default());
        let id = aloft(&mut fauna, &world, x, z, y, EAST, StartingStores::FULL);
        fly(&mut fauna, id, forward, 0.0, lift);
        let body = animal(&fauna, id).body;
        let paid = respired(&mut fauna, &world, flora, 10);
        let a = animal(&fauna, id);
        assert_eq!(a.mobility.mode, MobilityMode::Fly, "aloft throughout ({forward}, {lift})");
        (paid, a.pose.y_m - y, body)
    };
    let (hover, _, _) = run(0.0, 0.0, &mut flora);
    let (cruise, _, _) = run(1.0, 0.0, &mut flora);
    let (up, rise, body) = run(0.0, 1.0, &mut flora);
    let (down, fall, _) = run(0.0, -1.0, &mut flora);

    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = on_ground(&mut fauna, &world, BW, 8, 4, StartingStores::FULL);
    fly(&mut fauna, id, 0.0, 0.0, 0.0);
    assert_eq!(mode(&fauna, id), MobilityMode::Perched, "a flyer on a face is perched");
    let perched = respired(&mut fauna, &world, &mut flora, 10);
    assert_eq!(mode(&fauna, id), MobilityMode::Perched);

    assert!(hover > cruise, "hover {hover} over cruise {cruise}");
    assert!(cruise > perched, "cruise {cruise} over perched {perched}");
    assert!(cruise >= 3.5 * perched, "flight is several times basal: {cruise} vs {perched}");

    let climb_work = FaunaConfig::default().founder(BW).flight.expect("flies").climb_work_per_m;
    assert!(rise > 0.0 && fall < 0.0, "it rose {rise} and fell {fall}");
    let lift_work = climb_work * body * rise;
    assert!(
        ((up - hover) - lift_work).abs() <= 0.1 * lift_work,
        "going up pays the lift work: {} beyond the hover, {lift_work} expected",
        up - hover
    );
    assert!(down <= hover * (1.0 + 1e-9), "descent is free: {down} against a hover {hover}");
}

// ------------------------------------------------------------------ 4. perches

/// Descend (or rise) until perched; returns the ticks.
fn settle(fauna: &mut Fauna, world: &World, flora: &mut Flora, id: u64) -> usize {
    until(fauna, world, flora, 200, |f| mode(f, id) == MobilityMode::Perched)
}

/// It holds the perch at no airborne cost: 20 ticks resting there move it nowhere and
/// cost nothing beyond basal upkeep.
fn holds(fauna: &mut Fauna, world: &World, flora: &mut Flora, id: u64, what: &str) {
    fly(fauna, id, 0.0, 0.0, 0.0);
    let before = animal(fauna, id).pose;
    let extra = beyond_basal(fauna, world, flora, 20);
    let a = animal(fauna, id);
    assert_eq!(a.mobility.mode, MobilityMode::Perched, "{what}: still perched");
    assert!((a.pose.y_m - before.y_m).abs() < 1e-12, "{what}: it holds its height");
    assert!(extra.abs() < 1e-15, "{what}: no airborne cost on a perch, paid {extra}");
}

/// **4. It lands on a ground face, a bloomcrown crown top, a latticevine face and under a
/// frond's crown slab; each perch holds it at no airborne cost.** Descending at full
/// down-lift onto a plain, and onto a full-grown bloomcrown's crown from half a metre
/// over it; descending against a vine-covered wall (pressing into it) — and against the
/// same wall bare, where it slides down to the ground instead; rising at full lift under
/// a full-grown umbrellafrond's lowest foliage slab, two columns from its trunk.
#[test]
fn it_lands_on_ground_crown_vine_and_under_a_frond_and_each_perch_is_free() {
    // A ground face.
    let world = plain(16, 20, 8);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let (x, z) = centre(8, 4);
    let id = aloft(&mut fauna, &world, x, z, SURFACE + 1.0, EAST, StartingStores::FULL);
    fly(&mut fauna, id, 0.0, 0.0, -1.0);
    let n = settle(&mut fauna, &world, &mut flora, id);
    assert_eq!(mode(&fauna, id), MobilityMode::Perched, "landed in {n} ticks");
    assert_eq!(perch_kind(&fauna, id), Some(PerchKind::Face));
    assert!((animal(&fauna, id).pose.y_m - SURFACE).abs() <= ON, "on the ground");
    holds(&mut fauna, &world, &mut flora, id, "the ground");

    // A bloomcrown's crown top.
    let mut flora = Flora::new(FloraConfig::default());
    let site = full_stand(&mut flora, &world, Plant::Bloomcrown, 8, 4);
    let top = crown_top(&flora, site);
    assert!(top > SURFACE + 0.5, "fixture: a crown well off the ground at {top}");
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = aloft(&mut fauna, &world, x, z, top + 0.5, EAST, StartingStores::FULL);
    fly(&mut fauna, id, 0.0, 0.0, -1.0);
    let n = settle(&mut fauna, &world, &mut flora, id);
    assert_eq!(mode(&fauna, id), MobilityMode::Perched, "landed in {n} ticks");
    assert_eq!(perch_kind(&fauna, id), Some(PerchKind::CrownTop));
    let y = animal(&fauna, id).pose.y_m;
    assert!((y - top).abs() <= V / 2.0, "on the crown top {top}, not the ground: {y}");
    holds(&mut fauna, &world, &mut flora, id, "a crown top");

    // A latticevine face, and the same wall bare.
    for covered in [true, false] {
        let (world, mut flora, faces) = vine_wall(covered, FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::default());
        let r = bellwing_flight(&fauna).span_m / 2.0;
        let (_, z) = centre(0, WALL_Z);
        let id = aloft(&mut fauna, &world, WALL_FACE_M - r - 0.005, z, 6.0 * V, EAST, StartingStores::FULL);
        fly(&mut fauna, id, 0.3, 0.0, -0.3);
        let n = settle(&mut fauna, &world, &mut flora, id);
        let a = animal(&fauna, id);
        assert_eq!(a.mobility.mode, MobilityMode::Perched, "landed in {n} ticks");
        if covered {
            assert!(!faces.is_empty());
            assert_eq!(perch_kind(&fauna, id), Some(PerchKind::VineFace), "it clings to the vine");
            assert!(a.pose.y_m > SURFACE + V, "on the wall, not the ground: {}", a.pose.y_m);
            holds(&mut fauna, &world, &mut flora, id, "a vine face");
        } else {
            assert_eq!(perch_kind(&fauna, id), Some(PerchKind::Face), "a bare wall is no perch");
            assert!((a.pose.y_m - SURFACE).abs() <= ON, "it came down to the ground");
        }
    }

    // Under a frond's crown slab.
    let world = plain(20, 24, 8);
    let mut flora = Flora::new(FloraConfig::default());
    let site = full_stand(&mut flora, &world, Plant::Umbrellafrond, 8, 4);
    let lowest = foliage_layers(&flora, site)[0];
    let underside = lowest.cell as f64 * V;
    let view = world.view();
    let stand = *flora.view().stand_at(site).expect("the frond");
    assert!(
        crate::layer_columns(&view, &stand, &lowest).contains(&(10, 4)),
        "fixture: the lowest slab covers column 10"
    );
    let mut fauna = Fauna::new(FaunaConfig::default());
    let (x, z) = centre(10, 4);
    let id = aloft(&mut fauna, &world, x, z, SURFACE + 0.3, EAST, StartingStores::FULL);
    assert!(SURFACE + 0.3 < underside - 0.3, "fixture: it starts well under the slab");
    fly(&mut fauna, id, 0.0, 0.0, 1.0);
    let n = settle(&mut fauna, &world, &mut flora, id);
    assert_eq!(mode(&fauna, id), MobilityMode::Perched, "hung up in {n} ticks");
    assert_eq!(perch_kind(&fauna, id), Some(PerchKind::Underside));
    let y = animal(&fauna, id).pose.y_m;
    assert!(y < underside && y > underside - 0.5, "hanging under the slab at {underside}: {y}");
    holds(&mut fauna, &world, &mut flora, id, "an underside");
}

/// The vine wall's west face plane, metres: rock at `x = 6`.
const WALL_FACE_M: f64 = 6.0 * V;
/// The slab the vine climbs.
const WALL_Z: u32 = 2;

/// A rock wall at `x = 6`, `y = 1..=12`, on a wet plain 16 × 20 × 5, day clock off. With
/// `covered`, a latticevine rooted at its foot on slab [`WALL_Z`] covers its west faces
/// `y = 3..=10` at full leaf; returns those faces.
fn vine_wall(covered: bool, config: FloraConfig) -> (World, Flora, Vec<Face>) {
    let mut world = plain(16, 20, 5);
    rock(&mut world, 6..=6, 1..=12);
    let mut flora = Flora::new(config);
    let mut faces = Vec::new();
    if covered {
        let first = Face::new(6, 3, WALL_Z, FaceDir::NegX);
        let id = flora
            .seed_vine(
                &world,
                VineSeed {
                    root: Site { x: 5, y: 2, z: WALL_Z },
                    first,
                    reserve: 100.0,
                    lineage: None,
                },
            )
            .expect("fixture: the vine founder");
        faces.push(first);
        for y in 4..=10 {
            let f = Face::new(6, y, WALL_Z, FaceDir::NegX);
            assert!(flora.cover_face(&world, id, f, 1.0), "fixture: cover {f:?}");
            faces.push(f);
        }
    }
    (world, flora, faces)
}

// ------------------------------------------------------------------ 5. losing a perch

/// **5. Its perch crown is eaten away: it takes off, or drops when it can't pay.** A
/// bellwing lands on a full-grown bloomcrown's crown top; the crown's foliage is then
/// taken whole (the species' graze refuge set to 0 so a take can strip it). With a full
/// reserve — or an empty one and body to spare over `body_min` (JUVENILE-FOOD: the body
/// pays what the reserve cannot) — it takes off within a few ticks and stays up (holding a
/// hover). When a takeoff costs more than its reserve and spare body together — here a
/// takeoff made dearer than the whole body — it drops to the ground below and perches
/// there, never rising above the crown it lost.
#[test]
fn a_lost_crown_perch_means_a_takeoff_or_a_drop() {
    for (stores, dear, pays) in [
        (StartingStores::FULL, false, true),
        (StartingStores { body: 1.0, reserve: 0.0 }, false, true),
        (StartingStores::FULL, true, false),
    ] {
        let world = plain(16, 20, 8);
        let mut config = FloraConfig::default();
        config.bloomcrown.graze_refuge = 0.0;
        let mut flora = Flora::new(config);
        let site = full_stand(&mut flora, &world, Plant::Bloomcrown, 8, 4);
        let top = crown_top(&flora, site);
        let mut fconfig = FaunaConfig::default();
        if dear {
            let p = &mut fconfig.founders[Founder::Bellwing.index()];
            let flight = p.flight.as_mut().expect("a flyer");
            // Over the whole body and reserve: (body_max + its full reserve) over basal.
            let basal = p.core.maintenance_per_s * p.core.body_max;
            flight.takeoff_s = 2.0 * (1.0 + p.core.reserve_cap) * p.core.body_max / (flight.flight_factor * basal);
        }
        let mut fauna = Fauna::new(fconfig);
        let (x, z) = centre(8, 4);
        let id = aloft(&mut fauna, &world, x, z, top + 0.3, EAST, stores);
        fly(&mut fauna, id, 0.0, 0.0, -1.0);
        let n = until(&mut fauna, &world, &mut flora, 100, |f| mode(f, id) == MobilityMode::Perched);
        assert_eq!(perch_kind(&fauna, id), Some(PerchKind::CrownTop), "fixture: perched in {n} ticks");

        assert!(flora.take_foliage(site, 1e9).is_some(), "fixture: the crown is eaten");
        assert!(flora.view().stand_at(site).expect("alive").foliage <= 1e-12, "stripped");
        fly(&mut fauna, id, 0.0, 0.0, 0.0);
        if pays {
            let n = until(&mut fauna, &world, &mut flora, 5, |f| mode(f, id) == MobilityMode::Fly);
            assert_eq!(mode(&fauna, id), MobilityMode::Fly, "it took off ({n} ticks)");
            steps(&mut fauna, &world, &mut flora, 20);
            let a = animal(&fauna, id);
            assert_eq!(a.mobility.mode, MobilityMode::Fly, "and stays up");
            assert!(a.pose.y_m > top - V, "near where its perch was: {}", a.pose.y_m);
        } else {
            let mut highest = 0.0f64;
            for _ in 0..200 {
                fauna.step(&world, &mut flora);
                highest = highest.max(animal(&fauna, id).pose.y_m);
                if mode(&fauna, id) == MobilityMode::Perched
                    && perch_kind(&fauna, id) == Some(PerchKind::Face)
                {
                    break;
                }
            }
            let a = animal(&fauna, id);
            assert_eq!(a.mobility.mode, MobilityMode::Perched, "it dropped and perched");
            assert_eq!(perch_kind(&fauna, id), Some(PerchKind::Face), "on the ground");
            assert!((a.pose.y_m - SURFACE).abs() <= ON, "below its lost perch: {}", a.pose.y_m);
            assert!(highest <= top + 1e-9, "it never rose: {highest} over a crown at {top}");
        }
    }
}

// ------------------------------------------------------------------ 6. sipping

/// A spur config fast enough to flower inside a fixture: the latticevine tests' numbers
/// (`crates/cubarium-voxel-flora/tests/latticevine.rs`, `fast` with a fixed cover).
fn flowering_vines() -> FloraConfig {
    let mut c = FloraConfig {
        decomposition: 0.0,
        wood_decomposition: 0.0,
        carrion_decomposition: 0.0,
        ..FloraConfig::default()
    };
    c.latticevine = VineConfig {
        spur_reserve_min: 0.1,
        // Back to back, no rest; 0.01 a flower and 0.02 a bunch over the 31.5 s cycle.
        spur_share: 1.0,
        bud_s: 0.5,
        flower_s: 30.0,
        fruit_s: 0.5,
        spent_s: 0.5,
        nectar_per_face_s: 0.01 / 31.5,
        fruit_per_face_s: 0.02 / 31.5,
        spread_cost: 1e12,
        sister_cost: 1e12,
        ..c.latticevine
    };
    c
}

/// **6. A sip takes latticevine nectar through `take_nectar` and bloomcrown nectar from the
/// new stock; the books close; an empty flower gives nothing.**
/// - A vine wall rained on once and stepped dry until a face is in flower with nectar; a
///   bellwing hovering against that face, holding `feed`, takes nectar: a nectar bite, the
///   face's nectar falls, the plant layer books consumed exactly what the animal booked
///   eating, the body records the face as its last bloom, and both ledgers close.
/// - A bloomcrown given daylight to make nectar; a bellwing descending onto its crown
///   holding `feed` sips: the stand's nectar falls, booked the same way, the body builds
///   tissue from it (nectar carries a little mineral), records the stand, and the books
///   close.
/// - The same flowers drained beforehand give no bite.
#[test]
fn a_sip_takes_vine_and_bloomcrown_nectar_the_books_close_and_an_empty_flower_gives_nothing() {
    // The vine.
    for drained in [false, true] {
        let (mut sky, mut flora, faces) = vine_wall(true, flowering_vines());
        assert!(!sky.view().is_raining());
        // A shower ends: the spurs start. The rain twin is the same wall in an open world
        // with a rain rate (the latticevine tests' twin trick).
        let mut rain = {
            let mut c = sky.config().clone();
            c.rain_m_per_s = 1e-6;
            let mut wet = World::empty(c);
            soil(&mut wet);
            rock(&mut wet, 6..=6, 1..=12);
            wet
        };
        assert!(rain.view().is_raining(), "fixture: the twin rains");
        flora.step(&mut rain);
        let flowering = |flora: &Flora| {
            faces.iter().copied().find(|&f| {
                flora
                    .view()
                    .cover
                    .face(f)
                    .is_some_and(|c| c.spur == SpurPhase::Flower && c.nectar > 0.0)
            })
        };
        let mut t = 0;
        while flowering(&flora).is_none() && t < 160 {
            flora.step(&mut sky);
            t += 1;
        }
        let face = flowering(&flora).expect("fixture: a face flowers after the shower");
        if drained {
            assert!(flora.take_nectar(face, 1e9).is_some(), "fixture: drained");
        }
        let nectar = flora.view().cover.face(face).expect("the face").nectar;
        let organic = flora.view().organic();
        let consumed = flora.view().ledger.consumed_organic_out;
        let visits_before = flora.view().cover.face(face).expect("the face").visits;
        let flower_visits_before = flora.view().ledger.flower_visits[1];

        let mut fauna = Fauna::new(FaunaConfig::default());
        let r = bellwing_flight(&fauna).span_m / 2.0;
        let (_, z) = centre(0, WALL_Z);
        let y = f64::from(face.y) * V + 0.05;
        let id = aloft(&mut fauna, &sky, WALL_FACE_M - r - 0.005, z, y, EAST, StartingStores::HUNGRY);
        fly(&mut fauna, id, 0.0, 1.0, 0.0);
        steps(&mut fauna, &sky, &mut flora, 40);
        let l = &fauna.view().ledger;
        if drained {
            assert_eq!(l.bites_by_food[Food::Nectar.index()], 0, "an empty flower gives nothing");
            assert_eq!(animal(&fauna, id).last_bloom, None);
            continue;
        }
        assert!(l.bites_by_food[Food::Nectar.index()] >= 1, "a nectar bite: {:?}", l.bites_by_food);
        assert!(l.bites_by_founder_food[BW.index()][Food::Nectar.index()] >= 1);
        let eaten = l.eaten_by_food[Food::Nectar.index()];
        assert!(eaten > 0.0);
        let left = flora.view().cover.face(face).expect("the face").nectar;
        assert!(left < nectar, "the face's nectar fell: {left} of {nectar}");
        let booked = flora.view().ledger.consumed_organic_out - consumed;
        assert!((booked - eaten).abs() <= 1e-12, "take_nectar booked {booked}, the sip {eaten}");
        assert_eq!(animal(&fauna, id).last_bloom, Some(Bloom::Vine(face)), "the pollen seam");
        assert_eq!(flora.view().cover.face(face).unwrap().visits, visits_before + 1,
            "repeated sips book one arrival at this flowering face");
        // VINE-C: the one visit also pollinates any other open flower its wings brush.
        assert!(flora.view().ledger.flower_visits[1] >= flower_visits_before + 1,
            "the vine visit is booked in the flora ledger");
        ledgers_close(&fauna, &flora, organic);
    }

    // The bloomcrown.
    for drained in [false, true] {
        let mut world = plain_with(config(16, 20, 8, clock_at(12.0)));
        assert!(world.life_daylight() > 0.5, "fixture: noon");
        let mut flora = Flora::new(nectar_flora());
        let site = blooming(&mut flora, &mut world, 8, 4, 60);
        let nectar = stand_nectar(&flora, site);
        assert!(nectar > 0.0, "fixture: the bloomcrown made nectar in daylight (F1's flora rule)");
        if drained {
            assert!(flora.take_stand_nectar(site, 1e9).is_some(), "fixture: drained");
            assert_eq!(stand_nectar(&flora, site), 0.0);
        }
        let nectar = stand_nectar(&flora, site);
        let top = crown_top(&flora, site);
        let organic = flora.view().organic();
        let consumed = flora.view().ledger.consumed_organic_out;

        let mut fauna = Fauna::new(FaunaConfig::default());
        let (x, z) = centre(8, 4);
        let id = aloft(&mut fauna, &world, x, z, top + 0.3, EAST, StartingStores::HUNGRY);
        fly(&mut fauna, id, 0.0, 1.0, -1.0);
        steps(&mut fauna, &world, &mut flora, 60);
        let l = &fauna.view().ledger;
        if drained {
            assert_eq!(l.bites_by_food[Food::Nectar.index()], 0, "an empty flower gives nothing");
            assert_eq!(stand_nectar(&flora, site), 0.0);
            continue;
        }
        assert!(l.bites_by_food[Food::Nectar.index()] >= 1, "a nectar bite: {:?}", l.bites_by_food);
        let eaten = l.eaten_by_food[Food::Nectar.index()];
        assert!(stand_nectar(&flora, site) < nectar, "the stand's nectar fell");
        let booked = flora.view().ledger.consumed_organic_out - consumed;
        assert!((booked - eaten).abs() <= 1e-12, "the stock booked {booked}, the sip {eaten}");
        assert!(
            l.assimilated_by_founder[BW.index()] > 0.0,
            "nectar's small mineral share lets it build tissue"
        );
        assert_eq!(animal(&fauna, id).last_bloom, Some(Bloom::Stand(site)), "the pollen seam");
        ledgers_close(&fauna, &flora, organic);
    }
}

/// **FLOWER-FRUIT: a sip at a lanternberry's flowers is a pollinator's visit, booked to that
/// bush.** A full-grown lanternberry in flower (its flowers held open for the fixture) and
/// another out of reach: a hovering bellwing sipping at the first books one visit to it —
/// its arrival; the sips that follow at the same flowers are not new visits — and none to
/// the other. The books close.
#[test]
fn a_bellwing_sipping_lanternberry_flowers_books_its_visit_to_that_bush() {
    let mut world = plain_with(config(20, 20, 8, clock_at(12.0)));
    let mut c = FloraConfig::default();
    c.lanternberry.flower_s = 1e9;
    c.lanternberry.nectar_per_s = 1e-3;
    c.lanternberry.nectar_cap = 2e-3;
    let mut flora = Flora::new(c);
    let site = full_stand(&mut flora, &world, Plant::Lanternberry, 8, 4);
    let other = full_stand(&mut flora, &world, Plant::Lanternberry, 16, 4);
    for _ in 0..60 {
        flora.step(&mut world);
    }
    let st = *flora.view().stand_at(site).expect("the bush");
    assert_eq!(st.crop.phase, cubarium_voxel_flora::CropPhase::Flower, "fixture: in flower");
    assert!(st.nectar > 0.0, "fixture: its flowers hold nectar");
    let top = crown_top(&flora, site);
    let organic = flora.view().organic();
    let mut fauna = Fauna::new(FaunaConfig::default());
    let (x, z) = centre(8, 4);
    let id = aloft(&mut fauna, &world, x, z, top + 0.3, EAST, StartingStores::HUNGRY);
    fly(&mut fauna, id, 0.0, 1.0, -1.0);
    steps(&mut fauna, &world, &mut flora, 60);
    let sips = fauna.view().ledger.bites_by_founder_food[BW.index()][Food::Nectar.index()];
    assert!(sips >= 2, "it sipped there more than once: {sips}");
    assert_eq!(animal(&fauna, id).last_bloom, Some(Bloom::Stand(site)));
    assert_eq!(flora.view().stand_at(site).expect("the bush").crop.visits, 1, "one visit, booked to that bush");
    assert_eq!(flora.view().stand_at(other).expect("the other").crop.visits, 0, "none to the other");
    assert_eq!(flora.view().ledger.flower_visits[0], 1);
    ledgers_close(&fauna, &flora, organic);
}

// ------------------------------------------------------------------ 8. weather

/// A plain whose sky holds water, so it can rain; `downpour` forces one (the world is
/// never stepped, so it keeps falling).
fn weather_plain(downpour: bool) -> World {
    let mut c = config(16, 20, 8, clock_off());
    c.closed_water_budget = true;
    c.initial_atmosphere_m3 = 0.5;
    let mut w = plain_with(c);
    if downpour {
        assert!(w.force_rain(RainMode::Downpour), "fixture: a downpour");
        assert_eq!(w.weather().mode(), RainMode::Downpour);
    }
    w
}

/// **8. In a downpour a flyer aloft descends and lands, and a perched one can't take
/// off.** A bellwing 1.5 m up holding a hover comes down and perches within 200 ticks; a
/// bellwing perched on the ground holding full lift stays there for 40 ticks. In a dry
/// twin the same lift takes it up.
#[test]
fn a_downpour_grounds_a_flyer_and_keeps_a_perched_one_down() {
    let mut flora = Flora::new(FloraConfig::default());
    let wet = weather_plain(true);
    let (x, z) = centre(8, 4);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = aloft(&mut fauna, &wet, x, z, SURFACE + 1.5, EAST, StartingStores::FULL);
    fly(&mut fauna, id, 0.0, 0.0, 0.0);
    let n = until(&mut fauna, &wet, &mut flora, 200, |f| mode(f, id) == MobilityMode::Perched);
    let a = animal(&fauna, id);
    assert_eq!(a.mobility.mode, MobilityMode::Perched, "it came down in the downpour ({n} ticks)");
    assert!(a.pose.y_m < SURFACE + 1.5, "lower than it was: {}", a.pose.y_m);

    for (world, grounded) in [(weather_plain(true), true), (weather_plain(false), false)] {
        let mut fauna = Fauna::new(FaunaConfig::default());
        let id = on_ground(&mut fauna, &world, BW, 8, 4, StartingStores::FULL);
        assert_eq!(mode(&fauna, id), MobilityMode::Perched, "a flyer on a face is perched");
        fly(&mut fauna, id, 0.0, 0.0, 1.0);
        steps(&mut fauna, &world, &mut flora, 40);
        let a = animal(&fauna, id);
        if grounded {
            assert_eq!(a.mobility.mode, MobilityMode::Perched, "no takeoff in a downpour");
            assert!((a.pose.y_m - SURFACE).abs() <= ON);
        } else {
            assert_eq!(a.mobility.mode, MobilityMode::Fly, "dry, the same lift takes it up");
            assert!(a.pose.y_m > SURFACE + 0.1);
        }
    }
}

// ------------------------------------------------------------------ 9. foliage

/// **9. Inside foliage layers its speed cap halves.** Full forward for 4 ticks, level,
/// inside a full-grown umbrellafrond's lowest (widest) foliage slab, against the same in
/// open air at the same height: the distance inside is half the distance outside
/// (±10 %), and the path stays inside the slab's columns.
#[test]
fn inside_foliage_layers_the_speed_cap_halves() {
    let world = plain(32, 24, 8);
    let mut flora = Flora::new(FloraConfig::default());
    let site = full_stand(&mut flora, &world, Plant::Umbrellafrond, 8, 4);
    let slab = foliage_layers(&flora, site)[0];
    let stand = *flora.view().stand_at(site).expect("the frond");
    let cols = crate::layer_columns(&world.view(), &stand, &slab);
    for c in [(7, 4), (8, 4), (9, 4)] {
        assert!(cols.contains(&c), "fixture: the slab covers {c:?}");
    }
    let y = slab.cell as f64 * V + 0.05;
    let (cx, z) = centre(8, 4);
    let distance = |x0: f64, flora: &mut Flora| -> f64 {
        let mut fauna = Fauna::new(FaunaConfig::default());
        let id = aloft(&mut fauna, &world, x0, z, y, EAST, StartingStores::FULL);
        fly(&mut fauna, id, 1.0, 0.0, 0.0);
        steps(&mut fauna, &world, flora, 4);
        let a = animal(&fauna, id);
        assert_eq!(a.mobility.mode, MobilityMode::Fly, "level flight, no perch");
        a.pose.x - x0
    };
    let inside = distance(cx - 0.3, &mut flora);
    let open = distance(centre(24, 4).0, &mut flora);
    assert!(open > 0.0, "it flew: {open}");
    assert!(cx - 0.3 + inside < 10.0 * V, "the path stayed in the slab's columns");
    let ratio = inside / open;
    assert!((0.45..=0.55).contains(&ratio), "inside {inside} against open {open}: {ratio}");
}

// ------------------------------------------------------------------ 10. senses

/// **10. The flyer fan sees a flower 3 m below-ahead; `altitude` and `ceiling` read true
/// on a hand-built column.**
/// - The fan is §3.6's: pitches −80, −50, −20, +10, +40, range 4 m, three sectors.
/// - A bellwing over a rock ledge (columns 0..=8 up to `y = 6`, so 2.3 m over its own
///   support), facing east, with a nectar-holding bloomcrown's crown top 3 m away along
///   the −50° ray of its centre sector: a fan ray reports [`ConeHit::Bloom`] between 2
///   and 4 m. The same crown with no nectar reads as foliage and never as a bloom.
/// - Under a rock overhang whose underside is 0.75 m above its base, 1 m over a plain:
///   `altitude` 1.0 / 2.5 = 0.4 and `ceiling` 0.75 / 2.5 = 0.3; in the open beside it,
///   `altitude` 0.4 and `ceiling` 1 (capped). Both ±0.05 (the base or the top of a body
///   0.06 m tall).
#[test]
fn the_fan_sees_a_flower_below_ahead_and_altitude_and_ceiling_read_true() {
    let m = BW.manifest();
    assert_eq!(m.ray_pitch_offsets_deg, &[-80.0, -50.0, -20.0, 10.0, 40.0][..]);
    assert_eq!(m.cone_range_m, 4.0);
    assert_eq!(m.sector_centres_deg.len(), 3);
    let middle = m
        .sector_centres_deg
        .iter()
        .position(|&c| c == 0.0)
        .expect("a forward sector");

    let (d, drop) = (3.0 * 50f64.to_radians().cos(), 3.0 * 50f64.to_radians().sin());
    let mut bloom_ray = None;
    for nectar in [true, false] {
        let mut world = plain_with(config(24, 24, 9, clock_at(12.0)));
        rock(&mut world, 0..=8, 3..=6);
        // The no-nectar arm switches bloomcrown's nectar off explicitly (F1 implementer):
        // the shipped default makes nectar (`bloomcrown_is_the_one_species_with_a_nectar_stock`
        // requires it), so the default config cannot stand for "a crown with no nectar".
        let mut flora = Flora::new(if nectar {
            nectar_flora()
        } else {
            let mut c = FloraConfig::default();
            c.bloomcrown.nectar_per_s = 0.0;
            c
        });
        let site = blooming(&mut flora, &mut world, 16, 4, 60);
        assert_eq!(stand_nectar(&flora, site) > 0.0, nectar, "fixture: nectar {nectar}");
        let top = crown_top(&flora, site);
        let (sx, z) = centre(16, 4);
        let (x, y) = (sx - d, top + drop);
        let ledge = surface_m(6, V);
        assert!((x / V).floor() as u32 == 8, "fixture: over the ledge's last column");
        assert!(y - ledge <= 2.5, "fixture: within its band over the ledge: {}", y - ledge);

        let mut fauna = Fauna::new(FaunaConfig::default());
        let id = aloft(&mut fauna, &world, x, z, y, EAST, StartingStores::FULL);
        let (view, fv) = (world.view(), flora.view());
        let census = crate::flyer_fan_census(&view, &fv, &fauna.view(), &animal(&fauna, id), &[])
            .expect("a flyer has a fan");
        assert_eq!(census.range_m, 4.0);
        let blooms: Vec<_> = census.rays.iter().filter(|r| r.hit == ConeHit::Bloom).collect();
        if nectar {
            assert!(!blooms.is_empty(), "the fan sees the flower: {:?}", census.rays);
            let r = blooms
                .iter()
                .find(|r| r.sector == middle)
                .unwrap_or_else(|| panic!("in the forward sector: {blooms:?}"));
            assert!((2.0..=4.0).contains(&r.distance_m), "about 3 m off: {}", r.distance_m);
            bloom_ray = Some((r.sector, r.yaw_deg, r.pitch_deg));
        } else {
            assert!(blooms.is_empty(), "a crown with no nectar is no bloom: {blooms:?}");
            let (s, yaw, pitch) = bloom_ray.expect("the nectar arm ran first");
            let same = census
                .rays
                .iter()
                .find(|r| r.sector == s && r.yaw_deg == yaw && r.pitch_deg == pitch)
                .expect("the same ray");
            assert_eq!(same.hit, ConeHit::FoliageCrown, "the same ray reads the crown as foliage");
        }
    }

    // Altitude and ceiling.
    let mut world = plain(20, 20, 8);
    rock(&mut world, 3..=7, 10..=11);
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let y = SURFACE + 1.0;
    let (ux, z) = centre(5, 4);
    let under = aloft(&mut fauna, &world, ux, z, y, EAST, StartingStores::FULL);
    let open = aloft(&mut fauna, &world, centre(14, 4).0, z, y, EAST, StartingStores::FULL);
    let manifest = fauna.config().manifest(BW);
    let selfm = *manifest.modules.iter().find(|m| m.name == "Self").expect("a Self module");
    let channel = |name: &str| {
        selfm
            .channels
            .iter()
            .position(|&c| c == name)
            .unwrap_or_else(|| panic!("Self has `{name}`: {:?}", selfm.channels))
    };
    let (alt, ceil) = (selfm.offset + channel("altitude"), selfm.offset + channel("ceiling"));
    let read = |id: u64| {
        let i = fauna.animals.iter().position(|a| a.id == id).expect("alive");
        crate::body::observation(&fauna, i, &world.view(), &flora.view(), &manifest, None, None)
    };
    let (u, o) = (read(under), read(open));
    assert!((u[alt] - 0.4).abs() <= 0.05, "altitude under the overhang: {}", u[alt]);
    assert!((u[ceil] - 0.3).abs() <= 0.05, "ceiling under the overhang: {}", u[ceil]);
    assert!((o[alt] - 0.4).abs() <= 0.05, "altitude in the open: {}", o[alt]);
    assert!((o[ceil] - 1.0).abs() <= 1e-9, "ceiling in the open is capped: {}", o[ceil]);
}

// ------------------------------------------------------------------ 11. snapshot

/// **11. Snapshot round trip with a flying and a perched bellwing.** One bellwing hovering
/// 1.2 m up and one perched on the ground, stepped 5 ticks: the layer read back from its
/// bytes holds the same bodies — pose `y_m`, mode and perch included — and the same
/// ledger.
#[test]
fn a_snapshot_round_trip_keeps_a_flying_and_a_perched_bellwing() {
    let world = plain(16, 20, 8);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let (x, z) = centre(4, 4);
    let flying = aloft(&mut fauna, &world, x, z, SURFACE + 1.2, EAST, StartingStores::FULL);
    let perched = on_ground(&mut fauna, &world, BW, 10, 4, StartingStores::FULL);
    fly(&mut fauna, flying, 0.0, 0.0, 0.0);
    fly(&mut fauna, perched, 0.0, 0.0, 0.0);
    steps(&mut fauna, &world, &mut flora, 5);
    assert_eq!(mode(&fauna, flying), MobilityMode::Fly);
    assert_eq!(mode(&fauna, perched), MobilityMode::Perched);
    assert_eq!(perch_kind(&fauna, perched), Some(PerchKind::Face));

    let back = Fauna::load(&fauna.save()).expect("the layer round-trips");
    assert_eq!(back.view().animals, fauna.view().animals, "the same bodies");
    let a = *back.view().animal(flying).expect("the flyer");
    assert!((a.pose.y_m - (SURFACE + 1.2)).abs() < 1e-9, "its height: {}", a.pose.y_m);
    assert_eq!(a.mobility.mode, MobilityMode::Fly);
    assert_eq!(back.view().animal(perched).expect("the percher").mobility.perch, animal(&fauna, perched).mobility.perch);
    assert_eq!(back.view().ledger, fauna.view().ledger, "the same ledger");
}

/// **Nectar into the stomach** (the minimal gut; Astra's finding 8): a bellwing with an
/// empty stomach sips a bloomcrown; on that tick its nectar pool holds exactly what the
/// stand gave, in all three currencies.
#[test]
fn a_bloomcrown_sip_lands_in_the_stomach_whole() {
    let mut world = plain_with(config(16, 20, 8, clock_at(12.0)));
    let mut flora = Flora::new(nectar_flora());
    let site = blooming(&mut flora, &mut world, 8, 4, 60);
    let top = crown_top(&flora, site);
    let mut c = FaunaConfig::default();
    c.founders[BW.index()].gut.s0 = 0.0;
    let mut fauna = Fauna::new(c);
    let (x, z) = centre(8, 4);
    let id = aloft(&mut fauna, &world, x, z, top + 0.3, EAST, StartingStores::HUNGRY);
    fly(&mut fauna, id, 0.0, 1.0, -1.0);
    let consumed = flora.view().ledger.consumed_organic_out;
    let mut t = 0;
    while fauna.view().ledger.bites == 0 && t < 80 {
        fauna.step(&world, &mut flora);
        t += 1;
    }
    let l = fauna.view().ledger;
    assert_eq!(l.bites, 1, "one sip");
    let pool = animal(&fauna, id).stomach.pools[Food::Nectar.index()];
    assert_eq!(pool.organic, l.eaten_organic_in);
    assert_eq!(pool.mineral, l.eaten_mineral_in);
    assert_eq!(pool.energy, l.eaten_energy_in);
    assert!(pool.mineral > 0.0, "bloomcrown nectar carries mineral");
    assert!((flora.view().ledger.consumed_organic_out - consumed - pool.organic).abs() <= 1e-15);
}

/// **A real vine sip** (the minimal gut's repair 3 items 1–2; Astra's finding 2): a
/// bellwing sips a flowering latticevine whose store holds mineral; the nectar it swallows
/// carries mineral, and with feeding then stopped the pool digests into the reserve rather
/// than holding.
#[test]
fn a_vine_sip_carries_mineral_and_digests() {
    let (mut sky, mut flora, faces) = vine_wall(true, flowering_vines());
    let mut rain = {
        let mut c = sky.config().clone();
        c.rain_m_per_s = 1e-6;
        let mut wet = World::empty(c);
        soil(&mut wet);
        rock(&mut wet, 6..=6, 1..=12);
        wet
    };
    flora.step(&mut rain);
    let flowering = |flora: &Flora| {
        faces.iter().copied().find(|&f| {
            flora.view().cover.face(f).is_some_and(|c| c.spur == SpurPhase::Flower && c.nectar > 0.0)
        })
    };
    let mut t = 0;
    while flowering(&flora).is_none() && t < 160 {
        flora.step(&mut sky);
        t += 1;
    }
    let face = flowering(&flora).expect("fixture: a face flowers");
    let mut c = FaunaConfig::default();
    c.founders[BW.index()].gut.s0 = 0.0;
    c.founders[BW.index()].core.maintenance_per_s = 0.0;
    let mut fauna = Fauna::new(c);
    let r = bellwing_flight(&fauna).span_m / 2.0;
    let (_, z) = centre(0, WALL_Z);
    let y = f64::from(face.y) * V + 0.05;
    let id = aloft(&mut fauna, &sky, WALL_FACE_M - r - 0.005, z, y, EAST, StartingStores::HUNGRY);
    fly(&mut fauna, id, 0.0, 1.0, 0.0);
    let mut t = 0;
    while fauna.view().ledger.bites == 0 && t < 40 {
        fauna.step(&sky, &mut flora);
        t += 1;
    }
    assert!(fauna.view().ledger.bites >= 1, "a vine sip");
    let pool = animal(&fauna, id).stomach.pools[Food::Nectar.index()];
    assert!(pool.mineral > 0.0, "vine nectar carries mineral: {pool:?}");
    fly(&mut fauna, id, 0.0, 0.0, 0.0);
    let r0 = animal(&fauna, id).reserve;
    steps(&mut fauna, &sky, &mut flora, 40);
    let a = animal(&fauna, id);
    assert!(a.stomach.pools[Food::Nectar.index()].organic < pool.organic, "the pool digests");
    assert!(a.reserve > r0, "into the reserve");
}

/// **The hover-sip teacher climbs a step it can clear** (`design/handoffs/
/// bellwing-finding-2026-10-03.md` item 1): half the ring is a rock step 1.5 m over the
/// plain — taller than the teacher's search height (0.4 of the 2.5 m band, 1 m) but well
/// inside the band. A hungry bellwing searching over the plain, with nothing in its fan,
/// meets the step's wall whichever way it goes round the ring; it must get over it, not
/// turn away from it for good: within 60 s it flies over the step (its support face is
/// the step's top). The terrarium's flowers stand on terraces 4 m over the strip the
/// bellwings were confined to.
#[test]
fn the_hover_sip_teacher_climbs_over_a_step_inside_its_band() {
    let mut world = plain(48, 24, 6);
    rock(&mut world, 24..=47, 3..=8);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let (x, z) = centre(8, 3);
    // A full reserve (it can pay its takeoffs) and an empty stomach (its eating latch on).
    let id = aloft(&mut fauna, &world, x, z, SURFACE + 1.0, EAST, StartingStores::FULL);
    let teachers = fauna.config().teachers;
    assert!(fauna.set_controller(id, crate::controller::heuristic(BW, teachers)));
    let over = until(&mut fauna, &world, &mut flora, 1200, |f| {
        f.view().animal(id).is_some_and(|a| a.site.y == 8 && a.mobility.mode == MobilityMode::Fly)
    });
    assert!(animal(&fauna, id).eating, "the bellwing is hungry, so it searches");
    assert!(over < 1200, "the bellwing flew over the 1.5 m step: it is at {:?}", animal(&fauna, id).pose);
}

/// **A hatchling under low cover gets off the floor** (`design/handoffs/
/// bellwing-finding-2026-10-03.md` follow-up 2): a bellwing hatches in the air at its
/// face's top, and eggs are laid under the parent's perch — often an overhang. Here it
/// hatches in a hollow ringed by a one-voxel lip, under a rock roof 0.75 m over its face
/// (cover inside the teacher's `bellwing_roost_ceiling`, so it never climbs toward it). In
/// the terrarium every starved hatchling (14 of 14 in seed 5) hung at its hatching face
/// for its whole life, turning in place, base on the floor, never sipping. It must rise
/// off the floor under the roof and leave the hollow: within 30 s its column is outside
/// the lip.
#[test]
fn a_hatchling_under_low_cover_rises_off_the_floor_and_leaves_its_hollow() {
    let mut world = plain(32, 20, 9);
    // The lip: rock at y = 3 on the ring of columns around (8..=10, 3..=5).
    for z in 2..=6u32 {
        for x in 7..=11i64 {
            if (8..=10).contains(&x) && (3..=5).contains(&z) {
                continue;
            }
            world.apply(WorldCommand::SetMaterial { x, y: 3, z, material: Material::Rock });
        }
    }
    // The roof: rock at y = 6 over columns 4..=14 (its underside 0.75 m over the face).
    for z in 0..9u32 {
        for x in 4..=14i64 {
            world.apply(WorldCommand::SetMaterial { x, y: 6, z, material: Material::Rock });
        }
    }
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let (x, z) = centre(9, 4);
    // A hatchling's stores: body_min (a quarter of body_max) and its egg's remainder.
    let id = aloft(&mut fauna, &world, x, z, SURFACE, EAST, StartingStores { body: 0.25, reserve: 0.5 });
    let teachers = fauna.config().teachers;
    assert!(fauna.set_controller(id, crate::controller::heuristic(BW, teachers)));
    let out = until(&mut fauna, &world, &mut flora, 600, |f| {
        f.view().animal(id).is_some_and(|a| !(7..=11).contains(&a.site.x) || !(2..=6).contains(&a.site.z))
    });
    let a = animal(&fauna, id);
    assert!(out < 600, "the hatchling left its hollow: it is at {:?}, {:?}", a.pose, a.mobility.mode);
}

/// **No egg under a low perch** (`design/handoffs/bellwing-finding-2026-10-03.md`,
/// bellwing3 item 2): a bellwing's egg goes on the face below it, and its hatchling
/// appears there in the air. Three bellwings ready to lay, each over a littered face:
/// - **low:** hanging under a rock roof whose underside is 0.25 m over the face — never
///   lays (the terrarium's starved hatchlings hatched into such slots and never left);
/// - **boxed:** in a closed rock box with 1.75 m of headroom — never lays (its air does
///   not reach the open air);
/// - **open:** over a face in the open — lays.
#[test]
fn a_bellwing_never_lays_under_a_low_perch_or_in_closed_air() {
    let mut world = plain_with(config(40, 20, 9, clock_at(12.0)));
    // The low roof: rock at y = 4 over columns 2..=8 (underside 1.0 m, 0.25 m over the face).
    for z in 0..9u32 {
        for x in 2..=8i64 {
            world.apply(WorldCommand::SetMaterial { x, y: 4, z, material: Material::Rock });
        }
    }
    // The box: walls at columns 14 and 22 and rows z = 0 and z = 8 up to y = 10, a lid at
    // y = 10 (interior faces y = 2, headroom 1.75 m: rows 3..=9).
    for z in 0..9u32 {
        for x in 14..=22i64 {
            for y in 3..=10u32 {
                let wall = x == 14 || x == 22 || z == 0 || z == 8 || y == 10;
                if wall {
                    world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Rock });
                }
            }
        }
    }
    let mut flora = Flora::new(FloraConfig::default());
    let (low, boxed, open) = (Site { x: 5, y: 2, z: 4 }, Site { x: 18, y: 2, z: 4 }, Site { x: 30, y: 2, z: 4 });
    for s in [low, boxed, open] {
        assert!(flora.deposit(
            s,
            cubarium_voxel_flora::Deposit { kind: cubarium_voxel_flora::DepositKind::Litter, organic: 0.2, mineral: 0.004, energy: 0.4 }
        ));
    }
    let mut config = FaunaConfig::default();
    {
        let core = &mut config.founders[BW.index()].core;
        core.reserve_cap = 1.0;
        core.reproduction = crate::Reproduction {
            surplus_floor: 0.0,
            surplus_hold_s: DT,
            birth_interval_s: 20.0 * DT,
            clutch_size: 1,
            egg_organic: core.body_min,
            incubation_s: 1000.0 * DT,
            litter_min: 0.0,
            ..core.reproduction
        };
        assert_eq!(core.reproduction.mode, crate::BirthMode::Eggs, "an egg-layer");
    }
    // The flyer's egg room is the rule under test, on litter: VINE-C put the bellwing's own
    // nest in the vines (its vine nest asks the same room, `vine_nest_rooms` below).
    config.founders[BW.index()].egg_site = crate::EggSite::Litter;
    let mut fauna = Fauna::new(config);
    let mut ids = Vec::new();
    for (s, y) in [(low, 1.0 - 0.1), (boxed, SURFACE + 0.5), (open, SURFACE + 0.5)] {
        let (x, z) = centre(s.x, s.z);
        let id = aloft(&mut fauna, &world, x, z, y, EAST, StartingStores::FULL);
        // The low one rises into its roof and hangs there; the others hover.
        fly(&mut fauna, id, 0.0, 0.0, if s == low { 1.0 } else { 0.0 });
        ids.push(id);
    }
    steps(&mut fauna, &world, &mut flora, 40);
    assert_eq!(perch_kind(&fauna, ids[0]), Some(PerchKind::Underside), "the low one hangs under its roof");
    let laid: Vec<Site> = fauna.view().clutches.iter().filter(|c| c.lineage == BW).map(|c| c.site).collect();
    assert!(laid.contains(&open), "the open one lays: {laid:?}");
    assert!(!laid.contains(&low), "no egg under the low roof: {laid:?}");
    assert!(!laid.contains(&boxed), "no egg in the closed box: {laid:?}");
}

// ------------------------------------------------------------------ burrs

/// **Burrs and flight** (springturf spread): a bellwing aloft is no burr carrier on the
/// ground, and the same lineage on a ground face is. Carried through the plant layer's
/// burr pass, at a pickup of 1 and a coat of 1, the grounded one picks up a springturf
/// seed stepping onto a donor's face and the flying one, crossing the same faces, does not.
#[test]
fn a_grounded_bellwing_picks_up_burrs_and_one_in_flight_does_not() {
    let world = plain(8, 12, 3);
    let mut fconfig = FaunaConfig::default();
    fconfig.founders[BW.index()].burr_coat = 1.0;
    let mut fauna = Fauna::new(fconfig);
    let (x, z) = centre(2, 1);
    let up = aloft(&mut fauna, &world, x, z, SURFACE + 1.0, EAST, StartingStores::FULL);
    let down = on_ground(&mut fauna, &world, BW, 2, 1, StartingStores::FULL);
    let flying = animal(&fauna, up);
    let walking = animal(&fauna, down);
    assert_eq!(flying.mobility.mode, MobilityMode::Fly);
    assert!(!crate::step::grounded(&flying), "aloft is not grounded");
    assert!(crate::step::grounded(&walking), "on its face it is");

    let mut pconfig = FloraConfig::default();
    pconfig.springturf.burr_pickup = 1.0;
    for (body, expect) in [(flying, false), (walking, true)] {
        let mut flora = Flora::new(pconfig.clone());
        for x in 0..8u32 {
            let site = Site { x, y: 2, z: 1 };
            let wood = pconfig.springturf.wood_max;
            assert!(flora.apply(&world, FloraCommand::SeedOnFace { site, species: Plant::Springturf, wood }));
        }
        let mut c = crate::step::burr_carrier(&fauna, &body);
        assert_eq!(c.coat, 1.0, "the lineage's coat");
        flora.carry_burrs(&world, &[c]);
        c.site.x += 1;
        c.x_m += V;
        flora.carry_burrs(&world, &[c]);
        let picked = flora.view().ledger.burrs_picked[Plant::Springturf.index()];
        assert_eq!(picked > 0, expect, "grounded {}", c.grounded);
    }
}

/// **BELLWING: a clutch from surplus** — three hovering bellwings over open litter, their
/// reserves one and a half, two and a half and six and a half eggs above the floor, with
/// `clutch_max` 3 (a test value) and no wait: each lays once, and the more surplus the more
/// eggs, up to `clutch_max` — the first lays fewer than the second, the third exactly
/// `clutch_max` though it could pay for more. Control: with `clutch_max` 0 each lays the
/// fixed `clutch_size`, whatever its surplus.
#[test]
fn a_clutch_from_surplus_lays_more_eggs_the_more_surplus_up_to_clutch_max() {
    let world = plain_with(config(40, 20, 9, clock_at(12.0)));
    let sites = [Site { x: 8, y: 2, z: 4 }, Site { x: 20, y: 2, z: 4 }, Site { x: 32, y: 2, z: 4 }];
    let lay = |clutch_max: u32| -> Vec<u32> {
        let mut flora = Flora::new(FloraConfig::default());
        for s in sites {
            assert!(flora.deposit(
                s,
                cubarium_voxel_flora::Deposit { kind: cubarium_voxel_flora::DepositKind::Litter, organic: 0.2, mineral: 0.004, energy: 0.4 }
            ));
        }
        let mut config = FaunaConfig::default();
        let (egg, floor, cap) = {
            let core = &mut config.founders[BW.index()].core;
            core.reserve_cap = 4.0;
            core.reproduction = crate::Reproduction {
                surplus_floor: 0.001,
                surplus_hold_s: DT,
                birth_interval_s: 1e6,
                clutch_size: 1,
                clutch_max,
                clutch_hold_max_s: 0.0,
                egg_organic: core.body_min,
                incubation_s: 1e6,
                litter_min: 0.0,
                brood_share: 0.0,
                ..core.reproduction
            };
            (core.body_min, 0.001, core.reserve_of(core.body_max))
        };
        // The clutch's size is under test, on litter (VINE-C nests the bellwing in vines).
        config.founders[BW.index()].egg_site = crate::EggSite::Litter;
        let mut fauna = Fauna::new(config);
        let mut ids = Vec::new();
        for (s, eggs_above) in sites.iter().zip([1.5, 2.5, 6.5]) {
            let (x, z) = centre(s.x, s.z);
            let reserve = (floor + eggs_above * egg) / cap;
            assert!(reserve < 1.0, "fixture: the reserve fits the cap");
            let id = aloft(&mut fauna, &world, x, z, SURFACE + 0.5, EAST, StartingStores { body: 1.0, reserve });
            fly(&mut fauna, id, 0.0, 0.0, 0.0);
            ids.push(id);
        }
        steps(&mut fauna, &world, &mut flora, 40);
        sites.iter().map(|s| fauna.view().clutches.iter().filter(|c| c.lineage == BW && c.site == *s).map(|c| c.count).sum()).collect()
    };
    let max = 3;
    let eggs = lay(max);
    assert!(eggs[0] >= 1, "the least surplus still lays: {eggs:?}");
    assert!(eggs[0] < eggs[1], "more surplus, more eggs: {eggs:?}");
    assert_eq!(eggs[2], max, "capped at clutch_max: {eggs:?}");
    let fixed = lay(0);
    assert_eq!(fixed, vec![1, 1, 1], "control: the fixed clutch_size whatever the surplus");
}

/// A mature tree exposes lower leaves, while a mouth cannot strip its high canopy.
#[test]
fn a_mature_vaulttrees_lower_foliage_is_browsed_while_its_upper_foliage_stays() {
    let world = plain(40, 64, 16);
    let mut cfg = FaunaConfig::default();
    cfg.founders[Founder::Loftstrider.index()]
        .core
        .bite_half_stock = 0.0;
    let body = cfg.founder(Founder::Loftstrider).adult_body();
    let fc = FloraConfig::default();
    let mut flora = Flora::new(fc);
    let site = full_stand(&mut flora, &world, Plant::Vaulttree, 20, 8);
    let before = foliage_layers(&flora, site);
    let band = body.mouth_layers(site.y, V);
    assert!(
        before
            .iter()
            .any(|l| band.contains(&l.cell) && l.edible() > 0.0),
        "fixture: stocked lower canopy within reach"
    );
    assert!(
        before
            .iter()
            .any(|l| !band.contains(&l.cell) && l.stock > 0.0),
        "fixture: stocked upper canopy above reach"
    );
    let mut fauna = Fauna::new(cfg);
    let id = on_ground(
        &mut fauna,
        &world,
        Founder::Loftstrider,
        16,
        8,
        StartingStores {
            body: 1.0,
            reserve: 0.0,
        },
    );
    fly(&mut fauna, id, 0.0, 1.0, 0.0);
    steps(&mut fauna, &world, &mut flora, 12);
    assert!(fauna.view().ledger.bites > 0, "it actually browsed");
    let after = foliage_layers(&flora, site);
    assert!(
        before
            .iter()
            .zip(&after)
            .any(|(a, b)| band.contains(&a.cell) && b.stock < a.stock),
        "reachable leaves were taken"
    );
    for (a, b) in before.iter().zip(&after) {
        if !band.contains(&a.cell) {
            assert!((a.stock - b.stock).abs() <= 1e-12, "upper layer {} stays untouched", a.cell);
        }
    }
}

/// A hungry bellwing follows the flower cue all the way into sipping range.
#[test]
fn a_bellwing_approaches_a_flowering_vine_and_books_a_visit() {
    let (mut sky, mut flora, faces) = vine_wall(true, flowering_vines());
    let mut rain = {
        let mut c = sky.config().clone();
        c.rain_m_per_s = 1e-6;
        let mut wet = World::empty(c);
        soil(&mut wet);
        rock(&mut wet, 6..=6, 1..=12);
        wet
    };
    flora.step(&mut rain);
    let mut face = None;
    for _ in 0..160 {
        flora.step(&mut sky);
        face = faces.iter().copied().find(|&f| {
            flora
                .view()
                .cover
                .face(f)
                .is_some_and(|c| c.spur == SpurPhase::Flower && c.nectar > 0.0)
        });
        if face.is_some() {
            break;
        }
    }
    let face = face.expect("fixture: a face flowers after rain");
    let mut fauna = Fauna::new(FaunaConfig::default());
    let (_, z) = centre(0, WALL_Z);
    let y = (f64::from(face.y) + 0.5) * V;
    let r = bellwing_flight(&fauna).span_m / 2.0;
    let id = aloft(
        &mut fauna,
        &sky,
        WALL_FACE_M - r - 2.0 * V,
        z,
        y,
        EAST,
        StartingStores::HUNGRY,
    );
    let teachers = fauna.config().teachers;
    assert!(fauna.set_controller(id, crate::controller::heuristic(BW, teachers)));
    let n = until(&mut fauna, &sky, &mut flora, 200, |f| {
        f.view().ledger.bites_by_founder_food[BW.index()][Food::Nectar.index()] > 0
    });
    assert!(
        fauna.view().ledger.bites_by_founder_food[BW.index()][Food::Nectar.index()] > 0,
        "it reaches a flower and sips within {n} ticks: {:?}",
        animal(&fauna, id).pose
    );
    assert!(
        flora.view().ledger.flower_visits[1] > 0,
        "arrival is booked to the vine"
    );
    let Bloom::Vine(visited) = animal(&fauna, id).last_bloom.expect("the bloom it sipped") else {
        panic!("this fixture contains only vine flowers");
    };
    assert!(
        flora.view().cover.face(visited).unwrap().visits > 0,
        "the particular face receives the visit"
    );
}


// ------------------------------------------------------------------ VINE-C

/// The distance from `p` to the square of side face `f` (`x` unwrapped near `p`), metres.
fn to_face(p: (f64, f64, f64), f: Face) -> f64 {
    let (bx, by, bz) = (f64::from(f.x) * V, f64::from(f.y) * V, f64::from(f.z) * V);
    let (xr, zr) = match f.dir {
        FaceDir::NegX => ((bx, bx), (bz, bz + V)),
        FaceDir::PosX => ((bx + V, bx + V), (bz, bz + V)),
        FaceDir::NegZ => ((bx, bx + V), (bz, bz)),
        FaceDir::PosZ => ((bx, bx + V), (bz + V, bz + V)),
        FaceDir::Down => ((bx, bx + V), (bz, bz + V)),
    };
    let d = |q: f64, (lo, hi): (f64, f64)| (lo - q).max(0.0).max(q - hi);
    let (dx, dy, dz) = (d(p.0, xr), d(p.1, (by, by + V)), d(p.2, zr));
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// [`vine_wall`]'s wall with one vine covering its west faces at rows 3..=10 across every
/// `z`: a sheet of vine, under `config`. The covered faces.
fn vine_sheet(config: FloraConfig) -> (World, Flora, Vec<Face>) {
    let mut world = plain_with(config_noon_or_off(false));
    rock(&mut world, 6..=6, 1..=12);
    let mut flora = Flora::new(config);
    let first = Face::new(6, 3, WALL_Z, FaceDir::NegX);
    let id = flora
        .seed_vine(&world, VineSeed { root: Site { x: 5, y: 2, z: WALL_Z }, first, reserve: 100.0, lineage: None })
        .expect("fixture: the vine founder");
    let mut faces = vec![first];
    let depth = world.config().depth;
    // Outward from the founder's strip, row by row, so each face joins a covered one.
    let mut zs: Vec<u32> = (0..depth).collect();
    zs.sort_by_key(|&z| z.abs_diff(WALL_Z));
    for y in 3..=10 {
        for &z in &zs {
            let f = Face::new(6, y, z, FaceDir::NegX);
            if f != first {
                assert!(flora.cover_face(&world, id, f, 1.0), "fixture: cover {f:?}");
                faces.push(f);
            }
        }
    }
    (world, flora, faces)
}

/// The vine wall's world: 16 × 20 × 5 at 0.25 m, the clock off (or at noon).
fn config_noon_or_off(noon: bool) -> VoxelConfig {
    config(16, 20, 5, if noon { clock_at(12.0) } else { clock_off() })
}

/// **VINE-C (Wrysk 2026-10-09: one visit pollinates every open vine flower within its body
/// span): the visit reaches the flowers the wings brush.** A sheet of vine in flower, every
/// face holding nectar; a hungry bellwing hovers sipping at it. At its first sip every open
/// flower within half its span (its wing disc) of where its mouth meets the wall is
/// visited, so is every face it drank from (within its mouth's reach), more than one face
/// in all; a flower beyond both is not; the visits booked are the faces visited.
#[test]
fn one_visit_pollinates_the_open_vine_flowers_within_its_span_and_none_beyond() {
    let (world, mut flora, faces) = vine_sheet(flowering_vines());
    for &f in &faces {
        assert!(flora.set_spur(f, SpurPhase::Flower, 1.0e-3), "fixture: a flower on {f:?}");
    }
    let mut fauna = Fauna::new(FaunaConfig::default());
    let r = bellwing_flight(&fauna).span_m / 2.0;
    let (_, z) = centre(0, WALL_Z);
    let id = aloft(&mut fauna, &world, WALL_FACE_M - r - 0.005, z, 6.0 * V + 0.05, EAST, StartingStores::HUNGRY);
    fly(&mut fauna, id, 0.0, 1.0, 0.0);
    let n = until(&mut fauna, &world, &mut flora, 40, |f| f.view().ledger.bites_by_food[Food::Nectar.index()] >= 1);
    assert_eq!(fauna.view().ledger.bites_by_food[Food::Nectar.index()], 1, "one sip ({n} ticks)");
    let a = animal(&fauna, id);
    let g = fauna.config().founder(BW).body_at(a.body);
    let reach = r + g.mouth_reach_m;
    let p = (a.pose.x, a.pose.y_m + 0.5 * g.height_m.max(1e-6), a.pose.z);
    // Where the mouth meets the flat wall: straight across from it.
    let contact = (WALL_FACE_M, p.1, p.2);
    let mut visited = 0;
    for &f in &faces {
        let v = flora.view().cover.face(f).expect("covered").visits > 0;
        let (span, mouth) = (to_face(contact, f), to_face(p, f));
        if span < r - 1e-9 || mouth < reach - 1e-9 {
            assert!(v, "{f:?} within the span ({span} m of {r}) or the mouth's reach ({mouth} m of {reach}) is visited");
        }
        if span > r + 1e-9 && mouth > reach + 1e-9 {
            assert!(!v, "{f:?} beyond the span ({span} m) and the reach ({mouth} m) is not visited");
        }
        visited += usize::from(v);
    }
    assert!(visited >= 2, "one visit reached more than one flower: {visited}");
    assert_eq!(flora.view().ledger.flower_visits[1], visited as u64, "each visited flower booked once");
}

/// A bellwing config ready to lay at once: a one-tick hold, no clutch wait, no nest brake,
/// incubation `incubation_ticks`.
fn nesting(incubation_ticks: f64) -> FaunaConfig {
    let mut config = FaunaConfig::default();
    let core = &mut config.founders[BW.index()].core;
    core.reproduction = crate::Reproduction {
        surplus_hold_s: DT,
        birth_interval_s: 1000.0 * DT,
        incubation_s: incubation_ticks * DT,
        clutch_hold_max_s: 0.0,
        nest_radius_m: 0.0,
        ..core.reproduction
    };
    assert_eq!(config.founders[BW.index()].egg_site, crate::EggSite::Vine, "the bellwing nests in the vines");
    config
}

/// **P0, the executed-action tap: a bellwing flying to a vine nest records `Steer::Nest`.** A
/// ready bellwing on its own hover-sip heuristic, watched by a [`crate::tap_tests::Spy`],
/// a metre off a vine-covered wall (out of its mouth's reach, within its nest search): its
/// samples while it flies there are the nest reflex's, the executed label is what it held,
/// and the pre-override label is its heuristic's own answer.
#[test]
fn a_bellwing_flying_to_a_vine_nest_records_the_nest() {
    let (world, mut flora, _) = vine_wall(true, FloraConfig::default());
    let mut fauna = Fauna::new(nesting(4000.0));
    let (_, z) = centre(0, WALL_Z);
    let id = aloft(&mut fauna, &world, WALL_FACE_M - 1.0, z, 6.0 * V + 0.05, EAST, StartingStores::FULL);
    let (spy, answers) = crate::tap_tests::Spy::new(crate::heuristic(BW, fauna.config().teachers), BW);
    assert!(fauna.set_controller(id, Box::new(spy)));
    fauna.start_tap();
    let mut nest = 0;
    for _ in 0..200 {
        let before = fauna.tap().and_then(|t| t.stream(id)).map_or(0, <[_]>::len);
        fauna.step(&world, &mut flora);
        let Some(s) = fauna.tap().and_then(|t| t.stream(id)).filter(|s| s.len() > before).and_then(|s| s.last()) else {
            continue;
        };
        let a = animal(&fauna, id);
        let (own, _, _) = *answers.lock().expect("answers").last().expect("its answer");
        assert_eq!(s.own.actions, own, "the pre-override label is its heuristic's own answer");
        assert_eq!(s.executed.actions, a.founder_state.held, "the executed label is what it held");
        assert_eq!(s.executed.body.lift, a.founder_state.lift, "and the lift it held");
        nest += usize::from(s.steer == crate::Steer::Nest);
        if fauna.view().ledger.clutches_by_founder[BW.index()] >= 1 {
            break;
        }
    }
    assert!(nest > 0, "the nest reflex flew it to the vines and the tap saw it");
}

/// **VINE-C (Wrysk 2026-10-09: "bellwings can also nest in the vines"): a bellwing ready to
/// lay lays on the vine it hovers at, and its young hatch clinging there.** A bellwing with
/// a full reserve hovers at a vine-covered wall: within a few ticks it lays one clutch, on a
/// covered face within its mouth's reach, booked as a vine clutch. At term every egg
/// hatches into a bellwing perched on that face, pressed against it. The books close through
/// the laying and the hatching.
#[test]
fn a_ready_bellwing_lays_on_the_vine_and_its_young_hatch_clinging_there() {
    let (world, mut flora, faces) = vine_wall(true, FloraConfig::default());
    let organic = flora.view().organic();
    let mut fauna = Fauna::new(nesting(40.0));
    let r = bellwing_flight(&fauna).span_m / 2.0;
    let (_, z) = centre(0, WALL_Z);
    let id = aloft(&mut fauna, &world, WALL_FACE_M - r - 0.005, z, 6.0 * V + 0.05, EAST, StartingStores::FULL);
    fly(&mut fauna, id, 0.0, 0.0, 0.0);
    let n = until(&mut fauna, &world, &mut flora, 20, |f| f.view().ledger.clutches_by_founder[BW.index()] >= 1);
    let l = &fauna.view().ledger;
    assert_eq!(l.clutches_by_founder[BW.index()], 1, "a clutch within 20 ticks ({n})");
    assert_eq!(l.vine_clutches_by_founder[BW.index()], 1, "booked as a vine clutch");
    let c = *fauna.view().clutches.iter().find(|c| c.lineage == BW).expect("the clutch");
    let face = c.face.expect("the clutch lies on a vine face");
    assert!(faces.contains(&face), "on the covered wall: {face:?}");
    let a = animal(&fauna, id);
    let g = fauna.config().founder(BW).body_at(a.body);
    let p = (a.pose.x, a.pose.y_m + 0.5 * g.height_m.max(1e-6), a.pose.z);
    assert!(to_face(p, face) <= r + g.mouth_reach_m + 1e-9, "within its mouth's reach");
    ledgers_close(&fauna, &flora, organic);

    let hatched = until(&mut fauna, &world, &mut flora, 80, |f| f.view().ledger.eggs_hatched_by_founder[BW.index()] >= u64::from(c.count));
    assert_eq!(fauna.view().ledger.eggs_hatched_by_founder[BW.index()], u64::from(c.count), "every egg hatched ({hatched} ticks)");
    let young: Vec<Animal> = fauna.view().animals.iter().filter(|b| b.founder == Some(BW) && b.id != id).copied().collect();
    assert_eq!(young.len(), c.count as usize);
    for b in &young {
        assert_eq!(b.mobility.mode, MobilityMode::Perched, "{:?}", b.mobility);
        assert_eq!(b.mobility.perch.map(|p| p.anchor), Some(crate::PerchAnchor::Vine(face)), "clinging to the nest face");
        let gap = WALL_FACE_M - b.pose.x;
        assert!(gap >= r - 1e-9 && gap <= r + crate::flight::CLING_M, "pressed against the wall: {gap} m from it");
        assert_eq!(b.pose.y_m.div_euclid(V) as u32, face.y, "at the face's row");
    }
    ledgers_close(&fauna, &flora, organic);
}

/// **VINE-C ("they cant nest on bloomcrowns"): with bloomcrowns near and no vine, a ready
/// bellwing lays nothing.** A bellwing with a full reserve hovers over a full-grown
/// bloomcrown's crown, and another perches on its crown top, over littered ground that the
/// old litter rule would have taken: in 60 ticks neither lays and both keep their reserve
/// for it. A third hovers in the open over the littered plain: nothing either.
#[test]
fn with_bloomcrowns_near_and_no_vine_a_ready_bellwing_lays_nothing() {
    let mut world = plain_with(config(16, 20, 9, clock_at(12.0)));
    let mut flora = Flora::new(FloraConfig::default());
    let site = full_stand(&mut flora, &world, Plant::Bloomcrown, 8, 4);
    for s in [site, Site { x: 3, y: 2, z: 4 }] {
        assert!(flora.deposit(
            s,
            cubarium_voxel_flora::Deposit { kind: cubarium_voxel_flora::DepositKind::Litter, organic: 0.2, mineral: 0.004, energy: 0.4 }
        ));
    }
    let _ = &mut world;
    let mut fauna = Fauna::new(nesting(40.0));
    let top = crown_top(&flora, site);
    let (x, z) = centre(8, 4);
    let over = aloft(&mut fauna, &world, x, z, top + 0.3, EAST, StartingStores::FULL);
    let on = aloft(&mut fauna, &world, x, z, top, EAST, StartingStores::FULL);
    let (ox, oz) = centre(3, 4);
    let open = aloft(&mut fauna, &world, ox, oz, SURFACE + 0.5, EAST, StartingStores::FULL);
    for id in [over, on, open] {
        fly(&mut fauna, id, 0.0, 0.0, 0.0);
    }
    steps(&mut fauna, &world, &mut flora, 60);
    let l = &fauna.view().ledger;
    assert_eq!(l.clutches_by_founder[BW.index()], 0, "no clutch on a bloomcrown or the ground");
    assert!(fauna.view().clutches.is_empty());
    for id in [over, on, open] {
        assert_eq!(animal(&fauna, id).reproduction.given, 0.0, "nothing given to eggs");
    }
}

/// **VINE-C: a vine nest asks the same room as any flyer's egg.** The same vine wall with a
/// rock roof a quarter metre over the hovering bellwing's row: the hatchling would cling
/// with no room to fly from, so it lays nothing there.
#[test]
fn vine_nest_rooms() {
    let (mut world, mut flora, _) = vine_wall(true, FloraConfig::default());
    // A roof over the columns in front of the wall at y = 8 (the face rows 3..=7 under it).
    let depth = world.config().depth;
    for z in 0..depth {
        for x in 0..=5i64 {
            world.apply(WorldCommand::SetMaterial { x, y: 8, z, material: Material::Rock });
        }
    }
    let mut fauna = Fauna::new(nesting(40.0));
    let r = bellwing_flight(&fauna).span_m / 2.0;
    let (_, z) = centre(0, WALL_Z);
    let id = aloft(&mut fauna, &world, WALL_FACE_M - r - 0.005, z, 6.0 * V + 0.05, EAST, StartingStores::FULL);
    fly(&mut fauna, id, 0.0, 0.0, 0.0);
    steps(&mut fauna, &world, &mut flora, 20);
    assert_eq!(fauna.view().ledger.clutches_by_founder[BW.index()], 0, "no room under the roof: no clutch");
}

/// **VINE-C: a ready bellwing out of reach of any vine looks for one and lays there.** On its
/// own heuristic, a bellwing with a full reserve hovering 1.5 m from a vine-covered wall
/// (inside its [`crate::Flight::nest_search_m`]) flies to the wall and lays its clutch on a
/// covered face within 300 ticks, its nest search booked.
#[test]
fn a_ready_bellwing_flies_to_a_vine_it_can_find_and_lays_there() {
    let (world, mut flora, faces) = vine_wall(true, FloraConfig::default());
    let mut fauna = Fauna::new(nesting(1e6));
    assert!(fauna.config().founder(BW).flight.expect("a flyer").nest_search_m > 1.5 + 0.5, "fixture: the wall is in range");
    let r = bellwing_flight(&fauna).span_m / 2.0;
    let (_, z) = centre(0, WALL_Z);
    let id = aloft(&mut fauna, &world, WALL_FACE_M - r - 1.5, z, 6.0 * V + 0.05, EAST, StartingStores::FULL);
    let teachers = fauna.config().teachers;
    assert!(fauna.set_controller(id, crate::controller::heuristic(BW, teachers)));
    let n = until(&mut fauna, &world, &mut flora, 300, |f| f.view().ledger.clutches_by_founder[BW.index()] >= 1);
    assert_eq!(fauna.view().ledger.clutches_by_founder[BW.index()], 1, "it found the vine and laid ({n} ticks), at {:?}", animal(&fauna, id).pose);
    let face = fauna.view().clutches[0].face.expect("in the vines");
    assert!(faces.contains(&face));
    assert!(fauna.view().ledger.nest_searches_by_founder[BW.index()] > 0, "it searched");
}
