// Read-only reduction of a completed post-intake opportunity shadow pilot.
//
// Reduces `captures/post-intake-shadow-pilot-2026-09-13`-shaped output against the fixed rule of
// design/7_Research/root-post-intake-shadow-scope-2026-09-13.md. It writes nothing, steps nothing
// and decides nothing about ecology.
//
// **Nothing an arm says about itself is taken as evidence for itself.** Every counter, per-form
// table and distinct-ID summary is recomputed from `shadow-events.jsonl` and compared to the
// summary exactly; the rule is replayed per full generational ID as a state machine, so a window
// that outlived its length, an attempt inside a cooldown, an attempt with a window already open,
// a credit that is not the quota, a release that is not thirty decisions, or a close that never
// happened is a refusal rather than a footnote. The provenance manifests and the frozen
// executable are checksummed against what the manifest claims.
//
// What it deliberately does **not** do: turn a compatible window into a pause, a compatible-window
// fraction into an upper bound, baseline intake during a window into missed food, or a passing
// technical gate into evidence that anything would be readable at native 64 px. The usefulness
// screen root named needs nine of twelve no-care seeds; a two-seed pilot cannot evaluate it and
// this script reports the inputs to it rather than a verdict on it.

import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {join} from 'node:path';

/// The pilot root authorised, and nothing else.
export const SEEDS = [1, 8];
export const ARMS = ['off_nocare', 'off_feed'];
export const ELAPSED_TICKS = 12000;
export const OPENING_TICK = 144000;
/// The rule's fixed constants, restated here so the reduction is an independent check of the run
/// rather than a reading of its own manifest.
export const WINDOW_DECISIONS = 30;
export const REFRACTORY_TICKS = 600;
export const EPISODE_EXPIRY_TICKS = 20;
export const QUOTA_SECONDS = 2;
export const DT = 1 / 20;
/// Every reason the core can name, in its own fixed order.
export const REASONS = [
  'dead', 'hunter_member', 'juvenile', 'gestating', 'inactive_mode', 'invalid_inputs',
  'unaffordable', 'overflow', 'bounded', 'unaffordable_remaining', 'baseline_gone',
  'baseline_escrow', 'baseline_juvenile', 'run_end',
];
/// Root's unvalidated usefulness thresholds. Recorded so the inputs to them are visible; this
/// pilot cannot evaluate the screen, which needs nine of twelve no-care seeds.
export const SCREEN = {
  eligible_adult_exposure: 0.002,
  whole_world_compatible_window_time: 0.15,
  no_care_seeds: 9,
  of_seeds: 12,
};

const key = id => `${id.slot}/${id.generation}`;
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const sum = xs => xs.reduce((a, b) => a + b, 0);

export function median(sorted) {
  if (sorted.length === 0) return null;
  return sorted[Math.floor(sorted.length / 2)];
}

export async function readJson(path) {
  return JSON.parse(await readFile(path, 'utf8'));
}

export async function readEvents(path) {
  const text = await readFile(path, 'utf8');
  return text.split('\n').filter(l => l.length > 0).map(l => JSON.parse(l));
}

/// An empty per-reason table. Every reason is always a row, so a zero reads as "never happened"
/// rather than as an absent key.
export function emptyReasons() {
  return Object.fromEntries(REASONS.map(r => [r, 0]));
}

function blankExposure() {
  return {
    settlement_ticks: 0, attempts: 0, admissions: 0, releases: 0, aborts: 0, censored: 0,
    compatible_window_decisions: 0, baseline_intake_in_windows: 0, refusals: emptyReasons(),
  };
}

