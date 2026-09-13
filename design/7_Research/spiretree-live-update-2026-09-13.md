---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Spiretree wind: verified and deployed to the existing world

Live **`0.1.0+1236205`** includes Fable's centred spiretree dome and shifted
trunk-strip registration. Bare spires lean more within the unchanged nine-pixel
footprint; vines still limit their hosts' amplitude. Global wind timing/quiet
interval and other tall atlas rows are unchanged. See the
[implementation/capture record](spiretree-wind-room-2026-09-13.md) and
[independent review](astra-spire-wind-review-2026-09-13.md).

## Verification

Root froze commit `123620541376ef4d889659c8cde2c00bcb756d53` in detached
`/tmp/cubarium-spire-rollout-1236205`. Complete workspace `--lib --tests`:
**1079 passed, 0 failed, 19 ignored**, 70 summaries. Release build succeeded.
Logs: `/tmp/cubarium-spire-root-retry-{tests,build}.log`.

The first cold build terminated because `/tmp` filled, before verification
completed; it was not an animation test failure. Root removed only three inactive
incremental compiler caches (about1.4GiB freed), preserving source, recorded
executables, snapshots, journals and experiment artifacts. These caches are
regenerable. The retry used spacious workspace storage at
`captures/build-cache/spire-1236205`; subsequent workers use that filesystem too.

Executable: `captures/build-cache/spire-1236205/release/cubarium`, SHA256
`875f872fee38520c2ca4a5e5fe4f810cec93227dafa207d331c8b4a33f95b8f4`.
Art comes from the frozen worktree, not subsequent meal-animation edits.

Copied-world preview `/tmp/cubarium-spire-preview-b7naJG` loaded exact live
snapshot506400 and care journal, web-only7395/PID3450937. Root inspected native
comparison captures and `/tmp/cubarium-spire-preview.png`. A10-second isolated
browser measurement recorded59.899 distinctfps, new-frame p95 16.8ms/max17ms,
zero intervals over25ms. Preview stopped cleanly at508433; no care input sent.

## Same-world handover

Old owner PID3296401/build9598044 stopped cleanly at **tick510790**, population105,
writing `state/world-510790.cubw`. Terminal report:161285 shim frames, zero
coalesced and zero send errors. The shim daemon and hardware mapping were untouched.

New owner **PID3456152**, root execution handle53556:

```sh
captures/build-cache/spire-1236205/release/cubarium run \
  --art /tmp/cubarium-spire-rollout-1236205/assets/atelier \
  --sink shim --mirror-web --web-port 7393 --state state \
  --fps 60 --speed 1 --care --require-resume
```

`/status` confirmed exact resume from510790, expected source identity, then
advancing ticks511178/511278. New checkpoint511200 independently decoded as
schema12, care sequence5, no active shower, no hunter profile or member. All prior
care ledgers are unchanged: Feed6 material/12 energy, Rain8 depth (rounding
retained), Clean1.6964642483156285 material/2.919334361962081 energy.

The journal grew2086→2161 bytes only by its new startup epoch. Its old2086-byte
prefix still hashes to
`5ac52ecc039dd5f5df50e0e68fd8a8f07041d206f256c22d0c851473b50694da`.
Care is ready, with no outstanding commands. No user history reset or new input.

Root inspected `/tmp/cubarium-spire-live.png`. Separate10-second **live-owner**
browser measurement:60 submitted/drawn fps, **59.9 distinctfps**, new-frame
p95 16.8ms/max17ms, zero intervals over25ms. Source identity in
`/tmp/cubarium-spire-live-cadence.json` is PID3456152/build1236205/sinkshim.
This measures isolated Chromium delivery, not physical scanout, room-distance
readability or the user's personal browser. Runtime log: `/tmp/cubarium-spire-live.log`.
Ports7395–7396 are closed;7393 remains the shared cube viewer.

Local tag `checkpoint/live-spire-wind-2026-09-13` points to1236205. No push or
full backup package: this same-schema presentation update did not justify another
state-migration backup.

## Separate work in progress

The [complete108-case care screen](astra-care-response-results-2026-09-13.md)
confirms heterogeneous real feeding responses but almost no sustained rest.
Fable is comparing actual-intake meal onset; ordinary quiet physiology is being
diagnosed separately. Opus is implementing the isolated natural-rainfall/dose
comparison; no ambient default or dependence-on-attention preset changed.

The reviewed paid-charging comparison is frozen at3b06596, executable SHA256
`351997076c8a3e7a032109c45eee74541c75a87339a967018b083b23dd8674e0`.
Both12-seed/six-arm/two-hour jobs are running in
`captures/hunter-charge-{background,candidate}-two-hour-3b06596` (handles58224
and43911). Only explicit profile3→4 charging policy differs on the fixed reserve
background. All failures remain retained; partial progress is not a result or
live-introduction gate. Fragile types, diversity, lineage persistence, residual
flicker/AA and finer-detail backlog remain open: this rollout is not the full goal.
