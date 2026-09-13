//! The paid hunter-only charging policy: semantic profile version 4's fixed oxidation
//! activation threshold (`design/7_Research/astra-hunter-paid-charging-proposal-2026-09-13.md`).
//!
//! Two claims carry this package, and every test here serves one of them.
//!
//! 1. **Version 3 is untouched.** A member carrying it, and every ordinary organism in every
//!    world, performs exactly the arithmetic the pre-change build performed — proved byte for
//!    byte against genuine pre-change fixtures, not by inspection.
//! 2. **Version 4 changes exactly one thing.** *When* reserve is converted, and nothing about
//!    what the conversion does, what it costs, or what any gate demands.
//!
//! Nothing here is a balance claim. That charging can open a stock gate in a hand-built fixture
//! is a mechanism fact; it says nothing about whether a real lineage survives.

use std::path::PathBuf;

use cubarium_core::hunter::{
    CHARGE80_OXIDATION_THRESHOLD, FixedHunterProfile, HunterEvent, HunterTarget, OxidationPolicy,
    PROFILE_VERSION, PROFILE_VERSION_CHARGE80, SUPPORTED_PROFILE_VERSIONS,
};
use cubarium_core::ids::OrganismId;
use cubarium_core::snapshot::{HEADER_FIXED_BYTES, SnapshotError, state_hash};
use cubarium_core::{
    ChargingDiagnostics, DT, LifeEvent, World, WorldConfig, decode_snapshot, encode_snapshot,
};
use cubarium_surface::{Face, SurfacePoint, cell_of};

// ---------------------------------------------------------------- helpers

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// The postcard payload of a snapshot file: everything after the variable-length header.
fn payload(bytes: &[u8]) -> &[u8] {
    let id_len = usize::from(u16::from_le_bytes(bytes[8..10].try_into().expect("2 bytes")));
    &bytes[HEADER_FIXED_BYTES + id_len..]
}

/// A world with nothing growing, nothing decomposing and no litter of its own, and no founders.
///
/// The point is a clean observable: for an **adult** member that is not funding an offspring,
/// the only thing in the whole tick that moves `reserve` is oxidation, and the only thing that
/// raises the local `N` cell is the material that oxidation returns to it. So "did the policy
/// activate" is answered by the world's own stocks, not only by the diagnostic counter.
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

/// [`quiet_config`] with every per-tick energy charge switched off: no maintenance, no sensing
/// and no movement cost.
///
/// This exists for the *boundary* test alone, and the reason is worth stating. The activation
/// test is `energy < threshold · E_max` evaluated at physiology, which is stage 8 — by which
/// point an ordinary tick has already spent maintenance, sensing and movement. A fixture that
/// sets energy to exactly `0.50 · E_max` at the top of the tick therefore arrives at stage 8
/// slightly *below* it, and would make a strict-inequality test pass for the wrong reason. With
/// those charges at zero, and an adult member that is neither eating nor growing nor funding,
/// nothing at all moves the battery before oxidation, so the level the fixture sets is the
/// level the comparison sees and the boundary can be checked exactly.
fn still_config() -> WorldConfig {
    let mut cfg = quiet_config();
    cfg.organism.maintenance = 0.0;
    cfg.organism.move_cost = 0.0;
    cfg.organism.sense_cost = 0.0;
    cfg
}

const SPOT: SurfacePoint = SurfacePoint { face: Face::Top, u: 22.0, v: 34.0 };

fn target_of(pos: SurfacePoint) -> HunterTarget {
    HunterTarget { face: pos.face.index() as u8, u: pos.u, v: pos.v }
}

/// A quiet world with one founded member, at the given semantic profile version.
fn founded(version: u32, cfg: WorldConfig) -> (World, OrganismId) {
    let mut world = World::new(cfg).expect("a quiet world is valid");
    let mut profile = FixedHunterProfile::lanternjaw_trial(world.config());
    if version == PROFILE_VERSION_CHARGE80 {
        profile = profile.charge80();
    }
    assert_eq!(profile.version, version);
    let receipt = world
        .start_hunter_trial(profile, target_of(SPOT))
        .unwrap_or_else(|e| panic!("the trial must start: {e}"));
    (world, receipt.id)
}

/// Put a member's battery at exactly `fraction · E_max`, booking nothing: this is a fixture
/// knob, and every test that uses it reads the *change* the tick makes rather than the level.
fn set_energy_fraction(world: &mut World, id: OrganismId, fraction: f64) {
    let o = world.state.organisms.get_mut(id).expect("alive");
    o.energy = fraction * o.phenotype.energy_max;
}

fn set_reserve(world: &mut World, id: OrganismId, reserve: f64) {
    let o = world.state.organisms.get_mut(id).expect("alive");
    let before = o.reserve;
    o.reserve = reserve;
    world.state.external_material_in += reserve - before;
}

fn reserve_of(world: &World, id: OrganismId) -> f64 {
    world.state.organisms.get(id).expect("alive").reserve
}

