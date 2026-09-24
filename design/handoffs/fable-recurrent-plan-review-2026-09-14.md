---
design_status: exploration
last_reviewed: 2026-09-14
---

# Fable handoff: review the recurrent-organism plan

Work in `/home/wrysk/wryskware/cubarium` in a fresh thread. Perform one focused,
independent design review before Wrysk authorizes the remaining implementation.
The deliverable is a review with concrete proposed corrections, not implementation.

## Read first

Follow `AGENTS.md`, `WORKING_POLICY.md`, and
`design/0_Canon/README.md` / `DECISIONS.md`. Then read:

1. [Recurrent-organism plan](../recurrent-organism-plan.md) — primary review target.
2. [Movement and foraging overview](../movement-and-foraging-plan.md) — motivation.
3. [R0a handoff](r0a-movement-foundation-2026-09-14.md) — limited early-work scope.
4. [R0a food measurements](../7_Research/r0a-food-stock-flow-2026-09-14.md).
5. [R0a motor-cost measurements](../7_Research/r0a-motor-cost-ecology-2026-09-14.md).

Use Lore for relevant design evidence and Graft for targeted current source
verification. If unavailable or stale, use exact referenced files. Verify current
R0a status; the original plans predate these reports. Reports are evidence with
limitations, not accepted design decisions or proof of current deployment.

## Owner intent and decisions

Wrysk sees organisms, including apex, mostly spinning/wiggling in place. The goal
is an interesting ambient ecosystem where sensory experience, real needs and
finite opportunities produce activity, rest, competition and reproduction.

- Pivoting in place is allowed. Bodies must respect movement capability; no
  compulsory forward motion or car-like steering radius. The particular shared
  speed envelope and energy coefficients remain open to technical critique.
- A creature need not have an explicit destination. Local gradients, search,
  memory and other sensory responses are valid. Do not prescribe universal travel.
- Proceed directly to recurrent neural control; do not add an MLP comparison.
  GRU versus other recurrent choices, sizing and training method are proposals.
- Prefer learned behavior over an expanding collection of behavioral heuristics.
  The world still enforces physical possibility, resource accounting and physiology.
- Include reproduction and reproducing descendants. Explicitly identify any
  behavior that remains outside neural control and any capabilities deferred.
- Resolve the behavioral/ecological design before adapting M1 sustainability
  search. Later tuning should evaluate the whole ecosystem together.

The early R0a scope covered shared paid movement and food measurements. Its
implementation does not settle the remaining resource, neural or training design.
Challenge proposals freely; distinguish proposed changes from owner decisions.

## Questions the review must resolve

1. **Diagnosis and food:** Do the new measurements support the original diagnosis?
   Stationary consumers reportedly starved and recovery remained incomplete.
   Separate standing stock, mouth throughput, feeding thresholds, accessible
   footprint, renewal and behavior. What actually needs redesign, and what can
   remain? Do not assume excessive regrowth from visual inactivity, or generalize
   one staged fixture into a whole-world conclusion.
2. **Embodiment:** Does the implemented motor envelope use an available speed that
   actually matches translational capability? Check units, reference-radius
   normalization, size/effort scaling, pivoting, seams and all overrides. Separate
   kinematic limits from energy pricing. Assess whether the rotation-cost report's
   geometric argument justifies its work model and whether its ecological claims
   exceed the experiments. Recommend the smallest coherent contract.
3. **Recurrent interface:** Are observations sufficient but local, actions capable
   of the promised behavior, and memory timescales credible? Examine sensor costs,
   aliasing, body differences, contact feedback, update rate, persistence and the
   consequences of fixing the ABI before mating is fully specified.
4. **Learning feasibility:** Is the proposed mutation GA a sensible first learning
   method for this recurrent policy and budget? Compare credible recurrent
   alternatives where useful, including gradient training, without an MLP arm or
   optimizer survey. Examine discoverability, sparse objectives, mutation damage,
   hidden-state handling, curriculum, evaluation horizon and held-out discipline.
   Separate a bounded feasibility screen from evidence that the method cannot work.
5. **Lifecycle and variety:** Can policies learn paid reproduction, mate choice and
   refusal, and produce viable reproducing descendants? Address inheritance,
   mutation, body-policy compatibility, juvenile behavior, dormancy and remaining
   heuristic ownership. Does individual training transfer to interacting diets
   and predator/prey populations? Avoid equating an archive with coexistence.
6. **Sequence and acceptance:** What must be settled before each milestone? Identify
   premature infrastructure, missing prerequisites and tests that reward proxies.
   Recommend the next single bounded implementation handoff after review, including
   measurable acceptance criteria and a spending checkpoint.

Check the pivotal research claims against linked primary sources. Follow up only
where it could change a recommendation; no broad literature expedition. Clearly
separate owner intent, observed implementation, experimental evidence, inference
and your recommended choices. Do not manufacture a blocker for every uncertainty.

## Deliverable and stopping point

Write `design/7_Research/fable-recurrent-plan-review-2026-09-14.md` with exploration
frontmatter. Lead with whether the plan is ready, ready with named revisions, or
needs a specific redesign. Include:

- Prioritized substantive findings, each with an exact plan/source reference,
  practical consequence and concrete correction; distinguish blockers from tuning.
- A compact keep/change/defer table and recommended milestone sequence.
- Explicit behavioral coverage and exceptions, including reproduction.
- The few decisions Wrysk actually needs to make, with your preferred option.
- The next bounded handoff's scope, checks and unresolved evidence needs.

Keep the report focused, approximately 1,500–2,500 words unless a material finding
needs more. Do not rewrite the original plans or promote canon during this review.
Do not modify simulator code, run training/sweeps, change the live world, reset it,
or spawn more reviewers. One review only; stop after delivering the report and a
short user-facing verdict. Report actual usage if exposed; otherwise say unavailable.
