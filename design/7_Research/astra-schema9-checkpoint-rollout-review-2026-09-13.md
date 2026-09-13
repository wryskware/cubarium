---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Frozen schema 9 checkpoint: independent rollout review

Scope: exact Git tree `0725040`, compared with live-build revision `c60241f`.
Sources below were read with `git show`/`git diff` at those revisions, not inferred
from concurrently edited HEAD. Canon README and active ledger entries were read.
This is source-review evidence and proposed runtime gates, not deployment approval
or a report of physical-cube validation. No live state or process was touched.

## Finding

No new source-level blocker found for testing this frozen checkpoint. It separates
the completed presentation/accounting work from the unfinished hunter integration:
the decoder writes schema **9**, accepts **7/8/9**, and rejects schema 10. The new
code-native Lanternjaw and multipart renderer exist for the study, but this tree
does not add a hunter core extension or wire a predator into the normal presenter.
Do not describe this rollout as delivering live Lanternjaw ecology.

## Migration and rollback are asymmetric

At this revision, [snapshot.rs](../../crates/cubarium-core/src/snapshot.rs) appends
two correction `f64`s (16 payload bytes); the frozen v8 mirror preserves care,
organisms, fields, configuration and RNG, initializing only the corrections to zero.
The v7 migration also initializes the previously added care state. Raw heat/light
addition order is retained while a parallel compensated ledger tracks subsequent
rounding loss; migration cannot reconstruct rounding already lost in the old run.

[energy_correction.rs](../../crates/cubarium-core/tests/energy_correction.rs) contains
the genuine live-v8 migration fixture, exact next-600-tick v8-projection comparison
against the old binary, and an in-flight shower restart test. These are relevant
frozen-tree tests to run, not substitutes for trying the current copied world.
Full-state hashes change with the schema/corrections; compare the appropriate
legacy projection when checking cross-version ecological continuation.

**Keep a matched pre-upgrade state directory, care journal and launch/config record
outside checkpoint retention, as well as the old executable and art pack.** The
old decoder cannot read newly written schema 9 files. The unchanged
[state loader](../../crates/cubarium/src/state.rs) tries older snapshots after a
decode failure, and pruning retains only eight. Consequently an old binary can
resume an older schema 8 file instead of the newest world; `--require-resume`
prevents a fresh start, not this rewind. Eventually all retained snapshots may be
schema 9. Do not treat an executable-only rollback or journal replay across an
unplanned rewind as recovery. Restoring the matched backup sacrifices later world
time and inputs and must be an explicit recovery choice.

## Frozen art compatibility verified from Git blobs

Both manifests remain pack v5 with 24 plant frames. The atlas changes from
384×400 to 384×544: 25 rows become 34. I decoded both PNG blobs and compared RGBA
row bytes by `(name, stage, from, to)`, accounting for relocated rows. All 25
pre-existing rows are identical. Exactly nine growth rows are added: both steps
for glowcap, rootveil, tendrilfan and reedspire, plus lanternstalk 1→2; its 0→1
pilot already existed. Creatures, ground, habitat and tall PNGs are byte-identical.
The production [art loader](../../crates/cubarium/src/art.rs) is unchanged; its diff
only updates test expectations. New rows use the existing authored-growth path.

The one functional presenter change lets a top-face reed use its rooted bend
instead of the radial-only top-face branch. Canopy species with spin still turn
in place. Established plants initialize at their current stage on observation;
new clips do not replay every plant's birth merely because the process restarted.

Pin **both** the executable and explicit `--art` directory to the frozen tree.
Loading a matching v5 manifest/atlas is required; reusing the actively edited
worker directory would invalidate this review even if the executable stayed fixed.

## Other runtime changes and targeted gates

- Care flourishes are ephemeral, receipt-driven overlays, observed only after
  durable fresh application or boundary replay. They use the presenter's held
  time and the single encoded frame; they do not consume RNG or mutate ecology.
  Existing snapshot care history does not retrigger old flourishes. Verify one
  feed/clean on the **copied** world, duplicate rejection, pause/hold and restart;
  distinguish the brief receipt animation from real resource persistence.
- The Unix web accept loop replaces a 10 ms idle sleep with socket-readiness
  polling, bounded to 100 ms while idle. Existing HTTP guards and connection
  limits remain. Run idle/connected shutdown and mirror-web tests; inspect actual
  authored-world frame cadence, not only the cheaper orientation pattern. Prior
  59–60 fps measurements do not guarantee that rate under every world/load.
