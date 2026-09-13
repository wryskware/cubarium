// Read-only reduction of the ordinary-fauna developmental flow artifacts.
//
// This checks an artifact for COMPLETE expected coverage and internal agreement. It never
// writes, never replays, and never repairs. It is deliberately not the runtime gate: the
// harness asserted input identity, observer neutrality, reconciliation, closing ecology
// identity and the census reconstruction inside the replay itself, against the retained
// snapshots. What this reducer can do is refuse an artifact that does not carry that
// evidence, or that disagrees with itself.
//
// Usage:
//   node scripts/fauna-development-flow.mjs ARTIFACT.json [ARTIFACT.json ...]
import { createHash } from 'node:crypto';
import * as fs from 'node:fs/promises';
import { resolve, basename } from 'node:path';
import { pathToFileURL } from 'node:url';

export const KIND = 'fauna-development-flow';
export const ARTIFACT_SCHEMA = 1;
export const SNAPSHOT_SCHEMA = 9;
export const BASE_REVISION = '1d7b386';
export const BUILD_LABEL = '0.1.0+1d7b386';
export const HORIZON_TICKS = 144000;
export const RESIDUAL_TOLERANCE = 1e-12;
export const FORM_SLOTS = 8;
/// Forms 0-3 are the ordinary fauna of the pre-hunter baseline. 4-7 never exist there; they
/// are retained as zero-exposure slots and are never dropped.
export const ORDINARY_FORMS = ['grazer', 'glider', 'burrower', 'skimmer'];
export const SKIMMER_FORM = 3;
export const GATE_KEYS = [
  'gate_1_input_identity',
  'gate_2_closing_ecology_identity',
  'gate_3_observer_neutrality',
  'gate_4_flow_stock_reconciliation',
  'census_cross_check',
];
/// Every qualification the artifact must carry in its own text. A reduction that dropped
/// these would read as a stronger claim than the measurement supports.
export const REQUIRED_NOTE_TOPICS = [
  /potential/i,
  /occupancy|encounter/i,
  /upkeep/i,
  /censored/i,
  /build label/i,
  /zero[- ]exposure|forms 4/i,
];

class Refusal extends Error {}
const refuse = (m) => {
  throw new Refusal(m);
};

const isFiniteNumber = (v) => typeof v === 'number' && Number.isFinite(v);
const isCount = (v) => Number.isInteger(v) && v >= 0;
/// u64 hashes are carried as decimal strings; a JS number cannot hold them losslessly, so a
/// numeric hash in an artifact is a defect and not a formatting preference.
const isU64String = (v) => typeof v === 'string' && /^\d{1,20}$/.test(v);

export function parseArgs(args) {
  if (!args.length || args.some((a) => !a || a.startsWith('--')))
    throw new Error('Usage: node scripts/fauna-development-flow.mjs ARTIFACT.json [ARTIFACT.json ...]');
  return args.map((a) => resolve(a));
}

/// Preserve u64 hashes before JSON.parse can round them. The harness already emits them as
/// strings; this also accepts a bare-integer artifact so a malformed one is reported as a
/// hash-precision defect rather than silently mangled.
export function parseArtifactText(text) {
  const guarded = text.replace(/"((?:state|ecology)_hash(?:_fnv1a64)?)":\s*(\d+)/g, '"$1":"$2"');
  let value;
  try {
    value = JSON.parse(guarded);
  } catch (e) {
    refuse(`artifact is not valid JSON: ${e.message}`);
  }
  if (value === null || typeof value !== 'object' || Array.isArray(value))
    refuse('artifact must be a JSON object');
  return value;
}

