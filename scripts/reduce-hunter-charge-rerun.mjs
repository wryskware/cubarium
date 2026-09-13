// Read-only independent reduction of the corrected twelve-seed paid-charging rerun.
//
// This is NOT a replacement for `compare-hunter-recipes.mjs`. That script owns the strict
// artifact gate and is imported unchanged: `compareCharge` must pass before anything here is
// reported, and none of its assertions are relaxed, wrapped or re-implemented more permissively.
//
// What this adds is a second, independent pass that recomputes the biology from the raw
// `events.jsonl` transaction records and the raw `census.jsonl` boundary stocks, and then
// reconciles that reconstruction against each arm's own `summary.json`. The two are kept
// distinct on purpose and are labelled distinctly in the output:
//
//   summary_*   the run's own tickwise audit (every tick, produced by the frozen executable)
//   sampled_*   this script's reduction of the 200-tick census cadence (720 boundaries/arm)
//   ledger_*    this script's reduction of the exact transaction records (every event)
//
// A sampled quantity is never presented as an actual stock-time, and an agreement between the
// two is reported as a reconciliation, not as independent confirmation of a value the summary
// is the only source for.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve, join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {ARMS, compareCharge} from './compare-hunter-recipes.mjs';

const json = async path => JSON.parse(await readFile(path, 'utf8'));
const jsonl = async path => (await readFile(path, 'utf8')).trim().split('\n').filter(Boolean).map(JSON.parse);
const bump = (o, k, n = 1) => {o[k] = (o[k] || 0) + n;};
const idKey = id => `${id.slot}:${id.generation}`;
const ratio = (n, d) => d === 0 ? null : n / d;

/// The frozen core's step, the same 20 Hz constant `compare-hunter-recipes.mjs` reasons with.
/// It is a property of the executable, not of the independently chosen census cadence.
export const DT = 0.05;

/// The exact funding contract every paid gestation must satisfy, restated here so the ledger is
/// checked against the documented transaction rather than against whatever the summary reports.
export const FUNDING = {reserve_debit: 1.6, energy_debit: 1.0, escrow_structure: 0.8,
  escrow_reserve: 0.8, escrow_energy: 0.6, build_heat: 0.4, birth_heat: 1.6};

/// Rebuild the reproduction ledger from the raw `hunter/reproduction` records alone.
///
/// Every record is keyed by its own funding key (parent + started_tick), so a funded gestation
/// is followed to exactly one terminal outcome — born, refunded or miscarried — or is left open
/// at the horizon. Unfunded refusals are kept with their reason; they are real observed
/// outcomes, not absences.
export function reduceReproductionLedger(rows) {
  const keys = new Map();
  const counts = {funded: 0, born: 0, refunded: 0, miscarried: 0, closed: 0, open_at_horizon: 0,
    not_funded_cap: 0, not_funded_stocks: 0};
  const energy = {funded: 0, born: 0, refunded: 0, miscarried: 0};
  const material = {funded: 0, born: 0, refunded: 0, miscarried: 0};
  const heat = {build: 0, birth: 0};
  const violations = [];
  for (const row of rows) {
    if (row.stream !== 'hunter' || row.event.kind !== 'reproduction') continue;
    const q = row.event.record, kind = q.transaction;
    const key = q.key ? `${idKey(q.key.parent)}@${q.key.started_tick}` : null;
    if (kind === 'not_funded') {
      // A refusal carries no key: it is a gate observation, counted by its stated reason.
      bump(counts, q.reason === 'cap' ? 'not_funded_cap' : 'not_funded_stocks');
      continue;
    }
    assert(key, `reproduction ${kind} without a funding key at tick ${row.event.tick}`);
    if (kind === 'funded') {
      assert(!keys.has(key), `funding key ${key} funded twice`);
      // The debit is read from the record's own before/after pair, not inferred from a total.
      const dr = q.parent_reserve_before - q.parent_reserve_after;
      const de = q.parent_energy_before - q.parent_energy_after;
      if (Math.abs(dr - FUNDING.reserve_debit) > 1e-12) violations.push({key, field: 'reserve_debit', got: dr});
      if (Math.abs(de - FUNDING.energy_debit) > 1e-12) violations.push({key, field: 'energy_debit', got: de});
      for (const [field, want] of [['escrow_structure', FUNDING.escrow_structure],
        ['escrow_reserve', FUNDING.escrow_reserve], ['escrow_energy', FUNDING.escrow_energy],
        ['build_heat', FUNDING.build_heat]])
        if (q[field] !== want) violations.push({key, field, got: q[field]});
      keys.set(key, {key, parent: idKey(q.key.parent), started_tick: q.key.started_tick,
        funded_tick: row.event.tick, outcome: 'open_at_horizon', outcome_tick: null});
      counts.funded++;
      // Escrowed structure is built from the parent's material; the energy escrow plus the
      // conversion heat is what the parent actually paid out of its battery.
      energy.funded += FUNDING.escrow_energy + FUNDING.build_heat;
      material.funded += FUNDING.escrow_structure + FUNDING.escrow_reserve;
      heat.build += q.build_heat;
      continue;
    }
    const open = keys.get(key);
    assert(open, `reproduction ${kind} for unknown funding key ${key}`);
    assert.equal(open.outcome, 'open_at_horizon', `funding key ${key} closed twice`);
    open.outcome = kind; open.outcome_tick = row.event.tick;
    counts[kind]++; counts.closed++;
    if (kind === 'born') {
      if (q.birth_heat !== FUNDING.birth_heat) violations.push({key, field: 'birth_heat', got: q.birth_heat});
      heat.birth += q.birth_heat;
      energy.born += FUNDING.escrow_energy + FUNDING.birth_heat;
      material.born += FUNDING.escrow_structure + FUNDING.escrow_reserve;
    }
    if (kind === 'refunded') {
      energy.refunded += q.refunded_energy ?? 0;
      material.refunded += (q.refunded_structure ?? 0) + (q.refunded_reserve ?? 0);
    }
    if (kind === 'miscarried') {
      energy.miscarried += q.miscarried_energy ?? 0;
      material.miscarried += (q.miscarried_structure ?? 0) + (q.miscarried_reserve ?? 0);
    }
  }
  counts.open_at_horizon = counts.funded - counts.closed;
  assert.equal(counts.closed, counts.born + counts.refunded + counts.miscarried, 'ledger closure');
  return {counts, energy, material, heat, gestations: [...keys.values()], violations};
}

