---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Shared-instance viewer, optional care, and megafauna candidates

## Owner update: preferred megafauna direction

Wrysk now says “i prefer the fable creature” (2026-09-12). Continue with
Lanternjaw as the preferred visual direction; preserve Veilwarden as an alternate.
Root has recorded this in the gallery. Finish Fable's motion polish and commit
the body study. This preference does not make the study production-integrated
or choose its ecological parameters. Care remains the current live-rollout gate.

## Integrator gate: revision 2 abort recovery

Astra's bounded follow-up found that an uncertain partial accepted-line write
cannot safely be followed by a blind abort append (the torn suffix becomes
interior corruption). Also, a reserved batch seq whose accepted record never
reached disk loses its boundary under `abort {seq}` plus accepted-only replay.
Please read the new appendix in `astra-care-contract-review-2026-09-12.md` before
signing off host persistence. Simplest safe first slice: keep the boundary held
and require stop/recovery on uncertain writes, with no abort-and-resume path.
If keeping abort-resume, repair the verified journal suffix first, record
self-contained aborts with seq AND boundary for the entire reserved batch,
replay accepted/abort union with contiguous seq/nondecreasing boundaries, and
resume only after all aborts have durable acknowledgements. Add fault tests.

Root also committed `2775e56`, which makes build identity follow Git HEAD/ref
changes; workers no longer need to touch build.rs solely to refresh the hash.

User authorizes implementation and continued iteration. They now explicitly require
root and all workers to check in validated work. Approved animation checkpoint is
commit `cb9dc9c`. Do not change canon. Preserve `.vscode/` and all unrelated changes.
Read AGENTS, canon, README, the care-and-megafauna proposal, and the current shim
contract before working. Lore first where available. Root owns live rollout.

## Fable's mission and delegation

Act as Fable 5.1 orchestrator, effort high; favor bounded Opus 5 high/medium workers.
Implement the packages below, assign disjoint ownership, validate and COMMIT each
completed package's own files. Coordinate Git index use: serialize staging+commit,
never `git add .`, never commit another worker's in-progress files, no resets or
stashing, no hooks bypass. Report each hash. Root is independently authoring its
candidate and reviewing rollout. Astra is reviewing contracts in
`astra-care-review-2026-09-12.md`; read when it arrives. Await every worker to actual
completion before ending. Keep test suites focused on invariants, not thousands of
lines of redundant probes; investigate fixture assumptions before changing production.
Use apply_patch for edits. Writes stay in repository and /tmp. No external memory
edits. Do not launch any real shim sender or stop/restart a live process.

### A: same-world viewer FIRST, separate checkpoint

Current live shim process found by root: PID 2295402, cwd must be verified at rollout,
`target/release/cubarium run --art assets/atelier --sink shim --fresh --state state --fps 60`.
Other previews 7395,7396,7397 are distinct worlds; leave them alone. Port7393 is free.

One World, one observe/draw/encode, fan out identical Frame bytes to shim and optional
web mirror. Prefer a small explicit run flag such as `--mirror-web` with `--web-port`;
keep existing CLI behavior. Browser disconnects or slow reads must not stall cube or
sim. A second process using the same --state is NOT attachment. No shim changes,
hardware mapping/gamma changes, rerender in browser, or duplicate ecology. Add a
read-only source identity/world tick indication in the viewer's external UI; existing
/frame prefix is render sequence, not sim tick. Verify actual UDP payload against
web frame in an isolated local fixture and normal run/resume state hash equivalence.

Commit viewer package after tests, and preserve a release binary in a named /tmp
directory so root can deploy this v7-compatible change before care changes schemas.
Signal completion to root through a concise progress section in this handoff or a
separate `shared-care-progress-2026-09-12.md`. Root does safe live process replacement.

### B: optional Feed / Rain / Clean interaction

First write a precise short contract, incorporating Astra review, then implement.
Keep controls in a collapsible/explicit web care panel, never on cube pixels. The
same owning runner accepts bounded commands at sim ticks; default zero input must
preserve current autonomous evolution. No autonomy-default tuning yet: dose controls
can land now; hands-on ambient presets require independent ecology results.

