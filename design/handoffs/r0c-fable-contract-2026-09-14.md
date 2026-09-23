---
design_status: exploration
last_reviewed: 2026-09-14
---

# Fable: specify the recurrent sensory/action contract

Produce one implementation-ready proposed contract, not another review or code
implementation. Work in a fresh thread in `/home/wrysk/wryskware/cubarium`. Read
repository working rules, canon rules/ledger and the
[dispatch](post-fable-dispatch-2026-09-14.md). Read sections 2–5 and 8–9
of the [recurrent plan](../recurrent-organism-plan.md), F5/F6/F7 of your
[review](../7_Research/fable-recurrent-plan-review-2026-09-14.md), and the
[Opus brief](r0b-opus-2026-09-14.md) for the dependency boundary.

Owner direction remains recurrent control directly, no MLP comparison, pivoting
allowed, no compulsory destination, learned behavior within world-owned physical
limits, and reproduction included in the complete scope. Exact neural interfaces
remain proposals. Your sole write target is
`design/recurrent-interface-contract.md`, with exploration frontmatter. Do not
edit core code, the main plan, Opus's report, or accepted ledger entries.

## Draft now; finalize against R0b

Use Graft for current sensory construction, movement, feeding, lifecycle and
persistence touchpoints. Specify one practical v1 interface for the initial
foraging learner plus explicit later extensions for full diets/reproduction.
Prefer existing meaningful signals over invented mechanisms. No broad research
campaign, new simulator experiments or agent spawning.

1. **Enumerate the input vector exactly.** Give each index/range, dimension count,
   units, scaling/clamping, absent-value encoding, sampling geometry and source.
   Count the actual scalar values; do not force a target such as 30 or 64. Include
   local food magnitude/quality, useful internal state, real feedback and supported
   environment cues. Justify any omitted channel needed by the initial diet.
2. **Define the spatial sampler.** Body-relative sectors/rings, angular weighting,
   range/cost, seams, neighbor deduplication, empty sectors and occlusion assumptions.
   Distinguish an abstract local chemical sensor from an implemented odor field.
   Explain which relative motion/body cues are implemented versus future work.
   No global inventory, reward, hidden prey labels or organism IDs as neural inputs.
3. **Enumerate actions and their world interpretation.** Supported thrust/turn and
   feeding channels, ranges, deadband, update/hold cadence, capability masks and
   simultaneous spending. Decide explicitly whether reverse/lateral movement is
   supported now or deferred with reasons; no permanently dead output. Consumption
   channels share mouth capacity. Digestion/upkeep remain mandatory physiology.
4. **Make pure pivot actually expressible.** Distinguish requested translation from
   locomotion activation/capability. Zero thrust plus a turn request must permit
   paid rotation; do not derive capability solely from actual translational output.
   Conversely, separate effort fields must not recreate R0a's additive rotation
   allowance. Show pure pivot, pure translation, combined and zero-action examples
   through the R0b contract. Separate physical availability from actual resolution.
5. **Define recurrence state and persistence.** Per-animal hidden state, held action,
   feedback accumulation, cadence phase, birth/death/slot reuse, seams, dormancy and
   save/resume. Keep the planned GRU32/10 Hz as a starting proposal; no model sweep.
   Specify version identifiers and migration/retraining boundaries without coding
   them. Existing worlds remain explicitly legacy until a chosen migration exists.
6. **Show the path to complete behavioral ownership.** Map locomotion, feeding,
   departure/rest, pursuit/escape, attacks, mating and reproduction to neural
   decisions versus world constraints. Specify how future cues/actions can express
   partner-specific choice and refusal; global readiness of two nearby animals is
   insufficient by itself. Physical interaction may use internal deterministic
   handles, but must not silently choose the desired partner for the neural policy.
   Clearly label later capabilities, including dormancy's physiological exception.
7. **Keep inheritance and evaluation honest.** Describe same-parent body/policy as
   the preferred starting proposal and its remaining compatibility/mutation choices.
   Reproducing descendants are required evidence later; derive evaluation duration
   from each actual lifecycle, including apex, rather than using ordinary budding
   timing for every diet. Avoid raw lineage size as the sole success measure.
8. **Give the next implementation slice.** Exact source boundaries, dependency order,
   meaningful sampler/action fixtures, a two-history memory interface check, and
   explicit completion criteria. No training implementation or optimizer selection
   here. Identify what R0b and later recurrent throughput must measure before
   choosing horizons and compute; do not reuse M1 throughput as a neural benchmark.

When Opus's result arrives, incorporate its verified motor mapping and measured
grazing limitations. If it has not arrived, finish the draft and list precisely
which clauses remain conditional. Do not block the whole document or invent
results. A subsequent update on receiving that result is the same bounded task.

## Deliver and stop

Target a compact contract with tables, formulas/examples only where needed and
an exact dimension total. Separate v1 implementable fields from planned extensions
and annotate unsupported sources. Include a small open-decisions list with your
preferred option, plus factual dependencies rather than broad approval rituals.

Check internal counts, units, local document links and consistency of the action
examples. No simulation runs, extra review agents, implementation, live changes or
rewriting the research review. Commit only the owned artifact if appropriate and
report actual usage if exposed, otherwise unavailable. End with contract readiness
and the next bounded implementation scope; do not launch it.
