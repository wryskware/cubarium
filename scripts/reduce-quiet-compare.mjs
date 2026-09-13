// Read-only reduction of a completed four-arm ordinary-quiet screen.
// Does not modify artifacts and does not infer ecological acceptance. It does run the run's own
// frozen executable in its read-only `--inspect` mode, which decodes a snapshot and prints what
// is inside it; nothing is stepped and nothing is written.
//
// The contract this checks is the proposal's own
// (design/7_Research/astra-ordinary-quiet-experiment-proposal-2026-09-13.md): twelve prescribed
// openings, four arms each, one Standard Feed at elapsed 600 in the two fed arms, and a rest
// classification that separates post_birth_recovery from natural satiated rest and from a
// newborn's own first tick.
//
// **Nothing an arm says about itself is taken as evidence for itself.** The audit limits are
// re-derived from the opening snapshot's own semantic inventories and the fixed 1e-8 rule; the
// horizon name is checked against its tick count; the cohort is compared against the cohort
// directory it came from; every closing snapshot is parsed, checksummed and decoded; every
// admission is matched to a paid birth and to exactly one close; and every census window,
// bout, record and life event is reconciled against the others. A self-reported flag is only
// ever checked *in addition*.
//
// What it refuses is as important as what it reports. An Off arm that admitted a pause, a
// recovery bout that outlived its window, a bout file that disagrees with its own summary, a
// census that skipped a window, a relabelled horizon, an unreceipted care total, a cohort that
// is not the prescribed twelve — each is a refusal, not a footnote. Nothing here decides
// viability, and nothing here turns a passing technical gate into evidence about behaviour.

import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {existsSync} from 'node:fs';
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
/// The named horizons and what each one *is*. A run whose tick count does not match its own
/// name is a relabelled dataset, not a shorter one.
export const HORIZON_TICKS = {
  smoke: 2400,
  'ten-minute': 12000,
  'two-hour': 144000,
  'twenty-four-hour': 1728000,
  'seventy-two-hour': 5184000,
};
export const PRESCRIBED = ['ten-minute', 'two-hour', 'twenty-four-hour', 'seventy-two-hour'];
export const CADENCE = 200;
export const OPENING_TICK = 144000;
export const OPENING_SCHEMA = 9;
/// The one controlled input, exactly.
export const FEED = {kind: 'feed', elapsed: 600, dose_permille: 1000,
  target: {face: 0, u: 32, v: 48}};
/// The unchanged paired-experiment audit rule: a fixed fraction of the OPENING inventory.
export const AUDIT_FRACTION = 1e-8;
export const QUIET_REASONS = ['unaffordable', 'unaffordable_remaining', 'parent_gone', 'bounded',
  'already_paused', 'hunter_member', 'invalid_inputs', 'overflow'];
export const BOUT_ENDS = ['released', 'aborted', 'woke', 'reclassified', 'died', 'censored'];
export const DEATH_CAUSES = ['starvation', 'age', 'collapse', 'predation'];
export const NO_CARE_LEDGER = {admitted_seq: 0, allowance_used: 0, clean_energy_out: 0,
  clean_material_out: 0, feed_energy_in: 0, feed_material_in: 0, rain_depth_in: 0, showers: []};

const json = async path => JSON.parse(await readFile(path, 'utf8'));
const jsonl = async path =>
  (await readFile(path, 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse);
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const safeCount = n => assert(Number.isSafeInteger(n) && n >= 0, `invalid count ${n}`);
const ratio = (n, d) => (d === 0 ? null : n / d);
const idKey = id => {
  assert(id && Number.isSafeInteger(id.slot) && Number.isSafeInteger(id.generation),
    'an identity without both a slot and a generation');
  assert(id.slot >= 0 && id.generation >= 0, `a negative identity ${id.slot}/${id.generation}`);
  return `${id.slot}/${id.generation}`;
};
const finiteAtLeastZero = (x, what) =>
  assert(Number.isFinite(x) && x >= 0, `${what} is not a finite nonnegative number: ${x}`);

/// The audit limits, from inventories rather than from an arm's claim about them. Identical
/// arithmetic to the harness's own `Baseline::read`, so the two agree exactly or disagree
/// meaningfully.
export function limitsFrom(inventories) {
  const of = x => {
    finiteAtLeastZero(x, 'an opening inventory');
    return AUDIT_FRACTION * Math.max(x, 1.0);
  };
  return {material: of(inventories.material), energy: of(inventories.energy),
    water: of(inventories.water)};
}

/// CRC-32 (IEEE), so a snapshot's payload is checked against its own header here rather than
/// against the number the decoder reports for it.
const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  return table;
})();

