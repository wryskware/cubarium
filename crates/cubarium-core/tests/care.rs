//! Optional care: the invariants of `design/7_Research/care-contract-2026-09-12.md`
//! (revision 2).
//!
//! The load-bearing test here is [`zero_care_reproduces_the_pre_change_binarys_next_600_ticks`]:
//! two fixtures produced by the *pre-change* schema 7 release build prove that schema 8
//! reads a live world, steps it, and writes back the same ecological bytes. Everything else
//! checks that a command does exactly what the contract says and nothing else.

use std::path::PathBuf;

use cubarium_core::care::{
    ActiveShower, CLEAN_MATERIAL, CareCommand, CareKind, CareOutcome, CareState, CareTarget,
    FEED_ALLOWANCE, FEED_MATERIAL, RAIN_DEPTH_TOTAL, RAIN_TICKS, footprint, rain_envelope,
};
use cubarium_core::snapshot::{HEADER_FIXED_BYTES, state_hash, v7};
use cubarium_core::world::WorldState;
use cubarium_core::{
    SCHEMA_V7, SCHEMA_VERSION, World, WorldConfig, decode_snapshot, ecology_hash, encode_snapshot,
};
use cubarium_surface::{CellId, FACE_EXTENT, Face, FieldGraph, cell_of};

// ---------------------------------------------------------------- helpers

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// The postcard payload of a snapshot file: everything after the variable-length header.
fn payload(bytes: &[u8]) -> &[u8] {
    let id_len = usize::from(u16::from_le_bytes(bytes[8..10].try_into().expect("2 bytes")));
    &bytes[HEADER_FIXED_BYTES + id_len..]
}

/// FNV-1a 64, the same hash `state_hash` and `ecology_hash` use, computed here from the
/// fixture's own bytes so the test does not take the crate's word for it.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// `design/m2-world-spec.md` "Units and quantities", recomputed from public state so the
/// energy identity is checked against the spec and not against the world's own helper.
fn stored_energy(state: &WorldState) -> f64 {
    let e_p = state.config.producer.energy_density;
    let e_f = state.config.fruit.energy_density;
    let e_r = state.config.organism.reserve_energy_density;
    let cells: f64 = state.fields.p.iter().map(|p| e_p * p).sum::<f64>()
        + state.fields.f.iter().map(|f| e_f * f).sum::<f64>()
        + state.fields.de.iter().sum::<f64>();
    let organisms: f64 = state
        .organisms
        .iter()
        .map(|(_, o)| {
            o.energy
                + e_r * o.reserve
                + o.escrow.as_ref().map_or(0.0, |e| e_r * (e.structure + e.reserve) + e.energy)
        })
        .sum();
    cells + organisms
}

fn target_of(cell: CellId) -> CareTarget {
    let c = cell.center();
    CareTarget { face: c.face.index() as u8, u: c.u, v: c.v }
}

fn command(seq: u64, tick: u64, kind: CareKind, cell: CellId) -> CareCommand {
    CareCommand { seq, apply_after_tick: tick, kind, target: target_of(cell) }
}

/// A world with no natural rain and no flow or evaporation, so every drop of water in it
/// came from a shower and stays in the cell it fell on. Nothing else is changed.
fn still_water_config() -> WorldConfig {
    let mut cfg = WorldConfig::default();
    cfg.water.rain_rate = 0.0;
    cfg.water.flow = 0.0;
    cfg.water.evap = 0.0;
    cfg
}

// ---------------------------------------------------------------- migration

/// The live world loads as schema 7, gains an inert `care`, and its projection is the
/// original payload byte for byte: the migration adds a field and reinterprets nothing.
#[test]
fn the_live_schema_seven_snapshot_migrates_without_touching_its_ecology() {
    let bytes = std::fs::read(fixture("live-v7-55200.cubw")).expect("the fixture is committed");
    let (meta, state) = decode_snapshot(&bytes).expect("a genuine schema 7 snapshot loads");
    assert_eq!(meta.schema, SCHEMA_V7, "the header still reports what was read");
    assert_ne!(SCHEMA_VERSION, SCHEMA_V7);
    assert_eq!(state.tick, 55_200);
    assert_eq!(state.care, CareState::default(), "a migrated world has never been cared for");
    assert!(!state.organisms.is_empty(), "the fixture carries a living population");

    let projected = postcard::to_allocvec(&v7::project(&state)).expect("encodable");
    assert_eq!(projected, payload(&bytes), "re-encoding the projection is not the original payload");
    assert_eq!(ecology_hash(&state), fnv1a(payload(&bytes)));
    // The full-state hash is a different hash: it covers the appended care field.
    assert_ne!(state_hash(&state), ecology_hash(&state));

    // The world it builds is consistent and its residual opens at zero.
    let world = World::from_state(state).expect("the migrated state is a valid world");
    world.check_invariants().expect("invariants hold on the migrated world");
    assert!(world.mass_residual().abs() < 1e-9, "residual {}", world.mass_residual());
}

