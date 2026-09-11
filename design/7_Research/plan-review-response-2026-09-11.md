---
design_status: exploration
last_reviewed: 2026-09-11
decision_refs: []
---

# Response to the pre-implementation plan review

Source: [Claude Fable 5.1 review](plan-review-2026-09-11.md), supplied by Wrysk.
This response records the planning revisions and their rationale. The original
review is preserved as submitted. Neither the review nor this response promotes
architecture to canon. The ledger and accepted brief remain unchanged.

The review's strongest contribution is turning several acknowledged risks into
earlier tests and smaller initial mechanisms. Adopt that direction: readable
drives with memory, an initial grazer/scavenger ecology, one extinction fallback,
and individually justified additions. Mathematical/behavioral predictions still
need their stated conditions or actual measurements; no runtime has been built.

## Finding dispositions

| Finding | Disposition in the revised proposals |
| --- | --- |
| F1 — dense recurrent controller | Adopt named drives plus two leaky memories as the default; sparse 1–3-locus mutations; defer a network to a specific missing-behavior experiment |
| F2 — minimal grazer and sessile autotroph | Promote both to primary risks; make mouth/reserve costs explicit; E4 precedes predation; omit organism photosynthesis through M3 |
| F3 — global synchrony | Add height/patch gradients, slower initial motion, a tuning range and spatial-cycle telemetry; test actual coupling rather than assuming gradients solve it |
| F4 — recovery hides failures | Remove library/low-population recruitment; keep paid dormancy and one logged extinction reseed; archive becomes observer-only; add factorial controls |
| F5 — overloaded M4 | Move scavenging and slow forcing to M2; split M3 into representation, inheritance, and facultative predation; optional M4 mechanisms each get a prediction and control |
| F6 — geometry additions | Retain existing seam model; specify an independent 3D oracle, chord-based broad phase, qualified bias tests, vertex pixel ownership, and combined rim/seam fixtures |
| F7 — rim | Pure reflection first; add heritable zero-capable avoidance only if E8 shows a problem |
| F8 — simplifications | One simulation rate, no initial bins/signals/waste/sensor noise/stress mutation; renderer-only trails; minimal substrate filtering |
| F9 — visible traits | Limit initial geometry controls and emphasize gait; E6 tests native-size samples before expanding physiology/ornament coupling |

The [overview](../README.md), [ecology](../ecology.md), [evolution](../evolution.md),
[topology](../surface-topology.md), [architecture](../architecture.md),
[appearance](../appearance.md), [inputs](../environmental-inputs.md), and
[implementation plan](../implementation-plan.md) are updated together.
[Experiments](../experiments.md) preserve E1–E9 identifiers with corrected protocols.

## Qualifications and disagreements

### F1's conclusion is stronger than its inputs

The old plan named small perturbations but never specified that every birth
mutates every weight. It did not establish effective population size. Its
10–90 minute row described ecological/lineage turnover, not a measured generation
interval. Therefore the review's quantitative inevitability claim does not follow
from those documents. Dense parameter changes can also produce recognizable
habits; nameable behavior does not require every gene to have a name.

Still, a smaller, inspectable representation is the better starting engineering
choice for this display. We adopt it for lower tuning/attribution cost, not
because a network has been proved incapable. Reflex combinations are not
globally monotone, and legal decoded mutants are not viable by construction.
E3 and E5 measure reproductive opportunity, viability, and visible habit changes.

### Size diversity needs an honest gate, not a guaranteed polymorphism

The review correctly identifies lower-bound attraction as a major risk. Mouth
investment now explicitly affects intake and proportional allocation from a
contested cell. This does not guarantee coexistence: cost scaling could favor a
large-mouth strategy just as easily. We do not add arbitrary size-based priority.

E4 must test whether differentiated strategies emerge under grazing/scavenging
alone and force a response to repeated all-minimum convergence. Requiring an
asymptotically stable polymorphism in every seed would exceed what a finite run
can establish and could incentivize hidden quotas. Report both convergence and
temporary diversity. Later autotrophy requires shared light budgeting and edible
fan area; lower efficiency than producers alone is not sufficient protection.

### Slower motion and gradients reduce a risk, not prove its absence

The synchrony prediction is plausible but speeds, diffusion coefficients, and
intake rates had not been specified, so a first-hour cap/crash is not an observed
or inevitable result. Height variation need not make regions peak at different
times. E2 examines travel/recovery scales and spatial correlations under static
and moving weather. The 100–200 population range guides between-run tuning,
never a live density controller or per-face quota. Specialist predation can be
episodic; facultative digestion still costs investment.

### Correct the proposed symmetry tests

A random walk that chooses uniformly among only existing neighbors favors
higher-degree cells. For the scalar graph, choose four directions with a
self-loop at the open rim to give a symmetric transition operator and uniform
stationary density. Continuous walkers require an isotropic direction process,
mixing/burn-in checks, and sampling tolerances; evolved organisms are not expected
to have flat occupancy.

A delta on one of three vertex-adjacent cells is an asymmetric initial condition.
Use rotated source runs or an equal three-cell deposit for a local symmetry
test. The whole five-face world is not invariant under exchanging Top with a
side, because that changes the open boundary. A local early-time test must account
for boundary influence and solver stencil support. These qualifications prevent
the proposed tests from incorrectly rejecting valid geometry.

### Geometry reference and performance remain work to do

The review reports independent verification of the written seam arithmetic and
the existing shim. That is useful evidence, not execution of Cubarium tests.
The new oracle uses 3D edge rotations independent of production 2D seam formulas
and validates entire paths. Vertex ambiguity and rendering ownership still need
implementation/physical review. No claim about a few dozen lines is a delivery
estimate. The 130,816 unordered pairs at capacity is an arithmetic bound; actual
query cost, especially exact unfolding, must be profiled on the host.

Shortest-path pixel ownership prevents duplicate light but cannot preserve a
rigid planar body across a cube vertex. The plan explicitly accepts and reviews
a localized shape discontinuity. Normal seams still require visual continuity.
The shim's physical Top correction is never reproduced by Cubarium.

### Visual and experiment simplifications

Adopt a smaller grammar, but do not infer that all proposed ornaments are
indistinguishable before seeing them on LEDs. E6 tests silhouette and motion
separately, with related/unrelated samples and real behavior. A fan is not
included as a pretend photosynthetic organ while that physiology is absent.

E5 first tests reflex mutants and founder controls; a network comparison remains
conditional, avoiding implementation of a discarded default solely for a test.
E9 separates dormancy from external reseeding rather than labeling all continuity
mechanisms identically. Minimal seam-aware visual filtering is the starting
choice; interpolation can improve if visible block boundaries demand it.

## What follows

This revision completes review integration at the planning level. The next
implementation is still M1: reuse the shim, prove continuous surface travel,
pure reflection, vertex ownership, and the independent oracle, then run E1/E8.
The critical ecology/controller experiments follow M2 and M3a. No experiment
results, new accepted design decisions, or implementation are implied by this
response. Architectural promotion remains a separate specifically authorized act.
