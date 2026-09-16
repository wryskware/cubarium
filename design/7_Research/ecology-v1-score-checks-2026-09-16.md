---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# The score hypothesis, tried twice: the controller ignores food, and the score already pays
# enormously for staying

Workstream L of the ecology v1 round-3 next steps
([brief](../handoffs/ecology-v1-score-checks-opus-2026-09-16.md)), step 2 of the reconciled next
steps, from Astra's [round-2 review](ecology-v1-round2-review-2026-09-16.md) (P2 on H, next steps
item 2).

The per-tick intake diagnostic ([H](ecology-v1-intake-2026-09-16.md)) localised generation 9's
failure to **residence**: it stands on food on 8.3 % of its ticks against the surviving mobile
script's 95.2 %, its mouth is open on every one of those ticks, nothing clamps the bite, and
every departure happens with the cell still above the feeding threshold. H then inferred — from
three measured refusals, not from a measurement — that the trainer's score
`t_min + 0.25·stores` is what fails to pay for staying. Astra asked for that inference to be
attacked from two sides before it became a design change.

**One of the two attacks landed.** The controller really does ignore the food channel (check (a)
leaves H's phenotype finding standing and adds that nine generations of training have not moved
its food sensitivity at all). But the current score is **not** flat in residence: on the same
twelve layouts, moving a scripted forager from a 1-second dwell to a 50-second dwell is worth
**+12,510 ticks of `t_min` on the training four and +14,411 on all twelve** — a 53 % and a 111 %
increase in the score, against a stores tiebreak whose whole range is 0.25 ticks. The
missing-gradient diagnosis, as stated, is wrong.

Per the brief's own rule, **no score change is proposed**. The proposal document is written as
[*not proposed*](../forager-score-proposal-2026-09-16.md), with the constants Astra asked to see
fixed, the ladder under the proposed `S`, and the falsifier that fired.

## Build and provenance

- Code and tests, with the auxiliary's constants fixed **before** any run: `14345d9`.
- This note and the proposal: the commit that carries them.
- Search build id **`14345d9`**, pinned with `CUBARIUM_SEARCH_BUILD` (three workers share one
  `target/` and the build script's `HEAD` watch can bake another worktree's commit).
- Ecology `runs/ecology-v1-calibration/selected/fast-leaf.toml`, config hash
  `09e244392ec91768`. Policy `runs/es-eco-v1-fastleaf/selected/center-00009-policy.json`
  (generation 9), and `tensor::initial_center(20_260_915)` as the untrained contrast.
- Outputs under `runs/ecology-v1-score-checks/` (git-ignored by design): `sweep.json` 488 KiB,
  `ladder.json` 297 KiB — **0.77 MiB** against the brief's 30 MiB cap.
- Wall time: **22.3 s** of simulation (2.6 s for check (a)'s 24 episodes, 19.7 s for check (b)'s
  108 episodes, 8 workers) against the brief's 8-minute cap.
- `cargo test -p cubarium-core`: **502 passed, 0 failed, 4 ignored**.
  `cargo test -p cubarium-search`: **199 passed, 0 failed, 3 ignored** (H's experiment and L's
  two).

### Exact commands

```bash
cargo test -p cubarium-search --lib scorecheck
cargo test -p cubarium-search --test score_checks

CUBARIUM_SEARCH_BUILD=14345d9 \
  cargo test -p cubarium-search --release --test score_checks -- --ignored --nocapture
```

Both experiments are `#[ignore]`d tests rather than `es-*` subcommands, for the reason H
recorded: three other workers held `crates/cubarium-search/src/main.rs` open in worktrees for the
duration, and a subcommand is the one edit that would have collided. `scorecheck::run_sweep` and
`scorecheck::run_ladder` are public, so promoting them is a one-liner. Each reads its defaults
from the environment, so one row can be re-run without editing anything:

```bash
CUBARIUM_SCORECHECK_DWELLS=1000 CUBARIUM_SCORECHECK_WORKERS=1 \
CUBARIUM_SCORECHECK_LADDER_OUT=runs/ecology-v1-score-checks/recheck.json \
  cargo test -p cubarium-search --release --test score_checks -- --ignored the_dwell_ladder --nocapture
```

### Independent agreement with H

