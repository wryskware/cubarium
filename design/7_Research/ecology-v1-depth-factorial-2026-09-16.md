---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# The depth × diet factorial: is the skimmer's loss the pool it stands in?

Workstream O, from J's named next task
([the diet factorial](ecology-v1-diet-factorial-2026-09-16.md), "The next task
this implies"): *move `depth` and nothing else, in arm A's design.* J could
separate `diet` from body, and did; it could not separate `depth` from the rest
of the roster body, and said so. This campaign moves the one locus J named, on
the same eight cells, the same four worlds, the same ledger.

> **The answer is `depth`, and it is not close.** Moving the roster skimmer's
> `depth` from 0.10 to 0.55 — nothing else, not even `swim` — turns a body that
> starved in **32 of 32** clone lives into one that reaches the 75-minute
> horizon in **26 of 32** at the founder's diet and **27 of 32** at the foliage
> diet, in all four worlds. Its net margin
> changes sign, −0.00192 → +0.00035 e/s at `diet = 0.60`, winning **30 of 32**
> within-cell pairs. And the diet effect reverses with it: at `depth = 0.10`
> the founder's detrital-leaning 0.60 is the better diet (25 of 32 pairs), at
> `depth = 0.55` the foliage-end 0.85 is (26 of 32 pairs, 4 of 4 worlds). The
> skimmer's `depth` is not one of several contributing loci. It is the coupling
> that made "body" and "habitat" the same statement in J's verdict.

---

## The design

### The factor, and why 0.55

`depth` is **not** a water depth. It decodes to a preferred *embedded height*,
`h_pref = −1 + 2 · depth` (`crates/cubarium-core/src/genome.rs:434`, Top = +1,
rim = −1), and the only place it enters the simulation is one term of the
steering vector, `w_depth · (h_pref − h) · up`
(`crates/cubarium-core/src/controller.rs:234`). It changes where a body goes
and nothing else — no capacity, no rate, no bill. That is asserted, not
assumed: `depth_changes_only_the_preferred_height_and_no_capacity` decodes the
two genomes and compares every other field of the phenotype.

The low level is the roster skimmer's own `depth = 0.10`
(`crates/cubarium-core/src/config.rs:519`), `h_pref = −0.8`: the low rim. The
core's own comment calls it "the wet floor", and the world agrees —
`habitat.moisture_height_gain` is negative and standing water sits on
`z = h + basin_gain · n` (`crates/cubarium-core/src/habitat.rs:30`), so low
ground is where the pools are.

The high level is **0.55, the roster grazer's own** (`config.rs:513`),
`h_pref = +0.1`. Chosen over any invented number for three checkable reasons:

1. it is inside the genome's declared bounds (`depth` is clamped to 0–1,
   `genome.rs:267`), so the treatment is a legal genotype and not an
   extrapolation;
2. it is a value the roster already carries, so the world is known to be able
   to hold a body with it;
3. it is the smallest roster step that actually means *"does not seek the wet
   floor"*. `h_pref = +0.1` sits just **above** the equator, so the depth term
   pushes the body off the rim rather than merely pulling it there less hard.
   The glider's 1.00 would have meant "lives on the Top face" — a second change
   of habitat rather than the removal of one — and the burrower's is 0.10, the
   same value under test.

Four treatments, every other locus identical to the roster skimmer including
`swim = 1.0`, `speed`, `size`, `metabolism` and every drive (so `w_depth`, the
*gain* on the depth term, is held while `h_pref`, its *target*, moves):

| | `depth` | `h_pref` | `diet` | |
| --- | --- | --- | --- | --- |
| **T0** | 0.10 | −0.80 | 0.60 | the roster skimmer, exactly |
| **T1** | 0.10 | −0.80 | 0.85 | J's arm-A high half |
| **T2** | 0.55 | +0.10 | 0.60 | the founder's diet, out of the pool |
| **T3** | 0.55 | +0.10 | 0.85 | both loci moved |

