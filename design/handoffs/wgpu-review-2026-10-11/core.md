---
design_status: exploration
last_reviewed: 2026-10-12
---

# WGPU-REVIEW: CORE PASSES

Scope: `crates/cubarium-gpu/src/wgpu_backend.rs`, `wgpu_backend/*`, and the crate's CPU
modules (`render.rs voxel.rs figures.rs plants.rs bloom.rs present.rs sunvis.rs weather.rs
cord.rs lib.rs`) plus `Cargo.toml`, at `ce035f3a`, compared with the ash code at `953e688f`.
Everything below comes from reading the source. Nothing was built or run.

wgpu internals I relied on, read from the vendored `wgpu-core-30.0.1` source:
- `create_buffer_init` without `MAP_WRITE` (every per-frame buffer here) does the
  following, all of it per call:
  - it creates a device-local buffer **and** a separate `StagingBuffer`;
  - it zero-fills the staging buffer with `write_zeros`, then copies `contents` into it;
  - on unmap, it records a GPU `copy_buffer_to_buffer` into `queue.pending_writes`.
- Sources: `device/resource.rs:1142-1170`, `resource.rs:1000-1033`.

The cost tags below follow the brief: **per frame**, **per redraw** (lit tier with
swaying plants, posing animals or animated water: every frame), **per tick/upload**,
**per brick**, **per draw**, **per world**.

---

## Findings (most severe first)

### F1. HIGH, perf: voxel uploads went from zero-copy into mapped memory to four copies, with one staging allocation per brick
- **Where:** `wgpu_backend/voxel.rs:1164-1253` (the brick loop is at 1179-1202) and
  `wgpu_backend.rs:264-325` (`Gpu::upload_texture`).
- **Defect:** every upload, whether a whole plane or one 8³ brick, makes its own padded
  `Vec`, makes its own `create_buffer_init` staging buffer, and records its own
  `copy_buffer_to_texture`. Ash packed straight into a persistently mapped per-slot buffer
  and issued one `vkCmdCopyBufferToImage` that carried N regions.
- **Brick path, per carried brick:**
  1. It allocates a `texels` Vec and gathers 2 KiB into it.
  2. It allocates a `padded` Vec of 16 KiB, because 32-byte rows are padded to the 256-byte
     `COPY_BYTES_PER_ROW_ALIGNMENT`: 8× inflation.
  3. `create_buffer_init` creates a 16 KiB device buffer and a 16 KiB staging buffer, zeroes
     the staging buffer, copies into it, and adds a GPU buffer→buffer copy.
  4. The pass adds a GPU buffer→texture copy.
- **Full-plane path** (voxels, roof, sky, canopy, glow, styles, glyphs):
  - it zero-allocates a padded Vec and copies into it, even when `stride == row`;
  - the staging buffer is zero-filled and the data copied in again;
  - then it costs two GPU copies.
- **Failure scenario: a partial upload.** Take a 256×96×96 world: 4608 bricks.
  - `write_upload` (`voxel.rs:1810-1813`) carries bricks until they exceed 25 % of the
    total, here 1152 bricks.
  - The frame then makes 1152 × 2 buffer allocations, about 37 MiB of staging (half of it
    zeroed) for 2.3 MiB of real data, plus 2304 GPU copy commands.
  - Ash, for the same frame: one copy command, 2.3 MiB, no allocation.
- **Failure scenario: a full upload.** For the same world: about 9.4 MB of voxels plus
  2.4 MB each of roof and sky. Each goes through a Vec, a zeroed staging buffer and two GPU
  copies, roughly 4× ash's memory traffic.
- **Scaling:** per tick that carries an upload, linear in the number of changed bricks;
  full uploads are linear in the volume.
