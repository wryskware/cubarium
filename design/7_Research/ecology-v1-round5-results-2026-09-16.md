---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Ecology v1, round 5: the turn deadband, the depth ladder, the coupled grazed opening

> **Retained 2026-09-18.** This is the cited round-5 synthesis. Links to round
> notes and briefs deleted with the flat-era material are Git history.

Fable's consolidation of the round-5 workstreams, dispatched from Astra's
cleared round-4 order ([round-4 result](ecology-v1-round4-results-2026-09-16.md),
[review](ecology-v1-round4-review-2026-09-16.md)). Round 4's own additions —
U (grasp-only apex radius), V (the reach-envelope predicate adopted, schema
17, deployed to the cube at `46b6d18`) and W (the apex-motor 2 × 2) — are
recorded in the round-4 result and cleared there. This note carries items 4,
5 and 6: X (the turn deadband alone), Y (the skimmer depth ladder) and Z (the
coupled grazed opening, in flight). Each workstream's own note is the record;
this is the reading across them and the decisions it supports.

## Dispatch and budget

| stream | brief | model / effort | wall (measurement) | storage | merged |
| --- | --- | --- | ---: | ---: | --- |
| X | [turn deadband](../handoffs/ecology-v1-turn-deadband-opus-2026-09-16.md) | Opus 5, high | 17 s replay + 140 s training | 5.1 MiB | `16f94c2` (+ `997fb24`) |
| Y | [depth ladder](../handoffs/ecology-v1-depth-ladder-opus-2026-09-16.md) | Opus 5, medium | 254 s | 0.8 MiB | `80a3b20` |
| Z | [grazed opening](../handoffs/ecology-v1-grazed-opening-opus-2026-09-16.md) | Opus 5, high | 304 s | 6.5 MiB | `261240a` |
| XY2 | [follow-ups](../handoffs/ecology-v1-round5-followups-opus-2026-09-16.md) | Opus 5, medium | 451 s training + 112 s replays + 333 s ladder | 11.8 MiB | `8dac862` |

Fable's verification at each merge: the crate suites on the branch; a re-run
of the campaign with the branch's binary compared field for field against
the retained rows (X: replay and stability identical but for path strings; Y:
72 of 72 rows identical); the five crate suites on the merged tree (579 core,
315 search, 599 host, 101 render, 93 surface after Y); `graft build`.

## X — the turn deadband alone: frozen weights do not forage better; a retrained arm gains on aggregate columns; the two experiments disagree

[Note](ecology-v1-turn-deadband-2026-09-16.md). Astra's bounded change from
Q: `TURN_DEADBAND` 0.05 → 0.0 and nothing else. X built it as a second
action adapter, `cub-act-2` (`ActionAdapter::{CubAct1, CubAct2}`, a `World`
transient beside the motor model; the thrust band stays 0.05), with its own
profile text so the protocol hash and policy digest change and every
existing policy is refused by name; `--adapter` on `es-train`, `es-evaluate`
and the replay command (`es-turn-band`); `es-population` refuses `cub-act-2`
outright rather than run it, because its stage runner was another
workstream's file. Byte-identical pins printed at the brief commit (a neural
world over 3,000 ticks, and a small-turn fixture that hashes differently
under `cub-act-2`, so the pin is not vacuous); the retained protocol hash
and `holdout.json` reproduce field for field.

**Replay first, rule registered before it ran.** The frozen generation-9
centre and its sixteen candidates on their own layouts, 264 episodes, both
adapters:

| measure | + / − | p | median Δ |
| --- | ---: | ---: | ---: |
| turn active fraction | 132 / 0 | 3.7e−40 | +0.206 |
| on-food fraction | 72 / 60 | 0.34 | +0.003 |
| mean dwell bout | 53 / 78 | **0.036** | **−3.2 ticks** |
| `t_min` (= score, every episode dies) | 18 / 15 | 0.73 | +13 |

**Verdict by the rule: mixed.** Support fails (on-food and `t_min` do not
rise); falsification fails on its own terms because dwell moves
significantly, in the negative direction. **Read substantively (Astra, P1):
the frozen-policy expression hypothesis is refuted at this operating point,
and dwell worsened.** The rule's "mixed" branch let a significant
deterioration authorise the training run; that label stays as history, the
training run is the exploratory follow-up it permitted, and it is not
evidence that the bottleneck moved to learnability. Q's prediction about the
*channel* is confirmed exactly: under `cub-act-2` the body turns on every
tick (turn activity 1.000 on all 132 trajectories against 0.72), in more and
shorter bouts. The inference from that to residence does not hold on frozen
weights.

