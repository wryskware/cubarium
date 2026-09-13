---
design_status: leaning
last_reviewed: 2026-09-11
decision_refs: []
---

# Implementation plan and validation gates

Build a series of usable vertical slices. Topology precedes ecology, persistence
arrives with the first living world, and more traits follow evidence that the
existing ones produce observable consequences. Milestones below are proposed,
not estimates or claims of completed implementation.

Revised after the [Fable review](7_Research/plan-review-2026-09-11.md).
[Review dispositions](7_Research/plan-review-response-2026-09-11.md) record
reasoning and qualifications; the [experiment plan](experiments.md) gives E1–E9
protocols. These revisions remain `leaning`; completed experiments are recorded
in the current-status paragraph and the linked research reports below.

## Current release status

Updated2026-09-13. Live build **`0.1.0+3147775`** resumes the same persistent world
and drives the cube and shared viewer on7393. It includes independent care doses,
all seven plants' authored growth clips, spiretree wind, and actual-intake meal
continuity, plus the stable sail swimming body and vine-covered spire sway;
1115 workspace tests and copied/live cadence measurements are recorded in the
[rollout](7_Research/vine-live-update-2026-09-13.md). Neither hunters nor modified
ambient support nor a new rest policy is live.

| Work | Verified state | Next gate |
| --- | --- | --- |
| Residual animation flicker | [Stable sail body](7_Research/astra-sail-stable-body-2026-09-13.md) and [intact-atlas vine wind](7_Research/vine-wind-integration-2026-09-13.md) are deployed; [crown corner fix](7_Research/fable-growth-corner-review-2026-09-13.md) independently reviewed for bounded integration | Fable integrating the tested crown-owner choice with per-cap opt-in; production/copy-world checks precede deployment. Rest/feed fin twinkle and constrained glasscane sway remain; blanket fin coverage was not selected |
| Local care responses | [108-case screen](7_Research/astra-care-response-results-2026-09-13.md): Feed usually increases local feeding, rain mixed, Clean usually reduces feeding; meal continuity deployed | [Rain temporal/drizzle/restart review](7_Research/fable-rain-response-playback-review-2026-09-13.md) prefers slower v3 without glowcap. [Root native held-frame review](7_Research/root-visual-and-ecology-disposition-2026-09-13.md) keeps it experimental: subtle readability, not a shipped flourish |
| Quiet habits | Corrected [harness](7_Research/quiet-harness-handoff-2026-09-13.md) and whole 9fc29eb ten-minute reduction pass; all twelve no-care seeds show 3–16 genuine two-second post-birth bouts | Recovery occupies only about 0.011–0.056% of living organism-time; paired ecological losses retained, interpretation under review. Original42faaa2 metadata failure stays retained. Off stays default; schema13 not deployed |
| Ambient support | [Complete two-hour results](7_Research/astra-ambient-support-results-2026-09-13.md): all72 arms pass recorded audits;90% no-care loses skimmers in two additional seeds | Unchanged frozen24h collection running; [explicit horizon reducer](7_Research/ambient-explicit-horizon-reducer-2026-09-13.md) plus independent review has19 passing tests and byte-identical two-hour output. Live default unchanged; manual dose separate |
| Rare Lanternjaw lineage | [Size-aware gate seed-1 pilot](7_Research/hunter-size-gate-handoff-2026-09-13.md) produces paid juvenile growth to S=0.8485 but no adult descendants; both candidate lineages die while reference founders survive. All six reference arms reproduce retained artifacts | Wire the adapted mutation-site ledger into the runner and verify a newly pinned seed-1 replay before the remaining seeds; independent implementation review underway. Preserve adverse pilot; no hunter24/72h or live introduction yet |
| Fragile fauna/diversity | [Early-life census](7_Research/fauna-development-followup-2026-09-13.md) locates eight skimmer losses after initial population growth, about71–120 simulated minutes | Isolated ordinary-fauna flow diagnostic now in development; all twelve tick-zero histories need growth/intake/allocation evidence through2h before tuning |
| Later ideas | Persistent plant age/senescence/regrowth, tiny foliage details and possible LCD presentation remain recorded in the [animation roadmap](animation-roadmap.md) | Lower priority; not an authorized hardware migration |