- **Evidence:**
  - new: `let mut texels = Vec::with_capacity(...)` … `gpu.upload_texture(encoder, &self.world[0].texture, wgpu::Origin3d { x, y, z }, extent(size), 4, bytemuck::cast_slice(&texels))?;`
    and `let mut padded = vec![0; stride as usize * rows]; … create_buffer_init(... contents: &padded, usage: COPY_SRC)`;
  - ash `953e688f:voxel.rs:3307-3330` packs straight into mapped memory with
    `std::slice::from_raw_parts_mut(buffer.ptr().add(at(AT_VOXELS)) …)`, and
    `voxel.rs:1773-1806` (`brick_regions`) feeds one
    `d.cmd_copy_buffer_to_image(cb, source, self.voxel_image, …, &regions)`.
- **Fix direction:** keep a persistent per-slot `COPY_SRC|MAP_WRITE` staging buffer,
  re-mapped on retire, or a `wgpu::util::StagingBelt`. Pack each brick at a 256-byte row
  pitch and issue all copies from one buffer at offsets. That keeps the "encoder owns the
  upload" rule.
- **Verdict:** CONFIRMED (mechanism traced, including wgpu-core). The millisecond figures
  are PLAUSIBLE. To measure, time `record_inner` with 0, 100 and 1000 owed bricks, then a
  full upload, on ash and on wgpu.

### F2. HIGH, perf: every recorded frame, skipped or not, creates 5 buffers, a query set and 7 bind groups. Ash created none.
- **Where:**
  - `wgpu_backend/timing.rs:32-64`: `Timing::begin`, called from `voxel.rs:1052` and
    `flat.rs:331`;
  - `voxel.rs:1263-1281`: the uniform and emitter buffers;
  - `voxel.rs:1340-1369`: six `world_group` bind groups of 22 entries each;
  - `wgpu_backend.rs:411-431`: `PresentPass::record` makes one uniform buffer and one bind
    group per target.
- **Defect:** per-frame GPU objects that ash created once and reused through ring offsets.
- **Answer to the parent's question:**
  - `Timing::begin` runs on **every** `VoxelRenderer::record` (12 queries) and every flat
    `Renderer::record` (4 queries) whenever the adapter exposes
    `TIMESTAMP_QUERY_INSIDE_ENCODERS`.
  - `Gpu::open_instance` (`wgpu_backend.rs:170-174`) always requests that feature when it
    exists, so the cost is unconditional on desktop Vulkan and does not depend on `redraw`.
  - Each call creates one `QuerySet`, a `QUERY_RESOLVE` buffer and a `MAP_READ` buffer, and
    adds a `map_buffer_on_submit` callback.
- **Totals for a skip frame** (`!redraw`, e.g. the flat tier between ticks):
  - 2 `create_buffer_init`, each a device buffer plus a zeroed staging buffer plus a GPU copy;
  - 2 timing buffers and 1 query set;
  - 6 world bind groups, none of which a skip frame uses;
  - with a target, 1 present uniform and 1 present bind group.
- **Totals for a lit redraw frame:** the skip-frame objects, plus figures (5 buffers and
  1 bind group, F3) and plants (2 buffers and 1 bind group). That comes to 12 buffers, about
  10 hidden staging buffers, 1 query set and 9 bind groups per frame.
- **Ash, for the same work, created everything once:**
  - query pool `953e688f:voxel.rs:2287-2295`, `QUERY_SLOTS * STAGING_RING`, reset per frame;
  - one uniform host buffer written at `uniform_stride * slot` (`:3540`);
  - one emitter host buffer of `EMIT_TABLE*16*STAGING_RING` (`:2634`);
  - one descriptor set with a dynamic offset (`:2625`).
- **Scaling:** per frame (timing, uniform, emitter, 6 bind groups), per target (present),
  and per redraw (figures, plants).
- **Fix direction:**
  - keep `STAGING_RING` query sets, resolve buffers, uniform slots and emitter regions, as
    ash did;
  - build the world bind groups once per view set; they only change when textures are
    recreated;
  - skip building bind groups on `!redraw`.
