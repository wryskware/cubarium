---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Independent care and shared-instance implementation review

Wrysk authorized shared cube/web viewing, optional feeding/rain/cleanup, and continued
work while preserving the approved experience. This is an implementation review and
proposed contract, not a canonical decision. No runtime or production files changed.

## Findings from the current code

- `crates/cubarium/src/runner.rs` owns one `World`, presenter, clock, checkpoint worker,
  and selected sink. `open_sink` currently selects one output. Fan out its single
  encoded `Frame` to shim and web mailboxes to show the same instance and image.
- `crates/cubarium/src/sink/web.rs` has read-only HTTP routes and a latest-frame
  mailbox. Its eight-byte `/frame` prefix is a render submission sequence, not the
  world's simulation tick. Browser disconnects do not stop the simulation.
- `crates/cubarium-core/src/fields.rs` stores nutrient `N`, producer `P`, detritus
  `D`, detritus chemical energy `De`, fruit `F`, and water `w`. There is no toxicity
  state. Nutrient is recycled material used for growth, not a poison pool.
- `crates/cubarium-core/src/world.rs::step` has only a comment for stimulus admission.
  An exhaustive source search found no implemented input journal. Existing life-event
  and telemetry logs cannot provide care replay or durable acceptance.
- `WorldState` records external material input, light input, heat output, rain input,
  and evaporation. It lacks explicit care energy input and material/energy export.
  `mass_residual` subtracts initial material and external input; `from_state` rebuilds
  that initial baseline so the residual resets at every load. A zero residual after
  restart therefore does not establish historical conservation.
- `snapshot.rs` strictly accepts schema 7 and postcard-decodes the whole `WorldState`.
  A schema change needs explicit migration. Serde default annotations do not make
  this decoder accept older schema headers. If all snapshots fail, the runner
  currently starts a fresh world. This is a rollout hazard for the approved world.

## Minimal proposed care contract

Keep ambient support at its current value. Input dose and an optional lower-support
profile are independent controls; care does not alter time speed, hunger, birth
funding, or controller noise. Delay a lower-support preset until matched unattended
tests justify its range. Zero input should execute the old ecological operations in
the old order, with no new RNG draws or changes to weather configuration.

Use bounded server-defined doses first. A command identifies its world/session,
client request ID, kind, and optional canonical surface target. The simulation owner
assigns a monotonically ordered sequence and an application tick. Resolve target
weights deterministically through the existing surface graph, normalize their sum,
and store the resolved command. A seam target must not double a dose. Validate finite
quantities, IDs, positions, duration, total dose, queue capacity, and outstanding
commands before acceptance; report actual accepted and applied quantities separately.

### Feed

The narrowest implementation can deposit charged organic crumbs into `D` and `De`.
At current defaults `detritus.energy_cap = reserve_energy_density = 2`, fully charged
crumbs are fully edible to existing scavenging diets. Add material `m` and energy
`rho*m`, with `0 < rho <= energy_cap`. Book both external sources. Existing local
sensing and actual ingestion produce approach, feeding, and later rest. Mineral
nutrient alone cannot substitute for food. A fruit-based first food is another valid
choice, but it must credit `F` and `fruit.energy_density*m` and attract frugivores.

Choose one category in the implementation brief and label it truthfully. A deposit
visual should come from the admitted input's real location and remaining resource;
it should not make an unrelated plant appear to have produced fruit. One per-cell
aggregate cannot distinguish a supplied crumb from old litter after mixing; if
individual edible morsels must persist visually, define their provenance/removal
representation rather than keeping a false independent food sprite alive.

Cap cumulative net added material as well as request frequency. Decomposition only
converts detritus to nutrient; it does not export material. Successful cleanup can
restore part of a net material allowance, while per-action and per-time caps still
bound pulses. A cooldown alone is insufficient for repeated daily feeding.

### Rain

Admit a finite scheduled shower expressed in depth per cell per simulation second.
Add its rate to natural rain at the existing rain stage, before flow, evaporation,
and growth. `rain[c]` must publish the combined actual rate so the visible shower
and deposited water agree. The existing rain stage overwrites `rain[c]`; changing
only the render view or adding a temporary value before that stage would lose the
effect. A duration envelope should integrate to the admitted dose over whole ticks.

