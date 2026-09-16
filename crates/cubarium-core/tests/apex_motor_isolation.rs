//! Workstream W: **the disc model on the apex alone**, an opt-in `World` transient that names
//! the motor contract a *hunter member* runs while every other body keeps the world's own
//! (`design/handoffs/ecology-v1-apex-motor-isolation-opus-2026-09-16.md`).
//!
//! Written from the brief's definitions before the implementation. The one variable is which
//! `MotorModel` a body **with apex contact geometry** runs; `None` means the world's own, and no
//! ordinary body is reachable by the override at all, because only a hunter member is ever handed
//! contact geometry. Under the override all four of the per-body motor reads in `world::step` —
//! the envelope radius, the resolver, the bill and the rotation price — must be the override's,
//! and every other body's must be the world's, arithmetic for arithmetic.
//!
//! Why the workstream exists: T's arm A ran `Inertial` on *every* body, so its prey world at
//! introduction was 628 against `Sweep`'s 745 and its apex gain is not apportioned between the
//! quadrature envelope and that thinner world. The override makes the prey world identical
//! between the arms by construction.
//!
//! The pinned hashes below are workstream T's, printed by commit 2eb8a9f — the build before
//! `MotorModel` and therefore before either switch existed — on the fixture U rebuilt verbatim;
//! this file reuses that fixture and it was run green at the brief commit c04348a **before** any
//! of this workstream's implementation was written.

use cubarium_core::BodyBudget;
use cubarium_core::diagnostic::ScriptedIntent;
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
const WEST: Vec2 = Vec2 { x: -1.0, y: 0.0 };

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

/// Workstream T's paired fixture, as U rebuilt it: a default world run 1,000 ticks, two apex
/// adults introduced, and eight ordinary bodies dispatched to a recurrent policy that thrusts and
/// turns, so every motor path the override could touch is live in it.
fn paired_world(world_model: MotorModel, apex_model: Option<Option<MotorModel>>) -> World {
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
    if world_model != MotorModel::Sweep {
        world.set_motor_model(world_model);
    }
    if let Some(over) = apex_model {
        world.set_apex_motor_model(over);
    }
    world
}

/// The member ids of a fixture world, in slot order.
fn members(world: &World) -> Vec<OrganismId> {
    let hunters = &world.state.hunters;
    world.state.organisms.iter().map(|(id, _)| id).filter(|id| hunters.contains(*id)).collect()
}

/// The ordinary (non-member) ids of a fixture world, in slot order.
fn ordinary(world: &World) -> Vec<OrganismId> {
    let hunters = &world.state.hunters;
    world.state.organisms.iter().map(|(id, _)| id).filter(|id| !hunters.contains(*id)).collect()
}

/// The lanternjaw as the trial builds it: a world holding one apex member and its contact
/// geometry, alongside the ordinary bodies that share the world with it.
fn apex_member() -> (World, ContactGeometry) {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    let profile = FixedHunterProfile::lanternjaw_trial(world.config());
    let hunter = world
        .start_hunter_trial(profile.clone(), HunterTarget { face: 4, u: 32.0, v: 32.0 })
        .expect("started")
        .id;
    let o = world.state.organisms.get(hunter).expect("alive");
    let g = ContactGeometry::of(&profile, o);
    (world, g)
}

// --------------------------------------------------------------- the override's own arithmetic

/// The transient names itself, and `None` — the default — is the world's own contract for every
/// body, member or not.
#[test]
fn the_default_override_is_none_and_none_is_the_worlds_own_contract() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    assert_eq!(world.apex_motor_model(), None, "the shipped default is no override at all");
    world.set_apex_motor_model(Some(MotorModel::Inertial));
    assert_eq!(world.apex_motor_model(), Some(MotorModel::Inertial));
    world.set_apex_motor_model(Some(MotorModel::Sweep));
    assert_eq!(world.apex_motor_model(), Some(MotorModel::Sweep));
    world.set_apex_motor_model(None);
    assert_eq!(world.apex_motor_model(), None);

    // Independent of the other two transients of this experiment family.
    world.set_motor_model(MotorModel::Inertial);
    world.set_apex_turn_radius(ApexTurnRadius::Lobes);
    world.set_apex_motor_model(Some(MotorModel::Sweep));
    assert_eq!(world.motor_model(), MotorModel::Inertial);
    assert_eq!(world.apex_turn_radius(), ApexTurnRadius::Lobes);
    assert_eq!(world.apex_motor_model(), Some(MotorModel::Sweep));

    // `model_for_body` is the one place the choice is written, and with no override it is the
    // world's own contract whatever geometry a body has.
    let (_w, g) = apex_member();
    for world_model in [MotorModel::Sweep, MotorModel::Inertial] {
        assert_eq!(motor::model_for_body(world_model, None, Some(&g)), world_model);
        assert_eq!(motor::model_for_body(world_model, None, None), world_model);
    }
}

