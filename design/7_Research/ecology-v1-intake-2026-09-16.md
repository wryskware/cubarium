---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Why generation 9 does not eat: the per-tick intake diagnostic

Workstream H of the ecology v1 next steps
([brief](../handoffs/ecology-v1-intake-opus-2026-09-16.md)), step 1 of the reconciled next
steps and Astra's "single most informative cheap experiment now".

The feasibility experiment localised the trained controller's failure to *intake* rather than
travel ([budget note](ecology-v1-budget-2026-09-16.md)): generation 9 visits five times as many
cells as the surviving mobile control, travels 1.6 times as far, and takes in fifteen times
less material. It could not say why, because a life total cannot tell a shut mouth from an
unfed one. Three explanations were left standing and they imply three different next moves:

- **(a)** its mouth efforts are off when it stands on edible food — a score or action problem;
- **(b)** it is rarely on edible food at all — an observation or navigation problem;
- **(c)** efforts are on, food is present, and the served bite is clamped — an adapter or
  settlement problem.

All three are now measured at the sites the tick decides them, per tick, for three drivers on
twelve layouts. **The answer is (b), and it is not close.** (a) and (c) are refused by the
measurement outright, and (b) turns out to be sharper than "it cannot find food": generation 9
*finds* the food — it stands on 23 of the 27 food cells of `t1-corridor` — and then leaves,
every time, after about ninety ticks, with the cell still above the world's own feeding
threshold and its own mouth still open.

## Build and provenance

- Core diagnostic and its tests: `52fe697`.
- The experiment and its regressions: `a8292c7`.
- Clippy cleanups and the three-arm cost measurement: `1bb5f04`.
- The artifact was produced by search build id **`1bb5f04b1303-dirty`**, i.e. the tree at
  `1bb5f04`. The `-dirty` is other workers' uncommitted design documents in the shared
  checkout, not source. (The stamp had to be pinned explicitly with `CUBARIUM_SEARCH_BUILD`:
  three workers share one `target/`, and the build script's `HEAD` watch had baked another
  worktree's commit into an earlier run. Three runs of the experiment — two before the
  cleanups, one after — agree to every printed digit; only the stamp differs.)
- Ecology: `runs/ecology-v1-calibration/selected/fast-leaf.toml`, config hash
  `09e244392ec91768` — the hash the generation-9 policy records, so `PolicyFile::check_ecology`
  accepts it. Policy: `runs/es-eco-v1-fastleaf/selected/center-00009-policy.json`.
- Outputs are under `runs/ecology-v1-intake/`, which is git-ignored by design; the tables below
  carry what they say and the command below reproduces them.
- Wall time: **5.0 s** of simulation (36 episodes of 36,000 ticks, 8 workers) against the
  brief's 6-minute cap. Storage **23 MiB** against a 40 MiB cap: a 2.0 MiB aggregate JSON plus
  six per-tick CSV files (1.4–7.4 MiB each).

## What the trace records, and which contract section each line implements

`cubarium_core::IntakeTick` is one row per tick for **one** body, opened before the tick's first
mutation, filled by the motor settlement and the feeding settlement, and closed after the tick
commits. Nothing is reconstructed afterwards.

| Line | What it is | Contract |
| --- | --- | --- |
| `cell` | the cell the body occupied at the feeding settlement, i.e. **after** this tick's move | §6.2 |
| `stock[4]` | that cell's `P`, `F`, `D_eff`, `C_eff` before a single transfer | §3.1, §6.2, §9 |
| `above_threshold[4]`, `feed_threshold` | whether each stock is at or above `drives.feed_min` | `drives` |
| `effort[3]` | `graze_effort`, `fruit_effort`, `scavenge_effort` as the `Decision` carried them | §3–4 |
| `effort_normalised[3]` | the same after the world's one-mouth normalisation (`/Σ` when `Σ > 1`) | §6.3 |
| `raw_head` | the linear head **before** squash, deadband, mask and normalisation, for a neural body on a tick its controller ran | §3 |
| `saturation[3]`, `mouth_bite[3]` | `food/(food + K_P)` and `mouth_rate · effort · dt · S` | §6.3 |
| `requested[3]` | the same after the reserve-headroom clamp: what the cell was actually asked for | §6.4 |
| `served[4]`, `digestible[4]`, `reserve_credit[4]`, `battery_credit[4]` | `q`, `q_d = cap·q`, `η_m′·q_d`, and the charge after `η_e` and the `E_max` clamp | §6.4 |
| `battery_rejected[4]` | the charge the `E_max` clamp sent to heat instead of the battery | §6.4 |
| `reserve_headroom`, `energy_headroom`, `reserve`, `energy` | the stores the settlement started from | §6.4 |
| `bill_total`, `bill_paid` | this tick's `MotorBill::total_cost` and what the body raised | §7, motor contract |
| `limit[3]` | the one term that decided each mouth's bite | below |

