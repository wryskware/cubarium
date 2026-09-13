import test from 'node:test';
import assert from 'node:assert/strict';
import {reduceReproductionLedger, reduceMembers, reduceCensus, reserveBudget, priorRunDiff,
  compactView, FUNDING, DT} from './reduce-hunter-charge-rerun.mjs';

const id = (slot, generation) => ({slot, generation});
const repro = (tick, transaction, extra = {}) =>
  ({stream: 'hunter', event: {tick, kind: 'reproduction', record: {transaction, ...extra}}});
const funded = (tick, parent, started, over = {}) => repro(tick, 'funded', {
  key: {parent, started_tick: started},
  parent_reserve_before: 3.3, parent_reserve_after: 3.3 - FUNDING.reserve_debit,
  parent_energy_before: 3.1, parent_energy_after: 3.1 - FUNDING.energy_debit,
  escrow_structure: FUNDING.escrow_structure, escrow_reserve: FUNDING.escrow_reserve,
  escrow_energy: FUNDING.escrow_energy, build_heat: FUNDING.build_heat, ...over});

test('the ledger follows each funding key to exactly one outcome and keeps refusals', () => {
  const rows = [
    funded(10, id(1, 1), 9),
    repro(20, 'born', {key: {parent: id(1, 1), started_tick: 9}, birth_heat: FUNDING.birth_heat}),
    funded(30, id(2, 1), 29),
    repro(40, 'refunded', {key: {parent: id(2, 1), started_tick: 29},
      refunded_structure: 0.8, refunded_reserve: 0.8, refunded_energy: 0.6}),
    funded(50, id(3, 1), 49), // still open at the horizon
    repro(60, 'not_funded', {reason: 'stocks'}),
    repro(61, 'not_funded', {reason: 'cap'}),
    {stream: 'life', event: {tick: 62, kind: 'birth'}}, // a non-member birth is not a gestation
  ];
  const l = reduceReproductionLedger(rows);
  assert.deepEqual(l.counts, {funded: 3, born: 1, refunded: 1, miscarried: 0, closed: 2,
    open_at_horizon: 1, not_funded_cap: 1, not_funded_stocks: 1});
  assert.deepEqual(l.violations, []);
  assert.equal(l.heat.build, 3 * FUNDING.build_heat);
  assert.equal(l.heat.birth, FUNDING.birth_heat);
  assert.equal(l.material.born, FUNDING.escrow_structure + FUNDING.escrow_reserve);
  assert.deepEqual(l.gestations.map(g => g.outcome), ['born', 'refunded', 'open_at_horizon']);
});

test('a funding record that does not pay the exact contract is reported, not silently totalled', () => {
  for (const [over, field] of [
    [{parent_reserve_after: 3.3 - 1.5}, 'reserve_debit'],
    [{parent_energy_after: 3.1 - 0.9}, 'energy_debit'],
    [{escrow_structure: 0.7}, 'escrow_structure'],
    [{escrow_reserve: 0.9}, 'escrow_reserve'],
    [{escrow_energy: 0.5}, 'escrow_energy'],
    [{build_heat: 0.3}, 'build_heat'],
  ]) {
    const l = reduceReproductionLedger([funded(10, id(1, 1), 9, over)]);
    assert.deepEqual(l.violations.map(v => v.field), [field]);
  }
  const wrongBirthHeat = reduceReproductionLedger([funded(10, id(1, 1), 9),
    repro(20, 'born', {key: {parent: id(1, 1), started_tick: 9}, birth_heat: 1.2})]);
  assert.deepEqual(wrongBirthHeat.violations.map(v => v.field), ['birth_heat']);
});

test('the ledger refuses a malformed gestation history rather than guessing', () => {
  assert.throws(() => reduceReproductionLedger([funded(10, id(1, 1), 9), funded(11, id(1, 1), 9)]),
    /funded twice/);
  assert.throws(() => reduceReproductionLedger([
    repro(20, 'born', {key: {parent: id(1, 1), started_tick: 9}, birth_heat: FUNDING.birth_heat})]),
    /unknown funding key/);
  assert.throws(() => reduceReproductionLedger([funded(10, id(1, 1), 9),
    repro(20, 'born', {key: {parent: id(1, 1), started_tick: 9}, birth_heat: FUNDING.birth_heat}),
    repro(30, 'miscarried', {key: {parent: id(1, 1), started_tick: 9}})]), /closed twice/);
  assert.throws(() => reduceReproductionLedger([repro(10, 'funded', {})]), /without a funding key/);
});

