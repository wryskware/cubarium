# Conditional authored-coverage study

An isolated experiment, **not a new art-pack or renderer default**. See
[the evidence and disposition](../../../design/7_Research/astra-aa-comparison-2026-09-13.md).

The Godot helper extends the current exporter and compares its exact `raster`
against fixed 4×4 subpixel coverage of the same transformed cutouts. Both paths
retain the existing layer order and RGBA8 source-over compositor. Coverage is
resolved in linear premultiplied RGBA; the alternative does not repair the existing
encoded-RGB *inter-layer* blend convention. No jitter or temporal feedback.

The independent Cargo workspace depends only on the production render/surface
crates and the shim protocol. It does not compile the host, mutate ecology, or
communicate with any display. `net.rs` is reused from the host for capture layout.
All generated output directories must be new. Keep build output under `captures`.

```sh
godot --headless --path art --script res://studies/aa/bake_comparison.gd -- --out=/absolute/new/bake-directory
CARGO_TARGET_DIR=captures/build-cache/astra-aa cargo test --offline --manifest-path art/studies/aa/Cargo.toml
CARGO_TARGET_DIR=captures/build-cache/astra-aa cargo run --offline --release --manifest-path art/studies/aa/Cargo.toml -- /absolute/new/bake-directory /absolute/new/render-directory
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
