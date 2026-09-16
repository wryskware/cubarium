---
design_status: exploration
last_reviewed: 2026-09-16
---

# W1b (Opus, high): raster mode, rotation, and the panel at its real mode

Continuation of W1 for the same worker, same worktree
(`/home/wrysk/vuzic/led-cube-shim/.claude/worktrees/tachyon-screen`, branch
`tachyon-screen`, now at `b822af2` after W3's four test commits). Read W3's
six test files under `crates/cube-screen-shim/tests/` before changing
anything they pin.

## What changed (Wrysk, 2026-09-16, later in the day)

- The panel will show **one flat 2D world, not cube faces**. Cubarium is
  getting a rectangular world with solid edges (separate package). The shim
  therefore needs a **raster source**: a `W×H` RGB8 image that arrives over
  UDP, integer-upscaled, centred, letterboxed black.
- The panel's **native mode is 1080×1920 portrait and must not be changed**.
  The shim rotates the logical landscape image by 90° (configurable).
- The existing `net` and `cube` layouts stay in the code (they are the
  cube-frame test path and are pinned by W3), but the Tachyon's shipped
  config uses raster mode. Nothing cube-shaped is shown on the panel.
- The panel is now visible to the kernel: `DP-1` connected, EDID present,
  one mode `1080x1920@60.37` preferred, physical size 70×120 mm.
  `cube-screen-shim.service` is currently active again and holds DRM
  master (someone restarted it during the display bring-up). The debugfs
  `force` still reads `on`; leave it alone.

## Deliverable

### 1. `cube-proto`: a raster strip format (additive, version stays 1)

`Format::RasterStrip = 2`. Header unchanged (16 bytes; `face` = `NO_FACE`,
`flags` = 0). Payload = an 8-byte strip header, little-endian:
`width u16, height u16, y0 u16, rows u16`, followed by `rows·width·3` RGB8
bytes, row-major, `x` right, `y` down. Validity: `1 ≤ width, height ≤ 4096`,
`rows ≥ 1`, `y0 + rows ≤ height`, payload length exactly `8 + rows·width·3`,
datagram ≤ 65,507 bytes. Every strip of one frame carries the same `seq`.

API (shape is yours, names are not): `Raster { width, height, data }` with
`black(w, h)`, `get`, `set`, `as_bytes`; `encode_raster(raster, seq,
max_datagram, out: &mut Vec<Vec<u8>>)` cutting the largest row count that
fits; `decode` extended so formats 0 and 1 stay **byte-identical** (the
Python interop and `wire_format` tests must not change) and format 2 yields
the strip geometry; `CubeClient::send_raster(&Raster)`. Update
`docs/ARCHITECTURE.md`'s wire table. The Python client does not need format
2 yet.

### 2. Shim: raster ingest and `mode = "raster"`

- Ingest keeps a `Raster` beside the cube `Frame`. Per-strip staleness: a
  strip whose `seq` is older than the last seen for that `y0` is stale and
  counted; newer strips overwrite in place (latest wins, partial frames are
  shown as they are). A strip with a different `width/height` than the held
  raster reallocates it (black) and marks the layout for rebuild. Any
  accepted strip sets `dirty` and the arrival time. Malformed strips are
  counted as today.
- `[layout] mode = "raster"`, plus `rotation = 0 | 90 | 180 | 270` (default
  0; applies to every mode). The logical canvas is the native mode rotated:
  `(Lw, Lh) = (W, H)` for 0/180 and `(H, W)` for 90/270. For rotation 90
  the logical top edge lands on the panel's right edge:
  `px = W − 1 − ly, py = lx`; for 270: `px = ly, py = H − 1 − lx`; for 180
  both flipped. Scale `s = max(1, min(⌊Lw/rw⌋, ⌊Lh/rh⌋))` (or pinned),
  origin centred with the same floored arithmetic as the net. The LUT maps
  panel pixel → raster byte offset or background.
- `[raster] width = 320, height = 180` are hints only: what `layout` prints
  and what the idle/black canvas sizes before any frame arrives. The live
  size follows the stream. `screen = "logo"` is refused with a clear message
  in raster mode (the logo is cube-shaped); black is the idle screen.
- `test-pattern orient` renders, at the raster hint size, an up-arrow, a
  red dot top-left, a green dot top-right and the letters `TL` in the
  top-left cell, using `cube-pattern`'s glyphs, so rotation can be judged
  on the panel. `grid` and `solid:` work in raster mode too.
- Shipped Tachyon config `config/cube-screen-shim.toml`: `mode = "raster"`,
  `rotation = 90`, `[raster] 320×180`, bind loopback. Docs say to switch to
  270 if the orient pattern is upside down. The `layout` subcommand prints
  the rotation and the logical canvas.

### 3. On the device, at the real mode

1. `outputs` shows `DP-1 1080x1920`; `layout` at that mode in raster mode
   gives scale 6, viewport 1920×1080 exactly, origin (0, 0) on the logical
   canvas. Check against the formula; report if not.
2. `test-pattern orient` on the panel, then a note for Wrysk: which
   rotation was configured and what they should see (arrow up, red
   top-left, green top-right when the panel is held in the orientation the
   shelf will use). You cannot see the panel; say so and ask for a look.
3. Render time per frame at 1080×1920 into the real buffer, raster mode at
   scale 6, and the loopback stress: 1,800 raster frames at 60 fps from a
   small Rust example sender (add `examples/raster-sender.rs`), report
   received/stale/malformed, presented fps, ms/frame, CPU. Budget 8 ms;
   if over, drop the per-frame `vec!` in `render_with` you noted and
   re-measure.
4. Auto-attach: stop the service, start it, confirm it opens the panel;
   then `systemctl enable --now` and leave it running at the end.

## Constraints

- W3's tests keep passing unchanged unless a test pins something this
  brief changes (there should be none; the net/cube paths are untouched).
  Add your own tests for the strip codec (round trip, every validity rule,
  strip cutting at the datagram limit) and the rotation arithmetic (all four
  rotations map the four logical corners to the right panel corners).
- Never write to `/sys/class/drm/card0-DP-1/status` or to debugfs `force`.
- Fix the 1-ULP FOV inconsistency W3 found in `config.rs` (compute the
  radians the same way `Camera::default` does).
- Commit small on `tachyon-screen`, message bodies explain why, trailer
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. No merge, no
  push, no nested agents.

## Return format

Write the full W1 report now, `docs/reports/screen-shim-w1-2026-09-16.md`,
covering the original package and this continuation: files, decisions with
reasons, the measurement table at the real mode, what was verified on the
panel versus not, exact rebuild/redeploy commands, and the open question
of rotation 90 versus 270 for Wrysk. Same text as your final message.
