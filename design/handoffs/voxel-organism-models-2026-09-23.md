# Package V: voxel organism models baked from Blender

Wrysk, 2026-09-23: "for the voxel world rendering, we should totally use your voxelated
renderings from Blender as the new models." Today the presenter draws every plant as a
trunk column under one crown disc (`crates/cubarium/src/voxel/stand.rs` `parts_of`
L475) and every animal as a shell block (`animal.rs` `cells_of` L194). This package
replaces both with **voxel models baked by the Blender scripts from the simulation's own
numbers**. That is the voxel presentation (art direction §03 stage 1, "voxels describe
occupancy"); the stage-2 sprite pass is separate and unaffected.

After package L (sizes in metres, growth fix); the three new plants follow package N.
Lands on main. Recommended worker: Opus, **high** effort (new rendering construction).

Read first: `scripts/blender/organism_lineup.py` (the plant and animal builders and the
0.125 m voxelizer; renders in `runs/scale-lineup-2026-09-22/panel-ladder-vox*.png`),
`design/organism-anatomy-2026-09-21.md` §3, `design/art-direction/species-dossiers-2026-09-21.md`,
`design/art-direction/Cubarium_Art_Direction_v0.1.md` §03–§05,
`design/art-direction/organism-scale-and-roster-2026-09-21.md` §4 and §7.

## 1. One source of numbers

The Blender builders today carry their own copies of crown ranges, layer profiles and
body sizes. Replace the copies with a JSON dump from the model: a small example
(e.g. `cargo run -p cubarium-voxel-flora --example organism_numbers`) writing
`assets/voxel-models/source.json` with each species' crown ranges in metres, stage
profiles (layers: kind, band, radius, share, porosity, seedling cap) and each founder's
adult dimensions. The Blender scripts read that file. If the model changes, re-running
the dump and the bake reproduces the models; nothing is hand-edited.

## 2. The bake

`scripts/blender/bake_voxel_models.py` (headless, like the lineup):

- **Plants:** for each species, one model per **crown-height step of one voxel**, from
  the seedling to the adult maximum, following L's growth rule. There are 2–3 variants
  per step (a different random phase of vanes, fronds and lobes) so a meadow is not
  copies. Voxelize at each preset's voxel size (0.125 m and 0.25 m) with the existing rule:
  a cell's centre inside a part or within half a voxel of its surface.
- **Every cell carries a material and a tag.** The tag is trunk, foliage layer *i*,
  drape, or accent (bloom, fruit, lure, sense patch). With the tag, the presenter
  shows the simulation state on the model:
  - **Foliage thins by layer.** Drop a layer's cells deterministically (hash of stand
    and cell) in proportion to `1 − stock / capacity` of that layer, so a browsed
    rosette visibly empties while the crown stays full.
  - **Wilt** tints foliage cells, as today.
  - **Accents** follow their state: a warm bloom only when the parcel says so; lanternberry
    fruit by parcel fullness.
- **Animals:** each founder in 5 size bins (newborn to adult, length ∝ body^(1/3)), facing
  +x, one pose for now; the presenter rotates in 90° steps to the nearest heading. The
  frondgrazer and littershredder follow their dossier body plans (D3, D4), which Wrysk
  considers fairly settled. Starving is a tint.
- **Output:** `assets/voxel-models/<voxel_mm>/<species>.<ext>`. Each model holds its
  steps (height and radius in metres, cells as offsets from the anchor, anchor = the
  stand's site on its support face) plus a manifest. Compact binary or JSON, your choice;
  say why.

## 3. The presenter

- `Stands::rebuild` stamps the model for the stand's species, nearest height step and
  variant (from the stand id). It clips to the world, skips solid cells, wraps in x, and
  falls back to today's `parts_of` if a model is missing. Keep the old glyphs as a dev
  option (art direction: dev mode stays available).
- `Animals::rebuild` stamps the founder's model by size bin and heading.
- Materials map to the palette (plum #2A0E4A, violet #3A1A7A, detritus #510B6D, lilac
  #B99BE6, producer ramp #1E2798 / #2B6AD0 / #42C5F8, teal #248CA8, magenta #FF2AFC, warm
  #FF9B50 for bloom and fruit only) through the existing voxel lighting.
- **Raster to 6 px per voxel**, 960 × 540, ×2 nearest (roster §7 decision 1; today 4 px ×3
  in `config/tachyon/voxel.toml`). 160 × 6 = 960; 72 × 6 + 24 × 3 = 504 ≤ 540. Measure
  the pack and presenter cost on the desktop. Don't deploy to the board; that is Fable's
  or Wrysk's call after looking.

## 4. Tests (authored first, separate pass)

Anchor and offset placement; x wrap and back-wall clip; buried cells skipped; thinning
deterministic and proportional per layer (a layer at 50 % stock shows about half its
cells, and the same half every frame); nearest-step selection; the fallback when a model
is missing; the source dump round-trips the model's numbers exactly.

## Return (≤ 40 lines)

Commits, tests, file sizes of the model library, presenter and pack cost before/after,
screenshots at 1080p of the small ring (one `--background` run, 2 minutes in), and any
species whose model does not read at 6 px per voxel.
