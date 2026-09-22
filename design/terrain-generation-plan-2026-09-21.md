---
design_status: exploration
last_reviewed: 2026-09-21
decision_refs: []
---

# Terrain generation across scales

Proposal following Wrysk's request of 2026-09-21: richer multi-octave terrain,
erosion, settled starting water, better founders, and large landscapes from
which to choose a small Cubarium. Small ringworlds remain required. Larger live
worlds with pan/zoom and optional left/right boundaries are possibilities, not
accepted implementation choices. This document plans work; it changes no rules
or running world.

## Recommended shape

Separate **regional landscape**, **live habitat**, and **viewport**. A region
can be much larger than the simulated habitat; a live habitat can be larger
than the visible window. Neither relationship requires changing voxel size or
shrinking organisms. Build the same generation pipeline for native small rings
and extracted habitats, then add regional selection and larger live worlds.

```text
seed + physical scale + topology
  → regional relief / geology
  → erosion / deposition / drainage
  → select habitat and resolve its boundaries
  → final terrain / voxelization / hydrology
  → initialize and settle water
  → habitat-aware founders
  → live simulation → independently movable viewport
```

## What exists now

- `crates/cubarium-voxel/src/generate.rs:88–277`: one broad ridge and receiving
  basin, two broad cosine terms, four weak harmonic variations, soil estimated
  from slope and elevation, layered rock and three soil pockets. There is no
  erosion simulation in this generator. A visibility pass lowers foreground
  terrain to make the surface readable from the fixed camera.
- `crates/cubarium-voxel/src/config.rs:8–106`: configurable dimensions and
  voxel size; core defaults are 128 × 48 × 24 at 0.25 m/voxel. X wraps;
  front/back are no-flow walls. Host/display presets can differ.
- `crates/cubarium-voxel/src/world.rs:463–499`: generation charges aquifer and
  atmospheric stores and records initial water. The runtime already has a
  closed water cycle; see the [water-cycle handoff](handoffs/voxel-water-cycle-2026-09-20.md).
- `crates/cubarium/src/voxel/habitat.rs:119–334,436–517`: startup settles water
  for 400 ticks, selects clustered plants using material/water/elevation
  proxies, provides logs and litter, and places browsers near reachable crowns
  where possible. Browser placement can fall back to soil without food.

These are working-tree observations, including ongoing edits, rather than a
claim about what any particular display currently runs.

## 1. Generate landforms in physical units

Introduce a generation recipe separate from runtime water/ecology settings.
Keep named deterministic streams for relief, geology, erosion, water and
founders, so adjusting one pass does not reshuffle unrelated choices.

Use a floating-point heightfield with bedrock height, erodible sediment depth,
and material hardness. Delay voxel quantization until the habitat is prepared.
Start with several scales of coherent noise:

- Broad relief for hills, basins and plateaus.
- Ridged noise mixed into selected rocky regions, with modest domain warping
  to break parallel or obviously sinusoidal landforms.
- Intermediate detail for spurs, hollows and banks.
- Restrained fine detail, limited by generation and eventual voxel resolution.

Expose wavelength, relief, persistence and lacunarity, with an initial candidate
of 4–6 resolved octaves. Frequencies and amplitudes use metres. Increasing the
world extent adds geography; it must not merely stretch one hill. Reducing
voxel size improves resolution; it must not change physical feature size.
Small-map recipes use fewer resolved octaves and a compact selection of
landforms, without squeezing an entire regional landscape into a tiny ring.

Every field and domain warp must respect the chosen topology. Periodic noise
alone is insufficient if erosion, sediment transport or later smoothing sees
the seam as a wall. Use one boundary policy across all generation passes.

## 2. Erode before voxelization

Start with a CPU grid solver over heightfield layers, with a fixed work budget:
rainfall/runoff, downhill water flux, sediment entrainment and transport,
deposition, then slope relaxation of loose material. Hardness limits bedrock
erosion; transported sediment creates deeper soil in receiving areas and
exposed rock on erosional slopes. Track removed, deposited and exported material
so smoothing cannot silently manufacture soil.

