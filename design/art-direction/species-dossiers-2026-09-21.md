---
design_status: proposal
last_reviewed: 2026-10-08
synced_at: e66b92138e8a3aab6c5b15729b7195a0911f18a1
decision_refs: []
---

# Species dossiers (Wrysk's design session, part 2)

> **Sim sync (2026-10-08):** ecology below is checked against `e66b92138e8a3aab6c5b15729b7195a0911f18a1`.
> `Sim state` distinguishes shipped code defaults from the named desktop terrarium
> preset; founder status means configured availability, not an observed population.
> Other presets, saved worlds and trained brains can differ. Config keys refer to
> `config/desktop/terrarium.toml` where stated; `design/parameters.md` lists code
> defaults, not that preset's overrides. Numbers are tunable facts of this revision.
> The look, body proposals and visual-review notes are retained. Missing ecological
> intent is labelled *not in the sim yet*; implementation does not confer canon.

> **Visual review (2026-09-24–25):** the selected images in
> `art/gen/artbook/README.md` and the visual notes below supersede conflicting
> shape and colour proposals in this dossier for concept art. The earlier body
> plans remain below as exploration; model geometry and state art have not been
> reconciled. Use `design/art-direction/species-colours-2026-09-24.md` for the
> earlier colour pass and its later concept-pick exceptions.

The [visual artbook](../../art/gen/artbook/index.html) collects the picks and
retained alternatives. Veilhound (formerly Chorister), ripple-snail, capgnawer and seedporter have
their current selection notes in [the body dossiers](../animal-body-reimagining-2026-09-24.md).
Painted outer halos are cleanup defects, not organism emission. Terrain images
remain candidates; their inclusion does not approve a tile set or a new simulation feature.

Xenobiologist's field documentation for each Cubarium organism and substrate family,
written for Wrysk to pick from, mark, or mix. Each dossier proposes one **primary body
plan** in full and one or two alternates in brief; sprite-forge roughs of the primary
(and where useful an alternate) sit in `art/gen/runs/2026-09-21-dossiers-*/`. Nothing is
decided until Wrysk says so.

**The shape each organism has in the model** is in
`design/organism-anatomy-2026-09-21.md`: layered profiles per species, what the grazer
can reach, what shades what, and the model packages. A dossier's picture is drawn to
that profile; where the two disagree the anatomy document wins.

Ground rules (from `organism-scale-and-roster-2026-09-21.md` §7 and
`Cubarium_Art_Direction_v0.1.md`):

- **Unit.** One voxel = 0.125 m = **6 logical px**. Dimensions are given as
  voxels (px). Readability is checked at 4 px per voxel. The px figures are the
  **target logical size** after our own pixelation (kit v0.4, Wrysk 2026-09-21):
  generators render a clean graphic illustration and are never asked for pixel art;
  `pixel_snap.py` or the atelier bake pixelates to these sizes.
- **State first.** Every visible variation names the simulation state that drives it.
  Ornament is fixed per family and never pretends to be state. Where a state would make a
  better picture and does not exist, it is listed under *model requests*.
- **Composite sheets.** Where two things always appear together on one site (fungus and
  its wood, litter and the ground), they are one sheet with axes, not two sprites.
- **Say what to draw.** Descriptions are positive: every part, count, colour by hex,
  surface, angle and ground contact. The image model imagines nothing.
- **Phrasings that generate** (batch-2 roughs, 2026-09-21): every size line states the
  **aspect as a ratio** ("three times as tall as it is wide") and tall subjects render on
  a tall canvas, because a square canvas outweighs any measurement in prose; repeated
  parts state the **gap** between them, not only the count; **no similes** (a "petal"
  becomes a petal); a **surface** (velvetpad, litter) uses the tile framing and a
  **flyer** its own framing, never the sprite block's "base resting on the ground".
