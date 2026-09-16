---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Ecology v1 training — one fresh forager in `fast-leaf`, and the two numbers that disagree

Workstream C of
[the ecology v1 next-steps dispatch](../handoffs/ecology-v1-next-fable-2026-09-15.md),
under [the training brief](../handoffs/ecology-v1-training-opus-2026-09-15.md),
after workstream A's
[calibration result](ecology-v1-calibration-2026-09-15.md) selected `fast-leaf`.

Evidence, not a decision. Nothing here changes the GRU, the observation or action
layout, the optimizer, the score, the layouts, the motor contract, an ecological
equation or the snapshot schema. **`cubarium-core` is not modified at all**: not
one file under `crates/cubarium-core/` is touched by this workstream, and the
presenter, `crates/cubarium/`, `crates/cubarium-render/` and `assets/` are
untouched as well.

The short version. One fresh policy was trained from scratch in the calibrated
`fast-leaf` ecology. It **doubles** the survival of the body it drives — 12,263
ticks against the same body's 5,574 on the ordinary controller — and covers four
times the ground doing it. It **still starves**: nothing survives a held-out
episode, the imported lineage is extinct by tick 147,600 in every arm, and at the
scale of the whole world the two arms are indistinguishable in population, food
use and vegetation. The capability is real. What ends the run is starvation on
every layout; whether the body's budget or this short search is the binding
constraint is **not** settled here (see "Correction after review" below).

**Correction after review (Astra, 2026-09-15).** Two statements in the first
version of this note were wrong and are corrected in place, with the original
wording struck where it mattered. (1) The trained body is **not** a `diet` 0.85
herbivore: `World::found_training_animal` builds `Genome::founder`, whose v2
default `diet` is **0.7**, which ecology v1 decodes as `cap_foliage = 0.7`,
`cap_detrital = 0.3` — a **generalist**. The held-out rows show it: every
solitary episode records positive detrital intake. The mistaken value came from
Fable's brief, not from the fixture. (2) "Intake ≈ 0.6 m against ≈ 2.5 e of
upkeep" is a material total against an energy bill; a served bite passes
through capability, food energy density, assimilation and battery headroom
before it is energy (contract §6.4), so the ratio is not "a quarter of what it
burns" and the sentence "the energy budget, not the search, is what ends the
run" does not follow from the recorded columns. What the rows establish is
narrower: this controller always starves on these held-out patches, and no
feasible control (the disclosed mobile control on the same `fast-leaf` layouts)
was run beside it, so body infeasibility and controller failure remain
confounded. The review is
[ecology-v1-next-review-2026-09-15.md](ecology-v1-next-review-2026-09-15.md),
findings 1 and 8.

## Build and commits

