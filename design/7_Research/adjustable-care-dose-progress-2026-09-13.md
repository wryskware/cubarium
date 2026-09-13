---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Adjustable care dose: core and host implementation progress

Implementation record for `adjustable-care-dose-handoff-2026-09-13.md`. Evidence, not
decisions. **Nothing here is a balance claim**: no number below says a Generous feed is good
for the ecology, or that any preset should be offered in the live world. This package changes
what the owner *can ask for* and proves the amount survives the trip; matched ecological
evidence for any nonstandard preset remains the separate ambient-support package.

No live rollout was performed and no live interaction was run. The owning process, the shim,
`state/` and the frozen `hunter_compare` experiment were not touched.

## Commits

| Commit | Scope |
| --- | --- |
| `2f92076` | Core: `CareDose`, the scaled feed / rain / clean totals, schema 12 and its frozen mirrors, the genuine schema 11 fixtures, 21 new core tests |
| `1065dba` | Host: typed dose through submit / journal / runner, the `accepted_dose_v1` record, `/care/status` capability and per-row amount, the optional HTTP property, 2 new replay tests and 8 new unit tests |

Both were taken under `flock /tmp/cubarium-shared-care/git.lock`. Nothing was pushed.

`crates/cubarium/src/sink/web/index.html` (root), `art_present.rs` / `hunter_present.rs`
(Fable) and `.vscode/` are untouched in both.

## Wire ABI, for the viewer

This is the whole contract root needs; it is stable and on `main` now.

### Capability — `GET /care/status`

```json
"dose": { "version": 1, "min_permille": 250, "max_permille": 2000, "default_permille": 1000 }
```

Served only when care is enabled. A host without `--care` serves the existing
`{"enabled": false, …}` body with **no** `dose` key at all, and a pre-dose host serves no
`dose` key either. Absence, or a `version` the page does not recognize, means standard-only
requests with no dose property — which every host from the first care build onwards accepts.
The four numbers are read from the core's own constants, so they cannot drift from the bounds
the world enforces.

### Request — `POST /care`

One new **optional** property beside the existing `client` / `request` / `kind` / `target`:

```json
{ "client": "…", "request": 3, "kind": "rain",
  "target": {"face": 2, "u": 31, "v": 7}, "dose_permille": 1500 }
```

* **Omitted** means exactly 1000. It is the same semantic payload as an explicit 1000 —
  including for duplicate detection — so a page may keep sending nothing at all.
* **Present** must be an integer in `250..=2000`. An explicit `null`, a fractional or negative
  number, a string, a boolean, an array, an object, `0`, `249`, `2001` or anything larger is
  `400 {"error": "…dose_permille…"}`. Nothing is clamped and nothing is read as omission. The
  refusal happens before the identity is consulted, so a rejected amount does not consume a
  request number and the page may retry the same `request` with a corrected amount.
* The amount is part of the request's identity. Same `request` and same amount → the retained
  receipt, `200`, `"duplicate": true`. Same `request`, different amount → `409 conflict`,
  never a second command.

Every other status code, cooldown, limit and gate is unchanged.

### Receipts — `GET /care/status`

Every retained row gains one key, between `target` and `seq`:

```json
{"client":"…","request":3,"kind":"rain","target":{…},"dose_permille":1500,
 "seq":7,"apply_after_tick":1240,"state":"applied","reason":"","applied":{…},"duplicate":false}
```

`dose_permille` is what the **server** recorded for that request, so the panel can label a
receipt independently of whatever its selector currently shows. A row from a legacy journal
record reads 1000.

Rain's `applied.water_depth` is the *scheduled* total at the chosen amount (`4 · dose/1000`);
the depth actually delivered so far is the world's `care.rain_depth_in`, as before.

### Frame bytes

Unchanged. The dose touches no render path: `care_effects` draws from the receipt quantities
it already drew from, and every existing frame-byte test passes untouched.

## Dose semantics as implemented