/// Every hunter member's whole recorded life, from the transaction stream only.
///
/// `origin` separates the placed founders (which the harness gifts stocks to) from the paid
/// descendants; both are retained, including the ones that never attempt an attack.
export function reduceMembers(eventRows, openingTick, closingTick, placedFounderIds = []) {
  const members = new Map();
  const get = (id, origin, birth) => {
    const k = typeof id === 'string' ? id : idKey(id);
    if (!members.has(k)) members.set(k, {id: k, origin, parent: null, birth_tick: birth,
      death_tick: null, death_cause: null, attempts: {}, captures: 0, capture_material: 0,
      capture_energy: 0, paid_strike_energy: 0, meals: []});
    return members.get(k);
  };
  // A founder that never attacks and never dies emits no event at all; the opening census
  // boundary is the only record it exists, so it is registered before the streams are read.
  for (const id of placedFounderIds) get(id, 'placed_founder', openingTick);
  for (const row of eventRows) {
    if (row.stream !== 'hunter') continue;
    const e = row.event;
    if (e.kind === 'offspring') {
      const child = get(e.child, 'paid_descendant', e.tick);
      child.origin = 'paid_descendant'; child.parent = idKey(e.parent); child.birth_tick = e.tick;
    }
  }
  for (const row of eventRows) {
    if (row.stream !== 'hunter') continue;
    const e = row.event;
    switch (e.kind) {
      case 'attempt': {
        const m = get(e.hunter, 'placed_founder', openingTick);
        bump(m.attempts, e.outcome); m.paid_strike_energy += e.energy_paid; break;
      }
      case 'capture': {
        const m = get(e.hunter, 'placed_founder', openingTick);
        m.captures++; m.capture_material += e.material; m.capture_energy += e.energy;
        m.meals.push({tick: e.tick, material: e.material, energy: e.energy}); break;
      }
      case 'death': {
        const m = get(e.id, 'placed_founder', openingTick);
        m.death_tick = e.tick; m.death_cause = e.cause; break;
      }
      default: break;
    }
  }
  for (const m of members.values()) {
    m.censored_alive = m.death_tick === null;
    m.observed_ticks = (m.death_tick ?? closingTick) - m.birth_tick;
    m.observed_seconds = m.observed_ticks * DT;
  }
  return members;
}

