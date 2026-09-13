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

## Final committed-package review: `0980eab`

Reviewed commit `0980eabe8727372cf277b440b000807ede201132` from an isolated
`git archive`, not the moving working tree. Retained source is
`captures/build-cache/astra-quiet-review/source-0980eab-FPZ40r`.
Before adding any forensic probe, the exact committed sources passed:

```text
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/captures/build-cache/astra-quiet-review \
  cargo test --locked -p cubarium-core --test astra_quiet_policy \
  --test quiet_pause --test quiet_migration --test snapshot_hardening -- --nocapture
```

**34 passed, 0 failed, exit 0:** 8 independent tests, 12 quiet behavior tests,
6 migration tests, and 8 snapshot hardening tests. The exact B+40 restart fix
remains resolved in this committed package. Additional package tests verify
actual remaining upkeep, bounded movement, unchanged paid-child accounting,
refusal/refund/dead-parent non-admission and atomic hunter-initialization refusal.
This is targeted package evidence, not a full workspace test claim or a biological
screen. No core or independent committed test source changed during this pass.

### Genuine fixture provenance confirmed

All four committed schema-12 fixture envelopes, CRCs, payload hashes and SHA-256s
match `tests/fixtures/quiet-v12-provenance.md`; header build ID is
`pre-quiet-fixture`. The committed generator's SHA-256 is
`ce76719bde869c0413dfb556e24b6f17429e4417d5df01a6cee0c35d579842d2`.
The retained pre-quiet generator executable also exists and matches its recorded
SHA-256 `d48c81b2c015d5d952214aaddd25cc9cc6d10d8f25be16e3415fb2dbd0ba66e5`.
The generator explicitly requires schema 12 and resumes the fixed mature seed-1
opening; the plain and actual Standard-Feed histories both contain paid births.
This review verified those retained artifacts and their 600-tick continuation;
it did not rebuild or rerun the old generator, or rewrite any fixture.

### One remaining narrow decoder-hardening gap

`snapshot.rs::decode_snapshot` now uses `decode_exact` for every legacy mirror
(schemas 7–12). This correctly refuses a schema-13 payload relabelled schema 12
instead of silently dropping its appended quiet extension. The existing relabel
and migration tests pass. However, the current-schema branch still calls
`postcard::from_bytes`, which tolerates unconsumed bytes.

Reproduced on frozen `0980eab`: encode a valid schema-13 state, append byte `0x7f`,
update the envelope's payload length and CRC, then decode. It succeeds and returns
the original state, silently discarding the tail. The CRC and length are valid,
so the outer envelope checks cannot catch this shape mismatch. The isolated
forensic test `astra_current_schema_still_accepts_a_crc_valid_unconsumed_tail`
was added only to the retained review copy's independent test file, after the
34-test run; it passed by demonstrating that acceptance. It is not a passing
strict-rejection regression and was not added to production test sources.

Minimal fixture steps, sufficient to preserve the reproduction:

```rust
let mut bytes = encode_snapshot(&state, "astra");
let at = 10 + u16::from_le_bytes(bytes[8..10].try_into().unwrap()) as usize;
bytes.push(0x7f);
let length = (bytes.len() - at - 12) as u64;
bytes[at..at+8].copy_from_slice(&length.to_le_bytes());
let crc = crc32fast::hash(&bytes[at+12..]);
bytes[at+8..at+12].copy_from_slice(&crc.to_le_bytes());
assert!(decode_snapshot(&bytes).is_ok()); // observed gap, not desired behavior
```

Recommendation: use `decode_exact::<WorldState>(payload, schema)` in the current
branch too, and add a rejection test with a correct length and CRC. This is not
evidence that normal quiet snapshots or the measured continuation fail; it is an
incomplete fail-closed shape check that should be fixed before a schema-13 live
rollout. It does not justify tuning quiet biology or rerunning valid old fixtures.
The ordinary quiet policy is ready for its separately authorized copied-world
measurement, subject to that measurement's own complete numerical and observer
gates; no biological acceptance or live rollout approval is given here.

### Checked-in rejection regression

At the parent's explicit request, the forensic case is now also preserved as
`current_schema_rejects_a_crc_valid_unconsumed_payload_tail` in the independent
test file. Unlike the isolated demonstration, it asserts the desired rejection
and is intentionally red against `0980eab`. The original eight probes remain
unchanged. This test/report-only commit precedes the parent's separately scoped
core correction; it must not be reported as a passing nine-test package until
that correction is rerun.

The checked-in rejection test was executed in the separate workspace cache
`captures/build-cache/astra-quiet-review-current`: **0 passed, 1 failed, 8
filtered out**, exit 101, with the expected assertion about a correct envelope
not legitimizing an unconsumed tail. An earlier invocation reused the isolated
copy's binary and selected zero tests; that invocation is not counted as evidence.

### Root correction and rerun

The current-schema branch now uses `decode_exact::<WorldState>` as well. Its
error describes a payload/schema shape mismatch without claiming all trailing
bytes must come from a newer writer. The public decoder documentation is attached
to the decoder again and lists all seven supported schemas.

Root reran `CARGO_TARGET_DIR=captures/build-cache/quiet-policy cargo test --offline
-p cubarium-core`: **348 passed, 0 failed, 2 ignored**, exit 0, including all nine
independent probes and the genuine schema-12 continuation tests. Log:
`/tmp/cubarium-quiet-exact-decode-tests.log`. The original red result and frozen
forensic copy remain retained. This correction changes acceptance of malformed
payloads, not valid-state physiology; quiet remains Off and is not deployed.
