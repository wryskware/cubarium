# `game/world/`: the dev presentation (package B)

`WorldView` (`world_view.tscn`, `class_name WorldView`) draws a world: terrain and water
from the feed's chunks, organisms as their baked voxel models, daylight and rain, an
elevated orthographic camera at 30° that pans, and picking. Plain coloured voxels only;
every colour comes from the Rust presenter (`data/palette.json`). Package B of
`design/handoffs/godot-direction-2026-10-07.md`.

## API

| | |
|---|---|
| `attach(session)` / `detach()` | draw a live `CubariumSession`, taking `take_view()` and `take_events()` each frame. While attached, nothing else may take views or events from it (a take is consumed). `detach` keeps what is drawn; the session unions changes until the next `attach` |
| `show_view(view)` | draw one static view (`candidate_view(i)`); it can be panned |
| `input_enabled` | false: no pointer or pan input |
| `reset_framing()` | the default framing: the world's whole width across the view |
| `animal_hovered(id)`, `animal_clicked(id)` | `-1` for none. A press and release under 5 px of movement is a click; more is a drag |
| `set_highlight(id)`, `follow(id)`, `stop_follow()` | a white tint; follow eases the camera onto the animal; panning by hand stops it |
| `capture(size) -> Image` | the current framing at `size`, rendered alone (no UI over it), synchronously |

**Panning**: mouse drag, arrows/WASD, gamepad left stick, eased. The ring has no ends, so
`x` pans round it without limit and the seam moves (the wide world's lake can be brought
whole into view); up and down are clamped to the world, and move only when the world is
taller than the view (never at the default framing on a landscape screen). No zoom.

**In a SubViewport** (candidate thumbnails): give the SubViewport `own_world_3d = true`,
or several views share one world. The camera carries its own `Environment`, so no
`WorldEnvironment` is needed or wanted.

**Class cache**: a project never opened in the editor has no global class cache, so
`WorldView` as a type name does not resolve on a plain `godot --path game` run. Instance
the scene and keep the variable untyped (as `dev_view.gd` does), or generate the cache
once with `godot --headless --path game --import` (headless: no window).

## Light (`light_settings.gd`, `LightSettings`)

