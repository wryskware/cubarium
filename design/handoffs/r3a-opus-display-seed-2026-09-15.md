---
design_status: exploration
last_reviewed: 2026-09-15
---

# R3a (Opus): a seeding door for trained policies on the display cube

Wrysk wants to *see* the first trained forager on the cube. This brief adds the one
control that makes that possible and reusable: a launch flag that seeds a fresh world
with N copies of the training animal, each running a named policy file. Fable
orchestrates; you own all code and the development runner for this assignment.

Time target: about 45 minutes of your wall time. Do not widen scope.

## Objective

`cubarium run --fresh --neural <policy.json> [--neural-count N]` creates the ordinary
legacy world **and adds** N (default 4) neural animals to it at tick 0: the exact body
the trainer uses (`Genome::founder(FOUNDER_HUE, &cfg.drives)`, unit adult, the same
reserve/energy start fractions as `crates/cubarium-search/src/es/fixture.rs` `place`),
each with the loaded policy attached through `World::attach_neural_policy`. Then reset
`state/` and relaunch the cube with `runs/es-r2c-min64/centers/center-00059.json`.

## What exists (read these first)

- `crates/cubarium-core/src/world/view.rs:327` `World::attach_neural_policy(id, Policy)`:
  the only door in. Refuses apex members, a foreign digest, and a world with the quiet
  extension enabled (it is `Off` by default; `crates/cubarium-core/src/quiet.rs:4`).
- `crates/cubarium-search/src/es/export.rs` `PolicyFile` (hex LE IEEE-754 weights,
  digest, generation, protocol hash) with `PolicyFile::policy() -> Result<Policy, String>`.
  `crates/cubarium-search/src/es/bits.rs` decodes the hex. The crate enables
  serde_json `float_roundtrip`.
- `crates/cubarium-search/src/es/fixture.rs:193` `Layout::build` and `:251` `place`:
  how the trainer founds the animal (genome, `decode`, `START_RESERVE`/`START_ENERGY`
  fractions, `Origin::Founder`, `external_material_in += structure + reserve`).
- `crates/cubarium/src/cli.rs:119` `Run` (clap) and `:220` `validate`;
  `crates/cubarium/src/runner/mod.rs:126` `open_world` (fresh world is created at the
  bottom via `World::new(config)`), `:344` `run_world_until` (the `--fresh` guard).
- `scripts/run-cube.sh`: builds and launches; extra args are forwarded to `cubarium run`.
- Contract: `design/recurrent-interface-contract.md` §5 (persistence, digest) and §9.
- The neural runtime already gives an ordinary child its parent's policy with fresh
  hidden state, and snapshot schema 15 persists neural state, so resume works unchanged.

## Deliverable

1. **Core door** in `cubarium-core`: one function that founds the training body at a
   position/heading with a policy, e.g.
   `World::found_neural_animal(pos: SurfacePoint, heading: Vec2, policy: Policy) -> Result<OrganismId, String>`
   (name is yours). It must produce byte-for-byte the organism `fixture.rs` `place`
   produces (same genome, phenotype, structure, reserve, energy, born_tick = current tick,
   `Origin::Founder`, same `external_material_in` booking) and then attach the policy.
   Make `fixture.rs` `place` delegate to it (or to a shared founding helper it wraps) so
   there is one founding routine, not two. `FOUNDER_HUE`, `START_RESERVE`, `START_ENERGY`
   move to core with the helper (re-export from `es::fixture` so nothing else changes).
   Capacity: refuse when `capacity.max_organisms` would be exceeded.
2. **Policy reader**: the runner needs `PolicyFile`. Prefer adding
   `cubarium-search = { workspace = true }` to `crates/cubarium/Cargo.toml` and using
   `cubarium_search::es::export::PolicyFile` as is. Move it to core only if the
   dependency is refused by the workspace for a concrete reason; say which.
3. **Launch control** in `crates/cubarium/src/cli.rs` + `runner/mod.rs`:
   `--neural <path>` and `--neural-count <N>` (default 4, must be ≥ 1). Applies only when
   a **new** world is created. On a resumed world, `--neural` is refused with a message
   that names `--fresh` (a seeding control must not double-seed a live world). Placement:
   copy k goes to the centre of face k mod 5 (faces cycle), heading east in that face's
   chart, so 4 copies land one per side face; if a copy's start cell is not clear
   ground, use the nearest clear cell and say so on stderr. Log one stderr line at
   launch: `cubarium: seeded N neural animals from <file> (generation G, digest 0x…)`.
