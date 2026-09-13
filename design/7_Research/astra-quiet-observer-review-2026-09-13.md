---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Quiet comparison observer: independent boundary review

Review of the completed initial observer package `6d4ac7b`, after reading canon,
Lore/Graft context and the entire [ordinary quiet proposal](astra-ordinary-quiet-experiment-proposal-2026-09-13.md).
Scope is behavioral classification/reconciliation, not the parent's separate
manifest, audit and reducer-provenance review. The correction worker is active;
this is **not** a finding against its future corrected package.

## Reproducible failures

The separately owned example
`crates/cubarium/examples/astra_quiet_observer.rs` includes the observer by path
and supplies six independent fixtures. It changes no worker file or core behavior.
Its one-parent synthetic setup performs a genuine core escrow funding and child
insertion at B=3. Fixture-only stock/age changes isolate boundaries; they are not
the copied-world ecology recipe or balance evidence.

```text
CARGO_TARGET_DIR=captures/build-cache/astra-quiet-harness-review \
  cargo test -p cubarium --example astra_quiet_observer astra_ -- --nocapture
```

Result: **0 passed, 6 failed, 7 filtered out**, exit 101. Before/after source
checks show the tested `quiet_compare/bouts.rs` is identical to commit `6d4ac7b`,
SHA-256 `371c47cd4db6a6da08ed20538faf3d7fa4a78ce550363537040cd61db7fece89`.
The seven filtered tests are the included worker module's own tests, not seven
independent successes. All six checked-in assertions express desired behavior
and deliberately remain red until the worker correction.

### 1. Successful release into ordinary Resting loses its terminal reason

`Observer::after` labels a recovery bout's terminal event only in the active-mode
branch. If the ordinary controller legitimately chooses Resting after expiry,
the class-change branch instead closes recovery as `Reclassified(Satiated)`.
The fixture completes all forty held intervals and receives one real End, but
the bout is not `Released`. The reducer separately requires released bout count
to equal End count, so a valid world can be refused. Early abort into ordinary
Resting follows the same class-change branch and needs equivalent handling.

Preserve the recovery terminal reason from the matching End/Abort **before**
opening any subsequent natural-rest bout; continuing to rest does not cancel a
completed recovery release.

### 2. Death on the fortieth held interval is valid, not an overlong abort

An age death on completed B+40 follows the final held decision at B+39. Core
correctly emits `Abort(ParentGone, completed_ticks=40)`. The observer rejects all
aborts with `completed_ticks >= 40`, falsely marking this valid trajectory
unreconciled. Its post-state-only classification also records a 39-tick recovery
bout because the parent is no longer in the arena, despite the record confirming
forty executed intervals. The fixture exposes both facts: class recovery ticks
39 versus core completed-held ticks 40. The reducer has the same unconditional
abort-duration rejection and must use the corrected boundary contract too.

Distinguish pre-decision affordability aborts (the current interval is ordinary)
from post-decision ParentGone aborts (the current held decision executed).
Preserve the completed dead-parent interval in the recovery duration. If general
population organism-time remains a post-step census measure, name that denominator
explicitly rather than silently conflating it with executed decision intervals.
No unavailable dead-parent post-movement pose or intake amount should be invented.

### 3–6. Quiet event reconciliation is not one-to-one with actual state changes

Four independent malformed-stream fixtures are currently accepted with
`reconciled=true`:

- Duplicate a genuine Begin: the same paid birth increments admissions twice.
- Remove Begin while retaining the real newly inserted pause and actual birth.
- Append an Unaffordable Refuse for absent full IDs (slots 999/998, generations
  7/8), with no child insertion or matching life event.
- Remove the genuine End on release: the bout silently becomes Woke/Reclassified.

`Begin` currently searches for any matching LifeEvent but does not enforce
cardinality against the new actual pause. Refuse merely increments a reason
counter. End/Abort do not validate their full originating identity and expected
pre/post transition. This can change opportunity denominators or terminal counts
without making the harness's reconciliation gate fail.

