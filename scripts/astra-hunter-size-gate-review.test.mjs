// Independent error-path regressions; intentionally red against main 02b5823.
// These do not allege that the retained pilot artifacts are malformed.
import test from 'node:test';
import assert from 'node:assert/strict';
import {compareSnapshot, divergence} from './hunter-size-gate-parity.mjs';

test('two decoder failures cannot certify identical snapshot payloads', () => {
  assert.equal(compareSnapshot({error: 'CRC failed'}, {error: 'CRC failed'})
    .identical_payload, false);
});

test('a matching census prefix is not full stream identity', () => {
  for (const [a, b] of [
    [[{tick: 0}, {tick: 200}], [{tick: 0}]],
    [[{tick: 0}], [{tick: 0}, {tick: 200}]],
  ]) {
    assert.equal(divergence(a, b).identical_under_projection, false);
  }
});
