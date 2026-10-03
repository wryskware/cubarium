---
status: open
date: 2026-10-03
owner: opus-high worker, worktree `.claude/worktrees/gutsearch`, branch `gutsearch` (from `gut`)
---

# Rebase the search on the minimal gut

Package 2 of the gut line: repair 2 item 11 of `design/metabolism-meals-2026-10-02.md`.
- The gut is on branch `gut` (head 69dc7851), not yet merged. Its report is in the
  commits; the mechanics are in `crates/cubarium-voxel-fauna/src/{step,body,lib,pack}.rs`.
- Astra is reviewing the gut in parallel. If `gut` gets repair commits, merge `gut` into
  this branch again before you finish.

**Goal:** `cubarium-search` and `config/training/search/*` are correct under the gut, so that
`gut` and this branch can merge together with main green.

## Build

1. **Fix the 13 cubarium-search test failures** the gut's new defaults cause.
   - Fix by updating fixtures to the design's new behaviour, never by weakening an
     assertion.
   - List each test, why it failed, and what you changed.
2. **Genes**, in `roster.toml` and `roster-pilot.toml`, and in `cotrain.toml` only where it
   decodes a field the gut superseded:
   - **Upkeep genes are ×1 at the new `f`:** centred on the gut's new default, not the old
     value.
   - **The motor stays tied to upkeep.** The bite is no longer tied to it: `b` is its own
     gene.
   - **New genes per lineage:** stomach `k`, `τ` per food class the lineage eats, `b`, `s0`.
     Add one global gene, `parasite_take_per_day`.
   - **Remove** genes on fields the gut left unread: `bite_per_s`,
     `parasite_drain_of_host_basal`, and the teachers' reserve-satiety lines
     (`seedporter_full_at`, `bellwing_full_at`, `bellwing_hungry_at`, and any others).
   - **Ranges:** the design's defaults sit inside the range, with room on both sides.
     Widen registry search ranges where needed.
3. **Delete the unread fields** and their registry rows (marked UNREAD on `gut`), including
   the teacher knobs the latch replaced.
   - Keep fauna schema 36: `gut` hasn't shipped.
   - Regenerate `design/parameters.md`.
4. **Screens** (`crates/cubarium-search/src/es/voxel/live/screen.rs`):
   - **The night guard** counts digestible stomach stock as well as reserve. A stomach of
     `s0·K` at dusk, digested at the lineage's yield, adds to what lasts the night.
   - **Replenishment** counts reserve plus digestible stomach, so moving food between the
     two is not income.
   - **The food screens** use the gut's daily need `f` as demand, not the old
     `maintenance_per_s`.
   - **Tests:** each changed screen refuses a hand-built failing case and passes a passing
     one, at the design's worked numbers.
5. **Live presets.** `gut` removed the superseded per-lineage keys from the four presets, so
   code defaults apply. Leave that as it is unless a decode needs a key there; say if one
   does.

## Smoke

- Pilot: `config/training/search/roster-pilot.sh` at λ 2, 2 seeds, 1 h, `nice -n 10`, at most
  8 threads, at most 30 min of wall time. It must run end to end and write
  `generations.jsonl`.
- Lock step: one `voxel-ecology-search` unified-mode generation on `cotrain.toml`, at λ 2,
  1 seed, 30 min.

## Out

- The fauna mechanics. If a fauna bug shows up, report it; don't fix it here.
- `crates/cubarium-voxel-fauna` beyond deleting the unread fields and rows.
- Launching any real search.

## Constraints

- **Worktree:** `git worktree add .claude/worktrees/gutsearch -b gutsearch gut`. Commit with
  explicit paths only, early and often. Never merge into main, deploy, or restart a host.
- **Shared desktop:** use `nice -n 10` and `-j 8`. Headless only.
- Never `pkill -f` a pattern in your own command line. Leave no background shells.
- End each commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 40 lines)

- branch and head;
- the 13 failures and their fixes;
- the gene changes, as a count per lineage, with new and removed genes and ranges;
- the screens as changed;
- the fields deleted;
- the smoke results;
- open questions.
