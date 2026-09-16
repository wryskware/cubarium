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

**Disposition, in one paragraph** (corrected after
[Astra's review](ecology-v1-next-steps-review-2026-09-16.md); see "Review and
repair"). The single most informative cheap experiment was run and it reverses
one conclusion: the trained forager's body is feasible on eleven of the twelve
held-out patches (a scripted mobile control survives them), so on those the
controller, not the energy budget, binds; the trained policy travels more and
eats fifteen times less. The apex mating radius is the
wrong knob: no introduced apex ever lives to its own minimum reproduction age,
so no pair is ever eligible and the radius never becomes the deciding term.
Raising the movement price does what the spatial-dilution hypothesis predicted
for range and depletion, and then overshoots: at both raised prices the
founder grazers starve before their first brood (the gliders too, except in
`fast-leaf` at the lower raised price, where they breed), and recovery stays
rare — 13 crossings in 72 raised-price runs — for reasons the campaign did not
record. The skimmer dies of starvation carrying the generalist diet, and
foliage-diet skimmer descendants are the best-surviving bodies in that world,
an association that does not yet separate diet from body. None of this is
"ecosystem healthy"; the feasibility, eligibility and range results are
measured, and the recovery and skimmer mechanisms remain open.

## Dispatch and budget

| stream | model | where | simulation used / cap | storage | tests |
| --- | --- | --- | --- | --- | --- |
| E budget + apex audit | Opus 5 high | `main` | ≈ 63 s / 16 min | 108 KiB / 20 MiB | core 476, search 94 (+9, +8) |
| F movement arm + census | Opus 5 high | worktree, merged `a4262af` | 5.3 min / 20 min | 1.6 MiB / 30 MiB | search +22 |
| G presentation follow-up | Opus 5 medium | worktree, merged `77c42e8` | none | 25 KB sheet | host 596 (+7), render 101 |

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

Branch verdict: "the body's budget binds" is **refuted on 11 of 12 layouts**
(the control pays for itself; nine survivors end at full reserve, one ends
thin, and on `h2` it dies at 26,355); "the controller binds" is **supported and
sharper** (generation 9 visits five times the cells and travels 1.6 times the
distance of the control while taking in fifteen times less, so its failure is
a failure to feed, not to relocate); "relocation is necessary" is also
**supported** (the stationary grazer pays for itself while its cell lasts,
then its credit goes to exactly zero). The two supported branches are not in
tension. Two corrections to the record: the route foliage decline 34 → 14 in the
training rows is principally the fixture's painted stands relaxing, not the
policy's grazing (the initial centre ate 0.31 m in its whole life; grazing is
the smaller term); and generation 9's measured channel split on these layouts
is 44 / 28 / 28 foliage / fruit / litter (the initial centre's is about
50 / 26 / 23, so this is a controller-on-layouts measurement, not a signature
of every `diet` 0.7 body). For a wandering body the motor is 13.9 % of the whole bill,
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
hypothesis and supports it; the recovery clause fails everywhere, but
recovery is rare rather than absent (13 crossings in the 72 raised-price
runs against 3 in the control), and why depleted cells mostly stay below half
their opening foliage while whole-world foliage climbs to 3.4–4.3× is **not
resolved**: slow regrowth, returning consumers and marginal cells are all
live, and post-depletion visits and per-cell `L·μ` were not recorded. The
dominant effect is an overshoot: at both raised prices all 180 founder
grazers produce zero offspring, starving at 180 s (0.0018) and 79 s (0.006)
of life, before the brood gate (`bud_min_age` 120 s + gestation 30 s + the
reserve threshold) can be met; the gliders do the same in three of the four
raised cells, but in `fast-leaf` at 0.0018 they produce 357 births and 101
are alive at the horizon, so "burrower monoculture" holds only for the other
three cells (corrected after review). Four whole-world
extinctions occurred at 0.006, all in apex arms, the first this harness has
produced (A had none in 306 runs); suggestive at n = 4, not evidence. F
disclosed a defect in its own pre-registration: the depletion clause was
already met by the baseline control, so the conjunctive recovery clause
decided every cell. F proposed no equation; the refutation branch was not
reached. Named next run: a finer ladder {0.00036, 0.0006, 0.0009, 0.0012,
0.0018}, arm 0 only, gated on the founder **grazer's** first completed brood
with the glider reported separately, about 3 minutes.

**Skimmer attribution.** Lost to starvation (252 of 259 and 233 of 235
skimmer deaths), median loss at tick 18,510 (baseline) and 65,760
(`fast-leaf`). Within the skimmer body, the foliage-diet bin survives 84 %
against 20 % for the generalist bin, and among foliage-diet bodies the
skimmer survives best (84 % against glider 23 %, burrower 20 %). That is a
strong association and not an isolation (corrected after review): every
foliage-bin skimmer is a descendant (75 entrants, 12 deaths, 63 alive at the
horizon), so later birth, censoring, lineage selection and other loci are
mixed in, and the shared controller algorithm still behaves differently on
different bodies. Habitat neither marked nor excluded. The leading hypothesis
is lower realised yield of the generalist diet at γ = 1, from the contract's
arithmetic; the clean test is a matched form × diet factorial with E's ledger,
not more bins.
Net energy margin per body was not measured in F because E had not landed
when F ran; F reports death cause and last stores instead.

## G — presentation follow-up

Full note: [ecology-v1-presentation-2-2026-09-16.md](ecology-v1-presentation-2-2026-09-16.md).
Commits `a2eff47` (the cue, the env hook, the tests), `8fa7c6c` (note, cost
arm); merged at `77c42e8`. Sheet:
[ecology-v1-shoulder-2026-09-16.png](assets/ecology-v1-shoulder-2026-09-16.png)
(rows 1–3: a controlled 30 % foliage loss under shoulders 0.85 / 0.95 / 1.0;
rows 4–6: B's real strip-then-reflush trajectory under the same three). No
core change, no new asset, no mapping constant changed.

**The shoulder, measured.** Each frame is drawn twice from the same view, at
the shoulder under test and at a shoulder of 10⁻⁹, so only the shoulder
differs between the two encoded 8-bit frames.

| shoulder | first frame the stand's sprite changes | loss at that frame | ramp leaves 1 at | ungrazed average-light stand |
| --- | --- | --- | --- | --- |
| 0.85 (ships) | never in 180 frames | — | 30.2 % | whole |
| 0.95 | 132 | 22.4 % | 22.0 % | mix 0.001, not visible |
| 1.0 | 108 | 18.3 % | 17.9 % | mix 0.014, bare wood showing |

Astra's finding 7 is exactly right of the plant sprite at 0.85 and false of
the cell: ground cover, producer wash and flecks read `P` directly and change
an encoded pixel at frame 3 (0.5 % lost) whatever the shoulder. Of the 30.2
points the 0.85 sprite ignores, 17.9 are not the shoulder's at all: B0's
bright stand carries `P/W` = 1.217 and the fullness ratio clamps at 1, so
that much loss cannot move the sprite at any shoulder. B's stated cost of
raising the shoulder (the average-light class breathing) does not appear at
0.95 (wobble excess −0.0004 against the 0.05 bound); at 1.0 an ungrazed
average stand draws bare wood, reporting depletion that is not happening.
On the real trajectory the three are hard to tell apart (rows 4–6). **G
recommends 0.95; the default stays 0.85; the decision is Wrysk's**, and the
cube now runs 0.95 through `CUBARIUM_FOLIAGE_FULL` (below) so he can judge it.

**The soil-band dead-wood cue.** `soil_snag = clamp(Wd_density − W_density, 0, 1)`
through the same cube root, rather than a hard `W == 0` gate, because `W`
reaches zero exactly when `Wd` is largest and a hard gate would stamp the
whole mark in one frame (§12 forbids cuts); the difference fades it in over
the dieback. Named plainly (after review): this is a continuous dead-wood
*dominance* cue that can show while a stand is still dying, not a strict
"stand is dead" mark; Astra would not veto the mapping, only its first
description. The mark is the cell's own soil plant at stage 0 cut to its
bottom 2.5 tile rows, in the ash tone, at `SOIL_PLANT_OPACITY · 0.70 ·
soil_snag`, stamped over the band's scenery so the litter a dying stand
produces cannot hide the only record that it died. `D + C` still drive the
band alone. Visible effect: a dead or dying cell below the horizon carries a short
grey-blue stub among its mushrooms once dead wood dominates, brightest just
after death, fading as `Wd` decomposes; a never-planted cell, or a living
stand larger than its dead wood, is pixel-identical to before. Fable's note: this is a cut mushroom, not an authored snag; G says a
purpose-drawn one would read better.

**Runtime shoulder override** (Fable's mid-task scope addition at Wrysk's
request): `CUBARIUM_FOLIAGE_FULL=<float>` read once per process at first
presenter construction, clamped to 0.5..=1.0, unparsable values fall back to
0.85 with one stderr line; no CLI flag; default unchanged. G recorded that
this contradicts the brief's own "not a runtime setting" and that Fable
widened it. After review Fable added one startup log line naming the
effective shoulder when an override is in force, because the build id alone
does not identify the live mapping, and fixed the stale "no runtime setting"
source comment.

**Tests** `art_ecology` 17 → 24 (soil mark present for dead wood, absent at
`Wd` = 0, monotone, distinguishable from litter-only and empty; dead column
distinct, no crown, fading to soil; env reading by injected string). Two of
G's own tests failed first on their measurement design, not the
implementation, and were redesigned (differencing two saturating stocks to
isolate the dead column, since a side face has no pixel row the topmost
foliage cell's own plant cannot reach). Per-frame cost on B's four arms is
inside noise (interleaved A/B, min of three); a new fifth arm with every soil
cell dead (320 marks) costs +2.2 ms, 11.2 ms mean / 12.2 ms worst against the
16.7 ms budget.

**Not done:** cube and viewer not inspected by G; no world run long enough
to watch a stand die below the horizon (every dead soil cell seen is
synthetic); water band still shows nothing for dead wood.

## Deployment (Fable, 2026-09-16)

Wrysk authorised stopping the running cube for a build he can judge. The
runner someone had restarted on `e17e537` (resumed at tick 249,600, reached
337,653) was stopped with SIGINT; its `state/` moved to the session
scratchpad as `state-fastleaf-e17e537-retired-2026-09-16` (not deleted). Then:

```bash
CUBARIUM_FOLIAGE_FULL=0.95 ./scripts/run-cube.sh --fresh \
  --config runs/ecology-v1-calibration/selected/fast-leaf.toml
```

`/status`: build `0.1.0+77c42e8`, fresh (`resumed_from: null`), 24 founders,
20 Hz, shim sink with the viewer mirrored on port 7393; log
`runs/cube-eco-v1-fastleaf-2.log`. The opening snapshot decodes to schema 16,
config v8, seed 1, `fast-leaf` plant values, shipped `move_cost` 0.00036 (the
display world is not a movement-arm world). To compare shoulders on the
panels, restart with `CUBARIUM_FOLIAGE_FULL=0.85` (the shipped value); 1.0 is
vetoed by G and Astra (an ungrazed average stand shows bare wood). The
shoulder is an environment override, not persisted in the world and not in
`/status`: a restart without the variable silently returns to 0.85, so
screenshots and observations should record the effective shoulder (the
startup log line now states it). The physical cube was not inspected by
Fable; the viewer was.

## Review and repair (Astra, 2026-09-16)

Astra's review is
[ecology-v1-next-steps-review-2026-09-16.md](ecology-v1-next-steps-review-2026-09-16.md).
Disposition: keep the instrumentation, retained rows and presentation work;
repair F's ecological interpretation before using it to choose another
movement or diet change. Repair 1, all report-level except one comment and
one log line:

- **P1, F's glider claim.** "Both herbivore rigs miss their first brood" was
  contradicted by F's own census: `fast-leaf` at 0.0018 has 357 glider births.
  Corrected in F and here; the finer ladder now gates on the grazer.
- **P1, recovery.** "Never / absorbing / regrowth time, not pressure" reduced
  to "rare (13 crossings in 72 runs), mechanism unresolved"; post-depletion
  pressure named as the missing measurement.
- **P1, skimmer.** "Not the body / not the controller / supported reading is
  diet" reduced to a strong association among descendants; the matched
  form × diet factorial is the test.
- **P2, E's scope.** "Feasible, every survivor saturated" corrected to eleven
  of twelve layouts, with the `h2` death and the thin `h6` ending stated;
  "not because anything ate it" to "not principally"; the channel split
  scoped to generation 9 on these layouts.
- **P2, G's cue.** Named a dead-wood dominance cue that can show during
  dieback; the pixel-identical claim scoped.
- **P3, provenance.** Stale "no runtime setting" comment fixed; a startup log
  line states the effective shoulder when overridden.

## What this does and does not establish

- Established by measurement: the training body is feasible on eleven of
  twelve held-out patches under a competent scripted controller; the trained
  controller fails to feed; the apex never becomes eligible; range and
  depletion respond to the movement price; the skimmer starves on the
  generalist diet, and its foliage-diet descendants survive far better.
- Open: why recovery stays rare after depletion; whether diet or body
  explains the skimmer; whether an eligible apex would ever meet another.
- Not established: why generation 9 does not eat (intake, not travel, is
  localised; the three separable outcomes are named in E's note); why an
  introduced apex dies near 11,000 ticks; whether any price between the
  current one and five times it clears the brood gate; whether the depleted
  cells are the marginal ones (per-cell `L·μ` not recorded); anything about
  a whole-world neural population.
- The cube shows G's work and none of E's or F's. The display world is the
  legacy controller in `fast-leaf` at the shipped movement price; nothing in
  E or F changes a number the display produces. The soil-band cue will only
  appear once a stand below the horizon actually dies.

## Next recommendation

Reconciled with Astra's opinion (Fable accepts the reorder: movement before
apex, a controlled factorial instead of observational bins):

1. **Generation 9's intake diagnostic**, the single most informative cheap
   experiment now: generation 9 and the mobile script on the same 12 layouts,
   per tick, with the ledger on: edible stocks in the occupied cell and
   whether each is above `feed_min`; the three mouth efforts; requested,
   served, digestible and credited intake; the limiting clamp; the bill.
   Report fractions of ticks on edible food, effort-on while on food, and
   served/requested, plus credit/bill. Effort off on food points to
   action/score; effort on but no food to observation/navigation; effort on,
   food present, bite clamped to the action adapter or settlement. No
   training before this separates them.
2. **The finer price ladder** {0.00036, 0.0006, 0.0009, 0.0012, 0.0018}, arm
   0, gated on the founder grazer's first completed brood (glider reported
   separately), and for every depleted cell: `L·μ`, `P/P₀` through time,
   time and stock at the last consumer visit, post-depletion visits and
   served material, first recovery and re-depletion times. Recovery after
   grazing stops with adequate `L·μ` confirms a time-scale problem; continued
   bites explain pressure; no regrowth after pressure stops at adequate `L·μ`
   implicates the plant equation; marginal `L·μ` identifies poor cells.
3. **A controlled form × diet factorial**: matched tick-0 clones and
   locations, mutation and reproduction off; the founder skimmer genome at
   `diet` 0.60 and 0.85, then `diet` 0.85 across forms, with the ledger
   reporting served, digestible, credited, billed and terminal stores by
   channel. No `γ > 1` until both food channels are demonstrably usable and
   this is understood.
4. **Apex death and eligibility, radius untouched**: the ledger on an
   introduced adult through death (intake, gut credit, oxidation, upkeep,
   motor and combat bills, terminal stores, cause); then one eligibility
   intervention at a time, already-age-eligible introduction first. If
   readiness still never opens, stock or encounter terms bind; if it opens
   but no candidate pair forms, sensing and meeting are next; only then does
   the radius become a real choice.
5. **Wrysk's calls, with provenance visible**: the shoulder (0.85 or 0.95 on
   the panels; 1.0 vetoed); the soil cue's look (a dead-wood dominance stub
   cut from a mushroom now; an authored snag later or not).
