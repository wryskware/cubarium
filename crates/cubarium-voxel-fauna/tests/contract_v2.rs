//! **Contract v2** (package 5, P5-A; `design/handoffs/voxel-retrain-2026-09-22.md`,
//! decisions D1–D4 and D12).
//!
//! Written before the implementation, against the brief's list: 45 rays at the five
//! pitches; a ray over shallow water passes above its surface and is stopped below it;
//! a ground pool stops a ray only below its physical height; a ray and a contact probe
//! past the strip's `z` edge read solid; `birth_readiness` is 0 during the refractory
//! and while the surplus is still being held; the observation's cone (built from the
//! controller stage's windowed occupancy) equals a full rebuild on a seeded world; the
//! manifest's canonical text moves when any physical anchor moves; and the heuristic
//! teachers back off when the motor is refused or the forward rays read a drop.
//!
//! Public surface only, a handful of ticks each, no pinned hashes.

use std::sync::{Arc, Mutex};

use cubarium_voxel::{Command as WorldCommand, Config, Material, World};
use cubarium_voxel_fauna::{
    Actions, BlindForager, BrowserForager, Command, ConeHit, Controller, Fauna, FaunaConfig,
    Founder, FounderPhysiology, Manifest, Response, SightMap, StartingStores, birth_readiness,
    browser_cone_observation,
};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, Site, Species as Plant,
};

const GROUND_Y: u32 = 2;
const V: f64 = 0.25;
const EAST: f64 = std::f64::consts::FRAC_PI_2;

