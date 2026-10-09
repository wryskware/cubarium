//! **ROOSTS and RAIN** (backlog §12, Wrysk 2026-10-08; brief
//! `design/handoffs/roosts-and-rain-2026-10-08.md`): a seedporter at its rest hour prefers a
//! vaulttree crown, then a ledge top it climbs to up a latticevine wall, then today's
//! nearest stand, within `Glider::roost_budget_m` and den rest's reserve guard; rain at or
//! over `Flight::rain_ground_m_per_s` grounds a bellwing (under cover it can reach first),
//! until the rain is under that less the hysteresis, and a perched rain-grounded bellwing
//! pays its sleep upkeep.
//!
//! Hand-built fixtures at the 0.25 m reference voxel: soil at `y = 1..=2`, so a plain
//! column's support face is `y = 2` and its surface 0.75 m. The world is never stepped (its
//! clock and its rain stay put); a rain changes by stepping the fauna against a twin world.
//! Every number asserted is read from the config. No run is longer than 600 ticks.

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_flora::{
    Command as FloraCommand, Face, FaceDir, Flora, FloraConfig, Site, Species as Plant, VineSeed,
};

use crate::{Command, Fauna, FaunaConfig, Founder, MobilityMode, PerchAnchor, PerchKind, Pose, Scripted, StartingStores};

const V: f64 = 0.25;
const SP: Founder = Founder::Seedporter;
const BW: Founder = Founder::Bellwing;
const SURFACE: f64 = 0.75;
const EAST: f64 = std::f64::consts::FRAC_PI_2;

// ------------------------------------------------------------------ fixtures

fn config(width: u32, height: u32, depth: u32, day: DayClock, rain: f64) -> VoxelConfig {
    VoxelConfig {
        width,
        height,
        depth,
        voxel_m: V,
        seed: 7,
        day,
        rain_m_per_s: rain,
        ..VoxelConfig::default()
    }
}

fn soil(world: &mut World) {
    let c = world.config().clone();
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            for y in 1..=2 {
                world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
}

fn rock(world: &mut World, xs: std::ops::RangeInclusive<i64>, ys: std::ops::RangeInclusive<u32>) {
    let depth = world.config().depth;
    for z in 0..depth {
        for x in xs.clone() {
            for y in ys.clone() {
                world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Rock });
            }
        }
    }
}

/// A plain at midnight (the diurnal seedporter's rest hour; never stepped, so it stays).
fn night_plain(width: u32, height: u32, depth: u32) -> World {
    let day = DayClock { start_hour: 0.0, ..DayClock::default() };
    let mut w = World::empty(config(width, height, depth, day, 0.0));
    soil(&mut w);
    assert!(w.life_daylight() < crate::rhythm::fixed_config().life.day_line, "fixture: night");
    w
}

/// A plain with the clock off (the bellwing awake), raining at `rain` m/s (the open
/// budget's prescribed shower from the first tick; 0 is dry).
fn rain_plain(rain: f64) -> World {
    let day = DayClock { mode: DayMode::Off, ..DayClock::default() };
    let mut w = World::empty(config(20, 24, 8, day, rain));
    soil(&mut w);
    let got = f64::from(w.view().weather.rain_m_per_s);
    assert!((got - rain).abs() <= 1e-6 * rain, "fixture: the rain rate {got} (f32) for {rain}");
    w
}

fn seed(flora: &mut Flora, world: &World, species: Plant, x: i64, z: u32, wood: f64) -> Site {
    assert!(flora.apply(world, FloraCommand::Seed { x, z, species, wood }));
    let s = *flora
        .view()
        .stands
        .iter()
        .find(|s| i64::from(s.site.x) == x && s.site.z == z)
        .expect("the stand");
    assert!(s.foliage > 0.0, "fixture: a leafy {species:?}");
    s.site
}

