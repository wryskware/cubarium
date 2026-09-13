# Cubarium plants

Alien plants for the stratified cube (`design/stratified-world.md`): seven authored
plants, each with three growth stages and a calm four-frame sway, three of them with a
fourth `fruit` clip. They are drawn in Wrysk's Outrun family — indigo, violet, magenta,
blue, electric cyan — widened with turquoise and mint for the canopy, and the one warm
accent kept for fruit and bloom centers. Faint translucent halos carry the
bioluminescence. Names are asset names, not species: nothing here has a simulated
effect on its own.

| Plant | Band | Idea | Fruit |
| --- | --- | --- | --- |
| `glowcap` | soil | squat fungal shelves, violet caps, cyan gill glow | — |
| `rootveil` | soil | a low crust of root filaments converging on a dark mound, blue glints | — |
| `lanternstalk` | foliage | a violet stalk carrying a pulsing cyan lantern | bulb turns warm, two berries |
| `tendrilfan` | foliage | a chalice of curling magenta tendrils with hot pink tips | warm berries on the tips |
| `umbrellafrond` | canopy | a radial turquoise frond seen from above, mint ribs, cyan center | — |
| `bloomcrown` | canopy | six violet petals around a warm center | petals darken, a warm fruit with a cyan glint |
| `reedspire` | water | two or three thin blue reeds with cyan tips standing in a ripple | — |

## Editing

Each plant is a Godot scene in `plants/<name>.tscn` built from ordinary transparent
sprites in `parts/plant_<name>_*.svg` (pixel-path SVGs, import scale 1, `crispEdges`;
a PNG of the same size also works). Every visible part hangs under a named `Node2D`
pivot; sway is a rotation of that pivot, glow is a `self_modulate` pulse on the sprite,
and growth is which pivots are visible. One scene unit is one cube pixel; the tile is
16×16 with its pivot at (8, 8), the plant's base near the bottom edge for side-face
plants and centered for the top-down canopy plants. Keep every painted pixel within
about 7.8 px of the tile center: the surface renderer enforces a 9-pixel extent budget
(pixel distance plus 1.2 px of filter support) and the bake fails to load otherwise.

Open the editor with `./scripts/godot.sh --editor`, edit a scene or repaint a part, then
**Project → Tools → Bake Cubarium art** or `./scripts/art-bake.sh`.

## The stage and sway contract

Every scene's `AnimationPlayer` holds `RESET` plus looping clips `stage0`, `stage1`,
`stage2`, and optionally `fruit`. `RESET` hides every pivot and zeros every animated
property, so a clip only has to switch on what it shows. The bake samples each clip at
`PLANT_FRAMES` (24) evenly spaced phases of its length (pack v4; it was four); across a
clip, stages 1 and 2 and `fruit` must produce at least four distinct frames (rotation
plus a glow pulse is the easy way), stage 0 at least two. The current clips are 3 s
long; the runtime plays them on simulated time, blending each frame between the two
nearest samples (`Clip::sample`), so a plant's sway is continuous at the render rate
and slows and speeds with the world. Because the baker still floors each output
pixel's source coordinate, a slowly turning one-pixel stem can hold the same pixels
through several samples and then change; the temporal blend spreads that change over
one sample interval (125 ms) rather than removing it. Growth between stages is a
runtime reveal (up the stalk on side faces, from the center on the top face) over the
authored stage clips; authored "extending stalk" poses replace that reveal, not the
pacing — see "Pack v5: growth transitions" below, where `lanternstalk` has the first.

`fruit` is the full-grown plant in flower or bearing fruit, with the warm accent or a
bright cyan glow on the fruit bodies so it reads as food at 16 px. Fauna that eat fruit
will select this clip from real state; the clip itself claims nothing.

## Pack v2 layout

`assets/atelier/plants.png` is 64 px wide (four frames) by 16 px per row. Rows are
plant-major in the order of `PLANTS` in `bake.gd`: stages 0, 1, 2, then `fruit` where
the scene has one. `pack.json` gains `"version": 2`, `"plant_atlas": "plants.png"`,
`"plant_frames": 4` and a `"plants"` array of `{name, band, stage, row, frames,
seconds}` with `stage` 0, 1, 2 or `"fruit"`. The creature atlas and the legacy
`habitat.png` (rosette, fern, lichen) are unchanged. `cubarium::art::ArtPack` exposes
`plants: Vec<Plant>` with `name`, `band`, `stages: [Clip; 3]`, `fruit: Option<Clip>`,
and `plant(name)`. The bake is byte-reproducible; keep the baked files in Git so a
preview never needs the editor.

