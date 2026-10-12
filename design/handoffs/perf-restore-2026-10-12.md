---
design_status: exploration
last_reviewed: 2026-10-12
decision_refs:
  - D-0005
---

# PERF-RESTORE: bring every ash-era optimization over to the wgpu renderer

Wrysk, 2026-10-12, after the port review: "at the very least we should be reusing buffers
instead of recreating them.. but all the optimizations we did before should make it over too".
He also approved one `unsafe` call for the disk pipeline cache (below).

## Inputs
- Repo `/home/wrysk/wryskware/cubarium`. Your worktree and branch: `perf-restore`
  (`.claude/worktrees/perf-restore`, off main, own reflink-seeded `target/`).
- Ash renderer: `953e688f` (main just before the port). wgpu port: `ce035f3a`.
- Reviews of the port, with file:line evidence: `design/handoffs/wgpu-review-2026-10-11/core.md`
  (F1–F13) and `hosts.md` (H1–H2, M1–M7, L1–L9). Codex's port notes:
  `design/handoffs/wgpu-renderer-2026-10-11.md`.
- Baseline benchmark (ash vs wgpu, running until about 01:15 on 2026-10-12): voidrunner
  `~/bench-wgpu/out/summary.md` (ssh `-o HostKeyAlias=voidrunner wrysk@192.168.68.57`, fish login
  shell: send `bash -s` heredocs) and desktop
  `runs/lab/1012-000417-wgpu-bench-desktop-5090-3859/run/bench/summary.md`. The frame-timing patch
  both arms carry: `design/handoffs/wgpu-review-2026-10-11/bench-frames.diff`; harness
  `~/bench-wgpu/{run-voidrunner.sh,bench.py,bench.toml}` on voidrunner and
  `/home/wrysk/bench-wgpu-desktop/run-desktop.sh`. Read the summaries when they exist; don't run
  anything on voidrunner until `~/bench-wgpu/DONE` or `~/bench-wgpu/FAIL` exists.

## Deliverable
1. **Inventory first**, committed as `design/handoffs/perf-restore-inventory-2026-10-12.md`:
   every performance mechanism the ash renderer had (read the code at `953e688f` under
   `crates/cubarium-gpu/src` and `crates/cubarium/src/sink/gpu`, and the perf commits in
   `git log 953e688f -- crates/cubarium-gpu crates/cubarium/src/sink/gpu`). One row each: what it
   did, ash file:line, its state at `ce035f3a` (kept / lost / changed), and what you did (restored at
   file:line, or not applicable and why). Every review finding above appears in it.
2. **Restore them**, on the `perf-restore` branch. At least:
   - one persistent staging ring for world uploads, packed once at the 256-byte row pitch, all
     brick regions copied from it (F1); no per-brick `Vec`, buffer or zero-fill;
   - no per-frame creation of buffers, bind groups, query sets, textures, views or samplers in a
     steady frame: uniforms at ring offsets, bind groups built once per resource set, timing query
     sets reused (F2, F8, L1/L2);
   - figure models, plant cells and emitter tables uploaded only when they change (F3), plant
     tips not cloned per frame (F9);
   - readback handed out without the extra copy (F7, M5); the panel de-padded straight into a
     buffer mapped once, no `latest` copy (M1); restore ash's zero-copy panel path if safe Rust allows;
   - the window's presenting thread back (all WSI calls on it, the recorder never waits, a fresher
     frame replaces a waiting one), MAILBOX preferred when offered, FIFO fallback (F5, M3);
   - error handling: `on_uncaptured_error` and device-lost callbacks so wgpu errors become `Err` or
     a log line, never a panic on a shared-device thread; a warm-up failure must not leave the
     first live world waiting (F4, H2); the readback slot leak on a map error (F10); the panel
     in-flight count on a failed wait (L-items);
   - memory hints / allocator settings that avoid per-frame device allocations (F6);
   - the disk pipeline cache (M4), see below;
   - everything else the inventory finds.
