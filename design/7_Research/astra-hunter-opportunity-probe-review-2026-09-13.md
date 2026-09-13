---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Hunter opportunity diagnostics and replay probe: `0ff2f42`

Read the exact committed [eligibility observer](../../crates/cubarium/examples/hunter_compare/eligibility.rs),
[probe CLI](../../crates/cubarium/examples/hunter_probe.rs) and its harness hooks,
then compared the definitions with `hunter::may_reproduce`, `carrying`, `hunting`
and the actual world physiology funding branch. Those core/diagnostic files still
matched `0ff2f42` during this review. Concurrent presenter/minification and
reproduction-observer work was left untouched. Canon rules/ledger were read;
this report establishes no new accepted design decisions.

## Definitions match, with the intended observation boundary

The first nine blockers are exactly the complements of the local core gate for
validated finite state: existing escrow, positive gut material, target, hunting
phase, structure below `adult − TOLERANCE`, reserve/energy below the profile
fractions, minimum age and reproduction cooldown. Hunting means Stalking,
Windup or Strike; Recovering is not independently forbidden by this gate.
Gut obstruction is positive gut **material**, matching the core method rather
than an invented threshold on gut energy or a visual handling cue.

The extra stock requirements match the actual funding branch:
`child_structure + child_reserve` material from parent reserve, and
`build_cost * child_structure + child_energy` from parent battery. They remain
distinct from the earlier reserve/energy fraction gates. Population capacity and
already queued births are explicitly excluded; an open local gate with sufficient
stocks therefore does not promise an escrow will be created.

The observer sees END-OF-STEP state at `state.tick`. The physiology pass that just
completed checked age and cooldown using its opening `now = state.tick − 1`.
Thus a threshold can be open in the recorded boundary state one tick before it
has been tested by the next funding pass. Conversely, successful funding consumes
stocks and installs escrow, making the post-step observation look blocked.
Digestion, growth, births and deaths can also change the observed membership and
stocks. These are reasons to retain the explicit "opportunity, not transaction"
label, not off-by-one fixes to apply to a boundary-state diagnostic. Actual
`Reproduction` events are the separate mutation-site evidence.

## Denominators and no-hunter cases

Both harness and probe call `observe` once per completed world step. Each living
post-step member contributes one sample; simultaneous blockers overlap. Multiply
these member-ticks by `DT` for summed member-seconds, not elapsed world-seconds.
Newborns contribute at their first observed boundary; members that died during
the step do not contribute at its end. This is not an estimate of their fractional
within-step lifetime. The helper itself does not enforce one call per tick, so
that caller contract matters; repeating a read manually repeats the sample.

No profile, or an empty member list after extinction, contributes no member-ticks
and no eligibility. With no samples, peak reserve/energy fractions are null, not
zero-stock observations or evidence that an absent animal was eligible. A failed
harness step can partly update these statistics; its existing enclosing trust
flags/last-complete tick remain necessary when interpreting the summary.

## Probe safety and hash scope

The CLI reads and validates a snapshot, rejects nondefault care state, constructs
an in-memory world and steps that copy. It does not import a founder, alter tuning,
write an updated snapshot, contact a runner or write to its input path. Observer
reads use immutable state; draining transient events is not world-state/RNG input.
It runs invariants after each step and reports event counts separately from the
post-step opportunity measurements. It does not claim an independent resource audit.

The expected hash is compared to the closing full-state fingerprint as a STRING,
avoiding JSON/JavaScript integer rounding. Missing expectation yields null, not
success; mismatch emits a report with false and then returns an error/nonzero exit.
This verifies an endpoint fingerprint, not every intermediate state in a trajectory
and not collision-free byte equality. Read `trajectory_matches_frozen_trial` in
that limited sense; a future clearer label would be `closing_state_hash_matches`.
The metadata includes opening/closing hashes, ticks and build identity for context.

## Verification

`cargo test -p cubarium --example hunter_probe eligibility::tests`: **3 passed,
0 failed**. Tests cover a core-gate comparison with the stock obstacle separated,
overlapping target/gut/cooldown blockers, no-member handling, repeated-read state
hash identity, and 400-tick continuation identity with/without observation.
`cargo run -q -p cubarium --example hunter_probe -- --help` also succeeded.

No source-level blocker found in scope. The three tests do not exercise the CLI's
expected-hash match/mismatch exits, preservation of an input file, or every gate's
threshold equality independently. Recommended small follow-up: a scratch-snapshot
CLI test checks both expected-hash branches and input bytes unchanged; a boundary
table compares the first nine blockers to `may_reproduce` across age/cooldown,
adult tolerance, stock fractions and all phases. These are coverage improvements,
not claims that the current source has failed them. No live operations or worker
source edits were performed.

## Subsequent actual CLI evidence

Root supplied `/tmp/cubarium-hunter-seed6-opportunity.json`; I read it and checked
the archived arm's opening/summary. The care-free seed-6 facultative-on replay
runs tick 144000→288000 and its reported closing hash `8365334319777227972`
matches the frozen trial summary. It records 96 captures, 117 misses, 120
out-of-reach attempts, no offspring/reproduction records and one starvation death.
Both reserve and energy gates are blocked for all 138121 observed member-ticks;
maximum reserve fraction is 0.5 against the profile gate 0.8, and maximum energy
fraction 0.7499056079830375 against 0.75. This explains the measured end-of-step
opportunity absence for THIS arm, not a universal conclusion about hunter biology.

I independently hashed the input snapshot:
`f1c604f28f6aeb093438913d31fdb759144e676cbe2eaf5b450c94b6f8b3dd41`,
matching the archived opening record. Root reports replay exit 0 and a separate
one-tick `--expect-state-hash 0` run returning false/exit 1 with input unchanged;
I did not rerun those CLI processes. This supplies practical coverage beyond the
three unit tests while retaining attribution of process exit evidence.

Root also changed the current source label to `closing_state_matches_frozen_trial`,
which removes the intermediate-trajectory implication. I read that one-field
diff; it was uncommitted at this follow-up. The earlier saved replay JSON retains
the old field name and should not be silently rewritten as if it used the new one.
