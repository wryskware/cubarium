import test from 'node:test';
import assert from 'node:assert/strict';
import {checkGates, checkReportedIdentity, checkReportedClaims, checkArtifactIdentity,
  validateMemberNumbers, checkMemberReconciliation, boundaryAccounting, memberRecord,
  intakeBySource, crossCheck, cohortTotals, failureSummary, idKey, childKey,
  DEATH_TICK_OFFSET, RESIDUAL_TOLERANCE, EXPECTED_ARMS, EXPECTED_CHILDREN, EXPECTED_MEMBERS,
  INTAKE_SOURCES, SNAPSHOT_SCHEMA} from './reduce-juvenile-flow-cohort.mjs';

const opening = () => ({arm: 'specialist_on', profile_recipe: 'reserve-targets-charge80-v1',
  post_snapshot_sha256: 'aaa', post_state_hash: '111'});
const summary = () => ({closing_snapshot_sha256: 'bbb', closing_state_hash: '222', offspring: 1});
const report = () => ({
  kind: 'juvenile-mutation-site-flow-diagnostic',
  arm: '/retained/seed-8/specialist_on',
  replayed_ticks: 144000,
  opening: {seed: 8, arm: 'specialist_on', profile_recipe: 'reserve-targets-charge80-v1',
    tick: 144000, snapshot_sha256: 'aaa', state_hash: '111'},
  closing_identity: {checked: true, closing_snapshot_sha256: 'bbb', closing_state_hash: '222'},
  observer_neutrality: {checked: true, closing_bytes_equal: true, state_hash_equal: true,
    event_ticks_compared: 997},
  reconciliation: {violations: 0, tolerance: RESIDUAL_TOLERANCE},
  ledger: {unregistered_records: 0, members: []},
});
const actual = () => ({
  opening: {schema: SNAPSHOT_SCHEMA, build: '0.1.0+512ee52', sha256: 'aaa', state_hash: '111'},
  closing: {schema: SNAPSHOT_SCHEMA, build: '0.1.0+512ee52', sha256: 'bbb', state_hash: '222'},
});

test('the frozen cohort is eleven arms, eighteen children, twenty-nine members', () => {
  assert.equal(EXPECTED_ARMS.length, 11);
  assert.equal(EXPECTED_CHILDREN, 18);
  assert.equal(EXPECTED_MEMBERS, 29);
  assert.equal(new Set(EXPECTED_ARMS.map(a => `${a.seed}/${a.arm}`)).size, 11);
  assert.equal(RESIDUAL_TOLERANCE, 1e-9);
  assert.deepEqual(INTAKE_SOURCES, ['digestion', 'frugivory', 'grazing', 'scavenging']);
});

test('a sound arm reports no failures on either the label or the byte path', () => {
  assert.deepEqual(checkGates(report(), opening(), summary()), []);
  assert.deepEqual(checkArtifactIdentity(report(), opening(), summary(), actual()), []);
});

test('label-level identity is named as reported, not as independently derived', () => {
  for (const [mutate, gate] of [
    [r => {r.opening.snapshot_sha256 = 'zzz';}, 'reported_input_identity'],
    [r => {r.opening.state_hash = 'zzz';}, 'reported_input_identity'],
    [r => {r.closing_identity.closing_snapshot_sha256 = 'zzz';}, 'reported_output_identity'],
    [r => {r.closing_identity.checked = false;}, 'reported_output_identity'],
    [r => {r.opening.arm = 'facultative_on';}, 'provenance'],
    [r => {r.replayed_ticks = 32000;}, 'horizon'],
    [r => {r.opening.tick = 0;}, 'horizon'],
  ]) {
    const r = report();
    mutate(r);
    const failures = checkReportedIdentity(r, opening(), summary()).map(f => f.gate);
    assert(failures.includes(gate), `expected ${gate}, got ${failures}`);
  }
});