/// Recompute, from the event stream alone, everything the summary claims about it.
export function reduceEvents(rows) {
  const total = blankExposure();
  const byForm = Array.from({length: 8}, blankExposure);
  const distinct = {
    attempted: new Set(), admitted: new Set(), released: new Set(),
    refusedGestations: new Set(), refusedByReason: new Map(),
  };
  const windows = [];
  const episodes = Array.from({length: 8}, () => []);
  const openWindowForm = new Map();
  for (const row of rows) {
    const id = key(row.id);
    if (row.kind === 'attempt') {
      const form = row.sample.form;
      const bucket = [total, byForm[form]];
      for (const b of bucket) b.attempts += 1;
      distinct.attempted.add(id);
      episodes[form].push({
        elapsed_ticks: row.episode.elapsed_ticks,
        positive_ticks: row.episode.positive_ticks,
        assimilated: row.episode.assimilated,
        credit: row.episode.credit,
        quota: row.sample.quota,
        admitted: row.admitted,
      });
      if (row.admitted) {
        for (const b of bucket) b.admissions += 1;
        distinct.admitted.add(id);
        openWindowForm.set(id, form);
      } else {
        for (const b of bucket) b.refusals[row.reason] += 1;
        if (!distinct.refusedByReason.has(row.reason)) {
          distinct.refusedByReason.set(row.reason, new Set());
        }
        distinct.refusedByReason.get(row.reason).add(id);
        if (row.sample.escrow_started_tick !== null
            && row.sample.escrow_started_tick !== undefined) {
          distinct.refusedGestations.add(`${id}@${row.sample.escrow_started_tick}`);
        }
      }
      continue;
    }
    // A close. `abort` from the commit sweep carries no sample, so the form comes from the
    // admission that opened the window rather than from an absent field.
    const form = row.sample?.form ?? openWindowForm.get(id);
    const bucket = form === undefined ? [total] : [total, byForm[form]];
    windows.push({
      id, form, kind: row.kind, decisions: row.compatible_decisions,
      start: row.window_start_tick, end: row.tick,
      baseline_intake_material: row.baseline_intake_material,
      baseline_intake_ticks: row.baseline_intake_ticks,
      reason: row.reason ?? (row.kind === 'censor' ? 'run_end' : null),
    });
    for (const b of bucket) {
      b.compatible_window_decisions += row.compatible_decisions;
      b.baseline_intake_in_windows += row.baseline_intake_material;
    }
    if (row.kind === 'release') {
      for (const b of bucket) b.releases += 1;
      distinct.released.add(id);
    } else if (row.kind === 'abort') {
      for (const b of bucket) b.aborts += 1;
      for (const b of bucket) b.refusals[row.reason] += 1;
    } else {
      total.censored += 1;
      if (form !== undefined) byForm[form].censored += 1;
      total.refusals.run_end += 1;
      if (form !== undefined) byForm[form].refusals.run_end += 1;
    }
    openWindowForm.delete(id);
  }
  return {total, byForm, distinct, windows, episodes};
}

