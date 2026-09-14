---
design_status: exploration
last_reviewed: 2026-09-14
---

# Opus handoff: R0a movement foundation and food measurements

Work in `/home/wrysk/wryskware/cubarium` in a fresh Claude Opus thread. This is
the deliberately narrowed portion of R0 that Wrysk can start while awaiting
Fable's review of the broader recurrent-organism plan. Forwarding this handoff
as an implementation request starts this milestone only. Do not start R1–R4.

Read `AGENTS.md`, `WORKING_POLICY.md`, the personal `bounded-agent-work` skill,
and the canon rules/ledger. Then read this handoff and sections 2–4 of
`design/recurrent-organism-plan.md`. Current user instructions take precedence.
Use Graft for exact source context and caller/override coverage. Protect other
authored changes; no full-history agent resumes or unrelated cleanup.

## Deliverable

One coherent implementation of body-scaled turn limits and paid physical motion
for existing organisms, with focused regression checks and a short factual
food-stock/flow measurement. Leave a small controller-independent motor boundary
that a later RNN can request actions through. This is useful under either the
existing controller or the future one.

It is not a complete foraging redesign. Hungry relocation, neural decisions,
producer architecture and sustainable balance still await the broader review.

## Settled direction for this milestone

- Organisms may pivot in place. They are not cars and need no forward motion to
  turn. The outer body's swept speed must respect movement capability.
- Larger physical bodies cannot rotate like points. Translation and rotation
  are both physical effort and must be limited by available energy.
- Ordinary fauna, apex, pursuit, escape and encounter overrides share those
  physical limits. No direct-heading override bypasses the final resolver.
- A seam chart transformation is coordinate transport, not paid physical turning.
- Food consumption is real and finite. Before choosing a replacement regrowth
  model, report what current local consumption and renewal actually do.

Exact coefficients and motor API details are revisable implementation choices,
not canon. Keep them small and centralized so Fable can revise them cheaply.

## A. Implement the motor foundation

1. Trace all writers of heading, movement effort and bursts, including direct
   assignments in `world.rs`, not just callers of `turn_toward`. Record a compact
   coverage list. Current anchors include `controller.rs::decide_quiet` and
   `turn_toward`, hunter pursuit around `world.rs:1595`, escape around `1672`,
   encounters earlier in the step, and common movement around `1682–1724`.
   Verify locations against the current tree; these line numbers are pointers.
2. Separate requested movement/turn from resolved movement. Adapt the existing
   decisions at one final boundary after behavioral overrides. Preserve current
   behavioral choice wherever possible; do not rewrite it into new search,
   approach, feeding or resting heuristics.
3. Implement a pure, testable motor-budget resolver. Proposed initial envelope:
   `|v| + r × |ω| ≤ u_available`. It permits pure pivoting. Use signed shortest
   physical turn requests, current body extent and the available locomotion
   capability after relevant effort/environmental limits. Document simultaneous
   request scaling. Do not compare angular sweep with actual center speed: that
   would prohibit the pivoting Wrysk explicitly wants.
4. Derive `r` from the current physical body/scale, including relevant apex
   contact geometry. Explain any conservative approximation and exclude purely
   decorative effects. Do not hardcode one radius for every size or use rendered
   pixel coordinates as physical geometry.
5. Make motion affordable before applying it. Reserve unavoidable upkeep
   according to the existing physiology ordering, then constrain paid movement;
   charge translation and turning exactly once and send costs to the existing
   heat ledger. Fix any path that currently grants ordinary movement after
   movement energy is exhausted. Do not change base maintenance or metabolism
   merely to make this adjustment easier.
6. Reuse surface/shim travel and tangent transport. The presenter must receive
   resolved poses and travel. If any prediction/interpolation reintroduces a
   heading snap, fix that narrow integration rather than changing visual style.

