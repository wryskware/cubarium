---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# 04 — Glasscane motion and residual flicker

Outcome: one clearly readable improvement at native 64 px, preserving the existing
look. Pose interpolation, sail body stabilization, calm fins, seven-species growth,
spire/vine sway and crown corner continuity are already delivered. Do not redo them.

Glasscane's authored art leaves limited wind headroom. Fable's study was interrupted;
its helpers are preserved, but no production candidate was selected. Blanket
coverage-AA softened the sail body and was not adopted. “Smooth everything” is not
a sufficiently specific target.

## First bounded milestone

Inspect the current glasscane animation at native resolution and identify one
concrete pop, clipping issue or unreadable bend. If it is real, make at most one
candidate using the existing Godot/code-native art workflow. Preserve the bulbs,
root anchor, stalk joins and the established nine-pixel footprint; verify the
current plant contract before editing. Do not use an image generator to replace
the established rigs or atlas pipeline.

Compare a short baseline/candidate sequence at 64 px and enlarged nearest-neighbor
scale, including loop boundary, wind reversal and seam placement. Keep only source,
the concise result and any explicitly useful tiny evidence, not a render archive.
Stop if the gain is not visible; record that result instead of making more variants.

## Starting sources

- [Plant contract](../../art/PLANTS.md).
- [Wind-room investigation](../7_Research/spiretree-wind-room-2026-09-13.md).
- [Study status](../../art/studies/glasscane-wind/STATUS.md) and nearby author/bake helpers.
- `crates/cubarium/src/art_present.rs`; isolated study source also survives at
  `checkpoint/source-glasscane-wind-frozen`.

Done: a justified accept/reject decision, native temporal checks and relevant atlas
validation. If accepted, commit only intended art/source output and deploy through
the current-checkout launcher. Browser readability does not prove panel readability;
leave a brief owner viewing question if that cannot be checked directly.
