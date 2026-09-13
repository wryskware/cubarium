// Independent narrow contract regressions for 2fbd092. These are validator
// fixtures, not a simulated cohort or purported genuine snapshot inspection.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {crc32, verifyArm, verifyClosingSnapshot, crossCheck, reduceBouts,
  reduceQuietEvents, reduceLife, readOpeningIdentities} from './reduce-quiet-compare.mjs';

test('a completed B+40 snapshot may retain its pause until the following decision', () => {
  // A synthetic envelope and inspector response isolate the range validation;
  // genuine core B+40 decode behavior is independently covered in astra_quiet_policy.rs.
  const bytes = Buffer.alloc(24);
  bytes.write('CUBW'); bytes.writeUInt32LE(13, 4); bytes.writeUInt16LE(1, 8);
  bytes.write('x', 10); bytes.writeBigUInt64LE(1n, 11); bytes[23] = 0;
  const checksum = crc32(bytes.subarray(23)); bytes.writeUInt32LE(checksum, 19);
  const hash = createHash('sha256').update(bytes).digest('hex');
  const care = {};
  const inspection = {header: {schema: 13, current_schema: 13, payload_len: 1, crc32: checksum},
    sha256: hash, tick: 144040, state_hash: '123', ecology_hash: '456', population: 2,
    quiet: {policy: 'post_birth_pause_v1', open_pauses: 1, pauses: [{
      parent: {slot: 0, generation: 1}, child: {slot: 1, generation: 1},
      start_tick: 144000, end_tick: 144040}]}, care,
    hunters_present: false, inventories: {material: 1, energy: 1, water: 1}};
  const summary = {closing_snapshot_sha256: hash, closing_tick: inspection.tick,
    closing_state_hash: '123', closing_ecology_hash: '456', closing_population: 2,
    quiet_policy: 'post_birth_pause_v1', open_pauses_at_close: 1, care: {ledgers: care}};
  assert.doesNotThrow(() => verifyClosingSnapshot(bytes, inspection, summary,
    {build: 'x'}, 'candidate_nocare'));
  inspection.tick++;
  summary.closing_tick++;
  assert.throws(() => verifyClosingSnapshot(bytes, inspection, summary,
    {build: 'x'}, 'candidate_nocare'), /outside/);
});

test('legacy raw energy remains diagnostic when all fixed corrected energy gates pass', () => {
  const summary = JSON.parse(readFileSync(
    'captures/quiet-smoke-validated-2026-09-13c/seed-1/off_nocare/summary.json', 'utf8'));
  // Inject only the known representation-level condition; this is not evidence
  // that the smoke or any unrun biological cohort actually exhibited this drift.
  summary.max_absolute_drift.energy = 2 * summary.pre_intervention_baseline.limits.energy;
  summary.gates.legacy_raw_energy = false;
  assert.doesNotThrow(() => verifyArm(summary, summary.planned_ticks,
    summary.closing_tick - summary.elapsed_ticks, summary.arm));
  for (const field of ['corrected_energy_drift', 'independent_windowed_energy_drift',
    'care_boundary_energy_drift']) {
    const bad = structuredClone(summary);
    bad[field] = summary.pre_intervention_baseline.limits.energy;
    assert.throws(() => verifyArm(bad, bad.planned_ticks,
      bad.closing_tick - bad.elapsed_ticks, bad.arm));
  }
});

test('the exact retained wrong-child probe fails the full-ID crosswalk', () => {
  const dir = 'captures/quiet-smoke-validated-2026-09-13c/seed-1/candidate_nocare';
  const text = name => readFileSync(`${dir}/${name}`, 'utf8');
  const rows = name => text(name).trim().split('\n').filter(Boolean).map(JSON.parse);
  const summary = JSON.parse(text('summary.json'));
  const opening = summary.closing_tick - summary.elapsed_ticks, closing = summary.closing_tick;
  const identityText = text('opening-organisms.jsonl');
  const identities = readOpeningIdentities(identityText,
    identityText.trim().split('\n').length, 'candidate_nocare', opening);
  const life = reduceLife(rows('life.jsonl'), summary, opening, closing, identities);
  const bouts = reduceBouts(rows('bouts.jsonl'), summary, opening, closing);
  const quiet = rows('quiet-events.jsonl');
  assert.doesNotThrow(() => crossCheck(bouts,
    reduceQuietEvents(quiet, summary, opening, closing), life, 'candidate_nocare'));
  const begin = quiet.find(e => e.kind === 'begin');
  assert.deepEqual(begin.parent, {generation: 5, slot: 47});
  assert.deepEqual(begin.child, {generation: 5, slot: 11});
  assert.equal(begin.tick, 144228);
  const parent = JSON.stringify(begin.parent), child = JSON.stringify(begin.child);
  for (const e of quiet)
    if (JSON.stringify(e.parent) === parent && JSON.stringify(e.child) === child)
      e.child.generation += 100000;
  const events = reduceQuietEvents(quiet, summary, opening, closing);
  assert.throws(() => crossCheck(bouts, events, life, 'candidate_nocare'), /11\/100005/);
  // The reciprocal bout link must also retain the exact child even when the
  // independent life and quiet streams themselves were left intact.
  const badBouts = rows('bouts.jsonl');
  badBouts.find(b => b.origin_boundary === 144228).origin_child.generation += 100000;
  assert.throws(() => crossCheck(reduceBouts(badBouts, summary, opening, closing),
    reduceQuietEvents(rows('quiet-events.jsonl'), summary, opening, closing),
    life, 'candidate_nocare'), /11\/100005/);
});

test('a positive held-death close cannot silently lose its only bout', () => {
  const events = {offers: new Map([['1/1@10', new Set(['2/1'])]]),
    admitted: new Map([['1/1@10', '2/1']]),
    closes: new Map([['1/1@10', {child: '2/1', completed: 1, kind: 'abort'}]])};
  const life = {births: 1, birthsByParent: new Map([['1/1@10', new Set(['2/1'])]])};
  const bouts = {recoveryOrigins: new Map(), byClass: {newborn_initial: {bouts: 1}}};
  assert.throws(() => crossCheck(bouts, events, life, 'fixture'), /no recovery bout/);
  bouts.recoveryOrigins.set('1/1@10', {child: '2/1', ticks: 1, end: 'aborted'});
  assert.doesNotThrow(() => crossCheck(bouts, events, life, 'fixture'));
});
