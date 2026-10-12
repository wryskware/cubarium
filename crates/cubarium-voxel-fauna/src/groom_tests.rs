//! Grooming laws on hand-built contacts; no ecological tuning assertions.
#[allow(dead_code)]
#[path = "predation_fixture.rs"]
mod fixture;

use crate::{
    Activity, Attachment, Departure, Fauna, FaunaConfig, Founder, Grooming, StartingStores, State,
};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig, Taken};
use fixture::*;

const SP: Founder = Founder::Seedporter;
const LJ: Founder = Founder::Lanternjaw;

fn scene(groomer: Founder, host_kind: Founder, on: bool) -> (World, Flora, Fauna, u64, u64, u64) {
    let world = clock_off(903);
    let flora = Flora::new(FloraConfig::default());
    let mut c = FaunaConfig::default();
    for p in &mut c.founders {
        p.active = Activity::Always;
        p.catch_bellwings = false;
        p.herding = crate::Herding::default();
    }
    c.perception.remote_hz = f64::from(crate::TICK_HZ);
    let p = &mut c.founders[SP.index()];
    p.grooming = if on {
        Grooming {
            groom_radius_m: 0.4,
            groom_s: 0.25,
            groom_seek_m: 3.0,
        }
    } else {
        Grooming::default()
    };
    p.gut.stomach_k = 10.0;
    p.gut.s0 = 0.0;
    p.vigilance = crate::Vigilance::default();
    p.bite_spill_fraction = 0.0;
    p.senses.sight_still_fraction = 1.0;
    p.senses.sight_half_angle_deg = 180.0;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    let a = introduce(&mut fauna, &world, groomer, StartingStores::FULL);
    let h = introduce(&mut fauna, &world, host_kind, StartingStores::FULL);
    let r = introduce(
        &mut fauna,
        &world,
        LJ,
        StartingStores {
            body: 0.08,
            reserve: 0.3,
        },
    );
    for (id, x) in [(a, 2.0), (h, 2.2), (r, 2.2)] {
        place(&mut fauna, id, x, LANE_Z, EAST);
        animal_mut(&mut fauna, id).pose.y_m = SURFACE_M;
        drive(&mut fauna, id, 0.0, false);
    }
    animal_mut(&mut fauna, r).attached = Some(Attachment { host: h });
    (world, flora, fauna, a, h, r)
}

fn advance(world: &World, flora: &mut Flora, fauna: &mut Fauna, ticks: u64) {
    for _ in 0..ticks {
        fauna.tick += 1;
        crate::groom::step(fauna, &world.view(), flora);
    }
}

fn duration(fauna: &Fauna) -> u64 {
    (fauna.config.founder(SP).grooming.groom_s / crate::DT).ceil() as u64
}

fn whole(a: &crate::Animal) -> Taken {
    let seeds = a.gut_seeds.total();
    let fed = a.stomach.total();
    Taken {
        organic: a.organic() + a.gut.organic + seeds.organic + fed.organic,
        mineral: a.mineral + a.gut.mineral + seeds.mineral + fed.mineral,
        energy: a.energy + a.fuel_energy + a.gut.energy + seeds.energy + fed.energy,
    }
}

fn close(x: f64, y: f64) {
    assert!((x - y).abs() < 1e-11, "{x} != {y}");
}

#[test]
fn grooming_contact_kills_after_duration_and_eats_conservatively() {
    let (world, mut flora, mut fauna, a, _, r) = scene(SP, SP, true);
    let prey = whole(&animal(&fauna, r));
    let stomach = animal(&fauna, a).stomach.total();
    let n = duration(&fauna);
    advance(&world, &mut flora, &mut fauna, n - 1);
    assert!(fauna.view().animal(r).is_some());
    advance(&world, &mut flora, &mut fauna, 1);
    assert!(fauna.view().animal(r).is_none());
    assert_eq!(fauna.ledger.departed_founder(LJ, Departure::Groomed), 1);
    assert_eq!(fauna.ledger.groomed_by_groomer[SP.index()], 1);
    assert_eq!(fauna.ledger.groomed_by_rider[LJ.index()], 1);
    let now = animal(&fauna, a).stomach.total();
    close(now.organic - stomach.organic, prey.organic);
    close(now.mineral - stomach.mineral, prey.mineral);
    close(now.energy - stomach.energy, prey.energy);
    assert_closes(&fauna, "groomed whole prey");
    let fv = flora.view();
    close(fv.organic(), fv.ledger.expected_organic());
    close(fv.mineral(), fv.ledger.expected_mineral());
    close(fv.energy(), fv.ledger.expected_energy());
}