function checkEnvelope(a) {
  if (a.kind !== KIND) refuse(`kind ${JSON.stringify(a.kind)}, expected ${KIND}`);
  if (a.schema !== ARTIFACT_SCHEMA) refuse(`artifact schema ${a.schema}, expected ${ARTIFACT_SCHEMA}`);
  if (!Number.isInteger(a.seed) || a.seed < 1 || a.seed > 12) refuse(`seed ${a.seed} is not a prescribed seed 1-12`);
  if (a.base_revision !== BASE_REVISION) refuse(`base revision ${a.base_revision}, expected ${BASE_REVISION}`);
  if (a.build_label_supplied !== BUILD_LABEL) refuse(`build label ${a.build_label_supplied}, expected ${BUILD_LABEL}`);
  if (a.horizon_ticks !== HORIZON_TICKS)
    refuse(`horizon ${a.horizon_ticks} is not the retained two-hour opening tick ${HORIZON_TICKS}`);
  if (!isFiniteNumber(a.dt) || a.dt <= 0) refuse('dt must be a positive number');
  if (typeof a.diagnostic_binary_sha256 !== 'string' || !/^[0-9a-f]{64}$/.test(a.diagnostic_binary_sha256))
    refuse('diagnostic_binary_sha256 must be a 64-hex digest');
  // The computed sides name the schema they actually read or wrote; the retained manifest
  // entries name theirs plainly. Each is checked under its own field so a missing one is a
  // refusal rather than a silently skipped check.
  for (const [side, schemaField] of [
    ['input', 'schema_read'],
    ['expected_input', 'schema'],
    ['closing', 'schema_written'],
    ['expected_closing', 'schema'],
  ]) {
    const s = a[side];
    if (!s || typeof s !== 'object') refuse(`missing ${side}`);
    if (s[schemaField] !== SNAPSHOT_SCHEMA)
      refuse(`${side}.${schemaField} is ${s[schemaField]}, expected ${SNAPSHOT_SCHEMA}; this study never crosses a schema`);
    if (typeof s.sha256 !== 'string' || !/^[0-9a-f]{64}$/.test(s.sha256)) refuse(`${side}.sha256 must be a 64-hex digest`);
    if (!isU64String(s.state_hash)) refuse(`${side}.state_hash must be a decimal string, not a JS number`);
  }
  // The identity the harness asserted, restated here so a mislabelled artifact cannot pass.
  if (a.input.sha256 !== a.expected_input.sha256) refuse('input sha256 does not equal the retained tick-zero snapshot');
  if (a.input.state_hash !== a.expected_input.state_hash) refuse('input state hash does not equal the retained tick-zero snapshot');
  if (a.closing.sha256 !== a.expected_closing.sha256) refuse('closing sha256 does not equal the retained closing snapshot');
  if (a.closing.state_hash !== a.expected_closing.state_hash) refuse('closing state hash does not equal the retained closing');
}

