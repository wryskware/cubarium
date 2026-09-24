# Brief: in-world plant concept illustrations, three models side by side

Wrysk, 2026-09-23: happy with the plants so far; "wouldn't hurt to do a round with the
other image models". Worker: sprite-forge, low effort, mechanical. Direction only: Wrysk
judges; nothing is an asset, nothing goes to the artbook.

## Subjects (9)

Body plans: `design/art-direction/species-dossiers-2026-09-21.md` (the primary body plan
of each). Shape in the model (layers, stages): `design/organism-anatomy-2026-09-21.md` §3.
Sizes: the ladder (`design/art-direction/organism-scale-and-roster-2026-09-21.md` §4),
with the growth fix in `design/organism-large-animals-2026-09-23.md` §2 (seedlings are
small ground rosettes). The Blender blockouts at true scale:
`runs/scale-lineup-2026-09-22/tilt30-ladder.png`.

| Code | Plant | Dossier | Scene |
| --- | --- | --- | --- |
| P-ST | springturf | D6 pleated tuft | a turf clearing on moist open ground, several clumps, one clump with a cropped margin |
| P-VP | velvetpad | D7 carpet | a teal nodular carpet under a rock ledge and a frond's shade |
| P-SC | stonecushion | D8 rock-lip dome | domes on a rock shelf's lip, roots into a crack |
| P-BC | bloomcrown | D9 vaned crown | a meadow stand: seedling rosettes, juveniles, adults with the basal rosette and the crown of vanes, one warm bloom |
| P-UF | umbrellafrond | D10 layered umbrella | a wet hollow with fronds of three sizes, adult tiers wider at the bottom |
| P-VT | vaulttree | D11 vault | one adult vault in a gap: trunk, arching limbs, separate foliage lobes with sky between, hanging filaments; a juvenile nearby |
| P-LB | lanternberry | D12 bell-lantern shrub | a grove-edge shrub, stems in a fan, hanging bells, some ripe (warm inside) |
| P-SR | siphonreed | D13 siphon stems | clumps standing in shallow water at a pool edge |
| P-GC | glowcap on dead wood | D1 split trunk + saddle crown | a fallen split log with a glowcap colony (buttons, fruiting saddles with cyan lower lips, threads along the split) |

Where a scene has room, a frondgrazer grazing (a low six-legged grazer with a saddle-shaped
shield) gives scale. The grazer is about 0.75 m long, a bloomcrown up to 1 m, an
umbrellafrond up to 2 m and a vaulttree 3.5 m. In Ideogram JSON the grazer is its own
element with a full desc (skill rule 2); for prose prompts, leave it out if it would take
more than one sentence.

## Models and prompts

For each subject write ONE prose prompt (kit §1a scene block with its last sentence
replaced by an in-world framing sentence, the same authorised variant as the animal round,
§1c phrasing rules) and ONE Ideogram 4 JSON caption built from the same content, following
the `ideogram4-prompt` skill (`.agents/skills/ideogram4-prompt/SKILL.md`: read it; a
full habitat background, glow as a colour on a part, medium illustration, kit palette).

- Qwen-Image 2.1: the prose prompt, `art/gen/workflows/qwen21-t2i-illustration.json`.
- Ideogram 4 (local, saved workflow `image_ideogram4_t2i.json`): the JSON caption.
- Krea 2 (local, saved workflow `image_krea2_turbo_t2i.json`, prompt enhancer OFF):
  the prose prompt.

Seeds 2311 and 2312, 1664×928, template settings otherwise. 9 × 3 × 2 = 54 images. No
fix rounds. Check every Ideogram output for the grey "Image blocked by safety filter"
image and count those as failures. No paid API nodes, no downloads.

## Output

`art/gen/runs/2026-09-23-plant-concepts/`: `<code>-<model>-<seed>.png` with run.json
(prompt or JSON verbatim), `prompts/` (prose and JSON per subject), `compare.png` (one row
per plant, columns Qwen | Ideogram 4 | Krea 2, seed 2311), `compare-2312.png`. Commit
nothing.

## Return (≤ 15 lines)

Counts, times, blocked or failed images, per plant one short line on whether the dossier's
body plan came through in each model (fact, not taste), and the compare.png path.