/// Replay the rule per full generational ID. Every violation is collected, never thrown away: a
/// run with one bad window is a run with one bad window, and the reader is owed the list.
export function replayRule(rows) {
  const violations = [];
  const state = new Map();
  let last = -1;
  for (const row of rows) {
    if (row.tick < last) violations.push(`records are out of tick order at ${row.tick}`);
    last = row.tick;
    const id = key(row.id);
    const s = state.get(id) ?? {window: null, deadline: null, closes: 0, admissions: 0};
    state.set(id, s);
    if (row.kind === 'attempt') {
      if (s.window !== null) {
        violations.push(`${id}: an attempt at ${row.tick} while a window opened at ${s.window} \
was still open`);
      }
      if (s.deadline !== null && row.tick < s.deadline) {
        violations.push(`${id}: an attempt at ${row.tick} inside a cooldown to ${s.deadline}`);
      }
      if (row.sample.quota === null) {
        violations.push(`${id}: an attempt at ${row.tick} with no quota at all`);
      } else if (row.episode.credit !== row.sample.quota) {
        violations.push(`${id}: an attempt at ${row.tick} with credit ${row.episode.credit} \
against quota ${row.sample.quota}; every attempt must consume exactly the quota`);
      }
      if (row.episode.assimilated < row.episode.credit) {
        violations.push(`${id}: an attempt at ${row.tick} credited more than it assimilated`);
      }
      if (row.episode.elapsed_ticks !== row.episode.last_positive_tick - row.episode.start_tick) {
        violations.push(`${id}: an episode length at ${row.tick} is not its own span`);
      }
      if (row.episode.last_positive_tick !== row.tick) {
        violations.push(`${id}: an attempt at ${row.tick} fired without this boundary's own \
positive settlement (last positive ${row.episode.last_positive_tick})`);
      }
      if (row.admitted) {
        if (row.reason !== null) violations.push(`${id}: an admission at ${row.tick} carries a reason`);
        if (row.window_end_tick !== row.tick + WINDOW_DECISIONS) {
          violations.push(`${id}: a window opened at ${row.tick} does not end at +${WINDOW_DECISIONS}`);
        }
        // An admission reports the *spent* deadline it came out of; the operative one is armed by
        // the close.
        if (row.refractory_until !== null && row.refractory_until > row.tick) {
          violations.push(`${id}: an admission at ${row.tick} reports a live cooldown`);
        }
        s.window = row.tick;
        s.admissions += 1;
      } else {
        if (!REASONS.includes(row.reason)) {
          violations.push(`${id}: an unknown refusal reason ${row.reason} at ${row.tick}`);
        }
        if (row.window_end_tick !== null) {
          violations.push(`${id}: a refusal at ${row.tick} opened a window`);
        }
        if (row.refractory_until !== row.tick + REFRACTORY_TICKS) {
          violations.push(`${id}: a refusal at ${row.tick} did not arm the cooldown`);
        }
        s.deadline = row.refractory_until;
      }
      continue;
    }
    if (s.window === null) {
      violations.push(`${id}: a ${row.kind} at ${row.tick} closed no open window`);
    } else if (row.window_start_tick !== s.window) {
      violations.push(`${id}: a ${row.kind} at ${row.tick} names window ${row.window_start_tick}, \
not the open ${s.window}`);
    }
    const expected = Math.min(row.tick - row.window_start_tick, WINDOW_DECISIONS);
    if (row.compatible_decisions !== expected) {
      violations.push(`${id}: a ${row.kind} at ${row.tick} claims ${row.compatible_decisions} \
decisions, not ${expected}`);
    }
    if (row.kind === 'release') {
      if (row.compatible_decisions !== WINDOW_DECISIONS) {
        violations.push(`${id}: a release at ${row.tick} is not a whole window`);
      }
      if (row.refractory_until !== row.tick + REFRACTORY_TICKS) {
        violations.push(`${id}: a release at ${row.tick} did not arm the cooldown`);
      }
      s.deadline = row.refractory_until;
    } else if (row.kind === 'abort') {
      if (row.compatible_decisions >= WINDOW_DECISIONS) {
        violations.push(`${id}: an abort at ${row.tick} ran a whole window`);
      }
      // The commit sweep's abort for a removed organism arms nothing: the ID is gone.
      if (row.refractory_until !== null && row.refractory_until !== row.tick + REFRACTORY_TICKS) {
        violations.push(`${id}: an abort at ${row.tick} armed a wrong cooldown`);
      }
      s.deadline = row.refractory_until;
    } else if (row.compatible_decisions >= WINDOW_DECISIONS) {
      violations.push(`${id}: a censor at ${row.tick} ran a whole window`);
    }
    if (row.baseline_intake_ticks > row.compatible_decisions) {
      violations.push(`${id}: a ${row.kind} at ${row.tick} records more baseline intake ticks \
than the window had decisions`);
    }
    s.window = null;
    s.closes += 1;
  }
  let openAtEnd = 0;
  for (const [id, s] of state) {
    if (s.window !== null) {
      openAtEnd += 1;
      violations.push(`${id}: a window opened at ${s.window} was never closed or censored`);
    }
  }
  return {violations, open_at_end: openAtEnd, tracked_ids: state.size};
}

/// Per-form episode statistics: how long a form's animals really took to earn Q, and how much they
/// actually assimilated doing it. Fable's correction: without this, a per-form zero or a per-form
/// rarity is misread as biology when it is the quota's normalisation.
export function episodeStats(episodes) {
  return episodes.map((rows, form) => {
    if (rows.length === 0) {
      return {form, attempts: 0, seconds_to_quota: null, assimilated_mean: null, quota: null};
    }
    const spans = rows.map(r => r.elapsed_ticks).sort((a, b) => a - b);
    const positives = rows.map(r => r.positive_ticks).sort((a, b) => a - b);
    const quotas = [...new Set(rows.map(r => r.quota))];
    return {
      form,
      attempts: rows.length,
      seconds_to_quota: {
        min: spans[0] * DT,
        median: median(spans) * DT,
        max: spans[spans.length - 1] * DT,
        mean: (sum(spans) / spans.length) * DT,
      },
      positive_ticks: {min: positives[0], median: median(positives),
        max: positives[positives.length - 1]},
      assimilated_mean: sum(rows.map(r => r.assimilated)) / rows.length,
      quota: {min: Math.min(...quotas), max: Math.max(...quotas), distinct: quotas.length},
      basis: 'the elapsed span from an episode\'s first positive settlement to the one that '
        + 'carried it to Q. Gappy intake means this is not the same as time spent eating.',
    };
  });
}