/// The census cadence, reduced without pretending it is tickwise.
///
/// `sampled_adult_occupancy` counts census boundaries, not ticks; the summary's own
/// `adult_occupancy_ticks_0_1_2_over2` is the tickwise quantity and the two are reported side
/// by side rather than substituted for one another.
export function reduceCensus(censusRows, cfg, adultTolerance = 1e-9) {
  const sampled = {boundaries: 0, adult_occupancy: [0, 0, 0, 0], cadence_ticks: null,
    first_tick: null, last_tick: null};
  const perMember = new Map();
  const growthGate = cfg.growth_reserve_min;
  let previousTick = null;
  for (const row of censusRows) {
    sampled.boundaries++;
    if (previousTick !== null) {
      const gap = row.tick - previousTick;
      if (sampled.cadence_ticks === null) sampled.cadence_ticks = gap;
      else if (sampled.cadence_ticks !== gap) sampled.cadence_ticks = 'irregular';
    }
    previousTick = row.tick;
    sampled.first_tick ??= row.tick;
    sampled.last_tick = row.tick;
    let adults = 0;
    for (const h of row.hunter_stocks) {
      if (h.structure + adultTolerance >= h.adult_structure) adults++;
      const k = idKey(h.id);
      if (!perMember.has(k)) perMember.set(k, {id: k, samples: 0, first_tick: row.tick, last_tick: row.tick,
        structure_values: new Set(), min_structure: h.structure, max_structure: h.structure,
        max_reserve: h.reserve, max_energy: h.energy, max_gut_material: h.gut_material,
        max_gut_energy: h.gut_energy, reserve_max: h.reserve_max, energy_max: h.energy_max,
        adult_structure: h.adult_structure, reserve_zero_samples: 0,
        samples_above_growth_gate: 0, samples_above_world_oxidation_shutoff: 0,
        samples_above_member_oxidation_shutoff: 0, ever_adult: false, phases: {}, blockers: {},
        structure_decreased: false});
      const s = perMember.get(k);
      s.samples++; s.last_tick = row.tick;
      if (h.structure < s.max_structure - adultTolerance) s.structure_decreased = true;
      s.structure_values.add(h.structure);
      s.min_structure = Math.min(s.min_structure, h.structure);
      s.max_structure = Math.max(s.max_structure, h.structure);
      s.max_reserve = Math.max(s.max_reserve, h.reserve);
      s.max_energy = Math.max(s.max_energy, h.energy);
      s.max_gut_material = Math.max(s.max_gut_material, h.gut_material);
      s.max_gut_energy = Math.max(s.max_gut_energy, h.gut_energy);
      if (h.structure + adultTolerance >= h.adult_structure) s.ever_adult = true;
      if (h.reserve === 0) s.reserve_zero_samples++;
      // The exact `world.rs` growth predicate: reserve strictly above the configured fraction
      // of the phenotype's own reserve ceiling. Sampled, so its absence bounds nothing on its
      // own — the structure record below is what actually settles whether growth ever ran.
      if (h.reserve > growthGate * h.reserve_max) s.samples_above_growth_gate++;
      // Oxidation stops draining reserve only while energy is at or above the active
      // threshold's share of E_max. Both thresholds are evaluated on the same samples.
      if (h.energy >= cfg.world_threshold * h.energy_max) s.samples_above_world_oxidation_shutoff++;
      if (h.energy >= cfg.member_threshold * h.energy_max) s.samples_above_member_oxidation_shutoff++;
      bump(s.phases, h.phase);
      for (const b of h.blockers) bump(s.blockers, b);
    }
    assert.equal(adults, row.adults, `census adults disagrees with its own stocks at tick ${row.tick}`);
    sampled.adult_occupancy[Math.min(adults, 3)]++;
  }
  for (const s of perMember.values()) s.structure_values = [...s.structure_values].sort((a, b) => a - b);
  return {sampled, perMember};
}

/// The reserve budget a juvenile could possibly have run on, computed from its own meals.
///
/// Supply is the birth escrow plus what `hunter::digest_step` can assimilate from the exact
/// recorded meals (`eta_m · min(rho/e_r, 1) · material`, per meal, at full digestion and with
/// reserve headroom — the most generous reading of the record). Demand is the oxidation branch
/// running for the member's whole observed life at the configured rate. This is an order-of-
/// magnitude budget from recorded transactions, NOT a tickwise reconstruction of the core, and
/// the actual burn is bounded by whatever reserve was present, so a deficit predicts a member
/// pinned at zero rather than an impossible negative stock.
export function reserveBudget(member, cfg) {
  let assimilable = 0;
  for (const meal of member.meals) {
    if (!(meal.material > 0)) continue;
    const density = meal.energy / meal.material;
    const eta = cfg.assimilation_material * Math.min(cfg.reserve_energy_density > 0
      ? density / cfg.reserve_energy_density : 1, 1);
    assimilable += eta * meal.material;
  }
  const endowment = member.origin === 'paid_descendant' ? FUNDING.escrow_reserve : null;
  const supply = (endowment ?? 0) + assimilable;
  const oxidation_demand = cfg.oxidation_rate * member.observed_seconds;
  return {birth_reserve_escrow: endowment, assimilable_reserve_from_meals: assimilable,
    supply, oxidation_demand_whole_life: oxidation_demand, supply_over_demand: ratio(supply, oxidation_demand),
    basis: 'recorded meals at full assimilation vs the oxidation branch running every tick of the observed life'};
}

