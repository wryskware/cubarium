import test from 'node:test';
import assert from 'node:assert/strict';
import {checkGates, boundaryAccounting, childRecord, crossCheck, cohortTotals, idKey,
  DEATH_TICK_OFFSET} from './reduce-juvenile-flow-cohort.mjs';

const opening = () => ({arm: 'specialist_on', profile_recipe: 'reserve-targets-charge80-v1',
  post_snapshot_sha256: 'aaa', post_state_hash: '111'});
const summary = () => ({closing_snapshot_sha256: 'bbb', closing_state_hash: '222', offspring: 1});
const report = () => ({
  kind: 'juvenile-mutation-site-flow-diagnostic',
  replayed_ticks: 144000,
  opening: {seed: 8, arm: 'specialist_on', profile_recipe: 'reserve-targets-charge80-v1',
    snapshot_sha256: 'aaa', state_hash: '111'},
  closing_identity: {checked: true, closing_snapshot_sha256: 'bbb', closing_state_hash: '222'},
  observer_neutrality: {checked: true, closing_bytes_equal: true, state_hash_equal: true,
    event_ticks_compared: 997},
  reconciliation: {violations: 0},
  ledger: {unregistered_records: 0, members: []},
});

test('a sound arm reports no gate failures', () => {
  assert.deepEqual(checkGates(report(), opening(), summary()), []);
});

test('every gate is checked against the retained artifact, not against the report\'s own claim', () => {
  const cases = [
    [r => {r.opening.snapshot_sha256 = 'zzz';}, 'input_identity'],
    [r => {r.opening.state_hash = 'zzz';}, 'input_identity'],
    [r => {r.closing_identity.closing_snapshot_sha256 = 'zzz';}, 'output_identity'],
    [r => {r.closing_identity.closing_state_hash = 'zzz';}, 'output_identity'],
    [r => {r.closing_identity.checked = false;}, 'output_identity'],
    [r => {r.observer_neutrality.state_hash_equal = false;}, 'observer_neutrality'],
    [r => {r.observer_neutrality.event_ticks_compared = 0;}, 'observer_neutrality'],
    [r => {r.reconciliation.violations = 3;}, 'reconciliation'],
    [r => {r.ledger.unregistered_records = 1;}, 'reconciliation'],
    [r => {r.opening.arm = 'facultative_on';}, 'provenance'],
    [r => {r.replayed_ticks = 32000;}, 'horizon'],
  ];
  for (const [mutate, gate] of cases) {
    const r = report();
    mutate(r);
    const failures = checkGates(r, opening(), summary());
    assert(failures.length >= 1, `${gate} was not caught`);
    assert(failures.some(f => f.gate === gate), `expected ${gate}, got ${JSON.stringify(failures)}`);
  }
});

test('a prefix run is refused rather than pooled with full-horizon arms', () => {
  const r = report();
  r.replayed_ticks = 32000;
  r.closing_identity = {checked: false, reason: 'prefix'};
  const failures = checkGates(r, opening(), summary()).map(f => f.gate);
  assert(failures.includes('horizon'));
  assert(failures.includes('output_identity'));
});

const member = over => ({
  id: {slot: 41, generation: 5}, parent: {slot: 6, generation: 6}, origin: 'Descendant',
  born_tick: 173805, end_tick: 196434, end_cause: 'Starvation',
  open_stocks: {tick: 173805, structure: 0.8, reserve: 0.8, energy: 0.6},
  close_stocks: {tick: 196433, structure: 0.8, reserve: 0, energy: 0},
  death_stocks: {tick: 196434, structure: 0.8, reserve: 0, energy: 0},
  juvenile_ticks: 22630, adult_ticks: 0,
  digestion: {ticks: 1189, to_reserve: 2.0, energy_gain: 1.0, material: 4, to_detritus: 2, heat: 1},
  frugivory: {ticks: 0, to_reserve: 0, energy_gain: 0}, grazing: {ticks: 0, to_reserve: 0, energy_gain: 0},
  scavenging: {ticks: 0, to_reserve: 0, energy_gain: 0},
  oxidation: {ticks: 7050, reserve_burned: 2.8, energy_gained: 4.48, heat: 1.12,
    above_reference_ticks: 846, above_reference_reserve_burned: 0.28},
  growth: {ticks: 0, reserve_spent: 0, structure_gained: 0, energy_cost: 0, heat: 0,
    bound_by_rate: 0, bound_by_remaining_structure: 0, bound_by_reserve: 0, bound_by_energy: 0},
  upkeep: {ticks: 22630, demanded_maintenance: 2.263, demanded_move: 0.888, demanded_sense: 2.716,
    demanded_total: 5.867, paid: 5.867, shortfall: 0, shortfall_ticks: 0},
  strike: {ticks: 30, demanded: 2.4, paid: 2.4, shortfall: 0},
  handling: {ticks: 1189, demanded: 0.119, paid: 0.119, unaffordable_ticks: 0},
  funding: {funded_count: 0, reserve_debit: 0, energy_debit: 0, refunded_reserve: 0, refunded_energy: 0},
  gate: {observed_ticks: 22630, structure_below_adult_ticks: 22630, reserve_above_min_ticks: 0,
    branch_entered_ticks: 0, gate_reserve: 1.2, min_reserve_deficit: 0.3455, max_reserve: 0.8545,
    reserve_sum: 1597.4, max_energy: 2.223, energy_sum: 31715.3, reserve_zero_ticks: 15590},
  residual: {checked_ticks: 22631, max_structure: 0, max_reserve: 8.7e-17, max_energy: 4.7e-16, violations: 0},
  ...over});