/// A vaulttree small enough to climb in a test: the most wood whose crown is at most a
/// metre over the glider's shortest climbable stand.
fn small_vaulttree(flora: &mut Flora, world: &World, fauna: &Fauna, x: i64, z: u32) -> Site {
    let min = glider(fauna).climb_min_stand_m.max(crate::step::refuge_height_m(fauna.config()));
    let sc = flora.config().species(Plant::Vaulttree).clone();
    let mut wood = sc.wood_max;
    while sc.crown_height_m_at(wood) > min + 1.0 && wood > 1e-9 {
        wood *= 0.9;
    }
    assert!(sc.crown_height_m_at(wood) >= min, "fixture: a climbable vaulttree");
    seed(flora, world, Plant::Vaulttree, x, z, wood)
}

fn glider(fauna: &Fauna) -> crate::Glider {
    fauna.config().founder(SP).glider.expect("the seedporter glides")
}

fn on_ground(fauna: &mut Fauna, world: &World, founder: Founder, x: i64, z: u32) -> u64 {
    assert!(fauna.apply(
        world,
        Command::IntroduceFounder { x, z, founder, stores: StartingStores::FULL, heading_rad: EAST },
    ));
    let id = fauna.view().ledger.births - 1;
    let teachers = fauna.config().teachers;
    assert!(fauna.set_controller(id, crate::controller::heuristic(founder, teachers)));
    id
}

fn animal(fauna: &Fauna, id: u64) -> crate::Animal {
    *fauna.view().animal(id).expect("alive")
}

fn until(fauna: &mut Fauna, world: &World, flora: &mut Flora, cap: usize, done: impl Fn(&Fauna) -> bool) -> usize {
    let mut n = 0;
    while !done(fauna) && n < cap {
        fauna.step(world, flora);
        n += 1;
    }
    n
}

/// The stand a body is perched on.
fn perched_on(fauna: &Fauna, id: u64) -> Option<Site> {
    let a = animal(fauna, id);
    match (a.mobility.mode, a.mobility.perch.map(|p| p.anchor)) {
        (MobilityMode::Perched, Some(PerchAnchor::Stand { site, .. })) => Some(site),
        _ => None,
    }
}

fn asleep(fauna: &Fauna, id: u64) -> bool {
    animal(fauna, id).state == crate::State::Asleep
}

/// Horizontal metres from body `id` to the centre of column `site`.
fn distance(fauna: &Fauna, id: u64, site: Site) -> f64 {
    let a = animal(fauna, id);
    let (x, z) = ((f64::from(site.x) + 0.5) * V, (f64::from(site.z) + 0.5) * V);
    ((a.pose.x - x).powi(2) + (a.pose.z - z).powi(2)).sqrt()
}

/// The seedporter's night on a plain 32 × 24 × 9: on the ground at `(5, 4)` with a full
/// lanternberry (today's roost) at `(9, 4)` and, with `vault`, a small vaulttree at
/// `(16, 4)`; `tweak` edits its glider table. Where it sleeps, and the stands.
fn seedporter_night(vault: bool, tweak: impl Fn(&mut crate::Glider, f64)) -> (Option<Site>, Site, Option<Site>, bool) {
    let world = night_plain(32, 24, 9);
    let mut flora = Flora::new(FloraConfig::default());
    let mut cfg = crate::rhythm::fixed_config();
    let probe = Fauna::new(cfg.clone());
    let lantern_wood = flora.config().species(Plant::Lanternberry).wood_max;
    let lantern = seed(&mut flora, &world, Plant::Lanternberry, 9, 4, lantern_wood);
    let vt = vault.then(|| small_vaulttree(&mut flora, &world, &probe, 16, 4));
    // The vaulttree's distance from the lanternberry (the nearest it comes to it on its way
    // to today's roost), for a tweak that brackets it.
    let d = (16.0 - 9.0) * V;
    tweak(cfg.founders[SP.index()].glider.as_mut().expect("glider"), d);
    let mut fauna = Fauna::new(cfg);
    let id = on_ground(&mut fauna, &world, SP, 5, 4);
    until(&mut fauna, &world, &mut flora, 600, |f| asleep(f, id));
    (perched_on(&fauna, id), lantern, vt, asleep(&fauna, id))
}

// ------------------------------------------------------------------ seedporter roosts

