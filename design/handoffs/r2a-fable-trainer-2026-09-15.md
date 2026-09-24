---
design_status: exploration
last_reviewed: 2026-09-15
---

# Fable: R2a evolution-strategy trainer and plumbing smoke

Implement this bounded assignment in a fresh thread in
`/home/wrysk/wryskware/cubarium`. Read repository working rules, canon rules/ledger,
the implemented
[interface contract](../recurrent-interface-contract.md), and the final verification
in the [R1a review](../7_Research/r1a-runtime-review-2026-09-14.md).

The optimizer choice for this implementation is **antithetic Gaussian evolution
strategies with centered-rank utilities**, a fixed perturbation scale, and an Adam
ascent update of the central policy. Use this same optimizer in smoke and subsequent
learning. No separate GA, MLP comparison, recurrent PPO implementation or optimizer
survey. This resolves the dispatch's open first-optimizer choice for this slice;
it does not add an accepted canon entry or promise sample efficiency.

Deliver a real-core trainer, validated fixture protocol, deterministic optimizer
checks, one bounded plumbing smoke and an exact proposed first-learning command.
**Stop before the learning campaign.** The compute checkpoint uses the actual
single-animal episode timings, not the old ecosystem-search or 512-body benchmark.

## Research basis and limits

[Salimans et al., 2017, §2.1](https://arxiv.org/pdf/1703.03864) describes mirrored
Gaussian perturbations, rank-shaped returns and fixed noise scale for policy
optimization. Adopt those mechanics, without its distributed infrastructure or
neural architecture changes. Its results do not establish that our 10,215-parameter
GRU will learn in a small population or the proposed wall time. Rank transformation
is an optimization tool, not evidence that the ecological objective is correct.

The exact defaults below are Cubarium starting choices. Do not silently enlarge
the architecture, normalize observations from population statistics, alter ecology
or add action rewards to make the screen succeed.

## 1. Keep one simulator and a separate training entry point

Use the repaired core, its `Policy`/GRU, actual observation builder, held-action
adapter, motor resolver and world lifecycle. No second simulator or copied neural
forward implementation. Evolve all weights/biases in the current tensor order.
Every candidate gets a fresh isolated world, private zero hidden/held/feedback
state and the same defined initial cadence. Do not carry experience between episodes.

Prefer a small additional trainer binary/module within `cubarium-search`, reusing
its build stamping and useful infrastructure. Its existing ecological parameter
search, metrics and command behavior must remain intact: do not force neural weights
into M1's parameter vector or reuse Feeding-mode fitness. A separate small crate is
acceptable only if it materially simplifies ownership/dependencies; explain briefly.
No ML runtime, GPU port or broad new framework is needed.

Use stable `(generation, pair, sign, layout)` job identities, deterministic seeds
and reduction order. Both signs see identical initial layouts and environment seeds.
Scheduling must not affect policy updates. Training RNG and world RNG stay separate.
Use bounded CPU workers with shared cancellation checked inside each episode.

## 2. Pin the optimizer exactly

For `n` perturbation pairs, sample `epsilon_i ~ N(0,I)` and evaluate
`theta + sigma*epsilon_i` and `theta - sigma*epsilon_i`. In each generation rank the
2n aggregate candidate scores, using average ranks for exact ties, then map ranks
to utilities `u = rank/(2n-1) - 0.5` (zero-based ascending ranks).

```
g = sum_i ((u_plus_i - u_minus_i) * epsilon_i) / (2*n*sigma)
theta <- Adam-ascent(theta, g)
```

Document the exact flatten/unflatten order, Gaussian generation and update math.
Use initial `sigma = 0.02`, Adam learning rate `0.01`, beta1 `0.9`, beta2 `0.999`,
epsilon `1e-8`, bias correction, and no implicit weight decay. These are fixed
initial defaults, not tuned values. No adaptive sigma, mutation rescaling or
automatic retries with different hyperparameters in R2a.

Initialize one seeded center with small fan-in-scaled Gaussian matrix weights;
specify the exact scale in the protocol before evaluations. Use zero biases except
documented update-gate retention biases (e.g. fixed groups targeting 10/30/100/300
controller updates via `z = exp(-1/tau)`). No scripted-forager weight initialization.
Starting policies may fail; that is evidence, not a reason for a hidden heuristic.

Check independently: gradient direction/scaling on a small analytic objective,
equal-score utilities (all ties yield zero gradient), mirrored seeds, Adam state
continuation, tensor round-trip, finite values and deterministic serial/parallel
reduction. Validation failure/NaNs/invariant failures are experiment errors, not
low biological fitness. Cancel an incomplete generation without updating the center.
Ordinary organism death is a valid completed episode with a recorded survival time.

## 3. Prove the foraging fixture asks the right question

One ordinary mature grazer of fixed genotype, identical stores/body/config across
candidates, births disabled equally in these isolated capability episodes. Retain
current pace, energy prices, repaired starvation/age rules, sensing costs, real
fields and renewal. Account for fixture material/energy inputs. No floor, regrowth
change, care replenishment, respawns or scripted motion for a learned candidate.

Define four deterministic training layouts with distinct headings/patch geometry,
local food cues, a finite opening patch and reachable later food. Freeze their
construction before measurements. Use 36,000 ticks (1,800 s) as the initial horizon.
There must be an accessible opportunity, without giving the policy a destination
or global food information. Include a weak/depleted opening but do not make a
permanently foodless scene a supposedly solvable training layout.

Run three controls per training layout: no intake, stationary continuous grazing,
and a disclosed scripted mobile grazer using paid real motion. This is at most
12 episodes / 432,000 ticks, two workers, 120 seconds total execution including
retries. Positive-control scripts are diagnostic only and never enter candidate
rollouts. Distinguish shortened survival from actual funded survival through the
horizon, and quantify intake, costs and patch depletion rather than mode labels.

Require that starting stores alone cannot survive the horizon, stationary grazing
cannot pass the intended relocation task, and a paid mobile strategy can exploit
the food. If a layout fails, report the exact failure and stop before claiming a
learning-ready fixture; no automatic layout/seed/config search. The trainer and
plumbing smoke can still be completed independently of this ecological prerequisite.

Construct eight separately seeded held-out layouts with the same declared rules.
Keep them out of optimization, mutation calibration and candidate selection. Their
policy evaluations are deferred to the learning assignment. Store fixture/config
hashes and the split so future results cannot quietly redefine the task.

## 4. Score survival, not spectacle

For each candidate record per-layout survival ticks `t_l` and usable terminal
stores `E + e_r*R`. Let `t_min = min_l t_l`. Primary ordering is `t_min`; secondary
is the mean normalized usable terminal stores, clipped to [0,1] using fixed body
capacity, with dead animals contributing zero. One exact rankable scalar is
`score = t_min + 0.25*mean_terminal_stores`: no store bonus can outweigh one tick.
Freeze this convention before the smoke. A survival floor is an evaluation criterion,
not a reward granted to an unfunded body.

Report actual mouth intake, paid upkeep/motion, credits and stores separately.
Use settlement diagnostics and resolved motion, not field-stock loss or net energy
as proxies. Add body-length displacement, distinct cells, time near start and turn
sweep as diagnostics only. Do not reward path length, feeding mode, spin suppression,
raw births or neural action magnitudes. No reward shaping in this assignment.

## 5. Smoke, persistence and the next compute checkpoint

The plumbing smoke exercises ES itself: two perturbation pairs on one training
layout, 2,000 ticks/episode, four episodes. Repeat those exact four with a different
worker count to check deterministic scores/update: **eight episodes total**, at
most four workers and 60 seconds execution including repeats. This checks plumbing,
not foraging or learning. The short smoke need not outlast starting reserves.
It must never evaluate the held-out set or seed a claimed trained founder.

Persist exact central weights, Adam moments/step, perturbation/RNG state, schema,
protocol/config hashes, build, completed generation and counted work. Test resume
at completed generation boundaries. Export the same self-contained `Policy` the
core can attach; verify exact weight round-trip and ordinary core inference. Keep
optimizer checkpoint state separate from an animal's lifetime hidden state.

Prepare, but do not execute, a first-learning command using 16 pairs (32 candidates),
up to 16 updates, four training layouts and the validated horizon. With a 36,000-tick
horizon, perturbation evaluations alone cost at most 2,048 episodes / 73,728,000
ticks. Include evaluations of the initial and every updated center: 17*4 = 68
more episodes / 2,448,000 ticks, for **2,116 episodes / 76,176,000 ticks** before
any separate evaluation. A sampled perturbation's score is not the center's score.
Choose later finalists using training results only.

The proposed learning cap remains at most eight workers and 20 minutes wall time,
with all retries/evaluations counted and cancellation inside episodes. Use measured
single-body throughput to estimate feasibility; reduce the proposed work if needed
and state what will fit, never silently increase the cap. Report control, smoke and
prospective campaign costs separately. Do not assume this cap guarantees learning.

Reserve a later evaluation budget for selected policies on untouched layouts,
memory-use versus hidden-reset diagnostics, longer-horizon behavior and a small
shared-arena transfer check. Specify those in the next brief before execution;
R2a's two-history runtime check is not evidence of learned memory use.

## Deliver and stop

One normal build cache, compact weights and summaries targeted below 10 MiB, a
checkpoint before over 1 GiB extra storage. No per-tick capture archive or retained
population of binaries. Preserve unrelated authored changes; commit only owned work.
No live policy attachment, world reset or automatic display migration. A trainer-only
change requires no runner restart; follow working policy if a necessary core fix
actually changes live behavior, and do not fold in unrelated biology work.

Write `design/7_Research/r2a-trainer-result-2026-09-15.md` with the exact frozen
protocol/commands, implementation and tests, control/smoke results, checkpoint and
export verification, measured compute, proposed learning budget and open failures.
No agent spawning under this brief; at most one targeted review if separately
assigned, two repair cycles, and then the spending checkpoint. Report actual usage
if exposed, otherwise unavailable. Stop after R2a; do not run the proposed campaign,
change optimizer or start R3/M1 automatically.
