# Art direction (decided by Wrysk, 2026-09-17)

`Cubarium_Art_Direction_v0.1.md` is the art direction of the project, written by
Wrysk. The `.docx` is the illustrated edition of the same text. **Every brief that
touches the presenter, the GPU renderer, palettes, glyphs, organism bodies or
motion cites this document and does not invent look.** Agents implement it; they
do not make taste calls. A package that needs a look the document does not cover
stops and asks, shipping an interim glyph named as interim.

What it settles, in one line each (read the document for the rest):

- Presentation: the existing tilted, side-on cutaway diorama with visible depth.
- Voxels describe occupancy; art describes appearance. Production graphics must
  not expose a cube-per-cell aesthetic ("organic, not blocky"). Raw cubes are
  **dev mode**, kept as a diagnostic option.
- Staged: 1 dev mode → 2 graphic pixel ecology (the production foundation) → 3
  painterly pixel diorama. One identity, increasing richness.
- Screen first at 1080p and above (the AMOLED is 1080p, not a low-res target);
  the LED cube is a later adaptation with its own assets.
- 2D pixel-art sprites are the leading approach for organisms; 3D or hybrid
  techniques stay available where they serve the same look.
- Palette family: deep indigo / blue-violet / dark purple masses supporting
  electric cyan, blue, violet, magenta, with selective warm accents.
- Near-term assets are AI-generated and selected by Wrysk.

Deliberately open (do not decide these in a worker brief): sprite resolution
and pixel scale, internal render resolution, 2D/3D balance, terrain rendering
technique, lighting/shaders, animation density, asset tools, final species and
biome lists, game UI, the cube treatment.

## Reference images (`reference/`)

Extracted from the illustrated edition, in the document's inventory order.

| Ref | File | Role (from the document) |
| --- | --- | --- |
| R1 | `R1-existing-pixel-ecology-mockup.png` | Existing pixel-art ecology mockup; the strongest reference for pixel treatment, clustered ecology and atmosphere. Its 2D simulation is not a constraint. |
| R2 | `R2-exploration-board-progression.png` | Exploration board; the selected progression is 1 dev mode, 2 graphic pixel ecology, 3 painterly pixel diorama. |
| R3 | `R3-world-tilt35-d32-px4.png` | Existing world presentation, wide. |
| R4 | `R4-world-zoom-pool-tilt30-d24-px4.png` | Existing world presentation, close. Framing, not surface quality. |
| R5 | `R5-alien-river-lush-organic.jpg` | Primary lush organic-form and atmosphere reference. |
| R6 | `R6-owl-palette-only.jpg` | Palette only. |
| R7 | `R7-alien-landscape-silhouettes.jpg` | Alien silhouettes, depth, psychedelic colour. |
| R8 | `R8-tower-landscape-massing.jpg` | Massing, repeated forms, depth, luminous accents. |

Original filenames are in the document's "Reference inventory" table.