/// The cross-version proof. Both fixtures were produced by the pre-change schema 7 release
/// binary; stepping the migrated world 600 ticks with zero care must land on the second
/// fixture's payload exactly. If this fails, the new code changed an operation somewhere in
/// the tick and nothing else in this file means anything.
#[test]
fn zero_care_reproduces_the_pre_change_binarys_next_600_ticks() {
    let start = std::fs::read(fixture("live-v7-55200.cubw")).expect("fixture");
    let plus600 = std::fs::read(fixture("live-v7-55200-plus600.cubw")).expect("fixture");
    let (_, state) = decode_snapshot(&start).expect("schema 7 loads");
    let (meta, expected) = decode_snapshot(&plus600).expect("schema 7 loads");
    assert_eq!(meta.schema, SCHEMA_V7);
    assert_eq!(expected.tick, 55_800);

    let mut world = World::from_state(state).expect("valid");
    for _ in 0..600 {
        world.step();
    }
    assert_eq!(world.tick(), 55_800);
    assert_eq!(world.care(), &CareState::default(), "no command was ever admitted");

    let projected = postcard::to_allocvec(&v7::project(&world.state)).expect("encodable");
    assert_eq!(
        projected,
        payload(&plus600),
        "600 ticks of the new build diverged from the pre-change binary"
    );
    assert_eq!(ecology_hash(&world.state), fnv1a(payload(&plus600)));
    let sample = world.telemetry();
    assert_eq!(sample.ecology_hash, fnv1a(payload(&plus600)));
    assert_eq!(sample.care_admitted_seq, 0);
}

// ---------------------------------------------------------------- feed and clean

