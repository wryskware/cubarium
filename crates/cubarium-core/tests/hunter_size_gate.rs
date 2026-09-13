//! Semantic profile version 5: size-aware permission for paid hunter growth
//! (`design/7_Research/astra-hunter-size-aware-growth-proposal-2026-09-13.md`).
//!
//! Three claims carry this file.
//!
//! 1. **Versions 3 and 4, and every ordinary organism, are untouched.** They evaluate the
//!    original `growth_reserve_min · reserve_max` and the original increment.
//! 2. **Version 5 changes exactly one thing**: the *permission* threshold is scaled by the
//!    member's actual size. It keeps charge80's fixed oxidation activation, and the increment
//!    after the predicate — its four caps, its debits and its heat — is the same arithmetic.
//! 3. **The gate is a precondition, not a floor.** It never caps the step, never protects
//!    reserve, and never hands out an unpaid unit of structure.
//!
//! The retained cohort never entered the growth branch at all, so its caps and debit identities
//! had no artifact evidence. The positive-growth fixtures here are deliberately hand-built to
//! exercise each of the four caps for real before the experiment runs. Nothing here is a
//! balance or viability claim.

use cubarium_core::hunter::{
    FixedHunterProfile, HunterTarget, OxidationPolicy, PROFILE_VERSION, PROFILE_VERSION_CHARGE80,
    PROFILE_VERSION_SIZE_GATE, SUPPORTED_PROFILE_VERSIONS,
};
use cubarium_core::ids::OrganismId;
use cubarium_core::snapshot::{SnapshotError, state_hash};
use cubarium_core::{DT, World, WorldConfig, decode_snapshot, encode_snapshot};
use cubarium_surface::{Face, SurfacePoint};

const SPOT: SurfacePoint = SurfacePoint { face: Face::Top, u: 22.0, v: 34.0 };

/// A world with nothing growing, decomposing, raining or littering, and no founders of its own.
fn quiet_config() -> WorldConfig {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    cfg.weather.amplitude = 0.0;
    cfg.water.rain_rate = 0.0;
    cfg.habitat.light_base = 0.0;
    cfg.habitat.light_height_gain = 0.0;
    cfg.habitat.light_noise_gain = 0.0;
    cfg.detritus.initial_dark = 0.0;
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.fall = 0.0;
    cfg
}

/// [`quiet_config`] with every per-tick battery charge off, so the only thing that moves the
/// stocks between the top of the tick and the growth site is the site under test.
fn still_config() -> WorldConfig {
    let mut cfg = quiet_config();
    cfg.organism.maintenance = 0.0;
    cfg.organism.move_cost = 0.0;
    cfg.organism.sense_cost = 0.0;
    cfg
}

fn profile_at(world: &World, version: u32) -> FixedHunterProfile {
    let base = FixedHunterProfile::lanternjaw_trial(world.config());
    match version {
        PROFILE_VERSION_CHARGE80 => base.charge80(),
        PROFILE_VERSION_SIZE_GATE => base.size_gate(),
        _ => base,
    }
}

fn founded(version: u32, cfg: WorldConfig) -> (World, OrganismId) {
    let mut world = World::new(cfg).expect("a quiet world is valid");
    let profile = profile_at(&world, version);
    assert_eq!(profile.version, version);
    let receipt = world
        .start_hunter_trial(
            profile,
            HunterTarget { face: SPOT.face.index() as u8, u: SPOT.u, v: SPOT.v },
        )
        .unwrap_or_else(|e| panic!("the trial must start: {e}"));
    (world, receipt.id)
}

/// Fixture knobs. Structure and reserve are material, so what a fixture puts in is booked as
/// imported material exactly as `hunter_charging`'s reserve knob does; the conservation audit
/// stays honest and the growth transfer is still the only unbooked mover under test.
fn set_stocks(world: &mut World, id: OrganismId, structure: f64, reserve: f64, energy: f64) {
    let o = world.state.organisms.get_mut(id).expect("alive");
    let material_before = o.structure + o.reserve;
    o.structure = structure;
    o.reserve = reserve;
    o.energy = energy;
    world.state.external_material_in += (structure + reserve) - material_before;
}

