---
status: open
date: 2026-10-07
owner: opus-medium worker, worktree `.claude/worktrees/window-fill`, branch `window-fill` (from main)
---

# Fill the window, aspect kept; soften the eating flash

Wrysk, 2026-10-07: "for scaling to the display window, id prefer if it stretched
(maintaining aspect ratio though). right now my word is severely windowboxed. might need
some antialiasing, idk." And: "the eating flashes are too intense. the grazers look like
they are about to explode all the time lol."

Read `design/handoffs/worker-rules-2026-10-07.md` first.

## 1. Fill the window

- Find what `px_per_voxel = "auto"` ("the largest that fits the screen, the whole strip in
  view") and the presenter do today: the integer scale and the margins it leaves
  (`crates/cubarium/src/sink/gpu/voxel.rs`, `crates/cubarium-gpu`; use `graft ask`).
- New behaviour: the world image fills the window on its limiting axis, aspect kept, any
  (non-integer) scale; bars only on the other axis. Window resizes keep working; headless
  captures keep their explicit size.
- Pick the scaler that looks best for its cost and say why. Candidates:
  - render at the next integer px_per_voxel above the fit and downsample with a good filter
    (area or bicubic): antialiasing for free;
  - integer prescale then linear upscale ("sharp bilinear"), if rendering larger costs too
    much.
  The pixel-art illustration should stay crisp-looking, not blurry; effects may be smooth.
- Render ms/frame before and after at 1920×1200, 2560×1600 and 3840×2160 (on the 5080).
- Pictures: headless captures at 2560×1600, before and after, plus a 200 % crop of an
  animal and a plant edge for the antialiasing. If you try two scalers, show both.

## 2. The eating flash

- `ANIMAL_CROPPING_FLASH_SRGB` (`crates/cubarium/src/voxel/animal.rs:132`) and the meal-onset
  presentation (`crates/cubarium/src/meal_present.rs`).
- Make it subtle: much lower intensity and extent, eased in and out on the every-frame
  clock. Its strength is a presentation config knob.
- A short MP4 of a grazer eating, before and after side by side.

## Return

Commits, the scaler choice and its cost, picture and video paths.
