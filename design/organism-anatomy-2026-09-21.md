---
design_status: proposal
last_reviewed: 2026-09-21
decision_refs: []
---

# Organism anatomy: the shape each organism has in the simulation

Wrysk's design session, 2026-09-21. This is the organism-design pass proper: what shape
and life history each organism has **in the model**, so that the ecology (shade, reach,
sensing, occupancy), the lifecycle and the picture are one thing. The species dossiers
(`design/art-direction/species-dossiers-2026-09-21.md`) describe the picture; this
document is what the picture depicts. Everything here is a proposal until Wrysk marks
it; the model changes it implies are listed in §6 as packages.

## 1. What the model holds today

Verified in `crates/cubarium-voxel-flora/src/{lib.rs,step.rs}`,
`crates/cubarium-voxel-fauna/src/{senses.rs,manifest.rs}`,
`crates/cubarium/src/voxel/{stand.rs,animal.rs}`:

- **Every plant is a lollipop.** A stand occupies a trunk column from its support face
  to `crown_voxels(wood)` and one **crown disc**, one cell thick, of radius
  `crown_radius(wood)` at that level. Height and radius interpolate linearly in
  `wood / wood_max` between authored `[min, max]`. Glowcap is the degenerate case: one
  cell on the face.
- **Every consumer reads the lollipop.** Light: a stand is shaded by every crown whose
  top is higher and whose disc footprint contains it, by
  `exp(-shade_k · foliage / disc_area)` (`step::light_per_stand`). Reach: a mouth takes
  from any stand whose disc level is at or below its ceiling and whose footprint is
  within its horizontal reach (`FloraView::reachable_foliage`). Senses: the vision cone
  sees disc cells as `FoliageCrown` or `StrippedCrown` (`senses::cone_occupancy`).
  Presenter: trunk cells plus disc cells.
- **Foliage has no position.** `foliage` is one number per stand; the disc is where all
  of it is. There is no low foliage and no high foliage.
- **Animals are drawn at twice their model body.** `animal::cells_of` draws a shell of
  `2 × body_length × 2 × body_width`; sensing, collision and the mouth keep the manifest
  body (browser 0.25 × 0.125 m, crawler 0.125 × 0.0625 m).

Consequences that are wrong for the ecology as designed:

1. A mature bloomcrown (3 voxels today, 3–8 under the ladder) has **no foliage a
   frondgrazer can reach** (reach up 1). The biosphere's grazed meadow, where the grazer
   keeps bloomcrown young, cannot happen: it can only eat seedlings and turf.
2. Umbrellafrond's tiers, vaulttree's lobes with sky gaps, a shrub's fan: none exist for
   shade, reach or sight. Shade is all-or-nothing under one disc.
3. The animal the viewer sees is not the animal the model runs; its mouth is half a
   body-length behind where the picture puts it.

## 2. The shape model (proposal)

A species carries a **profile**: an ordered list of layers, each a band of the crown's
height with a radius, a kind, and a share of the stand's foliage. Height and radius
still scale with wood as now; the profile says what is in the volume.

```
Layer { kind: Trunk | Foliage | Drape | Mat,
        band: [from, to]      // fractions of crown_height(wood), 0 = support face
        radius: r,            // fraction of crown_radius(wood)
        share: s,             // fraction of the stand's foliage in this layer (Foliage/Drape/Mat; sums to 1)
        porosity: p }         // 0 = a solid disc, 1 = no shade; sky gaps, drapes
```

`FloraView::layers(stand) -> impl Iterator<(band_voxels, radius_voxels, kind, foliage_in_layer, porosity)>`
evaluated at the stand's current wood. The four consumers read layers instead of the
disc:

| Consumer | Today | With layers |
| --- | --- | --- |
| **light** (`light_per_stand`) | one disc per crown above me | every Foliage/Drape/Mat layer of another stand whose band is above my top and whose radius contains me attenuates by `exp(-shade_k · (1-p) · foliage_in_layer / layer_area)` |
| **reach** (`reachable_foliage`) | the whole stand if its disc level ≤ my ceiling | only the layers whose band intersects `[my face, my ceiling]` and whose radius is within my horizontal reach; the bite takes from those layers' foliage, and a stand's foliage is refilled bottom-up |
| **senses** (`cone_occupancy`) | disc cells | each layer's cells, Trunk as an occluder, Foliage as `FoliageCrown`, Drape as a thin occluder |
| **presenter / sprites** | trunk + disc | the sheet's growth states are the profile at four wood values; the art is drawn to the profile |

