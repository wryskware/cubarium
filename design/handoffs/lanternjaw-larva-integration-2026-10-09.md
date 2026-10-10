---
status: open
date: 2026-10-09
owner: opus-medium worker, worktree `.claude/worktrees/lj-larva`, branch `lj-larva`
---

# Lanternjaw larva: draw riders as the selected leechfork

Rules: `design/handoffs/worker-rules-2026-10-07.md`. No `unsafe` anywhere outside the
approved `ash` calls in `cubarium-gpu` (this package needs none).

Wrysk picked the larva on 2026-10-09: "lets go with leechfork"
(`design/organism-large-animals-2026-09-23.md`, D16, the larva bullet). Builder:
`lanternjaw_larva("leechfork")` in `scripts/blender/animal_models.py` on this branch. Today a
rider (`Animal::attached`, the lanternjaw's rider stage) draws as an interim glyph:
the rider branch at the top of `specs` in `crates/cubarium/src/voxel/figure.rs`
(`glyph_spec`, `rider_lift`).

## Build

1. **Bake** the leechfork as a model named `lanternjaw-larva`: one bin at its built length,
   facing +x, at both voxel sizes (`assets/voxel-models/125`, `/250`), in the manifest, like
   the specimens (`bake_specimen`). Rebake only that file and the manifests; check every
   other `.cvm` comes out byte-identical. At 0.125 m it must be the 8 cells of the sheet
   (`art/gen/runs/lanternjaw-larva-2026-10-09/r4/sheet.png` in the main checkout).
2. **Draw**: a rider whose founder is the lanternjaw draws the baked larva, when the library
   has it, in the lanternjaw's coat (`animal::model_colours`, as the founder models are
   painted). Its belly sits on the host's back (the existing `rider_lift`), it faces the host's
   heading, and its middle row is on the host's centre line. Every other case keeps the
   glyph fallback. One size from hatching to drop-off. The larva's 1-voxel body is on the
   model's centre row, so no half-voxel shift is needed.
3. **Drop-off**: when the rider lets go, it becomes a free juvenile and draws as the adult
   model's bin. That change must cross-fade (the figure track's model-swap fade), not pop.
   Check it does; fix it if the rider and the free body don't share a figure key.
4. Check whether anything else lists the library's models and needs the new entry, e.g.
   `crates/cubarium-godot/src/models.rs` and `voxel_specimens`.

## Evidence

A headless capture of a larva riding an adult frondgrazer that walks, stops and turns, and
of a drop-off, at px 6 and px 12. Place it with the world-edit API (`BodyStage::Rider`,
commit 7179d0d1) or a fixture, whichever is less code. One real-time MP4 per scale
(`-c:v libx264 -preset slow -crf 12 -pix_fmt yuv444p`), plus one still. A short test: a
rider with the library loaded gets the larva's cells, and the glyph without it.

Commit with explicit paths on `lj-larva` (it already holds the candidate builders). Don't
merge or deploy. Return ≤ 40 lines: commits, absolute MP4 and still paths, cells per voxel
size, anything that looked wrong.
