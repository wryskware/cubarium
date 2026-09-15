---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# R2a: the evolution-strategy trainer and the plumbing smoke — 2026-09-15

Evidence for Fable against [the R2a brief](../handoffs/r2a-fable-trainer-2026-09-15.md),
on top of [the R1a runtime](r1a-runtime-result-2026-09-14.md) as
[verified](r1a-runtime-review-2026-09-14.md) at `bf96ecb`. This decides nothing and marks
nothing canonical. It records what was built, the exact frozen protocol, what the controls
and the smoke actually measured, and the compute checkpoint.

**No learning campaign was run.** No policy was attached to the display world, no world was
reset, and the running display (pid 2062111) was not touched. There is no trained policy in
this milestone and no claim about foraging competence, learning or sustainability.

## 0. What shipped

| commit | what |
| --- | --- |
| `2331ae8` | `es/{tensor,rng,optimizer,fixture,episode,trainer,bits}.rs` and their checks |
| `f716256` | the six `es-*` commands, `es/export.rs`, persistence and the export |
| this one | the final-centre evaluation, the controls/smoke results and this document |

Everything lives in `crates/cubarium-search/src/es/`. M1's ecological search is untouched:
`params`, `metrics`, `evaluate`, `search` and all four of its commands (`params`, `baseline`,
`search`, `replay`) are unmodified, no neural weight enters M1's parameter vector, and no
`Feeding`-mode fitness is reused. The two share the crate because a bounded worker pool and a
build stamp are the same infrastructure, not because a policy is an ecology parameter.

## 1. One simulator, reused — the exact calls

An episode (`es/episode.rs::run`) is nothing but core calls:

| step | call |
| --- | --- |
| build the world | `World::new(cfg)`, field staging, `World::from_state`, `World::check_invariants` |
| place the body | `Genome::founder(0.5, &cfg.drives)`, `genome::decode`, `organisms.insert` |
| make it neural | `World::attach_neural_policy(id, policy)` — the single explicit door |
| disable births | `World::set_scripted_intents` with `bud: Some(false)` and nothing else set |
| run | `World::step()`, `World::drain_events()` |
| read | `World::intake_diagnostics()`, `World::moved_segments(id)`, `MotorBill::of/upkeep/motor_cost` |

There is **no second simulator and no copied neural forward pass**. The 70-scalar sampler, the
GRU32 forward, the action adapter, the motor envelope, the intake law, oxidation, growth,
ageing and the repaired starvation and maturity rules are all the world's own, unchanged. The
trainer's only contribution inside a tick is the weights it handed to `attach_neural_policy`.

Every candidate gets a fresh isolated world and private zero `hidden`/`held`/`feedback` (that
is what `AnimalState::fresh` does at attachment). Nothing is carried between episodes.

## 2. The optimizer, pinned

Antithetic Gaussian ES with centred-rank utilities and Adam ascent. `es-protocol` prints all
of this; **protocol hash `0xb69033f65e56f1df`**, policy schema digest `0xb409fed56734b25e`,
10,215 parameters.

**Flatten order** (`es/tensor.rs`, the only place that decides it): `w_i`, `w_h`, `b_i`, `b_h`,
`w_o`, `b_o`, each row-major with gate rows in `(r, z, n)` order — the declaration order of
`Gru32`'s own fields. Nothing reorders the GRU's loops.

**Gaussians** (`es/rng.rs`): `gaussian(seed, stream, key, index)` is Box–Muller over two
dedicated counters `2i`, `2i+1`, so a draw is a pure function of its position and element
9,000 can be produced without the first 8,999. `crate::rng::normal` is deliberately **not**
reused: it spends two *consecutive* counters per value, so adjacent elements of a
10,215-dimensional perturbation would share a uniform draw. The measured lag-1 correlation of
the replacement over 20,000 elements is below 0.05 with mean 0.00 and variance 1.00.