fn stocks(world: &World, id: OrganismId) -> (f64, f64, f64) {
    let o = world.state.organisms.get(id).expect("alive");
    (o.structure, o.reserve, o.energy)
}

fn adult_structure(world: &World, id: OrganismId) -> f64 {
    world.state.organisms.get(id).expect("alive").phenotype.structure_adult
}

/// The legacy threshold this world's config produces for this member.
fn legacy_gate(world: &World, id: OrganismId) -> f64 {
    let o = world.state.organisms.get(id).expect("alive");
    world.config().organism.growth_reserve_min * o.phenotype.reserve_max
}

const EXACT: f64 = 0.0;

/// Slack for reading a transaction back out of a **stock**, the same reasoning and the same
/// constant `hunter_charging` documents: `reserve -= grown` is exact as an operation, but
/// `before - after` recovered afterwards is a second rounding of a number near 0.8 or 3.0, so
/// it trails the amount by a few ulps. Whether anything moved at all is still exact — a step
/// that did not happen leaves the stock bit-for-bit unchanged, which is what [`EXACT`] checks.
const STOCK_SLACK: f64 = 1e-12;

// ---------------------------------------------------------------- the selector

/// Version 5 loads, keeps charge80's oxidation policy, and is not what the constructor writes.
#[test]
fn version_five_is_supported_and_still_carries_the_fixed_charging_threshold() {
    assert_eq!(PROFILE_VERSION_SIZE_GATE, 5);
    assert_eq!(SUPPORTED_PROFILE_VERSIONS, [3, 4, 5]);

    let cfg = WorldConfig::default();
    let base = FixedHunterProfile::lanternjaw_trial(&cfg);
    assert_eq!(base.version, PROFILE_VERSION, "the default trial is unchanged");

    let size = base.clone().size_gate();
    assert_eq!(size.version, PROFILE_VERSION_SIZE_GATE);
    size.validate().expect("version 5 validates");

    // The implementation trap the proposal names: bumping the version must not drop charging.
    assert_eq!(
        size.oxidation_policy(),
        OxidationPolicy::Fixed(0.80),
        "version 5 is charge80 PLUS the gate, not the gate instead of charging"
    );
    assert_eq!(size.oxidation_policy(), base.clone().charge80().oxidation_policy());
    assert_eq!(base.oxidation_policy(), OxidationPolicy::Configured);

    // The version is the entire serialized diff from charge80.
    let mut expected = base.clone().charge80();
    expected.version = PROFILE_VERSION_SIZE_GATE;
    assert_eq!(size, expected, "version 5 changed a field other than `version`");
}

/// Legacy profiles and ordinary organisms take the original expression, bit for bit.
#[test]
fn legacy_profiles_and_ordinary_organisms_keep_the_original_gate() {
    let cfg = WorldConfig::default();
    let base = FixedHunterProfile::lanternjaw_trial(&cfg);
    for version in [PROFILE_VERSION, PROFILE_VERSION_CHARGE80] {
        let mut p = base.clone();
        p.version = version;
        for (s, s_adult) in [(0.8, 2.0), (1.2, 2.0), (0.0, 2.0), (2.0, 2.0), (9.0, 2.0)] {
            assert_eq!(
                p.growth_gate(1.2, s, s_adult) - 1.2,
                EXACT,
                "v{version} must return the legacy gate untouched at S={s}"
            );
        }
    }
}

/// The candidate's values at the proposal's stated sizes, and at the ends of the ratio.
#[test]
fn the_candidate_gate_is_the_legacy_gate_scaled_by_actual_size() {
    let cfg = WorldConfig::default();
    let p = FixedHunterProfile::lanternjaw_trial(&cfg).size_gate();
    // For this frozen profile the legacy gate is 0.3 · 4 = 1.2, so the candidate is 0.6 · S.
    for (s, want) in [(0.8, 0.48), (1.2, 0.72), (1.6, 0.96), (2.0, 1.2)] {
        assert!(
            (p.growth_gate(1.2, s, 2.0) - want).abs() < 1e-12,
            "S={s} expected {want}, got {}",
            p.growth_gate(1.2, s, 2.0)
        );
    }
    // Clamped at both ends, and never negative or above the legacy gate.
    assert_eq!(p.growth_gate(1.2, -5.0, 2.0), 0.0, "the ratio clamps at zero");
    assert_eq!(p.growth_gate(1.2, 9.0, 2.0) - 1.2, EXACT, "the ratio clamps at one");
    assert_eq!(p.growth_gate(1.2, 2.0, 2.0) - 1.2, EXACT, "at adulthood the two agree exactly");
}