fn flat(width: u32, depth: u32) -> World {
    let mut world = World::empty(Config {
        width,
        height: 10,
        depth,
        voxel_m: V,
        seed: 1,
        ..Config::default()
    });
    for z in 0..depth {
        for x in 0..width as i64 {
            for y in 1..=GROUND_Y {
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

/// The height of the standing surface, metres.
fn surface() -> f64 {
    f64::from(GROUND_Y + 1) * V
}

fn empty_fauna() -> Fauna {
    Fauna::new(FaunaConfig::default())
}

// ------------------------------------------------------------------ the fan

/// D1 / decisions §6: the fan is five pitches, −40..+40, over the same three sectors and
/// three yaw offsets — 45 rays — and the observation keeps its 23 and 37 inputs.
#[test]
fn the_browser_fan_has_forty_five_rays_at_the_five_pitches() {
    let browser = Founder::Browser.manifest();
    assert_eq!(
        browser.ray_pitch_offsets_deg,
        &[-40.0, -20.0, 0.0, 20.0, 40.0],
        "the decided pitch set"
    );
    assert_eq!(browser.ray_count(), 45, "3 sectors x 3 yaws x 5 pitches");
    assert_eq!(browser.inputs(), 37, "the channel count does not move");
    assert_eq!(Founder::Blind.manifest().inputs(), 23);
    assert_eq!(cubarium_voxel_fauna::SCHEMA_VERSION, 2, "contract v2");
}

// --------------------------------------------------------------- occlusion (D3)

/// D3: water occludes a ray only below its free surface — `fill × cell height` over the
/// cell's floor. A half-full cell stops a ray a quarter-cell over the floor and lets one
/// three quarters up pass.
#[test]
fn a_ray_over_shallow_water_passes_above_the_surface_and_is_stopped_below_it() {
    let mut world = flat(16, 4);
    // Half a cell of water on the face two columns east of the eye.
    let accepted = world.apply(WorldCommand::AddWater {
        x: 4,
        y: GROUND_Y + 1,
        z: 1,
        volume_m3: 0.5 * V * V * V,
    });
    assert!(accepted > 0.0, "the fixture's water was refused");
    let flora = Flora::new(FloraConfig::default());
    let fauna = empty_fauna();
    let (v, fv, av) = (world.view(), flora.view(), fauna.view());
    let map = SightMap::new(&v, &fv, &av, true);
    let east = (1.0, 0.0, 0.0);
    let z = 1.5 * V;
    let x = 2.5 * V;

    let below = map.first_hit(&v, u64::MAX, (x, surface() + 0.25 * V, z), east, 1.0);
    assert!(
        matches!(below, Some((_, ConeHit::Water, _))),
        "a ray under the surface is stopped by the water: {below:?}"
    );
    let above = map.first_hit(&v, u64::MAX, (x, surface() + 0.75 * V, z), east, 1.0);
    assert!(
        above.is_none(),
        "a ray over the surface of a half-full cell passes: {above:?}"
    );
}

/// D3: a litter pool occludes below its physical height, organic / (cell area × bulk
/// density), and not the whole cell over its face.
#[test]
fn a_ground_pool_stops_a_ray_only_below_its_physical_height() {
    let world = flat(16, 4);
    let density = Founder::Browser.manifest().pool_bulk_density;
    assert!(density > 0.0 && density.is_finite());
    // A pool exactly half a cell tall.
    let organic = 0.5 * V * V * V * density;
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.deposit(
        Site {
            x: 4,
            y: GROUND_Y,
            z: 1
        },
        Deposit {
            kind: DepositKind::Litter,
            organic,
            mineral: 0.02 * organic,
            energy: 2.0 * organic,
        },
    ));
    let fauna = empty_fauna();
    let (v, fv, av) = (world.view(), flora.view(), fauna.view());
    let map = SightMap::new(&v, &fv, &av, true);
    let east = (1.0, 0.0, 0.0);
    let (x, z) = (2.5 * V, 1.5 * V);
    let low = map.first_hit(&v, u64::MAX, (x, surface() + 0.25 * V, z), east, 1.0);
    assert!(
        matches!(low, Some((_, ConeHit::GroundPool, _))),
        "a ray through the pool is stopped: {low:?}"
    );
    let high = map.first_hit(&v, u64::MAX, (x, surface() + 0.75 * V, z), east, 1.0);
    assert!(high.is_none(), "a ray over the pool passes: {high:?}");
}

// --------------------------------------------------------------- the world edge (D4)

/// D4: a ray leaving the strip's `z` range reads solid, not open.
#[test]
fn a_ray_past_the_z_edge_reads_solid() {
    let world = flat(16, 4);
    let flora = Flora::new(FloraConfig::default());
    let fauna = empty_fauna();
    let (v, fv, av) = (world.view(), flora.view(), fauna.view());
    let map = SightMap::new(&v, &fv, &av, true);
    // From the middle of row 0, looking straight at the front edge (−z), and from the
    // last row looking at the back edge (+z).
    let eye_y = surface() + 0.5 * V;
    let front = map.first_hit(
        &v,
        u64::MAX,
        (4.5 * V, eye_y, 0.5 * V),
        (0.0, 0.0, -1.0),
        2.0,
    );
    let back = map.first_hit(
        &v,
        u64::MAX,
        (4.5 * V, eye_y, 3.5 * V),
        (0.0, 0.0, 1.0),
        2.0,
    );
    for (name, hit) in [("front", front), ("back", back)] {
        let (d, class, _) = hit.unwrap_or_else(|| panic!("the {name} edge read open"));
        assert_eq!(class, ConeHit::Terrain, "the {name} edge reads solid");
        assert!(
            d <= 0.5 * V + 1e-9 + 0.25 * V,
            "at the edge, not beyond: {d}"
        );
    }
}

#[derive(Clone)]
struct Recorder {
    log: Arc<Mutex<Vec<Vec<f64>>>>,
    action: Actions,
}

impl Controller for Recorder {
    fn drive(&mut self, observation: &[f64]) -> Response {
        self.log.lock().unwrap().push(observation.to_vec());
        Response::Bounded(self.action)
    }
    fn reset(&mut self) {}
}

fn record(fauna: &mut Fauna, id: u64, action: Actions) -> Arc<Mutex<Vec<Vec<f64>>>> {
    let log = Arc::new(Mutex::new(Vec::new()));
    assert!(fauna.set_controller(
        id,
        Box::new(Recorder {
            log: Arc::clone(&log),
            action,
        })
    ));
    log
}

/// D4, contact: a body pressed against the strip's front edge feels it on its front
/// receptor, exactly as it would a wall.
#[test]
fn a_contact_probe_past_the_z_edge_reads_solid() {
    for founder in Founder::ALL {
        let world = flat(16, 4);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = empty_fauna();
        fauna.set_births_enabled(false);
        let id = fauna.view().ledger.births;
        // Heading π faces −z: straight at the front edge from row 0.
        assert!(fauna.apply(
            &world,
            Command::IntroduceFounder {
                x: 4,
                z: 0,
                founder,
                stores: StartingStores::FULL,
                heading_rad: std::f64::consts::PI,
            },
        ));
        let log = record(
            &mut fauna,
            id,
            Actions {
                forward: 1.0,
                turn: 0.0,
                feed: 0.0,
            },
        );
        for _ in 0..30 {
            fauna.step_with(&world, &mut flora, 1);
        }
        let front = founder.manifest().modules[1].offset; // Contact(4): front first
        assert_eq!(founder.manifest().modules[1].name, "Contact(4)");
        let last = log.lock().unwrap().last().cloned().expect("sampled");
        assert_eq!(
            last[front], 1.0,
            "{founder:?}: pressed against the front edge, the front receptor reads solid"
        );
    }
}

// --------------------------------------------------------------- birth readiness

/// The birth step's own rules: readiness is 1 only when this tick's reproduction step
/// would act — not during the refractory, not while the surplus is still being held,
/// not while gestating, and for an egg-layer only on a face holding litter.
#[test]
fn birth_readiness_is_zero_during_the_refractory_and_while_surplus_is_held() {
    for founder in Founder::ALL {
        let world = flat(8, 4);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = empty_fauna();
        assert!(fauna.apply(
            &world,
            Command::IntroduceFounder {
                x: 2,
                z: 1,
                founder,
                stores: StartingStores::FULL,
                heading_rad: 0.0,
            },
        ));
        // Litter under it, so an egg-layer's face rule is satisfied.
        assert!(flora.deposit(
            Site {
                x: 2,
                y: GROUND_Y,
                z: 1
            },
            Deposit {
                kind: DepositKind::Litter,
                organic: 0.05,
                mineral: 0.001,
                energy: 0.1,
            },
        ));
        let config = *fauna.config();
        let adult = fauna.view().animals[0];
        let rule = config.founder(founder).core.reproduction;
        let hold = rule.hold_ticks();
        let fv = flora.view();

        // A full adult in surplus, but the hold has not elapsed.
        let mut a = adult;
        a.reproduction.surplus_ticks = 0;
        assert!(
            !birth_readiness(&config, &a, &fv),
            "{founder:?}: hold not elapsed"
        );
        a.reproduction.surplus_ticks = hold / 2;
        assert!(
            !birth_readiness(&config, &a, &fv),
            "{founder:?}: still holding"
        );
        // The hold elapses on this tick: the step would act.
        a.reproduction.surplus_ticks = hold - 1;
        assert!(
            birth_readiness(&config, &a, &fv),
            "{founder:?}: the step acts now"
        );
        // Refractory: nothing advances.
        a.reproduction.refractory_ticks = 10;
        assert!(
            !birth_readiness(&config, &a, &fv),
            "{founder:?}: refractory"
        );
        a.reproduction.refractory_ticks = 0;
        // An empty reserve is not a surplus.
        let mut hungry = a;
        hungry.reserve = 0.0;
        assert!(
            !birth_readiness(&config, &hungry, &fv),
            "{founder:?}: no surplus"
        );
        // The egg rule: the same body on bare ground cannot lay.
        if founder == Founder::Blind {
            let bare = Flora::new(FloraConfig::default());
            assert!(
                !birth_readiness(&config, &a, &bare.view()),
                "an egg-layer on bare ground cannot lay"
            );
        }
    }
}

/// And the observation's `Self.birth_readiness` slot is that rule: a full founder at its
/// first sampling has held no surplus yet, so it reads 0 (the old rule read 1).
#[test]
fn the_observed_birth_readiness_is_the_step_rule() {
    for founder in Founder::ALL {
        let world = flat(8, 4);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = empty_fauna();
        let id = fauna.view().ledger.births;
        assert!(fauna.apply(
            &world,
            Command::IntroduceFounder {
                x: 2,
                z: 1,
                founder,
                stores: StartingStores::FULL,
                heading_rad: 0.0,
            },
        ));
        let log = record(&mut fauna, id, Actions::REST);
        // Ages advance before the controller stage, so the first sampling is at age 5.
        for _ in 0..founder.manifest().cadence_ticks() {
            fauna.step_with(&world, &mut flora, 1);
        }
        let first = log.lock().unwrap()[0].clone();
        assert_eq!(first[2], 0.0, "{founder:?}: no surplus has been held yet");
    }
}

// --------------------------------------------------------------- windowed occupancy

/// The controller stage builds its occupancy for a window around the due observers.
/// The cone it hands each browser equals the one a full rebuild gives, on a world with
/// stands on both sides of every window's edge and bodies inside and outside it.
#[test]
fn the_windowed_occupancy_reads_what_the_full_build_reads() {
    let world = flat(64, 8);
    let mut flora = Flora::new(FloraConfig::default());
    for (i, x) in [3, 6, 9, 11, 13, 17, 20, 25, 30, 38, 42, 47, 51, 58, 62]
        .into_iter()
        .enumerate()
    {
        let species = [Plant::Springturf, Plant::Bloomcrown, Plant::Umbrellafrond][i % 3];
        let wood = 0.5 * flora.config().species(species).wood_max;
        flora.apply(
            &world,
            FloraCommand::Seed {
                x,
                z: (i as u32 * 3) % 8,
                species,
                wood,
            },
        );
    }
    assert!(
        flora.view().stands.len() >= 10,
        "the fixture seeded its stands"
    );
    let mut fauna = empty_fauna();
    fauna.set_births_enabled(false);
    let mut logs = Vec::new();
    // Browsers far apart (so each window excludes most of the ring) plus shredders
    // everywhere, which are bodies in the occupancy but never observers.
    for (x, z, founder, heading) in [
        (8, 3, Founder::Browser, EAST),
        (40, 5, Founder::Browser, -EAST),
        (12, 1, Founder::Blind, 0.0),
        (20, 6, Founder::Blind, 0.0),
        (44, 2, Founder::Blind, 0.0),
        (60, 4, Founder::Browser, 0.3),
    ] {
        let id = fauna.view().ledger.births;
        assert!(fauna.apply(
            &world,
            Command::IntroduceFounder {
                x,
                z,
                founder,
                stores: StartingStores::FULL,
                heading_rad: heading,
            },
        ));
        if founder == Founder::Browser {
            logs.push((id, record(&mut fauna, id, Actions::REST)));
        }
    }
    // To the first sampling (ages advance before the controller stage, so it is at age
    // 5): every founder rests, and maintenance runs before the stage, so the state after
    // that tick is the state the controller stage observed.
    for _ in 0..Founder::Browser.manifest().cadence_ticks() {
        fauna.step_with(&world, &mut flora, 1);
    }
    let cone = Founder::Browser
        .manifest()
        .modules
        .iter()
        .find(|m| m.name == "Cone(3, foliage/body)")
        .copied()
        .unwrap();
    let (v, fv, av) = (world.view(), flora.view(), fauna.view());
    let mut saw_something = false;
    for (id, log) in logs {
        let obs = log.lock().unwrap()[0].clone();
        let animal = av.animals.iter().find(|a| a.id == id).copied().unwrap();
        let full = browser_cone_observation(&v, &fv, &av, &animal).expect("a browser");
        let windowed = &obs[cone.offset..cone.offset + cone.width];
        assert_eq!(
            windowed,
            &full[..],
            "browser {id}: windowed != full rebuild"
        );
        saw_something |= windowed.iter().step_by(6).any(|&clear| clear < 1.0);
    }
    assert!(saw_something, "the fixture's cones saw nothing at all");
}

// --------------------------------------------------------------- the manifest (D1, D2)

/// D1: the contract carries the body it was trained on; D2: cruise is one body length
/// per second. The anchors are the frozen physiology's, not a second copy.
#[test]
fn the_manifest_carries_the_physiologys_anchors_and_cruises_at_one_body_length() {
    for founder in Founder::ALL {
        let m = founder.manifest();
        let p = FounderPhysiology::frozen(founder);
        assert_eq!(m.body_length_m, p.adult_length_m, "{founder:?} length");
        assert_eq!(m.body_width_m, p.adult_width_m, "{founder:?} width");
        assert_eq!(m.body_height_m, p.adult_height_m, "{founder:?} height");
        assert_eq!(m.eye_height_fraction, p.eye_height_fraction);
        assert_eq!(m.mouth_ceiling_fraction, p.mouth_ceiling_fraction);
        assert_eq!(m.mouth_reach_body_lengths, p.mouth_reach_length_fraction);
        assert_eq!(m.contact_height_fraction, p.contact_height_fraction);
        assert_eq!(m.step_up_m, p.step_up_m);
        assert_eq!(m.step_down_m, p.step_down_m);
        assert_eq!(m.climbs_walls, p.climbs_walls);
        assert_eq!(m.wade_height_fraction, p.wade_height_fraction);
        assert_eq!(m.drown_height_fraction, p.drown_height_fraction);
        assert_eq!(m.drown_after_s, p.drown_after_s);
        assert_eq!(m.cruise_m_per_s, p.adult_length_m, "{founder:?}: 1 BL/s");
        assert!(
            (m.forward_reference_m - m.cruise_m_per_s * m.controller_period_s).abs() < 1e-15,
            "{founder:?}: the forward reference follows the cruise"
        );
    }
    // D2 is one body length per second, whatever the ladder's lengths are (package L:
    // the 0.75 m browser cruises 0.75 m/s, the 0.375 m shredder 0.375 m/s). The speeds
    // are derived above from the physiology rather than pinned here, so a ladder move
    // cannot leave a stale speed behind; the rate itself is the decision.
    assert_eq!(cubarium_voxel_fauna::CRUISE_BODY_LENGTHS_PER_S, 1.0);
    let blind = Founder::Blind.manifest();
    assert!(blind.modules.iter().any(|m| m.name == "Chem(detritus)"));
    assert!(!blind.canonical_text().contains("Chem(litter)"));
}

/// D1: every physical anchor is in the canonical text — changing any one of them moves
/// the digest, so a centre trained on one body cannot silently drive another.
#[test]
fn the_canonical_text_changes_when_any_anchor_changes() {
    for base in [Manifest::blind(), Manifest::browser()] {
        let text = base.canonical_text();
        let mut changed: Vec<(&str, Manifest)> = Vec::new();
        let mut m = base;
        m.body_length_m *= 1.1;
        changed.push(("length", m));
        let mut m = base;
        m.body_height_m *= 1.1;
        changed.push(("height", m));
        let mut m = base;
        m.eye_height_fraction = 0.7;
        changed.push(("eye", m));
        let mut m = base;
        m.mouth_ceiling_fraction = 1.2;
        changed.push(("mouth ceiling", m));
        let mut m = base;
        m.mouth_reach_body_lengths = 0.3;
        changed.push(("mouth reach", m));
        let mut m = base;
        m.contact_height_fraction = 0.4;
        changed.push(("contact", m));
        let mut m = base;
        m.step_up_m *= 2.0;
        changed.push(("step up", m));
        let mut m = base;
        m.step_down_m *= 2.0;
        changed.push(("step down", m));
        let mut m = base;
        m.climbs_walls = !m.climbs_walls;
        changed.push(("walls", m));
        let mut m = base;
        m.wade_height_fraction = 0.3;
        changed.push(("wade", m));
        let mut m = base;
        m.drown_height_fraction = 0.9;
        changed.push(("drown", m));
        let mut m = base;
        m.drown_after_s = 30.0;
        changed.push(("drown time", m));
        let mut m = base;
        m.cruise_m_per_s *= 1.1;
        changed.push(("cruise", m));
        let mut m = base;
        m.pool_bulk_density *= 2.0;
        changed.push(("pool density", m));
        let mut m = base;
        m.occlusion_rule = "something else";
        changed.push(("occlusion", m));
        let mut m = base;
        m.birth_readiness_rule = "something else";
        changed.push(("birth readiness", m));
        if !base.ray_pitch_offsets_deg.is_empty() {
            let mut m = base;
            m.ray_pitch_offsets_deg = &[-20.0, 0.0, 20.0];
            changed.push(("pitches", m));
        }
        for (name, m) in changed {
            assert_ne!(
                m.canonical_text(),
                text,
                "{:?}: a {name} change left the canonical text alone",
                base.founder
            );
            assert_ne!(m.digest(), base.digest());
        }
    }
}

// --------------------------------------------------------------- the teachers (item 9)

/// An observation with nothing in it but what the test sets: every module valid, the
/// cone (for a browser) reading flat open ground — the two downward pitch rows hit the
/// floor close by, everything else is clear.
fn quiet_observation(founder: Founder) -> Vec<f64> {
    let m = founder.manifest();
    let mut o = vec![0.0; m.inputs()];
    o[7] = 1.0; // motor_delivery: everything asked for was delivered
    for module in m.modules {
        if module.name.starts_with("Cone") {
            for k in 0..3 {
                let s = module.offset + k * 6;
                o[s] = 0.6; // 9 of 15 rays clear
                o[s + 1] = 0.85; // the floor, close
            }
        }
        if let Some(valid) = module.channels.iter().position(|c| *c == "valid") {
            o[module.offset + valid] = 1.0;
        }
        // Standing on ground: the underside receptor feels the support face (a body with
        // nothing under it is on a terrain face since package mobility).
        if module.name == "Contact(4)" {
            o[module.offset + 3] = 1.0;
        }
    }
    o
}

fn act(c: &mut dyn Controller, o: &[f64]) -> Actions {
    match c.drive(o) {
        Response::Bounded(a) => a,
        other => panic!("a heuristic answers bounded: {other:?}"),
    }
}

/// Item 9: a refused motor (`motor_delivery` low) makes either teacher stop pushing and
/// turn, from the observation alone.
#[test]
fn the_teachers_back_off_when_the_motor_is_refused() {
    let mut controllers: [(Founder, Box<dyn Controller>); 2] = [
        (Founder::Blind, Box::new(BlindForager::new())),
        (Founder::Browser, Box::new(BrowserForager::new())),
    ];
    for (founder, c) in controllers.iter_mut() {
        let quiet = quiet_observation(*founder);
        let free = act(c.as_mut(), &quiet);
        assert!(free.forward > 0.5, "{founder:?}: open ground, it walks");
        c.reset();
        let mut blocked = quiet.clone();
        blocked[7] = 0.1;
        let a = act(c.as_mut(), &blocked);
        assert!(
            a.forward < 0.5,
            "{founder:?}: refused, it stops pushing: {a:?}"
        );
        assert!(a.turn.abs() > 0.5, "{founder:?}: and turns: {a:?}");
    }
}

/// Item 9: the browser's forward sector reading a drop — its downward rays all clear or
/// all far — turns it away before the step refuses it.
#[test]
fn the_browser_teacher_turns_from_a_drop_ahead() {
    let mut c = BrowserForager::new();
    let cone = Founder::Browser
        .manifest()
        .modules
        .iter()
        .find(|m| m.name.starts_with("Cone"))
        .copied()
        .unwrap();
    let centre = cone.offset + 6;
    let mut drop = quiet_observation(Founder::Browser);
    drop[centre + 1] = 0.3; // the only hits in front are far below
    let a = act(&mut c, &drop);
    assert!(a.forward < 0.5 && a.turn.abs() > 0.5, "a drop ahead: {a:?}");
    c.reset();
    let mut void = quiet_observation(Founder::Browser);
    void[centre] = 1.0; // every forward ray escapes
    void[centre + 1] = 0.0;
    let a = act(&mut c, &void);
    assert!(
        a.forward < 0.5 && a.turn.abs() > 0.5,
        "nothing below ahead: {a:?}"
    );
}
