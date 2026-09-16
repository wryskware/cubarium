---
design_status: exploration
last_reviewed: 2026-09-16
---

# W3 (Opus, high): independent tests for `cube-screen-shim`

Read [the plan](tachyon-screen-plan-2026-09-16.md), especially the normative
layout spec, before you read any code. Fresh context. No nested agents.

## Objective

A test pass authored from the spec, not from the implementation, so W1's
green tests are not the only evidence. You write tests first from the spec,
then run them against W1's code, then read W1's tests only to avoid exact
duplicates.

## Where you work

Repo `/home/wrysk/vuzic/led-cube-shim`, **worktree**
`/home/wrysk/vuzic/led-cube-shim/.claude/worktrees/tachyon-screen`, branch
`tachyon-screen`. Crate under test: `crates/cube-screen-shim`. Never touch
the main checkout. No device access is needed; do not ssh anywhere.

## What to cover (each a named test, in `crates/cube-screen-shim/tests/`)

1. `net` placement at 1920×1080, gap 1: scale 7, origin (53, 88); every
   populated cell's top-left panel pixel maps to that face's (0, 0); the
   pixel one step left/up of it is background; the last pixel of a cell maps
   to (63, 63); gap columns are background.
2. `net` at other panel sizes: 1280×720 (scale 4), 3840×2160 (scale 14),
   a panel smaller than the net (scale 1, origin clamped or an error, per
   the spec, and say which the code does), and a pinned scale that does not
   fit.
3. Face adjacency in the net: Top's bottom row sits directly above Front's
   top row across the gap; Left/Front/Right/Back are in that order, unrotated
   and unmirrored (compare `source_of` against the spec formula for a random
   sample of 10,000 panel pixels, seeded).
4. `cube` mode: viewport is the centred `min(W,H)` square; background at the
   four viewport corners; the ported ray-cast still passes the round-trip
   over all 20,480 face-pixel centres; the default camera sees Front, Right
   and Top and nothing else.
5. `render` writes XRGB8888 bytes (B, G, R, X) with the given stride,
   background is zero bytes, and a frame with one distinct colour per face
   lands each colour in the right cell.
6. Ingest: a newer `seq` replaces, an older or equal `seq` is dropped and
   counted, a malformed datagram is counted and ignored, a single-face
   datagram updates only that face.
7. Idle policy: phases follow the clock exactly as the config says (`after`
   then `fade`), zero fade snaps.
8. Config: an empty file equals the documented defaults; unknown keys warn
   and do not fail; `scale = "auto"` and `scale = 3` both parse; a scale of
   0 is refused.

If the spec and the code disagree, the test encodes the spec and fails; you
report the disagreement, you do not change the implementation.

## Constraints

- Tests only, plus tiny `pub` visibility changes if a function you need is
  private (say which).
- `cargo test --workspace` in the worktree; report the exact pass/fail list.
- Commit on `tachyon-screen`, message body explains what each test pins,
  ending with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Return format

Final message: the list of tests with one line each on what they pin, the
run result, every spec/code disagreement found with the failing test name,
and anything you could not test and why.
