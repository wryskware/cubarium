---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# The optimiser is doing its job: nothing cancels, nothing is erased, and the variation it has

> **Correction after Astra's round-4 review (P1).** Three claims below are
> stronger than the measurement. (1) The exact replay and the retained ÷
> orthogonal-reference ratio establish *faithful execution* and *no excess
> geometric cancellation*; they do not rule out estimator variance, the pair
> count, the centred-rank reduction or the optimiser generally — no
> split-half or bootstrap gradient-direction stability, repeated training
> seed or alternative pair count was measured. "Refuted as an optimiser/update
> problem" and "the rank reduction and pair count are ruled out" are
> withdrawn to "candidate variation is real, useful residence is still small,
> the recorded update is reproduced exactly, and correlated-direction
> cancellation is not the loss". (2) The residence gap is about **18×**
> (96.9 % / 5.5 %), not two orders of magnitude; the "580 generations" figure
> is an extrapolated rate, not a measured scale. (3) The deadband finding is a
> **measured clipping site and the leading adapter hypothesis**, not the
> established place where weights lose their effect: carried deadband
> occupancy correlates only r = 0.21 with score and no trajectory was run
> under a different turn band. The four-layout minimum's 29 % disagreement
> with the mean is a design judgement (worst-layout robustness is what
> `Min` asks for), not a defect. The bounded next change Astra proposes:
> `TURN_DEADBAND` 0.05 → 0.0 only, with `Aggregate::Min`, σ, pair count,
> score, layouts and seed fixed and a new protocol hash; first replay the
> frozen centre and candidates on their own layouts under both adapters
> (falsified as the bottleneck if turn activity rises but on-food fraction,
> dwell and `t_min` do not); and, cheaply, bootstrap or split the retained
> sixteen pair contributions for gradient-direction stability before calling
> sixteen pairs sufficient.
# to work with is two orders of magnitude too small

Workstream Q of the ecology v1 round-4 next steps
([brief](../handoffs/ecology-v1-es-antithetic-opus-2026-09-16.md)), item 2 of the reconciled
round-3 next steps, from Astra's [round-3 review](ecology-v1-round3-review-2026-09-16.md) (P1 on
[L](ecology-v1-score-checks-2026-09-16.md)).

L left the question at: the perturbations *do* produce material score and feeding variation, so
where is the useful part lost — in the four-layout minimum, in the centred-rank reduction, in the
update, or in the mapping from weights to residence? This workstream answers it from the retained
rows, without simulating a single training episode.

**Astra's rule is refuted, with one qualification.** No pair pointing the right way is cancelled
in the centred-rank gradient (the summed contribution retains **1.001×** what mutually orthogonal
directions would retain, mean over all sixteen generations, range 0.983–1.014), and nothing is erased by the update (the
replay of all sixteen updates from the run's own first centre lands on the run's checkpoint with a
maximum per-parameter disagreement of **exactly 0**). Nor is the residence branch's refutation
met: better-scoring candidates *do* feed and reside more, in **75.7 %** of the 255 informative
pairs on intake per lived tick and **62.0 %** on opening residence. What the numbers say instead
is that the variation is **small at source**: across generation 9's 32 candidates the whole range
of opening residence is **0.8 % to 5.5 %** of life, against the **96.9 %** a scripted
route-follower holds, and the population's mean score climbs **6,896 → 7,655** ticks over sixteen
generations — 11 % of the way, at ~50 ticks a generation, against a gap of about 29,000.

The one qualification, measured: **the four-layout minimum inverts the estimator's preference on
29 % of pairs** (74 of 255), and on 27 of those the discarded member had won three or four layouts
of four.

## Build and provenance

- Definitions and tests, before the implementation: **`da2bdc8`**. Implementation and the two
  experiments: **`214df60`**. This note: the commit that carries it.
- Search build id **`214df60`**, pinned with `CUBARIUM_SEARCH_BUILD` (workers share one `target/`;
  a stale `cubarium-core` rmeta from another worktree broke one build during this workstream and
  was cleared with `touch crates/cubarium-core/src/lib.rs`, as the brief says).
