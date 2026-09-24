---
design_status: proposal
last_reviewed: 2026-09-23
decision_refs: []
---

# D15 · Latticevine (*Reticulum rupis*, "latticevine")

> **Colour note (2026-09-24):** the hexes in this document are superseded by
> `design/art-direction/species-colours-2026-09-24.md`. Parts, counts, shapes and
> states here still bind.

A new plant for Wrysk's species session (request 2026-09-23): an ivy-like colonist of
rock faces that gives flying and climbing animals somewhere to live and feeds them in
turn. Same ground rules, palette, unit (1 voxel = 0.125 m = 6 logical px) and plant axes
as `species-dossiers-2026-09-21.md`; this entry follows that file's format. Nothing
here is decided until Wrysk says so.

## Revision 2: a spreading cover, not a single plant (2026-09-23)

Where this section conflicts with the sections below (**Size**, **Model requests**, the
fixed four growth sizes), this section wins. The sections below still hold for the
parts, colours and phases.

**Decided by Wrysk, 2026-09-23:**

- The voxelized Blender model "looks fine". No further voxelization work.
- **Flowers and fruit show only when the simulation has them.** Nothing on the plant is
  decoration pretending to be state.
- The plant **tiles surfaces and grows without a size cap**: up walls, and **down from
  overhangs, with its roots on top**.
- **Roots spread too:** runners produce **sister plants** where they reach soil.
- Undersides of overhangs are allowed. Wrysk expects them to be unviable for lack of
  light, "but why not if it's not expensive to sim".
- Approved plan: a face-cover model in the simulation, a tile layer in the presenter,
  and the Blender builder as the texture source.
- Wrysk delegated the rule for how neighbouring plants compete for lateral space to
  Fable. Fable's rule is below.

### The simulation: a face-cover field

- **Where it lives.** On the exposed **faces of rock voxels**: vertical side faces, and
  the downward faces of overhangs. A **covered face** carries its owning plant, how leafy
  it is (0–1), and one **spur** slot with a phase: bare, bud, flower, fruit or spent.
- **Plants and roots.** A plant is rooted in a soil support face: at the **foot** of a
  wall, next to its first covered face, or on the **top of a ledge or overhang**, next
  to the face it hangs down. From the foot it spreads up and sideways. From a top it
  spreads down, sideways, and onto the underside of the overhang. The root draws water
  from that site's soil the way a stand does. It shares the site with any ground stand
  there, and they compete through soil water, not through space.
- **Spread.** A covered face can extend its plant onto an adjacent **bare** face (up,
  down, sideways, or around an edge onto the next face) when the plant has reserve to
  pay for it. There is **no size cap**. Extent is limited by:
  - water: root uptake grows with the root's age, and every covered face transpires;
  - light: each face earns from its own exposure, so undersides and deep shade barely
    earn;
  - grazing: frondgrazers crop the band within their reach, so that band stays thin;
  - dieback: a face whose upkeep isn't met loses leafiness. At zero it keeps its runner
    while the plant has reserve (see drought tolerance below), and it becomes bare rock
    only when the runner itself dies.
- **Drought tolerance: leaf drop and regreening (Wrysk, 2026-09-23).** In a drought
  the vine drops its leaves but keeps its runners and roots, and it regreens when
  conditions improve.
  - **Trigger:** when the root's soil moisture stays below a dry threshold for a
    sustained spell, the plant goes **dormant**. Its faces shed their leaves over a short
    fall (wilting shingles, then bare runner). The shed leaves go to the litter at the
    foot, which is littershredder food.
  - **Dormant state:** each face keeps its owner and its **runner**, at leafiness 0
    with a small woody upkeep. The plant's reserve sits in the runners and the root. Spurs
    don't cycle while dormant. A spur already in fruit keeps its beads until they're eaten
    or fall.
  - **Regreening:** when moisture stays above a wet threshold for a sustained spell, the
    plant leafs out, face by face from the root outward, paying for each face from reserve.
    The two thresholds differ, and each needs a sustained spell, so a single shower in a
    drought doesn't cause flicker.
  - **Death:** a dormant plant whose reserve can't pay the woody upkeep loses its
    **outermost runners first**. Those faces become bare rock, and only then are they
    free for others. A long enough drought shrinks the colony back toward its roots; a
    short one leaves it intact and bare.
  - **In competition:** a dormant face isn't up for grabs just because it's leafless.
    The owner counts among the candidates and keeps the face while its reserve per face
    is at least a challenger's. Drought-dropped cover holds its ground, and only a plant
    that is actually running out loses faces.
  - **Drawn as:** the bare-runner tile (leafiness 0). The runners stay visible on the
    rock, so a dormant wall reads as a net of dark vines, and the green comes back from
    the root outward.
