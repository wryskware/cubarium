import test from 'node:test';
import assert from 'node:assert/strict';
import {verifyChargePair, verifyOxidation, verifyChargeManifestOpening, verifyChargeControlPair} from './compare-hunter-recipes.mjs';

const opening = candidate => ({
  arm: 'specialist_on', profile_recipe: candidate ? 'reserve-targets-charge80-v1' : 'reserve-targets-v1',
  config: {capacity: {max_organisms:512}, organism: {oxidation_threshold: 0.5, oxidation_rate: 0.01,
    oxidation_efficiency: 0.8, reserve_energy_density: 2}},
  recipe_oxidation_policy: candidate ? 'fixed-member-threshold' : 'configured-world-threshold',
  recipe_oxidation_threshold: candidate ? 0.8 : 0.5,
  world_member_oxidation_threshold: candidate ? 0.8 : 0.5, world_oxidation_threshold: 0.5,
  profile: {version: candidate ? 4 : 3, seek_reserve_fraction: 0.8, perch_reserve_fraction: 0.9},
});
const summary = () => ({planned_ticks:144000,oxidation: {scope: 'paid extra oxidation', member_threshold: 0.8,
  world_threshold: 0.5, charging_above_reference: {transactions: 1, reserve_burned: 0.0005,
    energy_gained: 0.0008, conversion_heat: 0.0002}}});

test('charging totals obey conversion energy, efficiency and per-transaction rate limits', () => {
  verifyOxidation(summary(), 0.8, opening(true));
  for (const mutate of [
    c => c.energy_gained = 100,
    c => { c.energy_gained = 0.0009; c.conversion_heat = 0.0001; },
    c => { c.reserve_burned = 1; c.energy_gained = 1.6; c.conversion_heat = 0.4; },
  ]) {
    const s = summary(); mutate(s.oxidation.charging_above_reference);
    assert.throws(() => verifyOxidation(s, 0.8, opening(true)));
  }
});

test('reported thresholds cannot jointly contradict the actual shared config', () => {
  const [a,b] = [opening(false), opening(true)];
  verifyChargePair(a,b);
  a.config.organism.oxidation_threshold = 0.6;
  b.config.organism.oxidation_threshold = 0.6;
  assert.throws(() => verifyChargePair(a,b));
});

test('summary reference threshold agrees with its actual opening', () => {
  const s = summary(); s.oxidation.world_threshold = 0.4;
  assert.throws(() => verifyOxidation(s, 0.8, opening(true)));
});

test('manifest policy and reference are reconciled with every opening', () => {
  const o = opening(true);
  const m = {profile_recipe:o.profile_recipe, world_oxidation_threshold:0.5,
    member_oxidation_policy:o.recipe_oxidation_policy, member_oxidation_threshold:0.8};
  verifyChargeManifestOpening(m,o);
  for (const key of ['world_oxidation_threshold', 'member_oxidation_threshold']) {
    const bad = {...m, [key]:0.6};
    assert.throws(() => verifyChargeManifestOpening(bad,o));
  }
});

test('no-member budget control allows only persisted policy hashes and threshold metadata to differ', () => {
  const a = summary();
  a.oxidation.member_threshold = 0.5;
  a.oxidation.charging_above_reference = {transactions:0, reserve_burned:0, energy_gained:0, conversion_heat:0};
  a.closing_hunters = 0; a.reproductive_opportunity = {member_ticks:0};
  a.closing_state_hash = '1'; a.closing_snapshot_sha256 = 'a';
  a.closing_ecology_projection_hash = '7'; a.prey_tick_integral = 100;
  const b = structuredClone(a);
  b.closing_state_hash = '2'; b.closing_snapshot_sha256 = 'b';
  b.oxidation.member_threshold = 0.8;
  verifyChargeControlPair(a,b,1);
  for (const mutate of [
    s => s.prey_tick_integral++, s => s.closing_ecology_projection_hash = '8',
    s => s.oxidation.charging_above_reference.transactions++,
    s => s.reproductive_opportunity.member_ticks++,
  ]) { const bad = structuredClone(b); mutate(bad); assert.throws(() => verifyChargeControlPair(a,bad,1)); }
  assert.throws(() => verifyChargeControlPair(a,b,0)); // Untouched has no policy hash exception.
});

test('zero-rate, no-member and impossible transaction counts cannot claim charging', () => {
  const o = opening(true);
  for (const mutate of [
    p => p.config.organism.oxidation_rate = 0,
    p => p.arm = 'budget_control',
  ]) { const bad = structuredClone(o); mutate(bad); assert.throws(() => verifyOxidation(summary(),0.8,bad)); }
  const s = summary(); s.oxidation.charging_above_reference.transactions = 144000*512+1;
  assert.throws(() => verifyOxidation(s,0.8,o));
  // Boundary/headroom loss is legitimate: all released energy may leave as heat.
  const capped = summary();
  capped.oxidation.charging_above_reference.energy_gained = 0;
  capped.oxidation.charging_above_reference.conversion_heat = 0.001;
  verifyOxidation(capped,0.8,o);
});
