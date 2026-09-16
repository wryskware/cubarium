---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Ecology v1 next steps: results of workstreams E, F and G (2026-09-16)

Fable's consolidated result for the reconciled next steps recorded in
[ecology-v1-next-results-2026-09-15.md](ecology-v1-next-results-2026-09-15.md)
("Next recommendation") after
[Astra's review](ecology-v1-next-review-2026-09-15.md). Step 2 (the host
ecology guard) was done in the review's repair cycle. Steps 1 and 5 are
workstream E, steps 3 and 4 are workstream F, step 6 is workstream G. Briefs:
[E](../handoffs/ecology-v1-budget-opus-2026-09-16.md),
[F](../handoffs/ecology-v1-movement-opus-2026-09-16.md),
[G](../handoffs/ecology-v1-presentation-2-opus-2026-09-16.md), committed at
`b70624d`. All three ran in parallel: E on `main`, F and G in worktrees.

**Disposition, in one paragraph.** The single most informative cheap
experiment was run and it reverses one conclusion: the trained forager's body
is feasible on the held-out patches (a scripted mobile control survives 11 of
12 layouts), so the controller, not the energy budget, binds; the trained
policy travels more and eats fifteen times less. The apex mating radius is the
wrong knob: no introduced apex ever lives to its own minimum reproduction age,
so no pair is ever eligible and the radius never becomes the deciding term.
Raising the movement price does what the spatial-dilution hypothesis predicted
for range and depletion, and then overshoots: at both raised prices the
grazing founders starve before their first brood, and recovery still never
appears because depleted cells stay depleted while the rest of the world
greens. The skimmer dies of starvation carrying the generalist diet, and the
skimmer body is the best-surviving body when it carries a foliage diet. None
of this is "ecosystem healthy"; all of it is now measured rather than
inferred.

## Dispatch and budget

| stream | model | where | simulation used / cap | storage | tests |
| --- | --- | --- | --- | --- | --- |
| E budget + apex audit | Opus 5 high | `main` | ≈ 63 s / 16 min | 108 KiB / 20 MiB | core 476, search 94 (+9, +8) |
| F movement arm + census | Opus 5 high | worktree, merged `a4262af` | 5.3 min / 20 min | 1.6 MiB / 30 MiB | search +22 |
| G presentation follow-up | Opus 5 medium | worktree | none | sheet only | see below |

