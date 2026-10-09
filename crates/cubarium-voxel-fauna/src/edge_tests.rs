//! **EDGE**: a body does not press into a wall, a drop or the strip's end for good.
//!
//! The probe (`hunter_study edge`, fresh seeds 1–4 of the desktop terrarium, 30 min): a
//! grounded seedporter was wedged (motor refused while held forward, runs of 30 s and more)
//! for 0.60–0.99 of its awake ground time. None of it was at the strip's z end. Every wedge
//! was a reflex walking **straight** at a stand it could not reach: the arboreal refuge (a
//! stand 23–33 layers up a cliff, or across water or a drop it does not step) or the fruit
//! search (a fruiting stand across a 3-layer drop). The loftstrider's one wedge (seed 4) was
//! the herd's flight cornered against a wall, its axis rule into a second wall.
//!
//! Each case builds its scene by hand at 0.25 m and sets the config it relies on; its bounds
//! come from that config (the controller period, `teachers.press_replan_s`). No default,
//! ratio or verdict is asserted.

use super::predation_fixture::{animal, animal_mut};
use crate::perception::{self, class};
use crate::{Actions, Activity, Command, Fauna, FaunaConfig, Food, Founder, MobilityMode, StartingStores, Track, VigilMode, Vigilance};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, TICK_HZ, World};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species as Plant};
use std::f64::consts::{FRAC_PI_2, PI};

const V: f64 = 0.25;
const SP: Founder = Founder::Seedporter;
const VH: Founder = Founder::Chorister;
const GRAZER: Founder = Founder::Browser;
/// Headings: `Pose::forward` is `(sin h, cos h)`.
const NORTH: f64 = 0.0;
const SOUTH: f64 = PI;
const EAST: f64 = FRAC_PI_2;
const WEST: f64 = -FRAC_PI_2;
/// Hungry enough to want fruit, an adult's body.
const HUNGRY: StartingStores = StartingStores { body: 1.0, reserve: 0.3 };
/// The support face of a plain column: soil at `y = 1..=2`.
const FLOOR: u32 = 2;

// ------------------------------------------------------------------ fixtures

/// Soil at `y = 1..=2` at field capacity, `width × depth` columns, the clock at noon.
fn plain(width: u32, depth: u32, height: u32) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height,
        depth,
        voxel_m: V,
        seed: 11,
        day: DayClock { start_hour: 12.0, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    let m = cubarium_voxel::Materials::DEFAULT;
    let fc = m.field_capacity(Material::Soil) * m.pore_capacity(Material::Soil) * w.config().voxel_volume();
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=FLOOR {
                w.apply(WorldCommand::AddWater { x, y, z, volume_m3: fc });
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    let _ = DayMode::Off;
    w
}

/// Rock over columns `xs × zs` at layers `ys`.
fn rock(w: &mut World, xs: std::ops::RangeInclusive<i64>, zs: std::ops::RangeInclusive<u32>, ys: std::ops::RangeInclusive<u32>) {
    for z in zs {
        for x in xs.clone() {
            for y in ys.clone() {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Rock });
            }
        }
    }
}

/// Every lineage awake at every hour.
fn config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    c
}

/// Fast fruit that never falls.
fn fruit_flora() -> FloraConfig {
    let mut c = FloraConfig::default();
    c.lanternberry.fruit_per_s = 1e-3;
    c.lanternberry.fruit_cap = 2e-3;
    c.lanternberry.fruit_drop_s = 1e9;
    c.lanternberry.propagule_rate = 0.0;
    // F3's fruit alone (FLOWER-FRUIT): no flowering stage, and no rest after a crop is
    // eaten out, so the fixture's supply refills as it did.
    c.lanternberry.flower_s = 0.0;
    c.lanternberry.fruit_rest_s = 0.0;
    c
}

/// A full-grown lanternberry on column `(x, z)`; its site.
fn stand(flora: &mut Flora, world: &World, x: i64, z: u32) -> Site {
    let wood = flora.config().species(Plant::Lanternberry).wood_max;
    assert!(flora.apply(world, FloraCommand::Seed { x, z, species: Plant::Lanternberry, wood }));
    flora.view().stands.iter().find(|s| i64::from(s.site.x) == x && s.site.z == z).expect("the stand").site
}

fn stand_id(flora: &Flora, site: Site) -> u64 {
    flora.view().stand_at(site).expect("the stand").id
}

/// The centre of column `(x, z)`, metres.
fn centre(x: i64, z: u32) -> (f64, f64) {
    ((x as f64 + 0.5) * V, (f64::from(z) + 0.5) * V)
}

