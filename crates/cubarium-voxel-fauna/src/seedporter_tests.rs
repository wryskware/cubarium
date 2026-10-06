//! **F3, written before the rule** (`design/handoffs/voxel-roster-2026-10-01.md` Brief F3
//! "Tests first"; the design note is `design/predation-and-flight-2026-09-28.md` §3.5,
//! §3.7, §3.9): the seedporter — it climbs the stems of stands at least 1 m tall and
//! latticevine-covered faces, perches in crowns, glides down from height, eats fruit
//! (latticevine beads and the lanternberry's new fruit stock) and carries the seeds in its
//! gut to the face it stands on when they are voided. It builds on F1's `y_m`, modes and
//! perches.
//!
//! In-crate because the cone claim reads the observation vector (`body::observation`) and
//! the prey claim builds bodies in a mode, both crate-private.
//!
//! Fixtures are hand-built at the 0.25 m reference voxel: soil at `y = 1..=2`, so a plain
//! column's support face is `y = 2` and its top, the **surface**, is 0.75 m. Stands are
//! seeded full-grown: a lanternberry at `wood_max` is 1.125 m tall
//! (`crown_height_m_at`), its top foliage cell `y = 7`, so its crown top is 2.0 m; one at
//! 0.7 of `wood_max` is 0.75 m tall. Bodies are driven by [`Scripted`]. The day clock is
//! off unless fruit must grow, which needs noon, so the diurnal seedporter is awake. The
//! world is never stepped by the fauna; plants are stepped only to grow a fixture's fruit
//! or flowers. No run is longer than 200 ticks.
//!
//! Readings these tests fix where the brief leaves a choice (each named again where it is
//! used):
//! - **[`Command::IntroduceAloft`] takes a glider**: a pose whose base meets a crown top
//!   is perched on that stand; a pose in the air (at least `glide_min_height_m` over the
//!   landing surface) is gliding.
//! - **A perched seedporter's mouth reaches the fruit of the crown it perches on.**
//! - **"Climbs a 1 m stem"**: walking into the stem of a stand at least
//!   `climb_min_stand_m` tall it climbs ([`MobilityMode::Climb`]) and perches on that
//!   stand at least 1 m over the surface; the climb's vertical pace is
//!   `climb_speed_fraction` (0.5) of its walking pace and a second of it costs
//!   `wall_climb_cost_factor` × a second's walking cost beyond upkeep.
//! - **A glide starts from any height** at least `glide_min_height_m` over the landing
//!   surface: walking off a ledge that high glides; off a lower ledge it does not. Glide
//!   cost `glide_cost_factor` is the whole cost, upkeep included (F1's hover convention).
//! - **A glider that lands on the ground** is no longer gliding: `Ground` or `Perched`.
//! - **Gut seeds** go to the seed bank of the body's support face at the time they are
//!   voided, each meal's seeds no sooner than `gut_transit_s` after it.

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_flora::{
    Command as FloraCommand, Face, FaceDir, Flora, FloraConfig, Site, Species as Plant, SpurPhase,
    VineConfig, VineSeed,
};

use crate::{
    Actions, Animal, Command, Diet, Fauna, FaunaConfig, Food, Founder, FounderPhysiology,
    Founding, FoundingSite, GutSeeds, Hunter, MobilityMode, PerchAnchor, Pose, Scripted,
    StartingStores,
};

const V: f64 = 0.25;
const SP: Founder = Founder::Seedporter;
/// The top of a plain column's support face `y = 2`.
const SURFACE: f64 = 0.75;
/// Facing `+x`: `Pose::forward` is `(sin h, cos h)`.
const EAST: f64 = std::f64::consts::FRAC_PI_2;
/// Hungry enough to eat, an adult's body.
const HUNGRY_ADULT: StartingStores = StartingStores {
    body: 1.0,
    reserve: 0.3,
};
/// Fast fruit for a fixture: 1e-3 a second up to 2e-3 (full in 2 s of daylight), never
/// falling inside a test. Explicit, so the implementer's defaults change none of these.
const FRUIT_RATE: f64 = 1e-3;
const FRUIT_CAP: f64 = 2e-3;

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

/// Soil at `y = 1..=2` everywhere, every soil voxel at field capacity.
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

