//! The **inertial motor model**, paired against the shipped sweep model
//! (`design/handoffs/ecology-v1-motor-inertial-opus-2026-09-16.md`).
//!
//! Written from the brief's definitions, before the implementation:
//!
//! - every organism is a uniform **disc** of radius `r = phenotype.extent`; the apex's grasp
//!   (`capture_offset + capture_reach`) is contact geometry and no longer a turn radius;
//! - rotation is an **energy-equivalent speed** `v_rot = r·ω/√2` — the radius of gyration of a
//!   disc — which replaces both the outer-point sweep `r·ω` and the separate
//!   [`motor::ROTATION_COST_SCALE`];
//! - the envelope combines the two as energies, `√(v² + v_rot²) ≤ speed_cap`, where the
//!   shipped model has `|v| + r·|ω| ≤ speed_cap`;
//! - the bill is `move_cost · S · (|v| + v_rot) · dt`, so a body that only translates pays
//!   exactly the shipped bill.
//!
//! The switch is a `World`-level transient. `Sweep` is the default and is byte-identical to
//! the build that never heard of the switch: the six pinned hashes below were printed by
//! commit 2eb8a9f, before any of this existed.

use cubarium_core::hunter::{ContactGeometry, FixedHunterProfile, HunterTarget};
use cubarium_core::ids::OrganismId;
use cubarium_core::motor::{self, MotorBill, MotorLimits, MotorModel, MotorRequest};
use cubarium_core::neural::Policy;
use cubarium_core::neural::action::{Action7, Envelope};
use cubarium_core::neural::gru::{Gru32, HIDDEN, INPUT, N, R, Z};
use cubarium_core::neural::obs::FOOD_NEAR;
use cubarium_core::snapshot::state_hash;
use cubarium_core::{DT, World, WorldConfig};
use cubarium_surface::Vec2;

const SQRT_2: f64 = std::f64::consts::SQRT_2;

/// A bill for a body of unit structure at the shipped costs, so the arithmetic below is the
/// contract's and not a fixture's.
fn bill() -> MotorBill {
    let cfg = WorldConfig::default();
    MotorBill {
        structure: 1.0,
        maintenance: cfg.organism.maintenance,
        sense_radius: 3.0,
        move_cost: cfg.organism.move_cost,
        sense_cost: cfg.organism.sense_cost,
    }
}

fn limits(radius_px: f64, turn_rate_max: f64, speed_cap: f64, budget: f64) -> MotorLimits {
    MotorLimits { radius_px, turn_rate_max, speed_cap, motor_budget: budget, dt: DT }
}

const EAST: Vec2 = Vec2 { x: 1.0, y: 0.0 };

// ------------------------------------------------------------------ the model's own algebra

/// The two models name themselves, round-trip through the CLI spelling, and `Sweep` is the
/// default everywhere it can be read.
#[test]
fn the_default_model_is_sweep_and_both_names_round_trip() {
    assert_eq!(MotorModel::default(), MotorModel::Sweep);
    assert_eq!(MotorModel::Sweep.name(), "sweep");
    assert_eq!(MotorModel::Inertial.name(), "inertial");
    for m in [MotorModel::Sweep, MotorModel::Inertial] {
        assert_eq!(MotorModel::parse(m.name()).expect("its own name parses"), m);
    }
    assert!(MotorModel::parse("rod").is_err(), "an unknown model is refused, not defaulted");
    assert!(MotorModel::Sweep.is_sweep());
    assert!(!MotorModel::Inertial.is_sweep());
    assert_eq!(World::new(WorldConfig::default()).expect("valid").motor_model(), MotorModel::Sweep);
}

