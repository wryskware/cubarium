# Brief: in-world concept illustrations of every animal body candidate (Qwen-Image 2.1)

Wrysk, 2026-09-23: "have qwen image 2.1 generate fully illustrated concept art for each
body type. these shouldn't be sprites, they should be full in-world character concept
illustrations." Worker: sprite-forge. Direction only: Wrysk judges; nothing here is an
asset, nothing goes to the artbook.

## Subjects (29 candidates)

Descriptions: `design/animal-body-candidates-2026-09-23.md` (codes and body plans). More
detail where it exists: frondgrazer = `design/art-direction/species-dossiers-2026-09-21.md`
D3 (saddle shield), littershredder = D4 (plough segment), bellwing = D14 (BW-A) and the
alternates in the candidates doc; loftstrider, lanternjaw and chorister niches in
`design/organism-large-animals-2026-09-23.md`. Rough 3D blockouts of every candidate at
true scale: `runs/animal-bodies-2026-09-23/row-*.png` (silhouette and proportion
reference; they are crude blocks, not the look).

FG, LT, BW-A, BW-B, BW-C, LS-A..D, LJ-A..D, CH-A..D, CG-A..D, RS-A..D, SP-A..D.

## What each image is

A **character concept illustration in its world**: the creature is the clear subject,
whole body visible, in the middle ground, doing the thing its niche is about, with its
habitat around it. Not a sprite, not isolated, not a diorama cutaway. Suggested scenes
(adapt per candidate):

- loftstrider: feeding in the crown of a vaulttree (a sparse branching tree whose crown
  is separate rounded foliage lobes with sky between, hanging filaments below), 3.5 m
  up; a frondgrazer (a low six-legged grazer with a saddle-shaped shield) near its feet
  for scale.
- lanternjaw: waiting in a dark stand of layered umbrella fronds or a meadow, its
  cyan lure the brightest thing in the frame; a frondgrazer approaching.
- chorister: a pack of three to five running or closing on prey, the call organ
  flashing cyan on one or two of them.
- capgnawer: on a fallen, split log colonised by glowcaps (folded saddle-shaped caps
  with a cyan glowing lower edge).
- ripple snail: on a wet rock at a pool's edge coated with a thin gold-green film, a
  cleared trail behind it.
- seedporter: in a fruiting shrub with hanging luminous bell-lanterns (warm orange
  inside when ripe) or among a vaulttree's hanging filaments.
- frondgrazer: cropping the low rosette at the foot of a bloomcrown (a stem with a crown
  of upright blue vanes) in a turf meadow; littershredder: in leaf litter under fronds;
  bellwing: hovering at a bloomcrown's crown, sipping.

Relative scale matters (the sizes are decided): state it through neighbours
(e.g. "as tall as the tree it feeds in", "the grazer at its feet reaches its knee"),
not in metres.

## Style (authorised variant for this round only)

Use kit §1a (scene block) with its last sentence, "Side-on cutaway diorama view, seen
slightly from above.", replaced by a framing sentence of your own per image: an in-world
concept illustration at the creature's scale, the subject whole and readable, depth
behind it. Keep the palette sentence (warm orange only on fruit or bloom), the §1c
phrasing rules (say what you want; outline moves not heights; emissive tissue by its
neighbour; front-to-back anatomy; positive sentences; no similes). Landscape 16:9 at a
Qwen-native size (e.g. 1664×928). No RGBA wrapper, no background removal.

## Budget and procedure

- Qwen-Image 2.1 text-to-image only (reference-edit from the blockouts allowed if a
  body plan will not come through in text, max 6 images that way). No GPT lane.
- One prompt per candidate, 2 seeds each (58 images). Then one fix round, only for
  candidates whose images miss the body plan outright (name the failure, change one
  thing): max 30 more images. Hard stop at 90.
- Waived for this round: the in-scene 8/4 px checks (these are not sprites). Still log
  every image (`run.json`, `LOG.md` with the full prompt per candidate).
- Output: `art/gen/runs/2026-09-23-animal-concepts/` with `<code>-<seed>.png`, one
  contact sheet per species (`sheet-<species>.png`, codes labelled), and
  `sheet-all.png` (one best image per candidate, your pick marked as "worker pick", not
  a taste call). Commit nothing.

## Return (≤ 40 lines)

Counts and time; sheet paths; per species, which candidates came through as described
and which did not (be blunt: e.g. "LS-C never reads as an arch"); phrasing that worked or
failed; proposed kit notes for illustration rounds.