- **Verdict:** CONFIRMED.

### F3. HIGH/MEDIUM, perf: large unchanged data is re-uploaded on every redraw
- **Where:**
  - `wgpu_backend/geometry.rs:36-45` (`buffer()`, which starts with `bytes.to_vec()`);
  - `geometry.rs:467-474` (`Figures::prepare`);
  - `geometry.rs:804-811` (`Plants::prepare`);
  - `voxel.rs:1271-1281` (emitter table).
- **Defect:** three buffers are rebuilt and re-uploaded every time even when unchanged:
  - figure models, the whole append-only model store, up to `MAX_VOXELS` × 16 B = 4 MiB;
  - plant cells, up to `MAX_PLANT_CELLS` × 16 B = 4 MiB;
  - the emitter table, up to 32768 × 16 B = 512 KiB, every frame.
- **Each upload copies the data twice on the CPU** (`to_vec`, then staging), zero-fills the
  staging buffer, and adds a GPU copy.
- **Ash wrote each of these only when it changed:**
  - models once, in `put_models` (`953e688f:figures.rs:1251-1263`, `self.models.write_at(first, cells)`);
  - plant cells once per slot per version (`953e688f:plants.rs:741-750`,
    `if self.written[slot] != self.version { … }`);
  - the emitter table only when `cap > 0`.
- **Failure scenario:**
  - The lit tier redraws every frame while plants sway (`set_clock` → `p.set_clock`,
    `voxel.rs:187-189`).
  - With 31.6k plant cells (the count in the `plants.rs` header) and a few tens of
    thousands of model cells, that is about 1–2 MB of redundant memcpy, memset and GPU copy
    per frame, 60–120 MB/s at 60 fps.
- **Scaling:**
  - plants: per redraw, ∝ plant cells;
  - figures: per redraw, ∝ total model cells, not visible ones;
  - emitter table: per frame, ∝ the hash capacity.
- **Verdict:** CONFIRMED.

### F4. MEDIUM, correctness: no wgpu error handler or device-lost callback, so GPU errors panic where ash returned `Err`
- **Where:** `wgpu_backend.rs:190-207`. A `git grep` over `crates/` finds no
  `on_uncaptured_error`, `set_device_lost_callback` or error scopes outside examples.
- **Defect:** wgpu's default uncaptured-error handler panics. All of these therefore panic
  on whatever thread hit them, instead of propagating `vk::Result` errors as ash did:
  - validation, out-of-memory and device-lost errors;
  - errors from `create_buffer_init`, `queue.submit` and `surface.configure`.
- **Failure scenario:**
  - A driver reset or OOM is made more likely by F1/F3's multi-MiB per-frame staging.
  - With one `Arc<Gpu>` shared by Godot previews, live worlds and the warm thread, the
    first error panics inside a render call rather than giving the sink a chance to stop
    the target.
  - On an adapter without `IMMEDIATES`, `max_immediate_size` is 0 (`:183-187`). Every
    pipeline layout with immediates (figures 32, bloom 48, plants 64) is then a validation
    panic, not an error.
- **Verdict:**
  - CONFIRMED: the handler and callback are absent.
  - PLAUSIBLE: the crash path. To check, force device loss (or set
    `CUBARIUM_GPU` to a GL adapter without immediates) and observe a panic versus `Err`.

### F5. MEDIUM, perf/behaviour: the window path is forced to FIFO on the caller's thread; ash preferred MAILBOX on its own presenting thread
- **Where:** `wgpu_backend/window.rs:248`: `config.present_mode = wgpu::PresentMode::Fifo;`.
  `acquire` (`:119-180`) and `submit` (`:206-221`) run inside the caller's `draw`
  (`crates/cubarium/src/sink/gpu/window_target.rs:56-90`).
