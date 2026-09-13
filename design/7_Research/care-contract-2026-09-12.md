---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Optional care: implementation contract (feed, rain, clean), revision 3

Implementation contract for the bounded Feed / Rain / Clean interaction authorized
in `shared-care-implementation-handoff-2026-09-12.md`. Revision 2 resolved the
review gate root recorded there and adopted every correction in
`astra-care-contract-review-2026-09-12.md`: admission commits at a held simulation
boundary, an uncertain journal write never lets ecology advance past that boundary,
state ownership is an OS advisory lock, the journal has a hard bound with
server-issued client identities, and the rain dose is the total over the footprint.
Revision 3 adopts Astra's follow-up on abort recovery: there is no abort-and-resume
path; an uncertain write holds the world at its boundary until a clean stop, and
recovery validates the whole replay schedule before stepping (host steps 4, 6, 7).
It binds the core and host packages to one interface. It is not canon and changes
no default ecology: with zero input the world executes the old operations in the
old order with no new RNG draws.

## Split of ownership

- **Core** (`crates/cubarium-core`): `care.rs`, `WorldState.care`, schema 8 with an
  explicit schema 7 migration, `World::apply_care` / `World::void_care`, manual rain
  inside the water stage, accounting, `ecology_hash`, telemetry fields, focused tests.
- **Host** (`crates/cubarium`): `--care`, `--require-resume`, the `--fresh`
  occupancy guard, the state ownership lock, the durable care journal, bounded
  intake, held-boundary admission in the runner, `POST /care/register`,
  `POST /care`, `GET /care/status` on the web sink, the collapsible care panel in
  the viewer page, restart/replay, integration tests.

The host never mutates ecology itself; it hands journaled commands to the world at
a held tick boundary and reads receipts back.

## Doses are server-defined constants

No `WorldConfig` field changes (that would change every nested config type's
postcard shape and defeat the schema 7 migration). Doses live as documented
constants in `cubarium-core::care`:

| Constant | Value | Meaning |
| --- | --- | --- |
| `FEED_MATERIAL` | 3.0 m | total material per feed, summed over the footprint |
| `FEED_ENERGY_DENSITY` | `detritus.energy_cap` of the world's config (2.0 e/m at defaults) | fully edible crumbs; `De ≤ energy_cap·D` preserved |
| `FEED_HOPS` | 1 | footprint: target cell plus graph neighbours within 1 hop |
| `RAIN_DEPTH_TOTAL` | 4.0 d | total water depth per shower, summed over the footprint and the whole envelope |
| `RAIN_HOPS` | 2 | footprint radius in graph hops |
| `RAIN_TICKS` | 120 | shower duration (6 s); exactly 120 discrete samples |
| `CLEAN_MATERIAL` | 2.0 m | maximum litter removed per clean, summed over the footprint |
| `CLEAN_FRACTION` | 0.5 | at most this fraction of any one cell's `D` in one clean |
| `CLEAN_HOPS` | 1 | footprint |
| `FEED_ALLOWANCE` | 30.0 m | cumulative net material a world may be fed |

Footprint: the target cell and every cell reachable within `hops` hops on the
existing `FieldGraph` (breadth-first; seam crossing is the graph's; each cell
appears once, so a seam target never doubles a dose). Weight by hop distance
`1 / (1 + hop)`, normalized over the cells that exist so `Σ w_c = 1` to within
1e-12; at the rim the same total dose lands on fewer cells. Every dose above is a
TOTAL over the footprint; the centre cell receives `dose · w_center`.

## Commands, identity, ordering

```text
CareKind    = Feed | Rain | Clean
CareTarget  = { face: 0..4, u: [0,64), v: [0,64) }     // canonical surface point
CareCommand = { seq: u64, apply_after_tick: u64, kind, target }
CareReceipt = { seq, tick, outcome }                   // tick == apply_after_tick
CareOutcome = Applied(q) | Partial(q) | Rejected(reason)
q           = { material_in, energy_in, water_depth, material_out, energy_out,
                cells, ends_tick: Option<u64> }
```

`apply_after_tick = B` names the boundary with exactly `B` completed ticks: the
command is applied when `world.tick() == B`, before the step that produces `B + 1`.
Application, receipts, journal records, snapshots, and replay all use this one
definition.

Core admission rules (`World::apply_care(&CareCommand) -> CareReceipt`):
- `seq` is contiguous: only `seq == care.admitted_seq + 1` is accepted; any other
  `seq` returns `Rejected("out of order")` **without** changing state.
- `world.tick()` must equal `apply_after_tick`; otherwise `Rejected("wrong
  boundary")` without changing state (the host treats this as a recovery failure).
