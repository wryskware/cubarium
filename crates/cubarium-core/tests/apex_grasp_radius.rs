//! Workstream U: **the grasp-only apex turn radius**, an opt-in `World` transient under the
//! shipped `Sweep` motor
//! (`design/handoffs/ecology-v1-apex-grasp-opus-2026-09-16.md`).
//!
//! Written from the brief's definitions before the implementation. The rule is one variable:
//! under `MotorModel::Sweep`, is an apex member's 14.8 px grasp a **turn radius** as well as a
//! reach, or is its turn radius its own 9 px lobe extent? Nothing else about `Sweep` moves —
//! the additive envelope, `ROTATION_COST_SCALE` and the grasp's reach in the strike are
//! untouched — and **no ordinary body is reachable by it**, because only a hunter member is
//! ever handed contact geometry at all.
//!
//! The pinned hashes below were printed by commit 2eb8a9f, the build before `MotorModel` and
//! therefore before this switch existed, on the fixture workstream T used; this file rebuilds
//! that fixture verbatim and reproduced them at the brief commit a0a5957 **before** any of the
//! implementation was written.

use cubarium_core::hunter::{ContactGeometry, FixedHunterProfile, HunterTarget};
use cubarium_core::ids::OrganismId;
use cubarium_core::motor::{self, ApexTurnRadius, MotorBill, MotorLimits, MotorModel, MotorRequest};
use cubarium_core::neural::Policy;
use cubarium_core::neural::gru::{Gru32, HIDDEN, INPUT, N, R, Z};
use cubarium_core::neural::obs::FOOD_NEAR;
use cubarium_core::snapshot::state_hash;
use cubarium_core::{DT, World, WorldConfig};
use cubarium_surface::Vec2;

const EAST: Vec2 = Vec2 { x: 1.0, y: 0.0 };

fn remembering_policy() -> Gru32 {
    let mut w = Gru32::zeros();
    w.b_i[R] = 4.0;
    w.b_i[Z] = 2.0;
    w.w_i[N * INPUT + FOOD_NEAR] = 6.0;
    w.w_h[N * HIDDEN] = 1.0;
    w.b_i[N] = -2.0;
    w.w_o[0] = 5.0;
    w.w_o[HIDDEN] = -5.0;
    w
}

/// Workstream T's paired fixture, rebuilt here verbatim so the hashes below are comparable to
/// the ones commit 2eb8a9f printed: a default world run 1,000 ticks, two apex adults
/// introduced, and eight ordinary bodies dispatched to a recurrent policy that thrusts and
/// turns, so every motor path the switch could touch is live in it.
fn paired_world(rule: Option<ApexTurnRadius>) -> World {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    let profile = FixedHunterProfile::lanternjaw_trial(world.config());
    for _ in 0..1_000 {
        world.step();
        world.drain_events();
    }
    world
        .introduce_hunters(
            profile,
            &[
                HunterTarget { face: 1, u: 23.0, v: 31.0 },
                HunterTarget { face: 4, u: 55.0, v: 3.0 },
            ],
        )
        .expect("two adults are placed");
    let ids: Vec<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();
    let mut attached = 0;
    for id in ids {
        if attached >= 8 {
            break;
        }
        if world.attach_neural_policy(id, Policy::new(remembering_policy())).is_ok() {
            attached += 1;
        }
    }
    assert_eq!(attached, 8, "the fixture needs eight neural animals");
    if let Some(rule) = rule {
        world.set_apex_turn_radius(rule);
    }
    world
}

// ------------------------------------------------------------------ the rule's own arithmetic

/// The lanternjaw as the trial builds it: a world holding one apex member, its contact
/// geometry, its lobe extent and its grasp reach.
fn apex_member() -> (World, ContactGeometry, f64, f64) {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    let profile = FixedHunterProfile::lanternjaw_trial(world.config());
    let hunter = world
        .start_hunter_trial(profile.clone(), HunterTarget { face: 4, u: 32.0, v: 32.0 })
        .expect("started")
        .id;
    let o = world.state.organisms.get(hunter).expect("alive");
    let g = ContactGeometry::of(&profile, o);
    let lobes = o.phenotype.extent;
    let grasp = g.capture_offset_body.length() + g.capture_reach_px;
    (world, g, lobes, grasp)
}

