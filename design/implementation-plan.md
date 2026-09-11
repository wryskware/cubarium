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

## Current deliverable — repository and vault

Completed in the planning phase: Git repository initialization, root README and
ignore rules, `lore-v1` authority configuration, a canon constitution and ledger,
the accepted owner brief, and linked proposals for topology, ecology, evolution,
visuals, runtime, inputs, and this delivery plan. Agent working rules are a
reviewable proposal; they are not installed without agreement.

There is no application, manifest, dependency lock, simulation test suite,
service, or live display session yet. Planning verification checks source
contracts, local links, authority metadata, and Lore indexing. Ecological,
performance, visual, and durability results remain to be produced.

## M1 — prove the surface and presentation path

Create the minimal Rust workspace, pin/reuse `cube-proto` from the existing shim,
and add continuous surface travel, rim policy, field adjacency, and local
unfolding queries. A local path override is useful during development, but the
committed build must not depend on one person's home directory; select a
reproducible dependency revision when creating the manifest.

Produce identical five-face frames for a cube/net preview and the existing
client. Display an asymmetric moving body with a long trail, a footprint spanning
a top vertex, and a scalar patch diffusing over several faces. These are geometry
fixtures, not the promised ecosystem.

**Exit evidence:** the full [topology checks](surface-topology.md) pass;
independent distance/reference checks pass; field mass/constant-state invariants
hold; the same motion looks continuous in preview and on the physical cube.
Verify actual shim face mapping with its existing diagnostics if needed. The
physical check remains pending if the cube is unavailable; do not claim it from
a desktop preview.

**Deliverable:** a reusable surface substrate and a real output path. Failure
here blocks ecological features that would otherwise hide geometric defects.

## M2 — a persistent feeding world

Implement material/energy accounting, local producer growth, nutrient/detritus
recycling, light/moisture patches, and one mobile grazer phenotype with paid
movement, maintenance, budding, and death. Add state serialization, atomic
checkpointing, resume, bounded queues, and a headless observer now. Use fixed
founder genomes; mutation comes after the feedback loop is sound.

**Exit evidence:** a population can feed and reproduce through several local
patch depletions; bodies and resources cross seams correctly; paid births and
deaths conserve material; closed-energy tests cannot generate energy; exact
snapshot/replay matches on the supported build; output disconnection does not
halt the world. Watch at least 30 minutes in real time and run accelerated
hours over multiple seeds. A world kept alive only by reseeding does not pass.

**Deliverable:** a simple living installation that can resume its history.

## M3 — visibly inherited differences

Introduce the bounded body grammar, decoded physiology, recurrent controller,
lineage recording, local mutable budding, and a small set of diverse founder
genomes. Build the visual gallery from actual phenotype code. Add the immutable
render views and motion-path interpolation if the first slice used simpler
rendering. Establish versioned genome/sensory channels and mutation bounds.

**Exit evidence:** founder strategies feed and reproduce before mutation is
enabled; ancestor/descendant samples demonstrate inherited changes in shape,
movement, or environmental response. Mutated populations remain viable without
development replacing unsuccessful genomes. Frozen/mutable comparisons reveal
what mutation changes. Long runs contain births in independent lineages and
measurable phenotype differences, not just shifting colors.

**Deliverable:** the first evolutionary world, with readable inherited habits.

## M4 — food webs, succession, and ecological memory

Add the second producer pool, scavenging and predation allocations, defense and
stress, local signals, asynchronous slow forcing, paid dormant propagules, and
bounded historical recovery. Add shared patch improvement only when its physical
cost/benefit can be measured. Introduce one mechanism at a time so its effects
remain attributable.

**Exit evidence:** evaluate 12 fixed seeds for 24 simulated hours; extend at
least three representative seeds to seven simulated days. Record dominance,
collapse, recovery, cap pressure, trait/diet occupancy, lineage survival, and
intervention counts. Review clips at real speed and compare static-weather,
no-mutation, no-predation, and no-recovery controls on selected seeds.

The evaluation need not keep every founder alive or maintain a species quota.
It should demonstrate multiple periods of local occupation and turnover,
observable differences in viable strategies, and recovery that usually comes
from ordinary processes. A lineage dominating temporarily is acceptable. If
almost all change disappears with scripted weather disabled, or most runs need
repeated immigrants, revise the mechanisms before declaring this milestone done.

**Deliverable:** evidence for an evolving ecology rather than a continuously
redecorated movement simulation.

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
| Evolution | Decode bounds, paid birth, mutation inheritance, ancestor/descendant comparison, controls | Separates real inherited change from cosmetics |
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
| One strategy consumes every niche | Resource productivity and diet budgets, static-weather control | Rebalance real costs/productivity; avoid species quotas |
| Predators cannot persist | Capture success, handling cost, offspring reserve | Tune encounter/energy budgets before adding smarter brains |
| Controller mutations erase viability | Founder and offspring survival under matched conditions | Reduce dimensions, mutate gently, expose structured biases |
| Everything becomes a flashing dot | Native-size clips and real-cube observation | Strengthen silhouettes/gaits and lower effect intensity |
| Corners create duplicate food or bites | Seam/corner conservation and deduplicated neighbor tests | Fix shared topology/transaction layer before tuning ecology |
| Cap becomes the main selection pressure | Time at cap, rejected births, crowded-query fallback count | Reduce ecological productivity or cost footprint with evidence |
| Recovery hides stagnation | Assisted versus unassisted survival and intervention cadence | Repair ecological feedback; keep intervention provenance |
| Simulation survives but feels repetitive | Hours-apart footage and inherited behavior samples | Add one missing interaction, not undirected visual noise |

## Next implementation action

Begin M1 by confirming the shim dependency revision and creating a minimal
surface crate with continuous transport and the seam fixtures. The preferred
five-face/no-flux-rim policy, Rust runtime, and initial numerical budgets are
ready to review but remain `leaning`. Finalize an accepted architecture entry
only when specifically authorized; ordinary implementation can still proceed
under explicit task authorization without pretending the whole plan is canon.

Later questions to settle with evidence: host performance headroom, real viewing
distance/brightness, controller channel packing, useful genotype diversity
measures, dependency distribution, and how often historical recovery is actually
needed. None prevents the current repository and planning deliverable.