**The bounded pair, permitted by "mixed".** The retained command verbatim
with `--adapter cub-act-2`, 16 pairs, 16 generations, seed fixed, the
retained `cub-act-1` run as the control (not re-run), 140 s:

| paired by generation | `cub-act-2` higher | p |
| --- | ---: | ---: |
| mean population score | 15 / 16 | 0.0005 |
| mean producer intake per lived tick | 15 / 16 | 0.0005 |
| mean opening residence | 14 / 16 | 0.004 |
| best candidate | 10 / 15 | 0.30 |
| centre score | 7 / 16 | 0.80 |

Held out (selected generation 11 against the control's 9): higher on 6 of 8
layouts, minimum 6,914 → 7,870, intake per tick +20 %, **opening residence
unchanged** (0.032 → 0.033), and neither arm survives a single horizon.

**Stability, from Q's retained sixteen pair contributions.** Split-half
cosine over all 6,435 balanced splits: mean 0.016 (null sd 0.010 in 10,215
dimensions), so about 1.6 % of an eight-pair direction's squared length is
shared; bootstrap of the full direction against the recorded update: mean
cosine 0.26. Two qualifications X states: the split reuses the full
32-candidate centred ranks (which can only make halves look more alike), and
it is one generation of one run. Whether sixteen pairs suffice is not
concluded; a second `--train-seed` arm or an episode-matched `--pairs 32
--generations 8` would decide it.

**X's reading, which Fable carries forward for Astra:** the two experiments
disagree and that is the finding. Releasing the band does not make frozen
weights forage better; it lets the search move a channel that was clipped to
zero for most of a newborn candidate's ticks, and the training arm gains
consistently on aggregate columns while its held-out residence does not
move. "The search can exploit the released channel" is plausible;
"learnability rather than expression" is not identified, and sixteen
sequential generations are one autocorrelated trajectory, so the paired
p-values are descriptive. A second training seed is the independent repeat
(n = 2 over seeds), and the sharper test Astra names is the selected
centres replayed under both adapters as a weights × adapter 2 × 2; both are
in XY2.

**What changed on the cube: nothing.** The host names no adapter and runs
`cub-act-1`, byte for byte. A `cub-act-2` policy is refused by name at
attachment, at founding and by `PolicyFile::policy`. **Fable added one more
refusal at integration (`997fb24`):** X had widened the decode-time check to
accept any adapter this build knows, so a snapshot holding a `cub-act-2`
policy would have resumed silently under the shipped adapter (the adapter is
a transient the bytes cannot carry). `World::from_state` now refuses such a
snapshot by name, with a test; a shipped-adapter world round-trips as before.
After Astra's review (P1), `World::set_action_adapter` is fallible and atomic
too: it validates every attached policy against the requested adapter and
refuses a mismatch by name, leaving the world unchanged, so a live world can
no longer be flipped under an attached policy (both directions tested).
Astra's P3 on Y — the seed-agreement rule read one of two as a majority — is
made a strict majority (`runs / 2 + 1`), which changes no retained cell.

## Y — the skimmer depth ladder, arm 0: no rung is acceptable; lineage success goes with a shift onto the grazer's foliage channel

[Note](ecology-v1-depth-ladder-2026-09-16.md). R's named next task, on R's
census, pre-registered before a row existed: `skimmer.depth` ∈ {0.10, 0.20,
0.30, 0.40, 0.55, 0.75} × {`baseline`, `fast-leaf`} × six training seeds,
arm 0, 72 runs, with R's two cheap measures added (E's margin split by
generation; each body's foliage and litter served by channel). **R's 24
arm-0 rows reproduce field for field across the schema-17 predicate change**,
which is the second check that the adoption reaches nothing without a
predator; A's, I's and M's retained rows reproduce too (1,056 comparisons, 0
mismatches).

