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
| T inertial motor model (Wrysk's direction, added mid-round) | Opus 5 high | worktree | *(pending)* | | |

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
re-runs, cleared each time by touching the core crate root.

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

**Verdict by Astra's rule: refuted as an optimiser or update problem.**
Nothing cancels (the summed pair contribution retains 1.00× the orthogonal
reference in every generation; opposed pairs are not a thing in 10,215
dimensions with 16 draws) and nothing is erased (the replay is exact; the
centre sits in the top quartile of its own population for nine consecutive
generations; the population mean climbs 6,896 → 7,655). But the refutation
branch's premise also fails: candidates plainly do feed and reside more when
they score more. The honest statement is neither branch verbatim: **the
reduction and the update are faithful, and the residence variation they have
to work with is real and about two orders of magnitude too small** — the
whole candidate range of opening residence in generation 9 is 0.8–5.5 % of
life against a route-follower's 97 %, and the search gains about 50 ticks per
generation against a 29,000-tick gap. Attention moves, as the refutation
branch directs, to parameterisation and the adapter.

**Deadband occupancy** (generation-9 centre ± σε, 768 samples, 12 layouts):
thrust is never inside its deadband; the turn head is clipped to exactly zero
on 100 % of ticks from a reset state (29 of 32 candidates) and on 42 % of the
carried trajectory (candidates 12–77 %), with the centre's mean |turn head|
0.060 against an edge of 0.050 — the channel straddles the clip. **Named next
change, not implemented, the score untouched:** the adapter's turn deadband
operating point — residence requires stopping and turning, and this is the
one measured place where weights → behaviour throws away most of what the
search puts in; it changes what a policy can express, so it is a fresh run
under a new protocol hash, never a migration. Ranked behind it: the
four-layout minimum as the aggregation (it inverts the estimator's preference
on 29 % of pairs; `Aggregate::Mean` exists and hashes as a different task, so
a paired A/B costs no code; the minimum is what forces generality, and that
risk is stated), then σ after the adapter so the two are not confounded. The
rank reduction and the pair count are ruled out by measurement, not deferred.
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
| fraction of its own bill earned | 7.7 % | 12.9 % |
| lifetime mean / max | 12,440 / 16,585 | 13,164 / 23,201 |
| death cause | starvation 32 / 32 | starvation 32 / 32 |
| prey population at the end | 864 | 847 |

Captures by initial gap is the clearest row: under the shipped rule every
capture began inside 8 px and 186 attempts that began at 8–12 px produced no
contact; corrected, 4–8 px nearly doubles and 8–12 px becomes productive
(17 contacts, 9 captures); past 12 px nothing changes in either arm.

**Verdict by Astra's rule: confirmed.** The held fraction falls sixteenfold,
closure improves, contacts rise 59 % and captures 76 %, lifetime rises 6 %
and the earned fraction of the bill goes from 7.7 to 12.9 %. **Disclosed
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
viable: all 32 still starve at 12.9 % of their bill and readiness overlap
stays zero.

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

## S — a plant-only preconditioned opening: no age removes the crossings; one age changes the founders' fate

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

**Verdict: no age makes the ungrazed crossings vanish**; they rise
monotonically with age on the arm's own reference and on a common §11
reference (S added the second reading because the counter's reference moves
with the opening). The mechanism is M's own caveat read from the other side:
with animals present the over-seeded cells are kept alive by the animals'
recycling; a long plant-only prefix runs the world in the one regime that
kills them and hands the founders the corpses (at age 180,000, 63.5 of the
82.7 starved cells were stripped before founding). The field settles in total
by 96,000 (ΔΣP 0.4 % per window) but 811–883 of 1,110 watched cells still
move more than 1 % per window at every age: it is an age, not an
equilibrium, and the note says so.

**But the founders' fate changes completely, and one age gets both.** At
48,000 the field is 3.3–3.7× greener with essentially nothing starved yet and
the terminal starved count within noise of the status quo; every one of the
10 grazer founders breeds instead of 0.3 of 10 (baseline); the first grazer
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
foliage. On this evidence the honest target for a §11 change is the grazed
standing crop, not the ungrazed one, which is a different measurement. S
chose none; §11 and `producer.initial_fraction` are untouched.

## What this does and does not establish

- **Established by measurement:** the apex's pursuit stopping predicate is
  the cause of its near-zero strike motion (held 89 → 5 %, captures +76 %,
  ledger not worse), and once it charges the next term is the turn radius
  its grasp sets; the ES optimiser and update are faithful and the residence
  variation they see is real and two orders of magnitude too small, with the
  adapter's turn deadband the measured place weights lose their effect; the
  skimmer's depth rescues its lineage and pays for it out of the grazer, in
  both configurations; no plant-only age removes the ungrazed depletion
  crossings, one age (48,000 ticks) makes every grazer founder breed, and
  every arm converges to the same grazed standing crop whatever it opened on.
- **Not established:** whether a delivered lunge can pay for the apex once
  the turn radius is fixed (T, pending); whether a wider turn deadband lets a
  policy express residence (a fresh campaign under a new protocol, not run);
  whether some skimmer depth between 0.10 and 0.55 rescues the lineage without
  the grazer's loss (a ladder, not run); what a §11 seeded at the grazed
  standing crop would do (a different measurement from S's).
- **The cube is untouched by this round**: build `77c42e8`, `fast-leaf`,
  shipped price, shipped predicate, shipped motor, shipped roster, shipped
  §11, shoulder 0.95 by override.

## Next recommendation (Fable's, before Astra's opinion)

The round turns three of the pending owner decisions into concrete
proposals and withdraws one.

1. **Apex: approve the predicate correction** (`inside` → `in_contact()`,
   one line). Astra's condition for telling Wrysk to approve is met: the
   burst is delivered, the gap closes, contacts and captures rise, the ledger
   is not worse. It does not make the apex viable on its own; T's motor
   result decides whether the corrected apex can then close. Recommend
   deploying the predicate together with T's motor model if T confirms,
   as one fresh world, so the cube's apex changes once rather than twice.
2. **Skimmer: no roster change now.** Run R's depth ladder {0.10, 0.20,
   0.30, 0.40, 0.55, 0.75} at arm 0 (about 3.5 minutes) to ask whether any
   depth rescues the lineage without the grazer's loss, and split the ledger's
   margin bins by generation so founder and descendant stop being read as one.
   The wet floor's missing producer stays a separate design question.
3. **Seeding: neither option now; change the measurement.** S shows the
   world converges to a grazed standing crop of ΣP 215–230 regardless of the
   opening, so the honest target for §11 is that, not the ungrazed field.
   Next: seed at the grazed standing crop (the equivalent `initial_fraction`,
   uniform, no operator) and at S's 48,000-tick preconditioned field, and
   compare founder outcomes, the opening transient's size and sign, and the
   ungrazed crossings, with frames. Wrysk decides after that; leaving §11 is
   the default meanwhile.
4. **Training: widen the adapter's turn deadband operating point** as a
   fresh, bounded campaign under a new protocol hash (never a migration),
   with the four-layout aggregation A/B (`Aggregate::Mean` exists) run beside
   it at the same seed; the score stays. Not visible on the cube.
5. **Motor** (T, pending): if the inertial model confirms on both arms and
   keeps every gate, it becomes the contract proposal to Wrysk, with the
   acceleration cost as the following step on the physics-engine backlog.
