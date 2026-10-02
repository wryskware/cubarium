# Change-only pack: each tick packs and uploads only what changed — 2026-10-02

Wrysk, 2026-10-02, on the pack growing from 12 to 14.5 ms/tick for the glowcap candle and the
vine tiles: "adding glowy mushrooms in unreal or unity doesnt seem like it would add a bunch of
additional 'render prep'. and certainly neither would vine textures" / "i would prioritize 3"
(3 = pack and upload only what changed, instead of rebuilding the world every tick).

Worktree `.claude/worktrees/change-pack`, branch `change-pack` off main. Commit with explicit
paths; never merge, never deploy, never touch the live world, never open a window (AGENTS.md:
headless captures only). Follow `WORKING_POLICY.md`. **CPU:** no `taskset` pinning (this
overrides the agent definition: Wrysk 2026-10-01, let the scheduler spread), `nice -n 10`,
`CUBARIUM_THREADS=16`, `cargo … -j 16`. Stop your wait loops; leave no background shells.
Make the routine design calls yourself and report them.

## Why

Every sim tick (20 Hz) the host packer rebuilds the whole picture from the sim and uploads all
of it: `Packer::prepare` re-stamps every stand and animal, recomputes every vine cell, rebuilds
the canopy plane and the glow volume; `Packer::fill` writes every voxel of the 256×128×48
terrarium into the staging buffer (`crates/cubarium/src/sink/gpu/voxel.rs` ~880–1100), and the
renderer copies the whole volume (`crates/cubarium-gpu/src/voxel.rs`, `VoxelStaging` ~1176,
`StagingRing` ~1276, `VoxelRenderer::stage` ~2308). Anything added to the world costs on every
tick even when it did not change: the vine moves a few faces a minute, and its cells cost
1.4 ms/tick; the glowcap candle defeats the glow volume's cache, +0.5 ms/tick.

The 2026-09-21 verdict "change-only packing — replaced by a cheap full pack"
(`design/handoffs/terrain-generation-briefs-2026-09-21.md`, package PB) was taken when the pack
was a few ms. It has grown since (models, lit-tier planes, canopy, glow, vine tiles). Wrysk now
puts change-only first.

## Goal

A tick's CPU work and upload scale with what changed in it, in both tiers (flat: panel and
cube; lit: desktop). The picture is the full pack's, tick for tick. A first frame, a world load
or a terrain edit may pack in full.

- **Every layer the packer produces:** voxel texels (material, free and pore water, stands,
  animals, carcasses, vine cells), the style table, the pose table, the candle curve table, the
  roof (already terrain-keyed), the lit tier's canopy plane and glow volume (the sky plane is
  already terrain-keyed).
- **Upload:** only changed regions reach the GPU (bricks or row spans, your call). With several
  staging buffers in flight, each one gets every change made since it was last used.
- **Style slots stay put across ticks** (a persistent palette that reuses freed slots), or every
  texel's slot churns and nothing is "unchanged". Running out of slots behaves no worse than
  today.
- **Glow:** keep one glow grid per candle curve plus one for steady emitters. A grid is spread
  again only when its emitters or the terrain change. The candle's brightness mixes the grids
  on the GPU, or on the CPU only if that is measured at ≤ 0.1 ms/tick. The cast light follows
  the candle at least once per tick, as today; once per frame is a bonus if it comes free.
  `Candle::brightness` stays the one brightness function, because a later animal-proximity
  spike joins there.
- **Where the change information comes from:** prefer versions or change lists the sim already
  keeps. Additive, read-only accessors in the sim crates are allowed (no change to sim
  behaviour, sim tests untouched). Diffing a compact source array against last tick's copy is
  fine where it is cheaper than plumbing.
- Keep the panel's path working. If its GPU backend makes sub-region copies dear, say so at
  the checkpoint rather than working around it silently.

Out of scope: the lit tier redrawing every frame while lips show (a GPU cost per frame, not
the pack); animal-movement glow spikes; any look change.

## Checkpoint 1: measure and design, then stop

1. Profile today's pack phase by phase on the desktop terrarium
   (`config/desktop/terrarium.toml`, seed 1, around sim minute 110, lit, 6 px) **with the sim
   advancing**. `light_capture --repack` re-packs one frozen state, so a change-only pack would
   measure zero there. Profile the tachyon config (flat tier) on the desk the same way.
2. Count what actually changes per tick, per layer: voxels per tick for water, animals, stands,
   vine and carcasses, and how often the style table, canopy and emitters change. Give median
   and p95 numbers, and figures through a shower.
3. Return the design in ≤ 40 lines: for each layer, what changes and where the change
   information comes from; the upload scheme; the style-slot scheme; the glow scheme; the
   expected ms per tick. Stop and wait for the go.

## Tests first (after the go, one failing commit), then the implementation

Keep today's full pack as the oracle: the `reference` pack and the `Rig::tick` pattern in
`sink/gpu/voxel.rs` tests, from PB. Compare what the shader draws, not raw slot numbers: each
texel with its style resolved.

1. On a fixture world, after **every** tick of a scripted sequence, the change-only pack draws
   what the full pack draws. The sequence: an animal walks; a stand is born, grows a stage and
   dies; a carcass appears and decays; the vine spreads a face and flowers; water flows (a
   shower); a terrain edit; a glowcap appears and dies. Run it lit and flat.
2. Staging ring: buffers used in turn, and one that skipped ticks, each match the full pack
   after their upload.
3. Style palette: over time more distinct styles than `MAX_STYLES` (fewer at once) never
   exhaust it, and a reused slot never leaves a stale texel.
4. Glow: the per-curve mix at a given brightness equals the full glow build at that
   brightness within a stated tolerance; a tick with no emitter or terrain change does no
   spread.
5. A tick where nothing changed uploads no regions.

`cargo nextest run -p cubarium -p cubarium-gpu`, plus any sim crate you touch.

## Checks and return

- Pack ms per tick (median, p95, max) and bytes uploaded per tick, before and after: desktop
  terrarium lit at 6 px, and the tachyon config flat, both with the sim advancing. Target: a
  typical desktop tick ≤ 2 ms (from 14.5). If it isn't met, say what dominates.
- One still of the desktop terrarium at 6 px from this branch, after a few minutes of sim, as a
  sanity check. Give its absolute path.
- Return ≤ 40 lines: commits, the design calls you made, the numbers, and anything that refused
  a decision here.
