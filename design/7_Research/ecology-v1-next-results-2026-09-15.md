---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Ecology v1 next: calibration, apex comparisons, presentation, retraining, fresh world

Consolidated result for
[the next-steps handoff](../handoffs/ecology-v1-next-fable-2026-09-15.md).
Evidence, not decisions. Each workstream's own note carries its full tables;
this document records the dispatch, the budgets actually spent, what each
stream established, and what remains open. **A completed assignment is not a
healthy ecosystem**: the world now on the cube passes every pre-registered
gate on held-out seeds and still loses one founder kind in every run and
never lets its predator reproduce.

## Dispatch and budget checkpoint

| stream | brief | worker | status |
| --- | --- | --- | --- |
| A calibration + apex arms | [brief](../handoffs/ecology-v1-calibration-opus-2026-09-15.md) | Opus, high, on `main` | **done** |
| B presentation | [brief](../handoffs/ecology-v1-presentation-opus-2026-09-15.md) | Opus, high, isolated worktree | **done**, merged `7d9a5ae` |
| C fresh forager training | [brief](../handoffs/ecology-v1-training-opus-2026-09-15.md) | Opus, high, on `main` | see §C |
| D fresh display world | Fable | — | **done**, build `7d9a5ae`, `fast-leaf` |

Fable's own review of each stream was one targeted pass: A's harness diff
read for equation touches (none; `cubarium-core` untouched), one recorded
row replayed (`REPRODUCED`), the search suite re-run (76 passed); B's core
diff read (one read-only `RenderView` field), the contact sheet inspected,
host and render suites re-run on the merged tree. No repair cycle was needed
for A or B. Astra was not re-engaged: the handoff asked for no repeat of the
ecology-v1 review.

Compute actually spent, all on this host, ≤ 8 workers throughout:

| stage | cap | actual |
| --- | --- | --- |
| A smoke | 5 min | 2.1 min |
| A screen (270 runs, 180 k ticks) | — | 18.0 min |
| A held-out (36 runs, 360 k ticks) | — | 4.8 min |
| **A total** | **60 min** | **24.9 min**, 63.7 M ticks, 24 MiB RSS, 2.9 MiB stored |
| B viewer check | — | a few minutes, scratch state deleted |
| C training | 20 min | see §C |
| C evaluation | 10 min | see §C |

Model usage is not exposed by the harness. Subagent token use as reported
by the orchestration tool: A ≈ 379 k, B ≈ 409 k.

## A — calibration and matched apex comparisons

Full note: [ecology-v1-calibration-2026-09-15.md](ecology-v1-calibration-2026-09-15.md).
Commits `e635088` (harness), `e97c198` (checkpoint, before the campaign),
`d5e7d6c` (note). Harness: ecology v1 component vector, a 13-name parameter
box replacing the M1 vector, a `calibrate` matrix runner (candidates × seeds ×
apex arms 0/1/2, apex introduced at tick 6,000, never restocked), guild census
from `cap_foliage`/`cap_detrital`, late window = last 20 %. Six conjunctive
gates were declared before the screen ran (sound, persists, vegetated ≥ 0.5
of opening foliage, turning over, guilds intact, stands intact ≥ 0.8).

**The result that reframes the brief.** At whole-world scale the shipped
provisional defaults do not fail the way the §13 fixtures failed. Baseline,
6 seeds × 3 arms, 150 sim min: foliage 2.3× its opening value, 1,118 of
1,280 cells alive throughout, ≈ 2 stand deaths per late window, dead wood
≈ 0.1 m, **0 foliage recovery events in all 270 runs** because there were
almost no depletion events. A prey body visits ≈ 300 distinct cells per
30-minute window; B1b and B6b had confined animals to 25 and 49 cells. The
fixture failures are statements about confinement.

**What does fail is variety**, which is what Wrysk reported: 2.1 of 4 founder
kinds alive at 150 min, the skimmer gone in 17 of 18 baseline runs, the
generalist guild in 12 of 18. The sharpest trade-off measured: litter is the
detritivore's only food and litter is leaf senescence, so every candidate
that greens the world by lowering `producer.mortality` starves the
detritivore guild (18 of 18 runs for `plant-first` and `recycle-fast`, 0 of
18 for every candidate that leaves it alone).

