---
design_status: exploration
last_reviewed: 2026-09-21
decision_refs: []
---

# Caves, grottos and layered hollows

Companion to the [terrain generation plan](terrain-generation-plan-2026-09-21.md).
Wrysk, 2026-09-21: the world should use its vertical space through habitable
hollows and a layered design. This plans that work and names the seams the
current lane must leave open. It changes no rule.

## What the sources ask for

- Art direction v0.1: terrain reads as "slopes, ledges, banks, strata,
  cavities, pools, and continuous masses" (§ terrain, l. 78); the cutaway
  diorama "can reveal surface habitats, water volumes, cavities, and exposed
  terrain layers" (l. 152); crags, exposed layers and abrupt cliffs stay
  distinctive (l. 80).
- Terrain and ecosystem proposal (2026-09-16, ll. 118–121, 154–160): voxels
  permit "ledges, overhangs, multiple ground surfaces at the same horizontal
  location, and later burrows"; shape bedrock for "upper hollows, sloping
  flanks, ledges and shelves"; side views "look into small aquarium-like
  habitats, including their solid floors"; habitable slopes and terraces are
  generated deliberately.
- Voxel ecology sketch (2026-09-16): umbrellafrond tolerates low light in a
  "canopy, overhang, hollow"; glowcap wants "a humidity floor from pore water
  or overhead cover"; locations are a support face, not a skyline; a
  vertical-only light check would wrongly darken sheltered habitat.

## What already supports hollows

The core is further along than the generator. Only the generator and the
founders assume one ground per column.

| Layer | State | Where |
| --- | --- | --- |
| World model | Roofed voids are legal; only voids the sky cannot reach are filled (`repair_isolated`). Support faces exist per column (`is_support`, `supports_in_column`). | `cubarium-voxel/src/generate.rs:291–350`, `world.rs:169–195` |
| Light | `sky_visibility` is a hemisphere fan (zenith, 8 × 60°, 8 × 30°), so a floor under an overhang gets lateral light. Flora multiplies crown shade onto it. | `world.rs:264–281`, flora `step.rs:340` |
| Water | The local exchange handles stacked cavities and roofed passages (fixtures in the exchange-geometry brief). Perched pools work if the geometry holds them. | `water.rs` |
| Fauna | Animals stand on any support face shallow enough to wade, layers included; the senses graph is built per support layer and never links roof to floor. **No body-height clearance check.** | fauna `step.rs:544–560`, `senses.rs:831` |
| Presenter | Per-column roof table; floors under a roof are shaded with a 4-voxel falloff (`roof_shade`); the slab walk draws whatever the camera can see, so a cavity meeting the front cut reads as a section. GPU path has the same roof table. | host `present.rs:272–278, 515–533` |
| Fixture | `--scene authored` already has a hollow with water and an overhang. | host `voxel/scene.rs` |
| Generator | **Single heightfield, no carving; tests assert no overhang for the default landform.** | `generate.rs:88–277, 418–437` |
| Founders | Plant sites come from the skyline (`skyline_of` → `surface_y`), not from support faces. | host `voxel/habitat.rs:436–449` |

## Three kinds of hollow

1. **Undercuts and grottos.** A hard stratum caps softer sediment on a steep
   bank; the soft layer is notched back, leaving a roofed floor open at the
   mouth. Two to four voxels tall, a few voxels deep. Lit from the mouth and
   the lateral rays, damp from the roof's runoff. Habitat for umbrellafrond and
   glowcap, shelter for browsers. Cheap: derived from the eroded heightfield
   and the hardness layers, no 3D noise.
2. **Galleries and grottos with skylights.** Longer roofed passages inside
   rock, following soluble strata bands, opening to the surface at a mouth on a
   slope or a skylight from above. Dark beyond the falloff, fed by the spring
   cell or runoff through the mouth, holding pools in bowl-shaped floors.
   Habitat for the decomposer lane on litter washed in, and a refuge. Carved
   from periodic 3D coherent noise thresholded inside the chosen strata, then
   connected or filled.
3. **Shelves and terraces under a lip.** Two or more support faces in one
   column: a ledge with a roof over its back half, a terrace under an overhang.
   The "layered design" at its simplest. Falls out of 1 and 2 plus the
   erosion's benches; no separate mechanism.

Not in scope: burrowing by animals, live cave collapse, lava tubes or any
hollow that needs its own physics.

## How the camera sees them

The camera is a 30° elevated orthographic view of a cutaway; the front wall
`z = 0` is the cut. A hollow is visible only if

- it meets the cut, so its floor, ceiling and water show in section like an
  ant farm; or
- its mouth faces the camera on a front-facing bank; or
- it has a skylight the camera looks down into.