**Update.** For `n` pairs, one `epsilon_i ~ N(0, I)` per pair, evaluated at
`theta ± sigma·epsilon_i`. Rank the `2n` aggregate scores ascending and zero-based, **average
ranks** for exact ties, `u = rank/(2n-1) - 0.5`, then

```
g = sum_i (u_plus_i - u_minus_i) * epsilon_i / (2 * n * sigma)
theta <- theta + lr * m_hat / (sqrt(v_hat) + eps)      (Adam, ascent, bias-corrected)
```

`sigma = 0.02`, `lr = 0.01`, `beta1 = 0.9`, `beta2 = 0.999`, `eps = 1e-8`, no weight decay.
Nothing adaptive: no sigma schedule, no mutation rescaling, no retry with other
hyperparameters.

**Initialisation**, stated before any evaluation ran: matrix weights `N(0, (gain/sqrt(fan_in))²)`
with `gain = 0.5` — `w_i` at fan-in 70 gives sd 0.059761, `w_h` and `w_o` at fan-in 32 give sd
0.088388. All biases zero **except** `b_hz`, which carries four fixed groups of eight units at
retention timescales `tau = 10, 30, 100, 300` controller updates, `b_hz = logit(exp(-1/tau))`
= 2.25216846104409, 3.3844844191278542, 4.600166019324902, 5.7021153450266. The groups are
fixed, not drawn, so two training seeds start from the same retention structure. No
scripted-forager initialisation of any kind.

### The checks, and what they found

All in `cargo test -p cubarium-search --release` (43 lib tests).

| check | result |
| --- | --- |
| gradient value against the hand-computed formula (n = 2, known ranks) | equal to 1e-15, and halving `sigma` exactly doubles `g` |
| gradient direction on an analytic objective `f(x) = -‖x - c‖²`, 256 pairs | cosine to the true gradient **0.9+**; 60 Adam steps improve `f` |
| equal scores (ties) | utilities equal, `g` **exactly** 0.0 in every element, centre does not move at all |
| mirrored seeds | one `epsilon` per pair; `(theta+ + theta-)/2 = theta` to 1e-15; both signs build worlds with equal `state_hash` |
| Adam continuation | 3 steps uninterrupted == 2 steps, serialize, reload, 1 step — exactly equal `theta` and equal `Adam` |
| tensor round-trip | `flatten(unflatten(theta)) == theta` value for value, both directions |
| finite values | 25 ascent steps stay finite; a non-finite gradient or centre panics as an experiment error, not a low score |
| serial vs parallel | one generation at 1 and at 4 workers: identical scores, identical every-episode results, identical `theta`, identical Adam |

Cancellation was checked too: a generation that does not complete every job returns
`Cancelled`, leaves `theta` untouched and leaves `adam.step` at 0.

**A defect this uncovered.** `serde_json`'s default parser is not correctly rounded: on this
build about 11% of arbitrary `f64` values come back one ULP away from what `serde_json` wrote
(the `float_roundtrip` feature is what fixes parsing, and it is off by default). A checkpoint
whose centre differs in the last bit is not the run that was saved, so every float vector the
trainer persists is written as **little-endian IEEE-754 hex** — which is also exactly the
tensor layout contract §5 already states for weights. `es/bits.rs` holds the encoding and a
test that states the defect. See §8 for what this implies for M1's `replay`.

## 3. The fixtures and the twelve controls

Four training layouts and eight held-out ones, frozen in `es/fixture.rs` **before** any
measurement, each hashed over its canonical text together with the serialized `WorldConfig`.
The split is on record before any policy exists; the held-out set is kept out of optimisation,
of any calibration and of selection, and **was not evaluated** — that belongs to the learning
assignment.

