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
    Actions, Animal, Command, Fauna, FaunaConfig, Food, Founder, FounderPhysiology,
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
    // F3's fruit alone (FLOWER-FRUIT): no flowering stage, and no rest after a crop is
    // eaten out, so the fixture's supply refills as it did.
    c.lanternberry.flower_s = 0.0;
    c.lanternberry.fruit_rest_s = 0.0;
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
    assert!(sp.diet.eats(Food::Fruit) && sp.diet.eats(Food::Carrion));
    let r = (x1.adult_length_m * x1.adult_width_m * x1.adult_height_m)
        / (grazer.adult_length_m * grazer.adult_width_m * grazer.adult_height_m);
    assert!(close(x1.core.body_max, grazer.core.body_max * r), "the browser's by volume");
    assert!(close(sp.wall_climb_cost_factor, shredder.wall_climb_cost_factor), "the shredder's climb cost");
    // RHYTHMS-CADENCE: a diurnal leaning.
    assert_eq!(sp.active, crate::Activity::Leaning);
    assert_eq!((sp.founding, sp.founding_site), (Founding::Bodies, FoundingSite::AtFood));
    assert!(sp.hunter.expect("the opportunistic insect catcher").hunts(Founder::Bellwing));
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
    assert_eq!(m.tail_offset(), Founder::Browser.manifest().tail_offset(), "the browser's v2 layout");
    assert_eq!(m.actions.len(), 4, "the three and its sprint (contract v3)");
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
/// gliding one). A grown lanternjaw and a young seedporter (LANTERNJAW-DIET: only young
/// ones): [`crate::is_prey_of`] holds while the seedporter is in [`MobilityMode::Ground`]
/// and not in `Climb`, `Perched` or `Glide`.
#[test]
fn a_grounded_seedporter_is_lanternjaw_prey_and_a_climbing_one_is_not() {
    let world = plain(24, 20, 8);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let lj = on_ground(&mut fauna, &world, Founder::Lanternjaw, 4, 4, StartingStores::FULL);
    let core = fauna.config().founder(SP).core;
    let young = StartingStores { body: 0.5 * core.birth_body / core.body_max, reserve: 1.0 };
    let sp = on_ground(&mut fauna, &world, SP, 12, 4, young);
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

/// **A seedporter that has landed does not relaunch without a reason** (PACK-TUNE, the glide
/// loop: GAIT's s4 run had 49,704 glides). A vine-covered wall whose cover reaches its top.
/// A seedporter walks in, climbs the face and perches at its top; it is then set hugging
/// the face, its centre 0.02 m off it and its footprint in the wall's column (the probe's
/// state on GAIT's s4 save: a glide's fall goes down its centre column and can end there).
/// Held at full forward it glides off the face, from clear of the wall, once; it comes down
/// on the floor and walks on, and is not pressed back onto the face to climb it and launch
/// again (the loop: a glide blocked in place, a fall, the same face, a climb, a glide).
#[test]
fn a_seedporter_gliding_off_a_vine_face_lands_once_and_does_not_relaunch() {
    let (mut world, mut flora, _) = vine_wall_in_fruit();
    // The rock stops where the cover does (y = 10): the face's top is the wall's top.
    for z in 0..world.config().depth {
        for y in 11..=12 {
            world.apply(WorldCommand::SetMaterial { x: 6, y, z, material: Material::Air });
        }
    }
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = on_ground(&mut fauna, &world, SP, 3, 2, HUNGRY_ADULT);
    // ROOSTS (2026-10-08): a climb up to a lip it reaches now pulls itself over onto the top,
    // so the body is put clinging at the face's top by hand, as the climb used to leave it.
    let k = fauna.animals.iter().position(|a| a.id == id).expect("the body");
    {
        let a = &mut fauna.animals[k];
        a.pose.y_m = 10.0 * V;
        a.mobility.mode = MobilityMode::Perched;
        a.mobility.falling = false;
        a.mobility.perch = Some(crate::Perch {
            kind: crate::PerchKind::VineFace,
            anchor: PerchAnchor::Vine(Face::new(6, 10, 2, FaceDir::NegX)),
        });
    }
    fauna.animals[k].pose.x = 6.0 * V - 0.02;
    fauna.animals[k].pose.z = centre(5, 2).1;
    hold(&mut fauna, id, 1.0, 0.0);
    let climbs = fauna.view().ledger.climbs;
    let mut launches = 0;
    let mut last = mode(&fauna, id);
    for _ in 0..600 {
        fauna.step(&world, &mut flora);
        let m = mode(&fauna, id);
        if last == MobilityMode::Perched && m == MobilityMode::Glide {
            launches += 1;
        }
        last = m;
    }
    let again = fauna.view().ledger.climbs - climbs;
    assert!(launches <= 1, "it leaves the face once in 600 ticks, not a loop: {launches} launches, {again} climbs");
    assert_eq!(again, 0, "it does not climb the face again");
    assert_eq!(mode(&fauna, id), MobilityMode::Ground, "it came down and walks: {:?}", animal(&fauna, id).mobility);
}

/// With fruit only on a covered wall, the normal teacher finds it and climbs to eat.
#[test]
fn a_hungry_seedporter_sees_vine_fruit_climbs_to_it_and_eats() {
    let (world, mut flora, faces) = vine_wall_in_fruit();
    assert!(flora.view().stands.is_empty(), "fixture: no stand fruit to distract it");
    let cfg = FaunaConfig::default();
    let body = cfg.founder(SP).adult_body();
    // Keep the nearest beads above a ground mouth, so eating requires a climb.
    for &face in &faces {
        if f64::from(face.y) * V <= SURFACE + body.mouth_ceiling_m + V {
            assert!(flora.take_fruit(face, 1e9).is_some());
        }
    }
    let beads = |flora: &Flora| -> f64 {
        faces.iter().map(|&f| flora.view().cover.face(f).map_or(0.0, |c| c.fruit)).sum()
    };
    let before = beads(&flora);
    assert!(before > 0.0, "fixture: fruit remains higher on the vine");
    let organic = flora.view().organic();
    let consumed = flora.view().ledger.consumed_organic_out;
    let mut fauna = Fauna::new(cfg);
    let id = on_ground(&mut fauna, &world, SP, 1, 2, HUNGRY_ADULT);
    let teachers = fauna.config().teachers;
    assert!(fauna.set_controller(id, crate::controller::heuristic(SP, teachers)));
    let start = animal(&fauna, id).pose;
    let climbed = std::cell::Cell::new(false);
    let n = until(&mut fauna, &world, &mut flora, 300, |f| {
        climbed.set(climbed.get() || mode(f, id) == MobilityMode::Climb);
        f.view().ledger.eaten_by_food[Food::Fruit.index()] > 0.0
    });
    let eaten = fauna.view().ledger.eaten_by_food[Food::Fruit.index()];
    assert!(eaten > 0.0, "the teacher finds and eats the vine fruit within {n} ticks: {:?}", animal(&fauna, id));
    assert!(climbed.get() && fauna.view().ledger.climbs > 0, "it climbs to the high beads");
    assert!((animal(&fauna, id).pose.x - start.x).abs() > V, "it walks toward the wall");
    assert!((before - beads(&flora) - eaten).abs() <= 1e-12, "the vine loses precisely the fruit eaten");
    assert!((flora.view().ledger.consumed_organic_out - consumed - eaten).abs() <= 1e-12,
        "the plant and animal consumption ledgers agree");
    ledgers_close(&fauna, &flora, organic);
}

// ------------------------------------------------------------------ VINE-C: a bite over a patch

/// The distance from `p` to the square of wall face `f` (a side face; `x` unwrapped near
/// `p`), metres.
fn to_face(p: (f64, f64, f64), f: Face, v: f64) -> f64 {
    let (bx, by, bz) = (f64::from(f.x) * v, f64::from(f.y) * v, f64::from(f.z) * v);
    let (xr, zr) = match f.dir {
        FaceDir::NegX => ((bx, bx), (bz, bz + v)),
        FaceDir::PosX => ((bx + v, bx + v), (bz, bz + v)),
        FaceDir::NegZ => ((bx, bx + v), (bz, bz)),
        FaceDir::PosZ => ((bx, bx + v), (bz + v, bz + v)),
        FaceDir::Down => ((bx, bx + v), (bz, bz + v)),
    };
    let yr = if f.dir == FaceDir::Down { (by, by) } else { (by, by + v) };
    let d = |q: f64, (lo, hi): (f64, f64)| (lo - q).max(0.0).max(q - hi);
    let (dx, dy, dz) = (d(p.0, xr), d(p.1, yr), d(p.2, zr));
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// **VINE-C (Wrysk 2026-10-09, option C): a vine bite draws on every fruiting face within
/// the mouth's reach, and on none beyond.** A seedporter climbs the vine wall in fruit; every
/// covered face is then set to the same beads. At its first fruit bite, every face that lost
/// beads lies within its mouth's reach (its radius plus its mouth reach, from its middle),
/// more than one face lost beads, every face beyond the reach kept all of its beads, and the
/// faces in reach each gave the same share. The books close.
#[test]
fn a_vine_bite_draws_on_every_face_in_reach_and_none_beyond() {
    let (world, mut flora, faces) = vine_wall_in_fruit();
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = on_ground(&mut fauna, &world, SP, 3, 2, HUNGRY_ADULT);
    hold(&mut fauna, id, 1.0, 0.0);
    until(&mut fauna, &world, &mut flora, 150, |f| {
        let a = animal(f, id);
        a.mobility.mode == MobilityMode::Climb && a.pose.y_m >= SURFACE + V
    });
    assert_eq!(mode(&fauna, id), MobilityMode::Climb, "fixture: on the vine");
    hold(&mut fauna, id, 0.0, 1.0);
    let beads = 1.0e-3;
    for &f in &faces {
        assert!(flora.set_spur(f, SpurPhase::Fruit, beads), "fixture: beads on {f:?}");
    }
    let organic = flora.view().organic();
    let n = until(&mut fauna, &world, &mut flora, 60, |f| f.view().ledger.bites_by_food[Food::Fruit.index()] >= 1);
    assert!(fauna.view().ledger.bites_by_food[Food::Fruit.index()] == 1, "one fruit bite within 60 ticks ({n})");
    let a = animal(&fauna, id);
    let phys = fauna.config().founder(SP);
    let g = phys.body_at(a.body);
    let reach = g.footprint_radius() + g.mouth_reach_m;
    let p = (a.pose.x, a.pose.y_m + 0.5 * g.height_m.max(1e-6), a.pose.z);
    let mut drawn = Vec::new();
    for &f in &faces {
        let left = flora.view().cover.face(f).map_or(0.0, |c| c.fruit);
        let d = to_face(p, f, V);
        if left < beads {
            assert!(d <= reach + 1e-9, "{f:?} gave beads from {d} m, beyond the reach {reach} m");
            drawn.push(beads - left);
        }
        if d > reach + 1e-9 {
            assert_eq!(left, beads, "{f:?} beyond the reach ({d} m > {reach} m) keeps its beads");
        }
    }
    assert!(drawn.len() >= 2, "the bite reached more than one face: {drawn:?}");
    for x in &drawn {
        assert!((x - drawn[0]).abs() <= 1e-12, "equal beads give equal shares: {drawn:?}");
    }
    let eaten = fauna.view().ledger.eaten_by_food[Food::Fruit.index()];
    assert!((drawn.iter().sum::<f64>() - eaten).abs() <= 1e-12, "the faces lost what it ate");
    ledgers_close(&fauna, &flora, organic);
}

/// A plain of `voxel_m` cells, `w × h × d` cells, soil to 0.75 m, a rock column filling the
/// cells over `[1.5, 1.75)` m in `x` from the soil up to 2.5 m, and one vine covering the
/// wall's west faces over `z ∈ [0.5, 0.75)` m, `y ∈ [0.75, 1.0)` m: one face at 0.25 m, four
/// at 0.125 m. The covered faces.
fn wall_at(voxel_m: f64) -> (World, Flora, Vec<Face>) {
    let k = (0.25 / voxel_m).round() as u32;
    let mut w = World::empty(VoxelConfig {
        width: 16 * k,
        height: 12 * k,
        depth: 8 * k,
        voxel_m,
        seed: 7,
        day: clock_off(),
        ..VoxelConfig::default()
    });
    let c = w.config().clone();
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            for y in 0..3 * k {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    for z in 0..c.depth {
        for x in i64::from(6 * k)..i64::from(7 * k) {
            for y in 3 * k..10 * k {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Rock });
            }
        }
    }
    let mut flora = Flora::new(FloraConfig::default());
    let wf = |y: u32, z: u32| Face::new(6 * k, y, z, FaceDir::NegX);
    let first = wf(3 * k, 2 * k);
    let id = flora
        .seed_vine(&w, VineSeed { root: Site { x: 6 * k - 1, y: 3 * k - 1, z: 2 * k }, first, reserve: 100.0, lineage: None })
        .expect("fixture: the vine roots");
    let mut faces = vec![first];
    for y in 3 * k..4 * k {
        for z in 2 * k..3 * k {
            let f = wf(y, z);
            if f != first {
                assert!(flora.cover_face(&w, id, f, 1.0), "fixture: cover {f:?}");
                faces.push(f);
            }
        }
    }
    (w, flora, faces)
}

/// **VINE-C: the bite does not depend on the voxel size.** The same beads over the same
/// square of wall — one 0.25 m face, or four 0.125 m faces holding a quarter each — read as
/// the same stock to a mouth reaching the whole square, and the same bite takes the same
/// fruit out of it.
#[test]
fn the_same_beads_over_the_same_wall_give_the_same_bite_at_two_voxel_sizes() {
    let total = 2.0e-3;
    // A mouth 0.1 m off the wall, level with the square's middle: every part within 0.25 m.
    let p = (1.5 - 0.1, 0.875, 0.625);
    let reach = 0.25;
    let mut got = Vec::new();
    for v in [0.25, 0.125] {
        let (w, mut flora, faces) = wall_at(v);
        for &f in &faces {
            assert!(flora.set_spur(f, SpurPhase::Fruit, total / faces.len() as f64));
        }
        let (source, stock) = crate::flight::fruit_in_reach(&w.view(), &flora.view(), p, reach, None).expect("beads in reach");
        assert!(matches!(source, crate::Bloom::Vine(_)), "{source:?}");
        let patch: Vec<Face> = crate::flight::vine_patch(&w.view(), &flora.view(), p, reach, SpurPhase::Fruit).into_iter().map(|r| r.face).collect();
        assert_eq!(patch.len(), faces.len(), "every face of the square is in reach at {v} m");
        let bite = 0.3 * total;
        let taken = flora.take_fruit_patch(&patch, bite).expect("a bite");
        got.push((faces.len(), stock, taken.organic));
    }
    assert_eq!((got[0].0, got[1].0), (1, 4), "fixture: one face against four");
    assert!((got[0].1 - got[1].1).abs() <= 1e-15, "the same stock read: {got:?}");
    assert!((got[0].2 - got[1].2).abs() <= 1e-15, "the same bite taken: {got:?}");
}

// ------------------------------------------------------------------ SEEDPORTER-POLLEN

/// A plain 16 × 20 × 5 with the rock wall at `x = 6`, `y = 1..=12`, and one latticevine
/// covering every west face of it at rows `y = 3..=10` across every `z` (a sheet of vine). The
/// faces at the rows `flowering` keeps are set open in flower with a little nectar; the rest
/// are bare. The covered faces.
fn flower_sheet(flowering: impl Fn(Face) -> bool) -> (World, Flora, Vec<Face>) {
    let mut world = plain(16, 20, 5);
    rock(&mut world, 6..=6, 1..=12);
    let mut flora = Flora::new(FloraConfig::default());
    let first = Face::new(6, 3, 2, FaceDir::NegX);
    let id = flora
        .seed_vine(&world, VineSeed { root: Site { x: 5, y: 2, z: 2 }, first, reserve: 100.0, lineage: None })
        .expect("fixture: the vine founder");
    let mut faces = vec![first];
    let mut zs: Vec<u32> = (0..world.config().depth).collect();
    zs.sort_by_key(|&z| z.abs_diff(2));
    for y in 3..=10 {
        for &z in &zs {
            let f = Face::new(6, y, z, FaceDir::NegX);
            if f != first {
                assert!(flora.cover_face(&world, id, f, 1.0), "fixture: cover {f:?}");
                faces.push(f);
            }
        }
    }
    for &f in &faces {
        if flowering(f) {
            assert!(flora.set_spur(f, SpurPhase::Flower, 1.0e-4), "fixture: a flower on {f:?}");
        }
    }
    (world, flora, faces)
}

fn visits(flora: &Flora, f: Face) -> u32 {
    flora.view().cover.face(f).expect("covered").visits.into()
}

/// A seedporter at the foot of the flowering sheet walks east into it and climbs: full
/// forward for up to 150 ticks, until it is a metre up the wall. `(its id, its fauna)`.
fn climb_the_sheet(config: FaunaConfig, world: &World, flora: &mut Flora) -> (u64, Fauna) {
    let mut fauna = Fauna::new(config);
    let id = on_ground(&mut fauna, world, SP, 3, 2, HUNGRY_ADULT);
    hold(&mut fauna, id, 1.0, 0.0);
    until(&mut fauna, world, flora, 150, |f| {
        let a = animal(f, id);
        a.mobility.mode == MobilityMode::Climb && a.pose.y_m >= SURFACE + 1.0
    });
    assert_eq!(mode(&fauna, id), MobilityMode::Climb, "fixture: it climbs the sheet");
    (id, fauna)
}

/// **SEEDPORTER-POLLEN (Wrysk 2026-10-09: "seedporters should also polinate by moving
/// accross them or around them"): climbing across open vine flowers pollinates the ones it
/// moves onto, and a lineage without the switch pollinates none.** Every face of the sheet is
/// in flower. A seedporter walks into it and climbs a metre: the flowers it touched first
/// give it pollen, and the ones it climbed onto after that are visited — more than one, every
/// visit booked as a contact visit and as the plant layer's (there is no sip here), each
/// visited face within its body's reach of the column it climbed. Both books close. The same
/// climb with `pollen_by_contact` off visits nothing.
#[test]
fn climbing_over_open_vine_flowers_visits_the_ones_it_moves_onto_and_only_with_the_switch() {
    for on in [true, false] {
        let (world, mut flora, faces) = flower_sheet(|_| true);
        let organic = flora.view().organic();
        let mut config = FaunaConfig::default();
        assert!(config.founder(SP).pollen_by_contact, "the seedporter's body carries pollen");
        config.founders[SP.index()].pollen_by_contact = on;
        let (id, fauna) = climb_the_sheet(config, &world, &mut flora);
        let contact = fauna.view().ledger.contact_visits_by_founder[SP.index()];
        let booked = flora.view().ledger.flower_visits[1];
        let visited: Vec<Face> = faces.iter().copied().filter(|&f| visits(&flora, f) > 0).collect();
        if on {
            assert!(visited.len() >= 2, "it pollinated the flowers it climbed onto: {visited:?}");
            assert_eq!(contact, booked, "every vine visit here is a contact visit");
            assert_eq!(booked, visited.iter().map(|&f| u64::from(visits(&flora, f))).sum::<u64>(), "booked to the faces");
            let a = animal(&fauna, id);
            let r = fauna.config().founder(SP).body_at(a.body).footprint_radius();
            for f in &visited {
                let dz = (f64::from(f.z) * V - a.pose.z).max(a.pose.z - f64::from(f.z + 1) * V).max(0.0);
                assert!(dz <= r + crate::flight::PRESSED_M + 1e-9, "{f:?} is beside the column it climbed (dz {dz})");
            }
            assert!(
                matches!(a.last_bloom, Some(crate::Bloom::Vine(_))),
                "it carries pollen on from the flowers it touched: {:?}",
                a.last_bloom
            );
        } else {
            assert_eq!((contact, booked, visited.len()), (0, 0, 0), "without the switch no flower is visited");
        }
        ledgers_close(&fauna, &flora, organic);
    }
}

/// **The first flowers a body touches give it pollen; they are not visited.** Only the row
/// its body meets as it walks into the wall (`y = 3`) is in flower. It walks in and climbs on
/// past bare faces: no flower is visited, and it carries pollen from one of the flowers it
/// touched.
#[test]
fn a_seedporter_carrying_no_pollen_from_elsewhere_visits_nothing() {
    let (world, mut flora, _) = flower_sheet(|f| f.y == 3);
    let (id, fauna) = climb_the_sheet(FaunaConfig::default(), &world, &mut flora);
    assert_eq!(flora.view().ledger.flower_visits[1], 0, "nothing brought pollen from elsewhere");
    assert_eq!(fauna.view().ledger.contact_visits_by_founder[SP.index()], 0);
    match animal(&fauna, id).last_bloom {
        Some(crate::Bloom::Vine(f)) => assert_eq!(f.y, 3, "the pollen is from a flower it touched: {f:?}"),
        other => panic!("it picked pollen up from the flowers it touched: {other:?}"),
    }
}

/// **A body at rest against flowers does not visit them tick after tick.** The seedporter
/// climbs into the flowering sheet carrying pollen from elsewhere, then holds still for 200
/// ticks against the same flowers: not one visit more.
#[test]
fn a_seedporter_resting_against_the_same_flowers_adds_no_visits() {
    let (world, mut flora, _) = flower_sheet(|_| true);
    let (id, mut fauna) = climb_the_sheet(FaunaConfig::default(), &world, &mut flora);
    hold(&mut fauna, id, 0.0, 0.0);
    steps(&mut fauna, &world, &mut flora, 2);
    let (pose, booked) = (animal(&fauna, id).pose, flora.view().ledger.flower_visits[1]);
    assert!(booked > 0, "fixture: its climb pollinated");
    // It carries pollen from far along the wall, as if it came from another vine.
    let k = fauna.animals.iter().position(|a| a.id == id).expect("the body");
    fauna.animals[k].last_bloom = Some(crate::Bloom::Vine(Face::new(6, 10, 0, FaceDir::NegX)));
    steps(&mut fauna, &world, &mut flora, 200);
    assert_eq!(animal(&fauna, id).pose, pose, "fixture: it held still");
    assert_eq!(flora.view().ledger.flower_visits[1], booked, "no visit while it rests");
}

/// **The rule's two refusals, read directly**: a body that carries no pollen, or only the
/// pollen of the one flower it comes to, visits nothing as it moves onto it; the same move
/// carrying another flower's pollen visits it.
#[test]
fn moving_onto_a_flower_visits_it_only_with_pollen_from_another() {
    let (world, flora, _) = flower_sheet(|f| f.y == 4 && f.z == 2);
    let body = FaunaConfig::default().founder(SP).adult_body();
    let shape = crate::flight::Shape { r: body.footprint_radius(), h: body.height_m };
    let at = |x: f64| Pose { x, z: centre(0, 2).1, heading_rad: EAST, y_m: 4.0 * V + 0.02 };
    let (before, now) = (at(3.0 * V), at(6.0 * V - shape.r - 0.005));
    let flower = Face::new(6, 4, 2, FaceDir::NegX);
    let other = crate::Bloom::Vine(Face::new(6, 10, 0, FaceDir::NegX));
    let read = |carried| crate::flight::brushed_flowers(&world.view(), &flora.view(), &before, &now, shape, crate::flight::PRESSED_M, carried);
    assert_eq!(read(None), (vec![], Some(crate::Bloom::Vine(flower))), "no pollen: none visited, its pollen picked up");
    assert_eq!(read(Some(crate::Bloom::Vine(flower))).0, vec![], "its own pollen back: none visited");
    assert_eq!(read(Some(other)).0, vec![flower], "pollen from another flower: visited");
    // Already touching it a tick ago: nothing new.
    assert_eq!(crate::flight::brushed_flowers(&world.view(), &flora.view(), &now, &now, shape, crate::flight::PRESSED_M, Some(other)).0, vec![]);
}

/// **Contact is in metres.** The same body pressed against the same square of wall in
/// flower — one 0.25 m face, or four 0.125 m faces — moving onto it with another flower's
/// pollen visits the same area of flowers.
#[test]
fn the_same_body_against_the_same_square_pollinates_the_same_area_at_two_voxel_sizes() {
    let body = FaunaConfig::default().founder(SP).adult_body();
    let shape = crate::flight::Shape { r: body.footprint_radius(), h: body.height_m };
    // Level with the square (y ∈ [0.75, 1.0), z ∈ [0.5, 0.75)) and pressed on its wall at
    // x = 1.5, from a metre back.
    let pose = |x: f64| Pose { x, z: 0.625, heading_rad: EAST, y_m: 0.875 - 0.5 * shape.h };
    let (before, now) = (pose(0.5), pose(1.5 - shape.r - 0.005));
    let carried = Some(crate::Bloom::Stand(Site { x: 0, y: 0, z: 0 }));
    let mut got = Vec::new();
    for v in [0.25, 0.125] {
        let (w, mut flora, faces) = wall_at(v);
        for &f in &faces {
            assert!(flora.set_spur(f, SpurPhase::Flower, 1.0e-4));
        }
        let (visited, _) = crate::flight::brushed_flowers(&w.view(), &flora.view(), &before, &now, shape, crate::flight::PRESSED_M, carried);
        got.push((faces.len(), visited.len(), visited.len() as f64 * v * v));
    }
    assert_eq!((got[0].0, got[1].0), (1, 4), "fixture: one face against four");
    assert!(got[0].1 > 0, "fixture: the body reaches the square: {got:?}");
    assert!((got[0].2 - got[1].2).abs() <= 1e-12, "the same area of flowers visited: {got:?}");
}

// ------------------------------------------------------------------ juvenile fruit-search traps

fn freeze_config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.founders[SP.index()].active = crate::Activity::Always;
    // Short, explicit fixture clocks; isolate a fruit approach from hungry roaming and
    // patch abandonment. The bounded walk/perch repair must work without those layers.
    c.teachers.press_replan_s = 0.5;
    c.teachers.press_shun_s = 4.0;
    c.teachers.search_leg_bl = 0.0;
    c.teachers.lead_give_up_bl = 0.0;
    c.teachers.give_up_fraction = 0.0;
    c.founders[SP.index()].give_up_fraction = Some(0.0);
    c.perception.give_up_s = 20.0;
    c
}

