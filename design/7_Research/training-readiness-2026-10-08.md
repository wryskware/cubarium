# Training readiness: diagnosis (Opus), 2026-10-08

Evidence, not decisions (design/7_Research). Written against `main` at e5f214f5. Astra is
answering the same questions separately, and the sections follow the same order as Astra's.
Everything marked "Wrysk's decision" is a question for Wrysk, not something settled here.

## Fact sheet

**Trained brains today: two lineages, both from 2026-10-02.**
- **Which lineages.**
  - Littershredder (`Founder::Blind`) runs `littershredder-curio-c1-2026-10-02.json`.
  - Frondgrazer (`Founder::Browser`) runs `frondgrazer-curio-c1-2026-10-02.json`.
  - Both are embedded with `include_str!` in `crates/cubarium-session/src/founders.rs:13-24`.
  - Provenance: `crates/cubarium/assets/policies/README.md`. They come from curiosity lock-step run C1,
    generation 14, trained on build 6e69e7b2.
- **Where they run.** The live world and `voxel_census` install them by default.
  - The baseline log says "littershredder founders are driven by gru (the built-in centre)".
  - The `heuristic` census argument (CLI: `--founder-heuristic all`) is the control.
- **The other seven lineages have no GRU runtime.** `EpisodeDriver::gru`
  (`crates/cubarium-search/src/es/voxel/controller.rs:454-487`) refuses the lanternjaw,
  capgnawer, loftstrider, ripple-snail, bellwing, seedporter and chorister with the message
  "no GRU policy until contract v3 (P2)".
- **They still load because the digest still matches.** The manifest digest covers the
  shape and the physical anchors (`crates/cubarium-voxel-fauna/src/manifest.rs:1-50`,
  `SCHEMA_VERSION = 2`). It does not cover:
  - the gut and meals (10-03);
  - DarkActive shredders (10-05);
  - plant succession, dry season, senses S1–S3, den rest or roosts.

  So the centres load without complaint into a world they were never trained in.

**Brain contract (voxel; the old `cubarium-core/src/neural/obs.rs` Observation70 / `action.rs` Action7 is the cube-era contract and is not what the voxel brains use).**
- **Network.** GRU, hidden size 32. Two shapes: `GruBlind = GruPolicy<23>` and
  `GruBrowser = GruPolicy<37>` (`controller.rs:118-120`).
- **Inputs.**
  - Self(8): energy, reserve, birth readiness, structural loss, intake, delivered forward,
    delivered turn, motor delivery.
  - Contact(4) plus validity, Wet, Taste(1).
  - Blind only: Chem(detritus).
  - Browser only: Cone(3, foliage/body), 19 channels (`manifest.rs:424-664`).
- **Actions.** Three: forward, turn, feed (`manifest.rs:669-688`). The controller is sampled
  every 0.25 s.
- **What the heuristics have that a brain does not.** The controller trait gives a brain only
  the observation vector (`crates/cubarium-voxel-fauna/src/controller.rs:1-30`).
  - Everything the heuristics perceive since S1/S2 lives in `Percept` (`perception.rs:1-40`),
    "which its heuristics read and nothing else": tracks classed prey/threat/kin with sight,
    hearing and tremor masks, scent gradients, tremor L/R, carcasses, light, loom and beacon,
    and failed-visit marks.
  - Most of what the heuristics do is a reflex layer in the fauna tick, gated on the
    controller opting in (`step.rs:1300-1545`): `flees()`, `pack_tactic()`, `hunt_patrol()`,
    `rests_when_fed()`.
    - Gated on flees: the flee rule and ground response (herd, refuge, freeze), the
      shallows reflex, the snail's roam, thirst and shore laying, the glider's fruit walk and
      roost, and light steering.
    - Gated on the other three: the pack tactic, the lanternjaw patrol and carcass yield, and
      fed perching.
  - **A GRU-driven body gets none of these.** In particular a GRU frondgrazer does not
    flee choristers and gets no shallows reflex.
  - The circadian stage (sleep, den rest) and DROP apply to every driven body. While a body
    is asleep its controller is not consulted.
