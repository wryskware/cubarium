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

## Rerun after package J — 2026-09-17

Package J fixed the two specification defects above and reran the same two commands on
`main` at `318bddc`: fix 1 `387dab4` (aeration stress relaxes first-order toward
`target = clamp((f − establish_saturated_max) / (1 − establish_saturated_max), 0, 1)`
instead of ramping on a `stress`-independent increment), fix 2 `1e31136`
(`SpeciesConfig::seed_cohorts_max`, placeholder 4, merging a site's two oldest cohorts of
one species), fix 3 `318bddc` (`cubarium_voxel_flora::can_establish` — one germination
predicate, which the harness now calls for founder selection, the habitat sets and the
diagnosis instead of its own two).

| run | command | simulated | wall |
| --- | --- | --- | --- |
| comparison | `compare 2000 1 101 202 7` | 3 x 2,000 s | 1,687 s (28.1 min) |
| Chesson's probe | `chesson 1500 1500 1 101` | 2 x 3,000 s | 1,721 s (28.7 min) |

Both ran in parallel on separate cores; nothing else was running.

### The headline: there is a second generation, in every arm

```
life cycle per arm: base establishments 24 deaths 18; re-drawn noise 24 / 5; control 2 / 4
```

Against **0 establishments in every arm of every run** before. All of them are
umbrellafrond's, and they all happen in the same 100 s window, when the best seed bank in
the world finally crosses the germination threshold `alive_min / w_frac` = 0.05:

```
base (seed 1, noise 101)
  t   100 s: est 0, deaths  0; 8 bloomcrown (4 donors), stress 0.000, best bank  9.5%; 8 umbrellafrond (6 donors), stress 0.000, best bank 51.7%
  t   400 s: est 0, deaths  0; 8 bloomcrown (2 donors), stress 0.226, best bank 12.9%; 8 umbrellafrond (5 donors), stress 0.000, best bank 71.7%
  t   700 s: est 0, deaths  0; 8 bloomcrown (1 donors), stress 0.713, best bank 15.6%; 8 umbrellafrond (2 donors), stress 0.000, best bank 91.7%
  t  1000 s: est 0, deaths  2; 8 bloomcrown (2 donors), stress 0.724, best bank 11.6%; 6 umbrellafrond (5 donors), stress 0.000, best bank 87.5%
  t  1500 s: est 0, deaths  5; 8 bloomcrown (1 donors), stress 0.724, best bank 13.0%; 3 umbrellafrond (2 donors), stress 0.000, best bank 89.0%
  t  1700 s: est 0, deaths  5; 8 bloomcrown (0 donors), stress 0.724, best bank 13.7%; 3 umbrellafrond (0 donors), stress 0.000, best bank 99.7%
  t  1800 s: est 24, deaths 5; 8 bloomcrown (1 donors), stress 0.724, best bank 14.1%; 27 umbrellafrond (0 donors), stress 0.000, best bank 65.9%
  t  2000 s: est 24, deaths 18; 8 bloomcrown (2 donors), stress 0.724, best bank 14.7%; 14 umbrellafrond (0 donors), stress 0.000, best bank 58.2%

re-drawn noise (seed 1, noise 202)
  t  1700 s: est  0, deaths 5; 8 bloomcrown (0 donors), stress 0.615, best bank 15.3%; 3 umbrellafrond (2 donors), stress 0.000, best bank 95.9%
  t  1800 s: est 16, deaths 5; 8 bloomcrown (0 donors), stress 0.615, best bank 15.5%; 19 umbrellafrond (1 donors), stress 0.000, best bank 96.2%
  t  2000 s: est 24, deaths 5; 8 bloomcrown (2 donors), stress 0.615, best bank 16.1%; 27 umbrellafrond (1 donors), stress 0.000, best bank 62.0%

control (seed 7, noise 101)
  t  1700 s: est 0, deaths 4; 6 bloomcrown (1 donors), stress 0.798, best bank 18.8%; 6 umbrellafrond (4 donors), stress 0.000, best bank 97.9%
  t  1800 s: est 2, deaths 4; 6 bloomcrown (1 donors), stress 0.798, best bank 18.9%; 8 umbrellafrond (1 donors), stress 0.000, best bank 63.2%
  t  2000 s: est 2, deaths 4; 6 bloomcrown (1 donors), stress 0.798, best bank 19.1%; 8 umbrellafrond (3 donors), stress 0.000, best bank 70.8%
```