- Run read: `runs/es-eco-v1-fastleaf/`, protocol hash `8e51a1a9b1e2742b`, train seed `20260915`,
  σ = 0.02, lr = 0.01, aggregate `min`, 16 pairs, horizon 36,000, layouts `t1-corridor`,
  `t2-weak-open`, `t3-scatter`, `t4-ring`, ecology `fast-leaf` (`09e244392ec91768`).
- Outputs under `runs/ecology-v1-es-antithetic/` (git-ignored by design): `pairs.json` 228 KiB,
  `deadband.json` 18 KiB — **0.24 MiB** against the brief's 20 MiB cap.
- Wall time: **0.15 s** for deliverable 1 (no simulation) and **1.2 s** for deliverable 2's twelve
  episodes on 8 workers, against the brief's 2- and 3-minute caps.
- `cargo test -p cubarium-search`: **230 passed, 0 failed, 5 ignored** (H's experiment, L's two,
  and this workstream's two). `cargo test -p cubarium-core`: **517 passed, 0 failed, 4 ignored**;
  no core file is touched by this workstream.

### Exact commands

```bash
cargo test -p cubarium-search --lib antithetic
cargo test -p cubarium-search --test es_antithetic

CUBARIUM_SEARCH_BUILD=214df60 \
  cargo test -p cubarium-search --release --test es_antithetic -- --ignored --nocapture
```

Both experiments are `#[ignore]`d tests rather than `es-*` subcommands, for the reason H and L
recorded: other workers hold `crates/cubarium-search/src/main.rs` open in worktrees, and a
subcommand is the one edit that would have collided. `antithetic::run_reduction` and
`antithetic::run_deadband` are public, so promoting them is a one-liner.

### Why the numbers can be trusted

Nothing in `es::antithetic` re-implements any part of the trainer. The candidate scores are
re-derived with `trainer::score_by` and a report whose episodes disagree with the scores it
carries is **refused by name** (they agree exactly, on all 512 candidate-layout cells of all
sixteen generations); the utilities are `optimizer::centered_rank_utilities`; the gradient is
`optimizer::gradient`; the perturbations are `rng::perturbation`; the ascent is `Adam::ascend`;
the deadband predicate is `Action7::squash`'s own exact zero. Three independent checks say the
module is reading the run that happened:

1. **The replay is the run.** Starting from `centers/center-00000.json` — which is also exactly
   `tensor::initial_center(20260915)` — and using only the retained reports, the sixteen updates
   reproduce `checkpoint.json`'s centre with `max |Δθ| = 0`.
2. **The recomputed gradient norm equals the recorded one** in every generation, to the printed
   digit (213.2, 265.6, 317.4, …), so the pairing, the seeds and the sign convention are the
   trainer's.
3. **Astra's generation-9 figures reproduce**: span 6,459–8,915, sd **643** (population divisor;
   the sample sd is 653), r = **0.812** between candidate score and mean `intake_producer`, r =
   **0.544** with mean `ticks_in_opening`.

The observation set in deliverable 2 reproduces L's maximum reconstruction residual to the last
digit — **1.842313095327952 × 10⁻⁵** — which is how it is known to be L's set and not a new one.

## Deliverable 1 — the antithetic-pair reduction

`cnc-i`/`cnc-r`/`cnc-o` count pairs whose *preferred* member (the sign of `w`, not the sign of the
pair) also has more total intake, more intake per lived tick, and more opening residence as a
fraction of life. `mask` counts pairs where the minimum and the mean disagree about which member
is better. `retain` is `‖Σ wᵢεᵢ‖ / Σ|wᵢ|‖εᵢ‖`; `excess` is the same against the mutually
orthogonal reference `sqrt(Σ wᵢ²‖εᵢ‖²)`. `prog` is the applied step's component along the
generation's best candidate, as a fraction of the distance to it.

