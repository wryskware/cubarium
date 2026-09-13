// Independent, pure reducer fixtures. No experiment, process polling or artifact mutation.
import test from 'node:test';
import assert from 'node:assert/strict';
import {contractFor, Receipts, Samples, verifySummary, TARGETS} from './reduce-ambient-support.mjs';

const source={population:2,population_by_form:[2,0,0,0,0,0,0,0]};
const base={material:100,energy:100,water:10,limits:{material:1e-6,energy:1e-6,water:1e-7}};
function local(elapsed,producer) {
  return {observed_ticks:elapsed,opening_tick:144000,flood_threshold:1.5,
    global_water_tick_integral:10*elapsed,global_flooded_cell_ticks:0,global_drowned_cell_ticks:0,
    global_flooded_cells_now:0,global_drowned_cells_now:0,
    targets:TARGETS.map((target,i)=>({target,cells:[25,25,16][i],water_sampled:2**i,
      water_tick_integral:2**i*elapsed,producer:producer/2**i,fruit:producer/8/2**i,
      detritus:0,nutrient:0,flooded_cell_ticks:0,drowned_cell_ticks:0}))};
}
function summary(horizon) {
  const {ticks,opening}=contractFor(horizon);
  return {arm:'rain100_none',horizon,technical_complete:true,complete_experiment_measurement:true,audit_passed:true,
    termination:'planned_horizon',planned_ticks:ticks,elapsed_ticks:ticks,closing_tick:opening+ticks,rain_rate:.6,
    pre_intervention_baseline:base,max_absolute_drift:{material:0,energy:0,water:0},corrected_energy_drift:0,
    independent_windowed_energy_drift:0,care_boundary_energy_drift:0,legacy_audit_passed:true,
    population_min:2,population_max:4,closing_population:4,surviving_opening_cohorts:2,maximum_descendant_depth:1,
    first_extinction_tick:null,closing_state_hash:'1',closing_ecology_hash:'2',
    water:{natural:0,total_rain_in:0,manual_attributed:0,evap_out:0,closing_sampled:10},local:local(ticks,24)};
}

for(const [horizon,ticks,rows] of [['twenty-four-hour',1728000,8640],['seventy-two-hour',5184000,25920]]) {
  test(`${horizon}: nonconstant populations, form, field and local means have exact selected denominators`,()=>{
    const a=summary(horizon),o={arm:a.arm,arm_rain_rate:.6,config:{water:{flood:1.5}}};
    verifySummary(a,o,base,source,horizon);
    assert.equal(a.closing_tick,144000+ticks);
    const stream=new Samples(source,o,a,new Receipts(null,horizon),horizon);
    for(let n=1;n<=rows;n++) {
      const late=n>rows/2,elapsed=n*200,population=late?4:2,producer=late?24:8;
      stream.add({elapsed,tick:144000+elapsed,population,population_by_form:[population,0,0,0,0,0,0,0],
        population_by_face:[population,0,0,0,0],occupied_cells:population,
        window_births:n===rows/2+1?2:0,window_deaths:{age:0,collapse:0,starvation:0},window_cap_rejections:0,
        surviving_opening_cohorts:2,maximum_descendant_depth:late?1:0,
        water_sampled:10,water_by_face_sampled:[10,0,0,0,0],producer,producer_by_face:[producer,0,0,0,0],
        fruit:producer/8,detritus:0,nutrient:0,window_rain_in:0,window_evap_out:0,
        cumulative_rain_total:0,cumulative_rain_manual_attributed:0,cumulative_rain_natural:0,
        receipts_so_far:0,state_hash:'1',ecology_hash:'2',local:local(elapsed,producer)});
      if(n===720)assert.throws(()=>stream.finish(),'a complete 2h prefix is not the longer run');
    }
    const out=stream.finish();
    assert.equal(out.births,2);assert.equal(out.sampled_means.population,3);assert.equal(out.forms[0].sampled_mean,3);
    assert.equal(out.sampled_means.producer,16);assert.equal(out.sampled_means.fruit,2);
    assert.equal(out.mean_water_over_ticks,10);assert.deepEqual(out.mean_sampled_water_by_face,[10,0,0,0,0]);
    assert.deepEqual(out.local.map(x=>x.mean_water_over_ticks),[1,2,4]);
    assert.deepEqual(out.local.map(x=>x.mean_sampled_producer),[16,8,4]);
    assert.deepEqual(out.local.map(x=>x.mean_sampled_fruit),[2,1,.5]);
    const bad=structuredClone(a);bad.corrected_energy_drift=base.limits.energy;
    assert.throws(()=>verifySummary(bad,o,base,source,horizon),'longer horizon keeps strict opening limit');
    assert.throws(()=>verifySummary(a,o,base,source),'long horizon cannot pass implicit two-hour selection');
  });
  test(`${horizon}: Standard dose retains scheduled refusals through the exact last shower`,()=>{
    const r=new Receipts(1000,horizon),attempts=ticks/2400;
    let rejected=0;
    for(let n=0;n<attempts;n++) {
      const elapsed=60+2400*n,tick=144000+elapsed,target=n%3,refused=n%37===0;
      if(refused)rejected++;
      r.add({elapsed,tick,kind:'rain',target_index:target,target:TARGETS[target],dose_permille:1000,
        outcome:refused?'rejected':'applied',reason:refused?'shower active':null,
        receipt:{tick,seq:n+1,outcome:refused?{Rejected:'shower active'}:{Applied:{
          cells:[13,13,9][target],ends_tick:tick+120,material_in:0,material_out:0,energy_in:0,energy_out:0,water_depth:4}}}});
    }
    assert.equal(r.rows.length,attempts);assert.equal(r.rows.at(-1).elapsed,ticks-2340);
    const delivered=4*(attempts-rejected);
    r.finish({care:{dose_permille:1000,attempts,applied:attempts-rejected,rejected,partial:0,
      ledgers:{admitted_seq:attempts,showers:[],feed_material_in:0,feed_energy_in:0,clean_material_out:0,
        clean_energy_out:0,allowance_used:0,rain_depth_in:delivered}},water:{manual_attributed:delivered}},base.limits.water);
    assert.equal(r.depth,delivered);assert.equal(r.reasons['shower active'],rejected);
  });
}
