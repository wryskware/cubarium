---
status: resolved
date: 2026-09-17
owner: Fable
---
> **RESOLVED 2026-09-17.** Landed on main; see git log for the commits.

# Voxel round 3b: Astra's round-4 changes before the presets

Astra reviewed main at 332d3fc (round 2 producers + round 3 corrections H/I/J) and
requested changes before clearing for the presets round. The review is Round 4 of
`design/7_Research/astra-voxel-first-wave-review-2026-09-16.md` (findings R4.1–R4.8).
Read it first; this brief only turns the findings into one work package and makes
the design calls they leave open. Kept as they are: the organic/mineral split, the
proportional-fraction transfer rule, and the target-based aeration stress.

## Package K (one worker, Opus high, main checkout)

Files: `crates/cubarium-voxel-flora/**`, `design/backlog.md` rows, and the results
note. Core crate untouched unless a query is missing (add with a test, say so).
Commits in this order, explicit paths, one per numbered item at least.

### K1 — seed bank age without rejuvenation (R4.1)

Replace "merge same-species cohorts within one tick and keep the younger age" plus
"cap by merging the two oldest". New rule: cohorts are **arrival-time bins**. Bin
width `seed_max_age_s / seed_cohorts_max` (placeholders 600 s / 4 → 150 s). A landing
joins the bin whose start tick covers the current tick, creating it if absent; a
bin's `age_ticks` is measured from its **start** and never decreases; merging within a
bin sums organic and mineral. A bin whose start age exceeds `seed_max_age_s` goes to
litter whole. So a site holds at most `seed_cohorts_max + 1` bins per species by
construction, old material leaves on its own schedule, and small fresh arrivals cannot
keep it. Attrition unchanged. Rewrite `a_fed_bank_stays_one_cohort_at_age_zero…` and
`a_pulsing_donor_stacks…` in `tests/round3.rs` to the new rule and add Astra's fixture:
a two-tick lifetime (set `seed_max_age_s` tiny), germination disabled (site fails the
predicate), tiny continuing arrivals: old material reaches litter on schedule with
organic, mineral and energy booked.

### K2 — the harness water budget (R4.2)

The harness rain 0.0005 m/s over 192 m² is 0.096 m³/s in against an outlet of 0.05
m³/s and zero evaporation, so the table climbs for the whole run. Print, over every
progress interval and at the end: accepted rain, outlet export, evaporation,
transpiration, and storage change, and delete the harness comment claiming the tap
stays below export. Choose the harness rain so nominal input sits below outlet
capacity (ceiling 0.05/192 = 2.6e-4 m/s) and print the head at each interval to show
it is bounded; this is an **experiment condition**, not a model knob, and the
number you pick goes in the note. Hold across arms: terrain, initial water and
mineral, forcing, and observation phase.

### K3 — funded reproductive parcels (R4.4)

Today the donor's budget is `rate · DT · recipient_count` split over every
highest-support face within `hop`, so no recipient's bank ever nears
`alive_min / w_frac` while the donor is stressed. New rule: a donor accumulates a
**paid parcel** from its reproductive surplus at the existing rate (one recipient's
worth per tick, not `recipient_count` worth); when the parcel holds one minimum
package (net `alive_min / w_frac` = 0.05 organic; the donor pays `(1 + c_g)` times
that from reserve, construction respired to `respired_out`), the whole package lands
on **one** recipient site within `hop`, chosen from the donor's own deterministic RNG
stream without habitat screening, never the donor's own site. Mineral travels by the
fraction rule. `Stand` gains `parcel: f64` (organic being saved; its mineral stays in
the stand until delivery). The ledger's `propagules_out` semantics stay: what left
donors. The diagnosis prints **requested, funded and landed** reproductive flux per
species. Tests: a donor with ample surplus lands exactly one package after the
predicted number of ticks, on one site, with debit = package + construction; a
stressed donor with no surplus lands nothing and its parcel does not grow; the parcel
dies with the donor (booked to litter with its mineral).

### K4 — gap arbitration and one-package germination (R4.5)

Replace "first species in `Species::ALL` wins" with a reproducible local lottery:
among species whose bank on the site holds at least one package and pass the
predicate, draw with weights = whole packages held, from an RNG seeded by (world seed,
site index, tick) so storage order cannot pick the winner. Consume exactly **one**
package from the winner's oldest bins first (wood = w_frac · 0.05 etc.), leave the
remainder ageing, move the consumed bins' actual mineral proportionally. Losing banks
stay. Tests: a contested gap with the bank order swapped gives the same winner
distribution over 200 seeded draws (both species win somewhere); the losing bank
remains; a birth from an oversized bank has exactly the package's stocks and the
surplus stays banked; nothing is deleted (residuals).

### K5 — lineage by identity and honest probe wording (R4.7)

`Stand::id: u64` from a ledger birth counter (founders included). Descendant
counting in the harness compares identities, not sites. Replace "INVASION FAILS/
SUCCEEDS" with "recruitment observed / not observed within T s". Add a fixture where
a founder dies and its own species germinates on the site in the same tick and the
count still sees one death and one birth. Do **not** rerun the coexistence probe this
round; Astra wants a positive control (the founder treatment replacing itself alone
under the same forcing) designed first. Put that design question in the note's
open items.

### K6 — documentation corrections (R4.3, R4.6)

`design/backlog.md` row 42: the stock cap is 50·N and the per-tick rate cap 0.0005·N,
so the former never binds for positive N; say so. Document `Stand::mineral` as an
inventory, not a reusable internal reserve: a stand on a bare pool fixes nothing
because the Michaelis–Menten and rate cap read the ground pool, and that is a stated
limitation until nutrient physiology is split. Document the lazy `initial_mineral`
as provisioning previously unrepresented ground (colonisation imports mineral,
booked). In `lib.rs`, state that sharing the adult stress tolerance with the
germination ceiling is an assumption, and that umbrellafrond at 1.0 is a
saturation-immune wetland proxy, not the biosphere's aerated-understory role; do not
lower it.

### Then

One `compare 2000 1 101 202 7` under K2's bounded water regime, as evidence not a
gate (~30 min wall; background and poll). Append "Rerun after package K" to the
results note: establishments/deaths per arm, per-species descendant fractions by
identity, requested/funded/landed reproductive flux, and the water budget showing
the head bounded. Then Astra round 5 (`task --resume-last`).

## Deferred to the presets round (R4.8)

Each new preset gets a paid-recruitment/survival fixture at its soil/light boundary
with a failing neighbour; lower-face dispersal and canopy-sensitive germination are
explicit rule changes if a preset needs them. The exposed predicate is abiotic
eligibility, not realised recruitment habitat.

## Rules

Fast iteration; explicit-path commits; no cargo fmt; no knob tuning (placeholders
named in commit messages and backlog rows); always fresh; do not touch
`design/handoffs/README.md`, the cube, or the core unless a query is missing; never
HashMap iteration reaching state or output; short function tests only.
