---
design_status: exploration
last_reviewed: 2026-09-16
---

# FW-6 (Opus, high): independent tests for the ring world

Read `design/flat-world-plan-2026-09-16.md` §2, §4, §5, §5a and §9's FW-6
row, and `design/7_Research/flat-world-fw1-2026-09-16.md` (the frozen API)
**before** opening any implementation or its tests. Fresh context. No nested
agents.

## Objective

Tests written from the plan, not from the code, at the eleven reserved paths:
`crates/cubarium-surface/tests/{ring_travel,ring_field,ring_raster}.rs`,
`crates/cubarium-core/tests/{ring_world,ring_weather,ring_schema17}.rs`,
`crates/cubarium-render/tests/{ring_canvas,ring_stamp_scale}.rs`,
`crates/cubarium/tests/{ring_sinks,ring_present,ring_care}.rs`.
You create only those files (plus test data under a `tests/data/ring_*`
name). No implementation file is edited; if you need a `pub`, report it.

## Where

Worktree `/home/wrysk/wryskware/cubarium/.claude/worktrees/tachyon-screen`,
branch `tachyon-screen`. FW-2 (core) and FW-3 (render) are implementing
concurrently: write the surface tests first (FW-1 is frozen), then the
core, render and host tests as those packages land; if a crate under test
is mid-change and does not compile, commit what you have and continue with
the next file, then return. `git add` your own paths only; never
`commit -a`. Rebase onto the branch head as others land.

## What to cover (each a named test; the plan's numbers are the expectations)

- ring_travel: the self-seam (exit at `u = w` enters at `u = 0`, identity
  transport, length preserved), both rims reflect, exact-tie and near-tie
  at all four corners with the outcomes the plan states (only bottom-right
  crosses first), a step that wraps and reflects in one displacement, a
  displacement that wraps more than once, `S = 2` doubles coordinates.
- ring_field: 3,600 cells at 320×180, 7,120 edges, degrees (3,440 × 4,
  160 × 3, no corner), `downhill` `None` on the top row and `(cx, cy+1)`
  elsewhere, conservative diffusion across the wrap, `validate` refusals
  (non-multiple extents, too many cells, too narrow, cube with `S ≠ 1`).
- ring_raster: `unfold_pixels` exactly once per pixel across the wrap and
  at the rims; at most two images; `chord_sq` is the wrapped Euclidean.
- ring_world: a ring world steps, founders on one chart, height reads
  `1 − 2v/h` at the controllers, feed/rain/clean/apex targets beyond pixel
  63 resolve and off-world is refused, RNG stream parity on the cube.
- ring_weather: embedding wrap continuity, `embed` vs `height` are
  different functions on a ring and equal on the cube, the measured cap
  aspect at the rims recorded (not asserted round), `blobs_per_channel` 3.
- ring_schema17: schemas 7..=16 refused by name, `WorldState::validate`
  refuses each bad thing in §4's list, and the `CubeProjection` negative
  tests (perturb one organism field, one free-list entry, one weather
  blob, one field vector, one extension state, one non-added config field;
  equality must fail each time) plus the positive equality on a fixture.
- ring_canvas / ring_stamp_scale: canvas sized `w×h`, `encode_raster`
  bytes, band-split composite bit-identical to serial for N = 1, 2, 4, a
  `scale = 2` stamp draws and stays within `9·S`, budget refusal above it.
- ring_sinks / ring_present / ring_care: written when FW-4/FW-5 land; for
  now create them with the tests you can already write against `cube_proto`
  (raster strip round trip through the vendored crate) and leave a
  `// pending FW-4` marker for the rest.

## Return

Final message: the list of tests with one line each, the run result per
file, every plan/code disagreement with the failing test name, and
anything you could not test and why. Commit small on `tachyon-screen`,
trailer `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. No
merge, no push.
