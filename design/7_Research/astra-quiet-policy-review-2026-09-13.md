---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Independent post-birth quiet-policy review

Scope: the in-progress ordinary quiet implementation against the complete
[proposal](astra-ordinary-quiet-experiment-proposal-2026-09-13.md), not approval
of its biology or deployment. Canon, Lore and Graft were read first. Only this
note and `crates/cubarium-core/tests/astra_quiet_policy.rs` belong to this review;
the native implementation worker owns core and migration files.

## Reproduced boundary defect — resolved in the current working source

The initial dirty `QuietState::validate` rejects `end_tick <= state.tick` while
`quiet_hold` expires the entry on the next decision at `now == end_tick`.
A real successful child insertion at B=3 admits the parent; decisions 3 through
42 correctly complete forty resting intervals. At completed tick 43 the pause
still holds its underlying mode for the imminent ordinary decision, but normal
snapshot decode rejects it:

```text
Invalid("quiet pause OrganismId { slot: 0, generation: 1 } expired at 43 but is still held at tick 43")
```

The independent test
`successful_paid_birth_holds_exactly_forty_intervals_and_restarts_at_the_last_boundary`
preserves that exact fixture. It restarts immediately after the birth, checks
each held interval and its decode, and intends to restart again at B+40 before
the release decision. Validation may allow the legitimate equal endpoint, or an
alternative implementation must preserve both the just-completed Resting pose
and the underlying mode needed for the next decision. Simply removing the timer
early and feeding imposed Resting into hysteresis would be a different defect.

The native worker corrected validation to reject only `end_tick < tick`. The
equal endpoint now preserves the underlying mode until the release decision,
without repainting the fortieth completed interval. The unchanged independent
fixture passes: decode/restart at B+40 agrees with uninterrupted execution, and
the following decision emits exactly one End at B+40 with 40 completed ticks.
No core source was edited here.

## Independent probes

Command (workspace build cache, no live process or full study):

```text
CARGO_TARGET_DIR=captures/build-cache/astra-quiet-review cargo test -p cubarium-core --test astra_quiet_policy -- --nocapture
```

Initial result: **7 passed, 1 failed**, solely the exact endpoint decode above.
After the worker's correction: **8 passed, 0 failed**, exit 0. The core changes
were still uncommitted at review time (repository HEAD `8256dab`), so this is not
final frozen-package certification. Passing evidence is bounded to:

- A synthetic, active ordinary parent funds and inserts a real child. Candidate
  and Off states agree completely after removing only the quiet extension through
  that first admission, with identical life events: no added debit, changed child
  stores, retroactive birth-tick Resting, or extra RNG at admission. The newborn
  itself is not admitted. This fixture changes founder stocks/gestation only for
  deterministic mechanism testing, not as an ecological candidate.
- All forty held intervals execute Resting with no actual intake and no new
  escrow; the parent's turn counter advances by exactly the original four draws
  each tick. Decoding after every interval, including B+40, and the exact release
  assertions now pass.
- The conservative individual budget matches the proposal's arithmetic, rejects
  equality and nonfinite inputs, and is read-only. It is not a debit or a survival
  guarantee. Core source retains actual upkeep, growth, oxidation and age checks.
- Pure controller tests in the seek-off/seek-on band retain truthful ordinary
  mode and hunger memory, suppress intake/budding under the override, and make
  release equal a single ordinary decision from the retained underlying mode.
- Forced reserve depletion aborts before that same tick's ordinary controller.
  The resulting complete legacy state equals the corresponding Off decision;
  draining records is state-neutral and cannot rearm the opportunity.
- Age death after one held interval yields exactly one ParentGone abort and a
  normally decodable state. A reused slot has a new generation and cannot make
  the old parent ID valid. Duplicate entries, wrong version, Off with entries,
  stale parent, self-child, invalid duration/overflow and capacity overflow are
  rejected; the enabled-policy/hunter combination is refused by validation.
- Genuine pre-quiet schema-12 plain and Standard-Feed fixtures migrate Off and
  continue 600 ticks to byte-identical legacy projected payloads. Enabled policy
  refuses the schema-12 projection. The ordinary ecology hash still hashes the
  bare legacy diagnostic payload. These do not claim that a lower-schema
  diagnostic projection contains the policy or is a behavioral replay format.

## Reviewed source identity and remaining disposition

SHA-256 of the worker-owned source read immediately after the passing run:

```text
quiet.rs        7f6dfb9c0a220247e48cd21bf5507ef4cfd9906ecb6ad99f349eff7189f08668
controller.rs   fe17d9f76cd50f022db8c1606bbd75e30d24ddea25ab3648cc0c11efa3369421
world.rs        23660e1ff66e397d3520f985f39937b04ea0c334c438978b27959af4744fa74f
snapshot.rs     4a34b85ad685e04da7118c85f6c841a1bcc91de4c892c7c1cfc9f829608ebd17
snapshot/v12.rs cd79c63337e2db68d8e557587ab4a1af76659e0b517212790bfcac29f2426842
```

Rerun against the final frozen package after the worker finishes its remaining
migration tests. This review does not replace the separate real-world ten-minute,
twelve-seed no-care/fixed-Feed experiment or its required longer horizons. Passing
unit/migration tests cannot establish adequate quiet opportunity, improved
viability, paid offspring recruitment, or permission to change the Off default.
