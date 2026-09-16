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
