---
design_status: exploration
last_reviewed: 2026-09-17
decision_refs: []
---

# Voxel round 3: the rerun of the two-producer experiment

Package I, on `voxel-round3-i` from H's `a6ccc01`. Four runs of
`cargo run --release -p cubarium-voxel-flora --example two_producers`, all on the
generated 128x48x24 ring with the harness's `rain_m_per_s = 0.0005` tap and the outlet
open, 1,000 warm-up ticks before the founders go in.

| run | command | simulated | wall |
| --- | --- | --- | --- |
| comparison | `compare 2000 1 101 202 7` | 3 x 2,000 s | 1,710 s (28.5 min) |
| Chesson's probe | `chesson 1500 1500 1 101` | 2 x 3,000 s | 1,749 s (29 min) |
| mechanism | `2000 1 101` | 2,000 s | 573 s |
| longest single arm | `3000 1 101` | 3,000 s | 862 s |

## The headline: there is no second generation, at any length

**`establishments` is 0 in every arm of every run.** Deaths are not: 8, 6 and 4 in the
three comparison arms at 2,000 s, and 9 by 3,000 s on the base landform. The brief asked
for a length at which establishments > 0 and deaths > 0 in every arm. There is no such
length, and lengthening the run makes it worse rather than better, because the state the
world settles into has no seed banks in it at all.

The mechanism run prints both germination gates and both kill paths every 100 s. `stress`
is the mean `aeration_stress` over that species' living stands; `pool` is the mean
standing-water depth at their sites; "best bank" is the largest seed bank anywhere as a
percentage of the germination threshold `alive_min / w_frac` = 0.05.

```
  t   100 s: deaths 0; 8 bloomcrown (2 donors), stress 0.000, pool 0.000 m, best bank  9.5%; 8 umbrellafrond (1 donors), stress 0.876, pool 0.156 m, best bank 27.8%
  t   200 s: deaths 0; 8 bloomcrown (5 donors), stress 0.093, pool 0.000 m, best bank 10.4%; 8 umbrellafrond (0 donors), stress 1.000, pool 0.141 m, best bank 25.2%
  t   400 s: deaths 0; 8 bloomcrown (4 donors), stress 0.685, pool 0.000 m, best bank  9.6%; 8 umbrellafrond (0 donors), stress 1.000, pool 0.125 m, best bank 20.6%
  t   500 s: deaths 0; 8 bloomcrown (0 donors), stress 0.843, pool 0.000 m, best bank  8.7%; 8 umbrellafrond (0 donors), stress 1.000, pool 0.126 m, best bank 18.7%
  t   700 s: deaths 0; 8 bloomcrown (0 donors), stress 1.000, pool 0.000 m, best bank  7.1%; 8 umbrellafrond (0 donors), stress 1.000, pool 0.166 m, best bank 11.2%
  t   800 s: deaths 2; 8 bloomcrown (0 donors), stress 1.000, pool 0.000 m, best bank  6.4%; 6 umbrellafrond (0 donors), stress 1.000, pool 0.116 m, best bank  0.0%
  t  1200 s: deaths 4; 8 bloomcrown (0 donors), stress 1.000, pool 0.000 m, best bank  0.0%; 4 umbrellafrond (0 donors), stress 1.000, pool 0.208 m, best bank  0.0%
  t  2000 s: deaths 8; 8 bloomcrown (0 donors), stress 1.000, pool 0.000 m, best bank  0.0%; 0 umbrellafrond
```

Read left to right, that is one chain:

1. **Every stand in the world reaches `aeration_stress` 1.0 and stops earning.**
   Umbrellafrond by 200 s, bloomcrown by 700 s. Bloomcrown's mean standing-water depth is
   0.000 m the whole way, so this is waterlogged *soil* from the rising table, not a pool.
   Correction 2's income multiplier `(1 - stress)` then makes every stand's income zero.
2. **With no income there is no surplus, so no stand is a donor.** The donor count falls
   to 0 for umbrellafrond by 200 s and for bloomcrown by 500 s (`reserve > q_prop ·
   q_cap · W` can never be satisfied again once the reserve is being burned on
   maintenance). Wood then diebacks: at 3,000 s the seven surviving bloomcrowns hold
   1.598 of wood between them, a mean of 0.228 against the founder's 0.300, which is also
   below `donor_min` 0.3.