**Apex arms**: over 180 runs with one or two apexes, 0 matings, 0 births, 0
emergences, 0 survivors; predation is 1–3 % of deaths; paired arm differences
are all under 0.2 seed standard deviations. The apex is an input, not a
population, at every candidate ecology; its constants were held unsearched by
the brief and this is their measured effect.

**Held-out** (4 seeds, 300 sim min, all arms): the baseline and both
shortlisted candidates pass all six gates on every seed. The guild-loss gate
that discriminated in the screen did not replicate for the baseline; it is a
seed-dependent event. Nothing beats the defaults on vegetation or
persistence. One measure separates and replicates: `fast-leaf` keeps 3 of 4
founder kinds on 12 of 12 held-out runs against the baseline's 2–2.5.

**Selected**: `fast-leaf` (`plant.foliage_rate` 0.002 → 0.006,
`plant.maintenance` 0.0002 → 0.0001, `plant.reserve_share` 0.2 → 0.35), file
`runs/ecology-v1-calibration/selected/fast-leaf.toml`, config hash
`09e244392ec91768`; the defaults exported beside it as `baseline.toml`
(`fc1aefa33ebd70a1`). The preference rests on a reported component, not a
gate, and the note says so.

**Missing population feedbacks**, named from the measurements: nothing
couples an animal to a place (movement is charged per second, not per
distance); no consumer numerical response (late deaths are by age, population
tracks the birth interval); the detrital loop has one source and it is the
vegetation cap; the diet locus is an unregulated random walk across a hard
gate; the apex is an input; the fruit channel never opens (mean cell foliage
0.17–0.32 m against the 0.45 m ripening threshold); dead wood never appears.
A GA stage was affordable and declined: the only scalar is pre-ecology-v1 and
the discriminating quantity is a rare seed-dependent event.

Incidental, load-bearing for D: with `founders.kinds` set, `founders.count`
(72) is ignored and a default world places 24 animals
(`world/lifecycle.rs:52-63`).

## B — presentation

Full note: [ecology-v1-presentation-2026-09-15.md](ecology-v1-presentation-2026-09-15.md).
Contact sheet: [assets/ecology-v1-presentation-2026-09-15.png](assets/ecology-v1-presentation-2026-09-15.png).
Commits `785175a`, `3c06578`, `0a7ad31`, merged as `7d9a5ae`.

What changed on screen:

- **Structure comes from wood.** A cell's plant stage and a tall column's
  height follow `(W / W_max)^(1/3)` through the existing thresholds and
  hysteresis. Every living cell draws a plant (the stage-0 entry sits below
  `W_min`); B0's average stand reads stage 1, bright stage 2.
- **Foliage is a continuous fullness layer on that structure**: fullness
  `P / W` with a 0.85 shoulder, so both measured ungrazed classes read full,
  and a stripped living stand is the same sprite travelled to a dim warm
  ember tone (`0x9B4633`). The travel is one stamp with a per-pixel tone
  lerp, not a second silhouette stamp; the two-stamp version measured 16.6 ms
  worst on the half-grazed fixture, 0.06 ms inside the frame budget, and was
  replaced.
- **Dead wood** is the same silhouette in cool ash (`0x5A5E6E`) at opacity
  0.70 fading with the stock; soil-band plants and flecks read litter plus
  remains.
- `RenderView` gained one read-only field, `wood_max`; nothing else in core.

Verification: 17 new pixel-level tests in `crates/cubarium/tests/art_ecology.rs`
(states pairwise distinguishable, fullness monotone, structure preserved
under stripping, dead fade monotone to soil, no flicker: strip → reflush adds
0.000 excess frame step over a standing control, die → decompose 0.022
against a 0.05 bound, draw mutates nothing). Suites on the merged tree:
`cubarium` 586 passed / 18 ignored, `cubarium-render` 101, `cubarium-core`
467 / 2 ignored. Per-frame cost on the `animation_load` fixture went from
10.1 ms mean to 9.2 ms on the full canopy, and a fully half-grazed world costs
the same as an ungrazed one. Viewer only, on a scratch world: dense legible
vegetation, no seam discontinuity, 3.0 rendered frames per tick; **no stand
was grazed down or died in the minutes watched, so the stripped and dead
looks were verified on the sheet and in tests, not observed live.**

