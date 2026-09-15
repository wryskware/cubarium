---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Ecology v1 contract — structured plants, distinct remains, distinct digestion

A draft for Wrysk's one review, revised after
[Astra's contract review](7_Research/ecology-v1-contract-review-2026-09-15.md)
(§18 maps each finding to its fix). Wrysk's two decisions of 2026-09-15 are
fixed: worlds always restart fresh and are never migrated (§15.1); old trained
policies need not reload (§15.2). §17 lists the routine choices made on his
behalf. Written from the
[food-web review synthesis](7_Research/food-web-review-synthesis-2026-09-15.md)
and its owner clarification: keep inedible or hard-to-digest plant parts and slow
woody decomposition in the first model, defer anatomy, develop incrementally. It
promotes nothing to canon. Every numerical value in this document is
**provisional**: a starting point for the accounting tests and the first
measured scenarios, to be tuned together later (WORKING_POLICY 2026-09-14), not
a design requirement.

The contract keeps the shipped accounting (material `m`, usable energy `e`,
light as the only source, heat as the only sink), the five-face field graph,
the per-cell settlement, the escrow reproduction, the legacy controller and the
recurrent runtime with its 70-input / 7-action layout. It changes the inventory
of stocks and the meaning of the `diet` locus. It does not integrate predators,
add flesh-eating capabilities, seed transport, microbial biomass, moisture-scaled
decomposition, or a second producer; those stay in the later sequence.

## 1. Scope of the first slice

In:

1. Plants with edible foliage, persistent woody structure that ordinary grazers
   cannot take, and a finite stored reserve that pays for recovery.
2. Paid plant income, maintenance and growth; damage from unpaid maintenance;
   stand death; dead wood that decomposes slowly and keeps its identity.
3. Three detrital stocks with identity: plant litter, dead wood, animal remains.
4. Inherited digestive capability with one explicit trade-off and hard exclusions:
   a foliage digester cannot live on litter or remains; a detrital digester cannot
   live on leaves.
5. Background decomposition of each detrital stock with finite, one-way energy
   loss and nutrient return; the existing downhill transport unchanged.
6. Paid animal reproduction and paid juvenile growth, restated as contract terms
   with a test that a finite food input produces finite births.
7. Local recolonisation of a dead cell by paid, viable propagules from living
   neighbours (§4.8). This is the one mechanism not named in the brief; without
   it a killed stand is permanent. It is background recruitment, not the deferred
   animal-carried seed transport.

Out (later sequence): predator integration and a flesh capability, fruit
specialists, wood-eating detritivores, moisture-dependent decomposition, litter
locality changes, dormancy interaction, any parameter search, any training, and
the presentation of living structure, foliage loss and dead wood (§12).

## 2. Units, symbols and constants shared with M2

| Symbol | Meaning | Source |
| --- | --- | --- |
| `m`, `e` | material and usable energy units | [M2 spec](m2-world-spec.md) |
| `dt` | 0.05 s per tick, 20 Hz | `lib.rs` |
| cell | one of 1,280 field cells, 4 × 4 px | `cubarium-surface/src/field.rs:12` |
| `L`, `μ` | sampled light and effective moisture of a cell this tick (the M2 table writes moisture as `W_eff`; this document reserves `W` for wood) | `habitat`, `fields::water_factors` |
| `drown`, algae light | unchanged water terms | `design/water.md` |
| `e_r` | reserve energy density, 2 e/m | `organism.reserve_energy_density` |
| `e_f` | fruit energy density, 3 e/m | `fruit.energy_density` |
| `K_P` | type-II intake half-saturation, 0.45 m | `organism.intake_half_saturation` |
| `η_m`, `η_e` | material and energy assimilation, 0.6 and 0.5 | `organism.assimilation_*` |

## 3. State variables

### 3.1 Per cell (all material in `m`, energies in `e`)

| Symbol | Name | Wire field | Status | Notes |
| --- | --- | --- | --- | --- |
| `N` | mineral nutrient | `fields.n` | kept | required for all plant income |
| `P` | living foliage | `fields.p` | kept | edible by foliage digesters; fixed density `e_v` |
| `F` | fruit | `fields.f` | kept | ripens from `P` as today; fixed density `e_f` |
| `W` | living wood (persistent structure) | new `wood` | new | not edible by any v1 animal; fixed density `e_v`; carries foliage capacity and maintenance cost |
| `Q` | plant reserve | new `plant_reserve` | new | not edible; pays maintenance shortfall, reflush after defoliation, and propagules; fixed density `e_v` |
| `Wd` | dead wood | new `dead_wood` | new | not edible; decomposes slowly; fixed density `e_v` |
| `D`, `De` | plant litter and its energy | `fields.d`, `fields.de` | kept, narrowed | leaf fall, fruit drop, dead reserve, feces (energy-free), care Feed; edible by detrital digesters; variable density `ρ_D = De/D`, capped `De ≤ e_d_max·D` |
| `C`, `Ce` | animal remains and their energy | new `carrion`, `carrion_energy` | new | bodies, failed gestations, dead hunters; edible by detrital digesters in v1; variable density `ρ_C = Ce/C`, capped `Ce ≤ e_c_max·C` |
| `w` | surface water | `fields.w` | kept | not material |

Cell classes, decided once per tick from the **pre-tick** `W⁻`:

- **alive**: `W⁻ ≥ W_min`. Runs §4.1–4.7.
- **establishing**: `0 < W⁻ < W_min`. Holds propagule material (§4.8) frozen:
  no income, no maintenance, no growth, no senescence, no dieback, no death.
  Bounded by the propagule package, it does not decay in v1.
- **bare**: `W⁻ = 0`, and by construction `P⁻ = Q⁻ = 0` for plant purposes
  (litter, fruit and remains may lie there).

Derived quantities:

- `P_cap = min(P_max, α·W)`: foliage the structure can carry.
- `Q_max = q_cap·W`: reserve the structure can hold.
- `D_eff = D·min(1, ρ_D/e_r)`, `C_eff = C·min(1, ρ_C/e_r)`: the edible portions, the
  existing `edible_detritus` rule (`world/invariants.rs:142`) applied per stock.

Fixed plant density `e_v` (one value for `P`, `W`, `Q`, `Wd`) keeps the energy
side of plant tissue implicit exactly as `P` is today: wood is energy-dense and
unavailable, which is the point.

### 3.2 Per organism

Unchanged fields (`organism.rs:47`): `structure S`, `reserve R`, `energy E`,
escrow, genome, phenotype. The phenotype gains two decoded capabilities
(§6.1) and loses the diet-split rates: `graze_rate` and `scavenge_rate` are
both replaced by the one `mouth_rate`.

### 3.3 New world counters (trailing extension, §14)

`plant_deaths_total`, `recolonisations_total`.

## 4. Plant model

### 4.0 Subphases, inputs and outputs

The field-reaction phase (tick step 3) is eight subphases. Each names the state
it reads and the state it writes; a subphase never reads a value another
subphase wrote in the same tick unless the table says so. `X⁻` is the pre-tick
state; `X¹ … X⁸` are the states after each subphase. Cross-cell subphases take
one immutable snapshot and commit all their transfers together.

| Sub | Cells | Reads | Writes | Rule |
| --- | --- | --- | --- | --- |
| 3a | alive | `N⁻ P⁻ W⁻ Q⁻`, `L, μ, drown` | `N¹ P¹ W¹ Q¹` (own cell), `light_in`, `heat_out` | income, maintenance, growth (§4.1–4.4) |
| 3b | alive | `P¹` | `P² D² De²` | senescence (§4.5) |
| 3c | all | `P⁻` for the trigger, `P²` for the cap, `F⁻` | `P³ F³ D³ De³`, `light_in` | ripening and drop, today's rule |
| 3d | alive | `W¹`, `unpaid` from 3a, `P³ Q¹` | `W⁴ Wd⁴ P⁴ Q⁴ D⁴ De⁴`, counters | dieback and death (§4.6–4.7) |
| 3e | all | `D⁻ C⁻ Wd⁻` for the amounts, `D⁴ De⁴ C⁴ Ce⁴` for the energy density | `N⁵ D⁵ De⁵ C⁵ Ce⁵ Wd⁵`, `heat_out` | decomposition (§5); material added in 3a–3d is eligible from the next tick |
| 3f | all | `D⁻ C⁻` for the amounts, `D⁵ De⁵ C⁵ Ce⁵` for the energy density | `D⁶ De⁶ C⁶ Ce⁶` | downhill fall (§5) of the portion of the pre-tick stock that 3e left; one joint withdrawal budget per stock |
| 3g | all | snapshot `N⁵` | `N⁷` | nutrient diffusion, today's exchange on the snapshot |
| 3h | donors alive after 3d; recipients establishing or bare | snapshot `Q⁴, W⁴` of donors, `W⁴` of recipients | `Q⁸ W⁸ P⁸ N⁸` of donors and recipients, `heat_out`, counters | propagules (§4.8) |

Every clamp is a `min`; no subtraction can go negative because each removal is
capped by the stock it reads.

### 4.1 Potential income (3a)

Gross assimilation this tick, the existing growth law without its logistic term:

```
A_pot = min( g · L_eff · μ · P⁻ · N⁻/(N⁻ + K_N) · drown · dt,  f_max · N⁻ · dt,  N⁻ )
```

with `L_eff` including the algae floor exactly as today (`fields::algae_light`).

### 4.2 Demands (3a)

```
M   = m_w · W⁻ · dt                                   maintenance (respired material)
D_P = min( (P_cap − P⁻)⁺ , r_p · W⁻ · dt )             foliage regrowth
D_W = min( (W_max − W⁻)⁺ , r_w · W⁻ · dt )             wood growth
D_Q = (Q_max − Q⁻)⁺                                   reserve refill
A   = min( A_pot , M + (1 + c_g)·(D_P + D_W) + D_Q )  income actually taken
```

`N¹ = N⁻ − A` (then adjusted below); `light_in += e_v · A`. A plant cannot turn
light into material it has nowhere to put; the cap replaces the old
`(1 − P/P_max)` term. If `P⁻ > P_cap` (painted cells) `D_P = 0` and nothing is
shed.

### 4.3 Maintenance, paid from income first, then reserve (3a)

```
paid_A = min(A, M);   rem = A − paid_A;   short = M − paid_A
paid_Q = min(Q⁻, short);   Q¹ = Q⁻ − paid_Q;   unpaid = short − paid_Q
N¹ += paid_A + paid_Q;   heat += e_v · (paid_A + paid_Q)
```

`unpaid` is carried to 3d.

### 4.4 Growth: reserve share first, then foliage, then wood, then the rest to reserve (3a)

Revised after the first implementation run (§18, third round): the original
order refilled the reserve only from what foliage and wood left, which under
foliage-first allocation is nothing, and it spent reserve on routine foliage
top-up whenever `P < P_cap`, which is always. A reserve that can never persist
is not a "finite budget for recovery". Two rules fix that: a fixed share of
surplus goes to the reserve before anything else, and reserve is spent on
foliage only in an emergency, below `p_reflush·P_cap`.

```
ΔQ_s = min( q_share · rem, D_Q );                       rem −= ΔQ_s;   Q¹ += ΔQ_s      reserve share off the top
ΔP_A = min( rem / (1 + c_g), D_P );                    rem −= (1 + c_g)·ΔP_A
ΔP_Q = 0  unless  P⁻ < p_reflush · P_cap, then
       ΔP_Q = min( Q¹ / (1 + c_g),  D_P − ΔP_A,  (p_reflush·P_cap − P⁻ − ΔP_A)⁺ );   Q¹ −= (1 + c_g)·ΔP_Q
P¹ = P⁻ + ΔP_A + ΔP_Q;   N¹ += c_g·(ΔP_A + ΔP_Q);   heat += e_v · c_g · (ΔP_A + ΔP_Q)
ΔW   = min( rem / (1 + c_g), D_W );                    rem −= (1 + c_g)·ΔW
W¹ = W⁻ + ΔW;   N¹ += c_g·ΔW;   heat += e_v · c_g · ΔW
ΔQ_r = min( rem, D_Q − ΔQ_s );   Q¹ += ΔQ_r;   rem −= ΔQ_r
N¹ += rem;   heat += e_v · rem            (rounding residue only; must be ~0)
```

Construction nutrient (`c_g·…`) is deposited in the growing cell's own `N`.
Reflush from `Q` is the "finite budget for recovery": after defoliation the
stand rebuilds leaves out of reserve at up to `r_p·W` per second, but only up
to `p_reflush·P_cap` and only until the reserve is gone; from there growth comes
from new income, which is proportional to the foliage it has left. In ordinary
life (`P ≥ p_reflush·P_cap`) the reserve is never touched for foliage, and
every tick with a surplus puts `q_share` of it into the reserve until
`Q = Q_max`; a full reserve takes no share and foliage gets everything again.

### 4.5 Senescence (3b)

`ΔP_s = m_p · P¹ · dt`: `P² = P¹ − ΔP_s`, `D² = D⁻ + ΔP_s`,
`De² = min(De⁻ + e_v·ΔP_s, e_d_max·D²)`, excess heat (today's mortality rule,
litter now explicitly leaf litter).

### 4.6 Dieback (3d)

`ΔW_die = min(W¹, κ · unpaid)`: `W⁴ = W¹ − ΔW_die`, `Wd⁴ = Wd⁻ + ΔW_die`.
Structure the plant could not maintain this tick dies; nothing else damages wood
in v1 (no wood-eaters, no trampling).

### 4.7 Death (3d)

If the cell was alive (`W⁻ ≥ W_min`) and `W⁴ < W_min`: `Wd⁴ += W⁴`;
`D⁴ += P³ + Q¹`; `De⁴ += min(e_v·(P³ + Q¹), e_d_max·D⁴ − De)`, excess heat;
`W⁴ = P⁴ = Q⁴ = 0`; `plant_deaths_total += 1`. Fruit stays and drops as
usual. Dead wood keeps its identity: it is not litter and no v1 animal eats it.

### 4.8 Propagules and establishment (3h)

A **donor** is a cell alive after 3d with `W⁴ ≥ W_est` and `Q⁴ > q_prop·Q_max`:
only a stand doing well spends on offspring. Its budget this tick is

```
B_j = min( Q⁴_j − q_prop·Q_max_j , k_est · dt · n_j )
```

where `n_j` is the number of its graph neighbours that are establishing or
bare. Take one immutable snapshot of every donor's `B_j` and every recipient's
`W⁴`. Each donor splits `B_j` equally among its `n_j` recipients (`s = B_j/n_j`
to each). For each recipient `i`, sum the incoming `s` over its donors into
`S_i`, then commit all transfers together:

```
donor:      Q⁸_j = Q⁴_j − Σ s_j→i
recipient:  net = S_i / (1 + c_g)
            W⁸_i = W⁴_i + w_frac·net;  P⁸_i = P⁴_i + p_frac·net;  Q⁸_i = Q⁴_i + q_frac·net
            N⁸_i += c_g·net;   heat += e_v · c_g · net
```

with `w_frac + p_frac + q_frac = 1`. A propagule is a viable package: wood,
starter foliage and starter reserve, so a cell that crosses `W_min` also has
leaves to earn income and a reserve to survive its first shortfall. The
recipient stays *establishing* (frozen, §3.1) until the tick on which
`W⁸_i ≥ W_min`; that tick `recolonisations_total += 1` and from the next tick the
cell is alive and runs 3a–3d. Donors stop sending to it because it is no longer
establishing or bare. Everything is paid from donor reserve; there is no
starter material or energy from outside.

A dead cell with no qualifying donor stays bare: local losses are allowed. A
newly established stand in a dim cell may fail to cover its maintenance and die
again (3d); its donors may then try again. The reserve floor `q_prop` bounds
what a stand can lose to such attempts.

Fruit ripening and drop are unchanged in rule (`P → F` where `P > fruit_min·P_max`;
`F → D` on drop with `e_f` carried into `De` under the cap). Ripening imports
`(e_f − e_v)·ΔF` from light (`fields.rs:146-148`), a second light-to-energy path
that the no-input test in §13.1 disables explicitly.

## 5. Detrital stocks, decomposition, transport

All decomposition is background and implicit-microbial: material returns to `N`,
the stock's energy leaves as heat in the same proportion, nothing is ever
recharged. Decomposition (3e) and downhill fall (3f) both draw on the **pre-tick
material** of a stock, and they share one withdrawal budget: fall acts only on
the portion decomposition left. Material that arrived this tick (deposits from
3a–3d, incoming fall) is eligible from the next tick on. Energy is withdrawn at
the stock's **current** density, so withdrawal never changes a stock's energy
density and the caps `De ≤ e_d_max·D`, `Ce ≤ e_c_max·C` survive without a
re-clamp. Per stock `X ∈ {D, C}` with energy `Xe`, rate `k_X`, and a downhill
neighbour where one exists:

```
dec_X  = k_X · dt · X⁻                                 material to N
fall_X = fall · dt · (1 − k_X·dt) · X⁻                 material to the downhill cell (0 with no downhill)
X⁵  = X⁴ − dec_X;            Xe⁵ = Xe⁴ · (1 − dec_X / X⁴)       (Xe⁵ = 0 when X⁴ = 0)
X⁶  = X⁵ − fall_X + Σ in;    Xe⁶ = Xe⁵ · (1 − fall_X / X⁵) + Σ in_e
heat += Xe⁴ · dec_X / X⁴;    N⁵ += dec_X
Wd⁵ = Wd⁴ − k_w · dt · Wd⁻;  heat += e_v · k_w · dt · Wd⁻;  N⁵ += k_w · dt · Wd⁻
```

For every ratio above, a zero denominator means a zero withdrawal ratio: the
corresponding withdrawal must also be zero. In particular, an empty `X⁵` has
zero outgoing energy and still receives `Σ in_e`. Never evaluate `0/0` in the
implementation, including the heat expression for an empty `X⁴`.

`dec_X + fall_X = X⁻ · (k_X·dt + fall·dt − k_X·dt·fall·dt) ≤ X⁻ ≤ X⁴` whenever
`k_X·dt ≤ 1` and `fall·dt ≤ 1`, which `validate` requires, so no stock can go
negative even at both endpoints together. `Wd` does not fall: a dead trunk stays
where it stood. Nutrient diffusion is unchanged. The synthesis asks for baseline
flows before changing locality or decomposition rates, so these three rates and
the fall rate are the only knobs and start at the values in §11.

| Stock | Rate | Material | Energy |
| --- | --- | --- | --- |
| litter `D` | `k_d` | `D → N` | current density × material → heat |
| remains `C` | `k_c` | `C → N` | current density × material → heat |
| dead wood `Wd` | `k_w` (slow) | `Wd → N` | `e_v` per unit → heat |

Sources into each stock, for the audit:

- `D`: senescence, fruit drop, feces from every mouth including hunters, a dead
  stand's `P + Q`, care Feed (unchanged: charged litter), the initial dark litter.
- `C`: ordinary death (`S + R`, energy `E + e_r·R` capped at `e_c_max·(S+R)`,
  excess heat), failed gestation (the escrow's material and energy, same cap),
  a hunter member's death. Predation moves a whole body into a gut as today and
  leaves no remains.
- `Wd`: dieback and stand death only.

Care Clean keeps exporting `D` only; `C` and `Wd` are untouched by care in v1.

## 6. Feeding: capability, permissions, intake, assimilation

### 6.1 Inherited capability

The genome keeps the single `diet` locus (0–1, mutable, same range and
mutation step). Its meaning becomes an allocation of one digestive investment
between foliage machinery and detrital machinery:

```
h = diet,  d = 1 − diet
cap_h = φ(h) if h ≥ θ else 0        foliage capability (leaf, fruit)
cap_d = φ(d) if d ≥ θ else 0        detrital capability (litter, remains)
φ(x) = x^γ,  φ(0) = 0, φ(1) = 1, monotone
```

`θ` is the minimum investment below which machinery is nonfunctional: the hard
exclusion. `γ` is the one curvature knob; `γ = 1` makes `φ(h) + φ(d) = 1`
everywhere (a generalist's total yield equals a specialist's, split), `γ > 1`
makes it less than 1 (breadth costs yield), which is the condition for
specialisation to pay. The baseline runs with `γ = 1` so the exclusion, not the
curve, is what the first scenarios test; `γ` is reserved for the later
specialist/generalist experiment the synthesis names.

Founder kinds under `θ = 0.2` (`config.rs` `FounderKind::defaults`):

| Kind | `diet` | `cap_h` | `cap_d` | Digests |
| --- | --- | --- | --- | --- |
| burrower | 0.10 | 0 | 0.90 | litter, remains |
| grazer | 0.85 | 0.85 | 0 | leaf, fruit |
| glider | 0.90 | 0.90 | 0 | leaf, fruit |
| skimmer | 0.60 | 0.60 | 0.40 | all four, at reduced yield |

Today's gates (`DIET_GATE` 0.05, `FRUIT_DIET` 0.5, `controller.rs:122-124`,
`neural/action.rs:34`) are replaced by `θ` for graze, fruit and scavenge alike;
the `Capability` masks stay world-side and identical for legacy and neural
control.

### 6.2 Permissions

| Food | Stock | Mouth channel | Needs | Yield factor |
| --- | --- | --- | --- | --- |
| leaf | `P` | graze | `cap_h > 0`, `mechanisms.grazing` | `cap_h` |
| fruit | `F` | fruit | `cap_h > 0`, `mechanisms.grazing` | `cap_h` |
| litter | `D` | scavenge | `cap_d > 0`, `mechanisms.scavenging` | `cap_d · min(1, ρ_D/e_r)` |
| remains | `C` | scavenge | `cap_d > 0`, `mechanisms.scavenging` | `cap_d · min(1, ρ_C/e_r)` |
| wood, dead wood | `W`, `Wd` | none | — | 0 |

Living animals are not food in v1 (the hunter extension is unchanged and
off). The scavenge channel serves `D` and `C` together; the world splits the
bite between them in proportion to `D_eff : C_eff` in the cell.

### 6.3 One mouth, one rate

`mouth_rate = k_mouth · mouth · size^0.75` per second as today
(`genome.rs:406`); `graze_rate` and `scavenge_rate` disappear. Every channel
bites at `mouth_rate`; the three efforts are normalised world-side so
`graze + fruit + scavenge ≤ 1`. The neural squash already does this
(`neural/action.rs:105-114`); the legacy controller can set all three efforts to
1 at once (`controller.rs:247-249`), so settlement applies the same rule to
every decision. The trade-off is therefore in yield, once, not in rate and
yield twice. For the founders at `γ = 1` the material yield per second on their
own food equals today's (`0.85 · k` either way); what changes is that the
bitten-off remainder becomes litter instead of staying on the plant.

### 6.4 Bite and settlement

Requested bite per channel, as today (`world/step.rs:1441`):

```
q = clamp( mouth_rate · effort · dt · X/(X + K_P), 0, headroom )
```

with `X` the cell's pre-settlement edible stock (`P`, `F`, `D_eff + C_eff`) and
headroom `R_max − R` consumed fruit-first, then graze, then scavenge. Per-cell
proportional sharing when requests exceed stock is unchanged.

Accounting of one served bite `q` on a food with density `ρ` and capability
`cap` (`cap_h` for leaf and fruit, `cap_d` for litter and remains):

```
stock  −= q;  stock energy −= ρ·q               (P, F: implicit e_v·q, e_f·q)
q_d = cap · q                                     digestible portion
η_m' = η_m (leaf, fruit)  or  η_m·min(1, ρ/e_r) (litter, remains)
reserve += η_m' · q_d
E += min( η_e · (ρ·q_d − e_r·η_m'·q_d), E_max − E );  the rest → heat
feces: D += (1 − η_m')·q_d + (1 − cap)·q         energy-free
heat  += ρ · (1 − cap) · q                         the undigestible portion's energy
```

For `cap = 1` this is exactly today's law. For `cap = 0` the channel is masked
and `q = 0`. A generalist spends the mouth on food it half digests; that is the
whole cost, and it is paid per bite, not per genome.

## 7. Animal maintenance, growth, reproduction (restated, unchanged mechanisms)

- **Upkeep** each tick: `maintenance·metabolism·S + sense_cost·r_sense`, plus the
  resolved motor bill; starvation is decided before settlement when
  `raisable_energy < upkeep` (`world/step.rs:1118-1165`). Unchanged.
- **Oxidation** `R → N` at `oxidation_rate` when `E < 0.5·E_max`, `η_ox` to `E`,
  rest heat. Unchanged.
- **Growth** while `S < S_adult` and `R > growth_reserve_min·R_max`: reserve
  becomes structure at `growth_rate`, paying `build_cost` energy and releasing
  `e_r` per unit as heat (`world/step.rs:1856-1876`). A juvenile therefore grows
  only from what it has eaten. Unchanged.
- **Reproduction**: conception escrows `S_child + R_child` material out of the
  parent's reserve and pays `build_cost·S_child + E_child` energy; gestation
  `t_gest`; birth places the child with exactly the escrow; a cap-refused birth
  refunds exactly; a parent's death during gestation sends the escrow to `C`
  (§5). The legacy controller's thresholds (`bud_reserve`, `bud_energy`,
  `bud_min_age`) and the neural `reproduce` channel decide *when*; the world
  decides *whether it can be paid*. Unchanged.

