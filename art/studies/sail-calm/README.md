# Sail calm: two point-baked motion candidates

Isolated source-rig study; no production rig, atlas, sampler, host, core or live changes.
Read [the disposition](../../../design/7_Research/astra-sail-calm-study-2026-09-13.md).
`brace` is proposed for independent visual review; `settle` is retained as a rejected comparison.
All generated files go to **new** directories under `captures/`.

`bake.gd` deep-clones the current sail's animation library. It changes only left/right
fin rotation keys in rest/feed and always calls the existing point compositor.
It exports reviewable cloned scenes, separated body evidence, four animation rows and
complete copied packs. The original point bake must equal the shipped sail rows;
move/bud rows and all body/bud layer pixels must remain exact. There is no fin4 opt-in.

Dependencies deliberately point at the read-only schema12 source checkout
`captures/release-source/vine-endpoint-2026-09-13`, verified HEAD `3147775` for this run.
Do not repoint silently to moving schema13 main. The shared AA helper supplies the
unchanged renderer/metrics; this study authors motion, not another filter sweep.
The default binary does not compile the host; optional `meals` does.

## Reproduce

Run from the repository root. Replace output names with fresh names on repeat;
the regression fixture expects the bake path shown below.

```sh
./scripts/godot.sh --headless --script res://studies/sail-calm/bake.gd -- --out=/home/wrysk/wryskware/cubarium/captures/sail-calm-2026-09-13-bake
CARGO_TARGET_DIR=captures/build-cache/sail-calm cargo run --release --offline --manifest-path art/studies/sail-calm/Cargo.toml --bin sail_calm -- captures/sail-calm-2026-09-13-bake captures/sail-calm-2026-09-13-render-final
CARGO_TARGET_DIR=captures/build-cache/sail-calm cargo test --release --offline --manifest-path art/studies/sail-calm/Cargo.toml
```

The renderer writes 48 scenario records × three variants, 576 endpoint assertions,
24 six-second/60fps contact sheets, source-pose sheets and a 1× playback viewer.
The native canvas is deliberately not enlarged; the second canvas is diagnostic.
The six-second excerpt loops with a cut, not a claim of a six-second source period.

Actual meal example; repeat for `settle` and seeds 1/8, keeping each output separate:

```sh
CARGO_TARGET_DIR=captures/build-cache/sail-calm cargo run --release --offline --manifest-path art/studies/sail-calm/Cargo.toml --features meals --bin sail_calm_meals -- captures/hunter-openings-2026-09-13/seed-1/world-144000.cubw captures/sail-calm-2026-09-13-bake/original/pack captures/sail-calm-2026-09-13-bake/brace/pack captures/sail-calm-brace-meal-seed1-2026-09-13 brace
CARGO_TARGET_DIR=captures/build-cache/sail-calm cargo run --release --offline --manifest-path art/studies/sail-calm/Cargo.toml --bin sail_calm_contacts -- captures/sail-calm-brace-meal-seed1-2026-09-13 captures/sail-calm-settle-meal-seed1-2026-09-13 captures/sail-calm-meal-review-seed1-2026-09-13
```

The contact reducer checks all 933 original PNGs byte-for-byte across the independent
variant runs, identical observations/receipts/hashes, and chooses the first real sail
meal onset at/after care without hand-selecting a flattering animal. Rows are original,
brace, settle; columns are −1, 0, 1, 3, 6, 9, 15, 30 ticks from onset at fraction 2/3.
It also writes a three-way native full-net + tracking-detail playback viewer.
Tracking crop coordinates are diagnostic only; they never position a rendered animal.

## Review artifacts

- `captures/sail-calm-2026-09-13-render-final/viewer.html`: controlled all-state adult/.6, rooted/seam/rim clips.
- `captures/sail-calm-2026-09-13-render-final/source-{rest,feed}-4x.png`: crisp source definition, three rows.
- `captures/sail-calm-meal-review-seed{1,8}-2026-09-13/viewer.html`: actual crowded worlds; same controller and positions.
- Adjacent `meal-crops-native.png`, `meal-crops-6x.png`, `crowded-native.png`, `selection.json` are static/evidence alternatives.

Serve the repository/captures with a local static viewer if the browser restricts local
image loading. No tool here starts a server, accesses the cube or applies live care.
