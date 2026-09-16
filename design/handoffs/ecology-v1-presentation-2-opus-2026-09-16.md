---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream G (Opus): presentation follow-up — the foliage shoulder, a soil-band dead-wood cue, the dead column test

Fable orchestrates. This is step 6 of the reconciled next steps in
[the consolidated result](../7_Research/ecology-v1-next-results-2026-09-15.md),
from [Astra's review](../7_Research/ecology-v1-next-review-2026-09-15.md)
finding 7 and next-steps item 6, on top of
[workstream B's result](../7_Research/ecology-v1-presentation-2026-09-15.md).
You own this brief; Fable reviews once with at most one repair cycle. Model:
Opus 5, medium reasoning effort (a bounded change to an already-built
presenter; the verification is visual plus pixel tests). Time target: half a
session. Work under `AGENTS.md` and `WORKING_POLICY.md`.

**You are working in a separate git worktree.** Set
`CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target` for every cargo
command. Touch only `crates/cubarium/src/art_present/`, `crates/cubarium/src/art.rs`,
`crates/cubarium/src/present.rs`, `crates/cubarium-render/`,
`crates/cubarium/tests/`, `crates/cubarium/examples/`, `design/7_Research/`.
No core change of any kind this time. Commit on your worktree branch, staging
by path. Do not push, tag, restart the cube, or touch `state/`, port 7393, the
shim or the running `cubarium` process. Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Read first

- `design/7_Research/ecology-v1-presentation-2026-09-15.md`: the mapping
  (stage from `(W/W_max)^(1/3)`, fullness `P/(1.0·W)` with the 0.85 shoulder,
  ember living wood `0x9B4633`, ash dead wood `0x5A5E6E` at 0.70 fading with
  `Wd`, soil band reads `D + C`, tall height from wood), the veto list, the
  flicker bound and the sheet-generation commands.
- Astra's finding 7: the 0.85 shoulder holds a bright canopy unchanged
  through its first ≈ 30 % of foliage loss, which masks depletion under the
  handoff's "reflect the actual stocks" rule; the soil band (bottom five rows
  of each side face) ignores `W` and `Wd` so a stand that dies there leaves
  no trace; the dead tall column has no pixel-level test.
- `crates/cubarium/src/art_present/habitat.rs` (`FOLIAGE_FULL`,
  `FOLIAGE_PER_WOOD`, the soil band at `:719-728`), `tall.rs`,
  `crates/cubarium/tests/art_ecology.rs` (the 17 tests you extend).

## Deliverables

1. **The shoulder comparison**: render, through the real `ArtPresenter` at
   native 64 × 64, the same strip-then-reflush sequence from B's sheet under
   `FOLIAGE_FULL` = 0.85, 0.95 and 1.0, and measure per-frame flicker (B's
   bound) and the frame at which a 30 % foliage loss first becomes visible
   (any pixel change on the stand) for each. One contact sheet
   `design/7_Research/assets/ecology-v1-shoulder-2026-09-16.png`, a few
   hundred KB at most. Recommend one value with the measurement; **do not
   change the default** — Wrysk decides after seeing the cube. Make the
   constant overridable for the sheet only (a presenter constructor argument
   or test hook), not a runtime setting.
2. **Soil-band dead-wood cue**: in the bottom five rows, when a cell has
   `Wd > 0` and `W = 0`, draw a small distinct mark in the ash tone that
   fades with `Wd`, separate from the litter ramp (do not fold `Wd` into
   litter). Keep the soil scenery driven by `D + C`. State its visible effect.
3. **Dead tall column pixel test**: a tall-column cell with `W = 0`, `Wd > 0`
   draws the ash-toned column at a height from `Wd`, distinct from the living
   column and from empty ground, fading monotonically with `Wd` to soil at
   zero.
4. **Tests, their own pass**: the soil-band mark is present for dead wood,
   absent for `Wd = 0`, monotone in `Wd`, and pairwise distinguishable from
   litter-only and from empty; the dead column test above; the existing 17
   `art_ecology` tests still pass; per-frame cost before/after on B's fixture
   (must not rise more than 0.5 ms mean).
5. **Result note** `design/7_Research/ecology-v1-presentation-2-2026-09-16.md`:
   the shoulder table (value, flicker max, first-visible-loss frame,
   recommendation), the sheet, the cue's rule and tone, test totals, cost,
   and what was not done.
6. `graft build`; commit on the worktree branch.

## Constraints

- No core change; no new asset unless unavoidable (say so); no diagnostics in
  the normal display; keep `draw` pure and re-observe idempotent; hysteresis
  and paced clips unchanged.
- No viewer run is required this time; the sheet is the evidence.

## Decision authority

Yours: the mark's shape, size and exact tone, the sheet layout, test
structure. Fable's: changing the default shoulder or any mapping constant.
Wrysk's: the shoulder value and the look.

## Verification

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium --test art_ecology
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium-render
```

## Return format

The result note, plus in the final message: branch and commit hashes, the
shoulder table, the sheet path, test totals, cost before/after, and measured
usage. Link files; paste no logs.

## Stop

Stop after the note and commit. Fable merges.
