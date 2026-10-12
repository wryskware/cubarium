//! Contract v3 body actions on hand-built scenes, independent of tuned defaults.

use super::predation_fixture::*;
use crate::{
    Actions, Activity, BodyActions, Controller, Fauna, FaunaConfig, Founder, Hunker, Layer,
    MobilityMode, PackMember, Response, SenseTable, Sprint, StartingStores, State,
};
use cubarium_voxel::{DT, World};
use cubarium_voxel_flora::{Flora, FloraConfig};

struct Answer {
    response: Response,
    layer: Layer,
}

impl Controller for Answer {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        self.response
    }

    fn reset(&mut self) {}

    fn layer(&self) -> Layer {
        self.layer
    }
}

fn config() -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        let p = &mut c.founders[f.index()];
        p.active = Activity::Always;
        p.motor_respiration_per_s = 0.0;
    }
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    c
}

fn sprint_config() -> FaunaConfig {
    let mut c = config();
    for f in [Founder::Chorister, Founder::Seedporter] {
        let p = &mut c.founders[f.index()];
        p.cruise_body_lengths_per_s = 1.0;
        p.sprint = Sprint {
            body_lengths_per_s: 3.0,
            endurance_s: 2.0,
            recovery_s: 4.0,
            resume_fraction: 0.5,
        };
    }
    c
}

fn install(fauna: &mut Fauna, id: u64, response: Response, layer: Layer) {
    fauna
        .controllers
        .set(id, Box::new(Answer { response, layer }));
}

fn brain(fauna: &mut Fauna, id: u64, actions: Actions, body: BodyActions) {
    install(fauna, id, Response::with_body(actions, body), Layer::Brain);
}

fn body(fauna: &mut Fauna, world: &World, f: Founder, x: f64) -> u64 {
    let id = introduce(fauna, world, f, StartingStores::FULL);
    place(fauna, id, x, LANE_Z, EAST);
    brain(fauna, id, Actions::REST, BodyActions::default());
    let a = animal_mut(fauna, id);
    a.pose.y_m = crate::surface_m(a.site.y, world.config().voxel_m);
    a.state = State::Resting;
    a.mobility.mode = MobilityMode::Ground;
    a.mobility.perch = None;
    id
}

fn index(fauna: &Fauna, id: u64) -> usize {
    fauna
        .animals
        .iter()
        .position(|a| a.id == id)
        .expect("alive")
}

fn sky(clock_on: bool, daylight: f64) -> super::Sky {
    super::Sky {
        downpour: false,
        rain_m_per_s: 0.0,
        clock_on,
        daylight,
    }
}

fn control_at(fauna: &mut Fauna, world: &World, flora: &Flora, id: u64, sky: &super::Sky) {
    fauna.tick += 1;
    let founder = animal(fauna, id).founder.expect("a founder");
    animal_mut(fauna, id).age_ticks = founder.manifest().cadence_ticks();
    super::controllers(fauna, &world.view(), flora, None, sky, 1, None);
}

fn control(fauna: &mut Fauna, world: &World, flora: &Flora, id: u64) {
    control_at(fauna, world, flora, id, &sky(false, 1.0));
}

fn act(fauna: &mut Fauna, world: &World, flora: &mut Flora) {
    let tick = fauna.tick;
    super::act(
        fauna,
        &world.view(),
        flora,
        1,
        tick,
        &sky(false, 1.0),
        &mut Vec::new(),
    );
}

