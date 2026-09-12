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
phase 0, ¼, ½ and ¾ of its length; stages 1 and 2 and `fruit` must produce four
distinct frames (rotation plus a glow pulse with four different levels is the easy
way), stage 0 needs two. The current clips are 3 s long; the runtime plays them on
simulated time, so a plant's sway slows and speeds with the world.

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
`ArtPack::tall` (`TallPlant { name, base, trunk, crown }`), `ArtPack::tall_plant(name)`,
`ArtPack::ground` (`GroundTile { name, band, frames, seconds }`) and
`ArtPack::ground_for(band)`. Godot 4.7.2 is the reference baker.
