// Focused checks for the read-only developmental flow reduction.
//
// The reducer's value is what it REFUSES, so most of this exercises defects: a dropped form
// slot, a pooled reconciliation that hides a violation, a birth with no payment, a slot reuse
// recorded on one side only, a prefix run presented as two-hour coverage, and a u64 hash
// carried as a JS number. A test that only feeds it a good artifact would prove nothing.
import { strict as assert } from 'node:assert';
import { test } from 'node:test';

import {
  ARTIFACT_SCHEMA,
  BASE_REVISION,
  BUILD_LABEL,
  FORM_SLOTS,
  GATE_KEYS,
  HORIZON_TICKS,
  KIND,
  ORDINARY_FORMS,
  RESIDUAL_TOLERANCE,
  SNAPSHOT_SCHEMA,
  parseArgs,
  parseArtifactText,
  reduceArtifact,
} from './fauna-development-flow.mjs';

const HEX = (n) => String(n).padStart(64, '0');
const clone = (v) => JSON.parse(JSON.stringify(v));

function stocks(structure = 1, reserve = 0.5, energy = 0.5) {
  return { structure, reserve, energy };
}

function member(over = {}) {
  const id = over.id ?? { slot: 0, generation: 1 };
  const base = {
    id,
    form: 3,
    form_name: 'skimmer',
    origin: 'founder',
    parent: null,
    root: id,
    lineage_depth: 0,
    born_tick: 0,
    first_observed_tick: 0,
    last_observed_tick: HORIZON_TICKS - 1,
    reserve_max: 0.9,
    energy_max: 1.8,
    opening_stocks: stocks(),
    closing_stocks: stocks(),
    structure: {
      at_birth: 0.9,
      adult_target: 0.9,
      final: 0.9,
      adult_recruitment_tick: 0,
      ticks_juvenile: 0,
      ticks_adult: 10,
    },
    upkeep: {
      demand_maintenance: 2,
      demand_movement: 1,
      demand_sensing: 1,
      demand_total: 4,
      paid_total: 4,
      shortfall_total: 0,
      ticks: 10,
      ticks_underpaid: 0,
    },
    oxidation: { ticks_fired: 1, ticks_threshold_open: 1, ticks_blocked_by_empty_reserve: 0, reserve_burned: 0.1, energy_gained: 0.1, heat: 0 },
    growth_gate: {
      observations: 10,
      structure_side_open: 4,
      reserve_side_open: 3,
      both_open: 2,
      entered: 1,
      threshold: 0.27,
      closest_approach_reserve: 0.2,
      closest_deficit: 0.07,
      mean_pre_growth_reserve: 0.1,
      ticks_reserve_zero: 3,
      structure_gained: 0.1,
      reserve_spent: 0.1,
      energy_spent: 0.01,
      bound_by_rate: 1,
      bound_by_headroom: 0,
      bound_by_reserve: 0,
      bound_by_energy: 0,
    },
    reproduction: {
      bud_decisions: 0,
      blocked_by_existing_escrow: 0,
      blocked_by_cap: 0,
      blocked_by_reserve: 0,
      blocked_by_energy: 0,
      funded: 0,
      births_delivered: 0,
      refunds: 0,
      miscarriages: 0,
      reserve_debit: 0,
      energy_debit: 0,
      build_heat: 0,
    },
    intake: {
      ticks_any_actual_intake: 2,
      request_ticks: 3,
      frugivory: { request_ticks: 0, requested_amount: 0, actual_ticks: 0, actual_amount: 0, to_reserve: 0, energy: 0, heat: 0 },
      grazing: { request_ticks: 3, requested_amount: 0.3, actual_ticks: 2, actual_amount: 0.25, to_reserve: 0.2, energy: 0.02, heat: 0 },
      scavenging: { request_ticks: 0, requested_amount: 0, actual_ticks: 0, actual_amount: 0, to_reserve: 0, energy: 0, heat: 0 },
      contested_ticks: 0,
      headroom_limited_ticks: 0,
    },
    local_cell: {
      sampled_ticks: 10,
      water_sum: 1,
      water_max: 0.4,
      ticks_water_positive: 10,
      producer_sum: 2,
      fruit_sum: 0,
      edible_detritus_sum: 1,
      ticks_producer_zero: 0,
      ticks_fruit_zero: 10,
      ticks_edible_detritus_zero: 0,
      access_class_ticks: { none: 0, producer_only: 0, detritus_only: 0, both: 10 },
    },
    birth_payment: null,
    death: null,
    reconciliation: { checks: 10, violations: 0, worst_residual: stocks(0, 0, 0), first_violation_tick: null },
  };
  return { ...base, ...over };
}

