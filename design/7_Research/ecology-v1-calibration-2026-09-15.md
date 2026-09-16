---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Ecology v1 calibration — plausible ranges, matched apex arms, and what the model cannot feel

Workstream A of
[the ecology v1 next-steps dispatch](../handoffs/ecology-v1-next-fable-2026-09-15.md),
under [the calibration brief](../handoffs/ecology-v1-calibration-opus-2026-09-15.md).
Evidence, not a decision: nothing here changes an equation, a §11 shipped default,
an ordering, a digestive exclusion, the motor contract or the snapshot schema, and
`cubarium-core` is not modified at all.

Ecology v1 as accepted (`design/ecology-v1-contract.md`, schema 16, main through
`f9ff91e`) is the baseline and is not re-reviewed here.

## Build and commits

| | |
| --- | --- |
| Branch | `main` |
| Parent | `c8a30fa` |
| Harness commit | `e635088` — the v1 component vector, the v1 parameter box, the matrix runner |
| Result commit | *(this note, below)* |
| Build stamp in every row | `e635088fa5c4-dirty` |
| Host | 32 logical cores, 91 GiB; every stage capped at **8** workers per the brief |

The `-dirty` suffix is honest and unavoidable: the working tree carries another
worker's presenter worktree and a set of uncommitted design documents this brief is
forbidden to touch. Nothing under `crates/cubarium-search/` is uncommitted in any
row-producing build.

### Tests

| Command | Result |
| --- | --- |
| `cargo test --release -p cubarium-search` | **76 passed**, 0 failed (60 unit incl. 4 new `calibrate` + 3 new `params`, 12 `harness`, 4 `es_repair`) |
| `cargo test --release -p cubarium-core` | **467 passed**, 0 failed, 2 ignored |
| `graft build` | 6,373 nodes, 13,261 edges, clean |

`cubarium-core` is untouched, so its suite is a regression check rather than a
change check: the `hunter.rs` observation tests, the `ecology_hash` tests and the
§13.1 accounting tests A1–A9 all still pass, as they must.

## What was added, and what it does not do

`crates/cubarium-search/` gains the ecology v1 measurements and a matrix runner.
Everything it reads is already exposed by the core — `World::intake_diagnostics()`,
`world.state.ecology`, and the existing life / hunter / dormancy / encounter event
streams — so **no core file was edited and no new accessor was needed**. Nothing is
persisted, nothing is hashed, and no behaviour changed.

- `metrics.rs` — `Sample` now carries the ecology v1 stocks (`W`, `Q`, `Wd`, `C`,
  `Ce`), the three cell classes, the live census by **guild** and by visual `form`,
  and a snapshot of every cumulative counter. Because every counter in a sample is
  cumulative from the run's first tick, **any window's flow is the difference of two
  samples** and no second accumulator can drift from the first. `EcoMeasures` is one
  such window; `Components` carries one for the whole run and one for the **late
  window**.
- `evaluate.rs` — the recorder classifies each body's guild once and never revises
  it, and runs a one-second probe for foliage depletion and foraging range.
- `params.rs` — thirteen searched names, all of which exist on a schema 16 config.
- `calibrate.rs` — fifteen declared candidates, the stage runner, six plausibility
  gates, the config export.

