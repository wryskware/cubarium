---
design_status: exploration
last_reviewed: 2026-09-14
---

# Fable: R1a recurrent runtime, observations and action integration

This is the next bounded handoff following the R0 progress check. Forwarding it
as an implementation request starts R1a only. Read repository working rules,
canon rules/ledger, then
[the interface contract](../recurrent-interface-contract.md) and this brief.
Use fresh context and Graft for current integration/caller coverage. No nested
agents or extra review campaign under this brief.

## Progress checkpoint and baseline

- R0b corrected the budget (`99a2bfc`) and supplied the real-core grazing probe
  (`2822caa`); R0c specified 70 observations and seven actions (`4a04cc1`).
- R0d (`3e21c57`) subsequently changed new-world cruise to 1 body length/s and
  held the full-cruise energy bill per second. Read the
  [R0d report](../7_Research/r0d-pace-calibration-2026-09-14.md), not just the older
  [R0b measurements](../7_Research/r0b-motor-foraging-result-2026-09-14.md).
  The new-pace scripted mobile grazer remained funded through 1,800 seconds;
  ungated stationary grazing still exposed the tiny-intake survival defect.
- The progress check reran `motor_foundation` (7) and `diagnostic_seam` (5): all
  passed. The R0d report records 1,229 workspace passes; that broader suite and
  the visual demonstration were not rerun by the progress check.
- Live status could not be verified from the progress-check environment:
  localhost:7393 refused connection. Do not infer either running or stopped from
  historical PIDs. Inspect actual ownership before any development update.

R0 is sufficient to build the neural runtime. It does not establish sustainability
or learned behavior. Fixing unpaid survival remains required before survival-based
training; a harvesting floor is a separate ecological choice, not a necessary
consequence of correcting the death/upkeep accounting.

## Scope and implementation choices

Implement the interface contract's §9 slice: sampler, action adapter, GRU forward
pass, per-animal world dispatch, exact state persistence and a fixed-policy probe.
Use GRU32, 70 inputs, seven outputs, reset-after equations and 10 Hz inference over
20 Hz physics as the starting implementation. No optimizer or trained policies.

Use these bounded choices to resolve ambiguity while implementing:

1. Retain R0b's common-factor motor resolver and R0d's current defaults. Keep
   the contract's binary locomotion activation and separate translation request,
   allowing pure pivot without forward thrust. Capability never gains a separate
   rotation allowance. Preserve the seven-channel layout for versioning; attack
   is capability-masked for ordinary bodies, and neural apex attachment is rejected
   explicitly until its listed sensory/action extensions exist.
2. Implement neural ordinary-body dispatch end to end. Legacy mode hysteresis,
   heading/effort overrides, automatic flee/retreat and quiet behavior must not
   overwrite neural intents downstream. World contact, damage, capacity, paid
   motion and mandatory physiology remain. Inspect every downstream override;
   do not assume calling the GRU at the old decide stage transfers ownership.
3. Keep present physical maturity/age, funding and gestation constraints in this
   runtime slice; do not delete age gates or change biological inheritance here.
   Ordinary reproduce requests may start paid budding. A neural offspring copies
   the parent's compatible policy with fresh private state. Weight mutation,
   two-parent neural mating and changes to body inheritance belong to R3.
   Single-body runtime/performance fixtures may disable births explicitly, but
   one lifecycle test must exercise funded ordinary birth and fresh child state.
4. Keep all 70 inputs, including fruit sectors. Do not silently drop channels if
   performance disappoints; report the measurements at the checkpoint. Reject
   unsupported neural/quiet or neural/apex combinations clearly; do not disable
   unrelated ordinary care controls merely because quiet mode is unsupported.

## Correct the contract where the new baseline exposes ambiguity

Update the owned interface document alongside implementation, retaining exploration
status. Record these as implementation corrections, not new accepted ledger entries:

- Label R0b numbers historical and add current-pace worked motor cases. The true
  requested magnitude is
  `a_thrust * v_max/wading + r * abs(a_turn) * min(omega_max, u_full/r)`.
  It is not always `a_thrust*u_full + abs(a_turn)*u_full`: angular ceilings bind
  at the new pace, and low energy can cap `u_full` below translational capability.
  Test both regimes; do not encode `a_thrust + abs(a_turn) > 1` as the universal
  condition for scaling. Preserve explicit requested versus resolved quantities.
- Define `motor_avail` sampling in tick order, sharing the real motor affordability
  calculation rather than duplicating an approximate energy bill in the sampler.
  Give zero-capacity/zero-rate cases finite encodings. Recompute held signed turn
  from the current transported heading every physics tick.
- Resolve feedback initialization: the contract says `delivered = 1` when nothing
  was requested but elsewhere says feedback starts at zero. Use zero intake/motion
  and delivered ratio 1 for an empty interval; define partial first intervals using
  the actual accumulated tick count. Make consumption/reset timing unambiguous.
- Specify deterministic ties for body-sector contributors, exactly serialized
  schema/digest bytes and actual validation of finite weights/state and references.
  Schema numbers in the plan are pointers: inspect the current schema before adding
  the next one and freeze a historical decoder only as required by repository practice.
- A seam fixture must compare physically equivalent sampled fields and transported
  body frames. Do not demand all 70 observations match across different real light,
  height/up or habitat conditions. Those differences are genuine sensory information.
- Retain current lifecycle rules in the ownership table for this slice and clearly
  mark later changes; do not claim apex mating or complete neural behavior exists.

## Checks and deliverable

Implement meaningful sampler geometry/magnitude/truncation checks, motor cases at
old persisted and new default pace, masks/shared mouth handling, and held turns
across seams. Include a case where thrust is zero but paid rotation is nonzero,
and a zero-action case with no motor expenditure beyond mandatory upkeep.

Check the GRU against an independent numerical reference that exercises recurrent
terms and gates. Use a fixed hand-authored policy for the two-history/same-final-input
memory test and its hidden-reset control. No learned-memory claim follows from this.

Persist exact weights, private hidden state, held actions, feedback and cadence.
Test save/resume across both cadence phases and a seam, death/slot reuse, birth,
malformed/foreign schema rejection and legacy-only continuation. Old snapshots
load explicitly legacy-controlled; no silent replacement or reset. Comparisons
must allow intended format/hash-schema changes while verifying actual trajectory
equivalence, not demand byte-identical old and new serialized formats.

Provide a deterministic development probe that attaches the fixed policy explicitly
to an isolated fixture and exercises the actual world dispatch. Keep any ordinary
development access explicit; do not automatically attach untrained policies to the
current display world. No production trainer, migration campaign or subsidy.

Run the affected core/persistence/presentation checks. One performance screen:
32 and 128 bodies for 2,000 ticks each, 512 for 200, at most 60 seconds execution
wall time including repeats (compile separately). Report actual sensor, inference
and total-world timings and active-body counts. No sweep or automatic budget growth.
Keep one normal cache; ask before adding over 1 GiB. Use Git for owned changes and
protect other authored work. Follow the normal development-update policy when
shipping relevant runtime changes, coordinating the actual runner and preserving
its chosen state directory; this brief authorizes no world reset.

Write `design/7_Research/r1a-runtime-result-2026-09-14.md`: implementation/contract
changes, ownership coverage, tests, probe and performance evidence, current build
and development status, remaining limitations and actual usage if exposed. Context
counter changes are not a reliable billed-usage measurement.

At most one targeted implementation review if separately assigned, at most two
repair cycles. Stop after R1a. No training, optimizer selection, starvation-rule
redesign, cropping floor, pace/economy changes, M1 work or automatic R1b/R2 launch.
