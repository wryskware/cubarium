---
design_status: leaning
last_reviewed: 2026-09-11
decision_refs: []
---

# A living skin for the cube

The preferred direction is a small, resource-limited world with memory. Its
inhabitants inherit bodies and behavior, consume and redistribute material,
change the patches they occupy, and leave both descendants and dormant spores.
Their success changes the conditions that made them successful.

This is a design proposal. The [brief](brief.md) is accepted; the mechanisms
below are not yet implemented or validated. Read the
[canon rules](0_Canon/README.md) and [ledger](0_Canon/DECISIONS.md) for authority.

## What a glance might reveal

A dim mottled substrate, a few creatures with distinct movement, and a local
feeding patch should read immediately. A crawler follows an old scent across
Front onto Right; its head turns with the cube's fold while its tail still
occupies Front. A slow frilled colony grows near a bright patch. Small grazers
congregate there, deplete it, and disperse. A hunter follows the gathering, then
loses it as food and prey move elsewhere. Later, descendants have smaller fans,
longer pauses, or a different response to the patch's accumulating waste.

These are **scenarios to make possible**, not timelines to script. A world need
not exhibit every behavior on command. Quiet has value, and composition should
come from local ecology and restrained rendering rather than a hidden director
placing organisms for a desired picture.

## Preferred choices

| Area | Direction | Main uncertainty |
| --- | --- | --- |
| Surface | Five charts of one cube surface; reuse shim seams; closed-to-flux lower rim | Whether rim behavior looks natural |
| Environment | Continuous substrate, two producer pools, nutrients, detritus, waste, moving light and moisture | Whether niche differences persist long enough to matter |
| Evolution | Bounded body grammar plus a small inherited recurrent controller; local birth and death | Whether behavioral changes remain legible and viable |
| Diversity | Tradeoffs, depletion, predator feedback, habitat modification, slow forcing, dormancy | Whether these suffice without frequent recovery interventions |
| Persistence | Fixed-step deterministic core, bounded memory, versioned snapshots and normalized input logs | Actual cost and recovery behavior on the host |
| Presentation | CPU pixel renderer, restrained palette, partial bodies and trails carried across seams | What reads at real brightness and viewing distance |
| Integration | Rust and the existing `cube-proto` client/geometry; preview from identical face buffers | Dependency pin and host performance, confirmed during implementation |

The five-face surface has a real lower boundary. Prefer a gently repelling,
reflecting rim with no material flux, rather than an invisible sixth habitat or
an arbitrary teleport. This is a revisable assumption, not a new requirement.

## Read by purpose

- [Surface topology](surface-topology.md): coordinates, seams, distance, corners,
  diffusion, body rendering, and topology acceptance checks.
- [Ecology](ecology.md): material/energy bookkeeping, local interactions,
  environmental time scales, ecological memory, and recovery.
- [Evolution](evolution.md): genome, controller, birth, mutation, tradeoffs,
  visible phenotypes, and the limits of this model.
- [Appearance](appearance.md): a low-resolution visual language with a calm
  default rhythm and a plan for judging it on the real cube.
- [Architecture](architecture.md): runtime boundaries, scheduling, storage,
  replay, capacity, performance, and the shim adapter.
- [Environmental inputs](environmental-inputs.md): a bounded stimulus contract
  that future sensor systems can feed.
- [Implementation plan](implementation-plan.md): vertical slices, verification,
  risks, and concrete next work.
- [Local contract evidence](7_Research/local-contracts.md): what exists in Lore
  and the shim, checked on disk.
- [Agent instructions approval](agent-instructions-proposal.md): the approved
  installation record for the root [working rules](../AGENTS.md).

## Deliberate limits

There is no promise of unbounded open-ended evolution on a finite machine with
a bounded genotype. The aim is sustained, observable variety and continuity,
tested over progressively longer runs. Fixed resource types and a small body
grammar limit the possible worlds; they also make inheritance understandable at
64 pixels. Expand them only when observation identifies a missing possibility.

Do not begin with an unrestricted neural architecture, language-model creature
brains, arbitrary executable genomes, whole-cube physics, or a giant list of
special-cased species. First prove that a few real feedback loops create an
interesting place. The plan includes controls that can disprove that hypothesis.
