import {test} from 'node:test';
import assert from 'node:assert/strict';
import {reduceArm,aggregate} from './reduce-fauna-development.mjs';
function fixture() {
  const individual=(generation,opening,first,last)=>({id:{slot:4,generation},form:3,
    opening_member:opening,initial:{adult_fraction:first,tick:144000},
    last:{adult_fraction:last,tick:156000},
    counts:{ticks:10,immature:first<1?10:0,fed:3,reserve_zero:2,observed_escrow_starts:0}});
  const a=individual(1,true,0.4,0.8); a.last.tick=144500;
  const b=individual(2,false,0.4,0.5); b.initial.tick=145000;
  return {result:{technical_complete:true,audit_passed:true,population:1,
    observation:{opening_tick:144000,closing_tick:156000,individuals:[a,b]}},
    events:[{stream:'life',event:{kind:'death',id:a.id,tick:144501,cause:'Starvation'}},
      {stream:'life',event:{kind:'birth',id:b.id,tick:145000}}]};
}
test('full generations distinguish a juvenile death from a censored replacement',()=>{
  const {result,events}=fixture(), forms=reduceArm(result,events), f=forms[3];
  assert.equal(f.opening_juveniles,1); assert.equal(f.opening_juveniles_died,1);
  assert.equal(f.born,1); assert.equal(f.closing,1); assert.equal(f.deaths_last_observed_immature,1);
  const totals=aggregate([{forms}]); assert.equal(totals[0].fed_fraction,null);
  assert.equal(totals[3].fed_fraction,0.3); assert.equal(totals[3].immature_fraction,1);
});
test('unknown or duplicate life events and missing censored survivors are refused',()=>{
  for (const alter of [
    x=>x.events.push(x.events[0]),
    x=>x.events[0].event.id={slot:99,generation:1},
    x=>x.result.observation.individuals[1].last.tick=155999,
    x=>x.events.pop(),
    x=>x.result.population=2,
    x=>x.result.observation.individuals[0].last.tick=144501,
    x=>x.result.observation.individuals[1].initial.tick=145001,
    x=>x.result.observation.individuals[0].opening_member='yes',
  ]) {const x=fixture(); alter(x); assert.throws(()=>reduceArm(x.result,x.events));}
  assert.throws(()=>aggregate([]));
});
