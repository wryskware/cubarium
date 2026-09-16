---
design_status: exploration
last_reviewed: 2026-09-16
---

# GS-2 (Opus, high): the daemon takes dma-buf frames over a Unix socket

Read `design/7_Research/gpu-scanout-spike-2026-09-16.md` (the working
scanout path and the traps) and the shim's `docs/SCREEN.md`. Fresh context.
No nested agents.

## Objective

`cube-screen-shim` gains a second frame source beside UDP: a client process
on the same board renders on the GPU into linear dma-buf images and hands
their file descriptors to the daemon over a Unix socket; the daemon imports
each once as a DRM framebuffer and page-flips it. Zero copies. UDP raster
stays as the fallback and the idle/bring-up logic is untouched.

## Where

- led-cube-shim worktree `/home/wrysk/vuzic/led-cube-shim/.claude/worktrees/tachyon-screen`
  (branch `tachyon-screen`). Never touch the main checkout.
- Device `root@tachyon-8968c731.local`: same grant as W1 (build natively,
  install, restart the service, read logs; stop/start the service around
  scanout tests; never write connector status or debugfs). `core_ctl`
  isolates idle big cores: use `taskset -c 4-7`.
- Reference code: the spike at
  `/home/wrysk/wryskware/cubarium/.claude/worktrees/tachyon-screen/spikes/gpu-scanout/`
  (`vk_scanout`, variant (c)); read-only, you may copy from it with
  attribution.

## Deliverable

1. Protocol: a `SOCK_SEQPACKET` Unix socket at
   `/run/cube-screen-shim/frames.sock` (config `[handoff] socket = ...`).
   Message `Attach { width, height, fourcc, pitch, offset }` with one fd via
   `SCM_RIGHTS` → reply `Attached { slot }` (or an error); `Present { slot,
   seq }` → the daemon page-flips that framebuffer at the next vsync and
   replies `Presented { slot }` after flip-complete so the client knows the
   buffer is on screen and which one is free; `Detach { slot }`. Latest
   `Present` wins; at most 4 slots per client, one client at a time.
   Document it in `docs/SCREEN.md` with the exact byte layout.
2. Import via `drm`'s `prime_fd_to_buffer` + modifier-free
   `add_planar_framebuffer` with the client's pitch; XRGB8888/ARGB8888 only;
   reject anything else with a clear error. Rotation is the client's job on
   this path (the image is already panel-sized 1080×1920); say so.
3. Idle: no `Present` for `after_secs` → the daemon fades to black using
   its own buffers and stays there until the next `Present`. On client
   disconnect, release its framebuffers. UDP frames arriving while a
   handoff client is attached are counted and ignored (log once).
4. A Rust example `examples/dmabuf-sender.rs` that uses `ash` to render an
   animated gradient into 3 linear images, attaches them and presents at 60
   fps for 30 s (adapted from the spike), for the device test.
5. On the device: the service restarted with the new build; the sender
   running for 30 s: presented fps, flip pacing, CPU of daemon and sender
   (`top`), and the daemon's log. Confirm the panel returns to idle black
   after the sender exits and that a UDP `raster-sender` still works after.

## Constraints

- W3's and W1b's tests keep passing unchanged; add your own for the
  message codec and slot bookkeeping (no GPU needed).
- `ash` is allowed as a dev-dependency for the example only; the daemon
  itself needs no Vulkan.
- Commit small on `tachyon-screen`, trailer
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. No merge, no
  push.

## Return

Report at `docs/reports/screen-shim-gs2-2026-09-16.md` and as your final
message: the protocol as implemented, the device measurements, what was
verified on hardware, and anything left.