Ship verified visible slices without waiting for unrelated long experiments.
Retain failed seeds and comparisons; tests and total surviving population alone
do not establish ecology or whole-goal completion. Commit scoped work, use local
checkpoint tags, and preserve the running world and care journal. No pushes.

## Historical implementation record — not current deployment status

The dated entries below describe their own checkpoints and evidence. Statements
such as "in progress" or "live" in this section are historical; use the table
above for the current release and remaining gates.

Completed in the planning phase: Git repository initialization, root README and
ignore rules, `lore-v1` authority configuration, a canon constitution and ledger,
the accepted owner brief, and linked proposals for topology, ecology, evolution,
visuals, runtime, inputs, and this delivery plan. Wrysk subsequently approved the
agent working rules; they are installed in root `AGENTS.md` under D-0003.

Status 2026-09-11: M1 code exists and passes its automated checks (surface
crate with oracle-backed tests including E1, renderer, host with preview, shim
sink, and PNG captures; `cube-proto` vendored at shim revision 7a21b5f because
the shim has no published remote). Frames were sent to the running shim daemon
without errors. Pending M1 evidence: physical-cube observation of seam
continuity, the vertex-ownership fixture, and E8 reflection. M2's world,
persistence, telemetry, and ambient presentation are implemented and pass an
independent spec-derived suite (closed-box material to ~1e-11, per-tick energy
audit, exact snapshot replay). E2 ran through four batches and a 24-hour
confirmation (see the `7_Research/e2-*` notes): nutrient limitation, saturating
intake, and above all slow movement (0.3 px/s) were needed before any local
succession appeared; the adopted defaults sustain population cycles with a
patchy, repeatedly recovering producer field for a day in eleven of twelve
seeds, with one extinction from an ordinary trough. E3 (`7_Research/e3-default-2026-09-11.md`): generations are minutes
apart, but one founder lineage per seed survives the first cycle, so effective
population is tiny; M3 must track lineage survival. Still pending for M2's exit: 30 minutes of
real-time viewing and the cube itself. Durability (M5) is untested. M3a is
specified in [m3a-spec.md](m3a-spec.md).

Status 2026-09-12: Wrysk reviewed the current presentation as too uniform and
infestation-like, requested a game-development approach with sprites, rigging
and animation, allowed an engine port, and identified creature/habitat art as
their preferred contribution. The resulting [Godot art workflow](game-art-workflow.md)
is implemented as an authoring project and explicit cube art study. Three
cutout rigs, twelve clips, and three habitat motifs bake into the existing
surface renderer. It is scripted visual evidence, not M3 evolution or a full
runtime port. The proposed next slice is to select readable artwork and connect
its actions/growth to actual world state. M3a's narrow lobe grammar remains
revisable in light of this feedback; the art study does not satisfy E4/E5/E6.
Later the same day the live integration landed: `run --art` draws the M2 world
with the clips driven by real rest/seek/feed/gestation state and motifs by
producer biomass, leaving the default image untouched (see the workflow note).
Still open: mode-dependent turning/noise in the controller, which needs matched
ecological runs before it changes the E2 regime.

Status 2026-09-13: Wrysk rejected turning as the priority and redirected the
work to the ecosystem itself. Overnight the world was stratified into soil,
foliage and canopy bands, given water (rain, flow, pools, evaporation), alien
plants with fruit, tall plants, ground cover, and fauna v2 (genome v2 with
diet/depth/swim/form, four founder kinds, fruit pool, controller v2, sparse
mutation); see the three design notes and commit `a18398c`. The controller's
turn gate landed as part of controller v2. Validation was short runs (two to
six simulated hours) and captures, by Wrysk's instruction; the E2 regime's
long-run properties are unmeasured for this world and M3's experiments must
be rerun against it. Genome and config versions are 2 and 7; older
snapshots are refused and start fresh.