Feed: finite edible deposit through an existing appropriate food pool, visible
acknowledgement without falsely claiming a plant fruited. Rain: finite real water
input through existing rain/flow/growth processes with eased envelope. Clean:
remove a bounded fraction/amount of existing local detritus and its proportional
stored chemical energy; do not remove bodies, plants, or label nutrient as toxic.
No toxicity state is implemented, and useful detritus feeds scavengers. Partial
cleanup, not sterilization. Explicit source/sink ledgers, nonnegativity, seam-safe
distribution, cleanup floor/cap, finite validation, burst and cumulative limits.
Food accumulates material even after recycling: finite cumulative allowance or
equally explicit export accounting, not cooldown-as-an-infinite-budget claim.

Bound requests and worker concurrency; loopback-only, reject cross-origin mutation,
require same-origin JSON/custom header or unguessable session token with host/origin
validation; no permissive CORS or mutation via GET. UI distinguishes accepted,
rejected, and actually applied commands. Avoid unbounded journal/queue/memory growth.
Give events stable ids, authoritative target/applied ticks and deterministic
duplicate/restart semantics; durable acknowledgment and input replay must be tested.
Keep core free of I/O and wall time. Browser merely submits care intents.

CRITICAL persistence: v7 postcard schema is strict. Implement explicit v7 migration
if schema changes, retaining organisms, fields, RNG determinism and all accounting.
Test against a genuine pre-change v7 fixture, not a new struct serialized as "v7".
Do not let failed load silently create a fresh world. A require-resume guard and
single-writer state lock are appropriate safeguards; no `--fresh` in a live upgrade.
Old version snapshots must stay recoverable in a backup outside the retention path.

Tests: zero-input comparison; each effect and exact ledgers; scarce/empty cleanup;
invalid/duplicate/burst commands; seams and rims; restart/replay of pending rain;
post-acceptance interruption; old v7 migration; two tabs; UI targets match face
geometry. Test only copied/synthetic worlds. Short matched care/no-care scenarios
and native captures. Do not claim long-run stability from short tests.

### C: Fable's independent megafauna body candidate

USER WILL CHOOSE. Do not put candidate into live founder config or auto-pick it.
Design your own distinct larger apex body and motion language, native64px first.
Root independently designs a competitor; a tie may later be seed-selected or reused.
No hunting ecology needs to ship before visual choice. Treat moves as art studies,
not simulated successful kills. Preserve current production atlas/source rigs.

Root owns shared comparison gallery under `art/studies/megafauna/` and its HTTP
study server. Fable owns ONLY `art/studies/megafauna/fable.js` plus optional
`fable-*.svg` and `fable.md` there. Contract: ES module exports `candidate` object
with `{name, author, description, draw(ctx, {time, mode, x, y})}`. `ctx` is a
Canvas2D at native resolution; creature faces RIGHT, anchored at x,y. Common modes
are `rest`, `move`, `hunt`, `bud`. Draw within x±12,y±10 nominal; use a clear body
around 16–20px long, bold silhouette, restrained current cyan/purple/pink palette.
No canvas clears, backgrounds, UI, global transforms/state leakage (save/restore),
random wall clock, imports, or external assets required. `time` is seconds; loops
should be calm with a short articulated hunting motion. Deterministic function of
time/mode. Bud mode can depict one small offspring/cocoon within footprint.
Root gallery handles 64px habitat background, common-size creature reference,
same timing/modes, native and nearest-neighbor enlargement, pause and side-by-side.
Include design rationale and limitations in fable.md. Commit your scoped files.

## Root work and completion

Root: archived/committed approved checkpoint; live rollout with verified graceful
stop/resume and backup; independent body candidate and comparison gallery; review
source, conservation, selected tests, visuals, final integration and commit audit.
Astra: independent safety/accounting review, optional narrow regression gate.
Later growth expansion remains backlog if these new requested slices use the turn;
do not silently label it complete. No implemented apex ecology until tested later.

Deliver runnable shared viewer/care commands, evidence, exact commits, candidate
comparison, and remaining limitations. Check in your work throughout, not only at end.

## Root rollout finding (read before assuming latest tick means current history)

Live PID2295402 was started with --fresh in state/, now around tick49600, but that
directory retains old snapshots up to446277. The current highest-tick snapshot is
NOT the live world; pruning may continually discard this fresh run's checkpoints.
Root is coordinating recovery: archive exact old snapshots before graceful stop,
then verify the new live final snapshot and resume only it. Do not operate state/.
Consider a focused --fresh isolation/archive fix under the state-lock ownership
package so this cannot recur. Astra is reviewing the behavior independently.
Root binary backup is captures/checkpoints/pre-care/cubarium, SHA256
ca2a51f281ed7f7a6f515e2f22a814d615f8513e4c39bd5337a44ff3b82facaf.
The backup world-446277.cubw is explicitly an OLD historical snapshot, not a verified
rollback point for the current live world. Root will add the correct one at rollout.