For this fixed policy, compare the full pre/post pause sets and actual committed
birth identities with the event multiset at the exact boundary: every new pause
needs exactly one Begin; a rejected opportunity still needs its real birth;
every disappearing pause needs its matching release/death/abort, including the
child and start identity. Reject duplicates and missing records, not merely
impossible aggregate totals. Keep violations bounded and retain failure; no
transaction API expansion or changes to core reproduction payments are needed.

## Measurement limits to keep explicit

The proposal also asks for active-to-recovery versus natural active-to-rest
entries. The initial observer records classes and bout durations, but does not
emit prior-mode/active-entry evidence. Not every admitted parent is necessarily
active at admission; do not equate admission count with interrupted-foraging
count. Either expose this small classification distinction or label it unavailable
before interpreting the behavioral screen. The one-second/9-of-12 screen must
remain about genuine non-newborn recovery, separate from biological viability.

The initial module's same-face coordinate displacement is not a guaranteed
transported path length at a reflected rim; a body can reverse within the same
face. The parent already permits a minimal read-only movement accessor in the
correction task. It must expose actual core segments without changing simulation,
RNG, stocks or schema. Any unmeasured dead-parent segment remains unavailable.

The parent separately owns the discovered `verify_resume` comparison of two
decoded worlds (not uninterrupted versus resumed), the ineffective loader fixture,
and strict reducer/provenance gates; those are not duplicated here.

## Disposition

Do not launch the 48-arm biological screen or certify completed measurement from
the initial observer. The six red fixtures are a correction handoff, not a claim
that the future worker package fails them. Retest the exact final commit, then
distinguish passing mechanism/observer gates from actual quiet opportunity and
the unchanged no-care survival/diversity requirements. No core, art, live state,
care recipe or canonical decision changed in this review.

## Committed correction review — `2fbd092` / `d3f9782`

The completed correction was reviewed read-only, including its full handoff,
observer, runner shadow, life/path output and reducer. The relevant core, runner,
observer and reducer sources match `2fbd092` exactly during these tests. No
48-arm collection was launched. The original **six**, not three, independent
regressions above now pass unchanged. The handoff's occasional “three previously
failing” wording understates the preserved first-package result.

Verified closures and evidence:

- `cargo test -p cubarium --example quiet_compare`: **18 pass**.
- Original six `astra_` observer tests: **6 pass**, including successful release
  into natural rest and death on completed B+40.
- `node scripts/reduce-quiet-compare.test.mjs`: **24 pass**. Two frozen snapshot
  inspection tests initially encountered sandbox child-process EPERM; rerunning
  with permission passes both. EPERM is not a reducer defect.
- Root's unmodified `assets/root-quiet-gate-probe.mjs`, using
  `captures/quiet-smoke-validated-2026-09-13c`: all **five** cases have
  `gap:false` (untouched control accepted, four damaged summaries refused).
- Core `astra_quiet_policy` integration suite: **9 pass**, including genuine
  schema-12 continuations and actual last-boundary snapshot restart. Core
  movement-accessor parity test: **1 pass**. Filtered-out tests are not additional
  successes. This is not a new full-workspace test claim.

### Corrected restart, ancestry and receipt paths

`Arm::begin_shadow` now decodes once and compares that state directly with the
actual primary before allowing the shadow. `advance_shadow` follows the real
arm's normal step, comparing complete persisted state and both record streams.
The primary is never reconstructed to serve as its own oracle. Both receive the
identical care command if a comparison window overlaps elapsed 600. A failed or
unfinished proof is retained and disqualifies the arm. The damaged-shadow test
really demonstrates the old two-decodes agreement and a dropped pause's
disagreement with the uninterrupted primary. A rewritten underlying mode may
converge after a tick; equality at the initial decode, not a promise of later
divergence, is the right check for that case. No defect found in this correction.