/// **At dusk with a vaulttree crown in reach it roosts there**, past the nearer
/// lanternberry, and sleeps.
#[test]
fn a_seedporter_at_night_with_a_reachable_vaulttree_roosts_in_its_crown() {
    let (on, lantern, vt, slept) = seedporter_night(true, |_, _| {});
    let vt = vt.expect("the vaulttree");
    assert!(slept, "it fell asleep");
    assert_eq!(on, Some(vt), "in the vaulttree's crown, not the lanternberry at {lantern:?}");
}

/// With no safe crown available the roost chooser does not nominate a low bush.
#[test]
fn with_no_safe_vaulttree_or_ledge_it_does_not_roost_in_a_low_bush() {
    let world = night_plain(32, 24, 9);
    let mut flora = Flora::new(FloraConfig::default());
    let wood = flora.config().lanternberry.wood_max;
    seed(&mut flora, &world, Plant::Lanternberry, 9, 4, wood);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = on_ground(&mut fauna, &world, SP, 5, 4);
    let i = fauna.animals.iter().position(|a| a.id == id).unwrap();
    assert!(crate::step::better_roost(&mut fauna, i, &world.view(), &flora.view()).is_none(),
        "no unsafe bush is nominated as a roost");
}

/// A safe crown is still subject to the travel budget and reserve guard.
#[test]
fn a_vaulttree_out_of_budget_or_past_the_reserve_guard_is_skipped() {
    for reserve_blocked in [false, true] {
        let world = night_plain(32, 24, 9);
        let mut cfg = FaunaConfig::default();
        let g = cfg.founders[SP.index()].glider.as_mut().unwrap();
        if reserve_blocked { g.roost_reserve_share = 0.0; } else { g.roost_budget_m = V; }
        let mut fauna = Fauna::new(cfg);
        let mut flora = Flora::new(FloraConfig::default());
        small_vaulttree(&mut flora, &world, &fauna, 16, 4);
        let id = on_ground(&mut fauna, &world, SP, 5, 4);
        let i = fauna.animals.iter().position(|a| a.id == id).unwrap();
        assert!(crate::step::better_roost(&mut fauna, i, &world.view(), &flora.view()).is_none(),
            "the safe crown is skipped when travel cannot be afforded");
    }
}

/// A rock mesa over columns `x = 12..=15` up to `y = 8` (its top 1.5 m over the plain), on
/// a plain 32 × 24 × 5 at night (or with the clock off), a latticevine covering its west face on slab `z = 2` from
/// the foot (`y = 3`) to the lip row (`y = 8`); the flora.
fn vine_mesa(night: bool) -> (World, Flora) {
    vine_mesa_at(night, 8)
}

fn vine_mesa_at(night: bool, top: u32) -> (World, Flora) {
    let mut world = if night {
        night_plain(32, 24, 5)
    } else {
        let day = DayClock { mode: DayMode::Off, ..DayClock::default() };
        let mut w = World::empty(config(32, 24, 5, day, 0.0));
        soil(&mut w);
        w
    };
    rock(&mut world, 12..=15, 1..=top);
    let mut flora = Flora::new(FloraConfig::default());
    let first = Face::new(12, 3, 2, FaceDir::NegX);
    let id = flora
        .seed_vine(&world, VineSeed { root: Site { x: 11, y: 2, z: 2 }, first, reserve: 100.0, lineage: None })
        .expect("fixture: the vine");
    for y in 4..=top {
        assert!(flora.cover_face(&world, id, Face::new(12, y, 2, FaceDir::NegX), 1.0));
    }
    (world, flora)
}

