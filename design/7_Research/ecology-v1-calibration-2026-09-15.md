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
| Founder animals | `founders.count` 72 — burrower 4, grazer 10, glider 5, skimmer 5 by kind template, `initial_reserve_fraction` 0.6, `initial_energy_fraction` 0.7 |
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