| `fast-leaf` rung | L (seeds, 5 of 6 binds) | M | V | skimmer | grazer (× control 16.8) | acceptable |
| --- | --- | --- | --- | ---: | ---: | --- |
| 0.20 | no (2/6) | no | no | 0.5 | 1.23× | no |
| 0.30 | no (3/6) | no | no | 1.7 | 1.02× | no |
| 0.40 | no (4/6) | no | no | 3.2 | 0.94× | no |
| 0.55 | no (3/6) | no | no | 2.7 | 0.90× | no |
| 0.75 | no (2/6) | no | no | 0.7 | 1.24× | no |

`baseline` (reported, not weighed): the one rung whose lineage establishes
in 6 of 6 (0.55) takes the grazer to 0.40×; 0.75 to 0.36×.

**No depth is acceptable, in either configuration.** In the selected
ecology the failing clause is L at every rung: the lineage establishes in
at most 4 of 6 worlds. In Astra's words, the choice becomes three viable
heights and four kinds, or a wet-floor producer, which is a new food web and
not a repair. **No roster change is proposed.**

**What the two added measures showed.** Material taken per `fast-leaf`
skimmer descendant across 0.10 → 0.75: litter 2.60 → 0.02 m, foliage 1.64 →
26.3 m (the grazer's own descendants take 22–26). The shift is large overall
and not monotone at every rung (litter rises at 0.20, foliage peaks at 0.55;
baseline's sequence is rougher), and the margin sequences dip in both
configurations. What it establishes is **increasing dietary overlap with the
grazer's foliage and declining litter use**; it does not establish that the
skimmer removed material the grazer would otherwise have received, or that
the rescue and the grazer's cost are one mechanism — direct displacement was
not isolated, and at arm 0 the grazer's horizon population is 0.90–1.24×
control at every rung (Astra, P1). The rescue is a descendant phenomenon in
both configurations; R's founder-lifetime reversal is the founder's income.
R had disclosed that its pooled ledger could not separate founders from
descendants, so Y refines that record rather than correcting a sign claim.

**Found unasked, and it bears on R's reading:** re-reading R's own retained
rows by arm, R's `fast-leaf` result is largely a property of the **apex-arm
treatment** (arm number changes predator presence and count, predation,
recycling and feedbacks together; why the arm matters is not identified). At
0.55, arm 0 gives grazer 0.90× and lineage 3/6; arms 1 and 2 give 0.47× and
0.46× with lineage 5/6. R's rows reproduce exactly and R's pooled cell is
what R said; but R's sentence "nothing here turns on the apex" is wrong for
`fast-leaf` (`baseline` is uniform across arms). The ladder was run in the
arm where the effect is weakest, which the brief chose. **The same ladder at
arm 2 is the obvious next measurement**; named, not launched.

Routine decisions Y made, all pre-registered: seed agreement generalised to
a majority of a seed's own runs (reproduces R's 2-of-3 at R's cell size; at
one run per seed, 5 of 6 seeds binds); R's clauses F and D carried unrepaired
and unread by the acceptance, for comparability; served quantities recorded
for every form.

## Z — the coupled grazed opening: S's first-hour transient is removed, the founder gains reproduce, and 96,000 reads "better target" in both configurations

[Note](ecology-v1-grazed-opening-2026-09-16.md) ·
[pre-registration](ecology-v1-grazed-opening-preregistration-2026-09-16.md)
(committed before the operator, the tests or a row existed) · merged at
`261240a`. `World::remove_all_animals` removes every organism and neural
entry, refuses by name when an extension holds bodies (hunter members, quiet
pauses, dormant apexes, paired gestations or parentage), and books the
removed bodies' `Organism::material()` (escrow included, so the identity
holds for a gestating body) as a negative on `external_material_in` — the
mirror of `found_roster`'s booking, no new field; the bodies' energy is
reported (55–82 units per removal) and booked nowhere, stated as a limit.
Six tests in `found_roster.rs`. `precondition --stage grazed` runs the
ordinary coupled world to the declared ages, empties a clone at each,
founds the identical roster, saves and re-decodes the opening, and then
re-runs its own burn-in for the arm so the measured chain stays
snapshot-free; five tests in `precondition_measures.rs`. Fable's
verification: 573 core and 295 search on the branch; the whole campaign
re-run with the branch's binary reproduces all 48 arm rows field for field
(matched by configuration, seed and age; the workers write rows in
completion order) and all 12 burn-ins but for the snapshot's byte length,
whose header carries the build id; 586/321/599/101/93 on the merged tree.