fn juvenile_stores() -> StartingStores {
    StartingStores { body: 0.25, reserve: 0.3 }
}

fn use_teacher(fauna: &mut Fauna, id: u64) {
    let teachers = fauna.config().teachers;
    assert!(fauna.set_controller(id, crate::controller::heuristic(SP, teachers)));
}

fn freeze_ticks(c: &FaunaConfig) -> usize {
    let cadence = c.manifest(SP).cadence_ticks().max(1) as f64 * DT;
    ((4.0 * c.teachers.press_replan_s + 4.0 * cadence) / DT).ceil() as usize
}

fn remember_vine(fauna: &mut Fauna, id: u64, world: &World, face: Face) {
    let v = world.config().voxel_m;
    let (x, _, z) = face.front(&world.view()).expect("fixture: exposed vine");
    let dir = match face.dir {
        FaceDir::PosX => 0,
        FaceDir::NegX => 1,
        FaceDir::PosZ => 2,
        FaceDir::NegZ => 3,
        FaceDir::Down => 4,
    };
    let key = (1 << 63) | (1 << 62) | ((world.config().index(i64::from(face.x), face.y, face.z) as u64) << 3) | dir;
    let food = crate::perception::class::FOOD;
    let tick = fauna.tick;
    fauna.percepts.entry(id).or_default().best[food as usize] = crate::Track {
        known: true,
        id: key,
        class: food,
        modes: crate::perception::SIGHT,
        x: (f64::from(x) + 0.5) * v,
        z: (f64::from(z) + 0.5) * v,
        y_m: 3.0 * v,
        site: (x as u32, 2, z),
        tick,
        ..crate::Track::default()
    };
}