Construction, identical everywhere: every field cell emptied of `P`/`F`/`D`/`De` and booked as
an export, the layout's patches painted back and booked as an import (so the material box
closes and `check_invariants` holds), one mature founder (hue 0.5) at the opening patch's
centre with `structure = structure_adult`, reserve 0.5·R_max, energy 0.75·E_max — usable
stores 2.5 e of a 4.0 e capacity — and births disabled equally through `bud: Some(false)`,
which can only suppress a request and never create one. Weather amplitude 0, rain 0, mutation
off; producer growth, mortality, decomposition, nutrients, handling prices and the type-II
term are the world's ordinary configuration. No floor, no regrowth change, no respawn, no care
replenishment.

### Training layouts (frozen)

| layout | seed | start | heading | cells | opening fill | material | hash |
| --- | ---: | --- | --- | ---: | ---: | ---: | --- |
| `t1-corridor` | 20260915001 | (3, 8) | east | 27 | 0.55 | 34.425 m | `0x047c43d51e9b7380` |
| `t2-weak-open` | 20260915002 | (8, 12) | north | 27 | 0.30 | 31.050 m | `0xfceaa325b66775a4` |
| `t3-scatter` | 20260915003 | (12, 4) | west | 35 | 0.70 | 48.450 m | `0xac3d9cc17ffc9b51` |
| `t4-ring` | 20260915004 | (8, 8) | south-east | 29 | 0.45 | 36.075 m | `0xce6dc279827e527f` |

`t1` is two 3×3 patches due east of a 3×3 opening; `t2` opens weak and puts its food north then
north-west; `t3` opens strong with a single-cell cue and a 5×5 field to the west; `t4` opens
mid-strength with two 3×3 corners and two single-cell cues. Held-out hashes are in
`es-protocol`'s output (`h1`…`h8`, seeds 20260915100–107).

### The twelve controls

`cubarium-search es-controls --workers 2 --wall-seconds 120`, 36,000 ticks each (1,800 s),
**432,000 ticks total, wall time 6.6 s of the 120 s budget**, build
`f716256191ca+unrelated-uncommitted`. Intake is the world's own `IntakeDiagnostics` — material
that actually left a field through a mouth — and the two paid columns are priced from the
**resolved** motion through `MotorBill`, never from a stock delta or a net-energy difference.

| layout | control | ticks | s | alive | stores_end | intake P | upkeep | motion | px | cells |
| --- | --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| t1-corridor | no-intake | 7,420 | 371.0 | no | 0 | 0.0000 | 2.2999 | 0.0000 | 0.0 | 1 |
| t1-corridor | stationary-grazing | 10,978 | 548.9 | no | 0 | 0.8111 | 3.4029 | 0.0000 | 0.0 | 1 |
| t1-corridor | mobile-script | 36,000 | 1800.0 | **yes** | 2.9995 | 8.5158 | 11.1600 | 0.1218 | 260.3 | 48 |
| t2-weak-open | no-intake | 7,420 | 371.0 | no | 0 | 0.0000 | 2.2999 | 0.0000 | 0.0 | 1 |
| t2-weak-open | stationary-grazing | 9,404 | 470.2 | no | 0 | 0.4523 | 2.9149 | 0.0000 | 0.0 | 1 |
| t2-weak-open | mobile-script | 36,000 | 1800.0 | **yes** | 1.3967 | 8.1983 | 11.1600 | 0.9721 | 1959.3 | 44 |
| t3-scatter | no-intake | 7,420 | 371.0 | no | 0 | 0.0000 | 2.2999 | 0.0000 | 0.0 | 1 |
| t3-scatter | stationary-grazing | 11,873 | 593.6 | no | 0 | 1.0151 | 3.6803 | 0.0000 | 0.0 | 1 |
| t3-scatter | mobile-script | 36,000 | 1800.0 | **yes** | 3.0000 | 8.5065 | 11.1600 | 0.1088 | 237.2 | 42 |
| t4-ring | no-intake | 7,420 | 371.0 | no | 0 | 0.0000 | 2.2999 | 0.0000 | 0.0 | 1 |
| t4-ring | stationary-grazing | 10,401 | 520.1 | no | 0 | 0.6796 | 3.2240 | 0.0000 | 0.0 | 1 |
| t4-ring | mobile-script | 36,000 | 1800.0 | **yes** | 3.0002 | 8.5438 | 11.1600 | 0.1593 | 345.0 | 60 |

