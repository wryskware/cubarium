---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Care dose: browser acceptance and live update

The shared cube/web owner now runs the committed care-dose package, build
`0.1.0+e1aa426`, with Gentle/Standard/Generous controls. This records runtime
evidence, not ecological balance or completion of the full living-world backlog.

## Tested code and browser

Clean detached worktree `/tmp/cubarium-care-dose-rollout-e1aa426`, commit
`e1aa4263fedc0808a59d1035a64e504162a368b1`: root ran
`cargo test --workspace --lib --tests` to exit 0, **1056 passed, 0 failed,
17 ignored** across 68 test summaries, then built the release executable.
Log: `/tmp/cubarium-care-dose-clean-tests.log`. The inline viewer's 10 tests
also passed. Astra's [bounded independent review](astra-care-dose-package-review-2026-09-13.md)
passed 140 focused tests and reproduced the old fixtures.

An initial shared-worktree test run failed the old canopy-fallback assertion
while Fable's canopy assets were being edited; it is not counted as a pass.
The isolated committed care package above passed. Fable subsequently updated
that assertion in its own canopy package, which is verified separately.

Actual Chromium controls on copied world `/tmp/cubarium-dose-preview-W2cxAj`,
temporarily port 7395, targeted Front (31,62):

- Gentle Feed, sequence 6 at tick 445880: 1.5000000000000002 material and
  3.0000000000000004 energy over four rim-adjacent cells.
- Generous Clean, sequence 7 at 445884: **partial**, exporting
  2.2423511075444953 material and 2.21726961765622 energy, respecting local stock.
  The first browser probe wrongly required `applied` rather than accepting
  `partial`; inspecting the actual receipt established this was a probe error.
- Generous Rain, sequence 8 at 447178: six total depth scheduled over nine cells
  through 447298. The preview stopped cleanly at 447181 with three of 120 samples
  delivered and persisted dose 1500. On exact resume it ran 160 ticks to 447341:
  no remaining shower, admitted sequence still 8, unchanged feed/cleanup ledgers,
  rain ledger 13.999999999999996 (previous total eight plus six), unchanged total
  material. Both preview process handles exited 0.

Native browser capture inspected: `/tmp/cubarium-dose-browser-actual.png`.
This is actual browser/core/restart integration, not a balance trial. No copied
inputs or files were copied back to the live world.

## Live handover

Root checked old PID 2904734, build `a44dc98`, shim sink, intended `state/`, care
ready and zero outstanding requests before SIGINT. Its original handle exited 0
at tick **451222**, population 98, with 502607 shim frames and zero transport
errors. The final schema-9 snapshot decoded successfully in the new build;
ecology hash `6386663092543681155`, admitted care sequence 5.

Only one matched final snapshot/journal pair was retained for this schema
migration: `/tmp/cubarium-pre-dose-recovery-OkFySG/`. Ordinary checkpoints now
use lightweight local Git tags, per Wrysk's instruction. No new full executable/
art backup package was created. Tags do not preserve world history or make an
older binary understand schema 12; the recovery pair is specifically for that
compatibility boundary, outside live snapshot retention.

New owner PID **3283671**, root handle **61050**:

```sh
/tmp/cubarium-care-dose-rollout-e1aa426/target/release/cubarium run \
  --art /tmp/cubarium-care-dose-rollout-e1aa426/assets/atelier \
  --sink shim --mirror-web --web-port 7393 --state state \
  --fps 60 --speed 1 --care --require-resume
```

Executable SHA256:
`34dea7558d413ffb1c93d21886ef9bcb7f6709b826dee3b1b64134a1830a9e83`.
Tag `checkpoint/live-care-dose-2026-09-13` points to `e1aa426`.
Log: `/tmp/cubarium-care-dose-live.log`.

HTTP and an actual browser confirm build `0.1.0+e1aa426`, resumed file
`state/world-451222.cubw`, exact start tick 451222, advancing ticks, shim sink,
enabled dose capability and selector, care ready and zero outstanding requests.
The browser observed ticks 451774→451798 and render sequence 1652→1724 across
1.2 seconds; this short observation is not a full frame-pacing benchmark.
Capture inspected: `/tmp/cubarium-care-dose-live.png`. Root submitted no live
care. Hunter membership stays empty; neither experiment founders nor changed
ambient settings were introduced.

## Stale viewers

Per the owner's permission, root identified web-only PID 1258419 on 7395
(`state/strata-latest`, started Sep 12 07:16) and PID 1912268 on 7396
(`/tmp/cubarium-animation-preview-My6gKf`, started Sep 12 13:37). Both were old
deleted-inode executables, separate from the owning cube. SIGINT stopped both;
process and socket checks confirmed exit. The temporary dose preview also exited;
only 7393 remained listening among those three ports. No state files were deleted.
The display shim was never stopped or edited.