Godot's own built-in effects, lighting the meshes (the animals' models included, as they
turn). Nothing of the old renderer's voxel marches is ported. **Every effect is off by
default** (each waits for Wrysk's pick, and off skips its cost); the dev scene turns them on
with `--light all`, `--light shadows,ssao`, `--light all,gi`, and numbers as `name=value`
(`--light emitter_lights,emitter_group_voxels=8`).

| switch | built-in | notes |
|---|---|---|
| `shadows` | DirectionalLight3D shadow, one orthogonal split over the world | the terrain is meshed closed (`set_view_faces_only(false)`) so it casts; water casts none |
| `ssao`, `ssil` | Environment SSAO / SSIL | |
| `sky` | a gradient quad behind the world, on the camera | an orthographic camera sees one direction of a `Sky`, so a sky background would be one flat colour |
| `glow` | emissive surface on the emitting model cells (`data/emission.json`) + Environment glow | the glowcap candle wanders per instance in the shader |
| `haze` | depth fog, none at the world's front edge, `haze_amount` at its back wall | replaces the mesher's baked vertex haze while on |
| `reflections` | the water smooth and part metallic, lit by a `Sky`'s radiance | SSR skips translucent surfaces, so it cannot reflect the water |
| `volumetric` | froxel volumetric fog inside a FogVolume over the world | a global density would fog the empty air between the orthographic camera and the world |
| `emitter_lights` | an OmniLight3D per emitter, shadowed (or grouped, or only the nearest shadowed) | not in `all` |
| `gi` | SDFGI, or VoxelGI baked when a world is drawn (`gi_mode`) | not in `all` |

The sun is placed as the old lit tier places it (its noon direction, rise and set
azimuths, elevation floor; the moon opposite at night). Every number is a field of
`LightSettings` with its old-renderer counterpart named.

## Files

- `world_view.gd`: the API, chunk nodes, the camera, input and picking.
- `organisms.gd`: animals and plants. Animals move linearly from where they are drawn to
  the newest frame over the measured gap between frames (no tick jumps, `x` the short way
  round the ring); pose models by mobility mode and the hunker flag (`POSE_MODELS`,
  `HUNKER_MODELS`, the presenter's `mode_model`/`hunker_model`); a body that appears or
  leaves fades (slower after a `died` event). No model: a grey box.
- `weather.gd`: the sun and moon from the feed's `sky` (as the old lit tier places them),
  ambient light, the environment's effects by `LightSettings`, the sky gradient, the
  FogVolume, VoxelGI; rain streaks by rain mode. All eased.
- `light_settings.gd`: `LightSettings`, the switches and numbers.
- `emitter_lights.gd`: the emitter point lights.
- `data/emission.json`: which model cells emit (hand-written from the old presenter's rules).
- `data/palette.json`: generated, do not edit:
  `cargo run --release -p cubarium --example godot_palette -- --write game/world/data/palette.json`.
- `dev_view.tscn` / `dev_view.gd`: a world through `WorldView` for checks (options in its
  header). `tools/found_and_save.gd`: found a world once and save it, so checks `--load`.

The Rust side is `crates/cubarium-godot/src/mesher.rs` (`VoxelMesher`), `models.rs`
(`VoxelModels`, the `.cvm` reader) and `palette.rs`.

## Recipes (never a window on the desktop)

```sh
# found once, save (the wide world takes ~4 min: run it detached)
godot --headless --path game --script res://world/tools/found_and_save.gd -- \
    --config ../config/desktop/terrarium.toml --seed 7 --out /tmp/…/t256-s7
# pictures (Xvfb): framing, a pan, and capture()
scripts/hidden.sh godot --path game --resolution 1920x1080 res://world/dev_view.tscn -- \
    --load /tmp/…/t256-s7 --shots /tmp/…/shots --prefix t256
# a candidate through show_view
scripts/hidden.sh godot --path game res://world/dev_view.tscn -- --seed 3 --candidate --shots DIR
# fps (real presentation on Hyprland's hidden workspace)
scripts/hidden.sh --hyprland godot --path game --resolution 2560x720 res://world/dev_view.tscn -- \
    --load /tmp/…/wide-s1 --perf 15 --pan-during-perf
# a pan movie: frames at real-time spacing, then x264
scripts/hidden.sh godot --path game --resolution 2560x720 --fixed-fps 30 res://world/dev_view.tscn -- \
    --load /tmp/…/wide-s1 --pan-demo --record /tmp/…/frames --speed 0.43
```

```sh
# a still of one moment, lit (the sim paused once shown)
scripts/hidden.sh godot --path game --resolution 1920x1080 res://world/dev_view.tscn -- \
    --load /tmp/…/t256-s7-night --paused --light all,emitter_lights --still /tmp/…/night.png
# what each effect costs: GPU render ms per frame (vsync stays on under Hyprland's Wayland,
# so the frame time is capped; the GPU time is the number to read)
scripts/hidden.sh --hyprland godot --path game --resolution 1920x1080 res://world/dev_view.tscn -- \
    --load /tmp/…/t256-s7-midday --light shadows --no-vsync --perf 8
# a save at a time of day: run a save on to ticks and save (hour H of day 1 = (H − 8)·6000)
godot --headless --path game --script res://world/tools/advance_and_save.gd -- \
    --load /tmp/…/t256-s7 --speed 12 --stops midday:24000,dusk:66000,night:96000 --out-prefix /tmp/…/t256-s7
```

`--write-movie` ignores `--resolution` (it records the project's window size), hence
`--record`. Saving PNGs runs at about 13 fps, so `--speed 0.43` keeps the sim in step with
the movie's clock.
