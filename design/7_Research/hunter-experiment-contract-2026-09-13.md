---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Copied-world fixed hunter experiment: acceptance and data contract

This is an implementation-ready experimental recipe, not validated ecological
tuning or permission to introduce hunters into the live cube. It refines the
earlier Astra fixed-hunter plan and root's `fixed-hunter-core-handoff-2026-09-13.md`.
Accounting is now schema 9; the delegated hunter extension is schema 10. At this
review the hunter module/API is not yet on disk. API names below are requested
capabilities to map to the worker's delivered types, not claims they already exist.

Lore retrieval and direct source inspection covered the two plans, current
`WorldState`, corrected `care_compare`, material/escrow methods, life events and
the twelve-hour reports. Those reports show surviving total populations but loss
of skimmers and founder cohorts. Neither maturity nor sufficient prey supply for
hunters has been established by those results.

## 1. Freeze the cohort before observing predator outcomes

Prepare default, autonomous worlds for seeds **1 through 12 inclusive**, with no
care, hunter, rescue, reseeding or configuration edits. Preserve current ordinary
prey mutation settings; the hunter genotype alone is fixed. Use one frozen build,
save its commit/executable SHA256 and complete configuration. Drain transient
events/telemetry at documented cadences without consuming simulation RNG.

Save every seed at exactly **144,000 ticks (2 simulated hours)**. This is the
primary age cohort: enough elapsed time to observe multiple possible 120-second
prey budding opportunities, not a claim that a stationary ecosystem has formed.
Do not wait for a population plateau, retry a seed, select a rich moment or
replace a world that has lost a form or become extinct. Save the initial seed
snapshot too, and carry the observer ancestry map if founder ancestry is wanted.

Once all twelve aged snapshots exist, sort them by opening non-hunter population,
breaking ties by numerical seed. Assign ranks 1–4 / 5–8 / 9–12 to low / middle /
high population strata. Run **all twelve**; stratification is reporting, not a
filter. Record counts by form, occupied cells, producer/fruit, water, detritus
M/Q, and surviving founder cohorts if actually tracked. If ancestry was not
retained, call the twelve aged populations opening cohorts, not founder lineages.

An optional later aged-world robustness cohort uses the **same seeds at exactly
864,000 ticks (12 hours)**, independently evolved without hunters or care. Keep
that age cohort separate from the primary results, with its own strata. It is
not twelve new independent seeds and must not replace inconvenient two-hour
openings. Root can prepare the two-hour snapshots now; twelve-hour maturation
is optional follow-on work, not a prerequisite to the first experiment.

Use a fixed placement recipe unrelated to food/prey density. Candidate recipe for
seed index `i=seed-1`: Top face, `u=12+20*(i%3)`, `v=8+16*floor(i/3)`, with a fixed
valid heading or a documented seed-only heading. Save the exact transported
target/heading. All paired arms use the same target. This tests canopy placement,
not all habitats; do not relocate a starving hunter after looking at the world.

## 2. Six paired arms, not one ambiguous attacks-off control

| Arm | Founder import | Attacks | Detritus scavenging |
| --- | --- | --- | --- |
| Untouched | None | No hunter | No hunter |
| Budget control | Same derived founder M/Q, deposited locally as D/De | No hunter | No hunter |
| Specialist off | One identical living founder | Disabled | Zero |
| Specialist on | One identical living founder | Enabled | Zero |
| Facultative off | One identical living founder | Disabled | Candidate fraction from saved profile |
| Facultative on | One identical living founder | Enabled | Same fraction as facultative off |

Within each off/on pair, only `attacks_enabled` differs; use identical founder
genome, resources, placement and all other profile values. One shared disabled
arm would confound attack effects with the facultative diet. Disabled hunters
may still move, metabolize, reproduce or die according to the delivered profile;
record these effects rather than describing the arm as inert.

The budget control has equal external M/Q, not equal living storage or behavior.
Use a validated core initializer/receipt. Do not mutate D/De or source counters
from the host to manufacture this control. If a custom config's detritus cap
requires immediate heat, record that heat: imported chemical energy is still the
same, retained chemical energy need not be. If the initializer is unavailable,
report the budget arm as unavailable and implement that missing capability before
claiming the complete comparison, without substituting a different food dose.