/// A founder of `f` on the highest support face of the column under `(x_m, z_m)`, at exactly
/// that point, heading `heading`; its id.
fn put(fauna: &mut Fauna, world: &World, f: Founder, (x_m, z_m): (f64, f64), heading: f64, stores: StartingStores) -> u64 {
    let (x, z) = ((x_m / V).floor() as i64, (z_m / V).floor() as u32);
    assert!(fauna.apply(world, Command::IntroduceFounder { x, z, founder: f, stores, heading_rad: heading }), "placing a {}", f.name());
    let id = fauna.animals.last().expect("just placed").id;
    let a = animal_mut(fauna, id);
    a.pose.x = x_m;
    a.pose.z = z_m;
    a.pose.heading_rad = heading;
    id
}

/// Drive `id` by its lineage's own heuristic.
fn heuristic(fauna: &mut Fauna, id: u64) {
    let f = animal(fauna, id).founder.expect("a founder");
    let t = fauna.config.teachers;
    fauna.controllers.set(id, crate::controller::heuristic(f, t));
}

/// Hold `id` still (a threat that only stands there).
fn still(fauna: &mut Fauna, id: u64) {
    fauna.controllers.set(id, Box::new(crate::Scripted::new(vec![Actions::REST])));
}

/// The body's memory of a fruiting stand to walk to (its record's food track), as its sight
/// would leave it, fresh at this tick.
fn remember_fruit(fauna: &mut Fauna, id: u64, flora: &Flora, site: Site) {
    let (x, z) = centre(i64::from(site.x), site.z);
    let tick = fauna.tick;
    let p = fauna.percepts.entry(id).or_default();
    p.best[class::FOOD as usize] = Track {
        known: true,
        id: stand_id(flora, site),
        class: class::FOOD,
        modes: perception::SIGHT,
        x,
        z,
        y_m: crate::surface_m(site.y, V),
        site: (site.x, site.y, site.z),
        tick,
        ..Track::default()
    };
}

/// The pressing readout of one body over a run: at its controller cadence, a sample is
/// **pressed** when it held forward and its motor delivered under 5 % of what it attempted
/// over the interval (`hunter_study`'s wedged rule). The longest run of pressed samples,
/// seconds.
#[derive(Default)]
struct Press {
    run: u64,
    longest: u64,
    samples: u64,
    pressed: u64,
}

impl Press {
    fn sample(&mut self, fauna: &Fauna, id: u64) {
        let Some(a) = fauna.view().animal(id).copied() else { return };
        let f = a.founder.expect("a founder");
        let cadence = fauna.config.manifest(f).cadence_ticks().max(1);
        if a.age_ticks % cadence != 0 || a.mobility.mode != MobilityMode::Ground {
            return;
        }
        let fb = a.founder_state.feedback;
        let refused = a.founder_state.held.forward > 0.0 && fb.attempted_equivalent > 1e-12 && fb.delivered_equivalent / fb.attempted_equivalent < 0.05;
        self.samples += 1;
        if refused {
            self.pressed += 1;
            self.run += 1;
            self.longest = self.longest.max(self.run);
        } else {
            self.run = 0;
        }
    }

    fn longest_s(&self, fauna: &Fauna, f: Founder) -> f64 {
        self.longest as f64 * fauna.config.manifest(f).cadence_ticks().max(1) as f64 / f64::from(TICK_HZ)
    }
}

/// The bound a pressed walk re-plans within: `press_replan_s` and two controller periods.
fn replan_bound_s(c: &FaunaConfig, f: Founder) -> f64 {
    c.teachers.press_replan_s + 2.0 * c.manifest(f).cadence_ticks().max(1) as f64 / f64::from(TICK_HZ)
}

fn ticks(s: f64) -> usize {
    (s * f64::from(TICK_HZ)).round() as usize
}

// ------------------------------------------------------------------ the strip's end

