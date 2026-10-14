//! **SP-MEAL's laws** (2026-10-14; written before the fixes, from the wide-world probe of
//! seedporter larva meals):
//!
//! - **a meal at the mouth is eaten to its end**: a seedporter craving nitrogen, perched in a
//!   vaulttree crown with hatched larvae at its mouth and no nuts, keeps eating there until it
//!   is sated or the larvae are gone; it sets no check aim and takes no check walk meanwhile,
//!   though other hosts are in the check's reach (the probe: with larvae and no fruit at its
//!   mouth its forager tasted nothing and walked or glided off after a bite or two);
//! - **larvae on a latticevine are eaten as on a vaulttree**: a hungry seedporter on the
//!   ground beside a vine whose larvae are within its mouth's reach eats them, staying until
//!   sated or the larvae are gone, whichever way it faces (the probe: it walked along the
//!   wall past them, biting in passing);
//! - **a vine's larvae are seen on any leafy face of the vine**: a hungry seedporter within
//!   its larva sight of a leafy face of a vine holding larvae sees them there and walks to
//!   them by sight, though the vine's anchor face (where its larvae were laid) is beyond that
//!   sight; its mouth reaches them on any leafy face of the vine, and so does its eye.
//!
//! Fixtures are hand-built at the 0.25 m reference voxel as in `larva_tests`: soil at
//! `y = 1..=2`, a rock wall across the world at `x = 16` carrying a latticevine on its `−x` faces at `z = 2`,
//! rows 3..=10. Every bound comes from the config or is a test horizon; no shipped number,
//! ratio or world hash is asserted.

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command as FloraCommand, Face, FaceDir, Flora, FloraConfig, Site, Species as Plant, VineSeed};

use crate::larva::{Cohort, LarvaHost};
use crate::{Actions, Command, Fauna, FaunaConfig, Food, Founder, MobilityMode, Pose, Scripted, StartingStores, Steer};

const V: f64 = 0.25;
const BW: Founder = Founder::Bellwing;
const SP: Founder = Founder::Seedporter;
const EAST: f64 = std::f64::consts::FRAC_PI_2;
const WALL_X: i64 = 16;
const WALL_Z: u32 = 2;
/// A test horizon: ten minutes of ticks.
const HORIZON: usize = 20 * 60 * 10;

fn world(width: u32, height: u32, depth: u32) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height,
        depth,
        voxel_m: V,
        seed: 7,
        day: DayClock { mode: DayMode::Off, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    let c = w.config().clone();
    let m = cubarium_voxel::Materials::DEFAULT;
    let cap = m.pore_capacity(Material::Soil) * c.voxel_volume();
    let fc = m.field_capacity(Material::Soil);
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            for y in 1..=2 {
                w.apply(WorldCommand::AddWater { x, y, z, volume_m3: fc * cap });
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
        for y in 1..=12 {
            w.apply(WorldCommand::SetMaterial { x: WALL_X, y, z, material: Material::Rock });
        }
    }
    w
}

/// The vine on the wall's `−x` faces at `z = WALL_Z`, rows 3..=10, every face leafy.
fn vine(flora: &mut Flora, world: &World) -> (u64, Vec<Face>) {
    let first = Face::new(WALL_X as u32, 3, WALL_Z, FaceDir::NegX);
    let id = flora
        .seed_vine(world, VineSeed { root: Site { x: WALL_X as u32 - 1, y: 2, z: WALL_Z }, first, reserve: 100.0, lineage: None })
        .expect("fixture: the vine");
    let mut faces = vec![first];
    for y in 4..=10 {
        let f = Face::new(WALL_X as u32, y, WALL_Z, FaceDir::NegX);
        assert!(flora.cover_face(world, id, f, 1.0), "fixture: cover {f:?}");
        faces.push(f);
    }
    (id, faces)
}

/// A full-grown vaulttree on `(x, z)` hanging no nuts: its site and id.
fn vaulttree(flora: &mut Flora, world: &World, x: i64, z: u32) -> (Site, u64) {
    let wood = flora.config().species(Plant::Vaulttree).wood_max;
    assert!(flora.apply(world, FloraCommand::Seed { x, z, species: Plant::Vaulttree, wood }));
    let fv = flora.view();
    let s = fv.stands.iter().find(|s| i64::from(s.site.x) == x && s.site.z == z).expect("the stand");
    (s.site, s.id)
}