Opening organisms and all birth/death records now retain full IDs, form and
opening ancestry. Births are resolved before same-tick deaths, and living ancestry
is bounded. The low-level life reducer checks generational depth, known parents,
death identity/age and closing survivor counts. The new movement accessor only
borrows existing transient segments; births explicitly clear a reused slot's
segments. Observer segment sums cover **post-step living organisms**, matching
its population-time denominator. They do not include a removed organism's final
movement. Do not expand the claim to every pre-step creature's terminal motion,
or claim unavailable terminal intake/RNG checks. Prior active-mode versus
already-resting recovery-entry classification also remains absent; retain the
earlier limitation rather than implying all admissions interrupted activity.

The care checks now verify the single Standard Feed's dose/target/sequence,
elapsed and absolute application ticks, applied outcome and ledger equality;
no-care requires every ledger field zero. Root's four concrete false-certification
cases are closed. These are validation of recorded evidence, not a new care
replay or ecological conclusion.

### Remaining correction gates

1. **Death during the first held interval loses the whole bout.** The corrected
   `Observer::after` death branch iterates only `self.open`. At admission the
   parent was still active, so no recovery bout exists yet. If age death occurs
   during decision B, core emits the real `Abort(ParentGone, completed_ticks=1)`
   at B+1, but the observer remains reconciled and emits no recovery bout at all.
   New owned fixture
   `astra_death_on_first_held_interval_retains_the_actual_one_tick_bout` performs
   a real paid birth, sets fixture-only maximum age to B*DT, and fails because
   the one-tick bout is absent. Record this real terminal interval even when it
   has no previously open bout; keep it separate from post-step population-time.

2. **The closing-snapshot check repeats the already-fixed B+40 range error.**
   `verifyClosingSnapshot` requires `inspection.tick < pause.end_tick`
   (lines 670–674). A genuine completed B+40 state still carries that entry until
   the following ordinary decision. Core's independent 40-interval/restart test
   passes this exact boundary. The new separate JavaScript validator fixture
   demonstrates the false refusal and deliberately labels its envelope/inspection
   synthetic; it is not misrepresented as an actual captured world. Admit equality
   at this completed boundary, not a later expired entry.

3. **The reducer changes the energy completion contract.** `verifyArm` demands
   `gates.legacy_raw_energy == true` and raw energy drift below the opening limit.
   The runner's imported `shared/audit.rs::audit_passes` deliberately requires
   raw material/water plus persisted-corrected, independently windowed and
   immediate receipt-boundary energy, leaving raw energy diagnostic. Thus an arm
   can truthfully pass the runner's technical gate and be rejected by the reducer
   solely for naive-counter rounding. Preserve the raw value and honest false
   legacy flag, but use the same compensated contract and unchanged 1e-8 limits
   in both places. A separate JavaScript regression changes only raw energy and
   its flag in a copied smoke summary; it fails as expected. It does not claim
   this smoke actually had the injected drift.

4. **Cross-file paid-birth linkage discards the child identity.** In `loadRun`,
   the offer key contains `parent@tick>child`, but the check is only
   `life.birthsByParent.has(offer.split('>')[0])`. A read-only mutation of seed-1
   candidate no-care records changes the child generation in its matching Begin
   and End from `11/5` to `11/100005` at boundary 144228. `reduceQuietEvents` and
   `reduceLife` both pass, and the current cross-file predicate accepts
   `47/5@144228>11/100005`, despite the actual birth map naming `11/5`.
   Compare the map's child value as well. `recoveryOrigins` similarly retains
   parent+boundary without child, so carry/check the child in that crosswalk too.
   This is a concrete in-memory probe of the exported checks and exact current
   cross-file predicate, **not** a claim to have launched a full-horizon reduction.

After adding the first-death fixture, the independent Rust observer command is
**6 pass, 1 fail**. `node scripts/astra-quiet-correction-review.test.mjs` is
**0 pass, 2 fail**, intentionally preserving the other two desired-behavior
boundary/audit regressions. Correct these narrow issues and exact-child linkage
before freezing the first 48-arm behavioral screen. The existing green smoke is
technical evidence only; it establishes no biological acceptance or live default.

