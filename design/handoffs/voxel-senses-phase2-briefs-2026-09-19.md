---
status: closed
date: 2026-09-19
owner: Fable (orchestration); Wrysk decides
---

# Voxel senses — phase-one review and phase-two briefs

Read [voxel-senses.md](../voxel-senses.md), the
[phase-one plan](../voxel-senses-phase1-plan.md) and the
[phase-one tests plan](../voxel-senses-phase1-tests.md) first. This file records
the orchestrator's review of phase one at `389cb33` and the two bounded packages
that follow from it. Workers do not reinterpret the design; routine choices
inside a package are the worker's, stated in the return.

## Review of phase one (Fable, 2026-09-19, at 389cb33)

Verified, not taken from the close-out note:

- `cargo nextest run` over voxel-fauna, voxel-sim, voxel-flora, search and
  core: 797 passed, 0 failed, 1 skipped.
- Both pilots ran the full protocol (32 updates, 2,180 episodes, 2.6 M ticks
  each, nothing discarded). Training scores climbed monotonically-ish and were
  still rising at the cap (blind best 0.307 at update 26, browser 0.308 at 31).
- Held-out re-evaluation from the saved centres (score = intake − motor + 0.25
  survival, all survived 8/8, so 0.25 is the floor):

| founder | gru | heuristic | stationary | intake gru / heuristic |
| --- | --- | --- | --- | --- |
| blind | 0.287 | 0.208 | 0.250 | 0.065 / 0.028 |
| browser | 0.290 | 0.284 | 0.250 | 0.076 / 0.112 |

Findings that shape phase two:

1. **The Stage-A start hands the controller the answer.** `arena.rs` aims the
   founder at the in-signal resource with ±5° jitter, 0.5–1 m away. The learned
   policies are near open-loop: per-episode mean forward / turn / feed are
   0.389 / 0.13 / 0.57 (blind) and 0.48 / 0.24 / 0.50 (browser) to two decimals
   on every held-out layout, whatever the geometry. "Go forward, keep feeding"
   from a start that faces food acquires food. The held-out 7/8 and 8/8 do not
   yet show that either policy uses its senses. Not disproven either; untested.
2. **The observation-only heuristics pay more than they earn.** Both drive at
   0.997 forward with heavy turning; the blind one scores below doing nothing.
   Motor cost at full cruise equals basal upkeep by design, so a searcher that
   never slows loses the score even when it eats. That is a property of the
   score and the 60 s horizon, not a bug, but it means the heuristic is not
   yet a useful "impossible budget vs bad policy" split (biosphere §5).
3. **Survival is not a discriminating term at 1,200 ticks.** Full cruise is
   affordable for ~4,000 s (blind) and ~1,440 s (browser); nothing can die in
   60 s. Every controller gets the 0.25.
4. **Stage B as landed cannot show depletion inside the horizon.** The initial
   patch is one 0.2-organic litter tile (blind, bite 0.0005/s at full effort:
   400 s to empty) or one half-grown springturf (browser). At 1,200 ticks the
   first patch never runs out, so the second leg never starts. The arena is
   fine; the protocol around it does not exist yet, and `cubarium-search` has
   no `--stage`.
5. **The policy boundary held.** The driver never reads resources or stocks;
   Stage-B patch identities are evaluator-only (`ReacquisitionArena::into_parts`).
   The engineering deliverables of the tests plan §4 are all present.
6. The independent test-authoring pass the plan requires for model-rule
   functions (motion, senses, ledger split) was not run: every founder test was
   written by the implementing worker in the same commits.

Verdict: **engineering complete; learning target not yet demonstrated.** The
close-out's "learned acquisition" should be read as "acquisition under a
start convention that does not require sensing". Phase two makes the arenas
honest and asks the question properly before anything transfers to the live
world.

## Package P2-B: honest arenas, disclosed controls, Stage B protocol, retrain

Owner: one Opus 5 worker, high effort. Files: `crates/cubarium-voxel-sim/src/arena.rs`,
`crates/cubarium-search/src/es/voxel/*`, `crates/cubarium-search/src/main.rs`.
Nothing in voxel-fauna, voxel-flora or voxel core unless a one-line accessor is
unavoidable; say so in the return. Do not touch `design/handoffs/README.md`.

Do these in order; each step has its own commit with explicit paths.

