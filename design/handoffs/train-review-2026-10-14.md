---
status: open
date: 2026-10-14
owner: Opus (high), main checkout read-only + `.claude/worktrees/train-review` for any tool code
---

# TRAIN-REVIEW: how noisy is a training step, and how should we train (algorithm and speed)

## Rules

Read `design/handoffs/behaviour-ai-worker-rules-2026-10-12.md` first (the 2026-10-12 rules hold).
- Never wrap `scripts/lab/lab` in `timeout`. Quick lane only (`quick = true`, `hosts = ["desktop"]`,
  `cwd` set, under 15 minutes). Anything longer: submit and hand back with the job id. Never wait
  on a job; no tool call over 5 minutes.
- Build in your own worktree's `target/` (reflink-seeded from the main checkout's) at -j 8 or lower.
- Never write "chorister" except where code needs the identifier. The animal is the Veilhound.

## Why (Wrysk, 2026-10-14)

Live twin training (`comparison = "twins"`, `config/training/kin-twins-2026-10-13.toml`: 1024
all-sign pairs × 120 s windows a generation, Adam lr `optimizer::LEARNING_RATE` 0.01) stays flat
for 3 h: lanternjaw `runs/lab/1013-024442-*` (44 gens) and hybrid `*bc92*` (60), seedporter hybrid
`runs/lab/1013-073334-twin-mix-sp-stage1-169a` (33; roost scene .56 -> .06). Brain ≈ rules, a lead
from gen 0 that never grows; update_rms ~0.001. The SP stdout line shows `1024 pairs d=-0.0007
|d|=0.0053 sd=0.0226 zero=0.37`. Update code: `crates/cubarium-search/src/es/voxel/live/trainer.rs`
(`update` ~:395, `update_twins`), `es/voxel/kin/` (`ascend_pairs`, compare), the perturber.
He chose: **measure the step's noise first, then raise the signal per step to fit** (no blind long
run). And: "i wanna also revisit like how we are training though. i mean theres the algorithm we
use and then also optimizations for the runs. both could be subject to improvements."

## Do

1. **Noise of a step.** From the runs above, measure how much of a generation's step is signal:
   e.g. split each generation's pairs into two random halves, form each half's step (the
   perturbations regenerate from seed/generation/pair; reuse the trainer's code, don't
   re-implement it), and report the cosine between halves and between consecutive generations'
   steps, per run. If pair differences aren't logged, add the smallest logging needed and
   measure on a short fresh run (quick lane or a submitted job). From the cosine, estimate the
   pairs (or window length) a step needs to be mostly signal (say cosine 0.5), and what that
   costs in wall time at today's speed. Also: what the 37% zero-difference pairs are (bodies
   that never met the variant's difference? windows too short?).
2. **Run speed.** Where a generation's wall time goes (sim ticks, brain inference, capturing and
   forking twin copies, water, scoring), from logs or a short profile (no stress on the desktop;
   `perf` on one quick run is fine). Ticks/s per world and per core today; obvious wins
   (`fidelity` knobs, water_every, smaller twin worlds, batching inference, eidolon placement).
3. **Algorithm review.** Today's method (antithetic ES on the top layer, all-sign twins, Adam)
   against alternatives, judged for this setup (small nets, a 4 Hz discrete intent choice per
   body, energy returns over minutes, rare decisive events, many bodies per world): more pairs
   or longer windows; variance reduction (baselines/control variates, rank shaping, fewer
   perturbed parameters or a subspace); per-decision policy gradient on the intent choices
   (REINFORCE/PPO with a learned value baseline, per-body advantages, kin-weighted returns);
   anything else the evidence favours. For each: what changes, expected signal gain, cost to
   build, risks (e.g. reward hacking, "never reward behaviour" — rewards stay energy/kin).
4. **Write** `design/7_Research/training-review-2026-10-14.md` (evidence, not decisions):
   the measurements, then 3–5 topics he can decide one at a time, each with options, what he'd
   see, cost, and a recommendation.

## Boundaries

- Analysis and tooling only: no change to training defaults, scores or behaviour. Any logging you
  add is serde-optional and off the plain path (`a_plain_lineage_trains_exactly_as_today` passes).
- Don't merge (the parent merges). EC2 never. Commit explicit paths, never -a. Trailer:
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Return (at most 40 lines)

The noise numbers (per run, how computed), the speed breakdown, the topic list with your
recommendation each, the doc path, commits, job ids still running, departures.