/// `v_rot = r·ω/√2` is the whole of the inertial rotation term, and the sweep model's is
/// `r·ω` — the radius of gyration of a uniform disc against the outermost point.
#[test]
fn the_inertial_rotation_term_is_the_discs_radius_of_gyration() {
    let r = 2.5;
    let omega = 0.4;
    assert_eq!(MotorModel::Sweep.rotation_radius_px(r), r);
    assert!((MotorModel::Inertial.rotation_radius_px(r) - r / SQRT_2).abs() < 1e-15);
    assert!((MotorModel::Sweep.rotation_speed(r, omega) - r * omega).abs() < 1e-15);
    assert!((MotorModel::Inertial.rotation_speed(r, omega) - r * omega / SQRT_2).abs() < 1e-15);
    // The envelope combines as a sum under Sweep and as energies under Inertial.
    assert!((MotorModel::Sweep.envelope_magnitude(3.0, 4.0) - 7.0).abs() < 1e-15);
    assert!((MotorModel::Inertial.envelope_magnitude(3.0, 4.0) - 5.0).abs() < 1e-15);
    // And the price of a px/s of rotation: the rod figure 0.5, or one whole px/s of travel.
    assert_eq!(MotorModel::Sweep.rotation_price(), motor::ROTATION_COST_SCALE);
    assert_eq!(MotorModel::Inertial.rotation_price(), 1.0);
}

// ------------------------------------------------------------------ the bill

/// **A body that only translates pays exactly the shipped bill, bit for bit**, under both
/// models and at every speed — the invariant the brief pins.
#[test]
fn pure_translation_bills_identically_under_both_models() {
    let b = bill();
    for speed in [0.0, 0.03, 0.3, 1.0, 16.667, 40.0] {
        let sweep = b.total_cost_in(speed, 0.0, DT, MotorModel::Sweep);
        let inertial = b.total_cost_in(speed, 0.0, DT, MotorModel::Inertial);
        assert_eq!(
            sweep.to_bits(),
            inertial.to_bits(),
            "pure translation at {speed} px/s must be the same bill bit for bit"
        );
        // And the shipped entry point is the sweep model, unchanged.
        assert_eq!(b.total_cost(speed, 0.0, DT).to_bits(), sweep.to_bits());
    }
}

/// A pure rotation at the same `ω`: the inertial bill is the disc's `r·ω/√2` at full price
/// where the shipped bill is half of the outer point's `r·ω`. For an **ordinary** body that is
/// √2 dearer per radian; for an **apex** it is far cheaper, because the grasp is gone.
#[test]
fn a_pure_rotation_is_priced_by_the_radius_each_model_uses() {
    let b = bill();
    let per_motor = b.move_cost * b.structure * DT;
    let (r, omega) = (2.5, 0.4);

    let sweep_rot = MotorModel::Sweep.rotation_speed(r, omega);
    let inertial_rot = MotorModel::Inertial.rotation_speed(r, omega);
    let sweep = b.total_cost_in(0.0, sweep_rot, DT, MotorModel::Sweep) - b.upkeep(DT);
    let inertial = b.total_cost_in(0.0, inertial_rot, DT, MotorModel::Inertial) - b.upkeep(DT);
    assert!((sweep - per_motor * motor::ROTATION_COST_SCALE * r * omega).abs() < 1e-18);
    assert!((inertial - per_motor * r * omega / SQRT_2).abs() < 1e-18);
    // 1/√2 of the outer sweep, at full price, against half of it: √2 dearer per radian.
    assert!(
        (inertial / sweep - SQRT_2).abs() < 1e-9,
        "an ordinary disc turns {} times dearer, expected √2",
        inertial / sweep
    );

    // The apex, at its own measured geometry: 9 px of lobes against a 14.82 px grasp. Dropping
    // the grasp more than halves the radius, and the full price then gives most of that back —
    // the apex's radian gets *slightly* cheaper, not far cheaper. Where it gains is the
    // envelope, which `the_apex_gains_turning_room_rather_than_a_discount` measures.
    let (apex_lobes, apex_grasp) = apex_radii();
    assert!(apex_grasp > apex_lobes, "the lanternjaw's grasp reaches past its own lobes");
    let apex_sweep = per_motor * motor::ROTATION_COST_SCALE * apex_grasp * omega;
    let apex_inertial = per_motor * apex_lobes / SQRT_2 * omega;
    let apex_ratio = apex_inertial / apex_sweep;
    assert!(
        (0.80..0.90).contains(&apex_ratio),
        "the apex's radian costs {apex_ratio} of the shipped one; the geometry says 0.859"
    );
    assert!(
        apex_ratio < 1.0 && apex_ratio < inertial / sweep,
        "the apex is the one body whose radian gets cheaper, where an ordinary one's rises by √2"
    );
}

