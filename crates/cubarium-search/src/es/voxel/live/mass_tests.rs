//! The audit round's mass scale (`design/handoffs/voxel-audit-round-2026-09-28.md` §2,
//! audit §1.4): `founder.mass_scale` multiplies the nine absolute animal quantities and
//! nothing else; the heuristic parker; the opt-in mass-relative Self senses. Each at most
//! 200 ticks.

use std::sync::{Arc, Mutex};

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Actions, Command as FaunaCommand, Controller, Fauna, FaunaConfig, Founder, Response,
    StartingStores,
};
use cubarium_voxel_flora::{Flora, FloraConfig};

use super::ecology::EcologyParams;
use super::search::brain_driver;

fn flat_world() -> World {
    let mut world = World::empty(VoxelConfig {
        width: 8,
        height: 6,
        depth: 6,
        voxel_m: 0.25,
        ..VoxelConfig::default()
    });
    for z in 0..6 {
        for x in 0..8 {
            for y in 1..=2 {
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

fn with(name: &str, v: f64) -> EcologyParams {
    let mut e = EcologyParams::defaults();
    e.set(name, v).expect("in range");
    e
}

/// The nine absolute quantities, as `(name, value)`.
fn absolutes(sc: &cubarium_voxel_fauna::SpeciesConfig) -> [(&'static str, f64); 9] {
    [
        ("body_max", sc.body_max),
        ("body_min", sc.body_min),
        ("birth_body", sc.birth_body),
        ("birth_cost", sc.birth_cost),
        ("surplus_floor", sc.reproduction.surplus_floor),
        ("egg_organic", sc.reproduction.egg_organic),
        ("bite_per_s", sc.bite_per_s),
        ("bite_half_stock", sc.bite_half_stock),
        ("growth_max_per_s", sc.growth_max_per_s),
    ]
}

/// `founder.mass_scale@frondgrazer` = 100 multiplies the browser's nine absolute
/// quantities (and its placeholder species', which is the same body) by 100; per-unit
/// rates, yields, the reserve cap, the life history's clocks and the body in metres are
/// untouched, and the shredder is not moved at all. The shared key reaches both, and the
/// two multiply. The scaled config validates.
#[test]
fn mass_scale_multiplies_the_nine_absolute_quantities_and_nothing_else() {
    let base = FaunaConfig::default();
    let c = with("founder.mass_scale@frondgrazer", 100.0).fauna_config();
    c.validate().expect("a scaled world validates");
    let (b, s) = (Founder::Browser, Founder::Blind);
    for (sc, sb) in [
        (&c.founder(b).core, &base.founder(b).core),
        (&c.frondgrazer, &base.frondgrazer),
    ] {
        for ((name, v), (_, v0)) in absolutes(sc).into_iter().zip(absolutes(sb)) {
            assert!(
                (v - 100.0 * v0).abs() <= 1e-12 * v.abs().max(1.0),
                "{name}: {v} vs 100 x {v0}"
            );
        }
        assert_eq!(sc.maintenance_per_s, sb.maintenance_per_s);
        assert_eq!(sc.yield_fraction, sb.yield_fraction);
        assert_eq!(sc.n_tissue, sb.n_tissue);
        assert_eq!(sc.reserve_cap, sb.reserve_cap);
        assert_eq!(sc.energy_density, sb.energy_density);
        assert_eq!(sc.lifespan_median_s, sb.lifespan_median_s);
        assert_eq!(
            sc.reproduction.surplus_hold_s,
            sb.reproduction.surplus_hold_s
        );
        assert_eq!(sc.reproduction.gestation_s, sb.reproduction.gestation_s);
    }
    let (pb, pb0) = (c.founder(b), base.founder(b));
    assert_eq!(pb.motor_respiration_per_s, pb0.motor_respiration_per_s);
    assert_eq!(pb.yield_by_food, pb0.yield_by_food);
    assert_eq!(pb.adult_length_m, pb0.adult_length_m);
    assert_eq!(pb.adult_height_m, pb0.adult_height_m);
    assert_eq!(c.founder(s), base.founder(s), "the shredder is not moved");

    // Shared and per-lineage multiply; the shared key reaches the shredder's eggs too.
    let mut e = with("founder.mass_scale", 3.0);
    e.set("founder.mass_scale@frondgrazer", 10.0)
        .expect("in range");
    let c = e.fauna_config();
    c.validate().expect("validates");
    assert!((c.founder(b).core.body_max - 30.0 * base.founder(b).core.body_max).abs() < 1e-12);
    let (egg, egg0) = (
        c.founder(s).core.reproduction.egg_organic,
        base.founder(s).core.reproduction.egg_organic,
    );
    assert!((egg - 3.0 * egg0).abs() < 1e-12, "{egg} vs 3 x {egg0}");
}

/// `mass_scale` is a per-lineage search dimension on a log range of [1, 1000], and the
/// shipped default (1) sits at the bottom of it.
#[test]
fn mass_scale_is_a_per_lineage_search_dimension_on_1_to_1000() {
    let names = EcologyParams::search_names();
    let bounds = EcologyParams::search_bounds();
    for f in Founder::ALL {
        let n = format!("founder.mass_scale@{}", f.name());
        let i = names
            .iter()
            .position(|x| *x == n)
            .unwrap_or_else(|| panic!("{n} is searched"));
        assert!(bounds[i].0.abs() < 1e-12, "{n}: low end ln 1");
        assert!((bounds[i].1 - 1000f64.ln()).abs() < 1e-12, "{n}: high end ln 1000");
    }
    assert!(!names.iter().any(|n| n == "founder.mass_scale"));
    assert_eq!(
        EcologyParams::defaults().get("founder.mass_scale@frondgrazer"),
        Some(1.0)
    );
    assert!(EcologyParams::defaults()
        .set("founder.mass_scale", 0.5)
        .is_err());
}

/// A reserve fraction after `ticks` at rest with no food.
fn resting_reserve_fraction(e: &EcologyParams, founder: Founder, ticks: u32) -> f64 {
    let world = flat_world();
    let mut flora = Flora::new(e.flora_config(FloraConfig::for_voxel_size(0.25)));
    let mut fauna = Fauna::new(e.fauna_config());
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 3,
            z: 3,
            founder,
            stores: StartingStores::FULL,
            heading_rad: 0.0,
        }
    ));
    let id = fauna.view().animals[0].id;
    let rest: Box<dyn Controller> = Box::new(crate::es::voxel::controller::NoIntake);
    assert!(fauna.set_controller(id, rest));
    for _ in 0..ticks {
        fauna.step(&world, &mut flora);
    }
    let a = &fauna.view().animals[0];
    let sc = e.fauna_config().founder(founder).core;
    a.reserve / sc.reserve_of(sc.body_max)
}

/// Per-unit timescales do not move: a body 100x heavier burns the same share of its full
/// reserve per second at rest, so it starves on the same clock.
#[test]
fn a_heavier_body_starves_on_the_same_clock() {
    for f in Founder::ALL {
        let one = resting_reserve_fraction(&EcologyParams::defaults(), f, 200);
        let heavy = resting_reserve_fraction(&with("founder.mass_scale", 100.0), f, 200);
        assert!(one < 1.0 && one > 0.5, "{f:?}: it burned some reserve: {one}");
        assert!(
            (one - heavy).abs() < 1e-9,
            "{f:?}: x1 keeps {one} of its reserve, x100 {heavy}"
        );
    }
}

/// An observation with nothing in it but a valid taste reading of `cue`; the motor
/// delivery slot reads 1 ("everything asked for was delivered").
fn tasting(founder: Founder, cue: f64) -> Vec<f64> {
    let m = founder.manifest();
    let mut o = vec![0.0; m.inputs()];
    o[7] = 1.0;
    let t = m
        .modules
        .iter()
        .find(|m| m.name == "Taste(1)")
        .expect("a taste module");
    o[t.offset] = cue;
    o[t.offset + 2] = 1.0;
    o
}

fn act(c: &mut Box<dyn Controller>, o: &[f64]) -> Actions {
    match c.drive(o) {
        Response::Bounded(a) => a,
        Response::Logits(_) => unreachable!("a heuristic answers bounded"),
    }
}

/// The heuristic parker is the heuristic walker except that it stands still while its
/// mouth tastes food above the walker's own feed gate: on food it feeds and does not move;
/// off food it walks exactly as the walker does.
#[test]
fn the_parker_stands_on_food_and_walks_like_the_heuristic_off_it() {
    for f in Founder::ALL {
        let mut parker = brain_driver(f, "parker").expect("a parker").fresh();
        let mut walker = brain_driver(f, "heuristic").expect("a walker").fresh();
        let on = tasting(f, 0.45);
        let (p, w) = (act(&mut parker, &on), act(&mut walker, &on));
        assert_eq!(p.feed, 1.0, "{f:?}: the parker feeds on food");
        assert_eq!(w.feed, 1.0, "{f:?}: so does the walker");
        assert!(w.forward > 0.5, "{f:?}: the walker eats on the move");
        assert_eq!((p.forward, p.turn), (0.0, 0.0), "{f:?}: the parker stands");

        let mut parker = brain_driver(f, "parker").expect("a parker").fresh();
        let mut walker = brain_driver(f, "heuristic").expect("a walker").fresh();
        let off = tasting(f, 0.0);
        for k in 0..20 {
            let (p, w) = (act(&mut parker, &off), act(&mut walker, &off));
            assert_eq!(
                (p.forward, p.turn, p.feed),
                (w.forward, w.turn, w.feed),
                "{f:?} sample {k}: off food the parker walks as the walker does"
            );
        }
        assert!(brain_driver(f, "parker").expect("named").name().contains("parker"));
    }
}

#[derive(Clone)]
struct Recorder {
    log: Arc<Mutex<Vec<Vec<f64>>>>,
}

impl Controller for Recorder {
    fn drive(&mut self, observation: &[f64]) -> Response {
        self.log.lock().unwrap().push(observation.to_vec());
        Response::Bounded(Actions {
            forward: 0.0,
            turn: 0.0,
            feed: 0.0,
        })
    }
    fn reset(&mut self) {}
}

/// The first `Self` energy and reserve readings of an adult browser introduced with 40 %
/// of its full reserve.
fn self_readings(e: &EcologyParams) -> (f64, f64) {
    let world = flat_world();
    let mut flora = Flora::new(e.flora_config(FloraConfig::for_voxel_size(0.25)));
    let mut fauna = Fauna::new(e.fauna_config());
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 3,
            z: 3,
            founder: Founder::Browser,
            stores: StartingStores {
                body: 1.0,
                reserve: 0.4,
            },
            heading_rad: 0.0,
        }
    ));
    let id = fauna.view().animals[0].id;
    let log = Arc::new(Mutex::new(Vec::new()));
    assert!(fauna.set_controller(
        id,
        Box::new(Recorder {
            log: Arc::clone(&log),
        })
    ));
    for _ in 0..40 {
        fauna.step(&world, &mut flora);
        if !log.lock().unwrap().is_empty() {
            break;
        }
    }
    let log = log.lock().unwrap();
    let o = log.first().expect("the controller was asked within 40 ticks");
    (o[0], o[1])
}

