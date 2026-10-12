---
status: open
date: 2026-10-11
owner: Codex (xhigh), worktree `.claude/worktrees/fog-occlude`, branch `fog-occlude`
---

# FOG-OCCLUDE: the weather stops at the plant or animal in front

## Rules

Read `design/handoffs/worker-rules-2026-10-07.md`. In brief:
- **Windows:** never open one. Captures run headless (`--gpu-target headless`) or through
  `scripts/hidden.sh`.
- **Time:** no tool call over 5 minutes (`timeout 270`).
  - A longer build, capture or timing batch runs as one detached chain (`setsid nohup`,
    `chrt -i 0 nice -n 19`) that ends in a DONE marker. Then hand back: what is running,
    where it lands, and what's left.
  - Never wait, poll or sleep.
- **Builds:** seed your `target/` by reflink from a recent build. Never put worktrees or
  targets under `/tmp`.
- **Commits:**
  - Explicit paths only.
  - Edit the WGSL in `crates/cubarium-gpu/shaders/` (the only shader source since WGSL-SOURCE, 2026-10-12; no compile step).
  - Sign commits `Co-Authored-By: Codex (gpt-6.1-sol) <noreply@openai.com>`.
- **Unsafe:** only the `ash` FFI calls in `cubarium-gpu`.
- **Tests:** they check mechanisms, never tuned numbers.
- **MP4s:** x264 yuv444p crf 12, written to the main checkout's
  `art/gen/runs/fog-occlude-2026-10-11/`. Copy them out before you hand back.

## Why

Wrysk (2026-10-11), on the fog not being hidden behind plants and animals: "would that be
complicated / expensive to render?" I told him it's cheap and small, and he said build it.

**The cause.**
- The lit frame draws, in order (`crates/cubarium-gpu/src/voxel.rs` ~3571-3610):
  1. the walk;
  2. the plant pass (`record_vis`, `record_shade`);
  3. the figures (animals);
  4. the weather pass;
  5. bloom.
- The weather pass (`voxel.frag` `weatherOver`, ~1808) reads only the **walk's** end depth:
  `zEnd`, stored in the emission's alpha under `WEATHER_DEPTH` (~2151).
- It integrates the fog along each pixel's ray from the front all the way to the terrain
  behind.
- So a plant or animal drawn in front picks up fog from the air behind it: an animal standing
  in a bank, or a vaulttree crown above a foggy hollow, wears the hollow's fog.
- Rain layers and the bolt use the same `zEnd`, so they have the same fault.

## Do

1. **Give the weather pass the nearest depth.** Per pixel, take the minimum of the walk's end
   depth, the plant pass's depth and the figures' depth, in the walk's slab units. Use it
   for everything the weather pass draws: fog, the rain haze, the streaks and splashes, the
   bolt and the fog's glow sample.
   - The plant depth image is already `SAMPLED` (`plants.rs` ~394).
   - The figures' depth isn't (`figures.rs` ~881): add the usage, and the layout transition
     and barrier the weather pass needs to read it.
   - Work out each pass's depth encoding, and convert it exactly. Say how in the handback.
   - The water rule stays: the weather stops at a water surface in front (`wEntry`).
2. **Edges.** Figure silhouettes have partial coverage at their rims ("rotated silhouettes
   and edging gain partial coverage"). Make sure the fog shows no halo or dark fringe round
   an animal or a crown. If coverage needs blending between the two depths, do that, and say
   what you did.
3. **The other paths.**
   - With `[light]` volumetric on, the march carries the fog in the walk. Say whether it has
     the same fault, and fix it only if that is cheap.
   - The flat tier has no weather pass. Leave it.
4. **Nothing else changes.**
   - A frame with no weather is byte-identical to main: the pass is skipped.
   - A foggy frame with no plant or animal in front of the fog is identical too. Prove both
     with `cmp` on captures.
5. **Cost.** Time the weather pass on a foggy frame, main against the branch, interleaved, at
   the desktop's scale (px 6). If it adds more than about 0.1 ms per frame, stop and report
   why before polishing.

## Evidence

- **Headless, lit, fog on, today | fixed:**
  - animals standing in a fog bank;
  - a vaulttree grove above a foggy low ground;
  - stills at px 6 and px 12, and one short real-time MP4 with animals walking through fog.
- **Finding a scene:** FOG-MOTION's captures forced fog on a saved state. Find how in
  `design/handoffs/fog-motion-2026-10-10.md` and `art/gen/runs/fog-motion-2026-10-10/`.
- **Tests:** a mechanism test (follow the patterns in `cubarium-gpu`'s render tests). An
  opaque block in front of a fogged ground gets the fog of the air in front of it, not of
  the ground behind it, and the ground around it is unchanged.

## Return (40 lines or fewer)

- The depth encodings and the conversion, with file:line.
- The edge handling.
- The `cmp` results.
- The cost numbers.
- Stills and MP4 paths.
- Tests.
- Commits.

Don't merge.