The contract adds no free term anywhere: no birth without a debited escrow, no
growth without reserve, no reserve without a served bite. Scenario B6a checks
that this closes on a finite food input.

## 8. Death and remains

Ordinary death (starvation, age, collapse) routes the body to `C`/`Ce`
(replacing `fields.d/de` at `world/step.rs:2011-2018`); failed gestation likewise
(`:2020-2032`). Hunter digestion rejects (`DigestStep.to_detritus`) are feces and
go to `D` energy-free; a hunter member's death goes to `C`. Predation itself is
unchanged.

## 9. Update ordering (normative tick order, M2 §"Tick order" amended)

1. Admit stimuli.
2. Weather, light, moisture, water.
3. Field reactions, subphases 3a–3h exactly as the §4.0 table: each reads the
   named state, cross-cell subphases (3f, 3g, 3h) snapshot then commit together.
4. Pair pass (unchanged).
5. Observe and decide (unchanged layout; `d_here` and the `D` food channels read
   `D_eff + C_eff`).
6. Motor resolution, movement, upkeep, starvation decision (unchanged).
7. Settlement per cell, proportional, pre-transfer stocks: fruit, graze, scavenge
   (scavenge split `D : C` by edible share), efforts normalised (§6.3–6.4).
8. Physiology: oxidation, growth, gestation, budding, death checks (unchanged).
9. Commit: dead bodies to `C`, births placed, ages advanced.
10. Invariants, view, telemetry.

