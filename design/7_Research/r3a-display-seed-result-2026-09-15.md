---
design_status: exploration
last_reviewed: 2026-09-15
---

# R3a result: seeding trained policies onto the display cube

Evidence for the R3a brief (`design/handoffs/r3a-opus-display-seed-2026-09-15.md`).
Implementation commit `fd98972`, on top of Fable's `325fdd0`; the moving-bodies test
and this note are the commit that follows it. Nothing here is a decision;
it records what was built and what the cube did in its first minutes.

## What was built

**One founding routine, in the core.** `World::found_training_animal(pos, heading)`
(`crates/cubarium-core/src/world/lifecycle.rs`) is now the single definition of the body
every recurrent episode uses: the `TRAINING_FOUNDER_HUE` unit adult, `S = S_adult`,
`R = 0.5 · R_max`, `E = 0.75 · E_max`, `Mode::Seeking`, full hunger memory,
`Origin::Founder`, born at the world's current tick, `structure + reserve` booked into
`external_material_in`. It refuses rather than exceed `capacity.max_organisms`.
`World::found_neural_animal(pos, heading, policy)` is that plus
`attach_neural_policy`. `FOUNDER_HUE`, `START_RESERVE` and `START_ENERGY` moved to the core
beside it and are re-exported from `es::fixture`, whose `place` is now four lines that call
the core door. There is one founding routine, not two.

The two refusals that do not depend on the body — an invalid or foreign-digest policy, and
an enabled quiet extension — are made *before* the body is founded, so the ordinary case
never founds and unwinds; the unwind path (remove the body, un-book its material) remains
for the attachment failure that should be unreachable.

**Policy reader.** `crates/cubarium/Cargo.toml` gained `cubarium-search = { workspace = true }`
and the runner reads `cubarium_search::es::export::PolicyFile` as is. The workspace accepted
it without complaint: `cubarium-search` depends only on `cubarium-core` and
`cubarium-surface`, so there is no cycle. `PolicyFile` did **not** need to move to the core.

**Launch control.** `cubarium run --neural <policy.json> [--neural-count N]`, default 4,
minimum 1. It applies only to a world this run *creates*; a resumed world refuses it by name
and says `--fresh`. Copy *k* goes to cell (8, 8) of face `k mod 5` in `Face` index order —
Front, Right, Back, Left, Top — heading east in that face's chart, so the default four land
one per side face. A start cell whose standing water exceeds `water.flood` (the world's own
"producers drown here" threshold) is replaced by the nearest clear cell on that face,
searched in Chebyshev rings, and the substitution is reported on stderr.

**Status.** `/status` gained `population` and `neural_animals`. The `population` field is
beyond the brief's letter — the brief asked only for `neural_animals` — but its return
format asks for a population sample from `/status`, and a neural count with no denominator
is not readable. Both are carried by a new `FrameSink::observe_counts` with a no-op default,
alongside the existing `observe_tick`; nothing in the presenter changed.

## Tests

`cargo test -p cubarium-core`, `-p cubarium-search` and `-p cubarium` are all green.

| test | where | what it pins |
| --- | --- | --- |
| `the_training_body_is_the_unit_adult_the_fixtures_founded` | `cubarium-core/src/world/tests.rs` | every field of the founded body, with `structure = 1.0`, `reserve = 0.5`, `energy = 1.5` **written down**, not recomputed; the material booking and a zero residual |
| `a_refused_neural_founding_leaves_the_world_exactly_as_it_was` | same | a refusal changes nothing: same population, same `external_material_in`, same `state_hash` |
| `founding_past_the_capacity_cap_is_refused_by_name` | same | the cap refusal, and the invariants still hold |
| `the_neural_seeding_controls_parse_and_refuse_a_zero_count` | `cubarium/src/cli.rs` | defaults (`None`, 4), both flags parse, `--neural` without `--fresh` parses, `--neural-count 0` is refused |
| `the_status_route_reports_the_population_and_how_many_of_it_is_neural` | `cubarium/src/sink/web/tests.rs` | both fields present and zero before the first tick |
| `a_fresh_seeded_world_holds_exactly_the_cohort_and_a_resume_refuses_to_seed_again` | `cubarium/tests/run_neural_seed.rs` | `--neural-count 3` gives exactly 3; resume + `--neural` is refused naming `--fresh`; resume without it keeps all 3 through the snapshot |
| `the_default_cohort_is_four_and_they_are_still_there_after_twenty_seconds` | same | the default cohort, alive at tick 400 |
| `every_seeded_animal_leaves_its_starting_cell` | same | present **and moving**: each of four visits more than one cell over 400 ticks |
| `an_unreadable_policy_file_refuses_the_run` | same | a bad policy file stops the run, naming the file |