1. **Diagnostics before any change.** Add two disclosed controls and one
   evaluation option, then run them on the saved centres in
   `/tmp/cubarium-voxel-full-pilot-{blind,browser}/centers/gen{26,31}-center.json`
   (blind 26, browser 31) against the held-out set:
   - `cruise`: open-loop, forward 0.5, turn 0, feed 1.0, no observation read.
   - `--ablate-senses`: for a `gru` evaluation, every observation channel
     outside `Self` (indices 8..) is set to 0 including its validity, before the
     GRU sees it. The policy's own memory is untouched.
   Report the four scores next to the table above. If the ablated GRU and the
   cruise control match the unablated GRU within the layout spread, finding 1 is
   confirmed and the retrain in step 4 is the real first pilot.
2. **Stage-A start heading.** Blind founder: heading uniform over the full
   circle from the layout seed. Browser: uniform within ±90° of the bearing to
   the target, so the foliage is inside the −90..+90° covered by the three
   sectors at the first sample but usually not straight ahead. Keep the 0.5–1 m
   start distance. Fold the change into the ES protocol (the protocol hash must
   change; old centres are disposable and must be refused or clearly labelled).
   Fix the arena test that asserts the ±5° facing.
3. **Stage B protocol.** `--stage a|b` on `voxel-check`, `voxel-bench`,
   `voxel-train`, `voxel-evaluate` (default `a`). For `b`:
   - Size the initial patch so a founder feeding at full effort from contact
     empties it within roughly a quarter of the horizon; pick the horizon so
     the successor leg is feasible at 1 BL/s with searching (state the numbers
     in the return). Prefer changing the Stage-B patch stock over inflating the
     horizon; keep the successor at its landed 2 m minimum. Both founders.
   - Evaluator accounting from `into_parts`: tick the initial patch fell below
     a stated depletion threshold, tick of first successor bite, `reacquired`
     boolean, and intake split by patch. All outside the controller boundary.
   - Heuristic and both controls run on Stage B as well.
   - The score stays the plan's score. Report reacquisition alongside it, not
     inside it.
4. **Retrain and evaluate.** Both founders on Stage A (new heading protocol),
   then both on Stage B, from fresh random centres. Bounds: up to 64 updates
   per run (the curves were still rising at 32; add the constant), the landed
   16-worker cap, wall cap 15 min per run. Held-out evaluation for each with
   gru, ablated gru, heuristic, cruise, stationary-feeding, no-intake.
   Save centres under a disposable `runs/voxel-es-*` directory; report the
   selected generation and the exact commands.
5. **Return (≤40 lines):** the diagnostic table from step 1; the Stage-B sizing
   numbers; four held-out tables; whether the learning target (tests plan §4:
   ≥6/8 acquisition from an off-food start beating stationary feeding in median
   score, with the ablation and cruise controls clearly below the unablated
   policy) is met per founder and stage; commits; one recommended next change
   with the evidence for it. "Engineering complete / learning target unmet" is
   an acceptable answer. No report file; the return message is the report.

Tests: short function tests only (≤200 ticks) for the new controls, the
ablation, the heading protocol and the Stage-B accounting; `cargo nextest run
-p cubarium-voxel-sim -p cubarium-search` green before each commit. No golden
hashes, no bit-identical assertions.

Decision authority: patch sizes, horizon, the depletion threshold, the exact
cruise constants, any CLI spelling. Not yours: the score formula, the manifests,
the action set, anything in the fauna crate's model rules.

## Package P2-T: independent test pass over the phase-one model rules

Owner: a second Opus 5 worker, high effort, in its own worktree, writing only
**new** test files (`crates/cubarium-voxel-fauna/tests/`,
`crates/cubarium-voxel-sim/tests/`). It did not write the code under test and
must not read the existing founder tests first; read the plan's tables and the
source, then write the checks.

Cover the tests-plan §1 rows that a reviewer would most want an independent
witness for:

- Local motion: a refused step against a wall pays the requested equivalent
  displacement; turning while stopped is paid; sub-stepping cannot tunnel
  through a one-voxel wall at the cruise speed.
- Paid food: a bite debits the real stock exactly once and the organic,
  mineral and energy taken equal what the animal gained plus what it respired;
  a feed effort with no mouth contact transfers nothing.
- Feedback: `Self` channels 3..7 report the previous interval only and are
  zero at the first sample after a reset.
- Chemical field and sampler: two receptor positions inside one gradient cell
  differ; a one-voxel wall between two same-height nodes blocks interpolation;
  removing the source stops emission and the residue decays with the stated
  half-life within tolerance.
