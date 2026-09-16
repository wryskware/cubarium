---
design_status: exploration
last_reviewed: 2026-09-16
---

# FW-0 (Opus, medium): vendor the raster format; measure the render budget

Read `design/flat-world-plan-2026-09-16.md` §6 and the FW-0 row of §9, and
`design/handoffs/tachyon-screen-plan-2026-09-16.md` (device facts). Fresh
context. No nested agents.

## Objective

Two independent things: (1) the cubarium workspace builds against the
`cube-proto` that has the raster strip format; (2) four pinned numbers exist
that gate the flat world's scale and frame rate.

## Where

- Cubarium worktree `/home/wrysk/wryskware/cubarium/.claude/worktrees/tachyon-screen`
  (branch `tachyon-screen`). Never edit the main checkout.
- The `cube-proto` source of truth is the led-cube-shim worktree
  `/home/wrysk/vuzic/led-cube-shim/.claude/worktrees/tachyon-screen`
  (branch `tachyon-screen`, clean, HEAD `51bb763` or later). Read-only.
- Device `root@tachyon-8968c731.local` (ssh key). rustup is installed; the
  display daemon `cube-screen-shim.service` runs there and must be left
  running. You may rsync the cubarium worktree to `/root/cubarium`, build
  and run benchmarks there. Do not touch the daemon, `/etc`, sysfs or
  debugfs.

## Deliverable

1. **Vendor.** `scripts/sync-cube-proto.sh <path-to-shim-worktree>` (it
   refuses a dirty checkout and records the rev in `vendor/cube-proto.rev`).
   Review `vendor/cube-proto/Cargo.toml` against the source's by hand as the
   script says. `cargo test --workspace` green on the desktop. Commit.
   `Raster`, `encode_raster`, `decode_strip` (or whatever the shim named
   them) must be reachable from cubarium; write a one-paragraph note of the
   actual API names in your report for FW-3/FW-4.
2. **Measure, desktop and device.** Add `crates/cubarium/examples/render_bench.rs`
   (or extend an existing bench if one fits) that builds a real cube world
   from a fixed seed, runs it to a populated steady state (say 3,000 ticks
   headless), then times, over 600 frames: (a) one full render+encode of a
   frame on one thread, (b) the same split across four threads by row band
   if the renderer allows it today, else say so and skip, (c) one world
   tick. Report medians and p95 in ms. Then a real run: `cubarium run
   --fresh --sink none --seconds 120 --state <scratch>` for ticks/s, and
   `cubarium demo`/`run` with `--sink shim --addr 127.0.0.1:7392 --fps 60
   --seconds 120` on the device for achieved fps and ticks/s from the host
   log while the daemon's log reports presented fps and received/stale.
   On the device, pin to one A78 (`taskset -c 7`) for (a) and (c), and to
   `4-7` for (b) and the real run. Record `top` CPU during the real run.
3. **The gate.** Put the numbers into the `R_max` table of plan §6 and say
   which S and `--fps` they select under the shared-loop budget
   `20·tick_ms + fps·render_ms ≤ 1000 ms`. Do not change the plan's text
   otherwise; append a "FW-0 measurements" subsection to §6 with the table
   and the selection, and commit.

## Constraints

- No source changes beyond the vendored crate, the bench example, and
  anything the vendoring forces (say what).
- Cube frames must stay byte-identical: the vendored crate's formats 0/1
  are unchanged by construction; `crates/cubarium/tests/shim_sink.rs` is
  the check.
- Commit small on `tachyon-screen`, trailer
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. No merge, no
  push.

## Return

Report at `design/7_Research/flat-world-fw0-2026-09-16.md` and as your
final message: the vendored rev, the API note, the measurement table with
pinning, the selected S and fps with the arithmetic, and anything skipped.