test('the byte path catches a snapshot that disagrees with either JSON or the report', () => {
  for (const [mutate, gate] of [
    [a => {a.opening.sha256 = 'zzz';}, 'input_identity'],
    [a => {a.opening.state_hash = 'zzz';}, 'input_identity'],
    [a => {a.opening.schema = 11;}, 'input_identity'],
    [a => {a.closing.sha256 = 'zzz';}, 'output_identity'],
    [a => {a.closing.state_hash = 'zzz';}, 'output_identity'],
    [a => {a.closing.schema = 13;}, 'output_identity'],
    [a => {a.closing.build = '0.1.0+other';}, 'output_identity'],
  ]) {
    const a = actual();
    mutate(a);
    const failures = checkArtifactIdentity(report(), opening(), summary(), a).map(f => f.gate);
    assert(failures.includes(gate), `expected ${gate}, got ${failures}`);
  }
});

test('the replay\'s own claims are labelled reported and judged against the frozen tolerance', () => {
  for (const [mutate, gate] of [
    [r => {r.observer_neutrality.state_hash_equal = false;}, 'reported_observer_neutrality'],
    [r => {r.observer_neutrality.closing_bytes_equal = false;}, 'reported_observer_neutrality'],
    [r => {r.observer_neutrality.event_ticks_compared = 0;}, 'reported_observer_neutrality'],
    [r => {r.reconciliation.violations = 3;}, 'reported_reconciliation'],
    [r => {r.ledger.unregistered_records = 1;}, 'reported_reconciliation'],
    // A report may not widen the tolerance its evidence is judged against.
    [r => {r.reconciliation.tolerance = 1;}, 'reported_reconciliation'],
  ]) {
    const r = report();
    mutate(r);
    const failures = checkReportedClaims(r).map(f => f.gate);
    assert(failures.includes(gate), `expected ${gate}, got ${failures}`);
  }
});

const member = over => ({
  id: {slot: 41, generation: 5}, parent: {slot: 6, generation: 6}, origin: 'Descendant',
  born_tick: 173805, end_tick: 196434, end_cause: 'Starvation',
  open_stocks: {tick: 173805, structure: 0.8, reserve: 0.8, energy: 0.6},
  close_stocks: {tick: 196433, structure: 0.8, reserve: 0, energy: 0},
  death_stocks: {tick: 196434, structure: 0.8, reserve: 0, energy: 0},
  juvenile_ticks: 22630, adult_ticks: 0,
  digestion: {ticks: 1189, to_reserve: 2.0, energy_gain: 1.0, material: 4, to_detritus: 2, heat: 1},
  frugivory: {ticks: 0, to_reserve: 0, energy_gain: 0, material: 0, heat: 0},
  grazing: {ticks: 0, to_reserve: 0, energy_gain: 0, material: 0, heat: 0},
  scavenging: {ticks: 0, to_reserve: 0, energy_gain: 0, material: 0, heat: 0},
  oxidation: {ticks: 7050, reserve_burned: 2.8, energy_gained: 4.48, heat: 1.12,
    above_reference_ticks: 846, above_reference_reserve_burned: 0.28},
  growth: {ticks: 0, reserve_spent: 0, structure_gained: 0, energy_cost: 0, heat: 0,
    bound_by_rate: 0, bound_by_remaining_structure: 0, bound_by_reserve: 0, bound_by_energy: 0},
  upkeep: {ticks: 22630, demanded_maintenance: 2.263, demanded_move: 0.888, demanded_sense: 2.716,
    demanded_total: 5.867, paid: 5.867, shortfall: 0, shortfall_ticks: 0},
  strike: {ticks: 30, demanded: 2.4, paid: 2.4, shortfall: 0},
  handling: {ticks: 1189, demanded: 0.119, paid: 0.119, unaffordable_ticks: 0},
  funding: {funded_count: 0, reserve_debit: 0, energy_debit: 0, build_heat: 0, escrow_structure: 0,
    escrow_reserve: 0, escrow_energy: 0, refunded_count: 0, refunded_reserve: 0, refunded_energy: 0},
  gate: {observed_ticks: 22630, structure_below_adult_ticks: 22630, reserve_above_min_ticks: 0,
    branch_entered_ticks: 0, gate_reserve: 1.2, min_reserve_deficit: 0.3455, max_reserve: 0.8545,
    reserve_sum: 1597.4, max_energy: 2.223, energy_sum: 31715.3, reserve_zero_ticks: 15590},
  residual: {checked_ticks: 22631, max_structure: 0, max_reserve: 8.7e-17, max_energy: 4.7e-16, violations: 0},
  ...over});

