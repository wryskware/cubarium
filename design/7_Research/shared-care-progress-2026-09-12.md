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
- Revision 3 (`7ec73ac`) drops abort-and-resume after Astra's follow-up: an
  uncertain journal write holds the world at its boundary until a clean stop.
- **Core landed:** fixtures `01d8b73` (byte copy of root's genuine
  `captures/checkpoints/pre-care/world-55200.cubw`, plus its 600-tick continuation
  written by the schema 7 build `/tmp/cubarium-viewer-v7-e3ad20f/cubarium`) and
  implementation `a909aa0` (`cubarium-core`: `care.rs`, `WorldState.care` appended
  last, `SCHEMA_VERSION = 8`, schema 7 decoded through a frozen `WorldStateV7`
  mirror, `ecology_hash`, manual rain inside the water stage, ledgers in the mass,
  energy and water identities, Astra's allowance/shower-validation findings applied).
  Decisive proof, re-run by Fable: migrating the live schema 7 snapshot and stepping
  600 ticks with zero care reproduces the old binary's next snapshot payload
  byte-for-byte (`zero_care_reproduces_the_pre_change_binarys_next_600_ticks`).
  `cargo test -p cubarium-core`: 193 passed, 0 failed, 2 ignored (worker run);
  `--test care`: 13 passed (Fable's run). Rain footprints: interior 13 cells,
  seam 13, rim 9, each delivering the total dose.
  **Rollout note for root:** any build at or after `a909aa0` writes schema 8
  snapshots that the schema 7 binaries cannot read. Keep the schema 7 binary and a
  verified schema 7 snapshot for rollback; the new binary reads both.
- **Host landed:** `ff55aec` (OS advisory `flock` on `<state>/.lock` shared with
  the checkpoint and journal workers, `occupancy()` and the `--fresh` refusal in
  an occupied directory before any write, `--require-resume`, unloadable snapshot
  files are a hard error, `care.jsonl` journal with torn-tail repair and 4 MiB
  bound, care service with server-issued epoch-scoped client ids, `POST
  /care/register`, `POST /care`, `GET /care/status`, held-boundary admission in
  the runner, replay regardless of `--care`), `30c5ed6` (viewer care panel,
  `tests/care_replay.rs`, ENOSPC treated as uncertain, parent-directory sync
  barrier before the first acceptance, README), `c60241f` (panel registration
  retry, bounded row map). Evidence (worker's run): `cargo test -p cubarium` 362
  passed, 0 failed; delayed-ack hold keeps `world_tick` fixed at `B` for 2.5 s
  and applies once; crash after fsync and crash after application both replay at
  `B` with `ecology_hash` equal to an uninterrupted reference; mid-shower resume;
  uncertain write holds, appends nothing further, replays after restart; two
  concurrent launchers leave exactly one owner; `--fresh` beside eight higher-tick
  snapshots leaves every byte unchanged; cross-origin, GET mutation, oversize
  body, retired identity, journal full and replaying all refused over real
  sockets. Root's own copied-world browser check (`care-browser-rollout-review`)
  agrees on the accepted/duplicate/conflict/cooldown codes.
- Runnable shared command (no `--fresh`; requires a build at or after `c60241f`):
  `target/release/cubarium run --art assets/atelier --sink shim --mirror-web --web-port 7393 --state state --speed 1 --fps 60 --care`
  Default zero input preserves the autonomous world (proven by the fixture
  continuation test). Doses are the contract's constants; feed allowance 30 m per
  world; one shower at a time; cooldowns feed 30 s, rain 60 s, clean 30 s.
- Not verified: long-run ecology under repeated care (short runs only); the panel's
  click mapping is checked at string level and by the net-layout test, not by a
  synthetic browser click; `--sink preview` with care (no window here).

## C: Fable megafauna candidate — DELIVERED (commit `1e4a8e5`)

- `art/studies/megafauna/fable.js` + `fable.md` (Lanternjaw): 18×5 px segmented
  ambusher, cyan lantern chain, folded raptorial forelimbs, tail fan; modes rest /
  move / hunt / bud. Art gate applied: blink 280 ms and strike accent 240 ms raised
  cosine envelopes (largest single-frame step 0.23 and 0.30 of full swing; a hard
  cut would be 1.0), orange only as a 45 % tint at the strike peak, only in `hunt`.
  Harness: painted extent x−9…x+12, y−4…y+4; save/restore balanced; deterministic;
  `hunt` loops exactly at 6 s. Contact sheet `/tmp/cubarium-megafauna-fable/sheet.png`.
- Fable's art review of the sheet: the silhouette, segmentation and lantern crest
  read at 3× and 6× and the strike is legible without a flash; at native 1× the
  tail fan reads as a detached tab and the resting hull as a dark slab with lit
  points, which is acceptable for a study but worth a pass in root's production
  rendering slice (`art/studies/megafauna/integration-notes.md`). Motion is
  deliberately stepped to whole pixels; `fable.md` states this as a limitation.
  Wrysk's preference for Lanternjaw is recorded by root in `f6fb4c6`.
