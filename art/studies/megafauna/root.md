# Veilwarden — Codex candidate

A canopy sentinel with a plated abdomen, small forward head, and hooked forelimbs.
It should read as an individual watching the habitat, not a bigger version of a
common grazer. The long body and negative space under its neck distinguish it from
the existing rounded lantern, mossback, sail, and skimmer rigs.

Native body length is about 20 pixels, compared with the gallery's 8-pixel common
creature reference. The palette reuses the quiet blue/seafoam/plum language; the
single warm eye gives orientation without making the whole animal glow brightly.

Rest is mostly still: subtle plate breathing and planted feet. Move uses a slow
tripod gait. Hunt is a single forelimb reach during an eight-second study loop, then
recoil and rest. Bud shows one small hanging cocoon. These are animation proposals,
not implemented attacks, kills, fertility, or a funded offspring.

The study is deliberately separate from the production Godot atlas. Its whole body
exceeds the renderer's current per-stamp radius in some poses; integration would
need a bounded multipart rig with seam/registration tests, not scaling past the
existing limit. Canvas subpixel coverage is not evidence of the current Godot bake
quality. Review the 64×64 native view before choosing. Nothing is auto-selected.