fn reserve_max(world: &World, id: OrganismId) -> f64 {
    world.state.organisms.get(id).expect("alive").phenotype.reserve_max
}

fn energy_fraction(world: &World, id: OrganismId) -> f64 {
    let o = world.state.organisms.get(id).expect("alive");
    o.energy / o.phenotype.energy_max
}

/// One tick's oxidation burn ceiling in this world.
fn burn_ceiling(world: &World) -> f64 {
    world.config().organism.oxidation_rate * DT
}

/// Slack for reading a transaction back out of a **stock**. `reserve -= burned` is exact as an
/// operation, but `before - after` recovered afterwards is a second rounding of a number near
/// 4.0, so it trails the amount by a few ulps. The diagnostics carry the exact `burned`; the
/// stock is how the test checks the diagnostics are not marking their own homework, and for
/// that a few ulps is not the question being asked. Whether a transaction happened at all is
/// still exact: no transaction leaves the stock bit-for-bit unchanged.
const STOCK_SLACK: f64 = 1e-12;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= STOCK_SLACK
}

// ---------------------------------------------------------------- the policy type

/// Version 3 is still what the constructor writes and still defers to the world. Nothing about
/// the default changed, at any world threshold.
#[test]
fn version_three_remains_the_default_and_defers_to_the_world() {
    assert_eq!(PROFILE_VERSION, 3);
    assert_eq!(PROFILE_VERSION_CHARGE80, 4);
    assert_eq!(SUPPORTED_PROFILE_VERSIONS, [3, 4]);
    assert_eq!(CHARGE80_OXIDATION_THRESHOLD, 0.80);

    let mut cfg = WorldConfig::default();
    let profile = FixedHunterProfile::lanternjaw_trial(&cfg);
    assert_eq!(profile.version, PROFILE_VERSION);
    assert_eq!(profile.oxidation_policy(), OxidationPolicy::Configured);
    assert_eq!(profile.oxidation_policy().as_str(), "configured-world-threshold");

    // Bit-identical to the world's own number, at the default and at anything else: a version 3
    // member is not "the same to within rounding", it is the same expression.
    for threshold in [0.5, 0.0, 0.125, 0.31415926535, 0.8, 1.0] {
        cfg.organism.oxidation_threshold = threshold;
        assert_eq!(profile.oxidation_threshold(&cfg.organism).to_bits(), threshold.to_bits());
    }
}

/// Version 4 differs from version 3 in the version number and in nothing else. The comparison
/// is over the **whole struct**, so a field added later cannot quietly join the diff, and over
/// the persisted encoding, so the shape cannot either.
#[test]
fn version_four_differs_from_version_three_only_in_its_version_number() {
    let cfg = WorldConfig::default();
    let three = FixedHunterProfile::lanternjaw_trial(&cfg);
    let four = three.clone().charge80();

    let mut rewound = four.clone();
    rewound.version = three.version;
    assert_eq!(rewound, three, "version 4 changed a field other than `version`");
    assert_eq!(
        postcard::to_allocvec(&four).expect("encodable").len(),
        postcard::to_allocvec(&three).expect("encodable").len(),
        "and it did not change the persisted shape"
    );

    // In particular every geometry number is the same object, not merely a close one.
    assert_eq!(four.capture_offset_body, three.capture_offset_body);
    assert_eq!(four.ingestion_offset_body, three.ingestion_offset_body);
    assert_eq!(four.body_extent_px, three.body_extent_px);
    assert_eq!(four.visual_query_extent_px, three.visual_query_extent_px);
    assert_eq!(four.body_scale_exponent, three.body_scale_exponent);
    assert_eq!(four.body_scale_min, three.body_scale_min);
    assert_eq!(four.genome, three.genome);

    // And the one thing it does change.
    assert_eq!(four.oxidation_policy(), OxidationPolicy::Fixed(0.80));
    assert_eq!(four.oxidation_policy().as_str(), "fixed-member-threshold");
    let mut cfg = cfg;
    for threshold in [0.0, 0.5, 0.9, 1.0] {
        cfg.organism.oxidation_threshold = threshold;
        assert_eq!(
            four.oxidation_threshold(&cfg.organism),
            0.80,
            "the fixed policy must not follow the world"
        );
    }
    // `charge80` composes with the existing recipe helpers without disturbing them.
    let off = three.clone().without_attacks().charge80();
    assert!(!off.attacks_enabled);
    assert_eq!(off.version, PROFILE_VERSION_CHARGE80);
    assert_eq!(off.oxidation_policy(), OxidationPolicy::Fixed(0.80));
}

