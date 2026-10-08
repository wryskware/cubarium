# SHREDDER-PLATEAU handback (2026-10-08, read-only, no sims run)

Sources: runs/bl3b-m120-2026-10-08/heuristic/{s1,s4}/vitals/vitals.csv and s1/census.csv, report.md;
config/desktop/terrarium-d72.toml:361-395 (shredder), :115-117 (decomposition);
crates/cubarium-voxel-fauna/src/body.rs:1735-1840 (Blind physiology), :1576-1660 (Gut);
step.rs:775-787 (eating latch), :818-832 (full event), :885-899 (gut_bite), :911-968 (digest),
:1065-1093 (grow), :1103-1113 (hunger), :395-420 (brood pays its share); lib.rs:1946-1958 (brood_target);
controller.rs:702-716 (BlindForager "stay on food").

## 1. What sets "0.19" — there is no single plateau
The 0.19 is a late-window average of a synchronised sawtooth (every seed starts from the same
cohort structure, hence the narrow band). Per-minute adult p50, s1 (s4 identical within 0.01):
m128-176 0.160 -> 0.153 (falling 0.0012/min), m184 0.194, m192 0.254, m200 0.103 (22 juveniles
cross birth_body and enter "adult" at their juvenile fullness), m208-240 0.107 -> 0.336 (+0.0079/min).

Live numbers: body_max 0.0125x30 = 0.375; birth_body 0.15, body_min 0.045 (d72 cfg); reserve_cap 0.5
(cap = 0.5 body); gut k 0.02, tau 120 s, mass_exponent 0.75; litter yield 0.478; upkeep
0.03/7200 = 4.17e-6 /body/s; growth_max_per_s 1.163e-4; brood_share 0.8; egg 0.054, floor 0.0135.

Gut is the binding rate: stomach_fill p50 0.88-0.93 all run (latch refills 0.88K -> 0.98K, step.rs:784/826;
s_start capped at 0.98-0.10). Intake = digestion K/tau x fill x (b/0.375)^-0.25.
- Juvenile (b~0.10): 0.002x0.9/120x1.39 = 2.09e-5 raw/s; x0.478 = 1.00e-5; -upkeep 4e-7 => net 9.6e-6/s.
  grow() draws growth_max x fullness, so reserve settles where net = 1.163e-4 x f => f* = 0.083.
  Observed juvenile p50 0.07-0.10. THIS is the real plateau: by design (growth on plant time eats the surplus).
- Adult (b~0.15): 0.003x0.9/120x1.257 = 2.83e-5 raw/s -> net 1.29e-5/s = 7.7e-4/min. Growth takes the
  free 20%; brood keeps 80%: 6.2e-4/min / cap 0.075 = +0.0082/min. Observed +0.0079/min. Match.
- Falling phase m128-176: asleep_n 7-8 = the adults. DarkActive (cfg :376) sleeps them while their face is
  lit and hunger < wake_hunger 0.9; they pay sleep upkeep and lose ~0.0012/min. Juveniles at f~0.08 have
  hunger 0.92 >= 0.9 => permanent emergency => awake (step.rs:691-695), never clearing (needs hunger < 0.6).
World totals check: intake 0.034-0.078/min raw (41 bodies) vs gut ceiling sum ~0.075/min. Upkeep 0.0015/min.

## 2. Choice, not failure
BlindForager has no sated rule: tasting litter > feed_gate it holds still and feeds (controller.rs:702-716);
the latch cycles every ~13 s (0.1K drains in 0.1x120/0.93 s), feeding_share ~0.47, so it never needs to move.
They sit on litter they can eat and are gut-limited, not stock-limited. I did not measure litter within one
body length (needs a probe with code); indirect evidence: stomach 0.9 full, bite not refused, 0 starving adults.

## 3. Litter budget — shredders are not the litter sink at any plausible size
Litter stock s1: 4,571 -> 9,437 (m180) -> 3,800 (m210) -> 8,126. Decomposition k_d = 6.886e-4/s (cfg :116,
half-life 17 min): flux k_d x S ~ 0.041 x 6000 = ~250/min. Shredder intake ~0.06/min = 0.02-0.03% of it.
Inflow ~ dS/dt + k_d S ~ 250-420/min. Ceiling at 41 shredders all at body_max: 0.02x0.375/120x60 x41 =
0.15/min; matching even the 175/min end slope needs ~48,000 adult shredders. So litter is set by
inflow/k_d (S* = inflow/k_d) and the leaf-fall pulses; the scorer's "shredders: access and intake" is a
misattribution, not a rate or role gap the shredder can fill.
Breeding limit (hatchings 9, eggs_laid 24 -> 33 in s1): brood_target = 1 egg + floor = 0.0675 at birth_body
cap 0.075 (lib.rs:1950) => must reach fullness 0.90 to lay; from 0.10 at +0.0082/min awake = ~98 awake min,
but adults sleep the lit ~2/3 of each 120-min day and lose ground. ~1 egg per adult per 2+ h.
Generation: hatchling 0.045 -> 0.15 at ~1e-5/s net ~ 10^4 s ~ 3 h. nest brake (8 within 1 m) not measured.

## 4. Options (not implemented)
A. Fix the books, not the animal: credit litter to decomposition (k_d S) in the scorer and judge shredders by
   their own condition/breeding. Visible: none. Predicted: shredder share of litter loss 0.02%, flag clears.
B. Role/behaviour: a shredder under litter cover reads as dark (it lives in the litter) or stays awake while on
   food. Visible: adults feeding by day. Predicted: ~+0.008/min continuously, egg every ~100 min, eggs ~9 -> ~18/2h,
   adult band widens; litter unchanged.
C. If a visible litter consumer is wanted, it must be the decomposer role (a litter mould/fungus taking k_d's
   flow, like glowcap on wood). Predicted: litter totals unchanged unless k_d moves into it; shredders unaffected.