| gen | min | max | sd | centre | best | next centre | cnc-i | cnc-r | cnc-o | mask | retain | excess | prog |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 6478 | 7997 | 398 | 6521 | 7997 | 6948 | 10/16 | 11/16 | 8/16 | 6 | 0.307 | 1.003 | 0.023 |
| 1 | 6437 | 8262 | 450 | 6948 | 8262 | 6511 | 12/16 | 12/16 | 10/16 | 3 | 0.277 | 0.983 | 0.055 |
| 2 | 6406 | 7998 | 405 | 6511 | 7998 | 7852 | 10/16 | 12/16 | 6/16 | 5 | 0.297 | 0.990 | 0.043 |
| 3 | 6377 | 7864 | 459 | 7852 | 7864 | 8000 | 11/16 | 14/16 | 9/16 | 5 | 0.273 | 1.000 | 0.051 |
| 4 | 6424 | 8151 | 526 | 8000 | 8151 | 7929 | 12/16 | 12/16 | 11/16 | 5 | 0.311 | 1.008 | 0.028 |
| 5 | 6505 | 7910 | 435 | 7929 | 7910 | 7688 | 12/16 | 12/16 | 10/16 | 4 | 0.292 | 1.005 | 0.051 |
| 6 | 6443 | 8918 | 567 | 7688 | 8918 | 8227 | 13/16 | 13/16 | 10/16 | 3 | 0.301 | 0.996 | 0.013 |
| 7 | 6456 | 8681 | 577 | 8227 | 8681 | 8041 | 16/16 | 16/16 | 13/16 | 0 | 0.295 | 1.003 | 0.004 |
| 8 | 6460 | 8426 | 549 | 8041 | 8426 | 8703 | 9/16 | 11/16 | 11/16 | 8 | 0.314 | 1.006 | 0.011 |
| 9 | 6459 | 8915 | 643 | 8703 | 8915 | 8108 | 15/16 | 15/16 | 12/16 | 2 | 0.309 | 1.014 | 0.022 |
| 10 | 6470 | 8904 | 605 | 8108 | 8904 | 7761 | 9/15 | 10/15 | 11/15 | 6 | 0.334 | 1.007 | 0.016 |
| 11 | 6484 | 8625 | 557 | 7761 | 8625 | 7596 | 12/16 | 12/16 | 9/16 | 5 | 0.345 | 1.001 | 0.008 |
| 12 | 6400 | 10276 | 834 | 7596 | 10276 | 7402 | 11/16 | 11/16 | 8/16 | 5 | 0.289 | 0.992 | 0.007 |
| 13 | 6501 | 8458 | 544 | 7402 | 8458 | 7948 | 10/16 | 10/16 | 8/16 | 6 | 0.299 | 1.004 | 0.019 |
| 14 | 6457 | 9689 | 711 | 7948 | 9689 | 7110 | 14/16 | 13/16 | 12/16 | 2 | 0.317 | 1.012 | 0.028 |
| 15 | 6473 | 9045 | 757 | 7110 | 9045 | — | 7/16 | 9/16 | 10/16 | 9 | 0.340 | 0.999 | 0.034 |

Generation 10 has one exactly-tied pair, whose weight is exactly zero; it contributes nothing to
the update and is counted as evidence for nothing, which is why its denominators read 15.

Pooled over all sixteen generations, 255 informative pairs:

| question | answer |
| --- | --- |
| preferred member also has more total producer intake | **183 / 255 = 71.8 %** |
| …more producer intake **per lived tick** | **193 / 255 = 75.7 %** |
| …more opening residence | 162 / 255 = 63.5 % |
| …more opening residence **as a fraction of life** | **158 / 255 = 62.0 %** |
| minimum and mean disagree about the better member | **74 / 255 = 29.0 %** |
| of those, the discarded member had won 3 or 4 layouts of 4 | **27** |
| mean survival the masked pairs were worth, mean-over-layouts | **683 ticks** |

Generation 9's pair table in full, which is the one Astra's figures are about. `Δticks` is per
layout in plan order (`t1`, `t2`, `t3`, `t4`); `bind` names the layout that set each member's
`t_min`.

