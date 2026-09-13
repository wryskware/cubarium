// Read-only parity and divergence reduction for the size-aware growth-gate experiment.
//
// Two questions, answered separately because they need different evidence.
//
//   REFERENCE PARITY — does the experiment build, running the unchanged charge80 recipe,
//   reproduce the retained 512ee52 artifacts? The `.cubw` header carries the build id string,
//   which differs between `0.1.0+512ee52` and this build, so the file SHA256 *must* differ and
//   claiming otherwise would be false. The payload is what carries the world: its CRC32, its
//   FNV state hash and its length are recomputed from the retained and new bytes and must be
//   identical, and the recorded event and census streams must match line for line. That is the
//   schema-12 projection, stated in one place rather than assumed.
//
//   CANDIDATE DIVERGENCE — where does the size-gate recipe first depart from the reference?
//   The semantic profile selector differs from tick zero by construction, so a raw stream or
//   hash comparison would report a difference that is the experiment, not a finding. The
//   projection below removes exactly the selector and its derived diagnostic metadata from the
//   census rows — named field by field, nothing wildcarded — and then requires equality up to
//   the first tick whose member stocks differ. That tick is the first altered growth
//   transaction, and after it the worlds are legitimately different and are not compared.
//
// Nothing here judges biology. A pilot that grows nothing is a result, not a failure.
import assert from 'node:assert/strict';
import {readFile, readdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve, join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {inspectSnapshot} from './prepare-hunter-worlds.mjs';

const json = async path => JSON.parse(await readFile(path, 'utf8'));
const lines = async path => (await readFile(path, 'utf8')).split('\n').filter(Boolean);

export const ARMS = ['untouched', 'budget_control', 'specialist_off', 'specialist_on',
  'facultative_off', 'facultative_on'];
export const SNAPSHOT_SCHEMA = 12;
export const SNAPSHOT_FILES = ['post-initialization.cubw', 'closing.cubw'];

/// The exact fields the build id is allowed to move, and nothing else. A parity check that
/// silently ignored a whole record would not be one.
export const BUILD_DEPENDENT_FIELDS = ['sha256', 'build'];
/// The census fields the profile selector legitimately changes. Named individually: the
/// selector is the experiment, its resolved oxidation metadata is derived from it, and
/// everything else in the row is ecology that must still agree.
export const SELECTOR_PROJECTED_FIELDS = [
  'hunter_state.profile.version',
];

/// The ledger's own frozen reconciliation tolerance, restated here so a run cannot widen the
/// bar its evidence is judged against by reporting a looser number.
export const RESIDUAL_TOLERANCE = 1e-9;
/// The four reserve intake channels, named so no single one can stand in for the total.
export const INTAKE_SOURCES = ['digestion', 'frugivory', 'grazing', 'scavenging'];

/// Validate one arm's per-member flow record.
///
/// This is the evidence the census could not give, so it is checked rather than relayed: a
/// member the arm held and the ledger missed, a source total that is not actually recorded per
/// source, an incomplete horizon, or a payment that does not reconcile all fail the arm.
export function checkFlow(flow, summary) {
  const problems = [];
  const need = (ok, detail) => {if (!ok) problems.push(detail);};
  if (!flow) return ['no flow.json: the run produced no mutation-site record for this arm'];

  need(flow.kind === 'per-member-mutation-site-flow', `unexpected flow record kind ${flow.kind}`);
  need(flow.complete_horizon === true,
    `incomplete horizon: ${flow.elapsed_ticks} of ${flow.planned_ticks} ticks (${flow.incomplete_reason ?? 'no reason recorded'})`);
  need(flow.closing_tick === summary.closing_tick,
    `flow closing tick ${flow.closing_tick} disagrees with the summary's ${summary.closing_tick}`);
  need(flow.residual_tolerance === RESIDUAL_TOLERANCE,
    `the run reported tolerance ${flow.residual_tolerance}, not the frozen ${RESIDUAL_TOLERANCE}`);
  need(flow.recorded_members >= flow.expected_members,
    `${flow.recorded_members} records for ${flow.expected_members} members the arm held`);

  const probe = flow.observer_neutrality_probe;
  need(probe?.state_equal === true && probe?.event_records_equal === true,
    'the in-run observer-neutrality probe did not report both modes agreeing');
  need(Number.isSafeInteger(probe?.ticks) && probe.ticks > 0, 'the neutrality probe ran no ticks');

  const members = flow.ledger?.members ?? [];
  need(members.length === flow.recorded_members, 'the member list disagrees with its own count');
  for (const m of members) {
    const who = `${m.id.slot}:${m.id.generation}`;
    // Source identity: every channel individually present, finite and non-negative, and the
    // total is their sum rather than any one of them.
    let total = 0;
    for (const source of INTAKE_SOURCES) {
      const v = m[source]?.to_reserve;
      if (typeof v !== 'number' || !Number.isFinite(v) || v < 0) {
        problems.push(`${who}: ${source}.to_reserve is ${v}`);
        continue;
      }
      total += v;
    }
    need(Number.isFinite(total), `${who}: intake sources do not sum to a finite total`);
    need(m.residual?.violations === 0, `${who}: ${m.residual?.violations} reconciliation violations`);
    for (const field of ['max_structure', 'max_reserve', 'max_energy']) {
      const v = m.residual?.[field];
      need(Number.isFinite(v) && v >= 0 && v <= RESIDUAL_TOLERANCE,
        `${who}: residual ${field} is ${v}`);
    }
    // Paid construction: what was built came out of reserve one for one.
    const built = m.growth?.structure_gained ?? 0, spent = m.growth?.reserve_spent ?? 0;
    need(Math.abs(built - spent) <= RESIDUAL_TOLERANCE,
      `${who}: built ${built} structure against ${spent} reserve spent`);
    need((built > 0) === (m.gate?.first_growth_tick !== null && m.gate?.first_growth_tick !== undefined),
      `${who}: growth ${built} disagrees with first_growth_tick ${m.gate?.first_growth_tick}`);
    // The binding caps must account for exactly the steps taken.
    const caps = m.growth?.bound_by_rate + m.growth?.bound_by_remaining_structure
      + m.growth?.bound_by_reserve + m.growth?.bound_by_energy;
    need(caps === m.growth?.ticks,
      `${who}: ${caps} attributed caps for ${m.growth?.ticks} growth steps`);
  }
  return problems;
}

/// The actual first altered growth transaction, from the mutation-site record rather than from
/// the 200-tick census, which can only bound it.
export function firstGrowthFromFlow(flow) {
  let first = null;
  for (const m of flow?.ledger?.members ?? []) {
    const tick = m.gate?.first_growth_tick;
    if (tick === null || tick === undefined) continue;
    if (first === null || tick < first.tick)
      first = {tick, member: `${m.id.slot}:${m.id.generation}`,
        gate_at_first_growth: m.gate.gate_reserve_at_first_growth,
        structure_at_first_growth: m.gate.structure_at_first_growth};
  }
  return first;
}

/// Compare two snapshots through the payload, which is the world; the header is not.
export function compareSnapshot(a, b) {
  const differences = [];
  // A snapshot that could not be read has no payload facts at all. Two such records agree on
  // every field by being equally absent, so without this they would certify each other as
  // identical — an unreadable pair is missing evidence, not matching evidence.
  for (const [side, s] of [['retained', a], ['rerun', b]])
    if (s?.error || s?.unreadable || !Number.isFinite(s?.crc32))
      differences.push({field: 'readable', side, why: s?.error ?? 'no payload facts recorded'});
  if (differences.length === 0)
    for (const key of ['schema', 'crc32', 'state_hash', 'payload_bytes'])
      if (a[key] !== b[key]) differences.push({field: key, retained: a[key], rerun: b[key]});
  return {
    identical_payload: differences.length === 0,
    differences,
    schema: a.schema,
    state_hash: a.state_hash,
    crc32: a.crc32,
    payload_bytes: a.payload_bytes,
    // Recorded, never asserted equal: the CUBW header embeds the build id.
    retained_sha256: a.sha256, rerun_sha256: b.sha256,
    retained_build: a.build, rerun_build: b.build,
    header_differs_by_build_id: a.sha256 !== b.sha256,
  };
}

const drop = (row, path) => {
  const parts = path.split('.');
  let cursor = row;
  for (const part of parts.slice(0, -1)) {
    if (cursor === null || typeof cursor !== 'object') return;
    cursor = cursor[part];
  }
  if (cursor && typeof cursor === 'object') delete cursor[parts.at(-1)];
};

/// Remove exactly the selector-derived fields from a census row.
export function projectCensusRow(row, fields = SELECTOR_PROJECTED_FIELDS) {
  const copy = structuredClone(row);
  for (const path of fields) drop(copy, path);
  return copy;
}

/// Per-member stocks from a census row, keyed by full slot:generation id.
export function stocksOf(row) {
  const out = {};
  for (const h of row.hunter_stocks ?? [])
    out[`${h.id.slot}:${h.id.generation}`] =
      {structure: h.structure, reserve: h.reserve, energy: h.energy};
  return out;
}

/// Walk two census streams under the projection and report the first tick that differs, plus
/// the first tick whose member *structure* differs — the first altered growth transaction.
export function divergence(referenceRows, candidateRows) {
  const n = Math.min(referenceRows.length, candidateRows.length);
  let firstRowDifference = null, firstStructureDifference = null;
  for (let i = 0; i < n; i++) {
    const a = referenceRows[i], b = candidateRows[i];
    if (a.tick !== b.tick) {
      firstRowDifference ??= {tick: a.tick, why: `census ticks misaligned (${a.tick} vs ${b.tick})`};
      break;
    }
    if (firstStructureDifference === null) {
      const sa = stocksOf(a), sb = stocksOf(b);
      for (const id of new Set([...Object.keys(sa), ...Object.keys(sb)])) {
        const x = sa[id], y = sb[id];
        if (!x || !y || x.structure !== y.structure) {
          firstStructureDifference = {tick: a.tick, member: id,
            reference: x ?? null, candidate: y ?? null};
          break;
        }
      }
    }
    if (firstRowDifference === null) {
      const pa = JSON.stringify(projectCensusRow(a)), pb = JSON.stringify(projectCensusRow(b));
      if (pa !== pb) firstRowDifference = {tick: a.tick, why: 'projected census rows differ'};
    }
    if (firstRowDifference !== null && firstStructureDifference !== null) break;
  }
  // A matching prefix is not identity. Two streams of different length agree on every row
  // they share and still describe different runs, so the lengths are part of the claim.
  const sameLength = referenceRows.length === candidateRows.length;
  if (!sameLength)
    firstRowDifference ??= {tick: null,
      why: `census streams differ in length (${referenceRows.length} vs ${candidateRows.length})`};
  return {
    census_rows_compared: n,
    reference_rows: referenceRows.length,
    candidate_rows: candidateRows.length,
    same_length: sameLength,
    first_projected_row_difference: firstRowDifference,
    first_member_structure_difference: firstStructureDifference,
    identical_under_projection: firstRowDifference === null && sameLength,
    basis: 'census rows compared after removing exactly the selector-derived fields; structure is compared per member by full slot:generation identity',
  };
}

async function armSnapshots(dir) {
  const out = {};
  for (const file of SNAPSHOT_FILES) {
    const bytes = await readFile(join(dir, file));
    try {
      out[file] = inspectSnapshot(bytes, SNAPSHOT_SCHEMA);
    } catch (error) {
      out[file] = {error: error.message};
    }
  }
  return out;
}

async function sha256File(path) {
  return createHash('sha256').update(await readFile(path)).digest('hex');
}

/// The per-arm observations the pilot report cites. Every one is recorded, including zeros.
function armOutcome(summary) {
  const q = summary.reproductive_opportunity ?? {};
  return {
    closing_tick: summary.closing_tick,
    technical_complete: summary.technical_complete,
    complete_experiment_measurement: summary.complete_experiment_measurement,
    termination: summary.termination,
    audit_passed: summary.audit?.passed ?? null,
    audit_failure: summary.audit?.failure ?? null,
    audit_peak_over_fixed_limit: [['material', 0], ['corrected_energy', 1],
      ['independent_energy', 1], ['water', 2]]
      .map(([k, i]) => summary.audit?.[k]?.magnitude / summary.audit?.fixed_limits?.[i]),
    captures: summary.captures,
    offspring: summary.offspring,
    reproduction: summary.reproduction_audit?.counts ?? null,
    closing_hunters: summary.closing_hunters,
    adult_max: summary.adult_max,
    adult_descendants: summary.adult_descendants,
    descendants_that_reproduced: summary.descendants_that_reproduced,
    adult_occupancy_ticks_0_1_2_over2: summary.adult_occupancy_ticks_0_1_2_over2,
    founder_extinction_tick: summary.founder_extinction_tick,
    lineage_extinction_tick: summary.lineage_extinction_tick,
    member_ticks: q.member_ticks ?? null,
    mature_member_ticks: q.age_and_size_ready_member_ticks ?? null,
    max_reserve_fraction: q.max_reserve_fraction ?? null,
    max_energy_fraction: q.max_energy_fraction ?? null,
    charging: summary.oxidation?.charging_above_reference ?? null,
    closing_prey: summary.closing_population - summary.closing_hunters,
    prey_min: summary.prey_min,
    prey_tick_integral: summary.prey_tick_integral,
    closing_state_hash: summary.closing_state_hash,
  };
}

/// The pilot's growth question, answered from the 200-tick census: did any member's structure
/// ever move, and how far. A census cadence bounds *when* a change is first seen, never
/// whether a mutation-site transaction happened between boundaries; the per-member flow ledger
/// is the instrument for that and is not wired into this runner yet.
export function structureObservations(rows) {
  const members = new Map();
  for (const row of rows)
    for (const h of row.hunter_stocks ?? []) {
      const key = `${h.id.slot}:${h.id.generation}`;
      const m = members.get(key) ?? {id: key, samples: 0, first_tick: row.tick,
        adult_structure: h.adult_structure, opening_structure: h.structure,
        min_structure: h.structure, max_structure: h.structure,
        first_increase_tick: null, distinct_structure_values: new Set()};
      if (h.structure > m.max_structure && m.first_increase_tick === null)
        m.first_increase_tick = row.tick;
      m.samples++;
      m.last_tick = row.tick;
      m.min_structure = Math.min(m.min_structure, h.structure);
      m.max_structure = Math.max(m.max_structure, h.structure);
      m.distinct_structure_values.add(h.structure);
      members.set(key, m);
    }
  return [...members.values()].map(m => ({...m,
    distinct_structure_values: [...m.distinct_structure_values].sort((a, b) => a - b),
    structure_ever_increased: m.max_structure > m.opening_structure,
    reached_adult_structure: m.max_structure + 1e-9 >= m.adult_structure}));
}

async function loadRun(root, seeds, {flow = true} = {}) {
  const dir = resolve(root);
  const manifest = await json(join(dir, 'manifest.json'));
  const frozen = await sha256File(join(dir, 'hunter_compare.frozen'));
  const present = (await readdir(dir)).filter(n => n.startsWith('seed-')).sort();
  const arms = new Map();
  for (const seed of seeds)
    for (const arm of ARMS) {
      const armDir = join(dir, `seed-${seed}`, arm);
      arms.set(`${seed}/${arm}`, {
        dir: armDir,
        opening: await json(join(armDir, 'opening.json')),
        summary: await json(join(armDir, 'summary.json')),
        snapshots: await armSnapshots(armDir),
        events: await lines(join(armDir, 'events.jsonl')),
        census: (await lines(join(armDir, 'census.jsonl'))).map(JSON.parse),
        // The earlier pilot predates the ledger, so its arms legitimately carry none.
        flow: flow ? await json(join(armDir, 'flow.json')).catch(() => null) : null,
      });
    }
  return {dir, manifest, frozen, seed_dirs: present, arms};
}

/// Compare a run against an earlier one of the same recipe, through the payload and the event
/// stream. Used to show at full horizon that switching the observer on moved nothing: the
/// earlier pilot ran the identical recipe with no ledger in the loop at all.
async function comparePriorRun(run, priorRoot, seeds) {
  const prior = await loadRun(priorRoot, seeds, {flow: false});
  const rows = [];
  for (const [key, now] of run.arms) {
    const before = prior.arms.get(key);
    if (!before) {rows.push({arm: key, reproduced: false, why: 'absent from the earlier run'}); continue;}
    const snapshots = {};
    for (const file of SNAPSHOT_FILES)
      snapshots[file] = compareSnapshot(before.snapshots[file], now.snapshots[file]);
    const eventsEqual = before.events.length === now.events.length
      && before.events.every((l, i) => l === now.events[i]);
    const payload = SNAPSHOT_FILES.every(f => snapshots[f].identical_payload);
    const stateHashEqual = before.summary.closing_state_hash === now.summary.closing_state_hash;
    rows.push({arm: key, reproduced: payload && eventsEqual && stateHashEqual,
      payload_identical: payload, events_identical: eventsEqual,
      closing_state_hash_equal: stateHashEqual,
      prior_events: before.events.length, current_events: now.events.length, snapshots});
  }
  return {prior_root: prior.dir, prior_build: prior.manifest.build,
    prior_executable_sha256: prior.frozen,
    arms: rows.length, arms_reproduced: rows.filter(r => r.reproduced).length, rows};
}

export async function reduce({reference, candidate, retained, seeds, binarySha256,
  priorReference, priorCandidate}) {
  const seedList = seeds.map(Number);
  const ref = await loadRun(reference, seedList);
  const cand = await loadRun(candidate, seedList);
  const problems = [];

  for (const [label, run] of [['reference', ref], ['candidate', cand]]) {
    if (run.frozen !== binarySha256)
      problems.push({what: `${label} executable`, detail:
        `frozen copy is ${run.frozen}, expected the pinned ${binarySha256}`});
    if (run.manifest.ticks !== 144000)
      problems.push({what: `${label} horizon`, detail: `${run.manifest.ticks} ticks`});
  }
  if (ref.manifest.profile_recipe !== 'reserve-targets-charge80-v1')
    problems.push({what: 'reference recipe', detail: ref.manifest.profile_recipe});
  if (cand.manifest.profile_recipe !== 'reserve-targets-charge80-size-gate-v1')
    problems.push({what: 'candidate recipe', detail: cand.manifest.profile_recipe});
  for (const key of ['cohort', 'ticks', 'audit_window', 'care', 'world_oxidation_threshold'])
    if (JSON.stringify(ref.manifest[key]) !== JSON.stringify(cand.manifest[key]))
      problems.push({what: `manifest ${key}`, detail: 'reference and candidate disagree'});
  // Both recipes must resolve the same fixed member threshold: the size gate is not allowed
  // to be a second change to charging.
  if (ref.manifest.member_oxidation_threshold !== cand.manifest.member_oxidation_threshold)
    problems.push({what: 'member oxidation threshold',
      detail: `${ref.manifest.member_oxidation_threshold} vs ${cand.manifest.member_oxidation_threshold} — the size-gate recipe must still charge`});

  // ---- reference parity against the retained artifacts
  const retainedRoot = resolve(retained);
  const parity = [];
  for (const seed of seedList)
    for (const arm of ARMS) {
      const key = `${seed}/${arm}`;
      const old = {
        dir: join(retainedRoot, `seed-${seed}`, arm),
        summary: await json(join(retainedRoot, `seed-${seed}`, arm, 'summary.json')),
        snapshots: await armSnapshots(join(retainedRoot, `seed-${seed}`, arm)),
        events: await lines(join(retainedRoot, `seed-${seed}`, arm, 'events.jsonl')),
        census: await lines(join(retainedRoot, `seed-${seed}`, arm, 'census.jsonl')),
      };
      const now = ref.arms.get(key);
      const snapshots = {};
      for (const file of SNAPSHOT_FILES)
        snapshots[file] = compareSnapshot(old.snapshots[file], now.snapshots[file]);
      const eventsEqual = old.events.length === now.events.length
        && old.events.every((l, i) => l === now.events[i]);
      const nowCensus = (await lines(join(now.dir, 'census.jsonl')));
      const censusEqual = old.census.length === nowCensus.length
        && old.census.every((l, i) => l === nowCensus[i]);
      const row = {arm: key,
        payload_identical: SNAPSHOT_FILES.every(f => snapshots[f].identical_payload),
        events_identical: eventsEqual, census_identical: censusEqual,
        retained_closing_state_hash: old.summary.closing_state_hash,
        rerun_closing_state_hash: now.summary.closing_state_hash,
        closing_state_hash_equal:
          old.summary.closing_state_hash === now.summary.closing_state_hash,
        snapshots};
      if (!row.payload_identical || !row.events_identical || !row.census_identical
        || !row.closing_state_hash_equal)
        problems.push({what: `reference parity ${key}`, detail: 'the rerun did not reproduce the retained arm'});
      parity.push(row);
    }

  // ---- candidate divergence from the reference, under the documented projection
  const divergences = [];
  for (const seed of seedList)
    for (const arm of ARMS) {
      const key = `${seed}/${arm}`;
      const a = ref.arms.get(key), b = cand.arms.get(key);
      const d = divergence(a.census, b.census);
      const eventsEqual = a.events.length === b.events.length
        && a.events.every((l, i) => l === b.events[i]);
      divergences.push({arm: key, ...d, event_streams_identical: eventsEqual,
        reference_events: a.events.length, candidate_events: b.events.length});
    }

  // ---- the mutation-site flow records, which the census could only bound
  const flow = {};
  for (const [label, run] of [['reference', ref], ['candidate', cand]]) {
    flow[label] = {};
    for (const [key, a] of run.arms) {
      const found = checkFlow(a.flow, a.summary);
      if (found.length)
        problems.push({what: `${label} flow ${key}`, detail: found.join('; ')});
      flow[label][key] = {
        present: a.flow !== null,
        complete_horizon: a.flow?.complete_horizon ?? null,
        expected_members: a.flow?.expected_members ?? null,
        recorded_members: a.flow?.recorded_members ?? null,
        neutrality_probe: a.flow?.observer_neutrality_probe ?? null,
        first_growth: a.flow ? firstGrowthFromFlow(a.flow) : null,
        problems: found,
        members: (a.flow?.ledger?.members ?? []).map(m => ({
          id: `${m.id.slot}:${m.id.generation}`, origin: m.origin,
          born_tick: m.born_tick, end_tick: m.end_tick, end_cause: m.end_cause,
          censored_alive: m.end_tick === null,
          open_stocks: m.open_stocks, close_stocks: m.close_stocks, death_stocks: m.death_stocks,
          intake_by_source: Object.fromEntries(
            INTAKE_SOURCES.map(s => [s, m[s].to_reserve])),
          intake_total: INTAKE_SOURCES.reduce((a2, s) => a2 + m[s].to_reserve, 0),
          growth: {ticks: m.growth.ticks, structure_gained: m.growth.structure_gained,
            reserve_spent: m.growth.reserve_spent, energy_cost: m.growth.energy_cost,
            heat: m.growth.heat,
            bound_by: {rate: m.growth.bound_by_rate,
              remaining_structure: m.growth.bound_by_remaining_structure,
              reserve: m.growth.bound_by_reserve, energy: m.growth.bound_by_energy}},
          gate: {last: m.gate.gate_reserve_last, min: m.gate.gate_reserve_min,
            max: m.gate.gate_reserve_max, legacy_last: m.gate.legacy_gate_reserve_last,
            at_first_growth: m.gate.gate_reserve_at_first_growth,
            structure_at_first_growth: m.gate.structure_at_first_growth,
            first_growth_tick: m.gate.first_growth_tick,
            first_adult_tick: m.gate.first_adult_tick,
            max_structure: m.gate.max_structure,
            observed_ticks: m.gate.observed_ticks,
            branch_entered_ticks: m.gate.branch_entered_ticks},
          oxidation_reserve_burned: m.oxidation.reserve_burned,
          upkeep_paid: m.upkeep.paid, strike_paid: m.strike.paid,
          residual: m.residual, bins: m.bins.length})),
      };
    }
  }

  // ---- the same recipe, run before the observer existed
  const priorRuns = {};
  for (const [label, root, run] of [['reference', priorReference, ref],
    ['candidate', priorCandidate, cand]]) {
    if (!root) continue;
    priorRuns[label] = await comparePriorRun(run, root, seedList);
    if (priorRuns[label].arms_reproduced !== priorRuns[label].arms)
      problems.push({what: `${label} vs its earlier pilot`,
        detail: 'the instrumented build did not reproduce the pre-observer run'});
  }

  const outcomes = {};
  for (const [label, run] of [['reference', ref], ['candidate', cand]]) {
    outcomes[label] = {};
    for (const [key, a] of run.arms)
      outcomes[label][key] = {...armOutcome(a.summary),
        members: structureObservations(a.census)};
  }

  const grew = Object.values(outcomes.candidate)
    .flatMap(o => o.members).filter(m => m.structure_ever_increased);
  return {
    kind: 'hunter-size-gate-pilot-parity',
    pilot: true,
    pilot_note: 'A SEED SUBSET. Not a cohort result and not a viability claim: the unrun seeds were never started, and one seed cannot answer maturation or recruitment.',
    seeds: seedList,
    pinned_binary_sha256: binarySha256,
    runs: {reference: ref.dir, candidate: cand.dir, retained_reference: retainedRoot},
    builds: {reference: ref.manifest.build, candidate: cand.manifest.build,
      retained: parity[0]?.snapshots['closing.cubw']?.retained_build ?? null},
    manifests: {reference_recipe: ref.manifest.profile_recipe,
      candidate_recipe: cand.manifest.profile_recipe,
      reference_scope: ref.manifest.profile_recipe_scope,
      candidate_scope: cand.manifest.profile_recipe_scope,
      member_oxidation: {reference: ref.manifest.member_oxidation_threshold,
        candidate: cand.manifest.member_oxidation_threshold},
      seed_subset_pilot: {reference: ref.manifest.seed_subset_pilot,
        candidate: cand.manifest.seed_subset_pilot}},
    problems,
    reference_parity: {
      arms: parity.length,
      arms_reproduced: parity.filter(r => r.payload_identical && r.events_identical
        && r.census_identical && r.closing_state_hash_equal).length,
      projection: 'the CUBW header embeds the build id, so the file SHA256 differs by construction and is recorded rather than asserted. Parity is the payload: schema, CRC32, FNV state hash and length, plus the recorded event and census streams line for line.',
      rows: parity,
    },
    candidate_divergence: {
      projection: `census rows compared after removing exactly ${JSON.stringify(SELECTOR_PROJECTED_FIELDS)}; no raw state hash is claimed equal while the selector differs`,
      census_only_bounds_it: 'the 200-tick census bounds when a divergence is first visible; the actual first altered growth transaction is the mutation-site record in flow_records',
      rows: divergences,
    },
    flow_records: flow,
    observer_neutrality_at_full_horizon: priorRuns,
    outcomes,
    growth_observed: {
      members_whose_structure_increased: grew.length,
      detail: grew,
      basis: 'the 200-tick census. It bounds when a change is first seen, never whether a mutation-site transaction happened between boundaries.',
      instrument_gap: 'the per-member flow ledger (cubarium_core::flow) is adapted for the moving gate and unit-tested, but is not yet wired into hunter_compare, so this pilot has no per-tick growth record.',
    },
    limits: 'Read-only over recorded artifacts. It does not re-run the executable, decode semantic WorldState, or judge biology. A pilot that grows nothing is a recorded result. Identity after the first altered growth transaction is not expected and is not checked.',
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    const argv = process.argv.slice(2);
    const flag = name => {
      const i = argv.indexOf(name);
      assert(i >= 0 && argv[i + 1], `missing ${name}`);
      return argv[i + 1];
    };
    const optional = name => {
      const i = argv.indexOf(name);
      return i >= 0 ? argv[i + 1] : undefined;
    };
    const result = await reduce({
      reference: flag('--reference'), candidate: flag('--candidate'),
      retained: flag('--retained'), binarySha256: flag('--binary-sha256'),
      seeds: flag('--seeds').split(','),
      priorReference: optional('--prior-reference'),
      priorCandidate: optional('--prior-candidate'),
    });
    console.log(JSON.stringify(result, null, 1));
    if (result.problems.length) {
      console.error(`Parity refused: ${result.problems.map(p => p.what).join('; ')}`);
      process.exitCode = 1;
    }
  } catch (error) {console.error(`Reduction refused: ${error.message}`); process.exitCode = 1;}
}
