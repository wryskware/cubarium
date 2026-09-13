import test from 'node:test';
import assert from 'node:assert/strict';
import {existsSync, readFileSync} from 'node:fs';
import {join} from 'node:path';
import {
  ARMS, CLASSES, WINDOW_TICKS, SCREEN_TICKS,
  verifyArm, reduceBouts, reduceQuietEvents, pair,
} from './reduce-quiet-compare.mjs';

const OPENING = 144000;
const TICKS = 12000;
const CLOSING = OPENING + TICKS;

function rest(overrides = {}) {
  const base = {
    reconciled: true, violations: 0, retained_violations: [],
    newborn_initial: {organism_ticks: 10, bouts: 10, same_face_path_px: 0.5, seam_ticks: 0},
    post_birth_recovery: {organism_ticks: 80, bouts: 2, same_face_path_px: 0.1, seam_ticks: 0},
    satiated: {organism_ticks: 5, bouts: 1, same_face_path_px: 0.2, seam_ticks: 1},
    active_organism_ticks: 905,
    organism_ticks: 1000,
    mode_ticks: {resting: 95, seeking: 500, feeding: 405},
    intake_ticks: 300, held_intake_ticks: 0,
    quiet_records: {admissions: 2, releases: 2, refusals: {unaffordable: 3},
      aborts: {}, completed_held_ticks: 80},
    sampled_transported_px: 12.5, sampled_path_ticks: 60,
  };
  return {...base, ...overrides};
}

function arm(name = 'candidate_nocare', overrides = {}) {
  const limits = {material: 1e-6, energy: 1e-6, water: 1e-6};
  const candidate = name.startsWith('candidate'), fed = name.endsWith('_feed');
  const a = {
    arm: name, technical_complete: true, audit_passed: true,
    planned_ticks: TICKS, closing_tick: CLOSING, elapsed_ticks: TICKS,
    termination: 'planned_horizon',
    gates: {conservation_and_flow_audits: true, records_reconcile_to_the_world: true,
      snapshot_resume_equality: true, legacy_raw_energy: true},
    max_absolute_drift: {material: 1e-12, energy: 1e-12, water: 1e-12},
    corrected_energy_drift: 1e-12, independent_windowed_energy_drift: 1e-12,
    care_boundary_energy_drift: 0,
    pre_intervention_baseline: {material: 100, energy: 100, water: 100, limits},
    quiet_policy: candidate ? 'post_birth_pause_v1' : 'off',
    open_pauses_at_close: 0,
    care: {
      planned: fed, applied_at_tick: fed ? OPENING + 600 : null,
      receipts: fed ? [{elapsed: 600, kind: 'feed', dose_permille: 1000,
        target: {face: 0, u: 32, v: 48}, outcome: 'applied', reason: null}] : [],
      ledgers: {admitted_seq: fed ? 1 : 0, rain_depth_in: 0, clean_material_out: 0},
    },
    rest: candidate ? rest() : rest({
      post_birth_recovery: {organism_ticks: 0, bouts: 0, same_face_path_px: 0, seam_ticks: 0},
      active_organism_ticks: 985,
      mode_ticks: {resting: 15, seeking: 500, feeding: 485},
      quiet_records: {admissions: 0, releases: 0, refusals: {}, aborts: {},
        completed_held_ticks: 0},
    }),
    population_organism_ticks: 1000, births: 10,
    deaths: {starvation: 1, age: 0, collapse: 0, predation: 0},
    closing_population: 100, surviving_opening_cohorts: 90, maximum_descendant_depth: 2,
    first_extinction_tick: null, bouts_written: 13,
    unsupported_measurements: ['sampled path length'],
  };
  return {...a, ...overrides};
}

test('an arm must clear every technical gate separately, not just one summary flag', () => {
  verifyArm(arm(), TICKS, OPENING, 'candidate_nocare');
  // Each gate names itself in the refusal, so a failure says which one.
  for (const [gate, says] of [
    ['conservation_and_flow_audits', /conservation/],
    ['records_reconcile_to_the_world', /reconciliation/],
    ['snapshot_resume_equality', /restart equality/],
  ]) {
    const a = arm(); a.gates[gate] = false;
    assert.throws(() => verifyArm(a, TICKS, OPENING, 'candidate_nocare'), says);
  }
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
  ]) {const a = arm(); mutate(a); assert.throws(() => verifyArm(a, TICKS, OPENING, 'candidate_nocare'));}
});

test('organism-ticks are partitioned and the resting ones are exactly the classified ones', () => {
  for (const mutate of [
    a => a.rest.active_organism_ticks++,
    a => a.rest.satiated.organism_ticks++,
    a => a.rest.mode_ticks.resting++,
    a => a.rest.mode_ticks.seeking++,
    a => a.rest.organism_ticks++,
  ]) {
    const a = arm(); mutate(a);
    assert.throws(() => verifyArm(a, TICKS, OPENING, 'candidate_nocare'), /partition|disagree/);
  }
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
  ]) {
    const a = arm('off_nocare'); mutate(a);
    assert.throws(() => verifyArm(a, TICKS, OPENING, 'off_nocare'));
  }
});

