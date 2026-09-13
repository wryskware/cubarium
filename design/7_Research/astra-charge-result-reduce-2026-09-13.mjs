// Read-only evidence extraction, explicitly NOT a permissive replacement for compareCharge.
// Failed arms stay in the output with untrusted-prefix labels; no full-cohort pass is inferred.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve, join} from 'node:path';
import {ARMS, compareCharge, verifyArm, reduceEvents, metrics, verifyChargePair, verifyChargeManifestOpening,
  verifyOxidation, verifyChargeControlPair} from '../../scripts/compare-hunter-recipes.mjs';
import {inspectSnapshot} from '../../scripts/prepare-hunter-worlds.mjs';

const dirs = process.argv.slice(2);
assert.equal(dirs.length, 2, 'BACKGROUND_DIR CANDIDATE_DIR');
const json = async p => JSON.parse(await readFile(p, 'utf8'));
const jsonl = async p => (await readFile(p, 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse);
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const idKey = id => `${id.slot}:${id.generation}`;
const runs = await Promise.all(dirs.map(async (dir, index) => {
  const root = resolve(dir), manifest = await json(join(root, 'manifest.json'));
  const summary = await json(join(root, 'summary.json')); // Refuse still-running artifacts.
  assert.equal(manifest.profile_recipe, index ? 'reserve-targets-charge80-v1' : 'reserve-targets-v1');
  assert.equal(manifest.ticks, 144000);
  assert.equal(manifest.build, '0.1.0+3b06596');
  assert.equal(manifest.executable_sha256, '351997076c8a3e7a032109c45eee74541c75a87339a967018b083b23dd8674e0');
  assert.equal(hash(await readFile(join(root, 'hunter_compare.frozen'))), manifest.executable_sha256);
  assert.deepEqual(manifest.arms, ARMS);
  assert.deepEqual(summary.seeds.map(s => s.seed).sort((a,b) => a-b), Array.from({length:12}, (_,i) => i+1));
  return {root, manifest, summary};
}));
for (const key of ['cohort', 'ticks', 'audit_window', 'build', 'executable_sha256', 'observer_contract'])
  assert.deepEqual(runs[0].manifest[key], runs[1].manifest[key]);
// Same unchanged strict entry point used by the CLI; keep refusal, never patch its gates.
let strict;
try { strict = {passed:true, result:await compareCharge(...dirs)}; }
catch(error) { strict = {passed:false, error:error.message}; }
const seeds = [];
let completeArms = 0, incompleteArms = 0;
for (let seed = 1; seed <= 12; seed++) {
  const pair = [];
  for (const run of runs) {
    const s = run.summary.seeds.find(s => s.seed === seed), seedDir = join(run.root, `seed-${seed}`);
    assert.deepEqual(await json(join(seedDir, 'result.json')), s);
    assert.equal(s.arms.length, 6);
    const local = await jsonl(join(seedDir, 'local-recovery.jsonl'));
    const arms = [];
    for (const [i,name] of ARMS.entries()) {
      const dir = join(seedDir,name), a = await json(join(dir,'summary.json'));
      assert.deepEqual(a,s.arms[i]);
      const opening = await json(join(dir,'opening.json'));
      assert.equal(opening.arm,name);
      verifyChargeManifestOpening(run.manifest,opening);
      verifyOxidation(a,opening.world_member_oxidation_threshold,opening);
      for (const [file,sha,state] of [['post-initialization.cubw',opening.post_snapshot_sha256,opening.post_state_hash],
        ['closing.cubw',a.closing_snapshot_sha256,a.closing_state_hash]]) {
        const meta = inspectSnapshot(await readFile(join(dir,file)),12);
        assert.equal(meta.sha256,sha); assert.equal(meta.state_hash,state); assert.equal(meta.build,run.manifest.build);
      }
      const complete = a.technical_complete && a.complete_experiment_measurement;
      if (complete) {verifyArm(a,144000,144000); completeArms++;} else {
        incompleteArms++;
        assert.equal(a.technical_complete,false);
        assert.equal(a.complete_experiment_measurement,false);
        assert.equal(a.observer_statistics_trusted_through_closing_tick,false);
      }
      const eventRows = await jsonl(join(dir,'events.jsonl'));
      for(const row of eventRows) if(row.stream === 'hunter' && row.event.kind === 'reproduction') {
        const q = row.event.record;
        if(q.transaction === 'funded') {
          assert(Math.abs(q.parent_reserve_before-q.parent_reserve_after-1.6)<1e-12);
          assert(Math.abs(q.parent_energy_before-q.parent_energy_after-1.0)<1e-12);
          assert.equal(q.escrow_structure,0.8); assert.equal(q.escrow_reserve,0.8);
          assert.equal(q.escrow_energy,0.6); assert.equal(q.build_heat,0.4);
        }
        if(q.transaction === 'born') assert.equal(q.birth_heat,1.6);
      }
      const events = reduceEvents(eventRows,144000,a.closing_tick);
      assert.equal(events.captures,a.captures); assert.equal(events.offspring,a.offspring);
      assert.equal(events.observed_adult_descendants,a.adult_descendants);
      for (const key of ['funded','born','refunded','miscarried'])
        assert.equal(events.reproduction[key] || 0,a.reproduction_audit.counts[key]);
      const statuses = {};
      for (const row of local.filter(r => r.capture.arm === i)) statuses[row.status] = (statuses[row.status] || 0)+1;
      const children = new Map();
      for (const row of eventRows) if (row.stream === 'hunter' && row.event.kind === 'offspring') {
        const e = row.event;
        children.set(idKey(e.child), {id:idKey(e.child),parent:idKey(e.parent),birth_tick:e.tick,
          death_tick:null,death_cause:null,recorded_captures:0,samples:0,max_sampled_structure:null,
          max_sampled_reserve:null,max_sampled_energy:null});
      }
      for (const row of eventRows) {
        const e = row.event;
        if(row.stream === 'hunter' && e.kind === 'capture' && children.has(idKey(e.hunter)))
          children.get(idKey(e.hunter)).recorded_captures++;
        if(row.stream === 'life' && e.kind === 'death' && children.has(idKey(e.id)))
          Object.assign(children.get(idKey(e.id)),{death_tick:e.tick,death_cause:e.cause});
      }
      if(children.size) for (const row of await jsonl(join(dir,'census.jsonl')))
        for(const h of row.hunter_stocks) if(children.has(idKey(h.id))) {
          const child = children.get(idKey(h.id)); child.samples++;
          for(const key of ['structure','reserve','energy']) {
            const field = `max_sampled_${key}`;
            child[field] = child[field] === null ? h[key] : Math.max(child[field],h[key]);
          }
        }
      arms.push({name,opening,summary:a,events,local:statuses,
        offspring_evidence:[...children.values()],
        status: complete ? 'complete_two_hour_arm' : 'technical_failure_untrusted_prefix',
        actual_elapsed_ticks:a.closing_tick-144000,
        last_complete_observer_tick:a.last_complete_observer_tick,
        last_complete_reproduction_tick:a.last_complete_reproduction_tick});
    }
    pair.push({failure:s.failure,local:s.local_recovery,arms});
  }
  const arms = ARMS.map((name,i) => {
    const a = pair[0].arms[i],b=pair[1].arms[i];
    verifyChargePair(a.opening,b.opening);
    const matched = a.status === 'complete_two_hour_arm' && b.status === 'complete_two_hour_arm';
    if(matched) verifyChargeControlPair(a.summary,b.summary,i);
    const compact = x => ({status:x.status,actual_elapsed_ticks:x.actual_elapsed_ticks,
      last_complete_observer_tick:x.last_complete_observer_tick,last_complete_reproduction_tick:x.last_complete_reproduction_tick,
      recorded_metrics:metrics(x),readiness_counts:x.summary.reproductive_opportunity,
      extra_charging:x.summary.oxidation.charging_above_reference,
      audit:x.summary.audit,whole_recovery:x.summary.whole_recovery,
      offspring_evidence:x.offspring_evidence});
    return {arm:name,matched_complete_two_hour_pair:matched,background:compact(a),candidate:compact(b)};
  });
  seeds.push({seed,background_failure:pair[0].failure,candidate_failure:pair[1].failure,
    local_pair_status:{background:pair[0].local,candidate:pair[1].local},arms});
}
console.log(JSON.stringify({kind:'retained-hunter-charging-evidence-not-acceptance',
  strict_comparison:strict,
  artifact_checks_passed:strict.passed,
  evidence_note:'Individual file envelopes/identity/counts checked. Failed prefixes are not completed observations; do not pool seed6 with full horizons or call 11 surviving pairs a successful 12-seed experiment.',
  biological_acceptance:false,complete_arms:completeArms,incomplete_arms:incompleteArms,
  manifests:runs.map(r=>r.manifest),seeds},null,2));
