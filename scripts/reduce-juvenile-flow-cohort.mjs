// Read-only aggregation of the pinned juvenile-flow diagnostic replays.
//
// Each replay already refuses to emit anything unless its own four gates pass. This reducer
// does not take that on trust: it re-checks every gate against the retained artifacts the
// replay was supposed to reproduce (`opening.json`, `summary.json`), so an arm that somehow
// reported a pass while disagreeing with the cohort it came from is caught here rather than
// pooled. A failing arm is retained in the output with its failure named; nothing is dropped.
//
// It also cross-checks the flow ledgers against the earlier census/transaction reduction
// (`assets/hunter-charge-rerun-reduction-2026-09-13.json`). The two measurement paths are
// independent — one reads events and 200-tick census boundaries, the other reads mutation
// sites during a replay — so agreement on each child's identity, attempts and death is
// evidence, and disagreement would be a finding.
//
// Deliberately NOT done here, per the independent review:
//   * no counterfactual is inferred. `above_reference` is a classification of the recorded
//     path, not a claim about what a different threshold would have produced;
//   * the clamped upkeep payment is never split across its three demanded terms;
//   * gate observation ticks and reconciliation checks are reported as separate quantities.
import assert from 'node:assert/strict';
import {readFile, readdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve, join} from 'node:path';
import {pathToFileURL} from 'node:url';

const json = async path => JSON.parse(await readFile(path, 'utf8'));
const sum = xs => xs.reduce((a, b) => a + b, 0);
const ratio = (n, d) => d === 0 ? null : n / d;
export const idKey = id => `${id.slot}:${id.generation}`;

/// The four gates, re-derived from the retained artifacts rather than read off the report.
///
/// `--ticks` prefix runs are rejected outright: a prefix cannot satisfy gate 2, and a cohort
/// aggregate must not silently mix one in with full-horizon arms.
export function checkGates(report, opening, summary, plannedTicks = 144000) {
  const failures = [];
  const require_ = (name, ok, detail) => {if (!ok) failures.push({gate: name, detail});};

  require_('horizon', report.replayed_ticks === plannedTicks,
    `replayed ${report.replayed_ticks} ticks, not the recorded ${plannedTicks}`);

  // 1. input identity, against opening.json itself
  require_('input_identity', report.opening.snapshot_sha256 === opening.post_snapshot_sha256,
    'opening sha256 differs from the retained opening.json');
  require_('input_identity', report.opening.state_hash === opening.post_state_hash,
    'opening state hash differs from the retained opening.json');

  // 2. output identity, against summary.json itself
  const c = report.closing_identity;
  require_('output_identity', c.checked === true, 'closing identity was not asserted');
  require_('output_identity', c.closing_snapshot_sha256 === summary.closing_snapshot_sha256,
    'replayed closing sha256 differs from the retained summary');
  require_('output_identity', c.closing_state_hash === summary.closing_state_hash,
    'replayed closing state hash differs from the retained summary');

  // 3. observer neutrality
  const n = report.observer_neutrality;
  require_('observer_neutrality', n.checked === true && n.closing_bytes_equal === true
    && n.state_hash_equal === true, 'the two replay modes disagreed');
  require_('observer_neutrality', Number.isSafeInteger(n.event_ticks_compared) && n.event_ticks_compared > 0,
    'no event records were compared');

  // 4. reconciliation
  require_('reconciliation', report.reconciliation.violations === 0,
    `${report.reconciliation.violations} flow/stock violations`);
  require_('reconciliation', report.ledger.unregistered_records === 0,
    `${report.ledger.unregistered_records} records for unregistered members`);

  // The arm is the one it claims to be.
  require_('provenance', report.opening.arm === opening.arm, 'arm label mismatch');
  require_('provenance', report.opening.profile_recipe === opening.profile_recipe, 'recipe mismatch');
  return failures;
}

/// Gate observations and reconciliation checks are different counts and are kept apart.
///
/// A member registered before the replay starts is gate-observed and probed on the same ticks,
/// and its death tick swaps the missing end-of-tick probe for one removal-site check — so the
/// two totals match. A member born mid-replay is registered during the birth tick's commit,
/// after the physiology pass that evaluates the gate, so it gains exactly one extra check at
/// its birth boundary. This function asserts that identity rather than assuming it.
export function boundaryAccounting(member) {
  const gateTicks = member.gate.observed_ticks;
  const checks = member.residual.checked_ticks;
  const bornMidReplay = member.origin === 'Descendant' && member.parent !== null;
  const died = member.end_tick !== null;
  const expected = gateTicks + (bornMidReplay ? 1 : 0);
  return {
    gate_observation_ticks: gateTicks,
    reconciliation_checks: checks,
    newborn_boundary_checks: bornMidReplay ? 1 : 0,
    removal_boundary_checks: died ? 1 : 0,
    checks_minus_gate_ticks: checks - gateTicks,
    identity_holds: checks === expected,
    basis: 'a mid-replay birth is registered after the physiology pass, so its birth tick is probed but not gate-observed; a death replaces that tick\'s probe with one removal-site check',
  };
}