/// **The deliverable's first definition.** Under the override an apex member's envelope radius,
/// resolved motion, bill and rotation price are the override's, and an ordinary body's are the
/// world's. Each of the four is taken from the same entry point `world::step` takes it from.
#[test]
fn an_apex_members_four_reads_are_the_overrides_and_an_ordinary_bodys_are_the_worlds() {
    let (world, g) = apex_member();
    let member = world
        .state
        .organisms
        .iter()
        .find(|(id, _)| world.state.hunters.contains(*id))
        .map(|(_, o)| o)
        .expect("the member is alive");
    let other = world
        .state
        .organisms
        .iter()
        .find(|(id, _)| !world.state.hunters.contains(*id))
        .map(|(_, o)| o)
        .expect("the world has an ordinary body");
    let cfg = WorldConfig::default();

    // The world runs the shipped contract; the member is told to run the disc model.
    let world_model = MotorModel::Sweep;
    let over = Some(MotorModel::Inertial);

    let member_model = motor::model_for_body(world_model, over, Some(&g));
    let other_model = motor::model_for_body(world_model, over, None);
    assert_eq!(member_model, MotorModel::Inertial, "the member runs the override");
    assert_eq!(other_model, world_model, "an ordinary body runs the world's own contract");

    // 1. The envelope radius (`world::step` ≈ L1385), under the world's own apex turn rule.
    let radius_member =
        motor::turn_radius_px_in_with(member, Some(&g), member_model, ApexTurnRadius::Grasp);
    let radius_other = motor::turn_radius_px_in_with(other, None, other_model, ApexTurnRadius::Grasp);
    assert_eq!(
        radius_member,
        motor::turn_radius_px_in(member, Some(&g), MotorModel::Inertial),
        "the member's radius is the disc model's"
    );
    assert!(
        (radius_member - member.phenotype.extent / std::f64::consts::SQRT_2).abs() < 1e-12,
        "and that is the disc's radius of gyration, {radius_member}"
    );
    assert_ne!(
        radius_member,
        motor::turn_radius_px_in_with(member, Some(&g), world_model, ApexTurnRadius::Grasp),
        "the override is not the world's rule for the member"
    );
    assert_eq!(
        radius_other,
        motor::turn_radius_px_in(other, None, MotorModel::Sweep),
        "an ordinary body's radius is the world's"
    );

    // 2. The resolver (≈ L1403) and 3. the bill (≈ L1454), on a request that both travels and
    //    pivots, so neither term is vacuous.
    for (o, geom, model, expected) in [
        (member, Some(&g), member_model, MotorModel::Inertial),
        (other, None, other_model, MotorModel::Sweep),
    ] {
        let radius_px = motor::turn_radius_px_in_with(o, geom, model, ApexTurnRadius::Grasp);
        let limits =
            MotorLimits { radius_px, turn_rate_max: 1.0, speed_cap: 3.0, motor_budget: 1e9, dt: DT };
        let request = MotorRequest { heading: WEST, speed: 1.5 };
        let got = motor::resolve_in(EAST, &request, &limits, model);
        let want = motor::resolve_in(EAST, &request, &limits, expected);
        assert_eq!(got.speed, want.speed, "the resolver ran the wrong contract");
        assert_eq!(got.sweep, want.sweep, "the resolver ran the wrong contract");
        assert!(got.sweep > 0.0, "the case is only evidence if the body actually pivoted");

        let bill = MotorBill::of(o, &cfg);
        assert_eq!(
            bill.total_cost_in(got.speed, got.sweep, DT, model),
            bill.total_cost_in(got.speed, got.sweep, DT, expected),
            "the bill priced the wrong contract"
        );
        // 4. The rotation price (≈ L1472), the split the budget recorder books.
        assert_eq!(model.rotation_price(), expected.rotation_price());
    }
    assert!(
        (member_model.rotation_price() - 1.0).abs() < 1e-12,
        "the disc model prices a radian at one"
    );
    assert!(
        (other_model.rotation_price() - motor::ROTATION_COST_SCALE).abs() < 1e-12,
        "and the shipped sweep at ROTATION_COST_SCALE"
    );
}

