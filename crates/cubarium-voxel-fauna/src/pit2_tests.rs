//! **PIT-2**: a Veilhound walking its route graph keeps to it at a bend.
//!
//! The probe (seeds 1 and 4, the desktop terrarium, fresh foundings): every pup of the first
//! litter was born on the graph and walked home on the pack's route field, but steered at the
//! next node — one column ahead — at its full pace, which covers two to three columns in a
//! controller period. At a bend it ran on past the node, off the graph, onto a ledge its disc
//! could not leave (four or five layers down, a riser on one side and a drop on the other),
//! and stood there for the rest of its life: the pups' wedged share was 0.997 and 0.995, and
//! as adults the same bodies were the lasting adult wedges of both seeds.
//!
//! The scene: a plateau with a raised block over it, an L-shaped corridor cut through the
//! block — east along one leg, then north along the other — `CORRIDOR` columns wide. The
//! route graph's margin is set so that only the corridor's centre column holds nodes (the
//! disc plus the margin is the corridor's half width); the body's own disc fits a column
//! either side of it. At the bend, the corridor's floor east of the centre column is a **ledge**
//! one step down, `LEDGE` columns wide: off the graph, the place a body running on past the
//! bend steps down to. The den is at the end of the north leg. Every case sets the config it
//! relies on (the margin, the den radius, the clock) and takes its bounds from it; no shipped
//! default, ratio or verdict is asserted.

use crate::{Activity, Animal, Command, Fauna, FaunaConfig, Founder, PackMember, Pose, StartingStores};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};
use std::f64::consts::FRAC_PI_2;

const CH: Founder = Founder::Chorister;
const V: f64 = 0.125;
const WIDTH: u32 = 160;
const DEPTH: u32 = 96;
const HEIGHT: u32 = 40;
/// The plateau's support layer.
const TOP: u32 = 16;
/// The corridor's width in columns (odd: one centre column).
const CORRIDOR: u32 = 5;
/// The east leg: x from `XA` to `XC + CORRIDOR`, z from `ZA`.
const XA: u32 = 20;
const XC: u32 = 80;
const ZA: u32 = 20;
/// The north leg: z from `ZA` to `ZB`, x from `XC`.
const ZB: u32 = 80;
const EAST: f64 = FRAC_PI_2;

/// The ledge's width in columns, from the column east of the centre column.
const LEDGE: u32 = 2;

/// The corridor's centre column offset.
const MID: u32 = CORRIDOR / 2;

fn on_ledge(x: u32, z: u32) -> bool {
    (XC + MID + 1..XC + MID + 1 + LEDGE).contains(&x) && (ZA..ZA + CORRIDOR).contains(&z)
}

fn in_corridor(x: u32, z: u32) -> bool {
    let east = (XA..XC + CORRIDOR).contains(&x) && (ZA..ZA + CORRIDOR).contains(&z);
    let north = (XC..XC + CORRIDOR).contains(&x) && (ZA..=ZB).contains(&z);
    east || north
}