/// One paid descendant, reduced to what the cohort report cites. Every field is a recorded
/// flow or an identity; nothing is apportioned.
export function childRecord(seed, arm, m, derived) {
  const intake = {
    digestion: m.digestion.to_reserve,
    frugivory: m.frugivory.to_reserve,
    grazing: m.grazing.to_reserve,
    scavenging: m.scavenging.to_reserve,
  };
  return {
    seed, arm, id: idKey(m.id), parent: m.parent ? idKey(m.parent) : null,
    birth_tick: m.born_tick,
    end_tick: m.end_tick,
    end_cause: m.end_cause,
    censored_alive: m.end_tick === null,
    observed_seconds: m.gate.observed_ticks * 0.05,
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
      mean_reserve: ratio(m.gate.reserve_sum, m.gate.observed_ticks),
      max_energy: m.gate.max_energy,
      mean_energy: ratio(m.gate.energy_sum, m.gate.observed_ticks),
      reserve_zero_ticks: m.gate.reserve_zero_ticks,
    },
    reserve: {
      opening: m.open_stocks.reserve,
      in_by_source: intake,
      in_total: sum(Object.values(intake)),
      out_oxidation: m.oxidation.reserve_burned,
      out_growth: m.growth.reserve_spent,
      out_funding: m.funding.reserve_debit,
      out_total: derived.reserve_out_total,
      closure_residual: m.open_stocks.reserve + sum(Object.values(intake)) + m.funding.refunded_reserve
        - m.oxidation.reserve_burned - m.growth.reserve_spent - m.funding.reserve_debit
        - (m.death_stocks ? m.death_stocks.reserve : m.close_stocks.reserve),
    },
    energy: {
      opening: m.open_stocks.energy,
      in_digestion: m.digestion.energy_gain,
      in_field: m.frugivory.energy_gain + m.grazing.energy_gain + m.scavenging.energy_gain,
      in_oxidation: m.oxidation.energy_gained,
      in_total: derived.energy_in_total,
      out_upkeep_paid: m.upkeep.paid,
      out_strike_paid: m.strike.paid,
      out_handling_paid: m.handling.paid,
      out_growth: m.growth.energy_cost,
      out_funding: m.funding.energy_debit,
      out_total: derived.energy_out_total,
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

/// The two records label the death boundary differently, and that is a property of the
/// instrument, not a disagreement about when the member died.
///
/// The core emits its `life`/`hunter` death event at `now + 1`, and the ledger's own
/// `born_tick` uses the same `now + 1`; but `record_death` and the end-of-tick probe are
/// called with the pre-increment `now`, so a ledger `end_tick` is exactly one below the
/// event-stream death tick. Nothing measured depends on the label — every duration here is a
/// count of observations — and the convention-free check below proves it: the ledger's own
/// gate observation count equals the census reduction's birth-to-death span exactly.
export const DEATH_TICK_OFFSET = 1;

/// Cross-check one arm's ledger against the earlier census/transaction reduction. The two
/// paths measure different things from different inputs; agreement is worth recording.
export function crossCheck(children, censusChildren) {
  const rows = [];
  for (const c of children) {
    const other = censusChildren.find(x => x.seed === c.seed && x.arm === c.arm && x.id === c.id);
    if (!other) {rows.push({child: `${c.seed}/${c.arm}/${c.id}`, agreed: false, why: 'absent from the census reduction'}); continue;}
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
    // Convention-free: the ledger counted one gate observation per live tick, so its count
    // must equal the span the event stream reports, whatever either end is labelled.
    const censusSpan = other.death_tick === null ? null : other.death_tick - other.birth_tick;
    if (censusSpan !== null && censusSpan !== c.gate.observed_ticks)
      mismatches.push(`lifespan ${censusSpan} vs ${c.gate.observed_ticks} gate observations`);
    if (payable !== c.strike.ticks)
      mismatches.push(`payable attempts ${payable} vs paid strikes ${c.strike.ticks}`);
    if (Math.abs(other.paid_strike_energy - c.strike.paid) > 1e-12) mismatches.push('paid strike energy');
    rows.push({child: `${c.seed}/${c.arm}/${c.id}`, agreed: mismatches.length === 0,
      why: mismatches.length ? mismatches.join(', ') : null,
      census_attempts: attempts, census_unaffordable_attempts: unaffordable,
      census_payable_attempts: payable, census_captures: other.captures,
      census_lifespan_ticks: censusSpan,
      ledger_gate_observation_ticks: c.gate.observed_ticks,
      ledger_paid_strikes: c.strike.ticks, ledger_digestion_ticks: c.digestion_ticks});
  }
  return rows;
}

export function cohortTotals(children) {
  const t = (f) => sum(children.map(f));
  const withCaptures = children.filter(c => c.reserve.in_by_source.digestion > 0);
  return {
    children: children.length,
    censored_alive: children.filter(c => c.censored_alive).length,
    ended_by_cause: children.reduce((o, c) => {
      const k = c.end_cause ?? 'censored_alive'; o[k] = (o[k] || 0) + 1; return o;
    }, {}),
    children_with_zero_reserve_intake: children.length - withCaptures.length,
    children_that_entered_growth: children.filter(c => c.growth.branch_entered_ticks > 0).length,
    children_that_gained_structure: children.filter(c => c.growth.structure_gained > 0).length,
    children_ever_adult: children.filter(c => c.adult_ticks > 0).length,
    children_whose_reserve_ever_cleared_gate: children.filter(c => c.gate.reserve_above_min_ticks > 0).length,
    total_gate_observation_ticks: t(c => c.gate.observed_ticks),
    total_reconciliation_checks: t(c => c.boundaries.reconciliation_checks),
    total_structure_gained: t(c => c.growth.structure_gained),
    smallest_reserve_deficit: Math.min(...children.map(c => c.gate.min_reserve_deficit ?? Infinity)),
    largest_reserve_seen: Math.max(...children.map(c => c.gate.max_reserve)),
    reserve_in_total: t(c => c.reserve.in_total),
    reserve_out_oxidation: t(c => c.reserve.out_oxidation),
    reserve_out_growth: t(c => c.reserve.out_growth),
    reserve_out_funding: t(c => c.reserve.out_funding),
    energy_out_upkeep_paid: t(c => c.energy.out_upkeep_paid),
    energy_out_strike_paid: t(c => c.energy.out_strike_paid),
    energy_out_handling_paid: t(c => c.energy.out_handling_paid),
    energy_in_oxidation: t(c => c.energy.in_oxidation),
    energy_in_digestion: t(c => c.energy.in_digestion),
    upkeep_demanded_sense: t(c => c.upkeep.demanded_sense),
    upkeep_demanded_maintenance: t(c => c.upkeep.demanded_maintenance),
    upkeep_demanded_move: t(c => c.upkeep.demanded_move),
    upkeep_shortfall_ticks: t(c => c.upkeep.shortfall_ticks),
    oxidation_ticks: t(c => c.oxidation_recorded.ticks),
    oxidation_ticks_recorded_above_reference: t(c => c.oxidation_recorded.ticks_recorded_above_reference),
    oxidation_burn: t(c => c.oxidation_recorded.reserve_burned),
    oxidation_burn_recorded_above_reference: t(c => c.oxidation_recorded.reserve_burned_above_reference),
    worst_reserve_closure_residual: Math.max(...children.map(c => Math.abs(c.reserve.closure_residual))),
  };
}

async function sha256File(path) {
  return createHash('sha256').update(await readFile(path)).digest('hex');
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

  const arms = [], children = [], failures = [];
  for (const path of paths) {
    const report = await json(path);
    assert.equal(report.kind, 'juvenile-mutation-site-flow-diagnostic', `${path} is not a flow report`);
    const seed = report.opening.seed, arm = report.opening.arm;
    const armDir = join(cohortRoot, `seed-${seed}`, arm);
    const opening = await json(join(armDir, 'opening.json'));
    const summary = await json(join(armDir, 'summary.json'));
    const gateFailures = checkGates(report, opening, summary);
    if (gateFailures.length) failures.push({seed, arm, ledger: path, failures: gateFailures});

    const members = report.ledger.members;
    const kids = members.filter(m => m.origin === 'Descendant');
    // The ledger's descendant count must match the arm's own recorded offspring count, or the
    // cohort is not covering the children it claims to.
    if (kids.length !== summary.offspring)
      failures.push({seed, arm, ledger: path,
        failures: [{gate: 'coverage', detail: `${kids.length} descendants recorded, summary says ${summary.offspring}`}]});

    for (const m of kids) {
      const d = report.derived[idKey(m.id)];
      const rec = childRecord(seed, arm, m, d);
      rec.digestion_ticks = m.digestion.ticks;
      children.push(rec);
    }
    arms.push({
      seed, arm, ledger: path,
      // The raw ledgers live in the gitignored workspace, so the committed reduction pins
      // them by content: a later reader can tell whether the file on disk is this one.
      ledger_sha256: await sha256File(path),
      gates_passed: gateFailures.length === 0,
      gate_failures: gateFailures,
      identity: {
        opening_sha256: report.opening.snapshot_sha256, opening_state_hash: report.opening.state_hash,
        closing_sha256: report.closing_identity.closing_snapshot_sha256,
        closing_state_hash: report.closing_identity.closing_state_hash,
        event_ticks_compared: report.observer_neutrality.event_ticks_compared,
        event_stream_sha256: report.observer_neutrality.event_stream_sha256,
        reconciliation_violations: report.reconciliation.violations,
      },
      members: members.map(m => ({
        id: idKey(m.id), origin: m.origin, end_tick: m.end_tick, end_cause: m.end_cause,
        ...boundaryAccounting(m),
        worst_residual: Math.max(m.residual.max_structure, m.residual.max_reserve, m.residual.max_energy),
        growth_steps: m.growth.ticks,
        oxidation_ticks: m.oxidation.ticks,
        oxidation_ticks_recorded_above_reference: m.oxidation.above_reference_ticks,
        share_of_burn_recorded_above_reference: ratio(m.oxidation.above_reference_reserve_burned,
          m.oxidation.reserve_burned),
      })),
    });
  }
  children.sort((a, b) => a.seed - b.seed || a.arm.localeCompare(b.arm) || a.id.localeCompare(b.id));
  const cross = crossCheck(children, censusChildren);
  return {
    kind: 'juvenile-flow-cohort-reduction',
    pinned_binary: pinned,
    sources: {ledger_dirs: ledgers.map(d => resolve(d)), retained_cohort: cohortRoot,
      census_reduction: resolve(censusReduction)},
    arms_reduced: arms.length,
    arms_with_gate_failures: failures.length,
    gate_failures: failures,
    boundary_identity_holds: children.every(c => c.boundaries.identity_holds),
    cross_check_vs_census_reduction: {
      children_compared: cross.length,
      disagreements: cross.filter(r => !r.agreed),
      // Not a disagreement: an attempt the member could not pay for never reaches the charge
      // site, so it is counted by the event stream and absent from the ledger by construction.
      unaffordable_attempts_total: sum(cross.map(r => r.census_unaffordable_attempts ?? 0)),
      children_with_unaffordable_attempts: cross.filter(r => (r.census_unaffordable_attempts ?? 0) > 0)
        .map(r => ({child: r.child, unaffordable: r.census_unaffordable_attempts,
          of_attempts: r.census_attempts})),
      rows: cross,
      basis: 'the census/transaction reduction reads events and 200-tick boundaries; the ledger reads mutation sites during a replay. Independent paths, same identities.',
    },
    cohort: cohortTotals(children),
    arms, children,
    limits: [
      'Every quantity is a recorded flow or identity on the realized path of a retained arm.',
      'The above-reference oxidation counts classify recorded states. They are not a counterfactual: a different threshold can change the adult, its funding and birth timing, prey interactions and the juvenile\'s own later stocks, so nothing here shows what a background-policy juvenile would have done.',
      'The clamped upkeep payment is never divided across its three demanded terms.',
      'Gate observation ticks and reconciliation checks are separate counts; a mid-replay birth adds one check at its birth boundary and a death replaces that tick\'s probe with a removal-site check.',
      'Reconciliation proves completeness only for stock movements a replay actually exercised. The growth branch did not run, so its cap-attribution counters remain unit-test evidence.',
      'Encounter and approach are not measured here. Paid strikes and digestion ticks are counted; reachable-but-unstruck prey is not.',
    ].join(' '),
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
    const ledgers = argv.filter((a, i) => i > 0 && argv[i - 1] === '--ledgers');
    assert(ledgers.length >= 1, 'at least one --ledgers DIR');
    const result = await reduce({
      ledgers: argv.flatMap((a, i) => argv[i - 1] === '--ledgers' ? [a] : []),
      cohort: flag('--cohort'),
      censusReduction: flag('--census'),
      binary: flag('--binary'),
      binarySha256: flag('--binary-sha256'),
    });
    console.log(JSON.stringify(result, null, 1));
    if (result.arms_with_gate_failures > 0) process.exitCode = 1;
  } catch (error) {console.error(`Reduction refused: ${error.message}`); process.exitCode = 1;}
}