#[test]
fn feed_and_clean_keep_the_mass_and_energy_identities_exactly() {
    let mut world = World::new(WorldConfig::default()).expect("defaults are valid");
    for _ in 0..200 {
        world.step();
    }
    let cell = CellId::new(Face::Front, 8, 8);
    let cap = world.config().detritus.energy_cap;
    let rho = cap;

    let residual_before = world.mass_residual();
    let energy_before = stored_energy(&world.state);
    let d_cells_before: Vec<f64> = world.state.fields.d.clone();
    let de_cells_before: Vec<f64> = world.state.fields.de.clone();
    let d_before: f64 = world.state.fields.d.iter().sum();
    let de_before: f64 = world.state.fields.de.iter().sum();
    let n_before: Vec<f64> = world.state.fields.n.clone();
    let p_before: Vec<f64> = world.state.fields.p.clone();
    let f_before: Vec<f64> = world.state.fields.f.clone();
    let w_before: Vec<f64> = world.state.fields.w.clone();
    let population_before = world.population();

    // Feed: `D += m·w_c`, `De += rho·m·w_c`, booked as the actual f64 sums.
    let receipt = world.apply_care(&command(1, world.tick(), CareKind::Feed, cell));
    assert_eq!(receipt.seq, 1);
    assert_eq!(receipt.tick, world.tick());
    let CareOutcome::Applied(q) = receipt.outcome.clone() else {
        panic!("feed was not applied: {:?}", receipt.outcome)
    };
    assert_eq!(q.cells, 5, "an interior feed footprint is the cell and four neighbours");
    assert!((q.material_in - FEED_MATERIAL).abs() < 1e-12, "{}", q.material_in);
    assert!((q.energy_in - rho * FEED_MATERIAL).abs() < 1e-12, "{}", q.energy_in);
    assert_eq!(q.material_out, 0.0);
    assert_eq!(q.energy_out, 0.0);
    assert_eq!(q.ends_tick, None);
    assert_eq!(world.care().feed_material_in, q.material_in, "the ledger is the actual sum");
    assert_eq!(world.care().feed_energy_in, q.energy_in);
    assert_eq!(world.care().allowance_used, q.material_in);

    // Only D and De moved, only over the footprint, and each cell got exactly its share.
    let graph = FieldGraph::new();
    let fp = footprint(&graph, cell, 1);
    let mut inside = vec![false; cubarium_surface::CELL_COUNT];
    for (c, w) in &fp {
        let i = c.index();
        inside[i] = true;
        assert!((world.state.fields.d[i] - d_cells_before[i] - FEED_MATERIAL * w).abs() < 1e-15);
        assert!((world.state.fields.de[i] - de_cells_before[i] - rho * (FEED_MATERIAL * w)).abs() < 1e-15);
    }
    for i in 0..cubarium_surface::CELL_COUNT {
        if !inside[i] {
            assert_eq!(world.state.fields.d[i], d_cells_before[i], "cell {i} is outside the footprint");
            assert_eq!(world.state.fields.de[i], de_cells_before[i], "cell {i} is outside the footprint");
        }
    }
    assert_eq!(world.state.fields.n, n_before, "feeding moved nutrient");
    assert_eq!(world.state.fields.p, p_before, "feeding moved producer");
    assert_eq!(world.state.fields.f, f_before, "feeding moved fruit");
    assert_eq!(world.state.fields.w, w_before, "feeding moved water");
    assert_eq!(world.population(), population_before);
    let d_after: f64 = world.state.fields.d.iter().sum();
    let de_after: f64 = world.state.fields.de.iter().sum();
    assert!((d_after - d_before - q.material_in).abs() < 1e-12);
    assert!((de_after - de_before - q.energy_in).abs() < 1e-12);

    // The identities, both of them.
    assert!(
        (world.mass_residual() - residual_before).abs() < 1e-9,
        "mass residual moved from {residual_before} to {}",
        world.mass_residual()
    );
    let booked = world.care().feed_energy_in - world.care().clean_energy_out;
    assert!(
        ((stored_energy(&world.state) - energy_before) - booked).abs() < 1e-9,
        "stored energy is not opening + feed − clean"
    );
    // `De ≤ energy_cap · D` everywhere, which `check_invariants` enforces cell by cell.
    world.check_invariants().expect("fed crumbs keep De within the cap");

    // Clean: export D and the energy it carried at the cell's own density.
    let energy_mid = stored_energy(&world.state);
    let receipt = world.apply_care(&command(2, world.tick(), CareKind::Clean, cell));
    let q2 = match receipt.outcome.clone() {
        CareOutcome::Applied(q) | CareOutcome::Partial(q) => q,
        other => panic!("clean was rejected: {other:?}"),
    };
    assert_eq!(q2.cells, 5);
    assert!(q2.material_out > 0.0 && q2.material_out <= CLEAN_MATERIAL + 1e-12);
    assert!(q2.energy_out > 0.0);
    assert_eq!(q2.material_in, 0.0);
    assert_eq!(q2.energy_in, 0.0);
    assert_eq!(world.care().clean_material_out, q2.material_out);
    assert_eq!(world.care().clean_energy_out, q2.energy_out);
    assert_eq!(world.state.fields.n, n_before, "cleaning moved nutrient");
    assert_eq!(world.state.fields.p, p_before, "cleaning moved producer");
    assert_eq!(world.state.fields.f, f_before, "cleaning moved fruit");
    assert_eq!(world.state.fields.w, w_before, "cleaning moved water");
    assert_eq!(world.population(), population_before);
    assert!(
        (world.mass_residual() - residual_before).abs() < 1e-9,
        "mass residual moved to {}",
        world.mass_residual()
    );
    assert!(
        ((stored_energy(&world.state) - energy_mid) + q2.energy_out).abs() < 1e-9,
        "the cleaned energy left the world exactly once"
    );
    let booked = world.care().feed_energy_in - world.care().clean_energy_out;
    assert!(((stored_energy(&world.state) - energy_before) - booked).abs() < 1e-9);
    world.check_invariants().expect("cleaning keeps De within the cap");

    // The allowance: feeding spends it, cleaning gives it back, never below zero.
    let expected = (world.care().feed_material_in - world.care().clean_material_out).max(0.0);
    assert!((world.care().allowance_used - expected).abs() < 1e-12);
}

