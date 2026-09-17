# The voxel picture as a shader (VR-1) — measured, 2026-09-17

The voxel strip is now drawn per pixel by a slab walk in a fragment shader
(`crates/cubarium-gpu/shaders/voxel.frag`), fed by one voxel texture per tick,
through the present pass onto the existing targets. `cubarium voxel --sink gpu
--gpu-target window` shows the live generated world in the readback window with
the stdin controls working.

Desktop: NVIDIA GeForce RTX 5080, driver 615.71.09, `ash` + Vulkan 1.1 as
`Gpu::open` opens it (the first physical device the loader lists, which on this
machine is the 5080 rather than the 5090 — see the report's note).

## The picture agrees with the CPU presenter within one 8-bit code

`cargo run -p cubarium --release --example voxel_fidelity` renders the identical
`(World, Flora)` on both renderers over six scenes and writes CPU, GPU and diff
PNGs into `assets/voxel-render/fidelity/`.

| scene | worst channel diff | mean r/g/b | pixels differing by > 1 |
|---|---|---|---|
| authored, tick 0 | 1 | 0.007 / 0.002 / 0.018 | 0 of 122,880 |
| authored, 60 s of default rain | 1 | 0.013 / 0.006 / 0.012 | 0 |
| generated, tick 0 | 1 | 0.011 / 0.011 / 0.022 | 0 |
| generated, 60 s of default rain | 1 | 0.029 / 0.017 / 0.018 | 0 |
| hand-built fixture strip | 1 | 0.004 / 0.002 / 0.011 | 0 |
| authored at 400 s (a seed bank exists) | 1 | 0.015 / 0.016 / 0.009 | 0 |

The same at `--px 8` (1024 × 504): worst 1, no pixel over 1, on all six.

One code is the floor and not a defect: the CPU canvas encodes sRGB through
`cubarium_render::srgb`'s table and the GPU writes linear into an
`R8G8B8A8_SRGB` attachment the hardware encodes, which Vulkan pins to within
0.6 ULP of the correctly rounded value rather than to that table. The two
renderers do not share an encoder and never will.

The one knowingly lossy step in the data path is free water quantised to eight
bits (± 1/510 of the fraction). It can move a drawn water row only where
`free · s` sits within that of a half pixel, and it did not in any scene here.

## The per-rule checklist

Summed over the six scenes at `--px 4`. **Voxels** is how many voxels in those
scenes satisfy the rule's geometric precondition — whether the picture contains
the rule at all, because a rule nobody drew cannot have passed. **Worst** is the
largest single-channel CPU/GPU difference over the pixels of those voxels'
governed rectangles. **Verdicts** is the local pixel pair that decides the rule
(a rim row against the row below it, a bevel column against its neighbour, a
trunk's lit column against its far edge) read on both renderers; a pair inside
one 8-bit code of luminance is a **tie** and counts as neither, because the two
renderers do not share an sRGB encoder.

| rule | voxels | scenes | worst | verdicts |
|---|---|---|---|---|
| rim light on an exposed top row | 15,029 | 6 | 1 | 1038 / 1038, 13,168 ties |
| side bevel on an exposed flank | 4,940 | 6 | 1 | 1475 / 1475, 3,269 ties |
| chamfer corner where rim meets flank | 3,785 | 6 | 1 | 289 / 289, 3,171 ties |
| riser lean into depth, with no rim | 3,827 | 5 | 1 | 3658 / 3658, 42 ties |
| top-face contour where the ground ends going back | 575–1,725 | 6 | 1 | inactive at `s` = 4 — see below |
| contour suppressed on a plateau receding in z | 13,531 | 6 | 1 | census only |
| roof shadow under a lip, falling off with depth | 424 | 4 | 1 | census only |
| per-row haze across a cap and a riser | 18,856 | 6 | 1 | census only |
| pore-water darkening of soil and rock | 33,918 | 4 | 1 | census only |
| free water filling from the bottom of its voxel | 3,566 | 4 | 1 | 782 / 782, 2,766 ties |
| one water surface per pixel across a deep pool | 3,405 | 5 | 1 | census only |
| a partial roof clipping a water top row by row | 15 | 1 | 1 | census only |
| trunk cylinder shading across four pixels | 71 | 5 | 1 | 58 / 58, 13 ties |
| crown silhouette edge and shaded skirt | 343 | 5 | 1 | 116 / 116, 0 ties |
| sprout glyph on a seed bank | 2 | 1 | 1 | census only |

Two rules needed a scene built for them, and the census is what said so.

- **The partial roof over a water top.** The row-by-row `nearer_owns` clip does
  not occur anywhere in the authored fixture or the generated world at any tick
  tried — zero voxels in four scenes. The `fixtures` scene rebuilds the
  presenter's own `a_partial_roof_clips_a_water_top` geometry twelve times, and
  15 voxels of it land in the picture. Worst 1.
- **The sprout glyph.** A seed cohort does not exist before roughly 400 s of the
  default rain at `propagule_rate` 0.0002, and is gone again by 1000 s. The
  `authored-sprout` scene sits in that window; 2 sprout voxels, worst 1.

**The contour row needs `s` ≥ 3 px of depth step to exist at all**: the
presenter draws it only when `rise > 2`, and the chosen camera's `rise` is 2, so
at the default projection neither renderer draws it and the census counts a
precondition that nothing acts on. At `--px 8` (`rise` = 5) it is live, and
**4,732 of 4,732 verdicts agree** across the six scenes with 547 ties.

## Frame time

`--bench 500` on the authored scene, no readback, one frame at a time. The CPU
figure is `VoxelPresenter::draw` plus `Canvas::encode_raster` — what a frame of
`--sink web` or `--sink png` costs before a sink sees a pixel. The GPU figure is
record, submit and wait on the fence, i.e. the whole frame and not just the
shader.

| raster | px/voxel | depth step | CPU presenter | GPU frame | ratio |
|---|---|---|---|---|---|
| 512 × 240 | 4 | 2 px | 1.326 ms | 0.073 ms | 18 × |
| 1024 × 504 | 8 | 5 px | 2.942 ms | 0.107 ms | 27 × |
| 1536 × 744 | 12 | 7 px | 5.524 ms | 0.170 ms | 32 × |

The GPU's own timestamps over an 8 s run at 60 fps, split by pass:

| px/voxel | upload | slab walk | present | total |
|---|---|---|---|---|
| 4 | 0.022 ms | 0.027 ms | 0 (headless) | 0.049 ms |
| 8 | 0.022 ms | 0.048 ms | 0 (headless) | 0.071 ms |

The run loop caps at `clock::MAX_FPS` = 240, and both renderers clear 240 fps at
these sizes on this desktop, so a run's reported fps cannot separate them. The
numbers above can.

## The per-tick upload

One texel per voxel, 128 × 48 × 24 at the defaults:

| plane | format | bytes |
|---|---|---|
| material + plant part, free water, pore water, plant style | `R8G8B8A8_UINT` 3D | 576 KiB |
| the roof-gap table (`build_roof`) | `R8_UINT` 3D | 144 KiB |
| the frame's plant styles: wood, crown, heart | `R32G32B32A32_SFLOAT` 3 × 256 | 12 KiB |
| **total** | | **732 KiB** |

CPU cost of packing one tick, straight into mapped memory: **1.03 ms**. At the
world's 20 Hz that is 2.1 % of one core, and it is per *tick*, not per frame —
a frame is one full-screen triangle over whatever was last staged, which is why
the raster can be 4, 8 or 12 px per voxel over the same data.

## The roof table against a column walk in the shader

The brief asked which is cheaper. Measured both ways over the same 8 s run:

| px/voxel | roof from the uploaded table | roof walked in the shader |
|---|---|---|
| 4 | 0.027 ms slab walk | 0.031 ms (+15 %) |
| 8 | 0.048 ms | 0.059 ms (+23 %) |

**The table wins**, by less than the 144 KiB it costs would suggest — the walk
terminates at the first solid above, which on this terrain is usually a few
texels. `--gpu-roof-walk` keeps the walk available; the picture is identical
either way (the fidelity example agrees to the same one code with it on).

## The window

`cubarium voxel --scene generated --sink gpu --gpu-target window --fps 60` ran
15.8 s at **60.0 fps** (949 frames, 277 ticks) and answered every stdin command:
`r 5`, `a -20`, `m 10 20 0 rock`, `f 30 2 bloomcrown`, `i 10 20 0`, `p`, `s`,
`q`. GPU 0.053 ms/frame; the window target's own 4.3 ms/frame is the `minifb`
readback, the nearest ×3 upscale into its buffer and `update_with_buffer`, which
is the price of a development window and not of the renderer.

## Captures

`assets/voxel-render/`, from a `--gpu-target headless --gpu-capture` run at
10 fps with `initial_aquifer_head_m = 6.0` (the only change from the defaults,
so that `a` has a water table to withdraw from). Headless and not the window on
the owner's instruction, and it makes no difference to the bytes: the window
target draws that same raster and then reads it back to blit it, so a capture is
the window's own content either way. The names keep `window-` because the window
is the target they are evidence for.

- `window-1-fresh-world.png` — the generated world at tick ~20;
- `window-2-after-a-40m3-rain-pulse.png` — after `r 40` and 200 ticks;
- `window-3-after-the-aquifer-is-withdrawn.png` — after `a -400` (353 m³
  accepted, head 6.0 → 0.0 m) and 480 ticks.

Captures cost 3.5 ms/frame (raster readback plus the PNG encode), which is why
they are not on the shipped path.