Nothing here reuses H's numbers, and three of them come back to the digit from a separately
written measurement path. Generation 9's on-food fraction is **0.116** on `t1-corridor` and
**0.085** on `h1-holdout` (H: 0.116, 0.085); its survival is 9,429 and 8,560 ticks (H: 9,429,
8,560); the mobile script's on-food fraction on `t1-corridor` is **0.985** (H: 0.985). The
`no-intake` control dies at 7,420 ticks, the value `episode.rs` has pinned since R2a.

## Check (a) — the frozen controller's response to food

### What was varied, and what was held

For each of the twelve `fast-leaf` layouts, generation 9 was run through the ordinary episode
(horizon 36,000) and the world's **own** observation (`World::neural_observation`) plus the
animal's **own** hidden state were recorded at every tick preceding a controller update. Sixty‑four
ticks per layout were then kept — thirty-two on food and thirty-two off it, evenly spaced through
the life — giving **768 sampled ticks** per driver.

Each sampled observation was then re-run through the core's own `Gru32::forward` and
`Action7::squash` (no copy of either lives in this workstream) with exactly one edit:

- **the local food scalar**: `v[0] = P_here/P_max` driven to 0, 0.25, 0.5, 0.75, 1.0, every other
  one of the 70 scalars untouched;
- **the ring food sectors**: `v[3..39]` (six body-frame sectors × three channels, near and far
  rings) overwritten with *nothing in range*, *a full foliage reading dead ahead* (sector 0), or
  *the same behind* (sector 3), with `v[0]` left as recorded.

Each variant was run twice: from a **reset** hidden state (zeros, as at birth) and from the
**carried** hidden state the trajectory actually held at that tick. The untrained centre was run
identically, as a contrast.

### The reconstruction residual, stated before the effects

The recorded pair is not bit-identical to what the controller consumes. The world runs weather,
water, the field reactions and the pair pass **before** it observes and decides
(`crates/cubarium-core/src/world/step.rs`, stages 2–5), so an observation sampled at the close of
tick `t` is one tick of plant growth older than the one the update at `t + 1` reads. Measured
over **94,420** update ticks across all 24 episodes, the largest disagreement between the action
reconstructed from the recorded pair and the action the world went on to hold is
**1.84 × 10⁻⁵**, and it is below 10⁻⁵ on ten of generation 9's twelve layouts. The two movement
effects reported below are **59×** and **160×** that worst case, so they are real rather than
sampling artefacts — and they are still negligible against the action's own variation, which is
the point. The one effect that is *not* clear of the residual is the fruit channel's
7 × 10⁻⁵, about 4× it; read that row as zero.

### The numbers

Generation 9, pooled over the twelve layouts, 768 sampled ticks. `Δ` is the **within-tick**
change from `v[0] = 0` to `v[0] = 1`, averaged over samples; `sd` is the standard deviation of
the action the trajectory **actually held** over its 50,048 update ticks — the natural variation
an effect is measured against.

| channel | held mean | held sd | Δ(v₀: 0→1), reset | Δ, carried | Δ/sd, reset | Δ/sd, carried | ring span/sd, carried |
| --- | --- | --- | --- | --- | --- | --- | --- |
| thrust | 0.5241 | 0.00908 | −0.00106 | −0.00109 | **0.117** | **0.120** | 0.088 |
| turn | −0.0729 | 0.04569 | 0.00000 | −0.00294 | **0.000** | **0.064** | **0.162** |
| graze | 0.3239 | 0.00302 | +0.00061 | +0.00059 | 0.203 | 0.194 | 0.223 |
| fruit | 0.3254 | 0.00306 | −0.00007 | −0.00004 | 0.023 | 0.014 | 0.146 |
| scavenge | 0.3506 | 0.00302 | −0.00054 | −0.00054 | 0.180 | 0.180 | 0.077 |

The on-food and off-food strata agree to the third decimal of every entry; splitting them changes
nothing, which is itself a finding — the response does not depend on where the body is standing.

The ring arms, carried hidden state, pooled:

| channel | nothing in range | food dead ahead | food behind | span/sd |
| --- | --- | --- | --- | --- |
| thrust | 0.5229 | 0.5234 | 0.5226 | 0.088 |
| turn | −0.0497 | **−0.0568** | −0.0494 | **0.162** |
| graze | 0.3262 | 0.3257 | 0.3264 | 0.223 |