/// The plateau, the block over it but for the corridor: the block a voxel taller than the
/// body's headroom, so its walls are walls. The clock on at noon.
fn world(c: &FaunaConfig, seed: u64) -> World {
    let room = c.founder(CH).adult_body().headroom_voxels(V);
    let step = c.founder(CH).step_limits(V);
    assert!(TOP + room + 2 < HEIGHT, "the block fits under the sky");
    assert!(step.down >= 1 && step.down < TOP, "the ledge is a step down");
    let mut w = World::empty(VoxelConfig {
        width: WIDTH,
        height: HEIGHT,
        depth: DEPTH,
        voxel_m: V,
        seed,
        day: DayClock { mode: DayMode::Cycle, start_hour: 12.0, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    for z in 0..DEPTH {
        for x in 0..WIDTH {
            let top = if on_ledge(x, z) {
                TOP - step.down
            } else if in_corridor(x, z) {
                TOP
            } else {
                TOP + room + 2
            };
            for y in 1..=top {
                let material = if y > TOP { Material::Rock } else { Material::Soil };
                w.apply(WorldCommand::SetMaterial { x: i64::from(x), y, z, material });
            }
        }
    }
    w
}

/// A pack of one, awake always, its den radius, the route graph's margin the corridor's half
/// width less the adult's footprint radius (nodes only on the centre column), the turn's swing
/// and the breeder's whelping at the den on.
fn config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
    }
    let r = c.founder(CH).adult_body().footprint_radius();
    let half = 0.5 * f64::from(CORRIDOR) * V;
    assert!(r <= half - V, "the disc fits a column off the corridor's centre (the premise)");
    c.founding.clearance_margin_m = half - r;
    let pk = c.founders[CH.index()].pack.as_mut().expect("the chorister's pack table");
    pk.pack_size = 1;
    pk.den_radius_m = 1.0;
    pk.home_reserve_share = 1.0;
    // The turn onto the north leg: a quarter voxel across its line a period, well inside the
    // column between the centre column and the ledge.
    pk.route_swing_voxels = 0.25;
    pk.whelp_at_den = true;
    c
}

fn animal(fauna: &Fauna, id: u64) -> Animal {
    *fauna.view().animal(id).expect("alive")
}

fn animal_mut(fauna: &mut Fauna, id: u64) -> &mut Animal {
    fauna.animals.iter_mut().find(|a| a.id == id).expect("alive")
}

fn centre(x: u32, z: u32) -> (f64, f64) {
    ((f64::from(x) + 0.5) * V, (f64::from(z) + 0.5) * V)
}

fn place(fauna: &mut Fauna, id: u64, x: u32, z: u32, heading_rad: f64) {
    let (px, pz) = centre(x, z);
    let a = animal_mut(fauna, id);
    a.pose = Pose { x: px, z: pz, heading_rad, y_m: crate::surface_m(TOP, V) };
    a.site = Site { x, y: TOP, z };
    a.founder_state.held = crate::controller::Actions::REST;
}

/// The premise: the corridor's centre column holds the route graph's nodes, the columns either
/// side of it do not, and the body's disc stands clear on them; the ledge is off the graph,
/// and the body's disc stands clear on it.
fn assert_lane(fauna: &Fauna, world: &World) {
    let g = crate::RouteGraph::for_founder(&world.view(), fauna.config.founder(CH), fauna.config.founding.clearance_margin_m);
    let body = fauna.config.founder(CH).adult_body();
    let step = fauna.config.founder(CH).step_limits(V);
    let lane = Site { x: XA + 10, y: TOP, z: ZA + MID };
    let side = Site { x: XA + 10, y: TOP, z: ZA + MID + 1 };
    assert!(g.face(lane).is_some(), "the corridor's centre column is on the graph");
    assert!(g.face(side).is_none(), "the column beside it is off the graph");
    assert!(crate::body::disc_clear_on_face(&world.view(), &body, side, step), "the body's disc stands on the column beside it");
    assert!(g.face(Site { x: XC + MID, y: TOP, z: ZB - 4 }).is_some(), "the north leg's centre column is on the graph");
    let ledge = Site { x: XC + MID + 1, y: TOP - step.down, z: ZA + MID };
    assert!(g.face(ledge).is_none(), "the ledge is off the graph");
    assert!(crate::body::disc_clear_on_face(&world.view(), &body, ledge, step), "the body's disc stands on the ledge");
}

/// Walk `id` until it is within `radius` of the den or `max` ticks pass: at each of its
/// controller samples, whether its centre stood on the graph's centre column (a node on its
/// own face), and the lowest layer it stood on. Returns (home tick, samples, samples off the
/// graph, the first off sample's site, the lowest layer).
fn walk(fauna: &mut Fauna, world: &World, flora: &mut Flora, id: u64, den: Site, radius: f64, max: u32) -> (Option<u32>, u32, u32, Option<Site>, u32) {
    let g = crate::RouteGraph::for_founder(&world.view(), fauna.config.founder(CH), fauna.config.founding.clearance_margin_m);
    let cadence = fauna.config.manifest(CH).cadence_ticks().max(1);
    let (dx, dz) = centre(den.x, den.z);
    let (mut samples, mut off, mut first) = (0u32, 0u32, None);
    let mut lowest = animal(fauna, id).site.y;
    for t in 1..=max {
        fauna.step_with(world, flora, 1);
        let a = animal(fauna, id);
        lowest = lowest.min(a.site.y);
        if a.age_ticks % cadence == 0 {
            samples += 1;
            if g.face(a.site).is_none() {
                off += 1;
                first.get_or_insert(a.site);
            }
        }
        let (ex, ez) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, dx, dz, f64::from(WIDTH) * V);
        if (ex * ex + ez * ez).sqrt() <= radius {
            return (Some(t), samples, off, first, lowest);
        }
    }
    (None, samples, off, first, lowest)
}

