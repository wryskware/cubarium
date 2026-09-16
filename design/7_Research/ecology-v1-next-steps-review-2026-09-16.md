---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Ecology v1 next steps — independent finished-work review

**Disposition: keep the E/F/G instrumentation, retained rows and presentation work, but
repair F's ecological interpretation before using it to choose another movement or diet
change.** E answers the original feasibility question well, with one-layout and
window-selection qualifications: this body can sustain a competent scripted controller on
11 of 12 frozen layouts, while generation 9 fails to feed on all 12. The apex audit is also
decisive about the present arm: age eligibility is never reached, so radius cannot decide a
mating there. G's shoulder experiment is valid and 0.95 is a reasonable owner-facing trial.
F's pre-registration is honest and its price manipulation genuinely reduces range and raises
depletion, but three headline interpretations are too strong: `fast-leaf × 0.0018` gliders
did reproduce, recovery is rare rather than absent, and descendant skimmers do not isolate
body from diet. These are report-level scientific defects, not evidence that the new
diagnostics disturbed the simulation. Nothing here warrants interrupting the live cube.

Scope: I read the consolidated result, E, F and G reports and the three briefs in the
requested order; inspected the shoulder sheet; checked the retained 48 feasibility rows,
eight apex rows and selected aggregates over all 108 movement rows; reviewed the exact budget,
opportunity, movement, parameter-box and presenter seams; and inspected
`2041722..b18b6bc`. I ran the focused release tests: body-budget 9 passed / 1 timing test
ignored, search 116 passed, and `art_ecology` 24 passed. Lore's local daemon was registered
but unavailable, so retrieval fell back to Graft, the named files, Git and the retained rows.

## Findings

### 1. P1 — F's “both herbivore rigs miss their first brood” claim is contradicted by F's census

The pre-registration itself is honest. It was committed at `3934eb7` before the definition
tests, implementation and results (`ecology-v1-movement-2026-09-16.md:37-41,215-225`), and
the four verdicts were applied in their declared order. F also disclosed that the baseline
control already cleared its depletion threshold, leaving recovery as the discriminating
conjunct (`:152-183,347-367`). That disclosure is correct and does not invalidate the rows.

The subsequent brood interpretation does not check. F says that at every raised price all
180 grazer and 90 glider founders per configuration produce zero offspring, and therefore
that both herbivore rigs die before a first brood (`:401-415`; repeated in
`ecology-v1-next-steps-results-2026-09-16.md:134-145`). But the report's own pooled table
shows `fast-leaf × 0.0018`, form 1, with **447 entered**, **346 deaths** and **101 alive**
(`ecology-v1-movement-2026-09-16.md:459-464`). The 108 retained rows resolve that identity:

| cell checked in `runs/ecology-v1-movement/matrix/evals.jsonl` | opening form-1 | form-1 births | deaths | alive |
| --- | ---: | ---: | ---: | ---: |
| baseline, 0.0018 | 90 | 0 | 90 | 0 |
| baseline, 0.006 | 90 | 0 | 90 | 0 |
| `fast-leaf`, 0.0018 | 90 | **357** | 346 | **101** |
| `fast-leaf`, 0.006 | 90 | 0 | 90 | 0 |

Form is immutable and the census assigns it at birth (`ecology-v1-movement-2026-09-16.md:
125-146`), so those 357 form-1 births require the glider lineage to have crossed conception
and completed gestation. The grazer statement is supported—form 0 has 180 opening bodies and
zero births in all four raised cells—but the glider statement and “burrower monoculture”
generalisation are false for `fast-leaf × 0.0018`. Nor can last-observed terminal stores prove
that every founder never reached `bud_reserve`; ordinary conception escrows the child once the
age, controller and funding gates pass (`crates/cubarium-core/src/world/step.rs:2192-2239`),
and F did not record founder-specific escrow histories.

This changes the proposed stop condition. A finer ladder should gate explicitly on the
**grazer's** first completed brood and report the glider's separately; “first herbivore brood”
is already satisfied at one tested raised price.

