---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream XY2 — the two cheap follow-ups X and Y named

Items 1 and 2 of the next recommendation in
[the round-5 result](ecology-v1-round5-results-2026-09-16.md), as
[the brief](../handoffs/ecology-v1-round5-followups-opus-2026-09-16.md) bounds them.
Each is the one measurement its parent workstream said would decide its open question.

**Evidence, not a decision. No training default is proposed by Part 1 and no roster change is
proposed by Part 2.** No equation, no §11 shipped default, no ordering, no `WorldConfig` field
and no `cubarium-core` file is changed by this workstream.

---

# Pre-registration

*Everything in this section was written and committed before a single row of either part
existed. The commit that carries it is named under "Build and commits"; nothing below the
horizontal rule that closes this section existed when it was written.*

## Part 1 — X's second training seed, with its own control

### The question

[X](ecology-v1-turn-deadband-2026-09-16.md) trained one `cub-act-2` arm at
`--train-seed 20260915` against the retained `cub-act-1` run at the same seed and found the
aggregate columns up (mean population score 15/16 generations, p = 0.00052; mean producer
intake per lived tick 15/16, p = 0.00052; mean opening residence 14/16, p = 0.00418) with
held-out **opening residence flat** (0.0324 → 0.0327) and the held-out minimum up
(6,914 → 7,870). X's own §"What this does not establish" says it plainly: *"The training arm
is one pair of runs at one seed … A second seed is what would separate the two."* This part is
that second seed.

### The arms

The retained command verbatim, with `--train-seed 20260916` in place of `20260915`, run once
under each adapter:

| arm | adapter | `--out` |
| --- | --- | --- |
| control | `cub-act-1` | `runs/es-eco-v1-fastleaf-s2` |
| treatment | `cub-act-2` | `runs/es-eco-v1-fastleaf-act2-s2` |

`Aggregate::Min`, σ, 16 pairs, 16 generations, horizon 36,000, the same four training layouts
and the same `fast-leaf` ecology, `--center-eval true`, 8 workers, a **20-wall-minute cap each**,
**no retry, no continuation, no tuning**. The seed-20260915 arms are **not** re-run: X's two
retained runs are the first seed and are read from disk.

The selected centre of each arm is exported and evaluated on the eight held-out layouts by
**X's selection rule, unchanged**: the highest recorded centre score, the earliest generation on
ties, training results only, frozen before any held-out episode is run.

### The reading rule

A column's paired differences are `cub-act-2` minus `cub-act-1`, paired **by generation**
(sixteen pairs), and its test is the **exact two-sided sign test**; zero differences are ties,
are dropped, and `n` is the number that remain. A column **favours `cub-act-2`** when
`p < 0.05` **and** strictly more than half of the non-zero differences are positive.

The two aggregate columns are **mean population score** (the mean of a generation's 32 candidate
scores) and **mean producer intake per lived tick** (the mean over a generation's 128 candidate
episodes of `intake_producer / ticks`) — X's own two columns, computed the same way.

> - The adapter effect is **replicated** if both aggregate columns favour `cub-act-2`
>   **and** the held-out minimum over the eight layouts is strictly higher under `cub-act-2`.
> - It is **not replicated** if either aggregate column does not favour `cub-act-2`.
> - It is **mixed** otherwise — that is, both columns favour `cub-act-2` and the held-out
>   minimum is not higher.
>
> The three branches are exhaustive and disjoint by construction.

**Held-out opening residence is reported and is not part of the rule**, because X found it flat
under the first seed (0.0324 → 0.0327) and a measure already known to be flat cannot decide a
replication. Mean opening residence *during training* is likewise reported beside the two
aggregate columns and is not read by the rule: X's rule for this part names two columns, and
this part does not widen it after seeing X's numbers.

**What follows from each branch:** nothing but a sentence. This part proposes **no training
default** under any branch. It reports what the two seeds together support.

## Part 2 — Y's ladder at arm 2

### The question