/// The walk's bound: the route's length at cruise, three times over, and ten controller
/// periods.
fn bound(fauna: &Fauna, metres: f64) -> u32 {
    let m = fauna.config.manifest(CH);
    cubarium_voxel::ticks_at_least_one(3.0 * metres / m.cruise_m_per_s + 10.0 * m.controller_period_s) as u32
}

/// The pack, its den at the north leg's end on the centre column, and its one adult.
fn pack(fauna: &mut Fauna, world: &World) -> (Site, u64, u64) {
    let den = Site { x: XC + MID, y: TOP, z: ZB - 2 };
    assert!(fauna.apply(world, Command::FoundPack { den, founder: CH, stores: StartingStores::FULL, heading_rad: EAST }));
    let p = fauna.packs()[0].id;
    let adult = fauna.animals.iter().find(|a| a.founder == Some(CH)).expect("founded").id;
    let teachers = fauna.config.teachers;
    fauna.controllers.set(adult, crate::controller::heuristic(CH, teachers));
    (den, p, adult)
}

/// **A pup walking home round a bend keeps to its route graph**: a pup at the west end of
/// the east leg, its den at the north leg's end. It reaches the den within the bound of the
/// walk without stepping down onto the ledge past the bend (it ran past the corner node at its
/// full pace, down onto the ledge).
#[test]
fn a_pup_walking_home_round_a_bend_keeps_to_its_route_graph() {
    let c = config();
    let world = world(&c, 701);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    assert_lane(&fauna, &world);
    let (den, p, adult) = pack(&mut fauna, &world);
    place(&mut fauna, adult, den.x, den.z, EAST);
    assert!(fauna.apply(
        &world,
        Command::IntroduceFounderOnFace {
            site: Site { x: XA + 4, y: TOP, z: ZA + MID },
            founder: CH,
            stores: StartingStores { body: 0.3, reserve: 0.2 },
            heading_rad: EAST,
        }
    ));
    let pup = fauna.animals.last().expect("introduced").id;
    animal_mut(&mut fauna, pup).pack = Some(PackMember { pack: p, pup: true, calling_ticks: 0 });
    let teachers = fauna.config.teachers;
    fauna.controllers.set(pup, crate::controller::heuristic(CH, teachers));
    place(&mut fauna, pup, XA + 4, ZA + MID, EAST);
    let route = f64::from(XC + MID - (XA + 4)) * V + f64::from(den.z - (ZA + MID)) * V;
    let max = bound(&fauna, route);
    let radius = 0.5 * c.founder(CH).pack.unwrap().den_radius_m;
    let (home, samples, off, first, lowest) = walk(&mut fauna, &world, &mut flora, pup, den, radius, max);
    let a = animal(&fauna, pup);
    assert!(home.is_some(), "the pup is not home within {max} ticks: at ({:.2}, {:.2}) on layer {}", a.pose.x, a.pose.z, a.site.y);
    assert_eq!(lowest, TOP, "the pup stepped down off the corridor onto the ledge (off its graph at {off} of {samples} controller samples, first at {first:?})");
}

