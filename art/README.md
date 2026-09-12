# Cubarium creature atelier

This is the first game-art workflow for Cubarium: editable cutout sprite rigs,
Godot animation timelines, habitat artwork, and a bake into the actual cube
renderer. Start here to draw or animate. Godot 4.6.3 was used for verification.

```sh
./scripts/godot.sh --editor
```

The launcher uses `GODOT_BIN`, an installed `godot`/`godot4`, or the optional
ignored `.tools/godot/godot` executable. Download the standard editor from the
[official archive](https://godotengine.org/download/archive/4.6.3-stable/) if
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
# Twelve poses per face: columns lantern/sail/mossback;
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