/// A wall resolved more finely than the body's radius. Its reachable cover is real,
/// but the covered rock lies several columns beyond the centre's adjacent column.
fn fine_vine_wall(v: f64) -> (World, Flora, Vec<Face>) {
    let (_, stock, _) = vine_wall_in_fruit();
    let mut flora = Flora::new(stock.view().config.clone());
    let make_world = |rain_m_per_s| {
        let mut world = World::empty(VoxelConfig {
            width: 32, height: 40, depth: 8, voxel_m: v,
            day: clock_off(), rain_m_per_s,
            ..VoxelConfig::default()
        });
        soil(&mut world);
        rock(&mut world, 16..=16, 1..=28);
        world
    };
    let mut world = make_world(0.0);
    let first = Face::new(16, 3, 4, FaceDir::NegX);
    let vine = flora.seed_vine(&world, VineSeed {
        root: Site { x: 15, y: 2, z: 4 }, first, reserve: 100.0, lineage: None,
    }).expect("fixture: fine vine root");
    let mut faces = vec![first];
    for y in 4..=24 {
        let f = Face::new(16, y, 4, FaceDir::NegX);
        assert!(flora.cover_face(&world, vine, f, 1.0));
        faces.push(f);
    }
    flora.step(&mut make_world(1e-6));
    for _ in 0..40 {
        if faces.iter().all(|&f| flora.view().cover.face(f).is_some_and(|c| c.fruit > 0.0)) {
            return (world, flora, faces);
        }
        flora.step(&mut world);
    }
    panic!("fixture: the fine wall fruits");
}