export async function reduceArm(dir, manifest, summary) {
  const opening = await json(join(dir, 'opening.json'));
  const org = opening.config.organism;
  const cfg = {growth_reserve_min: org.growth_reserve_min, oxidation_rate: org.oxidation_rate,
    reserve_energy_density: org.reserve_energy_density, assimilation_material: org.assimilation_material,
    assimilation_energy: org.assimilation_energy, build_cost: org.build_cost,
    world_threshold: opening.world_oxidation_threshold,
    member_threshold: opening.world_member_oxidation_threshold};
  const eventRows = await jsonl(join(dir, 'events.jsonl'));
  const censusRows = await jsonl(join(dir, 'census.jsonl'));
  const ledger = reduceReproductionLedger(eventRows);
  const placed = (censusRows[0]?.hunter_stocks ?? []).map(h => idKey(h.id));
  const members = reduceMembers(eventRows, manifest.cohort.opening_tick, summary.closing_tick, placed);
  const {sampled, perMember} = reduceCensus(censusRows, cfg);

  // --- reconciliation: the independent pass against the arm's own audit --------------------
  const mismatches = [];
  const check = (what, got, want) => {if (got !== want) mismatches.push({what, independent: got, summary: want});};
  let captures = 0, paid = 0;
  for (const m of members.values()) {captures += m.captures; paid += m.paid_strike_energy;}
  check('captures', captures, summary.captures);
  check('offspring', ledger.counts.born, summary.offspring);
  for (const key of ['funded', 'born', 'refunded', 'miscarried', 'closed', 'open_at_horizon',
    'not_funded_cap', 'not_funded_stocks'])
    check(`reproduction.counts.${key}`, ledger.counts[key], summary.reproduction_audit.counts[key]);
  for (const key of ['funded', 'born', 'refunded', 'miscarried'])
    check(`reproduction.material.${key}`, ledger.material[key], summary.reproduction_audit.material[key]);
  for (const key of ['build', 'birth'])
    check(`reproduction.heat.${key}`, ledger.heat[key], summary.reproduction_audit.heat[key]);
  assert.deepEqual(ledger.violations, [], `${dir}: funding contract violated`);
  assert.deepEqual(mismatches, [], `${dir}: independent reduction disagrees with the summary`);

  const offspring = [];
  for (const m of members.values()) {
    if (m.origin !== 'paid_descendant') continue;
    const s = perMember.get(m.id) ?? null;
    offspring.push({id: m.id, parent: m.parent, birth_tick: m.birth_tick, death_tick: m.death_tick,
      death_cause: m.death_cause, censored_alive: m.censored_alive, observed_seconds: m.observed_seconds,
      attempts: m.attempts, captures: m.captures, capture_material: m.capture_material,
      capture_energy: m.capture_energy, paid_strike_energy: m.paid_strike_energy,
      reserve_budget: reserveBudget(m, cfg),
      sampled_stocks: s && {samples: s.samples, cadence_ticks: sampled.cadence_ticks,
        distinct_structure_values: s.structure_values, structure_decreased: s.structure_decreased,
        adult_structure: s.adult_structure, ever_adult: s.ever_adult,
        reserve_max: s.reserve_max, energy_max: s.energy_max,
        growth_gate_reserve: cfg.growth_reserve_min * s.reserve_max,
        samples_above_growth_gate: s.samples_above_growth_gate,
        member_oxidation_shutoff_energy: cfg.member_threshold * s.energy_max,
        samples_above_member_oxidation_shutoff: s.samples_above_member_oxidation_shutoff,
        world_oxidation_shutoff_energy: cfg.world_threshold * s.energy_max,
        samples_above_world_oxidation_shutoff: s.samples_above_world_oxidation_shutoff,
        reserve_zero_samples: s.reserve_zero_samples, max_reserve: s.max_reserve,
        max_energy: s.max_energy, max_gut_material: s.max_gut_material, max_gut_energy: s.max_gut_energy,
        phases: s.phases, blockers: s.blockers}});
  }
  const founders = [...members.values()].filter(m => m.origin === 'placed_founder').map(m =>
    ({id: m.id, death_tick: m.death_tick, death_cause: m.death_cause, censored_alive: m.censored_alive,
      observed_seconds: m.observed_seconds, captures: m.captures, attempts: m.attempts}));
  const lastCensus = censusRows[censusRows.length - 1];
  return {
    arm: opening.arm,
    config: cfg,
    ledger_reproduction: {counts: ledger.counts, material: ledger.material, heat: ledger.heat,
      gestations: ledger.gestations},
    ledger_hunting: {captures, paid_strike_energy: paid,
      attempts: [...members.values()].reduce((o, m) => {for (const [k, v] of Object.entries(m.attempts)) bump(o, k, v); return o;}, {})},
    sampled_adult_occupancy_boundaries: sampled.adult_occupancy,
    sampled_census: {boundaries: sampled.boundaries, cadence_ticks: sampled.cadence_ticks,
      first_tick: sampled.first_tick, last_tick: sampled.last_tick},
    summary_adult_occupancy_ticks: summary.adult_occupancy_ticks_0_1_2_over2,
    summary_blocker_member_ticks: summary.reproductive_opportunity.blocker_member_ticks,
    summary_reproductive_opportunity: summary.reproductive_opportunity,
    summary_survival: {founder_extinction_tick: summary.founder_extinction_tick,
      lineage_extinction_tick: summary.lineage_extinction_tick,
      adult_max: summary.adult_max, adult_descendants: summary.adult_descendants,
      descendants_that_reproduced: summary.descendants_that_reproduced,
      closing_hunters: summary.closing_hunters, zero_hunter_ticks: summary.zero_hunter_ticks,
      maximum_live_descendant_depth: summary.maximum_live_descendant_depth,
      surviving_opening_prey_cohorts: summary.surviving_opening_prey_cohorts},
    summary_prey: {closing_prey: summary.closing_population - summary.closing_hunters,
      prey_min: summary.prey_min, prey_tick_integral: summary.prey_tick_integral,
      recovery: summary.whole_recovery.map(r => ({channel: r.channel, opening: r.opening_count,
        minimum: r.minimum_count, status: r.status, first_zero_tick: r.first_zero_tick,
        ticks_below_half: r.ticks_below_half, crossing_tick: r.crossing_tick,
        confirmation_tick: r.confirmation_tick}))},
    summary_audit: {passed: summary.audit.passed, legacy_passed: summary.audit.legacy_passed,
      failure: summary.audit.failure,
      peak_over_limit: [['material', 0], ['corrected_energy', 1], ['independent_energy', 1], ['water', 2]]
        .map(([k, i]) => ({channel: k, ratio: summary.audit[k].magnitude / summary.audit.fixed_limits[i],
          first_crossing: summary.audit[k].first_crossing, nonfinite_at: summary.audit[k].nonfinite_at}))},
    charging_above_reference: summary.oxidation.charging_above_reference,
    closing_census: {tick: lastCensus.tick, hunters: lastCensus.hunters, juveniles: lastCensus.juveniles,
      adults: lastCensus.adults, prey: lastCensus.prey, prey_by_form: lastCensus.prey_by_form,
      hunter_state: {founders_placed: lastCensus.hunter_state.founders_placed,
        hunter_births_total: lastCensus.hunter_state.hunter_births_total,
        hunter_deaths_total: lastCensus.hunter_state.hunter_deaths_total,
        attacks_total: lastCensus.hunter_state.attacks_total,
        captures_total: lastCensus.hunter_state.captures_total,
        predation_deaths_total: lastCensus.hunter_state.predation_deaths_total,
        founder_energy_in: lastCensus.hunter_state.founder_energy_in,
        founder_material_in: lastCensus.hunter_state.founder_material_in}},
    founders, offspring, mismatches};
}

