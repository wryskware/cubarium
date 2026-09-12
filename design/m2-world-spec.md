---
design_status: leaning
last_reviewed: 2026-09-11
decision_refs: []
---

# M2 world specification — the persistent feeding world

This is the implementation-level plan for M2 in the
[implementation plan](implementation-plan.md): the concrete state, units,
conversion table, tick order, controller, and persistence format that
`cubarium-core` implements. It refines the [ecology](ecology.md),
[evolution](evolution.md), and [architecture](architecture.md) proposals for
the first living world. Everything numeric is a tuning hypothesis for E2/E3, not
a promise. Nothing here is canon.

## Units and quantities

| Symbol | Unit | Meaning |
| --- | --- | --- |
| `m` | material | The single conserved material unit |
| `e` | energy | Usable chemical energy; enters from light, leaves as heat |
| tick | 1/20 s | Simulation step; all rates are configured per second and multiplied by `dt = 0.05` |
| px | pixel | Surface length |

Per cell (1,280 cells, 4×4 px each): free nutrient `N` (m), producer pool `P`
(m), detritus `D` (m), detritus energy `De` (e, bounded by `e_d_max · D`).
Light `L ∈ [0,1]` and moisture `W ∈ [0,1]` are derived each tick from the
static habitat and slow weather; they are not stored material.

Per organism: structure `S` (m), reserve `R` (m), energy `E` (e), position,
unit heading, age (ticks), hunger memory, gestation escrow, stable ID with
generation, parent ID, genome, origin tag.

Closed-box invariant, checked every tick in debug and every 60 s in release:

```
M = Σ_cells (N + P + D + F) + Σ_organisms (S + R) + Σ_escrow (S_child + R_child)
```

(`F` is the fruit pool of `design/fauna-v2.md`, added 2026-09-12.)

`M` is constant to rounding except for named external material sources
(none in M2 besides the initial seed). Energy is not conserved; it is
audited exactly. Reserve material carries chemical energy at density `e_r`,
structure carries none (construction energy is heat), so the stored total is

```
E_total = Σ_cells (e_p · P + e_f · F + De) + Σ_organisms (E + e_r · R)
        + Σ_escrow (e_r · (S_child + R_child) + E_child)
```

and every tick `ΔE_total = light_in − heat_out` to rounding. Both sides are
logged. Validation requires `e_p ≥ e_r · η_m` so grazing never needs more
reserve energy than the food supplied.

## Conversion table

