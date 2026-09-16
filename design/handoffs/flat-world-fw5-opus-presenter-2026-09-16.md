---
design_status: exploration
last_reviewed: 2026-09-16
---

# FW-5 (Opus, high): the CPU presenter draws a ring world

Scope note: the panel is rendered by the GPU crate (GS-1); this package makes
the **CPU** art presenter correct on a ring for PNG captures, tests, the web
viewer and the desktop. The tick-rate caching layer FW-P proposed is **not**
required here; correctness and the cube's unchanged output are.

Read, in order: `design/7_Research/flat-world-fw3-2026-09-16.md` (canvas,
`PixelCells`, `Unfolds`, bands, the split's per-slot finding),
`design/7_Research/flat-world-fw2-2026-09-16.md` (`RenderView.topology`,
`Topology::height`, the moat, stratification), `design/flat-world-plan-2026-09-16.md`
§5 and §5a, the `pending FW-5` list at the end of
`crates/cubarium/tests/ring_present.rs` and its `#[ignore]`d test (FW-6's:
make them pass without editing them), `design/stratified-world.md`,
`design/appearance.md`. Fresh context. No nested agents.

## Objective

`ArtPresenter` built from the world's topology and cell count draws a ring
world: bands from `Topology::height` with `canopy_top = 0.67` (a config
default, plan §5), the horizon and floor at the rims, water and rain in
place, ground cover and plant columns chosen along `u` instead of per side
face, tall columns and Lanternjaw rigs continuous across the wrap, bodies
and care effects at any `u`; the cube's frames byte-identical to before.

## Where and what you own

Worktree `/home/wrysk/wryskware/cubarium/.claude/worktrees/tachyon-screen`,
branch `tachyon-screen`. You own `crates/cubarium/src/{present.rs,
art_present/**, lanternjaw/**, scene.rs}` and `crates/cubarium-core`'s
`canopy_top` config key if it does not exist yet (one field with a default,
validated 0..1; say so). FW-4 owns `sink/**, cli.rs, net.rs, run.rs,
runner/**, care/**, care_effects.rs` concurrently; GS-1's crate is not
yours. Commit ONLY as `git commit -m "..." -- <your paths>`, verify with
`git show --stat HEAD`; rebase onto the branch head as FW-4 lands; keep the
workspace compiling at every commit.

## Deliverable

1. `ArtPresenter::new(view/topology)`: caches sized from `cell_count()`,
   `GROUND_LATTICE`, `SEED_CELL`, slot tables and column choice generalised
   (cube path unchanged; ring: columns spaced along `u`, wrapping).
2. Bands: `band_of_height` takes `canopy_top`; on the cube the existing
   `h >= 1.0` rule must produce the same bands as today (pin it).
3. Rims: horizon fade and floor at `v = 0`/`v = h`; water pools and the
   moat drawn where the fields put them; rain marks anywhere.
4. Wrap: every stamp near `u = 0`/`u = w` lands on both sides through the
   canvas's unfold (FW-3's `Unfolds` cache handles the ring's images);
   tall columns and rigs that straddle the seam draw continuously.
5. Cube golden: the same-seed 20-frame hashes FW-3 recorded
   (`f24708a7806da988`/`aa872056423de435` at 3,000 ticks) unchanged.
6. Ring captures: `demo`/`run --sink png` at 320×180 and 640×360 (through
   FW-4's sink once it lands; until then a test-only PNG writer is fine),
   committed under `crates/cubarium/tests/golden/ring_*.png` as the new
   goldens with a short description of what is visible.
7. Optional, only if it does not touch the cube's bytes: the per-slot band
   rejection FW-3 named (skip a slot whose footprint cannot reach the band)
   with the measured split factor before/after.

## Verification you owe

FW-6's `ring_present` tests pass including the un-ignored one; the cube
golden; `cargo test --workspace --exclude cubarium-gpu` and clippy clean on
your files; the two ring PNGs. Commit small, trailer
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. No merge, no push.

## Return

Report at `design/7_Research/flat-world-fw5-2026-09-16.md` and as your final
message: what the ring capture shows (and what looks wrong, if anything, for
Wrysk), the `canopy_top` wiring, the cube proof, anything in §5 that proved
wrong.
