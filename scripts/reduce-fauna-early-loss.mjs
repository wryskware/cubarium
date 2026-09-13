// Read-only location of early form loss in the original, unfiltered cohort.
// Census samples cannot identify individual deaths, intake, or allocation causes.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {inspectSnapshot} from './prepare-hunter-worlds.mjs';

const SEEDS = Array.from({length:12}, (_, i)=>i+1);
const HASH = /^[0-9a-f]{64}$/;
const sha = b=>createHash('sha256').update(b).digest('hex');
const integer = n=>Number.isSafeInteger(n) && n>=0;
const sum = a=>a.reduce((x,y)=>x+y,0);

export function parseTelemetry(text) {
  assert.ok(text.endsWith('\n'), 'truncated telemetry record');
  return text.trimEnd().split('\n').map(line=>JSON.parse(line.replace(
    /"((?:state|ecology)_hash)":\s*(\d+)/g, '"$1":"$2"')));
}

export function reduceSamples(rows, endTick=144000, sampleTicks=100) {
  assert.ok(integer(endTick) && integer(sampleTicks) && sampleTicks>0 && endTick>0);
  assert.equal(endTick%sampleTicks,0);
  assert.equal(rows.length,endTick/sampleTicks,'missing/extra samples');
  const forms = Array.from({length:8},(_,form)=>({form, first_sample_count:null,
    closing:0, minimum:null, maximum:0, first_net_decline:null, first_absence:null,
    first_loss_bracket:null, last_positive_tick:null, sampled_reappearances:[],
    positive_samples:0, zero_samples:0}));
  let prior;
  for (const [i,row] of rows.entries()) {
    assert.equal(row.tick,(i+1)*sampleTicks,'missing/repeated/reordered sample');
    assert.ok(integer(row.population));
    assert.equal(row.population_by_form?.length,8);
    assert.ok(row.population_by_form.every(integer));
    assert.equal(sum(row.population_by_form),row.population,'form census mismatch');
    for (const key of ['births','deaths_starvation','deaths_age','deaths_collapse'])
      assert.ok(integer(row[key]),`invalid ${key}`);
    if (prior) assert.equal(row.population,prior.population+row.births
      -row.deaths_starvation-row.deaths_age-row.deaths_collapse,'population flow mismatch');
    for (const key of ['care_admitted_seq','care_feed_material_in','care_feed_energy_in',
      'care_rain_depth_in','care_clean_material_out','care_clean_energy_out','care_allowance_used'])
      assert.equal(row[key],0,`unexpected ${key}`);
    for (const key of ['fruit','water','producer','detritus','organism_material','organism_energy'])
      assert.ok(Number.isFinite(row[key]) && row[key]>=0,`invalid ${key}`);
    assert.match(row.state_hash,/^\d+$/,'state hash must be an exact decimal string');
    for (const f of forms) {
      const n=row.population_by_form[f.form], previous=prior?.population_by_form[f.form];
      f.first_sample_count ??= n;
      f.closing=n;
      f.minimum=f.minimum===null?n:Math.min(f.minimum,n);
      f.maximum=Math.max(f.maximum,n);
      if (n>0) { f.positive_samples++; f.last_positive_tick=row.tick; }
      else { f.zero_samples++; f.first_absence ??= row.tick; }
      if (previous!==undefined && n<previous && f.first_net_decline===null)
        f.first_net_decline={after_tick:prior.tick,by_tick:row.tick,from:previous,to:n};
      if (previous>0 && n===0 && f.first_loss_bracket===null)
        f.first_loss_bracket={after_tick:prior.tick,by_tick:row.tick};
      if (previous===0 && n>0) f.sampled_reappearances.push(row.tick);
    }
    prior=row;
  }
  return {sample_count:rows.length,first_sample_tick:sampleTicks,closing_tick:endTick,forms};
}

export async function reduce(directory) {
  const manifestBytes=await readFile(join(directory,'manifest.json'));
  const manifest=JSON.parse(manifestBytes);
  const initialBytes=await readFile(join(directory,'initial-manifest.json'));
  const initials=JSON.parse(initialBytes);
  assert.equal(manifest.kind,'pre-hunter-cohort-preparation');
  assert.equal(manifest.complete,true);
  assert.deepEqual(manifest.prescribed_seeds,SEEDS);
  assert.equal(manifest.opening_tick,144000);
  assert.equal(manifest.simulated_age_seconds,7200);
  assert.equal(manifest.openings.length,12);
  assert.equal(initials.initials.length,12);
  assert.match(manifest.runner_sha256,HASH);
  assert.equal(initials.runner_sha256,manifest.runner_sha256);
  assert.equal(sha(await readFile(join(directory,'pre-hunter-runner'))),manifest.runner_sha256);
  const results=[];
  for (const seed of SEEDS) {
    const select=list=>{
      const rows=list.filter(r=>r.seed===seed);
      assert.equal(rows.length,1,`missing/duplicate seed${seed}`);
      return rows[0];
    };
    const closing=select(manifest.openings), initial=select(initials.initials);
    assert.equal(initial.tick,0);
    for (const entry of [initial,closing]) {
      const snapshot=inspectSnapshot(await readFile(entry.snapshot),9);
      for (const key of ['schema','build','sha256','state_hash','crc32','payload_bytes'])
        assert.equal(snapshot[key],entry[key],`seed${seed} ${key} mismatch`);
    }
    assert.equal(initial.build,closing.build);
    const bytes=await readFile(join(directory,`seed-${seed}`,'telemetry.jsonl'));
    const rows=parseTelemetry(bytes.toString());
    const result=reduceSamples(rows);
    const final=rows.at(-1);
    assert.equal(final.state_hash,closing.state_hash);
    assert.equal(final.population,closing.population);
    assert.deepEqual(final.population_by_form,closing.population_by_form);
    assert.deepEqual(final,closing.telemetry,'closing telemetry changed');
    results.push({seed,build:closing.build,telemetry_sha256:sha(bytes),
      initial_snapshot_sha256:initial.sha256,closing_snapshot_sha256:closing.sha256,...result});
  }
  return {kind:'fauna-early-loss-census-v1',complete:true,
    manifest_sha256:sha(manifestBytes),initial_manifest_sha256:sha(initialBytes),
    runner_sha256:manifest.runner_sha256,simulated_seconds:7200,sample_seconds:5,
    independent_conservation_audit:false,individual_lineage_measurement:false,
    interpretation:'Original no-care cohort, not a new experiment. Form census loss is bracketed by 100-tick samples, not exact death time or cause. Net declines do not count all births/deaths. Forms initially absent are not extinctions. Snapshot checks are header/payload integrity, not semantic decoding.',
    results};
}

if (process.argv[1] && import.meta.url===pathToFileURL(resolve(process.argv[1])).href) {
  const args=process.argv.slice(2);
  if (args.length!==1) { console.error('Usage: node scripts/reduce-fauna-early-loss.mjs COHORT_DIRECTORY'); process.exitCode=1; }
  else reduce(resolve(args[0])).then(r=>console.log(JSON.stringify(r,null,2)))
    .catch(e=>{console.error(e);process.exitCode=1;});
}
