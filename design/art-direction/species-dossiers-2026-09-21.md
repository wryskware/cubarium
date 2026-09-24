---
design_status: proposal
last_reviewed: 2026-09-21
decision_refs: []
---

# Species dossiers (Wrysk's design session, part 2)

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
that has died, lying where it stood. *What the simulation holds:* one scalar amount of
dead wood per ground site (with its mineral and energy), no shape, no species, no
orientation. It arrives (a) when a stand dies (its wood to its own site), (b) when a
consumer kills a stand, (c) as declared founder logs in the dev habitat, and, once the
vaulttree lands, (d) as a **line of sites** under a fallen canopy tree (decision 4). It
leaves by decomposition (slow, `wood_decomposition` 1e-4/s) and by glowcap uptake.

The sprite family therefore has exactly three honest axes:

| Axis | Source | Values |
| --- | --- | --- |
| **amount** | the site's dead wood | fragment (0.05–0.25) · section (0.25–1.0) · trunk (> 1.0) |
| **joins** | dead wood ≥ 0.05 on the x-neighbour sites | lone · left end · middle · right end |
| **colony** | glowcap stand on this site: wood and parcel | none · button · fruiting · ripe · spent |

The presenter composes: `tile[amount][joins]` under `caps[amount][colony]`. A z-neighbour
(behind or in front) is not joined; a trunk line lying along the depth is drawn as
overlapping lone sections, which under the 30° camera reads as a log seen end-on.

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

Decay within an amount bin is the amount itself sliding down: fragment tiles are what
sections become, so the fragment inherits the section's split (now a hollow) and its
plate pattern with two plates missing.

### The glowcap (*Lucerna saprophaga*)

*Niche.* A saprotroph: no light income, no light gate. It establishes only where the
3 × 3 site box (one layer up and down) holds ≥ 0.02 of dead wood or litter, eats that box
down at 0.02/s, grows to wood 0.1, spreads by hop 1 along adjacent wood, pays for
propagules into a parcel, and diebacks when the box is exhausted. Its wood at death goes
back to the site's dead wood. There is no "spent" stage in the model: spent is a stand
whose wood has fallen below `donor_min` (0.05) after peaking.

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

### Field entry

*What it is:* fallen foliage, fine and coarse fragments and dead reserve from any stand;
a per-site pool with an energy cap. It is shredded by the littershredder, eaten by the
glowcap's box, and decomposes at 1e-3/s. *Decision 5:* it is not a sprite; it is an
axis of the ground surface.

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

### Field entry

*Niche.* The paid ground browser of the meadow: it eats foliage within one voxel out
and one voxel up of the face it stands on, walks to the next patch when it has nothing
in reach and something in sense (8 voxels), rests when it has neither. It climbs a
1-voxel step, wades 0.05 m and drowns in 0.2 m. Body grows from 0.005 (newborn) to
0.05; a birth is paid from reserve at body 0.03. It starves visibly: reserve empty and
body under 0.015 flips the starving flag.

*Size under the ladder.* Model body 0.375 m; drawn shell **6 × 3 × 3 voxels = 36 × 18
px** side view (adult). A newborn is about 0.46 of adult length by volume: 3 × 1.5
voxels (18 × 9 px). Length scales with the cube root of body.

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

### Field entry

*Niche.* The blind litter feeder: 23 sensory inputs, no cone of vision; contact,
wetness, taste and litter chemistry guide it. It walks the damp litter, stops and shreds.
Same states as the grazer (Cropping here means shredding litter in reach; Walking;
Resting), same starving flag. Model body 0.19 m under the ladder; drawn shell **3 × 1 × 1
voxels = 18 × 6 px**.

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

A stand holds `wood` (size), `foliage` (fullness, at most α·wood with α = 2), `reserve`,
`light` (0–1 after sky and canopy), `moisture` (0 = wilting), `aeration_stress`,
`parcel` (saved propagule material) and `mineral`. Crown height and radius are linear
in wood between the species' authored min and max. There is one stage, Alive; dieback
is wood shrinking, senescence is paid, death removes the stand and leaves dead wood on
the site and foliage in the litter. A site with a **seed cohort** and no stand shows a
sprout mark. So every plant sheet has the same axes:

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
Visible today as one `Sprout` mark. *Sprite:* a species-coloured **seed mark** on the
ground face: for bloomcrown a 2-px `#42C5F8` dot with a 1-px `#3A1A7A` tail; for
umbrellafrond a 3 × 1 px `#1E2798` crescent; for springturf three 1-px `#42C5F8` dots
in a row; for stonecushion and velvetpad a single `#B99BE6` pixel; for glowcap nothing
(spores are not drawn). Drawn as part of the terrain surface tile's decoration layer so
it lies in the ground like litter, never as a floating sprite. No growth animation:
germination replaces the mark with the newborn plant.

---

## D6 · Springturf (*Caespes primus*, "springturf")

*Niche.* The pioneer turf of open, moist, drained soil: shallow roots, needs open sky,
fast and cheap, burns out in 250 s without income, spreads with the widest hop (3). It
is the grazer's main food and the thing that visibly gets cropped.

