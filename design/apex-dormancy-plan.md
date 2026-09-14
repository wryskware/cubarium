---
design_status: exploration
last_reviewed: 2026-09-14
---

# Apex dormancy and encounters — implementation scope

Wrysk authorized implementation on 2026-09-14 after discussing the lifecycle
below. These are working implementation choices, not additions to the canon
ledger. New visual design is deferred to Fable.

## Behavior

Paid apex offspring enter a concealed, underground dormant state. They retain
their organism identity, lineage, material, and energy. Dormant offspring do
not move, hunt, reproduce, participate in surface encounters, or develop as
active juveniles. Their reserves pay a small maintenance cost; exhaustion can
kill them. They occupy real organism capacity. Burial is a state at a cube
surface location, not a new underground terrain simulation.

Sustained nearby abundance of suitable juvenile prey and sufficient emergence
reserves permit waking. Deterministic differences between offspring stagger
readiness; recheck local conditions when waking. Emergence resumes juvenile
development, not instant adulthood. Multiple active predators are supported.

Adult predator encounters can lead to mating or combat. Mutually ready, fed
adults contribute resources and inherited traits to offspring, then enter a
cooldown. Otherwise existing attack, injury, and escape mechanics should carry
combat where feasible. Dormant offspring are excluded. Avoid repeated pair
processing and interaction at arbitrary distance. The first paired policy
requires two adults; a lone adult has no automatic reproductive rescue.

For paired gestation, both parents pay into one carrier's escrow. Capacity
rejection restores each living contributor's inventory share; if the partner
has died, its prepaid share stays conserved with the carrier. Spent build
energy is not refunded. Partner death does not erase an already-funded embryo;
carrier death settles its whole escrow through existing death accounting.

The intended ecological hypothesis is that offspring wait through depletion,
then return when prey recover. Neither recovery nor lineage persistence is
guaranteed. Existing results include paid births without adult descendants;
successful juvenile feeding and maturation remain necessary.

## Delivery boundaries

1. Dormancy: implement opt-in lifecycle, concealment, paid maintenance, local
   emergence, persistence, and focused lifecycle/accounting tests.
2. Encounters: add paid paired inheritance/reproduction, cooldowns, and
   consequential combat, using the dormancy seam from milestone 1.
3. Validate: run focused tests and one small comparison with the existing
   hunter policy. Report emergence, maturation, reproduction, and prey response.
   No automatic parameter/seed/horizon sweep if the screen fails.

Use fresh workhorse contexts with concise delta reports. One targeted review
after integration, at most two repair cycles. Do not commission review of
reviews or read historical agent transcripts. Preserve unrelated work. No new
art, captures, or animations. The screen informs tuning, not access to features.

## Progress

- Dormancy implementation complete: workspace check,
  four dormancy tests, twelve snapshot tests, and existing reproduction and
  hunter/quiet migration tests pass. New behavior remains opt-in.
- Encounter implementation complete: eight encounter
  tests and the focused regression suites pass; workspace compilation passes.
  Enable with `world.state.apex_encounters = ApexEncounterState::paired_v1()`.
- Enable dormancy separately with
  `world.state.apex_dormancy = ApexDormancyState::underground_v1()`.
- Both additions share the unreleased schema-14 snapshot extension. Older saves
  migrate with both policies Off; live spawn controls enable both automatically.
- One focused integrated review found no substantive defects.
- `cargo test --workspace --all-targets` passed with localhost binding permitted.
  The full run found one stale schema-13 assertion, corrected to schema 14;
  the repaired full run passed, including twelve new apex tests.
- The single staged mechanism screen (seed 37, 12,000 ticks per arm) completed:
  baseline had two births; the combined policy had one mating, one birth, one
  emergence, and eight combats. Neither arm produced mature offspring, and
  both ended with zero prey. No next-generation emergence occurred. This
  exercises mechanics but does not establish viable recurring ecology.
  [Retained result](7_Research/apex-screen-2026-09-14.json).
  Both arms began with twelve stationary juvenile prey and two nearby fully
  stocked mature predators, with no post-opening forcing or restocking.
  Reproduce with `cargo run -p cubarium-core --example apex_screen`.
- Stop balance work at this checkpoint. The next bounded ecology question is
  whether emerged juveniles can feed and mature while enough prey survive to
  recover; changing seeds or thresholds repeatedly is not part of this pass.

## Live spawn controls — Wrysk's follow-up

Wrysk explicitly wants the implemented lifecycle available to try, regardless
of incomplete ecological balance. The viewer now offers **Spawn 1 apex** and
**Spawn 2 apex** in its existing control panel. They add active mature predators
at random positions across the five faces and enable dormancy and encounters
automatically. Subsequent clicks add predators without resetting earlier
founders or descendants. Pair admission is atomic at the organism cap.

The existing `/care` command queue journals the count and resolved placements
for exact replay. Founder material and energy are explicit external inputs;
spawning does not masquerade as reproduction. The normal launcher already
enables this control lane. No automatic founder placement is required: users
can introduce predators when they want to perturb the world.

The earlier stationary-prey screen remains limited mechanism evidence. The next
search should evaluate the complete evolving ecology together, as scoped in the
[Claude Code handoff](handoffs/ecology-search-2026-09-14.md).

Control validation: repeated founder/accounting/capacity tests, command and
journal tests, existing care-service tests, and the viewer declaration test pass.
Opus's one targeted review found a release-only sequence-admission bug and a
repeat-spawn lifecycle reset; both were fixed before deployment. A release-mode
spawn/spawn/feed replay test passes, as does a real gestation-to-buried-offspring
test that introduces more founders at both lifecycle stages.

The two completed Opus CLI calls reported 68 input, 93,976 cache-creation input,
987,404 cache-read input, and 20,407 output tokens. This includes the restricted
repair attempt that made no edits; the earlier canceled ecology run and the
Sol worker did not expose measured usage. No agent transcripts were reviewed.