/// **A seedporter whose goal lies past the strip's end does not press into it.** It flees a
/// Veilhound standing inward of it, with no stand to climb: straight away from the threat is
/// out through the z end it stands against. At either end, facing out, its response does
/// not walk on (it turns along the end); facing along the end, away from the threat, it
/// walks. The stillness rule (`still_after_s`, the cornered axis) is set out of reach, so
/// only the end's own rule can answer.
#[test]
fn a_seedporter_whose_goal_lies_past_the_strips_end_does_not_press_into_it() {
    let (width, depth) = (48u32, 16u32);
    let world = plain(width, depth, 12);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.predation.still_after_s = 1e6;
    c.perception.remote_hz = f64::from(TICK_HZ);
    let r = c.founder(SP).adult_body().footprint_radius();
    let depth_m = f64::from(depth) * V;
    let x = 6.0;
    // (the seedporter's z, the threat's z, the heading out through its end)
    for (z, threat_z, out) in [(r, 2.0, SOUTH), (depth_m - r, depth_m - 2.0, NORTH)] {
        let mut fauna = Fauna::new(c.clone());
        fauna.set_births_enabled(false);
        // Facing the threat first, so it sees it and its flight starts.
        let toward = if out == SOUTH { NORTH } else { SOUTH };
        let sp = put(&mut fauna, &world, SP, (x, z), toward, StartingStores::FULL);
        heuristic(&mut fauna, sp);
        let vh = put(&mut fauna, &world, VH, (x + 0.3, threat_z), out, StartingStores::FULL);
        still(&mut fauna, vh);
        let i = fauna.animals.iter().position(|a| a.id == sp).unwrap();
        fauna.tick += 1;
        perception::sense_pass(&mut fauna, &world.view(), false, 1.0, 1);
        let want = perception::Want::default();
        perception::refresh(&mut fauna, i, &world.view(), &flora.view(), None, want);
        let _ = crate::vigil::respond(&mut fauna, i, &world.view(), &flora.view(), Actions::REST);
        assert!(fauna.percept(sp).is_some_and(|p| p.vigil.fleeing), "fixture: the seedporter flees the Veilhound it sees");
        // Turned out through its end: it does not walk on.
        animal_mut(&mut fauna, sp).pose.heading_rad = out;
        let act = crate::vigil::respond(&mut fauna, i, &world.view(), &flora.view(), Actions::REST).expect("in flight");
        assert_eq!(act.forward, 0.0, "facing out through the end at z {z:.2}, it does not press ({act:?})");
        assert!(act.turn != 0.0, "it turns along the end ({act:?})");
        // Along the end, away from the threat's side (the threat is at +x of it): it walks.
        animal_mut(&mut fauna, sp).pose.heading_rad = WEST;
        let act = crate::vigil::respond(&mut fauna, i, &world.view(), &flora.view(), Actions::REST).expect("in flight");
        assert!(act.forward > 0.0, "along the end at z {z:.2}, away from the threat, it walks ({act:?})");
    }
}

// ------------------------------------------------------------------ a pressed walk re-plans

/// **A pressed body re-plans within a bounded time.** A hungry seedporter walks to a
/// fruiting stand it remembers on open ground, on its route; once it is on its way a rock
/// wall rises across the whole strip between them, which its route graph (rebuilt only
/// after `route_refresh_s`, set out of reach) does not know of. It presses the wall for no
/// longer than `press_replan_s` and two controller periods at a time.
#[test]
fn a_pressed_body_replans_within_a_bounded_time() {
    let (width, depth) = (64u32, 16u32);
    let mut world = plain(width, depth, 16);
    let mut flora = Flora::new(fruit_flora());
    let site = stand(&mut flora, &world, 40, 8);
    for _ in 0..60 {
        flora.step(&mut world);
    }
    assert!(flora.view().stand_at(site).expect("stand").fruit > 0.0, "fixture: fruit hangs");
    let mut c = config();
    c.teachers.press_replan_s = 2.0;
    c.teachers.press_shun_s = 1e6;
    c.teachers.route_refresh_s = 1e6;
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let sp = put(&mut fauna, &world, SP, centre(24, 8), EAST, HUNGRY);
    heuristic(&mut fauna, sp);
    // On its way, its route graph built on the open plain.
    for _ in 0..ticks(0.5) {
        remember_fruit(&mut fauna, sp, &flora, site);
        fauna.step(&world, &mut flora);
    }
    assert!(animal(&fauna, sp).pose.x > centre(24, 8).0, "fixture: it walks toward the stand");
    rock(&mut world, 32..=32, 0..=depth - 1, FLOOR + 1..=FLOOR + 8);
    let mut press = Press::default();
    for _ in 0..ticks(20.0) {
        remember_fruit(&mut fauna, sp, &flora, site);
        fauna.step(&world, &mut flora);
        press.sample(&fauna, sp);
    }
    assert!(press.samples > 0);
    let bound = replan_bound_s(&c, SP);
    assert!(press.longest_s(&fauna, SP) <= bound, "pressed {:.2} s at a time, bound {bound:.2} s", press.longest_s(&fauna, SP));
}

// ------------------------------------------------------------------ the causes

