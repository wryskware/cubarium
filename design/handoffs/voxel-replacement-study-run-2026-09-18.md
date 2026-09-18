---
status: leaning
date: 2026-09-18
owner: Fable
---

# Voxel replacement study: the run, with replicate seeds, at the new speed

Wrysk authorised the study runs (2026-09-18: "use all my cpu"); they waited on
performance. Main at 7c42d98 runs ~30× real time per process. The harness is
`crates/cubarium-voxel-flora/examples/replacement.rs` (brief:
`design/handoffs/voxel-replacement-study-brief-2026-09-17.md`). The pilot
(`design/7_Research/voxel-round3-experiment-2026-09-16.md`, "Pilot —
2026-09-18") found no replacement in either direction in 4,000 s and that
descendants grow materially slower than the cap, so G is unmeasured.

## The run

- Pair: bloomcrown ⇄ umbrellafrond, **both directions**, `full` mode (three
  predeclared sites × three arms), **5 seeds × 2 noise seeds**, on the study
  outlet 0.0384 m³/s and the conditioning that `condition` settles.
- **Re-probe conditioning first** on one seed (the local water model changed
  the wet-cell distribution; the old settle at ~2,000 s + 300 s may move) and
  use the measured settle for every run. If the eligible sets moved
  materially, say so with the numbers.
- Observation budget **12,000 s** (three times the pilot's), whatever flag
  the harness takes; if `--cap` cannot lengthen, add the smallest flag that
  does, with a test that it consumes its own value like `--cap`. No other
  harness change; if the run exposes a printing bug, fix that and say so.
- Processes in parallel, `cargo -j 24`, `--release`; one process per
  (direction, seed, noise). Raw outputs under
  `design/7_Research/assets/voxel-replacement/`.

## Report (statistical, never exact)

Per direction, over the 10 replicates × 3 sites:
- how many arms completed a replacement (a descendant reached donor size and
  funded a package), how many had a first birth, how many were refused;
- distribution (min / median / max) of first-birth time, descendant wood at
  end, and the limiter that held descendants back (the harness prints it);
- the resident-only control beside each, so a declining resident is visible;
- an estimate of G per direction if any arm completed, else the bound the
  data gives ("descendants reached x % of donor_min in 12,000 s").
Verdict wording is the harness's own (coexistence evidence / possible refuge /
unresolved at cap); do not go beyond it. New section in the experiment note:
"Replacement study — 2026-09-18", with wall time and process count.

## Package

**T — replacement run**, fresh worker, Opus medium, main checkout, files: the
experiment note, the assets dir, and `replacement.rs`/`harness/mod.rs` only
for the budget flag or a printing fix. Rules: explicit-path commits, no fmt,
no tuning, short tests only, never HashMap iteration, do not touch
`design/handoffs/README.md`, the cube, `crates/cubarium-voxel/**`,
`crates/cubarium-voxel-sim/**`. Return ≤ 30 lines.
