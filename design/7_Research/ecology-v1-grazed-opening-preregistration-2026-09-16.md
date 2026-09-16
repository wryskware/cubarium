---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream Z — pre-registration of the coupled grazed opening comparison

**Committed before a single row exists.** This file states what will be run, what will be
measured, against which denominators, and the rule by which the answer will be read — so
that the reading cannot be chosen after the numbers are seen. It is committed on its own,
ahead of the operator, the tests and the campaign
([brief Z](../handoffs/ecology-v1-grazed-opening-opus-2026-09-16.md), deliverable 3).

Evidence, not a decision. **Nothing in this workstream changes §11,
`producer.initial_fraction` or any equation, and no `WorldConfig` field is added.** No
uniform total is proposed for §11. §7 of the result note will state what Wrysk would be
choosing and will not choose it.

## 1. The question

[S](ecology-v1-precondition-2026-09-16.md) §7 "Neither" says the thing that settles the
field is the founding, and that every arm converges to the same **grazed** standing crop of
ΣP ≈ 215–230 whatever it opened on — roughly half the ungrazed 390–450 a plant-only prefix
produces. So a plant-only prefix is long enough to reach the *plants'* state and therefore
long enough to reach the *wrong* state: it hands the founders a field that then falls 166
units in eight simulated minutes.

The opening this workstream measures is the one S's reading points at and did not run: a
field that an **ordinary coupled world** has already grazed into shape. Burn in with an
ordinary roster, remove that burn-in population, found the identical fresh roster into the
field it left behind, and measure the 180,000 ticks after that founding exactly as S
measured hers.

## 2. What will be run

Selected ecology `fast-leaf`; `baseline` is run beside it as the second configuration, as in
every ecology v1 campaign so far. Six training seeds. Arm 0 (`apex_founders: 0`), 180,000
ticks after founding, `sample_every` 600, ledger on, plant record on, depletion split on,
`pursuit_stop` at the shipped `reach-envelope`.

Three openings per configuration:

| opening | where it comes from | age |
| --- | --- | --- |
| **status quo** | S's retained age-0 rows, **reproduced** by this workstream's own runner | 0 |
| **plant-only** | S's retained rows, read as retained | 48,000 |
| **coupled-grazed** | this workstream | 48,000 / 96,000 / 180,000 |

The coupled-grazed ages are S's plant-only ages, so the two openings are compared **at the
same age** and "age" means the same number of simulated ticks in both.

Budget, declared now: 12 burn-in runs to 180,000 with three saved states each, 36 comparison
runs of 180,000 ticks, 8 workers, ≤ 8 wall minutes, `runs/ecology-v1-grazed-opening/` ≤
80 MiB.

## 3. The operator, declared before it exists

`World::remove_all_animals()` removes every organism and every neural entry, leaves the
field, water, weather, detritus, carrion and every pool untouched, and books the removed
bodies' material out of the world's net external-material counter — the exact mirror of what
`found_roster` books in — so that the conservation identity a burn-in-then-found world keeps
still reads ≤ 1e−9. It refuses, by name and without touching the world, when an extension
holds state keyed to the bodies it would remove (hunter members, quiet pauses, apex dormancy,
paired gestation or parentage), because this operator has no accounted policy for those and a
silent drop would corrupt them.

A **coupled** burn-in is not `evaluate::precondition` (which is plant-only and refuses a
populated world): it is the ordinary tick loop with the ordinary roster present, run in
`precondition.rs`.

## 4. The measures

Per (configuration, seed, age), on S's §4/§5 measures:

- **founders that bred, by kind** (`FounderBroods::parents_by_form` — distinct founders of
  that form that produced at least one birth) and **first brood tick by form**, relative to
  the founding;
- **founder lineages alive** at the horizon (live bodies whose parent chain roots at a
  founder), **broods completed**, **final population**, **final forms present**, **mean form
  evenness** over the run's samples;
- **crossings** from the same `CrossingCounter` every workstream reports, on the arm's own
  opening reference **and** on the common §11 reference (the age-0 arm's own tick-0 foliage,
  same configuration and seed);
- **terminal starved cells** on both references: cells ending below ¼ and below ½ of the
  reference;
- **the first simulated hour** after founding: ΣP, ΣW, ΣQ, ΣN, population, and the exact
  cumulative plant income, foliage in, foliage out and consumer withdrawal, sampled every
  600 ticks over 72,000 ticks;
- **the opening**: absolute ΣP at the founding instant, its per-cell median / p10 / p90 / CV,
  and the field's ΣW / ΣQ / ΣN;
- **the burn-in**: population and composition by form at removal, material removed, energy
  removed, and the coupled field's `state_hash` at the removal instant;