/// One arm, verified end to end. Returns the recomputed reduction beside the arm's own claims and
/// the list of every disagreement.
export async function verifyArm(root, seed, arm) {
  const dir = join(root, `seed-${seed}`, arm);
  const summary = await readJson(join(dir, 'summary.json'));
  const opening = await readJson(join(dir, 'opening.json'));
  const rows = await readEvents(join(dir, 'shadow-events.jsonl'));
  const census = await readEvents(join(dir, 'census.jsonl'));
  const problems = [];
  const reduced = reduceEvents(rows);
  const replay = replayRule(rows);
  problems.push(...replay.violations);

  const check = (ok, message) => {
    if (!ok) problems.push(message);
  };
  check(summary.arm === arm, `the summary names arm ${summary.arm}`);
  check(summary.quiet_policy === 'off', `the arm ran policy ${summary.quiet_policy}, not Off`);
  check(opening.quiet_policy === 'off', 'the opening did not carry the Off policy');
  check(summary.elapsed_ticks === ELAPSED_TICKS,
    `the arm ran ${summary.elapsed_ticks} ticks, not ${ELAPSED_TICKS}`);
  check(summary.closing_tick === OPENING_TICK + ELAPSED_TICKS,
    `the arm closed at ${summary.closing_tick}`);
  check(opening.opening_tick === OPENING_TICK, `the arm opened at ${opening.opening_tick}`);
  check((arm === 'off_feed') === (summary.care.planned === true),
    'the arm\'s care does not match its name');
  if (arm === 'off_feed') {
    check(summary.care.applied_at_tick === OPENING_TICK + 600,
      `the feed landed at ${summary.care.applied_at_tick}`);
    check(summary.care.receipts.length === 1, 'the fed arm does not carry exactly one receipt');
  } else {
    check(summary.care.applied_at_tick === null, 'the no-care arm applied care');
    check(summary.care.receipts.length === 0, 'the no-care arm carries a receipt');
  }

  // The three gates, checked as facts rather than as flags.
  const gates = summary.gates;
  check(gates.observer_is_exactly_neutral === true, 'the observer was not proved neutral');
  check(summary.instrumentation.first_disagreement === null,
    `the instrumented and uninstrumented worlds disagreed: ${summary.instrumentation.first_disagreement}`);
  check(summary.instrumentation.compared_ticks === ELAPSED_TICKS,
    `only ${summary.instrumentation.compared_ticks} ticks were compared`);
  check(summary.instrumentation.snapshots_identical === true,
    'the two closing snapshots are not identical');
  check(gates.retained_off_continuation_identical === true,
    'the arm did not continue the retained Off arm exactly');
  const cont = summary.retained_off_continuation;
  for (const field of ['whole_state_equal', 'state_hash_equal', 'ecology_hash_equal',
    'population_equal', 'births_equal', 'deaths_equal']) {
    check(cont[field] === true, `the retained continuation differs: ${field}`);
  }
  check(gates.conservation_and_flow_audits === true, 'the conservation audits did not pass');
  // The audit limits, re-derived from the arm's own opening inventory and the fixed 1e-8 rule.
  const base = opening.pre_intervention_baseline;
  for (const [name, index] of [['material', 0], ['energy', 1], ['water', 2]]) {
    const limit = 1e-8 * Math.max(base[name], 1);
    check(Math.abs(base.limits[name] - limit) < 1e-18,
      `the ${name} limit is not the fixed 1e-8 rule`);
    check(summary.max_absolute_drift[name] < limit,
      `the ${name} drift ${summary.max_absolute_drift[name]} reaches its limit ${limit}`);
  }

  // Every recomputed counter against the arm's own.
  const claimed = summary.shadow.total;
  check(summary.shadow.events_written === rows.length,
    `the arm wrote ${summary.shadow.events_written} records and the stream holds ${rows.length}`);
  for (const field of ['attempts', 'admissions', 'releases', 'aborts', 'censored',
    'compatible_window_decisions']) {
    check(claimed[field] === reduced.total[field],
      `${field}: the summary says ${claimed[field]}, the stream says ${reduced.total[field]}`);
  }
  for (const reason of REASONS) {
    check(claimed.refusals[reason] === reduced.total.refusals[reason],
      `refusals.${reason}: summary ${claimed.refusals[reason]}, stream ${reduced.total.refusals[reason]}`);
  }
  for (let form = 0; form < 8; form += 1) {
    const row = summary.shadow.by_form[form];
    check(row.form === form, `the per-form table is out of order at ${form}`);
    for (const field of ['attempts', 'admissions', 'releases', 'compatible_window_decisions']) {
      check(row[field] === reduced.byForm[form][field],
        `form ${form} ${field}: summary ${row[field]}, stream ${reduced.byForm[form][field]}`);
    }
  }
  const distinct = summary.shadow.distinct;
  check(distinct.attempted_ids === reduced.distinct.attempted.size,
    `distinct attempts: summary ${distinct.attempted_ids}, stream ${reduced.distinct.attempted.size}`);
  check(distinct.admitted_ids === reduced.distinct.admitted.size,
    `distinct admissions: summary ${distinct.admitted_ids}, stream ${reduced.distinct.admitted.size}`);
  check(distinct.refused_gestations === reduced.distinct.refusedGestations.size,
    `refused gestations: summary ${distinct.refused_gestations}, stream ${reduced.distinct.refusedGestations.size}`);
  // Repeats really are preserved: attempts must be at least the distinct count, and the two are
  // allowed to differ. Collapsing them would be the failure.
  check(reduced.total.attempts >= reduced.distinct.attempted.size,
    'more distinct attempting IDs than attempts');

  // Memory: the map never exceeded the world's live organism capacity.
  const memory = summary.shadow.memory;
  check(memory.peak_entries <= memory.bound,
    `the per-ID map peaked at ${memory.peak_entries} against a bound of ${memory.bound}`);
  check(memory.bounded_refusals === 0,
    `${memory.bounded_refusals} records were lost to the bound`);
  for (const row of census) {
    check(row.shadow.entries <= row.shadow.bound,
      `the per-ID map exceeded its bound at tick ${row.tick}`);
    check(row.shadow.open_windows <= row.population,
      `more open windows than organisms at tick ${row.tick}`);
  }
  check(census.length === ELAPSED_TICKS / 200,
    `the census has ${census.length} windows`);

  // Censoring is reported as censoring, never as a release.
  const censored = reduced.windows.filter(w => w.kind === 'censor');
  check(censored.length === reduced.total.censored, 'the censored count disagrees with the stream');
  check(replay.open_at_end === 0, `${replay.open_at_end} windows were left open`);

  const eligible = summary.shadow.total.eligible_ticks;
  const exposure = eligible > 0 ? reduced.total.compatible_window_decisions / eligible : null;
  return {
    seed, arm, problems,
    reduced: {
      total: reduced.total,
      by_form: reduced.byForm.map((e, form) => ({form, ...e})),
      episodes: episodeStats(reduced.episodes),
      windows: {
        closed: reduced.windows.length,
        full_length: reduced.windows.filter(w => w.decisions === WINDOW_DECISIONS).length,
        by_reason: reduced.windows.reduce((acc, w) => {
          const k = w.reason ?? 'release';
          acc[k] = (acc[k] ?? 0) + 1;
          return acc;
        }, {}),
        decisions: (() => {
          const d = reduced.windows.map(w => w.decisions).sort((a, b) => a - b);
          return {min: d[0] ?? null, median: median(d), max: d[d.length - 1] ?? null};
        })(),
      },
      distinct: {
        attempted: reduced.distinct.attempted.size,
        admitted: reduced.distinct.admitted.size,
        released: reduced.distinct.released.size,
        refused_gestations: reduced.distinct.refusedGestations.size,
        refused_by_reason: Object.fromEntries(
          [...reduced.distinct.refusedByReason].map(([r, ids]) => [r, ids.size])),
      },
    },
    screen_inputs: {
      eligible_adult_ticks: eligible,
      organism_ticks: summary.shadow.total.organism_ticks,
      compatible_window_decisions: reduced.total.compatible_window_decisions,
      compatible_decisions_per_eligible_adult_tick: exposure,
      whole_world_compatible_window_fraction:
        summary.shadow.windows.whole_world_compatible_window_fraction,
      exposed_forms: reduced.byForm.filter(e => e.admissions > 0).length,
      basis: 'inputs to root\'s unvalidated usefulness screen, not a verdict on it: the screen '
        + 'asks for nine of twelve no-care seeds and this pilot ran two.',
    },
    replay,
  };
}

