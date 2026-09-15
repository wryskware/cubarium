---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Fable orchestration: ecology calibration, apex comparisons, presentation and retraining

Wrysk requests that you start this work, with presentation in parallel. You own
dispatch, integration, experiment budgets and the consolidated report. Ecology v1
has completed your Astra review and repair cycles: accept that baseline. Do not
commission another in-depth review of it or reopen the food-web design exercise.

## Outcome and sequencing

Find plausible ecological parameter ranges and identify missing population
feedbacks; make the new plant state visible; run matched apex comparisons; then
retrain from scratch in a selected revised ecology and reassess its effects.
Deploy a fresh development world once presentation is ready. Ecological balance
and neural training are not gates for trying implemented features on the cube.

Start two independent workers now:

- **A: ecology calibration and apex comparison**, owning search, diagnostics and
  experiment configurations. Advance to C after its bounded results.
- **B: presentation**, owning renderer/assets and visual verification. Advance to
  D when it is integrated, independently of A/C.

Fable owns shared interfaces and serializes changes to them. Use fresh, narrowly
briefed contexts; no full-history forks or unrelated task resumes. Prefer Opus
for implementation, following any current project model-routing rules. Use one
targeted review of new work, at most two repair cycles, and no standing premium
review agent. Do not repeat the completed ecology-v1 review. Consolidate review
where practical; independent ready work need not wait for the other branch.

## Read and reuse

Read `AGENTS.md`, `WORKING_POLICY.md`, canon README and relevant ledger entries.
Use Lore/Graft as instructed. The externally referenced bounded-agent-work skill
was absent when this handoff was written; the bounded discipline is explicit here.
Keep this work exploration; do not promote new accepted decisions.

Give each worker only its relevant sources:

- `design/ecology-v1-contract.md`: current mechanisms, provisional parameters,
  presentation boundary, compatibility and subsequent amendments.
- `design/7_Research/ecology-v1-implementation-2026-09-15.md`: current run-3 tables
  and final open findings, not superseded run-1/run-2 measurements.
- A: existing M1 search implementation and
  `design/handoffs/ecology-search-2026-09-14-result.md` for reusable infrastructure.
  Verify current protocol/parameter coverage; old scores and fixtures are not
  evidence for ecology v1.
- B: current presenter and normal `assets/atelier`; consult the display shim's
  current contract and reuse geometry helpers.
- C: existing R2 ES trainer, R2d findings and the first-learning handoff. Old
  policies/worlds need not load. Retain the GRU and optimizer infrastructure.

The motivating findings are already established: one mobile grazer depleted a
bright 25-stand patch and starved; the reproducing 49-cell fixture doubled in
150 s versus 823 s for half-foliage recovery, peaked at nine and went extinct.
These are fixture results, not whole-world forecasts. Food accounting and paid
reproduction work; neither guarantees coexistence at provisional defaults.

## A — bounded calibration with early apex comparisons

1. Reuse the real-core headless evaluator. Make only the adaptations necessary
   to measure ecology v1 and expose a small, documented set of relevant knobs.
   The existing search excluded `plant.*`; expanding that exclusion is in scope
   where needed for this assignment. Record names, units, bounds and rationale.
   Do not replace the simulator, introduce a GPU port, or launch a broad GA
   before a throughput smoke and interpretable baseline.
2. Establish whole-world baselines across at least three fixed training seeds,
   with reproduction and ordinary lifecycle enabled. Use the existing legacy
   controllers initially. Retain patch scenarios as diagnostic probes. State
   founder stocks, light/weather, care inputs, mutation settings and horizons.
   Autonomous baseline arms receive no ongoing care subsidy.
3. Screen a small joint parameter set spanning intake/assimilation pressure,
   foliage growth and reserve allocation, maturation/reproductive investment
   and timing, and recycling where indicated. Tune plant and animal timescales
   together. Test digestive breadth costs if evidence points to generalist
   dominance; do not expand every axis simultaneously. Keep accounting,
   digestive exclusions and the motor contract intact.
4. Include matched zero/one/two-apex arms early, using the actual existing apex
   and its complete paid lifecycle. Reuse the harness's accounted apex cohort
   support where possible. Pair seeds and ordinary initial conditions; record
   imported predator material/energy. Match spawn timing and distinguish an
   initial cohort from reproduction. No repeated replacement of dead predators.
   Necessary compatibility repairs and harness wiring are in scope; a new
   predator architecture, flesh-capability redesign or apex neural controller
   needs a separately bounded brief.
5. Compare predator effects across candidate ecologies, not just against frozen
   defaults. Measure kills, apex survival/reproduction, prey abundance and
   vegetation outcomes. If behavioral changes are measured, report those too;
   do not infer fear or beneficial population control from predator presence.
6. Validate the baseline and at most two shortlisted configurations on four
   held-out seeds and a predeclared longer horizon. Do not tune on held-out
   results. If no candidate is plausible, report that and the smallest supported
   next intervention. Do not keep searching until something passes.

Report separate components, including population by guild, births and deaths by
cause, foliage and living/dead wood, plant death/recolonization, intake by food,
plant production, actual body bills, recovery after depletion and spatial travel
or patch use. Track stocks and flows: gross plant income is not all edible leaf
replacement. Distinguish guilds from visual founder kinds as diets mutate.
Use late-window behavior and held-out performance; an initial food bank or
nonzero terminal population is not proof of sustainability. Reject invalid
accounting and report extinctions, censoring and failures explicitly.