/// `turn_radius_px_in` is the radius **the model in force** puts in the rotation term: the
/// larger of the lobes and the grasp under `Sweep`, and the disc's radius of gyration over the
/// body's own lobes — grasp excluded — under `Inertial`.
#[test]
fn the_apex_grasp_is_a_turn_radius_only_under_the_sweep_model() {
    let world = World::new(WorldConfig::default()).expect("valid");
    let profile = FixedHunterProfile::lanternjaw_trial(world.config());
    let mut world = world;
    let hunter = world
        .start_hunter_trial(profile.clone(), HunterTarget { face: 4, u: 32.0, v: 32.0 })
        .expect("started")
        .id;
    let o = world.state.organisms.get(hunter).expect("alive");
    let geometry = ContactGeometry::of(&profile, o);
    let lobes = o.phenotype.extent;
    let grasp = geometry.capture_offset_body.length() + geometry.capture_reach_px;

    let sweep = motor::turn_radius_px_in(o, Some(&geometry), MotorModel::Sweep);
    let inertial = motor::turn_radius_px_in(o, Some(&geometry), MotorModel::Inertial);
    assert_eq!(sweep, motor::turn_radius_px(o, Some(&geometry)), "the shipped entry point");
    assert!((sweep - lobes.max(grasp)).abs() < 1e-12);
    assert!((inertial - lobes / SQRT_2).abs() < 1e-12);
    assert!(
        grasp > 1.5 * lobes,
        "the lanternjaw's grasp reaches well past its own lobes: {grasp}/{lobes}"
    );
    // The observation already reads the lobe radius with no apex geometry, so under Inertial
    // the number the body is told and the number the envelope uses finally agree.
    assert_eq!(
        motor::turn_radius_px_in(o, None, MotorModel::Inertial),
        inertial,
        "with or without the grasp, the inertial radius is the same body"
    );
}

/// The lanternjaw's lobe extent and grasp reach, for the arithmetic above.
fn apex_radii() -> (f64, f64) {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    let profile = FixedHunterProfile::lanternjaw_trial(world.config());
    let hunter = world
        .start_hunter_trial(profile.clone(), HunterTarget { face: 4, u: 32.0, v: 32.0 })
        .expect("started")
        .id;
    let o = world.state.organisms.get(hunter).expect("alive");
    let g = ContactGeometry::of(&profile, o);
    (o.phenotype.extent, g.capture_offset_body.length() + g.capture_reach_px)
}

// ------------------------------------------------------------------ the envelope

/// The diagonal the two envelopes disagree about: `v = v_rot = cap/√2` sits exactly **on** the
/// inertial envelope (`√(v² + v_rot²) = cap`) and √2 **outside** the shipped one
/// (`v + v_rot = √2·cap`), which scales it back by `1/√2`.
#[test]
fn the_envelope_admits_the_diagonal_under_inertial_and_refuses_it_under_sweep() {
    let cap = 0.3;
    let r = 2.5;
    let half = cap / SQRT_2;
    let omega = half / r;
    let l = limits(r, 10.0, cap, f64::INFINITY);
    let request =
        MotorRequest { heading: rotate(EAST, omega * DT), speed: half };

    let inertial = motor::resolve_in(EAST, &request, &l, MotorModel::Inertial);
    assert!((inertial.speed - half).abs() < 1e-12, "the diagonal is granted whole");
    assert!((inertial.sweep - half).abs() < 1e-12);
    assert!(
        (MotorModel::Inertial.envelope_magnitude(inertial.speed, inertial.sweep) - cap).abs()
            < 1e-12,
        "and lands exactly on the envelope"
    );

    let sweep = motor::resolve_in(EAST, &request, &l, MotorModel::Sweep);
    assert!(
        (sweep.speed + sweep.sweep - cap).abs() < 1e-12,
        "the shipped envelope binds the same request to the cap"
    );
    assert!(
        (sweep.speed - half / SQRT_2).abs() < 1e-12,
        "scaled by one common factor 1/√2: {} against {}",
        sweep.speed,
        half / SQRT_2
    );
    // The shipped entry point is the sweep model.
    assert_eq!(motor::resolve(EAST, &request, &l), sweep);
}

