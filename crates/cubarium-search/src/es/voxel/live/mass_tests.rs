//! The audit round's mass scale (`design/handoffs/voxel-audit-round-2026-09-28.md` §2,
//! audit §1.4): `founder.mass_scale` multiplies the eight absolute animal quantities and
//! nothing else; the heuristic parker; the opt-in mass-relative Self senses. Each at most
//! 200 ticks.

use std::sync::{Arc, Mutex};

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Actions, Command as FaunaCommand, Controller, Fauna, FaunaConfig, Founder, Response,
    StartingStores,
};
use cubarium_voxel_flora::Flora;

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
    let mut e = EcologyParams::legacy();
    e.set(name, v).expect("in range");
    e
}

/// The eight absolute quantities a body's mass carries, as `(name, value)`. The bite's
/// half-saturation `bite_half_stock` is in food-stock units, not the body's, and is not
/// one of them (units correction, 2026-10-01).
fn absolutes(sc: &cubarium_voxel_fauna::SpeciesConfig) -> [(&'static str, f64); 8] {
    [
        ("body_max", sc.body_max),
        ("body_min", sc.body_min),
        ("birth_body", sc.birth_body),
        ("birth_cost", sc.birth_cost),
        ("surplus_floor", sc.reproduction.surplus_floor),
        ("egg_organic", sc.reproduction.egg_organic),
        ("bite_per_s", sc.bite_per_s),
        ("growth_max_per_s", sc.growth_max_per_s),
    ]
}

/// `founder.mass_scale@frondgrazer` = 100 multiplies the browser's eight absolute
/// quantities (and its placeholder species', which is the same body) by 100; per-unit
/// rates, yields, the reserve cap, the life history's clocks and the body in metres are
/// untouched, and the shredder is not moved at all. The shared key reaches both, and the
/// two multiply. The scaled config validates.
#[test]
fn mass_scale_multiplies_the_eight_absolute_quantities_and_nothing_else() {
    let base = FaunaConfig::default();
    let c = with("founder.mass_scale@frondgrazer", 100.0).fauna();
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
        assert_eq!(sc.bite_half_stock, sb.bite_half_stock, "K is in food units");
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
    assert_eq!(
        pb.mass_scale,
        100.0 * pb0.mass_scale,
        "the physiology records its scale"
    );
    assert_eq!(
        base.founder(b).mass_scale,
        cubarium_voxel_fauna::SHIPPED_MASS_SCALE
    );

    // Shared and per-lineage multiply; the shared key reaches the shredder's eggs too.
    let mut e = with("founder.mass_scale", 3.0);
    e.set("founder.mass_scale@frondgrazer", 10.0)
        .expect("in range");
    let c = e.fauna();
    c.validate().expect("validates");
    assert!((c.founder(b).core.body_max - 30.0 * base.founder(b).core.body_max).abs() < 1e-12);
    let (egg, egg0) = (
        c.founder(s).core.reproduction.egg_organic,
        base.founder(s).core.reproduction.egg_organic,
    );
    assert!((egg - 3.0 * egg0).abs() < 1e-12, "{egg} vs 3 x {egg0}");
}

/// A reserve fraction after `ticks` at rest with no food.
fn resting_reserve_fraction(e: &EcologyParams, founder: Founder, ticks: u32) -> f64 {
    let world = flat_world();
    let mut flora = Flora::new(e.flora(0.25));
    // The minimal gut: an empty founding stomach (`s0` 0). A full reserve with `s0 K`
    // in the stomach refills from digestion, so the reserve would not fall at all.
    let mut config = e.fauna();
    for p in config.founders.iter_mut() {
        p.gut.s0 = 0.0;
    }
    let mut fauna = Fauna::new(config);
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
    let sc = e.fauna().founder(founder).core;
    a.reserve / sc.reserve_of(sc.body_max)
}

/// Per-unit timescales do not move: a body 100x heavier burns the same share of its full
/// reserve per second at rest, so it starves on the same clock.
#[test]
fn a_heavier_body_starves_on_the_same_clock() {
    for f in Founder::ALL {
        let one = resting_reserve_fraction(&EcologyParams::defaults(), f, 200);
        let heavy = resting_reserve_fraction(&with("founder.mass_scale", 100.0), f, 200);
        assert!(
            one < 1.0 && one > 0.5,
            "{f:?}: it burned some reserve: {one}"
        );
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
    // The two grazing foragers; the lanternjaw's ambush does not eat on the move.
    for f in [Founder::Blind, Founder::Browser] {
        let mut parker = brain_driver(f, "parker")
            .expect("a parker")
            .fresh(cubarium_voxel_fauna::TeacherConfig::default());
        let mut walker = brain_driver(f, "heuristic")
            .expect("a walker")
            .fresh(cubarium_voxel_fauna::TeacherConfig::default());
        let on = tasting(f, 0.45);
        let (p, w) = (act(&mut parker, &on), act(&mut walker, &on));
        assert_eq!(p.feed, 1.0, "{f:?}: the parker feeds on food");
        assert_eq!(w.feed, 1.0, "{f:?}: so does the walker");
        assert!(w.forward > 0.5, "{f:?}: the walker eats on the move");
        assert_eq!((p.forward, p.turn), (0.0, 0.0), "{f:?}: the parker stands");

        let mut parker = brain_driver(f, "parker")
            .expect("a parker")
            .fresh(cubarium_voxel_fauna::TeacherConfig::default());
        let mut walker = brain_driver(f, "heuristic")
            .expect("a walker")
            .fresh(cubarium_voxel_fauna::TeacherConfig::default());
        let off = tasting(f, 0.0);
        for k in 0..20 {
            let (p, w) = (act(&mut parker, &off), act(&mut walker, &off));
            assert_eq!(
                (p.forward, p.turn, p.feed),
                (w.forward, w.turn, w.feed),
                "{f:?} sample {k}: off food the parker walks as the walker does"
            );
        }
        assert!(
            brain_driver(f, "parker")
                .expect("named")
                .name()
                .contains("parker")
        );
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
    let mut flora = Flora::new(e.flora(0.25));
    let mut fauna = Fauna::new(e.fauna());
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
    let o = log
        .first()
        .expect("the controller was asked within 40 ticks");
    (o[0], o[1])
}

/// The contract (audit round, second package): the `Self` references are read at the
/// body's mass scale, so a x100 body reads the same fraction of an adult as a x1 body.
#[test]
fn self_senses_read_a_fraction_of_the_adult_at_any_mass() {
    let (energy1, reserve1) = self_readings(&EcologyParams::defaults());
    assert!(
        (reserve1 - 0.4).abs() < 0.02,
        "reads 40 % of an adult reserve: {reserve1}"
    );
    assert!(energy1 > 0.5 && energy1 < 0.95, "energy {energy1}");
    let (energy, reserve) = self_readings(&with("founder.mass_scale@frondgrazer", 100.0));
    assert!(
        (reserve - reserve1).abs() < 1e-6 && (energy - energy1).abs() < 1e-6,
        "x100 reads ({energy}, {reserve}) as the shipped mass reads ({energy1}, {reserve1})"
    );
}

/// The manifest carries the shipped mass scale and both reference rules, so a centre
/// trained at one mass cannot silently load at another: a GRU centre in a world whose
/// lineage mass differs from its manifest's is refused, a heuristic is not.
#[test]
fn a_centre_is_refused_at_another_mass() {
    for f in Founder::ALL {
        let m = f.manifest();
        assert_eq!(m.mass_scale, FaunaConfig::default().founder(f).mass_scale);
        let text = m.canonical_text();
        assert!(text.contains("|mass:"), "{text}");
        assert!(
            text.contains("self_ref:") && text.contains("taste_ref:"),
            "{text}"
        );
        let mut other = m.clone();
        other.mass_scale *= 2.0;
        assert_ne!(other.digest(), m.digest(), "the mass is in the digest");
    }
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/training/centres");
    let r2 = format!(
        "transfer:{}",
        dir.join("r2-frondgrazer-upd160.json").display()
    );
    let gru = brain_driver(Founder::Browser, &r2).expect("a transferred centre");
    let heur = brain_driver(Founder::Browser, "heuristic").expect("h");
    let shipped = EcologyParams::defaults();
    let heavy = with("founder.mass_scale@frondgrazer", 2.0);
    assert!(super::search::check_mass(&[(Founder::Browser, gru.clone())], &shipped).is_ok());
    assert!(super::search::check_mass(&[(Founder::Browser, gru)], &heavy).is_err());
    assert!(super::search::check_mass(&[(Founder::Browser, heur)], &heavy).is_ok());
}

/// Round 2's centres were written against the old digest: they load only as a transfer
/// (`transfer:<file>`, same vector length, restamped to this build's digest).
#[test]
fn round_two_centres_load_only_as_a_transfer() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/training/centres");
    for (file, f) in [
        ("r2-frondgrazer-upd160.json", Founder::Browser),
        ("r2-littershredder-upd512.json", Founder::Blind),
    ] {
        let path = dir.join(file);
        assert!(
            brain_driver(f, &path.display().to_string()).is_err(),
            "{file}: strict load refused"
        );
        let d = brain_driver(f, &format!("transfer:{}", path.display())).expect("transfer");
        assert!(d.name().contains("gru"), "{file}");
        assert!(
            brain_driver(
                Founder::ALL[1 - f.index()],
                &format!("transfer:{}", path.display())
            )
            .is_err()
        );
    }
}

/// The scale sweep runs the world the desktop host runs: `voxel-ecology-check --world
/// config/desktop/terrarium.toml` founds from that file's `[world]`, read as the app reads
/// it (the designed terrarium at 256 x 128 x 48, 0.125 m).
#[test]
fn the_check_reads_a_host_world_file() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/desktop/terrarium.toml");
    let c = super::search::check_world_config(Some(&path)).expect("the desktop terrarium");
    assert_eq!(
        (c.width, c.height, c.depth, c.voxel_m),
        (256, 128, 48, 0.125)
    );
    assert!(matches!(c.landform, cubarium_voxel::Landform::Terrarium(_)));
    let small = super::search::check_world_config(None).expect("the default");
    assert_eq!(small, super::world::training_config());
}

/// The shipped x30 body against the x1 placeholder: the bite's K is the x1 K (the food it
/// is measured against is not scaled), and the other mass-scaled quantities are x30.
#[test]
fn at_x30_the_bites_k_is_the_x1_k_and_the_rest_scale() {
    let k = cubarium_voxel_fauna::SHIPPED_MASS_SCALE;
    assert_eq!(k, 30.0);
    for f in Founder::ALL {
        let (x1, x30) = (
            cubarium_voxel_fauna::FounderPhysiology::placeholder(f).core,
            cubarium_voxel_fauna::FounderPhysiology::frozen(f).core,
        );
        assert_eq!(x30.bite_half_stock, x1.bite_half_stock, "{f:?}: K");
        for ((name, v), (_, v0)) in absolutes(&x30).into_iter().zip(absolutes(&x1)) {
            assert!(
                (v - k * v0).abs() <= 1e-12 * v.abs().max(1.0),
                "{f:?} {name}: {v} vs {k} x {v0}"
            );
        }
    }
    let (x1, x30) = (
        cubarium_voxel_fauna::FaunaConfig::placeholder().frondgrazer,
        FaunaConfig::default().frondgrazer,
    );
    assert_eq!(
        x30.bite_half_stock, x1.bite_half_stock,
        "the placeholder species' K"
    );
    assert!((x30.body_max - k * x1.body_max).abs() < 1e-12);
}
