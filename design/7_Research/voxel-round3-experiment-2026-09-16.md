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
ledger residuals above are the check that matters. (**Corrected after package K:** it is
not a cohort at all. Rust's `Sum` for `f64` folds from `-0.0`, so an **empty** sum prints
as `-0.00000`. Nothing was negative.)

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

## Rerun after package K — 2026-09-17

Package K answered Astra's round-4 findings on `main`: `6accd14` K1 (arrival bins),
`8497068` K2 (the harness water budget and its tap), `ea6f15e` K3 (funded reproductive
parcels), `8e3931a` K4 (the gap lottery and one-package germination), `619f9a5` K5
(lineage by identity), `1c0c6c9` K6 (documentation), plus `687c54e` and `398af32`. The
binary that produced the numbers below was built at K5; the two later commits change no
number at the placeholders (`687c54e` rounds the bin width up, and 600 s over 4 bins is
3,000 ticks either way; `398af32` reorders two lines on an unreachable branch).

**Chesson's probe was deliberately not rerun.** R4.7 asks for a positive control to be
designed first; the open items at the end of this section say what it needs.

| run | command | simulated | wall |
| --- | --- | --- | --- |
| comparison | `compare 2000 1 101 202 7` | 3 x 2,000 s | 1,465 s (24.4 min) |

Four rules changed, so nothing here is comparable to package J's numbers except as a
before-and-after of the rules themselves:

1. A seed cohort is an **arrival bin** whose age never decreases, so a fed bank ages out
   on its own schedule instead of being held young by every fresh landing (R4.1).
2. A donor **saves** one recipient's worth per tick into `Stand::parcel` and sends one
   whole package — `alive_min / w_frac` = 0.05 net, 0.06 of reserve — to **one** drawn
   recipient, instead of paying `rate · dt` to every recipient in `hop` at once (R4.4).
3. A bare site draws its occupant by a **lottery weighted by whole packages**, and the
   winner spends exactly one package out of its oldest bins (R4.5).
4. The harness rain is **2e-4 m/s**, a nominal 0.0384 m³/s against the outlet's 0.05,
   where the old tap was 0.096 m³/s against the same outlet (R4.2).

### The water budget, which is what R4.2 asked for

The harness now prints in, out and storage every 100 s and again over the whole run. The
whole-run lines:

```
base      water m3/s over 2000 s: rain in 0.038400, outlet 0.036435, evaporation 0.000000,
          transpiration 0.00019437, storage change +0.001771 (residual -6.97e-11);
          head 2.502 m (-0.4846 m), stored 221.6132 m3
re-draw   rain in 0.038400, outlet 0.036230, transpiration 0.00019073, storage +0.001980;
          head 2.507 m (-0.4778 m), stored 222.0313 m3
control   rain in 0.038400, outlet 0.035970, transpiration 0.00009535, storage +0.002335;
          head 2.502 m (-0.4844 m), stored 222.7415 m3
```

**The head is bounded and stationary**, at 2.50 m in all three arms, and the run gets
there by *draining*: the metre of initial charge above the basin floor comes off in the
first 900 s at about 0.05 m per 100 s, and after that the head moves by +0.0001 m per
interval. The interval lines show the mechanism (base arm):

```
  t   100 s: rain in 0.038400, outlet 0.050000, storage change -0.011741; head 2.922 m (-0.0647 m)
  t   800 s: rain in 0.038400, outlet 0.050000, storage change -0.011804; head 2.532 m (-0.0526 m)
  t  1000 s: rain in 0.038400, outlet 0.017494, storage change +0.020706; head 2.501 m (+0.0001 m)
  t  1500 s: rain in 0.038400, outlet 0.023364, storage change +0.014838; head 2.501 m (+0.0001 m)
  t  2000 s: rain in 0.038400, outlet 0.036435, storage change +0.001771 (whole-run mean)
```

While there is standing water to export the outlet runs at its full 0.05 m³/s and storage
falls; once the head reaches 2.50 m the outlet cell is undersupplied and export drops to
0.017 m³/s, whereupon storage rises again and the export climbs back toward the input. By
2,000 s the arms are within 0.002 m³/s of balance and the whole-run storage change is
+0.0018 to +0.0023 m³/s, against **+0.046 m³/s or more** under the old tap. So bounding
nominal rain below the outlet's capacity bounds the head, and — exactly as R4.2 warned —
it does not by itself guarantee balance, because the outlet exports less than its capacity
when its own cell is dry. The arms are held on terrain, initial water (`basin floor + 1 m`
of charge), initial mineral, forcing and observation phase, and the budget is printed per
arm so a reader can check that rather than take it.

The core's own conservation residual over each interval is 1e-11 or smaller, and its
whole-run residual is 1.3–1.4e-7 m³ on 222 m³ stored (6e-10 relative), with the flora's
`transpired_m3` equal to the core's `transpiration_out` to the bit in every arm.

### Establishments, deaths and descendants by identity

```
life cycle per arm: base establishments 4 deaths 0; re-drawn noise 2 / 0; control 3 / 1

bloomcrown      descendant stands / living stands: base 0/8 (0.00), re-draw 0/8 (0.00), control 0/7 (0.00)
umbrellafrond   descendant stands / living stands: base 4/12 (0.33), re-draw 2/10 (0.20), control 3/11 (0.27)
```

These fractions are counted by `Stand::id` — the ledger's birth counter — and not by the
site watch, so they are exact lineage counts rather than the "not certified" ones R4.7
objected to. Package J's fractions were 0.86, 0.89 and 0.25 for umbrellafrond, from 24
establishments per arm; recruitment is an order of magnitude rarer here, and both changes
that made it so are intended:

- **A birth needs two packages on one site.** A package is exactly the minimum viable
  stand's material, and (in these arms) a bin paid attrition before germination was
  tested, so one package alone was short for ever — by `seed_attrition_per_s · dt` =
  `0.001 × 0.05` = **0.005 %** of itself, not the 0.1 % an earlier draft of this section
  said (Astra R5.3). The diagnosis shows every arm ending with banks just
  under the threshold — biggest 92.5 %, 93.6 % and 81.9 % of it for bloomcrown, 81.9 %,
  81.9 % and 94.5 % for umbrellafrond — which is one package minus its attrition, waiting
  for a second that has to arrive before the first bin ages out.
- **Packages scatter.** A donor sends one package to one drawn site per 300 s of funded
  output, so with umbrellafrond's `hop` 1 (8 candidate columns) two packages coincide on
  one site fairly often, and with bloomcrown's `hop` 2 (24 candidates) they rarely do.
  That, and not the predicate, is why bloomcrown still has no descendants: the diagnosis
  reports `0 of all 3 banked sites pass it` in the base arm — its banks are on sites its
  own predicate refuses — while umbrellafrond's read `12 of all 13 banked sites pass it`.

### Requested, funded and landed, which is what R4.4 asked for

Cumulative organic matter, net of construction, per species per arm:

```
base      bloomcrown:    requested 2.66667, funded 1.55823 (58.4 %), landed 1.45000 (29.0 packages); 0.10823 in parcels on 8 stands
          umbrellafrond: requested 2.66667, funded 2.66667 (100.0 %), landed 2.40000 (48.0 packages); 0.26667 in parcels on 12 stands
re-draw   bloomcrown:    requested 2.66667, funded 1.45171 (54.4 %), landed 1.35000 (27.0 packages); 0.10171 in parcels on 8 stands
          umbrellafrond: requested 2.66667, funded 2.66667 (100.0 %), landed 2.40000 (48.0 packages); 0.26667 in parcels on 10 stands
control   bloomcrown:    requested 2.23948, funded 1.53257 (68.4 %), landed 1.35000 (27.0 packages); 0.18257 in parcels on 7 stands
          umbrellafrond: requested 2.66667, funded 1.46663 (55.0 %), landed 1.20000 (24.0 packages); 0.26663 in parcels on 11 stands
```

This is the distinction the old rule could not make. **Bloomcrown is funding-limited and
umbrellafrond is not**: on the base landform the sun producer can pay for 58 % of what its
rate asks for, because its interior aeration stress (0.85 at 1,000 s) takes most of the
income that would refill the reserve above the donor floor, and its donor count falls from
8 to 1 over the run while umbrellafrond's stays at 8. On the control landform the two
swap — bloomcrown 68 % and umbrellafrond 55 % — which is the terrain, not the species.
Raising `propagule_rate` would move `requested` and nothing else in the columns that
matter, which is precisely R4.4's point; nothing was tuned.

`funded − landed` is the material standing in parcels, and it matches the per-stand parcels
exactly: 8 bloomcrown stands hold 0.10823 after 29 packages, which is 8 x 0.0135 of a
0.05 package each.

### The comparison itself

```
founder columns identical in all three: 16, off-predicate 1 in the re-draw and 13 in the control
landform: lowest skyline quartile  0.977 (re-drawn noise)  0.324 (another landform)
          highest skyline quartile 0.957                   0.515

bloomcrown     occupied 11 / 10 / 14 columns:      Jaccard noise 0.750   control 0.389
               living stands only 8 / 8 / 7:              noise 1.000   control 0.875
               eligible AT INTRODUCTION 96 / 99 / 95 of 3072: noise 0.822   control 0.016
               in the lowest skyline quartile: base 1.00, re-draw 1.00, control 0.36
umbrellafrond  occupied 22 / 22 / 20 columns:      Jaccard noise 0.692   control 0.400
               living stands only 12 / 10 / 11:           noise 0.833   control 0.643
               eligible AT INTRODUCTION 386 / 390 / 383 of 3072: noise 0.970   control 0.172
               in the lowest skyline quartile: base 1.00, re-draw 1.00, control 0.50
```

Every eligible count in this section — 96, 99, 95, 386, 390, 383 — was read **at
introduction**: after the 1,000-tick (50 s) warm-up and before a plant acted. None of them
is a habitat size. The water budget above shows the head still falling at 50 s and settling
only around 900 s, so these are readings of one early moment under one initial wetting, and
the arms' eligible sets differ in initial wetting as well as in terrain (Astra R5.2). The
harness now samples the same predicate again at observation and prints both; the section
below is what that shows.

Two things are worth reading here and one is worth not reading.

**The introduction-time eligible sets replicate, and the landform explains them.**
Umbrellafrond's is the same 97 % of columns under a re-drawn noise and 17 % under another
landform, which is round 2's finding again, and its control value of 0.172 against the
noise pair's 0.970 is terrain and not wetting. Bloomcrown's was **96 columns of 3,072** at
50 s here against 2,526 under the old rising tap — but that is two different early wettings
compared with each other, not a niche that shrank: see the next section, where the same
predicate at observation reads 2,688. Its 0.822 / 0.016 pair is a real measurement of the
50 s reading and nothing more.

**The occupancy Jaccards are dispersal draws as well as terrain.** 0.750 and 0.692 against
the re-draw, where package J read 1.000 and 0.977, because the occupied set is a handful of
drawn landing sites rather than every column in every donor's `hop`. This is **not**
independent replicated dispersal evidence (Astra R5.3): the model is deterministic, so an
identical initial state, seed, founder set and tick count reproduces a footprint exactly —
but a noise reseed moves support faces by a voxel, and a donor's draw key is
`(domain, world seed, donor voxel index, tick)`, so the re-draw arm changes terrain **and**
every donor's delivery sequence at once. The two effects are not separated here, and the
difference between those arms cannot be attributed to either.

**What not to read:** the per-arm establishment counts (4 / 2 / 3) are three samples of a
stochastic process with no replication, and the control arm's 13 off-predicate founders
make its arm a different treatment as well as a different landform. Nothing here is
evidence about coexistence, and the probe that would be was not run.

