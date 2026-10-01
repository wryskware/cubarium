---
design_status: leaning
last_reviewed: 2026-09-28
decision_refs: []
---

# Ecology parameters: one registry, searches as files

This follows the audit: `design/7_Research/ecology-parameter-audit-2026-09-28.md` §6 and
the 342 rows of its `parameters.tsv`. It is a proposal. The open choices are at the end.

## Today

- **About 340 numbers.** About 46 can be set from TOML or the command line: world, water
  and weather. Plants and animals have none. Their numbers are struct defaults, `const`s
  and inline literals.
- **The ecology search moves 9 shared multipliers and 3 per-lineage ones.** Each one is a
  `ParamSpec` row plus hand-written code in `live/ecology.rs` (`fauna_config`,
  `flora_config`).
- **Checks and rescaling are kept by hand.** That is ~600 lines of validators and the
  voxel-rescale field list. Every new species (P1, F1) adds more numbers the same way.

## Proposal

### 1. Declare every number once, as a config field

- **Where the numbers live.** Every ecological number becomes a field of the config
  structs the sim already passes around: `FaunaConfig`, `FounderPhysiology`,
  `SpeciesConfig`, `FloraConfig`, `WorldConfig`, and the water and weather configs. The
  `const`s and inline literals move in. Pure numerics, such as solver tolerances and cache
  sizes, stay `const`.
- **One registry table per crate,** with one row per field, keyed by path:
  ```rust
  P { path: "fauna.*.maintenance_per_s", unit: "1/s", range: (1e-4, 1e-1), log: true,
      scale: Scale::None, evolve: true, digest: false, what: "upkeep per unit body" },
  ```
  The rows come straight from `parameters.tsv`: its unit, the range in its `evolve`
  column, and its grade and provenance.
- **A coverage test** serializes the default config. It fails if any numeric field has no
  row, or if any row matches no field. A number cannot be added without being declared.

### 2. Generic code over the table, none per parameter

- **Get and set by path:** serialize to a TOML value, set it, deserialize. This runs only
  at config time.
- **Layers:** code defaults, then preset or world TOML, then the search genome, then
  `--set path=value`. An unknown path is an error.
- **Validation:** ranges from the table, plus a few hand-written cross-field rules.
- **Voxel rescale** by each row's `scale` tag: length, area, volume or none. This replaces
  the hand-kept list.
- **One dump** lists every path with its value, unit and range. It also writes
  `design/backlog.md` §1, and it is what a settings GUI would read.
- **Every run** writes its resolved parameter set next to its output.

### 3. A search is a file

```toml
# config/training/search/browser-movement.toml
[[gene]]
path  = "flora.@photo.foliage_rate"  # a glob or a named group of registry paths
mode  = "scale"                      # multiply the resolved value, or "value"
tie   = "all"                        # one gene for every match, or "each"
range = [0.25, 4.0]                  # optional; the registry's range otherwise
```

- **One generic decode** replaces `PARAMS` and the hand-written
  `fauna_config`/`flora_config`.
- **Today's vector becomes one such file.** Test: decoding it at ×1 gives today's configs.
- **Widening or narrowing a search** is then a file edit.

### 4. Evolve a few meaningful knobs, and derive the rest

Each evaluation is a whole world run, so CMA-ES cannot usefully search 300 raw numbers.
Keep a search to about 20–40 genes. Each gene is a multiplier, where ×1 is a baseline.

- **The main knobs,** per lineage or per plant group:
  - `mass_scale`, the org-units per body (the audit's ×100 question);
  - upkeep;
  - `motor_factor`, walking cost as a multiple of upkeep (audit §8.1);
  - `intake_factor`, maximum intake as a multiple of upkeep;
  - yield;
  - reserve, gestation, maturity and lifespan, from body-size allometry × the bio clock;
  - for plants: growth rate, leaf life, foliage per wood, and defence onset and
    relaxation;
  - decomposition.
- **Derived fields** are computed after layering, by one `derive()` per crate, for
  example `motor_respiration_per_s = motor_factor × maintenance_per_s`. A search can free
  a derived field explicitly.
- **The port changes nothing.** The main knobs default to values that reproduce today's
  numbers. Moving ×1 onto the real-world baselines of audit §6 is a separate, visible step.
- **Soil physics and numerics** are not evolved.

### 5. Screen worlds before simulating them

From the resolved parameters and the founding plan, compute three numbers (audit §1.4,
`scripts/invariants.py`):
- **R:** regrowth in reach ÷ need;
- **S:** standing food in reach ÷ need, per body turnover;
- **F:** the walker's feasibility.

A genome outside R 0.02–0.3, S 0.02–0.2 or F ≥ 10 scores as a failure without a run.
Worlds where parking wins by arithmetic then never cost a simulation.

### 6. Trained controllers

Rows marked `digest` are refused by ecology searches unless the search file says
`retrain = true`. They are body size, eye and mouth anchors, cruise, cone and taste.

## Packages, after P1 and WX3 are on main

- **R1** (opus-high):
  - build the registry types, path get/set, layers, validation, rescale, the coverage
    test and the dump;
  - port the existing fields, and delete the hand-written validators and rescale list;
  - no number changes.
- **R2** (sonnet-high; fauna, flora and world in parallel): move the remaining `const`s
  and literals that `parameters.tsv` marks evolvable into the structs, one registry row
  each.
- **R3** (opus-medium, with the tests written first by an opus-high pass):
  - search files and the generic decode;
  - the main knobs and `derive()`;
  - the R/S/F screen.
- **Size.** About −1,000 to −1,300 lines of logic (audit §7).
- **Audit §8 items 1–2** (walking cost tied to upkeep; the mass-scale test) don't wait
  for this.

## Decisions (2026-09-28)

Wrysk delegated these to the audit thread on 2026-09-28: "go ahead and make them for me,
unless they depend on some pending results".

1. **For the port, ×1 means today's values.** R1–R3 change no numbers. Moving ×1 onto
   real-world baselines is a later step, after the mass-scale test (audit §8.2).
2. **The bio clock `K_bio` arrives in R3 as a main knob, neutral at first.**
   - Time-scaled rows record their real-world value.
   - Each process gets a clock factor, whose default reproduces today's timing.
   - Snapping the processes onto one clock (every factor → 1) waits for the mass-scale
     test and WX3's night rule.
3. **Breeding eligibility (audit §8.5):** in surplus for at least 80 % of the trailing
   900 s, instead of 900 s unbroken.
   - An exponential average with a 900 s time constant is fine.
   - Another thread is already building this (Wrysk, 2026-09-28).

**Waiting on results:**
- **What training rewards (audit §8.3)** waits for the mass-scale test, which another thread
  is running (2026-09-28). Until then keep
  today's score. If walkers win at some mass scale, keep it, with no reward shaping. If
  they don't, switch to net energy change including starvation loss.
- **The night rule (audit §8.7)** waits for WX3 round 3.
