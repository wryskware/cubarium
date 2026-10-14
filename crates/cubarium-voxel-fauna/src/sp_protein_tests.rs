//! **SP-PROTEIN's laws** (2026-10-13; written before the fixes, from the wide-world probe of
//! the larva + nuts branch):
//!
//! - **no black hole on a one-column perch**: a seedporter pressed back into the corner of a
//!   perch one column wide, its centre over the column's edge toward the drops, finds its jump
//!   and lands where it can walk on (it stood on one such column for hours: its walk "to the
//!   edge" went back into the wall behind it, so no jump was ever on offer);
//! - **a check walk ends**: one that never arrives is given up within its walking bound
//!   (`check_walk_factor` × `route_detour` × `check_search_m` at its own cruise), and one left
//!   off longer than that bound starts afresh when the body checks again (it kept its first
//!   aim's clock for hours);
//! - **larvae at a fruiting mouth are eaten**: with fruit and larvae both in reach a bite takes
//!   from both, each in proportion to what it holds there (it was fruit first, so a fruiting
//!   vine's own larvae were never eaten).
//!
//! Every bound comes from the config (paces, periods, the drop wait); fixtures set the knobs
//! they rely on; no shipped number, ratio or world hash is asserted.

use crate::{Actions, Activity, Command, Fauna, FaunaConfig, Food, Founder, MobilityMode, Pose, Scripted, StartingStores, Steer};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, TICK_HZ, World};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species as Plant};
use std::f64::consts::PI;

const SP: Founder = Founder::Seedporter;
const BW: Founder = Founder::Bellwing;

fn rock(w: &mut World, xs: std::ops::RangeInclusive<i64>, zs: std::ops::RangeInclusive<u32>, ys: std::ops::RangeInclusive<u32>) {
    for z in zs {
        for x in xs.clone() {
            for y in ys.clone() {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Rock });
            }
        }
    }
}

fn index(fauna: &Fauna, id: u64) -> usize {
    fauna.animals.iter().position(|a| a.id == id).expect("the body")
}

fn ticks(s: f64) -> usize {
    (s / DT).round() as usize
}

// ------------------------------------------------------------------ 1. the one-column perch

mod perch {
    use super::*;

    pub const V: f64 = 0.125;
    /// The support face of a plain column: rock at `y = 1..=2`.
    pub const FLOOR: u32 = 2;
    /// The perch's layer: three over the floor (past the step, short of a glide).
    pub const T: u32 = FLOOR + 3;
    /// The perch column.
    pub const PX: i64 = 20;
    pub const PZ: u32 = 10;

    /// A rock plain with a perch one column wide at `(PX, PZ)`, its top `T`; behind it (`+x`
    /// and `+z`) a wall four layers over the perch, and the floor three layers under it on the
    /// other two sides.
    pub fn world() -> World {
        let mut w = World::empty(VoxelConfig {
            width: 48,
            height: 24,
            depth: 24,
            voxel_m: V,
            seed: 11,
            day: DayClock { mode: DayMode::Off, ..DayClock::default() },
            ..VoxelConfig::default()
        });
        rock(&mut w, 0..=47, 0..=23, 1..=FLOOR);
        rock(&mut w, PX..=PX, PZ..=PZ, FLOOR + 1..=T);
        rock(&mut w, PX + 1..=PX + 1, PZ - 1..=PZ + 1, FLOOR + 1..=T + 4);
        rock(&mut w, PX..=PX + 1, PZ + 1..=PZ + 1, FLOOR + 1..=T + 4);
        w
    }

    /// Every lineage awake at every hour; a short drop wait.
    pub fn config() -> FaunaConfig {
        let mut c = FaunaConfig::default();
        for f in Founder::ALL {
            c.founders[f.index()].active = Activity::Always;
        }
        c.founders[SP.index()].drop.after_s = 1.0;
        c
    }