### 2. P1 — Recovery fails the registered criterion, but it is neither absent nor diagnosed as “regrowth time, not pressure”

The registered D clause fails in every raised-price cell, exactly as reported: no cell reaches
one mean recovery per run with at least 12 of 18 runs recovering
(`ecology-v1-movement-2026-09-16.md:164-178,347-359`). The stronger prose does not follow.
F and the consolidated disposition call recovery absent or an absorbing state and assign the
cause to rebuilding time rather than grazing pressure (`ecology-v1-movement-2026-09-16.md:
386-400`; `ecology-v1-next-steps-results-2026-09-16.md:27-31,126-133`). The raw counters
contain recovery:

| configuration / price | depletions | recoveries | runs with recovery | cells ever depleted | depleted at end |
| --- | ---: | ---: | ---: | ---: | ---: |
| baseline / 0.00036 | 263 | 3 | 3 | 262 | 260 |
| baseline / 0.0018 | 1,006 | **6** | **6** | 1,003 | 1,000 |
| baseline / 0.006 | 822 | **5** | **5** | 820 | 817 |
| `fast-leaf` / 0.00036 | 13 | 0 | 0 | 13 | 13 |
| `fast-leaf` / 0.0018 | 219 | **2** | **2** | 217 | 217 |
| `fast-leaf` / 0.006 | 402 | 0 | 0 | 402 | 402 |

Those are 13 recovery crossings in the raised arms, not enough for the registered ecological
effect but enough to refute “never” and “absorbing.” Equality or near-equality between cells
ever depleted and cells depleted at the horizon does not repair the inference: a cell can
recover and be depleted again, as the `fast-leaf × 0.0018` seed-1006 arms do.

The alternatives the question names remain live. The 90% lifetime rule applies to the body
spatial summaries, not to the whole-run crossing counter (`ecology-v1-movement-2026-09-16.md:
77-108,329-337`), so it does not create this result. But the counter watches only cells with
opening foliage and requires recovery above 50% of each cell's opening value; whole-world
greening can occur in other watched cells or in the 168 initially unwatched cells. More
importantly, F did not record post-depletion visits or bites. A cell can stay below 50%
because `L·μ` makes recovery slow, because consumers keep returning, or both. F explicitly
admits that per-cell `L·μ` and trajectories were not recorded (`:396-400`), but omits the
equally necessary post-depletion grazing pressure. The supported conclusion is: **price
concentrates grazing and creates many more depletion crossings; robust repeated recovery was
not produced within 150 minutes, and why is unresolved.**

### 3. P1 — Skimmer starvation is measured; “diet, not body/controller” is not isolated

The solid part is substantial: nearly every founder-bin skimmer death is starvation—252 of
259 in the baseline control and 233 of 235 in `fast-leaf`—with empty usable stores
(`ecology-v1-movement-2026-09-16.md:448-475,483-488`). The association at
`fast-leaf × 0.0018` is also real: form-3 bodies in the 0.65–1.0 diet bin show 84% survival
against 20% in the 0.35–0.65 bin (`:493-506`). It is useful evidence for testing diet yield.

It does not exonerate the body or establish diet as the cause. Every foliage-bin skimmer is a
descendant, not a randomized version of the founder skimmer. Of 75 entrants, only 12 die and
63 are still alive at the horizon; the comparison therefore mixes later birth, right
censoring, selection into the mutant lineage and possible mutation at other loci. A lifetime
mean over the 12 deaths does not cancel those differences. The legacy-controller algorithm is
shared, but its realised behaviour still depends on inherited drives, body capabilities and
habitat. F itself says habitat and per-body channel yield were not measured
(`ecology-v1-movement-2026-09-16.md:508-541`). The consolidated result nevertheless upgrades
the association to “not the body” and “supported reading is lower realised yield”
(`ecology-v1-next-steps-results-2026-09-16.md:147-157`).

The clean test is a matched factorial, not another observational bin summary: clone a founder
skimmer at the same birth tick and locations with only `diet` changed from 0.60 to 0.85, then
cross the same fixed diet over forms, with mutation and reproduction off. Use E's ledger to
measure served, digestible, credited and billed energy. A birth-time-matched whole-world
hazard is a useful secondary check, but it cannot by itself remove genotype and habitat
confounding.

