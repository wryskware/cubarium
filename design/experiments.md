---
design_status: leaning
last_reviewed: 2026-09-11
decision_refs: []
---

# First experiments and their decision gates

These are proposed protocols derived from E1–E9 in the
[review](7_Research/plan-review-2026-09-11.md), with qualifications documented
in the [response](7_Research/plan-review-response-2026-09-11.md). None has run.
They are development tools; the installation has no evaluation score or UI.

## Common protocol

Record build/config/schema hashes, seed list, initial state, mechanism toggles,
simulated duration, wall time, all resource sources/sinks, and recovery events.
Pilot runs choose scales and numerical tolerances. Freeze settings and declared
success/failure criteria before running the held-out seed batch; do not tune on
it and present the same seeds as independent validation. Report failures and
time-outs, not only the best-looking worlds.

Use control pairs with matching initial conditions and process-specific random
streams. Divergence after a mutation/event is expected; matching seeds controls
the initial opportunity, not every subsequent ecological encounter. Compare
multiple seeds and distributions. No single scalar combines the metrics.

E1/E2/E4/E5 inform whether to proceed into M3b. E6 informs the visual grammar;
E3 informs the promised time scale. M1 remains the first implementation task.

## E1 — transport bias and field symmetry (M1)

Two complementary checks:

- A cell random walk on the 1,280-cell graph chooses one of four directions
  uniformly, staying put when it chooses the open bottom. Reciprocal transitions
  then have a uniform stationary distribution; choosing only actual neighbors
  would weight cells by degree instead. Check the operator algebraically before
  sampling long-run occupancy.
- Continuous, noninteracting walkers use isotropic direction refresh, fixed
  speed, pure reflection, no costs, no habitat preference, and no rim attraction.
  Compare area-normalized occupancy over burn-in-separated batches with several
  seeds. Persistent oriented walks require a mixing/correlation check; the
  histogram is not expected to be exactly flat in a finite sample.

For diffusion, run source/rotation pairs under symmetries preserving the five-face
domain. Around a top vertex, deposit equally on the three incident cells and
compare local early-time responses, or compare three rotated single-source
fixtures. A source on just one cell does not imply a symmetric result by itself.
Whole-world Top/side permutation is invalid because it moves the open bottom.

**Gate:** reciprocity and conservation pass numerical tolerances; measured
occupancy has no repeatable bias exceeding predeclared sampling error; rotated
responses match where their domains/boundaries correspond. Keep seam, corner,
and 3D-oracle fixtures even if these aggregate tests pass.

## E2 — local succession versus global crash cycles (M2)

Seed a fixed reproducing grazer/scavenger genotype, first from a single ancestor
and then from the normal spatial founder population. With mutation, dormancy,
predation, and reseeding off, sweep a small declared matrix of producer recovery,
nutrient diffusion, movement speed, and patch spacing. Include a static habitat
control before adding slow forcing. Log the matrix; do not promise a giant
exhaustive sweep before measuring runtime.

Measure resource spatial correlation length, local depletion/recovery lag,
time to traverse a patch, population coefficient of variation, lagged population
cross-correlation for faces and fixed spatial patches, and time at capacity.
Also record empty/occupied area and visible biomass: count alone cannot separate
tiny-dot saturation from a readable world. Show raw trajectories when low
counts/trends make correlation unstable.

**Gate:** identify multiple seeds/configurations with repeated local feeding and
detritus succession before the whole cube empties. Persistent cap-to-crash
synchrony across tested conditions blocks added complexity. The 100–200 active
count is a tuning hypothesis, not a survival pass/fail quota or an input to
live population control.

## E3 — actual evolutionary opportunity (M2 onward)

Record parent age at birth, time to first reproduction, birth/death counts,
ancestry depth over simulated time, lineage survival, and reproductive skew.
Report intervals and distributions. Births per hour per lineage help explain
activity but are not interchangeable with generations/day, especially when
parents reproduce repeatedly. Effective population size needs additional
assumptions; do not infer it from 72 founders or the 512 capacity.

**Gate:** revise time-scale claims and evaluation duration to match measured
reproductive opportunity. A few descendants in a week cannot support a claim
of deep adaptation; rapid births alone do not demonstrate useful variation.

## E4 — the minimal-grazer attractor (M3a, before predation)

Freeze behavior and body presentation apart from size. Mutate only size,
metabolism, sensing range, and reserve capacity, using the bounded sparse policy.
Grazing/scavenging and heterogeneous substrate remain active; mutation controls
use identical fixed genomes. Predation, autotrophy, dormancy, and reseeding stay
off. Run 12 seeds for 24 simulated hours in the candidate M2 habitat, with
declared representative habitat variations if the first batch collapses.