    /// An adult seedporter on the perch, its centre at the column's corner toward the drops
    /// (its disc touching both walls behind it), facing `heading`, on its own heuristic.
    pub fn place(fauna: &mut Fauna, world: &World, heading: f64) -> u64 {
        assert!(fauna.apply(world, Command::IntroduceFounder { x: 2, z: 2, founder: SP, stores: StartingStores::FULL, heading_rad: heading }));
        let id = fauna.animals.last().expect("just placed").id;
        let k = fauna.animals.len() - 1;
        let a = &mut fauna.animals[k];
        (a.pose.x, a.pose.z, a.pose.heading_rad) = (PX as f64 * V, f64::from(PZ) * V, heading);
        a.site = Site { x: PX as u32, y: T, z: PZ };
        a.pose.y_m = crate::surface_m(T, V);
        let t = fauna.config.teachers;
        fauna.controllers.set(id, crate::controller::heuristic(SP, t));
        id
    }

    /// The time a drop may take once trapped: the wait, a controller period to see it, a half
    /// turn at the yaw cap, two voxels at cruise, and two controller periods.
    pub fn drop_bound_s(c: &FaunaConfig) -> f64 {
        let m = c.manifest(SP);
        let period = m.cadence_ticks().max(1) as f64 / f64::from(TICK_HZ);
        c.founder(SP).drop.after_s + period + PI / m.yaw_cap_rad_per_s + 2.0 * V / m.cruise_m_per_s + 2.0 * period
    }
}

/// **A body pressed into the corner of a one-column perch finds its jump and lands where it
/// walks on.** Its disc is wider than the column, so no walk leaves the perch and no glide
/// starts from it: cornered. Its centre is already over the column's edge toward both drops;
/// the jump down is on offer from where it stands, and within the trap wait and the turn to
/// the edge it jumps, once, onto ground where it is on its graph again.
#[test]
fn a_body_pressed_into_the_corner_of_a_one_column_perch_jumps_down_and_walks_on() {
    use perch::*;
    let world = world();
    let mut flora = Flora::new(FloraConfig::default());
    let c = config();
    let glide = c.founder(SP).glider.expect("a glider").glide_min_height_m;
    assert!(f64::from(T - FLOOR) * V < glide, "fixture: the drops are short of a glide");
    let p = c.founder(SP);
    let body = p.adult_body();
    let (r, headroom, step, wade) = (body.footprint_radius(), body.headroom_voxels(V), p.step_limits(V), p.wade_depth_m(&body));
    assert!(2.0 * r > V, "fixture: the disc is wider than the perch");
    assert!(T - FLOOR > step.down, "fixture: the drops are past the step");
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let id = place(&mut fauna, &world, -PI / 2.0);
    let a = fauna.animals[index(&fauna, id)];
    let view = world.view();
    assert_eq!(
        crate::step::off_graph_exit(&mut fauna, &view, &flora.view(), &a, (0.0, 0.0)),
        crate::step::OffGraph::Cornered,
        "on the perch it is cornered"
    );
    let reach = crate::body::step_voxels(p.drop.reach_m(p, &body), V);
    let own = (r / V).ceil() as u32;
    let g = fauna.routes(&view, SP, 1e6);
    let way = g.drop_way(&view, a.site, (a.pose.x, a.pose.z), r, headroom, step, reach, wade, own, 32);
    assert!(way.is_some(), "a jump is on offer from where it stands");
    let bound = drop_bound_s(&c);
    let mut landed = None;
    for _ in 0..ticks(bound) {
        fauna.step(&world, &mut flora);
        let a = fauna.animals[index(&fauna, id)];
        if a.site.y < T {
            landed = Some(a);
            break;
        }
    }
    let a = landed.unwrap_or_else(|| panic!("it jumps down within {bound:.2} s"));
    assert_eq!(fauna.view().ledger.drops_by_founder[SP.index()], 1, "one drop booked");
    assert_eq!(a.mobility.mode, MobilityMode::Ground, "a jump, not a glide");
    assert_eq!(
        crate::step::off_graph_exit(&mut fauna, &world.view(), &flora.view(), &a, (0.0, 0.0)),
        crate::step::OffGraph::On,
        "where it lands it walks on its graph ({:?})",
        a.site
    );
}

