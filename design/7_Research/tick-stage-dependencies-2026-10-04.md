---
design_status: exploration
last_reviewed: 2026-10-04
decision_refs: []
---

# Which stages of a tick can run at the same time

Wrysk asked that stages overlap wherever they don't depend on each other, with plant growth vs
animals eating as the example. This note reads the code on main at `0f303256` (the schedule in
`crates/cubarium-voxel-sim/src/lib.rs`, `run` in `crates/cubarium-voxel-flora/src/step.rs`,
`tick_inner` in `crates/cubarium-voxel-fauna/src/step.rs`). It is evidence, not a decision.

**Timing.**
- **Run:** `phase_profile prof` on eidolon (Ryzen 9900X, otherwise idle). Binary: main plus
  FAUNA-PAR's first timers (`~/fauna-par/pp-before`). Start: the terrarium scenario, seed 1,
  minute 60 (98 animals, about 5k stands). 2 minutes after 1 minute of warm-up, 8 threads,
  `step_every = 5`.
- **Results:**

  | Tick | `water_every = 1` | `water_every = 4` |
  |---|---|---|
  | overlapped | **3.43 ms** | **2.90 ms** |
  | chained | not run | 3.28 ms |

- **Raw CSVs:** `<scratchpad>/stage-overlap/`.
- **Other costs:** costs not in that run come from the 2026-10-04 profile (s5 minute-120 save).

## 1. Phases: what each one reads and writes, and its cost

Costs are ms per occurrence at 8 threads, overlapped tick, from the eidolon run:
- per tick for fauna and the every-tick water;
- per due tick for the coarse water at `water_every = 4`;
- per plant step for flora.

| Phase | Reads | Writes | ms |
|---|---|---|---|
| **Water** (the water leg, live world) | | | |
| read copy (Begin) / copy-back | live `free`, `pore` (swapped) / the read copy | the read copy / live `free`, `pore` | ~0 / 0.18 |
| rain, evaporate | `weather`, `atmosphere_m3`, sky tops, `free` | `free`, `atmosphere_m3`, `ledger` | 0.03 |
| infiltrate + fall (×4 substeps) | `free`, `pore`, `material`, per column | `free`, `pore`, `wet`/`damp` sets | 0.20 |
| exchange (×4) | `free`, `pore`, neighbours' heads | `free`, `pore` (atomic adds) | 1.90 (due tick) |
| drain | `pore`, the water table | `pore`, `aquifer_m3` | 0.15 (due tick) |
| water_table | `aquifer_m3`, bottom `pore` | `pore`, `aquifer_m3` (scarce stock in index order) | 0.23 (due tick) |
| spring, outlet | one cell each, `aquifer_m3` | `free`, `ledger` | ~0 |
| withdraw (overlapped only; serial, ascending cells) | the drink plan, live `pore` | `pore`, `damp`/`drainable`, world `ledger` | 0.64 (plant step) |
| **Flora** (`run`, a plant step every 5 ticks) | | | **3.21 total** |
| decay_defences | `stands[].defence` | `stands[].defence` | ~0 |
| prune | `material`, `stands`, `ground` | removes stands and ground sites, `ledger` | 0.06 |
| sky cache, cover prep | `material`, `cover` | `sky`, `cover` (lights, leaf totals), `ledger` | ≈ 0.25 (half of "Cover") |
| drown | the read copy's water depth, `stands` | removes stands, `ground`, `ledger`, `death_log` | 0.05 |
| light | `stands[]` layers and foliage, `sky` | `sky` (new entries) and a per-stand light vector (local) | 0.41 |
| drink | `stands[]` roots, `cover`, the read copy's `pore` | overlapped: the plan only (`flora.drink`); chained: `pore`, `transpired`, `cover` | 1.02 |
| feed | `ground` pools, moisture | `ground`, `stands[]`, `substrate_uptake` | 0.40 |
| grow | light, moisture, substrate, climate | `stands[]` (foliage, wood, parcels), `ground` (litterfall), `ledger`, `death_log` | 0.24 |
| cover grow | `cover`, the world, crowns | `cover`, `ground`, `ledger` | ≈ 0.25 |
| decompose (+ film) | `pre` (ground as the step began) | `ground`, `ledger` | 0.27 |
| seed bank | `bank_wheel`, ground banks, the world | new stands, ground banks, `ledger` | 0.03 |
| propagate | `stands[]` parcels, `cover`, the world | new stands (runners), ground banks, `deliveries`, `ledger` | 0.15 |
| crowns | `stands` | `crowns` (crown cache and reach index) | 0.08 |
| finish_drink (barrier) | the plan's receipts | `stands[]` and `cover` shortfalls, `ledger` | small |
| **Fauna** (`tick_inner`, every tick, reads the world only) | | | **2.08 total** |
| terrain | `material` | `animals[].site` | ~0 |
| maintenance (circadian, upkeep, gut, eggs, packs, `void_guts`) | `animals`, the world view | `animals[]` (reserve, gut, state), fauna `ledger`; **flora `ground`** (dung, gut seeds) | 0.01 |
| sense field (every 10 ticks) | world view, flora `ground` litter, `stands` | `Senses` litter field | 0.07 |
| controllers | `animals`, world view, flora view (`stands`, `crowns`, `ground`, `cover`), `Senses` | `animals[]` held actions and interval feedback; the cone and predation windows (scratch) | 1.86 (cone window 0.67, predation window 0.80, observe 0.35) |
| act | held actions, world view, flora (stock at the mouth, perches, fruit) | `animals[]` (pose, gut, energy); **flora** through `take_*` (`stands[].layer_stock`, `foliage`, `defence`; `ground` litter, carrion, film; `cover` nectar, fruit; spill deposits); hits | 0.12 |
| births | `animals`, the world | `animals` (push), `ledger` | 0.01 |
| deaths | `animals` | `animals` (remove); **flora `ground`** (`lay_carcass`, deposits) | ~0 |
| burrs | `animals` poses, flora `burrs` | flora `burrs`, `bank_wheel`, `ground`, `deliveries`, `ledger` | ~0 |

