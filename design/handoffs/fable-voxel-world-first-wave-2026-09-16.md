---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Fable handoff: a voxel world, then a living game

Wrysk's request: review this direction and potentially start orchestrating the
first wave of material. This handoff collects the planning conversation, including
the engine discussion. It records no new accepted design decision. The current
thread authorized planning, research and design documents, not implementation or
deployment. Prepare a small, dispatchable first wave; distinguish that preparation
from launching an engine migration or a biosphere rewrite.

## Read first

Read [WORKING_POLICY.md](../../WORKING_POLICY.md), especially the **2026-09-16 fast
iteration correction**, and the [canon rules](../0_Canon/README.md) and relevant
[ledger entries](../0_Canon/DECISIONS.md). Then:

1. [Terrain and ecosystem proposal](../terrain-and-ecosystem-proposal-2026-09-16.md):
   spatial model, terrain, hydrology, niches and staged experiments.
2. [Game roadmap](../game-roadmap-2026-09-16.md): player loop, progression,
   terraforming, time modes and possible AI features.
3. [Research foundations](../7_Research/terrain-ecology-foundations-2026-09-16.md):
   primary sources, adaptations and limits. Evidence, not design authority.

Those documents also point to the landscape review, ecology experiments and
earlier training/search work. Follow a reference when needed; do not ingest the
whole archive. They are explorations, including where they sound detailed or
confident. Their larger experiment lists are a menu, not mandatory first-wave
validation. This handoff and the three linked documents need to travel together
if work moves to another checkout.

## What Wrysk actually wants

- **Recognizable places:** varied hills and rocks, forests, flower fields,
  glowcap patches and room between them. End the homogeneous scattering of bits.
- **Ecology that needs its geography:** plants disperse locally and differ in
  soil, light and weather preferences; animals prefer particular food sources.
  Add species, layered food webs and potentially inherited colors for lineages.
- **Water as part of the habitat:** pools, streams, infiltration and groundwater
  or aquifers, with sources beyond rain.
- **Voxel terrain. Ringworld first. Local desktop this cycle.** The ringworld is
  the initial world layout, not an instruction to deploy to the physical panel.
  Reusing the Tachyon renderer is a possibility, not a requirement.
- **Rethink the whole biosphere.** Construct and evaluate whole ecosystem
  candidates, rather than repeatedly refining a creature against a frozen food
  supply. Emergent niches are desirable; deliberately designed niches and
  controlled ecosystem-level evolution are acceptable.
- **An actual game and eventual desk companion product.** The expanded game
  ambition does not change the immediate terrain-and-physics priority.

The cube picture is five connected shallow 3D habitats with depth, viewed through
the side faces like aquariums, with real solid bottoms. Its interior can act as
a mountain or bedrock mass: top pools can feed water downhill toward lower
habitats. Underground voxels need not be visible. Water leaving the sides,
returning to groundwater, or evaporating and recycling are possibilities; the
boundary and reservoir rules are not settled. Cube and ringworld need not use
identical generators.

## Working spatial and physics proposal

Start with a **shallow 3D voxel strip, viewed in 2D**, wrapping horizontally.
Use height and a finite depth with gravity downward. This is our interpretation
of “2D ringworld,” not a settled grid size, camera or API. Horizontal periodicity
does not imply wrapping sky to bedrock: the strip need not be a mathematical
torus. The later cube's display edges likewise need not dictate simulation
boundaries. Reuse the display shim's mapping helpers when hardware work returns.

Voxels describe actual material occupancy, allowing rock ledges and potentially
overhangs. Organisms can have continuous positions. A single ground-height value
per column should not become the authoritative model. Generate broad landforms,
basins, soil pockets and connected drainage before adding small noise. Camera
and depth readability need an early visual check; more depth is useless if it
hides the ecosystem.

The candidate water model separates free water in empty voxel space, soil pore
water and groundwater storage/head. Transfers debit their sources. Gravity,
finite basin storage and spill heights produce pools and streams. A spring needs
a suitable water supply and hydraulic head; an uphill pump requires power and
moves existing water. Start with prescribed rain and evaporation. A finite
atmospheric reservoir and complete recycling can wait.

