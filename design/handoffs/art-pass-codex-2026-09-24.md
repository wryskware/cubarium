# Handoff: the final art pass (Codex thread with GPT-image) (2026-09-24)

For a Codex thread that Wrysk runs. Codex reads `AGENTS.md` and `WORKING_POLICY.md` at
the repo root. Read both first, then this file, then the sources it names. Fable (the
Claude orchestrator) owns the engine side and integration. You own generating and
preparing assets.

## Why this thread exists

Cubarium is an ambient voxel ecosystem. The GPU renderer has an **experimental** face
texture path (`--textures`, off by default; plain coloured voxels stay the main
look). Wrysk, 2026-09-24:

- the latticevine's tile textures "look way better in practice than the voxel shape";
- "what's really hurting me about the other textures is that all foliage has the exact
  same model rather than being created and tailored to that plant."

So the final pass makes **per-species textures**: each organism's faces are dressed in
material drawn for that organism, from the design Wrysk signed off. Plants come first.

## The gate

Work only on subjects marked **`final`** in `art/gen/SIGNOFF.md`. Wrysk fills that
file in. A subject that isn't final gets nothing from this thread. If nothing is final
yet, stop and say so.

## What is decided (read these; they bind)

- The art direction: `design/art-direction/Cubarium_Art_Direction_v0.1.md` (and
  `design/art-direction/README.md`). The look belongs to Wrysk. You never decide it:
  every candidate is `unjudged` until he judges it.
- The prompt kit: `art/gen/PROMPT-KIT.md` v0.4.
  - The style block leads every prompt.
  - Generators render **clean graphic illustration, never pixel art**. Pixelation is
    ours: the level tool reduces 48 px masters to each display size.
  - Positive phrasing only.
  - Palette hexes are in the dossiers.
- The designs: `design/art-direction/species-dossiers-2026-09-21.md` (D1–D14) and
  `design/art-direction/species-dossier-D15-latticevine-2026-09-23.md`. Each subject's
  parts, counts, colours and states are there.
- The heroes and sheets: under `art/gen/runs/` (all runs are listed in
  `art/gen/index.html`; rebuild it with `python3 art/gen/tools/build_index.py`). The
  2026-09-23 character-sheet run's known failures are in its `LOG.md`: habitat leaking
  from the hero, counts ignored, three-quarter views collapsing. Use sheet crops only as
  references, never whole sheets.

## What the engine consumes

Read `crates/cubarium/src/voxel/textures.rs` (module doc) for the contract. In short:

- **Masters** are 48 × 48 PNG per face. A top master is authored square, seen from
  straight above with row 0 at the back edge; the engine squeezes it for the 30° camera.
  Levels for each display size (4, 6, 9, 12, 13 … px per voxel) are derived by
  `cargo run -p cubarium --example voxel_texture_levels`, which area-averages and snaps
  to the master's own colours. Hand-fixed levels go in `override/<px>/` and win.
- **Cutout alpha:** foliage and drape are exactly 0 or 255, and the rock or whatever lies
  behind shows through the holes.
- **Terrain** masters modulate the material's base colour. **Organism** masters are
  sampled per role: bark (trunk), leaf (foliage), drape, and accent.
- **Latticevine** uses direct-colour tiles by neighbour mask in `masters/vine/`,
  documented in `design/handoffs/latticevine-visual-2026-09-24.md`.