Results never depend on organism or cell iteration order because every subphase
reads a state no other cell writes in that subphase, the three cross-cell
subphases work from snapshots with per-source caps, and 7 is proportional.

Every stock has one withdrawal budget per tick, listed so an implementer can
check it: `N`: income `A ≤ N⁻`, then diffusion on the 3g snapshot. `P`:
senescence from `P¹`, then ripening clamped to `P²`, then settlement on the
post-3 stock. `F`: drop from `F⁻`, then settlement. `W`: dieback `≤ W¹`. `Q`:
maintenance `≤ Q⁻`, then reflush `≤ Q¹`, then propagules `≤ Q⁴ − q_prop·Q_max`.
`D`, `C`: the joint 3e/3f rule of §5, then settlement on the post-3 edible
portion. `Wd`: decomposition `≤ Wd⁻`. Animal `R`, `E`: unchanged sequential
clamps in steps 6–8.

## 10. Conservation identities the audit sums

Material, closed world:

```
M = Σ_cells (N + P + F + W + Q + Wd + D + C) + Σ_organisms (S + R + escrow) + Σ hunter guts
  − external_material_in + clean_material_out
```

must equal the initial inventory to rounding every tick (`mass_residual`,
`world/invariants.rs:29`). Stored energy:

```
U = Σ_cells ( e_v·(P + W + Q + Wd) + e_f·F + De + Ce ) + Σ_organisms ( E + e_r·R + escrow energy ) + Σ gut energy
```