Status 2026-09-12, subsequent animation/care iteration: two presentation passes
add interpolated motion, quieter accents, shared breeze and a lanternstalk growth
pilot; broader authored growth remains open. The same owning runner now mirrors
its frames to the web viewer, with optional feed/rain/litter-cleanup. Schema 8
explicitly migrates schema 7; zero-care continuation matches the old executable's
fixture byte-for-byte. The host adds ownership locking, guarded resume, bounded
journaling and held-boundary crash replay. Final validation: 774 tests passed,
11 ignored, plus actual browser target/button checks on a copied world. The
[rollout record](7_Research/care-browser-rollout-review-2026-09-12.md) records
the live checkpoint recovery and limitations; this is not proof of long-run
ecological balance or complete M5 durability under real power loss. Fable's
Lanternjaw is Wrysk's preferred megafauna art study, not yet a live predator.

Latest runtime checkpoint, 2026-09-13: frozen `0.1.0+a44dc98` is live on the
same shim/web owner, resumed exactly at tick 283643 with care sequence 5 intact.
It adds completed authored growth, top-reed wind, care flourishes, viewer delivery
and schema 9 compensated accounting; 854 tests passed. See the
[rollout evidence](7_Research/presentation-v9-rollout-2026-09-13.md), including
copied-world restart, exact cross-version continuation and matched rollback set.
The in-progress Lanternjaw hunter extension is deliberately not deployed.

Hunter screen, 2026-09-13: the frozen profile-3 comparison completed all twelve
seeds × six arms × two elapsed hours with 72 passing numerical audits. Hunting
produced 479 captures, but all introduced hunters starved and none reproduced;
no self-replacing lineage is established. See the
[complete screen](7_Research/hunter-profile3-two-hour-results-2026-09-13.md).
Exact reproduction observation and end-of-step stock diagnostics now exist,
with the adversarial observer findings independently closed. The frozen paired
[reserve-target experiment](7_Research/hunter-reserve-targets-recipe-2026-09-13.md)
is in progress, changing only the seek/perch reserve targets. Fable's real-world
Lanternjaw adapter and continuous bounded minification are committed; the reviewed
runner-event drainage, repeated-observation/reset, profile-capability and exact
endpoint-heading issues are corrected. Retained prey now crosses face seams with
transported heading, while capture-tick path and restart reconstruction remain
explicit approximations. None changes the frozen live checkpoint above. Paid
adult descendants, longer matched runs, diversity and physical contact/readability
remain requirements to verify, not achievements inferred from passing tests.

Next care/art packages, 2026-09-13: the
[adjustable-dose handoff](7_Research/adjustable-care-dose-handoff-2026-09-13.md)
has a checked-in, tested viewer selector with legacy capability fallback; core
dose persistence and journal/snapshot migration are in progress, not accepted or
deployed. Ambient support remains separate: the
[natural-rainfall proposal](7_Research/astra-ambient-support-experiment-proposal-2026-09-13.md)
keeps today's default unchanged and requires paired evidence. Fable is authoring
the two remaining canopy species' opening clips. Keep local biological care
responses, quiet habits, stronger tall-plant wind, the AA comparison, and
lower-priority persistent plant age/LCD details open rather than treating these
packages as completion of the full iteration goal.

Latest live update, 2026-09-13: **`0.1.0+9598044`** now runs on the shared cube/web
owner, exactly resumed at tick 457013. Adjustable care doses and all seven plants'
authored growth clips are deployed; 1057 workspace tests passed, with independent
care and canopy checks plus copied-world browser/shower-restart evidence. See the
[update record](7_Research/care-dose-live-update-2026-09-13.md). No hunter or changed
ambient setting is live. Both reserve-target cohorts completed: more acquisition
and longer average survival, but zero funded gestations and all founders starved
([full result](7_Research/astra-hunter-reserve-complete-results-2026-09-13.md)).
Paid energy storage is the next bottleneck to investigate separately from strike
geometry. Tall-plant wind work continues; biological care responses, quiet habits,
AA comparison, autonomous-support tuning, lineage viability/diversity and the
lower-priority plant-history/LCD ideas remain open. Use lightweight Git tags for
code checkpoints and regularly ship verified visible improvements, per Wrysk's
updated working preference; do not hold ready packages for unrelated backlog work.