// ------------------------------------------------------------------ 2. a check walk ends

mod walled {
    use super::*;

    pub const V: f64 = 0.25;
    pub const ROW: u32 = 20;
    pub const TREE_X: i64 = 40;
    pub const EAST: f64 = std::f64::consts::FRAC_PI_2;
    pub const HUNGRY: StartingStores = StartingStores { body: 1.0, reserve: 0.3 };

    /// A wet soil plain at 0.25 m held at noon, a full-grown vaulttree at `(TREE_X, ROW)`
    /// hanging fixture nuts, ringed by a rock wall 1.5 m high two columns out: a host in the
    /// check's range that no walk reaches.
    pub fn scene() -> (World, Flora, Site) {
        let (width, depth) = (96u32, 40u32);
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
        let mut fc_ = FloraConfig::default();
        let t = &mut fc_.vaulttree;
        t.fruit_per_s = 1e-3;
        t.fruit_cap = 2e-3;
        t.fruit_drop_s = 1e9;
        t.fruit_rest_s = 1e9;
        t.flower_s = 0.0;
        t.propagule_rate = 0.0;
        fc_.lanternberry.fruit_per_s = 0.0;
        fc_.lanternberry.propagule_rate = 0.0;
        let mut flora = Flora::new(fc_);
        let wood = flora.config().species(Plant::Vaulttree).wood_max;
        assert!(flora.apply(&w, FloraCommand::Seed { x: TREE_X, z: ROW, species: Plant::Vaulttree, wood }));
        let site = flora.view().stands.iter().find(|s| i64::from(s.site.x) == TREE_X && s.site.z == ROW).expect("the stand").site;
        // The ring, six layers over the soil, a voxel and a body's length beyond its widest
        // crown layer (a body under a crown's lowest layer climbs into it).
        let crown = flora.view().layers_at(site).map(|l| l.radius_m).fold(0.0, f64::max);
        let out = ((crown + 0.5) / V).ceil() as i64 + 1;
        let (x0, x1, z0, z1) = (TREE_X - out, TREE_X + out, ROW as i64 - out, ROW as i64 + out);
        assert!(z0 >= 0 && z1 < i64::from(depth), "fixture: the ring fits ({out} columns out)");
        let (z0, z1) = (z0 as u32, z1 as u32);
        for x in x0..=x1 {
            for z in z0..=z1 {
                if x == x0 || x == x1 || z == z0 || z == z1 {
                    rock(&mut w, x..=x, z..=z, 3..=8);
                }
            }
        }
        for _ in 0..200 {
            flora.step(&mut w);
        }
        assert!(flora.view().stand_at(site).expect("the stand").fruit > 0.0, "fixture: the walled tree hangs nuts");
        (w, flora, site)
    }

    /// Every lineage awake, no ground response, no troop pull, no bellwing catching; the
    /// pressed-walk re-plan off, so only the check's own bound can end the walk.
    pub fn config() -> FaunaConfig {
        let mut c = FaunaConfig::default();
        for p in &mut c.founders {
            p.active = Activity::Always;
            p.vigilance = crate::Vigilance::default();
            p.catch_bellwings = false;
            p.herding = crate::Herding::default();
        }
        c.teachers.press_replan_s = 1e9;
        c
    }

    /// A hungry seedporter 2 m west of the ring facing the tree, on its own heuristic, its
    /// check walk aimed at the walled tree (set out at `since`).
    pub fn checker(f: &mut Fauna, w: &World, flora: &Flora, site: Site, since: u64) -> u64 {
        let crown = flora.view().layers_at(site).map(|l| l.radius_m).fold(0.0, f64::max);
        let out = ((crown + 0.5) / V).ceil() + 1.0;
        let x = ((f64::from(site.x) - out) * V - 2.0) / V;
        assert!((f64::from(site.x) - x) * V < f.config.founder(SP).glider.expect("a glider").check_search_m, "fixture: the tree is in the check's range");
        assert!(f.apply(w, Command::IntroduceFounder { x: x.floor() as i64, z: ROW, founder: SP, stores: HUNGRY, heading_rad: EAST }));
        let id = f.animals.last().expect("introduced").id;
        let teachers = f.config.teachers;
        f.controllers.set(id, crate::heuristic(SP, teachers));
        let host = crate::larva::LarvaHost::Stand(flora.view().stand_at(site).expect("the stand").id);
        let m = f.tree_checks.entry(id).or_default();
        m.aim = Some(host);
        m.aim_since = since;
        id
    }

