---
status: open
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