## M1 — prove the surface and presentation path

Create the minimal Rust workspace, pin/reuse `cube-proto` from the existing shim,
and add continuous surface travel, pure rim reflection, field adjacency, and local
unfolding queries. A local path override is useful during development, but the
committed build must not depend on one person's home directory; select a
reproducible dependency revision when creating the manifest.

Produce identical five-face frames for a cube/net preview and the existing
client. Display an asymmetric moving body with a long trail, a footprint spanning
a top vertex, and a scalar patch diffusing over several faces. These are geometry
fixtures, not the promised ecosystem.

**Exit evidence:** the full [topology checks](surface-topology.md) pass;
independent 3D-unfolding checks and E1 occupancy/equivariance checks pass;
field mass/constant-state invariants
hold; the same motion looks continuous in preview and on the physical cube.
Verify actual shim face mapping with its existing diagnostics if needed. The
physical check remains pending if the cube is unavailable; do not claim it from
a desktop preview.

Include shortest-path ownership of body pixels at top vertices and a step that
reflects off the lower rim while crossing a side seam. E8 is a ten-minute
physical observation of pure reflection with no rim sensor. The shim's physical
Top correction remains its layout concern; do not compensate for it in Cubarium.

**Deliverable:** a reusable surface substrate and a real output path. Failure
here blocks ecological features that would otherwise hide geometric defects.

## M2 — a persistent feeding world

The [M2 world specification](m2-world-spec.md) gives the concrete units,
conversion table, tick order, and persistence format for this milestone.

Implement material/energy accounting, one producer pool `P0`, grazing and
scavenging from `D`, recycling, a gentle height gradient and horizontal habitat
patches, paid movement, maintenance, budding, and death. Start with one fixed
grazer/scavenger genotype. Establish the recycling loop under static conditions,
then add slow light/moisture forcing and compare the same seeds. No photosynthetic
organisms, mutation, predation, dormancy, or rescue are enabled yet.

Add state serialization, atomic checkpoints, resume, bounded queues, and a
headless observer now. Use one 20 Hz simulation cadence and chord-filtered
all-pairs queries before adding scheduling or spatial-index optimizations.
Tune fixed productivity/costs toward a tentative 100–200 cube-wide population,
while measuring actual occupancy and keeping the 512 safety cap inactive.

**Exit evidence:** E2 distinguishes local succession from synchronized whole-world
depletion; E3 measures reproductive intervals and lineage depth. A population
feeds, scavenges, and reproduces through several patch depletions. Material/energy
and shared-transfer tests pass, snapshot/replay matches, and disconnected output
does not halt the world. Watch at least 30 real-time minutes and run accelerated
hours over multiple seeds. Repeated cap-to-extinction cycles halt progression
until habitat/cost changes are tested; reseeding cannot make this milestone pass.

**Deliverable:** a simple living installation that can resume its history.

## M3a — test the inheritance and visual representations

Create a versioned reflex-parameter genome with two leaky memories, small local
mutations, and lineage records. Keep the body grammar to lobe count, length/aspect,
and one head/tail appendage, plus hue accent and gait. Perform E5 on the candidate
controller and E6 on twelve native-size cube samples before expanding either.
A dense recurrent network is a deferred comparison, not required scaffolding.

Separately, run E4
with only size, metabolism, sensing, and reserve genes mutable. This diagnostic
uses simple bodies and fixed behavior, no predation or recovery. It can be built
on M2 before the full grammar. If every tested habitat pushes all these traits
to their lower bounds, revise the costs/opportunities before proceeding.