- Material cone: a rock in front of foliage yields a hit in the all-hit
  proximity and zero in the foliage fraction; a body behind the rock is not
  seen.
- Policy boundary: a `Controller` receives a vector of exactly the manifest
  length and nothing else; the observation contains no value that changes when
  a resource is moved outside sensing range.

Do not assert the Stage-A start heading; P2-B is changing it. Do not test the
ES trainer (covered). Keep each test to a few dozen ticks. Return: the list of
tests with what each would catch, anything that failed or looked wrong in the
source (with file:line), and the branch name. Do not fix the source; report.

## Not in phase two

Live-world transfer of the founder bodies (the omniscient frondgrazer still
runs the live schedule), genome linkage of appendages, Tilt, vibration, and
the remaining roles wait until the Stage-B evidence says which slice is next.

## Integration note (Fable, 2026-09-19, at dc14f49)

P2-T landed at 2db1535 (17 new independent tests) and found the litter-cue
stencil cornered instead of centred: a receptor at a face centre read a quarter
each of its own node and its +x/+z/+x+z neighbours, so one source read 0.25
behind the body against 0.11 the same distance ahead. Fixed at dc14f49 (one
line in `senses.rs`, witness test promoted). P2-B landed 320e624..83f6dc7 and
its blind retrains ran **before** that fix, so the blind rows below are on the
biased field. Workspace suite at dc14f49: 1,913 passed, 1 skipped.

P2-B's step-1 diagnostic half-refutes review finding 1: with senses ablated the
phase-one centres score exactly like the cruise control, and the unablated GRU
beats the ablated one on 6/8 (blind) and 8/8 (browser) layouts. The policies
read something, most likely taste/contact ("stop and chew"), not navigation.
Phase one's "7/8 and 8/8 acquired" counted 1e-15 conservation residue; counted
from real bites it is 2/8 and 6/8 on the freed Stage-A start.

P2-B's decisive finding: **the score is capped at 0.25 + maintenance/reference
because founders start full.** A founder is introduced at body_max with a full
reserve, so settled intake can only replace what upkeep and motion burned:
0.31 at 1,200 ticks, 0.37 at 2,400, and the per-seed maxima hit those numbers
exactly. Doing nothing scores 0.25, full cruise costs the same rate as upkeep,
so the optimiser's best move is to stop. Every learning-target cell is unmet
for that reason before sensing is even in question. Stage B: no controller
depleted the first patch (0/8 everywhere).

## Package P2-C: start founders hungry, rerun the four pilots

Owner: the P2-B worker, resumed with its context. High effort. This time the
fauna crate's `IntroduceFounder` and the founder tick's first-sample feedback
are inside scope; nothing else in fauna is.

