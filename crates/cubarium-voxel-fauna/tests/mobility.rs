//! **Mobility** (`design/handoffs/voxel-mobility-2026-09-23.md`), written before the rule:
//! shredders climb terrain walls, browsers take ledges up to their own height and drops
//! up to their own length, and water is waded and drowned in by the body's current
//! height, with a 60 s drowning.
//!
//! Small hand-built worlds, one founder each, a scripted controller holding full forward
//! effort, a few hundred ticks at most (the drowning case is 2,000 ticks of one body).

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species as Plant};

use cubarium_voxel_fauna::{
    Actions, Command as FaunaCommand, Departure, Fauna, FaunaConfig, Founder, RouteMap, RouteRule,
    Scripted, StartingStores, StepLimits,
};

const EAST: f64 = std::f64::consts::FRAC_PI_2;
const WEST: f64 = 3.0 * std::f64::consts::FRAC_PI_2;

/// A strip `width × depth` at `voxel_m`, `height` tall, whose column `x` is soil from
/// `y = 1` up to `top(x)`: the support face of column `x` is `top(x)`.
fn terrain(width: u32, depth: u32, height: u32, voxel_m: f64, top: impl Fn(i64) -> u32) -> World {
    let mut world = World::empty(VoxelConfig {
        width,
        height,
        depth,
        voxel_m,
        ..VoxelConfig::default()
    });
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=top(x) {
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    world
}

fn solid(world: &mut World, x: i64, y: u32, z: u32) {
    world.apply(WorldCommand::SetMaterial {
        x,
        y,
        z,
        material: Material::Soil,
    });
}

/// `depth_m` of free water over the support face `(x, top, z)`, in the cells above it.
fn flood(world: &mut World, x: i64, top: u32, z: u32, depth_m: f64) {
    let v = world.config().voxel_m;
    let mut left = depth_m;
    let mut y = top + 1;
    while left > 1e-12 {
        let d = left.min(v);
        let got = world.apply(WorldCommand::AddWater {
            x,
            y,
            z,
            volume_m3: d * v * v,
        });
        assert!(got > 0.0, "the void over ({x}, {top}, {z}) took water");
        left -= d;
        y += 1;
    }
}

/// One founder at column `(x, z)` facing `heading`, full stores, holding `forward`.
fn walker(
    world: &World,
    founder: Founder,
    x: i64,
    z: u32,
    heading: f64,
    forward: f64,
) -> (Fauna, u64) {
    let mut fauna = Fauna::new(FaunaConfig::default());
    fauna.set_births_enabled(false);
    let id = fauna.view().ledger.births;
    assert!(fauna.apply(
        world,
        FaunaCommand::IntroduceFounder {
            x,
            z,
            founder,
            stores: StartingStores::FULL,
            heading_rad: heading,
        },
    ));
    assert!(fauna.set_controller(
        id,
        Box::new(Scripted::new(vec![Actions {
            forward,
            turn: 0.0,
            feed: 0.0,
        }])),
    ));
    (fauna, id)
}

/// 0.25 m ground at `y = 2`, and a plateau whose face is `y = 2 + rise` over columns
/// `from..=to`.
fn plateau(width: u32, depth: u32, rise: u32, from: i64, to: i64) -> World {
    terrain(width, depth, 14, 0.25, |x| {
        if (from..=to).contains(&x) {
            2 + rise
        } else {
            2
        }
    })
}

// ----------------------------------------------------------------- 1. the ascent

/// Test 1: a shredder against a 4-voxel (1 m) wall reaches the top face in about
/// `height / (0.5 · pace)`, and each climbing tick pays the climbing factor on its motor
/// cost.
#[test]
fn a_shredder_climbs_a_wall_at_half_pace_and_pays_the_climbing_factor() {
    let world = plateau(32, 3, 4, 6, 12);
    let flora = Flora::new(FloraConfig::default());
    let (mut fauna, id) = walker(&world, Founder::Blind, 2, 1, EAST, 1.0);
    let phys = *fauna.config().founder(Founder::Blind);
    assert!(phys.climbs_walls);
    let pace = Founder::Blind.manifest().cruise_m_per_s;
    let expected = 1.0 / (0.5 * pace) / DT;

    let mut flora = flora;
    let (mut start, mut arrive) = (None, None);
    let mut factor_checked = false;
    for t in 1..=260u32 {
        let motor_before = fauna.view().ledger.respired_motor_out;
        fauna.step(&world, &mut flora);
        let a = *fauna.view().animal(id).expect("alive");
        let climbing = a.mobility.climb.is_some();
        if climbing && start.is_none() {
            start = Some(t);
        } else if climbing && !factor_checked {
            // A whole climbing tick at full effort: the walking cost at cruise, times the
            // climbing factor.
            let paid = fauna.view().ledger.respired_motor_out - motor_before;
            let want = phys.motor_respiration_per_s * a.body * phys.wall_climb_cost_factor * DT;
            assert!(
                (paid - want).abs() <= 1e-12 * want,
                "a climbing tick paid {paid}, not {want}"
            );
            assert!(
                phys.wall_climb_cost_factor > 1.0,
                "lifting its weight costs more"
            );
            factor_checked = true;
        }
        if a.site.y == 6 && arrive.is_none() {
            arrive = Some(t);
            assert_eq!(a.site.x, 6, "it stands on the top of the wall it climbed");
            assert!(a.mobility.climb.is_none(), "and is walking again");
        }
    }
    let (start, arrive) = (
        start.expect("it attached to the wall"),
        arrive.expect("it reached the top"),
    );
    let took = f64::from(arrive - start);
    assert!(
        (took - expected).abs() <= 2.0,
        "the 1 m climb took {took} ticks, not about {expected}"
    );
    assert!(factor_checked);
    assert_eq!(
        fauna.view().ledger.wall_ascents_by_founder[Founder::Blind.index()],
        1
    );
}

// ---------------------------------------------------------------- 2. the descent

/// Test 2: walking off a cliff deeper than its step, a shredder climbs down the face and
/// stands on the ground below instead of refusing the edge.
#[test]
fn a_shredder_climbs_down_a_cliff_instead_of_refusing() {
    let world = plateau(32, 3, 4, 0, 7);
    let mut flora = Flora::new(FloraConfig::default());
    let (mut fauna, id) = walker(&world, Founder::Blind, 4, 1, EAST, 1.0);
    assert_eq!(fauna.view().animal(id).unwrap().site.y, 6);
    let mut descended = None;
    for t in 1..=260u32 {
        fauna.step(&world, &mut flora);
        let a = fauna.view().animal(id).expect("alive");
        if a.site.y == 2 && descended.is_none() {
            descended = Some(t);
            assert!(
                a.site.x >= 8,
                "it is at the foot of the cliff, {:?}",
                a.site
            );
        }
    }
    assert!(descended.is_some(), "it never got down the cliff");
    let a = fauna.view().animal(id).unwrap();
    assert!(
        a.site.x > 8,
        "and walked on, away from the face: {:?}",
        a.site
    );
    assert_eq!(
        fauna.view().ledger.wall_descents_by_founder[Founder::Blind.index()],
        1
    );
}

// ---------------------------------------------------------------- 3. the overhang

/// Test 3: a wall whose top juts out over the foot — solid over the shredder's own column
/// below the wall's top — is not climbed: the body turns back.
#[test]
fn an_overhang_stops_the_climb_and_the_body_turns_back() {
    let mut world = plateau(48, 3, 4, 6, 10);
    for z in 0..3 {
        solid(&mut world, 5, 5, z);
    }
    let mut flora = Flora::new(FloraConfig::default());
    let (mut fauna, id) = walker(&world, Founder::Blind, 2, 1, EAST, 1.0);
    let mut turned = false;
    for _ in 0..120 {
        fauna.step(&world, &mut flora);
        let a = fauna.view().animal(id).expect("alive");
        assert!(a.mobility.climb.is_none(), "it attached under an overhang");
        assert_eq!(a.site.y, 2, "it never left the ground");
        if (a.pose.heading_rad - WEST).abs() < 1e-9 {
            turned = true;
        }
    }
    assert!(turned, "it turned back from the overhang");
    assert_eq!(
        fauna.view().ledger.wall_ascents_by_founder[Founder::Blind.index()],
        0
    );
}

// ------------------------------------------------------------------ 4. stands

/// Test 4: a stand is not terrain. A shredder walking into a full-grown vaulttree on
/// flat ground never attaches to it.
#[test]
fn a_stand_is_never_climbed() {
    let world = plateau(32, 3, 0, 0, 0);
    let mut flora = Flora::new(FloraConfig::default());
    let wood = flora.config().species(Plant::Vaulttree).wood_max;
    assert!(flora.apply(
        &world,
        FloraCommand::Seed {
            x: 6,
            z: 1,
            species: Plant::Vaulttree,
            wood,
        },
    ));
    let (mut fauna, id) = walker(&world, Founder::Blind, 2, 1, EAST, 1.0);
    for _ in 0..160 {
        fauna.step(&world, &mut flora);
        let a = fauna.view().animal(id).expect("alive");
        assert!(a.mobility.climb.is_none(), "it climbed a stand");
        assert_eq!(a.site.y, 2);
    }
    assert!(
        fauna.view().animal(id).unwrap().site.x > 6,
        "it walked on past the trunk"
    );
}

// ------------------------------------------------------------- 5. browser ledges

/// Where a browser walking east from column 8 of a 0.125 m strip ends up, against a
/// riser of `rise` voxels at column 16.
fn browser_after_riser(rise: u32) -> Site {
    let world = terrain(48, 6, 24, 0.125, |x| {
        if (16..=46).contains(&x) { 2 + rise } else { 2 }
    });
    let mut flora = Flora::new(FloraConfig::default());
    let (mut fauna, id) = walker(&world, Founder::Browser, 8, 3, EAST, 1.0);
    for _ in 0..80 {
        fauna.step(&world, &mut flora);
    }
    fauna.view().animal(id).expect("alive").site
}

/// Where a browser walking west from column 20 of a plateau `drop` voxels above the
/// ground to its west ends up.
fn browser_after_drop(drop: u32) -> Site {
    let world = terrain(48, 6, 24, 0.125, |x| {
        if (12..=30).contains(&x) { 2 + drop } else { 2 }
    });
    let mut flora = Flora::new(FloraConfig::default());
    let (mut fauna, id) = walker(&world, Founder::Browser, 20, 3, WEST, 1.0);
    for _ in 0..80 {
        fauna.step(&world, &mut flora);
    }
    fauna.view().animal(id).expect("alive").site
}

/// Test 5: a browser steps up 0.375 m (its own height) but not 0.5 m, and down the same
/// (Fable's decision: the brief's 0.75 m down made pits the heuristics starved in; a
/// longer drop comes back after the retrain). On a 0.125 m grid: 3 voxels, not 4.
#[test]
fn a_browser_steps_up_and_down_its_height() {
    let phys = *FaunaConfig::default().founder(Founder::Browser);
    assert!(!phys.climbs_walls, "a browser does not climb walls");
    let l = phys.step_limits(0.125);
    assert_eq!((l.up, l.down), (3, 3));
    let l = phys.step_limits(0.25);
    assert_eq!(
        (l.up, l.down),
        (1, 1),
        "0.375 m is one 0.25 m voxel, never two"
    );

    assert_eq!(browser_after_riser(3).y, 5, "a 0.375 m riser is a step up");
    assert_eq!(browser_after_riser(4).y, 2, "a 0.5 m riser is a wall");
    assert_eq!(browser_after_drop(3).y, 2, "a 0.375 m drop is a step down");
    assert_eq!(browser_after_drop(4).y, 6, "a 0.5 m drop is refused");
    assert_eq!(browser_after_drop(6).y, 8, "a 0.75 m drop is refused");
}

// ------------------------------------------------------------------ 6. wading

/// Test 6: the wade depth is a fraction of the **current** body height, so a juvenile
/// wades shallower water than an adult: the browser half its height, the shredder a
/// quarter.
#[test]
fn wade_depths_scale_with_the_current_body_height() {
    let c = FaunaConfig::default();
    for (founder, fraction) in [(Founder::Browser, 0.5), (Founder::Blind, 0.25)] {
        let phys = c.founder(founder);
        let adult = phys.body_at(phys.core.body_max);
        let young = phys.body_at(phys.core.body_min);
        assert!(young.height_m < adult.height_m);
        for body in [adult, young] {
            assert!(
                (phys.wade_depth_m(&body) - fraction * body.height_m).abs() < 1e-15,
                "{}: wade {} at height {}",
                founder.name(),
                phys.wade_depth_m(&body),
                body.height_m
            );
            assert!(
                (phys.drown_depth_m(&body) - body.height_m).abs() < 1e-15,
                "{}: drowns in water deeper than its own height",
                founder.name()
            );
        }
    }
    let browser = c.founder(Founder::Browser);
    assert!((browser.wade_depth_m(&browser.adult_body()) - 0.1875).abs() < 1e-15);
}

// ---------------------------------------------------------------- 7. drowning

/// Test 7: a body under water deeper than its height survives 59 s and drowns at 60 s;
/// a tick out of it resets the count.
#[test]
fn a_body_drowns_only_after_sixty_seconds_under_and_the_count_resets() {
    let dry = plateau(8, 3, 0, 0, 0);
    let mut wet = plateau(8, 3, 0, 0, 0);
    for z in 0..3 {
        for x in 0..8 {
            flood(&mut wet, x, 2, z, 0.25);
        }
    }
    let mut flora = Flora::new(FloraConfig::default());
    let (mut fauna, id) = walker(&dry, Founder::Blind, 3, 1, EAST, 0.0);
    let height = {
        let phys = fauna.config().founder(Founder::Blind);
        phys.body_at(fauna.view().animal(id).unwrap().body).height_m
    };
    assert!(0.25 > height, "the water is over its head");
    let ticks_60 = (60.0 / DT).round() as u32;

    // Continuously under: alive through 59 s, drowned by 60 s.
    let mut drowned_at = None;
    for t in 1..=ticks_60 + 5 {
        fauna.step(&wet, &mut flora);
        if fauna.view().animal(id).is_none() {
            drowned_at = Some(t);
            break;
        }
        if t == ticks_60 - 20 {
            assert!(fauna.view().animal(id).is_some(), "alive at 59 s");
        }
    }
    let t = drowned_at.expect("it drowned");
    assert!(
        t >= ticks_60 - 1 && t <= ticks_60 + 1,
        "drowned at tick {t}"
    );
    assert_eq!(
        fauna
            .view()
            .ledger
            .departed_founder(Founder::Blind, Departure::Drowned),
        1
    );

    // Under for 50 s, one dry tick, under for 50 s more: never 60 s at once.
    let (mut fauna, id) = walker(&dry, Founder::Blind, 3, 1, EAST, 0.0);
    for _ in 0..1000 {
        fauna.step(&wet, &mut flora);
    }
    assert_eq!(
        fauna.view().animal(id).unwrap().mobility.submerged_ticks,
        1000
    );
    fauna.step(&dry, &mut flora);
    assert_eq!(
        fauna.view().animal(id).unwrap().mobility.submerged_ticks,
        0,
        "reset on leaving"
    );
    for _ in 0..1000 {
        fauna.step(&wet, &mut flora);
    }
    assert!(fauna.view().animal(id).is_some(), "the count restarted");
}

// ------------------------------------------------------------------ 8. escape

/// Test 8: a browser standing in water deeper than it can wade may step to a face
/// shallower than its own, and not to a deeper one.
#[test]
fn a_body_in_deep_water_can_step_toward_shallower() {
    let mut world = plateau(12, 3, 0, 0, 0);
    for z in 0..3 {
        flood(&mut world, 3, 2, z, 0.35);
        flood(&mut world, 4, 2, z, 0.30);
        flood(&mut world, 5, 2, z, 0.25);
    }
    let phys = *FaunaConfig::default().founder(Founder::Browser);
    let wade = phys.wade_depth_m(&phys.adult_body());
    assert!(0.25 > wade, "every flooded face is deeper than it wades");

    let mut flora = Flora::new(FloraConfig::default());
    let (mut fauna, id) = walker(&world, Founder::Browser, 4, 1, EAST, 1.0);
    for _ in 0..40 {
        fauna.step(&world, &mut flora);
    }
    let a = fauna.view().animal(id).expect("alive");
    assert!(
        a.site.x >= 6,
        "it waded out toward the shallows: {:?}",
        a.site
    );

    let (mut fauna, id) = walker(&world, Founder::Browser, 4, 1, WEST, 1.0);
    for _ in 0..40 {
        fauna.step(&world, &mut flora);
    }
    assert_eq!(
        fauna.view().animal(id).expect("alive").site.x,
        4,
        "never into deeper water"
    );
}

// -------------------------------------------------------------- 9. components

/// Test 9: a shredder's walkable components join a wall's foot and its top; route
/// components join two faces only when each can reach the other — under a rule of one
/// voxel up and three down, a two-voxel riser (down, not up) separates them — and the
/// browser's own 0.375 m both ways joins a 0.25 m step and not a 0.5 m one.
#[test]
fn route_maps_join_a_climbable_wall_and_browsers_use_mutual_reachability() {
    let c = FaunaConfig::default();
    let at = |x: u32, y: u32| Site { x, y, z: 1 };

    let wall = plateau(16, 3, 4, 8, 12);
    let map = RouteMap::for_founder(&wall.view(), c.founder(Founder::Blind));
    let foot = map.component_of(at(7, 2)).expect("the foot is standable");
    let top = map.component_of(at(8, 6)).expect("the top is standable");
    assert_eq!(foot, top, "a shredder's route joins a wall's foot and top");

    let high = plateau(16, 3, 2, 8, 12);
    let browser = c.founder(Founder::Browser);
    let asymmetric = RouteMap::new(
        &high.view(),
        browser.adult_body(),
        browser.wade_depth_m(&browser.adult_body()),
        RouteRule {
            step: StepLimits { up: 1, down: 3 },
            climbs_walls: false,
        },
    );
    assert_ne!(
        asymmetric
            .component_of(at(7, 2))
            .expect("the ground is standable"),
        asymmetric
            .component_of(at(8, 4))
            .expect("the plateau is standable"),
        "a riser that can be stepped down but not up does not join"
    );
    let map = RouteMap::for_founder(&high.view(), browser);
    let (below, above) = (
        map.component_of(at(7, 2)).expect("the ground is standable"),
        map.component_of(at(8, 4))
            .expect("the plateau is standable"),
    );
    assert_ne!(
        below, above,
        "a 0.5 m riser is over the browser's step both ways"
    );
    let low = plateau(16, 3, 1, 8, 12);
    let map = RouteMap::for_founder(&low.view(), c.founder(Founder::Browser));
    assert_eq!(
        map.component_of(at(7, 2)).expect("the ground is standable"),
        map.component_of(at(8, 3)).expect("the step is standable"),
        "a 0.25 m riser is a step both ways"
    );
}
