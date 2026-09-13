import test from 'node:test';
import assert from 'node:assert/strict';
import {compareSnapshot, projectCensusRow, stocksOf, divergence, structureObservations,
  SELECTOR_PROJECTED_FIELDS, SNAPSHOT_FILES, SNAPSHOT_SCHEMA, ARMS}
  from './hunter-size-gate-parity.mjs';

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

  // A different world is a parity failure, on each payload field independently.
  for (const key of ['schema', 'crc32', 'state_hash', 'payload_bytes']) {
    const diff = compareSnapshot(snapshot(), snapshot({[key]: 'moved'}));
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