3. **The banks age out.** `seed_max_age_s` is 600 s, and the merge keeps a cohort young
   only while something is still feeding it. Nothing is, after 500 s, so every cohort
   falls to litter whole 600 s after its last arrival: umbrellafrond's banks are gone by
   800 s and bloomcrown's by 1,200 s. From then on there is nothing on any site that could
   germinate even if the predicate passed.
4. **The stands die.** All 8 umbrellafrond founders are dead by 2,000 s; one bloomcrown by
   2,800 s. Umbrellafrond's mean standing-water depth climbs from 0.126 m at 500 s to
   0.472 m at 1,900 s against a `drown_depth_m` of 0.5, so the last of them were about to
   drown by depth as well as starve.

The bank gate, not the predicate, is what was shut while banks still existed. At 200 s,
with the model's own `establishes` replicated in the harness:

```
germination gates:
      bloomcrown: threshold 0.0500; 182 banked sites, mean 0.00302, biggest 0.00519 (10.4% of threshold); 0 over threshold, 0 of those pass the model predicate; 156 of all 182 banked sites pass it
   umbrellafrond: threshold 0.0500; 58 banked sites, mean 0.00898, biggest 0.01260 (25.2% of threshold); 0 over threshold, 0 of those pass the model predicate; 58 of all 58 banked sites pass it
```

156 of 182 and 58 of 58 banked sites would germinate if the bank were big enough. It never
is: the biggest bank in the world peaks at about a quarter of the threshold and then falls.

## What that costs the comparison

Round 2's overlaps were read off `occupied`, which in round 2 included the frozen
`Establishing` stands. With no establishments and no surviving banks, `occupied` is now
**exactly the founder set**, and the founder columns are held identical in all three arms
by construction. So every occupancy Jaccard this round is 1.000 or 0.000 for reasons that
have nothing to do with the terrain:

```
bloomcrown:     occupied 8 / 8 / 8    Jaccard noise 1.000   control 1.000   founders excluded: 0 columns, n/a
umbrellafrond:  occupied 0 / 2 / 4    Jaccard noise 0.000   control 0.000   founders excluded: 0 columns, n/a
descendant stands / living stands: 0/8 (0.00) for bloomcrown in all three arms;
                                   0/0, 0/2, 0/4 for umbrellafrond
```

**The descendant fraction is 0.00 everywhere.** Every living stand at the end of every arm
is still its original founder, on the original founder column.

The two lines that do still measure something are the landform and the habitat — the
establishment predicate alone, on pure terrain and water, with no plant in the way:

```
landform: lowest skyline quartile  0.977 (re-drawn noise)  0.324 (another landform)
          highest skyline quartile 0.957                   0.515
bloomcrown    habitat 2821 / 2823 / 2840 of 3072 columns:  0.977   0.866
umbrellafrond habitat  291 /  291 /  288 of 3072 columns:  0.960   0.138
```

Umbrellafrond replicates round 2's qualitative finding sharply: re-draw the generator's
final weak noise and its habitat is the same 96 % of columns; change the landform and only
14 % survive. Bloomcrown's 0.977 against 0.866 is not evidence of much, because its
habitat is 92 % of the world and any two such sets overlap heavily.

One caveat on those numbers. The harness's founder-selection predicate (`passes`) is
**not** the model's germination predicate: it reads the support voxel's own pore fraction
and a binary saturation test, where `step.rs::establishes` reads the capacity-weighted
mean and the saturated *fraction* over the whole root box, which on a slope reaches
sideways into neighbouring columns. The habitat Jaccards above are the harness predicate's;
the `germination_diagnosis` block is the model's, replicated line for line in the example
so the two can be compared without exposing the private function.

## Chesson's probe: both directions fail, for the same reason

`chesson 1500 1500 1 101` — 1,500 s of the resident alone, then one founder of the
newcomer in the best site its own habitat still offers, then 1,500 s.