### Verdict: **the policy's movement does not respond to food.**

The response has the **right sign** and is **three orders of magnitude too small to matter**.
Filling the cell under the body from bare to a full stand:

- reduces thrust by **0.00109**, from 0.5232 to 0.5221 — 0.2 % of the channel, 12 % of the
  action's own standard deviation. The adapter stops the body when thrust falls below its 0.05
  deadband. Generation 9 would need **about 430 times** its measured sensitivity for a full stand
  of food to stop it;
- raises the grazing effort by **0.00059** — on a channel that H already showed is pinned at one
  third by the shared-mouth normalisation;
- turns the body by **0.00294** of the turn channel against a natural spread of 0.0457.

Food *dead ahead* against food *behind* moves the turn channel by 0.0074 — **0.16 sd**, the
largest single effect anywhere in the sweep, and still a sixth of the turning the policy does
anyway for its own reasons.

Two further facts the sweep makes plain:

- **From a reset hidden state the turn channel is identically zero**, at every food level, for
  both policies: the raw turn head sits inside the adapter's ±0.05 deadband, so a freshly born
  animal requests no turn at all whatever it is shown. All the turning in the trajectory comes
  from the recurrence, not from the observation.
- **Nine generations of training have not made the controller more sensitive to food.** The
  untrained centre's thrust response to the same sweep is **−0.00097**; generation 9's is
  **−0.00109**. In absolute terms training moved the food sensitivity of the movement head by
  about 12 %, on a quantity that would need to grow 430-fold to change the behaviour. (Relative
  to its own natural variation the untrained centre looks *more* responsive — 0.256 sd against
  0.120 — only because its action is 2.4× less variable overall.)

So check (a) does **not** weaken the score hypothesis: it confirms H's phenotype finding at the
level H said it had not measured. The policy is shown its own cell's food on every tick and does
not use it, and that is a property of the weights, not of the observation.

## Check (b) — the current score's dwell gradient

### The control family

`Control::Dwell(d)` is the disclosed mobile script with **one rule replaced**: leave a route cell
after exactly `d` ticks standing on it, whatever is left in it, instead of when its `P` falls
below the world's own `feed_min`. The route order, the heading request, the full grazing effort
and the paid motion through `motor::resolve` are the script's, unchanged. The top rung of the
ladder — "stay until below threshold" — *is* `Control::MobileScript`, so the ladder ends on the
driver H already measured rather than on a reimplementation of it.

The residence rule is pinned from outside the implementation, from the body's own post-step cell
and resolved travel: a stay is a maximal run of zero-travel ticks, its arrival is the moving tick
before it on the same cell, and every completed stay is **exactly `d` ticks on the cell** — one
arrival tick, which already feeds from the cell because the feeding settlement runs after the
move, plus `d − 1` ticks standing still.

Every rung ran on all twelve layouts at horizon 36,000, scored with `es::trainer::score` itself —
the same function the trainer calls, checked from outside against `t_min + 0.25·stores`.

### The ladder under the current score

`train-4` is the four training layouts, which is what the trainer optimises; `all-12` adds the
eight held-out ones.

| rung | set | alive | `t_min` | mean ticks | mean stores | **current score** | on food | served (m) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| dwell-20 | train-4 | 3 / 4 | 23,490 | 32,872 | 0.5620 | **23,490.140** | 0.768 | 12.48 |
| dwell-100 | train-4 | 3 / 4 | 24,269 | 33,067 | 0.5622 | **24,269.141** | 0.849 | 11.77 |
| dwell-300 | train-4 | 3 / 4 | 27,559 | 33,890 | 0.5624 | **27,559.141** | 0.899 | 11.53 |
| dwell-1000 | train-4 | 4 / 4 | 36,000 | 36,000 | 0.6263 | **36,000.157** | 0.906 | 12.09 |
| dwell-3000 | train-4 | 2 / 4 | 18,309 | 29,504 | 0.0823 | **18,309.021** | 0.693 | 7.59 |
| until-threshold (mobile script) | train-4 | 4 / 4 | 36,000 | 36,000 | 0.7222 | **36,000.181** | 0.969 | 12.23 |
| dwell-20 | all-12 | 6 / 12 | 12,951 | 28,382 | 0.3323 | **12,951.083** | 0.647 | 10.08 |
| dwell-100 | all-12 | 6 / 12 | 16,658 | 29,063 | 0.3733 | **16,658.093** | 0.712 | 9.97 |
| dwell-300 | all-12 | 6 / 12 | 21,370 | 30,983 | 0.3703 | **21,370.093** | 0.794 | 10.20 |
| dwell-1000 | all-12 | 11 / 12 | 27,362 | 35,280 | 0.5328 | **27,362.133** | 0.876 | 11.74 |
| dwell-3000 | all-12 | 6 / 12 | 18,309 | 31,108 | 0.0823 | **18,309.021** | 0.799 | 8.14 |
| until-threshold (mobile script) | all-12 | 11 / 12 | 26,355 | 35,196 | 0.6188 | **26,355.155** | 0.941 | 11.76 |