Foliage bookkeeping stays one number per stand (`P`); layers hold **shares** of it, so
the mass balance is untouched. A bite removes from the reachable layers' shares first;
regrowth fills shares in profile order (low first), which is what makes a browsed plant
look browsed from below. That is the one new rule. Nothing about water, mineral,
reproduction or death changes.

**Lifecycle over wood.** The profile may differ by **stage of wood**, so a seedling is
not a small adult: `profile: [(wood_fraction_threshold, Profile)]`, the first whose
threshold ≥ `wood/wood_max` applies. Two or three entries per species are enough
(seedling, juvenile, adult); senescence is a foliage loss, not a shape.

## 3. Plant anatomy, species by species

Bands are fractions of crown height; radii fractions of crown radius; `share` sums to 1
over the foliage layers. Sizes are the ladder's (`organism-scale-and-roster` §4).

### Springturf (crown 1–1.5 v, radius 1–2)

| Stage | Layers |
| --- | --- |
| all | Mat 0–1.0, r 1.0, share 1.0, p 0.3 |

No trunk at any size. A newborn is one tuft (r 0.5); a full clump 4 v wide. **Reach:**
entirely within a grazer's reach; cropped foliage is the mat thinning, regrowth is fast.
**Shade:** it shades nothing taller than itself; with p 0.3 it shades a seedling in its
footprint, which is the "turf closes the ground" effect the biosphere wants. **Lifecycle:**
germinates on drained moist soil in the open, spreads by hop 3, burns out in 250 s
without income; the *cropped margin* is the visible state, not a stage.

### Velvetpad (crown 1 v, radius 1.5–3)

| Stage | Layers |
| --- | --- |
| all | Mat 0–1.0, r 1.0, share 1.0, p 0.6 |

No trunk. A sheet, offered only under a standing crown. **Reach:** reachable, but it is
not on the grazer's diet list in the biosphere (the pad is shelter carpet); whether the
grazer eats it is a diet decision, default **no**. **Shade:** p 0.6, nearly none.

### Stonecushion (crown 1–1.5 v, radius 1–1.25)

| Stage | Layers |
| --- | --- |
| all | Foliage 0–0.6, r 1.0, share 0.7, p 0.2 · Foliage 0.6–1.0, r 0.6, share 0.3, p 0.2 |

No trunk; a dome. Lives on a rock face with a soil pocket in its root box. **Reach:**
reachable; not on the grazer's list (durable leaves); default no. **Lifecycle:** 11,500 s
to full size, one package per 600 s; drought-content, pool-killed.

### Bloomcrown (crown 3–8 v, radius 1–2.5)

| Stage (wood/max) | Layers |
| --- | --- |
| seedling ≤ 0.2 | Foliage 0–1.0, r 1.0, share 1.0, p 0.4 (a rosette on the ground, 1 v tall) |
| juvenile ≤ 0.5 | Trunk 0–0.4, r 0.15 · Foliage 0–0.25, r 0.7, share 0.4, p 0.4 (basal rosette) · Foliage 0.5–1.0, r 1.0, share 0.6, p 0.3 (crown) |
| adult | Trunk 0–0.5, r 0.15 · Foliage 0–0.15, r 0.6, share 0.25, p 0.4 (basal rosette) · Foliage 0.55–1.0, r 1.0, share 0.75, p 0.3 (crown of vanes) |

The **basal rosette** is the anatomy that makes the grazed meadow work: a grazer reaches
the rosette (band 0–0.15 of 8 v is the first voxel) and never the crown. Cropping the
rosette takes a quarter of the plant's foliage and its regrowth refills the rosette
first, so a browsed bloomcrown stays leaf-poor at the base and its income drops;
repeated browsing keeps it small. **Shade:** the crown is a real disc-like layer; p 0.3.
**Ripe:** the parcel is the bloom in the crown; the warm accent lives there.
**Lifecycle:** seedling rosette → stem lifts the crown → adult; senescence drops
foliage share from the crown first (the rosette persists), dieback shortens the stem.

