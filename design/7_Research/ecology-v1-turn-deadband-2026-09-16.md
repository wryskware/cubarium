---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# The turn deadband alone: releasing it makes every tick a turning tick, and that is not the same thing as feeding

Workstream X of the ecology v1 round-5 next steps
([brief](../handoffs/ecology-v1-turn-deadband-opus-2026-09-16.md)), item 4 of the reconciled
round-4 next steps, as Astra bounded it in the correction block of
[Q's note](ecology-v1-es-antithetic-2026-09-16.md).

**In one line.** Releasing the `TURN` deadband does exactly what Q's measurement predicted to
the *channel* — turn activity goes from 0.72 of measurable ticks to **1.000 on all 132
trajectories** — and on frozen weights it buys no residence: on-food fraction does not move
(p = 0.34) and the mean dwell bout **falls** (p = 0.036). The replay's verdict is **mixed**, so
the bounded training pair ran, and there the picture is different: a `cub-act-2` run at the same
σ, pair count, score, layouts and seed has a higher mean population score in **15 of 16**
generations (p = 0.0005), higher opening residence in 14 of 16 (p = 0.004) and higher producer
intake per lived tick in 15 of 16 (p = 0.0005), and its selected centre outlives the retained
one on 6 of 8 held-out layouts (min 6,914 → 7,870). Nothing survives a horizon in either arm,
and none of this is a decision: it is one seed, one generation of one run replayed, and one
sixteen-generation training pair.

## The decision rule, registered before the replay ran

This section was written and committed at **`3ad0a88`**, before a single replay episode was run.
The same words are in `crates/cubarium-search/src/es/turnband.rs`'s module documentation and in
the `decision_rule` field of the report the experiment writes. The verdict below is this rule
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

**What follows from each branch**, also registered there: the bounded training pair (deliverable
3) runs only if the verdict is *supported* or *mixed*, and it is the retained command verbatim
with `--adapter cub-act-2`, a new `--out`, and a 20-wall-minute cap. If the verdict is
*falsified*, no training runs.

## Units and definitions

- A **trajectory** is one `(candidate, layout)` episode: 33 weight sets (generation 9's centre
  and its sixteen pairs' both halves) × 4 training layouts = 132 paired trajectories.
- A **candidate** is one weight set: `t_min` and the protocol's score are the minimum and the
  aggregate over its four layouts, so they are 33 paired values, not 132.
- **Turn activity**: the fraction of ticks with a non-zero resolved turn, and mean `|ω|` in
  rad/s. Both are measured exactly as `Episode::turn_sweep_rad` is — from the two stored
  headings — and exclude exactly the same ticks it does: a seam crossing, where a chart
  transport and a real turn cannot be separated, and the tick a body dies on, which has no
  post-step heading. `turn_measured_ticks` is the denominator.
- **On-food fraction**: workstream H's own `on_food`, taken from the world's per-tick intake
  trace rather than re-derived — a tick the body finished on a cell holding at least one of the
  four stocks (`P`, `F`, `D_eff`, `C_eff`) at or above `drives.feed_min`
  (`cubarium_core::IntakeTick::above_threshold`) — over the ticks the trace covered.
- **Dwell bout**: a maximal run of consecutive on-food ticks. The tested metric is the mean bout
  length in ticks; the count and the longest bout are reported beside it.
- **`t_min`**: the minimum survival ticks over the four training layouts, which is the
  `Aggregate::Min` protocol's survival term.

## Deliverable 1 — the adapter variant

`cub-act-2` differs from `cub-act-1` in **one constant**: `ActionAdapter::turn_band()` is
`DEADBAND` (0.05) under the first and `0.0` under the second. `Action7::squash_in` is one body of
code taking the adapter as an argument, not a second adapter, so the claim is a property of the
implementation rather than of a comment: the thrust band, the three intake bands, the two
`LEVEL` triggers, the capability masks, the shared-mouth normalisation and the order they run in
are shared literally. The tests check the brief's own case (a raw turn head of ±0.03 is exactly
0 under `cub-act-1` and `±tanh(0.03)` under `cub-act-2` at the same thrust), the thrust band on
both sides of its edge under both adapters, and every other channel's equality across a spanning
set of heads and four capability masks.

`PROFILE_TEXT` and `PROFILE_TEXT_CUB_ACT_2` are built by the **same macro** from the same
pieces, so `one.replace("cub-act-1", "cub-act-2") == two` is a test that can pass and a drift
that cannot happen. The digests that follow:

| adapter | schema digest |
| --- | --- |
| `cub-act-1` | `0x8be01a3aa8e9f4a2` — unchanged; the retained `centers/center-00009.json` carries it |
| `cub-act-2` | `0x97d426fd6bbb5ee1` |

**Selection is a `World` transient** beside `motor_model`, `apex_turn_radius` and
`apex_motor_model`: `set_action_adapter` / `action_adapter`, never persisted, never hashed, never
a `WorldConfig` field. A snapshot of a `cub-act-2` world is byte-identical to the same world's
`cub-act-1` snapshot and a resumed world runs `cub-act-1`.

**The byte-identical pin.** A default world, 1,000 ticks of ordinary life, eight of its bodies
dispatched to a recurrent policy, 3,000 further ticks:
`state_hash = 0x21d82443024c5f46`, and with the small-turn fixture whose raw turn head sits
inside the band, `0xbe591897fa3a5d8d`. Both were printed at the brief commit **`3f73845` on the
unmodified tree**, in the `test` and the `release` profile (they agree), and committed as
constants at `6e3ea33` before any implementation existed. Both still hold. The second fixture
under `cub-act-2` hashes differently, which is the check that the switch is live and the pin is
not vacuous.

**The refusals.** `Policy::validate_in(adapter)` is what the one explicit door into the
extension makes: `World::attach_neural_policy` and `World::found_neural_animal` check the
policy's digest against the adapter **the world is running**, not against the build's default.
So a `cub-act-2` policy is refused by name by the display host — a world that names no adapter —
and a `cub-act-1` policy is refused by name by a `cub-act-2` world, and a refused seed founds
nothing. Tested in `crates/cubarium-search/tests/turn_deadband.rs`, as the brief asked, rather
than in the host crate.

`Policy::validate()` — the decode-time range check that `WorldState::validate` runs on every
tick boundary of a traced episode — was widened from "this build's digest" to "a digest this
build knows under *some* adapter", because it also runs inside a world that is deliberately on
`cub-act-2`. The by-name refusal moved to the attachment boundary, where the adapter in force is
known. That is the one place a check got weaker, and it is stated rather than buried.

**Provenance.** `Protocol.adapter` and `PolicyFile.adapter` record it by name, absent from the
JSON when it is `cub-act-1`, so **every protocol hash written before this workstream is
unchanged**: the retained run's `0x8e51a1a9b1e2742b` is pinned in a test and reproduces. A
`cub-act-2` protocol is a different task with a different hash (`0x8304e9e2e02ab8fb` for the run
below) and a different `policy_digest`, and the two protocols are equal in every other field.
`PolicyFile::check_adapter` refuses a mismatch by name; a file with no `adapter` field reads as
`cub-act-1`, which is what every file written before the switch in fact ran — not "unknown".

**Commands.** `es-train` and `es-evaluate` take `--adapter {cub-act-1,cub-act-2}`. `es-export`
does not need one: it reads the adapter from the checkpoint it exports, exactly as it reads the
motor contract. `es-population` **takes the flag and accepts only `cub-act-1`**, refusing the
other by name: its stage runner `crates/cubarium-search/src/population.rs` is another
workstream's file this round and the brief forbids touching it, so the flag refuses rather than
silently running the shipped adapter. That is the one deliverable-1 item not fully delivered,
and re-opening it is a two-line change to `population::run_stage`'s signature.

A `cub-act-1` control re-run at this build reproduces the retained `holdout.json` **field for
field on all eight layouts** (`runs/ecology-v1-turn-deadband/holdout-act1-rebuilt.json` against
`runs/es-eco-v1-fastleaf/holdout.json`, written by build `63a1f4c60e2a-dirty`). That is the
search-side counterpart of the core's pinned hash.

## Deliverable 2 — the replay: 264 episodes, one weight set per pair, two adapters

Generation 9's centre and its own 32 candidates, on their own four training layouts, weights
untouched (`tensor::policy_in` restamps a theta's digest and moves no bit of it — tested), at the
protocol's horizon of 36,000 ticks. 264 episodes, **16.9 s** on 8 workers.

### The per-trajectory sign tests

| metric | unit | + | − | tie | p | median Δ | mean Δ |
| --- | --- | --- | --- | --- | --- | --- | --- |
| turn active fraction | trajectory | **132** | 0 | 0 | **3.7e-40** | +0.2060 | +0.2787 |
| mean \|ω\| | trajectory | **109** | 23 | 0 | **1.4e-14** | +0.0107 | +0.0116 |
| on-food fraction | trajectory | 72 | 60 | 0 | 0.3384 | +0.0026 | +0.0049 |
| mean dwell bout | trajectory | 53 | **78** | 1 | **0.0356** | **−3.17** | −2.36 |
| survival ticks | trajectory | 75 | 57 | 0 | 0.1387 | +67.5 | +166.6 |
| producer intake | trajectory | 73 | 59 | 0 | 0.2578 | +0.0176 | +0.0268 |
| opening residence fraction | trajectory | 70 | 62 | 0 | 0.5425 | +0.0001 | +0.0016 |
| `t_min` | candidate | 18 | 15 | 0 | 0.7283 | +13 | +15.9 |
| score | candidate | 18 | 15 | 0 | 0.7283 | +13 | +15.9 |

`score` and `t_min` are the same column because **every one of the 264 episodes ends in death**
before the 36,000-tick horizon, so `Episode::normalized_stores` is exactly 0 everywhere and the
protocol's score is exactly its survival term. That is a fact about generation 9 at this horizon,
not a reconstruction shortcut: the row carries `terminal_stores` and `store_capacity` and the
score is `trainer::score_by` itself.

### Verdict, by the registered rule: **mixed**

Neither branch is met. The support branch fails: on-food fraction does not rise (p = 0.34) and
`t_min` does not rise (p = 0.73). The falsification branch fails too, on its own terms: it
requires `p > 0.1` on *all three* of on-food fraction, dwell and `t_min`, and **mean dwell bout
is significant at p = 0.036 — in the negative direction**. Releasing the band does not leave
residence untouched; it makes it worse in a specific way.

### What actually changed

| | `cub-act-1` | `cub-act-2` |
| --- | --- | --- |
| turn active fraction, mean over 132 | 0.7213 (range 0.2015–0.9988) | **1.0000 on all 132** |
| mean \|ω\|, rad/s | 0.1294 | 0.1410 |
| on-food fraction, mean | 0.0724 | 0.0773 |
| dwell bouts per life, mean | 8.4 | 9.4 |
| mean dwell bout, ticks | 73.3 | **70.9** |
| survival ticks, mean | 8,122 | 8,288 |
| producer intake, mean (m) | 0.3212 | 0.3480 |
| `t_min` over 33 candidates | 6,459–8,915, mean 7,417 | 6,411–8,813, mean 7,433 |

The mechanism is legible in those two dwell rows: the released band gives **more, shorter**
bouts. A body that can always turn revisits food more often and leaves it sooner; the fraction
of life spent on food is statistically unchanged and its organisation into bouts is worse. Q's
prediction about the *channel* is confirmed exactly — the operating point straddled the edge and
releasing it takes turn expression from 72 % of measurable ticks to 100 % of them — and the
inference from there to residence does not hold on frozen weights.

Per layout, the differences are not uniform: `t3-scatter` gains (on-food up on 23 of 33
candidates, median +382 ticks), `t2-weak-open` and `t4-ring` are near-neutral, and
`t1-corridor` loses (on-food down on 20 of 33, median −190 ticks). The centre itself is
2,000 ticks worse on `t1-corridor` (9,429 → 8,480) and 1,167 better on `t3-scatter`
(9,397 → 10,564); since `t1-corridor` is what sets its `t_min`, the centre's *score* falls by 223.

## Deliverable 3 — the bounded training pair

The verdict is *mixed*, so the pair ran: the retained command verbatim, `--adapter cub-act-2`,
`--out runs/es-eco-v1-fastleaf-act2`. `Min`, σ = 0.02, 16 pairs, 16 generations, horizon 36,000,
`--train-seed 20260915`, the same four layouts and the same `fast-leaf` ecology
(`09e244392ec91768`). 8 workers, **139.5 s wall** against the 20-minute cap; no retry, no
continuation, no tuning. The retained `cub-act-1` run is the control and was **not** re-run.
Protocol hash `0x8304e9e2e02ab8fb`.

| gen | centre 1 | centre 2 | best 1 | best 2 | mean 1 | mean 2 | opening 1 | opening 2 | intake/tick 1 | intake/tick 2 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 6521 | 6505 | 7997 | 7997 | 6896 | 6886 | 0.0332 | 0.0322 | 3.37e−5 | 3.26e−5 |
| 1 | 6948 | 6517 | 8262 | 8320 | 6913 | 7019 | 0.0372 | 0.0393 | 3.55e−5 | 3.74e−5 |
| 2 | 6511 | 7098 | 7998 | 8152 | 6832 | 7001 | 0.0240 | 0.0367 | 2.82e−5 | 3.87e−5 |
| 3 | 7852 | 7295 | 7864 | 8734 | 6972 | 7084 | 0.0201 | 0.0385 | 2.91e−5 | 3.95e−5 |
| 4 | 8000 | 7967 | 8151 | 8557 | 7121 | 7124 | 0.0215 | 0.0303 | 3.32e−5 | 3.80e−5 |
| 5 | 7929 | 8335 | 7910 | 9149 | 7202 | 7400 | 0.0223 | 0.0303 | 3.46e−5 | 4.07e−5 |
| 6 | 7688 | 8486 | 8918 | 8840 | 7325 | 7386 | 0.0222 | 0.0286 | 3.45e−5 | 3.88e−5 |
| 7 | 8227 | 7440 | 8681 | 8749 | 7287 | 7605 | 0.0199 | 0.0251 | 3.44e−5 | 3.92e−5 |
| 8 | 8041 | 8458 | 8426 | 8703 | 7111 | 7356 | 0.0233 | 0.0232 | 3.52e−5 | 3.73e−5 |
| 9 | 8703 | 7707 | 8915 | 8758 | 7376 | 7556 | 0.0231 | 0.0297 | 3.73e−5 | 4.22e−5 |
| 10 | 8108 | 7858 | 8904 | 8769 | 7402 | 7751 | 0.0228 | 0.0336 | 3.91e−5 | 4.71e−5 |
| 11 | 7761 | **8959** | 8625 | 9054 | 7335 | 7751 | 0.0230 | 0.0368 | 3.80e−5 | 4.91e−5 |
| 12 | 7596 | 8554 | **10276** | 9913 | 7477 | 7682 | 0.0221 | 0.0388 | 4.15e−5 | 4.64e−5 |
| 13 | 7402 | 7153 | 8458 | 9386 | 7364 | 7860 | 0.0232 | 0.0482 | 4.07e−5 | 5.22e−5 |
| 14 | 7948 | 7862 | 9689 | 8854 | 7656 | 7818 | 0.0284 | 0.0434 | 4.42e−5 | 5.06e−5 |
| 15 | 7110 | 7495 | 9045 | 9127 | 7655 | 7861 | 0.0255 | 0.0424 | 4.66e−5 | 5.14e−5 |

`opening` is the mean over a generation's 128 candidate episodes of `ticks_in_opening / ticks`,
and `intake/tick` the mean of `intake_producer / ticks`. They stand in for on-food fraction
because a `GenerationReport` retains `Episode`, which does not carry H's `on_food` column; the
replay above is where the real on-food number is measured, and this table says so rather than
relabelling a proxy.

Paired by generation, exact two-sided sign tests:

| quantity | `cub-act-2` higher | p |
| --- | --- | --- |
| mean population score | **15 / 16** | **0.00052** |
| mean producer intake per lived tick | **15 / 16** | **0.00052** |
| mean opening residence fraction | **14 / 16** | **0.00418** |
| best candidate | 10 / 15 | 0.302 |
| centre score | 7 / 16 | 0.804 |

Trend, OLS over the sixteen generations' mean population score: **49.9 ticks/generation** under
`cub-act-1`, **68.0** under `cub-act-2` (6,886 → 7,861 against 6,896 → 7,655). Mean opening
residence over the whole run 0.0245 → **0.0348** (+42 %); mean intake rate
3.66e−5 → **4.26e−5** (+16 %). Mean gradient norm 237 → 264. The *centre* score is the one
column that does not separate, which is the ordinary consequence of the ±1,200 of noise Q
measured on it.

### Held out

Selection rule as the retained run's: highest recorded centre score, earliest generation on
ties, training results only, frozen before any held-out episode. `cub-act-2` selects
**generation 11** (8,959); the control is generation 9 (8,703). Exported policy digest
`0x97d426fd6bbb5ee1`, weights `0x80a553cc362a82c8`, round trip exact bit for bit.

| layout | control ticks | `cub-act-2` ticks | Δ | intake P control | `cub-act-2` |
| --- | --- | --- | --- | --- | --- |
| h1 | 8560 | 8134 | −426 | 0.364 | 0.342 |
| h2 | 6914 | 8395 | **+1481** | 0.138 | 0.330 |
| h3 | 8707 | 9421 | +714 | 0.377 | 0.528 |
| h4 | 8070 | 8492 | +422 | 0.273 | 0.384 |
| h5 | 7848 | 7956 | +108 | 0.249 | 0.260 |
| h6 | 7722 | 8177 | +455 | 0.255 | 0.308 |
| h7 | 8282 | 7870 | −412 | 0.331 | 0.270 |
| h8 | 7743 | 8579 | +836 | 0.242 | 0.374 |

Minimum over the eight 6,914 → **7,870**; mean 7,981 → 8,378; higher on 6 of 8 (p = 0.29 — eight
layouts cannot carry a sign test, and the number is reported for what it is). Mean intake per
lived tick 3.44e−5 → 4.14e−5. Mean opening residence is **unchanged**, 0.0324 → 0.0327 — the
same dissociation the replay found, now on a trained policy: more food through the mouth, not
more time in the opening patch. **Neither arm survives a single holdout horizon**, and a
scripted route-follower scores 36,000 on this task ([L](ecology-v1-score-checks-2026-09-16.md)).

## Deliverable 4 — gradient-direction stability, from the retained pair contributions

From `runs/ecology-v1-es-antithetic/pairs.json`'s sixteen generation-9 `weight` values (Q's
`u_plus − u_minus`; none is exactly zero) with the perturbations regenerated from the run's own
seed. A subset `S` estimates the direction `Σ_{i∈S} wᵢεᵢ`; the whole estimate is that sum over
all sixteen, and the gradient the trainer applied is the same vector over the positive constant
`2nσ`, which no cosine can see. Everything below is exact from the 16×16 Gram matrix
`Gᵢⱼ = wᵢwⱼ⟨εᵢ, εⱼ⟩`, so **all 6,435** balanced 8/8 splits were evaluated, not the brief's
sample of 70. 0.01 s.

| distribution | n | mean | sd | min | p05 | median | p95 | max | ≤ 0 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| split-half cosine | 6435 | **0.0158** | 0.0057 | −0.0030 | 0.0066 | 0.0156 | 0.0252 | 0.0332 | 11 |
| bootstrap vs the whole estimate | 1000 | **0.7379** | 0.0981 | 0.3867 | 0.5684 | 0.7492 | 0.8868 | 0.9614 | 0 |
| bootstrap vs the recorded update `θ₁₀ − θ₉` | 1000 | 0.2640 | 0.0350 | 0.1391 | 0.2027 | 0.2673 | 0.3159 | 0.3439 | 0 |

The whole sixteen-pair direction's own cosine with the recorded update is **0.3581**
(‖update‖ 3.18e−1, ‖direction‖ 1.17e2). That is well below 1 *although the update is the
estimator's* — Adam rescales every coordinate to about `±lr`, which is a large rotation — and it
is reported so the bootstrap row has its centre to be read against.

**What the spread says.** Two independent isotropic directions in 10,215 dimensions have
cosine `≈ N(0, 1/10215)`, standard deviation **0.00989**. The median split-half cosine, 0.0156,
is about 1.6 of those. Under the usual decomposition — each half is a common direction plus
independent isotropic noise, so `E[cos] ≈ ‖signal‖²/(‖signal‖² + ‖noise‖²)` — eight pairs
estimate a direction whose **shared component is about 1.6 % of its squared length**; the rest is
sampling noise. 6,424 of the 6,435 splits are positive, so the shared component's *sign* is
consistent; but the 6,435 splits are recombinations of the same sixteen vectors, not 6,435
independent samples, and that count is one fact about this generation, not 6,435 pieces of
evidence. The bootstrap says the complementary thing: resampling the sixteen pairs reproduces
the whole estimate at cosine 0.74 and never points away from it, so the estimate is not
dominated by one or two pairs.

**This does not say whether sixteen pairs suffice**, and the brief asked it not to. Two
qualifications in particular:

- the split-half here reuses the **full 32-candidate population's** centred ranks for both
  halves, because those are what the retained `pairs.json` holds. A genuine eight-pair run would
  re-rank within its own sixteen candidates, which shares less information between the halves,
  so this measurement can only make the halves look *more* alike than two independent runs
  would;
- one generation of one run. Q's own table shows the pair weights vary a lot generation to
  generation.

**What would decide it**: a second arm at the same protocol with a different `--train-seed`, or
an episode-matched arm at 32 pairs × 8 generations, comparing how far the centres separate
between seeds against how far they move within one run. Both are `es-train` invocations with no
new code, and the second is exactly one `--pairs 32 --generations 8`.

## What would change on the cube

> **Added at integration (Fable, `997fb24`).** One more refusal beside the three below:
> `World::from_state` refuses, by name, a snapshot holding a `cub-act-2` policy. The adapter
> is a transient the bytes cannot carry, a resumed world runs `cub-act-1`, and decoding those
> weights under it would have been silent and wrong once `Policy::validate()` was widened to
> accept any adapter this build knows. A shipped-adapter world with a policy round-trips as
> before; tested in `crates/cubarium-core/tests/action_adapter.rs`.


**Nothing, today.** The display host builds worlds that never name an adapter, so it runs
`cub-act-1`, byte for byte the build before this workstream — that is what the pinned state hash
and the field-for-field holdout reproduction are for. A `cub-act-2` policy is refused **by name**
by `attach_neural_policy` and by `found_neural_animal`, and a `cub-act-2` policy *file* is
refused by `PolicyFile::policy` before its weights are rebuilt. The cube changes only if someone
(a) selects a `cub-act-2` policy and (b) teaches the host to set the adapter on the world it
seeds into — two deliberate steps, neither of which this workstream takes. `es-population`, the
one path that puts a trained policy into a whole calibrated world, refuses `cub-act-2` outright.

If that were done, what a viewer would see is the change the replay measured: a body that turns
on **every** tick rather than on 72 % of them, with a visibly less steady heading, revisiting
food patches more often and lingering on each one about 3 ticks less. It would not look like an
animal that feeds itself: it still starves at ~8,400 ticks on every layout.

## Build and provenance

- Tests and the pinned hash, from the definitions, before any implementation: **`6e3ea33`**.
  Implementation: **`f6cb2d4`**. The decision rule, before the replay: **`3ad0a88`**. This note
  and the results: the commit that carries them.
- Search build id **`3ad0a88`**, pinned with `CUBARIUM_SEARCH_BUILD`; the release binary was
  copied out of the shared `target/` before every run, as the brief requires. `cargo fmt` was
  never run.
- `cargo test -p cubarium-core`: **574 passed, 0 failed, 4 ignored** (11 new in
  `tests/action_adapter.rs`). `cargo test -p cubarium-search`: **295 passed, 0 failed, 5
  ignored** (9 new in `tests/turn_deadband.rs`, 4 new unit tests in `es::turnband`).
  `cargo build --workspace` is clean, `crates/cubarium` included and untouched.
- Wall time: replay 16.9 s (264 episodes, 8 workers), stability 0.01 s, training 139.5 s
  (2,116 episodes, 17,983,110 ticks), export + two holdout evaluations 11 s.
- Outputs, **5.1 MiB** against the brief's 60: `runs/ecology-v1-turn-deadband/`
  (`replay.json` 242 KiB, `stability.json` 2 KiB, `holdout-act1-rebuilt.json` 8 KiB) and
  `runs/es-eco-v1-fastleaf-act2/` 4.8 MiB.

### Exact commands

```bash
CUBARIUM_SEARCH_BUILD=3ad0a88 cubarium-search es-turn-band \
    --run runs/es-eco-v1-fastleaf \
    --config runs/ecology-v1-calibration/selected/fast-leaf.toml \
    --generation 9 --horizon 36000 --workers 8 --wall-seconds 900 \
    --pairs runs/ecology-v1-es-antithetic/pairs.json --bootstrap 1000 \
    --out runs/ecology-v1-turn-deadband

CUBARIUM_SEARCH_BUILD=3ad0a88 cubarium-search es-train \
    --config runs/ecology-v1-calibration/selected/fast-leaf.toml \
    --pairs 16 --generations 16 --horizon 36000 --workers 8 \
    --wall-seconds 1200 --train-seed 20260915 --center-eval true \
    --adapter cub-act-2 --out runs/es-eco-v1-fastleaf-act2

CUBARIUM_SEARCH_BUILD=3ad0a88 cubarium-search es-export \
    --checkpoint runs/es-eco-v1-fastleaf-act2/checkpoint.json \
    --config runs/ecology-v1-calibration/selected/fast-leaf.toml \
    --generation 11 \
    --out runs/es-eco-v1-fastleaf-act2/selected/center-00011-policy.json

CUBARIUM_SEARCH_BUILD=3ad0a88 cubarium-search es-evaluate \
    --policy runs/es-eco-v1-fastleaf-act2/selected/center-00011-policy.json \
    --set holdout --horizon 36000 --wall-seconds 300 \
    --config runs/ecology-v1-calibration/selected/fast-leaf.toml \
    --adapter cub-act-2 --out runs/es-eco-v1-fastleaf-act2/holdout.json
```

## Why the numbers can be trusted

- **The replay is a replay.** `tensor::policy_in` restamps a theta's digest and moves no bit of
  its weights — checked to the bit in `restamping_a_theta_for_the_other_adapter_moves_no_weight`
  — and the candidates are rebuilt by the trainer's own `rng::perturbation` at the run's own
  seed and generation, in the trainer's own candidate order.
- **The measurement does not disturb what it measures.** `a_traced_episode_is_the_same_episode`
  runs one policy on one layout with the trace on and off and asserts the two `Episode`s are
  equal field for field, that the trace's turn sum is the episode's `turn_sweep_rad` to 1e−12,
  and that the recorder drops nothing at the 2,048-tick drain cadence. No traced episode in the
  264 dropped a row.
- **The score is the protocol's.** `trainer::score_by` itself, on the four fields it reads, each
  recorded on the row.
- **The `cub-act-1` arm is the retained run.** The control's protocol hash reproduces
  (`0x8e51a1a9b1e2742b`, pinned in a test), the retained centre file's digest is this build's
  `cub-act-1` digest, and the retained `holdout.json` reproduces field for field on all eight
  layouts at this build.
- **The rule was applied, not chosen.** It is a `const` in the module, it is written into the
  report's own `decision_rule` field, its arithmetic is tested against hand-computed binomial
  values, and it was committed before the first episode ran.

## What this does not establish

- **Nothing about a whole world.** Twelve frozen single-body layouts, one ecology, one seed, one
  genotype, one horizon; and `es-population` — the only path from a policy to a whole calibrated
  world — was not run and currently refuses this adapter.
- **Nothing about the deadband being the constraint.** The training arm is *one* pair of runs at
  *one* seed. Its per-generation sign tests are over sixteen paired generations of the same two
  trajectories, which are not sixteen independent experiments: a single lucky early generation
  propagates. The honest reading is "this arm was better on the aggregate columns, consistently,
  and by a margin the frozen-weight replay did not predict", not "releasing the band is the fix".
  A second seed is what would separate the two.
- **Nothing about why the training arm gains** while the frozen-weight replay does not. The
  obvious hypothesis — that the gain is *learnability* rather than *expression*, i.e. the search
  can now move a channel whose output was clipped to zero for most of a newborn candidate's
  ticks (Q measured 29 of 32 candidates at 100 % turn occupancy from a reset hidden state) — is
  consistent with everything here and is not tested by anything here.
- **Nothing about the dwell regression persisting after training.** Mean dwell bout fell on
  frozen weights; it was not measured on the trained `cub-act-2` centre, because
  `GenerationReport` and `es-evaluate` do not carry the column. A traced comparison of the two
  selected centres is one `es-turn-band`-shaped run away.
- **Nothing about sixteen pairs.** See deliverable 4's two qualifications.
- **Nothing about `cub-act-2` being a good adapter to keep.** It is a fresh task under a fresh
  protocol hash, never a migration; no policy trained under it can be loaded by anything that
  runs `cub-act-1`, and that is by construction.
- `cargo clippy -p cubarium-core --all-targets` still fails on the pre-existing `erasing_op`
  deny in `crates/cubarium-core/src/neural/gru.rs:233`, untouched by this workstream.
