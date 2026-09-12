# Cubarium creature atelier

This is the first game-art workflow for Cubarium: editable cutout sprite rigs,
Godot animation timelines, habitat artwork, and a bake into the actual cube
renderer. Start here to draw or animate. Godot 4.7.2 is the reference baker (4.6.3 was used for the first verification).

```sh
./scripts/godot.sh --editor
```

The launcher uses `GODOT_BIN`, an installed `godot`/`godot4`, or the optional
ignored `.tools/godot/godot` executable. Download the standard editor from the
[official archive](https://godotengine.org/download/archive/4.7.2-stable/) if
needed. A workspace-local editor was placed under `.tools/godot/` during this
implementation; that installation is not part of Git.

Press **F6** with `atelier.tscn` open, or **F5**. The three specimens are shown
enlarged. Buttons or keys **1–4** select rest, move, feed and bud. The cube
gallery described below is the check at the final display resolution.

| Source | What to edit |
| --- | --- |
| `parts/*.svg` | Creature artwork. These are ordinary transparent sprites; PNG replacements also work. |
| `creatures/lantern.tscn` | Broad shell, little feet and a probing feeler. |
| `creatures/sail.tscn` | Angular body and two independently pivoting fins. |
| `creatures/mossback.tscn` | Compact body, asymmetric crown and walking feet. |
| `creatures/skimmer.tscn` | Long low hull, paddle fins on pivots, forked tail and two feelers. |
| `habitat/*.svg` | Rosette, fern and lichen artwork. |
| Each rig's `AnimationPlayer` | Native editable `rest`, `move`, `feed`, `bud` tracks and `RESET` pose. |

These names identify visual studies. They do not establish species, diets,
photosynthesis, or evolutionary advantages. The habitat motifs in the art study
are scenery candidates; they have not been connected to producer biomass yet.

For creature art, open a `.tscn` and replace its `Sprite2D` textures, move parts
or adjust their parent pivots. One scene unit is one cube pixel; the body faces
right along +x. Draw larger while designing, then test the reduced silhouette.
The initial adults occupy about 8–12 pixels, making these a larger-body
experiment relative to the older 3–7-pixel proposal. Their padding and pivot
are a 16×16 tile centered at (8,8), and the surface renderer enforces its
existing nine-pixel radial extent budget. Check visible size in the web net,
on the cube, and at room distance.

Each part is under a named `Node2D` pivot. Rotate feet/fins at those pivots;
animate the body separately. All are simple cutout rigs, without mesh skinning
or inverse kinematics. The animation timeline can change position, rotation,
scale and visibility. `RESET` restores the properties used by every clip, so a
bud or feeding pose does not leak into the next state. Rest has little or no
motion. Move has locomotion gestures, feeding has probing/folding, and budding
grows a real attached visual part in the study.

Choose **Project → Tools → Bake Cubarium art** to save the scenes and export.
The same operation is available from the repository root:

```sh
./scripts/art-bake.sh
./scripts/art-study.sh
```

Open **http://127.0.0.1:7394/** for the garden on the rotatable cube and net.
It runs at normal time. The pre-existing M2 world on port 7393 is separate.
Restart the study after a bake to load the new artwork. Ctrl-C closes it.

```sh
# Sixteen poses per face: columns lantern/sail/mossback/skimmer;
# rows rest/move/feed/bud. No scenery obscures the silhouettes.
./scripts/art-study.sh --scene gallery

# Save the same native face buffers used by the browser and cube.
./scripts/art-study.sh --sink png --seconds 12 --out captures/atelier

# Select the physical display explicitly when ready to review it.
./scripts/art-study.sh --sink shim
```

The garden has 25 deliberately placed specimens, long travel segments,
stationary rest/feeding/budding poses, and explicit seam crossings. It is an
animation and composition study with scripted state changes. It neither runs
nor replaces the M2 ecology, evolves these forms, nor uses a saved world.
The gallery's bud row repeats its growth for inspection; live integration must
instead use actual gestation progress and local birth placement.

The Godot bake samples the actual scene transforms and animation tracks into
`../assets/atelier/creatures.png`, `habitat.png` and `pack.json`. Edit source
scenes/art rather than those generated files. Keep the baked pack in Git so
viewing the study does not require Godot. Source PNG/SVG files must match their
imported size; the initial pipeline supports full-texture `Sprite2D` cutouts
with tree draw order and standard alpha blending. AtlasTexture, SpriteFrames,
regions, z-index sorting, shaders, skeletal mesh deformation and other custom
draw operations require exporter work before their output can be relied on.

The Rust renderer composites premultiplied linear-light RGBA through
`cubarium-surface::unfold_pixels`. Alpha masks preserve dark outlines, and
the shared atlas carries partial sprites over edges with one pixel owner at
vertices. The same geometry and original `cube-proto` transport remain in use.
This is a Godot authoring integration, not a full runtime port.

An effective first contribution is to repaint one creature's parts and one
habitat motif, bake, and compare the result in the gallery and garden. This
lets Wrysk change the visual vocabulary directly while the source rigs keep
its poses editable. The next runtime slice should connect selected art to
real feeding/rest/gestation and producer growth, then evaluate inheritance
and ecological diversity separately from these scripted demonstrations.

## Live world

```sh
cubarium run --art assets/atelier --sink web
```

This is the persistent M2 world, not a study: every organism is drawn with a
baked clip chosen by the state the world actually put it in — `rest` while
resting, `move` while seeking, `feed` while feeding, and `bud` while an escrow
is gestating, that clip advanced by the real gestation progress so its last
frame lands on the birth. Looping clips run on simulated time, so `--speed 8`
animates eight times faster and a paused world holds its pose.

Which rig an organism uses is its inherited `hue` gene's tercile —
`min(2, floor(hue × 3))`, so lantern/sail/mossback are hue 0–⅓, ⅓–⅔ and ⅔–1.
That is **cosmetic only**. `hue` is copied exactly at birth, which is why a
lineage keeps its look, but it is not a species, a diet, or a capability, and
the simulation does not know the rigs exist.

The plants of `PLANTS.md` grow from the fields, one slot per field cell at a
hashed placement. A slot's species is picked by its band (soil: glowcap or
rootveil; foliage: lanternstalk or tendrilfan; canopy: umbrellafrond or
bloomcrown; any cell deeper than `REED_DEPTH` of water: reedspire). The plant
climbs through its three authored stages as its field passes the band's
`*_STAGES` thresholds, with hysteresis (`STAGE_HYST`) so an oscillating field
does not flicker, and a hashed rank caps how far each slot may grow
(`RANK_FULL`, `RANK_MID`) so a rich patch is a few full plants, more mid ones
and many sprouts. Stalks stand up toward the canopy on the side faces; on the
top face plants face freely. A full-grown plant with a `fruit` clip plays it
where the cell holds fruit above `FRUIT_SHOW`, once the world publishes fruit.
All of these live in `crates/cubarium/src/art_present.rs` as review-tunable
constants. Plants are scenery that follows the fields: they are not organisms,
nothing in the world knows about them, they never move and they are never
eaten. The legacy rosette/fern/lichen tiles stay in the pack but are no longer
drawn.