[Y](ecology-v1-depth-ladder-2026-09-16.md) ran the six-rung skimmer `depth` ladder at **arm 0**
and found no rung acceptable. Re-reading [R's](ecology-v1-depth-census-2026-09-16.md) retained
rows by arm, Y found that R's `fast-leaf` result lives in the apex arms: the grazer cost is
0.47× and 0.46× at arms 1 and 2 against 0.90× at arm 0, and the lineage is 5/6 and 5/6 against
3/6. Y's own §"The next task this implies" names this measurement: *"The same ladder at arm 2 …
It is named here and **not** launched."*

### The design

Y's design, **one axis changed**: the arm.

| axis | levels |
| --- | --- |
| roster skimmer `depth` | **0.10** (control), 0.20, 0.30, 0.40, **0.55**, 0.75 |
| configuration | `baseline` (shipped §11 defaults), `fast-leaf` (A's selected configuration) |
| seed | `TRAINING_SEEDS[..6]` = 1001–1006 |
| apex arm | **2 only** — two apex adults introduced at R's tick 6,000 |

6 × 2 × 6 × 1 = **72 trials**, horizon 180,000 ticks, sampling every 600 ticks, foraging probe
every 20 ticks, `organism.move_cost` at its shipped 0.00036, the `lanternjaw_trial` profile
unsearched, E's per-body ledger on, M's plant record off — every one of these R's and Y's,
unchanged. The treatment is `census::apply_depth_override`, unchanged: `genome.depth` written on
every founder whose genome equals the roster skimmer's, found by genome equality, re-decoded
between `World::new` and the first `World::step`.

### The predicate, and what it forces

**Arm 2 has predators, so the pursuit predicate is reachable — which it was not at arm 0.**
Since R ran, the shipped rule changed: [V](ecology-v1-predicate-adoption-2026-09-16.md) adopted
`PursuitStop::ReachEnvelope` (schema 17); R's rows ran `PursuitStop::ForwardHalfSpace`. So this
part is **two runs, not one**:

1. **The reproduction run.** Depths {0.10, 0.55} × 2 configurations × 6 seeds × arm 2 = **24
   rows**, under `--pursuit-stop half-space`, which is R's rule. These 24 rows are the
   reproduction targets and are compared to R's arm-2 rows **field for field**,
   `final_state_hash` included, with `build_id` and `elapsed_ms` the only exclusions — Y's
   `ROW_REPRODUCTION_EXCLUDED`, unchanged, named here before the check runs. Their 12 control
   rows are additionally checked by `final_state_hash` against A's screen, I's ladder at the
   shipped price and M's present arm, at arm 2.
2. **The ladder.** All six rungs × 2 configurations × 6 seeds × arm 2 = **72 rows**, under the
   **shipped** rule (`reach-envelope`), which is the world the cube actually runs. Its rows have
   no retained reproduction target by construction — nothing retained was ever run at arm 2
   under the shipped predicate — so it is run with the retained checks switched off rather than
   against targets it cannot meet.

The `--pursuit-stop` flag is added to `census` if it lacks one, **defaulting to the shipped
rule** as every other command in this binary does. Nothing else about the harness changes.

The 24 cells the two runs share are also a **direct measurement of what V's adoption does at
arm 2**: same seeds, same configurations, same depths, same build, one rule apart. That
comparison is reported.

### The reproduction check, and the stop rule

> The 24-row half-space reproduction is run and reported **before any rung is interpreted**.
>
> - If it **passes**, the ladder's verdicts are stated as comparable to R's and to Y's, and R's
>   arm-2 numbers are treated as reproduced at this build.
> - If it **fails**, the note reports the failing fields and, where it can, which shipped change
>   since R's build `c38b5a6` accounts for them. The ladder is still run and reported — it is
>   this campaign's own measurement under the shipped rule and does not read a single field of
>   R's rows — but **no claim of comparability with R's arm-2 numbers is made**, R's ratios are
>   quoted as R's rather than as reproduced, and the verdicts are stated against **this
>   campaign's own 0.10 control cell at arm 2**, which is what they are evaluated against in
>   either case.

### The verdict rule

**Y's, unchanged, and through Y's code.** R's clauses **L**, **M**, **V**, **F**, **D**,
evaluated on each rung's cell against the **0.10 control cell of the same configuration** in the
same run:

- **L — the lineage establishes.** Form-3 bodies alive at the horizon ≥ 1 **and** form-3
  births > 0.
- **M — a new monoculture.** The rung's mean founder forms alive at the horizon is lower than
  the control's, **or** the rung's mean share of the horizon population held by its single most
  abundant form is ≥ 0.80 while the control's is < 0.80.
- **V — variety harmed.** Any founder form whose control-cell mean horizon population is ≥ 1.0
  falls below 0.60× that mean in the rung's cell, **or** any form present at the horizon in the
  control cell is absent from the horizon in at least the run threshold of the rung's runs.
- **F** and **D** are carried unrepaired, for comparability with R and Y, and are **reported and
  not read**.

**Acceptance is Astra's rule and Y's: a depth is acceptable if L holds and neither M nor V
holds.** The selected ecology is `fast-leaf`; `baseline` is reported, not weighed.

**Thresholds.** Y's generalisation, unchanged: a seed agrees when a majority of its own runs
agree, `⌈runs_of_seed / 2⌉`. At 6 runs over 6 seeds a clause needs ≥ 4 runs and ≥ 5 seeds, and
because arm 2 gives one run per seed those are the same six runs, so **the binding threshold is
5 of the 6 seeds** — exactly as at arm 0. The 0.10 cell is its own control and no clause is
evaluated for it.

### The measures

F's and R's census definitions and Y's two added measures are inherited **verbatim** and are not
restated: the generation-split margin (founder = a tick-0 `OrganismId`, descendant otherwise;
required by test to sum back to E's own bins) and the per-body served-by-channel profile in the
ledger's own `FOLIAGE, FRUIT, LITTER, CARRION` order, per `(form, generation)`. Nothing is added
and nothing is removed.

### Budgets

The ladder ≤ **6 wall minutes** on ≤ 8 workers, as the brief caps it; the 24-row reproduction run
is additional, is reported separately, and is budgeted at ≤ 2 further wall minutes.
`runs/ecology-v1-depth-ladder-arm2/` ≤ 30 MiB; Part 1's two run directories ≤ 12 MiB together.
Nothing left running, no held-out seed touched, `state/`, port 7393, the shim and the running
`cubarium` untouched.

### What this part is not

- It is **not a proposal to change the roster**, and it names no `depth` as shipped.
- It does not touch the diet, does not bundle a wet-floor producer, and does not fit a curve
  through six rungs.
- It cannot separate "`depth` steers the body" from "`depth` puts the body where the food is":
  `depth` has exactly one site in the simulation and every difference between rungs is that
  site's.
- Six training seeds, one arm, 150 simulated minutes. No held-out seed is touched and nothing is
  tuned.


## Addendum to the pre-registration — Astra's directions, registered before any result was read

*Astra's review of X and Y reached this workstream through Fable while Part 1's two training
arms were running and **before a line of either arm's output had been read**; no row of Part 2
existed. Everything in this addendum was committed at that moment. The registered rules and
thresholds above are **unchanged** — nothing whose rows had begun was altered — and this section
only adds, qualifies and names.*

**1. The registered rules and thresholds stand exactly as committed.** No clause, threshold,
column or branch above is edited.

**2. Part 1: the sixteen paired generations are descriptive, not inferential.** Sixteen
sequential generations of one run are one autocorrelated trajectory, not sixteen independent
observations, so the paired-by-generation sign-test p-values — X's and this part's alike — are
reported as **descriptive summaries of a within-run pattern** and are not read as evidence about
a population of runs. The **seed is the independent repeat**, and with X's seed and this part's
there are **n = 2**. Accordingly, if the registered rule's label *replicated* fires, the note
states it as **"the same directional pattern in a second pre-chosen training run"** and not as
statistical replication. The rule's arithmetic is unchanged; only the words the verdict is
reported in are fixed here.

**3. Part 1, added: the 2 × 2 of weights × adapter.** For **each** seed (20260915 and 20260916),
each arm's **selected centre** — by the selection rule registered above — is replayed on the same
four training layouts under **both** adapters with the intake and dwell trace, through
`es-turn-band`'s own machinery, weights untouched. That is four weight sets × two adapters, and
the reported columns are X's own: **`t_min`, on-food fraction, mean dwell bout and producer
intake per lived tick.**

> **The reading, registered here:** if swapping **only the adapter** on the same weights
> reproduces the gain, the effect is **expression**. If `cub-act-2`-trained weights keep their
> advantage under **both** adapters, the search reached a **different region**. If the advantage
> depends on the pairing — weights and adapter together — it is an **interaction**, i.e.
> co-adaptation. Anything that does not fall cleanly in one of the three is reported as such.

This is registered **before this workstream has looked at any second-seed result**. Its one
enabling code change is named: `es-turn-band` currently refuses a run whose centre file is not
`cub-act-1`; it is changed to check the centre file against **the run's own recorded adapter**
(`checkpoint.protocol.adapter`), which is what that check always meant, and nothing else about
the replay moves.

**4. Part 2: the treatment is named "the apex-arm treatment", not an apex mechanism.** Moving
from arm 0 to arm 2 changes predator presence, predator count, predation deaths, carrion
recycling and every feedback they carry **at once**. No sentence in this note attributes a
difference between arms to predation alone.

**5. Part 2: the predicate is reported, not assimilated.** R's rows and this ladder are **not**
treated as one predicate. The half-space and reach-envelope runs at the two reproduction depths
(0.10 and 0.55), same seeds and same build, are compared **explicitly and cell by cell**, and
what V's adoption does at arm 2 is reported as its own measurement rather than folded into the
ladder.

**6. Part 2: the per-rung sequences are reported raw and are not called monotone.** The
served-by-channel and margin sequences are reported rung by rung as numbers; Astra found that
Y's arm-0 sequences are **not** monotone at every rung, so no sentence in this note describes any
sequence as monotone unless the rung-by-rung numbers it cites are, and the direction is described
as a direction rather than as a law.

**7. Part 2: predation deaths by prey form are cross-tabulated.** `movement::Census`'s cells
already carry `deaths_by_cause` per `(form, diet bin, guild)`, over every body and not only the
founders, so the cross-tabulation is read out of the rows this campaign already writes. **No
extra run and no new world state is added for it.** If any part of the mechanism is not in the
rows, the note says the mechanism remains unresolved rather than adding a rerun.

**8. Neither part proposes a default or a roster change**, and two specific limits are stated in
the result:

- a second favourable `cub-act-2` seed would still **lack any whole-world evaluation**, because
  `es-population` — the only path from a policy to a whole calibrated world — **refuses
  `cub-act-2` by name** (X's deliverable-1 disclosure), so nothing here can say what the adapter
  does to a population;
- an acceptable rung in Part 2 would be a **candidate for held-out confirmation only**, never a
  roster value: six training seeds, one arm, one horizon.

**9. One note on a concurrent change on `main`.** Fable is making `World::set_action_adapter`
fallible there, so that it refuses a change that mismatches an interned policy. This worktree is
unaffected: it sets the adapter only through `es::fixture::Layout::with_adapter`, before any
policy is attached, and never after. Nothing in this workstream depends on the infallible
signature.

---

*Everything above was written before either part was launched. Everything below is what they
measured.*

# The headline

> **Part 1 — not replicated, and the reversal is in the weights, not the adapter.** At
> `--train-seed 20260916` the `cub-act-1` control is higher on **every** generation of both
> aggregate columns (16/16, the mirror image of X's 15/16 the other way) and its selected
> centre beats the `cub-act-2` arm on **all eight** held-out layouts, minimum 9,592 against
> 6,555. The registered rule reads **NOT REPLICATED**. Astra's 2 × 2 says where the difference
> lives: swapping *only the adapter* on frozen weights moves `t_min` by −223 / +124 at the
> first seed and −3,124 / +167 at the second, while swapping *which run's weights* moves it by
> +132 / +479 at the first seed and **−6,918 / −3,627** at the second. The effect is a
> **different region of weight space**, not expression — and across two seeds the region
> `cub-act-2` reaches is not consistently the better one. A likely reason is visible in the
> replay: the seed-20260916 `cub-act-1` centre already turns on **0.9997** of its measurable
> ticks, so the deadband Q measured as clipping the turn channel was not clipping *this*
> policy, and there was nothing for `cub-act-2` to release.
>
> **Part 2 — no rung is acceptable at arm 2 either, and V's predicate adoption has already
> removed most of the effect R's result rested on.** R's 24 arm-2 rows reproduce **field for
> field, 1,056 comparisons, 0 mismatches** under `--pursuit-stop half-space`, so the campaign
> is anchored and R's arm-2 numbers are reproduced at this build. Under the **shipped** rule
> the same cell reads differently: `fast-leaf` @ 0.55 gives a lineage in **3 of 6** worlds
> with 3.3 skimmers and a grazer at **0.82×**, where R's rule at the same seeds and the same
> build gives **5 of 6**, 8.3 skimmers and **0.46×** — both halves of the effect Y found
> living in the apex arms are **halved by the predicate the cube now runs**. Across the whole
> ladder no rung is acceptable in either configuration: in `fast-leaf` clause **L never holds**
> (best 4/6 seeds at 0.75, the one rung where **V** holds instead, grazer 0.58×); in
> `baseline` 0.55 establishes a lineage in 6/6 worlds and takes the grazer to 0.22×. The
> shipped rule roughly doubles to trebles predation at arm 2 and changes the state hash in
> **23 of the 24** cells where it is reachable. None of this is attributed to predation alone:
> the arm is a treatment — presence, count, deaths, recycling and their feedbacks at once —
> not a mechanism.

---

# Part 1 — X's second training seed, with its own control

## What ran

Both arms are the retained command verbatim with `--train-seed 20260916`. 16 pairs, 16
generations, horizon 36,000, `Aggregate::Min`, the same four training layouts, the same
`fast-leaf` ecology (`09e244392ec91768`), `--center-eval true`, 8 workers, the 20-minute cap;
no retry, no continuation, no tuning. The seed-20260915 arms were **not** re-run: X's two
retained runs are read from disk.

| arm | adapter | wall | episodes | ticks | protocol hash |
| --- | --- | --- | --- | --- | --- |
| control | `cub-act-1` | 279.0 s | 2,116 | 32,739,957 | `0x8e51a1a9b1e2742b` |
| treatment | `cub-act-2` | 171.9 s | 2,116 | 20,327,783 | `0x8304e9e2e02ab8fb` |

The two wall times are **not comparable to each other or to X's 139.5 s**: this session was
compiling in the same `target/` while the first arm ran, and the second arm's episodes die
sooner, which is itself the result. Ticks, not seconds, is the honest cost column.

**A reproduction, first.** The analysis script was run against X's two retained runs before it
was run against mine, and it reproduces X's published tables **number for number** — the
training table (6,521 / 6,505 at generation 0 through 7,110 / 7,495 at generation 15), the
sign tests (15/16 at p = 0.00052 on both aggregate columns, 14/16 at p = 0.00418 on opening
residence), and the held-out table (minimum 6,914 → 7,870, higher on 6 of 8). So the tables
below are produced by machinery that has been checked against X's on X's own data.

## The training table, seed 20260916

Column *1* is `cub-act-1`, column *2* is `cub-act-2`.

| gen | centre 1 | centre 2 | best 1 | best 2 | mean 1 | mean 2 | opening 1 | opening 2 | intake/tick 1 | intake/tick 2 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 6944 | 6699 | 7555 | 7790 | 6954 | 6848 | 0.0244 | 0.0160 | 3.13e−5 | 2.37e−5 |
| 1 | 7268 | 6767 | 7976 | 8199 | 6954 | 6895 | 0.0237 | 0.0172 | 3.04e−5 | 2.72e−5 |
| 2 | 6942 | 6719 | 8745 | 9491 | 7149 | 7025 | 0.0221 | 0.0182 | 3.32e−5 | 2.91e−5 |
| 3 | 7178 | 6733 | 10327 | 9263 | 7624 | 7032 | 0.0269 | 0.0200 | 4.30e−5 | 2.85e−5 |
| 4 | 9136 | 6799 | 13661 | 8563 | 8787 | 7180 | 0.0334 | 0.0192 | 6.26e−5 | 3.11e−5 |
| 5 | 13163 | 6806 | 13882 | 8849 | 10403 | 7170 | 0.0526 | 0.0191 | 8.25e−5 | 3.10e−5 |
| 6 | 11370 | 7045 | 15609 | 11433 | 9429 | 7376 | 0.0739 | 0.0197 | 8.55e−5 | 3.30e−5 |
| 7 | 8485 | 6887 | 15803 | 9574 | 9382 | 7438 | 0.1042 | 0.0204 | 9.30e−5 | 3.80e−5 |
| 8 | 8638 | 7783 | 18421 | 10845 | 10695 | 7948 | 0.1446 | 0.0262 | 1.17e−4 | 4.65e−5 |
| 9 | 9174 | 8810 | 20740 | 10473 | 11073 | 8001 | 0.1786 | 0.0334 | 1.29e−4 | 5.02e−5 |
| 10 | 10061 | 10335 | 19630 | 9859 | 11873 | 8392 | 0.1894 | 0.0337 | 1.37e−4 | 5.99e−5 |
| 11 | 10535 | 8752 | 19979 | 11579 | 12678 | 9188 | 0.2126 | 0.0485 | 1.49e−4 | 6.92e−5 |
| 12 | 10482 | 9050 | 23299 | 12549 | 15315 | 9145 | 0.1947 | 0.0532 | 1.57e−4 | 7.14e−5 |
| 13 | **17935** | 9639 | 21094 | 14287 | 14461 | 9695 | 0.1798 | 0.0675 | 1.49e−4 | 8.03e−5 |
| 14 | 17268 | 10754 | 23326 | 13057 | 15953 | 9756 | 0.1505 | 0.0751 | 1.53e−4 | 8.74e−5 |
| 15 | 17276 | **11184** | 23044 | 14401 | 16166 | 10695 | 0.1545 | 0.0911 | 1.56e−4 | 9.52e−5 |

Paired by generation, exact two-sided sign tests. **These p-values are descriptive**: sixteen
sequential generations of one run are one autocorrelated trajectory, not sixteen independent
observations, and the same qualification applies to X's.

| quantity | `cub-act-2` higher | p | read by the rule |
| --- | --- | --- | --- |
| mean population score | **0 / 16** | 0.00003 | **yes** |
| mean producer intake per lived tick | **0 / 16** | 0.00003 | **yes** |
| mean opening residence fraction | 0 / 16 | 0.00003 | no |
| best candidate | 3 / 16 | 0.02127 | no |
| centre score | 1 / 16 | 0.00052 | no |

Mean over the run: score 10,931 → 8,112; opening residence 0.1104 → 0.0362; intake rate
1.01e−4 → 5.01e−5. **Every one of these moves the opposite way from X's seed**, where the
same three read 7,245 → 7,446, 0.0245 → 0.0348 and 3.66e−5 → 4.26e−5.

## Held out, seed 20260916

Selection is X's rule, frozen before a held-out episode ran: `cub-act-1` selects generation 13
(centre 17,935), `cub-act-2` generation 15 (11,184).

| layout | control ticks | `cub-act-2` ticks | Δ | intake P control | `cub-act-2` |
| --- | --- | --- | --- | --- | --- |
| h1 | **36000** | 18305 | −17695 | 7.252 | 2.253 |
| h2 | 9592 | 6555 | −3037 | 0.954 | 0.057 |
| h3 | 29960 | 16555 | −13405 | 5.108 | 1.790 |
| h4 | 17800 | 15050 | −2750 | 2.829 | 1.864 |
| h5 | 11184 | 10154 | −1030 | 1.189 | 0.851 |
| h6 | **36000** | 15633 | −20367 | 8.211 | 1.742 |
| h7 | 11865 | 8666 | −3199 | 1.257 | 0.532 |
| h8 | 13717 | 10352 | −3365 | 1.566 | 0.705 |

Minimum 9,592 → 6,555; mean 20,765 → 12,659; higher on **0 of 8**. Mean opening residence
0.2360 → 0.1166; mean intake per lived tick 1.48e−4 → 8.61e−5. The control **survives two of
the eight held-out horizons outright**, which neither seed-20260915 arm did on any layout
(X: "neither arm survives a single holdout horizon"), and the `cub-act-2` arm survives none. Policy digests are the adapters' own,
`0x8be01a3aa8e9f4a2` and `0x97d426fd6bbb5ee1`.

## The verdict, by the rule registered before the runs

Both aggregate columns fail to favour `cub-act-2` — they favour `cub-act-1`, unanimously — so
the rule's second branch fires:

> **NOT REPLICATED.**

X's seed, put through the identical code, reads **REPLICATED** (both columns favour
`cub-act-2`, held-out minimum 6,914 → 7,870). Per the addendum, that label is reported as
*the same directional pattern in one pre-chosen training run*, not as statistical replication;
and the second pre-chosen run does not show it.

## The seed-by-seed picture, two seeds × two adapters

| | seed 20260915 | seed 20260916 |
| --- | --- | --- |
| mean population score, `cub-act-1` → `cub-act-2` (run mean) | 7,245 → **7,446** | 10,931 → **8,112** |
| generations `cub-act-2` higher | **15 / 16** | **0 / 16** |
| intake per lived tick (run mean) | 3.66e−5 → **4.26e−5** | 1.01e−4 → **5.01e−5** |
| opening residence (run mean) | 0.0245 → **0.0348** | 0.1104 → **0.0362** |
| selected centre | gen 9 (8,703) / gen 11 (8,959) | gen 13 (17,935) / gen 15 (11,184) |
| held-out minimum | 6,914 → **7,870** | 9,592 → **6,555** |
| held-out layouts `cub-act-2` higher | 6 / 8 | **0 / 8** |
| verdict by the registered rule | replicated | **not replicated** |

The two seeds disagree in **sign** on every column. The strongest single fact in the table is
not the disagreement but the scale of the seed effect: the seed-20260916 `cub-act-1` run ends
at a mean population score of 16,166 where **both** seed-20260915 arms end near 7,800. Between
seeds, one arm's runs differ by more than the adapter ever moves either arm.

## Astra's 2 × 2: weights × adapter

Each seed's two selected centres, replayed on the same four training layouts under both
adapters with the intake and dwell trace, weights untouched, 264 episodes per replay.
`t_min` is the minimum over the four layouts; the other columns are means over them.

| seed | weights | replayed under | `t_min` | on-food fraction | mean dwell bout | intake / tick | turn active fraction |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 20260915 | `cub-act-1` | `cub-act-1` | 8703 | 0.1071 | 87.4 | 5.39e−5 | 0.7955 |
| 20260915 | `cub-act-1` | `cub-act-2` | 8480 | 0.1102 | 76.5 | 5.60e−5 | 1.0000 |
| 20260915 | `cub-act-2` | `cub-act-1` | 8835 | 0.1299 | 82.3 | 6.59e−5 | 0.7684 |
| 20260915 | `cub-act-2` | `cub-act-2` | 8959 | 0.1164 | 73.5 | 6.03e−5 | 1.0000 |
| 20260916 | `cub-act-1` | `cub-act-1` | **17935** | **0.3588** | 92.1 | **1.67e−4** | **0.9997** |
| 20260916 | `cub-act-1` | `cub-act-2` | 14811 | 0.3446 | 85.4 | 1.61e−4 | 1.0000 |
| 20260916 | `cub-act-2` | `cub-act-1` | 11017 | 0.2113 | 80.3 | 1.01e−4 | 0.9866 |
| 20260916 | `cub-act-2` | `cub-act-2` | 11184 | 0.2046 | 76.2 | 9.81e−5 | 1.0000 |

The contrasts the reading names, per column:

| seed | column | adapter effect on `cub-act-1` weights | adapter effect on `cub-act-2` weights | **weight effect under `cub-act-1`** | **weight effect under `cub-act-2`** |
| --- | --- | --- | --- | --- | --- |
| 20260915 | `t_min` | −223 | +124 | **+132** | **+479** |
| 20260915 | on-food | +0.0031 | −0.0135 | +0.0228 | +0.0062 |
| 20260915 | dwell | −11.0 | −8.9 | −5.1 | −3.0 |
| 20260915 | intake/tick | +2.16e−6 | −5.60e−6 | +1.21e−5 | +4.32e−6 |
| 20260916 | `t_min` | −3124 | +167 | **−6918** | **−3627** |
| 20260916 | on-food | −0.0142 | −0.0067 | −0.1476 | −0.1400 |
| 20260916 | dwell | −6.7 | −4.1 | −11.8 | −9.2 |
| 20260916 | intake/tick | −6.15e−6 | −2.85e−6 | −6.58e−5 | −6.25e−5 |

**The reading, by the registered rule.** Swapping only the adapter does *not* reproduce the
gain at either seed: at 20260915 it moves `t_min` by −223 on one weight set and +124 on the
other, against a weight effect of +132 and +479; at 20260916 it moves it by −3,124 and +167
against a weight effect of −6,918 and −3,627. So the effect is **not expression**. At seed
20260915 the `cub-act-2` weights keep their advantage under **both** adapters on `t_min`,
on-food and intake — **a different region**, by the rule's second branch. At seed 20260916 the
`cub-act-2` weights are worse under **both** adapters, on every column, by a wide margin —
also a region effect, with the sign reversed. **Over the two seeds, the branch that fires is
"different region", and the region `cub-act-2` reached is better once and much worse once.**

The turn-activity column carries the mechanism. Under `cub-act-1`, X's seed-20260915 centre
turns on 0.7955 of its measurable ticks and the `cub-act-2`-trained centre on 0.7684 — both
well inside the band's reach, which is the operating point Q measured. The seed-20260916
`cub-act-1` centre turns on **0.9997** of them. That policy's raw turn head is already outside
the 0.05 band almost everywhere, so releasing the band has nothing to release — and the run
that was *given* the release from generation 0 never reached that region at all. On two seeds
this is a hypothesis with a measurement behind it, not a finding.

**Beside the 2 × 2**, each replay also applied X's own population-level replay rule to its
run's whole generation (centre plus 32 candidates, 132 trajectories per adapter). X's
condition reproduces exactly — `mixed`, on-food p = 0.3384, dwell p = 0.0356, `t_min`
p = 0.7283, X's three numbers to four places — which is this workstream's check that the one
change made to `es-turn-band` is inert on X's own data. The other three read `supported`
(s1-act2), `mixed` (s2-act1) and `falsified` (s2-act2): the rule's verdict depends on which
run's generation it is applied to, which is worth knowing and is not a result about the
adapter.

## What Part 1 does not establish, and proposes

**It proposes no training default**, under any branch, and the brief forbids one. Beyond X's
own list of limits, which all still stand:

- **Two seeds is two.** The rule fired one way on one and the other way on the other. Nothing
  here says `cub-act-1` is better either; it says the adapter is not what separated the two
  seed-20260915 runs, and that the seed dominates both.
- **No whole-world evaluation exists for `cub-act-2`.** `es-population`, the only path from a
  policy into a whole calibrated world, **refuses `cub-act-2` by name** (X's deliverable-1
  disclosure, `population.rs` being another workstream's file). So a favourable seed could not
  be taken to the cube even if there were two of them.
- **The seed-20260916 control is an outlier worth its own look and did not get one.** It is the
  first policy in this line to survive a held-out horizon, twice, and nothing here explains why
  this seed and not the other. That is a bigger question than the deadband.
- **Nothing about the dwell regression** is settled: mean dwell bout falls under `cub-act-2` in
  all four cells of the 2 × 2, between 4.1 and 11.0 ticks, exactly as X found on frozen
  weights.

---

# Part 2 — Y's ladder at arm 2

## The reproduction check, before anything was interpreted

**R's 24 arm-2 rows reproduce field for field under `--pursuit-stop half-space`: 24 of 24
matched, 1,056 field comparisons, 0 mismatches**, with `build_id` and `elapsed_ms` the only
exclusions, both named in the pre-registration before the check ran. 24 runs, 88.3 s.

| retained rows | rows checked | matched | fields compared | not in that file |
| --- | --- | --- | --- | --- |
| **R's own rows**, `runs/ecology-v1-depth-census/runs.jsonl` | **24** | **24** | **1,056** | 0 |
| A's screen, `runs/ecology-v1-calibration/screen/evals.jsonl` | 12 | **12** | — | 0 |
| I's ladder, `runs/ecology-v1-ladder/ladder/evals.jsonl` | 0 | 0 | — | 12 |
| M's present arm, `runs/ecology-v1-plant-budget/present-off/evals.jsonl` | 0 | 0 | — | 12 |

**One disclosure about the last two rows.** I's ladder and M's present arm contain **arm-0 rows
only** (60 and 12 rows, all arm 0), so at arm 2 there is nothing in them to check — "12 not in
that file", not twelve mismatches. The harness's `clean` gate requires `matched > 0` on every
retained file it is given and therefore printed its `STOP` banner on the reproduction run. That
banner is wrong here and is reported rather than suppressed: the run's only job was the check,
its tables were not needed, and the check it was for passed on every row of both files that
carry arm-2 rows at all. The ladder run itself was given no retained files, for the reason in
the pre-registration.

So the pre-registered stop rule's **first** branch holds: R's arm-2 numbers are reproduced at
this build, and the ladder's verdicts below are comparable to R's and to Y's. It also
establishes something R and Y could not: **across every shipped change between R's build
`c38b5a6` and this one, the only thing that moves an arm-2 world is the pursuit predicate** —
because holding the predicate fixed reproduces R bit for bit in all 24 cells, apex and all.

## The census at arm 2

Each row is a mean over the 6 runs (six training seeds, arm 2) of that cell, under the shipped
reach-envelope predicate. 72 runs, 244.5 s on 8 workers, 53,015 ticks/s, 12.96 M ticks, no
world ended empty.

| cand | `depth` | late pop | kinds at end | grazer | glider | burrower | **skimmer** | skimmer births | foliage × | worlds lost |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | **0.10** | 41.3 | 2.17 | 8.5 | 16.7 | 14.0 | **2.2** | 11.2 | 2.32 | 0 |
| baseline | 0.20 | 39.8 | 2.00 | 3.7 | 11.7 | 19.0 | **5.5** | 17.3 | 2.76 | 0 |
| baseline | 0.30 | 40.7 | 2.00 | 5.3 | 8.3 | 21.3 | **5.7** | 19.7 | 2.75 | 0 |
| baseline | 0.40 | 44.0 | 2.17 | 7.8 | 12.5 | 21.0 | **2.7** | 10.3 | 2.63 | 0 |
| baseline | 0.55 | 44.0 | 2.67 | **1.8** | 9.8 | 10.3 | **22.0** | 56.2 | 2.10 | 0 |
| baseline | 0.75 | 41.7 | 2.17 | **0.0** | 12.0 | 18.5 | **11.2** | 25.8 | 2.45 | 0 |
| fast-leaf | **0.10** | 60.0 | 3.00 | 19.7 | 28.5 | 11.8 | **0.0** | 8.0 | 2.14 | 0 |
| fast-leaf | 0.20 | 63.3 | 3.33 | 16.5 | 34.3 | 11.7 | **0.8** | 6.3 | 2.14 | 0 |
| fast-leaf | 0.30 | 59.0 | 3.00 | 25.8 | 21.2 | 11.8 | **0.2** | 5.5 | 2.15 | 0 |
| fast-leaf | 0.40 | 62.3 | 3.50 | 25.8 | 22.2 | 13.3 | **1.0** | 7.5 | 2.14 | 0 |
| fast-leaf | 0.55 | 62.3 | 3.33 | 16.2 | 30.5 | 12.3 | **3.3** | 15.5 | 2.13 | 0 |
| fast-leaf | 0.75 | 59.2 | 3.17 | 11.3 | 19.7 | 14.2 | **14.0** | 30.3 | 2.20 | 0 |

**Y's arm-0 table beside it**, for the same twelve cells (Y's note, unchanged):

| cand | `depth` | late pop arm 0 → arm 2 | grazer arm 0 → arm 2 | **skimmer** arm 0 → arm 2 | skimmer births arm 0 → arm 2 |
| --- | --- | --- | --- | --- | --- |
| baseline | 0.10 | 40.0 → 41.3 | 7.5 → 8.5 | 1.8 → **2.2** | 11.5 → 11.2 |
| baseline | 0.20 | 46.3 → 39.8 | 9.8 → 3.7 | 3.0 → **5.5** | 17.5 → 17.3 |
| baseline | 0.30 | 47.3 → 40.7 | 10.7 → 5.3 | 1.0 → **5.7** | 6.0 → 19.7 |
| baseline | 0.40 | 42.7 → 44.0 | 10.7 → 7.8 | 6.7 → **2.7** | 16.0 → 10.3 |
| baseline | 0.55 | 42.5 → 44.0 | 3.0 → 1.8 | 15.0 → **22.0** | 32.3 → 56.2 |
| baseline | 0.75 | 45.5 → 41.7 | 2.7 → 0.0 | 17.5 → **11.2** | 36.5 → 25.8 |
| fast-leaf | 0.10 | 62.3 → 60.0 | 16.8 → 19.7 | 0.0 → **0.0** | 8.0 → 8.0 |
| fast-leaf | 0.20 | 59.7 → 63.3 | 20.7 → 16.5 | 0.5 → **0.8** | 5.8 → 6.3 |
| fast-leaf | 0.30 | 60.7 → 59.0 | 17.2 → 25.8 | 1.7 → **0.2** | 6.5 → 5.5 |
| fast-leaf | 0.40 | 62.8 → 62.3 | 15.8 → 25.8 | 3.2 → **1.0** | 8.8 → 7.5 |
| fast-leaf | 0.55 | 58.5 → 62.3 | 15.2 → 16.2 | 3.3 → **3.3** | 11.5 → 15.5 |
| fast-leaf | 0.75 | 59.2 → 59.2 | 20.8 → 11.3 | 0.7 → **14.0** | 12.3 → 30.3 |

## The verdict, per rung

Each rung against the **0.10 control cell of its own configuration at arm 2**. 6 runs over 6
seeds; a clause needs 4 runs and 5 seeds, and because arm 2 gives one run per seed the binding
threshold is the 5 seeds. **Acceptable = L and not M and not V.** F and D are reported and are
not read.

**`fast-leaf` — the selected ecology.**

| rung | **L** | M | V | F | *D* | skimmer at the horizon | grazer | ratio to control | **acceptable** |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0.20 | no (2/6) | no | no | yes | no | 0.8 | 16.5 | 0.84× | **no** |
| 0.30 | no (1/6) | no | no | yes | no | 0.2 | 25.8 | 1.31× | **no** |
| 0.40 | no (3/6) | no | no | no | no | 1.0 | 25.8 | 1.31× | **no** |
| 0.55 | no (3/6) | no | no | no | no | 3.3 | 16.2 | 0.82× | **no** |
| 0.75 | no (**4/6**) | no | **yes** | no | no | 14.0 | 11.3 | **0.58×** | **no** |

**`baseline` — reported, not weighed.**

| rung | **L** | M | V | F | *D* | skimmer at the horizon | grazer | ratio to control | **acceptable** |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0.20 | no (2/6) | **yes** | **yes** | yes | no | 5.5 | 3.7 | 0.43× | **no** |
| 0.30 | no (1/6) | **yes** | **yes** | yes | no | 5.7 | 5.3 | 0.63× | **no** |
| 0.40 | no (2/6) | no | no | yes | no | 2.7 | 7.8 | 0.92× | **no** |
| 0.55 | **yes (6/6)** | no | **yes** | yes | no | 22.0 | 1.8 | **0.22×** | **no** |
| 0.75 | no (4/6) | no | **yes** | no | no | 11.2 | 0.0 | **0.00×** | **no** |

**No rung is acceptable in either configuration at arm 2**, which is Y's answer at arm 0 as
well — reached by a different route. In `fast-leaf` the binding clause is **L** at every rung,
as at arm 0; the best rung is now **0.75** at 4/6 seeds rather than 0.40, and it is the one
rung where **V** also holds, taking the grazer to 0.58×. In `baseline` the rung that
establishes a lineage in 6 of 6 worlds is 0.55, exactly as at arm 0, and it now costs the
grazer 0.22× rather than 0.40×.

## Per seed, because a mean of six is not agreement

A breeding skimmer lineage alive at the horizon (bodies in parentheses):

| cand | `depth` | 1001 | 1002 | 1003 | 1004 | 1005 | 1006 | seeds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.10 | no | no | no | no | no | yes (13) | 1/6 |
| baseline | 0.20 | no | no | no | yes (18) | no | yes (15) | 2/6 |
| baseline | 0.30 | no | no | no | no | no | yes (34) | 1/6 |
| baseline | 0.40 | no | no | yes (7) | yes (9) | no | no | 2/6 |
| baseline | 0.55 | yes (2) | yes (11) | yes (34) | yes (37) | yes (9) | yes (39) | **6/6** |
| baseline | 0.75 | yes (11) | no | yes (21) | no | yes (9) | yes (26) | 4/6 |
| fast-leaf | 0.10 | no | no | no | no | no | no | **0/6** |
| fast-leaf | 0.20 | no | no | yes (1) | yes (4) | no | no | 2/6 |
| fast-leaf | 0.30 | no | no | no | no | yes (1) | no | 1/6 |
| fast-leaf | 0.40 | no | yes (1) | yes (3) | yes (2) | no | no | 3/6 |
| fast-leaf | 0.55 | no | yes (3) | yes (15) | yes (2) | no | no | 3/6 |
| fast-leaf | 0.75 | no | yes (23) | yes (27) | yes (6) | no | yes (28) | **4/6** |

Grazer bodies alive at the horizon:

| cand | `depth` | 1001 | 1002 | 1003 | 1004 | 1005 | 1006 | mean |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.10 | 19 | 0 | 0 | 32 | 0 | 0 | 8.5 |
| baseline | 0.55 | 11 | 0 | 0 | 0 | 0 | 0 | 1.8 |
| baseline | 0.75 | 0 | 0 | 0 | 0 | 0 | 0 | 0.0 |
| fast-leaf | 0.10 | 20 | 21 | 14 | 20 | 17 | 26 | 19.7 |
| fast-leaf | 0.40 | 21 | 22 | 14 | 16 | 22 | 60 | 25.8 |
| fast-leaf | 0.55 | 12 | 17 | **0** | 27 | 18 | 23 | 16.2 |
| fast-leaf | 0.75 | 27 | **0** | **0** | 19 | 22 | **0** | 11.3 |

As at arm 0, the `baseline` grazer reaches the horizon in only 2 of 6 control worlds, so every
`baseline` V rests on those two worlds. The `fast-leaf` grazer's 0.58× at 0.75 rests on three
worlds — 1002, 1003 and 1006 — and those are three of the four worlds where the lineage
establishes. Seed 1001, which never established a `fast-leaf` lineage at any arm-0 rung, still
never does at arm 2.

## The skimmer's income by channel

Mean material taken off the field per body (m), pooled over the 6 runs of each cell. **These
are sequences, not laws**: read rung by rung.

| cand | `depth` | skimmer founder foliage | skimmer founder litter | skimmer descendant foliage | skimmer descendant litter | grazer founder foliage |
| --- | --- | --- | --- | --- | --- | --- |
| baseline | **0.10** | 0.01 | 6.49 | 9.41 | 2.36 | 1.05 |
| baseline | 0.20 | 0.05 | 4.40 | 14.90 | 1.12 | 0.67 |
| baseline | 0.30 | 0.07 | 3.37 | 10.70 | 1.15 | 1.86 |
| baseline | 0.40 | 0.08 | 2.75 | 16.49 | 0.61 | 0.83 |
| baseline | 0.55 | 0.14 | 1.97 | 17.43 | 0.19 | 0.89 |
| baseline | 0.75 | 4.63 | 1.28 | 29.51 | 0.04 | 0.10 |
| fast-leaf | **0.10** | 0.13 | 6.33 | 0.73 | 2.37 | 10.40 |
| fast-leaf | 0.20 | 0.26 | 4.50 | 10.10 | 2.73 | 9.48 |
| fast-leaf | 0.30 | 1.67 | 3.43 | 14.70 | 2.08 | 7.26 |
| fast-leaf | 0.40 | 2.93 | 2.60 | 16.00 | 0.91 | 6.80 |
| fast-leaf | 0.55 | 3.97 | 2.02 | 24.87 | 0.20 | 3.84 |
| fast-leaf | 0.75 | 5.10 | 1.23 | 23.60 | 0.05 | 4.94 |

Three readings, each with the numbers that carry it — and none of the sequences is called
monotone, because three of them are not:

- **The swap is in the same direction at arm 2 as at arm 0, and it is not monotone.** The
  skimmer's litter does fall at every rung of both configurations (6.49 → 1.28 and 6.33 → 1.23
  for founders). The descendant's litter in `fast-leaf` **rises** from 0.10 to 0.20 (2.37 →
  2.73) before falling; the descendant's foliage in `baseline` **falls** from 0.20 to 0.30
  (14.90 → 10.70) and again from 0.55 to 0.75 in `fast-leaf` (24.87 → 23.60). Astra's
  observation about Y's arm-0 sequences holds here too, so the direction is reported as a
  direction.
- **The grazer's founder gives up leaf as the skimmer takes it**, in `fast-leaf`: 10.40 → 3.84
  m from 0.10 to 0.55 while the skimmer founder goes 0.13 → 3.97, and the horizon *population*
  does not track it (0.82× at 0.55, 1.31× at 0.30). The plate and the population are different
  measurements, as Y found.
- **`baseline` again names the mechanism by exception.** Up to 0.55 the `baseline` skimmer
  founder's foliage never exceeds 0.14 m while its litter falls to a third; at 0.75 it reaches
  4.63 m. `baseline`'s `plant.foliage_rate` is 0.002 against `fast-leaf`'s 0.006.

There is still **no rung at which the skimmer is fed and is not on the leaf**, which is Y's
central result, and arm 2 does not change it.

## The margin, split by generation

Skimmer margin **rate** (e/s), pooled over each cell, at arm 2:

| cand | `depth` | founder bodies | **founder rate** | descendant bodies | **descendant rate** |
| --- | --- | --- | --- | --- | --- |
| baseline | **0.10** | 30 | −0.000324 | 67 | +0.000001 |
| baseline | 0.20 | 30 | −0.001509 | 104 | +0.001119 |
| baseline | 0.30 | 30 | −0.002027 | 118 | **+0.002577** |
| baseline | 0.40 | 30 | −0.002427 | 62 | +0.000476 |
| baseline | 0.55 | 30 | **−0.003046** | 337 | +0.001455 |
| baseline | 0.75 | 30 | −0.002895 | 155 | +0.001632 |
| fast-leaf | **0.10** | 30 | −0.000282 | 48 | −0.000856 |
| fast-leaf | 0.20 | 30 | −0.001381 | 38 | +0.000325 |
| fast-leaf | 0.30 | 30 | −0.001445 | 33 | +0.000308 |
| fast-leaf | 0.40 | 30 | −0.001702 | 45 | +0.000675 |
| fast-leaf | 0.55 | 30 | −0.001869 | 93 | +0.001221 |
| fast-leaf | 0.75 | 30 | −0.002428 | 182 | **+0.002547** |

The founder's rate falls with height in both configurations and the descendant's rises — Y's
finding, unchanged at arm 2 — and again the `baseline` sequences are not monotone at every
rung (the founder rate rises from 0.55 to 0.75, −0.003046 → −0.002895; the descendant rate
falls from 0.30 to 0.40, +0.002577 → +0.000476). The rescue is a **descendant** phenomenon in
both configurations at both arms.

## What the apex-arm treatment moved

This is reported as **the apex-arm treatment**, not as a fact about predation: moving from arm
0 to arm 2 changes predator presence, predator count, predation deaths, carrion recycling and
every feedback they carry at once, and nothing below separates them. The two campaigns' cells
are also one predicate apart at arm 2, so an arm-0 number and an arm-2 number are compared as
two measurements and are never differenced.

**The control cells barely move.** `fast-leaf` @ 0.10 arm 0 → arm 2: late population
62.3 → 60.0, grazer 16.8 → 19.7, glider 32.2 → 28.5, burrower 13.3 → 11.8, skimmer
0.0 → 0.0, kinds at the horizon 3.00 → 3.00. `baseline` @ 0.10: 40.0 → 41.3, grazer
7.5 → 8.5, burrower 13.2 → 14.0, skimmer 1.8 → 2.2. **Every founder kind that reached the
horizon at arm 0 still reaches it at arm 2**, at the control rung of both configurations.

**The treatment rungs move a lot, and the top one most.** `fast-leaf` @ 0.75 goes from 0.7
skimmers and a grazer at 1.24× at arm 0 to **14.0** skimmers, 4 of 6 seeds and a grazer at
**0.58×** at arm 2 — the rung that did nothing without predators becomes the rung with the
strongest lineage and the only `fast-leaf` **V**. `fast-leaf` @ 0.40, Y's best arm-0 rung at
4/6, falls to 3.2 → 1.0 skimmers and 3/6. **Which rung is "best" is a property of the arm**,
and the ladder's answer — no rung acceptable — survives the change while the rung that comes
closest to it does not.

**R's effect, and what the predicate did to it.** Y read R's arm-2 rows as 5/6 lineage and a
grazer at 0.46× in `fast-leaf` @ 0.55; this campaign reproduces exactly that under R's rule
(5/6, 8.3 skimmers, grazer 11.2 against the half-space control's 24.2) and measures **3/6,
3.3 skimmers and 16.2 against 19.7 — 0.82× — under the shipped rule**, at the same six seeds
and the same build. So the finding Y named as the reason to run this campaign is largely a
property of a predicate that is no longer shipped. That is a statement about six worlds per
cell and a handful of predation events each, and it is reported as a difference between two
conditions rather than an effect size.

**Predation deaths, cross-tabulated by prey form.** Read out of `movement::Census`'s own cells,
which carry `deaths_by_cause` per `(form, diet bin, guild)` over every body — the rows already
wrote it, so no rerun was added.

| cand | `depth` | grazer | glider | burrower | **skimmer** | all prey lost to predation | all prey deaths |
| --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.10 | 0.0 | 0.3 | 3.8 | **0.8** | 5.0 | 151.8 |
| baseline | 0.20 | 0.2 | 0.3 | 1.2 | **0.8** | 2.5 | 152.5 |
| baseline | 0.30 | 0.2 | 0.8 | 1.3 | **0.3** | 2.7 | 169.2 |
| baseline | 0.40 | 0.2 | 1.7 | 2.7 | **0.8** | 5.3 | 160.8 |
| baseline | 0.55 | 0.0 | 0.8 | 2.0 | **0.2** | 3.0 | 166.7 |
| baseline | 0.75 | 0.0 | 0.2 | 2.2 | **1.5** | 3.8 | 152.3 |
| fast-leaf | 0.10 | 1.2 | 2.8 | 3.7 | **0.7** | 8.3 | 162.0 |
| fast-leaf | 0.20 | 1.3 | 2.7 | 3.0 | **1.0** | 8.0 | 163.0 |
| fast-leaf | 0.30 | 1.5 | 2.8 | 2.3 | **1.0** | 7.7 | 168.7 |
| fast-leaf | 0.40 | 2.3 | 2.0 | 1.7 | **1.8** | 7.8 | 164.8 |
| fast-leaf | 0.55 | 1.5 | 3.5 | 1.8 | **2.0** | 8.8 | 168.8 |
| fast-leaf | 0.75 | 1.3 | 2.8 | 2.0 | **3.5** | 9.7 | 168.0 |

**Predation is under 6 % of all prey deaths in every cell** (2.5–9.7 of 152–169, i.e. 1.6–5.8 %), and it is not
where the burrower went: the burrower loses 1.7–3.8 bodies to predation across a run in which
its whole lineage disappears. **So the mechanism by which the apex arm empties the burrower
remains unresolved**, and this campaign does not add a run to resolve it. What the cross-tab
does say is that the skimmer's own predation load rises with height in `fast-leaf`
(0.7 → 3.5 from 0.10 to 0.75) alongside its population, which is what a larger lineage in a
world with predators looks like and is not evidence that predation shaped the ladder.

## What V's predicate adoption does at arm 2

The 24 cells the reproduction run and the ladder share: same seeds, same configurations, same
depths, same build, **one rule apart**. R's rows and this ladder are not treated as one
predicate, and this is the measurement that says why.

**23 of the 24 cells have a different `final_state_hash`.** The one that does not —
`baseline` / 0.55 / seed 1006 — is a run in which **no** body is lost to predation under
either rule, which is the only way the two can agree.

| cand | `depth` | predation deaths, envelope | half space | grazer, envelope | half space | skimmer, envelope | half space |
| --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.10 | **5.0** | 3.2 | 8.5 | 9.0 | 2.2 | 1.0 |
| baseline | 0.55 | **3.0** | 1.7 | 1.8 | 2.8 | 22.0 | 20.8 |
| fast-leaf | 0.10 | **8.3** | 3.3 | 19.7 | 24.2 | 0.0 | 0.0 |
| fast-leaf | 0.55 | **8.8** | 3.7 | 16.2 | 11.2 | 3.3 | 8.3 |

The shipped rule roughly **doubles to trebles predation** at arm 2 — which is the direction the
adoption was for: the reach envelope stops a pursuit when the prey is in contact rather than
merely somewhere ahead. The knock-on effects are not small and they are not one-signed: the
`fast-leaf` grazer reads 19.7 against 24.2 at 0.10 and 16.2 against 11.2 at 0.55, and the
`fast-leaf` skimmer at 0.55 reads 3.3 against 8.3 — a cell where R's rule would have given
the ladder a visibly stronger lineage. **Six runs per cell and a handful of predation events
each: these are differences between two conditions, not estimates of an effect size**, and
they are reported so that R's arm-2 rows and this ladder are never quoted as one campaign.

## Accounting

- **Conservation.** Worst `|material residual|` over 72 ladder rows **1.552e−11** (24
  reproduction rows: 1.359e−11); worst `|energy residual|` **1.405e−10** (1.124e−10) — both
  inside the contract's 1e−9.
- **Invariants.** `World::check_invariants` at every 600-tick sample of every run; no
  violation, or the run would have been an error rather than a row.
- **Ledger completeness.** **0** dropped closed records in 96 runs.
- **Refusals and failures: none.** 72 of 72 and 24 of 24 completed, 0 skipped.
- **Censoring: none.** No world ended empty.
- **Predation.** 2.5–9.7 prey per run, against 152–169 prey deaths per run.

## What Part 2 does not establish, and proposes

**No roster change is proposed and no `depth` is named as shipped.** Y's limits all carry over
with the arm changed, and three are this campaign's own:

- **Arm 2 only, and one arm is not a dose-response.** Arm 1 was not run, so nothing here says
  whether the differences between arm 0 and arm 2 scale with the cohort or arrive with the
  first predator.
- **The two campaigns' arms are not subtractable**, and at arm 2 they are also one predicate
  apart. An arm-0 ratio and an arm-2 ratio are compared as two measurements and never
  differenced.
- **The mechanism behind the predicate's effect on the lineage is not resolved.** Predation is
  under 6 % of prey deaths in every cell; that the shipped rule halves the 0.55 lineage while
  moving a handful of deaths per run is consistent with several stories and this campaign
  tests none of them. No rerun is added for it.
- **Six rungs, six training seeds, one horizon, one arm.** No held-out seed was touched and
  nothing was tuned. **An acceptable rung would be a candidate for held-out confirmation
  only**, never a roster value — and no rung was acceptable.
- Everything of Y's: one run per seed; six rungs of one locus, no curve; the foliage-bin
  association is still an association; 150 simulated minutes says nothing past the horizon; F
  and D are carried unrepaired and unread.

---

# What would change on the cube

**Nothing, from either part.** Part 1 trains policies under an adapter the display host cannot
run and `es-population` refuses; Part 2 proposes no roster value and found none it could
propose. `cubarium-core` is untouched by this workstream, no `WorldConfig` field was added, no
`config_hash` moved, and the running process, `state/`, port 7393 and the shim were not
touched.

# Build, commits and cost

| | |
| --- | --- |
| Branch | `worktree-agent-abb5e1c780bb966a7`, a worktree of `main` at `c60534d` |
| Pre-registration commit | **`2abe634`** — everything above the first horizontal rule, before a row existed |
| Astra's addendum | **`16c7d7d`** — committed while Part 1's arms were running and **before a line of their output was read** |
| Harness commit | **`d96aae3`** — `--arm`, `--levels`, `--pursuit-stop`, the optional `--census-rows` and `--pairs`, the adapter check, six new tests |
| Build stamps | Part 1's two training runs and its two held-out evaluations carry **`2abe634`**; the 2 × 2 replays and both Part 2 runs carry **`d96aae3`**. Both are pinned through `CUBARIUM_SEARCH_BUILD`, and `d96aae3` touches neither `es/trainer.rs` nor `es/fixture.rs`, so the two stamps name the same trainer and the same fixtures |
| Tests | `cargo test --release -p cubarium-search` **321 passed**, 0 failed, 5 ignored (6 new in `tests/depth_ladder_arm2.rs`); `-p cubarium-core` **579 passed**, 0 failed, 4 ignored — unchanged, which is what "core untouched" has to look like |

Files touched: `crates/cubarium-search/src/census.rs`, `crates/cubarium-search/src/es/turnband.rs`,
the `EsTurnBand` `--pairs` line in `main.rs`, `crates/cubarium-search/tests/{depth_ladder_arm2.rs,
depth_census.rs, depth_ladder.rs}`, `scripts/`, and this note. `cubarium-core`,
`precondition.rs`, `lifecycle.rs`, `evaluate.rs`, `calibrate.rs`, `factorial.rs`,
`apex_audit.rs`, `population.rs`, the ES trainer, protocol, export and fixture modules and
every other command in `main.rs` are **not** modified. `cargo fmt` was never run. The
row-producing binary was copied out of the shared `target/` before every run.

| what | wall | work |
| --- | --- | --- |
| Part 1, `cub-act-1` at seed 20260916 | 279.0 s | 2,116 episodes, 32.74 M ticks |
| Part 1, `cub-act-2` at seed 20260916 | 171.9 s | 2,116 episodes, 20.33 M ticks |
| Part 1, export + two held-out evaluations | 16 s | 16 episodes |
| Part 1, the 2 × 2 replays | 112 s | 1,056 episodes, four runs |
| Part 2, the reproduction run | **88.3 s** | 24 runs, 4.32 M ticks, 48,902 ticks/s |
| Part 2, the ladder | **244.5 s** | 72 runs, 12.96 M ticks, 53,015 ticks/s |

Part 2's ladder is inside the brief's 6 wall minutes and the reproduction run inside the
addendum's further 2. Part 1's first training arm shared the machine with this session's own
compiles and its wall time is not comparable to X's 139.5 s; ticks are the honest column.

Storage, against the brief's budgets: `runs/es-eco-v1-fastleaf-s2` **4.8 MiB** and
`runs/es-eco-v1-fastleaf-act2-s2` **4.8 MiB** (9.6 of 12); `runs/ecology-v1-depth-ladder-arm2`
**1.2 MiB** of 30; `runs/ecology-v1-round5-followups/replay` **980 KiB**, the four 2 × 2
replays.

## Exact commands

```bash
# Part 1, both arms, twice with one word changed
CUBARIUM_SEARCH_BUILD=2abe634 cubarium-search es-train \
    --config runs/ecology-v1-calibration/selected/fast-leaf.toml \
    --pairs 16 --generations 16 --horizon 36000 --workers 8 \
    --wall-seconds 1200 --train-seed 20260916 --center-eval true \
    --adapter {cub-act-1,cub-act-2} \
    --out runs/es-eco-v1-fastleaf{-s2,-act2-s2}

CUBARIUM_SEARCH_BUILD=2abe634 cubarium-search es-export \
    --checkpoint <arm>/checkpoint.json --config <fast-leaf.toml> \
    --generation {13,15} --out <arm>/selected/center-000NN-policy.json
CUBARIUM_SEARCH_BUILD=2abe634 cubarium-search es-evaluate \
    --policy <arm>/selected/center-000NN-policy.json --set holdout \
    --horizon 36000 --wall-seconds 300 --config <fast-leaf.toml> \
    --adapter {cub-act-1,cub-act-2} --out <arm>/holdout.json

# Astra's 2 x 2: four replays, each under both adapters, no stability half
CUBARIUM_SEARCH_BUILD=d96aae3 cubarium-search es-turn-band \
    --run <one of the four runs> --config <fast-leaf.toml> \
    --generation {9,11,13,15} --horizon 36000 --workers 8 --wall-seconds 900 \
    --out runs/ecology-v1-round5-followups/replay/<label>

# Part 2, the reproduction run: R's two levels, arm 2, R's predicate
CUBARIUM_SEARCH_BUILD=d96aae3 cubarium-search census \
    --seeds 6 --ticks 180000 --sample-every 600 --introduce-tick 6000 \
    --workers 8 --wall-seconds 600 --arm 2 --levels 0.10,0.55 \
    --pursuit-stop half-space \
    --retained <A's screen>,<I's ladder>,<M's present-off> \
    --census-rows runs/ecology-v1-depth-census/runs.jsonl \
    --out runs/ecology-v1-depth-ladder-arm2/reproduction

# Part 2, the ladder: six rungs, arm 2, the shipped predicate, no retained target
CUBARIUM_SEARCH_BUILD=d96aae3 cubarium-search census \
    --seeds 6 --ticks 180000 --sample-every 600 --introduce-tick 6000 \
    --workers 8 --wall-seconds 600 --arm 2 \
    --levels 0.10,0.20,0.30,0.40,0.55,0.75 --pursuit-stop reach-envelope \
    --retained "" --census-rows "" \
    --out runs/ecology-v1-depth-ladder-arm2/ladder
```

Every table above that the harness does not print is reproducible from the retained rows by one
command: `runs/ecology-v1-depth-ladder-arm2/analyse.py` (Y's, unchanged),
`runs/ecology-v1-depth-ladder-arm2/analyse-arm2.py` (the predicate and predation tables) and
`scripts/xy2-part1-analyse.py` and `scripts/xy2-part1-2x2-table.py` (Part 1's).

## Routine decisions made here, and their visible effect

| decision | effect |
| --- | --- |
| `--pursuit-stop` defaults to the **shipped** reach envelope | every other command in this binary defaults to the shipped rule; reproducing an old row is the thing that has to be asked for, not running the current world |
| the arm-2 reproduction is a **separate 24-row run**, not a flag on the ladder | the ladder is one condition end to end, and the 24 shared cells become a measurement of V's adoption rather than a confound inside the ladder |
| the ladder runs with `--retained ""` and `--census-rows ""` | nothing retained was ever run at arm 2 under the shipped predicate, so there is no target; pointing the check at R's rows would have manufactured 24 "mismatches" that are a change of condition |
| `--levels` refuses a rung off the ladder | a seventh height would be a different campaign, not a wider one |
| every row records `pursuit_stop` by the **core's own name** | a row cannot disagree with the rule it ran under, and the field is `#[serde(default)]` so R's rows, which lack it, are still readable and are never compared on it |
| `es-turn-band` checks the centre file against the **run's own** recorded adapter | what that check always meant; pinning it to `cub-act-1` refused a `cub-act-2` run's own centre, which is half of the 2 × 2. The change is inert on X's condition: X's replay reproduces to four decimal places |
| `--pairs` became optional on `es-turn-band` | Q's pair reduction exists for one run and one generation; replaying another run's centre should not have to name it |
| the predation cross-tab is read out of `movement::Census`'s existing cells | Astra asked for it only if the rows carried it; they do, over every body and not only the founders, so no run was added |
| Part 1's analysis was run against **X's retained runs first** | the tables below are produced by machinery checked against X's published numbers on X's own data before it touched mine |
| the reproduction run's spurious `STOP` banner is **reported, not suppressed** | I's and M's retained files hold arm-0 rows only, so `matched > 0` cannot hold for them at arm 2; the banner is a property of the gate, not a failed check, and hiding it would have been the dishonest fix |