1. **Introduce founders below their stores.** Add a starting-stores parameter to
   `Command::IntroduceFounder` (body fraction of body_max and reserve fraction of
   the full reserve; both validated, body ≥ body_min). Arenas introduce both
   founders at body 0.5·body_max and reserve 0. Keep the manifest references
   unchanged (they are the schema's fixed normalisers, not the start state). The
   live schedule's existing introductions keep their present full start. The ES
   protocol hash changes; earlier centres are refused.
2. **First sample stays zero.** P2-T finding 3: ticks 1..4 of maintenance before
   the first controller sample flow into the first feedback and read zero today
   only because the start is full. Make the plan's rule hold from a depleted
   start (initial intake/loss/motion feedback is zero at the first sample), with
   a short test.
3. **Check the cap before training.** Compute and report the new score ceiling
   per founder and stage (headroom ≈ half the body plus the whole reserve, in
   reference units) and the rest-until-death time from the depleted start at
   each horizon. If a resting founder dies inside the horizon, say so; do not
   tune it away, the survival term is now allowed to matter.
4. **Rerun the four pilots** (Stage A and B, both founders) on the fixed stencil
   and depleted start, same bounds as P2-B, and evaluate with the six
   controllers. Stage B: report depletion and reacquisition counts.
5. **Return (≤40 lines):** ceilings and death times, the four held-out tables,
   per-cell learning-target verdict with the ablation and cruise evidence,
   commits, commands, and one next change with its evidence.

Decision authority: the starting fractions if 0.5/0 turns out lethal at rest
inside a horizon (state what you chose and why), test fixtures. Not yours: the
score formula, manifests, action set, patch sizes and horizons from P2-B.

## Integration note 2 (Fable, 2026-09-19, at 554a4c7)

P2-C landed (6dd4b39, 554a4c7); workspace suite 1,915 passed, 1 skipped.
Hungry start (0.5·body_max, reserve 0) raised the score ceiling from 0.31/0.37
to about 1.3; rest-until-death is 693 s blind and 1,609 s browser, so nothing
dies inside the horizons yet but death is reachable. Held out (mean / median /
acquired of 8):

| stage founder | gru | gru-ablated | heuristic | cruise |
| --- | --- | --- | --- | --- |
| A blind | .265/.237/3 | .236/.236/0 | .248/.235/4 | .253/.236/1 |
| A browser | .373/.333/5 | .259/.238/4 | .489/.519/8 | .276/.263/4 |
| B blind | .340/.217/3 | .256/.219/2 | .201/.182/2 | .237/.223/1 |
| B browser | .403/.256/4 | .244/.220/1 | .461/.474/8 | .228/.223/1 |

Sensing is now demonstrated for the browser on both stages and for blind B
(ablated rows are constant across layouts; unablated wins by 0.11–0.16 mean).
Blind A is the weak cell. All four cells still miss the ≥6/8 acquisition
clause; per-layout scores are bimodal (about 0.22 when food is never reached,
0.5–0.75 when it is). Stage B: first depletions ever (1/8 blind, 3/8 browser),
reacquisition 0/8 everywhere. The browser heuristic acquires 8/8 with the same
inputs, so for the browser the budget is provably attainable and the gap is
search. Every training run used 1.6–3.7 s of a 900 s cap and selected a
generation at or near the last, so the ES is starved by its own bounds.

## Package P2-D: feed the search, then read the bimodality

Owner: the same worker, high effort. Files: `crates/cubarium-search/src/es/voxel/*`,
`crates/cubarium-search/src/main.rs`, arena layout-seed lists in `task.rs`
only. No fauna, flora or arena model-rule changes.

1. **Raise the ES bounds.** `DEFAULT_PAIRS` 8 → 32, `MAX_UPDATES` 64 → 512,
   training layouts 4 → 16 fresh seeds (the held-out 8 stay untouched and
   disjoint). Keep the wall cap at 900 s per run and the 16-worker cap; if a
   run would exceed the cap at the measured episode rate, reduce updates first
   and say so. The per-generation centre evaluation stays on.
2. **Rerun the four pilots** and evaluate with the six controllers, same
   tables as P2-C, plus Stage-B depletion and reacquisition counts. Report the
   selected generation and whether the curve had flattened (last 64 updates
   within the perturbation spread) or was still rising.
3. **Read the bimodality.** For each cell, from the held-out rows and the
   fixture-side geometry (start distance, heading offset from the bearing to
   the initial patch, patch side), say what separates the found-food layouts
   from the rest. Evaluator-side only; nothing enters an observation. If one
   geometric factor explains most misses, name it and the arena change that
   would test it; do not make that change.
4. **Return (≤40 lines):** run costs, the four tables, per-cell verdict on the
   tests-plan §4 target with the ablation and cruise evidence, the bimodality
   reading, commits, commands, one next change with its evidence.

Decision authority: exact seed lists, whether to spend remaining wall time on
more updates or more pairs, the flattening criterion. Not yours: score,
manifests, action set, start stores, patch sizes, horizons.

## Integration note 3 (Fable, 2026-09-19, at 387f66c) — phase two closes

P2-D landed (77d92fa, 387f66c); workspace suite 1,917 passed, 1 skipped. Fable
re-ran the browser-A and blind-A held-out evaluations from the saved centres
and reproduced the worker's numbers exactly. Four runs of 512 updates, 32
pairs, 16 training layouts cost 110–411 s each at 16 workers; all curves flat.

Held out (mean / median / acquired of 8; stationary and no-intake 0.25 / 0):

| stage founder | gru | gru-ablated | heuristic | cruise |
| --- | --- | --- | --- | --- |
| A blind | .733/.903/6 | .222/.222/0 | .248/.235/4 | .253/.236/1 |
| A browser | .992/1.002/8 | .243/.218/1 | .489/.519/8 | .276/.263/4 |
| B blind | .441/.246/4 | .214/.214/0 | .201/.182/2 | .237/.223/1 |
| B browser | .728/.728/8 | .241/.241/0 | .461/.474/8 | .228/.223/1 |

**Learning target (tests plan §4) MET for Stage A blind, Stage A browser and
Stage B browser; unmet for Stage B blind** (4/8, median just under
stationary). Ablated rows are identical across layouts in every cell: without
senses each policy is one fixed trajectory. Phase one's gap was search, not
sensing: the same manifests, bodies and score, with an honest start and a
fed trainer, forage at 55–79% of the ceiling.

Still open, with evidence:
- **Reacquisition 0/8 everywhere.** The browser empties its first patch 8/8
  (ticks 1005–1885) and then parks. Depletion lands at 42–100% of the horizon,
  not the 25% the Stage-B sizing assumed, so the second leg has too little
  time and the reward needs a conjunction a perturbation rarely produces.
  Recommended: halve the Stage-B initial patch (blind 0.015 → 0.0075; browser
  through foliage, keeping the crown), then rerun Stage B.
- **The blind founder learns a one-handed opening arc.** Blind-A misses are
  the two smallest start offsets; blind-B hits are all on one side. A
  stratified offset family (one layout, starts swept −180°..+180° in 15°
  steps) would measure acquisition versus offset directly, and stratifying the
  held-out eight into signed offset bins would stop a one-handed policy passing
  by luck.
- Survival is still constant at these horizons (rest-to-death 693 s / 1,609 s).

Not started, by decision: live-world transfer of the founder bodies, genome
linkage of appendages, Tilt, vibration, the remaining roles. Wrysk decides the
next slice.

## Phase-three diagnostic follow-up (Codex, 2026-09-20, at ee607a2)

The two recommendations above were implemented as one bounded diagnostic:
Stage B now starts with half the successor patch's edible resource (blind
0.0075 versus 0.015; browser foliage 0.03 versus 0.06, with wood and crown
unchanged), the held-out eight are balanced across signed near/far offset bins,
and `--set offset-sweep` evaluates one blind layout from -180 to +180 degrees
in 15-degree steps. The Stage-B arena protocol is versioned so equal-patch
policies cannot be evaluated as if they used this geometry.