/// The fruit reflex used to replace every refused-motor escape with another full
/// push: `vine_ahead` looked only one column out although the footprint hit farther rock.
#[test]
fn juvenile_fruit_wall_press_replans_within_the_configured_bound() {
    let c = freeze_config();
    let body = c.founder(SP).core.body_max * juvenile_stores().body;
    let geometry = c.founder(SP).body_at(body);
    let r = geometry.footprint_radius();
    let v = r / 3.0;
    let (world, mut flora, faces) = fine_vine_wall(v);
    for &face in &faces {
        if f64::from(face.y) * v <= 3.0 * v + geometry.mouth_ceiling_m + v {
            assert!(flora.take_fruit(face, 1e9).is_some());
        }
    }
    let fruit = faces.iter().copied().find(|&f| flora.view().cover.face(f).is_some_and(|c| c.fruit > 0.0)).expect("high fruit remains");
    let bound = freeze_ticks(&c);
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let id = on_ground(&mut fauna, &world, SP, 12, 4, juvenile_stores());
    let k = fauna.animals.iter().position(|a| a.id == id).unwrap();
    fauna.animals[k].pose.x = 16.0 * v - r - 1e-9;
    let start = animal(&fauna, id).pose;
    remember_vine(&mut fauna, id, &world, fruit);
    use_teacher(&mut fauna, id);
    let mut escaped = false;
    let mut pressed = 0;
    for _ in 0..bound {
        fauna.step(&world, &mut flora);
        let a = animal(&fauna, id);
        let fb = a.founder_state.feedback;
        pressed += usize::from(a.founder_state.held.forward > 0.0 && fb.attempted_equivalent > 0.0 && fb.delivered_equivalent <= 1e-12);
        escaped |= (a.pose.x - start.x).hypot(a.pose.z - start.z) > 0.1 * r
            || (a.pose.y_m - start.y_m).abs() > 0.1 * r
            || !fauna.walk_shunned(id, SP).is_empty();
    }
    assert!(escaped, "pressed {pressed}/{bound} ticks without moving or shunning: {:?}, {:?}", animal(&fauna, id), fauna.steer.get(&id));
}