test('members keep placed founders that never act, and mark censored survivors', () => {
  const rows = [
    {stream: 'hunter', event: {tick: 100, kind: 'offspring', child: id(7, 2), parent: id(1, 1)}},
    {stream: 'hunter', event: {tick: 110, kind: 'attempt', hunter: id(7, 2), outcome: 'OutOfReach', energy_paid: 0}},
    {stream: 'hunter', event: {tick: 120, kind: 'attempt', hunter: id(7, 2), outcome: 'Captured', energy_paid: 0.08}},
    {stream: 'hunter', event: {tick: 120, kind: 'capture', hunter: id(7, 2), material: 0.5, energy: 1.0}},
    {stream: 'hunter', event: {tick: 200, kind: 'death', id: id(7, 2), cause: 'Starvation'}},
  ];
  const m = reduceMembers(rows, 0, 400, ['1:1', '9:9']);
  assert.deepEqual([...m.keys()].sort(), ['1:1', '7:2', '9:9']);
  const quiet = m.get('9:9');
  assert.equal(quiet.origin, 'placed_founder');
  assert.equal(quiet.censored_alive, true);
  assert.equal(quiet.observed_seconds, 400 * DT);
  const child = m.get('7:2');
  assert.equal(child.origin, 'paid_descendant');
  assert.equal(child.parent, '1:1');
  assert.equal(child.captures, 1);
  assert.equal(child.paid_strike_energy, 0.08);
  assert.deepEqual(child.attempts, {OutOfReach: 1, Captured: 1});
  assert.equal(child.observed_seconds, 100 * DT);
  assert.equal(child.death_cause, 'Starvation');
});

const stock = over => ({id: id(1, 1), structure: 0.8, adult_structure: 2, reserve: 0, energy: 0,
  reserve_max: 4, energy_max: 4, gut_material: 0, gut_energy: 0, phase: 'Perched', blockers: ['juvenile'], ...over});
const cfg = {growth_reserve_min: 0.3, world_threshold: 0.5, member_threshold: 0.8, oxidation_rate: 0.01,
  reserve_energy_density: 2, assimilation_material: 0.6, assimilation_energy: 0.5, build_cost: 0.5};

test('census reduction counts boundaries, not ticks, and separates the two oxidation thresholds', () => {
  const rows = [
    {tick: 200, adults: 0, hunter_stocks: [stock({reserve: 1.3, energy: 2.2})]},
    {tick: 400, adults: 1, hunter_stocks: [stock({structure: 2, reserve: 0.1, energy: 3.5})]},
    {tick: 600, adults: 0, hunter_stocks: []},
  ];
  const {sampled, perMember} = reduceCensus(rows, cfg);
  assert.equal(sampled.boundaries, 3);
  assert.equal(sampled.cadence_ticks, 200);
  assert.deepEqual(sampled.adult_occupancy, [2, 1, 0, 0]);
  const s = perMember.get('1:1');
  assert.equal(s.samples, 2);
  assert.equal(s.samples_above_growth_gate, 1);          // 1.3 > 0.3 * 4, 0.1 is not
  assert.equal(s.samples_above_world_oxidation_shutoff, 2);  // 2.2 and 3.5 both clear 0.5 * 4
  assert.equal(s.samples_above_member_oxidation_shutoff, 1); // only 3.5 clears 0.8 * 4
  assert.equal(s.ever_adult, true);
  assert.equal(s.reserve_zero_samples, 0);
  assert.deepEqual(s.structure_values, [0.8, 2]);
  assert.equal(s.structure_decreased, false);
});

test('census reduction refuses a boundary whose adult count disagrees with its own stocks', () => {
  assert.throws(() => reduceCensus([{tick: 200, adults: 1, hunter_stocks: [stock({})]}], cfg),
    /adults disagrees/);
});

test('a shrinking structure is flagged rather than absorbed into a maximum', () => {
  const {perMember} = reduceCensus([
    {tick: 200, adults: 0, hunter_stocks: [stock({structure: 1.2})]},
    {tick: 400, adults: 0, hunter_stocks: [stock({structure: 0.9})]}], cfg);
  assert.equal(perMember.get('1:1').structure_decreased, true);
  assert.equal(perMember.get('1:1').min_structure, 0.9);
});