*Size under the ladder.* Crown 1–1.5 voxels tall, radius 1–2: a clump **2–4 voxels
wide, 6–9 px tall**, about three times as wide as it is tall, low to the ground.

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

*Niche.* The shelter carpet: thin damp soil, under ledges and crowns, low light, poor
competitive height; it only offers sites under a standing crown. Very low turnover.

*Size.* Crown 1 voxel tall, radius 1.5–3: **3–6 voxels wide, 6 px tall**.

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
twentieth of the water the others need; slow (11,500 s to full size); the only plant
that is fine while dry and finished under a pool.

*Size.* Crown 1–1.5 voxels tall, radius 1–1.25: **2–2.5 voxels wide, 6–9 px tall**.

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
grazer's second food; later nectar and pollen for the bellwing.

*Size under the ladder.* Crown 3–8 voxels tall, radius 1–2.5: **stem plus crown 18–48
px tall, crown 12–30 px wide**; the whole plant is about twice as tall as the crown is
wide, the stem the lower half.

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

*Niche.* The wet producer of hollows: light half-saturation 0.15, no aeration limit at
all (it is the saturation-immune stand-in for a wetland producer), deep roots (4),
slow; it shades what stands under it, and velvetpad lives under it.

*Size under the ladder.* Crown 8–16 voxels tall, radius 3–6: **48–96 px tall, crown
36–72 px wide**; the plant is a third taller than it is wide, the stem showing below
the lowest tier. The tallest live plant until the vaulttree.

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

## New roles (decision 2): what they need from the model

Each of these is a **new stand species** in `cubarium-voxel-flora` (a `SpeciesConfig`
preset plus, where noted, one new mechanism) or a new animal founder. The dossiers give
the visual form and the state it reads, and name the mechanism honestly.

## D11 · Vaulttree (*Arcus caeli*, "vaulttree")

*Niche (biosphere §4).* The canopy builder: deep aerated soil with enough water, needs
a gap to establish (a light gate above bloomcrown's), slow costly structure, long life,
large reserve. Its death opens a light gap and lays a **line of dead wood** (decision 4:
`die` deposits wood along `hop` sites in the direction of a site hash, each site taking
a share; a model addition packaged with the species).

*Size.* Crown 18–28 voxels tall, radius 5–8: **108–168 px tall, crown 60–96 px wide**;
nearly twice as tall as it is wide, rendered on a tall canvas.

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

*Model.* New preset (light gate ≥ 0.9 to establish, wood to ~5, slow rates, large
reserve), crown geometry [18, 28] × [5, 8], and the dead-wood line at death.

## D12 · Lanternberry (*Lucifructus*, "lanternberry")

*Niche.* Fruiting shrub of grove edges and bright moist gaps; repeated paid fruit
crops; fruit is the seedporter's food and its seeds travel. Until the seedporter and a
fruit product exist, its fruit is the visible `parcel` (a full parcel = fruit hanging).

*Size.* Crown 5–9 voxels tall, radius 2–3.5: **30–54 px tall, 24–42 px wide**.

### Body plan, primary: "bell-lantern shrub"

Three to five **stems** from one base, each 3 px thick `#3A1A7A`, spreading in a fan;
foliage is tiny **leaflets**, each a 2 × 1 px dash `#2B6AD0`, spaced along the upper
halves of the stems with bare stem showing between them; from the stem tips hang the **lanterns**: closed bells 3 × 4 px, `#2A0E4A` with
a 1-px `#FF2AFC` magenta seam, one per stem tip. **ripe**: the bells open into 5 × 5
px hanging lanterns with a `#FF9B50` warm interior seen through the opening (the second
warm-accent plant, fruit only). Fruit fall (a spent parcel) closes the bell again.

*Alternates.* **(B) Pod shrub**: pods along the stems instead of at the tips. **(C)
Coil shrub**: a single coiled stem with lanterns at each coil.

*Model.* New preset; fruit as parcel now, a fruit product later (biosphere "reproductive
products").

## D13 · Siphonreed (*Fistula stagni*, "siphonreed")

*Niche.* Emergent of saturated soil and shallow standing water: needs free water on or
beside its site, invests in aeration (its `aeration_stress` response is the tolerance
mechanism umbrellafrond skips), narrow upright clumps at the shore.

*Size.* Crown 6–12 voxels tall, radius 0.5–1: **36–72 px tall, 6–12 px wide**, in
clumps; six times as tall as wide, rendered on a tall canvas.

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

*Model.* New preset with a positive water-depth requirement at the site; the sway
should read the site's free water as a slow drift.

## D14 · Bellwing (*Campanula volans*, "bellwing")

*Niche.* Nectar for energy, pollen for nutrients; hover-land-sip rhythm; small body,
costly flight, frequent rest; carries pollen between bloomcrowns (outcrossed seed).
The first animal in the air column.

*Size.* Body 1 voxel, wingspan 3 (18 px); flies 4–20 voxels up.

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

*Model.* New animal founder with flight (a z-free walk along a height band), a nectar
food identity read from bloomcrown parcels, pollen carriage as a later product.

---

## Review order for Wrysk

Batch 1 (D1–D4) has roughs; batch 2 (D5–D14) is prose and gets roughs after Wrysk's
notes on batch 1, so the plant renders inherit the surface and outline verdicts.