fn executed(fauna: &Fauna, id: u64) -> crate::Held {
    fauna
        .tap()
        .expect("recording")
        .stream(id)
        .expect("a stream")
        .last()
        .expect("a sample")
        .executed
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

#[test]
fn bounded_and_logit_sprint_actions_reach_the_gate_and_tap_the_gait_used() {
    for response in [
        Response::with_body(
            Actions {
                forward: 1.0,
                ..Actions::REST
            },
            BodyActions {
                sprint: 0.5,
                ..BodyActions::default()
            },
        ),
        Response::logits(&[50.0, 0.0, -50.0, 50.0]),
    ] {
        let world = clock_off(701);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(sprint_config());
        let id = body(&mut fauna, &world, Founder::Chorister, 2.0);
        install(&mut fauna, id, response, Layer::Brain);
        fauna.start_tap();
        control(&mut fauna, &world, &flora, id);
        let p = *fauna.config.founder(Founder::Chorister);
        let top = p.sprint.effort(p.cruise_body_lengths_per_s);
        let ask = crate::controller::resolve(response, &fauna.config.manifest(Founder::Chorister));
        let forward = ask.actions.forward * (1.0 + ask.body.sprint * (top - 1.0));
        let cruise = fauna.config.manifest(Founder::Chorister).cruise_m_per_s;
        let before = animal(&fauna, id).pose.x;
        act(&mut fauna, &world, &mut flora);
        let a = animal(&fauna, id);
        let used = (a.pose.x - before) / (cruise * DT);
        close(used, forward);
        assert!(used > 1.0, "the body actually moved above cruise");
        close(
            a.founder_state.sprint_spent,
            DT / p.sprint.endurance_s * (used - 1.0) / (top - 1.0),
        );
        close(executed(&fauna, id).body.sprint, (used - 1.0) / (top - 1.0));
        assert_eq!(executed(&fauna, id).actions.forward, 1.0);
    }
}

#[test]
fn a_winded_brain_body_refuses_sprint_and_taps_zero() {
    let world = clock_off(702);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(sprint_config());
    let id = body(&mut fauna, &world, Founder::Chorister, 2.0);
    brain(
        &mut fauna,
        id,
        Actions {
            forward: 1.0,
            ..Actions::REST
        },
        BodyActions {
            sprint: 1.0,
            ..BodyActions::default()
        },
    );
    {
        let a = animal_mut(&mut fauna, id);
        a.founder_state.sprint_spent = 1.0;
        a.founder_state.winded = true;
    }
    fauna.start_tap();
    control(&mut fauna, &world, &flora, id);
    let before = animal(&fauna, id).pose.x;
    let cruise = fauna.config.manifest(Founder::Chorister).cruise_m_per_s;
    act(&mut fauna, &world, &mut flora);
    close((animal(&fauna, id).pose.x - before) / (cruise * DT), 1.0);
    assert!(
        animal(&fauna, id).founder_state.sprint_spent < 1.0,
        "the refused sprint rests"
    );
    assert_eq!(executed(&fauna, id).body.sprint, 0.0);
}

#[test]
fn a_strike_owns_the_tick_and_taps_no_sprint() {
    let world = clock_off(703);
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = sprint_config();
    c.founders[Founder::Chorister.index()]
        .hunter
        .as_mut()
        .expect("hunter")
        .windup_s = 1.0;
    let mut fauna = Fauna::new(c);
    let id = body(&mut fauna, &world, Founder::Chorister, 2.0);
    let prey = body(&mut fauna, &world, Founder::Browser, 3.0);
    brain(
        &mut fauna,
        id,
        Actions {
            forward: 1.0,
            ..Actions::REST
        },
        BodyActions {
            sprint: 1.0,
            ..BodyActions::default()
        },
    );
    let i = index(&fauna, id);
    super::start_strike(&mut fauna, i, prey);
    fauna.start_tap();
    control(&mut fauna, &world, &flora, id);
    let before = animal(&fauna, id).pose;
    act(&mut fauna, &world, &mut flora);
    assert_eq!(animal(&fauna, id).pose, before, "the windup holds position");
    assert_eq!(animal(&fauna, id).founder_state.sprint_spent, 0.0);
    assert_eq!(executed(&fauna, id).body.sprint, 0.0);
    assert_eq!(executed(&fauna, id).actions.forward, 0.0);
}

#[test]
fn an_off_ground_seedporter_taps_no_sprint_and_recovers_stamina() {
    let world = clock_off(704);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(sprint_config());
    let id = body(&mut fauna, &world, Founder::Seedporter, 2.0);
    brain(
        &mut fauna,
        id,
        Actions {
            forward: 1.0,
            ..Actions::REST
        },
        BodyActions {
            sprint: 1.0,
            ..BodyActions::default()
        },
    );
    {
        let a = animal_mut(&mut fauna, id);
        a.pose.y_m = SURFACE_M;
        let site = a.site;
        crate::flight::perch_on_face(a, site);
        a.founder_state.sprint_spent = 0.5;
    }
    fauna.start_tap();
    control(&mut fauna, &world, &flora, id);
    assert_eq!(animal(&fauna, id).mobility.mode, MobilityMode::Perched);
    act(&mut fauna, &world, &mut flora);
    assert!(animal(&fauna, id).founder_state.sprint_spent < 0.5);
    assert_eq!(executed(&fauna, id).body.sprint, 0.0);
    assert!(
        (0.0..=1.0).contains(&executed(&fauna, id).actions.forward),
        "an executed local action remains bounded"
    );
}

fn hunker_config() -> FaunaConfig {
    let mut c = config();
    c.founders[Founder::Browser.index()].hunker = Hunker {
        enabled: true,
        herd_enabled: false,
        enter_m: 3.0,
        leave_m: 4.0,
        calm_s: 1.0,
        shield: 0.75,
        shield_growth_exp: 1.0,
        pry_per_member_s: 0.25 / DT,
        pry_reach_m: 2.0,
        opened_s: 1.0,
        ..Hunker::default()
    };
    c.founders[Founder::Browser.index()].senses = SenseTable {
        live: true,
        sight_m: 6.0,
        sight_night_m: 6.0,
        sight_still_fraction: 1.0,
        sight_half_angle_deg: 180.0,
        ..SenseTable::default()
    };
    c
}

#[test]
fn a_brain_hunker_action_enters_the_shield_and_stops_its_motor() {
    let world = clock_off(705);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(hunker_config());
    let id = body(&mut fauna, &world, Founder::Browser, 2.0);
    brain(
        &mut fauna,
        id,
        Actions {
            forward: 1.0,
            feed: 1.0,
            ..Actions::REST
        },
        BodyActions {
            hunker: 1.0,
            ..BodyActions::default()
        },
    );
    control(&mut fauna, &world, &flora, id);
    let a = animal(&fauna, id);
    assert!(a.hunkered);
    assert!(fauna.percept(id).expect("vigil").vigil.hunkered);
    let guard = crate::vigil::guard(&fauna.config, &a, fauna.percept(id), fauna.tick);
    let crate::vigil::Guard::Shield(shield) = guard else {
        panic!("a strike meets the shield: {guard:?}")
    };
    let p = fauna.config.founder(Founder::Browser);
    close(
        shield,
        p.hunker.shield_of(a.body, p.core.birth_body, a.wound),
    );
    let before = a.pose;
    act(&mut fauna, &world, &mut flora);
    assert_eq!(animal(&fauna, id).pose, before);
}

#[test]
fn a_zero_brain_hunker_does_not_auto_hunker_at_a_seen_approaching_threat() {
    let world = clock_off(706);
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(hunker_config());
    let id = body(&mut fauna, &world, Founder::Browser, 2.0);
    let threat = body(&mut fauna, &world, Founder::Chorister, 3.0);
    place(&mut fauna, threat, 3.0, LANE_Z, WEST);
    let cruise = fauna.config.manifest(Founder::Chorister).cruise_m_per_s;
    {
        let a = animal_mut(&mut fauna, threat);
        a.speed_m_s = cruise;
        a.still_ticks = 0;
        a.state = State::Walking;
    }
    control(&mut fauna, &world, &flora, id);
    assert!(
        fauna
            .percept(id)
            .and_then(|p| p.track(threat))
            .is_some_and(|t| t.modes & crate::perception::SIGHT != 0),
        "the threat is actually seen"
    );
    assert!(!animal(&fauna, id).hunkered);
    assert!(!fauna.percept(id).expect("vigil").vigil.hunkered);
}

#[test]
fn holding_the_brain_hunker_preserves_pry_work_and_the_opened_interval() {
    let world = clock_off(707);
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(hunker_config());
    let id = body(&mut fauna, &world, Founder::Browser, 2.0);
    body(&mut fauna, &world, Founder::Chorister, 2.5);
    brain(
        &mut fauna,
        id,
        Actions::REST,
        BodyActions {
            hunker: 1.0,
            ..BodyActions::default()
        },
    );
    control(&mut fauna, &world, &flora, id);
    let ring = f64::from(world.view().config.width) * world.view().config.voxel_m;
    crate::vigil::pry(&mut fauna, ring);
    let pry = fauna.percept(id).expect("vigil").vigil.pry;
    assert!(pry > 0.0 && pry < 1.0, "the hunter did some work");
    control(&mut fauna, &world, &flora, id);
    assert_eq!(fauna.percept(id).expect("vigil").vigil.pry, pry);
    for _ in 0..3 {
        crate::vigil::pry(&mut fauna, ring);
    }
    let until = fauna.percept(id).expect("vigil").vigil.opened_until;
    assert!(until > fauna.tick, "the completed pry opens the body");
    control(&mut fauna, &world, &flora, id);
    assert_eq!(fauna.percept(id).expect("vigil").vigil.opened_until, until);
    assert!(matches!(
        crate::vigil::guard(
            &fauna.config,
            &animal(&fauna, id),
            fauna.percept(id),
            fauna.tick
        ),
        crate::vigil::Guard::Bared
    ));
}

#[test]
fn retraction_outlasts_the_action_for_its_configured_minimum_hold() {
    let world = clock_off(708);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    let hold_s = 4.0 * DT;
    c.founders[Founder::RippleSnail.index()]
        .amphibious
        .as_mut()
        .expect("amphibious")
        .retract_hold_s = hold_s;
    let mut fauna = Fauna::new(c);
    let id = body(&mut fauna, &world, Founder::RippleSnail, 2.0);
    brain(
        &mut fauna,
        id,
        Actions::REST,
        BodyActions {
            retract: 1.0,
            ..BodyActions::default()
        },
    );
    control(&mut fauna, &world, &flora, id);
    let hold = crate::Hunter::ticks(hold_s);
    assert_eq!(animal(&fauna, id).mobility.retract_ticks, hold);
    brain(&mut fauna, id, Actions::REST, BodyActions::default());
    control(&mut fauna, &world, &flora, id);
    assert_eq!(
        animal(&fauna, id).mobility.retract_ticks,
        hold,
        "dropping the action does not release it"
    );
    for remaining in (0..hold).rev() {
        super::desiccation(&mut fauna, &world.view(), &sky(false, 1.0));
        assert_eq!(animal(&fauna, id).mobility.retract_ticks, remaining);
    }
}

#[test]
fn a_brain_pups_strike_is_refused_while_the_same_adult_can_strike() {
    for pup in [false, true] {
        let world = clock_off(709);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(config());
        let id = body(&mut fauna, &world, Founder::Chorister, 2.0);
        let h = fauna
            .config
            .founder(Founder::Chorister)
            .hunter
            .expect("hunter");
        let g = fauna
            .config
            .founder(Founder::Chorister)
            .body_at(animal(&fauna, id).body);
        let (x, z) = ahead_of(
            &fauna,
            id,
            g.length_m * (h.jaw_forward_length_fraction + 0.5 * h.strike_reach_length_fraction),
        );
        let prey = introduce(
            &mut fauna,
            &world,
            Founder::Browser,
            StartingStores {
                body: 0.2,
                reserve: 1.0,
            },
        );
        place(&mut fauna, prey, x, z, WEST);
        brain(&mut fauna, prey, Actions::REST, BodyActions::default());
        animal_mut(&mut fauna, id).pack = Some(PackMember {
            pack: 1,
            pup,
            calling_ticks: 0,
        });
        animal_mut(&mut fauna, id).eating = true;
        brain(
            &mut fauna,
            id,
            Actions {
                feed: 1.0,
                ..Actions::REST
            },
            BodyActions::default(),
        );
        control(&mut fauna, &world, &flora, id);
        let i = index(&fauna, id);
        assert_eq!(
            super::prey_in_capsule(&fauna, i, &world.view(), &g, &h),
            Some(prey),
            "a usable prey lies in the strike capsule"
        );
        act(&mut fauna, &world, &mut flora);
        assert_eq!(animal(&fauna, id).strike.is_some(), !pup);
    }
}

#[test]
fn response_lure_is_held_in_darkness_and_refused_in_sunlight() {
    let world = clock_off(710);
    let flora = Flora::new(FloraConfig::default());
    let mut c = config();
    c.founders[Founder::Lanternjaw.index()].lure_daylight = false;
    let mut fauna = Fauna::new(c);
    let id = body(&mut fauna, &world, Founder::Lanternjaw, 2.0);
    animal_mut(&mut fauna, id).eating = true;
    brain(
        &mut fauna,
        id,
        Actions {
            feed: 1.0,
            ..Actions::REST
        },
        BodyActions {
            lure: 0.6,
            ..BodyActions::default()
        },
    );
    control_at(&mut fauna, &world, &flora, id, &sky(true, 0.0));
    assert_eq!(animal(&fauna, id).founder_state.lure, 0.6);
    control_at(&mut fauna, &world, &flora, id, &sky(true, 1.0));
    assert_eq!(animal(&fauna, id).founder_state.lure, 0.0);
}

#[test]
fn response_lift_reaches_the_held_flight_control_in_both_directions() {
    let world = clock_off(711);
    let flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config());
    let id = body(&mut fauna, &world, Founder::Bellwing, 2.0);
    for lift in [-0.4, 0.4] {
        brain(
            &mut fauna,
            id,
            Actions::REST,
            BodyActions {
                lift,
                ..BodyActions::default()
            },
        );
        control(&mut fauna, &world, &flora, id);
        assert_eq!(animal(&fauna, id).founder_state.lift, lift);
    }
}
