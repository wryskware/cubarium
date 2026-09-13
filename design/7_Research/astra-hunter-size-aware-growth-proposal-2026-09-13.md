---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Next juvenile experiment: size-aware permission for paid growth

Recommend **one opt-in hunter-member growth-gate change**, on the existing
charge80 background. Keep ordinary fauna, costs, allocation order and all other
hunter parameters unchanged. This is a biology experiment proposal, not a
validated parameter, accepted decision, implementation or live introduction.

## Why this mechanism next

The corrected cohort (`c045bf6`) and its independent verification (`b1f1d65`)
cover all eleven birth-producing arms, eighteen children and 29 members of the
retained charging experiment. Every child stayed at S=0.8; the growth branch was
entered zero times across 161,636 gate observations. Four children accumulated
more than their 0.8 birth reserve, but none reached the **1.2** gate. Eight
scavenged; five, not seven, had zero total intake. Seventeen starved and one was
right-censored. These facts support testing development permission; they do not
prove sufficient food or energy to mature.

The current code first performs oxidation and then requires
`R > growth_reserve_min * phenotype.reserve_max` before it considers even one
paid growth increment (`world.rs:1841`). The hunter's decoded adult Smax=2 and
Rmax=4 remain fixed throughout juvenile life (`genome.rs:409–411`); configured
growth fraction 0.3 therefore demands R>1.2 even from a body of S=0.8.
The paid increment already has rate, remaining-structure, reserve and battery
caps. The unexplored hypothesis is that the adult-scaled *permission* threshold
unnecessarily delays affordable incremental development, not that those caps
should be removed.

Alternative acquisition/steering changes could improve food access, but the
current evidence cannot choose among encounter, targeting, windup, cooldown and
payment constraints. Sensing is a large upkeep demand, not proof that its price
or radius is wrong. Removing juvenile charging would directly change usable
energy as well as reserve retention, and its recorded above-reference share is
not a counterfactual effect. Changing these together would obscure the growth
question. None is bundled into this candidate.

## Exact single candidate

Suggested recipe name: `reserve-targets-charge80-size-gate-v1`.

At the **existing post-oxidation, pre-growth site**, use:

```text
legacy_gate = growth_reserve_min * Rmax
size_ratio  = clamp(S / Sadult, 0, 1)

reference: gate = legacy_gate
candidate authoritative hunter member: gate = legacy_gate * size_ratio

enter only when S < Sadult AND R > gate
```

Use actual structure at this site, not age, art scale, rendered juvenile flag,
birth escrow, or a newly resized reserve capacity. Ordinary organisms and legacy
profiles retain the exact old multiplication and comparison. At adult structure
the candidate equals the old gate, and growth is already finished.

For this frozen profile the candidate gate is **0.6*S**: 0.48 at birth S=0.8,
0.72 at S=1.2, 0.96 at S=1.6 and 1.2 at adulthood S=2. This is an unvalidated
size-normalization hypothesis, not a threshold fitted to the richest child.
It introduces no extra tunable coefficient and does not set the gate to zero.

After the predicate, retain the existing arithmetic and ordering exactly:

```text
dS = min(juvenile_growth_rate*DT, Sadult-S, R, E/build_cost)
     [omit the E/build_cost term only when build_cost is zero, as today]
R -= dS; S += dS
cost = min(build_cost*dS, E)
E -= cost
heat += cost + reserve_energy_density*dS
```

The gate is a precondition, **not** a new protected reserve floor. Do not also
cap the increment to `R-gate`, reorder oxidation after growth, waive handling or
upkeep, speed growth, change reserve/energy maxima, or give a later meal. Those
would be additional biological mechanisms. Existing genotype, maintenance,
sensing, food selection, phase timings, strike charge, assimilation, offspring
funding and age checks remain unchanged. Changed body size can subsequently
alter contact scale, eligible prey size and movement bills; those are consequences
of growth, not separately tuned variables.

## What is paid, and what can get worse

