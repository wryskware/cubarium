---
design_status: exploration
last_reviewed: 2026-09-11
decision_refs: []
---

# Local contracts inspected during planning

This is evidence from local files, not Cubarium policy. Sources were inspected
on 2026-09-11. Paths below refer to this workstation; recheck the source when
implementing. No external artificial-life literature review was performed.

## Display shim

Repository: `/home/wrysk/vuzic/led-cube-shim`, inspected at clean Git revision
`7a21b5f7c7a22ee36b4930471cdf1afaef45abdd`.

| Source | Observed contract |
| --- | --- |
| [README](/home/wrysk/vuzic/led-cube-shim/README.md) | Five 64×64 RGB buffers; daemon owns hardware output; Rust and Python clients exist |
| [Architecture](/home/wrysk/vuzic/led-cube-shim/docs/ARCHITECTURE.md) | Front=0, Right=1, Back=2, Left=3, Top=4; x right/y down from outside; Top has Back at its upper edge |
| [Geometry document](/home/wrysk/vuzic/led-cube-shim/docs/GEOMETRY.md) | Eight connected seams, four open bottom edges; heading rotation and edge reversal; cube embedding |
| [Geometry implementation](/home/wrysk/vuzic/led-cube-shim/crates/cube-proto/src/geometry.rs) | `Face::neighbor`, `cross_seam`, `rotate_heading`, `pixel_direction`, `Edge`, and `Seam` exist in source |
| [Frame/library implementation](/home/wrysk/vuzic/led-cube-shim/crates/cube-proto/src/lib.rs) | `Frame`, 61,440 RGB payload bytes, exported `CubeClient`, geometry exports, and face order |
| [Workspace manifest](/home/wrysk/vuzic/led-cube-shim/Cargo.toml) | Rust workspace includes `cube-proto`, `cube-map`, `cube-kms`, and daemon crates |

The geometry APIs already handle the discrete seam contract. `cross_seam` takes
an integer edge coordinate and returns an entry pixel and quarter-turn count;
it does not by itself integrate arbitrary continuous overshoot. Cubarium's
proposed additions are continuous movement, surface distances/neighborhoods,
field stepping, and seam-spanning organism rendering. This distinction prevents
rebuilding existing work while leaving a real simulation abstraction to create.

The source's rotation is counter-clockwise in image coordinates: one turn sends
`(dx,dy)` to `(dy,-dx)`. The top Right and Back seams reverse their along-edge
parameter. No hidden bottom surface is defined by the shim. The shim's example
particle routine lets a particle leave the bottom; a different ecological rim
policy in Cubarium is therefore a new proposal, not an existing shim behavior.

The shim owns physical panel adjustments, color correction, brightness/power
limits, and output transport. Cubarium should use its client rather than copying
protocol serialization. No cube output, calibration, shim edits, or shim test
execution occurred during planning.

## Lore

Repository reference: `/home/wrysk/wryskware/lore`.

- [Canon rules](/home/wrysk/wryskware/lore/design/0_Canon/README.md) define
  tentative-by-default authority, frontmatter states, and promotion rules.
- [Ledger](/home/wrysk/wryskware/lore/design/0_Canon/DECISIONS.md), D-0012 and
  D-0013, records opt-in profiles and accepted per-file decision syntax.
- [Vault skill](/home/wrysk/.codex/skills/lore-vault/SKILL.md) prescribes
  `.lore.toml`, one `0_Canon`, ledger grammar, research/scratch path ceilings,
  status verification, and agreement before installing agent instructions.
- [Search skill](/home/wrysk/.codex/skills/lore-search/SKILL.md) describes
  retrieval provenance and fallback to files when the local index is unavailable.

`lore --help` and subcommand help were inspected from the installed CLI. The
initial sandboxed status request could not reach localhost; a permitted local
status request confirmed that `cubarium` was not yet registered. This was a
network boundary followed by an unregistered project, not evidence that the
daemon was down. Cubarium's profile and authored plans can be registered and
checked after creation.

Cubarium was subsequently registered through `lore add` during this task. The
indexing-scope review found only authored plans and local Obsidian editor state;
`.gitignore` excludes that editor state and planned runtime/build outputs.
No extra `.loreignore` exclusion is needed: the design vault stays searchable.

Verification on 2026-09-11: `lore status --project cubarium` reported
`authority lore-v1 (annotate)` and `decisions 2/2 active`, with full embedding
coverage and no decision violations. A hybrid search for `persistent ambient
ecosystem` filtered to `decided` returned the owner brief and its accepted ledger
entry. A local document check also passed for the TOML profile, unique IDs,
frontmatter/reference validity, local Markdown links, fenced blocks, and the
single-canon constraint. This verifies the planning artifacts, not simulation
correctness or long-term ecological behavior.

Lore's own `AGENTS.md` was read as an example of vault conventions. Its unrelated
project constraints and Git workflow are not inherited by Cubarium. There was
no existing Cubarium ADR collection or second canon directory to migrate.
