---
status: resolved
date: 2026-09-17
owner: Wrysk (art direction), with a Fable thread
---

> **RESOLVED 2026-09-17.** Wrysk delivered the art direction:
> `design/art-direction/Cubarium_Art_Direction_v0.1.md` (with the illustrated
> `.docx` and the reference images beside it). That document is the source of
> truth this handoff asked for; the rest of this file is kept as the record of
> what was open.

# Art direction for the voxel world: handoff for Wrysk's own thread

Wrysk (2026-09-17): the art direction of the voxel world is his, worked in his
own thread with Fable. It is **not** something an agent decides and hands
back. The consumer-look study committed at 68a8215
(`design/7_Research/voxel-consumer-art-2026-09-17.md`,
`art/studies/voxel-consumers/`) was produced that way and is **paused and not
approved**; treat it as one disposable exploration, nothing in it is canon.

This file is the prompt for that thread. Paste the "Prompt" section into a
fresh Fable session in this repo, or open the session and point it here.

## What the thread must produce

1. **`design/voxel-art-direction.md`**, the single source of truth for how the
   voxel world looks: what the piece is for, the palette and its roles, the
   pixel treatment, how terrain, water, plants, fungi and animals read, how
   motion reads, what is forbidden. Frontmatter `design_status: decided` once
   Wrysk says so. Every later brief that touches the presenter, the GPU
   renderer, palettes, glyphs, bodies or motion **must cite it** and may not
   invent look. `design/appearance.md` (the cube's palette and pixel rules)
   stays as the family reference; the voxel doc says where the voxel world
   departs from it.
2. **A study with captures** under `art/studies/voxel-art-direction/` that
   Wrysk iterated on and approved, so the decided look exists as pictures and
   not only prose.
3. **A disposition of the current renderer's look**: which of the presenter's
   autotile rules (rim, bevel, chamfer, riser, contour, roof shadow, haze,
   water skin, pore darkening) and which of the plant/fungus/animal treatments
   stay, change or go. The CPU presenter and the GPU shader draw the same
   picture; a change is specified once and implemented in both.
4. **Rules for agents**, written into `WORKING_POLICY.md` or the art doc: the
   artwork pass is Wrysk's thread; workers implement the art doc and do not
   make taste calls; a package that needs a look the doc does not cover stops
   and asks, with an interim glyph named as interim.

## What exists (read these, in this order)

- `design/appearance.md` — the palette family (Outrun: indigo floor `#12093A`,
  producer ramp `#1E2798` → `#42C5F8`, detritus `#510B6D`, body hue ramp
  `#FF2AFC` → `#42C6FF`, one warm accent `#FF9B50`), pixel treatment, rhythm.
  Wrysk's room and wallpaper set it (2026-09-11).
- `design/game-art-workflow.md` and `art/README.md` — Wrysk chose
  creature/habitat art as his own contribution; the Godot atelier pipeline
  (source scenes authoritative, baked atlases in git, 16 px art, 32 px chosen
  on the panel as "definitely better") is how the flat world's art is authored.
- `design/landscape-plan-2026-09-16.md` — what the GPU synthetic scene had
  that the live flat world lacked: negative space (27 % near-black), large
  field forms, no producer wash, a real rain column, wind five times larger.
  Wrysk called the synthetic scene "THE direction" and the live ring "a weird
  confetti layer cake" (2026-09-16).
- `art/studies/voxel-storyboard/` — the storyboard that chose the voxel camera
  (tilt 30°, 4 px per voxel, habitat depth 24) and the landform rule (front
  low, peaks and plateaus at the far edge, terrain never occludes terrain).
- `crates/cubarium/src/voxel/present.rs` (module doc + palette constants) and
  `stand.rs` — the current voxel look: strata palette, autotiled block faces,
  translucent water with one skin, per-row haze, plants as trunk column plus
  crown disc, sprout glyph on seed banks. `project.rs` is the projection.
- `crates/cubarium-gpu/src/voxel/**` and `design/7_Research/voxel-gpu-render-2026-09-17.md`
  — the GPU renderer draws the same picture from one voxel texture per tick
  (within one 8-bit code of the CPU presenter). Captures under
  `design/7_Research/assets/voxel-render/`. The contour rule is dead at 4 px
  (rise 2) and switches on at 8 px, the panel candidate.
- `design/theoretical-biosphere-2026-09-16.md` §4 — the "readable form"
  column per organism (glowcap: clusters on identifiable logs, restrained
  luminous caps; frondgrazer: low broad body, deliberate cropping pauses).
- `design/handoffs/voxel-round5bc-consumers-briefs-2026-09-17.md` — the
  glowcap and frondgrazer packages, which will ship **interim** glyphs until
  the art doc says otherwise.
- `design/7_Research/voxel-consumer-art-2026-09-17.md` — the paused study,
  for what not to assume.

## Tools for looking

- `cargo run -p cubarium --release -- voxel --scene generated --sink png --seconds 4 --out DIR`
  writes frames; `--scene authored` is the hand-built fixture with a ridge, a
  hollow with water and an overhang.
- `... --sink gpu --gpu-target window --fps 60` opens the live GPU window
  (Hyprland floats it unfocused top-right); stdin `r 40` rains, `a -400`
  drains a charged aquifer, `f X Z bloomcrown` seeds, `m X Y Z rock` edits,
  `q` quits. `--gpu-target headless --gpu-capture DIR` for captures without a
  window. `cargo run -p cubarium --release --example voxel_fidelity -- --out DIR --px 8`
  renders CPU/GPU pairs at panel scale.
- `art/studies/voxel-storyboard/storyboard.py` (PIL + numpy) hand-authors a
  strip and projects it as the presenter does, for mocks in seconds.
- The panel path (`--gpu-target shim` on the Tachyon) is wired but not yet run
  for the voxel world (package VR-2).

## Open questions the art doc should settle

- Is the autotiled-block look the piece, or a placeholder for authored
  textures/sprites through the atelier pipeline? Both can feed the GPU voxel
  texture (a material/style id per voxel), so this is a look decision, not a
  renderer one.
- Panel scale: 4 px per voxel (512-wide strip, ×3 on the panel with bars) or
  8 (world width and raster height are fresh-world config).
- Negative space and field forms on the voxel terrain: where is the dark?
- Water: skin, body, pore darkening; rain as a visible column?
- Plants: crown disc plus trunk, or authored silhouettes per species; wind.
- Fungi: glow or no glow; is any green allowed.
- Animals: body size in voxels, silhouette, motion between faces (pace in body
  lengths, ~1 BL/s cruise per the flat-world calibration), feeding read.
- Day/night and haze: constant presentation lighting or a cycle.

## Prompt (paste into a fresh Fable session)

> This is an art-direction session for the voxel world of cubarium, run with
> Wrysk. Read `design/handoffs/voxel-art-direction-handoff-2026-09-17.md`
> first, then the files it lists in order. Do not decide the look yourself:
> propose, render, show captures, ask, iterate. Produce
> `design/voxel-art-direction.md` as the single source of truth, a study with
> approved captures under `art/studies/voxel-art-direction/`, a disposition
> of the current renderer's rules, and the agent rules that make every later
> brief cite the art doc. The study at 68a8215 is paused and not canon. Keep
> windows floating (the Hyprland rule handles titles starting with
> "cubarium"); prefer PNG captures. Explicit-path commits, no cargo fmt, do
> not touch `design/handoffs/README.md`, and do not edit
> `crates/cubarium-voxel-flora/**` while package M/N/O workers are active
> (check `git log` for their commits and ask Wrysk if unsure).