test('malformed member data is rejected rather than averaged over', () => {
  assert.deepEqual(validateMemberNumbers(member()), []);
  for (const mutate of [
    m => {m.residual.max_energy = NaN;},
    m => {m.residual.max_reserve = -1;},
    m => {m.open_stocks.reserve = -1;},
    m => {m.close_stocks.energy = Infinity;},
    m => {m.oxidation.reserve_burned = -0.5;},
    m => {m.scavenging.to_reserve = NaN;},
    m => {delete m.upkeep;},
    m => {m.origin = 'Ghost';},
    m => {m.born_tick = -1;},
    m => {m.gate.max_reserve = 'high';},
  ]) {
    const m = member();
    mutate(m);
    assert(validateMemberNumbers(m).length > 0, 'expected a malformed-record finding');
  }
  // A censored survivor legitimately carries no death stocks.
  assert.deepEqual(validateMemberNumbers(member({death_stocks: null, end_tick: null, end_cause: null})), []);
});

test('per-member reconciliation is judged against the frozen tolerance, not a reported one', () => {
  assert.deepEqual(checkMemberReconciliation(member()), []);
  assert.equal(checkMemberReconciliation(member({
    residual: {...member().residual, violations: 1}})).length, 1);
  // A violation hidden behind an aggregate zero is still a violation.
  assert.equal(checkMemberReconciliation(member({
    residual: {...member().residual, max_structure: 1e-8}})).length, 1);
  assert.deepEqual(checkMemberReconciliation(member({
    residual: {...member().residual, max_structure: RESIDUAL_TOLERANCE}})), []);
});

test('checkGates covers every member, not only the children', () => {
  const founder = member({origin: 'Founder', parent: null, id: {slot: 6, generation: 6},
    gate: {...member().gate, observed_ticks: 39649},
    residual: {...member().residual, checked_ticks: 39649, violations: 2}});
  const r = report();
  r.ledger.members = [founder, member()];
  const failures = checkGates(r, opening(), summary());
  assert(failures.some(f => f.gate === 'member_reconciliation' && f.detail.startsWith('6:6')),
    `a founder violation must be caught: ${JSON.stringify(failures)}`);
});

test('boundary accounting separates gate observations from reconciliation checks', () => {
  const child = boundaryAccounting(member());
  assert.equal(child.gate_observation_ticks, 22630);
  assert.equal(child.reconciliation_checks, 22631);
  assert.equal(child.newborn_boundary_checks, 1);
  assert.equal(child.removal_boundary_checks, 1);
  assert.equal(child.identity_holds, true);

  const founder = boundaryAccounting(member({origin: 'Founder', parent: null,
    gate: {...member().gate, observed_ticks: 39649},
    residual: {...member().residual, checked_ticks: 39649}}));
  assert.equal(founder.newborn_boundary_checks, 0);
  assert.equal(founder.removal_boundary_checks, 1);
  assert.equal(founder.identity_holds, true);

  const odd = boundaryAccounting(member({residual: {...member().residual, checked_ticks: 22640}}));
  assert.equal(odd.identity_holds, false);
  assert.equal(odd.checks_minus_gate_ticks, 10);
});

test('reserve intake is reported per source and never substituted by one channel', () => {
  const fed = member({scavenging: {ticks: 980, to_reserve: 0.1827, energy_gain: 0.05, material: 0.3, heat: 0.01}});
  const i = intakeBySource(fed);
  assert.deepEqual(Object.keys(i.by_source), INTAKE_SOURCES);
  assert(Math.abs(i.total - (2.0 + 0.1827)) < 1e-12);
  assert.deepEqual(i.sources_used, ['digestion', 'scavenging']);
  const r = memberRecord(8, 'specialist_on', fed);
  assert(Math.abs(r.reserve.in_total - 2.1827) < 1e-12);
  assert(r.reserve.in_total > r.reserve.in_by_source.digestion, 'the total must exceed digestion alone');
});

