---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Shared cube/web instance: verified rollout

Implemented and committed by the Fable/Opus viewer worker as `e3ad20f`; independently
checked and deployed by root under Wrysk's request. No shim/hardware mapping changes.

## What changed

`--mirror-web` fans one owning runner's encoded Frame out to the primary sink and
a web sink. The world and renderer run once. The web `/status` distinguishes the
simulation tick from `/frame`'s render sequence and reports source PID, state path,
build, primary sink, and resume point. Browser disconnects do not stop the world.

The actual cube's shared viewer is **http://127.0.0.1:7393/**. Pre-existing viewers
7395,7396,7397 are separate worlds and were left untouched. The megafauna studio on
7400 is an art study, also separate; its candidate selection changes nothing live.

## Safety finding and preserved history

The original cube PID2295402 ran `--fresh --state state` while that directory retained
eight older-world snapshots with higher numeric ticks (438000–446277). Checkpoint
retention sorted by numeric tick and could immediately discard the current fresh
world's checkpoints, including its final shutdown checkpoint.

Root moved those eight exact stale files to
`captures/checkpoints/pre-care/retired-history/`. Nothing was deleted and the active
directory stayed in place. Current snapshots then persisted (52800,54000,55200 and
later). A read-only decoder verified the current55200 backup. Future prevention
is tracked in the care/state-lock package: refuse --fresh in an occupied state dir.

## Controlled handover

1. Preserved the actual old executable from `/proc/2295402/exe` and current-run
   checkpoint copies in the gitignored `captures/checkpoints/pre-care/` directory.
2. Root reran `cargo test -p cubarium --test mirror_web --quiet`: three passed,
   including exact HTTP/UDP/encoded-frame equality and unchanged simulated state.
3. The preserved viewer binary resumed a copy of55200 for20 ticks, wrote55220,
   and the read-only inspector validated that resulting schema7 snapshot.
4. Sent SIGINT to the verified old cube PID, confirmed exit, then decoded/copied
   its newly written final checkpoint67984: population94, schema7,
   state hash16684878898031239227. Its SHA256 is
   `278aa850c9df61328eb29e395b40cb4bbb910bcb610dc6f5f7dcc9af71924cf9`.
5. Resumed WITHOUT --fresh using the preserved e3ad20f binary:

   ```sh
   captures/checkpoints/shared-viewer-v7/cubarium run --art assets/atelier \
     --sink shim --mirror-web --web-port 7393 --state state --fps 60 --speed 1
   ```

   It reported resuming67984. Root execsession48903, PID2359616 at launch.
6. HTTP status independently confirmed primary sink `shim`, the expected state
   directory/build/resume point, and world progression68702→68748. Every sampled
   frame body was61448 bytes; source submission rate was60.06fps over about2s.
   These are transport/source checks, not a physical panel pacing measurement.

The frozen shared-viewer executable SHA256 is
`993a13709b5df582c6787f3d1e2160eee0944023331ed83866621ceb13bb4742`.
The pre-care backup README records exact recovery files and commands. Restore an
older history into a NEW state directory; never mix its snapshots into the current
history. The running frozen v7 binary remains independent of subsequent care builds.

Care controls and schema8 were not enabled at this handover. They need their own
validated migration and controlled upgrade before affecting this world.