**Exit evidence:** E1/E2/E4/E5 reports exist with reproducible seeds and limitations;
E6 identifies shapes/gaits that actually read on the cube. Legal genomes are not
declared viable without feeding and budding. Size variation under grazing alone
is tested explicitly; enduring stable polymorphism in every seed is not promised
or forced by quotas. Any failed tradeoff requires a recorded design response.

## M3b — visibly inherited habits

Enable the tested body and behavioral mutations in a small diverse founder
population. Preserve paid local budding and real energetic failure. Produce
matched ancestor/descendant captures and frozen/mutable controls; quantify gait,
pause, turning, patch preference, and shape, rather than relying on hue drift.

**Exit evidence:** independent lineages have reproducing descendants with
observable inherited differences, without a viability filter replacing mutants.
Measured generation/lineage times support the claimed time scale. If reflex
variation proves too narrow, name the missing behavior and test a bounded
controller extension before adopting it.

## M3c — facultative predation

Add paid flesh digestion, attack effort, and escape response as allocations
available to every genotype. First test a fixed hunter in the stable grazer
world (E7), then permit mutation and mixed diets. Defense starts with measured
structural/contact costs; extra armor organs remain optional.

**Exit evidence:** predation conserves resources and does not erase the active
ecology in every seed. Document specialist extinction times and mixed-diet
descendants. Episodic hunters are acceptable; do not seed a protected predator
guild or require permanent specialist survival.

**M3 deliverable:** a small evolving food web with readable inherited habits.

## M4 — ecological memory and optional extensions

M4a tests paid dormant propagules independently from one explicit extinction
reseed policy (E9). Historical genomes are observer-only; remove all archive
recruitment and low-population rescue. Log every external cohort and its costs.

M4b considers optional experiments **one at a time**, each with a prediction,
matched disabled control, and a keep/remove result:

| Addition | Prediction to test before keeping it |
| --- | --- |
| Second producer pool `P1` | Distinct suitability/recovery supports different diets after total productivity is matched |
| Waste `W` | Local metabolic history changes patch abandonment beyond what depletion/detritus already explain |
| One costly signal channel | Cue-driven aggregation or following has a measurable resource/behavioral consequence |
| Organism photosynthesis | Paid edible fans add a niche without replacing mobile activity with a static carpet |
| Shared patch improvement | Paid environmental changes benefit nearby organisms; no abstract cooperation reward |

These additions are not a checklist for completion. Waste must precede waste
clearance, and a useful cue/interaction must precede extra signal channels.
Keep disabled mechanisms absent from the starting controller and visual grammar.

M4c evaluates the selected combination: 12 seeds for 24 simulated hours, then
at least three representative seeds for seven simulated days, including weak
runs rather than only attractive successes. Compare static weather, frozen
genomes, no predation, and the dormancy/reseed controls on selected seeds. Report
local occupation, trait/diet distributions, lineage history, cap pressure,
synchrony, extinction, and reset counts alongside real-time clips.

**Exit evidence:** multiple periods of viable local occupation and turnover,
observable strategy differences, and continuity usually supplied by ordinary
processes. Temporary dominance and extinction are allowed. Weather-only variety,
repeated exogenous resets, or visual inactivity across most runs require another
design iteration. Expensive optional mechanisms with no demonstrated benefit
are removed rather than accumulated.

**Deliverable:** a measured ecological design ready for an unattended soak.

## M5 — perturbations and unattended installation

Implement the generalized stimulus envelope and scripted local adapter. Test
global light changes and localized events across seams with bounded rates,
expiry, budget accounting, and recorded replay. Add a user service and operating
configuration for the actual host using the existing shim connection.

Run a 72-hour real-time soak on that host, followed by a week of ordinary
installation use. Exercise restart, forced process termination, corrupt newest
snapshot, full/slow disk, absent shim, sensor flooding/dropout, sleep/resume, and
capacity saturation. Do not deliberately disrupt unrelated user services.

**Exit evidence:** bounded memory/disk/queue growth; no nonfinite world state;
frame and step p99 within the provisional budgets or an explained revised target;
restart returns to a valid recent history; unattended recovery is explicit in
logs; actual-room viewing confirms an ambient rhythm and readable creatures.