### The assignment: a 4 × 8 Latin square, not two arms

Astra's round-2 review found that J's arm A confounded diet with founding cell,
and J's counterbalanced arm `As` fixed that by swapping the two diets between
the same eight slots. Four treatments cannot be counterbalanced the same way:
eight slots hold four treatments twice each, so **two** arms can give every cell
only two of the four. The assignment therefore runs over **four rows**, which is
exactly the `4 × 8` table the brief describes — row `r` gives slot `s` the
treatment `(s mod 4) XOR r`:

| slot | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| *anchor face* | Front | Right | Back | Left | **Top** | Front | Right | Back |
| **D1** | T0 | T1 | T2 | T3 | T0 | T1 | T2 | T3 |
| **D2** | T1 | T0 | T3 | T2 | T1 | T0 | T3 | T2 |
| **D3** | T2 | T3 | T0 | T1 | T2 | T3 | T0 | T1 |
| **D4** | T3 | T2 | T1 | T0 | T3 | T2 | T1 | T0 |

The treatment index is a two-bit code — **bit 0 is the foliage-end diet, bit 1
is the mid-height depth** — and the row is a two-bit mask, so XOR makes this a
Klein four-group action. Three properties follow, and all three are tested
rather than asserted:

- **Every column is a permutation of the four**: each of the eight cells holds
  each of the four treatments exactly once across the rows. Neither factor is
  confounded with founding cell, and every contrast in this note is *within
  cell* — the same place, the same world, the same tick.
- **Every row holds each treatment exactly twice**, at slots four apart and so
  on two different faces. Every run is a balanced mix of all four, so each
  contrast is also inside one world realisation and one competitive
  environment.