/// A degenerate adult denominator is refused into the legacy gate, not propagated as NaN.
///
/// A NaN gate would make `R > gate` false forever and look like a very strict threshold, which
/// is a different mechanism wearing this one's name.
#[test]
fn a_degenerate_adult_denominator_falls_back_instead_of_comparing_against_nan() {
    let cfg = WorldConfig::default();
    let p = FixedHunterProfile::lanternjaw_trial(&cfg).size_gate();
    for bad_adult in [0.0, -1.0, f64::NAN, f64::INFINITY, -f64::INFINITY] {
        let gate = p.growth_gate(1.2, 0.8, bad_adult);
        assert!(gate.is_finite(), "adult={bad_adult} produced {gate}");
        assert_eq!(gate - 1.2, EXACT, "adult={bad_adult} must fall back to the legacy gate");
    }
    for bad_structure in [f64::NAN, f64::INFINITY] {
        assert_eq!(p.growth_gate(1.2, bad_structure, 2.0) - 1.2, EXACT);
    }
    for bad_gate in [f64::NAN, f64::INFINITY] {
        assert!(p.growth_gate(bad_gate, 0.8, 2.0).is_nan() == bad_gate.is_nan());
    }
}

// ---------------------------------------------------------------- the predicate in a world

/// The one-line contrast the experiment exists to create: the same juvenile, the same stocks,
/// growing under version 5 and refused under version 4.
#[test]
fn the_same_juvenile_grows_under_version_five_and_is_refused_under_version_four() {
    let mut outcomes = Vec::new();
    for version in [PROFILE_VERSION_CHARGE80, PROFILE_VERSION_SIZE_GATE] {
        let (mut world, id) = founded(version, still_config());
        // S=0.8 (a juvenile), R=0.6 (above 0.6·0.8=0.48, below the legacy 1.2), and a battery
        // above charge80's 3.2 activation so oxidation cannot move reserve first.
        set_stocks(&mut world, id, 0.8, 0.6, 3.5);
        world.step();
        outcomes.push(stocks(&world, id));
    }
    let (s4, r4, _) = outcomes[0];
    let (s5, r5, _) = outcomes[1];
    assert_eq!(s4 - 0.8, EXACT, "version 4 must not grow: 0.6 is below its 1.2 gate");
    assert_eq!(r4 - 0.6, EXACT, "version 4 moved reserve with no growth and no oxidation");
    assert!(s5 > 0.8, "version 5 must grow: 0.6 is above its 0.48 gate");
    assert!(r5 < 0.6, "the increment is paid out of reserve");
    assert!((s5 - 0.8) - (0.6 - r5) < 1e-15, "every unit built came from reserve");
}

/// Strict inequality, at the exact boundary and one representable step above it.
#[test]
fn the_gate_refuses_equality_and_permits_the_next_representable_reserve() {
    let (mut probe, probe_id) = founded(PROFILE_VERSION_SIZE_GATE, still_config());
    let s = 0.8;
    let gate = FixedHunterProfile::lanternjaw_trial(probe.config())
        .size_gate()
        .growth_gate(legacy_gate(&probe, probe_id), s, adult_structure(&probe, probe_id));
    drop(probe_id);
    let _ = &mut probe;

    for (reserve, should_grow) in [(gate, false), (f64::from_bits(gate.to_bits() + 1), true)] {
        let (mut world, id) = founded(PROFILE_VERSION_SIZE_GATE, still_config());
        set_stocks(&mut world, id, s, reserve, 3.5);
        world.step();
        let (grown_s, _, _) = stocks(&world, id);
        assert_eq!(
            grown_s > s,
            should_grow,
            "reserve {reserve:e} against gate {gate:e}: expected grow={should_grow}"
        );
    }
}

