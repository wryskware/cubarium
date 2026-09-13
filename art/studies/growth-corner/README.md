# Growth-corner ownership study

Study-only diagnostics and a retained-final-owner candidate. No production default,
assets, ecology or surface ownership rules are changed. See
[the review](../../../design/7_Research/astra-growth-corner-review-2026-09-13.md).

`build.rs` extracts the **actual private** `draw_column` from the current host source,
then creates a second function by substituting only the cap's owning chart. It does
not expose a new host API or overwrite that source. The captured source matches
`a9eb064`; this study intentionally uses current relative dependencies, unlike the
older frozen vine study. Recheck provenance if rerunning after presenter changes.

```sh
CARGO_TARGET_DIR=captures/build-cache/vine-production cargo test --offline --manifest-path art/studies/growth-corner/Cargo.toml
CARGO_TARGET_DIR=captures/build-cache/vine-production cargo run --offline --manifest-path art/studies/growth-corner/Cargo.toml -- captures/growth-corner-2026-09-13
```

The run prints JSON and writes native original/candidate nets, native/enlarged
contact sheets, and241 paired frames. Open `viewer.html` through an existing
repository-root development file server or directly where local file images work.
Playback is a synthetic four-second growth sweep, not real-world ecology/cadence.

Original contact columns: forced Front0 spire, forced Right15 spire, selected Left0
glasscane. Rows: height53/6 plus `[-.0075,-1e-8,+1e-8,+.0075]` at fixed phase.
Selected-column handoff sheet rows: heights7,8,53/6,9. Each row has original
`Top corner | Left crown`, then candidate `Top corner | Left crown`.

Three tests deliberately assert the existing regression remains reproducible;
three test the isolated candidate. A passing suite does **not** mean production
has been fixed. The original discontinuity must not be removed from evidence.
