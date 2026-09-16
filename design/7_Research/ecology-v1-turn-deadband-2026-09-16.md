---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# The turn deadband alone: `cub-act-2`, and the replay that has to earn the training run

Workstream X of the ecology v1 round-5 next steps
([brief](../handoffs/ecology-v1-turn-deadband-opus-2026-09-16.md)), item 4 of the reconciled
round-4 next steps, as Astra bounded it in the correction block of
[Q's note](ecology-v1-es-antithetic-2026-09-16.md).

## The decision rule, registered before the replay ran

This section was written and committed **before** a single replay episode was run, and the same
words are in `crates/cubarium-search/src/es/turnband.rs`'s module documentation and in the
`decision_rule` field of the report the experiment writes. The verdict below is this rule
applied by the code, not chosen after the numbers.

> A metric **rises** when the exact two-sided sign test on its paired
> (`cub-act-2` minus `cub-act-1`) differences gives `p ≤ 0.1` **and** strictly more than half of
> the non-zero differences are positive. Zero differences are ties: they are dropped, and the
> test's `n` is the number that remain.
>
> - Turn release is **falsified as the bottleneck** if turn activity rises while on-food
>   fraction, mean dwell bout and `t_min` all fail to (`p > 0.1` on all three).
> - It is **supported** if on-food fraction or `t_min` rises.
> - Anything else is **mixed**, and the report says so rather than choosing.

The two branches are disjoint by construction: "supported" requires `p ≤ 0.1` on a metric that
"falsified" requires `p > 0.1` on.

**What follows from each branch**, also registered here: the bounded training pair (deliverable
3) runs only if the verdict is *supported* or *mixed*, and it is the retained command verbatim
with `--adapter cub-act-2`, a new `--out`, and a 20-wall-minute cap. If the verdict is
*falsified*, no training runs.

## Units

- A **trajectory** is one `(candidate, layout)` episode: 33 weight sets (generation 9's centre
  and its sixteen pairs' both halves) × 4 training layouts = 132 paired trajectories. Turn
  activity, mean `|ω|`, on-food fraction, mean dwell bout, survival ticks, producer intake and
  opening residence are measured per trajectory.
- A **candidate** is one weight set: `t_min` and the protocol's score are the minimum and the
  aggregate over its four layouts, so they are 33 paired values, not 132.

## Definitions

- **Turn activity**: the fraction of ticks with a non-zero resolved turn, and mean `|ω|` in
  rad/s. Both are measured exactly as `Episode::turn_sweep_rad` is — from the two stored
  headings — and exclude exactly the same ticks it does: a seam crossing, where a chart
  transport and a real turn cannot be separated, and the tick a body dies on, which has no
  post-step heading. `turn_measured_ticks` is the denominator, so the fraction is a fraction of
  the ticks the turn could actually be read on.
- **On-food fraction**: workstream H's own `on_food`, taken from the world's per-tick intake
  trace rather than re-derived — a tick the body finished on a cell holding at least one of the
  four stocks (`P`, `F`, `D_eff`, `C_eff`) at or above `drives.feed_min`
  (`cubarium_core::IntakeTick::above_threshold`) — over the ticks the trace covered.
- **Dwell bout**: a maximal run of consecutive on-food ticks. The tested metric is the mean bout
  length in ticks; the count and the longest bout are reported beside it.
- **`t_min`**: the minimum survival ticks over the four training layouts, which is the
  `Aggregate::Min` protocol's survival term.

*(The remainder of this note is written after the experiments; everything above this line was
committed first.)*