/// The rule names itself, refuses a name it does not know, and the shipped rule is the default
/// everywhere it can be read.
#[test]
fn the_rule_names_itself_and_the_shipped_grasp_is_the_default() {
    assert_eq!(ApexTurnRadius::default(), ApexTurnRadius::Grasp);
    assert_eq!(ApexTurnRadius::Grasp.name(), "grasp");
    assert_eq!(ApexTurnRadius::Lobes.name(), "lobes");
    for r in [ApexTurnRadius::Grasp, ApexTurnRadius::Lobes] {
        assert_eq!(ApexTurnRadius::parse(r.name()).expect("its own name parses"), r);
    }
    assert!(
        ApexTurnRadius::parse("claws").is_err(),
        "an unknown rule is refused, not defaulted to the shipped one"
    );
    assert!(ApexTurnRadius::Grasp.is_grasp());
    assert!(!ApexTurnRadius::Lobes.is_grasp());
    let world = World::new(WorldConfig::default()).expect("valid");
    assert_eq!(world.apex_turn_radius(), ApexTurnRadius::Grasp);
}

/// **The deliverable's first definition.** Under the switch an apex member's turn radius is its
/// lobe extent; under the shipped rule it is the larger of the lobes and the grasp. The two
/// numbers the brief names — 9 px of lobes, 14.8 px of grasp — are re-derived from the profile
/// here rather than written down.
#[test]
fn the_switch_gives_an_apex_member_its_own_lobe_extent() {
    let (world, g, lobes, grasp) = apex_member();
    let o = world
        .state
        .organisms
        .iter()
        .find(|(id, _)| world.state.hunters.contains(*id))
        .map(|(_, o)| o)
        .expect("the member is alive");

    assert!((lobes - 9.0).abs() < 0.5, "the brief's 9 px lobes: {lobes}");
    assert!((grasp - 14.8).abs() < 0.2, "the brief's 14.8 px grasp: {grasp}");
    assert!(grasp > lobes, "the grasp reaches past the lobes, which is why the rule exists");

    let shipped =
        motor::turn_radius_px_in_with(o, Some(&g), MotorModel::Sweep, ApexTurnRadius::Grasp);
    let switched =
        motor::turn_radius_px_in_with(o, Some(&g), MotorModel::Sweep, ApexTurnRadius::Lobes);
    assert!((shipped - lobes.max(grasp)).abs() < 1e-12, "the shipped rule is max(lobes, grasp)");
    assert!((switched - lobes).abs() < 1e-12, "the switch is the lobe extent exactly");
    assert_eq!(
        shipped,
        motor::turn_radius_px_in(o, Some(&g), MotorModel::Sweep),
        "the three-argument entry point is the shipped rule"
    );
    assert_eq!(shipped, motor::turn_radius_px(o, Some(&g)), "and so is the two-argument one");
}

/// **The deliverable's second definition.** An ordinary body's radius is unchanged, by
/// construction: it has no contact geometry, so both rules return its lobe extent, at every
/// body size in a live world and under both motor contracts.
#[test]
fn an_ordinary_bodys_radius_is_untouched_by_the_rule() {
    let world = World::new(WorldConfig::default()).expect("valid");
    let ordinary: Vec<_> = world
        .state
        .organisms
        .iter()
        .filter(|(id, _)| !world.state.hunters.contains(*id))
        .map(|(_, o)| o)
        .collect();
    assert!(ordinary.len() >= 8, "the default world has ordinary bodies to check");
    for o in ordinary {
        for model in [MotorModel::Sweep, MotorModel::Inertial] {
            let grasp = motor::turn_radius_px_in_with(o, None, model, ApexTurnRadius::Grasp);
            let lobes = motor::turn_radius_px_in_with(o, None, model, ApexTurnRadius::Lobes);
            assert_eq!(grasp, lobes, "the rule moved an ordinary body under {model:?}");
            assert_eq!(
                grasp,
                motor::turn_radius_px_in(o, None, model),
                "and it is the shipped number"
            );
        }
    }
}