Keep manual rain separate from weather blob/moisture suitability. Mutating the
weather source can change ambient suitability and future weather, and can make a
manual dose depend on the natural rain threshold. Track manual water separately
for interaction reporting while including it once in total `rain_in` and its audit.
Persist remaining duration and progress so restart resumes the unspent portion.

### Clean up waste

Call this “Clean up litter” or explain that it removes unrecycled organic waste.
Remove existing `D` locally, capped by the requested dose and a conservative local
fraction. For each affected cell with opening stocks `d, de`, take
`removed_d <= d` and `removed_de = de * (removed_d / d)` (both zero if `d == 0`).
Book actual material and chemical energy exports separately; exported chemical
energy is not heat dissipated inside the world. Preserve `N`, `P`, `F`, organisms,
and water. This preserves the energy density and `De <= energy_cap*D` invariant.

Report a partial removal or nothing available honestly. Existing detritus is also
food and future fertilizer; cleanup is an optional tradeoff, not an unconditionally
beneficial cure. Removing mineral nutrient or inventing a toxicity threshold is a
different ecological feature and is unnecessary for the first interaction.

## Persistence and replay

The accounting identities become `delta material = care material in - material out`,
`delta stored energy = light in + care energy in - heat out - exported energy`, and
`delta water = total rain in - evaporation`. All cumulative counters and pending or
active effects belong in checkpointed state (or in a versioned extension with a
fully defined atomic relationship). Add finite/nonnegative validation for them.

Durable “accepted” means the command has been appended and synced before admission.
Use a bounded intake queue and a worker acknowledgement so an HTTP client or slow
disk does not block the world. If durable append fails, disable/reject further care
and continue autonomous simulation; observer-log best-effort semantics are not
sufficient for accepted actions. Distinguish queued, durably accepted, applied,
partial, and rejected status in the external panel.

Persist the application cursor and active shower schedule with the world snapshot.
On restart, recover accepted journal entries after that cursor and replay them in
the same tick/sequence order. Recover pending accepted entries as well as effects
already partially applied. A duplicate request with the same identity and payload
returns the original receipt; the same identity with different payload is a conflict.
Define a bounded durable deduplication window, or client epoch plus monotonic sequence
with stale-request rejection, so expired history cannot become a fresh deposit.

A durable accepted command whose application lies after the restored snapshot must
not disappear merely because the process crashed. Conversely an already applied
command in that snapshot must not apply twice. Include crashes before append sync,
after sync/before admission, mid-shower, and after application/before checkpoint in
verification. Torn final journal records may be ignored only as unaccepted suffixes;
an invalid interior record must report recovery failure, not silently skip actions.

## Shared-instance rollout and verification

1. Preserve the current binary, exact launch arguments, and a verified clean
   checkpoint. Run copies in distinct state directories for experiments.
2. Add combined shim/web output to the owning runner. Do not run two world writers
   against one state directory; it currently has no ownership lock. Add such a lock
   before making multiple viewer launch paths convenient.
3. Expose instance identity and simulation tick separately from render sequence.
   Test byte equality of the fan-out, continued ticks with no browser, multiple
   browsers receiving one instance, and prompt nonblocking output with a slow client.
4. Keep care mutation on the simulation thread at the tick boundary. The web server
   only enqueues validated commands and reads receipts. Use bounded request bodies
   and connections; same-origin POST plus a per-process capability/CSRF check is
   appropriate even for loopback. Preserve loopback binding unless separately scoped.
5. Implement and test schema-7 import explicitly before switching live state. Refuse
   an existing-but-unloadable state directory unless fresh creation was explicit.
   Verify the migrated snapshot preserves fields, organisms, tick, config, and
   weather, with care state initially empty. Preserve the old binary/checkpoint for
   rollback; do not rely on the old binary reading a new schema.
6. Compare zero-input trajectories from the same schema-7 fixture with the prior
   implementation. New serialization fields will change `state_hash`; compare old
   ecological state projections, or hash those projections, rather than claiming
   raw new hashes must equal schema-7 hashes. Render fan-out alone can retain exact
   current world hashes.