### 4. P2 — E's accounting is fit for this fixture, but “feasible/sufficient” needs an 11-of-12 scope

The ratio is the right diagnostic for the adult, no-growth, `bud = false` fixture. It values
food as direct battery credit plus the oxidisable value of material credited to reserve,
`battery_credit + η_ox·e_r·reserve_credit`, and divides differences of cumulative totals by
the complete bill rather than averaging per-tick ratios
(`crates/cubarium-search/src/es/budget.rs:188-191,213-238`; report
`ecology-v1-budget-2026-09-16.md:122-148`). That is potential intake yield, not realised
oxidation, which is exactly what asks whether a body *could* fund the bill while feeding. It
would not be a generic fitness ratio where credited reserve is later diverted into growth or
offspring, but those terms are zero here and are separately ledgered. Best-window for a
survivor and trailing-window for a dying body are sensible descriptive choices; neither
should replace survival and terminal stores.

The matched result supports a narrower branch conclusion. Generation 9 dies on 12 of 12 and
the mobile script survives 11 of 12 while travelling less and serving about fifteen times as
much material (`ecology-v1-budget-2026-09-16.md:155-171,197-218`). Thus the body/fixture is
feasible under a competent controller on **11 layouts**, and the selected controller's intake
behaviour binds there. It is not universal feasibility or universal sufficiency. In retained
row `mobile-script/h2-holdout`, the control dies at tick 26,355 despite a best-window ratio of
1.788; in `mobile-script/h6-holdout` it reaches the horizon with reserve 0 and a trailing ratio
of 0.234. Those rows also contradict the report's statement that every surviving mobile body
ends at `R_max` with a trailing ratio of 1 (`ecology-v1-budget-2026-09-16.md:141-148,190-195`).
The best ratio proves a profitable interval, not that every patch supports the body forever.

E's apex conclusion is sound. `max_ready = 0`, the oldest life is 14,101 ticks against the
24,000-tick age gate, and all eight rows match the re-derived screen candidate. Age alone is
sufficient to explain why eligibility never opens. Zero `pair_candidates` is also independent
evidence about encounter: that counter is formed before readiness failure and requires both
perched adults to be mutually sensed (`crates/cubarium-core/src/encounter.rs:465-475`), yet it
stays zero through 5,244–5,477 ticks with two mature-perched members. Lowering age therefore
would not guarantee mating either. E is right that radius is not yet an owner choice
(`ecology-v1-budget-2026-09-16.md:231-268`).

The route correction is sound in direction, not literally absolute. The initial centre serves
a median 0.31 m while route foliage falls about 20 m, and all four drivers see a similar large
background decline, so fixture relaxation is the dominant term (`ecology-v1-budget-2026-09-16.md:
220-229`). “Not because anything ate it” should read “not principally”; grazing contributes a
smaller amount. The 44/28/28 split checks exactly for generation 9 (4.183/2.598/2.652 m), but
the initial centre is about 50/26/23. It is a controller-on-these-layouts measurement, not a
general signature of every `diet = 0.7` body.

### 5. P2 — G's soil snag respects the presentation boundary, but it is not the brief's `W = 0` cue

The mapping is presentation-only and reads actual stocks, so it does not alter or conceal the
simulation state. Keeping `D + C` as the independent soil scenery and stamping a small ash cue
over it is a defensible presentation choice: the overlay may cover a few mushroom pixels, but
it does not fold `Wd` into litter or add analytical UI. That respects the boundary's purpose
(`design/ecology-v1-contract.md:604-619`).