Read left to right, the chain of finding 1 is broken in three places:

1. **Umbrellafrond's aeration stress is 0.000 in every arm, at every reading.** It was
   0.876 at 100 s and 1.000 from 200 s on. Its `establish_saturated_max` is 1.0 and the
   stress target measures from the same number, so the wet producer has no saturation
   stress anywhere: a waterlogged hollow is its habitat and it earns in it.
2. **Bloomcrown's stress settles at an interior level and stays there** — 0.724 in the
   base arm from 800 s, 0.615 under the re-drawn noise, 0.798 in the control. It earns
   27.6 %, 38.5 % and 20.2 % of its income respectively instead of nothing, which is what
   "the basin costs the sun producer something" was supposed to mean. The levels differ by
   arm because the saturated fraction of the root boxes differs by arm — the thing they are
   supposed to measure.
3. **The banks are fed continuously, so they grow instead of ageing out.** Umbrellafrond's
   best bank climbs 51.7 % → 99.7 % of the threshold over 1,700 s and crosses it; it peaked
   at 27.8 % and fell to 0 before. Its donor count never goes to zero for long, because its
   income never does.

What the germination spends the bank on: 24 new stands in the base arm and the re-draw, 2
in the control. Then the base arm kills 13 of them inside 200 s (deaths 5 → 18): a cohort
of same-age saplings born into a hollow, of which the ones in the deepest water drown at
`drown_depth_m`. The re-draw's 27 all survived to 2,000 s.

### Descendant fractions, which are no longer zero

```
bloomcrown:     descendant stands / living stands: base 0/8 (0.00), re-drawn noise 0/8 (0.00), control 0/6 (0.00)
umbrellafrond:  descendant stands / living stands: base 12/14 (0.86), re-drawn noise 24/27 (0.89), control 2/8 (0.25)
```

Most living umbrellafronds at the end of a 2,000 s arm were born in it. Bloomcrown's
fraction is still 0.00, and the diagnosis says which gate is shut for it — the bank, not
the predicate, and not by a little:

```
base        bloomcrown: threshold 0.0500; 67 banked sites, mean 0.00567, biggest 0.00735 (14.7% of threshold); 0 over threshold; 26 of all 67 banked sites pass the predicate
re-draw     bloomcrown: threshold 0.0500; 67 banked sites, mean 0.00693, biggest 0.00805 (16.1% of threshold); 0 over threshold; 30 of all 67 banked sites pass the predicate
control     bloomcrown: threshold 0.0500; 38 banked sites, mean 0.00611, biggest 0.00954 (19.1% of threshold); 0 over threshold;  2 of all 38 banked sites pass the predicate
base     umbrellafrond: threshold 0.0500; 40 banked sites, mean 0.02267, biggest 0.02908 (58.2% of threshold); 12 of 40 banked sites pass the predicate
re-draw  umbrellafrond: threshold 0.0500; 40 banked sites, mean 0.02148, biggest 0.03098 (62.0% of threshold); 28 of 40 banked sites pass the predicate
control  umbrellafrond: threshold 0.0500; 43 banked sites, mean 0.02167, biggest 0.03538 (70.8% of threshold); 43 of 43 banked sites pass the predicate
```

(The umbrellafrond banks read low at 2,000 s because the germination at 1,700–1,800 s spent
the sites that were over the threshold; the timeline above is where the crossing shows.)

Bloomcrown's bank sits at a seventh of the threshold and does not climb: `hop` 2 splits one
donor's spendable reserve across 24 recipient columns against umbrellafrond's 8, and its
income is down by the interior stress on top. Whether that is right is a knob call —
`propagule_rate`, `hop`, `alive_min` — and nothing here was tuned.

### The comparison, on the one predicate

