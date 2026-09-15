---
design_status: exploration
last_reviewed: 2026-09-14
decision_refs: []
---

# R0b: the corrected motor contract, and what mobile grazing actually pays — 2026-09-14

Evidence for Fable, against [the R0b brief](../handoffs/r0b-opus-2026-09-14.md) and the
[dispatch](../handoffs/post-fable-dispatch-2026-09-14.md). This document decides nothing: no
cropping floor, no coefficient change, no pace change, no population target. It reports a
contract correction, the tests that hold it, and four measured arms with their limitations.

Commits: `99a2bfc` (the envelope), `2822caa` (the diagnostic seam and the measurement).

## 1. The corrected contract

`crates/cubarium-core/src/motor.rs:161-163` (`MotorLimits::capability`) used to return
`speed_cap + REFERENCE_RADIUS_PX · turn_rate_max`. It now returns `speed_cap`.

```
u  = min(capability, affordable) = min(speed_cap, affordable_motor)
     bound:  |v| + r·|ω| ≤ u      and, additionally,  |ω| ≤ turn_rate_max
```

- `speed_cap` is what `world/step.rs:1089-1096` already computed: `effort · speed_max / wading`,
  raised to a burst ceiling if a hunter strike or a threatened prey's dash is on the boost list.
  **That one number is now the whole motor budget.** Effort, morphology, wading and a legitimate
  burst therefore throttle or lift *turning* exactly as they throttle or lift travel.
- `turn_rate_max` is retained as an extra clamp on `|ω|` inside
  `motor::resolve` (`motor.rs:237`) and adds nothing to the budget. A genome that turns slowly
  still turns slowly; a genome that could turn fast cannot buy capability with that fact.
- `affordable` is unchanged (`MotorBill::affordable_motor`): the motor magnitude the energy left
  after upkeep pays for, pricing the whole magnitude at the dearer of its two halves.

What the correction is worth, at the current pace, for a unit adult (`r` = 2.5 px,
`speed_max` = 0.3 px/s, genome ceiling 90 °/s):

