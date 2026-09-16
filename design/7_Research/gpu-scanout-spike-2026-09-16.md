# GPU scanout spike on the Particle Tachyon — 2026-09-16

Feasibility spike (GS-0): can a Rust program render on the Adreno 643 with no
compositor and get the result onto the DP-1 panel through `/dev/dri/card0`?

**Yes, with zero copies.** Vulkan through the Qualcomm blob renders into a
LINEAR-tiled image exported as a dma-buf; `drmPrimeFDToHandle` + a plain
`AddFB2` imports it; page flips run locked to the panel's 60.37 Hz at
**0.12 CPU core-seconds per second**. Recommended stack: **`ash` + `drm` 0.15**,
Qualcomm proprietary Vulkan, no Mesa, no wgpu.

Code: `spikes/gpu-scanout/` (`vk_probe`, `vk_scanout`, `egl_gles`,
`wgpu_headless`). Built and run on the device at `/root/gpu-scanout`.
All numbers are 1080x1920 (the panel's only mode) or 1920x1080 where stated,
`taskset -c 4-7`, release build, 60–300 frames with the first 5 discarded.

## Q1 — which GPU userspace renders headless

Both Qualcomm stacks work. Mesa does not reach the GPU at all.

| stack | how | result | render p50 | readback p50 |
|---|---|---|---|---|
| Vulkan, Qualcomm blob, raw `ash` | no surface, render to `VkImage`, `vkCmdCopyImageToBuffer`, map | **works** | 0.69 ms GPU; 5.65 ms submit..fence incl. image→buffer copy | 9.10 ms host memcpy |
| Vulkan, Qualcomm blob, `wgpu` 27 | `Instance::default()`, no surface | **works** | 3.93 ms submit..fence (render + copy) | 9.68 ms map + memcpy |
| GLES2, Qualcomm EGL, `khronos-egl` + `glow` | `EGL_KHR_platform_gbm` display on `renderD128`, surfaceless context, FBO | **works** | 1.51 ms clear + quad + `glFinish` | 3.78 ms `glReadPixels` |
| GLES, Mesa 21.2 `EGL_MESA_platform_surfaceless` | glvnd → `libEGL_mesa` | **llvmpipe (software)** | 15.06 ms | 1.33 ms |
| Mesa freedreno on the a6xx | any | **cannot load** | — | — |

All four rendering paths produce byte-identical output for the same scene
(corner `(13,5,31,255)`, centre `(152,126,126,255)` in RGBA), so the GLES and
Vulkan results are cross-checked against llvmpipe's software reference.

Facts worth keeping:

- **No Vulkan ICD manifest is needed.** There is no `/usr/share/vulkan/icd.d`,
  but Qualcomm's loader (`/usr/lib/libvulkan.so.1`, 1.2.162) finds
  `/usr/lib/libvulkan_adreno.so` by itself. `ash::Entry::load()` just works.
  Device reports `Adreno (TM) 643`, API 1.1.128, driver build `588d5245d2`
  (2023-06-21). One queue family (3 queues, graphics+compute), 7 memory types.
- **Mesa cannot drive this GPU.** The kernel calls itself `msm_drm`, so Mesa
  looks for `msm_drm_dri.so`; the package ships `msm_dri.so`. Symlinking is not
  enough — the module exports `__driDriverGetExtensions_msm`, not
  `..._msm_drm`, so the loader then reports `did not find extension DRI_Core
  version 1`. `MESA_LOADER_DRIVER_OVERRIDE=msm|kgsl` does not help either. Mesa
  always lands on llvmpipe. There is no turnip (`libvulkan_freedreno.so` absent).
- **`eglGetDisplay(EGL_DEFAULT_DISPLAY)` segfaults** inside
  `libEGL_adreno.so` (SIGSEGV, exit 139). The blob needs a real platform:
  `eglGetPlatformDisplayEXT(EGL_PLATFORM_GBM_KHR, gbm_create_device(renderD128))`
  with `/usr/lib/libgbm.so` (`qti-gbm`) works, then `EGL_KHR_surfaceless_context`
  makes a context current with no surface.
- **Do not `dlopen` `libGLESv2_adreno.so` yourself.** Two copies of that library
  exist on the loader path; the blob's own `libEGL` maps one, your `dlopen` maps
  the other. Calls then go to a copy with no current context and *silently
  no-op* — `glGetString` returns plausible strings (`Adreno (TM) 4XX`!), every
  draw and `glReadPixels` returns success and does nothing, in ~0.1 µs. Resolve
  every GL symbol through `eglGetProcAddress`
  (`EGL_KHR_get_all_proc_addresses` is present). This cost an hour; it is the
  single nastiest trap on this board.

## Q2 — zero-copy scanout: yes, but only the LINEAR route

Three variants tested against `/dev/dri/card0`, connector DP-1, 1080x1920@60.37.
`cube-screen-shim` was stopped for each and restarted after; it is active now.

| variant | AddFB2 | GPU render | submit..fence | flip pacing p50 | fps | CPU core-s/s |
|---|---|---|---|---|---|---|
| **(c) LINEAR image exported as dma-buf, plain AddFB2** | **accepted** | **1.43 ms** | **3.12 ms** | **13.49 ms** | **60.2** | **0.12** |
| (b) dumb buffer exported to Vulkan, GPU blits into it | accepted | 0.69 ms | 7.74 ms | 8.94 ms | 60.0 | 0.30 |
| (a) `VK_EXT_image_drm_format_modifier` + `AddFB2WithModifiers` | **EINVAL** | — | — | — | — | — |

Variant (a) fails because the two sides do not share a modifier vocabulary. The
Vulkan driver advertises exactly **one** modifier for `B8G8R8A8_UNORM`:
`0x0500000000000000` (QCOM vendor, i.e. UBWC), never `DRM_FORMAT_MOD_LINEAR`.
The 5.4.219 downstream KMS driver exposes **no `IN_FORMATS` blob** on its planes
at all, and rejects that modifier:

```
drmModeAddFB2WithModifiers(1080x1920 XR24 mod=0x0500000000000000 pitch=4352)
  -> Invalid argument (os error 22)
```

Passing the same UBWC dma-buf through a *plain* `AddFB2` (no modifier flag) is
accepted and flips at 60 Hz, but the display then reads UBWC bytes as linear, so
whatever is on the panel cannot be the rendered frame. That is a trap, not a path.

Variant (c) is the one to build on. `VK_IMAGE_TILING_LINEAR` +
`COLOR_ATTACHMENT` + `DMA_BUF` is fully supported
(`EXPORTABLE | IMPORTABLE | DEDICATED_ONLY`); `vkGetImageSubresourceLayout`
returns `rowPitch=4352, offset=0`, which is exactly what the dumb allocator
picks, and a modifier-free `AddFB2` with that pitch is accepted. Rendering goes
straight into the scanned-out memory: no readback, no blit, the CPU never
touches a pixel.

**Verified, not assumed.** After 120 flips the last scanned-out dma-buf was
pulled back through the GPU: BGRA corner `(87,5,106,255)` — the animated clear
colour for that frame — and centre `(127,127,153,255)`, byte-identical to the
headless reference, 8278944/8355840 bytes non-zero. Both (b) and (c) give the
same bytes. (No camera was available, so "visible on the panel" rests on
modeset + flip-complete + the scanout buffer's verified contents.)

Pacing: `submit..fence` 3.12 ms + flip wait 13.49 ms = 16.6 ms, one refresh
exactly; sustained 59.8–60.2 fps over 300 frames. The render and the flip wait
are serialised here — a real renderer would overlap them and have ~13 ms of
headroom per frame.

Cost of LINEAR: rendering into linear costs 1.43 ms against 0.69 ms into a tiled
image, i.e. ~0.75 ms of extra resolve. Variant (b) pays 0.69 ms plus a ~6.5 ms
GPU blit instead — the tiled render only pays for itself if the blit disappears,
and it cannot. Take the LINEAR target.

## Q3 — the readback fallback (measured anyway)

Render to a tiled image, `vkCmdCopyImageToBuffer`, memcpy into a KMS dumb
buffer, flip:

| stage | p50 |
|---|---|
| GPU render | 0.69 ms |
| submit..fence (render + image→buffer copy) | 7.72 ms |
| memcpy into the dumb buffer | 17.94 ms |
| flip queue..complete | 6.77 ms |

**29.9 fps, 0.68 CPU core-seconds per second.** The memcpy alone (8.3 MB into
write-combine KMS memory at ~460 MB/s) exceeds the whole frame budget. This
path cannot hit 60 fps at 1080x1920 and should be treated as a diagnostic, not
a fallback.

## Recommendation

Build the renderer on **raw `ash` (0.38) + `drm` (0.15)**, Qualcomm's Vulkan,
variant (c): a small ring of `VK_IMAGE_TILING_LINEAR` `B8G8R8A8_UNORM` images
with `VkExportMemoryAllocateInfo(DMA_BUF)`, each imported once at startup via
`prime_fd_to_buffer` + `add_planar_framebuffer(FbCmd2Flags::empty())` using the
pitch from `vkGetImageSubresourceLayout`, then legacy `page_flip` +
flip-complete on DP-1. `rustix` (already a `drm` dependency) for the poll.

Not wgpu: it works and would be pleasant, but it cannot export a dma-buf without
dropping to `wgpu-hal`'s `as_hal`, at which point `ash` directly is simpler and
its headless numbers are no worse. Not Mesa: it never reaches the GPU. Not
GLES: it renders fine and `glReadPixels` is cheaper than Vulkan's two-step
readback, but the dma-buf story (`EGL_EXT_image_dma_buf_import`, gbm scanout
buffers) is strictly more moving parts for the same result.

Do not enable `VK_EXT_image_drm_format_modifier` in the real renderer; it only
offers UBWC, which this kernel cannot scan out.

## Bonus (not tested)

Whether `cube-screen-shim` can import a dma-buf handed over a Unix socket with
`SCM_RIGHTS` was **not** tested. Nothing found here argues against it: the
exported fd is an ordinary dma-buf fd, and the importing side needs only DRM
master on `card0` (which the shim has) plus `(width, height, fourcc, pitch,
offset)` alongside the fd — no modifier, since the working path is
modifier-free. Worth one follow-up spike.