const derived = {reserve_out_total: 2.8, energy_in_total: 5.48, energy_out_total: 8.386};

test('boundary accounting separates gate observations from reconciliation checks', () => {
  const child = boundaryAccounting(member());
  assert.equal(child.gate_observation_ticks, 22630);
  assert.equal(child.reconciliation_checks, 22631);
  assert.equal(child.newborn_boundary_checks, 1);
  assert.equal(child.removal_boundary_checks, 1);
  assert.equal(child.checks_minus_gate_ticks, 1);
  assert.equal(child.identity_holds, true);

  // A founder present before the replay starts is probed and gate-observed on the same ticks.
  const founder = boundaryAccounting(member({origin: 'Founder', parent: null,
    gate: {...member().gate, observed_ticks: 39649},
    residual: {...member().residual, checked_ticks: 39649}}));
  assert.equal(founder.newborn_boundary_checks, 0);
  assert.equal(founder.removal_boundary_checks, 1);
  assert.equal(founder.checks_minus_gate_ticks, 0);
  assert.equal(founder.identity_holds, true);

  // An unexplained extra check is surfaced, not absorbed.
  const odd = boundaryAccounting(member({residual: {...member().residual, checked_ticks: 22640}}));
  assert.equal(odd.identity_holds, false);
  assert.equal(odd.checks_minus_gate_ticks, 10);
});

test('a child record keeps demand and payment apart and never apportions the lump', () => {
  const r = childRecord(8, 'specialist_on', member(), derived);
  assert.equal(r.id, '41:5');
  assert.equal(r.parent, '6:6');
  assert.equal(r.censored_alive, false);
  assert.equal(r.upkeep.paid_lump, 5.867);
  assert.equal(r.upkeep.demanded_total, 5.867);
  assert(Math.abs(r.upkeep.per_tick_sense - 2.716 / 22630) < 1e-15);
  // No field names a per-term payment.
  assert.deepEqual(Object.keys(r.upkeep).filter(k => /^paid_(maintenance|move|sense)/.test(k)), []);
  assert.equal(r.growth.structure_gained, 0);
  assert.equal(r.growth.branch_entered_ticks, 0);
  assert.deepEqual(r.reserve.in_by_source, {digestion: 2.0, frugivory: 0, grazing: 0, scavenging: 0});
  assert.equal(r.reserve.in_total, 2.0);
  // 0.8 opened + 2.0 in - 2.8 oxidised - 0 at death
  assert(Math.abs(r.reserve.closure_residual) < 1e-12);
  assert(Math.abs(r.oxidation_recorded.share_of_burn_recorded_above_reference - 0.1) < 1e-12);
});

test('a censored child is carried as censored, not as a zero-lifetime death', () => {
  const r = childRecord(2, 'facultative_on',
    member({end_tick: null, end_cause: null, death_stocks: null,
      close_stocks: {tick: 288000, structure: 0.8, reserve: 0.1, energy: 0.2}}), derived);
  assert.equal(r.censored_alive, true);
  assert.equal(r.end_tick, null);
  assert.equal(r.death_stocks, null);
  assert.equal(r.boundaries.removal_boundary_checks, 0);
});