Fable's verification: E's two experiments re-run from the note's commands and
compared field for field with the retained JSON (identical, timing keys
excluded); one fast-leaf row at each of F's three prices re-run and matched by
`final_state_hash`; F's 36 baseline-price rows carry A's retained screen
hashes (F's own check, 36 of 36); after the merge `cubarium-search` 116
passed, `cubarium-core` 476 passed, the host's neural-seed tests 7 passed.
The parameter box is now 14 names long, so A's retained 13-component rows are
refused by `replay` on length rather than padded; they replay under the build
that wrote them (`e635088`), and this build reproduces them by re-running.
Fable accepted that: silent padding would be the same mistake the ecology hash
exists to prevent.

## E — the per-body store budget, the feasibility experiment, the apex audit

Full note: [ecology-v1-budget-2026-09-16.md](ecology-v1-budget-2026-09-16.md).
Commits `46ac238` (core: `world::budget` per-organism ledger and
`encounter::ApexOpportunity` counters, opt-in through
`World::record_body_budgets`, default off, not persisted, snapshot schema
unchanged), `3bfb602` (search: `es-budget`, `apex-audit`), `2533cb1` (note).

**Inertness and identity.** Three hash tests show the ledger changes no
number the simulation produces (record off, record untouched, record and
drain every tick give one `state_hash` over 9,000 ticks), and two identities
(material and energy, kept separate so a sign error cannot hide) close to
3.9e-12 over every life in 48 rows. Throughput cost −0.8 % on the whole world,
−2.3 % on a plant-only world, inside the 3 % ceiling.

**Feasibility, `fast-leaf`, 12 layouts, horizon 36,000, medians:**

| driver | survived | lifetime | best 2 k-tick credit/bill | served m | cells |
| --- | --- | --- | --- | --- | --- |
| stationary grazing | 0/12 | 10,441 | 1.49 | 0.98 | 1 |
| mobile script (control) | **11/12** | 36,000 | 1.79 | 12.21 | 52 |
| initial centre | 0/12 | 7,239 | 0.33 | 0.31 | 148 |
| generation 9 | 0/12 | 8,421 | 0.53 | 0.83 | 254 |

Branch verdict: "the body's budget binds" is **refuted** (the control pays for
itself and ends at full reserve); "the controller binds" is **supported and
sharper** (generation 9 visits five times the cells and travels 1.6 times the
distance of the control while taking in fifteen times less, so its failure is
a failure to feed, not to relocate); "relocation is necessary" is also
**supported** (the stationary grazer pays for itself while its cell lasts,
then its credit goes to exactly zero). The two supported branches are not in
tension. Two free corrections to the record: the route foliage decline 34 → 14
in the training rows is the fixture's painted stands relaxing, not the
policy's grazing (the initial centre ate 0.31 m in its whole life); and the
`diet` 0.7 body's measured channel split is 44 / 28 / 28 foliage / fruit /
litter. For a wandering body the motor is 13.9 % of the whole bill,
translation 18 : 1 over turning.

**Apex audit, two-apex arm, baseline and `fast-leaf`, four held-out seeds,
horizon 180,000.** Readiness overlap is exactly zero in all 8 runs. Two adults
were alive together for 10,315–12,859 ticks and mature-and-perched together
for about 5,300 ticks, and no candidate pair ever formed. The gate is the
profile: `reproduce_min_age_seconds` = 1,200 s = 24,000 ticks, and the oldest
apex in any run reached 14,101, so every introduced adult died at 43–59 % of
its own minimum reproduction age. Moving `MATING_RADIUS_PX` would have changed
nothing in these runs; it is not yet an owner-facing choice. What decides
whether an apex can ever be a population is eligibility: a lower minimum age,
a longer-lived introduced adult, or introducing already-eligible adults, and
which depends on why an adult dies near 11,000 ticks against a 7,200 s
lifespan, which the same ledger can now answer for an apex.

## F — the movement-cost arm and the variety census

Full note: [ecology-v1-movement-2026-09-16.md](ecology-v1-movement-2026-09-16.md).
Commits `3934eb7` (pre-registration before any row), `7206fc7` (15
definition tests, 14 red against a stub), `18cace4` (`movement.rs`, the
recorder, `organism.move_cost` in the box, the `--prices` axis), `d3e907e`
(note); merged at `a4262af`. Matrix: `organism.move_cost` ∈ {0.00036, 0.0018,
0.006} × {baseline, `fast-leaf`} × 6 training seeds × arms 0/1/2, horizon
180,000, everything else as A's screen; 108 rows, 0 invalid.

| config | price | cells/body late | depletion events | recovery events | late pop | kinds | worlds lost | verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.00036 | 289 | 14.6 | 0.17 | 43.4 | 2.11 | 0 | control |
| baseline | 0.0018 | 52 | 55.9 | 0.33 | 33.9 | 1.00 | 0 | partial |
| baseline | 0.006 | 44 | 45.7 | 0.28 | 24.4 | 1.00 | 3 | partial |
| `fast-leaf` | 0.00036 | 369 | 0.7 | 0.00 | 59.4 | 3.00 | 0 | control |
| `fast-leaf` | 0.0018 | 174 | 12.2 | 0.11 | 43.4 | 1.67 | 0 | partial |
| `fast-leaf` | 0.006 | 55 | 22.3 | 0.00 | 37.4 | 1.00 | 1 | partial |

Against the pre-registered rules no cell is confirmed and none refuted. Range
falls below 60 % of the control in every raised-price cell, monotonically;
depletion rises 4–31×, which is the first direct test of the dilution
hypothesis and supports it; the recovery clause fails everywhere because
depleted cells stay depleted to the end of the run (cells still depleted at
the end ≈ cells ever depleted) while whole-world foliage climbs to 3.4–4.3×
opening and stands do not die. The missing half is regrowth time on the
depleted cells, not pressure and not stand death. The dominant effect is an
overshoot: at both raised prices all 180 grazer and 90 glider founders
produce zero offspring, starving at 180 s (0.0018) and 79 s (0.006) of life,
before the brood gate (`bud_min_age` 120 s + gestation 30 s + the reserve
threshold) can be met, leaving a burrower monoculture. Four whole-world
extinctions occurred at 0.006, all in apex arms, the first this harness has
produced (A had none in 306 runs); suggestive at n = 4, not evidence. F
disclosed a defect in its own pre-registration: the depletion clause was
already met by the baseline control, so the conjunctive recovery clause
decided every cell. F proposed no equation; the refutation branch was not
reached. Named next run: a finer ladder {0.00036, 0.0006, 0.0009, 0.0012,
0.0018}, arm 0 only, gated on the herbivore bodies' first brood, about 3
minutes.

**Skimmer attribution.** Lost to starvation (252 of 259 and 233 of 235
skimmer deaths), median loss at tick 18,510 (baseline) and 65,760
(`fast-leaf`). Not the controller (held constant). Not the body: within the
skimmer body, the foliage-diet bin survives 84 % against 20 % for the
generalist bin, and among foliage-diet bodies the skimmer survives best (84 %
against glider 23 %, burrower 20 %). Habitat neither marked nor excluded. The
supported reading is lower realised yield of the generalist diet at γ = 1,
inferred from the contract's arithmetic and not measured; E's ledger now
makes it measurable. Confound stated: foliage-bin skimmers are descendants.
Net energy margin per body was not measured in F because E had not landed
when F ran; F reports death cause and last stores instead.

## G — presentation follow-up

*(pending: G's result lands here)*

## What this does and does not establish

- Established by measurement: the training body is feasible on the held-out
  patches; the trained controller fails to feed; the apex never becomes
  eligible; range and depletion respond to the movement price; recovery does
  not; the skimmer starves on the generalist diet and thrives on a foliage
  diet.
- Not established: why generation 9 does not eat (intake, not travel, is
  localised; the three separable outcomes are named in E's note); why an
  introduced apex dies near 11,000 ticks; whether any price between the
  current one and five times it clears the brood gate; whether the depleted
  cells are the marginal ones (per-cell `L·μ` not recorded); anything about
  a whole-world neural population.
- The cube shows none of this. The display world is the legacy controller in
  `fast-leaf` at the shipped movement price; nothing in E or F changes a
  number the display produces.

## Next recommendation

1. **Diagnose generation 9's intake** with the ledger on: per tick, is the
   body on a cell with `P > feed_min`, what are its three mouth efforts, what
   did it take. Three outcomes separate cleanly (efforts off, never on food,
   bite clamped) and decide whether the next move is the score, the
   observation or the action adapter. No training until then.
2. **Apex eligibility before radius**: with the ledger, measure why an
   introduced adult dies near 11,000 ticks; then choose between a lower
   minimum reproduction age, a longer-lived introduced adult, or
   already-eligible introductions. The radius stays where it is.
3. **The finer movement-price ladder**, arm 0, gated on the first brood, with
   E's ledger giving net margin per body and per-depleted-cell `L·μ`
   recorded, so regrowth time on depleted cells is measured rather than
   inferred.
4. **Diet yield by bin** from the ledger in a whole world, to turn the skimmer
   reading from arithmetic into measurement before any γ experiment.
