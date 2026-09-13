---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Optional care: implementation contract (feed, rain, clean)

Implementation contract for the bounded Feed / Rain / Clean interaction authorized
in `shared-care-implementation-handoff-2026-09-12.md`, incorporating
`astra-care-review-2026-09-12.md`. It binds the two implementation packages
(core and host) to one interface. It is not canon and changes no default ecology:
with zero input the world executes the old operations in the old order with no
new RNG draws.

## Split of ownership

- **Core** (`crates/cubarium-core`): `care.rs`, `WorldState.care`, schema 8 with
  explicit schema 7 migration, `World::apply_care`, manual rain in the water
  stage, accounting, `ecology_hash`, telemetry fields, focused tests.
- **Host** (`crates/cubarium`): `--care`, `--require-resume`, the `--fresh`
  occupancy guard, state-directory ownership lock, durable care journal, bounded
  intake, `POST /care` and `GET /care/status` on the web sink, the collapsible care
  panel in the viewer page, restart/replay, integration tests.

The host never mutates ecology itself; it hands validated, journaled commands to
the world at tick boundaries and reads receipts back.

## Doses are server-defined constants

No `WorldConfig` field changes (that would change every nested config type's
postcard shape and defeat the schema 7 migration). Doses live as documented
constants in `cubarium-core::care`:

| Constant | Value | Meaning |
| --- | --- | --- |
| `FEED_MATERIAL` | 3.0 m | material per feed, spread over the footprint |
| `FEED_ENERGY_DENSITY` | `detritus.energy_cap` (2.0 e/m at defaults) | fully edible crumbs, `De ≤ energy_cap·D` preserved |
| `FEED_HOPS` | 1 | footprint: target cell plus graph neighbours within 1 hop |
| `RAIN_DEPTH` | 0.9 d | integrated depth at the centre cell over the whole shower |
| `RAIN_HOPS` | 2 | footprint radius in graph hops |
| `RAIN_TICKS` | 120 | shower duration (6 s), raised-cosine envelope |
| `CLEAN_MATERIAL` | 2.0 m | maximum litter removed per clean |
| `CLEAN_FRACTION` | 0.5 | at most this fraction of any cell's `D` in one clean |
| `CLEAN_HOPS` | 1 | footprint |
| `FEED_ALLOWANCE` | 30.0 m | cumulative net material a world may be fed |

Footprint weights: the target cell and every cell reachable within `hops` hops on
the existing `FieldGraph` (seam crossing is the graph's; each cell appears once, so
a seam target never doubles a dose). Weight by hop distance `1 / (1 + hop)`,
normalized so the weights sum to exactly 1 over the cells that exist; at the rim
the dose still integrates to the requested amount over fewer cells.

## Commands, identity, ordering

```text
CareKind    = Feed | Rain | Clean
CareTarget  = { face: 0..4, u: [0,64), v: [0,64) }     // canonical surface point
CareCommand = { seq: u64, kind, target }               // seq assigned by the host journal
CareReceipt = { seq, tick, outcome }
CareOutcome = Applied(q) | Partial(q) | Rejected(reason)
q           = { material_in, energy_in, water_depth, material_out, energy_out,
                cells, ends_tick: Option<u64> }
```

`seq` is contiguous and monotonic: the world accepts only `seq == care.admitted_seq + 1`,
otherwise returns `Rejected` **without** changing state. A rejected command that
carried the expected `seq` still consumes it (so replay is a pure function of the
journal). Feed and Clean apply immediately at the tick boundary they are handed in
(before the next `step`). Rain registers an active shower that begins at the next
`step` and runs `RAIN_TICKS` ticks.

Host identity: a client id (browser-generated, per tab) plus a per-client monotonic
request number. Same identity and same payload returns the original receipt
(`duplicate: true`); same identity with a different payload is a `409 conflict`;
a request number not greater than the client's last seen one is `409 stale`
(unless it is an exact duplicate inside the deduplication window of the last 256
accepted commands, restored from the journal tail on start).

## Core effects and accounting

Let `w_c` be the footprint weights.

