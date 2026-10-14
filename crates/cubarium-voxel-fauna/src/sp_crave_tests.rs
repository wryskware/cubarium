//! **SP-CRAVE's laws** (2026-10-14; Wrysk: "yes on seedporter question. i mean animals
//! normally with have different cravings based on needs."):
//!
//! - **the choice weighs foods by need**: a growing young (craving nitrogen) with plain fruit
//!   nearer and nuts farther, both in sight, goes to the nuts; a fed adult (no craving) in the
//!   same scene takes the nearer fruit; a body craving energy (stores near empty, a starving
//!   young too) takes the nearer fruit, which gives it as much as the nuts per organic;
//! - **the check gate opens on a nitrogen craving**: with only plain fruit in sight a body
//!   craving nitrogen walks to check a tree; one without a craving takes the fruit and checks
//!   none;
//! - **a trapped body drops into water it gets out of**: cornered on a pillar in a moat, the
//!   floor past its dry reach and under deep water, it jumps in and reaches dry ground within
//!   a bound from its config's paces; ringed by walls it cannot climb out of, it never jumps.
//!
//! Fixtures set the knobs they rely on (fast fixture crops, the nuts seen from afar where the
//! choice must see them, the fruit's seeds and handling equal so energy per organic is equal);
//! no shipped number, ratio or world hash is asserted.

use crate::{Activity, Command, Fauna, FaunaConfig, Founder, Herding, MobilityMode, SenseTable, StartingStores, Steer};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, TICK_HZ, World};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species as Plant};
use std::f64::consts::PI;

const SP: Founder = Founder::Seedporter;
const EAST: f64 = std::f64::consts::FRAC_PI_2;

fn index(fauna: &Fauna, id: u64) -> usize {
    fauna.animals.iter().position(|a| a.id == id).expect("the body")
}

fn animal(f: &Fauna, id: u64) -> crate::Animal {
    *f.view().animal(id).expect("alive")
}

// ------------------------------------------------------------------ the choice and the gate

mod grove {
    use super::*;

    pub const V: f64 = 0.25;
    pub const ROW: u32 = 12;
    /// The body's column, the bush's (1.5 m west of it, behind it) and the tree's (4 m east,
    /// ahead): no walk to one passes the other.
    pub const BODY_X: i64 = 24;
    pub const BUSH_X: i64 = 18;
    pub const TREE_X: i64 = 40;

    /// A wet soil plain 24 m around at 0.25 m, held at noon.
    pub fn world() -> World {
        let (width, depth) = (96u32, 24u32);
        let mut w = World::empty(VoxelConfig {
            width,
            height: 40,
            depth,
            voxel_m: V,
            seed: 29,
            day: DayClock { start_hour: 12.0, ..DayClock::default() },
            ..VoxelConfig::default()
        });
        let m = cubarium_voxel::Materials::DEFAULT;
        let cap = m.pore_capacity(Material::Soil) * w.config().voxel_volume();
        let fc = m.field_capacity(Material::Soil);
        for z in 0..depth {
            for x in 0..i64::from(width) {
                for y in 1..=2 {
                    w.apply(WorldCommand::AddWater { x, y, z, volume_m3: fc * cap });
                    w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
                }
            }
        }
        w
    }

    /// The lanternberry's fruit and the vaulttree's nuts as two equal fixture crops, eaten at
    /// the same handling with no seeds riding the gut, so each gives the same organic and the
    /// same yield per organic: they differ only in the mineral they carry. `nuts_seen`: the
    /// nuts are seen from afar (no sight limit), else at the configured sight.
    pub fn flora_config(nuts_seen: bool) -> FloraConfig {
        let mut c = FloraConfig::default();
        for sc in [&mut c.vaulttree, &mut c.lanternberry] {
            sc.fruit_per_s = 1e-3;
            sc.fruit_cap = 2e-3;
            sc.fruit_drop_s = 1e9;
            sc.fruit_rest_s = 1e9;
            sc.flower_s = 0.0;
            sc.propagule_rate = 0.0;
            sc.fruit_handling = 1.0;
            sc.fruit_seeds_in_gut = false;
        }
        if nuts_seen {
            c.vaulttree.fruit_sight_m = 0.0;
        }
        c
    }