function checkGates(a) {
  if (!a.gates || typeof a.gates !== 'object') refuse('missing gates');
  const present = Object.keys(a.gates).sort();
  const expected = [...GATE_KEYS].sort();
  if (present.length !== expected.length || present.some((k, i) => k !== expected[i]))
    refuse(`gates ${present.join(', ')}; expected exactly ${expected.join(', ')}`);
  for (const key of GATE_KEYS) {
    const g = a.gates[key];
    if (g.passed !== true) refuse(`${key} did not pass; a failing gate is preserved, never reduced as coverage`);
    if (Array.isArray(g.checks)) {
      if (!g.checks.length) refuse(`${key} carries no checks`);
      for (const c of g.checks) {
        if (typeof c.field !== 'string') refuse(`${key} has a check without a field name`);
        if (c.equal === false) refuse(`${key} check ${c.field} compared unequal but the gate claims to pass`);
      }
    }
  }
  const g2 = a.gates.gate_2_closing_ecology_identity;
  if (g2.asserted !== true) refuse('gate 2 was not asserted; a prefix run is not two-hour coverage');
  if (g2.schema_read !== SNAPSHOT_SCHEMA || g2.schema_written !== SNAPSHOT_SCHEMA)
    refuse(`gate 2 compared schema ${g2.schema_written} against ${g2.schema_read}; payload identity is only meaningful within one schema`);
  const g4 = a.gates.gate_4_flow_stock_reconciliation;
  const tol = g4.checks.find((c) => c.field === 'tolerance_absolute');
  if (!tol || Number(tol.computed) !== RESIDUAL_TOLERANCE)
    refuse(`gate 4 tolerance ${tol && tol.computed}, expected the stated ${RESIDUAL_TOLERANCE}`);
  for (const field of ['violations', 'unregistered_records']) {
    const c = g4.checks.find((x) => x.field === field);
    if (!c || Number(c.computed) !== 0) refuse(`gate 4 ${field} is ${c && c.computed}, expected 0`);
  }
  const worst = g4.checks.find((c) => c.field === 'worst_residual_per_stock');
  if (!worst) refuse('gate 4 does not report the worst residual per stock');
  for (const [stock, value] of Object.entries(worst.computed)) {
    if (!isFiniteNumber(value) || value < 0) refuse(`gate 4 worst residual for ${stock} is not a magnitude`);
    if (value > RESIDUAL_TOLERANCE) refuse(`gate 4 worst ${stock} residual ${value} exceeds ${RESIDUAL_TOLERANCE}`);
  }
  const census = a.gates.census_cross_check;
  if (census.first_disagreeing_sample !== null)
    refuse(`census reconstruction first disagrees at sample ${census.first_disagreeing_sample}`);
  if (!isCount(census.samples_compared) || census.samples_compared < 1) refuse('census cross-check compared no samples');
  const c = census.closing;
  if (c.reconstructed_population !== c.retained_population)
    refuse(`reconstructed closing population ${c.reconstructed_population} != retained ${c.retained_population}`);
  if (c.reconstructed_by_form.length !== FORM_SLOTS || c.retained_by_form.length !== FORM_SLOTS)
    refuse(`census by form must keep all ${FORM_SLOTS} slots including zeros`);
  for (let f = 0; f < FORM_SLOTS; f++)
    if (c.reconstructed_by_form[f] !== c.retained_by_form[f])
      refuse(`reconstructed closing form ${f} = ${c.reconstructed_by_form[f]} != retained ${c.retained_by_form[f]}`);
  return { samples_compared: census.samples_compared };
}

function checkForms(a) {
  if (!Array.isArray(a.forms) || a.forms.length !== FORM_SLOTS)
    refuse(`forms must carry all ${FORM_SLOTS} slots, zero exposure included`);
  a.forms.forEach((f, i) => {
    if (f.form !== i) refuse(`form slot ${i} reports form ${f.form}`);
    if (i < ORDINARY_FORMS.length && f.name !== ORDINARY_FORMS[i])
      refuse(`form ${i} is named ${f.name}, expected ${ORDINARY_FORMS[i]}`);
    for (const k of ['members_ever', 'founders', 'descendants', 'reached_adult_target', 'alive_at_horizon'])
      if (!isCount(f[k])) refuse(`form ${i} field ${k} is not a count`);
    if (f.founders + f.descendants !== f.members_ever)
      refuse(`form ${i}: founders ${f.founders} + descendants ${f.descendants} != members_ever ${f.members_ever}`);
    if (f.reached_adult_target > f.members_ever) refuse(`form ${i} recruited more adults than it ever had members`);
  });
  for (let i = ORDINARY_FORMS.length; i < FORM_SLOTS; i++)
    if (a.forms[i].members_ever !== 0)
      refuse(`form ${i} has members; the pre-hunter baseline carries only forms 0-${ORDINARY_FORMS.length - 1}`);
}