/// The whole point of the change, stated as freedom to turn: at zero speed a body may pivot
/// √2 faster under `Inertial` for the same budget, because the radius in the term is `r/√2`.
#[test]
fn a_resting_disc_pivots_root_two_faster_than_the_outer_point_model_allows() {
    let cap = 0.3;
    let r = 2.5;
    let far = MotorRequest { heading: Vec2 { x: -1.0, y: 0.0 }, speed: 0.0 };
    let sweep = motor::resolve_in(EAST, &far, &limits(r, 10.0, cap, f64::INFINITY), MotorModel::Sweep);
    let inertial = motor::resolve_in(
        EAST,
        &far,
        &limits(MotorModel::Inertial.rotation_radius_px(r), 10.0, cap, f64::INFINITY),
        MotorModel::Inertial,
    );
    let (ws, wi) = (sweep.omega(DT).abs(), inertial.omega(DT).abs());
    assert!((ws - cap / r).abs() < 1e-12, "the shipped pivot ceiling is u/r");
    assert!((wi - SQRT_2 * cap / r).abs() < 1e-12, "the inertial one is √2·u/r");
    assert!((wi / ws - SQRT_2).abs() < 1e-9);
}

/// **An energy-bound body that only translates behaves identically under both models**: the
/// same delivered speed and the same charge, so nothing about the paired comparison is a
/// change to travel. The inertial resolver bounds the *billed* motion by the budget and the
/// *envelope* magnitude by the cap, rather than collapsing both onto one conservative number.
#[test]
fn an_energy_bound_translation_is_identical_under_both_models() {
    let b = bill();
    let r = 2.5;
    for energy in [0.0, 1e-4, 5e-4, 1e-3, 1e-2, 1.0] {
        let budget = b.affordable_motor(energy, DT);
        let l = limits(r, 10.0, 0.3, budget);
        let request = MotorRequest { heading: EAST, speed: 0.3 };
        let sweep = motor::resolve_in(EAST, &request, &l, MotorModel::Sweep);
        let inertial = motor::resolve_in(EAST, &request, &l, MotorModel::Inertial);
        assert_eq!(
            sweep.speed.to_bits(),
            inertial.speed.to_bits(),
            "a pure translation at energy {energy} must be delivered identically"
        );
        assert_eq!(
            b.total_cost_in(sweep.speed, 0.0, DT, MotorModel::Sweep).to_bits(),
            b.total_cost_in(inertial.speed, 0.0, DT, MotorModel::Inertial).to_bits()
        );
    }
}

/// Whatever the split, the inertial resolver never grants motion the body cannot pay for, and
/// never leaves the envelope. Swept over a grid of requests including the degenerate ones.
#[test]
fn the_inertial_resolver_stays_inside_the_envelope_and_inside_the_purse() {
    let b = bill();
    let r = 2.5;
    let r_eff = MotorModel::Inertial.rotation_radius_px(r);
    for energy in [0.0, 1e-4, 1e-3, 1e-2, 0.5] {
        let budget = b.affordable_motor(energy, DT);
        for cap in [0.0, 0.05, 0.3, 16.667] {
            for turn in [-3.0, -0.4, 0.0, 0.11, 1.5, 3.0] {
                for speed in [0.0, 0.1, 0.3, 50.0] {
                    let l = limits(r_eff, 10.0, cap, budget);
                    let request =
                        MotorRequest { heading: rotate(EAST, turn), speed };
                    let m = motor::resolve_in(EAST, &request, &l, MotorModel::Inertial);
                    assert!(m.speed.is_finite() && m.sweep.is_finite() && m.turn.is_finite());
                    assert!(
                        MotorModel::Inertial.envelope_magnitude(m.speed, m.sweep)
                            <= l.capability() + 1e-9,
                        "√(v²+v_rot²) must stay inside the cap"
                    );
                    let charged = b.total_cost_in(m.speed, m.sweep, DT, MotorModel::Inertial);
                    assert!(
                        charged <= energy.max(b.upkeep(DT)) + 1e-12,
                        "the bill {charged} outran the energy {energy} it was sized from"
                    );
                }
            }
        }
    }
}

// ------------------------------------------------------------------ the neural adapter

