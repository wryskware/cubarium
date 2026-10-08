---
status: open
date: 2026-10-07
owner: opus-high worker, worktree `.claude/worktrees/observatory`, branch `observatory` (from main)
---

# Observatory: a running world's stats over HTTP, a companion dashboard, speed control

Wrysk, 2026-10-07: "can we give a running world a little http or websocket server so that we
can have an optional companion dashboard that shows like population stats for all the
species and probably other stuff like environmental conditions, world (diagetic) time, etc.
might be cool also to have a setting in the dash to control run speed multiplier. we can
think of some of this as working towards an in-game UI as well. like ideally, i would be
able to hover/click on an animal and see its stats and life history." Later in the same
message: "all the api/metric stuff i asked for above will just be reused internally as ABI
to the godot harness/UI."

Read `design/handoffs/worker-rules-2026-10-07.md` first.

## Shape (decided)

- **Query layer first, transport second.** A transport-free module (or a small crate) of
  plain Rust functions over the live world that return serde types. An in-game UI or a
  Godot GDExtension shell will call these same functions; HTTP only serializes them.
  - World summary: size, preset, seed, schema, tick.
  - Populations: animals by species and life stage; plants by species (stands and
    biomass or cover, whatever flora already counts).
  - Environment: weather state, rain, atmosphere water and humidity, temperature if
    modelled, light and day phase, dry season, water stocks (lake, aquifer, soil).
  - Diegetic time: world day and time of day from the day clock, plus sim elapsed.
  - Run state: speed multiplier, paused, ms/tick, fps.
  - Animals (checkpoint 2, below).
- **Opt-in.** `--serve ADDR` and a config key; off by default, so the panel pays nothing.
  Put `serve = "127.0.0.1:7878"` in `config/desktop/terrarium.toml` and `"0.0.0.0:7878"` in
  `config/voidrunner/terrarium.toml` (its dashboard is opened from the desktop on the home
  LAN; no auth).
- **Never stall the sim.** The server has its own thread. The loop publishes a snapshot about
  once a sim-second (counts, not voxels) and drains commands at its existing command point.
  The server never locks the world mid-step. Measure the loop's added cost; it should be
  near zero.
- **Commands** go through the same path as the stdin console (`COMMANDS` in
  `crates/cubarium/src/voxel/mod.rs`: `p`, `+/-`): set the speed multiplier (within
  `MIN_SPEED..MAX_SPEED`), pause and resume. Nothing else yet.
- **Transport**: JSON over HTTP plus a push stream (WebSocket or SSE, your call). Smallest
  dependency footprint; no async runtime unless it pays for itself, and say why.
- **History**: the server keeps a ring buffer of the population and environment series (a
  sample every few sim-seconds over the last several sim-hours, both knobs), so a dashboard
  opened late still sees the past.
- **Dashboard**: one static page embedded in the binary, served at `/`. Species population
  charts over time (animals by stage, plants), an environment panel, the world clock, and a
  speed control (presets, slider and pause). Dark, clean, legible from across the room. It
  is a dev tool beside the world, not in it (no analytical UI in the world display). Load
  the `dataviz` skill before the charts.

## Checkpoint 2: animals

- `/animals` (list) and `/animals/{id}`: species, stage, age, sex, energy and hydration,
  position, current behaviour or goal, parents, offspring.
- Life history: a bounded per-animal event log (born, matured, meals by kind, hunts and
  kills, injuries, births, den moves, death and its cause), recorded on the observer side
  from events fauna already emits. Add sim state only if there is no other way, and say so.
- Dashboard: a sortable animal table; clicking a row opens a detail panel with the history.
- Hover/click on an animal in the world window belongs to the app-shell thread
  (`design/handoffs/app-shell-2026-10-07.md`), not here.

## Checks

- Unit tests of the query functions on a tiny hand-built world (mechanism, not numbers).
- Headless: run `cubarium voxel --config config/desktop/terrarium.toml` with a headless GPU
  target and `--serve 127.0.0.1:<port>` for a few sim-minutes; curl every endpoint; set the
  speed and see the loop follow; screenshot the dashboard with headless Chrome or Chromium.

Commit at checkpoint 1 (server, summary, populations, environment, clock, speed, dashboard)
and go straight on to checkpoint 2.

## Return

Commits, the endpoint list, the transport choice and why, the loop's added cost, the
dashboard screenshot paths.
