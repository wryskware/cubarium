---
design_status: exploration
last_reviewed: 2026-09-14
---

# Result: bounded real-core search prototype (milestone 1)

Answers `design/handoffs/ecology-search-2026-09-14.md`. Nothing here is a decision or a
balance claim. The measurements are real; the ecology conclusions are one screen, not a result.

**Bounded assignment finished. The goal — parameters that sustain a varied, changing
ecosystem — is not finished, and the smoke budget could not have finished it.**

## What shipped

`crates/cubarium-search` — a new workspace member. No viewer, host, control or live-parameter
code was touched; the only edits outside the new crate are the workspace `members` list, the
workspace dependency entry and the resulting `Cargo.lock` line.

| module | what it owns |
| --- | --- |
| `params.rs` | the 13-parameter joint vector, its bounds, the round trip, and `EXCLUDED` — every candidate identified and deliberately left out, with the reason |
| `evaluate.rs` | one candidate × one seed on a real `World`, to a hard tick horizon; component metrics; panic and invariant capture |
| `metrics.rs` | the component vector, the seven objectives, `Scoring` (the declared reference scales), and the scalar rank |
| `search.rs` | deterministic init, binary tournament, uniform crossover, bounded Gaussian mutation, elitism, fixed seed schedule, and every hard limit |
| `rng.rs` | the search's own counter-based draws, keyed by `(stream, key, counter)` so a generation never depends on which worker finished first |

The world is the real one: `World::new` + `World::step`, the shipped `WorldConfig` defaults,
24 ordinary founders of four kinds that move, feed, steer and reproduce, real initial `N`/`P`/
`D`/`De` stocks, real weather and water. `examples/apex_screen.rs`'s stationary juvenile prey
are **not** reused: nothing in this harness freezes an organism or zeroes a maintenance cost.
The apex cohort is the only staging — two adults placed once through the core's own
`introduce_hunters`, with `material_in` / `energy_in` recorded on every row, and never
restocked.

Candidate configuration and inherited genomes stay apart: the harness only ever writes into a
`WorldConfig` and a `FixedHunterProfile`. Organism genomes are inherited and mutated by the
core, under the core's rules, and are never touched here.

### Tests — 14, all passing

`cargo test -p cubarium-search` (6.7 s). Every world they build runs under the core's own
`debug_assertions` per-tick invariant, energy-audit and water-budget assertions.

- the same `(candidate, seed)` reproduces bit for bit; a different seed and a different
  candidate each give a different `ecology_hash`
- a search reproduces from its `search_seed`, and its rows come out in
  `(generation, candidate, seed)` order whatever the workers did
- the default vector round-trips, sits inside its own box, and *is* the shipped world
- `drives.bud_reserve = 0.55` is **recorded as `Invalid` with the core's own reason**
  (`child material … exceeds the conception reserve`) and the value goes through unrepaired
- every protocol, budget and variation limit refuses what it cannot enforce, before any work
- the evaluation cap is the number of simulations actually run; the candidate the budget never
  reached is reported unevaluated, not dropped
- the wall clock stops a search between evaluations, and says so
- mass, energy and water residuals stay at rounding; the apex cohort books its import
- the scalar rank cannot hide a collapsed, idle, monoculture or dormant-only world
- dominance is a partial order over all seven objectives
- a recorded row replays from its exact bits, and a malformed bit vector is refused

**One defect, found by the harness checking itself.** `replay` originally rebuilt a candidate
from the row's readable decimals, and it diverged on a recorded row: `serde_json` 1.0.151
returned `fruit.ripen` one ULP away from the value that had been written
(`0.009362229560426979` out, `0.00936222956042698` back). One ULP on a growth rate is a
different world, and the divergence was real. The simulation was never at fault — the same
vector evaluated single-threaded and across four threads gives the same `ecology_hash` every
time. Every row now carries `param_bits` (the exact IEEE-754 patterns, in `PARAMS` order) and
`param_fingerprint`; `replay` reads those and refuses a row whose fingerprint does not match
what it reconstructs. All ten smoke rows now print `REPRODUCED`. The readable decimals are kept
for reading and are explicitly not authoritative. This is the milestone's one repair cycle.

## Measured throughput, memory and storage