**The solver is unresolved.** A conservative local fill/flow scheme is a useful
first candidate, but “fall, then equalize neighbors” does not automatically
handle connected vessels or pressure under roofs. Try a tiny basin/spill scene
and a connected passage before promising general voxel fluid behavior. Decide
whether its limits are acceptable or a pressure solve is needed. An engine's
rigid-body physics does not solve this ecological water problem for us.

## Engine choice: recommendation to evaluate, not a selection

Wrysk raised the growing cost of UI, input, saves, Steam integration and editor
tooling. Rust is attractive for an embedded Linux runtime, but that does not
require writing the whole desktop game without an engine.

**Recommendation: evaluate Godot 4 with an independent Rust simulation first;
keep Bevy as the strong alternative if an all-Rust stack matters more.**

| Approach | Why consider it | Cost or uncertainty |
| --- | --- | --- |
| Custom Rust application | Existing renderer knowledge and direct runtime control | We own the UI framework integration, tools, assets and much of the game platform work |
| Godot + Rust core | Established visual editor, UI and scene workflows; Rust through community GDExtension bindings | Native extension packaging and a cross-language boundary; custom terrain tools still required |
| Bevy | Rust throughout, flexible data-oriented application architecture | More tooling to assemble; evolving APIs and editor ecosystem |
| Fyrox | Rust with an existing visual editor | A possible fallback to inspect if the first two do not fit; no need for a four-engine bake-off |