/// A flat plain with the clock held at noon.
fn noon_plain(width: u32, height: u32, depth: u32) -> World {
    let w = plain_with(config(width, height, depth, clock_at(12.0)));
    assert!(w.life_daylight() > 0.5, "fixture: noon");
    w
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

/// The top of support face `y`, metres.
fn top_of(y: u32) -> f64 {
    f64::from(y + 1) * V
}

fn glider(fauna: &Fauna) -> crate::Glider {
    fauna
        .config()
        .founder(SP)
        .glider
        .expect("the seedporter has a glider table (F3)")
}

/// A flora config whose lanternberries hang fruit fast ([`FRUIT_RATE`] up to
/// [`FRUIT_CAP`], never dropping, a little mineral) and make no seed parcel of their own,
/// so every lanternberry seed on the ground came out of a gut.
fn fruit_flora() -> FloraConfig {
    let mut c = FloraConfig::default();
    c.lanternberry.fruit_per_s = FRUIT_RATE;
    c.lanternberry.fruit_cap = FRUIT_CAP;
    c.lanternberry.fruit_drop_s = 1e9;
    c.lanternberry.fruit_mineral_fraction = 0.01;
    c.lanternberry.propagule_rate = 0.0;
    c
}

/// The same lanternberries with no fruit at all: plain foliage.
fn barren_flora() -> FloraConfig {
    let mut c = fruit_flora();
    c.lanternberry.fruit_per_s = 0.0;
    c
}

/// Seed a stand of `species` with `wood` on column `(x, z)`; returns its site.
fn stand(flora: &mut Flora, world: &World, species: Plant, x: i64, z: u32, wood: f64) -> Site {
    assert!(flora.apply(world, FloraCommand::Seed { x, z, species, wood }));
    flora
        .view()
        .stands
        .iter()
        .find(|s| i64::from(s.site.x) == x && s.site.z == z)
        .expect("the stand")
        .site
}

/// A full-grown stand.
fn full_stand(flora: &mut Flora, world: &World, species: Plant, x: i64, z: u32) -> Site {
    let wood = flora.config().species(species).wood_max;
    stand(flora, world, species, x, z, wood)
}

/// The top of a stand's crown slab: the top of its highest foliage layer's cell.
fn crown_top(flora: &Flora, site: Site) -> f64 {
    let top = flora
        .view()
        .layers_at(site)
        .filter(|l| l.foliage_index.is_some())
        .map(|l| l.cell)
        .max()
        .expect("a crown");
    (top + 1) as f64 * V
}

/// A full-grown lanternberry on `(x, z)` given `ticks` of daylight to hang fruit (the
/// world's clock must be at day); its site.
fn fruiting(flora: &mut Flora, world: &mut World, x: i64, z: u32, ticks: usize) -> Site {
    let site = full_stand(flora, world, Plant::Lanternberry, x, z);
    for _ in 0..ticks {
        flora.step(world);
    }
    site
}

fn stand_fruit(flora: &Flora, site: Site) -> f64 {
    flora.view().stand_at(site).expect("the stand").fruit
}

/// Put a founder on the highest support face of column `(x, z)`, facing east; its id.
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

/// Put a seedporter at exactly `(x_m, y_m, z_m)` facing east through
/// [`Command::IntroduceAloft`]: gliding in the air, perched where its base meets a crown
/// top. Its id.
fn aloft(fauna: &mut Fauna, world: &World, x_m: f64, z_m: f64, y_m: f64, stores: StartingStores) -> u64 {
    assert!(
        fauna.apply(
            world,
            Command::IntroduceAloft {
                pose: Pose {
                    x: x_m,
                    z: z_m,
                    heading_rad: EAST,
                    y_m,
                },
                founder: SP,
                stores,
            },
        ),
        "the fixture places a seedporter at ({x_m}, {y_m}, {z_m})"
    );
    fauna.view().ledger.births - 1
}

/// Hold `forward` and `feed` every tick (no turn).
fn hold(fauna: &mut Fauna, id: u64, forward: f64, feed: f64) {
    assert!(fauna.set_controller(
        id,
        Box::new(Scripted::new(vec![Actions {
            forward,
            turn: 0.0,
            feed,
        }])),
    ));
}

fn animal(fauna: &Fauna, id: u64) -> Animal {
    *fauna.view().animal(id).expect("the body is alive")
}

fn mode(fauna: &Fauna, id: u64) -> MobilityMode {
    animal(fauna, id).mobility.mode
}

/// The stand a body is perched on, if its perch is anchored on one.
fn perched_on(fauna: &Fauna, id: u64) -> Option<Site> {
    let a = animal(fauna, id);
    if a.mobility.mode != MobilityMode::Perched {
        return None;
    }
    match a.mobility.perch.map(|p| p.anchor) {
        Some(PerchAnchor::Stand { site, .. }) => Some(site),
        _ => None,
    }
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

/// Respiration this tick beyond basal upkeep, and all of it.
fn tick_costs(fauna: &mut Fauna, world: &World, flora: &mut Flora) -> (f64, f64) {
    let l = &fauna.view().ledger;
    let (out, maint) = (l.respired_out, l.respired_maintenance_out);
    fauna.step(world, flora);
    let l = &fauna.view().ledger;
    (
        (l.respired_out - l.respired_maintenance_out) - (out - maint),
        l.respired_out - out,
    )
}

fn median(mut v: Vec<f64>) -> f64 {
    assert!(!v.is_empty(), "a median of nothing");
    v.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    v[v.len() / 2]
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

/// The lanternberry seed organic on every face of the ground.
fn lanternberry_seeds(flora: &Flora) -> f64 {
    flora
        .view()
        .ground
        .iter()
        .map(|g| g.seed_organic(Plant::Lanternberry))
        .sum()
}

// ------------------------------------------------------------------ the table

/// **The seedporter's table** (Brief F3, §3.9): the specimen's name and role; a fruit
/// diet; the browser's table scaled by the two tables' body volumes; the shredder's
/// wall-climb cost; a glider that carries seeds in its gut; diurnal; founded by bodies at
/// food; on the lanternjaw's list; not a flyer. Its own manifest at the browser's width
/// and three actions. Fruit is a food class, not a ground pool. No tuned number is pinned.
#[test]
fn the_seedporter_table_scales_the_browsers_and_glides() {
    assert_eq!(SP.name(), "seedporter", "the specimen key");
    assert_eq!(SP.role(), "glider");
    assert!(Founder::ALL.contains(&SP));
    assert!(Food::ALL.contains(&Food::Fruit) && !Food::Fruit.is_pool(), "a hanging product");
    assert_eq!(Food::Fruit.name(), "fruit");

    let x1 = FounderPhysiology::placeholder(SP);
    let grazer = FounderPhysiology::placeholder(Founder::Browser);
    let shredder = FounderPhysiology::frozen(Founder::Blind);
    let sp = FounderPhysiology::frozen(SP);
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-9 * b.abs().max(1.0);
    assert_eq!(sp.diet, Diet::only(Food::Fruit));
    let r = (x1.adult_length_m * x1.adult_width_m * x1.adult_height_m)
        / (grazer.adult_length_m * grazer.adult_width_m * grazer.adult_height_m);
    assert!(close(x1.core.body_max, grazer.core.body_max * r), "the browser's by volume");
    assert!(close(sp.wall_climb_cost_factor, shredder.wall_climb_cost_factor), "the shredder's climb cost");
    assert_eq!(sp.active, crate::Activity::Diurnal);
    assert_eq!((sp.founding, sp.founding_site), (Founding::Bodies, FoundingSite::AtFood));
    assert!(sp.hunter.is_none());
    assert!(sp.flight.is_none(), "it glides, it does not fly");

    let g = sp.glider.expect("the seedporter glides");
    assert!(g.glide_ratio > 0.0 && g.glide_speed_fraction > 0.0);
    for other in Founder::ALL.into_iter().filter(|&o| o != SP) {
        let p = FounderPhysiology::frozen(other);
        assert!(p.glider.is_none() && p.gut_seeds.is_none(), "{other:?} neither glides nor carries seeds");
    }
    let gut = sp.gut_seeds.expect("it carries seeds in its gut");
    assert!(gut.seeds_per_fruit_organic > 0.0 && gut.seeds_per_fruit_organic < 1.0);
    assert!(gut.gut_transit_s > 0.0);

    // Predation: a grounded seedporter is the lanternjaw's prey (the mode is the test's
    // own claim, `a_grounded_seedporter_is_lanternjaw_prey_and_a_climbing_one_is_not`).
    assert!(Hunter::LANTERNJAW.hunts(SP));

    // Its own manifest, the browser's width and actions.
    let m = SP.manifest();
    assert_eq!(m.inputs(), Founder::Browser.manifest().inputs(), "the browser's width");
    assert_eq!(m.actions.len(), 3);
    assert_ne!(m.digest(), Founder::Browser.manifest().digest());
}

// ------------------------------------------------------------------ climbing

/// **It climbs a 1 m lanternberry stem at half walking pace and perches in the crown; it
/// won't climb a stand under 1 m.**
/// - Three columns west of a full-grown lanternberry (1.125 m), holding full forward: it
///   walks on the ground, then climbs ([`MobilityMode::Climb`]) and within 200 ticks
///   perches on that stand at least 1 m over the surface and no higher than the crown
///   top. Its climb rises at `climb_speed_fraction` × its walking pace (medians of the
///   per-tick moves, ±15 %), and a climbing tick costs `wall_climb_cost_factor` × a
///   walking tick beyond upkeep (medians, ±15 %).
/// - The same walk at a lanternberry of 0.7 `wood_max` (0.75 m) never climbs and never
///   perches in 100 ticks: it stays on the ground.
#[test]
fn it_climbs_a_1_m_lanternberry_stem_at_half_walking_pace_and_perches_in_the_crown() {
    let world = plain(24, 20, 8);
    let mut flora = Flora::new(FloraConfig::default());
    let site = full_stand(&mut flora, &world, Plant::Lanternberry, 8, 4);
    let tall = flora.config().lanternberry.crown_height_m_at(flora.config().lanternberry.wood_max);
    assert!(tall >= 1.0, "fixture: a stand at least 1 m tall ({tall})");
    let top = crown_top(&flora, site);

    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = on_ground(&mut fauna, &world, SP, 5, 4, StartingStores::FULL);
    hold(&mut fauna, id, 1.0, 0.0);
    let p = *fauna.config().founder(SP);
    assert_eq!(mode(&fauna, id), MobilityMode::Ground, "it starts walking");

    let (mut walk, mut walk_cost, mut climb, mut climb_cost) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut ticks = 0;
    while perched_on(&fauna, id) != Some(site) && ticks < 200 {
        let before = animal(&fauna, id);
        let (beyond, _) = tick_costs(&mut fauna, &world, &mut flora);
        let after = animal(&fauna, id);
        match (before.mobility.mode, after.mobility.mode) {
            (MobilityMode::Ground, MobilityMode::Ground) => {
                walk.push(after.pose.x - before.pose.x);
                walk_cost.push(beyond);
            }
            (MobilityMode::Climb, MobilityMode::Climb) => {
                climb.push(after.pose.y_m - before.pose.y_m);
                climb_cost.push(beyond);
            }
            _ => {}
        }
        ticks += 1;
    }
    assert_eq!(perched_on(&fauna, id), Some(site), "it perched on the lanternberry in {ticks} ticks");
    let y = animal(&fauna, id).pose.y_m;
    assert!(y >= SURFACE + 1.0 - 1e-9, "a metre up the stand: {y}");
    assert!(y <= top + 0.05, "in the crown, not over it: {y} against a crown top {top}");
    assert!(walk.len() >= 3 && climb.len() >= 10, "it walked ({}) and climbed ({})", walk.len(), climb.len());
    let (w, c) = (median(walk), median(climb));
    assert!(w > 0.0 && c > 0.0, "it went east and up: {w}, {c}");
    // GAIT (Wrysk 2026-10-06): "climb is slower that walking".
    assert!(c < w, "it climbs ({c} m/tick) slower than it walks ({w} m/tick)");
    let pace = c / w;
    assert!(
        (pace - p.climb_speed_fraction).abs() <= 0.15 * p.climb_speed_fraction,
        "climb pace {c} m/tick against walking {w}: {pace}"
    );
    let factor = median(climb_cost) / median(walk_cost);
    assert!(
        (factor - p.wall_climb_cost_factor).abs() <= 0.15 * p.wall_climb_cost_factor,
        "a climbing tick costs {factor} walking ticks beyond upkeep, want {}",
        p.wall_climb_cost_factor
    );

    // A stand under a metre.
    let mut flora = Flora::new(FloraConfig::default());
    let wood = 0.7 * flora.config().lanternberry.wood_max;
    let short = stand(&mut flora, &world, Plant::Lanternberry, 8, 4, wood);
    let h = flora.config().lanternberry.crown_height_m_at(wood);
    assert!(h < 1.0 - V / 2.0, "fixture: a stand well under 1 m ({h})");
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = on_ground(&mut fauna, &world, SP, 5, 4, StartingStores::FULL);
    hold(&mut fauna, id, 1.0, 0.0);
    for t in 0..100 {
        fauna.step(&world, &mut flora);
        let a = animal(&fauna, id);
        assert_ne!(a.mobility.mode, MobilityMode::Climb, "tick {t}: it climbed a {h} m stand");
        assert_ne!(a.mobility.mode, MobilityMode::Perched, "tick {t}: it perched on {short:?}");
    }
}

// ------------------------------------------------------------------ gliding

/// **A glide from 2 m covers about 5 m before landing, at 1.2 × basal.** A seedporter put
/// in the air 2 m over a plain, facing east, glides ([`MobilityMode::Glide`]) and lands on
/// the ground (no longer gliding, its base on the surface) 5 m east (±0.5 m, the ratio
/// 2.5), at 3 BL/s horizontally (±15 %), and every gliding tick costs
/// `glide_cost_factor` × its basal upkeep in all (±10 %).
#[test]
fn a_glide_from_2_m_covers_about_5_m_before_landing_at_1_2_times_basal() {
    let world = plain(64, 24, 8);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let (x0, z) = centre(8, 4);
    let id = aloft(&mut fauna, &world, x0, z, SURFACE + 2.0, StartingStores::FULL);
    hold(&mut fauna, id, 1.0, 0.0);
    assert_eq!(mode(&fauna, id), MobilityMode::Glide, "placed in the air, it glides");
    let g = glider(&fauna);
    let p = *fauna.config().founder(SP);
    let body = animal(&fauna, id).body;
    let basal = p.core.maintenance_per_s * body * DT;

    let mut gliding = 0;
    let mut costs = Vec::new();
    while mode(&fauna, id) == MobilityMode::Glide && gliding < 200 {
        let (_, all) = tick_costs(&mut fauna, &world, &mut flora);
        if mode(&fauna, id) == MobilityMode::Glide {
            costs.push(all);
        }
        gliding += 1;
    }
    let a = animal(&fauna, id);
    assert!(
        matches!(a.mobility.mode, MobilityMode::Ground | MobilityMode::Perched),
        "it landed: {:?}",
        a.mobility.mode
    );
    assert!((a.pose.y_m - SURFACE).abs() <= 0.02, "on the ground: {}", a.pose.y_m);
    let across = a.pose.x - x0;
    let want = 2.0 * g.glide_ratio;
    assert!((across - want).abs() <= 0.5, "a 2 m glide covers about {want} m: {across}");
    let speed = across / (gliding as f64 * DT);
    // GAIT: `glide_speed_fraction` × its ground cruise.
    let pace = g.glide_speed_fraction * fauna.config().manifest(SP).cruise_m_per_s;
    assert!((speed - pace).abs() <= 0.15 * pace, "glide pace {speed} m/s against {pace}");
    let per_tick = median(costs);
    assert!(
        (per_tick / basal - g.glide_cost_factor).abs() <= 0.1 * g.glide_cost_factor,
        "a gliding tick costs {} x basal, want {}",
        per_tick / basal,
        g.glide_cost_factor
    );
}

/// **No glide starts below 1 m.** A seedporter on a rock ledge, facing its east edge,
/// holding full forward for 100 ticks: off a 0.75 m drop it never glides; off a 1.5 m drop
/// it does.
#[test]
fn no_glide_starts_below_1_m() {
    for (ledge_top, glides) in [(5u32, false), (8u32, true)] {
        let mut world = plain(32, 24, 8);
        rock(&mut world, 0..=9, 3..=ledge_top);
        let drop = top_of(ledge_top) - SURFACE;
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::default());
        let id = on_ground(&mut fauna, &world, SP, 7, 4, StartingStores::FULL);
        assert!((animal(&fauna, id).pose.y_m - top_of(ledge_top)).abs() < 1e-9, "fixture: on the ledge");
        hold(&mut fauna, id, 1.0, 0.0);
        let mut glided = false;
        for _ in 0..100 {
            fauna.step(&world, &mut flora);
            glided |= mode(&fauna, id) == MobilityMode::Glide;
        }
        assert_eq!(glided, glides, "off a {drop} m drop it glides: {glides}");
    }
}

// ------------------------------------------------------------------ fruit

/// A vine wall in fruit: a rock wall at `x = 6`, `y = 1..=12` on a wet plain 16 × 20 × 5,
/// a latticevine rooted at its foot covering its west faces `y = 3..=10`, rained on once
/// (the latticevine tests' twin trick) and stepped dry until every covered face hangs beads.
/// Returns the world, the plant layer and the faces.
fn vine_wall_in_fruit() -> (World, Flora, Vec<Face>) {
    let mut c = FloraConfig {
        decomposition: 0.0,
        wood_decomposition: 0.0,
        carrion_decomposition: 0.0,
        ..FloraConfig::default()
    };
    c.latticevine = VineConfig {
        spur_reserve_min: 0.1,
        // Back to back, no rest; 0.01 a flower and 0.02 a bunch over the 61.5 s cycle.
        spur_share: 1.0,
        bud_s: 0.5,
        flower_s: 0.5,
        fruit_s: 60.0,
        spent_s: 0.5,
        nectar_per_face_s: 0.01 / 61.5,
        fruit_per_face_s: 0.02 / 61.5,
        spread_cost: 1e12,
        sister_cost: 1e12,
        ..c.latticevine
    };
    let mut sky = plain(16, 20, 5);
    rock(&mut sky, 6..=6, 1..=12);
    let mut flora = Flora::new(c);
    let z = 2;
    let first = Face::new(6, 3, z, FaceDir::NegX);
    let id = flora
        .seed_vine(
            &sky,
            VineSeed {
                root: Site { x: 5, y: 2, z },
                first,
                reserve: 100.0,
                lineage: None,
            },
        )
        .expect("fixture: the vine founder");
    let mut faces = vec![first];
    for y in 4..=10 {
        let f = Face::new(6, y, z, FaceDir::NegX);
        assert!(flora.cover_face(&sky, id, f, 1.0), "fixture: cover {f:?}");
        faces.push(f);
    }
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
    let all_in_fruit = |flora: &Flora| {
        faces.iter().all(|&f| {
            flora
                .view()
                .cover
                .face(f)
                .is_some_and(|c| c.spur == SpurPhase::Fruit && c.fruit > 0.0)
        })
    };
    let mut t = 0;
    while !all_in_fruit(&flora) && t < 200 {
        flora.step(&mut sky);
        t += 1;
    }
    assert!(all_in_fruit(&flora), "fixture: every covered face hangs beads after the shower");
    (sky, flora, faces)
}

/// **A seedporter takes latticevine fruit and lanternberry fruit, and both books close.**
/// - Latticevine: at the foot of the vine wall in fruit, facing it, it walks in and climbs
///   the covered face (full forward until it is in [`MobilityMode::Climb`] a quarter metre
///   up), then holds still feeding for 60 ticks, still on the vine: fruit bites, the faces'
///   beads fall by what it booked eating, which is what the plant layer booked consumed
///   (through `take_fruit`), and both ledgers close.
/// - Lanternberry: perched on a fruiting lanternberry's crown top (`IntroduceAloft`),
///   feeding for 40 ticks: fruit bites, the stand's fruit falls by what it booked eating,
///   booked consumed (through `take_stand_fruit`), it builds tissue (fruit carries a
///   little mineral), and both ledgers close. The same crown without fruit gives no bite:
///   its foliage is not the seedporter's food.
#[test]
fn a_seedporter_takes_latticevine_and_lanternberry_fruit_and_both_books_close() {
    // Latticevine.
    let (world, mut flora, faces) = vine_wall_in_fruit();
    let beads = |flora: &Flora| -> f64 {
        faces
            .iter()
            .map(|&f| flora.view().cover.face(f).map_or(0.0, |c| c.fruit))
            .sum()
    };
    let (before, organic, consumed) = (
        beads(&flora),
        flora.view().organic(),
        flora.view().ledger.consumed_organic_out,
    );
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = on_ground(&mut fauna, &world, SP, 3, 2, HUNGRY_ADULT);
    hold(&mut fauna, id, 1.0, 0.0);
    let n = until(&mut fauna, &world, &mut flora, 150, |f| {
        let a = animal(f, id);
        a.mobility.mode == MobilityMode::Climb && a.pose.y_m >= SURFACE + V
    });
    assert_eq!(mode(&fauna, id), MobilityMode::Climb, "it climbs the vine-covered face ({n} ticks)");
    hold(&mut fauna, id, 0.0, 1.0);
    steps(&mut fauna, &world, &mut flora, 60);
    let held = mode(&fauna, id);
    assert!(
        matches!(held, MobilityMode::Climb | MobilityMode::Perched),
        "it holds on the vine while it eats: {held:?}"
    );
    let l = &fauna.view().ledger;
    assert!(l.bites_by_food[Food::Fruit.index()] >= 1, "a fruit bite: {:?}", l.bites_by_food);
    assert!(l.bites_by_founder_food[SP.index()][Food::Fruit.index()] >= 1);
    let eaten = l.eaten_by_food[Food::Fruit.index()];
    assert!(eaten > 0.0);
    assert!((before - beads(&flora) - eaten).abs() <= 1e-12, "the beads fell by what it ate");
    let booked = flora.view().ledger.consumed_organic_out - consumed;
    assert!((booked - eaten).abs() <= 1e-12, "take_fruit booked {booked}, the meal {eaten}");
    ledgers_close(&fauna, &flora, organic);

    // Lanternberry.
    for fruit in [true, false] {
        let mut world = noon_plain(16, 20, 8);
        let mut flora = Flora::new(if fruit { fruit_flora() } else { barren_flora() });
        let site = fruiting(&mut flora, &mut world, 8, 4, 60);
        assert_eq!(stand_fruit(&flora, site) > 0.0, fruit, "fixture: fruit hangs: {fruit}");
        let (have, organic, consumed) = (
            stand_fruit(&flora, site),
            flora.view().organic(),
            flora.view().ledger.consumed_organic_out,
        );
        let top = crown_top(&flora, site);
        let mut fauna = Fauna::new(FaunaConfig::default());
        let (x, z) = centre(8, 4);
        let id = aloft(&mut fauna, &world, x, z, top, HUNGRY_ADULT);
        // `Fauna::apply` sees the world, not the plant layer, so a crown perch is resolved
        // on the first step.
        steps(&mut fauna, &world, &mut flora, 1);
        assert_eq!(perched_on(&fauna, id), Some(site), "on its crown top");
        hold(&mut fauna, id, 0.0, 1.0);
        steps(&mut fauna, &world, &mut flora, 40);
        let l = &fauna.view().ledger;
        if !fruit {
            assert_eq!(l.bites_by_food.iter().sum::<u64>(), 0, "plain foliage is no meal: {:?}", l.bites_by_food);
            continue;
        }
        assert!(l.bites_by_food[Food::Fruit.index()] >= 1, "a fruit bite: {:?}", l.bites_by_food);
        let eaten = l.eaten_by_food[Food::Fruit.index()];
        assert!(
            (have - stand_fruit(&flora, site) - eaten).abs() <= 1e-12,
            "the stand's fruit fell by what it ate"
        );
        let booked = flora.view().ledger.consumed_organic_out - consumed;
        assert!((booked - eaten).abs() <= 1e-12, "take_stand_fruit booked {booked}, the meal {eaten}");
        assert!(l.assimilated_by_founder[SP.index()] > 0.0, "fruit's mineral lets it build tissue");
        ledgers_close(&fauna, &flora, organic);
    }
}

// ------------------------------------------------------------------ gut seeds

/// **Gut seeds are voided after the transit time at the current face, into the seed
/// bank.** A hungry seedporter perched on a fruiting lanternberry's crown feeds for 10
/// ticks from its first bite, then walks off the crown (full forward): it glides down and
/// lands east of the shrub, where it rests. Its gut is `seeds_per_fruit_organic` 0.1 and
/// `gut_transit_s` 4 s (set here). No lanternberry seed reaches the ground before 4 s
/// after the first bite; by 4 s after the last bite (and two ticks) the seed bank holds
/// lanternberry seeds of exactly 0.1 × the fruit it ate, all of it on the face it rests
/// on — none on the shrub's own site — and both ledgers close (the seeds came out of the
/// fruit and went back to the plant layer).
#[test]
fn gut_seeds_are_voided_after_the_transit_time_at_the_current_face_into_the_seed_bank() {
    const TRANSIT_S: f64 = 4.0;
    const SEEDS: f64 = 0.1;
    let mut world = noon_plain(48, 20, 8);
    let mut flora = Flora::new(fruit_flora());
    let source = fruiting(&mut flora, &mut world, 6, 4, 60);
    assert!(stand_fruit(&flora, source) > 0.0, "fixture: fruit hangs");
    assert_eq!(lanternberry_seeds(&flora), 0.0, "fixture: no lanternberry seed on the ground");
    let organic = flora.view().organic();
    let top = crown_top(&flora, source);

    let mut config = FaunaConfig::default();
    config.founders[SP.index()].gut_seeds = Some(GutSeeds {
        seeds_per_fruit_organic: SEEDS,
        gut_transit_s: TRANSIT_S,
    });
    let mut fauna = Fauna::new(config);
    let (x, z) = centre(6, 4);
    let id = aloft(&mut fauna, &world, x, z, top, HUNGRY_ADULT);
    hold(&mut fauna, id, 0.0, 1.0);
    let transit = (TRANSIT_S / DT).round() as usize;
    let bites = |f: &Fauna| f.view().ledger.bites_by_food[Food::Fruit.index()];

    let first = until(&mut fauna, &world, &mut flora, 20, |f| bites(f) > 0);
    assert!(bites(&fauna) > 0, "fixture: it eats from the crown");
    let mut tick = first;
    let mut last_bite = first;
    let mut landed_at = None;
    let mut glided = false;
    while tick < first + 10 + transit + 40 {
        if tick == first + 10 {
            hold(&mut fauna, id, 1.0, 0.0);
        }
        let b = bites(&fauna);
        fauna.step(&world, &mut flora);
        tick += 1;
        if bites(&fauna) > b {
            last_bite = tick;
        }
        let m = mode(&fauna, id);
        glided |= m == MobilityMode::Glide;
        if glided && landed_at.is_none() && m != MobilityMode::Glide {
            landed_at = Some(animal(&fauna, id).site);
            hold(&mut fauna, id, 0.0, 0.0);
        }
        if tick + 1 < first + transit {
            assert_eq!(lanternberry_seeds(&flora), 0.0, "tick {tick}: voided before the transit time");
        }
        if tick >= last_bite + transit + 2 {
            break;
        }
    }
    assert!(glided, "it glided off the crown");
    let rest = landed_at.expect("it landed");
    assert_eq!(animal(&fauna, id).site, rest, "it rests where it landed");
    assert_ne!(rest, source, "fixture: away from the shrub");
    let eaten = fauna.view().ledger.eaten_by_food[Food::Fruit.index()];
    let want = SEEDS * eaten;
    let voided = lanternberry_seeds(&flora);
    assert!(
        (voided - want).abs() <= 1e-9 * want.max(1e-12),
        "the seed bank holds {voided} of lanternberry seed, {SEEDS} of the {eaten} it ate is {want}"
    );
    let here = flora.view().ground_at(rest).map_or(0.0, |g| g.seed_organic(Plant::Lanternberry));
    assert!((here - voided).abs() <= 1e-15, "all of it on the face it rests on: {here} of {voided}");
    let there = flora.view().ground_at(source).map_or(0.0, |g| g.seed_organic(Plant::Lanternberry));
    assert_eq!(there, 0.0, "none under the shrub it ate from");
    ledgers_close(&fauna, &flora, organic);
}

// ------------------------------------------------------------------ the cone

/// The class slot of each cone sector (the browser's `foliage` slot, fraction and
/// proximity), from the observation vector.
fn cone_class(fauna: &Fauna, i: usize, world: &World, flora: &Flora) -> Vec<(f64, f64)> {
    let manifest = fauna.config().manifest(SP);
    let m = *manifest
        .modules
        .iter()
        .find(|m| m.name.starts_with("Cone"))
        .expect("the seedporter has a cone");
    assert_eq!(m.width, 19, "the browser's cone layout");
    let (view, fv) = (world.view(), flora.view());
    let occupancy = crate::senses::cone_occupancy(&view, &fv, &fauna.view());
    let obs = crate::body::observation(fauna, i, &view, &fv, &manifest, None, Some((&occupancy).into()));
    assert_eq!(obs[m.offset + m.width - 1], 1.0, "the cone is valid");
    (0..3).map(|k| (obs[m.offset + k * 6 + 2], obs[m.offset + k * 6 + 3])).collect()
}

/// **The cone reads fruit and doesn't read plain foliage.** A seedporter on the ground
/// three columns west of a full-grown lanternberry, facing it: with fruit hanging the
/// cone's class slot (the browser's foliage slot) reads it in some sector; the same shrub
/// with no fruit — its foliage only — reads nothing there, in any sector.
#[test]
fn the_cone_reads_fruit_and_not_plain_foliage() {
    let mut seen = Vec::new();
    for fruit in [true, false] {
        let mut world = noon_plain(20, 20, 8);
        let mut flora = Flora::new(if fruit { fruit_flora() } else { barren_flora() });
        let site = fruiting(&mut flora, &mut world, 8, 4, 60);
        assert_eq!(stand_fruit(&flora, site) > 0.0, fruit, "fixture: fruit hangs: {fruit}");
        let mut fauna = Fauna::new(FaunaConfig::default());
        let id = on_ground(&mut fauna, &world, SP, 5, 4, StartingStores::FULL);
        let i = fauna.animals.iter().position(|a| a.id == id).expect("alive");
        seen.push(cone_class(&fauna, i, &world, &flora));
    }
    assert!(
        seen[0].iter().any(|&(f, p)| f > 0.0 && p > 0.0),
        "the fruiting shrub is seen: {:?}",
        seen[0]
    );
    assert!(
        seen[1].iter().all(|&(f, p)| f == 0.0 && p == 0.0),
        "plain foliage is not its food and is not read: {:?}",
        seen[1]
    );
}

// ------------------------------------------------------------------ predation

/// **A grounded seedporter is lanternjaw prey; a climbing one isn't** (nor a perched or a
/// gliding one). A lanternjaw and a seedporter, both adults: [`crate::is_prey_of`] holds
/// while the seedporter is in [`MobilityMode::Ground`] and not in `Climb`, `Perched` or
/// `Glide`.
#[test]
fn a_grounded_seedporter_is_lanternjaw_prey_and_a_climbing_one_is_not() {
    let world = plain(24, 20, 8);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let lj = on_ground(&mut fauna, &world, Founder::Lanternjaw, 4, 4, StartingStores::FULL);
    let sp = on_ground(&mut fauna, &world, SP, 12, 4, StartingStores::FULL);
    let (hunter, prey) = (animal(&fauna, lj), animal(&fauna, sp));
    assert_eq!(prey.mobility.mode, MobilityMode::Ground);
    assert!(crate::is_prey_of(fauna.config(), &hunter, &prey), "a grounded seedporter is prey");
    for m in [MobilityMode::Climb, MobilityMode::Perched, MobilityMode::Glide] {
        let mut off = prey;
        off.mobility.mode = m;
        assert!(!crate::is_prey_of(fauna.config(), &hunter, &off), "a seedporter in {m:?} is not");
    }
}

// ------------------------------------------------------------------ snapshot

/// **Snapshot round trip with a climbing, a gliding and a perched seedporter.** One walks
/// into a full-grown lanternberry's stem until it climbs; one glides from 2.5 m; one is
/// perched on another lanternberry's crown top. The layer read back from its bytes holds
/// the same bodies — pose `y_m`, mode and perch included — and the same ledger.
#[test]
fn a_snapshot_round_trip_keeps_a_climbing_a_gliding_and_a_perched_seedporter() {
    let world = plain(64, 24, 8);
    let mut flora = Flora::new(FloraConfig::default());
    let _ = full_stand(&mut flora, &world, Plant::Lanternberry, 8, 4);
    let roost = full_stand(&mut flora, &world, Plant::Lanternberry, 20, 4);
    let top = crown_top(&flora, roost);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let climbing = on_ground(&mut fauna, &world, SP, 5, 4, StartingStores::FULL);
    let (gx, z) = centre(34, 4);
    let gliding = aloft(&mut fauna, &world, gx, z, SURFACE + 2.5, StartingStores::FULL);
    let (rx, _) = centre(20, 4);
    let perched = aloft(&mut fauna, &world, rx, z, top, StartingStores::FULL);
    hold(&mut fauna, climbing, 1.0, 0.0);
    hold(&mut fauna, gliding, 1.0, 0.0);
    hold(&mut fauna, perched, 0.0, 0.0);
    let n = until(&mut fauna, &world, &mut flora, 100, |f| mode(f, climbing) == MobilityMode::Climb);
    steps(&mut fauna, &world, &mut flora, 2);
    assert_eq!(mode(&fauna, climbing), MobilityMode::Climb, "climbing after {n} ticks");
    assert_eq!(mode(&fauna, gliding), MobilityMode::Glide, "still gliding");
    assert_eq!(perched_on(&fauna, perched), Some(roost), "still perched");

    let back = Fauna::load(&fauna.save()).expect("the layer round-trips");
    assert_eq!(back.view().animals, fauna.view().animals, "the same bodies");
    for id in [climbing, gliding, perched] {
        let (a, b) = (animal(&fauna, id), *back.view().animal(id).expect("kept"));
        assert_eq!(b.pose.y_m, a.pose.y_m, "{id}: its height");
        assert_eq!(b.mobility, a.mobility, "{id}: its mode and perch");
    }
    assert_eq!(back.view().ledger, fauna.view().ledger, "the same ledger");
}

/// **Fruit into the stomach** (the minimal gut; Astra's finding 8): a seedporter with an
/// empty stomach perched on a fruiting lanternberry takes a bite; on that tick its fruit
/// pool and its gut seeds together hold exactly what the stand gave, in all three
/// currencies, the seeds their `seeds_per_fruit_organic` share.
#[test]
fn a_fruit_bite_lands_in_the_stomach_and_the_gut_seeds() {
    let mut world = noon_plain(16, 20, 8);
    let mut flora = Flora::new(fruit_flora());
    let site = fruiting(&mut flora, &mut world, 8, 4, 60);
    let top = crown_top(&flora, site);
    let mut config = FaunaConfig::default();
    config.founders[SP.index()].gut.s0 = 0.0;
    let share = config.founders[SP.index()].gut_seeds.expect("seeds").seeds_per_fruit_organic;
    let mut fauna = Fauna::new(config);
    let (x, z) = centre(8, 4);
    let id = aloft(&mut fauna, &world, x, z, top, HUNGRY_ADULT);
    steps(&mut fauna, &world, &mut flora, 1);
    hold(&mut fauna, id, 0.0, 1.0);
    let consumed = flora.view().ledger.consumed_organic_out;
    let mut t = 0;
    while fauna.view().ledger.bites == 0 && t < 40 {
        fauna.step(&world, &mut flora);
        t += 1;
    }
    let l = fauna.view().ledger;
    assert_eq!(l.bites, 1, "one fruit bite");
    let a = animal(&fauna, id);
    let pool = a.stomach.pools[Food::Fruit.index()];
    let seeds = a.gut_seeds.total();
    for (got, want, what) in [
        (pool.organic + seeds.organic, l.eaten_organic_in, "organic"),
        (pool.mineral + seeds.mineral, l.eaten_mineral_in, "mineral"),
        (pool.energy + seeds.energy, l.eaten_energy_in, "energy"),
    ] {
        assert!((got - want).abs() <= 1e-15 * want.max(1e-12), "{what}: {got} against {want}");
    }
    assert!((seeds.organic - share * l.eaten_organic_in).abs() <= 1e-15);
    assert!(pool.mineral > 0.0, "stand fruit carries mineral");
    assert!((flora.view().ledger.consumed_organic_out - consumed - l.eaten_organic_in).abs() <= 1e-15);
}