Release build on this host (Ryzen 9 9950X3D, 16C/32T; 91 GiB RAM). Two builds are quoted:
`abbb051` is the core as handed off, and `6309655+cap` is the same core with the
`habitat::cap` fast rejection described under "One core optimization" below — a change that is
bit-identical, so the two columns time *exactly the same worlds*.

**Single world, default parameters, seed 1001, two apex founders**

| horizon | ticks/s `abbb051` | ticks/s `6309655+cap` | gain | peak RSS attributable |
| --- | --- | --- | --- | --- |
| 2 000 ticks (100 sim s) | 11 330 | 14 626 | 1.29× | 1.5 MiB |
| 120 000 ticks (100 sim min) | 6 848 | 7 571 | 1.11× | 1.6 MiB |

The gain is larger at small populations because the optimization is in the fixed per-tick cost,
not the per-organism cost.

Fitting the two points gives a per-tick cost of roughly **63 µs fixed + 0.97 µs per organism**.
The fixed part is the 1 280-cell field step and weather; it already dominates a 26-organism
tick. That number matters for the GPU decision below.

**Independent-world CPU batching, 20 000 ticks per world**

| workers | worlds | ticks/s `abbb051` | ticks/s `6309655+cap` | gain | peak RSS |
| --- | --- | --- | --- | --- | --- |
| 1 | 8 | 8 603 | 10 427 | 1.21× | 5.4 MiB |
| 4 | 8 | 30 052 | 35 773 | 1.19× | 7.2 MiB |
| 8 | 8 | 59 487 | 70 603 | 1.19× | 10.7 MiB |
| 16 | 16 | 99 428 | 139 110 | 1.40× | 17.0 MiB |
| 32 | 32 | 139 733 | 184 824 | 1.32× | 28.4 MiB |

Scaling is 6.8× at 8 workers, 13.3× at 16, 17.7× at 32 — the last doubling is SMT, not cores.

**The reference number for budgeting: 32 workers × 32 worlds × 120 000 ticks**

26.4 s wall, **145 732 ticks/s aggregate** (7 287× real time; 113 691 before the `cap` change),
31.2 MiB peak RSS for the whole process. All 32 completed, none collapsed, worst residuals
`mass 1.1e-10`, `energy 7.4e-11`, `water 2.9e-10`.

**Storage added:** ~106 MiB of build artifacts in the one normal `target/` cache, 48 KiB of
results, 128 KiB of source. `target/` is 10.8 GiB, unchanged from the ~11 GiB at handoff. No
alternate cache, no capture archive, no frozen binary. Well inside the 1 GiB checkpoint.

## The default world at a horizon where it is alive

Not a search result — the default-parameter baseline, 32 seeds × 120 000 ticks. It is the
reference every candidate is read against, and it quantifies the stated inadequacy.

Medians over the 32 seeds unless noted.

- the ecology runs: 371 prey births and 297 deaths, 64% of births reach adult structure, 47%
  of those reproduce, lineage depth 9, 11 of 24 founder lines still alive at the end,
  population 26 → 96, feeding fraction 0.50, standing crop 0.175 of capacity, gross production
  1.37 capacities/hour
- **the apex hunts and still does not persist: 209 paid attempts and 54 captures per seed,
  1 676 captures in total — and 0 apex births in every one of the 32 seeds, with an apex alive
  at the end in only 2 of them.** The introduced cohort is consumed and never replaced.
  (Seed 1001 is a quiet outlier at 64 attempts and 6 captures; both its adults starve.)
- and the reason is narrower than "the apex is underfed": across all 32 seeds there were
  **zero matings**. The apex is active in 58% of samples and eats well; it simply never pairs.
  Two adults placed on random faces of a five-face cube, with `encounter::MATING_RADIUS_PX`
  = 10 px, do not meet inside 100 simulated minutes. 30 of 32 seeds lost both adults, 2 lost
  one. No offspring existed, so dormancy never fired either (`emergences` and `exhausted`
  both 0 everywhere). That is a hypothesis this baseline supports, not a diagnosis — but it
  points at the encounter constants, not at the two apex parameters this vector can reach.
- objective medians: `persistence` 1.00, `maturation` 0.98, `lineage` 0.94, `prey_turnover`
  0.90, `plants` 0.89, `variety` 0.64, **`apex` 0.10**
