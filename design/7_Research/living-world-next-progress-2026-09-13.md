---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Living-world next packages — progress record (Fable orchestrator)

Evidence for root's reconciliation, not decisions. Authorization:
`living-world-next-handoff-2026-09-13.md`; work order:
`living-world-next-brief-2026-09-13.md`; geometry advisories:
`astra-lanternjaw-production-plan-2026-09-13.md` and
`astra-lanternjaw-brief-review-2026-09-13.md`. Nothing here touches the simulation, saved
worlds, founders, the shim, `state/`, live processes or the no-art image. No deployment.
Wrysk reaffirmed the Lanternjaw choice; that selects the body, not a predator ecology.

Concurrent work in the same tree during this session — root's `runner.rs`, `sink/web.rs`
and `Cargo.toml`, Astra's `care_effects.rs`, the accounting session's `cubarium-core`
(schema 9) — is **not** in any commit below; every commit names its paths explicitly and
was serialized on `/tmp/cubarium-shared-care/git.lock`.

## Status

| package | state | commits |
| --- | --- | --- |
| Reed on a flooded top-face cell bends | delivered, tested | `b8b8a11` |
| Authored growth: nine side-species clips, pack v5 rebaked | delivered, tested, strips inspected | `5d7ea69` |
| Lanternjaw production rig, study route, captures, cost | delivered, tested, captures inspected | see "Commits" |
| Canopy top-down opening (umbrellafrond, bloomcrown) | **not authored** — the radial reveal mask remains | — |

## Package 1 — Lanternjaw production rendering

### Geometry (revised after Astra)

The first draft carried each part to its own anchor with `travel` and a rim re-pivot.
Astra's plan and follow-up review showed, with numbers checked against the compiled
surface crate, that two independent stamps near a top vertex read the same physical pixel
at two different body coordinates (Front (63, 1), heading (1, 0), part offset (3, 0),
pixel Top (63, 63): body (0.5, −1.5) from the root, (1.5, −1.5) from the carried anchor), so
a joint tears or a texel doubles, and that a travelled attachment reflects at the rim. The
shipped geometry is therefore **one root-owned `unfold_pixels` query per body**:

- `cubarium_render::stamp_rig(canvas, root, heading, states, opacity, scratch)` with
  `RigPart { sprite, offset, layer }`. The query radius is `rig_radius` = max over the
  active parts of `|offset| + sprite.extent()` plus `RIG_MARGIN` (0.5, covering the gap
  between the radial extent and a texel's true bilinear corner), bounded by the surface's
  `MAX_LOCAL_RADIUS` of 32; nine pixels stays the per-sprite material budget. A radius past
  32, or a non-finite one (a non-finite offset makes it NaN on purpose), panics in every
  build — a configuration error, never silently clamped (Astra §6.2).
- Parts with equal `layer` are one **material** and are summed (a hull cut into
  lattice-aligned pieces reconstructs the uncut hull exactly, because bilinear sampling is
  linear); layers composite source-over in ascending order; states are composed completely
  and then mixed, and opacity is applied once to the assembled body.
- The open rim needs no rule: the root query has no pixels past it, so a hanging part is
  cut where the surface ends. A top vertex shows the single localized cut any stamp shows.
- `Sprite::from_premultiplied`, `sample_at`, `can_reach`, `pivot`, `texel` were added;
  `stamp_rig_with_radius` is the doc-hidden generous-query oracle for tests.

### The body (`crates/cubarium/src/lanternjaw.rs`)

Code-native, eight per-frame rasterized parts (`FarLimb` 0, `Underside` 1, `Tail`/`Abdomen`/
`Thorax`/`Head` 2, `Glow` 3, `NearLimb` 4), the hull cut by template column with integer
offsets and pivots so all pieces share the body lattice. The study's template, rhythms,
envelopes, limb poses and colours are ported verbatim; colours are computed in sRGB exactly
as `fable.js` computes them and decoded once. The opaque structure keeps the study's mixes
toward `#0B0525` as opaque colours (Astra: dark shell and seams are intended); the fan,
lantern halo/glow, cocoon and far limb carry real alpha. Every position is unrounded (wave,
coil, lunge, limb joints, legs, anchor); the study's binary leg lift is the continuous
bump `sin(π u / 0.38)`. Not in the atelier pack; no fifth form; no founder; nothing in
`art_present.rs` selects it. `Mode::{Rest, Move, Hunt, Bud}` are choreography names only.

### Study route and captures

`cargo run --release -p cubarium --example lanternjaw_study -- --scene gallery|seams|walk
--ground black|study|soil|water --sink web|preview|png|shim [--web-port 7399] [--seconds N
--every K --out DIR]`. The gallery draws the four modes on Front beside the common
atelier lantern rig for scale; `seams` places a body across the Front/Right seam, one
crossing Right→Top, one rotating at a Top vertex, one heading into the bottom rim in `hunt`
and one facing −x on Back; `walk` travels a body through several seams at 1.5 px/s with a
`hunt` body on Top. Native PNG frames were captured for 12 s per scene and assembled into
×4 nearest-neighbour sheets: `captures/lanternjaw/{gallery,seams,walk,grounds}.png` and
`captures/lanternjaw/gallery.mp4` (12 s, 30 fps). Fable's reading of them: the hull, the
cyan chain, the fan and the folded limbs read at native scale and the body is about twice
the common rig; the strike shows in the 3.3–3.5 s frames with the warm accent; the seam
bodies are continuous across Front/Right and Right/Top; the rim body is cut at the bottom
edge, not reflected; the vertex body shows the documented localized cut and no doubled
hull; over soil and water the dark structure reads as a silhouette with the chain on it.
Desktop captures do not replace observation on the cube.

### Cost (release, this machine, with root's 12-hour runs active)

`cargo test --release -p cubarium --test lanternjaw_cost -- --ignored --nocapture`:

| place | mode | bodies | mean µs | worst µs |
| --- | --- | --- | --- | --- |
| mid-face | move | 1 | 41.6 | 246.4 |
| mid-face | move | 2 | 85.0 | 261.3 |
| mid-face | hunt | 1 | 39.9 | 46.3 |
| mid-face | hunt | 2 | 82.5 | 116.4 |
| seam | move | 1 | 49.0 | 53.9 |
| seam | move | 2 | 102.9 | 192.0 |
| seam | hunt | 1 | 49.5 | 74.5 |
| seam | hunt | 2 | 107.7 | 191.2 |

On a dense art-mode frame (200 organisms, mature fields, water, rain) the mean was
13.0 ms with 0, 1 or 2 bodies in Fable's run (increments −18 µs and −10 µs, inside the
noise of a loaded machine); the worker's single-threaded run measured 12.87 ms alone,
+146 µs for one body and +224 µs for two. The slice-2 figure for a comparable frame on a
quiet machine was 11.45 ms, so the baseline, not the rig, is what moved. A Lanternjaw
costs on the order of 0.3–0.7 % of the 16.7 ms budget; the worst single frames are
scheduler noise, not the rig.

Two implementation conventions worth knowing: a study coordinate names a *pixel*, so every
template cell is splatted at +0.5 on both axes (the hull is then symmetric about the
anchor, an integer anchor is the crisp phase, and `BOUND_BACK`/`BOUND_FRONT` are exactly the
fan's and the claw's texels); and `captures/` is gitignored, so the sheets and the mp4 are
kept on disk at the paths above and are not in any commit — regenerate them with the
example if they are gone.

### Tests

`crates/cubarium-render/tests/multipart.rs` (15) and `crates/cubarium/tests/lanternjaw.rs`
(16), written by an independent worker from the doc comments and the brief, including
Astra's retained fixtures: the marked-partition vertex case reads 0.5, not 0.75; the rim
fixture keeps Front (32, 59) as the flat placement does and no part pops; a one-part rig
equals `stamp_layers_bent_with_radius` at its own radius bit for bit (the plain
`stamp_sprite` differs only by the filter tail its legacy radius clipped — Astra §6.1);
opaque and translucent materials cut into two and four aligned pieces draw the uncut
sprite; layers order; state mixtures; seam light conservation to 1e-6 with matched lattice
phase; all four top vertices from each incident face; radius identity against 24; the
eight parts and layers; the lattice rule; footprint, `PART_EXTENT_MAX` 8 and
`rig_radius ≤ QUERY_RADIUS_MAX` 16 over 12 s at 60 fps in all modes; purity; hunt
periodicity; the warm accent only inside the strike envelope; blink steps; near limb over
the hull and far limb under it at the strike; a per-frame bound in `move`; heading (−1, 0)
is a half turn, not a mirror (Astra §5); sub-pixel anchor property; opacity applied once
without ridging an overlap.