export async function reduceRun(root) {
  const dir = resolve(root);
  const manifest = await json(join(dir, 'manifest.json'));
  const summary = await json(join(dir, 'summary.json'));
  const seeds = [];
  for (const s of summary.seeds) {
    const arms = [];
    for (const [i, name] of ARMS.entries())
      arms.push(await reduceArm(join(dir, `seed-${s.seed}`, name), manifest, s.arms[i]));
    seeds.push({seed: s.seed, arms});
  }
  return {root: dir, manifest, seeds};
}

/// Per-arm closing-state comparison against the earlier run of the same recipe.
///
/// The corrected core is claimed to be narrow. That claim is settled here by the recorded
/// closing state hash and closing tick of every arm, not by reading the diff: an arm counts as
/// reproduced only when both match exactly.
export function priorRunDiff(priorSummary, currentSummary) {
  const index = s => {
    const m = new Map();
    for (const seed of s.seeds) for (const [i, a] of seed.arms.entries()) m.set(`${seed.seed}/${ARMS[i]}`, a);
    return m;
  };
  const before = index(priorSummary), after = index(currentSummary);
  const reproduced = [], changed = [];
  for (const [key, a] of before) {
    const b = after.get(key);
    assert(b, `the rerun is missing ${key}`);
    if (a.closing_state_hash === b.closing_state_hash && a.closing_tick === b.closing_tick) {
      reproduced.push(key);
      continue;
    }
    changed.push({arm: key,
      prior: {closing_tick: a.closing_tick, technical_complete: a.technical_complete,
        complete_experiment_measurement: a.complete_experiment_measurement,
        captures: a.captures, offspring: a.offspring, closing_hunters: a.closing_hunters,
        founder_extinction_tick: a.founder_extinction_tick, lineage_extinction_tick: a.lineage_extinction_tick,
        closing_state_hash: a.closing_state_hash},
      current: {closing_tick: b.closing_tick, technical_complete: b.technical_complete,
        complete_experiment_measurement: b.complete_experiment_measurement,
        captures: b.captures, offspring: b.offspring, closing_hunters: b.closing_hunters,
        founder_extinction_tick: b.founder_extinction_tick, lineage_extinction_tick: b.lineage_extinction_tick,
        closing_state_hash: b.closing_state_hash}});
  }
  return {arms_compared: before.size, arms_reproduced_bit_exact: reproduced.length,
    arms_changed: changed.length, changed,
    basis: 'recorded closing state hash and closing tick per arm; equality is reproduction of the whole trajectory, not of a chosen statistic'};
}