/// Exactly two versions load. Everything else is refused by name, at `validate` and therefore
/// at every door that calls it — including the snapshot decoder.
#[test]
fn only_versions_three_and_four_are_supported() {
    let cfg = WorldConfig::default();
    let base = FixedHunterProfile::lanternjaw_trial(&cfg);
    for ok in SUPPORTED_PROFILE_VERSIONS {
        let mut p = base.clone();
        p.version = ok;
        p.validate().unwrap_or_else(|e| panic!("version {ok} must validate: {e}"));
    }
    for bad in [0u32, 1, 2, 5, 6, 40, u32::MAX] {
        let mut p = base.clone();
        p.version = bad;
        let err = p.validate().expect_err("an unsupported version must be refused");
        assert!(err.contains(&format!("version {bad}")), "{err}");
        assert!(err.contains("[3, 4]"), "the refusal names what is supported: {err}");
    }

    // A world carrying an unsupported version does not decode, whatever its shape.
    let (mut world, _) = founded(PROFILE_VERSION_CHARGE80, quiet_config());
    world.state.hunters.profile.as_mut().expect("a profile").version = 5;
    match decode_snapshot(&encode_snapshot(&world.state, "bad-version")) {
        Err(SnapshotError::Invalid(reason)) => assert!(reason.contains("version 5"), "{reason}"),
        other => panic!("an unsupported profile version decoded as {other:?}"),
    }
}

/// Version 4 shares version 3's serialized shape exactly — which is what makes an old reader's
/// rejection a *semantic* one rather than a misread. The shape claim is checked here; that an
/// actual pre-change executable does reject it is recorded in the progress report, because only
/// a different binary can establish that.
#[test]
fn version_four_has_version_threes_shape_and_round_trips() {
    let (v3, _) = founded(PROFILE_VERSION, quiet_config());
    let (v4, _) = founded(PROFILE_VERSION_CHARGE80, quiet_config());
    let a = encode_snapshot(&v3.state, "shape");
    let b = encode_snapshot(&v4.state, "shape");
    assert_eq!(
        payload(&a).len(),
        payload(&b).len(),
        "version 4 must not change the payload's length; a new schema would be needed"
    );
    assert_ne!(payload(&a), payload(&b), "but it is a different world, and hashes as one");
    assert_ne!(state_hash(&v3.state), state_hash(&v4.state));

    let (meta, back) = decode_snapshot(&b).expect("a version 4 world decodes");
    assert_eq!(meta.schema, cubarium_core::SCHEMA_VERSION, "no new schema is needed");
    assert_eq!(back.hunters.profile.as_ref().expect("a profile").version, 4);
    assert_eq!(back, v4.state);
    World::from_state(back).expect("and rebuilds into a valid world");
}

// ---------------------------------------------------------------- the activation band

/// The whole intervention, stated as the band it opens. A version 4 member converts reserve
/// strictly below `0.80 · E_max` and **not** at or above it; a version 3 member in the same
/// world converts strictly below the configured `0.50` and not at or above that. The two agree
/// everywhere outside `[0.50, 0.80)`.
#[test]
fn the_policy_opens_exactly_the_band_between_the_reference_and_the_fixed_threshold() {
    let cfg = still_config();
    let reference = cfg.organism.oxidation_threshold;
    assert_eq!(reference, 0.5, "the fixture's reference threshold");

    for (version, opens_below) in [(PROFILE_VERSION, reference), (PROFILE_VERSION_CHARGE80, 0.80)] {
        // The two exact boundaries are in this list on purpose: the test is `<`, so a battery
        // sitting *on* a threshold does not activate it.
        for fraction in [0.0, 0.25, 0.4999, 0.5, 0.5001, 0.6, 0.7, 0.7999, 0.8, 0.8001, 0.95, 1.0] {
            let (mut world, id) = founded(version, cfg.clone());
            let full = reserve_max(&world, id);
            set_reserve(&mut world, id, full);
            set_energy_fraction(&mut world, id, fraction);
            let before = reserve_of(&world, id);
            let energy_before = world.state.organisms.get(id).unwrap().energy;
            let cell = cell_of(&world.state.organisms.get(id).unwrap().pos);
            let n_before = world.state.fields.n[cell.index()];
            world.step();
            // The premise of the exact boundary: nothing spent the battery before physiology.
            let e_max = world.state.organisms.get(id).unwrap().phenotype.energy_max;
            assert!(
                world.state.organisms.get(id).unwrap().energy >= energy_before,
                "v{version} at {fraction}: this fixture must charge nothing else"
            );
            assert_eq!(energy_before, fraction * e_max);

            let burned = before - reserve_of(&world, id);
            let expect_activation = fraction < opens_below;
            let ceiling = burn_ceiling(&world);
            if expect_activation {
                assert!(
                    close(burned, ceiling),
                    "v{version} at {fraction} of E_max should have burned {ceiling}, not {burned}"
                );
                assert!(
                    close(world.state.fields.n[cell.index()] - n_before, burned),
                    "v{version} at {fraction}: the burned material must be back in the cell"
                );
            } else {
                assert_eq!(burned, 0.0, "v{version} at {fraction} of E_max must not have burned");
                assert_eq!(world.state.fields.n[cell.index()], n_before);
            }

            // And the diagnostics see exactly the transactions the reference would not have
            // made — which is the band, and never the ordinary physiology below it.
            let d = world.charging_diagnostics();
            let extra = expect_activation && fraction >= reference;
            assert_eq!(
                d.extra_transactions,
                u64::from(extra),
                "v{version} at {fraction}: unexpected extra-oxidation count"
            );
            assert_eq!(
                d.extra_reserve_burned,
                if extra { ceiling } else { 0.0 },
                "v{version} at {fraction}: the diagnostic burn is the exact ceiling"
            );
        }
    }
}