**The limiting term** is evaluated in a fixed order and the *first* that applies is recorded,
because each earlier term makes every later one vacuous:

1. `capability` — the phenotype has no machinery for this food (`cap_foliage` or `cap_detrital`
   is zero), so the world refuses the channel whatever asked for it (§6.2).
2. `effort_zero` — the decoded effort is zero. For a neural body that is the adapter's deadband
   or its capability mask; for a scripted one it is the script.
3. `below_threshold` — the mouth was open and the cell was bare: the stock this mouth serves is
   under `drives.feed_min`. **This describes the cell; it gates nothing.** A stand below the
   threshold still serves a crumb, and the term still names the stock, because at that density
   the mouth's own rate is not what is costing the body. A hand-built tick pins exactly that.
4. `reserve_room` — `R_max − R`, less whatever an earlier-settling mouth took of it, clamped
   the bite below what the mouth rate would have taken.
5. `stock_share` — the proportional share of a contested or nearly empty stock delivered less
   than the request.
6. `mouth_rate` — nothing else bound it: the bite is exactly `mouth_rate · effort · dt · S(food)`.

The `E_max` clamp is deliberately **not** in that list: it limits the *credit*, never the bite,
so it is carried separately as `battery_rejected`. It is zero on all 36 episodes.

The detrital effort serves two stocks, so the efforts and the limits are three wide while the
stocks and the credits are four wide. `IntakeTick::mouth_of` and `served_by` are the mapping.

**Not persisted, not hashed, opt-in.** The rows live on `World`, not on `WorldState`; the
snapshot schema is unchanged and a reloaded world traces nobody. Recording is off by default and
opted into with `World::trace_intake(Some(id))`, one body at a time. An undrained recorder keeps
at most 4,096 rows and counts what it dropped.

**Cost**, measured best-of-five alternating A/B/C on the shipped-defaults whole world
(46–255 bodies, one of them traced, everything drained every tick). Two runs, because this
machine carries three workers and one shared `target/` and a single set of timings is dominated
by whatever else is compiling:

| | off | E's per-body ledger alone | the ledger **and** the trace |
| --- | --- | --- | --- |
| run 1 | 9,161 ticks/s | 9,060 (−1.10 %) | 9,053 (−1.17 % vs off, −0.07 % vs the ledger) |
| run 2 | 9,610 ticks/s | 9,601 (−0.09 %) | 9,571 (−0.40 % vs off, −0.31 % vs the ledger) |

The spread between the two runs is larger than either arm's effect, so the honest reading is a
**bound, not a point estimate**: the trace and the ledger together cost at most about 1.2 % of
tick throughput on this machine, and the trace's own marginal cost is not resolvable at this
precision. Both are far inside the brief's 3 % ceiling. With the trace off, every site is one
`Option` test and nothing is allocated. Reproduce with
`cargo test -p cubarium-core --release --test intake_trace -- --ignored --nocapture`.

## Exact commands

```bash
cargo test -p cubarium-core --test intake_trace
cargo test -p cubarium-search --test intake_diagnostic

CUBARIUM_SEARCH_BUILD=1bb5f04b1303-dirty \
  cargo test -p cubarium-search --release --test intake_diagnostic -- --ignored --nocapture
```

The experiment's entry point is an `#[ignore]`d test rather than an `es-*` subcommand. That is a
routine choice, recorded here: three other workers held `crates/cubarium-search/src/main.rs`
open in worktrees for the duration, and a subcommand is the one edit that would have collided.
`es::intake::run` is public, so promoting it to `es-intake` later is a one-liner. It reads its
defaults from the environment, so Fable can re-run one row without editing anything:

```bash
CUBARIUM_INTAKE_HORIZON=36000 CUBARIUM_INTAKE_WORKERS=1 \
CUBARIUM_INTAKE_PER_TICK=t1-corridor \
CUBARIUM_INTAKE_OUT=runs/ecology-v1-intake/recheck.json \
  cargo test -p cubarium-search --release --test intake_diagnostic -- --ignored --nocapture
```

## The experiment

