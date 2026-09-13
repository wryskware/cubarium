---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Vine wind: same-world live update

The cube and shared viewer now run **`0.1.0+3147775`**, resumed from the existing
world at exactly **tick707292**. This presentation-only release preserves schema12,
world history, care amounts, ambient support and ecological behavior. Quiet-policy
experiments and hunter introduction remain off the cube.

## Visible change and its limits

The [vine integration](vine-wind-integration-2026-09-13.md) enables the existing
spiretree wind response when a vine is attached. Its derived trunk and endpoint
remove the old restrictive vine budget without discarding visible endpoint pixels.
The spire's desired0.9px bend is admitted; glasscane remains limited by its own
unchanged artwork. All five atlas images are byte-identical to the previous release.
Only additive vine metadata, loading, budget calculation and stamping changed.

The [Astra study](astra-vine-wind-study-2026-09-13.md),
[Fable temporal review](fable-vine-wind-review-2026-09-13.md), and
[independent chart-helper review](renderer-chart-owned-stamp-review-2026-09-13.md)
precede this release. Their controlled quiet/equal-amplitude and physical-support
checks are bounded evidence, not proof of every possible custom sprite or corner.
The native64 visual gain is modest. Residual rest/feed fin twinkle, constrained
glasscane wind, and the pre-existing moving-crown corner discontinuity remain open.
The latter also appears on the old path; this release does not claim to fix it.

## Frozen release verification

Clean branch `release/vine-endpoint-2026-09-13` at3147775 is based on9cf0e1d,
with only these scoped changes cherry-picked:

- 33d1119 → 962410d: guarded chart-owned endpoint stamping and three tests.
- 1d648bf → 8b89f69: independent renderer tests/review.
- a9eb064 → 3147775: explicit intact-atlas vine integration, tests and docs.

No moving-main core or schema13 changes were included. Source is frozen at
`captures/release-source/vine-endpoint-2026-09-13`; the installed executable is
`captures/releases/3147775/cubarium`, separate from the reusable Cargo cache.

```text
binary SHA256    b27460b7b1dfb25ca195c1f69c4e56693479339f4e2e054541564465f5542dd2
pack.json SHA256 836888ff876369623b19f4dbeee6016a43045e9980f251d2e389cc7bf1f13ae2
```

`cargo test --offline --release --workspace --lib --tests` completed with
**1115 passed, 0 failed, 19 ignored**, across75 target summaries. Local socket
access was enabled for isolated web tests. The optimized build also exited0.
Logs: `/tmp/cubarium-vine-3147775-{workspace-tests,build}.log`.
The build retains the existing unused `AUDIT_TOLERANCE` warning. A subsequent
`--version` diagnostic was refused by this CLI; `/status` supplies the build ID.

The independent current-main host test result in the integration report is a
different scope; it is not substituted for these frozen release tests.

## Copied-world check and handover

Preview directory `captures/vine-release-preview-3147775` received only a sealed
live checkpoint at700800 and its care journal for testing. Its snapshot SHA256
matched the live source exactly. The new binary resumed it with `--require-resume`
on7396, and root inspected the browser's cube projection and net. It then stopped
cleanly at703862, exit0. This was a test copy, not a redundant rollback bundle.

Before handover, root checked old PID3771509, its immutable9cf executable, shim
sink, live state directory, and care readiness with no outstanding input. It
stopped cleanly at707292, exit0, reporting222192 shim frames sent with zero
coalesced and zero errors. New PID**3940022** reports that exact start tick and
`resumed_from: state/world-707292.cubw`, with the same state directory and shim sink.

```text
captures/releases/3147775/cubarium run \
  --art captures/release-source/vine-endpoint-2026-09-13/assets/atelier \
  --sink shim --mirror-web --web-port 7393 --state state \
  --fps 60 --speed 1 --care --require-resume
```

The shared viewer remains `http://127.0.0.1:7393/`;
ports7395 and7396 are closed after the preview. The shim daemon and hardware
mapping were consulted and left unchanged.

Semantic decoding of the final old checkpoint confirms admitted care sequence5,
Feed6 material/12 energy, Rain8 accumulated depth, Clean1.6964642483156285 material
and2.919334361962081 energy, no active showers, no hunter profile or members.
The complete2311-byte pre-handover journal prefix retains SHA256
`31f487d731fe7dda46977ff5f1264b096ab493c6cf7ae6426c7ac5f692b6ab3b`.
The journal gained only the normal75-byte new-owner epoch record, to2386 bytes.
No care command, migration, reseed, or support adjustment was submitted.

## Delivery measurements, including the uneven first sample

Separate isolated Chromium measurements (no simultaneous screenshot capture):

| Sample | Host / rAF / net draw fps | Distinct net fps | New-frame p95 / maximum | Intervals over25ms |
| --- | --- | --- | --- | --- |
| Copied world,10s | 60 /60 /60 | 59.8994 | 16.8 /16.9ms | 0 |
| Live first,10s | 60 /60 /59.4994 | 58.8994 | 16.8 /52.3ms | 8 |
| Live repeat,20s | 60 /60 /60 | 59.95 | 16.8 /17.0ms | 0 |

The first live sample is retained, not replaced by the better repeat. Its
delivery stalls have not been causally attributed; these samples do not prove
perfect pacing under every workload. Host submission remained60fps in both.
They measure browser callbacks and frame delivery, not physical panel scanout,
the user's personal browser, or subjective continuous-motion quality.

Artifacts: `/tmp/cubarium-vine-3147775-{preview,live}-browser.json`, corresponding
PNGs, `{preview,live}-cadence.json`, `live-cadence-repeat.json`, and
`handover-before.json`. Runtime log: `/tmp/cubarium-vine-3147775-live.log`.

Lightweight local tag: `checkpoint/live-vine-wind-2026-09-13` →3147775.
No push, state-backup bundle, or material deletion. This is one deployed slice,
not completion of the animation, ecological-diversity or whole-thread goal.
