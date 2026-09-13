# Conditional authored-coverage study

The original isolated experiment was **not a new art-pack or renderer default**. See
[the evidence and disposition](../../../design/7_Research/astra-aa-comparison-2026-09-13.md).

The Godot helper extends the current exporter and compares its exact `raster`
against fixed 4×4 subpixel coverage of the same transformed cutouts. Both paths
retain the existing layer order and RGBA8 source-over compositor. Coverage is
resolved in linear premultiplied RGBA; the alternative does not repair the existing
encoded-RGB *inter-layer* blend convention. No jitter or temporal feedback.

The default independent Cargo workspace uses only production render/surface
crates and the shim protocol; it does not compile the host or mutate ecology.
The optional `meals` feature compiles host/core for paired copied-world captures,
stepping one local world and drawing two art packs. Neither mode communicates
with a display. `net.rs` is reused from the host for capture layout.
All generated output directories must be new. Keep build output under `captures`.

```sh
godot --headless --path art --script res://studies/aa/bake_comparison.gd -- --out=/absolute/new/bake-directory
CARGO_TARGET_DIR=captures/build-cache/astra-aa cargo test --offline --manifest-path art/studies/aa/Cargo.toml
CARGO_TARGET_DIR=captures/build-cache/astra-aa cargo run --offline --release --manifest-path art/studies/aa/Cargo.toml --bin aa_comparison -- /absolute/new/bake-directory /absolute/new/render-directory
node art/studies/aa/browser_capture.mjs /absolute/new/render-directory
```

The last command starts and closes its own headless Chromium with a **new** profile
inside that render directory; it never attaches to an existing browser. Opening
`viewer.html` directly also works. Native canvases are exact output bytes; enlarged
copies use nearest-neighbor display scaling. Scene changes select matched rooted,
translated/rotated, 0.6-scale, seam, rim, vertex, and held-pose sequences. The
six-second excerpt has a deliberate replay cut (not every source clip divides six
seconds); the metrics exclude that artificial wrap.

Metrics integrate all five faces, not a crop. Alpha is measured by a white-RGB
copy of each sprite with the same alpha and the same production stamp. The root
band metric is meaningful only for the fixed native plant placement. Lower
first/second differences can mean blur, so also inspect peak light, alpha area and
the >=0.5-alpha silhouette. The seven scene benchmarks alternate paired batches;
they exclude clear/encode/IO and are per-stamp timings, not a world-frame budget.

Five focused tests pass: three study checks plus the two reused net-layout tests.
Each full run additionally asserts finite bounded light, nonempty visible coverage
for every frame, valid sprite extents, no travel fallback, and byte-exact held poses.
This is representative geometry coverage, not an exhaustive topology proof.

Frozen evidence uses Godot 4.7.2, the source blobs listed in the report, and exact
production atlas rows lanternstalk 11, reedspire 34, sail 5, skimmer 13. The baseline
study strips were independently byte-compared with those shipped RGBA rows.

## Sail-only follow-up

`bake_sail.gd` is a separate, fixed candidate: only `LeftFin/Sprite` and
`RightFin/Sprite` get 4×4 coverage. The known `Body/Sprite` and `Bud/Sprite` are
point-baked and composited above the resolved fins, preserving opaque highlights
exactly. Their original animation tracks still run. Uniform coverage blocks are
copied exactly, avoiding unnecessary color conversion/truncation. This override
was opt-in in that study; at commit8c0b40a the original whole-sprite comparison
reproduced its previous eight strips and 32 rendered sheets pixel-for-pixel.
Use that historical commit for the following historical-source commands.

```sh
godot --headless --path art --script res://studies/aa/bake_sail.gd -- --out=/absolute/new/sail-bake
CARGO_TARGET_DIR=captures/build-cache/astra-aa cargo run --offline --release --manifest-path art/studies/aa/Cargo.toml --bin sail_comparison -- /absolute/new/sail-bake /absolute/new/sail-render
node art/studies/aa/browser_capture.mjs /absolute/new/sail-render
```

The sail viewer adds adult/0.6 scale and all 12 directed inter-state blends. Bud
sampling clamps to the last frame; it never wraps as a loop. Inter-state blends
are 0.3 s sampling fixtures, not an ecology or meal-controller replay. Native/seam/
rim/vertex, quiet frames, peak light and coverage are measured for all four states.
The source bake asserts exact baseline agreement with shipped rows, preserved
visible body/bud pixels, and actual rig endpoints. See the
[sail-only report](../../../design/7_Research/astra-sail-aa-study-2026-09-13.md)
for the mixed disposition, including the remaining point-baked body squash.

## Stable sail source candidate

See [the stable-body + fin4 report](../../../design/7_Research/astra-sail-stable-body-2026-09-13.md).
This is an explicit source change plus the measured fin-only policy, not another
filter candidate. Original pack is preserved at `captures/sail-stable-2026-09-13-original-pack`.
The historical fin-only script now rejects an opted-in new rig rather than
mislabeling its candidate output as an original baseline.

```sh
godot --headless --path art --script bake.gd -- --out=NEW_PACK_ABSOLUTE
godot --headless --path art --script studies/aa/bake_sail_stable.gd -- --baseline=ORIGINAL_CREATURES_PNG_ABSOLUTE --out=NEW_BAKE_ABSOLUTE
godot --headless --path art --script studies/aa/verify_sail_pack.gd -- ORIGINAL_PACK_ABSOLUTE NEW_PACK_ABSOLUTE REPORT_ABSOLUTE
CARGO_TARGET_DIR=captures/build-cache/astra-aa cargo run --release --offline --manifest-path art/studies/aa/Cargo.toml --bin sail_comparison -- NEW_BAKE NEW_RENDER
CARGO_TARGET_DIR=captures/build-cache/astra-aa cargo run --release --offline --manifest-path art/studies/aa/Cargo.toml --features meals --bin sail_meal_comparison -- WORLD ORIGINAL_PACK NEW_PACK NEW_MEAL_CAPTURE
godot --headless --path art --script studies/aa/sail_meal_sheet.gd -- NEW_MEAL_CAPTURE_ABSOLUTE
node art/studies/aa/package_sail_meals.mjs NEW_MEAL_CAPTURE
node art/studies/aa/browser_capture.mjs NEW_MEAL_CAPTURE
```