- **Each pair of rows is a one-factor exchange in every slot**: D1/D2 and D3/D4
  exchange `diet` with `depth` held, D1/D3 and D2/D4 exchange `depth` with
  `diet` held, D1/D4 exchange both. The counterbalance is exact for each factor
  separately, which is what the two-arm version could not deliver. (A two-arm
  complement pairs each cell with the *diagonal* treatment only, so the depth
  contrast would have been between cells — reintroducing the very confound
  Astra's P1 flagged.)

Two arms would have cost 8 runs and 15 s; four cost 16 runs and 35 s, well
inside the brief's three minutes. The compute was not the binding constraint,
so the stronger design was taken. This is the one place where this note departs
from the brief's literal wording ("two counterbalanced arms") in order to reach
the balance the same sentence asks for ("every one of the eight cells holds
every treatment once").

### What is controlled, and what is not

Inherited unchanged from J, because the harness is J's: birth tick (every clone
founded at tick 0), place (the same eight cells, resolved from the same fixed
anchors on the seed's own warm-up landscape — asserted equal to arm A's in
`the_depth_rows_stand_where_arm_a_stood`), lineage (reproduction switched off
for the clones through the ES scripted seam, `bud: Some(false)`), mutation (it
only reaches offspring, and there are none), and ecology (one `fast-leaf` world
per seed with its ordinary 24 founders still breeding in it).

Added by this campaign: **treatment is orthogonal to cell** (the Latin square)
and **every treatment is present in every run** (two of each).

Not controlled, and named: the four runs of one seed are four *different world
trajectories*, because the eight clones differ between rows and the world
diverges from them. The Latin square balances that — each treatment appears
twice in every row — but a row is not a replicate of another row. The
independent replicate is the **world**, of which there are four.

---

## Build, commits and commands

| | |
| --- | --- |
| branch | `worktree-agent-ab0120f26388bf78a`, from `1525604` |
| harness commit | `94d148b` — the Latin square, the treatment constants, `print_depth_analysis`, the tests |
| build id | `94d148b` (pinned through `CUBARIUM_SEARCH_BUILD`), recorded in every row |
| ecology | `fast-leaf`, unchanged; no core change, no `γ` change |
| rows | `runs/ecology-v1-depth-factorial/runs.jsonl` (16 rows), `summary.json`, `report.txt` |

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium-search
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target CUBARIUM_SEARCH_BUILD=94d148b \
  cargo build --release -p cubarium-search
./target/release/cubarium-search factorial --arms D1,D2,D3,D4 --seeds 4 --workers 8 \
  --out runs/ecology-v1-depth-factorial
```

`--arms D1,D2,D3,D4` is the whole design; a single row is not one. The three
original arms and `As` are untouched — `plan` moves `depth` only for the depth
rows — apart from one extra column in `print_report`'s table.

### Tests

`cargo test -p cubarium-search`: **188 passed, 1 ignored, 0 failed** (29 in
`tests/diet_factorial.rs`, up from 17; 93 unit tests, up from 91). Written
before the implementation existed and run red first. The ones this result rests
on:

- `every_cell_holds_every_treatment_exactly_once_across_the_four_rows` — the
  Latin square, as `plan`'s output: the same cell, the same class, the same
  body in every row, and the four declared treatments once each.
- `every_row_holds_every_treatment_twice_on_two_faces`.
- `the_rows_are_slot_by_slot_complements_one_factor_at_a_time` — all six row
  pairs, every slot.
- `a_depth_arm_is_the_roster_skimmer_with_only_depth_and_diet_changed` — put
  the two loci back and the genome is the roster skimmer, locus for locus;
  `w_depth` and `swim` explicitly held.
- `the_two_depth_values_are_roster_values_that_straddle_the_equator` — 0.10 is
  the skimmer's, 0.55 the grazer's, both in bounds, `h_pref` −0.8 and +0.1, and
  `habitat.moisture_height_gain < 0` so "the wet floor" names a real place.
- `depth_changes_only_the_preferred_height_and_no_capacity`.
- `the_analysis_labels_a_clone_with_the_treatment_plan_gave_it` — the slot
  arithmetic the tables use is the one that built the genomes.
- `a_short_depth_run_is_sterile_closed_and_reproducible`,
  `the_depth_rows_stand_where_arm_a_stood`.
- Unit: `the_slot_arithmetic_is_a_latin_square_of_one_factor_swaps`,
  `the_median_is_the_ordinary_one`.

### The checks made before anything was interpreted

- **Sterility.** 0 clone births in 128 clone lives; 16 of 16 runs had the
  ordinary founders breeding around them.
- **Conservation, per body.** Worst `|material residual|` **1.62e-12**, worst
  `|energy residual|` **3.50e-11** over 128 records — both far inside the
  contract's 1e-9.
- **Nothing dropped.** 0 closed ledger records lost to the core's 4,096-record
  buffer.
- **Complete cells.** 32 of 32 cells carry all four rows.
- **Placement.** Identical to J's, by construction and by test; 5 of the 32
  cells started on standing water (1 / 0 / 3 / 1 by seed).

---

## The reproduction check against J

The `T0` vs `T1` contrast *is* J's counterbalanced diet pair — the same body at
`depth = 0.10`, the same eight cells, the same four seeds — so it must agree, or
the harness changed something it should not have.

| | J (arm A + `As`, 32 within-slot pairs) | O (T0 vs T1, 32 within-cell pairs) |
| --- | --- | --- |
| pairs won by the low diet | **29 / 32** | **25 / 32** |
| established (≥ 750 s) | **22 / 32** vs **2 / 32** | **27 / 32** vs **7 / 32** |
| median life | 831 s vs 381 s | 863 s vs 386 s |
| per-seed median, low diet | 901 / 800 / 828 / 835 s | 844 / 862 / 879 / 861 s |
| per-seed median, high diet | 376 / 375 / 394 / 399 s | 383 / 377 / **2493** / 387 s |
| established per seed, low | 7 / 5 / 5 / 5 of 8 | 6 / 7 / 7 / 7 of 8 |
| established per seed, high | 0 / 0 / **2** / 0 of 8 | **3** / 0 / **4** / 0 of 8 |
| worlds agreeing on establishment | 4 of 4 | 4 of 4 |

**It reproduces qualitatively, and the discrepancy is where J said to expect
it.** Direction, establishment ratio and per-seed agreement all carry over. The
high diet does somewhat better here, and all of the extra establishments sit on
seeds 1001 and 1003 — the seeds whose foliage-lottery winners J already
identified. Two reasons the rows are not identical and cannot be: the world is
not J's world (each run here holds two clones of each of four treatments, and
the two `depth = 0.55` clones survive the whole horizon eating ~20 m of foliage
apiece, so the competition every clone faces is different), and the wet-start
class of a given slot is unchanged but its neighbours are not. State hashes are
therefore expected to differ from J's and do; what is claimed is the direction
and the per-world agreement, and both hold.

---

## The treatments

Pooled over 32 cells (4 worlds × 8 cells); medians are over the same 32.

| | n | reached horizon | **established** | mean life | **median life** | served f/F/l/c (m) | digestible (m) | credited (m) | billed (e) | **net margin (e/s)** | distinct cells | wet probes | algae-band probes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **T0** 0.10 / 0.60 | 32 | **0** | 27 | 931 s | 863 s | 0.81 / 0 / **4.66** / 0 | 2.35 | 1.15 | 4.67 | **−0.00192** | 124 | 67 % | **34 %** |
| **T1** 0.10 / 0.85 | 32 | 6 | 7 | 1263 s | **386 s** | 4.29 / 0 / **0** / 0 | 3.64 | 2.19 | 6.72 | **−0.00390** | 104 | 36 % | 9 % |
| **T2** 0.55 / 0.60 | 32 | **26** | 30 | 4074 s | **4500 s** | **22.52** / 0 / 1.95 / 0 | 14.29 | 8.52 | 20.51 | **+0.00035** | 399 | 65 % | 7 % |
| **T3** 0.55 / 0.85 | 32 | **27** | 27 | 3860 s | **4500 s** | **16.43** / 0 / 0 / 0 | 13.96 | 8.38 | 19.40 | −0.00007 | 364 | 62 % | 5 % |

Median net margin, which is not censored by the horizon and is the number the
verdict rests on: **−0.00200 / −0.00530 / +0.00060 / +0.00080 e/s**.

**Death causes.** T0: **32 of 32 starved, none reached the horizon.** T1: 26
starved, 6 survived. T2: 6 starved, 26 survived. T3: 5 starved, 27 survived. No
age, no collapse, no predation; no apex was introduced.

### Per seed — the world is the replicate

| seed | T0 0.10/0.60 | T1 0.10/0.85 | T2 0.55/0.60 | T3 0.55/0.85 |
| --- | --- | --- | --- | --- |
| 1001 median life | 844 s | 383 s | 4323 s | 4500 s |
| 1002 median life | 862 s | 377 s | 4500 s | 4500 s |
| 1003 median life | 879 s | 2493 s | 4500 s | 4500 s |
| 1004 median life | 861 s | 387 s | 4500 s | 4500 s |
| 1001 established of 8 | 6 | 3 | 6 | 7 |
| 1002 established of 8 | 7 | 0 | 8 | 7 |
| 1003 established of 8 | 7 | 4 | 8 | 8 |
| 1004 established of 8 | 7 | 0 | 8 | 5 |
| 1001 median net margin | −0.00201 | −0.00539 | **+0.00037** | **+0.00086** |
| 1002 median net margin | −0.00196 | −0.00548 | **+0.00075** | **+0.00082** |
| 1003 median net margin | −0.00190 | −0.00163 | **+0.00057** | **+0.00089** |
| 1004 median net margin | −0.00194 | −0.00532 | **+0.00056** | **+0.00083** |

Every world, at both diets, puts `depth = 0.55` on the positive side of zero and
`depth = 0.10` on the negative side. There is no overlap anywhere in the table.
"Established" is J's threshold, 750 s, about twice its measured no-intake floor
of 308–431 s.

---

## The within-cell contrasts

Each pair is one cell of one world carrying both treatments, so the cell, the
landscape, the founding tick and the ordinary founders around it are shared.

**On lifetime** (censored at the 4,500 s horizon, which is why ties appear):

| contrast | pairs won by the first | ties | established | median life | worlds agreeing (median) | worlds agreeing (establishment) |
| --- | --- | --- | --- | --- | --- | --- |
| diet 0.60 vs 0.85 **at depth 0.10** | **25 / 32** | 0 | 27 vs 7 | 863 s vs 386 s | 3 of 4 | **4 of 4** |
| diet 0.60 vs 0.85 **at depth 0.55** | 5 / 32 | **22** | 30 vs 27 | 4500 s vs 4500 s | 0 of 4 | 2 of 4 |
| **depth 0.10 vs 0.55** at diet 0.60 | 2 / 32 | 0 | 27 vs 30 | 863 s vs **4500 s** | 0 of 4 | 0 of 4 |
| **depth 0.10 vs 0.55** at diet 0.85 | 2 / 32 | 6 | 7 vs 27 | 386 s vs **4500 s** | 0 of 4 | 0 of 4 |
| the roster skimmer vs both loci moved | 5 / 32 | 0 | 27 vs 27 | 863 s vs **4500 s** | 0 of 4 | 1 of 4 |

Read the depth rows the other way round: `depth = 0.55` wins **30 of 32** pairs
at `diet = 0.60` and **24 of 32** at `diet = 0.85` with 6 ties — and all six ties
are cells where *both* high-diet clones were **alive at the horizon**, four at
seed 1003 and two at seed 1001. Those six cells are precisely T1's six horizon
survivors: the foliage lottery's winners, where a `depth = 0.10` body happened
to find a stand and the lifetime measure then saturates. They are censored, not
counterexamples — but they are also the honest limit of the claim, and they say
that a `depth = 0.10` body *can* win if it finds foliage.

**On net margin per second**, which the horizon does not censor:

| contrast | pairs won by the first | median | per-seed pairs won | worlds where it wins the majority |
| --- | --- | --- | --- | --- |
| diet 0.60 > 0.85 at depth 0.10 | **25 / 32** | −0.00195 vs −0.00534 | 5, 8, 4, 8 of 8 | 3 of 4 |
| diet **0.85 > 0.60** at depth 0.55 | **26 / 32** | +0.00085 vs +0.00058 | 7, 6, 8, 5 of 8 | **4 of 4** |
| depth 0.55 > 0.10 at diet 0.60 | **30 / 32** | +0.00058 vs −0.00195 | 6, 8, 8, 8 of 8 | **4 of 4** |
| depth 0.55 > 0.10 at diet 0.85 | **29 / 32** | +0.00085 vs −0.00534 | 7, 7, 7, 8 of 8 | **4 of 4** |

---

## The interaction: the diet effect reverses with depth

| the diet step 0.60 → 0.85, within cell | median | cells where it is negative |
| --- | --- | --- |
| at `depth = 0.10` (the wet floor), lifetime | **−468 s** | 25 of 32 |
| at `depth = 0.55` (mid height), lifetime | +0 s | 5 of 32 (22 ties at the horizon) |
| at `depth = 0.10`, net margin (median of the within-cell steps) | **−0.00333 e/s** | 25 of 32 |
| at `depth = 0.55`, net margin (median of the within-cell steps) | **+0.00026 e/s** | 6 of 32 |
| the interaction (difference of the two steps, within cell) | **−0.00326 e/s** | — |

| establishment, of 32 cells | diet 0.60 | diet 0.85 |
| --- | --- | --- |
| depth 0.10 | **27** | **7** |
| depth 0.55 | 30 | 27 |

**Yes, and it is a sign reversal, not a magnitude change.** On lifetime the
interaction is partly a ceiling — 22 of the 32 cells have *both* `depth = 0.55`
clones alive at the horizon, so lifetime cannot separate the diets there. Net
margin per second has no ceiling, and on it the diet effect flips: the founder's
0.60 wins 25 of 32 pairs at `depth = 0.10` and **loses 26 of 32 at
`depth = 0.55`**, with 4 of 4 worlds agreeing on the reversed side. Establishment
tells the same story more coarsely: the diet gap is 27 vs 7 at the wet floor and
30 vs 27 at mid height.

That reversal is worth stating plainly, because it reconciles this campaign with
workstream F. **F's association — foliage-diet skimmers do better — is real, and
it is real only for a body that is not parked in a pool.** J found the opposite
sign because every clone it compared had `depth = 0.10`. Both are correct
statements about different bodies.

---

## The mechanism, on the ledger and on the probes

The probes say where the bodies went, and the wet-probe fraction alone says
almost nothing:

| | mean water depth under the body (d) | median | wet probes | algae-band probes | median distinct cells | median served foliage (m) | median served litter (m) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **T0** 0.10 / 0.60 | **0.306** | 0.171 | 67 % | **34 %** | **122** | 0.24 | **4.95** |
| **T1** 0.10 / 0.85 | 0.061 | 0.027 | 36 % | 9 % | 79 | 0.05 | 0 |
| **T2** 0.55 / 0.60 | 0.051 | 0.045 | 65 % | 7 % | **426** | **25.25** | 1.91 |
| **T3** 0.55 / 0.85 | 0.037 | 0.026 | 62 % | 5 % | 406 | **19.50** | 0 |

T0 and T2 stand on wet ground almost equally often — 67 % against 65 % — and are
nevertheless in completely different places. The discriminating measures are the
**depth of the water** (0.306 d against 0.051 d, six-fold) and the **algae-band
fraction** (34 % against 7 %). `wet_min = 0.05 d` is a reporting threshold that
a body at `depth = 0.55` crosses constantly while wandering; a body at
`depth = 0.10` sits *in the deep part of the pool*, and that is where ecology v1
grows no foliage at all (J: 39 of the 40 deepest cells over these four seeds
carry zero mean foliage, because subphase 3a only runs for a cell that already
carries wood, `crates/cubarium-core/src/fields.rs:398-409`).

The consequence is visible in one number: **median distinct cells, 122 against
426.** The `depth = 0.10` body is held in a basin and covers a third of the
ground; the `depth = 0.55` body roams and finds the eight-in-sixty-four cells
that pay. Its intake follows — 25.25 m of foliage against 0.24 m — and so does
its bill, 20.51 e against 4.67 e, which is the honest cost of moving. The margin
per second is what matters and it changes sign anyway.

`depth = 0.10` therefore buys exactly what J's verdict guessed it would: a diet
of **litter** (4.66 m served, 4.95 m median) in a place with no foliage, which
at `cap_detrital = φ(0.40) = 0.40` credits 1.15 m against a 4.67 e bill and
funds nothing. 32 of 32 starved.

### The Top-face check, which did not work as a null

`up_direction` is zero on the level Top face
(`crates/cubarium-core/src/world/lifecycle.rs:422-427`), so a clone standing
there feels no depth term at all. Slot 4's anchor is on Top in every seed, so
the design carried a free control: if the effect is the steering term, it should
be weaker there.

| slots | cells | wet probes at 0.10 | at 0.55 | median life at 0.10 | at 0.55 |
| --- | --- | --- | --- | --- | --- |
| slot 4 (Top) | 4 | 47 % | 66 % | 654 s | 4500 s |
| slots 0–3, 5–7 | 28 | 52 % | 63 % | 801 s | 4500 s |

**It is not a null, and the reason is instructive rather than disappointing:**
the clones do not stay on Top. The depth term starts acting the moment a body
steps onto a side face, and the effect at slot 4 is if anything *larger* (654 s
against 801 s at `depth = 0.10`). With four cells this is a description, not a
test. It is reported because the design produced it, not because it decides
anything.

---

## The verdict

Of the three readings the brief named — *"depth couples the body to litter"* /
*"depth is not the coupling"* / *"not resolvable at four seeds"* — the data
support the **first**, and support it more strongly than anything J measured.

1. **Does `depth` alone move the skimmer's establishment?** Yes, decisively, at
   both diets. At `diet = 0.60`: 0 of 32 reach the horizon at `depth = 0.10`,
   26 of 32 at 0.55; 30 of 32 within-cell pairs won on net margin; 4 of 4
   worlds. At `diet = 0.85`: 6 of 32 against 27 of 32; 29 of 32 pairs; 4 of 4
   worlds. The median net margin changes sign in every world.
2. **Does `depth` alone move the wet-probe fraction?** *Barely*, and that is a
   finding about the measure. 67 % → 65 % at `diet = 0.60`. What it moves is the
   **depth of the water the body stands in** (0.306 d → 0.051 d) and the
   **algae-band fraction** (34 % → 7 %). A future note should report mean water
   depth, not the binary wet fraction, as the habitat measure.
3. **Does the diet effect depend on depth?** Yes, and it *reverses*. The
   founder's 0.60 wins 25 of 32 pairs at `depth = 0.10` and loses 26 of 32 at
   `depth = 0.55` (4 of 4 worlds). At the wet floor the detrital half of the
   mouth is the only income there is, and it is a slow loss; away from it,
   foliage is the only income that pays, and the gate at `θ = 0.2` costs nothing
   to shut.
4. **The cell classes stood in.** 5 of the 32 cells started wet; the placement
   never selected on habitat. Every clone lived on foliage and litter: over all
   128 lives the largest fruit intake was 0.025 m and the largest carrion intake
   1.6 × 10⁻⁵ m, against 26.96 m of foliage and 7.58 m of litter, so the channel
   story is two-channel throughout.

**What this says about the roster.** The skimmer was given `depth = 0.10` and
`swim = 1.0` so it could exploit "the algae that standing water grows on the
floor" (`config.rs:516-518`). That larder does not exist in ecology v1:
`water.algae_light` raises a *living* cell's light floor, it does not create a
producer where there is no wood. So the roster's fourth guild is a body
steered into the one part of the world that cannot feed it, and this campaign
is the controlled demonstration. J's "body and habitat are not separable here"
now has a mechanism: **habitat *is* the body, through one locus, and that locus
is `depth`.**

---

## What this does not establish

- **One ecology, `fast-leaf`, and no `γ` change.** Every capability here is
  `φ(x) = x` above `θ = 0.2`. The all-or-nothing foliage channel and the
  always-losing detrital trickle are statements about this configuration.
- **No reproduction, so no lineage.** This measures one life from a cold
  founding. A body that establishes and breeds badly is indistinguishable here
  from one that does both well. That gap is exactly why F is still needed.
- **No apex.** Zero predation in 128 lives; nothing here speaks to it.
- **Four training seeds.** Large enough for effects this size; no held-out seed
  was touched.
- **`depth = 0.55` is not shown to be optimal.** Two levels measure a direction,
  not a curve. 0.55 may be past the best value or short of it.
- **The horizon censors the lifetime measure.** 53 of the 64 `depth = 0.55`
  clones were alive at 4,500 s, so "median 4500 s" is a floor. The depth effect
  on lifetime is *at least* what is reported. Net margin per second is not
  censored and is what the verdict uses.
- **A row is not a replicate of another row.** The four rows of one seed are
  four different world trajectories; the Latin square balances cell and
  treatment, not world-to-world divergence. Four worlds is the replication.
- **The Top-face control is not a null** (above), and n = 4.
- **`swim` was held at 1.0.** Whether a `depth = 0.10` body that could not swim
  would do better or worse is untested; the wading penalty
  `speed / (1 + w · (1 − swim))` never applied to any clone here.
- **Founding is not the only cold start.** All clones are founded at tick 0 into
  a world whose own founders have not yet depleted anything.

---

## The next task this implies, named and not launched

**Carry the depth result into a lineage census.** This campaign shows that at
founding, `depth = 0.55` rescues the skimmer body in all four worlds. F measured
*descendants over 150 simulated minutes with reproduction on* and found the
opposite-looking association. The one experiment that would join them is F's own
census re-run with the roster skimmer's `depth` at 0.55 and everything else
untouched: does the fourth guild persist over 150 minutes, and does the diet
drift upward now that the body can reach foliage? It costs roughly what F cost.
It is the test of whether a founding-time effect this large survives a lineage,
and it is the only way to decide whether the roster's `depth = 0.10` is a bug in
the roster or a bug in the world.

Two others worth naming, neither launched:

- **The wet floor has no producer, and that is a core question, not a genome
  one.** `water.algae_light` raises a living cell's light floor; it does not
  grow anything in a cell with no wood (`fields.rs:398-409`). Either the
  skimmer's niche is given a food — which is an ecology change and Fable's to
  decide — or the roster's fourth kind is a body with nowhere to live and should
  be re-specified. This campaign makes the choice unavoidable; it does not make
  it.
- **The detrital-funding calibration question is now worth asking, and is close
  to answered in the negative.** J named it. Here, across 128 clone lives, the
  litter channel funded nothing: T0 served 4.66 m of litter, was credited 1.15 m
  of it against a 4.67 e bill, and starved 32 times out of 32; the only positive
  margins in the whole campaign came from foliage, and T2's litter intake
  (1.95 m) is incidental to a life that ran on 22.52 m of foliage. The remaining
  question is a calibration one — is there *any* setting of
  `detritus.decomposition`, `energy_cap` and `mouth_rate` at which litter pays
  for a body? — and it belongs with workstream M's plant-budget measurement
  rather than with another genome factorial. Until it is answered, ecology v1
  has **one** working guild, not four, and two of the four roster kinds
  (burrower and skimmer) are slow deaths by construction.

---

## Routine decisions made here, and their visible effect

- **Four rows rather than two arms.** Stated above: two arms cannot give every
  cell every treatment, and the two-arm complement would have put the depth
  contrast between cells. Visible effect: 16 runs and 35 s instead of 8 and
  15 s, and every contrast in this note is within cell.
- **`depth = 0.55`, the grazer's own.** Justified above. Visible effect: the
  high level is a legal, roster-attested genotype whose `h_pref` is just above
  the equator, so the contrast is "seeks the rim" against "does not", not
  "seeks the rim" against "seeks the ceiling".
- **`CloneRow` gained `depth` and `h_pref`**, both `serde(default)`, so J's
  retained rows still read back. Visible effect: one extra column in every arm's
  table, including A, B and C.
- **The analysis is in the harness, not in a script.** `print_depth_analysis`
  reads the rows the run just wrote to disk, so no table in this note can
  disagree with `runs.jsonl`.
- **No clone-level p-value.** 32 lives sharing four worlds are not 32
  independent observations; every claim here is "k of 32 within-cell pairs" plus
  "k of 4 worlds", and J's withdrawn Fisher test is not reinstated.
- **Establishment kept at J's 750 s** rather than re-tuned, so the two
  campaigns' establishment columns mean the same thing.
- **Everything else inherited from J unchanged**: 90,000-tick horizon,
  24,000-tick warm-up averaged over its second half, probe every 20 ticks, drain
  every 250, `wet_min = 0.05 d`, the eight anchors and their order, stop when the
  last clone is gone (no run stopped early here — every row had a survivor).

## Compute and storage actually used

| | |
| --- | --- |
| campaign | 16 runs (4 rows × 4 seeds), **34.6 s** on 8 workers |
| simulated | 1,440,000 ticks — every run reached the full horizon |
| test suite | 188 passed, 1 ignored, 0 failed, ~12 s |
| wall time, whole workstream | about 40 minutes |
| `runs/ecology-v1-depth-factorial/` | **180 KiB** (limit 10 MiB) |
