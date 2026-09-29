//! **The grazing defence, readable** (`design/handoffs/voxel-audit-round-2026-09-28.md` §4,
//! audit §1.3.3): the mouth chooses the stand by what it would give, `(1 − defence) ×
//! stock`; taste reports the same; and the bite is shrunk by the defence once (the plant
//! layer's `(1 − defence)`), not again through the stock it diminishes on. Under 200
//! ticks; no pinned hashes.

use std::sync::{Arc, Mutex};

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Actions, Command as FaunaCommand, Controller, Fauna, FaunaConfig, Founder, Response,
    StartingStores, browser_mouth_candidates, browser_mouth_foliage,
};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species as Plant};

fn flat_world() -> World {
    let mut world = World::empty(VoxelConfig {
        width: 8,
        height: 6,
        depth: 6,
        voxel_m: 0.25,
        day: cubarium_voxel::weather::DayClock {
            mode: cubarium_voxel::weather::DayMode::Off,
            ..Default::default()
        },
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

fn turf(flora: &mut Flora, world: &World, x: i64, z: u32) {
    let wood = flora.config().species(Plant::Springturf).wood_max;
    assert!(flora.apply(
        world,
        FloraCommand::Seed {
            x,
            z,
            species: Plant::Springturf,
            wood,
        },
    ));
}

fn defence(flora: &Flora, s: Site) -> f64 {
    flora.view().stand_at(s).expect("standing").defence
}

/// Graze `s` with crumbs until its defence reaches `d`.
fn defend(flora: &mut Flora, s: Site, d: f64) {
    let mut n = 0;
    while defence(flora, s) < d {
        flora.take_foliage(s, 1e-8).expect("a bite");
        n += 1;
        assert!(n < 20_000, "the defence never reached {d}");
    }
}

#[derive(Clone)]
struct Recorder {
    log: Arc<Mutex<Vec<Vec<f64>>>>,
    feed: f64,
}

impl Controller for Recorder {
    fn drive(&mut self, observation: &[f64]) -> Response {
        self.log.lock().unwrap().push(observation.to_vec());
        Response::Bounded(Actions {
            forward: 0.0,
            turn: 0.0,
            feed: self.feed,
        })
    }
    fn reset(&mut self) {}
}

/// A browser at `(2, 2)` facing `+z`, recording what it senses and feeding when `feed`.
fn browser(
    fauna: &mut Fauna,
    world: &World,
    stores: StartingStores,
    feed: f64,
) -> Arc<Mutex<Vec<Vec<f64>>>> {
    let id = fauna.view().ledger.births;
    assert!(fauna.apply(
        world,
        FaunaCommand::IntroduceFounder {
            x: 2,
            z: 2,
            founder: Founder::Browser,
            stores,
            heading_rad: 0.0,
        },
    ));
    let log = Arc::new(Mutex::new(Vec::new()));
    assert!(fauna.set_controller(
        id,
        Box::new(Recorder {
            log: Arc::clone(&log),
            feed,
        })
    ));
    log
}

fn mouth(world: &World, flora: &Flora, fauna: &Fauna) -> Option<(Site, f64)> {
    let av = fauna.view();
    browser_mouth_foliage(&world.view(), &flora.view(), fauna.config(), &av.animals[0])
}

/// The taste cue the browser's first sample recorded.
fn taste(log: &Arc<Mutex<Vec<Vec<f64>>>>) -> f64 {
    let m = Founder::Browser.manifest();
    let t = m
        .modules
        .iter()
        .find(|m| m.name == "Taste(1)")
        .expect("a taste module");
    let log = log.lock().unwrap();
    let o = log.first().expect("sampled");
    assert_eq!(o[t.offset + 2], 1.0, "the mouth is on food");
    o[t.offset]
}

/// The taste's response to a stock: `c / (c + 1)` with `c = min(stock / 0.05, 1)`.
fn response(stock: f64) -> f64 {
    let c = (stock / 0.05).min(1.0);
    c / (c + 1.0)
}

/// Three turfs in the mouth's reach: the one it would take undefended is the richest;
/// defend that one to 0.9 and the mouth takes another, while still reporting the true
/// stock of the one it chose (the `E` a bite diminishes on).
#[test]
fn the_mouth_chooses_the_stand_by_what_it_would_give() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::for_voxel_size(0.25));
    for x in 1..=3 {
        turf(&mut flora, &world, x, 3);
    }
    let mut fauna = Fauna::new(FaunaConfig::default());
    browser(&mut fauna, &world, StartingStores::FULL, 0.0);
    let candidates = {
        let av = fauna.view();
        browser_mouth_candidates(&world.view(), &flora.view(), fauna.config(), &av.animals[0])
            .expect("a browser")
    };
    assert!(candidates.len() >= 2, "the fixture puts two stands in reach: {candidates:?}");
    let (first, _) = mouth(&world, &flora, &fauna).expect("on food");
    defend(&mut flora, first, 0.9);
    let (second, stock) = mouth(&world, &flora, &fauna).expect("still on food");
    assert_ne!(second, first, "a defended stand is not the best offer");
    let true_stock = candidates
        .iter()
        .find(|c| c.0 == second)
        .expect("a candidate")
        .1;
    assert!(
        (stock - true_stock).abs() < 1e-12,
        "the mouth reports the true stock {true_stock}, not the offer: {stock}"
    );
}

/// One turf in reach: taste reads `(1 − defence) × stock`, so a defended turf tastes thin.
#[test]
fn taste_reports_what_the_stand_would_give() {
    let read = |d: f64| {
        let world = flat_world();
        let mut flora = Flora::new(FloraConfig::for_voxel_size(0.25));
        turf(&mut flora, &world, 2, 3);
        let mut fauna = Fauna::new(FaunaConfig::default());
        let log = browser(&mut fauna, &world, StartingStores::FULL, 0.0);
        let (s, stock) = mouth(&world, &flora, &fauna).expect("on food");
        if d > 0.0 {
            defend(&mut flora, s, d);
        }
        let d = defence(&flora, s);
        for _ in 0..5 {
            fauna.step(&world, &mut flora);
        }
        (taste(&log), response((1.0 - d) * stock), d)
    };
    let (open, want_open, _) = read(0.0);
    assert!((open - want_open).abs() < 1e-3, "undefended: {open} vs {want_open}");
    let (shut, want_shut, d) = read(0.9);
    assert!(
        (shut - want_shut).abs() < 1e-3 && shut < 0.5 * open,
        "defended at {d}: tastes {shut}, want {want_shut} (undefended {open})"
    );
}

/// The bite is shrunk by the defence once: a hungry browser's first bite from a turf at
/// defence `d` is `(1 − d)` of the same bite from an undefended one — not less, as it
/// would be if the stock it diminishes on were also cut.
#[test]
fn the_defence_shrinks_the_bite_once() {
    let bite = |d: f64| {
        let world = flat_world();
        let mut flora = Flora::new(FloraConfig::for_voxel_size(0.25));
        turf(&mut flora, &world, 2, 3);
        let mut fauna = Fauna::new(FaunaConfig::default());
        browser(&mut fauna, &world, StartingStores::HUNGRY, 1.0);
        let (s, _) = mouth(&world, &flora, &fauna).expect("on food");
        if d > 0.0 {
            defend(&mut flora, s, d);
        }
        let d = defence(&flora, s);
        let before = flora.view().stand_at(s).expect("standing").foliage;
        for _ in 0..5 {
            fauna.step(&world, &mut flora);
        }
        let after = flora.view().stand_at(s).expect("standing").foliage;
        (before - after, d)
    };
    let (open, _) = bite(0.0);
    let (shut, d) = bite(0.9);
    assert!(open > 0.0, "the hungry browser bit");
    let ratio = shut / open;
    assert!(
        (ratio - (1.0 - d)).abs() < 0.01,
        "defence {d}: the bite kept {ratio}, not {}",
        1.0 - d
    );
}