- scalar rank: median **0.631**, range 0.561–0.943; seed 1001 scores 0.603

So the deficit is specific and measurable, and it is *not* that the predator cannot catch
anything: it catches plenty and the lineage still never closes. `Scoring`'s reference scales
are calibrated once against this baseline (seed 1001) and are not retuned per search.

## One core optimization (asked for after the baseline, applied and verified)

`crates/cubarium-core/src/habitat.rs`. Profiling the 60 000-tick baseline put ~31% of the
runtime in scalar libm `acos`/`cos`/`atan2`, most of it reached through `Weather::sample`, which
evaluates `cap()` for 1 280 cells × 6 blobs = 7 680 times per tick.

`cap` rejected a cell with `acos(dot) > radius`. `acos` is monotone decreasing on `[-1, 1]`, so
that test is exactly `dot < cos(radius)`, and `radius` is fixed for a whole sample. The rejection
is now that comparison, with `cos(radius)` hoisted out of the cell loop; the `acos` test is kept
as the authority for any cell the cheap test admits.

**It is bit-identical, and that is tested three ways.** `habitat.rs` gained
`the_dot_product_rejection_is_bit_identical_to_the_acos_definition`, which compares against a
local copy of the unoptimized definition over a 40 001-point sweep of the dot-product domain at
seven radii, and then walks 64 ulp either side of each rim — the only place the two tests could
ever disagree, and where the raised cosine is quadratically flat and worth about `1e-33` either
way. End to end: all ten smoke rows recorded by the *pre-change* build still print `REPRODUCED`,
and all 32 worlds of the 120 000-tick batched baseline come back with every metric and both
hashes unchanged. The full `cubarium-core` suite (151 unit tests plus every integration suite)
passes, and the host binary still builds.

Measured gain: **1.28–1.40×** on the batched configurations, 1.29× on a small-population world,
1.11× at 120 000 ticks where organism work dominates. Both throughput tables above are quoted
before and after.

### The fast-approximation follow-up: built, measured, reverted

Post-change profile: `Fields::react` 22.4%, `Weather::sample` 12.1%, `water::step` 10.6%,
libm `cos` 9.7% + `acos` 8.6% + `atan2` 2.8% = 21.1%, seam geometry
(`unfold_with` + `chord_sq` + `segment_is_valid`) 17.5%.

The obvious next step was a polynomial `cap`, and it was built properly: a degree-8 Chebyshev
`acos` (`sqrt(1−x)·P(x)`, worst error `3.6e-9`) composed with the cap shape factored as
`(1−t)(1 + ½ t h(t))` from `cos(π√t) = 1 − 2t + t(1−t)h(t)`, which makes the centre exactly 1
and the rim exactly 0 whatever the fit does. Worst absolute `cap` error `2.1e-9` over radii from
1° to 180° — four thousand times tighter than the quick Abramowitz–Stegun version, and pure
arithmetic, so identical across platforms and `libm` versions.

**It was reverted, for two measured reasons.**

*It is worth almost nothing.* A microbenchmark said 2.44 ns/call → 1.03 ns/call, and I
extrapolated ~1.2×. That extrapolation was wrong: it swept the dot-product domain uniformly, so
it exercised the expensive path far more often than a real tick does. Measured in place, after
the exact rejection has already removed four calls in five:

| config | original | exact rejection | + polynomial |
| --- | --- | --- | --- |
| 32 workers × 32 worlds, 20 000 ticks | 139 733 | 185 379 | 190 802 (**+3%**) |
| 32 workers × 32 worlds, 120 000 ticks | 113 691 | 145 732 | 149 005 (**+2%**) |
| single world, 2 000 ticks | 11 330 | 14 626 | 13 533 (**−7%**) |

The two optimizations are not additive: the cheap rejection already took the volume, and what
the polynomial replaces is the fifth of calls that remain. On a small world it is a net loss.

*It costs something irreplaceable.* Changing the weather forcing by even `1e-9` fails **seven**
tests, in five binaries:

```
astra_quiet_policy   genuine_schema12_off_plain_and_care_continue_as_recorded
care                 zero_care_reproduces_the_pre_change_binarys_next_600_ticks
care_dose_migration  a_standard_dose_reproduces_the_pre_dose_binarys_next_600_ticks
care_dose_migration  a_genuine_schema_eleven_hunter_world_migrates_and_continues_exactly
energy_correction    zero_corrections_reproduce_the_pre_correction_binarys_next_600_ticks
hunter_charging      a_version_three_member_reproduces_the_pre_change_binarys_next_600_ticks
hunter_migration     an_empty_extension_reproduces_the_pre_hunter_binarys_next_600_ticks
```

These replay `.cubw` snapshots that **earlier binaries wrote**, step them 600 ticks, and require
the result to match a recorded continuation — one per schema extension, the standing proof that
care, care doses, the energy correction, the hunter, hunter charging and quiet were each added
without perturbing worlds that already existed. `fixtures/*-provenance.md` records isolated
trees and binary SHA256s precisely so nobody can regenerate them from the current build;
regenerating would turn a real cross-build guarantee into the current build agreeing with
itself, and those trees are gone. So the fixtures cannot be refreshed, only retired.

Two to three percent is not worth retiring that. The exact rejection keeps all seven green,
which is why it is the change that shipped.

The remaining `atan2` (2.8%, `Vec2::screen_angle`) and the seam geometry sit under the same
constraint: anything that moves a result by one ulp costs the same seven tests. The honest
headroom on this core, without retiring them, is close to exhausted.

## Sensitivity: the metrics are noisier than one seed can show

Default world, seed 1001, 60 000 ticks, perturbing `fruit.ripen` only:

| | ripen | births | deaths | matured | apex captures | fitness |
| --- | --- | --- | --- | --- | --- | --- |
| base | 0.0200000000000000004 | 188 | 122 | 103 | 6 | 0.5973 |
| +1 ulp | 0.0200000000000000039 | 187 | 120 | 101 | 6 | 0.5944 |
| +1e-6 relative | 0.0200000200000000003 | 194 | 132 | 115 | **25** | 0.6339 |

A `1.7e-16` change gives a completely different `ecology_hash` and a statistically identical
ecology. A one-part-per-million change swings apex captures 6 → 25 — that is chaos, not a
mechanism, and it means **single-seed apex metrics carry noise comparable to a real parameter
effect**. Two consequences:

1. Bit-exactness buys *replay identity*, not ecological validity. An `f32` world would be a
   different draw from the same distribution, not a degraded one — but see the audits below.
2. The follow-up needs **more seeds per candidate than the three originally proposed**. The
   proposal below is raised to six. Throughput is not the binding constraint; seed count is,
   and no amount of further optimization changes that.

### On dropping to `f32`

The trajectory would not care. The audits would. `crates/cubarium-core/src/accounting.rs`
records that naive `+=` on `heat_out_total` already fails the cumulative energy audit **at
`f64`** after twelve simulated hours — measured `2.0344e-5` against a `1.9564e-5` limit — which
is why Kahan–Babuška–Neumaier compensation was added and persisted in schema 9. `f32` has about
`2^29` times less headroom; that ledger would fail in seconds and compensation could not rescue
it. `WORKING_POLICY.md` asks specifically that ecological decline not hide behind passing
numerical audits, and a residual tolerance five orders of magnitude looser is exactly where a
slow material leak would live. Recommendation: keep `f64`.

## The smoke search — exactly the handoff's caps

```
./target/release/cubarium-search search --label smoke \
  --evaluations 8 --ticks 2000 --workers 4 --wall-seconds 60 \
  --population 4 --elite 1 --generations 3 --seeds 1 --apex 2 \
  --search-seed 20260914 --out runs/ecology-search
```

The 60 s cutoff is informed by the baseline: 8 × 2 000 ticks over 4 workers is ~0.4 s of work,
so 60 s is a ~150× guard, not a schedule.

Stopped on `Evaluations` after 3 generations, **8 simulations**, 2 cached elite reuses, 0.52 s
wall, 30 860 ticks/s. All 8 completed; residuals ≤ 2.7e-12. Nondominated: candidates 1, 2, 4, 8.