    /// Every lineage awake, no ground response, no troop pull, no bellwing catching; the
    /// seedporter's senses live and all-round at 16 m, sensed every tick.
    pub fn config() -> FaunaConfig {
        let mut c = FaunaConfig::default();
        for p in &mut c.founders {
            p.active = Activity::Always;
            p.vigilance = crate::Vigilance::default();
            p.catch_bellwings = false;
            p.herding = Herding::default();
        }
        c.perception.remote_hz = f64::from(TICK_HZ);
        c.founders[SP.index()].senses = SenseTable {
            live: true,
            sight_m: 16.0,
            sight_night_m: 16.0,
            sight_still_fraction: 1.0,
            sight_half_angle_deg: 180.0,
            kin_tracks: 1,
            ..SenseTable::default()
        };
        c
    }

    /// The scene: the bush at `BUSH_X` and a full-grown vaulttree at `TREE_X` on the row,
    /// their crops equal. `(world, flora, bush, tree)`.
    pub fn scene(nuts_seen: bool) -> (World, Flora, Site, Site) {
        let mut w = world();
        let mut flora = Flora::new(flora_config(nuts_seen));
        let wood = flora.config().species(Plant::Vaulttree).wood_max;
        assert!(flora.apply(&w, FloraCommand::Seed { x: TREE_X, z: ROW, species: Plant::Vaulttree, wood }));
        let wood = flora.config().species(Plant::Lanternberry).wood_max;
        assert!(flora.apply(&w, FloraCommand::Seed { x: BUSH_X, z: ROW, species: Plant::Lanternberry, wood }));
        let site_of = |flora: &Flora, x: i64| flora.view().stands.iter().find(|s| i64::from(s.site.x) == x && s.site.z == ROW).expect("the stand").site;
        let (bush, tree) = (site_of(&flora, BUSH_X), site_of(&flora, TREE_X));
        for _ in 0..200 {
            flora.step(&mut w);
        }
        let fruit = |flora: &Flora, s: Site| flora.view().stand_at(s).expect("stand").fruit;
        let (b, t) = (fruit(&flora, bush), fruit(&flora, tree));
        assert!(b > 0.0 && t > 0.0, "fixture: both crops hang ({b}, {t})");
        // Equal stocks: the larger is trimmed to the smaller.
        let (big, extra) = if b > t { (bush, b - t) } else { (tree, t - b) };
        if extra > 0.0 {
            assert!(flora.take_stand_fruit(big, extra).is_some());
        }
        assert!((fruit(&flora, bush) - fruit(&flora, tree)).abs() <= 1e-12, "fixture: equal crops");
        let fc = flora.config();
        assert!(
            fc.vaulttree.fruit_mineral_fraction > fc.lanternberry.fruit_mineral_fraction,
            "fixture: the nuts carry more mineral per organic than the bush's fruit"
        );
        (w, flora, bush, tree)
    }

    /// A seedporter at `BODY_X` facing east on its own heuristic, with `stores` (fuel empty).
    pub fn body(f: &mut Fauna, w: &World, stores: StartingStores) -> u64 {
        assert!(f.apply(w, Command::IntroduceFounder { x: BODY_X, z: ROW, founder: SP, stores, heading_rad: EAST }));
        let id = f.animals.last().expect("introduced").id;
        let teachers = f.config.teachers;
        f.controllers.set(id, crate::heuristic(SP, teachers));
        id
    }

    /// A growing young's stores: under its breeding size, its reserve near full.
    pub fn young(c: &FaunaConfig, reserve: f64) -> StartingStores {
        let core = c.founder(SP).core;
        StartingStores { body: 0.5 * (core.body_min + core.birth_body) / core.body_max, reserve }
    }

    /// Run until the body's first fruit bite (at most `cap` ticks): the plant it bit.
    pub fn first_bite(f: &mut Fauna, w: &World, flora: &mut Flora, cap: usize) -> Option<Plant> {
        for _ in 0..cap {
            f.step_with(w, flora, 1);
            let l = &f.ledger;
            for p in [Plant::Lanternberry, Plant::Vaulttree] {
                if l.fruit_eaten_by_plant[p.index()] > 0.0 {
                    return Some(p);
                }
            }
        }
        None
    }
}

