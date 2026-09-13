import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {verifyArm, reduceEvents, verifyRecipePair, metrics} from './compare-hunter-recipes.mjs';
import {inspectSnapshot} from './prepare-hunter-worlds.mjs';

function arm() {
  const peak = () => ({magnitude:1e-12, first_crossing:null, nonfinite_at:null});
  return {
    technical_complete:true, complete_experiment_measurement:true, planned_ticks:144000,
    closing_tick:288000,last_complete_observer_tick:288000,last_complete_reproduction_tick:288000,
    observer_statistics_trusted_through_closing_tick:true,
    audit:{passed:true,opening_tick:144000,fixed_limits:[1e-6,1e-6,1e-6],
      material:peak(),corrected_energy:peak(),independent_energy:peak(),water:peak()},
    adult_occupancy_ticks_0_1_2_over2:[144000,0,0,0],offspring:0,open_gestations:0,
    reproduction_audit:{last_complete_tick:288000,ticks_observed:144000,
      counts:{funded:0,closed:0,open_at_horizon:0,born:0,refunded:0,miscarried:0}},
    reproductive_opportunity:{member_ticks:0,age_and_size_ready_member_ticks:0,
      age_and_size_ready_reserve_gate_open_member_ticks:0,
      age_and_size_ready_energy_gate_open_member_ticks:0,
      age_and_size_ready_both_stock_gates_open_member_ticks:0},
    whole_recovery:Array.from({length:9},()=>({opening_count:0,minimum_count:0,first_zero_tick:144000,ticks_below_half:0})),
    captures:0,closing_population:10,closing_hunters:0,
  };
}
const record = (kind, extra={}, tick=144001) => ({stream:'hunter',event:{tick,kind,...extra}});

test('complete arm requires coverage, numerical limits and balanced reproduction counts', () => {
  verifyArm(arm(),144000,144000);
  for (const mutate of [
    a=>a.technical_complete=false, a=>a.complete_experiment_measurement=false,
    a=>a.last_complete_observer_tick--,a=>a.last_complete_reproduction_tick--,
    a=>a.reproduction_audit.ticks_observed--,a=>a.adult_occupancy_ticks_0_1_2_over2[0]--,
    a=>a.reproduction_audit.counts.funded++,a=>a.audit.material.magnitude=NaN,
    a=>a.audit.independent_energy.magnitude=2e-6,a=>a.audit.water.first_crossing=145000,
    a=>a.reproductive_opportunity.age_and_size_ready_member_ticks++,
  ]) {const a=arm();mutate(a);assert.throws(()=>verifyArm(a,144000,144000));}
});

test('zero mature observations are null, not zero-percent stock success', () => {
  const a=arm(), b={summary:a,events:{paid_energy:0},local:{}};
  assert.equal(metrics(b).mature_joint_stock_open_fraction,null);
  a.reproductive_opportunity.member_ticks=10;
  a.reproductive_opportunity.age_and_size_ready_member_ticks=10;
  assert.equal(metrics(b).mature_joint_stock_open_fraction,0);
  a.reproductive_opportunity.age_and_size_ready_reserve_gate_open_member_ticks=5;
  assert.equal(metrics(b).mature_reserve_open_fraction,0.5);
  assert.equal(metrics(b).paid_energy_per_capture,null);
});

test('audit limits are strict and joint stock counts obey the intersection lower bound', () => {
  const a=arm();
  a.audit.material.magnitude=a.audit.fixed_limits[0];
  assert.throws(()=>verifyArm(a,144000,144000),/reaches\/exceeds/);
  a.audit.material.magnitude=1e-12;
  const q=a.reproductive_opportunity;
  q.member_ticks=100;q.age_and_size_ready_member_ticks=100;
  q.age_and_size_ready_reserve_gate_open_member_ticks=80;
  q.age_and_size_ready_energy_gate_open_member_ticks=70;
  q.age_and_size_ready_both_stock_gates_open_member_ticks=49;
  assert.throws(()=>verifyArm(a,144000,144000),/stock denominators/);
  q.age_and_size_ready_both_stock_gates_open_member_ticks=50;
  verifyArm(a,144000,144000);
});

