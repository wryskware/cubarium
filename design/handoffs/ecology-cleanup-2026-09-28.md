---
status: done 2026-10-01 (C1, C2, C3 on main)
date: 2026-09-28
owner: audit thread (cubarium-a1); Wrysk said go 2026-09-28
follows: design/7_Research/ecology-parameter-audit-2026-09-28.md §7
---

# Ecology code cleanup

Audit §7 lists where the ~38k lines of fauna + flora go and what can be removed. Two
branches are rewriting fauna and flora core files right now: P1 on `predation` (fauna
`step.rs` +979 lines, plus `lib.rs`, `body.rs`, sim founding, the search harness) and
WX3 round 3 on `worktree-agent-ae2a0976ade4bcce0` (flora `step.rs`, `cover.rs`, weather).
So the cleanup runs in parts:
- **C1 now**: nothing those branches touch.
- **C2 and C3** after both branches are on main.
- **The parameter registry** (`design/ecology-parameter-registry-2026-09-28.md`) waits
  for Wrysk's go.

## Rules for every package

- **Git.** Work in your own worktree, with `CARGO_TARGET_DIR` inside it. Commit with
  explicit paths only, never `commit -a`. The commit message is the report. Do not merge
  to main.
- **CPU.** Prefix builds with `taskset -c 8-15,24-31` and set `CARGO_BUILD_JOBS=16`. No
  sims, benches or long runs.
- **Check.** Run `cargo nextest run -p <crate>` for each crate you touched. When examples
  change, also run `cargo build --examples -p <crate>`. Don't run the full suite; the
  integrator does that once at merge.
- **Behaviour stays the same.** Conservation ledgers close, and the same seed gives the
  same qualitative behaviour. Float reordering is fine; don't compare bytes or hashes.
- **Before deleting anything,** find its callers with graft:
  `graft callers <sym> --depth all` and `graft grep "<literal>"`.
- **C1 only: leave the in-flight files alone.** Don't touch any file listed by
  `git diff --name-only main...predation` or
  `git diff --name-only main...worktree-agent-ae2a0976ade4bcce0`. If an edit needs one of
  them, skip it and list it.
- **Docs.** Don't edit canon or mark anything decided. `design/handoffs/` and
  `design/7_Research/` are history: leave them.
- **Return.** Stop your background shells first. Keep it to 40 lines or fewer: commits as
  SHA plus one line, lines removed, and anything you kept and why.

## C1: retire the old study programs (now)

**Done:** `edeff7cd` removed 8,092 lines and added 16. Merged to main at `c407b655`, where
the full suite passed: 2,508 tests. C2 picks up the two stale mentions
C1 had to skip, both in in-flight files: the `voxel_census.rs` comment near line 66, and
the flora `step.rs` doc comment above `can_establish`.

Their studies (two producers, replacement, grazed opening) finished 2026-09-16 to 09-18,
and the results are in `design/7_Research/`. Git keeps the code. Retire these four files,
8,046 lines in all:

- `crates/cubarium-voxel-flora/examples/two_producers.rs` (2,531 lines)
- `crates/cubarium-voxel-flora/examples/replacement.rs` (2,531)
- `crates/cubarium-voxel-flora/examples/harness/mod.rs` (1,938), only if no remaining
  target includes it by `#[path]`
- `crates/cubarium-voxel-fauna/examples/grazed.rs` (1,046)

Clean up after them:
- their `[[example]]` entries, and dev-dependencies only they used;
- rows in `design/backlog.md` that point only at them;
- comments outside the in-flight files that send readers to them (drop them, or say the
  program was retired).

If a kept program needs a piece of a retired file, move the smallest piece into the kept
program. If that piece is over ~150 lines, keep the file and say so.

Decided: the dense water fallback stays. Desktop worlds are 128 rows tall, exactly the
fast path's mask width, so deleting the fallback would cap world height.

## C2: fauna and flora de-duplication (after P1 and WX3 are on main)

From audit §7 (sources of bulk 3–5):
- **Shared helpers in one place** (`cubarium-voxel`):
  - the splitmix RNG, written 3× in fauna and 1× in flora;
  - seconds→ticks, written 5×;
  - the three-currency pro-rata transfer, 4× in fauna;
  - newborn construction, 2×;
  - the sated bite, 2×;
  - one germination predicate in place of its 7 entry points.
- **Retire the heuristic frondgrazer species path**: ~450 lines plus six config fields.
  First check that neither the lanternjaw heuristic nor the census uses it.
- **Dead branches**: fixture-only switches, the open-budget rain branch,
  `WeatherView::legacy`, and protocol-history strings.
- **Doc comments.** Keep what a reader needs: units, invariants, and why. Remove history:
  round numbers, package letters, "was X before Y". Git and the handoffs hold it.
- **The round-numbered flora suites** (round3, round4, round5b, round5c): keep the
  function tests and drop the contract pins, following WORKING_POLICY "Fast iteration".
- **Not in C2**: the two rules for "what a mouth reaches". Unifying them changes
  behaviour, so list the differences instead.

## C3: shared structures (after C2 and the registry's R1)

- One organism budget kernel for stands, the vine and animals (see the audit §7 table).
- The latticevine as a stand variant.
- One `found()`, one per-body meter and one episode-fixture config for host, live and
  frozen training. This is the pure refactor. Making the training worlds match the live
  world (audit §8.6) is a separate decision.
