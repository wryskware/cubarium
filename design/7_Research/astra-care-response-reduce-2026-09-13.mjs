// Read-only reduction of the fixed 108-comparison screen. JSON goes to stdout.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {join} from 'node:path';

const root = process.argv[2] ?? 'captures/care-response-single-pulse-4d10351';
const read = async p => JSON.parse(await readFile(p, 'utf8'));
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const manifestBytes = await readFile(join(root, 'manifest.json'));
const m = JSON.parse(manifestBytes), summary = await read(join(root, 'summary.json'));
assert.equal(m.kind, 'isolated-care-response-screen-v1');
assert.equal(summary.technical_complete, true);
assert.deepEqual(m.recipe, {ticks:3000, care_start:600, care_every:72000,
  dose_permille:1000, local_every:20, audit_window:200, targets:[0,1,2], actions:['feed','rain','clean']});
assert.equal(hash(await readFile(m.binary)), m.binary_sha256);
for (const opening of m.cohort.openings)
  assert.equal(hash(await readFile(opening.snapshot)), opening.sha256);
assert.equal(summary.outcomes.length, 108);
const keys = ['population','fed_this_tick','feeding_mode','resting','seeking','gestating'];
const fields = ['producer','fruit','litter','litter_energy','water'];
const delta = (a,b,ks=keys) => Object.fromEntries(ks.map(k=>[k,a[k]-b[k]]));
const fraction = c => c.population === 0 ? null : c.fed_this_tick/c.population;
const countCheck = c => {
  for (const k of keys) assert(Number.isSafeInteger(c[k]) && c[k]>=0);
  assert.equal(c.resting+c.seeking+c.feeding_mode,c.population);
  assert(c.fed_this_tick<=c.population && c.gestating<=c.population);
  assert.equal(c.by_form.reduce((a,b)=>a+b,0),c.population);
};
const references = new Map(), seen = new Set(), rows = [];
for (const entry of summary.outcomes) {
  assert.equal(entry.process_exit,0); assert.equal(entry.validation_error,null);
  const {seed,target,kind} = entry, key = `${seed}/${target}/${kind}`;
  assert(seed>=1 && seed<=12 && [0,1,2].includes(target) && m.recipe.actions.includes(kind));
  assert(!seen.has(key)); seen.add(key);
  assert.equal(entry.file,`seed-${seed}-target-${target}-${kind}.json`);
  const bytes = await readFile(join(root,entry.file)), d = JSON.parse(bytes);
  assert.equal(d.seed,seed); assert.equal(d.first_target_index,target); assert.equal(d.care_kind,kind);
  assert.equal(d.start_tick,144000); assert.equal(d.ticks,3000); assert.equal(d.care_start_tick,600);
  assert.equal(d.care_every_ticks,72000); assert.equal(d.dose_permille,1000);
  if (references.has(seed)) assert.deepEqual(d.baseline,references.get(seed));
  else references.set(seed,d.baseline);
  assert.equal(d.baseline.receipts.length,0); assert.equal(d.cared.receipts.length,1);
  assert.equal(d.cared.receipts[0].elapsed,600); assert.equal(d.cared.receipts[0].kind,kind);
  assert.equal(d.cared.receipts[0].receipt.tick,144600);
  const ledgerKeys = ['feed_material_in','feed_energy_in','rain_depth_in','clean_material_out','clean_energy_out'];
  const ledgerDelta = delta(d.cared.care_ledgers,d.baseline.care_ledgers,ledgerKeys);
  const receiptOutcome = d.cared.receipts[0].receipt.outcome;
  const applied = receiptOutcome.Applied ?? receiptOutcome.Partial;
  if (applied) {
    for (const [ledger,amount] of [['feed_material_in','material_in'],['feed_energy_in','energy_in'],
      ['rain_depth_in','water_depth'],['clean_material_out','material_out'],['clean_energy_out','energy_out']])
      assert(Math.abs(ledgerDelta[ledger]-applied[amount])<1e-10,'receipt versus delivered ledger');
  } else {
    for (const v of Object.values(ledgerDelta)) assert.equal(v,0);
  }
  assert.deepEqual(d.baseline.local_activity.first_pulse_cohorts,d.cared.local_activity.first_pulse_cohorts);
  assert.deepEqual(d.baseline.local_activity.samples.slice(0,31),d.cared.local_activity.samples.slice(0,31));
  for (const arm of [d.baseline,d.cared]) {
    assert.equal(arm.audit_passed,true); assert.equal(arm.final.tick,147000);
    assert.equal(arm.local_activity.samples.length,151);
    arm.local_activity.samples.forEach((s,i)=> {
      assert.equal(s.elapsed,i*20); assert.equal(s.tick,144000+i*20);
      assert.equal(s.regions.length,3);
      for (const region of s.regions) {
        countCheck(region.instant); countCheck(region.cumulative_member_ticks);
        if (region.first_pulse_cohort) {
          countCheck(region.first_pulse_cohort.living_anywhere);
          countCheck(region.first_pulse_cohort.cumulative_member_ticks_anywhere);
        }
      }
    });
  }
  const point = (arm, seconds) => arm.local_activity.samples[(600+seconds*20)/20].regions[target];
  const observation = (arm,seconds) => {
    const s = point(arm,seconds), pre = point(arm,0);
    const local = delta(s.cumulative_member_ticks,pre.cumulative_member_ticks);
    const cohort = s.first_pulse_cohort.cumulative_member_ticks_anywhere;
    return {local, fixed:Object.fromEntries(keys.map(k=>[k,cohort[k]])),
      local_fed_fraction:fraction(local), fixed_fed_fraction:fraction(cohort),
      fixed_living:s.first_pulse_cohort.living_anywhere.population,
      fields:Object.fromEntries(fields.map(k=>[k,s[k]]))};
  };
  const baseline = observation(d.baseline,120), cared = observation(d.cared,120);
  const opening = d.baseline.local_activity.first_pulse_cohorts[target];
  assert.equal(opening.tick,144600);
  const openingCount = opening.members.length;
  assert.equal(point(d.cared,120).first_pulse_cohort.opening_count,openingCount);
  for (const arm of [baseline,cared]) assert(arm.fixed.population<=openingCount*2400);
  rows.push({seed,target,kind,file:entry.file,sha256:hash(bytes),opening_count:openingCount,
    opening_modes:opening.members.map(o=>o.mode),
    outcome:receiptOutcome,care_ledger_deltas:ledgerDelta,
    baseline,cared,delta_local:delta(cared.local,baseline.local),
    delta_fixed:delta(cared.fixed,baseline.fixed),delta_fields:delta(cared.fields,baseline.fields,fields),
    early:[1,3,6,15,30].map(seconds=> {
      const a=observation(d.baseline,seconds),b=observation(d.cared,seconds);
      return {seconds,local_fed_delta:b.local.fed_this_tick-a.local.fed_this_tick,
        fixed_fed_delta:b.fixed.fed_this_tick-a.fixed.fed_this_tick};
    })});
}
assert.equal(seen.size,108); assert.equal(references.size,12);
const total = (rs,pick) => rs.reduce((a,r)=>a+pick(r),0);
const signs = values => ({positive:values.filter(x=>x>0).length,zero:values.filter(x=>x===0).length,
  negative:values.filter(x=>x<0).length});