const addTo = (o, k, n) => {o[k] = (o[k] ?? 0) + n;};

export function cohort(runs) {
  const out = {};
  for (const [side, run] of Object.entries(runs)) {
    const perArm = {};
    for (const name of ARMS) perArm[name] = {captures: 0, offspring: 0, funded: 0, born: 0,
      refunded: 0, miscarried: 0, open_at_horizon: 0, not_funded_cap: 0, not_funded_stocks: 0,
      closing_hunters: 0, adult_descendants: 0, descendants_that_reproduced: 0,
      adult_occupancy_ticks: [0, 0, 0, 0], founders_surviving: 0,
      charging: {transactions: 0, reserve_burned: 0, energy_gained: 0, conversion_heat: 0},
      prey_tick_integral: 0, closing_prey: 0, newly_zero_form_channels: []};
    for (const seed of run.seeds) for (const arm of seed.arms) {
      const t = perArm[arm.arm];
      t.captures += arm.ledger_hunting.captures;
      for (const k of ['funded', 'born', 'refunded', 'miscarried', 'open_at_horizon',
        'not_funded_cap', 'not_funded_stocks']) t[k] += arm.ledger_reproduction.counts[k];
      t.offspring += arm.ledger_reproduction.counts.born;
      t.closing_hunters += arm.summary_survival.closing_hunters;
      t.adult_descendants += arm.summary_survival.adult_descendants;
      t.descendants_that_reproduced += arm.summary_survival.descendants_that_reproduced;
      arm.summary_adult_occupancy_ticks.forEach((v, i) => {t.adult_occupancy_ticks[i] += v;});
      t.founders_surviving += arm.founders.filter(f => f.censored_alive).length;
      for (const k of ['transactions', 'reserve_burned', 'energy_gained', 'conversion_heat'])
        addTo(t.charging, k, arm.charging_above_reference[k]);
      t.prey_tick_integral += arm.summary_prey.prey_tick_integral;
      t.closing_prey += arm.summary_prey.closing_prey;
      for (const r of arm.summary_prey.recovery.slice(1))
        if (r.opening > 0 && r.first_zero_tick !== null)
          t.newly_zero_form_channels.push(`${seed.seed}:form${r.channel - 1}`);
    }
    out[side] = perArm;
  }
  return out;
}

