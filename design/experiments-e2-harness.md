---
design_status: leaning
last_reviewed: 2026-09-11
decision_refs: []
---

# E2 harness — local succession versus global crash cycles

Tooling contract for running [E2](experiments.md) (and the E3 timing records)
against the M2 world. The harness is a development tool; it never touches the
ambient display.

## Runner

`scripts/e2-run.sh <batch-name> <matrix.toml>` runs one headless world per
matrix row: `cubarium run --sink none --speed 0 --fresh --seconds <T> --config
<row.toml> --state runs/<batch>/<row>/state --telemetry runs/<batch>/<row>/telemetry.jsonl`,
recording the build id, the row's full config, seeds, and wall time in
`runs/<batch>/<row>/manifest.json`. Rows are generated from `matrix.toml`:
a base config plus named axes, expanded as a full factorial over the listed
values and the listed seeds. `runs/` is git-ignored; a selected summary is
copied into `design/7_Research/` by hand.

The telemetry cadence for experiments is `capacity.telemetry_seconds = 5` and
the field dump cadence `capacity.field_dump_seconds = 60`.

## Analyzer

`scripts/e2-analyze.py runs/<batch>` reads every `telemetry.jsonl` and writes
`runs/<batch>/summary.csv` plus `summary.md` with, per row and seed:

| Metric | Definition |
| --- | --- |
| `pop_min`, `pop_max`, `pop_final`, `pop_cv` | Population over the run after a 10-minute burn-in; CV = std/mean |
| `extinct_at` | First sample with population 0, else empty |
| `time_at_cap` | Fraction of samples with `population ≥ 0.95 · max_organisms` |
| `births_per_hour`, `deaths_per_hour` | From cumulative counters |
| `producer_mean`, `producer_min_frac` | `Σ P` mean and minimum as a fraction of `1280 · P_max` |
| `detritus_mean_frac` | `Σ D / (Σ N + Σ P + Σ D)` mean |
| `occupied_cells_mean` | Mean of `occupied_cells` |
| `face_sync` | Mean pairwise zero-lag correlation of the five per-face population series (detrended by a 30-minute moving average) |
| `face_lag_max` | Largest |lag| (in samples) at which any face pair's cross-correlation peaks, within ±30 minutes |
| `crash_cycles` | Count of population drops of more than 50 % from a local maximum within 30 minutes |
| `residual_max` | Max |mass residual| |

Spatial metrics from `fields.jsonl` (empty when the dump is absent):

| Metric | Definition |
| --- | --- |
| `p_corr_length` | Producer spatial correlation length in cells: over the second half of the window, the mean over samples of the graph distance `k` at which the Pearson correlation between `P` at cells and `P` at their distance-`k` neighbors (BFS over the header adjacency, `k = 1..8`) first falls below `1/e`, linearly interpolated |
| `p_spatial_cv` | Mean over samples of the coefficient of variation of `P` across cells (patchiness) |
| `cells_depleted_frac`, `cells_rich_frac` | Mean fraction of cells with `P ≤ 0.25 · P_max` and with `P ≥ 0.5 · P_max` |
| `depletions_per_cell_hour` | Per-cell transitions from `P ≥ 0.5 · P_max` to `P ≤ 0.25 · P_max`, summed over cells, per simulated hour, per cell |
| `recovery_lag_median` | Median simulated seconds from a depletion (`≤ 0.25 · P_max`) to the next recovery (`≥ 0.5 · P_max`) in the same cell; right-censored lags excluded but counted in `recoveries_censored` |
| `face_p_sync` | Mean pairwise zero-lag correlation of the five per-face `Σ P` series from telemetry, detrended like `face_sync` |

`summary.md` also embeds one ASCII sparkline per row for population and
`Σ P` so raw trajectories are visible without plotting, per the E2 protocol's
"show raw trajectories". No metric is combined into a score.

## First batch (`e2-first`)

Seeds 1–4, `T = 6 h` simulated. Axes over the M2 defaults:

| Axis | Values |
| --- | --- |
| `producer.growth` | 0.004, 0.008, 0.016 |
| `nutrient.diffusion` | 0.02, 0.05, 0.2 |
| `organism.speed_max` | 0.75, 1.5 |
| `weather.moving` | false, true |

That is 36 rows × 4 seeds = 144 runs of 6 simulated hours; at ~200× real time
each run is about two minutes, so the batch is a few hours of one core or under
an hour across several. The gate is stated in `experiments.md`: repeated local
feeding and detritus succession before the whole cube empties, in several
configurations; persistent cap-to-crash synchrony blocks added complexity.

## Second batch (`e2-second`)

After the nutrient-limitation revision. Seeds 1–4, `T = 6 h`, field dumps on.

| Axis | Values |
| --- | --- |
| `producer.growth` | 0.008, 0.016 |
| `nutrient.initial` | 0.35, 0.7 |
| `nutrient.diffusion` | 0.02, 0.2 |
| `organism.speed_max` | 0.75, 1.5 |
| `weather.moving` | false, true |

32 rows × 4 seeds = 128 runs. The question is whether nutrient recycling now
produces local depletion/recovery (nonzero `depletions_per_cell_hour`, a
`p_corr_length` well below the face width, `face_p_sync` near zero) without
whole-cube crashes, and whether diffusion now has a measurable effect.

## Third batch (`e2-third`)

After the saturating-intake revision. Seeds 1–4, `T = 6 h`, field dumps on,
`producer.growth = 0.016`, `nutrient.initial = 0.5`.

| Axis | Values |
| --- | --- |
| `organism.intake_half_saturation` | 0 (linear), 0.45 |
| habitat contrast (`habitat.light_noise_gain`, `habitat.moisture_noise_gain`) | (0.1, 0.2), (0.3, 0.4) |
| `weather.amplitude` | 0.15, 0.3 |
| `organism.sense_radius` | 4, 6 |
| `organism.speed_max` | 0.75, 1.5 |

32 rows × 4 seeds = 128 runs. Paired axes are expanded together (the matrix
file's `[pairs]` table). The question is which of these, alone or together,
first produces cells that recover to half of `P_max` after being grazed, a
producer correlation length of several cells, and repeated depletions per
cell-hour, without synchronized crashes.

## Fourth batch (`e2-fourth`)

Locality. Seeds 1–4, `T = 6 h`, field dumps on, `nutrient.initial = 0.5`.

| Axis | Values |
| --- | --- |
| `producer.growth` | 0.004, 0.008 |
| `organism.speed_max` | 0.3, 0.6, 1.2 |
| `organism.sense_radius` | 3, 6 |
| `drives.turn_noise` | 0.6, 1.5 |

24 rows × 4 seeds = 96 runs. The hypothesis is that consumers must
redistribute more slowly than producers recover for any patch structure to
exist; the metrics to watch are the same as before.

## Confirmation batch (`e2-confirm`)

The adopted defaults over 24 simulated hours, seeds 1–3, with the
static-weather control and the two candidate speeds: `organism.speed_max`
∈ {0.3, 0.4} × `weather.moving` ∈ {false, true}, 12 runs. The gate is the
E2 gate over the longer horizon: no extinction, repeated local depletion and
recovery, and no persistent whole-cube crash synchrony.
