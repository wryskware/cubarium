# Cubarium in Godot (`game/`)

The Godot 4.7 app. Rust is the simulation (`crates/cubarium-session`); this project is
everything you see and touch (D-0004, `design/handoffs/godot-direction-2026-10-07.md`).
Package A, the state of this directory: the extension, a bare project and a main scene that
prints. Nothing is drawn yet.

## Build

```sh
scripts/build-godot-ext.sh            # debug
scripts/build-godot-ext.sh --release  # release: founding a world is much faster
```

This builds `crates/cubarium-godot` (gdext 0.5.5, Godot API 4.7), copies the library to
`game/bin/`, where `cubarium.gdextension` points, and registers the extension in
`game/.godot/extension_list.cfg` so a headless run loads it without the editor ever having
opened the project.

## Run headless (no window)

```sh
godot --headless --path game -- --seed 7     # founds the desktop terrarium, prints every 5 s
godot --headless --path game --script res://tests/smoke.gd -- --out /tmp/smoke-save
```

`main.tscn` founds a world from `config/desktop/terrarium.toml` (a drawn seed without
`--seed`, redrawn while its habitat is refused, which can take a few tries), runs it and
prints the summary and populations. The smoke test founds two candidates, promotes one,
steps 100 ticks, queries, takes a view, saves and loads; it prints `PASS`/`FAIL` lines and
exits non-zero on a failure. `--headless` never opens a window; anything that needs one goes
through `scripts/hidden.sh`.

## `CubariumSession`

`CubariumSession.new()` is an empty session. Every call returns at once; founding, loading
and saving run off Godot's thread and report through `status()`, `candidates()` and
`take_notices()`. Paths are filesystem paths (`ProjectSettings.globalize_path(...)`).

| call | what |
|---|---|
| `generate_candidates(spec: Dictionary, count: int) -> String` | found `count` worlds in parallel; `""` or why not. Spec keys: `config`, `size` `[w,h,d]`, `terrain`, `climate`, `seed`, `sets`, `empty`; unknown keys refused |
| `candidates() -> Array` | `index`, `state` (generating/ready/failed), `seed`, `label`, `error`, `summary`, `populations` |
| `candidate_view(i: int) -> Dictionary` | a ready candidate's whole view (for thumbnails and panning) |
| `promote(i: int) -> String` | run candidate `i`, drop the rest |
| `load(dir: String)` / `save(dir: String)` / `close()` | a save is a directory with `meta.json`; a save from another version is refused (`load_failed` notice) |
| `command(line: String)` | the console (`p`, `s N`, `speed X`, `r`, `i X Y Z`, …) |
| `pause()` `resume()` `set_speed(x: float)` `step(n: int)` | run control |
| `status() -> Dictionary` | `running`, `paused`, `speed`, `tick`, `generation`, `loading`, `saving`, `label`, `last_error`, `step_ms`, `feed_ms` |
| `query(name: String, args) -> Variant` | `summary`, `clock`, `environment`, `populations`, `run`, `snapshot`, `animals`, `animal` `{id}`, `history` `{since}`, `species`, `status`, `candidates`, `saves` `{dir}`; `{"error": …}` on a refusal |
| `take_view() -> Dictionary` | see below |
| `take_events() -> Array` | life events since the last take: `tick`, `animal`, `kind` (born, meal, kill, injured, died, …) and the kind's fields |
| `take_notices() -> Array` | console replies; `saved`/`save_failed`/`loaded`/`load_failed` with `path` and `meta` |

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