- **Feed** ("scatter food", truthfully: charged organic crumbs into litter). Per
  cell `D += m·w_c`, `De += rho·m·w_c` with `m = FEED_MATERIAL`, `rho =
  FEED_ENERGY_DENSITY`. Book `care.feed_material_in += m`, `care.feed_energy_in +=
  rho·m`, `care.allowance_used += m`. Reject when `allowance_used + m >
  FEED_ALLOWANCE`. Existing scavenging diets sense and eat it; no organism state is
  touched. Nothing pretends a plant fruited: the viewer's acknowledgement is drawn
  from the receipt (target and quantities), not from the frame.
- **Rain** ("shower"). Envelope `e_k`, `k = 0..RAIN_TICKS`, raised cosine
  normalized so `Σ_k e_k · DT == 1` exactly in whole ticks. Manual rate at tick
  `k`, cell `c`: `r = RAIN_DEPTH · w_c · e_k`. `water::step` takes an optional manual
  rate array; the rain stage adds `natural + manual` and publishes the combined
  rate in `rain[c]`, so the visible shower and the deposited water agree. The
  weather source is never mutated. Manual water is booked once in `rain_in_total`
  and separately in `care.rain_depth_in`. At most one active shower; a second rain
  is `Rejected("shower active")`. The shower (cells, weights, start tick, delivered
  depth) is in `WorldState.care.showers` and resumes after restart.
- **Clean** ("clean up litter", not detoxify). Per cell with opening `d, de`:
  `take_c = min(CLEAN_FRACTION·d, CLEAN_MATERIAL·w_c)`, `removed_de = de·take_c/d`
  (both zero if `d == 0`). Book `care.clean_material_out += Σ take_c`,
  `care.clean_energy_out += Σ removed_de`, `care.allowance_used = max(0,
  allowance_used − Σ take_c)`. `N`, `P`, `F`, `w`, organisms untouched. Outcome
  is `Partial` when `Σ take_c < CLEAN_MATERIAL`, `Rejected("nothing to remove")`
  when zero. Exported chemical energy is not heat dissipated inside the world.

Identities (`design/m2-world-spec.md` invariants, extended):

```text
mass_residual  = Σfields + Σorganisms − external_material_in − feed_material_in
                 + clean_material_out − initial_material
stored_energy  = light_in_total + feed_energy_in − heat_out_total − clean_energy_out (+ opening)
Σw             = rain_in_total − evap_out_total       (manual water inside rain_in_total)
```

`from_state` re-derives the baseline with the same terms. `validate` checks every
care counter finite and nonnegative, `allowance_used ≤ FEED_ALLOWANCE`, and every
shower's cells, weights, and progress finite and in range.

## Persistence: schema 8 and the schema 7 migration

`WorldState` gains exactly one appended field, `care: CareState`. No nested type
changes shape. `SCHEMA_VERSION = 8`. `decode_snapshot` accepts schema 7 by decoding
a frozen `WorldStateV7` mirror (the field list as of commit `cb9dc9c`) and converting
with `care = CareState::default()`; it accepts schema 8 directly; anything else is
`UnsupportedSchema`. `SnapshotMeta.schema` reports what was read.

`ecology_hash(&WorldState) -> u64` is FNV-1a over the postcard encoding of the
schema 7 projection. For a migrated world with zero care it equals the old
`state_hash` of the same world exactly, which is the cross-version comparison
Astra asked for. Telemetry's `state_hash` stays the full-state hash; telemetry
gains `ecology_hash` and the care ledgers.

Fixtures (produced by the **pre-change** release binary of the viewer commit, never
by a new struct "saved as v7"): `crates/cubarium-core/tests/fixtures/strata-v7-540000.cubw`
(a genuine snapshot from the strata world) and `strata-v7-540000-plus600.cubw`
(that world resumed for 600 ticks by the same old binary). Required tests:
migration preserves tick, config, weather, every field array, every organism, and
re-encoding the projection reproduces the original payload bytes; stepping the
migrated world 600 ticks with zero care reproduces the second fixture's payload
bytes through the projection.

## Host: durability, replay, safety

