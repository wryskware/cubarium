import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {CONTRACT,contractFor,ARMS,TARGETS,verifySeeds,verifyManifest,verifyBaseline,verifyOpening,verifySummary,
  Receipts,Samples,verifyLocal,contrasts,reduceAmbient} from './reduce-ambient-support.mjs';

const clone=structuredClone;
const source={seed:1,population:2,population_by_form:[1,1,0,0,0,0,0,0],ecology_hash:'1'};
const baseline=()=>({material:100,energy:200,water:10,limits:{material:1e-6,energy:2e-6,water:1e-7},basis:'common pre'});
function opening(i=0) {
  return {arm:ARMS[i],dose_permille:[null,1000,1500,null,1000,1500][i],candidate:i>=3,natural_rain_percent:i>=3?90:100,
    opening_tick:144000,care_kind:'rain',care_start_tick:60,care_period_ticks:2400,targets:clone(TARGETS),region_hops:3,region_cells:[25,25,16],
    pre_intervention_baseline:baseline(),config:{seed:1,water:{rain_rate:i>=3?.6*.9:.6,flood:1.5},other:123},opening_rain_rate:.6,
    arm_rain_rate:i>=3?.6*.9:.6,state_hash_before_change:'1',state_hash_after_change:i>=3?'2':'1',ecology_hash_after_change:i>=3?'2':'1',
    rain_rate_bits_unchanged:i<3,config_sha256:'a'.repeat(64)};
}
function local(elapsed) {
  return {observed_ticks:elapsed,opening_tick:144000,flood_threshold:1.5,global_water_tick_integral:10*elapsed,
    global_flooded_cells_now:0,global_drowned_cells_now:0,global_flooded_cell_ticks:0,global_drowned_cell_ticks:0,
    targets:TARGETS.map((target,i)=>({target:clone(target),cells:[25,25,16][i],water_sampled:0,water_tick_integral:0,
      producer:0,fruit:0,detritus:0,nutrient:0,flooded_cell_ticks:0,drowned_cell_ticks:0}))};
}
function summary() {
  return {arm:ARMS[0],technical_complete:true,complete_experiment_measurement:true,audit_passed:true,
    horizon:'two-hour',termination:'planned_horizon',planned_ticks:144000,elapsed_ticks:144000,closing_tick:288000,
    pre_intervention_baseline:baseline(),rain_rate:.6,max_absolute_drift:{material:0,energy:0,water:0},corrected_energy_drift:0,
    independent_windowed_energy_drift:0,care_boundary_energy_drift:0,legacy_audit_passed:true,population_min:2,population_max:2,
    closing_population:2,surviving_opening_cohorts:2,maximum_descendant_depth:0,first_extinction_tick:null,
    closing_state_hash:'1',closing_ecology_hash:'1',water:{total_rain_in:0,manual_attributed:0,natural:0,evap_out:0,closing_sampled:10},
    local:local(144000),care:{dose_permille:null,attempts:0,applied:0,rejected:0,partial:0,
      ledgers:{admitted_seq:0,showers:[],feed_material_in:0,feed_energy_in:0,clean_material_out:0,clean_energy_out:0,allowance_used:0,rain_depth_in:0}}};
}
function sample(elapsed=200) {
  return {elapsed,tick:144000+elapsed,population:2,population_by_form:clone(source.population_by_form),population_by_face:[2,0,0,0,0],occupied_cells:2,
    window_births:0,window_deaths:{starvation:0,age:0,collapse:0},window_cap_rejections:0,surviving_opening_cohorts:2,maximum_descendant_depth:0,
    water_sampled:10,water_by_face_sampled:[10,0,0,0,0],producer_by_face:[0,0,0,0,0],producer:0,fruit:0,detritus:0,nutrient:0,
    window_rain_in:0,window_evap_out:0,cumulative_rain_total:0,cumulative_rain_manual_attributed:0,cumulative_rain_natural:0,
    local:local(elapsed),state_hash:'1',ecology_hash:'1',receipts_so_far:0};
}
function receipt(n=0,dose=1000,rejected=false) {
  const elapsed=60+n*2400,tick=144000+elapsed,i=n%3;
  return {elapsed,tick,kind:'rain',target_index:i,target:clone(TARGETS[i]),dose_permille:dose,outcome:rejected?'rejected':'applied',reason:rejected?'shower active':null,
    receipt:{seq:n+1,tick,outcome:rejected?{Rejected:'shower active'}:{Applied:{cells:[13,13,9][i],ends_tick:tick+120,
      material_in:0,material_out:0,energy_in:0,energy_out:0,water_depth:4*dose/1000}}}};
}
function manifest() {
  return {kind:'six-arm-ambient-rainfall-comparison',build:CONTRACT.build,executable_sha256:CONTRACT.sha,horizon:'two-hour',ticks:144000,sample_every:200,arms:ARMS,
    factors:{support:{field:'config.water.rain_rate',candidate_fraction:.9,levels:['100% untouched',"90% of the opening's own rate"]},
      dose:{levels:[null,1000,1500],kind:'rain',schedule:{start_tick:60,period_ticks:2400,targets:TARGETS}}},
    cohort:{complete:true,kind:'pre-hunter-cohort-preparation',opening_tick:144000,openings:Array.from({length:12},(_,i)=>({seed:i+1}))}};
}
test('pinned build, horizon, cadence and all twelve distinct seeds are required',()=>{
  verifyManifest(manifest());
  for(const mutate of [m=>m.build='other',m=>m.executable_sha256='b'.repeat(64),m=>m.sample_every=400,m=>m.ticks=2400,
    m=>m.factors.support.candidate_fraction=.8,m=>m.factors.dose.schedule.targets[0].u=1,m=>m.cohort.openings.pop()]) {
    const m=clone(manifest());mutate(m);assert.throws(()=>verifyManifest(m));
  }
  assert.throws(()=>verifySeeds(Array.from({length:12},()=>({seed:1}))));
});
test('factor isolation includes exact rate multiplication and common PRE inventories',()=>{
  for(let i=0;i<6;i++)verifyOpening(opening(i),opening(),i,baseline(),source);
  for(const mutate of [o=>o.config.other++,o=>o.config.water.rain_rate=.6,o=>o.dose_permille=1000,
    o=>o.region_cells[2]=25,o=>o.pre_intervention_baseline.energy++,o=>o.state_hash_before_change='3']) {
    const o=opening(5);mutate(o);assert.throws(()=>verifyOpening(o,opening(),5,baseline(),source));
  }
  const b=baseline();b.limits.energy*=2;assert.throws(()=>verifyBaseline(b));
});
test('all audit gates are strict, while failed legacy raw energy remains honestly separate',()=>{
  verifySummary(summary(),opening(),baseline(),source);
  for(const mutate of [a=>a.audit_passed=false,a=>a.technical_complete=false,a=>a.complete_experiment_measurement=false,
    a=>a.corrected_energy_drift=2e-6,a=>a.independent_windowed_energy_drift=2e-6,a=>a.care_boundary_energy_drift=2e-6,
    a=>a.max_absolute_drift.material=1e-6,a=>a.max_absolute_drift.water=1e-7,a=>a.water.natural=1,
    a=>a.water.closing_sampled=11,a=>a.corrected_energy_drift=NaN,a=>a.closing_tick--]) {
    const a=summary();mutate(a);assert.throws(()=>verifySummary(a,opening(),baseline(),source));
  }
  const a=summary();a.max_absolute_drift.energy=1;a.legacy_audit_passed=false;
  verifySummary(a,opening(),baseline(),source);
});
test('receipts retain genuine refusals and validate every scheduled target/dose/sequence/payment',()=>{
  const r=new Receipts(1500);for(let i=0;i<60;i++)r.add(receipt(i,1500,i===1));
  assert.equal(r.depth,354);assert.equal(r.outcomes.rejected,1);assert.equal(r.reasons['shower active'],1);
  for(const mutate of [x=>x.elapsed++,x=>x.receipt.seq++,x=>x.target_index=2,x=>x.target.u++,x=>x.dose_permille=1000,
    x=>x.receipt.outcome.Applied.water_depth=4,x=>x.receipt.outcome.Applied.energy_in=1,x=>x.receipt.outcome.Applied.cells=25]) {
    const x=receipt(0,1500);mutate(x);assert.throws(()=>new Receipts(1500).add(x));
  }
  assert.throws(()=>new Receipts(null).add(receipt()));assert.throws(()=>new Receipts(1000).finish(summary(),1e-7));
  const no=new Receipts(null);no.finish(summary(),1e-7);
});
test('sample sequence, exact window population accounting and local cell-time bounds fail closed',()=>{
  const fresh=()=>new Samples(source,opening(),summary(),new Receipts(null));
  for(const mutate of [s=>s.elapsed++,s=>s.tick++,s=>s.window_births++,s=>s.population_by_form[0]++,
    s=>s.surviving_opening_cohorts=3,s=>s.water_by_face_sampled[0]=9,s=>s.water_sampled=11,
    s=>s.local.observed_ticks++,s=>s.local.targets[2].cells=25,s=>s.local.targets[0].flooded_cell_ticks=5001,
    s=>s.cumulative_rain_natural=-1,s=>s.receipts_so_far=1,s=>s.state_hash=9007199254740992]) {
    const s=sample();mutate(s);assert.throws(()=>fresh().add(s));
  }
  const r=fresh();r.add(sample());assert.throws(()=>r.add(sample()));assert.throws(()=>r.finish());
  const prior=local(200),now=local(400);now.global_flooded_cell_ticks=1280*200+1;
  assert.throws(()=>verifyLocal(now,prior,400,1.5,10));
});
test('small signed attribution noise is preserved, not confused with negative water or clamped',()=>{
  const r=new Samples(source,opening(),summary(),new Receipts(null));const s=sample();s.cumulative_rain_natural=-1e-12;r.add(s);
  assert.equal(r.previous.cumulative_rain_natural,-1e-12);
});
test('720 samples close exactly; endpoint corruption and extra samples are refused',()=>{
  const r=new Samples(source,opening(),summary(),new Receipts(null));
  for(let i=1;i<=720;i++)r.add(sample(i*200));
  const out=r.finish();assert.equal(out.sampled_means.population,2);assert.equal(out.mean_water_over_ticks,10);
  assert.equal(out.forms[4].newly_sampled_zero,false);assert.equal(out.forms[4].first_sampled_zero,144000);
  assert.throws(()=>r.add(sample(144200)));r.a.closing_state_hash='2';assert.throws(()=>r.finish());
});
test('zero population and absent forms stay explicit; no artificial opportunity denominator',()=>{
  const a=summary(),s={...source,population:0,population_by_form:Array(8).fill(0)};
  Object.assign(a,{population_min:0,population_max:0,closing_population:0,surviving_opening_cohorts:0,first_extinction_tick:144000});
  verifySummary(a,opening(),baseline(),s);
});
test('support and care contrasts use matched levels and correct difference-in-differences',()=>{
  const arms=[10,11,13,20,24,28].map(n=>({sampled_means:{population:n},closing_population:n,births:n,deaths:{starvation:0},
    surviving_opening_cohorts:1,mean_water_over_ticks:n,global_flooded_cell_ticks:n,local:[{mean_water_over_ticks:n}]}));
  const c=contrasts(arms);assert.equal(c.support[1].difference_90_minus_100.sampled_population,13);
  assert.equal(c.care[1].standard_minus_none.sampled_population,4);assert.equal(c.interaction[0].sampled_population,3);
});
test('missing run refuses full pass and retains twelve missing seed cases',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'cubarium-ambient-reducer-'));
  try {const r=await reduceAmbient(dir);assert.equal(r.artifact_checks_passed,false);assert.equal(r.seeds.length,12);
    assert(r.seeds.every(s=>s.available===false));assert.equal(r.failures.length,1);
    await writeFile(join(dir,'manifest.json'),'{}');assert.equal((await reduceAmbient(dir)).artifact_checks_passed,false);
    await mkdir(join(dir,'seed-6'));
    await writeFile(join(dir,'seed-6/result.json'),JSON.stringify({technical_complete:false,failure:'original fault',
      arms:[{arm:ARMS[0],technical_complete:false,audit_passed:false,termination:'failed',error:'retained fault'}]}));
    const failed=await reduceAmbient(dir);assert.equal(failed.artifact_checks_passed,false);
    assert.equal(failed.seeds[5].available,true);assert.equal(failed.seeds[5].failure,'original fault');
    assert.equal(failed.seeds[5].arms[0].error,'retained fault');assert.equal(failed.seeds.length,12);
  }finally{await rm(dir,{recursive:true});}
});

