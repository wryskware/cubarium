---
status: open
date: 2026-10-09
owner: opus-high worker, worktree `.claude/worktrees/lj-larva`, branch `lj-larva`
---

# Lanternjaw larva: four candidates for Wrysk to pick from

Rules: `design/handoffs/worker-rules-2026-10-07.md` (no windows: `blender -b` only, 5-minute
calls, explicit-path commits, ≤ 40-line return).

## Why

Wrysk (2026-10-09): "we also need to come up with a larval model for lanternjaw". The larva
is the sim's **rider** stage (`design/predation-and-flight-2026-09-28.md` §2.13,
`design/lanternjaw-detritus-web-proposal-2026-10-05.md` §1.2):

- it hatches from a buried egg and clamps onto a host's back (any nearby animal that isn't
  a lanternjaw; frondgrazers mostly, but also capgnawers, shredders, snails);
- it drinks from the host;
- it drops off at about a quarter of adult body and from then on draws as the adult model's
  smallest bins.

Today a rider draws as an interim block (`glyph_spec` in `crates/cubarium/src/voxel/figure.rs`).

These are **candidates**: Wrysk picks the look. Build exactly the four directions below,
don't invent others, and flag where a direction can't read at this size instead of
redesigning it.

## The adult it grows into

`lanternjaw()` in `scripts/blender/animal_models.py` and `art/gen/artbook/lanternjaw.png`:
violet plates over a dark body, pale vertical fangs, magenta eyes, mantis arms with pale
blades, and two tails forking at the root, each with a cyan lure orb (`a_lure`). Use only
the existing palette slots (`a_main`, `a_dark`, `a_head`, `a_eye`, `a_lure`, `a_mark`,
`a_limb`, `a_foot`).

## Size

At 0.125 m voxels the rider is tiny. Build each candidate compact, about 3–4 voxels long, 2
wide and 1–2 tall. The sim's length for a rider (0.46–0.62 m) would make it two-thirds of
an adult grazer; how big to draw it is Wrysk's call, so note the length each candidate was
built at.

## The four directions

1. **Tick.** A flat, round-backed clamp of overlapping violet plates, one voxel tall, with
   pale hooked fangs dug into the host at the front. A single short tail nub curls up at
   the back with one cyan lure dot: the twin tails haven't forked yet. Reads as a purple
   button with a cyan dot.
2. **Grub.** A segmented larva, 4 segments, arched like an inchworm mid-step: head and tail
   on the host, middle raised. A pale head capsule biting in, a cyan glow at the tail tip.
   Reads as a violet loop.
3. **Nymph.** A miniature adult, like a mantis nymph: oversized head and fangs, striking arms
   folded tight, legs clamped round the host's flanks, the tail one unforked upright stub
   with a small lure bud. Reads as a baby lanternjaw hugging its host.
4. **Lamp-leech.** A limbless body draped along the host's spine, 1 voxel wide and 4 long; a
   pale sucker mouth at the front; the back end lifted with two tiny lure buds, the twin
   tails as two dots. Reads as a stripe with two cyan dots.

## Deliverable

- A builder per candidate in `animal_models.py`: `lanternjaw_larva(kind)` with kind one of
  tick, grub, nymph and leech. Voxelize with the bake's own rule (`voxelize_animal`) at
  0.125 m, placing parts on cell centres as the lanternjaw builder's notes describe.
- **One contact sheet**, `art/gen/runs/lanternjaw-larva-2026-10-09/sheet.png`: per candidate,
  the voxelized larva on an adult frondgrazer's back at true scale, three-quarter and side
  views; the larva alone enlarged; and beside them the smallest adult lanternjaw bin for
  the hand-over at drop-off. Render it with the existing voxel-cube sheet helpers
  (`voxel_cubes`, `ortho_camera`) and the live palette's colours.
- Commit the builder and the sheet's generator script. Images stay gitignored, as under
  `art/gen/runs/`.

Not in this package: baking into `assets/`, drawing riders in the engine, sim changes.
Return the sheet's absolute path, the cell count and length of each candidate, and any
direction that didn't read.