- Run the isolated checkpoint's full suite, especially energy correction,
  persistence, care replay plus the runner's durable/replay/uncertain-write hook,
  growth-pack endpoints, wind regressions and top-reed tests. Inspect native-64
  copied-world frames for rooted reeds, ordinary mature plants and a real growth
  transition; verify output remains finite and inside the existing frame budget.
- On the copied-state launch, record selected snapshot path/schema/tick, care
  cursor and pending showers; require the intended newest snapshot with no
  unexpected fallback. Keep operational config overrides and speed unchanged.
  Write a schema 9 checkpoint, stop cleanly and resume it; check journal/cursor
  continuity, no duplicate application and no unexplained conservation jump.
- A plain `git archive` under `/tmp` has no Git metadata: this checkpoint's
  [build script](../../crates/cubarium/build.rs) stamps `+unknown` in that case.
  Preserve an external revision/art manifest and binary digest, or provide
  truthful isolated Git metadata. A worker-HEAD hash or an old reused executable
  is not acceptable provenance. Confirm the actual built artifact, not only the
  source-directory name.

Root owns the isolated full-suite, copied-world browser checks and any later
operational rollout. Those results remain necessary before calling this ready.

Root's subsequent status: the candidate is actually a detached Git worktree at
`/tmp/cubarium-rollout-v9-OHYrp5`, so the archive-specific `+unknown` warning is
avoided. Its suite encountered an existing delayed-ack test asserting tick
advancement immediately after boundary application; root is cherry-picking a
test-only change that waits for advancement while retaining boundary/journal
checks. This review does not validate that patch or claim the rerun passed. If
the candidate gains that commit, record its actual revision and confirm its
production and asset paths remain identical to `0725040`.

## Independent verification appendix: tested candidate `a44dc98`

Later read-only verification supersedes the pending-test status above:

- Detached HEAD is `a44dc9834f8b5211fcbed606e72c55db5d8fdf25`. Its entire tracked
  diff from `0725040` is the ten-line delayed-ack test change; production and art
  are identical. I read that change: it waits at most 20 seconds for the world
  to advance, preserving the ready, later-tick and journal assertions. Tracked
  worktree files are clean; only the three isolated state directories are untracked.
- Independently hashing its release binary produced
  `342ff6a86fc3d0bf2ec4b49cd43b583d0abcc94289342f60e2ca72cdc45030d8`.
  The final suite log `/tmp/cubarium-rollout-v9-tests-final.log` contains 63
  successful summaries totaling **854 passed, 0 failed, 14 ignored**, with no
  failure markers. Root reports process exit 0; this review inspected the log,
  not the completed process's exit status. Ignored captures/timing checks are
  not counted as passing tests.
- `/tmp/cubarium-rollout-v{8,9}-continuation.json` both describe tick 262200,
  care cursor 5, population 87 and ecology hash `9293057068819118678`. I compared
  the full decoded states using Python's arbitrary-precision JSON integers,
  excluding only `energy_correction`: exact equality. More strongly, I read
  both referenced snapshot binaries, independently verified magic, schema,
  build identity, length and CRC, then compared payloads: **schema 9 minus its
  final 16 correction bytes is byte-identical to schema 8**. Thus this check
  does not depend on JavaScript rounding 64-bit RNG/ID integers. Candidate
  corrections are light `-6.2705118875072685e-12`, heat `6.33329920691663e-9`;
  the old projected corrections are zero. Root identifies the common opening
  as copied-live tick 261600, making this a 600-tick continuation window.
- The recorded 10-second cadence files show old live unique-net cadence
  52.2 fps versus candidate preview 59.988 fps; p95 new-frame intervals improve
  from 33.4 to 16.8 ms, and intervals over 25 ms fall from 77 to zero. Both
  browser RAF streams stay near 60 fps. This supports the readiness fix, with
  a limitation: old live used shim plus viewer, candidate used copied-world
  web-only output, at different world moments. It is not a strictly controlled
  output-cost comparison, personal-browser guarantee or physical scanout test.

At this review point, root's copied-preview shower 8 restart/completion check
is still in progress; I have not independently verified that result. The matched
backup, exact final artifact/art pin and latest-snapshot checks remain operational
gates. These stronger test/continuation results do not remove schema 9's rollback
asymmetry or authorize importing the unrelated in-progress schema 10 work.