/// **The refuge is a stand it can walk to.** A seedporter flees a Veilhound it sees; the
/// nearest stand it could climb stands on a rock block four of its steps up, the next on the
/// open ground beyond it. It climbs the one on the ground, and presses no wall on the way
/// longer than a re-plan.
#[test]
fn a_fleeing_seedporter_climbs_a_stand_it_can_walk_to_not_the_nearest_on_a_ledge() {
    let (width, depth) = (64u32, 16u32);
    let mut world = plain(width, depth, 20);
    let c = config();
    let up = c.founder(SP).step_limits(V).up;
    let top = FLOOR + 4 * up.max(1);
    rock(&mut world, 26..=30, 5..=11, FLOOR + 1..=top);
    let mut flora = Flora::new(fruit_flora());
    let safe_tree = |flora: &mut Flora, x, z| {
        let wood = flora.config().vaulttree.wood_max;
        assert!(flora.apply(&world, FloraCommand::Seed { x, z, species: Plant::Vaulttree, wood }));
        flora.view().stands.iter().find(|s| i64::from(s.site.x) == x && s.site.z == z).unwrap().site
    };
    let ledge = safe_tree(&mut flora, 27, 8);
    assert_eq!(ledge.y, top, "fixture: the near stand is on the block");
    let ground = safe_tree(&mut flora, 24, 14);
    assert_eq!(ground.y, FLOOR, "fixture: the far stand is on the ground");
    let me = centre(24, 8);
    let d = |s: Site| {
        let (x, z) = centre(i64::from(s.x), s.z);
        ((x - me.0).powi(2) + (z - me.1).powi(2)).sqrt()
    };
    assert!(d(ledge) < d(ground), "fixture: the ledge stand is the nearer");
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let sp = put(&mut fauna, &world, SP, me, WEST, StartingStores::FULL);
    heuristic(&mut fauna, sp);
    let vh = put(&mut fauna, &world, VH, centre(16, 8), EAST, StartingStores::FULL);
    still(&mut fauna, vh);
    let mut press = Press::default();
    let mut on = None;
    for _ in 0..ticks(30.0) {
        fauna.step(&world, &mut flora);
        press.sample(&fauna, sp);
        let a = animal(&fauna, sp);
        if a.mobility.mode != MobilityMode::Ground {
            on = match a.mobility.perch.map(|p| p.anchor) {
                Some(crate::PerchAnchor::Stand { site, .. }) => Some(site),
                _ => None,
            };
            break;
        }
    }
    assert!(fauna.ledger.vigil.refuges[SP.index()] > 0, "fixture: it fled to a refuge");
    assert_eq!(on, Some(ground), "it climbs the stand on the ground");
    let bound = replan_bound_s(&c, SP);
    assert!(press.longest_s(&fauna, SP) <= bound, "pressed {:.2} s at a time, bound {bound:.2} s", press.longest_s(&fauna, SP));
}

/// **A fruit walk goes round a wall.** A hungry seedporter remembers a fruiting stand behind
/// a rock wall that crosses the strip but for a gap at one side: it walks round through the
/// gap and eats, and presses the wall no longer than a re-plan.
#[test]
fn a_hungry_seedporter_walks_round_a_wall_to_fruit_it_remembers() {
    let (width, depth) = (64u32, 16u32);
    let mut world = plain(width, depth, 16);
    rock(&mut world, 30..=30, 0..=11, FLOOR + 1..=FLOOR + 8);
    let mut flora = Flora::new(fruit_flora());
    let site = stand(&mut flora, &world, 34, 6);
    for _ in 0..60 {
        flora.step(&mut world);
    }
    assert!(flora.view().stand_at(site).expect("stand").fruit > 0.0, "fixture: fruit hangs");
    let c = config();
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let sp = put(&mut fauna, &world, SP, centre(26, 6), EAST, HUNGRY);
    heuristic(&mut fauna, sp);
    let mut press = Press::default();
    let mut ate = false;
    for _ in 0..ticks(60.0) {
        remember_fruit(&mut fauna, sp, &flora, site);
        fauna.step(&world, &mut flora);
        press.sample(&fauna, sp);
        if fauna.ledger.bites_by_food[Food::Fruit.index()] > 0 {
            ate = true;
            break;
        }
    }
    assert!(ate, "it reaches the fruit round the wall");
    let bound = replan_bound_s(&c, SP);
    assert!(press.longest_s(&fauna, SP) <= bound, "pressed {:.2} s at a time, bound {bound:.2} s", press.longest_s(&fauna, SP));
}

