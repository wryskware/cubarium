import test from 'node:test';
import assert from 'node:assert/strict';
import {compareSnapshot, projectCensusRow, stocksOf, divergence, structureObservations,
  checkFlow, firstGrowthFromFlow, SELECTOR_PROJECTED_FIELDS, SNAPSHOT_FILES, SNAPSHOT_SCHEMA,
  ARMS, RESIDUAL_TOLERANCE, INTAKE_SOURCES} from './hunter-size-gate-parity.mjs';

const snapshot = over => ({schema: SNAPSHOT_SCHEMA, crc32: 123456, state_hash: '999',
  payload_bytes: 4096, sha256: 'aaa', build: '0.1.0+512ee52', ...over});

test('the contract names six arms, two snapshots and schema 12', () => {
  assert.equal(ARMS.length, 6);
  assert.deepEqual(SNAPSHOT_FILES, ['post-initialization.cubw', 'closing.cubw']);
  assert.equal(SNAPSHOT_SCHEMA, 12);
});

test('parity is the payload; the build id is recorded, never asserted equal', () => {
  // Same world, different build: the header SHA differs by construction and that is not a
  // parity failure.
  const same = compareSnapshot(snapshot(), snapshot({sha256: 'bbb', build: '0.1.0+9f4cf7d'}));
  assert.equal(same.identical_payload, true, JSON.stringify(same.differences));
  assert.equal(same.header_differs_by_build_id, true);
  assert.equal(same.retained_sha256, 'aaa');
  assert.equal(same.rerun_sha256, 'bbb');

  // A different world is a parity failure, on each payload field independently. The values
  // stay type-appropriate so this exercises the field comparison, not the readability guard.
  for (const [key, moved] of [['schema', 11], ['crc32', 999999], ['state_hash', '111'],
    ['payload_bytes', 8192]]) {
    const diff = compareSnapshot(snapshot(), snapshot({[key]: moved}));
    assert.equal(diff.identical_payload, false, key);
    assert.deepEqual(diff.differences.map(d => d.field), [key]);
  }
});

const row = over => ({tick: 200, adults: 0, prey: 90,
  hunter_state: {profile: {version: 4, seek_reserve_fraction: 0.8}, founders_placed: 1},
  hunter_stocks: [{id: {slot: 11, generation: 5}, structure: 0.8, reserve: 0.5, energy: 1.0,
    adult_structure: 2.0}],
  ...over});

test('the projection removes exactly the named selector fields and nothing else', () => {
  assert.deepEqual(SELECTOR_PROJECTED_FIELDS, ['hunter_state.profile.version']);
  const projected = projectCensusRow(row());
  assert.equal(projected.hunter_state.profile.version, undefined);
  // Everything else survives, including the rest of the profile.
  assert.equal(projected.hunter_state.profile.seek_reserve_fraction, 0.8);
  assert.equal(projected.hunter_state.founders_placed, 1);
  assert.equal(projected.prey, 90);
  assert.equal(projected.hunter_stocks[0].structure, 0.8);
  // The input is not mutated.
  assert.equal(row().hunter_state.profile.version, 4);
});

test('two runs that differ only in the selector are identical under the projection', () => {
  const reference = [row(), row({tick: 400})];
  const candidate = [
    row({hunter_state: {profile: {version: 5, seek_reserve_fraction: 0.8}, founders_placed: 1}}),
    row({tick: 400,
      hunter_state: {profile: {version: 5, seek_reserve_fraction: 0.8}, founders_placed: 1}}),
  ];
  const d = divergence(reference, candidate);
  assert.equal(d.identical_under_projection, true, JSON.stringify(d.first_projected_row_difference));
  assert.equal(d.first_member_structure_difference, null);
  assert.equal(d.census_rows_compared, 2);
});

test('the first altered growth transaction is reported by tick and member', () => {
  const grown = row({tick: 400,
    hunter_stocks: [{id: {slot: 11, generation: 5}, structure: 0.8004, reserve: 0.49,
      energy: 1.0, adult_structure: 2.0}]});
  const d = divergence([row(), row({tick: 400})], [row(), grown]);
  assert.equal(d.first_member_structure_difference.tick, 400);
  assert.equal(d.first_member_structure_difference.member, '11:5');
  assert.equal(d.first_member_structure_difference.reference.structure, 0.8);
  assert.equal(d.first_member_structure_difference.candidate.structure, 0.8004);
  assert.equal(d.identical_under_projection, false, 'the row itself also differs');
  assert.equal(d.first_projected_row_difference.tick, 400);
});

