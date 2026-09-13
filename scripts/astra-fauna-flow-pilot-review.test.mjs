// Independent regressions against reducer 9259c2b: four accepted malformed
// copies of the real seed-1 pilot. Originals are read-only and never written.
// Run from any directory with: node scripts/astra-fauna-flow-pilot-review.test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {reduceArtifact} from './fauna-development-flow.mjs';

const original = JSON.parse(await readFile(new URL(
  '../captures/fauna-development-flow-pilots-2026-09-13/seed-1.json', import.meta.url),
  'utf8'));

test('per-form adult totals must agree with the actual member records', () => {
  const a = structuredClone(original);
  assert.equal(a.forms[3].reached_adult_target, 27, 'pin the real pilot fixture');
  a.forms[3].reached_adult_target = 38;
  assert.throws(() => reduceArtifact(a));
});

test('a full horizon cannot certify a one-sample empty census series', () => {
  const a = structuredClone(original);
  a.gates.census_cross_check.samples_compared = 1;
  a.gates.census_cross_check.reconstructed_series = [];
  assert.throws(() => reduceArtifact(a));
});

test('a placed child cannot carry a negative parental energy debit', () => {
  const a = structuredClone(original);
  const child = a.members.find(m => m.origin === 'descendant');
  assert.ok(child.birth_payment.parent_energy_debit > 0);
  child.birth_payment.parent_energy_debit = -1;
  assert.throws(() => reduceArtifact(a));
});

test('every member must carry all three residual stock components', () => {
  const a = structuredClone(original);
  delete a.members[0].reconciliation.worst_residual.energy;
  assert.throws(() => reduceArtifact(a));
});