and `ΔU = light_in − heat_out + care energy in − clean energy out` over any
interval (`stored_energy`, `world/invariants.rs:117`). `light_in` has exactly
two entries: plant income (§4.2) and fruit ripening. In a test with both
disabled and no external input, `U` never rises. Every transfer named in §4–§8
books both sides.

## 11. Provisional parameters

All provisional. Kept values are today's defaults, listed so the table is
complete.

| Parameter | Symbol | Value | Status | Why this order of magnitude |
| --- | --- | --- | --- | --- |
| plant tissue energy density | `e_v` | 2 e/m | provisional | equals today's `e_p`; one density for all plant tissue |
| foliage per wood | `α` | 2 | provisional | `P_cap` does not bind at the steady state (arithmetic below); it limits painted stands and reflush |
| wood maximum | `W_max` | 0.6 m | provisional | reached only in bright cells; average cells settle near 0.33 |
| reserve per wood | `q_cap` | 0.5 | provisional | a leafless stand covers ~40 min of maintenance if it does not reflush |
| wood maintenance | `m_w` | 0.0002 /s | provisional | breakeven foliage after reflush `P ≈ 0.12` (average) / `0.07` (bright), so a reflushed stand can grow |
| foliage regrowth rate | `r_p` | 0.002 /s per m of `W` | provisional | reflush of a stripped stand from reserve in ~3.5 min; also the leaf-growth cap that sets `W*` |
| wood growth rate | `r_w` | 0.001 /s | provisional | e-folding of wood ~25 min in average light when income allows |
| construction respiration | `c_g` | 0.2 | provisional | growth is paid, not free |
| reserve share | `q_share` | 0.2 | provisional | a stand with a surplus fills its reserve in ~1,000 s (`0.2·rem ≈ 0.0003` bright, `0.00016` average) |
| reflush threshold | `p_reflush` | 0.25 | provisional | reserve is spent on foliage only below a quarter of the structural cap (`0.3` bright, `0.165` average), above the recovery breakeven in both classes |
| dieback per unpaid maintenance | `κ` | 1 | provisional | wood of a stand that cannot pay dies at `m_w`: e-fold ~80 min, so death of a large stand takes hours and is censored in every 30-min scenario (first implementation run) |
| alive threshold | `W_min` | 0.02 m | provisional | |
| donor threshold | `W_est` | 0.3 m | provisional | half-grown stands can seed |
| donor reserve floor | `q_prop` | 0.5 | provisional | only a stand with half its reserve spends on propagules |
| propagule rate | `k_est` | 0.0002 m/s per recipient | provisional | `W` gains `0.4·k_est/(1 + c_g) ≈ 6.7e-5 m/s` per donor: ~300 s to reach `W_min` beside one donor, ~40 s inside a ring of eight |
| propagule split | `w_frac, p_frac, q_frac` | 0.4, 0.4, 0.2 | provisional | at `W = W_min`: `P = 0.02`, `Q = 0.01` |
| capability gate | `θ` | 0.2 | provisional | excludes the 0.10–0.15 fallback the founders carry today |
| capability curvature | `γ` | 1 | provisional | neutral split; the later specialisation test's knob |
| litter decomposition | `k_d` | 0.002 /s | kept | |
| remains decomposition | `k_c` | 0.004 /s | provisional | a carcass is gone in ~4 min e-fold |
| dead wood decomposition | `k_w` | 0.0002 /s | provisional | ~1 h e-fold: bare trunks persist |
| remains energy cap | `e_c_max` | 2 e/m | provisional | equals `e_r`, as `e_d_max` does |
| initial wood | `W_0` | `0.5·W_max·L_0·μ_0`, set to 0 where below `W_min` | provisional | a cell below the threshold starts bare with `P_0 = Q_0 = 0` |
| initial foliage, reserve | `P_0`, `Q_0` | `0.4·P_cap0`, `0.5·Q_max0` in cells with `W_0 ≥ W_min` | provisional | `P_0` keeps today's `initial_fraction` |
| growth, mortality, Monod, uptake | `g`, `m_p`, `K_N`, `f_max` | 0.008, 0.001, 0.25, 0.5 | kept | |
| every animal constant | | | kept | tuned together later, per WORKING_POLICY |

