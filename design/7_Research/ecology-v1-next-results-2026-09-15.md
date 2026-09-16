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
| C fresh forager training | [brief](../handoffs/ecology-v1-training-opus-2026-09-15.md) | Opus, high, on `main` | **done**, one integration repair (`617703f`) |
| D fresh display world | Fable | — | **done**, build `7d9a5ae`, `fast-leaf` |

Fable's own review of each stream was one targeted pass: A's harness diff
read for equation touches (none; `cubarium-core` untouched), one recorded
row replayed (`REPRODUCED`), the search suite re-run (76 passed); B's core
diff read (one read-only `RenderView` field), the contact sheet inspected,
host and render suites re-run on the merged tree. No repair cycle was needed
for A or B. C's review: the held-out evaluation re-run on its exported policy
reproduced all eight episodes field for field, and the protocol hash moves
with the config (`0x8e51a1a9b1e2742b` for `fast-leaf`, `0x65c51e05060f0d5a`
for the defaults). C's one integration defect was found by Fable running the
whole workspace: the widened `PolicyFile::new` broke the host crate's
neural-seed test, which C's brief had walled off. Fixed inline in `617703f`.
Final suites on `617703f`, release: `cubarium-core` 467, `cubarium-search`
86, `cubarium` 586, `cubarium-render` 101 passed, 0 failed. Astra was not re-engaged: the handoff asked for no repeat of the
ecology-v1 review.

Compute actually spent, all on this host, ≤ 8 workers throughout:

| stage | cap | actual |
| --- | --- | --- |
| A smoke | 5 min | 2.1 min |
| A screen (270 runs, 180 k ticks) | — | 18.0 min |
| A held-out (36 runs, 360 k ticks) | — | 4.8 min |
| **A total** | **60 min** | **24.9 min**, 63.7 M ticks, 24 MiB RSS, 2.9 MiB stored |
| B viewer check | — | a few minutes, scratch state deleted |
| C smoke + training (2,116 episodes, 17.2 M ticks) | 20 min | 2.3 min |
| C held-out + population (12 worlds, run twice) | 10 min | 1.9 min |

