---
design_status: decided
last_reviewed: 2026-09-11
decision_refs:
  - D-0001
  - D-0002
  - D-0003
  - D-0004
  - D-0005
---

# Cubarium decision ledger

Append-only. Newest entries at the bottom. Schema per [README](README.md).
Only decisions already authorized in the owner's request are accepted below.

## D-0001 — Adopt the lore-v1 design vault

- **Date:** 2026-09-11
- **Status:** Accepted
- **Scope:** Design documentation and planning in this repository
- **Decided by:** Wrysk, explicitly requesting repository initialization and plans in Lore's design vault format on 2026-09-11
- **Decision:** Adopt `lore-v1`, an append-only decision ledger, and tentative-by-default design documents. Start authority behavior at `annotate` as the setup default, with `rank` available later.
- **Rationale:** Separate the owner's requirements and accepted choices from the agent's detailed, revisable proposals.
- **Consequences:** Consult this ledger before treating plans as binding. Promotion of additional choices requires specific authorization. Repository-wide agent instructions are proposed separately for review.
- **Supersedes:** None
- **Canonical sources:** [Canon rules](README.md)

## D-0002 — Establish the persistent cube ecosystem brief and current planning scope

- **Date:** 2026-09-11
- **Status:** Accepted
- **Scope:** Product intent, physical display constraints, engineering priorities, and the deliverable requested now
- **Decided by:** Wrysk, the ecosystem brief and final scope instruction supplied on 2026-09-11
- **Decision:** Plan a persistent ambient ecosystem on five connected 64×64 cube faces; prioritize continuous topology, ecological and evolutionary variety, expressive tiny organisms, long-running stability, and environmental input hooks. Reuse the existing display shim. For now, initialize the repository and produce plans; implementation follows later.
- **Rationale:** The installation should remain a living ambient world over long periods, with meaningful changes between glances, without depending on user attention or a scalar fitness objective.
- **Consequences:** Default presentation is the world without analytical overlays. The briefs may select and explain preferred mechanisms, but specific algorithms, bottom-edge behavior, runtime choices, numerical budgets, and milestone gates remain proposals.
- **Supersedes:** None
- **Canonical sources:** [Owner's brief](../brief.md)

## D-0003 — Install the approved repository agent instructions

- **Date:** 2026-09-11
- **Status:** Accepted
- **Scope:** Repository agent working rules and their installation
- **Decided by:** Wrysk, replying "go ahead" to the explicit request to install the reviewed rules as `AGENTS.md`
- **Decision:** Install the exact reviewed working rules in root `AGENTS.md`, making the vault's authority conventions and the existing display-shim boundary available to agents working in this repository.
- **Rationale:** Keep design provenance and project constraints available during ordinary work without requiring the vault-setup skill to be invoked again.
- **Consequences:** The approved rules are installed. This approval does not promote the proposed simulation architecture or authorize starting implementation.
- **Supersedes:** None
- **Canonical sources:** [Repository working rules](../../AGENTS.md)

## D-0004 — Godot owns presentation and the app; Rust stays the simulation

- **Date:** 2026-10-07
- **Status:** Accepted
- **Scope:** The engine and the split between simulation and presentation, for the ambient piece and the future game
- **Decided by:** Wrysk, on 2026-10-07: "i would rather just do the full port" and, after weighing Unity and Unreal, "yeah godot is the pick for sure"
- **Decision:** Godot becomes the presentation and application layer: terrain, water and organism rendering, rigging, animation, lighting, camera, picking, menus, save slots, presets, seed picking and input. Rust remains the presentation-agnostic simulation (world, water, flora, fauna, weather, world generation, save format, query layer), reached through a narrow, data-driven GDExtension binding. The custom Vulkan presentation is retired once Godot reaches parity. Unity, Unreal and an egui shell over the Rust renderer were considered and not chosen.
- **Rationale:** The game needs an engine's UI, input, gamepad and platform export (macOS is a target for the full game). The Rust–Godot binding is needed in any Godot future, so keeping it narrow limits upkeep, and building the look once in Godot avoids porting a look that may change.
- **Consequences:** No new features go into the Rust GPU presentation; fixes only until parity. The Tachyon panel does not constrain this direction. The visual style (voxel or illustrated pixel art) is not decided by this entry. Packages, checks and names in the canonical source remain proposals.
- **Supersedes:** None
- **Canonical sources:** [Godot direction handoff](../handoffs/godot-direction-2026-10-07.md), section "The split"

## D-0005 — Godot owns the app; the Rust renderer draws the world

- **Date:** 2026-10-07
- **Status:** Accepted
- **Scope:** The engine and the split between simulation, world rendering and application, for the ambient piece and the future game
- **Decided by:** Wrysk, 2026-10-07, after the Godot built-in lighting comparison ("dude the new one looks like absolute fucking dogshit ... its not even CLOSE to parity with the old renderer"): "agree" to keeping the Rust renderer for the world inside the Godot app; "the pixel effect was a lot more load bearing to the look than i had realized. definitely keep it"; "the smooth movement and rotations and interpolation was the one thing about godot that was like, oooh this is good. so yeah i think we should get that working in our renderer"
- **Decision:** Godot is the application layer: menus, save slots, presets, seed picking, inspect, input and gamepad, audio, platform export, and animation authoring tools. The world is drawn by the Rust renderer (`cubarium-gpu`) in its pixel-scale look, running inside the GDExtension and shown in Godot's view under the app's UI. Organisms gain smooth interpolated movement and free rotation (later rigging) inside the Rust renderer, lit by its own lighting model. Rust remains the simulation behind the session API. Godot's own world rendering (meshes and built-in lighting) is not the presentation.
- **Rationale:** The comparison showed that the look is the custom renderer (the voxel walk, face rules, lighting model and the pixel-scale raster with its nearest upscale); Godot's built-ins could not stand in, and porting meant rebuilding a working renderer. The shell already built in Godot keeps its value. The ambient displays keep the same renderer.
- **Consequences:** The Rust GPU presentation is no longer retired: it gains a frame bridge into Godot, panning, picking, and a model pass for moving organisms; the pixel-scale raster is kept; the pack moves off the simulation's critical path. Godot's WorldView meshes and built-in lighting become a fallback to remove. Packages, checks and names in the canonical source remain proposals.
- **Supersedes:** D-0004
- **Canonical sources:** [Hybrid renderer handoff](../handoffs/hybrid-renderer-2026-10-07.md), section "The split"

## D-0006 — Game UI look: neon pixel diorama

- **Date:** 2026-10-09
- **Status:** Accepted
- **Scope:** The look of the Godot app's menus (field guide, populations, world, settings); not the cube display
- **Decided by:** Wrysk, over UI mockup rounds 1–3, 2026-10-08/09: "something around pixel neon diorama lands best for me"; Gridlite for title and chrome with his axis settings; "i kinda still want pixelify at 23px"; "notched lines, dithered bands, i really like lit dots with glow focus"; "brighter tabs only." On recording it: "dont get pedantic about it. design decisions like this can change at any time"
- **Decision:** Pixel neon diorama, as written up in the canonical source: Gridlite PE for titles and chrome, Pixelify Sans for text, notched neon panels on dithered violet bands, lit-dot controls, glow focus, brighter tabs.
- **Rationale:** Wrysk's picks from side-by-side mockups.
- **Consequences:** Agents build UI to this look rather than inventing one. It's a living direction: Wrysk changes any part of it whenever he likes, and the canonical source is simply updated to match. Open parts (hover, settings controls, the palette as a whole, Pixelify's 5) stay open.
- **Supersedes:** None
- **Canonical sources:** [Game UI design language](../ui/design-language.md)