/// Provenance: the run's own frozen executable and copied manifests really are what the manifest
/// says they are.
export async function verifyProvenance(root) {
  const manifest = await readJson(join(root, 'manifest.json'));
  const problems = [];
  const frozen = await readFile(join(root, 'post_intake_shadow.frozen'));
  if (sha256(frozen) !== manifest.executable_sha256) {
    problems.push('the frozen executable does not match its recorded checksum');
  }
  for (const [copy, block] of [['cohort-manifest.json', manifest.cohort_manifest],
    ['retained-quiet-manifest.json', manifest.retained_quiet_manifest]]) {
    const bytes = await readFile(join(root, copy));
    if (bytes.length !== block.bytes) problems.push(`${copy} is not its recorded length`);
    if (sha256(bytes) !== block.sha256) problems.push(`${copy} does not match its checksum`);
  }
  if (!manifest.source?.files || Object.keys(manifest.source.files).length === 0) {
    problems.push('the manifest records no source hashes');
  }
  if (manifest.kind !== 'post-intake-opportunity-shadow-pilot') {
    problems.push(`the manifest is a ${manifest.kind}`);
  }
  if (manifest.pilot.quiet_policy !== 'off' || manifest.pilot.hunters !== false) {
    problems.push('the pilot did not run Off and hunter-free');
  }
  const seeds = manifest.pilot.seeds;
  if (JSON.stringify(seeds) !== JSON.stringify(SEEDS)) {
    problems.push(`the pilot ran seeds ${JSON.stringify(seeds)}`);
  }
  if (manifest.pilot.elapsed_ticks !== ELAPSED_TICKS) {
    problems.push(`the pilot ran ${manifest.pilot.elapsed_ticks} ticks`);
  }
  if (manifest.rule.window_decisions !== WINDOW_DECISIONS
      || manifest.rule.refractory_ticks !== REFRACTORY_TICKS
      || manifest.rule.episode_expiry_ticks !== EPISODE_EXPIRY_TICKS
      || manifest.rule.quota_seconds !== QUOTA_SECONDS) {
    problems.push('the recorded rule is not the one this reduction checks');
  }
  return {manifest, problems};
}