**Definitions this note is responsible for** (brief: "the guild and depletion-event
definitions (state them)"):

- **Guild**, from the decoded phenotype and nothing else (§6.1): *herbivore* =
  `cap_foliage > 0, cap_detrital = 0`; *detritivore* = the reverse; *generalist* =
  both positive. `θ ≤ 0.5` makes the three cases exhaustive. A body's guild is fixed
  for life — `diet` mutates at conception and never after — so a guild census moves
  only through birth and death and `births = deaths + Δpopulation` holds per guild.
  Apex members are in no guild and are counted apart. The four **visual founder
  kinds** are counted separately, by `form`, because a grazer's descendants can be
  born into any guild while staying the same creature on screen.
- **Depletion event**: a watched cell's foliage falling below **0.25 ×** its own
  foliage at tick 0. **Recovery event**: that same cell later climbing back above
  **0.5 ×** the same reference. The reference is the cell's own `P_0 =
  initial_fraction · min(P_max, α·W_0)` with `W_0 ∝ L_0·μ_0` (§11), so it is already
  proportional to the cell's light band and needs no separate per-band baseline run.
  It is **not** a steady state — B0 measured bright stands still climbing at their
  horizon — so a depletion event means "this cell lost three quarters of the foliage
  it was given", not "this cell fell below its equilibrium". Cells that opened bare
  are not watched (`depletion_cells_watched` reports how many are). The probe runs
  every 20 ticks (1 s), so a dip and recovery inside one second is not resolved.
- **Spatial use**: distinct cells a prey body stood in during one window, for bodies
  present on ≥ 90 % of that window's probes, meaned over bodies. Five equal windows
  per run; the fifth is the same interval as the late window.
- **Late window**: the last fifth of the **declared** horizon. A world that never
  reached it reports `late: null`, and every ecological gate treats that as a
  failure. Censoring is not a pass.
- **Apex introduction tick**: **6,000** (5 simulated minutes) in every arm. Tick 0
  would drop the cohort into a world whose founders have not yet spread from their
  placement, so an arm difference would partly be a placement artefact; five minutes
  is long enough for the founders to disperse and short enough to leave 145 of the
  150 simulated minutes under whatever regime the arm sets. The arm-0 runs execute
  the identical branch and introduce nothing.

## Stage 2 — throughput smoke (the brief's ≤ 5 wall minutes)

The shipped defaults, one world per apex arm, at the campaign horizon, on 1 and on 8
workers. Measured on this host with nothing else running.

| | 1 worker | 8 workers |
| --- | --- | --- |
| Trials | 3 (arms 0, 1, 2; seed 1001) | 9 (arms × seeds 1001–1003) |
| Horizon | 180,000 ticks each | 180,000 ticks each |
| Wall | **74.8 s** | **49.7 s** |
| Ticks simulated | 540,000 | 1,620,000 |
| Aggregate ticks/s | **7,223** | **32,619** |
| Per-tick cost, one world one core | **138 µs** | — |
| Wall per run | 24.3–25.4 s | 23.3–26.5 s |
| Peak RSS (process) | 7.2 MiB | 14.5 MiB (≈ 1.8 MiB per concurrent world) |

The 8-worker aggregate is **depressed by its own tail**: nine jobs on eight workers
is two waves, and a single run costs the same 25 s whether one or eight are in
flight. The honest per-wave rate is `8 × 180,000 / 26 s ≈ 55,000 ticks/s`, and the
32,619 figure is what a matrix whose trial count is not a multiple of eight actually
achieves. The prediction below uses a deliberately conservative **45,000 ticks/s**,
between the two.

Smoke total: **124.5 s = 2.1 wall minutes**, inside the 5-minute cap.

Commands:

```bash
cargo build --release -p cubarium-search
./target/release/cubarium-search calibrate --stage smoke-w1 --candidates baseline \
  --seeds 1 --arms 0,1,2 --ticks 180000 --sample-every 600 --introduce-tick 6000 \
  --workers 1 --wall-seconds 300 --out runs/ecology-v1-calibration
./target/release/cubarium-search calibrate --stage smoke-w8 --candidates baseline \
  --seeds 3 --arms 0,1,2 --ticks 180000 --sample-every 600 --introduce-tick 6000 \
  --workers 8 --wall-seconds 300 --out runs/ecology-v1-calibration
```

`runs/ecology-v1-calibration/smoke-w1/` and `.../smoke-w8/`.

**`smoke-w8` is also the brief's deliverable 3**, the whole-world baseline on
`TRAINING_SEEDS[..3]` across all three arms at the campaign horizon; the screen
extends the same baseline candidate to six seeds. Its founder stocks, weather and
mutation settings are stated under "The world every arm runs" below.

## The world every arm runs

Identical for every candidate, every seed and every arm; only the thirteen searched
scalars move.

| | |
| --- | --- |
| Founder animals | **24**: burrower 4, grazer 10, glider 5, skimmer 5 (`founders.count = 72` is ignored whenever `founders.kinds` is non-empty — `lifecycle.rs:52-63`; measured opening guild census `[15 herbivore, 4 detritivore, 5 generalist]`). `initial_reserve_fraction` 0.6, `initial_energy_fraction` 0.7 |
| Founder plants (§11) | `W_0 = 0.5·W_max·L_0·μ_0`, zeroed below `W_min`; `P_0 = 0.4·P_cap0`, `Q_0 = 0.5·Q_max0` where `W_0 ≥ W_min` |
| Initial litter | `detritus.initial_dark = 1.2 · (1 − L_0)` fully charged |
| Light / moisture / water | shipped `habitat`, `water` and **default moving weather** (`weather.moving = true`, 3 blobs per channel, amplitude 0.3, periods 20/33/47 min) |
| Mutation | the core's own: `probability` 0.3, `step` 0.08, `mechanisms.mutation = true` |
| Care | off |
| Control | the legacy controller; no neural animal, no quiet policy |
| Capacity | `max_organisms` 512, `event_log` off |
| Apex profile | `FixedHunterProfile::lanternjaw_trial`, **unsearched and identical in every arm** |
| Horizon | 180,000 ticks = 9,000 s = **150 simulated minutes** |
| Sampling | every 600 ticks (30 s) → 300 samples per run; late window = last 36,000 ticks (30 sim min, exactly one §13 scenario horizon) = 60 samples |
| Foraging probe | every 20 ticks (1 s) |

## Stage 4 — the checkpoint, published before the campaign ran

### The searched vector (13 names)

`./target/release/cubarium-search params` prints it with units and rationale.

| axis | names |
| --- | --- |
| production and foliage turnover | `producer.growth` 0.004–0.020, `producer.mortality` 0.0003–0.0020, `plant.foliage_rate` 0.001–0.010, `plant.maintenance` 5e-5–6e-4 |
| plant reserve policy | `plant.reserve_share` 0.05–0.60, `plant.reflush_below` 0.10–0.90 |
| animal intake and upkeep | `organism.mouth_rate` 0.010–0.080, `organism.intake_half_saturation` 0.15–1.20, `organism.maintenance` 0.0020–0.0100 |
| maturation and reproductive timing | `organism.growth_rate` 0.004–0.030, `drives.bud_reserve` 0.55–0.95, `drives.bud_min_age_seconds` 60–900 |
| recycling | `detritus.decomposition` 0.0005–0.0080 |

Thirteen, where the brief asked for "roughly 8–12", and the deviation is deliberate.
Eleven of the names are the brief's own. `producer.growth` and `producer.mortality`
were added because they are the only two terms in

```text
(L_eff·μ)_crit = (1 + c_g)·m_p / ((1 − q_share)·g·Monod) = 2.439 · m_p / g   at N = 0.4
```

the **critical light below which a cell can hold no foliage at all**. Setting the
income-limited steady state `(1 − q_share)(c·P − m_w·W)/(1 + c_g) = m_p·P` to have a
positive root gives that condition; at the shipped defaults it is **0.305**, and the
contract's own "average" reference band sits at `L_eff·μ = 0.35`, 15 % above it.
That is not a coincidence — it is why B0 measured average `P = 0.0977` where the §11
hand table predicted 0.50, and the closed form reproduces the measurement to 3 %
(`P* = 0.6667·m_w·W / (0.6667·c − m_p) = 0.095` against 0.0977). Without `g` and
`m_p` this calibration would be tuning how fast a grazer eats a world that has
almost nothing to eat. `plant.wood_rate` and `plant.alpha` were dropped to make
room, both with reasons recorded in `params::EXCLUDED`; so are the apex profile
fields, the dormancy and encounter `pub const`s, `θ`/`γ`, the establishment block,
water, habitat, weather, founders and the capacity ceiling.

`organism.capability_gate` and `capability_exponent` were **not** added: the brief
admits them only if the baseline shows generalist dominance, and the baseline's
guild census is the test rather than an assumption. The answer is under
"Does the baseline show generalist dominance?" below.

### The run matrix

| stage | candidates | seeds | arms | horizon | sampling | trials | ticks |
| --- | --- | --- | --- | --- | --- | --- | --- |
| smoke (done) | baseline | 1 / 3 | 0, 1, 2 | 180,000 | 600 | 3 + 9 | 2.16 M |
| **screen** | **15** (baseline + 14 joint) | `TRAINING_SEEDS[..6]` = 1001–1006 | 0, 1, 2 | 180,000 | 600 | **270** | **48.6 M** |
| **held-out** | baseline + ≤ 2 shortlisted | `HELDOUT_SEEDS[..4]` = 9001–9004 | 0, 1, 2 | **360,000** (2 ×) | 600 | **36** | **12.96 M** |

Seed split: the held-out seeds 9001–9004 are **not touched** before the validation
stage, and nothing is tuned on their results. No GA stage is planned; one is
permitted only if the designed screen finishes with time to spare, and it would be
reported as a separate stage.

### Predicted wall time, from the smoke

At the conservative 45,000 ticks/s:

| stage | predicted wall |
| --- | --- |
| smoke | 2.1 min (measured) |
| screen | 48.6 M / 45,000 = **18.0 min** |
| held-out | 12.96 M / 45,000 = **4.8 min** |
| **total** | **24.9 min** against the brief's 60-minute cap |

Hard stops, so an overrun cannot happen silently: the screen runs under
`--wall-seconds 2100` and the held-out under `--wall-seconds 900`. A trial not
**started** by the cap is recorded as skipped and reported; nothing is extended and
no horizon is shortened mid-stage. Worst case, including the smoke: 2.1 + 35 + 15 =
**52.1 min**.

### The candidates

`./target/release/cubarium-search calibrate-candidates` prints all fifteen with
their overrides. Every one but the baseline moves **two or more** searched names
(a test enforces it), and each is a hypothesis about a measured Run 3 failure.

| candidate | axes | moves |
| --- | --- | --- |
| `baseline` | — | shipped §11 defaults |
| `lit-world` | P | `g` 0.016, `m_p` 0.0006 |
| `lit-world-slow-breeders` | P+D | + `bud_min_age` 300 s, `bud_reserve` 0.80 |
| `fast-leaf` | P+R | `r_p` 0.006, `m_w` 0.0001, `q_share` 0.35 |
| `fast-leaf-slow-breeders` | P+R+D | + `bud_min_age` 300 s, `growth_rate` 0.006 |
| `small-mouths` | A | `mouth` 0.020, `K_P` 0.80, upkeep 0.0035 |
| `small-mouths-lit` | A+P | + `g` 0.014, `m_p` 0.0007 |
| `joint-moderate` | P+R+A+D | `g` 0.012, `m_p` 0.0007, `r_p` 0.004, `mouth` 0.030, `K_P` 0.65, `bud_min_age` 240 s |
| `joint-strong` | P+R+A+D | `g` 0.018, `m_p` 0.0005, `r_p` 0.008, `m_w` 0.0001, `mouth` 0.020, `K_P` 0.90, `growth_rate` 0.006, `bud_min_age` 360 s, `bud_reserve` 0.85 |
| `reflush-refuge` | R+A | `p_reflush` 0.70, `q_share` 0.40, `K_P` 0.90 |
| `recycle-fast` | C+P | `k_d` 0.006, `g` 0.012, `m_p` 0.0007 |
| `recycle-slow` | C+A | `k_d` 0.0008, `mouth` 0.030, upkeep 0.0035 |
| `cheap-fast-fauna` | A+D | upkeep 0.0028, `growth_rate` 0.020, `bud_min_age` 90 s — the deliberate bracket in the failing direction |
| `plant-first` | P+R | `g` 0.016, `m_p` 0.0005, `r_p` 0.006, `m_w` 0.0001, `p_reflush` 0.50, `q_share` 0.30 — plants only, the control for whether any fauna change is needed |
| `demography-only` | D+A | `bud_min_age` 480 s, `bud_reserve` 0.88, `growth_rate` 0.005, `K_P` 0.80 — fauna only, the mirror control |

### The gates, declared before the screen ran

A `(candidate, arm)` cell is **plausible** only if, on **every** seed:

1. **sound** — status `Completed`, audits intact, no refused configuration;
2. **persists** — no seed ends with an empty world;
3. **vegetated** — late-window mean `ΣP ≥ 0.5 ×` the world's opening `ΣP`;
4. **turning over** — the late window has births **and** deaths;
5. **guilds intact** — herbivore and detritivore both alive at the end;
6. **stands intact** — late-window alive cells ≥ 0.8 × the opening alive-cell count.

They are ecological statements, not a score, and they are conjunctive. Old M1
`Scoring` reference scales are used **nowhere** in this module: they were calibrated
on the pre-ecology-v1 world and are not evidence here. Thresholds 3 and 6 are set
where the baseline's own fixture failure is unambiguous — B1b lost 99.8 % of a
bright region's foliage and killed 17 of 25 stands (retained 0.32).

Shortlisting for the held-out stage, also declared here: among cells plausible in
**all three arms**, prefer (a) the highest late-window foliage retention that is not
achieved by a dead fauna, then (b) the smallest drift between whole-run and
late-window prey population, then (c) apex survival in arms 1 and 2. At most two go
forward.

---

*Everything above was written before the screen was launched. Everything below is
what it measured.*

## Stage 5 — the screen, and the result that reframes the brief

270 trials, **1,078.6 s = 18.0 wall minutes** on 8 workers, **0 skipped**, 45,056 ticks/s
aggregate, peak RSS 24 MiB. Predicted 18.0 minutes. Zero refused configurations, zero
failures, **zero extinctions** — every one of the 270 worlds still held organisms at
150 simulated minutes.

### The headline: at whole-world scale the shipped defaults do not fail the way the fixtures failed

`design/ecology-v1-implementation-2026-09-15.md` Run 3 left three open tuning
questions: B1b's bright region run down to 0.16 % of its foliage with 17 of 25 stands
dead, B6b's population doubling 5.5× faster than its stand recovered, and B3's censored
average-light recovery. **None of them reproduces on the whole cube.** On the shipped
defaults, averaged over 6 seeds × 3 arms at 150 simulated minutes:

| | opening | late window (last 30 sim min) |
| --- | --- | --- |
| foliage `ΣP` | 107.2 m | **247.7 m — 2.31 ×** |
| living wood `ΣW` | 137.7 m | 322.7 m — 2.34 × |
| alive cells | 1,118 of 1,280 | **1,118** |
| stand deaths, whole run | — | 2.2 in the late window |
| dead wood `ΣWd` | 0 | **0.09 m** |
| foliage depletion events | — | **3.2** of 1,118 watched cells |
| foliage recovery events | — | **0** |
| leaf eaten / gross leaf grown | — | **0.42** |

The default world **greens**. Grazing takes 42 % of gross leaf production, stands
essentially never die, and the dead-wood pool the contract added is empty. The reason
is in the spatial measure: a prey body visits **303 distinct cells per 30-minute
window** out of 1,280 — a quarter of the cube each, per body, per window. B1b and B6b
pinned their animals in a 5 × 5 or 7 × 7 patch; the same animals on 1,280 cells simply
walk away from what they have eaten, and no cell ever carries sustained pressure. The
fixture failures are real *for confined animals* and are a statement about confinement,
not about the parameters.

**What the default world does fail is variety.** Per-seed final census, shipped
defaults, 6 seeds × 3 arms:

| | |
| --- | --- |
| Founder kinds still alive at 150 min | **2.11 of 4** (mean); never 4, once 3 |
| Skimmer (`form` 3) alive at the end | **1 of 18 runs** |
| Grazer (`form` 0) alive at the end | 8 of 18 |
| Herbivore **guild** extinct | 4 of 18 runs (seeds 1002 arm 1, 1006 all arms) |
| Generalist guild extinct | 12 of 18 runs |

In seed 1006 every arm ends as a **burrower-only litter economy**: `guild [0, 17, 11]`,
`forms [0, 0, 17, 11, 0]`, and the foliage then rises to 355 m because nothing eats it.
That is Wrysk's reported "lost variety", reproduced at whole-world scale and measured.

**A related fact worth carrying to workstream D**: the shipped default places **24**
animals, not 72. `founders.count = 72` is dead whenever `founders.kinds` is non-empty —
`lifecycle.rs:52-63` builds the roster from the kind counts (4 + 10 + 5 + 5) and ignores
`count`. The measured opening guild census is exactly `[15 herbivore, 4 detritivore,
5 generalist]` = grazer 10 + glider 5, burrower 4, skimmer 5.

### The trade-off table

Arm 0 (no apex), mean over the 6 training seeds, **late-window** values unless the
column says otherwise. Stocks are `Σ` over all 1,280 cells in `m`; flows are totals over
the 30-minute late window; bills are in `e`. `gates` is one character per arm: `P`
plausible, `G` failed *only* the guilds-intact gate, `-` failed something else. The full
per-arm, per-seed numbers are in `runs/ecology-v1-calibration/screen/summary.json` and
`evals.jsonl`.

| candidate | gates 0/1/2 | pop | herb | detr | gen | kinds | births h/d/g | starv | age | pred | ΣP/ΣP₀ | ΣW | ΣQ | ΣWd | litter | remains | ΣN | plant deaths | recol | deplete | recover | ΣA | ΣΔP | leaf | fruit | litter eaten | remains eaten | leaf/ΣΔP | undigest | bill | upkeep | cells/body |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | GGG | 42.8 | 28.0 | 12.8 | 2.0 | 2.33 | 14/7/0 | 7 | 18 | 0.0 | 2.31 | 323 | 146 | 0.09 | 161 | 3.3 | 695 | 2.2 | 4.8 | 3.2 | 0.0 | 1127 | 822 | 336 | 0.0 | 269 | 1.6 | 0.42 | 377 | 463 | 409 | 303 |
| lit-world | GGG | 92.0 | 85.1 | 1.9 | 5.1 | 3.83 | 58/1/5 | 2 | 67 | 0.0 | 2.25 | 474 | 104 | 0.02 | 209 | 10.4 | 480 | 0.0 | 17.5 | 0.0 | 0.0 | 1777 | 1328 | 1063 | 0.0 | 100 | 3.1 | 0.80 | 591 | 1078 | 941 | 407 |
| lit-world-slow-breeders | GGG | 89.3 | 85.1 | 1.1 | 3.1 | 3.33 | 56/1/3 | 1 | 68 | 0.0 | 2.27 | 474 | 106 | 0.03 | 209 | 11.2 | 471 | 0.0 | 16.8 | 0.0 | 0.0 | 1773 | 1323 | 1055 | 0.0 | 64 | 2.2 | 0.80 | 553 | 1054 | 920 | 405 |
| fast-leaf | PPP | 60.8 | 48.6 | 12.0 | 0.2 | 3.00 | 23/7/0 | 10 | 20 | 0.0 | 2.11 | 162 | 81 | 0.01 | 169 | 4.0 | 920 | 0.0 | 0.5 | 0.2 | 0.0 | 1157 | 937 | 526 | 0.0 | 268 | 3.0 | 0.56 | 468 | 658 | 579 | 378 |
| fast-leaf-slow-breeders | PPP | 58.9 | 47.7 | 11.2 | 0.0 | 3.00 | 23/6/0 | 7 | 21 | 0.0 | 2.11 | 163 | 81 | 0.01 | 169 | 4.1 | 920 | 0.0 | 0.5 | 0.0 | 0.0 | 1158 | 937 | 528 | 0.0 | 255 | 3.0 | 0.56 | 454 | 658 | 578 | 379 |
| small-mouths | PPG | 62.2 | 52.5 | 9.5 | 0.2 | 2.50 | 18/5/0 | 0 | 3 | 0.0 | 2.40 | 382 | 126 | 0.11 | 175 | 0.4 | 615 | 2.3 | 5.0 | 4.2 | 0.0 | 1154 | 838 | 414 | 0.0 | 112 | 0.6 | 0.49 | 271 | 511 | 475 | 266 |
| small-mouths-lit | GGG | 105.2 | 103.3 | 1.4 | 0.5 | 2.33 | 16/0/0 | 2 | 31 | 0.0 | 2.54 | 500 | 120 | 0.01 | 200 | 3.9 | 406 | 0.0 | 17.8 | 0.0 | 0.0 | 1599 | 1166 | 809 | 0.0 | 19 | 0.3 | 0.69 | 391 | 924 | 861 | 284 |
| joint-moderate | GGG | 88.0 | 86.5 | 1.3 | 0.2 | 2.00 | 59/0/0 | 7 | 55 | 0.0 | 2.34 | 277 | 137 | 0.00 | 204 | 9.3 | 651 | 0.0 | 14.0 | 0.0 | 0.0 | 1650 | 1276 | 940 | 0.0 | 25 | 0.1 | 0.74 | 449 | 978 | 904 | 342 |
| joint-strong | PPG | 113.2 | 110.5 | 2.0 | 0.7 | 3.00 | 16/0/0 | 2 | 45 | 0.0 | 3.81 | 344 | 169 | 0.00 | 268 | 6.4 | 281 | 0.0 | 19.3 | 0.0 | 0.0 | 2035 | 1626 | 1125 | 3.9 | 36 | 0.3 | 0.69 | 559 | 1296 | 1237 | 259 |
| reflush-refuge | PGP | 53.1 | 41.2 | 11.8 | 0.0 | 2.17 | 8/7/0 | 6 | 14 | 0.0 | 2.23 | 311 | 20 | 0.09 | 164 | 2.5 | 832 | 3.2 | 7.2 | 1.8 | 0.0 | 1180 | 883 | 435 | 0.0 | 222 | 1.7 | 0.49 | 378 | 588 | 533 | 363 |
| recycle-fast | GGG | 81.9 | 81.3 | 0.0 | 0.6 | 2.00 | 46/0/0 | 4 | 46 | 0.0 | 2.22 | 438 | 130 | 0.01 | 67 | 7.5 | 657 | 0.0 | 17.8 | 0.0 | 0.0 | 1630 | 1215 | 911 | 0.0 | 0 | 0.1 | 0.75 | 422 | 949 | 824 | 449 |
| recycle-slow | PPP | 58.0 | 35.9 | 21.9 | 0.2 | 3.00 | 12/14/1 | 6 | 34 | 0.0 | 2.01 | 274 | 131 | 0.09 | 315 | 4.7 | 624 | 2.5 | 4.3 | 4.0 | 0.0 | 962 | 706 | 311 | 0.0 | 562 | 8.1 | 0.44 | 633 | 472 | 416 | 282 |
| cheap-fast-fauna | PPP | 61.5 | 46.9 | 14.6 | 0.0 | 3.00 | 18/8/0 | 1 | 35 | 0.0 | 2.02 | 290 | 144 | 0.06 | 147 | 4.3 | 758 | 1.8 | 5.3 | 2.7 | 0.0 | 1054 | 777 | 385 | 0.0 | 232 | 5.1 | 0.50 | 371 | 483 | 393 | 348 |
| plant-first | GGG | 133.8 | 128.7 | 0.0 | 5.0 | 2.83 | 76/0/4 | 5 | 79 | 0.0 | 2.15 | 220 | 60 | 0.00 | 246 | 12.6 | 710 | 0.0 | 1.0 | 0.0 | 0.0 | 2053 | 1676 | 1468 | 0.0 | 33 | 1.1 | 0.88 | 707 | 1524 | 1324 | 432 |
| demography-only | PPG | 45.4 | 35.5 | 9.7 | 0.2 | 2.50 | 5/5/0 | 5 | 11 | 0.0 | 2.15 | 333 | 131 | 0.06 | 153 | 2.0 | 724 | 2.0 | 5.5 | 2.8 | 0.0 | 1101 | 799 | 370 | 0.0 | 187 | 2.1 | 0.46 | 321 | 503 | 453 | 373 |

Column key: `pop`/`herb`/`detr`/`gen` late-window mean live prey and its guild split;
`kinds` founder kinds alive at the end (of 4); `births h/d/g` late-window births by
guild; `starv`/`age`/`pred` late-window deaths by cause (collapse was 0 in all 270
runs); `ΣP/ΣP₀` late foliage over opening foliage; `ΣA` plant income, `ΣΔP` gross
foliage grown — the edible leaf replacement the intake is read against; `leaf/ΣΔP` the
fraction of gross leaf production a mouth took; `bill`/`upkeep` the complete body bill
the world booked and its mandatory half; `cells/body` distinct cells per prey body in
the late window.

### What the screen actually found

1. **Every candidate persists and greens.** 270 of 270 completed, 0 extinctions, every
   `ΣP/ΣP₀` between 2.01 and 3.81. Vegetation is not the scarce thing.
2. **The discriminating gate is guild survival, and it is decided almost entirely by
   `producer.mortality`.** Litter is the detritivore's only food, and litter is leaf
   senescence `m_p·P` plus feces. Every candidate that greens the world by lowering
   `m_p` starves the burrowers:

   | candidate | `m_p` | `k_d` | late detritivores | seeds (of 6, × 3 arms) losing the detritivore guild |
   | --- | --- | --- | --- | --- |
   | `recycle-slow` | 0.0010 | **0.0008** | **21.9** | 0 of 18 |
   | `baseline` | 0.0010 | 0.0020 | 12.8 | 0 of 18 |
   | `fast-leaf` | 0.0010 | 0.0020 | 12.0 | 0 of 18 |
   | `lit-world` | 0.0006 | 0.0020 | 1.9 | 6 of 18 |
   | `joint-moderate` | 0.0007 | 0.0020 | 1.3 | 17 of 18 |
   | `recycle-fast` | 0.0007 | **0.0060** | **0.0** | 18 of 18 |
   | `plant-first` | 0.0005 | 0.0020 | **0.0** | 18 of 18 |

   `recycle-fast` and `plant-first` lose the guild on **every seed in every arm**. The
   single parameter that decides whether the cube is green is the same parameter that
   feeds half its fauna, and nothing else in ecology v1 can supply litter.
3. **Population is set by demography, not by food.** `plant-first` produces 2,053 m of
   plant income per late window and holds 134 bodies; `demography-only` produces 1,101 m
   and holds 45 — but their foliage retention is the same, 2.15 and 2.15. Deaths in the
   late window are dominated by **age**, not starvation, in 11 of 15 candidates
   (`baseline` 18 age against 7 starvation; `plant-first` 79 against 5). The standing
   population is `lifespan / birth interval`, and the plants absorb whatever is left.
4. **`organism.mouth_rate` buys activity, not vegetation.** `small-mouths` (mouth 0.020,
   `K_P` 0.80) raises the feeding fraction from 0.48 to **0.93** and drops late-window
   starvation from 7 to 0.3 — a fifth of the mouth means a body must feed almost
   continuously to break even — while foliage retention moves only 2.31 → 2.40.
   `joint-strong` reaches feeding fraction **0.99**: every animal feeding on every
   sample, which is a world with no behavioural slack left.
5. **Fruit is a niche that does not exist.** `fruit_eaten` is **0.0 in 264 of 270 runs**;
   only `joint-strong` shows any (3.9 m). Ripening needs a cell above
   `fruit_min·P_max = 0.45 m`, and mean cell foliage across the candidates is
   0.17–0.32 m.
6. **No cell ever recovers.** 0 recovery events in **all 270 runs**, against 0–9
   depletion events. Nothing exercises §4.4's reflush at world scale, which is why
   `reflush-refuge` — the candidate built entirely around it — is indistinguishable from
   the baseline on every plant measure except its own reserve (`ΣQ` 20 against 146,
   because `p_reflush = 0.70` keeps the emergency draw permanently open).
7. **Nutrient falls everywhere.** `ΣN` drops 3–15 % over the late window in every
   candidate, furthest in `joint-strong` (342 → 284). The 300-minute held-out stage was
   the test of whether that becomes limiting; it does not (below).

## Predator effects, compared across candidates

The three arms are the **same world**: same seed, same parameter vector, same profile,
introduction at tick 6,000, cohort never restocked. Paired differences, 15 candidates ×
6 seeds = 90 matched triples:

| measure | arm 0 | Δ (1 apex) | Δ (2 apexes) | seed-to-seed sd |
| --- | --- | --- | --- | --- |
| late foliage `ΣP` | 249.53 | **+5.20** | **+2.71** | 21.7–29.9 |
| late prey population | 76.41 | +0.40 | +0.30 | 5.1 |
| late leaf eaten | 711.73 | −8.42 | −5.67 | 58–64 |
| whole-run prey deaths | 140.87 | +2.39 | +4.06 | 9.6–10.4 |
| whole-run prey births | 193.72 | +2.92 | +3.98 | 13.2–13.7 |
| late alive cells | 1,139.90 | +0.11 | +0.16 | 1.1–1.7 |
| founder kinds at end | 2.72 | +0.00 | −0.06 | 0.4–0.5 |

**Every effect is a small fraction of one seed's standard deviation.** Introducing two
apex adults into a 1,280-cell world changes nothing measurable about its plants, its
prey population, its guilds or its variety.

The apex itself, over all 180 apex-bearing runs:

| arm | paid attempts | captures | predation deaths | matings | births | emergences | active sample fraction | alive at the end | imported |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 32.0 | 1.69 | **1.69** (1.17 % of all deaths) | **0** | **0** | **0** | 0.078 | **0 of 6 seeds × 15 candidates** | 4.00 m, 7.00 e |
| 2 | 65.1 | 3.81 | **3.81** (2.59 %) | **0** | **0** | **0** | 0.089 | **0** | 8.00 m, 14.00 e |

Captures equal predation deaths to the row in every cell, which is the internal check
that the two counters are reading the same events. The cohort is active for 7.8–8.9 % of
the run and then dies, in **every candidate**: the richest world in the screen
(`joint-strong`, 113 prey, 3.81 × foliage) kills its apexes exactly as reliably as the
poorest. The arms therefore measure *one or two transient predators*, not a predator
population — see the feedbacks section.

## Stage 6 — held-out validation

Shortlist, by the preference declared at the checkpoint, among the cells plausible in
**all three** arms (`fast-leaf`, `fast-leaf-slow-breeders`, `recycle-slow`,
`cheap-fast-fauna`):

| rank by declared rule | candidate | late retention | population drift | taken? |
| --- | --- | --- | --- | --- |
| 1 | `fast-leaf` | 2.114 | 0.031 | **yes** |
| 2 | `fast-leaf-slow-breeders` | 2.112 | 0.053 | **no** |
| 3 | `recycle-slow` | 2.021 | 0.135 | **yes** |
| 4 | `cheap-fast-fauna` | 2.018 | 0.026 | no |

Rank 2 was skipped and the reason is recorded rather than hidden: the declared
preference orders *hypotheses*, and `fast-leaf-slow-breeders` is `fast-leaf` plus a
demography change that the screen shows makes no measurable difference — retention
2.114 against 2.112, population 60.8 against 58.9, three founder kinds in both. Spending
a third of the held-out budget on it would have bought no independent information,
whereas `recycle-slow` moves a different pair of axes and is the only plausible
candidate that keeps a large detritivore population. This is a judgement, it is
disclosed, and it changes which candidate was *validated*, never which was *tuned* —
nothing was tuned on the held-out seeds.

36 trials, `HELDOUT_SEEDS[..4]` = 9001–9004, all three arms, **360,000 ticks = 300
simulated minutes** (twice the campaign horizon, as predeclared). **288.8 s = 4.8 wall
minutes**, 0 skipped, 44,882 ticks/s. Predicted 4.8 minutes.

| candidate | arm | gates | `ΣP/ΣP₀` | `ΣW` | `ΣN` | pop | herb | detr | gen | kinds at end | starv | age | predation |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0 | **all pass** | 2.17 | 334 | 717 | 53.6 | 39.4 | 14.2 | 0.1 | **2.50** | 27.0 | 13.0 | 0.00 |
| baseline | 1 | all pass | 2.25 | 345 | 695 | 54.6 | 36.7 | 17.9 | 0.0 | **2.00** | 20.8 | 17.8 | 0.00 |
| baseline | 2 | all pass | 2.20 | 337 | 712 | 51.1 | 35.6 | 15.5 | 0.0 | **2.50** | 24.0 | 16.2 | 0.00 |
| `fast-leaf` | 0 | all pass | 2.19 | 169 | 911 | 66.8 | 52.0 | 14.3 | 0.5 | **3.00** | 17.0 | 27.8 | 0.00 |
| `fast-leaf` | 1 | all pass | 2.18 | 169 | 916 | 64.8 | 49.9 | 14.7 | 0.2 | **3.00** | 28.5 | 20.8 | 0.00 |
| `fast-leaf` | 2 | all pass | 2.19 | 171 | 915 | 65.5 | 50.8 | 14.5 | 0.3 | **3.00** | 22.0 | 23.8 | 0.00 |
| `recycle-slow` | 0 | all pass | 2.19 | 305 | 567 | 70.6 | 35.2 | 34.8 | 0.6 | 2.75 | 15.5 | 35.0 | 0.00 |
| `recycle-slow` | 1 | all pass | 2.19 | 307 | 571 | 69.9 | 34.6 | 35.0 | 0.3 | 2.75 | 15.5 | 33.0 | 0.00 |
| `recycle-slow` | 2 | all pass | 2.22 | 319 | 561 | 69.7 | 33.9 | 35.8 | 0.0 | 2.25 | 17.8 | 32.0 | 0.00 |

**The held-out outcome, stated plainly.**

1. **All three configurations, the shipped baseline included, pass all six gates on all
   four held-out seeds in all three arms.** The gate that discriminated in the screen —
   guilds intact — did **not** reproduce for the baseline out of sample. It is a
   seed-dependent event, not a candidate property, and the screen's `GGG` for the
   baseline should be read as "this happens on some worlds", not "this always happens".
   Saying otherwise would be tuning a story to six seeds.
2. **No candidate beats the shipped defaults on vegetation or on persistence.**
   Retention is 2.17–2.25 for the baseline against 2.18–2.19 for `fast-leaf` and
   2.19–2.22 for `recycle-slow`. Nothing separates them.
3. **One thing does separate them and it replicates**: `fast-leaf` holds **3.00 of 4
   founder kinds on every held-out seed and every arm** (12 of 12 runs), against the
   baseline's 2.00–2.50 and `recycle-slow`'s 2.25–2.75. It also holds the largest prey
   population (65–67 against 51–55), the smallest population drift, the largest nutrient
   reserve (911–916 against 695–717), and — over 300 minutes, twice the tuned horizon —
   no collapse in any measure. It also carries **about half the living wood** (169–171
   against 334–345), a larger population and a different death balance and nutrient
   inventory: those are trade-offs, not free.
4. **Doubling the horizon changes nothing qualitatively.** No extinction, no nutrient
   crash, no runaway. Whether the 3–15 % `ΣN` decline seen at 150 minutes is the
   approach to a plateau is **not** established by one 300-minute horizon: in the
   retained `fast-leaf` rows seeds 9002 and 9004 still end below their own late-window
   nutrient mean in every arm while 9001 and 9003 turn upward — mixed finite-horizon
   behaviour (corrected after review; the first version called it a plateau).
5. **The skimmer is gone in 12 of 12 runs of every configuration**, and predation deaths
   in the late window are **0.00** in every arm: by 300 simulated minutes the introduced
   apexes have been dead for hours.

## The selected configuration

**One configuration is selected**, not two.

| | |
| --- | --- |
| Candidate | `fast-leaf` |
| Moves | `plant.foliage_rate` 0.002 → **0.006**, `plant.maintenance` 0.0002 → **0.0001**, `plant.reserve_share` 0.2 → **0.35**. Nothing else. |
| Config file | `runs/ecology-v1-calibration/selected/fast-leaf.toml` |
| Config hash | **`09e244392ec91768`** |
| `param_fingerprint` | `17222255921416995329` |
| Build | `e635088fa5c4-dirty` |
| Load with | `cubarium run --config runs/ecology-v1-calibration/selected/fast-leaf.toml` |
| Metadata | `runs/ecology-v1-calibration/selected/fast-leaf.json` — hypothesis, overrides, exact `param_bits` |

The shipped defaults are exported beside it as
`runs/ecology-v1-calibration/selected/baseline.toml`, hash **`fc1aefa33ebd70a1`**,
because the held-out stage found them plausible too and workstream D should be able to
run the control without rebuilding it.

Both files are complete `WorldConfig` TOML at `version = 8` with `seed = 1`; the seed is
the intended knob for choosing a world. A unit test
(`calibrate::tests::an_exported_candidate_round_trips_through_toml`) parses each export
back through the same `toml::from_str::<WorldConfig>` + `validate()` that
`crates/cubarium/src/runner/mod.rs:106` uses and checks the config hash is unchanged.

**What the selection is, and what it is not.** `fast-leaf` was shortlisted by the
preference declared before the screen ran, and the held-out confirms it is plausible on
every seed and arm. The reason to prefer it *over the shipped defaults* is the variety
measurement — founder kinds retained — which was a reported component throughout but was
**not** one of the six pre-registered gates, nor a predeclared rule for choosing over
the baseline: preferring `fast-leaf` to the defaults is a transparent post-hoc
exploratory choice made after the held-out result was seen. The six gates are
minimum-plausibility gates (completion, non-empty world, late foliage ≥ 50 % of opening,
any births and deaths, herbivore and detritivore present, ≥ 80 % of opening living
cells); they do not require all founder kinds, the generalist guild, any apex mating,
a single depletion/recovery cycle, fruit or dead wood, which is why the baseline passes
all of them while losing every skimmer. The honest summary is: **the campaign found no
configuration decisively better than the shipped defaults on vegetation or persistence,
because the shipped defaults do not fail on those at whole-world scale; it found one
configuration that holds one more creature kind, consistently, out of sample, at the
price of about half the living wood.** `fast-leaf` is a provisional development
configuration, not a demonstrated improvement. What it does not
fix: the skimmer still dies everywhere, the generalist guild still collapses, the apex
still never reproduces, and 1 of 4 kinds is still lost.

## The population feedbacks the model is missing

Named from the measurements, each with the evidence that names it. These are diagnoses
for a later assignment to act on, not proposals; nothing here is implemented.

1. **Nothing couples an animal to a place.** A prey body visits 259–451 distinct cells
   per 30-minute window out of 1,280 — a fifth to a third of the whole cube per body per
   window — in every one of the 15 candidates. Consequently the world records 0–9
   foliage depletion events per run and **0 recovery events in all 270 runs**, and stand
   deaths of 0–3 per late window. Grazing is spatially averaged into a uniform tax of
   0.42–0.88 of gross leaf production. There is no territory, no site fidelity, no
   memory of a good patch, and travel is cheap: the motor bill is
   `move_cost · S · (speed + k·r·|ω|) · dt`, charged **per distance** covered
   (`crates/cubarium-core/src/motor.rs:354-408`), but the per-pixel price was
   deliberately cut ≈ 16.7× when cruise speed rose (`config.rs:558-565`). (Corrected
   after review: the first version of this note said the bill was per second and
   travel therefore free; it is per distance and cheap, which is a different claim.)
   Without a meaningful cost of leaving, local depletion — the mechanism B1b and B6b
   measured — is almost never observed at world scale, and the local plant feedbacks in
   §4.4 are rarely exercised. This is the supported **hypothesis**; no screen arm moved
   `move_cost`, site fidelity, density or area while holding the rest fixed, so high
   range as a *response* to thin local food is not ruled out and causality was not
   tested here.
2. **The consumer has no numerical response to its food.** Late-window deaths are
   dominated by **age** rather than starvation in 11 of 15 candidates, and the standing
   population tracks the birth interval rather than the harvest: `plant-first` produces
   1.9 × the baseline's plant income and holds 3.1 × its population at the *same* foliage
   retention, while `demography-only` produces 0.98 × the income and holds 1.06 × the
   population. Reproduction is gated on a reserve threshold that abundant food meets
   everywhere at once, so births are synchronised by the world's mean richness instead of
   being regulated by a local shortage.
3. **The detrital loop has exactly one source, and it is the term that also caps the
   vegetation.** Litter = leaf senescence (`m_p·P`) + feces. Lowering `m_p` to green the
   world starves the detritivores: `plant-first` (`m_p` 0.0005) and `recycle-fast`
   (`k_d` 0.006) lose the detritivore guild on 18 of 18 runs, `joint-moderate` on 17 of
   18, `lit-world` on 6 of 18, while the three candidates that keep `m_p` at 0.0010 lose
   it on 0 of 18. There is no wood-eater, no root channel, no moisture- or
   temperature-scaled decomposition, and remains are negligible (0.1–12.6 m standing
   against 67–315 m of litter). Half the fauna therefore lives on a by-product of the
   other half's food supply, with no feedback of its own.
4. **The diet locus is an unregulated random walk with a hard cliff.** `γ = 1` makes
   breadth free, so a generalist pays nothing and gains nothing, and `θ = 0.2` makes the
   transition abrupt. Mutation (p 0.3, step 0.08) then moves lineages across the gate at
   random. The measured consequence is monotone loss: the generalist guild is extinct in
   12 of 18 baseline runs, the skimmer in 17 of 18, and the world settles on 2.11 of 4
   founder kinds. Nothing is frequency-dependent — being the last of a kind confers no
   advantage — and nothing is density-dependent — a crowded niche is no worse than an
   empty one. With drift and no stabilising term, losing variety is the only available
   long-run outcome, and the run length only decides how much is lost.
5. **The apex is an input, not a population.** Over 180 apex runs: 0 matings, 0 births,
   0 emergences, 0 survivors, 1.2–2.6 % of deaths by predation. `MATING_RADIUS_PX` = 10
   on a 5 × 64 × 64 surface means two introduced adults essentially never meet, and the
   §13 constants that would let a paid offspring emerge are never reached because no
   offspring is ever conceived. The predator therefore cannot respond to prey density
   and cannot exert the top-down control the arms exist to measure — which is why the
   paired arm differences are all under 0.2 seed standard deviations. The apex constants
   were held unsearched by the brief; this is their measured effect.
6. **The second plant food channel does not open.** Fruit ripens only above
   `fruit_min·P_max = 0.45 m` in a cell and the mean cell foliage is 0.17–0.32 m across
   every candidate, so `fruit_eaten` is 0.0 in 264 of 270 runs. A frugivore has no
   resource to specialise on, which removes one of the three ways the model could have
   produced coexisting diets.
7. **Dead wood never appears.** `ΣWd` is 0.00–0.11 m against 162–500 m of living wood,
   because stands almost never die (0–3 per late window). The §12 presentation promise of
   visible dead wood has, at these parameters, nothing to draw.

The first four are the ones that would change the picture. The measured order of
leverage is: (1) makes every local plant mechanism inert, (3) forces a direct trade
between vegetation and half the fauna, (4) guarantees the variety loss Wrysk reported,
and (2) decouples the two halves of the food web.

## Extinctions, censoring, invalid accounting and refused configurations

Reported explicitly, as the brief requires.

- **Refused configurations: none.** 0 of 306 trials returned `Invalid`. Every declared
  candidate was checked against `WorldConfig::validate` by a unit test before any compute
  was spent, so the search box's known invalid region (`drives.bud_reserve < 0.60`) was
  never entered by a hand-written candidate. It remains inside the declared box and is
  exercised by a test rather than by the campaign.
- **Failures: none.** 0 of 306 trials returned `Failed`. No panic, no invariant
  violation, no audit drift.
- **World extinctions: none.** 0 of 306 worlds ended empty. `survived_ticks` equals the
  horizon in every run.
- **Guild extinctions: many, and they are the finding.** Counted above. They are not
  world extinctions and the `persists` gate does not see them; the `guilds_intact` gate
  does, which is why it is the only gate that ever fired.
- **Censoring.** The late window is `null` in 0 of 306 runs, so no ecological gate was
  decided by censoring. Three quantities are genuinely censored at the horizon and are
  reported as such rather than as zeros: apex reproduction (`reproduce_min_age_seconds`
  1,200 s and `reproduce_interval_seconds` 1,800 s are reachable inside 9,000 s, and were
  reached without producing a single mating, so this is a measurement, not censoring);
  foliage **recovery** events, where 0 observations against 0–9 depletions is too few
  events to state a recovery time; and the nutrient decline, which the 300-minute
  held-out shows flattening rather than continuing, but which is not followed to a
  plateau.
- **Accounting.** Three independent checks over all 270 screen rows:
  - **Census identity.** `guild_final[g] = opening[g] + births[g] − deaths[g]` with the
    measured opening census `[15, 4, 5]`: **0 violations in 810 checks**. Per-guild
    births and deaths also sum exactly to the totals in every row.
  - **Intake against production.** `(leaf + fruit eaten) ≤ (gross foliage grown +
    opening ΣP)` in every row; the worst case is **0.850** (`plant-first`, arm 2, seed
    1006), so the bound is never approached from above.
  - **Conservation.** `max_abs_mass_residual ≤ 4.6e-10` over the 270 screen rows and
    `≤ 7.8e-10` over the 36 held-out rows; `max_abs_energy_residual ≤ 5.5e-10` in arm 0
    (corrected after review: the first version quoted `1.4e-10` and `1.2e-10`, a
    report arithmetic error; every value is inside the contract's `1e-9` tolerance).
    In arms 1 and 2 the energy residual equals
    **`apex_energy_in` to within 1.1e-11** — 7.00 e for one apex, 14.00 e for two. That is
    not drift: the harness's energy identity is `ΔU = light_in − heat_out`, and the apex
    cohort's imported energy enters through neither ledger. The residual reading exactly
    the accounted import, in 180 runs, is an independent confirmation that the import is
    the *only* unbooked energy in the world. The mass identity books
    `external_material_in` and therefore stays at 1e-10 in every arm.

## No GA stage, and why

The brief permits a small GA only if the designed screen finishes with time to spare. It
did — 24.9 of 60 minutes were used — and the stage is still **declined**, for a reason
the screen itself produced. A GA needs a scalar to rank on. The only scalar this crate
has is `Components::fitness`, whose `Scoring` reference scales were calibrated on the
pre-ecology-v1 world and which the brief rules out as evidence; and the screen shows that
the quantity that actually separates configurations — guild and kind survival — is a
rare, seed-dependent event that a 400-evaluation GA would fit to noise. Inventing a new
scalar after seeing the screen would be exactly the post-hoc tuning the held-out split
exists to prevent. The honest use of the remaining budget was to leave it unspent and
name the missing feedbacks instead.

## Compute and storage actually used

| stage | trials | ticks | wall | workers | predicted |
| --- | --- | --- | --- | --- | --- |
| smoke, 1 worker | 3 | 0.54 M | 74.8 s | 1 | — |
| smoke, 8 workers | 9 | 1.62 M | 49.7 s | 8 | — |
| screen | 270 | 48.60 M | **1,078.6 s** | 8 | 1,080 s |
| held-out | 36 | 12.96 M | **288.8 s** | 8 | 288 s |
| **total** | **318** | **63.72 M** | **1,491.9 s = 24.9 min** | ≤ 8 | **24.9 min** |

Against the brief's 60-minute cap: **24.9 minutes used, 35.1 left unspent**. Zero trials
skipped at any wall cap. Peak RSS 24 MiB. No background process is running.

Storage: `runs/ecology-v1-calibration/` is **2.9 MiB** against the 50 MiB budget — 318
JSONL rows at ~7 KiB, four stage summaries, two exported configs. `runs/` is
git-ignored, so none of it is committed. One normal `target/` cache; nothing else was
built.

Model usage: the harness exposes none, and this session's is not visible to it. The
measured resource is the table above.

## Reproducing every stage

```bash
cargo build --release -p cubarium-search
cargo test --release -p cubarium-search      # 76 passed
cargo test --release -p cubarium-core        # 467 passed, 2 ignored
./target/release/cubarium-search params
./target/release/cubarium-search calibrate-candidates

# smoke (≤ 5 wall minutes)
./target/release/cubarium-search calibrate --stage smoke-w1 --candidates baseline \
  --seeds 1 --arms 0,1,2 --ticks 180000 --sample-every 600 --introduce-tick 6000 \
  --workers 1 --wall-seconds 300 --out runs/ecology-v1-calibration
./target/release/cubarium-search calibrate --stage smoke-w8 --candidates baseline \
  --seeds 3 --arms 0,1,2 --ticks 180000 --sample-every 600 --introduce-tick 6000 \
  --workers 8 --wall-seconds 300 --out runs/ecology-v1-calibration

# screen: 15 candidates × 6 training seeds × 3 apex arms
./target/release/cubarium-search calibrate --stage screen --candidates all \
  --seed-set training --seeds 6 --arms 0,1,2 --ticks 180000 --sample-every 600 \
  --introduce-tick 6000 --workers 8 --wall-seconds 2100 --out runs/ecology-v1-calibration

# held-out: baseline + two shortlisted × 4 held-out seeds × 3 arms, double horizon
./target/release/cubarium-search calibrate --stage holdout \
  --candidates baseline,fast-leaf,recycle-slow --seed-set holdout --seeds 4 \
  --arms 0,1,2 --ticks 360000 --sample-every 600 --introduce-tick 6000 \
  --workers 8 --wall-seconds 900 --out runs/ecology-v1-calibration

# the exports
./target/release/cubarium-search calibrate-export --candidate fast-leaf --seed 1 \
  --selected --why "..." --out runs/ecology-v1-calibration/selected
./target/release/cubarium-search calibrate-export --candidate baseline --seed 1 \
  --why "..." --out runs/ecology-v1-calibration/selected

# re-derive any row from its own bits
./target/release/cubarium-search replay \
  --record runs/ecology-v1-calibration/screen/evals.jsonl --index 0
```

`replay` reads a calibration row exactly as it reads a search row: it reconstructs the
vector from `param_bits`, checks it against the recorded `param_fingerprint`, re-runs the
evaluation and compares `final_ecology_hash`. Verified on
`runs/ecology-v1-calibration/smoke-w1/evals.jsonl --index 2`: `REPRODUCED`.

## Routine decisions made here, and their visible effect

Per the standing rule, these were made rather than asked about.

| decision | effect |
| --- | --- |
| Thirteen searched names, two more than "roughly 8–12" | `producer.growth` and `producer.mortality` are searchable; without them the vector could not move the critical light, and the screen's central trade-off (3) could not have been found |
| `plant.wood_rate` and `plant.alpha` dropped from the brief's list | their only animal-facing couplings are already covered; recolonisation speed is reported at the default instead of searched |
| Apex profile held at `lanternjaw_trial`, unsearched | an arm difference is the predator's presence, not its design |
| Apex introduced at tick 6,000 in every arm | founders have dispersed first, so an arm difference is not a placement artefact |
| Campaign horizon 180,000 ticks, held-out 360,000 | the late window is exactly one §13 scenario horizon (30 sim min), so a late-window number is comparable with a B-scenario number |
| Depletion measured against each cell's own opening foliage | no per-band baseline run is needed and the threshold is light-proportional by construction; the cost is that it measures loss from the seeded value, not from an equilibrium |
| Probe cadence 20 ticks | a sub-second dip and recovery is not resolved; nothing at these parameters is that fast |
| Shortlist rank 2 skipped for rank 3 | the held-out validated two distinct hypotheses instead of one hypothesis twice |
| One configuration selected, not two | `recycle-slow` is not better than the shipped defaults on any pre-registered measure, so selecting it would overstate the evidence |
| No GA stage | the only available scalar is not evidence for ecology v1, and the discriminating quantity is too rare to fit |

## Stop

The note and the commits are the deliverable. Training (C), presentation (B) and
deployment (D) are separate assignments. Nothing here changed an equation, a §11 value,
an ordering, the snapshot schema, `view.rs`, the trainer, the display, the running
process, `state/`, or port 7393, and no `cubarium-core` file was modified at all.