- **Journal** `<state>/care.jsonl`, append-only JSON lines, `fsync` after every
  `accepted` record before the command may be admitted. Records:
  `{"rec":"accepted","seq":N,"client":"…","request":R,"kind":"feed","target":{…},"target_tick":T}` and
  `{"rec":"outcome","seq":N,"tick":T,"outcome":"applied|partial|rejected","reason":"…","applied":{…}}`.
  A torn final line is ignored as an unaccepted suffix; an invalid interior line is a
  recovery failure that refuses to start. If an append or fsync fails, care is
  disabled for the rest of the process (every `POST /care` answers 503) and the
  world keeps running.
- **Threads.** HTTP handlers only `try_send` into a bounded intake (8). One care
  worker thread owns validation, rate limits, deduplication, the journal, and the
  receipt table. It forwards accepted commands into a bounded FIFO (8) that the
  simulation loop drains at the tick boundary, calling `World::apply_care` in seq
  order; outcomes go back to the worker for the `outcome` record. The simulation
  thread never touches the socket or the disk for care.
- **Target tick.** `target_tick = current_tick + 1` at acceptance. On restart the
  host reads the journal, sets the dedup window from its tail, and re-hands every
  `accepted` record with `seq > care.admitted_seq` to the world at its target tick;
  a target already in the past is applied at the next tick and the receipt carries
  the actual tick. A command already inside the snapshot (`seq ≤ admitted_seq`)
  is never applied twice. Crash points to test: before the accepted fsync (command
  absent, client sees no acceptance), after fsync before admission (replayed),
  mid-shower (shower resumes from the snapshot's delivered depth; journal replay
  does not restart it), after application before checkpoint (replayed from the
  journal to the same tick).
- **Rate limits** (host, evaluated in sim ticks from an atomic the loop publishes):
  per kind cooldown feed 30 s, rain 60 s, clean 30 s; at most 4 outstanding
  commands; request body ≤ 4 KiB; receipts table bounded to the last 64.
- **HTTP.** Loopback only (existing). `POST /care` requires `Content-Type:
  application/json`, the custom header `X-Cubarium-Care: 1` (a cross-origin page
  cannot send it without a preflight, which is never answered), a `Host` of
  `127.0.0.1:<port>` or `localhost:<port>`, an `Origin`, when present, matching the
  same, and the per-process session token from `GET /care/session` in the body.
  No CORS headers, ever. Mutation through GET is a 405. Responses: `202` accepted
  `{seq, target_tick}`, `200` duplicate, `400` invalid, `409` conflict/stale, `429`
  cooldown or outstanding limit, `503` intake full or care disabled. `GET
  /care/status` returns `{enabled, world_tick, outstanding, cooldowns, receipts:[…]}`.
- **State ownership.** `<state>/LOCK` holds `pid` and a start stamp; a live pid in
  `/proc` refuses the launch, a dead one is replaced. `--fresh` is refused before
  logs, workers, or sinks are opened when the directory already holds any
  `world-*.cubw`, `tmp-*.cubw`, or a non-empty `care.jsonl` (directory read errors
  propagate); the message says to choose a new directory. `--require-resume` fails
  when no snapshot loads. Without `--fresh`, a directory that holds snapshot files
  none of which load is an error, never a silent new world; an empty directory
  still creates a new world.
- **Viewer panel.** `<details>` "Care (optional)", closed by default, outside the
  cube and net images. Choose the target by clicking the net (face and pixel via
  `NET_CELL`), draw the marker on an overlay canvas, never in frame bytes. Buttons
  "Scatter food", "Shower", "Clean up litter". Each request row shows queued →
  accepted (seq, target tick) → applied / partial / rejected with quantities. Two
  tabs each generate their own client id and share the session token.

## Tests, focused

Core: zero-input projection equality (fixture pair); feed/clean exact ledgers and
the mass/energy identities before and after; clean on empty and scarce litter;
rain envelope sums to the dose to 1e-12 and `rain[c]` publishes natural+manual;
seam and rim footprints integrate exactly; seq contiguity and rejection without
state change; shower resume mid-way through a snapshot round trip. Host: journal
torn-tail and invalid-interior behaviour; duplicate, conflict, stale, burst, and
saturation over the socket; cross-origin and GET mutation refused; restart replay
of a pending command and of a mid-shower snapshot; `--fresh` guard with eight
higher-tick snapshots; lock refusal; `--require-resume`. Short matched care /
no-care scenario for a visual capture; no claim of long-run stability.
