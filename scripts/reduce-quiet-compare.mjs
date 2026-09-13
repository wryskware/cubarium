// Read-only reduction of a completed four-arm ordinary-quiet screen.
// Does not run an executable, modify artifacts, or infer ecological acceptance.
//
// The contract this checks is the proposal's own
// (design/7_Research/astra-ordinary-quiet-experiment-proposal-2026-09-13.md): twelve prescribed
// openings, four arms each, one Standard Feed at elapsed 600 in the two fed arms, and a rest
// classification that separates post_birth_recovery from natural satiated rest and from a
// newborn's own first tick.
//
// What it refuses is as important as what it reports. An Off arm that admitted a pause, a
// recovery bout that outlived its window, a released bout that did not complete it, a bout file
// that disagrees with its own summary, a census that skipped a window, a cohort that is not the
// prescribed twelve — each is a refusal, not a footnote. Nothing here decides viability, and
// nothing here turns a passing technical gate into evidence about behaviour.

import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {resolve, join} from 'node:path';
import {pathToFileURL} from 'node:url';

export const ARMS = ['off_nocare', 'candidate_nocare', 'off_feed', 'candidate_feed'];
export const CLASSES = ['newborn_initial', 'post_birth_recovery', 'satiated'];
/// The candidate's whole window, in ticks. A released bout is exactly this long.
export const WINDOW_TICKS = 40;
/// The proposal's behavioural screen: a bout of at least one simulated second.
export const SCREEN_TICKS = 20;
export const SCREEN_SEEDS = 9;