// The regression root asked for: a child that never digested but did scavenge has intake.
test('a child with zero digestion and nonzero scavenging is not a zero-intake child', () => {
  const scavenger = member({
    digestion: {ticks: 0, to_reserve: 0, energy_gain: 0, material: 0, to_detritus: 0, heat: 0},
    scavenging: {ticks: 980, to_reserve: 0.1827216171559611, energy_gain: 0.05, material: 0.3, heat: 0.01},
    strike: {ticks: 0, demanded: 0, paid: 0, shortfall: 0},
    handling: {ticks: 0, demanded: 0, paid: 0, unaffordable_ticks: 0},
  });
  const r = memberRecord(5, 'facultative_on', scavenger);
  assert.equal(r.reserve.in_by_source.digestion, 0);
  assert(r.reserve.in_total > 0, 'scavenging is acquisition');
  assert.deepEqual(r.reserve.in_sources_used, ['scavenging']);
  assert.equal(r.digestion_ticks, 0);
  assert.equal(r.scavenging_ticks, 980);

  const starved = memberRecord(1, 'specialist_on', member({
    digestion: {ticks: 0, to_reserve: 0, energy_gain: 0, material: 0, to_detritus: 0, heat: 0}}));
  const totals = cohortTotals([r, starved]);
  assert.equal(totals.children_with_zero_digestion, 2, 'both have zero digestion');
  assert.equal(totals.children_with_zero_reserve_intake, 1, 'only one has zero total intake');
  assert.equal(totals.children_that_scavenged, 1);
  assert.equal(totals.children_with_scavenging_but_no_digestion, 1);
  assert(Math.abs(totals.reserve_in_by_source.scavenging - 0.1827216171559611) < 1e-15);
  assert.equal(totals.reserve_in_by_source.frugivory, 0);
});

test('reserve above the birth escrow is counted, not asserted away', () => {
  const risen = memberRecord(2, 'facultative_on',
    member({gate: {...member().gate, max_reserve: 1.0870443357568322}}));
  assert.equal(risen.gate.max_reserve_above_birth_escrow, true);
  const flat = memberRecord(1, 'specialist_on', member({gate: {...member().gate, max_reserve: 0.7995}}));
  assert.equal(flat.gate.max_reserve_above_birth_escrow, false);
  const totals = cohortTotals([risen, flat]);
  assert.equal(totals.children_whose_reserve_exceeded_birth_escrow, 1);
  assert.equal(totals.children_whose_reserve_ever_cleared_gate, 0, 'neither cleared 1.2');
});

test('a censored child is carried as censored, not as a zero-lifetime death', () => {
  const r = memberRecord(2, 'facultative_on',
    member({end_tick: null, end_cause: null, death_stocks: null,
      close_stocks: {tick: 288000, structure: 0.8, reserve: 0.1, energy: 0.2}}));
  assert.equal(r.censored_alive, true);
  assert.equal(r.death_stocks, null);
  assert.equal(r.boundaries.removal_boundary_checks, 0);
  // Closure is measured against the last probe when there is no removal site.
  assert(Math.abs(r.reserve.closure_residual - (0.8 + 2.0 - 2.8 - 0.1)) < 1e-12);
});

test('the cross-check runs in both directions', () => {
  const ledger = [memberRecord(8, 'specialist_on', member())];
  const censusChild = {seed: 8, arm: 'specialist_on', id: '41:5', birth_tick: 173805,
    death_tick: 196434 + DEATH_TICK_OFFSET, death_cause: 'Starvation', censored_alive: false,
    captures: 13, attempts: {Captured: 13, Missed: 11, OutOfReach: 6}, paid_strike_energy: 2.4};
  const agreed = crossCheck(ledger, [censusChild])[0];
  assert.equal(agreed.agreed, true, agreed.why);
  assert.equal(agreed.census_lifespan_ticks, 22630);

  // A census child with no ledger counterpart is reported, not ignored.
  const extra = crossCheck(ledger, [censusChild,
    {...censusChild, seed: 12, arm: 'facultative_on', id: '35:8'}]);
  assert.equal(extra.length, 2);
  assert.equal(extra[1].child, '12/facultative_on/35:8');
  assert.equal(extra[1].why, 'present in the census reduction, absent from the ledgers');

  // …and a ledger child with no census counterpart likewise.
  assert.equal(crossCheck(ledger, [])[0].why, 'absent from the census reduction');

  // Unaffordable attempts never reach the charge site and are not a disagreement.
  const unaffordable = crossCheck(ledger, [{...censusChild,
    attempts: {Captured: 13, Missed: 11, OutOfReach: 6, Unaffordable: 12}}])[0];
  assert.equal(unaffordable.agreed, true, unaffordable.why);
  assert.equal(unaffordable.census_unaffordable_attempts, 12);
  assert.equal(unaffordable.census_payable_attempts, 30);

  for (const [patch, why] of [
    [{death_tick: 196434}, 'death boundary'], [{death_cause: 'Age'}, 'death_cause'],
    [{birth_tick: 173800}, 'birth_tick'], [{attempts: {Captured: 1}}, 'payable attempts'],
    [{paid_strike_energy: 1}, 'paid strike energy'], [{censored_alive: true}, 'censored_alive'],
    [{birth_tick: 173805, death_tick: 199999}, 'lifespan'],
  ]) {
    const row = crossCheck(ledger, [{...censusChild, ...patch}])[0];
    assert.equal(row.agreed, false, why);
    assert(row.why.includes(why), `${row.why} should mention ${why}`);
  }
});