export async function reduce(root) {
  const provenance = await verifyProvenance(root);
  const summary = await readJson(join(root, 'summary.json'));
  const arms = [];
  for (const seed of SEEDS) {
    for (const arm of ARMS) {
      arms.push(await verifyArm(root, seed, arm));
    }
  }
  const problems = [...provenance.problems, ...arms.flatMap(a => a.problems)];
  if (summary.complete_measurement !== true) {
    problems.push('the run does not claim a complete measurement');
  }
  return {
    kind: 'post-intake-opportunity-shadow-reduction',
    root,
    build: provenance.manifest.build,
    executable_sha256: provenance.manifest.executable_sha256,
    source: provenance.manifest.source,
    reduction_passed: problems.length === 0,
    problems,
    arms,
    screen: {
      thresholds: SCREEN,
      evaluated: false,
      why: 'root\'s usefulness screen needs at least nine of twelve no-care seeds and '
        + 'contributions from more than one exposed form. This pilot ran two seeds, so the '
        + 'screen is not evaluated here and no pass or fail is claimed for it.',
    },
    limits: [
      'A compatible window is the number of decisions the freely feeding baseline stayed '
      + 'compatible with holding. It is not a pause, not rest, not an upper bound on an '
      + 'intervention, and not measured missed food.',
      'Baseline intake during a window is the baseline\'s own meal, recorded separately. An '
      + 'intervention would have changed the patch, the neighbours\' shares, the position and '
      + 'every later stock in either direction.',
      'Nothing here says whether any interval would be readable at native 64 px.',
    ],
  };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const root = process.argv[2];
  assert(root, 'usage: node scripts/post-intake-shadow.mjs <pilot directory>');
  const report = await reduce(root);
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  if (!report.reduction_passed) process.exitCode = 1;
}