test('recipe pair permits only the two reserve fields, not hidden costs or imports', () => {
  const a={profile_recipe:'baseline',profile:{seek_reserve_fraction:.35,perch_reserve_fraction:.65,cost:1},receipt:{material:4}};
  const b={...structuredClone(a),profile_recipe:'reserve-targets-v1'};
  b.profile.seek_reserve_fraction=.8;b.profile.perch_reserve_fraction=.9;
  verifyRecipePair(a,b);
  b.profile.cost=0;assert.throws(()=>verifyRecipePair(a,b),/profile field/);b.profile.cost=1;
  b.receipt.material=5;assert.throws(()=>verifyRecipePair(a,b),/receipt/);
  verifyRecipePair({profile_recipe:'baseline',profile:null},{profile_recipe:'reserve-targets-v1',profile:null});
});

test('event reduction separates paid attempts, inward/far misses and actual captures', () => {
  const miss = x => record('attempt',{outcome:'OutOfReach',energy_paid:.08,
    evidence:{measure:{body:{x}},geometry:{capture_offset_body:{x:13}}}});
  const rows=[miss(10),miss(16),record('attempt',{outcome:'Captured',energy_paid:.08}),
    record('capture',{material:1,energy:2}),record('death',{cause:'Starvation'})];
  const r=reduceEvents(rows,144000,288000);
  assert.equal(r.captures,1);assert.equal(r.near_out_of_reach,1);assert.equal(r.far_out_of_reach,1);
  assert.equal(r.paid_energy,.24);assert.equal(r.deaths.Starvation,1);
  assert.equal(r.captured_material,1);assert.equal(r.captured_energy,2);
});

test('funding, births and maturity stay separate from unrelated prey lineage depth', () => {
  const rows=[record('reproduction',{record:{transaction:'funded'}}),
    record('offspring'),record('reproduction',{record:{transaction:'born'}}),
    {stream:'observed_maturity',tick:144002,depth:1}];
  const r=reduceEvents(rows,144000,288000);
  assert.equal(r.reproduction.funded,1);assert.equal(r.offspring,1);assert.equal(r.observed_adult_descendants,1);
  assert.throws(()=>reduceEvents([record('offspring')],144000,288000),/offspring/);
  assert.throws(()=>reduceEvents([record('capture',{material:1,energy:2})],144000,288000),/capture/);
});

test('missing or nonfinite axial contact coordinates are not counted as far misses', () => {
  for (const bad of [undefined,null,NaN,Infinity,'10']) {
    for (const side of ['body','claw']) {
      const e=record('attempt',{outcome:'OutOfReach',energy_paid:.08,
        evidence:{measure:{body:{x:10}},geometry:{capture_offset_body:{x:13}}}});
      if(side==='body')e.event.evidence.measure.body.x=bad;
      else e.event.evidence.geometry.capture_offset_body.x=bad;
      assert.throws(()=>reduceEvents([e],144000,288000),/axial coordinates/);
    }
  }
});

test('event reduction refuses malformed time, unknown variants and nonfinite quantities', () => {
  for (const rows of [
    [record('death',{cause:'Starvation'},144000)],
    [record('death',{cause:'Starvation'},288001)],
    [record('death',{cause:'Starvation'},144002),record('death',{cause:'Age'},144001)],
    [record('future')],[{stream:'unknown',event:{tick:144001}}],
    [record('attempt',{outcome:'Missed',energy_paid:NaN})],
    [record('attempt',{outcome:'Future',energy_paid:0})],
    [record('reproduction',{record:{transaction:'future'}})],
  ]) assert.throws(()=>reduceEvents(rows,144000,288000));
});

test('snapshot envelope integrity can explicitly check schema11 without loosening preparation', () => {
  const original=readFileSync(new URL('../crates/cubarium-core/tests/fixtures/pre-hunter-v9-173400.cubw',import.meta.url));
  const relabeled=Buffer.from(original);relabeled.writeUInt32LE(11,4);
  // Deliberately relabeled envelope only: NOT a semantic schema11 or migration fixture.
  assert.throws(()=>inspectSnapshot(relabeled),/schema9/);
  const checked=inspectSnapshot(relabeled,11);
  assert.equal(checked.schema,11);
  assert.equal(checked.state_hash,inspectSnapshot(original).state_hash);
  const damaged=Buffer.from(relabeled);damaged[damaged.length-1]^=1;
  assert.throws(()=>inspectSnapshot(damaged,11),/checksum/);
});
