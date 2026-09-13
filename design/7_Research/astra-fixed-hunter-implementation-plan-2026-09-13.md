---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Opt-in fixed Lanternjaw hunter: implementation-ready experiment

Wrysk prefers Fable's Lanternjaw body and wants rare larger predators with costly
single-offspring reproduction. The target is usually one or two adults, with
possible absence and overlapping generations. This document proposes a bounded
first ecological implementation; all numbers below are trial parameters, not
validated balance or accepted canon. Fable owns the body/rendering work separately.

Sources inspected: canon ledger and rules; M3c in `implementation-plan.md`; E7 in
`experiments.md`; `care-and-megafauna-proposal-2026-09-12.md`; the Lanternjaw study
and integration notes; current core `world.rs`, `organism.rs`, `genome.rs`,
`events.rs`, `pairs.rs`, `rng.rs`, `view.rs`, and snapshot schema 8. Lore retrieved
the design and implementation pointers before source inspection.

## Scope and invariants

Start in copied worlds with one fixed hunter and ordinary prey. No live founder
insertion, new care button, prey rescue, mating requirement, mutation of hunting,
armor system, or permanent predator quota belongs to this slice. Off by default;
an empty hunter extension leaves the existing operations and RNG draws unchanged.
One or two adults is measured as an outcome, never enforced by a population test.

Keep the current material and energy model. In particular, existing adult
structure `S` carries **zero chemical energy**; reserve `R` carries `e_r*R`, and
the usable battery is `E`. Escrow carries `e_r*(S_c+R_c)+E_c` until birth converts
its structural part and releases that energy as heat. Calling prey “meat” cannot
create a new energy density for its structural material.

## State, API, and migration

Append `hunters: HunterState` to `WorldState`, bump schema 8 to 9, retain explicit
schema-7 decoding and add an immutable schema-8 projection/mirror. Preserve all
current `WorldConfig`, `Genome`, `Phenotype`, `Organism`, and `Escrow` fields and
their wire order in this slice. The schema-8 projection must reproduce genuine
old payloads, including care state. Keep the existing schema-7 ecology comparison
for its current purpose; name the new schema-8 projection hash explicitly.

Proposed extension:

```text
HunterState {
  profile: Option<FixedHunterProfile>,  // version + all numeric trial parameters
  members: Vec<HunterMember>,           // sorted by full OrganismId
  founder_material_in, founder_energy_in,
  predation_deaths_total, attacks_total, captures_total
}
HunterMember {
  id: OrganismId,
  phase: Perched | Stalking | Windup | Strike | Recovering | Handling,
  phase_started_tick, phase_ends_tick,
  target: Option<OrganismId>,
  attack_counter, next_reproduction_tick,
  gut_material, gut_energy
}
```

Include a persisted attempt timeout if it cannot be derived from phase timestamps.
Do not store stale prey handles as slot numbers: use slot plus generation. Validate
unique sorted member IDs, live membership, finite nonnegative stocks/profile,
phase/tick consistency, bounded gut capacity, target validity where required, and
the extension's cumulative counters. Members may survive with no prey; a stale
target is cleared deterministically, not resolved to a reused slot.

Use one explicit initialization API, for example
`World::start_hunter_trial(profile, founder_target) -> Result<HunterFounderReceipt>`.
It refuses an already initialized extension and insufficient organism capacity,
validates everything before mutation, and adds exactly one founder. The host
experiment entrypoint requires a distinct output state directory, saves its source
snapshot hash/seed/recipe, initializes once, then writes a durable opening snapshot
before running. Resume reads the saved extension and never inserts another founder.
The first implementation need not add this command to the live care journal.

Membership grants the experimental hunting behavior; `genome.form` only selects
appearance. A saved unrelated form-4 organism must never become a predator. Select
the production Lanternjaw rig through the explicit render-view role/rig mapping
agreed with Fable, preserving old form fallback semantics.

## Founder inventory and candidate physical profile

