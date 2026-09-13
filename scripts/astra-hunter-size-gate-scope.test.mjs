import assert from 'node:assert/strict';
import test from 'node:test';
import {reductionScope} from './hunter-size-gate-parity.mjs';
const seeds = Array.from({length:12},(_,i)=>i+1);
const complete = {ran_seeds:seeds,cohort_seeds:seeds,seed_subset_pilot:false};
test('full cohort metadata requires all prescribed seeds in both manifests',()=>{
  const r = reductionScope(seeds,complete,complete);
  assert.equal(r.pilot,false); assert.equal(r.kind,'hunter-size-gate-cohort-parity');
  assert.match(r.pilot_note,/not biological acceptance/);
});
test('subset, duplicated or unsupported seed declarations never masquerade as full cohort',()=>{
  for(const s of [[1],[],[...seeds.slice(0,11),11],[...seeds.slice(0,11),13]])
    assert.equal(reductionScope(s,complete,complete).pilot,true);
  for(const bad of [{...complete,ran_seeds:[1]}, {...complete,cohort_seeds:[1]},
    {...complete,seed_subset_pilot:true},{}]) {
    assert.equal(reductionScope(seeds,bad,complete).pilot,true);
    assert.equal(reductionScope(seeds,complete,bad).pilot,true);
  }
});
