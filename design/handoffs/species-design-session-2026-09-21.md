# Species design session: handoff for Wrysk's own thread

Wrysk (2026-09-21): the conceptual organism designs are done **interactively, by Wrysk
with a model**, in a fresh thread. Not a review round, not a worker package. The
generation-infrastructure thread stays separate. Paste the "Prompt" section below into a
new Fable session in this repo.

## What exists

- `design/art-direction/Cubarium_Art_Direction_v0.1.md`: the art direction, binding.
- `design/art-direction/species-designs-draft-2026-09-21.md`: the sheet format and two
  first sketches (G1 glowcap on a log, F1 frondgrazer). Opening moves, not proposals.
- `design/art-direction/species-designs-draft-2026-09-21-astra-notes.md`: Astra's notes
  on that draft; Wrysk found several good. Use as ideas, not verdicts.
- `art/gen/PROMPT-KIT.md` v0.3: style block, §1c "say what you want" (positive phrasing
  only, nothing left to the model), Qwen 2.1 dialect and RGBA wrapper, run-log schema.
- `design/theoretical-biosphere-2026-09-16.md` §4: each organism's role and its
  "readable form" column. Live species: bloomcrown, umbrellafrond, springturf,
  stonecushion, velvetpad, glowcap (fungus), frondgrazer.
- `design/appearance.md`: the palette family with hex values.
- Rendering on demand: the `sprite-forge` subagent (`.claude/agents/sprite-forge.md`,
  its own ComfyUI MCP) renders a description as a transparent sprite in ~10 s on
  Qwen-Image 2.1 with `art/gen/workflows/qwen21-official-t2i.json`; GPT-image via
  `art/gen/tools/gpt_image.sh` for a quality pass. Results land in `art/gen/runs/`.

## Prompt (paste into a fresh Fable session)

> This is Wrysk's organism design session for Cubarium's Stage 2 sprites. Read
> `design/handoffs/species-design-session-2026-09-21.md`, then the files it lists, in
> order. You are the sketching partner, not a reviewer and not a decider: for each species
> I name, offer two or three body plans in words (proportions in voxels, the one feature
> that tells the niche), let me pick or mix, then fill the design sheet with me part by
> part: named parts for a Godot cutout rig, colours by hex from the palette family,
> surface treatment, poses, the three angles for animals (side, three-quarter toward,
> three-quarter away) or growth states for plants, ground contact. Everything phrased as
> what to draw, never as "no X"; the image model must be left nothing to imagine. Whenever
> a description is settled enough, have `sprite-forge` render it once on Qwen-Image 2.1
> (transparent, 1024², never mention a background in the prompt) and show me the PNG
> before we go on. Write results into
> `design/art-direction/species-designs-draft-2026-09-21.md`; a sheet becomes decided
> only when I say so. Start with the glowcap log, then the frondgrazer, then the plants.
> Keep the thread lean: no plates, no Astra rounds, no reports; the sheet is the record.

## What the simulation actually has (read before designing any "log")

Sprites render simulation state, never decoration. Verified 2026-09-21 in
`crates/cubarium-voxel-flora/src/{lib.rs,step.rs}` and `crates/cubarium/src/voxel/{stand.rs,habitat.rs}`:

- **There is no log object.** Dead wood is a per-column scalar pool on a ground site
  (`Ground::dead_wood`, with its mineral and energy). It has an amount, a site, and no
  shape, species or orientation. Litter and carrion are two more scalar pools on the same
  site.
- **Where it comes from.** (a) Any stand that dies (`step::die`): its wood goes to dead
  wood on its own site, its foliage and reserve to litter; so a dead plant leaves a
  one-column deposit where it stood. (b) A consumer that kills a stand deposits DeadWood.
  (c) The dev habitat lays "declared logs" of 0.4 organic under each of 12 glowcap
  founders, clustered in threes, because a fresh world has no dead wood at all.
- **Glowcap** is a stand rooted on a site whose mycelium box (radius 1, one layer up and
  down) sums the dead wood and litter of the neighbouring sites, eats them down, and can
  only establish where the box holds enough dead wood. It is half a voxel tall, one cell,
  spreads by hop 1 along whatever dead wood is adjacent, wood 0.01–0.1.
- **The presenter today** draws a `Part::Log` in the cell above any site with dead wood
  ≥ 0.05, a `Part::Litter` mark for litter ≥ 0.05, and the glowcap as an interim
  one-cell cap glyph on its ground face.

So the sprite work for "glowcap on a log" is three independent families that the
presenter composes from state, not one picture: (1) **dead-wood tiles** per column,
autotiled by neighbours (lone chunk, log middle, left end, right end) and scaled or
staged by amount as the fungus eats it out; (2) **glowcap caps** on the site face, sized
by stand wood, with growth and spent states; (3) **litter marks**. A multi-cell fallen
trunk with a direction would be a simulation change (a stand's death depositing along a
line), which is a model decision for later, not an art decision.
