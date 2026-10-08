---
status: open
date: 2026-10-08
---

# Dev metrics panel in the Godot app (2026-10-08)

Rules: `design/handoffs/worker-rules-2026-10-07.md` (all of it applies), AGENTS.md,
WORKING_POLICY.md. Worktree `.claude/worktrees/dev-metrics`, branch `dev-metrics`.
Seed the worktree's own `target/` by reflink from main's, only the profile directories
you build (main's `target/` is 557 GB; don't copy all of it).

## Objective

Wrysk wants to watch a live world in the Godot app with **the same numbers the training
harness reads**. Those come from the vitals recorder
(`crates/cubarium-observatory/src/vitals.rs`: `Recorder`, `AnimalVitals`, `PlantVitals`,
`VITALS_HEADER`, `PLANTS_HEADER`), which today only `crates/cubarium/examples/voxel_census.rs`
drives (it writes `vitals.csv` / plants rows; `scripts/studies/score.py` reads them).
The Godot session (`crates/cubarium-session`) does not run it.

This is a **developer** panel. A later pass with Wrysk picks a viewer/player subset; don't
design that here.

## Deliverable

1. **Session.** Run `vitals::Recorder` inside the session the way `voxel_census.rs` does
   (same `Options` defaults, same observe/sample/minute cadence, so a row means the same
   thing in both places). Keep the per-minute animal and plant rows in a bounded ring in
   the hub (span like `HistoryConfig::span_s`, a config field). Add a `vitals` query
   (`{"since": minute}` optional) returning both row kinds plus the column list. Add it
   to the unknown-query error's list. Rows start fresh at load (not saved); the query
   says from which minute it holds rows.
   - Keep the sim tick cost honest: measure step ms with and without the recorder on
     terrarium and wide (`config/desktop/`), report the numbers. If it costs more than
     ~0.3 ms/tick, move the work off the sim thread or onto the publish cadence and say how.
2. **Godot.** A dev metrics overlay under `game/ui/dev/`, opened from the pause menu and
   a key (pick one that's free in `main.gd`'s `_actions()`), closed with back/escape:
   - a table per minute for animals (one row per lineage) with grouped columns:
     population by stage; condition (fullness p10/p50 per stage, starving per stage);
     activity shares; energy and budget; movement; stuck. A plants table (stands, vigour
     p10/p50, stressed counts). Every column header has a tooltip giving its meaning and
     unit, taken from the doc comments in `vitals.rs` (and `score.py` where it reads it).
   - a line chart: pick a lineage and a column, see it over the ring; plus all lineages'
     population counts on one chart. Custom `Control._draw()` is fine.
   - Series colours: if the session exposes species colours, use them; otherwise an
     interim distinguishable set, named as interim in the code.
   - Look: `game/ui/theme.tres` as it is (the dev theme). **No new look**: no fonts,
     palettes, borders or ornament. The real UI design language is being chosen with
     Wrysk in another thread and will restyle this later, so use theme type variations,
     not inline style overrides.
   - Don't re-implement score.py's terms (F, S, trends, feasibility). Raw vitals only.

## Tests (write these; they are specified here on purpose)

- Session: once the recorder has passed two minute boundaries, `vitals` returns one row
  per live lineage carrying every `VITALS_HEADER` field, plants rows carry every
  `PLANTS_HEADER` field, and `since` filters. Stay within a few hundred ticks (drive the
  recorder's minute directly or shorten the minute in the test config).
- Ring: rows older than the span are dropped.
- Godot (headless, extend `game/tests/shell_test.gd` or add one): the panel opens from
  the pause menu and from its key, lists every lineage the query returns, closes with
  back, and logs no errors.
- No asserts on tuned values, populations or ratios.

## Verification

`timeout 270 cargo nextest run -p cubarium-session -p cubarium-observatory`; the Godot
tests headless; two headless screenshots of the panel on a running terrarium (table and
chart), absolute paths. Never build into the main checkout's `game/bin` (Wrysk may be
running the app).

## Return (≤40 lines)

Commits, the tick-cost numbers, where the recorder runs and why, the key you picked,
screenshot paths, what looks wrong or is missing.