**Arithmetic of the written model** (per second, provisional values, every cap
in §4.2 applied). Two reference cells are used throughout §13: **average**
`L = 0.5, μ = 0.7` (`L_eff·μ = 0.35`) and **bright** `L = 0.8, μ = 0.75`
(`0.6`), both with `N = 0.4` (Monod factor 0.615) and `drown = 1`. Income per
unit of foliage is `c = g·L_eff·μ·0.615`: 0.00172 (average), 0.00295 (bright).
Fruit ripening reads `L` alone.

Lone stand, ungrazed. Growth and sinks per second:

```
G(P, W) = min( (c·P − m_w·W)/(1 + c_g) ,  r_p·W )          foliage growth, both caps
S(P)    = m_p·P + ripen·P·(P/P_max − fruit_min)⁺·L          senescence + ripening
```

Foliage-first allocation means wood grows only while income exceeds what the
leaf cap `r_p·W` can absorb, so `W` rises until `(c·P* − m_w·W)/(1 + c_g) = r_p·W`
or `W_max`, and `P*` sits where `G = S`:

| cell | `P*` | `W*` | `Q_max` | `F*` | `G` at `P*` | senescence | ripening |
| --- | --- | --- | --- | --- | --- | --- | --- |
| average | ≈ 0.50 | ≈ 0.33 | 0.16 | ≈ 0.03 | 0.00065 | 0.00050 | 0.00013 |
| bright | ≈ 0.56 | 0.60 (`W_max`) | 0.30 | ≈ 0.16 | 0.00120 | 0.00056 | 0.00066 |

Neither cell reaches its structural cap `P_cap = α·W` (0.66, 1.2): at these
values foliage is limited by senescence and fruit ripening, both kept
parameters, not by structure. `α` and `P_max` matter only for painted stands and
during reflush. These are the "mature stand" values §13 paints, and B0 measures
how far the simulator's steady state is from this hand calculation.

Sustainable edible yield under grazing. A grazer that holds `P` below `P*`
harvests at most `Y(P) = G(P, W*) − m_p·P` (ripened fruit is still its food;
`drop·F` is a further loss only while fruit is left uneaten). `Y` rises with `P`
up to `P*`:

```
average:  Y_max ≈ 0.00065 − 0.00050 ≈ 0.00015 m/s
bright:   Y_max ≈ 0.00120 − 0.00056 ≈ 0.00064 m/s
```

Unit grazer (`size 1`, `mouth 1`, `diet 0.85`):

```
upkeep  0.005·1 + 0.0002·6                                  = 0.0062 e/s resting  (+0.0018 e/s at full cruise)
bite    q = 0.05 · 1 · 0.5/(0.5 + 0.45)                     = 0.026 m/s at P = 0.5 (when not headroom-limited)
per m bitten: digestible 0.85; reserve 0.6·0.85 = 0.51 m carrying 1.02 e;
              direct energy 0.5·(2·0.85 − 1.02) = 0.34 e; later oxidation 0.8·1.02 = 0.82 e
usable energy per m bitten                                  ≈ 1.16 e
foliage a grazer must bite to stay even                     = 0.0054 m/s resting, 0.0069 cruising
stands per resting grazer                                   ≈ 35 average, ≈ 8 bright  (43 / 11 cruising)
```

One grazer's bite is about 40× a bright stand's yield and 170× an average
one's. A single stand cannot support one grazer; a stand under a pinned grazer
is stripped, reflushes from reserve, is stripped again, and dies. This is the
expected result of B1a and B4a, not a failure of them. Coexistence is a question
of area, light and movement (B1b, B6b), and the ratios above are what those
scenarios measure.

Reserve. With `q_share = 0.2` of every tick's surplus, a bright stand at `P*`
(`rem ≈ 0.00153`) fills `Q_max = 0.3` in ≈ 1,000 s and an average one
(`rem ≈ 0.00079`) fills `0.16` in the same time; while filling, foliage gets
80 % of the surplus, so `P*` sits a few hundredths lower until the reserve is
full. A full reserve is what makes a stand a §4.8 donor.

Recovery after stripping. Reflush spends reserve at `r_p·W*` up to
`p_reflush·P_cap`: an average stand rebuilds ≈ 0.13 m of leaf (to its 0.165
ceiling) from `Q = 0.16` in ≈ 3.5 min, a bright one ≈ 0.25 m (ceiling 0.3)
from `Q = 0.3` in the same time. Income then has to cover senescence:
breakeven `c·P − m_w·W* = (1 + c_g)·m_p·P` gives `P ≈ 0.12` (average) and
`P ≈ 0.07` (bright). A reflushed average stand therefore sits near breakeven at
`P ≈ 0.13` and climbs back very slowly; a bright one at `P ≈ 0.25` reaches
`0.9·P*` in roughly ten minutes. B3 runs both and reports the average case
censored if that is what happens. Whether these time constants are acceptable
on the cube is for the later whole-ecosystem tuning, not for this slice.

## 12. Expected visible consequences and the presentation boundary

- A grazed stand drops through the presenter's stages as `P` falls and climbs
  back as it reflushes; nothing new is needed to see grazing and recovery.
- A stripped living stand shows the lowest foliage stage while `W > 0`; today's
  lowest stage is what the presenter draws for `P ≈ 0`, so "bare trunk" is only
  readable if that stage is not empty soil.
