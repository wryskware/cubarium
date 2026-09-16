---
design_status: exploration
last_reviewed: 2026-09-16
---

# SYNC-1 (Opus, high): merge `main` into `tachyon-screen`

`main` has moved 88 commits since this branch's base `2a1cedd` (ecology v1
rounds 4 and 5: apex and hunter work, calibration, design notes). A dry-run
merge conflicts in five files. This package brings the branch up to date so
the final merge is small, and re-proves the cube unchanged against the new
`main`.

## Where

Worktree `/home/wrysk/wryskware/cubarium/.claude/worktrees/tachyon-screen`,
branch `tachyon-screen`. Never edit the main checkout
`/home/wrysk/wryskware/cubarium` (it has another session's uncommitted
work); read `main` through `git show main:<path>` or a temporary
`git worktree` of `main` under your scratch directory (remove it after).
GS-1b is working concurrently in `crates/cubarium-gpu/**`,
`crates/cubarium/src/sink/gpu.rs`, `cli.rs`, `runner/mod.rs`; the merge may
touch those only where `main` itself changed them (it did not, per the
dry run). Commit the merge as a **merge commit** (`git merge main`, no
squash, no rebase of the branch), then any follow-up fixes as path-only
commits verified with `git show --stat HEAD`.

## Deliverable

1. `git merge main` resolved: the five conflicts (`cubarium-core/src/
   fields.rs`, `hunter/geometry.rs`, `world/lifecycle.rs`, `world/mod.rs`,
   `cubarium-search/src/es/episode.rs`) are FW-1/FW-2's topology threading
   against main's ecology edits; resolve by keeping both intents, and
   thread `Topology`/`Scale` through any **new** code main added that
   assumes the cube (grep main's diff for `embed()`, `CELL_COUNT`,
   `FACE_EXTENT`, `64`, `Face::`, `pixel_center`, `unfold_pixels`, and
   `CellId::all(`). Design docs merge trivially; keep both.
2. Workspace green: `cargo test --workspace --exclude cubarium-gpu` and
   clippy no worse than either side. Every FW-6 `ring_*` test still
   passes.
3. The cube proof, redone against the new base: export a fresh
   `CubeProjection` fixture from an unmodified `main` build at its current
   head (same procedure FW-2 used: temporary worktree of `main`, fixed seed,
   6,000 ticks) into `crates/cubarium-core/tests/fixtures/` with the head
   hash in its name, and make `cube_projection.rs` compare against it; keep
   the old `2a1cedd` fixture and its test as history (it will no longer be
   equal, since main changed the ecology; mark that test as pinning the
   pre-sync state, or remove it with a note, your call, say which).
4. The cube frame hashes FW-3/FW-5 pinned will move if main changed what a
   cube world draws; re-record them only with a sentence per hash saying
   which `main` change moved the pixels (find it by bisecting main's
   commits if not obvious), never silently.
5. Report: `design/7_Research/flat-world-sync-main-2026-09-16.md`: the
   conflict resolutions (each with the two intents), the new fixture hash,
   which goldens moved and why, anything on `main` that a ring world will
   need and does not yet have (e.g. new per-cell vectors sized at the cube).

Trailer `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. No
push. No nested agents. No device access needed.
