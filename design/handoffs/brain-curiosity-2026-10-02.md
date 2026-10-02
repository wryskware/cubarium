---
status: open
date: 2026-10-02
owner: opus-medium worker, worktree `.claude/worktrees/curiosity`, branch `curiosity`
---

# Brain curiosity: a per-species exploration term in the training reward

## Why

In the co-training gate (`design/cotraining-plan-2026-10-01.md`, "Result"), breeding is
solved, but the brains park. Browsers move 0.14–0.20 of the time, against a 0.30 target
and the defaults' 0.40. The lock step's shredders move 0.22, against 0.51.

The brains' reward is net energy, including the loss at death. It pays for sitting still
whenever food is near.

## Wrysk's direction (2026-10-02)

- Add a curiosity parameter and reward to the brains, **variable by species**.
- Seedporters should have **extremely high** curiosity. They should want to explore and
  should choose out-of-reach places to nest and sleep, such as mesa tops or vaulttrees.
- Seedporter and chorister brains will later need to be much more complex than today's,
  for social behaviour and for choosing among actions. **That is not this package;** it
  will be its own design thread.

## The design (Fable's call; build it as written)

The score of one body over one window becomes:

```
score = delta + given + curiosity.weight × novelty
```

Today's two terms are unchanged: `delta` and `given` in
`crates/cubarium-search/src/es/voxel/live/window.rs:83`.

**Novelty** is ground new to this body.
- **Cells:** the ground is cut into square cells of `cell_bl` adult body lengths of the
  body's lineage. Take the adult length from the lineage physiology, `body_at(body_max)`.
- **Familiarity:** each body keeps a familiarity `f ∈ [0, 1]` for every cell it has
  entered.
  - `f` decays exponentially with a half-life of `memory_min` minutes.
  - When the body **enters** a cell (its cell changes), it earns `1 − f(cell)`, and
    `f(cell)` becomes 1.
  - Staying in a cell earns nothing.
- **Consequences:**
  - A first visit is worth 1.
  - A return after one half-life is worth 0.5.
  - Spinning in place, or rocking between two cells, earns about 0 after the first entry.
- **Normalising:** a window's novelty is the earnings divided by `n_cruise`.
  - `n_cruise` is the number of cells a straight walk at the lineage's cruise speed would
    enter in the window: window seconds × cruise m/s ÷ cell metres.
  - So novelty is about 0–1, and 1 means "covered fresh ground at cruise all window".
- **Lifetime of the memory:** the memory lives with the body for its whole life in that
  world, **across windows**.
  - Keep it on the training side, keyed by body id, outside the per-window `Window`. A
    newborn starts empty. A body's memory is dropped when it dies or is gone.
  - Nothing goes into the sim or the snapshot.
  - Unified mode founds fresh worlds every generation, so a checkpoint never needs to
    carry the memory.
- **Death:** novelty earned before a death still counts. The death's `E_end = 0` already
  dominates.

### Configuration

Each training lineage gets a `[<lineage>]` table in a new file,
`config/training/curiosity.toml`. Write it as a serde struct with `deny_unknown_fields`,
holding:

| field | meaning | unit |
|---|---|---|
| `weight` | body references the window is worth at novelty 1 | body ref per window |
| `memory_min` | the familiarity half-life | minutes, > 0 |
| `cell_bl` | the cell side | adult body lengths, > 0 |

- `--curiosity FILE` loads it into both `voxel-ecology-search` (unified mode) and
  `voxel-live-train`.
- Without the flag every weight is 0, which is today's score exactly.
- The file refuses:
  - an unknown lineage name;
  - a negative weight;
  - `memory_min` ≤ 0 or `cell_bl` ≤ 0.
- These are training knobs, not world knobs. They do **not** go into `FaunaConfig` or the
  sim schema, so no saved world is refused.
- Document each field where the other training knobs are documented. If
  `design/parameters.md` lists training knobs, add rows there.
- **They are not ecology-search genes.** Curiosity is a species' character, set by design.
  The ecology CMA must not tune it.

### Default values: calibrate, then write them into the file