Model usage is not exposed by the harness. Subagent token use as reported
by the orchestration tool: A ≈ 379 k, B ≈ 409 k, C ≈ 291 k.

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
couples an animal to a place (movement is charged per distance but at a
per-pixel price cut ≈ 16.7× with the pace calibration, and no arm varied it,
so this is the supported hypothesis, not a tested cause); no consumer numerical response (late deaths are by age, population
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

Full note: [ecology-v1-training-2026-09-15.md](ecology-v1-training-2026-09-15.md).
Commits `7553975` (harness), `5711b4d` (note), `617703f` (Fable's integration
repair). The ES fixtures now carry a named ecology: `--config` on every ES
command, layout and protocol hashes that move with it, a policy file that
records its ecology and is refused by name against any other (the host's
`cubarium run --neural` did **not** make that check until the post-review
repair below), and an
`es-population` command that founds neural or legacy copies of the training
body in a whole world. The GRU, optimizer, score and R2 fixtures are byte
unchanged; `cubarium-core` untouched.

**The campaign**: exactly the brief's command, `fast-leaf` frozen at config
hash `09e244392ec91768`, protocol `0x8e51a1a9b1e2742b`, 16 of 16 updates in
139 s of the 1,200 s cap, 0 discarded work. Trained body: the founder animal
of `found_training_animal`, diet **0.7**, a generalist (`cap_foliage` 0.7,
`cap_detrital` 0.3; the first version of this note and C's brief said 0.85,
herbivore, which was wrong), births disabled in training episodes. Centre
score 6,521 → best 8,703 at generation 9 → 7,269 at generation 16. **No
centre or candidate ever reached the 36,000-tick horizon on any layout**; in
the pre-ecology-v1 world the same protocol reached it on every layout. The
selected centre (generation 9, weight hash `0x84e359e171fc7cf6`,
`runs/es-eco-v1-fastleaf/selected/center-00009-policy.json`) was chosen by
the recorded-score rule before any held-out episode.

**Held-out**: survived 0 of 8 layouts, 6,914–8,707 ticks, no censoring (every
death is observed, none reaches the horizon). Intake ≈ 0.6 m of material
served against ≈ 2.5 e of upkeep billed per episode: two different units, not
a ratio of intake to burn. The controller always starves on these patches;
whether the body's budget or this controller is the binding constraint is
not separable from these rows, because no feasible control was run on the
same `fast-leaf` layouts (corrected after review).

**Population comparison** (4 copies of the training body founded at tick 0,
neural vs legacy control, 24 legacy founders beside them, reproduction and
mutation on, apex arms 0/1/2 as A's screen, 2 seeds, 180 k ticks): the
trained controller doubles its body's lifetime (≈ 12,200 vs 5,574 ticks on
both seeds and every arm) and covers ≈ 4× the distinct ground, and the world
does not notice: producer intake −0.3 %, foliage retention within 1 %,
population within noise. It trades reproduction for range (2.5 vs 4.0
offspring). Every copy in both mixes starves; the neural lineage peaks at
6–7 and is extinct by tick 147,600 in all six trials. **Provenance, read
from the world at each birth**: every child of a neural parent is neural
(parent's policy, fresh hidden state), so the arms are neural lineages inside
a mixed population; the legacy arm never holds a neural body. Two findings
that bind future screens: the legacy copies die at ticks 5,290–5,994, before
the apex arrives at tick 6,000, so **the predator arms cannot discriminate
controllers at all yet**; and intake by food per controller is not
measurable without a per-organism intake accumulator in core, which C's
brief excluded, so food use is reported per arm only.

One quirk C found and left alone: A's `calibrate::config_hash` uses a
multiplier one hex digit longer than FNV-1a's, so it is not FNV-1a despite
its comment. It is consistent with itself, C calls it rather than
re-deriving it, and changing it would move every published hash; a later
cleanup may rename it.

Policy on the display: **not installed.** It survives about ten minutes on
its own and would add nothing visible; the ordinary
`cubarium run --fresh --neural <policy>` control remains the door if Wrysk
wants to see it, and it would require a fresh world.

## Review and repair (Astra, 2026-09-15)

Astra's review of this package is
[ecology-v1-next-review-2026-09-15.md](ecology-v1-next-review-2026-09-15.md).
Disposition: keep the presentation work and the bounded experiment artifacts;
correct the scientific interpretation and one host seam before treating the
package as a basis for another training or ecology change. One repair cycle
was spent, all on `main`:

- **P1, training body misidentified.** The fixture's body is the `diet` 0.7
  generalist, not a 0.85 herbivore; the error was in Fable's brief and was
  copied into C's note and this one. Corrected in place in both notes.
- **P1, material versus energy.** "Intake ≈ 0.6 m against ≈ 2.5 e" was
  presented as an energy shortfall; it is not a ratio. The "body budget, not
  controller" conclusion is withdrawn to "starves on every patch; body and
  controller confounded until a feasible control runs on the same layouts".
- **P1, host did not enforce the ecology.** `cubarium run --neural` checked
  the schema digest only. Repaired: `seed_neural_animals` now builds the
  ecology from `--config` as loaded (the defaults without it), exactly what
  `es-train --config` hashed, and calls `PolicyFile::check_ecology`; three
  host tests cover mismatch both ways, an absent hash, and a match under
  `--config` with a `--seed` override. Dormant on the live cube (no policy is
  installed), fixed before the control is used.
- **P1, gates and selection.** The six gates are minimum-plausibility gates
  that do not test variety, apex mating, depletion/recovery, fruit or dead
  wood; preferring `fast-leaf` over the baseline was a post-hoc choice on a
  reported component, and it costs about half the living wood. Both notes now
  say so; "for free" and "plateau" are withdrawn.
- **P1, movement cost.** The motor bill is per distance, not per second; the
  per-pixel price was cut ≈ 16.7× with the pace calibration. Corrected; the
  spatial-dilution reading is kept as the supported hypothesis, not a cause.
- **P2, residual arithmetic.** The retained rows give mass ≤ 4.6e-10 (screen)
  and ≤ 7.8e-10 (held-out), arm-0 energy ≤ 5.5e-10, not the 1.4e-10/1.2e-10
  quoted. All inside tolerance; corrected.
- **P2, presentation vetoes for Wrysk** (not applied): the 0.85 foliage
  shoulder hides the first ≈ 30 % of foliage loss on a bright canopy, so
  compare 0.85/0.95/1.0 on the cube before accepting it; the soil band
  suppresses dead wood entirely, so a separate dead-wood mark is owed there.
  `TALL_STEP` stays until Wrysk sees the physical cube.
- **P3, hash name and apex diagnosis.** `config_hash` is not FNV-1a (already
  recorded); "refused by name" means a named error, not label equality. The
  10 px mating radius is not shown to be the deciding constant: no
  ready-pair distance or per-predicate failure count was recorded.

## What this does and does not establish

- Food accounting, paid reproduction and the presenter's readability are
  verified by tests and by the campaign's zero invalid rows.
- The `fast-leaf` world passes six minimum-plausibility gates on every
  held-out seed at 300 simulated minutes with the legacy controllers; that is
  five hours, not indefinite, the baseline passes the same gates, and the
  preference for `fast-leaf` (one more founder kind kept, at about half the
  living wood) was made after the result was seen. It is a provisional
  development configuration.
- Unresolved ecological failures, stated plainly: one founder kind (the
  skimmer) dies in every run of every configuration; the generalist guild
  usually follows; the apex never mates, so predator presence is a transient
  input; local depletion and recovery, the mechanisms ecology v1 was built to
  show, are almost never triggered at world scale, with cheap wide-ranging
  movement the leading but untested hypothesis; fruit and dead wood, two of the looks B built, have
  almost nothing to draw at these parameters.

## Next recommendation

Fable's first recommendation was: intake accumulator plus energy-budget
measurement; then one coupled design task (place coupling plus diet-breadth
cost `γ > 1`); the apex mating radius as a Wrysk decision. Astra's review
agrees that measurement precedes search but reorders and splits it, and Fable
accepts that order. The reconciled recommendation, measurements before any
new equation:

1. **Per-body store budget and one matched feasibility experiment** (Astra 1,
   Fable 1): the per-organism accumulator, the actual body named, and on the
   same `fast-leaf` layouts a stationary grazer, the disclosed mobile control,
   the initial centre and generation 9, each with served → digestible →
   credited → oxidised → billed → terminal stores. This is the single most
   informative cheap experiment; it says whether body, controller or
   relocation binds.
2. **Host ecology guard** (Astra 2): done in this repair cycle.
3. **Movement cost alone, as a matched arm** (Astra 3): `organism.move_cost`
   at 0.00036 / 0.0018 / 0.006 with everything else as A's screen; measure
   cells per body per window, revisit interval, residence time, depletion and
   recovery crossings, death cause and net energy margin. Only if range does
   not fall or deaths rise without depletion appearing is a new site-fidelity
   mechanism worth designing. Fable's "coupled design task" is withdrawn in
   favour of this.
4. **Do not couple `γ > 1` to that** (Astra 4): first report births, deaths
   and lifetime intake by founder form × diet bin × guild to learn whether the
   skimmer is lost through body, controller, habitat or realised diet yield;
   a `γ` matrix (1, 1.5, 2) is its own later experiment and must show stable
   occupation of both food channels, not just fewer intermediates.
5. **Apex opportunity audit before any radius decision** (Astra 5): count
   simultaneously ready pairs, minimum ready-pair distance and failures by
   mating predicate on the two-apex baseline and `fast-leaf` arms. Only if
   ready pairs exist and never come within 10 px does the radius become
   Wrysk's choice. Fable's "radius as a Wrysk decision now" is withdrawn
   until that audit.
6. **Presentation follow-up** (Astra 6, B's veto list): shoulder
   0.85/0.95/1.0 compared at native scale, a soil-band dead-wood cue, the
   missing dead-column pixel test; `TALL_STEP` waits for the physical cube.

Astra reads the open failures as several causes, not one: no depletion and
no dead wood are probably one spatial-pressure cluster (price, behaviour,
density and area still confounded); absent fruit is a separate threshold
problem (mean `P` below the 0.45 m ripening threshold, which more depletion
would worsen); skimmer-form and generalist-guild loss may overlap only because
the founder skimmer is a generalist; apex non-reproduction is separate again.
Another joint calibration or training campaign before these checks would
mostly fit ambiguity.
