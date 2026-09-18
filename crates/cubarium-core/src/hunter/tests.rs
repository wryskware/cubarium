use cubarium_surface::Vec2;

use crate::config::WorldConfig;
use crate::ids::OrganismId;

use super::*;

use crate::DT;

fn profile() -> FixedHunterProfile {
    FixedHunterProfile::lanternjaw_trial(&WorldConfig::default())
}

/// The default world's metabolic constants: `e_r = 2`, `eta_m = 0.6`, `eta_e = 0.5`.
fn metabolism() -> Metabolism {
    Metabolism::of(&WorldConfig::default())
}

/// The founder's own movement bill, spelled out rather than read off an organism.
fn bill() -> MoveBill {
    MoveBill {
        structure: 1.0,
        maintenance: 0.005,
        sense_radius: 6.0,
        move_cost: 0.006,
        sense_cost: 0.0002,
    }
}

/// One way to make a valid profile invalid.
type Break = fn(&mut FixedHunterProfile);

#[test]
fn the_trial_profile_is_valid_and_its_variants_stay_valid() {
    let p = profile();
    p.validate().expect("the trial profile is valid");
    p.clone().facultative().validate().expect("facultative");
    p.clone()
        .without_attacks()
        .validate()
        .expect("attack-disabled");
    assert_eq!(p.role.as_str(), "lanternjaw");
    assert_eq!(p.version, PROFILE_VERSION);
}

/// The trial's effectors are Fable's authored ones, to the bit: the same expressions
/// `Lanternjaw::effectors(1.0)` evaluates, recomputed here rather than copied as decimals.
/// If the art moves a claw, this fails and the constant follows it — with a new
/// [`PROFILE_VERSION`], never a silent re-measurement of a frozen trial.
#[test]
fn the_capture_effector_is_the_authored_near_claw_and_the_mouth_is_the_authored_jaw() {
    let head_dx = -0.3 * (1.0 - 13.0 / 17.0) + 1.1 * ((13.0 - 9.0) / 8.0f64).clamp(0.0, 1.0);
    let near = Vec2::new(12.3 + head_dx + 0.5, 0.6 + 0.5);
    assert_eq!(CAPTURE_OFFSET_BODY, near);
    assert_eq!(
        CAPTURE_OFFSET_BODY.x, 13.279_411_764_705_882,
        "the documented x"
    );
    assert_eq!(
        CAPTURE_OFFSET_BODY.y, 1.1,
        "the authored y, not the decorated study's 1.162368"
    );
    assert_eq!(INGESTION_OFFSET_BODY, Vec2::new(9.6, 0.0));
    assert_ne!(
        CAPTURE_OFFSET_BODY, INGESTION_OFFSET_BODY,
        "grasp and mouth are not one point"
    );

    let p = profile();
    assert_eq!(p.capture_offset_body, CAPTURE_OFFSET_BODY);
    assert_eq!(p.ingestion_offset_body, INGESTION_OFFSET_BODY);
    // The version moved with the constant, so a saved version 2 trial cannot be re-read as
    // if it had always meant this.
    assert_eq!(PROFILE_VERSION, 3);
    let mut old = p.clone();
    old.version = 2;
    let err = old
        .validate()
        .expect_err("a version 2 trial must be refused");
    assert!(err.contains("version 2 is not one of [3, 4]"), "{err}");
}