/// The override is a *contract selector*, not a one-way flag: in an `Inertial` world a member
/// told to run `Sweep` gets the shipped envelope back while every other body keeps the disc.
#[test]
fn the_override_runs_either_contract_in_either_direction() {
    let (world, g) = apex_member();
    let member = world
        .state
        .organisms
        .iter()
        .find(|(id, _)| world.state.hunters.contains(*id))
        .map(|(_, o)| o)
        .expect("alive");
    let over = Some(MotorModel::Sweep);
    assert_eq!(motor::model_for_body(MotorModel::Inertial, over, Some(&g)), MotorModel::Sweep);
    assert_eq!(motor::model_for_body(MotorModel::Inertial, over, None), MotorModel::Inertial);
    let sweep_radius = motor::turn_radius_px_in_with(
        member,
        Some(&g),
        motor::model_for_body(MotorModel::Inertial, over, Some(&g)),
        ApexTurnRadius::Grasp,
    );
    assert_eq!(sweep_radius, motor::turn_radius_px(member, Some(&g)), "the grasp is back");
}

// ------------------------------------------------------------------- the override in the world

/// The pinned hashes: six 1,500-tick boundaries of the paired world, printed by commit 2eb8a9f,
/// the build before `MotorModel` — and therefore before any of these switches — existed.
const SWEEP_HASHES: [(u64, u64); 6] = [
    (1_500, 0x1dd0_4659_c55d_2980),
    (3_000, 0x56a0_f4a8_11c9_964d),
    (4_500, 0xbc76_70f1_377f_24d9),
    (6_000, 0x61e6_d61a_4bc3_5fe4),
    (7_500, 0x15ce_4554_a89f_bae3),
    (9_000, 0xfa65_65de_e6ae_2dd3),
];

/// **Default off is byte-identical.** A world that never names an override, one that names
/// `None`, and one that names the world's own `Sweep` for its member must all reproduce the
/// hashes the build before the switch printed, at every 1,500-tick boundary of 9,000 ticks of a
/// two-apex world.
#[test]
fn default_off_is_byte_identical_to_the_build_that_never_heard_of_the_override() {
    let mut untouched = paired_world(MotorModel::Sweep, None);
    let mut named_none = paired_world(MotorModel::Sweep, Some(None));
    let mut named_sweep = paired_world(MotorModel::Sweep, Some(Some(MotorModel::Sweep)));
    assert_eq!(untouched.apex_motor_model(), None);
    assert_eq!(named_none.apex_motor_model(), None);
    assert_eq!(named_sweep.apex_motor_model(), Some(MotorModel::Sweep));
    let mut pinned = SWEEP_HASHES.iter();
    let mut next = pinned.next();
    for tick in 1..=9_000u64 {
        for w in [&mut untouched, &mut named_none, &mut named_sweep] {
            w.step();
            w.drain_events();
            w.drain_hunter_events();
        }
        if let Some((at, hash)) = next
            && tick == *at
        {
            for (label, w) in [
                ("the default", &untouched),
                ("naming None", &named_none),
                ("naming the world's own Sweep", &named_sweep),
            ] {
                assert_eq!(
                    state_hash(&w.state),
                    *hash,
                    "{label} diverged from commit 2eb8a9f at tick {tick}"
                );
            }
            next = pinned.next();
        }
    }
    assert!(next.is_none(), "every pinned boundary was checked");
    assert!(
        !untouched.state.hunters.members.is_empty(),
        "the run is only evidence if an apex was alive in it"
    );
}

