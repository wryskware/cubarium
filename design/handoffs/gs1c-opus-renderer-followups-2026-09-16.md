---
design_status: exploration
last_reviewed: 2026-09-16
---

# GS-1c (Opus, high): renderer follow-ups on the live panel

Read, in order: `design/7_Research/gs1b-status-2026-09-16.md` (§3 traps,
§4 interfaces, §6 next steps), `design/7_Research/gs1b-live-world-2026-09-16.md`
§3 and §5, `design/7_Research/tachyon-w2-deploy-2026-09-16.md` (the service,
`CUBARIUM_EXTRA_ARGS`, the `--mirror-web` cost), `design/7_Research/
flat-world-fw5-2026-09-16.md` (the CPU presenter's ring choices),
`crates/cubarium/src/art_present/wind.rs` (or wherever `bend_headroom`
lives; find it with graft), and the last sections of
`design/handoffs/tachyon-screen-plan-2026-09-16.md`. Fresh context. No
nested agents.

## Objective

Five bounded items, each independently verifiable, in this order:

1. **32-pixel art at 60 fps.** With `--gpu-art-scale 2` at 640×360 the GPU
   is fill-bound (8.5 ms, 43 fps). Profile where the fill goes (quad area
   vs. atlas texels actually opaque; 82 % of atlas texels are clear per
   GS-1's count) and cut it: tighter per-frame quads from the sprite's
   opaque bounding box, `discard` early, front-to-back ordering within a
   layer if it helps, or a depth pre-pass. Target: 60 fps at 640×360 S=2
   with `--gpu-art-scale 2`, measured on the board for 60 s. Do not
   change the picture (golden images and the fidelity test stay within
   their thresholds).
2. **Ring bend budgets.** `art_present::wind` derives the shipped pack's
   bend budgets (0.3–1.3 source texels) from the cube's 9 px footprint.
   On a ring the constraint is the topology's `max_local_radius` (90 px at
   320×180). Make the budget derivation topology-aware (cube unchanged,
   byte-identical) so the ring gets the room it has; expose the resulting
   per-clip budgets in the report. Then make `--gpu-bend-substep` the
   default on a ring at S ≥ 2 (keep the flag to turn it off), since Wrysk
   asked for smoother wind at the higher resolution.
3. **A hunter on the GPU.** Run a ring world with a hunter profile (find
   how the cube world founds one: `--neural`? a founder kind? read
   `lifecycle.rs` and the CLI) and verify the per-part rig composites as
   the CPU's single-query rig does: fidelity numbers for a frame with a
   hunter, and a PNG pair (CPU vs GPU) committed under `captures/` is
   gitignored, so under `crates/cubarium-gpu/tests/golden/` with a short
   README line.
4. **Lift FW-4's guard** on `demo --scene patch|all` for a ring (FW-5
   made the fixtures topology-aware): `run.rs:61-64` use
   `Scenes::on(topology, scale, kind, seed)`, delete the `bail!` in
   `cli.rs:428-444`, rewrite the test at `cli.rs:875-893` to assert the
   scenes now run. Verify with `demo --scene all --topology ring:320x180
   --sink png --seconds 2`.
5. **A web viewer that does not cost the panel.** `--mirror-web` today
   forces the CPU rasterisation (`FanOutSink::wants_pixels`). Add a
   `WebSink` mode fed from the GPU renderer's readback of its own raster
   at a low rate (e.g. 2 fps, `--gpu-web-rate`), so an operator can watch
   over `ssh -L` while the panel stays at 60 fps; measure the fps cost on
   the board with the viewer open.

## Where and what you own

Worktree `/home/wrysk/wryskware/cubarium/.claude/worktrees/tachyon-screen`,
branch `tachyon-screen`. You own `crates/cubarium-gpu/**`,
`crates/cubarium/src/sink/{gpu.rs,web/**,fanout.rs,mod.rs}`, `cli.rs`,
`run.rs`, `runner/**`, and the wind budget code in `art_present/` for item
2 only. No other worker is active on the branch; keep path-only commits
verified with `git show --stat HEAD` anyway, and keep the workspace
compiling at every commit.

Device `root@192.168.68.68`: `cubarium.service` (user `cubarium`) is live
and holds the daemon's socket. To test, `systemctl stop cubarium`, run as
`cubarium` (`sudo -u cubarium -H`, `taskset -c 4-7`, state under
`/var/lib/cubarium/state-test`, art at `/var/lib/cubarium/art`), and
`systemctl start cubarium` when done; leave it running and resumed at the
end. Deploy the binary with `scripts/tachyon-deploy.sh` (read it first;
the cold build is long, the incremental ~2 min). Never touch
`cube-screen-shim`, sysfs or debugfs; no apt; **do not reboot the board**
(a plain reboot hung it once today).

## Return

Report at `design/7_Research/gs1c-renderer-followups-2026-09-16.md` and as
your final message: per item what changed, the measurements before/after
on the board, the per-clip ring bend budgets, the hunter fidelity numbers,
and what is left. Trailer `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
No merge, no push.
