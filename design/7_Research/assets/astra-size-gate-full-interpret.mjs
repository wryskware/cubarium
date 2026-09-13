// Additional read-only full-cohort interpretation; never steps a world or changes a gate.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {join} from 'node:path';
import {ARMS} from '../../../scripts/hunter-size-gate-parity.mjs';
const read=p=>readFileSync(p), json=p=>JSON.parse(read(p)), lines=p=>read(p).toString().trim().split('\n').filter(Boolean).map(JSON.parse);
const sha=p=>createHash('sha256').update(read(p)).digest('hex');
const reduction=json(process.argv[2]);
assert.deepEqual(reduction.problems,[]);assert.equal(reduction.reference_parity.arms_reproduced,72);
assert.deepEqual(reduction.seeds,Array.from({length:12},(_,i)=>i+1));
const id=m=>`${m.id.slot}:${m.id.generation}`;
const payload=b=>b.subarray(22+b.readUInt16LE(8));
const sum=(xs,f)=>xs.reduce((n,x)=>n+f(x),0);
const finite=n=>assert(Number.isFinite(n)&&n>=0);
const near=(a,b)=>assert(Math.abs(a-b)<=1e-9,`${a} != ${b}`);
const output={builds:reduction.builds,reduction_sha256:sha(process.argv[2]),scope:'All12 seeds × six arms × two recipes; auxiliary checks of recorded evidence, not semantic snapshot decode or a new core audit.',
  complete_arms:0,reference_payloads_compared_bytewise:0,flow_members_checked:0,checks:0,raw_energy_failures:[],sides:{},pairs:[],local_statuses:{}};
