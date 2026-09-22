# Cubarium prompt kit — v0.3 (2026-09-21)

Working material for the Stage 2 art pass. Condensed from
`design/art-direction/Cubarium_Art_Direction_v0.1.md` (Wrysk, decided); when they disagree the
art direction wins. Every prompt to any provider = **style block** + **subject card** +
**provider dialect**. Version the style block; write the version into every run log.

Nothing here is canon. The kit is what we revise after each manual round, per provider,
until `art/gen/artbook/` exists and becomes the image reference instead.

---

## 1. Style block

### 1a. Prose form (GPT-image, Gemini, Qwen-Image; ~110 words)

> Cubarium concept art: a lush, visibly pixelated alien ecology, science-fiction and
> psychedelic biology rather than fantasy or magic. Clean pixel art with flat solid colours,
> crisp stepped edges, no gradients, no blur, no haze. Palette: deep indigo, blue-violet and
> dark purple masses supporting electric cyan, blue, violet and magenta, with at most one small
> warm orange accent. Bioluminescent colour comes from the forms themselves, not from fog or
> glow effects. Strong readable silhouette, detail gathered in clusters with quiet dark negative
> space between them. Organic shapes: slopes, banks, pads, fronds, caps, never visible cubes.
> Side-on cutaway diorama view, seen slightly from above. No text, no watermark, no border.

### 1b. Compact form (Midjourney, tag-style models; put it first)

> pixel art, alien ecology, psychedelic sci-fi biology, flat solid colours, crisp stepped
> edges, deep indigo and violet masses, electric cyan and magenta accents, one warm orange
> accent, bioluminescent forms, strong silhouette, clustered detail, dark negative space,
> organic not blocky, side-on cutaway diorama, slightly from above, no gradients, no haze

### 1c. Say what you want (Wrysk, 2026-09-21)

"No …" lists do not work in image generation; the model hears the noun. Every constraint
is phrased as the thing we want instead: not "no deer" but "a low, wide, shell-backed
browser with a blunt wedge head and six short legs hidden under the shell rim". The
generator is given **nothing to imagine**: every sprite subject arrives with a complete
visual description from `design/art-direction/species-designs*.md` (silhouette,
proportions in voxels and logical pixels, every part, every colour by hex, surface
treatment, pose, view angle, ground contact). The creative pass that writes those
descriptions is Fable + Astra + Wrysk; the generator only renders them.

### 1d. What the block deliberately does *not* say

- No pixel scale or sprite resolution (open decision). Scale lives in the subject card.
- No species list, no biome list (open). Cards name one subject each.
- No "cute" and no "grotesque"; neither is the creature language (§05).
- The eight references are not fed wholesale (§07). Use them by role: R1 mockup for pixel
  treatment, R6 owl for palette only, R5 river for organic richness, R7/R8 for silhouette,
  massing and depth. Once the artbook exists, use the artbook instead.

---

## 2. Two arms, two phrasings

Every provider gets both until we know which wins (the plan's phase 1 experiment).

**Arm A, pixel-look direct.** Add to the style block:
> rendered as genuine pixel art on a uniform pixel grid of about [64×64 / 96×96] logical
> pixels, each pixel a single flat colour, limited palette of about 16 colours, 1-pixel
> darker outline on the silhouette, no anti-aliasing
Then `pixel_snap` to a true grid. Models fake grids; expect to snap and quantise.

Wrysk's expectation (2026-09-21): A looks good but its logical scale is off and inconsistent
between assets, so every candidate is scale-normalised to the card's voxel size before it
goes on the sheet, and B (or A + snap + downsample) is the likely final-asset path.

**Arm B, clean graphic then downsample.** Replace "pixel art" wording with:
> flat vector-like illustration, hard edges, cel shading in two tones plus one highlight,
> no outline noise, simple large shapes that survive being shrunk to a thumbnail
Downsample with nearest / area filter to sprite scale and quantise to the palette.

---

## 3. Subject-card template

```
subject:      one organism, plate, or material (from §4)
role:         niche + habitat + neighbours (§07 "generate for a role in the world")
scale:        size in voxels (0.125 m each); show at 8 px and 4 px per voxel in-scene
framing:      sprite | plate | tile | parts
pose/state:   rest | move | feed | growth stage n | decline
must read:    the 2–3 features that must survive at sprite scale
may invent:   ornament left to the model
avoid:        card-specific additions to 1c
```

Angles (the world is 3D; animals move along the strip and in depth under a 30° tilt):
every accepted animal design gets side (mirrored), three-quarter toward camera and
three-quarter away, made from the accepted side view with the edit lane. Plants get growth
states and sway, not angles. Wrysk may also hand in **full sprite overrides** made from
reference photos; they skip generation and go straight to prep.

Framing suffixes:
- **sprite**: "a single [subject], isolated and centred, full body visible, side view facing
  right, base touching an invisible ground line, no cast shadow, no scene, no other objects."
  Qwen 2.1: inside the RGBA wrapper (§5) and **never mention a background at all**; a
  background phrase contradicts the wrapper and the alpha comes back opaque (verified
  2026-09-21: 0 % vs 71 % transparent pixels on the same seed). GPT-image: ask for a
  transparent background. Providers without alpha: "on a plain solid black background",
  then key it out in prep.
- **plate**: "a wide cutaway diorama composition, [aspect], terrain in strata, one pool, a few
  clustered organisms, large dark quiet areas."
- **tile**: "seamless repeating texture tile of [material], no distinct objects."
- **parts**: "the same [subject] as before, showing only its [head / front leg / fin],
  same palette and pixel scale, black background." (edit models only, with the accepted
  sprite as the input image.)