All arms are care-free in the primary experiment. A later care experiment is a
separate factorial block with identical scheduled requests/targets across arms;
actual receipts may differ with local waste or allowance, so equal requests do
not imply equal delivered resources. No automatic top-up or replacement hunter.

## 3. Inventory and numerical gates

Keep the common aged **pre-initialization** snapshot as the audit opening. Do not
open the audit after insertion or recompute away its resource jump on reload.
At defaults the proposed founder is S=2, R=2, E=3, hence 4 material and 7 energy;
derive the actual amounts from config and the initializer receipt, never from
these illustrative constants. Record pre/post initializer stocks, import ledgers,
corrected light/heat readings and any immediate heat. Repeat initialization must
be refused without state change; resume never initializes again.

With deltas measured from that common opening, define:

```text
M = sum(N + P + F + D) + sum(organism.material()) + sum(hunter gut M)
Q = sum(e_p*P + e_f*F + De)
    + sum(E + e_r*R + escrow.E + e_r*(escrow.S + escrow.R))
    + sum(hunter gut Q)
W = sum(water depth)

R_M = delta M - delta external_material_in - delta hunter_import_M
      - delta care_feed_M + delta care_clean_M
R_Q = delta Q - corrected_energy_ledgers.net_since(opening_ledgers)
      - delta hunter_import_Q - delta care_feed_Q + delta care_clean_Q
R_W = delta W - delta rain_in_total + delta evap_out_total
```

Absent escrow terms are zero. Hunter bodies remain ordinary organisms and are
counted exactly once; only their separate guts are added. Adult structure has no
chemical energy. Capture/digestion/reproduction move internal stocks, not external
imports. Budget-control material belongs to fields, not an imaginary body or gut.
The new import is booked in exactly one source ledger, not both the hunter ledger
and existing `external_material_in`. Rain's delivered care contribution is already
part of total rain; never add it twice.

Use unchanged per-resource limits `1e-8 * max(common_opening_inventory, 1)` across
all six arms. Check finite signed residuals before maxima; save peak magnitude,
sign/tick and first crossing. Material/water, persisted corrected energy,
independent windowed energy and initializer/receipt boundary identities must all
pass. Preserve the legacy raw energy peak/verdict separately as in care_compare.
Run `check_invariants` every tick. An accounting failure is a failed command and
failed variant, even if its final population looks appealing; emit evidence first.

The independent energy observer sums transient light/heat once per 200-tick
window with compensation, plus actual initializer/care receipts, and includes any
initializer heat in that window. Start observer accounting before initialization,
not after clearing its heat away. `step()` returns counters accumulated since
telemetry reset, not a new independent increment on each call. Compare 20-tick
cadence on a fixed subset; full state hashes must agree. Neither corrected nor
independent gates may use a larger threshold as the run gets longer.

For continuations, persist the experiment's common opening inventories, ledger
readings, accumulated observer sums and censoring state in a separate manifest,
alongside exact arm checkpoints. New World construction's rederived mass baseline
does not prove historical conservation. A resumed segment may instead explicitly
report a new segment audit, but cannot relabel that as a continuous full-horizon
audit without carrying/reconciling the previous segment. Initial implementation
can run uninterrupted and checkpoint outputs for reproducibility.

## 4. Minimal data and API mapping

Write one immutable manifest per cohort/profile revision, one summary per
seed/arm, streamed events and sparse census/window records. Include:

- Schema/build/executable identifiers; complete config/profile and SHA256; input
  snapshot SHA256 and full/projection hashes; seed, maturation age, stratum,
  target/heading, tick duration, planned/actual observation horizon and termination
  reason. Serialize u64 hashes as strings and organism IDs as slot+generation.
- Initializer outcome, exact founder ID, imported M/Q, immediate heat and all
  opening stocks/ledgers. Hash each saved post-initialization arm too.
- Per-tick aggregate prey/hunter counts for integrals and minima; census output
  every 200 ticks with per-form prey counts, adults/juveniles/escrows, phase counts,
  gut M/Q, stocks, source/sink deltas, audit residuals and observer event cursors.
