---
status: open
date: 2026-09-21
owner: Fable (orchestration); one Opus worker, high effort
---

# Package 0: the physical encounter contract, and an edible-stock observer

## Why

The systems audit (`design/7_Research/organism-systems-audit-2026-09-21.md`,
§2 "Seeded and six-hour edible fraction", §10 order 0) found that no
diagnostic can say what share of the standing foliage a browser could actually
eat: the census prints counts and pooled totals, the plant autopsy steps no
fauna and knows no preset, the founder autopsy measures dying bodies against
the nearest crown. So "low food is absent / unreachable / unseen / not
acquired" is undecided after four diagnoses. This package writes down the
physical encounter contract the later packages implement, and builds the
observer that measures against it, on today's model and on the decided mouth.

Decisions in force: `design/handoffs/voxel-organism-decisions-2026-09-21.md`
(read all nine). This package implements none of them in the simulation; it
measures against §1, §2 and §6 as hypotheses.

## Deliverable A — the contract, as a document

`design/voxel-encounter-contract-2026-09-21.md` (`design_status: proposal`),
short, in metres and seconds. It defines, for a body at a pose on a support
face:

- **Body**: adult length/width/height per lineage (decisions §1), the growth
  convention (length ∝ (body / body_max)^(1/3)), the standing surface datum
  (`Site.y` is the support cell; the surface is `(y + 1) · voxel_m`; audit §1),
  footprint and clearance.
- **Anchors**: eye height (0.8 × body height), mouth band `[0, 1.33 × body
  height]` and horizontal reach 0.25 × length ahead of the footprint
  (decisions §2, §6), contact receptor positions.
- **Encounter**: one query giving, for every stand and pool within a radius,
  its physical foliage slab(s) (today: the crown disc, one cell thick at
  `crown_voxels(wood)` above the support cell, radius `crown_radius(wood)`,
  in metres), whether the mouth band intersects it, whether any ray from the
  eye within the fan reaches it unoccluded, and whether the body can stand on
  a face from which the band intersects it. Ground pools have a physical
  height from their volume; a pool is an occluder only above that height.
- **Discretisation**: how each physical quantity is converted to cells at the
  consumer (rounding rule stated once), and the invariance claim: the same
  encounter within one cell of error at 0.125 m and 0.25 m.
- **What today's code does instead**, per item, with file:line (audit §1
  table is the starting inventory).

## Deliverable B — the observer

A new example `crates/cubarium/examples/voxel_edible_stock.rs` (reusing
`voxel::ambient_world` and the `preset=<name>` path the autopsy uses; see
`voxel_founder_autopsy.rs`), run as

```
cargo run --release -p cubarium --example voxel_edible_stock -- HOURS preset=small
```

It steps the shipped world with the built-in founders exactly as the census
does and, at t = 0 and every 30 simulated minutes, prints one CSV block:

1. **Stock**: per species, stands, total foliage organic, and foliage in
   each of: (a) *reachable*: inside today's implemented mouth acceptance from
   some legal standing face (use `mouth_foliage_stand(s)` / the acceptance
   rule in `body.rs`, not a re-derivation) — **and separately** inside the
   decided band `[0, 1.33 × body_height]` from some legal face; (b)
   *visible*: some eye position on a legal face within 2 m sees it unoccluded
   with today's fan, and separately with the decided fan (pitches −40..+40,
   eye 0.8 × body height); (c) *route-connected*: the legal face in (a) is in
   the same walkable component as at least one living browser (component by
   the founder motion rule: support faces at the same standing height
   adjacent, plus whatever step the rule allows — read `body.rs` and the
   step rule, do not invent one; the terrain crate's `walk` module has the
   traversal used by its tests). Report each as organic and as a fraction of
   total foliage.
2. **Per lineage**: browsers alive, their standing-height distribution, mean
   distance to the nearest *reachable* stand (not the nearest crown), and
   the share of browser bodies with any reachable stand within 2 m.
3. **Detritus** (for the shredder, decisions §3): litter, carrion and glowcap
   cap organic, each with the share lying in the shredders' walkable
   components.