| pair | score + | score − | Δscore | Δticks per layout | wins | bind +/− | Δintake/tick | Δopening frac | w | masked |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 6904 | 6529 | +375 | +82 +508 +904 −1068 | 3/4 | t4/t3 | +6.6e−06 | +0.0045 | +0.129 | |
| 1 | 8915 | 7425 | +1490 | +3057 +1227 +1245 +1181 | 4/4 | t2/t1 | +2.3e−05 | +0.0093 | +0.452 | |
| 2 | 7530 | 7195 | +335 | −185 +977 +5338 +189 | 3/4 | t1/t2 | +1.6e−05 | −0.0106 | +0.194 | |
| 3 | 6933 | 7879 | −946 | −792 −1194 −325 +95 | 1/4 | t2/t3 | −9.0e−06 | −0.0114 | −0.484 | |
| 4 | 8649 | 7310 | +1339 | +2649 +520 −1029 +457 | 3/4 | t2/t1 | +5.9e−06 | −0.0030 | +0.452 | |
| 5 | 6739 | 6459 | +280 | +682 +280 −2886 +62 | 3/4 | t2/t2 | −7.5e−06 | +0.0068 | +0.161 | **yes** |
| 6 | 7243 | 7904 | −661 | +690 −1829 +1224 −927 | 2/4 | t2/t1 | −3.1e−06 | −0.0022 | −0.355 | |
| 7 | 6787 | 7684 | −897 | +36 −1761 −467 −1899 | 1/4 | t2/t1 | −1.5e−05 | −0.0055 | −0.516 | |
| 8 | 7902 | 7512 | +390 | +2328 +1195 +2798 −262 | 3/4 | t4/t2 | +1.9e−05 | +0.0083 | +0.226 | |
| 9 | 7179 | 6822 | +357 | +1095 −557 +241 +13 | 3/4 | t2/t1 | +3.7e−06 | +0.0149 | +0.161 | |
| 10 | 8036 | 7777 | +259 | −820 +579 +2507 −138 | 2/4 | t1/t2 | +6.6e−06 | −0.0031 | +0.129 | |
| 11 | 7211 | 7174 | +37 | +787 −390 +2391 +122 | 3/4 | t2/t3 | +9.7e−06 | +0.0174 | +0.097 | |
| 12 | 6939 | 6481 | +458 | −560 +274 +811 +810 | 3/4 | t2/t3 | +7.7e−06 | −0.0035 | +0.290 | |
| 13 | 7588 | 7668 | −80 | −131 +767 +514 −921 | 2/4 | t4/t2 | −3.4e−06 | −0.0134 | −0.032 | **yes** |
| 14 | 6517 | 6487 | +30 | −643 −371 +3457 −185 | 1/4 | t2/t3 | +1.6e−05 | +0.0381 | +0.032 | |
| 15 | 8262 | 8406 | −144 | −637 +568 −332 −309 | 1/4 | t3/t2 | −6.3e−06 | −0.0127 | −0.032 | |

### (a) Is the four-layout minimum hiding gains on three layouts? Yes, on 29 % of pairs

The pair table above is the mechanism in one row: pair 2's plus member lives **5,338 ticks longer
on `t3-scatter`** and 977 longer on `t2-weak-open`, and the whole of that is reduced to `+335`
because both members are bound by a different layout. Pair 5 is the sign inversion: the plus
member wins three layouts of four, loses 2,886 ticks on `t3-scatter`, and is therefore ranked
*below* a member it beats on average by 460 ticks. Pooled, 74 of 255 informative pairs have the
minimum and the mean disagreeing about which member is better, the masked pairs are worth **683
ticks** of mean survival each, and in 27 of them the member the estimator was pushed away from had
won three or four layouts of four.

The rank transform then destroys the rest of the magnitude by design. Pair 1 is worth `+1,490`
ticks and pair 11 `+37`; their weights are `+0.452` and `+0.097`, a ratio of 4.7 against a score
ratio of 40. That is what a centred rank is *for* — it is not a defect — but it is where the
remaining 40× of scale goes.

### (b) Is the minimum dominated by one layout? No, but it is concentrated