/// **The deliverable's third definition.** Under the switch the number the observation helper
/// reports, the number the envelope bounds and the number the bill prices are one number for
/// the apex. This is arithmetic on `turn_radius_px_in_with`, not a correctness claim: the
/// observation helper (`neural_observation`, `neural_decision`) serves neural animals only and
/// no apex member consumes it (Astra, round-4 addendum P1), so under the shipped rule the
/// apex is not "told" one radius and "bounded" by another; `Sweep` charging the outermost
/// contacting point is internally consistent with that model.
#[test]
fn the_observation_the_envelope_and_the_bill_read_the_same_number() {
    let (world, g, lobes, grasp) = apex_member();
    let o = world
        .state
        .organisms
        .iter()
        .find(|(id, _)| world.state.hunters.contains(*id))
        .map(|(_, o)| o)
        .expect("alive");

    // What `world::view::neural_observation` and `world::step::neural_decision` read.
    let observed = motor::turn_radius_px_in(o, None, MotorModel::Sweep);
    // What the envelope in `world::step` uses, under each rule.
    let envelope_shipped =
        motor::turn_radius_px_in_with(o, Some(&g), MotorModel::Sweep, ApexTurnRadius::Grasp);
    let envelope_switched =
        motor::turn_radius_px_in_with(o, Some(&g), MotorModel::Sweep, ApexTurnRadius::Lobes);
    assert_eq!(observed, lobes);
    assert_ne!(envelope_shipped, observed, "the shipped rule is exactly that disagreement");
    assert_eq!(envelope_switched, observed, "under the switch they agree");

    // And the bill, which prices `k · r · |ω|` at whatever radius the envelope resolved with.
    let cfg = WorldConfig::default();
    let bill = MotorBill::of(o, &cfg);
    let omega = 0.05;
    for (rule, expected_radius) in
        [(ApexTurnRadius::Grasp, envelope_shipped), (ApexTurnRadius::Lobes, envelope_switched)]
    {
        let radius_px = motor::turn_radius_px_in_with(o, Some(&g), MotorModel::Sweep, rule);
        assert_eq!(radius_px, expected_radius);
        let limits = MotorLimits {
            radius_px,
            turn_rate_max: omega,
            speed_cap: 100.0,
            motor_budget: 1e9,
            dt: DT,
        };
        // A pure pivot: the resolved sweep is `radius · |ω|` at the rule's own radius, and the
        // bill prices that same sweep.
        let request = MotorRequest { heading: Vec2 { x: -1.0, y: 0.0 }, speed: 0.0 };
        let m = motor::resolve_in(EAST, &request, &limits, MotorModel::Sweep);
        assert!((m.sweep - radius_px * omega).abs() < 1e-9, "{rule:?}: sweep is r·|ω|");
        let charged = bill.motor_cost(m.speed, m.sweep, DT);
        let expected =
            bill.move_cost * bill.structure * (m.speed + motor::ROTATION_COST_SCALE * m.sweep) * DT;
        assert!((charged - expected).abs() < 1e-15, "{rule:?}: the bill prices the same sweep");
    }
    assert!(grasp / lobes > 1.6, "the ratio the pivot test is stated on: {}", grasp / lobes);
}

/// What the apex actually gains: `grasp / lobes` times the pivot rate for the same budget —
/// and, unlike the inertial model, a cheaper radian too, because `Sweep`'s price per radian is
/// `k · r` and `r` is what fell.
#[test]
fn the_apex_pivots_the_grasp_ratio_faster_and_pays_less_per_radian() {
    let (_w, _g, lobes, grasp) = apex_member();
    let ratio = grasp / lobes;
    assert!((1.6..1.7).contains(&ratio), "the geometry says about 1.647: {ratio}");

    // The pivot at the same budget, from the resolver rather than from the algebra.
    let budget = 0.9;
    let mut rates = Vec::new();
    for r in [grasp, lobes] {
        let limits = MotorLimits {
            radius_px: r,
            turn_rate_max: 10.0,
            speed_cap: budget,
            motor_budget: 1e9,
            dt: DT,
        };
        let request = MotorRequest { heading: Vec2 { x: -1.0, y: 0.0 }, speed: 0.0 };
        let m = motor::resolve_in(EAST, &request, &limits, MotorModel::Sweep);
        rates.push(m.sweep / r);
    }
    assert!(
        (rates[1] / rates[0] - ratio).abs() < 1e-9,
        "the switch buys {} times the pivot rate, not {ratio}",
        rates[1] / rates[0]
    );
    // Price per radian is `k · r`, so it falls by the same ratio. (Under `Inertial` the apex's
    // radian kept 0.859 of its price; here it keeps 0.607.)
    let per_radian = |r: f64| motor::ROTATION_COST_SCALE * r;
    assert!(
        (per_radian(lobes) / per_radian(grasp) - lobes / grasp).abs() < 1e-12,
        "the radian's price falls with the radius"
    );
    assert!(per_radian(lobes) < per_radian(grasp));
}

/// The rule is a `Sweep` rule and cannot matter under `Inertial`, which already ignores the
/// grasp: the same radius of gyration under both.
#[test]
fn the_rule_cannot_move_the_inertial_model() {
    let (world, g, lobes, _) = apex_member();
    let o = world
        .state
        .organisms
        .iter()
        .find(|(id, _)| world.state.hunters.contains(*id))
        .map(|(_, o)| o)
        .expect("alive");
    let a = motor::turn_radius_px_in_with(o, Some(&g), MotorModel::Inertial, ApexTurnRadius::Grasp);
    let b = motor::turn_radius_px_in_with(o, Some(&g), MotorModel::Inertial, ApexTurnRadius::Lobes);
    assert_eq!(a, b);
    assert!((a - lobes / std::f64::consts::SQRT_2).abs() < 1e-12);
}

// ------------------------------------------------------------------ the switch in the world