test('long horizons require an explicit pinned selection, never inferred or relabelled',()=>{
  for(const [horizon,ticks] of [['twenty-four-hour',1728000],['seventy-two-hour',5184000]]) {
    assert.deepEqual(contractFor(horizon),{...CONTRACT,ticks});
    const m=manifest();Object.assign(m,{horizon,ticks});
    verifyManifest(m,horizon);assert.throws(()=>verifyManifest(m));
    assert.throws(()=>verifyManifest(manifest(),horizon));
    for(const mutate of [x=>x.ticks--,x=>x.horizon='two-hour',x=>x.sample_every=2400,
      x=>x.build='other',x=>x.factors.dose.schedule.period_ticks=4800]) {
      const bad=clone(m);mutate(bad);assert.throws(()=>verifyManifest(bad,horizon));
    }
    const a=summary();Object.assign(a,{horizon,planned_ticks:ticks,elapsed_ticks:ticks,closing_tick:144000+ticks});
    verifySummary(a,opening(),baseline(),source,horizon);
    assert.throws(()=>verifySummary(a,opening(),baseline(),source));
    for(const mutate of [x=>x.elapsed_ticks=144000,x=>x.planned_ticks=144000,x=>x.closing_tick=288000,
      x=>x.pre_intervention_baseline.limits.water*=12,x=>x.corrected_energy_drift=2e-6]) {
      const bad=clone(a);mutate(bad);assert.throws(()=>verifySummary(bad,opening(),baseline(),source,horizon));
    }
    Object.assign(a,{population_min:0,closing_population:0,surviving_opening_cohorts:0,first_extinction_tick:a.closing_tick});
    verifySummary(a,opening(),baseline(),source,horizon);
    a.first_extinction_tick++;assert.throws(()=>verifySummary(a,opening(),baseline(),source,horizon));
  }
  for(const h of ['auto','24h','ten-minute','constructor','__proto__',1728000])assert.throws(()=>contractFor(h));
});