`CareDose` is a `u16` permille, private field, constructible only through `CareDose::new`,
which **refuses** anything outside `250..=2000` rather than clamping. `CareDose::VERSION` is 1.

`CareDose::scale(nominal)` returns `nominal` unchanged at 1000 through an explicit identity
branch, and `nominal * permille / 1000.0` otherwise. The branch is load-bearing rather than
decorative: `x * 1000.0 / 1000.0` is not `x` for every `x` (`f64::MAX` overflows to infinity),
and a world that takes no nonstandard amount must step exactly as the pre-dose build did.

| Command | What the dose scales | What it does not touch |
| --- | --- | --- |
| Feed | the nominal 3 m total and the `rho · (m · w)` chemical energy that rides with it | the footprint, the weights, `rho`, the 30 m net allowance |
| Rain | the nominal 4 d total | the footprint, the eased 120-tick envelope, the sample count |
| Clean | the nominal 2 m maximum export | the 50%-per-cell limit, the footprint, the proportional energy export, the partial receipt |

A feed whose amount exceeds the remaining allowance is **refused** (`"allowance exhausted"`),
not served smaller — and the refusal still spends its sequence number, like every other
admitted-then-rejected command. The allowance therefore buys 40 quarter feeds, 20 half feeds,
10 standard, 6 at 1500 and 5 at 2000: thirty units either way.

A clean at 2000 takes twice the litter *where there is litter*, and still cannot take more
than half of any one cell. With 0.01 m in a cell, every amount from 250 to 2000 removes the
same 0.005 m and the cell is not emptied. Cleanup did not become sterilization.

An out-of-range amount that arrives from a wire rather than the constructor is rejected on
admission by the world (`"care dose: dose_permille N is outside 250..=2000"`), spends its
sequence and moves nothing.

## Durability

### Snapshot: schema 12

`ActiveShower` gained `dose_permille`, which changes the postcard shape of `CareState`.
Postcard is not self-describing, so schemas 8, 9, 10 and 11 — all of which nested the old care
shape — could not go on borrowing the live type. `snapshot::care_v1` freezes the pre-dose
`CareState` / `ActiveShower` field lists, and `snapshot::v11` freezes the schema 11
`WorldState`. No `#[serde(default)]` was added to any of them; a missing trailing field in
postcard is not a missing field, it is the next value read from the wrong offset.

A migrated in-flight shower opens at `CareDose::STANDARD`. That is not a guess: it is the only
amount those builds could deliver.

The schema 10 non-empty-extension refusal is unchanged — an active schema 10 trial is still
refused by name.

Going the other way, `v8`, `v9`, `v10` and `v11`'s `project` now return `Option` and **refuse**
a world whose active shower is nonstandard, rather than writing a legacy shower that silently
dropped the amount. The refusal lasts exactly as long as the shower: once the 120th sample has
fallen there is a legacy image again, carrying the nonstandard depth in the cumulative ledger,
because the ledgers are amounts and the old shape has always held amounts. `v7::project` drops
care entirely and is still total, so `ecology_hash` is unchanged and a care run stays directly
comparable to a matched no-care run.

`snapshot::v11` deliberately keeps the **live** `HunterState` rather than freezing a second
copy, because this package did not touch it. That borrow is guarded rather than assumed: the
genuine schema 11 hunter fixture below fails if the hunter shape ever changes without being
frozen there.

### Journal: two accepted records

A standard-dose command is still written as `{"rec":"accepted", …}` with **no** amount field.
An older binary reads it and applies precisely the right thing, so a standard-only history
stays readable by the build that wrote it.

A nonstandard command is written as `{"rec":"accepted_dose_v1", …, "dose_permille":N}`. The new
discriminator is the point: an older binary does not know the record kind, so it refuses to
start rather than reading the line as an ordinary command and quietly applying 1000. An extra
field on the old record would have been ignored, which is exactly the silent failure this
avoids.

Parsing is asymmetric, on purpose:

* `accepted` must carry **no** `dose_permille`. Its presence is refused *even at 1000* — the
  writer's intent is what is wrong, not the number.
* `accepted_dose_v1` must carry an explicit, valid, integral `dose_permille`. Missing never
  defaults to standard.
* `accepted_dose_v2`, a fractional, negative, null or out-of-range amount, or any other
  unknown kind fails closed at open, **including at the final newline-terminated record**. Only
  an actually unterminated byte suffix is still truncatable, and a half-written
  `accepted_dose_v1` is still just a torn tail.

Journals stay append-only. No history is rewritten and no dose is ever inferred from a receipt.

## Evidence

### Genuine pre-change fixtures

Four schema 11 snapshots written by the **pre-dose release build**, in an isolated
`git archive e55501dbd685f7f02c3599fbaf86c8710f299e7a` tree at
`/tmp/cubarium-pre-dose-lvTTlU`, by a generator that asserts `SCHEMA_VERSION == 11` before
writing anything. Generator source committed verbatim beside them as
`care-v11-generator.rs.txt`; full provenance, SHA256s and payload hashes in
`crates/cubarium-core/tests/fixtures/care-v11-provenance.md`. Header schema verified `11` on
all four from the raw bytes. These are not a current struct relabelled as old — the schema 12
shape did not exist anywhere on disk when they were produced.

| Fixture | What it is |
| --- | --- |
| `care-v11-shower-360.cubw` | tick 360, real feed + clean history, shower seq 3 at 60 of 120 samples |
| `care-v11-shower-360-plus600.cubw` | the same world 600 ticks later, shower finished |
| `care-v11-hunters-200.cubw` | tick 200, an actual Lanternjaw trial, one live member, real imports |
| `care-v11-hunters-200-plus600.cubw` | the same world 600 ticks later |

Loading each `-360` / `-200` file into the schema 12 build, stepping 600 ticks and re-encoding
the schema 11 projection reproduces the `-plus600` payload **byte for byte**. That is the
standard-dose continuation claim: not numerically close, the same arithmetic. The existing
genuine schema 7, 8 and 9 continuations (`live-v7-55200`, `live-v8-172800`,
`pre-hunter-v9-173400`) still pass unchanged beside them.

### Executable old-reader evidence

Pre-dose host binary built from a second isolated `git archive e55501d` tree
(`/tmp/cubarium-pre-dose-host-j7Elof`), SHA256
`e48f2e340c30f0c5bf9469f5d63fe39ca95e95d882ae298dc7cc9d747f4e2f85`.

1. Given a journal containing one `accepted_dose_v1` record, it **refuses to start**, exit 1:

   ```text
   cubarium: …/care.jsonl: line 2 is not a valid care record
   (unknown record kind `accepted_dose_v1`); a complete, newline-terminated record is not an
   interrupted write, so it is preserved rather than discarded
   ```

   The journal was byte-identical afterwards (`diff` clean). The amount was not ignored.

2. Given the same journal with a legacy `accepted` record instead, the same binary replays it
   correctly: `cubarium: replayed care seq 1 (rain) at tick 120: applied`.

3. Given a schema 12 snapshot, it refuses by name — `skipping …/world-100.cubw:
   UnsupportedSchema(12)` — declines to create a new world in a directory it judges damaged,
   and exits 1. No misread, no silent data loss. This is the documented limit, not a fix:
   **executable-only rollback is unsafe once a schema 12 snapshot or an `accepted_dose_v1`
   record has been written.** A downgrade needs a matched snapshot/journal backup.

4. Incidental no-input check: the pre-dose binary and the schema 12 binary both report
   `hash 16a70b7edd600c5d`, `residual -2.728e-12`, population 24 at tick 100 from
   `--fresh --seed 4242`.

### Tests

All foreground, all run to terminal completion.

