// Read-only aggregation of the pinned juvenile-flow diagnostic replays.
//
// What this can and cannot verify is kept explicit, because an earlier version of this file
// blurred it. Two of the replay's four gates are artifact facts this reducer can establish on
// its own; the other two are observations only the running replay could make.
//
//   independently_verified  — the retained `post-initialization.cubw` and `closing.cubw` are
//                             read, their CUBW envelopes parsed and their CRC32, FNV state
//                             hash and SHA256 recomputed from the actual bytes, then compared
//                             against `opening.json`, `summary.json` AND the report. This is
//                             not two JSON labels agreeing with each other.
//   reported_by_replay      — observer neutrality and flow/stock reconciliation. Reproducing
//                             those needs the replay itself. Here they are validated for
//                             shape, sign, finiteness and against this file's own frozen
//                             tolerance, and reported as the replay's claims.
//
// The cohort is a frozen set of eleven birth-producing arms. A missing, duplicate, unexpected
// or malformed arm is a failure, not a smaller cohort: statistics are computed only from arms
// that passed every check, and the output says which were excluded and why.
//
// Deliberately NOT done, per the independent review and root's audit:
//   * no counterfactual is inferred. `above_reference` classifies recorded states; the
//     fixed-intake ceiling below is bookkeeping under an explicit no-new-intake condition and
//     is not a causal partition of the cohort;
//   * the clamped upkeep payment is never split across its three demanded terms;
//   * gate observation ticks and reconciliation checks are separate counts;
//   * reserve intake is reported per source. Digestion is not "reserve in".
import assert from 'node:assert/strict';
import {readFile, readdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve, join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {inspectSnapshot} from './prepare-hunter-worlds.mjs';

const json = async path => JSON.parse(await readFile(path, 'utf8'));
const sum = xs => xs.reduce((a, b) => a + b, 0);
const ratio = (n, d) => d === 0 ? null : n / d;
export const idKey = id => `${id.slot}:${id.generation}`;
/// Generation-bearing and arm-scoped: seed 1's two hunting arms both contain a child `17:3`.
export const childKey = (seed, arm, id) => `${seed}/${arm}/${typeof id === 'string' ? id : idKey(id)}`;

/// The frozen cohort. These are the eleven arms of the corrected charging cohort that produced
/// paid offspring; the reduction covers exactly this set or it fails.
export const EXPECTED_ARMS = [
  {seed: 1, arm: 'specialist_on'}, {seed: 5, arm: 'specialist_on'},
  {seed: 7, arm: 'specialist_on'}, {seed: 8, arm: 'specialist_on'},
  {seed: 1, arm: 'facultative_on'}, {seed: 2, arm: 'facultative_on'},
  {seed: 5, arm: 'facultative_on'}, {seed: 6, arm: 'facultative_on'},
  {seed: 7, arm: 'facultative_on'}, {seed: 8, arm: 'facultative_on'},
  {seed: 12, arm: 'facultative_on'},
];
export const EXPECTED_CHILDREN = 18;
export const EXPECTED_MEMBERS = 29;
export const PLANNED_TICKS = 144000;
export const OPENING_TICK = 144000;
export const SNAPSHOT_SCHEMA = 12;
/// Frozen here to match `cubarium_core::flow::RESIDUAL_TOLERANCE`. Never read from a report:
/// a report that claimed a looser tolerance would be accepting looser evidence.
export const RESIDUAL_TOLERANCE = 1e-9;
export const armKey = (seed, arm) => `${seed}/${arm}`;

/// The three reserve intake channels a member can use, kept named so no caller can quietly
/// substitute one for the total.
export const INTAKE_SOURCES = ['digestion', 'frugivory', 'grazing', 'scavenging'];

const FLOW_GROUPS = {
  digestion: ['ticks', 'material', 'to_reserve', 'energy_gain', 'to_detritus', 'heat'],
  frugivory: ['ticks', 'material', 'to_reserve', 'energy_gain', 'heat'],
  grazing: ['ticks', 'material', 'to_reserve', 'energy_gain', 'heat'],
  scavenging: ['ticks', 'material', 'to_reserve', 'energy_gain', 'heat'],
  oxidation: ['ticks', 'reserve_burned', 'energy_gained', 'heat', 'above_reference_ticks',
    'above_reference_reserve_burned'],
  growth: ['ticks', 'reserve_spent', 'structure_gained', 'energy_cost', 'heat',
    'bound_by_rate', 'bound_by_remaining_structure', 'bound_by_reserve', 'bound_by_energy'],
  upkeep: ['ticks', 'demanded_maintenance', 'demanded_move', 'demanded_sense', 'demanded_total',
    'paid', 'shortfall', 'shortfall_ticks'],
  strike: ['ticks', 'demanded', 'paid', 'shortfall'],
  handling: ['ticks', 'demanded', 'paid', 'unaffordable_ticks'],
  funding: ['funded_count', 'reserve_debit', 'energy_debit', 'build_heat', 'escrow_structure',
    'escrow_reserve', 'escrow_energy', 'refunded_count', 'refunded_reserve', 'refunded_energy'],
};

/// Every recorded flow must be a finite, non-negative number, and every stock a finite one.
/// A NaN or a negative accumulator is a malformed record, not a small anomaly to average over.
export function validateMemberNumbers(member) {
  const problems = [];
  const check = (path, value, {allowNegative = false} = {}) => {
    if (typeof value !== 'number' || !Number.isFinite(value)) {
      problems.push({field: path, value: String(value), why: 'not a finite number'});
      return;
    }
    if (!allowNegative && value < 0) problems.push({field: path, value, why: 'negative'});
  };
  for (const [group, fields] of Object.entries(FLOW_GROUPS)) {
    const g = member[group];
    if (g === undefined || g === null) {problems.push({field: group, value: null, why: 'missing group'}); continue;}
    for (const f of fields) check(`${group}.${f}`, g[f]);
  }
  for (const [name, stocks] of [['open_stocks', member.open_stocks], ['close_stocks', member.close_stocks],
    ['death_stocks', member.death_stocks]]) {
    if (stocks === null || stocks === undefined) {
      if (name === 'death_stocks') continue; // a censored survivor legitimately has none
      problems.push({field: name, value: null, why: 'missing'});
      continue;
    }
    for (const f of ['structure', 'reserve', 'energy']) check(`${name}.${f}`, stocks[f]);
  }
  for (const f of ['observed_ticks', 'structure_below_adult_ticks', 'reserve_above_min_ticks',
    'branch_entered_ticks', 'gate_reserve', 'max_reserve', 'reserve_sum', 'max_energy',
    'energy_sum', 'reserve_zero_ticks'])
    check(`gate.${f}`, member.gate?.[f]);
  for (const f of ['checked_ticks', 'max_structure', 'max_reserve', 'max_energy', 'violations'])
    check(`residual.${f}`, member.residual?.[f]);
  for (const f of ['juvenile_ticks', 'adult_ticks']) check(f, member[f]);
  if (!Number.isSafeInteger(member.born_tick) || member.born_tick < 0)
    problems.push({field: 'born_tick', value: member.born_tick, why: 'not a tick'});
  if (member.end_tick !== null && !Number.isSafeInteger(member.end_tick))
    problems.push({field: 'end_tick', value: member.end_tick, why: 'not a tick or null'});
  if (!['Founder', 'Descendant'].includes(member.origin))
    problems.push({field: 'origin', value: member.origin, why: 'unknown origin'});
  return problems;
}

/// Per-member reconciliation, judged against this file's frozen tolerance rather than the
/// tolerance the report happens to state.
export function checkMemberReconciliation(member) {
  const problems = [];
  if (member.residual.violations !== 0)
    problems.push({field: 'residual.violations', value: member.residual.violations, why: 'nonzero'});
  for (const f of ['max_structure', 'max_reserve', 'max_energy'])
    if (member.residual[f] > RESIDUAL_TOLERANCE)
      problems.push({field: `residual.${f}`, value: member.residual[f],
        why: `exceeds the frozen tolerance ${RESIDUAL_TOLERANCE}`});
  return problems;
}

/// The label-level identity comparison: the report against `opening.json` and `summary.json`.
///
/// This is two JSON records agreeing with each other. It is necessary but weak, and
/// [`checkArtifactIdentity`] is the one that reads the retained bytes.
export function checkReportedIdentity(report, opening, summary) {
  const failures = [];
  const need = (gate, ok, detail) => {if (!ok) failures.push({gate, detail});};
  need('horizon', report.replayed_ticks === PLANNED_TICKS,
    `replayed ${report.replayed_ticks} ticks, not the recorded ${PLANNED_TICKS}`);
  need('horizon', report.opening?.tick === OPENING_TICK,
    `opening tick ${report.opening?.tick}, not ${OPENING_TICK}`);
  need('reported_input_identity', report.opening?.snapshot_sha256 === opening.post_snapshot_sha256,
    'reported opening sha256 differs from opening.json');
  need('reported_input_identity', report.opening?.state_hash === opening.post_state_hash,
    'reported opening state hash differs from opening.json');
  const c = report.closing_identity;
  need('reported_output_identity', c?.checked === true, 'the replay did not assert closing identity');
  need('reported_output_identity', c?.closing_snapshot_sha256 === summary.closing_snapshot_sha256,
    'reported closing sha256 differs from summary.json');
  need('reported_output_identity', c?.closing_state_hash === summary.closing_state_hash,
    'reported closing state hash differs from summary.json');
  need('provenance', report.opening?.arm === opening.arm, 'arm label mismatch');
  need('provenance', report.opening?.profile_recipe === opening.profile_recipe, 'recipe mismatch');
  return failures;
}

/// Everything checkable without opening the retained snapshots: the label-level identity, the
/// replay's own reported claims, and the per-member evidence the ledger carries.
///
/// This is deliberately **not** an independent re-derivation of all four gates. Snapshot
/// identity is only re-derived by [`checkArtifactIdentity`], which reads the `.cubw` bytes;
/// observer neutrality and per-tick reconciliation cannot be re-derived without the replay.
export function checkGates(report, opening, summary) {
  const failures = [
    ...checkReportedIdentity(report, opening, summary),
    ...checkReportedClaims(report),
  ];
  for (const m of report.ledger?.members ?? []) {
    const label = m.id ? idKey(m.id) : '?';
    for (const p of validateMemberNumbers(m))
      failures.push({gate: 'malformed', detail: `${label} ${p.field}: ${p.why} (${p.value})`});
    for (const p of checkMemberReconciliation(m))
      failures.push({gate: 'member_reconciliation', detail: `${label} ${p.field}: ${p.why} (${p.value})`});
    const b = boundaryAccounting(m);
    if (!b.identity_holds)
      failures.push({gate: 'boundary_identity',
        detail: `${label} ${b.reconciliation_checks} checks vs ${b.gate_observation_ticks} gate ticks`});
  }
  return failures;
}

/// Gates 1 and 2, established from the retained bytes rather than from agreeing JSON labels.
export function checkArtifactIdentity(report, opening, summary, actual) {
  const failures = [];
  const need = (gate, ok, detail) => {if (!ok) failures.push({gate, detail});};

  // The opening file itself.
  need('input_identity', actual.opening.schema === SNAPSHOT_SCHEMA,
    `opening snapshot is schema ${actual.opening.schema}, not ${SNAPSHOT_SCHEMA}`);
  need('input_identity', actual.opening.sha256 === opening.post_snapshot_sha256,
    'recomputed opening sha256 differs from opening.json');
  need('input_identity', actual.opening.state_hash === opening.post_state_hash,
    'recomputed opening state hash differs from opening.json');
  need('input_identity', actual.opening.sha256 === report.opening.snapshot_sha256,
    'recomputed opening sha256 differs from the report');
  need('input_identity', actual.opening.state_hash === report.opening.state_hash,
    'recomputed opening state hash differs from the report');

  // The closing file itself.
  const c = report.closing_identity;
  need('output_identity', c?.checked === true, 'the replay did not assert closing identity');
  need('output_identity', actual.closing.schema === SNAPSHOT_SCHEMA,
    `closing snapshot is schema ${actual.closing.schema}, not ${SNAPSHOT_SCHEMA}`);
  need('output_identity', actual.closing.sha256 === summary.closing_snapshot_sha256,
    'recomputed closing sha256 differs from summary.json');
  need('output_identity', actual.closing.state_hash === summary.closing_state_hash,
    'recomputed closing state hash differs from summary.json');
  need('output_identity', actual.closing.sha256 === c?.closing_snapshot_sha256,
    'recomputed closing sha256 differs from the report');
  need('output_identity', actual.closing.state_hash === c?.closing_state_hash,
    'recomputed closing state hash differs from the report');
  need('output_identity', actual.opening.build === actual.closing.build,
    `snapshot build labels differ: ${actual.opening.build} vs ${actual.closing.build}`);

  need('provenance', report.opening.arm === opening.arm, 'arm label mismatch');
  need('provenance', report.opening.profile_recipe === opening.profile_recipe, 'recipe mismatch');
  return failures;
}

/// Gates 3 and 4 as the replay reported them. Validated for shape and sign; not re-derived.
export function checkReportedClaims(report) {
  const failures = [];
  const need = (gate, ok, detail) => {if (!ok) failures.push({gate, detail});};
  const n = report.observer_neutrality;
  need('reported_observer_neutrality', n?.checked === true && n.closing_bytes_equal === true
    && n.state_hash_equal === true, 'the replay did not report both modes agreeing');
  need('reported_observer_neutrality', Number.isSafeInteger(n?.event_ticks_compared)
    && n.event_ticks_compared > 0, 'no event records were reported compared');
  const r = report.reconciliation;
  need('reported_reconciliation', r?.violations === 0, `${r?.violations} reported violations`);
  need('reported_reconciliation', r?.tolerance === RESIDUAL_TOLERANCE,
    `reported tolerance ${r?.tolerance} is not the frozen ${RESIDUAL_TOLERANCE}`);
  need('reported_reconciliation', report.ledger?.unregistered_records === 0,
    `${report.ledger?.unregistered_records} records for unregistered members`);
  return failures;
}

/// Gate observations and reconciliation checks are different counts and are kept apart.
///
/// A member registered before the replay starts is gate-observed and probed on the same ticks,
/// and its death tick swaps the missing end-of-tick probe for one removal-site check — so the
/// two totals match. A member born mid-replay is registered during the birth tick's commit,
/// after the physiology pass that evaluates the gate, so it gains exactly one extra check at
/// its birth boundary. This is asserted per member, founders included.
export function boundaryAccounting(member) {
  const gateTicks = member.gate.observed_ticks;
  const checks = member.residual.checked_ticks;
  const bornMidReplay = member.origin === 'Descendant' && member.parent !== null;
  const died = member.end_tick !== null;
  return {
    gate_observation_ticks: gateTicks,
    reconciliation_checks: checks,
    newborn_boundary_checks: bornMidReplay ? 1 : 0,
    removal_boundary_checks: died ? 1 : 0,
    checks_minus_gate_ticks: checks - gateTicks,
    identity_holds: checks === gateTicks + (bornMidReplay ? 1 : 0),
    basis: 'a mid-replay birth is registered after the physiology pass, so its birth tick is probed but not gate-observed; a death replaces that tick\'s probe with one removal-site check',
  };
}

/// Reserve intake, per source and totalled. The total is the sum of all four channels; no
/// single channel is ever presented as the total.
export function intakeBySource(member) {
  const by = {};
  for (const source of INTAKE_SOURCES) by[source] = member[source].to_reserve;
  return {by_source: by, total: sum(Object.values(by)),
    sources_used: INTAKE_SOURCES.filter(s => by[s] > 0)};
}

/// One member, reduced to what the cohort report cites. Every field is a recorded flow or an
/// identity; nothing is apportioned.
export function memberRecord(seed, arm, m) {
  const intake = intakeBySource(m);
  const finalStocks = m.death_stocks ?? m.close_stocks;
  const reserveOut = m.oxidation.reserve_burned + m.growth.reserve_spent + m.funding.reserve_debit;
  return {
    seed, arm, key: childKey(seed, arm, m.id), id: idKey(m.id), origin: m.origin,
    parent: m.parent ? idKey(m.parent) : null,
    birth_tick: m.born_tick,
    end_tick: m.end_tick,
    end_cause: m.end_cause,
    censored_alive: m.end_tick === null,
    juvenile_ticks: m.juvenile_ticks,
    adult_ticks: m.adult_ticks,
    open_stocks: m.open_stocks,
    close_stocks: m.close_stocks,
    death_stocks: m.death_stocks,
    boundaries: boundaryAccounting(m),
    residual: m.residual,

    growth: {
      branch_entered_ticks: m.gate.branch_entered_ticks,
      steps_taken: m.growth.ticks,
      structure_gained: m.growth.structure_gained,
      reserve_spent: m.growth.reserve_spent,
      energy_cost: m.growth.energy_cost,
      cap_attribution: {
        rate: m.growth.bound_by_rate, remaining_structure: m.growth.bound_by_remaining_structure,
        reserve: m.growth.bound_by_reserve, energy: m.growth.bound_by_energy,
      },
    },
    gate: {
      observed_ticks: m.gate.observed_ticks,
      structure_below_adult_ticks: m.gate.structure_below_adult_ticks,
      reserve_above_min_ticks: m.gate.reserve_above_min_ticks,
      gate_reserve: m.gate.gate_reserve,
      min_reserve_deficit: Number.isFinite(m.gate.min_reserve_deficit) ? m.gate.min_reserve_deficit : null,
      max_reserve: m.gate.max_reserve,
      max_reserve_above_birth_escrow: m.gate.max_reserve > m.open_stocks.reserve,
      mean_reserve: ratio(m.gate.reserve_sum, m.gate.observed_ticks),
      max_energy: m.gate.max_energy,
      mean_energy: ratio(m.gate.energy_sum, m.gate.observed_ticks),
      reserve_zero_ticks: m.gate.reserve_zero_ticks,
    },
    reserve: {
      opening: m.open_stocks.reserve,
      in_by_source: intake.by_source,
      in_sources_used: intake.sources_used,
      in_total: intake.total,
      out_oxidation: m.oxidation.reserve_burned,
      out_growth: m.growth.reserve_spent,
      out_funding: m.funding.reserve_debit,
      out_total: reserveOut,
      closure_residual: m.open_stocks.reserve + intake.total + m.funding.refunded_reserve
        - reserveOut - finalStocks.reserve,
    },
    energy: {
      opening: m.open_stocks.energy,
      in_digestion: m.digestion.energy_gain,
      in_field: m.frugivory.energy_gain + m.grazing.energy_gain + m.scavenging.energy_gain,
      in_oxidation: m.oxidation.energy_gained,
      in_total: m.digestion.energy_gain + m.frugivory.energy_gain + m.grazing.energy_gain
        + m.scavenging.energy_gain + m.oxidation.energy_gained,
      out_upkeep_paid: m.upkeep.paid,
      out_strike_paid: m.strike.paid,
      out_handling_paid: m.handling.paid,
      out_growth: m.growth.energy_cost,
      out_funding: m.funding.energy_debit,
      out_total: m.upkeep.paid + m.strike.paid + m.handling.paid + m.growth.energy_cost
        + m.funding.energy_debit,
    },
    // Demand and payment are reported side by side and never merged. The three demanded
    // terms describe the core's own expression; the single clamped debit is what it paid.
    upkeep: {
      ticks: m.upkeep.ticks,
      demanded_maintenance: m.upkeep.demanded_maintenance,
      demanded_move: m.upkeep.demanded_move,
      demanded_sense: m.upkeep.demanded_sense,
      demanded_total: m.upkeep.demanded_total,
      paid_lump: m.upkeep.paid,
      shortfall: m.upkeep.shortfall,
      shortfall_ticks: m.upkeep.shortfall_ticks,
      per_tick_maintenance: ratio(m.upkeep.demanded_maintenance, m.upkeep.ticks),
      per_tick_sense: ratio(m.upkeep.demanded_sense, m.upkeep.ticks),
      note: 'the paid lump is not divided across the demanded terms',
    },
    strike: m.strike,
    handling: m.handling,
    digestion_ticks: m.digestion.ticks,
    scavenging_ticks: m.scavenging.ticks,
    // A classification of the recorded path only. It is not a counterfactual, and it does
    // not exclude adult or environment effects of a different threshold.
    oxidation_recorded: {
      ticks: m.oxidation.ticks,
      reserve_burned: m.oxidation.reserve_burned,
      energy_gained: m.oxidation.energy_gained,
      ticks_recorded_above_reference: m.oxidation.above_reference_ticks,
      reserve_burned_above_reference: m.oxidation.above_reference_reserve_burned,
      share_of_burn_recorded_above_reference: ratio(m.oxidation.above_reference_reserve_burned,
        m.oxidation.reserve_burned),
    },
  };
}

/// Cross-check against the earlier census/transaction reduction, in both directions: no ledger
/// child may be absent from the census, and no census child from the ledgers.
///
/// The two records label the death boundary differently, and that is a property of the
/// instrument. The core emits its death event at `now + 1`, and the ledger's `born_tick` uses
/// the same `now + 1`; but `record_death` and the end-of-tick probe use the pre-increment
/// `now`, so a ledger `end_tick` is exactly one below the event-stream death tick. Nothing
/// measured depends on the label, and the convention-free check below proves it: the ledger's
/// gate observation count must equal the census reduction's birth-to-death span.
export const DEATH_TICK_OFFSET = 1;

export function crossCheck(children, censusChildren) {
  const rows = [];
  const seenCensus = new Set();
  for (const c of children) {
    const other = censusChildren.find(x => childKey(x.seed, x.arm, x.id) === c.key);
    if (!other) {rows.push({child: c.key, agreed: false, why: 'absent from the census reduction'}); continue;}
    seenCensus.add(c.key);
    const attempts = sum(Object.values(other.attempts));
    // An `Unaffordable` attempt is one the member could not pay the strike cost for, so it
    // never reaches the charge site the ledger records. Comparing total attempts against paid
    // strikes would report that as a disagreement; the payable subset is the like-for-like
    // quantity, and the unaffordable count is carried through as its own observation.
    const unaffordable = other.attempts.Unaffordable || 0;
    const payable = attempts - unaffordable;
    const mismatches = [];
    if (other.birth_tick !== c.birth_tick) mismatches.push('birth_tick');
    if (other.censored_alive !== c.censored_alive) mismatches.push('censored_alive');
    if ((other.death_cause ?? null) !== (c.end_cause ?? null)) mismatches.push('death_cause');
    if (c.end_tick !== null && other.death_tick !== c.end_tick + DEATH_TICK_OFFSET)
      mismatches.push(`death boundary ${other.death_tick} vs ledger ${c.end_tick} + ${DEATH_TICK_OFFSET}`);
    const censusSpan = other.death_tick === null ? null : other.death_tick - other.birth_tick;
    if (censusSpan !== null && censusSpan !== c.gate.observed_ticks)
      mismatches.push(`lifespan ${censusSpan} vs ${c.gate.observed_ticks} gate observations`);
    if (payable !== c.strike.ticks)
      mismatches.push(`payable attempts ${payable} vs paid strikes ${c.strike.ticks}`);
    if (Math.abs(other.paid_strike_energy - c.strike.paid) > 1e-12) mismatches.push('paid strike energy');
    rows.push({child: c.key, agreed: mismatches.length === 0,
      why: mismatches.length ? mismatches.join(', ') : null,
      census_attempts: attempts, census_unaffordable_attempts: unaffordable,
      census_payable_attempts: payable, census_captures: other.captures,
      census_lifespan_ticks: censusSpan,
      ledger_gate_observation_ticks: c.gate.observed_ticks,
      ledger_paid_strikes: c.strike.ticks, ledger_digestion_ticks: c.digestion_ticks});
  }
  const unmatchedCensus = censusChildren
    .map(x => childKey(x.seed, x.arm, x.id))
    .filter(k => !seenCensus.has(k));
  for (const key of unmatchedCensus)
    rows.push({child: key, agreed: false, why: 'present in the census reduction, absent from the ledgers'});
  return rows;
}

export function cohortTotals(children) {
  const t = f => sum(children.map(f));
  const bySource = {};
  for (const s of INTAKE_SOURCES) bySource[s] = t(c => c.reserve.in_by_source[s]);
  return {
    children: children.length,
    censored_alive: children.filter(c => c.censored_alive).length,
    ended_by_cause: children.reduce((o, c) => {
      const k = c.end_cause ?? 'censored_alive'; o[k] = (o[k] || 0) + 1; return o;
    }, {}),
    // Total intake across all four channels, and digestion alone, counted separately. An
    // earlier version computed this name from digestion, which undercounted acquisition by
    // ignoring scavenging: seed-5 `108:1` never struck or digested and still took in reserve.
    children_with_zero_reserve_intake: children.filter(c => c.reserve.in_total === 0).length,
    children_with_zero_digestion: children.filter(c => c.reserve.in_by_source.digestion === 0).length,
    children_that_scavenged: children.filter(c => c.reserve.in_by_source.scavenging > 0).length,
    children_with_scavenging_but_no_digestion: children.filter(c =>
      c.reserve.in_by_source.digestion === 0 && c.reserve.in_by_source.scavenging > 0).length,
    children_that_entered_growth: children.filter(c => c.growth.branch_entered_ticks > 0).length,
    children_that_gained_structure: children.filter(c => c.growth.structure_gained > 0).length,
    children_ever_adult: children.filter(c => c.adult_ticks > 0).length,
    children_whose_reserve_ever_cleared_gate: children.filter(c => c.gate.reserve_above_min_ticks > 0).length,
    children_whose_reserve_exceeded_birth_escrow: children.filter(c => c.gate.max_reserve_above_birth_escrow).length,
    total_gate_observation_ticks: t(c => c.gate.observed_ticks),
    total_reconciliation_checks: t(c => c.boundaries.reconciliation_checks),
    total_structure_gained: t(c => c.growth.structure_gained),
    smallest_reserve_deficit: Math.min(...children.map(c => c.gate.min_reserve_deficit ?? Infinity)),
    largest_reserve_seen: Math.max(...children.map(c => c.gate.max_reserve)),
    reserve_in_by_source: bySource,
    reserve_in_total: t(c => c.reserve.in_total),
    reserve_out_oxidation: t(c => c.reserve.out_oxidation),
    reserve_out_growth: t(c => c.reserve.out_growth),
    reserve_out_funding: t(c => c.reserve.out_funding),
    energy_out_upkeep_paid: t(c => c.energy.out_upkeep_paid),
    energy_out_strike_paid: t(c => c.energy.out_strike_paid),
    energy_out_handling_paid: t(c => c.energy.out_handling_paid),
    energy_in_oxidation: t(c => c.energy.in_oxidation),
    energy_in_digestion: t(c => c.energy.in_digestion),
    energy_in_field: t(c => c.energy.in_field),
    upkeep_demanded_sense: t(c => c.upkeep.demanded_sense),
    upkeep_demanded_maintenance: t(c => c.upkeep.demanded_maintenance),
    upkeep_demanded_move: t(c => c.upkeep.demanded_move),
    upkeep_paid_lump: t(c => c.upkeep.paid_lump),
    upkeep_shortfall_ticks: t(c => c.upkeep.shortfall_ticks),
    oxidation_ticks: t(c => c.oxidation_recorded.ticks),
    oxidation_ticks_recorded_above_reference: t(c => c.oxidation_recorded.ticks_recorded_above_reference),
    oxidation_burn: t(c => c.oxidation_recorded.reserve_burned),
    oxidation_burn_recorded_above_reference: t(c => c.oxidation_recorded.reserve_burned_above_reference),
    worst_reserve_closure_residual: Math.max(...children.map(c => Math.abs(c.reserve.closure_residual))),
    // Bookkeeping only, under the explicit condition that intake and its timing are unchanged
    // and no other flow responds. Reserve has one inflow set and, below the gate, one outflow,
    // so `escrow + recorded intake` is a ceiling on what the recorded intake could have
    // supported. It is NOT a counterfactual: removing a sink changes energy, lifespan,
    // movement, strike affordability and encounters, any of which changes intake itself.
    fixed_intake_ceiling: {
      condition: 'holds each child\'s recorded intake and its timing fixed and lets no other flow respond',
      children_whose_escrow_plus_recorded_intake_exceeds_the_gate:
        children.filter(c => c.reserve.opening + c.reserve.in_total > c.gate.gate_reserve).length,
      children_at_or_below_the_gate:
        children.filter(c => c.reserve.opening + c.reserve.in_total <= c.gate.gate_reserve).length,
      not_a_causal_partition: true,
    },
  };
}

async function sha256File(path) {
  return createHash('sha256').update(await readFile(path)).digest('hex');
}

/// Read and independently verify the retained snapshots for one arm.
///
/// An unreadable snapshot is reported as its own failure rather than folded into a generic
/// source error: without the bytes there is no independent identity evidence at all, and that
/// is a missing-evidence condition, not a smaller check.
export async function verifyArtifacts(armDir) {
  const out = {};
  for (const [name, file] of [['opening', 'post-initialization.cubw'], ['closing', 'closing.cubw']]) {
    const path = join(armDir, file);
    let bytes;
    try {
      bytes = await readFile(path);
    } catch (error) {
      out[name] = {unreadable: path, error: error.message};
      continue;
    }
    try {
      out[name] = inspectSnapshot(bytes, SNAPSHOT_SCHEMA);
    } catch (error) {
      out[name] = {error: error.message, sha256: createHash('sha256').update(bytes).digest('hex')};
    }
  }
  return out;
}

export async function reduce({ledgers, cohort, censusReduction, binary, binarySha256}) {
  const cohortRoot = resolve(cohort);
  const pinned = {path: resolve(binary), expected_sha256: binarySha256,
    actual_sha256: await sha256File(binary)};
  pinned.verified = pinned.actual_sha256 === pinned.expected_sha256;
  assert(pinned.verified, `the diagnostic binary is not the pinned one (${pinned.actual_sha256})`);

  const paths = [];
  for (const dir of ledgers)
    for (const name of (await readdir(dir)).sort())
      if (name.endsWith('.json')) paths.push(join(resolve(dir), name));

  const census = await json(resolve(censusReduction));
  const censusChildren = census.juvenile_evidence.children_detail;

  const arms = [], invalid = [], seenArms = new Map();
  for (const path of paths) {
    let report;
    try {
      report = await json(path);
    } catch (error) {
      invalid.push({ledger: path, failures: [{gate: 'malformed', detail: error.message}]});
      continue;
    }
    if (report.kind !== 'juvenile-mutation-site-flow-diagnostic') {
      invalid.push({ledger: path, failures: [{gate: 'malformed',
        detail: `kind is ${report.kind}, not a flow report`}]});
      continue;
    }
    const seed = report.opening?.seed, arm = report.opening?.arm, key = armKey(seed, arm);
    const failures = [];

    if (!EXPECTED_ARMS.some(a => a.seed === seed && a.arm === arm))
      failures.push({gate: 'cohort_membership', detail: `${key} is not in the frozen cohort`});
    if (seenArms.has(key))
      failures.push({gate: 'cohort_membership', detail: `${key} is a duplicate of ${seenArms.get(key)}`});
    seenArms.set(key, path);

    const armDir = join(cohortRoot, `seed-${seed}`, arm);
    let opening, summary, actual;
    try {
      [opening, summary, actual] = await Promise.all([
        json(join(armDir, 'opening.json')), json(join(armDir, 'summary.json')), verifyArtifacts(armDir)]);
    } catch (error) {
      invalid.push({seed, arm, ledger: path,
        failures: [...failures, {gate: 'source_cohort', detail: error.message}]});
      continue;
    }
    for (const which of ['opening', 'closing'])
      if (actual[which].unreadable)
        failures.push({gate: 'snapshot_unreadable',
          detail: `${which} snapshot ${actual[which].unreadable} could not be read, so no independent identity evidence exists for this arm: ${actual[which].error}`});
      else if (actual[which].error)
        failures.push({gate: 'snapshot_envelope', detail: `${which}: ${actual[which].error}`});

    // The ledger's own replayed path must name the seed and arm it claims. Matched on the
    // arm-relative suffix rather than an absolute prefix, so a relocated or copied artifact
    // tree is still checked for internal consistency.
    const suffix = `seed-${seed}/${arm}`;
    if (typeof report.arm !== 'string' || !report.arm.replace(/\/+$/, '').endsWith(suffix))
      failures.push({gate: 'source_cohort',
        detail: `the ledger replayed ${report.arm}, which does not end with ${suffix}`});

    failures.push(...checkArtifactIdentity(report, opening, summary, actual));
    failures.push(...checkGates(report, opening, summary)
      .map(f => ({...f, detail: `${key} ${f.detail}`})));

    const members = report.ledger?.members ?? [];
    const kids = members.filter(m => m.origin === 'Descendant');
    if (kids.length !== summary.offspring)
      failures.push({gate: 'coverage',
        detail: `${kids.length} descendants recorded, summary says ${summary.offspring}`});
    const localKeys = new Set();
    for (const m of kids) {
      const k = childKey(seed, arm, m.id);
      if (localKeys.has(k)) failures.push({gate: 'duplicate_child', detail: k});
      localKeys.add(k);
    }

    const record = {
      seed, arm, ledger: path, ledger_sha256: await sha256File(path),
      valid: failures.length === 0,
      failures,
      independently_verified: {
        basis: 'the retained .cubw files were read and their envelope, CRC32, FNV state hash and SHA256 recomputed from the bytes',
        opening: actual.opening, closing: actual.closing,
      },
      reported_by_replay: {
        basis: 'observations only the running replay could make; validated for shape and against the frozen tolerance, not re-derived',
        observer_neutrality: report.observer_neutrality,
        reconciliation: report.reconciliation,
      },
      members: members.map(m => ({
        id: idKey(m.id), origin: m.origin, end_tick: m.end_tick, end_cause: m.end_cause,
        ...boundaryAccounting(m),
        worst_residual: Math.max(m.residual.max_structure, m.residual.max_reserve, m.residual.max_energy),
        growth_steps: m.growth.ticks,
        reserve_in_by_source: intakeBySource(m).by_source,
        oxidation_ticks: m.oxidation.ticks,
        oxidation_ticks_recorded_above_reference: m.oxidation.above_reference_ticks,
        share_of_burn_recorded_above_reference: ratio(m.oxidation.above_reference_reserve_burned,
          m.oxidation.reserve_burned),
      })),
      _children: kids.map(m => memberRecord(seed, arm, m)),
      _adults: members.filter(m => m.origin === 'Founder').map(m => memberRecord(seed, arm, m)),
    };
    (record.valid ? arms : invalid).push(record);
  }

  const missing = EXPECTED_ARMS
    .filter(a => !seenArms.has(armKey(a.seed, a.arm)))
    .map(a => ({seed: a.seed, arm: a.arm, failures: [{gate: 'cohort_membership', detail: 'no ledger found'}]}));
  invalid.push(...missing);

  // Statistics come only from arms that passed everything.
  const children = arms.flatMap(a => a._children)
    .sort((a, b) => a.seed - b.seed || a.arm.localeCompare(b.arm) || a.id.localeCompare(b.id));
  const adults = arms.flatMap(a => a._adults)
    .sort((a, b) => a.seed - b.seed || a.arm.localeCompare(b.arm) || a.id.localeCompare(b.id));
  for (const a of arms) {delete a._children; delete a._adults;}
  for (const a of invalid) {delete a._children; delete a._adults;}

  const complete = invalid.length === 0 && arms.length === EXPECTED_ARMS.length
    && children.length === EXPECTED_CHILDREN && children.length + adults.length === EXPECTED_MEMBERS;
  const cross = complete ? crossCheck(children, censusChildren) : [];
  const crossDisagreements = cross.filter(r => !r.agreed);
  const boundaryFailures = [...children, ...adults].filter(m => !m.boundaries.identity_holds).map(m => m.key);

  return {
    kind: 'juvenile-flow-cohort-reduction',
    complete,
    arms_reduced: arms.length,
    cohort_basis: {
      expected_arms: EXPECTED_ARMS.length, valid_arms: arms.length, invalid_arms: invalid.length,
      expected_children: EXPECTED_CHILDREN, children_reduced: children.length,
      expected_members: EXPECTED_MEMBERS, members_reduced: children.length + adults.length,
      statistics_include_only_valid_arms: true,
      note: complete ? 'every expected arm passed every check'
        : 'INCOMPLETE — the totals below cover only the valid arms and are not a cohort result',
    },
    pinned_binary: pinned,
    sources: {ledger_dirs: ledgers.map(d => resolve(d)), retained_cohort: cohortRoot,
      census_reduction: resolve(censusReduction)},
    invalid_arms: invalid,
    boundary_identity_failures: boundaryFailures,
    cross_check_vs_census_reduction: {
      ran: complete,
      children_compared: cross.length,
      census_children_available: censusChildren.length,
      disagreements: crossDisagreements,
      unaffordable_attempts_total: sum(cross.map(r => r.census_unaffordable_attempts ?? 0)),
      children_with_unaffordable_attempts: cross.filter(r => (r.census_unaffordable_attempts ?? 0) > 0)
        .map(r => ({child: r.child, unaffordable: r.census_unaffordable_attempts,
          of_attempts: r.census_attempts})),
      rows: cross,
      basis: 'both directions: no ledger child may be absent from the census reduction, and no census child from the ledgers. The census path reads events and 200-tick boundaries; the ledger path reads mutation sites during a replay.',
    },
    cohort: cohortTotals(children),
    adult_reserve_in_by_source: adults.reduce((o, a) => {
      for (const s of INTAKE_SOURCES) o[s] = (o[s] ?? 0) + a.reserve.in_by_source[s];
      return o;
    }, {}),
    adults_that_scavenged: adults.filter(a => a.reserve.in_by_source.scavenging > 0).length,
    arms, children, adults,
    limits: [
      'Every quantity is a recorded flow or identity on the realized path of a retained arm.',
      'Snapshot identity is verified from the retained bytes; observer neutrality and flow/stock reconciliation are the replay\'s own observations, validated for shape and against this file\'s frozen tolerance rather than re-derived.',
      'The above-reference oxidation counts classify recorded states. They are not a counterfactual: a different threshold can change the adult, its funding and birth timing, prey interactions and the juvenile\'s own later stocks.',
      'The fixed-intake ceiling holds each child\'s recorded intake and timing fixed and lets no other flow respond. Removing a sink would change energy, lifespan, movement, strike affordability and encounters, so it is bookkeeping, not a causal partition of the cohort.',
      'The clamped upkeep payment is never divided across its three demanded terms.',
      'Gate observation ticks and reconciliation checks are separate counts; a mid-replay birth adds one check at its birth boundary and a death replaces that tick\'s probe with a removal-site check.',
      'Reconciliation proves completeness only for stock movements a replay actually exercised. The growth branch did not run, so its cap-attribution counters remain unit-test evidence.',
      'Encounter and approach are not measured here.',
    ].join(' '),
  };
}

/// Every condition that must hold for the output to be a cohort result. The CLI exits nonzero
/// on any of them, so a partial or internally inconsistent set can never pass as one.
export function failureSummary(result) {
  const reasons = [];
  if (!result.complete) reasons.push('the frozen cohort is not completely and validly covered');
  if (result.invalid_arms.length) reasons.push(`${result.invalid_arms.length} invalid arm(s)`);
  if (result.boundary_identity_failures.length)
    reasons.push(`${result.boundary_identity_failures.length} boundary identity failure(s)`);
  if (!result.cross_check_vs_census_reduction.ran) reasons.push('the cross-check did not run');
  else if (result.cross_check_vs_census_reduction.disagreements.length)
    reasons.push(`${result.cross_check_vs_census_reduction.disagreements.length} cross-check disagreement(s)`);
  else if (result.cross_check_vs_census_reduction.children_compared !== EXPECTED_CHILDREN)
    reasons.push(`cross-checked ${result.cross_check_vs_census_reduction.children_compared} children, expected ${EXPECTED_CHILDREN}`);
  return reasons;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    const argv = process.argv.slice(2);
    const flag = name => {
      const i = argv.indexOf(name);
      assert(i >= 0 && argv[i + 1], `missing ${name}`);
      return argv[i + 1];
    };
    const ledgers = argv.flatMap((a, i) => argv[i - 1] === '--ledgers' ? [a] : []);
    assert(ledgers.length >= 1, 'at least one --ledgers DIR');
    const result = await reduce({
      ledgers, cohort: flag('--cohort'), censusReduction: flag('--census'),
      binary: flag('--binary'), binarySha256: flag('--binary-sha256'),
    });
    console.log(JSON.stringify(result, null, 1));
    const reasons = failureSummary(result);
    if (reasons.length) {
      console.error(`Reduction is not a cohort result: ${reasons.join('; ')}`);
      process.exitCode = 1;
    }
  } catch (error) {console.error(`Reduction refused: ${error.message}`); process.exitCode = 1;}
}
