// Tests for the post-intake opportunity reduction.
//
// The unit checks run on synthetic streams, because the interesting cases — an attempt inside a
// cooldown, a window that outlives its length, a release that is not thirty decisions, a credit
// that is not the quota — must not depend on a real run happening to contain them. The end-to-end
// check then runs against the completed pilot, so every check above has met artifacts of the real
// shape and not only fixtures. It is skipped, loudly, when that run is not on this machine.

import test from 'node:test';
import assert from 'node:assert/strict';
import {existsSync} from 'node:fs';
import {
  ARMS, DT, ELAPSED_TICKS, EPISODE_EXPIRY_TICKS, OPENING_TICK, REASONS, REFRACTORY_TICKS, SCREEN,
  SEEDS, WINDOW_DECISIONS, emptyReasons, episodeStats, median, reduce, reduceEvents, replayRule,
} from './post-intake-shadow.mjs';

/// The completed four-arm pilot. Resolved against the repository this script lives in, and also
/// against the main checkout, because the harness runs from an isolated worktree whose own
/// captures tree is not where the run was written.
const CANDIDATES = [
  new URL('../captures/post-intake-shadow-pilot-2026-09-13-v2/', import.meta.url).pathname,
  '/home/wrysk/wryskware/cubarium/captures/post-intake-shadow-pilot-2026-09-13-v2',
];
const PILOT = CANDIDATES.find(p => existsSync(p));

const id = (slot, generation = 1) => ({slot, generation});

function attempt(tick, who, over = {}) {
  const {sample = {}, episode = {}, ...rest} = over;
  return {
    kind: 'attempt', tick, id: who, admitted: true, reason: null,
    episode: {
      start_tick: tick - 10, last_positive_tick: tick, positive_ticks: 4, elapsed_ticks: 10,
      elapsed_seconds: 10 * DT, assimilated: 0.2, credit: 0.1, ...episode,
    },
    sample: {
      form: 2, structure: 1, structure_adult: 1, reserve: 1, energy: 1, reserve_max: 2,
      energy_max: 2, reserve_fraction: 0.5, mode: 'Feeding', gestating: false,
      escrow_started_tick: null, hunter_member: false, age_ticks: 5000, quota: 0.1,
      graze_rate: 0.03, scavenge_rate: 0.01, diet: 0.6, budget: null, ...sample,
    },
    window_end_tick: tick + WINDOW_DECISIONS, refractory_until: null,
    ...rest,
  };
}

function refusal(tick, who, reason, over = {}) {
  const {sample, episode, ...rest} = over;
  const row = attempt(tick, who, {sample, episode});
  return {
    ...row, admitted: false, reason, window_end_tick: null,
    refractory_until: tick + REFRACTORY_TICKS,
    ...rest,
  };
}

function release(tick, who, start, over = {}) {
  return {
    kind: 'release', tick, id: who, window_start_tick: start,
    compatible_decisions: WINDOW_DECISIONS, compatible_seconds: WINDOW_DECISIONS * DT,
    baseline_intake_material: 0.5, baseline_intake_ticks: 10, baseline_died_at_release: false,
    sample: attempt(tick, who).sample, refractory_until: tick + REFRACTORY_TICKS, ...over,
  };
}

test('the pilot this reduction checks is the four arms root authorised', () => {
  assert.deepEqual(SEEDS, [1, 8]);
  assert.deepEqual(ARMS, ['off_nocare', 'off_feed']);
  assert.equal(SEEDS.length * ARMS.length, 4);
  assert.equal(ELAPSED_TICKS, 12000);
  assert.equal(OPENING_TICK, 144000);
  assert.equal(WINDOW_DECISIONS, 30);
  assert.equal(REFRACTORY_TICKS, 600);
  assert.equal(EPISODE_EXPIRY_TICKS, 20);
});

test('every reason is always a row, so a zero reads as never happened', () => {
  const table = emptyReasons();
  assert.equal(Object.keys(table).length, REASONS.length);
  for (const reason of REASONS) assert.equal(table[reason], 0, reason);
});

