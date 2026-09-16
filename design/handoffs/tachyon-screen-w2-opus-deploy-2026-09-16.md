---
design_status: exploration
last_reviewed: 2026-09-16
---

# W2 (Opus, high): the shelf piece — cubarium as a service on the Tachyon

Supersedes the earlier W2 brief (CPU shim path). Read, in order:
`design/7_Research/gs1b-status-2026-09-16.md` §3–§5 (traps, interfaces,
commands), `design/7_Research/gs1b-live-world-2026-09-16.md`,
`docs/reports/screen-shim-gs2b-2026-09-16.md` in the led-cube-shim worktree
(`/home/wrysk/vuzic/led-cube-shim/.claude/worktrees/tachyon-screen`) for the
socket's group/mode, `crates/cubarium/src/cli.rs` (`Run`: `--state`,
`--fresh`, `--require-resume`, `--topology`, `--world-scale`, `--sink gpu`,
`--gpu-*`, `--fps`, `--mirror-web`) and `runner/mod.rs` for the resume
semantics, and the last sections of
`design/handoffs/tachyon-screen-plan-2026-09-16.md`. Fresh context. No
nested agents.

## Objective

The Tachyon boots and, with no hands on it, shows the live ring world on
the panel within a minute: `cube-screen-shim.service` (root, already
installed) then `cubarium.service` running as the unprivileged user
`cubarium`, resuming its own world across restarts and reboots.

## Where

Worktree `/home/wrysk/wryskware/cubarium/.claude/worktrees/tachyon-screen`,
branch `tachyon-screen`. You own `config/tachyon/**`, `scripts/tachyon-*.sh`,
`docs/tachyon.md`, one README section, and nothing in `crates/**` beyond a
CLI refusal message if one is wrong. Path-only commits verified with
`git show --stat HEAD`. Device `root@192.168.68.68` (use the IP; the mDNS
name sometimes resolves to a link-local IPv6 address ssh refuses). Allowed
on the device: create the system user and its directories, install the
binary and unit, enable/restart `cubarium.service`, reboot the board once
for the boot check, read logs. Never stop or reconfigure
`cube-screen-shim`, never write to sysfs/debugfs, never `apt`.

## Deliverable

1. **User and paths.** `useradd --system --home /var/lib/cubarium
   --groups video,render cubarium`; `/var/lib/cubarium/{state,art}` owned by
   it; the atelier pack copied to `/var/lib/cubarium/art`; binary at
   `/usr/local/bin/cubarium` (root-owned, 0755). Put the exact commands in
   `scripts/tachyon-install.sh` (idempotent) and run it.
2. **`config/tachyon/cubarium.service`**: `User=cubarium`, `Group=cubarium`,
   `SupplementaryGroups=video render`, `After=cube-screen-shim.service
   network.target`, `Wants=cube-screen-shim.service`, `Restart=always`,
   `RestartSec=3`, `WorkingDirectory=/var/lib/cubarium`,
   `Environment=RUST_LOG=info`, `CPUAffinity=4-7`, and an `ExecStart` that
   resumes the existing world when one exists and founds a fresh ring
   otherwise. Read `cli.rs`/`runner` to get that right; if the CLI cannot
   express "resume if present else fresh" in one invocation, add a tiny
   `scripts/tachyon-run.sh` wrapper installed to `/usr/local/bin` that
   checks for the snapshot and picks the flags, and say so. World:
   `ring:640x360`, `--world-scale 2`, `--sink gpu`, `--art
   /var/lib/cubarium/art`, `--fps 60`, `--mirror-web` on loopback (document
   `ssh -L` for the viewer). Leave `--gpu-art-scale` and
   `--gpu-bend-substep` as an `Environment=CUBARIUM_EXTRA_ARGS=` line the
   wrapper appends, so Wrysk's pending look decision is a one-line edit.
   Founders on a fresh ring: 67 (the plan's density call) via the config
   file `/var/lib/cubarium/world.toml` if the CLI has no flag; document it.
3. **`scripts/tachyon-deploy.sh`**: rsync + native release build on the
   board (`taskset -c 4-7`, under `/root/cubarium-deploy`), install,
   restart `cubarium.service`, print the build time. **`scripts/
   tachyon-status.sh`**: both services' state, last 20 log lines each,
   `DP-1` state, `top -b -n 1 | head -15`.
4. **Checks on the board**: service up and presenting (daemon log shows the
   dma-buf client at ~60 fps); `systemctl restart cubarium` resumes the
   same world (tick continues, log says resumed); **`systemctl reboot`**,
   then within 60 s both services active and frames flowing, the world
   resumed; 10 minutes of steady state: CPU, RSS, fps. Also confirm the
   panel goes black when `cubarium` is stopped and returns when started.
5. **Docs**: `docs/tachyon.md` (architecture in two paragraphs, install,
   rebuild, service layout, the measured table, the viewer tunnel, the
   `CUBARIUM_EXTRA_ARGS` knob) and a README section pointing at it.

## Constraints

- No change to world schema, renderer or protocol. `cargo test --workspace
  --exclude cubarium-gpu` green after your commits. Trailer
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. No merge, no
  push.

## Return

Report at `design/7_Research/tachyon-w2-deploy-2026-09-16.md` and as your
final message: the unit and wrapper as installed, the resume semantics with
the code they come from, the reboot result with timestamps, the steady-state
numbers, and what is left.