## 2. What runs at the same time today, and the critical path

- **Chained tick:** one phase at a time. Each phase uses threads internally: water by column,
  flora per stand. Fauna is serial apart from its small field update.
- **Overlapped tick** (the display and the studies):
  1. `Begin` makes the read copy.
  2. The water leg runs beside the bio leg (`Flora → Fauna`), which reads the read copy.
  3. `withdraw` waits for water and the whole of Flora, then runs beside Fauna.
  4. `finish_drink` runs at the barrier.

So: `tick = max(Flora + Fauna, max(Water, Flora) + Withdraw) + ~0.1 fixed`.

**Today, 8 threads, `water_every = 4`.** The 20-tick cycle holds 4 plant ticks, 4 water ticks
and 12 plain ticks:

| Tick | Water | Bio leg | Tick ms |
|---|---|---|---|
| plain | 0.4 | Fauna 2.08 | **2.08** |
| water due | 2.65 | 2.08 | **2.65** |
| plant | — | Flora 3.2 + Fauna 2.08 | **5.3** |
| mean | | | **2.84** derived (2.90 measured) |

Plant ticks are about 37% of the time.

At `water_every = 1`:
- water binds the plain ticks at 2.59;
- the plant tick is 3.4 + 2.25 = 5.65;
- the mean is 3.2 derived, 3.43 measured.

**The critical path is:**
- the serial controller stage on every tick: 1.9 ms, 1.5 of it window builds;
- plus the 3.2 ms plant step in series with it on one tick in 5.

The overlap that exists is already worth 12% (3.28 → 2.90).

- At 1 thread nothing overlaps.
- The best-throughput mode (16 runs × 1 thread) gains nothing from anything below.
- These proposals only shorten a single run with about 4 or more threads.

**After FAUNA-PAR** (fauna taken as `f` = 0.7–1.0 ms at 8 threads; FAUNA-PAR measures the real
figure), at `water_every = 4`:
- plain ticks become `f`;
- water ticks stay 2.65;
- plant ticks become 3.2 + `f` = 3.9–4.2. The withdrawal path is 3.84, a near tie.
- The mean is **1.73–1.97 ms**, and plant ticks are then about **45%** of the time.