Behavioural diagnostics for the mobile arm, reported separately and never scored: body-length
displacement 104.1 / 783.7 / 94.9 / 138.0, turn sweep 3,574° / 33,970° / 2,979° / 4,464°,
ticks spent in the opening patch 14,479 / 9,141 / 15,615 / 14,639 of 36,000.

### Verdict per layout

| requirement | t1 | t2 | t3 | t4 |
| --- | --- | --- | --- | --- |
| R1 starting stores alone cannot survive the horizon | pass | pass | pass | pass |
| R2 stationary grazing cannot pass the relocation task | pass | pass | pass | pass |
| R3 a paid mobile strategy can exploit the food | pass | pass | pass | pass |

All four layouts meet the three requirements. Read carefully:

- **R1 is layout-independent, and that is expected.** The no-intake arm dies at tick 7,420
  (371.0 s) on every layout, because the body, the stores and the stillness are identical and
  nothing in the layout reaches a closed mouth. Starting stores buy 371 s of a 1,800 s horizon.
- **R2's margin tracks the opening fill**, which is the sign that the opening patch is what
  kills it: 470 s on the 0.30 opening, 549 s on 0.55, 593 s on 0.70. Even the strongest opening
  cell, grazed continuously with the legacy `feed_min` gate bypassed and the world's own
  regrowth running, funds 593 s of 1,800. Stationary grazing is *funded* while it lasts — it
  eats 0.45–1.02 m — and still dies.
- **R3 is survival plus funding**, not survival alone: the mobile arm ends the horizon holding
  1.40–3.00 e of usable stores, having eaten 8.20–8.54 m through its mouth and paid
  11.16 e of upkeep and 0.11–0.97 e of motion. Three of the four end at the store ceiling; the
  weak-open layout ends at 1.40, which is the one that costs the most travel (1,959 px).
- These are **capability measurements on one genotype, one body, one seed per layout**. They
  say a paid mobile strategy exists on these layouts. They say nothing about whether a learned
  policy will find one, and nothing about world-average ecology.

## 4. The score, frozen before the smoke

```
score = t_min + 0.25 * mean_l clip((E + e_r*R)_l / (E_max + e_r*R_max), 0, 1)
```

`t_min` is the **minimum** survival ticks over the candidate's layouts, so a policy that solves
three and dies at once on the fourth is ordered by the fourth. The secondary term is a mean
over the same layouts with a dead animal contributing exactly zero. The whole secondary term
is at most 0.25 of one tick, so **no store bonus can outweigh one tick of survival**; a test
asserts that 101 ticks with empty stores beats 100 ticks with full ones. The constant was
committed in `2331ae8`, before any smoke ran.

A survival floor here is an evaluation criterion. Nothing is granted to an unfunded body: a
body that cannot pay its upkeep dies under the repaired starvation predicate exactly as it
does anywhere else, and that death is a completed episode with a recorded survival time, not
an error.

Every other number the trainer records — intake, paid upkeep, paid motion, starting and
terminal stores, travelled px, body lengths, distinct cells, ticks in the opening patch, turn
sweep, and the route's producer stock before and after — is a diagnostic and enters no
ordering. No reward shaping, no path-length reward, no mode label, no births, no action
magnitudes.

## 5. The plumbing smoke

`cubarium-search es-smoke`, protocol hash `0x9bef61fd641c6b55` (2 pairs, 2,000 ticks,
`t1-corridor` only). Four episodes, then the **same four** at a different worker count: eight
episodes, 16,000 ticks, **wall time 0.4 s of the 60 s budget**. It exercises ES, not foraging:
2,000 ticks does not outlast the starting reserves, and every candidate survived by
construction.