**Read it as a tooling check and nothing else.** At 2 000 ticks the world has lived 100
simulated seconds. Nine of ten rows have zero births, zero deaths, zero attacks and zero
captures; `prey_turnover`, `maturation`, `lineage` and `apex` are at or near the scoring floor
for every candidate. The one candidate that scored above the others did so because mutation put
`drives.bud_min_age_seconds` at 48.8 s — below the horizon — so its founders could bud at all.
That is a horizon artifact, not an ecology. A candidate that wins a 100-second screen has
demonstrated nothing about sustainability, and this run is not evidence for any parameter value.

Results: `runs/ecology-search/smoke/evals.jsonl` (10 rows) and `summary.json`, 48 KiB total.

## Exact replay commands

```
# the searched box and everything deliberately excluded from it
./target/release/cubarium-search params

# single-world baseline
./target/release/cubarium-search baseline --ticks 120000 --sample-every 200 --seed 1001 --apex 2

# the batched reference measurement
./target/release/cubarium-search baseline --ticks 120000 --sample-every 500 \
  --workers 32 --worlds 32 --seed 1001 --apex 2

# the smoke search, byte for byte (deterministic in --search-seed)
./target/release/cubarium-search search --label smoke \
  --evaluations 8 --ticks 2000 --workers 4 --wall-seconds 60 \
  --population 4 --elite 1 --generations 3 --seeds 1 --apex 2 \
  --search-seed 20260914 --out runs/ecology-search

# re-run any recorded row and check its ecology hash
./target/release/cubarium-search replay --record runs/ecology-search/smoke/evals.jsonl --index 0

cargo test -p cubarium-search
```

`replay --index 0` on the shipped rows prints `REPRODUCED`.

## GPU decision: do not build one

**Recommendation: no GPU backend, now or as a follow-up, unless the requirement changes from
"faster search" to "a different, cheaper model".** This is a measured conclusion, not a
preference.

**1. The arithmetic this core uses is the arithmetic this GPU is worst at.** The world is `f64`
end to end — fields, organism state, the energy and mass identities, and the Neumaier
compensated ledgers in `accounting.rs`, whose whole purpose is `f64` rounding control. NVIDIA's
own compute-capability table gives compute capability **12.x an FP32:FP64 throughput ratio of
64:1**; `nvidia-smi` reports this card as compute capability **12.0**. At ~105 TFLOPS FP32 that
is ~1.6 TFLOPS FP64 — *below* this host's own 16-core AVX-512 FP64 peak. A faithful port has
no headroom to win back. An `f32` port would be a different model, which the handoff already
rules out calling acceleration.

**2. One world is far too small to fill a launch.** A tick is ten ordered phases over 1 280
cells and (today) tens of organisms, at ~63 µs + ~1 µs/organism. Ten kernel launches at a few
µs each would cost as much as the whole CPU tick. The only parallel axis is many independent
worlds — and to beat the measured 145 732 ticks/s the GPU would need thousands of worlds
resident and stepping in lockstep, every one of them running the FP64 path above.

**3. The hot paths are branchy and order-dependent, not arithmetic.** Phase 4 unfolds seams
exactly (`pairs.rs` → `cubarium_surface::unfold_with`) with documented vertex tie rules and
sorts bounded neighbour lists. Phase 5 is a deeply gated controller. Phase 7 settles feeding
once per cell, proportionally, from pre-transfer fields. Phase 9 removes the dead and then
places births into a lowest-slot-first free-list arena — a sequential commit that defines
organism identity. Reproducing that ordering, and the ledgers' summation order, bit-exactly
under parallel reduction is the hard part of the port, and it is not the part a GPU helps with.

**4. The one GPU-friendly piece is already there and costs nothing.** `rng.rs` is counter-based
(`draw(seed, stream, key, counter)`), stateless and order-independent. That is genuinely
kernel-shaped — and it is not where the time goes.

**5. The toolchain would itself need a checkpoint.** No CUDA toolkit is installed (`nvcc` absent;
only `libcuda.so` from the driver). Installing one is several GiB, past the storage limit this
milestone was given. Vulkan 1.4 compute *is* available with no install (`libvulkan`, `glslc`,
`spirv-as`, and the device reports `shaderFloat64 = true`) — so if a port ever happened, that is
the path that needs no new storage. It would still be the 64:1 FP64 path.

**6. And it is not needed.** Search is not compute-bound here. A 500-evaluation × 120 000-tick
campaign is 60 M ticks ≈ **7 minutes** on 32 CPU workers. The binding constraint on this
project is horizon and seed count, which the CPU already affords, not throughput.

