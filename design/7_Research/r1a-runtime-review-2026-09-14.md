---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# R1a implementation review

**Repair verification, 2026-09-15: R1a clears this review at `bf96ecb`.** The
original runtime reproductions pass, the probe findings are corrected, and the
previously missing geometry and component-timing checks now exist and pass.
See the verification record below. This signs off the runtime milestone, not
learned behavior, ecological balance or a training budget.

**Initial verdict (2026-09-14): repairs required before R1a sign-off.** Reviewed checkout `693a300`,
including the owner overrides recorded in the result report for starvation and
the deployment reset. Those overrides are not treated as scope violations.
No training, deployment or simulation implementation changes were made here.

The recurrent runtime is substantial: real per-animal dispatch, the action adapter,
GRU reference checks, private state and snapshot migration exist. However, three
focused reproductions fail despite all 45 existing neural/starvation tests passing.

## Findings

### P1 — Predation leaves dangling neural state and invalidates the world

Location: [world/step.rs:1341](../../crates/cubarium-core/src/world/step.rs#L1341),
capture settlement. The ordinary death path removes private state at the same
boundary as the organism; capture removes only the organism. Ordinary neural
prey can coexist with legacy hunters; refusing neural apex attachment does not
exclude this supported combination.

**Reproduced:** stage a certain capture using the existing hunter fixture, attach
a motionless neural policy to the prey, and step until capture. The prey's neural
entry survives. `WorldState::validate()` then returns:

```text
Err("neural animal 1:1 has no organism")
```

This breaks valid continuation/snapshot loading after a normal interaction and
retains dead animals' private state. Remove neural state at the capture boundary
too. Add a real capture regression that validates the world, round-trips the
post-capture snapshot, and exercises subsequent slot reuse.

### P1 — Solvency is checked against reserve, but upkeep is not collected from it

Locations: [world/step.rs:1158](../../crates/cubarium-core/src/world/step.rs#L1158)
and the payment at `1217–1220`; later oxidation in the physiology pass.
`raisable_energy` admits survival using energy obtainable from reserve, but the
payment remains `min(cost, o.energy)`. If stored energy is short, the difference
is forgiven. Oxidation subsequently credits energy without subtracting the missed
upkeep. The new test checks survival and nonnegative energy, not actual payment.

**Reproduced:** a stationary, nonfeeding body starts with zero energy and enough
reserve for oxidation. In one tick:

```text
upkeep owed:       0.000185
oxidation credit:  0.000800
ending energy:    0.000800
actual payment:   approximately 0 (7.59e-19 rounding residual)
```

It survives with the entire oxidation credit. The infinitesimal-intake immortality
case is fixed, but the promised paid-survival contract remains incomplete. Make
solvency and settlement agree: collect the full mandatory bill from the permitted
pre-intake resources, or explicitly settle the shortfall without double charging
oxidation. Preserve material/energy/heat accounting. Test zero/partial stored energy,
adequate and inadequate reserve, no intake, and intake arriving later in the tick.
Do not introduce a cropping floor as a substitute for collecting upkeep.

### P2 — Neural budding skips the retained minimum-age gate

Locations: `world/step.rs::decision_from` (`bud: action.reproduce()`) and the
ordinary gestation admission in stage 9. The legacy age gate is implemented in
[controller.rs:258](../../crates/cubarium-core/src/controller.rs#L258), which neural
animals correctly skip. No equivalent world admission check was added.

**Reproduced:** a funded ordinary neural body at age zero with reproduce held high
opens escrow on its first tick, despite its configured minimum age being 120 s.
The existing birth test starts at world tick zero and sets `born_tick = 0`; its
comment that the body is old enough does not establish that precondition.

This changes the lifecycle that the handoff and result say was retained, allowing
premature reproduction and misleading future lineage evaluation. Keep reproductive
intent neural but enforce the retained age condition at admission. Add below-age,
at-age and insufficient-funding cases. Do not invent a new maturity rule under the
guise of restoring an existing one; document the exact retained conditions.

### P2 — The probe reports field loss as intake and net energy as motor cost

Location: [neural_probe.rs:180](../../crates/cubarium-core/examples/neural_probe.rs#L180)
and its output at `183–195`. `P eaten` is accumulated from own-cell stock decreases,
which include field processes and miss the distinction between the pre-move cell
and the cell actually grazed. `motor` is computed as
`max(start.energy - end.energy - assumed_upkeep, 0)`, which mixes expenditure with
assimilation and oxidation. Consequently resting mouths report positive "P eaten"
and a moving, feeding body can report zero motor expenditure.

The report acknowledges some of this, but the labeled columns still cannot verify
the actual transactions required by the handoff. Use the existing actual-intake
diagnostics and resolved motion/bill accounting. Report paid upkeep, paid motion
and net stores separately, including credits. Check zero mouth effort produces
zero intake and simultaneous travel/feeding records both debit and intake.

## Verification and remaining acceptance gaps

Reran on this checkout:

- `cargo test -p cubarium-core --release neural --lib`: 33 passed.
- `cargo test -p cubarium-core --release --test neural_runtime --test starvation`:
  12 passed.
- Three additional isolated regression checks: all three fail with the outputs
  above. The [reproduction source](assets/r1a-review-regressions.rs) is retained
  as a small authored review artifact. Copy it to
  `crates/cubarium-core/tests/review_r1a_temporary.rs` and run
  `cargo test -p cubarium-core --release --test review_r1a_temporary -- --nocapture`.
  The temporary test file was removed after review; the normal suite is unchanged.

The result report also explicitly leaves required checks unfinished: equivalent
seam sensory geometry, 17-neighbor truncation, and signed held-turn behavior through
a seam. Save/resume agreement does not replace these: two runs can agree on the
same incorrect sensor or transport behavior. Complete these in the repair pass.

Sensor and inference timings were not separated. The reported legacy-versus-neural
world timings support whole-runtime feasibility for those fixtures, but their
difference also includes differing policies, trajectories and legacy controller
work. It is not an isolated measurement of sampler/GRU cost. Supply the requested
component timings or explicitly keep that acceptance item incomplete; no optimizer
or expanded throughput campaign is needed to resolve it.

The reported full workspace run and live deployment were not independently rerun
or verified here. No additional model review was used. Billed token usage is not
available in this environment.

## Bounded repair handoff

Use the same R1a assignment for one repair pass: fix the three runtime defects,
promote their regressions into the appropriate permanent suites, correct the probe,
and close the listed geometry/measurement gaps. Preserve the chosen pace, resource
model and neural interface unless a named correctness fix requires a documented
version change. Update the result report with actual tests and measurements.

No training, new architecture, cropping floor, parameter sweep or additional design
review. Run the relevant affected suites and stop with the corrected R1a result.
The existing at-most-two-repair-cycle limit still applies; this is the first review
report for the implementation, not a request for another reviewer.

## Repair cycle 1 verification — 2026-09-15

Inspected changes `2055a9b` and `a106cee` and the updated result/contract through
`bf96ecb`, scoped to the original findings and their affected paths. No additional
model review or new implementation changes.

- **Predation:** capture now removes private state beside the organism. The permanent
  regression validates, round-trips and continues the world after capture.
- **Upkeep:** settlement burns reserve for the battery shortfall, pays the bill and
  deducts that burn from the remaining oxidation allowance for the tick. Tests cover
  zero/partial battery, inadequate reserve, later intake and the shared rate ceiling.
- **Reproduction:** the world now applies the retained age gate at gestation admission.
  Below-age, at-age and unfunded cases pass; the birth fixture uses a genuinely
  old-enough parent.
- **Probe:** actual intake comes from the settlement diagnostics, with movement bills
  separated from net energy. Re-execution reports zero intake in the closed-mouth
  rows and, for travel-and-graze, 0.2649 m producer intake plus 0.01799 e motion cost.
  These are controlled-fixture measurements, not ordinary-world foraging evidence.

**Checks rerun:** 33 neural unit tests and 24 tests across `neural_runtime`,
`neural_predation`, `neural_geometry` and `starvation`: **57 passed**. The three
original review reproductions were copied unchanged into a temporary test target:
**all three now pass**. That temporary target was removed afterward.

The new geometry suite tests seam-equivalent observations, neighbor truncation and
held-turn behavior before/after a seam. The far-ring comparison allows 1e-5;
equivalence is not claimed to be bit-exact. The crossing tick itself is excluded
from raw chart-heading comparisons.

One execution of `neural_probe` completed successfully. Its throughput screen took
2.0 seconds: all-neural throughput was 8,169 / 2,562 / 536 ticks/s at 32 / 128 / 512
bodies respectively; all requested bodies remained alive. Direct sampler, GRU and
adapter timings and cadence counts are now reported separately. These measurements
include timing overhead and do not establish training sample requirements.

No blocking finding remains from this review. The full workspace suite and live
display were not independently rerun or verified in this repair check. No training
or deployment was started. The next checkpoint is the bounded training protocol;
its optimizer, evaluation fixtures and compute budget still need to be concretely
specified rather than inferred from runtime throughput alone.
