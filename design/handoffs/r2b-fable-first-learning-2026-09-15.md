---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# R2b — Execute the first bounded learning screen

Run this assignment in `/home/wrysk/wryskware/cubarium`, following repository working
rules. R2a is verified at `dbb769e` in the
[review](../7_Research/r2a-trainer-review-2026-09-15.md). The
[R2a result](../7_Research/r2a-trainer-result-2026-09-15.md), sections 7, 11 and 12,
records the frozen protocol, measured compute and repairs. No new implementation
or optimizer research is requested.

## Question and scope

Does the first fixed-budget antithetic-ES screen improve the central GRU policy's
survival on the four validated foraging layouts? Use the real-core trainer unchanged:
one ordinary grazer, births disabled, fixed body/ecology, private recurrent state.
The apex is deferred to R3. This screen does not establish generalization, memory
use, reproduction or sustainability.

## Existing pre-repair run

Read-only inspection found `runs/es-first` already completed 16 updates at build
`80bf7186d98c-dirty`, with 2,116 episodes and 16,342,111 simulated ticks. It predates
the reviewed repairs and has no saved intermediate center files. Preserve it;
this handoff explicitly repeats the fixed seed/protocol with the repaired trainer
and complete diagnostics, writing to **`runs/es-first-repaired`**. Do not present
the old run as produced by the verified build or as an independent seed replicate.

## Execute once

Confirm the checkout includes `dbb769e` and no concurrent worker is changing the
trainer/core during execution. Build in the normal cache and record the actual
build and protocol hash. Expected protocol hash: `0xb69033f65e56f1df`.

```bash
cargo run -p cubarium-search --release -- \
    es-train --pairs 16 --generations 16 --horizon 36000 \
             --workers 8 --wall-seconds 1200 --train-seed 20260915 \
             --center-eval true --out runs/es-first-repaired
```

Maximum scheduled work: 2,048 perturbation episodes plus 68 center episodes =
**2,116 episodes / 76,176,000 ticks**. Eight workers, **20 minutes execution wall
time**, no automatic retry, continuation, extra seed, horizon increase or tuning.
Measured throughput predicts about 9.7 minutes at the full tick budget; early deaths
can make it shorter. Report actual completed and discarded work separately.

If `runs/es-first-repaired` already contains a run, inspect its provenance/status first.
Do not overwrite it or silently rerun an already completed campaign. A partial run
does not reset the shared 20-minute budget. Report the existing state if the
remaining budget cannot be established.

Do not rerun the full control suite or benchmark merely to start: they and the
repair tests are already verified. If the command fails, retain the named error and
last completed checkpoint and report it; no repair campaign follows automatically.

## Analyze existing outputs and stop

From the emitted logs/checkpoint/center files, report initial, best and final center
scores; completed updates; per-layout survival and terminal stores where recorded;
actual intake and movement diagnostics; wall time; and completed/discarded work.
Distinguish billed costs from payments. Where `motion_billed_partial` is true,
the sweep/motor estimate is a lower bound, not an exact total.

Select the highest **recorded center score**, with ties resolved to the earliest
generation. Use training results only. Report the selected center's existing file
and weight hash, alongside the final center. An unscored final center is not a
winner and must be described as unscored. A perturbed candidate's score is not a
center score. Do not spend extra episodes to fill gaps in this report.

Write `design/7_Research/r2b-first-learning-result-2026-09-15.md` with the exact
command, source/build/protocol, measured outcome, selected artifact and next
scientific question. Keep the checkpoint, compact generation summaries and center
weights (expected total under 10 MiB); no per-tick archives or binary copies.

**Stop after this screen and report.** Held-out policies, hidden-reset diagnostics,
longer horizons and shared-arena transfer need a separately specified evaluation
budget before execution. Do not attach a policy to the live world, reset/restart the
display, train the apex, tune M1 or spawn agents under this assignment.