- **Can today's GRU express each lineage's heuristic?**
  - **Frondgrazer and littershredder:** the foraging part, yes. Fleeing and herding, no: the
    cone has one undifferentiated `body` class and the blind layout has no tremor channel.
    The shallows walk, which is a path query, also no.
  - **The other seven: no.** Their behaviour lives in Percept, path queries and actions the
    3-action contract lacks: lure, lift and glide, climb or roost intent, hunker, and pack
    roles.
  - **What a brain for them needs is contract v3** (`design/predation-and-flight-2026-09-28.md`
    §2.11; S4 "brain encoding of the percept record", `design/7_Research/senses-review-2026-10-06.md`
    step 5):
    - Self + wound + daylight/sleep pressure + stomach fill;
    - per-class (prey/threat/kin) nearest track as bearing, distance and modality;
    - scent level and gradient per channel, tremor L/R, light and loom;
    - lineage actions: lure (lanternjaw), lift (bellwing; `BELLWING_ACTIONS` exists),
      climb/glide/roost (seedporter), hunker (frondgrazer).
  - **Option to put to Wrysk** (B2): keep the safety reflexes as a layer under the brain.

**`cubarium-search` subcommands** (`crates/cubarium-search/src/main.rs:20-1260`).

| Subcommand | What it does | Current? |
|---|---|---|
| `voxel-live-train` | Per-body antithetic ES in persistent worlds; one update per lineage per 10-min window. Default world is the `small` preset with still water, so it needs `--world` | Trainable lineages: Blind and Browser only |
| `voxel-ecology-search` | CMA-ES over a search genome: screens, then fresh founding × seeds, scored by `penalty` (`live/search.rs:281`). `--brain-pairs > 0` makes it the co-training lock step | |
| `voxel-ecology-screen` | Screens a genome (R/S/F, night endurance, roster food) | |
| `voxel-ecology-check` | Held-out arms × seeds; `--finalist` | |
| `voxel-ecology-grid` | Grid of knob scalings × seeds | |
| `voxel-starts` | E0/E1 start libraries | E1 grown on code from 10-03/04 |
| `voxel-train`, `voxel-imitate`, `voxel-evaluate`, `voxel-holdout`, `voxel-landscapes`, `voxel-check` | Arena/landscape episode ES and teacher cloning | Blind and Browser only. `RecordingController` records the forager's own response, not the post-reflex held action, so cloning cannot capture flee/roost |
| `voxel-eval-worker` | Remote evaluation over ssh | |
| `es-*`, `search`, `calibrate*`, `factorial`, `census`, `replay` | Legacy 2D/cube tooling | Not relevant |

**Search files are stale.** The genes in `config/training/search/roster.toml` (40 genes, base
`config/desktop/terrarium.toml`, brains = C1 centres), `cotrain.toml` and the rest were last
edited 10-03. Since then the seedporter diet gained carrion (HUNTER-MEAL), and the roster gene
set lacks `tau_carrion@seedporter`; see the tests below. The screen bands were set on the old
turf, and Fable already made the R band a readout (`roster.toml` `[screen] r_readout`).

**Tests on main.** Run in my own target dir with `cargo nextest`:
- `cubarium-voxel-sim`: 99/99 pass.
- `cubarium-search`: 345 pass, 3 fail. All three are the ones reported on 10-07, and all three
  have one cause: the seedporter's diet now includes carrion.
  - `live::roster_tests::roster_genes_are_the_agreed_set`: the gene `tau_carrion@seedporter`
    is missing from `roster.toml`.
  - `live::roster_tests::roster_decode_touches_exactly_the_listed_paths`: "listed but
    untouched: fauna.founders.seedporter.gut.digestion_s.carrion". This is the same missing gene.
  - `live::gut_screen_tests::vine_fruit_feeds_at_the_yield_its_mineral_funds` (`gut_screen_tests.rs:219`):
    the test reads `foods[0]` as fruit, but `foods[0]` is now carrion.

