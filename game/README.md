# Cubarium in Godot (`game/`)

The Godot 4.7 app. Rust is the simulation (`crates/cubarium-session`); this project is
everything you see and touch (D-0004, `design/handoffs/godot-direction-2026-10-07.md`).
The shell (package C) runs the world through `world/world_view.tscn`, which shows the Rust
renderer's frames (D-0005, H1; `world/README.md`); the mesh view of package B is in
`world/legacy/`.

## Build

```sh
scripts/build-godot-ext.sh            # debug
scripts/build-godot-ext.sh --release  # release: founding a world is much faster
```

This builds `crates/cubarium-godot` (gdext 0.5.5, Godot API 4.7), copies the library to
`game/bin/`, where `cubarium.gdextension` points, and registers the extension in
`game/.godot/extension_list.cfg` so a headless run loads it without the editor ever having
opened the project.

## Run the app

```sh
scripts/run-game.sh            # the session's GPU, and this host's run CPUs
```

The launcher keeps the app to the CPUs the host profile names (`RUN_CPUS` in
`config/hosts/$(hostname).env`: the V-cache chiplet, 0-7,16-23, on the desktop), as
`scripts/run-voxel.sh` does for `cubarium voxel`; `CUBARIUM_CPUS` overrides it and
`CUBARIUM_CPUS=` runs unpinned. A plain `godot --path game` lands on the right GPU but
runs on every core.

The project asks for Godot's Wayland display driver (`display_server/driver.linuxbsd` in
project.godot) and falls back to X11 where there is no Wayland. Under Godot's default, X11
through XWayland, Godot picked the 5090 on the two-GPU desktop while the compositor runs on
the 5080, so every frame crossed GPUs (2026-10-08); on Wayland it takes the compositor's GPU.
The launcher also passes `--gpu-index 0` (`CUBARIUM_GODOT_GPU` overrides). The world's renderer
opens whatever GPU Godot reports (`cubarium_gpu::vk::prefer_device`; `CUBARIUM_GPU=<name>`
overrides). Agents' hidden captures on Xvfb need `CUBARIUM_GODOT_GPU=1`: the 5080 cannot
present there.

## Animal rendering per run

`CUBARIUM_ANIMALS` overrides `[figures]` when the renderer starts: `classic` uses the
original stamped voxels; `crisp` uses edged models placed at whole pixels, with eight
headings blended over the config's `turn_blend_s`; `smooth` uses edged models with free
turning and box-filtered subpixel placement; `stepped` uses edged models that hop a voxel
at a time (their drawn place snapped to the world's voxel grid, as the stamped animals
moved) and turn in `turn_steps` headings (16 by default), each change a snap
(`stepped_turn_blend_s = 0`). With no environment override, the config applies
(`[figures] movement = "smooth" | "stepped"`). The renderer logs the effective settings
once at startup.

`CUBARIUM_ANIMAL_AA` overrides `[figures] supersample` (default **2**): a positive
integer giving tile samples per pixel axis. `1` keeps the original model raster;
`2` shades 2×2 samples, smoothing silhouettes and rotated face edging. The edging
stays one output pixel wide. Whole-pixel and Stepped placement downsample aligned
blocks; Smooth+Subpixel shifts the footprint with the anchor. This is a per-run
setting, requiring a restart, and does not affect `animals = "stamped"`.
The atlas lowers the sampling factor with a diagnostic if a frame cannot fit;
exhausted hard budgets report the omitted figure count.

**Settings page** (main menu or pause menu → Settings): Movement **Smooth / Stepped** and
**Subpixel** on/off (Smooth only; greyed under Stepped) apply to the running world on its
next frame, no restart, and are kept per viewer in `user://shell.cfg` (`[animals]`).
Precedence: the config, then the environment at startup; then the viewer's stored choice
as the renderer starts, except for what the environment named for this run
(`CUBARIUM_ANIMALS` names the movement, and with `crisp`/`smooth` the placement;
`CUBARIUM_ANIMAL_SUBPIXEL` the placement), which keeps the environment's value; then any
change made on the page while it runs, which wins over everything. Turn counts and blends
stay in presets, env and config.