Founder columns and habitat sets moved when fix 3 deleted the harness's own predicate, so
none of the habitat numbers below is comparable to the pre-J section; they are the model's
own predicate now, in all three arms and in the diagnosis.

```
founder columns identical in all three: 16, off-predicate 0 in the re-draw and 6 in the control
landform: lowest skyline quartile  0.977 (re-drawn noise)  0.324 (another landform)
          highest skyline quartile 0.957                   0.515

bloomcrown     occupied 75 / 75 / 44 columns:      Jaccard noise 1.000   control 0.053
               founders excluded 67 / 67 / 38:             noise 1.000   control 0.000
               living stands only 8 / 8 / 6:               noise 1.000   control 0.750
               habitat 2526 / 2546 / 2540 of 3072:         noise 0.881   control 0.747
umbrellafrond  occupied 42 / 43 / 49 columns:      Jaccard noise 0.977   control 0.569
               founders excluded 40 / 40 / 43:             noise 1.000   control 0.627
               living stands only 14 / 27 / 8:             noise 0.519   control 0.048
               habitat 415 / 418 / 411 of 3072:            noise 0.983   control 0.168
               in the lowest skyline quartile: base 1.00, re-drawn noise 1.00, control 0.20
```

Umbrellafrond replicates round 2's finding on every line it has: re-draw the generator's
final weak noise and the habitat is the same 98 % of columns and the occupied set the same
98 %; change the landform and 17 % of the habitat and 57 % of the occupancy survive, with
the lowest-quartile fraction falling from 1.00 to 0.20. Its living-stand Jaccard of 0.519
against the re-draw is the one number that is genuinely about the plants rather than the
terrain, and it is low for a reason the run states: the two arms germinated 24 stands each
but not on the same columns, and the base arm then drowned 13 of them.

Bloomcrown's `occupied` is 75 columns of which 8 hold a stand — the rest are seed-bank
sites — so its 1.000 against the re-draw is the footprint of eight founders' `hop`-2
neighbourhoods, which is identical by construction. Its 0.053 against the control is the
founders' neighbourhoods landing elsewhere on another landform. Neither is evidence about
the terrain coupling. The habitat pair, 0.881 against 0.747, is weak evidence at best: the
set is 82 % of the world.

### Chesson's probe: both directions still fail, for a new reason

```
=== resident bloomcrown, newcomer umbrellafrond ===
  after 1500 s alone: 8 resident stands, seed bank on 91 sites, establishments 0, deaths 0
  one umbrellafrond founder at x3 z0 y12 (2526 habitat sites free of the resident)
  after 1500 s of invasion: newcomer 0 stands (0 descendants); resident 8 -> 8
  establishments 0 -> 0, deaths 0 -> 1; the founder's own site was vacated at some point
  INVASION FAILS

=== resident umbrellafrond, newcomer bloomcrown ===
  after 1500 s alone: 3 resident stands, seed bank on 48 sites, establishments 0, deaths 5
  one bloomcrown founder at x75 z15 y31 (289 habitat sites free of the resident)
  after 1500 s of invasion: newcomer 1 stands (0 descendants), seed bank on 24 sites holding 0.17026; resident 3 -> 1
  establishments 0 -> 24, deaths 5 -> 31; the founder's own site held its founder throughout
  INVASION FAILS
```

The residents are standing now — 8 bloomcrown through 3,000 s with no deaths at all, where
the pre-J probe's "resident" umbrellafrond had lost 6 of 8 founders before the newcomer
arrived. And the 24 establishments in direction 2 are real: they are the **resident**
umbrellafrond's, its bank crossing the threshold at the same ~1,700 s as in the comparison,
which is why its deaths jump to 31 as well.

One thing to remember when reading those counts: the resident's habitat is taken at the
warm-up (bloomcrown 2,526 columns, umbrellafrond 415, the same numbers the comparison
prints) but the newcomer's is recomputed on the post-fill world, and 1,500 s of the
harness's rain tap moves it a long way — umbrellafrond's free habitat is 2,526 sites there
against 415 at the warm-up, and bloomcrown's is 289 against 2,526. The two species' habitats
very nearly swap places as the world wets up, which is a fact about the harness's tap and
the rising table, not about the probe.