Root archived the eight exact stale snapshots to
captures/checkpoints/pre-care/retired-history/ without deleting them or moving the
active state directory. The cube now retains current-run snapshots (52800,54000,
55200 observed). Root preserved55200 too; final clean checkpoint still required.
Root studio is served on7400, files art/studies/megafauna/{index.html,gallery.js,
root.js,root.md,README.md}; your fable.js will be loaded automatically on page reload.
Root visually reviewed its candidate with isolated Chromium. No art changes to live
world. Fable should commit only its own candidate files; root handles gallery.

Root added the read-only example crates/cubarium/examples/inspect_snapshot.rs and
preserved its schema7 executable in captures/checkpoints/pre-care/inspect-snapshot-v7.
It verifies the current-run55200 snapshot (95 organisms) and can emit full JSON state
for ecological projection comparison. This is root-owned; do not stage it in worker
commits. Schema7 genuine bytes are available in captures/checkpoints/pre-care/
world-55200.cubw for migration tests; preserve a fixture before changing the schema.

Art review gate: preserve Wrysk's anti-blinking direction in both studies. No one-frame
jaw flashes or hard blink cuts: any strike accent needs a short smooth envelope
(roughly150–300ms), subordinate to articulated motion. An orange accent is optional,
not required. Fable should review its worker's rendered candidate as art director.
Root saw your commit coordination lock and will also use
`flock /tmp/cubarium-shared-care/git.lock` for subsequent commits.

## Core/host contract review gate — resolve before claiming crash replay

Root read care-contract-2026-09-12.md. Its current target-tick protocol is not yet
sufficient: worker sets current_tick+1, then fsyncs while world advances. Actual
application may be later. A crash after apply but before durable outcome/checkpoint
would replay at the original earlier tick, changing ecology. Do not claim exact
replay from tests that avoid delayed fsync. Define a robust admission/commit boundary;
a brief explicit simulation hold while a scheduled durable admission completes may
be necessary. Rendering/HTTP stay responsive; don't promise simultaneously no sim
waiting and exact replay without a proven protocol. Astra is reviewing alternatives.

The proposed PID-file live/dead test also needs atomic exclusive ownership and PID
reuse safety. Prefer an OS-held advisory file lock on the canonical state directory
over liveness-only stale-file deletion (concurrent starters must not both own it).
Bound journal growth and durable client dedup high-water state: a recent256-record
tail alone cannot remember old client sequences once that client's last record ages
out. Clarify stale-epoch rejection/rotation rather than accepting forgotten duplicates.
Rain_DEPTH table currently says centre depth but normalized weights implement TOTAL
depth summed over footprint; choose and document one. Root favors total dose.

Astra confirms a minimal robust option: owner reserves application boundary T and
holds tick advancement there; worker durably appends command+T; only after ack may
owner apply and step T. Outcome logs are diagnostic, accepted schedule authoritative.
An fsync error is ambiguous (record may survive): do not resume autonomous ticking
past T while recovery might later replay that command at T. Hold/cleanly stop there
or establish a durable abort before advancing. Surface this explicitly in care UI.

Root opened both candidate modules in the real gallery: Veilwarden and Lanternjaw
load together and are visually distinct. Screenshot /tmp/cubarium-megafauna-pair-rest.png.
Initial Lanternjaw still uses hard blink booleans and integer-rounded motion; please
smooth the blink/strike accent per gate above and explicitly identify any deliberately
stepped pixel-art motion as a study limitation. No unsupported claim of already-smooth
production playback. Root's studio is at7400 and same mode/time supports direct review.

## Viewer live rollout completed by root

Root independently passed mirror_web's three integration tests and resumed a copied
current v7 checkpoint with the e3ad20f viewer build. Preserved that executable at
captures/checkpoints/shared-viewer-v7/cubarium (SHA256993a1370...13bb4742).
Gracefully stopped old cube PID2295402, verified/copy-backed its NEW final67984
snapshot (schema7, population94, state hash16684878898031239227), then launched:
`captures/checkpoints/shared-viewer-v7/cubarium run --art assets/atelier --sink shim
--mirror-web --web-port 7393 --state state --fps 60 --speed 1`.
Root execsession48903. It reports resuming67984. Care NOT deployed yet; this frozen
v7 binary remains running while you implement schema8. Do not signal it or touch
state/. Old previews7395/7396/7397 remain distinct and untouched.