- **Ash:**
  - `953e688f:target/window.rs:552-560` picked `MAILBOX` when offered, with FIFO only
    under `CUBARIUM_WINDOW_FIFO=1`;
  - all WSI calls ran on a separate thread, because "a FIFO present to a window that is on
    another workspace can block indefinitely: that stops this thread and the picture, and
    not the world" (`:23-26`).
- **Failure scenario:**
  - The development window now paces the caller to vsync, and `get_current_texture` and
    `present` block the run-loop thread.
  - On Wayland/NVIDIA with the window on another workspace, the world loop stalls for the
    acquire timeout every draw. The old design existed to avoid exactly this.
- **Scaling:** per window frame.
- **Not disclosed:** the handoff does not mention this change.
- **Overlap:** host reviewer (`sink/gpu`).
- **Verdict:**
  - CONFIRMED: the mode change and the same-thread acquire/present.
  - PLAUSIBLE: the stall. To check, put the window on another workspace with NVIDIA
    Wayland and time `WindowTarget::draw`.

### F6. MEDIUM, perf: the `MemoryHints::MemoryUsage` device hint makes per-frame staging likely to hit real allocations
- **Where:** `wgpu_backend.rs:195`.
- **Defect:**
  - `wgpu-hal-30.0.1/src/lib.rs:451-456` maps `MemoryUsage` to device blocks of 8–64 MiB and
    host blocks of **4–32 MiB**, against 128–256 / 64–128 MiB for `Performance`.
  - With F1 to F3 creating several multi-MiB staging buffers per frame (full uploads,
    plant cells, models), allocations larger than or near the host block size get their own
    `vkAllocateMemory`/`vkFreeMemory` per frame.
- **Scaling:** per frame, ∝ upload size.
- **Verdict:** PLAUSIBLE. To check, count `vkAllocateMemory` per frame with the API-dump
  layer, or call `device.generate_allocator_report()` before and after N frames.

### F7. MEDIUM, perf: readback adds per-frame CPU copies, and the panel lost zero-copy scanout
- **Where:** `wgpu_backend.rs:622-627` (`ReadbackRing::poll`):
  `view.chunks_exact(stride).flat_map(|r| r[..row].iter().copied()).collect()`.
- **Defect:**
  - It allocates and copies a fresh `Vec` per frame, byte by byte, from an iterator with no
    exact size hint, so the Vec reallocates repeatedly.
  - Ash called `done(tag, &bytes[..len])` on the mapped host-cached memory, with no copy
    (`953e688f:target/readback.rs:144-160`).