/// **With only a vine-reachable mesa it climbs up**: from the plain it walks to the vine,
/// climbs it, pulls itself over the lip and sleeps perched on the top.
#[test]
fn with_only_a_vine_mesa_in_reach_it_climbs_up_and_roosts_on_top() {
    let (world, mut flora) = vine_mesa(true);
    let mut cfg = crate::rhythm::fixed_config();
    cfg.founders[Founder::Chorister.index()].hunter.as_mut().unwrap().vertical_reach_m = 1.0;
    let mut fauna = Fauna::new(cfg);
    assert!(glider(&fauna).roost_ledge_min_m <= 6.0 * V, "fixture: the mesa is a ledge worth roosting on");
    let id = on_ground(&mut fauna, &world, SP, 6, 2);
    let n = until(&mut fauna, &world, &mut flora, 600, |f| asleep(f, id));
    let a = animal(&fauna, id);
    assert!(asleep(&fauna, id), "it fell asleep ({n} ticks): {:?}", a.mobility);
    assert_eq!(a.mobility.mode, MobilityMode::Perched);
    assert_eq!(a.mobility.perch.map(|p| p.kind), Some(PerchKind::Face), "on the top");
    assert!((a.pose.y_m - 9.0 * V).abs() < 1e-9, "on the mesa's top: {}", a.pose.y_m);
    assert!(fauna.view().ledger.climbs > 0, "it climbed");
    // With the preference off it does not (no stand in reach: it stays awake).
    let (world, mut flora) = vine_mesa(true);
    let mut cfg = crate::rhythm::fixed_config();
    cfg.founders[SP.index()].glider.as_mut().unwrap().roost_budget_m = 0.0;
    let mut fauna = Fauna::new(cfg);
    let id = on_ground(&mut fauna, &world, SP, 6, 2);
    until(&mut fauna, &world, &mut flora, 300, |f| animal(f, id).pose.y_m > SURFACE + V);
    assert!(animal(&fauna, id).pose.y_m < 9.0 * V, "off, it does not roost on the mesa");
}

/// **A vine climb that tops out perches on the top**, whatever drives it: held forward at
/// the vine's foot it climbs, pulls over the lip, and ends perched on the top face; forward
/// again stands it up to walk.
#[test]
fn a_vine_climb_to_a_reachable_lip_ends_perched_on_the_top_and_forward_walks_off() {
    let (world, mut flora) = vine_mesa(false);
    let mut fauna = Fauna::new(crate::rhythm::fixed_config());
    let id = on_ground(&mut fauna, &world, SP, 11, 2);
    assert!(fauna.set_controller(id, Box::new(Scripted::new(vec![crate::Actions { forward: 1.0, turn: 0.0, feed: 0.0 }]))));
    let n = until(&mut fauna, &world, &mut flora, 400, |f| {
        animal(f, id).mobility.perch.is_some_and(|p| p.kind == PerchKind::Face)
    });
    let a = animal(&fauna, id);
    assert_eq!(a.mobility.mode, MobilityMode::Perched, "perched after {n} ticks: {:?}", a.mobility);
    assert_eq!(a.site, Site { x: 12, y: 8, z: 2 }, "on the wall column's top");
    fauna.step(&world, &mut flora);
    assert_eq!(animal(&fauna, id).mobility.mode, MobilityMode::Ground, "forward stands it up");
}

// ------------------------------------------------------------------ bellwing rain

fn aloft(fauna: &mut Fauna, world: &World, x: f64, z: f64, y: f64) -> u64 {
    assert!(fauna.apply(
        world,
        Command::IntroduceAloft { pose: Pose { x, z, heading_rad: EAST, y_m: y }, founder: BW, stores: StartingStores::FULL },
    ));
    fauna.view().ledger.births - 1
}

fn fly(fauna: &mut Fauna, id: u64, forward: f64, lift: f64) {
    assert!(fauna.set_controller(
        id,
        Box::new(Scripted::new(vec![crate::Actions { forward, turn: 0.0, feed: 1.0 }]).lifting(lift)),
    ));
}

fn flight(fauna: &Fauna) -> crate::Flight {
    fauna.config().founder(BW).flight.expect("the bellwing flies")
}