Its semantics need honest naming. The brief requested a mark when `Wd > 0` and `W = 0`
(`design/handoffs/ecology-v1-presentation-2-opus-2026-09-16.md:57-60`). The implemented
`clamp(Wd_density - W_density, 0, 1)` appears as soon as dead density exceeds living density,
while `W` is still positive (`crates/cubarium/src/art_present/habitat.rs:699-726`). G's own
dieback test deliberately exercises that mixed-stock interval
(`crates/cubarium/tests/art_ecology.rs:920-957`). It is therefore a continuous **dead-wood
dominance cue**, not an exact realization of “the stand is dead,” and the claims that every
living cell is pixel-identical or that the rule is exactly the brief at all relevant states
are too strong (`ecology-v1-presentation-2-2026-09-16.md:148-182`).

I would not veto the cue on ecological grounds—it displays real `Wd`, and the continuous
arrival avoids a conspicuous pop—but I would tell Wrysk plainly that it can show during
dieback and that its current silhouette is a cut mushroom. Veto the *description*, not
necessarily the mapping. Whether that reads as a snag on the panels is an owner-facing visual
choice.

### 6. P3 — The shoulder experiment is valid and answers the earlier presentation finding

The experiment draws the same `RenderView` through two otherwise identical presenters, changes
only the shoulder, and compares encoded 8-bit frames (`crates/cubarium/examples/shoulder_sheet.rs:
260-288`). That correctly isolates when the shoulder changes a pixel the cube can show. The
separate comparison against an undepleted view establishes that other cell layers respond
earlier (`:291-313`). The sheet is consistent with the table and makes the difference between
the controlled ladder and the much faster real grazing trajectory visible.

The clamp arithmetic checks. With opening `P/W = 1.217`, reaching the fullness clamp at 1
requires `1 - 1/1.217 = 17.8%` foliage loss; reaching shoulder 0.85 requires about 30.2%.
Thus 17.9 of those 30.2 percentage points are caused by over-leafing/clamping, not by the
0.85 shoulder (`ecology-v1-presentation-2-2026-09-16.md:56-83`). The original finding was
right about the plant sprite and overbroad about the cell: ground cover, wash and flecks expose
an encoded change at 0.51% loss. G answers finding 7 rather than evading it.

I agree with G's veto of 1.0: it shows bare wood on the ungrazed average-light fixture. I would
not veto 0.95; the measured average-light change is only `mix = 0.001`, flicker remains within
the existing bound, and Wrysk can compare it with the shipped 0.85 on the physical panels
(`ecology-v1-presentation-2-2026-09-16.md:102-116`).

### 7. P3 — The merge seams are consistent; deployment provenance has one operational caveat

I found no E/F integration mismatch. F appends `organism.move_cost` as parameter 14 with the
shipped value as its default (`crates/cubarium-search/src/params.rs:47-62,235-250`); it does
not change the `WorldConfig` or ecology hash at the control price. The 36 control state hashes
match A. E's apex audit derives each seed from the loaded ecology and checks it against the
currently declared candidate (`crates/cubarium-search/src/apex_audit.rs:138-150`); all eight
retained rows say `matches_screen_candidate: true` after the merge. The ES policy ecology hash
identifies `WorldConfig`, not the width of the calibration search vector, so adding a search
axis does not invalidate the generation-9 policy at the default price.

Refusing A's old 13-component replay rows is the correct call. Exact parameter bits are the
replay identity, and `from_bit_labels` now requires the current 14-name length
(`crates/cubarium-search/src/params.rs:470-493`). Silent padding would reinterpret an old
record under a new protocol. Historical rows remain reproducible by their writer build; the
current build's matched re-run supplies the cross-version comparison.

The focused merged-tree tests pass as stated above. One stale source comment remains:
`foliage_ramp_at` says there is no runtime setting and that the display always uses 0.85 even
though `FOLIAGE_FULL_ENV` is now read once per process
(`crates/cubarium/src/art_present/habitat.rs:95-123,762-773`). More importantly for the owner,
build `77c42e8` does not by itself identify the live visual mapping: 0.95 is an environment
override, is not persisted in the world, and is not reported by `/status`. A restart without
the variable silently returns to 0.85. That is acceptable for this explicit viewing trial,
but observations and screenshots should record the effective shoulder. The display otherwise
runs the intended fresh `fast-leaf` world at shipped movement price; E and F diagnostics do
not change it (`ecology-v1-next-steps-results-2026-09-16.md:228-265`).