- **Per-species sets (package S, landed; contract in the module doc of
  `crates/cubarium/src/voxel/textures.rs`):**
  `assets/voxel-textures/masters/species/<species>/<role>-<face>-<variant>.png`.
  - species uses the flora names (`bloomcrown`, `umbrellafrond`, `springturf`,
    `velvetpad`, `stonecushion`, `vaulttree`, `lanternberry`, `siphonreed`, `glowcap`).
  - role ∈ `bark` (the model's trunk cells), `leaf`, `drape`, `accent`. `leaf1`,
    `leaf2`, … name one foliage layer each: `leaf<i+1>` is the model's foliage index
    `i`, **bottom-up** among the stand's foliage layers (bloomcrown: `leaf1` is the low
    rosette, `leaf2` the vanes and the core). A layer with no file of its own uses `leaf`.
  - face ∈ `top`, `side`; variant ∈ `0..4`.
  - Direct colour (it replaces the colour pass's hue for that role; lighting still
    shades it), cutout alpha for leaf and drape. Anything missing falls back per role
    and per face to the generic tinted set.
  - Which roles a species' model uses: `bloomcrown` bark, leaf1, leaf2, accent;
    `lanternberry` bark, leaf (two layers), accent; `springturf`, `stonecushion`,
    `siphonreed` leaf only (`assets/voxel-models/125`).
  - Interim masters for the six signed-off plants are in the repository
    (`scripts/voxel-textures/species_masters.py`); a candidate set replaces them.
  - Preview: `voxel_specimens --gpu --textures DIR` puts `DIR` over
    `assets/voxel-textures`, so `DIR` need hold only the candidate species
    (`DIR/masters/species/<species>/…`); `--textures-only DIR` draws with `DIR` alone.

## The plan

**Stage Q (Qwen, local, cheap; runs now, in parallel with Wrysk's sign-off).** This is
Fable's sprite-forge worker, not you. Its jobs:

- redo the plant sheets with the four fixes: crop heroes to the subject, one pose or
  view per image, counts from the blockout only, and remove the rock block from the
  stonecushion blockout;
- make **texture exploration swatches** per plant role, reduced to 48 px and shown in
  the engine, so Wrysk can pick a direction per species cheaply.

Qwen is for exploration; its output is never final.

**Stage G (this thread, GPT-image; final).** For each `final` subject, in the order
plants → terrain materials → animals (animals only once their body plans are chosen):

1. References: the hero cropped to the subject, the chosen sheet crops, and any
   direction Wrysk picked from the Stage Q swatches.
2. For each role the species uses (see its dossier and the model builder
   `scripts/blender/organism_lineup.py`: which parts are trunk, foliage, drape or
   accent), generate a **high-resolution, seamless, tileable** material image, flat and
   face-on, in the dossier hexes. That's `top` and `side` for each role. Make 4
   variants that tile with each other.
3. Reduce each to a 48 × 48 master. Snap to the dossier hexes (see
   `art/gen/tools/palette_quantise.py`), and cut foliage and drape alpha to 0/255.
   Keep leaf holes large and few. Wrysk found fine speckle unreadable at 4–6 px.
4. Derive the levels with the level tool. Look at 4, 6, 9 and 13 px. Hand-fix the 4 and
   6 px levels in `override/` if they turn to mush.
5. **Check in-engine:**
   `cargo run --release -p cubarium --example voxel_specimens -- OUT.png --gpu --px N --textures DIR`,
   at 6 and 13 px, with the candidate set in a scratch textures directory. A
   beautiful swatch that dissolves in-scene is a failure (art direction, "Test assets
   against representative terrain").
6. Write a contact sheet per species: the swatch, the 48 px master, the 6 and 13 px
   in-engine crops, generic vs species side by side.
7. Stop per species for Wrysk's judgment. Promote into
   `assets/voxel-textures/masters/species/<species>/` only after he approves.

## Logging and files

- Runs: `art/gen/runs/<date>-final-<species>/`, with every image logged in a
  `.run.json` in the existing schema (see any file in
  `art/gen/runs/2026-09-23-character-sheets/`). Provider `gpt-image`, the full prompt,
  the references, and `"verdict": "unjudged"`. A `LOG.md` per run. Rebuild the index.
- Work on a branch in your own worktree (e.g. `art-pass-g`). Commit assets and logs
  with **explicit paths** (never `commit -a`; other threads share this repo). Don't
  touch engine code. If the contract above can't hold something you need, write it
  down for Fable instead of changing Rust.
- Never deploy, and never touch the Tachyon panel.

## Checkpoints (report back to Wrysk after each)

1. First species (start with **bloomcrown** if it's final, else the first final
   plant): the full loop once, with the contact sheet and in-engine crops. Wait for his
   read before doing the rest.
2. The remaining final plants, one contact sheet each.
3. The terrain materials.

A checkpoint report is ≤ 20 lines: what was made, the paths to look at (**absolute
paths**; Wrysk asks for them), what fails in-engine, and what you need decided.