If a GPU milestone is ever wanted anyway, the first kernel/state boundary is phases 2–3 (weather
→ light/moisture → field reactions and `N` diffusion): a pure per-cell stencil over
`Fields::{n,p,d,de,f,w}` with no organisms, no seam unfolding and no arena mutation, whose
equivalence test is a per-cell `f64` comparison of the field arrays plus the light/heat ledger
deltas against a CPU run of the same seed for 1 000 ticks. That boundary is worth naming, and
by the measurement above it is worth about 63 µs of a tick — which is why the recommendation is
still no.

## Proposed follow-up (separately capped — not started)

One milestone, the screening run the smoke could not be:

- **horizon 120 000 ticks** (100 simulated minutes), the shortest horizon at which the measured
  default world has real births, maturation, lineages and apex attempts
- **6 training seeds** from `TRAINING_SEEDS` — raised from three by the sensitivity result
  above — **500 evaluations**, population 12, elite 2, 8 generations, **32 workers**, wall-time
  cutoff **1 800 s** (measured cost ≈ 410 s, so ~4.4×)
- retest only the nondominated set on the **held-out** seeds `HELDOUT_SEEDS` and one longer
  horizon, in a second, separately capped step — and report it as a retest, never as the search
- expected output: ~500 rows, well under 10 MiB

```
./target/release/cubarium-search search --label screen-120k \
  --evaluations 500 --ticks 120000 --sample-every 500 --workers 32 \
  --wall-seconds 1800 --population 12 --elite 2 --generations 8 --seeds 6 \
  --apex 2 --search-seed 20260915 --out runs/ecology-search
```

Two things to decide before running it, both out of scope here:

1. **The apex constants are the apex's main knobs and are not searchable.** `dormancy.rs` and
   `encounter.rs` carry them as `pub const` (`SUSTAIN_TICKS`, `PREY_REQUIRED`,
   `MAINTENANCE_PER_STRUCTURE_SECOND`, `EMERGENCE_*`, `MATING_RADIUS_PX`, …). The measured
   deficit is that the apex lineage never closes — zero matings in 32 seeds — and the levers on
   that are exactly these. Making them searchable means a persisted policy config and a
   snapshot schema bump — a decision for Wrysk, not a thing to do inside a search milestone.
   This is sharper than it looks: `introduce_hunters` always installs `PairedV1`
   (`world.rs:3216`), and under that policy solo apex reproduction is switched off outright —
   `crates/cubarium-core/src/world.rs:2313-2319` returns `bud = false` for any member while
   `apex_encounters_on`. So the *only* route to an apex birth through the spawn control is a
   mating, and no parameter in this vector moves either the 10 px encounter radius or where the
   two adults are placed. A screening run today would therefore search prey ecology with the
   apex objective near its floor, improving it only by chance meetings.
2. **`hunter.{reproduce_min_age_seconds, reproduce_interval_seconds}` are 1 200 s / 1 800 s.**
   Even a 100-minute horizon only spans a few apex reproduction intervals. Either the horizon
   goes up again or those two enter the vector; a short run cannot score them as they stand.

A short successful run would still be a screening result. It cannot demonstrate indefinite
sustainability, and this document does not claim any candidate does.

## Explicitly not done

No full workspace rebuild, no model-review loop, no capture archive, no frozen executable, no
background campaign, no live deployment, no unrelated cleanup, no seed or horizon sweep to
obtain a prettier smoke result, and no edits to viewer or control code. The one core edit
(`habitat::cap`) was made at Wrysk's explicit request after the spawn-control thread landed
as `6309655`, is bit-identical, and is covered by a new test. One repair cycle was
spent: the replay round-trip defect above, found by the tool and fixed with a regression test.
(Three of my own wrong test expectations were corrected before the suite first went green;
those were authoring, not repair.)

**Claude usage:** this harness does not expose a token counter to the session, so no measured
figure is available. Wall-clock compute spent on simulation was about 2 minutes: the 120 000-
tick single-world baseline (17.5 s), the batching sweep (34 s total), the 32-seed batched
reference (33.8 s), and three smoke searches at 0.5 s each.
