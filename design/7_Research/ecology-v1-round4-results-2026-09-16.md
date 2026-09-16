---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Ecology v1, round 4: results of workstreams P, Q, R and S (2026-09-16)

Fable's consolidated result for the reconciled next steps of
[the round-3 result](ecology-v1-round3-results-2026-09-16.md), in the order
Astra set in [its round-3 review](ecology-v1-round3-review-2026-09-16.md).
Briefs: [P](../handoffs/ecology-v1-apex-predicate-opus-2026-09-16.md),
[Q](../handoffs/ecology-v1-es-antithetic-opus-2026-09-16.md),
[R](../handoffs/ecology-v1-depth-census-opus-2026-09-16.md),
[S](../handoffs/ecology-v1-precondition-opus-2026-09-16.md), committed at
`15e13c8`. Four Opus workers in parallel: Q on `main`, P, R and S in worktrees.
These are the measurements that precede three owner decisions (the apex
predicate, the skimmer's roster depth, a preconditioned opening), each of
which would be visible on the cube; none of this round touches the cube.

## Dispatch and budget

| stream | model | where | simulation used / cap | storage | tests after |
| --- | --- | --- | --- | --- | --- |
| Q antithetic ES analysis | Opus 5 high | `main` (`da2bdc8`…`a281162`) | 1.4 s / 5 min | 0.24 MiB / 20 | search 230, core 517 |
| P apex predicate pair | Opus 5 high | worktree, merged `36515fa` | 190 s / 4 min | 8.0 MiB / 20 | core 523, search 224 |
| R skimmer depth census | Opus 5 high | worktree, merged `c506af9` | 4.2 min / 6 min | 0.6 MiB / 30 | search 257 |
| S preconditioned opening | Opus 5 high | worktree, merged `0fb9754` | 4.5 min / 10 min | 33 MiB / 80 | core 530, search 270, host 596 |
| T inertial motor model (Wrysk's direction, added mid-round) | Opus 5 high | worktree, merged | 374 s / 11 min | 7.8 MiB / 40 | core 544, search 275, host 596 |

Fable's verification so far: Q's two ignored experiments re-run and the
retained files rewritten with identical generation-9 figures (span
6,459–8,915, sd 643, concordance 15 and 12 of 16, 2 masked); its integration
tests pass. P's four-seed reach-envelope arm re-run and compared field for
field with its retained rows (identical, timing keys excluded); its 13 new
tests pass on the branch and the suites after merge. R's census re-run for
one seed (both configurations, both depths, three arms) and matched by
`final_state_hash` on 12 of 12; S's field and compare stages re-run for one
seed at ages 0 and 48,000 and matched on 4 of 4. One semantic conflict between
R and S (R's test constructed the run options before S added its
`precondition` field) was repaired at integration (`1200075`); the shared
cache's stale-artifact race recurred in every worker and in Fable's own
re-runs, cleared each time by touching the core crate root. T's inertial apex arm
re-run and compared field for field with its retained rows (identical); T's
merge conflicted with S in the run options and the evaluation entry (both
sides kept: the motor model is set before any preconditioning), and five
initialisers written by R, S and T had to name each other's fields — repaired
at integration, suites green after.

## Q — where the ES search loses candidate variation: it does not; there is too little at source

Full note: [ecology-v1-es-antithetic-2026-09-16.md](ecology-v1-es-antithetic-2026-09-16.md).
Commits `da2bdc8` (definitions and 11 tests against `todo!()` bodies, red
first), `214df60` (the reduction, the step geometry, the deadband occupancy,
two integration tests that run a real generation through the trainer and
replay it), `a281162` (note). No change to the trainer's score, update,
sampler or protocol; no core change.

**Integrity checks first.** Replaying the sixteen recorded updates from the
initial centre reproduces the checkpoint with max |Δθ| = 0; every recorded
gradient norm is recomputed exactly; all 512 candidate-layout scores
re-derive from the episodes; Astra's generation-9 figures reproduce (span
6,459–8,915, sd 643, r = 0.81 with intake, r = 0.54 with opening residence).

| generation | candidate span | sd | centre | best | pairs concordant on intake per tick | masked by the minimum | retained ÷ orthogonal reference |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 6,478–7,997 | 398 | 6,521 | 7,997 | 11 / 16 | 6 | 1.003 |
| 3 | 6,377–7,864 | 459 | 7,852 | 7,864 | 14 / 16 | 5 | 1.000 |
| 7 | 6,456–8,681 | 577 | 8,227 | 8,681 | 16 / 16 | 0 | 1.003 |
| 9 | 6,459–8,915 | 643 | 8,703 | 8,915 | 15 / 16 | 2 | 1.014 |
| 12 | 6,400–10,276 | 834 | 7,596 | 10,276 | 11 / 16 | 5 | 0.992 |
| 15 | 6,473–9,045 | 757 | 7,110 | 9,045 | 9 / 16 | 9 | 0.999 |

Pooled over 255 informative pairs: the preferred member has more intake per
lived tick in 76 %, more opening residence in 62 %; 29 % of pairs are ranked
by a layout the estimator's mean would not choose.

**Verdict, as corrected after Astra's review:** the recorded update is
reproduced exactly and correlated-direction cancellation is not the loss
(the summed pair contribution retains 1.00× the orthogonal reference in every
generation; the centre sits in the top quartile of its own population for nine
generations; the population mean climbs 6,896 → 7,655). Candidates do feed and
reside more when they score more. What this does **not** establish: that the
estimator's variance, the sixteen-pair count, the centred-rank reduction or
the optimiser are adequate — none was tested (no split-half or bootstrap
gradient stability, no repeated seed, no alternative pair count). The
residence gap is about **18×** (generation 9's candidate range 0.8–5.5 % of
life against a route-follower's 97 %), not two orders of magnitude; "580
generations" is an extrapolated rate. The safe statement: **candidate
variation is real, useful residence is still small, the update is faithful,
and geometric cancellation is not where it is lost.**

**Deadband occupancy** (generation-9 centre ± σε, 768 samples, 12 layouts):
thrust is never inside its deadband; the turn head is clipped to exactly zero
on 100 % of ticks from a reset state (29 of 32 candidates) and on 42 % of the
carried trajectory (candidates 12–77 %), with the centre's mean |turn head|
0.060 against an edge of 0.050 — the channel straddles the clip. **Named next
change, not implemented, the score untouched:** the adapter's turn deadband
operating point — a measured clipping site and the leading adapter
hypothesis (carried deadband occupancy correlates only r = 0.21 with score;
no trajectory was run under another band), since residence requires stopping
and turning. Astra's bounded form: `TURN_DEADBAND` 0.05 → 0.0 only, with
`Aggregate::Min`, σ, pair count, score, layouts and seed fixed and a new
protocol hash; first a trajectory replay of the frozen centre and candidates
under both adapters (falsified as the bottleneck if turn activity rises
without on-food fraction, dwell or `t_min`); then a fixed-length
16-generation pair. The four-layout minimum's 29 % disagreement with the
mean is the price of worst-layout robustness, a design judgement, and is not
to be changed in the same run. The pair count and the rank reduction are
untested, not ruled out; a bootstrap or split of the retained pair
contributions is the cheap missing test.
Deliverable 3 (reconstructing the 32 candidates for H's exact on-food column)
was not needed: intake is material that left the stand through the mouth, so
it already requires on-food residence, and it separates the preferred member
in 76 % of pairs.

## P — the apex pursuit predicate, paired: confirmed, and the next term is the turn radius

Full note: [ecology-v1-apex-predicate-2026-09-16.md](ecology-v1-apex-predicate-2026-09-16.md).
Commits `d19150e` (the switch, tests, audit aggregations, CLI), `8d1639a`
(note). The switch is a `World`-level transient on the strike recorder (no
`WorldConfig` field, no new `World` field, since `lifecycle.rs` belonged to
another worker); `ContactMeasure::pursuit_holds(stop, …)` is the single place
either rule is written, so the record cannot transcribe a rule the world did
not run; `--pursuit-stop` refuses an unknown name rather than defaulting.
Tests first and red: before `step.rs` was touched, the behavioural tests
failed with "the burst was requested but not delivered: 0.0019 px/s against a
held cap of 0.21". The shipped arm reproduces K's and N's retained rows to
the attempt (449 paid, the same class histogram, 15 captures).

| | half-space (shipped) | reach envelope |
| --- | --- | --- |
| paid attempts (8 seeds, 32 lives) | 894 | 969 |
| **held at the burst's start** | **89.4 %** | **5.4 %** |
| gap change per burst | −1.07 px | −0.21 px |
| delivered burst: translation / turn sweep / whole motor | 2.7 / 1.7 / 4.4 px/s | 4.6 / **8.0** / 12.5 px/s |
| contacts | 88 | 140 |
| **captures per life** | **1.19** | **2.09** |
| usable energy earned ÷ whole bill (E's ratio; corrected after review) | 11.1 % | 18.5 % |
| lifetime mean / max | 12,440 / 16,585 | 13,164 / 23,201 |
| death cause | starvation 32 / 32 | starvation 32 / 32 |
| prey population at the end | 864 | 847 |

Captures by initial gap is the clearest row: under the shipped rule every
capture began inside 8 px and 186 attempts that began at 8–12 px produced no
contact; corrected, 4–8 px nearly doubles and 8–12 px becomes productive
(17 contacts, 9 captures); past 12 px nothing changes in either arm.

**Verdict by Astra's rule: confirmed as the mechanism; the capture gain is
an exploratory replication.** The held fraction falls sixteenfold, closure
improves, contacts rise 59 % and captures 76 %, lifetime rises 6 % and the
usable earned share of the bill goes from 11.1 to 18.5 %. **Disclosed
caveat:** the brief's four-seed pair ran first with contacts flat (46 against
46) and captures 15 → 21; P widened to eight seeds because sixteen lives
cannot separate a 10 % contact rate from 14 %, and that widening was not
pre-registered. What rescues it: the four added seeds are out of sample for
the decision to widen, and on them alone the effect is larger in every
direction (contacts 42 → 94, captures 23 → 46, the gap actually closes).

**Strike constants: not adequate once delivered, and raising them is not the
repair.** The delivered burst translates at 4.6 px/s against a nominal 16.7,
because 64 % of the boosted budget is turn sweep priced at
`motor::turn_radius_px` = |capture offset| + capture reach = 14.8 px; relative
closure is about 2 px/s, so the mean 11.4 px gap needs about 6 s, not 1.25.
The escape multiple is exonerated a second time on new evidence: the prey the
corrected hunter chases realises 2.6 px/s, a quarter of its cap. **Named next
term, untouched:** `motor::turn_radius_px` — the same arithmetic N found
consuming the resting envelope now consumes the boosted one; it is not
apex-specific, so an arm on it must be read against the whole world.

**What Wrysk would be approving:** the one-line change of the predicate to
`in_contact()`. Visible whenever an apex is spawned from the viewer: it
charges (0.3 → 4.6 px/s during a burst), stalks half as much, handles twice
as much, eats about twice as often, ends the prey population about 2 % lower,
one apex reached 97 % of its minimum reproduction age, and a new
`GraspUnmapped` outcome appears in 5 of 969 attempts where a charging body's
grasp lands off the surface at the open rim. It does **not** make the apex
viable: all 32 still starve (usable energy earned 18.5 % of the whole bill
by E's ratio) and readiness overlap stays zero.

## R — the skimmer at depth 0.55 in a reproducing world: refuted, a trade not an addition

Full note: [ecology-v1-depth-census-2026-09-16.md](ecology-v1-depth-census-2026-09-16.md).
Commits `6431156` (pre-registration before any row), `d08884e` (16
definition tests, 12 red against a stub), `080bee6` (the override, run loop,
driver, rule), `c38b5a6` (brood counter repair), `6b91316` (note). Search-only
(`census.rs`); no core file, no `WorldConfig` field. The override rewrites the
five roster skimmers' `depth` between `World::new` and the first step and
re-decodes their phenotypes; a control run at 0.10 reproduces the ordinary
harness world field for field at 2,000 ticks and, at full length, all 60
retained control rows by `final_state_hash` (A's screen, I's ladder, M's
present-off) and F's published variety census row for row.

| config | skimmer `depth` | kinds at end | grazer at horizon | glider | burrower | **skimmer** | seeds with any skimmer at the horizon |
| --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0.10 | 2.11 | 8.1 | 14.3 | 16.1 | 0.9 | 1 of 6 |
| baseline | **0.55** | 2.72 | **2.8** | 12.4 | 10.4 | **18.1** | **6 of 6** |
| `fast-leaf` | 0.10 | 3.00 | 21.3 | 27.9 | 12.7 | 0.0 | 0 of 6 |
| `fast-leaf` | **0.55** | 3.50 | **12.4** | 30.3 | 12.6 | **5.6** | **6 of 6** |

(18 runs per cell: 6 seeds × 3 apex arms; no world lost.) The skimmer's
ledger margin changes sign in both configurations; the grazer's falls in
both (to 0.35× and 0.58× its horizon population, margin rate to a fifth and a
forty-fifth); glider and burrower move 1–3 %. Water depth under a skimmer
falls 0.19 → 0.05 d and 0.25 → 0.08 d; the binary wet flag moves the wrong
way, as O warned. O's predicted diet drift appears: skimmer entrants in the
foliage bin 2 → 27 % and 0 → 14 %, and those bodies survive best in the world
(still an association; every one is a descendant).

**Verdict by Astra's rule: refuted in both configurations, on the variety
clause.** The lineage persists (baseline 16 of 18 worlds, `fast-leaf` 13 of
18 against 2 and 0 for the control) and no monoculture forms, but the grazer
pays: in baseline the loss is decisive on its face (0.35×) and thin
underneath (the baseline grazer reaches the horizon in only 2 of 6 control
seeds, so the loss is one world); in `fast-leaf` it is marginal on its face
(0.58× against a 0.60× line) and solid underneath (the grazer falls in 6 of 6
seeds and is gone from all three arms of one seed). R disclosed two defects in
its own pre-registration (a clause measuring a breeding founder's lifetime
where O's quantity was a sterile clone's; a per-seed agreement clause for a
lineage-founding rate that is concentrated in a few worlds) and reported both
as written. **The most consequential finding the brief did not anticipate:**
0 of 360 founder skimmers reach any horizon at either depth in either
configuration — the rescue is a lineage effect, not founder survival — and the
founder's own life moves in opposite directions (`fast-leaf` 609 → 1,352 s,
baseline 580 → 373 s), read as a 0.55 body paying 4.4× the motor bill for 3.5×
the range, which `fast-leaf`'s foliage funds and baseline's does not.

**What Wrysk would be approving, corrected:** not "skimmers that stop dying on
the rim" — they all still die. One genome value buys a fourth lineage that
persists, paid for out of the grazer: a trade of one kind for another, exactly
what the refutation clause was written to catch. And 0.55 is the grazer's own
`depth`, so "off the wet floor" and "onto the grazer's height" are the same
move here and nothing separates them. **R's recommendation, which Fable
accepts: do not take the roster change as tested.** Named next: a depth
ladder {0.10, 0.20, 0.30, 0.40, 0.55, 0.75} at arm 0, 60 runs, about 3.5
minutes — is there a depth that rescues the skimmer's lineage without taking
the grazer's horizon population? — plus splitting the ledger's margin bins by
generation and recording form-3 served material by channel.

## S — a plant-only preconditioned opening: no tested age removes the crossings; one age changes the founders' fate

Full note: [ecology-v1-precondition-2026-09-16.md](ecology-v1-precondition-2026-09-16.md);
frames [ecology-v1-precondition-2026-09-16.png](assets/ecology-v1-precondition-2026-09-16.png)
(four ages across, baseline and `fast-leaf` at founding and one hour later,
seed 1001, Front face, through the real presenter; the physical cube was not
inspected). Commits `5d3941c` (the door, the operator, the arms), `ea04ab3`
(note, frames, scripts). `World::found_roster` shares the constructor's
founder loop (two private helpers), so "the same 24 founders" is a fact;
because `state_hash` hashes the config, the caller restores the roster to the
config after the plant-only prefix, and the door refuses a config that
declares no roster (a cleared-and-not-restored config would otherwise have
produced a silent zero-founder arm reported as 24). Tests first: 7 core, 8
integration, 5 unit. **Reproduction:** 12 of 12 age-0 rows founded *through
the door* carry M's present-arm `final_state_hash` and ecology hash; 48 of 48
arms' founding hashes match the field stage's independent founding; every
saved state decodes to its hash; the operator reproduces M's herbivore-absent
crossings seed for seed and M's opening distributions. Two changes a reviewer
should look at first, disclosed by S: the recorder's windows, horizon, late
window and collapse tick are now relative to a recording origin (unchanged at
origin 0, checked by the 12 of 12 reproduction and an equality of metrics
through and around the door), and `Evaluation` gained two optional fields.

| config | plant-only age | opening ΣP | CV | crossings (with / without withdrawal) | grazer founders that bred, of 10 | founder lineages alive | population | forms | cells below ¼ of §11 seeding at the horizon |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 0 (status quo) | 107 | 0.39 | 68 (0 / 68) | **0.3** | 4.7 | 40 | 2.3 | 11.2 |
| baseline | 48,000 | 358 | 0.55 | 77 (2 / 75) | **10 of 10** | 11.0 | 46 | 3.0 | 13.0 |
| baseline | 96,000 | 394 | 0.49 | 169 (3 / 166) | 10 of 10 | | 49 | 3.0 | 29.0 |
| baseline | 180,000 | 390 | 0.53 | 582 (122 / 460) | 10 of 10 | | 47 | 3.2 | 82.7 |
| `fast-leaf` | 0 | 107 | 0.39 | 5 (0 / 5) | 4.8 | | 62 | 3.0 | 0.8 |
| `fast-leaf` | 48,000 | 401 | 0.51 | 17 (2 / 15) | 10 of 10 | | 58 | 3.2 | 2.5 |
| `fast-leaf` | 96,000 | 439 | 0.43 | 50 (5 / 45) | 10 of 10 | | 61 | 3.0 | 8.0 |
| `fast-leaf` | 180,000 | 449 | 0.45 | 287 (93 / 194) | 10 of 10 | | 61 | 3.3 | 34.5 |

**Verdict: no tested age makes the ungrazed crossings vanish**; they rise
monotonically with age on the arm's own reference and on a common §11
reference (S added the second reading because the counter's reference moves
with the opening). The mechanism, as measured: animal presence has a net
preserving effect on the over-seeded cells, consistent with recycling
(removing the animals also removed grazing, carcasses, movement and their
history); a long plant-only prefix runs the world in the one regime that
kills those cells and hands the founders the corpses (at age 180,000, 63.5 of
the 82.7 starved cells were stripped before founding). The field settles in total
by 96,000 (ΔΣP 0.4 % per window) but 811–883 of 1,110 watched cells still
move more than 1 % per window at every age: it is an age, not an
equilibrium, and the note says so.

**But the founders' fate changes completely, and one age gets both.** At
48,000 the field is 3.3–3.7× greener with essentially nothing starved yet and
the terminal starved count within noise of the status quo; every one of the
10 grazer founders breeds instead of 0.3 of 10 (baseline) — the benefit of a
much greener, still-transient coupled field (wood, reserve and nutrient
evolved too), not of a stationary opening; the first grazer
brood arrives at the 3,001-tick floor instead of 21,000; founder lineages
alive 4.7 → 11.0; form evenness 0.44 → 0.70; forms 2.3 → 3.0; and the one
baseline seed whose herbivore guild collapses at the status quo (failing the
`guilds_intact` gate) does not collapse. Paired per seed: 6 of 6 better on
lineages, broods and evenness; 0–1 of 6 on crossings. The split falls exactly
along diet: the foliage feeders (grazer 0.85, glider 0.90) change, the
burrower (0.10) and the skimmer (0.60, on the floor) do not, in every arm.

**The three options, measured, and what Wrysk would be choosing:** leave §11
(a thin uniform opening that greens; 11 cells starved; grazers barely breed;
one seed in six loses its herbivores); adopt preconditioning at 48,000 ticks
(a green, heterogeneous opening that visibly browns over the first eight
simulated minutes; every founder breeds; 13 cells starved; 40 simulated
minutes before a fresh world can be shown; every gate measured against
opening foliage changes denominator — late ÷ opening 2.31 → 0.62 — and
`fast-leaf` at 180,000 would fall under the 0.5 foliage floor); or neither,
because every arm converges to the same *grazed* standing crop (ΣP 215–230)
whatever it opened on — the status quo climbs 107 → 233 while the
preconditioned field falls 358 → 192 in eight minutes and returns to 217 —
so the opening transient is inverted, not removed, and larger in absolute
foliage. On this evidence the honest object of a §11 change is the grazed
coupled state, not the ungrazed field — and, per Astra, not a uniform total
either: ΣP 215–230 is a grazed total whose per-cell field is what produced
it. S chose none; §11 and `producer.initial_fraction` are untouched.

## T — the inertial motor model, paired against the shipped sweep (Wrysk's direction)

Full note: [ecology-v1-motor-inertial-2026-09-16.md](ecology-v1-motor-inertial-2026-09-16.md).
Commits `0629ac8` (the switch, the model, the provenance, 19 tests written
first), `6b90a2c` (note). Brief
[T](../handoffs/ecology-v1-motor-inertial-opus-2026-09-16.md), written on
Wrysk's direction that movement cost follow rough physics — mass and
momentum, bodies as balls or cylinders, turning cheaper than moving, no
modelling of outstretched claws — after P found the apex spending two thirds
of its lunge on turn sweep at its claw-reach radius. The model: every body a
uniform disc of radius its own lobe extent, rotation as the energy-equivalent
speed `r·ω/√2`, envelope `√(v² + v_rot²) ≤ cap`, bill `move_cost·S·(|v| +
v_rot)`, the apex's grasp contact geometry only. `MotorModel::{Sweep,
Inertial}` is a `World` transient (no `WorldConfig` field); `Sweep` is
byte-identical to the shipped contract (six state hashes over 9,000 ticks of a
two-apex, eight-neural world, captured before any code was written); the
motor model is recorded in the ES protocol and policy file and a mismatch is
refused by name, with a missing field reading as `sweep` (exactly one contract
existed when those files were written). Two design calls T made and argued:
one radius everywhere (the observation, the envelope, the bill and the neural
adapter all read the same number, which under `Sweep` had silently disagreed
for the apex — told 9 px, bounded by 14.8), and two constraints under
`Inertial` (capability bounds the quadrature magnitude, the purse bounds the
billed sum) so energy-bound pure translation is identical under both models.
A geometry correction to the brief: the apex's lobes are 9 px, so its
rotation radius falls 2.3×, not "far less"; per radian it pays 0.86 of the
shipped cost while an ordinary 2.5 px body pays 1.41×; the apex's gain is
turning room, not a discount. Two files outside T's list, one line each,
forced (the transient's default in `World::assemble`; the option on
`RunOptions`), plus `Protocol` in `trainer.rs` and the host test's constructor.

**Arm A, the apex** (P's eight-seed design, predicate corrected, both models;
the `sweep` arm reproduces P's rows row for row):

| | sweep | inertial |
| --- | --- | --- |
| delivered burst: translation / rotation / whole motor | 4.6 / 8.0 / 12.5 px/s | **8.6 / 15.6 / 24.0** |
| gap change per burst, whole arm | +0.21 px (grows) | **−0.57 px (closes)** |
| contacts | 140 | **207** |
| captures per life | 2.09 | **2.91** |
| attempts beginning past 12 px: contacts / captures | 0 / 0 of 355 | **21 / 12** of 449 |
| usable energy earned ÷ whole bill (E's ratio; corrected after review) | 18.5 % | **27.0 %** |
| lifetime max | 23,201 | **46,449** |
| death cause | starvation 32 / 32 | starvation 32 / 32 |

The burst closes the gap for the first time; attempts past 12 px produce
contacts where they never could; two members pass the reproduction age
threshold (46,449 ticks against 24,000) and the first refusing readiness term
becomes the reserve stock fraction — which is not readiness or viability.
Still no mating. Caveat T could not remove and Astra weighs as material: the
switch changes every body from tick 0, so the apex is dropped into a 16 %
thinner prey stock and the attribution among the disc envelope, the omitted
grasp and the changed prey dynamics is not separated; the direction is
compelling, the magnitude is not isolated.

**Arm B, the whole world** (A's screen rows, both configurations, six seeds,
three arms; all 36 `sweep` rows reproduce the retained hashes, with the
ledger on, so the ledger's inertness is measured again): the legacy
controller turns more — motor billed +50 %, range +14 % (cells per body per
window +58 in `fast-leaf`), feeding fraction +3 points, on 18 of 18 rows in
`fast-leaf`; population −5.6 % (`fast-leaf` arm 0: 60.8 → 56.0, seed ranges
not overlapping); foliage, litter and living cells within 1.5 %; every one of
the six gates kept in every arm under both models. Against the r0a memory:
the 93 → 39 collapse belonged to the old union-of-ceilings envelope where
sweep could be thirteen times travel; the shared budget removed that lever
and `Inertial` does not bring it back. Worth watching: baseline's scavenger
guild all but vanishes in arm 0 (2.2 → 0.2) on an ecology that already fails
the guild gate under both contracts, with a large seed spread.

**Verdict:** (a) decisively yes on the apex; (b) partly on the world (four
motor and behaviour measures move beyond seed noise, the rest within 1.5 %);
(c) every gate kept. **What Wrysk would be adopting:** the rotation term and
envelope for every body — on the cube, bodies that turn √2 faster and hold
their travel through a turn (curvier paths, more ground per window, more time
feeding), an apex that pivots 2.3× faster and can actually charge; a slightly
leaner `fast-leaf`; a new protocol hash with every trained policy refused by
name and needing retraining (the single largest cost); and these retained
rows needing re-measurement under the new contract: A's screen and held-out,
I's ladder, F's movement rows and P's predicate rows (the plant-budget,
factorial, depth and census rows only where they quote a motor or range
measure). T's own recommendation, recorded: the apex evidence is strong but
the retraining bill is not worth paying for the apex alone; the cheap
intermediate — drop the grasp from `turn_radius_px` under `Sweep` alone,
changing no ordinary body and no protocol — would capture most of arm A's
gain. Fable's view differs and is stated in the recommendation below: Wrysk
asked for the physics model, no trained policy is installed anywhere and the
one that exists cannot forage, so the retraining cost is currently zero in
practice, and the world-level change is small and gate-clean.

## U — the grasp-only apex turn radius under the shipped motor: refuted as the principal defect

[Note](ecology-v1-apex-grasp-2026-09-16.md) ·
[brief](../handoffs/ecology-v1-apex-grasp-opus-2026-09-16.md) · merged at
`8a8e1be`. Item 1 of the recommendation below, run after Astra's clearance:
under `Sweep`, P's corrected predicate, eight held-out seeds and both
configurations, a paired arm whose apex `turn_radius_px` is its 9 px lobe
extent rather than its 14.8 px grasp (`ApexTurnRadius::{Grasp, Lobes}`, a
`World` transient beside the motor model and the pursuit stop; `apex-audit
--apex-turn-radius`). Default off is byte-identical against T's six pinned
hashes, run green before the implementation; the off arm reproduces P's and
T's retained rows field for field; Fable's re-run of the on arm reproduces
U's file field for field (only per-row timings differ). 13 new tests; 554 core
and 278 search pass.

**The prey world is identical between the arms, row for row** (745 at
introduction in both), which T's arm A could not say (628 against 745).
Against that clean pair:

| | `sweep` | `grasp-only` | `inertial` (T, retained) |
| --- | ---: | ---: | ---: |
| gap change per burst (+ = grew) | +0.212 px | −0.031 px | −0.574 px |
| contacts | 140 | 165 | 207 |
| captures per life | 2.09 | 2.44 | 2.91 |
| delivered translation / rotation (px/s) | 4.57 / 7.90 | 5.62 / 10.47 | 8.55 / 15.45 |
| E's usable-energy ratio | 18.5 % | 25.8 % | 27.0 % |
| lives past the 24,000-tick age gate | 0 / 32 | 4 / 32 | 3 / 32 |
| death cause | starvation 32/32 | starvation 32/32 | starvation 32/32 |

**Refuted by Astra's rule.** The switch recovers 31 % of `Inertial`'s closure
and 42 % of its captures pooled, and that third rests on one run
(`fast-leaf/9006`: 9 → 27 captures alone); without it pooled captures fall
below the shipped arm and the gap still grows. Paired sign tests over the 16
runs do not separate either hunting limb from run-to-run variation (gap 9/16,
captures 9/14). Two limbs are robust and they are the member's own budget,
not its luck: it turns more and pays less for turning (translation billed up
in 13/16, the only limb at the nominal 0.05 line; turn billed down in 11/16),
and E's ratio rises in 12/16 (p = 0.077). **The one behavioural change
replicated beyond the influential run is the age gate:** no life reached
24,000 ticks with the grasp counted; four do without it, in three runs, two of
them the runs `Inertial` crossed in; thirteen paired worlds tie, so this is
descriptive, and it is about those four lives. T had credited that to the disc
model; the grasp correction alone produces it in the identical prey world.

**What this supports:** the geometry correction alone does not reproduce most
of the *observed, confounded* T arm. It does not say the rest belongs to the
disc envelope: T's thinner, faster-turning prey world could have helped or hurt
contact opportunity, so the third recovered is not a bound in either direction
(Astra, addendum). An `Inertial` arm in which only the apex runs the disc model,
with its complement read as a 2 × 2, is the named next experiment (W). The
"correctness fix" reading is withdrawn: the 9 px readers serve neural animals
only and no apex consumes them, so `Lobes` is an experimental alternative to
`Sweep`'s outermost-contacting-point rule, byte-identical for every non-apex
body, and it is the setting under which the four age-gate crossings happened.
U also found T's
gap-bin table printing 221/0/0 for `sweep` at 12–16 px where the rows say
221/4/0; T's sentence about captures stands.

**Fable's decision, confirmed by Astra's addendum:** hold the `Lobes` default,
whether or not `Sweep` remains the contract. The motor question now turns on
W's 2 × 2 (apex motor × ordinary-body motor, both controls byte-reproduced
under one build, pre-introduction state hash equal row for row), which is
cheap and runs before any host contract or recalibration is paid for.

## What this does and does not establish

- **Established by measurement:** the apex's pursuit stopping predicate
  suppresses its paid burst (held 89 → 5 %; the capture gain is an
  exploratory replication; the usable earned share of the bill rises 11.1 →
  18.5 %), and once it charges the next term is the turn radius its grasp
  sets; the ES update is reproduced exactly and correlated-direction
  cancellation is not where variation is lost, candidate variation is real
  and useful residence is still small (about 18× below a route-follower),
  and the adapter's turn deadband is a measured clipping site and the leading
  adapter hypothesis; the skimmer's depth rescues its lineage and pays for it
  out of the grazer, in both configurations; no *tested* plant-only age
  removes the ungrazed depletion crossings, one age (48,000 ticks) makes every
  grazer founder breed in a much greener, still-transient coupled field, and
  every arm converges to the same grazed coupled state whatever it opened on.
- **Established by T:** under the inertial model the corrected apex closes
  the gap and reaches prey it never could, two members pass the reproduction
  age threshold, and all 32 still starve at 27 % of their bill by E's ratio;
  the world absorbs the model with every gate kept and a 6 % leaner
  `fast-leaf`; the apex arm is not isolated from the world change.
- **Not established:** the estimator's variance, the pair count and the rank
  reduction (untested, not ruled out); whether the apex can ever fund itself
  (the reserve stock fraction is now the first refusing term); whether the
  turn deadband change lets a policy express residence (a trajectory replay
  and a fresh campaign under a new protocol, not run); whether some skimmer
  depth between 0.10 and 0.55 rescues the lineage without the grazer's loss
  (a ladder, not run); what a coupled grazed opening would do (S's next
  measurement, not a uniform total).
- **The cube is untouched by this round**: build `77c42e8`, `fast-leaf`,
  shipped price, shipped predicate, shipped motor, shipped roster, shipped
  §11, shoulder 0.95 by override.

## Review and repair (Astra, 2026-09-16)

Astra's review is
[ecology-v1-round4-review-2026-09-16.md](ecology-v1-round4-review-2026-09-16.md).
Disposition: retain P, R, S and T as evidence; do not deploy predicate and
motor together from this package. Repair 1, report-level:

- **P1, Q.** Exact replay and the orthogonal reference prove faithful
  execution and no geometric cancellation; the estimator, pair count and
  rank reduction are untested, not ruled out; the residence gap is about 18×;
  the deadband is a measured clipping site and the leading hypothesis; the
  four-layout minimum's masking is a design judgement. Corrected in Q and
  here.
- **P1, deployment.** Neither switch has a production contract (no host
  caller of the pursuit rule; a resumed world returns to `Sweep`; the host's
  neural seeding does not call `check_motor`), and T's own note requires the
  fifteen-candidate screen and held-out selection rerun before `fast-leaf`
  can still be called the selected ecology under `Inertial`. Fable's
  deploy-together recommendation is withdrawn; the order below is Astra's.
- **P2, ledger units.** P's and T's "fraction of the bill earned" were a
  material ratio and a mixed-unit sum; E's usable-energy ratio over the
  whole bill (upkeep + motor + strike and handling) is 11.1 → 18.5 %
  (predicate) and 18.5 → 27.0 % (motor), verified by Fable from the retained
  records. Corrected in both notes and the tables here.
- **P2, P's widening** read as exploratory replication with the four added
  seeds as an independent cohort; **P2, S's wording** qualified ("no tested
  age", "net preserving effect consistent with recycling", not "only
  foliage", a grazed total is not an initialiser); **P2, T's arm A** not
  isolated from the world change.
- **P3, R and integration:** sound; no change.

## Next recommendation (reconciled with Astra)

Astra's order, which Fable accepts. Three items are Wrysk's, and what we
would tell him is stated at each.

1. **The grasp-only apex pair — the single most informative cheap
   experiment now.** Under the shipped `Sweep` motor, P's corrected predicate,
   eight held-out seeds and both configurations: a variant whose apex
   `turn_radius_px` uses its lobe extent rather than its grasp reach, every
   ordinary body bit-identical and the prey world unchanged. Record prey at
   introduction, delivered translation and rotation, signed gap closure,
   contacts and captures by initial gap, E's usable-energy ratio, lifetime,
   readiness terms. Confirmation that geometry is the principal apex motor
   defect: it recovers most of `Inertial`'s −0.57 px closure and 2.9 captures
   per life in the identical prey world; refutation: it stays near +0.21 px
   and 2.1. About one minute of simulation; dispatched as workstream U.
   **Result (U, above): refuted.** A third of the observed, confounded T
   arm, resting on one run; the age gate is the one change replicated beyond
   that run.
2. **Adopt the reach-envelope predicate separately.** *What we would tell
   Wrysk: yes.* The half-space contradicts its own reach meaning and
   suppresses 89 % of paid bursts; the added seeds replicate the direction.
   Adoption means making reach-envelope the production rule with a
   production default and a resume regression test, nothing else changed
   (escape, strike duration, sense and mating radius untouched). Visible: an
   introduced apex charges and handles prey more often; it still starves and
   is not a lineage.
3. **The motor after isolation, contract and recalibration.** *What we would
   tell Wrysk: the inertial model is a faithful implementation of the
   approximation you asked for, and it should not go on the cube from this
   evidence yet.* If the grasp-only pair captures most of the benefit, take
   that geometry correction and keep `Sweep`. It did not (U). The disc model
   remains the physics Wrysk asked for, so the next step is the isolation U
   names, as a full 2 × 2 (W): the disc model on the apex only in the shipped
   prey world, its complement, and both controls under one build, read for
   the apex effect, the ordinary-body effect and their interaction; an
   apex-only contrast, not an apportionment, until all four cells exist.
   Only after that: a host production default, resume
   and status rule; `check_motor` in the host's neural seeding; an action
   adapter that can request the quadratic envelope's diagonal; the
   fifteen-candidate screen and held-out selection rerun; then one fresh
   world. Never a migration. Acceleration cost waits until the memoryless
   contract is chosen.
4. **Training: the turn deadband alone.** *What we would tell Wrysk: no score
   change.* `TURN_DEADBAND` 0.05 → 0.0 under a new protocol with
   `Aggregate::Min`, σ, pair count, layouts and score fixed; first trajectory
   replays of the frozen centre and candidates under both adapters, then a
   bounded 16-generation pair only if turn release raises on-food residence
   or `t_min`. Bootstrap or split Q's retained pair contributions for
   gradient-direction stability before declaring sixteen pairs sufficient.
   Not visible on the cube.
5. **The skimmer depth ladder** {0.10, 0.20, 0.30, 0.40, 0.55, 0.75}, arm 0,
   with founder and descendant ledger bins and served foliage and litter per
   body, after the apex decision. *What we would tell Wrysk: no roster change
   at 0.55.* A depth is acceptable only if a lineage persists across seeds
   without materially reducing the grazer; if none exists, the choice
   becomes three viable heights and four kinds, or a wet-floor producer,
   which is a new food web and display layer and not a repair.
6. **§11: leave it; measure a coupled opening.** *What we would tell Wrysk:
   no §11 or preconditioning change now.* Compare conservation-accounted
   snapshots of the full coupled field after an ordinary roster has produced
   the grazed state (burn-in population removed, identical fresh roster
   founded) against status quo and the 48,000 plant-only opening, with
   opening and one-hour frames, exact plant budgets and founder broods. No
   uniform total written into §11. The current thin-green opening is the
   honest default meanwhile.
7. **Record repairs** — done in this repair.