- Use a short unified-mode smoke on the terrarium:
  - `--brain-pairs 8`, λ 2, 2 seeds, 1 h horizon, 10 min windows;
  - `--threads 8`, `nice -n 10`, at most 20 min of wall time;
  - see "Machine load" below.
- Per lineage, over the twin pairs, measure:
  - the standard deviation of `delta + given`;
  - the standard deviation of novelty.
- Set `weight` so that `weight × sd(novelty)` is a fixed share of `sd(delta + given)`:

  | lineage | share | reason |
  |---|---|---|
  | frondgrazer | **0.5** | a grazer that must roam to fresh fronds |
  | littershredder | **0.3** | crawls litter, but must not park |
  | lanternjaw | **0** | an ambusher; it has no trained brain yet |

- **Seedporter:** it is not on main yet. It is being added in the roster worktree. **Do
  not add it.** Put a commented `[seedporter]` line in the file at share **≥ 2**,
  "extremely high, Wrysk 2026-10-02", so whoever merges the roster fills it in.
- Use `memory_min = 30` and `cell_bl = 1.0` for every lineage, unless the smoke shows a
  problem. If it does, say what.
- Report:
  - the measured spreads;
  - the weights you chose;
  - each lineage's mean novelty, and its moving share, from the smoke.

### Readout

- Each window's signal line gains each lineage's mean novelty and the mean curiosity term
  `weight × novelty`, beside the existing twin and pair numbers.
- `BodyScore` and the `NetEnergy` parts carry `novelty` and `curiosity`, so that a score
  can always be split into its parts.
- `voxel-ecology-check` CSV: add a per-lineage mean-novelty column. Compute it with the
  same cell and memory rules, from the check's file, or from the defaults if no file is
  given. It is a measurement only.

## Tests (written first, from this spec; do not weaken them to pass)

Put them in a new `crates/cubarium-search/src/es/voxel/live/curiosity_tests.rs`. Where a
test needs a body track, drive the accounting directly with positions; don't run a sim.

1. **Zero weight is today.** With weight 0, the score equals today's `NetEnergy` score for
   the same inputs, compared with `==`.
2. **Straight line.** A body entering N fresh cells in one window earns novelty
   `N / n_cruise`.
3. **Rocking.** A body crossing back and forth between two cells for a whole window
   earns at most 2 + ε cells' worth, where ε comes from decay over the window. Spinning
   in one cell earns 0.
4. **Half-life.** A return to a cell exactly one `memory_min` after leaving it earns
   0.5 ± 1e-6.
5. **Across windows.** The same body id keeps its familiarity across a window boundary,
   so its second window over the same ground earns less than its first. A newborn starts
   empty. After a body dies, its id holds no memory.
6. **Per lineage.** Give a frondgrazer and a littershredder identical tracks and
   energies. Their scores differ by exactly
   `(w_frond × novelty_frond) − (w_shred × novelty_shred)`, and the novelties use each
   lineage's own adult length and cruise.
7. **Config refusals:** an unknown lineage, a negative weight, `memory_min = 0`,
   `cell_bl = 0`, and an unknown field.
8. **Death.** Novelty earned before a death is kept in the score, and `delta` uses
   `E_end = 0`.

Then the existing `cubarium-search` tests must still pass.

## Constraints

- **Worktree:** `git worktree add .claude/worktrees/curiosity -b curiosity main`. Commit
  there with **explicit paths only**. Never commit to main or merge; Fable merges.
- **Machine load:** the desktop is shared with several other sessions. Run every cargo
  build or test with `nice -n 10` and `-j 8`. Run every sim with `--threads 8` or fewer.
- **No windows:** headless only (AGENTS.md).
- Never `pkill -f` a pattern that appears in your own shell's command line. Use
  `pgrep -x`.
- Don't change the ecology targets, the genes, the sense contract (`design/voxel-senses.md`),
  the sim or the live defaults.
- Leave no background shells. Run `ps --ppid $$` before you return.
- End commit messages with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 40 lines)

- Branch and head commit.
- Test results.
- The calibration table: per lineage, sd(energy), sd(novelty), the weight, the mean
  novelty and the moving share.
- Any deviation from this spec, and why.