## Package 2 — Authored growth expansion

Commit `5d7ea69`. Nine hand-authored 4 s `grow01`/`grow12` clips for glowcap, rootveil,
lanternstalk (1→2 new), tendrilfan and reedspire, each in its own hidden `Grow<from><to>`
group reusing the stages' textures; `art/plants/author_grow.py` is the splice script;
the `.tscn` text is the source of truth. Pack stays v5 (34 plant rows, was 25). The bake
is byte-reproducible; every pre-existing stage/fruit row is byte-identical to the previous
atlas; the other four atlases are unchanged files; every species' wind budget and
limiting frame are unchanged. Conventions and per-clip descriptions are in
`art/PLANTS.md` "The ten authored steps": endpoints are RESET-neutral poses (for the two
rotating-sway species no loop sample has both pivots at rotation 0, so the presenter's
12 % endpoint blends carry the join — the pilot's stricter loop-sample property holds for
lanternstalk 0→1 only); every frame paints tile row 14 and never row 15 (the brief's
one-row root change for lanternstalk was a misreading — `stalk2` is nine rows, 6–14);
berries never appear; nothing fades to near-nothing before its replacement is up (a
correction Fable made after the first strips showed the 1→2 lantern vanishing to a dot).

Inspected: 8× strips of all ten clips in `/tmp/growth-strips/` (not committed; regenerate
from the atlas rows). Tests: `crates/cubarium/tests/art_growth_pack.rs` (11, generic over
every transition), `art_growth_clip.rs` adapted (9), the loader lists updated;
`cargo test -p cubarium` green at commit time.

## Reed on the top face

Commit `b8b8a11`. `slot_wind` turns a top-face slot in place only when its species has
`spin_deg > 0`; a reed in a flooded canopy cell bends along its own tile's horizontal axis
like a side-face plant, rooted at its ripple row. `crates/cubarium/tests/art_wind_top.rs`
(5). `art/README.md` "Wind" documents it.

## Commits, commands, artefacts

- `b8b8a11` reed rule; `5d7ea69` growth pack; Package 1 and the design notes: see the
  final entries appended below.
- Validation: `cargo test -p cubarium-render -p cubarium` (every suite green at each
  commit); `./scripts/art-bake.sh` twice with `cmp`; the ignored cost test above; the PNG
  captures above.
- Artefacts: `captures/lanternjaw/*`, `assets/atelier/{pack.json,plants.png}`,
  `art/plants/author_grow.py`.

## Limitations and next work

- **Canopy opening is not authored.** umbrellafrond and bloomcrown keep the radial reveal
  mask; a top-down "opening" convention (petals/frond scaling about a fixed centre) is the
  named remainder of package 2.
- The Lanternjaw is a body, not an animal: no presenter wiring, no mode cross-fade in the
  live world (the `stamp_rig` state mixture exists for it), no founder, no ecology. The
  next package is the paid-predator experiment on copied worlds (pursuit, attack, meals,
  digestion, single offspring, prey recovery, conservation audits) before any live
  introduction — `astra`'s fixed-hunter plan is the candidate recipe.
- The hunt strike is a 6 s scripted loop for the study only; real attacks must be
  event-triggered with their own progress.
- Colour mixes are the study's sRGB mixes; on the cube the composite over real ground is a
  linear-light source-over, so translucent parts differ slightly from the browser study.
  Hardware legibility at room distance is not established by these captures.
- The worst-frame cost figures were measured on a machine running root's 12-hour care
  experiments; re-measure on a quiet machine before quoting a distribution.
- Everything in the handoff's "Remaining full-goal scope" stays open: feed deposits, rain
  aftermath, cleanup acknowledgement, creature/plant responses and rituals, stronger
  tall-plant wind, coverage-AA comparison, matched longer care/autonomous runs, tunable
  input dose, the predator ecology, persistent plant age, droplets and nibbles.
