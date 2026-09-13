import test from 'node:test';
import assert from 'node:assert/strict';
import {existsSync, readFileSync} from 'node:fs';
import {join} from 'node:path';
import {
  ARMS, CLASSES, WINDOW_TICKS, SCREEN_TICKS, NO_CARE_LEDGER, HORIZON_TICKS,
  verifyArm, reduceBouts, reduceQuietEvents, reduceLife, reduceCensus, limitsFrom,
  parseSnapshotHeader, crc32, pair, inspect, verifyClosingSnapshot, loadRun,
  verifySeedOpening, verifyArmOpening, readOpeningIdentities,
} from './reduce-quiet-compare.mjs';

const OPENING = 144000;
const TICKS = 12000;
const CLOSING = OPENING + TICKS;
const id = (slot, generation = 1) => ({slot, generation});

function rest(overrides = {}) {
  const base = {
    reconciled: true, violations: 0, retained_violations: [],
    newborn_initial: {organism_ticks: 10, bouts: 10, transported_path_px: 0.5, seam_ticks: 0},
    post_birth_recovery: {organism_ticks: 80, bouts: 2, transported_path_px: 0.1, seam_ticks: 0},
    satiated: {organism_ticks: 5, bouts: 1, transported_path_px: 0.2, seam_ticks: 1},
    active_organism_ticks: 905,
    organism_ticks: 1000,
    mode_ticks: {resting: 95, seeking: 500, feeding: 405},
    intake_ticks: 300, held_intake_ticks: 0,
    transported_path_px: 120, seam_ticks: 12,
    quiet_records: {admissions: 2, releases: 2, refusals: {unaffordable: 3}, aborts: {},
      completed_held_ticks: 80, unmatched_closes: 0, open_admissions_at_close: 0,
      deaths_on_final_held_interval: 0, held_intervals_ended_by_death: 0},
  };
  return {...base, ...overrides};
}

const emptyClass = () => ({organism_ticks: 0, bouts: 0, transported_path_px: 0, seam_ticks: 0});

function arm(name = 'candidate_nocare', overrides = {}) {
  const candidate = name.startsWith('candidate'), fed = name.endsWith('_feed');
  const inventories = {material: 100, energy: 100, water: 100};
  const applied = {cells: 5, ends_tick: null, energy_in: 6, energy_out: 0, material_in: 3,
    material_out: 0, water_depth: 0};
  const a = {
    arm: name, technical_complete: true, audit_passed: true,
    planned_ticks: TICKS, closing_tick: CLOSING, elapsed_ticks: TICKS,
    horizon: 'ten-minute',
    termination: 'planned_horizon',
    gates: {conservation_and_flow_audits: true, records_reconcile_to_the_world: true,
      restart_proofs_complete_and_equal: true, legacy_raw_energy: true},
    restart_proofs: [candidate
      ? {trigger: 'mid_pause', start_tick: OPENING + 100, finish_tick: OPENING + 160,
        planned_window_ticks: 60, compared_ticks: 60, complete: true,
        open_pauses_carried_into_the_shadow: 1, failure: null}
      : {trigger: 'fixed_boundary', start_tick: OPENING + 400, finish_tick: OPENING + 460,
        planned_window_ticks: 60, compared_ticks: 60, complete: true,
        open_pauses_carried_into_the_shadow: 0, failure: null}],
    a_pause_was_open_with_room_for_a_full_window: candidate,
    max_absolute_drift: {material: 1e-12, energy: 1e-12, water: 1e-12},
    corrected_energy_drift: 1e-12, independent_windowed_energy_drift: 1e-12,
    care_boundary_energy_drift: 0,
    pre_intervention_baseline: {...inventories, limits: limitsFrom(inventories)},
    quiet_policy: candidate ? 'post_birth_pause_v1' : 'off',
    open_pauses_at_close: 0,
    care: {
      planned: fed, applied_at_tick: fed ? OPENING + 600 : null,
      receipts: fed ? [{elapsed: 600, tick: OPENING + 600, kind: 'feed', dose_permille: 1000,
        target: {face: 0, u: 32, v: 48}, outcome: 'applied', reason: null,
        receipt: {seq: 1, tick: OPENING + 600, outcome: {Applied: applied}}}] : [],
      ledgers: fed
        ? {admitted_seq: 1, allowance_used: 3, clean_energy_out: 0, clean_material_out: 0,
          feed_energy_in: 6, feed_material_in: 3, rain_depth_in: 0, showers: []}
        : {...NO_CARE_LEDGER},
    },
    rest: candidate ? rest() : rest({
      post_birth_recovery: emptyClass(),
      active_organism_ticks: 985,
      mode_ticks: {resting: 15, seeking: 500, feeding: 485},
      quiet_records: {admissions: 0, releases: 0, refusals: {}, aborts: {},
        completed_held_ticks: 0, unmatched_closes: 0, open_admissions_at_close: 0,
        deaths_on_final_held_interval: 0, held_intervals_ended_by_death: 0},
    }),
    population_organism_ticks: 1000, births: candidate ? 5 : 10,
    deaths: {starvation: 1, age: 0, collapse: 0, predation: 0},
    closing_population: 100, surviving_opening_cohorts: 90, maximum_descendant_depth: 2,
    first_extinction_tick: null, bouts_written: 13,
    life_records_written: 6, living_lineages_at_close: 100,
    unsupported_measurements: ['exact funding and oxidation amounts'],
  };
  return {...a, ...overrides};
}

const refuses = (mutate, name = 'candidate_nocare', says = undefined) => {
  const a = arm(name); mutate(a);
  assert.throws(() => verifyArm(a, TICKS, OPENING, name), says);
};