**Deliverable:** the first supported persistent installation. Audio/camera
adapters and additional biology follow observed needs, not a prerequisite list.

## Verification strategy

| Layer | Checks | Why it matters |
| --- | --- | --- |
| Geometry | Exhaustive seams, randomized transport, exact-corner fixtures, independent distance reference | Errors here infect every system |
| Accounting | Closed-box mass tolerance, nonnegative pools, no-light energy depletion, simultaneous feeding/attack | Prevents immortal exploiters and accidental resource creation |
| Evolution | Decode bounds, paid birth, sparse mutation, E3–E6, ancestor/descendant comparison | Separates inherited change from cosmetics and claims based on parameter count |
| Determinism/storage | Same-build tick hashes, save/reload continuation, corrupt/old schema handling, injected I/O failures | A persistent world needs trustworthy continuity |
| Runtime | Dense-corner contacts, cap rejection, dt stalls, output backpressure, bounded histories | Prevents slow degradation over days |
| Inputs | Invalid event rejection, source timeout, bounded energy/material, seam footprint equality, replay | Sensors cannot overwhelm or bypass ecology |
| Experience | Real-time clips and cube viewing in office/party lighting | An analytically varied world may still look dull or noisy |

Set numeric mass tolerances from the chosen representation and verify accumulated
drift over long tests, rather than selecting a permissive error after failure.
Capture seeds, build/config hashes, durations, and mechanism toggles alongside
every evaluation report. Store selected results in `design/7_Research/` as
evidence, not automatically as canon. Runtime metrics never feed an evolutionary
fitness function or appear in the normal display.

## Risk register and response

| Risk | First evidence to seek | Preferred response |
| --- | --- | --- |
| Minimal grazer erases body/sensing variety | E4 lower-bound occupancy and lineage distribution across habitats | Revise mouth/reserve costs and patch opportunities before predation |
| Sessile autotroph carpets the world | Matched photosynthesis-enabled trial, edible area, mobility and grazer survival | Keep photosynthesis absent until its niche is demonstrated |
| Synchronous boom-bust wipes out the food web | E2 spatial lag/correlation, generation/depletion intervals, cap contact | Reduce coupling and test gradients; no live density regulator |
| Predators cannot persist | E7 capture success, extinction time, offspring reserve | Allow facultative diets and episodic hunting; no protected guild |
| Controller mutations erase viability | E5 matched founder/mutant trials and visible habit measures | Sparse named-drive mutation before a network extension |
| Everything becomes a flashing dot | Native-size clips and real-cube observation | Strengthen silhouettes/gaits and lower effect intensity |
| Corners create duplicate food or bites | Seam/corner conservation and deduplicated neighbor tests | Fix shared topology/transaction layer before tuning ecology |
| Cap becomes the main selection pressure | Time at cap, rejected births, crowded-query fallback count | Reduce ecological productivity or cost footprint with evidence |
| Recovery hides stagnation | E9 separates dormancy and reseeding; reset frequency and no-assistance controls | Repair feedback; observer archive never recruits |
| Simulation survives but feels repetitive | Hours-apart footage and inherited behavior samples | Add one missing interaction, not undirected visual noise |

## Next implementation action

Begin M1 by confirming the shim dependency revision and creating a minimal
surface crate with continuous transport, pure rim reflection, the 3D oracle,
and seam/vertex fixtures. Then execute E1 and E8 before M2. The preferred
five-face/no-flux-rim policy, Rust runtime, and initial numerical budgets are
ready to review but remain `leaning`. Finalize an accepted architecture entry
only when specifically authorized; ordinary implementation can still proceed
under explicit task authorization without pretending the whole plan is canon.

Later questions to settle with evidence: host performance headroom, real viewing
distance/brightness, named-drive parameter packing, useful genotype diversity
measures, dependency distribution, and whether the extinction fallback is needed
often enough to reveal a design failure. None prevents this planning revision.