4. **Status**: the web mirror `/status` JSON (`crates/cubarium/src/net.rs` or wherever
   it is assembled) gains `neural_animals` = count of live neural animals. Nothing else in
   the presenter changes: neural animals are ordinary organisms and draw as such. Verify,
   do not assume: a 20 s headless or web-mirror run must show them present and moving.
5. **Reset and launch** (Wrysk has authorised resetting `state/` at every major change):
   the live runner is pid 2062111 (`cubarium run … --fresh`, launched from a tmux pane).
   Stop it with `kill 2062111` and wait for exit, move `state/*` aside or delete it
   (`state/` must be empty for `--fresh`), then
   `nohup ./scripts/run-cube.sh --fresh --neural runs/es-r2c-min64/centers/center-00059.json > runs/cube-neural.log 2>&1 &`
   and confirm via `curl -s localhost:7393/status` that `neural_animals` is 4 and the
   tick advances. The shim in `~/vuzic/led-cube-shim` is not yours; do not touch it.
6. **Result note** `design/7_Research/r3a-display-seed-result-2026-09-15.md`: what was
   built, the evidence below, and what the first minute on the cube showed (population,
   neural count over time, anything odd).

## Tests (specified here; write them before or alongside the implementation)

- `cli.rs`: `--neural` / `--neural-count` parse; `--neural-count 0` is refused;
  `--neural` without `--fresh` is accepted by the parser (the runtime refuses on resume).
- Core: founding through the new door then reading the organism back equals what
  `fixture.rs` `place` produced before your change, field for field (pin the expected
  structure/reserve/energy numbers, not just "equal to itself").
- Runner integration: a fresh world in a scratch `--state` with `--neural <fixture policy>`
  and `--neural-count 3` holds exactly 3 neural animals after `run --seconds 2 --sink null`
  (or whatever the headless sink is), and a second launch on that state **without**
  `--fresh` and **with** `--neural` is refused.
- Determinism guard for the delegation: `runs/es-r2c-eval/min64-g59-holdout.json` was
  produced by `es-evaluate` before your change. Rerun
  `target/release/cubarium-search es-evaluate --policy runs/es-r2c-min64/centers/center-00059.json --set holdout --out /tmp/…/holdout-after.json`
  and show that every per-episode `ticks`, `terminal_stores` and intake number is
  identical (only `build`/`wall_seconds` may differ). Also `cargo test -p cubarium-search`
  and `cargo test -p cubarium-core` green, and `cargo test -p cubarium` green.

## Constraints

- Only these files are yours: `crates/cubarium/**`, `crates/cubarium-core/src/**` (new
  door + moved constants only), `crates/cubarium-search/src/es/fixture.rs` (delegation
  only; the layouts, their hash text and every other search file are frozen), the new
  result note, `Cargo.lock` if the dependency changes it.
- Do not touch the trainer, `episode.rs`, `commands.rs`, `main.rs` in cubarium-search
  (Fable is editing `commands.rs`/`main.rs` concurrently).
- One build cache (`target/`). Fable is also running cargo; if the lock is held, wait.
  A 20-worker training job is running in the background; do not kill any
  `cubarium-search` process.
- Never `git add -A`, never stash, never touch files you do not own. Uncommitted work in
  `.claude/`, `design/handoffs/`, `design/*.md`, `.agents/`, `design/7_Research/assets/`
  belongs to others; leave it exactly as is.
- Commit your files when green with a message ending in
  `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
- Known caveat, not a bug to fix: the policy's `reproduce` output is untrained and the
  live world does not script `bud = false`, so the seeded animals may bud; children
  inherit the policy. Report whether it happened. Do not add a bud gate.

## Return format

Changed files and commit hash; the test names and their output summary; the
holdout-JSON identity diff; the launch stderr line; two `/status` samples 30 s apart
(tick, population, neural_animals); what the cube showed; anything you decided that the
brief left open, with the reason; your measured wall time.
