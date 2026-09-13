---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Opt-in paid Lanternjaw ecology: core implementation work order

Wrysk requested continued implementation of rare apex predators, selected Fable's
Lanternjaw, and authorized delegation. This package implements an experimental
fixed lineage with real costs, local capture, digestion and one funded offspring.
It does NOT introduce it into the live world or assert that trial defaults are
balanced. Zero-hunter behavior must preserve the approved autonomous ecosystem.

## Ownership and coordination

Native Opus5 high owns `crates/cubarium-core/**` and its own new implementation
report. Root owns host orchestration, paired experiment harness and live processes.
Fable is actively editing host art/presenter/loader, render multipart modules,
Lanternjaw, art sources/assets/tests; DO NOT edit those or their new files. Root's
care_compare.rs and runner.rs are also outside this worker's ownership.
Do not touch state/, live care, .vscode/, the shim or actual output processes.

Read AGENTS, canon README/ledger, current README, and the COMPLETE
`astra-fixed-hunter-implementation-plan-2026-09-13.md`. That is the detailed
working recipe, not accepted ecological tuning. Read current source and geometry
contract. Respect the following updates to that plan:

- Accounting landed as `b47eacc`, reviewed `1d7b386`. CURRENT schema is9, so this
  package adds hunters in **schema10**, preserving the whole schema9 prefix,
  including signed energy corrections. Retain explicit9/8/7 decode/projections.
- Root is creating genuine pre-hunter schema9 continuation fixtures before handing
  you the tree. They must match byte-for-byte under the empty hunter extension,
  including corrections. Existing genuine7/8 continuation tests must also pass.
- Care flourish and corrected harness are committed (`0324098`, `57408c4`). Frozen
  twelve-hour audits run independently; do not mutate their executables/artifacts.
- Long-run prey margins are NOT validated: twelve-hour comparisons retain total
  population but lose skimmers and founder diversity. Implement/run isolated tests;
  no predation defaults or live introduction on the strength of unit tests.
- To avoid concurrent host literal changes, leave `RenderView` and `OrganismView`
  struct fields unchanged in this slice. Expose `World::hunter_view()` separately,
  keyed by full IDs, containing actual phase/progress, jaw target/anchor, size,
  gut and gestation. Root/Fable will integrate it after the renderer package lands.
  Never select predator membership merely from genome.form.
- Use an appended `DeathCause::Predation` and separate hunter death/capture stats;
  preserve the three existing natural-death counters and wire prefix. Keep one
  ordinary `LifeEvent::Death` per consumed prey, plus separate `HunterEvent` records
  if useful. Do not add LifeEvent enum variants that force concurrent host changes.

## Required implementation, not a skeleton

### New geometry and experiment evidence (read before core sign-off)

Astra has now measured the actual selected rig in
`lanternjaw-ecology-animation-contract-2026-09-13.md` (`2c8e90d`). The6px placeholder
does NOT align with the body: ingestion jaw is8.5–9.6px forward and the extended
near capture claw is approximately(13.2794,1.1624). Distinguish ingestion and
capture; coordinate sense range, stopping distance, phase-end settlement and
whole-body juvenile scale rather than merely moving an unconfirmed point.
Root-owned contact charts must match the rig at vertices and clip at the rim;
independently traveling/reflected appendage anchors can create invisible captures.
If this evidence arrives after the first core profile is authored, label that
profile/contact geometry unintegrated and expose the necessary parameters/data
for the correction. Do not claim visual contact from six-pixel unit fixtures.

`hunter-experiment-contract-2026-09-13.md` (`5ad6bbc`) defines the paired measurement:
six arms, not one diet-confounded attacks-off control; all twelve seeds at2h,
fixed placements, strict PRE-import inventories, capture-position-based recovery
and right censoring. Root is preparing the exact cohort. These are candidate
experimental parameters, not new canonical tuning decisions.

Implement the detailed plan's explicit initialization, persisted finite validated
profile/member extension, dedicated new RNG stream, local phase machine and paid
escape/strike, common post-movement contact and single-claim resolution, carried
M/Q gut, energy-honest digestion, hunter gut/escrow death cleanup, slow maturation
and costly single offspring. Retain the user's selected art direction through a
separate semantic role; no hidden free food, automatic reseeding, invulnerability,
global prey sensing, population-based reproduction gate or hard two-hunter quota.

Preserve every unchanged raw light/heat operation and its correction in the
no-hunter path. All new heat uses compensated accounting. Adult structure has
ZERO chemical energy; reserve/usable energy/escrow have the plan's explicit
identities. Include gut and founder sources in stored totals, invariants and both
per-tick/cumulative audits. Charge all attempts, including losing contenders.
Do not silently replace difficult core behavior with decorative scripted hunts.

Candidate public API (publish the concrete API early in your report):

```text
World::start_hunter_trial(profile, target) -> Result<HunterFounderReceipt>
World::hunter_view() -> Vec<HunterView>
World::drain_hunter_events() -> Vec<HunterEvent>
```

Profile should retain `attacks_enabled` and a bounded detritus scavenging fraction
so the same initial living hunter can be tested with attacks disabled, as a
specialist, or facultatively scavenging. No producer/fruit grazing for hunters.
Also provide a narrowly scoped budget-control initializer if practical: identical
derived founder M/Q booked once, but placed as local D/De (cap excess to real heat),
no hunter. This enables paired experimental controls without raw host mutations.
All initializers validate before mutation, reject repeats and return actual booked
amounts/full founder ID. Never reset a world's opening history to hide an import.

For jaw geometry, expose profile jaw offset/reach and actual transported mouth.
The art worker must confirm contact visually before live integration; a six-pixel
forward offset and1.5px reach are trial placeholders, not assumed art agreement.

## Verification and delivery

Implement deterministic tests specified in the detailed plan: exact founder
inventory; energy-poor prey; partial digestion and headroom; full gut refusal;
gut/escrow death; no free repeated capture; stale slot/contested claims; paid
failed attacks and escaping prey; seam/vertex/rim contact; escrow/cap/miscarriage;
one funded descendant; persisted restart at each hunt/handling/gestation phase;
genuine schema9/8/7 empty-extension continuation. Explicitly test invalid profiles,
malformed snapshots and no-hunter inertness. Retain current raw-prefix fixtures.

Run full core tests and workspace compile (coordinate any exhaustive-match errors
outside core with root, do not take over other agents' files). Use apply_patch for
authored edits; do not run workspace-wide formatting. Record exact commands,
tests, API, trial parameters, defects, limitations and remaining ecological gates
in `fixed-hunter-core-progress-2026-09-13.md`. Commit validated core/report paths
under `flock /tmp/cubarium-shared-care/git.lock`; NEVER git add . or amend another
agent's commit. No push. This is actual implementation; keep going until the
bounded core package is delivered or a concrete missing authority blocks it.