    /// The walking bound, seconds, read from the config: `check_walk_factor` ×
    /// `route_detour` × `check_search_m` at the body's own cruise.
    pub fn bound_s(f: &Fauna, id: u64) -> f64 {
        let a = f.animals[index(f, id)];
        let p = f.config.founder(SP);
        let g = p.glider.expect("a glider");
        let length = crate::body_of(&f.config, &a).expect("a body").length_m;
        g.check_walk_factor * f.config.teachers.route_detour.max(1.0) * g.check_search_m / (p.cruise_body_lengths_per_s * length)
    }

    pub fn period_s(f: &Fauna) -> f64 {
        f.config.manifest(SP).cadence_ticks().max(1) as f64 / f64::from(TICK_HZ)
    }
}

/// **A check walk that never arrives is given up within its walking bound.** Aimed at a
/// walled tree in range that no walk reaches (the pressed-walk re-plan off), the body walks
/// at it and presses; within the bound and a controller period of setting out it gives the
/// walk up: the aim goes, the give-up is counted, and the tree is remembered as one not to
/// check again (held no food). Before the bound it still holds the aim.
#[test]
fn a_check_walk_that_never_arrives_is_given_up_within_its_walking_bound() {
    use walled::*;
    let (w, mut flora, site) = scene();
    let mut f = Fauna::new(config());
    f.set_births_enabled(false);
    let id = checker(&mut f, &w, &flora, site, 0);
    let host = f.tree_checks[&id].aim.expect("aimed");
    let (bound, period) = (bound_s(&f, id), period_s(&f));
    let mut set_out = None;
    let mut gave_up = None;
    for k in 0..ticks(4.0 * bound + 10.0) {
        f.step_with(&w, &mut flora, 1);
        if set_out.is_none() && f.steer_of(id) == Some(Steer::Check) {
            set_out = Some(k);
        }
        if f.tree_checks.get(&id).is_some_and(|m| m.aim != Some(host)) {
            gave_up = Some(k);
            break;
        }
    }
    let set_out = set_out.expect("it walks its check");
    let gave_up = gave_up.unwrap_or_else(|| panic!("it gives the walk up (bound {bound:.1} s)"));
    let took = (gave_up - set_out) as f64 * DT;
    assert!(took <= bound + 2.0 * period, "given up within its bound: {took:.2} s against {bound:.2} s");
    assert!(took >= bound - 2.0 * period, "and not before it: {took:.2} s against {bound:.2} s");
    assert!(f.check_tally().give_ups >= 1, "the give-up is counted: {:?}", f.check_tally());
    let now = f.tick;
    let memory = cubarium_voxel::ticks_at_least_one(f.config.founder(SP).glider.expect("a glider").check_memory_s);
    let recalled = crate::treecheck::recall(&f, id, host, now, memory).expect("the tree is remembered");
    assert!(!recalled.held_food, "as one not to check again within the memory");
}