test('a clean cycle replays without a single violation', () => {
  const who = id(1);
  const rows = [
    attempt(100, who),
    release(130, who, 100),
    // The next attempt is allowed exactly at the deadline, not one tick before it.
    attempt(730, who),
    release(760, who, 730),
  ];
  const {violations, open_at_end: open} = replayRule(rows);
  assert.deepEqual(violations, []);
  assert.equal(open, 0);
});

test('an attempt inside a cooldown is a refusal of the run, not a footnote', () => {
  const who = id(1);
  const {violations} = replayRule([attempt(100, who), release(130, who, 100), attempt(729, who)]);
  assert.ok(violations.some(v => /inside a cooldown to 730/.test(v)), violations);
});

test('an attempt with a window already open is caught', () => {
  const who = id(1);
  const {violations} = replayRule([attempt(100, who), attempt(110, who)]);
  assert.match(violations[0], /while a window opened at 100/);
});

test('a credit that is not exactly the quota is caught', () => {
  const who = id(1);
  const {violations} = replayRule([attempt(100, who, {episode: {credit: 0.09}})]);
  assert.match(violations[0], /credit 0.09 against quota 0.1/);
});

test('an attempt without this boundary\'s own positive settlement is caught', () => {
  const who = id(1);
  const rows = [attempt(100, who, {episode: {last_positive_tick: 98, elapsed_ticks: 8}})];
  const {violations} = replayRule(rows);
  assert.ok(violations.some(v => /without this boundary's own positive settlement/.test(v)),
    violations);
});

test('a release that is not a whole window, and an abort that is one, are both caught', () => {
  const who = id(1);
  const short = replayRule([
    attempt(100, who),
    release(120, who, 100, {compatible_decisions: 20, tick: 120}),
  ]);
  assert.ok(short.violations.some(v => /is not a whole window/.test(v)), short.violations);

  const long = replayRule([
    attempt(100, who),
    {
      kind: 'abort', tick: 130, id: who, window_start_tick: 100, compatible_decisions: 30,
      compatible_seconds: 1.5, reason: 'unaffordable_remaining', baseline_intake_material: 0,
      baseline_intake_ticks: 0, sample: null, refractory_until: 730,
    },
  ]);
  assert.ok(long.violations.some(v => /ran a whole window/.test(v)), long.violations);
});

test('a window that is never closed is reported rather than forgotten', () => {
  const who = id(4, 2);
  const {violations, open_at_end: open} = replayRule([attempt(100, who)]);
  assert.equal(open, 1);
  assert.match(violations[0], /never closed or censored/);
});

test('a refusal that failed to arm the cooldown is caught', () => {
  const who = id(1);
  const {violations} = replayRule([refusal(100, who, 'gestating', {refractory_until: 100})]);
  assert.match(violations[0], /did not arm the cooldown/);
});

test('repeats are counted as repeats and distinct IDs as distinct', () => {
  const who = id(7);
  const rows = [
    refusal(100, who, 'gestating', {sample: {escrow_started_tick: 40}}),
    refusal(700, who, 'gestating', {sample: {escrow_started_tick: 40}}),
    refusal(1300, who, 'gestating', {sample: {escrow_started_tick: 40}}),
    refusal(1900, id(8), 'juvenile'),
  ];
  const {total, distinct, byForm} = reduceEvents(rows);
  assert.equal(total.attempts, 4, 'every real attempt is kept');
  assert.equal(total.refusals.gestating, 3);
  assert.equal(distinct.attempted.size, 2);
  assert.equal(distinct.refusedGestations.size, 1, 'three attempts, one gestation');
  assert.equal(byForm[2].attempts, 4);
});

test('a censored window is a censored window and never a release', () => {
  const who = id(3);
  const rows = [
    attempt(100, who),
    {
      kind: 'censor', tick: 112, id: who, window_start_tick: 100, compatible_decisions: 12,
      compatible_seconds: 12 * DT, baseline_intake_material: 0.3, baseline_intake_ticks: 5,
      censored: true,
    },
  ];
  const {total, byForm, windows} = reduceEvents(rows);
  assert.equal(total.releases, 0);
  assert.equal(total.censored, 1);
  assert.equal(total.refusals.run_end, 1);
  // A right-censored window's decisions are real and belong to the form that earned them. This is
  // the disagreement that failed the first pilot's own per-form table.
  assert.equal(total.compatible_window_decisions, 12);
  assert.equal(byForm[2].compatible_window_decisions, 12);
  assert.equal(byForm[2].censored, 1);
  assert.equal(windows[0].reason, 'run_end');
  assert.deepEqual(replayRule(rows).violations, []);
});

test('a swept abort with no sample is still attributed to the form that opened the window', () => {
  const who = id(5);
  const rows = [
    attempt(100, who, {sample: {form: 1}}),
    {
      kind: 'abort', tick: 105, id: who, window_start_tick: 100, compatible_decisions: 5,
      compatible_seconds: 5 * DT, reason: 'baseline_gone', baseline_intake_material: 0.1,
      baseline_intake_ticks: 2, sample: null, refractory_until: null,
    },
  ];
  const {total, byForm} = reduceEvents(rows);
  assert.equal(byForm[1].aborts, 1);
  assert.equal(byForm[1].compatible_window_decisions, 5);
  assert.equal(total.refusals.baseline_gone, 1);
  assert.deepEqual(replayRule(rows).violations, []);
});

test('baseline intake cannot exceed the decisions the window had', () => {
  const who = id(1);
  const rows = [attempt(100, who), release(130, who, 100, {baseline_intake_ticks: 31})];
  const {violations} = replayRule(rows);
  assert.match(violations[0], /more baseline intake ticks than the window had decisions/);
});

test('episode statistics are per form and say what they measure', () => {
  const episodes = Array.from({length: 8}, () => []);
  episodes[2] = [
    {elapsed_ticks: 20, positive_ticks: 4, assimilated: 0.2, credit: 0.1, quota: 0.1},
    {elapsed_ticks: 60, positive_ticks: 9, assimilated: 0.3, credit: 0.1, quota: 0.1},
    {elapsed_ticks: 100, positive_ticks: 12, assimilated: 0.4, credit: 0.1, quota: 0.1},
  ];
  const stats = episodeStats(episodes);
  assert.equal(stats[2].attempts, 3);
  assert.equal(stats[2].seconds_to_quota.min, 1);
  assert.equal(stats[2].seconds_to_quota.max, 5);
  assert.equal(stats[2].seconds_to_quota.median, 3);
  assert.ok(stats[2].basis.includes('not the same as time spent eating'));
  // A form that never attempted keeps a visible row rather than vanishing.
  assert.equal(stats[0].attempts, 0);
  assert.equal(stats[0].seconds_to_quota, null);
});

test('median of an empty distribution is absent, not zero', () => {
  assert.equal(median([]), null);
  assert.equal(median([1, 2, 3]), 2);
});

test('the usefulness screen is reported as unevaluated on a two-seed pilot', async t => {
  if (!PILOT) {
    t.skip(`the completed pilot is not on this machine (looked in ${CANDIDATES.join(', ')})`);
    return;
  }
  const report = await reduce(PILOT);
  assert.equal(report.screen.evaluated, false);
  assert.equal(report.screen.thresholds.no_care_seeds, SCREEN.no_care_seeds);
  assert.match(report.screen.why, /two seeds/);
  assert.ok(report.limits.some(l => /not an upper bound/.test(l)));
});

test('the completed pilot reduces without a single disagreement', async t => {
  if (!PILOT) {
    t.skip(`the completed pilot is not on this machine (looked in ${CANDIDATES.join(', ')})`);
    return;
  }
  const report = await reduce(PILOT);
  assert.deepEqual(report.problems, [], 'the pilot disagrees with its own event stream');
  assert.equal(report.reduction_passed, true);
  assert.equal(report.arms.length, 4);
  for (const arm of report.arms) {
    assert.deepEqual(arm.replay.violations, [], `${arm.seed}/${arm.arm}`);
    assert.equal(arm.replay.open_at_end, 0);
    // Real opportunity really was found, and repeats really were preserved.
    assert.ok(arm.reduced.total.attempts > 0, 'no attempt at all');
    assert.ok(arm.reduced.total.attempts >= arm.reduced.distinct.attempted,
      'attempts were collapsed to distinct IDs');
    assert.ok(arm.screen_inputs.exposed_forms >= 2, 'only one form was ever exposed');
  }
});