| candidate | ticks | alive | stores_end | score | intake P | px | cells |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| pair0+ | 2,000 | yes | 1.788178 | 2000.111761 | 0.0296 | 251.8 | 67 |
| pair0− | 2,000 | yes | 1.791694 | 2000.111981 | 0.0339 | 261.9 | 64 |
| pair1+ | 2,000 | yes | 2.370920 | 2000.148182 | 0.2056 | 251.5 | 39 |
| pair1− | 2,000 | yes | 1.859527 | 2000.116220 | 0.0601 | 258.3 | 45 |

The update: gradient L2 norm `5.978362473094e2`, update RMS `9.999999878948e-3` (Adam's first
step moves every element by almost exactly the learning rate, which is what bias correction
does at `t = 1`), centre L2 displacement `1.010692819441e0`.

The repeat at four workers:

| comparison | result |
| --- | --- |
| candidate scores | identical |
| every episode, field for field | identical |
| centre, as little-endian IEEE-754 hex | **byte-identical** |
| Adam moments and step | identical |

`deterministic`.

**What the smoke is not.** It evaluated no held-out layout, it produced no trained founder, and
four survivals on a 2,000-tick horizon are not evidence about foraging. The score spread across
the four candidates is entirely in the store term, because every candidate reached the horizon.

## 6. Persistence and export

The checkpoint holds the exact centre, both Adam moments and the step, the training seed, the
completed generation, the counted episodes and ticks, every centre score, the full protocol,
the protocol hash, the policy schema digest and the build. Perturbations are **positional** —
`epsilon` for `(generation, pair, element)` is a pure function of `(train_seed, generation,
pair, element)` — so the seed together with `generation_completed` *is* the perturbation state;
there is no stream cursor that could be lost. `validate` refuses a wrong dimension, a
non-finite value, a protocol whose hash no longer matches, and a foreign schema digest by name.

Verified end to end through the CLI, at 400 ticks and 2 pairs over all four layouts:

| check | result |
| --- | --- |
| two generations in one run (4 workers) vs one generation, save, reload, one more (2 then 7 workers) | `theta` **equal**, Adam equal, counted work equal |
| the centre evaluation changes nothing | the same `theta` with `--center-eval true` and `false` |
| checkpoint size | 491,724 bytes (three 10,215-element vectors in hex) |
| export round trip | `es-export`: **exact, bit for bit** (compared on `to_bits`) |
| ordinary core inference | the exported policy attached to a fresh `t1-corridor` world, stepped 400 ticks: population 1, still neural, `check_invariants` passes |
| exported policy size | 163,661 bytes |

The export carries weights and provenance only. The optimizer's moments are **not** in it, and
an animal's lifetime `hidden`/`held`/`feedback` is the world's, created fresh at attachment —
optimizer state and lifetime state never mix.

A partial generation writes no checkpoint and updates no centre, so the last saved state is
always the last *completed* generation.

Total artefacts under `runs/es-r2a/`: **1.2 MiB**. No per-tick archive, no retained population
of binaries, one normal build cache.

## 7. Measured compute and the proposed first-learning command

`cubarium-search es-bench --ticks 4000 --workers 8`, on this machine (32 logical cores, load
average about 5 from the running display):

| layout | detail | ticks/s (one worker) |
| --- | --- | ---: |
| t1-corridor | score | 17,423 |
| t2-weak-open | score | 17,942 |
| t3-scatter | score | 17,238 |
| t4-ring | score | 17,411 |
| all four | full diagnostics | 17,234–18,054 |

Per-tick diagnostics cost nothing measurable against a world step, so the campaign runs with
them on or off at the same rate. Sixteen episodes at eight workers: **130,393 ticks/s
aggregate**, 0.245 s per episode — near-linear scaling. A 36,000-tick single-animal episode
therefore costs **2.21 s of one worker** and 0.28 s of wall time at eight workers.