Over all 512 candidates of all sixteen generations, the layout that set `t_min` was
`t2-weak-open` **171** times (33.4 %), `t1-corridor` **153** (29.9 %), `t3-scatter` **107**
(20.9 %) and `t4-ring` **81** (15.8 %). No layout binds even a third of the time, so the score is
not secretly a one-layout score; but the two hardest layouts carry 63 % of it against the 50 % a
uniform binding would give.

### (c) How much of the summed update is cancelled between pairs? None, beyond geometry

`retain` sits at 0.27–0.35, which looks like heavy cancellation and is not: sixteen independent
Gaussian directions in 10,215 dimensions are very nearly mutually orthogonal, and orthogonal
vectors sum to `sqrt(Σ wᵢ²‖εᵢ‖²)` rather than `Σ|wᵢ|‖εᵢ‖` whatever their signs. Measured against
that reference the summed contribution retains **0.983–1.014**, mean **1.001**, in every one of
the sixteen generations. Pairs pointing "in opposite directions" is not a thing that happens here:
in 10,215 dimensions there are no opposite directions to point in, and a pair's whole contribution
survives the sum. **Cancellation in the centred-rank gradient is refuted as a mechanism.**

### (d) Is the update moving the centre toward the best candidates, or past them?

Neither, and not by a defect. Per generation:

- The applied step is **1.011** long at generation 0 and decays to **0.265** by generation 15,
  against a perturbation radius `σ‖ε‖` of **2.02**. Adam with a single gradient sample per step
  gives every coordinate almost exactly `±lr`, so the first step's RMS is `0.00999999` — the
  learning rate itself — and the decay to `0.00262` is `lr/√t`, which is what Adam does when
  successive gradients are near-independent.
- The step's component along the generation's best candidate is **0.4 % to 5.5 %** of the distance
  to it. The distance to the best candidate after the update is **0.97–1.10×** what it was before:
  in the best generation the centre closes 3 % of the gap, in generation 0 it moves 10 % further
  away. Averaged over the pairs' own preferred directions the progress is **8.3 %** of a σ at
  generation 0 and **1.0–1.3 %** from generation 8 on.
- Over sixteen generations the centre travels a path of **6.81** and ends **4.75** from where it
  started (ratio 0.70, so it is not a random walk — it is going somewhere), which is 2.35
  perturbation radii in sixteen steps.

So the update does not overshoot and it does not stall; it takes small, consistent steps. And it
works: **the centre sits in the top quartile of its own population in nine consecutive generations**
(3 through 11; rank 31/32 at generations 3, 8 and 9, and 32/32 at generation 5), which is what a
correctly centred ES looks like. The centre's score is below the generation's best candidate at 13 of 15
transitions, by 883 ticks on average — but that is the ordinary fact that the best of 32 draws
beats the mean, not evidence of a failure to move.

What *is* damning is the rate. The population's mean score climbs **6,896 → 7,655** over sixteen
generations, ~50 ticks a generation, and the centre's own score climbs at 46 ticks a generation
with ±1,200 of noise on top (6,521 → 8,703 at generation 9 → 7,110 at generation 15). A scripted
route-follower scores 36,000 on these four layouts ([L](ecology-v1-score-checks-2026-09-16.md)).
At this rate the gap is about 580 generations wide.

### (e) Do better candidates have better residence? Yes — and it is tiny

Deliverable 3 was **not run**, because deliverable 1 answers its question. `intake_producer` is
material that actually left `P` through the mouth: collecting it *requires* standing on a fed cell
with the mouth open, so it is a strictly stronger residence signal than a cell-set occupancy
count, and it is in every retained episode. The preferred member of a pair has more of it per
lived tick in **75.7 %** of 255 pairs; within generation 9 the correlation between candidate score
and mean intake per tick is **r = 0.859** (against 0.812 on the un-normalised total, so the
correlation is not an artefact of longer lives), and with opening residence as a fraction of life
**r = 0.436**.