| Process | Material | Energy | Rate (initial values) |
| --- | --- | --- | --- |
| Producer growth | `N → P` | `+e_p` per m from light (source) | `g · L_eff · W_eff · P · (1 − P/P_max) · N/(N + K_N) · drown` with `L_eff = max(L, algae_light · min(w/algae_depth, 1))` (`design/water.md` "Algae"), capped by `N` and by `f_max · N`; `g = 0.008/s`, `P_max = 1.5 m`, `K_N = 0.25 m`, `f_max = 0.5/s`, `e_p = 2 e/m` |
| Producer mortality | `P → D` | `De += e_p · ΔP`, then clamp to `e_d_max · D` (excess is heat) | `m_p = 0.001/s` (0.0005 until fauna v2: the soil's scavengers live on what falls from above), `e_d_max = 2 e/m` (= `e_r`; 1 e/m until fauna v2, 2026-09-12) |
| Decomposition | `D → N` | `De` shrinks by the fraction of detritus removed; that energy is heat | `k_d = 0.002/s` |
| Detritus fall | D → D of the downhill neighbor, De in the same proportion | none (transfer) | fall = 0.02 /s; downhill = the graph neighbor with the lowest embedded y if strictly lower; none on the top face or the bottom row |
| Rain / flow / evaporation | none: water `w` is not material | none: an open, audited budget `Δ Σw = rain_in − evap_out` | `design/water.md`: rain `0.6 d/s` per unit blob sum above 0.35, flow `3.0 /s` on surface level `z + 0.4·w` over basins of ±0.15, evaporation `0.008 · max(L, 0.5) /s`; growth sees `W_eff` and drowning; movement wades at `1/(1 + w)` |
| Grazing intake | `P → R` (η_m) and `P → D` (1 − η_m) in the cell; feces carry no energy | food energy `e_p · q`; reserve stores `e_r · η_m · q`; `E += η_e · (e_p − e_r · η_m) · q`; the rest is heat | `q = min(k_mouth · effort · dt · P/(P + K_P), R_max − R)` requested against the cell's pre-settlement `P`; `k_mouth = 0.05 m/s`, `K_P = 0.45 m` (a saturating, type-II intake: a poor cell is poor food), `η_m = 0.6`, `η_e = 0.5` |
| Scavenging intake | `D → R` with effective `η = η_m · min(1, ρ/e_r)`, `ρ = De/D`; the un-assimilated `(1 − η) · q` stays in `D` energy-free | food energy `ρ · q` leaves `De`; reserve stores `e_r · η · q`; `E += η_e · (ρ · q − e_r · η · q)`; the rest is heat | same `q` law with its own effort and `D_eff/(D_eff + K_P)` as the saturation term; its headroom is `R_max − R − q_graze` (grazing settles first) |
| Maintenance, movement, sensing | none | `paid = min(cost · dt, E)`; `E −= paid`; heat `paid` (so `E ≥ 0` always) | `c_maint = 0.005 e/s per m`, `c_move = 0.006 e/s per m per px/s`, `c_sense = 0.0002 e/s per px`; `v ≤ v_max = 0.3 px/s` (1.5 until the fourth E2 batch showed that fast movers erase all patch structure; 0.3 survived every 24-hour confirmation seed), `r_sense = 6 px` |
| Reserve oxidation | `R → N` in the organism's cell | `E += η_ox · e_r · ΔR`; heat `(1 − η_ox) · e_r · ΔR` | when `E < 0.5 · E_max` and `R > 0`: `ΔR = 0.01 m/s`, `e_r = 2 e/m`, `η_ox = 0.8` |
| Growth | `R → S` | heat `e_r · ΔS` (reserve energy released) plus `E −= c_build · ΔS` (heat) | while `S < S_adult` and `R > 0.3 · R_max`: `ΔS = 0.01 m/s`, `c_build = 0.5 e/m` |
| Budding escrow | `R → escrow` (`S_child + R_child`) | escrow holds `e_r · (S_child + R_child) + E_child`; `E −= c_build · S_child + E_child` (`c_build` part is heat) | at conception; see controller |
| Birth | `escrow → child (S, R)` | child `E = E_child`, child reserve carries `e_r · R_child`; heat `e_r · S_child` | after `t_gest = 30 s` |
| Cap-rejected birth | `escrow → parent R` | `E += E_child` (unclamped; exact refund) | at commit when the world is full |
| Failed gestation | `escrow → D` in the parent's cell | `De += min(escrow energy, e_d_max · escrow material)`; rest heat | parent death during gestation |
| Death | `S + R → D` in the cell | `De += min(E + e_r · R, e_d_max · (S + R))`; rest heat | `E ≤ 0 && R ≤ 0`, or age ≥ `t_max = 2 h`, or `S < 0.1 m` |
| Ripening (`design/fauna-v2.md`) | `P → F` where `P > fruit_min · P_max` | `light_in += (e_f − e_p) · ΔF`; fruit carries `e_f = 3 e/m` | `ripen · P · (P/P_max − fruit_min)⁺ · L`, `ripen = 0.02 /s`, `fruit_min = 0.3`, never more than the cell holds after growth and mortality |
| Fruit drop | `F → D` | `De += e_f · ΔF`, clamped to `e_d_max · D`, excess heat | `drop = 0.004 /s` |
| Frugivory | `F → R` (η_m) and `F → D` (1 − η_m) | food energy `e_f · q`; reserve stores `e_r · η_m · q`; `E += η_e · (e_f − e_r · η_m) · q`; rest heat | same `q` law as grazing with `F/(F + K_P)` and the diet's `graze_rate`; requested first, grazing and scavenging take the remaining headroom; only for `diet ≥ 0.5` |

Proportional allocation: when the intake requests on a cell sum above the
available `P` (or `D`), each consumer receives `Q · q_i / Σq`. Requests are
collected after movement and settled once from the pre-transfer field.

Every process is a pure function of the pre-tick state; no process reads a
value another process changed in the same phase.

## Habitat and weather

Static habitat is a smooth function of the embedded position `p ∈ [−1,1]³`:

- height `h = p.y` (Top = 1, rim = −1), continuous across seams.
- patch noise `n(p) ∈ [−1,1]`: a fixed sum of six 3D cosine waves with
  irrational-ratio frequencies (wavelength 0.6–1.4 cube units) and phases from
  the habitat seed. No per-face noise seeds.
- base light `L₀ = clamp(0.45 + 0.55·h + 0.3·n, 0, 1)`. Adopted 2026-09-12 for the
  stratified cube (`design/stratified-world.md`): the rim is dark, the canopy fully lit.
- base moisture `W₀ = clamp(0.8 − 0.3·h + 0.4·n(p + shift), 0.1, 1)`.

The noise gains were 0.1 and 0.2 through the second E2 batch; the third batch
tests the stronger contrast because a nearly uniform habitat gave a uniform
lawn.

Weather adds three broad moving blobs for light and three for moisture, each a
raised-cosine cap on the unit sphere (angular radius 55°) with amplitude 0.3
(0.15 through the second E2 batch),
centers drifting on the sphere with periods 20, 33 and 47 minutes plus a slow
random walk from the `Weather` stream (one draw per blob per minute). Static
mode freezes the blob centers. `L = clamp(L₀ + Σ light blobs, 0, 1)`, likewise `W`.

Initial fields: `N = 0.5 m` per cell, `P = 0.4 · L₀ · W₀ · P_max` (0.6 until
fauna v2: a fresh world starts as regrowth, not a saturated thicket), litter
`D = 1.2 · (1 − L₀)` with `De = e_d_max · D` (initial material, adopted
2026-09-12 so the dark soil starts littered and the lit canopy clean) (the
founding stock must exceed `feed_min` in ordinary foliage and canopy cells, or
the world starts in famine; soil cells start nearly bare by design since
2026-09-12, see `stratified-world.md`).

The first E2 batch (2026-09-11, `runs/e2-first`) showed that with `N = 1.0`
and no saturation term nutrient never limited growth: nutrient diffusion had
no ecological effect and every viable configuration settled at the `feed_min`
producer floor everywhere with a flat population. The Monod term and the
scarcer initial nutrient make local recovery depend on recycling through
detritus, which is the feedback the ecology proposal relies on.

## Water

Adopted 2026-09-12 (`design/water.md`, which is normative). Each cell holds a
water depth `w` (unit `d`), checkpointed with the fields but outside the
material invariant. Rain falls where the moisture weather blob sum exceeds a
threshold, so showers are the cores of the moving blobs; water flows along
graph edges from higher to lower surface level `s = z + depth_gain · w` over
terrain `z = h + basin_gain · n_b`, capped per edge so depth never goes
negative; it evaporates in proportion to light. Growth sees the effective
moisture `W_eff = clamp(W + wet_gain · min(w, 1), W_min, 1)` and a drowning
factor above `flood`; organisms wade at `speed / (1 + w)`. The budget
`Σw = rain_in_total − evap_out_total` is asserted every tick in debug builds
and reported in telemetry.

## Organism representation

Genome v2 (`design/fauna-v2.md`, 2026-09-12). The v1 fields (all `f32`,
bounded): `size` (0.5–2), `metabolism` (0.5–2), `sense` (2–12 px), `reserve`
(0.5–2), `mouth` (0.2–1), `speed` (0.3–1), `hue` (0–1), and the drive vector
below. Version 2 adds `diet` (0–1: grazing rate `k_mouth · diet`, scavenging
rate `k_mouth · (1 − diet)`; grazing needs `diet ≥ 0.05`, scavenging
`diet ≤ 0.95`, fruit `diet ≥ 0.5`), `depth` (0–1: preferred height
`h_pref = −1 + 2 · depth`), `swim` (0–1: wading penalty
`speed / (1 + w · (1 − swim))`), `form` (0–7: the authored rig, heritable,
never mutated), and the drive `w_depth` (0–2, 1.0). A v1 genome loads with
`diet` 0.7, `depth` 0.5, `swim` 0, `form` = hue tercile and is stamped
version 2. Founders come in kinds (`founders.kinds`: burrower, grazer, glider,
skimmer by default; an empty list is the v1 path of `founders.count`
identical founders). With `mechanisms.mutation` on, each birth differs from
its parent with probability `p_mut = 0.3` at one or two loci from {`size`,
`speed`, `sense`, `reserve`, `mouth`, `hue`, `diet`, `depth`, `swim`,
`w_depth`} by a Gaussian step of 0.08 of the range, clamped; the birth event
records `(locus, from, to)`.
Phenotype decode (once at birth):

- `S_adult = size · 1 m`, `R_max = reserve · size · 1 m`, `E_max = 2 e · size`
- `v_max = speed · 0.3 px/s · size^(−0.25)`
- `k_mouth = mouth · 0.05 m/s · size^0.75`, `graze_rate = k_mouth · diet`,
  `scavenge_rate = k_mouth · (1 − diet)`, `r_sense = sense` (founders take `sense` from
  `organism.sense_radius`, default 6; the third E2 batch's sensing axis was
  inert because founders ignored the config)
- maintenance multiplier `metabolism`; `c_move`, `c_sense` fixed
- body: two lobes (core radius `0.9 + 0.5·size`, head radius `0.6 + 0.3·size`
  at `+1.6·size` forward), plus a tail lobe at `−1.4·size` when `speed > 0.6`.
  Extent must stay ≤ 9 px.

Drives (M2 fixed values in parentheses): `w_food` (1.0), `w_detritus` (0.4),
`w_persist` (0.3), `w_crowd` (0.6), `seek_on` (0.3), `seek_off` (0.1),
`feed_min` (0.2 m per cell, leaving a regrowth refuge of a tenth of `P_max`), `rest_effort` (0.05), `bud_reserve` (0.7),
`bud_energy` (0.3), `bud_min_age` (120 s), `tau_hunger` (10 s),
`turn_rate_max` (90°/s), `turn_noise` (0.6 rad/√s), `w_depth` (1.0).

## Controller (named drives, two memories)

Observation each tick, all in the organism's chart via `unfold` to the
centers of the graph cells within `ceil(r_sense / 4)` hops of the own cell
(a 6 px sensor sees two cells out; a rim cell simply lacks neighbors across
the rim): `P`, fruit and edible-detritus gradients as
`Σ (value_i − value_0) · dir_i / dist_i`, normalized (controller v2,
`design/fauna-v2.md`; v1 read only the four adjacent cells). Edible detritus is `D_eff = D · min(1, ρ / e_r)` with `ρ = De / D`
(zero when `D = 0`): detritus that cannot fuel reserve storage is not food,
so `d_here`, the detritus gradient, and the Feeding threshold all use `D_eff`.
Organisms therefore leave an energy-poor detritus carpet instead of grazing it
forever at near-zero yield. Since fauna v2 (2026-09-12) `e_d_max = e_r = 2 e/m`,
so fresh detritus and the initial litter are fully edible and a soil
scavenger can live on litter (through the second E2 batch the cap was 1 e/m
and detritus at most half edible, which starved every pure scavenger);
energy-poor detritus still reads as poor food through `D_eff`;
neighbor organisms within `r_sense` from the pair pass (positions in own chart,
IDs deduplicated). Hunger `h = 1 − R/R_max`; hunger memory
`m_h += (1 − exp(−dt/τ)) (h − m_h)`.

Mode with hysteresis: `Seeking` when `m_h > seek_on`, back to `Resting` when
`m_h < seek_off`. `Feeding` when Seeking and the own cell holds a food this
diet can take: `P ≥ feed_min` with `diet ≥ 0.05`, `F ≥ feed_min` with
`diet ≥ 0.5`, or `D_eff ≥ feed_min` with `diet ≤ 0.95` and the scavenging
channel.

Steering vector (controller v2, `design/fauna-v2.md`)
`s = diet · w_food · h · (∇P + ∇F) + (1 − diet) · w_detritus · h · ∇D_eff +
w_crowd · repulsion + w_depth · (h_pref − h) · up + w_persist · ou`, where
`up` is the unit chart direction of increasing embedded height (zero on the
top face), `ou` is the organism's Ornstein–Uhlenbeck turn noise (draws from
the `OrganismTurn(id)` stream, one per tick) and `repulsion` sums
`(own − other)/|·|² · extent_sum` over neighbors closer than the sum of body
extents plus 1 px.