for(const [horizon,ticks,attempts] of [['twenty-four-hour',1728000,720],['seventy-two-hour',5184000,2160]]) {
  test(`${horizon}: exact receipt count, last shower and paid dose totals`,()=>{
    const r=new Receipts(1500,horizon);
    for(let i=0;i<attempts-1;i++)r.add(receipt(i,1500));
    assert.throws(()=>r.finish(summary(),1e-7));
    const last=receipt(attempts-1,1500);assert(last.receipt.outcome.Applied.ends_tick<144000+ticks);r.add(last);
    const a=summary();Object.assign(a.care,{dose_permille:1500,attempts,applied:attempts});
    Object.assign(a.care.ledgers,{admitted_seq:attempts,rain_depth_in:attempts*6});
    a.water.manual_attributed=attempts*6;r.finish(a,1e-7);
    assert.throws(()=>r.add(receipt(attempts,1500)));
    const no=new Receipts(null,horizon);no.finish(summary(),1e-7);assert.throws(()=>no.add(receipt()));
  });
  test(`${horizon}: complete streaming census, correct means, no short-prefix completion`,()=>{
    const a=summary();Object.assign(a,{horizon,planned_ticks:ticks,elapsed_ticks:ticks,closing_tick:144000+ticks,local:local(ticks)});
    const r=new Samples(source,opening(),a,new Receipts(null,horizon),horizon);
    for(let i=1;i<=ticks/200;i++) {
      r.add(sample(i*200));if(i===720)assert.throws(()=>r.finish());
    }
    const out=r.finish();assert.equal(out.sampled_means.population,2);assert.equal(out.forms[0].sampled_mean,1);
    assert.equal(out.mean_water_over_ticks,10);assert.deepEqual(out.mean_sampled_water_by_face,[10,0,0,0,0]);
    assert.equal(out.forms[4].newly_sampled_zero,false);assert.equal(out.local[0].mean_sampled_producer,0);
    assert.throws(()=>r.add(sample(ticks+200)));
    r.a.local=local(144000);assert.throws(()=>r.finish());
    assert.throws(()=>new Samples(source,opening(),a,new Receipts(null),horizon));
  });
}