**The declared deviation.** The brief asked for S's `Evaluation` row shape;
that is unreachable without editing `evaluate.rs`, which the brief forbade,
so Z declared it in the pre-registration and wrote its own recorder from the
shared public definitions, checked measure for measure against S's on the
twelve status-quo rows (204 of 204 exact). The `Evaluation`-only measures
(spatial windows, census, margins, the four-way depleted-cell reading) are
not reported. **Reproductions:** 12 of 12 status-quo rows carry S's retained
state and ecology hashes across the schema-17 predicate change (arm 0, no
predator); 36 of 36 founding hashes equal the independently saved openings;
residuals ≤ 9e−10.

| `fast-leaf` opening | opening ΣP | bred /24 | lineages | evenness | pop | forms | late ΣP (abs) | late ÷ opening |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| status quo | 107 | 14.8 | 7.3 | 0.64 | 62 | 3.0 | 226 | 2.11 |
| plant-only 48,000 (S) | 401 | 24.0 | 11.7 | 0.70 | 58 | 3.2 | 228 | 0.57 |
| coupled-grazed 48,000 | 204 | 20.2 | 11.5 | 0.66 | 65 | 3.0 | 228 | 1.12 |
| coupled-grazed 96,000 | 221 | 22.3 | 13.2 | 0.69 | 64 | 3.2 | 230 | 1.04 |
| coupled-grazed 180,000 | 229 | 23.3 | 12.7 | 0.70 | 65 | 3.5 | 229 | 1.00 |