**Score and reward code today.**
- **Brain window score.** `NetEnergy` in `live/window.rs:5-10, 81-205`:
  `(E_end − E_start + E_given)/body_ref + weight × novelty`.
  - `E` = tissue + reserve + gestation escrow.
  - `E_end = 0` on death.
  - Curiosity is in `live/curiosity.rs:1-15, 142-150`, with weights in
    `config/training/curiosity.toml` (frondgrazer 81, littershredder 27, lanternjaw 0;
    calibrated on novelty sd ≈ 0.02–0.04 on 10-02).
- **Ecology objective.** `penalty()` in `live/search.rs:281-345`. It sums:
  - band, peak, lively (browser moving share ≥ `moving_min`), lanternjaw share;
  - `missing_cost × (lost lineages + lost plant species + lineages silent in the second half)`;
  - the v3 stage ladders (`live/targets.rs:1-30`: birth-required, lanternjaw progress,
    chorister pups, replenishment ≥ 1 per day-cycle);
  - balance.

  `counts_readout` makes the population band a readout only.

**Live baseline on eidolon (read at 07:45, minutes 1–12 of 120).** 16 runs: d72/d96 ×
shipped/heuristic × seeds 1–4, 1 thread each, 16 concurrent on 24 threads.
- **Speed.**
  - d72: 5.2–6.5× real time (ms/tick 7–12), so a 2 h world takes about 20–24 min wall.
  - d96: 2.8–4.8× (ms/tick 10–19), about 25–43 min.
  - Founding takes 112 s on d72.
  - The water phase logs about 8 of 10 ms per tick (`phases: water=8.1`; overlap=1, so the
    share is unverified).
- **Founding massacre in every run, both arms.** The four Veilhounds kill 1–8 of the 46
  founders in sim-minute 1. In d72 seed 1 they struck 15 times (heuristic arm) and 30 times
  (shipped arm).
  - Example, d72-shipped-s1: 2 loftstriders, 3 bellwings, 2 seedporters and 1 frondgrazer
    killed (`kills_chorister_*`).
  - Loftstriders reach 0 in 4 of 14 runs by minute 4.
  - Nothing more dies through minute 10.
  - Likely cause: founders start with `Stomach::default()` (empty; `fauna/src/lib.rs:4830`).
    The eating latch then goes on at the first meal phase (`step.rs:777-787`), and a hungry
    pack hunts naive prey placed nearby.
- **Movement.** At the minute-3 check, shipped frondgrazers moved 69–157 m against
  374–619 m on the heuristic: about 4× less.
- **The liveness check cannot see three lineages.** Shredders, capgnawers and snails are
  DarkActive and asleep at the 08:00 start (1–17 % awake), so the check passes without
  showing they work.
- **A seed fell back silently.** d96 seed 3 was refused as a habitat (bellwing placed 4 of
  6) and fell back to random seed 5866944632500246677. The run's "s3" label no longer
  names its world.

## 1. Verdict

1. Brain training can resume this week, but only for frondgrazer and littershredder. The other
   seven lineages need contract v3 (Percept encoding plus extra actions), which is a design
   package awaiting Wrysk's review.