/// A world with the override set and **no apex ever introduced** is hash-identical to one without
/// it over 3,000 ticks: the override reaches nothing but a body with contact geometry, and this
/// measures that rather than asserting it.
#[test]
fn a_world_that_never_sees_an_apex_is_identical_under_the_override() {
    let mut plain = World::new(WorldConfig::default()).expect("valid");
    let mut overridden = World::new(WorldConfig::default()).expect("valid");
    overridden.set_apex_motor_model(Some(MotorModel::Inertial));
    assert!(plain.state.hunters.members.is_empty());
    for tick in 1..=3_000u64 {
        plain.step();
        overridden.step();
        plain.drain_events();
        overridden.drain_events();
        assert_eq!(
            state_hash(&plain.state),
            state_hash(&overridden.state),
            "the override moved a world with no apex in it, at tick {tick}"
        );
    }
    assert!(
        plain.state.organisms.iter().count() > 0,
        "the run is evidence only if bodies lived through it"
    );
    assert!(overridden.state.hunters.members.is_empty(), "and no apex was ever introduced");
}

/// And the override is not vacuous: the same world, the same seed, the same introductions, with
/// the member alone on the disc model, is a different world — and still closes its books.
#[test]
fn the_override_moves_the_same_two_apex_world() {
    let mut shipped = paired_world(MotorModel::Sweep, None);
    let mut overridden = paired_world(MotorModel::Sweep, Some(Some(MotorModel::Inertial)));
    let mut diverged_at = None;
    for tick in 1..=2_000u64 {
        shipped.step();
        overridden.step();
        shipped.drain_events();
        overridden.drain_events();
        shipped.drain_hunter_events();
        overridden.drain_hunter_events();
        if diverged_at.is_none() && state_hash(&shipped.state) != state_hash(&overridden.state) {
            diverged_at = Some(tick);
        }
    }
    assert!(diverged_at.is_some(), "the override changed nothing, so it is not an override");
    shipped.check_invariants().expect("the shipped world still closes its books");
    overridden.check_invariants().expect("the overridden world still closes its books");
}

/// One tick of three worlds that are byte-identical up to it, with the ledger opened for that
/// tick alone and every body scripted into a hard reversal so translation and rotation are both
/// live.
///
/// Because the three worlds enter the compared tick in the same state, a body's decision, its
/// limits and its bill in that tick depend on nothing but the contract it runs — so the record
/// this returns *is* the four reads, read where the world levies them.
fn one_scripted_tick(
    world_model: MotorModel,
    apex_model: Option<MotorModel>,
) -> (Vec<(OrganismId, BodyBudget)>, Vec<(OrganismId, BodyBudget)>) {
    let mut world = paired_world(MotorModel::Sweep, None);
    // Let the members settle into the world under the shipped contract, identically in all three
    // worlds, before anything is switched.
    for _ in 0..200 {
        world.step();
        world.drain_events();
        world.drain_hunter_events();
    }
    let ids: Vec<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();
    let scripts: Vec<_> = ids
        .iter()
        .map(|id| {
            let o = world.state.organisms.get(*id).expect("alive");
            let reversed = Vec2 { x: -o.heading.x, y: -o.heading.y };
            let intent = ScriptedIntent {
                heading: Some(reversed),
                effort: Some(1.0),
                bud: Some(false),
                ..ScriptedIntent::default()
            };
            (*id, intent)
        })
        .collect();
    world.set_scripted_intents(scripts);
    if world_model != MotorModel::Sweep {
        world.set_motor_model(world_model);
    }
    world.set_apex_motor_model(apex_model);
    // Recording starts at the next tick's opening stores, so the records below hold exactly the
    // one tick that follows.
    world.record_body_budgets(true);
    world.step();
    world.drain_events();
    world.drain_hunter_events();
    let member_ids = members(&world);
    let ordinary_ids = ordinary(&world);
    let take = |list: &[OrganismId]| -> Vec<(OrganismId, BodyBudget)> {
        list.iter().filter_map(|id| world.body_budget(*id).map(|b| (*id, *b))).collect()
    };
    (take(&member_ids), take(&ordinary_ids))
}

fn motor_terms(b: &BodyBudget) -> (f64, f64, f64) {
    (b.motor_translation_billed, b.motor_turn_billed, b.bill_total)
}