/// **An adult walking home to rest round a bend keeps to its route graph**: the lineage awake
/// by night at noon (it wants its rest), the adult at the west end of the east leg, its den at
/// the north leg's end. As the pup's case.
#[test]
fn an_adult_walking_home_round_a_bend_keeps_to_its_route_graph() {
    let mut c = config();
    c.founders[CH.index()].active = Activity::Nocturnal;
    let world = world(&c, 702);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    assert_lane(&fauna, &world);
    let (den, _, adult) = pack(&mut fauna, &world);
    place(&mut fauna, adult, XA + 4, ZA + MID, EAST);
    let route = f64::from(XC + MID - (XA + 4)) * V + f64::from(den.z - (ZA + MID)) * V;
    let max = bound(&fauna, route);
    let radius = c.founder(CH).pack.unwrap().den_radius_m;
    let (home, samples, off, first, lowest) = walk(&mut fauna, &world, &mut flora, adult, den, radius, max);
    let a = animal(&fauna, adult);
    assert!(home.is_some(), "the adult is not home within {max} ticks: at ({:.2}, {:.2}) on layer {}", a.pose.x, a.pose.z, a.site.y);
    assert_eq!(lowest, TOP, "the adult stepped down off the corridor onto the ledge (off its graph at {off} of {samples} controller samples, first at {first:?})");
}

/// **A breeder carrying a litter whelps at its den**: the pack's adult, its breeder, with an
/// open gestation (an escrow carrying two young), awake and wanting nothing, at the west end
/// of the east leg; its den at the north leg's end. It walks home within the bound of the walk
/// to within half the den radius (where its pups stay, so they are born where they live), and
/// is still there a gestation's length later (its own short gestation). (The probe's breeders gave birth where they
/// stood, 5 to 13 metres from the den, and the pups walked home across ground they could not
/// cross: off the graph at a bend, or into water deeper than a pup wades.) Births off, so the
/// litter stays carried for the case.
#[test]
fn a_breeder_carrying_a_litter_whelps_at_its_den() {
    let mut c = config();
    // Its own short gestation (10 s), so the stay is a few hundred ticks, not the default's.
    c.founders[CH.index()].core.reproduction.gestation_s = 10.0;
    let world = world(&c, 703);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(c.clone());
    fauna.set_births_enabled(false);
    assert_lane(&fauna, &world);
    let (den, p, adult) = pack(&mut fauna, &world);
    assert_eq!(fauna.packs()[0].breeder, Some(adult), "the one adult is the breeder (the premise)");
    assert_eq!(fauna.packs()[0].id, p);
    place(&mut fauna, adult, XA + 4, ZA + MID, EAST);
    animal_mut(&mut fauna, adult).reproduction.escrow = Some(crate::Escrow { young: 2, ..crate::Escrow::default() });
    let route = f64::from(XC + MID - (XA + 4)) * V + f64::from(den.z - (ZA + MID)) * V;
    let max = bound(&fauna, route);
    let radius = 0.5 * c.founder(CH).pack.unwrap().den_radius_m;
    let (home, _, _, _, _) = walk(&mut fauna, &world, &mut flora, adult, den, radius, max);
    let a = animal(&fauna, adult);
    assert!(home.is_some(), "the breeder is not home within {max} ticks: at ({:.2}, {:.2})", a.pose.x, a.pose.z);
    let gestation = cubarium_voxel::ticks_at_least_one(c.founder(CH).core.reproduction.gestation_s) as u32;
    for _ in 0..gestation {
        fauna.step_with(&world, &mut flora, 1);
    }
    let (dx, dz) = centre(den.x, den.z);
    let a = animal(&fauna, adult);
    let (ex, ez) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, dx, dz, f64::from(WIDTH) * V);
    assert!((ex * ex + ez * ez).sqrt() <= radius, "the breeder left its den carrying its litter: at ({:.2}, {:.2})", a.pose.x, a.pose.z);
}