`fast-leaf`, the 4 training and 8 held-out layouts, one episode each, horizon 36,000, three
drivers, the trace and the ledger on. Medians over the twelve layouts, with `[min–max]`.

| driver | alive | ticks | on food | mouth open \| food (>0.05) | (>0.5) | open on bare ground | served / requested | credit / bill |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| mobile-script | 11 / 12 | 36,000 | **0.952** [0.794–0.993] | **0.989** [0.638–0.991] | 0.989 | 0.048 | **1.000000** | 1.026 |
| initial centre (20260915) | 0 / 12 | 7,240 | **0.046** [0.007–0.127] | **1.000** [1.000–1.000] | 0.000 | 0.954 | **1.000000** | 0.116 |
| generation 9 | 0 / 12 | 8,421 | **0.083** [0.044–0.118] | **1.000** [1.000–1.000] | 0.000 | 0.917 | **1.000000** | 0.241 |

"On food" is a tick on a cell holding any of the four stocks at or above `drives.feed_min = 0.2`.
"Mouth open | food" is the fraction of *those* ticks on which the mouth serving one of the
above-threshold stocks was open. Pooled over all twelve layouts rather than per-layout medians:
generation 9 is on food on **8,365 of 100,101** ticks (0.0836) and its matching mouth is open on
**8,365 of 8,365** of them — every one.

The limiting term for the **grazing** mouth, summed over the twelve layouts:

| driver | capability | effort zero | below threshold | reserve room | stock share | mouth rate |
| --- | --- | --- | --- | --- | --- | --- |
| mobile-script | 0 | 0 | 50,603 (12.0 %) | **349,359 (82.7 %)** | 0 | 22,393 (5.3 %) |
| initial centre | 0 | 0 | **84,249 (94.9 %)** | 0 | 0 | 4,548 (5.1 %) |
| generation 9 | 0 | 0 | **91,736 (91.6 %)** | 0 | 0 | 8,365 (8.4 %) |

The same, restricted to ticks on which that mouth's own food was above the threshold:

| driver | capability | effort zero | below threshold | reserve room | stock share | mouth rate |
| --- | --- | --- | --- | --- | --- | --- |
| mobile-script | 0 | 0 | 0 | 349,359 (94.0 %) | 0 | 22,393 (6.0 %) |
| initial centre | 0 | 0 | 0 | 0 | 0 | **4,548 (100.0 %)** |
| generation 9 | 0 | 0 | 0 | 0 | 0 | **8,365 (100.0 %)** |

Served material by channel, summed over the twelve layouts (m) — identical to the feasibility
experiment's table to every printed digit, which is an independent check that the trace did not
move the episode:

| driver | foliage | fruit | litter | carrion |
| --- | --- | --- | --- | --- |
| mobile-script | 141.1307 | 0 | 0 | 0 |
| initial centre | 2.3855 | 1.2604 | 1.1189 | 0 |
| generation 9 | 4.1830 | 2.5981 | 2.6524 | 0 |

## The three-way verdict

### (a) "its mouth efforts are off when it stands on edible food" — **refuted**

On **8,365 of 8,365** ticks that generation 9 stood on a cell with something edible in it, the
mouth that serves that food was open above the action adapter's own deadband. Not one tick in
100,101 recorded `effort_zero`, on any of the three mouths, on either policy. The initial centre
is the same: 4,548 of 4,548. The mouths are never shut.

The `> 0.5` column is 0.000 for both policies, and that is worth stating precisely rather than
reading as a second version of (a). Both policies' three efforts sum to **exactly 1.0000 on
every tick** (range `[1.0000, 1.0000]` over 9,429 ticks of `t1-corridor` and 8,560 of
`h1-holdout`), because the adapter's shared-mouth normalisation divides three sigmoids that each
clear 1/3. Generation 9's grazing effort sits at **0.323**, and over 9,429 ticks it never leaves
`[0.3186, 0.3374]` — a span of 2 % of its own value. The mouth is not off; it is a **constant**,
and it is one third. Nothing forbids the policy from pushing graze to 0.9 and the other two to
0.05 — the normalisation only bites when the sum exceeds one — but it never does, on any tick,
on any layout.

That costs a factor of about three on the bite, against the mobile script's effort of 1.0, and
two thirds of the shared mouth is spent on fruit and detrital channels that hold nothing above
the threshold on 94.5 % and 94.7 % of ticks. It is real, it is measured, and it is **not** the
binding term: three times 0.83 m is 2.5 m against the control's 12.2 m.

