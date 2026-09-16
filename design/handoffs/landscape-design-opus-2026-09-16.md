---
design_status: exploration
last_reviewed: 2026-09-16
---

# LP-A (Opus, high): from strata to landscape — a design pass

Wrysk, after seeing both on the panel (2026-09-16): the GPU renderer's
*synthetic* scene "was really great: a landscape with what looked like
hills, a moving rain column, smooth creature movement with floaty animations
as they move around and the trees swaying — that's more the direction I want
this to go. The last two scenes [the live ring world] look like a weird
confetti layer cake." The 32-px art scale is chosen. This pass explains the
gap precisely and turns the direction into buildable packages. Read-only
apart from the document. Fresh context. No nested agents.

## Read first

`crates/cubarium-gpu/examples/synthetic.rs` and `crates/cubarium-gpu/src/
scene.rs` (what the synthetic scene actually put on screen: how its fields
were shaped, how many instances per layer, how bodies moved, how rain was
placed, how it animated); `crates/cubarium-gpu/src/adapter.rs` (how the
live world's `RenderView` becomes a scene today); `crates/cubarium/src/
art_present/` via `graft skeleton` (the CPU presenter's slot rules,
`RANK_FULL`/`RANK_MID`, band choice, column choice, the wind) and
`design/7_Research/flat-world-fw5-2026-09-16.md` (what the ring presenter
chose and what it flagged: dense, columns slender, crowns on one row);
`crates/cubarium-core/src/habitat.rs` (light/moisture from `height`, the
noise, the weather blobs), `design/stratified-world.md`,
`design/flat-world-plan-2026-09-16.md` §5, §5a, §8 (biomes, separable);
`design/appearance.md`, `design/game-art-workflow.md`,
`design/backlog.md`; the golden PNGs `crates/cubarium-gpu/tests/golden/
synthetic-*.png` and `crates/cubarium/tests/golden/ring_*.png` (look at
them side by side).

## Questions the document must answer, with evidence

1. **What made the synthetic scene read as a landscape?** Enumerate the
   concrete differences from the live world as rendered: terrain (a height
   profile along `u` versus flat bands from `height = 1 − 2v/h`), instance
   density per layer (count both), size and spacing of plants and columns,
   how bodies moved (continuous position, easing, bob) versus the live
   world's 20 Hz tick with interpolation fraction `f`, how rain was placed
   (a column/band versus the spherical caps on the cylinder), colour and
   value contrast between layers, the wind.
2. **Terrain along the ring.** Propose a ground profile `ground(u)` (a
   low-frequency seeded function, optionally authored) so that height
   becomes distance above the local ground: `height(u, v)` replaces
   `1 − 2v/h` for light, moisture, bands, `downhill`, and the controllers'
   height channel. State where it lives (a `Topology::height` that takes a
   profile, or a habitat-level field) with the trade-offs: the plan's
   `downhill` (`cy + 1`) becomes "toward lower ground", which changes the
   field graph; the canopy row is no longer a row. Say what the cube keeps
   (nothing changes there) and what the ring's schema needs (a fresh world).
   Give the visible effect: hills and valleys, soil following the profile,
   water pooling in valleys instead of a moat along the floor.
3. **Density and composition.** Which knobs turn the confetti into a
   scene: slot ranks, founder density, plant column spacing, tall crown
   height variation (the "all crowns on one row" finding), and what the
   synthetic scene's counts were. Recommend values for 640×360, S = 2, and
   say which are config, which are presenter constants, which are world
   scale.
4. **Weather that reads.** Why the synthetic rain column read and the
   cylinder-cap showers do not (shape, edge, movement, contrast), and the
   cheapest change that gives a moving column: cap shape, a rain intensity
   ramp, the renderer's rain layer, or the weather model's axis.
5. **Motion.** What "smooth, floaty" needs: continuous body positions
   between ticks (the interpolation exists; is it used by the adapter?),
   heading smoothing, a vertical bob for gliders/skimmers, easing; and the
   trees' sway (item 2 of GS-1c gives the budget; what amplitude and period
   the synthetic scene used).
6. **Packages.** Ordered, bounded, with owners: which changes are
   renderer/adapter only (no schema), which are presenter constants, which
   are habitat/topology (fresh world, schema bump), which belong to FW-7
   (art) and FW-8 (biomes). Each with a verification (a capture or a panel
   look) and an estimate. The first package should be the one that most
   changes the look for the least risk.

## Return

`design/landscape-plan-2026-09-16.md` committed on `tachyon-screen`
(path-only commit, trailer `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`),
under 2,500 words plus tables, a 10-line summary at the top, and the same
summary plus the package table as your final message. Do not edit source.
No device access. Cite file:line for every claim about what the code does.
