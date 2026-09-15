# The live schema 7 and schema 8 snapshots, and their retirement

`live-v7-55200.cubw`, `live-v7-55200-plus600.cubw`, `live-v8-172800.cubw` and
`live-v8-172800-plus600.cubw` are payloads the **live display's own release builds actually
wrote** — a schema 7 world at tick 55,200 and a schema 8 world at tick 172,800, each with its
recorded next 600 ticks. `live-v7-55200-plus600-r0b.cubw` and `live-v8-172800-plus600-r0b.cubw`
are this repository's own R0b recordings of those same continuations, made when paid rotation
(R0a) and the corrected motor budget (R0b) put the old binaries' continuations out of reach.

They anchored two claims: that schema 8 loads a schema 7 world with an inert `care` and
projects back byte for byte (`tests/care.rs`), and that schema 9 does the same for the signed
energy corrections (`tests/energy_correction.rs`), each with a 600-tick continuation beside it.

## Retired by ecology v1 (2026-09-15)

Wrysk's standing rule of 2026-09-15 is that **worlds always restart fresh and are never
migrated** (`design/ecology-v1-contract.md` §15.1). Schema 16 refuses every older snapshot **by
name** (`SnapshotError::UnsupportedSchema`) rather than synthesising the wood, plant reserve and
animal-remains pools for a world that never had them.

So both claims are retired, not re-anchored: this build cannot load these payloads, and it
cannot write a schema 7 or schema 8 world to compare a fresh continuation against. The files
stay in the tree unchanged — they are genuine artefacts of the builds that wrote them — and what
the suite asserts on them now is the refusal itself
(`tests/care.rs::the_live_schema_seven_fixtures_are_refused_by_name`,
`tests/energy_correction.rs::the_pre_correction_fixtures_are_refused_by_name`, and the
whole-set check in `tests/continuation_fixtures.rs`).

New continuation fixtures, if wanted, are generated from a schema 16 world in a later
assignment. Nothing in these fixtures was ever a balance claim: they are one world's history,
kept because it reproduced.