/// An adult never grows under either policy: the structure side of the predicate is unchanged.
#[test]
fn an_adult_never_grows_under_either_policy() {
    for version in [PROFILE_VERSION_CHARGE80, PROFILE_VERSION_SIZE_GATE] {
        let (mut world, id) = founded(version, still_config());
        let adult = adult_structure(&world, id);
        set_stocks(&mut world, id, adult, 3.9, 3.5);
        world.step();
        assert_eq!(stocks(&world, id).0 - adult, EXACT, "v{version} grew an adult");
    }
}

// ---------------------------------------------------------------- the four caps, for real

/// Run one tick from the given stocks and config, and return what the growth site moved.
fn one_growth_step(cfg: WorldConfig, structure: f64, reserve: f64, energy: f64) -> (f64, f64, f64) {
    let (mut world, id) = founded(PROFILE_VERSION_SIZE_GATE, cfg);
    set_stocks(&mut world, id, structure, reserve, energy);
    let before = stocks(&world, id);
    world.step();
    let after = stocks(&world, id);
    (after.0 - before.0, before.1 - after.1, before.2 - after.2)
}

/// The rate cap, and the debit identity at the mutation site: `dS` costs exactly `dS` reserve
/// and `build_cost · dS` battery.
#[test]
fn the_rate_cap_binds_and_its_debits_close() {
    let cfg = still_config();
    let build_cost = cfg.organism.build_cost;
    let rate = FixedHunterProfile::lanternjaw_trial(&cfg).juvenile_growth_rate;
    let want = rate * DT;
    let (ds, dr, de) = one_growth_step(cfg, 0.8, 3.0, 3.5);
    assert!((ds - want).abs() < STOCK_SLACK, "expected the rate cap {want:e}, grew {ds:e}");
    assert!((dr - ds).abs() < STOCK_SLACK, "reserve paid {dr:e} for {ds:e} of structure");
    assert!((de - build_cost * ds).abs() < STOCK_SLACK, "battery paid {de:e}, expected {:e}", build_cost * ds);
}

/// The remaining-structure cap: the last increment lands exactly on adult structure and stops.
#[test]
fn the_remaining_structure_cap_binds_on_the_final_increment() {
    let cfg = still_config();
    let adult = 2.0;
    let remaining = 0.000_02; // smaller than one rate-limited step
    let (ds, dr, _) = one_growth_step(cfg, adult - remaining, 3.0, 3.5);
    assert!((ds - remaining).abs() < STOCK_SLACK, "expected {remaining:e}, grew {ds:e}");
    assert!((dr - ds).abs() < STOCK_SLACK);
}

/// The reserve cap: with the permission threshold at zero, a member with less reserve than one
/// rate-limited step spends all of it and no more. Structure is never built unpaid.
#[test]
fn the_reserve_cap_binds_and_never_builds_more_than_the_reserve_holds() {
    let mut cfg = still_config();
    cfg.organism.growth_reserve_min = 0.0; // a legal configured value; the gate is then 0
    cfg.organism.oxidation_rate = 0.0; // so nothing else can touch the reserve first
    let reserve = 0.000_05;
    let (ds, dr, _) = one_growth_step(cfg, 0.8, reserve, 3.5);
    assert!((ds - reserve).abs() < STOCK_SLACK, "expected the reserve cap {reserve:e}, grew {ds:e}");
    assert!((dr - reserve).abs() < STOCK_SLACK, "the whole reserve was spent and no more");
}

/// The battery cap: what the energy cannot cover is not built, and the cost never exceeds the
/// battery that was there.
#[test]
fn the_battery_cap_binds_and_the_cost_never_exceeds_the_battery() {
    let mut cfg = still_config();
    cfg.organism.oxidation_rate = 0.0; // otherwise a low battery would be refilled from reserve
    let build_cost = cfg.organism.build_cost;
    let energy = 0.000_04;
    let want = energy / build_cost;
    let (ds, dr, de) = one_growth_step(cfg, 0.8, 3.0, energy);
    assert!((ds - want).abs() < STOCK_SLACK, "expected the battery cap {want:e}, grew {ds:e}");
    assert!((dr - ds).abs() < STOCK_SLACK);
    assert!((de - energy).abs() < STOCK_SLACK, "the whole battery paid for it");
    assert!(de <= energy + STOCK_SLACK, "the cost cannot exceed the battery that existed");
}