/// Per-member coverage. This is where "complete" is earned: every member is checked, not a
/// sampled or pooled subset, and every lifecycle edge must be internally consistent.
function checkMembers(a) {
  if (!Array.isArray(a.members) || !a.members.length) refuse('artifact carries no members');
  const byId = new Map();
  const key = (id) => `${id.slot}:${id.generation}`;
  for (const m of a.members) {
    if (!m.id || !isCount(m.id.slot) || !isCount(m.id.generation))
      refuse('every member needs a generation-bearing id; a bare slot is ambiguous under reuse');
    const k = key(m.id);
    if (byId.has(k)) refuse(`duplicate member id ${k}`);
    byId.set(k, m);
  }
  const tally = { founders: 0, descendants: 0, deaths: 0, censored: 0, reuse_pairs: 0, adults: 0 };
  const byForm = new Array(FORM_SLOTS).fill(0);
  for (const m of a.members) {
    const k = key(m.id);
    if (!Number.isInteger(m.form) || m.form < 0 || m.form >= FORM_SLOTS) refuse(`member ${k} has form ${m.form}`);
    if (m.form < ORDINARY_FORMS.length && m.form_name !== ORDINARY_FORMS[m.form])
      refuse(`member ${k} form ${m.form} named ${m.form_name}`);
    if (m.origin !== 'founder' && m.origin !== 'descendant') refuse(`member ${k} origin ${m.origin}`);

    // Lineage: a parent must exist in this artifact, and the root must be a founder.
    if (m.origin === 'founder') {
      tally.founders++;
      if (m.parent !== null) refuse(`founder ${k} carries a parent`);
      if (m.lineage_depth !== 0) refuse(`founder ${k} has lineage depth ${m.lineage_depth}`);
      if (key(m.root) !== k) refuse(`founder ${k} roots at ${key(m.root)}`);
    } else {
      tally.descendants++;
      if (!m.parent) refuse(`descendant ${k} has no parent`);
      if (!byId.has(key(m.parent))) refuse(`descendant ${k} names absent parent ${key(m.parent)}`);
      if (!isCount(m.lineage_depth) || m.lineage_depth < 1) refuse(`descendant ${k} lineage depth ${m.lineage_depth}`);
      const root = byId.get(key(m.root));
      if (!root) refuse(`member ${k} names absent root ${key(m.root)}`);
      if (root.origin !== 'founder') refuse(`member ${k} roots at ${key(m.root)}, which is not a founder`);
      // A paid birth is the only way a descendant enters the world.
      if (!m.birth_payment) refuse(`descendant ${k} has no birth payment; its parent's debit is unaccounted`);
      const p = m.birth_payment;
      const e = p.escrow;
      if (Math.abs(p.parent_reserve_debit - (e.structure + e.reserve)) > RESIDUAL_TOLERANCE)
        refuse(`descendant ${k}: parent reserve debit ${p.parent_reserve_debit} != escrow structure + reserve`);
      if (p.funded_tick > m.born_tick) refuse(`descendant ${k} was funded at ${p.funded_tick}, after it was born`);
      if (p.refunded) refuse(`descendant ${k} was placed from a refunded escrow`);
    }

    byForm[m.form]++;
    if (m.structure.adult_recruitment_tick !== null) {
      tally.adults++;
      if (m.structure.adult_recruitment_tick < m.born_tick)
        refuse(`member ${k} recruited to adult before it was born`);
    }
    if (m.first_observed_tick > m.last_observed_tick) refuse(`member ${k} observation window is inverted`);

    // Reconciliation is per member, not pooled.
    const r = m.reconciliation;
    if (!isCount(r.checks) || r.checks < 1) refuse(`member ${k} was never reconciled`);
    if (r.violations !== 0) refuse(`member ${k} has ${r.violations} reconciliation violations`);
    for (const [stock, v] of Object.entries(r.worst_residual))
      if (!isFiniteNumber(v) || v < 0 || v > RESIDUAL_TOLERANCE)
        refuse(`member ${k} worst ${stock} residual ${v} exceeds ${RESIDUAL_TOLERANCE}`);

    // Potential and actual intake must stay distinguishable, and actual can never exceed it.
    for (const ch of ['frugivory', 'grazing', 'scavenging']) {
      const c = m.intake[ch];
      if (!isFiniteNumber(c.requested_amount) || !isFiniteNumber(c.actual_amount))
        refuse(`member ${k} channel ${ch} lacks a potential/actual pair`);
      if (c.actual_amount > c.requested_amount + RESIDUAL_TOLERANCE)
        refuse(`member ${k} channel ${ch} took ${c.actual_amount} of a ${c.requested_amount} request`);
      if (c.actual_ticks > c.request_ticks) refuse(`member ${k} channel ${ch} fed on more ticks than it asked`);
    }

    // Upkeep demand is carried in three terms; the payment is one amount and is never split.
    const u = m.upkeep;
    const demand = u.demand_maintenance + u.demand_movement + u.demand_sensing;
    if (Math.abs(demand - u.demand_total) > 1e-9)
      refuse(`member ${k} upkeep terms ${demand} do not sum to the recorded demand ${u.demand_total}`);
    if (u.paid_total > u.demand_total + 1e-9) refuse(`member ${k} paid more upkeep than was demanded`);
    if (Math.abs(u.demand_total - u.paid_total - u.shortfall_total) > 1e-9)
      refuse(`member ${k} upkeep shortfall does not close against demand and payment`);

    // The growth gate must be an observation on every stepped tick, open or shut.
    const g = m.growth_gate;
    if (g.entered > g.both_open) refuse(`member ${k} entered growth more often than both sides were open`);
    if (g.both_open > Math.min(g.structure_side_open, g.reserve_side_open))
      refuse(`member ${k} reports both gate sides open more often than either side alone`);
    if (g.observations < g.both_open) refuse(`member ${k} has fewer gate observations than openings`);

    // Boundaries: a death is stamped one tick after the decision, and the interval is explicit.
    if (m.death) {
      tally.deaths++;
      const d = m.death;
      if (d.event_tick !== d.decision_tick + 1)
        refuse(`member ${k} death event ${d.event_tick} is not one tick after the decision ${d.decision_tick}`);
      if (d.ticks_between_last_step_and_event !== d.event_tick - d.last_stepped_tick)
        refuse(`member ${k} final death interval is not the recorded boundary difference`);
      if (!['starvation', 'age', 'collapse'].includes(d.cause)) refuse(`member ${k} died of ${d.cause}`);
      if (d.event_tick > a.horizon_ticks) refuse(`member ${k} died after the horizon`);
      if (d.slot_reused_same_boundary_by) {
        tally.reuse_pairs++;
        const heir = byId.get(key(d.slot_reused_same_boundary_by));
        if (!heir) refuse(`member ${k} names an absent slot heir`);
        if (heir.id.slot !== m.id.slot) refuse(`member ${k} slot heir took a different slot`);
        if (heir.id.generation <= m.id.generation)
          refuse(`member ${k} slot heir did not advance the generation; the ids would be ambiguous`);
        if (!heir.birth_payment || key(heir.birth_payment.slot_freed_same_boundary_by || {}) !== k)
          refuse(`slot reuse at ${k} is recorded on the death but not on the birth`);
      }
    } else {
      // A survivor at the horizon is censored, never a success. The world steps tick `now`
      // and then increments, so the last tick actually stepped before a horizon of H is
      // H - 1. A member placed at the closing boundary itself is never stepped at all.
      tally.censored++;
      const lastStepped = a.horizon_ticks - 1;
      if (m.last_observed_tick !== lastStepped && m.born_tick < a.horizon_ticks)
        refuse(`member ${k} neither died nor was stepped through the final tick ${lastStepped}`);
    }
  }
  // Every birth taking a freed slot must be matched by the death that freed it.
  for (const m of a.members) {
    const freed = m.birth_payment && m.birth_payment.slot_freed_same_boundary_by;
    if (!freed) continue;
    const dead = byId.get(key(freed));
    if (!dead || !dead.death || key(dead.death.slot_reused_same_boundary_by || {}) !== key(m.id))
      refuse(`birth ${key(m.id)} claims a freed slot the death does not confirm`);
  }
  // The per-member records must reproduce the form table and the closing census on their own.
  a.forms.forEach((f, i) => {
    if (f.members_ever !== byForm[i]) refuse(`form ${i} claims ${f.members_ever} members, ${byForm[i]} are recorded`);
  });
  const alive = a.gates.census_cross_check.closing.reconstructed_population;
  if (tally.censored !== alive)
    refuse(`${tally.censored} members are censored at the horizon but the closing census reconstructs ${alive}`);
  return tally;
}

