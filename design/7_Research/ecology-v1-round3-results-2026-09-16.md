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
| M plant budget | Opus 5 high | worktree, merged `4819a5c` | 2.5 min / 8 min | 13 MiB / 60 | core 517, search 217, host 596, render 101 |
| N apex strike reach | Opus 5 high | worktree, merged `9c608b5` | 58 s / 6 min | 2.6 MiB / 20 | core 508, search 205 |

Fable's verification so far: O's whole design re-run and matched by
`final_state_hash` on 16 of 16 rows; its search suite 188 on the branch, 199
after merge. L's dwell-1000 rung re-run from its ignored test and matched to
the tick on both layout sets (36,000 and 27,362). N's death arm re-run and
compared field for field with its retained rows (identical, timing keys
excluded); its 12 new tests pass on the branch and the suites after merge.
M's herbivore-present arm re-run for one seed of each configuration with the
plant record on: matches M's rows and I's original arm-0 rows by
`final_state_hash` (4 of 4). All four merges were conflict-free; the shared
build cache's stale-artifact race and the ancestor-checkout worktree hand-out
recurred and were handled as before.

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
and loses 26 of 32 at depth 0.55, in 4 of 4 worlds. That makes J and F consistent:
F's association (foliage-diet skimmers do better) is plausibly real for a
body not parked in a pool, and J saw the opposite sign because every clone it
compared had `depth` 0.10; the cause of F's descendant association is not
identified here (sterile cold-founded clones for 4,500 s against a reproducing
150-minute census), and F's census with only depth changed is still required. The T0-versus-T1 contrast is J's counterbalanced
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
That answers the question negatively for the current skimmer body at the
current parameters in `fast-leaf`; whether any body or calibrated detrital
channel can be funded is open (scoped after review).

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