## Next steps: Astra's opinion

Fable's first experiment is right; I would move the movement follow-up ahead of apex, replace
the observational diet follow-up with a controlled comparison, and make the F report repair a
prerequisite rather than spending compute on top of a false summary.

1. **Correct the retained F/consolidated claims, then run generation 9's intake diagnostic.**
   The report correction needs no simulation: retain the pre-registered PARTIAL verdicts, but
   state 13 raised-arm recovery crossings, the unresolved recovery mechanism, and the 357
   `fast-leaf × 0.0018` glider births. Then instrument generation 9 and the mobile script on
   the same 12 layouts, per tick, with: edible stocks in the occupied cell and whether each is
   above `feed_min`; the three mouth efforts; requested, served, digestible and credited intake;
   the limiting clamp; and bill. Report fractions of ticks **on edible food**, **effort on while
   on food**, and **served/requested**, plus credit/bill. This is still the **single most
   informative cheap experiment now**. Effort off on food points to action/score; effort on but
   no food points to observation/navigation; effort on, food present and a clamped bite points
   to the action adapter or settlement. Do no further training before this separates them.

2. **Run the finer price ladder, but gate it on the grazer and measure pressure after
   depletion.** Use Fable's `{0.00036, 0.0006, 0.0009, 0.0012, 0.0018}`, arm 0 design. Report
   first completed brood separately for founder grazers and gliders; require at least a grazer
   brood before interpreting a cell as a viable spatial intervention. For every depleted cell,
   record `L·μ`, `P/P₀` through time, time and stock at the last consumer visit, post-depletion
   visit count, served material after depletion, and the first recovery/re-depletion times.
   Recovery after grazing stops with adequate `L·μ` confirms a time-scale problem; continued
   bites explain pressure; no regrowth after pressure stops at adequate `L·μ` implicates the
   plant equation/parameters; marginal `L·μ` identifies selection of intrinsically poor cells.

3. **Measure diet yield with a controlled form × diet factorial, not bins of descendants.**
   Use matched tick-0 clones and locations, mutation and reproduction off. First compare the
   founder skimmer genome at `diet = 0.60` and `0.85`; then hold `diet = 0.85` across forms.
   E's ledger should report lifetime served material by channel, digestible material, energy
   credit, bill and terminal stores. If changing only diet rescues the skimmer, the yield
   hypothesis is confirmed; if form remains important at fixed diet, the body is not
   exonerated. Do not run `γ > 1` until both food channels are demonstrably usable and this
   factorial is understood.

4. **Then diagnose apex death and eligibility; leave radius untouched.** I agree with using
   the ledger on an introduced adult through death: record intake/gut credit, oxidation,
   upkeep/motor/combat bills, terminal stores and death cause. After that, test one eligibility
   intervention at a time. Already-age-eligible introduction is the cleanest first probe: if
   readiness still never opens, stock/encounter terms—not minimum age—bind; if it opens but
   `pair_candidates` remains zero, sensing/meeting is next. A radius choice becomes meaningful
   only after ready candidates exist and fail distance.

5. **Let Wrysk decide the two presentation questions with provenance visible.** Compare 0.85
   and 0.95 on the panels; keep 1.0 vetoed. For the soil cue, explain that the current mark is
   a mixed-stock dead-wood-dominance stub made from a cut mushroom, not a strict `W = 0` snag.
   If retained beyond this trial, report the effective shoulder in status/log output and fix
   the stale “no runtime setting” source comment. No ecological rerun is needed for either
   visual choice.

## What I did not check

I did not rerun `es-budget`, `apex-audit` or any movement row; regenerate the shoulder sheet;
run the ignored timing benchmark; run the full host, renderer or all-target core suites; audit
every changed line in the 7,105-line diff; inspect the deployment log, `/status`, state files,
viewer or physical LEDs; or verify the live process. I did not inspect every value in every
movement row, although the birth, crossing and census aggregates cited above were computed
across all 108 retained rows. I did not touch `state/`, port 7393, the shim or the running
cube.
