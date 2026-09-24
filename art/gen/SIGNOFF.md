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
| LJ   | lanternjaw     | A–D         | prefer D, the scorpid version. overall idea is good. however, needs refinement or some reimagining.                            |        |
| CH   | chorister      | A–D         | A, leaning. needs some rework and reimagining. perhaps hexapod, or somewhat raptorial like C? not sure i like the smooth body. |        |
| LS   | loftstrider    | A–D         | none of these really satisfy. full reimagining.                                                                                |        |
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
