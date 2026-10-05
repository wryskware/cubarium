---
design_status: exploration
last_reviewed: 2026-10-05
decision_refs: []
---

# The whole-web model: the producer half

**Status:** built, not yet validated. The stand law and the ledgers pass their checks. The
occupancy dynamics do not yet meet the bar (see Validation). Not canon.

Code: `crates/cubarium-webmodel/src/flora/`. Driver: `examples/flora_model.rs`
(`dump`, `census DIR`, `physio …`, `run start=DIR snaps=DIR,… [log=SOAKLOG] [weather=frozen]`).
Tests: `tests/flora_laws.rs`.

## State and laws

| piece | what it is | sim source | what the aggregation loses |
|---|---|---|---|
| zones (`zones.rs`) | faces classed by material, highest-in-column, sky band cut at the light gates (0.05/0.35/0.6/0.75/0.9), wet = springturf's aeration or flooding gate fails in ≥ half the snapshots. 26 zones on the terrarium. | gates `step.rs:4029` | within-zone spread of sky, water and mineral |
| weather | each census snapshot is one weather state; per zone and species: mean `μ`, aeration target, gate pass, and the same on the faces the species holds. `Replay` draws a state every 60 min; `Frozen` holds the time mean. | `establishment_gates_with_sky` | spatial pattern of wetting; the 60 min period is an assumption |
| stand law (`stand.rs`) | W, P, Q, parcel, aeration stress, cure clock, nectar, fruit, runner support; restated line by line | `grow_stand_cause` `step.rs:2924–3260`, `dormant` :3293, funding `seeds.rs:880` | layer stocks, defence, stand mineral inventory (taken as `n_tissue · material`), fruit rest as its mean |
| cohorts (`model.rs`) | stands per zone and species, binned hourly by log-wood (16 bins); newborn nurseries 10 min | – | individual variation inside a bin |
| space | plant place: vacant + every species; fungus place separate (`FaceLayer`, `lib.rs:801`) | – | adjacency |
| establishment | seed bank per zone and species; vacant faces taken within two checks by weighted lottery (`seeds.rs:183`, :665); covers overtopped at `overtop_per_check / seed_check_s` (`seeds.rs:80`, :466); the canopy gate as a mineral threshold `N*` from the newborn break-even (`step.rs:3760`, :3787) over a lognormal spread of face mineral | `seeds.rs` | which faces carry the bank |
| runners | `clonal_share`, a target in the runner box with prob `1 − (1 − θ)^n`, kept otherwise (`seeds.rs:913`); parent support 1 h | `seeds.rs:1078`, `step.rs:2525` | the box's real contents |
| dispersal (`kernel.rs`) | zone-to-zone landing matrices sampled over the world's own faces (Drop, Wind Lomax, Wald, Spores with gate screening, dark spores) | `seeds.rs:1168–1524` | donors' clumping |
| bank decay | one rate `1 / ((1 − e⁻¹) · seed_max_age_s)` | `seeds.rs:745` | the age cut-off's shape |
| mineral | site pool per zone and occupant; packages export their mineral; dead seeds return it where they rot | `step.rs:3213`, `seeds.rs:953` | litter mineral returns at once; fungal translocation |
| fungus | substrate feed from the zone's litter + dead wood, yield, seep in the dark | `feed` `step.rs:2183`, `saprotrophy` :2898 | the 5×5 mycelium box: every fungus reaches its zone's whole pool |
| detritus | litter, dead wood (`decomposition`, `wood_decomposition`), carrion (`carrion_decomposition`), film (logistic) | `step.rs:3659`, `film.rs` | film spread and dry death |
| exchange | `supply()` edible via the sim's own `layers_of(...).edible()`; `take` / `returns` applied each step | `lib.rs:3792` | per-species split of the logged foliage take |

## Fitted rates (spatial, never sim knobs)

- **Relative light** per species and stage, `L/sky = a − k·CAI` (`fit::light`), from the
  censuses of the seed's own saves (s5: m250, m310, m370, m1460, m1520, m1570). k came out ≈ 0
  for nearly every species (CAI does not explain shade); a ≈ 0.45 (turf juveniles) to 0.95
  (vaulttree adults). Spreads over snapshots are printed by `run`.