/// **The deliverable's second definition, measured in the world.** In the compared tick the
/// member's translation billed, turn billed and whole bill under `Sweep` + the `Inertial`
/// override are *exactly* the ones it has when the whole world runs `Inertial`, and are not the
/// ones it has under plain `Sweep`. Every ordinary body's are exactly plain `Sweep`'s, and not
/// the whole-world `Inertial` ones.
#[test]
fn the_member_runs_the_override_and_every_other_body_runs_the_worlds_own() {
    let (plain_m, plain_o) = one_scripted_tick(MotorModel::Sweep, None);
    let (over_m, over_o) = one_scripted_tick(MotorModel::Sweep, Some(MotorModel::Inertial));
    let (whole_m, whole_o) = one_scripted_tick(MotorModel::Inertial, None);

    assert_eq!(plain_m.len(), over_m.len(), "the same members are alive in the compared tick");
    assert_eq!(plain_m.len(), whole_m.len());
    assert!(!plain_m.is_empty(), "the tick is evidence only with a member in it");
    assert_eq!(plain_o.len(), over_o.len());
    assert_eq!(plain_o.len(), whole_o.len());
    assert!(plain_o.len() >= 8, "and with ordinary bodies to leave alone");

    let mut member_moved = false;
    for i in 0..plain_m.len() {
        let (id, plain) = &plain_m[i];
        let (over_id, over) = &over_m[i];
        let (whole_id, whole) = &whole_m[i];
        assert_eq!(id, over_id);
        assert_eq!(id, whole_id);
        assert_eq!(
            motor_terms(over),
            motor_terms(whole),
            "member {id:?} did not run the override's contract"
        );
        if motor_terms(plain) != motor_terms(whole) {
            member_moved = true;
        }
    }
    assert!(member_moved, "no member's bill moved at all, so the tick proves nothing");
    assert!(
        plain_m.iter().any(|(_, b)| b.motor_turn_billed > 0.0),
        "the compared tick must have a member actually turning in it"
    );

    let mut other_moved = false;
    for i in 0..plain_o.len() {
        let (id, plain) = &plain_o[i];
        let (over_id, over) = &over_o[i];
        let (whole_id, whole) = &whole_o[i];
        assert_eq!(id, over_id);
        assert_eq!(id, whole_id);
        assert_eq!(
            motor_terms(over),
            motor_terms(plain),
            "ordinary body {id:?} was moved by the apex override"
        );
        if motor_terms(plain) != motor_terms(whole) {
            other_moved = true;
        }
    }
    assert!(
        other_moved,
        "no ordinary body's bill differs between the contracts, so the check is vacuous"
    );
}

/// The same reading in the other direction: in an `Inertial` world a member told to run `Sweep`
/// bills exactly what it bills in a plain `Sweep` world, while the ordinary bodies around it bill
/// the disc model's numbers. This is Astra's fourth cell of the 2×2, pinned in the code.
#[test]
fn a_sweep_override_in_an_inertial_world_puts_the_member_back_on_the_shipped_contract() {
    let (plain_m, plain_o) = one_scripted_tick(MotorModel::Sweep, None);
    let (whole_m, whole_o) = one_scripted_tick(MotorModel::Inertial, None);
    let (back_m, back_o) = one_scripted_tick(MotorModel::Inertial, Some(MotorModel::Sweep));

    let mut member_moved = false;
    for i in 0..plain_m.len() {
        assert_eq!(
            motor_terms(&back_m[i].1),
            motor_terms(&plain_m[i].1),
            "the member did not get the shipped contract back"
        );
        if motor_terms(&whole_m[i].1) != motor_terms(&plain_m[i].1) {
            member_moved = true;
        }
    }
    assert!(member_moved, "the member bills the same under both contracts, so this is vacuous");
    for i in 0..plain_o.len() {
        assert_eq!(
            motor_terms(&back_o[i].1),
            motor_terms(&whole_o[i].1),
            "an ordinary body left the world's own contract"
        );
    }
}

/// The override is a transient: it survives no snapshot and is not part of the world's state.
#[test]
fn the_override_is_a_transient_and_is_not_persisted() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    world.set_apex_motor_model(Some(MotorModel::Inertial));
    for _ in 0..20 {
        world.step();
        world.drain_events();
    }
    let bytes = cubarium_core::encode_snapshot(&world.state, "apex-motor-test");
    let (_, state) = cubarium_core::decode_snapshot(&bytes).expect("decodes");
    let resumed = World::from_state(state).expect("resumes");
    assert_eq!(
        resumed.apex_motor_model(),
        None,
        "a resumed world runs the world's own contract until it is told otherwise"
    );
    assert_eq!(resumed.motor_model(), MotorModel::Sweep, "and that contract is the shipped one");
    assert_eq!(resumed.apex_turn_radius(), ApexTurnRadius::Grasp);
}