/// **A cornered herd body with nowhere farther to go holds still.** A frondgrazer (its
/// hunker off, flight only) flees a Veilhound walking at the mouth of a rock pocket it stands
/// at the back of: every way out is toward the threat. Cornered (still in its flight for
/// `still_after_s`), it does not walk into the pocket's walls: its response holds still.
#[test]
fn a_cornered_grazer_in_a_pocket_holds_still_rather_than_pressing() {
    let (width, depth) = (48u32, 16u32);
    let mut world = plain(width, depth, 12);
    // The pocket: open to +x, interior x 21..=27, z 6..=9.
    rock(&mut world, 20..=20, 5..=10, FLOOR + 1..=FLOOR + 6);
    rock(&mut world, 20..=27, 5..=5, FLOOR + 1..=FLOOR + 6);
    rock(&mut world, 20..=27, 10..=10, FLOOR + 1..=FLOOR + 6);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[GRAZER.index()].hunker.enabled = false;
    c.founders[GRAZER.index()].vigilance =
        Vigilance { mode: VigilMode::Herd, flee_start_m: 4.0, flee_stop_m: 8.0, calm_s: 10.0, group_min: 1, run_noise: 1.0, ..Vigilance::default() };
    c.perception.remote_hz = f64::from(TICK_HZ);
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let (bx, bz) = (21.0 * V + 0.5 * V + 0.2, 8.0 * V);
    let gr = put(&mut fauna, &world, GRAZER, (bx, bz), EAST, StartingStores::FULL);
    heuristic(&mut fauna, gr);
    let vh = put(&mut fauna, &world, VH, centre(30, 7), WEST, StartingStores::FULL);
    still(&mut fauna, vh);
    animal_mut(&mut fauna, vh).speed_m_s = 1.0;
    animal_mut(&mut fauna, vh).still_ticks = 0;
    let i = fauna.animals.iter().position(|a| a.id == gr).unwrap();
    fauna.tick += 1;
    perception::sense_pass(&mut fauna, &world.view(), false, 1.0, 1);
    perception::refresh(&mut fauna, i, &world.view(), &flora.view(), None, perception::Want::default());
    let _ = crate::vigil::respond(&mut fauna, i, &world.view(), &flora.view(), Actions::REST);
    assert!(fauna.percept(gr).is_some_and(|p| p.vigil.fleeing), "fixture: the grazer flees the walking Veilhound");
    // Cornered: its flight older than `still_after_s`, and it still that long, facing the
    // back wall (straight away from the threat).
    let still_ticks = c.predation.still_after_ticks();
    fauna.tick += still_ticks + 1;
    fauna.percepts.get_mut(&gr).unwrap().vigil.close_tick = fauna.tick;
    animal_mut(&mut fauna, gr).still_ticks = still_ticks + 1;
    animal_mut(&mut fauna, gr).pose.heading_rad = WEST;
    for heading in [WEST, NORTH, SOUTH] {
        animal_mut(&mut fauna, gr).pose.heading_rad = heading;
        let act = crate::vigil::respond(&mut fauna, i, &world.view(), &flora.view(), Actions::REST).expect("in flight");
        assert_eq!(act.forward, 0.0, "facing {heading:.2} at the back of the pocket, it does not press ({act:?})");
    }
}

/// **A body in a stream walks out where the water's surface slopes.** A seedporter stands in
/// water deeper than it wades; the bank it can wade to lies upstream, across a channel whose
/// water is a few millimetres deeper every column (a flowing stream's surface), and rock closes
/// the other way. It walks up the channel and out within the time the walk takes at cruise
/// and a re-plan's bound, and presses no longer than a re-plan at a time.
#[test]
fn a_seedporter_in_a_stream_walks_out_up_a_surface_that_slopes_by_millimetres() {
    let (width, depth) = (32u32, 8u32);
    let mut world = plain(width, depth, 12);
    let c = config();
    let wade = c.founder(SP).wade_depth_m(&c.founder(SP).adult_body());
    let cell = world.config().voxel_volume();
    // The channel: x 9..=16 on every z, its water `wade + 0.05` at x = 16, 2 mm deeper each
    // column west; the bank at x = 8 and west is dry; rock from x = 17.
    let (bank, here) = (8i64, 16i64);
    for z in 0..depth {
        for x in bank + 1..=here {
            let d = wade + 0.05 + 0.002 * (here - x) as f64;
            world.apply(WorldCommand::AddWater { x, y: FLOOR + 1, z, volume_m3: cell * d / V });
        }
    }
    rock(&mut world, here + 1..=here + 4, 0..=depth - 1, FLOOR + 1..=FLOOR + 6);
    let (x0, z0) = centre(here, 4);
    let wv = world.view();
    assert!(wv.water_depth_m(here, FLOOR, 4) > wade, "fixture: it stands deeper than it wades");
    assert!(wv.water_depth_m(here - 1, FLOOR, 4) > wv.water_depth_m(here, FLOOR, 4), "fixture: upstream is deeper by millimetres");
    assert!(wv.water_depth_m(bank, FLOOR, 4) <= wade, "fixture: the bank is wadeable");
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let sp = put(&mut fauna, &world, SP, centre(4, 4), WEST, StartingStores::FULL);
    {
        let a = animal_mut(&mut fauna, sp);
        a.pose.x = x0;
        a.pose.z = z0;
        a.site = Site { x: here as u32, y: FLOOR, z: 4 };
    }
    heuristic(&mut fauna, sp);
    let cruise = c.manifest(SP).cruise_m_per_s;
    let walk_s = (f64::from((here - bank) as u32) * V) / cruise + replan_bound_s(&c, SP) + 2.0;
    let mut press = Press::default();
    let mut out = false;
    for _ in 0..ticks(walk_s) {
        fauna.step(&world, &mut flora);
        press.sample(&fauna, sp);
        let a = animal(&fauna, sp);
        if world.view().water_depth_m(i64::from(a.site.x), a.site.y, a.site.z) <= wade {
            out = true;
            break;
        }
    }
    assert!(out, "it walks out of the water within {walk_s:.1} s");
    let bound = replan_bound_s(&c, SP);
    assert!(press.longest_s(&fauna, SP) <= bound, "pressed {:.2} s at a time, bound {bound:.2} s", press.longest_s(&fauna, SP));
}