function descendant(slot, generation, parent, over = {}) {
  const id = { slot, generation };
  return member({
    id,
    origin: 'descendant',
    parent,
    root: parent,
    lineage_depth: 1,
    born_tick: 1000,
    first_observed_tick: 1000,
    // Recruitment cannot precede the birth boundary; the founder default of 0 would.
    structure: { ...member().structure, at_birth: 0.36, adult_recruitment_tick: 1200 },
    birth_payment: {
      funded_tick: 999,
      escrow: { structure: 0.36, reserve: 0.27, energy: 0.36 },
      parent_reserve_debit: 0.63,
      parent_energy_debit: 0.4,
      build_heat: 0.04,
      refunded: false,
      refund_tick: null,
      slot_freed_same_boundary_by: null,
    },
    ...over,
  });
}

function deathOf(decision_tick, over = {}) {
  return {
    decision_tick,
    event_tick: decision_tick + 1,
    cause: 'starvation',
    age_ticks: 100,
    stocks_at_death: stocks(0.9, 0, 0),
    escrow_at_death: null,
    to_detritus: { material: 0.9, energy_kept: 0, heat: 0 },
    escrow_to_detritus: null,
    slot_reused_same_boundary_by: null,
    last_stepped_tick: decision_tick,
    ticks_between_last_step_and_event: 1,
    ...over,
  };
}

function formRow(form, members) {
  const mine = members.filter((m) => m.form === form);
  return {
    form,
    name: ORDINARY_FORMS[form] ?? `form-${form}`,
    members_ever: mine.length,
    founders: mine.filter((m) => m.origin === 'founder').length,
    descendants: mine.filter((m) => m.origin === 'descendant').length,
    reached_adult_target: mine.filter((m) => m.structure.adult_recruitment_tick !== null).length,
    alive_at_horizon: mine.filter((m) => !m.death).length,
    deaths_by_cause: {},
  };
}

