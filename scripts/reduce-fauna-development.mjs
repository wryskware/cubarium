// Read-only developmental follow-up of the immutable quiet-diagnosis cohort.
// No simulation, parameter selection, or inferred settlement transactions.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve, join} from 'node:path';
import {pathToFileURL} from 'node:url';

const idKey = id => {
  assert(Number.isSafeInteger(id?.slot) && id.slot >= 0);
  assert(Number.isSafeInteger(id.generation) && id.generation >= 0);
  return `${id.slot}/${id.generation}`;
};
const count = x => { assert(Number.isSafeInteger(x) && x >= 0); return x; };
const total = (rows, key) => rows.reduce((sum, x) => sum + x.counts[key], 0);

export function reduceArm(result, events) {
  const o = result.observation;
  assert.equal(result.technical_complete, true); assert.equal(result.audit_passed, true);
  assert.equal(o.opening_tick, 144000); assert.equal(o.closing_tick, 156000);
  const ids = new Map(), deaths = new Map(), births = new Map();
  for (const x of o.individuals) {
    const key = idKey(x.id); assert(!ids.has(key), 'duplicate full ID'); ids.set(key, x);
    assert(Number.isInteger(x.form) && x.form >= 0 && x.form < 8);
    for (const k of ['ticks','immature','fed','reserve_zero','observed_escrow_starts']) count(x.counts[k]);
    assert(x.counts.immature <= x.counts.ticks && x.counts.fed <= x.counts.ticks);
    assert(x.counts.reserve_zero <= x.counts.ticks);
    for (const point of [x.initial,x.last]) {
      assert(Number.isFinite(point.adult_fraction) && point.adult_fraction >= 0);
      count(point.tick);
      assert(point.tick >= o.opening_tick && point.tick <= o.closing_tick);
    }
    assert.equal(typeof x.opening_member, 'boolean');
    assert(x.initial.tick <= x.last.tick, 'reversed observation interval');
    if (x.opening_member) assert.equal(x.initial.tick, o.opening_tick);
  }
  for (const row of events.filter(x => x.stream === 'life')) {
    const e = row.event, key = idKey(e.id);
    assert(e.tick > o.opening_tick && e.tick <= o.closing_tick);
    assert(ids.has(key), 'life event has no retained individual');
    const map = e.kind === 'birth' ? births : e.kind === 'death' ? deaths : null;
    assert(map, 'unknown life event'); assert(!map.has(key), 'duplicate life event'); map.set(key,e);
    if (e.kind === 'birth') assert.equal(ids.get(key).opening_member, false);
    if (e.kind === 'death') assert(['Starvation','Age','Collapse','Predation'].includes(e.cause));
  }
  for (const [key,x] of ids) {
    assert.equal(births.has(key), !x.opening_member, 'birth/retained ID mismatch');
    if (births.has(key)) assert.equal(births.get(key).tick, x.initial.tick, 'birth/initial time mismatch');
    if (deaths.has(key)) assert(x.last.tick < deaths.get(key).tick, 'observation at/after death');
    if (!deaths.has(key)) assert.equal(x.last.tick, o.closing_tick, 'uncensored missing survivor');
  }
  assert.equal(ids.size - deaths.size, result.population, 'closing census mismatch');
  return Array.from({length:8}, (_,form) => {
    const rows = [...ids.values()].filter(x => x.form === form);
    const opening = rows.filter(x => x.opening_member);
    const juveniles = opening.filter(x => x.initial.adult_fraction < 1);
    const born = rows.filter(x => !x.opening_member);
    const lost = rows.filter(x => deaths.has(idKey(x.id)));
    const alive = rows.filter(x => !deaths.has(idKey(x.id)));
    return {form, opening:opening.length, opening_adults:opening.length-juveniles.length,
      opening_juveniles:juveniles.length, born:born.length, closing:alive.length,
      closing_adults:alive.filter(x=>x.last.adult_fraction>=1).length,
      deaths:lost.length,
      starvation:lost.filter(x=>deaths.get(idKey(x.id)).cause==='Starvation').length,
      deaths_last_observed_immature:lost.filter(x=>x.last.adult_fraction<1).length,
      opening_juveniles_last_observed_adult:juveniles.filter(x=>x.last.adult_fraction>=1).length,
      opening_juveniles_died:juveniles.filter(x=>deaths.has(idKey(x.id))).length,
      newborns_last_observed_adult:born.filter(x=>x.last.adult_fraction>=1).length,
      member_ticks:total(rows,'ticks'), immature_ticks:total(rows,'immature'),
      fed_ticks:total(rows,'fed'), reserve_zero_ticks:total(rows,'reserve_zero'),
      observed_escrow_starts:total(rows,'observed_escrow_starts')};
  });
}

