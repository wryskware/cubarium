// Bug-demonstration probes for reducer 35861cc, not ecological simulation tests.
// These assertions must stop passing when their corresponding gaps are fixed.
import test from 'node:test';
import assert from 'node:assert/strict';
import {verifyArm, reduceEvents} from '../../scripts/compare-hunter-recipes.mjs';

function arm() {
  const peak = () => ({magnitude: 1e-12, first_crossing: null, nonfinite_at: null});
  return {
    technical_complete: true, complete_experiment_measurement: true,
    planned_ticks: 144000, closing_tick: 288000,
    last_complete_observer_tick: 288000, last_complete_reproduction_tick: 288000,
    observer_statistics_trusted_through_closing_tick: true,
    audit: {passed: true, opening_tick: 144000, fixed_limits: [1e-6, 1e-6, 1e-6],
      material: peak(), corrected_energy: peak(), independent_energy: peak(), water: peak()},
    adult_occupancy_ticks_0_1_2_over2: [143990, 10, 0, 0], offspring: 0, open_gestations: 0,
    reproduction_audit: {last_complete_tick: 288000, ticks_observed: 144000,
      counts: {funded: 0, closed: 0, open_at_horizon: 0, born: 0, refunded: 0, miscarried: 0}},
    reproductive_opportunity: {member_ticks: 10, age_and_size_ready_member_ticks: 10,
      age_and_size_ready_reserve_gate_open_member_ticks: 10,
      age_and_size_ready_energy_gate_open_member_ticks: 10,
      age_and_size_ready_both_stock_gates_open_member_ticks: 10},
    whole_recovery: Array.from({length: 9}, () => ({})),
  };
}

test('35861cc accepts equality at an explicitly strict numerical limit', () => {
  const a = arm();
  a.audit.material.magnitude = a.audit.fixed_limits[0];
  assert.doesNotThrow(() => verifyArm(a, 144000, 144000));
});

test('35861cc accepts every mature tick passing both separate gates but no joint tick', () => {
  const a = arm();
  a.reproductive_opportunity.age_and_size_ready_both_stock_gates_open_member_ticks = 0;
  assert.doesNotThrow(() => verifyArm(a, 144000, 144000));
});

test('35861cc classifies a missing contact coordinate as a far miss', () => {
  const result = reduceEvents([{stream: 'hunter', event: {
    kind: 'attempt', tick: 144001, outcome: 'OutOfReach', energy_paid: 0.08,
    evidence: {measure: {body: {}}, geometry: {capture_offset_body: {x: 13}}},
  }}], 144000, 288000);
  assert.equal(result.far_out_of_reach, 1);
});