Turning is gated by mode. With `k = 1` while Seeking, `k = feed_turn_fraction`
while Feeding and `k = rest_turn_fraction` while Resting
(`organism.feed_turn_fraction`, default 0.1, and `organism.rest_turn_fraction`,
default 0.0): the noise update is `ou' = ou · (1 − dt/τ_ou) + noise ·
turn_noise · √dt · k`, and the heading turns toward `s` by at most
`turn_rate_max · dt · k`. `k` uses the mode decided this tick. A resting body
holds its heading while its noise decays; a feeding body reorients at a tenth
of its seeking rate. Both fractions at 1 restore the ungated behaviour of the
E2 batches. Adopted 2026-09-12 after the art review found resting and feeding
bodies rotating fully while translating a fraction of a pixel.

Effort: Seeking 1.0, Feeding 0.2, Resting `rest_effort`. Movement
displacement `heading · effort · v_max · dt / (1 + w · (1 − swim))`, then
`travel`; the heading and the OU vector are transported by `Travel::map`.

Budding: when `R ≥ bud_reserve · R_max`, `E ≥ bud_energy · E_max`,
`age ≥ bud_min_age`, not gestating, and the population is below the cap
(checked before escrow): escrow `S_child = 0.4 · S_adult`, `R_child = 0.2 ·
R_max`, pay `c_build · S_child + E_child` with `E_child = 0.15 · E_max`.
After `t_gest` the child is placed 2.5 px away in a direction from the
`Birth(id)` stream via `travel` (reflection included), heading random,
origin `Descendant`, genome copied exactly or, with `mechanisms.mutation`,
mutated as "Organism representation" describes; each birth's draws occupy
their own block of sixteen counters (placement first, then mutation).

