# Brief: character reference sheets (2026-09-23)

Worker: sprite-forge. Wrysk's request: character sheets for every subject — every animal
variant and every plant — to serve as reference inputs for later Qwen-Image 2.1
generations. **This dispatch is the smoke run only (5 images). Stop after it and report;
the full run waits for Wrysk's review.**

## Approach (decided by Fable, 2026-09-23)

- Sheets are anchored to a **hero image**, never generated cold from text: a text-only
  sheet invents a new design per seed and drifts anatomy between panels.
- Provisional heroes (Wrysk may replace them): animals = worker picks in
  `art/gen/runs/2026-09-23-animal-concepts/picks.json`; plants = the Qwen rough of
  `art/gen/runs/2026-09-23-plant-concepts/` (pick one of the two seeds, log which and why).
- Generation stays on **Qwen-Image 2.1**. `TextEncodeQwenImage21` (node 470 in
  `art/gen/workflows/qwen21-official-t2i.json`) takes reference images natively: the
  `images` autogrow input (`images.image_1` … up to 16) plus the optional `vae` input;
  `resolution` sets the reference resize. The workflows README says there is no 2.1
  reference path; that is out of date. Build `art/gen/workflows/qwen21-ref-sheet.json`
  (official graph + LoadImage nodes wired into `images`, vae wired from node 472, wide
  latent e.g. 1664x928 or wider if the five views are cramped), make `comfy_run.py` able to
  set the reference image names, and update the README row. Verify the refs actually take
  effect (one throwaway render with vs. without is fine; say which node names the API
  wants for the autogrow slots).
- Animals may add the **Blender blockout** as a geometry reference: render the subject's
  blockout from `runs/animal-bodies-2026-09-23/animal_bodies.blend` (built by
  `scripts/blender/animal_bodies.py`; FG/LT/BW are in the "settled" row) as one strip of
  the five sheet views, same order and scale, flat shading, plain background. Blender 5.2
  headless is installed. Image 1 = hero (identity, colour, markings), image 2 = blockout
  strip (geometry, counts, attachment). Known risk: in the concept round a blockout used
  as the only reference was copied literally (BW-A crossed bars) — the hero pairing is
  what this run tests.

## Sheet types

- **Turnaround**: one row, five views of the same animal at the same scale on one shared
  ground line: side view facing right, front, back, three-quarter front, three-quarter
  back. Neutral standing pose, plain flat dark neutral background, even lighting, no
  habitat, no text or labels. Same colours, markings, counts and proportions in every view.
- **Action sheet** (animals): 3–4 poses of the same animal in one row, side or ¾ views —
  feeding (its own diet from the dossier), walking stride, alert, resting. References: hero
  + the best turnaround panel(s).
- **Growth sheet** (plants, instead of actions): seedling, juvenile, adult side views, plus
  a top-down view of the adult, same ground line, adult at the same scale as its turnaround.
  Growth geometry follows `design/organism-large-animals-2026-09-23.md` / the dossiers
  (seedling ≤ 0.125 m tall and wide, then linear to adult).

Prompts: kit v0.4 style block (`art/gen/PROMPT-KIT.md`) with the scene/framing sentence
replaced by the sheet instruction (authorised for this round), then the subject
description from its dossier / the concept-round prompt (`prompts.py`, `prompts/`).
Positive phrasing only; refer to references as "the creature in image 1" etc. Kit rule:
clean graphic illustration, never pixel art.

## Smoke run — exactly these 5

| # | subject | sheet | references |
|---|---|---|---|
| 1 | FG frondgrazer | turnaround | hero only |
| 2 | FG frondgrazer | turnaround | hero + blockout strip |
| 3 | BW-A bellwing | turnaround | hero + blockout strip |
| 4 | P-BC bloomcrown | turnaround (side, front, back, ¾, top) | hero only |
| 5 | FG frondgrazer | action sheet | hero + best of #1/#2 cropped panels |

One seed each (2301). Fixes are allowed only for mechanical failures (refs not applied,
wrong canvas); do not iterate on look. Add a panel-crop helper
(`art/gen/tools/crop_panels.py`: split a sheet into per-view PNGs by background gaps or an
even grid) and run it on the outputs.

## Output

- `art/gen/runs/2026-09-23-character-sheets/`: images with `.run.json` logs (existing
  schema, `reference` lists every ref), blockout strips in `refs/`, crops in `crops/`,
  `LOG.md`, and `smoke-sheet.png` (contact sheet of the 5 plus their references).
- Do not commit. Verdicts stay `unjudged`; you never decide look.
- Return ≤ 40 lines: workflow built + evidence refs take effect; per image, what reads and
  the largest failure (counts, drift between views, blockout copied literally, layout);
  hero-only vs hero+blockout comparison for FG; crop helper result; anything that should
  change before the full run (all animal variants in picks.json + all nine plants,
  turnaround + action/growth each, 2 seeds).