- A command with the expected `seq` at the right boundary always consumes it, even
  when its own validation rejects it (allowance exhausted, shower active, nothing to
  remove). `World::void_care(seq)` consumes a seq with no effect; the host uses it
  for durably aborted records so replay stays a pure function of the journal.
- Feed and Clean change fields immediately at the boundary. Rain registers an
  active shower whose first sample falls in the step `B → B + 1`.

Host identity: the server issues client identities (`POST /care/register` →
`{client, epoch}`); a request carries `client` and a per-client monotonic
`request`. Same identity and same payload returns the retained receipt
(`duplicate: true`); same identity with a different payload is `409 conflict`; a
`request` not above the client's high-water mark is `409 stale`; an unknown or
retired identity is `409 retired` and is never re-registered by a `POST /care`.

## Core effects and accounting

Let `w_c` be the footprint weights.

- **Feed** ("scatter food"; truthfully, charged organic crumbs into litter). Per
  cell `D += m·w_c`, `De += rho·m·w_c` with `m = FEED_MATERIAL`, `rho =
  FEED_ENERGY_DENSITY`. Book the actual f64 sums added:
  `care.feed_material_in += Σ_c m·w_c`, `care.feed_energy_in += Σ_c rho·m·w_c`,
  `care.allowance_used += Σ_c m·w_c`. Reject with `Rejected("allowance exhausted")`
  when `allowance_used + m > FEED_ALLOWANCE`. Existing scavenging diets sense and
  eat it; no organism state is touched. Nothing pretends a plant fruited: the
  viewer's acknowledgement is drawn from the receipt, never from the frame.
- **Rain** ("shower"). Exactly 120 samples, `k = 0..=119`:
  `e_k = (1 − cos(2π (k + 0.5) / 120)) / S` with `S = Σ_k (1 − cos(2π (k + 0.5) / 120)) · DT`,
  so `Σ_k e_k · DT = 1` to within 1e-12. Manual rate in the step that produces tick
  `B + 1 + k`, cell `c`: `r_{c,k} = RAIN_DEPTH_TOTAL · w_c · e_k` (d/s).
  `water::step` takes an optional manual rate array; the rain stage adds
  `natural + manual`, publishes the combined rate in `rain[c]`, and books the actual
  f64 depth added. The weather source is never mutated. Manual water is inside
  `rain_in_total` once and also counted in `care.rain_depth_in`. At most one active
  shower; a second rain is `Rejected("shower active")`. The shower (cells, weights,
  `apply_after_tick`, samples delivered) lives in `WorldState.care.showers` and
  resumes from a snapshot at the next undelivered sample.
- **Clean** ("clean up litter", not detoxify). Per cell with opening `d, de`:
  `take_c = min(CLEAN_FRACTION·d, CLEAN_MATERIAL·w_c)`, `removed_de = de·take_c/d`
  (both zero if `d == 0`). Book `care.clean_material_out += Σ take_c`,
  `care.clean_energy_out += Σ removed_de`, `care.allowance_used = max(0,
  allowance_used − Σ take_c)`. `N`, `P`, `F`, `w`, organisms untouched. `Partial`
  when `0 < Σ take_c < CLEAN_MATERIAL`, `Rejected("nothing to remove")` when zero.
  Exported chemical energy is not heat dissipated inside the world.

Identities (`design/m2-world-spec.md` invariants, extended):

```text
mass_residual  = Σfields + Σorganisms − external_material_in − feed_material_in
                 + clean_material_out − initial_material
stored_energy  = opening + light_in_total + feed_energy_in − heat_out_total − clean_energy_out
Σw             = rain_in_total − evap_out_total       (manual water inside rain_in_total)
```

`from_state` re-derives the baseline with the same terms. `validate` checks every
care counter finite and nonnegative, `allowance_used ≤ FEED_ALLOWANCE`, and every
shower's cells, weights, boundary, and progress finite and in range.

## Persistence: schema 8 and the schema 7 migration

`WorldState` gains exactly one appended field, `care: CareState`. No nested type
changes shape. `SCHEMA_VERSION = 8`. `decode_snapshot` accepts schema 7 by decoding
a frozen `WorldStateV7` mirror (the field list as of commit `e3ad20f`) and converting
with `care = CareState::default()`; it accepts schema 8 directly; anything else is
`UnsupportedSchema`. `SnapshotMeta.schema` reports what was read.

`ecology_hash(&WorldState) -> u64` is FNV-1a over the postcard encoding of the
schema 7 projection. For a migrated world with zero care it equals the old
`state_hash` of the same world exactly. Telemetry's `state_hash` stays the
full-state hash; telemetry gains `ecology_hash` and the care ledgers.

