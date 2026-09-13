---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Ambient support: one independently testable first axis

Proposal only. Keep the normal autonomous world exactly unchanged. Do not bundle
this with dose implementation or promote it to a default/preset before evidence.

The smallest first experiment is **natural rainfall availability**, not a generic
"autonomy" or activity multiplier. Use the existing `config.water.rain_rate` at
100% of the copied world's value versus one candidate at 90%. The default is 0.6
depth/second per unit weather excess; the candidate is `opening_rate * 0.9` (about
0.54 at defaults). Candidate magnitude is unvalidated. No sweep or seed-specific
rescue. Keep rain threshold, weather trajectories/RNG, evaporation, light,
producers, fertility, movement, reproduction, care doses and all inventories fixed.

This is a real independent input: `water::step` computes natural rain from that
rate and weather excess, then adds manual rain separately. It does not change
simulation time or animation speed. Growth sees wetting, flooding and the algae
light floor (`fields.rs`); less rain can **increase** some growth by relieving
flooding. Therefore do not label 90% "less self-sustaining" before measurement.
It tests the water part of ambient support, not automatic food provisioning or a
universal dependence on attention.

## Minimal implementation boundary for a future experiment

No production feature is required to test this axis. Clone the twelve prescribed
mature, no-hunter openings, change only the candidate's existing rain-rate config
field before constructing its world, validate, and record the exact config and
hash. The 100% arm must leave the original field untouched, not round-trip it
through a percent calculation. All input stocks, weather, IDs and opening ledgers
are identical. This uses an existing persisted field, adding no nested config
shape or weather RNG draws. Use a fixed common PRE-intervention inventory audit;
do not treat rebuilding a World as evidence that history balances.

A later live-adjustable setting is a separate persistence task: apply at a durable
simulation boundary, preserve the chosen rate through restart, and journal its
versioned identity/order. A browser preference must never silently rewrite the
world's config on reconnect. Name the control "Natural rainfall" until a broader
support meaning has been implemented and measured.

## Preregistered six-arm data collection

For every seed, pair 100% and 90% natural rainfall with three identical care
schedules: no input, Standard rain (1000), and Generous rain (1500). That is twelve
seeds × six arms; no hunters, feeding or cleaning in this mechanistic test.
Dose is an independent factor, not a consequence of the support selection.
Use one immutable schedule: a shower at elapsed tick 60 and every 2400 ticks
thereafter, rotating fixed ordinary-face/seam/rim targets using the existing care
comparison recipe. Keep each rejection and its reason. This two-minute schedule
is a controlled diagnostic exposure, **not** a proposed human-care obligation.
Freeze and retain every seed, including extinction and technical failure.

Run the two-hour screen first. If numerical/default-identity gates pass, extend
the same recipe to 24 and 72 hours; do not tune between horizons and call it the
same candidate. A technical pass is distinct from ecological acceptance.

At a fixed 200-tick cadence record total/by-face water, target-neighborhood water,
producer/fruit stocks, population by form, births/deaths, occupied cells and
surviving opening ancestry. Retain immediate care receipts and actual manual
water attribution. Report natural water as total rain minus manual attribution,
not a second external source. Use strict existing material/energy/water audits,
independent energy windows and receipt-boundary checks without tolerance changes.

Primary contrasts: 90%-minus-100% within each dose/schedule, then the difference
of care effects between support levels. Show every seed and zeros/censored losses,
not just pooled population or the most responsive target. Count increased local
water as physical response, not proof of improved reproduction, diversity or
long-term recovery. Changing dose should not change natural-weather input for a
matched elapsed interval; changing support should not rescale scheduled manual
dose. These are direct separation tests, alongside default continuation identity
and nonstandard-shower restart tests.

## Scale and interpretation caution

The actual old seed-1 twelve-hour diagnostic reports 27215.632366305632 natural
depth units and 1439.9999999999347 manual units in its repeated-care arm (360
standard showers). A 10% natural reduction would remove about 2722 units on that
same weather trajectory; 360 generous showers schedule 2160 units. These totals
do not imply spatial replacement: local rain, flow and evaporation matter.
That historical run's raw energy audit failed, so this arithmetic is a scale
observation from its retained JSON, not a claim that its ledger gate passed or
that three-seed care evidence establishes support balance.

If both support levels respond identically, that is an informative small/ineffective
axis, not grounds to secretly reduce support further. If 90% improves growth by
reducing flooding, report that instead of claiming greater care dependence. Any
future productivity or nutrient-support axis needs its own isolated proposal.
No experiment, production edit, live change or canonical decision was made here.