/// Feed until the allowance refuses, and report how many went in.
fn feed_until_exhausted(world: &mut World, cell: CellId, from_seq: u64) -> (u64, u32, String) {
    let mut seq = from_seq;
    let mut accepted = 0;
    loop {
        seq += 1;
        match world.apply_care(&command(seq, 0, CareKind::Feed, cell)).outcome {
            CareOutcome::Applied(_) => accepted += 1,
            CareOutcome::Rejected(reason) => return (seq, accepted, reason),
            other => panic!("a feed came back {other:?}"),
        }
        assert!(seq < 64, "the allowance never ran out");
    }
}

/// Astra's rim-feeding regression: a rim footprint has weights `0.4, 0.2, 0.2, 0.2` and
/// sums `3 · w_c` to `3.0000000000000004`, so a bare `allowance_used + 3 > 30` boundary
/// would buy ten interior feeds but only nine rim ones. The allowance is a rule, not a
/// rounding artefact: both must be ten, and the eleventh must be refused with no cleanup.
#[test]
fn the_allowance_buys_the_same_number_of_feeds_at_the_rim_as_inside() {
    for (name, cell, cells) in [
        ("interior", CellId::new(Face::Front, 8, 8), 5),
        ("seam", CellId::new(Face::Front, 15, 8), 5),
        ("top corner", CellId::new(Face::Top, 0, 0), 5),
        // The 64 open-rim cells have degree three: weights 0.4, 0.2, 0.2, 0.2.
        ("rim", CellId::new(Face::Front, 8, 15), 4),
        ("rim corner", CellId::new(Face::Front, 0, 15), 4),
    ] {
        let mut world = World::new(WorldConfig::default()).expect("valid");
        let graph = FieldGraph::new();
        let fp = footprint(&graph, cell, 1);
        assert_eq!(fp.len(), cells, "{name} footprint");
        if cells == 4 {
            let sum: f64 = fp.iter().map(|(_, w)| FEED_MATERIAL * w).sum();
            assert!(sum > FEED_MATERIAL, "{name} should be the case that sums above the dose: {sum}");
        }
        let (seq, accepted, reason) = feed_until_exhausted(&mut world, cell, 0);
        assert_eq!(accepted, 10, "{name}: 30 m of allowance is ten 3 m feeds");
        assert_eq!(reason, "allowance exhausted");
        assert_eq!(world.care().admitted_seq, seq, "{name}: a refusal still spends its seq");
        // The eleventh is still refused with no cleanup in between.
        let again = world.apply_care(&command(seq + 1, 0, CareKind::Feed, cell)).outcome;
        assert_eq!(again, CareOutcome::Rejected("allowance exhausted".into()), "{name}");
        assert!(
            (world.care().feed_material_in - 10.0 * FEED_MATERIAL).abs() < 1e-9,
            "{name} booked {}",
            world.care().feed_material_in
        );
        world.check_invariants().expect("ten feeds leave a consistent world");
    }
}

#[test]
fn the_feed_allowance_runs_out_and_a_clean_gives_it_back() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    let cell = CellId::new(Face::Front, 8, 8);
    let (mut seq, accepted, rejection) = feed_until_exhausted(&mut world, cell, 0);
    assert_eq!(rejection, "allowance exhausted");
    assert_eq!(accepted, 10, "30 m of allowance is ten 3 m feeds");
    // A refused command still consumed its sequence number.
    assert_eq!(world.care().admitted_seq, seq);
    assert!(world.care().allowance_used <= FEED_ALLOWANCE + 1e-9);
    assert!(world.care().allowance_used > FEED_ALLOWANCE - FEED_MATERIAL);

    // One clean is not enough to buy another 3 m feed; two are.
    seq += 1;
    let first = world.apply_care(&command(seq, 0, CareKind::Clean, cell)).outcome;
    assert!(matches!(first, CareOutcome::Applied(_)), "{first:?}");
    seq += 1;
    assert!(matches!(
        world.apply_care(&command(seq, 0, CareKind::Feed, cell)).outcome,
        CareOutcome::Rejected(_)
    ));
    seq += 1;
    assert!(matches!(
        world.apply_care(&command(seq, 0, CareKind::Clean, cell)).outcome,
        CareOutcome::Applied(_)
    ));
    seq += 1;
    assert!(
        matches!(world.apply_care(&command(seq, 0, CareKind::Feed, cell)).outcome, CareOutcome::Applied(_)),
        "two cleans should have freed enough allowance for one feed"
    );
    world.check_invariants().expect("ten feeds and two cleans leave a consistent world");
}