Fixtures, produced by the **pre-change** binaries only (never a new struct saved
as schema 7): `crates/cubarium-core/tests/fixtures/live-v7-55200.cubw` (a copy of
root's genuine `captures/checkpoints/pre-care/world-55200.cubw`) and
`live-v7-55200-plus600.cubw` (that world resumed for 600 ticks by
`/tmp/cubarium-viewer-v7-e3ad20f/cubarium`, the schema 7 release build). Required
tests: migration preserves tick, config, weather, every field array, every organism,
and re-encoding the projection reproduces the original payload bytes; stepping the
migrated world 600 ticks with zero care reproduces the second fixture's payload
bytes through the projection.

## Host: admission at a held boundary

1. HTTP handlers validate shape, identity, rate limits and `try_send` into a bounded
   prepared FIFO (8). They never touch the disk.
2. At a boundary with exactly `B` completed ticks the runner drains the prepared
   FIFO (in arrival order), assigns the next contiguous `seq` values and
   `apply_after_tick = B`, hands the records to the journal worker, and **holds
   tick advancement at B**. Rendering, `/frame`, `/status`, and `/care/status` keep
   serving during the hold; `/care/status` reports `holding_at: B`.
3. The journal worker appends every record and `fsync`s, then acknowledges. Only on
   the acknowledgement does the runner call `apply_care` for each record in `seq`
   order at `B`, hand the receipts back for diagnostic `outcome` records, answer the
   waiting HTTP clients with `202 {seq, apply_after_tick}`, and resume stepping. The
   clock re-bases after the hold instead of fast-forwarding.
4. An append or `fsync` error after a scheduled write was attempted is ambiguous:
   the record may survive. **Revision 3 (after Astra's follow-up on abort recovery):
   there is no abort-and-resume path in this slice.** The runner holds at `B`,
   refuses further care, reports on stderr and in `/care/status`
   (`"care": "failed", "holding_at": B, "reason": …`), keeps rendering and serving,
   and permits only a clean stop at `B` (Ctrl-C writes the final snapshot at `B`).
   Nothing is ever appended after an uncertain write in the same process, and a
   timeout is never proof that a record is absent. Failures before any scheduled
   write is attempted (the journal-full precheck) reject care and keep running.
   This is an explicit availability trade: a disk failure pauses the world until a
   clean stop and recovery. `World::void_care` remains available for tests and
   future protocols but is not used on the live path.
5. Journal `<state>/care.jsonl`, append-only JSON lines. Records:
   `{"rec":"epoch","epoch":"<process start stamp>","build":"…"}` at every start,
   `{"rec":"accepted","seq":N,"apply_after_tick":B,"client":"…","request":R,"kind":"feed","target":{…}}`,
   `{"rec":"abort","seq":N}`, `{"rec":"outcome","seq":N,"tick":B,"outcome":"applied|partial|rejected","reason":"…","applied":{…}}`.
   On open, a torn final line is truncated back to the verified prefix before any
   append (the prefix is preserved; the truncation is logged); an invalid interior
   line is a recovery failure that refuses to start. The journal has a hard bound of
   4 MiB: once the file, plus a reserve of 512 bytes for each accepted-but-not-yet-
   outcome record, would exceed it, further care is refused with `503 journal
   full` while autonomous life continues; the bound and usage are in `/care/status`.
   No compaction in this slice.
6. Replay on start: verify the journal prefix (truncate only a torn final suffix,
   logged; interior corruption refuses to start), take the `accepted` records with
   `seq > care.admitted_seq` of the loaded snapshot, and validate the whole schedule
   before stepping: contiguous `seq` after the cursor, nondecreasing `B` in `seq`
   order, no `B` below the snapshot tick `S`. Entries are applied when the world
   reaches `B`, before advancing to `B + 1`, each exactly once; any unexpected core
   rejection during replay is a recovery error that stops the process. Reserved
   sequences absent from the surviving journal were never applied and are not
   consumed; after replay, new allocations start after the surviving journal's
   maximum `seq`. While the recovered schedule still holds future commands,
   `POST /care` answers `503 {"care": "replaying"}`. A shower already in the
   snapshot is not restarted by replay. A newly created world enables care only
   after its opening checkpoint is durably written; a journal-only directory is
   occupied and never silently creates a world from defaults.
7. Crash points that must be tested: before the accepted `fsync` returns (no
   application in the original process; a complete record that survived is applied
   at `B` on recovery even though the client never saw acceptance), injected short
   writes mid-record and between two batch records (the world stays held at `B`;
   restart replays exactly the surviving complete records), a new request during
   replay (`503 replaying`), after `fsync` before application (replayed at `B`), after
   application before any outcome record or checkpoint (replayed at `B` from an
   older snapshot and ecology matches an uninterrupted run at the same final tick),
   mid-shower (resumes from the snapshot's delivered samples), and an injected
   delayed acknowledgement (the world provably stays at `B` and applies once).
   Comparisons use `ecology_hash`.

## Host: rate limits, HTTP, ownership

- **Rate limits** (host, evaluated in sim ticks from an atomic the loop publishes):
  per kind cooldown feed 30 s, rain 60 s, clean 30 s; at most 4 outstanding
  commands; request body ≤ 4 KiB; receipts table bounded to the last 64; at most
  64 registered clients per process (`429 too many clients` afterwards).
- **HTTP.** Loopback only (existing). `POST /care/register` and `POST /care` require
  `Content-Type: application/json`, the custom header `X-Cubarium-Care: 1` (a
  cross-origin page cannot send it without a preflight, which is never answered), a
  `Host` of `127.0.0.1:<port>` or `localhost:<port>`, and an `Origin`, when present,
  matching the same. No CORS headers, ever. Any mutation through GET is `405`.
  Responses for `POST /care`: `202` accepted `{seq, apply_after_tick}` (sent only
  after the durable acknowledgement; a client waiting more than 5 s gets `202
  {pending: true}` and reads its result from `/care/status`), `200` duplicate,
  `400` invalid, `409` conflict / stale / retired, `429` cooldown or limits, `503`
  intake full, journal full, care failed, or care disabled. `GET /care/status`
  returns `{enabled, care, holding_at, epoch, world_tick, outstanding, cooldowns,
  journal: {bytes, limit}, receipts: [...]}`.
- **Epochs.** The epoch is the process start stamp written in the journal's first
  record for this run. Client identities embed it; after a restart every earlier
  identity is retired and a page must register again. A request that was in flight
  across the restart is either in the journal (and will be replayed) or was never
  accepted; the page shows it as "unknown after restart, check the receipts".
- **State ownership.** `<state>/.lock` is opened (created if absent, never unlinked
  or replaced) and locked with an exclusive, non-blocking OS advisory lock
  (`flock(2)` through the `libc` crate) before the occupancy check, before loading,
  and before any persistent write; the handle is held for the whole run and released
  only after the checkpoint worker has stopped. A second launcher gets a clear
  refusal naming the directory. Pid and start stamp are diagnostic content of the
  locked file only.
- **`--fresh` guard.** Refused, before logs, workers, or sinks are opened, when the
  directory already holds any `world-*.cubw`, `tmp-*.cubw`, or a non-empty
  `care.jsonl`; directory read errors propagate; the message says to choose a new
  directory. `--require-resume` fails when no snapshot loads. Without `--fresh`, a
  directory that holds snapshot files none of which load is an error, never a silent
  new world; an empty directory still creates a new world. The existing test
  `a_snapshot_that_is_not_a_snapshot_at_all_is_skipped_too` encodes the old
  behaviour and changes to expect the error (the handoff authorizes this).
- **Viewer panel.** `<details>` "Care (optional)", closed by default, outside the
  cube and net images. Choose the target by clicking the net (face and pixel via
  `NET_CELL`), draw the marker on an overlay canvas, never in frame bytes. Buttons
  "Scatter food", "Shower", "Clean up litter". Each request row shows queued →
  accepted (seq, boundary) → applied / partial / rejected with quantities, plus the
  hold and failure states above. Two tabs register separately and share nothing but
  the epoch.

## Tests, focused

Core: zero-input projection equality (fixture pair); feed/clean exact ledgers and
the mass/energy identities before and after; clean on empty and scarce litter;
rain samples sum to the dose within 1e-12, `rain[c]` publishes natural+manual, and
per-cell and summed delivery for an ordinary, a seam, and a rim target; seq
contiguity and wrong-boundary rejection without state change; shower resume
mid-way through a snapshot round trip. Host: journal torn-tail truncation and
invalid-interior refusal; duplicate, conflict, stale, retired, burst, saturation,
and journal-full over the socket; cross-origin and GET mutation refused; the hold
under an injected delayed acknowledgement; the crash points listed above; two
concurrent launchers into one empty directory with exactly one owner and the lock
reusable after the owner is killed; `--fresh` guard with eight higher-tick
snapshots; `--require-resume`. Short matched care / no-care scenario for a visual
capture; no claim of long-run stability.