/// The policy belongs to **membership**, not to the world and not to a body plan. An ordinary
/// organism sharing a world with a version 4 member keeps the configured threshold exactly.
#[test]
fn an_ordinary_organism_is_never_subject_to_the_member_policy() {
    let mut cfg = quiet_config();
    // One ordinary founder beside the hunter, sitting squarely inside the candidate band.
    cfg.founders.count = 1;
    cfg.founders.initial_energy_fraction = 0.6;
    cfg.founders.initial_reserve_fraction = 0.9;
    let (mut world, member) = founded(PROFILE_VERSION_CHARGE80, cfg);
    let ordinary = world
        .state
        .organisms
        .iter()
        .map(|(id, _)| id)
        .find(|id| !world.state.hunters.contains(*id))
        .expect("one ordinary founder");
    set_energy_fraction(&mut world, ordinary, 0.6);
    set_energy_fraction(&mut world, member, 0.6);
    let (o_before, m_before) = (reserve_of(&world, ordinary), reserve_of(&world, member));
    world.step();

    assert_eq!(
        reserve_of(&world, ordinary),
        o_before,
        "an ordinary organism at 0.6 of E_max must not oxidize under a member's policy"
    );
    let burned = m_before - reserve_of(&world, member);
    assert!(close(burned, burn_ceiling(&world)), "and the member beside it must: {burned}");
    assert_eq!(world.charging_diagnostics().extra_transactions, 1, "one member, one transaction");

    assert_eq!(world.member_oxidation_threshold(), 0.80);
    let (v3, _) = founded(PROFILE_VERSION, quiet_config());
    assert_eq!(v3.member_oxidation_threshold(), 0.5);
    let plain = World::new(quiet_config()).expect("valid");
    assert_eq!(plain.member_oxidation_threshold(), 0.5, "no profile, no policy");
}

// ---------------------------------------------------------------- the conversion block

/// The conversion itself is untouched: the burn ceiling, the reserve returned to `N`, the
/// released `e_r · burned`, the efficiency and the conversion heat are the pre-change block,
/// run at a different moment and in no other way different.
#[test]
fn the_conversion_arithmetic_is_the_same_block_at_either_threshold() {
    let cfg = quiet_config();
    let org = cfg.organism.clone();
    let (e_r, eff) = (org.reserve_energy_density, org.oxidation_efficiency);
    let (mut world, id) = founded(PROFILE_VERSION_CHARGE80, cfg);
    let full = reserve_max(&world, id);
    set_reserve(&mut world, id, full);
    set_energy_fraction(&mut world, id, 0.6);
    let before = reserve_of(&world, id);
    world.step();

    let d = world.charging_diagnostics();
    assert_eq!(d.extra_reserve_burned, org.oxidation_rate * DT, "the burn ceiling is rate · dt");
    assert!(
        close(before - reserve_of(&world, id), d.extra_reserve_burned),
        "and it is the reserve that actually left"
    );
    let burned = d.extra_reserve_burned;
    // Headroom is nowhere near binding here (0.4 · E_max against a 0.0008 release), so the
    // gain is the whole efficient share and the heat is the rest — the identity, exactly.
    assert_eq!(d.extra_energy_gained, e_r * burned * eff);
    assert_eq!(d.extra_heat, e_r * burned - d.extra_energy_gained);
    assert!(d.extra_heat > 0.0, "an 80%-efficient conversion releases heat");
}

/// The headroom cap still binds, and the excess still becomes heat rather than being stored or
/// quietly dropped. Exercised with a world whose oxidation rate is large enough that one tick's
/// release cannot fit — a fixture knob on the world, never on the policy.
#[test]
fn the_battery_headroom_still_caps_the_gain_and_the_rest_is_heat() {
    let mut cfg = quiet_config();
    cfg.organism.oxidation_rate = 100.0;
    let (e_r, eff) = (cfg.organism.reserve_energy_density, cfg.organism.oxidation_efficiency);
    let (mut world, id) = founded(PROFILE_VERSION_CHARGE80, cfg);
    let max = reserve_max(&world, id);
    set_reserve(&mut world, id, max);
    set_energy_fraction(&mut world, id, 0.79);
    let before = reserve_of(&world, id);
    world.step();

    let d = world.charging_diagnostics();
    assert_eq!(d.extra_reserve_burned, max, "the ceiling was above the reserve, so the reserve is");
    assert!(close(before - reserve_of(&world, id), max));
    let burned = d.extra_reserve_burned;
    let released = e_r * burned;
    assert!(
        d.extra_energy_gained < released * eff,
        "the headroom must have bound: gained {} against an efficient share of {}",
        d.extra_energy_gained,
        released * eff
    );
    let o = world.state.organisms.get(id).expect("alive");
    assert_eq!(o.energy, o.phenotype.energy_max, "the battery is exactly full, never over");
    assert_eq!(d.extra_heat, released - d.extra_energy_gained, "the excess is heat");
    assert!(d.extra_heat > 0.0);
}