The aim is readable catchments, banks, rocky shoulders and depositional flats.
Keep geological erosion time separate from ecological ticks. Initial erosion
water is a modelling tool, not automatically the live world's water inventory.
Live erosion, caves, GPU generation and infinite streaming are later options.

[Mei, Decaudin and Hu's hydraulic erosion work](https://evasion.imag.fr/Publications/2007/MDH07/)
is an implementation reference to investigate, not a commitment to its GPU
architecture. For basin analysis, [Priority-Flood](https://arxiv.org/abs/1511.04463)
provides depression filling and watershed labelling. Our proposed adaptation
uses a derived drainage surface to find spill levels while preserving actual
terrain depressions as potential ponds. A closed ring has no exterior drain:
closed basins need explicit storage/merge handling, not a fictitious edge outlet.

After any habitat boundary or visibility adjustment, recompute local drainage,
soil relationships and basin geometry. After quantization, check again for
blocked one-cell channels and changed spill heights.

## 3. Large regions and honest patch selection

Generate a region as compact 2D fields first. An illustrative first regional
size is 1024 × 256 samples, followed by 2048 × 512 if timing is comfortable;
sample spacing is independent of the live voxel size. At 0.25 m spacing the
first example spans 256 × 64 m. Allocate dense 3D cells only for the selected
habitat. Refine the selection with surrounding context; local detail can use
a margin, but watershed inflows require regional drainage information and
cannot be inferred from a narrow margin alone.

Offer several candidates with distinct geography, plus manual selection.
Compare wet/dry area, soil/rock, connected walkable habitat, pond accessibility,
headroom and side-view readability. Prefer a variety of plausible habitats over
maximizing one score or requiring every species in every patch.

| Habitat mode | Boundary treatment |
| --- | --- |
| Native small or large ring | Generate and erode periodically in X from the outset. This is the first implementation. |
| Selected region adapted into a small ring | Prefer compatible edges; reconstruct a transition band with periodic constraints, preserving the interior where possible. Recompute drainage and water afterward. Label the result as adapted. Reject excessively destructive joins. |
| Bounded crop, optional | Preserve the crop geometry, use solid/no-flow left/right walls initially, and recompute the cut catchments. Later inlet/outlet boundaries would need explicit water accounting. |
| Viewport into a larger live ring | Only the camera crops. The simulated world keeps its original circumference and hydrology. |

An arbitrary regional crop cannot simultaneously preserve its exact geometry,
original river flows and a new smaller periodic circumference. Do not conceal
this by just wrapping array indices or crossfading the rendered edge. An
alternative to adapting a crop is regenerating a periodic habitat from the
selected region's relief/material characteristics; that is inspiration from
the region, not literal extraction.

Recommend self-contained extracted habitats initially: no imaginary upstream
river continuously importing water from an unsimulated region. A preserved
riverbed can become seasonal or dry. Continuous through-flow is a separate
boundary feature.

The current camera visibility pass belongs to habitat preparation, not regional
geology. Initially retain it as an explicit diorama constraint and prefer
readable crops that need little alteration. Apply it before final hydrology;
do not level an established river after filling it. Presentation work follows
the [art direction](art-direction/Cubarium_Art_Direction_v0.1.md).

## 4. Start with water already in place

Target **hydrostatic balance plus a settled water cycle**, rather than claiming
one exact steady state under intermittent rain, evaporation and plant uptake.

1. Choose an explicit total water inventory scaled to physical habitat area
   and storage capacity. Account for surface, pore, aquifer and atmosphere
   together; do not independently charge each to a full target.
2. Use basin volumes and spill heights to seed connected pools at consistent
   heads. Allocate moisture to soil according to the existing retention rules
   and aquifer exchange. A single aquifer head cannot encode arbitrary perched
   water tables; initialize those only where actual geometry/materials retain them.
3. Reserve atmospheric water for the cycle and use the runtime's own water
   solver for bounded relaxation. Never interpret erosion runoff as proof of
   live hydrological equilibrium.
4. Replace the unconditional 400-tick startup assumption with convergence
   diagnostics plus a hard time/iteration cap: changing pool levels, storage
   drift, pore moisture and active versus inaccessible water. A stable but dry
   locked state is not a viable wet habitat.
5. Where startup can cheaply cover shower cycles, compare successive cycle
   ranges rather than demanding zero change per tick. Otherwise label the
   result physically settled with cycle viability still unverified. Longer
   checks belong in an explicit bounded study, not ordinary tests or every launch.

Expose regenerate, keep candidate and add water in generation controls. A
generator may recommend another candidate when the requested wet habitat is
unsupported; it should report why and cap retries. Do not repeatedly inject
water or move organisms during the live run to preserve the initial composition.

## 5. Founders follow the habitat

Reuse the actual flora establishment and resource rules for site suitability:
root-zone moisture, soil depth/material, light, drowning and substrate. Factor
shared predicates if necessary rather than maintaining a second ecological
model in the host. Evaluate adult starter suitability too: passing a seed gate
alone does not establish that a large founder can maintain itself.

Place patch centres with minimum spacing, then grow irregular, mixed-size
colonies within connected suitable habitat. Keep gaps and transition zones.
Count founders by usable physical area and intended starting coverage, with
small-world caps, rather than fixed totals or raw voxel count. Seed logs/litter
before decomposers and book those starting resources. Recheck light suitability
as canopies are placed; independently suitable stands can shade each other.

Place fauna after food exists. Check body clearance, support, water tolerance,
mouth reach, nearby edible stock and connected routes to another food patch.
Retain seed diversity and spatial separation without forcing an unsuitable
species into every map. Report a shortfall instead of using the browser's
foodless fallback. Resource-based counts are a starting heuristic, not a claim
of measured carrying capacity or successful learned navigation.

Any maturation/settling after planting must be bounded and reported, including
founder deaths. Recheck water with transpiration active. Do not use a hidden
long ecological burn-in to make a broken initial condition appear successful.

## 6. Larger live worlds and viewing

Add viewport position and zoom independently of simulation dimensions, voxel
metres and organism size. Pan should cross the ring seam continuously; picking,
care actions and CPU/GPU presentation must use the same world-to-view mapping.
Changing camera position must not regenerate terrain, pause offscreen organisms
or redefine the topology. Keep controls and generation diagnostics in explicit
development UI; the normal ambient display remains the world.

Increase live extent incrementally, measuring memory and tick/frame cost. Dense
voxel allocation scales with width × height × depth, and camera culling reduces
drawing work without eliminating offscreen water or ecology cost. Keep the
whole live habitat active first. Consider chunking or distant simulation only
if measurements demand it. Consult and reuse the shim's current geometry and
transport contract when adapting this to physical cube output.

## Delivery order and small checks

| Slice | Reviewable result | Focused verification |
| --- | --- | --- |
| 1. Staged native ring generator | Recipe, physical scales, periodic multi-octave relief; existing generated-scene entry point can try it | Seam neighbourhoods, bounds, same-seed repeatability, small/default/wide sizes |
| 2. Erosion and soil | Visible catchments, deposition and material variation on those rings | Sediment accounting, no negative layers, periodic flux, tiny slope/basin fixtures |
| 3. Water and founders | Pools and moist habitat at startup; suitable plant colonies and feeding opportunities | Water inventory, communicating basins, bounded settling, establishment and first feeding on short fixtures |
| 4. Regional generation and selection | Large field preview, candidate patches, explicitly adapted ring extraction | Crop coordinate/scale consistency, join quality, recomputed drainage and removed upstream dependencies |
| 5. Larger live habitat and pan/zoom | Explore a wider ring while ecology continues outside the view | Seam rendering/picking, unchanged simulation under camera moves, measured memory and tick cost |

Optional bounded left/right worlds can branch from slice 1 if literal cropping
becomes the priority. This is a shared topology change across indexing, flow,
movement, senses, dispersal and rendering, not a generator-only flag; trace
those dependencies before implementation. Walls are the narrow first option;
open water/organism exchange adds a separate budget and behavior contract.

First practical milestone: a handful of clearly different small/default/wide
rings, with eroded wet and dry landforms, initialized water and plausible
founders, available through ordinary development controls. The regional atlas
then reuses those passes. No new test should run past a few hundred ticks;
run only touched-crate checks. Explicit longer studies remain bounded and
ignored by default. Retain recipes in source history, not collections of
generated worlds or captures.
