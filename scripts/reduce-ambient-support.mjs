// Read-only, pinned reduction of the 3e9bc2f two-hour ambient rainfall experiment.
// No simulation, artifact writes, semantic snapshot decoding or biological acceptance.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {createReadStream} from 'node:fs';
import {readFile} from 'node:fs/promises';
import {createInterface} from 'node:readline';
import {resolve, join, dirname} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {inspectSnapshot} from './prepare-hunter-worlds.mjs';

export const CONTRACT = Object.freeze({build:'0.1.0+3e9bc2f',
  sha:'17b624087fb51874f6abe523860abec5cbfb47854ff917a0ca23804c6fc91af0',
  opening:144000,ticks:144000,cadence:200,start:60,period:2400});
export const ARMS = ['rain100_none','rain100_standard','rain100_generous','rain90_none','rain90_standard','rain90_generous'];
export const TARGETS = [{face:0,u:32,v:48},{face:0,u:63.5,v:48},{face:1,u:32,v:63.5}];
const CELLS = [25,25,16];
const DOSES = [null,1000,1500,null,1000,1500];
const json = async p => JSON.parse(await readFile(p,'utf8'));
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const total = xs => xs.reduce((a,b)=>a+b,0);
const count = n => assert(Number.isSafeInteger(n)&&n>=0,`invalid count ${n}`);
const positive = n => assert(Number.isFinite(n)&&n>=0,`invalid nonnegative quantity ${n}`);
const hash64 = s => assert(typeof s==='string'&&/^\d+$/.test(s)&&BigInt(s)<=0xffffffffffffffffn,'invalid full-width hash');
const close = (a,b,tolerance,what) => {
  assert(Number.isFinite(a)&&Number.isFinite(b)&&Number.isFinite(tolerance)&&tolerance>0,`${what}: nonfinite`);
  assert(Math.abs(a-b)<tolerance,`${what}: ${a} != ${b} (limit ${tolerance})`);
};
function vector(xs,n,check=positive) {assert.equal(xs?.length,n);xs.forEach(check);}
export function verifySeeds(rows) {
  assert.equal(rows?.length,12,'all twelve seeds required');
  assert.deepEqual(rows.map(x=>x.seed).sort((a,b)=>a-b),Array.from({length:12},(_,i)=>i+1),'seed identity');
}
export function verifyManifest(m) {
  assert.equal(m.kind,'six-arm-ambient-rainfall-comparison');
  assert.equal(m.build,CONTRACT.build);assert.equal(m.executable_sha256,CONTRACT.sha);
  assert.equal(m.horizon,'two-hour');assert.equal(m.ticks,CONTRACT.ticks);assert.equal(m.sample_every,200);
  assert.deepEqual(m.arms,ARMS);
  assert.equal(m.factors.support.field,'config.water.rain_rate');
  assert.equal(m.factors.support.candidate_fraction,.9);
  assert.deepEqual(m.factors.support.levels,['100% untouched',"90% of the opening's own rate"]);
  assert.deepEqual(m.factors.dose.levels,[null,1000,1500]);assert.equal(m.factors.dose.kind,'rain');
  const schedule=m.factors.dose.schedule;
  assert.equal(schedule.start_tick,60);assert.equal(schedule.period_ticks,2400);assert.deepEqual(schedule.targets,TARGETS);
  assert.equal(m.cohort.complete,true);assert.equal(m.cohort.kind,'pre-hunter-cohort-preparation');
  assert.equal(m.cohort.opening_tick,144000);verifySeeds(m.cohort.openings);
}
export function verifyBaseline(b) {
  for(const key of ['material','energy','water']) {
    positive(b[key]);assert.equal(b.limits[key],1e-8*Math.max(b[key],1),'opening-scaled fixed limit');
  }
}
export function verifyOpening(o,reference,index,base,source) {
  verifyBaseline(base);assert.equal(o.arm,ARMS[index]);assert.equal(o.dose_permille,DOSES[index]);
  assert.equal(o.candidate,index>=3);assert.equal(o.natural_rain_percent,index>=3?90:100);
  assert.equal(o.opening_tick,144000);assert.equal(o.care_kind,'rain');
  assert.equal(o.care_start_tick,60);assert.equal(o.care_period_ticks,2400);
  assert.deepEqual(o.targets,TARGETS);assert.equal(o.region_hops,3);assert.deepEqual(o.region_cells,CELLS);
  assert.deepEqual(o.pre_intervention_baseline,base);
  assert.equal(o.config.seed,source.seed);assert(o.opening_rain_rate>0&&Number.isFinite(o.opening_rain_rate));
  assert.equal(o.opening_rain_rate,reference.opening_rain_rate);
  assert.equal(o.arm_rain_rate,reference.opening_rain_rate*(index>=3?.9:1));
  assert.equal(o.config.water.rain_rate,o.arm_rain_rate);
  const config=structuredClone(o.config);config.water.rain_rate=reference.config.water.rain_rate;
  assert.deepEqual(config,reference.config,'only rain_rate may differ');
  assert.equal(o.state_hash_before_change,reference.state_hash_before_change);
  hash64(o.state_hash_before_change);hash64(o.state_hash_after_change);hash64(o.ecology_hash_after_change);
  assert.equal(o.rain_rate_bits_unchanged,index<3);
  if(index<3) {
    assert.equal(o.state_hash_after_change,o.state_hash_before_change);
    assert.equal(o.ecology_hash_after_change,source.ecology_hash);
    assert.equal(o.config_sha256,reference.config_sha256);
  } else assert.notEqual(o.state_hash_before_change,o.state_hash_after_change);
  assert.match(o.config_sha256,/^[a-f0-9]{64}$/);
}
export function verifySummary(a,o,base,source) {
  for(const flag of ['technical_complete','complete_experiment_measurement','audit_passed']) assert.equal(a[flag],true,flag);
  assert.equal(a.arm,o.arm);assert.equal(a.horizon,'two-hour');assert.equal(a.termination,'planned_horizon');
  assert.equal(a.planned_ticks,144000);assert.equal(a.elapsed_ticks,144000);assert.equal(a.closing_tick,288000);
  assert.deepEqual(a.pre_intervention_baseline,base);verifyBaseline(base);assert.equal(a.rain_rate,o.arm_rain_rate);
  for(const [v,limit] of [[a.max_absolute_drift.material,base.limits.material],
    [a.max_absolute_drift.water,base.limits.water],[a.corrected_energy_drift,base.limits.energy],
    [a.independent_windowed_energy_drift,base.limits.energy],[a.care_boundary_energy_drift,base.limits.energy]]) {
    positive(v);assert(v<limit,'audit reaches/exceeds fixed limit');
  }
  positive(a.max_absolute_drift.energy);
  assert.equal(a.legacy_audit_passed,a.max_absolute_drift.material<base.limits.material&&
    a.max_absolute_drift.energy<base.limits.energy&&a.max_absolute_drift.water<base.limits.water);
  for(const k of ['population_min','population_max','closing_population','surviving_opening_cohorts','maximum_descendant_depth'])count(a[k]);
  assert(a.population_min<=source.population&&a.population_max>=source.population);
  assert(a.population_min<=a.closing_population&&a.closing_population<=a.population_max);
  assert(a.surviving_opening_cohorts<=Math.min(source.population,a.closing_population));
  if(a.first_extinction_tick!==null) {
    count(a.first_extinction_tick);assert(a.first_extinction_tick>=144000&&a.first_extinction_tick<=288000);
    assert.equal(a.population_min,0);assert.equal(a.closing_population,0);
  } else assert(a.population_min>0);
  hash64(a.closing_state_hash);hash64(a.closing_ecology_hash);
  const w=a.water;for(const k of ['total_rain_in','manual_attributed','evap_out','closing_sampled'])positive(w[k]);
  close(w.natural,w.total_rain_in-w.manual_attributed,base.limits.water,'natural attribution');
  assert(w.natural>-base.limits.water,'materially negative natural attribution');
  close(w.closing_sampled-base.water,w.total_rain_in-w.evap_out,base.limits.water,'closing water budget');
}
export class Receipts {
  constructor(dose) {this.dose=dose;this.rows=[];this.depth=0;this.outcomes={applied:0,rejected:0};this.reasons={};}
  add(r) {
    assert.notEqual(this.dose,null,'no-input arm received care');assert(this.rows.length<60,'excess receipts');
    const n=this.rows.length,elapsed=60+2400*n,target=n%3;
    assert.equal(r.elapsed,elapsed);assert.equal(r.tick,144000+elapsed);assert.equal(r.kind,'rain');
    assert.equal(r.target_index,target);assert.deepEqual(r.target,TARGETS[target]);assert.equal(r.dose_permille,this.dose);
    assert.equal(r.receipt.tick,r.tick);assert.equal(r.receipt.seq,n+1);
    const keys=Object.keys(r.receipt.outcome);assert.equal(keys.length,1);
    let depth=0;
    if(keys[0]==='Applied') {
      const q=r.receipt.outcome.Applied;assert.equal(r.outcome,'applied');assert.equal(r.reason,null);
      assert.equal(q.cells,[13,13,9][target]);assert.equal(q.ends_tick,r.tick+120);
      for(const k of ['material_in','material_out','energy_in','energy_out'])assert.equal(q[k],0);
      assert.equal(q.water_depth,4*this.dose/1000);depth=q.water_depth;this.outcomes.applied++;
    } else {
      assert.equal(keys[0],'Rejected','frozen rain has no Partial outcome');assert.equal(r.outcome,'rejected');
      assert.equal(typeof r.reason,'string');assert(r.reason.length>0);assert.equal(r.receipt.outcome.Rejected,r.reason);
      this.outcomes.rejected++;this.reasons[r.reason]=(this.reasons[r.reason]||0)+1;
    }
    this.depth+=depth;this.rows.push({elapsed,depth,cumulative:this.depth});
  }
  finish(a,limit) {
    assert.equal(this.rows.length,this.dose===null?0:60,'missing receipts');
    assert.equal(a.care.dose_permille,this.dose);assert.equal(a.care.attempts,this.rows.length);
    assert.equal(a.care.applied,this.outcomes.applied);assert.equal(a.care.rejected,this.outcomes.rejected);assert.equal(a.care.partial,0);
    const l=a.care.ledgers;assert.equal(l.admitted_seq,this.rows.length);assert.deepEqual(l.showers,[]);
    for(const k of ['feed_material_in','feed_energy_in','clean_material_out','clean_energy_out','allowance_used'])assert.equal(l[k],0);
    close(l.rain_depth_in,this.depth,limit,'delivered scheduled rain');
    assert.equal(l.rain_depth_in,a.water.manual_attributed);
  }
}
export function verifyLocal(l,prior,elapsed,flood,water) {
  assert.equal(l.observed_ticks,elapsed);assert.equal(l.opening_tick,144000);assert.equal(l.flood_threshold,flood);
  assert.equal(l.targets.length,3);
  for(const [i,r] of l.targets.entries()) {
    assert.equal(r.cells,CELLS[i]);assert.deepEqual(r.target,TARGETS[i]);
    for(const k of ['water_sampled','water_tick_integral','producer','fruit','detritus','nutrient'])positive(r[k]);
    assert(r.water_sampled<=water+1e-10*Math.max(1,water));
    for(const k of ['flooded_cell_ticks','drowned_cell_ticks']) {count(r[k]);assert(r[k]<=r.cells*elapsed);if(prior)assert(r[k]>=prior.targets[i][k]&&r[k]-prior.targets[i][k]<=r.cells*200);}
    assert(r.drowned_cell_ticks<=r.flooded_cell_ticks);
    if(prior)assert(r.water_tick_integral>=prior.targets[i].water_tick_integral);
  }
  positive(l.global_water_tick_integral);if(prior)assert(l.global_water_tick_integral>=prior.global_water_tick_integral);
  for(const k of ['global_flooded_cell_ticks','global_drowned_cell_ticks']) {count(l[k]);assert(l[k]<=1280*elapsed);if(prior)assert(l[k]>=prior[k]&&l[k]-prior[k]<=1280*200);}
  for(const k of ['global_flooded_cells_now','global_drowned_cells_now']){count(l[k]);assert(l[k]<=1280);}
  assert(l.global_drowned_cell_ticks<=l.global_flooded_cell_ticks);assert(l.global_drowned_cells_now<=l.global_flooded_cells_now);
}
export class Samples {
  constructor(source,o,a,receipts) {
    Object.assign(this,{source,o,a,receipts,n:0,previous:null,births:0,capRejections:0,deaths:{starvation:0,age:0,collapse:0},
      sums:{population:0,occupied_cells:0,producer:0,fruit:0},forms:Array(8).fill(0),
      waterFaces:Array(5).fill(0),localSums:Array.from({length:3},()=>({producer:0,fruit:0})),
      minima:source.population_by_form.slice(),firstZero:source.population_by_form.map(n=>n===0?144000:null),rain:0,evap:0});
  }
  add(s) {
    assert(this.n<720,'extra sample');const elapsed=(this.n+1)*200,prior=this.previous,limit=this.a.pre_intervention_baseline.limits.water;
    assert.equal(s.elapsed,elapsed);assert.equal(s.tick,144000+elapsed);
    count(s.population);vector(s.population_by_form,8,count);vector(s.population_by_face,5,count);
    assert.equal(total(s.population_by_form),s.population);assert.equal(total(s.population_by_face),s.population);
    count(s.occupied_cells);assert(s.occupied_cells<=Math.min(1280,s.population));
    assert.deepEqual(Object.keys(s.window_deaths).sort(),['age','collapse','starvation']);
    count(s.window_births);for(const k of Object.keys(this.deaths))count(s.window_deaths[k]);count(s.window_cap_rejections);
    assert.equal(s.population,(prior?.population??this.source.population)+s.window_births-total(Object.values(s.window_deaths)),'population/window events');
    assert(s.population>=this.a.population_min&&s.population<=this.a.population_max);
    count(s.surviving_opening_cohorts);assert(s.surviving_opening_cohorts<=Math.min(s.population,prior?.surviving_opening_cohorts??this.source.population));
    count(s.maximum_descendant_depth);assert(s.maximum_descendant_depth>=(prior?.maximum_descendant_depth??0));
    for(const k of ['water_sampled','producer','fruit','detritus','nutrient','window_rain_in','window_evap_out',
      'cumulative_rain_total','cumulative_rain_manual_attributed'])positive(s[k]);
    vector(s.water_by_face_sampled,5);vector(s.producer_by_face,5);
    close(total(s.water_by_face_sampled),s.water_sampled,limit,'water face total');
    close(total(s.producer_by_face),s.producer,1e-10*Math.max(1,s.producer),'producer face total');
    close(s.water_sampled-(prior?.water_sampled??this.a.pre_intervention_baseline.water),s.window_rain_in-s.window_evap_out,limit,'window water budget');
    this.rain+=s.window_rain_in;this.evap+=s.window_evap_out;
    close(s.cumulative_rain_total,this.rain,limit,'cumulative/window rain');
    close(s.cumulative_rain_natural,s.cumulative_rain_total-s.cumulative_rain_manual_attributed,limit,'sample attribution');
    assert(s.cumulative_rain_natural>-limit); // Preserve small signed subtraction noise, never clamp.
    const admitted=this.receipts.rows.filter(r=>r.elapsed<elapsed);
    assert.equal(s.receipts_so_far,admitted.length);close(s.cumulative_rain_manual_attributed,admitted.at(-1)?.cumulative??0,limit,'sample manual delivery');
    verifyLocal(s.local,prior?.local,elapsed,this.o.config.water.flood,s.water_sampled);
    hash64(s.state_hash);hash64(s.ecology_hash);
    this.births+=s.window_births;this.capRejections+=s.window_cap_rejections;
    for(const k of Object.keys(this.deaths))this.deaths[k]+=s.window_deaths[k];
    for(const k of Object.keys(this.sums))this.sums[k]+=s[k];
    s.water_by_face_sampled.forEach((v,i)=>this.waterFaces[i]+=v);
    s.local.targets.forEach((r,i)=>{for(const k of Object.keys(this.localSums[i]))this.localSums[i][k]+=r[k];});
    s.population_by_form.forEach((v,i)=>{this.forms[i]+=v;this.minima[i]=Math.min(this.minima[i],v);if(v===0&&this.firstZero[i]===null)this.firstZero[i]=s.tick;});
    this.previous=s;this.n++;
  }
  finish() {
    assert.equal(this.n,720,'missing samples');const s=this.previous,a=this.a,limit=a.pre_intervention_baseline.limits.water;
    assert.equal(s.state_hash,a.closing_state_hash);assert.equal(s.ecology_hash,a.closing_ecology_hash);
    assert.equal(s.population,a.closing_population);assert.equal(s.surviving_opening_cohorts,a.surviving_opening_cohorts);
    assert.equal(s.maximum_descendant_depth,a.maximum_descendant_depth);assert.deepEqual(s.local,a.local);
    assert.equal(s.water_sampled,a.water.closing_sampled);
    assert.equal(s.cumulative_rain_total,a.water.total_rain_in);assert.equal(s.cumulative_rain_natural,a.water.natural);
    close(this.evap,a.water.evap_out,limit,'cumulative/window evaporation');
    return {opening_population:this.source.population,closing_population:a.closing_population,
      population_min:a.population_min,population_max:a.population_max,first_extinction_tick:a.first_extinction_tick,
      births:this.births,deaths:this.deaths,cap_rejections:this.capRejections,
      closing_state_hash:a.closing_state_hash,closing_snapshot_sha256:a.closing_snapshot_sha256,
      sampled_means:Object.fromEntries(Object.entries(this.sums).map(([k,v])=>[k,v/720])),
      forms:this.forms.map((sum,i)=>({form:i,opening:this.source.population_by_form[i],closing:s.population_by_form[i],
        sampled_mean:sum/720,sampled_min:this.minima[i],first_sampled_zero:this.firstZero[i],
        newly_sampled_zero:this.source.population_by_form[i]>0&&this.firstZero[i]!==null})),
      surviving_opening_cohorts:a.surviving_opening_cohorts,maximum_descendant_depth:a.maximum_descendant_depth,
      water:a.water,mean_water_over_ticks:a.local.global_water_tick_integral/144000,
      mean_sampled_water_by_face:this.waterFaces.map(v=>v/720),
      global_flooded_cell_ticks:a.local.global_flooded_cell_ticks,global_drowned_cell_ticks:a.local.global_drowned_cell_ticks,
      local:a.local.targets.map((r,i)=>({...r,mean_water_over_ticks:r.water_tick_integral/144000,
        mean_sampled_producer:this.localSums[i].producer/720,mean_sampled_fruit:this.localSums[i].fruit/720})),
      receipts:{attempts:this.receipts.rows.length,...this.receipts.outcomes,reasons:this.receipts.reasons,scheduled_depth:this.receipts.depth},
      audits:{legacy_passed:a.legacy_audit_passed,raw_energy:a.max_absolute_drift.energy,
        material:a.max_absolute_drift.material,water:a.max_absolute_drift.water,corrected_energy:a.corrected_energy_drift,
        independent_energy:a.independent_windowed_energy_drift,care_boundary_energy:a.care_boundary_energy_drift,
        limits:a.pre_intervention_baseline.limits}};
  }
}
async function lines(path,visit) {
  const stream=createReadStream(path,{encoding:'utf8'}), reader=createInterface({input:stream,crlfDelay:Infinity});
  try {for await(const line of reader) {assert(line.length>0&&line.length<65536,'empty/oversized JSONL record');visit(JSON.parse(line));}}
  finally {reader.close();stream.destroy();}
}
export function contrasts(arms) {
  const difference=(a,b)=>({sampled_population:a.sampled_means.population-b.sampled_means.population,
    closing_population:a.closing_population-b.closing_population,births:a.births-b.births,
    deaths:total(Object.values(a.deaths))-total(Object.values(b.deaths)),ancestry:a.surviving_opening_cohorts-b.surviving_opening_cohorts,
    mean_water_over_ticks:a.mean_water_over_ticks-b.mean_water_over_ticks,
    flooded_cell_ticks:a.global_flooded_cell_ticks-b.global_flooded_cell_ticks,
    local_mean_water:a.local.map((r,i)=>r.mean_water_over_ticks-b.local[i].mean_water_over_ticks)});
  const support=[0,1,2].map(i=>({dose:DOSES[i],difference_90_minus_100:difference(arms[i+3],arms[i])}));
  const care=[0,3].map(i=>({natural_fraction:i===0?1:.9,standard_minus_none:difference(arms[i+1],arms[i]),generous_minus_none:difference(arms[i+2],arms[i])}));
  return {support,care,interaction:[1,2].map(i=>({dose:DOSES[i],sampled_population:
    (arms[i+3].sampled_means.population-arms[3].sampled_means.population)-(arms[i].sampled_means.population-arms[0].sampled_means.population)}))};
}
export async function reduceAmbient(directory) {
  const root=resolve(directory), failures=[], seeds=[];
  const refusal=()=>({kind:'ambient-support-reduction',artifact_checks_passed:false,biological_acceptance:false,failures,seeds});
  let manifest,aggregate;
  try {manifest=await json(join(root,'manifest.json'));verifyManifest(manifest);
    assert.equal(sha(await readFile(join(root,'ambient_compare.frozen'))),CONTRACT.sha);
    aggregate=await json(join(root,'summary.json'));verifySeeds(aggregate.seeds);
    for(const k of ['technical_complete','complete_experiment_measurement','audit_passed'])assert.equal(aggregate[k],true,`aggregate ${k}`);
    assert.equal(aggregate.horizon,'two-hour');assert.equal(aggregate.ticks,144000);
  } catch(e) {
    failures.push({scope:'run',error:e.message});
    for(let seed=1;seed<=12;seed++) {try{const r=await json(join(root,`seed-${seed}/result.json`));seeds.push({seed,available:true,technical_complete:r.technical_complete,failure:r.failure,
      arms:r.arms?.map(a=>({arm:a.arm,technical_complete:a.technical_complete,audit_passed:a.audit_passed,
        termination:a.termination,error:a.error}))});}
      catch(error){seeds.push({seed,available:false,error:error.message});}}
    return refusal();
  }
  const cohortDir=resolve(dirname(fileURLToPath(import.meta.url)),'../captures/hunter-openings-2026-09-13');
  for(let seed=1;seed<=12;seed++) {
    const dir=join(root,`seed-${seed}`), source=manifest.cohort.openings.find(o=>o.seed===seed), arms=[];
    try {
      const initial=inspectSnapshot(await readFile(join(cohortDir,`seed-${seed}/world-144000.cubw`)),9);
      for(const k of Object.keys(initial))assert.equal(initial[k],source[k],`opening snapshot ${k}`);
      vector(source.population_by_form,8,count);assert.equal(total(source.population_by_form),source.population);
      const result=await json(join(dir,'result.json'));
      assert.deepEqual(result,aggregate.seeds.find(s=>s.seed===seed));assert.equal(result.technical_complete,true);assert.equal(result.failure,null);
      assert.deepEqual(result.opening,source);assert.equal(result.arms.length,6);verifyBaseline(result.pre_intervention_baseline);
      const openings=await Promise.all(ARMS.map(name=>json(join(dir,name,'opening.json'))));
      for(const [i,name] of ARMS.entries()) {
        try {
          const path=join(dir,name),o=openings[i],a=await json(join(path,'summary.json'));
          verifyOpening(o,openings[0],i,result.pre_intervention_baseline,source);assert.deepEqual(a,result.arms[i]);
          verifySummary(a,o,result.pre_intervention_baseline,source);
          const snap=inspectSnapshot(await readFile(join(path,'closing.cubw')),12);
          assert.equal(snap.sha256,a.closing_snapshot_sha256);assert.equal(snap.state_hash,a.closing_state_hash);assert.equal(snap.build,CONTRACT.build);
          const receipts=new Receipts(DOSES[i]);await lines(join(path,'receipts.jsonl'),r=>receipts.add(r));receipts.finish(a,result.pre_intervention_baseline.limits.water);
          const samples=new Samples(source,o,a,receipts);await lines(join(path,'samples.jsonl'),s=>samples.add(s));arms.push({arm:name,verified:true,...samples.finish()});
        } catch(e) {failures.push({seed,arm:name,error:e.message});arms.push({arm:name,verified:false,error:e.message});}
      }
      if(arms.every(a=>a.verified)) {
        const limit=result.pre_intervention_baseline.limits.water;
        for(const start of [0,3])for(const dose of [1,2])close(arms[start+dose].water.natural,arms[start].water.natural,limit,'natural input dose independence');
        for(const i of [0,1,2])close(arms[i+3].water.natural,.9*arms[i].water.natural,limit,'natural input support ratio');
        for(const i of [1,2])close(arms[i+3].water.manual_attributed,arms[i].water.manual_attributed,limit,'manual dose support independence');
        for(const i of [1,2]) {
          assert.equal(openings[i+3].state_hash_after_change,openings[3].state_hash_after_change);
          assert.equal(openings[i+3].config_sha256,openings[3].config_sha256);
        }
        seeds.push({seed,verified:true,opening:{sha256:source.sha256,state_hash:source.state_hash,
          ecology_hash:source.ecology_hash,population_stratum:source.population_stratum},arms,contrasts:contrasts(arms)});
      } else seeds.push({seed,verified:false,arms});
    } catch(e) {failures.push({seed,error:e.message});seeds.push({seed,verified:false,arms});}
  }
  if(failures.length)return refusal();
  return {kind:'ambient-support-reduction',artifact_checks_passed:true,biological_acceptance:false,contract:CONTRACT,
    limits:'Recorded strict audits and snapshot envelopes checked, not semantic snapshot decoding or independent material/energy transaction replay. Biology means/minima/form losses are 200-tick sampled, not per-tick trajectories. Water integrals are actual per-tick depth-time. Ancestry means descendants of organisms alive at opening, not original founder lineage. No claim of unattended viability or required care.',seeds};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href) {
  if(process.argv.length!==3) {console.error('Usage: node scripts/reduce-ambient-support.mjs RUN_DIRECTORY');process.exitCode=1;}
  else {const result=await reduceAmbient(process.argv[2]);console.log(JSON.stringify(result,null,2));if(!result.artifact_checks_passed)process.exitCode=1;}
}