/// **A bellwing lands when the rain crosses the threshold, stays down through the
/// hysteresis band, and takes off under it.** Hovering 1.5 m up in the dry it stays aloft;
/// at the threshold it comes down and perches, latched; at a rate between the threshold
/// less the hysteresis and the threshold, full lift does not take it up; under that band
/// the same lift does.
#[test]
fn rain_over_the_threshold_grounds_a_bellwing_until_it_falls_under_the_hysteresis() {
    let probe = Fauna::new(crate::rhythm::fixed_config());
    let f = flight(&probe);
    let (thr, hyst) = (f.rain_ground_m_per_s, f.rain_ground_hysteresis_m_per_s);
    assert!(thr > 0.0 && hyst > 0.0 && hyst < thr, "fixture: a band to test");
    let (dry, wet, band, under) = (rain_plain(0.0), rain_plain(thr * (1.0 + 1e-4)), rain_plain(thr - 0.5 * hyst), rain_plain(thr - 1.5 * hyst));
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(crate::rhythm::fixed_config());
    let (x, z) = ((8.5) * V, 4.5 * V);
    let id = aloft(&mut fauna, &dry, x, z, SURFACE + 1.5);
    fly(&mut fauna, id, 0.0, 0.0);
    for _ in 0..20 {
        fauna.step(&dry, &mut flora);
    }
    assert_eq!(animal(&fauna, id).mobility.mode, MobilityMode::Fly, "dry, it hovers");
    let n = until(&mut fauna, &wet, &mut flora, 300, |f| animal(f, id).mobility.mode == MobilityMode::Perched);
    let a = animal(&fauna, id);
    assert_eq!(a.mobility.mode, MobilityMode::Perched, "at the threshold it came down ({n} ticks)");
    assert!(a.mobility.rain_grounded);
    fly(&mut fauna, id, 0.0, 1.0);
    for _ in 0..40 {
        fauna.step(&band, &mut flora);
    }
    let a = animal(&fauna, id);
    assert_eq!(a.mobility.mode, MobilityMode::Perched, "inside the hysteresis band it stays down");
    assert!(a.mobility.rain_grounded);
    for _ in 0..20 {
        fauna.step(&under, &mut flora);
    }
    let a = animal(&fauna, id);
    assert!(!a.mobility.rain_grounded, "under the band the latch is off");
    assert_eq!(a.mobility.mode, MobilityMode::Fly, "and the same lift takes it up");
}

/// **It prefers cover**: grounded by rain under a full-grown umbrellafrond's lowest slab,
/// holding full down-lift, it rises and hangs under the slab; in the open the same body comes
/// down onto the ground.
#[test]
fn a_rain_grounded_bellwing_hangs_under_cover_it_can_reach_else_lands_below() {
    let probe = Fauna::new(crate::rhythm::fixed_config());
    let wet = rain_plain(flight(&probe).rain_ground_m_per_s * 2.0);
    let mut flora = Flora::new(FloraConfig::default());
    let wood = flora.config().species(Plant::Umbrellafrond).wood_max;
    let site = seed(&mut flora, &wet, Plant::Umbrellafrond, 8, 4, wood);
    let lowest = flora
        .view()
        .layers_at(site)
        .filter(|l| l.foliage_index.is_some())
        .min_by_key(|l| l.cell)
        .expect("a slab");
    let underside = lowest.cell as f64 * V;
    let stand = *flora.view().stand_at(site).unwrap();
    assert!(crate::layer_columns(&wet.view(), &stand, &lowest).contains(&(10, 4)), "fixture: the slab covers column 10");
    for (x, covered) in [(10.5 * V, true), (17.5 * V, false)] {
        let mut fauna = Fauna::new(crate::rhythm::fixed_config());
        let id = aloft(&mut fauna, &wet, x, 4.5 * V, SURFACE + 0.3);
        fly(&mut fauna, id, 0.0, -1.0);
        let n = until(&mut fauna, &wet, &mut flora, 300, |f| animal(f, id).mobility.mode == MobilityMode::Perched);
        let a = animal(&fauna, id);
        assert_eq!(a.mobility.mode, MobilityMode::Perched, "perched in {n} ticks");
        if covered {
            assert_eq!(a.mobility.perch.map(|p| p.kind), Some(PerchKind::Underside), "under the frond");
            assert!(a.pose.y_m < underside && a.pose.y_m > SURFACE + 0.3);
        } else {
            assert_eq!(a.mobility.perch.map(|p| p.kind), Some(PerchKind::Face), "on the ground");
            assert!((a.pose.y_m - SURFACE).abs() < 0.02);
        }
    }
}

