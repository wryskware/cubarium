# Vine wind: study-only endpoint retile

An isolated comparison against **9cf0e1d**, not a production art pack switch.
Read [the disposition](../../../design/7_Research/astra-vine-wind-study-2026-09-13.md).
The candidate pack **requires its companion renderer patch and derived endpoint**;
installing its `tall.png` alone loses visible material at full growth.

`bake.gd` clears only the extreme rows of a copied vine source image. `main.rs`
derives a four-row phase-matched endpoint once at load and temporarily represents
it as the vine's `cap`. `renderer.patch` is the exact49-insertion isolated change:
ordinary eight-row vine strips plus an endpoint with the original tile's owning
chart and a recentered, still nine-pixel-bounded support. It also exposes the
private column draw function for the study. It is **not a hardened public API**.

## Reproduce

All commands start at repository root. These explicit worktree/output paths must
be new (the current reviewed copies already exist). Never repurpose a release
worktree. No whole-cube supersampling, ecology, renderer default or live process
is changed.

```sh
flock /tmp/cubarium-shared-care/git.lock git worktree add --detach captures/build-cache/vine-wind-frozen 9cf0e1d
git -C captures/build-cache/vine-wind-frozen apply ../../../art/studies/vine-wind/renderer.patch
cp -a captures/build-cache/vine-wind-frozen/assets/atelier captures/vine-wind-original-2026-09-13
godot --headless --path art --script studies/vine-wind/bake.gd -- --out=/home/wrysk/wryskware/cubarium/captures/vine-wind-clear-ends-2026-09-13
godot --headless --path art --script studies/vine-wind/verify_pack.gd -- /home/wrysk/wryskware/cubarium/captures/vine-wind-original-2026-09-13 /home/wrysk/wryskware/cubarium/captures/vine-wind-clear-ends-2026-09-13 /home/wrysk/wryskware/cubarium/captures/vine-wind-pack-verification.json
CARGO_TARGET_DIR=captures/build-cache/vine-wind cargo test --release --offline --manifest-path art/studies/vine-wind/Cargo.toml -- --nocapture
CARGO_TARGET_DIR=captures/build-cache/vine-wind cargo run --release --offline --manifest-path art/studies/vine-wind/Cargo.toml -- captures/vine-wind-original-2026-09-13 captures/vine-wind-clear-ends-2026-09-13 captures/vine-wind-retile-2026-09-13
CARGO_TARGET_DIR=captures/build-cache/vine-wind cargo run --release --offline --manifest-path art/studies/vine-wind/Cargo.toml -- captures/vine-wind-original-2026-09-13 captures/vine-wind-clear-ends-2026-09-13 --measure
cp art/studies/vine-wind/viewer.html captures/vine-wind-retile-2026-09-13/viewer.html
node art/studies/vine-wind/browser_capture.mjs captures/vine-wind-retile-2026-09-13
```

The bake uses tracked `art/` from the reviewed9cf0e1d source; if that source has
changed, use an isolated copy of that revision's Godot project and copy this
study's two `.gd` scripts into its `studies/vine-wind/` directory first. Do not
silently accept a different baseline. `verify_pack.gd` rejects every changed
unrelated atlas pixel/file. Godot's sandboxed user-log warning did not prevent
the reviewed bake or verification (both exit0).

Rust dependencies resolve **only into the isolated worktree**, not moving main
(which has experimental schema13). Four tests exercise exact quiet identity,
same-bend registration, all side slots/both hosts, growth/loop boundaries,
physical support and two rejected approaches. `--verify` reruns acceptance
without captures. `--measure` emits two JSON lines, validation then measurements.

The browser preloads two tracks with bounded concurrent image loading, displays
native and nearest-neighbor3× versions, and uses the recorded60fps timebase.
Its20s growth sweep is deliberately a linear-height fixture, not world ecology
or the presenter's real growth timing. It wraps the recording; it does not
assert the wind has a20s period. Native PNGs are authoritative; optional side-by-side
60fps H.264 clips are viewing conveniences made with:

```sh
ffmpeg -hide_banner -loglevel error -n -framerate 60 -i captures/vine-wind-retile-2026-09-13/interior-full-old/frame_%04d.png -framerate 60 -i captures/vine-wind-retile-2026-09-13/interior-full-new/frame_%04d.png -filter_complex hstack -c:v libx264 -preset fast -crf 0 -pix_fmt yuv444p captures/vine-wind-retile-2026-09-13/interior-full-pair-native60.mp4
```

Bulk assets/captures/builds remain under ignored `captures/`. `evidence.json`
preserves the small measured results. No physical-cube validation was performed.