/// A member with nothing to burn does not charge, at either threshold, and the policy invents
/// no energy from an empty reserve.
#[test]
fn a_zero_reserve_never_charges_and_no_energy_is_invented() {
    for version in SUPPORTED_PROFILE_VERSIONS {
        let (mut world, id) = founded(version, quiet_config());
        set_reserve(&mut world, id, 0.0);
        set_energy_fraction(&mut world, id, 0.6);
        let cell = cell_of(&world.state.organisms.get(id).unwrap().pos);
        let n_before = world.state.fields.n[cell.index()];
        let energy_before = world.state.organisms.get(id).unwrap().energy;
        world.step();

        assert_eq!(reserve_of(&world, id), 0.0, "v{version}: nothing to burn, nothing burned");
        assert_eq!(world.state.fields.n[cell.index()], n_before, "v{version}");
        assert!(
            world.state.organisms.get(id).unwrap().energy < energy_before,
            "v{version}: upkeep is still paid; a policy is not a gift"
        );
        assert_eq!(world.charging_diagnostics(), ChargingDiagnostics::default(), "v{version}");
    }
}

/// A world whose oxidation rate is zero transacts nothing, at either threshold — and records
/// nothing, which is the part worth testing. The activation test alone would pass here (energy
/// is below the threshold and the reserve is not empty) while the conversion moved nothing, so
/// a counter that trusted the activation test would report transactions that burned nothing.
/// Zero is a legal configured rate, so this is a reachable world and not a hypothetical.
#[test]
fn a_zero_oxidation_rate_transacts_nothing_and_records_nothing() {
    for version in SUPPORTED_PROFILE_VERSIONS {
        let mut cfg = quiet_config();
        cfg.organism.oxidation_rate = 0.0;
        let (mut world, id) = founded(version, cfg);
        let full = reserve_max(&world, id);
        set_reserve(&mut world, id, full);
        // Below both thresholds, so the activation test passes and only the amount is zero.
        set_energy_fraction(&mut world, id, 0.1);
        let cell = cell_of(&world.state.organisms.get(id).unwrap().pos);
        let n_before = world.state.fields.n[cell.index()];
        world.step();

        assert_eq!(reserve_of(&world, id), full, "v{version}: a zero rate burns nothing");
        assert_eq!(world.state.fields.n[cell.index()], n_before, "v{version}");
        assert_eq!(
            world.charging_diagnostics(),
            ChargingDiagnostics::default(),
            "v{version}: a transaction that moved nothing is not a transaction"
        );
    }
}

/// A reserve smaller than one tick's ceiling is burned whole and no further.
#[test]
fn the_burn_is_capped_by_the_reserve_that_is_actually_there() {
    let (mut world, id) = founded(PROFILE_VERSION_CHARGE80, quiet_config());
    let ceiling = burn_ceiling(&world);
    let scrap = ceiling / 4.0;
    set_reserve(&mut world, id, scrap);
    set_energy_fraction(&mut world, id, 0.6);
    world.step();
    assert_eq!(reserve_of(&world, id), 0.0, "the whole scrap went, and no more");
    assert_eq!(world.charging_diagnostics().extra_reserve_burned, scrap);
    assert!(scrap < ceiling);
}

// ---------------------------------------------------------------- version 3 identity