function checkNotes(a) {
  if (!Array.isArray(a.notes) || !a.notes.length) refuse('artifact carries no qualifications');
  const text = a.notes.join('\n');
  for (const topic of REQUIRED_NOTE_TOPICS)
    if (!topic.test(text)) refuse(`artifact notes do not qualify ${topic}`);
}

export function reduceArtifact(artifact, { sha256 = null, name = 'artifact' } = {}) {
  checkEnvelope(artifact);
  const census = checkGates(artifact);
  checkForms(artifact);
  const tally = checkMembers(artifact);
  checkNotes(artifact);
  const skimmer = artifact.forms[SKIMMER_FORM];
  return {
    name,
    seed: artifact.seed,
    sha256,
    base_revision: artifact.base_revision,
    snapshot_schema: SNAPSHOT_SCHEMA,
    input_sha256: artifact.input.sha256,
    closing_sha256: artifact.closing.sha256,
    gates_passed: GATE_KEYS.length,
    census_samples_compared: census.samples_compared,
    reconciliation_checks: artifact.world.totals.reconciliation_checks,
    members: artifact.members.length,
    ...tally,
    skimmer: {
      members_ever: skimmer.members_ever,
      founders: skimmer.founders,
      descendants: skimmer.descendants,
      reached_adult_target: skimmer.reached_adult_target,
      alive_at_horizon: skimmer.alive_at_horizon,
      deaths_by_cause: skimmer.deaths_by_cause,
    },
  };
}