export function crc32(bytes) {
  let c = 0xffffffff;
  for (const b of bytes) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/// `[magic][schema u32][build_id_len u16][build_id][payload_len u64][crc32 u32][payload]`.
export function parseSnapshotHeader(bytes) {
  assert(bytes.length > 22, 'a snapshot shorter than its own fixed header');
  assert.equal(bytes.subarray(0, 4).toString('latin1'), 'CUBW', 'not a snapshot: bad magic');
  const schema = bytes.readUInt32LE(4);
  const idLength = bytes.readUInt16LE(8);
  assert(bytes.length >= 22 + idLength, 'a snapshot truncated inside its header');
  const build = bytes.subarray(10, 10 + idLength).toString('utf8');
  const payloadLength = Number(bytes.readBigUInt64LE(10 + idLength));
  const checksum = bytes.readUInt32LE(18 + idLength);
  const payload = bytes.subarray(22 + idLength);
  assert.equal(payload.length, payloadLength,
    'the payload length in the header is not the payload that follows it');
  assert.equal(crc32(payload), checksum, 'the payload does not match its own CRC32');
  return {schema, build, payloadLength, checksum, bytes: bytes.length};
}

/// Decode one snapshot with the run's **own frozen executable**, read-only. This is the only way
/// to see a payload's semantics from here: a header parse proves the bytes are a snapshot and
/// says nothing whatever about what is inside it.
export function inspect(executable, path) {
  let out;
  try {
    out = execFileSync(executable, ['--inspect', path], {maxBuffer: 64 << 20});
  } catch (error) {
    throw new Error(`the frozen executable could not inspect ${path}: ${error.message}`);
  }
  const value = JSON.parse(out.toString('utf8'));
  assert.equal(value.kind, 'quiet-compare-snapshot-inspection', 'not an inspection');
  return value;
}

/// Every technical gate an arm had to clear, checked here rather than believed from one flag.
///
/// `derived` carries what an independent read of the seed's own opening snapshot found. Without
/// it the limits are still re-derived from the arm's reported opening **inventories** and the
/// fixed rule — never from its reported limits — so a widened tolerance is refused either way;
/// with it, the inventories themselves are checked against the snapshot they claim to describe.
export function verifyArm(a, ticks, opening, arm, derived = null) {
  assert.equal(a.arm, arm, 'arm name mismatch');
  assert.equal(a.technical_complete, true, `${arm}: technically incomplete`);
  assert.equal(a.audit_passed, true, `${arm}: a technical gate failed`);
  assert.equal(a.planned_ticks, ticks);
  assert.equal(a.closing_tick, opening + ticks);
  assert.equal(a.elapsed_ticks, ticks);
  assert.equal(a.termination, 'planned_horizon');
  // The horizon is a name *and* a length, and the two have to be each other.
  assert(a.horizon in HORIZON_TICKS, `${arm}: unknown horizon ${a.horizon}`);
  assert.equal(HORIZON_TICKS[a.horizon], ticks,
    `${arm}: horizon ${a.horizon} is ${HORIZON_TICKS[a.horizon]} ticks, not ${ticks}`);
  // Each gate is separately true; `audit_passed` is never the only evidence.
  assert.equal(a.gates.conservation_and_flow_audits, true, `${arm}: conservation`);
  assert.equal(a.gates.records_reconcile_to_the_world, true, `${arm}: reconciliation`);
  assert.equal(a.gates.restart_proofs_complete_and_equal, true, `${arm}: restart proof`);
  // Raw and compensated energy stay separate numbers, and both have to hold. Neither is widened
  // and neither substitutes for the other.
  assert.equal(a.gates.legacy_raw_energy, true, `${arm}: raw energy`);

  // The restart proof, as a proof: complete, equal, inside the run, and genuinely mid-pause
  // wherever a pause was there to interrupt.
  assert(Array.isArray(a.restart_proofs) && a.restart_proofs.length > 0,
    `${arm}: no restart proof was recorded`);
  for (const p of a.restart_proofs) {
    assert(['mid_pause', 'fixed_boundary'].includes(p.trigger), `${arm}: unknown proof trigger`);
    assert.equal(p.failure, null, `${arm}: a restart proof failed: ${p.failure}`);
    assert.equal(p.complete, true, `${arm}: an incomplete restart proof`);
    safeCount(p.compared_ticks);
    assert.equal(p.compared_ticks, p.planned_window_ticks,
      `${arm}: a proof compared fewer ticks than it planned`);
    assert.equal(p.finish_tick, p.start_tick + p.compared_ticks, `${arm}: proof range`);
    assert(p.start_tick >= opening && p.finish_tick <= opening + ticks,
      `${arm}: a proof outside the run`);
    if (p.trigger === 'mid_pause')
      assert(p.open_pauses_carried_into_the_shadow > 0,
        `${arm}: a mid-pause proof carried no pause`);
    else
      assert.equal(p.open_pauses_carried_into_the_shadow, 0,
        `${arm}: a fallback proof claimed an open pause`);
  }
  if (a.a_pause_was_open_with_room_for_a_full_window)
    assert(a.restart_proofs.some(p => p.trigger === 'mid_pause'),
      `${arm}: a pause was open and no mid-pause restart was ever proved`);

  // --- the audit limits, derived rather than believed --------------------------------------
  const base = a.pre_intervention_baseline;
  const inventories = {material: base.material, energy: base.energy, water: base.water};
  if (derived) {
    for (const key of ['material', 'energy', 'water'])
      assert.equal(inventories[key], derived.inventories[key],
        `${arm}: the reported opening ${key} is not the opening snapshot's own inventory`);
  }
  const limits = limitsFrom(inventories);
  for (const key of ['material', 'energy', 'water']) {
    assert.equal(base.limits[key], limits[key],
      `${arm}: the reported ${key} limit is not ${AUDIT_FRACTION} of the opening inventory`);
    const drift = a.max_absolute_drift[key];
    assert(Number.isFinite(drift) && drift >= 0 && drift < limits[key],
      `${arm}: ${key} drift ${drift} against the derived limit ${limits[key]}`);
  }
  for (const key of ['corrected_energy_drift', 'independent_windowed_energy_drift',
    'care_boundary_energy_drift']) {
    const drift = a[key];
    assert(Number.isFinite(drift) && drift >= 0 && drift < limits.energy,
      `${arm}: ${key} ${drift} against the derived energy limit ${limits.energy}`);
  }

  // --- the rest classification ----------------------------------------------------------
  const rest = a.rest;
  assert.equal(rest.reconciled, true, `${arm}: the observer did not reconcile`);
  assert.equal(rest.violations, 0, `${arm}: retained violations`);
  assert.equal(rest.held_intake_ticks, 0, `${arm}: a held interval took intake`);
  for (const c of CLASSES) {
    safeCount(rest[c].organism_ticks);
    safeCount(rest[c].bouts);
    safeCount(rest[c].seam_ticks);
    finiteAtLeastZero(rest[c].transported_path_px, `${arm}: ${c} transported path`);
    assert(rest[c].seam_ticks <= rest[c].organism_ticks, `${arm}: ${c} seam ticks`);
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
  assert.equal(rest.organism_ticks, a.population_organism_ticks,
    `${arm}: classified organism-ticks are not the population's own`);
  // The whole world's transported path, every organism-tick, with classified rest inside it.
  finiteAtLeastZero(rest.transported_path_px, `${arm}: transported path`);
  safeCount(rest.seam_ticks);
  assert(rest.seam_ticks <= rest.organism_ticks, `${arm}: more seam ticks than organism-ticks`);
  const classifiedPath = CLASSES.reduce((n, c) => n + rest[c].transported_path_px, 0);
  assert(classifiedPath <= rest.transported_path_px + 1e-9,
    `${arm}: classified rest moved further than the whole world did`);
  const classifiedSeams = CLASSES.reduce((n, c) => n + rest[c].seam_ticks, 0);
  assert(classifiedSeams <= rest.seam_ticks, `${arm}: classified seam ticks exceed the world's`);

  const q = rest.quiet_records;
  safeCount(q.admissions); safeCount(q.releases); safeCount(q.unmatched_closes);
  safeCount(q.open_admissions_at_close); safeCount(q.deaths_on_final_held_interval);
  safeCount(q.held_intervals_ended_by_death);
  assert(q.deaths_on_final_held_interval <= q.held_intervals_ended_by_death,
    `${arm}: a death on the fortieth held interval that no bout covers`);
  const aborts = Object.entries(q.aborts).reduce((n, [reason, x]) => {
    assert(QUIET_REASONS.includes(reason), `${arm}: unknown abort reason ${reason}`);
    safeCount(x);
    return n + x;
  }, 0);
  const refusals = Object.entries(q.refusals).reduce((n, [reason, x]) => {
    assert(QUIET_REASONS.includes(reason), `${arm}: unknown refusal reason ${reason}`);
    safeCount(x);
    return n + x;
  }, 0);
  assert.equal(q.unmatched_closes, 0, `${arm}: a release or abort matched no admission`);
  assert.equal(q.releases + aborts + q.open_admissions_at_close, q.admissions,
    `${arm}: pauses began, ended and remained open in numbers that do not add up`);
  assert.equal(q.open_admissions_at_close, a.open_pauses_at_close,
    `${arm}: open admissions disagree with the persisted pause set`);
  assert(q.deaths_on_final_held_interval <= aborts, `${arm}: more final-interval deaths than aborts`);
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
  } else {
    // Every real birth produced exactly one offer, admitted or refused.
    assert.equal(q.admissions + refusals, a.births,
      `${arm}: ${q.admissions + refusals} quiet offers for ${a.births} births`);
  }

  // --- the one controlled input ---------------------------------------------------------
  const fed = arm.endsWith('_feed');
  const ledgers = a.care.ledgers;
  assert.equal(a.care.planned, fed);
  // Nothing in this recipe is ever rain or cleanup, in any arm.
  assert.equal(ledgers.rain_depth_in, 0, `${arm}: rain is not in this recipe`);
  assert.equal(ledgers.clean_material_out, 0, `${arm}: cleanup is not in this recipe`);
  assert.equal(ledgers.clean_energy_out, 0, `${arm}: cleanup is not in this recipe`);
  assert.deepEqual(ledgers.showers, [], `${arm}: a shower is not in this recipe`);
  if (fed) {
    assert.equal(a.care.receipts.length, 1, `${arm}: the recipe is exactly one Feed`);
    const r = a.care.receipts[0];
    assert.equal(r.elapsed, FEED.elapsed);
    assert.equal(r.kind, FEED.kind);
    assert.equal(r.dose_permille, FEED.dose_permille);
    assert.deepEqual(r.target, FEED.target);
    // The **absolute** tick, not just the elapsed one an arm chose to report.
    assert.equal(r.tick, opening + FEED.elapsed,
      `${arm}: the Feed was applied at tick ${r.tick}, not ${opening + FEED.elapsed}`);
    assert.equal(a.care.applied_at_tick, opening + FEED.elapsed,
      `${arm}: the recorded application tick is not the prescribed one`);
    assert.equal(r.receipt.tick, opening + FEED.elapsed, `${arm}: the receipt's own tick`);
    assert.equal(r.receipt.seq, 1, `${arm}: the Feed is the first and only care command`);
    assert.equal(r.outcome, 'applied',
      `${arm}: the one prescribed input was ${r.outcome}, not applied`);
    assert.equal(r.reason, null, `${arm}: an applied receipt with a refusal reason`);
    const applied = r.receipt.outcome && r.receipt.outcome.Applied;
    assert(applied, `${arm}: a receipt carrying no applied quantities`);
    for (const key of ['material_in', 'energy_in'])
      assert(Number.isFinite(applied[key]) && applied[key] > 0,
        `${arm}: the Feed booked no ${key}`);
    for (const key of ['material_out', 'energy_out', 'water_depth'])
      assert.equal(applied[key], 0, `${arm}: a Feed took ${key} out`);
    assert(Number.isSafeInteger(applied.cells) && applied.cells > 0, `${arm}: no cells fed`);
    assert.equal(applied.ends_tick, null, `${arm}: a Feed is instantaneous`);
    // The ledger is exactly the receipt: an unreceipted total is refused.
    assert.equal(ledgers.admitted_seq, 1, `${arm}: a second care command`);
    assert.equal(ledgers.feed_material_in, applied.material_in,
      `${arm}: the material ledger is not the receipt`);
    assert.equal(ledgers.feed_energy_in, applied.energy_in,
      `${arm}: the energy ledger is not the receipt`);
    assert.equal(ledgers.allowance_used, applied.material_in,
      `${arm}: the allowance drawn is not the material the receipt booked`);
  } else {
    assert.equal(a.care.receipts.length, 0, `${arm}: a no-care arm received an input`);
    assert.equal(a.care.applied_at_tick, null, `${arm}: a no-care arm applied something`);
    // Every field at zero, so an unreceipted total cannot hide in one of them.
    assert.deepEqual(ledgers, NO_CARE_LEDGER, `${arm}: a no-care arm carries a care ledger`);
  }

  assert(Array.isArray(a.unsupported_measurements) && a.unsupported_measurements.length > 0,
    `${arm}: unsupported measurements must be stated, not omitted`);
  assert(!a.unsupported_measurements.some(s => /transported path/.test(s)),
    `${arm}: transported path length is measured exactly now and must not be called unsupported`);
  return {aborts, refusals, limits};
}

/// The bout file against the summary it belongs to. Individual full-ID histories are the point:
/// a total that no line supports is not evidence.
export function reduceBouts(rows, summary, opening, closing) {
  const out = {byClass: {}, released: 0, aborted: {}, censored: 0, died: 0, woke: 0,
    reclassified: 0, screenBouts: 0, longestRecovery: 0, recoveryOrigins: new Set()};
  for (const c of CLASSES) out.byClass[c] = {bouts: 0, ticks: 0};
  // One organism's bouts never overlap, and a bout that says what came next has to be followed
  // by exactly that.
  const previous = new Map();
  for (const b of rows) {
    assert(CLASSES.includes(b.class), `unknown rest class ${b.class}`);
    assert(BOUT_ENDS.includes(b.end), `unknown bout end ${b.end}`);
    safeCount(b.ticks);
    assert(b.ticks >= 1, 'a bout of no ticks');
    assert(Number.isSafeInteger(b.start_tick) && Number.isSafeInteger(b.end_tick));
    assert(b.start_tick > opening && b.end_tick <= closing && b.start_tick <= b.end_tick,
      `bout outside the run: ${b.start_tick}..${b.end_tick}`);
    assert.equal(b.end_tick - b.start_tick + 1, b.ticks, 'bout length disagrees with its range');
    const key = idKey(b.id);
    const before = previous.get(key);
    if (before) {
      assert(before.end_tick < b.start_tick,
        `two bouts of ${key} overlap at ${b.start_tick}`);
      if (before.next_class !== null && before.next_class !== undefined) {
        assert.equal(b.start_tick, before.end_tick + 1,
          `${key}: a bout promised a next class and left a gap`);
        assert.equal(b.class, before.next_class,
          `${key}: a bout promised ${before.next_class} and was followed by ${b.class}`);
      }
    }
    assert(b.next_class === null || CLASSES.includes(b.next_class),
      `unknown next class ${b.next_class}`);
    previous.set(key, b);
    out.byClass[b.class].bouts++;
    out.byClass[b.class].ticks += b.ticks;
    if (b.class === 'post_birth_recovery') {
      assert(b.ticks <= WINDOW_TICKS, `a recovery bout of ${b.ticks} ticks outlived its window`);
      assert(b.origin_child && Number.isSafeInteger(b.origin_boundary),
        'a recovery bout without its originating birth');
      idKey(b.origin_child);
      assert.equal(b.start_tick, b.origin_boundary + 1,
        'a recovery bout does not start at B+1');
      const origin = `${idKey(b.id)}@${b.origin_boundary}`;
      assert(!out.recoveryOrigins.has(origin), 'two recovery bouts share one originating birth');
      out.recoveryOrigins.add(origin);
      out.longestRecovery = Math.max(out.longestRecovery, b.ticks);
      if (b.ticks >= SCREEN_TICKS) out.screenBouts++;
      // A recovery bout ends because the world released or abandoned the pause — or because the
      // horizon arrived. Anything else means a terminal record was lost.
      assert(['released', 'aborted', 'censored'].includes(b.end),
        `a recovery bout ended as ${b.end} rather than by the world's own record`);
      if (b.end === 'released') {
        assert.equal(b.ticks, WINDOW_TICKS, 'a released bout did not complete the window');
        assert.equal(b.end_tick, b.origin_boundary + WINDOW_TICKS);
        out.released++;
      }
      if (b.end === 'aborted') {
        assert(QUIET_REASONS.includes(b.end_detail), `an abort with reason ${b.end_detail}`);
        // A whole window is legitimate for exactly one reason: the organism died in the step
        // that completed its fortieth held interval, so the interval really was held.
        if (b.ticks === WINDOW_TICKS)
          assert.equal(b.end_detail, 'parent_gone',
            `an aborted bout completed the whole window for reason ${b.end_detail}`);
        else assert(b.ticks < WINDOW_TICKS, 'an aborted bout outlived the window');
        out.aborted[b.end_detail] = (out.aborted[b.end_detail] || 0) + 1;
      }
    } else {
      assert.equal(b.origin_child, null, `${b.class} must not name an originating birth`);
      assert.equal(b.origin_boundary, null, `${b.class} must not name an originating boundary`);
    }
    if (b.end === 'reclassified') {
      assert(CLASSES.includes(b.end_detail), `a reclassification into ${b.end_detail}`);
      assert.equal(b.end_detail, b.next_class, 'a reclassification that disagrees with itself');
      out.reclassified++;
    }
    if (b.end === 'censored') {
      assert.equal(b.end_tick, closing, 'a censored bout that did not reach the horizon');
      out.censored++;
    }
    if (b.end === 'died') out.died++;
    if (b.end === 'woke') out.woke++;
  }
  // The file and the summary must be the same measurement. A bout covers the interval its
  // organism lived through, so an interval that killed the organism is in the bout and not in
  // the world's own resting census of that tick — the difference is counted, not absorbed.
  const heldDeaths = summary.rest.quiet_records.held_intervals_ended_by_death;
  safeCount(heldDeaths);
  for (const c of CLASSES) {
    assert.equal(out.byClass[c].bouts, summary.rest[c].bouts, `${c}: bout count mismatch`);
    const expected = summary.rest[c].organism_ticks
      + (c === 'post_birth_recovery' ? heldDeaths : 0);
    assert.equal(out.byClass[c].ticks, expected,
      `${c}: bout ticks disagree with the classified organism-ticks`);
  }
  assert.equal(out.released, summary.rest.quiet_records.releases,
    'released bouts disagree with the release records');
  assert.equal(rows.length, summary.bouts_written, 'bout file length disagrees with the summary');
  assert(out.byClass.post_birth_recovery.bouts <= summary.rest.quiet_records.admissions,
    'more recovery bouts than admissions');
  return out;
}

/// The transient record stream against the same summary, reconciled as a lifecycle: every close
/// belongs to exactly one earlier admission, with the same child and a tick that follows its
/// boundary. Matching totals are not matching histories.
export function reduceQuietEvents(rows, summary, opening, closing) {
  const out = {begin: 0, refuse: 0, end: 0, abort: 0, refusals: {}, aborts: {},
    admitted: new Set(), offers: new Set(), finalIntervalDeaths: 0, open: 0};
  const live = new Map();
  for (const e of rows) {
    assert(['begin', 'refuse', 'end', 'abort'].includes(e.kind), `unknown quiet record ${e.kind}`);
    assert(Number.isSafeInteger(e.tick) && e.tick > opening && e.tick <= closing,
      `quiet record outside the run at ${e.tick}`);
    const parent = idKey(e.parent), child = idKey(e.child);
    assert(parent !== child, 'a record naming one organism as its own child');
    out[e.kind]++;
    if (e.kind !== 'end' && e.kind !== 'abort') out.offers.add(`${parent}@${e.tick}>${child}`);
    if (e.kind === 'begin') {
      assert.equal(e.end_tick, e.tick + WINDOW_TICKS,
        "an admission window is not the candidate's");
      assert(typeof e.underlying === 'string' && e.underlying.length > 0,
        'an admission with no carried underlying mode');
      assert(!live.has(parent), `${parent} was admitted again before its pause closed`);
      live.set(parent, {child, boundary: e.tick});
      out.admitted.add(`${parent}@${e.tick}`);
    }
    if (e.kind === 'refuse') {
      assert(QUIET_REASONS.includes(e.reason), `a refusal with reason ${e.reason}`);
      out.refusals[e.reason] = (out.refusals[e.reason] || 0) + 1;
    }
    if (e.kind === 'end' || e.kind === 'abort') {
      const open = live.get(parent);
      assert(open, `a ${e.kind} for ${parent} with no open admission`);
      assert.equal(open.child, child,
        `a ${e.kind} naming ${child} where its admission named ${open.child}`);
      assert.equal(e.tick, open.boundary + e.completed_ticks,
        `a ${e.kind} at ${e.tick} does not follow its boundary ${open.boundary}`);
      live.delete(parent);
    }
    if (e.kind === 'end') {
      assert.equal(e.completed_ticks, WINDOW_TICKS, 'a release did not complete the window');
      assert(typeof e.underlying === 'string' && e.underlying.length > 0,
        'a release with no underlying mode to resume');
    }
    if (e.kind === 'abort') {
      assert(QUIET_REASONS.includes(e.reason), `an abort with reason ${e.reason}`);
      safeCount(e.completed_ticks);
      if (e.completed_ticks === WINDOW_TICKS) {
        // The one legitimate whole-window abort: the parent died *during* its fortieth held
        // interval, so the interval really was held and there was no parent left to release.
        assert.equal(e.reason, 'parent_gone',
          `an abort completed the whole window for reason ${e.reason}`);
        out.finalIntervalDeaths++;
      } else {
        assert(e.completed_ticks < WINDOW_TICKS, 'an abort outlived the window');
      }
      out.aborts[e.reason] = (out.aborts[e.reason] || 0) + 1;
    }
  }
  const q = summary.rest.quiet_records;
  out.open = live.size;
  assert.equal(out.begin, q.admissions, 'admissions disagree with the record stream');
  assert.equal(out.end, q.releases, 'releases disagree with the record stream');
  assert.deepEqual(out.aborts, q.aborts, 'abort reasons disagree with the record stream');
  assert.deepEqual(out.refusals, q.refusals, 'refusal reasons disagree with the record stream');
  assert.equal(out.open, q.open_admissions_at_close,
    'pauses left open disagree with the summary');
  assert.equal(out.finalIntervalDeaths, q.deaths_on_final_held_interval,
    'whole-window aborts disagree with the summary');
  return out;
}

/// The life stream: every birth and death, by full generational identity, reconciled into one
/// population history. Counts are checked *because* the individuals behind them are.
export function reduceLife(rows, summary, opening, closing, openingIdentities) {
  const live = new Map(openingIdentities);
  const openingForms = new Set([...live.values()].map(o => o.form));
  const seen = new Set(live.keys());
  const out = {births: 0, deaths: {starvation: 0, age: 0, collapse: 0, predation: 0},
    birthsByParent: new Map(), formsSeen: new Set(openingForms),
    formsOnlyAfterOpening: new Set(), openingOrganismDeaths: 0, lastTick: opening};
  for (const r of rows) {
    assert(['birth', 'death'].includes(r.kind), `unknown life record ${r.kind}`);
    assert(Number.isSafeInteger(r.tick) && r.tick > opening && r.tick <= closing,
      `life record outside the run at ${r.tick}`);
    assert(r.tick >= out.lastTick, 'the life stream is out of order');
    out.lastTick = r.tick;
    const key = idKey(r.id);
    if (r.kind === 'birth') {
      const parent = idKey(r.parent);
      assert(!seen.has(key), `a birth reused the identity ${key}`);
      assert(live.has(parent), `a birth from ${parent}, who is not on the books`);
      assert(Number.isSafeInteger(r.form) && r.form >= 0 && r.form < 8, `a birth with form ${r.form}`);
      assert(Number.isSafeInteger(r.descendant_depth) && r.descendant_depth >= 1,
        'a birth at no generational depth');
      assert.equal(r.descendant_depth, live.get(parent).depth + 1, 'a generation was skipped');
      assert.equal(r.opening_cohort === null, false, 'a birth with no opening ancestor');
      assert.equal(idKey(r.opening_cohort), live.get(parent).cohort,
        'a child descends from a different opening organism than its parent');
      for (const k of ['structure', 'reserve', 'energy'])
        finiteAtLeastZero(r[k], `a newborn's ${k}`);
      seen.add(key);
      live.set(key, {form: r.form, born: r.tick, depth: r.descendant_depth,
        cohort: live.get(parent).cohort});
      out.births++;
      out.birthsByParent.set(`${parent}@${r.tick}`, key);
      out.formsSeen.add(r.form);
      if (!openingForms.has(r.form)) out.formsOnlyAfterOpening.add(r.form);
    } else {
      const was = live.get(key);
      assert(was, `a death of ${key}, who is not alive`);
      const cause = String(r.cause).toLowerCase();
      assert(DEATH_CAUSES.includes(cause), `a death by ${r.cause}`);
      assert.equal(r.form, was.form, 'a death reporting a form the organism did not live as');
      assert.equal(r.born_tick, was.born, 'a death reporting another birth tick');
      assert.equal(r.age_ticks, r.tick - was.born, 'a death whose age is not its own lifetime');
      if (was.depth === 0) out.openingOrganismDeaths++;
      assert.equal(r.was_an_opening_organism, was.depth === 0, 'a mislabelled opening organism');
      live.delete(key);
      out.deaths[cause]++;
    }
  }
  assert.equal(out.births, summary.births, 'life records disagree with the reported births');
  for (const cause of DEATH_CAUSES)
    assert.equal(out.deaths[cause], summary.deaths[cause],
      `life records disagree with the reported ${cause} deaths`);
  assert.equal(rows.length, summary.life_records_written, 'the life file is not the one reported');
  assert.equal(live.size, summary.closing_population,
    'the survivors of the life stream are not the closing population');
  assert.equal(live.size, summary.living_lineages_at_close, 'living lineages disagree');
  const cohorts = new Set([...live.values()].map(o => o.cohort));
  assert.equal(cohorts.size, summary.surviving_opening_cohorts,
    'surviving opening cohorts disagree with the individuals behind them');
  const depth = [...live.values()].reduce((n, o) => Math.max(n, o.depth), 0);
  assert(depth <= summary.maximum_descendant_depth, 'a living lineage deeper than reported');
  out.survivors = live.size;
  out.survivingCohorts = cohorts.size;
  out.formsSeen = [...out.formsSeen].sort();
  out.formsOnlyAfterOpening = [...out.formsOnlyAfterOpening].sort();
  return out;
}

/// Every census window, in order, covering every interior tick of the run — not just the last.
export function reduceCensus(rows, summary, opening, ticks, cadence) {
  assert.equal(rows.length, ticks / cadence, 'census windows missing');
  const flow = {births: 0, starvation: 0, age: 0, collapse: 0};
  rows.forEach((c, i) => {
    assert.equal(c.tick, opening + (i + 1) * cadence, `census window ${i} is out of order`);
    assert.equal(c.elapsed, (i + 1) * cadence, `census window ${i} elapsed`);
    safeCount(c.population); safeCount(c.occupied_cells); safeCount(c.open_pauses);
    safeCount(c.escrows); safeCount(c.surviving_opening_cohorts);
    safeCount(c.maximum_descendant_depth);
    assert(Array.isArray(c.population_by_form) && c.population_by_form.length === 8,
      'a census without the full form vector');
    c.population_by_form.forEach(safeCount);
    assert.equal(c.population_by_form.reduce((a, b) => a + b, 0), c.population,
      'population by form does not sum to the population');
    assert.equal(c.mode.resting + c.mode.seeking + c.mode.feeding, c.population,
      'mode counts do not cover the population');
    assert(c.surviving_opening_cohorts <= c.population, 'more cohorts than organisms');
    for (const key of ['producer', 'fruit', 'detritus', 'nutrient', 'water', 'organism_material',
      'organism_energy', 'window_light_in', 'window_heat_out'])
      finiteAtLeastZero(c[key], `census ${key} at ${c.tick}`);
    for (const key of ['state_hash', 'ecology_hash'])
      assert(/^[0-9]+$/.test(c[key]), `census ${key} at ${c.tick} is not a hash`);
    safeCount(c.window_births);
    flow.births += c.window_births;
    for (const cause of ['starvation', 'age', 'collapse']) {
      safeCount(c.window_deaths[cause]);
      flow[cause] += c.window_deaths[cause];
    }
  });
  const last = rows[rows.length - 1];
  assert.equal(last.tick, opening + ticks, 'the census does not reach the horizon');
  assert.equal(last.population, summary.closing_population, 'the census closes on another population');
  assert.equal(last.open_pauses, summary.open_pauses_at_close, 'closing open pauses disagree');
  assert.equal(last.state_hash, summary.closing_state_hash, 'the closing state hash disagrees');
  assert.equal(last.ecology_hash, summary.closing_ecology_hash, 'the closing ecology hash disagrees');
  assert.equal(last.surviving_opening_cohorts, summary.surviving_opening_cohorts,
    'surviving cohorts disagree with the census');
  assert.equal(last.maximum_descendant_depth, summary.maximum_descendant_depth,
    'descendant depth disagrees with the census');
  // The windowed population flow is the whole run's, not a sample of it.
  assert.equal(flow.births, summary.births, 'census window births do not sum to the run');
  for (const cause of ['starvation', 'age', 'collapse'])
    assert.equal(flow[cause], summary.deaths[cause],
      `census window ${cause} deaths do not sum to the run`);
  assert.equal(summary.deaths.predation, 0, 'this family has no hunters and no predation');
  return flow;
}

/// One arm's closing snapshot: the bytes, their own header and CRC, their checksum, and then
/// what is actually inside them.
export function verifyClosingSnapshot(bytes, inspection, summary, manifest, arm) {
  const header = parseSnapshotHeader(bytes);
  assert.equal(sha256(bytes), summary.closing_snapshot_sha256,
    `${arm}: the closing snapshot is not the one the summary checksummed`);
  assert.equal(header.schema, inspection.header.schema, `${arm}: schema disagrees`);
  assert.equal(header.schema, inspection.header.current_schema,
    `${arm}: a closing snapshot at another schema than the build writes`);
  assert.equal(header.build, manifest.build, `${arm}: written by another build`);
  assert.equal(header.payloadLength, inspection.header.payload_len, `${arm}: payload length`);
  assert.equal(header.checksum, inspection.header.crc32, `${arm}: CRC disagrees`);
  assert.equal(inspection.sha256, summary.closing_snapshot_sha256,
    `${arm}: the inspection is of another file than the one checksummed here`);
  // And the semantics, which no header parse can see.
  assert.equal(inspection.tick, summary.closing_tick, `${arm}: the snapshot closes at another tick`);
  assert.equal(inspection.state_hash, summary.closing_state_hash, `${arm}: state hash`);
  assert.equal(inspection.ecology_hash, summary.closing_ecology_hash, `${arm}: ecology hash`);
  assert.equal(inspection.population, summary.closing_population, `${arm}: population`);
  assert.equal(inspection.quiet.policy, summary.quiet_policy, `${arm}: persisted policy`);
  assert.equal(inspection.quiet.open_pauses, summary.open_pauses_at_close, `${arm}: open pauses`);
  assert.deepEqual(inspection.care, summary.care.ledgers, `${arm}: persisted care ledgers`);
  assert.equal(inspection.hunters_present, false, `${arm}: a hunter extension`);
  for (const key of Object.keys(inspection.inventories))
    finiteAtLeastZero(inspection.inventories[key], `${arm}: closing ${key}`);
  for (const pause of inspection.quiet.pauses) {
    idKey(pause.parent); idKey(pause.child);
    assert.equal(pause.end_tick, pause.start_tick + WINDOW_TICKS, `${arm}: a persisted pause window`);
    assert(pause.start_tick <= inspection.tick && inspection.tick < pause.end_tick,
      `${arm}: a persisted pause that is not open at the closing tick`);
  }
  return header;
}

/// What a seed's opening **actually is**, read from the cohort's own bytes and decoded: the
/// source-frozen fingerprint every arm of that seed is then held to. A baseline an arm reports
/// about itself, copied four times, is not an independent check of anything.
export function verifySeedOpening(openingBytes, inspected, row, seed, build, openingTick) {
  assert.equal(sha256(openingBytes), row.sha256, `seed ${seed}: the opening changed`);
  const header = parseSnapshotHeader(openingBytes);
  assert.equal(header.schema, OPENING_SCHEMA, `seed ${seed}: opening schema`);
  assert.equal(inspected.sha256, row.sha256, `seed ${seed}: another file was inspected`);
  assert.equal(inspected.inspector_build, build,
    'the frozen executable is not the build that wrote this run');
  assert.equal(inspected.tick, openingTick, `seed ${seed}: the opening tick`);
  assert.equal(inspected.seed, seed, `seed ${seed}: the opening carries another seed`);
  assert.equal(inspected.population, row.population, `seed ${seed}: opening population`);
  assert.equal(inspected.ecology_hash, row.ecology_hash, `seed ${seed}: opening ecology hash`);
  assert.equal(inspected.quiet.policy, 'off', `seed ${seed}: the opening already had a policy`);
  assert.equal(inspected.quiet.open_pauses, 0, `seed ${seed}: the opening already had a pause`);
  assert.deepEqual(inspected.care, NO_CARE_LEDGER, `seed ${seed}: the opening had care`);
  assert.equal(inspected.hunters_present, false, `seed ${seed}: the opening had hunters`);
  const derived = {inventories: inspected.inventories, limits: limitsFrom(inspected.inventories),
    state_hash: inspected.state_hash, ecology_hash: inspected.ecology_hash,
    config_sha256: inspected.config_sha256, population: inspected.population};
  assert.deepEqual(derived.limits, {material: inspected.audit_limits.material,
    energy: inspected.audit_limits.energy, water: inspected.audit_limits.water},
  `seed ${seed}: the derived limits disagree with the inspector's own`);
  return derived;
}

/// One arm's own opening: the same world as the cohort's, the same config, and a policy that is
/// the only thing about it that differs.
export function verifyArmOpening(openingJson, derived, name, seed, openingTick) {
  const candidate = name.startsWith('candidate');
  assert.equal(openingJson.arm, name);
  assert.equal(openingJson.candidate, candidate);
  assert.equal(openingJson.opening_tick, openingTick);
  assert.equal(openingJson.state_hash_before_choice, derived.state_hash,
    `${name}: the arm did not start from this seed's opening`);
  assert.equal(openingJson.ecology_hash, derived.ecology_hash, `${name}: ecology hash`);
  assert.equal(openingJson.config_sha256, derived.config_sha256, `${name}: config identity`);
  assert.equal(openingJson.config.seed, seed, `${name}: config seed`);
  assert.deepEqual(openingJson.quiet.pauses, [], `${name}: an opening pause`);
  assert.equal(openingJson.quiet_policy, candidate ? 'post_birth_pause_v1' : 'off');
  if (candidate)
    assert.notEqual(openingJson.state_hash_after_choice, derived.state_hash,
      `${name}: the candidate choice left the state identical`);
  else
    assert.equal(openingJson.state_hash_after_choice, derived.state_hash,
      `${name}: an Off arm changed the opening`);
  assert.deepEqual(openingJson.care, name.endsWith('_feed')
    ? {kind: FEED.kind, dose_permille: FEED.dose_permille, elapsed_tick: FEED.elapsed,
      target: FEED.target, count: 1}
    : null, `${name}: the recorded recipe is not the prescribed one`);
}

/// The opening identity census: who was actually here, by full generational identity.
export function readOpeningIdentities(text, population, name, openingTick) {
  const rows = text.trim().split('\n').filter(Boolean).map(JSON.parse);
  assert.equal(rows.length, population,
    `${name}: the opening census is not the opening population`);
  const identities = new Map();
  for (const o of rows) {
    const key = idKey(o.id);
    assert(!identities.has(key), `${name}: a duplicate opening identity`);
    assert(Number.isSafeInteger(o.form) && o.form >= 0 && o.form < 8, `${name}: opening form`);
    assert(Number.isSafeInteger(o.born_tick) && o.born_tick <= openingTick,
      `${name}: an opening organism born after the opening`);
    assert.equal(o.age_ticks, openingTick - o.born_tick, `${name}: opening age`);
    for (const k of ['structure', 'reserve', 'energy', 'material'])
      finiteAtLeastZero(o[k], `${name}: opening ${k}`);
    identities.set(key, {form: o.form, born: o.born_tick, depth: 0, cohort: key});
  }
  return identities;
}

export async function loadRun(directory, options = {}) {
  const root = resolve(directory);
  const m = await json(join(root, 'manifest.json'));
  const summary = await json(join(root, 'summary.json')); // Missing means still running.
  assert.equal(m.kind, 'four-arm-ordinary-quiet-comparison');
  assert.deepEqual(m.arms, ARMS, 'the four prescribed arms are required, in order');
  assert.equal(m.sample_every, CADENCE, 'the prescribed census cadence is 200 ticks');
  // The horizon is checked by name against its own length, in the manifest and in every arm.
  assert(PRESCRIBED.includes(m.horizon), `${m.horizon} is not a prescribed horizon`);
  assert.equal(HORIZON_TICKS[m.horizon], m.ticks,
    `horizon ${m.horizon} is ${HORIZON_TICKS[m.horizon]} ticks, not ${m.ticks}`);
  assert.equal(m.ticks % m.sample_every, 0, 'the horizon is not a whole number of windows');
  // The factor definition itself: one policy field, two levels, and exactly one Feed.
  assert.equal(m.factors.policy.field, 'WorldState.quiet');
  assert.deepEqual(m.factors.policy.levels, ['off', 'post_birth_pause_v1']);
  assert.deepEqual(m.factors.care.levels, [null, 'one Standard Feed']);
  assert.equal(m.factors.care.recipe.count, 1);
  assert.equal(m.factors.care.recipe.kind, FEED.kind);
  assert.equal(m.factors.care.recipe.elapsed_tick, FEED.elapsed);
  assert.equal(m.factors.care.recipe.dose_permille, FEED.dose_permille);
  assert.deepEqual(m.factors.care.recipe.target, FEED.target);

  const executable = join(root, 'quiet_compare.frozen');
  const executableBytes = await readFile(executable);
  assert.equal(sha256(executableBytes), m.executable_sha256,
    'the frozen executable does not match its manifest');
  assert.equal(summary.technical_complete, true);
  assert.equal(summary.audit_passed, true);
  assert.equal(summary.complete_experiment_measurement, true);
  assert.deepEqual(summary.seeds.map(s => s.seed).sort((a, b) => a - b),
    Array.from({length: 12}, (_, i) => i + 1), 'the prescribed twelve seeds are required');

  // The cohort, from the directory it came from rather than from this run's copy of it. The
  // recorded path is the one the run was given, so it is tried as written, against this process
  // and against the run itself; an explicit argument always wins.
  const recorded = [options.cohort, m.cohort_source].filter(Boolean);
  assert(recorded.length > 0, 'the run does not record the cohort it came from');
  const cohortRoot = recorded
    .flatMap(p => [resolve(p), resolve(root, p), resolve(root, '..', p)])
    .find(p => existsSync(join(p, 'manifest.json')));
  assert(cohortRoot,
    `the cohort this run came from (${recorded[0]}) is not reachable; pass it as the second argument`);
  const cohort = await json(join(cohortRoot, 'manifest.json'));
  assert.deepEqual(cohort, m.cohort, 'the recorded cohort is not the cohort on disk');
  assert.equal(cohort.complete, true);
  assert.equal(cohort.kind, 'pre-hunter-cohort-preparation');
  assert.equal(cohort.opening_tick, OPENING_TICK);
  assert.equal(cohort.openings.length, 12);

  const opening = cohort.opening_tick;
  const closing = opening + m.ticks;
  const seeds = [];
  for (const s of summary.seeds) {
    const dir = join(root, `seed-${s.seed}`);
    assert.deepEqual(await json(join(dir, 'result.json')), s);
    assert.equal(s.failure, null, `seed ${s.seed}: retained failure`);
    assert.equal(s.arms.length, ARMS.length);

    // What this seed's opening actually is, read from the cohort's own bytes.
    const row = cohort.openings.find(o => o.seed === s.seed);
    assert(row, `seed ${s.seed} is not in the cohort manifest`);
    const openingPath = join(cohortRoot, `seed-${s.seed}`, 'world-144000.cubw');
    const openingBytes = await readFile(openingPath);
    const derived = verifySeedOpening(openingBytes, inspect(executable, openingPath), row,
      s.seed, m.build, opening);
    // One shared pre-intervention baseline per seed, and it is the opening's own inventory.
    for (const key of ['material', 'energy', 'water'])
      assert.equal(s.pre_intervention_baseline[key], derived.inventories[key],
        `seed ${s.seed}: the shared baseline ${key} is not the opening's`);

    const arms = {};
    let openingCensusDigest = null;
    for (const [i, name] of ARMS.entries()) {
      const path = join(dir, name), a = s.arms[i];
      const openingJson = await json(join(path, 'opening.json'));
      verifyArmOpening(openingJson, derived, name, s.seed, opening);
      assert.deepEqual(openingJson.pre_intervention_baseline, s.pre_intervention_baseline,
        `${name}: a different baseline`);

      assert.deepEqual(await json(join(path, 'summary.json')), a);
      verifyArm(a, m.ticks, opening, name, derived);

      // The opening identity census: the same individuals for all four arms of a seed.
      const openingCensusText = await readFile(join(path, 'opening-organisms.jsonl'), 'utf8');
      const digest = sha256(openingCensusText);
      if (openingCensusDigest === null) openingCensusDigest = digest;
      assert.equal(digest, openingCensusDigest,
        `${name}: a different opening population than its siblings`);
      const openingIdentities =
        readOpeningIdentities(openingCensusText, derived.population, name, opening);

      const bouts = reduceBouts(await jsonl(join(path, 'bouts.jsonl')), a, opening, closing);
      const events = reduceQuietEvents(await jsonl(join(path, 'quiet-events.jsonl')), a,
        opening, closing);
      const life = reduceLife(await jsonl(join(path, 'life.jsonl')), a, opening, closing,
        openingIdentities);
      const census = await jsonl(join(path, 'census.jsonl'));
      reduceCensus(census, a, opening, m.ticks, m.sample_every);

      // Cross-file: an admission is an offer made by a real paid birth, and a recovery bout
      // belongs to an admission. Identity and boundary, not counts.
      for (const offer of events.offers)
        assert(life.birthsByParent.has(offer.split('>')[0]),
          `${name}: a quiet offer at ${offer} with no paid birth behind it`);
      for (const origin of bouts.recoveryOrigins)
        assert(events.admitted.has(origin),
          `${name}: a recovery bout at ${origin} with no admission behind it`);
      assert.equal(bouts.byClass.newborn_initial.bouts, life.births,
        `${name}: newborn bouts are not the births`);

      const closingBytes = await readFile(join(path, 'closing.cubw'));
      const inspectedClosing = inspect(executable, join(path, 'closing.cubw'));
      verifyClosingSnapshot(closingBytes, inspectedClosing, a, m, name);
      assert.equal(inspectedClosing.seed, s.seed, `${name}: the closing snapshot's seed`);
      assert.equal(inspectedClosing.config_sha256, derived.config_sha256,
        `${name}: the config changed during the run`);

      arms[name] = {opening: openingJson, summary: a, bouts, events, life, census,
        closing: inspectedClosing};
    }
    seeds.push({seed: s.seed, arms, derived});
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
    forms_seen: a.life.formsSeen,
    opening_organism_deaths: a.life.openingOrganismDeaths,
    rest: Object.fromEntries(CLASSES.map(c => [c, a.summary.rest[c]])),
    intake_ticks: a.summary.rest.intake_ticks,
    mode_ticks: a.summary.rest.mode_ticks,
    quiet: a.summary.rest.quiet_records,
    transported_path_px: a.summary.rest.transported_path_px,
    transported_path_px_resting: CLASSES.reduce(
      (n, c) => n + a.summary.rest[c].transported_path_px, 0),
    seam_ticks: a.summary.rest.seam_ticks,
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
      transported_path_px: b.transported_path_px - a.transported_path_px,
    },
    // Losses the candidate has and its matched reference does not. The proposal asks for every
    // paired loss to be inspected, not a favourable pooled mean.
    paired_losses: {
      new_extinction: a.first_extinction_tick === null && b.first_extinction_tick !== null,
      lost_forms: Math.max(0, a.forms_present - b.forms_present),
      lost_opening_cohorts: Math.max(0, a.surviving_opening_cohorts - b.surviving_opening_cohorts),
      fewer_births: Math.max(0, a.births - b.births),
      more_opening_organism_deaths:
        Math.max(0, b.opening_organism_deaths - a.opening_organism_deaths),
    },
  };
}

export async function compare(directory, options = {}) {
  const run = await loadRun(directory, options);
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
      deaths_on_the_final_held_interval: a.events.finalIntervalDeaths,
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
    limits: 'Checks artifact integrity against the cohort it came from, gate-by-gate technical completion with limits re-derived from the opening snapshot rather than believed, the exact care recipe and its receipt, the rest classification partition, the lifecycle of every admission and bout, every census window, the whole life stream by identity, and each closing snapshot down to its CRC and its decoded semantics. It does not re-run core audits and it does not establish causality. Recovery classification and transported path length are exact. No funding or oxidation amount here is inferred from a post-step delta. Twelve seeds cannot prove universal noninferiority.',
    root: run.root,
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    const [, , directory, cohort] = process.argv;
    assert(directory && process.argv.length <= 4,
      'Usage: node scripts/reduce-quiet-compare.mjs RUN_DIR [COHORT_DIR]');
    console.log(JSON.stringify(await compare(directory, {cohort}), null, 2));
  } catch (error) {
    console.error(`Reduction refused: ${error.message}`);
    process.exitCode = 1;
  }
}
