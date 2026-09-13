# Megafauna comparison studio

Two independent, code-native body studies for Wrysk to choose between. They do not
modify the Godot production atlas, founder configuration, or predation mechanics.
`root.js` is Codex's Veilwarden; `fable.js` is Fable's independent candidate.

Serve this directory on loopback, for example:

```sh
python3 -m http.server 7400 --bind 127.0.0.1 --directory art/studies/megafauna
```

Open `http://127.0.0.1:7400/`. Select rest/move/hunt/offspring, pause, or use the
plain background to compare silhouettes. `?mode=hunt&t=3.9` opens an exact paused
instant. `window.study.set(seconds, mode)` provides deterministic browser captures.
Each body draws in one native 64×64 canvas, copied to an unscaled reference; the
large view is only nearest-neighbour enlargement. Both use the same clock/anchor.

Candidate contract: export `candidate = {name, author, description, draw(ctx, args)}`
from a standalone ES module; args are `{time, mode, x, y}`. Face right; aim for
16–20px apparent body length and keep ordinary poses within x±12,y±10. Save/restore
canvas state; do not clear the canvas or use wall-clock/random state. Neither
candidate has yet passed production renderer footprint/seam validation.

The winner is not selected by code. A later owner-approved tie can use a stable
seed choice or retain the other design for a later lineage.