/// Under the contract as it stands (A) the `Self` references are the manifest's fixed
/// absolutes, so a x100 body reads full whatever its reserve; with the opt-in
/// mass-relative senses (B) it reads the same fraction of an adult as a x1 body.
#[test]
fn mass_relative_self_senses_read_a_fraction_of_the_adult() {
    let (energy1, reserve1) = self_readings(&EcologyParams::defaults());
    assert!(
        (reserve1 - 0.4).abs() < 0.02,
        "x1 reads 40 % of an adult reserve: {reserve1}"
    );
    assert!(energy1 > 0.5 && energy1 < 0.95, "x1 energy {energy1}");

    let heavy = with("founder.mass_scale@frondgrazer", 100.0);
    let (energy_a, reserve_a) = self_readings(&heavy);
    assert_eq!((energy_a, reserve_a), (1.0, 1.0), "A: a x100 body reads full");

    let (energy_b, reserve_b) = self_readings(&heavy.clone().with_mass_relative_self());
    assert!(
        (reserve_b - reserve1).abs() < 1e-6 && (energy_b - energy1).abs() < 1e-6,
        "B: x100 reads ({energy_b}, {reserve_b}) as x1 reads ({energy1}, {reserve1})"
    );
    // B is opt-in: the defaults and a plain scaled vector keep the contract's references.
    assert_eq!(
        EcologyParams::defaults()
            .with_mass_relative_self()
            .fauna_config(),
        FaunaConfig::default(),
        "at x1 the mass-relative references are the manifest's"
    );
}