test('an arm must clear every technical gate separately, not just one summary flag', () => {
  verifyArm(arm(), TICKS, OPENING, 'candidate_nocare');
  // Each gate names itself in the refusal, so a failure says which one.
  for (const [gate, says] of [
    ['conservation_and_flow_audits', /conservation/],
    ['records_reconcile_to_the_world', /reconciliation/],
    ['restart_proofs_complete_and_equal', /restart proof/],
    ['legacy_raw_energy', /raw energy/],
  ]) refuses(a => a.gates[gate] = false, 'candidate_nocare', says);
  for (const mutate of [
    a => a.technical_complete = false,
    a => a.audit_passed = false,
    a => a.elapsed_ticks--,
    a => a.closing_tick--,
    a => a.termination = 'step_failure at elapsed 3',
    a => a.max_absolute_drift.water = 1e-6,          // exactly at the limit is a failure
    a => a.corrected_energy_drift = 2e-6,
    a => a.rest.reconciled = false,
    a => a.rest.violations = 1,
    a => a.rest.held_intake_ticks = 1,
    a => a.unsupported_measurements = [],
  ]) refuses(mutate);
});

test('a relabelled horizon is refused: the name and the tick count must be each other', () => {
  assert.equal(HORIZON_TICKS['ten-minute'], TICKS);
  refuses(a => a.horizon = 'two-hour', 'candidate_nocare', /is 144000 ticks, not 12000/);
  refuses(a => a.horizon = 'overnight', 'candidate_nocare', /unknown horizon/);
});