fn flora_config() -> FloraConfig {
    let mut c = FloraConfig::default();
    c.vaulttree.fruit_per_s = 0.0;
    c.vaulttree.fruit_cap = 0.0;
    c
}

/// Every lineage awake, no ground response, no troop pull, no bellwing catching.
fn config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for p in &mut c.founders {
        p.active = crate::Activity::Always;
        p.vigilance = crate::Vigilance::default();
        p.catch_bellwings = false;
        p.herding = crate::Herding::default();
    }
    c
}

/// `n` hatched larvae of `body` each on `host`.
fn put(fauna: &mut Fauna, host: LarvaHost, site: Site, anchor: Option<Face>, n: f64, body: f64) {
    let sc = fauna.config().founder(BW).core;
    let organic = n * body;
    let age_s = sc.reproduction.incubation_s + 100.0;
    let c = Cohort { count: n, organic, mineral: sc.n_tissue * organic, energy: sc.energy_density * organic, age_s, unfed_s: 0.0 };
    fauna.introduce_larvae(BW, host, site, anchor, c);
}

fn egg(fauna: &Fauna) -> f64 {
    fauna.config().founder(BW).core.reproduction.egg_organic
}

fn index(fauna: &Fauna, id: u64) -> usize {
    fauna.animals.iter().position(|a| a.id == id).expect("the body")
}

fn animal(fauna: &Fauna, id: u64) -> crate::Animal {
    fauna.animals[index(fauna, id)]
}

fn larva_bites(fauna: &Fauna) -> u64 {
    fauna.view().ledger.bites_by_founder_food[SP.index()][Food::Larva.index()]
}

fn larvae_at_mouth(fauna: &Fauna, world: &World, flora: &Flora, id: u64) -> Option<LarvaHost> {
    let a = animal(fauna, id);
    let phys = *fauna.config().founder(SP);
    crate::step::glider_larva_in_reach(fauna, &world.view(), &flora.view(), &a, &phys).map(|(h, _)| h)
}

/// **The meal law**: from now, while body `id` wants food and `host`'s larvae remain, its
/// mouth stays at them; with `no_check`, it sets no check aim and takes no check walk. Runs to
/// the meal's end (sated, or the larvae gone) within the horizon: how it ended.
fn eats_to_the_end(fauna: &mut Fauna, world: &World, flora: &mut Flora, id: u64, host: LarvaHost, no_check: bool) -> &'static str {
    let bites0 = larva_bites(fauna);
    let mut ended = None;
    for k in 0..HORIZON {
        fauna.step(world, flora);
        let a = animal(fauna, id);
        if !a.eating {
            ended = Some("sated");
            break;
        }
        if crate::larva::visible(fauna, host).is_none() {
            ended = Some("the larvae gone");
            break;
        }
        assert_eq!(
            larvae_at_mouth(fauna, world, flora, id),
            Some(host),
            "tick {k}: it left the larvae hungry with larvae left (steer {:?}, mode {:?}, at {:?})",
            fauna.steer_of(id),
            a.mobility.mode,
            (a.pose.x, a.pose.y_m, a.pose.z)
        );
        if no_check {
            assert!(crate::treecheck::aim_of(fauna, id).is_none(), "tick {k}: no check aim while it eats");
            assert_ne!(fauna.steer_of(id), Some(Steer::Check), "tick {k}: no check walk while it eats");
        }
    }
    assert!(larva_bites(fauna) > bites0, "it ate the larvae");
    ended.expect("the meal ends within the horizon (sated or the larvae gone)")
}