### The eligible band is a reading of a moment (R5.2's short diagnostic)

Astra's R5.2 asked for a short diagnostic over an available state rather than another long
comparison, using the model's own gates instead of a second approximate predicate. The
harness now samples `can_establish` at **observation** as well as at introduction, and
prints a per-gate breakdown over the final state through
`cubarium_voxel_flora::establishment_gates`, whose `passes()` **is** `can_establish`. A
400 s single run at the same forcing (`two_producers 400 1 101`: the same terrain and tap
as the base arm, stopped early on purpose):

```
 umbrellafrond: 386 of 3072 skyline sites pass its establishment predicate at introduction
    bloomcrown:  96 of 3072 skyline sites pass its establishment predicate at introduction

establishment gates at observation (3072 skyline columns):
      bloomcrown: 2688 eligible; shut gates (a column can fail several): no soil in the
                  root box 0, mean pore < 0.10 0, saturated fraction > 0.25 384, water
                  over 0.05 m 55, sky < 0.60 0
                : sole cause - pore alone 0, saturation alone 329, light alone 0; mean
                  root-box pore over the 3072 columns with soil 0.386
                : 8 donors, 182 candidate faces within hop 2, 85 of them eligible (10.6 each)
   umbrellafrond: 668 eligible; shut gates: no soil 0, mean pore < 0.45 2404, saturated
                  fraction > 1.00 0, water over 0.50 m 0, sky < 0.10 0
                : sole cause - pore alone 2404, saturation alone 0, light alone 0; mean
                  root-box pore over the 3072 columns with soil 0.401
                : 8 donors, 61 candidate faces within hop 1, 61 of them eligible (7.6 each)
```

**Bloomcrown's 96 columns was the wetting at 50 s and not a niche size.** On the same
terrain, after **400 coupled seconds** — the harness warms the world up for 50 s and then
runs the duration it was asked for, so this reading is of a 450-second-old world and not of
one 350 s on from the first reading (Astra R6.3) — the same predicate admits **2,688 of
3,072** columns. The soil comes down off saturation as the initial charge drains, and what
shuts the remaining 384 is the saturation ceiling (329 of them for that reason alone) plus
55 columns under standing water. At observation not one column fails on mean pore and not
one on light. That is a statement about **those two gates at 450 s** and nothing else: the
96-column reading at introduction was not broken down by gate, so this does not identify
what held bloomcrown to 96 (Astra R6.3). Umbrellafrond moves the other way and for the
opposite reason, 386 → 668, with 2,404 columns refused on **mean pore alone**, because its
`establish_pore_min` of 0.45 sits above the mean root-box pore fraction over its own boxes
on this skyline, 0.40 — which is this draining arm's reading and not drained soil's
equilibrium, which is `field_capacity` 0.25 (Astra R7.5).

Two things this does **not** say. It is not a settled-habitat measurement either — the head
is still falling at 400 s, −0.24 m over the run with the outlet at its full 0.05 m³/s — and
a count of eligible columns is not a count of reachable ones: what a donor can recruit on is
the eligible faces inside its own `hop`, 85 of 182 for the eight bloomcrown donors and 61 of
61 for the eight umbrellafrond donors. And it says nothing about which germination gate was
shut in the earlier 2,000 s arms, which reported refused bloomcrown banks: an
eligible-neighbour count taken here, on a world of a different age under a different
recruitment rule, cannot dismiss the predicate there. Those arms' own diagnosis is the only
evidence about them (Astra R6.3).


### The three residuals, still fine

```
base     ledger: fixed_in 36.019642 respired_out 21.126060 light_in 72.039284 heat_out 42.252121 transpired 0.388746 m3 deaths 0 establishments 4
         residuals: organic -8.962e-11 mineral -4.462e-12 energy -1.792e-10 (stocks: organic 31.6936 mineral 76.3360 energy 63.3872)
re-draw  residuals: organic -8.573e-11 mineral -4.576e-12 energy -1.715e-10 (stocks: organic 31.1964 mineral 77.3360 energy 62.3928)
control  residuals: organic -3.011e-11 mineral -1.258e-12 energy -6.021e-11 (stocks: organic 21.8891 mineral 59.3360 energy 43.7782)
```

Relative: 2.8e-12, 5.8e-14 and 2.8e-12 in the base arm, with the parcel counted in the
organic and energy stocks through `Stand::material` and its mineral counted in the stand.
The mineral stock differs by arm now — 76.336, 77.336 and 59.336 against a flat 249.336 in
package J — because `initial_mineral` is provisioned lazily per site and the arms colonize
far fewer sites when a donor sends one package at a time. That is the accounting R4.3 asked
to have stated: colonization imports booked mineral, so the total is a function of how far
the plants spread, and a fertility comparison wants a fixed per-site inventory instead.

One correction to the record: the `seed bank on 0 sites holding -0.00000` print noted after
package J is not a cohort at or below zero. Rust's `Sum` for `f64` folds from **-0.0**, so
an empty sum prints as `-0.00000`; the same `-0.0` reaches the progress line's
`best bank -0.0% of threshold` through `f64::max`. It is a print artifact of an empty bank
and nothing is negative anywhere.

### Open items

1. **The probe's positive control (R4.7), owner's call on the design.** What it needs, as
   far as this rerun can say: the same founder treatment — one founder, or a declared small
   cohort — planted alone under this same bounded forcing, run until it has replaced itself
   *beyond the initial reserve subsidy*, with the generation time measured rather than
   assumed. One package is 300 s of a fully funded donor's output, and a single funded
   umbrellafrond donor with 8 candidate sites took 1,300 s to produce its first birth in
   the base arm. A probe shorter than several times that cannot read an invasion, whatever
   it prints — and neither of those two figures is a generation time: from a newborn's
   `alive_min` of 0.02, the `wood_rate` 0.001 growth cap needs at least **2,708 s** to
   reach `donor_min` 0.3 even on unlimited income, and only then can it start saving the
   300 s for a package (Astra R5.4). The study design Astra wrote in R5.4 — the matched
   arms, the resident-and-bank exclusion arm, the provisioned per-site mineral, the
   declared 6,000 s observation cap and the `3 × G` probe window — is the design to
   follow; it is not restated here, and this note's numbers are not inputs to it beyond
   the funding scale above. The probe's own observation bug is fixed (K8/R5.4: it counts
   newcomer birth identities per tick, so a descendant born and dead inside the window is
   reported as a birth, with survivors printed separately).

   **The recruitment cost has since changed, and the three-arm comparison predates it.**
   All three arms of that comparison ran before `K7`, when a bin paid its attrition
   *before* germination was tested, so a single package sat 0.005 % under its own threshold
   for ever — one tick of `seed_attrition_per_s` 0.001 at `DT` 0.05 is 5e-5 of the bank —
   and a recruit cost **two packages on one site**. The 400-second gate diagnostic added in
   `K8` (the section above) is **not** pre-K7: it ran under the new rule, and the scoping
   here is to the comparison alone (Astra R6.3). K7 made germination read the bank as it stands at the
   start of the tick and charged attrition on what stays — the rule the package size always
   stated: **one package is one recruit**, born on the next step at exactly `alive_min` of
   wood. The three-arm comparison is therefore a **pre-K7 observation, and the coupled
   post-K7 outcomes of those arms have not been measured** — not a lower bound on them
   (Astra R5.3).
   Earlier births change which gaps are occupied, and therefore water withdrawal, shading,
   litter and the funding that follows; there is no monotonicity to appeal to for the
   counts or the fractions, and unchanged accounting *rules* do not imply unchanged
   numerical budgets or residuals. Whoever designs the positive control should measure the
   generation time again under the new rule rather than scaling these.
2. **Bloomcrown's niche has not been measured, and no eligible count here measures it.**
   96 columns of 3,072 is its **introduction-time** reading under this tap, 2,688 is its
   **observation-time** reading on the same terrain (next section), and 2,526 was an
   introduction-time reading under the old rising tap. The contract to settle before the
   presets round is not a number but a statement of intent, per Astra's R5.2: whether the
   three new roles want sunny moist soil, soil pockets near rock, or damp soil under a
   canopy — and if any of them is meant to take bare rock or a dry ridge, that is an
   explicit substrate or trait decision and not a preset value. Bloomcrown already passes
   its own 0.1 pore and 0.6 light thresholds on open aerated soil at pore fraction 0.25, so
   the rule is not incapable of a sunny-soil niche; whether this generated world offers
   that condition persistently is unmeasured.
3. **Two packages per recruit was a consequence, not a decision — and it is settled.**
   Taken up as `K7` straight after this rerun, as the phase-order change of the two
   candidates: germination happens before that tick's attrition and expiry, so one package
   germinates whole and attrition applies to what stays banked. The other candidate — a
   margin added to the package size — was refused, because it would have turned a derived
   number into a tuned one.

## Round 4 — 2026-09-17

Five presets, one world, one duration. `community 400` in `--release` on the default
generated world and on one noise reseed, run by package R4 after springturf, stonecushion
and velvetpad landed:

```text
cargo run --release -p cubarium-voxel-flora --example two_producers -- community 400 1 0
cargo run --release -p cubarium-voxel-flora --example two_producers -- community 400 1 101
```

**This is a smoke run and not a study.** No control, no replication, no stationary
resident, no measured generation time, one duration and no arms. What it can say is that
five presets coexist in one world for 400 coupled seconds without anything dying, which
gate is shutting for each of them, and where a placement rule and a preset disagree. It
says nothing about habitat size, self-replacement or coexistence, and every eligible count
in it is a reading of the moment it was taken (Astra R5.2).

Conditions, both arms: 128 × 48 × 24 voxels of 0.25 m, `seed` 1, rain 2e-4 m/s with the
outlet open, 1,000 warm-up ticks (50 s) and then 8,000 coupled ticks (400 s), so the world
ends at 450 s. Eight founders per species at half of each species' own `wood_max`, which is
exactly its `donor_min`, placed by the harness's `Habitat` table — bloomcrown on the
highest eligible faces, umbrellafrond on the lowest, springturf on a **strided sample of
its eligible faces sorted by mean root-box pore** (not the eight wettest, which is what this
note used to say — Astra R7.4), stonecushion on eligible faces that are **not** soil,
velvetpad inside a taller founder's crown. That table is an experiment condition and not a
model rule, and springturf's rule has since been replaced: `Habitat::OpenSoil` now draws
deterministically among unoccupied, gate-passing sites whose support is soil and which have
no crown over them, so a rerun will not reproduce these founder columns.

**Wall time: 69.6 s for the default arm and 86.6 s for the reseed**, 8,000 coupled ticks
each, on this machine.

### Per species, at observation (450 s)

Counts are by `Stand::id`: "births" is every identity of that species ever seen alive at
the end of a tick, so a birth that died inside the run would still be counted (none did).
"desc." is a living stand that is not one of the founders.