Candidate existing genome values: `size=2`, `reserve=2`, `mouth=1`, `speed=1`,
`sense=8`, `metabolism=0.5`, `depth=0.85`, `swim=0.1`, fixed hue and Lanternjaw
visual role. Decode through current code. At current organism defaults this gives
`S_adult=2`, `R_max=4`, `E_max=4`, ordinary maximum speed about 0.252 px/s and
maintenance coefficient 0.0025 per structural unit per second. Additional hunting
speed is a paid profile capability; it is not smuggled through an out-of-range gene.

Initialize `S=2, R=2, E=3`, no escrow and an empty gut, at a seed-determined canopy
target. At default `e_r=2`, external founder inventory is **4 material and 7 energy**.
Derive and record these values from the actual config; never hardcode that inventory
for a custom world. Book the founder once in the extension's import ledgers, not
again in the existing `external_material_in`. Include it in the opening audit:

```text
material residual adds: + sum(gut_material) - hunter_founder_material_in
stored energy adds:    + sum(gut_energy)
energy sources add:    + hunter_founder_energy_in
```

Retain the existing care sources/exports and founding baseline terms. Compare
before/after initialization as well as each later transfer; reload's rederived
material baseline is not evidence of historical conservation.

The 18px art silhouette is longer than the existing phenotype's lobe extent. Define
a hunter collision/sensing extent from the assembled body's tested support, subject
to the existing surface-unfolding limit. Do not silently enlarge every creature's
radius or reuse the whole tail fan as a mouth. Candidate jaw anchor is six pixels
forward with 1.5px reach; Fable must confirm that this point tracks the visible jaw.
Transport the mouth offset and headings with existing surface helpers.

## Tick integration and local hunting

All additional passes branch off immediately when `profile` is absent. Preserve
ordinary prey decisions and turn draws, then add only a local escape term where
an observed hunter is actually threatening. Hunt sensing uses the current surface
unfolding and generation-checked IDs, never global prey population or a face-local
distance that misses seams. Record neighbour-list truncation in experiments.

Candidate phase rules:

- Perch/rest while reserve is above 65% or a meal is being handled. Seek when
  reserve falls below 35%, with hysteresis between those values.
- Stalk a locally sensed non-hunter with `0.15 <= prey.S <= 0.75*hunter.S`.
  Choose the nearest eligible prey, ties by full ID; abandon after eight seconds
  without contact or upon loss of local sensing. Juveniles use their actual S.
- When the mouth is in reach, wind up for 0.6s with a visible folded-to-open
  gesture; it grants no capture. Prey sensing this threat biases motion away,
  can turn within a documented escape turn limit, and can briefly move up to
  twice its own maximum. Escape distance is limited by available movement energy
  before movement, and all movement/escape costs become heat.
- After windup the hunter enters Strike and may burst at up to 1px/s for one second,
  paying the existing structure-scaled movement cost plus a trial strike cost
  of 0.08 energy. It must have the full strike cost available before attempting;
  no free strike after its battery empties. Cap traveled distance by affordable
  movement energy for this experimental burst, preserving ordinary movement
  semantics for creatures outside the interaction.
- Once at the end of Strike, re-evaluate mouth contact **after both creatures move**. Out-of-range,
  stale, ineligible, or unaffordable targets are failed attempts. In-range paid
  contact succeeds with trial probability
  `clamp(0.65*hunter.S/(hunter.S+prey.S), 0.1, 0.75)`. This is a bounded first
  size-based hypothesis, not an evolved defense model. A failed strike recovers
  for five seconds; a successful one enters Handling.

Add a new RNG stream value without renumbering existing streams. Key it by the
full hunter ID and a persisted attack counter. Specify one draw per paid attempt;
rendering, logging and failed target lookup consume no draws. Prey escapes through
actual steering and affordable movement; capture probability is not a substitute
for the contact check.

Resolve contested captures once per prey. Build attempts from a common post-move
state, order by deterministic attempt priority (dedicated seeded draw, then full
ID), and reserve at most one successful claim per prey. All paid attempts still
pay, including a loser whose target was claimed. Never remove/harvest the same
prey twice across seams or two hunters.