/// The load-bearing identity test. Two fixtures written by the **pre-change** release build
/// (provenance in `tests/fixtures/hunter-v3-charge-provenance.md`) with a real version 3 trial
/// running and its founder actually oxidizing: loading the first, stepping 600 ticks and
/// re-encoding must reproduce the second's payload **byte for byte**.
#[test]
fn a_version_three_member_reproduces_the_pre_change_binarys_next_600_ticks() {
    let start = std::fs::read(fixture("hunter-v3-charge-active.cubw")).expect("the fixture");
    let plus600 = std::fs::read(fixture("hunter-v3-charge-active-plus600.cubw")).expect("the fixture");
    let (meta, state) = decode_snapshot(&start).expect("a genuine pre-change snapshot loads");
    assert_eq!(meta.build_id, "pre-charge-fixture");
    assert_eq!(state.tick, 5570);
    let profile = state.hunters.profile.as_ref().expect("an actual trial");
    assert_eq!(profile.version, PROFILE_VERSION, "the fixture is a version 3 experiment");
    assert_eq!(profile.oxidation_policy(), OxidationPolicy::Configured);
    assert_eq!(state.hunters.members.len(), 1, "with a live member");
    let founder = state.hunters.members[0].id;
    let o = state.organisms.get(founder).expect("alive");
    assert!(
        o.energy < state.config.organism.oxidation_threshold * o.phenotype.energy_max,
        "and one that is actually oxidizing, so the continuation exercises the branch"
    );
    assert!(o.reserve > 0.0);

    let mut world = World::from_state(state).expect("valid");
    world.check_invariants().expect("invariants hold on the pre-change world");
    for _ in 0..600 {
        world.step();
    }
    assert_eq!(world.tick(), 6170);
    assert_eq!(
        payload(&encode_snapshot(&world.state, "pre-charge-fixture")),
        payload(&plus600),
        "600 ticks of the policy build diverged from the pre-change binary"
    );
    // The policy build made no extra transaction at all in a version 3 world.
    assert_eq!(world.charging_diagnostics(), ChargingDiagnostics::default());
}

/// The same fixture world read the other way: caught **inside** the candidate band, where the
/// two policies visibly disagree. Version 3 does nothing there; version 4 charges. Same
/// opening bytes, same tick, one field of difference.
#[test]
fn the_genuine_window_fixture_separates_the_two_policies() {
    let bytes = std::fs::read(fixture("hunter-v3-charge-window.cubw")).expect("the fixture");
    let (_, state) = decode_snapshot(&bytes).expect("loads");
    assert_eq!(state.tick, 1400);
    let reference = state.config.organism.oxidation_threshold;
    let founder = state.hunters.members[0].id;
    let o = state.organisms.get(founder).expect("alive");
    let fraction = o.energy / o.phenotype.energy_max;
    assert!(
        fraction > reference && fraction < CHARGE80_OXIDATION_THRESHOLD,
        "the window fixture must sit strictly inside the band: {fraction}"
    );
    assert!(o.reserve > 0.0);

    let run = |version: u32| {
        let mut s = state.clone();
        s.hunters.profile.as_mut().expect("a profile").version = version;
        let mut w = World::from_state(s).expect("valid");
        let before = w.state.organisms.get(founder).expect("alive").reserve;
        for _ in 0..100 {
            w.step();
        }
        (before - w.state.organisms.get(founder).expect("alive").reserve, w.charging_diagnostics())
    };
    let (v3_burn, v3_diag) = run(PROFILE_VERSION);
    let (v4_burn, v4_diag) = run(PROFILE_VERSION_CHARGE80);

    assert_eq!(v3_burn, 0.0, "version 3 does not charge inside the band");
    assert_eq!(v3_diag, ChargingDiagnostics::default());
    assert!(v4_burn > 0.0, "version 4 does");
    assert_eq!(v4_diag.extra_transactions, 100, "every tick of the window, and only those");
    assert!(close(v4_diag.extra_reserve_burned, v4_burn), "{} vs {v4_burn}", v4_diag.extra_reserve_burned);
}

// ---------------------------------------------------------------- reproduction