**What the evidence points at instead** (corrected after Astra's review, P1):
the score rewards the missing behaviour and the frozen controller's one-step
response to food is far below the deadband; the one-step sweep does *not*
show that the controller cannot express residence or that the search moves
nothing in parameter space. Astra checked the retained generation reports:
candidate scores in generation 9 span 6,459–8,915 ticks (sd 643) and
correlate r = 0.81 with mean producer intake across the training layouts, so
perturbations do produce material variation and L's "measure the spread"
task is already answered. The open question is where useful candidate
variation is lost — in the four-layout minimum, the centred-rank reduction,
the update, or the weights-to-residence mapping. Named next, no simulation:
the antithetic-pair reduction on the retained reports (score difference
against intake, opening residence and signed update contribution), then
deadband occupancy under a σ-scale perturbation; only if on-food time is
needed, reconstruct generation 9's 32 candidates and run H's trace. The score
stays. L's numbers reproduce H's independently (generation 9
on food 0.116 and 0.085 on H's two per-tick layouts; the no-intake control
dying at the pinned 7,420).

## M — the plant budget of a depleted cell: over-seeding confirmed, no crossing grazed

Full note: [ecology-v1-plant-budget-2026-09-16.md](ecology-v1-plant-budget-2026-09-16.md).
Commits `bdc2fcd` (record, split, tests), `933d92d` (note); merged `4819a5c`.
The per-cell plant budget is recorded **where §4.1–4.8 are computed**
(`crates/cubarium-core/src/fields.rs::react`, not `world/step.rs` as the brief
assumed — recomputing the equations in the recorder would have been a second
implementation of the contract, the class of proxy this workstream exists to
remove; four three-line withdrawal hooks in `step.rs` at the §6.4 site);
opt-in, hanging off the transient `EcoScratch`, inert by hash (12 of 12 rows
byte-identical in every metrics block with the record on and off, and equal
to I's retained rows), cost +1.4 % of run time, per-cell identity residual
≤ 1.2e-11 over 180,000 ticks. The depletion counter is now split by **exact
withdrawal since the last recovery**, with I's probe flag kept beside it.
`--no-animals` empties the founder roster for the absent arm. Tests first: 9
core (also green in debug with the per-tick audits live) and 12 search.

| arm | config | crossings | with exact withdrawal | without | without **and** negative budget | I's probe called "visited" |
| --- | --- | --- | --- | --- | --- | --- |
| herbivores present | baseline | 68 | **0** | 68 | **68** | 41 |
| herbivores present | `fast-leaf` | 5 | **0** | 5 | **5** | 1 |
| herbivores absent | baseline | 382 | 0 | 382 | **382** | 0 |
| herbivores absent | `fast-leaf` | 180 | 0 | 180 | **180** | 0 |

**Verdict: confirmed on both branches of Astra's rule.** Not one of the 73
present-arm crossing cells lost a metre of foliage to any mouth in 180,000
ticks, while the same runs withdrew 1,500–2,400 m per run from 663–1,113
*other* cells. The same cells cross without herbivores (73 of 73 overlap;
present-only crossings 0 in every one of the 12 seeds), at the same or an
earlier time (73 of 73; median about 9,000 ticks earlier), with a negative
measured plant budget over the 6,000 ticks before the crossing and over the
whole run, in both arms. Neither refutation branch fired: crossings rose 5.6×
and 36× without herbivores and no crossing cell's budget was positive. I's
probe flag called 42 of 73 crossings "visited" and the exact counter finds a
bite in none, so the old split **over**-stated grazing, the opposite of the
direction Astra worried about. Mechanism at the crossing cells (absent arm,
medians against other watched cells): opening foliage 0.065 against 0.101,
static `L·μ` 0.27 against 0.42 driven by low **moisture** not low light, `N`
0.20 against 0.26, wood 0.08 against 0.30, whole-run gross income 0.45
against 6.06; 71 of 73 thinned while alive. §11 seeds 15 % (baseline) and 9 %
(`fast-leaf`) of watched cells above what the plant step alone sustains, and
the crossing cells at about 25× what they hold at the horizon. **At the
shipped price the depletion counter measures the seeding, not grazing**; A's,
F's and I's counts are correct as counts and the sentences around them should
be re-read that way.

**Limits M flagged:** the absent arm is not "the same world without grazing"
— removing the founders removes their faeces and carcasses, so its `N` is much
lower — which is why the confirmation rests on the 73 present-arm crossings
having zero withdrawal and a negative budget *in the herbivore-present world*;
foliage at 180,000 ticks is a state, not a proven equilibrium; one price, two
configurations, arm 0, training seeds; 33 of the 68 baseline crossings come
from one seed; only 2 recoveries in 24 runs, so the "since the last recovery"
clause is exercised by the hand-built tests, not the campaign; no critical
`L·μ` is defined or used anywhere.

**The owner's options, stated and not decided (§11 untouched):**
- **A, a plant-only warm-up before founding animals.** The absent arm's first
  crossing is at tick 42,000–55,000 and the median at about 145,000, with
  most crossings after 120,000, so an adequate warm-up is of the order of
  the whole 150-minute horizon. Visible: the display opens on a world that
  has already sorted itself, no uniform green-then-fade, but founders meet
  3.6–4.2× the seeded standing foliage.
- **B, seed each cell below its own measured terminal state** (not an
  equilibrium: the tick-180,000 plant-only state is not a proven fixed point
  and its predictors are endogenous and coupled; Astra). Today's
  opening foliage over watched cells: total 107, median 0.099, spread (CV)
  0.39; the plant-only world at the horizon: total 389–447, median 0.45–0.46,
  CV 0.46–0.53. Visible: dim dry cells open visibly thinner than bright wet
  ones and nothing is seeded into a deficit, and the world opens 3.6–4.2×
  greener overall, which is a presentation decision too.
- Not supported: keeping §11 as is and reading the counter as a grazing
  signal.

**Named next, not launched** (revised after review): rather than fitting a
per-cell "equilibrium", save plant-only whole-field states at several ages
spanning the first crossing through 180,000 ticks, measure moving-window
changes in total and per-cell `P`, `W`, `Q`, `N`, exact plant income and
loss, threshold crossings and spatial variance, found identical rosters into
status quo and a few of those states, and give Wrysk the opening frames and
early founder outcomes. That is option A with a declared, deterministic
procedure; option B's endpoint fit is at most an initialiser.

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
the prey's speed cap) by 6.7 px/s, so the escape multiple is exonerated as
the cause of the *current* near-zero motion — not yet shown adequate for a
corrected, delivered lunge, which needs 1.25 s for the mean gap against a
1.0 s strike (Astra). **Named, not changed:** the pursuit stopping predicate
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

- **Established by measurement:** the forager's score already rewards
  staying (a 53–111 % dwell gradient) and its frozen controller's movement
  does not respond to food (effects 0.06–0.19 of the action's own spread),
  so the score is not the constraint; where candidate variation is lost
  between the objective and the update is open (the retained generation
  reports show a wide, intake-correlated score spread); the apex's paid bursts are suppressed by the pursuit
  stopping predicate on 408 of 449 attempts and its realised strike speed is
  near zero, while the escape multiple is exonerated for the current motion
  only; the counted depletions
  at the shipped price are seeding artefacts in dim, dry, low-wood cells with
  a negative plant budget and zero withdrawal, confirmed in the
  herbivore-present world itself; the skimmer body thrives once its depth
  preference leaves the wet rim, and there the foliage diet wins, reconciling
  J and F.
- **Not established:** why the ES search does not convert the behavioural
  gradient (optimiser step, deadband, or the local landscape around
  generation 9); what reading the pursuit predicate as the envelope would do
  to captures, phase occupancy and the apex's bill; whether the skimmer's
  founding-time rescue survives a reproducing lineage, and whether `depth`
  0.10 is a roster bug or a world bug (the wet floor grows no producer);
  whether any setting lets a detrital diet fund a body; the fit that would
  replace §11's uniform seeding.
- **The cube is untouched by this round**: build `77c42e8`, `fast-leaf`,
  shipped price, shoulder 0.95 by override. On M's result, the slow fading of
  its dim dry cells is the seeding relaxing, not grazing; on O's, its skimmers
  are starving on the rim by roster design; on N's, any apex spawned from the
  viewer stops short of nearly every prey it pays to strike.

## Review and repair (Astra, 2026-09-16)

Astra's review is
[ecology-v1-round3-review-2026-09-16.md](ecology-v1-round3-review-2026-09-16.md).
Disposition: retain M and N as decisive diagnostics and O as a strong
founding-time result; L correctly falsifies the score term; the package
overstated what follows. Repair 1, all report-level plus one doc line:

- **P1, L.** "The controller cannot express residence / the search converts
  nothing" withdrawn; the one-step sweep does not test that, and the retained
  generation reports already show the candidate-score spread L named as its
  next task (sd 643 in generation 9, r = 0.81 with intake). The next task is
  the antithetic-pair reduction, not the spread.
- **P2, M.** Option B renamed a terminal-state fit; the local-equilibrium
  wording withdrawn; Astra's whole-field preconditioning operator recorded as
  the alternative that keeps the coupled equations. No universal critical
  `L·μ` anywhere.
- **P2, N.** The escape multiple and strike constants are exonerated for the
  current motion only; adequacy for a delivered lunge is the paired arm's
  question (1.25 s needed against a 1.0 s strike at the escape cap).
- **P2, O.** F's reconciliation stated as plausible, not identified; "one
  working guild" scoped to the current body and parameters.
- **P3.** `apex-audit`'s `--no-ledger` also disables the strike record; its
  help now says so.
- **Errata** added to A's, F's and I's notes: their depletion counts at the
  shipped price are opening-stock plant-budget declines with zero measured
  withdrawal, per M.

## Next recommendation (reconciled with Astra)

Astra's order, which Fable accepts and which the numbering below now follows:
the apex predicate pair first; the antithetic-pair ES analysis second (instead
of the already-answered spread); the depth census third, before any roster
decision; the whole-field preconditioning comparison fourth, instead of a
per-cell fit; detrital calibration later; the errata done now.
Three items would be visible on the cube and are Wrysk's; what Fable and Astra
would tell him is stated at each. None runs on the cube until he has seen the
result.

1. **Apex pursuit predicate** (N) — the single most informative cheap
   experiment: N's paired arm with `inside` read as `in_contact()`, one
   variable, identical seeds and introductions, scored on held fraction,
   initial gap, relative closure during delivered bursts, realised translation
   and turn consumption, contact and capture class, captures per life by
   initial-gap bin, phase occupancy, and K's credited, billed and net energy.
   Confirmed if the corrected arm delivers the burst, closes the gap and
   raises contacts and captures; refuted if the held fraction falls but
   closure and contact do not improve. **What Fable and Astra would tell
   Wrysk:** approve the one-line correction of the predicate to its own
   comment if it delivers those outcomes without a worse ledger; it need not
   reach the ten captures per life that break even to show the rule is wrong
   (that is a later ecological gate); do not touch escape speed, sense radius,
   strike duration or the mating radius in that arm.
2. **The ES search** (L): the antithetic-pair reduction on the retained
   generation reports (no simulation), then deadband occupancy under a
   σ-scale perturbation; then decide whether the reduction, the update or the
   adapter is the next change; no score change.
3. **Skimmer depth** (O): F's six-seed, 150-minute census with the roster
   skimmer's `depth` 0.10 → 0.55 and every other value fixed; measure founder
   survival and brood, descendant census by form × diet bin × guild, net
   margin, water-depth distribution, and the effect on the other kinds.
   Confirmed if the skimmer establishes a lineage across seeds without
   replacing the world with a new monoculture. **What Fable and Astra would
   tell Wrysk:** conditionally approve the roster depth change on that result
   (the cube's skimmers would stop dying on the rim); do not bundle it with a
   wet-floor producer, which is a separate ecology design (a new stock, food
   web and visible layer), not the repair O needs.
4. **Foliage seeding** (M): a whole-field plant-only preconditioning
   comparison, not a per-cell equilibrium fit — save plant-only states at a
   few ages, found identical rosters into them and into status quo, and give
   Wrysk the opening frames and early founder outcomes before he chooses
   between preconditioning (A, with a declared procedure), leaving §11, or B
   as a mere initialiser. This is his call; Astra advises neither as a
   contract change yet.
5. **Detrital funding** (O, M), after the niche decision: on a fixed body,
   habitat and control, vary one declared detrital capacity or energy term at
   a time and measure the ledger chain served litter → digestible → credited
   → whole bill, plus substrate persistence; the first gate is a non-negative
   median margin in nearly all worlds without exhausting litter faster than
   §4 replenishes it. O answered the question negatively only for the current
   skimmer at current parameters.
6. **Errata in A, F and I** — done in this repair, not a research campaign.

Nothing in this list touches the cube.
