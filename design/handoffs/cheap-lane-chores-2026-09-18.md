---
status: open
date: 2026-09-18
owner: any non-frontier agent (opencode + OpenRouter autorouter)
---

# Cheap-lane chores: no design calls, loud checks

Bounded housekeeping on the cubarium repo that needs no frontier model. Each task has
a deterministic check. Do them in order, one commit per task, and stop at anything
that would need a judgment call: leave a note in the return instead of deciding.

## Rules (read first, they override your defaults)

- Read `WORKING_POLICY.md` and `AGENTS.md` before touching anything.
- Work on branch `main` in `/home/wrysk/wryskware/cubarium`. Do not touch
  `.claude/worktrees/**`, `design/handoffs/README.md` (has a user's uncommitted edit),
  anything under `art/`, or any deployed process (the cube, the Tachyon, a toy on port 7402).
- Commit **explicit paths only**: `git add <file> …`, never `git add -A` or `commit -a`.
  Commit message: one line saying what changed, then a blank line, then a short body.
- Never edit numbers in configs or presets. Never
  add a test that pins a byte hash of a world. Never write a design decision; if a doc
  needs one, note it and move on.
- Build with `cargo build -j 24 --release -p <crate>`; test only the crate you touched.
  No simulation runs longer than a few seconds unless a task says so.
- `gh` is authenticated; use it for issues.
- Return format at the end: per task, ≤10 lines: what you changed (commit hash), the
  check you ran and its output, anything you skipped and why.

## Task 0 — format the repo, once, in its own commit

The repo has never been run through rustfmt (about 4,700 hunks pending). Do it as one
commit that changes nothing else, before any other task:

```
git status --short          # must show no modified tracked files except design/handoffs/README.md
cargo fmt --all
git add $(git diff --name-only -- '*.rs')
git commit -m "cargo fmt --all, once, no other change"
cargo build -j 24 --release --workspace
cargo fmt --all --check     # must print nothing
```

If `git status` shows other modified tracked files before you start, stop and report;
do not format on top of someone's work. Every later commit must keep
`cargo fmt --all --check` clean for the files it touches.

## Task 1 — close issue #13

`gh issue view 13`. The files it lists no longer exist (`ls` them to confirm). Close it
with `gh issue close 13 --comment "Resolved: the listed pinned-hash tests were deleted
before 2026-09-18; nothing under crates/ asserts a literal world hash."` Check:
`grep -rn "SWEEP_HASHES" crates/` returns nothing.

## Task 2 — backlog and issue hygiene

`design/backlog.md` §1 has knob rows that issue #14 does not mention: the voxel water
placeholders (`FLOW_PER_SUBSTEP`, `HEAD_PASSES`, `ROOM_EPS`, line ~40) and the voxel
thread count (`SimConfig::threads` / `[voxel] threads`, line ~41). Add one comment to
issue #14 listing those rows in the same style as the issue body (name, where it lives,
live or restart). Do not edit `backlog.md` content. Check: `gh issue view 14 --comments`
shows the comment.

## Task 3 — handoff frontmatter sweep

Every file in `design/handoffs/*.md` has `status:` frontmatter (`open`, `leaning`,
`resolved`). For each file with `leaning` in the list below, its work has landed on main.
Change `status: leaning` to `status: resolved` and add one line right under the
frontmatter: `> **RESOLVED <date>.** Landed on main; see git log for the commits.` using
the date from the file's own `date:` field's package (use 2026-09-18 if unsure).

Files: `voxel-producers-briefs-2026-09-16.md`, `voxel-round3-briefs-2026-09-16.md`,
`voxel-round3b-briefs-2026-09-17.md`, `voxel-round4-presets-briefs-2026-09-17.md`,
`voxel-round5a-transfers-briefs-2026-09-17.md`,
`voxel-round5bc-consumers-briefs-2026-09-17.md`, `voxel-render-briefs-2026-09-17.md`,
`voxel-replacement-study-brief-2026-09-17.md`,
`voxel-replacement-study-run-2026-09-18.md`, `voxel-schedule-brief-2026-09-18.md`.

Leave `voxel-senses-handoff-2026-09-18.md` as `open`. Do not touch README.md there.
Check: `grep -l "^status: leaning" design/handoffs/*.md` returns nothing.

## Task 4 — replacement study reduction script

Raw outputs: `design/7_Research/assets/voxel-replacement/full-*.txt` (20 files, named
`full-<resident>-resident-seed<N>-noise<M>.txt`). Each file has three sites × arms; a
completed arm prints a line containing `**replacement completed**` with
`first birth at t <s> s` and `delivered its own package at t <s> s`; other arms print
`unresolved at cap`, `possible refuge`, or a refusal line (grep the files to see the
exact wording, quote it, do not paraphrase).

Write `scripts/reduce_replacement.py` (Python 3, stdlib only) that reads a directory
of those files and prints, per direction (resident species): number of arms, number
completed, and the median / min / max of the replacement time (the "delivered its own
package" time) over completed arms, plus the count of each other outcome. Output as a
markdown table to stdout. Run it on the committed directory and paste the table in
your return. Do **not** edit the experiment note. The numbers must reproduce what the
note at `design/7_Research/voxel-round3-experiment-2026-09-16.md` (section
"Replacement study — 2026-09-18") already states: bloomcrown resident 30/30
completed, median ≈ 6,516 s; umbrellafrond resident 7/30. If they do not, say so and
do not adjust the script to force them.

## Task 5 — bench rerun for the profile note

Command (usage in `crates/cubarium-voxel-sim/examples/bench.rs`, top comment):

```
cargo run --release -p cubarium-voxel-sim --features profile --example bench -- 1000 4 50 <seed> 100 <threads>
```

Run it for threads 1, 4, 8, 16, each at seeds 1..5 (20 runs, one process each,
sequentially; each takes seconds). Record the mean and half-spread of the ms/tick figure
the bench prints (find the line; quote its label). Append a dated subsection
"### Bench rerun — <date> at <git short hash>" at the end of
`design/7_Research/voxel-tick-profile-2026-09-18.md` with one table (threads × ms/tick,
mean ± half-spread) and the command line. No interpretation, no comparison sentence.
Check: the file renders as markdown and `git diff` shows only the appended block.

## Out of scope

Any edit under `crates/**` other than task 0's formatting. Senses, rendering, art, the persistence
envelope, per-band cell sets, moving harnesses onto the schedule. If a task is
blocked, skip it, say why, continue with the next.
