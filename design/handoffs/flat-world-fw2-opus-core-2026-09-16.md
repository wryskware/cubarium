---
design_status: exploration
last_reviewed: 2026-09-16
---

# FW-2 (Opus, high): topology and scale through the world (`cubarium-core`)

Read, in order: `design/7_Research/flat-world-fw1-2026-09-16.md` (the frozen
surface API you build on; note `cell_of`, `embed`, `cell_count` take a `Scale`),
`design/flat-world-plan-2026-09-16.md` §4 (persistence, `CubeProjection`),
§5 and §5a (height, the cylinder, weather unchanged, canopy `downhill`),
§9's FW-2 row, and `WORKING_POLICY.md`. Fresh context. No nested agents.

## Objective

A ring world runs: `WorldConfig` chooses the topology and scale at `--fresh`,
every cube-hardcoded height/embedding/target/founder site in core reads the
topology, schema 17 refuses older worlds, and a cube world's behaviour is
proven unchanged by the `CubeProjection` comparator against a fixture from
`main`.

## Where and what you own

Worktree `/home/wrysk/wryskware/cubarium/.claude/worktrees/tachyon-screen`,
branch `tachyon-screen`. You own `crates/cubarium-core/**` except the
reserved FW-6 paths `crates/cubarium-core/tests/{ring_world,ring_weather,ring_schema17}.rs`.
FW-3 owns `crates/cubarium-render/**` concurrently; FW-6 writes only the
reserved test files; GS-1 owns `crates/cubarium-gpu/**`. Do not edit their
paths; mechanical follow-through of your own signature changes into
`crates/cubarium/**` (host) is allowed only if the workspace would not
compile otherwise, listed in the report. `git add` your own paths only;
never `commit -a`. Rebase onto the branch head as others land.

## Deliverable (plan §4/§5/§5a/§9 are normative; checklist)

1. `WorldConfig.{topology: Topology, world_scale: Scale}`, validated via
   `Topology::validate(scale)`, `CONFIG_VERSION` 8 → 9, schema 17;
   `decode_snapshot` refuses 7..=16 by name; `WorldState::validate` extended
   per §4 (topology, dimensions, every organism and care `face`, field
   vector lengths, shower cell ids `< cell_count`).
2. `World::topology()`, `World::scale()`, `RenderView.topology` (+ scale);
   the five fixed-size weather/habitat caches sized at runtime.
3. Every `embed()[1]` height read → `Topology::height` (habitat light and
   moisture, both controllers' height channel, `up_direction`, depth
   preference); noise sampled at `embed(scale, p)`; weather unchanged
   (§5a: it runs on the normalized cylinder embedding; no draw changes,
   assert RNG stream parity on the cube).
4. Ring `downhill` consumed where the cube's is; founders on one chart
   (keep and discard the face draw, per §5, so the cube's stream is
   unchanged); `blobs_per_channel` stays 3.
5. Core `CareTarget::resolve` and `HunterTarget::resolve` topology-aware;
   targets beyond pixel 63 resolve on a ring and are refused off-world.
6. `CubeProjection` (§4: `WorldState` verbatim with `config` minus
   `version`, `topology`, `world_scale`), the frozen `v16.rs` mirror, an
   exporter, and the comparator test that reads the fixture. The fixture:
   one snapshot from an unmodified `main` build (`git worktree` of `main`
   at 2a1cedd or later is fine; do not touch the main checkout's working
   tree) after a fixed-seed run of N ticks, plus the same run on your build,
   equal by projection.
7. A ring world reaches steady state: `--fresh` ring 320×180 S = 1 and
   640×360 S = 2, 3,000 ticks headless each, population and field
   summaries in the report; the same for the cube, unchanged.

## Verification you owe

Existing core tests green unchanged (the ones that pin `CELL_COUNT`
literals may use `CUBE_CELL_COUNT`); RNG parity test; the projection
comparator; ring runs at both scales; `cargo test --workspace` and clippy
clean. Commit small on `tachyon-screen`, trailer
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. No merge, no push.

## Return

Report at `design/7_Research/flat-world-fw2-2026-09-16.md` and as your final
message: the config surface (TOML example), every height/embedding site
converted (file:line), the projection result, the ring steady-state numbers,
files touched outside the crate, anything in §4/§5 that proved wrong.