With DT=.05 and the current member growth rate .002, a rate-limited step builds
at most **0.0001** structure, spends the same reserve, consumes **0.00005**
battery at build_cost=.5, and emits **0.00025** heat including the reserve's
stored energy. No imported resource is added. Building the missing 1.2 structure
would take at least **600 seconds** at uninterrupted maximum growth, require
1.2 reserve and 0.6 battery for construction, and emit 3.0 construction heat,
in addition to all maintenance, movement, handling and oxidation.

Earlier development diverts reserve away from later battery conversion and makes
a larger body more expensive to maintain/move. It can shorten survival, reduce
strike affordability, or stop at a slightly larger juvenile. Conversely it can
change prey eligibility/capture mechanics and later resource access. The answer
must be measured in the paired worlds.

A useful **arithmetic illustration only**: freezing out intake and every other
flow, converting some of the 0.8 reserve to structure meets the rising gate near
S=1.0, R=0.6, not adulthood (up to one rate-sized crossing). Allowing continuous
maximum oxidation as well moves that intersection to approximately S=.8485.
Neither calculation is a simulated trajectory; real energy, oxidation activity,
encounters and death respond. The corrected cohort's “0.4 intake” bookkeeping
split likewise predicts no candidate child's outcome. Small initial growth can
spend parental endowment even for a child that never eats; count it as paid
development, not successful acquisition or self-sustaining recruitment.

## Persistence and isolation contract

Use one explicit, validated semantic profile selector, not a hidden harness
mutation of the global organism config. A same-shaped next unused profile
version is sufficient if it means exactly **charge80 + this growth gate**;
version 5 is unused in the source inspected here, but reserve it at implementation
time. Do not reinterpret existing versions 3 or 4 or change the default trial.

Important implementation trap: `FixedHunterProfile::oxidation_policy` currently
recognizes charge80 only for version 4 and falls back to Configured otherwise.
The new version must explicitly preserve fixed **0.80 Emax** oxidation. Simply
bumping its version and leaving that match unchanged introduces a second change.
Retain seek/perch .80/.90, all geometry and costs, the fixed genome and every
other serialized profile value from the retained charge80 openings.

No new evolving state is required for this memoryless predicate. A same-layout
semantic selector can use the existing snapshot envelope; old binaries must
reject the unsupported profile rather than resume it under another policy.
Legacy no-hunter/old-profile continuation and migration retain their exact
meaning. If integrated into a newer envelope, prove reference identity through
the genuine schema-12 projection, not by comparing incompatible header hashes.
Do not add the candidate to the cube's startup configuration or share quiet-policy
or ambient-support experiments with these worlds.

## Matched experiment, fixed before running

Reference biology is the **charge80 candidate of the corrected charging study**,
not its uncharged background:

- Frozen source `512ee52a54207ff3d6b59b6fee753c9603480af6`, build
  `0.1.0+512ee52`, executable SHA256
  `65d42d78ecc3fc6fff4e9f5490d5e78df61f7f275ee8584e45221c3177b02828`.
- Retained reference artifacts:
  `captures/hunter-charge-candidate-two-hour-512ee52`, recipe
  `reserve-targets-charge80-v1`. Do not modify or relabel them.
- All **seeds 1–12**, the same schema-9 mature openings at tick 144000 and
  recorded seed-only Top placements/headings. No choice based on prey density,
  prior births or which eleven arms produced offspring. Retain zero-child arms.
- Two recipes × the existing **six arms**: untouched, budget control,
  specialist attacks off/on, facultative attacks off/on. That is 144 arm outcomes
  for the paired two-hour study. No care in any arm; natural weather unchanged.
- Each living-founder arm imports one identical adult, S=2,R=2,E=3 at these
  openings, M=4/Q=7 through the existing initializer and receipt. Budget control
  receives the same imported M/Q in its existing form; retain any immediate
  conversion heat. No second initialization, relocated founder, extra child or
  adult population clamp.
- First biological horizon: **144000 elapsed ticks**, closing tick **288000**,
  samples every **200**. Run all arms to that horizon or retain their exact
  technical failure; numerical failure is not extinction and neither disappears
  from the report. Use fresh exclusive output directories and pinned binaries.