These are single-animal episode timings on the actual fixtures. They are *not* the old
ecosystem-search numbers and not R1a's 512-body throughput screen.

### The budget

| item | episodes | ticks |
| --- | ---: | ---: |
| perturbations: 32 candidates × 4 layouts × 16 generations | 2,048 | 73,728,000 |
| centres: the initial one and every updated one, 17 × 4 | 68 | 2,448,000 |
| **total before any separate evaluation** | **2,116** | **76,176,000** |

At the measured 130,393 ticks/s aggregate, 76,176,000 ticks is **584 s ≈ 9.7 minutes** of wall
time at eight workers — inside the 20-minute cap with about 2× headroom. The estimate is an
*upper* bound on ticks: an episode that dies early costs proportionally less, and early
generations will die early. It would still fit if throughput halved; it would not fit if
throughput fell below about 63,500 ticks/s, so the cap is enforced rather than assumed.

### The exact command

```
cargo run -p cubarium-search --release -- \
    es-train --pairs 16 --generations 16 --horizon 36000 \
             --workers 8 --wall-seconds 1200 --train-seed 20260915 \
             --center-eval true --out runs/es-first
```

Confirmed to parse and to resolve to **protocol hash `0xb69033f65e56f1df`**, the same hash
`es-protocol` prints. `--wall-seconds 1200` is the hard cap: a generation that would cross it
is cancelled inside its episodes, updates nothing and writes nothing, and the run stops at the
last completed generation. Retries are not a separate budget — there are none; a failed
generation is a stopped run.

**This command was not executed.** Nothing about the cap implies the policy will learn.

### Reserved, and deliberately not specified here

The later evaluation budget — selected policies on the eight untouched held-out layouts,
memory-use versus hidden-reset diagnostics, longer horizons and a small shared-arena transfer
check — belongs in the next brief, before execution. R1a's two-history runtime check is a
statement about the interface carrying memory; it is not evidence of learned memory use, and
nothing in R2a adds any.

## 8. Contract corrections

For Fable to apply; the implementation follows these, not the current document.

### §5, "Network (starting proposal)" — the initialisation is fixed groups, not a draw

Replace

> Initialisation (a Cubarium choice, not an imported result): `b_hz` drawn so that `σ(b_hz)`
> spans retention timescales of 1–100 updates, all weights small and non-saturating; verified
> by the two-history check, not assumed.

with

> Initialisation (a Cubarium choice, not an imported result): matrix weights are drawn
> `N(0, (gain/sqrt(fan_in))²)` with `gain = 0.5`, so a gate pre-activation has a width that does
> not depend on the tensor's width and sits well inside the non-saturating part of `σ` and
> `tanh`. Every bias is zero **except** `b_hz`, which is *not* drawn: the 32 units are split into
> four fixed groups of eight at retention timescales `tau = 10, 30, 100, 300` controller
> updates, with `b_hz = logit(exp(-1/tau))`. Fixing the groups rather than sampling them means
> two training seeds start from the same retention structure and differ only in their matrix
> weights. Retention is a starting *structure*, not a verified property: the two-history check
> shows the interface carries memory, and says nothing about a trained policy using it.

### §8, "Evaluation is lineage-based and lifecycle-derived" — name the two evaluations apart

§8 ends "never lineage size alone, and never terminal stores". R2a's training score is
`t_min + 0.25 · mean normalised usable terminal stores` (brief §4), which uses terminal stores
as a bounded tiebreak. The two are not in conflict, but only because they measure different
things, and the document should say which is which. Add after that paragraph:

> **This is the standard for a lineage in a living world.** It is not the fitness of a
> candidate inside an isolated capability episode, where births are disabled, there is no
> lineage to count and the question is whether one body can fund itself. That fitness orders by
> survival time first and may use terminal stores as a bounded tiebreak — bounded so that no
> store bonus can outweigh one tick of survival. A capability score is not evidence about a
> lineage and must never be reported as one.

