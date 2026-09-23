---
design_status: exploration
status: open
last_reviewed: 2026-09-18
decision_refs: []
---

# Reuse geometry in study eligibility observations

Wrysk requested this handoff on 2026-09-18 following the performance survey.
This is study-harness performance work. Animal sensing and its identity-tracking
queries are deferred while sensing is actively redesigned. Read
[working policy](../../WORKING_POLICY.md); use the flora/harness worker.
This package is independent of the two water packages, apart from coordinating
any edits to the shared `cubarium-voxel-sim/examples/bench.rs`.

## Evidence and entry points

The survey measured about 142 ms per eligibility observation: 3,072 skyline
sites × six species. This is outside the simulation phase timings; it is not
142 ms on every tick. The benchmark's sample-every-100 setting would add about
1.42 ms/tick from this observer alone. Actual studies have their own cadence.
See [original observer measurement](../7_Research/voxel-tick-profile-2026-09-18.md#what-the-harness-observers-cost-which-is-not-the-tick).

Start at flora `examples/harness/mod.rs::{passes, eligible_sites}` (425–427,
1549–1562), `examples/two_producers.rs::{eligible_sets, gate_diagnosis}`
(486 onward), and `examples/replacement.rs::settle_phase` (853–969).
The bench replica is `cubarium-voxel-sim/examples/bench.rs::Observers::measure`.
The model path is `FloraView::establishment_gates` →
`step::establishment_gates_on_substrate` → `gates`; `sky_at` already shows a
simulation-side sky cache. Follow current callers before choosing the API.

## Work

- Share geometric sky visibility across species at the same site, and reuse
  it across observations while terrain is unchanged. Keep the same hemisphere
  ray calculation. Vertical sky exposure and plant canopy shading are not
  substitutes for this value.
- Provide the smallest reusable model-owned cache/batch entry point needed by
  the actual study callers and bench. Keep one establishment predicate: do not
  copy gate formulas into the harness or optimize only the benchmark replica.
  Carry the flora view's real dead-wood substrate input for fungi.
- Tie cached geometry to the correct world and terrain revision. Reset/load,
  clones and matched study arms must not reuse another world's stale cache
  merely because dimensions and version numbers match. Bound cache lifetime
  and memory to the study/world; no process-global accumulating cache.
- Re-read pore water, saturation, standing-water depth and dead wood on each
  observation. Do not cache gate outcomes across ticks. Root-box membership
  caching is a follow-up only if the post-sky profile justifies it, with keys
  covering species/config geometry as well as site and terrain.
- Preserve eligibility membership/order, individual diagnostic fields,
  observation cadence, founder/site selection, conditioning/stopping rules
  and study output meaning. No ecology tuning or reduced sampling frequency.

## Check and return

Compare cached and uncached **gate fields and eligible sets**, not just counts,
on small open/roofed fixtures, across a terrain edit, and across worlds/arms.
With fixed terrain, change pore water, water depth and dead wood and verify
the next observation responds. Include the saprotroph substrate gate.
Run only touched-crate tests; build the affected study examples. A short
helper smoke is enough; do not launch a replacement or conditioning study.

Time both the actual shared observation helper and the bench observer on the
same prepared state, all six species. Report cold cost, repeated warm cost,
post-terrain-edit rebuild cost, and cache memory; keep preprocessing visible.
Use a bounded release probe (for example 20 repeated observations, seeds 1–3).
The existing bench command is
`target/release/examples/bench 1000 0 50 <seed> 100 8` after building with
`cargo build --release -p cubarium-voxel-sim --features profile --example bench`.
Report ms/observation and amortized cost at the caller's actual cadence,
separately from simulation ms/tick. Do not claim the whole 142 ms is sky work.

Return the commit and concise before/after measurements. No new report file,
deployment, captures or retained benchmark binaries/data.
