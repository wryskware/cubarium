---
status: open
date: 2026-10-15
owner: Opus (high), worktree `.claude/worktrees/choice-probe`, branch `choice-probe`
---

# CHOICE-PROBE: does one intent choice change the outcome, and by how much?

## Rules

Read `design/handoffs/behaviour-ai-worker-rules-2026-10-12.md` first (the 2026-10-12 rules hold).
- Make the worktree from main (`git worktree add .claude/worktrees/choice-probe -b choice-probe
  main`), then seed its `target/` with `cp -a --reflink=auto` from the main checkout's. -j 8 or
  lower.
- Never wrap `scripts/lab/lab` in `timeout`. Quick lane only for what you run yourself
  (`quick = true`, `hosts = ["desktop"]`, `cwd` set, under 15 minutes). The probe batch is
  submitted, then you hand back. Never wait on it, never poll it. No tool call over 5 minutes.
- A `command` job whose argv is a local path must pin `hosts = ["desktop"]`. A file a batch arm
  reads goes in as a plain path (labkit stages it); `{job}/inputs/...` paths exist only on a
  remote host.
- Never write "chorister" except where code needs the identifier. The animal is the Veilhound.

## Why

SCORE-V2 (main b394c3f0) ran 3 h on the lanternjaw from its clone:
`runs/lab/1015-081457-score-v2-lj-c838/run/run`, read with
`python3 -I scripts/studies/pg_pilot_read.py --pg <dir> --every 40`.
- The pull toward the clone worked: entropy stayed at the clone's (~0.1 nats), KL from the clone
  ~0.0004, 617 generations. v1 had drifted to uniform.
- Nothing was learned. Split cosine over worlds +.019 ± .011 against a null of +.009 ± .014;
  over bodies +.015 against +.013. The brain-vs-rules gain was +.010 per body at generation 0
  (the clone itself) and stayed between −.000 and +.030 with no trend.
- Only 3% of decisions were sampled off the clone's top choice. For the lanternjaw the judge
  is energy times a constant (no deaths, eggs have no parent), and its diet is 99% one prey.

Two readings, and they need different fixes:
1. **Choices matter but the learner barely tries alternatives** (3% exploration, noisy
   per-decision returns). The fix is more exploration and bigger batches.
2. **One intent choice hardly changes the outcome** over the horizons the learner sees, at
   least for the lanternjaw. The fix is another pilot species, a longer horizon, or a finer
   choice.

Wrysk (2026-10-14): measure the signal first, then raise signal per step; no blind long runs.
This probe measures it directly, so the next training run is not another blind 3 h.

## Do

1. **The probe.** In `crates/cubarium-search` (beside `es/voxel/pg/`; `kin/twins.rs`
   `run_copy` already runs copies of one start), build a mode that:
   - loads a settled start and runs it under the baseline policy (the lineage's clone; the
     rules where no clone loads, and say which);
   - at sampled top-layer decisions of sampled bodies, forks the world and runs one branch per
     intent on that body's menu that is valid there. The body takes that intent for that one
     decision (held as a normal hold), and everything else runs the baseline;
   - runs each branch to 10 world-minutes and records the body's **judge** (and the teacher)
     at 30 s, 2 min and 10 min, plus its own energy change;
   - uses the same random stream in every branch of one fork, so differences are the choice's
     effect. Two extra branches repeat the baseline's own choice with the stream perturbed (or
     whatever perturbation the sim allows), which gives the noise floor. First check that a
     repeated identical branch reproduces (no bit-identical requirement; report the spread).
2. **Lineages:** the lanternjaw (the pilot's starts and clone, `runs/pg-pilot-2026-10-14/inputs/`),
   the loftstrider and the Veilhound. For the last two, the settled wide saves at m240 in
   `runs/lab/1015-080850-calf-roam-saves-*/batch-task/batch/settle/s*/run/ckpt/m000240`
   (main c507ad00, fauna schema 89). Clones: LS 35ba, VH 8090 (find them in the ledger
   `.claude/ledgers/behaviour-ai.md` and the run dirs). If a clone won't load on main, use the rules
   and say so. Skip the seedporter (its clone needs redoing on ≥2153d885).
3. **Size:** about 200 decisions per lineage over 8 starts, spread across situations
   (feeding, resting, alarm, hunt). Fit in a few desktop hours; say the estimate.
4. **Read script** `scripts/studies/choice_probe_read.py`, which prints per lineage and horizon:
   - the spread across intents (best − worst, best − baseline) against the noise floor;
   - the share of decisions where some other intent beats the clone's choice by more than the
     floor, and by how much;
   - the clone's probability on that better intent, i.e. how often today's learner would even
     try it;
   - which intents and situations carry the gaps;
   - the implied number of decisions a policy-gradient step needs for half its gradient to be
     signal, at today's entropy and at 2–3 higher exploration levels.
5. **Tests, mechanism only:** a fork with the same choice and stream reproduces the parent's
   outcome within tolerance; a forced intent is the one the body actually holds; the judge read
   in a branch matches the judge the trainer would compute. No tuned numbers.

## Boundaries

- Probe code and its read only. No change to behaviour, senses, fauna defaults, the trainer's
  defaults or the scores.
- Don't merge (the parent merges). EC2 never. Commit explicit paths, never -a. Trailers:
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` and
  `Claude-Session: https://claude.ai/code/session_01BCL2219aartFBspT73Nfbh`.

## Return (at most 40 lines)

What you built (file:line), how the fork and the forced intent work, the reproduction spread,
the job id(s) and what each runs, where outputs land, the exact read command, quick-lane
numbers if any, commits, departures.