### Umbrellafrond (crown 8–16 v, radius 3–6)

| Stage | Layers |
| --- | --- |
| seedling ≤ 0.15 | Foliage 0–1.0, r 1.0, share 1.0, p 0.4 (one tier at the ground) |
| juvenile ≤ 0.5 | Trunk 0–0.5, r 0.15 · Foliage 0.5–0.65, r 1.0, share 0.6, p 0.4 · Foliage 0.85–1.0, r 0.6, share 0.4, p 0.5 |
| adult | Trunk 0–0.4, r 0.2 · Foliage 0.4–0.55, r 1.0, share 0.4, p 0.4 (lowest tier) · Foliage 0.65–0.8, r 0.8, share 0.35, p 0.4 · Foliage 0.9–1.0, r 0.5, share 0.25, p 0.5 (top tier) |

Three tiers whose lowest is the widest, so the shade under an adult is layered: the
outer footprint gets one tier's shade, the centre gets three. **Reach:** an adult's
lowest tier starts at 0.4 × 8 = 3.2 v, out of the grazer's reach; a seedling and a
juvenile's lowest tier are reachable, which is "tougher frond at lower throughput" only
while the plant is young. If Wrysk wants adults browsed, the grazer needs a taller reach
or the frond a hanging Drape layer; default **no**. **Lifecycle:** slow; reserve-buffered
recovery; saturation-immune (its niche stand-in).

### Vaulttree (crown 18–28 v, radius 5–8; new species)

| Stage | Layers |
| --- | --- |
| seedling ≤ 0.1 | Foliage 0–1.0, r 1.0, share 1.0, p 0.4 |
| juvenile ≤ 0.4 | Trunk 0–0.6, r 0.15 · Foliage 0.6–1.0, r 1.0, share 1.0, p 0.4 |
| adult | Trunk 0–0.5, r 0.15 · Trunk 0.5–0.7, r 0.6, p 0.85 (limbs: sparse occluder, no foliage) · Foliage 0.7–1.0, r 1.0, share 0.85, p 0.45 (lobes with sky gaps) · Drape 0.35–0.5, r 0.7, share 0.15, p 0.9 (hanging filaments) |

The **sky gaps** are a number (p 0.45): an adult vault lets more than half the light
through, which is what keeps an understory alive under it and is why velvetpad and
umbrellafrond can be its neighbours. **Establishment** needs a gap: light gate ≥ 0.9.
**The fall** (decision 4): at death, `die` deposits `wood` as dead wood along
`crown_radius` sites in a hashed direction from the site (each site an equal share), so
the log the glowcap colonises is as long as the crown was wide. The drapes are the
seedporter's road, later. **Lifecycle:** slow, long-lived, large reserve.

### Lanternberry (crown 5–9 v, radius 2–3.5; new species)

| Stage | Layers |
| --- | --- |
| seedling ≤ 0.2 | Foliage 0–1.0, r 1.0, share 1.0, p 0.5 |
| adult | Trunk 0–0.3, r 0.4, p 0.7 (the fan of stems) · Foliage 0.3–1.0, r 1.0, share 1.0, p 0.5 |

No single trunk; foliage low and wide. **Reach:** band 0.3 of 5 v = 1.5 v: a small
adult's lowest foliage is at the edge of the grazer's reach (1 up), a large adult's is
not. So the shrub is browsed while young and escapes when grown, which is the
biosphere's "grove edge". **Fruit:** the parcel now; a fruit product later.

### Siphonreed (crown 6–12 v, radius 0.5–1; new species)

| Stage | Layers |
| --- | --- |
| all | Foliage 0–1.0, r 1.0, share 1.0, p 0.7 |

A porous column, no trunk. **Site:** requires free water on or beside the site (a
positive `water_depth_min`), which is the model addition. **Reach:** standing in water
deeper than the grazer wades (0.05 m) it is unreachable; on a saturated bank it is.
**Shade:** p 0.7; a reed bed shades the water's edge lightly.