## Fauna v2

Adopted 2026-09-12 (`design/fauna-v2.md`, which is normative): genome v2 with
`diet`, `depth`, `swim`, `form` and `w_depth`; founder kinds (burrower, grazer,
glider, skimmer); the fruit pool `F` with ripening, drop and frugivory;
controller v2 with diet-weighted gradients sensed `ceil(r_sense / 4)` hops
out, a depth preference, and the mode turn gate; sparse mutation at birth
recorded in the birth event. The render view carries each organism's `form`
and the fruit field; telemetry carries `fruit`, `population_by_form` and
`mean_height_by_form`.

## Tick order

1. Admit due stimuli (none in M2; the queue exists).
2. Weather advance; compute `L`, `W` and the rain source per cell; then water:
   rain, flow, evaporation (`design/water.md`).
3. Field reactions from double-buffered values: growth, mortality,
   decomposition, fruit ripening and drop, detritus fall; then `N` diffusion
   at rate 0.05/s (`diffuse` with 0.0025 per tick, no substeps needed).
4. Pair pass: chord-filtered all unordered pairs, exact `unfold` for candidates
   with chord ≤ `max(r_sense) + 2·extent_max`; build each organism's neighbor
   list (bounded to the 16 nearest by distance, deterministic tie by ID).
5. Observe and decide (pure): steering, mode, effort, feed requests, bud intent.
6. Move: `travel`, transport tangents, pay movement/sensing/maintenance.
7. Settle feeding requests per cell proportionally (fruit, then grazing,
   then scavenging against the remaining headroom); apply assimilation.