/// The juvenile-development evidence, assembled only from paid descendants that actually exist.
export function juvenileEvidence(run) {
  const children = [];
  for (const seed of run.seeds) for (const arm of seed.arms)
    for (const child of arm.offspring) children.push({seed: seed.seed, arm: arm.arm, ...child});
  const sampled = children.filter(c => c.sampled_stocks);
  const total = k => sampled.reduce((a, c) => a + c.sampled_stocks[k], 0);
  const structures = new Set();
  for (const c of sampled) for (const v of c.sampled_stocks.distinct_structure_values) structures.add(v);
  return {
    children: children.length,
    children_with_census_coverage: sampled.length,
    census_samples: total('samples'),
    distinct_structure_values_observed: [...structures].sort((a, b) => a - b),
    any_structure_decreased: sampled.some(c => c.sampled_stocks.structure_decreased),
    ever_adult: sampled.filter(c => c.sampled_stocks.ever_adult).length,
    samples_above_growth_gate: total('samples_above_growth_gate'),
    samples_above_member_oxidation_shutoff: total('samples_above_member_oxidation_shutoff'),
    samples_above_world_oxidation_shutoff: total('samples_above_world_oxidation_shutoff'),
    reserve_zero_samples: total('reserve_zero_samples'),
    max_reserve_over_children: Math.max(...sampled.map(c => c.sampled_stocks.max_reserve)),
    max_energy_over_children: Math.max(...sampled.map(c => c.sampled_stocks.max_energy)),
    children_with_zero_captures: children.filter(c => c.captures === 0).length,
    children_with_zero_attempts: children.filter(c => Object.keys(c.attempts).length === 0).length,
    deaths_by_cause: children.reduce((o, c) => {bump(o, c.death_cause ?? 'censored_alive'); return o;}, {}),
    total_observed_seconds: children.reduce((a, c) => a + c.observed_seconds, 0),
    total_capture_material: children.reduce((a, c) => a + c.capture_material, 0),
    total_capture_energy: children.reduce((a, c) => a + c.capture_energy, 0),
    reserve_supply: children.reduce((a, c) => a + c.reserve_budget.supply, 0),
    reserve_oxidation_demand: children.reduce((a, c) => a + c.reserve_budget.oxidation_demand_whole_life, 0),
    children_detail: children};
}

/// The committed-asset projection: every seed and every arm is still present, but each side is
/// reduced to the fields the report actually cites. Nothing is filtered out by outcome — the
/// unsuccessful arms, the zero-capture children and the null opportunities all survive this.
export function compactView(full) {
  const side = a => ({
    captures: a.ledger_hunting.captures, attempts: a.ledger_hunting.attempts,
    paid_strike_energy: a.ledger_hunting.paid_strike_energy,
    reproduction: a.ledger_reproduction.counts,
    survival: a.summary_survival,
    summary_adult_occupancy_ticks: a.summary_adult_occupancy_ticks,
    sampled_adult_occupancy_boundaries: a.sampled_adult_occupancy_boundaries,
    mature_member_ticks: a.summary_reproductive_opportunity.age_and_size_ready_member_ticks,
    member_ticks: a.summary_reproductive_opportunity.member_ticks,
    stock_gates_open_member_ticks: {
      reserve: a.summary_reproductive_opportunity.age_and_size_ready_reserve_gate_open_member_ticks,
      energy: a.summary_reproductive_opportunity.age_and_size_ready_energy_gate_open_member_ticks,
      both: a.summary_reproductive_opportunity.age_and_size_ready_both_stock_gates_open_member_ticks},
    max_reserve_fraction: a.summary_reproductive_opportunity.max_reserve_fraction,
    max_energy_fraction: a.summary_reproductive_opportunity.max_energy_fraction,
    blocker_member_ticks: a.summary_blocker_member_ticks,
    charging_above_reference: a.charging_above_reference,
    prey: {closing_prey: a.summary_prey.closing_prey, prey_min: a.summary_prey.prey_min,
      prey_tick_integral: a.summary_prey.prey_tick_integral,
      // Channel 0 is total prey, channels 1..8 are forms 0..7. Every channel is kept,
      // including the ones with no opening stock, so "no opportunity" stays distinguishable
      // from "did not decline".
      recovery: a.summary_prey.recovery.map(r => [r.opening, r.minimum, r.status,
        r.first_zero_tick, r.ticks_below_half, r.crossing_tick, r.confirmation_tick])},
    audit: {passed: a.summary_audit.passed, legacy_passed: a.summary_audit.legacy_passed,
      failure: a.summary_audit.failure,
      peak_over_fixed_limit: a.summary_audit.peak_over_limit.map(p => p.ratio),
      any_crossing: a.summary_audit.peak_over_limit.some(p => p.first_crossing !== null || p.nonfinite_at !== null)},
    founders: a.founders,
    closing_census: a.closing_census});
  return {...full,
    compact_legend: {
      prey_recovery_channels: 'index 0 = total prey, indices 1..8 = forms 0..7',
      prey_recovery_row: ['opening_count', 'minimum_count', 'status', 'first_zero_tick',
        'ticks_below_half', 'crossing_tick', 'confirmation_tick'],
      audit_peak_over_fixed_limit: ['material', 'corrected_energy', 'independent_energy', 'water']},
    seeds: full.seeds.map(s => ({seed: s.seed, arms: s.arms.map(p =>
      ({arm: p.arm, background: side(p.background), candidate: side(p.candidate)}))})),
    projection: 'compact: every seed/arm/outcome retained; per-arm gestation lists, per-child blocker and phase histograms and the full per-child records live in juvenile_evidence.children_detail and in the unprojected reduction'};
}