/// **A seedporter craving nitrogen keeps eating the larvae at its mouth in a crown until sated
/// or the larvae are gone, and sets no check aim meanwhile.** Perched in a vaulttree crown
/// holding hatched larvae and no nuts, on its own heuristic; another crowned vaulttree and a
/// leafy vine are in the check's reach.
#[test]
fn a_nitrogen_craving_seedporter_eating_larvae_in_a_crown_keeps_eating_until_sated_or_the_larvae_are_gone() {
    let world = world(48, 64, 16);
    let mut flora = Flora::new(flora_config());
    let tree = vaulttree(&mut flora, &world, 24, 8);
    vaulttree(&mut flora, &world, 34, 8);
    vine(&mut flora, &world);
    let mut fauna = Fauna::new(config());
    fauna.set_births_enabled(false);
    let host = LarvaHost::Stand(tree.1);
    let e = egg(&fauna);
    put(&mut fauna, host, tree.0, None, 20.0, 20.0 * e);
    let top = flora.view().layers_at(tree.0).filter(|l| l.foliage_index.is_some()).map(|l| l.cell).max().expect("a crown");
    let (x, z) = ((f64::from(tree.0.x) + 0.5) * V, (f64::from(tree.0.z) + 0.5) * V);
    // A growing young (under its breeding size, its reserve near full): it craves nitrogen.
    let core = fauna.config().founder(SP).core;
    let stores = StartingStores { body: 0.5 * (core.body_min + core.birth_body) / core.body_max, reserve: 0.9 };
    assert!(fauna.apply(&world, Command::IntroduceAloft { pose: Pose { x, z, heading_rad: EAST, y_m: (top + 1) as f64 * V }, founder: SP, stores }));
    let id = fauna.view().ledger.births - 1;
    assert!(fauna.set_controller(id, Box::new(Scripted::new(vec![Actions { forward: 0.0, turn: 0.0, feed: 0.0 }]))));
    for _ in 0..40 {
        if animal(&fauna, id).mobility.mode == MobilityMode::Perched {
            break;
        }
        fauna.step(&world, &mut flora);
    }
    assert_eq!(animal(&fauna, id).mobility.mode, MobilityMode::Perched, "fixture: perched in the crown");
    assert_eq!(larvae_at_mouth(&fauna, &world, &flora, id), Some(host), "fixture: the larvae at its mouth");
    let craved = crate::crave::cravings(&fauna, index(&fauna, id));
    assert_eq!(craved.need(), crate::crave::Need::Nitrogen, "fixture: it craves nitrogen: {craved:?}");
    assert!(animal(&fauna, id).eating, "fixture: it wants food");
    {
        let fv = flora.view();
        assert_eq!(fv.stand_at(tree.0).expect("the tree").fruit, 0.0, "fixture: no nuts");
    }
    let teachers = fauna.config.teachers;
    fauna.controllers.set(id, crate::heuristic(SP, teachers));
    let how = eats_to_the_end(&mut fauna, &world, &mut flora, id, host, true);
    assert!(fauna.check_tally().walks == 0, "no check walk set out on: {:?} (ended: {how})", fauna.check_tally());
}

/// **A hungry seedporter beside a latticevine whose larvae are within its mouth's reach eats
/// them until sated or they are gone**, facing the wall or along it, on its own heuristic.
#[test]
fn a_seedporter_beside_a_latticevine_with_larvae_in_reach_eats_them_until_sated_or_gone() {
    for heading in [EAST, 0.0, std::f64::consts::PI] {
        let world = world(40, 64, 16);
        let mut flora = Flora::new(flora_config());
        let vine = vine(&mut flora, &world);
        let mut fauna = Fauna::new(config());
        fauna.set_births_enabled(false);
        let host = LarvaHost::Vine(vine.0);
        let root = flora.view().cover.vine(vine.0).expect("the vine").root;
        let e = egg(&fauna);
        put(&mut fauna, host, root, Some(vine.1[0]), 20.0, 5.0 * e);
        assert!(fauna.apply(
            &world,
            Command::IntroduceFounder { x: WALL_X - 1, z: WALL_Z, founder: SP, stores: StartingStores { body: 1.0, reserve: 0.3 }, heading_rad: heading },
        ));
        let id = fauna.animals.last().expect("introduced").id;
        let teachers = fauna.config.teachers;
        fauna.controllers.set(id, crate::heuristic(SP, teachers));
        assert_eq!(larvae_at_mouth(&fauna, &world, &flora, id), Some(host), "fixture: the vine's larvae at its mouth (heading {heading})");
        fauna.step(&world, &mut flora);
        assert!(animal(&fauna, id).eating, "fixture: it wants food");
        eats_to_the_end(&mut fauna, &world, &mut flora, id, host, false);
    }
}

