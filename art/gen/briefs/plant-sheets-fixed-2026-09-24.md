# Brief: fixed plant sheets (2026-09-24)

Worker: sprite-forge. Approved by Wrysk: "have qwen do the fixed plant sheets".

**Subjects:** only the plants signed off or rated 4/5 in
`/home/wrysk/wryskware/cubarium/art/gen/SIGNOFF.md`: **BC** bloomcrown, **LB**
lanternberry, **ST** springturf, **SC** stonecushion, **SR** siphonreed. UF, VP, VT and
GC get design rework first; skip them.

**Inputs:** the 2026-09-23 character-sheet run,
`/home/wrysk/wryskware/cubarium/art/gen/runs/2026-09-23-character-sheets/`: its `LOG.md`
(the failures, and the hero seed per plant: BC 2312, ST 2312, SC 2311, LB 2311,
SR 2311), its `gen.py`, and `scripts/blender/blockout_strip.py` (plant mode). The
reference workflow is `art/gen/workflows/qwen21-ref-sheet.json`, run through
`art/gen/tools/comfy_run.py` with `--ref` and `--batch`. Designs are in
`design/art-direction/species-dossiers-2026-09-21.md` (D6, D8, D9, D12, D13) and the kit
in `art/gen/PROMPT-KIT.md` v0.4 (never pixel art, positive phrasing).

## The four fixes

1. **Crop each hero to the subject alone** on a plain flat background. Save it as
   `refs/<code>-hero-crop.png` so no habitat leaks into the sheets.
2. **One view per image, not multi-panel sheets.** Per plant: adult side, front,
   three-quarter and top views, plus growth stages seedling, juvenile and adult. Each
   is its own image on a plain flat dark background, at the same scale.
3. **Counts come from the blockout only.** State the dossier counts outright
   ("exactly six vanes, a gap between each"), and pass the blockout view that matches
   each image as the second reference.
4. **Stonecushion without the blockout's rock block.** Draw its rock lip as the
   dossier's thin lip, or with no rock.

Use **latent batches**: batch_size 2, one seed per prompt, each image logged with its
seed and batch_index. Local Qwen-Image 2.1 only.

## Output

- `/home/wrysk/wryskware/cubarium/art/gen/runs/2026-09-24-plant-sheets-fixed/`: images
  with `.run.json`, `refs/`, `LOG.md`, `sheet-<code>.png` per plant (views in one row,
  growth row beneath), and an overview `contact-sheet.png`. Rerun
  `python3 art/gen/tools/build_index.py`.
- Don't commit. Verdicts stay `unjudged`; you never decide the look.
- Return ≤ 20 lines: per plant, whether the counts held and what fails, plus the
  **absolute paths** of the contact sheets.
