---
status: open
date: 2026-10-09
---

# Remove the painted outer glow from artbook picks (2026-10-09)

**Rules.** AGENTS.md and WORKING_POLICY.md apply.
- No windows.
- No tool call over 5 minutes: wrap in `timeout 270`, never poll or wait on a job. A long batch goes detached and you hand back.
- Local ComfyUI weights only, never paid API nodes.
- Commit with explicit paths, trailer `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Don't merge.

## Objective

Wrysk: "for some of the accepted art work, there was a problem where they had outter glow generated on the image. idk if thats easy to remove but should be done and the pixel art variants in the compendium regenerated. ive started comfyui so you should be able to use qwen image 2.1 to edit them and remove the glow."

The artbook README (`art/gen/artbook/README.md`, "Remaining cleanup") already lists this: painted outer halos or backgrounds are image artifacts, not creature emission. It names the seedporter's outer glow as unwanted.

This is a **cleanup that keeps each image's look**, not a redesign. Each cleaned image must be the same subject with the same shape, colours, markings and pose. Only the outer glow, halo or painted backdrop around the silhouette goes. Glow that belongs to the creature or plant stays:
- the lanternjaw's cyan lures;
- the glowcap's cyan brim and the mycelium glowing in its log;
- the vaulttree's cyan tendrils;
- any other emissive part the artbook README describes.

## Scope

The 18 images the field guide uses (`scripts/make-field-guide-plates.sh` lists them).

1. **Find which have an outer glow.** Composite each over flat bright green and over white, and look. A measurement of semi-transparent pixels helps as a first pass. The parent's measurement, as the fraction of pixels with alpha between 0.03 and 0.9:
   - littershredder 0.43;
   - capgnawer 0.16;
   - vaulttree 0.08;
   - umbrellafrond 0.04;
   - lanternjaw 0.03.

   frondgrazer.png is fully opaque on a painted dark backdrop. Most others are under 0.02. Check them all by eye anyway, and list each one with its verdict.
2. **Clean each flagged image with Qwen-Image 2.1** on the local ComfyUI (`127.0.0.1:8188`), using `art/gen/workflows/` and `art/gen/tools/comfy_run.py` (see `art/gen/workflows/README.md`).
   - Use the reference-anchored 2.1 graph (`qwen21-ref-sheet.json`) with the original as image 1, and the README's transparent-sprite wrapper ("This is an RGBA format image with transparency. … The image has an alpha channel and a transparent background.").
   - Ask for the same subject with no outer glow or halo.
   - Generate several candidates per image in one latent batch, and log seed and batch index for each.
   - If 2.1 can't hold fidelity, say so and show the best you got. Don't hand-paint.
   - If a plain alpha cleanup does better than the model for an image (an alpha threshold, or flooding out a backdrop), show both.
3. **Pick the most faithful candidate per image.** Judge only fidelity to the original, never which looks better. Save it as `art/gen/artbook/clean/<original-filename>`. Don't overwrite or rename the accepted picks, and don't edit the artbook README's pick tables.
4. **Contact sheets.** Per image: the original and the chosen clean version, on the plate colour `#24115F` and on green. Also one sheet of every chosen image at the field guide's plate size: 176 px wide, box filter, 40 colours, nearest upscale ×3.

## Not in scope

- New art.
- Recolours or pose changes.
- Latticevine, which has no art.
- Changing the plate script or the game. The parent wires the clean images into the plates.

## Return (≤40 lines)

- Which images had an outer glow, and the evidence.
- Per image: the method that won (model or alpha cleanup), seed and batch index, and any fidelity problem you saw.
- Paths of the clean files and contact sheets.
- Commits.