7. Verify exact source/export accounting for feed and partial cleanup, real rain
   including a seam target, repeated clicks/dedup, queue saturation, restart/replay,
   and long runs with no/occasional/repeated care on matched seeds. Review native
   64px captures for truthful local response and quiet recovery afterward.

The current render protocol has no live attach/control endpoint that can add a new
sink to an already running binary. The straightforward path is a controlled clean
restart of the same saved world with the combined output enabled. Inspect the actual
deployment owner and shim contract before doing that switch; this review did not
operate the display or inspect its live process.

## Follow-up: fresh worlds mixed with older checkpoint generations

Root reported a running process launched with `--fresh --state state`, current
telemetry near tick 49,600, while that same directory contains older-world snapshots
as high as tick 446,277. These live observations are root's evidence; this reviewer
checked the code path read-only and did not operate the process or its state files.

The cause is confirmed in `state.rs::checkpoint_loop`: after writing every snapshot,
the worker calls `prune_snapshots`, which sorts filenames by descending numeric tick
and retains eight. It has no world identity or creation generation. Eight old files
with higher ticks can therefore cause the just-written current checkpoint to be
deleted immediately. `load_newest` selects those old files by the same ordering.
`--fresh` only skips loading; it does not isolate or retire old snapshots and it
appends observer logs to the same files. This is a real persistence defect, not
merely an ambiguous “latest” label.

The clean-stop final checkpoint uses the same worker and pruning path. Sending
SIGINT before isolating the stale snapshots can lose the final current-world state.
The runner reports its final tick and returns success even when the worker logged a
write failure, so successful process exit alone does not prove a usable checkpoint.

Recommended recovery sequence for the process owner:

1. Preserve the current executable, exact command line/config/art inputs, and the
   latest log evidence. Identify stale snapshots from the actual run history and
   decoded metadata; do not call the greatest filename tick the current world.
2. Make a recoverable archive outside the active directory's direct snapshot scan.
   Copy and verify the exact stale snapshot files, then relocate those exact files
   out of the active scan. Higher-than-current-run ticks are strong evidence here;
   equal or lower ticks require provenance rather than a blanket assumption.
   Re-list afterward to confirm no stale candidates remain that could win loading.
3. Leave the active state directory itself in place. The worker retains its path,
   not an open directory handle; renaming the whole directory while it runs would
   redirect subsequent writes to a missing or replacement location. Avoid handling
   `tmp-*.cubw` or a current write as though it were a completed stale snapshot.
4. Request one graceful SIGINT and wait for the original process to exit. Avoid a
   second interrupt, which exits without the final checkpoint. Read its final tick
   and all checkpoint error lines.
5. Decode and validate the resulting final snapshot. Its embedded tick must equal
   the final reported tick, and its state hash should match the final outcome where
   available. Preserve a verified copy separately before any schema migration.
   If this fails, do not assume an old high-tick snapshot is an acceptable substitute.
6. Resume using the verified state and compatible binary without `--fresh`. Verify
   startup reports the expected checkpoint and tick. Root retains ownership of the
   eventual combined-output switch and rollback.

The narrow prevention fix is to refuse `--fresh` when the selected state directory
already contains recognized snapshot files, before opening logs, spawning workers,
or opening sinks. Tell the operator to choose a new state directory. Enumerate the
directory with propagated I/O errors for this guard: existing `list_snapshots` treats
an unreadable directory as empty, which is unsuitable for a protective check.
Include stale temporary snapshot files and known nonempty persistent logs/journals
in the occupied-state policy so interrupted or partially initialized worlds do not
silently mix histories. Keep arbitrary unrelated files outside that narrow policy.

Add an integration regression with eight higher-tick old snapshots: a fresh launch
into that directory must fail before changing any snapshot or appending logs; a
fresh launch into a new directory must persist and resume its own final tick.
Normal resume into existing state should keep working. A state ownership lock closes
the race between checking the directory and another writer starting. Sorting by
modification time or always retaining the just-written low-tick file is insufficient:
neither gives `load_newest` a world identity or disentangles the mixed history.
