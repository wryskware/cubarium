---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# The controlled form × diet factorial: is the skimmer's loss its diet or its body?

Workstream J of [the reconciled next steps](ecology-v1-next-steps-results-2026-09-16.md),
from the brief at
[`design/handoffs/ecology-v1-diet-factorial-opus-2026-09-16.md`](../handoffs/ecology-v1-diet-factorial-opus-2026-09-16.md).
Evidence, not a decision. Nothing here proposes a configuration and nothing here
was tuned.

Workstream F
([the movement note](ecology-v1-movement-2026-09-16.md), "The skimmer: which leg
is it?") measured that skimmer-rigged bodies carrying a foliage-end diet survive
at 84 % where the founder's `diet = 0.60` survives at 20 %, and said plainly
what that is worth: an **association**. Every foliage-diet skimmer in those rows
is a descendant whose diet mutated upward, so the comparison carries later
birth, right-censoring, selection into a mutant lineage and possible mutation at
other loci along with it. F named the clean test: clone the founder at the same
tick and cells with only `diet` changed, then cross a fixed diet over forms,
with mutation and reproduction off and E's per-body ledger reading yield by
channel. This is that test.

**It reverses the sign.** At founding, in matched cells, moving *only* `diet`
from 0.60 to 0.85 takes the skimmer from establishing 13 times in 16 to
establishing once in 16, on every seed. The founder diet is the better of the
two for that body, and it is not close.

---

## The design

Three arms, each eight cloned founders placed into a whole `fast-leaf` world at
tick 0 alongside the ordinary 24 legacy founders, who are left breeding so the
food competition is the real one. The same eight cells in every arm at a seed.

| Arm | What varies | What is held |
| --- | --- | --- |
| **A** — diet within body | `diet`: four clones at 0.60, four at 0.85 | the roster **skimmer** genome, locus for locus, everywhere else |
| **B** — body within diet | the roster body: two each of burrower, grazer, glider, skimmer | `diet = 0.85` for all eight |
| **C** — the founder pairing | the roster body, each at its own roster diet | nothing — this is the observational baseline under the same controls |

At `diet = 0.85` the detrital gate shuts (`1 − 0.85 = 0.15 < θ = 0.2`), so every
arm-B clone is a **pure foliage feeder** and no difference between them can be a
diet difference in disguise.

**Two cells of the factorial are free internal controls**, and both were checked
rather than assumed: the roster grazer is already at 0.85, so arm B's grazer is
arm C's grazer; and arm A's high half is arm B's skimmer. Both pairs stand in
the same cells with the same genome and differ only in the rest of the world
around them.

### What is controlled, and the confound that is not

Controlled: **birth tick** (every clone founded at tick 0), **place** (the same
eight cells, all three arms, each seed), **lineage** (no clone reproduces),
**mutation** (mutation only reaches offspring, and these have none), and the
**ecology** (one `fast-leaf` world per seed).

Not controlled, and named: **the roster bodies differ in more than `form` and
`diet`.** The skimmer is `depth 0.10, speed 0.9, size 0.9, swim 1.0,
metabolism 0.7` against the grazer's `0.55, 1.0, 1.0, 0.0, 1.0`
(`crates/cubarium-core/src/config.rs:496-521`). **Arm A holds every one of those
fixed and is therefore clean.** Arm B deliberately does not: "body" there means
the whole roster body, and the arm cannot say which locus carries the effect. It
is very likely `depth` — see the habitat section.

### Reproduction and mutation, switched off with what already existed

`WorldConfig` has no reproduction switch. What exists is the diagnostic seam the
ES fixtures use: `World::set_scripted_intents` with
`ScriptedIntent { bud: Some(false), .. }`, which can only *suppress* a bud
(`d.bud = d.bud && b`) and grants nothing. The eight clone ids are given that
intent once at founding; generation-checked `OrganismId`s mean a reused slot can
never inherit it. **Reproduction is off for the clones alone** — the legacy
founders go on breeding, 42–145 births per run. Mutation needed no switch:
mutation reaches offspring only, so a clone that never buds never mutates.
Measured, not asserted: **0 clone births in all 96 clones.**

### Where the eight clones stand, and why the habitat stratification was abandoned

The brief's design puts the skimmer's niche — "algae on the wet floor" — into
the placement. Two versions of that were written and **the world refused both**.

1. *Four deepest pools against four dry cells.* The deepest pools hold no food.
   Over the four training seeds, **39 of the 40 deepest cells carry zero mean
   foliage**; the fortieth carries 0.025 m against a typical dry cell's 0.2.
   Ecology v1 grows foliage only in a cell that carries wood
   (`crates/cubarium-core/src/fields.rs:398-409`: subphase 3a runs only for
   `CellClass::Alive`, and the income term is proportional to the foliage
   already there). `water.algae_light` raises a *living* cell's light floor; it
   does not create a producer where there is no wood. **There is no separate
   algae larder on the wet floor to stand in.**
2. *Four wet **vegetated** cells against four dry vegetated ones.* Those barely
   exist, and not on every seed. The landscape census below finds, at seed 1002,
   68 cells of 1,280 with any standing water and **6** of those carrying any
   foliage — nowhere near four that are also three cells apart.

A design that is stratified on one seed and not on another is two designs, so
habitat is **measured instead of assigned**: eight fixed anchors spread over all
five faces, each resolved to the nearest *living* cell on its own face at least
three cells from the ones already placed. Ranking the eight by foliage was
rejected in the other direction — it would have handed the foliage diets the
better ground and the detrital diets the worse. Every clone's own time on wet
ground is then probed once a simulated second through the run.

**The landscape census** (mean over the second half of a 24,000-tick clone-free
warm-up; `runs/ecology-v1-diet-factorial/cells.txt`):

| seed | dry (≤0.001 d) | of those, fed | 0.001–0.05 d | fed | 0.05–0.15 d | fed | >0.15 d | fed |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1001 | 939 | 929 | 151 | 129 | 66 | 36 | 124 | **12** |
| 1002 | 1212 | 1112 | 31 | 5 | 31 | 1 | 6 | **0** |
| 1003 | 661 | 661 | 86 | 86 | 76 | 76 | 457 | 256 |
| 1004 | 910 | 910 | 46 | 46 | 55 | 55 | 269 | 120 |

Standing water and food are anticorrelated everywhere and nearly disjoint on two
of the four seeds, and the *deepest* water carries none on all four.

---

## Build, commits and commands

| | |
| --- | --- |
| Branch | `worktree-agent-a6fac8fbf384a4229` (a worktree of `main`) |
| Parent | `2a1cedd` |
| Test-authoring commit | `d0daef1` — 27 definition tests against a stub; 11 in the core file (which does not compile) and 16 in the search file, all red |
| Core commit | `c469569` — the third founding door |
| Implementation commit | `2368073` — `factorial.rs`, its `lib.rs` line, the `factorial` subcommand |
| Census commit | `a4dc519` — the ten-deepest-cells print |
| Result commit | `dc0a844` — this note |
| Tidying commit | `7e0041f` — the subcommand's body moved out of `main.rs` |
| Build stamp in every row | **`7e0041f15fcb`** |
| Host | 8 workers, as the brief caps |

Files touched, and only these: `crates/cubarium-core/src/world/lifecycle.rs`
(the one door), `crates/cubarium-core/tests/found_with_genome.rs` (new), the new
`crates/cubarium-search/src/factorial.rs` with its `lib.rs` module line and one
dispatch line in `main.rs`, `crates/cubarium-search/tests/diet_factorial.rs`
(new), this note, and `runs/`.

### The core change: one founding door

`World::found_animal_with_genome(pos, heading, genome)` beside
`found_training_animal` and `found_neural_animal`
(`crates/cubarium-core/src/world/lifecycle.rs`). Neither existing door can put a
*stated* genome into a live world at a chosen cell and tick: the roster path in
`World::new` runs only at creation over the configured kinds, and the training
door fixes the genome to the unit adult.

The body is the body the training door founds — adult at founding,
`R = 0.5 R_max`, `E = 0.75 E_max`, `Mode::Seeking`, full hunger memory,
`Origin::Founder`, `born_tick` = the current tick, `structure + reserve` booked
into `external_material_in`. Rather than duplicate that definition, both public
doors now call one private `found_body`, so a founded adult is still described
in exactly one place. `found_training_animal` keeps its behaviour bit for bit
and is not subject to the new door's genome refusal.

Three refusals, each by name, each leaving the world untouched: capacity, a zero
heading, and a genome outside the genome's own bounds. The third **refuses
rather than clamps** on purpose — silently founding a `size = 2` animal for a
caller who asked for `size = 9` would corrupt a control without saying so.

The clones' genomes are **read off the built world's own founders**
(`Roster::of`), not rebuilt from the kind rules, so a clone is the roster body by
construction. That every founder of a kind carries one genome is itself a test.

### Tests

| Command | Result |
| --- | --- |
| `cargo test --release -p cubarium-core` | **487 passed**, 0 failed, 3 ignored — 476 before, plus the 11 here |
| `cargo test --release -p cubarium-search` | **138 passed**, 0 failed (86 unit incl. 4 new, 3 `budget_feasibility`, **18 `diet_factorial`**, 4 `es_repair`, 12 `harness`, 15 `movement_measures`) |
| `graft build` | clean |

**33 tests are new**: 11 in `crates/cubarium-core/tests/found_with_genome.rs`, 18
in `crates/cubarium-search/tests/diet_factorial.rs`, 4 unit tests in
`factorial.rs`. The authoring order is on the record rather than asserted:
commit `d0daef1` carries **27** of them — 11 core, 16 search — against a module
that returns nothing and a method that does not exist, and **all 27 are red**
there (the core file does not compile; all 16 search tests fail). The commit
messages of `d0daef1` and `c469569` miscount the core file as ten; the counts
here are from `grep -c '^#[test]'` on the file as that commit stored it, and are
the ones to believe.

Three of the search tests were **rewritten** after the world refused the
stratified placement — they asserted "each diet gets two wet-floor and two
dry-vegetated starts", which is a claim about a design that cannot be built on
this ecology. They were replaced by tests of the anchor rule and of the measured
habitat class. That is a change to the design under test, and it is recorded
here rather than folded in quietly.

### Exact commands

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test --release -p cubarium-core    # 487 passed
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test --release -p cubarium-search  # 138 passed
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo build --release -p cubarium-search

# the landscape, before any clone was placed
./target/release/cubarium-search factorial --seeds 4 --cells-only \
  > runs/ecology-v1-diet-factorial/cells.txt

# the campaign: 3 arms x 4 training seeds, horizon 90,000
./target/release/cubarium-search factorial --seeds 4 --workers 8 \
  --out runs/ecology-v1-diet-factorial
```

Defaults used: `--ticks 90000 --warm-up-ticks 24000 --probe-every 20
--drain-every 250 --wet-min 0.05 --seed-set training`.
`runs/ecology-v1-diet-factorial/{runs.jsonl,summary.json,cells.txt,report.txt}`,
12 rows, 96 clone records, **128 KiB**.

### The reproduction checks, before anything was interpreted

1. **Arm A re-run at a different worker count** (4 instead of 8). All four seeds
   reproduce exactly — state hash, placements, stop tick and every field of every
   clone row: **4 of 4**.
2. **The whole campaign re-run after a rebuild.** **12 of 12** rows identical.
3. **The whole campaign re-run after the `main.rs` handler was moved into
   `factorial.rs`** (commit `7e0041f`, the last change in this workstream).
   **12 of 12** rows identical again, including both conservation residuals to
   the last digit. The rows in `runs/` are the ones this final binary produced.

**A build hazard, recorded because it can produce a wrong attribution.** The
worktree shares `CARGO_TARGET_DIR` with the main checkout and with other
workers. Twice during this session another tree's build overwrote the
`cubarium-search` binary, once leaving `unrecognized subcommand 'factorial'` and
once a stale `unresolved import`; `touch crates/cubarium-core/src/lib.rs` and a
rebuild cleared both. Worse, the shared build-script output made the stamp read
another tree's commit. The rows above were produced by a binary whose stamp was
pinned with `CUBARIUM_SEARCH_BUILD=7e0041f15fcb`, this branch's own HEAD, and
that campaign was verified identical to the unpinned ones row for row.

---

## The measures

Per clone, from `World::record_body_budgets` (workstream E's ledger, one record
open from the clone's first tick to its death or the run's end) plus a probe of
its cell once a simulated second:

- lifetime and death cause; whether it reached the horizon
- **served**, **digestible** (`cap · served`), **reserve credit** and **battery
  credit**, each split over the four channels foliage / fruit / litter / carrion
- `upkeep_billed`, the two motor terms, `bill_total`, `bill_paid`, billed ticks
- **net margin** = `Σ battery_credit + e_r · Σ reserve_credit − bill_total` (e),
  and per simulated second
- distinct cells; fraction of probes on a cell with standing water; fraction in
  the algae band; mean depth under the body

**Billing is not split by channel**, because the core does not bill by channel —
the motor settlement levies upkeep and two motor terms against the body, not
against what it ate. Those three are reported instead.

**A reference the tables need: the no-intake floor.** 19 of the 96 clones served
less than 0.05 m in their entire lives. They lived **308–431 s, median 375 s**.
That is what a founded adult gets from its founding stores alone, and any
lifetime near it means "this body never ate".

---

## Arm A — diet within the body

The roster skimmer, with only `diet` moved. Every other locus identical.

| | n | reached horizon | established (≥750 s) | mean life | **median life** | served f/F/l/c (m) | digestible (m) | credited (m) | billed (e) | net margin (e/s) | cells | wet probes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `diet = 0.60` (the founder's) | 16 | 0 | **13** | 1005 s | **885 s** | 1.48 / 0 / **4.31** / 0 | 2.61 | 1.33 | 5.08 | −0.00182 | 141 | 57 % |
| `diet = 0.85` (the foliage end) | 16 | 1 | **1** | 644 s | **378 s** | 1.32 / 0 / **0** / 0 | 1.13 | 0.68 | 3.48 | −0.00495 | 84 | 32 % |

"Established" means a lifetime of at least 750 s, about twice the no-intake
floor: a body that is still alive then has found food.

**Per seed, and the spread is the point — every seed agrees:**

| seed | median life 0.60 | median life 0.85 | ratio | established 0.60 | established 0.85 |
| --- | --- | --- | --- | --- | --- |
| 1001 | 850 s | 376 s | **0.44** | 3/4 | 0/4 |
| 1002 | 1133 s | 374 s | **0.33** | 3/4 | 0/4 |
| 1003 | 921 s | 388 s | **0.42** | 3/4 | 1/4 |
| 1004 | 885 s | 399 s | **0.45** | 4/4 | 0/4 |

Every clone, ordered, at `diet = 0.85`: **15 of the 16 served under 0.3 m in
their whole lives** and died at 374–432 s, on the no-intake floor. The
sixteenth, at seed 1003, served 20.4 m and reached the horizon.

```
served (m), diet 0.85:  0, 0, 0, 0, 0, 0.001, 0.004, 0.011, 0.021, 0.037,
                        0.094, 0.099, 0.126, 0.170, 0.260, 20.361
```

**The mechanism is on the ledger.** The gate `θ = 0.2` shuts the litter channel
at 0.85, removing 4.31 m of the 5.79 m the 0.60 body was eating. The foliage
channel does not make it up — it *falls*, 1.48 → 1.32 m. The body pays 2.7×
worse per second for the privilege (−0.00182 → −0.00495 e/s).

> **Does changing only `diet` from 0.60 to 0.85 rescue the skimmer body? No. It
> roughly halves its median life and takes it from 13/16 established to 1/16, on
> all four seeds, with no seed overlapping.**

---

## Arm B — body within diet, every body a pure foliage feeder at 0.85

| body | n | reached horizon | established | mean life | median life | served (m) | digestible (m) | credited (m) | billed (e) | upkeep (e) | motor (e) | net margin (e/s) | cells | wet |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| burrower | 8 | 0 | 2 | 993 s | 436 s | 2.62 | 2.22 | 1.33 | 5.33 | 4.67 | 0.66 | −0.00398 | 65 | 31 % |
| grazer | 8 | 2 | 2 | 1377 s | 347 s | 6.82 | 5.80 | 3.48 | 9.87 | 8.54 | 1.34 | −0.00482 | 162 | 30 % |
| glider | 8 | 1 | 2 | 1304 s | 369 s | 6.29 | 5.35 | 3.21 | 9.42 | 8.09 | 1.33 | −0.00458 | 174 | 9 % |
| **skimmer** | 8 | **0** | **0** | 398 s | 385 s | **0.10** | 0.09 | 0.05 | 2.19 | 1.73 | 0.46 | −0.00519 | 69 | 23 % |

Every arm-B clone's lifetime, by seed:

| seed | burrower | grazer | glider | skimmer |
| --- | --- | --- | --- | --- |
| 1001 | 431, 2178 | 351, **4500** | 328, **4500** | 374, 418 |
| 1002 | 420, 435 | 308, 344 | 376, 385 | 375, 381 |
| 1003 | 461, 3154 | 339, 363 | 350, 3814 | 379, 470 |
| 1004 | 425, 437 | 311, **4500** | 320, 362 | 390, 395 |

> **At fixed `diet = 0.85`, does form still matter? Not resolvably at this sample
> size.** Every body has the same all-or-nothing outcome and the same
> establishment rate: burrower 2/8, grazer 2/8, glider 2/8, skimmer 0/8. **0/8
> against 2/8 is not a difference four seeds can establish** (Fisher exact
> p ≈ 0.47), and saying otherwise would be reading a coin.

What the arm *does* show, strongly, is that **the all-or-nothing structure
belongs to the foliage channel, not to any body.** Across all 64 pure-foliage
clones in the campaign, **51 served under 0.5 m** in an entire life and died at
308–470 s, **11 served over 5 m** (8.1–27.2 m, of which 8 reached the horizon
and 3 died at 2,178, 3,154 and 3,814 s), and **only 2 served anything in
between**. 53 of the 64 died at or under 480 s, on the no-intake floor. The
arm-B grazer is the clearest picture of it:

```
served (m), arm B grazer:  0.034, 0.052, 0.230, 0.259, 0.300, 0.375, 26.472, 26.855
```

The skimmer's distribution has no upper mode at all — its longest of eight is
470 s and its largest meal in a whole life is 0.43 m — but with 2/8 as the
comparison, "it never wins the lottery" is eight tickets and no win, which is
not yet evidence of a loaded ticket.

---

## Arm C — the founder pairing, under the same controls

| body | diet | n | reached horizon | established | mean life | median life | served f/F/l/c (m) | digestible (m) | **digestible / served** | billed (e) | motor (e) | net margin (e/s) | cells | wet |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| burrower | 0.10 | 8 | 0 | **8** | 1088 s | 1076 s | 0 / 0 / **4.84** / 0 | 4.36 | **0.900** | 5.39 | 0.28 | −0.00163 | 55 | 75 % |
| grazer | 0.85 | 8 | 2 | 2 | 1372 s | 335 s | **6.89** / 0 / 0 / 0 | 5.86 | 0.850 | 9.85 | 1.34 | −0.00491 | 144 | 33 % |
| glider | 0.90 | 8 | 2 | 2 | 1392 s | 364 s | **6.57** / 0 / 0 / 0 | 5.92 | 0.900 | 10.06 | 1.43 | −0.00450 | 171 | 11 % |
| **skimmer** | 0.60 | 8 | 0 | **7** | 860 s | 826 s | 0.36 / 0 / **4.96** / 0 | 2.20 | **0.413** | 4.31 | 0.57 | −0.00200 | 119 | 65 % |

Every arm-C clone's lifetime, by seed:

| seed | burrower | grazer | glider | skimmer |
| --- | --- | --- | --- | --- |
| 1001 | 942, 1023 | 351, **4500** | 323, **4500** | 926, 1059 |
| 1002 | 1023, 1132 | 310, 310 | 350, 408 | 658, 828 |
| 1003 | 1189, 1275 | 318, 375 | 350, **4500** | 782, 1002 |
| 1004 | 995, 1129 | 314, **4500** | 328, 377 | 802, 824 |

Three things here, and they are the substance of the note.

**1. The breadth cost, measured rather than inferred.** F could only derive it
from the contract's arithmetic. Here it is on the ledger, from the two detrital
bodies standing in the same eight cells of the same worlds: the skimmer served
**42.51 m** in total and digested **17.57 m** — a realised yield of **0.413**,
which is `cap_detrital = φ(1 − 0.60) = 0.40` — while the burrower served
**38.75 m** and digested **34.87 m**, a yield of **0.900** = `φ(1 − 0.10)`.
**The skimmer takes 10 % more material out of the world than the burrower and
gets half as much out of it**, and lives 826 s to the burrower's 1076 s median.
That is the whole of "a lower realised diet yield", and it is the one body-level
penalty this design resolves cleanly, with no seed disagreeing (the skimmer's
eight lifetimes span 658–1059 s, the burrower's 942–1275 s).

**2. The detrital channel is a reliable trickle that is always below upkeep.**
There is no lottery and there are no zeroes: **every one of the 32 clones in the
campaign with an open detrital channel served between 1.1 and 14.3 m** (the 16
in arm C, between 2.6 and 6.9), **not one of the 32 ever fell to the no-intake
floor, and not one of the 32 ever reached the horizon or ever had a positive net
margin.** They eat steadily, lose steadily, and starve — at 547–2,270 s over the
campaign, 658–1,275 s in arm C. Set against the pure-foliage clones' 51-of-64 at
under 0.5 m, that is two completely different ways to die.

**3. The foliage channel is the only one that can pay.** All 8 horizon survivors
are pure foliage feeders, all 8 had positive margins, and their margins are
thin: +0.00086 to +0.00118 e/s. **0 of 32 detrital clones against 8 of 64
foliage clones** (Fisher exact, two-sided, p = 0.049) — suggestive at four
seeds, not settled.

---

## The verdict

**Of "diet / body / both / neither — habitat", the design supports *both*, and
neither leg points where the association pointed.**

**The diet leg is large, sign-definite — and reversed; it needed the
counterbalanced arm below to be called controlled.** (Corrected after Astra's
round-2 review, P1.) Arm A as first run moved one locus on one body at the same
tick, but its diet assignment was fixed to slots — 0.60 in slots 0, 1, 4, 5 and
0.85 in slots 2, 3, 6, 7 on every seed — so diet was confounded with founding
cell, and the low-diet slots happened to be the richer ones (mean opening
foliage 0.17–0.19 against 0.11–0.16 on every seed). The Fisher p of
3.9 × 10⁻⁵ treats 32 clone lives sharing four worlds as independent and is
descriptive only. The **swapped arm** (`--arms As`, the same cells with the
diets exchanged) resolves this: within the same slot, the low diet outlives the
high diet in **29 of 32** pairs, establishes **22 of 32** against **2 of 32**,
and does so in all four worlds (per-seed medians 800–901 s against 375–399 s);
in the swapped arm alone, where the originally richer slots hold the high diet,
the high diet establishes 1 of 16 and the low diet 9 of 16. The effect follows
the diet, not the cell. At the independent level of worlds it is 4 of 4. The
founder's `diet = 0.60` is the better of the two for the skimmer body. F's 84 %
cannot be read as "give the skimmer a foliage diet and it lives".

**Arm C measures a diet-locus yield difference, not a body cost** (corrected
after review). Arm C changes form *and* diet together, and the realised yields
**0.413 against 0.900** follow directly from the two roster diets' capacities
(φ(1 − 0.60) against φ(1 − 0.10)); the 23 % shorter life for a body that ate more
is real for those two roster pairings and does not isolate a body-level
penalty. At a foliage diet (arm B) the body leg is **not resolvable**: 0/8
against 2/8 with four seeds is not a measurement. The body leg remains
unmeasured; the depth-only factorial named below is the test.

**Habitat is not excluded, and is what couples the two legs.** The skimmer's
`depth = 0.10, swim = 1.0` body does go to the water — 65 % of its probes on wet
ground in arm C, against the glider's 11 % — and the water in ecology v1 grows
**litter, not algae**. Of the ten deepest cells on each of the four seeds, 39 of
40 carry no foliage at all. The niche the skimmer's body was built for supplies
exactly the food its diet can only half digest, and moving its diet to the
foliage end shuts that supply without opening another. Arm B cannot separate
`form` from `depth`, and roster form also bundles size, speed, swimming and
metabolism, so body and habitat are **not separable here** (softened after
review from "one statement"); `depth` is the leading mechanism, not the shown
one.

### Reconciling with workstream F, carefully

These results do **not** contradict F, and it matters to say why rather than to
declare one of them wrong.

F measured **descendants over 150 simulated minutes with reproduction on**. Only
a lineage that found a persistent stand leaves descendants at all — and this
campaign shows that in `fast-leaf` the *only* way to a positive margin is the
foliage channel, which pays 8 times in 64 and nothing the other 56. So a
surviving skimmer lineage is most plausibly a foliage-diet lineage born into
the patch its parent won, and F's 84 % is most plausibly survivorship of the
winners of an all-or-nothing lottery. (Softened after review: this campaign
measured cold founding with reproduction off, so it makes the survivorship
explanation plausible; it does not identify the causal process inside F's
reproductive descendant census.)

This campaign measures **establishment from a cold founding with reproduction
off**. Different hazard, different question. What it establishes is that the
association is not a causal *founding-time* diet effect — at founding the
opposite diet wins — and that the confound F named, lineage habitat travelling
with lineage diet, is a sufficient explanation, not a demonstrated one. The two results together say something neither says alone: **a detrital diet
buys a long, certain, always-losing decline, and a foliage diet is a lottery
that mostly ends in six minutes and occasionally pays indefinitely.** Over 150
minutes with reproduction, only the lottery winners have descendants.

---

## Addendum after review: the counterbalanced arm

Astra's round-2 review found that arm A's diet assignment was fixed to slots,
so diet was confounded with founding cell. Fable added `Arm::ASwap` (`--arms
As`): the same eight placements, the same body, the same tick, with the two
diets exchanged between the slot pairs (0.85 in slots 0, 1, 4, 5; 0.60 in
slots 2, 3, 6, 7). Not part of the default three arms. Command and rows:

```bash
./target/release/cubarium-search factorial --arms A,As --seeds 4 --workers 8 \
  --out runs/ecology-v1-diet-factorial/swap
```

The `A` rows reproduce this note's arm A by `final_state_hash` on all four
seeds. Within-slot pairing across the two arms (32 pairs, one per slot per
seed):

| | low diet 0.60 | high diet 0.85 |
| --- | --- | --- |
| established (≥ 750 s) | **22 / 32** | **2 / 32** |
| median life | 831 s | 381 s |
| within-slot pairs won | **29** | 3 |
| per-seed medians | 901 / 800 / 828 / 835 s | 376 / 375 / 394 / 399 s |
| established per seed | 7 / 5 / 5 / 5 of 8 | 0 / 0 / 2 / 0 of 8 |

In the swapped arm alone the originally richer slots now hold the high diet and
it establishes 1 of 16 there; the low diet in the originally poorer slots
establishes 9 of 16. The advantage follows the diet, not the cell. The two
high-diet establishments are both seed 1003 slots 3 and 5, alive at the
horizon under both assignments' high diet (the foliage lottery's winners). The
independent replicate is the world: 4 of 4 agree in direction and in
establishment count. The clone-level Fisher test in the original arm A is
withdrawn as an inferential statistic.

## Accounting, and what was not measured

- **Conservation, per body.** Worst `|material residual|` over 96 clone records
  **6.41e-12**; worst `|energy residual|` **3.09e-11**. Both are `Σ credits −
  Σ debits − Δ(stores)` for a whole recorded life, and both are far inside the
  contract's 1e-9.
- **Sterility.** 0 clone births in 96 clones; 42–145 births in the world around
  them in every run, so the legacy founders were breeding throughout.
- **Nothing was dropped.** 0 closed ledger records lost to the core's
  4,096-record buffer.
- **Recording is inert.** A test in the suite runs arm A at seed 5 with the
  ledger on and off and asserts the same final state hash.
- **Death causes.** Every clone that died, died of **starvation**: 88 of 88. No
  age, no collapse, no predation — no apex was introduced (see below).
- **Billing is not per channel.** The core bills the body, not the meal; upkeep
  and the two motor terms are reported instead.
- **The run stops when the last clone is gone.** 6 of 12 runs stopped before
  90,000 ticks (8,701, 18,652, 20,811, 22,646, 30,073, 76,275). The condition reads the organisms, not the
  ledger, so a ledger-off run stops on the same tick. World-level context is
  reported at the stop tick, not at a common tick.

## What this does not establish

- **No apex was introduced.** The brief did not ask for one and predation was 5
  of 259 form-3 deaths in F's baseline. Every arm is the zero-apex arm; nothing
  here says anything about predation.
- **Four training seeds, eight clones per arm.** Arm A's effect is large enough
  that four seeds settle it; **arm B's is not**, and the note says so rather than
  reporting 0/8 against 2/8 as a finding. No held-out seed was touched.
- **One ecology.** `fast-leaf` only. The all-or-nothing foliage channel and the
  always-losing detrital trickle are statements about this configuration's
  `plant.foliage_rate`, `organism.mouth_rate`, `intake_half_saturation` and
  `maintenance`, not about ecology v1 in general.
- **No reproduction, so no lineage.** This measures one life. A body that
  establishes well and breeds badly looks identical here to one that does both
  well. That is exactly the gap between this and F, and it is the reason both
  are needed.
- **`γ` was not moved.** The brief forbids it and none was run. Every capability
  here is `φ(x) = x` above `θ = 0.2`.
- **Arm B cannot separate `form` from `depth`, `speed`, `size`, `swim` or
  `metabolism`.** It moves the whole roster body.
- **Founding is not the only cold start.** Clones are founded at tick 0 into a
  world whose own founders have not yet depleted anything. A clone introduced at
  tick 40,000 would face a different world.
- **75 simulated minutes.** A clone "reaching the horizon" means 4,500 s with a
  margin of about +0.001 e/s, which is a thin living, not a demonstrated one.

## The next task this implies, named and not launched

**Move `depth` and nothing else, in arm A's design.** The one hypothesis this
campaign raises and cannot test is that the skimmer's `depth = 0.10` is what
costs it: it steers the body to the wet floor, which carries litter but no
foliage, and that — not the rig, not the speed, not the size — is what makes
"body" and "habitat" the same statement. Arm A already proves the machinery can
move one locus cleanly on a matched clone. The same eight cells, the same
skimmer genome, `depth` at 0.10 against 0.55, at both `diet = 0.60` and
`diet = 0.85`: four cells of a 2 × 2, 8 clones each, the same four seeds, the
same ledger. It costs about the same 25 seconds this campaign cost.

Two others worth naming, neither launched:

- **The lottery's mechanism.** 8 of 64 foliage clones found 25–27 m and 56 found
  under half a gram. What distinguishes them? The 8 survivors visited **271–595**
  distinct cells (median 486); the 51 that never really ate visited **30–108**
  (median 56); the 22 that ate well and still died sit between, at 33–560
  (median 126). Cause or consequence is exactly what that does not say. A probe
  of *what the winner's cell was doing* would answer it.
- **Whether the detrital trickle can ever pay.** Every detrital clone lost, at
  every seed, at both `cap_detrital = 0.40` and `0.90`. If litter cannot fund a
  body in `fast-leaf`, the burrower and the skimmer are both slow deaths and the
  roster has one working guild, not four. That is a statement about
  `detritus.decomposition`, `energy_cap` and `mouth_rate`, and it is a
  calibration question rather than a genome one.

## Routine decisions made here, and their visible effect

- **Zero apex founders.** Predation is not the question and starvation is;
  including an apex would have added noise to 96 lifetimes to answer nothing.
- **Warm-up 24,000 ticks, averaged over the second half.** A world is created
  dry; at 2,000 ticks no cell has pooled at all. 24,000 is 20 simulated minutes
  against the run's 75.
- **`wet_min = 0.05 d`**, the depth at which a non-swimmer starts paying a
  measurable wading penalty (`speed / (1 + w · (1 − swim))`), rather than half
  `algae_depth`. It is a reporting threshold only: nothing selects on it.
- **Eight anchors, ordered so consecutive slots are on different faces.** Slots
  are assigned to treatments in pairs, so this puts each treatment on two faces
  rather than twice in one place.
- **Stop when the last clone is dead.** It cut the campaign from about 65 s to
  24 s and changes no measured number.
- **"Established" at 750 s.** About twice the measured no-intake floor of
  308–431 s. The arm-A separation is 13/16 against 1/16 and survives any
  threshold between 500 and 1,200 s.

## Compute and storage actually used

| | |
| --- | --- |
| Campaign | **22.3 s** wall on 8 workers, 717,158 simulated ticks, 12 runs |
| Landscape census (4 seeds × 24,000 ticks) | ~4 s |
| Reproduction checks (3 of them) | ~70 s |
| Budget | ≤ 10 wall minutes of simulation, ≤ 8 workers — **used about 100 s and 8** |
| `runs/ecology-v1-diet-factorial/` | **128 KiB** of the 20 MiB allowed |
| Left running | nothing |