### Glowcap (cap 1–2 v, radius 0.5)

| Stage | Layers |
| --- | --- |
| all | Foliage 0–1.0, r 1.0, share 1.0, p 0.5 (the fruiting body on the face) |

A saprotroph; `foliage` is the cap tissue. No change to its box, uptake or spread.
**The colony axis** (button / fruiting / ripe / spent) is wood and parcel, as the
dossier reads it. **Model request kept:** none.

## 4. Animal anatomy

The manifest body is the animal. **Retire the 2× drawn shell**: the drawn body equals
the model body, and the ladder's sizes are model sizes.

| Founder | Model body (ladder) | Occupancy | Mouth | Sensing (the manifest's inputs, as anatomy) |
| --- | --- | --- | --- | --- |
| **frondgrazer** (Browser) | 0.75 × 0.375 × 0.375 m = 6 × 3 × 3 v | its cells on the face it stands on, one layer | at the front of the shell's arched notch, reach horizontal 1, up 1 (a raised neck), from the face it stands on | `Cone(3, foliage/body)`: three forward cones; drawn as the two flush magenta patches on the shoulders (a wide, low binocular pair). `Contact(4)`: the four pads' contact. `Wet`, `Taste(1)`, `Chem(litter)`, `Light`. |
| **littershredder** (Blind) | 0.375 × 0.125 × 0.125 m = 3 × 1 × 1 v | one layer | the plough at the front, on the face; reach horizontal 1, up 0 | `Contact(4)`: the two feelers and two front pads. No cone: no eye anywhere on the body. `Chem(litter)` is what it steers by: feelers down. |

**Lifecycle as anatomy.** Body 0.005 → 0.05; length scales with the cube root of body,
so a newborn grazer is 0.46 adult length (3 v). A birth costs 0.01 from the parent's
reserve at body ≥ 0.03; the newborn eats the same diet. The **starving** flag (reserve 0,
body < 0.015) is the shell lowered onto the legs. Sizes drawn: five body sheets per
angle.

**Reach as anatomy.** With the profiles above, "reach up 1" is a real statement about
what the meadow looks like: turf, rosettes, seedlings and young shrubs are food; crowns
are not. A taller browser is a different animal (Wrysk's reference creature, over 2 m).

## 5. What this pass changes in the ecology, stated plainly

- Grazing pressure shifts from "any plant whose disc is low" to "the low foliage of any
  plant": bloomcrown and lanternberry become browsable while young and escape when tall.
  This is the biosphere's succession story and it was not possible before.
- Shade becomes graded: tiers and lobes with porosity, so an understory under a vault
  gets real light. Expect more coexistence under canopy, fewer sharp exclusions.
- Bites come from the bottom, regrowth fills the bottom first: a browsed plant is visibly
  browsed and its income drops before its structure does.
- Nothing about water, nutrients, reproduction, seed banks or death changes; the mass
  balance is untouched.

## 6. Model packages (owed; not dispatched)

1. **Layers.** `Layer`, `Profile`, `SpeciesConfig::profile` (staged by wood fraction),
   `FloraView::layers(stand)`; `light_per_stand`, `reachable_foliage`,
   `cone_occupancy` and `stand::parts_of` read layers. Bites take from reachable layers,
   regrowth refills in profile order. Presets for the live six per §3. Schema bump; fresh
   worlds only (`always-fresh-never-migrate`). Tests: the reach and shade rows of §3.
2. **Ladder geometry.** `crown_height_voxels` / `crown_radius_voxels` per §3 sizes;
   world height 72; presenter raster 6 px per voxel.
3. **Animals 1:1.** Manifest body lengths to the ladder; `animal::cells_of` draws the
   model body.
4. **Vaulttree, lanternberry, siphonreed** presets with their profiles; vaulttree's
   dead-wood line at death; siphonreed's water-depth establishment gate.
5. **Bellwing** is a later package (flight is a new movement rule).

Each is a fresh-world schema change; the fast-iteration policy applies (short function
tests, no long runs). Rebalance by looking at the meadow, not by tuning first.