2. The tooling builds and is nearly green. The 3 failing tests share one cause (the
   seedporter's carrion diet against the stale `roster.toml`), about 1 h to fix.
3. The two biggest confounds are the founding: a Veilhound massacre in minute 1, and dark
   lineages asleep for the first 60 min. Both are harness or founding issues, not ecology.
   Fix them, and branch from saved starts, before any fitness number means anything.
4. Fitness: the parts exist (`penalty`, targets v3, `NetEnergy`). What is missing is one
   scorer that reads census CSVs per lineage and per seed, plus Wrysk's weights for the
   landscape and hunt terms.
5. Spare compute today: run the baseline to completion, build saved starts, and run an
   overnight warm-start retrain of the two centres on d72. Defer EC2 until a 1-generation
   smoke passes on the fixed founding.

## 2. Blockers, ranked

**B-1 (high): the founding transient corrupts every run.**
- The Veilhounds kill 1–8 founders in minute 1. Evidence: baseline CSVs `kills_chorister_*`
  at m1; `healed/killed` columns.
- Lineages with few founders (loftstrider 2, seedporter 4) are lost by founding, not by
  ecology. That fails "every lineage persists" at random.
- The cause is very likely the empty stomach at founding (`lib.rs:4830`, `step.rs:777-787`).
- Fix: a founding stomach knob (`founding.arrival_stomach`, a share of capacity, registry
  row), or a predator founding grace, or a prey-free radius around dens. Then add a test that
  founders are not hungry at t=0. About 1–2 h. Confirm the cause first (§8).

**B-2 (high): GRU bodies run without the reflex layer, so the arms are not comparable and the brains learn without safety reflexes.**
- Evidence: the `flees()` gating at `step.rs:1334-1535`. The `controller.rs:104` doc says "a
  trained policy's behaviour changes only through the world".
- A GRU frondgrazer neither flees choristers or lanternjaws nor walks to the shallows. In
  today's world, with Veilhound packs, that is a different animal from the one trained on
  10-02, when no packs were founded in training.
- Fix: either a design call (B2: keep the reflexes under brains, about 2 h) or contract v3
  (days).

**B-3 (high): the shipped centres were trained against a world that no longer exists.**
- That world had no gut and meals, diurnal shredders, no packs and the old turf. The digest
  cannot notice, because those are world changes, not contract changes.
- The baseline shows GRU frondgrazers moving about 4× less than the heuristic at minute 3.
- Fix: a warm-start retrain on d72 with the current ecology frozen. Overnight on eidolon after
  B-1, B-4 and B-6.

**B-4 (medium): search files and tests are out of step with current diets.**
- Evidence: the 3 failing tests above.
- Fix: add `tau_carrion@seedporter` to `roster.toml` and `roster-pilot.toml`, and make the
  gut-screen test find fruit by name rather than by index. Also add a generic check that every
  search file's genes decode and cover every food in each lineage's current diet; that one
  already exists for roster.toml, so extend it to all files. About 1 h.

**B-5 (medium): no single scorer.**
- `voxel_census` writes about 450 CSV columns per minute, and the search's `penalty` reads its
  own trace, not those CSVs. No tool turns a census batch into the fitness vector and a paired
  comparison.
- Fix: a scorer (§3). About 2–3 h.

**B-6 (medium): clock mismatches.** Times below are world time; one world day is 2 h.
- DarkActive lineages are asleep from 08:00 to about 20:00, which is minutes 0–60. Their
  active window in a 2 h run is about minutes 60–100.
- The Veilhound litters once a year (10 h).
- The lanternjaw needs about 5–7 h from egg to adult.
- A 2 h run from fresh founding therefore spends half its time on founders' transients.
- Fix: saved starts aged past founding (60–120 min), and for slow clocks starts saved inside
  the relevant phase: the Veilhound at year phase 0.2, the lanternjaw with free juveniles.
  The tools exist: `save=`/`load=`, `voxel-starts`.

**B-7 (low–medium): imitation cannot clone the full heuristic.**
- `RecordingController` (`controller.rs:214-269`) records the forager's response before the
  reflexes. Teacher cloning for v3 lineages must record the held action after overrides,
  together with the `Steer` source (`fauna/src/lib.rs:3744-3779`).

**B-8 (low): seed fallback breaks seed labels and common random numbers.**
- A refused habitat seed falls back to a random seed. The arms stay matched (both used the
  same fallback), but seed sets across maps differ.
- Fix: deterministic fallback (seed + k) and report it in the CSV header. About 0.5 h.

**B-9 (low): throughput.**
- Water may be about 80 % of tick time at `water_every 4`.
- For brain-only training on flat zones, a coarser water would roughly double throughput.
  That is Wrysk's call (fidelity per mode).

## 3. Fitness score proposal

### (a) Brain reward per lineage (per body, per window)

Keep the current form; it is what Wrysk set on 10-01 and 10-02.

`r = [E_end − E_start + E_given] / body_ref + w_L × novelty`, in body references.

- `E_end = 0` on death, whatever the cause. A death is the loss of everything the body holds,
  which already prices in predation, drowning and starving.
- Readouts: `live/window.rs` `EnergyMeter`, `BodyMeter`; curiosity `live/curiosity.rs`.

Routine changes:
1. **Score only awake time.** A sleeping body's controller is not consulted, so sleep-window
   variance is pure noise for both twins. Readout: `State::Asleep`, which exists. This is a
   routine call.
2. **Recalibrate `w_L`** with the existing rule `w × sd(novelty) = share × sd(E-term)` on
   d72 with the current ecology. The shares are frondgrazer 0.5 and littershredder 0.3, as on
   10-02. One smoke run, about 1 h.
3. **Pool scores across 8 worlds per generation**, rank-normalised within a world (centred
   ranks). That removes world-level offsets, the noise the 10-01 run saw as the "window
   barrier waits for slowest world".

Wrysk's decisions:
- whether `E_given` should extend to offspring surviving one window (an inclusive-fitness
  term, so brains do not breed into a famine);
- whether curiosity stays once v3 brains can see food at range.

### (b) Ecology whole-world objective (per world; minimise)

Over 8 common-random-number seeds, score each world with the existing `penalty()` terms, plus
three that are missing:

```
J_world = M · (L_lost + P_lost + L_silent + Σ stage_shortfall)      # persistence / breeding (hard-ish), exists
        + Σ_L  band_L                                                # per-lineage band, NEW (see below)
        + w_move · max(0, 0.30 − moving_browser)/0.30                # exists (lively)
        + w_lj  · outside(LJ/animals, 0.05, 0.15)                    # exists (share)
        + w_rep · Σ_L max(0, 1 − R̂_L)                                # replenishment ≥ 1/day-cycle, exists (v3)
        + w_land· [max(0, m* − meadow_patch_share) / m*  +  [grove_patch_max(lanternberry) < g*]  +  [vaulttree recruits = 0]]   # NEW, readouts exist in census CSV
        + w_hunt· outside(kills per pack-hour, h_lo, h_hi)           # NEW, readout exists (kills_chorister_*, strikes/hits)
```

The new terms in more detail:
- `band_L = mean over the second half of |log(n_L / n_L,0)| beyond log 3`, in log units.
  - It replaces the single total-animals band. That band let one booming lineage hide
    another's collapse.
  - Readouts: census `founders_*` / `eggs_*` per lineage.
- `meadow_patch_share`, `grove_patch_*` and `recruits_vaulttree` are census CSV columns already.
- Kills are `kills_<hunter>_<prey>`. Hunts per hour come from `strikes_*`, `hits_*`.

Noise and aggregation:
- Report the per-seed vector.
- The candidate score is the mean `J` over seeds plus 0.5 × sd. A world that lost a lineage
  counts in full, never trimmed.
- Compare a change by the paired difference to the baseline on the same seeds. Call it a win
  only if the sign holds on at least 6 of 8 seeds.

Weights: keep the current values (`missing_cost`, `lively_weight` 10, `replenish_weight`);
`w_land` and `w_hunt` are 1 until Wrysk sets them.

Wrysk's decisions:
- `m*` (meadow continuous-patch share);
- `g*` (lanternberry grove size);
- `h_lo`/`h_hi` ("rare, watchable hunts": proposal 0.1–0.5 kills per pack-hour);
- whether the band is per lineage (recommended) or total;
- whether the landscape terms are hard (in `M`) or soft.

## 4. Run protocol

**Inner loop: one change answered in under 30 min.**
1. **Check t = 0 first**: screens, budget arithmetic, a predicted number.
2. **Load 8 saved d72 starts** (seeds 1–8, saved at minute 60 after the B-1 fix, past founding
   and at dusk). Use `voxel_census 1.5 load=<start> config=... threads=1`, 1.5 h beside the
   baseline arm from the same save.
3. **Run on eidolon**: 16 concurrent 1-thread runs through `run-study.py --slot`.
   - 8 seeds × 2 arms finish in about 17 min wall at 5.5× (no founding cost).
   - Staged: `check_min=3`, `segment_min=15`.
4. **Liveness at minute 3** after load. Shredders are awake at dusk, so the check sees them.
5. **Early stop** a seed once a hard term is decided: a lineage lost, or a boom over 5×.
6. **Score** with the B-5 scorer: per-seed `J` vector and the paired sign test.

Desktop, 32 threads shared: at most 8–12 concurrent runs.

**Overnight training loop (eidolon, about 10 h).**
- Run `voxel-ecology-search` in unified mode with the ecology frozen (brains only), or
  `voxel-live-train --world config/desktop/terrarium-d72.toml`.
  - Centres: C1 warm start, σ 0.05.
  - 8 worlds × 2 h, 10-min windows, brain-pairs 16, `water_every 4`, checkpoint every
    generation.
- **Gate.** Every 3 generations, an automatic held-out census (seeds 9–16 × 2 h from saved
  starts, centre against heuristic) appends to a TSV.
- **Stop** if the centre's held-out alive or births fall below the heuristic for 2
  consecutive gates.
- **Throughput.** About 22 worlds-2h per hour on eidolon at 16 concurrent, which is about
  1 generation per 25–30 min, so roughly 20 generations overnight.

## 5. Maps

**What d72 and d96 are.** Both are the desk `terrarium` generator (32 m ring, massif 6 m deep,
`Terrarium::DESK`, `crates/cubarium-voxel/src/terrarium.rs:441-470`) with depth 72 or 96.
The extra depth becomes a foreground lowland:

| | Added depth | Plain (share 0.3) | Rolling hills |
|---|---|---|---|
| d72 | 3 m | 0.9 m | 0.375 m relief, 5 m wavelength |
| d96 | 6 m | 1.8 m | same |

- So d96 already has the largest flat, connected zone the generator makes.
- d96 costs about 1.5× d72 per tick and misses the 30-min target at 1 thread.

**What the generator supports today, with no code** (all of these are `[world.landform]` keys,
as `config/scenarios/range.toml` shows):
- `lowland_plain_share`, `lowland_relief_m`, `lowland_rise_m`, `massif_depth_m`;
- `shelves`, `arches`, `spires` (ranges);
- `lake_m`, `ground_fall`, `ring_m`, `width`.

Other generators: `terrarium-small` (the Tachyon `patch` map) and the staged `small`,
`default` and `wide` recipes (`recipe.rs:1590-1615`).

**Proposed training-map repertoire** (TOML only; about 1 h to write and check 8 seeds each):
- **`train-flat`**: d96 with `lowland_plain_share 0.7`, `lowland_relief_m 0.125`,
  shelves/arches `[1,1]`, spires `[1,1]`. A wide, consistent, connected front plain, for brain
  movement and foraging and to rule out stuck or isolated bodies.
- **`d72`**: the ecology default; matches the display world closely.
- **`range`** (64 m, exists): Veilhound packs and lanternjaws.
- **`patch`** (exists): plants and grazing.
- **A connectivity readout**: the largest walkable component's share of land per lineage at
  founding. The founding already computes components (`habitat.rs`). Report it in the CSV
  header so stuck or isolated bodies can be ruled out by number.

## 6. Work lists

**A: autonomous, next 8–12 h, ranked by value per hour.**

| # | Item | Size | Lane / machine |
|---|---|---|---|
| 1 | Fix B-4: roster genes, the gut-screen test, and a decode/diet-coverage check over every search file | 1 h | Codex medium, desktop |
| 2 | Confirm the B-1 cause (§8 measurement), then add a founding stomach knob + registry row + mechanism test (founders' latch off at t=0) | 0.5 + 1.5 h | Opus medium, desktop. State the visible effect: no minute-1 kills |
| 3 | B-5 scorer: census CSV batch → per-seed `J` terms + paired table (Python or Rust example, no tuned numbers in tests) | 2–3 h | Opus medium, desktop |
| 4 | Let the baseline finish (DONE marker); score it with item 3: shipped vs heuristic per lineage (alive, births, moving, drowned, killed) | 1 h analysis | Sonnet medium, after DONE |
| 5 | Saved-start library: d72 and train-flat × seeds 1–16, founded on the item-2 build and saved at minutes 60 and 120 (detached, staged) | 1 h worker + about 1 h machine | Codex medium, eidolon |
| 6 | Deterministic seed fallback + record it (B-8) | 0.5 h | Codex low/medium |
| 7 | Curiosity recalibration smoke on d72 (1 h sim, 2 seeds) | 1 h | eidolon, detached |
| 8 | Overnight warm-start retrain of both centres on d72, ecology frozen, with the held-out gate every 3 generations (§4). Only after items 1, 2, 5 and 7 | 1 h set-up + about 8 h machine | Opus medium set-up, eidolon |
| 9 | Water-cost measurement: one 10-min run at `water_every` 4, 8 and 16 with threads=1, ms/tick breakdown. Measurement only; no default change | 0.5 h | desktop |
| 10 | Draft contract v3 (S4): Percept encoding + per-lineage actions + reflex-layer option, for Wrysk's review. Doc only | 3 h | Opus high |

Low-hanging fruit: items 1, 2, 6 and 9.

**B: Wrysk's decisions.**
1. **Fitness weights and thresholds:** `m*`, `g*`, the hunt band, a per-lineage band, and
   whether landscape terms are hard. Recommendation: per-lineage band, landscape soft at
   weight 1, hunts 0.1–0.5 kills per pack-hour.
2. **Reflexes under brains.**
   - Options: (a) brains under the safety reflex layer (flee, shallows, drop, roost), with
     brains learning foraging and movement; (b) brains learn everything (requires v3 before
     any predator-world training).
   - Recommendation: (a) now, then (b) per lineage as v3 lands.
3. **Contract v3 scope and order of lineages.** Recommendation: v3 for prey first (threat,
   kin and tremor channels plus hunker), then lanternjaw (lure), then chorister. Bellwing and
   seedporter last (flight and glide actions).
4. **Founding fix.** If he treats founders' stomach state as ecology rather than harness:
   full stomach (recommended), predator grace, or a prey-free den radius.
5. **Fidelity for brain-only training:** `water_every` 8–16, or frozen water, on train-flat?
   Recommendation: allow it for brain-only training, keep 4 for ecology.
6. **Maps:** d72 as the ecology default and train-flat for brain training (recommended), or
   d96 for both?
7. **EC2 go/no-go** for a CMA run with 16–32 candidates × 8 seeds (§7).

## 7. EC2 plan

**Shape.** Two spot c7g.16xlarge, the 128-vCPU quota (`docs/graviton-training.md`), running
`voxel-ecology-search`:
- 16 candidates × 8 seeds, 2 h worlds, 1 thread each;
- about 128 worlds in parallel;
- 1 generation per about 25 min, so about 25 generations in 12 h.

**Cost.**
- Spot is about $0.64–0.75 per host-hour, so about $18 for 12 h.
- On-demand is $2.32 per host-hour, about $56.
- A forgotten host costs $17 a day on spot.
- An aarch64 build of main is needed (`target/aarch64-graviton2` exists; rebuild).

**Preconditions.**
1. Items A1, A2, A3 and A5 done.
2. A 1-generation smoke on eidolon passes and the scorer reads its output.
3. The fitness thresholds in B1 are set.
4. Per-generation checkpoints, with spot interruption tested.
5. A seeds-parallel layout. The 10-01 run got only 15 % from two hosts because the window
   barrier waits for the slowest world. Scale candidates and seeds, not window length.

Launch only on Wrysk's go, and always run `down`.

## 8. Uncertainties and the measurement that resolves each

| Uncertainty | Measurement |
|---|---|
| The founding-massacre cause: empty stomach versus pack placement | One d72 seed, 2 sim-min, with founders' stomach set full (or the eating latch logged per chorister at ticks 0–1200), counting `kills_chorister_*` |
| Shipped brains against heuristic in today's world | The finished baseline: per-lineage alive, births in the second half, moving share and drowned, 4 seeds × 2 maps, paired |
| Whether GRU frondgrazers suffer from the missing flee and shallows reflexes | Baseline `killed_frondgrazer` and `drowned_frondgrazer`, shipped against heuristic, minutes 60–120 |
| Water's share of tick time at 1 thread | `phases:` line at `water_every` 4/8/16 with overlap off, 10 sim-min |
| Whether a 2 h horizon from a 60-min save reaches a verdict for DarkActive lineages | One 8-seed batch from saves, `asleep_*` and `births` in minutes 0–60 after load |
| Whether curiosity weights are still sane | sd(novelty) and sd(E) per lineage from one unified smoke on d72 |