| Command | Result |
| --- | --- |
| `cargo test -p cubarium-core` | 16 binaries, 0 failed, **exit 0** |
| `cargo test -p cubarium` | 0 failed, **exit 0** |
| `cargo test --workspace` | **1056 passed, 0 failed, exit 0** |
| `cargo clippy -p cubarium-core --all-targets` | clean |
| `cargo clippy -p cubarium --all-targets` | no new findings in any file this package touched |

New tests, 31 in total:

* `crates/cubarium-core/tests/care_dose.rs` (15) — the bounds and the refusal to clamp; the
  bit-for-bit standard identity including `f64::MAX`; explicit-standard ≡ no-dose over 125
  lockstep ticks; feed / rain / clean scaling at 250, 500, 1000, 1500 and 2000 across an
  interior cell, a face seam and the open rim, checked per cell against `m · w` and
  `rho · (m · w)`; the 30-unit allowance at every amount and the without-a-trace refusal of one
  that does not fit; the 50%-per-cell limit holding at every amount; empty and sparse cleanup;
  rain save/resume at 1500 and at 250 with the whole shower delivered once; a wire dose outside
  the bounds rejected on admission while still spending its seq; a crafted snapshot dose
  refused by the decoder; the dose inside the replay hash.
* `crates/cubarium-core/tests/care_dose_migration.rs` (6) — the two byte-for-byte continuations
  above, the mid-shower migration opening at standard, the hunter-extension guard, a schema 12
  payload relabelled as schema 11 failing rather than misdecoding, and the legacy-projection
  refusal in both its forms.
* `crates/cubarium/src/care/journal.rs` (5) — the two record kinds and their fields; both
  bounds round-tripping; the asymmetric parse across eight malformed or mislabelled cases, each
  as the **final** newline-terminated line, with the bytes preserved; a half-written dosed
  record still being just a torn tail; the discriminator staying outside the pre-dose reader's
  vocabulary.
* `crates/cubarium/src/care/mod.rs` (4) — the advertised capability and its absence when care
  is disabled; per-row `dose_permille`; the standard wrapper and duplicate/conflict identity;
  the amount reaching the planned command unchanged at every documented value.
* `crates/cubarium/src/sink/web.rs` (3) — twelve unusable `dose_permille` bodies answered 400
  and never clamped; a posted amount reaching the planned command and the receipt row, with
  omission standard; the capability over the real route and its absence without care.
* `crates/cubarium/tests/care_replay.rs` (2) — a **mixed** old/new journal replaying each
  command at its own amount and landing on the ecology a dosed uninterrupted run would have had
  (and provably *not* the all-standard one); a 1500-permille shower interrupted by a snapshot,
  its amount verified inside the mid-shower snapshot, resuming to the dosed reference and not
  the standard one.

Every pre-existing care durability and security test still passes: the held boundary, the
delayed acknowledgement, the uncertain write, the torn tail, the journal bound, the ENOSPC
asymmetry, the epoch-scoped identities, the CORS refusals and the cooldowns.

## Limits, stated plainly

* **No balance or biological claim.** Nothing here says any preset is ecologically sound. The
  three presets root's panel offers are interaction settings awaiting the ambient-support
  package's matched evidence.
* **No rollout approval.** Nothing was enabled in the live world and no live interaction was
  run. A live upgrade needs a matched snapshot *and* journal backup taken together; after the
  first schema 12 snapshot or `accepted_dose_v1` record, swapping the executable back is not a
  rollback (evidence item 3 above).
* **`snapshot::v11` borrows the live `HunterState`.** It is guarded by a genuine schema 11
  hunter fixture, not frozen. A future hunter-shape change must freeze it there; the test will
  fail rather than misread if it is forgotten.
* **The dose is not in the render.** A Generous feed looks like a standard one. If the panel
  should show the difference, that is a presentation package and Fable's file.
* **No compaction, no new limits.** Queue depth, outstanding count, retained receipts,
  cooldowns, the accept wait and the 4 MiB journal bound are all exactly as they were.
* **Browser acceptance is root's.** Nothing here exercises the actual panel; the wire contract
  above is what it should be tested against.
