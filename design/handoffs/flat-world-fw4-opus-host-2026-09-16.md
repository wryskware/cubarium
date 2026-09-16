---
design_status: exploration
last_reviewed: 2026-09-16
---

# FW-4 (Opus, medium): the host — outputs, sinks, CLI, care on a ring

Read, in order: `design/7_Research/flat-world-fw2-2026-09-16.md` (config
surface, `RenderView.topology`, the core resolvers),
`design/7_Research/flat-world-fw3-2026-09-16.md` (`Canvas::new(topo,
scale)`, `encode_raster`, `pixels()` vs `coords()`),
`design/flat-world-plan-2026-09-16.md` §3, §4 and §9's FW-4 row, the
`pending FW-4` lists at the end of `crates/cubarium/tests/{ring_sinks,
ring_present,ring_care}.rs` (FW-6's tests: make them pass without editing
them), and `design/handoffs/tachyon-screen-plan-2026-09-16.md`'s last four
sections (the GPU path, the daemon, the unprivileged user). Fresh context. No
nested agents.

## Objective

`cubarium run --fresh` on a ring world produces frames that leave the host:
the shim sink sends raster strips, the PNG sink writes the raster, the web
viewer shows it, `/status` reports the topology; the CLI and config expose
topology and scale; the preview sink refuses a ring with a clear message; the
host care chain takes 16-bit targets validated against the topology extent.

## Where and what you own

Worktree `/home/wrysk/wryskware/cubarium/.claude/worktrees/tachyon-screen`,
branch `tachyon-screen`. You own `crates/cubarium/src/{sink/**, cli.rs,
net.rs, run.rs, runner/**, care/**, care_effects.rs}` and
`crates/cubarium/src/sink/web/index.html`; the presenter files
(`present.rs`, `art_present/**`, `lanternjaw/**`, `scene.rs`) are FW-5's,
`crates/cubarium-gpu/**` is GS-1's. Commit ONLY as
`git commit -m "..." -- <your paths>` and verify with `git show --stat HEAD`.
Rebase onto the branch head as others land; keep the workspace compiling at
every commit.

## Deliverable

1. `enum Output<'a> { Cube(&'a Frame), Ring(&'a Raster) }` and
   `FrameSink::submit(&mut self, out: Output<'_>)` (object-safe; `FanOutSink`
   keeps working); `ShimSink` sends `encode_full` for a cube and
   `encode_raster` strips (all strips of one image under one `seq`) for a
   ring; `PngSink` writes the raster; `WebSink` serves the raster with its
   size in `/status` (`topology`, `w`, `h`, `scale`) and `index.html` draws
   a ring as one image at an integer scale; `PreviewSink` refuses a ring at
   construction with a message that names `--sink web` or `png`.
2. Runner: a ring world's canvas is `Canvas::new(topo, scale)`; the
   presenter is asked to draw whatever it can today (FW-5 makes it right);
   `--fresh` takes `--topology ring:320x180` / `--world-scale 2` (or the
   config file's keys) and refuses a resume whose snapshot topology differs
   from the requested one by name.
3. Care: host `CareTarget.{u, v}` → `u16`, validated against the topology
   extent instead of a literal 64; `PlannedCommand` journal and web request
   compatibility (a journal written before the widening still replays;
   pin it); `care_effects.rs` draws on either topology.
4. `cubarium demo --sink png` and `run --sink png` produce a ring PNG at
   320×180 and one at 640×360 (scale 2); a cube PNG at a fixed seed is
   byte-identical to before your change.
5. Measurements on the desktop: the runner's per-frame split (tick, draw,
   encode, sink) for cube and ring at both scales, from the existing bench
   or the runner's own log, as the numbers that pick `--fps` on the board.

## Verification you owe

FW-6's `ring_sinks`, `ring_care` and the non-ignored `ring_present` tests
pass unchanged; the cube PNG golden; `cargo test --workspace --exclude
cubarium-gpu` and clippy clean on your files. Commit small, trailer
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. No merge, no push.

## Return

Report at `design/7_Research/flat-world-fw4-2026-09-16.md` and as your final
message: the `Output` API, the CLI/TOML surface, the journal compatibility
proof, the measurement table, anything in §3/§4 that proved wrong.