Fresh 512-update, 32-pair, 16-worker Stage-B runs completed without discarded
episodes. Blind selected generation 494 after 385 s; browser selected 505 after
260 s. On the balanced held-out eight:

| founder | score mean / median | acquired | initial depleted | successor bitten | reacquired |
| --- | --- | --- | --- | --- | --- |
| blind | .327 / .236 | 3/8 | 1/8 | 0/8 | 0/8 |
| browser | .478 / .489 | 8/8 | 7/8 | 0/8 | 0/8 |

The blind offset sweep acquired 5/25: negative near 2/6, negative far 1/6,
positive near 0/7 and positive far 2/6. Its balanced gate is unmet, so the
one-sided/angle-banded weakness remains visible rather than being hidden by the
held-out sample.

The smaller patch worked mechanically but did not teach the transition. Browser
depletion moved earlier and became common, yet the learned controller still
parks at the first patch and never bites the successor. The next bounded test
should therefore isolate and reward post-depletion departure and successor
contact (or stage that transition as its own curriculum) before another full
Stage-B search. No live-world transfer or broader ecology work is implied by
this result.

## Integration note 4 (Fable, 2026-09-20, at f9819c2) — why nobody reacquires

P3-A is on main and reproduced exactly (browser gen505: acquired 8/8, initial
depleted 7/8 at ticks 825–1420, successor bitten 0/8, motor ≤ 0.0033 for the
whole episode; blind gen494: 3/8, 1/8, 0/8). Three measurements explain the
zero, none of which is timing:

1. **The litter cue does not reach the successor.** Settled field response
   `q = cue/(cue+1)` on a flat support, one 0.015 patch (the successor stock),
   by distance: 0 m 0.17, 0.25 m 6.7e-2, 0.5 m 1.5e-2, 0.75 m 3.5e-3, 1.0 m
   8.3e-4, 1.25 m 1.9e-4, 1.5 m 4.2e-5, ≥ 1.75 m exactly 0 (below the field's
   1e-5 discard). A 0.2 Stage-A tile reaches 1.75 m. Half-life 2 s and 0.4
   diffusion give the field a decay length of about 0.2 m; that is the model
   as designed (plan §"Chem", half-life 2 s) and is not changed here.
2. **The cone stops at 2.0 m** (`cone_range_m`), and a stripped crown reads as
   an occluder, not foliage, so the browser does know its patch is empty.