---

## 4. Starter subject cards

> Superseded for generation by the species design sheets (§1c): a card below is the
> ecological brief, the design sheet is the prompt. Cards without a design sheet are not
> generated.

Species names and niches are the live voxel world's (`cubarium-voxel-flora`, `-fauna`).
Physical scale: one voxel = 0.125 m. Frondgrazer body ≈ 0.6–0.8 m ≈ 5–6 voxels ≈ 24 px at 4
px/voxel, 48 px at 8. Nothing here fixes anatomy; "must read" is the readability contract.

**S0 · Habitat plate** (conceptual only: palette and mood reference, never a sprite source; round S0-01 done 2026-09-21, no further plate rounds unless Wrysk asks)
role: one representative screen habitat; sets terrain, water, pixel treatment and palette
relationships for everything after. framing: plate, 16:9 or 21:9. Content: front-low terrain
rising to peaks at the far edge, exposed strata, one pool with a readable edge, one fallen log
with luminous fungus, a few tall crowned plants, a low browsing animal, plenty of near-black.
must read: terrain masses vs water vs living patches vs the one moving animal (§ pillar 6).
avoid: waterfalls, buildings, sky drama, evenly spaced specimens.

**S1 · Bloomcrown** (light-demanding producer of sunny, aerated soil; shallow roots; dies in
standing water). scale: crown 2–4 voxels tall. framing: sprite, states: young / mature /
declining. must read: a distinct crown mass on a stem, clearly a sun plant; where it touches
the ground. may invent: what the crown is (bloom, disc, tuft), texture.

**S2 · Umbrellafrond** (wetland producer; low light, deep roots, wants sustained wetness;
taller crown). scale: 4–6 voxels tall. framing: sprite, young / mature. must read: taller
than bloomcrown, umbrella or frond-like canopy, belongs at a water edge. avoid: earth palm.

**S3 · Springturf** (pioneer turf of open moist soil; fast, short-lived; crown one voxel tall).
scale: 1 voxel tall, spreads sideways. framing: sprite variants ×3 for clumping. must read:
low ground cover, reads as a patch not an individual. avoid: grass blades.

**S4 · Stonecushion** (cushion on bare rock whose roots reach a soil pocket; slow, tiny).
scale: 1–2 voxels. framing: sprite ×2. must read: a dome hugging rock, drought-hardy.

**S5 · Velvetpad** (shade understory pad; damp not waterlogged; low and broad).
scale: 1 voxel tall, 2–3 wide. framing: sprite ×2. must read: broad low pad, lives under
canopies, softer than stonecushion.

**S6 · Glowcap on a log** (wood fungus, **not a plant**; eats dead wood; fruits one cap; grows
in clusters on identifiable logs; restrained luminous caps). scale: log 4–8 voxels long, cap
under 1 voxel. framing: sprite of log + cluster; also cap alone for growth. must read: the log
is identifiable as dead wood; caps are few and restrained, glow is local. avoid: fairy-ring,
toadstool, spots, green.

**S7 · Frondgrazer** (paid ground browser of the grazed meadow; crops reachable foliage with
a mouth at body height; walks to better food; drowns in water it cannot wade). scale: body
5–6 voxels long, low and broad. framing: sprite, poses rest / walk / crop (head lifted to a
crown). must read: which end is the head, that it is low and heavy, a mouth that reaches
upward. may invent: sensing appendages, back ornament. avoid: mammal face, eyes as the
focal point, insect legs count.