- **Sister plants (roots spreading).** When a covered face lies next to a soil support
  face that has no latticevine root, the plant can pay to **root a sister** there. The
  sister is a new plant. It takes over ownership of the faces nearer its own root than
  the parent's (by path along the cover). From then on it keeps its own water and
  reserve; sisters don't share resources after rooting. That's how a colony climbs a
  tall wall indefinitely: it re-roots on every ledge it reaches.
- **Lateral competition (Fable's rule).** First come holds ground, and a weakened face
  goes to the strongest neighbour:
  - A plant never spreads onto a face another plant owns, sister or stranger.
  - A face whose leafiness drops below a threshold (starved, shaded, cropped) becomes
    **contestable**. Among the plants owning an adjacent face, the one with the most
    reserve per covered face takes it over.
  - So a boundary between two plants stays put while both are healthy, and moves toward
    the weaker one when it struggles. A colony of sisters behaves the same way, so the
    well-watered sisters slowly take the wall from the dry ones.
  - Simple, local and cheap: one comparison per contested face, only when a face
    weakens.
- **Spurs and food.** Each covered face has one spur slot. A spur starts a cycle
  (bud → flower → fruit → spent → bare) when its plant has reserve and a rain pulse has
  just passed, with a jittered start, so phases scatter across the colony. A face in
  **flower holds nectar** (the bellwing's food). A face in **fruit holds beads** (the
  seedporter's food; seeds ride in the gut to the foot of another face, the only long
  dispersal). Nectar and fruit are two separate products that animals deplete.
- **Cost.** One record per covered face, updated on the flora schedule. Underside faces
  are the same record type with a different exposure, so there's no extra system and no
  extra cost.

### The presenter: a tile layer

- A covered face draws as a **foliage cell in the air voxel in front of the rock face**
  (below it for an underside), using the see-through foliage textures from the voxel
  texture package (`design/handoffs/voxel-textures-lod-2026-09-23.md`), so rock shows
  through the gaps at every level of detail.
- The tile is picked by the **neighbour mask**: which adjacent faces are also covered,
  and whether the face is rooted. Runners join up across tiles, cover edges are ragged,
  and rooted faces show the root arch.
- Leafiness picks the density: full, thinned or bare runner.
- **A bell appears in a face's cell only while its spur is in flower, and a bead bunch
  only while it is in fruit.** A bud shows only in the bud phase. Bare and spent spurs
  show nothing, or a stub. No accent is baked into any tile.
- **Texture source:** the Blender builder (`scripts/blender/latticevine.py`) renders
  the tile set at 48 px per face: a runner, braid and rosette tile for each neighbour
  mask, in the dense look Wrysk signed off. The smaller levels are derived by the
  texture package's tool.

### Order of work

1. This revision (done).
2. A simulation package in the flora crate, tests first: the face-cover field, rooting,
   spread, sister rooting, lateral competition, spurs, and nectar and fruit products. It
   needs new schemas; fresh worlds only.
3. The presenter tile layer, after the voxel texture package lands.
4. The fauna side (climbing, roosting, feeding on nectar and fruit, gut dispersal) with
   the bellwing and seedporter bodies, later.

## Field entry

The terrarium's rock faces are the one surface nothing lives on: stonecushion keeps to
the lips, and the grazers can't reach up the rock. Latticevine takes them over. A
stand roots in a soil pocket at the foot of a face or on a ledge and sends flat runners
up the rock, which branch and rejoin into a **diamond lattice** pinned to the stone at
every node. Older runners thicken and bow away from the rock, leaving hollow
**bowers** between the lattice and the stone. Bellwings roost and nest in the small
bowers, and seedporters climb the lattice as a ladder and den in the large ones.

It flowers and fruits **in patches, never all at once**. Each node along a runner
carries a **spur**, and every spur runs its own cycle: *bud → flower → fruit → spent*.
A spur starts only when the stand has reserve to spend and a rain pulse has just passed,
and each spur starts at a slightly different time. A grown stand therefore always shows
a scatter of phases across the face: a few spurs in flower (nectar, for bellwings), a
few in fruit (beads, for seedporters), most bare leaf. One face feeds both animals
through the year without a mast.

It's a mutualism on both sides. Bellwings sipping the flowers carry pollen from spur
to spur, and the fruit set follows the visits. Seedporters eat the beads and pass the
hard seeds at the foot of the next face they climb, which is how the vine crosses gaps
between faces.

*Latin:* *reticulum* (a net), *rupis* (of the rock).

## Niche

| | |
| --- | --- |
| **Where** | Rooted in a soil pocket at the **foot of a rock face** or on a **ledge**; its cover spreads up (and from ledges, down) the adjacent vertical faces. In the elevated-oblique view the terrace risers face the camera, so a face covered in latticevine is among the most visible surfaces in the scene. |
| **Light** | Side light on the face. It tolerates part shade (bowers are dark), but it has to reach open face to flower. |
| **Water** | Moderate. Roots in the pocket, and the leaf shingles hold a little water, so it gets through short dry spells. It's finished if the foot pocket is under standing water. |
| **Grazing** | A **refuge**: frondgrazers crop only the lowest band (≤ 0.75 m, their reach); everything above is out of reach. The cropped lower band is the visible grazing line. |
| **Competition** | Stonecushion holds the lips and latticevine holds the faces, so the two share a cliff. Where it roots at the foot it sits among springturf and velvetpad. |
| **Pace** | Slow to cover (like stonecushion, a long-lived rock plant), then steady, with sporadic spur cycles for as long as it has reserve. |
| **Gives** | Nectar (flower phase) → bellwing. Fruit beads (fruit phase) → seedporter. Roost/nest bowers → bellwing (small), seedporter (large). A climbing substrate → seedporter, and any climber. Leaf-shingle litter at the foot → littershredder. |

## Size

- **Rooted base**: 1 voxel wide (6 px) on its support face.
- **Cover** on the face: newborn 1 × 2 voxels (one runner); young 3 × 5; grown 6 × 10;
  full **10 voxels wide × 16 voxels up (1.25 × 2 m; 60 × 96 px)**, five times as tall
  as its root pocket is wide and about one and a half times as tall as it is wide.
- **Stand-off from the rock**: runners lie flat (0 px) when young; old runners bow out
  to **1 voxel (6 px)**, which is the depth of a bower.
- **Bowers**: small ones 1 × 1 voxel (a bellwing's body); from grown size on, the
  lowest bower on a full stand is 3 × 2 voxels (a seedporter's body, 0.5 × 0.25 m).

## Revision 1 (Wrysk's notes on round 1, 2026-09-23)

Round 1 (`art/gen/runs/2026-09-23-latticevine/`): Wrysk likes the **flowers, the beads
and the colours**; the **diamond lattice doesn't feel plant-like**; and flowers and
fruit should grow **from anywhere on the plant**, not only along its outer edges. The
next round looks for a more organic growth habit that still reads as alien ivy. The
lattice below stays on record as round 1's primary; nothing replaces it until Wrysk
picks.

What carries over unchanged: palette and hexes, holdfast pads, leaf-shingle rows,
magenta bead clusters, the root pocket, root arches at the foot (round 1 drew the dens
there), and per-spur phases. The flower is now drawn **as round 1 drew it**: one hanging
bell per spur, pale lilac `#B99BE6`, its lip flaring into a glowing `#42C5F8` mouth
(the three-trumpet fan is dropped). Spurs rise from **any point along any runner**, across
the whole interior of the cover as well as its edges, spaced irregularly.

Growth habits for round 2 (all pressed flat to the rock, with bare rock showing in
irregular gaps):

- **(O1) Crack-follower.** Thick main runners follow the rock's cracks and joints
  upward, then fork at irregular intervals into thinner and thinner runners that wander
  across the face and curl at their tips. There's no repeating grid; the gaps are
  irregular shapes of every size. Short fringes of rootlets along each runner's
  underside grip the stone between holdfast pads.
- **(O2) Forking fan.** From the root pocket, runners split again and again in Y-forks,
  like a river delta running upward, or coral. Each fork is thinner than its parent and
  spreads into a wide, uneven fan with rounded lobes of leaf-shingles at the tips.
  Older forks near the base fuse into a thick braided trunk.
- **(O3) Rosette mat.** Short hidden runners carry dense overlapping **rosettes** of
  shingle scales, 1–2 voxels across, clumped into an irregular mat with ragged edges
  and rock showing through tears in it. Spurs rise from between the rosettes, and older
  runners show only where the mat has thinned.

### Round 3 habit: (O4) braided fan with rosette tips (Wrysk: "try your suggestion")

Round 2 (`contact-sheet-r2.png`): every habit was drawn as a tree with a central trunk.
O2's braided forks had the strongest structure and O3's rosettes the best surface. O4
combines them:

- **No single trunk.** Three to five runners start from **separate points** along the
  foot of the face, a voxel or more apart, each from its own small root arch. Some
  runners meet and braid together partway up, then split again. The spread is
  **lopsided**: one side climbs higher and further than the other, following the rock's
  diagonal joints.
- **Forks.** Each runner splits in Y-forks, each fork thinner than its parent,
  `#2A0E4A` deep plum low and `#3A1A7A` higher up, with holdfast pads where it touches
  the rock.
- **Rosette clumps.** The fork tips and the upper runners end in **clumps of overlapping
  shingle rosettes** (O3's surface), 1–2 voxels across, in the producer ramp. The clumps
  are irregular in size, some merged and some alone.
- **Rock between.** Bare rock shows in irregular gaps between clumps and between runners;
  the plant covers about half the face it spans.
- **Spurs.** Bells and bead bunches rise from **between the rosettes and along the
  runners**, scattered through the middle as well as the edges. A bunch is round, as wide
  as it is tall.
- **Dens.** The root arches at the foot are the seedporter dens. The small bellwing
  roosts are the dark hollows under the lowest clumps.

### Model shape: signed off by Wrysk (2026-09-23)

Round 3's images still read like O1, so Wrysk stopped image iteration and had the shape
modelled in Blender instead: `scripts/blender/latticevine.py`, renders in
`runs/latticevine-model-2026-09-23/`. Wrysk: "smooth shape looks good; dense". The
**dense variant of the smooth Blender model** (about 80 % leafed above the bower band)
is the latticevine's shape; the builder's README records the look choices it made. The
voxelization is still in progress (thin runners so rock gaps survive at 0.125 m).

## Body plan, round 1 primary: "diamond lattice" (drawn with its rock)

Authored **with its rock face**, like stonecushion is with its lip: the sheet includes
the face behind the lattice as flat `#12093A` stone with a `#2A0E4A` plate joint here
and there, so the vine never floats in the air.

- **Root pocket.** At the foot, a tuft of 3–4 short root claws `#510B6D` gripping the
  lip of the soil pocket, with the first runner rising out of it.
- **Runners.** Stems 2 px thick, young ones `#3A1A7A` mid violet and old ones `#2A0E4A`
  deep plum and 3 px thick. They rise at 30° off vertical, alternating left and right,
  and cross at **nodes**, making a diamond lattice with **open rock showing in every
  diamond**. A diamond is 2 voxels tall × 1.5 voxels wide (12 × 9 px).
- **Holdfasts.** At every node, a flat round pad 3 px across, `#510B6D` detritus violet
  with a `#B99BE6` pale-lilac rim, pressed onto the stone. The pads pin the lattice to
  the rock.
- **Leaf shingles.** Along the upper side of each runner between nodes, a row of 3–4
  overlapping rounded scales, 2 × 2 px each, laid flat and pointing upward like roof
  tiles on the runner: `#1E2798` at the base of the row, `#2B6AD0` in the middle,
  `#42C5F8` at the newest scale at the top. Bare runner shows between one row and the
  next node.
- **Bowers.** On grown and full stands, the lowest runners are thick and arch **away**
  from the rock between two holdfasts, so a dark hollow (`#12093A`, one step darker
  than the stone) shows behind them. That hollow is the bower. The arch's top edge
  catches a `#3A1A7A` highlight.
- **Spurs.** At each node, a short stalk 1 px thick pointing outward and down, carrying
  its current phase:

| Phase | Drawn as |
| --- | --- |
| **bare** | a 1-px `#3A1A7A` stub |
| **bud** | a closed teardrop 2 × 3 px, `#3A1A7A` with a `#B99BE6` tip |
| **flower** | three narrow trumpets 1 × 3 px fanned downward from the stalk, `#B99BE6` pale lilac with a glowing `#42C5F8` throat at each mouth. The throats are the stand's brightest points; bellwings hover beneath and siphon upward into them |
| **fruit** | a hanging cluster of 4–6 round glassy **beads**, 2 px each, touching in a bunch, `#FF2AFC` magenta, translucent, with one `#42C5F8` seed dot inside each |
| **spent** | the cluster's empty stalk, drooping, `#510B6D` |

The ripe accent is magenta beads, **not** warm: lanternberry keeps the warm-orange fruit,
and latticevine's fruit reads as its own food.

## States (per the plant axes)

| Axis | Latticevine shows |
| --- | --- |
| **size** (wood/wood_max) | the four cover sizes above; the lattice extends from the top, and from grown size on the lowest runners thicken into bowers |
| **fullness** | full: every runner shingled. Thinned (cropped/shaded): the lowest band below 6 voxels loses its shingles to bare runner (the grazing line), and shaded rows go `#1E2798` throughout. Bare: runners and holdfasts only |
| **water** | wilting: shingles fold edge-on (1-px dashes), rock shows through everywhere; waterlogged: the foot band dark, spurs bare |
| **ripe / phase** | per-spur phase as in the table. **Model request** below: needs a phase per spur (or per band), not the single parcel flag |

Sway: the flowers and bead clusters swing on their stalks. Runners are pinned and do
not sway, apart from the bowed arches, which breathe 1 px in strong wind.

## Poses of its visitors (so the sheets agree)

- **Bellwing sips**: hovers beneath a flowering spur, bell hanging, siphon rising into
  a trumpet throat, vanes beating. **Bellwing roosts**: folded flat inside a small bower,
  magenta rim showing in the dark.
- **Seedporter climbs**: moves up the lattice from node to node using the holdfasts as
  rungs. **Eats**: sits on a node, pulls a bead cluster. **Dens**: curled in the large
  low bower. (The seedporter's body plan is **not decided**, SP-A…D; sheets draw it only
  as a placeholder or leave it out.)

## Alternates

- **(B) Curtain vine.** Roots on a **ledge** and hangs **down** the face in long
  parallel strands, fruit and flower at the strand tips. Bowers are the gaps between the
  curtain and the rock. It reads as a waterfall of leaves; it's easier to climb down than
  up.
- **(C) Knot-net.** Runners run horizontally in rows like a woven net, with a ball-shaped
  knot at each crossing that's either a flower head or a fruit head. It's more
  graphic and less plant-like, and each knot is a readable food point at 4 px.

## Model requests (nothing here exists yet)

1. **Face cover.** Stands sit on upward support faces only. Latticevine needs a stand
   rooted on a support face whose crown extent lies on the adjacent **vertical faces**
   (a cover rectangle up/down the face), with light read from the face's exposure.
2. **Two food products.** Nectar (flower phase) separate from fruit (fruit phase); today
   a plant has one parcel. The per-spur phases can be a small ring of spur cohorts per
   stand, each started on reserve + a recent rain pulse, with a jittered start.
3. **Climb substrate.** Fauna "climb" is a rise test today; a covered face would be
   climbable for species with a climbing locomotion flag (seedporter).
4. **Roost / nest sites.** Bowers as sites an animal can occupy: small for the bellwing
   (rest and, later, breeding), large for the seedporter.
5. **Gut dispersal.** Seedporter-carried seeds dropped at the foot of another face,
   the vine's only long-range dispersal.

## Generation notes

- **Framing: a rock-face tile.** Not the sprite block's "base resting on the ground". Use
  "a flat rock wall filling the frame, the vine pressed against it", on a **tall canvas**
  (about 2:3), with the root pocket at the bottom edge.
- State the lattice by **gap and angle**: "open rock showing inside every diamond",
  "runners cross at 30° from vertical". Say "six to nine flowering spurs and six to nine
  fruiting spurs, scattered, the rest bare leaf", so the model doesn't bloom the whole
  wall.
- The beads are "round glassy magenta beads", never "berries" or "grapes", which
  bring in earth colours. The trumpets are "narrow trumpets", never "flowers like …".