From the repository root, compare the same saved world (a slot directory with `meta.json`):

```sh
SAVES=/path/to/saves
SAVE="$SAVES/t256-s7-midday"
CUBARIUM_ANIMALS=classic scripts/run-game.sh -- --saves "$SAVES" --no-autoload
CUBARIUM_ANIMALS=crisp scripts/run-game.sh -- --saves "$SAVES" --no-autoload
CUBARIUM_ANIMALS=smooth CUBARIUM_ANIMAL_AA=2 scripts/run-game.sh -- --saves "$SAVES" --no-autoload
CUBARIUM_ANIMALS=stepped CUBARIUM_ANIMAL_AA=2 scripts/run-game.sh -- --saves "$SAVES" --no-autoload
```

After each app launch choose **Load** and the same slot (`t256-s7-midday` above). Quit
between runs; loading leaves the save unchanged unless you explicitly save over it.
For `cubarium voxel`, build with `cargo build --release -p cubarium`, then run:

```sh
CUBARIUM_ANIMALS=classic target/release/cubarium voxel --config config/desktop/terrarium.toml --load "$SAVE" --sink gpu --gpu-target window
CUBARIUM_ANIMALS=crisp target/release/cubarium voxel --config config/desktop/terrarium.toml --load "$SAVE" --sink gpu --gpu-target window
CUBARIUM_ANIMALS=smooth target/release/cubarium voxel --config config/desktop/terrarium.toml --load "$SAVE" --sink gpu --gpu-target window
CUBARIUM_ANIMALS=stepped target/release/cubarium voxel --config config/desktop/terrarium.toml --load "$SAVE" --sink gpu --gpu-target window
```

For the wide world choose `wide-s1-midday` in both and use
`config/desktop/terrarium-wide.toml` for `cubarium voxel`.
`CUBARIUM_ANIMAL_TURN=free` or a positive integer (headings per turn, e.g. `16`; under
`stepped` only the count applies, its headings are always stepped) and
`CUBARIUM_ANIMAL_SUBPIXEL=0|1` (smooth only) apply on top of any preset, or the config if no preset is
given: e.g. `CUBARIUM_ANIMALS=crisp CUBARIUM_ANIMAL_TURN=free CUBARIUM_ANIMAL_SUBPIXEL=1
scripts/run-game.sh`. Invalid values stop startup with the valid values named.

## Plant rendering per run

`CUBARIUM_PLANTS` overrides `[figures] plants` when the renderer starts:

- `stamped` draws plants in the world's voxels, still, exactly as before.
- `models` (the default since Wrysk picked it, 2026-10-08) draws every trunk and crown cell in the plant pass
  (`crates/cubarium-gpu/src/plants.rs`). At rest it is the stamped picture, pixel for pixel.
  - **Sway:** each plant leans in a breeze. The root stays put; the top moves most; a
    short plant barely moves.
  - **Grow-in:** a plant appearing, growing a model step, changing foliage state or going
    changes voxel by voxel. Each voxel scales in or out from its own centre over
    `[plants] grow_s`, staggered root up (`order = "centre_out"` for axis out). The whole
    change takes `foliage_fade_s`, and the stamp's cell-by-cell foliage cross-fade is
    off.

Any other value stops startup with the valid values named. The startup log line
`cubarium animals: …; plants=…` reports the effective setting. The setting needs a
restart.

The knobs live in `[plants]`; the defaults are in `crates/cubarium/src/voxel/plant_motion.rs`:

- `supersample` (layer samples per pixel axis, 1–4)
- `grow_s`, `order`, `jitter`
- `bend`, `max_tip_voxels`, `bend_exponent`, `lag_s_per_voxel`, `direction`
- `species.<name>` (sway multipliers; `glowcap = 0`)

The breeze is in `[plants.wind]`:

- `period_s`, `rise_s`, `hold_s`, `fall_s`: one gust packet a period, exactly calm
  between packets