/// The zero-rate case, which turns out to be a contract fact rather than a reachable branch.
///
/// A member's `juvenile_growth_rate` is in the profile's *positive* validation set, so a zero
/// rate cannot reach a validated profile at all: the door refuses it before the growth site
/// ever sees it. That is worth pinning, because it is the reason the size-aware gate cannot
/// combine with a zero rate to produce anything — there is no such member. The rate cap itself
/// is exercised at a real positive rate above, and again here at an arbitrarily small one, so
/// the limiting arithmetic is covered on the side where it is reachable.
#[test]
fn a_zero_member_growth_rate_is_refused_by_the_profile_contract() {
    let cfg = still_config();
    let mut profile = FixedHunterProfile::lanternjaw_trial(&cfg).size_gate();
    profile.juvenile_growth_rate = 0.0;
    let err = profile.validate().expect_err("a zero member growth rate must be refused");
    assert!(err.contains("juvenile_growth_rate"), "{err}");
    assert!(err.contains("positive"), "{err}");

    // A vanishingly small but legal rate still binds exactly, and still pays for what it builds.
    let tiny = 1e-9;
    let (mut world, id) = founded(PROFILE_VERSION_SIZE_GATE, cfg.clone());
    world.state.hunters.profile.as_mut().expect("a profile").juvenile_growth_rate = tiny;
    world.state.hunters.profile.as_ref().expect("a profile").validate().expect("still valid");
    set_stocks(&mut world, id, 0.8, 3.0, 3.5);
    let before = stocks(&world, id);
    world.step();
    let after = stocks(&world, id);
    assert!((after.0 - before.0 - tiny * DT).abs() < STOCK_SLACK);
    assert!((before.1 - after.1 - (after.0 - before.0)).abs() < STOCK_SLACK);
}

/// A zero build cost omits the battery term, exactly as the original expression does, and
/// charges nothing for construction.
#[test]
fn a_zero_build_cost_omits_the_battery_term() {
    let mut cfg = still_config();
    cfg.organism.build_cost = 0.0;
    cfg.organism.oxidation_rate = 0.0;
    let rate = FixedHunterProfile::lanternjaw_trial(&cfg).juvenile_growth_rate;
    // A battery far too small to cover any construction, which now costs nothing.
    let (ds, dr, de) = one_growth_step(cfg, 0.8, 3.0, 1e-9);
    assert!((ds - rate * DT).abs() < STOCK_SLACK, "the rate cap must still bind");
    assert!((dr - ds).abs() < STOCK_SLACK);
    assert_eq!(de, EXACT, "a zero build cost charged the battery");
}

// ---------------------------------------------------------------- persistence and restart

/// A mid-growth snapshot round-trips exactly, keeps the selector, and resuming from it is
/// indistinguishable from never stopping — state and both event streams.
#[test]
fn a_restart_while_growing_preserves_the_state_the_selector_and_the_events() {
    let (mut uninterrupted, id) = founded(PROFILE_VERSION_SIZE_GATE, still_config());
    set_stocks(&mut uninterrupted, id, 0.8, 3.0, 3.5);
    for _ in 0..40 {
        uninterrupted.step();
    }
    // Mid-growth: structure has moved and adulthood is nowhere near.
    let (s, _, _) = stocks(&uninterrupted, id);
    assert!(s > 0.8 && s < adult_structure(&uninterrupted, id), "the fixture must be growing: S={s}");

    let bytes = encode_snapshot(&uninterrupted.state, "size-gate-restart");
    let (_, decoded) = decode_snapshot(&bytes).expect("a version 5 world round-trips");
    assert_eq!(
        decoded.hunters.profile.as_ref().expect("a profile").version,
        PROFILE_VERSION_SIZE_GATE,
        "the selector must survive the round trip"
    );
    assert_eq!(state_hash(&decoded), state_hash(&uninterrupted.state));
    let mut resumed = World::from_state(decoded).expect("valid");

    for _ in 0..120 {
        uninterrupted.step();
        resumed.step();
        assert_eq!(
            format!("{:?}", uninterrupted.drain_events()),
            format!("{:?}", resumed.drain_events()),
            "life events diverged after the restart"
        );
        assert_eq!(
            format!("{:?}", uninterrupted.drain_hunter_events()),
            format!("{:?}", resumed.drain_hunter_events()),
            "hunter events diverged after the restart"
        );
    }
    assert_eq!(state_hash(&uninterrupted.state), state_hash(&resumed.state));
    assert_eq!(
        encode_snapshot(&uninterrupted.state, "x"),
        encode_snapshot(&resumed.state, "x"),
        "a restart mid-growth changed the world"
    );
    assert!(stocks(&resumed, id).0 > s, "it kept growing after the restart");
}