- **the standing crop the arm converges to**: mean ΣP over the late window (the last fifth of
  the declared horizon, S's window).

## 5. The denominators, stated before the numbers

S §7's cost (c) is that every quantity measured *relative to opening foliage* changes
denominator when the opening changes. So both forms are reported, always, side by side:

- **`late foliage ÷ opening foliage`** — the relative form. Its denominator is that arm's
  own opening ΣP, which differs by a factor of ~3.6 between the status quo and a plant-only
  opening, and is expected to differ again for a coupled-grazed one. A ratio is therefore
  **not** comparable across openings and is reported only so the declared `FOLIAGE_FLOOR`
  gate can be read.
- **absolute late foliage** (ΣP, units) — the comparable form. This is the quantity the
  reading rule in §6 uses.
- **terminal starved cells** are reported twice: on the arm's own reference (what the
  shipped counter counts) and on the **common §11 reference** taken from the age-0 arm of the
  same (configuration, seed). Only the common-reference column is compared across openings;
  S §4.2 is the reason.
- **"the grazed standing crop the arm converges to"** means the mean ΣP over the late
  window of that same arm, not a number carried over from S.

## 6. The reading rule, stated before the campaign runs

Per configuration, the coupled-grazed opening is:

- **the better target for a §11 change** if *all three* hold —
  1. S's 48,000 founder gains are reproduced: **every grazer founder breeds**
     (`parents_by_form[lantern] == 10` of 10, mean over seeds), **founder lineages alive at
     least 1.5×** the status quo of the same configuration, and **mean form evenness up** on
     the status quo;
  2. its **opening ΣP is within 20 %** of the grazed standing crop that arm converges to
     (|opening − late mean| ≤ 0.20 × late mean), so it does not brown over the first hour the
     way the plant-only opening does;
  3. its **terminal starved cells are within noise of the status quo** on the common §11
     reference, where "within noise" is: the paired per-seed difference against the age-0 arm
     of the same seed does not exceed the status quo's own across-seed standard deviation;
- **not** the better target if any of those three fails;
- **mixed** otherwise — which here means the rule is not applicable as stated because a
  clause could not be evaluated — and said plainly, naming which clause and why.

Which of the three it is will be reported per configuration, and the selected ecology is
`fast-leaf`.

## 7. Reproduction, before anything is interpreted

Nothing is read until the status-quo arm is shown to be the status quo. S's age-0 rows are
the target: this workstream's runner, run at age 0 with no burn-in, must reproduce them
`state_hash` for `state_hash` at the horizon (`final_state_hash` and `final_ecology_hash`).

S's rows were produced under **schema 16** and the `half-space` pursuit rule of the time;
arm 0 has **no predator**, so the schema-17 predicate change reaches nobody in these worlds
and the hashes are expected to carry across. If they do not, that is reported as a failed
reproduction and nothing downstream is interpreted.

## 8. A declared deviation from the brief's row shape

Brief Z asks for `--stage grazed` rows "with the same row shape" as `--stage compare` — that
is, `evaluate::Evaluation`. **That is not reachable from the files this workstream owns.**
`Evaluation` is produced by `evaluate::run`, which is private, builds its own world from a
config and a seed, and cannot be entered with a world that has already been burnt in and
re-founded; the only public preconditioning entry, `evaluate::precondition`, is plant-only
and refuses a populated world. `evaluate.rs` is owned by two other workstreams this round and
must not be touched.

So the grazed stage carries its **own** row type, `GrazedRow`, measuring the §4 list above
and nothing else. Where a measure already exists as a public, shared definition it is the
shared one and not a second implementation: `movement::FounderBroods`,
`movement::CrossingCounter`, `movement::CensusKey`, `metrics::guild_of`,
`evaluate::OpeningPoint`, `plant_budget::CellSample` and the core's own plant record. What is
new is only the loop that drives them.

Three things follow, and are declared here rather than discovered later:

1. The reproduction check in §7 is by `state_hash`, which is a property of the **world** and
   not of the recorder, so it tests the runner regardless of the row type.
2. Because the recorder is new, it is *also* checked against S's retained age-0 rows on every
   measure it shares with them — founders bred by form, first brood tick by form, founder
   lineages alive, mean form evenness, crossings, final population, final forms, final ΣP.
   Any disagreement is reported as a disagreement, not reconciled.
3. Measures that only `Evaluation` carries — the spatial windows, the census, the margins,
   the per-depleted-cell record's four-way reading — are **not** reported for the
   coupled-grazed arms, and no claim will be made about them.

## 9. What this comparison will not establish

Declared now, so it is not conceded later:

- Three ages, six training seeds, two configurations, one price, arm 0. Nothing about the
  held-out seeds, the other ladder prices, arms 1 and 2, or any other configuration.
- No age is an equilibrium and none will be called one.
- The burn-in population is an *ordinary* roster and its descendants; the field it leaves is
  the field *that* population grazed, not a general grazed field.
- Removing the burn-in population removes its bodies, and with them their future faeces,
  carcasses and respiration. What the fresh roster meets is the nutrient and litter history
  the burn-in *already* deposited, not an ongoing recycling regime — the same qualification
  S's §4.2 mechanism carries, read from the other side.
- The frames are single stills through the real presenter, one seed and one face. They are
  not a video and carry no animation-phase continuity.
- Nothing is measured about whether such a world is more or less interesting to watch over
  hours.