- Dead wood (`Wd > 0`, `W = 0`) has no appearance yet and reads as bare ground.
- Remains are drawn as today's detritus flecks (the presenter may sum `D + C`
  until a carcass look exists).

This milestone is headless and does **not** resolve Wrysk's visual observation
about vegetation. A separate, small presentation task follows it and must show
three things before the cube is judged: persistent living structure, foliage
loss and recovery on it, and dead wood. The RenderView carries every pool from
day one so that task needs no core change.

## 13. Acceptance scenarios

Two kinds, kept apart. **Accounting tests** pass or fail at machine precision
and gate the merge. **Ecological scenarios** report measurements with an
expected direction; a contradicted mechanism is a finding to report, not a
number to tune before review. All scenarios are headless and run **36,000 ticks
at most** (30 min simulated); a quantity not reached by the horizon is reported
as censored at the horizon, never by extending the run. Fixtures follow the
`food_stock_flow.rs` pattern: no founders, pinned or single mobile bodies, no
weather, no rain, no mutation, world stripped to the named cells. Light and
moisture are pinned per cell to one of the two §11 reference classes
(**average** `L = 0.5, μ = 0.7`; **bright** `L = 0.8, μ = 0.75`) with
`N = 0.4`. "Mature stand" means the §11 steady-state values of that class
painted at tick 0 (average: `P 0.50, W 0.33, Q 0.16, F 0.03`; bright:
`P 0.56, W 0.60, Q 0.30, F 0.16`) and booked as initial material; B0 measures
how far the simulator's own steady state is from those hand values, and every
later scenario reports its stands against B0, not against the hand table.
Scenario measurements come from the split `IntakeDiagnostics` (§14), which
record served material per stock, so the food actually consumed is known rather
than inferred from what was painted.

B0 is a finite-horizon baseline, not proof of equilibrium. Record its terminal
`P,W,Q,F` for each light class and use those measured values when constructing
later mature-stand fixtures, booking their material and energy. Report whether
the sampled trajectory is still changing; do not extend the horizon to make it
converge. The hand-calculated values above are predictions for comparison, not
replacement fixtures when B0 differs.

### 13.1 Accounting tests

| ID | Test | Passes when |
| --- | --- | --- |
| A1 | material identity across a sweep of configs that force income, reflush, dieback, death, establishment, decomposition, fall, including the A3b joint endpoints | `mass_residual` within 1e-9 every tick |
| A2a | no-input energy: `g = 0`, `ripen = 0`, care off, no organisms, no imports | `U` (§10) is non-increasing every tick and `light_in` stays 0 |
| A2b | full energy ledger: everything on, including ripening and care Feed/Clean | `ΔU = light_in − heat_out + care_in − clean_out` within 1e-9 every tick |
| A3a | inert fixture: `g = ripen = drop = m_p = k_d = k_c = k_w = m_w = r_p = r_w = k_est = fall = diffusion = 0`, `rain_rate = 0`, initial fruit and water zero, weather amplitude zero, no organisms, care off | every field vector is bit-identical before and after any number of ticks |
| A3b | rate validation and joint endpoints | `validate` refuses each of `m_w, m_p, ripen, drop, k_d, k_c, k_w, fall` with `rate·dt > 1` by name; with `k_d·dt = fall·dt = 1` on a stocked cell with a downhill neighbour, and likewise `k_c`, the stock ends exactly zero and nothing goes negative (§5 joint budget); the same at `m_p·dt = 1` with ripening on |
| A4 | capability decode | the four founder kinds decode to the table in §6.1; a bite on a masked food is refused as `q = 0`; a bite with `cap < 1` books `q_d`, feces and heat as §6.4 to 1e-12 |
| A5 | settlement | effort normalisation on a legacy decision with all three efforts at 1, `D : C` split, proportional sharing among several mouths, total removed equals total served |
| A6 | remains routing | ordinary death, miscarriage and hunter death land in `C`; hunter rejects land in `D`; decomposition of each stock leaks into no other stock |
| A7 | snapshot | schema 16 round-trips and its header is as documented; every schema-7…15 fixture is refused with `UnsupportedSchema` naming its version; a relabelled schema-16 payload is refused |
| A8 | determinism | two runs of each scenario give equal `state_hash` at the end |
| A9 | establishment arithmetic | one donor, one bare neighbour, `k_est·dt` per tick: the donor's reserve falls by exactly the sent amount, the recipient's `W, P, Q` rise by the split of `net`, construction nutrient lands in the recipient, the establishing cell's stocks are otherwise untouched, and `recolonisations_total` increments once on the crossing tick |

### 13.2 Ecological scenarios

| ID | Scenario | Fixture | Measure | Expected direction |
| --- | --- | --- | --- | --- |
| B0 | stand baseline | one lone stand per light class, seeded at `W_0` of §11 (not painted mature), no animals, 36,000 ticks | `P, W, Q, F`, income, senescence, ripening every 1,000 ticks; values at horizon | approaches the §11 table (average `P ≈ 0.5, W ≈ 0.33`; bright `P ≈ 0.56, W = 0.6`); the measured values become the "mature stand" every later scenario is judged against |
| B1a | one grazer, one stand | one pinned grazer on one bright mature stand, 12,000 ticks | income, leaf and fruit bitten, feces, `P`, `Q`, `W`, reserve, headroom-limited ticks | bite exceeds yield ~40× (§11); reserve saturates first; the stand is stripped, reflushes from `Q`, and dies; the grazer then starves. Coexistence is **not** expected |
| B1b | one grazer, a region | one mobile legacy grazer on a 5 × 5 mature region, 36,000 ticks; twice, bright and average | as B1a plus cells visited, region foliage total, minimum stand `P`, stand deaths | bright: 25 × 0.00064 ≈ 0.016 m/s against a need of 0.0069, coexistence with retained foliage is **expected**; average: 25 × 0.00015 ≈ 0.004 against 0.0069, the grazer is expected to run the region down. Both reported; a bright failure is a finding |
| B2 | depletion and relocation | one mobile legacy grazer, three bright mature stands three cells apart, 36,000 ticks | cells visited, time per stand, `P` minima, `Q` at departure, state on return | the grazer leaves a stand near the type-II floor and moves on; whether a stand recovers before return is reported, not assumed |
| B3 | plant recovery | one mature stand, `P` moved by fiat to `D` at tick 0 (booked as an internal transfer), no animals; twice, bright and average | time to `0.5·P*` and `0.9·P*` of the B0 value (censored if unreached), `Q` dip depth and refill, `N` drawdown | reflush from `Q` first (visible `Q` fall), then income-limited; bright reaches `0.9·P*` in ~10 min, average is expected to sit near breakeven and be reported censored (§11) |
| B4a | repeated stripping and death | three pinned grazers on one isolated bright mature stand | time to `Q = 0`, dieback onset, `W` decline rate, death tick (censored if unreached), `Wd` at horizon, grazers' fate | `Q → 0`, dieback opens, `W` declines at `κ·m_w` (e-fold ~80 min at §11, so death is expected censored), no regrowth (no donors), grazers starve |
| B4b | recovery after death | B4a's stand killed, grazers removed, living ring of eight bright mature stands around it, 36,000 ticks | tick of establishment, `W`, `P` at horizon as fractions of the B0 bright values | establishes within a minute (eight donors, §11); rebuilding is reported censored at 30 min |
| B5 | dietary exclusion | each founder kind pinned alone on (a) a foliage-only cell, (b) a charged litter-only cell, (c) a placed carcass (booked external material); conversions frozen: `m_p = ripen = drop = k_d = k_c = k_w = fall = 0` so a cell's food keeps its identity | survival time, reserve, served bites per stock | grazer and glider starve on (b), (c); burrower starves on (a); skimmer lives on all three at lower intake. Tests dependence, not desirability |
| B6a | reproduction on a finite input | closed 3 × 3 bright mature region with renewal off (`g = ripen = m_p = 0`), two mobile legacy grazers, 36,000 ticks | births, deaths, population, escrow debits per birth, `U` | births while the stock lasts, then starvation to zero; no birth without an escrow debit; `U` non-increasing |
| B6b | reproduction on a renewing patch | closed 7 × 7 bright mature region with renewal on, two mobile legacy grazers, 36,000 ticks | births, deaths, population, income vs total upkeep, doubling time vs stand recovery time | 49 × 0.00064 ≈ 0.031 m/s against 0.014 for two cruising grazers leaves a surplus, so some births are expected; whether they are followed by starvation is measured, not assumed; the ratio is reported |
| B7 | establishment | one bright mature donor beside one bare cell, no animals, 36,000 ticks; twice, with the bare cell bright and at `L_eff·μ = 0.2` | establishment tick, then `P`, `A`, `W` every 1,000 ticks | the bright cell establishes (~5 min with one donor, §11) and shows positive foliage and income that grow; the dim cell establishes and is expected to die back; both reported |