Measure trait distributions and occupancy near each lower bound, parent/child
changes, survival across lean patches, intake/maintenance by size, and whether
trait differences accompany different local strategies. Separate surviving
founder variation from new inherited variation. Include extinct runs.

**Gate:** if all successful lineages converge to the cheapest bounds across the
tested opportunities, revise mouth/reserve/locomotion costs or habitat structure
before adding predation. Demonstrated differentiated strategies support moving
on, but do not prove asymptotically stable polymorphism in every world. Never
protect a size class with quotas or bonuses for rarity to make this test pass.

## E5 — mutation viability and readable habits (M3a)

For each viable founder in a small set of feeding strategies, generate 1,000
independent mutant samples, each one sparse mutation step from that founder.
Run each separately in the same declared habitat suite for a bounded lifetime;
an unmutated founder is the control. Include a familiar viable patch and a
challenge such as patch depletion or changed moisture. Offspring can be counted
and then removed from the measurement scene to isolate the parent's behavior;
any resulting resource changes are part of the declared test harness, not live
world rules. This is diagnostic sampling, not a replacement population or GA.

Measure fractions that feed, survive to the observation window, and bud; also
measure pause durations, path curvature, directed feeding response, patch
residence, and alarm persistence. Select a declared sample for blinded real-time
clip comparison, including failures and unchanged mutants. Nameable gene changes
alone do not establish nameable behavioral changes.

**Gate:** a useful fraction of mutants must remain viable and show repeatable
behavioral differences beyond founder repeat-run variability, using pilot-defined
criteria frozen before the held-out batch. If reflex mutations fail, inspect
costs/mutation size before increasing controller dimensions. A residual network
comparison is conditional on a named missing behavior; do not implement the
deferred dense network solely to run this initial test.

## E6 — small visual grammar (M1 gallery, finalized in M3a)

Show twelve samples at native resolution on the physical cube: vary lobe count,
length/aspect, one appendage, and gait while keeping palettes controlled. Include
related/unrelated pairs, same-color different-behavior pairs, and top-seam/vertex
crossings. Compare still silhouettes with real-time motion clips. The owner
judges them; a second viewer is useful when available, not a project dependency.

Ask which are related, which way they move, and which visibly feed, pursue,
pause, or bud. A pointed shape alone does not prove a hunting role; use actual
behavior for action recognition. Record viewing distance, brightness, room
lighting, pairing accuracy, ambiguities, and comments.

**Gate:** keep controls that produce repeatable visible distinctions. Remove
indistinguishable ornaments before coupling a large organ grammar to physiology.
Record physical review as pending if the cube is unavailable; preview results
cannot silently substitute for it.

## E7 — what predation can sustain (M3c)

In a stable fixed grazer world, introduce a fixed specialist hunter with paid
resources, no mutation, and no recovery. Compare 12 seeds with no hunter and
with a facultative grazer/hunter at matching declared resource budgets. Record
specialist extinction time (including right-censored survivors), offspring,
capture success, handling costs, prey crashes, and mixed-diet resource use.

**Gate:** choose costs and scope from observed outcomes. Episodic specialist
hunting is acceptable. If all predation variants consistently erase the world
or never feed/reproduce, repair them before mutating both sides. Do not claim
a stable predator guild from a single surviving founder.

## E8 — pure reflection on the cube (M1/M2)

Watch ten minutes with no rim sensing. Include approach angles, speeds, and
steps that cross a side seam while reflecting off the bottom. Separate a bad
numerical bounce from an aesthetic dislike of reflection. Observe crowding and
edge residence with food held uniform, then under ordinary habitat conditions.

**Gate:** keep pure reflection unless it causes a documented problem. A later
rim drive must be local, paid through ordinary motion, heritable, and able to
reach zero. Do not reserve a wide invisible border without ecological access.

## E9 — persistence with and without storage/rescue (M4a)

Run a two-by-two comparison: paid dormancy on/off and extinction reseeding
on/off, with matched initial total material/energy. Enabling dormancy reserves
real resources; it does not begin with a free extra seed bank. There is no
historical-library recruitment in any arm. Distinguish zero active organisms
with live propagules from extinction of both active and dormant life.
Stationary but living creatures still count as active; low motion cannot
trigger reseeding.

Report time to mobile extinction, time to lineage/propagule extinction, lengths
of inactive intervals, endogenous germination, external reset count, and material/
energy admitted per reset. Summaries include all seeds and censored survivors.
Do not collapse the four arms into one favorable survival median.

**Gate:** state what actually sustains the world. Dormancy may be valuable even
when the no-dormancy world dies, but that dependence must be explicit. Repeated
exogenous resets across most seeds require ecological revision before claiming
unattended continuity. Determine the acceptable inactive/reset cadence from
recorded real-time viewing, not from a hidden fitness or diversity target.