The size is the finding. Generation 9's 32 candidates span an opening-residence fraction of
**0.0076 to 0.0548** and an intake rate of **2 × 10⁻⁵ to 6 × 10⁻⁵** m/tick. The behaviour the
score would pay **12,510 ticks on the training four** for ([L](ecology-v1-score-checks-2026-09-16.md))
is 96.9 % on-food residence. A σ = 0.02 perturbation of these
weights reaches about **5 %** of that, and the best candidate in sixteen generations reaches 5.5 %.

## Deliverable 2 — deadband occupancy under a σ-scale perturbation

Generation 9's centre and its own 32 candidates `θ ± 0.02·ε`, run forward over L's recorded
observation set (768 samples, 384 on food, twelve layouts, both hidden-state starts). Membership
is `Action7::squash(&head, cap).0[channel] == 0.0` — the adapter's own zero — and the head edges
below are the inverse of the documented squash at `DEADBAND = 0.05`: a thrust head below
**−2.9444** and a turn head inside **±0.050042**.

| | centre | candidates (32) | at exactly 1.000 | below 0.5 |
| --- | --- | --- | --- | --- |
| thrust inside, reset | **0.0000** | 0.0000 (all) | 0 | 32 |
| thrust inside, carried | **0.0000** | 0.0000 (all) | 0 | 32 |
| turn inside, reset | **1.0000** | mean 0.935, sd 0.239, range 0.0026–1.0000 | **29 / 32** | 2 |
| turn inside, carried | **0.4206** | mean 0.433, sd 0.171, range 0.1185–0.7669 | 0 | 24 |

- **Thrust is never in the deadband**, for any candidate, at any state. The mean raw thrust head
  runs −0.012 to +0.127 — that is σ(0.09) ≈ 0.52, L's measured held thrust of 0.524 — against an
  edge at −2.944. The body always pushes. Nothing the search can do at σ = 0.02 will stop it by
  releasing thrust.
- **From a reset hidden state, 29 of 32 candidates request exactly zero turn on 100 % of the 768
  observations.** L found this for the centre; it survives a σ-perturbation almost unchanged. Two
  candidates escape almost completely (`pair2−` at 0.26 %, `pair5+` at 2.34 %) and one partially
  (`pair1+` at 90.1 %), so σ is right at the threshold that flips this channel between "always
  dead" and "always live" — but the modal outcome is that a newborn perturbed policy cannot turn
  at all, whatever it is shown.
- **Along the trajectory the turn channel straddles the edge.** The centre's turn head is inside
  the band on 42 % of ticks and its mean `|head|` is **0.0600** against an edge of **0.0500**. The
  candidates run 11.9 %–76.7 % with mean `|head|` from 0.033 to 0.110. So a σ-perturbation *does*
  reach the turn channel — it moves the fraction of the life on which turning is expressible over
  a 65-point range — but it is moving a quantity whose operating point sits on the clipping
  threshold, so roughly half of what it buys is thrown away by `band`.
- Turn occupancy is **not** what separates good candidates from bad ones within generation 9:
  r = 0.206 between candidate score and carried turn-inside fraction. Turning more is not, on its
  own, scoring better.

## Verdict, by Astra's rule

> Confirmation of an optimiser/update problem: individual perturbations with better residence and
> score that cancel in the centred-rank gradient or are erased by the update. Refutation: no
> candidate-level increase in on-food residence despite the score spread.

**Refuted as an optimiser/update problem.** Both halves of the confirmation fail on measurement:

- **Nothing cancels.** The summed pair contribution retains 1.002× the mutually orthogonal
  reference, in all sixteen generations. There is no pair whose contribution is destroyed by
  another pair's.
- **Nothing is erased.** The replay from the run's own first centre reproduces the run's
  checkpoint exactly, so the update applied is the estimator's, unaltered; the centre lands in the
  top quartile of its own population for nine consecutive generations; and the population's mean
  score rises in trend across the run.

The refutation branch as Astra wrote it is **also not met** — candidates plainly do feed and
reside more when they score more (75.7 % concordance on intake per tick, r = 0.859 within
generation 9). So the honest verdict is neither of the two branches as stated: **the reduction and
the update are faithful, and the residence variation they have to work with is real and two orders
of magnitude too small.** Attention moves, as the refutation branch directs, to parameterisation
and the adapter.