/// **Grounded by rain it pays its sleep upkeep**: a perched bellwing's maintenance a second
/// in rain over the threshold is `sleep_upkeep` × what the same perched body pays in the
/// dry.
#[test]
fn a_rain_grounded_perched_bellwing_pays_its_sleep_upkeep() {
    let probe = Fauna::new(crate::rhythm::fixed_config());
    let sleep = probe.config().founder(BW).sleep_upkeep;
    let rate = |world: &World| {
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(crate::rhythm::fixed_config());
        let id = on_ground(&mut fauna, world, BW, 8, 4);
        fly(&mut fauna, id, 0.0, 0.0);
        fauna.step(world, &mut flora);
        assert_eq!(animal(&fauna, id).mobility.mode, MobilityMode::Perched);
        let before = fauna.view().ledger.respired_maintenance_out;
        let n = 20;
        for _ in 0..n {
            fauna.step(world, &mut flora);
        }
        let paid = fauna.view().ledger.respired_maintenance_out - before;
        (paid / (n as f64 * DT), animal(&fauna, id).mobility.rain_grounded)
    };
    let (dry, dry_latched) = rate(&rain_plain(0.0));
    let (wet, wet_latched) = rate(&rain_plain(flight(&probe).rain_ground_m_per_s * 2.0));
    assert!(!dry_latched && wet_latched);
    assert!(dry > 0.0);
    assert!((wet / dry - sleep).abs() < 1e-6, "rain upkeep {wet} = {sleep} x dry {dry}");
}

/// A nearer low vaulttree is skipped for a crown above the configured hunter reach.
#[test]
fn night_roost_selection_rejects_a_low_vaulttree_and_chooses_the_high_one() {
    let world = night_plain(40, 40, 9);
    let mut cfg = FaunaConfig::default();
    cfg.founders[Founder::Chorister.index()]
        .hunter
        .as_mut()
        .unwrap()
        .vertical_reach_m = 2.0;
    cfg.founders[SP.index()]
        .glider
        .as_mut()
        .unwrap()
        .roost_reserve_share = 1.0;
    let reach = crate::step::refuge_height_m(&cfg);
    let mut fc = FloraConfig::default();
    fc.vaulttree.crown_height_m = [0.25 * reach, 2.0 * reach];
    let max = fc.vaulttree.wood_max;
    let mut flora = Flora::new(fc);
    let low = seed(&mut flora, &world, Plant::Vaulttree, 9, 4, 0.1 * max);
    let high = seed(&mut flora, &world, Plant::Vaulttree, 16, 4, max);
    assert!(
        !crate::step::safe_crown(&flora.view(), low, V, reach),
        "fixture: lower crown is unsafe"
    );
    assert!(
        crate::step::safe_crown(&flora.view(), high, V, reach),
        "fixture: higher crown is safe"
    );
    let mut fauna = Fauna::new(cfg);
    let id = on_ground(&mut fauna, &world, SP, 5, 4);
    let i = fauna.animals.iter().position(|a| a.id == id).unwrap();
    let choice = crate::step::better_roost(&mut fauna, i, &world.view(), &flora.view());
    assert!(
        matches!(choice, Some(crate::step::Roost::Crown(_, site)) if site == high),
        "chooses the safe crown: {choice:?}"
    );
}

/// The ledge minimum is an extra requirement; satisfying it alone grants no safety.
#[test]
fn night_roost_selection_rejects_a_low_vine_ledge_and_accepts_a_high_one() {
    for top in [8, 12] {
        let (world, flora) = vine_mesa_at(true, top);
        let mut cfg = FaunaConfig::default();
        cfg.founders[Founder::Chorister.index()]
            .hunter
            .as_mut()
            .unwrap()
            .vertical_reach_m = 2.0;
        let g = cfg.founders[SP.index()].glider.as_mut().unwrap();
        g.roost_ledge_min_m = 0.0;
        g.roost_reserve_share = 1.0;
        let reach = crate::step::refuge_height_m(&cfg);
        let safe = (f64::from(top) + 1.0) * V - SURFACE > reach;
        let mut fauna = Fauna::new(cfg);
        let id = on_ground(&mut fauna, &world, SP, 6, 2);
        let i = fauna.animals.iter().position(|a| a.id == id).unwrap();
        let choice = crate::step::better_roost(&mut fauna, i, &world.view(), &flora.view());
        assert_eq!(
            matches!(choice, Some(crate::step::Roost::Ledge(..))),
            safe,
            "ledge top {top} must clear the configured reach: {choice:?}"
        );
    }
}