/// Standing on a ledge above remembered fruit, the old wall-centre target is exactly
/// the body's position: no forward, no turn, and neither mouth reach nor a mount.
#[test]
fn juvenile_beside_unreachable_vine_replans_instead_of_holding_zero_actions() {
    let (world, mut flora, faces) = vine_wall_in_fruit();
    let c = freeze_config();
    let bound = freeze_ticks(&c);
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let id = on_ground(&mut fauna, &world, SP, 6, 2, juvenile_stores());
    let start = animal(&fauna, id).pose;
    assert!(start.y_m > f64::from(faces.last().unwrap().y + 1) * V, "fixture: below the ledge, outside the mouth");
    remember_vine(&mut fauna, id, &world, *faces.last().unwrap());
    use_teacher(&mut fauna, id);
    let mut escaped = false;
    let mut zero = 0;
    for _ in 0..bound {
        fauna.step(&world, &mut flora);
        let a = animal(&fauna, id);
        zero += usize::from(a.founder_state.held.forward == 0.0 && a.founder_state.held.turn == 0.0);
        escaped |= (a.pose.x - start.x).hypot(a.pose.z - start.z) > 0.1 * V
            || (a.pose.y_m - start.y_m).abs() > 0.1 * V
            || !fauna.walk_shunned(id, SP).is_empty();
    }
    assert!(escaped, "held zero actions {zero}/{bound} ticks without moving or shunning: {:?}, {:?}", animal(&fauna, id), fauna.steer.get(&id));
}