test('the care recipe is exactly one Standard Feed at elapsed 600, or nothing at all', () => {
  verifyArm(arm('candidate_feed'), TICKS, OPENING, 'candidate_feed');
  for (const mutate of [
    a => a.care.receipts.push({...a.care.receipts[0], elapsed: 1200}),
    a => a.care.receipts[0].elapsed = 601,
    a => a.care.receipts[0].dose_permille = 1500,
    a => a.care.receipts[0].kind = 'rain',
    a => a.care.receipts[0].target = {face: 1, u: 32, v: 48},
    a => a.care.ledgers.admitted_seq = 2,
    a => a.care.ledgers.rain_depth_in = 4,
    a => a.care.ledgers.clean_material_out = 1,
  ]) {
    const a = arm('candidate_feed'); mutate(a);
    assert.throws(() => verifyArm(a, TICKS, OPENING, 'candidate_feed'));
  }
  // And a no-care arm that received anything.
  const fed = arm('candidate_nocare');
  fed.care.receipts = [{elapsed: 600, kind: 'feed', dose_permille: 1000,
    target: {face: 0, u: 32, v: 48}}];
  assert.throws(() => verifyArm(fed, TICKS, OPENING, 'candidate_nocare'), /received an input/);
});

// --- bouts --------------------------------------------------------------------------------

const id = (slot, generation = 1) => ({slot, generation});
function bout(overrides = {}) {
  return {id: id(1), class: 'post_birth_recovery', start_tick: OPENING + 101,
    end_tick: OPENING + 140, ticks: WINDOW_TICKS, origin_child: id(2),
    origin_boundary: OPENING + 100, end: 'released', end_detail: null, ...overrides};
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
  bad(r => r[0].start_tick = OPENING);                      // outside the run
  bad(r => r[0].end_tick = CLOSING + 1);
  bad(r => r[0].ticks = 0);
  bad(r => r[0].class = 'napping');                         // unknown class
  bad(r => r[1] = structuredClone(r[0]));                   // two bouts, one birth
});

test('an aborted bout keeps its reason and cannot fill the window', () => {
  const rows = [bout({ticks: 5, end_tick: OPENING + 105, end: 'aborted',
    end_detail: 'unaffordable_remaining'})];
  const a = arm('candidate_nocare', {bouts_written: 1, rest: rest({
    post_birth_recovery: {organism_ticks: 5, bouts: 1, same_face_path_px: 0, seam_ticks: 0},
    newborn_initial: {organism_ticks: 0, bouts: 0, same_face_path_px: 0, seam_ticks: 0},
    satiated: {organism_ticks: 0, bouts: 0, same_face_path_px: 0, seam_ticks: 0},
    quiet_records: {admissions: 1, releases: 0, refusals: {}, aborts: {unaffordable_remaining: 1},
      completed_held_ticks: 5},
  })});
  const out = reduceBouts(rows, a, OPENING, CLOSING);
  assert.deepEqual(out.aborted, {unaffordable_remaining: 1});
  assert.equal(out.released, 0);
  assert.equal(out.screenBouts, 0, 'five ticks is under the one-second screen');
  const noReason = structuredClone(rows); noReason[0].end_detail = null;
  assert.throws(() => reduceBouts(noReason, a, OPENING, CLOSING), /without a reason/);
});

test('a censored bout is retained and counted, not dropped or rounded up', () => {
  const rows = [bout({ticks: 12, end_tick: CLOSING, start_tick: CLOSING - 11,
    origin_boundary: CLOSING - 12, end: 'censored'})];
  const a = arm('candidate_nocare', {bouts_written: 1, rest: rest({
    post_birth_recovery: {organism_ticks: 12, bouts: 1, same_face_path_px: 0, seam_ticks: 0},
    newborn_initial: {organism_ticks: 0, bouts: 0, same_face_path_px: 0, seam_ticks: 0},
    satiated: {organism_ticks: 0, bouts: 0, same_face_path_px: 0, seam_ticks: 0},
    quiet_records: {admissions: 1, releases: 0, refusals: {}, aborts: {}, completed_held_ticks: 0},
  })});
  const out = reduceBouts(rows, a, OPENING, CLOSING);
  assert.equal(out.censored, 1);
  assert.equal(out.released, 0);
  assert.equal(out.byClass.post_birth_recovery.ticks, 12);
});

// --- records ------------------------------------------------------------------------------