- Drain every `LifeEvent` and `HunterEvent` after each step. Keep birth/death IDs,
  parent IDs, cause, tick, hunter membership and ancestry. Founders emit no ordinary
  birth in current core, so initialize the hunter lineage from the receipt.
- Every attempt: stable attempt ID, hunter/target full IDs, contact/claim result,
  failure reason, paid strike/movement/handling energy when available. Every
  capture: prey ID/form, actual capture surface position, transferred M/Q and
  hunter ID. A capture and its ordinary predation death are one prey death, not two.
- Escrow creation, cancellation, birth and maturity records; parent/child full IDs,
  stored escrow S/R/E, actual parent funding debits and build/birth heat when
  exposed. Track founder and each descendant separately.

The existing LifeEvent payload has no capture position or funding transaction.
Prefer explicit HunterEvent fields for those facts. If only previous-tick position
or before/after parent stocks are available, label them approximate: movement and
other physiology occur in the same tick. Missing transaction data cannot support
a claim of independently measured exact reproduction funding. It is acceptable
to combine exact core transfer tests with observed escrow-to-child linkage, clearly
distinguishing those two sources of evidence; do not block the core worker merely
for richer presentation telemetry. Capture position is needed for the exact local
recovery metric below; otherwise report that metric unavailable.

Bound memory: stream events and maintain current living ancestry only; do not
retain every birth/event in a giant JSON array. Running all six worlds for one
seed in lockstep makes matched spatial queries possible without a complete
per-cell history for every arm. Never truncate silently: an exhausted observer
budget or disk error is an incomplete measurement with its own termination reason.

## 5. Local recovery after captures

Use surface field-graph distance, not face-local coordinate distance. Define a
fixed neighbourhood as all cells at most **three graph edges** from the prey's
capture cell, crossing seams and respecting the open rim. This is a candidate
spatial scale, explicitly recorded. It measures local occupancy, not descendants
replacing that particular prey or the whole food web returning to equilibrium.

Record every capture, but select recovery windows reproducibly: the first capture
in each fixed **600-second elapsed bin** per hunting arm, ties by attempt ID.
Selection never depends on subsequent recovery. Record the number of unselected
captures. For each selected capture, retain the same cells/tick in all paired
controls. The chosen locations are treatment-selected exposure sites, not random
spatial samples or perfectly exchangeable causal controls.

Maintain a 60-second rolling history of per-cell non-hunter counts sampled every
10 seconds (six snapshots) in each arm. For a selected capture, freeze its
pre-event reference as the neighbourhood's mean over those six prior snapshots.
Also record the immediate post-step count and prey-form-specific counts; do not
call that post-step count the exact instant before/after removal. Captures with
less than 60 seconds of prior observation remain logged with `insufficient_pre`.

For a complete pre-window, set `target = ceil(pre_mean)`. A measurable local
deficit requires `target >= 1` and immediate post-step count `< target`. Otherwise
classify `no_measured_deficit`, not recovered at time zero. In a deficit window,
recovery means count reaches at least target and stays there for **60 seconds**
at the ten-second sampling cadence. Record first qualifying crossing and its
confirmation tick; mark the result interval-observed at that cadence.

Follow for at most **3,600 seconds**, reporting counts and paired
differences-in-change at 60 / 300 / 900 / 3,600 seconds where observed:
`(treated_N(t)-treated_pre_mean) - (control_N(t)-control_pre_mean)`.
Controls use their own pre-window mean in the same cells. Subsequent captures in
the neighbourhood are additional exposures: count and flag them, but do not
discard difficult windows or reset the clock. Report single-capture and recurrent
exposure windows separately. Local births, natural deaths and migration can all
contribute; occupancy recovery does not identify their relative contributions.

Statuses distinguish `recovered`, `no_measured_deficit`, `insufficient_pre`,
`not_recovered_by_3600s`, and `right_censored` by run end or technical termination.
The one-hour non-recovery status is itself censored beyond one hour, not proof
recovery never occurs. A crossing without the full 60-second confirmation before
closing remains unconfirmed/censored. Never divide recoveries by all captures or
treat censored windows as successes/failures; show denominators and observation
time. At most seven one-hour windows per hunting arm are active with this binning.

