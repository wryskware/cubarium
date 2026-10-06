---
design_status: exploration
last_reviewed: 2026-10-05
decision_refs: []
---

# The whole-web model

**Status:** plan, with the work under way. Not canon.

## Why

**Wrysk, 2026-10-05:** "every change youre making seems like a shot in the dark rather than
being considered in the greater context of things, purely reactive to one measurement, then
tested with 4h runs that often are invalidated by other bugs."

He named two routes and a hybrid:
1. a mathematical model of the world;
2. very stripped-down simulations (water frozen after settling, other parts frozen or
   approximated).

**Choice: route 1 first.** It is less work and far faster, if it is shown to be
representative.
- That test comes before any use (below).
- Route 2's frozen water is used inside route 1 already: moisture per zone is fixed from a
  settled world.
- A stripped-down sim comes in later only where the model and the sim disagree and the cause
  (space, behaviour) has to be isolated.

The case that started this: on patch, D2b's switching rule could not save the small prey,
because 3 lanternjaws killed about 11 in 2 h while their births were far fewer. That is one
line of arithmetic, and it should have come before any code.

## Form

A deterministic stock-and-flow model (mean field), in Rust, crate `crates/cubarium-webmodel`.
- **Animals:** each species is stage-structured: juveniles and adults (biomass and numbers),
  with maturation, κ-rule reproduction, and mortality by cause.
- **Plants:** for each species, the area occupied per habitat zone (colonisation, loss,
  displacement), and biomass per occupied area under the sim's own growth law.
- **Detritus:** pools of litter, carrion and film, controlled by their donors.
- **A stochastic twin for the animals,** run on integer counts. Populations here are 2–15
  bodies, and the mean field alone hides demographic extinction.

**Numbers.** Every rule-level number is read from the config the sim reads, through the sim's
own loaders. Only rates that emerge from behaviour are fitted from sim telemetry, each recorded
with the runs it came from: search and encounter, the share of time spent foraging, and
activity.

**The two halves meet at `exchange`:** `Supply` → `Take`, plus `Returns`.
- Each half is first validated **open loop**: the other half is clamped to the sim's recorded
  values.
- Then the halves are closed into one loop.
- A closed-loop failure then points at a half.

## Literature anchors

- Yodzis & Innes 1992; Brose, Williams & Martinez 2006 (the allometric trophic network):
  bioenergetic multi-species food webs, and functional responses with a Hill exponent.
- de Roos et al. 2008: the stage-structured biomass model. It takes juveniles and adults from
  an individual energy budget with a κ-rule, which matches the sim's brood earmark and its
  maturation bottleneck.
- Kooijman: Dynamic Energy Budget theory (the individual budget).
- Levins; Tilman 1994: competition for space through colonisation and displacement. This is
  the moss → grass → woody succession.
- Moore et al. 2004: detritus-based food webs and donor control.
- Grimm et al. 2005: pattern-oriented modelling, which checks an aggregate model against an
  individual-based one by several patterns at once.

## The usefulness test (validation set, already paid for)

| set | what it holds |
|---|---|
| soak v2 | Full terrarium, 3 seeds, m370 → 1570. Plant stocks, grass → woody succession, capgnawers. |
| D1b | Detritivore generation: arms main / gut / brood / rate, 8 seeds × 2 machines. |
| D2b | patch-hunters, 6 arms × 8 seeds, including 1 vs 3 hunters, 2 h and 6 h observation. |
| capfix | Glowcap cap regrowth and senescence arms. |
| maint | Springturf upkeep arms. |

**The bar has two parts:**
- Starting from a save's census, each main stock's model trajectory falls within the sim
  seeds' spread, or within a stated factor of it.
- Each arm-vs-arm difference in the set has the sign and rough size the sim showed.

## Outputs

- **The budget sheet**, per stock and per world day: production, each consumer's take, and
  losses. Per species: births, maturation, deaths by cause, and net change.
- **Equilibria:** where they sit, whether they are stable, and how long they take to reach.
- **The inverse:** from Wrysk's target abundances and turnover, the rates needed, shown
  against the current knob values.

## How it is used

Every proposed ecology change goes through the model first, with a predicted number. The sim
run then tests that prediction, and runs are sized by the event count the question needs.

## Halves

- `design/webmodel/fauna.md`: the animal half, its laws as built, and its open-loop
  validation.
- `design/webmodel/flora.md`: the producer half, the same.
- `design/webmodel/meadow.md`: candidate mechanisms for a lasting grass niche, as plant-half
  levers, with their batch.