| arm | species | founders | alive | still founder | desc. | births | deaths | wood | light | moisture | aeration stress | banked sites |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| default (noise 0) | bloomcrown | 8 | 14 | 8 | 6 | 6 | 0 | 2.9080 | 0.880 | 0.851 | 0.132 | 2 |
| | umbrellafrond | 8 | 16 | 8 | 8 | 8 | 0 | 3.3999 | 0.969 | 0.980 | 0.000 | 0 |
| | springturf | 8 | 9 | 8 | 1 | 1 | 0 | 0.2914 | 0.931 | 0.955 | **0.862** | 1 |
| | stonecushion | 8 | 8 | 8 | 0 | 0 | 0 | 0.4312 | 0.943 | 0.962 | 0.000 | 0 |
| | velvetpad | **6** | 8 | 6 | 2 | 2 | 0 | 0.7510 | 0.976 | 1.000 | 0.458 | 3 |
| reseed (noise 101) | bloomcrown | 8 | 12 | 8 | 4 | 4 | 0 | 2.7942 | 0.892 | 0.873 | 0.099 | 4 |
| | umbrellafrond | 8 | 15 | 8 | 7 | 7 | 0 | 3.4171 | 0.962 | 0.973 | 0.000 | 1 |
| | springturf | 8 | 9 | 8 | 1 | 1 | 0 | 0.2871 | 0.921 | 0.926 | **0.887** | 0 |
| | stonecushion | 8 | 8 | 8 | 0 | 0 | 0 | 0.4324 | 0.949 | 0.924 | 0.000 | 0 |
| | velvetpad | 8 | 14 | 8 | 6 | 6 | 0 | 1.0713 | 0.964 | 0.997 | 0.358 | 2 |

Two of the eight velvetpad founders were refused in the default arm, and it is a **placement
collision and not a failed habitat trial** (Astra R7.1): the `UnderACrown` rule picked two
columns an earlier founder had already been given, and `Command::Seed` refuses an occupied
site. Nothing about velvetpad's habitat, gates or crowns produced that six — the sample
simply kept columns it had already spent. The printed count is the six that were planted.
The harness now drops already-reserved and already-occupied columns **before** it samples,
so a rerun plants eight; the same repair changed `UnderACrown` itself, which compared crown
heights over their own support faces and left both support heights out, so this arm's
velvetpad columns will not reproduce either.

Ledgers and residuals, which is the part that has to hold before anything else means
anything:

```text
default  ledger: fixed_in 10.495539 respired_out 3.092541 light_in 20.991079 heat_out 6.185082
                 transpired 0.088653 m3 births 55 establishments 17 deaths 0
         residuals: organic -9.621e-12 mineral -1.421e-14 energy -1.924e-11
                    (stocks: organic 28.5430 mineral 61.4228 energy 57.0860)
         water: rain in 0.038400 m3/s, outlet 0.050000, transpiration 0.00022163,
                storage -0.011822 (residual -1.61e-11); head 2.741 m (-0.2434 m over the run)
reseed   ledger: fixed_in 10.830468 respired_out 3.183825 transpired 0.091839 m3
                 births 58 establishments 18 deaths 0
         residuals: organic -9.543e-12 mineral 2.842e-14 energy -1.909e-11
         water: storage -0.011830 (residual -9.14e-12); head 2.744 m (-0.2428 m)
```

The per-species birth counts sum to the ledger's own `establishments` in both arms (17 and
18), which is the identity count and the ledger counter agreeing. Relative residuals are
3.4e-13, 2.3e-16 and 3.4e-13 in the default arm. The head is still falling at 450 s in both
arms, −0.24 m over the run with the outlet at its full 0.05 m³/s, so **neither arm is a
settled world**.

### Which gate is shut, and for whom

Eligible skyline columns per species, introduction against observation, of 3,072:

| species | default: 50 s → 450 s | reseed: 50 s → 450 s |
| --- | --- | --- |
| bloomcrown | 96 → 2,692 | 96 → 2,689 |
| umbrellafrond | 373 → 695 | 386 → 667 |
| springturf | 75 → 2,346 | 72 → 2,327 |
| stonecushion | 1,720 → 2,737 | 1,723 → 2,731 |
| velvetpad | 117 → 1,274 | 111 → 1,282 |

Every species' eligible set grows, for the reason round 3 already measured: the initial
aquifer charge drains and the soil comes down off saturation, so the gates that were shut at
50 s open. The introduction reading is a reading of a wet world and not of a niche size, for
all five.

The observation diagnosis, default arm, with the sole-cause counts (a column can fail
several gates at once, so the totals overlap):

```text
      bloomcrown: 2692 eligible; pore < 0.10 0, saturated > 0.25 378, water over 0.05 m 60,
                  sky < 0.60 2; sole cause — saturation 318, light 2
   umbrellafrond:  695 eligible; pore < 0.45 2377 (all of them sole cause), saturated 0,
                  water over 0.50 m 0, sky < 0.10 0
      springturf: 2346 eligible; pore < 0.25 142, saturated > 0.30 291, water over 0.03 m 60,
                  sky < 0.75 327; sole cause — pore 139, saturation 204, light 293
    stonecushion: 2737 eligible; pore < 0.05 0, saturated > 0.40 335, water over 0.02 m 60,
                  sky < 0.40 0; sole cause — saturation 275
       velvetpad: 1274 eligible; pore < 0.30 1509, saturated > 0.60 289, water over 0.10 m 60,
                  sky < 0.05 0; sole cause — pore 1489, saturation 229
```

Four things this says, all of them about these two arms at 450 s and nothing more.