/// The smallest admitted scale and a below-art-minimum juvenile both produce a coherent,
/// finite geometry: core admits `body_scale_min = 0.2`, while Fable's renderer currently
/// admits `0.5..=1.0` and is extending it — the two ranges are reconciled by the art
/// worker, never by clamping the world's contact geometry
/// (`astra-hunter-geometry-review-2026-09-13.md`).
#[test]
fn the_smallest_admitted_scale_still_produces_a_coherent_geometry() {
    let p = profile();
    let adult = 2.0;
    // The review's own fixture: `child_structure_fraction = 0.1` of a 2.0 adult.
    let small = body_scale(&p, 0.2, adult);
    assert!((small - 0.316_227_766_016_837_94).abs() < 1e-15, "{small}");
    assert!(
        small < 0.5,
        "this fixture is below the renderer's current minimum on purpose"
    );
    // The floor holds, and nothing below it is ever published.
    assert_eq!(body_scale(&p, 1e-9, adult), p.body_scale_min);
    assert_eq!(body_scale(&p, 0.0, adult), p.body_scale_min);
    assert_eq!(body_scale(&p, adult, adult), 1.0, "an adult is scale 1");
    for structure in [0.2, 0.4, 0.8, 1.6, adult] {
        let scale = body_scale(&p, structure, adult);
        assert!(
            scale.is_finite() && (p.body_scale_min..=1.0).contains(&scale),
            "{structure}: {scale}"
        );
    }
}

#[test]
fn an_invalid_profile_is_rejected_one_property_at_a_time() {
    let cases: [(&str, Break); 13] = [
        // Version 1 is the schema 10 profile shape and version 2 the earlier decorated
        // effector: both are refused, never reinterpreted.
        ("version", |p| p.version = 1),
        ("version", |p| p.version = 2),
        ("gut_capacity_material", |p| p.gut_capacity_material = 0.0),
        ("digest_rate", |p| p.digest_rate = f64::NAN),
        ("strike_energy_cost", |p| p.strike_energy_cost = -1.0),
        ("capture_min", |p| p.capture_min = 0.9),
        ("seek_reserve_fraction", |p| p.seek_reserve_fraction = 0.9),
        ("escape_speed_multiple", |p| p.escape_speed_multiple = 0.5),
        ("capture reach", |p| {
            p.capture_offset_body = Vec2::new(40.0, 0.0)
        }),
        ("query extent", |p| p.visual_query_extent_px = 1.0),
        ("body scale", |p| p.body_scale_min = 0.0),
        ("genome", |p| p.genome.size = 9.0),
        ("scavenge_fraction", |p| p.scavenge_fraction = 2.0),
    ];
    for (what, break_it) in cases {
        let mut p = profile();
        break_it(&mut p);
        let err = match p.validate() {
            Ok(()) => panic!("an invalid {what} must be rejected"),
            Err(e) => e,
        };
        assert!(!err.is_empty(), "{what} was rejected without saying why");
    }
}

#[test]
fn capture_probability_is_bounded_and_falls_with_prey_size() {
    let p = profile();
    let big = capture_probability(&p, 2.0, 0.2);
    let small = capture_probability(&p, 2.0, 1.5);
    assert!(big > small, "{big} vs {small}");
    for prey in [0.0, 0.01, 0.5, 1.5, 100.0] {
        let q = capture_probability(&p, 2.0, prey);
        assert!(
            (p.capture_min..=p.capture_max).contains(&q),
            "prey {prey} gave {q}"
        );
    }
    // The documented formula, exactly.
    assert_eq!(
        capture_probability(&p, 2.0, 1.0),
        (0.65 * 2.0 / 3.0f64).clamp(0.1, 0.75)
    );
}

#[test]
fn digestion_conserves_material_and_energy_exactly() {
    let p = profile();
    let m = metabolism();
    let e_r = m.e_r;
    for (gut_m, gut_q) in [(4.0, 7.0), (1.0, 0.1), (0.001, 0.002), (2.0, 12.0)] {
        let step = digest_step(&p, DT, gut_m, gut_q, m, 10.0, 10.0);
        assert!(step.material > 0.0, "no digestion of {gut_m}/{gut_q}");
        // Material: what leaves the gut is reserve plus rejected detritus.
        assert!((step.material - step.to_reserve - step.to_detritus).abs() < 1e-15);
        // Energy: what leaves the gut is stored in the reserve, gained, or heat.
        let booked = e_r * step.to_reserve + step.energy_gain + step.heat;
        assert!((step.carried - booked).abs() < 1e-12, "{:?}", step);
        assert!(step.heat >= 0.0 && step.to_detritus >= -1e-15, "{:?}", step);
    }
}