const json = async path => JSON.parse(await readFile(path, 'utf8'));
const jsonl = async path =>
  (await readFile(path, 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse);
const safeCount = n => assert(Number.isSafeInteger(n) && n >= 0, `invalid count ${n}`);
const ratio = (n, d) => (d === 0 ? null : n / d);
const idKey = id => `${id.slot}/${id.generation}`;

/// Every technical gate an arm had to clear, checked here rather than believed from one flag.
export function verifyArm(a, ticks, opening, arm) {
  assert.equal(a.arm, arm, 'arm name mismatch');
  assert.equal(a.technical_complete, true, `${arm}: technically incomplete`);
  assert.equal(a.audit_passed, true, `${arm}: a technical gate failed`);
  assert.equal(a.planned_ticks, ticks);
  assert.equal(a.closing_tick, opening + ticks);
  assert.equal(a.elapsed_ticks, ticks);
  assert.equal(a.termination, 'planned_horizon');
  // Each gate is separately true; `audit_passed` is never the only evidence.
  assert.equal(a.gates.conservation_and_flow_audits, true, `${arm}: conservation`);
  assert.equal(a.gates.records_reconcile_to_the_world, true, `${arm}: reconciliation`);
  assert.equal(a.gates.snapshot_resume_equality, true, `${arm}: restart equality`);
  for (const key of ['material', 'energy', 'water']) {
    const drift = a.max_absolute_drift[key], limit = a.pre_intervention_baseline.limits[key];
    assert(Number.isFinite(limit) && limit > 0, `${arm}: invalid ${key} limit`);
    assert(Number.isFinite(drift) && drift >= 0 && drift < limit, `${arm}: ${key} drift`);
  }
  for (const key of ['corrected_energy_drift', 'independent_windowed_energy_drift',
    'care_boundary_energy_drift']) {
    const drift = a[key], limit = a.pre_intervention_baseline.limits.energy;
    assert(Number.isFinite(drift) && drift >= 0 && drift < limit, `${arm}: ${key}`);
  }

  const rest = a.rest;
  assert.equal(rest.reconciled, true, `${arm}: the observer did not reconcile`);
  assert.equal(rest.violations, 0, `${arm}: retained violations`);
  assert.equal(rest.held_intake_ticks, 0, `${arm}: a held interval took intake`);
  for (const c of CLASSES) {
    safeCount(rest[c].organism_ticks);
    safeCount(rest[c].bouts);
    safeCount(rest[c].seam_ticks);
    assert(Number.isFinite(rest[c].same_face_path_px) && rest[c].same_face_path_px >= 0);
  }
  safeCount(rest.organism_ticks);
  safeCount(rest.active_organism_ticks);
  // Every organism-tick is classified exactly once, and the resting ones are exactly the
  // classified ones. A mode-only reading could not make this statement.
  const classified = CLASSES.reduce((n, c) => n + rest[c].organism_ticks, 0);
  assert.equal(classified + rest.active_organism_ticks, rest.organism_ticks,
    `${arm}: organism-ticks are not partitioned`);
  assert.equal(rest.mode_ticks.resting, classified,
    `${arm}: resting mode-ticks disagree with the classification`);
  assert.equal(rest.mode_ticks.resting + rest.mode_ticks.seeking + rest.mode_ticks.feeding,
    rest.organism_ticks, `${arm}: mode-ticks are not partitioned`);
  assert(rest.intake_ticks <= rest.organism_ticks);

  const q = rest.quiet_records;
  safeCount(q.admissions); safeCount(q.releases);
  const aborts = Object.values(q.aborts).reduce((n, x) => (safeCount(x), n + x), 0);
  const refusals = Object.values(q.refusals).reduce((n, x) => (safeCount(x), n + x), 0);
  assert(q.releases + aborts <= q.admissions, `${arm}: more pauses ended than began`);
  // Held ticks reported by the world's own records cannot exceed the window per admission.
  assert(q.completed_held_ticks <= q.admissions * WINDOW_TICKS + 1e-9,
    `${arm}: reported held ticks exceed the admitted windows`);

  const candidate = arm.startsWith('candidate');
  assert.equal(a.quiet_policy, candidate ? 'post_birth_pause_v1' : 'off');
  if (!candidate) {
    // The reference arm is the unchanged autonomous controller, and must show it.
    assert.equal(q.admissions, 0, `${arm}: an Off arm admitted a pause`);
    assert.equal(refusals, 0, `${arm}: an Off arm published a refusal`);
    assert.equal(rest.post_birth_recovery.organism_ticks, 0, `${arm}: an Off arm recovered`);
    assert.equal(rest.post_birth_recovery.bouts, 0);
    assert.equal(a.open_pauses_at_close, 0);
  }

  const fed = arm.endsWith('_feed');
  assert.equal(a.care.planned, fed);
  if (fed) {
    assert.equal(a.care.receipts.length, 1, `${arm}: the recipe is exactly one Feed`);
    const r = a.care.receipts[0];
    assert.equal(r.elapsed, 600); assert.equal(r.kind, 'feed');
    assert.equal(r.dose_permille, 1000);
    assert.deepEqual(r.target, {face: 0, u: 32, v: 48});
    assert.equal(a.care.ledgers.admitted_seq, 1, `${arm}: a second care command`);
    assert.equal(a.care.ledgers.rain_depth_in, 0, `${arm}: rain is not in this recipe`);
    assert.equal(a.care.ledgers.clean_material_out, 0, `${arm}: cleanup is not in this recipe`);
  } else {
    assert.equal(a.care.receipts.length, 0, `${arm}: a no-care arm received an input`);
    assert.equal(a.care.ledgers.admitted_seq, 0);
  }
  assert(Array.isArray(a.unsupported_measurements) && a.unsupported_measurements.length > 0,
    `${arm}: unsupported measurements must be stated, not omitted`);
}

/// The bout file against the summary it belongs to. Individual full-ID histories are the point:
/// a total that no line supports is not evidence.
export function reduceBouts(rows, summary, opening, closing) {
  const out = {byClass: {}, released: 0, aborted: {}, censored: 0, died: 0,
    screenBouts: 0, longestRecovery: 0, recoveryOrigins: new Set()};
  for (const c of CLASSES) out.byClass[c] = {bouts: 0, ticks: 0};
  for (const b of rows) {
    assert(CLASSES.includes(b.class), `unknown rest class ${b.class}`);
    safeCount(b.ticks);
    assert(b.ticks >= 1, 'a bout of no ticks');
    assert(Number.isSafeInteger(b.start_tick) && Number.isSafeInteger(b.end_tick));
    assert(b.start_tick > opening && b.end_tick <= closing && b.start_tick <= b.end_tick,
      `bout outside the run: ${b.start_tick}..${b.end_tick}`);
    assert.equal(b.end_tick - b.start_tick + 1, b.ticks, 'bout length disagrees with its range');
    assert(b.id && Number.isSafeInteger(b.id.slot) && Number.isSafeInteger(b.id.generation),
      'a bout without a full generational identity');
    out.byClass[b.class].bouts++;
    out.byClass[b.class].ticks += b.ticks;
    if (b.class === 'post_birth_recovery') {
      assert(b.ticks <= WINDOW_TICKS, `a recovery bout of ${b.ticks} ticks outlived its window`);
      assert(b.origin_child && Number.isSafeInteger(b.origin_boundary),
        'a recovery bout without its originating birth');
      assert.equal(b.start_tick, b.origin_boundary + 1,
        'a recovery bout does not start at B+1');
      out.recoveryOrigins.add(`${idKey(b.id)}@${b.origin_boundary}`);
      out.longestRecovery = Math.max(out.longestRecovery, b.ticks);
      if (b.ticks >= SCREEN_TICKS) out.screenBouts++;
      if (b.end === 'released') {
        assert.equal(b.ticks, WINDOW_TICKS, 'a released bout did not complete the window');
        assert.equal(b.end_tick, b.origin_boundary + WINDOW_TICKS);
        out.released++;
      }
      if (b.end === 'aborted') {
        assert(b.ticks < WINDOW_TICKS, 'an aborted bout completed the whole window');
        assert(typeof b.end_detail === 'string' && b.end_detail.length > 0,
          'an abort without a reason');
        out.aborted[b.end_detail] = (out.aborted[b.end_detail] || 0) + 1;
      }
    } else {
      assert.equal(b.origin_child, null, `${b.class} must not name an originating birth`);
    }
    if (b.class === 'newborn_initial') {
      assert.equal(b.start_tick, b.end_tick - b.ticks + 1);
    }
    if (b.end === 'censored') out.censored++;
    if (b.end === 'died') out.died++;
  }
  // The file and the summary must be the same measurement.
  for (const c of CLASSES) {
    assert.equal(out.byClass[c].bouts, summary.rest[c].bouts, `${c}: bout count mismatch`);
    assert.equal(out.byClass[c].ticks, summary.rest[c].organism_ticks,
      `${c}: bout ticks disagree with the classified organism-ticks`);
  }
  assert.equal(out.released, summary.rest.quiet_records.releases,
    'released bouts disagree with the release records');
  assert.equal(rows.length, summary.bouts_written, 'bout file length disagrees with the summary');
  // One recovery bout per admitted pause at most: a duplicate observation cannot extend a timer
  // or invent a second bout for the same birth.
  assert(out.recoveryOrigins.size === out.byClass.post_birth_recovery.bouts,
    'two recovery bouts share one originating birth');
  assert(out.byClass.post_birth_recovery.bouts <= summary.rest.quiet_records.admissions,
    'more recovery bouts than admissions');
  return out;
}

/// The transient record stream against the same summary.
export function reduceQuietEvents(rows, summary, opening, closing) {
  const out = {begin: 0, refuse: 0, end: 0, abort: 0, refusals: {}, aborts: {}};
  const admitted = new Set();
  for (const e of rows) {
    assert(e.kind in out, `unknown quiet record ${e.kind}`);
    assert(Number.isSafeInteger(e.tick) && e.tick > opening && e.tick <= closing,
      `quiet record outside the run at ${e.tick}`);
    assert(e.parent && e.child, 'a record without both full identities');
    out[e.kind]++;
    if (e.kind === 'begin') {
      assert.equal(e.end_tick, e.tick + WINDOW_TICKS, 'an admission window is not the candidate\'s');
      const key = `${idKey(e.parent)}@${e.tick}`;
      assert(!admitted.has(key), 'the same parent was admitted twice at one boundary');
      admitted.add(key);
    }
    if (e.kind === 'end') assert.equal(e.completed_ticks, WINDOW_TICKS, 'a release did not complete');
    if (e.kind === 'abort') {
      assert(e.completed_ticks < WINDOW_TICKS, 'an abort completed the whole window');
      out.aborts[e.reason] = (out.aborts[e.reason] || 0) + 1;
    }
    if (e.kind === 'refuse') out.refusals[e.reason] = (out.refusals[e.reason] || 0) + 1;
  }
  const q = summary.rest.quiet_records;
  assert.equal(out.begin, q.admissions, 'admissions disagree with the record stream');
  assert.equal(out.end, q.releases, 'releases disagree with the record stream');
  assert.deepEqual(out.aborts, q.aborts, 'abort reasons disagree with the record stream');
  assert.deepEqual(out.refusals, q.refusals, 'refusal reasons disagree with the record stream');
  return out;
}

export async function loadRun(directory) {
  const root = resolve(directory);
  const m = await json(join(root, 'manifest.json'));
  const summary = await json(join(root, 'summary.json')); // Missing means still running.
  assert.equal(m.kind, 'four-arm-ordinary-quiet-comparison');
  assert.deepEqual(m.arms, ARMS);
  assert.equal(m.sample_every, 200, 'the prescribed census cadence is 200 ticks');
  assert(Number.isSafeInteger(m.ticks) && m.ticks >= 12000,
    'a prescribed horizon is at least the ten-minute one');
  assert.equal(createHash('sha256').update(await readFile(join(root, 'quiet_compare.frozen')))
    .digest('hex'), m.executable_sha256, 'the frozen executable does not match its manifest');
  assert.equal(summary.technical_complete, true);
  assert.equal(summary.audit_passed, true);
  assert.equal(summary.complete_experiment_measurement, true);
  assert.deepEqual(summary.seeds.map(s => s.seed).sort((a, b) => a - b),
    Array.from({length: 12}, (_, i) => i + 1), 'the prescribed twelve seeds are required');

  const opening = m.cohort.opening_tick;
  const closing = opening + m.ticks;
  const seeds = [];
  for (const s of summary.seeds) {
    const dir = join(root, `seed-${s.seed}`);
    assert.deepEqual(await json(join(dir, 'result.json')), s);
    assert.equal(s.failure, null, `seed ${s.seed}: retained failure`);
    assert.equal(s.arms.length, ARMS.length);
    const arms = {};
    for (const [i, name] of ARMS.entries()) {
      const path = join(dir, name), a = s.arms[i];
      const openingJson = await json(join(path, 'opening.json'));
      assert.equal(openingJson.arm, name);
      assert.deepEqual(await json(join(path, 'summary.json')), a);
      verifyArm(a, m.ticks, opening, name);
      const bouts = reduceBouts(await jsonl(join(path, 'bouts.jsonl')), a, opening, closing);
      const events = reduceQuietEvents(await jsonl(join(path, 'quiet-events.jsonl')), a,
        opening, closing);
      const census = await jsonl(join(path, 'census.jsonl'));
      assert.equal(census.length, m.ticks / m.sample_every, `${name}: census windows missing`);
      assert.equal(census[census.length - 1].tick, closing, `${name}: census stops early`);
      arms[name] = {opening: openingJson, summary: a, bouts, events, census};
    }
    // Every arm of a seed starts from one shared pre-intervention baseline.
    const base = JSON.stringify(s.pre_intervention_baseline);
    for (const name of ARMS)
      assert.equal(JSON.stringify(arms[name].opening.pre_intervention_baseline), base,
        `${name}: a different baseline`);
    seeds.push({seed: s.seed, arms});
  }
  return {root, manifest: m, seeds};
}

/// The paired comparison, per seed. Every pair is reported; nothing is pooled into a verdict.
export function pair(off, candidate) {
  const of_ = a => ({
    population_organism_ticks: a.summary.population_organism_ticks,
    births: a.summary.births,
    deaths: a.summary.deaths,
    closing_population: a.summary.closing_population,
    surviving_opening_cohorts: a.summary.surviving_opening_cohorts,
    maximum_descendant_depth: a.summary.maximum_descendant_depth,
    first_extinction_tick: a.summary.first_extinction_tick,
    forms_present: a.census[a.census.length - 1].population_by_form.filter(n => n > 0).length,
    rest: Object.fromEntries(CLASSES.map(c => [c, a.summary.rest[c]])),
    intake_ticks: a.summary.rest.intake_ticks,
    mode_ticks: a.summary.rest.mode_ticks,
    quiet: a.summary.rest.quiet_records,
    sampled_transported_px: a.summary.rest.sampled_transported_px,
    sampled_path_ticks: a.summary.rest.sampled_path_ticks,
  });
  const a = of_(off), b = of_(candidate);
  return {
    off: a, candidate: b,
    delta: {
      population_organism_ticks: b.population_organism_ticks - a.population_organism_ticks,
      births: b.births - a.births,
      closing_population: b.closing_population - a.closing_population,
      surviving_opening_cohorts: b.surviving_opening_cohorts - a.surviving_opening_cohorts,
      forms_present: b.forms_present - a.forms_present,
      intake_ticks: b.intake_ticks - a.intake_ticks,
    },
    // Losses the candidate has and its matched reference does not. The proposal asks for every
    // paired loss to be inspected, not a favourable pooled mean.
    paired_losses: {
      new_extinction: a.first_extinction_tick === null && b.first_extinction_tick !== null,
      lost_forms: Math.max(0, a.forms_present - b.forms_present),
      lost_opening_cohorts: Math.max(0, a.surviving_opening_cohorts - b.surviving_opening_cohorts),
      fewer_births: Math.max(0, a.births - b.births),
    },
  };
}

export async function compare(directory) {
  const run = await loadRun(directory);
  const seeds = run.seeds.map(s => ({
    seed: s.seed,
    nocare: pair(s.arms.off_nocare, s.arms.candidate_nocare),
    feed: pair(s.arms.off_feed, s.arms.candidate_feed),
  }));
  // The proposal's behavioural screen, reported and never treated as success on its own.
  const screened = run.seeds.filter(s => s.arms.candidate_nocare.bouts.screenBouts > 0).length;
  const opportunity = run.seeds.map(s => {
    const a = s.arms.candidate_nocare;
    return {
      seed: s.seed,
      births: a.summary.births,
      admissions: a.summary.rest.quiet_records.admissions,
      refusals: a.summary.rest.quiet_records.refusals,
      admitted_per_birth: ratio(a.summary.rest.quiet_records.admissions, a.summary.births),
      recovery_bouts: a.bouts.byClass.post_birth_recovery.bouts,
      bouts_at_least_one_second: a.bouts.screenBouts,
      longest_recovery_ticks: a.bouts.longestRecovery,
      recovery_share_of_organism_time:
        ratio(a.summary.rest.post_birth_recovery.organism_ticks, a.summary.rest.organism_ticks),
    };
  });
  return {
    kind: 'four-arm-ordinary-quiet-reduction',
    artifact_checks_passed: true,
    horizon: run.manifest.horizon, ticks: run.manifest.ticks, build: run.manifest.build,
    behavioural_screen: {
      rule: `a non-newborn recovery bout of at least ${SCREEN_TICKS} ticks in at least ${SCREEN_SEEDS} of 12 no-care seeds`,
      seeds_meeting_it: screened,
      met: screened >= SCREEN_SEEDS,
      note: 'proposed and unvalidated. Meeting it is not success, and a candidate whose entries are all one tick has not met the objective whatever this number says.',
    },
    opportunity,
    seeds,
    biological_acceptance: 'not inferred. Viability is decided from the no-care arms first, by inspecting every paired loss — new extinctions, forms or opening ancestry lost where the matched reference kept them, and declines in living-population time and paid reproduction across seeds and horizons. Longer-term child recruitment matters more than raw birth count.',
    limits: 'Checks artifact integrity, gate-by-gate technical completion, the exact care recipe, the rest classification partition, and the agreement of each bout and record file with its own summary. It does not re-run core audits, decode semantic WorldState, or establish causality. Recovery classification is exact; total transported path length is sampled on the census cadence and reported as a rate, never a total. No funding or oxidation amount here is inferred from a post-step delta. Twelve seeds cannot prove universal noninferiority.',
    root: run.root,
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    assert.equal(process.argv.length, 3, 'Usage: node scripts/reduce-quiet-compare.mjs RUN_DIR');
    console.log(JSON.stringify(await compare(process.argv[2]), null, 2));
  } catch (error) {
    console.error(`Reduction refused: ${error.message}`);
    process.exitCode = 1;
  }
}