1. **No bank held a whole package at observation — and that is a reading of the banks at
   450 s, not a diagnosis of which gate refused a recruitment.** A germination needs three
   things at once: a bank over its threshold, a vacant site, and the predicate open. This
   run measured them separately, so they are reported separately (Astra R7.5).
   - **Bank sufficiency at observation.** Every non-empty bank sits at **90.5 % to 93.3 %**
     of its own threshold and **zero** banks are over it: bloomcrown 90.5 %, springturf
     91.2 %, velvetpad 92.1 % in the default arm, and bloomcrown 90.5 % and velvetpad
     93.3 % in the reseed. A package minus its own attrition is what a bank holds between
     deliveries, and a second package has to arrive before the first bin ages out.
   - **Predicate at observation.** Of velvetpad's three banked sites in the default arm,
     **zero** pass; in the reseed **one of two** does. So the predicate is not uniformly
     open at the banked sites either, and "the predicate is not what is refusing these
     sites" — what this note said before — was not a reading this run took.
   - **Vacancy at observation.** **Not recorded.** The diagnosis printed bank and predicate
     only, so this run cannot say whether a banked site was occupied.
     `germination_diagnosis` now prints the vacant count and the conjunction of all three,
     so the next authorised run can.

   What none of the three can say is *why* an earlier germination did not happen. Under K7
   a whole newly landed package at a vacant, passing site germinates out of the bank **as
   it stands at the start of a tick**, before that tick's attrition — and a package lands
   in phase 9, *after* the lottery of phase 8, so its first opportunity is the **next**
   tick and not the one it landed in (Astra R8.5: this note said "the same tick's
   lottery", which the phase order does not allow). So a fractional bank at 450 s is equally
   consistent with a package that landed on an occupied site, with one the predicate
   refused and attrition then wore down, and with one that has simply not been topped up
   yet. An earlier refusal is attributable only to an **observed event**, and this run
   logged none: it counted establishments, not refusals. Stonecushion is not in this
   finding at all — it has no landed bank (finding 2), so none of the three constraints
   ever applied to it.
2. **Stonecushion did not reproduce at all, and the reason is saving time, not habitat.**
   It is the most permissive of the five at the gate — 2,737 of 3,072 columns eligible, with
   `establish_pore_min` 0.05 refusing not one column — and 61 of 61 of its donors' candidate
   faces are eligible. But its `propagule_rate` 0.00005 funded 0.1333 of organic matter
   across eight donors in 400 s, which is 0.0167 each against a 0.025 package: **no donor
   reached one whole package**, so nothing landed. `funded / requested` is 100 %: the rate is
   the binding constraint here and not the reserve, which is the other half of Astra's R4.4
   distinction and the first time a preset in this world has been on that side of it.
3. **Springturf is funding-limited and, separately, under aeration stress.** It requested
   5.333 — the largest ask of the five by an order of magnitude — and was funded **2.2 %** of
   it, because `reserve_cap` 0.5 on a `wood_max` of 0.06 leaves a donor a surplus of at most
   0.0075 above its own floor while `maintenance` ten times the base draws on the same
   reserve. Raising `propagule_rate` further would create no income at all.

   Separately, its mean aeration stress at observation is **0.862**, the highest of the
   five by a wide margin — and this run does **not** show that the placement rule caused
   it (Astra R7.4). Three corrections to what this note claimed:
   - The placement was never "the eight wettest sites". `pick_founders` filtered on all
     four gates, `order_for` sorted that pool by mean root-box pore, and the caller then
     took a **strided sample of the whole sorted pool**, so the founders were spread across
     it rather than taken off its wet end.
   - Whatever the ordering, **every** site that passes springturf's gates has saturated
     fraction at most its own `establish_saturated_max` 0.3, so `aeration_target` is
     **zero** at planting for every founder it could have chosen (`step.rs`'s
     `aeration_target`). A mean stress of 0.862 at 450 s therefore needs later root-zone
     conditions, or stress retained from an intervening wet period — mean pore and
     saturated fraction are different measurements of the same box. Which of the two
     happened was not recorded, so it stays unattributed.
   - 0.862 is a cohort mean at the final tick and cannot become "about a seventh of its
     potential income for the whole run": that claim is **withdrawn**. `1 - stress`
     multiplies income at the instant it is read and this run kept no stress history.

   The placement rule changed anyway, for the reason Astra gives: ordering a gate-passing
   pool by mean pore is a habitat claim the role does not make. `Habitat::OpenSoil` draws
   deterministically among unoccupied, gate-passing sites whose support face is soil and
   which have no crown over them, and there is no longer any fallback to off-predicate
   founders — a species with no contract site is reported and not planted. The harness now
   also records each founder's gate values at planting and prints saturated fraction and
   stress **by identity** at observation, so the next authorised run can say what this one
   could not. Nothing promises that the new draw removes the stress, and no ceiling was
   tuned.
4. **Springturf's light gate is the only large sole-cause count of the five.** 293 columns
   refuse it on `establish_light_min` 0.75 alone (286 in the reseed), where bloomcrown's 0.6
   refuses 2. If this species turns out to be too rare, that is the first number to look at —
   and it is worth noting that the same threshold is the one the model cannot use for the
   role's own "loses under a canopy", because germination light is geometric sky with no
   canopy in it.

Umbrellafrond is unchanged from round 3 on purpose: 2,377 columns refused on **mean pore
alone**, its `establish_pore_min` 0.45 sitting above the mean root-box pore fraction over
umbrellafrond's own boxes on this skyline, **0.402**. That mean is a reading of these two
draining arms at 450 s and **not** drained soil's equilibrium (Astra R7.5): soil's retained
fraction is `field_capacity` **0.25**, and the head was still falling, −0.24 m over the run.
Astra's R6.1 said to keep the wetland role rather than lower that floor as a repair, and it
was kept.

### What this does and does not establish

Established: five presets run together for 400 coupled seconds on two landform variants with
**no deaths at all**, the three currencies conserved to 3e-13 relative, the water budget
closed to 1.6e-11, per-species lineage counts agreeing with the ledger's own counter, and a
gate diagnosis that names the binding constraint separately for each species.

Not established: nothing about habitat size (both arms are draining, not settled), nothing
about self-replacement (every species' bank is under its threshold at the end, and the
replacement-control study Astra designed in R5.4 is still the separate later package),
nothing about coexistence (no invasion, no rare-species arm), and nothing about whether any
of the fifteen thresholds these three presets introduced is a good number. They are
placeholders encoding roles, and `design/backlog.md` §1 lists all of them.

## Round 5a — 2026-09-17

Bounded food transfers landed (package M): three withdrawals, a deposit with a carrion
pool, a reach query, and two new named boundary flows carrying six ledger terms between them
(`consumed_organic_out` / `consumed_mineral_out` / `consumed_energy_out` and the three
`deposited_*_in`; the brief said "four", and three currencies in each direction is six) — and
with them the probe the biosphere asks for before any voxel animal exists (`design/theoretical-biosphere-2026-09-16.md` §5: before
voxel animals exist "a bounded experimental harvest of reachable foliage can test only the
producer response"). One command, four arms:

```text
cargo run --release -p cubarium-voxel-flora --example two_producers -- harvest 400
```

**A probe, not a study, and it says nothing about an animal.** No body, no movement, no
population, no birth of anything that eats, no carrying capacity, no viability claim. What
it measures is what the plant model's existing `foliage_rate` and reflush-from-reserve rules
do to a stand whose canopy is being taken, and what the ledger does with the material that
left. There is no replication (one seed, one landform), no dose series (one rate) and no
measured recovery time to have chosen 200 s of recovery against.

Conditions, all four arms: 128 × 48 × 24 voxels of 0.25 m, `seed` 1, `noise_seed` 0, rain
2e-4 m/s with the outlet open, 1,000 warm-up ticks (50 s) and then 8,000 coupled ticks
(400 s), so the world ends at 450 s. Eight founders per species at half its own `wood_max`,
placed by the harness's `Habitat` table — an experiment condition and not a model rule
(`design/backlog.md` §1). Two patches, springturf and bloomcrown; each patch is a
**plant-only arm and a harvested arm of the same conditioned world**, built from the same
deterministic generation and the same founder selection, which the harness asserts rather
than assumes (the two arms' founder lists and declared cohorts are compared, and the
plant-only arm's `consumed_*` are asserted to be zero).

The treatment: every tick, from each of **three predeclared support faces** — the first
three founder faces of the patch species in site order — a harvester takes up to
`0.002 · dt` of organic matter off the foliage of the stands inside
`Reach { horizontal: 2, up: 1 }`, spending that budget on them in site order, and deposits
nothing. It **stops at 200 s**, leaving 200 s of recovery. The rate and the box are declared
conditions, not model knobs, and both are now backlog rows: 0.002 /s per face is *faster*
than one springturf founder's own `foliage_rate · W` (6e-4 /s at founder wood 0.03), on
purpose, so what the probe reads is a stripped stand and not a grazing equilibrium.

**Wall time: 273.2 s for four arms of 400 coupled seconds** (8,000 ticks each, plus each
arm's own generation and 1,000 warm-up ticks), on this machine. An earlier run of the same
four arms took 279.9 s and produced **bit-identical** numbers — the only difference between
the two outputs is the two lines this note's residual table needed and the wall time itself,
which is what "deterministic in the seed" is supposed to mean.

### The reach box excludes a grown bloomcrown, and that is the first result

`SpeciesConfig::crown_voxels` is 2 for a bloomcrown at the harness's founder wood (0.3 of
`wood_max` 0.6 gives `crown_height` 2.0), against `up: 1`. So **no bloomcrown was ever
eaten in either patch**: a harvester standing on a bloomcrown founder's own face cannot
reach that founder's crown, and what it ate instead was the springturf and velvetpad within
two voxels of it. The species that *were* bitten are in the report by species, because the
ledger's `consumed_*` have no species in them and a reach box does not choose one.

That is the rule behaving as `design/theoretical-biosphere-2026-09-16.md` §6 asks — food
above reach does not feed a ground browser — and it is worth stating before the tables,
because it makes the "bloomcrown patch" arm a statement about *where the harvester stood*
and not about what it ate. A bloomcrown **newborn** is a different matter: at `alive_min`
0.02 of wood its `crown_height` is 1.07, which rounds to one voxel and is in reach. Six
bloomcrown newborns appeared between 300 s and 400 s in every arm — after the harvest had
stopped — so none of them was ever bitten either. A dose series that wanted to graze
bloomcrown seedlings would have to run the harvest window over the recruitment window.

### Springturf patch: what was taken and what came back

Foliage / reserve / wood, summed per species over the stands standing at that moment
(`stands` in the first column of each cell):

| t (s) | arm | springturf | velvetpad | bloomcrown |
| --- | --- | --- | --- | --- |
| 0 | plant-only | 8 / 0.4800 / 0.1200 / 0.2400 | 8 / 1.6000 / 0.4000 / 0.8000 | 8 / 4.8000 / 1.2000 / 2.4000 |
| 0 | harvested | 8 / 0.4800 / 0.1200 / 0.2400 | 8 / 1.6000 / 0.4000 / 0.8000 | 8 / 4.8000 / 1.2000 / 2.4000 |
| 100 | plant-only | 8 / 0.5337 / 0.0667 / 0.2669 | 8 / 1.6996 / 0.2125 / 0.8498 | 8 / 4.6955 / 1.1555 / 2.4158 |
| 100 | harvested | 8 / 0.3346 / 0.0418 / 0.2461 | 8 / 1.4334 / 0.2098 / 0.8392 | 8 / 4.6955 / 1.1555 / 2.4158 |
| 200 | plant-only | 8 / 0.5909 / 0.0739 / 0.2957 | 8 / 1.8221 / 0.2278 / 0.9111 | 8 / 4.6996 / 1.1674 / 2.5139 |
| 200 | harvested | 8 / 0.3714 / 0.0465 / 0.2503 | 8 / 1.2646 / 0.1685 / 0.8778 | 8 / 4.6996 / 1.1676 / 2.5142 |
| 300 | plant-only | 8 / 0.5692 / 0.0489 / 0.3004 | 9 / 1.8974 / 0.2425 / 0.9590 | 8 / 4.7468 / 1.2155 / 2.6881 |
| 300 | harvested | 8 / 0.3503 / 0.0276 / 0.2395 | 9 / 1.3360 / 0.1822 / 0.9138 | 8 / 4.7469 / 1.2157 / 2.6884 |
| 400 | plant-only | 8 / 0.5195 / 0.0200 / 0.2829 | 9 / 1.9132 / 0.2436 / 0.9797 | 14 / 4.9151 / 1.3054 / 2.9146 |
| 400 | harvested | 8 / 0.3186 / 0.0085 / 0.2173 | 10 / 1.3860 / 0.1933 / 0.9445 | 14 / 4.9134 / 1.3043 / 2.9127 |

Umbrellafrond and stonecushion had no stand within reach of a face and were never bitten.
They are identical in the two arms to four decimals through 300 s and differ in the **fourth
decimal at 400 s** (umbrellafrond 5.2664 against 5.2665 of foliage; bloomcrown 4.9151
against 4.9134). That is not noise and it is not a leak — but it is not *coupling between
the arms* either, which is what this note said before (Astra R8.3). Each arm is its own
`World` and its own `Flora`; nothing crosses between them. What is shared is **within** an
arm: the stands of one arm share that arm's water table and shade field, so a grazed stand
transpires and shades less and every other stand in *that* world reads a slightly different
world from then on. "Untouched" therefore means **not bitten**, never unaffected, and a
fourth-decimal difference in an unbitten species is a **downstream effect of the treatment**
— its magnitude is not a threshold below which attribution becomes impossible. What this run
has not done is separate the direct bite from those mediators, or test whether the sign of
such a difference holds across seeds.

The declared cohort — the six stands reachable from the three faces **at the start**, fixed
by identity before anything was eaten, three springturf and three velvetpad:

| arm | foliage at 0 s / 200 s / 400 s | reserve at 0 s / 200 s / 400 s | mean fill `P/(α·W)` | alive |
| --- | --- | --- | --- | --- |
| plant-only | 0.78000 / 0.91778 / 0.88709 | 0.19500 / 0.11475 / 0.09664 | 1.000 / 0.999 / 0.941 | 6 / 6 / 6 |
| harvested | 0.78000 / 0.14054 / 0.14199 | 0.19500 / 0.02794 / 0.02720 | 1.000 / 0.105 / 0.106 | 6 / 6 / 6 |

**The fill column is the measure, and the strict recovery bar is not.** The harness counts a
stand "recovered" at 99 % of its own `α · W`, and the control says how strict that is: an
**ungrazed** cohort here sits at 0.941 of its own cap at 400 s. Senescence is one term in
that and not the explanation (Astra R8.3): base senescence removes 0.005 % of `P` per tick,
and whether it is replaced is decided jointly by the growth, resource and stress budgets —
`foliage_rate · W` against the gap, the income funding it, the reflush rule, and whatever
the mineral pool and the aeration stress allow. So "0 of 6 recovered" cannot be
read as "nothing regrew" — the comparison that can is the grazed cohort's 0.106 against the
control's 0.941, and its flatness (0.105 at the stop, 0.106 at the end).

Taken: **8,520 withdrawals that returned something, on 6 distinct stands, 0.772133 of
organic matter, 0.0157000 of mineral and 1.544266 of energy** — 0.200734 off three
springturf and 0.571399 off three velvetpad. The three faces' nominal budget over the window
was `3 · 0.002 · 200` = 1.2, so the harvester actually got **64 %** of what it asked for: for
much of the window there was less foliage in reach than the budget, which is the reach box
and the stripped stands and not any rule refusing it. (A face can draw on more than one
stand in a tick when the first cannot fill its budget, so the withdrawal count is not a
count of ticks.)

### Bloomcrown patch: the same treatment from three higher faces

| t (s) | arm | springturf | velvetpad |
| --- | --- | --- | --- |
| 0 | plant-only | 8 / 0.4800 / 0.1200 / 0.2400 | 8 / 1.6000 / 0.4000 / 0.8000 |
| 0 | harvested | 8 / 0.4800 / 0.1200 / 0.2400 | 8 / 1.6000 / 0.4000 / 0.8000 |
| 100 | plant-only | 8 / 0.5337 / 0.0667 / 0.2669 | 8 / 1.6996 / 0.2125 / 0.8498 |
| 100 | harvested | 8 / 0.4679 / 0.0585 / 0.2602 | 8 / 1.3738 / 0.1995 / 0.8434 |
| 200 | plant-only | 8 / 0.5909 / 0.0739 / 0.2957 | 8 / 1.8221 / 0.2278 / 0.9111 |
| 200 | harvested | 8 / 0.4460 / 0.0558 / 0.2746 | 8 / 1.2740 / 0.1710 / 0.8845 |
| 300 | plant-only | 8 / 0.5692 / 0.0489 / 0.3004 | 9 / 1.8974 / 0.2425 / 0.9590 |
| 300 | harvested | 8 / 0.4178 / 0.0297 / 0.2662 | 8 / 1.3118 / 0.1749 / 0.9005 |
| 400 | plant-only | 8 / 0.5195 / 0.0200 / 0.2829 | 9 / 1.9132 / 0.2436 / 0.9797 |
| 400 | harvested | 8 / 0.3797 / 0.0084 / 0.2409 | 9 / 1.3328 / 0.1817 / 0.9226 |

| arm | foliage at 0 s / 200 s / 400 s | reserve at 0 s / 200 s / 400 s | mean fill `P/(α·W)` | alive |
| --- | --- | --- | --- | --- |
| plant-only | 0.92000 / 1.03653 / 1.08822 | 0.23000 / 0.12957 / 0.12930 | 1.000 / 1.000 / 0.955 | 6 / 6 / 6 |
| harvested | 0.92000 / 0.34358 / 0.37102 | 0.23000 / 0.05470 / 0.05588 | 1.000 / 0.264 / 0.257 | 6 / 6 / 6 |

This cohort's absolute foliage **rose** 8 % over the recovery window while its mean fill
**fell**, from 0.264 to 0.257: the same stands' own `α · W` caps grew faster than their
canopies did, because grazing foliage does not stop wood growth. A stand can therefore come
back in absolute terms and fall further behind itself at the same time, which is the reason
this note reports both columns and not one.

Taken: **7,829 bites on 5 distinct stands, 0.704971 organic, 0.0143281 mineral, 1.409943
energy** — 0.141104 off two springturf and 0.563867 off three velvetpad, and nothing off a
bloomcrown. That is **59 %** of the same nominal 1.2, and one of the three faces, `(12, 8)`
at `y13`, had **nothing in reach at all** for the whole run, which the report says at the top
rather than hiding inside a total.

### The producer response, in plain words

1. **Grazing at this rate strips a stand and the stand does not die of it.** Deaths were
   **zero in all four arms**, as they were in the round-4 smoke. Foliage is not wood: the
   withdrawal takes `P` and never touches `W`, and death is `W < alive_min`, so a harvester
   cannot kill a stand directly — it can only take its income away.
2. **The income falls with the foliage, and the reserve pays the difference.** The grazed
   cohorts ended the harvest window at 15 % (springturf patch) and 33 % (bloomcrown patch)
   of their own control's foliage, and at 24 % and 42 % of its reserve. Both arms' reserves
   fall over the run — the controls' cohort reserve fell to 50 % and 56 % of its starting
   value on its own, because these founders are spending on wood, foliage and propagule
   parcels — so what grazing did was **deepen an existing draw-down**, not create one.
3. **Limited partial regrowth in 200 s, and no stand reaching the declared full-foliage
   threshold.** The grazed cohorts gained absolute foliage after the harvest stopped —
   **+1.03 %** in the springturf patch and **+7.99 %** in the bloomcrown patch — and their
   deficits against their own controls moved in *opposite* directions: the
   springturf-location deficit narrowed from **0.77724 to 0.74510** while the
   bloomcrown-location deficit **grew** from **0.69295 to 0.71720**, because the control was
   moving too. Neither closed, and "nothing measurably recovered" — what this note said
   before — was too strong (Astra R8.3). In canopy fill, which
   is the measure that reads each stand against its own cap, the grazed cohorts sat at
   **0.105 → 0.106** and **0.264 → 0.257** against controls at **0.999 → 0.941** and
   **1.000 → 0.955**. The strict recovery count (0 of 6 and 0 of 5 stands back within 1 % of
   their own `α · W`) is consistent with that but cannot carry it on its own, because an
   ungrazed cohort here ends below that bar too. Both controls were themselves drifting over
   the same window — the springturf patch's control cohort *lost* 3 % of its foliage while
   the grazed one gained 1 % — which is why every comparison here is against the control and
   never an absolute slope.
4. **Why so little, in the model's own terms.** Income is
   `assimilation · L_eff · μ · (1 − stress) · P · monod · dt`, proportional to the very stock
   that was taken, and the other way back to full foliage is reflush — spent out of a reserve
   the harvest had already drawn down. A stripped stand is therefore slow to come back
   *because* it is stripped, and it is slowest exactly when its reserve is lowest. That is
   the two existing rules composing, and this run shows their joint outcome; it does **not**
   separate them, and nothing here measures a recovery time constant. A dose series and a
   window long enough to see a reserve refill are what would.
5. **One arm differed in recruitment, and this run cannot attribute it.** The springturf
   patch's grazed arm ended with 15 establishments against the control's 14, and one more
   living velvetpad (10 against 9); the bloomcrown patch's arms both ended at 14. That is a
   **measured contrast with its mechanism unmeasured**, and it is reported and not explained.
   No direction was expected of it either (Astra R8.3): competition, water, shade, the timing
   of a vacancy and the keyed lotteries supply no monotonicity between "less income" and
   "fewer recruits", so the earlier gloss — that a grazed world should if anything recruit
   less — was not a prediction this model licenses.

### The three residuals, with a consumer taking material out

`consumed_organic_out`, `consumed_mineral_out` and `consumed_energy_out` are named boundary
flows, subtracted in `expected_organic`, `expected_mineral` and `expected_energy`, so the
three residuals mean what they meant in round 3 and round 4 — with material now leaving the
layer into a consumer's hands. At 400 s, each residual against the stock it is a residual of:

| patch | arm | organic | mineral | energy |
| --- | --- | --- | --- | --- |
| springturf | plant-only | −9.692e-12 of 29.5635 (−3.28e-13) | 7.105e-15 of 62.4368 (1.14e-16) | −1.938e-11 of 59.1271 (−3.28e-13) |
| springturf | harvested | −8.026e-12 of 28.4927 (−2.82e-13) | 7.105e-15 of 61.4211 (1.16e-16) | −1.605e-11 of 56.9853 (−2.82e-13) |
| bloomcrown | plant-only | −9.692e-12 of 29.5635 (−3.28e-13) | 7.105e-15 of 62.4368 (1.14e-16) | −1.938e-11 of 59.1271 (−3.28e-13) |
| bloomcrown | harvested | −8.818e-12 of 28.5376 (−3.09e-13) | 1.421e-14 of 60.4225 (2.35e-16) | −1.764e-11 of 57.0753 (−3.09e-13) |

The same 1e-13 relative float noise round 4 reported, with 0.772133 and 0.704971 of organic
matter, 0.0157000 and 0.0143281 of mineral and 1.544266 and 1.409943 of energy taken out
through `take_foliage` in the two harvested arms. The harness books nothing of its own: it
reports the `Taken`s it was handed, and the ledger's `consumed_*` are the sum of exactly
those by construction. The two plant-only arms are the same arm run twice — one per patch,
so that each patch's control can report that patch's own cohort — and their ledgers agree to
the bit, which is a second reading of the determinism.

**Nothing deposited anything in this run.** `deposited_*` and `Ground::carrion` are zero
throughout, so `carrion_decomposition`'s **flux** was zero — its configured rate is 0.005 /s
and unmeasured, which is a different statement (Astra R8.3) — and the only place any of them
is exercised is `tests/round5a.rs` and, from round 5b, `tests/round5b.rs`. The cross-layer check — flora `consumed_*` against a fauna layer's
`eaten_*`, and flora `deposited_*` against its own — belongs to the round that has a fauna
ledger to check against.

### Open items this round leaves

1. **The deposit side has no coupled measurement.** Nothing in a running world deposits
   carrion or dung yet, so the carrion pool, its rate and the litter-deposit cap are pinned
   by short function tests and by nothing else. The first run with a corpse in it is
   rounds 5b/5c's.
2. **Remains are invisible, and the host does not print them.** The voxel presenter draws no
   glyph for `Ground::carrion` and the host's `i X Y Z` inspect line does not print the pool
   (`crates/cubarium/src/voxel/mod.rs`). Both were left alone deliberately: **`crates/cubarium`
   needed no change at all for this round** — the new `Ground` fields arrive through
   `Ground::new` — and a glyph for a stock nothing fills would be drawing an empty pool. The
   glyph is a backlog row (§2) and its look belongs to Wrysk's art-direction thread.
3. **`Deposit::DeadWood` does not exist.** Rounds 5b/5c's brief wants a dead-wood deposit
   kind to lay down declared logs and says to add it there if this round did not. It did not:
   this round's brief named `Carrion` and `Litter`, and adding a third kind was out of its
   scope.
4. **One rate, one seed, one landform, one window.** No dose series, no replication, no
   second landform, and a 200 s recovery window chosen because the run is 400 s rather than
   measured against anything. A recovery **time constant** is what a dose series and a window
   long enough to see a reserve refill would produce; this probe does not have one.
5. **The harvester is not an animal and must not be read as one.** It has no body, pays
   nothing, never moves, never dies and eats from three fixed faces at a declared rate. Its
   totals are a *treatment*, not a diet, and no consumer's bite rate, reach or population can
   be inferred from them.

### What this does and does not establish

Established: three bounded withdrawals and one deposit with its own new pool, all booked as
named boundary flows, with the three residuals holding at 1e-13 relative float noise across
four 400-second coupled arms **with material leaving the layer**; a reach rule that admits a
turf and excludes a grown bloomcrown crown, measured rather than asserted; zero deaths under
a harvest that took 0.77 and 0.70 of organic matter off six and five stands; and a producer
response that is the existing rules composing — income falls with the stock that was taken,
reflush is paid out of a reserve the harvest had drawn down, and neither grazed cohort closed
its gap to its own control within 200 s in either foliage or canopy fill.

Not established: anything about an animal, and this round deliberately leaves the biosphere's
two candidate first consumers unchosen; any recovery time constant, or how much of the
non-recovery is the income term and how much the emptied reserve (the run shows their joint
outcome and does not separate them); whether `carrion_decomposition` 0.005 /s or the harvest
rate is a reasonable number (nothing has measured either, and no result here depends on the
first); and the mechanism of the fourth-decimal differences in unbitten species, which are
downstream effects of the treatment through each arm's **own** water and shade rather than
coupling between the arms, with the direct bite and its mediators not separated and no test
of generality across seeds.

## Round 5b — 2026-09-17

Package N landed `Species::Glowcap`, a **saprotroph** stand: the same stand the five
producers are — the same establishment, growth, senescence, dieback, death, seed bank and
paid parcel — with its income line replaced. `SpeciesConfig::trophic` is the only thing that
selects between them. It eats the dead wood of its **mycelium box** (the root box's geometry
read as support sites), keeps `substrate_yield` of what it takes and respires the rest at
once, has no light income and **no light gate**, and needs
`establish_substrate_min` of wood in that box before a spore may germinate. The biosphere's
§5 branch 2 and its §6 request, and nothing more than that.

One command, one arm, six species:

```text
cargo run --release -p cubarium-voxel-flora --example two_producers -- community 400
```

**A smoke run, not a study.** No control, no replication, no dose series, one seed, one
landform. It exists to say whether six presets can be in one world at once, whether every
quantity a fungus moves is paid and observable, and which gate is shutting for each species.
**Nothing here is a population claim, a viability claim or a carrying capacity**, and the
first thing to say about it is that the fungus's *own* second generation did not happen —
see "the grove did not spread" below.

Conditions: 128 × 48 × 24 voxels of 0.25 m, `seed` 1, `noise_seed` 0, rain 2e-4 m/s with the
outlet open, 1,000 warm-up ticks (50 s) and then 8,000 coupled ticks (400 s), so the world
ends at 450 s. Eight founders per species at half its own `wood_max`, placed by the harness's
`Habitat` table — an experiment condition and not a model rule (`design/backlog.md` §1).

**New condition, and the one a reader has to know about: eight declared logs.** A fresh world
holds **no dead wood at all**, so a saprotroph's substrate gate is shut everywhere and a
glowcap cannot be introduced. The harness therefore lays 8 logs of 1.0 organic matter each,
with the mineral and the energy a dead trunk holds (`n_tissue · organic` = 0.02 and
`e_v · organic` = 2.0), through round 5a's `deposit` with round 5b's new
`DepositKind::DeadWood`, on faces that pass every glowcap gate but the substrate one, drawn
in the keyed spread `Habitat::OpenSoil` uses, before anything is planted. They are printed
with the run. **That wood is imported into the world** — booked as `deposited_organic_in`,
an inflow like `seeded_*`, and not a stock the plants grew — so the organic inventory is not
closed across this run, exactly as it is not closed while first landings are still
provisioning `initial_mineral`.

**Wall time: 70.4 s for 8,000 coupled ticks** (plus the generation and 1,000 warm-up ticks),
on this machine. A first run of the same arm took 69.2 s and produced **bit-identical**
numbers; the only difference between the two outputs is the per-banked-site gate lines the
second build added and the wall time itself.

### The community, at observation (450 s)

Counts are by `Stand::id`, so "desc." is a germination and not a site that changed hands;
`wood`, `light`, `moist` and `stress` are the species' living totals and means.

| species | founders | alive | founders still | desc. | births | deaths | wood | light | moist | stress | banks |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| bloomcrown | 8 | 14 | 8 | 6 | 6 | 0 | 2.9160 | 0.883 | 0.851 | 0.133 | 2 |
| umbrellafrond | 8 | 15 | 8 | 7 | 7 | 0 | 3.4041 | 0.964 | 1.000 | 0.000 | 1 |
| springturf | 8 | 8 | 8 | 0 | 0 | 0 | 0.2828 | 0.916 | 1.000 | 1.000 | 3 |
| stonecushion | 8 | 8 | 8 | 0 | 0 | 0 | 0.4312 | 0.943 | 0.962 | 0.000 | 0 |
| velvetpad | 8 | 8 | 8 | 0 | 0 | 0 | 0.9767 | 0.965 | 1.000 | 0.597 | 8 |
| **glowcap** | **8** | **8** | **8** | **0** | **0** | **0** | **0.4716** | 0.948 | **0.688** | **0.292** | **3** |

61 births in all (48 founders and 13 establishments), **zero deaths**, and the five
producers' numbers are the round-4 smoke's shape: bloomcrown and umbrellafrond recruit, the
other three hold their founders. The glowcap's `light` 0.948 is reported and **inert** — it
is the sky its cells see, and its income never reads it.

### What the fungus did, in its own currency

| quantity | value |
| --- | --- |
| dead wood laid (8 logs × 1.0) | 8.0000 |
| dead wood left on those logs at 450 s | 5.7924 |
| dead wood elsewhere in the world at 450 s | 0.0176 |
| `substrate_uptake[glowcap]`, cumulative | **1.93599** |
| of that, tissue at `substrate_yield` 0.4 | 0.77440 |
| of that, respired at once | 1.16159 |
| mycelium (`W`) at 0 s → 450 s | 0.4000 → **0.4716** |
| parcels funded (net) / landed / standing | 0.19245 / 0.07500 (3 packages of 0.025) / 0.11745 |
| requested against funded | 1.33333 / 0.19245 (**14.4 %**) |

Three readings worth keeping. **The logs are being eaten and not only rotting**: 2.2076 of
organic matter left them, 1.9360 of it into the fungi and 0.2716 to `wood_decomposition` on
its own, so the saprotroph took **88 %** of the log turnover and the fungi ate **24 %** of
the wood laid in 400 s. **The world made a little dead wood of its own** — 0.0176 away from
the declared logs, from the dieback of stressed stands with nothing dying — which is the
first dead wood in this world that a plant produced, and the only supply a grove would have
after the declared logs are gone. And **the income was nearly all spent**: against 0.77440 of
tissue income the budget is maintenance `0.0002 · W · t` ≈ 0.035, senesced caps replaced at
`(1 + c_g) · senescence · P · t` ≈ 0.43, parcels 0.231 gross and a body up by 0.072, which
closes to about 0.77 — so the surplus the uptake rule *can* produce (it is bounded by the
rate and the pools and **not** by what the stand can spend) was a small term here, at a mean
moisture of 0.688. That is a measurement of this arm and not a property of the rule.

**A drying log starves it, and saturated roots do not reduce adult uptake.** Mean moisture
0.688 across the eight fungi, and the two on the driest logs sat at 0.500 — `μ` multiplies
the uptake, so those two earned half. Mean `aeration_stress` 0.292, and it **cost them
nothing**: a saprotroph's uptake reads `μ` and not `1 − aeration_stress`, so three fungi whose
root boxes are now over half saturated pay for it on the way in (the gate refuses a spore
there) and nothing as adults. Free water still **kills** it — `drown_depth_m` 0.05, and a
cap under a pool dies in step 3 — so "a drowned one does not" would have been wrong twice
over (Astra R9.3): what saturation does not do is reduce an established mycelium's income.
And `μ` is read off the **soil voxels** of the root box, so "bare rock" means no soil anywhere
in that box: a rock face with a soil pocket inside the box is not bare and does not starve. That is the preset's stated limitation, now measured rather than asserted, and it is
the first thing to revisit if waterlogging is meant to cost a mycelium anything.

### The grove did not spread, and the gate diagnosis says why

Zero glowcap establishments. Not for want of reproduction: three whole packages left donors
and landed, and the biggest bank stood at 85.2 % of the 0.025 threshold at observation.
**All three banked sites are refused by the substrate gate**, and the per-site gate line says
so in the same numbers the model refused them on:

```text
glowcap: 3 banked sites, biggest 0.02130 (85.2% of threshold); 0 of all 3 banked sites pass it
  banked site (47,3) y12 — mean pore 0.719 (>= 0.10), saturated fraction 0.625 (<= 0.50), dead wood in the box 0.000 (>= 0.020)
  banked site (48,1) y12 — mean pore 1.000 (>= 0.10), saturated fraction 1.000 (<= 0.50), dead wood in the box 0.000 (>= 0.020)
  banked site (49,1) y13 — mean pore 0.372 (>= 0.10), saturated fraction 0.000 (<= 0.50), dead wood in the box 0.000 (>= 0.020)
```

The mechanism is a **geometric mismatch between dispersal and the box**, and it is worth
stating precisely because it is a model finding and not a tuning question. A package lands on
the **highest support face** of a column inside the donor's `hop` (`dispersal_target`, one
candidate per column so that a stand cannot seed the terraces below itself). The glowcap's
mycelium box is `rooting_depth` **1**, so it is the nine support sites of the recipient's
**own** `y` row. When the neighbouring column's highest face sits at a different height from
the donor's — which on this terrain is usual, and `(49,1) y13` against a donor at `y12` is
exactly it — the recipient's box does not contain the log the donor is standing on, and the
substrate gate shuts on a face one voxel from a full log. Two of the three were also refused
on aeration. Across the whole strip the shape is the same: at observation 33 of 3,072 skyline
columns pass the glowcap predicate, the substrate gate shuts **3,039** of them, and the
donors' own dispersal neighbourhood is 12 eligible faces of 52 candidates (**1.5 per
donor**). So a grove's spread at these placeholders is bounded by the log and by the box's
geometry, not by the hop and not by reproduction.

Three ways out were on the table, all of them rule decisions rather than numbers: a mycelium
box with a vertical reach of its own; a dispersal rule for a saprotroph that lands on the
substrate rather than on the skyline; or a deeper `rooting_depth`, which is only a number
and the least honest of the three, because it moves the water too and repairs one direction
only. **Decided (Astra R9.3, package Q):** substrate access has its own species field,
`substrate_reach_up_down`, placeholder **1**, symmetric — one row up and one row down — and
the soil-water root box is untouched. Blind paid landing and germination-time selection
stay: a spore still lands on the highest support face of a column inside its `hop` and the
gates are still read there, so a grove still ends where the wood does. The four geometry
cases are pinned in `crates/cubarium-voxel-flora/tests/round5b.rs`
(`substrate_access_reaches_one_row_up_and_one_row_down`): same level, one up, one down, and
two genuinely substrate-free faces refused on substrate alone. This run predates the change,
so its zero establishments are a reading of the old box.

The other half of the same finding is that the run's own supply of dead wood — 0.0176 from
dieback — is about **57 times** smaller than one declared log of 1.0, not three orders of
magnitude; and that 0.0176 is a **standing stock** at one moment and not a cumulative
supply, so it says what was lying there and not what the plants produced over the run.
Either way a self-sustaining decomposer grove needs plants that actually die, which is the
round-4/5a observation (zero deaths in 400 s) reappearing from the other side.

### The three residuals, with a fungus digesting

| currency | residual | stock | relative |
| --- | --- | --- | --- |
| organic | −1.031e-11 | 37.3409 | −2.76e-13 |
| mineral | −3.268e-13 | 74.6248 | −4.38e-15 |
| energy | −2.062e-11 | 74.6819 | −2.76e-13 |

The same 1e-13 relative float noise rounds 3, 4 and 5a report, with a saprotroph moving
1.936 of organic matter out of ground stocks and into living tissue over 8,000 ticks. It
closes because that transfer **crosses no boundary**: `substrate_uptake` is a per-species
diagnostic flux and not a ledger term, like the three `propagule_*` arrays, and the only
boundary flows the fungus touches are the `respired_out`/`heat_out` it pays and the
`deposited_*_in` of the declared logs. Booking the uptake out through `consumed_*_out` and
back in again would have kept these residuals and destroyed the meaning of
`consumed_organic_out`, which is "what a consumer outside this layer took" and is what a
harvest or a grazing study reads.

The core's water ledger over the same run: rain in 0.038400 m³/s, outlet 0.050000,
evaporation 0, transpiration 0.00023086, storage change −0.011831 (residual −1.60e-11), head
2.741 m (−0.2434 m over the run), stored 213.3395 m³. The head is still falling at 450 s, so
every eligible-column count here is a reading of a moment and not a settled habitat.

### What this does and does not establish

**Established.** Six presets, one of them not a plant, run together for 400 coupled seconds
with zero deaths and the three residuals at 1e-13 relative. A saprotroph's whole metabolism
is paid and observable: it withdraws from a bounded pool through the same pro-rata rule a
consumer's `take_dead_wood` uses, respires what it does not keep, moves its mineral between
stocks without creating any, and funds its spores out of its reserve like every other stand.
The gates behave as the brief asks — no light gate, a substrate gate that bites, and the
existing pore and aeration gates doing the "damp but aerated attachment" work with no new
rule. And the harness can put a fungus in a fresh world at all, which needed
`DepositKind::DeadWood`.

**Not established.** Anything about a fungal population: this arm's glowcaps are eight
founders that fed, grew 18 % of mycelium and failed to recruit, and *why* they failed is
attributed to the substrate gate by a per-site reading at one moment, not by a controlled
comparison. Nothing about whether 0.02 /s of uptake, a 0.4 yield, a 0.02 substrate threshold
or a 1.0 log is a reasonable number — **nothing measured any of them**, and the preset's own
doc says which sentence each of them was chosen to encode. Nothing about competition between
the fungus and the plants (they share a mineral pool and a water box, and no arm varied it),
nothing about two fungi on one log *in a run* — the collect-then-withdraw rule now has its
own unit test on `feed` itself, two mycelia of unequal demand sharing one log one-to-three in
all three currencies (`src/step.rs`,
`two_fungi_share_one_log_in_proportion_to_their_demand`), which is what Astra's R9.3 found
missing: the earlier claim rested on a test of the scalar proportional arithmetic. This
arm's boxes did not overlap enough to exercise it —
and nothing about what a waterlogged log should cost a mycelium.

## Round 5c — 2026-09-17

The first animal (package O): a new crate `cubarium-voxel-fauna` holding the **frondgrazer**,
a heuristic ground browser that stands on a support face, crops through round 5a's bounded
withdrawals, walks toward the best food it can sense, breeds out of its own reserve and hands
its body back as carrion. One command, two arms:

```text
cargo run --release -p cubarium-voxel-fauna --example grazed -- grazed 400 4
```

**A probe, not a study, and it claims nothing about a population.** One seed, one landform,
one grazer count, one introduction time, no replication and no dose series. Astra's round 7
left population targets and carrying capacity unclaimed and nothing here takes them: what is
measured is whether every quantity an animal spends is **paid and observable**, and whether
the two layers' ledgers close against each other.

Conditions, both arms: the default 128 × 48 × 24 voxels of 0.25 m, `seed` 1, `noise_seed` 0,
rain 2e-4 m/s with the outlet open, 1,000 warm-up ticks and then 8,000 coupled ticks (400 s).
Eight founders each of **springturf and bloomcrown** at half their own `wood_max`, placed by
this harness's own declared rule — each species' gate-passing skyline sites, in skyline order,
strided — which is *not* `two_producers.rs`'s `Habitat` table and is a backlog row
(`design/backlog.md` §1). The two arms are **separate `World` and `Flora` instances** from the
same seed, and the run asserts their founder lists are identical rather than assuming it, so
an arm-to-arm difference is the treatment reaching the producers through that arm's own water
table, shade field and germination lotteries, and never a coupling between arms.

The treatment: at the halfway point (200 s), **four** grazers are introduced at `body_max`
0.05 with a full reserve on four declared gate-passing **open-soil** faces — `(4,0)y12`,
`(14,6)y12`, `(23,6)y12`, `(34,6)y12` — and nothing else changes. Each animal's numbers are
the untuned placeholders of `SpeciesConfig::frondgrazer`.

**Wall time: 136.8 s for two arms of 400 coupled seconds** (8,000 ticks each, plus each arm's
own generation and 1,000 warm-up ticks), on this machine.

### The producers, both arms

Stands / foliage / reserve / wood, summed per species:

| t (s) | arm | springturf | bloomcrown |
| --- | --- | --- | --- |
| 0 | plant-only | 8 / 0.4800 / 0.1200 / 0.2400 | 8 / 4.8000 / 1.2000 / 2.4000 |
| 100 | plant-only | 8 / 0.5331 / 0.0666 / 0.2666 | 8 / 4.7242 / 1.1671 / 2.4245 |
| 200 | plant-only | 8 / 0.5886 / 0.0736 / 0.2945 | 8 / 4.7413 / 1.1949 / 2.5551 |
| 300 | plant-only | 9 / 0.5812 / 0.0643 / 0.3049 | 8 / 4.7955 / 1.2542 / 2.7555 |
| 400 | plant-only | 10 / 0.5333 / 0.0257 / 0.3020 | 11 / 4.8993 / 1.3096 / 2.9055 |
| 0 | grazed | 8 / 0.4800 / 0.1200 / 0.2400 | 8 / 4.8000 / 1.2000 / 2.4000 |
| 100 | grazed | 8 / 0.5331 / 0.0666 / 0.2666 | 8 / 4.7242 / 1.1671 / 2.4245 |
| 200 | grazed | 8 / 0.5886 / 0.0736 / 0.2945 | 8 / 4.7413 / 1.1949 / 2.5551 |
| 300 | grazed | 8 / 0.0011 / 0.0000 / 0.2647 | 8 / 1.7812 / 1.0468 / 2.6770 |
| 400 | grazed | 8 / 0.0004 / 0.0000 / 0.2167 | 11 / 1.2304 / 0.5767 / 2.7548 |

The arms are **identical to four decimals through 200 s**, which is what "the same conditioned
world" means here: the animals arrive at 200 s and everything before that line is one run
computed twice.

### The animals

| t (s) | animals | mean body | mean reserve | bites | steps | born | deaths |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 0 | — | — | 0 | 0 | 0 | 0 |
| 100 | 0 | — | — | 0 | 0 | 0 | 0 |
| 200 | 4 | 0.05000 | 0.02500 | 0 | 0 | 0 | 0 |
| 300 | 58 | 0.03116 | 0.00458 | 38,197 | 463 | 54 | 0 |
| 400 | 79 | 0.02679 | 0.00313 | 51,833 | 648 | 75 | 0 |

### What was eaten, by the species it came off

| species | bites | organic |
| --- | --- | --- |
| bloomcrown | 45,448 | 4.106674 |
| springturf | 6,385 | 0.623095 |

**87 % of the intake came off the species round 5a's probe could never reach**, and that is
the first result. The reach rule compares a crown's **absolute** height with the eater's own
ceiling (`stand.y + crown_voxels <= from.y + reach.up`), so a bloomcrown whose crown sits two
voxels over a `y13` face is out of reach from that face and in reach from a `y14` one. A
scripted harvester standing on three fixed faces therefore ate no bloomcrown at all in round
5a; a **walking** browser finds the face it can eat from, which is Astra's round-8 point
measured rather than argued: what a consumer eats is a fact about where it stands, not a
property of a preset. (The harness records the species of every bite but not the face it was
taken from, so *which* faces did it is unattributed — a follow-up that wants that attribution
has to record it.)

### The three residuals, and the union of the two ledgers

| arm | consumed − eaten | received − deposited | flora residual | fauna residual |
| --- | --- | --- | --- | --- |
| plant-only | 0 / 0 / 0 | 0 / 0 / 0 | −9.08e-13 of 12.2161 | 0 of 0 |
| grazed | 0 / 0 / 0 | 0 / 0 / 0 | 3.17e-12 of 6.5157 | −1.03e-11 of 2.3638 |

**Two different checks, and only one of them is exactly zero (Astra R9.2).** The
**transfer mismatch** — what one layer says it handed over against what the other says it
received — is exactly zero in all three currencies, by construction rather than by luck:
`Flora::take_foliage` returns the `Taken` it books and the animal books the same value, so
`consumed_organic_out == eaten_organic_in` to the bit, and a deposit is the same triplet
seen from its two sides. The **stock residuals** are a separate check and are float noise,
not zero: the two published organic residuals sum to about −7.13e-12 (3.17e-12 in the flora
and −1.03e-11 in the fauna), the same 1e-12 relative noise rounds 3, 4 and 5a reported, now
with 4.729769 of organic matter, 0.0959539 of mineral and 9.459538 of energy crossing
between two layers that each account for it independently. Neither number is a nutrient
sufficiency claim.

One caveat for any harness that compares the two sides: the plant layer's `deposited_*_in`
counts **every** deposit it received, so a run whose harness also lays material — the
`community` run's eight declared logs, for instance — has to subtract those external
deposits before the animal layer's `deposited_*_out` can be cancelled against it. In the
grazed arms the animals are the only depositors, which is why the equality holds as printed.

### What the run says, in plain words

1. **Every flow is paid, and the ledgers close together.** Four introduced bodies, 51,833
   bites, 75 births, 648 steps and 4.73 of organic matter moved between the layers, with both
   residuals at 1e-12 relative and the union at zero.
2. **Four founders and their descendants stripped the meadow in 200 s and nothing died.**
   Four animals were introduced and 79 were eating by the end, so "four grazers" names the
   treatment and not the mouths. Springturf foliage went
   from 0.5886 to 0.0004 — held at zero, not recovering — and bloomcrown's from 4.7413 to
   1.2304 against a control that ended at 4.8993. No plant died in either arm: a withdrawal
   takes `P` and death is `W < alive_min`, so a browser cannot kill a stand directly, only
   take its income away (round 5a's finding, unchanged by an animal doing the taking). What
   grazing did reach was the **wood**: springturf's fell 0.2945 → 0.2167 while the control's
   rose to 0.3020, and bloomcrown's reserve fell 1.1949 → 0.5767 against 1.3096.
3. **No animal died either, and the population quadrupled twice.** 4 → 58 → 79 with **zero
   deaths**. The arithmetic, corrected (Astra R9.2): `maintenance_per_s` 0.001 on a body of
   0.05 is 5e-5 per second, so a full reserve of `reserve_cap · body` = 0.025 is **500 s**
   of standing still — about **100 s** once the animal has paid 0.01 each for two newborns
   — and then thousands of seconds of body before `body_min` (`0.05 · e^{−0.001 t}` reaches
   0.005 at ≈ 2,300 s). The earlier "~200 s" was wrong. The 400 s window is therefore
   shorter than this placeholder's starvation time from a *full* reserve, and **the falling
   mean body (0.0500 → 0.0268) and mean reserve (0.0250 → 0.0031) are composition and not
   shrinking individuals**: 75 newborns arrive at `body_min` 0.005 and pull any mean down on
   their own. Reading them as "what the crash looks like before it arrives" was an
   inference this run cannot support. What would support it is founder-versus-descendant
   stocks **by identity**, which the harness did not record then and does now — a 60 s
   two-grazer smoke on the repaired code shows founders still at body 0.05 while their
   descendants average 0.018, which is the composition effect measured. Nothing here
   measures whether a crash arrives, and nothing here is a viability claim.
4. **Births are bounded by income and by nothing else.** There is no mating system, no
   gestation and no refractory period this round (stated in the crate doc): an adult pays
   `birth_cost` 0.01 out of its reserve whenever `body >= birth_body` 0.03 and the reserve
   allows. So the population tracks the food the mouths can earn, with a lag of one body's
   growth, which is exactly what 4 → 79 while the foliage went to zero is.
5. **The producers' recruitment differed and this run cannot attribute it.** Five
   establishments in the plant-only arm against three in the grazed one, and the grazed arm
   ended with 8 springturf against the control's 10. Two lottery draws in one arm are not a
   mechanism, and this note states **no expected direction** for them: recruitment here
   runs through a funded bank, a keyed lottery and each arm's own water and shade, and
   Astra's R8.3 is that a one-arm difference of two draws does not identify any of those.
6. **Excretion happened, and the 7.6e-6 of mineral deposited in 400 s is it** (Astra
   R9.2; this note previously said the opposite in the same sentence as the number). Dung is
   the mineral a bite carries in excess of the tissue **actually placed**, and that is not
   `yield_fraction`'s business: an animal at `body_max` with a full reserve places only what
   its own upkeep just freed, so most of the bite's mineral has nowhere to go and is
   excreted. Plant `n_tissue` is also not current foliage mineral density — respiration
   retains mineral, so a stand's tissue drifts richer than 0.02 — which moves the threshold
   the other way too. What *is* true at the placeholders is narrower: a **fresh founder's
   first bite** carries no excess, because 0.05 of tissue mineral against a plant's 0.02
   leaves a young animal needing every unit it can fund. Both cases are pinned in
   `crates/cubarium-voxel-fauna/tests/round5c.rs`, together with a mineral-rich food case at
   unchanged grazer knobs.

### Not established

Anything about viability, carrying capacity or a population target. Whether this animal can
persist — the run ends before its own starvation time, with zero deaths, which is not
evidence either way. Whether four is a reasonable number of founders, whether the halfway
introduction is a reasonable time, or whether any of the sixteen placeholders is a reasonable
value: nothing measured any of them, and no result above depends on one. Which faces the
bloomcrown bites were taken from. And anything about the *look* of a consumer: the presenter
draws an interim 2×1×2 block in a placeholder colour, and the art direction is Wrysk's own
thread. (The `--sink gpu` omission recorded here is stale: round 5c's own staging work gives
an animal precedence over the plant in its cell on the GPU path too.)

**This run predates the R9.1–R9.6 repairs and would not reproduce.** The mineral budget now
bounds what a bite builds (R9.1), sensing is bounded in candidate-face coordinates (R9.5)
and a mycelium box reaches one row up as well as down (R9.3), so every growth, population
and spread number above belongs to the code at 680a9af. What survives the repairs is the
accounting: paid flows, matching transfers, and residuals at float noise.
draws an interim 2×1×2 block in a placeholder colour, the art direction is Wrysk's own thread,
and the `--sink gpu` path does not draw animals at all yet because the staging that would feed
it lives in `crates/cubarium/src/sink/**`, which package O was told not to touch.

## Replacement harness smoke — 2026-09-17

Package P built the **replacement-control study harness** Astra designed in R5.4 and
corrected in R7.2, and ran **one short smoke of it**. This is the harness working, not the
study: the controls and the probe are a separate run Wrysk authorises, and nothing below is
coexistence evidence, an invasion, a preset gate or a clearance condition.

```text
cargo run --release -p cubarium-voxel-flora --example replacement -- \
    bloomcrown umbrellafrond 1000 1 101 --cap 300
```

**Wall time: 836.5 s** — 1,000 s of conditioning (20,000 coupled ticks) plus **seven** arms of
300 s (6,000 coupled ticks each, 87.0–90.6 s per arm, about 68 ticks/s), on this machine. Run
**twice** (849.5 s and 836.5 s, one of them before a wording fix to two printed lines): the two
runs are **identical line for line** apart from those two lines and the wall times, so every
number here is reproducible from the command above.

Seven arms and not nine: the resident-only control introduces nothing, so with one conditioned
state and one forcing three copies of it would be bit-identical, and it is run once and
reported once.

### The predeclared cap, printed before anything moved

| species | newborn → donor at the growth cap | one funded package | cap |
| --- | --- | --- | --- |
| bloomcrown | 0.02 → 0.3 at `wood_rate` 0.001 = 54,163 ticks = **2,708.15 s** | 0.05 (= 0.02/0.4) at `propagule_rate` 0.0002 net of `build` 0.2 = 6,000 ticks = **300.00 s** | **3,008.15 s** |
| umbrellafrond | the same preset numbers = **2,708.15 s** | the same = **300.00 s** | **3,008.15 s** |

Pair cap **3,008.15 s**, the larger of the two. The smoke's `--cap 300` is **10.0 %** of it, and
the harness says so in its own header: a window this short cannot resolve a replacement, and an
arm that has not completed one is reported as *unresolved at cap* rather than as exclusion.
Astra's stonecushion correction is in the same arithmetic — 160,945 ticks = 8,047.25 s plus
600 s, so 8,647.25 s, which a 6,000 s cap cannot resolve — and `observation_cap` reproduces
both to the hundredth of a second (`examples/harness/mod.rs`, `cap_tests`).

### The conditioned state

3,072 skyline columns; **3,072 support faces provisioned at creation** with 1.0 of mineral
each, booked once as `seeded_mineral_in` 3,072.0. That is the R5.4 requirement in force: no
arm can import mineral by colonising further, so fertility is matched across the arms by
construction rather than by hope.

Eight bloomcrown founders at wood 0.3 on 96 gate-passing `Ridge` candidates, every founder's
gate values printed by identity (all at y13, mean root-box pore 0.219–0.283 against its 0.10
floor, saturated fraction 0.000, sky 0.615–0.874 against its 0.60 floor). Over 1,000 s the
head fell 2.987 → 2.501 m with storage settling — the last interval is **+0.0001 m** and
+0.0207 m³/s of storage against 0.0384 in and 0.0176 out — and the eligible sets moved a long
way while it did: bloomcrown's 2,771 → 1,560 columns, umbrellafrond's 427 → 2,429. An eligible
count is a reading of its own moment (R5.2/R6.3) and these are the moments.

The resident **grew**: 8 founders → **19 stands**, 11 establishments, 0 deaths, a seed bank on
9 sites holding 0.37202. Residuals at the end of conditioning: organic −1.25e−11, mineral
−1.36e−12, energy −2.49e−11 against stocks of 11.6, 3,072.2 and 23.2.

### The three predeclared sites, declared once and reused by every arm

| site | column | recipients | gate values at the declared site |
| --- | --- | --- | --- |
| 1 | (41,0) y8 | **5** eligible of 5 candidate faces in hop 1 | mean pore 1.000 (≥0.45), saturated 1.000 (≤1.00), water 0.000 m, sky 0.718, 23 soil voxels |
| 2 | (32,18) y18 | **6** eligible of 8 | mean pore 0.521, saturated 0.267, water 0.000 m, sky 0.760, 30 soil voxels |
| 3 | (57,15) y25 | **8** eligible of 8 | mean pore 1.000, saturated 1.000, water 0.000 m, sky 0.802, 11 soil voxels |

Drawn as a spread of umbrellafrond's whole ordered `Hollow` pool — 2,429 columns pass its own
predicate in the conditioned state, 2,411 of them unoccupied. The list is **not re-derived per
arm**: `tests::the_declared_site_list_is_one_list_and_an_arm_would_derive_another` shows that
the exclusion arm's own pool declares a *different* list, which is exactly what R7.2 and
package L's note warned would silently unmatch the arms.

### The arms

Every introduction arm ran the same course, at all three sites and in both treatments:

| arm | resident stands at 100/200/300 s | newcomer | descendant births | losses | survivors | founder parcel at 300 s | deliveries |
| --- | --- | --- | --- | --- | --- | --- | --- |
| resident only (control) | 19 / 19 / 19 | 0 | — | — | — | — | — |
| site 1, resident + newcomer | 19 / 19 / 19 | 1 | 0 | 0 | 0 | 0.05000 | 0 |
| site 1, resident excluded | 0 / 0 / 0 | 1 | 0 | 0 | 0 | 0.05000 | 0 |
| site 2, resident + newcomer | 19 / 19 / 19 | 1 | 0 | 0 | 0 | 0.05000 | 0 |
| site 2, resident excluded | 0 / 0 / 0 | 1 | 0 | 0 | 0 | 0.05000 | 0 |
| site 3, resident + newcomer | 19 / 19 / 19 | 1 | 0 | 0 | 0 | 0.05000 | 0 |
| site 3, resident excluded | 0 / 0 / 0 | 1 | 0 | 0 | 0 | 0.05000 | 0 |

Every one of the six introduction arms printed **"recruitment NOT OBSERVED within 300 s …
unresolved at cap"**, which is the correct reading and not a disappointment: see finding 1.

### What the smoke establishes

1. **The window was the package term of the cap and nothing more, and the harness's own
   arithmetic predicted exactly that.** The introduced founder starts at wood 0.3, which is
   `donor_min` for this preset, so it reaches donor size on its first tick — the harness prints
   that as the founder's own event and says it is *not* a descendant — and then funds
   `propagule_rate · dt / (1 + c_g)` = 8.3333e−6 per tick. **Corrected (Astra R10.1): that is
   not 0.05 exactly and the tick matters.** 6,000 repeated additions of that increment give
   `0.049999999999996936`, which is *below* the package `0.049999999999999996` — "0.05000" in
   the table above is rounded display, not exact funding — so the 6,000th tick does **not**
   deliver, and the honest count is **6,001 ticks (300.05 s)**. The delivery then happens in
   that same tick, **after** its own funding: `src/step.rs` funds and sends in one phase and
   there is no separate delivery step. No model epsilon and no rate tuning is called for by
   this; the arithmetic that has to change is the harness's, and R10.1's earliest-possible
   timeline now carries it. The arm still ends with the first package saved and not sent, and a
   birth still needs that delivery, a germination tick and then 2,708.15 s of capped growth
   before a *descendant* could donate: 300 s could not have produced a replacement, and no arm
   inferred exclusion from the fact that it did not.
2. **The exclusion arm excludes, and retains everything else.** 19 of 19 stands cleared and 9
   of 9 seed banks removed, booked out as `removed_*` (organic +8.4229, mineral +0.1737), with
   stored water 210.8303 m³, head 2.501 m, litter 3.18641, dead wood 0.00000, soil mineral
   3,071.9297 and imported mineral 3,072.2 **all unmoved** — checked to 1e−9 by the harness
   itself, which refuses the arm rather than reporting it if any of them shifts or if anything
   of the resident survives. A `Clear` alone would have left the 9 banks germinating, which is
   why `Command::ClearBank` exists (`tests/replacement.rs`,
   `clearing_a_resident_without_its_bank_leaves_it_able_to_come_back`).
3. **The resident is not declining, so it would not have disqualified a longer arm.** 8 → 19
   stands over conditioning and 19 → 19 through the control window, with zero deaths. R5.4 is
   explicit that a newcomer increasing against a declining resident is not evidence, and this
   is the arm that says which case we are in.
4. **Fertility is matched and the cloning is exact.** Every arm carries the same 3,072.2 of
   imported mineral; the only ledger difference the exclusion arm has is the removal it
   declared. Residuals stayed at 1e−11 relative through the branch, the removals and the
   arms — organic −1.25e−11 to −1.84e−11 — and a cloned state is pinned to step identically
   for ten ticks by `a_cloned_conditioned_state_steps_identically_for_ten_ticks`.
5. **Cost, for sizing the authorised run.** About 67 coupled ticks per second of wall time on
   this machine with all 3,072 faces provisioned. One **full** control at the predeclared cap
   for this pair is 3,008.15 s = 60,163 ticks ≈ 15 minutes of wall time per arm, so seven arms
   is about **1.8 hours**; a stonecushion pair at 8,647.25 s per arm is about 5 hours for
   seven. R5.4's probe window of 3 × the larger measured G is a multiple of that again. Those
   are the numbers an authorisation is choosing between, not a claim that the run is worth
   making.

### What it does not establish

Anything at all about coexistence, invasion, replacement time G, or these two species'
relation to each other: no arm ran long enough for a single generation and the harness says so
in its own words. Whether 1,000 s of conditioning is the right amount, whether three sites is
enough, whether `seed` 1 / `noise_seed` 101 is representative, and whether any of the presets'
placeholders is a reasonable value — nothing here measured any of them, and no number above
depends on one. The other direction of this pair (umbrellafrond resident, bloomcrown newcomer)
was **not run**: it is the same command with the two species exchanged, and coexistence
evidence needs both directions.

### After Astra round 10 (R10.1–R10.4) — 2026-09-17

The harness changed under the four round-10 items and was re-smoked **short**, as a pilot:
`replacement pilot bloomcrown umbrellafrond 600 1 101 --cap 60` in `--release`, **135.6 s** of
wall time (12,000 conditioning ticks in two phases, then 1,200 ticks of one arm in 11.4 s). No
long run was made and the tables above are unchanged; this is what the repaired harness now
prints.

| what | before round 10 | now |
| --- | --- | --- |
| the observable window | published cap **3,008.15 s** (a newborn's clock) | earliest-possible **bound 3,308.30 s** = fund 6,001 t + germinate 1 t + grow 54,163 t + refund 6,001 t; stonecushion **9,247.40 s** |
| the budget | the cap, doubling as a sufficient window | a separate **stopping budget**; a `full` study at or below the bound is **refused** and pointed at `pilot` |
| conditioning | a fixed 1,000 s, then accept | two phases against one budget (half to the hydrology), tolerances **5 % of rain / 0.001 m / 2 % turnover on 2 intervals** |
| phase A (hydrology alone, 300 s) | — | **unresolved**: storage −0.0116 m³/s = **30 % of rain** every interval, head −0.059 m per interval, umbrellafrond turnover 9.1 % |
| phase B (with the resident, 300 s) | — | **unresolved**: storage 30 % of rain, head −0.054 m, turnover bloomcrown 13.3 %, umbrellafrond **26.4 %** |
| the arm (60 s) | — | founder funded 0.01000 in 1,200 ticks, **0 deliveries, 0 births** → *unresolved at the stopping budget (1.8 % of the bound)* |
| delivery destinations | the largest bank increase, shared by every donor that tick | the model's own `DeliveryReceipt { donor, recipient, organic, mineral }` |
| `--cap 300` after three positionals | also set the **world seed** to 300 | consumed with its value; seed stays 1 |

The substantive finding is the conditioning one: on the published conditions this world **does
not settle**. The outlet exports 0.050 m³/s against 0.0384 m³/s of accepted rain, so storage
falls at a constant −0.0116 m³/s and the head drops ~0.06 m per 100 s throughout — and the
eligible sets move with it, umbrellafrond's by a quarter of its union in one interval while
bloomcrown's shrinks. A fixed 1,000 s of "conditioning" was measuring the middle of that
drainage. No tolerance was loosened and no gate was touched to make it pass (R10.2).