#[test]
fn cleaning_reports_nothing_to_remove_and_partial_removal_honestly() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    let cell = CellId::new(Face::Front, 4, 4);
    world.state.fields.d.fill(0.0);
    world.state.fields.de.fill(0.0);

    let receipt = world.apply_care(&command(1, 0, CareKind::Clean, cell));
    assert_eq!(receipt.outcome, CareOutcome::Rejected("nothing to remove".into()));
    assert_eq!(world.care().admitted_seq, 1, "a refusal still consumes its seq");
    assert_eq!(world.care().clean_material_out, 0.0);

    // Scarce litter: the 0.5·d cap bites long before the 2 m dose does.
    world.state.fields.d[cell.index()] = 0.01;
    world.state.fields.de[cell.index()] = 0.02;
    let receipt = world.apply_care(&command(2, 0, CareKind::Clean, cell));
    let CareOutcome::Partial(q) = receipt.outcome.clone() else {
        panic!("scarce litter was not partial: {:?}", receipt.outcome)
    };
    assert_eq!(world.care().admitted_seq, 2);
    assert!((q.material_out - 0.005).abs() < 1e-15, "{}", q.material_out);
    assert!((q.energy_out - 0.01).abs() < 1e-15, "{}", q.energy_out);
    assert!(q.material_out < CLEAN_MATERIAL);
    assert!((world.state.fields.d[cell.index()] - 0.005).abs() < 1e-15);
    assert!((world.state.fields.de[cell.index()] - 0.01).abs() < 1e-15);
    world.check_invariants().expect("a partial clean preserves the energy density");
}

// ---------------------------------------------------------------- rain

#[test]
fn the_rain_envelope_and_footprint_deliver_exactly_the_dose() {
    let e = rain_envelope();
    let total: f64 = e.iter().sum::<f64>() * cubarium_core::DT;
    assert!((total - 1.0).abs() < 1e-12, "the envelope integrates to {total} s");

    let graph = FieldGraph::new();
    // An ordinary interior cell, a cell on a face seam, and a cell on the open bottom rim.
    for (name, cell, cells) in [
        ("interior", CellId::new(Face::Front, 8, 8), 13),
        ("seam", CellId::new(Face::Front, 15, 8), 13),
        ("rim", CellId::new(Face::Front, 8, 15), 9),
    ] {
        let fp = footprint(&graph, cell, 2);
        assert_eq!(fp.len(), cells, "{name} footprint is {} cells", fp.len());

        let mut world = World::new(still_water_config()).expect("valid");
        world.state.fields.w.fill(0.0);
        let opening: f64 = world.state.fields.w.iter().sum();
        assert_eq!(opening, 0.0);
        let rain_before = world.state.rain_in_total;

        let receipt = world.apply_care(&command(1, 0, CareKind::Rain, cell));
        let CareOutcome::Applied(q) = receipt.outcome.clone() else {
            panic!("{name} rain was not applied: {:?}", receipt.outcome)
        };
        assert_eq!(q.cells, cells as u32);
        assert_eq!(q.water_depth, RAIN_DEPTH_TOTAL, "the receipt quotes the scheduled total");
        assert_eq!(q.ends_tick, Some(u64::from(RAIN_TICKS)));
        assert_eq!(world.care().showers.len(), 1);

        // A second shower while one is running is refused, and still spends its seq.
        let second = world.apply_care(&command(2, 0, CareKind::Rain, cell));
        assert_eq!(second.outcome, CareOutcome::Rejected("shower active".into()));
        assert_eq!(world.care().admitted_seq, 2);

        for _ in 0..RAIN_TICKS {
            world.step();
        }
        assert!(world.care().showers.is_empty(), "{name}: the shower did not retire");
        assert_eq!(world.tick(), u64::from(RAIN_TICKS));

        // With no natural rain, no flow and no evaporation, every drop is the manual dose.
        let delivered: f64 = world.state.fields.w.iter().sum();
        assert!((delivered - RAIN_DEPTH_TOTAL).abs() < 1e-9, "{name} delivered {delivered} d");
        assert!(
            (world.care().rain_depth_in - RAIN_DEPTH_TOTAL).abs() < 1e-9,
            "{name} booked {} d",
            world.care().rain_depth_in
        );
        assert_eq!(
            world.state.rain_in_total - rain_before,
            world.care().rain_depth_in,
            "{name}: manual water is inside rain_in_total exactly once"
        );
        // Per cell it is the footprint weight's share, and nothing fell outside.
        let mut inside = vec![false; cubarium_surface::CELL_COUNT];
        for (c, w) in &fp {
            inside[c.index()] = true;
            let got = world.state.fields.w[c.index()];
            assert!((got - RAIN_DEPTH_TOTAL * w).abs() < 1e-9, "{name} cell {}: {got}", c.index());
        }
        assert!(
            world.state.fields.w.iter().enumerate().all(|(i, &x)| inside[i] || x == 0.0),
            "{name}: water fell outside the footprint"
        );
        world.check_invariants().expect("a shower leaves a consistent world");
    }
}