Current references: [Godot features](https://godotengine.org/features/),
[godot-rust bindings](https://godot-rust.github.io/book/),
[Bevy's August 2026 development update](https://bevy.org/news/bevys-sixth-birthday/),
[Bevy introduction](https://bevy.org/learn/quick-start/introduction/), and
[Fyrox](https://fyrox.rs/). Godot documents
[Linux ARM64 exports](https://docs.godotengine.org/en/stable/tutorials/export/exporting_for_linux.html),
so embedded Linux alone does not rule it out; actual board/GPU suitability still
depends on its [system requirements](https://docs.godotengine.org/en/stable/about/system_requirements.html)
and measurement. No board qualification belongs in this first wave.

Keep authoritative terrain, water, ecology and eventual progression in Rust,
usable without a renderer. A frontend sends batched commands and consumes views
or changed chunks. Avoid an engine node or language-boundary call per voxel.
Use engine rendering for a Godot spike rather than first trying to embed the
existing custom Vulkan renderer. Existing simulation/render separation is useful,
but the present surface-grid world is not already a voxel implementation.

Saves should represent simulation data, not require an engine scene tree. The
engine's [save facilities](https://docs.godotengine.org/en/stable/tutorials/io/saving_games.html)
and community [GodotSteam integration](https://store.godotengine.org/asset/godotsteam/godotsteam-gdextension/)
provide pieces, not finished game persistence or cloud conflict handling.
Prove only a simple local save/load path now; old-world migration is unnecessary.
An eventual device runner may use a lighter frontend over the same core. Do not
build and maintain two new renderers before there is a reason.

## Proposed first wave for Fable to shape

Aim for one inspectable **local terrain-and-water toy**, not the complete game.
Keep the planning output to short work slices with owners and concrete outcomes;
do not create a report bureaucracy around them.

| Slice | Small useful outcome | Dependency |
| --- | --- | --- |
| Integration decisions | Choose a provisional grid/depth, camera, core/frontend boundary and one frontend to try; identify actual code reuse | First; use Graft/Lore and verify the current sources |
| Terrain and water | Small periodic voxel strip with a ridge, basin, soil/rock variation and a controllable water supply; a short headless run shows filling and spill behavior | Agreed shared data/command boundary |
| Desktop frontend trial | Display the same world, select a cell, inspect it, make a simple terrain edit, pause/advance and save/reload | Can develop alongside physics once the boundary is agreed |
| Ecology sketch | A compact candidate community linking habitat, producers, diets, decomposition and reproduction; identify fields terrain must expose | Planning can proceed alongside the prototype; no training campaign |

Prefer one person owning shared interfaces rather than parallel incompatible
frameworks. The frontend trial should answer whether rendering, picking, UI and
Rust integration feel practical. If it fails, use what it taught us to try Bevy;
do not spend weeks polishing a throwaway engine comparison. The existing desktop
renderer is also a useful fallback for observing physics while frontend choice
is investigated.

A useful stopping point: launch locally, recognize a ridge and wet hollow, add
water, watch it collect and overflow, inspect/edit terrain, and speed up or pause
the same simulation. Check the periodic seam and water transfers with tiny
fixtures. State any solver limitation plainly. A provisional source/sink is fine
if its water accounting is explicit. A powered pump is a later extension, not a
dependency for demonstrating water. Full caves, erosion, seasonal weather,
plant communities, Steam and hardware output are not first-wave prerequisites.

Follow the latest working policy: tests exercise the smallest useful behavior;
no long simulation tests, pinned world hashes, migration/provenance suites,
mandatory independent review rounds or package evidence reports. Longer studies
are explicit, bounded, ignored-by-default work. Run checks only for touched
crates; documentation does not need a workspace test run. The commit message and
a short run instruction can be the implementation report. Keep generated data
small, use the normal build cache, and do not create archival builds/captures.

## Roadmap context to preserve

**Ecology:** terrain should eventually support distinct producer species with
local establishment and dispersal, resource/light competition, dietary
specialists, omnivores, predators and decomposers. Glowing fungi still need an
energy/substrate source. Start with a small coherent community, then add roles.
Avoid using visual variety or founder survival as evidence of a functioning web.
Whole-ecosystem search should vary compatible environmental and biological
parameters together, preserve diverse viable candidates, and measure throughput
before selecting a compute budget. Reuse or rewrite `cubarium-search` as useful.
Offline ecosystem selection, inherited traits and within-lifetime learning are
different mechanisms; none implies the others already work.

**Game loop:** a possibly barren starting world and budget; terraform and
introduce life; observe successful reproduction; earn genome points; spend on
biology or world upgrades; establish a new habitat. Wrysk suggested reproduction
rewards. Rewarding maturation or lasting descendant success to discourage birth
farming is our proposal, not an accepted mechanic. Currency structure, unlock
costs, failure recovery and how genome edits affect descendants remain open.

**Tools and science:** a player terrain builder could be a separate application,
sharing the runner's world format and physics. Groundwater pumps creating
hilltop springs, solar panels and other science-fiction terraforming devices
can populate a science tree. A developer engine editor does not automatically
provide the player-facing voxel editor. Leave room for devices without building
the entire technology tree now.

**Time and presentation:** support accelerated and real-time simulation later,
with optional synchronization to the player's local day/night. Keep simulation
time, environmental light and displayed brightness separable: the user should
be able to keep the view bright while the ecological cycle continues. Offline
catch-up and time-mode transitions still need design. Normal ambient views stay
uncluttered; interactive game inspectors and explicit development views are
appropriate places for explanation and diagnostics.

**Product and AI:** active habitat-building plus a persistent desk companion is
the product hypothesis. Neural creatures, inherited variation and ecosystem
search are substantive AI directions. An optional naturalist or terrain-design
assistant could come later. Neither market demand nor open-ended autonomous
evolution has been demonstrated. Keep the simulation useful without a required
cloud model. Hardware versions follow a compelling local experience.

## What the review should resolve

Check the proposed shallow-depth ring interpretation and camera, pick a
provisional frontend exercise, and reduce the water model to something we can
observe quickly without baking in impossible pressure or groundwater behavior.
Surface only the consequential open choices. Wrysk has already specified voxels,
ringworld-first, local desktop development and whole-ecosystem redesign; do not
reopen those as permission questions. Treat exact APIs, dimensions, solver and
engine as provisional choices to test. Keep the first wave focused enough that
its output informs the next ecological design instead of postponing it.