For context on the same axis, the three drivers that are not ladder rungs:

| driver | set | alive | `t_min` | current score | on food | served (m) |
| --- | --- | --- | --- | --- | --- | --- |
| stationary-grazing | all-12 | 0 / 12 | 9,090 | 9,090.000 | 0.046 | 0.96 |
| no-intake | all-12 | 0 / 12 | 7,420 | 7,420.000 | 1.000 | 0.00 |
| generation 9 | all-12 | 0 / 12 | 6,914 | 6,914.000 | 0.082 | 0.79 |

### Verdict: **the current score is strongly increasing in dwell. The hypothesis is falsified.**

- Across the whole feasible range of residence — `d` = 20 → 100 → 300 → 1,000 ticks — the score
  rises **monotonically and by a lot**: `23,490 → 24,269 → 27,559 → 36,000` on the training four
  (**+12,510 ticks, +53 %**) and `12,951 → 16,658 → 21,370 → 27,362` on all twelve
  (**+14,411 ticks, +111 %**). Survivors go from 6/12 to 11/12.
- The gradient is not marginal against anything in the scoring function. The stores tiebreak's
  entire range is **0.25 ticks**; the auxiliary Astra proposed would have a range of **200
  ticks**; the residence gradient the *current* score already carries is **14,411 ticks**, nearly
  five orders of magnitude larger than the tiebreak and seventy times larger than the proposed
  auxiliary.
- It turns down only at `d = 3,000` (150 s), and for a legible reason rather than a scoring
  artefact: on `t2-weak-open` a body that stands on the weak opening for 150 s crops it to
  nothing and then starves on it (on-food 0.122 on that layout, against 0.783 at `d = 1,000`).
  The ladder is therefore **unimodal with an interior optimum near `d ≈ 1,000`**, which is what a
  well-shaped objective over residence should look like — including a penalty for over-staying.
  From where generation 9 sits, the direction is unambiguous.
- The score is not flat at the bottom of the range either, where generation 9 actually lives:
  eating one cell and then starving (`stationary-grazing`, 9,090) outscores eating nothing at all
  (`no-intake`, 7,420) by **1,670 ticks, 22 %**. A single cell's worth of food is worth 1,670
  ticks of score.

The claim that "nothing the search scores pays for staying" is refused by the measurement. What
generation 9 is missing is worth between 1,670 and 14,411 ticks of the score it is already being
optimised against, and it is not collecting any of it.

### What check (b) does not isolate

Every rung of the ladder is already a perfect navigator: it walks the declared route and stands
only on food. The ladder therefore measures the score's gradient **in residence, given
route-following**, and it does not separate "stay longer" from "go to food at all". Generation 9
is not a route-follower — H measured it spending 37 % of its life on faces with no food painted
on them — so the ladder does not measure the score's gradient in the immediate neighbourhood of
generation 9's own behaviour. That neighbourhood is the one place the score hypothesis could
still be resurrected, and measuring it needs a different experiment (below), not this one.

## What the evidence points at instead

The two checks together say something sharper than either alone: **the score already pays,
enormously, for the behaviour that is missing, and the frozen controller's one-step response to
food is far below the adapter's deadband.** (Corrected after Astra's round-3 review, P1: the
first version said the controller "cannot express" residence and that the search "is not
converting" the gradient "into any movement in parameter space". The one-forward-pass sweep at
sampled states does not test sustained food input, recurrent integration across updates, a
trajectory under the altered action, or nearby weight perturbations, so neither claim follows.
And the retained generation reports already hold the spread candidate 1 below asked for:
`GenerationReport` persists the 32 ordered candidate scores and every candidate-layout episode;
in generation 9 they span 6,459–8,915 ticks, sd 643, and candidate score correlates r = 0.81
with mean producer intake across the four training layouts and r = 0.54 with mean ticks in the
opening. Perturbations do produce material score and feeding variation. The open question is
therefore **where useful candidate variation is lost** — in the centred-rank reduction across
the four-layout minimum, in the update, or in the mapping from weights to residence — not
whether it exists.)

