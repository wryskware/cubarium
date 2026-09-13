---
design_status: exploration
last_reviewed: 2026-09-13
---

# Calm sail fins: verified same-world release

**0.1.0+6d7a831 is live** on the cube and shared viewer7393, resumed from the
previous owner's exact final tick801935. This presentation-only slice changes
four sail rest/feed fin tracks and two creature-atlas rows, not organism behavior,
ecology, care, AA, physics, persistence or the crown/vine contracts.

## Visual disposition

Astra studied original/brace/settle in `bc7b2a9`; Fable reviewed and adopted brace
in `d7e61df`. See the [study](astra-sail-calm-study-2026-09-13.md) and
[independent review](fable-sail-calm-review-2026-09-13.md). Root corrected the
review's prose in `a3367b8`: real-meal seed1's worst crop step rises120→123, while
seed8 stays106; the original “does not rise” sentence contradicted its table.

Rest replaces continuous point-baked fin-tip flutter with one peak pose, faded
in/out over0.5s once per4s. Feed keeps fins braced while the original body chew
continues. Mean temporal variation improves, but the isolated rest gesture's
largest single-frame change is about3× larger. This is a deliberate occasional
gesture, not a uniformly smoother articulated swing or a physiological pause.
Move, bud, body tracks and all non-sail atlas rows are unchanged; settle is rejected.

Root inspected source pose/contact sheets and the actual browser comparison at
frame180, with native192×64 and enlarged canvases. Screenshot and load/frame
verification: `/tmp/cubarium-sail-calm-root-peak.{png,json}`. This is held-frame
inspection, not a claim of watching temporal playback or viewing physical panels.
The copied running world looked intact; its screenshot is retained below.

## Frozen release and verification

Clean branch `release/sail-calm-2026-09-13` at
`6d7a831c2c9000e79b19cd152cc58d6e29f6d657`, assembled on deployed1267e3b with the
study, adoption and review correction only. Source:
`captures/release-source/sail-calm-2026-09-13`. Core/host source and pack metadata
diffs against1267e3b are empty; quiet/schema13 and hunter experiments are excluded.

- Optimized offline workspace library/integration suite: **1130 passed,0 failed,
  19 ignored**,76 target summaries. Release build subsequently exited0.
- Logs: `/tmp/cubarium-sail-calm-6d7a831-workspace-tests.log` and
  `/tmp/cubarium-sail-calm-6d7a831-build.log`.
- Immutable `captures/releases/6d7a831/cubarium`, SHA256
  `7b350f9c7e220fc0750458e24b6cc2121940aba6818e5ef3765174fadbfae794`.
- Creature atlas SHA256
  `3b1c3e44483b711928e984b8ad2c5c6695b2e26756679961022ffb11b489bf22`,
  matching the studied brace atlas. Current-source bake preserves all other files.
- Unchanged pack SHA256
  `4f6c12749d0ff1d147a3acaa5cd902daeb68ee11573194fd10c38d7dfdefebeb`.

## Preview and handover

Preview copied only sealed live checkpoint798000 and its care journal into
`captures/sail-calm-release-preview-6d7a831`. The checkpoint/source SHA256 matches:
`a1fd32d2ea746d7157694527640ab91dd20839a75023da5ad4daf2060f81ac25`.
No input was issued. The browser confirmed build6d7a831, exact resume798000,
advancing ticks and care ready. A10s sample measured59.8994 distinctfps,
new-frame p9516.8ms/max17ms, no intervals>25ms. Screenshot/status/cadence are
`/tmp/cubarium-sail-calm-6d7a831-preview*`. Preview was SIGINT-stopped, exit0,
at799859; its state never replaced the live world.

Verified old owner PID4107126, build1267e3b, immutable executable path, shim sink,
existing state directory and ready/empty care queue before SIGINT. It exited0 at
**801935**, population146,79353 shim frames sent,0 coalesced/errors. The new runner
PID4153282/root session40940 resumed `state/world-801935.cubw`, not the preview.
Post-handover status advanced to803264, speed1, same state directory and shim sink.

The actual saved checkpoint decodes as schema12, care sequence5, Feed6 material/
12 energy, Rain8.000000000000004 depth, Clean1.6964642483156285 material/
2.919334361962081 energy, no active shower, no hunter profile/members and no quiet
state. Its146 organisms remain the continuing world's history, not a new seed.

All2461 original care-journal bytes were preserved, prefix SHA256
`77344c441e5b31e3419dfaeb4253b063817d7309fbca89ca03de82f6020101ca`.
The only addition is the75-byte new-build epoch record, total2536 bytes. Care
reports ready, no outstanding input and unchanged250–2000permille dose bounds.
Verification JSON: `/tmp/cubarium-sail-calm-6d7a831-handover-{before,after}.json`;
decoded checkpoint: `/tmp/cubarium-sail-calm-6d7a831-resumed-checkpoint.json`.
No redundant backup, new care command, biological experiment or hardware change.

## Live viewer and checkpoint

Actual shared-viewer10s sample:59.8994 distinctfps,59.9994 host/callback/drawfps,
new-frame p9516.8ms, maximum17ms, zero intervals>25ms.
`/tmp/cubarium-sail-calm-6d7a831-live-cadence.json` identifies the same PID/build/
state/tick source. These are isolated Chromium measurements, not LED scanout or
the owner's personal-browser pacing. Ports7395/7396 are closed.

Local lightweight tag `checkpoint/live-sail-calm-2026-09-13` points to6d7a831.
No push. The full goal remains open: actual quiet habits, long-run diversity,
optional-support balance and sustainable rare hunter recruitment are separate
unfinished biological requirements.