Insert capture settlement after movement and before ordinary field feeding and
physiology. Mark claimed prey as unavailable to those later passes and commit their
removal once. This may be a deferred removal set or immediate arena removal with
every later ID access rechecked; preserve exactly one death event and do not also
run the normal death-to-detritus path for the claimed body. Unclaimed ordinary
organisms keep the existing tick order. Prey escrow cannot also produce a child
after the parent was captured that tick.

## Carcass transfer, handling, and hunter death

Use one bounded carried carcass pool per hunter, initially empty. Before a capture,
require the entire prey inventory to fit the trial gut capacity (candidate 4m);
otherwise do not attempt that prey. No partial-body damage model is needed yet.
Transfer exactly:

```text
M = prey.S + prey.R + escrow.S + escrow.R
Q = prey.E + e_r*prey.R + escrow.E + e_r*(escrow.S+escrow.R)
gut_material += M; gut_energy += Q; remove prey and its escrow exactly once
```

Omit escrow terms when absent. This is internal transfer, with no source ledger and
no detritus-energy cap: the gut retains the actual energy removed. During Handling,
no attacks, grazing or fruit intake occur. Spend a candidate 0.002e/s handling cost,
bounded by available energy, in addition to normal maintenance/sensing. If the
handling cost cannot be fully paid, no digestion happens that tick; reserve
oxidation can recover energy through the existing physiology on later ticks.

Digest at a candidate maximum 0.1m/s, stopping when reserve/energy headroom makes
further intake unnecessary. For a consumed portion `q` of homogeneous gut contents,
take `carried = gut_energy*q/gut_material`; let
`a = eta_m*min(q, carried/e_r)` (use the current zero-e_r convention if allowed),
also bound `a` by reserve headroom by reducing q consistently. Then:

```text
gut M -= q; gut Q -= carried
hunter.R += a
fields.D += q-a                    // rejected material, zero energy
spare = carried - e_r*a
gain = min(eta_e*spare, E_max-E)
hunter.E += gain; heat += spare-gain
```

Clamp only rounding-scale residue; book actual deltas. This handles energy-poor
prey honestly, including prey with much structure and almost no reserves. After
the meal finishes, keep a candidate 20s recovery pause. If gut contents persist
through satiety they remain counted and checkpointed; no hidden discard timer.

For a facultative variant only, allow existing detritus scavenging at 25% of the
decoded maximum when no target or gut exists, with a reserved fraction of handling
capacity; disable producer/fruit grazing for hunters. Run a specialist variant
with that fraction zero. These are explicit profile allocations, not a claim that
the old two-way diet gene already encodes flesh digestion.

Hunter death uses the current body/escrow transfer and additionally transfers its
gut to local `D/De`, retaining at most `energy_cap*gut_material` and emitting the
rest as heat. Remove its member record and targeting references. No corpse, escrow,
or held gut may disappear because a member ID was removed.

## One paid offspring and slow replacement

Reuse `Organism.escrow`; each parent can hold only one. Override the hunter's
reproduction thresholds/timing through the saved fixed profile rather than changing
all organisms' config. Candidate gate: full adult structure, age at least 1200s,
reserve at least 80%, energy at least 75%, no active gut/hunt, and a 1800s recovery
interval after the previous birth. Gate only on local parent state, not world counts.

Keep current child inventory fractions initially: at defaults a hunter escrows
`S_c=.8, R_c=.8, E_c=.6`, debiting parent reserve by 1.6 and usable energy by
`build_cost*.8 + .6 = 1.0`; the .4 build energy goes to heat. Escrow's structural
reserve energy remains stored until birth, when `e_r*.8=1.6` becomes heat. Use
120s gestation and candidate juvenile growth cap .002m/s, retaining the existing
growth reserve/build-cost constraints. This makes minimum structural maturation
about 600s from .8 to 2, longer when food is scarce. These times need measurement.