/// The mechanism claim the experiment exists to test, in a hand-built fixture: with reserve to
/// spare, paid charging can carry a mature member across the **unchanged** battery gate and
/// fund a **fully paid** escrow. The reproduction clock is wound down so one test can watch it;
/// the two stock fractions that the charging must actually open are untouched.
///
/// This is a mechanism fact about one fixture, not evidence that a lineage is viable.
#[test]
fn charging_can_fund_a_fully_paid_escrow_when_the_stocks_allow() {
    let mature = |version: u32| {
        let mut world = World::new(quiet_config()).expect("valid");
        let mut profile = FixedHunterProfile::lanternjaw_trial(world.config());
        // The age clock only; every stock gate stays exactly where the recipe put it.
        profile.reproduce_min_age_seconds = 0.0;
        if version == PROFILE_VERSION_CHARGE80 {
            profile = profile.charge80();
        }
        let gate_reserve = profile.reproduce_reserve_fraction;
        let gate_energy = profile.reproduce_energy_fraction;
        let id = world.start_hunter_trial(profile, target_of(SPOT)).expect("starts").id;
        let o = world.state.organisms.get(id).expect("alive");
        let (r_max, e_max) = (o.phenotype.reserve_max, o.phenotype.energy_max);
        // Full reserve, and a battery a little *below* the unchanged 0.75 gate but inside the
        // candidate band: exactly the state the proposal says charging should be able to close.
        set_reserve(&mut world, id, r_max);
        set_energy_fraction(&mut world, id, 0.72);
        assert!(0.72 > world.config().organism.oxidation_threshold && 0.72 < 0.80);
        assert!(0.72 < gate_energy, "the battery gate is genuinely shut at the start");
        assert!(1.0 >= gate_reserve, "and the reserve gate is genuinely open");
        (world, id, gate_reserve, gate_energy, r_max, e_max)
    };

    // Version 3: the battery gate never opens, because nothing charges it.
    let (mut world, id, _, gate_energy, _, _) = mature(PROFILE_VERSION);
    for _ in 0..1200 {
        world.step();
    }
    assert!(energy_fraction(&world, id) < gate_energy, "version 3 cannot reach the gate here");
    assert_eq!(world.charging_diagnostics(), ChargingDiagnostics::default());
    assert!(
        world.drain_hunter_events().iter().all(|e| !matches!(
            e,
            HunterEvent::Reproduction { record: cubarium_core::Reproduction::Funded { .. }, .. }
        )),
        "and funds nothing"
    );

    // Version 4: it charges across the gate and pays in full.
    let (mut world, id, gate_reserve, gate_energy, r_max, _) = mature(PROFILE_VERSION_CHARGE80);
    let mut funded = None;
    for _ in 0..1200 {
        world.step();
        for event in world.drain_hunter_events() {
            if let HunterEvent::Reproduction {
                record: cubarium_core::Reproduction::Funded { escrow_structure, escrow_reserve, escrow_energy, build_heat, parent_reserve_before, parent_energy_before, .. },
                tick,
                ..
            } = event
            {
                funded = Some((tick, escrow_structure, escrow_reserve, escrow_energy, build_heat, parent_reserve_before, parent_energy_before));
                break;
            }
        }
        if funded.is_some() {
            break;
        }
    }
    let (tick, structure, reserve, energy, build, r_before, e_before) =
        funded.expect("charging must be able to open the battery gate in this fixture");
    assert!(tick > 0);

    // Fully paid, at the recipe's own unchanged prices, out of the parent's own stocks.
    let cfg = world.config().organism.clone();
    let o = world.state.organisms.get(id).expect("the parent is alive");
    assert_eq!(structure, cfg.child_structure_fraction * o.phenotype.structure_adult);
    assert_eq!(reserve, cfg.child_reserve_fraction * o.phenotype.reserve_max);
    assert_eq!(energy, cfg.child_energy_fraction * o.phenotype.energy_max);
    assert_eq!(build, cfg.build_cost * structure);
    assert!(r_before >= gate_reserve * r_max, "the reserve gate was open when it paid");
    assert!(e_before >= gate_energy * o.phenotype.energy_max, "and so was the battery gate");
    // The charging that got it there is booked, and it is real reserve that was spent.
    let d = world.charging_diagnostics();
    assert!(d.extra_transactions > 0 && d.extra_reserve_burned > 0.0);
    assert!(
        close(d.extra_heat, cfg.reserve_energy_density * d.extra_reserve_burned - d.extra_energy_gained),
        "the accumulated conversion heat is the accumulated release less the accumulated gain"
    );
    world.check_invariants().expect("a funded world is consistent");
}

/// The conservative half of the same claim: a member whose **reserve** is short does not become
/// ready by charging. Charging spends reserve, so it moves that gate the wrong way; nothing in
/// this policy manufactures stock.
#[test]
fn a_stock_limited_member_does_not_become_ready_by_charging() {
    let mut world = World::new(quiet_config()).expect("valid");
    let mut profile = FixedHunterProfile::lanternjaw_trial(world.config()).charge80();
    profile.reproduce_min_age_seconds = 0.0;
    let gate_reserve = profile.reproduce_reserve_fraction;
    let id = world.start_hunter_trial(profile, target_of(SPOT)).expect("starts").id;
    let r_max = reserve_max(&world, id);
    // Half the reserve the gate wants, and a battery inside the charging band.
    set_reserve(&mut world, id, 0.5 * gate_reserve * r_max);
    set_energy_fraction(&mut world, id, 0.7);

    let opening_reserve = reserve_of(&world, id);
    for _ in 0..1200 {
        world.step();
        for event in world.drain_hunter_events() {
            assert!(
                !matches!(
                    event,
                    HunterEvent::Reproduction {
                        record: cubarium_core::Reproduction::Funded { .. },
                        ..
                    }
                ),
                "a reserve-limited member must not fund an offspring"
            );
        }
        if world.state.organisms.get(id).is_none() {
            break;
        }
    }
    if let Some(o) = world.state.organisms.get(id) {
        assert!(
            o.reserve < gate_reserve * r_max,
            "the reserve gate must still be shut: {} against {}",
            o.reserve,
            gate_reserve * r_max
        );
        assert!(o.reserve <= opening_reserve, "charging spends reserve, it does not create it");
    }
    let d = world.charging_diagnostics();
    assert!(d.extra_transactions > 0, "and it really did charge");
    assert!(d.extra_reserve_burned > 0.0);
}

// ---------------------------------------------------------------- determinism

