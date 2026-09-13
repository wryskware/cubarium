---
design_status: exploration
last_reviewed: 2026-09-13
---

# Crown continuity: live update

Build **0.1.0+1267e3b** is running the existing cube and shared viewer on7393.
This is a presentation-only release, not a quiet-policy, ambient-support or
hunter deployment. The previous live build was3147775. Read the
[integration evidence](fable-growth-corner-integration-2026-09-13.md) for the
bounded visual change and its limitations.

## What changed

Only the shipped spiretree and glasscane crown rows opt into the versioned
final-position chart owner near upper corners. This removes a discontinuous
ownership change during growth. Mature endpoints, ordinary interior/lower caps,
unflagged custom assets and the ecology retain their prior paths. The exporter
preserves the selector through fresh Godot bakes; all five atlas PNGs are unchanged.
This does not establish persistent individual plant age or eliminate all flicker.

## Release and checks

- Clean release branch `release/crown-owner-2026-09-13`, commit
  `1267e3ba39764723114b6905ed963749fa86e4da`, under
  `captures/release-source/crown-owner-2026-09-13`.
- Immutable executable `captures/releases/1267e3b/cubarium`, SHA256
  `7a0b3867926c38ea2f7db705ae95262ef985d3fb02d5362c52fb417e4afb5742`.
- Pack SHA256
  `4f6c12749d0ff1d147a3acaa5cd902daeb68ee11573194fd10c38d7dfdefebeb`.
- Optimized offline workspace library/integration suite:1126 passed,0 failed,
  19 ignored across75 target summaries; subsequent release build exited0.
  Logs: `/tmp/cubarium-crown-1267e3b-workspace-tests.log` and
  `/tmp/cubarium-crown-1267e3b-build.log`.
- Fresh Godot output matches production pack and atlases byte-for-byte.
  Six pinned historical crown-study tests also pass, preserving the original
  failure fixture rather than silently testing the already-fixed presenter.

The first release-assembly attempt lacked historical study dependencies and was
aborted in the owned release checkout. Its sandboxed test attempt failed on socket
permissions; it was not the tested release or a live update. The clean1267e3b
suite above was run separately with the required host permissions.

## Copied-world preview and exact handover

Preview used a copy of sealed tick769200 and the care journal, not a new world.
The source checkpoint SHA256 was
`fd93b7087eeab6c9bea0d34f6da86c66a3bb1105a0d3271f03a11e384c84c15d`.
The isolated browser screenshot looked intact; ten-second cadence measured
59.8994 distinct frames/s. Preview artifacts are under
`captures/crown-release-preview-1267e3b`, with screenshot and browser/cadence JSON
at `/tmp/cubarium-crown-1267e3b-preview*`. This copied preview was stopped with
SIGTERM (exit143), not the runner's graceful SIGINT path; no final preview save
is claimed.

The real owning runner was stopped with SIGINT and exited0, saving tick775478.
The new process resumed **that exact checkpoint**, not the older preview copy.
The inspected schema12 checkpoint contains122 organisms, no hunter members or
profile, no quiet-policy state, and the prior durable care accounting. No live
care actions were issued for verification.

The journal's entire2386-byte prefix was preserved, SHA256
`5cacde97b3f1b4875b91cd56635873a556229041bd8aeadd185650beddd0639e`;
the only handover addition was the75-byte new-build epoch record. Care reports
ready with no outstanding requests and unchanged250–2000permille dose range.
New source PID4107126 identifies build1267e3b, sinkshim, speed1 and the existing
`/home/wrysk/wryskware/cubarium/state` directory. An additional read-only status
check confirmed progress to tick784035. Ports7395/7396 have no listeners.

## Live cadence and limits

The isolated Chromium ten-second sample measured60 host submissions, animation
callbacks and canvas draws per second; **59.9 distinct net frames/s**. New-frame
interval p95 was16.8ms, maximum17.1ms, and none exceeded25ms.
Evidence: `/tmp/cubarium-crown-1267e3b-live-cadence.json` and
`/tmp/cubarium-crown-1267e3b-handover-after.json`.

These are browser/host measurements, not physical LED scanout or a personal-browser
measurement. Static inspection and synthetic growth tests do not establish the
frequency or perceptual salience of this corner event in the live ecosystem.
The same world remains available for the owner's physical viewing.

No redundant state backup or remote push was made. The local lightweight release
tag is `checkpoint/live-crown-continuity-2026-09-13` at1267e3b.