Child membership and fixed genome are copied explicitly, even if ordinary prey
mutation is enabled; hunter mutation is outside this experiment. The child starts
with no target/gut and a fresh attack counter. Place through the existing surface
birth transport, then add its new full ID to the hunter extension. Parent death
miscarries once; cap refusal returns the escrow through the existing resource
path and sets a retry/recovery interval to avoid a free per-tick birth loop.
Retain the ordinary total-organism safety cap, not a special two-hunter ceiling.

## Observer and renderer contract

Add a predation cause/event and separate predation counter without resizing the
old three natural-death counters in the persisted schema-8 prefix. Emit a capture
record naming predator, prey, tick and actual transferred M/Q; emit attempts/failures
with cause and paid energy in explicit diagnostic logs. Birth/death analysis must
handle the new cause. Add adult/juvenile hunter counts, meal/capture rate, gut M/Q,
hunter offspring, starvation/extinction time, and per-form prey populations.

Expose a transient `HunterView` keyed by full ID: phase, phase progress, mouth
anchor, target when valid, gut fraction and actual escrow progress. Fable can map
windup/strike/recovery/handling to the chosen body. A strike animation must not imply
a successful capture; a cocoon appears only when resource escrow exists. Keep all
analytical counters outside the ambient frame.

## Deterministic tests and paired experiment

Before tuning, test founder exact inventory, each transfer identity, energy-poor
prey, partial final digestion, full reserve/gut refusal, hunter death with gut and
escrow, miscarriage/cap refusal, and one paid offspring. Contact tests cover prey
escaping during windup, unaffordable attacks, two hunters claiming one prey, reused
prey slot, and seam/vertex mouth transport. Snapshot/restart at windup, paid attempt,
part-filled gut, and gestation must match uninterrupted full-state hashes. Empty
extension plus genuine schema-8 fixture continuation must match old payloads.

Run twelve paired seeds from the same documented mature prey snapshots; stratify
by initial prey population rather than choosing only rich snapshots. Keep care
off or replay exactly the same external-input schedule in every arm. Save profiles
and all opening inventories. Compare:

1. Untouched prey-only baseline (context; no hunter import).
2. Budget-matched prey control: deposit the founder's 4m/7e as local `D/De` at the
   same target; default cap2 admits this density. For another config distribute
   imported energy only into valid stores and book any required initial heat.
3. Same living hunter and inventory with attacks disabled (isolates its maintenance,
   movement/scavenging/reproduction effects).
4. Fixed specialist hunter and facultative hunter variants, separately.

Use the deposited-material control for total-budget comparisons and the living
attack-disabled control for attack causality; their initial storage composition
differs, so neither is described as a perfect ecological counterfactual. Include
founder imports in every relevant audit rather than hiding them in residual baselines.

Begin with two simulated hours per seed, then 24h and 72h only for variants that pass
accounting and do not universally collapse the prey. Sample predator/prey time
series and adult occupancy, capture failures, paid effort, offspring funding,
prey minima and recovery after local hunts, resource residuals, and right-censored
hunter survival. Use matched spatial cells to measure prey recovery over minutes
after a capture, and whole-world per-form recovery over hours. Record zero-hunter
periods and time above two adults; do not silently reseed or censor these outcomes.

Trial success means readable paid hunting, some funded descendants across seeds,
surviving/recovering prey, and mostly sparse adult occupancy; it does not require
permanent founder survival. If all hunters starve or prey are erased across seeds,
adjust one documented profile parameter family and repeat paired runs before
enabling mutation or putting the lineage into the approved live world.

Key risks are current prey margins, prey battery/structure energy mismatch,
multipart body contact alignment, juvenile food access, predation biased toward
the slowest kind, and excess attacks needed to pay a larger body's ongoing costs.
At the candidate founder defaults, maintenance alone costs about 18e/hour before
sensing/movement: apparently leisurely behavior can still require substantial
prey turnover. Measure that demand; do not conceal it with free energy or guaranteed
rescue. Longer lifespan or lower maintenance would be explicit later tuning choices.