#[test]
fn energy_poor_prey_stores_proportionally_less_material() {
    let p = profile();
    let m = metabolism();
    let eta_m = m.eta_m;
    // Density 2.0 (as rich as reserve material) versus 0.5 (structure-heavy prey).
    let rich = digest_step(&p, DT, 4.0, 8.0, m, 10.0, 10.0);
    let poor = digest_step(&p, DT, 4.0, 2.0, m, 10.0, 10.0);
    assert_eq!(rich.material, poor.material, "the same portion is taken");
    assert!(
        (rich.to_reserve - eta_m * rich.material).abs() < 1e-15,
        "rich prey stores eta_m"
    );
    assert!(
        (poor.to_reserve - eta_m * 0.25 * poor.material).abs() < 1e-15,
        "poor prey stores rho/e_r of it"
    );
    assert!(poor.to_detritus > rich.to_detritus);
}

#[test]
fn reserve_headroom_reduces_the_portion_rather_than_the_assimilation() {
    let p = profile();
    let m = metabolism();
    let room = 0.0004;
    let step = digest_step(&p, DT, 4.0, 8.0, m, room, 10.0);
    assert!((step.to_reserve - room).abs() < 1e-15, "{:?}", step);
    // Still the same identity, on the reduced portion.
    assert!((step.carried - (2.0 * step.to_reserve + step.energy_gain + step.heat)).abs() < 1e-12);
    assert!(step.material < p.digest_rate * DT, "the portion shrank");
    // No headroom at all digests nothing and keeps the gut.
    assert_eq!(
        digest_step(&p, DT, 4.0, 8.0, m, 0.0, 10.0),
        DigestStep::default()
    );
    assert_eq!(
        digest_step(&p, DT, 0.0, 0.0, m, 1.0, 1.0),
        DigestStep::default()
    );
}

#[test]
fn a_full_battery_sends_the_spare_energy_to_heat() {
    let p = profile();
    let step = digest_step(&p, DT, 4.0, 8.0, metabolism(), 10.0, 0.0);
    assert_eq!(step.energy_gain, 0.0);
    assert!(step.heat > 0.0);
    assert!((step.carried - (2.0 * step.to_reserve + step.heat)).abs() < 1e-12);
}

#[test]
fn affordable_speed_never_slows_ordinary_movement_and_bounds_a_boost() {
    // A creature with plenty of energy gets the whole boost.
    let fast = bill().affordable_speed(10.0, DT, 0.3, 1.0);
    assert_eq!(fast, 1.0);
    // An empty battery still moves at its ordinary speed, never slower.
    let broke = bill().affordable_speed(0.0, DT, 0.3, 1.0);
    assert_eq!(broke, 0.3);
    // In between, the boost is exactly what the energy pays for, and no more.
    let energy = 0.0005;
    let some = bill().affordable_speed(energy, DT, 0.3, 5.0);
    let fixed = (0.005 * 1.0 + 0.0002 * 6.0) * DT;
    let budget = (energy - fixed) / (0.006 * 1.0 * DT);
    assert!(
        budget > 0.3 && budget < 5.0,
        "the budget must bind for this to prove anything: {budget}"
    );
    assert!((some - budget).abs() < 1e-12, "{some} vs {budget}");
    // A request at or below the ordinary speed is returned untouched.
    assert_eq!(bill().affordable_speed(0.0, DT, 0.3, 0.2), 0.2);
}