#[test]
fn grooming_eats_its_rider_even_over_an_older_carcass() {
    let (world, mut flora, mut fauna, a, h, r) = scene(SP, SP, true);
    let prey = whole(&animal(&fauna, r));
    let site = animal(&fauna, h).site;
    assert!(flora.deposit(
        site,
        cubarium_voxel_flora::Deposit {
            kind: cubarium_voxel_flora::DepositKind::Carrion,
            organic: 1.0,
            mineral: 2.0,
            energy: 17.0,
        }
    ));
    let before = animal(&fauna, a).stomach.total();
    let n = duration(&fauna);
    advance(&world, &mut flora, &mut fauna, n);
    let fed = animal(&fauna, a).stomach.total();
    close(fed.organic - before.organic, prey.organic);
    close(fed.mineral - before.mineral, prey.mineral);
    close(fed.energy - before.energy, prey.energy);
    let fv = flora.view();
    let old = fv.ground_at(site).unwrap();
    close(old.carrion, 1.0);
    close(old.carrion_mineral, 2.0);
    close(old.carrion_energy, 17.0);
    assert_closes(&fauna, "fresh groomed prey above old carrion");
}

#[test]
fn grooming_snapshot_keeps_partial_contact() {
    let (world, mut flora, mut fauna, _, _, r) = scene(SP, SP, true);
    let n = duration(&fauna);
    advance(&world, &mut flora, &mut fauna, n - 1);
    let mut loaded = Fauna::load(&fauna.save()).unwrap();
    assert!(loaded.view().animal(r).is_some());
    advance(&world, &mut flora, &mut loaded, 1);
    assert!(loaded.view().animal(r).is_none());
    assert_eq!(loaded.ledger.departed_founder(LJ, Departure::Groomed), 1);
    assert_closes(&loaded, "saved grooming contact");
}

#[test]
fn grooming_overflow_uses_the_existing_carcass_pool() {
    let (world, mut flora, mut fauna, a, h, r) = scene(SP, SP, true);
    let prey = whole(&animal(&fauna, r));
    fauna.config.founders[SP.index()].gut.stomach_k = 0.01;
    let room = fauna
        .config
        .founder(SP)
        .gut
        .capacity(animal(&fauna, a).body);
    let n = duration(&fauna);
    advance(&world, &mut flora, &mut fauna, n);
    close(animal(&fauna, a).stomach.organic(), room.min(prey.organic));
    let fv = flora.view();
    let corpse = fv.ground_at(animal(&fauna, h).site).unwrap();
    close(
        animal(&fauna, a).stomach.total().organic + corpse.carrion,
        prey.organic,
    );
    close(
        animal(&fauna, a).stomach.total().mineral + corpse.carrion_mineral,
        prey.mineral,
    );
    close(
        animal(&fauna, a).stomach.total().energy + corpse.carrion_energy,
        prey.energy,
    );
    assert_closes(&fauna, "groomed overflow");
}

#[test]
fn grooming_stops_the_hosts_rider_drain() {
    let (world, mut flora, mut fauna, _, h, _) = scene(SP, SP, true);
    let before = animal(&fauna, h).stored_organic();
    super::drain_riders(&mut fauna);
    assert!(animal(&fauna, h).stored_organic() < before);
    let n = duration(&fauna);
    advance(&world, &mut flora, &mut fauna, n);
    let before = animal(&fauna, h).stored_organic();
    let drained = fauna.ledger.parasite_taken_organic;
    super::drain_riders(&mut fauna);
    assert_eq!(animal(&fauna, h).stored_organic(), before);
    assert_eq!(fauna.ledger.parasite_taken_organic, drained);
    assert_closes(&fauna, "no rider drain after grooming");
}

#[test]
fn grooming_rejects_other_lineages_and_self() {
    for self_host in [false, true] {
        let (world, mut flora, mut fauna, a, _, r) = scene(SP, Founder::Browser, true);
        if self_host {
            animal_mut(&mut fauna, r).attached = Some(Attachment { host: a });
        }
        let before = fauna.ledger;
        let n = duration(&fauna);
        advance(&world, &mut flora, &mut fauna, 2 * n);
        assert!(fauna.view().animal(r).is_some());
        assert_eq!(fauna.ledger, before);
    }
}