test('a zero-capture child survives the reduction with explicit zeros', () => {
  const quiet = member({
    digestion: {ticks: 0, to_reserve: 0, energy_gain: 0, material: 0, to_detritus: 0, heat: 0},
    oxidation: {ticks: 100, reserve_burned: 0.8, energy_gained: 1.28, heat: 0.32,
      above_reference_ticks: 0, above_reference_reserve_burned: 0},
    strike: {ticks: 0, demanded: 0, paid: 0, shortfall: 0},
    handling: {ticks: 0, demanded: 0, paid: 0, unaffordable_ticks: 0},
  });
  const r = childRecord(1, 'specialist_on', quiet, {reserve_out_total: 0.8, energy_in_total: 1.28, energy_out_total: 1});
  assert.equal(r.reserve.in_total, 0);
  assert.equal(r.strike.ticks, 0);
  assert.equal(r.oxidation_recorded.share_of_burn_recorded_above_reference, 0);
  // A child with no burn at all reports null rather than a fabricated zero share.
  const never = childRecord(1, 'specialist_on', member({
    oxidation: {ticks: 0, reserve_burned: 0, energy_gained: 0, heat: 0,
      above_reference_ticks: 0, above_reference_reserve_burned: 0}}), derived);
  assert.equal(never.oxidation_recorded.share_of_burn_recorded_above_reference, null);
});

test('the cross-check compares identities across two independent measurement paths', () => {
  const ledger = [childRecord(8, 'specialist_on', member(), derived)];
  ledger[0].digestion_ticks = 1189;
  // The event stream labels the death boundary one above the ledger's pre-increment counter,
  // and the span is 22630 either way.
  const censusChild = {seed: 8, arm: 'specialist_on', id: '41:5', birth_tick: 173805,
    death_tick: 196434 + DEATH_TICK_OFFSET, death_cause: 'Starvation', censored_alive: false,
    captures: 13, attempts: {Captured: 13, Missed: 11, OutOfReach: 6}, paid_strike_energy: 2.4};
  const agreed = crossCheck(ledger, [censusChild])[0];
  assert.equal(agreed.agreed, true, agreed.why);
  assert.equal(agreed.census_lifespan_ticks, 22630);
  assert.equal(agreed.ledger_gate_observation_ticks, 22630);
  // An attempt the member could not pay for never reaches the charge site, so it is counted
  // by the event stream and absent from the ledger by construction — not a disagreement.
  const unaffordable = crossCheck(ledger, [{...censusChild,
    attempts: {Captured: 13, Missed: 11, OutOfReach: 6, Unaffordable: 12}}])[0];
  assert.equal(unaffordable.agreed, true, unaffordable.why);
  assert.equal(unaffordable.census_attempts, 42);
  assert.equal(unaffordable.census_unaffordable_attempts, 12);
  assert.equal(unaffordable.census_payable_attempts, 30);

  for (const [patch, why] of [
    [{death_tick: 196434}, 'death boundary'], [{death_cause: 'Age'}, 'death_cause'],
    [{birth_tick: 173800}, 'birth_tick'], [{attempts: {Captured: 1}}, 'payable attempts'],
    [{paid_strike_energy: 1}, 'paid strike energy'], [{censored_alive: true}, 'censored_alive'],
  ]) {
    const row = crossCheck(ledger, [{...censusChild, ...patch}])[0];
    assert.equal(row.agreed, false, why);
    assert(row.why.includes(why), `${row.why} should mention ${why}`);
  }
  // The span check is convention-free: a lifespan that disagrees is caught even when both
  // ends are labelled consistently.
  const stretched = crossCheck(ledger, [{...censusChild, birth_tick: 173805, death_tick: 199999}])[0];
  assert.equal(stretched.agreed, false);
  assert(stretched.why.includes('lifespan'));
  assert.equal(crossCheck(ledger, [])[0].why, 'absent from the census reduction');
});

test('cohort totals count outcomes without dropping the quiet cases', () => {
  const a = childRecord(8, 'specialist_on', member(), derived);
  const b = childRecord(2, 'facultative_on', member({end_tick: null, end_cause: null, death_stocks: null}), derived);
  const c = childRecord(1, 'specialist_on', member({
    digestion: {ticks: 0, to_reserve: 0, energy_gain: 0, material: 0, to_detritus: 0, heat: 0}}), derived);
  const t = cohortTotals([a, b, c]);
  assert.equal(t.children, 3);
  assert.equal(t.censored_alive, 1);
  assert.deepEqual(t.ended_by_cause, {Starvation: 2, censored_alive: 1});
  assert.equal(t.children_with_zero_reserve_intake, 1);
  assert.equal(t.children_that_entered_growth, 0);
  assert.equal(t.children_that_gained_structure, 0);
  assert.equal(t.children_ever_adult, 0);
  assert.equal(t.children_whose_reserve_ever_cleared_gate, 0);
  assert.equal(t.total_gate_observation_ticks, 3 * 22630);
  assert.equal(t.total_reconciliation_checks, 3 * 22631);
  assert(Math.abs(t.smallest_reserve_deficit - 0.3455) < 1e-12);
});

test('ids are formatted slot:generation', () => {
  assert.equal(idKey({slot: 41, generation: 5}), '41:5');
});