test('a member present in one run and not the other is a difference, not a skip', () => {
  const empty = row({tick: 400, hunter_stocks: []});
  const d = divergence([row(), row({tick: 400})], [row(), empty]);
  assert.equal(d.first_member_structure_difference.tick, 400);
  assert.equal(d.first_member_structure_difference.candidate, null);
});

test('misaligned census ticks stop the walk rather than comparing unrelated rows', () => {
  const d = divergence([row()], [row({tick: 999})]);
  assert.equal(d.first_projected_row_difference.why.includes('misaligned'), true);
});

test('member stocks are keyed by full slot and generation', () => {
  assert.deepEqual(Object.keys(stocksOf(row())), ['11:5']);
  assert.deepEqual(stocksOf({hunter_stocks: []}), {});
  assert.deepEqual(stocksOf({}), {});
});

test('two unreadable snapshots are missing evidence, not matching evidence', () => {
  // Independently reported: both sides absent means every payload field "agrees" by being
  // undefined, which would certify an unreadable pair as identical.
  assert.equal(compareSnapshot({error: 'CRC failed'}, {error: 'CRC failed'}).identical_payload,
    false);
  assert.equal(compareSnapshot({unreadable: '/x.cubw'}, snapshot()).identical_payload, false);
  assert.equal(compareSnapshot(snapshot(), {}).identical_payload, false);
  const reasons = compareSnapshot({error: 'a'}, {error: 'b'}).differences.map(d => d.field);
  assert.deepEqual(reasons, ['readable', 'readable']);
});

test('a matching census prefix is not full stream identity', () => {
  // Independently reported: walking min(length) alone certifies a truncated stream.
  for (const [a, b] of [
    [[{tick: 0}, {tick: 200}], [{tick: 0}]],
    [[{tick: 0}], [{tick: 0}, {tick: 200}]],
  ]) {
    const d = divergence(a, b);
    assert.equal(d.identical_under_projection, false);
    assert.equal(d.same_length, false);
    assert(d.first_projected_row_difference.why.includes('length'));
  }
  assert.equal(divergence([{tick: 0}], [{tick: 0}]).same_length, true);
});

const flowMember = over => ({
  id: {slot: 17, generation: 3}, origin: 'Descendant', born_tick: 170401,
  end_tick: 174863, end_cause: 'Starvation',
  open_stocks: {structure: 0.8, reserve: 0.8, energy: 0.6},
  close_stocks: {structure: 0.8485, reserve: 0, energy: 0},
  death_stocks: {structure: 0.8485, reserve: 0, energy: 0},
  digestion: {to_reserve: 0}, frugivory: {to_reserve: 0}, grazing: {to_reserve: 0},
  scavenging: {to_reserve: 0},
  oxidation: {reserve_burned: 0.7}, upkeep: {paid: 1.2}, strike: {paid: 0.16},
  growth: {ticks: 485, structure_gained: 0.0485, reserve_spent: 0.0485, energy_cost: 0.02425,
    heat: 0.12125, bound_by_rate: 485, bound_by_remaining_structure: 0, bound_by_reserve: 0,
    bound_by_energy: 0},
  gate: {gate_reserve_last: 0.5091, gate_reserve_min: 0.48, gate_reserve_max: 0.5091,
    legacy_gate_reserve_last: 1.2, gate_reserve_at_first_growth: 0.48,
    structure_at_first_growth: 0.8, first_growth_tick: 170401, first_adult_tick: null,
    max_structure: 0.8485, observed_ticks: 4462, branch_entered_ticks: 485},
  residual: {violations: 0, max_structure: 1e-17, max_reserve: 2e-17, max_energy: 3e-17},
  bins: [1, 2, 3, 4],
  ...over});

const flowRecord = over => ({
  kind: 'per-member-mutation-site-flow', complete_horizon: true, closing_tick: 288000,
  planned_ticks: 144000, elapsed_ticks: 144000, incomplete_reason: null,
  expected_members: 1, recorded_members: 1, residual_tolerance: RESIDUAL_TOLERANCE,
  observer_neutrality_probe: {ticks: 2000, state_equal: true, event_records_equal: true},
  ledger: {members: [flowMember()]},
  ...over});