export async function reduce({background, candidate, priorBackground, priorCandidate}) {
  // The strict gate first, unmodified and unwrapped. Nothing below is reported without it.
  const strict = await compareCharge(background, candidate);
  assert.equal(strict.artifact_checks_passed, true);
  const runs = {background: await reduceRun(background), candidate: await reduceRun(candidate)};
  const prior = {};
  for (const [side, dir] of [['background', priorBackground], ['candidate', priorCandidate]]) {
    if (!dir) continue;
    prior[side] = {prior_root: resolve(dir), current_root: runs[side].root,
      ...priorRunDiff(await json(join(resolve(dir), 'summary.json')),
        await json(join(runs[side].root, 'summary.json')))};
  }
  const mismatches = [];
  for (const run of Object.values(runs)) for (const seed of run.seeds) for (const arm of seed.arms)
    mismatches.push(...arm.mismatches);
  return {
    kind: 'independent-hunter-charge-rerun-reduction',
    strict_gate: {entry_point: 'compare-hunter-recipes.mjs compareCharge (imported unchanged)',
      kind: strict.kind, artifact_checks_passed: strict.artifact_checks_passed,
      policy: strict.policy, limits: strict.limits,
      result_sha256: createHash('sha256').update(JSON.stringify(strict)).digest('hex')},
    biological_acceptance: false,
    acceptance_note: 'A passing artifact gate plus a passing independent reduction means the twelve-seed rerun is technically complete and internally consistent. It is not a lineage-viability result: no paid descendant reached adult structure in any arm.',
    provenance: {build: runs.candidate.manifest.build,
      executable_sha256: runs.candidate.manifest.executable_sha256,
      arms: ARMS, planned_ticks: runs.candidate.manifest.ticks,
      opening_tick: runs.candidate.manifest.cohort.opening_tick,
      audit_window: runs.candidate.manifest.audit_window,
      background_recipe: runs.background.manifest.profile_recipe,
      candidate_recipe: runs.candidate.manifest.profile_recipe},
    summary_vs_independent_mismatches: mismatches,
    prior_run_reproduction: prior,
    cohort: cohort(runs),
    juvenile_evidence: juvenileEvidence(runs.candidate),
    seeds: runs.background.seeds.map((b, i) => ({seed: b.seed,
      arms: b.arms.map((arm, k) => ({arm: arm.arm, background: arm, candidate: runs.candidate.seeds[i].arms[k]}))})),
    limits: 'Reads recorded artifacts only. It does not run the executable, decode semantic WorldState, reconstruct contact geometry, or re-derive the core audit. Sampled census quantities are 200-tick boundaries and are labelled sampled; tickwise quantities come from the run\'s own audit and are labelled summary. The reserve budget is a bound from recorded meals and the configured oxidation rate, not a tickwise reconstruction. Charging totals are per-arm process-scoped. Off-hunter arms carry the policy and are not identity controls.'};
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    const argv = process.argv.slice(2);
    const compact = argv.includes('--compact');
    const [background, candidate, priorBackground, priorCandidate] = argv.filter(a => a !== '--compact');
    assert(background && candidate && argv.length <= 5,
      'Usage: node scripts/reduce-hunter-charge-rerun.mjs BACKGROUND_DIR CANDIDATE_DIR [PRIOR_BACKGROUND_DIR PRIOR_CANDIDATE_DIR] [--compact]');
    const full = await reduce({background, candidate, priorBackground, priorCandidate});
    console.log(JSON.stringify(compact ? compactView(full) : full, null, 1));
  } catch (error) {console.error(`Reduction refused: ${error.message}`); process.exitCode = 1;}
}
