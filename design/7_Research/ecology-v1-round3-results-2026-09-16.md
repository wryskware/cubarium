---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Ecology v1, round 3: results of workstreams L, M, N and O (2026-09-16)

Fable's consolidated result for the reconciled next steps of
[the round-2 result](ecology-v1-round2-results-2026-09-16.md) ("Next
recommendation (reconciled with Astra)"), after Astra's
[round-2 review](ecology-v1-round2-review-2026-09-16.md) cleared round 2.
Briefs: [L](../handoffs/ecology-v1-score-checks-opus-2026-09-16.md),
[M](../handoffs/ecology-v1-plant-budget-opus-2026-09-16.md),
[N](../handoffs/ecology-v1-apex-reach-opus-2026-09-16.md),
[O](../handoffs/ecology-v1-depth-factorial-opus-2026-09-16.md), committed at
`1525604`. Four Opus workers in parallel: L on `main`, M, N and O in worktrees.
Wrysk's standing instruction: keep going, tune nothing, backlog operator
controls and artwork ([backlog](../backlog.md)); the one decision reserved for
him this round is whether the bounded score experiment runs if L's checks leave
the hypothesis standing.

## Dispatch and budget

| stream | model | where | simulation used / cap | storage | tests after |
| --- | --- | --- | --- | --- | --- |
| O depth × diet factorial | Opus 5 medium | worktree, merged `10d3e2a` | 35 s / 3 min | 180 KiB / 10 MiB | search 199 |
| L score falsification checks | Opus 5 high | `main` (`14345d9`…`f52d441`) | 22 s / 8 min | 0.8 MiB / 30 | core 502, search 199 |
| M plant budget | Opus 5 high | worktree | *(pending)* | | |
| N apex strike reach | Opus 5 high | worktree, merged `9c608b5` | 58 s / 6 min | 2.6 MiB / 20 | core 508, search 205 |

Fable's verification so far: O's whole design re-run and matched by
`final_state_hash` on 16 of 16 rows; its search suite 188 on the branch, 199
after merge. L's dwell-1000 rung re-run from its ignored test and matched to
the tick on both layout sets (36,000 and 27,362). N's death arm re-run and
compared field for field with its retained rows (identical, timing keys
excluded); its 12 new tests pass on the branch and the suites after merge.

## O — the depth × diet factorial

Full note: [ecology-v1-depth-factorial-2026-09-16.md](ecology-v1-depth-factorial-2026-09-16.md).
Commits `94d148b` (harness), `3c384be` (note); merged `10d3e2a`. Search-only;
no core change. Tests first, 29 in the factorial suite (+12) and two new unit
tests, run red before the arms existed.

**Design decision, disclosed.** The brief asked for "two counterbalanced arms"
and a "4 × 8 Latin-style assignment" in one sentence; two arms cannot give
every cell every one of four treatments, and a slot-by-slot complement on both
factors would have put the depth contrast *between* cells — the confound Astra
caught in J. O used four rows (`Arm::D1..D4`), treatment `(slot mod 4) XOR row`
(bit 0 diet, bit 1 depth): every cell holds every treatment once, every row
holds each treatment twice on two faces, and each pair of rows is a
one-factor exchange in every slot. 16 runs, 35 s. `depth` decodes only to the
preferred height in the steering term (no capacity, rate or bill; asserted by
test); 0.55 is the roster grazer's value (`h_pref` +0.1, just above the
equator) against the skimmer's roster 0.10 (`h_pref` −0.8, the wet rim).

| treatment (depth / diet) | reached horizon | established | median life | served foliage / litter | median net margin | algae-band probes | distinct cells |
| --- | --- | --- | --- | --- | --- | --- | --- |
| T0 0.10 / 0.60 (roster skimmer) | **0 / 32** | 27 | 863 s | 0.81 / **4.66** m | **−0.0020** e/s | 34 % | 122 |
| T1 0.10 / 0.85 | 6 | 7 | 386 s | 4.29 / 0 | −0.0053 | 9 % | 79 |
| T2 0.55 / 0.60 | **26** | 30 | 4,500 s (horizon) | **22.5** / 1.95 | **+0.0006** | 7 % | 426 |
| T3 0.55 / 0.85 | **27** | 27 | 4,500 s | 16.4 / 0 | **+0.0008** | 5 % | 406 |

