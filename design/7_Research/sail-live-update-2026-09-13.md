---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Stable sail body: same-world live update

At approximately 04:51 PDT, the cube and shared viewer moved from `d55d8af` to
**`9cf0e1d`**, resuming the existing world at exactly **tick 633208**. This is a
verified art release, not acceptance of the unfinished ecological experiments.
The viewer remains `http://127.0.0.1:7393/`, fed by the same host as the shim.

## What changed

The selected [sail revision](astra-sail-stable-body-2026-09-13.md), `02f83c9`,
holds the swimming body's scale steady instead of crossing coarse raster
thresholds with a tiny squash. Original point-sampled fins are retained after
the body-only ablation and [Fable's review](fable-sail-candidate-review-2026-09-13.md).
Only the sail move row differs from the original production atlas. Rest, feeding,
budding, other creatures, plants and palette are unchanged. The experimental
fin-coverage exporter remains available but is not enabled in this pack.

The retained controlled comparison measured move-coverage standard deviation
falling about 71% at adult size and 80% at juvenile size. Those are specific
render-study measurements, not claims that all flicker is solved. Rest/feed fin
twinkle and constrained glasscane/vine sway remain open.

## Release isolation and verification

Release branch `release/sail-body-hold-2026-09-13` contains the committed art
through `02f83c9` plus the test-only repair `9cf0e1d` (main equivalent `da060e3`).
It deliberately excludes the schema-13 ordinary-quiet implementation and its
subsequent decoder hardening. It writes schema 12 and leaves quiet policy,
ambient support, manual care amounts and hunter introduction unchanged.

The previously reviewed target-invalidation fix is present: an unpaid hunter
stalk/windup loses its phase when its target disappears, while a paid strike
settles normally. There are no hunters in the running world. Its earlier
failure/replay evidence remains in the charging reports; incidental formatting
in that prior commit has not been rewritten during this rollout.

Frozen source:
`captures/release-source/sail-body-hold-2026-09-13` (clean at `9cf0e1d`).
Installed executable: `captures/releases/9cf0e1d/cubarium`, separate from the
reusable Cargo cache. SHA-256:

```text
binary     543ec4a39640865daf16f5ec28d77c07bdc915d8acf9cebd01f692374f2c761b
creatures  21ee9b341238304d1e8972f4b104d3325d595b0e431ebd22592a1721e12285de
```

Full frozen command `cargo test --offline --workspace --lib --tests` completed
with **1102 passed, 0 failed, 19 ignored**, followed by a successful optimized
release build. Logs are `/tmp/cubarium-sail-9cf0e1d-{tests,build}.log`.

### A real test-fixture failure, not a weakened assertion

The first body-only full-suite attempt at `02f83c9` failed two `art_motion`
cross-fade tests. Both depended on the shipped sail rest and move poses differing
at fixed phases; those poses can legitimately coincide after removing the
quantized squash. The original failed log remains
`/tmp/cubarium-sail-02f83c9-tests.log`.

The two controller tests now use explicitly distinct, animated color-channel
clips. They retain their strict weighted-stamp and non-endpoint assertions.
A new test also verifies coincident animated poses through interrupted changes
at 60Hz without a brightness flash. The focused suite passes **26 tests**, with
one capture-only test ignored. No presenter/runtime code was changed to satisfy
these tests; production-art playback is covered separately by the render study
and the remaining art tests.

## Copied-world preview and actual handover

The final release first resumed the existing isolated preview copy at tick
600104 on port 7396. Root inspected the running browser and native/enlarged
three-way art comparison, then measured cadence without concurrent screenshots.
The copied preview later stopped cleanly at tick 604129, exit 0. Its state and
captures remain retained; ports 7395 and 7396 are no longer listening.

Before stopping the old live process, root verified the authoritative PID,
executable, state directory, build and shim sink; care was ready with no queued
inputs. PID 3592720 stopped cleanly, exit 0, saving `state/world-633208.cubw`.
Its log reports 216898 shim frames sent, zero coalesced and zero errors.
New PID **3771509** advertises build `0.1.0+9cf0e1d`, the same state directory,
sink `shim`, and `start_tick: 633208` from that exact final snapshot.

Command:

```text
captures/releases/9cf0e1d/cubarium run \
  --art captures/release-source/sail-body-hold-2026-09-13/assets/atelier \
  --sink shim --mirror-web --web-port 7393 --state state \
  --fps 60 --speed 1 --care --require-resume
```

The resumed snapshot was semantically decoded: admitted care sequence remains 5,
Feed totals 6 material/12 energy, Rain 8 accumulated depth, Clean
1.6964642483156285 material/2.919334361962081 energy, and no active showers.
Hunter profile is absent and membership empty. The complete 2236-byte old journal
prefix retains SHA-256
`8cd21f5f9e4e1063d588057d984a8a67f3364cdc11c09a48ed55d875ca3ae4d5`;
the journal is now 2311 bytes after the normal new-host epoch record. No care
input, reseed, support adjustment or migration was submitted for this rollout.

## Presentation evidence and limits

Separate ten-second isolated Chromium measurements:

| Host | Submitted / rAF / net draw fps | Distinct frame fps | New-frame p95 / maximum | Intervals over 25ms |
| --- | --- | --- | --- | --- |
| Copied-world preview | approximately 60 / 60 / 60 | 59.8994 | 16.8 / 17.1ms | 0 |
| Actual shared live viewer | 60 / 60 / 60 | 59.9 | 16.8 / 17.0ms | 0 |

The screenshots show the updated cube projection and five-face net; browser
status identifies the live shim owner and functioning optional care endpoint.
Artifacts: `/tmp/cubarium-sail-9cf0e1d-{preview,live}-cadence.json`,
`/tmp/cubarium-sail-9cf0e1d-{preview,live}-browser.json`, corresponding PNGs, and
`/tmp/cubarium-sail-9cf0e1d-handover-before.json`. The running log is
`/tmp/cubarium-sail-9cf0e1d-live.log`.

These measure host submission and browser callbacks/draws, not physical scanout,
the user's personal browser, or subjective continuous-motion quality. The shim
contract and hardware mapping were consulted and left unchanged. No physical
cube inspection is claimed.

Local lightweight tag: `checkpoint/live-sail-body-hold-2026-09-13` → `9cf0e1d`.
No push, redundant state-backup bundle, or material deletion was performed.
