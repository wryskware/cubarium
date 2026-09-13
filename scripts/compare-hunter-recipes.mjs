// Read-only reduction of completed matched hunter-recipe screens.
// Does not run an executable, modify artifacts, or infer ecological acceptance.
//
// Two contracts live here, and they are deliberately separate rather than one permissive check:
//
//   compare()        baseline            vs reserve-targets-v1            (schema 11 artifacts)
//   compareCharge()  reserve-targets-v1  vs reserve-targets-charge80-v1   (schema 12 artifacts)
//
// The first describes a terminal, immutable study; it must keep verifying exactly what it
// always verified, including that study's snapshot schema. The second is a different pair with
// a different expected profile diff, so it gets its own pair check rather than an ignore-list
// bolted onto the first.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {resolve, join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {inspectSnapshot} from './prepare-hunter-worlds.mjs';

export const ARMS = ['untouched', 'budget_control', 'specialist_off', 'specialist_on', 'facultative_off', 'facultative_on'];
const json = async path => JSON.parse(await readFile(path, 'utf8'));
const jsonl = async path => (await readFile(path, 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse);
const sum = values => values.reduce((a, b) => a + b, 0);
const count = (o, key) => {o[key] = (o[key] || 0) + 1;};
const safeCount = n => assert(Number.isSafeInteger(n) && n >= 0, `invalid count ${n}`);
const ratio = (n, d) => d === 0 ? null : n / d;

export function verifyArm(a, ticks, opening) {
  assert.equal(a.technical_complete, true, 'arm technically incomplete');
  assert.equal(a.complete_experiment_measurement, true, 'arm measurement incomplete');
  assert.equal(a.planned_ticks, ticks);
  assert.equal(a.closing_tick, opening + ticks);
  assert.equal(a.last_complete_observer_tick, a.closing_tick);
  assert.equal(a.last_complete_reproduction_tick, a.closing_tick);
  assert.equal(a.observer_statistics_trusted_through_closing_tick, true);
  assert.equal(a.audit.passed, true);
  assert.equal(a.audit.opening_tick, opening);
  for (const [key, index] of [['material', 0], ['corrected_energy', 1], ['independent_energy', 1], ['water', 2]]) {
    const peak = a.audit[key], limit = a.audit.fixed_limits[index];
    assert(Number.isFinite(limit) && limit > 0);
    assert(Number.isFinite(peak.magnitude) && peak.magnitude >= 0 && peak.magnitude < limit, `${key} reaches/exceeds limit`);
    assert.equal(peak.first_crossing, null);
    assert.equal(peak.nonfinite_at, null);
  }
  a.adult_occupancy_ticks_0_1_2_over2.forEach(safeCount);
  assert.equal(a.adult_occupancy_ticks_0_1_2_over2.length, 4);
  assert.equal(sum(a.adult_occupancy_ticks_0_1_2_over2), ticks, 'adult occupancy denominator');
  const r = a.reproduction_audit;
  assert.equal(r.last_complete_tick, a.closing_tick);
  assert.equal(r.ticks_observed, ticks);
  Object.values(r.counts).forEach(safeCount);
  assert.equal(r.counts.funded, r.counts.closed + r.counts.open_at_horizon);
  assert.equal(r.counts.closed, r.counts.born + r.counts.refunded + r.counts.miscarried);
  assert.equal(r.counts.born, a.offspring);
  assert.equal(r.counts.open_at_horizon, a.open_gestations);
  const q = a.reproductive_opportunity;
  const [members, mature, reserve, energy, both] = [q.member_ticks, q.age_and_size_ready_member_ticks,
    q.age_and_size_ready_reserve_gate_open_member_ticks, q.age_and_size_ready_energy_gate_open_member_ticks,
    q.age_and_size_ready_both_stock_gates_open_member_ticks];
  [members, mature, reserve, energy, both].forEach(safeCount);
  assert(mature <= members && reserve <= mature && energy <= mature
    && both <= Math.min(reserve, energy) && both >= Math.max(0, reserve + energy - mature), 'stock denominators');
  assert.equal(a.whole_recovery.length, 9);
}

export function reduceEvents(rows, opening, closing) {
  const out = {attempts: {}, paid_energy: 0, captures: 0, captured_material: 0, captured_energy: 0,
    offspring: 0, observed_adult_descendants: 0, reproduction: {}, deaths: {}, near_out_of_reach: 0, far_out_of_reach: 0};
  let lastTick = opening;
  for (const row of rows) {
    assert(['life', 'hunter', 'observed_maturity'].includes(row.stream), 'unknown event stream');
    const e = row.stream === 'observed_maturity' ? row : row.event;
    assert(Number.isSafeInteger(e.tick) && e.tick > opening && e.tick <= closing && e.tick >= lastTick, 'event coverage/order');
    lastTick = e.tick;
    if (row.stream === 'observed_maturity') {
      safeCount(e.depth); if (e.depth > 0) out.observed_adult_descendants++;
    }
    if (row.stream !== 'hunter') continue;
    switch (e.kind) {
      case 'attempt':
        assert(['Captured','Missed','OutOfReach','TargetLost','TargetClaimed','Ineligible','GraspUnmapped','Unaffordable'].includes(e.outcome));
        count(out.attempts, e.outcome);
        assert(Number.isFinite(e.energy_paid) && e.energy_paid >= 0);
        out.paid_energy += e.energy_paid;
        if (e.outcome === 'OutOfReach') {
          assert(e.evidence?.measure && e.evidence.geometry, 'missing out-of-reach evidence');
          const bodyX = e.evidence.measure.body?.x, clawX = e.evidence.geometry.capture_offset_body?.x;
          assert(Number.isFinite(bodyX) && Number.isFinite(clawX), 'invalid out-of-reach axial coordinates');
          count(out, bodyX < clawX
            ? 'near_out_of_reach' : 'far_out_of_reach');
        }
        break;
      case 'capture':
        assert(Number.isFinite(e.material) && e.material >= 0 && Number.isFinite(e.energy) && e.energy >= 0);
        out.captures++; out.captured_material += e.material; out.captured_energy += e.energy; break;
      case 'offspring': out.offspring++; break;
      case 'reproduction':
        assert(['funded','born','refunded','miscarried','not_funded'].includes(e.record.transaction));
        count(out.reproduction, e.record.transaction); break;
      case 'death': count(out.deaths, e.cause); break;
      default: throw new Error(`unknown hunter event ${e.kind}`);
    }
  }
  assert.equal(out.captures, out.attempts.Captured || 0, 'capture/attempt count mismatch');
  assert.equal(out.offspring, out.reproduction.born || 0, 'offspring/born count mismatch');
  return out;
}

export function verifyRecipePair(a, b) {
  assert.equal(a.profile_recipe, 'baseline'); assert.equal(b.profile_recipe, 'reserve-targets-v1');
  for (const key of ['arm', 'config', 'heading', 'target', 'pre_import_inventory', 'post_import_inventory', 'receipt'])
    assert.deepEqual(a[key], b[key], `opening ${key} changed`);
  if (a.profile === null) {assert.equal(b.profile, null); return;}
  const expected = structuredClone(a.profile);
  assert.equal(expected.seek_reserve_fraction, 0.35);
  assert.equal(expected.perch_reserve_fraction, 0.65);
  expected.seek_reserve_fraction = 0.80; expected.perch_reserve_fraction = 0.90;
  assert.deepEqual(b.profile, expected, 'candidate changed another profile field');
}

export async function loadRun(directory, recipe, schema = 11) {
  const root = resolve(directory), m = await json(join(root, 'manifest.json'));
  const summary = await json(join(root, 'summary.json')); // Missing means still running, never partial success.
  assert.equal(m.kind, 'six-arm-hunter-comparison');
  assert.equal(m.profile_recipe, recipe); assert.equal(m.care, false);
  assert.match(m.observer_contract, /^exact-reproduction-v2;/);
  assert(Number.isSafeInteger(m.ticks) && m.ticks >= 144000);
  assert.deepEqual(m.arms, ARMS);
  assert.equal(summary.technical_complete, true); assert.equal(summary.complete_experiment_measurement, true);
  assert.deepEqual(summary.seeds.map(s => s.seed).sort((a,b) => a-b), Array.from({length:12}, (_,i) => i+1));
  assert.equal(createHash('sha256').update(await readFile(join(root, 'hunter_compare.frozen'))).digest('hex'), m.executable_sha256);
  const seeds = [];
  for (const s of summary.seeds) {
    const dir = join(root, `seed-${s.seed}`);
    assert.deepEqual(await json(join(dir, 'result.json')), s);
    assert.equal(s.failure, null); assert.equal(s.arms.length, 6);
    assert.equal(s.local_recovery.technical_complete, true);
    assert.equal(s.local_recovery.last_complete_paired_tick, m.cohort.opening_tick + m.ticks);
    const localRows = await jsonl(join(dir, 'local-recovery.jsonl'));
    const arms = [];
    for (const [i, name] of ARMS.entries()) {
      const path = join(dir, name), a = s.arms[i], opening = await json(join(path, 'opening.json'));
      assert.equal(opening.arm, name); assert.equal(opening.profile_recipe, recipe);
      assert.deepEqual(await json(join(path, 'summary.json')), a);
      verifyArm(a, m.ticks, m.cohort.opening_tick);
      for (const [file, sha, state] of [['post-initialization.cubw', opening.post_snapshot_sha256, opening.post_state_hash],
        ['closing.cubw', a.closing_snapshot_sha256, a.closing_state_hash]]) {
        const snapshot = inspectSnapshot(await readFile(join(path, file)), schema);
        assert.equal(snapshot.sha256, sha); assert.equal(snapshot.state_hash, state); assert.equal(snapshot.build, m.build);
      }
      const events = reduceEvents(await jsonl(join(path, 'events.jsonl')), m.cohort.opening_tick, a.closing_tick);
      assert.equal(events.captures, a.captures); assert.equal(events.offspring, a.offspring);
      assert.equal(events.observed_adult_descendants, a.adult_descendants);
      for (const key of ['funded', 'born', 'refunded', 'miscarried'])
        assert.equal(events.reproduction[key] || 0, a.reproduction_audit.counts[key]);
      assert.equal(events.reproduction.not_funded || 0,
        a.reproduction_audit.counts.not_funded_cap + a.reproduction_audit.counts.not_funded_stocks);
      assert.equal(s.local_recovery.captures_seen[i], a.captures);
      const local = {};
      for (const row of localRows.filter(r => r.capture.arm === i)) count(local, row.status);
      assert.equal(sum(Object.values(local)), s.local_recovery.selected_captures[i]);
      assert.equal(s.local_recovery.selected_captures[i] + s.local_recovery.unselected_captures[i], a.captures);
      arms.push({name, opening, summary:a, events, local});
    }
    seeds.push({seed:s.seed, arms});
  }
  return {root, manifest:m, seeds};
}

// --- the paid-charging pair -------------------------------------------------------------
//
// `reserve-targets-charge80-v1` is `reserve-targets-v1` with semantic profile version 3 raised
// to 4 and nothing else. Version 4's one meaning is a fixed 0.80 E_max oxidation activation
// threshold for authoritative members, so the recipe must also *say* that in words and numbers:
// a version number alone could be mistaken for a geometry update.

export const CHARGE_POLICY = {baseline: 'configured-world-threshold', candidate: 'fixed-member-threshold', threshold: 0.8};

export function verifyChargePair(a, b) {
  assert.equal(a.profile_recipe, 'reserve-targets-v1');
  assert.equal(b.profile_recipe, 'reserve-targets-charge80-v1');
  for (const key of ['arm', 'config', 'heading', 'target', 'pre_import_inventory', 'post_import_inventory', 'receipt'])
    assert.deepEqual(a[key], b[key], `opening ${key} changed`);

  // The resolved policy, recorded per arm, is the one the recipe claims.
  assert.equal(a.recipe_oxidation_policy, CHARGE_POLICY.baseline);
  assert.equal(b.recipe_oxidation_policy, CHARGE_POLICY.candidate);
  assert.equal(a.world_oxidation_threshold, b.world_oxidation_threshold, 'the worlds disagree about the configured threshold');
  assert.equal(a.recipe_oxidation_threshold, a.world_oxidation_threshold, 'the baseline recipe must defer to the world');
  assert.equal(b.recipe_oxidation_threshold, CHARGE_POLICY.threshold);
  assert(b.recipe_oxidation_threshold > b.world_oxidation_threshold, 'the candidate threshold must be the raised one');
  // An arm that installs no profile resolves the world's own threshold, whatever the label.
  for (const o of [a, b])
    assert.equal(o.world_member_oxidation_threshold,
      o.profile === null ? o.world_oxidation_threshold : o.recipe_oxidation_threshold,
      'the arm\'s world resolved a threshold its profile does not imply');

  if (a.profile === null) {assert.equal(b.profile, null); return;}
  const expected = structuredClone(a.profile);
  assert.equal(expected.version, 3);
  // The fixed background this family runs on, asserted rather than assumed.
  assert.equal(expected.seek_reserve_fraction, 0.80);
  assert.equal(expected.perch_reserve_fraction, 0.90);
  expected.version = 4;
  assert.deepEqual(b.profile, expected, 'candidate changed a profile field other than `version`');
}

// The bounded oxidation diagnostics, checked for shape, sign and internal consistency — never
// for a value, which is the experiment's result and not this script's business.
export function verifyOxidation(a, recipeThreshold) {
  const o = a.oxidation;
  assert(o && typeof o.scope === 'string' && o.scope.length > 0, 'missing oxidation scope');
  assert.equal(o.member_threshold, recipeThreshold);
  assert(Number.isFinite(o.world_threshold) && o.world_threshold > 0);
  const c = o.charging_above_reference;
  safeCount(c.transactions);
  for (const key of ['reserve_burned', 'energy_gained', 'conversion_heat'])
    assert(Number.isFinite(c[key]) && c[key] >= 0, `invalid ${key}`);
  if (o.member_threshold <= o.world_threshold)
    assert.equal(c.transactions, 0, 'an unraised threshold cannot produce extra oxidation');
  if (c.transactions === 0)
    for (const key of ['reserve_burned', 'energy_gained', 'conversion_heat'])
      assert.equal(c[key], 0, `${key} without a transaction`);
  else
    assert(c.reserve_burned > 0, 'a transaction that burned nothing');
  return c;
}

export async function compareCharge(backgroundDir, candidateDir) {
  const a = await loadRun(backgroundDir, 'reserve-targets-v1', 12);
  const b = await loadRun(candidateDir, 'reserve-targets-charge80-v1', 12);
  for (const key of ['cohort', 'ticks', 'audit_window', 'build', 'executable_sha256', 'observer_contract'])
    assert.deepEqual(a.manifest[key], b.manifest[key], `unmatched ${key}`);
  // The manifest states the policy in words and in a number, not only as a version.
  assert.equal(a.manifest.member_oxidation_policy, CHARGE_POLICY.baseline);
  assert.equal(b.manifest.member_oxidation_policy, CHARGE_POLICY.candidate);
  assert.equal(b.manifest.member_oxidation_threshold, CHARGE_POLICY.threshold);
  assert.equal(a.manifest.member_oxidation_threshold, a.manifest.world_oxidation_threshold);
  assert.equal(a.manifest.world_oxidation_threshold, b.manifest.world_oxidation_threshold);
  assert(typeof b.manifest.oxidation_observer === 'string' && b.manifest.oxidation_observer.includes('charging_above_reference'));
  for (const m of [a.manifest, b.manifest]) assert(typeof m.profile_recipe_scope === 'string' && m.profile_recipe_scope.length > 0);
  assert(b.manifest.profile_recipe_scope.includes('version 3 raised to 4'), 'the candidate scope must name its whole diff');

  const seeds = a.seeds.map(s => {
    const t = b.seeds.find(x => x.seed === s.seed);
    assert(t, `candidate is missing seed ${s.seed}`);
    return {seed: s.seed, arms: s.arms.map((arm, i) => {
      verifyChargePair(arm.opening, t.arms[i].opening);
      const bg = verifyOxidation(arm.summary, arm.opening.world_member_oxidation_threshold);
      const cand = verifyOxidation(t.arms[i].summary, t.arms[i].opening.world_member_oxidation_threshold);
      assert.equal(bg.transactions, 0, 'the background recipe must never charge above the reference');
      // Only the untouched arm is required to be identical. Every arm carrying a member —
      // including the attack-disabled controls — may legitimately diverge, because the policy
      // is theirs too.
      if (i === 0) assert.deepEqual(arm.summary, t.arms[i].summary, 'the untouched arm changed');
      return {arm: arm.name, background: metrics(arm), candidate: metrics(t.arms[i]),
        charging: {background: bg, candidate: cand}};
    })};
  });
  return {kind: 'matched-hunter-charging-reduction',
    artifact_checks_passed: true,
    policy: `member oxidation activation raised from the configured ${a.manifest.world_oxidation_threshold} to a fixed ${CHARGE_POLICY.threshold} of E_max, for authoritative members only, at every age and phase`,
    biological_acceptance: 'not inferred; more battery is not success if reserve readiness collapses or founders still starve',
    limits: 'Checks artifact integrity, summary coverage/count reconciliation, recipe isolation to the semantic version alone, and the internal consistency of the bounded oxidation diagnostics. Does not re-run core audits, decode semantic WorldState, reconstruct contact geometry, or judge whether charging helped. Charging totals are per-arm process-scoped and are not persisted, so they describe each run, not each world. Off-hunter arms carry the policy and are not expected to match.',
    background: a.root, candidate: b.root, build: a.manifest.build, ticks: a.manifest.ticks, seeds};
}

export function metrics(arm) {
  const a = arm.summary, q = a.reproductive_opportunity, mature = q.age_and_size_ready_member_ticks;
  return {captures:a.captures, offspring:a.offspring, closing_hunters:a.closing_hunters,
    reproduction:a.reproduction_audit.counts, adult_descendants:a.adult_descendants,
    hunter_descendants_that_reproduced:a.descendants_that_reproduced,
    founder_extinction_tick:a.founder_extinction_tick, lineage_extinction_tick:a.lineage_extinction_tick,
    adult_occupancy:a.adult_occupancy_ticks_0_1_2_over2,
    member_ticks:q.member_ticks, age_and_size_ready_member_ticks:mature,
    mature_reserve_open_fraction:ratio(q.age_and_size_ready_reserve_gate_open_member_ticks,mature),
    mature_energy_open_fraction:ratio(q.age_and_size_ready_energy_gate_open_member_ticks,mature),
    mature_joint_stock_open_fraction:ratio(q.age_and_size_ready_both_stock_gates_open_member_ticks,mature),
    max_reserve_fraction:q.max_reserve_fraction, max_energy_fraction:q.max_energy_fraction,
    closing_prey:a.closing_population-a.closing_hunters, prey_min:a.prey_min, prey_tick_integral:a.prey_tick_integral,
    form_losses:a.whole_recovery.slice(1).map(r=>({opening:r.opening_count,minimum:r.minimum_count,
      newly_zero:r.opening_count>0 && r.first_zero_tick!==null,ticks_below_half:r.ticks_below_half})),
    events:arm.events, paid_energy_per_capture:ratio(arm.events.paid_energy,a.captures),
    local_recovery:arm.local};
}

export async function compare(baselineDir, candidateDir) {
  const a = await loadRun(baselineDir, 'baseline'), b = await loadRun(candidateDir, 'reserve-targets-v1');
  for (const key of ['cohort','ticks','audit_window','build','executable_sha256','observer_contract'])
    assert.deepEqual(a.manifest[key], b.manifest[key], `unmatched ${key}`);
  const seeds = a.seeds.map(s => {
    const t = b.seeds.find(x => x.seed === s.seed);
    return {seed:s.seed, arms:s.arms.map((arm,i)=>{
      verifyRecipePair(arm.opening,t.arms[i].opening);
      // Stored profiles differ in the budget control, even though its ecology is identical.
      if (i === 0) assert.deepEqual(arm.summary,t.arms[i].summary);
      if (i === 1) {
        const x=structuredClone(arm.summary), y=structuredClone(t.arms[i].summary);
        for (const key of ['closing_state_hash','closing_snapshot_sha256']) {delete x[key];delete y[key];}
        assert.deepEqual(x,y,'budget-only ecology/control statistics changed');
      }
      return {arm:arm.name,baseline:metrics(arm),candidate:metrics(t.arms[i])};
    })};
  });
  return {kind:'matched-hunter-recipe-reduction',artifact_checks_passed:true,
    biological_acceptance:'not inferred; inspect all seeds, funded descendants, survival and prey recovery',
    limits:'Checks artifact integrity, summary coverage/count reconciliation and recipe isolation; does not re-run core audits, decode semantic WorldState, or independently reconstruct contact/transaction geometry. Stock ratios are conditional member-ticks, null means no age-and-size-ready observation. Near/far describes settlement, not admission cause. Local recovery is treatment-selected, not a random sample.',
    baseline:a.root,candidate:b.root,build:a.manifest.build,ticks:a.manifest.ticks,seeds};
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    const [,, a, b, family = 'reserve-targets'] = process.argv;
    assert(a && b && process.argv.length <= 6,
      'Usage: node scripts/compare-hunter-recipes.mjs BASELINE_DIR CANDIDATE_DIR [reserve-targets|charge80]');
    assert(['reserve-targets', 'charge80'].includes(family), `unknown family ${family}`);
    const result = family === 'charge80' ? await compareCharge(a, b) : await compare(a, b);
    console.log(JSON.stringify(result, null, 2));
  } catch(error) {console.error(`Comparison refused: ${error.message}`);process.exitCode=1;}
}
