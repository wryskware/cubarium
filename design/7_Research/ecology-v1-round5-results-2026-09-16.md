---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Ecology v1, round 5: the turn deadband, the depth ladder, the coupled grazed opening

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
| Z | [grazed opening](../handoffs/ecology-v1-grazed-opening-opus-2026-09-16.md) | Opus 5, high | — | — | in flight |

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
significantly, in the negative direction. Q's prediction about the
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
move. That is a *learnability* hypothesis, not an *expression* one, and
nothing here tests it. A second training seed (140 s) is the cheapest thing
that separates "this adapter is better" from "this seed was luckier".

**What changed on the cube: nothing.** The host names no adapter and runs
`cub-act-1`, byte for byte. A `cub-act-2` policy is refused by name at
attachment, at founding and by `PolicyFile::policy`. **Fable added one more
refusal at integration (`997fb24`):** X had widened the decode-time check to
accept any adapter this build knows, so a snapshot holding a `cub-act-2`
policy would have resumed silently under the shipped adapter (the adapter is
a transient the bytes cannot carry). `World::from_state` now refuses such a
snapshot by name, with a test; a shipped-adapter world round-trips as before.

## Y — the skimmer depth ladder, arm 0: no rung is acceptable; the rescue is one move off the litter onto the grazer's leaf

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

**What the two added measures settled.** Material taken per `fast-leaf`
skimmer descendant across 0.10 → 0.75: litter 2.60 → 0.02 m, foliage 1.64 →
26.3 m (the grazer's own descendants take 22–26). The swap is monotone from
the first rung, and the descendant margin turns positive exactly where it
happens: "off the wet floor" and "onto the grazer's leaf" are one move
across the whole ladder, which was the confound R could not separate. The
founder's margin rate falls with height in both configurations while the
descendant's rises, so the rescue is a descendant phenomenon everywhere; R's
founder-lifetime reversal is the founder's income, not a sign change in its
economics (a one-sentence correction to R, recorded in Y).

**Found unasked, and it bears on R's reading:** re-reading R's own retained
rows by arm, R's `fast-leaf` result is largely an apex-arm property. At
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

## Z — the coupled grazed opening

In flight. Brief: `World::remove_all_animals` with its material booked out
exactly; `precondition --stage grazed` (ordinary coupled burn-in to 48,000 /
96,000 / 180,000, remove the population, found the identical roster, S's
180,000-tick comparison); three openings compared (status quo, plant-only
48,000, coupled-grazed) with absolute foliage and the standing crop beside
S's measures; frames at founding and one hour; a pre-registered reading rule;
no §11 change proposed.

## What this does and does not establish (X and Y; Z pending)

- The turn band is not where frozen weights lose residence (replay: on-food
  and `t_min` flat, dwell shorter). Whether a released band lets training
  find residence is open: one seed, aggregate columns up, held-out residence
  flat. Nothing on the cube changes.
- No skimmer depth rescues the lineage in the selected ecology without
  predators; the mechanism of the rescue where it occurs is the skimmer
  taking the grazer's foliage. The arm-2 ladder is untested and R's own rows
  say the apex is load-bearing for `fast-leaf`.
- Sixteen pairs' sufficiency remains untested; the split-half spread is
  reported, not concluded on.

## Next recommendation (Fable, pending Astra)

1. **X, second training seed** (140 s): the retained command with `--adapter
   cub-act-2 --train-seed <second>` and its `cub-act-1` control at the same
   seed (a second control is owed too, 140 s), to separate adapter from
   seed. Only then decide whether `cub-act-2` becomes the training default;
   the host stays on `cub-act-1` regardless until a policy is selected and
   the host learns to name an adapter.
2. **Y, the ladder at arm 2** (4 min): same design, two apex adults
   introduced, because R's rows say that is where the effect lives.
3. **Z** lands and is read by its rule.
4. The motor decision remains Wrysk's, on the terms in the round-4 result.