/// The adapter is not edited by this milestone: it is handed the radius **the model in force**
/// puts in the rotation term, and its `omega_attain`, `requested_speed` and `requested_magnitude`
/// then produce finite, in-envelope, payable motion under both models.
#[test]
fn the_neural_adapter_produces_in_envelope_motion_under_both_models() {
    let b = bill();
    let r = 2.5;
    for model in [MotorModel::Sweep, MotorModel::Inertial] {
        let radius_px = model.rotation_radius_px(r);
        for energy in [1e-4, 1e-3, 1e-2, 0.5] {
            let u_full = (0.3f64).min(b.affordable_motor(energy, DT));
            let e = Envelope {
                speed_max: 0.3,
                wading: 1.0,
                radius_px,
                turn_rate_max: 0.5,
                u_full,
                dt: DT,
            };
            for thrust in [0.0, 0.2, 0.6, 1.0] {
                for turn in [-1.0, -0.3, 0.0, 0.7, 1.0] {
                    let a = Action7([thrust, turn, 0.0, 0.0, 0.0, 0.0, 0.0]);
                    let l = limits(radius_px, 0.5, 0.3 * a.effort(), b.affordable_motor(energy, DT));
                    let m = motor::resolve_in(EAST, &e.request(EAST, &a), &l, model);
                    assert!(e.omega_attain().is_finite() && e.omega_attain() >= 0.0);
                    assert!(e.requested_speed(a.thrust()).is_finite());
                    assert!(e.requested_magnitude(&a).is_finite());
                    assert!(m.speed.is_finite() && m.sweep.is_finite());
                    assert!(
                        model.envelope_magnitude(m.speed, m.sweep) <= l.capability() + 1e-9,
                        "{model:?}: the adapter's request left the envelope"
                    );
                    // The feedback ratio the policy reads stays a fraction: the adapter's
                    // requested magnitude and the resolved `motor_magnitude` are the same
                    // linear measure under both models.
                    assert!(
                        m.motor_magnitude() <= e.requested_magnitude(&a) + 1e-9,
                        "{model:?}: delivered {} above requested {}",
                        m.motor_magnitude(),
                        e.requested_magnitude(&a)
                    );
                    assert!(
                        b.total_cost_in(m.speed, m.sweep, DT, model)
                            <= energy.max(b.upkeep(DT)) + 1e-12,
                        "{model:?}: the adapter's motion outran the purse"
                    );
                }
            }
        }
    }
}

// ------------------------------------------------------------------ the switch in the world

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

/// A default world run 1,000 ticks, then two apex adults introduced and eight ordinary bodies
/// dispatched to a recurrent policy that both thrusts and turns: every motor path the switch
/// touches — the ordinary controller, the apex override, the neural adapter, the observation —
/// is live in it.
///
/// **The pursuit stopping rule is pinned to `ForwardHalfSpace` here, deliberately.** The
/// hashes below were printed by commit `2eb8a9f`, when that was the shipped rule; the reach
/// envelope was adopted in its place on 2026-09-16, which moves the apex override and so moves
/// this world. Naming the rule this fixture was pinned under is what keeps the pin a statement
/// about the **motor** switch and about nothing else — exactly as naming `Sweep` keeps it a
/// statement about the motor rather than about whatever ships next. The adoption itself has
/// its own pins in `crates/cubarium-core/tests/pursuit_predicate_adoption.rs`.
fn paired_world(model: Option<MotorModel>) -> World {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    world.set_pursuit_stop(cubarium_core::hunter::PursuitStop::ForwardHalfSpace);
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
    if let Some(model) = model {
        world.set_motor_model(model);
    }
    world
}

/// **The pinned hashes.** Six 1,500-tick boundaries of the paired world, printed by commit
/// 2eb8a9f — the build before `MotorModel` existed. A world that leaves the default alone and
/// one that names `Sweep` must both reproduce them exactly.
const SWEEP_HASHES: [(u64, u64); 6] = [
    (1_500, 0x1dd0_4659_c55d_2980),
    (3_000, 0x56a0_f4a8_11c9_964d),
    (4_500, 0xbc76_70f1_377f_24d9),
    (6_000, 0x61e6_d61a_4bc3_5fe4),
    (7_500, 0x15ce_4554_a89f_bae3),
    (9_000, 0xfa65_65de_e6ae_2dd3),
];

