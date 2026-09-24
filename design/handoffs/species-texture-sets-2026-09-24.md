# Brief: package S, per-species texture sets (2026-09-24)

Part of `design/handoffs/presentation-plan-2026-09-24.md` (read §S and the standing
constraints). Wrysk, 2026-09-24: "what's really hurting me about the other textures is
that all foliage has the exact same model rather than being created and tailored to
that plant … we can probably start S for the ones I signed off on."

Signed off, or "okay to ship" at 4/5 (`art/gen/SIGNOFF.md`, "Fable's reading"):
**bloomcrown, lanternberry, springturf, stonecushion, siphonreed**, and **latticevine**
(tiles). Main is at `5314e7f` or later.

## Deliverable

1. **Engine (GPU renderer only):** organism face textures are looked up by **species
   and role**. Layout and naming:
   `assets/voxel-textures/masters/species/<species>/<role>-<face>-<variant>.png`
   - role ∈ `bark`, `leaf`, `leaf1`…`leafN` (inner to outer foliage layer, when the
     model has several), `drape`, `accent`;
   - face ∈ `top`, `side`; variant `0..4`.
   - Species masters are **direct colour** with cutout alpha for leaf and drape. Any
     missing file falls back per role to today's generic tinted set.
   - The levels are derived, and `override/<px>/species/...` wins, exactly like the
     existing sets.
   - Species identity reaches the shader through whatever the organism model cells
     already carry (style or species index; see `crates/cubarium/src/voxel/model.rs`,
     `colours.rs`, `textures.rs`). Keep the texel budget in mind.
   - Update the contract in the module doc of `crates/cubarium/src/voxel/textures.rs`.
   - Make `voxel_specimens --gpu --textures DIR` work with a scratch directory that
     holds only some species. The Codex art pass
     (`design/handoffs/art-pass-codex-2026-09-24.md`) previews its candidates that way.
2. **Interim per-species masters for the six plants above,** so the look is tailored
   now, before Codex finals. Build them procedurally (extend
   `scripts/voxel-textures/interim_masters.py`, or a sibling script) from each dossier's
   parts and hexes (`design/art-direction/species-dossiers-2026-09-21.md`: D6
   springturf, D8 stonecushion, D9 bloomcrown, D12 lanternberry, D13 siphonreed):
   - bloomcrown: narrow upright vane strips around a core;
   - lanternberry: leaflet dashes on bare stem;
   - springturf: pleated tuft blades;
   - stonecushion: a mosaic of tight lilac rosette knots;
   - siphonreed: hollow stem strips with a ring.
   Leaf holes large and few (fine speckle was unreadable at 4–6 px). These are
   placeholders that follow the dossier, not new look decisions. Say where you had to
   choose.
3. **Latticevine tile colours:** the full tiles are mostly `#1E2798` and vanish against
   rock. Shift the tile generator's leaf ramp up one step (`#2B6AD0` body, `#42C5F8`
   newest scales, `#1E2798` only as the shadow edge), and re-render the tiles with
   `blender -b -P scripts/blender/latticevine.py -- --tiles assets/voxel-textures/masters/vine`
   (about 5 min).

## Standing rules

- Textures stay opt-in (`textures = false` by default). The plain-voxel picture must not
  change.
- GPU only. The CPU renderer is untouched.
- Work in your worktree. Commit with explicit paths. Don't merge or deploy; Fable
  integrates. Use `./scripts/run-voxel.sh --background` for any window. Keep cargo at
  about `-j 8`.
- Short function tests: species lookup with fallback per role, override precedence, and
  a species absent from the directory.

## Evidence and return

- Screenshots in `/home/wrysk/wryskware/cubarium/runs/species-textures-2026-09-24/`:
  each of the six species at 6 and 13 px with `--textures`, generic vs species side by
  side (use `voxel_specimens`); one terrarium crop at auto px; and the plain default,
  unchanged.
- Rerun `python3 art/gen/tools/build_index.py`.
- Return ≤ 25 lines: branch and commits, how species reaches the shader, the look
  choices you made, frame time before and after, and the **absolute path** of the
  first screenshot to open.