8. Physiology: oxidation, growth, gestation progress, death checks. Queue
   births and deaths.
9. Commit: remove dead (to `D`), place births (capacity re-checked; a rejected
   birth refunds escrow to the parent's `R`), advance ages.
10. Invariants (mass, nonnegativity, finiteness), render view, telemetry event.

Iteration is by slot index; results never depend on order because every phase
reads the pre-phase state and settlement is proportional.

## Randomness

Counter-based keyed draws: `draw(stream, key, counter) → u64` via SplitMix64
finalization of `(world_seed ⊕ stream_id, key, counter)`, then `u64 → f64` in
`[0,1)`. Streams: `Weather` (key = blob index, counter = minute),
`OrganismTurn` (key = organism ID, counter = per-organism counter),
`Birth` (key = parent ID, counter = birth count), `Founders` (key = index).
Rendering and logging never draw. Counters are checkpointed with their owners.

## Capacity and IDs

Slots: `OrganismId { slot: u32, generation: u32 }`, free list reused with
generation bump; a stale ID never resolves. Cap 512 active; conception is
refused at the cap before escrow. Neighbor lists bounded at 16.

## Persistence

Snapshot = header + `postcard`-encoded `WorldState` (serde). Header: magic
`CUBW`, schema version `u32`, build ID (git hash string), payload length `u64`,
CRC32 of payload. Written by a worker thread to `state/tmp-<tick>.cubw`,
fsynced, renamed to `state/world-<tick>.cubw`, directory fsynced; keep the
newest 8, never delete the newest valid one. Every 60 simulated seconds and on
shutdown. Load validates magic, version, length, CRC, then value ranges
(finite, nonnegative, capacities, canonical positions); on failure try the
next older file; if none load, start a new world only under the configured
unattended policy, logging loudly. Spatial caches rebuild on load.

The journal (`state/journal-<n>.log`) records admitted stimuli and config
changes as JSON lines with tick and sequence; empty in M2 but wired.

## Observer

Field dumps: when `capacity.field_dump_seconds > 0`, the host writes
`fields.jsonl` in the state directory (or the `--fields` path). Its first line is a header
`{"cells": [[n0, n1, n2, n3], …]}` giving each cell's graph neighbors in `Edge`
order (`null` at the rim) so analyzers can compute graph distances without the
Rust crate; each later line is `{"tick", "n", "p", "d", "de", "organisms"}`
with 1,280-element arrays (fields rounded to four decimals, organisms as
per-cell counts), on the same absolute-tick cadence rule as telemetry.
Telemetry samples also carry `producer_by_face` and `detritus_by_face` sums.

Life events: when `capacity.event_log` is true, the host appends one JSON line
per birth and death to `events.jsonl` beside the telemetry file:
`{"kind":"birth","tick","id","parent","parent_age_ticks","parent_births","genome":digest,"origin"}`
and `{"kind":"death","tick","id","age_ticks","cause","births","genome":digest}`
(`id` and `parent` as `slot:generation`). These records are what E3 needs
(parent age at birth, time to first reproduction, ancestry depth, reproductive
skew); the world exposes them as `World::drain_events()` after each tick and
never reads them back.

Headless telemetry as JSON lines every 5 simulated seconds: tick, population,
births, deaths by cause, escrows, cap rejections, `Σ N/P/D`, organism material,
total energy, light in, heat out, per-face population, occupied cells, fallbacks
and ties from `travel`, and the mass residual. Plus `--speed N` accelerated runs
without a sink for E2/E3, and fixed-seed replay hash of the state each 60 s.

## Presentation

Colors follow [appearance](appearance.md) "Palette" (sRGB, decoded to linear light in
the host). A uniform floor of `#12093A · 0.12` is added to every pixel first, so empty
surface reads as night rather than as an off panel. Substrate: `P` ramped
`#1E2798` → `#42C5F8` by `t = min(P/P_max, 1)` and scaled by the same `t` (seam-aware
filter on), `D` as sparse violet flecks (`#510B6D · min(D/1.5, 1)` at cell level,
nearest, only when `D > 0.05`). Bodies: hue accent from the genome mapped to the
`#FF2AFC` → `#42C6FF` ramp, brightness 0.55 resting, 0.8 seeking, the core lobe
flashing `#FF9B50` at brightness 1.0 while feeding; a short trail (12 segments, 3 s) at
0.25 of the body color. No overlays.

Between simulation ticks the presenter interpolates each body along `moved`, the path it
traveled during the last completed tick, by arc length: at fraction 0 it sits where that
tick began and approaches `pos` as the fraction approaches 1. Fields and trails are not
interpolated.
