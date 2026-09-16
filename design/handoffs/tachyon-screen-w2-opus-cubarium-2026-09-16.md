---
design_status: exploration
last_reviewed: 2026-09-16
---

# W2 (Opus, medium): the cubarium side of the Tachyon screen

Read [the plan](tachyon-screen-plan-2026-09-16.md) and W1's report
(`docs/reports/screen-shim-w1-2026-09-16.md` in the led-cube-shim worktree
`/home/wrysk/vuzic/led-cube-shim/.claude/worktrees/tachyon-screen`) first.
Fresh context. No nested agents.

## Objective

Make the Tachyon a first-class target for cubarium's existing shim sink:
documented, scripted, measured end to end from the desktop, and a measured
answer to whether cubarium itself can run on the Tachyon.

## Where you work

- Repo `/home/wrysk/wryskware/cubarium`, **worktree**
  `/home/wrysk/wryskware/cubarium/.claude/worktrees/tachyon-screen`, branch
  `tachyon-screen`. Run everything from the worktree; never edit the main
  checkout (it has unrelated uncommitted work).
- Device `root@tachyon-8968c731.local` (ssh key). The shim daemon
  `cube-screen-shim` is installed there by W1 as a systemd service. You may
  read its logs (`journalctl -u cube-screen-shim`), restart it, rsync a copy
  of the cubarium worktree to `/root/cubarium`, and build/run cubarium
  there. Nothing else on the device. Never write to
  `/sys/class/drm/card0-DP-1/status`.
- Start by reading `crates/cubarium/src/sink/shim.rs`, `src/cli.rs`
  (`RunSinkArg`, `Run.addr`), `vendor/cube-proto/src/{client,wire}.rs`, and
  the README's sink section. Use graft; the repo is indexed.

## Deliverable

1. README section "Tachyon screen" (short) plus `docs/tachyon.md` (or the
   repo's existing docs location if one fits better): how to point a run at
   the panel, what the shim does, where its config and logs live, the
   measured numbers below.
2. `scripts/tachyon-run.sh`: wraps `cubarium run` (or `demo`, by flag) with
   `--sink shim --addr tachyon-8968c731.local:7392` and `--mirror-web` so the
   desktop viewer still works, passing extra args through.
3. `scripts/tachyon-status.sh`: prints the shim's last log lines and
   `DP-1` connector state over ssh, so nobody has to remember the commands.
4. Measurement, from the desktop over wifi, `cubarium demo --sink shim
   --addr tachyon-8968c731.local:7392 --seconds 60` at the default fps:
   frames the sink reported sent, frames the shim reported received and
   stale-dropped, and the shim's presented fps. Repeat once. Put the numbers
   in `docs/tachyon.md`.
5. **Only if** measured loss over wifi exceeds 5% of frames: add a
   per-face send mode to `ShimSink` (five `encode_face` datagrams per frame
   via the existing `CubeClient::send_face`), behind a CLI flag
   `--shim-faces` on `run` and `demo`, default off, and re-measure. Pin it
   with a byte-exact test in `crates/cubarium/tests/shim_sink.rs` in the
   style of the existing one. If loss is under 5%, do not add it; record
   the numbers and move on.
6. Feasibility of running cubarium on the Tachyon: install nothing new on
   the desktop; on the device (rustup is already there from W1) rsync the
   worktree and `cargo build --release -p cubarium`. If it builds, run
   `cubarium demo --sink shim --addr 127.0.0.1:7392 --seconds 60` and
   `cubarium run --fresh --sink shim --addr 127.0.0.1:7392 --seconds 120`
   with a scratch `--state` dir under `/root/cubarium-state`, and record:
   build time, binary size, achieved render fps (the host logs it), sim
   tick rate versus real time, CPU from `top -b -n 1`. If it does not
   build, record the first error and stop; do not port dependencies.

## Constraints

- No change to cubarium's world schema, renderer, or `cube_proto`.
- `ShimSink::submit` must stay non-blocking; any per-face mode lives in the
  worker thread.
- `cargo test --workspace` green in the worktree before you report.
- Commit on `tachyon-screen` in small logical commits; message bodies
  explain why; end each with
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Do not merge
  or push.
- The panel may still be invisible to the kernel. The shim's counters and
  logs are the evidence then; say explicitly whether anything was seen on
  the real screen.

## Return format

Report at `design/7_Research/tachyon-screen-w2-2026-09-16.md` and as your
final message: files changed, the measurement table, the per-face decision
with the numbers behind it, the on-device feasibility numbers, what was seen
on the panel, and what is left.
