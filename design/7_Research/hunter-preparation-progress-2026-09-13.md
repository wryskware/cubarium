---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Fixed prey cohort preparation and delegated hunter work

Root implemented `scripts/prepare-hunter-worlds.mjs` for the primary cohort in
`hunter-experiment-contract-2026-09-13.md`: all seeds1–12 at exactly144000ticks,
using one frozen pre-hunter schema9 runner. No care, display transport, rescue,
food-based placement, retries or survival filtering. Every completed age endpoint
is retained. Strata are assigned only after all12 exist, sorted by population
then seed into three groups of four. Age-mature is not a stability claim.

The script refuses existing destinations, freezes/checks the runner SHA256,
validates snapshot header/schema/payload length/CRC, computes the full payload
hash independently, and matches it against exact-tick telemetry without losing
u64 precision in JavaScript. It retains logs, original complete configs inside
snapshots, endpoint resource/form censuses and source/build identities. It does
not claim an independent conservation audit of preparation. Hunter-arm audits
will begin from these PRE-import openings and retain all new gut/source stocks.

Tick-zero seed archives use the same frozen deterministic runner/seed with a
positive sub-tick duration (`--seconds0.001`, NOT0, which means unlimited). The
pinned runner rounds that to zero ticks and writes a final `world-0.cubw`.
Root verified this actual command: tick0, population24, valid schema9/CRC, no
frames. `--initials COHORT` can add the archive to a completed first-version age
cohort; it rejects changed binaries, incomplete cohorts and existing archives.
No aged state is reset. These are reconstructed deterministic seed openings,
not claimed to be checkpoint files preserved since the aged process began.

Six focused tests pass with `node scripts/prepare-hunter-worlds.test.mjs`:
arguments, genuine snapshot identity/corruption, exact telemetry and u64 hashes,
unfiltered deterministic strata including extinction, destination preservation,
and initial archive provenance/completion guards. Node syntax check passes.
The local Node26 `--test` invocation only displayed a file-level result, so the
direct command above was also run and explicitly reported all six named tests.

## Cohort preparation completed

Preparation root exec67729 completed with exit0:

```text
node scripts/prepare-hunter-worlds.mjs
  /tmp/cubarium-pre-hunter-v9-fN4Cls/cubarium-v9
  captures/hunter-openings-2026-09-13
```

Runner SHA256 `725305bc6bf8be924e5e46d5663b34bd6396855861af2933f671fb4ced421d0f`,
core `b47eacc`, host build `0.1.0+1d7b386`. This active invocation began before the
initial-archive helper was added, so after it exits successfully root must run
`node scripts/prepare-hunter-worlds.mjs --initials captures/hunter-openings-2026-09-13`.
The follow-up initial archive command also completed with exit0. Root reread
all24aged/initial snapshots, independently checked schema/header/CRC/SHA256/full
payload hashes, and matched every record to its manifest. Both manifests retain
the frozen executable identity. All12aged populations survive at this endpoint,
with78–101organisms; eight have already lost skimmers. No seed was replaced and
no founder ancestry map is claimed for these aged openings.

| Population stratum | Seeds in rank order | Opening populations |
| --- | --- | --- |
| Low | 11,10,3,2 | 78,86,87,88 |
| Middle | 5,6,12,8 | 90,90,93,95 |
| High | 9,4,1,7 | 97,99,101,101 |

These are reporting strata, not a selection/filter. Exact form counts and all
endpoint telemetry are in `captures/hunter-openings-2026-09-13/manifest.json`;
initial seed identities are in `initial-manifest.json` beside it. The six-arm
hunter harness and predator outcomes are not implemented/measured by preparation.

## Delegated implementation still running

Native Opus5 high core implementation: root exec9109, resumed session
`f1579c62-fe40-4506-b0c7-94f064f5ab92`, output
`/tmp/cubarium-opus-hunter-core-2026-09-13.jsonl`. It owns core and its report only.
The work order and genuine pre-hunter schema9 +600tick fixture pair were committed
BEFORE delegation in `1f0fc3a`. Empty-extension schema10 must preserve that full
schema9 payload as well as the existing genuine7/8 continuations. No live insertion.

## Integration finding requiring resolution

Astra's `2c8e90d` contract measures actual Lanternjaw capture-claw reach around
(13.2794,1.1624), versus the old unconfirmed6px core placeholder. Ingestion mouth
and grasp region differ. Core sensing/stopping/phase timing and juvenile scaling
must agree with the visible claws in root-owned surface charts; no off-rim
reflection or invisible vertex capture. The core work order now links this finding.
It may arrive after initial profile authoring, so do not call that first profile
visually integrated until the contact correction is implemented and checked.

Fable remains on exec69674/session135e2be2-1019-48c7-ab48-bab462bc4684, finalizing
the multipart body package. Side-family growth and flooded-top reed wind are
committed as `5d7ea69` and `b8b8a11`; source/tests/progress records still changing
under that agent remain its ownership. Root will verify its final package before
deployment or biological animation integration.