export function aggregate(rows) {
  assert(rows.length > 0, 'cannot aggregate an absent cohort');
  return Array.from({length:8}, (_,form) => {
    const xs = rows.flatMap(x=>x.forms).filter(x=>x.form===form);
    const r = {form};
    for (const k of Object.keys(xs[0]).filter(k=>k!=='form')) r[k]=xs.reduce((sum,x)=>sum+x[k],0);
    r.fed_fraction=r.member_ticks?r.fed_ticks/r.member_ticks:null;
    r.immature_fraction=r.member_ticks?r.immature_ticks/r.member_ticks:null;
    r.zero_reserve_fraction=r.member_ticks?r.reserve_zero_ticks/r.member_ticks:null;
    return r;
  });
}

export async function reduce(directory) {
  const root=resolve(directory), read=async path=>JSON.parse(await readFile(path,'utf8'));
  const hash=b=>createHash('sha256').update(b).digest('hex');
  const m=await read(join(root,'manifest.json')), s=await read(join(root,'summary.json'));
  assert.equal(m.kind,'astra-ordinary-quiet-diagnosis-v1'); assert.equal(m.ticks,12000);
  assert.equal(s.technical_complete,true); assert.equal(s.results.length,24);
  assert.equal(hash(await readFile(join(root,'astra_quiet_diagnosis.frozen'))),m.binary_sha256);
  assert.deepEqual(m.cohort.openings.map(x=>x.seed).sort((a,b)=>a-b),Array.from({length:12},(_,i)=>i+1));
  const rows=[], seen=new Set();
  for (const entry of s.results) {
    assert(Number.isInteger(entry.seed) && entry.seed>=1 && entry.seed<=12);
    assert(['baseline','feed'].includes(entry.arm)); assert.equal(entry.technical_complete,true);
    assert.equal(entry.error,null);
    const key=`${entry.seed}/${entry.arm}`; assert(!seen.has(key)); seen.add(key);
    const dir=join(root,`seed-${entry.seed}-${entry.arm}`);
    const resultBytes=await readFile(join(dir,'result.json'));
    const eventBytes=await readFile(join(dir,'events.jsonl'));
    const result=JSON.parse(resultBytes);
    assert.equal(result.closing_hash,entry.closing_hash); assert.equal(result.build,m.build);
    const events=eventBytes.toString().trim().split('\n').filter(Boolean).map(JSON.parse);
    rows.push({seed:entry.seed,arm:entry.arm,result_sha256:hash(resultBytes),
      events_sha256:hash(eventBytes),forms:reduceArm(result,events)});
  }
  return {kind:'fauna-development-reduction-v1', source:root, build:m.build,
    binary_sha256:m.binary_sha256, compared_arms:rows.length,
    limits:'Fixed 10-minute mature-world follow-up, not development from original founders or long-run viability. All forms, seeds, births, deaths and empty strata retained. Form is not ancestry. Immature means structure below adult target, not age or renderer scale. Last-observed size is not necessarily size at a death transaction. Escrow starts are surviving-state observations, not exact funding events. No direct intake amount, metabolic allocation or causal niche inference. Survivors at the horizon remain censored.',
    by_arm:Object.fromEntries(['baseline','feed'].map(arm=>[arm,aggregate(rows.filter(x=>x.arm===arm))])),rows};
}

if (process.argv[1] && import.meta.url===pathToFileURL(resolve(process.argv[1])).href)
  console.log(JSON.stringify(await reduce(process.argv[2]??'captures/astra-quiet-a89179a'),null,2));
