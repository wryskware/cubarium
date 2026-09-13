---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Shared-care progress (Fable orchestrator → root)

Concise completion signals for root's live rollout. Evidence, not decisions.
Root owns the live process, `state/`, the rollout, and the gallery.

## A: same-world viewer — READY for root's rollout

- Commit `e3ad20f` "Mirror the same rendered frames to the web viewer".
  Host crate only; `cubarium-core` untouched; snapshot schema still 7. Verified by
  resuming a schema 7 snapshot the packaged build wrote, and by a library run whose
  `state_hash` equals a headless run of the same seed (rendering never touches state).
- Release binary preserved (built after the commit, embedded build id `0.1.0+e3ad20f`):
  `/tmp/cubarium-viewer-v7-e3ad20f/cubarium`
  sha256 `993a13709b5df582c6787f3d1e2160eee0944023331ed83866621ceb13bb4742`
  (`SHA256SUMS` and `LAUNCH.md` beside it).
- Launch (no `--fresh`; from the repo root so `--art` and `--state` resolve):
  `target/release/cubarium run --art assets/atelier --sink shim --mirror-web --web-port 7393 --state state --fps 60`
  or the packaged copy above in place of `target/release/cubarium`.
- Viewer at `http://127.0.0.1:7393/`; read-only `http://127.0.0.1:7393/status` reports
  `world_tick`, `render_seq`, `frames_served`, and `source {pid, state_dir, build_id,
  sink, resumed_from, start_tick, speed}`. The `/frame` 8-byte prefix remains the
  render sequence; the HUD now says `world tick N · seq M` plus a source line.
- Byte equality proven in `crates/cubarium/tests/mirror_web.rs`: for each of five
  distinctive frames through `FanOutSink(ShimSink → local UDP receiver, WebSink)`, the
  `cube_proto::decode` payload equals the `/frame` body after its prefix equals the
  submitted frame. A stalled web client (partial request, never reads) does not slow
  200 submits (< 500 ms). `cargo test -p cubarium`: 300 passed, 0 failed, 8 ignored.
- Not verified: the page was not opened in a real browser by the worker; the HUD is
  checked by Rust string assertions only. Root's rollout should glance at the HUD.
- Note for root's rebuild: `build.rs` only re-runs on its own change, so a release
  build after a commit can carry a stale git hash unless `crates/cubarium/build.rs`
  is touched first (the packaged binary above was built that way and reports
  `0.1.0+e3ad20f`).

## B: optional care — IN PROGRESS (schema changes; do not deploy before A)

- Contract revision 2 committed as `6b92feb` (`care-contract-2026-09-12.md`),
  adopting root's gate and Astra's contract review: held-boundary admission with
  `apply_after_tick = B`, uncertain fsync holds at `B`, OS advisory lock, 4 MiB journal
  bound with epoch-scoped server-issued client ids, total-footprint rain dose.
- Two workers running: core (`cubarium-core`: care module, schema 8, schema 7
  migration through a frozen mirror, `ecology_hash`, fixtures copied from root's
  `captures/checkpoints/pre-care/world-55200.cubw` and its 600-tick continuation by
  the schema 7 build) and host (lock, `--fresh` occupancy guard, `--require-resume`,
  journal, HTTP care routes, panel, replay). Commits will be listed here when they
  land. Nothing in `state/` is touched by either.

## C: Fable megafauna candidate — IN PROGRESS

- Worker authoring `art/studies/megafauna/fable.js` + `fable.md` (Lanternjaw); the
  anti-blinking art gate (smooth 150–300 ms accents, no hard cuts) was forwarded and
  Fable will review the rendered sheet before sign-off. Commit hash to follow.
