// Read-only reduction; detailed full IDs and trajectories remain in the original artifacts.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {join} from 'node:path';
import {createHash} from 'node:crypto';
const root=process.argv[2]??'captures/astra-quiet-a89179a';
const read=async p=>JSON.parse(await readFile(p,'utf8'));
const lines=async p=>(await readFile(p,'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse);
const m=await read(join(root,'manifest.json')),s=await read(join(root,'summary.json'));
assert.equal(m.kind,'astra-ordinary-quiet-diagnosis-v1');assert.equal(m.ticks,12000);
assert.equal(createHash('sha256').update(await readFile(join(root,'astra_quiet_diagnosis.frozen'))).digest('hex'),m.binary_sha256);
assert.equal(s.technical_complete,true);assert.equal(s.results.length,24);
const individuals=[],eventRows=[],arms=[],seen=new Set();
for(const row of s.results){
  const key=`${row.seed}/${row.arm}`;assert(!seen.has(key));seen.add(key);
  assert(row.seed>=1&&row.seed<=12&&['baseline','feed'].includes(row.arm));
  assert.equal(row.technical_complete,true);assert.equal(row.error,null);
  const dir=join(root,`seed-${row.seed}-${row.arm}`),r=await read(join(dir,'result.json'));
  assert.equal(r.technical_complete,true);assert.equal(r.audit_passed,true);
  assert.equal(r.observation.opening_tick,144000);assert.equal(r.observation.closing_tick,156000);
  assert.equal(r.closing_hash,row.closing_hash);
  const ids=new Set();
  for(const x of r.observation.individuals){
    const id=`${x.id.slot}/${x.id.generation}`;assert(!ids.has(id));ids.add(id);
    const c=x.counts;assert.equal(c.resting+c.seeking+c.feeding_mode,c.ticks);
    assert(c.fed<=c.ticks&&c.fed_reserve_zero<=c.fed);
    individuals.push({seed:row.seed,arm:row.arm,...x});
  }
  const events=await lines(join(dir,'events.jsonl'));
  eventRows.push(...events.map(e=>({seed:row.seed,arm:row.arm,...e})));
  const samples=await lines(join(dir,'samples.jsonl'));assert.equal(samples.length,61);
  samples.forEach((x,i)=>assert.equal(x.tick,144000+i*200));
  arms.push({seed:row.seed,arm:row.arm,population:r.population,
    individuals:r.observation.individuals.length,peaks:r.peak_residuals_material_corrected_energy_water_independent_energy});
}
const stats=xs=>{
  const counts={};for(const x of xs)for(const[k,v]of Object.entries(x.counts))counts[k]=(counts[k]??0)+v;
  const ticks=counts.ticks??0;
  return {individuals:xs.length,never_fed:xs.filter(x=>x.counts.fed===0).length,
    ever_reserve_above_instant_rest_threshold:xs.filter(x=>x.max_reserve_fraction>1-x.initial.seek_off).length,
    min_memory:xs.length?Math.min(...xs.map(x=>x.min_memory)):null,
    max_reserve_fraction:xs.length?Math.max(...xs.map(x=>x.max_reserve_fraction)):null,
    mean_reserve_fraction:ticks?counts.reserve_fraction_sum/ticks:null,
    mean_energy_fraction:ticks?counts.energy_fraction_sum/ticks:null,
    mean_memory:ticks?counts.memory_sum/ticks:null,counts};
};
const byArm={};
for(const arm of ['baseline','feed']){
  const xs=individuals.filter(x=>x.arm===arm),ev=eventRows.filter(e=>e.arm===arm);
  const bouts=ev.filter(e=>e.kind==='rest_bout');
  const funds=ev.filter(e=>e.kind==='observed_escrow_transition'&&e.after.escrow!==null);
  const range=values=>values.length?[Math.min(...values),Math.max(...values)]:null;
  byArm[arm]={all:stats(xs),opening:stats(xs.filter(x=>x.opening_member)),
    by_form:Object.fromEntries(Array.from({length:8},(_,form)=>[form,stats(xs.filter(x=>x.form===form))])),
    births:ev.filter(e=>e.stream==='life'&&e.event.kind==='birth').length,
    deaths:ev.filter(e=>e.stream==='life'&&e.event.kind==='death').reduce((a,e)=>{const c=e.event.cause;a[c]=(a[c]??0)+1;return a},{}),
    bouts:bouts.reduce((a,e)=>{const k=e.entry;const v=a[k]??={count:0,ticks:0,max_ticks:0,right_censored:0,left_censored:0};
      v.count++;v.ticks+=e.observed_resting_ticks;v.max_ticks=Math.max(v.max_ticks,e.observed_resting_ticks);
      v.right_censored+=Number(e.right_censored);v.left_censored+=Number(e.left_censored);return a},{}),
    escrow_observation:{count:funds.length,prior_reserve_fraction_range:range(funds.map(e=>e.before.reserve_fraction)),
      after_reserve_fraction_range:range(funds.map(e=>e.after.reserve_fraction)),
      prior_memory_range:range(funds.map(e=>e.before.memory)),
      prior_above_instant_rest_reserve:funds.filter(e=>e.before.reserve_fraction>1-e.before.seek_off).length},
    by_seed:Object.fromEntries(Array.from({length:12},(_,i)=>[i+1,stats(xs.filter(x=>x.seed===i+1))]))};
}
console.log(JSON.stringify({source:root,build:m.build,binary_sha256:m.binary_sha256,arms,by_arm:byArm},null,2));