export async function reduceFile(path) {
  const bytes = await fs.readFile(path);
  const sha256 = createHash('sha256').update(bytes).digest('hex');
  const artifact = parseArtifactText(bytes.toString('utf8'));
  return reduceArtifact(artifact, { sha256, name: basename(path) });
}

export async function main(argv) {
  const paths = parseArgs(argv);
  const rows = [];
  for (const p of paths) rows.push(await reduceFile(p));
  for (const r of rows) {
    console.log(`${r.name}  seed ${r.seed}`);
    console.log(`  artifact sha256 ${r.sha256}`);
    console.log(`  base ${r.base_revision}, snapshot schema ${r.snapshot_schema}`);
    console.log(`  input ${r.input_sha256}`);
    console.log(`  closing ${r.closing_sha256}`);
    console.log(`  ${r.gates_passed} gates passed, ${r.census_samples_compared} census samples compared`);
    console.log(`  ${r.members} members (${r.founders} founders, ${r.descendants} descendants), ${r.reconciliation_checks} reconciliation checks`);
    console.log(`  ${r.deaths} deaths, ${r.censored} censored at the horizon, ${r.adults} reached the adult target, ${r.reuse_pairs} same-boundary slot reuse pairs`);
    const s = r.skimmer;
    console.log(`  skimmer: ${s.members_ever} ever (${s.founders} founder, ${s.descendants} born), ${s.reached_adult_target} reached adult, ${s.alive_at_horizon} censored, deaths ${JSON.stringify(s.deaths_by_cause)}`);
  }
  console.log(`\n${rows.length} artifact(s) reduced; every gate and every member check passed.`);
  console.log('This is an artifact reduction, not a replay: the runtime gates were asserted');
  console.log('inside the harness against the retained snapshots, and are restated here.');
  return rows;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  main(process.argv.slice(2)).catch((e) => {
    console.error(`fauna-development-flow: ${e.message}`);
    process.exit(1);
  });
}