One qualification belongs to the score's **aggregation** rather than to the optimiser, and it is
the only measured information loss in the whole chain: the four-layout minimum inverts the
estimator's preference on 29 % of pairs, worth 683 ticks of mean survival each, and on 27 of them
against a three- or four-of-four majority.

## The next change, named and not implemented

**The adapter's turn deadband.** Its operating point, not its existence: the centre's turn head
has mean `|head|` 0.0600 against an edge of 0.0500, so the channel spends 42 % of the trajectory
and essentially 100 % of every newborn candidate's ticks clipped to exactly zero, and a σ-scale
perturbation moves that fraction over a 65-point range without ever getting the channel clear of
the threshold. Residence requires stopping and turning; this is the one measured place where the
mapping from weights to behaviour discards most of what the search puts into it. The change is to
the adapter — narrowing or removing `DEADBAND` on `TURN` alone, or re-centring the turn head's
initialisation so the band is not straddled — and it changes what any policy *can express*, so it
is a fresh run under a new protocol hash, never a migration.

Ranked behind it, with the evidence for each:

1. **The layout aggregation.** `Aggregate::Mean` already exists, already hashes as a different
   task, and needs no new code. The measured case is the 29 % sign inversion above. The risk is
   explicit and must be stated with the change: the minimum is what forces generality, and a mean
   lets a candidate that dies at once on `t2-weak-open` win on the other three. The falsifier is a
   paired A/B at the same seed — if the `Mean` arm's centre does not gain more residence than the
   retained `Min` arm's by generation 16, the aggregation was not the constraint.
2. **σ.** The estimator is not the bottleneck, so raising σ is a way of reaching further into
   behaviour space rather than a repair. It also interacts with the turn deadband in the direction
   the first item wants. It should follow the adapter change, not precede it, so that the two are
   not confounded.
3. **The rank reduction and the pair count are ruled out**, not deferred: the excess-cancellation
   measurement says sixteen pairs neither fight each other nor waste each other's contribution,
   and the concordance columns say the ordering the rank keeps is the right ordering 62–76 % of
   the time.

**The score stays as it is**, as the brief requires. Nothing in this workstream changes the
trainer's score, update, sampler or protocol.

## What this does not establish

- **Nothing about a whole world's foraging.** One run, one ecology, one config hash, one seed,
  four training layouts, sixteen generations, one horizon.
- **Nothing about what any named change would do.** Every one of them is named and none is
  implemented, measured or optimised against. The `Mean` aggregate in particular has never ranked
  a candidate in this project.
- **Nothing about residence beyond two proxies.** `intake_producer` and `ticks_in_opening` are
  what the retained episodes hold. H's per-tick on-food residence trace was **not** run on the
  candidates (deliverable 3's gate was not reached), so "candidates with better scores reside
  more" is established on intake per lived tick and opening occupancy, not on H's own on-food
  fraction. If Astra wants that exact column per candidate it is a 128-episode run and about 25
  seconds of simulation.
- **Nothing about the carried hidden state being the right one.** Deliverable 2 runs perturbed
  weights from the *centre's* hidden states: a perturbed policy would have built its own. That
  mismatch is the experiment the brief asked for — what a σ-perturbation does to the head at
  states the run actually visited — and the reset arm is the one that carries no such assumption.
- **Nothing about sixteen generations being a fair sample of the search.** The run stopped at 16;
  Adam's per-coordinate step was still decaying as `lr/√t` when it did, and whether the trend
  continues, flattens or reverses past generation 16 is not measured here.
- **Nothing about the deadband's effect on a trajectory.** Deliverable 2 counts membership at
  sampled states. It does not run an episode with the band changed, and it therefore does not say
  that removing it would produce more residence.
- `cargo clippy -p cubarium-core --all-targets` still fails on the pre-existing `erasing_op` deny
  in `crates/cubarium-core/src/neural/gru.rs:233`, untouched by this workstream.