To add a plant: paint its parts, author its scene with the clips above, append
`[name, band]` to `PLANTS` in `bake.gd`, bake, and extend the loader test's expected
list in `crates/cubarium/src/art.rs`.

## Palette rule (2026-09-12)

Flora leans mint, teal and turquoise over the indigo ground; fauna keeps magenta, blue
and cyan with warm accents. Magenta appears on plants only as accents: tendril tips,
berries, bloom centres, cane joints. `tendrilfan` was repainted to this rule (turquoise
body, magenta tips and berries); the tall species and ground tiles below follow it.

## Tall plants (pack v3)

A tall plant is a **column** of 16×16 tiles stacked along the up direction with a 4-px
step per cell: `base` at the horizon, `trunk` segments up the foliage band, `crown` at
the top. Because 16-px tiles sit on 4-px steps, segments overlap by 12 px; a trunk tile
is therefore **periodic in y with period 4**, so every overlap draws identical pixels,
and its sway is a brightness pulse only (a sub-pixel drift never moves a nearest-sampled
texel and a whole-pixel one leaves the 9-px budget at the tile ends). A cap's rows that
lie over a trunk segment repeat the trunk pattern in the same phase, so crown and base
join without a seam; the cap's own art beside the trunk simply covers empty ground. All
parts of one species share one pulse sequence so the join rows match in every frame.

| name | parts | look |
| --- | --- | --- |
| `spiretree` | base, trunk, crown | turquoise trunk with a cyan vein and a mint ring every 4 px; mint/cyan dome crown; knotted bole |
| `glasscane` | base, trunk, crown | translucent blue cane, magenta joint every 4 px; crown of three lantern bulbs, one warm; root-glass bulb base |
| `vinecoil` | trunk only | a teal tendril coiling round a hollow centre with a magenta tip per turn; drawn over another column's trunk |

Source: `art/plants/<name>.tscn` with `RESET` plus looping `base`/`trunk`/`crown` clips
(3 s, four frames sampled at 0, ¼, ½, ¾), parts under `art/parts/plant_<name>_*.svg`.
`art/plants/author_tall.py` is the script Fable painted them with; the scenes and SVGs
are the source of truth. The renderer's heading points the column "up"; on the canopy
face crowns can spill over the rim onto the top through the seam-continuous stamp.

## Ground cover (pack v3)

Three **8×8** tileable textures, alpha as coverage, four slowly breathing frames (6 s),
meant to be tiled under a band's plants and faded by density:

| name | band | look |
| --- | --- | --- |
| `grit` | soil | sparse dark-plum grains on a torus lattice, one magenta glint that walks |
| `mossweave` | foliage | a teal weave with cyan nodes that pulse |
| `frondmat` | canopy | an interlocking mint frond lattice with a walking cyan highlight |

Source: `art/ground/<name>_<frame>.svg`, blitted as they are (no rig). They are periodic
by construction; the loader test checks the wrap step against the interior.

## Pack v3 layout

`version: 3` is additive over v2. `tall_atlas: "tall.png"` is 64 px wide (four frames) by
16 × rows; `tall: [{name, part: "base"|"trunk"|"crown", row, frames: 4, seconds}]` in
species-major order with parts in base, trunk, crown order. `ground_atlas: "ground.png"`
is 32 × (8 × rows) with `ground_tile: 8`, `ground_frames: 4` and
`ground: [{name, band, row, frames: 4, seconds}]`. The Rust side exposes
`ArtPack::tall` (`TallPlant { name, base, trunk, crown, cap, tail_row }`),
`ArtPack::tall_plant(name)`, `ArtPack::ground` (`GroundTile { name, band, frames,
seconds }`) and `ArtPack::ground_for(band)`. Godot 4.7.2 is the reference baker.

## Pack v4: sample counts are data

`version: 4` changes no file layout; it makes the sample counts data. `frames` (creature
samples per clip, 16) and `plant_frames` (plant and tall samples per clip, 24) may be
anything in 2..=32; the atlases are `16 · frames` wide accordingly. Packs v1–v3 still
load and still say 8 and 4. `art/bake.gd` `FRAMES` and `PLANT_FRAMES` set them; the bake
stays byte-reproducible and the phase-0 sample of every clip is unchanged from v3.