/// A minimal artifact that the reducer must accept. Every defect test mutates a copy of this,
/// so a test that fails to break something is visible as a missing refusal.
function goodArtifact(members = [member(), descendant(1, 1, { slot: 0, generation: 1 })]) {
  const alive = members.filter((m) => !m.death).length;
  const byForm = new Array(FORM_SLOTS).fill(0);
  for (const m of members) if (!m.death) byForm[m.form]++;
  return {
    kind: KIND,
    schema: ARTIFACT_SCHEMA,
    seed: 1,
    base_revision: BASE_REVISION,
    build_label_supplied: BUILD_LABEL,
    diagnostic_binary_sha256: HEX(1),
    horizon_ticks: HORIZON_TICKS,
    dt: 0.05,
    input: { schema_read: SNAPSHOT_SCHEMA, sha256: HEX(2), state_hash: '17068701073499880296', payload_bytes: 73392, crc32: 1 },
    expected_input: { schema: SNAPSHOT_SCHEMA, sha256: HEX(2), state_hash: '17068701073499880296', payload_bytes: 73392, crc32: 1 },
    closing: { schema_written: SNAPSHOT_SCHEMA, sha256: HEX(3), state_hash: '8611632631396418712', payload_bytes: 107878, crc32: 2 },
    expected_closing: { schema: SNAPSHOT_SCHEMA, sha256: HEX(3), state_hash: '8611632631396418712', payload_bytes: 107878, crc32: 2 },
    gates: {
      gate_1_input_identity: { passed: true, checks: [{ field: 'sha256', computed: HEX(2), expected: HEX(2), equal: true }] },
      gate_2_closing_ecology_identity: {
        passed: true,
        asserted: true,
        status: 'asserted',
        schema_read: SNAPSHOT_SCHEMA,
        schema_written: SNAPSHOT_SCHEMA,
        checks: [{ field: 'sha256', computed: HEX(3), expected: HEX(3), equal: true }],
      },
      gate_3_observer_neutrality: { passed: true, checks: [{ field: 'ticks_compared', computed: HORIZON_TICKS, expected: HORIZON_TICKS, equal: true }] },
      gate_4_flow_stock_reconciliation: {
        passed: true,
        checks: [
          { field: 'tolerance_absolute', computed: RESIDUAL_TOLERANCE, expected: RESIDUAL_TOLERANCE, equal: true },
          { field: 'violations', computed: 0, expected: 0, equal: true },
          { field: 'unregistered_records', computed: 0, expected: 0, equal: true },
          { field: 'worst_residual_per_stock', computed: { structure: 0, reserve: 1e-16, energy: 4e-16 }, expected: { each: '<= 1e-12' }, equal: true },
        ],
      },
      census_cross_check: {
        passed: true,
        label: 'census cross-check',
        samples_compared: 1440,
        first_disagreeing_sample: null,
        closing: {
          tick: HORIZON_TICKS,
          reconstructed_population: alive,
          retained_population: alive,
          reconstructed_by_form: byForm,
          retained_by_form: byForm,
        },
      },
    },
    forms: Array.from({ length: FORM_SLOTS }, (_, f) => formRow(f, members)),
    world: { opening: {}, closing: {}, residuals: {}, totals: { reconciliation_checks: 20, births: 1, deaths: 0, life_events: 1, members_ever_recorded: members.length } },
    members,
    notes: [
      'Own-cell availability is potential access and is not intake.',
      'An access class counts occupancy of a cell holding stock; a bare encounter identifies nothing.',
      'Upkeep demand has three terms; the payment is one amount and is not split across them.',
      'Survivors at the horizon are censored, not successes.',
      'The build label is supplied to the encoder, not derived from the diagnostic binary.',
      'Forms 4-7 are zero-exposure: a null ratio, not a successful outcome.',
    ],
  };
}

const refuses = (mutate, pattern) => {
  const a = clone(goodArtifact());
  mutate(a);
  assert.throws(() => reduceArtifact(a), pattern);
};

test('a complete artifact reduces and reports its own provenance', () => {
  const r = reduceArtifact(goodArtifact(), { sha256: HEX(9), name: 'seed-1.json' });
  assert.equal(r.seed, 1);
  assert.equal(r.gates_passed, GATE_KEYS.length);
  assert.equal(r.founders, 1);
  assert.equal(r.descendants, 1);
  assert.equal(r.censored, 2);
  assert.equal(r.base_revision, BASE_REVISION);
  assert.equal(r.snapshot_schema, SNAPSHOT_SCHEMA);
});

test('u64 hashes survive parsing as strings even when an artifact wrote them as numbers', () => {
  const text = '{"state_hash": 17068701073499880296, "ecology_hash": 9224994461224389297}';
  const parsed = parseArtifactText(text);
  assert.equal(parsed.state_hash, '17068701073499880296');
  assert.equal(parsed.ecology_hash, '9224994461224389297');
});

test('a prefix run is refused rather than counted as two-hour coverage', () => {
  refuses((a) => {
    a.gates.gate_2_closing_ecology_identity.asserted = false;
    a.gates.gate_2_closing_ecology_identity.status = 'not asserted';
  }, /not asserted/);
  refuses((a) => {
    a.horizon_ticks = 20000;
  }, /not the retained two-hour opening tick/);
});

test('a failing or missing gate is never reduced as coverage', () => {
  for (const key of GATE_KEYS) refuses((a) => { a.gates[key].passed = false; }, /did not pass/);
  refuses((a) => { delete a.gates.census_cross_check; }, /expected exactly/);
  refuses((a) => { a.gates.extra_gate = { passed: true }; }, /expected exactly/);
});

test('a gate that claims to pass while a check compared unequal is refused', () => {
  refuses((a) => { a.gates.gate_1_input_identity.checks[0].equal = false; }, /compared unequal/);
});

