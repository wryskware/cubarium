// Independent narrow contract regressions for 2fbd092. These are validator
// fixtures, not a simulated cohort or purported genuine snapshot inspection.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {crc32, verifyArm, verifyClosingSnapshot} from './reduce-quiet-compare.mjs';

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
});
