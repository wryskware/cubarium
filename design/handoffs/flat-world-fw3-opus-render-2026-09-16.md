---
design_status: exploration
last_reviewed: 2026-09-16
---

# FW-3 (Opus, high): `cubarium-render` by topology, plus the CPU levers

Read, in order: `design/7_Research/flat-world-fw1-2026-09-16.md` (frozen
surface API), `design/flat-world-plan-2026-09-16.md` §3 and §9's FW-3 row,
`design/7_Research/presenter-budget-2026-09-16.md` (W1, W2, W7 and the
row-band split are yours), and `WORKING_POLICY.md`. Fresh context. No nested
agents.

## Objective

The render crate draws on either topology into a topology-shaped `Canvas`,
encodes a ring canvas to `cube_proto::Raster`, adopts the stamp budget
`Scale::footprint_radius()`, and gets the three mechanical speedups and the
deterministic row-band hook FW-P costed, with the cube's output bit-identical.

## Where and what you own

Worktree `/home/wrysk/wryskware/cubarium/.claude/worktrees/tachyon-screen`,
branch `tachyon-screen`. You own `crates/cubarium-render/**` except the
reserved FW-6 paths `crates/cubarium-render/tests/{ring_canvas,ring_stamp_scale}.rs`.
FW-2 owns `cubarium-core`, GS-1 owns `cubarium-gpu`; the host crate is
FW-4/FW-5's later, but mechanical follow-through of your signature changes
into `crates/cubarium/**` is allowed if the workspace would not compile
otherwise (list it). `git add` your own paths only; never `commit -a`.
Rebase onto the branch head as others land.

## Deliverable

1. `Canvas::new(topo, scale)` sized from the topology (cube: five 64×64 as
   today; ring: one `w×h`), `pixels()`/`pixels_mut()` for the ring,
   `encode` for the cube `Frame` unchanged, `encode_raster(&self, &mut
   Raster)` for the ring (nearest, sRGB). Port `field`, `trail`, `sprite`,
   `body`, `multipart` to take the topology and scale.
2. `FOOTPRINT_RADIUS` → `Scale::footprint_radius()` at both check sites
   (`sprite.rs:18-22,61-64` and the silent stamp-time rejection
   `sprite.rs:810-818`); a `scale = 2` stamp draws instead of vanishing.
3. FW-P's levers, each measured before/after with
   `crates/cubarium/examples/presenter_budget.rs` on the desktop:
   W1 a per-topology pixel→cell (+4 neighbours) table built once;
   W2 a per-slot `unfold_pixels` cache keyed by anchor and radius, bucketed
   by row band, invalidated only when the anchor set changes; W7 a
   table-driven `srgb_encode` (4,096-entry interpolated, or exact 256-entry
   inverse where the input is already quantised) that is **bit-identical**
   to the current output on every code (test all 256 round-trips and a
   sweep). Also W2b (no per-stamp allocation) and W5 (reject dry pixels
   before the `exp`).
4. The deterministic row-band hook: `Canvas` split into N horizontal bands
   with a `for_each_band(|band| ..)` that the presenter can drive from a
   thread pool, where each band owns its pixels and stamps are clipped to
   the band, so the composite is bit-identical to the serial draw for any N
   (test N = 1, 2, 4 against the serial image on both topologies).
5. Measurements: `R` before and after on the desktop, and on the board
   (`taskset -c 4-7`, `render_bench --pin`) for one A78 core.

## Verification you owe

Same-seed cube canvas bit-identical for every change (keep a golden from
before your first commit); `cargo test --workspace` and clippy clean; the
measured table. Commit small on `tachyon-screen`, trailer
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. No merge, no push.

## Return

Report at `design/7_Research/flat-world-fw3-2026-09-16.md` and as your final
message: the API changes, the before/after table per lever on desktop and
board, the band-split proof, files touched outside the crate, anything in
§3 that proved wrong.