The loader also derives each tall plant's **cap**: the crown with its trunk-joining tail
cleared. `tail_row` is measured from the baked pixels — the first crown row from which,
in every frame, every texel the trunk paints the crown paints identically (spiretree 8,
glasscane 6) — and only rows `tail_row..16` are cleared, so a dome pixel that happens
to share the trunk's colour is never taken for trunk. The runtime draws the cap, never
the crown, at the column's continuous height, over trunk rows each painted exactly
once. For that pairing the loader requires a plant's crown and trunk clips to share one
sample count and duration, and rejects a tail that would start above row 4 (the top
segment reaches four rows into the crown's tile).

## Pack v5: growth transitions

`version: 5` is additive over v4: a plant scene may carry **authored growth clips** beside
its looping stage clips. An animation named `grow<from><to>` — `to == from + 1`, `loop_mode`
`LOOP_NONE` — is baked as one extra row of `plant_frames` samples in `plants.png`, placed
after that plant's own stage and fruit rows so the atlas stays plant-major, and described in
the `plants` array as `{name, band, stage: "grow", from, to, row, frames, seconds, loop:
false}`. Every other row, every other atlas and every stage/fruit tile is unchanged from v4;
the bake stays byte-reproducible.

A growth row is sampled **inclusively**, at `i / (plant_frames − 1)` of the clip's length,
not at `i / plant_frames` like a loop: frame 0 is the source stage's pose, the last frame the
target's, and nothing wraps the end back into the beginning. `cubarium::art` exposes
`Plant::transitions: Vec<Transition { from, to, clip }>` (`clip.looping == false`) and
`Plant::transition(from, to) -> Option<&Clip>`; a v1–v4 pack, and any pair without an
authored clip, returns `None` and keeps the runtime's reveal mask. The loader requires one
stage step up, no duplicate pair, the pack's sample count, and the same 9-pixel extent budget
as every other frame.

`lanternstalk` has the first one, `grow01` (4 s, stage 0 → stage 1): the sprout dissolves into
a stalk that extends upward — a `stalk1` sprite scaled about its bottom, its centre offset
kept at exactly `−2.5 · scale.y` so the lower edge stays on the root — and a `bulb1` sprite
that fades in at the stalk's tip and enlarges to the mature lantern, always overlapping the
stem's top row by the one row stage 1 overlaps, so no gap can open. The root contact is fixed:
tile row 14 is the lowest painted row of every frame. Because `stalk1` is uniform along its
length, the scaled stem reproduces the mature art exactly, and the clip's **last frame is
pixel-identical to the neutral `Stalk1` image** — which is stage 1's *half-period* sample
(rotation 0, bulb modulate 1), not its phase-0 one; likewise the first frame is stage 0's
quarter-period sample, the sprout at modulate 1. The presenter blends the endpoints into the
running stage loops, so neither endpoint has to match an arbitrary sway phase.

### The ten authored steps (2026-09-13)

Every side-face species now carries both of its steps; the two canopy species still use the
runtime's radial reveal. Each clip is 4 s, `loop_mode = LOOP_NONE`, and lives in its own
hidden `Grow<from><to>` group of Sprite2D copies of the parts the stages already use — no
clip touches another clip's pivots and no new part was painted for any of them.

| clip | what it shows |
| --- | --- |
| `glowcap grow01` | the sprout fades under a `stem1` stem scaling 0.5 → 1 about its bottom; a `Cap1` pivot (cap1 + gills2b) rides one pixel above the stem top, fading in from 1.2 s while **widening** `scale.x` 0.43 → 1 — an umbrella opening — and the gills light last (2.8–3.9 s) as the cap settles onto its mature offset |
| `glowcap grow12` | `stem1` cross-fades into `stem2` extending 0.67 → 1; `Cap1` cross-fades into a `Cap2` that widens 0.78 → 1 and climbs with the stem top; the side shelf `Cap2b` sprouts out of the stem, fading in 2.0–2.8 s and scaling 0.5 → 1 about its own root |
| `rootveil grow01` | the crust spreads: `veil1` scales `scale.x` 0.43 → 1 about its bottom-centre pivot (0.6–3.3 s) while the sprout fades out, and `glints1` light 2.6–3.9 s |
| `rootveil grow12` | `veil2` cross-fades over veil1 at scale (0.64, 0.6) — exactly veil1's footprint — then spreads to 1 over 1.0–3.4 s; `glints2` light 2.8–3.9 s |
| `lanternstalk grow01` | the pilot, described above |
| `lanternstalk grow12` | `Stalk1` cross-fades (0.4–1.3 s) into a `stalk2` stem pinned to row 14 at `scale.y` 0.625 that extends to 1 over 1.0–2.6 s (nine painted rows at the end); a `bulb2` lantern fades in over bulb1 at 1.6–2.4 s at scale 0.5 and enlarges to 1 by 3.6 s, riding at least 2.1 px into the stem's top row throughout |
| `tendrilfan grow01` | `base1` fades in over the sprout, then each tendril **uncurls**: the `TendrilL1`/`TendrilR1` pivots scale 0.3 → 1 about their bases and rotate from ±0.6 rad inward to 0, the left leading the right by 0.3 s |
| `tendrilfan grow12` | `base2` widens `scale.x` 0.7 → 1 over base1; the tendril1s cross-fade into tendril2s that scale 0.6 → 1 and rotate ±0.4 → 0 (0.8–3.4 s); `Center2` rises `scale.y` 0.3 → 1 about its pivot (1.4–3.9 s) |
| `reedspire grow01` | the sprout fades under `ReedA1` shooting up `scale.y` 3/7 → 1 about its bottom pivot (0.6–2.8 s), `ReedB1` following 0.6 s later (3/5 → 1, to 3.9 s); `Ripple1` fades in 0.4–1.2 s |
| `reedspire grow12` | A1/B1 cross-fade into A2/B2 at matched heights (A2 `scale.y` 7/11, B2 5/8 — the one-column shift left is carried inside the cross-fade), which then extend to 1 over 1.0–3.2 s while `ReedC2` shoots up out of the ripple (0.2 → 1, 1.8–3.9 s) and `Ripple2` cross-fades in |

Three conventions bind all of them:

- **Endpoints are neutral poses, not loop samples.** Frame 0 is the source stage's
  RESET-neutral pose (every pivot at rotation 0, every `self_modulate` at 1) and the last
  frame the target's. For `glowcap`, `rootveil` and `lanternstalk` that happens to *be* the
  stage's phase-0 sample; for `tendrilfan` and `reedspire` it is **no** sample of the stage
  row at all, because their two sway pivots are never at rotation 0 at the same phase. The
  presenter's 12 % endpoint blends absorb the mismatch, so only the pilot's 0 → 1 keeps the
  stricter "last frame is a loop sample" property. Frame 0 of a `grow12` clip is therefore
  byte-identical to the last frame of the same plant's `grow01`.
- **Roots stay planted.** Every stage of every side species bottoms on tile row 14, so every
  frame of every clip paints row 14 and nothing paints row 15. (There is no one-row root
  change anywhere: `lanternstalk`'s mature `stalk2` sits on rows 6–14, not 6–13.)
- **Berries and fruit never appear.** The growth groups simply carry no berry or fruit
  sprite, so `tendrilfan`'s berries and `bloomcrown`'s `Fruit` pivot cannot leak into a step.

A fourth, softer rule came out of looking at the 8× strips: **nothing may fade to
near-nothing before its replacement is up**. Two parts covering the same texels at alpha
0.5 composite to 0.75, which reads as the plant blinking; so an outgoing part holds full
opacity until its successor is opaque and only then fades. Measured on the shipped pack, no
frame of any clip carries less total alpha than its own source pose.

Authoring one: keep the plant's shared root, extend the support before the head, never scale
a part to zero (a singular transform is skipped by the baker — fade in with visibility and
`self_modulate` alpha instead), and add **every** newly animated property's neutral value to
`RESET`, including the new group's `visible`, or the bake's RESET-before-each-sample loop will
leak a growth transform into the stage and fruit rows. A `Node2D` pivot's `modulate` fades a
whole sub-assembly at once (the baker multiplies every `CanvasItem` parent's `modulate`);
scaling that pivot scales its children's positions too, which is how a part is scaled about
a chosen root rather than about its own centre.