test('a sound flow record passes and reports the actual first growth', () => {
  assert.deepEqual(checkFlow(flowRecord(), {closing_tick: 288000}), []);
  const first = firstGrowthFromFlow(flowRecord());
  assert.equal(first.tick, 170401);
  assert.equal(first.member, '17:3');
  assert.equal(first.gate_at_first_growth, 0.48);
  assert.equal(first.structure_at_first_growth, 0.8);
  // A member that never grew contributes no first growth rather than a fabricated one.
  assert.equal(firstGrowthFromFlow(flowRecord({ledger: {members: [flowMember({
    growth: {...flowMember().growth, ticks: 0, structure_gained: 0, reserve_spent: 0,
      bound_by_rate: 0},
    gate: {...flowMember().gate, first_growth_tick: null, gate_reserve_at_first_growth: null,
      structure_at_first_growth: null}})]}})), null);
});

test('the flow check fails the arm on every gap it exists to catch', () => {
  const cases = [
    [undefined, 'no flow.json'],
    [flowRecord({complete_horizon: false, elapsed_ticks: 5, incomplete_reason: 'stopped'}),
      'incomplete horizon'],
    [flowRecord({recorded_members: 0, ledger: {members: []}}), 'members the arm held'],
    [flowRecord({residual_tolerance: 1}), 'not the frozen'],
    [flowRecord({observer_neutrality_probe: {ticks: 2000, state_equal: false,
      event_records_equal: true}}), 'neutrality'],
    [flowRecord({closing_tick: 1}), 'disagrees with the summary'],
    [flowRecord({ledger: {members: [flowMember({digestion: {to_reserve: NaN}})]}}),
      'digestion.to_reserve is NaN'],
    [flowRecord({ledger: {members: [flowMember({scavenging: {to_reserve: -1}})]}}),
      'scavenging.to_reserve is -1'],
    [flowRecord({ledger: {members: [flowMember({residual: {violations: 2,
      max_structure: 0, max_reserve: 0, max_energy: 0}})]}}), 'reconciliation violations'],
    [flowRecord({ledger: {members: [flowMember({residual: {violations: 0,
      max_structure: 1e-6, max_reserve: 0, max_energy: 0}})]}}), 'residual max_structure'],
    // Structure built without reserve spent for it.
    [flowRecord({ledger: {members: [flowMember({growth: {...flowMember().growth,
      reserve_spent: 0}})]}}), 'against 0 reserve spent'],
    // A cap attribution that does not account for the steps taken.
    [flowRecord({ledger: {members: [flowMember({growth: {...flowMember().growth,
      bound_by_rate: 1}})]}}), 'attributed caps'],
    // Growth recorded with no first-growth boundary.
    [flowRecord({ledger: {members: [flowMember({gate: {...flowMember().gate,
      first_growth_tick: null}})]}}), 'disagrees with first_growth_tick'],
  ];
  for (const [record, expected] of cases) {
    const found = checkFlow(record, {closing_tick: 288000}).join('; ');
    assert(found.includes(expected), `expected "${expected}", got "${found}"`);
  }
});

test('source identity is per channel, and the total is their sum', () => {
  assert.deepEqual(INTAKE_SOURCES, ['digestion', 'frugivory', 'grazing', 'scavenging']);
  // A record that omits a channel entirely cannot pass by having a plausible total.
  const missing = flowRecord({ledger: {members: [{...flowMember(), scavenging: undefined}]}});
  assert(checkFlow(missing, {closing_tick: 288000}).join('; ').includes('scavenging.to_reserve'));
});

test('structure observations record growth, its first boundary and adulthood', () => {
  const flat = structureObservations([row(), row({tick: 400})]);
  assert.equal(flat.length, 1);
  assert.equal(flat[0].structure_ever_increased, false);
  assert.equal(flat[0].first_increase_tick, null);
  assert.equal(flat[0].reached_adult_structure, false);
  assert.deepEqual(flat[0].distinct_structure_values, [0.8]);

  const rising = structureObservations([
    row(),
    row({tick: 400, hunter_stocks: [{id: {slot: 11, generation: 5}, structure: 0.9,
      reserve: 0.4, energy: 1.0, adult_structure: 2.0}]}),
    row({tick: 600, hunter_stocks: [{id: {slot: 11, generation: 5}, structure: 2.0,
      reserve: 0.4, energy: 1.0, adult_structure: 2.0}]}),
  ]);
  assert.equal(rising[0].structure_ever_increased, true);
  assert.equal(rising[0].first_increase_tick, 400);
  assert.equal(rising[0].reached_adult_structure, true);
  assert.equal(rising[0].opening_structure, 0.8);
  assert.equal(rising[0].max_structure, 2.0);
  assert.deepEqual(rising[0].distinct_structure_values, [0.8, 0.9, 2.0]);
  assert.equal(rising[0].samples, 3);
});