Also a summary at the end: the t = 0 and final rows side by side, and the
residuals of both ledgers.

Run it for 6 h on `small`, `default` and `wide` (parallel processes, all
cores) and put the table in a new section **"Edible stock, 2026-09-21"** of
`design/7_Research/voxel-census-2026-09-20.md`: at t = 0 and 6 h per preset,
reachable / visible / route-connected fractions under today's rules and
under the decided band and fan, and the one sentence the numbers support
about which link of the chain is broken on each preset.

## Constraints

- Read-only on simulation semantics: no change to any controller's inputs,
  `body.rs`, `manifest.rs`, `senses.rs` readings, flora, water, seeding. You
  may add `pub` query helpers (a stand's physical slab in metres; a legal-face
  enumerator; a ray test parameterised by eye height and pitch set) so long
  as the existing readings are byte-for-byte unchanged and a ≤200-tick test
  says so where a helper is derived from a live path.
- No knob tuning; no golden hashes; new tests ≤200 ticks; fresh worlds only.
- Worktree `.claude/worktrees/edible-stock`, branch `edible-stock` from main
  at 35ea0f3; `CARGO_TARGET_DIR` inside it. Explicit-path commits only; every
  commit ends with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Never touch `design/handoffs/README.md`, `design/README.md`,
  `config/tachyon/*`, `scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`.
- Use `graft ask "<question>" --source` before opening files. No windows.

## Verification

`cargo nextest run -p cubarium-voxel-fauna -p cubarium-voxel-flora -p
cubarium` green; the observer's t = 0 stock totals equal the census's
seeded totals on the same preset and seed; residuals ≤ 1e-9; Fable re-runs
one preset.

## Return (≤40 lines)

The table; the sentence per preset; how you computed route connectivity and
legal faces (cite the rule you reused); the commit list; what you could not
measure and why.

## Integration note (Fable, 2026-09-22)

Landed 59c2c2a..5e9892c, rebased onto main after the shelf-landform merge
(45eb645); 2105 tests green. Contract: `design/voxel-encounter-contract-2026-09-21.md`.
Observer: `voxel_edible_stock HOURS preset=<name>`. Research section "Edible
stock, 2026-09-21" in the census note (measured on 7dba001's landforms).

Fable re-ran t = 0 on the rebased main (shelf landforms), which moved the
stock numbers but not the finding:

| t = 0 | small | default | wide |
| --- | --- | --- | --- |
| foliage reachable today, from any legal face | 0.49 | 0.72 | 0.75 |
| reachable under the decided band | 0.42 | 0.50 | 0.49 |
| **route-connected to a browser, today** | **0.40** | **0.25** | **0.35** |
| bloomcrown route-connected | 0.00 | 0.00 | 0.21 |
| shredder founders' distinct standing heights | 8 | 7 | 7 |

The broken link is the route. `advance_candidate` (body.rs) refuses any
centre column that is not a support face at the body's own standing layer
and nothing ever writes `site.y`: **a founder cannot step up or down one
voxel**. On terraced landscapes every founder is confined to the level
component it was seeded on; the eight shredders start alone on eight
heights; the bloomcrowns that hold most of the foliage share no component
with any browser. (The legacy species motion rule has `climb`; the founders
never got one.) Visibility is not the limit: today's fan and the decided fan
see the same foliage on every arm. The decided band would cut reach on the
0.25 m presets because bloomcrown's one-cell slab sits just above 0.25 m;
that is the layers package's business (a basal rosette), not the band's.

Worker's design call kept: `Mouth::decided` uses the ladder's 0.1875 m width
(decision §1's proportions). Unmeasured, by construction: within-stand
shares (one slab), juvenile geometry (no body height in the model), pools
as partial occluders (no volume→height convention), shredder intake on the
three-food diet. Residuals at 6 h: fauna ≤ 1.8e-10, flora ≤ 2.7e-8 absolute
(≤ 1e-9 relative), water ≤ 2.1e-6 m³ over 432k ticks.
