// Read-only interpretation of an already validated quiet run. Not a replacement reducer.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {join} from 'node:path';
const root=process.argv[2];
assert(root,'usage: node astra-quiet-two-hour-interpret.mjs RUN VERIFIED_REDUCTION [TEN_MINUTE_RUN]');
const json=p=>JSON.parse(readFileSync(p,'utf8'));
const lines=p=>readFileSync(p,'utf8').trim().split('\n').filter(Boolean).map(JSON.parse);
const id=o=>`${o.slot}/${o.generation}`;
const diff=(a,b)=>[...a].filter(x=>!b.has(x)).sort();
const reduction=json(process.argv[3]);
assert(process.argv[3], 'supply independently verified reduction JSON');
assert(reduction.artifact_checks_passed);
assert.equal(reduction.horizon,'two-hour');assert.equal(reduction.ticks,144000);
assert.deepEqual(reduction.seeds.map(s=>s.seed),Array.from({length:12},(_,i)=>i+1));
const arms={};
for(const pair of reduction.seeds) {
  arms[pair.seed]={};
  for(const condition of ['nocare','feed']) for(const policy of ['off','candidate']) {
    const path=join(root,`seed-${pair.seed}`,`${policy}_${condition}`);
    const opening=lines(join(path,'opening-organisms.jsonl'));
    const alive=new Map(opening.map(o=>[id(o.id),{cohort:id(o.id),form:o.form,newborn:false,bornTick:o.born_tick}]));
    const births = new Map(), reproduced = new Set();
    for(const e of lines(join(path,'life.jsonl'))) {
      if(e.kind==='birth') {
        const parent=alive.get(id(e.parent));assert(parent);assert(!alive.has(id(e.id)));
        assert.equal(parent.cohort,id(e.opening_cohort));
        alive.set(id(e.id),{cohort:parent.cohort,form:e.form,newborn:true,bornTick:e.tick});
        births.set(id(e.id), e.tick); if(births.has(id(e.parent))) reproduced.add(id(e.parent));
      } else {assert.equal(e.kind,'death');assert(alive.delete(id(e.id)));}
    }
    const cohorts=new Set([...alive.values()].map(o=>o.cohort));
    const forms=new Set([...alive.values()].map(o=>o.form));
    const summary=pair[condition][policy];
    assert.equal(alive.size,summary.closing_population);
    assert.equal(cohorts.size,summary.surviving_opening_cohorts);
    const events=lines(join(path,'quiet-events.jsonl'));
    const begins=events.filter(e=>e.kind==='begin');
    const modes={},parents=new Set(),heldTimes=new Set();let heldTicks=0;
    const admittedChildren = new Set(begins.map(e=>id(e.child)));
    for(const e of begins){modes[e.underlying]=(modes[e.underlying]||0)+1;parents.add(id(e.parent));}
    const bouts=lines(join(path,'bouts.jsonl')).filter(b=>b.class==='post_birth_recovery');
    for(const b of bouts){assert.equal(b.ticks,b.end_tick-b.start_tick+1);heldTicks+=b.ticks;for(let t=b.start_tick;t<=b.end_tick;t++)heldTimes.add(t);}
    assert.equal(heldTicks,summary.rest.post_birth_recovery.organism_ticks+summary.quiet.held_intervals_ended_by_death);
    arms[pair.seed][`${policy}_${condition}`]={summary,cohorts,forms,modes,parents:parents.size,unionTicks:heldTimes.size,heldTicks,
      newbornSurvivors:[...alive.values()].filter(o=>o.newborn).length,
      reproducedBornInRun:reproduced.size, admittedChildrenReproduced:[...admittedChildren].filter(k=>reproduced.has(k)).length,
      admittedChildrenSurviving:[...admittedChildren].filter(k=>alive.has(k)).length,
      closingBornAgeTicks:[...alive.values()].filter(o=>o.newborn).map(o=>288000-o.bornTick),
      boutLengths:bouts.map(b=>b.ticks), residualRecoveryPath:summary.rest.post_birth_recovery.transported_path_px,
      boutEnds:bouts.reduce((out,b)=>(out[b.end]=(out[b.end]||0)+1,out),{})};
  }
}
let prefixStreamsChecked=0;
if(process.argv[4]) for(let seed=1;seed<=12;seed++) for(const arm of ['off_nocare','candidate_nocare','off_feed','candidate_feed']) {
  for(const name of ['life.jsonl','quiet-events.jsonl']) {
    // End is a decision-boundary record: End(156000) is published while stepping
    // to 156001, outside the short run. Birth/Begin at156000 are already observed.
    const long=lines(join(root,`seed-${seed}`,arm,name)).filter(e=>e.tick<=156000 &&
      !(name==='quiet-events.jsonl' && e.kind==='end' && e.tick===156000));
    assert.deepEqual(long,lines(join(process.argv[4],`seed-${seed}`,arm,name)));prefixStreamsChecked++;
  }
}
const output={scope:'All12 seeds, all48 arms. Extra identity-set and temporal-union interpretation of the validated two-hour artifact; no rerun of biology.',build:reduction.build,prefixStreamsChecked,conditions:{}};
for(const condition of ['nocare','feed']) {
  const totals={},rows=[];
  for(const policy of ['off','candidate']) {
    const all=reduction.seeds.map(s=>arms[s.seed][`${policy}_${condition}`]);
    const sum=f=>all.reduce((v,a)=>v+f(a),0);
    const modes={},ends={};for(const a of all){for(const [k,n]of Object.entries(a.modes))modes[k]=(modes[k]||0)+n;for(const[k,n]of Object.entries(a.boutEnds))ends[k]=(ends[k]||0)+n;}
    totals[policy]={populationTicks:sum(a=>a.summary.population_organism_ticks),births:sum(a=>a.summary.births),
      deaths:sum(a=>Object.values(a.summary.deaths).reduce((x,y)=>x+y,0)),starvation:sum(a=>a.summary.deaths.starvation),
      closingPopulation:sum(a=>a.summary.closing_population),survivingCohorts:sum(a=>a.cohorts.size),
      newbornSurvivors:sum(a=>a.newbornSurvivors),reproducedBornInRun:sum(a=>a.reproducedBornInRun),
      admittedChildrenReproduced:sum(a=>a.admittedChildrenReproduced),admittedChildrenSurviving:sum(a=>a.admittedChildrenSurviving),
      residualRecoveryPath:sum(a=>a.residualRecoveryPath),releases:sum(a=>a.summary.quiet.releases),
      refusals:sum(a=>Object.values(a.summary.quiet.refusals).reduce((n,v)=>n+v,0)),
      minimumClosingBornAgeTicks:Math.min(...all.flatMap(a=>a.closingBornAgeTicks)),
      maximumClosingBornAgeTicks:Math.max(...all.flatMap(a=>a.closingBornAgeTicks)),
      intakeTicks:sum(a=>a.summary.intake_ticks),path:sum(a=>a.summary.transported_path_px),
      recoveryTicks:sum(a=>a.heldTicks),recoveryBouts:sum(a=>a.summary.quiet.admissions),
      underlyingAtAdmission:modes,boutEnds:ends,uniquePausedParents:sum(a=>a.parents),
      worldTimeWithAnyRecoverySeconds:sum(a=>a.unionTicks)*.05,worldTimeWithAnyRecoveryFraction:sum(a=>a.unionTicks)/(12*reduction.ticks),
      recoveryLivingTimeFraction:sum(a=>a.summary.rest.post_birth_recovery.organism_ticks)/sum(a=>a.summary.population_organism_ticks)};
  }
  for(const s of reduction.seeds){const a=arms[s.seed][`off_${condition}`],b=arms[s.seed][`candidate_${condition}`];
    rows.push({seed:s.seed,recovery:b.summary.quiet.admissions,pausedParents:b.parents,worldRecoverySeconds:b.unionTicks*.05,
      recoveryLivingTimePct:100*b.heldTicks/b.summary.population_organism_ticks,underlying:b.modes,
      residualRecoveryPath:b.residualRecoveryPath,boutLengthRange:b.boutLengths.length?[Math.min(...b.boutLengths),Math.max(...b.boutLengths)]:[],
      reproducedBornInRun:[a.reproducedBornInRun,b.reproducedBornInRun],
      populationTimePct:100*(b.summary.population_organism_ticks/a.summary.population_organism_ticks-1),birthDelta:b.summary.births-a.summary.births,
      deathDelta:Object.values(b.summary.deaths).reduce((x,y)=>x+y,0)-Object.values(a.summary.deaths).reduce((x,y)=>x+y,0),
      closingDelta:b.summary.closing_population-a.summary.closing_population,cohortCountDelta:b.cohorts.size-a.cohorts.size,
      referenceOnlyCohorts:diff(a.cohorts,b.cohorts),candidateOnlyCohorts:diff(b.cohorts,a.cohorts),
      referenceOnlyForms:diff(a.forms,b.forms),candidateOnlyForms:diff(b.forms,a.forms),newbornSurvivorDelta:b.newbornSurvivors-a.newbornSurvivors});
  }
  for(const row of rows){const pair = reduction.seeds.find(s=>s.seed===row.seed)[condition];
    assert.deepEqual(row.referenceOnlyCohorts,pair.matched_survivors.opening_cohorts.reference_only);
    assert.deepEqual(row.candidateOnlyCohorts,pair.matched_survivors.opening_cohorts.candidate_only);
    assert.deepEqual(row.referenceOnlyForms,pair.matched_survivors.forms.reference_only);
    assert.deepEqual(row.candidateOnlyForms,pair.matched_survivors.forms.candidate_only);
  }
  output.conditions[condition]={totals,rows};
}
console.log(JSON.stringify(output,null,2));