#[test]
fn member_bookkeeping_is_sorted_and_forgets_removed_ids() {
    let mut state = HunterState::default();
    assert!(!state.active());
    let a = OrganismId {
        slot: 5,
        generation: 1,
    };
    let b = OrganismId {
        slot: 2,
        generation: 3,
    };
    assert!(state.insert_member(HunterMember::new(a, 10)));
    assert!(state.insert_member(HunterMember::new(b, 10)));
    assert!(
        !state.insert_member(HunterMember::new(a, 10)),
        "a repeat is refused"
    );
    assert_eq!(
        state.members.iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![b, a]
    );
    assert!(state.contains(a) && state.contains(b));
    assert!(
        !state.contains(OrganismId {
            slot: 5,
            generation: 2
        }),
        "generation is part of the ID"
    );

    state.member_mut(b).expect("member").target = Some(a);
    let gone = state.remove_member(a, 11).expect("removed");
    assert_eq!(gone.id, a);
    assert_eq!(
        state.member(b).expect("member").target,
        None,
        "a removed hunter is forgotten"
    );
    state.member_mut(b).expect("member").target = Some(OrganismId {
        slot: 9,
        generation: 1,
    });
    let member = state.member_mut(b).expect("member");
    member.phase = HunterPhase::Stalking;
    member.phase_started_tick = 10;
    member.phase_ends_tick = 10;
    state.forget_target(
        OrganismId {
            slot: 9,
            generation: 1,
        },
        12,
    );
    let b = state.member(b).expect("member");
    assert_eq!(b.target, None);
    assert_eq!(
        b.phase,
        HunterPhase::Perched,
        "an unpaid stalk cannot outlive its prey"
    );
    assert_eq!(
        b.phase_started_tick, 12,
        "the perch begins at the removal boundary"
    );
    assert_eq!(b.entered_from, HunterPhase::Stalking);
}

#[test]
fn phase_bookkeeping_reports_progress_and_drops_targets() {
    let mut m = HunterMember::new(
        OrganismId {
            slot: 0,
            generation: 1,
        },
        100,
    );
    m.target = Some(OrganismId {
        slot: 1,
        generation: 1,
    });
    m.enter(HunterPhase::Windup, 100, 112, 0);
    assert_eq!(
        m.target,
        Some(OrganismId {
            slot: 1,
            generation: 1
        }),
        "a hunting phase keeps it"
    );
    assert_eq!(
        m.entered_from,
        HunterPhase::Perched,
        "the transition origin is recorded"
    );
    assert_eq!(m.progress(100), Some(0.0));
    assert_eq!(m.progress(106), Some(0.5));
    assert_eq!(m.progress(999), Some(1.0));
    m.enter(HunterPhase::Recovering, 112, 212, 7);
    assert_eq!(m.target, None, "a non-hunting phase drops the target");
    assert_eq!(
        m.entered_from,
        HunterPhase::Windup,
        "a recoil knows what it recoiled from"
    );
    assert_eq!(m.episode, 7, "and which attack episode it belongs to");
    m.enter(HunterPhase::Perched, 212, 212, 0);
    assert_eq!(m.progress(300), None);
    assert_eq!(m.entered_from, HunterPhase::Recovering);
    assert_eq!(m.episode, 0);
}

#[test]
fn a_target_resolves_only_inside_the_charts() {
    assert!(
        HunterTarget {
            face: 0,
            u: 1.0,
            v: 2.0
        }
        .resolve(cubarium_surface::Topology::Cube)
        .is_some()
    );
    for bad in [
        HunterTarget {
            face: 9,
            u: 1.0,
            v: 1.0,
        },
        HunterTarget {
            face: 0,
            u: -1.0,
            v: 1.0,
        },
        HunterTarget {
            face: 0,
            u: 64.0,
            v: 1.0,
        },
        HunterTarget {
            face: 0,
            u: f64::NAN,
            v: 1.0,
        },
    ] {
        assert!(
            bad.resolve(cubarium_surface::Topology::Cube).is_none(),
            "{bad:?} resolved"
        );
    }
}