test('reconciliation must be per member and within the stated tolerance', () => {
  refuses((a) => { a.gates.gate_4_flow_stock_reconciliation.checks[0].computed = 1e-9; }, /tolerance/);
  refuses((a) => { a.gates.gate_4_flow_stock_reconciliation.checks[1].computed = 3; }, /violations is 3/);
  refuses((a) => { a.gates.gate_4_flow_stock_reconciliation.checks[2].computed = 1; }, /unregistered_records is 1/);
  refuses((a) => { a.gates.gate_4_flow_stock_reconciliation.checks[3].computed.reserve = 1e-6; }, /worst reserve residual/);
  // A pooled zero cannot hide a per-member violation.
  refuses((a) => { a.members[0].reconciliation.violations = 1; }, /reconciliation violations/);
  refuses((a) => { a.members[0].reconciliation.checks = 0; }, /never reconciled/);
});

test('every form slot is kept, including the zero-exposure ones', () => {
  refuses((a) => { a.forms.pop(); }, /all 8 slots/);
  refuses((a) => { a.forms[3].name = 'lanternjaw'; }, /named lanternjaw/);
  // Internally consistent, so the zero-exposure check is what must refuse it.
  refuses((a) => {
    a.forms[5].members_ever = 2;
    a.forms[5].founders = 2;
  }, /pre-hunter baseline carries only forms/);
  refuses((a) => { a.forms[3].founders = 9; }, /!= members_ever/);
});

test('a descendant without a paid birth is refused', () => {
  refuses((a) => { a.members[1].birth_payment = null; }, /no birth payment/);
  refuses((a) => { a.members[1].birth_payment.parent_reserve_debit = 99; }, /parent reserve debit/);
  refuses((a) => { a.members[1].birth_payment.funded_tick = 5000; }, /funded at 5000, after it was born/);
  refuses((a) => { a.members[1].parent = { slot: 77, generation: 3 }; }, /absent parent/);
});

test('lineage must root in a founder and a founder must carry no parent', () => {
  refuses((a) => { a.members[1].root = { slot: 1, generation: 1 }; }, /is not a founder/);
  refuses((a) => { a.members[0].parent = { slot: 1, generation: 1 }; }, /founder .* carries a parent/);
  refuses((a) => { a.members[1].lineage_depth = 0; }, /lineage depth 0/);
});

test('a generation-bearing id is required and duplicates are refused', () => {
  refuses((a) => { delete a.members[0].id.generation; }, /generation-bearing id/);
  refuses((a) => { a.members[1].id = clone(a.members[0].id); }, /duplicate member id/);
});

test('a death boundary is one tick after its decision and carries an explicit interval', () => {
  const dead = member({ id: { slot: 4, generation: 1 }, death: deathOf(500), last_observed_tick: 500 });
  const a = goodArtifact([member(), dead]);
  a.gates.census_cross_check.closing.reconstructed_population = 1;
  a.gates.census_cross_check.closing.retained_population = 1;
  reduceArtifact(a);

  const bad = clone(a);
  bad.members[1].death.event_tick = 500;
  assert.throws(() => reduceArtifact(bad), /not one tick after the decision/);

  const badInterval = clone(a);
  badInterval.members[1].death.ticks_between_last_step_and_event = 7;
  assert.throws(() => reduceArtifact(badInterval), /final death interval/);

  const badCause = clone(a);
  badCause.members[1].death.cause = 'predation';
  assert.throws(() => reduceArtifact(badCause), /died of predation/);
});

test('same-boundary slot reuse must be recorded on both sides', () => {
  const dead = member({ id: { slot: 1, generation: 1 }, death: deathOf(800, { slot_reused_same_boundary_by: { slot: 1, generation: 2 } }), last_observed_tick: 800 });
  const heir = descendant(1, 2, { slot: 0, generation: 1 }, { born_tick: 801, first_observed_tick: 801 });
  heir.birth_payment.slot_freed_same_boundary_by = { slot: 1, generation: 1 };
  heir.birth_payment.funded_tick = 800;
  const a = goodArtifact([member(), dead, heir]);
  a.gates.census_cross_check.closing.reconstructed_population = 2;
  a.gates.census_cross_check.closing.retained_population = 2;
  const r = reduceArtifact(a);
  assert.equal(r.reuse_pairs, 1);

  const oneSided = clone(a);
  oneSided.members[2].birth_payment.slot_freed_same_boundary_by = null;
  assert.throws(() => reduceArtifact(oneSided), /recorded on the death but not on the birth/);

  const stale = clone(a);
  stale.members[1].death.slot_reused_same_boundary_by = { slot: 1, generation: 1 };
  assert.throws(() => reduceArtifact(stale), /did not advance the generation/);
});