Expose tuning constants in a small existing configuration or versioned policy
surface where practical. Keep acceleration state, a new contact solver, and a
complete neural action ABI out of this slice unless strictly necessary for
correctness. The full plan's acceleration/contact refinements remain follow-up
work; do not label this slice a complete physical simulation.

Zero motor requests must hold the body still. Distinguish actual requests from
legacy `Mode` labels: a fleeing animal may still carry a resting/feeding label,
so mode alone must not disable a legitimate movement override or exempt it from
payment. Explicitly test the apex's late mode/effort overrides so early steering
cannot retain an unlimited rotation budget.

If persisted configuration/state changes are required, implement the needed
migration and same-build continuation tests within this slice. Do not silently
reset a loaded world, invent an RNN migration, or change schema just to reserve
future fields. Consult the shim's current contract before geometry work.

## B. Measure the food question without deciding the redesign

Use one small diagnostic on the real core with explicit controlled fixtures.
Compare an undisturbed patch, one consumer, and several consumers; also observe
recovery after consumption ceases. Fix initial inventories and environmental
conditions across matched cases. A fixed feeding footprint is acceptable for
isolating rates, provided it is clearly a staged measurement rather than an
ordinary-world behavior demonstration. Preserve metabolic/resource accounting.

Report absolute local edible stocks, gross production, actual intake, competing
shares, nutrient/energy changes, time below profitable feeding levels and
recovery observed within the horizon. Do not infer intake from reserve deltas
or Feeding mode. Distinguish unreachable food, low chemical quality, saturated
reserves, and insufficient mouth throughput where they explain the result.
List values that remain unmeasured/censored rather than extrapolating them.

At most four fixture runs, 12,000 ticks each, two workers and 60 seconds wall
time in total. Count retries within those limits. No parameter sweep, seed
campaign, GPU work, capture collection or changes to M1. If existing diagnostics
already provide these quantities, reuse them. Add only the narrow transient
measurements needed; no general telemetry framework.

Do not choose or implement a new edible-tissue pool, root model, cell resolution,
odor diffusion, reproduction balance or global regrowth change in this handoff.
The measured stock/flow relationship is an input to Fable's review.

## Checks and completion

- Pure pivot, pure translation, simultaneous requests, large/small and juvenile
  bodies, low/zero available energy, and zero requests satisfy the chosen bound.
- The angular distance paid is the actual resolved physical turn, not a chart
  jump or an unexecuted request; combined costs reconcile with energy/heat.
- Exercise ordinary steering, apex target alignment, escape bursts and encounter
  retreats through the shared resolver. Verify feasible contact/attack timing
  still functions; do not restore instant heading changes to save an old test.
- Exercise a seam/rim path and same-build snapshot continuation when affected.
- Run the relevant core/presentation suites. Add a regression for the reported
  late-override spinning defect. Tests that encode the old free/instant movement
  behavior must be revised around physical correctness, with the reason stated.
- Provide a short native-size development demonstration of moving and pivoting
  bodies using normal assets. Inspect rather than archive captures. This shows
  motor correctness, not learned foraging or ecosystem health.
- Deliver one compact result: changes, checked overrides, tests, food measurements,
  performance/storage deltas, remaining questions for Fable and actual model
  usage if available. Explicitly say when usage is unavailable.

Use one normal build cache. At most one targeted review and two repair cycles.
No agent spawning or additional model review unless separately requested. If
this becomes a broad redesign, stop and name the dependency instead of absorbing
R0's remaining work. Check before exceeding 1 GiB additional storage.

Ship verified development changes under `WORKING_POLICY.md`, including the
normal current-checkout development update when appropriate. Preserve world
state; no reset is authorized here. Coordinate with the current runner owner
and do not disrupt unrelated services. Git holds recovery history; do not retain
frozen binaries. Refresh Graft after substantive code changes.

The first handoff ends here. Fable's review still precedes committing to the
resource redesign, complete neural interface, GRU sizing, optimizer/objectives,
inheritance details and subsequent milestones.