3. **Tests** — these are the acceptance; write them as specified:
   - *Steady state allocates nothing:* with the `gpu-counters` feature (see cdf79fad's probe in
     `crates/cubarium/src/sink/gpu/hosted.rs`), after warm-up, 120 frames of an unchanged lit world,
     and 120 frames with animal motion, create zero new buffers, bind groups, query sets and
     textures. Same for the flat renderer and the present pass.
   - *Pipeline cache:* a saved cache reloads and is used; a truncated file, a random-bytes file
     and a file written under another adapter key each fall back cleanly and render the same image;
     an interrupted write never leaves a partial file under the real name.
   - *Errors:* an injected validation error on one renderer returns `Err` (or logs) without a
     panic, and a second renderer on the same shared device keeps rendering; a warm-up that fails
     doesn't block the first live world.
   - *Parity:* the parity capture matrix (`scripts/gpu-parity.py`; specs `scripts/gpu-parity-*.json`
     hard-code `.claude/worktrees/wgpu-renderer`, so make worktree-independent copies; frozen inputs
     in `art/gen/runs/wgpu-parity-2026-10-11/fixture/`) is pixel-identical to `ce035f3a`. Any
     changed pixel is a bug unless you can explain it.
   - Existing GPU tests and the workspace tests pass.
   Tests check behaviour and conservation; they never pin tuned numbers or timings.
4. **Benchmark rerun**, launched by you at the end and not waited on: ash `953e688f` against your
   branch head, both with the bench patch, same scenarios as the baseline but trimmed to
   2 repeats × 45 s, on voidrunner and the desktop 5090 (headless only there). Hand back with
   where it runs and where its `DONE`/`FAIL` markers and `summary.md` land.

## The approved `unsafe`
Wrysk, 2026-10-12: "i approve caching with the unsafe call". Exactly one call is approved:
`wgpu::Device::create_pipeline_cache`, which hands a saved blob back to the driver
(`PipelineCache::get_data`, the read-out, is safe). Put only that call in the `unsafe` block, with
a `// SAFETY:` comment, in one function marked `#[allow(unsafe_code)]`. The crate-level
`#![forbid(unsafe_code)]` can't be overridden, so change the crate root to `deny` and keep
`forbid` on every other module. Key the file with `wgpu::util::pipeline_cache_key(adapter info)`
(no key → no disk cache), set `fallback: true`, request `Features::PIPELINE_CACHE` only when the
adapter offers it, and write atomically (temp file + rename). Revive the dead directory API in
`pipeline_cache.rs` and the `CUBARIUM_PIPELINE_CACHE` override; Godot uses `user://pipeline-cache`
as at 087ed6ab; the CLI uses `$XDG_CACHE_HOME/cubarium/pipeline-cache`. Save when the warm-up
finishes and on clean shutdown, as ash did.
**No other `unsafe`** anywhere, including `create_shader_module_trusted` (M6 stays as is).

## Boundaries
- **Don't touch shaders.** A parallel package (WGSL-SOURCE) is rewriting every file in
  `crates/cubarium-gpu/shaders/` and owns `wgpu_backend/shader.rs` (including L3, the source
  hashing). Keep every shader interface fixed: bind group layouts, binding numbers, struct layouts,
  entry points, vertex inputs, outputs. If an item truly needs an interface change, leave it and
  list it for the parent.
- No look changes. The pictures must not change.
- Windows: never open a window on the desktop. A window test runs on voidrunner only, or under
  `scripts/hidden.sh` (Xvfb). New window code calls `sink::hidden::window_allowed()` first.
- Never write the word "chorister" anywhere, file names included.

## How to run things
- **No command of yours may take longer than 5 minutes.** Wrap anything that could in
  `timeout 270`. Never sleep, poll or wait on a job. This limit covers your own commands only: a
  detached chain (full test suite, parity matrix, benchmark) runs straight through under one
  long timeout, with no chunking and no early aborts, and you hand back while it runs.
- Quick compiles and targeted tests run directly in your worktree (`~/.local/bin/cargo` applies
  `-j 8` and idle priority; never pass a bigger `-j`). Full suites, the parity matrix and the
  benchmark go through labd (`scripts/lab/lab submit SPEC`, `design/lab-daemon.md`; GPU tests run
  serially in the GPU group) or a detached script, ending in a `DONE`/`FAIL` marker.
- No stress tools or synthetic load on the desktop, ever.
- Commit on `perf-restore` with explicit paths (never `commit -a`), trailer
  `Co-Authored-By: Codex (gpt-6.1-sol) <noreply@openai.com>` and no `Claude-Session:` line.
  Never merge, rebase main, deploy, or touch other worktrees.

## Hand-back (at most 40 lines)
- Branch head, the inventory path, and what's restored vs left (with why).
- Test results with raw pass/fail counts and the parity comparator's output.
- Anything you measured, with how; what is still running, where, and its markers.
- Decisions you made that the parent should check.