// ------------------------------------------------------------------ EDGE-2

// EDGE-2's probe (`hunter_study edge detail=1` on EDGE's seed 2 and 4 saves at 310 and 360
// min): of the seedporter's refused samples, 95 % (seed 2) and 100 % (seed 4) were bodies
// whose disc already overlapped solid (`motor_probe`: pose not clear), every heading
// refused: a body caught in a wall could not step out of it. The rest (seed 2, 5 %) stood on
// a face off its route graph with no walking way back on and no glide: its fruit and roost
// walks went straight at the stem, over a drop it does not step.

/// One forward action held whatever happens (a scripted push).
fn push(fauna: &mut Fauna, id: u64) {
    fauna.controllers.set(id, Box::new(crate::Scripted::new(vec![Actions { forward: 1.0, turn: 0.0, feed: 0.0 }])));
}

/// Whether body `id`'s pose is clear of solid (the motor's own rule).
fn pose_clear_of(fauna: &Fauna, world: &World, id: u64) -> bool {
    let a = animal(fauna, id);
    let f = a.founder.expect("a founder");
    let p = fauna.config.founder(f);
    let body = p.body_at(a.body);
    crate::motor_probe(&world.view(), a.pose.x, a.pose.z, (0.0, 1.0), a.site.y, 0.0, &body, p.wade_depth_m(&body), p.step_limits(V)).1
}

/// **A body caught in a wall steps out of it, and never deeper.** A seedporter and a
/// frondgrazer each stand with their disc half a radius into a rock wall at their east (as a
/// body grows against a wall, or comes down a stem or a vine beside one). Pushed east, into
/// the wall, neither moves deeper; pushed west, each walks out, its pose clear, within the
/// time its cruise takes over its radius and two controller periods.
#[test]
fn a_body_caught_in_a_wall_steps_out_of_it_and_never_deeper() {
    let (width, depth) = (32u32, 16u32);
    let mut world = plain(width, depth, 12);
    rock(&mut world, 16..=19, 0..=depth - 1, FLOOR + 1..=FLOOR + 6);
    let c = config();
    for f in [SP, GRAZER] {
        let r = c.founder(f).adult_body().footprint_radius();
        let start = (16.0 * V - 0.5 * r, 8.0 * V + 0.5 * V);
        // Into the wall.
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(c.clone());
        fauna.set_births_enabled(false);
        let id = put(&mut fauna, &world, f, start, EAST, StartingStores::FULL);
        push(&mut fauna, id);
        assert!(!pose_clear_of(&fauna, &world, id), "fixture: the {} is caught in the wall", f.name());
        for _ in 0..ticks(1.0) {
            fauna.step(&world, &mut flora);
        }
        assert!(animal(&fauna, id).pose.x <= start.0 + 1e-9, "pushed into the wall, the {} goes no deeper", f.name());
        // Out of it.
        let mut fauna = Fauna::new(c.clone());
        fauna.set_births_enabled(false);
        let id = put(&mut fauna, &world, f, start, WEST, StartingStores::FULL);
        push(&mut fauna, id);
        let period = c.manifest(f).cadence_ticks().max(1) as f64 / f64::from(TICK_HZ);
        let bound = r / c.manifest(f).cruise_m_per_s + 2.0 * period;
        let mut out = false;
        for _ in 0..ticks(bound).max(1) {
            fauna.step(&world, &mut flora);
            if pose_clear_of(&fauna, &world, id) {
                out = true;
                break;
            }
        }
        assert!(out, "pushed away from the wall, the {} is out of it within {bound:.2} s (x {:.3} from {:.3})", f.name(), animal(&fauna, id).pose.x, start.0);
    }
}