/// **A growing young craving nitrogen goes to the nuts past nearer plain fruit; a fed adult in
/// the same scene takes the nearer fruit.** Both crops are in sight and equal, eaten alike;
/// the nuts carry more mineral, so they fund more tissue per organic. The young is under its
/// breeding size with a near-full reserve (no energy craving); the adult is full grown with a
/// full reserve (no craving).
#[test]
fn a_growing_young_goes_to_the_nuts_past_nearer_fruit_and_a_fed_adult_takes_the_nearest() {
    use grove::*;
    let c = config();
    let run = |stores: StartingStores| -> (Option<Plant>, crate::crave::Cravings) {
        let (w, mut flora, _, _) = scene(true);
        let mut f = Fauna::new(c.clone());
        f.set_births_enabled(false);
        let id = body(&mut f, &w, stores);
        let craved = crate::crave::cravings(&f, index(&f, id));
        (first_bite(&mut f, &w, &mut flora, 8000), craved)
    };
    let (young_bite, young) = run(young(&c, 0.9));
    assert_eq!(young.need(), crate::crave::Need::Nitrogen, "fixture: the young craves nitrogen: {young:?}");
    assert_eq!(young_bite, Some(Plant::Vaulttree), "the young goes to the nuts");
    let (adult_bite, adult) = run(StartingStores::FULL);
    assert_eq!(adult.need(), crate::crave::Need::None, "fixture: the fed adult craves nothing: {adult:?}");
    assert_eq!(adult_bite, Some(Plant::Lanternberry), "the fed adult takes the nearer fruit");
}

/// **A body craving energy takes the near energy food over the farther protein one**: an adult
/// with its stores near empty, and a growing young starving (its energy craving over its
/// nitrogen one), each take the nearer fruit: per organic it gives them as much as the nuts.
#[test]
fn a_body_craving_energy_takes_the_near_fruit_over_the_farther_nuts() {
    use grove::*;
    let c = config();
    for (what, stores) in [("an adult", StartingStores { body: 1.0, reserve: 0.05 }), ("a starving young", young(&c, 0.05))] {
        let (w, mut flora, _, _) = scene(true);
        let mut f = Fauna::new(c.clone());
        f.set_births_enabled(false);
        let id = body(&mut f, &w, stores);
        let craved = crate::crave::cravings(&f, index(&f, id));
        assert_eq!(craved.need(), crate::crave::Need::Energy, "fixture: {what} craves energy: {craved:?}");
        assert_eq!(first_bite(&mut f, &w, &mut flora, 8000), Some(Plant::Lanternberry), "{what} takes the nearer fruit");
    }
}

/// **A body craving nitrogen with only plain fruit in sight checks a tree; one without a
/// craving does not.** The nuts hang beyond their sight (the configured sight), in the
/// check's range; the bush is in sight.
#[test]
fn a_nitrogen_craving_body_with_only_plain_fruit_in_sight_checks_a_tree() {
    use grove::*;
    let c = config();
    let run = |stores: StartingStores| -> (bool, Option<Steer>, u64) {
        let (w, mut flora, _, tree) = scene(false);
        let sight = flora.config().vaulttree.fruit_sight_m;
        assert!(sight > 0.0 && f64::from(tree.x - BODY_X as u32) * V > sight + 1.0, "fixture: the nuts are out of sight");
        let mut f = Fauna::new(c.clone());
        f.set_births_enabled(false);
        let id = body(&mut f, &w, stores);
        let mut checked = false;
        let mut first = None;
        for _ in 0..40 {
            f.step_with(&w, &mut flora, 1);
            let s = f.steer_of(id);
            first = first.or(s.filter(|s| matches!(s, Steer::Fruit | Steer::Check)));
            checked |= s == Some(Steer::Check);
        }
        (checked, first, f.check_tally().walks)
    };
    let (checked, first, walks) = run(young(&c, 0.9));
    assert!(checked && walks >= 1, "craving nitrogen it checks a tree (first {first:?}, walks {walks})");
    assert_eq!(first, Some(Steer::Check), "the check comes before the plain fruit");
    let (checked, first, walks) = run(StartingStores::FULL);
    assert_eq!((checked, walks), (false, 0), "without a craving it does not check");
    assert_eq!(first, Some(Steer::Fruit), "it walks to the fruit");
}

// ------------------------------------------------------------------ the drop into water

mod moat {
    use super::*;

