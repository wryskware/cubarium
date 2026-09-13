---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Completed presentation checkpoint: live rollout evidence

The shared cube/web owner now runs frozen build **0.1.0+a44dc98** at
http://127.0.0.1:7393/. This rolls out completed side-face authored growth,
rooted top-reed wind, receipt-driven feed/cleanup flourishes, web socket-readiness
cadence, and compensated energy accounting. Ecology remains 20 Hz; presentation
is 60 fps. There is **no live hunter extension or Lanternjaw integration**.
Wrysk's selected visual direction remains Fable's Lanternjaw; its ongoing work
is deliberately separate from this frozen live checkpoint.

## Exact artifact and tests

Detached worktree `/tmp/cubarium-rollout-v9-OHYrp5` is at
`a44dc9834f8b5211fcbed606e72c55db5d8fdf25`. All production code and assets are
identical to `0725040`; only a test-only cherry-pick of `75cb298` differs.
That fix waits for actual tick advancement after a durable care receipt rather
than assuming receipt publication and the next simulation tick are simultaneous.

The final full workspace test process exited 0: **854 passed, 0 failed,
14 ignored**. An earlier sandbox run failed on forbidden socket binding; the
first host run exposed the test race above. Neither is counted as a passing run.
The corrected isolated suite and release build completed successfully.

Executable and matching atelier pack are frozen under
`captures/checkpoints/presentation-v9-a44dc98/`, outside worker source paths.
Executable SHA256:
`342ff6a86fc3d0bf2ec4b49cd43b583d0abcc94289342f60e2ca72cdc45030d8`.
The adjacent README records atlas/manifest hashes and exact launch flags.
[Astra's independent review](astra-schema9-checkpoint-rollout-review-2026-09-13.md)
verified artifact provenance, unchanged old atlas rows, full-suite totals and
the actual cross-version continuation bytes.

## Copied-world migration and browser checks

Before changing the live owner, root copied its genuine schema 8 snapshot
`world-261600.cubw` and matching care journal (admitted sequence 5). Old frozen
`c60241f` and candidate `a44dc98` each ran this input for exactly 600 ticks,
without additional care. Both ended at 262200, population 87, ecology hash
`9293057068819118678`. The schema 9 payload excluding its appended 16 correction
bytes is byte-identical to the schema 8 payload. Full decoded legacy state also
matches; the new corrections only account for subsequent floating-point loss.
This does not reconstruct historical rounding loss before migration.

An isolated candidate web preview on port 7403 used the copied world and pinned
art. Actual Chromium panel buttons targeted Front (32,48):

- Feed sequence 6 at 262956 applied 3 material/6 energy over five cells. Repeating
  its exact request returned duplicate=true, sequence 6, with no second action.
- Clean sequence 7 at 263367 partially exported 1.4886821661267735 material and
  2.7066702160507305 energy over five cells.
- Rain sequence 8 at 264949 scheduled four depth over 13 cells through 265069.

The copied owner stopped cleanly at 264951, with sequence 8's shower delivered
2 of 120 ticks. Restart selected exactly that schema 9 file, became ready with
no outstanding care and no reissued old receipts. A second clean stop at 266138
validated sequence 8 unchanged, no remaining showers, rain depth ledger
12.000000000000021, unchanged feed/export ledgers, and material drift about
2.5e-12 across the restart continuation. No preview inputs were copied live.

Root inspected real browser captures before/after care and after restart.
These are image/integration checks, not evidence of strong biological responses
to care or a newly observed complete plant life cycle in the live world.

## Controlled live handover and recovery

Root verified PID2511131, build c60241f, expected executable hash, state path,
shim sink, speed 1, ready care and no outstanding work before sending SIGINT.
Its original process handle confirmed **exit 0**, final tick **283643**,
population 86; its transport summary reported 462040 shim frames, zero errors.

The final schema 8 snapshot and matching journal were copied after shutdown to
`captures/checkpoints/pre-presentation-v9/state/`, outside retention. Snapshot
ecology hash is `12623117655090838007`; admitted sequence is 5, no showers.

- Snapshot SHA256: `3320abee3cbba1229be59501bb0d233ddea5f8f8be2f12fe91a9431041120c5f`.
- Journal SHA256: `cbbc6c9b4df193f9fe122083d44ac89cdc9170f0b7d805b20a08d6f2004b2b30`.

The recovery set also retains the old executable, art from the c60241f tree,
and launch/recovery instructions. **Executable-only rollback is unsafe:** old
code cannot read schema 9 and could silently choose an older schema 8 file.
An intentional recovery needs the matched backup in a new state directory and
would lose subsequent world time/inputs. No files were deleted or histories mixed.

The new owner is **PID2904734**, root process handle 33359, using:

```sh
captures/checkpoints/presentation-v9-a44dc98/cubarium run \
  --art captures/checkpoints/presentation-v9-a44dc98/atelier \
  --sink shim --mirror-web --web-port 7393 --state state \
  --fps 60 --speed 1 --care --require-resume
```

HTTP independently confirmed exact resume file `state/world-283643.cubw`, start
tick 283643, expected build/state path/shim sink, speed 1 and advancing ticks.
Care was ready with no outstanding inputs; its new epoch did not reissue old
receipts. The first periodic checkpoint **284400** validated CRC/schema 9,
population 88 and unchanged admitted sequence 5/all care ledgers. Its SHA256 is
`6de3b55cf01fdffdd0a51458a86c5806da9394a5a3adaae3285c74692c980e5d`.
Root added no live care. The display shim and unrelated viewers were not stopped.

## Measured browser delivery and limits

Ten-second isolated Chromium measurements of the actual shim+mirror owner:

| Measurement | Before, c60241f | After, a44dc98 |
| --- | ---: | ---: |
| Distinct net frames/s | 52.2 | 59.8994 |
| New-frame interval p95 | 33.4 ms | 16.8 ms |
| Largest new-frame interval | 33.6 ms | 17.0 ms |
| Intervals above 25 ms | 77 | 0 |

Both observations used the live shim+mirror configuration, but different moments
of the advancing world; this is not a deterministic CPU benchmark. The copied
web-only preview independently delivered 59.988 distinct frames/s. These are
headless browser callback/canvas measurements, not personal-browser pacing or
physical LED scanout. No claim is made of direct physical-cube visual validation.

Suite logs, continuation JSON, browser cadence JSON, preview restart logs and
the live screenshot are retained under the frozen checkpoint's `evidence/`.
Hunter geometry/animation integration, biological care reactions, autonomy/dose
tuning and ecological diversity remain open; this rollout does not resolve them.
