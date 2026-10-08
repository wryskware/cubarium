# `game/world/`: the world view (H1, D-0005)

`WorldView` (`world_view.tscn`, `class_name WorldView`) shows the world drawn by the Rust
renderer: `cubarium-gpu`'s voxel walk with its lighting, sky and weather, at the
pixel-scale raster, exactly as `cubarium voxel` draws it (`design/handoffs/hybrid-renderer-2026-10-07.md`).
The mesh view of package B is in `legacy/`, runnable for comparison until Wrysk has seen
this one.

## How a frame gets here

- `WorldRenderer` (Rust, `crates/cubarium-godot/src/renderer.rs`) wraps
  `cubarium::sink::gpu::voxel::hosted::Hosted`.
- **The session's loop** packs the world into the renderer through a `LoopHook`
  (`crates/cubarium-session/src/hook.rs`), once a loop period when it moved, as
  `run_voxel` does.
- **A presenter thread** draws a frame when the picture would change (at most 60 a
  second): the frame clock (tick plus the fraction toward the next), the view's turn round
  the ring, the highlight; then copies the raster into one of two read-back slots
  (`cubarium-gpu`'s `target::readback::ReadbackRing`). It polls the slots and never waits
  on the GPU. A finished frame goes into a newest-wins mailbox.
- **This node** takes the newest frame each `_process` into an `ImageTexture` and draws it
  through a canvas shader that is the desktop window's present pass (`present.frag`,
  `WindowFit`): the raster fitted to the view with black bars, each screen pixel the area
  average of the raster under it in linear light (the nearest upscale at a whole scale).
- `px_per_voxel = "auto"` covers the view's size as the desktop window does
  (`cover_px_per_voxel`); a config that fixes it (the wide world's 4) keeps it.

## API

| | |
|---|---|
| `attach(session)` / `detach()` | draw a live `CubariumSession`: the renderer hooks into its loop (one view per session). `detach` keeps the last frame |
| `show_view(view)` | draw a candidate: `candidate_ref(i)` (or `candidate_view(i)`) names its session and index (`source`, `candidate`), and the candidate's own world is rendered once. With input off (a thumbnail) it is staged in its turn, one thumbnail at a time across the app, first asked first drawn (each one's sky plane takes every core but one), and its renderer is freed, off Godot's thread, after the first frame. With input on (opened large) it starts at once |
| `problem() -> String` | why nothing is drawn yet: the renderer refused to start or reported an error before its first frame (the picker shows it on the card) |
| `input_enabled` | false: no pointer or pan input |
| `reset_framing()` | the whole strip in view, the ring unturned |
| `animal_hovered(id)`, `animal_clicked(id)` | `-1` for none. A press and release under 5 px of movement is a click; more is a drag |
| `set_highlight(id)`, `follow(id)`, `stop_follow()` | the renderer draws the animal leaning 40 % toward white (the mesh view's overlay); follow eases the view onto it; panning by hand stops it |
| `capture(size) -> Image` | the latest frame as framed, fitted into `size` with bars |
| `config_path` | another presentation config (set before `attach`/`show_view`) |
| `pick(point)`, `animal_screen(id)`, `screen_to_raster(p)`, `raster_to_screen(r)` | for tools and tests |

**Panning**: mouse drag, arrows/WASD, gamepad left stick, eased. `x` turns round the ring
without end. The renderer turns its raster by whole pixels (a uniform in `voxel.frag`
that offsets the walk's column, so screen-space effects such as the bloom have no seam);
this node shifts the frame by the fraction left over, with `x` wrapping, so the camera
glides while the world's pixels stay locked to each other. `y` never moves: the fit
always shows the whole strip.

**Animals** (H2): drawn by the renderer's model pass, not stamped into the voxels —
each its baked model in a tile at the raster's scale, placed between pixels at its
interpolated place, turned freely, lit by the walk's light, depth-tested against the walk
(`crates/cubarium-gpu/src/figures.rs`). `[figures]` in the presentation config picks
`animals = "models" | "stamped"`, `turn = "free" | "steps"` and the fades.

**Picking**: screen point → raster pixel (the fit and the shift inverted) → the ray through
it against the frame's figures (`FigurePick`), the nearest cell in front of the terrain's
own hit (`PickMap::terrain_z`); failing that, the figure with a cell nearest within 5
screen pixels. With `animals = "stamped"`: the renderer's slab walk inverted over the last
pack's body cells, as before.

## Recipes (never a window on the desktop)

```sh
scripts/build-godot-ext.sh --release
# a still of a save (load a copy: the world runs on), 40 ticks then paused
scripts/hidden.sh godot --path game --resolution 1920x1080 res://world/dev_view.tscn -- \
    --load /tmp/…/copy --still /tmp/…/still.png --raster /tmp/…/raster.png
# panned and picked
scripts/hidden.sh godot --path game --resolution 1920x1080 res://world/dev_view.tscn -- \
    --load /tmp/…/copy --pan 0.37 --panned /tmp/…/panned.png --pick /tmp/…/pick.png
# a candidate through show_view
scripts/hidden.sh godot --path game res://world/dev_view.tscn -- --candidate 7 --still /tmp/…/c.png
# frame times (real presentation on Hyprland's hidden workspace)
scripts/hidden.sh --hyprland godot --path game --resolution 2560x720 res://world/dev_view.tscn -- \
    --load /tmp/…/copy --perf 15 --pan-during-perf
```

`dev_view.gd`'s header lists every option. Saves to look at:

```sh
godot --headless --path game --script res://world/tools/found_and_save.gd -- \
    --config ../config/desktop/terrarium.toml --seed 7 --out /tmp/…/t256-s7
godot --headless --path game --script res://world/tools/advance_and_save.gd -- \
    --load /tmp/…/t256-s7 --speed 12 --stops midday:24000,dusk:66000,night:96000 --out-prefix /tmp/…/t256-s7
```

## Legacy (`legacy/`)

The mesh view (`LegacyWorldView`), its organisms, weather, light settings and dev scene:
`scripts/hidden.sh godot --path game res://world/legacy/dev_view.tscn -- --load …`. Data it
reads stays in `data/`.