test('audit limits come from the opening inventory, never from the arm\'s own claim', () => {
  // Root's probe: widen the limit, hide a real drift behind it.
  refuses(a => {
    a.pre_intervention_baseline.limits.material = 1e20;
    a.max_absolute_drift.material = 1e10;
  }, 'candidate_nocare', /is not 1e-8 of the opening inventory/);
  // The same drift with an honest limit is still refused, for the real reason.
  refuses(a => a.max_absolute_drift.material = 1e10, 'candidate_nocare', /material drift/);
  // And a baseline that is not the inspected opening's inventory is refused when one is known.
  const a = arm();
  const derived = {inventories: {material: 100, energy: 100, water: 100}};
  verifyArm(a, TICKS, OPENING, 'candidate_nocare', derived);
  const moved = arm();
  moved.pre_intervention_baseline.material = 101;
  moved.pre_intervention_baseline.limits = limitsFrom({material: 101, energy: 100, water: 100});
  assert.throws(() => verifyArm(moved, TICKS, OPENING, 'candidate_nocare', derived),
    /is not the opening snapshot's own inventory/);
});

test('the restart proof must be complete, equal and genuinely mid-pause where one was open', () => {
  for (const [mutate, says] of [
    [a => a.restart_proofs = [], /no restart proof/],
    [a => a.restart_proofs[0].complete = false, /incomplete restart proof/],
    [a => a.restart_proofs[0].failure = 'the persisted pause set differs', /failed/],
    [a => a.restart_proofs[0].compared_ticks = 30, /fewer ticks than it planned/],
    [a => a.restart_proofs[0].finish_tick += 1, /proof range/],
    [a => {
      a.restart_proofs[0].start_tick = OPENING - 1;
      a.restart_proofs[0].finish_tick = OPENING + 59;
    }, /outside the run/],
    [a => a.restart_proofs[0].open_pauses_carried_into_the_shadow = 0, /carried no pause/],
    [a => a.restart_proofs[0].trigger = 'whenever', /unknown proof trigger/],
  ]) refuses(mutate, 'candidate_nocare', says);
  // A fallback that claims a pause it did not interrupt, and a pause that was never proved.
  refuses(a => a.restart_proofs[0].open_pauses_carried_into_the_shadow = 1, 'off_nocare',
    /claimed an open pause/);
  refuses(a => {
    a.restart_proofs = [{trigger: 'fixed_boundary', start_tick: OPENING + 400,
      finish_tick: OPENING + 460, planned_window_ticks: 60, compared_ticks: 60, complete: true,
      open_pauses_carried_into_the_shadow: 0, failure: null}];
  }, 'candidate_nocare', /no mid-pause restart was ever proved/);
});

test('organism-ticks are partitioned and the resting ones are exactly the classified ones', () => {
  for (const mutate of [
    a => a.rest.active_organism_ticks++,
    a => a.rest.satiated.organism_ticks++,
    a => a.rest.mode_ticks.resting++,
    a => a.rest.mode_ticks.seeking++,
    a => a.rest.organism_ticks++,
    a => a.population_organism_ticks++,
  ]) refuses(mutate, 'candidate_nocare', /partition|disagree|not the population/);
});

test('the transported path covers the classified rest inside it, and is never called unsupported', () => {
  refuses(a => a.rest.transported_path_px = 0.2, 'candidate_nocare', /moved further/);
  refuses(a => a.rest.seam_ticks = 0, 'candidate_nocare', /seam ticks exceed/);
  refuses(a => a.rest.seam_ticks = a.rest.organism_ticks + 1, 'candidate_nocare',
    /more seam ticks than organism-ticks/);
  refuses(a => a.unsupported_measurements.push('total transported path length at full resolution'),
    'candidate_nocare', /must not be called unsupported/);
});

test('an Off arm that recovered, refused or held a pause is refused', () => {
  verifyArm(arm('off_nocare'), TICKS, OPENING, 'off_nocare');
  for (const mutate of [
    a => a.rest.quiet_records.admissions = 1,
    a => a.rest.quiet_records.refusals = {unaffordable: 1},
    a => a.rest.post_birth_recovery.organism_ticks = 1,
    a => a.rest.post_birth_recovery.bouts = 1,
    a => a.open_pauses_at_close = 1,
    a => a.quiet_policy = 'post_birth_pause_v1',
  ]) refuses(mutate, 'off_nocare');
});

test('a candidate arm must offer every birth exactly one pause, and close every one it opened', () => {
  refuses(a => a.births = 6, 'candidate_nocare', /quiet offers for/);
  refuses(a => a.rest.quiet_records.unmatched_closes = 1, 'candidate_nocare',
    /matched no admission/);
  refuses(a => a.rest.quiet_records.releases = 1, 'candidate_nocare', /do not add up/);
  refuses(a => a.rest.quiet_records.open_admissions_at_close = 1, 'candidate_nocare',
    /do not add up|persisted pause set/);
  refuses(a => a.rest.quiet_records.aborts = {mystery: 1}, 'candidate_nocare',
    /unknown abort reason/);
  refuses(a => a.rest.quiet_records.deaths_on_final_held_interval = 1, 'candidate_nocare',
    /no bout covers/);
  refuses(a => {
    a.rest.quiet_records.deaths_on_final_held_interval = 1;
    a.rest.quiet_records.held_intervals_ended_by_death = 1;
  }, 'candidate_nocare', /more final-interval deaths than aborts/);
});

test('the care recipe is exactly one applied Standard Feed at the prescribed absolute tick', () => {
  verifyArm(arm('candidate_feed'), TICKS, OPENING, 'candidate_feed');
  for (const [mutate, says] of [
    [a => a.care.receipts.push({...a.care.receipts[0], elapsed: 1200}), /exactly one Feed/],
    [a => a.care.receipts[0].elapsed = 601, undefined],
    [a => a.care.receipts[0].dose_permille = 1500, undefined],
    [a => a.care.receipts[0].kind = 'rain', undefined],
    [a => a.care.receipts[0].target = {face: 1, u: 32, v: 48}, undefined],
    // Root's probe: a wrong absolute application tick behind a correct elapsed one.
    [a => a.care.applied_at_tick = 1, /not the prescribed one/],
    [a => a.care.receipts[0].tick = OPENING + 1, /was applied at tick/],
    [a => a.care.receipts[0].receipt.tick = OPENING + 1, /the receipt's own tick/],
    [a => a.care.receipts[0].receipt.seq = 2, /first and only care command/],
    // Root's probe: a receipt that was not applied at all.
    [a => a.care.receipts[0].outcome = 'rejected', /not applied/],
    [a => a.care.receipts[0].receipt.outcome = {Rejected: {reason: 'forensic_fixture'}},
      /no applied quantities/],
    [a => a.care.receipts[0].receipt.outcome.Applied.material_in = 0, /booked no material_in/],
    [a => a.care.receipts[0].receipt.outcome.Applied.material_out = 1, /took material_out out/],
    [a => a.care.ledgers.admitted_seq = 2, /second care command/],
    [a => a.care.ledgers.rain_depth_in = 4, /rain is not in this recipe/],
    [a => a.care.ledgers.clean_material_out = 1, /cleanup is not in this recipe/],
    [a => a.care.ledgers.showers = [{}], /shower is not in this recipe/],
    // An unreceipted total, in the arm that did receive one.
    [a => a.care.ledgers.feed_material_in += 2, /material ledger is not the receipt/],
    [a => a.care.ledgers.feed_energy_in += 4, /energy ledger is not the receipt/],
    [a => a.care.ledgers.allowance_used += 1, /allowance drawn is not the material/],
  ]) refuses(mutate, 'candidate_feed', says);

  // Root's probe: unreceipted Feed totals in an arm that was never fed.
  refuses(a => {
    a.care.ledgers.feed_material_in += 2;
    a.care.ledgers.feed_energy_in += 4;
  }, 'off_nocare', /carries a care ledger/);
  refuses(a => a.care.receipts = [{elapsed: 600, kind: 'feed', dose_permille: 1000}],
    'candidate_nocare', /received an input/);
  refuses(a => a.care.applied_at_tick = OPENING + 600, 'candidate_nocare', /applied something/);
});

// --- bouts --------------------------------------------------------------------------------

function bout(overrides = {}) {
  return {id: id(1), class: 'post_birth_recovery', start_tick: OPENING + 101,
    end_tick: OPENING + 140, ticks: WINDOW_TICKS, origin_child: id(2),
    origin_boundary: OPENING + 100, end: 'released', end_detail: null, next_class: null,
    ...overrides};
}

test('bout files must agree with their own summary, line by line', () => {
  const rows = [
    bout(),
    bout({id: id(3), origin_child: id(4), origin_boundary: OPENING + 200,
      start_tick: OPENING + 201, end_tick: OPENING + 240}),
    ...Array.from({length: 10}, (_, i) => bout({
      id: id(10 + i), class: 'newborn_initial', ticks: 1,
      start_tick: OPENING + 300 + i, end_tick: OPENING + 300 + i,
      origin_child: null, origin_boundary: null, end: 'woke'})),
    bout({id: id(30), class: 'satiated', ticks: 5, start_tick: OPENING + 400,
      end_tick: OPENING + 404, origin_child: null, origin_boundary: null, end: 'woke'}),
  ];
  const a = arm();
  const out = reduceBouts(rows, a, OPENING, CLOSING);
  assert.equal(out.released, 2);
  assert.equal(out.byClass.post_birth_recovery.ticks, 80);
  assert.equal(out.screenBouts, 2, `both bouts are at least ${SCREEN_TICKS} ticks`);
  assert.equal(out.longestRecovery, WINDOW_TICKS);

  // Every way the file can disagree with the summary, or with itself.
  const bad = mutate => {
    const r = structuredClone(rows); mutate(r);
    assert.throws(() => reduceBouts(r, a, OPENING, CLOSING));
  };
  bad(r => r.pop());                                        // count mismatch
  bad(r => r[0].ticks = WINDOW_TICKS + 1);                  // outlived the window
  bad(r => {r[0].ticks = 39; r[0].end_tick--;});            // released but incomplete
  bad(r => r[0].end = 'aborted');                           // aborted for a whole window
  bad(r => r[0].origin_boundary = OPENING + 99);            // does not start at B+1
  bad(r => r[0].origin_child = null);                       // no originating birth
  bad(r => r[0].class = 'satiated');                        // a non-recovery naming an origin
  bad(r => r[0].id = null);                                 // no full identity
  bad(r => r[0].id = {slot: -1, generation: 0});            // a negative identity
  bad(r => r[0].start_tick = OPENING);                      // outside the run
  bad(r => r[0].end_tick = CLOSING + 1);
  bad(r => r[0].ticks = 0);
  bad(r => r[0].class = 'napping');                         // unknown class
  bad(r => r[0].end = 'evaporated');                        // unknown end
  bad(r => r[1] = structuredClone(r[0]));                   // two bouts, one birth
  bad(r => {                                                // one organism, overlapping bouts
    r[1].id = id(1);
    r[1].origin_boundary = OPENING + 129;
    r[1].start_tick = OPENING + 130;
    r[1].end_tick = OPENING + 169;
  });
});

test('a recovery bout that loses its release or abort record is refused', () => {
  // Astra's finding: a release into ordinary rest used to be filed as Reclassified(Satiated),
  // which discarded the terminal the world published. A recovery bout must end by that record.
  const a = arm('candidate_nocare', {bouts_written: 1, rest: rest({
    newborn_initial: emptyClass(), satiated: emptyClass(),
    post_birth_recovery: {organism_ticks: 40, bouts: 1, transported_path_px: 0, seam_ticks: 0},
    organism_ticks: 1000, active_organism_ticks: 960,
    mode_ticks: {resting: 40, seeking: 500, feeding: 460},
    quiet_records: {admissions: 1, releases: 1, refusals: {unaffordable: 4}, aborts: {},
      completed_held_ticks: 40, unmatched_closes: 0, open_admissions_at_close: 0,
      deaths_on_final_held_interval: 0, held_intervals_ended_by_death: 0},
  })});
  // Released, and it went on resting for an ordinary reason: both facts are kept.
  const released = [bout({end: 'released', next_class: 'satiated'})];
  const out = reduceBouts(released, a, OPENING, CLOSING);
  assert.equal(out.released, 1);
  for (const end of ['reclassified', 'woke', 'died']) {
    const rows = [bout({end, end_detail: end === 'reclassified' ? 'satiated' : null,
      next_class: end === 'reclassified' ? 'satiated' : null})];
    assert.throws(() => reduceBouts(rows, a, OPENING, CLOSING),
      /ended as .* rather than by the world's own record/);
  }
});

test('a bout that promises what came next must be followed by exactly that', () => {
  const a = arm('candidate_nocare', {bouts_written: 2, rest: rest({
    newborn_initial: emptyClass(),
    post_birth_recovery: {organism_ticks: 40, bouts: 1, transported_path_px: 0, seam_ticks: 0},
    satiated: {organism_ticks: 3, bouts: 1, transported_path_px: 0, seam_ticks: 0},
    organism_ticks: 1000, active_organism_ticks: 957,
    mode_ticks: {resting: 43, seeking: 500, feeding: 457},
    quiet_records: {admissions: 1, releases: 1, refusals: {unaffordable: 4}, aborts: {},
      completed_held_ticks: 40, unmatched_closes: 0, open_admissions_at_close: 0,
      deaths_on_final_held_interval: 0, held_intervals_ended_by_death: 0},
  })});
  const rows = [
    bout({end: 'released', next_class: 'satiated'}),
    bout({class: 'satiated', ticks: 3, start_tick: OPENING + 141, end_tick: OPENING + 143,
      origin_child: null, origin_boundary: null, end: 'woke'}),
  ];
  reduceBouts(rows, a, OPENING, CLOSING);
  const gap = structuredClone(rows);
  gap[1].start_tick += 1; gap[1].end_tick += 1;
  assert.throws(() => reduceBouts(gap, a, OPENING, CLOSING), /left a gap/);
  const other = structuredClone(rows);
  other[0].next_class = 'newborn_initial';
  assert.throws(() => reduceBouts(other, a, OPENING, CLOSING), /promised newborn_initial/);
});

test('an aborted bout keeps its reason and cannot fill the window', () => {
  const rows = [bout({ticks: 5, end_tick: OPENING + 105, end: 'aborted',
    end_detail: 'unaffordable_remaining'})];
  const a = arm('candidate_nocare', {bouts_written: 1, births: 5, rest: rest({
    post_birth_recovery: {organism_ticks: 5, bouts: 1, transported_path_px: 0, seam_ticks: 0},
    newborn_initial: emptyClass(), satiated: emptyClass(),
    organism_ticks: 1000, active_organism_ticks: 995,
    mode_ticks: {resting: 5, seeking: 500, feeding: 495},
    quiet_records: {admissions: 1, releases: 0, refusals: {unaffordable: 4},
      aborts: {unaffordable_remaining: 1}, completed_held_ticks: 5, unmatched_closes: 0,
      open_admissions_at_close: 0, deaths_on_final_held_interval: 0,
      held_intervals_ended_by_death: 0},
  })});
  const out = reduceBouts(rows, a, OPENING, CLOSING);
  assert.deepEqual(out.aborted, {unaffordable_remaining: 1});
  assert.equal(out.released, 0);
  assert.equal(out.screenBouts, 0, 'five ticks is under the one-second screen');
  const noReason = structuredClone(rows); noReason[0].end_detail = null;
  assert.throws(() => reduceBouts(noReason, a, OPENING, CLOSING), /an abort with reason/);
});

test('a censored bout is retained, counted, and really at the horizon', () => {
  const rows = [bout({ticks: 12, end_tick: CLOSING, start_tick: CLOSING - 11,
    origin_boundary: CLOSING - 12, end: 'censored'})];
  const a = arm('candidate_nocare', {bouts_written: 1, births: 5, rest: rest({
    post_birth_recovery: {organism_ticks: 12, bouts: 1, transported_path_px: 0, seam_ticks: 0},
    newborn_initial: emptyClass(), satiated: emptyClass(),
    organism_ticks: 1000, active_organism_ticks: 988,
    mode_ticks: {resting: 12, seeking: 500, feeding: 488},
    quiet_records: {admissions: 1, releases: 0, refusals: {unaffordable: 4}, aborts: {},
      completed_held_ticks: 0, unmatched_closes: 0, open_admissions_at_close: 1,
      deaths_on_final_held_interval: 0, held_intervals_ended_by_death: 0},
    })}, );
  a.open_pauses_at_close = 1;
  const out = reduceBouts(rows, a, OPENING, CLOSING);
  assert.equal(out.censored, 1);
  assert.equal(out.released, 0);
  assert.equal(out.byClass.post_birth_recovery.ticks, 12);
  // "Censored" is only honest at the horizon itself.
  const early = structuredClone(rows);
  early[0].origin_boundary = CLOSING - 13;
  early[0].start_tick = CLOSING - 12;
  early[0].end_tick = CLOSING - 1;
  assert.throws(() => reduceBouts(early, a, OPENING, CLOSING), /did not reach the horizon/);
});

// --- records ------------------------------------------------------------------------------

const begin = (parent, child, tick) => ({kind: 'begin', tick, parent, child,
  end_tick: tick + WINDOW_TICKS, underlying: 'Seeking'});
const end = (parent, child, tick) => ({kind: 'end', tick: tick + WINDOW_TICKS, parent, child,
  completed_ticks: WINDOW_TICKS, underlying: 'Feeding'});

test('the record stream must agree with the summary and with the contract', () => {
  const rows = [
    begin(id(1), id(2), OPENING + 100),
    end(id(1), id(2), OPENING + 100),
    begin(id(3), id(4), OPENING + 200),
    end(id(3), id(4), OPENING + 200),
    ...Array.from({length: 3}, () => ({kind: 'refuse', tick: OPENING + 50, parent: id(9),
      child: id(8), reason: 'unaffordable'})),
  ];
  const a = arm();
  const out = reduceQuietEvents(rows, a, OPENING, CLOSING);
  assert.equal(out.begin, 2); assert.equal(out.end, 2); assert.equal(out.refuse, 3);
  assert.equal(out.open, 0);

  const bad = (mutate, says) => {
    const r = structuredClone(rows); mutate(r);
    assert.throws(() => reduceQuietEvents(r, a, OPENING, CLOSING), says);
  };
  bad(r => r[0].end_tick = OPENING + 139);             // not the candidate's window
  bad(r => r[1].completed_ticks = 39);                 // a release that did not complete
  bad(r => r[0].tick = OPENING);                       // outside the run
  bad(r => r.pop());                                   // refusal counts disagree
  bad(r => r[4].reason = 'mystery');                   // an unlisted reason
  bad(r => r.push({kind: 'sleep', tick: OPENING + 1, parent: id(1), child: id(2)}));
  bad(r => delete r[0].child);                         // an incomplete identity
  bad(r => r[0].child = id(1));                        // its own child
  bad(r => delete r[0].underlying);                    // no carried ordinary mode
});

test('every close belongs to exactly one admission, by parent, child and boundary', () => {
  const a = arm();
  const bad = (rows, says) =>
    assert.throws(() => reduceQuietEvents(rows, a, OPENING, CLOSING), says);
  // A close with nothing open, a duplicate admission, a swapped child, a swapped generation,
  // and a tick that does not follow the boundary. None of these changes a single total.
  bad([end(id(1), id(2), OPENING + 100)], /with no open admission/);
  bad([begin(id(1), id(2), OPENING + 100), begin(id(1), id(5), OPENING + 120)],
    /admitted again before its pause closed/);
  bad([begin(id(1), id(2), OPENING + 100), end(id(1), id(7), OPENING + 100)], /naming 7\/1/);
  bad([begin(id(1), id(2), OPENING + 100),
    {...end(id(1), id(2), OPENING + 100), child: {slot: 2, generation: 9}}], /naming 2\/9/);
  bad([begin(id(1), id(2), OPENING + 100),
    {...end(id(1), id(2), OPENING + 100), tick: OPENING + 141}], /does not follow its boundary/);
  bad([begin(id(1), id(2), OPENING + 100),
    {...end(id(1), id(2), OPENING + 100), parent: {slot: 1, generation: 9}}],
  /with no open admission/);
});

test('a parent that dies on its fortieth held interval is counted, not smoothed away', () => {
  // Astra's finding: ParentGone at exactly the window is legitimate — the interval really was
  // held and there was no parent left to release. Any other whole-window abort is not.
  const a = arm('candidate_nocare', {births: 3, rest: rest({
    newborn_initial: emptyClass(), satiated: emptyClass(),
    post_birth_recovery: {organism_ticks: 39, bouts: 1, transported_path_px: 0, seam_ticks: 0},
    organism_ticks: 1000, active_organism_ticks: 961,
    mode_ticks: {resting: 39, seeking: 500, feeding: 461},
    quiet_records: {admissions: 1, releases: 0, refusals: {unaffordable: 2},
      aborts: {parent_gone: 1}, completed_held_ticks: 40, unmatched_closes: 0,
      open_admissions_at_close: 0, deaths_on_final_held_interval: 1,
      held_intervals_ended_by_death: 1},
  })});
  const rows = [
    begin(id(1), id(2), OPENING + 100),
    {kind: 'abort', tick: OPENING + 140, parent: id(1), child: id(2), completed_ticks: 40,
      reason: 'parent_gone'},
    ...Array.from({length: 2}, () => ({kind: 'refuse', tick: OPENING + 50, parent: id(9),
      child: id(8), reason: 'unaffordable'})),
  ];
  const out = reduceQuietEvents(rows, a, OPENING, CLOSING);
  assert.equal(out.finalIntervalDeaths, 1);
  // The same abort for any other reason, and one that outlived the window, are both refused.
  const other = structuredClone(rows); other[1].reason = 'unaffordable_remaining';
  assert.throws(() => reduceQuietEvents(other, a, OPENING, CLOSING), /whole window for reason/);
  const over = structuredClone(rows);
  over[1].completed_ticks = 41;
  over[1].tick = OPENING + 141;
  assert.throws(() => reduceQuietEvents(over, a, OPENING, CLOSING), /outlived the window/);
  // And the summary has to say so too: a real death on the last interval is not free.
  const quiet = structuredClone(a);
  quiet.rest.quiet_records.deaths_on_final_held_interval = 0;
  assert.throws(() => reduceQuietEvents(rows, quiet, OPENING, CLOSING),
    /whole-window aborts disagree/);
});

// --- the life stream ------------------------------------------------------------------------

function lifeFixture() {
  const openingIdentities = new Map([
    ['1/1', {form: 0, born: OPENING - 500, depth: 0, cohort: '1/1'}],
    ['2/1', {form: 1, born: OPENING - 400, depth: 0, cohort: '2/1'}],
  ]);
  const rows = [
    {kind: 'birth', tick: OPENING + 10, id: id(3), parent: id(1), form: 0, genome_digest: 7,
      descendant_depth: 1, opening_cohort: id(1), structure: 1, reserve: 1, energy: 1,
      parent_age_ticks: 510, parent_births: 1, origin: 'Born', mutated_loci: 0, mutations: []},
    {kind: 'birth', tick: OPENING + 20, id: id(4), parent: id(3), form: 5, genome_digest: 8,
      descendant_depth: 2, opening_cohort: id(1), structure: 1, reserve: 1, energy: 1,
      parent_age_ticks: 10, parent_births: 1, origin: 'Born', mutated_loci: 1, mutations: ['Hue']},
    {kind: 'death', tick: OPENING + 30, id: id(2), cause: 'Starvation', age_ticks: 430,
      births: 0, genome_digest: 2, form: 1, born_tick: OPENING - 400, opening_cohort: id(2),
      descendant_depth: 0, was_an_opening_organism: true},
  ];
  const summary = arm('candidate_nocare', {births: 2, life_records_written: 3,
    closing_population: 3, living_lineages_at_close: 3, surviving_opening_cohorts: 1,
    maximum_descendant_depth: 2,
    deaths: {starvation: 1, age: 0, collapse: 0, predation: 0}});
  return {rows, summary, openingIdentities};
}

test('the life stream is one population history, by identity rather than by count', () => {
  const {rows, summary, openingIdentities} = lifeFixture();
  const out = reduceLife(rows, summary, OPENING, CLOSING, openingIdentities);
  assert.equal(out.births, 2);
  assert.equal(out.survivors, 3);
  assert.equal(out.survivingCohorts, 1, 'both survivors descend from one opening organism');
  assert.equal(out.openingOrganismDeaths, 1);
  // A form that never existed at the opening still leaves a record.
  assert.deepEqual(out.formsOnlyAfterOpening, [5]);

  const bad = (mutate, says) => {
    const r = structuredClone(rows); mutate(r);
    assert.throws(() => reduceLife(r, summary, OPENING, CLOSING, new Map(openingIdentities)), says);
  };
  bad(r => r[1].id = id(3), /reused the identity/);
  bad(r => r[0].parent = id(77), /not on the books/);
  bad(r => r[1].descendant_depth = 3, /generation was skipped/);
  bad(r => r[1].opening_cohort = id(2), /descends from a different opening organism/);
  bad(r => r[2].id = id(77), /who is not alive/);
  bad(r => r[2].form = 3, /a form the organism did not live as/);
  bad(r => r[2].age_ticks = 1, /not its own lifetime/);
  bad(r => r[2].cause = 'Boredom', /a death by/);
  bad(r => r[2].was_an_opening_organism = false, /mislabelled opening organism/);
  bad(r => r.pop(), /disagree with the reported starvation deaths|is not the one reported/);
  bad(r => r[0].tick = OPENING, /outside the run/);
  bad(r => r.reverse(), /out of order|not on the books/);
  // And the counts still have to match the summary they belong to.
  const inflated = structuredClone(summary);
  inflated.surviving_opening_cohorts = 2;
  assert.throws(() => reduceLife(rows, inflated, OPENING, CLOSING, new Map(openingIdentities)),
    /surviving opening cohorts disagree/);
});

// --- the census -----------------------------------------------------------------------------

function censusFixture() {
  const windows = TICKS / 200;
  const rows = Array.from({length: windows}, (_, i) => ({
    tick: OPENING + (i + 1) * 200, elapsed: (i + 1) * 200,
    population: 6, population_by_form: [3, 2, 1, 0, 0, 0, 0, 0], occupied_cells: 5,
    window_births: i === 0 ? 2 : 0,
    window_deaths: {starvation: i === 1 ? 1 : 0, age: 0, collapse: 0},
    mode: {resting: 1, seeking: 3, feeding: 2},
    escrows: 0, producer: 1, fruit: 1, detritus: 1, nutrient: 1, water: 1,
    organism_material: 1, organism_energy: 1, window_light_in: 1, window_heat_out: 1,
    surviving_opening_cohorts: 1, maximum_descendant_depth: 2, open_pauses: 0,
    state_hash: '123', ecology_hash: '456',
  }));
  const summary = arm('candidate_nocare', {births: 2, closing_population: 6,
    surviving_opening_cohorts: 1, maximum_descendant_depth: 2,
    closing_state_hash: '123', closing_ecology_hash: '456',
    deaths: {starvation: 1, age: 0, collapse: 0, predation: 0}});
  return {rows, summary};
}

test('every census window is present, in order, and its flows are the run\'s own', () => {
  const {rows, summary} = censusFixture();
  const flow = reduceCensus(rows, summary, OPENING, TICKS, 200);
  assert.deepEqual(flow, {births: 2, starvation: 1, age: 0, collapse: 0});

  const bad = (mutate, says) => {
    const r = structuredClone(rows); mutate(r);
    assert.throws(() => reduceCensus(r, summary, OPENING, TICKS, 200), says);
  };
  bad(r => r.pop(), /windows missing/);
  bad(r => r[3].tick += 200, /out of order/);
  bad(r => r[3].elapsed += 1, /elapsed/);
  bad(r => r[3].population_by_form[0]++, /does not sum to the population/);
  bad(r => r[3].population_by_form.pop(), /full form vector/);
  bad(r => r[3].mode.resting++, /do not cover the population/);
  bad(r => r[3].water = -1, /not a finite nonnegative number/);
  bad(r => r[3].producer = null, /not a finite nonnegative number/);
  bad(r => r[0].window_births = 3, /window births do not sum/);
  bad(r => r[1].window_deaths.starvation = 0, /window starvation deaths do not sum/);
  bad(r => r[r.length - 1].state_hash = '999', /closing state hash disagrees/);
  bad(r => {
    const last = r[r.length - 1];
    last.population = 7;
    last.population_by_form[0] = 4;
    last.mode.resting = 2;
  }, /closes on another population/);
  bad(r => r[3].surviving_opening_cohorts = 99, /more cohorts than organisms/);
  // A predation death in a world with no hunters is refused outright.
  const hunted = structuredClone(summary);
  hunted.deaths.predation = 1;
  assert.throws(() => reduceCensus(rows, hunted, OPENING, TICKS, 200), /no hunters/);
});

// --- snapshots -------------------------------------------------------------------------------

test('a snapshot is parsed, length-checked and CRC-checked here, not taken on trust', () => {
  const build = Buffer.from('0.1.0+test', 'utf8');
  const payload = Buffer.from('a plausible looking payload', 'utf8');
  const bytes = Buffer.concat([
    Buffer.from('CUBW', 'latin1'),
    (() => {const b = Buffer.alloc(4); b.writeUInt32LE(13); return b;})(),
    (() => {const b = Buffer.alloc(2); b.writeUInt16LE(build.length); return b;})(),
    build,
    (() => {const b = Buffer.alloc(8); b.writeBigUInt64LE(BigInt(payload.length)); return b;})(),
    (() => {const b = Buffer.alloc(4); b.writeUInt32LE(crc32(payload)); return b;})(),
    payload,
  ]);
  const header = parseSnapshotHeader(bytes);
  assert.equal(header.schema, 13);
  assert.equal(header.build, '0.1.0+test');
  assert.equal(header.payloadLength, payload.length);

  const flipped = Buffer.from(bytes); flipped[flipped.length - 1] ^= 1;
  assert.throws(() => parseSnapshotHeader(flipped), /does not match its own CRC32/);
  const truncated = bytes.subarray(0, bytes.length - 1);
  assert.throws(() => parseSnapshotHeader(truncated), /is not the payload that follows it/);
  const wrongMagic = Buffer.from(bytes); wrongMagic.write('NOPE', 0, 'latin1');
  assert.throws(() => parseSnapshotHeader(wrongMagic), /bad magic/);
});

// --- pairing ------------------------------------------------------------------------------

test('pairing reports every loss and reaches no verdict', () => {
  const armOf = (name, over) => ({summary: arm(name, over),
    census: [{population_by_form: [3, 2, 1, 0, 0, 0, 0, 0]}],
    life: {formsSeen: [0, 1, 2], openingOrganismDeaths: 1}});
  const off = armOf('off_nocare');
  const worse = armOf('candidate_nocare', {births: 5, surviving_opening_cohorts: 80,
    first_extinction_tick: CLOSING - 5});
  worse.census = [{population_by_form: [3, 0, 0, 0, 0, 0, 0, 0]}];
  worse.life = {formsSeen: [0], openingOrganismDeaths: 4};
  const p = pair(off, worse);
  assert.equal(p.delta.births, -5);
  assert.equal(p.paired_losses.fewer_births, 5);
  assert.equal(p.paired_losses.lost_opening_cohorts, 10);
  assert.equal(p.paired_losses.lost_forms, 2);
  assert.equal(p.paired_losses.new_extinction, true);
  assert.equal(p.paired_losses.more_opening_organism_deaths, 3);
  assert.equal(p.off.rest.post_birth_recovery.organism_ticks, 0);
  assert.equal(p.candidate.rest.post_birth_recovery.organism_ticks, 80);
  // A better candidate reports no losses, and still no verdict of any kind.
  const better = armOf('candidate_nocare', {births: 14, surviving_opening_cohorts: 95});
  const q = pair(off, better);
  assert.deepEqual(q.paired_losses, {new_extinction: false, lost_forms: 0,
    lost_opening_cohorts: 0, fewer_births: 0, more_opening_organism_deaths: 0});
  assert.equal(q.delta.births, 4);
  assert(!('verdict' in q) && !('passed' in q) && !('better' in q));
});

// --- the real smoke artifacts ---------------------------------------------------------------

test('the smoke artifacts pass every per-arm check they are eligible for', () => {
  const root = new URL('../captures/quiet-smoke-validated-2026-09-13c/', import.meta.url).pathname;
  if (!existsSync(join(root, 'summary.json'))) {
    // The smoke output is not present; the synthetic fixtures above carry the contract.
    return;
  }
  const m = JSON.parse(readFileSync(join(root, 'manifest.json'), 'utf8'));
  const s = JSON.parse(readFileSync(join(root, 'summary.json'), 'utf8'));
  const opening = m.cohort.opening_tick, closing = opening + m.ticks;
  let armsChecked = 0, recoveryBouts = 0, midPauseProofs = 0, lifeRecords = 0;
  for (const seed of s.seeds) {
    for (const [i, name] of ARMS.entries()) {
      const a = seed.arms[i];
      verifyArm(a, m.ticks, opening, name);
      const dir = join(root, `seed-${seed.seed}`, name);
      const lines = p => readFileSync(join(dir, p), 'utf8').trim().split('\n')
        .filter(Boolean).map(JSON.parse);
      const bouts = reduceBouts(lines('bouts.jsonl'), a, opening, closing);
      const events = reduceQuietEvents(lines('quiet-events.jsonl'), a, opening, closing);
      const identities = readOpeningIdentities(
        readFileSync(join(dir, 'opening-organisms.jsonl'), 'utf8'),
        m.cohort.openings.find(o => o.seed === seed.seed).population, name, opening);
      const life = reduceLife(lines('life.jsonl'), a, opening, closing, identities);
      reduceCensus(lines('census.jsonl'), a, opening, m.ticks, m.sample_every);
      // The cross-file links the totals cannot make: an offer had a birth behind it, and a
      // recovery bout had an admission behind it.
      for (const offer of events.offers)
        assert(life.birthsByParent.has(offer.split('>')[0]), `${name}: an offer with no birth`);
      for (const origin of bouts.recoveryOrigins)
        assert(events.admitted.has(origin), `${name}: a recovery bout with no admission`);
      assert.equal(bouts.byClass.newborn_initial.bouts, life.births,
        `${name}: newborn bouts are not the births`);
      const closingBytes = readFileSync(join(dir, 'closing.cubw'));
      const header = parseSnapshotHeader(closingBytes);
      assert.equal(header.build, m.build, `${name}: the closing snapshot's build`);
      recoveryBouts += bouts.byClass.post_birth_recovery.bouts;
      midPauseProofs += a.restart_proofs.filter(p => p.trigger === 'mid_pause').length;
      lifeRecords += life.births;
      armsChecked++;
    }
  }
  assert.equal(armsChecked, 48, 'twelve seeds by four arms');
  assert(recoveryBouts > 0, 'the smoke must contain real recovery bouts to be worth checking');
  assert(midPauseProofs > 0, 'at least one arm must have proved a genuine mid-pause restart');
  assert(lifeRecords > 0, 'the life stream must carry real births');
  // Every class is named in the contract, and the smoke exercises the partition.
  assert.deepEqual(CLASSES, ['newborn_initial', 'post_birth_recovery', 'satiated']);
});

test('an opening is fingerprinted from the cohort source, and every arm is held to it', () => {
  const root = new URL('../captures/quiet-smoke-validated-2026-09-13c/', import.meta.url).pathname;
  if (!existsSync(join(root, 'summary.json'))) return;
  const m = JSON.parse(readFileSync(join(root, 'manifest.json'), 'utf8'));
  const executable = join(root, 'quiet_compare.frozen');
  const cohortRoot = new URL(`../${m.cohort_source}/`, import.meta.url).pathname;
  const opening = m.cohort.opening_tick;
  const row = m.cohort.openings.find(o => o.seed === 1);
  const path = join(cohortRoot, 'seed-1', 'world-144000.cubw');
  const bytes = readFileSync(path);
  const inspected = inspect(executable, path);
  const derived = verifySeedOpening(bytes, inspected, row, 1, m.build, opening);
  assert.equal(derived.population, row.population);
  assert.deepEqual(derived.limits, limitsFrom(derived.inventories));

  // The manifest's description of the opening is checked against the opening, not trusted.
  for (const [mutate, says] of [
    [r => r.sha256 = '0'.repeat(64), /the opening changed/],
    [r => r.population += 1, /opening population/],
    [r => r.ecology_hash = '1', /opening ecology hash/],
  ]) {
    const damaged = structuredClone(row); mutate(damaged);
    assert.throws(() => verifySeedOpening(bytes, inspected, damaged, 1, m.build, opening), says);
  }
  assert.throws(() => verifySeedOpening(bytes, inspected, row, 2, m.build, opening),
    /carries another seed/);
  assert.throws(() => verifySeedOpening(bytes, inspected, row, 1, 'someone-elses-build', opening),
    /not the build that wrote this run/);

  // And each arm's own opening.json against that fingerprint.
  for (const name of ARMS) {
    const openingJson = JSON.parse(
      readFileSync(join(root, 'seed-1', name, 'opening.json'), 'utf8'));
    verifyArmOpening(openingJson, derived, name, 1, opening);
    for (const [mutate, says] of [
      [o => o.state_hash_before_choice = '1', /did not start from this seed's opening/],
      [o => o.config_sha256 = 'x', /config identity/],
      [o => o.ecology_hash = '1', /ecology hash/],
      [o => o.quiet.pauses = [{}], /an opening pause/],
      [o => o.state_hash_after_choice = o.state_hash_before_choice === '1' ? '2' : '1',
        name.startsWith('candidate') ? undefined : /an Off arm changed the opening/],
      [o => o.care = o.care ? null : {kind: 'feed'}, /the recorded recipe/],
    ]) {
      const damaged = structuredClone(openingJson); mutate(damaged);
      if (name.startsWith('candidate') && says === undefined) continue;
      assert.throws(() => verifyArmOpening(damaged, derived, name, 1, opening), says);
    }
  }
  // A census that is not the opening population, and a duplicate identity in it.
  const text = readFileSync(join(root, 'seed-1', 'off_nocare', 'opening-organisms.jsonl'), 'utf8');
  const identities = readOpeningIdentities(text, row.population, 'off_nocare', opening);
  assert.equal(identities.size, row.population);
  assert.throws(() => readOpeningIdentities(text, row.population + 1, 'off_nocare', opening),
    /not the opening population/);
  const doubled = text.trim().split('\n');
  assert.throws(() => readOpeningIdentities(
    [...doubled, doubled[0]].join('\n'), row.population + 1, 'off_nocare', opening),
  /duplicate opening identity/);
});

test('a closing snapshot is checked as bytes and then decoded for what is inside it', async () => {
  const root = new URL('../captures/quiet-smoke-validated-2026-09-13c/', import.meta.url).pathname;
  if (!existsSync(join(root, 'summary.json'))) return;
  const m = JSON.parse(readFileSync(join(root, 'manifest.json'), 'utf8'));
  const executable = join(root, 'quiet_compare.frozen');
  for (const name of ARMS) {
    const dir = join(root, 'seed-1', name);
    const summary = JSON.parse(readFileSync(join(dir, 'summary.json'), 'utf8'));
    const bytes = readFileSync(join(dir, 'closing.cubw'));
    const inspected = inspect(executable, join(dir, 'closing.cubw'));
    verifyClosingSnapshot(bytes, inspected, summary, m, name);
    // The semantics a header parse cannot see.
    assert.equal(inspected.quiet.policy, name.startsWith('candidate') ? 'post_birth_pause_v1' : 'off');
    assert.equal(inspected.tick, summary.closing_tick);
    assert.deepEqual(inspected.care, summary.care.ledgers);
    // A snapshot whose decoded state hash is not the one claimed is refused.
    const lying = structuredClone(summary);
    lying.closing_state_hash = '1';
    assert.throws(() => verifyClosingSnapshot(bytes, inspected, lying, m, name), /state hash/);
  }
  // And the whole reduction still refuses a smoke, whatever its artifacts look like.
  await assert.rejects(loadRun(root), /not a prescribed horizon/);
});
