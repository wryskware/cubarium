---
name: generate-cubarium-assets
description: Generate, inspect, compare and iterate Cubarium Stage 2 sprite and plate candidates through the local ComfyUI MCP (Qwen-Image 2.1 text-to-image and reference-edit) and the GPT-image lane via Codex, judged in-scene against the art direction. Use for organism sprites, growth states, body angles, habitat plates, material tiles, part separation for rigging, and prompt iteration.
---

# Generate Cubarium assets

Produce logged, review-ready candidates. Wrysk selects; the artbook is the golden reference.

## Read first

1. `art/gen/PROMPT-KIT.md`: style block (use the current version), subject cards, dialects,
   run-log schema. `design/art-direction/Cubarium_Art_Direction_v0.1.md` when a card is
   silent. `art/gen/artbook/README.md` once it exists: accepted images are the image
   reference; the eight `design/art-direction/reference/` images are used only by role.
2. The card's `scale` line. One voxel = 0.125 m. Judge at 8 and 4 px per voxel.

## Lanes

**ComfyUI (breadth and edits).** Discover system status, models and saved workflows
through the MCP before generating. The only model is **Qwen-Image 2.1**, in both modes:
text-to-image for new designs, reference-to-image / edit (Qwen-Image-Edit 2509 with the
4-step Lightning LoRA where it is wired) for the next pose, growth state, body angle, or
part of an accepted design. Do not build or run workflows for Flux, Z-Image, HiDream or
any other installed checkpoint (Wrysk, 2026-09-21: not worth using). Save validated
API-format workflows under
`art/gen/workflows/` (one per asset family) and reuse them; do not rebuild graphs per run.
Qwen 2.1 specifics (kit §5): sprites use the RGBA wrapper sentence and are saved as PNG;
size is set on the ResolutionSelector node (1 MP default, 2048² native 2K, multiples of
32); cfg 1 with no negative on the official path; euler 40–50 steps.
Record checkpoint, workflow file, seed, steps, sampler, cfg, size and full prompt.

**GPT-image (quality and consistency).** `art/gen/tools/gpt_image.sh <run-dir> <n>
<prompt-file> [--edit ref.png ...]` runs Codex (`gpt-5.6-terra`, reasoning high) with its
image_generation tool and saves `NNNN.png` into the run dir. Use prose prompts; it follows
layout and count instructions (grids of poses, transparent background). One call per
subject per round; it is Wrysk's quota.

**Manual drops.** Wrysk's Midjourney / GPT / Gemini / Firefly keepers arrive in
`art/gen/inbox/<subject>/`. Ingest them into the round: copy into the run dir, write a
`run.json` with `"by": "wrysk"` and whatever provenance the filename or a sidecar gives,
and put them on the same contact sheet. Reference-photo **full sprite overrides** from
Wrysk are accepted as-is: they bypass generation and go straight to sprite prep.

## Procedure per subject card

1. Compose the prompt: style block + card + dialect. Both arms unless the card says one:
   **A** pixel-look direct at 1024² then `pixel_snap`; **B** clean graphic then downsample.
   Wrysk's expectation (2026-09-21): pixel-look generations look good but their scale is
   inconsistent between assets, so A is for exploration and B or a snap-plus-normalise
   step is likely the final-asset path. Always normalise a candidate's logical scale to the
   card's voxel size before it goes on the sheet.
2. Baseline: one image, recorded seed. Retrieve the finished file (`list_assets` /
   `view_image`), do not stop at the enqueue reply.
3. Inspect full size and at sprite scale. State the largest visible failure. Change one
   thing. Hold the seed for prompt changes; change the seed for composition.
4. Breadth: ≤ 8 candidates per lane per round. Heavy models sequentially on the 5090.
5. In-scene check: `art/gen/tools/compose_in_scene.py` pastes each candidate onto a
   current voxel capture at 8 and 4 px per voxel. Every sheet shows both.
6. Contact sheet: labelled grid, candidate id, lane, arm, model, seed. Save as
   `<run-dir>/sheet.png`. Write `LOG.md` for the round.
7. Return: counts, sheet path, failures, and proposed kit revisions. Stop; Wrysk judges.

## Body angles (the world is 3D)

Animals move along the strip and in depth under a 30° tilt. Each accepted animal design
gets angle bases from the accepted side view via the edit lane: side (mirrored for the
other direction), three-quarter toward camera, three-quarter away. Ask for "the same
creature, same palette and pixel scale, turned to face …". Plants get sway and growth
states, not angles. Which angles the renderer actually needs is a later decision; produce
the three and log them.

## Sprite prep and rigging hand-off

`art/gen/tools/`: `pixel_snap.py` (detect grid, nearest-downsample), `palette_quantise.py`
(Cubarium family, §05 of the art direction), background → alpha, part separation for
cutout rigs (edit lane: "only the head / front leg / fin, same palette, black background").
Rigging happens in the Godot atelier (`art/README.md`): cutout parts under named pivots,
clip names rest / move / feed / bud for animals and stage1 / stage2 / fruit / grow01 /
grow12 for plants, baked by `bake.gd`. Tile size for the panel is undecided; do not bake
to 16 px without asking.

## Boundaries

- Loopback only; ask before custom nodes, ComfyUI restarts, tunnels or paid APIs. LoRA
  downloads for Qwen are pre-authorised; record what was fetched and where.
- Never promote to `artbook/`, never edit the style block, never call a candidate final.
- Keep rejects and their logs until the round closes.