/// A young seedporter's stores: a third of the adult's structure, hungry.
const YOUNG: StartingStores = StartingStores { body: 0.3, reserve: 0.3 };

/// **The ledge**: a face `p` at `(20, 8)` whose east and north (and the corners beside them)
/// are rock walls high over it, so the adult disc its lineage's route graph is built for does
/// not stand on it; west, south and south-west lie `drop_ws` layers below and, when `deep_s`,
/// the south column falls to the floor. Its layer.
fn ledge(world: &mut World, h: u32, drop_ws: u32, deep_s: bool) -> u32 {
    let top = FLOOR + h;
    let low = top - drop_ws;
    // The platform west and south of it, and its own column.
    rock(world, 14..=19, 4..=8, FLOOR + 1..=low);
    if !deep_s {
        rock(world, 14..=20, 4..=7, FLOOR + 1..=low);
    }
    rock(world, 20..=20, 8..=8, FLOOR + 1..=top);
    // The walls.
    rock(world, 21..=23, 4..=12, FLOOR + 1..=top + 6);
    rock(world, 14..=23, 9..=11, FLOOR + 1..=top + 6);
    top
}

/// A young seedporter on the ledge, centred on it; its id.
fn on_ledge(fauna: &mut Fauna, world: &World, top: u32) -> u64 {
    let sp = put(fauna, world, SP, centre(4, 2), WEST, YOUNG);
    let a = animal_mut(fauna, sp);
    let (x, z) = centre(20, 8);
    a.pose.x = x;
    a.pose.z = z;
    a.site = Site { x: 20, y: top, z: 8 };
    a.pose.y_m = crate::surface_m(top, V);
    sp
}

/// The fixture's claims about the ledge: the young body stands clear on it, off its route
/// graph, and its own body has no walking way back onto it.
fn assert_off_graph_pocket(fauna: &mut Fauna, world: &World, sp: u64) {
    assert!(pose_clear_of(fauna, world, sp), "fixture: the young seedporter stands clear on the ledge");
    let a = animal(fauna, sp);
    let p = *fauna.config.founder(SP);
    let body = p.body_at(a.body);
    let r = body.footprint_radius();
    let own = (r / V).ceil() as u32;
    let wv = world.view();
    let g = fauna.routes(&wv, SP, 1e6);
    assert!(g.node_near(a.site, (a.pose.x, a.pose.z), own).is_none(), "fixture: the ledge is off the route graph");
    assert!(g.way_on(&wv, a.site, r, body.headroom_voxels(V), p.step_limits(V), p.wade_depth_m(&body), own, 64).is_none(), "fixture: no walking way back on");
}

