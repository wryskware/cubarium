---
design_status: leaning
last_reviewed: 2026-09-12
decision_refs: []
---

# Fauna v2: kinds, diets, depth, and rigs

With the cube stratified (`stratified-world.md`) and wet (`water.md`), the
fauna need to belong to places. Wrysk's brief: creatures should sense food
and move toward it, never spin aimlessly, and each band should have its own
inhabitants drawn with authored rigs in the Outrun palette. This note is the
design Fable chose. `leaning`, adopted for implementation under task
authorization; it supersedes the narrower M3a body grammar for the body, and
keeps M3a's mutation policy for the loci it names.

## Genome v2

Version 2 adds four heritable fields to v1; a v1 genome loads with the
defaults below and is upgraded in place.

| Field | Range | Default (v1 upgrade) | Effect |
| --- | --- | --- | --- |
| `diet` | 0–1 | 0.7 | Grazing rate `mouth_rate · diet`, scavenging rate `mouth_rate · (1 − diet)`; steering weight on `∇P` scales with `diet`, on `∇D_eff` with `1 − diet`; `can_graze` needs `diet ≥ 0.05`, `can_scavenge` needs `diet ≤ 0.95` |
| `depth` | 0–1 | 0.5 | Preferred height `h_pref = −1 + 2 · depth`; steering term `w_depth · (h_pref − h) · up`, where `up` is the unit chart direction of increasing embedded height (zero on the top face) |
| `swim` | 0–1 | 0.0 | Wading penalty becomes `speed / (1 + w · (1 − swim))`; a swimmer ignores pools |
| `form` | 0–(rigs − 1) | hue tercile | Which authored rig draws the body; heritable, copied exactly, never mutated in this slice |

`Drives` gains `w_depth` (0–2, default 1.0). Everything else in v1 is
unchanged. Specialization is a trade: total mouth rate is constant across
`diet`, so a generalist eats both foods at half rate.

## Founders come in kinds

`founders.kinds` in the world config is a list; the old `founders.count`
becomes the fallback when the list is empty. Each kind has a `count` and the
genome fields it fixes; unspecified fields take the v1 founder values. The
default list, one kind per band plus the water's edge:

| Kind | Band | `diet` | `depth` | `speed` | `size` | `swim` | Rig | `hue` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| burrower | soil | 0.10 | 0.10 | 0.6 | 1.0 (`metabolism` 0.7) | 0.0 | mossback | 0.15 |
| grazer | foliage | 0.85 | 0.55 | 1.0 | 1.0 | 0.0 | lantern | 0.50 |
| glider | canopy | 0.90 | 1.00 | 1.2 | 0.8 | 0.0 | sail | 0.85 |
| skimmer | the wet floor | 0.60 | 0.10 | 0.9 | 0.9 (`metabolism` 0.7) | 1.0 | skimmer (new rig) | 0.65 |

Counts default to 4, 10, 5, 5: the burrower colony overshoots its litter less from four; skimmers fizzled from three founders on two seeds in three, so five; grazers lost to gliders at six hours on the mid-wall crop twice, so the balance tilts toward them. Both floor kinds run cool at `metabolism` 0.7. Founders are placed uniformly by area as
before; the depth drive walks them to their bands within minutes, which is
itself worth watching.

## Mutation (M3a policy, narrowed)

With `mechanisms.mutation` on (default on): at each birth, with probability
`p_mut = 0.3` the child differs from the parent at 1–2 loci drawn from
{`size`, `speed`, `sense`, `reserve`, `mouth`, `hue`, `diet`, `depth`,
`swim`, `w_depth`}, each a Gaussian step of 0.08 of the locus range, clamped.
`form` never mutates: the look is the lineage's badge. Draws come from
`Stream::Birth` after the placement draws. The birth event records
`(locus, from, to)`.

## Fruit (a food the plants make)

Wrysk asked for flowers and fruit that some fauna eat. Fruit is a fifth
per-cell material pool `F` (m), conserved like the others:

| Process | Material | Energy | Rate |
| --- | --- | --- | --- |
| Ripening | `P → F` where `P > fruit_min · P_max` | `light_in += (e_f − e_p) · ΔF` (ripening spends light; `e_f = 3 e/m`) | `ripen · P · (P/P_max − fruit_min)⁺ · L`, `ripen = 0.02 /s`, `fruit_min = 0.3` (0.004 and 0.5 never fruited at the E2 standing crop). Steady state in a rich lit cell (`P ≈ 0.6`, `L ≈ 0.7`): `F ≈ ripen · P · (P/P_max − fruit_min) · L / drop ≈ 0.02 · 0.6 · 0.1 · 0.7 / 0.004 ≈ 0.2 m` |
| Drop | `F → D` | `De += e_f · ΔF`, clamped to `e_d_max · D`, excess heat | `drop = 0.004 /s` |
| Frugivory | `F → R` (η_m) and `F → D` (1 − η_m) | food energy `e_f · q`; reserve stores `e_r · η_m · q`; `E += η_e · (e_f − e_r · η_m) · q`; rest heat | same saturating `q` law as grazing with `F/(F + K_P)`; requested first, grazing takes the remaining headroom; only for `diet ≥ 0.5` |

Fruit is richer than leaf, so a grazer that finds a fruiting patch fills up
fast and then rests, which is the behavior the fruit stage on the plant
sprites is meant to show. `∇F` joins the steering with the grazing weight.
The render view gains `fruit: Vec<f64>`; the art presenter draws a plant's
`fruit` clip where `F` is high. The closed-box invariant adds `F` to the cell
sum; the energy audit adds `e_f · F`.

## Controller v2

Steering:
`s = diet · w_food · h · (∇P + ∇F) + (1 − diet) · w_detritus · h · ∇D_eff + w_crowd · repulsion + w_depth · (h_pref − h) · up + w_persist · ou`.

Sensing reaches further than the four neighbors: the gradients are taken
over graph cells within BFS depth `ceil(r_sense / 4)` of the own cell, each
neighbor's `(value − value_own) · dir / dist` weighted by `1 / dist`, so a
creature with 6 px of sense sees two cells out and heads for the richer side
of a patch rather than dithering on its edge.

Turning is gated by mode: with `k = 1` while Seeking, `k = 0.1` while
Feeding and `k = 0` while Resting (`organism.feed_turn_fraction`,
`organism.rest_turn_fraction`), the noise update is
`ou' = ou · (1 − dt/τ_ou) + noise · turn_noise · √dt · k` and the heading
turns toward `s` by at most `turn_rate_max · dt · k`. A resting body is
still; a feeding body reorients slowly. (The parked stash `mode-gated
turning` has a working implementation and tests to draw from.)

Everything else in the M2 controller stands.

## Render view and presentation

`OrganismView` gains `form: u8`. The art presenter chooses the rig by
`form`, not by the hue tercile; the hue tercile remains the fallback for a
`form` beyond the pack. Telemetry gains `population_by_form`.

## What this slice must show

A fresh default world at 8× in the art viewer, within ten simulated minutes:
burrowers working the soil floor's detritus, grazers in the foliage,
gliders on the canopy, skimmers at the pools; no body turning in place while
it rests or feeds; bodies visibly heading toward food. Short runs (two
simulated hours, three seeds) showing all four kinds alive with their mean
heights in their bands.

## Deferred

Predation, dormancy, mutation of `form`, and long-run lineage experiments
(E4–E6), which should run only once Wrysk has seen and accepted the picture.