**Verdict: depth couples the body to litter — supported, more strongly than
anything J measured.** Within-cell, depth 0.55 beats 0.10 on net margin in
30 of 32 pairs at the founder diet and 29 of 32 at the foliage diet, in 4 of 4
worlds, and the median margin changes sign in every world (per-seed medians
never overlap). The roster skimmer starves 32 of 32 and never reaches the
horizon; moved off the rim it reaches the horizon 26 of 32 while eating thirty
times the foliage over three and a half times the cells. Depth barely moves
the binary wet-probe fraction (67 → 65 %) but moves the water depth under the
body (0.31 → 0.05 d) and the algae-band fraction (34 → 7 %); future notes
should report water depth, not the wet flag. **Interaction, a sign reversal:**
on uncensored net margin the founder's 0.60 wins 25 of 32 pairs at depth 0.10
and loses 26 of 32 at depth 0.55, in 4 of 4 worlds. That reconciles J with F:
F's association (foliage-diet skimmers do better) is real, and real only for a
body not parked in a pool; J saw the opposite sign because every clone it
compared had `depth` 0.10. The T0-versus-T1 contrast is J's counterbalanced
pair on the same cells and seeds and agrees qualitatively (low diet wins 25 of
32 pairs against J's 29; established 27 against 7; 4 of 4 worlds), with hashes
necessarily different because each run now holds all four treatments.

Corrections O made against its own rows before publishing: the six depth ties
at the foliage diet are cells where both clones reached the horizon (T1's
lottery winners), not double starvations; fruit and carrion are near zero,
not zero; the Top-face slot did not work as a null (n = 4, described only).

**Named, not launched:** re-run F's 150-minute reproduction census with the
roster skimmer at `depth` 0.55 and nothing else changed — the test of whether
a founding-time effect this large survives a lineage, and the only way to
decide whether `depth` 0.10 is a bug in the roster or a bug in the world. The
wet floor has no producer (`water.algae_light` raises a *living* cell's light
floor; foliage grows only where wood already is), so the skimmer's designed
larder does not exist; that is an ecology decision and Fable's. And the
detrital-funding question is now close to answered in the negative: across
128 lives litter funded nothing (T0 served 4.66 m, credited 1.15 m against a
4.67 e bill, starved 32 of 32) and every positive margin came from foliage.
Until a calibration shows some setting at which litter pays, ecology v1 has
one working guild, not four.

## L — the score falsification checks: the hypothesis is falsified

Full note: [ecology-v1-score-checks-2026-09-16.md](ecology-v1-score-checks-2026-09-16.md);
proposal, written and marked **not proposed**:
[forager-score-proposal-2026-09-16.md](../forager-score-proposal-2026-09-16.md).
Commits `14345d9` (code, tests, and the auxiliary's constants fixed before
any run — checkable from history), `ce99e87` (note and proposal), `f52d441`
(one wording correction). No new `neural/` accessors were needed; the forward
pass, squash and observation were already public. Entry points are ignored
tests, as H's, because three worktrees held the CLI open.

**Check (a): the frozen controller's movement does not respond to food.** 768
real generation-9 observations per driver (on- and off-food, all twelve
layouts), each re-run through the core's own GRU forward pass and action
squash with only the local food scalar moved (0 → 1) or only the ring food
sectors moved, from a reset and from the carried hidden state. Every sign is
right (more food → less thrust, more graze) and every magnitude is about 430
times too small: a full stand moves thrust by 0.0011 against a held value of
0.52 and a stopping deadband of 0.05; the effects are 0.06–0.19 of the
action's own standard deviation. From a reset hidden state both policies
request exactly zero turn. Nine generations of training barely moved food
sensitivity (untrained −0.00097 thrust per unit food, generation 9 −0.00109).
L measured the one-tick observation lag it relied on rather than assuming it:
max 1.8e-5, 59–160× below the effects. This confirms H's phenotype at the
level H said it had not measured; it does not weaken the score hypothesis.

**Check (b): the current score already pays enormously for staying.** A
dwell-parameterised control (the disclosed mobile script with its departure
rule replaced by a counter; the top rung *is* the script) scored with the
trainer's own `t_min + 0.25·stores`:

| rung | `t_min`, training four | `t_min`, all twelve | alive of 12 | on-food fraction |
| --- | --- | --- | --- | --- |
| dwell 20 ticks | 23,490 | 12,951 | 6 | 0.65 |
| dwell 100 | 24,269 | 16,658 | 6 | 0.71 |
| dwell 300 | 27,559 | 21,370 | 6 | 0.79 |
| dwell 1,000 | **36,000** | **27,362** | 11 | 0.88 |
| dwell 3,000 | 18,309 | 18,309 | 6 | 0.80 |
| until below threshold (the script) | 36,000 | 26,355 | 11 | 0.94 |
| generation 9 | 8,703 | 6,914 | 0 | 0.08 |

+53 % on the training four and +111 % on all twelve between a one-second and
a fifty-second dwell, monotone up to 1,000 and down only at 3,000 for a legible
reason (150 s on the weak opening crops it bare). The gradient is five orders
of magnitude larger than the stores tie-break and 70 times larger than the
auxiliary Astra proposed. **Falsifier 1 fired: the current score already has
a strong dwell gradient, so no score change is proposed.** The proposal was
nevertheless written in full with its constants fixed in advance (`T` 36,000,
clip ±1, ticks after death −1, `b_ref` the body's one-tick upkeep, `λ` 100 so
the whole term spans 10 s and cannot erase a survival gap above that), and
the ladder under the proposed `S` is monotone and inverts no ordering of the
current score: well-behaved and unnecessary.

**What the evidence points at instead**, named not launched: the search is
not converting a 14,000-tick behavioural gradient into parameter-space
movement. Cheapest first, no simulation: measure the within-generation score
spread from the existing `es-eco-v1` checkpoints, which separates "the
optimiser cannot move" from "the objective does not reward". Then: how much
of the action sits inside the adapter deadband under a σ-scale perturbation
(the reset-state zero-turn finding). Then, most expensive: the score's
gradient in generation 9's *own* behavioural neighbourhood — every ladder rung
is already a perfect navigator and generation 9 is not, which is the gap
check (b) leaves open. L's numbers reproduce H's independently (generation 9
on food 0.116 and 0.085 on H's two per-tick layouts; the no-intake control
dying at the pinned 7,420).

## M — the plant budget of a depleted cell

*(pending)*

## N — why an apex strike ends out of reach: the pursuit stopping rule

Full note: [ecology-v1-apex-reach-2026-09-16.md](ecology-v1-apex-reach-2026-09-16.md).
Commits `5ef88d1` (the per-attempt strike record, inert and opt-in, with core
tests and the audit classification), `b144605` (the body-frame coordinate,
the hunter's own turn and the hold counters, so the mechanism is measured
rather than inferred), `6310825` (note); merged `9c608b5`. Three frames per
paid attempt (intent, strike start, resolution), each gathered through the
same `ContactEvidence::gather` the settlement uses, 12 and 20 ticks apart by
construction. Inertness by state hash at every 500-tick boundary of a 9,000-tick
two-apex world with the recorder on and off, with a non-vacuity assertion.
Two additive lines outside the brief's list (the recorder's field in
`world/mod.rs` and its initialiser in `lifecycle.rs`) and no CLI hunk (the
record follows the existing `--no-ledger` flag) — both accepted as routine.
N wrote the recorder before its own test file and then verified the tests
bite by mutation; the classification and search tests were written from
definitions first. Disclosed.

