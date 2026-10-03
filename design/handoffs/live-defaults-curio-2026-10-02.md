---
status: open
date: 2026-10-02
owner: opus-medium worker, worktree `.claude/worktrees/live-defaults`, branch `live-defaults`
---

# Lock in the curiosity result as the live defaults

Wrysk, 2026-10-02: "yeah lets get the new defaults locked in now."

The setup, from the gate in `design/cotraining-plan-2026-10-01.md` ("Result, 2026-10-02"):
- **Ecology:** C2's ecology, `config/training/curiosity-2026-10-02/C2-mean.toml`. This is a
  search genome of 41 genes; ×1 means the shipped value.
- **Brains:** C1's brains,
  `config/training/curiosity-2026-10-02/C1-centre-{frondgrazer,littershredder}.json`.

## 1. Brains: the built-in centres

- Copy the two C1 centres into `crates/cubarium/assets/policies/` with provenance names:
  - `littershredder-curio-c1-2026-10-02.json`
  - `frondgrazer-curio-c1-2026-10-02.json`
- Point the two `include_str!` calls in `crates/cubarium/src/voxel/mod.rs` (~L1812, ~L1817) at
  them. Delete the old two files; git keeps them.
- Rewrite the README table and its provenance paragraphs. State that they are the
  co-training lock step with curiosity, desktop run C1, generation 13, selected by the
  2026-10-02 gate.
- Fix any test that pins the old file names or digests. Don't weaken tests.

## 2. Ecology: bake the genome into the live presets

The genes must become plain registry-path values in each live preset's `[flora]` and
`[fauna]` tables. The binary's `--params` must not be required.
- **Tool:** add `--params FILE` (a genome) to `crates/cubarium-voxel-sim/examples/params.rs`.
  `Layers.genome` already exists; use the genome loader that `cubarium-search`'s
  `EcologyParams::load` uses.
- **Derive the edits per preset:** dump the preset alone, then the preset with
  `--params C2-mean.toml`. Every leaf that differs is an edit.
  - Group genes (`flora.@turf.*` …) expand to per-species rows.
  - Write the edits into the preset under a comment block naming the source file and the
    gate.
  - Leave a preset's other tables alone.
- **Presets:**
  - `config/desktop/terrarium.toml` and `config/voidrunner/terrarium.toml`: the world
    trained on.
  - `config/desktop/voxel.toml` and `config/tachyon/voxel.toml`, if they found shredders
    or browsers: say which you touched.
  - Don't change code defaults. Co-training worlds are built from code defaults plus
    `--world`.
- **Check:** for each touched preset, the params example's dump of the edited preset must
  equal its dump of the old preset plus `--params C2-mean.toml`, leaf for leaf, with no
  leaf missing. Report the number of edited leaves per preset.
- **Gene values:** if a gene's value is not a simple product of the default (a derived
  gene), still take the value from the dump. The dump is the truth.

## 3. A smoke

- Run `./scripts/run-voxel.sh --config config/desktop/terrarium.toml` (hidden under an agent;
  never a window on Wrysk's screen) for 60 s with no `--params` or `--founder-policy`.
- Its stderr must show the founders on gru with the new built-in centre names, and no
  refusal.
- Run `cargo test -p cubarium -p cubarium-voxel-sim` with `nice -n 10 -j 8`.

## Constraints

- **Worktree:** `git worktree add .claude/worktrees/live-defaults -b live-defaults main`.
  Commit there with explicit paths only. Never merge; Fable merges and deploys.
- **Shared desktop:** use `nice -n 10`, `-j 8`, at most 8 threads.
- **Never deploy or restart a host.** Never `pkill -f` a pattern in your own command line.
- **Don't touch** the film code (worktree `film-dry` is another session's), the sim, or the
  sense contract.
- **Leave no background shells.**
- End each commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 30 lines)

Branch and head, the presets touched with their leaf counts, the check result, the smoke's
founder lines, test results, and any deviation.