#[test]
fn the_published_rain_rate_is_natural_plus_manual() {
    // The default config does rain naturally, so the published rate must be the sum.
    let mut with_care = World::new(WorldConfig::default()).expect("valid");
    let mut without = World::new(WorldConfig::default()).expect("valid");
    let cell = CellId::new(Face::Front, 8, 8);
    let graph = FieldGraph::new();
    let fp = footprint(&graph, cell, 2);
    let envelope = rain_envelope();

    assert!(matches!(
        with_care.apply_care(&command(1, 0, CareKind::Rain, cell)).outcome,
        CareOutcome::Applied(_)
    ));
    with_care.step();
    without.step();

    let care_rain = with_care.render_view().rain;
    let bare_rain = without.render_view().rain;
    assert!(bare_rain.iter().any(|&r| r > 0.0), "this test needs natural rain to be falling");
    let mut inside = vec![0.0f64; cubarium_surface::CELL_COUNT];
    for (c, w) in &fp {
        inside[c.index()] = RAIN_DEPTH_TOTAL * w * envelope[0];
    }
    for i in 0..cubarium_surface::CELL_COUNT {
        let expected = (f64::from(bare_rain[i]) + inside[i]) as f32;
        let got = care_rain[i];
        assert!(
            (f64::from(got) - f64::from(expected)).abs() <= 1e-6 * f64::from(expected).abs().max(1.0),
            "cell {i}: published {got}, natural {} + manual {}",
            bare_rain[i],
            inside[i]
        );
    }
    // Natural rain is untouched outside the footprint.
    for (i, (&a, &b)) in care_rain.iter().zip(bare_rain.iter()).enumerate() {
        if inside[i] == 0.0 {
            assert_eq!(a, b, "cell {i} outside the footprint changed");
        }
    }
    // The weather source itself was never mutated.
    assert_eq!(with_care.state.weather, without.state.weather);
}

// ---------------------------------------------------------------- ordering