- `flutter`, `flutter_s`, `peak_vary`, `peak_s`
- `sweeps`: whole periods the gust front takes to go round the ring
- `flutter_waves`
- `shower_gain`, `storm_gain`: rain strengthens it

From the repository root:

```sh
SAVES=/path/to/saves
CUBARIUM_PLANTS=models scripts/run-game.sh -- --saves "$SAVES" --no-autoload
CUBARIUM_PLANTS=stamped scripts/run-game.sh -- --saves "$SAVES" --no-autoload
```

With `cubarium voxel`:

```sh
CUBARIUM_PLANTS=models target/release/cubarium voxel --config config/desktop/terrarium.toml --load "$SAVE" --sink gpu --gpu-target window
```

Plants behind water (a fall, a pool's surface) are drawn still by the walk, under the water's
colour. Cells in water are drawn the same way.

## Run headless (no window)

```sh
godot --headless --path game -- --seed 7     # founds the desktop terrarium, prints every 5 s
godot --headless --path game --script res://tests/smoke.gd -- --out /tmp/smoke-save
```

`main.tscn` founds a world from `config/desktop/terrarium.toml` (a drawn seed without
`--seed`, redrawn while its habitat is refused, which can take a few tries), runs it and
prints the summary and populations. The smoke test draws two terrain candidates, founds and
runs one, steps 100 ticks, queries, takes a view, saves and loads; it prints `PASS`/`FAIL` lines and
exits non-zero on a failure. `--headless` never opens a window; anything that needs one goes
through `scripts/hidden.sh`.

**Dev metrics** (F3, or the pause menu's Dev metrics; back or Esc closes it): the study
recorder's per-minute rows, the same numbers `voxel_census vitals=` writes and the
training harness reads (`query("vitals")`, `crates/cubarium-session/src/vitals.rs`), as an
animal table by lineage with grouped columns, a plant table, and line charts. Every column
header's tooltip is the column's meaning and unit. Rows start fresh at each load and keep
`[vitals] span_s` (default 6 h); `[vitals] enabled = false` turns the recorder off.
`tests/metrics_test.gd -- --open SAVE` checks it headless (without `--open` it founds a
terrarium first); with `--shots DIR` under `scripts/hidden.sh` it takes two pictures.

**Field guide** (G, the pad's Y, or the pause menu's Field guide; back or Esc closes it to
where it was opened; it never changes the pause state): `ui/field_guide/field_guide.tscn`,
laid out in the editor from `ui/neon/` and opened there it shows round 3's Lanternjaw page.
The order, swatches and text come from `data/field_guide.md`; sizes, diets, prey and
eaten-by from `query("guide")` (the running world's config,
`crates/cubarium-session/src/guide.rs`); counts from `populations`, the 36 bars from
`history`, the clock from `clock`. List rows and chips are `species_row.tscn` and
`species_chip.tscn`; plates come from `scripts/make-field-guide-plates.sh`. Inside it the
pad's Y is Show in world. `tests/guide_test.gd -- --open SAVE [--shots DIR]` checks it.

`tests/quit_test.gd -- --saves DIR --at draw|founding|running [--via menu|tree]` quits while
terrains are drawn, while the pick is founded, or with a world running. **Every test here
must exit 0, not 139**: the extension stops and joins its own threads before Godot unloads
it (sessions, renderers, the sky-plane and sun-visibility workers; a backstop stops any
still alive at the extension's deinit), and it is linked `-z nodelete` so the process-wide
worker pools it cannot join (water, bevy tasks) and cancelled draws end with the process.
A SIGSEGV after the last line means a thread was left running in the unloaded library.

## Making saves without Godot

`cubarium-world` is a headless Rust tool in `cubarium-session`, with no GPU or
presentation dependencies. Build and run it from the repository root:

```sh
cargo build --release -p cubarium-session --bin cubarium-world
target/release/cubarium-world found --config config/desktop/terrarium.toml --seed 7 --out /tmp/world
chrt -i 0 nice -n 19 scripts/run-study.py --slot 0 -- target/release/cubarium-world advance \
  --threads 8 --load /tmp/world --stop dawn --stop midday --stop dusk --stop night --out-prefix /tmp/day
```

`found --spec world.toml` (or `.json`) accepts the same `WorldSpec` keys listed
below; unknown keys fail. Spec paths are relative to the working directory.
`--seed` overrides its seed; `--empty` leaves out plants and animals.

`advance` steps unpaced using the normal simulation step. Stops run in argument
order: `tick:N` is an absolute world tick, `minute:M` is absolute simulated minutes
since tick 0, rounded up to a tick. `HH:MM` means the next crossing on the 24-hour
day clock, strictly after the loaded state or preceding stop. Named stops use
the world's sunrise and sunset phases: `dawn` is sunrise, `midday` is the midpoint
of daylight, `dusk` is sunset, and `night` is the midpoint of the following night.
Clock stops wrap as needed and require cycle mode; local/off clocks cannot be
advanced by simulation ticks. Clock saves land within one tick of the target.

Every stop writes a normal game-loadable save with `meta.json`, e.g.
`/tmp/day-dawn`, `PREFIX-tick-2000`, `PREFIX-minute-2.5`, or `PREFIX-06-30`.
Duplicate names and stops earlier than the current tick fail. Progress prints
tick, simulated seconds and ms/tick every three seconds; final timing excludes
load/save time. `--threads N` works on either command (0 uses config/default), and
all workers inherit the launcher's CPU affinity without widening it. See
`cubarium-world --help` and `cubarium-world advance --help`.

## `CubariumSession`

`CubariumSession.new()` is an empty session. Every call returns at once; drawing candidates,
founding, loading and saving run off Godot's thread and report through `status()`,
`candidates()`, `founding()` and `take_notices()`. Paths are filesystem paths
(`ProjectSettings.globalize_path(...)`).

**Terrain first** (2026-10-08): a candidate is a bare terrain — generated, lake-gated and
checked for room for the founders, nothing founded — so a batch takes seconds and a reroll
is cheap. Only the picked one is founded (`promote`), on its own terrain: the founding never
changes it. A refused placement is tried again on the same terrain with the weather and the
keyed draws re-keyed (up to 3 tries, fewer when two tries running are refused for the same
reasons); when every try is refused the founding fails with a plain message and the
candidates stay to pick from.

**Seeds are Strings** across the binding: Godot's ints are signed 64-bit and seeds are
unsigned. Every `seed`/`placement_seed` returned is a decimal String; a spec's `seed` may be
a String (up to 18446744073709551615) or a non-negative int.

| call | what |
|---|---|
| `generate_candidates(spec: Dictionary, count: int) -> String` | draw `count` terrains in parallel (candidate `k` takes `seed + k` when a seed is given); `""` or why not. Spec keys: `config`, `size` `[w,h,d]`, `terrain`, `climate`, `seed`, `sets`, `empty`, `water_level_m` (a terrarium's water table; the picker's Low/Medium/High are 0.375/0.75/1.125 m); unknown keys refused |
| `regenerate_candidates(spec: Dictionary, seeds: Array) -> String` | one candidate per seed (`null` draws one): the same terrains again, e.g. at another water level |
| `candidates() -> Array` | `index`, `state` (generating/ready/failed/cancelled), `seed`, `label`, `error`, `summary`, `populations` (empty until founded), `terrain` (hash of the terrain), `feasible` (whether the founders could all be placed on the bare terrain; a drawn terrain is offered only when true), `notes` |
| `candidate_ref(i: int) -> Dictionary` | a ready candidate's `source` (this session) and `candidate` (i), all the renderer's world view needs to draw it (`show_view`); empty while it is drawn. Converts nothing, so it costs Godot's thread nothing |
| `candidate_view(i: int) -> Dictionary` | a ready candidate's whole view (every chunk, for the legacy mesh view and the stub; slow on the wide world, so the picker uses `candidate_ref` with the renderer), also naming its `source` and `candidate` |
| `promote(i: int) -> String` | found candidate `i` on its own terrain and run it once founded (the rest are dropped then); `""` once started or why not |
| `founding() -> Dictionary` | `state` (idle/founding/done/failed/cancelled), `candidate`, `seed`, `terrain`, `phase` (settling, shower, draining, lake_rest, stream_watch, placing, starting), `phase_label`, `progress` (0–1 through the current try), `attempt`/`attempts`, `placement_seed`, `elapsed_s`, `error`, `label` |
| `cancel()` | stop the terrains being drawn and the founding in flight; ready candidates stay |
| `workers() -> int` | candidate and founding threads still running |
| `load(dir: String)` / `save(dir: String)` / `close()` | a save is a directory with `meta.json`; a save from another version is refused (`load_failed` notice) |
| `command(line: String)` | the console (`p`, `s N`, `speed X`, `r`, `i X Y Z`, …) |
| `pause()` `resume()` `set_speed(x: float)` `step(n: int)` | run control |
| `status() -> Dictionary` | `running`, `paused`, `speed`, `tick`, `generation`, `loading`, `saving`, `label`, `last_error`, `step_ms`, `feed_ms` |
| `query(name: String, args) -> Variant` | `summary`, `clock`, `environment`, `populations`, `run`, `snapshot`, `animals`, `animal` `{id}`, `history` `{since}`, `guide` (per species: body, diet with sources, prey, height, eaten-by, from the running world's config), `species`, `status`, `candidates`, `founding`, `spec` `{spec}` (its landform kind, `water_level_m` and the water steps), `saves` `{dir}`; `{"error": …}` on a refusal |
| `take_view() -> Dictionary` | see below |
| `take_events() -> Array` | life events since the last take: `tick`, `animal`, `kind` (born, meal, kill, injured, died, …) and the kind's fields |
| `take_notices() -> Array` | console replies; `saved`/`save_failed`/`loaded`/`load_failed` with `path` and `meta` |

`WorldRenderer` (`crates/cubarium-godot/src/renderer.rs`) is the Rust renderer for a world
view: `start(config, root, screen, fps)`, `attach(session)` / `detach()` (it hooks into the
session's loop), `show_candidate(session, i)`, `take_frame()`, `set_view_x`,
`set_highlight`, `pick`, `animal_at`, `stats`, `release`. `world/world_view.gd` is its one user.

## The view

`take_view()` returns what changed since the last take — every chunk on the first take after
a world is installed (`full`), and whenever `generation` moves (a load or a promotion: drop
everything). Changes between takes are unioned, so a slow reader never misses one.

- `dims`, `chunk_dims` (Vector3i), `chunk_size` (16), `voxel_m`.
- `chunk_coords: PackedInt32Array`, three per chunk `(cx, cy, cz)`; `voxels: PackedByteArray`,
  `3 · 16³` bytes per chunk in the same order: a material plane, a free-water plane and a
  pore-water plane, each `x` fastest, then `y` (up), then `z` (depth):
  `(lz · 16 + ly) · 16 + lx`. Material: 0 air, 1 bedrock, 2 rock, 3 soil. Water: 0–255, the
  void (free) or pore fraction full; free water is never 0 in a cell that holds any. Cells
  past the world's edge read as air.
- `tick`, `animals` (`ids`, `species`, `pos` (x, y, z metres, three each), `heading`,
  `state`, `mobility`, `stage`, `flags`, `size`), `plants` (`ids`, `species`, `site`
  (x, y, z voxels), `growth` (0–1), `foliage`), `sky` (`day`, `day_phase`, `hour`,
  `daylight`, `sun_elevation`, `cloud_cover`, `fog`, `rain`, `rain_m_per_s`,
  `temperature_c`, `rh`, `strike`).
- What the ids and codes mean: `query("species", null)` (lineages, plant species, states,
  mobility modes, life stages, flag bits, materials). A new species is a new row there, not a
  binding change.

The additional view columns are packed as follows. `I32`, `I64`, `F32`, `F64` and `Byte`
below mean Godot's corresponding `Packed*Array`. Animal, plant and carcass columns are
complete newest-tick lists on every take with a frame; absent ids remove the old records.

- `animals.flags` adds `starving` (named in `species.flags`), using the legacy presenter's
  reserve/body condition. `animals.riding_host: I64` holds the host id, or −1.
- `animals.wall_toward: I32` has two values per animal `(dx, dz)`, the unit direction into
  its wall; `(0, 0)` without a wall climb. Heading describes travel, and on descent the
  wall is behind that direction. These are world x/z directions, not face codes.
- `plants.moisture: F32` is the simulation's last moisture factor (0–1).
  `plants.wilt: Byte` is the legacy quantised wilt level, named by `species.wilt_levels`
  (0 turgid, last level fully wilted).
- `plants.ripeness: F64` is the legacy accent input: nectar present (0/1), else
  fruit/full fruit capacity (0–1), else parcel/propagule package (may exceed 1).
  `species.plant_ripe_at` names the shared threshold at which the bloom opens.
- `plants.parcel_fullness: F64` is parcel/propagule package (0 for a zero package);
  this remains the seed parcel even for nectar/fruit species.
- `plants.foliage_offsets: I32` has plant-count + 1 entries, beginning with 0.
  Plant i's model layer indices run bottom-up through
  `[offsets[i], offsets[i+1])` of `foliage_stock: F64` and `foliage_capacity: F64`.
  Divide stock by capacity, or use 0 for nonpositive capacity, for the exact fraction
  the old `keeps` rule reads. Entries include all supported model indices (currently 4);
  an index absent from the current growth stage carries whole-stand stock/capacity,
  preserving the old fallback when a neighbouring model step has another layer.
- `carcasses` has `ids: I64`, `species: I32` (animal lineage ids), `pos: F32`
  (three metres x/y/z each, y at the support cell's top), `heading: F32` (radians),
  `state: Byte` (named by `species.decay_states`), `remaining: F32` (carrion/original
  organic matter, clamped 0–1; decreases as decay advances), and `organic: F32`
  (original organic matter for body sizing). A carcass id is its support-cell address
  `(y · depth + z) · width + x`, scoped to the generation, **not** the dead animal id:
  the simulation retains no animal identity on a carcass and a replacement at that site
  reuses the address.

`marks` and `vines` are sparse **replacement lists per returned terrain chunk**. Each has
`offsets: I32`, chunk-count + 1 entries beginning with 0; records for `chunk_coords` entry i
are `[offsets[i], offsets[i+1])` in every column (three times that range for `site`).
Replace that chunk's whole list, including with an empty list. Chunks resend when terrain,
water, any mark, or any vine record changes; all share the same union between takes.
Sparse records avoid dense empty surface planes and use the established generation/load
reset contract. Coordinates below name the **support/rock voxel**, in global voxels.

- `marks.site: I32` is x/y/z, three each; `kind: Byte` is named by
  `species.mark_kinds` (log, litter, carrion, glassfilm, seed_bank), `species: I32` is the
  seed mark's plant species or −1, `stock: F32` is organic stock (the named seed species'
  summed bank stock), and `strength: F32` is glassfilm's stock/capacity clamped 0–1,
  otherwise 1. Logs/litter/carrion use the legacy minimum visible stock; seed banks use
  the simulation's recent-landing/sprouting rule. Film strength is the uneased target;
  easing belongs to the presenter.
- `vines.site: I32` is rock x/y/z, three each; `direction: Byte` names its outward face
  via `species.vine_directions`; `owner: I64` is the vine id; `density: Byte` and
  `accent: Byte` are named by `species.vine_densities` and `species.vine_accents`.
  `flags: Byte` is named by `species.vine_flags`: rooted, hanging, emissive (flower
  mouth). The emissive bit identifies the accent's luminous texels, not the whole face.
- `vines.tile: I32` is the shared legacy `VineCell::tile` selection, flattened as
  `row · 256 + column`, where `row = 3 · set + density` and
  `column = 16 · mask + (exits & mask)`. The layout is named by `species.vine_tiles`;
  its accent row is separate, with bud/flower/fruit in columns 0/1/2. The feed retains
  every covered face, including inside corners sharing an air voxel. A face's tile can
  change when a neighbour changes, including across chunk borders.