```
=== resident bloomcrown, newcomer umbrellafrond ===
  after 1500 s alone: 8 resident stands, seed bank on 0 sites, establishments 0, deaths 0
  one umbrellafrond founder at x3 z0 y12 (2156 habitat sites free of the resident)
  after 1500 s of invasion: newcomer 0 stands (0 descendants); resident 8 -> 7
  establishments 0 -> 0, deaths 0 -> 2; the founder's own site was vacated at some point
  INVASION FAILS

=== resident umbrellafrond, newcomer bloomcrown ===
  after 1500 s alone: 2 resident stands, seed bank on 0 sites, establishments 0, deaths 6
  one bloomcrown founder at x85 z19 y31 (658 habitat sites free of the resident)
  after 1500 s of invasion: newcomer 1 stands (0 descendants); resident 2 -> 0
  establishments 0 -> 0, deaths 6 -> 8; the founder's own site held its founder throughout
  INVASION FAILS
```

Neither newcomer's descendants establish, because **nothing's** descendants establish.
The probe cannot say anything about coexistence yet: the invasion criterion needs the rare
species to be able to increase, and in this world no species can increase, resident or
newcomer. Note the second direction: the umbrellafrond resident had already lost 6 of its
8 founders before the newcomer arrived, and lost the other 2 during the probe, so the
"resident" was not standing either. The one arm where the newcomer's own site was *vacated*
(direction 1) is the umbrellafrond founder dying in the hollow it was planted in.

## The three residuals, which are fine

3,000 s on the base landform, 60,000 coupled ticks:

```
ledger: fixed_in 2.719217 respired_out 13.093378 light_in 5.438433 heat_out 26.186755 transpired 0.162123 m3 deaths 9 establishments 0
residuals: organic 7.875e-12 mineral -7.958e-13 energy 1.575e-11 (stocks: organic 6.4258 mineral 256.3360 energy 12.8517)
core water: stored 358.7865 m3, residual 2.785e-7, transpiration_out 0.162123 m3 (flora says 0.162123)
```

Relative: 1.2e-12, 3.1e-15, 1.2e-12. The two currencies hold over the longest run anyone
has done on this crate, and the flora's `transpired_m3` still agrees with the core's
`transpiration_out` to the bit.

## What this says about the corrections

Corrections 1 and 3 are not what is holding the experiment up: the ledger is clean, banks
form where the model says they should, and 156 of 182 bank sites pass the germination
predicate. Two numbers are:

1. **`f*` = `relax_rate_per_s / (stress_rate_per_s + relax_rate_per_s)`** is 0.0909 for
   bloomcrown. Because the stress update's increment does not depend on `stress`, that is
   not a strength but a **threshold**: two saturated voxels out of an eighteen-voxel root
   box pin a bloomcrown at stress 1 forever, and under the harness's rain and rising water
   table nearly every root box in the world has two. Umbrellafrond's `f*` is 0.8333, which
   a wholly saturated box also clears, so it drowns too — 20 times more slowly and just as
   completely. `crates/cubarium-voxel-flora/tests/round3.rs` pins both, with
   `a_half_saturated_root_box_settles_at_an_interior_stress` left `#[ignore]`d as the
   FINDING.
2. **`propagule_rate` 2e-4 /s against the germination threshold `alive_min / w_frac`
   = 0.05, with `seed_attrition_per_s` 0.001 /s and `seed_max_age_s` 600 s.** Even before
   anything stresses, one donor's spendable reserve is split across its whole `hop`
   neighbourhood every tick — 24 recipient columns for bloomcrown at `hop` 2, 8 for
   umbrellafrond at `hop` 1 — and the biggest bank in the world peaks at 27.8 % of the
   threshold (umbrellafrond, 100 s) and 10.6 % (bloomcrown, 300 s) before the stress
   chain above takes the income away. The 600 s age limit then means the bank has to cross
   the threshold *while it is still being fed continuously*, which nothing manages.

Both are placeholders with backlog rows (`design/backlog.md` §1, rows 43 and 44). Nothing
here was tuned; the point of the rerun was to find out what the placeholders do, and this
is what they do.
