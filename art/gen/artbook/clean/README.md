# Artbook picks without the painted outer glow (2026-10-09)

Wrysk: "for some of the accepted art work, there was a problem where they had outter glow generated on the image … should be done and the pixel art variants in the compendium regenerated." Brief: `design/handoffs/artbook-glow-cleanup-2026-10-09.md`.

The accepted picks in `../` stay untouched. These are the same subjects with only the outer glow, haze or backdrop removed. They were chosen on fidelity to the original alone. The field guide's plates (`scripts/make-field-guide-plates.sh`) use a file here wherever one exists.

| File | Method | Fidelity |
| --- | --- | --- |
| capgnawer-rose-bark.png | Alpha cleanup: keep alpha ≥ 230, a 5 px open, a 1 px feather. | Pixel-exact body. |
| frondgrazer.png | Qwen-Image 2.1, `qwen21-ref-sheet.json` with the original as image 1, 40 steps, cfg 1, seed 4101, batch_index 1. | Mean colour difference 5.0 on the body (best of 4). |
| littershredder.png | Same graph, seed 4101, batch_index 3. Alpha cleanup alone left an opaque brown fringe. | Mean colour difference 7.9 (best of 4). |

The other 15 plates had no outer glow. Glow that belongs to the subject stays: the lanternjaw's lures, the glowcap's brim and mycelium, and the vaulttree's tendrils.

`sheets/` holds the before-and-after pairs on the plate colour and on green, and every plate at field-guide size.