## 6. Whole-world prey, sparse adults and paid descendants

Count prey by nonmembership, not form index. Record per-form and total prey
minima, time integrals, closing values, exact first zero ticks and time spent below
50% of each arm's common pre-import opening population. Forms absent at opening
are marked absent, not newly lost. Report treatment-control paired differences by
seed, separately from founder-cohort loss; forms are not lineages.

A candidate whole-world recovery statistic follows the first drop below 50% of
opening prey: return to at least 90%, maintained for 30 minutes, observing for up
to six hours or run end. Report no-decline cases separately and censor unfinished
follow-up. Use the same rule for each opening-present form and all controls; do
not attribute baseline attrition to hunting. These numerical thresholds are
preregistered measurement choices, not already validated ecological targets.

Define adult explicitly from the worker's structural/adult-size rule; report
reproductive eligibility separately (age/reserve/energy/cooldown). Integrate every
tick into occupancy bins **0, 1, 2, >2 adults**, with raw tick denominators, maximum,
longest >2 episode and juvenile counts. Report both all observation time and
conditional-on-at-least-one-adult time. A hunter that dies immediately must not
look perfectly sparse by contributing only zeros. Record all-founder and
whole-lineage extinction ticks, right-censor living IDs at closing, and separately
report zero-hunter intervals. Never reseed or enforce a two-adult cap.

For reproduction count escrow starts, miscarriages, cap refusals, births, offspring
reaching adult size and descendants themselves reproducing. Link each child to
one prior parent escrow; retain funding evidence and both build/birth heat terms.
Exact funding identities belong in transaction/core tests as well as the common
resource audit. Birth count alone is not funded reproduction evidence, and one
surviving imported founder is not a self-replacing lineage.

## 7. Staging and acceptance

Trial horizons are elapsed **after** hunter/control initialization:
2h = 144,000 ticks; 24h = 1,728,000; 72h = 5,184,000. The existing care_compare
CLI caps at 24h, so a hunter-specific harness needs an explicit checked 72h bound;
do not silently truncate it. Advance the entire twelve-seed set for a selected
profile, not just successful seeds. Extinct and sparse worlds still contribute
through the same horizon; their outcomes are never replaced. Checkpoints can
continue to the next horizon only if profile/config remain identical.

Hard executable gates are conservation/finite state, initializer exactly once,
event/population reconciliation, single capture/death per prey, escrow linkage,
deterministic restart and observer-cadence identity. Numerical failures stop that
arm with an evidence artifact; they do not become an ecological extinction datum.
Any corrected-core twelve-hour accounting gate still open in the parent work
remains open; this contract does not waive it.

For the two-hour screen, report all six arms across all twelve seeds. A hunting
profile with no successful captures anywhere is a mechanism/profile failure to
investigate, not evidence of balanced rarity. If all on-arms erase prey while
their paired controls survive, do not extend that profile unchanged as a candidate
success. Otherwise longer runs are useful even if all imported hunters die:
episodic specialist survival is allowed, but a profile with no hunter descendants
cannot satisfy the user's reproducing-lineage aspiration. Extend controls with
the chosen profiles, preserving failed/dead seeds in every summary.

For candidate evaluation at 24/72h, report across-seed distributions of extra prey
loss, recovery/censoring, funded adult descendants and the adult occupancy bins.
Operationalize the user's one-or-two preference with a **candidate** screen of
at least 90% of occupied-adult time at one or two adults and no more than 5% of
all observed time above two, both per-seed and pooled. These thresholds are
proposals, not acceptance canon; show the raw durations so they remain revisable.
Also report how many seeds produce a funded child, an adult descendant and a
second-generation parent, without hiding absence behind the sparsity percentages.

Passing this harness means the declared experiment completed with trustworthy
accounting/data. It does not automatically approve a profile for the cube. A
convincing candidate additionally needs observed hunting, some paid lineage
replacement, recoverable prey and sparse occupancy across the cohort, plus separate
visual contact/readability review. Parameter revisions get a new profile hash and
repeat paired seeds; change one documented parameter family at a time. Do not
retroactively select ages, placements, windows or denominators to make it win.