## Determinism guard for the delegation

`runs/es-r2c-eval/min64-g59-holdout.json` (written before the change, build
`47af2985a211-dirty`) against a rerun on this build:

```
target/release/cubarium-search es-evaluate \
  --policy runs/es-r2c-min64/centers/center-00059.json --set holdout --out …/holdout-after.json
survived 7 of 8 layouts; min 6501 ticks, mean 32313; wall 20.7 s of 300
```

Every one of the 24 per-episode fields — `ticks`, `terminal_stores`, `intake_producer`,
`intake_fruit`, `intake_detritus`, `travelled_px`, `body_lengths`, `upkeep_billed`,
`motion_billed`, `route_p_*`, `turn_*`, `distinct_cells`, `seam_crossing_ticks`,
`store_start`, `store_capacity`, `validations`, `alive`, `died_on_last_tick` — is identical
on all eight holdout layouts, and so are `layout_hashes`, `survived` (7), `min_ticks`
(6501), `mean_ticks` (32312.625), `policy_digest` and `protocol_hash`. The only differences
are `build`, `wall_seconds`, and four keys R2d added concurrently (`copies`,
`copies_alive_final`, `copies_deaths`, `reset_hidden_every`). The delegation did not move
the animal.

## The cube

The previous runner (pid 2062111, tick 228 000) was stopped and its `state/` moved to
`runs/state-retired-2026-09-15/` — moved, not deleted, so it can still be resumed if Wrysk
wants that world back. Then:

```
nohup ./scripts/run-cube.sh --fresh --neural runs/es-r2c-min64/centers/center-00059.json \
  > runs/cube-neural.log 2>&1 &
```

The launch line, exactly as printed:

```
cubarium: seeded 4 neural animals from runs/es-r2c-min64/centers/center-00059.json (generation 59, digest 0xb409fed56734b25e)
```

No "not clear ground" line: all four face-centre cells were dry.

`/status`, three samples 30 s apart (build `0.1.0+fd98972`, pid 2352636):

| wall | `world_tick` | `population` | `neural_animals` |
| --- | --- | --- | --- |
| 04:16:27 | 185 | 28 | 4 |
| 04:16:58 | 785 | 28 | 4 |
| 04:17:28 | 1386 | 28 | 4 |

20 ticks per wall second, as configured. Population is the 24 legacy founders plus the four
seeded animals.

The first two minutes of telemetry (ticks 100–2400, 24 samples): population held at 28,
**zero births and zero deaths**, mass residual within ±3.2 × 10⁻¹² throughout, occupied cells
27–28. `population_by_face` moved steadily — `[10, 4, 7, 5, 2]` at tick 100 to
`[7, 5, 5, 2, 9]` at tick 2400 — so bodies are crossing seams, not sitting where they were
put.

Watched on to tick 6157 (5.1 minutes of world time). The **legacy** population is booming:
all 24 founders opened an escrow around tick 2500 and delivered together at tick ~3050
(28 → 52), and births have continued steadily since (52 → 90 by tick 6100, still zero
deaths). Residual stayed within ±4.1 × 10⁻¹².

**The brief's known caveat: it has not happened yet.** `neural_animals` is still exactly 4
at tick 6157 while the ordinary population more than tripled, so through five minutes none
of the seeded animals has budded and no child has inherited the policy — even though the
same five minutes were long enough for every legacy founder to reproduce once. That is a
five-minute observation, not a result: the untrained `reproduce` output is still live, and
`neural_animals` is the number to watch. No bud gate was added.

Nothing odd to report otherwise: no shim errors in `runs/cube-neural.log`, no invariant
failures, no cap rejections. The world is young and food-rich, so the legacy birth wave is
ordinary M2 behaviour, not something the seeding caused.