Prefer a trade-off table to a single opaque winner. Avoid solutions that leave
only plants, only one consumer guild, immobile survivors, or a constant full
canopy with no local depletion. Do not add arbitrary culling, population quotas,
free replenishment or controller rules telling organisms to preserve plants.
Slower reproduction alone is not a sufficient explanation of the one-grazer
failure; faster regrowth alone risks restoring stationary grazing.

**Compute envelope:** release throughput smoke at most 5 wall minutes; A's entire
simulation workload, including smoke, screens, predator arms and validation,
at most 60 wall minutes with at most eight workers. Before the campaign, publish
the concrete run matrix, horizon, seed split, trial count and predicted cost
within that envelope, then proceed without another permission round. Reserve
time for held-out checks. If full-world throughput cannot support the proposed
matrix, reduce candidate count before shortening all horizons; report the
resulting limits. Stop at the cap, including incomplete evaluations. No hidden
background run or automatic budget extension. These are spending limits, not
claims that an hour of compute can establish long-term balance.

## B — presentation in parallel

Make ecology-v1 state readable at the actual 64×64 face resolution using normal
assets and the established visual language:

- Living plant structure persists when foliage is stripped; living bare wood
  must be distinguishable from empty soil.
- Foliage loss and recovery occur on that structure and reflect the actual
  stocks without flicker or visual regrowth masking ecological depletion.
- Dead wood is visibly distinct from living structure and fades with its stock.

Do not imply seven independently simulated plant species if those are visual
forms over shared ecological stocks. Remains may retain the existing fleck
treatment unless a small distinction is needed for legibility. Keep normal
display free of analytical overlays. No core ecological changes in this branch.

Verify healthy, partially grazed, stripped-but-living, dead-wood and empty states,
plus one recovery sequence, through the real presenter. Use a small disposable
contact sheet/capture if useful, not an asset-generation campaign. Inspect the
normal viewer for seams, readability and frame pacing. Run relevant checks;
report precisely what was observed on viewer versus physical hardware.

## C — bounded fresh RNN training, after A

Use A's selected configuration, or explicitly designate a provisional diagnostic
configuration if A finds no viable range. Freeze and hash it for the run. Do not
quietly train against the old ecology or resume an old policy. Adapt fixtures
and ecological parameter/protocol compatibility checks where needed; retain
the current recurrent architecture and ES optimizer.

Start with the existing forager training scope, one fresh campaign: plumbing
smoke then up to 16 updates, at most eight workers, at most 20 wall minutes total
for training. State the trained body/diet and reproduction setting. This is not
training every guild or the apex, and a reproduction-disabled forager result
does not establish population sustainability. Name the next guild/lifecycle
training tasks in the report rather than launching them unboundedly.

Reserve a separate maximum 10 wall minutes for fixed held-out behavior checks
and a small population-level evaluation in the selected ecology, with ordinary
reproduction and matched predator treatment. Record policy provenance for every
body, including offspring; never imply a fully neural population if offspring
or other guilds use legacy control. Compare food use, travel and vegetation
pressure against the legacy baseline. A policy that feeds better may destabilize
the world. Report that without restarting calibration/training in a loop.

## D — fresh development display world

Once B is integrated and relevant checks pass, deploy the current development
checkout with normal `assets/atelier` and a fresh schema-16 world under
`WORKING_POLICY.md`. This assignment includes that fresh-world deployment;
no migration or old-policy restoration. Use the supported reset flow, limit
reset to the active Cubarium world's state, and preserve unrelated data. Stop
the existing owning runner before launching through `./scripts/run-cube.sh`.

Use a tested configuration available at deployment time; report whether it is
the provisional default or A's candidate. Keep normal apex controls available
with the full lifecycle; do not quietly introduce recurrent predator restocking.
Do not wait for sustainability or C's completion. If C later produces a policy
worth trying, make it accessible through ordinary supported controls and report
its limitations; do not silently reset the display again to install it.

Verify build/config identity, fresh tick progression, viewer output and the
owning process. Report what was deployed, controller mixture, initial population
and apex state. Do not claim the physical cube was inspected if only the viewer
was available. An external blocker should leave a concrete ready-to-run command
and an honest status, not trigger unrelated infrastructure work.

## Deliver and stop

Use Git commits for authored work and one normal build cache. Keep compact
configuration/protocol records, tables and only the policy artifacts needed to
reproduce/evaluate the selected run. Ask before generating more than 1 GiB;
existing unrelated data cleanup is outside scope.

Write `design/7_Research/ecology-v1-next-results-2026-09-15.md`, linking concise
worker results as needed. Include the dispatch/budget checkpoint, actual
wall-time/worker use and model usage if exposed, commits, checks, experiment
matrix, observed trade-offs, held-out outcomes, visual evidence, training result
and live deployment status. Update the handoff index. Mark each workstream done,
partial or blocked; distinguish a completed assignment from a healthy ecosystem.

Start A and B now. Continue through their dependent steps within these bounds.
Do not ask Wrysk to reconfirm routine implementation choices or these already
scoped runs. At the caps, return the evidence and a concrete next recommendation;
do not spend further on repeated reviews, research or search campaigns.
