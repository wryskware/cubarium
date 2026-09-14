---
design_status: exploration
last_reviewed: 2026-09-14
decision_refs: []
---

# R0a: what paid rotation costs the ecology — 2026-09-14

Evidence for Fable. R0a makes body rotation a physical, paid act. This note separates the two
things that changed and measures each, because only one of them turns out to matter
ecologically — and it is the one whose coefficient is a free choice, not the one the handoff
settled.

## The two changes are not the same change

- **Kinematic:** a body can no longer rotate like a point. Its turn rate is bounded by
  `|v| + r · |ω| ≤ u`, so a body wider than `motor::REFERENCE_RADIUS_PX` (2.5 px, the decoded
  extent of a unit adult) trades speed against turning. This is what the handoff settled.
- **Economic:** turning now costs energy, at `move_cost · S · k · r|ω| · dt` with
  `k = motor::ROTATION_COST_SCALE`. The handoff settled *that* rotation is paid. It did not
  settle *how much*, and explicitly left coefficients revisable.

These are separate constants on purpose. `REFERENCE_RADIUS_PX` sets how fast a body may turn;
`ROTATION_COST_SCALE` sets what turning costs. Neither moves the other.

## Measurement

The live world (`state/world-115200.cubw`, schema 14, tick 115,200, 41 organisms) resumed
headless on each build. Six runs, all headless, about 45 s of wall time in total.

| Build | `k` | Population after 300 s | Population after 2 h |
| --- | --- | --- | --- |
| pre-R0a (`main`) | rotation free | 96 | 96 |
| R0a | 0.00 | 93 | — |
| R0a | 0.25 | 76 | — |
| R0a | **0.50** (shipped) | 57 | **76** |
| R0a | 1.00 | 39 | — |

## What that says

- **The kinematic limits are ecologically nearly free.** At `k = 0` — every turn bounded by the
  envelope, none of them paid — the population reaches 93 against the pre-R0a 96. Slowing large
  bodies down does almost nothing to the food web.
- **The whole effect is the price.** Population at 300 s falls monotonically with `k`, from 93
  to 39. Every ecological consequence of this milestone is the energy rotation now costs.
- **Why `k = 1` would be too much.** `move_cost` was set against a top centre speed of
  0.3 px/s. A unit adult turning at its genome's 90°/s sweeps its outer point at 3.9 px/s, so
  pricing sweep at par with travel makes turning cost roughly thirteen times as much as going.
  That is a change to the world's energy economy far larger than the kinematic change this
  milestone is about, and it is not implied by "rotation is effort".
- **Why 0.5.** The *constraint* is on the fastest-moving part of the body, so the envelope uses
  the outermost radius. The *cost* is work done by the whole body, whose mean sweep radius is
  smaller: one half the outer radius for a uniform rod about its centre, two thirds for a
  uniform disc. 0.5 is the rod figure — physics, conservative against the mean, and still a
  real price. It is not a balance choice and is not offered as one.
- **This is not decline.** At `k = 0.5` the world carries 76 after two hours and was still
  climbing (57 at 300 s, 76 at 2 h); `main` sits flat at 96 from 300 s onward. The shipped
  build lowers the level the world settles at by about a fifth and slows the approach to it. It
  does not empty out.

## Open for Fable

1. Is a fifth off the standing population an acceptable price for honest turning, or should
   `ROTATION_COST_SCALE` come down (0.25 gives 76 at 300 s) — or `move_cost` be reconsidered
   against the sweep it now has to cover?
2. `REFERENCE_RADIUS_PX` is calibrated so a unit adult keeps exactly its genome's turn rate at
   full effort. That is a choice about where the size penalty starts biting, and every larger
   body pays for it. An apex, at ~14.8 px root-to-claw, comes down from 90°/s to about 16°/s.
   Its stalk timeout is 8 s; a 90° reorientation now takes 5.6 s. Contact still works (the
   hunter suite passes, including capture timing), but the margin is thin and worth a look.
3. Neither number is canon. Both are single `pub const`s in `crates/cubarium-core/src/motor.rs`
   with the reasoning above them.