- **Mineral spread** σ (log-sd of site mineral over one occupant's faces): census means of
  m250/m370 and m1570, seeds 5 and 6; it grows from ~0.25 to ~0.5 over the soak (`Params::sigma`).
- **Seed clumping** ψ on takeable faces, from the census banks: lanternberry 0.54–0.93,
  vaulttree 0.98–1.15, springturf 0.98–1.15. Set to 1 for now.
- **Weather period** 60 min (per-face correlation of available water 0.55–0.88 between
  snapshots 50–60 min apart; zone means swing 0.17 → 0.57 in an hour).

## Tests (`tests/flora_laws.rs`, 0.05 s)

1. Law anchor: the stand law at the sim's tick equals `study::probe` (the sim's own
   `grow_stand` on a one-stand fixture) to 1e-9 for springturf, velvetpad, vaulttree,
   stonecushion, bloomcrown over dusk and night. Lanternberry differs ~3 % (its keyed
   fruit-rest draw).
2. Law anchor: the plant clock equals `DayClock::sun`.
3. Conservation: the organic ledger and the mineral close over 1.5 world days with growth,
   seed rain, takeover, deaths, decay and a clamped take.
4. Config flow: springturf maintenance up → fewer packages; propagule rate up → never fewer;
   a woody `overtop_per_check` of 0 → no turf overtopped by it; glowcap `foliage_rate` up →
   caps regrow faster.

## Validation so far (soak v2, s5, m370 → 1570, replay weather, take clamped to the log)

Sim / model at m1570: springturf 1040/4, stonecushion 786/42, velvetpad 6401/3484,
lanternberry 2412/8153, vaulttree 445/296, glowcap 10227/10590, glowcap edible caps 1727/1799.
**Fails the bar.** The model's woody establishment is 2–3× the sim's, so turf is overtopped
instead of starving into gaps, and stonecushion (which lives off those gaps) never rises. The
glowcap's stands and caps are within 4 %.

What the model needs to fix this is per-species demography from the sim, which the logs did
not have: the `sp` lines now printed by `succession_study` (seeds landed, germinated, died,
lost; runner births; deaths by cause; overtopped by whom; canopy refusals).

## Findings that stand without the dynamics

- **The grass drains its own ground.** Site mineral under springturf falls from ~0.9 (m250)
  to a median 0.42–0.58 (m1460–1570) while stonecushion faces rise to 1.0–1.5 and bare dim
  ground to 1.3–1.9. The turf's seed rain (≈120 seeds per adult per day, 1.55 M over the soak
  on s5) carries ~0.036 mineral per adult per day off its face. On that ground a mature turf
  dies in a dry spell: `physio springturf 0.72 0.15 0 0.42` starves in 1.55 days, at
  N = 1 it lives. Mature turf starvation (~2,600 on s5, beside 694 overtopped) is the main
  loss, and the gaps go to stonecushion and woody seedlings.
- **Succession on the hill (census, s5, open + summit drained, 2,610 faces):** turf
  2,001 → 947, vaulttree +434, lanternberry +359, stonecushion +584, bare −340, m370 → m1570.
- **Woody seedlings stall under their parents' clumps** (raw light ≈ 0.33; lanternberry
  juveniles' median wood 0.09–0.10 at both ends of the soak); the probe confirms a newborn
  at that light barely grows in two days and matures in two at 0.5.
- **The glowcap is bounded by space and its own size.** At m1570 every fungus is at
  `wood_max` 0.1 with full caps (0.2): 10,227 (s5) and 10,836 (s6) stands, 100 % of the
  drained and dark faces it can hold. Its cap production at most `foliage_rate · W` per stand,
  0.173 per world day: about 1,770 per world day for the whole world if grazed below full.
  The capgnawers took about 1.5 per world day (assimilation 7.4 over the soak at yield 0.48).
  It does not crowd plants: it holds the face's fungus place only, casts no shade, and holds
  83 of 14,000 mineral. It does move mineral: dark drained ground rises from 1.0 to 1.8.