/// **A check walk left off longer than its bound starts afresh.** A body walking its check is
/// held still by another driver for twice the bound (no check runs); back on its heuristic,
/// its next check walk to the same tree sets out anew: its clock starts at that sample, it is
/// counted as a walk set out on, and not as a switch.
#[test]
fn a_check_walk_left_off_longer_than_its_bound_starts_afresh() {
    use walled::*;
    let (w, mut flora, site) = scene();
    let mut f = Fauna::new(config());
    f.set_births_enabled(false);
    let id = checker(&mut f, &w, &flora, site, 0);
    let (bound, period) = (bound_s(&f, id), period_s(&f));
    let host = f.tree_checks[&id].aim.expect("aimed");
    let mut first = None;
    for _ in 0..ticks(0.25 * bound) {
        f.step_with(&w, &mut flora, 1);
        if f.steer_of(id) == Some(Steer::Check) && first.is_none() {
            first = Some(f.tick);
        }
    }
    assert!(first.is_some(), "it walks its check");
    let walks = f.check_tally().walks;
    let switches = f.check_tally().switches;
    assert!(f.set_controller(id, Box::new(Scripted::new(vec![Actions { forward: 0.0, turn: 0.0, feed: 0.0 }]))));
    for _ in 0..ticks(2.0 * bound) {
        f.step_with(&w, &mut flora, 1);
    }
    let teachers = f.config.teachers;
    f.controllers.set(id, crate::heuristic(SP, teachers));
    let resumed = f.tick;
    let mut again = None;
    for _ in 0..ticks(0.25 * bound) {
        f.step_with(&w, &mut flora, 1);
        if f.steer_of(id) == Some(Steer::Check) {
            again = Some(f.tick);
            break;
        }
    }
    let again = again.expect("it checks again");
    let m = &f.tree_checks[&id];
    assert_eq!(m.aim, Some(host), "the same tree");
    assert!(m.aim_since >= resumed, "its clock starts afresh: set out at {} (resumed at {resumed}, checked at {again})", m.aim_since);
    assert!(((again - m.aim_since) as f64) * DT <= 2.0 * period, "at the sample it checked again");
    assert_eq!(f.check_tally().walks, walks + 1, "a walk set out on");
    assert_eq!(f.check_tally().switches, switches, "not a switch");
}

// ------------------------------------------------------------------ 3. larvae at a fruiting mouth