- **Overlap (`target/panel_readback.rs`, the other reviewer's file):**
  - `PanelImage::draw` does `drain` (a blocking `poll(Wait)`), then `rgba.to_vec()`, then
    `scanout.write_rgba`, then stores the copy (`:70-78`).
  - Ash's `target/dmabuf.rs` rendered straight into the scanout buffer: "No readback, no
    blit, the CPU never touches a pixel".
  - The handoff discloses the copy. It does not mention the blocking wait.
- **Failure scenario:** at a 1920×1080 Tachyon face, about 8 MB per frame goes through 3–4
  CPU copies, plus a synchronous GPU wait per frame.
- **Scaling:** per frame, ∝ pixels.
- **Verdict:**
  - CONFIRMED: the mechanism.
  - PLAUSIBLE: the ms cost. To check, time `PanelImage::draw` on the Tachyon against
    `953e688f`.

### F8. LOW, perf: the flat renderer rebuilds all frame data each frame
- **Where:** `wgpu_backend/flat.rs:298-330`, `:351-360` and `:384-388`.
- **Defect:** each frame creates:
  - a scene uniform buffer;
  - an instance buffer of up to 8192 × `SpriteInstance` (~136 B), about 1.1 MiB, plus a
    `Vec` gather;
  - a bind group;
  - the timing objects (F2);
  - one staging buffer per scratch sprite region.
- **Ash:** persistent host buffers (`953e688f:render.rs:408-413`, `:698`
  `self.instances.write_at`).
- **Scaling:** per frame, ∝ instances and scratch regions.
- **Verdict:** CONFIRMED.

### F9. LOW, perf: `Plants::frame_recorded` clones the tip vector every frame
- **Where:** `geometry.rs:814-819`: `self.recorded_tips.push_back(self.drawn_tips.clone());`.
- **Defect:** this runs on every frame, skipped frames included.
- **Bounded:** retire and discard pop it, and every `retire`/`discard` caller goes through
  `FrameSource`.
- **Scaling:** per frame, ∝ plants.
- **Verdict:** CONFIRMED.

### F10. LOW, correctness: map-error and timeout paths leave a frame permanently unretired
- **Where:**
  - `wgpu_backend.rs:617-621`: a map error pops the slot but never calls `done`, so
    `source.frame_retired()` is never called;
  - `drain` / `headless.rs:79-83`: on a `poll(Wait)` 30 s timeout, `?` returns before
    `frame_retired`.
- **Failure scenario:** each such event permanently takes one of `STAGING_RING = 2`
  renderer slots. After two, `can_stage` and `can_submit_source` are false forever. Ash
  waited `u64::MAX`.
- **Verdict:** CONFIRMED (it is reachable only after device-level errors, so low).

### F11. LOW, perf: `StagingRing` now refuses packs it no longer needs to
- **Where:** `voxel.rs:360-362` (`can_stage`) and `900-903`.
- **Defect:**
  - The CPU `Staging` Vecs are copied into wgpu staging at record time, so a slot is free
    once its frame is recorded.
  - The ring still holds it until retire, an ash-era constraint for mapped memory the GPU
    reads.
- **Failure scenario:** with 2 frames in flight, a tick's pack is deferred a tick for no
  reason.
- **Verdict:** CONFIRMED (harmless to correctness).

### F12. LOW, perf (per world): pipeline creation is CPU-heavier than ash
- **Where:**
  - `voxel.rs:748-822`, which builds up to 8 walk variants plus sky and weather;
  - `fullscreen()` at `:972-1028`;
  - `shader.rs:242-269`.
- **Defect:**
  - The runtime voxel WGSL is about 10.7k lines (`shaders/wgpu/voxel.frag.wgsl`).
  - wgpu parses and validates it once per device module.
  - For each pipeline it then processes overrides and emits SPIR-V on the CPU before the
    driver compiles.
  - Ash used precompiled SPIR-V with spec constants.
- **Scaling:** per world and per warm-up, not per frame.
- **Overlap:** the other reviewer covers `pipeline_cache`.
- **Verdict:** PLAUSIBLE. To check, time `VoxelRenderer::with_pipelines(All)` cold on ash
  and on wgpu.

### F13. LOW, maintainability
- **Three present shaders:**
  - the GLSL `present.frag`;
  - `shaders/wgpu/present.frag.wgsl`, reachable as `Shader::Present` but never used for a
    runtime pipeline;
  - the hand-written `shaders/present.wgsl`, which `PresentPass::new` actually uses
    (`wgpu_backend.rs:344`).
  - The other reviewer covers the source-of-truth question.
- **`SCRATCH_SIDE` is defined twice:** `render.rs:4` (pub) and `flat.rs:17` (private), both
  256.
- **Dead remnants of the per-slot emitter region:** `voxel.rs:1116` `if cap > 0 {}` and
  `:1117` `block.view[2] = 0`. Also `bloom.rs:261` `let _ = step.source;`.
- **Runtime dependencies used only by examples or the conversion tool:**
  - `Cargo.toml:18-19` wgpu features `spirv` and `naga-ir`;
  - naga `glsl-in`, `msl-out` and `hlsl-out` are runtime dependencies used only by
    examples or the conversion tool. They cost build time and binary size.
- **Verdict:** CONFIRMED.

---

## Checked and found sound
- **No `unsafe`:**
  - `#![forbid(unsafe_code)]` is on `lib.rs` and on every `wgpu_backend/*` module;
  - `git grep unsafe` in `crates/cubarium-gpu/src` finds only a comment in
    `pipeline_cache.rs`.
- **Discard and rollback:**
  - `record()` error paths keep the ring, timing pending, plant tips, sunvis upload state
    and owed bricks consistent;
  - nothing fallible lies between `ring.record()` (`voxel.rs:1060`) and
    `frame_recorded` (`:1162`);
  - in flat, `timing.begin` and `recorded.push_back` are adjacent.
- **Upload ownership:**
  - encoder-scoped staging is dropped with the encoder;
  - the core passes have no `queue.write_*`;
  - a discarded frame restores `dirty`, the slow planes and the owed bricks as ash did.
- **`ReadbackRing`:**
  - `map_async` is registered after `submit`;
  - the FIFO `flight` deque keeps tags in submission order;
  - a slot is reused only after unmap;
  - `submit_source` calls `frame_discarded` if the submit fails after a successful record.
- **Bind-group hazards:** each pass's world group substitutes dummies for the textures it
  attaches:
  - sky: 11, 17, 18;
  - sun: 13;
  - walk: 12, 15;
  - plant: 12, 15, 17;
  - figures: 12, 16, 18.
  - No texture is both sampled and attached in a pass.
- **Walk-variant fallback:** `walk[index] or walk[index & 2]` is equivalent to ash's
  depth/plain fallback for `Pipelines::Still`.
- **Timestamps:**
  - all 12 voxel slots (0,1,8,4,6,9,7,5,10,11,2,3, or 4..12 when skipped) and all 4 flat
    slots are written on every path, so no unwritten query is resolved;
  - `Timing` pending stays one-to-one with the frame ring.
- **Pass structure:**
  - it matches ash (sky, sun, walk, plant vis/shade, figure tiles/composite, weather,
    bloom, present);
  - one encoder and one submit per frame; the ash window used two.
- **Bloom:** bind groups are prebuilt and there are no per-frame allocations, only
  immediates. Passes per frame: 3 for blocky, 2·levels+2 for smooth.
- **Sun visibility:** the Owed/Recorded/Idle state machine retires and discards per slot as
  ash did. Its upload goes through F1's path, but only once per bake.
- **Window surface:**
  - Outdated/Lost get one repair and a retry;
  - Timeout/Occluded skip;
  - the acquired lease blocks reconfiguring;
  - the old surface is dropped before the new one is configured.
- **Headless:** `Headless::draw` blocking on `poll(Wait)` is the same as ash's fence wait.
- **Device limits:** the 21-sampled-texture limit is checked with an `Err`, and the
  immediate sizes stay at or under 64.
- **Teardown:** `texture.destroy()` on textures still in flight is deferred by wgpu-core,
  and the `Gpu` field drop order is irrelevant because the handles are refcounted.
- **CPU modules** (`weather, cord, voxel CPU, figures pack, plants, sunvis, bloom`):
  - the diffs are rustfmt changes plus removed vk code;
  - I spot-checked `uniforms`, `emit_table`, `write_upload`, `brick_dims` and `BrickOwed`,
    and their logic is unchanged.

## Benchmark axes suggested by these findings
1. **Skip frame against redraw frame:** CPU `record` time and object count (F2).
2. **Upload size:** 0, 100, 1000 bricks and a full upload per tick (F1). Vary world depth.
3. **Plant cells and model cells:** vary both, in the lit tier (F3).
4. **Target:**
   - headless with readback;
   - panel at 1920×1080 (F7);
   - window, FIFO against MAILBOX (F5).
5. **For each, measure:**
   - CPU record and submit ms;
   - GPU ms from the timestamps;
   - allocations per frame (`generate_allocator_report`, F6);
   - RSS churn.
6. **Per world:** cold pipeline build time (F12).