3. **The successor is placed at ≥ 2 m** (`arena_distance_squared ≥ 64`, up to
   4.85 m). From the depleted patch, therefore, neither founder can sense the
   successor at all. Reacquisition as built is an undirected search rewarded
   only on contact, and an ES perturbation of a parking policy that walks a
   little, finds nothing and pays motor is pushed back to parking.

Controls on the P3-A held-out eight, Stage B (this build): browser heuristic
takes 0.0035–0.0135 off the first patch, wanders, and bites the successor in
6/8 (first bites at ticks 280–1875) without depleting anything; browser cruise
0/8; blind heuristic bites the successor 2/8, the first patch 0/8; blind
cruise 1/8. So the second patch *is* findable by an observation-only wander
for the browser inside the horizon; what the GRU never learns is to leave.

The plan's own words for Stage B are "reach a second **nearby** cue patch" and
"reach another **visible** patch", and it warns not to "demand that blind
animals solve scent-free maze navigation before their first reward". The
landed arena asks exactly that. The next package trains the transition as a
curriculum inside the sensed radius first, then widens to the landed task,
without touching senses, score or the held-out task.

## Package P3-B: the sensed transition (curriculum, not shaping)

Owner: one Opus 5 worker, high effort. Files: `crates/cubarium-voxel-sim/src/arena.rs`,
`crates/cubarium-search/src/es/voxel/{task,driver,trainer,commands,store}.rs`,
`crates/cubarium-search/src/main.rs`. Read integration notes 1–4 and the P3-A
follow-up above first. Not in scope: `senses.rs`, `body.rs`, manifest ranges,
the score, any observation channel, any reward term, the held-out seeds.

Steps, in order, each committed by explicit path:

1. **Fixture-side accounting.** Extend `Reacquisition` (driver.rs) with the
   wrapped initial→successor `separation_m`, the founder's closest approach to
   the successor after the depletion tick (`min_successor_distance_m`, none
   when never depleted), and `sense_ticks`: ticks after depletion spent within
   the founder's sensed radius of the successor (blind 1.5 m from measurement
   1 above, browser 2.0 m = `cone_range_m`; declare both as named constants
   in task.rs with that provenance). Print them per row and summarised in the
   Stage-B table. Nothing here may enter an observation or the score.
   Re-evaluate the two P3-A centres (blind gen494, browser gen505) with it.
2. **Separation band.** Stage B gains a band: `near` places the successor with
   `4 ≤ d² < 36` columns (1.0–1.5 m, inside both senses); `landed` keeps
   `d² ≥ 64`. `Arena::build_reacquisition` stays `landed`. The band is part of
   the Stage-B arena protocol string and the trainer's protocol hash, so a
   `near` centre is refused by an unqualified `landed` evaluation. Held-out
   and training seeds are unchanged; a short test proves every training and
   held-out `near` layout satisfies the band and keeps its start off food.
3. **Warm start.** `voxel-train --init-center <centers/genN-center.json>`:
   same founder and shape, fresh Adam state, the new run's own protocol hash;
   the checkpoint records the source file, its `weights_fnv1a` and its
   protocol hash as provenance. Refuse a centre of the other founder.
4. **Runs** (32 pairs, 512 updates, 16 layouts, 16 workers each; two runs may
   share the machine): for each founder, chain A-centre → B-near → B-landed,
   starting from `runs/voxel-es-p2d-browser-a/centers/gen504-center.json`
   and `runs/voxel-es-p2d-blind-a/centers/gen508-center.json`. As the
   curriculum's own control, also run A-centre → B-landed directly (no near
   rung). Six runs; keep them under `runs/voxel-es-p3b-*`.
5. **Report** every rung on the landed held-out eight (the near rung also on
   its own near held-out): score mean/median, acquired, depleted, successor
   bitten, reacquired, `min_successor_distance_m`, `sense_ticks`. The
   question the table must answer: does the near rung teach departure, and
   does departure survive the widening to ≥ 2 m?

Decision authority: the exact band bounds within the stated radii, the CLI
and protocol encodings, and whether the near rung's held-out set needs its
own balance check. Escalate by returning early if step 1's accounting shows
the P3-A browser centre already coming within 2 m of the successor after
depletion (that would falsify measurement 3 for the layouts in question).

Verification: `cargo nextest run -p cubarium-search -p cubarium-voxel-sim`
green; tests ≤ 200 ticks; no bit-identical pins. Do not edit
`design/handoffs/README.md` or this file. Return ≤ 40 lines: commits, the
table, and the evidence for each conclusion. Fable integrates and runs the
workspace suite.