#[test]
fn the_sweep_model_is_byte_identical_to_the_build_that_never_heard_of_the_switch() {
    let mut untouched = paired_world(None);
    let mut named = paired_world(Some(MotorModel::Sweep));
    assert_eq!(untouched.motor_model(), MotorModel::Sweep);
    assert_eq!(named.motor_model(), MotorModel::Sweep);
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
                "naming the default diverged from commit 2eb8a9f at tick {tick}"
            );
            next = pinned.next();
        }
    }
    assert!(next.is_none(), "every pinned boundary was checked");
    assert!(
        untouched.state.hunters.members.len() >= 1,
        "the run is only evidence if an apex was alive in it"
    );
}

/// And the variant is not vacuous: the same world, the same seed, the same introductions, run
/// under the inertial model, is a different world.
#[test]
fn the_inertial_model_moves_the_same_world() {
    let mut sweep = paired_world(Some(MotorModel::Sweep));
    let mut inertial = paired_world(Some(MotorModel::Inertial));
    assert_eq!(inertial.motor_model(), MotorModel::Inertial);
    let mut diverged_at = None;
    for tick in 1..=2_000u64 {
        sweep.step();
        inertial.step();
        sweep.drain_events();
        inertial.drain_events();
        sweep.drain_hunter_events();
        inertial.drain_hunter_events();
        if diverged_at.is_none() && state_hash(&sweep.state) != state_hash(&inertial.state) {
            diverged_at = Some(tick);
        }
    }
    assert!(diverged_at.is_some(), "the inertial model changed nothing, so it is not a model");
    inertial.check_invariants().expect("the inertial world still closes its books");
    sweep.check_invariants().expect("the sweep world still closes its books");
}

/// The switch is a transient: it survives no snapshot and is not part of the world's state.
#[test]
fn the_switch_is_a_transient_and_is_not_persisted() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    world.set_motor_model(MotorModel::Inertial);
    for _ in 0..20 {
        world.step();
        world.drain_events();
    }
    let bytes = cubarium_core::encode_snapshot(&world.state, "motor-inertial-test");
    let (_, state) = cubarium_core::decode_snapshot(&bytes).expect("decodes");
    let resumed = World::from_state(state).expect("resumes");
    assert_eq!(
        resumed.motor_model(),
        MotorModel::Sweep,
        "a resumed world runs the shipped contract until it is told otherwise"
    );
}

/// **Where the apex actually gains.** Dropping the 14.82 px grasp for a 9 px disc more than
/// halves the radius in the *envelope*, so a member pivots 2.3 times faster for the same
/// budget — and the quadrature envelope then lets it keep most of its travel while it does.
/// Its price per radian barely moves. Turning room, not a discount, is the change.
#[test]
fn the_apex_gains_turning_room_rather_than_a_discount() {
    let (lobes, grasp) = apex_radii();
    let sweep_radius = lobes.max(grasp);
    let inertial_radius = MotorModel::Inertial.rotation_radius_px(lobes);
    let freedom = sweep_radius / inertial_radius;
    assert!(
        (2.2..2.5).contains(&freedom),
        "the apex pivots {freedom} times faster for the same budget; the geometry says 2.33"
    );
    // An ordinary 2.5 px body gains only √2, so the correction is specific to the apex.
    let ordinary = 2.5 / MotorModel::Inertial.rotation_radius_px(2.5);
    assert!((ordinary - SQRT_2).abs() < 1e-12);
    assert!(freedom > ordinary, "the apex was the body the shipped radius punished");

    // And the quadrature envelope on top: at the pivot that spends half its budget, a body may
    // still travel √3/2 of the cap under `Inertial` where `Sweep` leaves it only half.
    let cap: f64 = 16.667;
    let rot: f64 = cap / 2.0;
    assert!(((cap - rot) / cap - 0.5).abs() < 1e-12);
    assert!(((cap * cap - rot * rot).sqrt() / cap - 3.0f64.sqrt() / 2.0).abs() < 1e-12);
}

/// Rotate a screen-space vector clockwise, the adapter's convention.
fn rotate(v: Vec2, a: f64) -> Vec2 {
    let (s, c) = a.sin_cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}