const originals={};
for(const side of ['reference','candidate']) {
  const root=reduction.runs[side], manifest=json(join(root,'manifest.json'));
  assert.equal(sha(join(root,'hunter_compare.frozen')),reduction.pinned_binary_sha256);
  assert.deepEqual(manifest.ran_seeds,reduction.seeds);assert.deepEqual(manifest.cohort_seeds,reduction.seeds);
  assert.equal(manifest.seed_subset_pilot,false);assert.equal(manifest.ticks,144000);
  output.sides[side]={arms:{},children:[],manifest_sha256:sha(join(root,'manifest.json'))};
  output.local_statuses[side]={};
  for(const seed of reduction.seeds) {
    for(const r of lines(join(root,`seed-${seed}`,'local-recovery.jsonl'))) {
      const k=`${ARMS[r.capture.arm]}:${r.status}`;
      output.local_statuses[side][k]=(output.local_statuses[side][k]||0)+1;
    }
    for(const arm of ARMS) {
      const key=`${seed}/${arm}`,dir=join(root,`seed-${seed}`,arm),s=json(join(dir,'summary.json')),
        o=json(join(dir,'opening.json')),f=json(join(dir,'flow.json')),c=lines(join(dir,'census.jsonl'));
      for(const k of ['technical_complete','complete_experiment_measurement','observer_statistics_trusted_through_closing_tick'])assert.equal(s[k],true,`${key}/${k}`);
      assert.equal(s.audit.passed,true);assert.equal(s.audit.failure,null);
      assert.equal(s.closing_tick,288000);assert.equal(s.planned_ticks,144000);
      assert.equal(s.last_complete_observer_tick,288000);assert.equal(s.last_complete_reproduction_tick,288000);
      assert.equal(c.length,720);c.forEach((r,i)=>assert.equal(r.tick,144200+i*200));
      assert.equal(s.reproduction_audit.ticks_observed,144000);
      const limits=['material','energy','water'].map(k=>1e-8*Math.max(1,o.pre_import_inventory[k]));
      assert.deepEqual(s.audit.fixed_limits,limits);
      for(const [k,i]of [['material',0],['corrected_energy',1],['independent_energy',1],['water',2]]) {
        const g=s.audit[k];finite(g.magnitude);assert(g.magnitude<=limits[i]);assert.equal(g.first_crossing,null);assert.equal(g.nonfinite_at,null);
      }
      for(let i=0;i<3;i++){const g=s.audit.initializer_boundary[i];finite(g.magnitude);assert(g.magnitude<=limits[i]);assert.equal(g.first_crossing,null);assert.equal(g.nonfinite_at,null);}
      if(!s.audit.legacy_passed)output.raw_energy_failures.push(`${side}/${key}`);
      for(const file of ['post-initialization.cubw','closing.cubw']) {
        const bytes=read(join(dir,file));assert.equal(bytes.readUInt32LE(4),12);
        if(side==='reference'){
          const old=read(join(reduction.runs.retained_reference,`seed-${seed}`,arm,file));
          assert(payload(bytes).equals(payload(old)),key+'/'+file);output.reference_payloads_compared_bytewise++;
        }
      }
      assert.equal(sha(join(dir,'closing.cubw')),s.closing_snapshot_sha256);
      assert.equal(sha(join(dir,'post-initialization.cubw')),o.post_snapshot_sha256);
      const members=f.ledger.members,children=members.filter(m=>m.origin==='Descendant'),founders=members.filter(m=>m.origin==='Founder');
      assert.equal(new Set(members.map(id)).size,members.length);assert.equal(f.expected_members,f.recorded_members);assert.equal(members.length,f.expected_members);
      assert.equal(children.length,s.offspring);assert.equal(founders.length,ARMS.indexOf(arm)>=2?1:0);
      assert.equal(s.reproduction_audit.counts.born,s.offspring);
      for(const m of members) {
        assert.equal(m.residual.violations,0);assert.equal(m.residual.first_violation_tick,null);
        for(const k of ['max_structure','max_reserve','max_energy']){finite(m.residual[k]);assert(m.residual[k]<=1e-9);}
        output.checks+=m.residual.checked_ticks;output.flow_members_checked++;
        const g=m.growth;for(const k of ['structure_gained','reserve_spent','energy_cost','heat'])finite(g[k]);
        near(g.structure_gained,g.reserve_spent);near(g.energy_cost,o.config.organism.build_cost*g.structure_gained);
        near(g.heat,(o.config.organism.build_cost+o.config.organism.reserve_energy_density)*g.structure_gained);
        near(m.gate.max_structure,m.open_stocks.structure+g.structure_gained);
      }
      assert.equal(children.filter(m=>m.gate.first_adult_tick!==null).length,s.adult_descendants);
      assert.equal(children.filter(m=>m.funding.funded_count>0).length,s.descendants_that_reproduced);
      const quantities={captures:s.captures,births:s.offspring,closingHunters:s.closing_hunters,adultDescendants:s.adult_descendants,
        adultTicks:s.adult_occupancy_ticks_0_1_2_over2.slice(1).reduce((n,v)=>n+v,0),occupancy:s.adult_occupancy_ticks_0_1_2_over2,
        founderSurvived:founders.filter(m=>m.end_tick===null).length,founderEnd:founders.map(m=>({tick:m.end_tick,cause:m.end_cause})),
        lineageExtinction:s.lineage_extinction_tick,closingPrey:s.closing_population-s.closing_hunters,preyTicks:s.prey_tick_integral,
        closingPreyForms:c.at(-1).prey_by_form,preyCohorts:s.surviving_opening_prey_cohorts,
        wholeRecovery:s.whole_recovery,memberTicks:s.reproductive_opportunity.member_ticks,
        growth:sum(children,m=>m.growth.structure_gained),growthHeat:sum(children,m=>m.growth.heat),growthEnergy:sum(children,m=>m.growth.energy_cost),
        childIntake:sum(children,m=>['digestion','frugivory','grazing','scavenging'].reduce((n,k)=>n+m[k].to_reserve,0)),
        childOxidation:sum(children,m=>m.oxidation.reserve_burned),childUpkeep:sum(children,m=>m.upkeep.paid),childStrike:sum(children,m=>m.strike.paid),
        childHandling:sum(children,m=>m.handling.paid),reproduction:s.reproduction_audit.counts};
      output.sides[side].arms[key]=quantities;
      for(const m of children)output.sides[side].children.push({key:`${key}/${id(m)}`,born:m.born_tick,end:m.end_tick,cause:m.end_cause,
        adult:m.gate.first_adult_tick,adultTicks:m.adult_ticks,maxStructure:m.gate.max_structure,
        maxReserve:m.gate.max_reserve,gate:m.gate,initial:m.open_stocks,closing:m.close_stocks,death:m.death_stocks,growth:m.growth,
        intake:Object.fromEntries(['digestion','frugivory','grazing','scavenging'].map(k=>[k,m[k].to_reserve])),oxidation:m.oxidation,
        upkeep:m.upkeep,strike:m.strike,handling:m.handling,
        growthBins:m.bins.filter(b=>b.growth_ticks>0).map(b=>({start:b.start_tick,ticks:b.ticks,growthTicks:b.growth_ticks,
          gateOpenTicks:b.gate_open_ticks,endStocks:b.end_stocks,reserveIntake:b.reserve_in_digestion+b.reserve_in_field,
          reserveOxidation:b.reserve_out_oxidation,reserveGrowth:b.reserve_out_growth})),
        creditedEnergy:['digestion','frugivory','grazing','scavenging'].reduce((n,k)=>n+m[k].energy_gain,0)});
      if(side==='reference')originals[key]=o;
      else {assert.deepEqual(o.config,originals[key].config);const profiles=[o.profile,originals[key].profile].map(p=>{p=structuredClone(p);if(p)delete p.version;return p;});assert.deepEqual(...profiles);}
      output.complete_arms++;
    }
  }
}
for(const seed of reduction.seeds)for(const arm of ARMS){const key=`${seed}/${arm}`,a=output.sides.reference.arms[key],b=output.sides.candidate.arms[key];
  output.pairs.push({seed,arm,captures:[a.captures,b.captures],births:[a.births,b.births],closingHunters:[a.closingHunters,b.closingHunters],
    adultTicks:[a.adultTicks,b.adultTicks],founderSurvival:[a.founderSurvived,b.founderSurvived],lineageExtinction:[a.lineageExtinction,b.lineageExtinction],
    preyTimePct:100*(b.preyTicks/a.preyTicks-1),closingPreyDelta:b.closingPrey-a.closingPrey,preyCohortDelta:b.preyCohorts-a.preyCohorts,
    referenceOnlyForms:a.closingPreyForms.flatMap((n,i)=>n>0&&b.closingPreyForms[i]===0?[i]:[]),
    candidateOnlyForms:b.closingPreyForms.flatMap((n,i)=>n>0&&a.closingPreyForms[i]===0?[i]:[])});
}
assert.equal(output.complete_arms,144);assert.equal(output.reference_payloads_compared_bytewise,144);
console.log(JSON.stringify(output,null,2));