    pub const V: f64 = 0.125;
    /// The floor's support face (the pool's and the shore's), the water's surface layer (the
    /// pool's water fills the cells over the floor up to it) and the pillar's top.
    pub const FLOOR: u32 = 2;
    pub const SURFACE: u32 = 6;
    pub const TOP: u32 = 12;
    /// The pillar's centre column; the pillar is 3 × 3, the pool's water `POOL` columns out
    /// round it.
    pub const PX: i64 = 48;
    pub const PZ: u32 = 32;
    pub const POOL: i64 = 4;
    pub const WIDTH: i64 = 96;
    pub const DEPTH: u32 = 64;

    fn rock(w: &mut World, x: i64, z: u32, top: u32) {
        for y in 1..=top {
            w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Rock });
        }
    }

    /// Water over column `(x, z)` from the floor up to `SURFACE`. The fauna never steps the
    /// world, so it stays as laid.
    fn flood(w: &mut World, x: i64, z: u32) {
        let cell = w.config().voxel_volume();
        for y in FLOOR + 1..=SURFACE {
            w.apply(WorldCommand::AddWater { x, y, z, volume_m3: cell });
        }
    }

    /// A rock floor with a 3 × 3 pillar (top `TOP`) standing in a square pool, its water
    /// `SURFACE − FLOOR` layers deep out to `POOL` columns. `shore`: dry floor beyond, which a
    /// body in the water walks out onto; else a wall round the pool higher than the pillar,
    /// which nothing in the water climbs and nothing on the pillar lands on.
    pub fn world(shore: bool) -> World {
        // Wide enough that the shore is an open part of its graph (more nodes than the
        // way-on window, `teachers.way_on_m`, holds).
        let mut w = World::empty(VoxelConfig {
            width: WIDTH as u32,
            height: 24,
            depth: DEPTH,
            voxel_m: V,
            seed: 7,
            day: DayClock { mode: DayMode::Off, ..DayClock::default() },
            ..VoxelConfig::default()
        });
        for z in 0..DEPTH {
            for x in 0..WIDTH {
                let ring = (x - PX).abs().max((i64::from(z) - i64::from(PZ)).abs());
                if ring <= 1 {
                    rock(&mut w, x, z, TOP);
                } else if ring <= POOL {
                    rock(&mut w, x, z, FLOOR);
                    flood(&mut w, x, z);
                } else if ring == POOL + 1 && !shore {
                    rock(&mut w, x, z, TOP + 4);
                } else {
                    rock(&mut w, x, z, FLOOR);
                }
            }
        }
        w
    }

    /// Every lineage awake; a short drop wait; no glide (its start height out of reach), as
    /// on s5 where the glide off the hollow was refused for landing in the water.
    pub fn config() -> FaunaConfig {
        let mut c = FaunaConfig::default();
        for f in Founder::ALL {
            c.founders[f.index()].active = Activity::Always;
            c.founders[f.index()].vigilance = crate::Vigilance::default();
            c.founders[f.index()].herding = Herding::default();
        }
        c.founders[SP.index()].drop.after_s = 1.0;
        c.founders[SP.index()].glider.as_mut().expect("a glider").glide_min_height_m = 100.0;
        c
    }

    /// An adult seedporter on the pillar's centre, facing east, on its own heuristic.
    pub fn place(fauna: &mut Fauna, world: &World) -> u64 {
        assert!(fauna.apply(world, Command::IntroduceFounder { x: PX, z: PZ, founder: SP, stores: StartingStores::FULL, heading_rad: EAST }));
        let id = fauna.animals.last().expect("just placed").id;
        let a = animal(fauna, id);
        assert_eq!(a.site, Site { x: PX as u32, y: TOP, z: PZ }, "fixture: it stands on the pillar");
        let t = fauna.config.teachers;
        fauna.controllers.set(id, crate::controller::heuristic(SP, t));
        id
    }

    /// The time the drop and the swim out may take: the wait, a controller period to see it, a
    /// half turn at the yaw cap, the pillar's half-width at cruise, two periods; then the
    /// wade-out's own time bound for a way across the pool to the shore
    /// (`WADE_TIME_FACTOR` × its walk at cruise + `WADE_TIME_MARGIN_S`) and two periods.
    pub fn bound_s(c: &FaunaConfig) -> f64 {
        let m = c.manifest(SP);
        let period = m.cadence_ticks().max(1) as f64 / f64::from(TICK_HZ);
        let p = c.founder(SP);
        let cruise = p.cruise_body_lengths_per_s * p.adult_length_m;
        let drop = p.drop.after_s + period + PI / m.yaw_cap_rad_per_s + 2.0 * V / cruise + 2.0 * period;
        let way = (2 * POOL) as f64 * V / cruise;
        drop + crate::step::jump::WADE_TIME_FACTOR * way + crate::step::jump::WADE_TIME_MARGIN_S + 2.0 * period
    }
}

