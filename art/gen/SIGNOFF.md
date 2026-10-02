# Art sign-off sheet

Wrysk fills this in. It is the gate for the final art pass
(`design/handoffs/art-pass-codex-2026-09-24.md`): the final pass, run in Codex, works
only on rows marked **final**. Everything else stays exploration.

Status values: `final` (design and hero locked; generate finals), `explore` (more Qwen
work first), `redo` (the design isn't right yet), or blank (not looked at).

Heroes and sheets are under `/home/wrysk/wryskware/cubarium/art/gen/runs/`. Browse them
all in `/home/wrysk/wryskware/cubarium/art/gen/index.html`.

## Plants (first)

| code | species                  | hero / sheets                                                                         | status                                                                                                            | notes                                       |
| ---- | ------------------------ | ------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | ------------------------------------------- |
| LV   | latticevine              | signed-off dense Blender model + tiles (`runs/latticevine-model-2026-09-23/`)         | final (shape, 2026-09-23)                                                                                         | textures: tiles exist; colours need lifting |
| BC   | bloomcrown               | `2026-09-23-plant-concepts/P-BC-qwen-*`, `2026-09-23-character-sheets/sheet-P-BC.png` | signed off                                                                                                        | sheets show 7–8 vanes; dossier says 6       |
| UF   | umbrellafrond            | `P-UF-qwen-*`, `sheet-P-UF.png`                                                       | 3/5. i dont love the radial symmetry                                                                              |                                             |
| ST   | springturf               | `P-ST-qwen-*`, `sheet-P-ST.png`                                                       | 4/5 stars                                                                                                         | sheets read well                            |
| VP   | velvetpad                | `P-VP-qwen-*`, `sheet-P-VP.png`                                                       | dunno how i feel about it. its a little odd looking. maybe it just looks too regular of a pattern                 |                                             |
| SC   | stonecushion             | `P-SC-qwen-*`, `sheet-P-SC.png`                                                       | 4/5 stars                                                                                                         | blockout rock leaked into the sheet         |
| VT   | vaulttree                | `P-VT-qwen-*`, `sheet-P-VT.png`                                                       | 3/5 stars. looks okay but just feels like one of those africa trees in the illustrations. would need improvement. |                                             |
| LB   | lanternberry             | `P-LB-qwen-*`, `sheet-P-LB.png`                                                       | signed off                                                                                                        | counts drift (5–8 stems)                    |
| SR   | siphonreed               | `P-SR-qwen-*`, `sheet-P-SR.png`                                                       | 4/5 stars                                                                                                         | counts drift (1–4 stems)                    |
| GC   | glowcap (with dead wood) | `P-GC-qwen-*`, `sheet-P-GC.png`                                                       | 3/5. the top side is just a fleshy pink.. not very interesting. they just read as regular mushrooms here.         | drawn as a log colony                       |

## Terrain materials (second)

| material | status | notes |
| --- | --- | --- |
| rock | | interim textures in `assets/voxel-textures/masters/` |
| soil / turf top | | |
| bedrock | | |
| litter (terrain axis) | | |
| water surface | | shader work first (see the presentation plan) |

## Animals (third; body plans still open)

| code | species        | variants    | chosen body plan                                                                                                               | status |
| ---- | -------------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------ | ------ |
| FG   | frondgrazer    | — (settled) | saddle shield                                                                                                                  |        |
| LT   | littershredder | — (settled) | plough segment                                                                                                                 |        |
| BW   | bellwing       | A / B / C   | A, final                                                                                                                       |        |
| LJ   | lanternjaw     | A–D         | prefer D, the scorpid version. overall idea is good. however, needs refinement or some reimagining.                            | final 2026-10-01: `artbook/lanternjaw.png` + model |
| CH   | chorister      | A–D         | A, leaning. needs some rework and reimagining. perhaps hexapod, or somewhat raptorial like C? not sure i like the smooth body. |        |
| LS   | loftstrider    | A–D         | none of these really satisfy. full reimagining.                                                                                | final 2026-10-01: `artbook/loftstrider.png` + model |
| CG   | capgnawer      | A–D         | torn between A and D. not sure it has any reason to fly though.                                                                |        |
| RS   | ripple-snail   | A–D         | B or D. D is giving canoli squid, while B is giving sand dollar. might need some adjustments.                                  |        |
| SP   | seedporter     | A–D         | i would combine A and B. keep the alien monkey vibe but also give it skin flaps to glide with                                  |        |

*for reimagining, lets ask astra to write some body descriptions in prose, then generate new exploration images with qwen.
## Fable's reading of the comments above (2026-09-24)

Wrysk: "4/5 means I'd be okay with this shipping, but it could still be improved."

- **Final** (the Codex pass may generate finals): **LV** (shape), **BC**, **LB**, and at
  4/5 **ST**, **SC**, **SR**. Animals: **BW-A**.
- **Rework before finals** (3/5 or unsure):
  - **UF**: the radial symmetry doesn't work.
  - **VP**: too regular a pattern, odd-looking.
  - **VT**: reads as an African savanna tree.
  - **GC**: the cap top is plain fleshy pink and reads as ordinary mushrooms.
- **Animals to reimagine**, via Astra prose and then Qwen exploration:
  - **LJ**: refine D, the scorpid.
  - **CH**: rework from A; maybe a hexapod, or raptorial like C; not a smooth body.
  - **LS**: full reimagining.
  - **CG**: A or D; question whether it needs to fly.
  - **RS**: B (sand dollar) or D (cannoli squid), adjusted.
  - **SP**: combine A and B, an alien monkey with gliding skin flaps.
  - **FG** and **LT** stay settled.
- **Terrain materials:** not yet reviewed.

## Reimagining picks (Wrysk, 2026-09-24)

From `art/gen/runs/2026-09-24-reimagining-concepts/` (options described in
`design/animal-body-reimagining-2026-09-24.md`):

- **Chorister: CH-A2** (six-footed split-jaw runner). "Works for me. The images aren't
  great but the idea is good."
- **Ripple-snail: RS-D2** (open-front folded case). Works.
- **Capgnawer: leaning CG-D2** (folded spore mantle).
- **Seedporter: probably SP-AB2** (six-limbed grove glider). "The image doesn't really
  depict it."
- **Still open:** lanternjaw, loftstrider, umbrellafrond, velvetpad, vaulttree,
  glowcap. "Either the pictures don't really work for it or the new body plan is bad."

## Blockout review (Wrysk, 2026-09-24)

Blockouts in `runs/animal-bodies-2026-09-24/`:

- **Seedporter SP-AB2:** blockout is good.
- **Chorister:** the blockout "still reads like a bug". The look to follow is
  `art/gen/runs/2026-09-24-reimagining-concepts/CH-A2-2401-b0.png`, but with **six legs**
  and **no blob-shaped tail**.
- **Ripple-snail:** follow `RS-D2-2401-b0.png` (dome case, foot skirt, two pink-tipped
  lobes out front, trail behind).
- **Capgnawer:** follow `CG-D2-2401-b0.png` ("this little dude": squat, fringed mantle,
  pink eyes, short legs, on a log).
- **All animals:** "we need more colour variety and texture on all the animals in
  general." The 2026-09-23 colour pass reads well in the world; keep its spirit.

## Animal bodies and colour: final (Wrysk, 2026-09-24)

Blockouts are in `runs/animal-bodies-2026-09-24/`; script `scripts/blender/animal_bodies_r2.py`.
Hexes are in that folder's `README.md`.

| code | body (final) | colour (final) | kept alternate |
| --- | --- | --- | --- |
| CH | CH-A2 v3 (`row-chorister-v3.png`, `strip-CH-A2-v3.png`) | colourway 1, violet stalker | colourway 2, rust |
| RS | RS-D2 v2 (`row-ripple-snail-v2.png`, `strip-RS-D2-v2.png`) | colourway 1, azure dome | colourway 2, jade |
| CG | CG-D2 v2 (`row-capgnawer-v2.png`, `strip-CG-D2-v2.png`) | colourway 1, rose-mauve | colourway 2, lichen |
| SP | SP-AB2 (`row-seedporter.png`, `strip-SP-AB2-v2.png`) | colourway 1, moss | **colourway 2, orchid pink (Wrysk likes it)** |
| FG | saddle shield (settled) | colourway 1, coral | **colourway 2, periwinkle blue (Wrysk likes it)** |
| LT | plough segment (settled) | colourway 1, gold | colourway 2, teal |
| BW | A (final) | **original colours, not recoloured** | — |

Wrysk: "top row for everything … except bellwing, which we should keep the original and
not try to recolor." The chorister is good; GPT-image does the final details pass.
Alternates stay available ("maybe different lineages, or just an option to pick from").
See backlog §8.

## Lanternjaw pick (Wrysk, 2026-10-01)

Final: Wrysk's own mantis-scorpid reference with twin tail lures
(`art/gen/artbook/lanternjaw.png`), refined in a Blender/voxel round
(`scripts/blender/animal_models.py` `lanternjaw()`, baked as the lanternjaw founder):

- The tail forks at its root. The image only draws a second end beside one tail.
- The tails climb higher and reach over the face.
- The body tapers and turns up into the tail early.
- The fangs are larger and vertical.
- The striking arms end in pale razor blades.
- The orbs are one voxel each, one column behind the fangs, held from the cell behind.
- No cells join the two tails on top.

"great. lets go with this one."

## Loftstrider pick (Wrysk, 2026-10-01)

Final: Wrysk's own radial reference (`art/gen/artbook/loftstrider.png`), refined in a
Blender/voxel round (`scripts/blender/animal_models.py` `loftstrider()`, baked as a
specimen). His counts, which the image does not keep:

- Six legs.
- Six eyes, each between two legs.
- Six tusks and six fronds round the mouth, alternating and equally spaced.

The voxel colours are more vibrant to match the image. The fronds are mixed sizes: 100, 75
and 60 %, each twice ("lets do mixed").

## Seedporter poses (Wrysk, 2026-10-01)

Final: four models of the one seedporter. The existing model is the **climbing** pose
("we can keep the existing one as the climbing model"). Three new poses were picked from
Qwen 2.1 edits of the artbook image (`art/gen/runs/2026-10-01-seedporter-poses/`, his
annotated sheet `sheet-seedporter-poses-annotated.png`):

- **Standing** (`artbook/seedporter-stand.png`): perched upright, eating. Ground level is
  under its feet and the tail hangs below it.
- **On the ground** (`artbook/seedporter-ground.png`): on all fours. "tail could be
  thinner on the ground model and hind legs move one voxel back."
- **Gliding** (`artbook/seedporter-glide.png`): level, the membranes spread.

His anatomy, which the images do not keep: "wing flaps should connect from the knees of
the hind legs to the wrists of the fore arms. the small arms are the middle pair." The
images' outer glow is unwanted ("none should really have that stupid outter glow") but
stays for now. Built on one rig in `scripts/blender/animal_models.py` and baked as
specimens `seedporter-stand`, `seedporter-ground` and `seedporter-glide`.