- **Palette family** (`design/appearance.md` plus the audit's working additions):
  floor `#12093A`; deep plum `#2A0E4A` (the one dark support); mid violet `#3A1A7A`;
  detritus violet `#510B6D`; pale lilac `#B99BE6`; producer ramp `#1E2798` → `#42C5F8`;
  electric cyan `#42C5F8`; magenta `#FF2AFC`; warm accent `#FF9B50`, reserved.
- **Camera.** Elevated orthographic, 30°, side-on; the world is a ring seen along its
  depth. Animal angles: side (facing right, mirrored), yaw 45° toward, yaw 45° away.

---

## D1 · Dead wood (substrate family) with the glowcap colony

### Field entry

*Common name:* dead wood, "the fall". *What it is:* the woody structure of any stand
that has died, lying where it stood. *What the simulation holds:* a dead-wood pool per support site (with mineral and energy), without host-species
or log orientation. Vascular stand death contributes wood; founder logs supply an
initial substrate. Vaulttree death already distributes its wood across a line of
support sites. Glowcap mycelium dies into litter rather than manufacturing more
woody substrate. Wood leaves through decomposition and glowcap uptake; the named
desktop preset uses `flora.wood_decomposition` ≈6.886e-5/s, rather than the code
default 1e-4/s.

The sprite family therefore has exactly three honest axes:

| Axis | Source | Values |
| --- | --- | --- |
| **amount** | the site's dead wood | fragment (0.05–0.25) · section (0.25–1.0) · trunk (> 1.0) |
| **joins** | dead wood ≥ 0.05 on the x-neighbour sites | lone · left end · middle · right end |
| **colony** | glowcap stand on this site: wood and parcel | none · button · fruiting · ripe · spent |

The presenter composes: `tile[amount][joins]` under `caps[amount][colony]`. A z-neighbour
(behind or in front) is not joined; a trunk line lying along the depth is drawn as
overlapping lone sections, which under the 30° camera reads as a log seen end-on.

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live per-support substrate pool, not a species (source: `crates/cubarium-voxel-flora/src/lib.rs:566`)
- inputs: vascular wood at death, vaulttree fall-line shares, founder logs; fungal body goes to litter (source: `crates/cubarium-voxel-flora/src/step.rs:3768`)
- consumers: glowcap substrate uptake (source: `crates/cubarium-voxel-flora/src/lib.rs:2596`)
- decay: preset ≈6.886e-5/s; code default 0.0001/s (sources: `config/desktop/terrarium.toml` → `flora.wood_decomposition`; `crates/cubarium-voxel-flora/src/lib.rs:3230`)
- identity: scalar organic/mineral/energy; no host species or log shape/orientation (source: `crates/cubarium-voxel-flora/src/lib.rs:566`)

### Body plan of the wood: "split trunk" (primary)

Each site's tile is **1 voxel wide (6 px)** on the ground face, so the tile is a slice
of a trunk lying along x. Heights by amount:

- **fragment**: 0.75 (5 px) tall, a broken chunk with one flat split face up.
- **section**: 1.5 (9 px) tall, a trunk slice with bark on the upper half and a
  lengthwise split running along the middle of the front face.
- **trunk**: 2.5 (15 px) tall, thick, bark plates in two rows, the split wider and
  deeper (the vaulttree's fall).

Joins: a **middle** tile continues the bark plates and the split across both edges. A
**left end** closes the split with a torn break: three blunt fibre splinters project left
from the top, middle and bottom of the face, 3, 2 and 3 px long, tips one pixel. A
**right end** shows the root end for a trunk (a flared foot 1 px wider than the tile at
the base) or a clean oblique cross-section for a section (two concentric rings, `#510B6D`
on `#2A0E4A`). A **lone** tile has the torn break on the left and the cross-section on
the right.

Surface: bark plates deep plum `#2A0E4A`, plate seams `#12093A`, the split's interior
pale lilac `#B99BE6` with a `#510B6D` edge, underside third `#12093A`. Contact: two
flats under the left and right thirds joined by a shallow concave bark edge, each flat
with a 1-px `#12093A` contact shadow; the tile never floats.

**Selected host appearance (2026-09-24):** the [glowcap-on-vaulttree image](../../art/gen/artbook/glowcap-on-vaulttree.png)
uses a fallen vaulttree section with angular violet bark plates and a pale split.
The cyan lines seen within that log are **glowing fungal mycelium growing through
the wood**, not teal veins belonging to the wood.
The mycelial network extends through the colonized log, rather than being confined
to small areas around individual mushroom stalks.

Decay within an amount bin is the amount itself sliding down: fragment tiles are what
sections become, so the fragment inherits the section's split (now a hollow) and its
plate pattern with two plates missing.

### The glowcap (*Lucerna saprophaga*)

**Selected appearance (2026-09-24):** [purple spotted glowcap on fallen vaulttree](../../art/gen/artbook/glowcap-on-vaulttree.png).
Use a broad, tilted purple mushroom cap with flat dark-violet spots, lilac and
magenta markings, a vivid cyan brim and underside, narrow cap-side fissures with
cyan light peeking through, and a violet stalk emerging
through the split. The selected image shows one prominent mushroom on a short
log; colony count and presentation at other wood sizes remain to be worked out.
The saddle-crown anatomy below is the earlier proposal, not the selected look.

*Niche.* A substrate-fed shade/cave fungus sharing the support site with a vascular
stand. Its root box reaches dead wood and litter; that carbon is required even when
bounded seep income in dark habitat supplements it. Budgeted uptake pays needs
rather than blindly draining the box at its maximum rate. Caps are edible; after
they are cropped, a configured share of mycelium becomes available to fungal diets.
The capgnawer occupies that niche; the littershredder now eats litter and carrion.
Open sun dries exposed fungal tissue, with shelter from the fungus’s own dark habitat.
It spreads by spores, and its dead body returns to litter. “Spent” remains a proposed
visual interpretation of depleted tissue, *not in the sim yet* as a separate life stage.

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:2596`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- food: substrate-fed saprotroph; no photosynthesis; dead wood/litter carbon required, plus bounded dark seep income (source: `crates/cubarium-voxel-flora/src/lib.rs:2596`)
- habitat: shade, caves and detritus patches; fungus slot shares a site with vascular plant; dark-biased establishment and dampness (source: `crates/cubarium-voxel-flora/src/lib.rs:2596`)
- size: authored crown height × diameter 0.125–0.25 × 0.125 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:2596`)
- life_stages: seed bank → living stand; morphology profiles by wood, dieback/death by stores and stress (sources: `crates/cubarium-voxel-flora/src/lib.rs:2596`; `crates/cubarium-voxel-flora/src/lib.rs:1585`; `crates/cubarium-voxel-flora/src/step.rs:3768`)
- breeding: spores (hop 6), spore funding scales with cap fullness; body at death becomes litter when substrate_required; paid parcels and species-specific seed-bank clocks; preset rates override defaults (sources: `crates/cubarium-voxel-flora/src/lib.rs:2596`; `config/desktop/terrarium.toml` → `flora.glowcap`)
- consumers: capgnawer; fungal mycelium exposed after caps are cropped; littershredder excluded by diet (sources: `crates/cubarium-voxel-fauna/src/body.rs:1827`; `crates/cubarium-voxel-fauna/src/body.rs:2047`; `crates/cubarium-voxel-flora/src/lib.rs:2596`)
- defence: induced grazing defence and a refuge floor (sources: `crates/cubarium-voxel-flora/src/lib.rs:2596`; `config/desktop/terrarium.toml` → `flora.glowcap.defence_half_life_s`)
- movement_senses: sessile; responds to light/water/stress; dispersal is not locomotion (source: `crates/cubarium-voxel-flora/src/lib.rs:2596`)
- substrate: budgeted uptake; substrate_required true, mycelium_graze_fraction 0.5; open sun dries caps/mycelium above sun_shade_max 0.2, dark habitat shelters it (sources: `config/desktop/terrarium.toml` → `flora.glowcap`; `crates/cubarium-voxel-flora/src/step.rs:3768`)
- not_in_sim_yet: explicit spent life stage or release-driven spore-puff animation (retained D1 request; source: `crates/cubarium-voxel-flora/src/lib.rs:242`, stage remains Alive)

*Body plan, primary: "saddle crown".* The visible organism is a fleshy folded **fan**
growing out of the trunk's split, not a stem with a cap. One colony = one stand:

- **[collar]** a lobed attachment 4 × 2 px `#510B6D` sitting in the split, one pale
  lilac `#B99BE6` edge on its upper lip. It is the part that says "grows *through* the
  wood".
- **[crown_1]** the mature crown: an asymmetric saddle 9 × 5 px (1.5 × 0.8 voxels)
  on a thick short stalk 3 × 3 px. The left lobe rises 2 px above the right; a broad
  central dip joins them. Upper surface mid violet `#3A1A7A`, underside `#2A0E4A`.
- **[crown_2]** a juvenile saddle 5 × 3 px on a 2 × 2 stalk, to the right and a little
  behind crown_1, its base painted under crown_1's overlap.
- **[button]** a closed knob 2 × 2 px `#3A1A7A` on a 1-px stalk at the collar's left.
- **[lip]** the luminous tissue: a continuous 1-px strip of electric cyan `#42C5F8`
  along each crown's **lower-facing edge**. This is the only emissive read; the upper
  crown is a quiet mass and the surrounding bark stays dark.
- **[threads]** two 1-px `#510B6D` mycelial lines running from the collar along the
  split into the neighbouring tile, so a colony visibly reaches along the wood.

*Colony states* (the `colony` axis), all at the same anchors so identity holds:

| State | Drives from | What is drawn |
| --- | --- | --- |
| button | wood ≤ 0.02 | collar + button; lip absent |
| fruiting | 0.02 < wood, parcel < half | collar, crown_1 sized by wood (5 → 9 px wide), crown_2 from wood 0.06, button; lip at half intensity `#2A86B8` |
| ripe | parcel ≥ half of a package | as fruiting; lip full `#42C5F8`, and a 1-px lip on crown_2 |
| spent | wood falling, below donor_min after a peak | crowns fold their lobes down to half height, `#510B6D`; collar and threads unchanged; lip absent |

The paid glow is honest: the lip brightens only as the stand has saved propagule
material, and a colony that has just released is dim.

*Alternates.* **(B) Bracket shelves:** three stacked half-discs projecting from the
split, scalloped outer edges, cyan crescent under each; growth adds shelves from the
bottom. **(C) Lantern cups:** two or three upward cups on 2-px stalks, rim luminous,
the cup interior dark; ripe cups tilt as if heavy. A is the least like an Earth
mushroom; C is the most legible at 4 px.

*Model requests.* None needed for the family above. Nice-to-have: an event when a
parcel is released (a spore puff), which would be driven by the existing paid
propagule step.

*Sheet plan.* Wood: 3 amounts × 4 joins = 12 tiles, each 6 px wide × up to 15 tall.
Caps: 3 amounts × 4 colony states = 12 overlays (the caps sit differently on a fragment
than on a trunk), registered to the tile's split. Static; no sway.

---

## D2 · Litter on the ground (terrain axis), and carrion

**Terrain candidate review (2026-09-25):** [dead-foliage litter](../../art/gen/artbook/litter-dead-foliage.png)
uses recognisable fallen fragments from our plants, including umbrellafrond,
vaulttree and springturf, in muted petrol, mauve and lilac. Further positions,
densities and mixtures are needed. The [artbook](../../art/gen/artbook/index.html)
also retains rock, damp/puddle/growth, soil-depth, bedrock and water studies as
candidate sources only. These do not fix tile counts or add simulated lichen.

### Field entry

*What it is:* fallen foliage, fine/coarse fragments, dead reserve, fungal remains
and gut waste; a per-site organic/mineral/energy pool. Littershredders eat it,
capgnawers can fall back to it, and glowcaps draw substrate from it. Decomposition
is ≈6.886e-4/s in the desktop terrarium (code default 1e-3/s). Carrion has its own
slower clock and a carcass identity record; hunters and shredders eat the carrion
pool. The retained surface treatment below is the earlier art direction.

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live litter/carrion pools with mineral and energy (source: `crates/cubarium-voxel-flora/src/lib.rs:566`)
- litter_food_web: shredder primary; capgnawer fallback; glowcap substrate (sources: `crates/cubarium-voxel-fauna/src/body.rs:1827`; `crates/cubarium-voxel-fauna/src/body.rs:2074`; `crates/cubarium-voxel-flora/src/lib.rs:2596`)
- carrion_food_web: littershredder, lanternjaw, Veilhound, opportunist seedporter; carcass record preserves body identity (sources: `crates/cubarium-voxel-fauna/src/body.rs:1733`; `crates/cubarium-voxel-flora/src/lib.rs:540`)
- decay: preset litter ≈0.0006886/s, carrion ≈0.0001928/s; separate code defaults 0.001/s and 0.00028/s (sources: `config/desktop/terrarium.toml` → `flora`; `crates/cubarium-voxel-flora/src/step.rs:4012`)

### Terrain surface tiles gain a litter axis

The topsoil surface autotile (edge, inner, slope, corner, lip-under-rock) gets four
**density** variants driven by the site's litter amount:

| Density | Litter | What changes on the tile |
| --- | --- | --- |
| bare | < 0.05 | the plain material |
| light | 0.05–0.3 | four to six single `#510B6D` flecks lying in the tile's low pixels |
| drift | 0.3–1.0 | a continuous 1–2 px band of `#510B6D` with `#3A1A7A` flecks along the tile's downhill edge and against any rock lip; the band follows the contour |
| deep | > 1.0 | the band deepens to 3 px, pale lilac `#B99BE6` fragments (curled leaf ends, 2 × 1 px) lie on it, and the topsoil's own texture is covered on the lower half |

Rock surface tiles get two variants only (bare, cracks: litter lodges as 1-px lines
in the crack pattern). Under standing water no litter is drawn (the water skin owns the
surface). The band always sits on the **downhill** side and against obstacles, which is
what makes it lie *in* the ground instead of on it.

### Carrion

Rare and short. Two marks, drawn on the ground face: **fresh** (carrion ≥ 0.05): a
collapsed body silhouette 2 × 0.5 voxels (12 × 3 px), `#2A1628` with a `#8A7C9A` bone
line along its top; **bone** (0.01–0.05): the bone line alone, 6 × 1 px. It is a mark
in the same litter band, so a carcass lies in the drift.

---

## D3 · Frondgrazer (*Tardigradus pratensis*, "the frondgrazer")

**Selected appearance (2026-09-25):** [coral frondgrazer](../../art/gen/artbook/frondgrazer.png).
A low six-legged browser has a continuous coral saddle shield with raised front
and rear masses, dark wine saddle markings, and a pale underside. Its muscular
folded neck ends in a transverse upward-facing cropping scoop. Each of the two
corner stalks now ends in a small eye-like sensor with a dark plum lens and magenta
rim. These replace the plain tapered palp tips in the earlier proposal; the shield's
magenta sensory patches remain. Outer background glow is an artifact to remove.
The selected illustration guides anatomy; earlier pixel dimensions and rigs below
have not yet been updated.

### Field entry

*Niche.* The ground foliage browser. The physical mouth band and footprint choose
reachable vascular layers and vine leaves; no fungus is food. Its gut limits intake,
and juvenile surplus grows body before adult reproduction. The desktop preset’s
newborn body is 0.15 org-units, adult breeding threshold 0.9, full body 1.5. Birth
is a paid gestation, not an immediate reserve debit. It searches beyond exhausted
patches, scans while feeding, and reacts to hunters and herd alarms. The herd runs
first; caught adults can hunker together, while calves seek their shields. Steps,
wading and drowning depend on its body in metres, not fixed “one voxel” reach.

*Size under the ladder.* Model body 0.375 m; drawn shell **6 × 3 × 3 voxels = 36 × 18
px** side view (adult). A newborn is about 0.46 of adult length by volume: 3 × 1.5
voxels (18 × 9 px). Length scales with the cube root of body.

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- code_id: `frondgrazer`; display name Frondgrazer (sources: `crates/cubarium-voxel-fauna/src/body.rs:1846`; `crates/cubarium-observatory/src/query.rs:20`)
- status: live founder in the desktop terrarium: 8 bodies; code default 8 bodies (sources: `crates/cubarium-voxel-fauna/src/body.rs:1846`; `crates/cubarium-voxel-fauna/src/body.rs:1846`)
- diet: vascular foliage only; includes reachable stand layers and vine leaves, excludes fungus (source: `crates/cubarium-voxel-fauna/src/body.rs:1846`)
- size: adult length × width × height 0.75 × 0.375 × 0.375 m; juvenile dimensions scale with cube root of body / body_max (sources: `crates/cubarium-voxel-fauna/src/body.rs:1846`; `crates/cubarium-voxel-fauna/src/body.rs:2510`)
- body: core.body_min 0.15, breeding threshold 0.9, full size 1.5 org-units in the desktop preset; shipped mass scale 30 × placeholder quantities, not 30 × physical dimensions (sources: `config/desktop/terrarium.toml` → `fauna.founders.frondgrazer.core`; `crates/cubarium-voxel-fauna/src/body.rs:1846`; `crates/cubarium-voxel-fauna/src/lib.rs:2308`)
- activity: diurnal; hunger can keep an off-phase animal awake (sources: `crates/cubarium-voxel-fauna/src/body.rs:1846`; `crates/cubarium-voxel-fauna/src/body.rs:1846`)
- life_stages: juvenile → adult; juvenile/adult boundary is core.birth_body, not full adult size (source: `crates/cubarium-observatory/src/query.rs:342`)
- lifespan: median 57600 s (8 reference days at 7200 s/day); each body draws its lifespan (sources: `fauna.founders.frondgrazer.core.lifespan_median_s` in `design/parameters.md`; `crates/cubarium-voxel-fauna/src/body.rs:143`; history `3021eea9`)
- breeding: paid gestation; preset gestation ≈84.28 s, interval ≈1097.83 s and surplus hold ≈548.91 s (sources: `config/desktop/terrarium.toml` → `fauna.founders.frondgrazer.core.reproduction`; `crates/cubarium-voxel-fauna/src/body.rs:1846`)
- senses: food cone plus live sight (8 m day / 4 m night), hearing, smell, tremor and kin tracks (source: `crates/cubarium-voxel-fauna/src/body.rs:5987`)
- movement_habitat: ground walk; 0.375 m up/down; adult wade 0.1875 m, drown depth 0.375 m after sustained submersion; canopy food limited by its physical mouth band (source: `crates/cubarium-voxel-fauna/src/body.rs:1846`)
- defence: herd runs first; caught adults bunch and hunker, juveniles seek an adult shield; nearby pack members can pry adults open (sources: `crates/cubarium-voxel-fauna/src/vigil.rs:401`; `crates/cubarium-voxel-fauna/src/vigil.rs:964`)
- predators: Veilhound; lanternjaw for size-eligible smaller bodies; actual eligibility also reads size and mobility (source: `crates/cubarium-voxel-fauna/src/predation.rs:94`)
- not_in_sim_yet: drinking and an animated birth emerging under the skirt (retained D3 model requests; current repertoire: `crates/cubarium-voxel-fauna/src/body.rs:1846`; the reachable foliage band is implemented)

### Body plan, primary: "saddle shield" (developed from Astra's alternative)

A low, wide browser whose top is one continuous **shield**, dipped in the middle and
raised at both shoulders, over six short legs that spread outward so the footprint is
wide and there is visible ground between the supports. From the front of the shield a
thick **folded neck** rises into a transverse, upward-facing **cropping scoop**: the
animal feeds by unfolding the neck and pressing the scoop's inner edge up into foliage
above it. There is no face. Sensing is a pair of **magenta patches** on the shield's
shoulders.

Parts (rig names), side view facing right, adult:

- **[shield]** 30 × 8 px, a long shallow saddle: front shoulder at x = 6 rising to 8 px,
  dip to 5 px at x = 15, rear shoulder 7 px at x = 26, then a rounded skirt falling to
  x = 30 that stops 2 px above the ground line. The shield's rim is a 1-px `#3A1A7A`
  band; the mass is deep plum `#2A0E4A`, lit upper third mid violet `#3A1A7A`. Two
  overlap seams curve down from the shoulders, dark `#12093A`, so the shield reads as
  three plates in one rigid part.
- **[sense_l] [sense_r]** on each shoulder, a magenta `#FF2AFC` patch 3 × 2 px with a
  1-px `#12093A` inset below it. These are the inherited body accent (hue drifts toward
  cyan in kin). Not eyes: they are flush, on the shield, above the front legs.
- **[neck]** 6 px long, 4 thick, rooted under the front shoulder at the shield's front
  opening (an arched notch, 6 px wide, 4 high). At rest it lies folded back into the
  notch; cropping unfolds it forward and up through 60°, a two-segment hinge.
- **[scoop]** a transverse trough 8 px wide, 3 tall, open upward, on the neck's end:
  outer surface `#2A0E4A`, inner cutting edge pale lilac `#B99BE6` (1 px). While
  cropping the scoop tips up and the inner edge shows; at rest it faces forward and
  only its dark outer surface shows below the shield's front.
- **[palp_l] [palp_r]** two 4 × 1 px tapered stalks rooted at the scoop's corners,
  pale lilac; folded inward beside the scoop at rest, spread up and forward while
  cropping, tips separate.
- **[leg_f/m/r] × near/far** six legs; pairs attach 7, 16 and 25 px behind the front
  edge. Each: an upper segment 3 px, a lower segment 3 px, a two-toed pad 4 × 2 px
  `#510B6D`. Legs `#2A0E4A`; the far three are `#12093A`-shaded copies 2 px lower on
  the screen (the camera's 30°), showing between the near ones.
- **[spur]** a blunt 3 × 2 px rear spur under the skirt; tucked at rest, lifted 1 px
  when walking.

Ground contact: every pad on one world plane; the near pads on the ground line, the
far pads 2 px below it (the tilt), the skirt clear of the ground by 2 px.

Poses from `State`:

- **Resting**: neck folded, scoop forward, palps in, spur tucked, all pads planted.
  Slow 2-frame breathing: the shield's dip rises and falls 1 px over 4 s.
- **Walking**: alternating tripods (near-front + far-middle + near-rear, then the
  other three); 4 frames; lifted pads rise 2 px; the shield rocks 1 px; spur lifted.
  Pace at 1 body length per second, from the pace rule.
- **Cropping**: neck unfolded 60°, scoop tipped up with its lilac edge showing, palps
  spread; all six pads braced (planted, legs a little wider); 3 frames of the scoop
  pressing up 1 px and back. The foliage it takes belongs to the plant sprite (the
  crown's cropped state), so the bite reads from both sides.

Condition from `body` and `reserve`: **size** from body (juvenile 18 px, adult 36 px,
three intermediate sheets); **starving**: shield lowered onto the legs (skirt touches
the ground), the seams open 1 px showing `#12093A` between the plates, the sense
patches desaturate to `#7A2A78`. No warm accent anywhere on this animal; the bite is
mechanical and the palette stays cool.

Angles: yaw 45° toward shows the arched front notch, both sense patches and the scoop's
open trough; yaw 45° away shows the skirt, the spur and the seams sweeping toward the
viewer, the neck hidden under the far shoulder. Same parts, same world dimensions.

*Alternates.* **(B) Dome carapace** (draft F1 with Astra's corrections): three
overlapping dorsal plates, an arched head opening, a separate lower jaw, six grouped
sensory pits along each flank, short cyan shoulder edges only. Reads "armoured" faster;
risks the tortoise. **(C) Bellows grazer**: the front third of the body is a soft
pleated bellows with a rasp plate on top; feeding inflates the bellows so the rasp is
pushed up under foliage; the rear two thirds a hard low shell. Strangest silhouette;
hardest to keep from reading as a slug.

*Model-request sync:* the physical mouth band and low foliage profiles in request (1)
are implemented. Drinking and the animated birth in (2)–(3) are *not in the sim yet*.
The original requests are retained:

*Model requests.* (1) With crowns rising to 3–8 voxels, "reach up 1" needs a defined
reachable band per species (a low-foliage fraction), otherwise the grazer can no longer
crop a mature bloomcrown; a package with the ladder. (2) A *drinking* state does not
exist; `drink` is not drawn. (3) A birth event would let the young appear from under the
skirt rather than pop in; nice-to-have.

*Sheet plan.* Per angle (side, toward, away): parts sheet for the cutout rig (shield,
neck ×2 segments, scoop, palps ×2, legs ×18, spur, sense patches in the shield mask),
plus baked clips rest (2 f), walk (4 f), crop (3 f), at 5 body sizes and 2 conditions.

---

## D4 · Littershredder (*Segmenta caeca*, "the blind shredder")

**Selected appearance (2026-09-25):** [long tapered littershredder](../../art/gen/artbook/littershredder.png).
Use the second generated version: an elongated crawler with overlapping rounded
gold plates, dark brown bands, pale underside and short legs, a large blunt front
plough, two downward feelers and one magenta sensor spot on the second plate.
It remains eyeless. The rear progressively tapers; the later bulkier, less-tapered
version was not selected. Preserve the selected silhouette; the exact plate/leg
count needs reconciliation rather than enforcing the earlier five-segment sketch.
Outer background glow is an artifact to remove.

### Field entry

*Niche.* The eyeless detritivore: litter and carrion, with caps excluded from its
diet. Detritus chemistry, contact, taste, wetness and its hunters’ footfall guide
feeding and freezing/withdrawal. It is dark-active at its own face, can climb terrain
walls, and lays paid clutches in litter; the live preset sizes a clutch from surplus
and reserves income for a brood. Cropping still means feeding, Walking locomotion
and Resting stillness. The earlier drawn-size proposal remains below; current
physical dimensions are in Sim state.

*Earlier size annotation (retained look):* Model body 0.19 m under the ladder; drawn shell **3 × 1 × 1
voxels = 18 × 6 px**. The model-body annotation differs from current physiology
and is listed in the drift file.

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- code_id: `littershredder`; display name Littershredder (sources: `crates/cubarium-voxel-fauna/src/body.rs:1735`; `crates/cubarium-observatory/src/query.rs:20`)
- status: live founder in the desktop terrarium: 8 bodies; code default 8 bodies (sources: `crates/cubarium-voxel-fauna/src/body.rs:1735`; `crates/cubarium-voxel-fauna/src/body.rs:1735`)
- diet: litter and carrion only; cap tissue is excluded (source: `crates/cubarium-voxel-fauna/src/body.rs:1735`)
- size: adult length × width × height 0.375 × 0.125 × 0.125 m; juvenile dimensions scale with cube root of body / body_max (sources: `crates/cubarium-voxel-fauna/src/body.rs:1735`; `crates/cubarium-voxel-fauna/src/body.rs:2510`)
- body: core.body_min 0.045, breeding threshold 0.15, full size 0.375 org-units in the desktop preset; shipped mass scale 30 × placeholder quantities, not 30 × physical dimensions (sources: `config/desktop/terrarium.toml` → `fauna.founders.littershredder.core`; `crates/cubarium-voxel-fauna/src/body.rs:1735`; `crates/cubarium-voxel-fauna/src/lib.rs:2308`)
- activity: dark-active by its own face light, rather than only the global night (sources: `crates/cubarium-voxel-fauna/src/body.rs:1735`; `config/desktop/terrarium.toml` → `fauna.founders.littershredder.active`)
- life_stages: juvenile → adult; eggs are counted separately; juvenile/adult boundary is core.birth_body, not full adult size (source: `crates/cubarium-observatory/src/query.rs:342`)
- lifespan: median 43200 s (6 reference days at 7200 s/day); each body draws its lifespan (sources: `fauna.founders.littershredder.core.lifespan_median_s` in `design/parameters.md`; `crates/cubarium-voxel-fauna/src/body.rs:143`; history `3021eea9`)
- breeding: litter clutches paid from surplus, up to 3 eggs; 0.8 brood share; waits up to 1800 s for full clutch; crowd brake (sources: `config/desktop/terrarium.toml` → `fauna.founders.littershredder.core.reproduction`; `crates/cubarium-voxel-fauna/src/body.rs:1735`)
- senses: eyeless: detritus chemistry, taste, contact, wetness and predator footfall tremor (source: `crates/cubarium-voxel-fauna/src/body.rs:5987`)
- movement_habitat: damp litter and carcass patches; walks and climbs terrain walls; step 0.25 m (source: `crates/cubarium-voxel-fauna/src/body.rs:1735`)
- defence: freezes then withdraws on its hunters’ footfall (source: `crates/cubarium-voxel-fauna/src/vigil.rs:477`; history `3b69a248`)
- predators: lanternjaw; actual eligibility also reads size and mobility (source: `crates/cubarium-voxel-fauna/src/predation.rs:94`)

### Body plan, primary: "plough segment"

Five overlapping segments, each a rounded plate 4 px long and 6 tall, the front plate
tallest and tipped forward into a **blunt plough** with two short **feelers** (3 × 1 px,
pale lilac) angled down to the ground; the rear plate tapers to a 1-px point. Under each
segment a pair of 1-px legs, so the animal moves as a rippling row. Colour: plates
`#1E2798` (the cool end of the producer ramp, so it differs from the grazer's plum), lit
top line `#2B4AC8`, feelers `#B99BE6`, a single magenta `#FF2AFC` pixel on the second
segment's top (the inherited accent). No eyes at all.

Poses: Resting: plates closed, feelers down. Walking: 3 frames, a wave passing back
along the segments, feelers sweeping. Cropping (shredding): the front plate rocks down
and up 1 px, 3 frames, the litter band under it losing a fleck (that pixel belongs to
the terrain litter axis, D2, and changes with the pool). Starving: the segments pull
apart 1 px, the magenta pixel goes `#7A2A78`.

*Alternate.* **(B) Comb crawler**: a single low hull with a row of 6 comb teeth along the
underside front, dragging litter into itself. Simpler rig; less motion.

*Sheet plan.* Side (mirrored), toward, away; rest 2 f, walk 3 f, shred 3 f; 3 sizes.

---

## Plants: what every plant sprite reads

A stand holds `wood` (size), `foliage` (fullness, at most α·wood with species/preset α), `reserve`,
`light` (0–1 after sky and canopy), `moisture` (0 = wilting), `aeration_stress`,
`parcel` (saved propagule material), separate `nectar` and `fruit`, and `mineral`.
Crown dimensions follow authored profiles, with capped seedlings for tall species. There is one stage, Alive; dieback
is wood shrinking, senescence is paid, death removes the stand and leaves dead wood on
the site and foliage in the litter. A **seed mark** shows a recent landing or an armed germination while its bank
still holds seed; an old dormant cohort alone does not show one. Springturf can
also cure/dormant and wake. The retained sheet axes below are look proposals;
nectar/fruit state is not interchangeable with the reproductive parcel:

| Axis | Drives | Values |
| --- | --- | --- |
| size | wood / wood_max | 4 sizes (newborn, young, grown, full) |
| fullness | foliage / (α·wood) | full · thinned (cropped or shaded) · bare |
| water | moisture, aeration_stress | turgid · wilting · waterlogged (species with a response) |
| ripe | parcel ≥ half a package | plain · ripe (the one warm or bright accent per species) |

Sway is decoration and stays continuous; growth between sizes is a runtime blend. A
plant claims nothing else.

---

## D5 · Seed cohorts ("sprouts")

*What it is.* Paid propagule material of one species lying on a site in an arrival
bin, with no income, waiting for the site to pass the species' establishment predicate.
A mark represents a recent landing or armed germination, not every dormant bank. *Sprite:* a species-coloured **seed mark** on the
ground face: for bloomcrown a 2-px `#42C5F8` dot with a 1-px `#3A1A7A` tail; for
umbrellafrond a 3 × 1 px `#1E2798` crescent; for springturf three 1-px `#42C5F8` dots
in a row; for stonecushion and velvetpad a single `#B99BE6` pixel; for glowcap nothing
(spores are not drawn). Drawn as part of the terrain surface tile's decoration layer so
it lies in the ground like litter, never as a floating sprite. No growth animation:
germination replaces the mark with the newborn plant.

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live banked seed cohorts, not growing seedlings (source: `crates/cubarium-voxel-flora/src/lib.rs:259`)
- life_stages: arrival → dormant bank → armed germination → newborn, or attrition to litter (sources: `crates/cubarium-voxel-flora/src/lib.rs:259`; `crates/cubarium-voxel-flora/src/lib.rs:727`)
- mark: recent arrival (seed_mark_recent_s 120 s) or armed germination while bank still holds seed (source: `crates/cubarium-voxel-flora/src/lib.rs:727`)
- dispersal: drop, wind/Wald, spores, water, runners, animal burrs and fruit gut lots per species (sources: `crates/cubarium-voxel-flora/src/lib.rs:866`; `crates/cubarium-voxel-flora/src/lib.rs:2198`; `crates/cubarium-voxel-fauna/src/body.rs:1109`)

---

## D6 · Springturf (*Caespes primus*, "springturf")

*Niche (revised 2026-10-03, Wrysk: "springturf should really be a wild grass. think,
rolling hills of california along with the wildflowers. maybe even drought resistant
too").* The **wild grass of open hills**: it holds open, lit, drained ground in sweeping
stands, the way grass and wildflowers cover rolling hills.
- **Drought-resistant:** a low wilting point and store-funded curing/dormancy, with
  regrowth when water returns. The deeper-root possibility is *not in the sim yet*
  as a species distinction: `rooting_depth` remains 2.
- **The world's main grazing crop:** a large standing crop that regrows fast from its
  basal buds after cropping. It's the grazer's staple and the thing that visibly gets
  cropped.
- **Heavy thatch:** it sheds plenty of litter, which feeds the decomposers.
- **Seeds:** abundant, with a long-lived bank.
- **Succession:** it comes up through velvetpad's moss on open ground and replaces it
  (moss first, then grass, then the woody plants). Woody seedlings come up through it in
  turn.

Its crop, foliage and litter are a different class from velvetpad's: tall, abundant and
fast-renewing, against the moss's thin, slow carpet. The numbers come from the
succession study (`design/plant-succession-2026-10-03.md`).

*Look, open for Wrysk:* the art below predates the wild-grass role. Two questions are open:
- whether the clump should read taller, more like a meadow;
- whether a dry-season state (golden or tawny in a drought) belongs in the states list.

*Size under the ladder.* Crown 1–1.5 voxels tall, radius 1–2: a clump **2–4 voxels
wide, 6–9 px tall**, about three times as wide as it is tall, low to the ground.

**Selected appearance (2026-09-24):** [springturf concept](../../art/gen/artbook/springturf.png),
a low spreading clump. Use its silhouette and colour balance as the visual
reference when revisiting the exact leaf pattern below.

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:2198`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- food: photosynthesis, root water and soluble mineral (source: `crates/cubarium-voxel-flora/src/lib.rs:2198`)
- habitat: sunlit drained ground; establishment light ≥0.75, preset available water ≥0.2; drought curing/dormancy lives on stores (source: `crates/cubarium-voxel-flora/src/lib.rs:2198`)
- size: authored crown height × diameter 0.125–0.1875 × 0.25–0.5 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:2198`)
- life_stages: seed bank → living stand; morphology profiles by wood, dieback/death by stores and stress (sources: `crates/cubarium-voxel-flora/src/lib.rs:2198`; `crates/cubarium-voxel-flora/src/lib.rs:1585`; `crates/cubarium-voxel-flora/src/step.rs:3768`)
- breeding: Wald wind seeds, burrs on animals and runners; preset clonal share 0.6/reach 3; overtops velvetpad; paid parcels and species-specific seed-bank clocks; preset rates override defaults (sources: `crates/cubarium-voxel-flora/src/lib.rs:2198`; `config/desktop/terrarium.toml` → `flora.springturf`)
- consumers: frondgrazer and loftstrider within their mouth bands (sources: `crates/cubarium-voxel-fauna/src/body.rs:1846`; `crates/cubarium-voxel-fauna/src/body.rs:2099`; `crates/cubarium-voxel-flora/src/lib.rs:2198`)
- defence: induced grazing defence and a refuge floor (sources: `crates/cubarium-voxel-flora/src/lib.rs:2198`; `config/desktop/terrarium.toml` → `flora.springturf.defence_half_life_s`)
- movement_senses: sessile; responds to light/water/stress; dispersal is not locomotion (source: `crates/cubarium-voxel-flora/src/lib.rs:2198`)
- dormancy: drops 0.8 foliage share; preset cure_after_s 1097.825, wakes at available water 0.25, reduced upkeep 0.1; cure_max_s 0 removes a fixed time limit (sources: `config/desktop/terrarium.toml` → `flora.springturf.cure_after_s`; `crates/cubarium-voxel-flora/src/lib.rs:2260`)
- not_in_sim_yet: deeper drought roots as distinct from the other two-layer-root ground covers (retained D6 intent; source: `crates/cubarium-voxel-flora/src/lib.rs:2198`, rooting_depth remains 2)

### Body plan, primary: "pleated tuft"

Not blades. Each plant is a dense fan of **pleated, ribbon-like leaves** rising from a
buried crown, every leaf a flat strap 1 px wide that widens to 2 px at a rounded,
slightly cupped tip; the leaves lean outward from the centre so the clump is a
low half-ellipse with a scalloped top edge. In front of the tall leaves stands a second,
shorter rank of leaves in the darker step, overlapping their bases. Leaves producer-blue `#1E2798` at the base
ramping to `#42C5F8` at the tips (two steps: `#1E2798`, `#2B6AD0`, `#42C5F8`). A young
clump has 5 leaves in a 2-voxel width; a full clump 11 leaves across 4 voxels, with a
second, shorter rank of leaves in front in the darker step.

**Clump-pattern sheets** (decision 3): three authored arrangements per size, `A`
symmetric, `B` leaning left with a gap, `C` two tufts joined; the presenter picks by a
site hash so a lawn is many clumps and one texture. Neighbouring springturf clumps are
drawn touching so their outer leaves overlap.

States:

- **cropped** (fullness thinned): the tips of the tallest leaves are missing, ending in
  a flat `#2B6AD0` cut 1 px wide; the scalloped top becomes a level, torn line. This is
  the "visible cropped margin" the biosphere doc asks for.
- **bare**: three stubs 2 px tall.
- **wilting**: the leaves lean 30° further outward and lose the top colour step.
- **ripe**: the clump's centre shows a 2-px `#B99BE6` bud (the paid package).

*Alternates.* **(B) Bead turf**: leaves are strings of 1-px beads; cropped removes
beads. **(C) Tongue rosette**: five broad flat tongues lying near the ground; cropped
tongues show a bite notch. B is the most alien; A reads "grass-like lawn" fastest
without being grass.

*Sheet plan.* 4 sizes × 3 patterns × 3 fullness × 2 water; sway 4 f. ≈ 72 tiles of at
most 24 × 9 px.

---

## D7 · Velvetpad (*Stratum umbrae*, "velvetpad")

*Niche (revised 2026-10-03, Wrysk: "its really more of a mossy undergrowth / early
colonizer").* **Mossy undergrowth and the early colonizer.**
- **Bare ground:** its spores reach new ground first, lit or shaded, and carpet it fast.
- **Open, drained ground:** the wild grass (springturf) comes up through the moss and
  replaces it.
- **Where it stays:** in shade and damp, under ledges and crowns, where the grass can't
  follow.
- **Low and thin:** a small crop and little litter. It's a minor food beside the
  grass, more cover than pasture.

*Size.* Crown 1 voxel tall, radius 1.5–3: **3–6 voxels wide, 6 px tall**.

**Provisional selected appearance (2026-09-24):** [velvetpad concept](../../art/gen/artbook/velvetpad-provisional.png).
An indefinitely spreading violet carpet has a raised purple perimeter, irregular
cobalt mossy tufts, thin cyan branching channels, and scattered smooth round
blue-to-cyan young buds. The regular brick-pattern nodules below are superseded
for visual design. The bud treatment is still open to refinement.

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:2446`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- food: photosynthesis, root water and soluble mineral (source: `crates/cubarium-voxel-flora/src/lib.rs:2446`)
- habitat: damp lit or shaded ground, saturation tolerated at establishment; early colonizer displaced by sun-loving turf (source: `crates/cubarium-voxel-flora/src/lib.rs:2446`)
- size: authored crown height × diameter 0.125 × 0.375–0.75 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:2446`)
- life_stages: seed bank → living stand; morphology profiles by wood, dieback/death by stores and stress (sources: `crates/cubarium-voxel-flora/src/lib.rs:2446`; `crates/cubarium-voxel-flora/src/lib.rs:1585`; `crates/cubarium-voxel-flora/src/step.rs:3768`)
- breeding: spores (2 attempts), local runners (clonal share 0.3); not shade-exclusive; paid parcels and species-specific seed-bank clocks; preset rates override defaults (sources: `crates/cubarium-voxel-flora/src/lib.rs:2446`; `config/desktop/terrarium.toml` → `flora.velvetpad`)
- consumers: frondgrazer and loftstrider within their mouth bands (sources: `crates/cubarium-voxel-fauna/src/body.rs:1846`; `crates/cubarium-voxel-fauna/src/body.rs:2099`; `crates/cubarium-voxel-flora/src/lib.rs:2446`)
- defence: induced grazing defence and a refuge floor (sources: `crates/cubarium-voxel-flora/src/lib.rs:2446`; `config/desktop/terrarium.toml` → `flora.velvetpad.defence_half_life_s`)
- movement_senses: sessile; responds to light/water/stress; dispersal is not locomotion (source: `crates/cubarium-voxel-flora/src/lib.rs:2446`)

### Body plan, primary: "carpet autotile"

The pad is not a plant sprite; it is a **surface** drawn as an autotile over the ground
under a crown. *Framing for generation:* tile, not sprite: "a flat patch of ground
surface seen from slightly above, its raised lip along the near edge, filling the lower
half of the frame with empty space above it". Its texture: close-set rounded **nodules** 2 × 2 px in a brick pattern,
`#248CA8` (a teal step added to the producer ramp, so shade growth reads as a different
green-blue from the sun plants), with `#1E2798` between the nodules and a 1-px `#42C5F8`
glint on every fourth nodule. The edge tile has a raised lip 1 px lighter, so the carpet
has a thickness. Inner, edge, corner and lip-under-rock tiles.

States: **thinned**: nodules spaced, `#1E2798` dominant. **wilting**: nodules flatten
to 2 × 1 and the glints go. **ripe**: a scattering of `#B99BE6` spore-caps 1 px on the
nodules. Size: coverage radius, not a taller sprite.

*Alternate.* **(B) Fringe pad**: a fringe of 2-px tongues around a flat centre, drawn
per plant. Reads as an individual; less like a carpet.

---

## D8 · Stonecushion (*Pulvinus saxi*, "stonecushion")

*Niche.* Bare rock and bedrock faces with a soil pocket in the root box; content on a
small available-water threshold; slow structural growth, with no fixed full-size
clock. It tolerates drained ground but dies under shallow standing water. The old
11,500 s estimate was conditional on rates and resources, not a life stage.

*Size.* Crown 1–1.5 voxels tall, radius 1–1.25: **2–2.5 voxels wide, 6–9 px tall**.

**Selected appearance (2026-09-24):** [stonecushion concept](../../art/gen/artbook/stonecushion.png).
Its compact alien mound is made of several plump lilac lobes; the tightly packed
miniature rosettes below are not the current visual target.

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:2350`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- food: photosynthesis, root water and soluble mineral (source: `crates/cubarium-voxel-flora/src/lib.rs:2350`)
- habitat: rock/bedrock support with reachable soil water; establishment available water ≥0.05, light ≥0.4, drowning threshold 0.02 m (source: `crates/cubarium-voxel-flora/src/lib.rs:2350`)
- size: authored crown height × diameter 0.125–0.1875 × 0.25–0.3125 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:2350`)
- life_stages: seed bank → living stand; morphology profiles by wood, dieback/death by stores and stress (sources: `crates/cubarium-voxel-flora/src/lib.rs:2350`; `crates/cubarium-voxel-flora/src/lib.rs:1585`; `crates/cubarium-voxel-flora/src/step.rs:3768`)
- breeding: wind seeds, low growth rates; old 11500 s full-size estimate is not a fixed clock; paid parcels and species-specific seed-bank clocks; preset rates override defaults (sources: `crates/cubarium-voxel-flora/src/lib.rs:2350`; `config/desktop/terrarium.toml` → `flora.stonecushion`)
- consumers: frondgrazer and loftstrider within their mouth bands (sources: `crates/cubarium-voxel-fauna/src/body.rs:1846`; `crates/cubarium-voxel-fauna/src/body.rs:2099`; `crates/cubarium-voxel-flora/src/lib.rs:2350`)
- defence: induced grazing defence and a refuge floor (sources: `crates/cubarium-voxel-flora/src/lib.rs:2350`; `config/desktop/terrarium.toml` → `flora.stonecushion.defence_half_life_s`)
- movement_senses: sessile; responds to light/water/stress; dispersal is not locomotion (source: `crates/cubarium-voxel-flora/src/lib.rs:2350`)

### Body plan, primary: "rock-lip dome" (drawn with its rock)

A compact **dome of tight rosettes**, each rosette a 2-px knot of `#B99BE6` pale lilac
with a `#3A1A7A` centre, packed so the dome's surface is a mosaic; the dome's base
spreads a 1-px `#510B6D` root-mat over the rock lip it sits on, reaching into a crack
drawn as a 1-px `#12093A` line. The sheet is authored **with the rock lip** (decision
3's cohesion rule): the tile includes 2 px of the rock face's edge under the dome so the
plant is never a dome floating on a cliff.

States: **thinned** (rare: shade): rosettes open and darker. **wilting**: the sim can
make it wilt below pore 0.02; the dome's rosettes close to 1-px points. **ripe**: two
rosettes at the top turn `#42C5F8`. Size: 3 → 7 rosettes across.

*Alternate.* **(B) Barnacle cluster**: five conical plates with a slit at each apex.

---

## D9 · Bloomcrown (*Corona solis*, "bloomcrown")

*Niche.* The sun producer of ridges and terraces: light half-saturation 0.8, will not
germinate a quarter waterlogged, dies in a pool, shrugs off a shower; wood to 0.6; the
grazer’s reachable foliage food; it now pays nectar into a separate stock for
bellwings. Pollen carriage and visit-dependent outcrossing are *not in the sim yet*.

*Size under the ladder.* Crown 3–8 voxels tall, radius 1–2.5: **stem plus crown 18–48
px tall, crown 12–30 px wide**; the whole plant is about twice as tall as the crown is
wide, the stem the lower half.

**Selected alternatives (2026-09-24):** keep both the [preferred pink crown](../../art/gen/artbook/bloomcrown-pink-preferred.png)
and [original blue crown](../../art/gen/runs/2026-09-24-final-pilot/bloomcrown-01.png).
The preferred version has more than six hot-pink upper vanes, a pale blush-lilac
central pod and warm orange tip, retaining its violet stem and basal leaf colours.
The exact crown layout and stem/base adjustment are not locked.

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:1942`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- food: photosynthesis, root water and soluble mineral (source: `crates/cubarium-voxel-flora/src/lib.rs:1942`)
- habitat: open bright ground; light_half 0.8, establishment light ≥0.6, available water ≥0.1, saturated share ≤0.25; pool threshold 0.05 m (source: `crates/cubarium-voxel-flora/src/lib.rs:1942`)
- size: authored crown height × diameter 0.375–1.0 × 0.25–0.625 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:1942`)
- life_stages: seed bank → living stand; morphology profiles by wood, dieback/death by stores and stress (sources: `crates/cubarium-voxel-flora/src/lib.rs:1942`; `crates/cubarium-voxel-flora/src/lib.rs:1585`; `crates/cubarium-voxel-flora/src/step.rs:3768`)
- breeding: drop dispersal, woody recruitment through ground cover; separate nectar stock; paid parcels and species-specific seed-bank clocks; preset rates override defaults (sources: `crates/cubarium-voxel-flora/src/lib.rs:1942`; `config/desktop/terrarium.toml` → `flora.bloomcrown`)
- consumers: frondgrazer and loftstrider within their mouth bands (sources: `crates/cubarium-voxel-fauna/src/body.rs:1846`; `crates/cubarium-voxel-fauna/src/body.rs:2099`; `crates/cubarium-voxel-flora/src/lib.rs:1942`)
- defence: induced grazing defence and a refuge floor (sources: `crates/cubarium-voxel-flora/src/lib.rs:1942`; `config/desktop/terrarium.toml` → `flora.bloomcrown.defence_half_life_s`)
- movement_senses: sessile; responds to light/water/stress; dispersal is not locomotion (source: `crates/cubarium-voxel-flora/src/lib.rs:1942`)
- nectar: paid nectar_per_s 0.0001, nectar_cap 0.02 org-units; nectar_mineral_fraction 0.01; reproductive parcel is a separate store (source: `crates/cubarium-voxel-flora/src/lib.rs:2026`)
- not_in_sim_yet: visit-dependent outcrossed seed/pollen transfer (source: `crates/cubarium-voxel-fauna/src/lib.rs:981`)

### Body plan, primary: "vaned crown"

A single straight **stem** (1 voxel, 6 px thick at the base tapering to 3 px), mid
violet `#3A1A7A` with a `#2A0E4A` shaded left edge and a row of 1-px `#42C5F8` nodes
every 6 px (a growth mark: one node per size step). At the top, the **crown**: not a
disc but a **ring of six upright vanes**, each vane a narrow flat blade 2 px wide, straight-sided, curving
outward and up, with open space between one vane and the next, `#1E2798` at the base ramping to `#42C5F8` at
the upper edge; between the vanes, the crown's **core**, a rounded mass `#2B6AD0` with a
1-px `#B99BE6` top. Foliage is the vanes: a full crown has all six and a second inner
ring of four; thinned has four outer vanes with gaps; bare has the core alone on the
stem.

**Stand-group variants**: two authored group sprites (two stems, three stems of mixed
size rising separately from one patch of ground, ground showing between the stems) chosen when neighbouring bloomcrown stands are within one voxel,
so a meadow shows clumps, not a lattice.

States: **wilting**: the vanes droop outward to 45°, the core darkens to `#1E2798`.
**ripe**: the core turns the warm accent `#FF9B50` (the bloom; this is the one warm
accent among the plants and it is a real state: a full parcel). **senescent / dieback**:
the lowest nodes go dark and the stem gains a 1-px `#510B6D` band per lost size step.
Sway: the crown rocks 1 px, the stem bends from its middle node.

*Alternates.* **(B) Disc crown** (today's presenter idea): a flat luminous disc on a
stem, foliage as disc radius. Simplest; least alien. **(C) Bell crown**: a downward
bell of overlapping scales with the core hanging inside; ripe shows the core glowing
through the scales.

*Sheet plan.* 4 sizes × 3 fullness × 2 water × 2 ripe, plus 2 group sprites × 3 sizes;
sway 4 f; grow blends. Cutout rig: stem (3 segments), core, vanes ×6 (+4 inner).

---

## D10 · Umbrellafrond (*Pluvia tecta*, "umbrellafrond")

*Niche.* The wet producer of hollows: light half-saturation 0.15 and deep roots (4).
It accepts saturated establishment but has nonzero aeration stress and a 0.5 m
standing-water drowning threshold; absolute saturation immunity is *not in the sim
yet* and remains an earlier intent. Its canopy shades undergrowth, including velvetpad.

*Size under the ladder.* Crown 8–16 voxels tall, radius 3–6: **48–96 px tall, crown
36–72 px wide**; the plant is a third taller than it is wide, the stem showing below
the lowest tier. The tallest live plant until the vaulttree.

**Selected appearance (2026-09-24):** [umbrellafrond concept](../../art/gen/artbook/umbrellafrond.png).
The crown uses three regular, nearly geometric tiers of straight lacy teal fronds
with cyan ribs over a violet stem. Its ordered symmetry and distinct frond
structure are intentional; the drooping strap fronds below are the earlier plan.

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:2036`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- food: photosynthesis, root water and soluble mineral (source: `crates/cubarium-voxel-flora/src/lib.rs:2036`)
- habitat: wet hollows; deep rooting_depth 4; establishment available water ≥1.2, light ≥0.1; saturation accepted but stress_rate_per_s 0.01, drown_depth_m 0.5 (source: `crates/cubarium-voxel-flora/src/lib.rs:2036`)
- size: authored crown height × diameter 1.0–2.0 × 0.75–1.5 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:2036`)
- life_stages: seed bank → living stand; morphology profiles by wood, dieback/death by stores and stress (sources: `crates/cubarium-voxel-flora/src/lib.rs:2036`; `crates/cubarium-voxel-flora/src/lib.rs:1585`; `crates/cubarium-voxel-flora/src/step.rs:3768`)
- breeding: spores and runners, clonal share 0.5; woody recruitment above ground cover; paid parcels and species-specific seed-bank clocks; preset rates override defaults (sources: `crates/cubarium-voxel-flora/src/lib.rs:2036`; `config/desktop/terrarium.toml` → `flora.umbrellafrond`)
- consumers: frondgrazer and loftstrider within their mouth bands (sources: `crates/cubarium-voxel-fauna/src/body.rs:1846`; `crates/cubarium-voxel-fauna/src/body.rs:2099`; `crates/cubarium-voxel-flora/src/lib.rs:2036`)
- defence: induced grazing defence and a refuge floor (sources: `crates/cubarium-voxel-flora/src/lib.rs:2036`; `config/desktop/terrarium.toml` → `flora.umbrellafrond.defence_half_life_s`)
- movement_senses: sessile; responds to light/water/stress; dispersal is not locomotion (source: `crates/cubarium-voxel-flora/src/lib.rs:2036`)

### Body plan, primary: "layered umbrella"

A thick **stem** (1.5 voxels, 9 px at the base) `#3A1A7A` with a `#2A0E4A` shaded side
and horizontal `#2A0E4A` bands every 8 px (leaf scars). The crown is **two or three
tiers of drooping fronds**: each tier a few fronds radiating from the stem with open sky showing
between one frond and the next, each
frond a tapering strap 3 px wide that arches out and down, its underside `#1E2798`, its
upper surface `#2B6AD0`, a 1-px `#42C5F8` midrib. The top tier is smallest and points
up; the lowest tier reaches down to a third of the plant's height. Between tiers the
stem shows. Seen from the 30° camera the tiers overlap into a broad, layered umbrella
with a scalloped lower edge.

States: **thinned** (shade or grazing of its low fronds): the lowest tier loses fronds
to 3, gaps show. **bare**: stem and midribs only. **wilting** (dry root zone): all
fronds droop to vertical, colour drops one step. **waterlogged** does not exist for this
species (immune) and is not drawn. **ripe**: the top tier's midribs go full `#42C5F8`
and a 2-px `#B99BE6` spadix stands from the crown's centre. Sway: tiers lag the stem;
the lowest tier swings 2 px.

*Alternates.* **(B) Single umbrella**: one broad canopy disc with a scalloped rim on a
tall stem (today's name literalised). **(C) Curtain frond**: a stem carrying a single
hanging curtain of fronds on one side, like a banner; the alien option.

*Sheet plan.* 4 sizes × 3 fullness × 2 water × 2 ripe; sway 6 f. Rig: stem (4
segments), tier ×3, frond ×15.

---

## Roles introduced since the original dossier

Vaulttree, lanternberry and siphonreed are implemented stand species; bellwing is
an implemented animal founder. The retained model requests record earlier intent;
Sim state below records what is present at this sync, without deciding the look.

## D11 · Vaulttree (*Arcus caeli*, "vaulttree")

*Niche (biosphere §4).* The canopy builder: deep aerated soil with enough water, needs
a gap to establish (a light gate above bloomcrown's), slow costly structure, long life,
large reserve. Its death opens a light gap and already lays a **line of dead wood**: the fall
length follows crown radius and voxel size, with a keyed direction and conserved
shares. `hop` instead controls dispersal reach.

*Size.* Crown 18–28 voxels tall, radius 5–8: **108–168 px tall, crown 60–96 px wide**;
nearly twice as tall as it is wide, rendered on a tall canvas.

**Selected appearance (2026-09-24):** [vaulttree concept](../../art/gen/artbook/vaulttree.png).
An asymmetric plated violet trunk carries open arching limbs at different depths.
The canopy forms separated dark-teal layered shelves with bright mint rims, and
many fine cyan tendrils hang far down the trunk. The earlier rounded green lobes
and two short drapes below are no longer the visual target.

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:2720`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- food: photosynthesis, root water and soluble mineral (source: `crates/cubarium-voxel-flora/src/lib.rs:2720`)
- habitat: lit gaps, deep aerated soil: rooting_depth 6, establishment light ≥0.9 and saturated share ≤0.2 (source: `crates/cubarium-voxel-flora/src/lib.rs:2720`)
- size: authored crown height × diameter 2.25–3.5 × 1.25–2.0 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:2720`)
- life_stages: seed bank → living stand; morphology profiles by wood, dieback/death by stores and stress (sources: `crates/cubarium-voxel-flora/src/lib.rs:2720`; `crates/cubarium-voxel-flora/src/lib.rs:1585`; `crates/cubarium-voxel-flora/src/step.rs:3768`)
- breeding: wind seed, woody recruitment through cover; fall deposits conserved wood over a site line; paid parcels and species-specific seed-bank clocks; preset rates override defaults (sources: `crates/cubarium-voxel-flora/src/lib.rs:2720`; `config/desktop/terrarium.toml` → `flora.vaulttree`)
- consumers: frondgrazer and loftstrider within their mouth bands (sources: `crates/cubarium-voxel-fauna/src/body.rs:1846`; `crates/cubarium-voxel-fauna/src/body.rs:2099`; `crates/cubarium-voxel-flora/src/lib.rs:2720`)
- defence: induced grazing defence and a refuge floor (sources: `crates/cubarium-voxel-flora/src/lib.rs:2720`; `config/desktop/terrarium.toml` → `flora.vaulttree.defence_half_life_s`)
- movement_senses: sessile; responds to light/water/stress; dispersal is not locomotion (source: `crates/cubarium-voxel-flora/src/lib.rs:2720`)
- fall: line length follows current crown radius / voxel size, keyed direction; not hop count (source: `crates/cubarium-voxel-flora/src/step.rs:3859`)

### Body plan, primary: "vault"

A **trunk** 2 voxels thick (12 px) `#2A0E4A` with `#3A1A7A` lit face, bark in long
plates with `#12093A` seams, rising alone through the whole lower half of the plant, then
splitting into **a few arching limbs** with sky between them that curve outward and up, each limb 4
px thick tapering to 2. The crown is **lobes**: each limb ends in a rounded foliage
lobe 12–20 px across, `#1E2798` underside, `#2B6AD0` body, `#42C5F8` upper rim 1 px;
lobes are separated by **gaps of sky**, the vault's signature, so the crown never reads
as a solid disc. From the lowest limbs hang two **drapes** of 1-px `#248CA8` filaments
6–10 px long (the hanging growth the art direction asks for; decoration, fixed).

States: **thinned**: lobes shrink toward their limbs, gaps widen. **bare**: limbs only,
a bare vault. **wilting**: lobes droop below their limbs. **ripe**: a 3-px `#B99BE6`
cluster at the tip of the top limb. **senescent**: the lowest limb goes bare first.
Dieback of a big tree is visible as one lobe at a time lost.

*Alternates.* **(B) Column tree**: a tall column with foliage in horizontal shelves.
**(C) Buttress tree**: three fused trunks flaring at the base into buttresses, one
crown; the fall (dead wood line) is visually anticipated by the flare.

*Model.* Implemented preset: establishment light ≥0.9, wood_max 5, large reserve
and the dead-wood line at death. The retained crown design is described above;
Sim state gives the authored physical envelope.

## D12 · Lanternberry (*Lucifructus*, "lanternberry")

*Niche.* Fruiting shrub of grove edges and bright moist gaps; repeated paid fruit
crops; fruit is the seedporter’s food and seeds travel in its gut. `fruit` is now
a paid stock separate from the reproductive `parcel`; uneaten crops fall to litter
and the stand rests before regrowing another crop.

*Size.* Crown 5–9 voxels tall, radius 2–3.5: **30–54 px tall, 24–42 px wide**.

**Selected appearance (2026-09-24):** [lanternberry concept](../../art/gen/artbook/lanternberry-blue.png).
Return to blue foliage on violet stems, with deep-plum lanterns, magenta seams,
and warm orange interiors. The later acid-lime foliage pass was rejected here.

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:2826`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- food: photosynthesis, root water and soluble mineral (source: `crates/cubarium-voxel-flora/src/lib.rs:2826`)
- habitat: bright moist gaps; establishment water ≥0.4, light ≥0.35; wilt/full available water 0.1/0.5 (source: `crates/cubarium-voxel-flora/src/lib.rs:2826`)
- size: authored crown height × diameter 0.625–1.125 × 0.5–0.875 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:2826`)
- life_stages: seed bank → living stand; morphology profiles by wood, dieback/death by stores and stress (sources: `crates/cubarium-voxel-flora/src/lib.rs:2826`; `crates/cubarium-voxel-flora/src/lib.rs:1585`; `crates/cubarium-voxel-flora/src/step.rs:3768`)
- breeding: drop seeds and separate paid fruit crops; seedporter gut dispersal; paid parcels and species-specific seed-bank clocks; preset rates override defaults (sources: `crates/cubarium-voxel-flora/src/lib.rs:2826`; `config/desktop/terrarium.toml` → `flora.lanternberry`)
- consumers: frondgrazer and loftstrider within their mouth bands (sources: `crates/cubarium-voxel-fauna/src/body.rs:1846`; `crates/cubarium-voxel-fauna/src/body.rs:2099`; `crates/cubarium-voxel-flora/src/lib.rs:2826`)
- defence: induced grazing defence and a refuge floor (sources: `crates/cubarium-voxel-flora/src/lib.rs:2826`; `config/desktop/terrarium.toml` → `flora.lanternberry.defence_half_life_s`)
- movement_senses: sessile; responds to light/water/stress; dispersal is not locomotion (source: `crates/cubarium-voxel-flora/src/lib.rs:2826`)
- fruit: fruit_per_s 0.0001, fruit_cap 0.02 org-units; uneaten ripe crop falls at fruit_drop_s 1800, mean fruit_rest_s 900 before regrowing; fruit is separate from parcel (source: `crates/cubarium-voxel-flora/src/lib.rs:2859`)

### Body plan, primary: "bell-lantern shrub"

Three to five **stems** from one base, each 3 px thick `#3A1A7A`, spreading in a fan;
foliage is tiny **leaflets**, each a 2 × 1 px dash `#2B6AD0`, spaced along the upper
halves of the stems with bare stem showing between them; from the stem tips hang the **lanterns**: closed bells 3 × 4 px, `#2A0E4A` with
a 1-px `#FF2AFC` magenta seam, one per stem tip. **ripe**: the bells open into 5 × 5
px hanging lanterns with a `#FF9B50` warm interior seen through the opening (the second
warm-accent plant, fruit only). Fruit fall (a spent parcel) closes the bell again.

*Alternates.* **(B) Pod shrub**: pods along the stems instead of at the tips. **(C)
Coil shrub**: a single coiled stem with lanterns at each coil.

*Model.* Implemented preset and separate fruit product. The earlier parcel-as-fruit
shortcut is retained in the historical sheet proposal above, not the current ecology.

## D13 · Siphonreed (*Fistula stagni*, "siphonreed")

*Niche.* Emergent of saturated soil and shallow standing water: needs free water on or
beside its site in the earlier niche proposal. The implemented gate requires
at least 0.1 m of settled water **on its own site**, and it has a low but nonzero
aeration stress response, like umbrellafrond. Adjacent-water-only establishment
is *not in the sim yet*. The retained look is narrow upright clumps at the shore.

*Size.* Crown 6–12 voxels tall, radius 0.5–1: **36–72 px tall, 6–12 px wide**, in
clumps; six times as tall as wide, rendered on a tall canvas.

**Selected appearance (2026-09-24):** [siphonreed concept](../../art/gen/artbook/siphonreed.png).
Four slim celadon stems have swollen upper shafts and broad rounded pink fan
heads, without side nubs. This replaces the narrow ringed siphon heads below
as the visual target; clump counts at different growth stages remain open.

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- status: live founder species on eligible sites; counts are area/gate/cap dependent (source: `crates/cubarium-voxel-flora/src/lib.rs:2915`, `founder_per_m2`, `founder_colony`, `founder_cap`)
- food: photosynthesis, root water and soluble mineral (source: `crates/cubarium-voxel-flora/src/lib.rs:2915`)
- habitat: wet sunlit shallows; settled water depth at its own site ≥0.1 m; establishment available water ≥1.2/light ≥0.6, drown_depth_m 0.5 (source: `crates/cubarium-voxel-flora/src/lib.rs:2915`)
- size: authored crown height × diameter 0.75–1.5 × 0.125–0.25 m; profile may cap seedlings below these envelopes (source: `crates/cubarium-voxel-flora/src/lib.rs:2915`)
- life_stages: seed bank → living stand; morphology profiles by wood, dieback/death by stores and stress (sources: `crates/cubarium-voxel-flora/src/lib.rs:2915`; `crates/cubarium-voxel-flora/src/lib.rs:1585`; `crates/cubarium-voxel-flora/src/step.rs:3768`)
- breeding: water-dispersed seeds and runners, clonal share 0.7; paid parcels and species-specific seed-bank clocks; preset rates override defaults (sources: `crates/cubarium-voxel-flora/src/lib.rs:2915`; `config/desktop/terrarium.toml` → `flora.siphonreed`)
- consumers: frondgrazer and loftstrider within their mouth bands (sources: `crates/cubarium-voxel-fauna/src/body.rs:1846`; `crates/cubarium-voxel-fauna/src/body.rs:2099`; `crates/cubarium-voxel-flora/src/lib.rs:2915`)
- defence: induced grazing defence and a refuge floor (sources: `crates/cubarium-voxel-flora/src/lib.rs:2915`; `config/desktop/terrarium.toml` → `flora.siphonreed.defence_half_life_s`)
- movement_senses: sessile; responds to light/water/stress; dispersal is not locomotion (source: `crates/cubarium-voxel-flora/src/lib.rs:2915`)

### Body plan, primary: "siphon stems"

A few **hollow stems**, each 2 px thick, standing apart with open water showing
between one stem and the next, `#1E2798` below the water line and
`#2B6AD0` above, each ending in a **siphon head**: a 3-px swelling with a 1-px `#42C5F8`
ring (the aeration organ); stems lean in the sway. The clump's sheet is authored **with
the water line**: the tile includes the water skin crossing the stems, so where the
water is deeper (the site's free water) the presenter picks the variant with the line
higher. Ripe: the tallest head opens into a 3 × 3 `#B99BE6` tuft.

*Alternates.* **(B) Flag reed**: one broad flag leaf per stem. **(C) Bladder reed**: a
floating bladder at the base of each stem.

*Model.* Implemented preset with a positive water-depth requirement at the site.
The retained request is: the sway should read the site’s free water as a slow drift
(*not in the sim yet*).

## D14 · Bellwing (*Campanula volans*, "bellwing")

**Selected appearance (2026-09-25):** [bellwing with thin corkscrew siphon](../../art/gen/artbook/bellwing.png).
Keep the hanging plum bell, magenta open rim/underside, and four lilac vanes with
cyan leading edges. The feeding siphon is a fine round tendril whose distal end
makes a spindly three-dimensional corkscrew, not a flat spiral, broad ribbon or
droplet tip. The selected tendril is muted peach; Wrysk requests a little more
yellow in a later pass, without returning to the strongly saturated yellow/orange
versions. This image has verified alpha transparency. The earlier poses below
remain animation proposals, not rendered states.

*Niche.* Nectar, carrying a little mineral, funds its small body and costly flight.
It flies, hovers, perches and sips bloomcrown or latticevine flowers, following
nectar scent and near-IR/cyan beacons and learning unpaying visits. Flowers reflect
near-IR from the red dwarf sun, bright by day and dark at night; the lanternjaw
emits near-IR alongside cyan to mimic that flower signal. The bellwing's
photoreceptor sees near-IR plus cyan. Its activity extends into dusk/dawn; heavy rain grounds it. Pollen as a separate nutrient food, carriage
and outcrossed seed are *not in the sim yet*. The last visited bloom is recorded
as a future seam. Both ground hunters and seedporters can catch reachable bellwings.

*Size.* Body 1 voxel, wingspan 3 (18 px); flies 4–20 voxels up.

### Sim state

- synced_against: `e66b92138e8a3aab6c5b15729b7195a0911f18a1` (source: Git HEAD at review, 2026-10-08)
- code_id: `bellwing`; display name Bellwing (sources: `crates/cubarium-voxel-fauna/src/body.rs:2167`; `crates/cubarium-observatory/src/query.rs:20`)
- status: live founder in the desktop terrarium: 6 bodies; code default 0 (sources: `config/desktop/terrarium.toml` → `fauna.founders.bellwing.founding_count`; `crates/cubarium-voxel-fauna/src/body.rs:2167`)
- diet: nectar from bloomcrown and latticevine; mineral rides in nectar, not a separate pollen meal (source: `crates/cubarium-voxel-fauna/src/body.rs:2167`)
- size: adult length × width × height 0.125 × 0.125 × 0.0625 m; juvenile dimensions scale with cube root of body / body_max (sources: `crates/cubarium-voxel-fauna/src/body.rs:2167`; `crates/cubarium-voxel-fauna/src/body.rs:2510`)
- body: core.body_min 0.003, breeding threshold 0.015, full size 0.03 org-units in the desktop preset; shipped mass scale 30 × placeholder quantities, not 30 × physical dimensions (sources: `config/desktop/terrarium.toml` → `fauna.founders.bellwing.core`; `crates/cubarium-voxel-fauna/src/body.rs:2167`; `crates/cubarium-voxel-fauna/src/lib.rs:2308`)
- activity: diurnal with 1 world-clock hour before sunrise and after sunset (sources: `crates/cubarium-voxel-fauna/src/body.rs:2167`; `crates/cubarium-voxel-fauna/src/body.rs:2167`)
- life_stages: juvenile → adult; eggs are counted separately; juvenile/adult boundary is core.birth_body, not full adult size (source: `crates/cubarium-observatory/src/query.rs:342`)
- lifespan: median 43200 s (6 reference days at 7200 s/day); each body draws its lifespan (sources: `fauna.founders.bellwing.core.lifespan_median_s` in `design/parameters.md`; `crates/cubarium-voxel-fauna/src/body.rs:143`; history `3021eea9`)
- breeding: surplus-sized litter-site clutches up to 6; checks hatchling wing clearance and open air; preset incubation ≈182.97 s, surplus hold ≈182.97 s (sources: `config/desktop/terrarium.toml` → `fauna.founders.bellwing.core.reproduction`; `crates/cubarium-voxel-fauna/src/body.rs:2167`)
- senses: near-IR/cyan beacon sense to 4 m, nectar smell, looming to 1 m and perch tremor; learns failed/unpaying beacons (source: `crates/cubarium-voxel-fauna/src/body.rs:5987`)
- movement_habitat: fly, hover, perch and ground rest; span 0.375 m, height band max 2.5 m; heavy rain grounds it (source: `crates/cubarium-voxel-fauna/src/body.rs:2167`)
- defence: loom escape, flight and learned beacon avoidance (source: `crates/cubarium-voxel-fauna/src/body.rs:6064`; history `2a4579ff`)
- predators: lanternjaw, Veilhound and seedporter when perched, grounded or hovering within strike reach; actual eligibility also reads size and mobility (source: `crates/cubarium-voxel-fauna/src/predation.rs:94`)
- not_in_sim_yet: pollen carriage/outcrossing, distinct pollen food and bower nests (sources: `crates/cubarium-voxel-fauna/src/lib.rs:981`; D14 and D15 retained intent)

### Body plan, primary: "bell with paired vanes"

*Framing for generation:* flyer: "in the air, the frame empty all around it, nothing
beneath it"; never the sprite block's resting base.

A hanging **bell body** 4 × 6 px `#2A0E4A` with a `#FF2AFC` magenta rim at its open
lower edge (the inherited accent), a coiled **siphon** 3 px under it, and two pairs of
**vanes**: stiff flat wings 7 × 2 px each, one pair rooted at the bell's front shoulder
and one at its rear shoulder with a gap between the pairs, `#B99BE6` with a `#42C5F8`
leading edge,
the front pair swept forward and the rear pair back. In flight the pairs beat in
alternation (2 frames); at rest they fold flat along the bell. Sip: the bell hangs from
a bloomcrown core, siphon extended into it, vanes still.

Poses: **fly** (2 f, moving), **hover** (2 f, in place), **land / sip** (bell on a
core), **rest** (folded on a stem or under a frond). Starving: the vanes lose the cyan
edge.

*Alternates.* **(B) Fan-tail**: a single fan wing and a long counterweight tail. **(C)
Spinner**: a disc body spun by one helical vane; the strangest.

*Model.* Implemented founder with free-height flight and separate Nectar food
from bloomcrown stocks or vine spurs. Pollen carriage remains *not in the sim yet*.

---

## Review order for Wrysk

Batch 1 (D1–D4) has roughs; batch 2 (D5–D14) is prose and gets roughs after Wrysk's
notes on batch 1, so the plant renders inherit the surface and outline verdicts.
