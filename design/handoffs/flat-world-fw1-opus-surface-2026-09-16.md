---
design_status: exploration
last_reviewed: 2026-09-16
---

# FW-1 (Opus, high): `Topology` and `Scale` in the surface crate

The first implementation package of the ring world. Read, in this order:
`design/flat-world-plan-2026-09-16.md` §2 (the contract you implement), §5a
(the embedding), §9's FW-1 row and its ordering notes, §4 only for what
FW-2 will need from you; then `design/surface-topology.md` and
`WORKING_POLICY.md`. The repair sections at the end of the plan are history;
where they disagree with the body, the body wins. Fresh context. No nested
agents.

## Objective

`cubarium-surface` gains a second topology, `Topology::Ring { w, h }`, and a
`Scale`, beside the cube, with the cube's outputs unchanged by value and the
existing tests green. FW-1's API is the freeze point for FW-2, FW-3 and FW-6,
so its public surface must be stable and documented when you report.

## Where

- Cubarium worktree `/home/wrysk/wryskware/cubarium/.claude/worktrees/tachyon-screen`,
  branch `tachyon-screen`. Run everything from there; never edit the main
  checkout. Other workers own other files: FW-0 owns `vendor/cube-proto/**`
  and `crates/cubarium/examples/render_bench.rs`; do not touch them.
- Files you own: `crates/cubarium-surface/**` **except** the reserved FW-6
  paths `crates/cubarium-surface/tests/{ring_travel,ring_field,ring_raster}.rs`,
  which you must not create.
- You will need to make the workspace compile after widening signatures.
  Changes outside the surface crate are allowed **only** as the minimal
  mechanical follow-through of a signature change (a `u8` → `u16` cast, a
  `Topology::Cube` argument threaded through an existing call, the
  `CUBE_CELL_COUNT` rename at cube-only sites); no behaviour change, no new
  feature, and list every such file in the report. `cubarium-surface-oracle`
  and `cubarium-search` stay cube-only (plan §1).
- Use graft (`graft skeleton`, `graft callers <symbol> --depth all`,
  `graft grep`) before editing; run `graft callers` on every public function
  whose signature changes.

## Deliverable (the plan's §2 is normative; this is the checklist)

1. `Topology { Cube, Ring { w: u16, h: u16 } }`, `Copy`, `Eq`, serde; the
   methods §2 lists (`charts`, `extent`, `cells`, `cell_count`, `embed`,
   `height`, `chord_sq`, `max_local_radius`, `validate`). `validate` refuses
   `w`/`h` not multiples of the cell size, `cell_count > u16::MAX`, and
   `w < 2·max_local_radius() + 2·CELL_PIXELS`.
2. `Scale` (cell pixels `4·S`, `world_scale` `S` ring-only with the cube
   pinned to `S = 1`, `footprint_radius() = 9·S`), owned here; FW-3 adopts
   the value, do not touch the render crate.
3. Pixel indices `u8` → `u16` everywhere the plan names, and `CELL_COUNT`
   → runtime `cell_count()`, keeping `pub const CUBE_CELL_COUNT: usize = 1280`
   for cube-only literals.
4. Ring travel: the vertical edge as a seam of the chart to itself through
   the existing `Some(seam)` branch (identity transport, translation by
   `∓w`); rims at `v = 0` and `v = h` through the existing `REFLECT_Y`;
   the existing lowest-`Edge` tie rule unchanged. Ring `downhill`: top row
   `None`, else `(cx, cy + 1)`.
5. Ring unfold: `chart_images` enumerates the direct and `±w` images,
   shortest wins; `unfold_pixels` keeps its exactly-once guarantee across
   the wrap; `FieldGraph` rows are rings (3,600 cells, 7,120 edges at
   320×180, top/bottom rows degree 3, no corners).
6. The isotropic cylinder `embed()` (§5a: `θ = 2π·u/w`, `r = w/(2π·32·S)`,
   `y_e = (h/2 − v)/(32·S)`, axis order matching the cube's up axis) and
   `height() = 1 − 2v/h` as separate methods; on the cube both are today's
   values.
7. Docs: the surface crate's module docs and `design/surface-topology.md`
   gain a ring section (the seam table row for the self-seam, the two rims,
   the four-corner tie outcomes).

## Verification you owe

- Every existing surface test green **unchanged**, plus a by-value check
  that the cube's `travel`, `unfold`, `unfold_pixels`, `FieldGraph` edges
  and `embed` outputs are identical before and after on a fixed fixture set
  (write the fixture from `main` before you change anything; keep it in
  `crates/cubarium-surface/tests/` under a name that is not reserved).
- Ring exercised at `S = 1` (320×180) and `S = 2` (640×360) from the first
  commit that introduces it.
- Exact-tie and near-tie fixtures at **all four corners**, whose resolutions
  differ under `Edge::Top = 0 < Right = 1 < Bottom = 2 < Left = 3`: top-left,
  top-right and bottom-left reflect first, bottom-right crosses the seam
  first. Also a step that wraps and reflects in one displacement, and a
  long displacement that wraps more than once.
- `cargo test --workspace` and `cargo clippy --workspace --all-targets`
  clean.
- These are your own tests; FW-6 writes the independent ones from the plan
  without reading yours.

## Constraints

- No behaviour change on the cube. No change to `cube_proto`. No feature
  beyond §2. No new dependencies.
- Commit small on `tachyon-screen`, message bodies explain why, trailer
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. No merge, no
  push. If FW-0's commits land on the branch while you work, rebase your
  local work onto them rather than merging.

## Return

Report at `design/7_Research/flat-world-fw1-2026-09-16.md` and as your
final message: the frozen public API (signatures, one line each), every
file touched outside the crate and why, the by-value cube evidence, the
four-corner outcomes as observed, test counts before/after, and anything
in §2 you found impossible or wrong as written (with evidence), since FW-2,
FW-3 and FW-6 are briefed against your freeze.
