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
