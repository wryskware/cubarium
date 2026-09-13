---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Post-intake opportunity shadow: implementation and four-arm pilot

Retention update, 2026-09-13: this isolated study is not deployed. Raw captures
and its worktree were removed at Wrysk's request; tagged source and saved review
probes remain. The [current handoff](../handoffs/02-quiet-habits.md) identifies the
unresolved review boundary. Do not resume commands against the old input paths.

Implements and runs exactly the check
[root authorised](root-post-intake-shadow-scope-2026-09-13.md): a bounded,
non-intervening opportunity measurement built from
[Astra's proposal](astra-post-intake-quiet-proposal-2026-09-13.md) as corrected by
[Fable's review](fable-post-intake-quiet-review-2026-09-13.md). **Evidence, not a
decision.** No active rest policy, no schema change, no ecological retuning, no
canon promotion, no default change. The sparse birth policy stays Off and every
world here ran `QuietPolicy::Off` with no hunters.

Root's full twelve-by-two ten-minute shadow is **not** authorised and was not run.
This is the four-arm pilot root asked to review first.

## Where the work is

Isolated worktree and branch `captures/diagnostic-source/post-intake-shadow-2026-09-13`,
from **9fc29eb**, the frozen quiet-comparison baseline. Three commits, nothing pushed:

| commit | what |
| --- | --- |
| `0effd34` | `crates/cubarium-core/src/post_intake.rs`, the rule and its transient state, plus three read-only hooks in `world.rs` |
| `ac5975b` | `crates/cubarium/examples/post_intake_shadow.rs`, the pilot harness |
| `33e78b8` | the per-form attribution correction found by the reduction, plus `scripts/post-intake-shadow.mjs` and its tests |

A compact patch of all three is
[assets/post-intake-shadow-instrumentation-2026-09-13.patch](assets/post-intake-shadow-instrumentation-2026-09-13.patch)
(4017 added lines, six files, no deletions). Nothing in `main`, the live state, the
sail art, the quiet policy/default/schema, or the fauna diagnostic branch was touched.

## What was built

**Core (`post_intake.rs`).** A transient `Option<PostIntakeShadow>` on `World` —
absent from every snapshot, hash, decision, stock, draw, transport and mode, in
exactly the sense `ChargingDiagnostics` and the quiet records already are. Three
hooks, all read-only:

1. the settlement stage sums the actual `to_reserve` transfers it applies, per
   channel, and hands them to the shadow. Not a `fed` flag, not a reserve
   difference, not a lifetime total;
2. the **last** thing each organism's physiology iteration does — after its own
   oxidation, growth, gestation/funding and death checks — evaluates one attempt.
   Nothing is reordered and no child that tick already paid for is delayed;
3. the commit stage, after every removal path, reconciles the per-ID map against
   the arena.

The quota is root's diet-permitted channel sum with the controller's own
`DIET_GATE`/`FRUIT_DIET` and the world's own `η_m`; `Budget::of`/`affordable` are
reused unchanged over 30 decisions plus the existing safety tick. Entries are keyed
by full generational `OrganismId`, so a reused slot inherits no credit and no
cooldown; they are dropped at removal and when credit, window and deadline are all
absent; the map may never exceed the world's live organism capacity.

**Harness (`post_intake_shadow.rs`).** Four arms: seeds 1 and 8 × no-care and one
Standard Feed (elapsed 600, Front (32, 48), dose 1000), 12000 elapsed ticks from the
retained tick-144000 openings, census every 200 ticks — the retained quiet
comparison's own cadence and recipe.

**Reduction (`scripts/post-intake-shadow.mjs`).** Recomputes every counter, per-form
table and distinct-ID summary from `shadow-events.jsonl` and replays the rule per
full ID as a state machine. 19 core unit tests, 10 harness tests, 18 reduction tests;
the whole 84-binary workspace suite passes.

## What the pilot proved before reporting a number

Every arm passed all three gates.

* **Observer neutrality, exactly.** Each arm stepped an uninstrumented twin from the
  identical opening in lockstep and compared the whole persisted `WorldState` and
  both record streams after every one of the 12000 ticks. Zero disagreements; the
  two closing snapshots are byte-identical. This is equality, not a small residual.
* **Retained Off continuation, exactly.** Each arm's closing state equals the
  corresponding `captures/quiet-ten-minute-provenance-2026-09-13` Off arm's, decoded
  from that run's own `closing.cubw` and compared as a whole state — including state
  hash, ecology hash, population, births and the death table. The snapshot *file*
  checksum deliberately is not the test: a header records the build that wrote it,
  and this build is not that one.
* **Conservation.** Worst raw energy drift 2.6e-7 against a 1.04e-5 limit, material
  1.8e-11 against 1.63e-5, water 4.7e-11 against 2.5e-7; corrected energy 5.5e-12 and
  independent windowed energy 5.1e-11. The limits are the unchanged
  `1e-8 · max(opening inventory, 1)` rule, re-derived by the reduction from each arm's
  own opening rather than read from its summary.

## Results

Output: `captures/post-intake-shadow-pilot-2026-09-13-v2` (90 MB, exclusive
directory, frozen binary `4a84d78…` copied in, both provenance manifests copied byte
for byte, source hashes recorded by the process that ran). Reduction copied to
[assets/post-intake-shadow-pilot-reduction-2026-09-13.json](assets/post-intake-shadow-pilot-reduction-2026-09-13.json);
it passes with **zero** problems and zero rule violations on all four arms.

| arm | attempts | admitted | released | aborted | censored | distinct attempting IDs | distinct admitted IDs |
| --- | --- | --- | --- | --- | --- | --- | --- |
| seed 1 no-care | 814 | 353 | 350 | 1 | 2 | 114 | 60 |
| seed 1 feed | 791 | 343 | 340 | 1 | 2 | 112 | 61 |
| seed 8 no-care | 699 | 264 | 264 | 0 | 0 | 94 | 46 |
| seed 8 feed | 706 | 259 | 256 | 1 | 2 | 98 | 47 |

Repeat attempts are preserved in the event stream and summarised beside the distinct
counts, never in place of them; the gestation de-duplication root asked for is a
distinct `(ID, gestation start)` count (21, 19, 12, 13).

**Refusals**, repeats included, by reason:

| arm | juvenile | unaffordable | gestating | baseline_escrow (abort) | run_end (censor) |
| --- | --- | --- | --- | --- | --- |
| seed 1 no-care | 311 | 129 | 21 | 1 | 2 |
| seed 1 feed | 297 | 132 | 19 | 1 | 2 |
| seed 8 no-care | 298 | 125 | 12 | 0 | 0 |
| seed 8 feed | 316 | 118 | 13 | 1 | 2 |

`dead`, `hunter_member`, `inactive_mode`, `invalid_inputs`, `overflow`, `bounded`,
`unaffordable_remaining`, `baseline_gone` and `baseline_juvenile` are zero in all
four arms and are printed as zeros, not omitted.

**Joint per-ID quantities at admission** (medians over admitted attempts):

| arm | reserve fraction | energy fraction | energy headroom over budget | material headroom |
| --- | --- | --- | --- | --- |
| seed 1 no-care | .332 (.017–.687) | .500 | .993 (min .609) | .326 (min .00125) |
| seed 1 feed | .321 (.016–.693) | .500 | .993 (min .568) | .314 (min .00010) |
| seed 8 no-care | .293 (.016–.678) | .500 | .993 (min .247) | .273 (min .00077) |
| seed 8 feed | .251 (.016–.699) | .500 | .993 (min .247) | .236 (min .00036) |

**Per form**, with the absent forms' zeros visible. Median episode duration to `Q`,
in simulated seconds:

| arm | form 0 | form 1 | form 2 | form 3 | forms 4–7 |
| --- | --- | --- | --- | --- | --- |
| seed 1 no-care | 10.60 (n 184) | 10.20 (n 164) | 30.40 (n 449) | 12.25 (n 17) | absent, 0 |
| seed 1 feed | 10.65 (n 176) | 10.00 (n 153) | 30.55 (n 444) | 13.10 (n 18) | absent, 0 |
| seed 8 no-care | 10.50 (n 190) | 10.25 (n 89) | 29.95 (n 420) | absent, 0 | absent, 0 |
| seed 8 feed | 10.20 (n 201) | 10.05 (n 82) | 29.95 (n 423) | absent, 0 | absent, 0 |

Four forms contributed admissions in seed 1, three in seed 8.

## What these numbers mean, and what they do not

* **A window is baseline-compatible window time, not a pause.** The animal was never
  held. A released window means the freely feeding baseline stayed compatible with
  holding 30 decisions throughout, checked at every boundary. It is not rest, not an
  upper bound on an intervention, and not evidence that a pause would have occurred.
* **Baseline intake during a window is not missed food.** Released windows carried a
  median of 30 intake ticks out of 30 and a median of ~.006 material — the baseline
  ate through essentially every window. That is recorded separately, exactly as root
  required, and is *not* the food a pause would have cost: a real pause changes the
  patch, the neighbours' shares, the position and every later stock in either
  direction.
* **Admission opportunity, baseline-compatible window length and native readability
  are three different things.** This run measures the first two. It says nothing
  about the third. The conservative arithmetic remainder — 1.5 s of window minus the
  presenter's .55 s settle-plus-fade — is .95 s and is reported as arithmetic on
  presenter constants, not as an observed native-64 transition. The 1.5 s timer was
  not lengthened.
* **Root's usefulness screen is not evaluated.** It needs ≥.2 % eligible-adult
  exposure and ≥15 % whole-world compatible-window time in 9 of 12 no-care seeds. Two
  seeds cannot evaluate that. The inputs are recorded: compatible decisions per
  eligible-adult tick ran .0148–.0162, and the whole-world compatible-window fraction
  .479–.587. Both exceed root's figures in these four arms; neither is a pass,
  because the screen is a twelve-seed screen and its thresholds are unvalidated
  anyway.

## Three findings root may want before authorising more

1. **The quota's "two seconds" is nowhere near two seconds of real eating.** Median
   time from an episode's first positive settlement to the one that reaches `Q` is
   ~10 s for forms 0, 1 and 3 and ~30 s for form 2, with a maximum of 109.6 s. Form 2
   assimilates on almost every tick of its episode (median 600 positive ticks) and
   still takes thirty seconds. The quota is a diet-permitted *unconstrained rate
   ceiling*, and the gap between it and the attainable rate is an order of magnitude.
   Any later reading of "how often could this fire" has to carry that.
2. **The juvenile exclusion, not the budget, is the largest refusal class.**
   Juveniles reach `Q` and are refused 297–316 times per arm, against 118–132
   affordability refusals. Every `unaffordable` refusal occurred at a reserve
   fraction of essentially zero (median .0000, max .0154): the strict material bound
   excludes only near-starving animals, which is the behaviour the proposal wanted,
   but it means affordability is not what limits this rule.
3. **Every attempt in all four arms occurred in `Feeding` mode; `inactive_mode` is
   structurally unreachable.** A fresh positive settlement requires feeding effort,
   which requires the Feeding mode, so the Seeking half of root's "Seeking or
   Feeding" admission condition never fires. The zero is a fact about the trigger,
   not about the ecology, and should not later be read as one.

## Retained failure

The first pilot run is retained unaltered at
`captures/post-intake-shadow-pilot-2026-09-13` and is **not** relabelled. Its event
streams are correct; its summaries under-counted compatible-window decisions, because
a window closed away from its organism — censored at the run's end, or swept when the
organism was removed — reached only the whole-world total and never its form's row.
The independent reduction caught it (eight disagreements, preserved in
[assets/post-intake-shadow-first-run-reduction-2026-09-13.json](assets/post-intake-shadow-first-run-reduction-2026-09-13.json)),
`33e78b8` fixed it with a regression test, and the corrected run is the separate
`-v2` directory above. Nothing was overwritten, retried in place or rehashed.

## Limits of this note

Two seeds, one care level each, one horizon, one rule, one build. No parameter was
swept and none may be inferred to be tuneable from this. The shadow bounds nothing
about an intervention. Quiet/hunter coexistence remains an unfinished overall-goal
requirement and is untouched here.

## The exact next gate

Root reviews this four-arm pilot. Nothing beyond it is authorised: not the twelve-by-two
ten-minute shadow, not a longer horizon, not a parameter sweep, not a native-64
capture, and not any active-policy implementation.