**S8 · Terrain and water materials** framing: tile ×4: topsoil with strata edge, bare rock,
dead wood / litter, still water surface. must read: distinct by contour and texture, not by
hue alone (§05). Pool skin reads as a surface, body reads as depth.

---

## 5. Provider dialects (revise per round)

**Midjourney (manual).** Compact block first, then the card in ≤ 25 words. Suggested start:
`--ar 1:1 --style raw --stylize 50 --chaos 5 --no text,watermark,gradient,fog,blur,frame`.
Style reference: `--sref <R1 mockup>` with `--sw 150` for pixel treatment; try adding R6 for
palette only. MJ will not hold a true pixel grid: always arm A + `pixel_snap`, or arm B.
Once the artbook exists, `--sref` its plate and drop R1. Use `--cref` / omni-reference for
pose variants of an accepted creature.

**GPT-image (manual now, API later).** Prose block + card as full sentences; it follows
layout and count instructions, so ask for "a 2×2 grid of the same creature in rest, walk,
crop and rest-facing-left, identical palette and scale" once a design is accepted. Ask for a
transparent background for sprites. State the logical pixel resolution explicitly for arm A.

**Gemini image (manual now, API later).** Prose block; strongest as an *editor*: give it the
accepted sprite and ask for the next pose or growth state. Say "no text" explicitly; it likes
captions. Keep one change per turn.

**ComfyUI local (agent lane).** Wrysk (2026-09-21): **Qwen-Image 2.1 only.** The other
installed checkpoints (Flux, Z-Image, HiDream) are not as good and not worth using; do not
build workflows for them. Models by job:
- New designs: Qwen-Image 2.1 text-to-image, prose block, 1024².
- Next pose / growth state / body angle / part of an accepted design: Qwen-Image 2.1
  reference-to-image (Qwen-Image-Edit 2509 + Lightning 4-step LoRA where wired), input =
  the accepted sprite or artbook plate.
- Pixel-art LoRAs for Qwen may be downloaded (pre-authorised); log what and where.

Qwen-Image 2.1 usage notes (Wrysk, 2026-09-21):
- **Transparent sprites:** wrap the prompt exactly as
  `This is an RGBA format image with transparency. [subject]. The image has an alpha
  channel and a transparent background.` and save as PNG. Use it for every sprite and
  parts framing; plates and tiles stay opaque.
- **Size:** set on the ResolutionSelector node. Default 1 MP (1024×1024); native 2K is 1:1
  at 4 MP (2048×2048). Prefer multiples of 32 for wide plate sizes.
- **Sampling:** cfg 1 on the official path; the negative prompt is unused at cfg 1 and cfg
  goes up only if a negative is used (so 1c is folded into the prompt as "no …" here).
  Euler at 40–50 steps for the official pipeline; the template starts at 25; advanced
  samplers need fewer. Log steps, sampler and cfg per candidate.

**GPT-image via Codex (agent lane, quality).** No API key: `art/gen/tools/gpt_image.sh`
runs `codex exec -m gpt-5.6-terra` with its `image_generation` tool. Prose block + card as
sentences; grids of poses, transparent backgrounds and reference-image edits all work.
Wrysk's read: by far the best upstream provider. Adobe Firefly MCP (GPT-image 2.5) is a
coming second upstream lane, same cards and log.
Record seed, model, workflow file, steps, guidance, size, full prompt per image. Nothing
pixel-art-specific is installed; a pixel-art LoRA for Qwen is pre-authorised to download.

---

## 6. Run log (one `run.json` per candidate, one `LOG.md` per round)

```
{ "subject": "S6", "arm": "A", "provider": "comfyui", "model": "qwen_image_2.1_int8_convrot",
  "workflow": "workflows/qwen21-sprite.json", "seed": 0, "size": [1024,1024],
  "style_block": "v0.1", "prompt": "…", "negative": "…", "params": {"steps": 40, "sampler": "euler", "cfg": 1, "rgba": true},
  "file": "0007.png", "in_scene": ["0007@8px.png","0007@4px.png"],
  "verdict": "keep|iterate|reject", "largest_failure": "one sentence", "by": "wrysk|agent" }
```

Round protocol: one subject, ≤ 8 per provider, contact sheet, in-scene at 8 and 4 px per
voxel, Wrysk marks verdicts, then revise the style block and that provider's dialect and
bump the version. Keep rejects and their logs until the round closes.