test('the fixed-intake ceiling is labelled bookkeeping, not a causal partition', () => {
  const rich = memberRecord(8, 'specialist_on', member());                   // 0.8 + 2.0 > 1.2
  const poor = memberRecord(1, 'specialist_on', member({
    digestion: {ticks: 0, to_reserve: 0, energy_gain: 0, material: 0, to_detritus: 0, heat: 0}}));
  const t = cohortTotals([rich, poor]);
  assert.equal(t.fixed_intake_ceiling.children_whose_escrow_plus_recorded_intake_exceeds_the_gate, 1);
  assert.equal(t.fixed_intake_ceiling.children_at_or_below_the_gate, 1);
  assert.equal(t.fixed_intake_ceiling.not_a_causal_partition, true);
  assert(t.fixed_intake_ceiling.condition.includes('recorded intake'));
});

test('cohort totals count outcomes without dropping the quiet cases', () => {
  const a = memberRecord(8, 'specialist_on', member());
  const b = memberRecord(2, 'facultative_on', member({end_tick: null, end_cause: null, death_stocks: null}));
  const c = memberRecord(1, 'specialist_on', member({
    digestion: {ticks: 0, to_reserve: 0, energy_gain: 0, material: 0, to_detritus: 0, heat: 0}}));
  const t = cohortTotals([a, b, c]);
  assert.equal(t.children, 3);
  assert.equal(t.censored_alive, 1);
  assert.deepEqual(t.ended_by_cause, {Starvation: 2, censored_alive: 1});
  assert.equal(t.children_with_zero_reserve_intake, 1);
  assert.equal(t.children_that_entered_growth, 0);
  assert.equal(t.children_ever_adult, 0);
  assert.equal(t.total_gate_observation_ticks, 3 * 22630);
  assert.equal(t.total_reconciliation_checks, 3 * 22631);
});

test('the completion verdict fails on coverage, cross-check or boundary problems alike', () => {
  const sound = {complete: true, invalid_arms: [], boundary_identity_failures: [],
    cross_check_vs_census_reduction: {ran: true, disagreements: [], children_compared: EXPECTED_CHILDREN}};
  assert.deepEqual(failureSummary(sound), []);
  assert.equal(failureSummary({...sound, complete: false}).length, 1);
  assert.equal(failureSummary({...sound, invalid_arms: [{seed: 1}]}).length, 1);
  assert.equal(failureSummary({...sound, boundary_identity_failures: ['8/specialist_on/41:5']}).length, 1);
  assert.equal(failureSummary({...sound,
    cross_check_vs_census_reduction: {...sound.cross_check_vs_census_reduction, ran: false}}).length, 1);
  assert.equal(failureSummary({...sound,
    cross_check_vs_census_reduction: {...sound.cross_check_vs_census_reduction,
      disagreements: [{child: 'x'}]}}).length, 1);
  // A short cross-check is a failure even with nothing disagreeing.
  assert.equal(failureSummary({...sound,
    cross_check_vs_census_reduction: {...sound.cross_check_vs_census_reduction,
      children_compared: 17}}).length, 1);
});

test('identities are arm-scoped and generation-bearing', () => {
  assert.equal(idKey({slot: 41, generation: 5}), '41:5');
  // Seed 1's two hunting arms both contain a child `17:3`.
  assert.notEqual(childKey(1, 'specialist_on', '17:3'), childKey(1, 'facultative_on', '17:3'));
  assert.equal(childKey(1, 'specialist_on', {slot: 17, generation: 3}), '1/specialist_on/17:3');
});