### (c) "the served bite is clamped" — **refuted**

`Σ served / Σ requested` is **1.000000000** for all three drivers, pooled over all twelve
layouts. Across all **611,253** traced ticks, not one asked for something and was served
nothing. `stock_share` is zero everywhere, on every mouth, for both policies; so is
`reserve_room`; so is `capability`; and `battery_rejected` is exactly 0 on all 36 episodes. On
every tick on which either policy stood on its own food, the limiting term was `mouth_rate` —
the contract's own `mouth_rate · effort · dt · S(food)` — **100.0 % of the time** (8,365 of
8,365 for generation 9, 4,548 of 4,548 for the initial centre). There is no clamp anywhere in
generation 9's intake path. Nothing is being taken away from it.

(The one driver that *is* clamped is the control that survives: the mobile script is
`reserve_room`-limited on 94.0 % of its on-food ticks, which is a body whose reserve is full
standing on food it cannot fit — the saturated steady state the feasibility note described.)

### (b) "it is rarely on edible food at all" — **the answer**

Generation 9 is on food **8.3 %** of its ticks against the surviving control's **95.2 %** — an
11.4× gap in the medians, and the control is ahead on every one of the twelve layouts, by
between 8.5× (`t1-corridor`) and 18.0× (`h2-holdout`). That, and the shorter life it causes, is
the whole 15× intake gap:

```text
served        = (ticks alive) · (fraction on food) · (served per on-food tick)
generation 9  =      8,421    ·       0.0832       ·      1.135e-3   ~= 0.83 m
mobile script =     36,000    ·       0.9518       ·      3.515e-4   ~= 12.21 m
```