test('the reserve budget uses the recorded meals and the configured oxidation rate', () => {
  const child = {origin: 'paid_descendant', observed_seconds: 100,
    meals: [{material: 1, energy: 4}, {material: 2, energy: 1}]};
  const b = reserveBudget(child, cfg);
  // First meal: density 4 -> capped at 1, eta = 0.6. Second: density 0.5, eta = 0.6 * 0.25 = 0.15.
  assert.equal(b.assimilable_reserve_from_meals, 0.6 * 1 + 0.15 * 2);
  assert.equal(b.birth_reserve_escrow, FUNDING.escrow_reserve);
  assert(Math.abs(b.supply - 1.7) < 1e-12);
  assert.equal(b.oxidation_demand_whole_life, 1);
  assert(Math.abs(b.supply_over_demand - 1.7) < 1e-12);
  const founder = reserveBudget({origin: 'placed_founder', observed_seconds: 0, meals: []}, cfg);
  assert.equal(founder.birth_reserve_escrow, null);
  assert.equal(founder.supply_over_demand, null); // no division by a zero life
});

test('prior-run reproduction is decided by the closing state hash, not by a chosen statistic', () => {
  const armOf = over => ({closing_state_hash: 'h', closing_tick: 288000, technical_complete: true,
    complete_experiment_measurement: true, captures: 1, offspring: 0, closing_hunters: 1,
    founder_extinction_tick: null, lineage_extinction_tick: null, ...over});
  const run = over => ({seeds: [{seed: 1, arms: Array.from({length: 6}, (_, i) =>
    armOf(i === 5 ? over : {}))}]});
  const same = priorRunDiff(run({}), run({}));
  assert.equal(same.arms_reproduced_bit_exact, 6);
  assert.equal(same.arms_changed, 0);
  // Same captures and same closing tick, different world: still a change.
  const drift = priorRunDiff(run({}), run({closing_state_hash: 'other'}));
  assert.equal(drift.arms_changed, 1);
  assert.equal(drift.changed[0].arm, '1/facultative_on');
  assert.throws(() => priorRunDiff(run({}), {seeds: []}), /missing/);
});

test('the compact projection drops no seed, arm, channel or unsuccessful outcome', () => {
  const side = () => ({ledger_hunting: {captures: 0, attempts: {}, paid_strike_energy: 0},
    ledger_reproduction: {counts: {funded: 0, born: 0}, gestations: []},
    summary_survival: {closing_hunters: 0}, summary_adult_occupancy_ticks: [1, 0, 0, 0],
    sampled_adult_occupancy_boundaries: [1, 0, 0, 0],
    summary_reproductive_opportunity: {member_ticks: 0, age_and_size_ready_member_ticks: 0,
      age_and_size_ready_reserve_gate_open_member_ticks: 0,
      age_and_size_ready_energy_gate_open_member_ticks: 0,
      age_and_size_ready_both_stock_gates_open_member_ticks: 0,
      max_reserve_fraction: 0, max_energy_fraction: 0},
    summary_blocker_member_ticks: {}, charging_above_reference: {transactions: 0},
    summary_prey: {closing_prey: 0, prey_min: 0, prey_tick_integral: 0,
      // The shape `reduceArm` produces: `opening_count`/`minimum_count` are already renamed.
      recovery: Array.from({length: 9}, (_, channel) => ({channel, opening: 0, minimum: 0,
        status: 'no_opportunity', first_zero_tick: null, ticks_below_half: 0,
        crossing_tick: null, confirmation_tick: null}))},
    summary_audit: {passed: true, legacy_passed: true, failure: null,
      peak_over_limit: [0.1, 0.2, 0.3, 0.4].map(ratio => ({ratio, first_crossing: null, nonfinite_at: null}))},
    founders: [], closing_census: {}});
  const full = {seeds: Array.from({length: 12}, (_, s) => ({seed: s + 1,
    arms: Array.from({length: 6}, (_, a) => ({arm: `arm${a}`, background: side(), candidate: side()}))}))};
  const c = compactView(full);
  assert.equal(c.seeds.length, 12);
  assert.equal(c.seeds.flatMap(s => s.arms).length, 72);
  assert.equal(c.seeds[0].arms[0].candidate.prey.recovery.length, 9);
  assert.deepEqual(c.seeds[0].arms[0].candidate.prey.recovery[8], [0, 0, 'no_opportunity', null, 0, null, null]);
  assert.deepEqual(c.seeds[0].arms[0].background.audit.peak_over_fixed_limit, [0.1, 0.2, 0.3, 0.4]);
  assert.equal(c.seeds[0].arms[0].background.audit.any_crossing, false);
  assert.equal(c.compact_legend.prey_recovery_row.length, 7);
});