/// **A vine's larvae are seen on a leafy face of the vine within the seedporter's larva
/// sight, though the vine's anchor face is beyond it; it walks to them by sight and eats
/// them.** Hungry on the ground west of the wall facing it, nearer the vine's low faces than
/// its larva sight, its anchor (the top face) farther.
#[test]
fn a_vines_larvae_are_seen_on_any_leafy_face_of_the_vine_within_larva_sight() {
    let world = world(40, 64, 16);
    let mut flora = Flora::new(flora_config());
    let vine = vine(&mut flora, &world);
    let mut fauna = Fauna::new(config());
    fauna.set_births_enabled(false);
    let host = LarvaHost::Vine(vine.0);
    let root = flora.view().cover.vine(vine.0).expect("the vine").root;
    let anchor = *vine.1.last().expect("the top face");
    let e = egg(&fauna);
    put(&mut fauna, host, root, Some(anchor), 20.0, 5.0 * e);
    let sight = fauna.config().founder(SP).larva_sight_m;
    assert!(sight > 0.0, "the seedporter looks for larvae");
    let col = WALL_X - 1 - ((0.8 * sight) / V).round() as i64;
    assert!(fauna.apply(
        &world,
        Command::IntroduceFounder { x: col, z: WALL_Z, founder: SP, stores: StartingStores { body: 1.0, reserve: 0.3 }, heading_rad: EAST },
    ));
    let id = fauna.animals.last().expect("introduced").id;
    let i = index(&fauna, id);
    let view = world.view();
    let d_eye = |f: Face| {
        let a = fauna.animals[i];
        let g = crate::body_of(fauna.config(), &a).expect("a body");
        let o = crate::senses::cone_origin(&view, &a.pose, a.sense_layer(V), &g);
        let (x, y, z) = f.front(&view).expect("a front");
        let (sx, my, sz) = ((f64::from(x) + 0.5) * V, (f64::from(y) + 0.5) * V, (f64::from(z) + 0.5) * V);
        let ring = f64::from(view.config.width) * V;
        let (dx, dz) = crate::pose::wrapped_delta(o.0, o.2, sx, sz, ring);
        dx.hypot(dz).hypot(my - o.1)
    };
    assert!(d_eye(anchor) > sight, "fixture: the anchor beyond larva sight ({} m > {sight} m)", d_eye(anchor));
    assert!(d_eye(vine.1[0]) < sight, "fixture: the low face within it ({} m < {sight} m)", d_eye(vine.1[0]));
    assert!(larvae_at_mouth(&fauna, &world, &flora, id).is_none(), "fixture: nothing at its mouth yet");
    let fronts: Vec<(f64, f64)> = vine
        .1
        .iter()
        .map(|f| {
            let (x, _, z) = f.front(&view).expect("a front");
            ((f64::from(x) + 0.5) * V, (f64::from(z) + 0.5) * V)
        })
        .collect();
    let seen = crate::step::fruit_candidates(&fauna, i, &view, &flora.view(), 100.0).expect("a glider");
    assert!(
        seen.iter().any(|c| fronts.iter().any(|&(fx, fz)| (c.3.0 - fx).abs() < 1e-9 && (c.3.2 - fz).abs() < 1e-9)),
        "it sees the larvae on the vine's near faces: {seen:?}"
    );
    let teachers = fauna.config.teachers;
    fauna.controllers.set(id, crate::heuristic(SP, teachers));
    let mut first = None;
    for _ in 0..HORIZON {
        fauna.step(&world, &mut flora);
        first = first.or(fauna.steer_of(id).filter(|s| matches!(s, Steer::Fruit | Steer::Check)));
        if larva_bites(&fauna) > 0 {
            break;
        }
    }
    assert_eq!(first, Some(Steer::Fruit), "it walks to them by sight");
    assert!(larva_bites(&fauna) > 0, "and eats them");
}
