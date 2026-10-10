---
status: open
date: 2026-10-10
owner: renderer-worker, worktree `.claude/worktrees/rig-states`, branch `rig-states`
---

# RIG-STATES: sleep, idle and hunker move onto the bone rig

## Rules

Read `design/handoffs/worker-rules-2026-10-07.md`. In short:
- **Windows and Blender:** no windows; Blender runs `-b` only.
- **Time:** no tool call over 5 minutes (`timeout 270`). A longer bake or capture batch runs
  as one detached chain (`setsid nohup`, `chrt -i 0 nice -n 19`) ending in a DONE marker,
  and you hand back.
- **Locations:** seed your own `target/` by reflink. Worktrees and targets never go under
  `/tmp`.
- **Commits:** explicit paths, and only the files you meant to change. The bake is not
  reproducible (backlog 21).
- **Videos:** MP4s (x264 yuv444p crf 12) go to the main checkout's
  `art/gen/runs/rig-states-2026-10-10/`.
- **Unsafe:** only the `ash` FFI calls in `cubarium-gpu`.
- **Tests:** they check mechanisms, never tuned numbers.
- **Return:** 40 lines or fewer.

## Why

Every eater now has a feeding clip on the bone rig (`design/handoffs/feed-rollout-2026-10-10.md`,
merged through e3f7743d). The plan from the start was that sleep, idle and hunker would then
move onto the rig (FEED-RIG; Wrysk picked the rig 2026-10-10). Today they are drawn another
way:
- **The class-path pose** (`PoseEntry`, `cubarium-gpu/src/voxel.rs` `pose_with`): a sleep
  and hunker settle (`drop`), a sleep and idle breath, and the idle head-out, eased by
  per-entry rates.
- **The hunker pose model** (`frondgrazer-hunker`, the bake's `FOUNDER_POSES`, with its tests
  in `crates/cubarium/src/voxel/hunker_tests.rs`), swapped in while hunkered.
- **The loftstrider alone** already has an `asleep` track clip on the rig: Wrysk's rest,
  with the body lowered to the ground, spider-bent legs and flat feet.

A model swap can't ease, and a whole-body settle can't fold a leg. On the rig, each state
eases in and out like the cropping clip, and the parts move as the animal would.

## Round 1: the engine, and the frondgrazer

1. **Survey, briefly.** In the handback, say:
   - which fauna states drive sleep, idle and hunker, and how each is drawn today per
     species: class-path pose, pose model, or rig clip;
   - how a rigged animal's clips and the class-path pose combine now;
   - what the hunker swap does at its edges (does it pop?).
2. **Engine.** A rigged species' `idle`, `asleep` and `hunker` clips play in their states
   with eased weights, as `cropping` does. The loftstrider's `asleep` track is the
   precedent.
   - For a species that has the clip, the class-path pose stands down for that state.
   - For a species without one, nothing changes.
   - Say how clips blend when states overlap or hand over (for example cropping → hunker
     when a threat comes).
3. **The frondgrazer's three clips** (`scripts/blender/animal_models.py`, its rig as merged
   in e3f7743d):
   - **hunker:** the shield comes down onto the ground and the neck draws back into the
     notch. It must end on today's hunker pose, which is Wrysk's approved look and tested
     in `hunker_tests.rs`. The legs fold under the shield as that model folds them, so
     they need bones. Once the clip reaches that pose, the pose-model swap can go; say if
     anything still needs the model.
   - **asleep:** today's settle and breath, on the rig.
   - **idle:** today's head-out and breath.

   These are placeholders that reproduce today's look as the rig plays it: no new looks.
   Wrysk directs them from the video. The resting silhouette stays byte-identical (check it
   as the feeding rounds did).
4. **Evidence** (extend `crates/cubarium/examples/voxel_feed_capture.rs`, or add an example,
   to script the states): two grown frondgrazers, lit, real time, today | rig, side-on and
   three-quarter, at px 6 and px 12. Show:
   - an idle bout;
   - falling asleep, breathing, and waking;
   - hunkering in from standing, holding, and standing back up;
   - hunkering in from cropping.

   Report cells per bone.

## Return

- The survey answers.
- The engine change, with file:line.
- Each clip and what it reproduces.
- The silhouette and hunker checks, with raw evidence.
- MP4 paths.
- Tests.
- Commits.
- What the other species need next. For each of them, name its sleep, idle and any
  defensive pose today, and which parts would move.

Don't merge.
