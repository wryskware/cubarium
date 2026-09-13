// Read-only interpretation of the fixed twelve original histories, not a new replay.
// Run from the repository root; stdout is the evidence artifact. Missing/failed seeds refuse.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve, join} from 'node:path';
import {parseArtifactText, reduceArtifact, HORIZON_TICKS, CENSUS_EVERY, FORM_SLOTS}
  from '../../../scripts/fauna-development-flow.mjs';
import {parseTelemetry} from '../../../scripts/reduce-fauna-early-loss.mjs';
import {inspectSnapshot} from '../../../scripts/prepare-hunter-worlds.mjs';

const sha = b => createHash('sha256').update(b).digest('hex');
const read = path => readFile(path);
const json = async path => JSON.parse(await read(path));
const sum = values => values.reduce((a,b)=>a+b,0);
const ratio = (n,d) => d ? n/d : null;
const cohort = resolve('captures/hunter-openings-2026-09-13');
const remaining = resolve('captures/fauna-development-flow-remaining-cdeeb55-2026-09-13');
const pilot = resolve('captures/fauna-development-flow-pilots-2026-09-13');
const manifest = await json(join(remaining,'manifest.json'));
const completion = await json(join(remaining,'completion.json'));
const laterSeeds = [3,4,5,6,7,8,9,10,11,12];
assert.deepEqual(manifest.seeds,laterSeeds);
assert.deepEqual(completion.results.map(r=>r.seed),laterSeeds);
assert.equal(completion.all_succeeded,true);
assert(completion.results.every(r=>r.exit_code===0 && r.signal===null && r.error===null));
assert.equal(sha(await read(join(remaining,'fauna_development_flow.frozen'))),manifest.binary_sha256);
const rows = [];
for (let seed=1;seed<=12;seed++) {
  const path=join(seed<3?pilot:remaining,`seed-${seed}.json`);
  const bytes=await read(path), a=parseArtifactText(bytes.toString());
  const validation=reduceArtifact(a,{sha256:sha(bytes),name:path});
  assert.equal(a.seed,seed);
  const original=manifest.retained_pilots.find(p=>p.seed===seed);
  assert.equal(a.diagnostic_binary_sha256,original?.diagnostic_binary_sha256??manifest.binary_sha256);
  if(original) assert.equal(sha(bytes),original.sha256);
  for (const [where,file] of [
    ['input',join(cohort,`initial-seeds/seed-${seed}/world-0.cubw`)],
    ['closing',join(cohort,`seed-${seed}/world-144000.cubw`)]]) {
    const actual=inspectSnapshot(await read(file),9);
    for(const field of ['sha256','state_hash','payload_bytes','crc32'])
      assert.equal(a[where][field],actual[field],`seed${seed} ${where} ${field}`);
  }
  const telemetryBytes=await read(join(cohort,`seed-${seed}/telemetry.jsonl`));
  const telemetry=parseTelemetry(telemetryBytes.toString());
  assert.equal(telemetry.length,HORIZON_TICKS/CENSUS_EVERY);
  // Rebuild from identities and life boundaries, independently of the carried series.
  for(let i=0;i<telemetry.length;i++) {
    const tick=(i+1)*CENSUS_EVERY, counts=Array(FORM_SLOTS).fill(0);
    assert.equal(telemetry[i].tick,tick);
    for(const m of a.members)
      if(m.born_tick<=tick && (!m.death || m.death.event_tick>tick)) counts[m.form]++;
    assert.deepEqual(counts,telemetry[i].population_by_form,`seed${seed} tick${tick}`);
    assert.equal(sum(counts),telemetry[i].population);
    assert.deepEqual(a.gates.census_cross_check.reconstructed_series[i],[tick,counts,sum(counts)]);
  }
  for(const m of a.members) {
    const ticks=(m.death?.event_tick??HORIZON_TICKS)-m.born_tick;
    for(const n of [m.reconciliation.checks,m.growth_gate.observations,m.upkeep.ticks])
      assert.equal(n,ticks,`seed${seed} ${m.id.slot}:${m.id.generation} lifetime`);
  }
  assert.equal(sum(a.members.map(m=>m.reconciliation.checks)),a.world.totals.reconciliation_checks);
  const forms=Array.from({length:FORM_SLOTS},(_,form)=>{
    const members=a.members.filter(m=>m.form===form), children=members.filter(m=>m.origin==='descendant');
    const survivors=members.filter(m=>!m.death), deaths=members.filter(m=>m.death);
    const total=fn=>sum(members.map(fn));
    const ticks=total(m=>m.upkeep.ticks);
    const juvenile=total(m=>m.growth_gate.structure_side_open);
    const admitted=total(m=>m.growth_gate.both_open);
    const raw=total(m=>sum(Object.values(m.intake).filter(v=>typeof v==='object').map(v=>v.actual_amount)));
    const assimilated=total(m=>sum(Object.values(m.intake).filter(v=>typeof v==='object').map(v=>v.to_reserve)));
    return {form,name:a.forms[form].name,exposed:members.length>0,
      members:members.length,founders:members.length-children.length,children:children.length,
      children_reaching_adult:children.filter(m=>m.structure.adult_recruitment_tick!==null).length,
      children_starved_juvenile:children.filter(m=>m.death?.cause==='starvation' && m.structure.adult_recruitment_tick===null).length,
      children_censored_juvenile:children.filter(m=>!m.death && m.structure.adult_recruitment_tick===null).length,
      closing:survivors.length,deaths_by_cause:a.forms[form].deaths_by_cause,
      last_birth:children.length?Math.max(...children.map(m=>m.born_tick)):null,
      extinction_tick:members.length && !survivors.length?Math.max(...deaths.map(m=>m.death.event_tick)):null,
      closing_founder_roots:new Set(survivors.map(m=>`${m.root.slot}:${m.root.generation}`)).size,
      member_ticks:ticks,pre_growth_juvenile_observations:juvenile,
      juvenile_reserve_prerequisite_closed:juvenile-admitted,
      juvenile_reserve_prerequisite_closed_fraction:ratio(juvenile-admitted,juvenile),
      growth_steps:total(m=>m.growth_gate.entered),
      growth_caps:Object.fromEntries(['rate','headroom','reserve','energy'].map(k=>[k,total(m=>m.growth_gate[`bound_by_${k}`])])),
      structure_built:total(m=>m.growth_gate.structure_gained),
      growth_reserve_spent:total(m=>m.growth_gate.reserve_spent),
      raw_intake:raw,assimilated_reserve:assimilated,
      raw_intake_per_1000_member_ticks:ratio(raw*1000,ticks),
      assimilated_reserve_per_1000_member_ticks:ratio(assimilated*1000,ticks),
      oxidation_reserve_spent:total(m=>m.oxidation.reserve_burned),
      reproduction_reserve_spent:total(m=>m.reproduction.reserve_debit),
      bud_decisions:total(m=>m.reproduction.bud_decisions),funded:total(m=>m.reproduction.funded),
      upkeep_demand:total(m=>m.upkeep.demand_total),upkeep_paid:total(m=>m.upkeep.paid_total),
      sensing_demand:total(m=>m.upkeep.demand_sensing)};
  });
  rows.push({seed,path,artifact_sha256:sha(bytes),binary_sha256:a.diagnostic_binary_sha256,
    telemetry_sha256:sha(telemetryBytes),validation,forms});
}
console.log(JSON.stringify({kind:'root-fauna-flow-full-cohort-interpretation',seeds:rows.map(r=>r.seed),
  horizon_ticks:HORIZON_TICKS,census_samples_independently_rebuilt:rows.length*HORIZON_TICKS/CENSUS_EVERY,
  members:sum(rows.map(r=>r.validation.members)),reconciliation_checks:sum(rows.map(r=>r.validation.reconciliation_checks)),
  limits:'Original pre-hunter no-care histories, not a policy comparison. Snapshot envelope/payload identities and every census independently checked; observer neutrality remains the runtime replay evidence. All forms and losses retained. Founder-root counts are ancestry counts, not genetic diversity. Juvenile prerequisite rates are realized permission, not a lowering-gate counterfactual; funding-site success is not upstream reproductive eligibility; stocks and lifetime sums do not identify joint causal bottlenecks. Censored animals are not proven long-run survivors.',rows},null,2));