Three candidates, in the order their cost says to try them, none launched:

1. **Measure the score spread across candidates inside a generation** (cheapest; no new training
   run — the checkpoints already hold it). Sixteen antithetic pairs over 10,215 parameters at a
   fixed `σ`. If the 32 perturbed candidates' `t_min` values sit within a few hundred ticks of the
   centre's, the perturbations are not producing behavioural variation the centred-rank
   reduction can exploit, and `σ`, the population size or the parameterisation is what binds —
   not the score's shape. This is the one measurement that would distinguish "the objective is
   wrong" from "the optimiser cannot move".
2. **Measure how much of the action is inside the adapter's deadband under a `σ`-scale
   perturbation.** Under a reset hidden state both frozen policies request *exactly zero* turn,
   because the raw turn head is inside the ±0.05 deadband. If a `σ`-sized perturbation of the
   weights usually leaves it there, the turn channel is invisible to the ranking for most
   candidates, and a search that cannot vary turning cannot discover residence. Frozen weights
   plus the existing sampler answer this without simulating anything.
3. **The score's gradient in generation 9's own neighbourhood** — the gap check (b) leaves open.
   A ladder of scripted controls interpolating between generation 9's measured behaviour (wander,
   93-tick dwells, 8 % on food) and the route-follower would say whether the score is flat
   *there* even though it is steep among route-followers. This is the experiment that would
   revive the score hypothesis if anything can, and it is strictly more expensive than 1 and 2.

H's secondary item stands unchanged and is still not the next task: both policies pin their three
mouths at exactly one third, which costs about 3× on the bite and becomes worth fixing after
residence, not before.

## What this does not establish

- **Nothing about a whole world's foraging.** Twelve frozen single-body layouts, one ecology, one
  horizon, one config hash.
- **Nothing about the score's gradient in parameter space.** Check (b) measures the score over a
  scripted *behaviour* family. A strong gradient in behaviour space is necessary for the search
  to have something to climb; it is not sufficient, and this workstream did not measure whether a
  `σ`-sized weight perturbation moves behaviour at all.
- **Nothing about the score's gradient near generation 9's own behaviour**, as set out above.
- **Nothing about what the proposed auxiliary would do to a training run.** It was computed, not
  optimised. No candidate was ever ranked by it.
- **Nothing about the observation or the adapter being adequate.** Check (a) says the policy does
  not use `v[0]`; it does not say a policy *could* use it well enough, and the reset-state
  deadband finding hints that the turn channel's operating point may itself be a problem.
- **Nothing about a longer horizon, a second ecology or a different genotype.** One of each.
- The recorded observation is one tick of field dynamics older than the controller's exact input
  (residual ≤ 1.84 × 10⁻⁵, quantified above). The sweep's baselines are real observations the
  world's sampler built on this trajectory, not the exact vectors the controller consumed.
- `cargo clippy -p cubarium-core --all-targets` still fails on the pre-existing `erasing_op`
  deny in `crates/cubarium-core/src/neural/gru.rs:233`, untouched by this workstream.

## The next task this implies

Named, not launched (revised after review; candidate 1's spread is already in the retained
`generations.jsonl` and is not a new task): **the antithetic-pair reduction on the retained
generation reports** — for each plus/minus pair, relate the score difference to intake, opening
residence and the signed contribution to the centred-rank update; then, only if on-food time is
needed, reconstruct generation 9's 32 candidates and run H's residence trace on the four training
layouts. Confirmation of an optimiser/update problem: individual perturbations with better
residence and score that cancel in the centred-rank gradient or are erased by the update.
Refutation: no candidate-level increase in on-food residence despite the score spread, which
moves attention to parameterisation, recurrence, cadence or the adapter. Beside it, candidate 2
(deadband occupancy under a σ-scale perturbation) stands. The score stays as it is.