/// A nearby low bellwing is within the catch reflex's horizontal and height window,
/// but behind the locked wall heading. The old catch turn overrides every
/// escape with zero forward, although turning has no physical effect while attached.
#[test]
fn juvenile_clinging_escape_does_not_turn_forever_with_zero_forward() {
    for initial in [MobilityMode::Climb, MobilityMode::Perched] {
        let (world, mut flora, faces) = vine_wall_in_fruit();
        for face in faces {
            assert!(flora.take_fruit(face, 1e9).is_some());
        }
        let c = freeze_config();
        let bound = freeze_ticks(&c);
        let mut fauna = Fauna::new(c);
        fauna.set_births_enabled(false);
        let id = on_ground(&mut fauna, &world, SP, 5, 2, juvenile_stores());
        let k = fauna.animals.iter().position(|a| a.id == id).unwrap();
        let r = fauna.config().founder(SP).body_at(fauna.animals[k].body).footprint_radius();
        let a = &mut fauna.animals[k];
        a.pose.x = 6.0 * V - r;
        a.pose.y_m = 6.0 * V;
        a.mobility.mode = initial;
        a.mobility.falling = false;
        a.mobility.perch = Some(crate::Perch {
            kind: crate::PerchKind::VineFace,
            anchor: PerchAnchor::Vine(Face::new(6, 6, 2, FaceDir::NegX)),
        });
        a.founder_state.held.forward = 1.0;
        a.founder_state.feedback.attempted_equivalent = 1.0;
        a.stomach = crate::Stomach::default();
        a.eating = true;
        let start = a.pose;
        assert!(fauna.apply(&world, Command::IntroduceAloft {
            pose: Pose { x: start.x - 0.3, z: start.z, y_m: start.y_m, heading_rad: 0.0 },
            founder: Founder::Bellwing,
            stores: StartingStores::FULL,
        }));
        let prey = fauna.view().ledger.births - 1;
        hold(&mut fauna, prey, 0.0, 0.0);
        assert!(crate::is_prey_of(fauna.config(), &animal(&fauna, id), &animal(&fauna, prey)), "fixture: the catch reflex considers this low bellwing");
        let catch = crate::hunt::catch_step(&mut fauna, k, &world.view(), &flora.view());
        assert!(catch.is_none(), "{initial:?}: a turn cannot bring this prey into a vine-locked mouth: {catch:?}");
        use_teacher(&mut fauna, id);
        let mut escaped = false;
        let mut futile_turns = 0;
        for _ in 0..bound {
            fauna.step(&world, &mut flora);
            let a = animal(&fauna, id);
            futile_turns += usize::from(a.founder_state.held.forward == 0.0 && a.founder_state.held.turn != 0.0);
            escaped |= (a.pose.x - start.x).hypot(a.pose.z - start.z) > 0.1 * V
                || (a.pose.y_m - start.y_m).abs() > 0.1 * V;
        }
        assert!(fauna.view().animal(prey).is_some(), "fixture: a bellwing behind the face is not a reachable meal");
        assert!(escaped, "{initial:?}: futile turn {futile_turns}/{bound} ticks with no displacement: {:?}, {:?}", animal(&fauna, id), fauna.steer.get(&id));
    }
}

/// Give fruit an explicit stock scale without asserting today's tuned K. A controlled
/// quarter-stock mouthful must be halved at E = K, in the actual bite
/// as well as the forage offer; foliage keeps its existing core-stock formula.
#[test]
fn fruit_scale_bite_and_offer_use_the_configured_k_while_foliage_keeps_its_rule() {
    let mut cfg = FaunaConfig::default();
    let stock = 2e-4;
    cfg.founders[SP.index()].fruit_bite_half_stock = Some(stock);
    let period = cfg.manifest(SP).controller_period_s;
    let body = cfg.founder(SP).core.body_max;
    cfg.founders[SP.index()].gut.s0 = 0.0;
    cfg.founders[SP.index()].gut.bite_per_body_s = stock / (4.0 * body * period);
    let mut plants = fruit_flora();
    plants.lanternberry.fruit_cap = stock;
    plants.lanternberry.fruit_per_s = stock / DT;
    let mut world = noon_plain(16, 20, 8);
    let mut flora = Flora::new(plants);
    let site = fruiting(&mut flora, &mut world, 8, 4, 60);
    let available = stand_fruit(&flora, site);
    assert!((available - stock).abs() <= 1e-12 * stock.max(1.0), "fixture: one K of ripe fruit");
    let mut fauna = Fauna::new(cfg);
    fauna.set_births_enabled(false);
    let (x, z) = centre(8, 4);
    let id = aloft(&mut fauna, &world, x, z, crown_top(&flora, site), HUNGRY_ADULT);
    hold(&mut fauna, id, 0.0, 0.0);
    steps(&mut fauna, &world, &mut flora, 1);
    assert_eq!(perched_on(&fauna, id), Some(site));
    hold(&mut fauna, id, 0.0, 1.0);
    let before = fauna.ledger.eaten_by_food[Food::Fruit.index()];
    let mut expected = 0.0;
    let mut expected_offer = 0.0;
    for _ in 0..fauna.config().manifest(SP).cadence_ticks().max(1) + 1 {
        let a = animal(&fauna, id);
        let phys = fauna.config().founder(SP);
        let speed = phys.gut.throughput_factor(a.body, phys.core.body_max);
        let want = phys.gut.bite_per_body_s * speed * a.body * period;
        let saturation = available / (available + phys.bite_half_stock_for(Food::Fruit));
        let room = (phys.gut.capacity(a.body) - a.stomach.organic()).max(0.0);
        let reaches = (1.0 - phys.bite_spill_fraction.clamp(0.0, 1.0)).max(1e-12);
        expected = (want * saturation).min(room / reaches).min(available);
        let fill = phys.gut.full_tolerance * phys.gut.capacity(a.body) * period
            / (phys.gut.digestion_s[Food::Fruit.index()] / speed);
        expected_offer = want * saturation / fill;
        let offered = crate::step::forage::offer(&a, phys, Food::Fruit, period, 1.0, available);
        assert!((offered - expected_offer).abs() <= 1e-12 * expected_offer.max(1.0));
        fauna.step(&world, &mut flora);
        if fauna.ledger.eaten_by_food[Food::Fruit.index()] > before { break; }
    }
    let taken = fauna.ledger.eaten_by_food[Food::Fruit.index()] - before;
    assert!(taken > 0.0 && expected > 0.0 && expected_offer > 0.0, "the configured mouthful is offered and bitten");
    assert!((taken - expected).abs() <= 1e-10 * expected.max(1.0), "fruit bite {taken}, predicted {expected} from configured K");
    assert!((available - stand_fruit(&flora, site) - taken).abs() <= 1e-12, "the bite draws that stock");

    let phys = *fauna.config().founder(Founder::Browser);
    let mut grazer = animal(&fauna, id);
    grazer.body = phys.core.body_max;
    let speed = phys.gut.throughput_factor(grazer.body, phys.core.body_max);
    let want = phys.gut.bite_per_body_s * speed * grazer.body * period;
    let tau = phys.gut.digestion_s[Food::Foliage.index()] / speed;
    let fill = phys.gut.full_tolerance * phys.gut.capacity(grazer.body) * period / tau;
    let predicted = want * available / (available + phys.core.bite_half_stock) / fill;
    let got = crate::step::forage::offer(&grazer, &phys, Food::Foliage, period, 1.0, available);
    assert!((got - predicted).abs() <= 1e-12 * predicted.max(1.0), "foliage offer {got}, existing rule {predicted}");
}

