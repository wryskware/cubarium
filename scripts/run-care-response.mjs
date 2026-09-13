// Fixed, isolated single-pulse screen. Writes only to a NEW result directory.
// No live owner, HTTP, shim, world mutation outside child in-memory comparisons.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {readFile, writeFile, mkdir} from 'node:fs/promises';
import {resolve, join} from 'node:path';

const [binaryArg, cohortArg, outputArg] = process.argv.slice(2);
assert(binaryArg && cohortArg && outputArg && process.argv.length === 5,
  'Usage: node scripts/run-care-response.mjs BINARY COHORT_MANIFEST NEW_OUTPUT_DIR');
const binary = resolve(binaryArg), output = resolve(outputArg);
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const cohortBytes = await readFile(cohortArg), cohort = JSON.parse(cohortBytes);
assert.equal(cohort.kind, 'pre-hunter-cohort-preparation');
assert.equal(cohort.complete, true);
assert.equal(cohort.opening_tick, 144000);
const seeds = Array.from({length:12}, (_,i)=>i+1);
assert.deepEqual([...cohort.prescribed_seeds].sort((a,b)=>a-b), seeds);
assert.deepEqual(cohort.openings.map(o=>o.seed).sort((a,b)=>a-b), seeds);
for (const opening of cohort.openings) {
  assert.equal(sha(await readFile(opening.snapshot)), opening.sha256, 'opening checksum');
  assert.equal(opening.schema, 9);
}
await mkdir(output); // Refuse an existing result directory; never overwrite a cohort.
const recipe = {ticks:3000, care_start:600, care_every:72000, dose_permille:1000,
  local_every:20, audit_window:200, targets:[0,1,2], actions:['feed','rain','clean']};
await writeFile(join(output, 'manifest.json'), JSON.stringify({
  kind:'isolated-care-response-screen-v1', binary, binary_sha256:sha(await readFile(binary)),
  script_sha256:sha(await readFile(new URL(import.meta.url))),
  cohort_manifest_sha256:sha(cohortBytes), cohort, recipe,
  note:'All12 aged seeds, three fixed targets, three independent Standard pulses. Each child also runs an untreated control. No ecological acceptance inferred.',
}, null, 2), {flag:'wx'});
const run = promisify(execFile), outcomes = [];
for (const opening of [...cohort.openings].sort((a,b)=>a.seed-b.seed)) {
  let reference = null;
  for (const target of recipe.targets) for (const kind of recipe.actions) {
    const name = `seed-${opening.seed}-target-${target}-${kind}`;
    const args = [opening.snapshot, '--ticks','3000','--care-start','600',
      '--care-every','72000','--dose-permille','1000','--local-every','20',
      '--audit-window','200','--target-index',String(target),'--care-kind',kind];
    let stdout = '', stderr = '', code = 0;
    try { ({stdout,stderr} = await run(binary,args,{maxBuffer:16*1024*1024})); }
    catch (error) { stdout=error.stdout || '';stderr=error.stderr || String(error);code=error.code ?? 'unknown'; }
    await writeFile(join(output,name+'.json'),stdout,{flag:'wx'});
    if (stderr) await writeFile(join(output,name+'.stderr.txt'),stderr,{flag:'wx'});
    let validation_error = null;
    try {
      assert.equal(code,0,'child exit');
      const data=JSON.parse(stdout);
      assert.equal(data.start_tick,144000);assert.equal(data.seed,opening.seed);
      assert.equal(data.ticks,3000);assert.equal(data.care_kind,kind);
      assert.equal(data.first_target_index,target);assert.equal(data.dose_permille,1000);
      assert.equal(data.baseline.audit_passed,true);assert.equal(data.cared.audit_passed,true);
      assert.equal(data.baseline.final.tick,147000);assert.equal(data.cared.final.tick,147000);
      assert.equal(data.baseline.receipts.length,0);assert.equal(data.cared.receipts.length,1);
      assert.equal(data.cared.receipts[0].elapsed,600);
      assert.equal(data.cared.receipts[0].kind,kind);
      for (const arm of [data.baseline,data.cared]) {
        assert.equal(arm.local_activity.samples.length,151);
        assert.deepEqual(arm.local_activity.samples.map(s=>s.elapsed),Array.from({length:151},(_,i)=>i*20));
      }
      assert.deepEqual(data.baseline.local_activity.first_pulse_cohorts,data.cared.local_activity.first_pulse_cohorts);
      assert.deepEqual(data.baseline.local_activity.samples.slice(0,31),data.cared.local_activity.samples.slice(0,31));
      if (reference === null) reference=data.baseline;
      else assert.deepEqual(data.baseline,reference,'all nine untreated copies must match exactly');
    } catch (error) { validation_error=String(error); }
    outcomes.push({seed:opening.seed,target,kind,file:name+'.json',process_exit:code,validation_error});
  }
  console.log(`seed ${opening.seed}/12 complete; failures so far ${outcomes.filter(o=>o.validation_error).length}`);
}
const technical_complete=outcomes.length===108 && outcomes.every(o=>o.validation_error===null);
await writeFile(join(output,'summary.json'),JSON.stringify({technical_complete,outcomes,
  note:'Execution and bounded artifact consistency only. Local response interpretation remains separate.'},null,2),{flag:'wx'});
if (!technical_complete) process.exitCode=1;