The plant step becomes the main spike. The 2.65 ms water tick, with a bio leg of only `f`, is the
other.

## 3. The real ordering constraints

**True read-after-write conflicts:**
- **Water:** each phase moves the stocks the next one reads. Rain → infiltrate → fall → exchange
  → drain → water_table → spring → outlet, all on `free`, `pore` and the aquifer.
- **Flora:**
  - the drink comes before feed and grow (moisture), and light before grow;
  - grow comes before propagate (parcels) and before cover grow (the crowns vines contest);
  - the crowns are refreshed after everything that adds or removes a stand.
- **Fauna:** controllers come before act (held actions), act before births and deaths (reserve,
  wounds), and deaths before burrs (seeds drop where a body died).
- **Structure:** `Begin` (the read copy) comes before both legs, and `Advance` comes last.
- The plant clock's `advance` clears `deliveries` and `death_log`. It must come before anything
  in the tick that books one.

**Orders that are only convention.** Each could read an older value, or be buffered into a
commit step:
- **Flora before Fauna in the bio leg.** On 4 ticks in 5 the animals already read the previous
  plant step's flora. Reading it on the plant tick too is a lag of at most 0.25 s.
- **Withdraw after the whole plant step.** It needs only the plan, which is complete once the
  drink has run. It doesn't need feed through crowns. The shortfall booking (`finish_drink`) can
  stay at the barrier.
- **The plant clock's phase relative to the water clock.** Both are counters, and nothing orders
  them.