/// **A body trapped above water it can get out of drops into it and reaches dry ground in
/// time.** On a pillar in a moat, no glide and no walk off: cornered. The floor round it is in
/// its dry reach but under water deeper than it wades, so no dry jump; the water's surface is
/// within its reach into water. It jumps in, once, booked as a drop counted to the surface,
/// and stands on dry shore within the bound, alive.
#[test]
fn a_body_trapped_above_water_it_can_leave_drops_in_and_reaches_dry_ground() {
    use moat::*;
    let world = world(true);
    let mut flora = Flora::new(FloraConfig::default());
    let c = config();
    let p = *c.founder(SP);
    let body = p.adult_body();
    let (wade, step) = (p.wade_depth_m(&body), p.step_limits(V));
    let surface = crate::surface_m(SURFACE, V);
    let over = crate::surface_m(TOP, V) - surface;
    assert!(over > f64::from(step.down) * V, "fixture: the drop is past its step");
    assert!(over <= p.drop.water_reach_m(&p, &body), "fixture: the surface is in its reach into water");
    assert!(world.view().water_depth_m(PX + 2, FLOOR, PZ) > wade, "fixture: the moat is deeper than it wades");
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let id = place(&mut fauna, &world);
    let a = animal(&fauna, id);
    assert_eq!(
        crate::step::off_graph_exit(&mut fauna, &world.view(), &flora.view(), &a, (0.0, 0.0)),
        crate::step::OffGraph::Cornered,
        "on the pillar it is cornered"
    );
    let bound = bound_s(&c);
    let mut ashore = None;
    let mut wet = false;
    for k in 0..(bound / DT).ceil() as usize {
        fauna.step(&world, &mut flora);
        let a = animal(&fauna, id);
        let depth = world.view().water_depth_m(i64::from(a.site.x), a.site.y, a.site.z);
        wet |= a.site.y < TOP && depth > wade;
        if wet && depth <= wade {
            ashore = Some((k, a));
            break;
        }
    }
    let (_, a) = ashore.unwrap_or_else(|| panic!("in the water and out on dry ground within {bound:.1} s: {:?} (wet {wet})", animal(&fauna, id)));
    assert_eq!(a.mobility.mode, MobilityMode::Ground, "it walks out");
    let l = fauna.view().ledger;
    assert_eq!(l.drops_by_founder[SP.index()], 1, "one drop booked");
    assert!((l.drop_m_by_founder[SP.index()] - over).abs() < 1e-9, "the drop is counted to the surface: {} against {over}", l.drop_m_by_founder[SP.index()]);
    assert!((l.drop_wound_by_founder[SP.index()] - p.drop.wound(&p, over)).abs() < 1e-9, "its wound is from the height over the surface");
}

/// **A body above water with no way out does not drop.** The same pillar, its pool's sides
/// rising straight to the shore past its step: nothing it lands on in the water leads out, so
/// it stays on the pillar, alive, for twice the bound.
#[test]
fn a_body_above_water_with_no_way_out_does_not_drop() {
    use moat::*;
    let world = world(false);
    let mut flora = Flora::new(FloraConfig::default());
    let c = config();
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let id = place(&mut fauna, &world);
    let a = animal(&fauna, id);
    assert_eq!(
        crate::step::off_graph_exit(&mut fauna, &world.view(), &flora.view(), &a, (0.0, 0.0)),
        crate::step::OffGraph::Cornered,
        "fixture: on the pillar it is cornered"
    );
    for _ in 0..(2.0 * bound_s(&c) / DT).ceil() as usize {
        fauna.step(&world, &mut flora);
    }
    let a = animal(&fauna, id);
    assert_eq!(a.site.y, TOP, "it stays on the pillar: {:?}", a.site);
    assert_eq!(fauna.view().ledger.drops_by_founder[SP.index()], 0, "no drop");
}