/// The pinned hashes: six 1,500-tick boundaries of the paired world, printed by commit 2eb8a9f,
/// the build before `MotorModel` — and therefore before this switch — existed.
const SWEEP_HASHES: [(u64, u64); 6] = [
    (1_500, 0x1dd0_4659_c55d_2980),
    (3_000, 0x56a0_f4a8_11c9_964d),
    (4_500, 0xbc76_70f1_377f_24d9),
    (6_000, 0x61e6_d61a_4bc3_5fe4),
    (7_500, 0x15ce_4554_a89f_bae3),
    (9_000, 0xfa65_65de_e6ae_2dd3),
];

/// **Default off is byte-identical.** A world that never names a rule and one that names the
/// shipped `Grasp` must both reproduce the hashes the build before the switch printed, at every
/// 1,500-tick boundary of 9,000 ticks of a two-apex world.
#[test]
fn the_shipped_rule_is_byte_identical_to_the_build_that_never_heard_of_the_switch() {
    let mut untouched = paired_world(None);
    let mut named = paired_world(Some(ApexTurnRadius::Grasp));
    assert_eq!(untouched.apex_turn_radius(), ApexTurnRadius::Grasp);
    assert_eq!(named.apex_turn_radius(), ApexTurnRadius::Grasp);
    let mut pinned = SWEEP_HASHES.iter();
    let mut next = pinned.next();
    for tick in 1..=9_000u64 {
        untouched.step();
        named.step();
        untouched.drain_events();
        named.drain_events();
        untouched.drain_hunter_events();
        named.drain_hunter_events();
        if let Some((at, hash)) = next
            && tick == *at
        {
            assert_eq!(
                state_hash(&untouched.state),
                *hash,
                "the default diverged from commit 2eb8a9f at tick {tick}"
            );
            assert_eq!(
                state_hash(&named.state),
                *hash,
                "naming the shipped rule diverged from commit 2eb8a9f at tick {tick}"
            );
            next = pinned.next();
        }
    }
    assert!(next.is_none(), "every pinned boundary was checked");
    assert!(
        !untouched.state.hunters.members.is_empty(),
        "the run is only evidence if an apex was alive in it"
    );
}

/// And the switch is not vacuous: the same world, the same seed, the same introductions, run
/// with the grasp dropped from the turn budget, is a different world — and still closes its
/// books.
#[test]
fn the_switch_moves_the_same_two_apex_world() {
    let mut shipped = paired_world(Some(ApexTurnRadius::Grasp));
    let mut switched = paired_world(Some(ApexTurnRadius::Lobes));
    assert_eq!(switched.apex_turn_radius(), ApexTurnRadius::Lobes);
    let mut diverged_at = None;
    for tick in 1..=2_000u64 {
        shipped.step();
        switched.step();
        shipped.drain_events();
        switched.drain_events();
        shipped.drain_hunter_events();
        switched.drain_hunter_events();
        if diverged_at.is_none() && state_hash(&shipped.state) != state_hash(&switched.state) {
            diverged_at = Some(tick);
        }
    }
    assert!(diverged_at.is_some(), "the switch changed nothing, so it is not a switch");
    shipped.check_invariants().expect("the shipped world still closes its books");
    switched.check_invariants().expect("the switched world still closes its books");
}

/// A world with **no apex at all** is byte-identical under both rules: the rule reaches nothing
/// but a hunter member, and this measures that rather than asserting it.
#[test]
fn a_world_without_an_apex_is_identical_under_both_rules() {
    let mut shipped = World::new(WorldConfig::default()).expect("valid");
    let mut switched = World::new(WorldConfig::default()).expect("valid");
    switched.set_apex_turn_radius(ApexTurnRadius::Lobes);
    assert!(shipped.state.hunters.members.is_empty());
    for tick in 1..=3_000u64 {
        shipped.step();
        switched.step();
        shipped.drain_events();
        switched.drain_events();
        assert_eq!(
            state_hash(&shipped.state),
            state_hash(&switched.state),
            "the rule moved a world with no apex in it, at tick {tick}"
        );
    }
    assert!(
        shipped.state.organisms.iter().count() > 0,
        "the run is evidence only if bodies lived through it"
    );
}

/// The switch is a transient: it survives no snapshot and is not part of the world's state.
#[test]
fn the_switch_is_a_transient_and_is_not_persisted() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    world.set_apex_turn_radius(ApexTurnRadius::Lobes);
    for _ in 0..20 {
        world.step();
        world.drain_events();
    }
    let bytes = cubarium_core::encode_snapshot(&world.state, "apex-grasp-test");
    let (_, state) = cubarium_core::decode_snapshot(&bytes).expect("decodes");
    let resumed = World::from_state(state).expect("resumes");
    assert_eq!(
        resumed.apex_turn_radius(),
        ApexTurnRadius::Grasp,
        "a resumed world runs the shipped rule until it is told otherwise"
    );
    assert_eq!(resumed.motor_model(), MotorModel::Sweep, "and the shipped motor contract");
}