Veto candidates, in B's order: tall columns roughly double in height and now
appear in average-light cells (the knob is `TALL_STEP`); the soil band shows
no dead wood; the 0.85 shoulder means a bright stand holds a full canopy
through the first 30 % of depletion; the two tones; establishing cells show a
faint sprout. Not done: the dead tall column has no pixel test of its own;
`plant_reserve` is drawn nowhere.

## D — fresh development display world

Deployed at 15:38 on 2026-09-15 by Fable, no worker.

- The previous owner (pid 2352636, build `0.1.0+fd98972`, a pre-schema-16
  world seeded on 2026-09-15 04:16 with the R2c policy, at tick ≈ 818 k with
  0 neural animals left) was stopped with SIGINT and wrote its final snapshot.
  Its `state/` (schema 15, 16 MB, can never load again) was moved to the
  session scratchpad, not deleted:
  `/tmp/claude-1000/-home-wrysk-wryskware-cubarium/968856ae-…/scratchpad/state-schema15-retired-2026-09-15/`.
  Nothing else was touched.
- The display binary was built from the merged commit `7d9a5ae` in a clean
  worktree with the shared `target/` cache, because the main checkout carried
  C's uncommitted trainer edits at the time; `scripts/run-cube.sh`'s build
  step was therefore run by hand and its launch line used as is:

  ```bash
  ./target/release/cubarium run --art ./assets/atelier --state ./state \
    --sink shim --mirror-web --web-port 7393 --fps 60 --speed 1 --care \
    --fresh --config runs/ecology-v1-calibration/selected/fast-leaf.toml
  ```

  Log: `runs/cube-eco-v1-fastleaf.log`.
- Verified from `/status` and the decoded opening checkpoint
  (`inspect_snapshot state/world-0.cubw`): build `0.1.0+7d9a5ae`, schema 16,
  config version 8, seed 1, `plant.foliage_rate` 0.006 / `maintenance`
  0.0001 / `reserve_share` 0.35 (**A's `fast-leaf`, not the provisional
  default**), 24 legacy founders (burrower 4, grazer 10, glider 5, skimmer 5),
  0 neural animals, apex dormancy and encounter policies `Off` with the
  viewer's "Spawn 1 / 2 apex" controls available for the full paid lifecycle,
  no restocking. Tick advanced 499 → 699 in 10 wall seconds (20 Hz) and 3.0
  frames per tick reached the shim; at tick 2,100 telemetry read 137 bare
  cells, wood 136 m, foliage 117 m, dead wood 0, mass residual 2e-13.
- Viewer evidence: [assets/fresh-world-viewer-2026-09-15.png](assets/fresh-world-viewer-2026-09-15.png)
  (cube pane) and
  [assets/fresh-world-viewer-page-2026-09-15.jpg](assets/fresh-world-viewer-page-2026-09-15.jpg).
  **The physical cube was not inspected**; the shim received frames and the
  viewer mirrors the same bytes, which is all that was verified.
- The zsh poller left by an older session (pid 2315661, waiting on a finished
  ES run) is unrelated and was left alone.

## C — fresh forager training in `fast-leaf`

*Pending: the worker is running under its brief; this section is filled in
when it reports and Fable has reviewed it.*

## What this does and does not establish

- Food accounting, paid reproduction and the presenter's readability are
  verified by tests and by the campaign's zero invalid rows.
- The `fast-leaf` world is plausible on every held-out seed at 300 simulated
  minutes with the legacy controllers; that is five hours, not indefinite,
  and it is one candidate among several that pass.
- Unresolved ecological failures, stated plainly: one founder kind (the
  skimmer) dies in every run of every configuration; the generalist guild
  usually follows; the apex never mates, so predator presence is a transient
  input; local depletion and recovery, the mechanisms ecology v1 was built to
  show, are almost never triggered at world scale because nothing holds an
  animal to a place; fruit and dead wood, two of the looks B built, have
  almost nothing to draw at these parameters.

## Next recommendation

One bounded design task before any further search: give animals a reason to
stay (a per-distance movement cost or site fidelity) and give the diet locus
a cost of breadth (`γ > 1`) with a stabilising term; then re-run A's matrix
unchanged as the comparison. The apex mating radius is the single constant
that decides whether predators can ever be a population and should be a
Wrysk decision, not a search knob. Presentation vetoes go to a small
follow-up on `TALL_STEP` and the soil band.
