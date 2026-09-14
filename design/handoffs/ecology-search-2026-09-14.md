---
design_status: exploration
last_reviewed: 2026-09-14
---

# Claude Code handoff: whole-ecosystem search, with a GPU path

Work in `/home/wrysk/wryskware/cubarium`. Use Claude Opus for this bounded
implementation milestone. Read `AGENTS.md`, `WORKING_POLICY.md`, the personal
`bounded-agent-work` skill, and the canon instructions before design material.
Use Graft for targeted code retrieval. Do not load old agent transcripts.

## Objective and current state

Build tools to find parameter combinations that sustain a varied, changing
Cubarium ecosystem using fast headless simulations and a genetic algorithm.
Tune the whole ecology together: plants, nutrient recycling, prey, predators,
and their costs/conditions. Predator survival against frozen prey is inadequate.

The real simulation is the Rust `cubarium-core` crate. Its paid apex dormancy,
two-parent mating, and combat were implemented at `abbb051`; snapshot schema 14
persists those additions. Another thread is adding live random apex-spawn
controls. Coordinate around that work: own the search harness and its tests;
do not edit viewer/control code or change live parameters. No new visual design.
Implemented features are available for live experimentation even if unbalanced;
search results are not a release gate.

Host hardware: RTX 5090 with 32607 MiB VRAM, driver 615.71.09, and 32 logical
CPU cores. Verify toolchain availability. No GPU simulation backend has been
established or benchmarked. The normal `target/` cache was about 11 GiB at
handoff; do not create alternate caches or exceed 1 GiB of additional storage
without a checkpoint.

## First milestone: a bounded real-core search prototype

1. Inspect `crates/cubarium-core/src/config.rs`, `world.rs`, `dormancy.rs`, and
   `encounter.rs` through targeted graph results. Use the real tick loop and
   conservation checks. `examples/apex_screen.rs` illustrates headless APIs and
   event collection, but its stationary juvenile prey are a staged mechanism
   fixture: do not reuse that opening as a realistic ecological evaluation.
2. Select a small joint vector of existing configurable ecological parameters.
   Document exact fields, defaults, proposed bounds, and why each matters.
   Include producer renewal/recycling and prey costs/reproduction as well as
   relevant apex controls. Identify hardcoded candidates explicitly; do not
   expand into a general configuration rewrite.
3. Run one actual-core baseline to measure throughput and memory. Start with
   independent-world CPU batching as the measurable reference. Initial worlds
   must have ordinary movement, feeding, reproduction, and real resource
   inventories. Give candidates comparable starting resources and seeds. Any
   initial apex cohort is an explicit accounted input; no restocking mid-run.
4. Implement deterministic initialization, bounded mutation/crossover, elitism,
   seed scheduling, component metrics, and hard evaluation/tick/worker limits.
   Keep candidate configuration separate from inherited organism genomes.
   Reject invalid configurations and record failed simulations instead of
   silently discarding them or repairing their ecology.
5. Measure plant/prey turnover, ecological variety, reproductive lineages,
   offspring maturation, recovery after predator pressure, and terminal
   collapse. Distinguish dormant persistence from active life. Neither maximum
   population, immortal unfed bodies, nor one dominant species is sufficient.
   Apex activity may be episodic. Keep component results and nondominated
   candidates visible even if a scalar rank is used to select parents.
6. Test determinism, parameter/budget validation, and resource invariants. Then
   run one smoke search capped at **8 candidate evaluations, 2,000 ticks per
   evaluation, and 4 workers**, with an explicit wall-time cutoff informed by
   the baseline. These proposed limits test the tooling, not sustainability.
   No automatic seed/horizon sweeps or retries to obtain prettier outcomes.

## GPU decision

Assess a faithful batched GPU backend from actual hot paths, data layout,
numerical precision, dynamic organism capacity, RNG, and resource accounting.
Many independent worlds may offer parallelism, but existing Rust methods cannot
simply be called from a GPU kernel. Keep state resident where feasible; include
transfer/synchronization costs in any proposed comparison. Verify relevant GPU
capabilities against primary documentation when needed; do not invent speedups.

If GPU execution requires a substantial port, finish the usable CPU baseline
and return a separate bounded GPU milestone naming the first kernel/state
boundary and CPU/GPU equivalence tests. A simplified GPU ecosystem is a separate
model and must not be sold as an acceleration of the real core. Before a larger
search, compare measured throughput and fidelity on matched initial worlds.

## Deliver and stop

Deliver the harness, focused tests, one compact smoke result, exact replay
commands, measured throughput/storage, and a concrete GPU recommendation.
Propose a separately capped follow-up using held-out seeds and longer horizons;
a short run cannot demonstrate indefinite sustainability. No full workspace
rebuild by default, model-review loop, capture archive, frozen executable,
background campaign, live deployment, or unrelated cleanup. At most two repair
cycles. Report actual Claude usage if exposed.

Supporting plan: `design/ecology-search-plan.md`. Start by implementing this
milestone; do not spend a new thread rewriting the same plan.