- **Grazing vs growth (Wrysk's example).**
  - Today a bite is `take_*` on the live flora inside act. It returns `Taken` at once, in animal
    id order, so a second animal at the same stand sees what the first one left.
  - It could instead be a **request** (animal, source, food, want, layers) settled at one point,
    as the drink plan is: clamped per source pro rata, shortfall to the asker, then swallowed.
  - Conservation holds by construction: the settle uses the same `withdraw_foliage` and
    `take_pool` rules.
  - Deposits, carcasses and burrs buffer the same way. They only add.
- **`void_guts` before the controllers.** The controllers see litter only through the sense
  field, which is rebuilt every 10 ticks.
- **Drain and water_table share the aquifer.** The table could read the previous tick's stock.
  This is 0.2 ms, off the path. Leave it.

## 4. Proposals, ranked by critical-path ms saved per effort

All figures are at 8 threads and `water_every = 4`, against the post-FAUNA-PAR mean of 1.73–1.97
ms. They are derived from the measured phases, so measure each once it is built.

**1. Put the plant step on the coarse-water tick.** Effort: small. Saving: −0.17 to −0.23
ms/tick (−9 to −13%); with #2 as well, −0.36 to −0.38 (−19 to −21%).

- Run the plant step on the ticks where `world.tick % water_every == 0`, with `step_every =
  water_every`. Today it runs off its own `since_step` counter.
- The long water leg (2.65) then hides under the plant tick's bio leg (3.9–4.2), instead of
  costing a separate tick that the animals barely fill.
- **Cadence choice for Wrysk:**
  - `step_every = 4`: plants step 25% more often, a cost already counted in the saving. No
    ecological change beyond a finer clock (0.2 s steps).
  - Or keep 5 and make the experiments' `water_every = 5`: −0.33 to −0.39 ms/tick, but lateral
    water is coarsened 5× instead of 4×.
- The plant step's dt stays exact: `since_step` still counts the ticks it covers.
- **Today, before FAUNA-PAR, this gains nothing:** fauna (2.08) fills the water tick anyway.

**2. Plant reads beside animal reads on the plant tick, with the withdrawal handed out at the
drink.** Effort: medium. Saving: −0.11 to −0.17 ms/tick (−6 to −8%); before FAUNA-PAR, −0.29
(−10%).

- The plant tick becomes:
  1. flora `advance`, prune, sky, cover prep, drown;
  2. fauna terrain and maintenance;
  3. **{flora light + drink plan} ‖ {fauna field + controllers}**;
  4. flora feed → grow → cover → decompose → seeds → propagate → crowns;
  5. fauna act, births, deaths, burrs.
- The drink plan goes to `withdraw` as soon as it exists, so the withdrawal runs beside steps 4
  and 5.
- **No copy and no buffering.**
  - Light and the planned drink write only `sky` and the drink scratch. Fauna never reads those.
  - The controllers read `stands`, `ground`, `cover` and `crowns`, which nothing writes during
    step 3.
  - This needs a split borrow of `Flora`: a shared view plus `&mut` of those two fields.
- **Saving on the plant tick:** 3.9–4.2 → about 3.4 ms. The withdrawal path is then at most
  2.65 + 0.64 = 3.3.
- Without the early withdrawal, the old withdrawal path (3.84) binds, so the two parts ship
  together.
- **Ecological effect:**
  - the controllers see foliage and crowns from before this step's growth (≤ 0.25 s old), as on
    every other tick;
  - this tick's dung is on the ground before the step's feed, so fungi can take it one step
    sooner.
- Conservation is untouched: every write stays where it is.

**3. Bite requests, so the whole fauna step runs beside the whole plant step** (Wrysk's example).
Effort: large. Saving: about −0.05 ms/tick beyond #2.

- Act writes requests. One settle point after both legs clamps them pro rata per source and
  swallows. Deposits, carcasses and burrs are appended there too.
- **Act's reads must stop seeing the plant step's writes.** Act reads the stock at the mouth,
  the perches and the fruit. That needs a flora read copy (stands, ground, cover, crowns) after
  each plant step, a few MB at est. 0.3–1 ms, or #2's ordering extended to act.
- **Why it buys so little:**
  - the plant step (3.2) is already longer than the post-FAUNA-PAR fauna step, so the plant tick
    is bound by flora either way;
  - the animals' cost is in reading (window builds and observation), not eating (act 0.12).
- What it would buy is fairness: animals at one stand in one tick share it pro rata instead of
  first come by id. It would also allow a parallel act.
- **Ecological effect:**
  - bite size is judged on stock up to one plant step old;
  - the swallow still lands in the same tick, before births and deaths.
- **Verdict:** feasible and conserving, but not worth it for speed.

**4. Smaller and superseded:**
- **Parallel withdrawal:** cells are independent, so the batch can be folded in chunk order.
  After #1–#2 it is off the path, so it saves about 0.
- **Spreading the plant step over its 5-tick window:** this only helps where it lands in slack,
  and after #1 the only slack (the water tick) is already used.
- **Running water one tick ahead** (two read copies): a large change, the drink lags 2 ticks, and
  it needs ~27 MB more per world. It does the same job as #1 at `water_every = 1`, where the
  display doesn't need the speed.

**What is left after these:** the plant step itself (3.2 ms at 8 threads). Cover, decompose,
propagate and crowns barely shrink with threads, about 1 ms per step together. That is
parallelism inside a phase, not overlap, and it is the next lever.

## 5. Fit with FAUNA-PAR

FAUNA-PAR (`.claude/worktrees/fauna-par`) parallelises the controller stage in
`cubarium-voxel-fauna/src/step.rs` and adds timers in `cubarium-voxel/src/profile.rs`.

- **#1 depends on FAUNA-PAR for its gain, not its code.**
  - It touches the plant clock in `cubarium-voxel-flora/src/step.rs` (`advance`) and the clock's
    config. It can be built now.
  - Its saving only appears once fauna is below about 1.5 ms. Measure before and after.
- **#2's flora half** (split `run` at the drink, hand the plan out early, add the schedule edge in
  `cubarium-voxel-sim/src/lib.rs`) touches only flora and the sim. It can be built now.
- **#2's fauna half touches FAUNA-PAR's file.** The fauna step must become a read half (terrain,
  maintenance, field, controllers) taking a flora view, and a write half (act, births, deaths,
  burrs). Build it after FAUNA-PAR merges. Both legs share one compute pool, so the gain shrinks
  the better FAUNA-PAR fills it.
- **#3** rewrites act's feeding (FAUNA-PAR's file). Last, if at all.

**Order:**
1. FAUNA-PAR merges and is measured.
2. #1, then #2's flora half (or both now on a branch).
3. #2's fauna half.
4. Re-measure, then look inside the plant step.
