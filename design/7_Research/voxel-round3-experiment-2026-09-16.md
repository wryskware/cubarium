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
`establish_pore_min` of 0.45 sits above where drained soil settles (the mean root-box pore
over the whole skyline is 0.40).

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
highest eligible faces, umbrellafrond on the lowest, springturf on the wettest eligible
root boxes, stonecushion on eligible faces that are **not** soil, velvetpad inside a taller
founder's crown. That table is an experiment condition and not a model rule.

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

Two of the eight velvetpad founders were refused in the default arm: the `UnderACrown` rule
picked columns an earlier founder already stood on, and `Command::Seed` refuses an occupied
site. The printed count is the six that were planted.

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

1. **The shut gate for further recruitment is the bank, in every species, in both arms.**
   The germination diagnosis has every non-empty bank at **90.5 % to 93.3 %** of its own
   threshold and **zero** banks over it: bloomcrown 90.5 %, springturf 91.2 %, velvetpad
   92.1 % in the default arm, and 90.5 %, — and 93.3 % in the reseed. A package minus its
   own attrition is what a bank holds between deliveries, and a second package has to arrive
   before the first bin ages out. The predicate is not what is refusing these sites: of
   velvetpad's three banked sites in the default arm, zero pass, but in the reseed one of
   two does, and either way none of them has a whole package in it.
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
   reserve. Raising `propagule_rate` further would create no income at all. And its mean
   aeration stress is **0.862**, the highest of the five by a wide margin: the harness plants
   it on the *wettest* eligible root boxes, which is exactly where its own
   `establish_saturated_max` 0.3 bites hardest. That is a disagreement between the placement
   rule and the preset, not a model fault — but it means the springturf cohort in these two
   arms was earning about a seventh of its potential income for the whole run, and the
   "pioneer of open **moist** soil" role has to decide whether "moist" means the wettest
   ground available or merely not dry.
4. **Springturf's light gate is the only large sole-cause count of the five.** 293 columns
   refuse it on `establish_light_min` 0.75 alone (286 in the reseed), where bloomcrown's 0.6
   refuses 2. If this species turns out to be too rare, that is the first number to look at —
   and it is worth noting that the same threshold is the one the model cannot use for the
   role's own "loses under a canopy", because germination light is geometric sky with no
   canopy in it.

Umbrellafrond is unchanged from round 3 on purpose: 2,377 columns refused on **mean pore
alone**, its `establish_pore_min` 0.45 sitting above where drained soil settles (skyline mean
0.402). Astra's R6.1 said to keep the wetland role rather than lower that floor as a repair,
and it was kept.

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