test('potential and actual intake never merge, and actual cannot exceed the request', () => {
  refuses((a) => { a.members[0].intake.grazing.actual_amount = 9; }, /took 9 of a 0.3 request/);
  refuses((a) => { a.members[0].intake.grazing.actual_ticks = 99; }, /fed on more ticks than it asked/);
  refuses((a) => { delete a.members[0].intake.grazing.requested_amount; }, /potential\/actual pair/);
});

test('upkeep keeps three demand terms and one unsplit payment', () => {
  refuses((a) => { a.members[0].upkeep.demand_sensing = 5; }, /do not sum to the recorded demand/);
  refuses((a) => { a.members[0].upkeep.paid_total = 99; }, /paid more upkeep than was demanded/);
  refuses((a) => { a.members[0].upkeep.shortfall_total = 3; }, /shortfall does not close/);
});

test('the growth gate must be an observation, open or shut', () => {
  refuses((a) => { a.members[0].growth_gate.entered = 9; }, /entered growth more often/);
  refuses((a) => { a.members[0].growth_gate.both_open = 4; }, /both gate sides open more often than either/);
  refuses((a) => { a.members[0].growth_gate.observations = 1; }, /fewer gate observations than openings/);
});

test('the closing census must reconstruct from the members themselves', () => {
  refuses((a) => { a.gates.census_cross_check.closing.reconstructed_by_form[3] = 99; }, /!= retained/);
  refuses((a) => { a.gates.census_cross_check.closing.reconstructed_population = 99; }, /!= retained/);
  refuses((a) => { a.gates.census_cross_check.first_disagreeing_sample = 88700; }, /first disagrees at sample 88700/);
  refuses((a) => { a.gates.census_cross_check.samples_compared = 0; }, /compared no samples/);
  // The censored members and the reconstructed closing population are the same fact.
  refuses((a) => {
    a.gates.census_cross_check.closing.reconstructed_population = 1;
    a.gates.census_cross_check.closing.retained_population = 1;
  }, /censored at the horizon but the closing census reconstructs/);
});

test('the envelope pins the baseline, the schema and the identities', () => {
  refuses((a) => { a.kind = 'something-else'; }, /kind/);
  refuses((a) => { a.schema = 2; }, /artifact schema 2/);
  refuses((a) => { a.base_revision = '512ee52'; }, /base revision 512ee52/);
  refuses((a) => { a.build_label_supplied = '0.1.0+512ee52'; }, /build label/);
  refuses((a) => { a.seed = 13; }, /not a prescribed seed/);
  refuses((a) => { a.closing.schema_written = 12; }, /never crosses a schema/);
  refuses((a) => { a.gates.gate_2_closing_ecology_identity.schema_read = 12; }, /only meaningful within one schema/);
  refuses((a) => { a.closing.sha256 = HEX(7); }, /does not equal the retained closing/);
  refuses((a) => { a.input.sha256 = HEX(7); }, /does not equal the retained tick-zero/);
  refuses((a) => { a.input.state_hash = 17068701073499880296; }, /decimal string, not a JS number/);
});

test('the qualifications must travel with the artifact', () => {
  refuses((a) => { a.notes = []; }, /no qualifications/);
  refuses((a) => { a.notes = a.notes.filter((n) => !/potential/i.test(n)); }, /do not qualify/);
  refuses((a) => { a.notes = a.notes.filter((n) => !/censored/i.test(n)); }, /do not qualify/);
});

test('malformed input is refused, not guessed at', () => {
  assert.throws(() => parseArtifactText('not json'), /not valid JSON/);
  assert.throws(() => parseArtifactText('[1,2,3]'), /must be a JSON object/);
  assert.throws(() => reduceArtifact({}), /kind/);
  refuses((a) => { a.members = []; }, /no members/);
  assert.throws(() => parseArgs([]), /Usage/);
  assert.throws(() => parseArgs(['--seed']), /Usage/);
  assert.deepEqual(parseArgs(['a.json']).length, 1);
});