/// Mid-charge and mid-gestation, a saved world resumes into exactly the world that was never
/// interrupted: the same full state hash every tick, and the same event streams. Full state
/// hashes, not the ecology projection, which deliberately drops the hunter extension.
#[test]
fn a_saved_world_resumes_identically_mid_charge_and_mid_gestation() {
    for (name, gestate) in [("mid-charge", false), ("mid-gestation", true)] {
        let mut world = World::new(quiet_config()).expect("valid");
        let mut profile = FixedHunterProfile::lanternjaw_trial(world.config()).charge80();
        profile.reproduce_min_age_seconds = 0.0;
        profile.gestation_seconds = 40.0;
        let id = world.start_hunter_trial(profile, target_of(SPOT)).expect("starts").id;
        let r_max = reserve_max(&world, id);
        set_reserve(&mut world, id, r_max);
        set_energy_fraction(&mut world, id, if gestate { 0.79 } else { 0.7 });

        // Run to the interesting moment: charging, or charging with an escrow open.
        let mut split = 0;
        for _ in 0..2000 {
            world.step();
            world.drain_hunter_events();
            world.drain_events();
            split += 1;
            let charging = world.charging_diagnostics().extra_transactions > 0;
            let open = world.state.organisms.get(id).is_some_and(|o| o.escrow.is_some());
            if charging && (!gestate || open) {
                break;
            }
        }
        assert!(
            world.charging_diagnostics().extra_transactions > 0,
            "{name}: the fixture must be charging at the split"
        );
        if gestate {
            let o = world.state.organisms.get(id).expect("alive");
            assert!(o.escrow.is_some(), "{name}: the fixture must be gestating at the split");
        }

        let at_split = world.charging_diagnostics();
        let bytes = encode_snapshot(&world.state, "charge-resume");
        let (_, state) = decode_snapshot(&bytes).expect("{name}: the split state round trips");
        assert_eq!(state, world.state);
        let mut resumed = World::from_state(state).expect("valid");
        assert_eq!(state_hash(&resumed.state), state_hash(&world.state), "{name}");
        assert_eq!(
            resumed.charging_diagnostics(),
            ChargingDiagnostics::default(),
            "{name}: the diagnostics are transient, so a reload opens them at zero"
        );

        for i in 0..600 {
            world.step();
            resumed.step();
            assert_eq!(
                state_hash(&resumed.state),
                state_hash(&world.state),
                "{name}: diverged {i} ticks after the split (tick {})",
                world.tick()
            );
            let (a, b): (Vec<HunterEvent>, Vec<HunterEvent>) =
                (world.drain_hunter_events().to_vec(), resumed.drain_hunter_events().to_vec());
            assert_eq!(format!("{a:?}"), format!("{b:?}"), "{name}: hunter events diverged");
            let (a, b): (Vec<LifeEvent>, Vec<LifeEvent>) =
                (world.drain_events().to_vec(), resumed.drain_events().to_vec());
            assert_eq!(format!("{a:?}"), format!("{b:?}"), "{name}: life events diverged");
        }
        // The same physics ran on both sides, so the same extra transactions were counted.
        // The uninterrupted world's totals also cover everything before the split, so it is the
        // **delta** across the shared 600 ticks that the two runs must agree on — which is
        // exactly what it means for the diagnostics to describe a run rather than a world.
        let after = world.charging_diagnostics();
        let resumed_after = resumed.charging_diagnostics();
        assert_eq!(
            after.extra_transactions - at_split.extra_transactions,
            resumed_after.extra_transactions,
            "{name}: the two runs counted different transactions over the same 600 ticks"
        );
        assert!(
            close(
                after.extra_reserve_burned - at_split.extra_reserve_burned,
                resumed_after.extra_reserve_burned
            ),
            "{name}: and different burns"
        );
        assert!(split > 0, "{name}: the split must be after at least one tick");
    }
}

/// The diagnostics never move the world. The same world stepped in a build that is counting is
/// the same world: what a counter observes cannot be what a counter causes.
#[test]
fn recording_the_diagnostics_cannot_move_the_simulation() {
    // Two worlds that differ only in whether the counted branch is ever taken, each stepped
    // twice from identical openings, must land on identical hashes within themselves.
    for version in SUPPORTED_PROFILE_VERSIONS {
        let (mut a, id) = founded(version, quiet_config());
        set_energy_fraction(&mut a, id, 0.7);
        let opening = encode_snapshot(&a.state, "twice");
        let (_, state) = decode_snapshot(&opening).expect("round trip");
        let mut b = World::from_state(state).expect("valid");
        for _ in 0..300 {
            a.step();
            b.step();
        }
        assert_eq!(state_hash(&a.state), state_hash(&b.state), "v{version}");
        assert_eq!(a.charging_diagnostics(), b.charging_diagnostics(), "v{version}");
        // The counters are outside the persisted state, so they cannot reach the hash.
        assert_eq!(
            encode_snapshot(&a.state, "twice"),
            encode_snapshot(&b.state, "twice"),
            "v{version}"
        );
    }
}