#[test]
fn fruit_stock_override_falls_back_and_leaves_other_foods_and_lineages_alone() {
    let mut cfg = FaunaConfig::default();
    cfg.founders[SP.index()].fruit_bite_half_stock = None;
    let base = cfg.clone();
    cfg.founders[SP.index()].fruit_bite_half_stock = Some(2e-4);
    for founder in Founder::ALL {
        for food in Food::ALL {
            let before = base.founder(founder).bite_half_stock_for(food);
            let after = cfg.founder(founder).bite_half_stock_for(food);
            if founder == SP && food == Food::Fruit {
                assert_eq!(after, cfg.founder(SP).fruit_bite_half_stock.unwrap());
            } else {
                assert_eq!(after, before, "{founder:?}, {food:?}: unchanged by a fruit override");
            }
        }
    }
    assert_eq!(base.founder(SP).bite_half_stock_for(Food::Fruit), base.founder(SP).core.bite_half_stock, "an absent override inherits core K");
    cfg.founders[SP.index()].fruit_bite_half_stock = Some(0.0);
    assert_eq!(cfg.founder(SP).bite_half_stock_for(Food::Fruit), 0.0, "zero is an explicit unlimited-stock-scale override");
}

#[test]
fn lineage_give_up_override_can_inherit_enable_and_disable_the_patch_rule() {
    let world = plain(16, 20, 5);
    for (shared, own, gives_up) in [(0.5, None, true), (0.0, None, false), (0.0, Some(0.5), true), (0.5, Some(0.0), false)] {
        let mut cfg = freeze_config();
        cfg.teachers.give_up_fraction = shared;
        cfg.teachers.give_up_min_s = 0.0;
        cfg.teachers.give_up_window_s = 0.0;
        cfg.founders[SP.index()].give_up_fraction = own;
        let period = cfg.manifest(SP).cadence_ticks().max(1);
        let mut fauna = Fauna::new(cfg);
        let id = on_ground(&mut fauna, &world, SP, 3, 2, juvenile_stores());
        let k = fauna.animals.iter().position(|a| a.id == id).unwrap();
        let still_feeding = Actions { forward: 0.0, turn: 0.0, feed: 1.0 };
        fauna.animals[k].eating = true;
        fauna.animals[k].founder_state.held = still_feeding;
        let phys = *fauna.config().founder(SP);
        let ring = f64::from(world.config().width) * V;
        let mut left = false;
        for _ in 0..4 {
            // A zero offer is a hand-built exhausted patch, independently of every K.
            crate::step::forage::note_bite(&mut fauna, k, &phys, Food::Fruit, period as f64 * DT, 0.0, 1.0, None);
            left |= crate::step::forage::leave(&mut fauna, k, still_feeding, ring).is_some();
            fauna.tick += period;
        }
        assert_eq!(left, gives_up, "shared {shared}, lineage {own:?}");
    }
}

struct StillFruitTeacher;

impl crate::Controller for StillFruitTeacher {
    fn drive(&mut self, _: &[f64]) -> crate::Response {
        crate::Response::Bounded(Actions::REST)
    }
    fn reset(&mut self) {}
    fn layer(&self) -> crate::Layer { crate::Layer::Heuristic }
}

/// An explicit zero-effort teacher isolates the fruit reflex's timer. The normal
/// controller pipeline must count a fruitless vine hold and move out of it.
#[test]
fn a_hungry_vine_climber_or_perch_replans_after_the_configured_stall_clock() {
    for initial in [MobilityMode::Climb, MobilityMode::Perched] {
        let (world, mut flora, faces) = vine_wall_in_fruit();
        for face in faces { assert!(flora.take_fruit(face, 1e9).is_some()); }
        let cfg = freeze_config();
        let threshold = cubarium_voxel::ticks_at_least_one(cfg.teachers.press_replan_s);
        let period = cfg.manifest(SP).cadence_ticks().max(1);
        let mut fauna = Fauna::new(cfg);
        let id = on_ground(&mut fauna, &world, SP, 5, 2, juvenile_stores());
        let k = fauna.animals.iter().position(|a| a.id == id).unwrap();
        let r = fauna.config().founder(SP).body_at(fauna.animals[k].body).footprint_radius();
        let a = &mut fauna.animals[k];
        a.pose.x = 6.0 * V - r;
        a.pose.y_m = 6.0 * V;
        a.mobility.mode = initial;
        a.mobility.perch = Some(crate::Perch {
            kind: crate::PerchKind::VineFace,
            anchor: PerchAnchor::Vine(Face::new(6, 6, 2, FaceDir::NegX)),
        });
        a.stomach = crate::Stomach::default();
        a.eating = true;
        let start = a.pose;
        assert!(fauna.set_controller(id, Box::new(StillFruitTeacher)));
        let mut replanned = None;
        for elapsed in 1..=threshold + 3 * period {
            fauna.step(&world, &mut flora);
            let a = animal(&fauna, id);
            if (a.pose.x - start.x).hypot(a.pose.z - start.z) > 1e-12 || (a.pose.y_m - start.y_m).abs() > 1e-12 {
                replanned = Some(elapsed);
                break;
            }
        }
        let elapsed = replanned.expect("the fruit approach replans an unproductive vine hold");
        assert!(elapsed >= threshold && elapsed <= threshold + 3 * period, "{initial:?}: elapsed {elapsed}, configured clock {threshold}");
        if initial == MobilityMode::Climb { assert!(animal(&fauna, id).mobility.falling, "a stalled climber descends"); }
    }
}

// ------------------------------------------------------------------ juvenile wade pockets (SP-FREEZE2)