B1 and B6 are the coupled test the synthesis asks for: realised leaf consumption
against production, intake limits and population growth, with retained foliage
and recovery as the success criterion rather than persistent bare trunks.

## 14. Repository change map

| Area | File(s) | Change |
| --- | --- | --- |
| config | `crates/cubarium-core/src/config.rs` | `PlantConfig { alpha, wood_max, reserve_cap, maintenance, foliage_rate, wood_rate, build, dieback, alive_min, donor_min, donor_reserve_floor, propagule_rate, propagule_split: [f64; 3], energy_density, initial_wood, reserve_share, reflush_below }` (`plant.energy_density` is the one `e_v`; `producer.energy_density` no longer exists); `DetritusConfig` gains `carrion_decomposition`, `wood_decomposition`, `carrion_energy_cap`; `OrganismConfig` gains `capability_gate`, `capability_exponent`; `CONFIG_VERSION` 7 → 8; `validate` refuses `rate·dt > 1` for every per-second fraction rate by name, requires `alive_min < donor_min ≤ wood_max`, `propagule_split` summing to 1, `capability_gate ∈ [0, 0.5]`, `capability_exponent > 0`, `reserve_share ∈ [0, 1]`, `reflush_below ∈ [0, 1]` |
| fields | `crates/cubarium-core/src/fields.rs` | subphases 3a–3h (§4.0) with the three decompositions; `total_material`, `check`, initial seeding of `W_0, P_0, Q_0` (§11) |
| state | `crates/cubarium-core/src/world/state.rs` | trailing `EcologyV1State { wood, plant_reserve, dead_wood, carrion, carrion_energy, plant_deaths_total, recolonisations_total }`; `validate` |
| snapshot | `crates/cubarium-core/src/snapshot.rs` | `SCHEMA_VERSION` 16; `decode_snapshot` refuses every schema below 16 by name; no v15 mirror, no migration (§15.1) |
| lifecycle | `crates/cubarium-core/src/world/lifecycle.rs` | `new` seeds plants; `from_state` validates the extension is present and sized |
| step | `crates/cubarium-core/src/world/step.rs` | settlement (`:1441-1500`): shared rate, effort normalisation, `D : C` split, feces and undigestible heat; observation (`:420`, `:447`, `:2696`, `:2722`): `D_eff + C_eff`; commit (`:2011-2032`): remains to `C`; hunter reject routing |
| invariants | `crates/cubarium-core/src/world/invariants.rs` | `mass_residual`, `stored_energy`, per-stock `edible` |
| genome | `crates/cubarium-core/src/genome.rs` | `Phenotype`: drop `graze_rate`/`scavenge_rate`, add `cap_foliage`, `cap_detrital`; `decode` |
| controller | `crates/cubarium-core/src/controller.rs` | gates from `cap_* > 0`; feeding threshold per stock |
| neural | `crates/cubarium-core/src/neural/mod.rs`, `action.rs`, `state.rs` | `PROFILE_TEXT` gains `\|eco:v1` (§15.2); `Capability` from `cap_* > 0`; feedback `ate` normalised by `mouth_rate` |
| hunter | `crates/cubarium-core/src/hunter/*`, `world/hunter.rs` | routing only (§8) |
| care | `crates/cubarium-core/src/world/care.rs` | unchanged semantics, named in tests |
| view, telemetry | `crates/cubarium-core/src/view.rs`, `telemetry.rs`, `world/view.rs` | `RenderView` and `FieldDump` carry the new pools; `Telemetry` adds totals for `wood`, `plant_reserve`, `dead_wood`, `carrion`, `carrion_energy`, `bare_cells`, `establishing_cells`, `plant_deaths`, `recolonisations`; `IntakeDiagnostics` splits `detritus_eaten` into `litter_eaten` and `carrion_eaten` and adds `undigested`, `plant_income`, `plant_maintenance_unpaid`, `propagule_sent` |
| examples | `crates/cubarium-core/examples/ecology_v1_scenarios.rs` (new) | B1a–B7 in one binary with a subcommand per scenario, printing the §13.2 measurements, censoring at the horizon |
| tests | `crates/cubarium-core/tests/ecology_v1.rs` (new); updates to `accounting.rs`, `starvation.rs`, `population.rs`, `controller_rules.rs`, `redesign_rules.rs`, `neural_runtime.rs`, `snapshot_hardening.rs`, `continuation_fixtures.rs`, `*_migration.rs`, `energy_correction.rs` | A1–A9; old-schema fixtures become refusal tests, continuations retired (§15.1) |
| search | `crates/cubarium-search/src/es/fixture.rs` | `Layout::build` paints `W = P/α`, `Q = q_cap·W` with its `P` so a painted patch is a live stand; `painted_material` includes them; the layout hash and protocol hash change by construction. Optimizer, trainer loop and export format untouched |
| host | `crates/cubarium/src/runner`, `art_present` | read the wider `RenderView`; no art change in this slice |
| graph | `graft build` | after the change |

## 15. Compatibility consequences

### 15.1 Saved worlds: no migration

Wrysk's standing rule (2026-09-15): **always restart fresh, never migrate a
world.** Schema 16 therefore refuses every older snapshot by name
(`SnapshotError::UnsupportedSchema`) instead of synthesising wood for it. No
migration rule, no synthesised material, no subsidy booking. The frozen mirror
modules for schemas 7–15 stay in the tree only as long as their refusal tests
need them; nothing new is added to them.

Consequences for the existing tests: the schema-7…15 load-and-continue fixtures
(`crates/cubarium-core/tests/fixtures/*.cubw`) become refusal tests (the loader
names the old schema and stops); the `*-plus600-r0b` / `-r0d` continuation
comparisons are retired rather than re-anchored, with a note in their
provenance files. New continuation fixtures, if wanted, are generated from a
schema-16 world in a later assignment. The `ecology_hash` schema-7 projection
loses its purpose once no schema-7 world can load; keep it only if the
care/no-care comparison still uses it, otherwise retire it with the fixtures.
`state_hash` covers everything.

The live display starts a fresh world after this lands. Trainer and tool runs
likewise begin from fresh schema-16 worlds.

### 15.2 Trained policies: refused by name

