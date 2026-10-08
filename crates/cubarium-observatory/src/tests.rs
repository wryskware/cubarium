//! Mechanism tests on tiny hand-built worlds and hand-built censuses. Every expectation is
//! read from the config or from the input; nothing pins a tuned number.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_fauna::{BoutKind, Command as FaunaCommand, Departure, Fauna, FaunaConfig, Food, Founder, StartingStores};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species};

use crate::life::{BodyState, Census, ClutchState};
use crate::query::{self, LifeStage};
use crate::{Cadence, Command, Controls, History, HistoryConfig, LifeBook, LifeRecorder, Observer, Sample, What};

/// A flat soil plain two cells deep: every column's support face is at `y = 2`.
fn plain(day: DayClock) -> World {
    let mut w = World::empty(VoxelConfig { width: 12, height: 6, depth: 6, voxel_m: 0.25, seed: 5, day, ..VoxelConfig::default() });
    for z in 0..6 {
        for x in 0..12 {
            for y in 1..=2 {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

fn introduce(fauna: &mut Fauna, world: &World, x: i64, founder: Founder, body: f64) {
    assert!(
        fauna.apply(
            world,
            FaunaCommand::IntroduceFounder { x, z: 3, founder, stores: StartingStores { body, reserve: 0.5 }, heading_rad: 0.0 }
        ),
        "introducing a {} at x = {x}",
        founder.name()
    );
}

/// An adult and a juvenile browser, a stand, and a buried lanternjaw clutch if the
/// lanternjaw's config lays dormant eggs.
pub(super) fn small_world() -> (World, Flora, Fauna) {
    let world = plain(DayClock::default());
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let core = fauna.config().founder(Founder::Browser).core;
    let juvenile = (core.body_min + core.birth_body) / 2.0 / core.body_max;
    introduce(&mut fauna, &world, 2, Founder::Browser, 1.0);
    introduce(&mut fauna, &world, 6, Founder::Browser, juvenile);
    let wood = flora.config().species(Species::Springturf).alive_min * 2.0;
    assert!(flora.apply(&world, FloraCommand::Seed { x: 9, z: 1, species: Species::Springturf, wood }));
    (world, flora, fauna)
}

// --- Queries ----------------------------------------------------------------------------

#[test]
fn populations_count_each_body_in_the_stage_its_config_puts_it() {
    let (world, mut flora, mut fauna) = small_world();
    let lj = fauna.config().founder(Founder::Lanternjaw).core.reproduction;
    let buried = lj.dormant
        && fauna.apply(&world, FaunaCommand::BuryClutch { site: Site { x: 4, y: 2, z: 4 }, lineage: Founder::Lanternjaw, eggs: 3 });
    let _ = &mut flora;
    let pops = query::populations(&flora, &fauna);

    assert_eq!(pops.animals.len(), Founder::COUNT, "every lineage is listed, alive or not");
    assert_eq!(pops.plants.len(), Species::ALL.len(), "every plant species is listed");
    let b = &pops.animals[Founder::Browser.index()];
    assert_eq!((b.alive, b.adults, b.juveniles, b.pups, b.riders), (2, 1, 1, 0, 0));
    for f in Founder::ALL.into_iter().filter(|f| *f != Founder::Browser) {
        assert_eq!(pops.animals[f.index()].alive, 0, "{} has no bodies", f.name());
    }
    if buried {
        assert_eq!(pops.animals[Founder::Lanternjaw.index()].eggs, 3);
    }
    let stand = &flora.view().stands[0];
    let p = &pops.plants[Species::Springturf.index()];
    assert_eq!(p.stands, 1);
    assert!((p.organic - stand.organic()).abs() < 1e-15);
    assert!((p.foliage - stand.foliage).abs() < 1e-15);

    // The stage follows `birth_body`, and the row's size is the body over it.
    for a in fauna.view().animals {
        let row = query::animal_row(&fauna, a);
        let birth = fauna.config().founder(Founder::Browser).core.birth_body;
        assert_eq!(row.stage == LifeStage::Adult, a.body >= birth);
        let full = fauna.config().founder(Founder::Browser).core.body_max;
        assert!((row.size - a.body / full).abs() < 1e-12);
        assert!((0.0..=1.0).contains(&row.fullness));
        assert_eq!(row.label, "frondgrazer");
    }
}

#[test]
fn an_animal_query_finds_the_living_and_only_them() {
    let (_, _, fauna) = small_world();
    let id = fauna.view().animals[0].id;
    let detail = query::animal(&fauna, id).expect("the first body lives");
    assert_eq!(detail.row.id, id);
    assert_eq!(detail.state, query::state_name(fauna.view().animals[0].state));
    let unused = fauna.view().animals.iter().map(|a| a.id).max().unwrap() + 1;
    assert!(query::animal(&fauna, unused).is_none());
}

#[test]
fn the_standing_water_is_the_free_water_and_the_soil_the_pore_water() {
    let mut world = plain(DayClock::default());
    world.apply(WorldCommand::RainPulse { volume_m3: 0.05 });
    for _ in 0..5 {
        world.step();
    }
    let env = query::environment(&world);
    let pooled = world.pooled_m3();
    assert!(pooled > 0.0, "the pulse left water standing");
    assert!((env.water.standing_m3 - pooled).abs() <= 1e-9 * pooled.max(1.0));
    let pore = world.pore_m3();
    assert!((env.water.soil_m3 - pore).abs() <= 1e-6 * (pore + pooled).max(1e-3), "{} vs {pore}", env.water.soil_m3);
    assert_eq!(env.water.atmosphere_m3, world.atmosphere_m3());
    assert_eq!(env.rain, query::rain_name(world.weather().mode()));
}

#[test]
fn the_clock_counts_days_from_the_start_hour() {
    // A one-minute day starting at 18:00: midnight is a quarter of a day (300 ticks) in.
    let day = DayClock { length_min: 1.0, start_hour: 18.0, ..DayClock::default() };
    let mut world = plain(day);
    world.step();
    let c = query::clock(&world);
    assert_eq!(c.day, 1);
    assert!((c.hour - c.day_phase * 24.0).abs() < 1e-12);
    assert!((c.day_length_s - 60.0).abs() < 1e-12);
    let to_midnight = (0.25 * 60.0 / DT).ceil() as u64;
    while world.tick() <= to_midnight {
        world.step();
    }
    let c = query::clock(&world);
    assert_eq!(c.day, 2, "past midnight at tick {}", c.tick);
    assert!((c.sim_elapsed_s - world.tick() as f64 * DT).abs() < 1e-9);
    assert!(c.hour < 1.0, "just after midnight: {}", c.time_of_day);
}

// --- Life histories on hand-built censuses ---------------------------------------------

fn body(id: u64, founder: Founder) -> BodyState {
    BodyState {
        id,
        founder: Some(founder),
        site: Site { x: id as u32, y: 2, z: 1 },
        x: id as f64,
        z: 0.25,
        stage: LifeStage::Adult,
        bout: None,
        stomach: [0.0; Food::COUNT],
        wound: 0.0,
        striking: None,
        escrow: false,
        given: 0.0,
        pack: None,
        riding: None,
        native: false,
        submerged: false,
        dryness: 0.0,
        aged: false,
        age_ticks: 100,
        meal: [0.0; Food::COUNT],
    }
}

fn census(tick: u64, bodies: Vec<BodyState>) -> Census {
    Census { tick, world_tick: None, bodies, clutches: vec![], dens: vec![], departed: vec![[0; Departure::COUNT]; Founder::COUNT] }
}

/// The world's tick runs this far ahead of the animal layer's (its pre-roll).
const WORLD_AHEAD: u64 = 1000;

fn stamped(tick: u64, bodies: Vec<BodyState>) -> Census {
    Census { world_tick: Some(tick + WORLD_AHEAD), ..census(tick, bodies) }
}

fn kinds(events: &[crate::LifeEvent], animal: u64) -> Vec<&'static str> {
    events.iter().filter(|e| e.animal == animal).map(|e| e.what.name()).collect()
}

#[test]
fn the_first_census_is_observed_and_a_birth_names_the_body_whose_gestation_ended() {
    let mut r = LifeRecorder::new();
    let mut mother = body(1, Founder::Browser);
    mother.escrow = true;
    let other = body(2, Founder::Browser);
    r.observe_census(&mut census(10, vec![mother, other]));
    let first = r.take_events();
    assert_eq!(kinds(&first, 1), ["observed"]);
    assert_eq!(kinds(&first, 2), ["observed"]);

    mother.escrow = false;
    mother.given = 0.2;
    let mut calf = body(3, Founder::Browser);
    calf.native = true;
    calf.stage = LifeStage::Juvenile;
    calf.age_ticks = 0;
    r.observe_census(&mut census(11, vec![mother, other, calf]));
    let ev = r.take_events();
    let born = ev.iter().find(|e| e.animal == 3).expect("the calf's birth");
    assert_eq!(born.what, What::Born { species: "frondgrazer".into(), how: "live_birth".into(), parent: Some(1) });
    assert!(ev.iter().any(|e| e.animal == 1 && e.what == What::GaveBirth { young: vec![3] }));
    assert!(kinds(&ev, 2).is_empty(), "the other adult did nothing");

    // An introduced body has no parent.
    let stranger = body(4, Founder::Browser);
    r.observe_census(&mut census(12, vec![mother, other, calf, stranger]));
    let ev = r.take_events();
    assert!(matches!(&ev[0].what, What::Born { how, parent: None, .. } if how == "introduced"));
}

#[test]
fn a_meal_is_what_the_stomach_gained_over_one_bout() {
    let mut r = LifeRecorder::new();
    let mut a = body(1, Founder::Blind);
    r.observe_census(&mut census(10, vec![a]));
    r.take_events();
    let litter = Food::Litter.index();
    // Bout opens on tick 11 with a bite; two more bites; digestion lowers it once.
    let mut eaten = 0.0;
    for (tick, bite, digest) in [(11, 0.01, 0.0), (12, 0.02, 0.0), (13, 0.0, 0.005), (14, 0.03, 0.0)] {
        a.bout = Some((11, BoutKind::Growth));
        a.stomach[litter] += bite - digest;
        eaten += bite;
        r.observe_census(&mut stamped(tick, vec![a]));
    }
    assert!((r.unfinished_meal(a.id)[litter] - eaten).abs() < 1e-12);
    assert!(r.unfinished_meals().any(|(id, meal)| id == a.id && meal[litter] > 0.0));
    a.bout = None;
    r.observe_census(&mut stamped(15, vec![a]));
    assert_eq!(r.unfinished_meal(a.id), [0.0; Food::COUNT]);
    let ev = r.take_events();
    assert_eq!(ev.len(), 1, "{ev:?}");
    let What::Meal { bout, foods, duration_s } = &ev[0].what else { panic!("{ev:?}") };
    assert_eq!(bout, "growth");
    assert_eq!(foods.len(), 1);
    assert_eq!(foods[0].food, "litter");
    assert!((foods[0].organic - eaten).abs() < 1e-12, "{} vs {eaten}", foods[0].organic);
    assert!((duration_s - 4.0 * DT).abs() < 1e-12, "timed on the layer's ticks");
    assert_eq!(ev[0].tick, 15 + WORLD_AHEAD, "stamped on the world's");
}

#[test]
fn departures_take_their_causes_from_the_ledger_and_a_kill_names_the_striker() {
    let mut r = LifeRecorder::new();
    let mut wet = body(1, Founder::Browser);
    wet.submerged = true;
    let dry = body(2, Founder::Browser);
    let prey = body(3, Founder::Blind);
    let mut hunter = body(4, Founder::Lanternjaw);
    hunter.striking = Some(3);
    r.observe_census(&mut census(10, vec![wet, dry, prey, hunter]));
    r.take_events();

    let mut c = census(11, vec![hunter]);
    c.departed[Founder::Browser.index()][Departure::Drowned.index()] = 1;
    c.departed[Founder::Browser.index()][Departure::Starved.index()] = 1;
    c.departed[Founder::Blind.index()][Departure::Killed { by: Founder::Lanternjaw }.index()] = 1;
    r.observe_census(&mut c);
    let ev = r.take_events();
    let died = |id| ev.iter().find_map(|e| match &e.what {
        What::Died { cause, by } if e.animal == id => Some((cause.clone(), *by)),
        _ => None,
    });
    assert_eq!(died(1), Some(("drowned".into(), None)), "the submerged one drowned");
    assert_eq!(died(2), Some(("starved".into(), None)));
    assert_eq!(died(3), Some((Departure::Killed { by: Founder::Lanternjaw }.name().into(), Some(4))));
    assert!(ev.iter().any(|e| e.animal == 4 && matches!(&e.what, What::Kill { prey: 3, .. })));
}

#[test]
fn a_clutch_names_its_layer_and_its_hatchlings_that_layer() {
    let mut r = LifeRecorder::new();
    let mut hen = body(1, Founder::Blind);
    r.observe_census(&mut census(10, vec![hen]));
    r.take_events();

    hen.given = 0.05;
    let mut c = census(11, vec![hen]);
    let site = Site { x: 1, y: 2, z: 1 };
    c.clutches.push(ClutchState { lineage: Founder::Blind, site, laid_tick: 11, eggs: 2 });
    r.observe_census(&mut c);
    let ev = r.take_events();
    assert_eq!(ev, vec![crate::LifeEvent { tick: 11, animal: 1, what: What::LaidEggs { eggs: 2, site: [1, 2, 1] } }]);

    let mut chicks = [body(5, Founder::Blind), body(6, Founder::Blind)];
    for c in &mut chicks {
        c.native = true;
        c.site = site;
    }
    r.observe_census(&mut census(12, vec![hen, chicks[0], chicks[1]]));
    let ev = r.take_events();
    for id in [5, 6] {
        let e = ev.iter().find(|e| e.animal == id).expect("hatched");
        assert_eq!(e.what, What::Born { species: Founder::Blind.name().into(), how: "hatched".into(), parent: Some(1) });
    }
}

#[test]
fn wounds_strikes_maturing_and_dens_are_recorded_as_they_change() {
    let mut r = LifeRecorder::new();
    let mut young = body(1, Founder::Chorister);
    young.stage = LifeStage::Pup;
    young.pack = Some(9);
    let mut hunter = body(2, Founder::Chorister);
    hunter.pack = Some(9);
    let mut c = census(10, vec![young, hunter]);
    c.dens.push((9, Site { x: 0, y: 2, z: 0 }));
    r.observe_census(&mut c);
    r.take_events();

    young.stage = LifeStage::Adult;
    young.wound = 0.3;
    hunter.striking = Some(1);
    let mut c = census(11, vec![young, hunter]);
    c.dens.push((9, Site { x: 3, y: 2, z: 0 }));
    r.observe_census(&mut c);
    let ev = r.take_events();
    let k = kinds(&ev, 1);
    for want in ["matured", "independent", "injured", "den_moved"] {
        assert!(k.contains(&want), "{want} in {k:?}");
    }
    assert_eq!(kinds(&ev, 2), ["hunt", "den_moved"]);
    // The injury was dealt before the strike was seen: no striker is named for it.
    assert!(ev.iter().any(|e| e.animal == 1 && e.what == What::Injured { wound: 0.3, by: None }));
}

#[test]
fn the_book_keeps_family_ties_and_lets_old_events_and_old_dead_go() {
    let mut r = LifeRecorder::new();
    let mut mother = body(1, Founder::Browser);
    mother.escrow = true;
    r.observe_census(&mut census(10, vec![mother]));
    mother.escrow = false;
    mother.given = 1.0;
    let mut calf = body(2, Founder::Browser);
    calf.native = true;
    r.observe_census(&mut census(11, vec![mother, calf]));
    let mut book = LifeBook::new(3, 1);
    book.apply(&r.take_events());
    assert_eq!(book.record(2).unwrap().parent, Some(1));
    assert_eq!(book.record(1).unwrap().offspring, vec![2]);
    assert_eq!(book.record(1).unwrap().origin.what.name(), "observed");

    // Five more events on the mother: three kept, two counted as dropped.
    let events: Vec<_> = (0..5)
        .map(|k| crate::LifeEvent { tick: 20 + k, animal: 1, what: What::Hunt { prey: 99 } })
        .collect();
    book.apply(&events);
    let m = book.record(1).unwrap();
    assert_eq!((m.events.len() as u64, m.dropped), (3, 3), "the birth and two hunts went");

    // One dead kept: the second death pushes the first out.
    let died = |id, tick| crate::LifeEvent { tick, animal: id, what: What::Died { cause: "aged".into(), by: None } };
    book.apply(&[died(2, 30)]);
    assert!(book.record(2).unwrap().death.is_some());
    book.apply(&[died(1, 31)]);
    assert!(book.record(2).is_none());
    assert!(book.record(1).is_some());
}

// --- History ----------------------------------------------------------------------------

fn sample_at(sim_s: f64) -> Sample {
    let (world, flora, fauna) = small_world();
    let mut s = Sample::of(&query::clock(&world), &query::populations(&flora, &fauna), &query::environment(&world));
    s.sim_s = sim_s;
    s.tick = (sim_s / DT) as u64;
    s
}

#[test]
fn the_history_takes_one_sample_per_interval_and_forgets_past_its_span() {
    let cfg = HistoryConfig { sample_s: 10.0, span_s: 30.0 };
    assert_eq!(cfg.capacity(), 3);
    let mut h = History::new(cfg);
    let template = sample_at(0.0);
    let at = |t: f64| Sample { sim_s: t, tick: (t / DT) as u64, ..template.clone() };
    assert!(h.offer(at(0.0)));
    assert!(!h.offer(at(4.0)), "too soon");
    assert!(h.offer(at(10.0)));
    assert!(h.offer(at(21.0)));
    assert!(h.offer(at(31.0)));
    assert_eq!(h.len(), 3, "the oldest went");
    assert_eq!(h.since(None).next().unwrap().sim_s, 10.0);
    assert_eq!(h.since(Some(at(21.0).tick)).count(), 1);
    // A world replaced under the host: time goes backwards and the ring starts again.
    assert!(h.offer(at(5.0)));
    assert_eq!(h.len(), 1);
}

// --- The observer -----------------------------------------------------------------------

fn controls() -> Controls {
    Controls { speed: 1.0, paused: false, min_speed: 1.0 / 64.0, max_speed: 64.0 }
}

#[test]
fn the_observer_publishes_at_once_then_once_per_sim_interval() {
    let (mut world, flora, fauna) = small_world();
    let cadence = Cadence { sim_s: 1.0, min_wall_s: 0.0, paused_wall_s: 1.0 };
    let mut o = Observer::new("test", cadence);
    let t0 = Instant::now();
    assert!(o.due(world.tick(), t0, false), "the first publication is due at once");
    let p = o.publish(&world, &flora, &fauna, controls(), t0);
    assert_eq!(p.snapshot.seq, 1);
    assert_eq!(p.snapshot.animals.len(), fauna.view().animals.len());
    assert!(!o.due(world.tick(), t0, false));
    let per = (cadence.sim_s / DT).round() as u64;
    for _ in 0..per - 1 {
        world.step();
        o.after_tick(&world, &fauna, 1);
    }
    assert!(!o.due(world.tick(), t0, false), "one tick short");
    world.step();
    assert!(o.due(world.tick(), t0, false));
    // Paused, the wall clock decides.
    assert!(!o.due(world.tick() + 1000, t0, true));
    assert!(o.due(world.tick(), t0 + Duration::from_secs_f64(cadence.paused_wall_s), true));
    o.request();
    assert!(o.due(world.tick(), t0, true), "a request is due whatever the cadence");
}

// --- The server -------------------------------------------------------------------------

#[cfg(feature = "server")]
mod server {
    use super::*;
    use crate::server::{Server, parse_control};

    fn get(addr: std::net::SocketAddr, path: &str) -> (u16, String) {
        request(addr, &format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\n\r\n"))
    }

    fn request(addr: std::net::SocketAddr, raw: &str) -> (u16, String) {
        let mut s = TcpStream::connect(addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        s.write_all(raw.as_bytes()).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        let code = out[9..12].parse().unwrap();
        let body = out.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or_default();
        (code, body)
    }

    fn post(addr: std::net::SocketAddr, body: &str, extra: &str) -> u16 {
        request(
            addr,
            &format!(
                "POST /control HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\n\r\n{body}",
                body.len()
            ),
        )
        .0
    }

    #[test]
    fn every_route_answers_and_commands_reach_the_host() {
        let (tx, rx) = mpsc::channel();
        let server = Server::start("127.0.0.1:0", HistoryConfig::default(), tx).unwrap();
        let addr = server.addr();
        let (world, flora, fauna) = small_world();
        let mut o = Observer::new("test", Cadence::default());
        server.publish(o.publish(&world, &flora, &fauna, controls(), Instant::now()));
        let until = Instant::now() + Duration::from_secs(5);
        while server.published() == 0 && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(server.published(), 1);

        let (code, page) = get(addr, "/");
        assert_eq!(code, 200);
        assert!(page.contains("<html") || page.contains("<!doctype"));
        for path in ["/snapshot", "/summary", "/clock", "/run", "/populations", "/environment", "/history", "/animals"] {
            let (code, body) = get(addr, path);
            assert_eq!(code, 200, "{path}");
            serde_json::from_str::<serde_json::Value>(&body).unwrap_or_else(|e| panic!("{path}: {e}"));
        }
        let (_, snap) = get(addr, "/snapshot");
        let snap: serde_json::Value = serde_json::from_str(&snap).unwrap();
        assert_eq!(snap["animals_alive"], fauna.view().animals.len());
        assert!(snap["sample"].is_object(), "the first publication is sampled");
        let (_, history) = get(addr, "/history");
        let history: serde_json::Value = serde_json::from_str(&history).unwrap();
        assert_eq!(history["samples"].as_array().unwrap().len(), 1);

        let id = fauna.view().animals[0].id;
        let (code, one) = get(addr, &format!("/animals/{id}"));
        assert_eq!(code, 200);
        let one: serde_json::Value = serde_json::from_str(&one).unwrap();
        assert_eq!(one["alive"], true);
        assert_eq!(one["history"]["origin"]["kind"], "observed");
        assert_eq!(get(addr, "/animals/987654321").0, 404);
        assert_eq!(get(addr, "/animals/x").0, 400);
        assert_eq!(get(addr, "/nowhere").0, 404);

        assert_eq!(post(addr, r#"{"speed": 4}"#, ""), 202);
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), Command::SetSpeed(4.0));
        assert_eq!(post(addr, r#"{"paused": true}"#, &format!("Origin: http://{addr}\r\n")), 202);
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), Command::Pause);
        assert_eq!(post(addr, r#"{"paused": false}"#, "Origin: http://elsewhere.example\r\n"), 403);
        assert_eq!(post(addr, r#"{"speed": -1}"#, ""), 400);
        let raw = format!("POST /control HTTP/1.1\r\nHost: {addr}\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\n{{}}");
        assert_eq!(request(addr, &raw).0, 415);
        assert!(rx.try_recv().is_err(), "refused commands never reach the host");

        // The stream sends the newest snapshot at once.
        let mut s = TcpStream::connect(addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        s.write_all(format!("GET /stream HTTP/1.1\r\nHost: {addr}\r\n\r\n").as_bytes()).unwrap();
        let mut lines = BufReader::new(s).lines();
        assert!(lines.next().unwrap().unwrap().starts_with("HTTP/1.1 200"));
        let event = lines.map(|l| l.unwrap()).find(|l| l.starts_with("event:")).unwrap();
        assert_eq!(event, "event: snapshot");
    }

    #[test]
    fn control_bodies_parse_to_commands() {
        assert_eq!(parse_control(br#"{"speed": 0.5}"#), Ok(vec![Command::SetSpeed(0.5)]));
        assert_eq!(parse_control(br#"{"paused": false}"#), Ok(vec![Command::Resume]));
        assert_eq!(parse_control(br#"{"step": "slower"}"#), Ok(vec![Command::Slower]));
        assert!(parse_control(br#"{"speed": 0}"#).is_err());
        assert!(parse_control(br#"{}"#).is_err());
        assert!(parse_control(b"speed=4").is_err());
    }
}