### Not a contract item: `serde_json`'s parser, and M1's `replay`

Recorded here because it is a real finding and because it touches a command this milestone was
told to leave alone. `serde_json`'s default deserializer is not correctly rounded; measured on
this build, about 11% of arbitrary `f64` values do not survive a write/read round trip. M1's
`cubarium-search replay` reads recorded parameter values back out of `evals.jsonl` and claims
to "re-run one recorded row and check that it reproduces bit for bit". If any searched
parameter's value is one of the affected ones, replay is re-running a *slightly different*
vector than the one recorded. The one-word fix is
`serde_json = { version = "1", features = ["float_roundtrip"] }` in
`crates/cubarium-search/Cargo.toml`. **It was not applied**: the brief requires M1's command
behaviour to remain intact, and this would change what `replay` computes (for the better).
The trainer does not rely on it — every float vector it persists is hex.

### Not a contract item: the build stamp is stale

`crates/cubarium-search/build.rs` declares only `rerun-if-changed=build.rs` and
`rerun-if-env-changed=CUBARIUM_SEARCH_BUILD`, so cargo never re-runs it when `HEAD` moves. On
first build in this session `BUILD_ID` reported `962c82bd7c1a` — a commit from far back in the
history — while `HEAD` was `f716256191ca`. Every `build` field recorded by any M1 run is
therefore only as current as the last time that file or that variable changed, which defeats
the stamp's stated purpose. Again **not fixed here**, for the same ownership reason. The runs
in this document were produced with the documented override,
`CUBARIUM_SEARCH_BUILD="$(git rev-parse --short=12 HEAD)+unrelated-uncommitted"`, so their
recorded build id is true. The `+unrelated-uncommitted` suffix records that the shared checkout
carries unrelated uncommitted work in `.claude/`, `design/handoffs/`, `design/7_Research/assets/`
and `design/*.md` — none of it authored or touched here.

## 9. Tests

| command | result |
| --- | --- |
| `cargo test -p cubarium-search --release` | 55 passed (43 new lib tests + the 12 existing M1 harness tests), 0 failed |
| `cargo test --workspace --release` | **1,327 passed, 0 failed, 20 ignored** (1,286 before R2a) |
| `cargo clippy -p cubarium-search --release --all-targets` | clean; the two remaining warnings are pre-existing in `cubarium-core` |

## 10. What is left, and what is not claimed

- **No policy has been trained and none exists.** Every weight in this milestone is either the
  seeded centre or a perturbation of it. The smoke's centre moved once, by a gradient built
  from four 2,000-tick episodes; that is plumbing, not learning.
- **The held-out set was constructed and hashed but never evaluated**, deliberately. Its
  evaluation, the memory-use diagnostics, longer horizons and the shared-arena transfer check
  are the next brief's, to be specified before execution.
- **The controls prove a paid mobile strategy exists on these four layouts.** They do not show
  that this is the *only* funded strategy, that the layouts are well-graded for learning, or
  that 36,000 ticks is the right horizon. No layout, seed or config search was run, and none is
  proposed.
- **The compute estimate is an upper bound on ticks at one machine's measured throughput**,
  taken while the display was running. It is not a guarantee, and the 20-minute cap is enforced
  by cancellation rather than by the estimate.
- **The display world is untouched.** No policy was attached to it, `state/` was not read or
  written, the runner was not stopped or restarted, and no core file was modified in this
  milestone — the whole change is additive inside `crates/cubarium-search`, so live behaviour
  cannot have changed.
- **Two defects were found and deliberately not fixed** (§8): `serde_json`'s parser under M1's
  `replay`, and the stale build stamp. Both are in M1's scope, both have one-line fixes, and
  both are Fable's call.

## Usage

Measured token usage: unavailable in this harness. Context-counter deltas are not a
billed-usage measurement and are not reported as one.