#[test]
fn grooming_interruptions_reset_continuous_contact() {
    for interruption in [
        "distance",
        "host_distance",
        "height",
        "sleep",
        "flee",
        "ride",
        "strike",
    ] {
        let (world, mut flora, mut fauna, a, h, r) = scene(SP, SP, true);
        let n = duration(&fauna);
        advance(&world, &mut flora, &mut fauna, n - 1);
        let saved = animal(&fauna, a);
        let host = animal(&fauna, h);
        match interruption {
            "distance" => animal_mut(&mut fauna, a).pose.x += 2.0,
            "host_distance" => animal_mut(&mut fauna, h).pose.x += 2.0,
            "height" => animal_mut(&mut fauna, h).pose.y_m += 2.0,
            "sleep" => animal_mut(&mut fauna, a).state = State::Asleep,
            "flee" => {
                fauna.steer.insert(a, crate::Steer::Flee);
            }
            "ride" => animal_mut(&mut fauna, a).attached = Some(Attachment { host: h }),
            "strike" => {
                animal_mut(&mut fauna, a).strike = Some(crate::Strike {
                    prey: r,
                    phase: crate::StrikePhase::Windup,
                    ticks_left: 10,
                    from_x: 2.0,
                    from_z: LANE_Z,
                    lure: false,
                })
            }
            _ => unreachable!(),
        }
        advance(&world, &mut flora, &mut fauna, 1);
        assert!(fauna.view().animal(r).is_some(), "{interruption}");
        *animal_mut(&mut fauna, a) = saved;
        *animal_mut(&mut fauna, h) = host;
        fauna.steer.remove(&a);
        advance(&world, &mut flora, &mut fauna, n - 1);
        assert!(
            fauna.view().animal(r).is_some(),
            "{interruption} reset progress"
        );
        advance(&world, &mut flora, &mut fauna, 1);
        assert!(
            fauna.view().animal(r).is_none(),
            "{interruption} fresh bout completes"
        );
    }
}

#[test]
fn grooming_seeks_a_seen_troopmate_and_respects_emergency_feeding() {
    let (world, mut flora, mut fauna, a, h, _) = scene(SP, SP, true);
    place(&mut fauna, h, 3.5, LANE_Z, EAST);
    animal_mut(&mut fauna, h).pose.y_m = SURFACE_M;
    crate::perception::sense_pass(&mut fauna, &world.view(), false, 1.0, 1);
    let i = fauna.animals.iter().position(|b| b.id == a).unwrap();
    let go = crate::groom::steer(&mut fauna, i, &world.view(), &flora.view(), true)
        .expect("seen infested kin draws grooming");
    assert!(go.forward > 0.0);
    assert!(go.turn.abs() < 0.5);
    animal_mut(&mut fauna, a).emergency = true;
    assert_eq!(
        crate::groom::steer(&mut fauna, i, &world.view(), &flora.view(), true),
        None
    );
    animal_mut(&mut fauna, a).emergency = false;
    animal_mut(&mut fauna, a).bout = Some(crate::Bout {
        start_tick: fauna.tick,
        kind: crate::BoutKind::Emergency,
    });
    assert_eq!(
        crate::groom::steer(&mut fauna, i, &world.view(), &flora.view(), true),
        None
    );
    animal_mut(&mut fauna, a).bout = None;
    let before = animal(&fauna, a).pose.x;
    for _ in 0..20 {
        fauna.step_with(&world, &mut flora, 1);
    }
    assert!(
        animal(&fauna, a).pose.x > before,
        "normal movement approaches the host: {:?}, {:?}",
        animal(&fauna, a),
        fauna.steer_of(a)
    );
}

#[test]
fn grooming_off_and_inapplicable_tables_leave_the_step_unchanged() {
    let (w, mut fl_on, mut on, _, _, _) = scene(SP, Founder::Browser, true);
    let (_, mut fl_off, mut off, _, _, _) = scene(SP, Founder::Browser, false);
    for _ in 0..10 {
        on.step_with(&w, &mut fl_on, 1);
        off.step_with(&w, &mut fl_off, 1);
    }
    assert_eq!(on.view().animals, off.view().animals);
    assert_eq!(on.ledger, off.ledger);
    for f in Founder::ALL.into_iter().filter(|f| *f != SP) {
        let (w, mut fl, mut fauna, _, _, r) = scene(f, f, true);
        let before = fauna.ledger;
        advance(&w, &mut fl, &mut fauna, 20);
        assert!(fauna.view().animal(r).is_some());
        assert_eq!(fauna.ledger, before);
    }
    let (w, mut fl, mut fauna, _, _, r) = scene(SP, SP, false);
    advance(&w, &mut fl, &mut fauna, 20);
    assert!(fauna.view().animal(r).is_some());
    assert_eq!(fauna.ledger.groomed_by_groomer[SP.index()], 0);
}