| | before R0b | after R0b |
| --- | --- | --- |
| capability | 0.3 + 3.93 = 4.23 px/s | 0.30 px/s |
| full-effort pure pivot | 90 °/s (the genome's) | **6.88 °/s** (`u/r`) |
| Resting (effort 0.05) sweep | 3.93 px/s | **0.0149 px/s** |
| threatened prey's pivot | 240 °/s (escape ceiling) | ≤ 13.8 °/s (`2·speed_max/r`) |
| apex member, `r` ≈ 14.8 px, full effort | 16 °/s | **0.97 °/s** |

Everything else in the slice is untouched: no acceleration state, no contact solver, no neural
ABI; `speed_max` 0.3, `move_cost`, `maintenance`, `sense_cost` and `ROTATION_COST_SCALE` 0.5 all
as shipped. Physical body extent, paid actual motion, heat accounting and unpaid seam transport
are unchanged, and `motor::resolve` is still called exactly once per organism per tick, from
`world/step.rs:1110` — the single boundary every override path (ordinary steering, apex pursuit,
escape, encounter retreat) already funnelled through.

**One consequence worth naming.** The resolver still scales both channels by one common factor
`u / demand`, so a caller that asks for full speed *and* a large turn gets both shrunk. Granted
travel is therefore `u² / demand` — it falls faster than linearly in `u` while the turn request
stays the same. That is why "wading halves the speed" is no longer literally true of travel: at
unit depth travel falls 3.86× while the *budget* halves exactly. The world test now asserts the
budget, which is the honest statement of the rule.

**`ROTATION_COST_SCALE` is no longer an ecological lever.** Sweep can never exceed `speed_cap`,
so the most the rotation price can ever cost is `move_cost · S · k · speed_max · dt` — at most
half of what pure travel already costs. The population figures in
[the R0a cost report](r0a-motor-cost-ecology-2026-09-14.md) (93 at `k = 0` down to 39 at
`k = 1`) describe the legacy controller's turning habit under the inflated envelope and do not
transfer to the corrected one. R0b keeps `k = 0.5` and did not rerun that experiment.

## 2. Coverage and tests

New or reworked, with the reason in each source file:

- `motor.rs` unit tests — pure translation, pure pivot at `u/r` (and half that at twice the
  radius, and nothing at all on a zero budget), simultaneous requests under one common factor,
  a reachable target reached bit for bit, low/zero energy, zero request, degenerate inputs, the
  apex radius, the pre-R0a translation bill. **New:** `an_angular_ceiling_never_enlarges_the_budget`
  (0, 90°/s, 240°/s and 10⁶ rad/s all give the same capability),
  `wading_and_bursts_move_the_whole_budget`, `a_resting_body_barely_sweeps`.
- `tests/motor_foundation.rs` — the world-level envelope over a mixed-size world now asserts
  `|v| + r|ω| ≤ speed_max` (previously `speed_max + 2.5·turn_rate_max`, an addend worth 3.93
  px/s against a 0.3 px/s ceiling, which is why it never bound anything) *and* that every
  turning body's rate is within `speed_max / extent`. The escaping-prey test no longer asserts a
  240 °/s spin — a raised angular ceiling buys nothing now — and instead asserts the escape
  *speed multiple* reaching the envelope (so no Mode label suppresses a legitimate escape) and
  the turn staying inside the boosted budget. The apex regression keeps its bound and adds a
  guard that the pre-R0b rate is now unreachable by more than 10×.
- `tests/diagnostic_seam.rs` (new, 5 tests) — the seam is inert when empty and never reaches a
  snapshot; a scripted heading is bounded by the resolver at `u·dt/r`; a scripted stillness pays
  exactly upkeep and does not move; a scripted graze opens the `feed_min` gate but still takes
  only what the cell holds; a degenerate script cannot break invariants or leave the surface.
- Fixtures widened, with the measured reason recorded in each: starvation horizon 6,000 →
  20,000 ticks, the depth-band walk 200 → 1,600, the two-hop sensing walk 600 → 2,400, the
  quiet-pause first-birth window 20,000 → 60,000 (the first birth moved to step 32,442), and
  the zeroed-stores parent death window 600 → 20,000 (measured 10,708). A body that turns
  travels less, pays a smaller motor bill and lives longer on the same stores; none of the
  claims moved.
- Continuation oracles re-anchored to this build as `*-plus600-r0b.cubw`
  (`tests/continuation_fixtures.rs`, renamed from `r0a_fixtures.rs`), and the two hunter
  observation hashes re-recorded — exactly what R0a did for the same reason. **Snapshot
  continuity is preserved and no migration was needed:** the schema is unchanged, every genuine
  historical fixture still decodes and migrates, and `a_saved_world_resumes_identically_through_paid_pivots`
  still passes. The resolver is memoryless and reads only persisted state.

Also fixed on the way: `cargo build --release` was broken on `main` since `962c82b` —
`world/step.rs` imported the debug-only `stored_energy` unconditionally. The release binary,
the release examples and `scripts/run-cube.sh` could not build before this.

### Commands actually run

```text
cargo test --workspace --release --no-fail-fast   →  1229 passed, 0 failed
cargo test -p cubarium-core --release             →   393 passed, 0 failed (29 binaries)
cargo test -p cubarium-core --release --test motor_foundation    → 7 passed, 0 failed
cargo test -p cubarium-core --release --test diagnostic_seam     → 5 passed, 0 failed
cargo test -p cubarium-core --lib motor                          → 14 passed, 0 failed
cargo clippy --workspace --release --all-targets  →  no new warnings from R0b files
cargo run --release -p cubarium-core --example mobile_grazing    → 5.1 s wall
```

The workspace figure includes `crates/cubarium`'s presentation and replay suites — art motion,
growth packs, wind, water, hunter presentation, care replay, the web mirror, run persistence,
`run_headless`, `run_present`, `shim_sink` — all green.

## 3. The diagnostic seam

`crate::diagnostic::ScriptedIntent` substitutes part of one organism's decided intent for one
tick: heading, effort, the three intake efforts, the mode label, and whether to request
gestation. It lives on `World`, never on `WorldState`, so it is not serialized, migrated or
resumed; the world never sets one; and an empty list — every ordinary world — costs one
`is_empty` test per tick and moves nothing (asserted bit for bit over 400 ticks, snapshot
included). It states an *intent*: the heading still goes through `motor::resolve`, the intake
efforts still go through the per-cell proportional share, the type-II term and reserve headroom,
and gestation still faces the capacity and funding checks. It is applied before the hunter,
escape and encounter passes, so a legitimate override still beats a script.

## 4. Measured: can physical travel pay?

`crates/cubarium-core/examples/mobile_grazing.rs`. One unit adult on a matched 3×3 patch of the
top face (nine 4 px cells, `P` 3.3892 m standing at `t = 0`, nutrients 4.50, no rain, no weather
swing, mutation off, every other cell in the world emptied and booked out so world totals are
patch totals). Births disabled equally in all four arms through the seam. Four arms, 36,000
ticks (1,800 s) each. Leave rule: hold a cell until its `P` < 0.20 m — the world's own
`feed_min`, so the script leaves where the legacy gate would have closed. The tour is the eight
border cells as a closed ring of neighbours; the centre cell is never visited and is therefore a
matched undisturbed control inside the same patch.

| arm | s | eaten_P (m) | grown (m) | Δ store (e) | upkeep (e) | travel (e) | rotate (e) | px | cell transitions | body turns | survival |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| gated-still | 460 | 0.4035 | 4.1338 | −2.5000 | 2.8486 | 0 | 0 | 0 | 0 | 0 | died 9190 |
| open-still | 1800 | 0.4237 | 15.8805 | −2.5000 | 11.1600 | 0 | 0 | 0 | 0 | 0 | broke 9278 |
| **mobile** | 1800 | **5.6531** | 7.0475 | −2.5000 | 11.1600 | 0.94777 | 0.29588 | 157.96 | 52 | 6.28 | broke 28199 |
| no-intake | 350 | 0 | 3.2855 | −2.5000 | 2.1669 | 0.11067 | 0.02233 | 18.45 | 6 | 0.47 | died 6991 |

Units: `m` material, `e` energy, world seconds (20 ticks = 1 s). `broke T` is the first tick the
usable store (`E + e_r·R`) could no longer pay one tick of upkeep; `died T` is the world's own
`E ≤ 0 && R ≤ 0`. See the censoring note below for why they differ.

**Mobile grazing pays, and by a wide margin, until the patch runs out.** The mobile grazer took
**14× the material** the legacy-gated stationary one did (5.65 m against 0.40 m). It filled its
reserve within the first tour and then sat at its store ceiling — `E` 1.0, `R` 1.0, usable 3.0 e
against a 2.5 e start — for roughly 900 s, refusing 2.8% of its own requests because it was
full. That is not a marginal cycle; it is a body that cannot store what it can gather. The
stationary reference starved at 460 s, which is what R0a measured, re-measured here under the
corrected envelope.

**The motor is not the expensive part.** Over 1,800 s the mobile arm paid 0.948 e for 157.96 px
of travel and 0.296 e for 6.28 full body rotations, against 11.160 e of upkeep: **motion is 10%
of the whole bill.** At this pace a body's problem is standing still, not moving.

Per tour of the eight-cell ring:

| tour | s | ate (m) | Δ usable store (e) |
| ---: | ---: | ---: | ---: |
| 1 | 538 | 2.7890 | +0.5001 (censored from above: store at its ceiling) |
| 2 | 253 | 1.2948 | −0.0001 (censored from above) |
| 3 | 133 | 0.7411 | −0.0132 |
| 4 | 133 | 0.4783 | −0.4600 |
| 5 | 133 | 0.2235 | −0.8933 |
| 6 | 133 | 0.0943 | −1.0168 |

Tours 1 and 2 are censored from above — the animal was full, so the store delta understates the
yield. The decline from tour 3 on is the patch emptying, not the cycle's cost: the tour time
collapses from 538 s to 133 s because cells are already below the leave threshold on arrival, so
the grazer walks without dwelling.

**The continuous request is the thing that strips the patch.** With the behavioural gate
removed, the mobile arm took the whole 3×3 patch from 3.3892 m to 0.6041 m while gross renewal
over the same window was only 7.05 m, and individual cells were driven to ~10⁻³ m and below.
Logistic regrowth is proportional to `P`, so a cell taken to near zero barely comes back. The
never-visited centre cell went 0.3908 → 0.5426 m in the same world, which is what an undisturbed
cell does. The gated stationary arm instead pinned its own cell at 0.1998 m and the patch as a
whole *grew* (3.3892 → 4.3824 m) — the legacy gate is currently the only thing stopping a mouth.

**Recovery** (all eight estimates censored by the grazer's own return):

| cell | left at `P` | s away | on return |
| --- | ---: | ---: | ---: |
| (7,7) | 0.1999 | 539 | 0.3979 |
| (8,7) | 0.1996 | 585 | 0.2659 |
| (9,7) | 0.1998 | 592 | 0.2668 |
| (9,8) | 0.1996 | 507 | 0.2704 |
| (9,9) | 0.1999 | 475 | 0.2237 |
| (8,9) | 0.1998 | 410 | 0.2330 |
| (7,9) | 0.1998 | 334 | 0.2287 |
| (7,8) | 0.1999 | 252 | 0.2041 |

Only the first cell recovered materially (0.20 → 0.40 m in 539 s); the rest recovered little,
because the patch's nutrients and light are shared and the grazer was cropping its neighbours
the whole time. Every row stops recovering the moment the mouth arrives, so all eight are lower
bounds. No extra recovery arm was run.

**The no-intake control** replayed the mobile arm's recorded per-tick intent with every intake
effort at zero and died at tick 6,991 (350 s). Its *resolved* motion diverges from the mobile
arm's once its energy budget binds (18.45 px against 157.96 px), which is the honest limit of
the match: the intent schedule is identical tick for tick, the delivered motion is not. 350 s is
what 2.5 e of starting store buys at this activity level, against the 460 s the motionless gated
arm managed — so the whole scripted tour costs about 110 s of life, and bought 14× the food.

### A pre-existing defect this exposed

In both ungated arms the body goes broke and then does **not** die. Intake is settled before the
death check, so a cell stripped to ~10⁻²⁰ m still serves an infinitesimally small bite each
tick, and that bite is the only energy the body holds when `E ≤ 0 && R ≤ 0` is evaluated. The
result is a body funding itself on a geometrically shrinking trickle for the rest of the run
(`open-still` reaches `E` = 5.5e-57 and is still "alive"). **This is a property of the existing
world, not of the motor correction** — the `feed_min` gate normally prevents it by refusing to
open on a cell that poor. R0b measures it and changes nothing; a cropping floor was explicitly
out of scope. It is named here because it is exactly the failure mode F2 predicted for a policy
with no reason to stop chewing, and because any survival statistic taken from an ungated fixture
has to use `broke`, not `died`.

### Cost and limits of the measurement

5.1 s of wall time against the 120 s cap, one worker, one seed, one patch, one light level, one
genotype, no births, no fixture retuning and no extra arms. The example polls the budget every
tick and stops with the shortfall named. Not measured: world-average gross production, any other
light level or nutrient condition, fruit or detritus as channels (`F` and `D_eff` never opened),
anything after a body starved in its arm, and whether any of this is sustainable.

## 5. Native-size development observation

`./scripts/run-cube.sh` with normal `assets/atelier`, `--sink shim --mirror-web --web-port 7393
--fps 60 --speed 1 --care`. `pgrep -af cubarium` before launching showed no runner; the shim
(`led-cube-shim run`, pid 1446219) was already up and was not touched. **No world reset:** the
runner resumed `state/world-115200.cubw` at tick 115,200, which `/status` confirms
(`"resumed_from"`, `"start_tick": 115200`, `"build_id": "0.1.0+2822caa"`).

**What was actually inspected** — rendered frames pulled from the live web mirror's `/frame`
(raw 5 × 64 × 64 RGB, the same bytes the shim receives), decoded and viewed:

1. The whole cube as a five-face cross at native 64×64, nearest-neighbour upscaled. The world
   renders correctly under the corrected envelope: strata, water band, canopy, reeds, soil.
2. A 60 s strip (8 samples) of a 24 px window on the Front face: an umbrellafrond canopy
   visibly grows and sways. Frame-to-frame change averaged 7,495 of 20,480 pixels per 0.5 s, so
   the display is not static.
3. A 3 × 4 grid at 16× zoom of one fauna body on the Front face, sampled every 8 s over 88 s.
   This is the travel/pivot/rest observation. The body is about 3 px across. Over the 88 s it
   moved a few pixels down and to the right, its orientation changed gradually across several
   tiles, and there are runs of tiles where it barely changes at all. **No turn ever appeared
   faster than travel.** A centroid track over 118 s put the net displacement at ~3 px; the
   per-sample path length is not a usable speed estimate because plant sway contaminates the
   residual, so the pace numbers in §4 are the ones to quote.

**Pace consequences.** `/status` gave `render_seq` 3,462 → 7,071 in 60 s = 60.15 fps, and
`world_tick` 116,355 → 117,558 = 20.05 ticks/s: the display holds 60 fps at real-time speed.
What changed is legibility of motion, not frame rate. At 64×64 a unit adult is ~3 px and its
maximum pivot is 6.88 °/s — a half turn takes 26 s — so a turn is now a sub-pixel sprite change
over tens of seconds rather than something an observer notices. Travel is 0.3 px/s at best, one
4 px cell per 13 s. Nothing in the scene is wrong; it is slower and quieter.

**Legacy apex pursuit is degraded, as expected.** In `motor_foundation`'s trial fixture the
member spends its time `Stalking` with a target already inside its grasp envelope, where the
pursuit override sets `hold` and runs at `rest_effort`. Under the shared budget that is
0.05 × 0.25 / 14.8 = 8.5e-4 rad/s ≈ 0.049 °/s, so facing the prey would take about an hour of
world time and the member never reaches `Windup`. The previous envelope hid this by letting a
*resting* member spin at 0.28 rad/s. The hunter suite still passes (28 tests in `hunter.rs`, 17
in `hunter_charging.rs`, contact, funding and windup assertions intact) because those fixtures
stage contact rather than rely on the heuristic aligning itself; the timing fixture that broke
was `motor_foundation`'s "turned hard" assertion, and it was re-anchored to the rate the
member's *actual* effort buys plus a guard that the pre-R0b rate is unreachable. **No runtime
timeout was changed and no hunting heuristic was added.** The legacy heuristic's deadlock —
holding at rest effort while needing budget to turn — is a real design problem for R3 to solve
by learning approach-based alignment, not by restoring free rotation.

**The ordinary world's population fell during the observation.** Over 13,900 live ticks
(695 s at speed 1) the resumed world went from 94 organisms to 50, with zero births and
starvation the only cause, while producer stock *rose* from 333 to 508. The legacy controller
turns constantly and now pays for it in travel it does not get, so it forages worse and never
reaches the budding thresholds. A matched controlled version of the same effect is in the test
suite: on the same saved fixture, the first birth moved from inside 20,000 ticks to step 32,442.
**This is reported, not gated on** — the dispatch is explicit that no population outcome
qualifies the motor correction, and no coefficient was tuned in response.

## 6. Development runner status

Running. `target/release/cubarium run` pid 1646454, build `0.1.0+2822caa`, sink `shim`, web
mirror on 7393, resumed from `state/world-115200.cubw`, speed 1, 60 fps, `--care` on. It is the
current checkout with the corrected envelope, using normal assets, and it was left running.

## 7. What was deliberately not done

- No cropping floor, no stubble, no change to producer/regrowth rules, cell resolution, handling
  prices or any speed/upkeep/movement/rotation coefficient.
- No rerun of the six-run population experiment, no extra seeds, no extra arms, no fixture
  tuning after a result.
- No pace change, and no proposal of one — but see §5: whether 0.3 px/s and 6.88 °/s is the pace
  Wrysk wants to watch is now a live question, and it is Fable's to put.
- No sensors, no RNN, no training, no schema change, no edits to M1, the recurrent plan, or
  `design/recurrent-interface-contract.md`.
- The starvation-predicate defect in §4 is reported, not fixed: fixing it means deciding what a
  mouth may take from an empty cell, which is the floor question the dispatch deferred.

## 8. Usage

The harness exposes a remaining-context counter, which fell from 15,000,000 to about 14,650,000
tokens over this assignment — roughly 350 k of context including cached re-reads. Billed token
usage is **unavailable**.