So the generator biases hollows toward small `z` and toward front-facing banks,
and every habitable hollow must pass a camera check: some floor cell of it is
drawn by the presenter's own occlusion walk. A hollow nobody can see is filled
or moved, not kept. The skyline visibility pass keeps its current job on the
skyline only; it does not level a hollow's floor and needs no change.

## Generation pipeline placement

Carving is a stage on the voxel **volume**, after voxelisation and before
habitat preparation, and after erosion, because undercuts are differential
erosion and galleries follow the strata the erosion exposed:

```text
heightfield (bedrock, sediment, hardness)  ← slice 1, slice 2
  → voxelise into a Volume                 ← slice 1
  → carve hollows                          ← THIS WORK
  → prepare habitat: skyline visibility pass, repair isolated voids,
    camera check for hollows               ← slice 1 (+ camera check here)
  → hydrology, founders                    ← slice 3, on support faces
```

Carve inputs: the material volume, the per-column layer depths, a hardness
field in metres, the recipe's `hollows` section (density, target heights,
front bias, strata to follow, clearance). Carve outputs: the volume, a list of
hollows (cells, floor faces, mouths) for the founders and the camera check.

Sealed sections keep today's rule: filled. A gallery that would be sealed gets
a mouth cut toward the nearest slope face within a bounded distance or is
dropped. Passages under the clearance are drainage, not habitat, and are not
counted as hollows.

## Ecology and presentation follow-ups

- **Founders** (slice 3) enumerate support faces, not the skyline, and treat
  hollow floors as candidate umbrellafrond and glowcap sites using the light
  and moisture the world reports. Already the plan's intent ("support, not
  skyline"); hollows just make it necessary.
- **Fauna clearance**: `faces_in_column` gains a headroom test against the
  species' body height, so a browser does not stand in a two-voxel slot.
  Model-rule change; its own test pass.
- **Water in rock**: caves in impermeable rock are wet only through the mouth,
  the spring cell or a seep. If galleries stay dry, a recipe seep point (a
  small spring conductance on a cave ceiling) is the lever; not before the
  water is seen.
- **Presentation**: roof shade exists; luminous accents from glowcaps in dark
  hollows are stage-2 art direction work, not part of this package.

## Slicing

Recommended as **slice 2b** of the terrain line, between erosion and water:
water and founders must see the final geometry, and undercuts need the eroded
layers. Doing it after slice 3 would redo slice 3 for hollows.

| Package | Result | Focused checks |
| --- | --- | --- |
| 2b-i Undercuts and camera check | Grottos under hard caps on front-facing banks, all camera-visible, on the three presets | No isolated voids after carve + repair; every hollow's floor is a support face with clearance; presenter draws at least one floor cell per hollow; seam periodicity |
| 2b-ii Galleries and skylights | Periodic 3D-noise galleries in chosen strata, connected by mouth or skylight, bowl floors holding pools after a short settle | Connectivity, clearance, pool on a bowl fixture within a few hundred ticks, no roof over the spring cell |
| 2b-iii Ecology fit | Founders on hollow floors; fauna headroom; a browser reaches a grotto floor on a fixture | Establishment gate on a sheltered floor with hemisphere light; `faces_in_column` rejects a low slot; reach test on the authored scene |

2b-i and 2b-ii could be one work package for the generator worker after slice
2. 2b-iii touches flora, fauna and the host and is a separate package with its
own test pass. Nothing here runs a world past a few hundred ticks.

## Seams to leave open in the current lane

Cheap now, expensive later. Ordered by urgency.

1. **Hardness as a field, not a column sine** (slice 1, in flight). Emit rock
   strata from a periodic hardness function of `(x, y, z)` in metres carried
   in the recipe, so erosion (slice 2) and carving (2b) key off the same
   layers the renderer shows. The current generator's per-column sine is the
   thing being carried over; carry it over as such a function.
2. **Explicit stages with a value between them** (slice 1). Heightfield →
   `voxelise` → `Volume` → `prepare`. Carving slots between the last two.
3. **`Recipe.hollows`, default empty** (slice 1). A serde-defaulted section so
   later recipes add hollows without moving any existing preset.
4. **No-overhang is a per-preset assertion, not a generator invariant**
   (slice 1). Keep the test for `Ridge` and the slice-1 presets; do not build
   the staged voxeliser so that a roofed cell is unrepresentable.
5. **Slice 2 keeps its layers** (slice 2 brief). The erosion solver's bedrock,
   sediment and hardness per column survive into the voxeliser's output, and
   the solver flags "hard cap over soft" banks; that flag is 2b-i's input.
6. **Front bias knob** (2b). Nothing now.

Seams 1–4 fit inside the slice 1 package already running and need one message
to the worker. Seam 5 goes into the slice 2 brief.