/// An unsupported version is refused by name rather than resumed under another policy. This is
/// the same contract version 4 relies on, and it is what makes reusing the shape safe.
///
/// **This is a stand-in, not an old-reader test.** It runs in *this* build, where version 6 is
/// the unsupported one; it does not execute a genuine pre-change executable against a version 5
/// payload. That a real old reader refuses version 5 follows from its `[3, 4]` supported set by
/// source inspection, and only a different binary could establish it — exactly as the version 4
/// package recorded for its own case.
#[test]
fn an_unsupported_version_is_refused_rather_than_reinterpreted() {
    let (mut world, id) = founded(PROFILE_VERSION_SIZE_GATE, still_config());
    set_stocks(&mut world, id, 0.8, 3.0, 3.5);
    world.step();
    // Version 5 itself decodes here.
    decode_snapshot(&encode_snapshot(&world.state, "ok")).expect("this build supports version 5");
    // A build that does not know version 5 refuses it; version 6 stands in for such a reader's
    // view of any version outside its own list.
    world.state.hunters.profile.as_mut().expect("a profile").version = 6;
    match decode_snapshot(&encode_snapshot(&world.state, "bad")) {
        Err(SnapshotError::Invalid(reason)) => {
            assert!(reason.contains("version 6"), "{reason}");
            assert!(reason.contains("[3, 4, 5]"), "the refusal names what is supported: {reason}");
        }
        other => panic!("an unsupported profile version decoded as {other:?}"),
    }
}

// ---------------------------------------------------------------- the observer

/// The flow ledger observes the growth it now sees and moves nothing. Enabled and disabled must
/// land on the same world, tick for tick and event for event.
#[test]
fn the_flow_ledger_records_growth_without_moving_the_world() {
    let mut worlds = Vec::new();
    for record in [false, true] {
        let (mut world, id) = founded(PROFILE_VERSION_SIZE_GATE, still_config());
        set_stocks(&mut world, id, 0.8, 3.0, 3.5);
        if record {
            world.enable_flow_ledger();
        }
        let mut stream = String::new();
        for _ in 0..200 {
            world.step();
            stream.push_str(&format!("{:?}", world.drain_events()));
            stream.push_str(&format!("{:?}", world.drain_hunter_events()));
        }
        worlds.push((world, id, stream));
    }
    let (plain, _, plain_stream) = &worlds[0];
    let (observed, id, observed_stream) = &worlds[1];
    assert_eq!(state_hash(&plain.state), state_hash(&observed.state), "the observer moved the world");
    assert_eq!(
        encode_snapshot(&plain.state, "x"),
        encode_snapshot(&observed.state, "x"),
        "the observer changed the closing bytes"
    );
    assert_eq!(plain_stream, observed_stream, "the observer changed an event stream");

    // …and it actually recorded the growth, with the moving gate summarised as a range.
    let ledger = observed.flow_ledger().expect("a ledger was requested");
    let m = ledger.members.get(id).expect("the member is registered");
    assert!(m.growth.ticks > 0, "the fixture must actually grow");
    assert!(m.growth.structure_gained > 0.0);
    assert_eq!(m.residual.violations, 0, "the growth transfers must reconcile");
    assert_eq!(m.gate.first_growth_tick.is_some(), true);
    assert!(
        m.gate.gate_reserve_max > m.gate.gate_reserve_min,
        "a size-aware gate must be recorded as a range, not one constant: {:?}..{:?}",
        m.gate.gate_reserve_min,
        m.gate.gate_reserve_max
    );
    assert!(
        m.gate.gate_reserve_at_first_growth.expect("it grew") < m.gate.legacy_gate_reserve_last,
        "the crossing happened below the legacy threshold, which is the whole experiment"
    );
    assert!(m.gate.max_structure > 0.8);
}