#[test]
fn a_wrong_seq_or_a_wrong_boundary_changes_nothing_and_void_only_moves_the_cursor() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    for _ in 0..10 {
        world.step();
    }
    let before = state_hash(&world.state);
    let cell = CellId::new(Face::Front, 8, 8);
    let tick = world.tick();

    for seq in [0u64, 2, 7, u64::MAX] {
        let receipt = world.apply_care(&command(seq, tick, CareKind::Feed, cell));
        assert_eq!(receipt.outcome, CareOutcome::Rejected("out of order".into()), "seq {seq}");
        assert_eq!(state_hash(&world.state), before, "seq {seq} changed the state");
        assert_eq!(world.care().admitted_seq, 0);
    }

    for boundary in [tick - 1, tick + 1, 0] {
        let mut cmd = command(1, boundary, CareKind::Feed, cell);
        cmd.apply_after_tick = boundary;
        let receipt = world.apply_care(&cmd);
        assert_eq!(receipt.outcome, CareOutcome::Rejected("wrong boundary".into()), "at {boundary}");
        assert_eq!(receipt.tick, tick, "the receipt reports where the world actually is");
        assert_eq!(state_hash(&world.state), before, "boundary {boundary} changed the state");
        assert_eq!(world.care().admitted_seq, 0);
    }

    // `void_care` spends the cursor and nothing else: the ecology is bit-identical.
    let ecology = ecology_hash(&world.state);
    assert!(world.void_care(1));
    assert_eq!(world.care().admitted_seq, 1);
    assert_eq!(ecology_hash(&world.state), ecology, "void_care touched the ecology");
    assert_ne!(state_hash(&world.state), before, "void_care must be in the full-state hash");
    assert!(!world.void_care(3), "void_care is contiguous too");
    assert!(!world.void_care(1));
    assert_eq!(world.care().admitted_seq, 1);

    // And the next real command picks up from there.
    let receipt = world.apply_care(&command(2, tick, CareKind::Feed, cell));
    assert!(matches!(receipt.outcome, CareOutcome::Applied(_)), "{:?}", receipt.outcome);

    // An unresolvable target is a rejection, not a panic, and still spends its seq.
    let bad = CareCommand {
        seq: 3,
        apply_after_tick: tick,
        kind: CareKind::Feed,
        target: CareTarget { face: 9, u: FACE_EXTENT * 2.0, v: f64::NAN },
    };
    assert_eq!(world.apply_care(&bad).outcome, CareOutcome::Rejected("invalid target".into()));
    assert_eq!(world.care().admitted_seq, 3);
}

// ---------------------------------------------------------------- resume

#[test]
fn a_shower_resumes_from_a_snapshot_at_the_next_undelivered_sample() {
    let mut original = World::new(still_water_config()).expect("valid");
    let cell = CellId::new(Face::Front, 6, 9);
    assert!(matches!(
        original.apply_care(&command(1, 0, CareKind::Rain, cell)).outcome,
        CareOutcome::Applied(_)
    ));
    for _ in 0..50 {
        original.step();
    }
    let shower: &ActiveShower = &original.care().showers[0];
    assert_eq!(shower.delivered, 50, "50 of 120 samples are gone");
    assert_eq!(shower.apply_after_tick, 0);
    assert_eq!(shower.seq, 1);
    assert_eq!(shower.cells.len(), 13);

    let bytes = encode_snapshot(&original.state, "care-resume-test");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), SCHEMA_VERSION);
    let (meta, state) = decode_snapshot(&bytes).expect("round trip");
    assert_eq!(meta.schema, SCHEMA_VERSION);
    let mut reloaded = World::from_state(state).expect("the mid-shower state is valid");
    assert_eq!(state_hash(&reloaded.state), state_hash(&original.state));

    // Both worlds run to the end of the shower and past it, in lockstep.
    for _ in 0..(RAIN_TICKS + 30) {
        original.step();
        reloaded.step();
        assert_eq!(
            state_hash(&reloaded.state),
            state_hash(&original.state),
            "diverged at tick {}",
            original.tick()
        );
    }
    assert!(original.care().showers.is_empty());
    let delivered: f64 = original.state.fields.w.iter().sum();
    assert!((delivered - RAIN_DEPTH_TOTAL).abs() < 1e-9, "the resumed shower delivered {delivered}");
    assert!((original.care().rain_depth_in - RAIN_DEPTH_TOTAL).abs() < 1e-9);
}

// ---------------------------------------------------------------- decode hardening