/// Standing water `depth_m` deep over column `(x, z)` of a plain (support face `y = 2`), in
/// the cells above it. The fauna never steps the world, so it stays as laid.
fn pool(world: &mut World, x: i64, z: u32, depth_m: f64) {
    let c = world.config().clone();
    let cell = c.voxel_volume();
    let mut left = depth_m * c.voxel_m * c.voxel_m;
    let mut y = 3;
    while left > 1e-15 {
        let v = left.min(cell);
        world.apply(WorldCommand::AddWater { x, y, z, volume_m3: v });
        left -= v;
        y += 1;
    }
    let got = world.view().water_depth_m(x, 2, z);
    assert!((got - depth_m).abs() < 1e-9, "fixture: {got} m of water at ({x}, {z}), wanted {depth_m}");
}

/// A 96 × 40 plain with a lake over columns `30..=60` (every `z`) `lake_m` deep; the columns
/// in `spot` are `spot_m` deep instead. Dry land either side, open to any body.
fn lake(lake_m: f64, spot: &[(i64, u32)], spot_m: f64) -> World {
    let mut world = plain(96, 12, 40);
    for x in 30..=60 {
        for z in 0..40 {
            pool(&mut world, x, z, if spot.contains(&(x, z)) { spot_m } else { lake_m });
        }
    }
    world
}

/// The juvenile's wade and drown depths, and the adult's wade depth.
fn juvenile_depths(c: &FaunaConfig) -> (f64, f64, f64) {
    let p = c.founder(SP);
    let g = p.body_at(p.core.body_max * juvenile_stores().body);
    (p.wade_depth_m(&g), p.drown_depth_m(&g), p.wade_depth_m(&p.adult_body()))
}

/// Run the juvenile on its heuristic until it stands on dry land outside the lake, at most
/// `cap` ticks; the tick it got there.
fn walks_ashore(fauna: &mut Fauna, world: &World, flora: &mut Flora, id: u64, cap: usize) -> Option<usize> {
    for t in 0..cap {
        fauna.step(world, flora);
        let a = *fauna.view().animal(id).expect("alive: it never drowns on the way");
        if !(30..=60).contains(&a.site.x) && world.view().water_depth_m(i64::from(a.site.x), a.site.y, a.site.z) <= 0.0 {
            return Some(t);
        }
    }
    None
}

/// WADE-OUT, a juvenile's own pocket: a shallow on a lake its adult wades, ringed by water
/// over the juvenile's depth. The adult's route graph has it on open ground, so DROP never
/// finds it cornered, and the shallows reflex never fires (it stands in water it wades);
/// it wandered the shallow for good. Held its lineage's `drop.after_s`, it wades out.
#[test]
fn a_juvenile_on_a_shallow_ringed_by_water_over_its_depth_wades_ashore() {
    let mut c = freeze_config();
    c.teachers.press_replan_s = 3.0;
    let (wade, drown, adult) = juvenile_depths(&c);
    assert!(adult > wade && drown > wade, "fixture: an adult wades deeper and it drowns deeper than it wades");
    let lake_m = wade + 0.5 * (adult.min(drown) - wade);
    let spot = [(45, 20), (46, 20), (45, 21), (46, 21)];
    let world = lake(lake_m, &spot, 0.5 * wade);
    let mut flora = Flora::new(barren_flora());
    let after = c.founder(SP).drop.after_s;
    let walk = 32.0 * V / (c.founder(SP).cruise_body_lengths_per_s * c.founder(SP).adult_length_m);
    let cap = ((after + 10.0 * walk + 10.0) / DT).ceil() as usize;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let id = on_ground(&mut fauna, &world, SP, 45, 20, juvenile_stores());
    use_teacher(&mut fauna, id);
    let t = walks_ashore(&mut fauna, &world, &mut flora, id, cap);
    assert!(t.is_some(), "still in the lake after {cap} ticks: {:?}, steer {:?}", animal(&fauna, id), fauna.steer.get(&id));
}

/// WADE-OUT, out of its depth with the shallows past deeper water: the motor lets a body
/// out of its depth step no deeper, so the shallows reflex's straight walk to the nearest
/// face it wades was refused at its first step and it stood there for good.
#[test]
fn a_juvenile_out_of_its_depth_with_deeper_water_round_it_wades_ashore() {
    let mut c = freeze_config();
    c.teachers.press_replan_s = 3.0;
    let (wade, drown, _) = juvenile_depths(&c);
    let here = wade + 0.25 * (drown - wade);
    let around = wade + 0.75 * (drown - wade);
    assert!(around - here > crate::body::LEVEL_TOLERANCE_M, "fixture: the water round it is a real step deeper");
    let world = lake(around, &[(45, 20)], here);
    let mut flora = Flora::new(barren_flora());
    let after = c.founder(SP).drop.after_s;
    let walk = 32.0 * V / (c.founder(SP).cruise_body_lengths_per_s * c.founder(SP).adult_length_m);
    let cap = ((after + 10.0 * walk + 10.0) / DT).ceil() as usize;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let id = on_ground(&mut fauna, &world, SP, 45, 20, juvenile_stores());
    use_teacher(&mut fauna, id);
    let t = walks_ashore(&mut fauna, &world, &mut flora, id, cap);
    assert!(t.is_some(), "still in the lake after {cap} ticks: {:?}, steer {:?}", animal(&fauna, id), fauna.steer.get(&id));
}

/// WADE-OUT never takes a body under water it drowns in for longer than its drown clock: a
/// shallow ringed by water over its drown depth wider than it can cross holds it, alive.
#[test]
fn a_juvenile_ringed_by_drowning_water_too_wide_to_cross_stays_and_lives() {
    let mut c = freeze_config();
    c.teachers.press_replan_s = 3.0;
    // A drown clock too short to cross the lake's half-width at cruise.
    let (wade, drown, _) = juvenile_depths(&c);
    let cruise = c.founder(SP).cruise_body_lengths_per_s * c.founder(SP).adult_length_m;
    c.founders[SP.index()].drown_after_s = 4.0 * V / cruise;
    let spot = [(45, 20), (46, 20), (45, 21), (46, 21)];
    let world = lake(drown + V, &spot, 0.5 * wade);
    let mut flora = Flora::new(barren_flora());
    let after = c.founder(SP).drop.after_s;
    let cap = ((2.0 * after + 10.0) / DT).ceil() as usize;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let id = on_ground(&mut fauna, &world, SP, 45, 20, juvenile_stores());
    use_teacher(&mut fauna, id);
    assert_eq!(walks_ashore(&mut fauna, &world, &mut flora, id, cap), None, "no way out it survives");
    let a = animal(&fauna, id);
    assert!(spot.contains(&(i64::from(a.site.x), a.site.z)), "it stays on the shallow: {:?}", a.site);
}