Wrysk (2026-09-15): training was cheap, so old policies need not reload. The
contract therefore makes a hard break: append `|eco:v1` to `PROFILE_TEXT`
(`neural/mod.rs:33`) so the schema digest changes and every existing policy
file, every `runs/es-*` centre and the R3a display seed are refused by name.
The observation and action layouts themselves do not change (`d_here` becomes
`D_eff + C_eff`, still "edible detrital material here"), so the GRU, the
optimizer and the trainer loop are untouched. The runtime edits this slice does
make are enumerated: the digest text, the `Capability` masks (§6.1), the `ate`
feedback normalisation (§6.3) and the observation sampling of `D_eff + C_eff`.
No `runs/es-*` campaign is resumable, and the R2b/R2c/R2d results are not
evidence about the new ecology. Retrain from scratch after the §13 scenarios
pass.

### 15.3 Search and telemetry consumers

`params.rs` names stay valid; new plant parameters join the search vector only
in a later assignment. Telemetry readers that sum `detritus` as "all dead
matter" now need `detritus + carrion + dead_wood`. The viewer and captures do
not change.

## 16. Deferred, explicitly

Flesh capability and predator integration (a third machinery with high yield
on `C` and living prey, low on `D`), fruit specialists, wood-eating
detritivores, animal-carried seed transport, moisture- or temperature-scaled
decomposition, litter locality experiments, initial-litter sensitivity,
specialist/generalist competition under varying food (`γ`), propagule decay,
any parameter search or training, and the presentation task in §12. Each waits
for the baseline measured by §13.

## 17. Choices made

Wrysk decided on 2026-09-15: **always restart fresh, never migrate** (§15.1),
and old policies need not reload (§15.2). The remaining choices are routine
design calls made here, each with its plain-language effect, so the review can
veto rather than decide:

1. **Old trained policies are refused by name** (Wrysk, 2026-09-15: training
   was cheap, no need to reload them). The digest text gains `|eco:v1`; nothing
   else in the runtime's meaning moves. Retrain from scratch. No visible effect.
2. **New pools travel in a trailing snapshot extension**, the same pattern every
   earlier schema step used. Purely how a save is stored. No visible effect.
3. **Strict diets** (`θ = 0.2`): a grazer put on litter starves and a burrower
   put on leaves starves; today each could nibble a little of the other. The
   skimmer stays an omnivore. Visible effect: animals depend on their own food.
4. **One mouth**: an animal bites at one speed whatever it eats and is nourished
   only by what its body digests; an omnivore wastes part of every bite. This is
   the single trade-off, paid once. No direct visible effect.
5. **Paid recolonisation** (§4.8): neighbouring plants that are doing well send
   viable propagules into a dead spot; it re-establishes within minutes in good
   light and regrows over a long time. Without it a killed stand would be bare
   forever and the cube would slowly go bald.

## 18. Response to the 2026-09-15 review

| Finding | Fix |
| --- | --- |
| 1 (P1) recolonisation could not establish a growing plant | §4.8 rewritten: a propagule is a viable package split into wood, starter foliage and starter reserve (`w_frac, p_frac, q_frac`); establishing cells are a named class frozen below `W_min` (§3.1); donors need `W_est` and a reserve above `q_prop·Q_max`; one-time establishment counter; initial seeding no longer creates `P, Q` below the threshold (§11); A9 and B7 test establishment arithmetic and growth after it |
| 2 (P1) three different donor balances and unspecified subphase inputs | §4.0 is a subphase table naming what each reads and writes; cross-cell subphases snapshot and commit together with per-source caps; decomposition and fall read pre-tick stocks so new material is eligible next tick; construction nutrient lands in the growing cell; the contradictory sentences in §4.8 and §9 are removed |
| 3 (P2) scenarios could not test their claims | A2 split into no-input (`g = ripen = 0`) and full ledger; A3 split into a genuinely inert fixture and a validation-refusal test; B3/B4b use fractions and censoring inside the fixed 30-minute horizon; B5 freezes conversions and reads per-stock served bites; B6 split into finite-input (renewal off) and renewing-patch scenarios with no starvation assumption |
| 4 (P2) plant/grazer arithmetic | §11 recomputed through the whole chain (bite, digestible, reserve, oxidation, upkeep, senescence, construction): a single stand under a pinned grazer dies (the first-round "roughly a dozen stands" ignored the growth caps and is superseded by the second-round table below). B1 is split so B1a expects exactly that. `m_w` was lowered from 0.0005 to 0.0002 because the arithmetic showed a reflushed stand could not grow at all in average light, which contradicts the mechanism itself, not merely the expectation; the change is disclosed here |
| 5 (P2) handoff contradictions | the handoff no longer asks for a v15 mirror, enumerates the permitted runtime edits (§15.2), permits ES fixture painting while freezing the optimizer and trainer loop, asks for refusal coverage instead of re-anchored fixtures, and states its own scope without the removed skill reference |
| presentation boundary | §12 states that this milestone does not resolve the visual observation and names the follow-up presentation task |

Second round (Astra's first repair verification, same day):

| Finding | Fix |
| --- | --- |
| shared withdrawal budget for decomposition and fall | §5 rewritten as one joint rule: fall acts on the portion of the pre-tick stock that decomposition left, energy is withdrawn at the current density so the caps survive, and the sum of both withdrawals is bounded by the pre-tick stock at every admitted endpoint. §9 now lists every stock's single withdrawal budget. A3b and A1 test the joint endpoints |
| uncapped plant-yield arithmetic | §11 recomputed with both growth caps and the ripening sink: the model's ungrazed steady state is `P ≈ 0.5, W ≈ 0.33` (average) and `P ≈ 0.56, W = 0.6` (bright), well below `P_cap`; sustainable yield is ≈ 0.00015 / 0.00064 m/s per stand, so a resting grazer needs ≈ 35 average or ≈ 8 bright stands. "Mature stand" in §13 now means those steady-state values; a new B0 measures them in the simulator; B1b, B3, B6b and B7 name their light class and their expected outcome follows from the corrected numbers. No parameter changed |
| inert fixture completeness | A3a also zeroes `drop`, `rain_rate`, weather amplitude, initial fruit and initial water |
| self-audit for the same defect classes | the joint-budget check was applied to every stock (§9 list); the cap check to every §11 claim (reflush, breakeven, establishment time, wood e-folding), which were corrected where stale |

Third round (first implementation run, `b1dd394`, result note
[ecology-v1-implementation-2026-09-15](7_Research/ecology-v1-implementation-2026-09-15.md)):

| Finding | Fix |
| --- | --- |
| B0-1: the plant reserve never refills, so no simulator-grown stand is ever a donor and B3/B4b/B7 turn on a stock that cannot exist | contract defect in §4.4, two causes: refill came last and routine foliage top-up drew on reserve whenever `P < P_cap` (always). §4.4 rewritten: `q_share` of every surplus goes to reserve first; reserve is spent on foliage only below `p_reflush·P_cap`. Two provisional values added to §11, two config fields to §14. Repair cycle 1 |
| B4a: death censored because dieback at `κ = 1` e-folds wood in ~80 min | not a defect; the expectation was wrong. §11 and B4a now say death is expected censored and the `W` decline rate is the measurement. `κ` stays provisional for the later tuning |
| B7-1: establishment took 1,146 s not ~300 s because the donor's reserve drained into its own foliage | same root cause as B0-1; the §11 estimate stands once the reserve persists and is re-measured in the repair |
| interpretations 6 (`e_p` removed in favour of `e_v`), 7 (`Q_0` as a constant), 3, 4, 5, 8–11 | accepted as reported; `ProducerConfig.energy_density` is gone and `plant.energy_density` is the one density, recorded in §14 |

The bounded implementation handoff is
[ecology-v1-opus-2026-09-15](handoffs/ecology-v1-opus-2026-09-15.md). Training
resumes only after the §13 scenarios are verified in the revised ecology.