### Bands

The art mode draws the cube as three places rather than one top-down field, by
the embedded height `h` of a point (top face 1, open rim −1). **Soil** is the
bottom five of the sixteen cell rows of each side face, `h < SOIL_TOP`: no
producer lawn and no detritus flecks there, only a ramp of the detritus field
from dark plum to violet-mauve, with dim glowcaps and rootveils where detritus
is deep. **Foliage** is the rest of the side faces and keeps the decided
producer ramp and its flecks, with lanternstalks and tendrilfans by producer
density. **Canopy** is the top face, which keeps the producer ramp and grows
umbrellafronds and bloomcrowns at lower thresholds, so a healthy top reads as
covered.

The horizon between soil and foliage is a soft blend evaluated at every
pixel's own height, not its cell's, so it is one horizontal line all the way
round the cube rather than a staircase of cell edges.

`SOIL_TOP`, the horizon half-width `HORIZON`, the soil palette
(`SOIL_LOW_SRGB`, `SOIL_HIGH_SRGB`, and their brightnesses) and the per-band
plant stage thresholds all live in `crates/cubarium/src/art_present.rs` as
review-tunable constants; the soil colors are meant to move only within the
Outrun family of `design/appearance.md` "Palette". **The simulation does not
know about bands.** Nothing in the world reads `SOIL_TOP`; the world publishes
the same producer and detritus fields it always did, and the bands are a
consequence of how light falls with depth and where detritus ends up
(`design/stratified-world.md`).

Without `--art` the image is the decided M2 one, pixel for pixel. The web
viewer's HUD carries the simulation speed (`tick N · 60 fps · 8× time`), so a
fast run is never mistaken for a live-speed one.

### Water

Pools and streams are drawn from the world's per-cell depth, source-over the ground and
under the plants: `mix(#1E9BF2, #42C5F8)` by depth, coverage `1 − exp(−w / WATER_FILM)`
(0.6), brightness `WATER_BRIGHT` (0.55) with a `WATER_SHIMMER` (8 %) glint on a 2.5 s
simulated cycle, seam-filtered like the ramps. Rain is this tick's per-cell rate: up to
`RAIN_MAX_STREAKS` one-by-two streaks of `#B8F0FF` per cell (`RAIN_DENSITY` per unit of
rate), falling down the side faces at `RAIN_SPEED` (20 px/s) and wrapping every
`RAIN_PERIOD` (0.4 s), sparkling on the level top face. Reeds stand only in pools deeper
than `REED_STAGES[0]` (0.6) and only in the slots whose rank may grow beyond a sprout, so a
flooded floor row is clumps of reeds with open water between them, not a fence. All of
these are `pub const`s in `crates/cubarium/src/art_present.rs`.

### Tall plants

The foliage band is eleven cells tall and a 16-pixel plant cannot fill it, so tall plants
grow in columns: a hash selects `TALL_COLUMN_P` (22 %) of the side-face cell columns for a
`spiretree` or `glasscane`, and `TALL_VINE_P` (40 %) of those also carry a `vinecoil` on
every second trunk. A column shows a base at the horizon cell, `round((t_col − t₀) /
TALL_STEP)` trunk segments 4 px apart (`TALL_STEP` 0.08 above the foliage's first stage
threshold `t₀`, up to `TALL_MAX_SEGMENTS` = 9) and a crown, where `t_col` is the mean
producer density of the column's foliage cells; the height lags downward by `TALL_HYST`.
A full column's crown sits on the rim cell's center and the shared surface carries it onto
the top face: the tree tops are the canopy. Bodies draw in front of trunks.

### Ground

Between the plants each band lays its tileable 8×8 texture (`grit`, `mossweave`,
`frondmat`) on an 8-pixel lattice, breathing on its own clip, at `GROUND_OPACITY` (0.30)
times how far the band's density sits above its first stage threshold, cross-faded
through the horizon like the ground under it. Drop `GROUND_OPACITY` first if the lattice
reads as a grid on the cube.