What still fails is the newcomer, in both directions, and the reason is now a measured
quantity rather than a broken rule: **one** founder needs about 1,700 s of uninterrupted
donation to push one recipient bank over the threshold, and the probe gives it 1,500 s. In
direction 1 it does not even get that — the single umbrellafrond founder placed in the
deepest free hollow drowns at `drown_depth_m` (deaths 0 → 1, its site vacated), and its bank
ages out behind it. In direction 2 the bloomcrown founder survives the whole probe and banks
0.170 of organic matter over 24 sites, a mean of 0.0071 against the threshold 0.05 — the
same seventh-of-a-threshold ceiling the comparison measured. The invasion criterion still
cannot be read: it needs the rare species to increase, and a single founder cannot in
1,500 s. A longer probe, or a founder cohort rather than one stand, is the next thing to
try, and both are the owner's call.

### The three residuals, still fine

```
base     ledger: fixed_in 30.844525 respired_out 21.870407 light_in 61.689050 heat_out 43.740814 transpired 0.302307 m3 deaths 18 establishments 24
         residuals: organic 2.567e-11 mineral -1.307e-12 energy 5.134e-11 (stocks: organic 25.7741 mineral 249.3360 energy 51.5482)
         core water: stored 312.6464 m3, residual 1.460e-7, transpiration_out 0.302307 m3 (flora says 0.302307)
re-draw  residuals: organic 2.778e-11 mineral -7.390e-13 energy 5.555e-11 (stocks: organic 26.4927 mineral 249.3360 energy 52.9854)
control  residuals: organic 5.329e-13 mineral  2.416e-12 energy 1.066e-12 (stocks: organic 22.9886 mineral 249.3360 energy 45.9772)
```

Relative: 1.0e-12, 5.2e-15, 1.0e-12 in the base arm, and the flora's `transpired_m3` still
agrees with the core's `transpiration_out` to the bit. The mineral stock is 249.3360 in all
three arms, as a closed stock should be. The runs are eleven times more productive than
package I's (fixed_in 30.8 against 2.7) and the residuals did not grow with them.

One cosmetic oddity, harmless and worth one look later: direction 1 of the probe printed
`seed bank on 0 sites holding -0.00000` for the newcomer. The site count uses `> 0.0` and
the sum does not, so the sum is over cohorts that are all at or below zero — a cohort kept
for its mineral after its organic matter went to zero. It is a print, not a stock: the
ledger residuals above are the check that matters.

### What changed and what did not

What changed is the *shape* of two rules, not any number. Aeration stress became a level
the root box sets rather than a ramp to a boundary, and that alone is what put a second
generation in the world: umbrellafrond stopped drowning in its own habitat, bloomcrown
settled at 0.62–0.80 instead of 1.00, both kept earning, their reserves kept crossing the
donor floor, and the seed banks they feed grew for 1,700 s instead of ageing out at 600 s
and reaching a quarter of the threshold. Establishments went from 0 in every arm to 24, 24
and 2; descendant fractions from 0.00 everywhere to 0.86, 0.89 and 0.25 for umbrellafrond;
and the occupancy Jaccards stopped being 1.000-or-0.000 artifacts of a frozen founder set,
so the comparison measures something again. The seed-cohort cap and the single predicate
changed no outcome in these runs — no site ever held five cohorts of one species here, and
the predicate change moved the founder columns and habitat sets but not the mechanism.

What did not change is bloomcrown's side of the experiment. Its bank still peaks at a
seventh of the germination threshold and it still has 0.00 descendants in every arm, for
reasons this rerun measures precisely: `hop` 2 splits one donor's propagule budget across
24 columns, and an interior stress of 0.6–0.8 takes two thirds of the income that would
refill the reserve. Chesson's probe still fails in both directions, because a single founder
cannot lift one recipient bank over the threshold inside 1,500 s. Neither is a rule that is
wrong; both are `propagule_rate`, `hop`, `alive_min` and probe-length calls, which are the
owner's, and nothing here was tuned.