| | |
| --- | --- |
| Branch | `main` |
| Parent | `63a1f4c` (the brief is `e4df2cd`; workstream B's merge `7d9a5ae` and the consolidated results `63a1f4c` landed on `main` while this ran) |
| Harness commit | `7553975` — the named ecology, `--config` on every ES command, `es-population` |
| Note commit | this document |
| Build at training | `7d9a5aefbcf7-dirty` — `main` at workstream B's merge |
| Build at evaluation | `63a1f4c60e2a-dirty` — `main` at the consolidated results |
| Config | `runs/ecology-v1-calibration/selected/fast-leaf.toml`, config hash `09e244392ec91768` |
| Protocol hash (campaign) | `0x8e51a1a9b1e2742b` |
| Policy schema digest | `0x8be01a3aa8e9f4a2` (unchanged; the GRU is untouched) |

The evaluation ran on a later build than the training, because `main` moved under
this workstream: the presenter worker's workstream B merge (`7d9a5ae`) and
Fable's consolidated results (`63a1f4c`) landed while the campaign was running.
Neither touches the simulation the score depends on, and that is checked rather
than asserted: `es-protocol --config …/fast-leaf.toml` on the evaluation build
still prints protocol hash `0x8e51a1a9b1e2742b` and policy digest
`0x8be01a3aa8e9f4a2`, which are the checkpoint's own, and `es-evaluate` accepted
the exported policy on both counts. The task did not move between the two
builds. The exported policy carries the **training** build,
`7d9a5aefbcf7-dirty`, in its own `build` field, so its provenance points at the
run that produced it and not at the binary that read it. What did change between
the builds inside this workstream is reporting only: the per-copy accounting in
`es-population`.

## What the harness gained

A layout was always a set of overrides on top of *some* `WorldConfig`, and that
configuration was silently `WorldConfig::default()` — the pre-calibration world.
Training in `fast-leaf` therefore needed the base configuration to become a named
thing rather than an assumption.

- `es::fixture::Ecology` holds the base config, a label and a hash. The hash is
  `calibrate::config_hash` **called, not reimplemented**: the calibration's
  published `09e244392ec91768` is the same sixteen digits here. (The first
  attempt did reimplement it as FNV-1a and produced `1bec0b392ec91768`, because
  `config_hash`'s multiplier is `0x1000000001b3` and FNV-1a's is
  `0x100000001b3`. Two hashes that agreed in their low forty bits and disagreed
  above; one call removed the class of error.)
- `--config <toml>` on `es-protocol`, `es-controls`, `es-smoke`, `es-bench`,
  `es-train`, `es-export` and `es-evaluate`. `Layout::config` starts from that
  base and still applies its own overrides — founders cleared, `weather.amplitude
  = 0`, `water.rain_rate = 0`, `mechanisms.mutation = false`, its own seed — so
  an isolated episode stays isolated.
- `Protocol` gains `config` and `config_hash`, so the protocol hash moves with
  the ecology: `0x65c51e05060f0d5a` on the defaults (config hash `fc1aefa33ebd70a1`), `0x8e51a1a9b1e2742b` on
  `fast-leaf`. Every layout hash moves too, because a layout hash already covered
  the whole serialized config.
- `PolicyFile` records both, and `check_ecology` refuses a foreign one **by
  name**. A file that records *no* config — every policy written before this
  plumbing, including all of `runs/es-r2c-*` — is refused for that reason and not
  silently accepted: "unknown" is not "the defaults". Those runs were not
  evaluated here.
- `es-export --generation <g>` exports a **named recorded centre**. The selection
  rule is the highest recorded centre score, earliest generation on ties, and
  that is rarely the last generation; a command that could only export the final
  `theta` would quietly export a different policy than the one selected. The
  weights come from the file the run wrote at the time and are checked against
  the checkpoint's recorded hash.

Deliberately not done: no change to `neural/` at all (the plumbing needed none),
no change to `calibrate`'s candidates, gates, rows or summaries, no new
ecological quantity.

## The plumbing smoke (≤ 2 wall minutes; it took 0.4 s)

`es-smoke --config …/fast-leaf.toml`, written to
`runs/es-eco-v1-fastleaf/smoke.json`. Its own protocol — two pairs, 2,000 ticks,
one layout — hashes `0x267cf7dab88393cb`; the campaign's is different and is
above.

The smoke now checks the layouts against the configuration **before** any ES
arithmetic. Every painted cell must be a live stand under *that* config's plant
constants, which is the thing a new ecology could silently break: foliage with no
wood under it is a scene of dying leaves, and the score would then be measuring
the dieback rather than the forager.

| layout | layout hash | cells | population | Σ P | Σ W | Σ Q | live stands |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `t1-corridor` | `0xb5d6c6d706b2fc11` | 27 | 1 | 34.425 | 17.212 | 8.606 | 27/27 |
| `t2-weak-open` | `0xb2f8ce155dd87041` | 27 | 1 | 31.050 | 15.525 | 7.762 | 27/27 |
| `t3-scatter` | `0x87c2dafcdaf8265c` | 35 | 1 | 48.450 | 24.225 | 12.113 | 35/35 |
| `t4-ring` | `0x9a47ba729f2c4a9a` | 29 | 1 | 36.075 | 18.038 | 9.019 | 29/29 |

Every cell satisfies `W = P/α` and `Q = q_cap·W` to 1e-12 under `fast-leaf`'s own
α and reserve cap, every world passes `check_invariants`, and the ES repeat at a
different worker count reproduced scores, episodes, centre bytes and Adam state
exactly: **deterministic**.

## The campaign, once

```bash
cargo run -p cubarium-search --release -- es-train \
    --config runs/ecology-v1-calibration/selected/fast-leaf.toml \
    --pairs 16 --generations 16 --horizon 36000 --workers 8 \
    --wall-seconds 1200 --train-seed 20260915 --center-eval true \
    --out runs/es-eco-v1-fastleaf
```

Run once, as written. No retry, no continuation, no extra seed, no horizon
change, no tuning. 16 of 16 updates completed in **138.7 s** of the 1,200 s cap,
2,116 episodes, 17,180,924 ticks, **0 discarded** episodes and 0 discarded ticks.

The trained body is the founder animal of `World::found_training_animal`: hue
`TRAINING_FOUNDER_HUE` = 0.5, `diet` **0.7** — the generalist, `cap_foliage`
0.7 and `cap_detrital` 0.3 (corrected after review; the first version of this
note said 0.85, herbivore, which was Fable's brief and not the fixture:
`crates/cubarium-core/src/genome.rs:56-58,207-223,408-433`) — structure at
`structure_adult`, reserve 0.5 of `R_max`, energy 0.75 of `E_max`, and births
disabled inside a training episode through the diagnostic seam's
`bud: Some(false)`. One animal, no other organism anywhere.

### Trajectory (recorded centre scores)

| | generation | score |
| --- | --- | --- |
| Initial centre | 0 | **6,521.000** |
| Best centre | 9 | **8,703.000** |
| Final centre | 16 | **7,269.000** |

The score is `t_min + 0.25 · mean normalised terminal stores`, and no candidate or
centre ever survived a layout, so every score above is essentially the **minimum
survival ticks over the four layouts** — 326 s of world time at the start, 435 s
at the best centre. Per-generation best candidate scores rose from 7,997 to a
peak of 10,276 at generation 12; medians rose monotonically-ish from 6,884 to
7,830. The centre series is noisy, which is expected: a 33rd episode set is one
sample of a stochastic world, not a smoothed statistic.

### Selection

Highest **recorded centre score**, earliest generation on ties, **training
results only**, frozen before any held-out episode ran.

| | |
| --- | --- |
| Selected centre | generation **9**, score **8,703.000** |
| Weights | `runs/es-eco-v1-fastleaf/centers/center-00009.json`, FNV-1a `0x84e359e171fc7cf6` |
| Exported policy | `runs/es-eco-v1-fastleaf/selected/center-00009-policy.json` |
| Round trip | exact, bit for bit; 400 ticks of ordinary core inference on `t1-corridor`, world consistent |

The final centre (7,269) is *worse* than the selected one, which is why the
selection rule reads the recorded history rather than the last checkpoint.

## Held-out evaluation

```bash
./target/release/cubarium-search es-evaluate \
    --policy runs/es-eco-v1-fastleaf/selected/center-00009-policy.json \
    --config runs/ecology-v1-calibration/selected/fast-leaf.toml \
    --set holdout --horizon 36000 --wall-seconds 300 \
    --out runs/es-eco-v1-fastleaf/holdout.json
```

One episode per held-out layout, plain probe (one copy, no hidden-state reset).
3.8 s wall. Intake columns are **served bites** in material; `travel` is measured
pixels, `BL` is body lengths.

| layout | ticks | alive | stores end | intake P | intake F | intake D | upkeep billed | travel px | BL | distinct cells | route P start → end |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `h1-holdout` | 8,560 | no | 0.000 | 0.364 | 0.253 | 0.265 | 2.654 | 1,124 | 450 | 251 | 34.05 → 14.30 |
| `h2-holdout` | 6,914 | no | 0.000 | 0.138 | 0.029 | 0.041 | 2.143 | 900 | 360 | 207 | 31.88 → 14.52 |
| `h3-holdout` | 8,707 | no | 0.000 | 0.377 | 0.278 | 0.287 | 2.699 | 1,144 | 458 | 268 | 36.81 → 14.21 |
| `h4-holdout` | 8,070 | no | 0.000 | 0.273 | 0.202 | 0.198 | 2.502 | 1,053 | 421 | 168 | 34.92 → 14.70 |
| `h5-holdout` | 7,848 | no | 0.000 | 0.249 | 0.171 | 0.169 | 2.433 | 1,033 | 413 | 232 | 34.39 → 13.74 |
| `h6-holdout` | 7,722 | no | 0.000 | 0.255 | 0.138 | 0.148 | 2.394 | 1,016 | 406 | 228 | 33.25 → 14.78 |
| `h7-holdout` | 8,282 | no | 0.000 | 0.331 | 0.207 | 0.242 | 2.567 | 1,088 | 435 | 260 | 34.52 → 14.27 |
| `h8-holdout` | 7,743 | no | 0.000 | 0.242 | 0.147 | 0.146 | 2.400 | 1,006 | 403 | 244 | 36.40 → 13.99 |

Survived 0 of 8. Min 6,914 ticks, mean 7,981 ticks (399 s of world time).

**Censoring, as R2d finding 1 requires it to be stated.** There is none. The
horizon is 36,000 ticks and the longest episode ended at 8,707, so every survival
time here is *observed exactly*; no episode is right-censored at the horizon and
none was cut short by the wall clock (3.8 s of a 300 s cap). The mean is a mean of
complete lifetimes, not of a censored sample — which also means it cannot be
compared with any number drawn from a run where animals did reach the horizon.

**What the intake columns say.** Over roughly 8,000 ticks the animal takes in
about 0.25 of producer, 0.18 of fruit and 0.19 of detritus — roughly 0.6 of
material in total — while its upkeep alone bills about 2.5 of energy. Those two
numbers are in different units (material served versus energy billed) and are
not a ratio of intake to burn; the energy the served material became is not a
recorded column (corrected after review). The body does not fail to find food;
it fails to find *enough* to survive, and the route's own foliage falls from
~34 to ~14 over the episode. R2d finding 3's caution applies in the other
direction here: survival on a patch is not sustained foraging, and this policy
is not even achieving survival. Whether the shortfall is the body's budget on
these patches or this controller leaving reachable intake unused is not
separable from these rows alone.

## Population-level comparison

A new, additive subcommand, `es-population`. A whole `fast-leaf` world with its
own 24 founders (`founders.kinds` gives 4 + 10 + 5 + 5; `founders.count` is
ignored when kinds are set) receives **4 copies of the same body** at tick 0:

- the **neural** arm founds them through `World::found_neural_animal` with the
  selected policy;
- the **legacy** arm founds them through `World::found_training_animal` and
  leaves them on the ordinary controller.

Both arms import the same genotype, the same four places (deterministic in the
seed, same for both mixes) and — measured, not assumed — **the same 6.000 of
material**. Reproduction and mutation are on for every body. Apex arms 0/1/2 are
matched exactly to A's screen: `FixedHunterProfile::lanternjaw_trial` derived from
the *base* config for the seed, introduced at tick 6,000, never restocked. Two
training seeds (1001, 1002), horizon 180,000 ticks, `sample_every` 600 — the
screen's own numbers.

```bash
./target/release/cubarium-search es-population \
    --policy runs/es-eco-v1-fastleaf/selected/center-00009-policy.json \
    --config runs/ecology-v1-calibration/selected/fast-leaf.toml \
    --seeds 2 --arms 0,1,2 --copies 4 --ticks 180000 --sample-every 600 \
    --introduce-tick 6000 --workers 8 --wall-seconds 480 \
    --out runs/es-eco-v1-fastleaf/population
```

12 trials, 12 completed, 0 skipped, 54.6 s wall at 39,565 ticks/s. Worst absolute
mass residual over all trials 3.3e-10; the energy residual is 7.00 per apex adult
introduced, which is the accounted import of the cohort and is identical in both
mixes of an arm.

### Provenance, read from the world

**Every child of a neural parent was itself neural.** Across the six neural
trials there were 2 or 3 such births per trial and
`neural_children_of_neural_parents == births_of_neural_parents` in every one.

This was checked in the code as well as measured:
`crates/cubarium-core/src/world/step.rs:2467-2471` inserts
`neural::AnimalState::fresh(now + 1, parent_policy)` for a child whose parent has
a neural entry — the parent's **policy** with zero hidden state, nothing held, no
feedback and its own birth-tick phase. `world/step.rs:2130` and `:1427` remove the
entry with the body at each of the two death boundaries. R3a's statement is
correct, and these arms are therefore neural **lineages**, not four bodies. The
populations are still *mixed* in the ordinary sense — 24 legacy founders and their
descendants share the world — and every count below is split by the controller the
world reports, never by how a body was founded.

The legacy arm holds **no** neural body at any time: 0 at tick 0, 0 births, 0
deaths.

### Per arm, the whole world (means over the two seeds)

| arm | mix | final pop | final neural | births | deaths | intake P | intake detrital | foliage retention | depletion events | apex alive |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | neural | 60.0 | 0.0 | 179.0 | 147.0 | 2,412.5 | 1,511.8 | 2.094 | 1.5 | 0.0 |
| 0 | legacy | 68.5 | — | 198.0 | 157.5 | 2,419.2 | 1,493.5 | 2.086 | 1.0 | 0.0 |
| 1 | neural | 64.0 | 0.0 | 188.0 | 152.0 | 2,418.0 | 1,524.5 | 2.085 | 1.5 | 0.0 |
| 1 | legacy | 63.0 | — | 189.5 | 154.5 | 2,425.1 | 1,493.9 | 2.083 | 1.0 | 0.0 |
| 2 | neural | 61.0 | 0.0 | 185.0 | 152.0 | 2,417.8 | 1,534.1 | 2.082 | 1.5 | 0.0 |
| 2 | legacy | 60.0 | — | 184.5 | 152.5 | 2,426.5 | 1,531.0 | 2.099 | 1.0 | 0.0 |

Four imported bodies out of twenty-eight move nothing at world scale. Producer
intake differs by 0.3 %, foliage retention by under 1 %, depletion events by half
an event out of the 1,106–1,118 cells that opened with foliage. Retention above 2.0 in every arm means the
world's foliage **more than doubled** from its opening stock: this is not a world
under grazing pressure, which is the same thing A's screen found.

The neural arm at apex 0 has a lower final population (60.0 vs 68.5) and fewer
births (179 vs 198), but the two seeds disagree in size (62/58 against 80/57), so
two seeds cannot separate that from seed noise, and arms 1 and 2 show no such
gap. It is not evidence of anything yet.

### The four imported copies, body for body

This is the comparison the arms actually differ in, and the whole-world table
above hides it inside twenty-four founders the arms share.

| arm | mix | mean lifetime (ticks) | alive at end | births | distinct cells per window | body lengths |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | neural | **12,263** | 0 | 2.5 | **193.6** | **558** |
| 0 | legacy | 5,574 | 0 | 4.0 | 47.9 | 192 |
| 1 | neural | **11,942** | 0 | 2.5 | **183.6** | **541** |
| 1 | legacy | 5,574 | 0 | 4.0 | 47.9 | 192 |
| 2 | neural | **12,184** | 0 | 2.5 | **188.8** | **554** |
| 2 | legacy | 5,574 | 0 | 4.0 | 47.9 | 192 |

Every copy in every arm, both mixes, died of **starvation** — no predation, no
old age. Per-copy lifetimes: neural `[8864, 17498, 18067, 9084]` and
`[8849, 8863, 17935, 8943]`; legacy `[5808, 5316, 5462, 5562]` and
`[5994, 5290, 5528, 5633]`.

Three things follow.

1. The trained controller **doubles** the survival of the body it drives, 2.2× on
   both seeds, while covering four times the distinct ground per window and
   walking nearly three times as far. That is a real capability, measured against
   the same genotype with the same stores in the same world on the same tick.
2. The legacy copies die at ticks 5,290–5,994 — **before the apex cohort is
   introduced at tick 6,000**. Their lifetime is byte-identical across arms 0, 1
   and 2 for that reason. The predator arms cannot separate the two controllers
   at all here: nothing that distinguishes them survives to meet a predator.
3. The trained copies have **fewer** offspring (2.5 against 4.0 per trial). Budding
   needs a reserve above `bud_reserve`; a body that keeps moving spends what a
   body that sits still banks. The policy trades reproduction for range. Whether
   that trade is good is not decidable from these runs, because neither lineage
   persists.

### Inside the neural arm, by controller (means over the two seeds)

| arm | births n/l | deaths n/l | distinct cells per body-window n/l | body lengths per body n/l |
| --- | --- | --- | --- | --- |
| 0 | 5 / 353 | 13 / 281 | 400.5 / 244.8 | 3,123 / 1,662 |
| 1 | 5 / 371 | 13 / 291 | 407.8 / 253.5 | 3,108 / 1,756 |
| 2 | 5 / 365 | 13 / 291 | 396.6 / 243.2 | 3,134 / 1,632 |

Deaths of neural bodies split 4 starvation (the four founded copies) and 2–3 age
(their children, which reached `max_age_seconds`). The neural sub-population peaks
at 6–7 bodies and is **extinct by tick 147,600** in all six trials. `neural_series`
for seed 1001 arm 0 reads `4,4,4,4,4,4,6,6,…,6,5,4,4,…` — the copies bud early,
then the founders starve one by one.

(`neural_extinct_at` is 600 in every *legacy* row. That is an artefact of the
field's definition, not a finding: a legacy arm has zero neural bodies from the
start, so the first sample after tick 0 records the count as zero. The field is
meaningful only in a neural arm.)

### Apex outcomes

Every apex adult is dead by the end of every arm, in both mixes: `apex_alive_final`
0.0 everywhere, `apex_births` 0 everywhere. Apex deaths match the cohort size (1
in arm 1, 2 in arm 2). Predation deaths are 0–4 per trial (arm 1: 2.0 neural-arm / 1.0
legacy-arm; arm 2: 2.5 / 1.5, means over the two seeds) and fall entirely on
legacy bodies; no neural body was ever eaten, because none was alive in the same
place at the same time as a hunter for long enough. The apex arms are matched
scenery in this comparison, not a treatment it can measure.

### Intake by food per controller — what is measured and what is not

The brief asks for intake by food **per controller**. The core does not expose it:
`World::intake_diagnostics` returns one `IntakeDiagnostics` for the whole world,
and there is no per-organism intake accessor. The neural `Feedback.ate` array is
per-animal but is reset at every controller update (every second tick) and exists
only for neural bodies, so it is neither complete nor comparable. Adding a
per-organism accumulator would be a change to the core, which this brief
excludes.

So intake by food is reported **per arm** — the world totals in the table above —
and the controller's effect on food use is the neural-minus-legacy difference of
a matched pair, which is the population-level quantity the comparison is about.
That difference is −0.3 % on producer and +0.6 % on detrital material: nothing.
The limitation is written into `population.rs`'s module header, into the printed
report and into the JSON row, so it cannot be read as a measurement that came out
zero.

## What this does not establish

- **Not every guild.** One genotype was trained: the founder animal, `diet`
  0.7, a generalist (corrected after review). The burrower, glider and skimmer
  kinds in `fast-leaf`'s roster were
  never trained and are legacy-controlled throughout.
- **Not the apex.** No apex body is neural; the core refuses a neural apex member.
  The predator arms here are matched conditions, and in fact the imported legacy
  copies die before the cohort arrives, so the arms do not discriminate.
- **Not sustainability.** The imported lineage is extinct by tick 147,600 in every
  trial. Nothing here shows a trained forager persisting, and the held-out set
  shows it surviving 22 % of one episode horizon.
- **Not a world effect.** Four bodies in twenty-eight moved no whole-world number
  by more than about 1 %. This comparison can detect a per-body capability
  difference and cannot yet detect a population one.
- **Not two seeds' worth of confidence.** Two seeds per cell. Per-copy lifetimes
  agree closely across seeds (2.2× both times); whole-world populations do not
  (62/58 against 80/57 at arm 0). Treat the first as a finding and the second as
  noise.
- **Not a comparison with R2c.** Those policies were trained in the
  pre-calibration ecology, are refused by `check_ecology`, and were not run.

**Does the trained policy destabilise the world?** No destabilisation was detected
at this tested scale: four bodies in twenty-eight, two seeds, and per-controller
intake unmeasured, so the comparison can miss a population effect. It feeds no
better at world scale by the per-arm totals, it does not deplete more (1.5
depletion events against 1.0, out of the 1,106–1,118 cells that opened with
foliage), foliage retention is equal, and the population it joins is the same
size. It is a longer-lived, wider-ranging controller whose effect on the world,
if any, is below what this comparison can see (wording corrected after review).

## Compute and storage actually used

| stage | wall | cap | workers |
| --- | --- | --- | --- |
| Plumbing smoke | 0.4 s | 60 s (command's own budget) | 2 then 4 |
| Training campaign | 138.7 s | 1,200 s | 8 |
| **Training total** | **~140 s** | **20 min** | |
| Export + verify | ~1 s | — | 1 |
| Held-out evaluation | 3.8 s | 300 s | 1 |
| Population comparison (final) | 54.6 s | 480 s | 8 |
| Population comparison (first, without per-copy accounting) | 55.5 s | 480 s | 8 |
| **Evaluation total** | **~115 s** | **10 min** | |

The population comparison was run twice. The first run lacked the per-copy
accounting and its numbers are the whole-world half of the table above,
identical; the second added the four-copy rows and its output is what is recorded
in `runs/es-eco-v1-fastleaf/population/`. Both are counted against the budget.
No stage was extended, no stage hit its cap, nothing is left running.

Storage: `runs/es-eco-v1-fastleaf/` is **4.9 MiB** of the 20 MiB allowance —
17 centre files, the checkpoint, the generation log, the smoke, the held-out JSON
and the population rows and summary. One `target/` cache; no second build.

Tests: `cargo test -p cubarium-search` **86 passed, 0 failed** (70 unit, 4
`es_repair`, 12 `harness`), up from 76 — the 10 added cover the ecology hash
against `calibrate::config_hash`, a layout's overrides and live stands on a moved
ecology, the layout and protocol hashes moving with the ecology, the ecology
refusal by name (foreign and absent), and the population arms being matched,
reproducible and read from the world. `cargo test -p cubarium-core` **467 passed,
0 failed**, unchanged.

## Routine decisions made here, and their visible effect

1. **The config hash is `calibrate::config_hash`, called.** Effect: one number,
   `09e244392ec91768`, in the calibration note, the protocol, the policy file and
   this document.
2. **The ecology lives on the `Layout`, not threaded through every signature.**
   Effect: `episode::run`, `Plan` and `Protocol::new` are unchanged, so the R2
   fixtures, the trainer and the optimizer are untouched. The field is
   `#[serde(skip)]`; nothing in the workspace deserializes a `Layout`, and the
   field's doc comment says what a future reader gets if something starts to.
3. **A policy with no recorded config is refused, not defaulted.** Effect:
   `runs/es-r2c-*` cannot be evaluated here at all, which is what the brief
   requires, and the refusal says why.
4. **`es-export --generation`.** Effect: the exported policy is the centre that
   earned the score (generation 9, 8,703), not the run's last centre (7,269).
5. **`es-population` is a sibling subcommand, not an extension of `calibrate`.**
   Effect: `calibrate`'s candidates, gates, rows and summaries are byte-unchanged;
   the new command reuses `config_hash`, `apex_targets`, `base_config`,
   `stored_energy`, the depletion/recovery thresholds and the five-window split so
   the numbers are the screen's own quantities.
6. **Copy placements draw from the apex placement stream under keys `1000 + i`.**
   Effect: a copy placement and an apex placement never consume each other's
   randomness, and both mixes of a `(seed, arm)` get identical places.
7. **The four imported copies are reported separately from the founders.** Effect:
   the one real finding — 2.2× survival — is visible; it is invisible in the
   whole-world columns.
8. **Two seeds, `TRAINING_SEEDS[..2]`.** Effect: 12 trials in 55 s, well inside the
   evaluation cap, at the cost of not being able to separate small whole-world
   differences from seed noise. Stated above wherever it matters.

## Reproducing every stage

```bash
cargo test -p cubarium-search
cargo test -p cubarium-core
cargo build -p cubarium-search --release

# the protocol, and that its hash moves with the ecology
./target/release/cubarium-search es-protocol
./target/release/cubarium-search es-protocol \
    --config runs/ecology-v1-calibration/selected/fast-leaf.toml

# the smoke
./target/release/cubarium-search es-smoke \
    --config runs/ecology-v1-calibration/selected/fast-leaf.toml \
    --out runs/es-eco-v1-fastleaf/smoke.json

# the campaign (once; it existed before this note and is not re-run)
cargo run -p cubarium-search --release -- es-train \
    --config runs/ecology-v1-calibration/selected/fast-leaf.toml \
    --pairs 16 --generations 16 --horizon 36000 --workers 8 \
    --wall-seconds 1200 --train-seed 20260915 --center-eval true \
    --out runs/es-eco-v1-fastleaf

# the selected centre
./target/release/cubarium-search es-export \
    --checkpoint runs/es-eco-v1-fastleaf/checkpoint.json \
    --config runs/ecology-v1-calibration/selected/fast-leaf.toml \
    --generation 9 \
    --out runs/es-eco-v1-fastleaf/selected/center-00009-policy.json \
    --verify-ticks 400

# held out, then the population
./target/release/cubarium-search es-evaluate \
    --policy runs/es-eco-v1-fastleaf/selected/center-00009-policy.json \
    --config runs/ecology-v1-calibration/selected/fast-leaf.toml \
    --set holdout --horizon 36000 --wall-seconds 300 \
    --out runs/es-eco-v1-fastleaf/holdout.json

./target/release/cubarium-search es-population \
    --policy runs/es-eco-v1-fastleaf/selected/center-00009-policy.json \
    --config runs/ecology-v1-calibration/selected/fast-leaf.toml \
    --seeds 2 --arms 0,1,2 --copies 4 --ticks 180000 --sample-every 600 \
    --introduce-tick 6000 --workers 8 --wall-seconds 480 \
    --out runs/es-eco-v1-fastleaf/population
```

`es-evaluate` on the exported policy reproduces field for field; the held-out
table above is the whole of `runs/es-eco-v1-fastleaf/holdout.json`'s episode
records.

Both refusals, exercised end to end on the real files:

```
$ es-evaluate --policy runs/es-r2c-min64/centers/center-00000.json               --config runs/ecology-v1-calibration/selected/fast-leaf.toml --set holdout
Error: "policy file records no config hash: it was written before the ecology was
part of the protocol, so which world it was trained in is unknown. This evaluation
is fast-leaf (config hash 09e244392ec91768). Retrain, or evaluate it with the
build that produced it."

$ es-evaluate --policy runs/es-eco-v1-fastleaf/selected/center-00009-policy.json --set holdout
Error: "policy file was trained in ecology fast-leaf (config hash
09e244392ec91768), this evaluation is default (config hash fc1aefa33ebd70a1): the
plant, animal and detrital constants differ and the two scores are not the same
task"
```

## The next guild and lifecycle training tasks — named, not launched

These are the tasks this result implies. **None of them is started here.**

1. **The energy budget before the next campaign.** Every body in every arm and
   every held-out episode dies of starvation. Whether the forager's intake law,
   its upkeep, or this controller is the binding constraint is not yet known,
   and a second search against an unmeasured wall will find the same wall. The
   task is a measurement, not a search: the actual body (`diet` 0.7, or a
   deliberately constructed 0.85 grazer, named either way), the disclosed
   mobile control **and** a stationary grazer beside the initial centre and
   generation 9, on the same `fast-leaf` training and held-out layouts, with a
   per-body accounting of served bite → digestible share (`cap · q`) → reserve
   and battery credit → oxidation → upkeep and motor bill → terminal stores,
   over one episode. If the mobile control also dies with its sustained
   credit/bill ratio below 1, the body's budget binds; if it survives while
   generation 9 dies, the controller or the 16-update search binds. It needs
   the per-organism intake the core does not yet expose, which is the first
   thing to decide.
2. **Reproduction as part of the objective.** The trained policy trades offspring
   for range (2.5 against 4.0) because nothing in `t_min + 0.25·stores` values a
   child. A forager meant to found a lineage needs the budding decision inside the
   episode — births are currently disabled in training through the diagnostic seam
   — and a score that counts descendants. That is a change to the trainer's score
   and to the episode, which is Fable's call, not this workstream's.
3. **The detrital and aquatic guilds.** `fast-leaf`'s roster is 4 burrowers, 10
   grazers, 5 gliders, 5 skimmers. Only the grazer genotype has a policy. Each
   other kind is a separate `found_training_animal`-equivalent body, a separate
   layout family (litter and carrion for the burrower, water for the skimmer), and
   a separate campaign under the same protocol.
4. **A population screen with enough copies to see something.** Four bodies in
   twenty-eight moved no world number. A comparison that could detect a population
   effect needs the neural share to be a meaningful fraction of the world — for
   instance the whole grazer kind founded neural — and more than two seeds.
5. **The apex arms, once a forager survives to meet one.** The imported legacy
   copies die before tick 6,000. Until a controller keeps a body alive past the
   introduction tick, the predator arms cannot tell two controllers apart.

## Stop

This workstream stops at this note and its commits. Deployment is Fable's: the
policy is made available through the existing `cubarium run --neural` control only
if Fable decides so after review. Nothing here restarted the cube, touched
`state/`, port 7393, the running `cubarium` process, or the presenter worktree.
