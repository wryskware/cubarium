---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Meal continuity: checked and deployed to the same world

Live **`0.1.0+d55d8af`** adds Fable's actual-intake meal continuity, with root's
bud handoff and Astra's retained-prey boundary corrections. Short nibble gaps no
longer repeatedly switch the visible body back to its movement clip. It is a
subtle pose-continuity change, not more food, invented satiation, automatic care,
or a new rest controller. Existing plant art, wind, ecology and care settings
remain unchanged. Lore/source checks informed the boundary review and status
reconciliation; research proposals remain separate from accepted decisions.

## Frozen verification and paired visual evidence

Source `d55d8afee76f8b16b0fd15a278981fb06b94e2d0`, detached worktree
`/tmp/cubarium-meal-rollout-d55d8af`. Full workspace `--lib --tests` completed:
**1098 passed, 0 failed, 19 ignored**, 73 summaries. Release build exited0.
Logs: `/tmp/cubarium-meal-final-{tests,build}.log`. The prior8d489f7 candidate
passed1094 tests; the final four tests close the newly found capture-pose cut.
Source under `crates/cubarium-core/src` is identical to the previous live1236205.
The separate hunter cleanup correction is deliberately not in this rollout.

Binary `captures/build-cache/meal-d55d8af/release/cubarium`, SHA256
`c2c458a027090fe3ae98ed7a9f2f24902a2200d2f844824e776fd734e0ee0c29`.
Capture executable SHA256
`3533e274a885ce2c16111be17cdbedcd695484cc8fda379bcd6552ec93830f76`.

The three prescribed matched captures at `captures/meal-reviewed-8d489f7` contain
933/933/963 frame pairs for seed1/target0/Feed, seed8/target0/Feed, and seed1/no
input. Each old/new pair is drawn from one actual copied-world trajectory at
three frames per20Hz tick. Their closing hashes are respectively
`b255587eeb6b53de`, `dda40aa87ac237fe`, `ec81d804bd1669a0`.
The finald55 no-hunter recapture reproduces all126 checked PNGs around the seed1
onset byte-for-byte, and the same closing state after1000 ticks. This verifies
that the hunter-only handoff correction did not alter that ordinary-world case;
it is not an exhaustive all-world image identity claim.

Root inspected synchronized browser snapshots of all three cases at native
face resolution with nearest-neighbor enlargement, while the matched recordings
played at1×. The loop holds an identifiable feeding pose through short shuffles
and releases gradually; full-world differences are restrained and sometimes hard
to distinguish against bright foliage. The weaker seed8 response remains in the
comparison. Playback measured about60.002 callbacks/s, p95≤16.7ms/max16.8ms in
all three observations. This is sampled visual judgment and browser cadence,
not human continuous-video or room-distance physical-cube verification.

The [reusable player/packager](../../art/studies/meals/README.md) preserves exact
native frames and per-body observation context. Existing capture and package
outputs were both correctly refused on repeat invocation. Early player attempts
are retained: one used thousands of HTTP requests, and an initial animation
timestamp could precede the load origin and select a negative frame. Bundled
images and clamping that elapsed time corrected the diagnostic player. These
were not production viewer defects. Preliminary Fable captures before the final
nibble-hold change are not used as final-candidate evidence.

## Copied-world admission and pacing limits

Web-only preview PID3572538/port7396 resumed copied live snapshot552000 and its
care journal in `/tmp/cubarium-meal-preview-H0t28X`. No care was submitted. Root
inspected `/tmp/cubarium-meal-final-preview.png`, source identity and readiness.

All pacing samples remain under `/tmp/cubarium-meal-final-preview-cadence*.json`.
The first, concurrent with a screenshot session, had a~1.95s browser pause and
47.8 distinctfps despite60 host submissions. A second sample during verification
had56.9 distinctfps and19 intervals>25ms. These are not erased or conclusively
attributed to one cause. After verification, two serial candidate samples both
measured59.9 distinctfps, p95 16.8ms/max17ms, no intervals>25ms; the current live
baseline also measured59.9. This supports rollout under the observed conditions,
not a guarantee of zero stutter under arbitrary machine load.

## Same-world handover and deployed verification

Old PID3456152/build1236205 exited cleanly at **tick560883**, population106.
It reported150159 shim frames, zero coalescing/errors, and wrote
`state/world-560883.cubw`. The web-only preview stopped cleanly at559428 and the
completed capture server stopped too; ports7395/7396 are closed. The display shim
daemon, hardware mapping and calibration were not touched.

New owning runner **PID3592720**, root handle78683:

```sh
captures/build-cache/meal-d55d8af/release/cubarium run \
  --art /tmp/cubarium-meal-rollout-d55d8af/assets/atelier \
  --sink shim --mirror-web --web-port 7393 --state state \
  --fps 60 --speed 1 --care --require-resume
```

`/status` verifies exact resume from560883, expected build/sink/state identity and
advancing ticks. Checkpoint561600 independently decodes as schema12/buildd55,
care sequence5, no shower and no hunter profile/member. Original totals remain
Feed6 material/12 energy, Rain8.000000000000004 depth, and
Clean1.6964642483156285 material/2.919334361962081 energy. No new input or reset.

Journal2161→2236 bytes adds only the startup epoch. Its2161-byte original prefix
retains SHA256 `5e01def064c65a10ad223bf8fe8a601716f2bfdeaa4fa815e0b26e5687d7044c`.
Care is ready with no outstanding command. Actual live-owner viewer pacing:
**59.9 distinctfps**,60 host/drawfps, p95 16.8ms/max16.9ms, no intervals>25ms.
Evidence: `/tmp/cubarium-meal-live-cadence.json`, `/tmp/cubarium-meal-live.png`,
`/tmp/cubarium-meal-live-browser.json`, runtime `/tmp/cubarium-meal-live.log`.
This is isolated Chromium measurement, not physical panel scanout.

Local tag `checkpoint/live-meal-continuity-2026-09-13` points tod55d8af. No push
or extra backup bundle: same-schema presentation changes did not create a new
migration risk. Ordinary Git history and the world checkpoint/journal remain.

## Still open

The [charging experiment](astra-hunter-charging-results-2026-09-13.md) produced
paid births but failed a shared-target invariant and produced no adult descendants.
Its isolated correction/review precedes an unchanged full-cohort rerun, not a
live predator introduction. The reviewed3e9bc2f ambient rainfall two-hour screen
is running separately (root handle61872); no default support changed. Genuine
quiet behavior, early fragile-fauna losses, long-run viability, selective sail
coverage, and lower-priority plant history/LCD ideas remain open. This rollout
completes one visible slice, not the persistent goal.