**The denominator, settled at the mean.** Every configuration/opening mean
ends at 222–248 absolute foliage (rows span 211–338; one baseline seed's
herbivore collapse is the top of that range) — S's grazed standing crop from
a third direction — so
S's 2.31 and 0.63 were the same late field over openings 3.7× apart. **The
first hour:** the status quo greens by +117 % of its opening, the plant-only
opening browns by −40 %, the coupled-grazed openings move by −13 % to +9 %.
S's transient is not inverted; it is removed. Terminal starved cells on the
common §11 reference stay near the status quo at every coupled age (10.8 →
19.5 against S's plant-only 180,000 at 82.7), a net effect consistent with
S §4.2's recycling reading — not a confirmation of recycling as the cause,
since a coupled burn-in changes the whole animal regime at once. **By kind:** grazer founders breeding
0.3 → 10 of 10 in the selected `fast-leaf` cell (9.7 at baseline 48,000)
and first broods at the 3,001-tick floor in 6 of 6 seeds, S's gains from a
different opening; two costs S's opening did not have —
the skimmer founders fall to 1.3–3.5 of 5 at 48,000 and 96,000 (the burn-in
grazes the wet band down; the frames show it) and recover only at 180,000,
and founder broods fall in nearly every paired seed while lineages, forms
and population rise (descendants carry the population). Not scored: the
baseline status quo loses one seed's whole herbivore guild; 0 of 36
coupled-grazed arms lose one.

**Reading, by the pre-registered rule** (founder gains ∧ opening within 20 %
of the late field ∧ starved cells within noise): `fast-leaf` reads *better
target* at 96,000 and *not* at 48,000 and 180,000 (starved-cell excesses of
+1.3 and +2.2 cells against a tight sd of 1.33); `baseline` reads *better
target* at 96,000 and 180,000 and *not* at 48,000 (one grazer founder in one
seed against an equality clause). **The one age both agree on is 96,000**,
as a group-mean reading (baseline seed 1006 alone opens 46 % above where it
settles).
Z applied the rule as written and says where it reads badly: clause 3's
baseline tolerance is manufactured by one seed's guild collapse, and the
rule has no clause for the two costs found. **No §11 change and no
`producer.initial_fraction` change is proposed.** Its "Neither" is
strengthened by the frames: the field's total is stationary while its
*spatial* pattern is not (the wet band's reeds are eaten off in every
column), so a §11 rule reproducing the grazed *total* would still not
reproduce the grazed *field* — Astra's "no uniform total", seen.

**Cost of adoption, measured:** a coupled burn-in runs at ~6,700 ticks/s
per core, about 1.3× the plant-only prefix, so preconditioning coupled is
not materially dearer than S's option; at 96,000 ticks that is about 15 s
per core before a fresh world can be shown.

**Frames:** `assets/ecology-v1-grazed-opening-2026-09-16.png`, three
openings × two configurations × (founding, +1 h), seed 1001, Front face,
through the real presenter. At the founding the coupled-grazed reed bed is
visibly thinner than the plant-only one; an hour later all three are much
alike.

## XY2 — the two cheap follow-ups: X's gain does not replicate, and the shipped predicate has halved R's arm-2 effect

[Note](ecology-v1-round5-followups-2026-09-16.md) ·
[brief](../handoffs/ecology-v1-round5-followups-opus-2026-09-16.md) · merged
at `8dac862`. Both parts pre-registered before a row existed; Astra's
directions registered while Part 1's training ran and before any output
was read. Fable's verification: 321 search on the branch; the arm-2 ladder
re-run with the branch binary reproduces all 72 rows field for field; the
training runs were not re-run (deterministic, and XY2's analysis script
reproduced X's retained tables number for number before touching its own).

**Part 1 — X's second training seed: not replicated.** At `--train-seed
20260916`, with its own `cub-act-1` control, the direction reverses on every
column: `cub-act-2` higher in **0 of 16** generations on mean population
score and on intake per lived tick (X's seed: 15 of 16), held-out minimum
9,592 → 6,555 (X's: 6,914 → 7,870), higher on 0 of 8 held-out layouts. The
registered rule's second branch fires. The weights × adapter 2 × 2 of the
four selected centres, registered before the second seed was read, says
where the difference lives: swapping only the adapter moves `t_min` by
−223/+124 (seed 1) and −3,124/+167 (seed 2); swapping which run's weights
moves it by +132/+479 and **−6,918/−3,627**. **On the selected centres the
difference is carried by the weights the search produced, not by acute
decoding of the same weights** — a different region, better once and much
worse once. The crossed replay cannot say whether the adapter altered the
training trajectory that produced those weights, and it says nothing about
other seeds, band widths, scores, pair counts or optimisers. The number
behind it: the seed-2 `cub-act-1` centre already turns on 0.9997 of its
measurable ticks, so the deadband was not clipping that policy and there was
nothing to release. **No training default proposed**, and none is
supportable: the seed is the replicate (n = 2, one each way), the
generation-wise p-values are descriptive, and `es-population` refuses
`cub-act-2` so no whole-world evaluation exists. XY2 also notes, unchased:
the seed-2 `cub-act-1` control survives two held-out horizons outright, the
first policy in this line to survive any, and nothing explains why that
seed. **Fable's reading, scoped as Astra asked:** the 0.05 → 0.0 deadband
change is closed **as a default and as the next training line under this
fixed protocol**; not the adapter question in general. Q's channel finding
was real and its inference to residence was not; the released band neither
helps frozen weights nor reliably helps this search. The interesting object
is the surviving seed-2 control, which deserves one bounded diagnostic (item
1 below), not installation or another campaign.

**Part 2 — Y's ladder at arm 2: no rung acceptable, and the predicate is
load-bearing.** R's 24 arm-2 rows reproduce field for field under
`--pursuit-stop half-space` (1,056 comparisons, 0 mismatches), which also
establishes that across every shipped change since R's build, the only thing
that moves an arm-2 world is the pursuit predicate. Under the shipped reach
envelope: `fast-leaf` L never holds (best 4 of 6 seeds at 0.75, the one rung
where V also holds at 0.58×; Y's arm-0 best rung 0.40 falls to 3 of 6);
`baseline` 0.55 establishes in 6 of 6 and takes the grazer to 0.22×. **At the
same seeds and build, `fast-leaf` at 0.55 reads 3 of 6 lineage, 3.3
skimmers, grazer 0.82× under the shipped rule against 5 of 6, 8.3, 0.46×
under R's half-space:** both halves of the effect that motivated this
campaign are halved by the predicate the cube now runs, which doubles to
trebles predation at arm 2 and changes the state hash in 23 of 24 cells.
Named throughout as the apex-arm treatment; predation deaths cross-tabulated
by prey form from the census's existing cells are under 6 % of prey deaths in
every cell, so the mechanism is stated as unresolved. Per-rung channel and
margin sequences reported raw and not called monotone. **No roster change
proposed.** **Fable's reading, scoped as Astra asked:** R's fast-leaf result
was a property of the old predicate in the arms with predators. What is
closed is **the six tested values of the roster skimmer's depth locus at arms
0 and 2, at this price, horizon, seed set, ecology and shipped predicate**.
Diet, metabolism, form and their interactions were not varied, arm 1 was not
run, and a wet-floor producer is one world-level alternative among others.

## What this does and does not establish

- The turn band is not where frozen weights lose residence (replay: on-food
  and `t_min` flat, dwell shorter). Whether a released band lets training
  find residence is open: one seed, aggregate columns up, held-out residence
  flat. Nothing on the cube changes.
- No skimmer depth rescues the lineage in the selected ecology without
  predators; where a lineage establishes, its diet has shifted onto the
  foliage channel the grazer also uses, and whether that is the grazer's cost
  was not isolated. The arm-2 ladder was then run (XY2): no rung is
  acceptable there either, and the shipped predicate halves R's arm-2 effect;
  R's own rows say the apex-arm treatment matters for `fast-leaf`.
- Sixteen pairs' sufficiency remains untested; the split-half spread is
  reported, not concluded on.
- The released turn band does not replicate as a training advantage; on the
  selected centres the difference between seeds is in the weights, not the
  adapter. Closed as a default and as the next training line under this
  protocol.
- Under the shipped predicate no skimmer depth is acceptable in either arm,
  and R's arm-2 effect was largely a property of the old predicate. Closed
  for this depth-only tuning path at arms 0 and 2; the genome generally and
  the world-level alternatives are not closed.
- A coupled-grazed opening at 96,000 ticks gives S's founder gains without
  S's first-hour browning, at near the status quo's starved-cell count, at
  about 15 s per core; it costs the skimmer founders at the shorter ages
  and founder broods everywhere, and its reading rule was pre-registered but
  has no clause for either cost. Whether the opening is worth its
  computation, and whether a visibly thinner reed bed at founding is what
  Wrysk wants to see, are his; nothing in §11 changes.
- Z could not use S's row shape and wrote its own recorder; the
  `Evaluation`-only measures are absent from its rows.

## Next recommendation (Fable, pending Astra)

1. ~~X, second training seed~~ **Done (XY2): not replicated; the deadband
   default is closed under this protocol.** *Next, bounded (Astra):* replay
   the seed-2 `cub-act-1` control and the prior selected `cub-act-1` centre
   on h1 and h6 (the two horizons it survived) and two pre-declared failed
   layouts, with the intake and body-budget trace; compare on-food fraction,
   dwell, served-to-credited intake, total bill and terminal reserve. A
   repeatable store-margin difference on the surviving layouts makes it a
   candidate for the whole-world population test; otherwise the lead ends.
   Two favourable layouts are not deployment evidence. Was: the retained command with `--adapter
   cub-act-2 --train-seed <second>` and its `cub-act-1` control at the same
   seed (a second control is owed too, 140 s), the seed being the replicate;
   plus the selected centres of both seeds replayed under both adapters as a
   weights × adapter 2 × 2 (expression, a different region, or
   co-adaptation). No training default from this: a favourable second seed
   still lacks whole-world evaluation while `es-population` refuses
   `cub-act-2`; the host stays on `cub-act-1` regardless.
2. ~~Y, the ladder at arm 2~~ **Done (XY2): no rung; the predicate halved R's effect; this depth-only path is closed at arms 0 and 2.** Was: same design, two apex adults
   introduced, under the shipped predicate with R's arm-2 rows reproduced
   under the half-space and the predicate contrast reported at 0.10 and
   0.55; an acceptable rung would be a candidate for held-out confirmation,
   not a roster decision from six training seeds.
3. **Z has landed.** *What we would tell Wrysk:* a coupled-grazed opening at
   96,000 ticks is the first §11 alternative that reproduces S's founder
   gains without its transient, and the frames show what it looks like; it
   is not proposed as a change, and if he wants it, it is a declared opening
   age on a fresh world, at about 15 s per core, with the skimmer and
   founder-brood costs stated. A held-out-seed confirmation at 96,000 against
   the adoption gate now registered at the end of Z's note (Astra's design;
   Fable's defaults for the two owner-choice clauses, stated) precedes any
   adoption.
4. The motor decision remains Wrysk's, on the terms in the round-4 result.
