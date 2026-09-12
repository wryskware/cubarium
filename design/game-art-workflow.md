---
design_status: leaning
last_reviewed: 2026-09-12
decision_refs: []
---

# Game-art workflow and the next visual slice

Wrysk's current instruction is to approach Cubarium development like a video
game, including sprite design, rigging and animation. They allowed Unity,
Godot or a Rust engine, and selected creature/habitat art as their preferred
personal contribution. This permits implementation work; it does not make a
particular engine, creature catalog or ecological mechanism canonical.

The [progress review](7_Research/progress-art-review-2026-09-12.md) identified
near-identical disc bodies, poorly matched turning/translation, weak life-event
expression and a habitat that reads as colored field cells. The existing live
viewer also runs at 8x; ambient pacing must be judged at normal speed.

**Implemented choice for this slice:** Godot 4.6.3 provides the art editor,
Sprite2D cutout hierarchy and native AnimationPlayer timelines. A display-free
baker evaluates the actual scene poses into native RGBA atlases. Rust loads the
pack, uses the existing continuous surface geometry for partial sprites, and
sends identical encoded face buffers through the established web/PNG/preview/
shim sinks. This integrates Godot authoring while retaining the current
simulation and display contracts. It is not a full runtime port.

Godot's [cutout workflow](https://docs.godotengine.org/en/4.6/tutorials/animation/cutout_animation.html)
and [AnimationPlayer](https://docs.godotengine.org/en/4.6/classes/class_animationplayer.html)
fit editable parts and poses. The [verified editor version](https://godotengine.org/download/archive/4.6.3-stable/)
can run the baker without a display. Unity remains an allowed runtime option;
[Bevy](https://bevy.org/) remains an alternative for a Rust engine. The
[Amethyst repository](https://github.com/amethyst/amethyst) is archived and
states that engine development halted, so it is not the implementation choice.

The working deliverable is in [art/README.md](../art/README.md): three
contrasting visual candidates, twelve rest/move/feed/bud clips, three
replaceable habitat motifs, an editor bake menu, command-line bake, a native
pose gallery, and a composed garden on the actual five-face renderer. The
creatures use pivot rigs; mesh skinning/IK is not implemented. Their larger
8–12-pixel silhouettes are an experiment in legibility relative to the older
3–7-pixel proposal, subject to the unchanged nine-pixel radial surface budget.

The garden explicitly choreographs 25 specimens and places scenery. The
gallery is an artist's inspection tool, not ecological evidence. None of these
visual types currently has its own diet or simulated capabilities; fern and
rosette graphics do not imply organism photosynthesis. The main M2 world and
its state are unchanged. The studio output is opt-in, and its controls remain
outside the ambient face images.

Wrysk can repaint the sprite parts or habitat SVGs, substitute ordinary PNG
textures, change pivots and key poses in the scene, then bake and review the
same assets on a cube. Source scenes and textures are authoritative for this
asset pipeline; baked PNGs and JSON are reproducible runtime inputs kept in
Git so a preview does not require the editor. The current exporter supports
full-texture Sprite2D cutouts and native property tracks, rather than every
Godot drawing feature. Its supported settings and limits are documented for
the artist.

**Proposed next slice:** choose or revise silhouettes from actual-size and
physical-cube observation. Connect selected clips to actual rest, displacement,
feeding and gestation progress, and habitat coverage to producer biomass.
Retune turning/noise by movement state with matched ecological runs; merely
freezing the artwork would conceal a controller problem. Choose how body art
varies with inherited traits after observing the candidate grammar. Preserve
the distinction between decorative art and organs with simulated effects.

Then evaluate whether varied descendants remain visible over time. E3's
founder bottlenecks mean more founder artwork alone cannot establish lasting
diversity. Keep lineage survival and ecological opportunity in the next
experiments. A full runtime port should follow a demonstrated benefit in
authoring or rendering, with shared geometry and the shim boundary preserved.

Validation for this slice: Godot editor import and project startup, headless
bake of the actual scenes, runtime loading of all clips, visible frame changes
for move/feed/bud, sprite alpha compositing, asymmetric seam sampling and
light conservation, the existing renderer/host unit suites, and visual review
of native PNG captures. Godot's window was not visually inspected; the baked
output was. Physical LED review and ecological integration remain outstanding.