/// **A seedporter on a ledge it can neither walk nor glide off does not press.** A young
/// seedporter stands on a ledge off its route graph: walls east and north, and west and south
/// a drop one layer beyond its step and short of its glide height. Hungry, remembering a
/// fruiting stand west on the ground, its fruit walk does not hold it forward into the drop
/// (no pressed sample whose layer is the fruit search); in flight from a Veilhound walking
/// on the platform below, its response holds still at once, whatever its heading. Re-plans are
/// set out of reach,
/// so only the ledge's own rule can answer.
#[test]
fn a_seedporter_on_a_ledge_it_cannot_walk_or_glide_off_does_not_press() {
    let (width, depth) = (48u32, 16u32);
    let mut world = plain(width, depth, 24);
    let mut c = config();
    c.teachers.press_replan_s = 1e6;
    c.teachers.route_refresh_s = 1e6;
    c.perception.remote_hz = f64::from(TICK_HZ);
    let down = c.founder(SP).step_limits(V).down;
    let glide = c.founder(SP).glider.expect("a glider").glide_min_height_m;
    let drop = down + 1;
    assert!(f64::from(drop) * V < glide, "fixture: the drop is short of a glide");
    let top = ledge(&mut world, drop + 2, drop, false);
    let mut flora = Flora::new(fruit_flora());
    let site = stand(&mut flora, &world, 6, 6);
    for _ in 0..60 {
        flora.step(&mut world);
    }
    assert!(flora.view().stand_at(site).expect("stand").fruit > 0.0, "fixture: fruit hangs");
    // Hungry.
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let sp = on_ledge(&mut fauna, &world, top);
    heuristic(&mut fauna, sp);
    assert_off_graph_pocket(&mut fauna, &world, sp);
    let mut fruit_pressed = 0;
    let mut fruit_walks = 0;
    for _ in 0..ticks(10.0) {
        remember_fruit(&mut fauna, sp, &flora, site);
        fauna.step(&world, &mut flora);
        let a = animal(&fauna, sp);
        let cadence = c.manifest(SP).cadence_ticks().max(1);
        if a.age_ticks % cadence != 0 || a.mobility.mode != MobilityMode::Ground {
            continue;
        }
        if fauna.steer_of(sp) == Some(crate::Steer::Fruit) {
            fruit_walks += 1;
            let fb = a.founder_state.feedback;
            if a.founder_state.held.forward > 0.0 && fb.attempted_equivalent > 1e-12 && fb.delivered_equivalent / fb.attempted_equivalent < 0.05 {
                fruit_pressed += 1;
            }
        }
    }
    assert_eq!(fruit_pressed, 0, "its fruit walk presses the drop ({fruit_walks} fruit samples)");
    // In flight.
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let sp = on_ledge(&mut fauna, &world, top);
    heuristic(&mut fauna, sp);
    let vh = put(&mut fauna, &world, VH, centre(16, 5), EAST, StartingStores::FULL);
    still(&mut fauna, vh);
    animal_mut(&mut fauna, vh).speed_m_s = 1.0;
    animal_mut(&mut fauna, vh).still_ticks = 0;
    let i = fauna.animals.iter().position(|a| a.id == sp).unwrap();
    fauna.tick += 1;
    perception::sense_pass(&mut fauna, &world.view(), false, 1.0, 1);
    perception::refresh(&mut fauna, i, &world.view(), &flora.view(), None, perception::Want::default());
    let _ = crate::vigil::respond(&mut fauna, i, &world.view(), &flora.view(), Actions::REST);
    assert!(fauna.percept(sp).is_some_and(|p| p.vigil.fleeing), "fixture: the seedporter flees the Veilhound");
    for heading in [WEST, SOUTH, EAST, NORTH, WEST + 0.3] {
        animal_mut(&mut fauna, sp).pose.heading_rad = heading;
        let act = crate::vigil::respond(&mut fauna, i, &world.view(), &flora.view(), Actions::REST).expect("in flight");
        assert_eq!(act.forward, 0.0, "facing {heading:.2} on the ledge, it holds still ({act:?})");
    }
}

/// **A seedporter on a ledge glides off where it can.** The same ledge, but its south column
/// falls to the floor, more than its glide height; west is still the short drop. Hungry,
/// remembering a fruiting stand west, it leaves the ledge by gliding south within the time it
/// takes to turn half round at its yaw cap, walk to the edge at cruise, and two controller
/// periods. Re-plans are out of reach: walking west it would press for good.
#[test]
fn a_seedporter_on_a_ledge_glides_off_where_it_can() {
    let (width, depth) = (48u32, 16u32);
    let mut world = plain(width, depth, 24);
    let mut c = config();
    c.teachers.press_replan_s = 1e6;
    c.teachers.route_refresh_s = 1e6;
    let down = c.founder(SP).step_limits(V).down;
    let glide = c.founder(SP).glider.expect("a glider").glide_min_height_m;
    let drop = down + 1;
    let h = (glide / V).ceil() as u32 + 1;
    assert!(f64::from(drop) * V < glide && f64::from(h) * V >= glide, "fixture: west short of a glide, south past one");
    let top = ledge(&mut world, h, drop, true);
    let mut flora = Flora::new(fruit_flora());
    let site = stand(&mut flora, &world, 6, 6);
    for _ in 0..60 {
        flora.step(&mut world);
    }
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    let sp = on_ledge(&mut fauna, &world, top);
    heuristic(&mut fauna, sp);
    assert_off_graph_pocket(&mut fauna, &world, sp);
    let m = c.manifest(SP);
    let period = m.cadence_ticks().max(1) as f64 / f64::from(TICK_HZ);
    let bound = PI / m.yaw_cap_rad_per_s + V / m.cruise_m_per_s + 2.0 * period;
    let mut left = false;
    for _ in 0..ticks(bound) {
        remember_fruit(&mut fauna, sp, &flora, site);
        fauna.step(&world, &mut flora);
        let a = animal(&fauna, sp);
        if a.mobility.mode != MobilityMode::Ground || a.site != (Site { x: 20, y: top, z: 8 }) {
            left = a.mobility.mode == MobilityMode::Glide || a.site.y < top;
            break;
        }
    }
    assert!(left, "it glides off the ledge within {bound:.2} s");
}
