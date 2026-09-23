---
design_status: exploration
last_reviewed: 2026-09-14
---

# Post-Fable dispatch: R0b implementation and R0c contract design

Wrysk requested these handoffs after accepting the response to Fable's review.
Forwarding a worker brief starts its bounded assignment. This dispatch does not
start R1, training, M1 changes or a world reset, and promotes no ledger entries.

## Two useful concurrent assignments

| Owner | Brief | Writes | Dependency / completion |
| --- | --- | --- | --- |
| Opus | [R0b motor correction and grazing evidence](r0b-opus-2026-09-14.md) | Core motor/integration fixes, focused tests, one diagnostic, R0b result | Start now; sole code and development-runner owner |
| Fable | [R0c sensory/action contract design](r0c-fable-contract-2026-09-14.md) | One proposed contract document | Draft now; incorporate Opus's final contract/evidence before declaring ready for implementation |

This is implementation plus interface design, not another review of the review.
The Fable assignment does not implement the sampler or neural runtime. If Opus
has not finished, Fable may deliver a useful draft with the exact unresolved
dependencies listed; do not guess results or wait indefinitely. Finalizing the
same draft after receiving R0b is continuation of the same bounded assignment.

Each worker reads repository working rules, canon rules/ledger, then its own
brief. Use fresh contexts and linked
artifacts. Protect unrelated work; one normal build cache. Do not run duplicate
assignments under both manual and orchestrated modes.

## Disposition of the review

The [Fable review](../7_Research/fable-recurrent-plan-review-2026-09-14.md) is evidence.
Use these corrections when its proposed handoff differs from this dispatch:

- Fix the shared speed budget: no additive rotation allowance. Pure pivot remains.
- Keep current regrowth and food architecture for the first measurements. Test
  continuous grazing without the legacy feeding gate before proposing stubble.
- Do not require population to return to 96, profitable grazing by assumption, or
  any particular population outcome to pass the motor correction.
- Do not rerun the six-run population experiment or tune speed/cost coefficients
  in this slice. Observe native-size pace and report consequences.
- Exact input count, action semantics and reproductive scope need an explicit
  contract. A list labelled approximately 30 is not an enumerated schema.
- Training horizon/budget depend on paid-foraging evidence and measured recurrent
  throughput. Old M1 throughput cannot establish the new runtime budget.
- ES is a candidate, not an already selected optimizer. Use the selected optimizer
  for both smoke and learning; do not build GA only to smoke-test an ES trainer.
- Same-parent body/policy inheritance is a proposal. Partner-specific choice and
  refusal remain necessary; a global readiness bit alone does not settle them.

Older plan sections remain design context; these briefs define this dispatch's
scope. A completed R0b plus a ready contract is the checkpoint for the next
implementation assignment, not permission to automatically launch the RNN work.

## Optional orchestration

Wrysk may give this dispatch to one Fable or Opus orchestrator instead of opening
two worker threads. That authorizes at most two fresh worker assignments, using
the roles above where supported. No full-history forks, nested delegation or
additional research agents. If requested models are unavailable, provide the
external prompts rather than silently substituting a model.

The orchestrator owns coordination and one short final summary, not another
parallel code implementation. Opus alone owns core integration and runner changes;
Fable alone owns its new contract file. On a shared checkout, coordinate paths
and commit only task-owned files. Isolated checkouts are optional, not a reason
for extra build caches or automatic cleanup of existing worktrees.

Wait for Opus's fixed motor semantics before Fable finalizes the action mapping.
Resolve ordinary integration details within these briefs. At most one targeted
implementation review across R0b and at most two repair cycles; do not add another
design review or ask another model to review Fable's review. If no review is
needed, relevant checks and the final evidence suffice.

Close with changed files/commits, tests, measured grazing outcomes, native-size
observation, contract readiness, remaining decisions and actual usage if exposed.
Report unavailable usage honestly; remaining context is not billed token usage.
Stop at this checkpoint. No training, compute expansion or further milestones.
