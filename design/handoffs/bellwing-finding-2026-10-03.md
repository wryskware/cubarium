---
status: open
date: 2026-10-03
owner: opus-high worker, worktree `.claude/worktrees/bellwing`, branch `bellwing`
---

# Bellwings that never reach a flower; the empty feasts readout; the chase timeouts

From the 8-world measure after gut repair 3. The record is in
`design/metabolism-meals-2026-10-02.md`, section "Measure after repair 3". The raw output is
eidolon `~/gut-measure/r3`, and a local copy is at
`/tmp/claude-1000/-home-wrysk-wryskware-cubarium/0183970f-d99c-4145-9cdc-fd35b3ab6476/scratchpad/r3`
(aggregator `agg3.py` beside it).

## 1. Bellwings (the main item)

- **The symptom:** in 5 of 8 worlds (seeds 3, 5, 8, 9 and 12), bellwings swallowed almost no
  nectar and logged 0 sip misses: they never reached a flower. Their reserve went from full to
  empty within the world day, and 1–5 starved. In seeds 1, 6 and 14 they found flowers.
- **Find out why, with evidence:**
  - Were there flowers with nectar stock within the bellwings' sensing range and flight range?
  - Did the hover-sip teacher ever pick a target?
  - Were they founded far from any flower?
  - Do they fly but never descend?
  - Does the night or sleep keep them perched?
- **Fix any defect** in sensing, targeting, flight reach or the teacher. Don't tune a threshold
  so a counter passes.
- **Out of scope:** a parallel worker on branch `plants3` is changing vine flowering (sparse
  and staggered) and making bloomcrowns flower constantly. Don't touch flora. If the cause is
  only "no flowers anywhere near", say so with numbers; that is the flora change plus staged
  arrival (Wrysk: bellwings may arrive later, once latticevine is established).

## 2. The feasts readout

`founder_tally`'s pack lines show `feasts=[]` in every world, even after chases ending
`Killed`. Find out whether packs feast after a kill (do they eat the carcass?) and whether
the readout records it. Fix whichever is broken. If the pack doesn't eat its kill, that's a
behaviour defect, so fix it with a test.

## 3. Chorister chase timeouts

In world 1, one pack ran 76 chases; 74 ended at the timeout (`Late`). Diagnose why: an
unreachable target, flipping between targets, a slow pack, or terrain. Report it. Fix only
a clear defect.

## Tests (write first; don't weaken them)

For each defect fixed, write a fixture that fails before the fix and passes after it.

## Measure

- `founder_tally` at main's live preset, the same 8 worlds (seeds 1, 3, 5, 6, 8, 9, 12 and
  14) × 2 h, on main and on the branch.
- Report per lineage:
  - alive at 60 and 120 min;
  - the new `balance:` line (B, R̂, access, sated share);
  - for bellwings, the gut line;
  - for packs, the feasts and chase ends.
- Use `nice -n 10`, one thread per run, at most 8 at once on the desktop.
- Eidolon (`ssh eidolon.local`) is available with up to 8 threads. An overnight soak uses 8
  more, and the plants worker may use 8. Eidolon's shell is fish, so run a bash script file
  and give your binary a new name.

## Constraints

- **Worktree:** `git worktree add .claude/worktrees/bellwing -b bellwing main`. Commit with
  explicit paths only. Never merge, deploy, or restart a host. Headless only.
- **Stay in `crates/cubarium-voxel-fauna`** and `founder_tally`. Don't touch flora or
  `cubarium-search`.
- `design/parameters.md` is generated. On a conflict, regenerate it.
- Never `pkill -f` a pattern in your own command line. Leave no background shells.
- End each commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 40 lines)

- branch and head;
- each item's diagnosis, with evidence (numbers, `file:line`);
- the fixes and their tests;
- the measure table, main against the branch.