/// **With fruit and larvae both at its mouth, a bite takes from both in proportion to what
/// each holds there.** A hungry seedporter perched in a vaulttree hanging nuts and hosting
/// hatched larvae, holding feed, both bites saturated (half-stocks near zero, the nut taken as
/// plain fruit): at its first bite the larvae and the nuts it took stand as their stocks at
/// the mouth did.
#[test]
fn larvae_and_fruit_at_one_mouth_are_bitten_in_proportion_to_what_each_holds() {
    const V: f64 = 0.25;
    let mut world = {
        let mut w = World::empty(VoxelConfig {
            width: 40,
            height: 64,
            depth: 16,
            voxel_m: V,
            seed: 7,
            day: DayClock { start_hour: 12.0, ..DayClock::default() },
            ..VoxelConfig::default()
        });
        let m = cubarium_voxel::Materials::DEFAULT;
        let cap = m.pore_capacity(Material::Soil) * w.config().voxel_volume();
        let fc = m.field_capacity(Material::Soil);
        for z in 0..16 {
            for x in 0..40 {
                for y in 1..=2 {
                    w.apply(WorldCommand::AddWater { x, y, z, volume_m3: fc * cap });
                    w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
                }
            }
        }
        w
    };
    let mut fcfg = FloraConfig::default();
    fcfg.vaulttree.fruit_per_s = 1e-3;
    fcfg.vaulttree.fruit_cap = 2e-3;
    fcfg.vaulttree.fruit_drop_s = 1e9;
    fcfg.vaulttree.fruit_rest_s = 1e9;
    fcfg.vaulttree.flower_s = 0.0;
    fcfg.vaulttree.fruit_handling = 1.0;
    fcfg.vaulttree.propagule_rate = 0.0;
    let mut flora = Flora::new(fcfg);
    let wood = flora.config().species(Plant::Vaulttree).wood_max;
    assert!(flora.apply(&world, FloraCommand::Seed { x: 20, z: 8, species: Plant::Vaulttree, wood }));
    let (site, stand_id) = {
        let fv = flora.view();
        let s = fv.stands.iter().find(|s| s.site.x == 20 && s.site.z == 8).expect("the stand");
        (s.site, s.id)
    };
    for _ in 0..200 {
        flora.step(&mut world);
    }
    assert!(flora.view().stand_at(site).expect("the stand").fruit > 0.0, "fixture: nuts hang");
    let mut c = FaunaConfig::default();
    c.founders[SP.index()].larva_bite_half_stock = Some(1e-15);
    c.founders[SP.index()].fruit_bite_half_stock = Some(1e-15);
    c.founders[SP.index()].catch_bellwings = false;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let egg = fauna.config().founder(BW).core.reproduction.egg_organic;
    let inc = fauna.config().founder(BW).core.reproduction.incubation_s;
    let sc = fauna.config().founder(BW).core;
    let organic = 20.0 * 20.0 * egg;
    let cohort = crate::larva::Cohort { count: 20.0, organic, mineral: sc.n_tissue * organic, energy: sc.energy_density * organic, age_s: inc + 100.0, unfed_s: 0.0 };
    fauna.introduce_larvae(BW, crate::larva::LarvaHost::Stand(stand_id), site, None, cohort);
    let top = flora.view().layers_at(site).filter(|l| l.foliage_index.is_some()).map(|l| l.cell).max().expect("a crown");
    let (x, z) = ((f64::from(site.x) + 0.5) * V, (f64::from(site.z) + 0.5) * V);
    assert!(fauna.apply(
        &world,
        Command::IntroduceAloft { pose: Pose { x, z, heading_rad: 0.0, y_m: (top + 1) as f64 * V }, founder: SP, stores: StartingStores { body: 1.0, reserve: 0.3 } },
    ));
    let id = fauna.view().ledger.births - 1;
    assert!(fauna.set_controller(id, Box::new(Scripted::new(vec![Actions { forward: 0.0, turn: 0.0, feed: 1.0 }]))));
    for _ in 0..40 {
        if fauna.animals[index(&fauna, id)].mobility.mode == MobilityMode::Perched {
            break;
        }
        fauna.step(&world, &mut flora);
    }
    assert_eq!(fauna.animals[index(&fauna, id)].mobility.mode, MobilityMode::Perched, "fixture: perched in the crown");
    let bites = |f: &Fauna| f.view().ledger.bites_by_founder_food[SP.index()];
    let mut seen = None;
    for _ in 0..400 {
        let a = fauna.animals[index(&fauna, id)];
        let phys = *fauna.config.founder(SP);
        let fv = flora.view();
        let fruit = crate::step::glider_fruit_in_reach(&world.view(), &fv, &a, &phys).map(|(_, s)| s);
        let larvae = crate::step::glider_larva_in_reach(&fauna, &world.view(), &fv, &a, &phys).map(|(_, s)| s);
        let before = (bites(&fauna), fauna.ledger.fruit_eaten_by_plant[Plant::Vaulttree.index()], fauna.ledger.larva.eaten_seedporter_out.organic);
        fauna.step(&world, &mut flora);
        let after = bites(&fauna);
        if after[Food::Fruit.index()] > before.0[Food::Fruit.index()] || after[Food::Larva.index()] > before.0[Food::Larva.index()] {
            let nut = fauna.ledger.fruit_eaten_by_plant[Plant::Vaulttree.index()] - before.1;
            let lar = fauna.ledger.larva.eaten_seedporter_out.organic - before.2;
            seen = Some((fruit, larvae, nut, lar));
            break;
        }
    }
    let (fruit, larvae, nut, lar) = seen.expect("it bites");
    let (fruit, larvae) = (fruit.expect("fixture: nuts at the mouth"), larvae.expect("fixture: larvae at the mouth"));
    assert!(lar > 0.0, "the bite takes larvae too ({lar} larva, {nut} nut)");
    assert!(nut > 0.0, "and nuts ({lar} larva, {nut} nut)");
    let want = larvae / fruit;
    let got = lar / nut;
    assert!((got - want).abs() <= 1e-6 * want.max(1.0), "in proportion to their stocks at the mouth: taken {got} against held {want}");
}