## Second correction closure — `fe114f9` / `42faaa2`

**Ready for root to freeze and run the first 48-arm, ten-minute screen.** This
closes the four findings in `5479e11`; it is not evidence of biological benefit,
adequate quiet opportunity or permission to change the Off/live default. No
experiment was launched by this reviewer. The compared runner, observer and
reducer sources match `fe114f9` during the tests below; the handoff-only follow-up
is `42faaa2`.

1. **First-held death: resolved.** `Observer::after` now derives held deaths
   from the full pre-decision pause set and actual LifeEvent death, not just
   already-open bouts. It opens/extends the real recovery bout before terminal
   closure, handling a parent previously active or resting. The independent
   one-tick fixture now also checks exactly one `held_intervals_ended_by_death`,
   one completed core held tick, one recovery bout, and zero post-step living
   recovery ticks. Thus the removed interval is retained without silently
   entering the population-time denominator. The original fortieth-interval
   fixture remains green. The crosswalk additionally refuses a positive-duration
   close whose bout was dropped, and accepts its restored one-tick bout.

2. **Completed B+40 snapshot: resolved.** The reducer permits
   `start_tick <= tick <= end_tick`. The independent validator fixture accepts
   exact equality and now explicitly rejects B+41. This is the core boundary,
   not a widened pause duration.

3. **Raw energy re-gating: resolved.** The reducer matches the runner's fixed
   material/water and persisted-corrected/windowed/receipt energy gates. Raw
   energy remains finite, nonnegative, visible and separately flagged; it no
   longer determines completion. The independent test accepts a correctly
   flagged raw-only failure and still refuses equality at the unchanged energy
   limit for each of the three actual energy gates. No tolerance changed.

4. **Exact child crosswalk: resolved.** New exported `crossCheck` compares the
   child identity against the actual birth set and separately carries it from
   recovery bout to admission/close. The exact retained seed-1 probe now rejects
   `47/5@144228` with substituted child `11/100005` instead of `11/5`, despite
   the Begin/End stream remaining internally consistent. A separate reciprocal
   mutation of the bout's origin child is also rejected. The independent fixture
   invokes the actual exported crosswalk, not a copied predicate. Positive
   terminal durations must have matching-length bouts in the reverse direction.

Fresh checks:

| Check | Result |
| --- | --- |
| Full `astra_quiet_observer` example | 16 pass, including seven independent fixtures and nine included module tests |
| Seven independent fixtures after strengthening first-death counters | 7 pass, 9 module tests filtered |
| `quiet_compare` example | 19 pass |
| Native quiet reducer tests, including frozen read-only inspection | 27 pass |
| Independent `astra-quiet-correction-review.test.mjs` | 4 pass: retained boundary/audit fixtures plus exact-child and missing-bout crosswalk probes |
| Root's unmodified five summary probes against smoke `…-13d` | all five `gap:false` |

The two Rust example suites share module tests; their results are reported by
command, not added together as independent coverage. The native full-core test
claim was not rerun for this bounded correction; core source did not change in
the correction. The prior independent core boundary/compatibility checks remain
separate evidence.

Read-only comparison of all twelve seeds × four arms in confirmation smoke
`captures/quiet-smoke-validated-2026-09-13d` against `…-13c` confirms **48/48
closing state hashes identical**. Every new smoke arm reports technical/audit
completion and correctly leaves `complete_experiment_measurement:false`.
`held_intervals_ended_by_death` is zero across this smoke: it supports inertness
where the death branch did not execute, not observational coverage of that branch.
The actual synthetic core-death fixtures above supply boundary coverage.

No further directly relevant boundary defect was found in this bounded pass.
Keep the explicit measurement limits now acknowledged in the native handoff:
post-step living path/population-time only, terminal intake/RNG unavailable,
entry activity not distinguished from already-resting admission, and no exact
funding/oxidation attribution through the ordinary API. Report the first screen
as completed-horizon measurements with those limits, not a result inferred from
the short smoke or a recommendation to deploy the candidate.
