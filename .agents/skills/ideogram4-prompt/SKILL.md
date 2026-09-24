---
name: ideogram4-prompt
description: Write Ideogram 4 prompts in its structured JSON caption format (high-level description, style block, background, per-subject elements with bounding boxes and hex palettes) and run them on the local ComfyUI Ideogram 4 workflow. Use whenever generating with Ideogram 4, converting a Cubarium prose prompt or subject card for Ideogram, or when Ideogram output ignores layout, adds subjects, or drifts off-palette.
---

# Ideogram 4 prompts: structured JSON captions

Ideogram 4 is trained on **structured JSON captions**, and its official inference validates
prompts against that schema. Plain prose works, but JSON gives predictable layout, subject
count and palette, and trips the built-in safety filter less. **Always send JSON.**

Sources (read when in doubt): the saved ComfyUI workflow `image_ideogram4_t2i.json` (its
default prompt is a worked example), and `caption-template.txt` beside this file: the
template's own prose→JSON conversion rules, verbatim. The official guide is
`github.com/ideogram-oss/ideogram4/blob/main/docs/prompting.md`.

## Shape

One JSON object, minified to a single line, passed as the **text of the `CLIPTextEncode`
node** in the Ideogram 4 graph. The template's "Caption Prompt Template" subgraph is
display-only and is not wired to the model, so you write the JSON yourself.

```json
{
  "high_level_description": "≤50 words, one sentence: subject, medium, composition.",
  "style_description": {
    "aesthetics": "style keywords",
    "lighting": "scene-wide lighting",
    "medium": "illustration",
    "art_style": "named style, short",
    "color_palette": ["#12093A", "#2A0E4A", "#42C5F8"]
  },
  "compositional_deconstruction": {
    "background": "the scene shell: ground, sky, distant scenery, atmosphere, lighting",
    "elements": [
      {"type": "obj", "bbox": [y1, x1, y2, x2], "desc": "30–60 words", "color_palette": ["#…"]}
    ]
  }
}
```

- `medium`: say `illustration` explicitly. Silent or ambiguous means **photograph**
  to this model, and aliens do not change that.
- For photos use `"photo": "lens/film details"` instead of `art_style`.
- `color_palette`: up to 16 hex codes globally, and a short per-element list steers that
  subject's colours. This is where the Cubarium palette goes.
- `bbox` is `[y1, x1, y2, x2]`, normalised 0–1000 on **both** axes, top-left origin, and
  optional per element. On a 16:9 frame a 0–1000 span in x is 1.78× longer than the same
  span in y, so size boxes for the on-screen shape you want (e.g. a square subject is
  about `[300, 400, 700, 625]`).

## Rules that matter (from the template's conversion rules)

1. **One subject = one element.** An animal is ONE `obj`; its parts (legs, vanes, lure,
   shell) go in that element's `desc`. Never split a creature into parts.
2. **Each other subject gets its own element with a full standalone desc.** A neighbour
   named in passing drifts into an Earth animal (seen: the frondgrazer became a tapir or
   deer). If a neighbour is only there for scale, give it an element with its own
   front-to-back description and bbox, or leave it out.
3. **Ground, sky, horizon, water surface, distant trees: `background` only.** Never emit
   the ground as an element (the renderer clips feet into it). Discrete things on the
   ground (a log, rocks, litter) are elements.
4. **Desc = identity first, then major attributes, 30–60 words.** No shadows, no camera
   language, no impression words (`luminous`, `glowing`, `vibrant`, `lush`,
   `stunning`). Say the physical fact instead: "a band of bright cyan tissue along the
   lower edge of the cap".
5. **Commit to one value.** No `or`, `such as`, `various`, `implied`, `suggested`.
6. **No `warm` as a grade.** A warm-coloured source is named concretely (e.g. "orange
   fruit, #FF9B50") and the global grade stays as specified.
7. **Anchor placements** to named landmarks ("perched on the upper edge of the log,
   directly above the split").

## Learned on our subjects (model compare, 2026-09-23)

- **Plain prose gets blocked.** 5 of 8 plain-text Cubarium prompts came back as a grey
  "Image blocked by safety filter" image. That is a normal-looking PNG, not an error, so
  check every output. The same 8 as JSON: 0 blocked.
- **A short background renders a void.** "dark … sky" in one sentence plus a dark
  `lighting` line gave the bellwings an empty black frame. `background` carries the whole
  habitat: ground surface, mid-distance plants, the sky's colour as a hex, depth.
- **Creature descs may run to about 90 words** when the body plan needs it. The
  template's 60-word cap is a captioner guideline; 61–94-word creature descs rendered fine
  and kept their anatomy. Keep neighbours and props at 30–60.
- **Glow is a colour on a part**, not an impression: "a bright cyan bulb", "a band of
  #42C5F8 along the lower edge", never "glowing"/"luminous".

## Cubarium mapping

- Kit style block (`art/gen/PROMPT-KIT.md` §1a/§1a′) → `style_description`:
  - `aesthetics`: "clean flat-colour graphic illustration, hard edges, two-tone cel shading
    plus one highlight, large simple shapes, science-fiction psychedelic biology"
  - `medium`: "illustration"
  - `color_palette`: the kit palette hexes (#12093A, #2A0E4A, #3A1A7A, #510B6D, #B99BE6,
    #1E2798, #2B6AD0, #42C5F8, #FF2AFC, plus #FF9B50 only when fruit or bloom is in frame)
- Body plan (dossier / candidate entry) → the creature's element: identity sentence
  first ("Alien canopy browser, the only animal in the scene"), then the front-to-back
  anatomy, then the one distinguishing detail. Part counts above two are unreliable in
  any model. Put them in, but judge them and don't expect them.
- Habitat → `background`; named plants and logs the creature touches → their own elements.
- Relative scale: state it through bboxes (the loftstrider's box as tall as the tree's
  box) plus one comparative phrase in the desc.

## Running it locally

Flatten the saved `image_ideogram4_t2i.json` subgraph and keep its settings (Default
preset: 20 steps, euler, `Ideogram4Scheduler` mu 0 / std 1.75, `DualModelGuider` cfg 7,
`CFGOverride` cfg 3 from 0.7 to 1.0; models `ideogram4_fp8_scaled`,
`ideogram4_unconditional_fp8_scaled`, `qwen3vl_8b_fp8_scaled` with type `ideogram4`,
`flux2-vae`). Change only the prompt text, seed and size (multiples of 16; 1664×928 for
16:9). There is no negative prompt: the unconditional model is the negative. Never use
the paid `IdeogramV4` API node. Log the JSON verbatim in `run.json`.