const aggregate = rs => ({n:rs.length,empty_openings:rs.filter(r=>r.opening_count===0).length,
  opening_count_sum:total(rs,r=>r.opening_count),
  outcomes:rs.reduce((a,r)=>{const k=Object.keys(r.outcome)[0];a[k]=(a[k]??0)+1;return a;},{}),
  local:Object.fromEntries(keys.map(k=>[k,{baseline:total(rs,r=>r.baseline.local[k]),
    cared:total(rs,r=>r.cared.local[k]),delta:total(rs,r=>r.delta_local[k]),
    signs:signs(rs.map(r=>r.delta_local[k]))}])),
  fixed:Object.fromEntries(keys.map(k=>[k,{baseline:total(rs,r=>r.baseline.fixed[k]),
    cared:total(rs,r=>r.cared.fixed[k]),delta:total(rs,r=>r.delta_fixed[k]),
    signs:signs(rs.map(r=>r.delta_fixed[k]))}])),
  fed_fraction_difference_signs:Object.fromEntries(['local','fixed'].map(scope=>[scope,
    signs(rs.filter(r=>r.baseline[scope+'_fed_fraction']!==null && r.cared[scope+'_fed_fraction']!==null)
      .map(r=>r.cared[scope+'_fed_fraction']-r.baseline[scope+'_fed_fraction']))])),
  zero_local_exposure:{baseline:rs.filter(r=>r.baseline.local.population===0).length,
    cared:rs.filter(r=>r.cared.local.population===0).length},
  closing_fixed_losses:{baseline:total(rs,r=>r.opening_count-r.baseline.fixed_living),
    cared:total(rs,r=>r.opening_count-r.cared.fixed_living)},
  early:[1,3,6,15,30].map(seconds=>({seconds,...Object.fromEntries(['local','fixed'].map(scope=> {
    const xs=rs.map(r=>r.early.find(p=>p.seconds===seconds)[scope+'_fed_delta']);
    return [scope,{delta:xs.reduce((a,b)=>a+b,0),signs:signs(xs)}];
  }))}))});
const grouped = select => Object.fromEntries([...new Set(rows.map(select))].map(key=>
  [key,aggregate(rows.filter(r=>select(r)===key))]));
process.stdout.write(JSON.stringify({kind:'astra-care-response-reduction-v1',
  source:root,manifest_sha256:hash(manifestBytes),validated_comparisons:rows.length,
  basis:'Counts are organism-ticks, 20 per organism-second. Local counts subtract elapsed600. Fixed IDs follow anywhere from the pre-pulse boundary. Fractions divide by living selected organism-ticks; zero denominators are null. No manual-crumb, per-diet, path, rest-bout or flood-time inference. Repeated controls are paired references, not independent replicates.',
  by_kind:grouped(r=>r.kind),by_kind_target:grouped(r=>`${r.kind}/${r.target}`),
  by_kind_seed:grouped(r=>`${r.kind}/${r.seed}`),rows},null,2));