Run the reference and candidate with the same newly frozen implementation and
observer. Require all 72 reference continuations to reproduce the old reference
state and event histories (schema-12 projection if needed). Untouched and budget
arms must be identical between recipes. Attack-disabled arms are real living
controls; the policy still applies to a juvenile if one is born. Since the old
two-hour disabled arms had no offspring, their ecological trajectories should
also remain identical here; report the profile selector difference separately.
For a hunting pair, demand identity until the **first actual altered growth
transaction**, not forever after descendants and prey begin interacting differently.

The first collection is a mechanism/viability screen, not balance certification.
After complete technical review and all-seed biological interpretation, consider
the **unchanged** recipe at 24h, then 72h to test repeat recruitment, prey recovery,
rarity and unattended persistence. No automatic extension from a green inventory
audit, and no retuning during an extension. One adult in one seed is not a winner.

## Minimal observations and tests that make the biology answerable

Keep the existing full-ID hunter/reproduction/contact events, all-seed census,
founder/descendant ancestry, paid escrow linkage and local/whole-prey recovery
measurement. No ID matching across diverged worlds after the shared history ends;
pair by seed/arm and retain each world's complete descendant histories.

Reuse the existing bounded mutation-site growth ledger in the experimental build
if available (`0d867f1:flow.rs`), rather than starting another preliminary replay
loop. Record at the same gate site actual S,R,E, actual gate, predicate outcome,
growth-positive ticks/amounts and binding caps, then actual reserve/build/heat
transfers. The existing ledger's scalar `gate_reserve` is overwritten on every
observation: for a dynamic gate, label it **last**, add minimum/maximum and retain
the threshold at the first positive growth. Do not present its final value as a
constant lifetime requirement. Keep per-member first growth, maximum structure,
first adult boundary, observed adult duration, source-specific intake, death or
right-censoring, funding and any descendant births. Drain bounded records/bins;
do not retain every tick's full world.

Minimum pre-run tests:

- Old profiles and ordinary prey take exactly the original gate/arithmetic;
  new profile still resolves charge80 and changes no founder inventory.
- Candidate gate values at S=.8,1.2,1.6,2; strict equality refuses and the next
  representable value above permits consideration. Invalid/zero adult denominators
  cannot enter through validated profiles/states; no NaN comparison fallback.
- Actual growth respects all four existing caps, including no battery, low
  reserve, final adult increment and zero growth rate; debit/heat identities
  close at the mutation site. These branches were dormant in the retained cohort
  and need genuine positive-growth fixtures before the experiment.
- Snapshot restart while a child is growing preserves exact state/events and
  selector; an old reader refuses the new semantic version. Observer enabled/
  disabled remains trajectory-identical; no extra RNG or shifted physiology.

Audit all arms from the common **pre-import** inventory with original
`1e-8 * max(opening,1)` limits: material, water, persisted-corrected energy,
independent windowed energy and initializer boundaries. Include bodies, escrow
and hunter guts once each; growth transfers R→S and releases chemical energy as
heat, not an external source. Keep raw legacy energy as a separately honest
diagnostic, not a substituted or widened conservation gate.

Report per seed and role, including zero opportunities: paid children, any growth,
maximum S/Sadult, adult descendants and adult-descendant time, reproduced
descendants, restricted survival/death cause and censoring, total paid growth
versus intake/oxidation/upkeep/strike costs, and offspring escrow balances. Show
founder survival, 0/1/2/>2 adult occupancy and every matched prey/form/ancestry
loss and recovery/censored window. Never pool away a failed or barren seed.

Interpretation is deliberately separable: new paid increments establish that the
gate prevented those increments under the shared initial conditions; **maturation
and sustainable reproduction are separate outcomes**. Growth with worse child
survival is a meaningful adverse result. No growth despite the now-open gate
points to the measured energy/rate/remaining-stock caps, not proof of absent prey.
More adults with damaging prey losses fails the ambient ecosystem objective.
The next action comes from that complete comparison, not a manufactured target
of one or two immortal adults.