**Classification of the death arm's 449 paid attempts** (reconciles exactly
with K: 46 contacts = 15 captured + 31 missed; 30 + 217 + 155 = 402
out-of-reach):

| class | n | separation at intent → resolution (px) | prey forward coordinate at intent (px; grasp at 13.3) | hunter realised speed (px/s) |
| --- | --- | --- | --- | --- |
| resolved in reach | 46 | 3.7 → 2.9 | 13.6 | 1.24 |
| began in reach, resolved out | 30 | 2.8 → 5.2 | 12.9 | 1.43 |
| **prey outran** | **217** | 12.3 → 14.6 | 6.2 | **0.12** |
| **began out of reach** | **155** | 12.4 → 10.3 | 7.9 | 1.12 |
| target lost | 1 | — | — | — |

**Captures are not lunges that worked; they are prey already in the claws.**
Captures began at 3.9 px separation with the prey 12.6 px forward, level with
the grasp; refusals began at 11.6 px with the prey 7.4 px forward, 6 px short.
Tolerance and advertised reach are identical across outcomes; prey speed
barely differs. The probe arm (age-eligible introduction, 501 attempts)
replicates every reading.

**Verdict: the pursuit controller.** The strike speed constant is 16.7 px/s;
the median realised hunter speed over a paid one-second burst is **0.002
px/s**, and 80 % of held bursts are at or below the resting effort's cruise
(0.21 px/s). The gap that needed closing at the burst's start averaged 8.3 px;
the gap actually closed averaged **−0.9 px** — it grew. A *delivered* burst
would out-close a prey at its escape cap (10 px/s, exactly the multiple times
the prey's speed cap) by 6.7 px/s, so the escape multiple is exonerated
quantitatively. **Named, not changed:** the pursuit stopping predicate
`inside` in the hunt-intent pass of `world/step.rs` is a one-sided forward
half-space (17.25 px) although its own comment says it means the reach
envelope, as `in_contact` does everywhere else in the file. The apex senses at
12 px and its grasp closes 13.3 px out, so nearly every huntable prey satisfies
it — 408 of 449 paid attempts, 459 of 501 in the probe. When it is true the
member drops to the resting effort *and* suppresses the burst it has just paid
0.08 e for; the 0.21 px/s left is consumed by the turn, priced at the claws'
own 14.8 px radius. Two constants are named only as downstream
(`drives.rest_effort` 0.05, the 14.8 px turn radius). A corroborating but
non-randomised comparison: the 41 attempts where the burst was actually pushed
delivered 5.7 px/s and resolved in reach 27 % of the time against 9 %.

**What this does not establish:** no intervention was run; nothing says what
reading `inside` as the envelope would do, nor whether an apex that closes
would then feed itself (K's arithmetic needs about ten captures per life
against 0.9 achieved), nor how the 12 px sense radius interacts with the rule;
the capture roll (15 of 46 contacts) was not examined; the small "began in
reach, resolved out" class is weakly estimated. **Named next, not launched:** a
paired arm with `inside` read as the envelope (`in_contact()`), scored on
exactly these rows — held fraction, gap closed, class histogram, captures per
life — one 30 s run per variant, with two cautions: the same hold governs the
stalk, so phase occupancy must be re-read, and an apex that closes spends
more on motor, so K's ledger decides whether it pays. The predicate decision
is Fable's; the display's apex is unchanged by this workstream.

## What this does and does not establish

*(after L, M, N)*

## Next recommendation

*(after L, M, N, reconciled with Astra)*
