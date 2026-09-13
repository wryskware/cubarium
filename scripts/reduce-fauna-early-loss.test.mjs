import {test} from 'node:test';
import assert from 'node:assert/strict';
import {parseTelemetry,reduceSamples} from './reduce-fauna-early-loss.mjs';

function fixture() {
  return [2,1,0,0].map((n,i)=>({tick:(i+1)*100,population:n,
    population_by_form:[0,0,0,n,0,0,0,0],births:0,
    deaths_starvation:i===1||i===2?1:0,deaths_age:0,deaths_collapse:0,
    care_admitted_seq:0,care_feed_material_in:0,care_feed_energy_in:0,
    care_rain_depth_in:0,care_clean_material_out:0,care_clean_energy_out:0,
    care_allowance_used:0,fruit:0,water:0,producer:0,detritus:0,
    organism_material:n,organism_energy:n,state_hash:'18446744073709551615'}));
}
test('census loss is bracketed, never treated as exact death or initial absence',()=>{
  const r=reduceSamples(fixture(),400);
  assert.deepEqual(r.forms[3].first_loss_bracket,{after_tick:200,by_tick:300});
  assert.deepEqual(r.forms[3].first_net_decline,{after_tick:100,by_tick:200,from:2,to:1});
  assert.equal(r.forms[3].last_positive_tick,200);
  assert.equal(r.forms[3].zero_samples,2);
  assert.equal(r.forms[0].first_absence,100);
  assert.equal(r.forms[0].first_loss_bracket,null);
});
test('censoring and sampled reappearance remain visible',()=>{
  const rows=fixture(); rows[3].population=1; rows[3].population_by_form[3]=1; rows[3].births=1;
  assert.deepEqual(reduceSamples(rows,400).forms[3].sampled_reappearances,[400]);
  const retained=fixture().slice(0,2);
  assert.equal(reduceSamples(retained,200).forms[3].first_loss_bracket,null);
});
test('truncated, reordered, malformed, care-contaminated or inconsistent samples refuse',()=>{
  for (const mutate of [
    r=>r.pop(),r=>r.reverse(),r=>r[2].tick=200,r=>r[1].births=1,
    r=>r[1].population_by_form[0]=1,r=>r[1].care_admitted_seq=1,
    r=>r[1].water=NaN,r=>r[1].state_hash=123,r=>r[1].births=-1,
  ]) { const r=fixture(); mutate(r); assert.throws(()=>reduceSamples(r,400)); }
  assert.throws(()=>parseTelemetry('{}'));
});
test('u64 telemetry hashes are parsed losslessly',()=>{
  const [r]=parseTelemetry('{"state_hash":18446744073709551615,"ecology_hash":18446744073709551614}\n');
  assert.equal(r.state_hash,'18446744073709551615');
  assert.equal(r.ecology_hash,'18446744073709551614');
});