(medians of the twelve layouts, so the products are the right size rather than exact.)
Note the third factor: generation 9 gets **3.2× more material per on-food tick** than the
control, because it stands on undepleted stands (median foliage under it 0.65 against the
control's 0.35, saturation 0.58 against 0.43) and eats from three channels instead of one. It is
out-earning the control by 3.2× per tick of contact and losing by 48.9× on contact time (701
on-food ticks against 34,265), which is the 15× net.

**And the failure is residence, not detection.** From the per-tick rows of the two layouts
written out in full:

| | generation 9, t1-corridor | generation 9, h1-holdout | mobile script, t1-corridor |
| --- | --- | --- | --- |
| food cells painted (Top face) | 27 of 256 | 27 of 256 | 27 of 256 |
| distinct cells visited | 290 (163 Top, 127 elsewhere) | 251 (139, 112) | 44 (44, 0) |
| food cells actually stood on | **23 of 27** | **21 of 27** | 22 of 27 |
| fraction of ticks on food | **0.116** | **0.085** | 0.985 |
| fraction of life on faces with no food painted | **0.370** | **0.374** | 0.000 |
| ticks per food cell / per bare cell | **47.6 / 31.2 = 1.53×** | **34.5 / 34.1 = 1.01×** | 1,612.5 / 23.9 = 67× |
| on food, given it is on the Top face | 0.184 (null 0.141) | 0.135 (null 0.151) | 0.985 (null 0.500) |
| visits to food | 13 | 9 | 11 |
| median dwell per visit | **93 ticks (4.6 s)** | **93 ticks (4.6 s)** | 2,299 ticks (115 s) |
| foliage at arrival → departure | 0.611 → 0.584 | 0.603 → 0.602 | 0.568 → 0.200 |
| departures with the cell still above `feed_min` | **13 / 13** | **9 / 9** | 7 / 11 |

The "null" on the seventh row is what an unbiased tour of the Top cells that body actually
visited would give: the food cells it stood on, over the Top cells it stood on. Generation 9
beats it by 1.3× on `t1-corridor` and misses it by 0.9× on `h1-holdout`; the residence ratio in
the row above says the same thing from the other side. A preference that small is not a
foraging behaviour.

Generation 9 walks over 23 of the 27 food cells. It is not blind to them and it is not failing
to reach them. It stays on a food cell 1.53× as long as on a bare one on `t1-corridor` and
**1.01×** — the exact null — on `h1-holdout`, against the control's 67×. Every one of its
twenty-two departures from food happened with the cell still above the threshold, having removed
about 4 % of the stand it was standing on. It also spends 37 % of its life on side faces that
hold no food painted on them at all.

The control's number for the same statistic is 1,612 ticks per food cell against 23.9 — a 67×
preference. That is the behaviour generation 9 does not have, and it is the only one it is
missing.

**Is it the observation?** No — and this is worth saying plainly, because the brief's branch (b)
named the observation as the thing to change. The 70-scalar layout
(`crates/cubarium-core/src/neural/obs.rs`) already carries exactly this signal:

- `v[0] = P_here / P_max`, `v[1] = F_here / P_max`, `v[2] = D_eff_here / P_max` — the own cell's
  three stocks, which is literally "am I standing on food";
- `v[3..21]` and `v[21..39]` — the near and far sensing rings, six body-frame sectors × three
  food channels, as weighted means;
- `v[39..51]` bodies, `v[51..53]` crowd, `v[53..58]` water/light/height/up, `v[58..63]` stores
  and age, `v[63]` motor availability, `v[64..70]` the six feedback channels, which include what
  the body *ate* on the last interval.

Every one of those is finite, bounded and present on every tick. The policy is shown its own
cell's food and is shown what its last bites yielded. It is standing on a stand of 0.6 with
`v[0] = 0.4` and walking off it. **The observation does not lack the signal; the policy does not
use it.**

## What this does not establish

- **Nothing about a whole world's foraging.** Twelve frozen single-body layouts on one ecology.
- **Nothing about why the policy ignores `v[0]`.** The trace shows that it does; it does not
  show whether the weights are near-degenerate, whether the GRU's recurrence is saturated, or
  whether the search simply has not had enough updates. A policy-side probe — hold the body
  still and sweep `p_here`, and watch the head move or not — would settle that and is not in
  this workstream.
- **Nothing about the score's gradient directly.** The claim below that the score is the thing
  to change is an inference from measured facts (the mouth is never the binding term, the bite
  is never clamped, residence is at the null), not a measurement of the score's landscape.
- **Nothing about the mobile script's own limits.** It is `reserve_room`-limited on 94 % of its
  on-food ticks, which means these twelve layouts do not test a controller's ability to *find*
  more food once it is full; they test whether it stays put.
- **Nothing about a longer horizon or a different ecology.** One horizon, one config hash, three
  named drivers.
- `cargo clippy -p cubarium-core --all-targets` still fails on the pre-existing `erasing_op`
  deny in `crates/cubarium-core/src/neural/gru.rs:233`, untouched by this workstream.

## The next task this implies

Named, not launched, and named as one thing rather than four.

**A score change.** `score = t_min + 0.25 · mean normalised terminal stores`
(`es::trainer::score`) is the wrong shape for the behaviour that is missing. `t_min` is the
minimum survival ticks over the layouts and a dead animal contributes zero stores, so for a
policy that dies on every layout the score is *only* survival time — and survival time responds
to an extra hundred ticks of grazing only after that material has passed through the reserve,
the oxidation threshold and the bill. The gradient toward "stand still on this cell" is
therefore many thousands of ticks downstream of the action that earns it, and sixteen antithetic
updates cannot climb it. The evidence that the score, and not the observation, the adapter or
the settlement, is the binding constraint is the three measured refusals above: the mouth is
open on 100 % of on-food ticks so the adapter is not withholding the request; the cell serves
100 % of what is asked so the settlement is not withholding the bite; the observation carries
`P_here` on every tick so the policy is not blind. What is left is that nothing the search scores
pays for staying.

The ledger this campaign already built measures the replacement exactly and for free: per body,
per life, `income = Σ battery_credit + η_ox · e_r · Σ reserve_credit` against
`bill = bill_total + …`, which is dense, accumulates from the first bite, and is above 1 exactly
when the body could pay for itself. A scored term on that — or, more simply, on served material
per tick alive — gives the same sixteen updates a signal on every tick of contact instead of one
that only settles at death. Whether it should *replace* `t_min` or join it is a design call for
Fable, not this workstream.

Two secondary items, both worth recording and neither the next task:

- **An adapter note.** Both policies saturate the shared mouth at `Σ effort = 1.0000` on every
  tick and split it evenly, so two thirds of a generalist's mouth is spent on channels that hold
  nothing 94 % of the time. Worth about 3× on the bite, which does not close a 48.9× contact
  deficit. It becomes worth doing *after* residence is fixed, not before.
- **A fixture note.** Generation 9 spends 37 % of its life on side faces with no food painted on
  them at all, and 127 of the 290 distinct cells it visits on `t1-corridor` are off the Top
  face. The layouts paint the Top face only, and nothing in the fixture or the score discourages
  leaving it. That is a property of the task, not of the controller, and it is why the
  "distinct cells" column of the feasibility table is larger than one face.
