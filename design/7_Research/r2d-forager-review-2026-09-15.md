---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# R2d targeted evidence review

The recurrent forager is real progress. Keep the GRU and ES approach. The R2d
measurements support dependence on hidden state and limited transfer across duration
and population; several causal interpretations exceed what these probes establish.
Correct those before using the report to change the sensory contract or ecology.

Scope: reviewed the R2c/R2d reports, the R2d command diff (`325fdd0`), relevant
episode-driver and observation spans at checkout `fd98972`, and existing JSON outputs
and checkpoints. No simulation episodes, training, tests, agents or live-display
actions were run for this review. Concurrent display-seeding work was not reviewed.

## Confirmed from retained evidence

- Selected seed-1 center is generation 59, score 36000.18002178731, weights FNV-1a
  15300230604599313182. Generation **58** first reached the horizon on all training
  layouts; generation 59 was the best-scoring center, not the first success.
- The plain R2d held-out episode records exactly match the original R2c records:
  7/8 survive 36,000 ticks. Both reset cadences yield 0/4 training and 0/8 held-out
  survivors. The long-horizon outputs give 1/4 and 0/8 at 108,000 ticks as reported.
- Seed 2 has now completed 64 updates: 8,452 episodes, 251,068,201 ticks, zero
  discarded work. Its first horizon-reaching center is generation 19; its best is
  generation 64, score 36000.18357564603, weights FNV-1a 16562626748431622726.
  This is training evidence; this review did not establish a seed-2 held-out result.
- The nine retained R2d diagnostic JSON files report 140.858 seconds of evaluation
  wall time in total. This excludes seed-2 training and implementation work.

## Findings and corrections

### 1. Group survivor counts use different stopping times

`episode::run_with_fault` stops when the focal animal dies
(`crates/cubarium-search/src/es/episode.rs:401`, `:488`). The new evaluator reports
the remaining neural count at that time as `copies_alive_final`. Consequently, group
counts in the R2d table are not uniformly measured at 36,000 ticks.

For example, h3 stops at tick 18,759 with three other animals alive; h8 stops at
21,841 with three alive. Training t2 stops at 22,060 with two alive. Their eventual
survival is unknown. Focal survival results remain valid. World-wide intake is also
measured only to these stopping times, while movement, stores and upkeep in each
episode describe the focal animal; they are not a cohort energy balance.

**Correction:** label the existing table “copies alive at focal death or horizon,”
include each stopping tick, and describe surviving copies as censored. No rerun is
needed to make this report honest. Before a future cohort survival experiment, let
the shared-arena evaluator continue until the horizon or the last death, retaining
each individual's death time separately from the world's stopping time.

### 2. The policy already senses nearby animals

The report's “no conspecific input, so it neither spreads out nor yields” explanation
is misleading. `neural/obs.rs:183–234` supplies six sectors of body presence and
relative size, plus a crowd vector. There is no species/role label, but copies are
observable bodies. Those inputs were uninformative during solitary training;
responses to them have not been selected or validated.

**Correction:** describe this as an untested response to crowding combined with an
unvalidated food budget for four animals. These results do not isolate a sensory
deficiency, demonstrate a failure to yield, or establish cooperation as necessary.
Population competence can include dispersal and competition. Nor is a cube face a
resource boundary: competition depends on overlapping foraging areas over time.

### 3. Memory mechanism and sustainable foraging remain hypotheses

Periodic hidden resets demonstrate that this policy's intact recurrent state is
important under the tested conditions. They disrupt both any startup transient and
any accumulated sensory history. Aggregate intake and visited-cell counts cannot
separate a clock from sensory integration, navigation history or recurrent dynamics.
“Mostly a clock and a mode” is a plausible hypothesis, not an identified mechanism.
Fresh hidden state also does not guarantee identical offspring behavior: body,
reserves and surrounding observations can differ from the training adult.

Similarly, enough aggregate regrowth plus 36,000-tick survival does not imply
sustained foraging. Food accessibility, travel costs, initial stocks and repeated
relocation still matter. The existing long-horizon failures do not separate resource
insufficiency from policy failure, as the report partly acknowledges. They also do
not establish a live-world lifespan.

**Correction:** retain the measured outcomes and qualify those mechanism/lifespan
claims. Do not increase regeneration or prescribe a startup heuristic based on them.

### 4. Close the second-seed result and selection bookkeeping

The report ends with a promised addendum despite a completed checkpoint. Add the
actual final/best result above and correct seed 1's first success to generation 58.
Record seed-2 elapsed time from an existing source if available; otherwise mark it
unavailable. If a seed-2 evaluation already exists, link its actual provenance and
result; otherwise explicitly say it has not been established. Do not launch one to
fill this documentation gap.

## Fable correction handoff

In a fresh bounded context, update
[the R2d report](r2d-forager-diagnostics-2026-09-15.md) using the four corrections
above and existing artifacts. Preserve the original measurements and identify
censoring. No new learning or evaluation episodes, core/runtime edits, sensory
channels, live changes or additional review campaign. Report the corrected claims
and stop. This is one evidence-report repair, not a request to rebuild the evaluator.

## Recommended next scientific step (proposal, not an execution brief)

Ask whether a trained forager repeatedly finds another usable food patch after local
depletion in an arena where a paid mobile control demonstrates that survival is
feasible over the same duration. Pair that with stationary grazing to establish that
relocation is actually necessary. Preserve core physics and ecology; avoid making
the task solvable simply by increasing local regeneration.

Validate that task before another optimizer campaign. Use compact patch-departure,
arrival and intake summaries to distinguish repeated relocation from a startup walk
followed by circling one productive area. Vary starts/headings in a separately
specified training protocol rather than designing around the now-inspected held-out
failures. Treat the current held-out layouts as a known diagnostic set for future
development; a later generalization claim needs a fresh untouched test set.

Then extend to a resource-calibrated shared arena with complete cohort observation.
Single-animal foraging remains a prerequisite, not evidence for reproduction, apex
behavior or ecosystem sustainability. Keep those later milestones in scope without
requiring this first policy to solve them retroactively.

No new compute budget is authorized by this review. Billed token usage is unavailable.