/// A snapshot is untrusted input. A crafted shower must not be able to restart its envelope,
/// double a cell's share, claim a sequence the world never admitted, or carry weights that
/// deliver more than the dose. Each case changes exactly one property of a state that is
/// otherwise valid, so a failure names the check that stopped working.
#[test]
fn a_crafted_shower_is_refused_by_the_decoder_one_property_at_a_time() {
    let sound = || ActiveShower {
        seq: 1,
        apply_after_tick: 0,
        cells: vec![0],
        weights: vec![1.0],
        delivered: 0,
    };
    let base = || {
        let mut world = World::new(still_water_config()).expect("valid");
        world.state.care.admitted_seq = 1;
        world.state.care.showers.push(sound());
        world.state
    };

    // The control: the state these are built from really does load.
    let ok = base();
    ok.validate().expect("the control state is valid");
    let bytes = encode_snapshot(&ok, "hardening");
    decode_snapshot(&bytes).expect("the control snapshot loads");

    // 1. Progress that does not match the elapsed ticks would replay rain already delivered.
    let mut restart = base();
    restart.tick = 60;
    for (_, o) in restart.organisms.iter_mut() {
        o.born_tick = 0;
    }
    // `delivered` is still 0 sixty ticks after the shower began.
    assert_shower_refused(restart, "delivered");

    // 2. A repeated footprint cell would take its share twice.
    let mut duplicate = base();
    duplicate.care.showers[0].cells = vec![0, 0];
    duplicate.care.showers[0].weights = vec![0.5, 0.5];
    assert_shower_refused(duplicate, "twice");

    // 3. A shower the sequence cursor never admitted.
    let mut unadmitted = base();
    unadmitted.care.admitted_seq = 0;
    unadmitted.care.showers[0].seq = 0;
    assert_shower_refused(unadmitted, "never admitted");

    // 4. Weights that sum past the contract's 1e-12 tolerance deliver more than the dose.
    let mut heavy = base();
    heavy.care.showers[0].weights = vec![1.0 - 5e-10];
    assert_shower_refused(heavy, "sum to");

    // 5. Two showers at once.
    let mut two = base();
    two.care.admitted_seq = 2;
    let mut other = sound();
    other.seq = 2;
    other.cells = vec![1];
    two.care.showers.push(other);
    assert_shower_refused(two, "at most one");

    // 6. An allowance past the bound.
    let mut greedy = base();
    greedy.care.allowance_used = FEED_ALLOWANCE + 1.0;
    assert_shower_refused(greedy, "exceeds");

    // 7. A non-finite ledger.
    let mut nan = base();
    nan.care.feed_energy_in = f64::NAN;
    assert_shower_refused(nan, "not finite");
}

fn assert_shower_refused(state: WorldState, needle: &str) {
    let message = state.validate().expect_err(&format!("a state that should fail on {needle:?} passed"));
    assert!(message.contains(needle), "{message:?} does not mention {needle:?}");
    // The same refusal through the real door: encode it and try to load it back.
    let bytes = encode_snapshot(&state, "hardening");
    match decode_snapshot(&bytes) {
        Err(cubarium_core::SnapshotError::Invalid(m)) => {
            assert!(m.contains(needle), "{m:?} does not mention {needle:?}")
        }
        other => panic!("a crafted snapshot decoded: {other:?}"),
    }
    // And `World::from_state` refuses it too, so a live world never reaches the tick loop.
    assert!(World::from_state(state).is_err(), "from_state accepted a state failing on {needle:?}");
}

/// The complement of the hardening test: a real mid-shower snapshot is bit-identical through
/// a round trip and finishes its remaining samples exactly.
#[test]
fn a_normal_mid_shower_snapshot_round_trips_unchanged() {
    let mut world = World::new(still_water_config()).expect("valid");
    assert!(matches!(
        world.apply_care(&command(1, 0, CareKind::Rain, CellId::new(Face::Back, 9, 4))).outcome,
        CareOutcome::Applied(_)
    ));
    for _ in 0..77 {
        world.step();
    }
    let bytes = encode_snapshot(&world.state, "round-trip");
    let (_, back) = decode_snapshot(&bytes).expect("a real mid-shower snapshot loads");
    assert_eq!(back, world.state, "the round trip is not bit-identical");
    assert_eq!(encode_snapshot(&back, "round-trip"), bytes);
    assert_eq!(back.care.showers[0].delivered, 77);
    assert_eq!(back.care.showers[0].apply_after_tick, 0);
}

// ---------------------------------------------------------------- targets

#[test]
fn a_target_names_the_cell_that_contains_it() {
    for cell in [
        CellId::new(Face::Front, 0, 0),
        CellId::new(Face::Top, 15, 15),
        CellId::new(Face::Back, 7, 9),
        CellId::new(Face::Left, 15, 0),
    ] {
        let t = target_of(cell);
        assert_eq!(t.resolve(), Some(cell));
        assert_eq!(cell_of(&cell.center()), cell);
    }
}
