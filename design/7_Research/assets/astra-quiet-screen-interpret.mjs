// Read-only interpretation of an already validated quiet run. Not a replacement reducer.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {join} from 'node:path';
const root=process.argv[2];
assert(root,'usage: node astra-quiet-screen-interpret.mjs RUN');
const json=p=>JSON.parse(readFileSync(p,'utf8'));
const lines=p=>readFileSync(p,'utf8').trim().split('\n').filter(Boolean).map(JSON.parse);
const id=o=>`${o.slot}/${o.generation}`;
const diff=(a,b)=>[...a].filter(x=>!b.has(x)).sort();
const reduction=json(join(root,'reduction.json'));
assert(reduction.artifact_checks_passed);
assert.equal(reduction.horizon,'ten-minute');assert.equal(reduction.ticks,12000);
assert.deepEqual(reduction.seeds.map(s=>s.seed),Array.from({length:12},(_,i)=>i+1));
const arms={};
for(const pair of reduction.seeds) {
  arms[pair.seed]={};
  for(const condition of ['nocare','feed']) for(const policy of ['off','candidate']) {
    const path=join(root,`seed-${pair.seed}`,`${policy}_${condition}`);
    const opening=lines(join(path,'opening-organisms.jsonl'));
    const alive=new Map(opening.map(o=>[id(o.id),{cohort:id(o.id),form:o.form,newborn:false}]));
    for(const e of lines(join(path,'life.jsonl'))) {
      if(e.kind==='birth') {
        const parent=alive.get(id(e.parent));assert(parent);assert(!alive.has(id(e.id)));
        assert.equal(parent.cohort,id(e.opening_cohort));
        alive.set(id(e.id),{cohort:parent.cohort,form:e.form,newborn:true});
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
    for(const e of begins){modes[e.underlying]=(modes[e.underlying]||0)+1;parents.add(id(e.parent));}
    const bouts=lines(join(path,'bouts.jsonl')).filter(b=>b.class==='post_birth_recovery');
    for(const b of bouts){assert.equal(b.ticks,b.end_tick-b.start_tick+1);heldTicks+=b.ticks;for(let t=b.start_tick;t<=b.end_tick;t++)heldTimes.add(t);}
    assert.equal(heldTicks,summary.rest.post_birth_recovery.organism_ticks+summary.quiet.held_intervals_ended_by_death);
    arms[pair.seed][`${policy}_${condition}`]={summary,cohorts,forms,modes,parents:parents.size,unionTicks:heldTimes.size,heldTicks,
      newbornSurvivors:[...alive.values()].filter(o=>o.newborn).length,boutEnds:bouts.reduce((out,b)=>(out[b.end]=(out[b.end]||0)+1,out),{})};
  }
}
const output={scope:'All12 seeds, all48 arms. Extra identity-set and temporal-union interpretation of the validated ten-minute artifact; no rerun of biology.',build:reduction.build,conditions:{}};
for(const condition of ['nocare','feed']) {
  const totals={},rows=[];
  for(const policy of ['off','candidate']) {
    const all=reduction.seeds.map(s=>arms[s.seed][`${policy}_${condition}`]);
    const sum=f=>all.reduce((v,a)=>v+f(a),0);
    const modes={},ends={};for(const a of all){for(const [k,n]of Object.entries(a.modes))modes[k]=(modes[k]||0)+n;for(const[k,n]of Object.entries(a.boutEnds))ends[k]=(ends[k]||0)+n;}
    totals[policy]={populationTicks:sum(a=>a.summary.population_organism_ticks),births:sum(a=>a.summary.births),
      deaths:sum(a=>Object.values(a.summary.deaths).reduce((x,y)=>x+y,0)),starvation:sum(a=>a.summary.deaths.starvation),
      closingPopulation:sum(a=>a.summary.closing_population),survivingCohorts:sum(a=>a.cohorts.size),
      newbornSurvivors:sum(a=>a.newbornSurvivors),intakeTicks:sum(a=>a.summary.intake_ticks),path:sum(a=>a.summary.transported_path_px),
      recoveryTicks:sum(a=>a.heldTicks),recoveryBouts:sum(a=>a.summary.quiet.admissions),
      underlyingAtAdmission:modes,boutEnds:ends,uniquePausedParents:sum(a=>a.parents),
      worldTimeWithAnyRecoverySeconds:sum(a=>a.unionTicks)*.05,worldTimeWithAnyRecoveryFraction:sum(a=>a.unionTicks)/(12*12000),
      recoveryLivingTimeFraction:sum(a=>a.summary.rest.post_birth_recovery.organism_ticks)/sum(a=>a.summary.population_organism_ticks)};
  }
  for(const s of reduction.seeds){const a=arms[s.seed][`off_${condition}`],b=arms[s.seed][`candidate_${condition}`];
    rows.push({seed:s.seed,recovery:b.summary.quiet.admissions,pausedParents:b.parents,worldRecoverySeconds:b.unionTicks*.05,
      populationTimePct:100*(b.summary.population_organism_ticks/a.summary.population_organism_ticks-1),birthDelta:b.summary.births-a.summary.births,
      deathDelta:Object.values(b.summary.deaths).reduce((x,y)=>x+y,0)-Object.values(a.summary.deaths).reduce((x,y)=>x+y,0),
      closingDelta:b.summary.closing_population-a.summary.closing_population,cohortCountDelta:b.cohorts.size-a.cohorts.size,
      referenceOnlyCohorts:diff(a.cohorts,b.cohorts),candidateOnlyCohorts:diff(b.cohorts,a.cohorts),
      referenceOnlyForms:diff(a.forms,b.forms),candidateOnlyForms:diff(b.forms,a.forms),newbornSurvivorDelta:b.newbornSurvivors-a.newbornSurvivors});
  }
  output.conditions[condition]={totals,rows};
}
console.log(JSON.stringify(output,null,2));
