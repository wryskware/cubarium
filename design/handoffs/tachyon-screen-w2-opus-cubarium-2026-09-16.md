---
design_status: exploration
last_reviewed: 2026-09-16
---

# W2 (Opus, high): cubarium running on the Tachyon, standalone

Read [the plan](tachyon-screen-plan-2026-09-16.md) and W1's report
(`docs/reports/screen-shim-w1-2026-09-16.md` in the led-cube-shim worktree
`/home/wrysk/vuzic/led-cube-shim/.claude/worktrees/tachyon-screen`) first.
Fresh context. No nested agents.

## Objective

The Tachyon becomes a shelf piece: it boots, the shim brings up the panel,
and cubarium runs its world on the same board and sends frames to loopback.
No desktop in the loop after deployment. Wrysk's words: "make this a
standalone instance I can put on a shelf."

## Where you work

- Repo `/home/wrysk/wryskware/cubarium`, **worktree**
  `/home/wrysk/wryskware/cubarium/.claude/worktrees/tachyon-screen`, branch
  `tachyon-screen`. Run everything from the worktree; never edit the main
  checkout (it has unrelated uncommitted work).
- Device `root@tachyon-8968c731.local` (ssh key). W1 installed rustup and
  the `cube-screen-shim` service there. You may rsync the cubarium worktree
  to `/root/cubarium`, build there, install the binary to `/usr/local/bin`,
  create `/var/lib/cubarium` for state, install and enable
  `cubarium.service`, restart either service, reboot the board once for the
  boot-to-world check, and read logs. Nothing else on the device. Never
  write to `/sys/class/drm/card0-DP-1/status`.
- Start by reading `crates/cubarium/src/sink/shim.rs`, `src/cli.rs` (the
  `Run` struct: `--sink`, `--addr`, `--state`, `--fps`, `--speed`,
  `--fresh`, `--require-resume`, `--mirror-web`), `src/runner/mod.rs`
  (`open_sink`, the render/tick loop and what it logs), and the README.
  Use graft; the repo is indexed.

## Deliverable

1. **Native build on the device.** rsync the worktree (exclude `target`,
   `.git`) to `/root/cubarium`, `cargo build --release -p cubarium`. Record
   wall time and binary size. If a dependency fails to build on the board
   (the likely one is `minifb`, only needed by the preview sink), gate the
   preview sink behind a cargo feature `preview` (default on, so the
   desktop is unchanged) and build the device binary with
   `--no-default-features`; keep the CLI refusing `--sink preview` with a
   clear message when the feature is off. Do not port or replace
   dependencies beyond that.
2. **`config/tachyon/cubarium.service`** (system unit): `User=root`,
   `After=cube-screen-shim.service network.target`,
   `Wants=cube-screen-shim.service`, `Restart=always`, `RestartSec=3`,
   `Environment=RUST_LOG=info`, `WorkingDirectory=/var/lib/cubarium`,
   `ExecStart=/usr/local/bin/cubarium run --sink shim --addr 127.0.0.1:7392
   --state /var/lib/cubarium/state --fps <measured>` plus whatever the run
   command needs to resume an existing world on restart and start fresh
   only when no world exists (read `cli.rs` and the runner to get the
   resume semantics right; do not guess). Also `config/tachyon/README.md`
   with the install steps.
3. **`scripts/tachyon-deploy.sh`**: rsync + remote build + install +
   `systemctl restart cubarium`, idempotent, prints the build time.
   **`scripts/tachyon-status.sh`**: both services' status, last 20 log
   lines of each, `DP-1` connector state, and `top -b -n 1 | head -15`.
4. **Measurements on the device**, with the shim running:
   - `cubarium demo --sink shim --addr 127.0.0.1:7392 --seconds 60` at
     `--fps 60`: achieved render fps from the host's log, shim's presented
     fps and received/stale counters, CPU from `top`.
   - `cubarium run --fresh --sink shim --addr 127.0.0.1:7392 --seconds 300
     --state /root/cubarium-state-test` at `--fps 60` and again at
     `--fps 30`: render fps, sim ticks per real second versus the configured
     tick rate, CPU, RSS. Pick the `--fps` for the unit from these numbers
     (60 if it holds within 5%, else 30) and say why. Leave `--speed 1`
     unless the sim cannot keep real time; if it cannot, report that
     prominently rather than lowering speed silently.
   - Steady state after 10 minutes of the service running: CPU and RSS.
5. **Boot-to-world check**: both services enabled, `systemctl reboot`,
   wait, then confirm via `tachyon-status.sh` that both are active and the
   shim is receiving frames. If the panel is visible to the kernel, say what
   is on it; if not, the counters are the evidence and you say so.
6. **Docs**: README section "Tachyon (standalone)" pointing at
   `docs/tachyon.md`, which holds the architecture in two paragraphs, the
   install/rebuild procedure, the service layout, the measured table, and
   how to reach the web viewer over an ssh tunnel (`--mirror-web` binds
   loopback on the device; document `ssh -L`).

## Constraints

- No change to cubarium's world schema, renderer, or `cube_proto`.
- `ShimSink::submit` stays non-blocking; no per-face send mode (loopback
  carries 61 KB datagrams fine; check the shim's stale counter says so).
- `cargo test --workspace` green in the worktree on the desktop before you
  report (with the default features).
- Commit on `tachyon-screen` in small logical commits; message bodies
  explain why; end each with
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Do not merge
  or push.
- If the native release build takes longer than 40 minutes, finish with it
  anyway, record the time, and put a one-paragraph cross-compilation
  proposal (what to install on the desktop, nothing installed by you) in
  the report for Wrysk to decide.
- The panel may still be invisible to the kernel. The shim's counters and
  cubarium's logs are the evidence then; say explicitly whether anything was
  seen on the real screen.

## Return format

Report at `design/7_Research/tachyon-screen-w2-2026-09-16.md` and as your
final message: files changed, the measurement table, the `--fps` decision
with the numbers behind it, the resume semantics you wired and where in the
code they come from, the boot-to-world result, what was seen on the panel,
and what is left.
