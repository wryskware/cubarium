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
M = Σ_cells (N + P + D) + Σ_organisms (S + R) + Σ_escrow (S_child + R_child)
```

`M` is constant to rounding except for named external material sources
(none in M2 besides the initial seed). Energy is not conserved; it is
audited exactly. Reserve material carries chemical energy at density `e_r`,
structure carries none (construction energy is heat), so the stored total is

```
E_total = Σ_cells (e_p · P + De) + Σ_organisms (E + e_r · R)
        + Σ_escrow (e_r · (S_child + R_child) + E_child)
```

and every tick `ΔE_total = light_in − heat_out` to rounding. Both sides are
logged. Validation requires `e_p ≥ e_r · η_m` so grazing never needs more
reserve energy than the food supplied.

## Conversion table

| Process | Material | Energy | Rate (initial values) |
| --- | --- | --- | --- |
| Producer growth | `N → P` | `+e_p` per m from light (source) | `g · L · W · P · (1 − P/P_max)`, capped by `N` and by `f_max · N`; `g = 0.008/s`, `P_max = 2 m`, `f_max = 0.5/s`, `e_p = 2 e/m` |
| Producer mortality | `P → D` | `De += e_p · ΔP`, then clamp to `e_d_max · D` (excess is heat) | `m_p = 0.0005/s`, `e_d_max = 1 e/m` |
| Decomposition | `D → N` | `De` shrinks by the fraction of detritus removed; that energy is heat | `k_d = 0.002/s` |
| Grazing intake | `P → R` (η_m) and `P → D` (1 − η_m) in the cell; feces carry no energy | food energy `e_p · q`; reserve stores `e_r · η_m · q`; `E += η_e · (e_p − e_r · η_m) · q`; the rest is heat | `q = min(k_mouth · effort · dt, R_max − R)`, `k_mouth = 0.05 m/s`, `η_m = 0.6`, `η_e = 0.5` |
| Scavenging intake | `D → R` with effective `η = η_m · min(1, ρ/e_r)`, `ρ = De/D`; the un-assimilated `(1 − η) · q` stays in `D` energy-free | food energy `ρ · q` leaves `De`; reserve stores `e_r · η · q`; `E += η_e · (ρ · q − e_r · η · q)`; the rest is heat | same `q` law with its own effort; its headroom is `R_max − R − q_graze` (grazing settles first) |
| Maintenance, movement, sensing | none | `paid = min(cost · dt, E)`; `E −= paid`; heat `paid` (so `E ≥ 0` always) | `c_maint = 0.005 e/s per m`, `c_move = 0.006 e/s per m per px/s`, `c_sense = 0.0002 e/s per px`; `v ≤ v_max = 1.5 px/s`, `r_sense = 8 px` |
| Reserve oxidation | `R → N` in the organism's cell | `E += η_ox · e_r · ΔR`; heat `(1 − η_ox) · e_r · ΔR` | when `E < 0.5 · E_max` and `R > 0`: `ΔR = 0.01 m/s`, `e_r = 2 e/m`, `η_ox = 0.8` |
| Growth | `R → S` | heat `e_r · ΔS` (reserve energy released) plus `E −= c_build · ΔS` (heat) | while `S < S_adult` and `R > 0.3 · R_max`: `ΔS = 0.01 m/s`, `c_build = 0.5 e/m` |
| Budding escrow | `R → escrow` (`S_child + R_child`) | escrow holds `e_r · (S_child + R_child) + E_child`; `E −= c_build · S_child + E_child` (`c_build` part is heat) | at conception; see controller |
| Birth | `escrow → child (S, R)` | child `E = E_child`, child reserve carries `e_r · R_child`; heat `e_r · S_child` | after `t_gest = 30 s` |
| Cap-rejected birth | `escrow → parent R` | `E += E_child` (unclamped; exact refund) | at commit when the world is full |
| Failed gestation | `escrow → D` in the parent's cell | `De += min(escrow energy, e_d_max · escrow material)`; rest heat | parent death during gestation |
| Death | `S + R → D` in the cell | `De += min(E + e_r · R, e_d_max · (S + R))`; rest heat | `E ≤ 0 && R ≤ 0`, or age ≥ `t_max = 2 h`, or `S < 0.1 m` |

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
- base light `L₀ = clamp(0.55 + 0.35·h + 0.1·n, 0, 1)`.
- base moisture `W₀ = clamp(0.8 − 0.3·h + 0.2·n(p + shift), 0.1, 1)`.

Weather adds three broad moving blobs for light and three for moisture, each a
raised-cosine cap on the unit sphere (angular radius 55°) with amplitude 0.15,
centers drifting on the sphere with periods 20, 33 and 47 minutes plus a slow
random walk from the `Weather` stream (one draw per blob per minute). Static
mode freezes the blob centers. `L = clamp(L₀ + Σ light blobs, 0, 1)`, likewise `W`.

Initial fields: `N = 1.0 m` per cell, `P = 0.3 · L₀ · W₀ · 2 m`, `D = 0`.

## Organism representation

M2 uses one fixed genotype, but the genome struct and phenotype decode exist so
M3a adds mutation without restructuring. Genome v1 fields (all `f32`, bounded):
`size` (0.5–2), `metabolism` (0.5–2), `sense` (2–12 px), `reserve` (0.5–2),
`mouth` (0.2–1), `speed` (0.3–1), `hue` (0–1), and the drive vector below.
Phenotype decode (once at birth):

- `S_adult = size · 1 m`, `R_max = reserve · size · 1 m`, `E_max = 2 e · size`
- `v_max = speed · 1.5 px/s · size^(−0.25)`
- `k_mouth = mouth · 0.05 m/s · size^0.75`, `r_sense = sense`
- maintenance multiplier `metabolism`; `c_move`, `c_sense` fixed
- body: two lobes (core radius `0.9 + 0.5·size`, head radius `0.6 + 0.3·size`
  at `+1.6·size` forward), plus a tail lobe at `−1.4·size` when `speed > 0.6`.
  Extent must stay ≤ 9 px.

Drives (M2 fixed values in parentheses): `w_food` (1.0), `w_detritus` (0.4),
`w_persist` (0.3), `w_crowd` (0.6), `seek_on` (0.3), `seek_off` (0.1),
`feed_min` (0.05 m per cell), `rest_effort` (0.05), `bud_reserve` (0.7),
`bud_energy` (0.3), `bud_min_age` (120 s), `tau_hunger` (10 s),
`turn_rate_max` (90°/s), `turn_noise` (0.6 rad/√s).

## Controller (named drives, two memories)

Observation each tick, all in the organism's chart via `unfold` to the five
cell centers (own cell and its four graph neighbors; a rim cell simply lacks
one): `P` and `D` gradients as `Σ (value_i − value_0) · dir_i`, normalized;
neighbor organisms within `r_sense` from the pair pass (positions in own chart,
IDs deduplicated). Hunger `h = 1 − R/R_max`; hunger memory
`m_h += (1 − exp(−dt/τ)) (h − m_h)`.

Mode with hysteresis: `Seeking` when `m_h > seek_on`, back to `Resting` when
`m_h < seek_off`. `Feeding` when Seeking and the own cell holds `P ≥ feed_min`
(or `D ≥ feed_min` with the scavenging channel).

Steering vector `s = w_food · h · ∇P + w_detritus · h · ∇D + w_crowd · repulsion +
w_persist · ou`, where `ou` is the organism's Ornstein–Uhlenbeck turn noise
(draws from the `OrganismTurn(id)` stream, one per tick) and `repulsion` sums
`(own − other)/|·|² · extent_sum` over neighbors closer than the sum of body
extents plus 1 px. The heading turns toward `s` at most `turn_rate_max · dt`.
Effort: Seeking 1.0, Feeding 0.2, Resting `rest_effort`. Movement
displacement `heading · effort · v_max · dt`, then `travel`; the heading and
the OU vector are transported by `Travel::map`.

Budding: when `R ≥ bud_reserve · R_max`, `E ≥ bud_energy · E_max`,
`age ≥ bud_min_age`, not gestating, and the population is below the cap
(checked before escrow): escrow `S_child = 0.4 · S_adult`, `R_child = 0.2 ·
R_max`, pay `c_build · S_child + E_child` with `E_child = 0.15 · E_max`.
After `t_gest` the child is placed 2.5 px away in a direction from the
`Birth(id)` stream via `travel` (reflection included), heading random,
origin `Descendant`, genome copied (M2) or mutated (M3a).

## Tick order

1. Admit due stimuli (none in M2; the queue exists).
2. Weather advance; compute `L`, `W` per cell.
3. Field reactions from double-buffered values: growth, mortality,
   decomposition; then `N` diffusion at rate 0.05/s (`diffuse` with 0.0025 per
   tick, no substeps needed).
4. Pair pass: chord-filtered all unordered pairs, exact `unfold` for candidates
   with chord ≤ `max(r_sense) + 2·extent_max`; build each organism's neighbor
   list (bounded to the 16 nearest by distance, deterministic tie by ID).
5. Observe and decide (pure): steering, mode, effort, feed requests, bud intent.
6. Move: `travel`, transport tangents, pay movement/sensing/maintenance.
7. Settle feeding requests per cell proportionally; apply assimilation.
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

Headless telemetry as JSON lines every 5 simulated seconds: tick, population,
births, deaths by cause, escrows, cap rejections, `Σ N/P/D`, organism material,
total energy, light in, heat out, per-face population, occupied cells, fallbacks
and ties from `travel`, and the mass residual. Plus `--speed N` accelerated runs
without a sink for E2/E3, and fixed-seed replay hash of the state each 60 s.

## Presentation

Substrate: `P` rendered dim green (`[0.10, 0.40, 0.16] · min(P/P_max, 1)`, seam-aware
filter on), `D` as sparse warm flecks (`[0.30, 0.18, 0.08] · min(D/1, 1)` at cell level, nearest,
only when `D > 0.05`). Bodies: hue accent from the genome mapped to a warm-to-cool
low-saturation palette, brightness 0.55 resting, 0.8 seeking, core lobe
brightness 1.0 while feeding; a short trail (12 segments, 3 s) at 0.25
brightness. No overlays.