test('the record stream must agree with the summary and with the contract', () => {
  const rows = [
    {kind: 'begin', tick: OPENING + 100, parent: id(1), child: id(2), end_tick: OPENING + 140,
      underlying: 'Seeking'},
    {kind: 'end', tick: OPENING + 140, parent: id(1), child: id(2),
      completed_ticks: WINDOW_TICKS, underlying: 'Feeding'},
    {kind: 'begin', tick: OPENING + 200, parent: id(3), child: id(4), end_tick: OPENING + 240,
      underlying: 'Seeking'},
    {kind: 'end', tick: OPENING + 240, parent: id(3), child: id(4),
      completed_ticks: WINDOW_TICKS, underlying: 'Seeking'},
    ...Array.from({length: 3}, () => ({kind: 'refuse', tick: OPENING + 50, parent: id(9),
      child: id(8), reason: 'unaffordable'})),
  ];
  const a = arm();
  const out = reduceQuietEvents(rows, a, OPENING, CLOSING);
  assert.equal(out.begin, 2); assert.equal(out.end, 2); assert.equal(out.refuse, 3);

  const bad = mutate => {
    const r = structuredClone(rows); mutate(r);
    assert.throws(() => reduceQuietEvents(r, a, OPENING, CLOSING));
  };
  bad(r => r[0].end_tick = OPENING + 139);             // not the candidate's window
  bad(r => r[1].completed_ticks = 39);                 // a release that did not complete
  bad(r => r.push(structuredClone(r[0])));             // admitted twice at one boundary
  bad(r => r[0].tick = OPENING);                       // outside the run
  bad(r => r.pop());                                   // refusal counts disagree
  bad(r => r[4].reason = 'mystery');                   // an unlisted reason
  bad(r => r.push({kind: 'sleep', tick: OPENING + 1, parent: id(1), child: id(2)}));
  bad(r => delete r[0].child);                         // an incomplete identity
  bad(r => r.push({kind: 'abort', tick: OPENING + 300, parent: id(5), child: id(6),
    completed_ticks: WINDOW_TICKS, reason: 'parent_gone'}));  // an abort that filled the window
});

// --- pairing ------------------------------------------------------------------------------

test('pairing reports every loss and reaches no verdict', () => {
  const armOf = (name, over) => ({summary: arm(name, over),
    census: [{population_by_form: [3, 2, 1, 0, 0, 0, 0, 0]}]});
  const off = armOf('off_nocare');
  const worse = armOf('candidate_nocare', {births: 6, surviving_opening_cohorts: 80,
    first_extinction_tick: CLOSING - 5});
  worse.census = [{population_by_form: [3, 0, 0, 0, 0, 0, 0, 0]}];
  const p = pair(off, worse);
  assert.equal(p.delta.births, -4);
  assert.equal(p.paired_losses.fewer_births, 4);
  assert.equal(p.paired_losses.lost_opening_cohorts, 10);
  assert.equal(p.paired_losses.lost_forms, 2);
  assert.equal(p.paired_losses.new_extinction, true);
  assert.equal(p.off.rest.post_birth_recovery.organism_ticks, 0);
  assert.equal(p.candidate.rest.post_birth_recovery.organism_ticks, 80);
  // A better candidate reports no losses, and still no verdict of any kind.
  const better = armOf('candidate_nocare', {births: 14, surviving_opening_cohorts: 95});
  const q = pair(off, better);
  assert.deepEqual(q.paired_losses,
    {new_extinction: false, lost_forms: 0, lost_opening_cohorts: 0, fewer_births: 0});
  assert.equal(q.delta.births, 4);
  assert(!('verdict' in q) && !('passed' in q) && !('better' in q));
});

// --- the real smoke artifacts ---------------------------------------------------------------

test('the committed smoke artifacts pass every per-arm check they are eligible for', () => {
  const root = new URL('../captures/quiet-smoke-probe/', import.meta.url).pathname;
  if (!existsSync(join(root, 'summary.json'))) {
    // The smoke output is not committed; the synthetic fixtures above carry the contract.
    return;
  }
  const m = JSON.parse(readFileSync(join(root, 'manifest.json'), 'utf8'));
  const s = JSON.parse(readFileSync(join(root, 'summary.json'), 'utf8'));
  const opening = m.cohort.opening_tick, closing = opening + m.ticks;
  let armsChecked = 0, recoveryBouts = 0;
  for (const seed of s.seeds) {
    for (const [i, name] of ARMS.entries()) {
      const a = seed.arms[i];
      verifyArm(a, m.ticks, opening, name);
      const dir = join(root, `seed-${seed.seed}`, name);
      const lines = p => readFileSync(join(dir, p), 'utf8').trim().split('\n')
        .filter(Boolean).map(JSON.parse);
      const bouts = reduceBouts(lines('bouts.jsonl'), a, opening, closing);
      reduceQuietEvents(lines('quiet-events.jsonl'), a, opening, closing);
      assert.equal(lines('census.jsonl').length, m.ticks / m.sample_every);
      recoveryBouts += bouts.byClass.post_birth_recovery.bouts;
      armsChecked++;
    }
  }
  assert.equal(armsChecked, 48, 'twelve seeds by four arms');
  assert(recoveryBouts > 0, 'the smoke must contain real recovery bouts to be worth checking');
  // Every class is named in the contract, and the smoke exercises the partition.
  assert.deepEqual(CLASSES, ['newborn_initial', 'post_birth_recovery', 'satiated']);
});
